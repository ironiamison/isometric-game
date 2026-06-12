use super::*;
use crate::boss::BossEvent;
use crate::chunk::ChunkCoord;
use crate::npc::{Npc, NpcState};
use crate::protocol::ServerMessage;
use rand::Rng;

type RolledLootItem = (String, i32, String);
type RolledLoot = (String, String, i32, Vec<RolledLootItem>);

impl GameRoom {
    pub(super) fn handle_boss_event<'a>(
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
                    let prototype_id = if npc_id.starts_with("pharaoh_minion_") {
                        // Look up the pharaoh boss state to determine phase-appropriate prototype
                        let pharaoh_states = self.pharaoh_boss_states.read().await;
                        if let Some(boss) = pharaoh_states.get(&instance_id) {
                            // Extract minion counter from npc_id for Frenzy alternation
                            let index = npc_id
                                .rsplit('_')
                                .next()
                                .and_then(|s| s.parse::<u32>().ok())
                                .unwrap_or(0);
                            match &boss.phase {
                                crate::boss::BossPhase::Hunt => "pharaoh_mummy",
                                crate::boss::BossPhase::Storm => "pharaoh_skeleton",
                                crate::boss::BossPhase::Frenzy => {
                                    if index % 2 == 0 {
                                        "pharaoh_mummy"
                                    } else {
                                        "pharaoh_skeleton"
                                    }
                                }
                            }
                        } else {
                            "pharaoh_mummy" // fallback
                        }
                    } else {
                        "wurm_minion"
                    };
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
                    // Pharaoh projectile: find closest player and deal direct damage
                    if let Some(damage_str) = effect.strip_prefix("pharaoh_projectile:") {
                        let damage: i32 = damage_str.parse().unwrap_or(5);
                        // Boss position is encoded in tiles[0]
                        let (boss_x, boss_y) = tiles.first().copied().unwrap_or((0, 0));

                        // Find closest player in the instance
                        let player_ids = self.get_instance_player_ids(&instance_id).await;
                        let mut closest: Option<(String, i32, i32, i64)> = None; // (id, x, y, dist_sq)

                        {
                            let players = self.players.read().await;
                            for pid in &player_ids {
                                if let Some(player) = players.get(pid) {
                                    if player.is_dead {
                                        continue;
                                    }
                                    let dx = (player.x - boss_x) as i64;
                                    let dy = (player.y - boss_y) as i64;
                                    let dist_sq = dx * dx + dy * dy;
                                    if closest.as_ref().is_none_or(|(_, _, _, d)| dist_sq < *d) {
                                        closest = Some((pid.clone(), player.x, player.y, dist_sq));
                                    }
                                }
                            }
                        }

                        if let Some((target_id, target_x, target_y, _)) = closest {
                            // Apply damage
                            let result = {
                                let mut players = self.players.write().await;
                                if let Some(player) = players.get_mut(&target_id) {
                                    player.hp = (player.hp - damage).max(0);
                                    let died = player.hp <= 0 && !player.is_dead;
                                    if died {
                                        player.die(current_time);
                                    }
                                    Some((player.hp, died))
                                } else {
                                    None
                                }
                            };

                            if let Some((target_hp, died)) = result {
                                if died {
                                    self.broadcast_to_zone(
                                        &target_id,
                                        ServerMessage::PlayerDied {
                                            id: target_id.clone(),
                                            killer_id: "pharaoh_boss".to_string(),
                                        },
                                    )
                                    .await;
                                }

                                // Send DamageEvent with projectile visual
                                self.broadcast_to_area(
                                    Some(&instance_id),
                                    target_x,
                                    target_y,
                                    ServerMessage::DamageEvent {
                                        source_id: String::new(),
                                        target_id: target_id.clone(),
                                        damage,
                                        target_hp,
                                        target_x: target_x as f32,
                                        target_y: target_y as f32,
                                        projectile: Some("pharaoh_projectile".to_string()),
                                    },
                                )
                                .await;
                            }
                        }
                    } else {
                        let (effect_x, effect_y) = tiles.first().copied().unwrap_or_default();
                        self.broadcast_to_area(
                            Some(&instance_id),
                            effect_x,
                            effect_y,
                            ServerMessage::AoeWarning {
                                tiles,
                                delay_ms,
                                effect,
                            },
                        )
                        .await;
                    }
                }
                BossEvent::AoeDamage {
                    instance_id,
                    tiles,
                    damage,
                    effect,
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

                    // Apply damage to hit players and send damage events
                    let mut died_players = Vec::new();
                    {
                        let mut players = self.players.write().await;
                        for pid in &hit_players {
                            if let Some(player) = players.get_mut(pid) {
                                player.hp = (player.hp - damage).max(0);
                                if player.hp <= 0 && !player.is_dead {
                                    player.die(current_time);
                                    died_players.push(pid.clone());
                                }
                            }
                        }
                    }

                    // Send PlayerDied for any players killed by AOE
                    for pid in &died_players {
                        self.broadcast_to_zone(
                            pid,
                            ServerMessage::PlayerDied {
                                id: pid.clone(),
                                killer_id: "desert_wurm".to_string(),
                            },
                        )
                        .await;
                    }

                    // Send floating damage numbers for each hit player
                    let hit_updates = {
                        let players = self.players.read().await;
                        hit_players
                            .iter()
                            .filter_map(|pid| {
                                players
                                    .get(pid)
                                    .map(|player| (pid.clone(), player.hp, player.x, player.y))
                            })
                            .collect::<Vec<_>>()
                    };
                    for (pid, hp, x, y) in hit_updates {
                        self.broadcast_to_area(
                            Some(&instance_id),
                            x,
                            y,
                            ServerMessage::DamageEvent {
                                source_id: String::new(),
                                target_id: pid,
                                damage,
                                target_hp: hp,
                                target_x: x as f32,
                                target_y: y as f32,
                                projectile: None,
                            },
                        )
                        .await;
                    }

                    let (effect_x, effect_y) = tiles.first().copied().unwrap_or_default();
                    self.broadcast_to_area(
                        Some(&instance_id),
                        effect_x,
                        effect_y,
                        ServerMessage::AoeDamage {
                            tiles,
                            damage,
                            effect,
                        },
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
                    let mut hit_players = Vec::new();
                    let mut died_players = Vec::new();
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
                                        died_players.push(pid.clone());
                                    }
                                    hit_players.push(pid.clone());
                                }
                            }
                        }
                    }

                    // Send PlayerDied for any players killed by explosion
                    for pid in &died_players {
                        self.broadcast_to_zone(
                            pid,
                            ServerMessage::PlayerDied {
                                id: pid.clone(),
                                killer_id: "exploding_rock".to_string(),
                            },
                        )
                        .await;
                    }

                    // Send floating damage numbers for explosion hits
                    let hit_updates = {
                        let players = self.players.read().await;
                        hit_players
                            .iter()
                            .filter_map(|pid| {
                                players
                                    .get(pid)
                                    .map(|player| (pid.clone(), player.hp, player.x, player.y))
                            })
                            .collect::<Vec<_>>()
                    };
                    for (pid, hp, x, y) in hit_updates {
                        self.broadcast_to_area(
                            Some(&instance_id),
                            x,
                            y,
                            ServerMessage::DamageEvent {
                                source_id: String::new(),
                                target_id: pid,
                                damage,
                                target_hp: hp,
                                target_x: x as f32,
                                target_y: y as f32,
                                projectile: None,
                            },
                        )
                        .await;
                    }

                    // Damage boss if any of its occupied tiles are in blast zone
                    {
                        let boss_npc_id = {
                            let boss_states = self.boss_states.read().await;
                            boss_states
                                .get(&instance_id)
                                .map(|b| (b.boss_npc_id.clone(), b.boss_x, b.boss_y))
                        };
                        if let Some((npc_id, bx, by)) = boss_npc_id {
                            // Boss is 2x2 — check all occupied tiles
                            let boss_hit = crate::npc::npc_occupied_tiles(bx, by, 2)
                                .any(|tile| blast_tiles.contains(&tile));
                            if boss_hit {
                                // Apply damage to the actual NPC
                                let boss_hp_after = if let Some(instance) =
                                    self.instance_manager.get_by_instance_id(&instance_id)
                                {
                                    let mut npcs = instance.npcs.write().await;
                                    if let Some(npc) = npcs.get_mut(&npc_id) {
                                        npc.hp = (npc.hp - damage).max(0);
                                        Some(npc.hp)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };
                                // Show damage number on boss
                                if let Some(hp) = boss_hp_after {
                                    self.broadcast_to_area(
                                        Some(&instance_id),
                                        bx,
                                        by,
                                        ServerMessage::DamageEvent {
                                            source_id: String::new(),
                                            target_id: npc_id.clone(),
                                            damage,
                                            target_hp: hp,
                                            target_x: bx as f32,
                                            target_y: by as f32,
                                            projectile: None,
                                        },
                                    )
                                    .await;
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
                                if npc_id.starts_with("boss_minion_") && npc.is_alive() {
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
                            for (npc_id, _mx, _my) in &chain_minions {
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

                    self.broadcast_to_area(
                        Some(&instance_id),
                        x,
                        y,
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
                        }
                    }
                }
                BossEvent::SetBossNpcState {
                    instance_id,
                    npc_id,
                    state,
                } => {
                    if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                        let mut npcs = instance.npcs.write().await;
                        if let Some(npc) = npcs.get_mut(&npc_id) {
                            npc.state = match state {
                                6 => NpcState::Submerging,
                                7 => NpcState::Emerging,
                                8 => NpcState::Burrowing,
                                _ => NpcState::Idle,
                            };
                        }
                    }
                }
                BossEvent::HideBoss {
                    instance_id,
                    npc_id,
                    hidden,
                } => {
                    if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                        let mut npcs = instance.npcs.write().await;
                        if let Some(npc) = npcs.get_mut(&npc_id) {
                            npc.hidden = hidden;
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
                    {
                        let mut pharaoh_states = self.pharaoh_boss_states.write().await;
                        if let Some(boss) = pharaoh_states.get_mut(&instance_id) {
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
                        npcs.retain(|id, _| {
                            !id.starts_with("boss_minion_") && !id.starts_with("pharaoh_minion_")
                        });
                    }

                    // Roll loot for each damage dealer
                    let damage_dealers = {
                        let boss_states = self.boss_states.read().await;
                        if let Some(b) = boss_states.get(&instance_id) {
                            b.damage_dealers.clone()
                        } else {
                            drop(boss_states);
                            let pharaoh_states = self.pharaoh_boss_states.read().await;
                            pharaoh_states
                                .get(&instance_id)
                                .map(|b| b.damage_dealers.clone())
                                .unwrap_or_default()
                        }
                    };

                    // Determine which boss prototype to use for loot
                    let boss_prototype_id = {
                        let pharaoh_states = self.pharaoh_boss_states.read().await;
                        if pharaoh_states.contains_key(&instance_id) {
                            "cursed_pharaoh"
                        } else {
                            "desert_wurm"
                        }
                    };
                    if let Some(prototype) = self.entity_registry.get(boss_prototype_id) {
                        let player_names: std::collections::HashMap<String, String> = {
                            let players = self.players.read().await;
                            damage_dealers
                                .iter()
                                .filter_map(|pid| {
                                    players.get(pid).map(|p| (pid.clone(), p.name.clone()))
                                })
                                .collect()
                        };

                        // Roll all loot synchronously (ThreadRng is not Send)
                        // Each entry: (player_id, player_name, gold, Vec<(item_id, quantity, display_name)>)
                        let rolled_loot: Vec<RolledLoot> = {
                            let mut rng = rand::thread_rng();
                            damage_dealers
                                .iter()
                                .map(|pid| {
                                    let gold = rng.gen_range(
                                        prototype.rewards.gold_min..=prototype.rewards.gold_max,
                                    );
                                    let mut items = Vec::new();
                                    // Flat loot (independent rolls)
                                    for entry in &prototype.loot {
                                        if rng.r#gen::<f32>() < entry.drop_chance {
                                            let quantity = rng
                                                .gen_range(entry.quantity_min..=entry.quantity_max);
                                            let display_name = self
                                                .item_registry
                                                .get(&entry.item_id)
                                                .map(|item| item.display_name.clone())
                                                .unwrap_or_else(|| entry.item_id.clone());
                                            items.push((
                                                entry.item_id.clone(),
                                                quantity,
                                                display_name,
                                            ));
                                        }
                                    }
                                    // Roll tables (weighted, pick one per table)
                                    for table in &prototype.loot_tables {
                                        if rng.r#gen::<f32>() >= table.chance {
                                            continue;
                                        }
                                        let total_weight: i32 =
                                            table.entries.iter().map(|e| e.weight).sum();
                                        if total_weight <= 0 {
                                            continue;
                                        }
                                        let mut roll = rng.gen_range(0..total_weight);
                                        for entry in &table.entries {
                                            roll -= entry.weight;
                                            if roll < 0 {
                                                if entry.item_id != "nothing" {
                                                    let quantity = rng.gen_range(
                                                        entry.quantity_min..=entry.quantity_max,
                                                    );
                                                    let display_name = self
                                                        .item_registry
                                                        .get(&entry.item_id)
                                                        .map(|item| item.display_name.clone())
                                                        .unwrap_or_else(|| entry.item_id.clone());
                                                    items.push((
                                                        entry.item_id.clone(),
                                                        quantity,
                                                        display_name,
                                                    ));
                                                }
                                                break;
                                            }
                                        }
                                    }
                                    let player_name = player_names
                                        .get(pid)
                                        .cloned()
                                        .unwrap_or_else(|| pid.clone());
                                    (pid.clone(), player_name, gold, items)
                                })
                                .collect()
                        };

                        // Persist to DB and build announcement
                        let mut all_loot_lines: Vec<String> = Vec::new();
                        for (pid, player_name, gold, items) in &rolled_loot {
                            if let Some(db) = self.db.as_ref() {
                                if let Err(e) =
                                    db.add_boss_pending_reward(pid, "gold", *gold as u32).await
                                {
                                    tracing::error!(
                                        "Failed to persist boss gold reward for {}: {}",
                                        pid,
                                        e
                                    );
                                }
                                for (item_id, quantity, _) in items {
                                    if let Err(e) = db
                                        .add_boss_pending_reward(pid, item_id, *quantity as u32)
                                        .await
                                    {
                                        tracing::error!(
                                            "Failed to persist boss loot reward for {}: {}",
                                            pid,
                                            e
                                        );
                                    }
                                }
                            }

                            // Record collection log entries for boss rewards
                            for (item_id, _, _) in items {
                                self.record_collection_entry(
                                    pid,
                                    item_id,
                                    "boss_rewards",
                                    boss_prototype_id,
                                )
                                .await;
                            }

                            let mut display_parts: Vec<String> = items
                                .iter()
                                .map(|(_, qty, name)| format!("{}x {}", qty, name))
                                .collect();
                            display_parts.push(format!("{} gold", gold));

                            all_loot_lines.push(format!(
                                "{} received: {}",
                                player_name,
                                display_parts.join(", ")
                            ));
                        }

                        if !all_loot_lines.is_empty() {
                            for line in &all_loot_lines {
                                self.send_to_instance(
                                    &instance_id,
                                    ServerMessage::ChatMessage {
                                        sender_id: "system".to_string(),
                                        sender_name: "[System]".to_string(),
                                        text: line.clone(),
                                        timestamp: current_time,
                                        channel: "system".to_string(),
                                    },
                                )
                                .await;
                            }
                        }
                    } else {
                        tracing::error!(
                            "Could not find {} prototype for loot rolling",
                            boss_prototype_id
                        );
                    }

                    // Send initial countdown as system message
                    self.send_to_instance(
                        &instance_id,
                        ServerMessage::ChatMessage {
                            sender_id: String::new(),
                            sender_name: String::new(),
                            text: "Returning to overworld in 3...".to_string(),
                            timestamp: current_time,
                            channel: "system".to_string(),
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
                        ServerMessage::ChatMessage {
                            sender_id: String::new(),
                            sender_name: String::new(),
                            text: message,
                            timestamp: current_time,
                            channel: "system".to_string(),
                        },
                    )
                    .await;
                }
                BossEvent::TeleportOut { instance_id } => {
                    // Reset boss NPC to full HP for the next fight
                    if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                        let mut npcs = instance.npcs.write().await;
                        for npc in npcs.values_mut() {
                            if npc.prototype_id == "desert_wurm"
                                || npc.prototype_id == "cursed_pharaoh"
                            {
                                npc.hp = npc.max_hp;
                                npc.state = NpcState::Idle;
                                npc.hidden = false;
                                npc.invulnerable = false;
                                npc.death_time = 0;
                            }
                        }
                        // Clean up any remaining pharaoh minions
                        npcs.retain(|id, _| !id.starts_with("pharaoh_minion_"));
                    }

                    let player_ids = self.get_instance_player_ids(&instance_id).await;
                    let spawn_x: i32 = -258;
                    let spawn_y: i32 = -125;

                    for pid in player_ids {
                        self.player_instances.write().await.remove(&pid);
                        self.reset_sync_state(&pid).await;

                        if let Some(instance) =
                            self.instance_manager.get_by_instance_id(&instance_id)
                        {
                            instance.remove_player(&pid).await;
                        }

                        // Update player position server-side
                        {
                            let mut players = self.players.write().await;
                            if let Some(player) = players.get_mut(&pid) {
                                player.x = spawn_x;
                                player.y = spawn_y;
                                player.z = 0;
                            }
                        }

                        // Preload overworld chunks around the exit before transitioning
                        let exit_chunk = ChunkCoord::from_world(spawn_x, spawn_y);
                        self.world()
                            .preload_chunks(exit_chunk, super::SPAWN_PRELOAD_RADIUS)
                            .await;

                        // Spawn at the boss cave exit
                        self.send_to_player(
                            &pid,
                            ServerMessage::MapTransition {
                                map_type: "overworld".to_string(),
                                map_id: "world_0".to_string(),
                                spawn_x: spawn_x as f32,
                                spawn_y: spawn_y as f32,
                                instance_id: String::new(),
                            },
                        )
                        .await;

                        // Re-send overworld data that was cleared on instance entry
                        self.send_to_player(&pid, self.get_chair_positions_message().await)
                            .await;
                        self.send_to_player(&pid, self.get_gathering_markers_message(None).await)
                            .await;
                        self.send_to_player(&pid, self.get_chest_positions_message(None).await)
                            .await;

                        // Send overworld ground items
                        for item_msg in self.get_visible_ground_items(&pid).await {
                            self.send_to_player(&pid, item_msg).await;
                        }

                        // Notify overworld players that this player has returned
                        {
                            let player_name = self.get_player_name(&pid).await.unwrap_or_default();
                            let (gender, skin) = self
                                .get_player_appearance(&pid)
                                .await
                                .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
                            let (hair_style, hair_color) =
                                self.get_player_hair(&pid).await.unwrap_or((None, None));
                            self.send_to_overworld_players(
                                ServerMessage::PlayerJoined {
                                    id: pid.clone(),
                                    name: player_name,
                                    x: spawn_x,
                                    y: spawn_y,
                                    gender,
                                    skin,
                                    hair_style,
                                    hair_color,
                                },
                                Some(&pid),
                            )
                            .await;
                        }
                    }
                }
            }
        })
    }

    pub(super) async fn send_to_instance(&self, instance_id: &str, msg: ServerMessage) {
        let player_ids = self.get_instance_player_ids(instance_id).await;
        if player_ids.is_empty() {
            return;
        }
        for pid in player_ids {
            self.send_to_player(&pid, msg.clone()).await;
        }
    }

    pub(super) async fn get_instance_player_ids(&self, instance_id: &str) -> Vec<String> {
        let instances = self.player_instances.read().await;
        instances
            .iter()
            .filter(|(_, iid)| iid.as_str() == instance_id)
            .map(|(pid, _)| pid.clone())
            .collect()
    }
}
