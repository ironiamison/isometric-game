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
    "/boss",
    "/kick",
    "/ban",
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
        let sender_name = if let Some(ref title) = sender.active_title {
            format!("{} ({})", sender.name, title)
        } else {
            sender.name.clone()
        };
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

                // Re-initialize rankings from DB and broadcast to all clients
                self.init_top_level_player().await;
                let top_msg = self.get_top_player_message().await;
                self.broadcast(top_msg).await;
            }
            "/help" => {
                if is_admin {
                    self.send_system_message(player_id, "Commands: /give <item> [qty], /setlevel [skill] <lvl>, /teleport <x> <y>, /tpto <player>, /spawn <npc> [x] [y], /heal [player], /kill <player>, /god, /announce <msg>, /kick <player>, /ban <player> <hours> [reason], /items, /help").await;
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
            "/boss" => {
                use crate::protocol::{
                    ChunkLayerData, ChunkObjectData, ChunkPortalData, ChunkWallData,
                };
                use base64::Engine;

                let map_id = crate::game::boss_tick::BOSS_MAP_ID;

                let interior = match self.interior_registry.get(map_id) {
                    Some(i) => i,
                    None => {
                        self.send_system_message(player_id, "Boss arena map not found.")
                            .await;
                        return;
                    }
                };

                let spawn = match interior.spawn_points.values().next() {
                    Some(s) => s.clone(),
                    None => {
                        self.send_system_message(player_id, "Boss arena has no spawn point.")
                            .await;
                        return;
                    }
                };

                // Get or create public instance
                let (instance, is_new) = self.instance_manager.get_or_create_public(
                    &interior.id,
                    interior.size.width,
                    interior.size.height,
                    interior.pvp_enabled,
                );

                // Spawn NPCs and set collision if new instance
                if is_new || !*instance.npcs_spawned.read().await {
                    if !interior.collision.is_empty() {
                        if let Ok(bytes) =
                            base64::engine::general_purpose::STANDARD.decode(&interior.collision)
                        {
                            instance.set_collision(&bytes).await;
                        }
                    }
                    instance
                        .spawn_npcs(&interior.entities, &self.entity_registry)
                        .await;

                    // Register gathering markers for this instance
                    if !interior.gathering_zones.is_empty() {
                        let markers: Vec<crate::gathering::GatheringMarker> = interior
                            .gathering_zones
                            .iter()
                            .map(|gz| crate::gathering::GatheringMarker {
                                x: gz.x,
                                y: gz.y,
                                zone_id: gz.zone_id.clone(),
                            })
                            .collect();
                        self.register_instance_gathering_markers(&instance.id, markers)
                            .await;
                    }
                }

                // Save current overworld position for return
                let (entrance_x, entrance_y) =
                    self.get_player_position(player_id).await.unwrap_or((0, 0));

                // Track player's instance
                {
                    let mut player_instances = self.player_instances.write().await;
                    player_instances.insert(player_id.to_string(), instance.id.clone());
                }
                self.reset_sync_state(player_id).await;

                // Notify overworld players that this player has "left"
                self.send_to_overworld_players(
                    ServerMessage::PlayerLeft {
                        id: player_id.to_string(),
                    },
                    Some(player_id),
                )
                .await;

                // Get other players already in the instance BEFORE adding
                let other_players_in_instance: Vec<String> = instance.get_player_ids().await;

                instance.add_player(player_id).await;

                // Set player position to spawn point
                self.set_player_position(player_id, spawn.x as i32, spawn.y as i32)
                    .await;

                // Notify other instance players about this player joining
                if !other_players_in_instance.is_empty() {
                    let player_name = self.get_player_name(player_id).await.unwrap_or_default();
                    let (gender, skin) = self
                        .get_player_appearance(player_id)
                        .await
                        .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
                    let (hair_style, hair_color) = self
                        .get_player_hair(player_id)
                        .await
                        .unwrap_or((None, None));

                    for other_id in &other_players_in_instance {
                        self.send_to_player(
                            other_id,
                            ServerMessage::PlayerJoined {
                                id: player_id.to_string(),
                                name: player_name.clone(),
                                x: spawn.x as i32,
                                y: spawn.y as i32,
                                gender: gender.clone(),
                                skin: skin.clone(),
                                hair_style: hair_style.clone(),
                                hair_color: hair_color.clone(),
                            },
                        )
                        .await;
                    }

                    // Tell the entering player about other players already in the instance
                    for other_id in &other_players_in_instance {
                        if let Some(other_name) = self.get_player_name(other_id).await {
                            let (other_x, other_y) = self
                                .get_player_position(other_id)
                                .await
                                .unwrap_or((spawn.x as i32, spawn.y as i32));
                            let (other_gender, other_skin) = self
                                .get_player_appearance(other_id)
                                .await
                                .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
                            let (other_hair_style, other_hair_color) =
                                self.get_player_hair(other_id).await.unwrap_or((None, None));

                            self.send_to_player(
                                player_id,
                                ServerMessage::PlayerJoined {
                                    id: other_id.clone(),
                                    name: other_name,
                                    x: other_x,
                                    y: other_y,
                                    gender: other_gender,
                                    skin: other_skin,
                                    hair_style: other_hair_style,
                                    hair_color: other_hair_color,
                                },
                            )
                            .await;
                        }
                    }
                }

                // Start or join boss session
                let ct = chat_timestamp_ms();

                if self.has_boss_session(&instance.id).await {
                    self.add_boss_player(&instance.id, player_id).await;
                } else {
                    let npcs = instance.npcs.read().await;
                    if let Some(boss_npc) = npcs.values().find(|n| n.prototype_id == "desert_wurm")
                    {
                        self.start_boss_session(
                            &instance.id,
                            &boss_npc.id,
                            boss_npc.hp,
                            boss_npc.max_hp,
                            boss_npc.x,
                            boss_npc.y,
                            instance.map_width as i32,
                            instance.map_height as i32,
                            ct,
                        )
                        .await;
                    }
                }

                // Send transition message
                self.send_to_player(
                    player_id,
                    ServerMessage::MapTransition {
                        map_type: "interior".to_string(),
                        map_id: interior.id.clone(),
                        spawn_x: spawn.x,
                        spawn_y: spawn.y,
                        instance_id: instance.id.clone(),
                    },
                )
                .await;

                // Build and send interior data
                let layers = vec![
                    ChunkLayerData {
                        layer_type: 0,
                        tiles: interior.layers.ground.clone(),
                    },
                    ChunkLayerData {
                        layer_type: 1,
                        tiles: interior.layers.objects.clone(),
                    },
                    ChunkLayerData {
                        layer_type: 2,
                        tiles: interior.layers.overhead.clone(),
                    },
                ];

                let collision = if interior.collision.is_empty() {
                    vec![]
                } else {
                    base64::engine::general_purpose::STANDARD
                        .decode(&interior.collision)
                        .unwrap_or_default()
                };

                let portals: Vec<ChunkPortalData> = interior
                    .portals
                    .iter()
                    .map(|p| ChunkPortalData {
                        id: p.id.clone(),
                        x: p.x,
                        y: p.y,
                        width: p.width,
                        height: p.height,
                        target_map: p.target_map.clone(),
                        target_spawn: p.target_spawn.clone().unwrap_or_default(),
                    })
                    .collect();

                let objects: Vec<ChunkObjectData> = interior
                    .map_objects
                    .iter()
                    .map(|o| ChunkObjectData {
                        gid: o.gid,
                        tile_x: o.x,
                        tile_y: o.y,
                        width: o.width,
                        height: o.height,
                    })
                    .collect();

                let walls: Vec<ChunkWallData> = interior
                    .walls
                    .iter()
                    .map(|w| ChunkWallData {
                        gid: w.gid,
                        tile_x: w.x,
                        tile_y: w.y,
                        edge: w.edge.clone(),
                    })
                    .collect();

                self.send_to_player(
                    player_id,
                    ServerMessage::InteriorData {
                        map_id: interior.id.clone(),
                        name: interior.name.clone(),
                        instance_id: instance.id.clone(),
                        width: interior.size.width,
                        height: interior.size.height,
                        spawn_x: spawn.x,
                        spawn_y: spawn.y,
                        layers,
                        collision,
                        portals,
                        objects,
                        walls,
                        heightmap: interior.heightmap.clone(),
                        block_types_down: interior.block_types_down.clone(),
                        block_types_right: interior.block_types_right.clone(),
                    },
                )
                .await;

                // Send NPC updates for this instance
                let npc_updates = instance.get_npc_updates().await;
                if !npc_updates.is_empty() {
                    self.send_to_player(
                        player_id,
                        ServerMessage::StateSync {
                            tick: 0,
                            players: vec![],
                            npcs: npc_updates,
                            instance_id: instance.id.clone(),
                        },
                    )
                    .await;
                }

                tracing::info!(
                    "Admin {} entered boss arena (instance: {}) at ({}, {})",
                    player_id,
                    instance.id,
                    spawn.x,
                    spawn.y
                );

                self.send_system_message(player_id, "Entering the Desert Wurm arena...")
                    .await;
            }
            "/kick" => {
                if parts.len() < 2 {
                    self.send_system_message(player_id, "Usage: /kick <player_name>")
                        .await;
                    return;
                }
                let target_name = parts[1];

                let target_id = {
                    let players = self.players.read().await;
                    players
                        .values()
                        .find(|p| p.name.eq_ignore_ascii_case(target_name))
                        .map(|p| p.id.clone())
                };

                match target_id {
                    Some(tid) if tid == player_id => {
                        self.send_system_message(player_id, "You cannot kick yourself.")
                            .await;
                    }
                    Some(tid) => {
                        let admin_name = {
                            let players = self.players.read().await;
                            players
                                .get(player_id)
                                .map(|p| p.name.clone())
                                .unwrap_or_default()
                        };
                        tracing::info!("Admin {} kicked player {}", admin_name, target_name);
                        self.send_system_message(&tid, "You have been kicked by an admin.")
                            .await;
                        // Drop the player's sender — closes their mpsc channel,
                        // which causes the send task to exit and triggers normal disconnect cleanup.
                        self.unregister_player_sender(&tid).await;
                        self.send_system_message(player_id, &format!("Kicked {}", target_name))
                            .await;
                        self.broadcast(ServerMessage::ChatMessage {
                            sender_id: "system".to_string(),
                            sender_name: "[System]".to_string(),
                            text: format!("{} has been kicked.", target_name),
                            timestamp: chat_timestamp_ms(),
                            channel: "global".to_string(),
                        })
                        .await;
                    }
                    None => {
                        self.send_system_message(player_id, "Player not found or not online.")
                            .await;
                    }
                }
            }
            "/ban" => {
                if parts.len() < 3 {
                    self.send_system_message(
                        player_id,
                        "Usage: /ban <player_name> <hours> [reason]",
                    )
                    .await;
                    return;
                }
                let target_name = parts[1];
                let hours: f64 = match parts[2].parse() {
                    Ok(h) if h > 0.0 && h <= 87600.0 => h,
                    _ => {
                        self.send_system_message(
                            player_id,
                            "Hours must be between 0 and 87600 (10 years).",
                        )
                        .await;
                        return;
                    }
                };
                let reason = if parts.len() > 3 {
                    Some(parts[3..].join(" "))
                } else {
                    None
                };

                let admin_name = {
                    let players = self.players.read().await;
                    players
                        .get(player_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_default()
                };

                // Check if player is online — get account_id and IP
                let online_info = {
                    let players = self.players.read().await;
                    players
                        .values()
                        .find(|p| p.name.eq_ignore_ascii_case(target_name))
                        .map(|p| (p.id.clone(), p.account_id, p.ip_address.clone()))
                };

                let db = match &self.db {
                    Some(db) => db.clone(),
                    None => {
                        self.send_system_message(player_id, "Database not available.")
                            .await;
                        return;
                    }
                };

                if let Some((tid, account_id, ip)) = online_info {
                    if tid == player_id {
                        self.send_system_message(player_id, "You cannot ban yourself.")
                            .await;
                        return;
                    }
                    // Online player — ban and kick
                    if let Err(e) = db
                        .insert_ban(account_id, None, &admin_name, reason.as_deref(), hours)
                        .await
                    {
                        self.send_system_message(player_id, &format!("Failed to ban: {}", e))
                            .await;
                        return;
                    }

                    let ban_msg = match &reason {
                        Some(r) => {
                            format!("You have been banned for {} hours. Reason: {}", hours, r)
                        }
                        None => format!("You have been banned for {} hours.", hours),
                    };
                    self.send_system_message(&tid, &ban_msg).await;
                    self.unregister_player_sender(&tid).await;

                    tracing::info!(
                        "Admin {} banned {} for {} hours (reason: {:?})",
                        admin_name,
                        target_name,
                        hours,
                        reason
                    );
                    self.send_system_message(
                        player_id,
                        &format!("Banned {} for {} hours", target_name, hours),
                    )
                    .await;
                    self.broadcast(ServerMessage::ChatMessage {
                        sender_id: "system".to_string(),
                        sender_name: "[System]".to_string(),
                        text: format!("{} has been banned.", target_name),
                        timestamp: chat_timestamp_ms(),
                        channel: "global".to_string(),
                    })
                    .await;
                } else {
                    // Offline player — look up from DB
                    match db.get_account_id_by_character_name(target_name).await {
                        Some(account_id) => {
                            if let Err(e) = db
                                .insert_ban(account_id, None, &admin_name, reason.as_deref(), hours)
                                .await
                            {
                                self.send_system_message(
                                    player_id,
                                    &format!("Failed to ban: {}", e),
                                )
                                .await;
                                return;
                            }
                            tracing::info!(
                                "Admin {} banned offline player {} for {} hours (reason: {:?})",
                                admin_name,
                                target_name,
                                hours,
                                reason
                            );
                            self.send_system_message(
                                player_id,
                                &format!("Banned {} (offline) for {} hours", target_name, hours),
                            )
                            .await;
                        }
                        None => {
                            self.send_system_message(player_id, "Character not found.")
                                .await;
                        }
                    }
                }
            }
            "/title" => {
                self.handle_title_command(player_id, &parts[1..]).await;
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
