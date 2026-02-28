use super::GameRoom;
use crate::data::ItemRegistry;
use crate::protocol::{ChestSlotUpdate, ServerMessage};
use std::collections::HashMap;

fn chest_position_prefix(interior_id: Option<&str>) -> String {
    match interior_id {
        Some(id) => format!("int_{}_", id),
        None => "ow_".to_string(),
    }
}

fn chest_positions_for_context(
    chests: &HashMap<String, crate::chest::ChestInstance>,
    interior_id: Option<&str>,
) -> Vec<(i32, i32)> {
    let prefix = chest_position_prefix(interior_id);
    chests
        .keys()
        .filter_map(|key| {
            if key.starts_with(&prefix) {
                let rest = &key[prefix.len()..];
                let parts: Vec<&str> = rest.splitn(2, '_').collect();
                if parts.len() == 2 {
                    if let (Ok(x), Ok(y)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                        return Some((x, y));
                    }
                }
            }
            None
        })
        .collect()
}

fn chest_slot_updates(
    chest: &crate::chest::ChestInstance,
    item_registry: &ItemRegistry,
) -> Vec<ChestSlotUpdate> {
    chest
        .slots
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| {
            slot.as_ref().map(|stack| {
                let base_price = item_registry
                    .get(&stack.item_id)
                    .map(|definition| definition.base_price)
                    .unwrap_or(0);
                ChestSlotUpdate {
                    slot: i as u8,
                    item_id: stack.item_id.clone(),
                    quantity: stack.quantity,
                    value: base_price * stack.quantity,
                }
            })
        })
        .collect()
}

impl GameRoom {
    pub async fn get_chest_positions_message(&self, interior_id: Option<&str>) -> ServerMessage {
        let chest_manager = self.chest_manager.read().await;
        let all_keys: Vec<&String> = chest_manager.chests.keys().collect();
        let prefix = chest_position_prefix(interior_id);
        tracing::debug!(
            "get_chest_positions_message: prefix='{}', all keys={:?}",
            prefix,
            all_keys
        );
        let positions = chest_positions_for_context(&chest_manager.chests, interior_id);
        tracing::info!("Chest positions for {:?}: {:?}", interior_id, positions);
        ServerMessage::ChestPositions { positions }
    }

    pub(in crate::game) async fn process_chest_respawns(&self) {
        let respawned_chests = {
            let mut chest_manager = self.chest_manager.write().await;
            chest_manager.tick_spawns(&self.chest_registry)
        };

        if respawned_chests.is_empty() {
            return;
        }

        let chest_manager = self.chest_manager.read().await;
        for chest_key in &respawned_chests {
            if let Some(chest) = chest_manager.get(chest_key) {
                if chest.viewers.is_empty() {
                    continue;
                }

                let slots = chest_slot_updates(chest, &self.item_registry);
                let total_value = chest.total_value(&self.item_registry);
                let viewer_ids: Vec<String> = chest.viewers.iter().cloned().collect();
                let msg = ServerMessage::ChestUpdate {
                    chest_id: chest_key.clone(),
                    slots,
                    total_value,
                };
                for viewer_id in &viewer_ids {
                    self.send_to_player(viewer_id, msg.clone()).await;
                }
            }
        }
    }

    /// Check if a chest exists at the given position and open it if so.
    /// Returns true if a chest was found (and opened), false otherwise.
    pub(in crate::game) async fn try_open_chest(&self, player_id: &str, x: i32, y: i32) -> bool {
        let instance_id = {
            let player_instances = self.player_instances.read().await;
            player_instances.get(player_id).cloned()
        };

        let chest_key = if let Some(ref instance_id) = instance_id {
            if let Some(instance) = self.instance_manager.get_by_instance_id(instance_id) {
                crate::chest::ChestManager::interior_key(&instance.map_id, x, y)
            } else {
                return false;
            }
        } else {
            crate::chest::ChestManager::overworld_key(x, y)
        };

        let exists = {
            let chest_manager = self.chest_manager.read().await;
            chest_manager.get(&chest_key).is_some()
        };

        if exists {
            self.handle_open_chest(player_id, x, y).await;
            true
        } else {
            false
        }
    }

    pub async fn handle_open_chest(&self, player_id: &str, x: i32, y: i32) {
        let instance_id = {
            let player_instances = self.player_instances.read().await;
            player_instances.get(player_id).cloned()
        };

        let chest_key = if let Some(ref instance_id) = instance_id {
            if let Some(instance) = self.instance_manager.get_by_instance_id(instance_id) {
                crate::chest::ChestManager::interior_key(&instance.map_id, x, y)
            } else {
                return;
            }
        } else {
            crate::chest::ChestManager::overworld_key(x, y)
        };

        let mut chest_manager = self.chest_manager.write().await;
        if let Some(chest) = chest_manager.get_mut(&chest_key) {
            chest.viewers.insert(player_id.to_string());
            let slots = chest_slot_updates(chest, &self.item_registry);
            let total_value = chest.total_value(&self.item_registry);
            let name = self
                .chest_registry
                .get(&chest.chest_def_id)
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| "Chest".to_string());

            let msg = ServerMessage::ChestOpen {
                chest_id: chest_key.clone(),
                name,
                slots,
                total_value,
            };
            drop(chest_manager);
            self.send_to_player(player_id, msg).await;

            self.player_open_chests
                .write()
                .await
                .insert(player_id.to_string(), chest_key);
        }
    }

    pub async fn handle_chest_take(&self, player_id: &str, chest_id: &str, slot: u8) {
        tracing::info!(
            "handle_chest_take: player={}, chest={}, slot={}",
            player_id,
            chest_id,
            slot
        );

        let taken_item = {
            let mut chest_manager = self.chest_manager.write().await;
            let chest = match chest_manager.get_mut(chest_id) {
                Some(chest) => chest,
                None => return,
            };

            if !chest.viewers.contains(player_id) {
                return;
            }

            let slot_idx = slot as usize;
            if slot_idx >= chest.slots.len() {
                return;
            }

            match chest.slots[slot_idx].take() {
                Some(item) => item,
                None => return,
            }
        };

        let add_success = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                if !player.inventory.has_space_for(
                    &taken_item.item_id,
                    taken_item.quantity,
                    &self.item_registry,
                ) {
                    false
                } else {
                    player.inventory.add_item(
                        &taken_item.item_id,
                        taken_item.quantity,
                        &self.item_registry,
                    );
                    let inv_update = ServerMessage::InventoryUpdate {
                        player_id: player_id.to_string(),
                        slots: player.inventory.to_update(),
                        gold: player.inventory.gold,
                    };
                    drop(players);
                    self.send_to_player(player_id, inv_update).await;
                    true
                }
            } else {
                false
            }
        };

        let mut chest_manager = self.chest_manager.write().await;
        let chest = match chest_manager.get_mut(chest_id) {
            Some(chest) => chest,
            None => return,
        };

        if !add_success {
            let slot_idx = slot as usize;
            if slot_idx < chest.slots.len() {
                chest.slots[slot_idx] = Some(taken_item);
            }
            return;
        }

        self.process_quest_item_collect(player_id, &taken_item.item_id, taken_item.quantity)
            .await;

        if let Some(definition) = self.chest_registry.get(&chest.chest_def_id) {
            if definition
                .spawn_items
                .iter()
                .any(|spawn| spawn.slot == slot)
            {
                chest.spawn_timers.insert(slot, std::time::Instant::now());
            }
        }

        let slots = chest_slot_updates(chest, &self.item_registry);
        let total_value = chest.total_value(&self.item_registry);
        let viewer_ids: Vec<String> = chest.viewers.iter().cloned().collect();
        let msg = ServerMessage::ChestUpdate {
            chest_id: chest_id.to_string(),
            slots,
            total_value,
        };
        drop(chest_manager);
        for viewer_id in &viewer_ids {
            self.send_to_player(viewer_id, msg.clone()).await;
        }
    }

    pub async fn handle_chest_deposit(&self, player_id: &str, chest_id: &str, inventory_slot: u8) {
        tracing::info!(
            "handle_chest_deposit: player={}, chest={}, slot={}",
            player_id,
            chest_id,
            inventory_slot
        );

        let taken_item = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(player) => player,
                None => return,
            };

            let inv_idx = inventory_slot as usize;
            if inv_idx >= player.inventory.slots.len() {
                return;
            }

            match player.inventory.slots[inv_idx].take() {
                Some(item) => item,
                None => return,
            }
        };

        let deposit_success = {
            let mut chest_manager = self.chest_manager.write().await;
            let chest = match chest_manager.get_mut(chest_id) {
                Some(chest) => chest,
                None => return,
            };

            if !chest.viewers.contains(player_id) {
                drop(chest_manager);
                let mut players = self.players.write().await;
                if let Some(player) = players.get_mut(player_id) {
                    let inv_idx = inventory_slot as usize;
                    if inv_idx < player.inventory.slots.len() {
                        player.inventory.slots[inv_idx] = Some(taken_item);
                    }
                }
                return;
            }

            let empty_slot = chest.slots.iter().position(|slot| slot.is_none());
            match empty_slot {
                Some(idx) => {
                    chest.slots[idx] = Some(crate::item::InventorySlot::new(
                        taken_item.item_id.clone(),
                        taken_item.quantity,
                    ));
                    true
                }
                None => false,
            }
        };

        if !deposit_success {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                let inv_idx = inventory_slot as usize;
                if inv_idx < player.inventory.slots.len() {
                    player.inventory.slots[inv_idx] = Some(taken_item);
                }
            }
            return;
        }

        {
            let players = self.players.read().await;
            if let Some(player) = players.get(player_id) {
                let inv_update = ServerMessage::InventoryUpdate {
                    player_id: player_id.to_string(),
                    slots: player.inventory.to_update(),
                    gold: player.inventory.gold,
                };
                drop(players);
                self.send_to_player(player_id, inv_update).await;
            }
        }

        let chest_manager = self.chest_manager.read().await;
        if let Some(chest) = chest_manager.get(chest_id) {
            let slots = chest_slot_updates(chest, &self.item_registry);
            let total_value = chest.total_value(&self.item_registry);
            let viewer_ids: Vec<String> = chest.viewers.iter().cloned().collect();
            let msg = ServerMessage::ChestUpdate {
                chest_id: chest_id.to_string(),
                slots,
                total_value,
            };
            drop(chest_manager);
            for viewer_id in &viewer_ids {
                self.send_to_player(viewer_id, msg.clone()).await;
            }
        }
    }

    pub async fn get_chest_save_data(&self) -> HashMap<String, String> {
        self.chest_manager.read().await.get_save_data()
    }

    pub async fn close_player_chest(&self, player_id: &str) {
        let chest_key = self.player_open_chests.write().await.remove(player_id);
        if let Some(key) = chest_key {
            let mut chest_manager = self.chest_manager.write().await;
            if let Some(chest) = chest_manager.get_mut(&key) {
                chest.viewers.remove(player_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chest::{ChestDef, ChestInstance};
    use crate::item::InventorySlot;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_items_dir() -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("isometric-game-chest-test-{suffix}"));
        fs::create_dir_all(dir.join("items")).unwrap();
        dir
    }

    fn test_item_registry() -> ItemRegistry {
        let dir = temp_items_dir();
        fs::write(
            dir.join("items/test_items.toml"),
            r#"
[coin_bag]
display_name = "Coin Bag"
base_price = 25
max_stack = 999
"#,
        )
        .unwrap();

        let mut registry = ItemRegistry::new();
        registry.load_from_directory(&dir).unwrap();
        fs::remove_dir_all(dir).unwrap();
        registry
    }

    #[test]
    fn chest_positions_for_context_filters_overworld_and_interior_keys() {
        let def = ChestDef {
            name: "Chest".to_string(),
            slots: 2,
            spawn_items: vec![],
        };
        let mut chests = HashMap::new();
        chests.insert(
            crate::chest::ChestManager::overworld_key(10, 20),
            ChestInstance::new("test_chest", &def),
        );
        chests.insert(
            crate::chest::ChestManager::interior_key("house", 4, 5),
            ChestInstance::new("test_chest", &def),
        );

        assert_eq!(chest_positions_for_context(&chests, None), vec![(10, 20)]);
        assert_eq!(
            chest_positions_for_context(&chests, Some("house")),
            vec![(4, 5)]
        );
    }

    #[test]
    fn chest_slot_updates_skip_empty_slots_and_compute_values() {
        let def = ChestDef {
            name: "Chest".to_string(),
            slots: 3,
            spawn_items: vec![],
        };
        let mut chest = ChestInstance::new("test_chest", &def);
        chest.slots[0] = Some(InventorySlot::new("coin_bag".to_string(), 3));
        chest.slots[2] = Some(InventorySlot::new("unknown_item".to_string(), 2));

        let updates = chest_slot_updates(&chest, &test_item_registry());

        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].slot, 0);
        assert_eq!(updates[0].item_id, "coin_bag");
        assert_eq!(updates[0].quantity, 3);
        assert_eq!(updates[0].value, 75);
        assert_eq!(updates[1].slot, 2);
        assert_eq!(updates[1].value, 0);
    }
}
