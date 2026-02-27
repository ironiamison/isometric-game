use super::*;

const MAX_CHAT_MESSAGE_CHARS: usize = 200;
const ADMIN_COMMANDS: &[&str] = &[
    "/give",
    "/setlevel",
    "/teleport",
    "/tpto",
    "/spawn",
    "/heal",
    "/kill",
    "/god",
    "/announce",
    "/arena",
];

fn sanitize_chat_text(text: &str) -> Option<String> {
    let sanitized = text
        .trim()
        .chars()
        .take(MAX_CHAT_MESSAGE_CHARS)
        .collect::<String>();
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

fn chat_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

impl GameRoom {
    pub async fn handle_chat(&self, player_id: &str, text: &str, channel: &str) {
        let Some(sanitized) = sanitize_chat_text(text) else {
            return;
        };

        if sanitized.starts_with('/') {
            self.handle_chat_command(player_id, &sanitized).await;
            return;
        }

        let players = self.players.read().await;
        let sender = match players.get(player_id) {
            Some(p) => p,
            None => return,
        };
        let sender_name = sender.name.clone();
        let sender_x = sender.x as i32;
        let sender_y = sender.y as i32;
        drop(players);

        let timestamp = chat_timestamp_ms();

        if channel == "global" {
            let msg = ServerMessage::ChatMessage {
                sender_id: player_id.to_string(),
                sender_name,
                text: sanitized,
                timestamp,
                channel: "global".to_string(),
            };
            self.broadcast(msg).await;
        } else {
            let msg = ServerMessage::ChatMessage {
                sender_id: player_id.to_string(),
                sender_name,
                text: sanitized,
                timestamp,
                channel: "public".to_string(),
            };

            let player_instances = self.player_instances.read().await;
            let sender_instance = player_instances.get(player_id).cloned();
            let all_players = self.players.read().await;
            let senders = self.player_senders.read().await;

            if let Ok(bytes) = crate::protocol::encode_server_message(&msg) {
                for (pid, sender_ch) in senders.iter() {
                    if player_instances.get(pid).cloned() != sender_instance {
                        continue;
                    }
                    if pid == player_id {
                        let _ = sender_ch.try_send(bytes.clone());
                        continue;
                    }
                    if let Some(other) = all_players.get(pid) {
                        let dx = (other.x as i32 - sender_x).abs();
                        let dy = (other.y as i32 - sender_y).abs();
                        if dx.max(dy) <= VIEW_DISTANCE {
                            let _ = sender_ch.try_send(bytes.clone());
                        }
                    }
                }
            }
        }
    }

    async fn handle_chat_command(&self, player_id: &str, text: &str) {
        let parts: Vec<&str> = text.split_whitespace().collect();
        let command = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();

        let is_admin = {
            let players = self.players.read().await;
            players.get(player_id).map(|p| p.is_admin).unwrap_or(false)
        };

        if ADMIN_COMMANDS.contains(&command.as_str()) && !is_admin {
            self.send_system_message(player_id, "This command requires admin privileges.")
                .await;
            return;
        }

        match command.as_str() {
            "/give" => {
                if parts.len() < 2 {
                    self.send_system_message(player_id, "Usage: /give <item_id> [quantity]")
                        .await;
                    return;
                }

                let item_id = parts[1];
                let quantity = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);

                if self.item_registry.get(item_id).is_none() {
                    self.send_system_message(player_id, &format!("Unknown item: {}", item_id))
                        .await;
                    return;
                }

                let (leftover, inventory_update, gold) = {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        let leftover =
                            player
                                .inventory
                                .add_item(item_id, quantity, &self.item_registry);
                        (
                            leftover,
                            player.inventory.to_update(),
                            player.inventory.gold,
                        )
                    } else {
                        (quantity, vec![], 0)
                    }
                };

                let added = quantity - leftover;
                if added > 0 {
                    tracing::info!("Player {} spawned {}x {}", player_id, added, item_id);
                    self.send_system_message(player_id, &format!("Gave {}x {}", added, item_id))
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
                } else {
                    self.send_system_message(player_id, "Inventory full").await;
                }
            }
            "/setlevel" => {
                use crate::skills::{Skill, SkillType};

                if parts.len() < 2 {
                    self.send_system_message(
                        player_id,
                        "Usage: /setlevel <skill> <level> or /setlevel <level>",
                    )
                    .await;
                    return;
                }

                let (skill_type, level) = if parts.len() >= 3 {
                    let skill_type = match SkillType::from_str(parts[1]) {
                        Some(st) => st,
                        None => {
                            let valid: Vec<&str> =
                                SkillType::all().iter().map(|s| s.as_str()).collect();
                            self.send_system_message(
                                player_id,
                                &format!(
                                    "Unknown skill '{}'. Valid skills: {}",
                                    parts[1],
                                    valid.join(", ")
                                ),
                            )
                            .await;
                            return;
                        }
                    };
                    let level: i32 = match parts[2].parse() {
                        Ok(l) if (1..=99).contains(&l) => l,
                        _ => {
                            self.send_system_message(player_id, "Level must be between 1 and 99")
                                .await;
                            return;
                        }
                    };
                    (Some(skill_type), level)
                } else {
                    let level: i32 = match parts[1].parse() {
                        Ok(l) if (1..=99).contains(&l) => l,
                        _ => {
                            if SkillType::from_str(parts[1]).is_some() {
                                self.send_system_message(
                                    player_id,
                                    "Usage: /setlevel <skill> <level>",
                                )
                                .await;
                            } else {
                                self.send_system_message(
                                    player_id,
                                    "Level must be between 1 and 99",
                                )
                                .await;
                            }
                            return;
                        }
                    };
                    (None, level)
                };

                {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        if let Some(st) = skill_type {
                            *player.skills.get_mut(st) = Skill::new(level);
                            if st == SkillType::Hitpoints {
                                player.hp = player.max_hp();
                            }
                            tracing::info!(
                                "Player {} set {} to level {}",
                                player_id,
                                st.as_str(),
                                level
                            );
                        } else {
                            for &st in SkillType::all() {
                                *player.skills.get_mut(st) = Skill::new(level);
                            }
                            player.hp = player.max_hp();
                            tracing::info!(
                                "Player {} set all skills to level {}",
                                player_id,
                                level
                            );
                        }
                    } else {
                        return;
                    }
                };

                if let Some(st) = skill_type {
                    self.send_system_message(
                        player_id,
                        &format!("{} set to level {}", st.as_str(), level),
                    )
                    .await;
                } else {
                    let combat_level = {
                        let players = self.players.read().await;
                        players
                            .get(player_id)
                            .map(|p| p.skills.combat_level())
                            .unwrap_or(0)
                    };
                    self.send_system_message(
                        player_id,
                        &format!(
                            "All skills set to level {} (Combat Level: {})",
                            level, combat_level
                        ),
                    )
                    .await;
                }
            }
            "/help" => {
                if is_admin {
                    self.send_system_message(player_id, "Commands: /give <item> [qty], /setlevel [skill] <lvl>, /teleport <x> <y>, /tpto <player>, /spawn <npc> [x] [y], /heal [player], /kill <player>, /god, /announce <msg>, /items, /help").await;
                } else {
                    self.send_system_message(player_id, "Commands: /items, /help")
                        .await;
                }
            }
            "/items" => {
                let items: Vec<&String> = self.item_registry.ids().collect();
                let list = items
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.send_system_message(player_id, &format!("Items: {}", list))
                    .await;
            }
            "/teleport" => {
                if parts.len() < 3 {
                    self.send_system_message(player_id, "Usage: /teleport <x> <y>")
                        .await;
                    return;
                }
                let x: i32 = match parts[1].parse() {
                    Ok(v) => v,
                    Err(_) => {
                        self.send_system_message(player_id, "Invalid x coordinate")
                            .await;
                        return;
                    }
                };
                let y: i32 = match parts[2].parse() {
                    Ok(v) => v,
                    Err(_) => {
                        self.send_system_message(player_id, "Invalid y coordinate")
                            .await;
                        return;
                    }
                };
                {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        player.x = x;
                        player.y = y;
                        tracing::info!("Player {} teleported to ({}, {})", player_id, x, y);
                    }
                }
                self.send_system_message(player_id, &format!("Teleported to ({}, {})", x, y))
                    .await;
                self.broadcast_to_zone(
                    player_id,
                    ServerMessage::SpellEffect {
                        caster_id: player_id.to_string(),
                        target_id: None,
                        spell_id: "teleport".to_string(),
                        target_x: x,
                        target_y: y,
                    },
                )
                .await;
            }
            "/tpto" => {
                if parts.len() < 2 {
                    self.send_system_message(player_id, "Usage: /tpto <player_name>")
                        .await;
                    return;
                }
                let target_name = parts[1];

                let target_info = {
                    let players = self.players.read().await;
                    players
                        .values()
                        .find(|p| p.name.eq_ignore_ascii_case(target_name))
                        .map(|t| (t.id.clone(), t.name.clone(), t.x, t.y))
                };

                let (target_id, target_display, target_x, target_y) = match target_info {
                    Some(info) => info,
                    None => {
                        self.send_system_message(player_id, "Player not found")
                            .await;
                        return;
                    }
                };

                let target_instance = {
                    let instances = self.player_instances.read().await;
                    instances.get(&target_id).cloned()
                };

                let sender_instance = {
                    let instances = self.player_instances.read().await;
                    instances.get(player_id).cloned()
                };

                if sender_instance != target_instance {
                    self.send_system_message(
                        player_id,
                        "Target is in a different instance. Use a portal to enter that interior.",
                    )
                    .await;
                    return;
                }

                {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        player.x = target_x;
                        player.y = target_y;
                        tracing::info!(
                            "Player {} teleported to {} at ({}, {})",
                            player_id,
                            target_id,
                            target_x,
                            target_y
                        );
                    }
                }

                self.send_system_message(
                    player_id,
                    &format!(
                        "Teleported to {} at ({}, {})",
                        target_display, target_x, target_y
                    ),
                )
                .await;

                self.broadcast_to_zone(
                    player_id,
                    ServerMessage::SpellEffect {
                        caster_id: player_id.to_string(),
                        target_id: None,
                        spell_id: "teleport".to_string(),
                        target_x,
                        target_y,
                    },
                )
                .await;
            }
            "/spawn" => {
                if parts.len() < 2 {
                    self.send_system_message(player_id, "Usage: /spawn <npc_id> [x] [y]")
                        .await;
                    return;
                }
                let npc_id = parts[1];

                let (spawn_x, spawn_y) = {
                    let players = self.players.read().await;
                    if let Some(player) = players.get(player_id) {
                        let x = parts
                            .get(2)
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(player.x);
                        let y = parts
                            .get(3)
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(player.y);
                        (x, y)
                    } else {
                        return;
                    }
                };

                if self.entity_registry.get(npc_id).is_none() {
                    let available: Vec<&String> = self.entity_registry.ids().collect();
                    let list = available
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.send_system_message(
                        player_id,
                        &format!("Unknown NPC: {}. Available: {}", npc_id, list),
                    )
                    .await;
                    return;
                }

                if let Some(spawned_id) = self
                    .spawn_npc_at(npc_id, spawn_x as f32, spawn_y as f32)
                    .await
                {
                    self.send_system_message(
                        player_id,
                        &format!(
                            "Spawned {} at ({}, {}) [id: {}]",
                            npc_id, spawn_x, spawn_y, spawned_id
                        ),
                    )
                    .await;
                    tracing::info!(
                        "Admin {} spawned {} at ({}, {})",
                        player_id,
                        npc_id,
                        spawn_x,
                        spawn_y
                    );
                }
            }
            "/heal" => {
                let target_name = parts.get(1).copied();

                let healed = {
                    let mut players = self.players.write().await;
                    if let Some(name) = target_name {
                        if let Some(player) = players
                            .values_mut()
                            .find(|p| p.name.eq_ignore_ascii_case(name))
                        {
                            player.hp = player.max_hp();
                            player.is_dead = false;
                            Some(player.name.clone())
                        } else {
                            None
                        }
                    } else if let Some(player) = players.get_mut(player_id) {
                        player.hp = player.max_hp();
                        player.is_dead = false;
                        Some(player.name.clone())
                    } else {
                        None
                    }
                };

                match healed {
                    Some(name) => {
                        self.send_system_message(player_id, &format!("Healed {} to full HP", name))
                            .await
                    }
                    None => {
                        self.send_system_message(player_id, "Player not found")
                            .await
                    }
                }
            }
            "/kill" => {
                if parts.len() < 2 {
                    self.send_system_message(player_id, "Usage: /kill <player_name>")
                        .await;
                    return;
                }
                let target_name = parts[1];

                let killed = {
                    let current_time = chat_timestamp_ms();
                    let mut players = self.players.write().await;
                    if let Some(player) = players
                        .values_mut()
                        .find(|p| p.name.eq_ignore_ascii_case(target_name))
                    {
                        let id = player.id.clone();
                        let name = player.name.clone();
                        player.die(current_time);
                        Some((id, name))
                    } else {
                        None
                    }
                };

                match killed {
                    Some((target_id, name)) => {
                        self.send_system_message(player_id, &format!("Killed {}", name))
                            .await;
                        tracing::info!("Admin {} killed player {}", player_id, name);

                        let arena_death = {
                            let arena = self.arena_manager.read().await;
                            arena.is_fighting() && arena.is_in_ring(&target_id)
                        };
                        if arena_death {
                            let (eliminated_name, killer_name, remaining) = {
                                let mut arena = self.arena_manager.write().await;
                                arena.on_player_death(&target_id, Some(player_id));
                                let eliminated_name = arena
                                    .match_stats
                                    .fighter_names
                                    .get(&target_id)
                                    .cloned()
                                    .unwrap_or_default();
                                let killer_name = arena
                                    .match_stats
                                    .fighter_names
                                    .get(player_id)
                                    .cloned()
                                    .unwrap_or_default();
                                let remaining = arena.active_fighters.len() as u32;
                                (eliminated_name, killer_name, remaining)
                            };

                            {
                                let spectator_spawn = {
                                    let arena = self.arena_manager.read().await;
                                    arena.active_spectator_spawn()
                                };
                                let mut players = self.players.write().await;
                                if let Some(p) = players.get_mut(&target_id) {
                                    p.hp = p.skills.hitpoints.level;
                                    p.is_dead = false;
                                    p.x = spectator_spawn.0;
                                    p.y = spectator_spawn.1;
                                }
                            }

                            self.broadcast_to_arena(ServerMessage::ArenaPlayerEliminated {
                                player_id: target_id.clone(),
                                player_name: eliminated_name,
                                killer_id: player_id.to_string(),
                                killer_name,
                                remaining,
                            })
                            .await;

                            let should_end = {
                                let arena = self.arena_manager.read().await;
                                arena.check_match_end()
                            };
                            if should_end {
                                let placements = {
                                    let mut arena = self.arena_manager.write().await;
                                    arena.end_match(chat_timestamp_ms())
                                };

                                {
                                    let mut players = self.players.write().await;
                                    for placement in &placements {
                                        if placement.gold_reward > 0 {
                                            if let Some(p) = players.get_mut(&placement.player_id) {
                                                p.inventory.gold += placement.gold_reward;
                                            }
                                        }
                                    }
                                }

                                self.broadcast_to_arena(ServerMessage::ArenaMatchEnd {
                                    placements: placements
                                        .iter()
                                        .map(|p| crate::protocol::ArenaPlacementData {
                                            rank: p.rank,
                                            player_id: p.player_id.clone(),
                                            player_name: p.player_name.clone(),
                                            kills: p.kills,
                                            gold_reward: p.gold_reward,
                                        })
                                        .collect(),
                                })
                                .await;

                                self.broadcast_to_arena(ServerMessage::ArenaStateUpdate {
                                    state: "results".to_string(),
                                    countdown_remaining: None,
                                    queued_count: 0,
                                    fighter_count: 0,
                                    entry_fee: {
                                        let arena = self.arena_manager.read().await;
                                        arena.config.entry_fee
                                    },
                                })
                                .await;
                            }
                        } else {
                            self.broadcast(ServerMessage::PlayerDied {
                                id: target_id,
                                killer_id: player_id.to_string(),
                            })
                            .await;
                        }
                    }
                    None => {
                        self.send_system_message(player_id, "Player not found")
                            .await
                    }
                }
            }
            "/god" => {
                let (enabled, player_name) = {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        player.is_god_mode = !player.is_god_mode;
                        (player.is_god_mode, player.name.clone())
                    } else {
                        return;
                    }
                };
                let status = if enabled { "enabled" } else { "disabled" };
                self.send_system_message(player_id, &format!("God mode {}", status))
                    .await;
                tracing::info!(
                    "Admin {} ({}) toggled god mode: {}",
                    player_name,
                    player_id,
                    status
                );
            }
            "/announce" => {
                if parts.len() < 2 {
                    self.send_system_message(player_id, "Usage: /announce <message>")
                        .await;
                    return;
                }
                let message = parts[1..].join(" ");

                self.broadcast(ServerMessage::Announcement {
                    text: message.clone(),
                })
                .await;

                self.broadcast(ServerMessage::ChatMessage {
                    sender_id: "system".to_string(),
                    sender_name: "[Announcement]".to_string(),
                    text: message.clone(),
                    timestamp: chat_timestamp_ms(),
                    channel: "public".to_string(),
                })
                .await;

                tracing::info!("Admin {} announced: {}", player_id, message);
            }
            "/arena" => {
                if !is_admin {
                    self.send_system_message(player_id, "Arena commands require admin privileges.")
                        .await;
                    return;
                }

                let sub = parts.get(1).map(|s| s.to_lowercase()).unwrap_or_default();
                let current_time = chat_timestamp_ms();

                match sub.as_str() {
                    "fee" => {
                        if let Some(fee) = parts.get(2).and_then(|s| s.parse::<i32>().ok()) {
                            if fee < 0 {
                                self.send_system_message(player_id, "Fee must be non-negative.")
                                    .await;
                                return;
                            }
                            let mut arena = self.arena_manager.write().await;
                            arena.set_entry_fee(fee);
                            self.send_system_message(
                                player_id,
                                &format!("Arena entry fee set to {} gold.", fee),
                            )
                            .await;
                        } else {
                            self.send_system_message(player_id, "Usage: /arena fee <gold>")
                                .await;
                        }
                    }
                    "start" => {
                        let result = {
                            let mut arena = self.arena_manager.write().await;
                            arena.start_countdown(current_time, None)
                        };
                        match result {
                            Ok(charges) => {
                                {
                                    let mut players = self.players.write().await;
                                    for (pid, amount) in &charges {
                                        if let Some(p) = players.get_mut(pid) {
                                            p.inventory.gold -= amount;
                                        }
                                    }
                                }
                                for (pid, _) in &charges {
                                    let update = {
                                        let players = self.players.read().await;
                                        players
                                            .get(pid)
                                            .map(|p| (p.inventory.to_update(), p.inventory.gold))
                                    };
                                    if let Some((slots, gold)) = update {
                                        self.send_to_player(
                                            pid,
                                            ServerMessage::InventoryUpdate {
                                                player_id: pid.clone(),
                                                slots,
                                                gold,
                                            },
                                        )
                                        .await;
                                    }
                                }
                                self.send_system_message(player_id, "Arena countdown started!")
                                    .await;
                            }
                            Err(e) => {
                                self.send_system_message(player_id, &e).await;
                            }
                        }
                    }
                    "timer" => {
                        if let Some(seconds) = parts.get(2).and_then(|s| s.parse::<u64>().ok()) {
                            let result = {
                                let mut arena = self.arena_manager.write().await;
                                arena.start_countdown(current_time, Some(seconds * 1000))
                            };
                            match result {
                                Ok(charges) => {
                                    {
                                        let mut players = self.players.write().await;
                                        for (pid, amount) in &charges {
                                            if let Some(p) = players.get_mut(pid) {
                                                p.inventory.gold -= amount;
                                            }
                                        }
                                    }
                                    for (pid, _) in &charges {
                                        let update = {
                                            let players = self.players.read().await;
                                            players.get(pid).map(|p| {
                                                (p.inventory.to_update(), p.inventory.gold)
                                            })
                                        };
                                        if let Some((slots, gold)) = update {
                                            self.send_to_player(
                                                pid,
                                                ServerMessage::InventoryUpdate {
                                                    player_id: pid.clone(),
                                                    slots,
                                                    gold,
                                                },
                                            )
                                            .await;
                                        }
                                    }
                                    self.send_system_message(
                                        player_id,
                                        &format!("Arena countdown started ({}s)!", seconds),
                                    )
                                    .await;
                                }
                                Err(e) => self.send_system_message(player_id, &e).await,
                            }
                        } else {
                            self.send_system_message(player_id, "Usage: /arena timer <seconds>")
                                .await;
                        }
                    }
                    "cancel" => {
                        let refunds = {
                            let mut arena = self.arena_manager.write().await;
                            arena.cancel()
                        };
                        {
                            let mut players = self.players.write().await;
                            for (pid, amount) in &refunds {
                                if let Some(p) = players.get_mut(pid) {
                                    p.inventory.gold += amount;
                                }
                            }
                        }
                        for (pid, _) in &refunds {
                            let update = {
                                let players = self.players.read().await;
                                players
                                    .get(pid)
                                    .map(|p| (p.inventory.to_update(), p.inventory.gold))
                            };
                            if let Some((slots, gold)) = update {
                                self.send_to_player(
                                    pid,
                                    ServerMessage::InventoryUpdate {
                                        player_id: pid.clone(),
                                        slots,
                                        gold,
                                    },
                                )
                                .await;
                            }
                        }
                        self.send_system_message(player_id, "Arena cancelled. All fees refunded.")
                            .await;
                    }
                    "status" => {
                        let status = {
                            let arena = self.arena_manager.read().await;
                            arena.get_status_text()
                        };
                        self.send_system_message(player_id, &status).await;
                    }
                    _ => {
                        self.send_system_message(
                            player_id,
                            "Usage: /arena <start|timer|fee|cancel|status>",
                        )
                        .await;
                    }
                }
            }
            _ => {
                self.send_system_message(
                    player_id,
                    &format!("Unknown command: {}. Try /help", command),
                )
                .await;
            }
        }
    }

    pub(super) async fn send_system_message(&self, player_id: &str, text: &str) {
        let msg = ServerMessage::ChatMessage {
            sender_id: "system".to_string(),
            sender_name: "[System]".to_string(),
            text: text.to_string(),
            timestamp: chat_timestamp_ms(),
            channel: "system".to_string(),
        };
        self.send_to_player(player_id, msg).await;
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_chat_text;

    #[test]
    fn sanitize_chat_text_trims_and_rejects_empty_messages() {
        assert_eq!(sanitize_chat_text("   hello   ").as_deref(), Some("hello"));
        assert_eq!(sanitize_chat_text("   "), None);
    }

    #[test]
    fn sanitize_chat_text_caps_messages_at_200_characters() {
        let input = "x".repeat(250);
        let sanitized = sanitize_chat_text(&input).expect("message preserved");

        assert_eq!(sanitized.len(), 200);
        assert!(sanitized.chars().all(|ch| ch == 'x'));
    }
}
