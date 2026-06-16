use super::*;
use crate::protocol::{FarmingPatchData, TileOverride};

/// Per-swing chance a farmed tree falls when chopped (gives roughly 1–10 chops).
const FARM_TREE_FELL_CHANCE: f32 = 0.30;

impl GameRoom {
    async fn can_interact_with_farming_patch(&self, player_id: &str, patch_id: &str) -> bool {
        if self.player_instances.read().await.contains_key(player_id) {
            return false;
        }
        let footprint = {
            let farming = self.farming.read().await;
            farming
                .patches
                .get(patch_id)
                .map(|patch| (patch.x, patch.y, patch.width as i32, patch.height as i32))
        };
        let Some((px0, py0, w, h)) = footprint else {
            return false;
        };
        let players = self.players.read().await;
        players.get(player_id).is_some_and(|player| {
            // Distance to the nearest footprint tile (Chebyshev), within 2 tiles.
            let cx = player.x.clamp(px0, px0 + w - 1);
            let cy = player.y.clamp(py0, py0 + h - 1);
            player.active
                && !player.is_dead
                && (player.x - cx).abs() <= 2
                && (player.y - cy).abs() <= 2
        })
    }

    pub async fn handle_plant_seed(&self, player_id: &str, patch_id: &str, item_id: &str) {
        if !self
            .can_interact_with_farming_patch(player_id, patch_id)
            .await
        {
            tracing::warn!(
                "Rejected remote farming action from {} for patch {}",
                player_id,
                patch_id
            );
            return;
        }
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // A multi-tile bed needs one seed per plant (its capacity).
        let patch_capacity = {
            let farming = self.farming.read().await;
            farming
                .patches
                .get(patch_id)
                .map(|p| p.capacity.max(1))
                .unwrap_or(1)
        };

        let farming_level = {
            let players = self.players.read().await;
            let Some(player) = players.get(player_id) else {
                return;
            };
            if player.inventory.has_item(item_id, patch_capacity as i32) {
                Some(player.skills.farming.level)
            } else {
                None
            }
        };

        let Some(farming_level) = farming_level else {
            let message = if patch_capacity > 1 {
                format!("You need {} seeds to plant this bed.", patch_capacity)
            } else {
                "You don't have that seed".to_string()
            };
            self.send_to_player(player_id, ServerMessage::Error { code: 400, message })
                .await;
            return;
        };

        let result = {
            let mut farming = self.farming.write().await;
            farming.plant_seed(patch_id, item_id, player_id, farming_level, current_time)
        };

        match result {
            Ok((crop_id, xp, capacity)) => {
                let (inv_msg, total_xp, level, leveled_up) = {
                    let mut players = self.players.write().await;
                    let Some(player) = players.get_mut(player_id) else {
                        return;
                    };
                    player.inventory.remove_item(item_id, capacity as i32);
                    let leveled_up = player.skills.farming.add_xp(xp);
                    let total_xp = player.skills.farming.xp;
                    let level = player.skills.farming.level;
                    (
                        inventory_update_message(player_id, &player.inventory),
                        total_xp,
                        level,
                        leveled_up,
                    )
                };

                self.send_to_player(player_id, inv_msg).await;
                self.send_to_player(
                    player_id,
                    farming_xp_message(player_id, xp, total_xp, level),
                )
                .await;

                if leveled_up {
                    self.broadcast_skill_level_up(player_id, "farming", level)
                        .await;
                    self.process_quest_progression_snapshot(player_id).await;
                }

                let update = {
                    let farming = self.farming.read().await;
                    farming.patch_update_for(patch_id, player_id, current_time)
                };

                if let Some(ref db) = self.db
                    && let Err(e) = db
                        .save_farming_patch(
                            patch_id,
                            player_id,
                            &crop_id,
                            current_time,
                            update.lives_remaining,
                            &update.health,
                            update.composted,
                            0,
                        )
                        .await
                {
                    tracing::warn!("Failed to save farming patch {}: {}", patch_id, e);
                }

                self.send_to_player(player_id, patch_update_message(&update))
                    .await;

                self.process_quest_item_collect(player_id, &format!("plant_{}", item_id), 1)
                    .await;
            }
            Err(message) => {
                self.send_to_player(player_id, ServerMessage::Error { code: 400, message })
                    .await;
            }
        }
    }

    pub async fn handle_harvest_crop(&self, player_id: &str, patch_id: &str) {
        if !self
            .can_interact_with_farming_patch(player_id, patch_id)
            .await
        {
            tracing::warn!(
                "Rejected remote farming action from {} for patch {}",
                player_id,
                patch_id
            );
            return;
        }
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Trees aren't grabbed like crops — they must be chopped down.
        let is_tree = {
            let farming = self.farming.read().await;
            farming
                .patches
                .get(patch_id)
                .map(|p| p.patch_type == "tree")
                .unwrap_or(false)
        };
        if is_tree {
            self.send_system_message(player_id, "You need to chop this tree down.")
                .await;
            return;
        }

        let inventory_full = {
            let farming = self.farming.read().await;
            let players = self.players.read().await;
            let key = (patch_id.to_string(), player_id.to_string());
            farming
                .player_states
                .get(&key)
                .and_then(|state| farming.crops.get(&state.crop_id))
                .map(|crop| {
                    players.get(player_id).is_some_and(|player| {
                        !player.inventory.has_space_for(
                            &crop.produce_item,
                            crop.harvest_amount_max,
                            &self.item_registry,
                        )
                    })
                })
                .unwrap_or(false)
        };

        if inventory_full {
            self.send_system_message(player_id, "Your inventory is full.")
                .await;
            return;
        }

        let result = {
            let mut farming = self.farming.write().await;
            farming.harvest_crop(patch_id, player_id, current_time)
        };

        match result {
            Ok(harvest) => {
                self.apply_harvest_reward(player_id, patch_id, harvest, current_time)
                    .await;
            }
            Err(message) => {
                self.send_to_player(player_id, ServerMessage::Error { code: 400, message })
                    .await;
            }
        }
    }

    /// Grant the produce, farming XP, and patch/DB updates for a successful harvest.
    /// Shared by the normal harvest action and chopping down a mature tree.
    async fn apply_harvest_reward(
        &self,
        player_id: &str,
        patch_id: &str,
        harvest: crate::farming::HarvestResult,
        current_time: u64,
    ) {
        let (inv_msg, total_xp, level, leveled_up) = {
            let mut players = self.players.write().await;
            let Some(player) = players.get_mut(player_id) else {
                return;
            };

            player
                .inventory
                .add_item(&harvest.produce_item, harvest.amount, &self.item_registry);
            if harvest.seed_returned {
                player
                    .inventory
                    .add_item(&harvest.seed_item, 1, &self.item_registry);
            }

            let leveled_up = player.skills.farming.add_xp(harvest.xp_gained);
            let total_xp = player.skills.farming.xp;
            let level = player.skills.farming.level;

            (
                inventory_update_message(player_id, &player.inventory),
                total_xp,
                level,
                leveled_up,
            )
        };

        self.send_to_player(player_id, inv_msg).await;
        self.send_to_player(
            player_id,
            farming_xp_message(player_id, harvest.xp_gained, total_xp, level),
        )
        .await;

        if leveled_up {
            self.broadcast_skill_level_up(player_id, "farming", level)
                .await;
            self.process_quest_progression_snapshot(player_id).await;
        }

        let update = {
            let farming = self.farming.read().await;
            farming.patch_update_for(patch_id, player_id, current_time)
        };
        self.send_to_player(player_id, patch_update_message(&update))
            .await;

        if let Some(ref db) = self.db {
            if harvest.patch_emptied {
                if let Err(e) = db.delete_farming_patch(patch_id, player_id).await {
                    tracing::warn!("Failed to delete farming patch {}: {}", patch_id, e);
                }
            } else if let Err(e) = db
                .update_farming_patch_lives(patch_id, player_id, harvest.lives_remaining)
                .await
            {
                tracing::warn!("Failed to update farming patch lives {}: {}", patch_id, e);
            }
        }

        self.process_quest_item_collect(
            player_id,
            &format!("harvest_{}", harvest.produce_item),
            harvest.amount,
        )
        .await;
        self.process_quest_item_collect(player_id, &harvest.produce_item, harvest.amount)
            .await;

        self.record_resource_contract_progress(player_id, &harvest.produce_item, harvest.amount)
            .await;
    }

    /// One swing at a mature farmed tree. Repeated by the auto-action loop until the
    /// tree falls, at which point it yields its produce + Farming XP (like a harvest).
    pub async fn handle_chop_farm_tree(&self, player_id: &str, patch_id: &str) {
        if !self
            .can_interact_with_farming_patch(player_id, patch_id)
            .await
        {
            self.clear_auto_action(player_id, "interrupted").await;
            return;
        }
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Must still be a mature, healthy tree.
        let patch_pos = {
            let farming = self.farming.read().await;
            let key = (patch_id.to_string(), player_id.to_string());
            let is_tree = farming
                .patches
                .get(patch_id)
                .map(|p| p.patch_type == "tree")
                .unwrap_or(false);
            let mature = farming
                .player_states
                .get(&key)
                .map(|s| s.is_harvestable(&farming.crops, current_time))
                .unwrap_or(false);
            if is_tree && mature {
                farming.patches.get(patch_id).map(|p| (p.x, p.y))
            } else {
                None
            }
        };
        let Some((px, py)) = patch_pos else {
            self.clear_auto_action(player_id, "target_depleted").await;
            return;
        };

        // Stop if there's no room for the logs.
        let inventory_full = {
            let farming = self.farming.read().await;
            let players = self.players.read().await;
            let key = (patch_id.to_string(), player_id.to_string());
            farming
                .player_states
                .get(&key)
                .and_then(|state| farming.crops.get(&state.crop_id))
                .map(|crop| {
                    players.get(player_id).is_some_and(|player| {
                        !player.inventory.has_space_for(
                            &crop.produce_item,
                            crop.harvest_amount_max,
                            &self.item_registry,
                        )
                    })
                })
                .unwrap_or(false)
        };
        if inventory_full {
            self.send_system_message(player_id, "Your inventory is full.")
                .await;
            self.clear_auto_action(player_id, "inventory_full").await;
            return;
        }

        // Claim the swing cooldown atomically (mirrors handle_chop_tree).
        {
            let mut players = self.players.write().await;
            let Some(player) = players.get_mut(player_id) else {
                return;
            };
            if player.is_dead {
                return;
            }
            if current_time.saturating_sub(player.last_attack_time) < ATTACK_COOLDOWN_MS {
                return;
            }
            player.last_attack_time = current_time;
        }

        // Chopping a farmed tree requires an axe, just like a wild tree.
        let has_axe = {
            let players = self.players.read().await;
            players.get(player_id).is_some_and(|p| {
                p.equipped_weapon
                    .as_ref()
                    .and_then(|w| self.item_registry.get(w))
                    .and_then(|i| i.equipment.as_ref())
                    .map(|e| e.chop_speed_multiplier > 0.0)
                    .unwrap_or(false)
            })
        };
        if !has_axe {
            self.send_to_player(
                player_id,
                ServerMessage::Error {
                    code: 400,
                    message: "You need an axe to chop trees".to_string(),
                },
            )
            .await;
            self.clear_auto_action(player_id, "no_axe").await;
            return;
        }

        // Swing: animation + sfx + shake for everyone nearby (reuses woodcutting visuals).
        self.broadcast_to_zone(
            player_id,
            ServerMessage::WoodcuttingSwing {
                player_id: player_id.to_string(),
                tree_x: px,
                tree_y: py,
            },
        )
        .await;

        // Roll whether the tree comes down this swing.
        if rand::random::<f32>() < FARM_TREE_FELL_CHANCE {
            let result = {
                let mut farming = self.farming.write().await;
                farming.harvest_crop(patch_id, player_id, current_time)
            };
            if let Ok(harvest) = result {
                self.apply_harvest_reward(player_id, patch_id, harvest, current_time)
                    .await;
            }
            self.clear_auto_action(player_id, "target_depleted").await;
        }
    }

    pub async fn handle_apply_compost(&self, player_id: &str, patch_id: &str, item_id: &str) {
        if !self
            .can_interact_with_farming_patch(player_id, patch_id)
            .await
        {
            tracing::warn!(
                "Rejected remote farming action from {} for patch {}",
                player_id,
                patch_id
            );
            return;
        }

        if item_id != "compost" {
            self.send_to_player(
                player_id,
                ServerMessage::Error {
                    code: 400,
                    message: "That isn't compost.".to_string(),
                },
            )
            .await;
            return;
        }

        let has_compost = {
            let players = self.players.read().await;
            players
                .get(player_id)
                .is_some_and(|p| p.inventory.has_item("compost", 1))
        };
        if !has_compost {
            self.send_to_player(
                player_id,
                ServerMessage::Error {
                    code: 400,
                    message: "You don't have any compost.".to_string(),
                },
            )
            .await;
            return;
        }

        let result = {
            let mut farming = self.farming.write().await;
            farming.apply_compost(patch_id, player_id)
        };

        match result {
            Ok(()) => {
                let inv_msg = {
                    let mut players = self.players.write().await;
                    let Some(player) = players.get_mut(player_id) else {
                        return;
                    };
                    player.inventory.remove_item("compost", 1);
                    inventory_update_message(player_id, &player.inventory)
                };
                self.send_to_player(player_id, inv_msg).await;

                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                let update = {
                    let farming = self.farming.read().await;
                    farming.patch_update_for(patch_id, player_id, current_time)
                };
                // Only a planted patch has a row to update; an empty pre-composted
                // patch is folded into save_farming_patch when the seed is planted.
                if update.state != "empty"
                    && let Some(ref db) = self.db
                    && let Err(e) = db
                        .update_farming_patch_composted(patch_id, player_id, true)
                        .await
                {
                    tracing::warn!("Failed to persist compost {}: {}", patch_id, e);
                }
                self.send_to_player(player_id, patch_update_message(&update))
                    .await;
                self.send_system_message(player_id, "You treat the patch with compost.")
                    .await;
            }
            Err(message) => {
                self.send_to_player(player_id, ServerMessage::Error { code: 400, message })
                    .await;
            }
        }
    }

    pub async fn handle_cure_patch(&self, player_id: &str, patch_id: &str) {
        if !self
            .can_interact_with_farming_patch(player_id, patch_id)
            .await
        {
            tracing::warn!(
                "Rejected remote farming action from {} for patch {}",
                player_id,
                patch_id
            );
            return;
        }
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let has_cure = {
            let players = self.players.read().await;
            players
                .get(player_id)
                .is_some_and(|p| p.inventory.has_item("plant_cure_potion", 1))
        };
        if !has_cure {
            self.send_to_player(
                player_id,
                ServerMessage::Error {
                    code: 400,
                    message: "You need a Plant Cure Potion to cure this crop.".to_string(),
                },
            )
            .await;
            return;
        }

        let result = {
            let mut farming = self.farming.write().await;
            farming.cure_patch(patch_id, player_id, current_time)
        };

        match result {
            Ok(()) => {
                // Consume one cure potion now that the cure succeeded.
                let inv_msg = {
                    let mut players = self.players.write().await;
                    let Some(player) = players.get_mut(player_id) else {
                        return;
                    };
                    player.inventory.remove_item("plant_cure_potion", 1);
                    inventory_update_message(player_id, &player.inventory)
                };
                self.send_to_player(player_id, inv_msg).await;

                // Curing rebases planted_at, so re-save the full row.
                let (state, update) = {
                    let farming = self.farming.read().await;
                    (
                        farming.get_state(patch_id, player_id),
                        farming.patch_update_for(patch_id, player_id, current_time),
                    )
                };
                if let (Some(state), Some(db)) = (state, self.db.as_ref())
                    && let Err(e) = db
                        .save_farming_patch(
                            patch_id,
                            player_id,
                            &state.crop_id,
                            state.planted_at,
                            state.lives_remaining,
                            state.health.as_str(),
                            state.composted,
                            state.disease_cycle_marker,
                        )
                        .await
                {
                    tracing::warn!("Failed to persist cured patch {}: {}", patch_id, e);
                }
                self.send_to_player(player_id, patch_update_message(&update))
                    .await;
                self.send_system_message(player_id, "You nurse the crop back to health.")
                    .await;
            }
            Err(message) => {
                self.send_to_player(player_id, ServerMessage::Error { code: 400, message })
                    .await;
            }
        }
    }

    pub async fn handle_clear_patch(&self, player_id: &str, patch_id: &str) {
        if !self
            .can_interact_with_farming_patch(player_id, patch_id)
            .await
        {
            tracing::warn!(
                "Rejected remote farming action from {} for patch {}",
                player_id,
                patch_id
            );
            return;
        }

        let result = {
            let mut farming = self.farming.write().await;
            farming.clear_patch(patch_id, player_id)
        };

        match result {
            Ok(()) => {
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                let update = {
                    let farming = self.farming.read().await;
                    farming.patch_update_for(patch_id, player_id, current_time)
                };
                if let Some(ref db) = self.db
                    && let Err(e) = db.delete_farming_patch(patch_id, player_id).await
                {
                    tracing::warn!("Failed to delete farming patch {}: {}", patch_id, e);
                }
                self.send_to_player(player_id, patch_update_message(&update))
                    .await;
                self.send_system_message(player_id, "You clear the dead crop.")
                    .await;
            }
            Err(message) => {
                self.send_to_player(player_id, ServerMessage::Error { code: 400, message })
                    .await;
            }
        }
    }

    pub async fn get_farming_patches_message(&self, player_id: &str) -> ServerMessage {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let farming = self.farming.read().await;
        let updates = farming.get_player_patch_states(player_id, current_time);
        let patches: Vec<FarmingPatchData> = updates
            .into_iter()
            .map(|update| {
                let patch = farming.patches.get(&update.patch_id).unwrap();
                FarmingPatchData {
                    patch_id: update.patch_id,
                    x: patch.x,
                    y: patch.y,
                    state: update.state,
                    crop_id: update.crop_id,
                    growth_stage: update.growth_stage,
                    owner_id: update.owner_id,
                    health: update.health,
                    lives_remaining: update.lives_remaining,
                    composted: update.composted,
                    patch_type: update.patch_type,
                    width: patch.width,
                    height: patch.height,
                    capacity: patch.capacity,
                }
            })
            .collect();
        let unlocked_plots = farming.get_unlocked_plots(player_id);
        // Paint plot soil across every footprint tile of every patch.
        let tile_overrides: Vec<TileOverride> = farming
            .patches
            .values()
            .flat_map(|patch| {
                let tile_id = plot_tile_id(farming.is_plot_unlocked(player_id, patch.plot));
                patch
                    .occupied_tiles()
                    .map(move |(x, y)| TileOverride { x, y, tile_id })
            })
            .collect();

        ServerMessage::FarmingPatchStates {
            patches,
            unlocked_plots,
            tile_overrides,
        }
    }

    pub(in crate::game) async fn process_farming_growth_updates(
        &self,
        current_tick: u64,
        current_time: u64,
    ) -> u128 {
        if !farming_growth_is_due(current_tick) {
            return 0;
        }

        let farming_growth_start = std::time::Instant::now();
        let updates = {
            let mut farming = self.farming.write().await;
            farming.tick_growth(current_time)
        };
        for (target_player_id, update) in updates {
            // Persist disease/death transitions (growth itself is derived from planted_at).
            if (update.state == "diseased" || update.state == "dead")
                && let Some(ref db) = self.db
                && let Err(e) = db
                    .update_farming_patch_health(
                        &update.patch_id,
                        &target_player_id,
                        &update.health,
                        update.disease_cycle_marker,
                    )
                    .await
            {
                tracing::warn!("Failed to persist farming patch health {}: {}", update.patch_id, e);
            }
            self.send_to_player(&target_player_id, patch_update_message(&update))
                .await;
        }

        farming_growth_start.elapsed().as_millis()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plot_tile_id_matches_locked_and_unlocked_tiles() {
        assert_eq!(plot_tile_id(true), UNLOCKED_PLOT_TILE_ID);
        assert_eq!(plot_tile_id(false), LOCKED_PLOT_TILE_ID);
    }

    #[test]
    fn farming_growth_is_due_only_on_expected_ticks() {
        assert!(!farming_growth_is_due(49));
        assert!(farming_growth_is_due(50));
        assert!(!farming_growth_is_due(99));
        assert!(!farming_growth_is_due(100));
        assert!(farming_growth_is_due(150));
    }
}
