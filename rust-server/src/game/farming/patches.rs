use super::*;
use crate::protocol::{DialogueChoice, FarmingPatchData, TileOverride};

impl GameRoom {
    pub async fn handle_plant_seed(&self, player_id: &str, patch_id: &str, item_id: &str) {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let farming_level = {
            let players = self.players.read().await;
            let Some(player) = players.get(player_id) else {
                return;
            };
            if player.inventory.has_item(item_id, 1) {
                Some(player.skills.farming.level)
            } else {
                None
            }
        };

        let Some(farming_level) = farming_level else {
            self.send_to_player(
                player_id,
                ServerMessage::Error {
                    code: 400,
                    message: "You don't have that seed".to_string(),
                },
            )
            .await;
            return;
        };

        let result = {
            let mut farming = self.farming.write().await;
            farming.plant_seed(patch_id, item_id, player_id, farming_level, current_time)
        };

        match result {
            Ok((crop_id, xp)) => {
                let (inv_msg, total_xp, level, leveled_up) = {
                    let mut players = self.players.write().await;
                    let Some(player) = players.get_mut(player_id) else {
                        return;
                    };
                    player.inventory.remove_item(item_id, 1);
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
                    self.broadcast(ServerMessage::SkillLevelUp {
                        player_id: player_id.to_string(),
                        skill: "farming".to_string(),
                        new_level: level,
                    })
                    .await;
                    self.process_quest_progression_snapshot(player_id).await;
                }

                if let Some(ref db) = self.db {
                    if let Err(e) = db
                        .save_farming_patch(patch_id, player_id, &crop_id, current_time)
                        .await
                    {
                        tracing::warn!("Failed to save farming patch {}: {}", patch_id, e);
                    }
                }

                self.send_to_player(
                    player_id,
                    patch_state_update(patch_id, "growing", &crop_id, 0, player_id),
                )
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
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let farming_level = {
            let players = self.players.read().await;
            players
                .get(player_id)
                .map(|player| player.skills.farming.level)
                .unwrap_or(1)
        };

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
            farming.harvest_crop(patch_id, player_id, current_time, farming_level)
        };

        match result {
            Ok(harvest) => {
                let (inv_msg, total_xp, level, leveled_up) = {
                    let mut players = self.players.write().await;
                    let Some(player) = players.get_mut(player_id) else {
                        return;
                    };

                    player.inventory.add_item(
                        &harvest.produce_item,
                        harvest.amount,
                        &self.item_registry,
                    );
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
                    self.broadcast(ServerMessage::SkillLevelUp {
                        player_id: player_id.to_string(),
                        skill: "farming".to_string(),
                        new_level: level,
                    })
                    .await;
                    self.process_quest_progression_snapshot(player_id).await;
                }

                self.send_to_player(player_id, patch_state_update(patch_id, "empty", "", 0, ""))
                    .await;

                if let Some(ref db) = self.db {
                    if let Err(e) = db.delete_farming_patch(patch_id, player_id).await {
                        tracing::warn!("Failed to delete farming patch {}: {}", patch_id, e);
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

                let contract_progress = {
                    let mut farming = self.farming.write().await;
                    let crop_id = farming
                        .crops
                        .iter()
                        .find(|(_, crop)| crop.produce_item == harvest.produce_item)
                        .map(|(crop_id, _)| crop_id.clone());

                    crop_id.and_then(|crop_id| {
                        farming.record_contract_harvest(player_id, &crop_id, harvest.amount)
                    })
                };

                if let Some((harvested, required, complete)) = contract_progress {
                    if complete {
                        self.send_system_message(
                            player_id,
                            &format!(
                                "Contract complete! ({}/{}) Return to the Master Farmer to claim your rewards.",
                                harvested, required
                            ),
                        )
                        .await;
                    } else {
                        self.send_system_message(
                            player_id,
                            &format!("Contract progress: {}/{} harvested.", harvested, required),
                        )
                        .await;
                    }

                    if let Some(ref db) = self.db {
                        if let Err(e) = db
                            .update_farming_contract_progress(player_id, harvested)
                            .await
                        {
                            tracing::warn!("Failed to update contract progress: {}", e);
                        }
                    }

                    self.send_farming_contract_update(player_id).await;
                }
            }
            Err(message) => {
                self.send_to_player(player_id, ServerMessage::Error { code: 400, message })
                    .await;
            }
        }
    }

    pub(in crate::game) async fn show_plot_purchase_dialogue(&self, player_id: &str, npc_id: &str) {
        let npc_name = self.master_farmer_name(npc_id).await;
        let (farming_level, gold) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) => (player.skills.farming.level, player.inventory.gold),
                None => return,
            }
        };
        let unlocked = {
            let farming = self.farming.read().await;
            farming.get_unlocked_plots(player_id)
        };

        let mut choices: Vec<DialogueChoice> = crate::farming::PLOT_REQUIREMENTS
            .iter()
            .map(|req| {
                plot_purchase_choice(req, unlocked.contains(&req.plot_id), farming_level, gold)
            })
            .collect();
        choices.push(DialogueChoice {
            id: "nevermind".to_string(),
            text: "Go back".to_string(),
        });

        let text = format!(
            "Each allotment plot gives you 16 farming patches to grow crops.\n\nYour gold: {}gp | Farming level: {}",
            gold, farming_level
        );

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("plot_seller:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker: npc_name,
                text,
                choices,
            },
        )
        .await;
    }

    pub(in crate::game) async fn handle_plot_purchase(&self, player_id: &str, plot_id: u32) {
        let Some(req) = crate::farming::PLOT_REQUIREMENTS
            .iter()
            .find(|req| req.plot_id == plot_id)
        else {
            self.send_system_message(player_id, "Invalid plot.").await;
            return;
        };

        let already_unlocked = {
            let farming = self.farming.read().await;
            farming.is_plot_unlocked(player_id, plot_id)
        };
        if already_unlocked {
            self.send_system_message(player_id, "You already own this plot.")
                .await;
            return;
        }

        let (farming_level, gold) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) => (player.skills.farming.level, player.inventory.gold),
                None => return,
            }
        };

        if farming_level < req.farming_level {
            self.send_system_message(
                player_id,
                &format!(
                    "You need Farming level {} to unlock this plot.",
                    req.farming_level
                ),
            )
            .await;
            return;
        }

        if gold < req.gold_cost {
            self.send_system_message(
                player_id,
                &format!(
                    "You need {}gp to unlock this plot. You have {}gp.",
                    req.gold_cost, gold
                ),
            )
            .await;
            return;
        }

        let inv_msg = {
            let mut players = self.players.write().await;
            let Some(player) = players.get_mut(player_id) else {
                return;
            };
            player.inventory.gold -= req.gold_cost;
            inventory_update_message(player_id, &player.inventory)
        };

        self.send_to_player(player_id, inv_msg).await;

        {
            let mut farming = self.farming.write().await;
            farming.unlock_plot(player_id, plot_id);
        }

        if let Some(ref db) = self.db {
            if let Err(e) = db.save_plot_unlock(player_id, plot_id).await {
                tracing::error!("Failed to save plot unlock: {}", e);
            }
        }

        self.send_system_message(
            player_id,
            &format!(
                "You've unlocked Plot {}! 16 new allotment patches are now available.",
                plot_id
            ),
        )
        .await;

        let patches_msg = self.get_farming_patches_message(player_id).await;
        self.send_to_player(player_id, patches_msg).await;
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
                }
            })
            .collect();
        let unlocked_plots = farming.get_unlocked_plots(player_id);
        let tile_overrides: Vec<TileOverride> = farming
            .patches
            .values()
            .map(|patch| TileOverride {
                x: patch.x,
                y: patch.y,
                tile_id: plot_tile_id(farming.is_plot_unlocked(player_id, patch.plot)),
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
            self.send_to_player(
                &target_player_id,
                patch_state_update(
                    &update.patch_id,
                    &update.state,
                    &update.crop_id,
                    update.growth_stage,
                    &update.owner_id,
                ),
            )
            .await;
        }

        farming_growth_start.elapsed().as_millis()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plot_purchase_choice_reflects_owned_locked_and_affordable_states() {
        let req = crate::farming::PlotRequirement {
            plot_id: 2,
            farming_level: 15,
            gold_cost: 500,
        };

        let owned = plot_purchase_choice(&req, true, 99, 9999);
        assert_eq!(owned.id, "owned_2");
        assert_eq!(owned.text, "Plot 2 (Owned)");

        let level_locked = plot_purchase_choice(&req, false, 10, 9999);
        assert_eq!(level_locked.id, "locked_2");
        assert!(level_locked.text.contains("Requires Farming 15"));

        let gold_locked = plot_purchase_choice(&req, false, 20, 100);
        assert_eq!(gold_locked.id, "locked_2");
        assert!(gold_locked.text.contains("Not enough gold"));

        let affordable = plot_purchase_choice(&req, false, 20, 500);
        assert_eq!(affordable.id, "unlock_2");
        assert_eq!(affordable.text, "Plot 2 - 500gp");
    }

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
