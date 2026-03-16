use super::GameRoom;
use crate::boss::BossEvent;
use crate::npc::Npc;
use crate::protocol::ServerMessage;

pub const BOSS_MAP_ID: &str = "desert_wurm_arena";

impl GameRoom {
    /// Process all active boss fight sessions each tick
    pub(in crate::game) async fn process_boss_tick(&self, current_time: u64) {
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
            if let Some(instance) = self.instance_manager.get_by_instance_id(instance_id) {
                let npcs = instance.npcs.read().await;
                if let Some(npc) = npcs.get(&boss.boss_npc_id) {
                    boss.boss_hp = npc.hp;
                    boss.boss_x = npc.x;
                    boss.boss_y = npc.y;

                    // Detect boss death from combat damage
                    if npc.hp <= 0 && boss.wurm_state != crate::boss::WurmState::Dead {
                        tracing::info!("Boss NPC killed via combat, triggering BossDied");
                        boss.wurm_state = crate::boss::WurmState::Dead;
                        all_events.push(BossEvent::BossDied {
                            instance_id: instance_id.clone(),
                            killer_id: npc.target_id.clone(),
                        });
                        continue;
                    }
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
    fn handle_boss_event<'a>(
        &'a self,
        event: BossEvent,
        current_time: u64,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
        match event {
            BossEvent::StateUpdate {
                instance_id,
                boss_hp,
                boss_max_hp,
                phase,
                wurm_state,
            } => {
                self.send_to_instance(
                    &instance_id,
                    ServerMessage::BossStateUpdate {
                        boss_id: String::new(), // filled per-instance
                        hp: boss_hp,
                        max_hp: boss_max_hp,
                        phase,
                        wurm_state,
                    },
                )
                .await;
            }
            BossEvent::SpawnMinion {
                instance_id,
                npc_id,
                x,
                y,
            } => {
                let prototype_id = "wurm_minion";
                if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                    if let Some(prototype) = self.entity_registry.get(prototype_id) {
                        let npc = Npc::from_prototype(
                            &npc_id,
                            prototype_id,
                            prototype,
                            x,
                            y,
                            1, // minion level
                            None,
                        );
                        let mut npcs = instance.npcs.write().await;
                        npcs.insert(npc_id, npc);
                    } else {
                        tracing::warn!("Boss: prototype '{}' not found", prototype_id);
                    }
                }
            }
            BossEvent::AoeWarning {
                instance_id,
                tiles,
                delay_ms,
                effect,
            } => {
                self.send_to_instance(
                    &instance_id,
                    ServerMessage::AoeWarning {
                        tiles,
                        delay_ms,
                        effect,
                    },
                )
                .await;
            }
            BossEvent::AoeDamage {
                instance_id,
                tiles,
                damage,
            } => {
                // Damage players standing on affected tiles
                let player_ids = self.get_instance_player_ids(&instance_id).await;
                let mut hit_players = Vec::new();

                {
                    let players = self.players.read().await;
                    for pid in &player_ids {
                        if let Some(player) = players.get(pid) {
                            let px = player.x;
                            let py = player.y;
                            if tiles.contains(&(px, py)) {
                                hit_players.push(pid.clone());
                            }
                        }
                    }
                }

                // Apply damage to hit players
                {
                    let mut players = self.players.write().await;
                    for pid in &hit_players {
                        if let Some(player) = players.get_mut(pid) {
                            player.hp = (player.hp - damage).max(0);
                            if player.hp <= 0 && !player.is_dead {
                                player.die(current_time);
                            }
                        }
                    }
                }

                self.send_to_instance(
                    &instance_id,
                    ServerMessage::AoeDamage { tiles, damage },
                )
                .await;
            }
            BossEvent::Explosion {
                instance_id,
                x,
                y,
                radius,
                damage,
            } => {
                // Calculate 3x3 blast zone tiles
                let mut blast_tiles = Vec::new();
                for dx in -radius..=radius {
                    for dy in -radius..=radius {
                        blast_tiles.push((x + dx, y + dy));
                    }
                }

                // Damage players in blast zone
                let player_ids = self.get_instance_player_ids(&instance_id).await;
                {
                    let mut players = self.players.write().await;
                    for pid in &player_ids {
                        if let Some(player) = players.get_mut(pid) {
                            let px = player.x;
                            let py = player.y;
                            if blast_tiles.contains(&(px, py)) {
                                player.hp = (player.hp - damage).max(0);
                                if player.hp <= 0 && !player.is_dead {
                                    player.die(current_time);
                                }
                            }
                        }
                    }
                }

                // Damage boss if in blast zone
                {
                    let boss_npc_id = {
                        let boss_states = self.boss_states.read().await;
                        boss_states.get(&instance_id).map(|b| (b.boss_npc_id.clone(), b.boss_x, b.boss_y))
                    };
                    if let Some((npc_id, bx, by)) = boss_npc_id {
                        if blast_tiles.contains(&(bx, by)) {
                            // Apply damage to the actual NPC
                            if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                                let mut npcs = instance.npcs.write().await;
                                if let Some(npc) = npcs.get_mut(&npc_id) {
                                    npc.hp = (npc.hp - damage).max(0);
                                }
                            }
                            // Update boss state machine
                            let mut boss_states = self.boss_states.write().await;
                            if let Some(boss) = boss_states.get_mut(&instance_id) {
                                let events = boss.on_boss_damaged(damage, None);
                                drop(boss_states);
                                for ev in events {
                                    self.handle_boss_event(ev, current_time).await;
                                }
                            }
                        }
                    }
                }

                // Kill chain-reaction minions in blast zone
                if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                    let mut chain_minions = Vec::new();
                    {
                        let npcs = instance.npcs.read().await;
                        for (npc_id, npc) in npcs.iter() {
                            if npc_id.starts_with("boss_minion_") {
                                let nx = npc.x;
                                let ny = npc.y;
                                if blast_tiles.contains(&(nx, ny)) {
                                    chain_minions.push((npc_id.clone(), nx, ny));
                                }
                            }
                        }
                    }

                    // Remove chain-killed minions and trigger their explosions
                    if !chain_minions.is_empty() {
                        let mut npcs = instance.npcs.write().await;
                        for (npc_id, mx, my) in &chain_minions {
                            npcs.remove(npc_id);
                            // Note: chain explosions handled via recursive event processing
                        }
                        drop(npcs);

                        for (_npc_id, mx, my) in chain_minions {
                            let mut boss_states = self.boss_states.write().await;
                            if let Some(boss) = boss_states.get_mut(&instance_id) {
                                let chain_events = boss.on_minion_exploded(mx, my);
                                drop(boss_states);
                                for ev in chain_events {
                                    self.handle_boss_event(ev, current_time).await;
                                }
                            }
                        }
                    }
                }

                self.send_to_instance(
                    &instance_id,
                    ServerMessage::Explosion {
                        x,
                        y,
                        radius,
                        damage,
                    },
                )
                .await;
            }
            BossEvent::MoveBoss {
                instance_id,
                npc_id,
                x,
                y,
            } => {
                // Update NPC position in instance
                if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                    let mut npcs = instance.npcs.write().await;
                    if let Some(npc) = npcs.get_mut(&npc_id) {
                        npc.x = x;
                        npc.y = y;
                        npc.spawn_x = x;
                        npc.spawn_y = y;
                    }
                }
            }
            BossEvent::SetBossInvulnerable {
                instance_id,
                npc_id,
                invulnerable,
            } => {
                if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                    let mut npcs = instance.npcs.write().await;
                    if let Some(npc) = npcs.get_mut(&npc_id) {
                        npc.invulnerable = invulnerable;
                        npc.hidden = invulnerable;
                    }
                }
            }
            BossEvent::BossDied {
                instance_id,
                killer_id,
            } => {
                tracing::info!(
                    "Boss died in instance {} (killer: {:?})",
                    instance_id,
                    killer_id
                );

                // Record death time for countdown
                {
                    let mut boss_states = self.boss_states.write().await;
                    if let Some(boss) = boss_states.get_mut(&instance_id) {
                        boss.death_time = current_time;
                    }
                }

                // Send final state update with 0 HP
                self.send_to_instance(
                    &instance_id,
                    ServerMessage::BossStateUpdate {
                        boss_id: String::new(),
                        hp: 0,
                        max_hp: 0,
                        phase: "dead".to_string(),
                        wurm_state: "dead".to_string(),
                    },
                )
                .await;

                // Clean up minions from instance
                if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                    let mut npcs = instance.npcs.write().await;
                    npcs.retain(|id, _| !id.starts_with("boss_minion_"));
                }

                // Send initial countdown
                self.send_to_instance(
                    &instance_id,
                    ServerMessage::Announcement {
                        text: "Returning to overworld in 3...".to_string(),
                    },
                )
                .await;
            }
            BossEvent::Announcement {
                instance_id,
                message,
            } => {
                self.send_to_instance(
                    &instance_id,
                    ServerMessage::Announcement {
                        text: message,
                    },
                )
                .await;
            }
            BossEvent::TeleportOut { instance_id } => {
                let player_ids = self.get_instance_player_ids(&instance_id).await;
                for pid in player_ids {
                    self.player_instances.write().await.remove(&pid);
                    self.reset_sync_state(&pid).await;

                    if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                        instance.remove_player(&pid).await;
                    }

                    self.send_to_player(
                        &pid,
                        ServerMessage::MapTransition {
                            map_type: "overworld".to_string(),
                            map_id: "world_0".to_string(),
                            spawn_x: crate::game::WORLD_SPAWN_X as f32,
                            spawn_y: crate::game::WORLD_SPAWN_Y as f32,
                            instance_id: String::new(),
                        },
                    )
                    .await;
                }
            }
        }
        })
    }

    /// Helper: send a message to all players in a specific instance
    async fn send_to_instance(&self, instance_id: &str, msg: ServerMessage) {
        let player_ids = self.get_instance_player_ids(instance_id).await;
        if player_ids.is_empty() {
            tracing::warn!("send_to_instance: no players found for instance '{}'", instance_id);
        }
        for pid in player_ids {
            self.send_to_player(&pid, msg.clone()).await;
        }
    }

    /// Helper: get all player IDs currently in a given instance
    async fn get_instance_player_ids(&self, instance_id: &str) -> Vec<String> {
        let instances = self.player_instances.read().await;
        instances
            .iter()
            .filter(|(_, iid)| iid.as_str() == instance_id)
            .map(|(pid, _)| pid.clone())
            .collect()
    }

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
}
