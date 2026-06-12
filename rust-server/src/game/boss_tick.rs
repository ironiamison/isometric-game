use super::GameRoom;
use crate::boss::BossEvent;
use crate::item;
use crate::protocol::ServerMessage;

pub const BOSS_MAP_ID: &str = "desert_boss_cave";
pub const PHARAOH_BOSS_MAP_ID: &str = "pyramid_tomb";

impl GameRoom {
    /// Process all active boss fight sessions each tick
    pub(in crate::game) async fn process_boss_tick(&self, current_time: u64) {
        let boss_npc_ids: Vec<(String, String)> = {
            let states = self.boss_states.read().await;
            states
                .iter()
                .map(|(instance_id, boss)| (instance_id.clone(), boss.boss_npc_id.clone()))
                .collect()
        };
        let mut npc_snapshots = std::collections::HashMap::new();
        for (instance_id, npc_id) in boss_npc_ids {
            if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                let npcs = instance.npcs.read().await;
                if let Some(npc) = npcs.get(&npc_id) {
                    npc_snapshots
                        .insert(instance_id, (npc.hp, npc.x, npc.y, npc.target_id.clone()));
                }
            }
        }

        let mut boss_states = self.boss_states.write().await;
        let mut finished_instances: Vec<String> = Vec::new();

        let mut all_events: Vec<BossEvent> = Vec::new();

        for (instance_id, boss) in boss_states.iter_mut() {
            if boss.is_dead() {
                // Death countdown: 3 seconds before teleporting out
                if boss.death_time > 0 {
                    let elapsed = current_time.saturating_sub(boss.death_time);
                    let seconds_left = 3u64.saturating_sub(elapsed / 1000);

                    // Send countdown announcements
                    let announced = boss.countdown_sent;
                    if announced < 3 - seconds_left as u8 {
                        boss.countdown_sent = 3 - seconds_left as u8;
                        let msg = if seconds_left == 0 {
                            "Returning to overworld...".to_string()
                        } else {
                            format!("Returning to overworld in {}...", seconds_left)
                        };
                        all_events.push(BossEvent::Announcement {
                            instance_id: instance_id.clone(),
                            message: msg,
                        });
                    }

                    if elapsed >= 3500 {
                        // Time to teleport and clean up
                        all_events.push(BossEvent::TeleportOut {
                            instance_id: instance_id.clone(),
                        });
                        finished_instances.push(instance_id.clone());
                    }
                }
                continue;
            }

            // Sync boss HP from the actual NPC so combat damage is reflected
            if let Some((npc_hp, npc_x, npc_y, target_id)) = npc_snapshots.get(instance_id) {
                boss.boss_hp = *npc_hp;
                boss.boss_x = *npc_x;
                boss.boss_y = *npc_y;

                // Detect boss death from combat damage
                if *npc_hp <= 0 && boss.wurm_state != crate::boss::WurmState::Dead {
                    tracing::info!("Boss NPC killed via combat, triggering BossDied");
                    boss.wurm_state = crate::boss::WurmState::Dead;
                    all_events.push(BossEvent::BossDied {
                        instance_id: instance_id.clone(),
                        killer_id: target_id.clone(),
                    });
                    continue;
                }
            }

            let events = boss.tick(current_time);
            all_events.extend(events);
        }

        // Remove finished instances
        for id in &finished_instances {
            boss_states.remove(id);
        }

        drop(boss_states);

        // Process events
        for event in all_events {
            self.handle_boss_event(event, current_time).await;
        }
    }

    /// Handle a single boss event
    /// Helper: send a message to all players in a specific instance
    /// Helper: get all player IDs currently in a given instance
    /// Called when a minion NPC dies in an instance - triggers explosion
    pub(in crate::game) async fn check_boss_minion_death(
        &self,
        npc_id: &str,
        instance_id: &str,
        npc_x: i32,
        npc_y: i32,
        current_time: u64,
    ) {
        if !npc_id.starts_with("boss_minion_") {
            return;
        }

        let mut boss_states = self.boss_states.write().await;
        if let Some(boss) = boss_states.get_mut(instance_id) {
            let events = boss.on_minion_exploded(npc_x, npc_y);
            drop(boss_states);

            for event in events {
                self.handle_boss_event(event, current_time).await;
            }
        }
    }

    /// Called when a pharaoh minion NPC dies in an instance
    pub(in crate::game) async fn check_pharaoh_minion_death(
        &self,
        npc_id: &str,
        instance_id: &str,
        _current_time: u64,
    ) {
        if !npc_id.starts_with("pharaoh_minion_") {
            return;
        }
        let mut states = self.pharaoh_boss_states.write().await;
        if let Some(boss) = states.get_mut(instance_id) {
            boss.on_minion_died();
        }
    }

    /// Called when the boss NPC itself is killed
    pub(in crate::game) async fn check_boss_npc_death(
        &self,
        npc_id: &str,
        instance_id: &str,
        killer_id: Option<&str>,
        current_time: u64,
    ) {
        let mut boss_states = self.boss_states.write().await;
        if let Some(boss) = boss_states.get_mut(instance_id) {
            if boss.boss_npc_id != npc_id {
                return;
            }
            let events = boss.on_boss_damaged(boss.boss_hp, killer_id.map(|s| s.to_string()));
            drop(boss_states);

            for event in events {
                self.handle_boss_event(event, current_time).await;
            }
        }
    }

    /// Check if a boss session already exists for an instance
    pub async fn has_boss_session(&self, instance_id: &str) -> bool {
        let states = self.boss_states.read().await;
        states.contains_key(instance_id)
    }

    /// Add a player to an existing boss fight session
    pub async fn add_boss_player(&self, instance_id: &str, player_id: &str) {
        let mut states = self.boss_states.write().await;
        if let Some(boss) = states.get_mut(instance_id) {
            boss.add_player(player_id.to_string());
            tracing::info!(
                "Player {} joined boss fight in instance {}",
                player_id,
                instance_id
            );
        }
    }

    /// Start a boss fight session for an instance
    pub async fn start_boss_session(
        &self,
        instance_id: &str,
        boss_npc_id: &str,
        boss_hp: i32,
        boss_max_hp: i32,
        boss_x: i32,
        boss_y: i32,
        map_width: i32,
        map_height: i32,
        current_time: u64,
    ) {
        let boss = crate::boss::BossState::new(
            instance_id.to_string(),
            boss_npc_id.to_string(),
            boss_hp,
            boss_max_hp,
            boss_x,
            boss_y,
            map_width,
            map_height,
            current_time,
        );
        let mut states = self.boss_states.write().await;
        states.insert(instance_id.to_string(), boss);
        tracing::info!(
            "Boss session started in instance {} (npc: {})",
            instance_id,
            boss_npc_id
        );
    }

    /// Show pending boss rewards dialogue to a player
    pub async fn show_boss_rewards_dialogue(&self, player_id: &str, npc_id: &str) {
        let pending = if let Some(ref db) = self.db {
            match db.get_boss_pending_rewards(player_id).await {
                Ok(rewards) => rewards,
                Err(e) => {
                    tracing::error!(
                        "Failed to get boss pending rewards for {}: {}",
                        player_id,
                        e
                    );
                    return;
                }
            }
        } else {
            return;
        };

        if pending.is_empty() {
            self.send_to_player(
                player_id,
                ServerMessage::ShowDialogue {
                    quest_id: String::new(),
                    npc_id: npc_id.to_string(),
                    speaker: "Battle Master".to_string(),
                    text: "Hail, hunter! Defeat the Desert Wurm and I'll distribute the spoils."
                        .to_string(),
                    choices: vec![crate::protocol::DialogueChoice {
                        id: "close".to_string(),
                        text: "Farewell".to_string(),
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

        let mut text = String::from("Your unclaimed boss rewards:\n\n");
        for (item_id, quantity) in &aggregated {
            if item_id == "gold" {
                text.push_str(&format!("  {} gold\n", quantity));
            } else {
                let display_name = self
                    .item_registry
                    .get(item_id)
                    .map(|def| def.display_name.clone())
                    .unwrap_or_else(|| item_id.clone());
                text.push_str(&format!("  {} x{}\n", display_name, quantity));
            }
        }
        text.push_str("\nWhere would you like your rewards?");

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("boss_rewards:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker: "Battle Master".to_string(),
                text,
                choices: vec![
                    crate::protocol::DialogueChoice {
                        id: "claim".to_string(),
                        text: "Send to Inventory".to_string(),
                    },
                    crate::protocol::DialogueChoice {
                        id: "bank".to_string(),
                        text: "Send to Bank".to_string(),
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

    /// Claim all pending boss rewards and add to inventory
    pub async fn claim_boss_rewards(&self, player_id: &str) {
        tracing::info!("claim_boss_rewards called for player_id='{}'", player_id);
        let rewards = if let Some(ref db) = self.db {
            match db.claim_boss_pending_rewards(player_id).await {
                Ok(rewards) => {
                    tracing::info!(
                        "claim_boss_rewards: got {} reward entries for '{}'",
                        rewards.len(),
                        player_id
                    );
                    rewards
                }
                Err(e) => {
                    tracing::error!("Failed to claim boss rewards for {}: {}", player_id, e);
                    return;
                }
            }
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
        let mut kept_pending = 0u32;

        for (item_id, quantity) in &rewards {
            tracing::info!(
                "claim_boss_rewards: processing item='{}' qty={}",
                item_id,
                quantity
            );
            if item_id == "gold" {
                let Ok(quantity) = i32::try_from(*quantity) else {
                    tracing::error!("Discarding oversized boss gold reward: {}", quantity);
                    continue;
                };
                let Some(new_total) = item::checked_gold_credit(total_gold, quantity) else {
                    tracing::error!("Discarding boss gold rewards above the currency cap");
                    continue;
                };
                total_gold = new_total;
            } else {
                tracing::info!(
                    "claim_boss_rewards: granting {} x{} to '{}'",
                    item_id,
                    quantity,
                    player_id
                );
                if self.grant_item_to_player(player_id, item_id, *quantity).await {
                    item_count += quantity;
                } else {
                    // Inventory full — re-queue so the reward stays claimable
                    // instead of vanishing.
                    if let Some(ref db) = self.db
                        && let Err(e) =
                            db.add_boss_pending_reward(player_id, item_id, *quantity).await
                    {
                        tracing::error!(
                            "Failed to re-queue boss reward {} x{} for {}: {}",
                            item_id,
                            quantity,
                            player_id,
                            e
                        );
                    }
                    kept_pending += quantity;
                }
            }
        }

        if total_gold > 0 {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                if let Some(new_gold) = item::checked_gold_credit(player.inventory.gold, total_gold)
                {
                    player.inventory.gold = new_gold;
                } else {
                    tracing::error!("Player {} cannot receive boss gold at cap", player_id);
                }
            }
        }

        // Send inventory update to client so items appear immediately
        {
            let players = self.players.read().await;
            if let Some(player) = players.get(player_id) {
                let inventory_update = player.inventory.to_update();
                let gold = player.inventory.gold;
                drop(players);
                self.send_to_player(
                    player_id,
                    ServerMessage::InventoryUpdate {
                        player_id: player_id.to_string(),
                        slots: inventory_update,
                        gold,
                    },
                )
                .await;
            }
        }

        let msg = if total_gold > 0 && item_count > 0 {
            format!(
                "Claimed {} gold and {} items from boss rewards!",
                total_gold, item_count
            )
        } else if total_gold > 0 {
            format!("Claimed {} gold from boss rewards!", total_gold)
        } else {
            format!("Claimed {} items from boss rewards!", item_count)
        };
        self.send_system_message(player_id, &msg).await;
        if kept_pending > 0 {
            self.send_system_message(
                player_id,
                "Your inventory was full — the remaining rewards are still waiting to be claimed. Make some space and claim again.",
            )
            .await;
        }
    }

    /// Claim all pending boss rewards and send directly to bank
    pub async fn claim_boss_rewards_to_bank(&self, player_id: &str) {
        let rewards = if let Some(ref db) = self.db {
            match db.claim_boss_pending_rewards(player_id).await {
                Ok(rewards) => rewards,
                Err(e) => {
                    tracing::error!("Failed to claim boss rewards for {}: {}", player_id, e);
                    return;
                }
            }
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
        let mut overflow_items: Vec<(String, u32)> = Vec::new();

        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                for (item_id, quantity) in &rewards {
                    if item_id == "gold" {
                        let Ok(quantity) = i32::try_from(*quantity) else {
                            tracing::error!("Discarding oversized boss gold reward: {}", quantity);
                            continue;
                        };
                        let Some(new_bank_gold) =
                            item::checked_gold_credit(player.bank.gold, quantity)
                        else {
                            tracing::error!(
                                "Player {} cannot receive boss bank gold at cap",
                                player_id
                            );
                            continue;
                        };
                        player.bank.gold = new_bank_gold;
                        total_gold = item::checked_gold_credit(total_gold, quantity)
                            .unwrap_or(item::MAX_GOLD);
                    } else if player.bank.has_space_for(
                        item_id,
                        *quantity as i32,
                        &self.item_registry,
                    ) {
                        player
                            .bank
                            .add_item(item_id, *quantity as i32, &self.item_registry);
                        item_count += quantity;
                    } else {
                        overflow_items.push((item_id.clone(), *quantity));
                    }
                }
            }
        }

        // Bank was full for these — try the inventory, and if that's full too
        // re-queue them as pending so nothing is lost.
        for (item_id, quantity) in &overflow_items {
            if self.grant_item_to_player(player_id, item_id, *quantity).await {
                item_count += quantity;
            } else if let Some(ref db) = self.db
                && let Err(e) = db.add_boss_pending_reward(player_id, item_id, *quantity).await
            {
                tracing::error!(
                    "Failed to re-queue boss reward {} x{} for {}: {}",
                    item_id,
                    quantity,
                    player_id,
                    e
                );
            }
        }

        // Send bank update
        {
            let players = self.players.read().await;
            if let Some(player) = players.get(player_id) {
                let bank_msg = ServerMessage::BankUpdate {
                    slots: player.bank.to_update(),
                    gold: player.bank.gold,
                };
                let inv_msg = ServerMessage::InventoryUpdate {
                    player_id: player_id.to_string(),
                    slots: player.inventory.to_update(),
                    gold: player.inventory.gold,
                };
                drop(players);
                self.send_to_player(player_id, bank_msg).await;
                self.send_to_player(player_id, inv_msg).await;
            }
        }

        let mut msg = if total_gold > 0 && item_count > 0 {
            format!(
                "Sent {} gold and {} items to your bank!",
                total_gold, item_count
            )
        } else if total_gold > 0 {
            format!("Sent {} gold to your bank!", total_gold)
        } else {
            format!("Sent {} items to your bank!", item_count)
        };

        if !overflow_items.is_empty() {
            msg.push_str(" Some items were sent to inventory (bank full).");
        }

        self.send_system_message(player_id, &msg).await;
    }

    // -----------------------------------------------------------------------
    // Pharaoh boss tick pipeline
    // -----------------------------------------------------------------------

    /// Process all active pharaoh boss fight sessions each tick
    pub(in crate::game) async fn process_pharaoh_boss_tick(&self, current_time: u64) {
        let boss_npc_ids: Vec<(String, String)> = {
            let states = self.pharaoh_boss_states.read().await;
            states
                .iter()
                .map(|(instance_id, boss)| (instance_id.clone(), boss.boss_npc_id.clone()))
                .collect()
        };
        let mut npc_snapshots = std::collections::HashMap::new();
        for (instance_id, npc_id) in boss_npc_ids {
            if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                let npcs = instance.npcs.read().await;
                if let Some(npc) = npcs.get(&npc_id) {
                    npc_snapshots
                        .insert(instance_id, (npc.hp, npc.x, npc.y, npc.target_id.clone()));
                }
            }
        }

        let mut pharaoh_states = self.pharaoh_boss_states.write().await;
        let mut finished_instances: Vec<String> = Vec::new();
        let mut all_events: Vec<BossEvent> = Vec::new();

        for (instance_id, boss) in pharaoh_states.iter_mut() {
            if boss.is_dead() {
                // Death countdown: 3 seconds before teleporting out
                if boss.death_time > 0 {
                    let elapsed = current_time.saturating_sub(boss.death_time);
                    let seconds_left = 3u64.saturating_sub(elapsed / 1000);

                    let announced = boss.countdown_sent;
                    if announced < 3 - seconds_left as u8 {
                        boss.countdown_sent = 3 - seconds_left as u8;
                        let msg = if seconds_left == 0 {
                            "Returning to overworld...".to_string()
                        } else {
                            format!("Returning to overworld in {}...", seconds_left)
                        };
                        all_events.push(BossEvent::Announcement {
                            instance_id: instance_id.clone(),
                            message: msg,
                        });
                    }

                    if elapsed >= 3500 {
                        all_events.push(BossEvent::TeleportOut {
                            instance_id: instance_id.clone(),
                        });
                        finished_instances.push(instance_id.clone());
                    }
                }
                continue;
            }

            // Sync boss HP from the actual NPC so combat damage is reflected
            if let Some((npc_hp, npc_x, npc_y, target_id)) = npc_snapshots.get(instance_id) {
                boss.boss_hp = *npc_hp;
                boss.boss_x = *npc_x;
                boss.boss_y = *npc_y;

                // Detect boss death from combat damage
                if *npc_hp <= 0 && !boss.is_dead() {
                    tracing::info!("Pharaoh boss NPC killed via combat, triggering BossDied");
                    boss.state = crate::pharaoh_boss::PharaohState::Dead;
                    all_events.push(BossEvent::BossDied {
                        instance_id: instance_id.clone(),
                        killer_id: target_id.clone(),
                    });
                    continue;
                }
            }

            let events = boss.tick(current_time);
            all_events.extend(events);
        }

        // Remove finished instances
        for id in &finished_instances {
            pharaoh_states.remove(id);
        }

        drop(pharaoh_states);

        // Process events
        for event in all_events {
            self.handle_boss_event(event, current_time).await;
        }
    }

    /// Start a pharaoh boss fight session for an instance
    pub async fn start_pharaoh_boss_session(
        &self,
        instance_id: &str,
        boss_npc_id: &str,
        boss_hp: i32,
        boss_max_hp: i32,
        boss_x: i32,
        boss_y: i32,
        map_width: i32,
        map_height: i32,
        current_time: u64,
    ) {
        let boss = crate::pharaoh_boss::PharaohBossState::new(
            instance_id.to_string(),
            boss_npc_id.to_string(),
            boss_hp,
            boss_max_hp,
            boss_x,
            boss_y,
            map_width,
            map_height,
            current_time,
        );
        let mut states = self.pharaoh_boss_states.write().await;
        states.insert(instance_id.to_string(), boss);
        tracing::info!(
            "Pharaoh boss session started in instance {} (npc: {})",
            instance_id,
            boss_npc_id
        );
    }

    /// Check if a pharaoh boss session already exists for an instance
    pub async fn has_pharaoh_boss_session(&self, instance_id: &str) -> bool {
        let states = self.pharaoh_boss_states.read().await;
        states.contains_key(instance_id)
    }

    /// Add a player to an existing pharaoh boss fight session
    pub async fn add_pharaoh_boss_player(&self, instance_id: &str, player_id: &str) {
        let mut states = self.pharaoh_boss_states.write().await;
        if let Some(boss) = states.get_mut(instance_id) {
            boss.add_player(player_id.to_string());
            tracing::info!(
                "Player {} joined pharaoh boss fight in instance {}",
                player_id,
                instance_id
            );
        }
    }
}
