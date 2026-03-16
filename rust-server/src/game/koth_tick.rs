use super::GameRoom;
use crate::koth::{KothEvent, KothState};
use crate::npc::Npc;
use crate::protocol::{KothRewardData, ServerMessage};

/// The KOTH arena interior map ID
pub const KOTH_MAP_ID: &str = "koth_arena";

impl GameRoom {
    /// Process all active KOTH sessions each tick
    pub(in crate::game) async fn process_koth_tick(&self, current_time: u64) {
        let mut koth_states = self.koth_states.write().await;
        let mut finished_instances: Vec<String> = Vec::new();

        // Collect player levels for wave scaling
        let player_levels: std::collections::HashMap<String, i32> = {
            let players = self.players.read().await;
            koth_states
                .values()
                .filter_map(|ks| {
                    players.get(&ks.player_id).map(|p| {
                        let combat_level = p.combat_level();
                        (ks.player_id.clone(), combat_level)
                    })
                })
                .collect()
        };

        let mut all_events: Vec<KothEvent> = Vec::new();

        for (instance_id, koth) in koth_states.iter_mut() {
            if koth.is_game_over() {
                finished_instances.push(instance_id.clone());
                continue;
            }

            let player_level = player_levels
                .get(&koth.player_id)
                .copied()
                .unwrap_or(10);

            let events = koth.tick(current_time, player_level);
            all_events.extend(events);
        }

        // Remove finished instances
        for id in &finished_instances {
            koth_states.remove(id);
        }

        drop(koth_states);

        // Process events
        for event in all_events {
            self.handle_koth_event(event, current_time).await;
        }
    }

    /// Handle a single KOTH event
    async fn handle_koth_event(&self, event: KothEvent, _current_time: u64) {
        match event {
            KothEvent::SpawnNpc {
                instance_id,
                npc_id,
                prototype_id,
                level,
                x,
                y,
            } => {
                // Spawn NPC in the instance
                if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                    if let Some(prototype) = self.entity_registry.get(&prototype_id) {
                        let npc = Npc::from_prototype(
                            &npc_id,
                            &prototype_id,
                            prototype,
                            x,
                            y,
                            level,
                            None,
                        );
                        let mut npcs = instance.npcs.write().await;
                        npcs.insert(npc_id, npc);
                    } else {
                        tracing::warn!("KOTH: prototype '{}' not found", prototype_id);
                    }
                }
            }
            KothEvent::StateUpdate {
                player_id,
                phase,
                wave,
                points,
                enemies_alive,
                enemies_total,
                countdown_ms,
            } => {
                self.send_to_player(
                    &player_id,
                    ServerMessage::KothStateUpdate {
                        phase,
                        wave,
                        points,
                        enemies_alive,
                        enemies_total,
                        countdown_ms,
                    },
                )
                .await;
            }
            KothEvent::CheckpointReached {
                player_id,
                wave,
                points,
                rewards,
                next_wave_enemy_count,
            } => {
                let reward_data: Vec<KothRewardData> = rewards
                    .iter()
                    .map(|r| KothRewardData {
                        item_id: r.item_id.clone(),
                        quantity: r.quantity,
                    })
                    .collect();
                self.send_to_player(
                    &player_id,
                    ServerMessage::KothCheckpoint {
                        wave,
                        points,
                        rewards: reward_data,
                        next_wave_enemy_count,
                    },
                )
                .await;
            }
            KothEvent::GameOver {
                player_id,
                instance_id,
                waves_completed,
                total_points,
                rewards,
                victory,
            } => {
                // Store rewards as pending (claimable from NPC)
                if let Some(ref db) = self.db {
                    for reward in &rewards {
                        if let Err(e) = db
                            .add_koth_pending_reward(&player_id, &reward.item_id, reward.quantity)
                            .await
                        {
                            tracing::error!(
                                "Failed to store KOTH reward for {}: {}",
                                player_id,
                                e
                            );
                        }
                    }
                }

                let reward_data: Vec<KothRewardData> = rewards
                    .iter()
                    .map(|r| KothRewardData {
                        item_id: r.item_id.clone(),
                        quantity: r.quantity,
                    })
                    .collect();

                self.send_to_player(
                    &player_id,
                    ServerMessage::KothGameOver {
                        waves_completed,
                        total_points,
                        rewards: reward_data,
                        victory,
                    },
                )
                .await;

                // Clean up instance NPCs
                if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                    let mut npcs = instance.npcs.write().await;
                    // Remove all KOTH-spawned NPCs
                    npcs.retain(|id, _| !id.starts_with("koth_"));
                }

                // Teleport player back to overworld after a short delay
                // (client shows game over screen, then we move them out)
                self.koth_exit_player(&player_id, &instance_id).await;
            }
        }
    }

    /// Called when an NPC dies in an instance - checks if it's a KOTH NPC
    pub(in crate::game) async fn check_koth_npc_death(
        &self,
        npc_id: &str,
        instance_id: &str,
        current_time: u64,
    ) {
        if !npc_id.starts_with("koth_") {
            return;
        }

        let mut koth_states = self.koth_states.write().await;
        if let Some(koth) = koth_states.get_mut(instance_id) {
            let events = koth.on_npc_killed(current_time);
            drop(koth_states);

            for event in events {
                self.handle_koth_event(event, current_time).await;
            }
        }
    }

    /// Called when a player dies in a KOTH instance
    pub(in crate::game) async fn check_koth_player_death(
        &self,
        player_id: &str,
        current_time: u64,
    ) {
        // Find instance
        let instance_id = {
            let instances = self.player_instances.read().await;
            instances.get(player_id).cloned()
        };

        if let Some(instance_id) = instance_id {
            let mut koth_states = self.koth_states.write().await;
            if let Some(koth) = koth_states.get_mut(&instance_id) {
                let events = koth.on_player_died();
                drop(koth_states);

                for event in events {
                    self.handle_koth_event(event, current_time).await;
                }
            }
        }
    }

    /// Start a KOTH session for a player entering the arena
    pub async fn start_koth_session(
        &self,
        instance_id: &str,
        player_id: &str,
        map_width: u32,
        map_height: u32,
        current_time: u64,
        entrance_x: i32,
        entrance_y: i32,
    ) {
        let koth = KothState::new(
            instance_id.to_string(),
            player_id.to_string(),
            map_width,
            map_height,
            current_time,
            entrance_x,
            entrance_y,
        );
        let mut states = self.koth_states.write().await;
        states.insert(instance_id.to_string(), koth);
        tracing::info!(
            "KOTH session started for player {} in instance {}",
            player_id,
            instance_id
        );
    }

    /// Handle KOTH continue/leave messages from player
    pub async fn handle_koth_continue(&self, player_id: &str, current_time: u64) {
        let instance_id = {
            let instances = self.player_instances.read().await;
            instances.get(player_id).cloned()
        };
        if let Some(instance_id) = instance_id {
            let mut koth_states = self.koth_states.write().await;
            if let Some(koth) = koth_states.get_mut(&instance_id) {
                let events = koth.on_continue(current_time);
                drop(koth_states);
                for event in events {
                    self.handle_koth_event(event, current_time).await;
                }
            }
        }
    }

    pub async fn handle_koth_leave(&self, player_id: &str, current_time: u64) {
        let instance_id = {
            let instances = self.player_instances.read().await;
            instances.get(player_id).cloned()
        };
        if let Some(instance_id) = instance_id {
            let mut koth_states = self.koth_states.write().await;
            if let Some(koth) = koth_states.get_mut(&instance_id) {
                let events = koth.on_leave();
                drop(koth_states);
                for event in events {
                    self.handle_koth_event(event, current_time).await;
                }
            }
        }
    }

    /// Teleport player out of KOTH instance back to overworld
    async fn koth_exit_player(&self, player_id: &str, instance_id: &str) {
        // Get entrance position from KOTH state
        let entrance_pos = {
            let states = self.koth_states.read().await;
            states
                .get(instance_id)
                .map(|k| (k.entrance_x, k.entrance_y))
                .unwrap_or((0, 0))
        };

        // Remove from player_instances
        {
            let mut instances = self.player_instances.write().await;
            instances.remove(player_id);
        }

        // Clean up KOTH state
        self.cleanup_koth_session(instance_id).await;
        self.reset_sync_state(player_id).await;

        // Remove player from instance and clean up private instance
        if let Some(instance) = self.instance_manager.get_by_instance_id(instance_id) {
            let remaining = instance.remove_player(player_id).await;
            if remaining == 0
                && instance.instance_type == crate::interior::InstanceType::Private
            {
                if let Some(owner_id) = &instance.owner_id {
                    self.instance_manager
                        .remove_private(owner_id, &instance.map_id);
                }
            }
        }

        // Update player position to entrance
        let (ex, ey) = entrance_pos;
        self.set_player_position(player_id, ex, ey).await;

        // Send map transition back to overworld
        self.send_to_player(
            player_id,
            ServerMessage::MapTransition {
                map_type: "overworld".to_string(),
                map_id: "world_0".to_string(),
                spawn_x: ex as f32,
                spawn_y: ey as f32,
                instance_id: String::new(),
            },
        )
        .await;

        // Notify overworld players that this player returned
        let player_name = self.get_player_name(player_id).await.unwrap_or_default();
        let (gender, skin) = self
            .get_player_appearance(player_id)
            .await
            .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
        let (hair_style, hair_color) = self
            .get_player_hair(player_id)
            .await
            .unwrap_or((None, None));
        self.send_to_overworld_players(
            ServerMessage::PlayerJoined {
                id: player_id.to_string(),
                name: player_name,
                x: ex,
                y: ey,
                gender,
                skin,
                hair_style,
                hair_color,
            },
            Some(player_id),
        )
        .await;
    }

    /// Get the entrance position for a KOTH session (where the player should return to)
    pub async fn get_koth_entrance(&self, instance_id: &str) -> Option<(i32, i32)> {
        let states = self.koth_states.read().await;
        states.get(instance_id).map(|k| (k.entrance_x, k.entrance_y))
    }

    /// Clean up KOTH state when a player exits the instance (via portal or disconnect)
    pub async fn cleanup_koth_session(&self, instance_id: &str) {
        let mut states = self.koth_states.write().await;
        if states.remove(instance_id).is_some() {
            tracing::info!("Cleaned up KOTH session for instance {}", instance_id);
        }
    }

    /// Helper to grant a non-gold item to a player
    async fn grant_item_to_player(&self, player_id: &str, item_id: &str, quantity: u32) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player
                .inventory
                .add_item(item_id, quantity as i32, &self.item_registry);
        }
    }

    /// Show the KOTH rewards NPC dialogue with pending rewards summary
    pub async fn show_koth_rewards_dialogue(&self, player_id: &str, npc_id: &str) {
        let pending = if let Some(ref db) = self.db {
            db.get_koth_pending_rewards(player_id).await.unwrap_or_default()
        } else {
            vec![]
        };

        if pending.is_empty() {
            self.send_to_player(
                player_id,
                ServerMessage::ShowDialogue {
                    quest_id: String::new(),
                    npc_id: npc_id.to_string(),
                    speaker: "Battle Master".to_string(),
                    text: "You have no unclaimed rewards.\n\nFight in the arena to earn loot!"
                        .to_string(),
                    choices: vec![crate::protocol::DialogueChoice {
                        id: "close".to_string(),
                        text: "Close".to_string(),
                    }],
                },
            )
            .await;
            return;
        }

        // Aggregate rewards by item_id
        let mut aggregated: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        for (_id, item_id, quantity) in &pending {
            *aggregated.entry(item_id.clone()).or_insert(0) += quantity;
        }

        let mut text = String::from("Your unclaimed KOTH rewards:\n\n");
        for (item_id, quantity) in &aggregated {
            let display_name = self
                .item_registry
                .get(item_id)
                .map(|def| def.display_name.clone())
                .unwrap_or_else(|| item_id.clone());
            text.push_str(&format!("  {} x{}\n", display_name, quantity));
        }
        text.push_str("\nClaim all rewards to your inventory?");

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("koth_rewards:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker: "Battle Master".to_string(),
                text,
                choices: vec![
                    crate::protocol::DialogueChoice {
                        id: "claim".to_string(),
                        text: "Claim All".to_string(),
                    },
                    crate::protocol::DialogueChoice {
                        id: "close".to_string(),
                        text: "Not Yet".to_string(),
                    },
                ],
            },
        )
        .await;
    }

    /// Claim all pending KOTH rewards and add to inventory
    pub async fn claim_koth_rewards(&self, player_id: &str) {
        let rewards = if let Some(ref db) = self.db {
            db.claim_koth_pending_rewards(player_id)
                .await
                .unwrap_or_default()
        } else {
            return;
        };

        if rewards.is_empty() {
            self.send_system_message(player_id, "No rewards to claim.")
                .await;
            return;
        }

        let mut total_gold = 0i32;
        let mut item_count = 0u32;

        for (item_id, quantity) in &rewards {
            if item_id == "gold_coins" {
                total_gold += *quantity as i32;
            } else {
                self.grant_item_to_player(player_id, item_id, *quantity)
                    .await;
                item_count += quantity;
            }
        }

        if total_gold > 0 {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.inventory.gold += total_gold;
            }
        }

        let msg = if total_gold > 0 && item_count > 0 {
            format!(
                "Claimed {} gold and {} items from KOTH rewards!",
                total_gold, item_count
            )
        } else if total_gold > 0 {
            format!("Claimed {} gold from KOTH rewards!", total_gold)
        } else {
            format!("Claimed {} items from KOTH rewards!", item_count)
        };
        self.send_system_message(player_id, &msg).await;
    }
}
