use super::*;

fn bank_upgrade_text(bank_max_slots: u32) -> String {
    if bank_max_slots >= item::BANK_MAX_SIZE as u32 {
        "Upgrade slots (fully upgraded)".to_string()
    } else {
        format!(
            "Upgrade +{} slots ({}gp)",
            item::BANK_UPGRADE_SLOTS,
            item::BANK_UPGRADE_COST
        )
    }
}

fn bank_update_message(bank: &item::Bank) -> ServerMessage {
    ServerMessage::BankUpdate {
        slots: bank.to_update(),
        gold: bank.gold,
    }
}

fn inventory_update_message(player_id: &str, inventory: &Inventory) -> ServerMessage {
    ServerMessage::InventoryUpdate {
        player_id: player_id.to_string(),
        slots: inventory.to_update(),
        gold: inventory.gold,
    }
}

fn merge_or_swap_bank_slots(bank: &mut item::Bank, slot_a: usize, slot_b: usize) {
    let should_merge = match (&bank.slots[slot_a], &bank.slots[slot_b]) {
        (Some(slot_a), Some(slot_b)) => slot_a.item_id == slot_b.item_id,
        _ => false,
    };

    if should_merge {
        let src_qty = bank.slots[slot_a].as_ref().unwrap().quantity;
        bank.slots[slot_b].as_mut().unwrap().quantity = bank.slots[slot_b]
            .as_ref()
            .unwrap()
            .quantity
            .saturating_add(src_qty);
        bank.slots[slot_a] = None;
    } else {
        bank.slots.swap(slot_a, slot_b);
    }
}

fn sort_bank_slots(
    slots: &[Option<item::InventorySlot>],
    registry: &ItemRegistry,
) -> Vec<Option<item::InventorySlot>> {
    let mut merged = HashMap::<String, i64>::new();
    for slot in slots.iter().flatten() {
        *merged.entry(slot.item_id.clone()).or_insert(0) += slot.quantity as i64;
    }

    let mut items: Vec<item::InventorySlot> = merged
        .into_iter()
        .map(|(item_id, qty)| {
            let clamped = qty.min(i32::MAX as i64) as i32;
            item::InventorySlot::new(item_id, clamped)
        })
        .collect();

    items.sort_by(|a, b| {
        let def_a = registry.get(&a.item_id);
        let def_b = registry.get(&b.item_id);
        let cat_a = def_a.map(|d| d.category.sort_priority()).unwrap_or(255);
        let cat_b = def_b.map(|d| d.category.sort_priority()).unwrap_or(255);
        let name_a = def_a.map(|d| d.display_name.as_str()).unwrap_or(&a.item_id);
        let name_b = def_b.map(|d| d.display_name.as_str()).unwrap_or(&b.item_id);
        cat_a.cmp(&cat_b).then_with(|| name_a.cmp(name_b))
    });

    let mut sorted_slots: Vec<Option<item::InventorySlot>> = items.into_iter().map(Some).collect();
    sorted_slots.resize(slots.len(), None);
    sorted_slots
}

impl GameRoom {
    /// Send full bank contents to the player
    pub async fn handle_bank_open(&self, player_id: &str) {
        let players = self.players.read().await;
        let player = match players.get(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        let msg = ServerMessage::BankOpen {
            slots: player.bank.to_update(),
            gold: player.bank.gold,
            max_slots: player.bank_max_slots,
        };
        drop(players);
        self.send_to_player(player_id, msg).await;
    }

    /// Show banker dialogue menu with bank access and upgrade options
    pub(super) async fn show_banker_dialogue(&self, player_id: &str, npc_id: &str) {
        let npc_name = {
            let npcs = self.npcs.read().await;
            npcs.get(npc_id)
                .map(|n| {
                    self.entity_registry
                        .get(&n.prototype_id)
                        .map(|p| p.display_name.clone())
                        .unwrap_or_else(|| "Banker".to_string())
                })
                .unwrap_or_else(|| "Banker".to_string())
        };

        let (bank_max_slots, gold) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => (p.bank_max_slots, p.inventory.gold),
                _ => return,
            }
        };

        let text = format!(
            "Welcome to the bank! Your vault currently has {}/{} slots.\n\nYour gold: {}gp",
            bank_max_slots,
            item::BANK_MAX_SIZE,
            gold
        );

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("banker:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker: npc_name,
                text,
                choices: vec![
                    crate::protocol::DialogueChoice {
                        id: "open_bank".to_string(),
                        text: "Access my bank".to_string(),
                    },
                    crate::protocol::DialogueChoice {
                        id: "upgrade".to_string(),
                        text: bank_upgrade_text(bank_max_slots),
                    },
                    crate::protocol::DialogueChoice {
                        id: "close".to_string(),
                        text: "Nevermind".to_string(),
                    },
                ],
            },
        )
        .await;
    }

    /// Handle bank slot upgrade purchase
    pub(super) async fn handle_bank_upgrade(&self, player_id: &str, npc_id: &str) {
        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        if player.bank_max_slots >= item::BANK_MAX_SIZE as u32 {
            drop(players);
            self.send_system_message(player_id, "Your bank is already fully upgraded!")
                .await;
            self.show_banker_dialogue(player_id, npc_id).await;
            return;
        }

        if player.inventory.gold < item::BANK_UPGRADE_COST {
            let current_gold = player.inventory.gold;
            drop(players);
            self.send_system_message(
                player_id,
                &format!(
                    "You need {}gp to upgrade your bank. You only have {}gp.",
                    item::BANK_UPGRADE_COST,
                    current_gold
                ),
            )
            .await;
            self.show_banker_dialogue(player_id, npc_id).await;
            return;
        }

        player.inventory.gold -= item::BANK_UPGRADE_COST;
        player.bank_max_slots += item::BANK_UPGRADE_SLOTS as u32;
        player.bank.expand(item::BANK_UPGRADE_SLOTS);

        let inv_msg = inventory_update_message(player_id, &player.inventory);
        let new_slots = player.bank_max_slots;
        drop(players);

        self.send_to_player(player_id, inv_msg).await;
        self.send_system_message(
            player_id,
            &format!("Bank upgraded! You now have {} slots.", new_slots),
        )
        .await;
        self.show_banker_dialogue(player_id, npc_id).await;
    }

    /// Deposit an item from inventory into bank
    pub async fn handle_bank_deposit(&self, player_id: &str, item_id: &str, quantity: i32) {
        if quantity <= 0 {
            return;
        }

        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        if !player.inventory.has_item(item_id, quantity) {
            let msg = ServerMessage::BankResult {
                success: false,
                action: "deposit".to_string(),
                error: Some("Not enough items in inventory.".to_string()),
            };
            drop(players);
            self.send_to_player(player_id, msg).await;
            return;
        }

        if !player
            .bank
            .has_space_for(item_id, quantity, &self.item_registry)
        {
            let msg = ServerMessage::BankResult {
                success: false,
                action: "deposit".to_string(),
                error: Some("Bank is full.".to_string()),
            };
            drop(players);
            self.send_to_player(player_id, msg).await;
            return;
        }

        player.inventory.remove_item(item_id, quantity);
        player.bank.add_item(item_id, quantity, &self.item_registry);

        let inv_msg = inventory_update_message(player_id, &player.inventory);
        let bank_msg = bank_update_message(&player.bank);
        drop(players);
        self.send_to_player(player_id, inv_msg).await;
        self.send_to_player(player_id, bank_msg).await;
    }

    /// Withdraw an item from bank into inventory
    pub async fn handle_bank_withdraw(&self, player_id: &str, item_id: &str, quantity: i32) {
        if quantity <= 0 {
            return;
        }

        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        if !player.bank.has_item(item_id, quantity) {
            let msg = ServerMessage::BankResult {
                success: false,
                action: "withdraw".to_string(),
                error: Some("Not enough items in bank.".to_string()),
            };
            drop(players);
            self.send_to_player(player_id, msg).await;
            return;
        }

        if !player
            .inventory
            .has_space_for(item_id, quantity, &self.item_registry)
        {
            let msg = ServerMessage::BankResult {
                success: false,
                action: "withdraw".to_string(),
                error: Some("Inventory is full.".to_string()),
            };
            drop(players);
            self.send_to_player(player_id, msg).await;
            return;
        }

        player.bank.remove_item(item_id, quantity);
        player
            .inventory
            .add_item(item_id, quantity, &self.item_registry);

        let inv_msg = inventory_update_message(player_id, &player.inventory);
        let bank_msg = bank_update_message(&player.bank);
        drop(players);
        self.send_to_player(player_id, inv_msg).await;
        self.send_to_player(player_id, bank_msg).await;
    }

    /// Deposit gold from inventory into bank
    pub async fn handle_bank_deposit_gold(&self, player_id: &str, amount: i32) {
        if amount <= 0 {
            return;
        }

        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        if player.inventory.gold < amount {
            let msg = ServerMessage::BankResult {
                success: false,
                action: "depositGold".to_string(),
                error: Some("Not enough gold.".to_string()),
            };
            drop(players);
            self.send_to_player(player_id, msg).await;
            return;
        }

        player.inventory.gold -= amount;
        player.bank.gold += amount;

        let inv_msg = inventory_update_message(player_id, &player.inventory);
        let bank_msg = bank_update_message(&player.bank);
        drop(players);
        self.send_to_player(player_id, inv_msg).await;
        self.send_to_player(player_id, bank_msg).await;
    }

    /// Withdraw gold from bank into inventory
    pub async fn handle_bank_withdraw_gold(&self, player_id: &str, amount: i32) {
        if amount <= 0 {
            return;
        }

        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        if player.bank.gold < amount {
            let msg = ServerMessage::BankResult {
                success: false,
                action: "withdrawGold".to_string(),
                error: Some("Not enough gold in bank.".to_string()),
            };
            drop(players);
            self.send_to_player(player_id, msg).await;
            return;
        }

        player.bank.gold -= amount;
        player.inventory.gold += amount;

        let inv_msg = inventory_update_message(player_id, &player.inventory);
        let bank_msg = bank_update_message(&player.bank);
        drop(players);
        self.send_to_player(player_id, inv_msg).await;
        self.send_to_player(player_id, bank_msg).await;
    }

    /// Deposit all inventory items into bank
    pub async fn handle_bank_deposit_all(&self, player_id: &str) {
        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        let items_to_deposit: Vec<(String, i32)> = player
            .inventory
            .slots
            .iter()
            .filter_map(|slot| slot.as_ref().map(|s| (s.item_id.clone(), s.quantity)))
            .collect();

        if items_to_deposit.is_empty() {
            return;
        }

        for (item_id, quantity) in &items_to_deposit {
            if player
                .bank
                .has_space_for(item_id, *quantity, &self.item_registry)
            {
                player.inventory.remove_item(item_id, *quantity);
                player
                    .bank
                    .add_item(item_id, *quantity, &self.item_registry);
            }
        }

        let inv_msg = inventory_update_message(player_id, &player.inventory);
        let bank_msg = bank_update_message(&player.bank);
        drop(players);
        self.send_to_player(player_id, inv_msg).await;
        self.send_to_player(player_id, bank_msg).await;
    }

    /// Swap two bank slots. If both contain the same item, merge stacks.
    pub async fn handle_bank_swap_slots(&self, player_id: &str, slot_a: u32, slot_b: u32) {
        if slot_a == slot_b {
            return;
        }

        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        let len = player.bank.slots.len();
        let a = slot_a as usize;
        let b = slot_b as usize;
        if a >= len || b >= len {
            return;
        }

        merge_or_swap_bank_slots(&mut player.bank, a, b);

        let bank_msg = bank_update_message(&player.bank);
        drop(players);
        self.send_to_player(player_id, bank_msg).await;
    }

    /// Sort bank by item category then alphabetically by display name.
    pub async fn handle_bank_sort(&self, player_id: &str) {
        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        player.bank.slots = sort_bank_slots(&player.bank.slots, &self.item_registry);

        let bank_msg = bank_update_message(&player.bank);
        drop(players);
        self.send_to_player(player_id, bank_msg).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_item_registry() -> ItemRegistry {
        let temp_dir = TempDir::new().unwrap();
        let items_dir = temp_dir.path().join("items");
        std::fs::create_dir(&items_dir).unwrap();
        std::fs::write(
            items_dir.join("items.toml"),
            r#"
[bronze_sword]
display_name = "Bronze Sword"
category = "equipment"

[apple]
display_name = "Apple"
category = "consumable"

[oak_log]
display_name = "Oak Log"
category = "material"
"#,
        )
        .unwrap();

        let mut registry = ItemRegistry::new();
        registry.load_from_directory(temp_dir.path()).unwrap();
        registry
    }

    #[test]
    fn merge_or_swap_bank_slots_merges_matching_items_and_swaps_distinct_items() {
        let mut bank = item::Bank::new_with_size(4);
        bank.slots[0] = Some(item::InventorySlot::new("oak_log".to_string(), 2));
        bank.slots[1] = Some(item::InventorySlot::new("oak_log".to_string(), 5));

        merge_or_swap_bank_slots(&mut bank, 0, 1);

        assert!(bank.slots[0].is_none());
        assert_eq!(bank.slots[1].as_ref().unwrap().item_id, "oak_log");
        assert_eq!(bank.slots[1].as_ref().unwrap().quantity, 7);

        bank.slots[2] = Some(item::InventorySlot::new("apple".to_string(), 3));
        merge_or_swap_bank_slots(&mut bank, 1, 2);

        assert_eq!(bank.slots[1].as_ref().unwrap().item_id, "apple");
        assert_eq!(bank.slots[1].as_ref().unwrap().quantity, 3);
        assert_eq!(bank.slots[2].as_ref().unwrap().item_id, "oak_log");
        assert_eq!(bank.slots[2].as_ref().unwrap().quantity, 7);
    }

    #[test]
    fn sort_bank_slots_consolidates_duplicates_and_orders_by_category_then_name() {
        let registry = test_item_registry();
        let slots = vec![
            Some(item::InventorySlot::new("oak_log".to_string(), 2)),
            Some(item::InventorySlot::new("apple".to_string(), 3)),
            Some(item::InventorySlot::new("oak_log".to_string(), 4)),
            Some(item::InventorySlot::new("bronze_sword".to_string(), 1)),
            None,
        ];

        let sorted = sort_bank_slots(&slots, &registry);

        assert_eq!(sorted.len(), 5);
        assert_eq!(sorted[0].as_ref().unwrap().item_id, "bronze_sword");
        assert_eq!(sorted[0].as_ref().unwrap().quantity, 1);
        assert_eq!(sorted[1].as_ref().unwrap().item_id, "apple");
        assert_eq!(sorted[1].as_ref().unwrap().quantity, 3);
        assert_eq!(sorted[2].as_ref().unwrap().item_id, "oak_log");
        assert_eq!(sorted[2].as_ref().unwrap().quantity, 6);
        assert!(sorted[3].is_none());
        assert!(sorted[4].is_none());
    }

    #[test]
    fn bank_upgrade_text_reflects_upgrade_availability() {
        assert_eq!(
            bank_upgrade_text(item::BANK_MAX_SIZE as u32),
            "Upgrade slots (fully upgraded)"
        );
        assert_eq!(
            bank_upgrade_text(item::DEFAULT_BANK_SIZE as u32),
            format!(
                "Upgrade +{} slots ({}gp)",
                item::BANK_UPGRADE_SLOTS,
                item::BANK_UPGRADE_COST
            )
        );
    }
}
