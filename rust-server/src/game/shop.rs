use super::*;
use crate::entity::prototype::MerchantConfig;
use crate::protocol::{ShopData, ShopStockItemData};
use crate::shop::ShopDefinition;

const SHOP_INTERACTION_DISTANCE: f32 = 2.5;

fn shop_price(base_price: i32, multiplier: f32) -> i32 {
    (base_price as f32 * multiplier).max(1.0) as i32
}

fn should_open_shop(
    is_merchant: bool,
    merchant_quest_met: bool,
    is_quest_giver: bool,
    has_quests: bool,
    all_quests_completed: bool,
) -> bool {
    is_merchant && merchant_quest_met && (!has_quests || !is_quest_giver || all_quests_completed)
}

fn build_shop_data(
    shop_def: &ShopDefinition,
    merchant_config: &MerchantConfig,
    item_registry: &ItemRegistry,
) -> ShopData {
    let stock = shop_def
        .stock
        .iter()
        .map(|item| {
            let base_price = item_registry
                .get(&item.item_id)
                .map(|def| def.base_price)
                .unwrap_or(10);

            ShopStockItemData {
                item_id: item.item_id.clone(),
                quantity: item.current_quantity,
                price: shop_price(base_price, merchant_config.sell_multiplier),
            }
        })
        .collect();

    ShopData {
        shop_id: shop_def.id.clone(),
        display_name: shop_def.display_name.clone(),
        buy_multiplier: merchant_config.buy_multiplier,
        sell_multiplier: merchant_config.sell_multiplier,
        crafting_categories: merchant_config.crafting_categories.clone(),
        crafting_stations: merchant_config.crafting_stations.clone(),
        stock,
    }
}

fn expand_stock_updates(
    npc_ids: &[String],
    changed_stock: &[(String, i32)],
) -> Vec<(String, String, i32)> {
    let mut updates = Vec::with_capacity(npc_ids.len() * changed_stock.len());
    for npc_id in npc_ids {
        for (item_id, quantity) in changed_stock {
            updates.push((npc_id.clone(), item_id.clone(), *quantity));
        }
    }
    updates
}

impl GameRoom {
    async fn player_can_use_merchant(&self, player_id: &str, entity_type: &str) -> bool {
        let Some(prototype) = self.entity_registry.get(entity_type) else {
            return false;
        };
        let Some(merchant) = prototype.merchant.as_ref() else {
            return false;
        };
        let quests = if prototype.behaviors.quest_giver {
            self.quest_registry.get_quests_for_npc(entity_type).await
        } else {
            Vec::new()
        };
        let quest_states = self.player_quest_states.read().await;
        let quest_state = quest_states.get(player_id);
        if merchant.required_quest.as_ref().is_some_and(|quest_id| {
            !quest_state.is_some_and(|state| state.is_quest_completed(quest_id))
        }) {
            return false;
        }

        if prototype.behaviors.quest_giver {
            if !quests.is_empty()
                && !quest_state.is_some_and(|state| {
                    quests
                        .iter()
                        .all(|quest| state.is_quest_completed(&quest.id))
                })
            {
                return false;
            }
        }
        true
    }

    pub(super) async fn try_open_merchant_shop(
        &self,
        player_id: &str,
        npc_id: &str,
        entity_type: &str,
    ) -> bool {
        let prototype = self.entity_registry.get(entity_type);
        let is_merchant = prototype
            .as_ref()
            .map(|proto| proto.behaviors.merchant || proto.behaviors.craftsman)
            .unwrap_or(false);

        if !is_merchant {
            return false;
        }

        let quests = self.quest_registry.get_quests_for_npc(entity_type).await;
        let is_quest_giver = prototype
            .as_ref()
            .map(|proto| proto.behaviors.quest_giver)
            .unwrap_or(false);
        let has_quests = !quests.is_empty();

        let all_quests_completed = if is_quest_giver && has_quests {
            let quest_states = self.player_quest_states.read().await;
            quest_states
                .get(player_id)
                .map(|quest_state| {
                    quests
                        .iter()
                        .all(|quest| quest_state.is_quest_completed(&quest.id))
                })
                .unwrap_or(false)
        } else {
            false
        };

        let merchant_quest_met = if let Some(proto) = prototype.as_ref() {
            if let Some(merchant_config) = &proto.merchant {
                if let Some(required_quest) = merchant_config.required_quest.as_deref() {
                    let quest_states = self.player_quest_states.read().await;
                    quest_states
                        .get(player_id)
                        .map(|state| state.is_quest_completed(required_quest))
                        .unwrap_or(false)
                } else {
                    true
                }
            } else {
                true
            }
        } else {
            true
        };

        if !should_open_shop(
            is_merchant,
            merchant_quest_met,
            is_quest_giver,
            has_quests,
            all_quests_completed,
        ) {
            return false;
        }

        tracing::info!(
            "Player {} opening shop with NPC {} ({})",
            player_id,
            npc_id,
            entity_type
        );

        // Stop any in-progress gathering/woodcutting action
        self.handle_stop_gathering(player_id).await;

        if let Some(proto) = prototype {
            if let Some(merchant_config) = &proto.merchant {
                let shop_registry = self.shop_registry.read().await;
                if let Some(shop_def) = shop_registry.get(&merchant_config.shop_id) {
                    let msg = ServerMessage::ShopData {
                        npc_id: npc_id.to_string(),
                        shop: build_shop_data(shop_def, merchant_config, &self.item_registry),
                    };
                    drop(shop_registry);
                    self.send_to_player(player_id, msg).await;
                    return true;
                }

                tracing::warn!(
                    "Shop '{}' not found for merchant NPC {}",
                    merchant_config.shop_id,
                    npc_id
                );
            }
        }

        self.send_to_player(
            player_id,
            ServerMessage::ShopOpen {
                npc_id: npc_id.to_string(),
            },
        )
        .await;
        true
    }

    async fn merchant_npc_context(
        &self,
        player_id: &str,
        npc_id: &str,
    ) -> Option<(String, f32, bool)> {
        let (player_x, player_y) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) if player.active && !player.is_dead => (player.x, player.y),
                _ => return None,
            }
        };

        let instance_id = {
            let instances = self.player_instances.read().await;
            instances.get(player_id).cloned()
        };

        if let Some(inst_id) = instance_id {
            if let Some(instance) = self.instance_manager.find_player_instance(player_id).await {
                let npcs = instance.npcs.read().await;
                npcs.get(npc_id).map(|npc| {
                    let dx = (npc.x - player_x) as f32;
                    let dy = (npc.y - player_y) as f32;
                    let distance = (dx * dx + dy * dy).sqrt();
                    (npc.prototype_id.clone(), distance, npc.is_alive())
                })
            } else {
                tracing::warn!(
                    "Player {} in instance {} but instance not found",
                    player_id,
                    inst_id
                );
                None
            }
        } else {
            let npcs = self.npcs.read().await;
            npcs.get(npc_id).map(|npc| {
                let dx = (npc.x - player_x) as f32;
                let dy = (npc.y - player_y) as f32;
                let distance = (dx * dx + dy * dy).sqrt();
                (npc.prototype_id.clone(), distance, npc.is_alive())
            })
        }
    }

    async fn send_shop_result(
        &self,
        player_id: &str,
        success: bool,
        action: &str,
        item_id: &str,
        quantity: i32,
        gold_change: i32,
        error: Option<&str>,
    ) {
        self.send_to_player(
            player_id,
            ServerMessage::ShopResult {
                success,
                action: action.to_string(),
                item_id: item_id.to_string(),
                quantity,
                gold_change,
                error: error.map(|message| message.to_string()),
            },
        )
        .await;
    }

    async fn broadcast_shop_stock_update(&self, npc_id: &str, item_id: &str, new_quantity: i32) {
        self.broadcast(ServerMessage::ShopStockUpdate {
            npc_id: npc_id.to_string(),
            item_id: item_id.to_string(),
            new_quantity,
        })
        .await;
    }

    pub async fn handle_shop_buy(
        &self,
        player_id: &str,
        npc_id: &str,
        item_id: &str,
        quantity: i32,
    ) {
        if quantity <= 0 {
            self.send_shop_result(
                player_id,
                false,
                "buy",
                item_id,
                0,
                0,
                Some("Invalid quantity"),
            )
            .await;
            return;
        }

        let player_gold = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) if player.active && !player.is_dead => player.inventory.gold,
                _ => return,
            }
        };

        let (prototype_id, distance, is_alive) =
            match self.merchant_npc_context(player_id, npc_id).await {
                Some(context) => context,
                None => {
                    self.send_shop_result(
                        player_id,
                        false,
                        "buy",
                        item_id,
                        0,
                        0,
                        Some("NPC not found"),
                    )
                    .await;
                    return;
                }
            };

        if distance > SHOP_INTERACTION_DISTANCE || !is_alive {
            self.send_shop_result(
                player_id,
                false,
                "buy",
                item_id,
                0,
                0,
                Some("Too far from merchant"),
            )
            .await;
            return;
        }

        let merchant_config = match self.entity_registry.get(&prototype_id) {
            Some(proto) => match &proto.merchant {
                Some(config) => config.clone(),
                None => {
                    self.send_shop_result(
                        player_id,
                        false,
                        "buy",
                        item_id,
                        0,
                        0,
                        Some("Not a merchant"),
                    )
                    .await;
                    return;
                }
            },
            None => {
                self.send_shop_result(
                    player_id,
                    false,
                    "buy",
                    item_id,
                    0,
                    0,
                    Some("Invalid merchant"),
                )
                .await;
                return;
            }
        };
        if !self.player_can_use_merchant(player_id, &prototype_id).await {
            self.send_shop_result(
                player_id,
                false,
                "buy",
                item_id,
                0,
                0,
                Some("Merchant access is locked"),
            )
            .await;
            return;
        }

        let mut shop_registry = self.shop_registry.write().await;
        let shop = match shop_registry.get_mut(&merchant_config.shop_id) {
            Some(shop) => shop,
            None => {
                drop(shop_registry);
                self.send_shop_result(
                    player_id,
                    false,
                    "buy",
                    item_id,
                    0,
                    0,
                    Some("Shop not found"),
                )
                .await;
                return;
            }
        };

        let stock_item = match shop.get_stock_mut(item_id) {
            Some(stock_item) => stock_item,
            None => {
                drop(shop_registry);
                self.send_shop_result(
                    player_id,
                    false,
                    "buy",
                    item_id,
                    0,
                    0,
                    Some("Item not sold here"),
                )
                .await;
                return;
            }
        };

        if stock_item.current_quantity < quantity {
            drop(shop_registry);
            self.send_shop_result(
                player_id,
                false,
                "buy",
                item_id,
                0,
                0,
                Some("Insufficient stock"),
            )
            .await;
            return;
        }

        let item_def = match self.item_registry.get(item_id) {
            Some(def) => def.clone(),
            None => {
                drop(shop_registry);
                self.send_shop_result(
                    player_id,
                    false,
                    "buy",
                    item_id,
                    0,
                    0,
                    Some("Item not found"),
                )
                .await;
                return;
            }
        };

        let unit_price = shop_price(item_def.base_price, merchant_config.sell_multiplier);
        let Some(total_cost) = item::checked_gold_total(unit_price, quantity) else {
            drop(shop_registry);
            self.send_shop_result(
                player_id,
                false,
                "buy",
                item_id,
                0,
                0,
                Some("Invalid transaction total"),
            )
            .await;
            return;
        };

        if item::checked_gold_debit(player_gold, total_cost).is_none() {
            drop(shop_registry);
            self.send_shop_result(
                player_id,
                false,
                "buy",
                item_id,
                0,
                0,
                Some("Not enough gold"),
            )
            .await;
            return;
        }

        {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(player) if player.active && !player.is_dead => player,
                _ => {
                    drop(shop_registry);
                    return;
                }
            };

            if !player
                .inventory
                .has_space_for(item_id, quantity, &self.item_registry)
            {
                drop(shop_registry);
                self.send_shop_result(
                    player_id,
                    false,
                    "buy",
                    item_id,
                    0,
                    0,
                    Some("Inventory full"),
                )
                .await;
                return;
            }
        }

        let mut players = self.players.write().await;
        let Some(player) = players.get_mut(player_id) else {
            return;
        };
        let Some(new_gold) = item::checked_gold_debit(player.inventory.gold, total_cost) else {
            drop(players);
            drop(shop_registry);
            self.send_shop_result(
                player_id,
                false,
                "buy",
                item_id,
                0,
                0,
                Some("Not enough gold"),
            )
            .await;
            return;
        };
        if !player
            .inventory
            .has_space_for(item_id, quantity, &self.item_registry)
        {
            drop(players);
            drop(shop_registry);
            self.send_shop_result(
                player_id,
                false,
                "buy",
                item_id,
                0,
                0,
                Some("Inventory full"),
            )
            .await;
            return;
        }

        stock_item.current_quantity -= quantity;
        let new_stock = stock_item.current_quantity;
        player.inventory.gold = new_gold;
        player
            .inventory
            .add_item(item_id, quantity, &self.item_registry);

        let inventory_update = player.inventory.to_update();
        let gold = player.inventory.gold;
        drop(players);
        drop(shop_registry);

        self.send_shop_result(player_id, true, "buy", item_id, quantity, -total_cost, None)
            .await;
        self.send_to_player(
            player_id,
            ServerMessage::InventoryUpdate {
                player_id: player_id.to_string(),
                slots: inventory_update,
                gold,
            },
        )
        .await;
        self.broadcast_shop_stock_update(npc_id, item_id, new_stock)
            .await;

        tracing::info!(
            "Player {} bought {}x{} from {} for {} gold",
            player_id,
            quantity,
            item_id,
            npc_id,
            total_cost
        );
    }

    pub async fn handle_shop_sell(
        &self,
        player_id: &str,
        npc_id: &str,
        item_id: &str,
        quantity: i32,
    ) {
        if quantity <= 0 {
            self.send_shop_result(
                player_id,
                false,
                "sell",
                item_id,
                0,
                0,
                Some("Invalid quantity"),
            )
            .await;
            return;
        }

        let (prototype_id, distance, is_alive) =
            match self.merchant_npc_context(player_id, npc_id).await {
                Some(context) => context,
                None => {
                    self.send_shop_result(
                        player_id,
                        false,
                        "sell",
                        item_id,
                        0,
                        0,
                        Some("NPC not found"),
                    )
                    .await;
                    return;
                }
            };

        if distance > SHOP_INTERACTION_DISTANCE || !is_alive {
            self.send_shop_result(
                player_id,
                false,
                "sell",
                item_id,
                0,
                0,
                Some("Too far from merchant"),
            )
            .await;
            return;
        }

        let merchant_config = match self.entity_registry.get(&prototype_id) {
            Some(proto) => match &proto.merchant {
                Some(config) => config.clone(),
                None => {
                    self.send_shop_result(
                        player_id,
                        false,
                        "sell",
                        item_id,
                        0,
                        0,
                        Some("Not a merchant"),
                    )
                    .await;
                    return;
                }
            },
            None => {
                self.send_shop_result(
                    player_id,
                    false,
                    "sell",
                    item_id,
                    0,
                    0,
                    Some("Invalid merchant"),
                )
                .await;
                return;
            }
        };
        if !self.player_can_use_merchant(player_id, &prototype_id).await {
            self.send_shop_result(
                player_id,
                false,
                "sell",
                item_id,
                0,
                0,
                Some("Merchant access is locked"),
            )
            .await;
            return;
        }

        let item_def = match self.item_registry.get(item_id) {
            Some(def) => def.clone(),
            None => {
                self.send_shop_result(
                    player_id,
                    false,
                    "sell",
                    item_id,
                    0,
                    0,
                    Some("Item not found"),
                )
                .await;
                return;
            }
        };

        if !item_def.sellable {
            self.send_shop_result(
                player_id,
                false,
                "sell",
                item_id,
                0,
                0,
                Some("Item cannot be sold"),
            )
            .await;
            return;
        }

        let has_item = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) if player.active && !player.is_dead => {
                    player.inventory.has_item(item_id, quantity)
                }
                _ => false,
            }
        };

        if !has_item {
            self.send_shop_result(
                player_id,
                false,
                "sell",
                item_id,
                0,
                0,
                Some("You don't have enough of that item"),
            )
            .await;
            return;
        }

        let unit_price = shop_price(item_def.base_price, merchant_config.buy_multiplier);
        let Some(total_value) = item::checked_gold_total(unit_price, quantity) else {
            self.send_shop_result(
                player_id,
                false,
                "sell",
                item_id,
                0,
                0,
                Some("Invalid transaction total"),
            )
            .await;
            return;
        };

        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            let Some(new_gold) = item::checked_gold_credit(player.inventory.gold, total_value)
            else {
                drop(players);
                self.send_shop_result(
                    player_id,
                    false,
                    "sell",
                    item_id,
                    0,
                    0,
                    Some("Gold limit reached"),
                )
                .await;
                return;
            };
            player.inventory.remove_item(item_id, quantity);
            player.inventory.gold = new_gold;

            let inventory_update = player.inventory.to_update();
            let gold = player.inventory.gold;
            drop(players);

            self.send_shop_result(
                player_id,
                true,
                "sell",
                item_id,
                quantity,
                total_value,
                None,
            )
            .await;
            self.send_to_player(
                player_id,
                ServerMessage::InventoryUpdate {
                    player_id: player_id.to_string(),
                    slots: inventory_update,
                    gold,
                },
            )
            .await;

            tracing::info!(
                "Player {} sold {}x{} to {} for {} gold",
                player_id,
                quantity,
                item_id,
                npc_id,
                total_value
            );
        }
    }

    pub(super) async fn restock_shops(&self) {
        let mut shop_registry = self.shop_registry.write().await;
        let npcs = self.npcs.read().await;
        let mut restocked_shops = Vec::new();

        for proto in self.entity_registry.all() {
            let Some(merchant_config) = &proto.merchant else {
                continue;
            };

            if merchant_config.restock_interval_minutes.is_none() {
                continue;
            }

            if let Some(shop) = shop_registry.get_mut(&merchant_config.shop_id) {
                let changed_stock = shop.restock();
                if changed_stock.is_empty() {
                    continue;
                }

                let npc_ids: Vec<String> = npcs
                    .iter()
                    .filter(|(_, npc)| npc.prototype_id == proto.id)
                    .map(|(npc_id, _)| npc_id.clone())
                    .collect();
                restocked_shops.extend(expand_stock_updates(&npc_ids, &changed_stock));

                tracing::info!(
                    "Restocked shop '{}' for entity type '{}' ({} stock entries changed)",
                    merchant_config.shop_id,
                    proto.id,
                    changed_stock.len(),
                );
            }
        }

        drop(shop_registry);
        drop(npcs);

        for (npc_id, item_id, quantity) in restocked_shops {
            self.broadcast_shop_stock_update(&npc_id, &item_id, quantity)
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::prototype::MerchantConfig;
    use crate::shop::ShopStockItem;
    use tempfile::TempDir;

    fn test_item_registry() -> ItemRegistry {
        let temp_dir = TempDir::new().unwrap();
        let items_dir = temp_dir.path().join("items");
        std::fs::create_dir(&items_dir).unwrap();
        std::fs::write(
            items_dir.join("items.toml"),
            r#"
[apple]
display_name = "Apple"
category = "consumable"
base_price = 12

[oak_log]
display_name = "Oak Log"
category = "material"
base_price = 3
"#,
        )
        .unwrap();

        let mut registry = ItemRegistry::new();
        registry.load_from_directory(temp_dir.path()).unwrap();
        registry
    }

    fn merchant_config() -> MerchantConfig {
        MerchantConfig {
            shop_id: "general_store".to_string(),
            buy_multiplier: 0.5,
            sell_multiplier: 1.5,
            restock_interval_minutes: Some(5),
            crafting_categories: vec!["crafting".to_string()],
            crafting_stations: vec!["workbench".to_string()],
            required_quest: None,
        }
    }

    #[test]
    fn should_open_shop_only_when_merchant_access_rules_pass() {
        assert!(should_open_shop(true, true, false, false, false));
        assert!(should_open_shop(true, true, true, true, true));
        assert!(!should_open_shop(false, true, false, false, false));
        assert!(!should_open_shop(true, false, false, false, false));
        assert!(!should_open_shop(true, true, true, true, false));
    }

    #[test]
    fn build_shop_data_uses_current_stock_and_sell_multiplier() {
        let registry = test_item_registry();
        let shop_def = ShopDefinition {
            id: "general_store".to_string(),
            display_name: "General Store".to_string(),
            stock: vec![
                ShopStockItem {
                    item_id: "apple".to_string(),
                    max_quantity: 10,
                    restock_rate: 1,
                    current_quantity: 7,
                },
                ShopStockItem {
                    item_id: "mystery_item".to_string(),
                    max_quantity: 2,
                    restock_rate: 1,
                    current_quantity: 1,
                },
            ],
        };

        let shop = build_shop_data(&shop_def, &merchant_config(), &registry);

        assert_eq!(shop.shop_id, "general_store");
        assert_eq!(shop.display_name, "General Store");
        assert_eq!(shop.buy_multiplier, 0.5);
        assert_eq!(shop.sell_multiplier, 1.5);
        assert_eq!(shop.crafting_categories, vec!["crafting"]);
        assert_eq!(shop.crafting_stations, vec!["workbench"]);
        assert_eq!(shop.stock[0].item_id, "apple");
        assert_eq!(shop.stock[0].quantity, 7);
        assert_eq!(shop.stock[0].price, 18);
        assert_eq!(shop.stock[1].item_id, "mystery_item");
        assert_eq!(shop.stock[1].price, 15);
    }

    #[test]
    fn expand_stock_updates_creates_broadcast_event_for_each_npc_and_item() {
        let npc_ids = vec!["merchant_a".to_string(), "merchant_b".to_string()];
        let changed_stock = vec![("apple".to_string(), 5), ("oak_log".to_string(), 2)];

        let updates = expand_stock_updates(&npc_ids, &changed_stock);

        assert_eq!(updates.len(), 4);
        assert!(updates.contains(&("merchant_a".to_string(), "apple".to_string(), 5)));
        assert!(updates.contains(&("merchant_a".to_string(), "oak_log".to_string(), 2)));
        assert!(updates.contains(&("merchant_b".to_string(), "apple".to_string(), 5)));
        assert!(updates.contains(&("merchant_b".to_string(), "oak_log".to_string(), 2)));
    }
}
