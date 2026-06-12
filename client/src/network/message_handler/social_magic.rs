use super::*;

pub(super) fn handle(msg_type: &str, data: Option<&rmpv::Value>, state: &mut GameState) -> bool {
    match msg_type {
        "friendsList" => {
            if let Some(value) = data {
                if let Some(friends_array) = extract_array(value, "friends") {
                    state.social_state.friends.clear();
                    for friend_value in friends_array {
                        let id = extract_i32(friend_value, "id").unwrap_or(0) as i64;
                        let name = extract_string(friend_value, "name").unwrap_or_default();
                        let online = extract_bool(friend_value, "online").unwrap_or(false);
                        state.social_state.friends.push(crate::game::FriendInfo {
                            id,
                            name,
                            online,
                        });
                    }
                    // Sort: online friends first, then alphabetical
                    state
                        .social_state
                        .friends
                        .sort_by(|a, b| match (a.online, b.online) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                        });
                    log::info!(
                        "Received friends list: {} friends",
                        state.social_state.friends.len()
                    );
                }
            }
        }
        "pendingFriendRequests" => {
            if let Some(value) = data {
                if let Some(requests_array) = extract_array(value, "requests") {
                    state.social_state.pending_requests.clear();
                    for req_value in requests_array {
                        let from_id = extract_i32(req_value, "from_id").unwrap_or(0) as i64;
                        let from_name = extract_string(req_value, "from_name").unwrap_or_default();
                        state
                            .social_state
                            .pending_requests
                            .push(crate::game::PendingRequestInfo { from_id, from_name });
                    }
                    state.social_state.pending_request_count =
                        state.social_state.pending_requests.len();
                    log::info!(
                        "Received {} pending friend requests",
                        state.social_state.pending_request_count
                    );
                }
            }
        }
        "onlinePlayersList" => {
            if let Some(value) = data {
                if let Some(players_array) = extract_array(value, "players") {
                    state.social_state.online_players.clear();
                    for player_value in players_array {
                        let id = extract_i32(player_value, "id").unwrap_or(0) as i64;
                        let name = extract_string(player_value, "name").unwrap_or_default();
                        let is_friend = extract_bool(player_value, "is_friend").unwrap_or(false);
                        state
                            .social_state
                            .online_players
                            .push(crate::game::OnlinePlayerInfo {
                                id,
                                name,
                                is_friend,
                            });
                    }
                    // Sort alphabetically
                    state
                        .social_state
                        .online_players
                        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                    log::info!(
                        "Received online players list: {} players",
                        state.social_state.online_players.len()
                    );
                }
            }
        }
        "friendRequestReceived" => {
            if let Some(value) = data {
                let from_id = extract_i32(value, "from_id").unwrap_or(0) as i64;
                let from_name = extract_string(value, "from_name").unwrap_or_default();

                // Add to pending requests if not already there
                if !state
                    .social_state
                    .pending_requests
                    .iter()
                    .any(|r| r.from_id == from_id)
                {
                    state
                        .social_state
                        .pending_requests
                        .push(crate::game::PendingRequestInfo {
                            from_id,
                            from_name: from_name.clone(),
                        });
                    state.social_state.pending_request_count =
                        state.social_state.pending_requests.len();
                }
                log::info!("Received friend request from {}", from_name);
            }
        }
        "friendRequestAccepted" => {
            if let Some(value) = data {
                let friend_id = extract_i32(value, "friend_id").unwrap_or(0) as i64;
                let friend_name = extract_string(value, "friend_name").unwrap_or_default();

                // Add to friends list if not already there
                if !state.social_state.friends.iter().any(|f| f.id == friend_id) {
                    state.social_state.friends.push(crate::game::FriendInfo {
                        id: friend_id,
                        name: friend_name.clone(),
                        online: true, // They just accepted, so they're online
                    });
                    // Re-sort friends list
                    state
                        .social_state
                        .friends
                        .sort_by(|a, b| match (a.online, b.online) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                        });
                }
                log::info!("Friend request accepted by {}", friend_name);
            }
        }
        "friendRequestDeclined" => {
            if let Some(value) = data {
                let by_id = extract_i32(value, "by_id").unwrap_or(0) as i64;
                log::info!("Friend request declined by character {}", by_id);
            }
        }
        "friendRemoved" => {
            if let Some(value) = data {
                let friend_id = extract_i32(value, "friend_id").unwrap_or(0) as i64;
                state.social_state.friends.retain(|f| f.id != friend_id);
                log::info!("Friend removed: {}", friend_id);
            }
        }
        "friendStatusChanged" => {
            if let Some(value) = data {
                let friend_id = extract_i32(value, "friend_id").unwrap_or(0) as i64;
                let online = extract_bool(value, "online").unwrap_or(false);

                // Update friend's online status
                if let Some(friend) = state
                    .social_state
                    .friends
                    .iter_mut()
                    .find(|f| f.id == friend_id)
                {
                    friend.online = online;
                    log::info!(
                        "Friend {} is now {}",
                        friend.name,
                        if online { "online" } else { "offline" }
                    );
                }

                // Re-sort friends list
                state
                    .social_state
                    .friends
                    .sort_by(|a, b| match (a.online, b.online) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    });
            }
        }
        "friendActionResult" => {
            if let Some(value) = data {
                let action = extract_string(value, "action").unwrap_or_default();
                let success = extract_bool(value, "success").unwrap_or(false);
                let error = extract_string(value, "error");

                if success {
                    log::info!("Friend action '{}' succeeded", action);
                } else if let Some(err) = error {
                    log::warn!("Friend action '{}' failed: {}", action, err);
                    // Add error to chat as system message
                    state.push_chat_message(ChatMessage::system(err));
                }
            }
        }

        // =====================================================================
        // Prayer System Messages
        // =====================================================================
        "prayerStateUpdate" => {
            if let Some(value) = data {
                let points = extract_i32(value, "points").unwrap_or(0);
                let max_points = extract_i32(value, "max_points").unwrap_or(1);

                // Parse active prayers array
                let mut active_prayers = Vec::new();
                if let Some(prayers_arr) = extract_array(value, "active_prayers") {
                    for prayer_value in prayers_arr {
                        if let Some(prayer_id) = prayer_value.as_str() {
                            active_prayers.push(prayer_id.to_string());
                        }
                    }
                }

                log::info!(
                    "Prayer state update: {}/{} points, {} active prayers",
                    points,
                    max_points,
                    active_prayers.len()
                );

                state.prayer_points = points;
                state.max_prayer_points = max_points;
                state.active_prayers = active_prayers;
            }
        }
        "spellEffect" => {
            if let Some(value) = data {
                let caster_id = extract_string(value, "caster_id").unwrap_or_default();
                let target_id = extract_string(value, "target_id");
                let spell_id = extract_string(value, "spell_id").unwrap_or_default();
                let target_x = extract_i32(value, "target_x").unwrap_or(0);
                let target_y = extract_i32(value, "target_y").unwrap_or(0);

                log::info!(
                    "Spell effect: {} cast {} at ({}, {}), target: {:?}",
                    caster_id,
                    spell_id,
                    target_x,
                    target_y,
                    target_id
                );

                // Trigger casting animation on caster
                if let Some(player) = state.players.get_mut(&caster_id) {
                    player.play_cast();
                }

                // Blast spell: spawn a projectile instead of on-target effect
                if spell_id.ends_with("_blast") {
                    let source_pos = state
                        .players
                        .get(&caster_id)
                        .map(|player| (player.x.round(), player.y.round(), player.z));

                    if let Some((src_x, src_y, src_z)) = source_pos {
                        let end_z = state.chunk_manager.get_height(target_x, target_y) as f32;
                        let dx = target_x as f32 - src_x;
                        let dy = target_y as f32 - src_y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        let duration = (dist * 0.12).clamp(0.25, 0.5) as f64; // ~0.12s per tile
                        state.projectiles.push(crate::game::Projectile {
                            sprite: spell_id.clone(),
                            start_x: src_x,
                            start_y: src_y,
                            start_z: src_z,
                            end_x: target_x as f32,
                            end_y: target_y as f32,
                            end_z,
                            start_time: macroquad::time::get_time(),
                            duration,
                        });
                    }
                } else {
                    // Store for rendering - existing on-target effects
                    state.spell_effects.push(SpellEffect {
                        caster_id,
                        target_id,
                        spell_id,
                        target_x,
                        target_y,
                        time: macroquad::time::get_time(),
                    });
                }
            }
        }
        "spellResult" => {
            if let Some(value) = data {
                let success = extract_bool(value, "success").unwrap_or(false);
                let reason = extract_string(value, "reason");
                let spell_id = extract_string(value, "spell_id");

                if !success {
                    if let Some(reason) = &reason {
                        log::info!("Spell cast failed: {}", reason);
                        // Add system chat message for failure feedback
                        state.push_system_chat(format!("Spell failed: {}", reason));
                    }
                    // Clear client-side cooldown so the spell can be retried immediately
                    if let Some(ref id) = spell_id {
                        state.spell_cooldowns.remove(id);
                    }
                }
            }
        }
        "scrollSpellDefinitions" => {
            if let Some(value) = data {
                if let Some(spells_arr) = extract_array(value, "spells") {
                    state.scroll_spell_definitions.clear();
                    for spell_val in spells_arr {
                        let id = extract_string(spell_val, "id").unwrap_or_default();
                        let name = extract_string(spell_val, "name").unwrap_or_default();
                        let spell_type_str =
                            extract_string(spell_val, "spell_type").unwrap_or_default();
                        let spell_type = match spell_type_str.as_str() {
                            "damage" => crate::game::spell::SpellType::Damage,
                            "heal" => crate::game::spell::SpellType::Heal,
                            "teleport" => crate::game::spell::SpellType::Teleport,
                            _ => crate::game::spell::SpellType::Damage,
                        };
                        state
                            .scroll_spell_definitions
                            .push(crate::game::spell::ScrollSpellDef {
                                id,
                                name,
                                spell_type,
                                mana_cost: extract_i32(spell_val, "mana_cost").unwrap_or(0),
                                cooldown_ms: extract_i32(spell_val, "cooldown_ms").unwrap_or(0)
                                    as u64,
                                base_power: extract_i32(spell_val, "base_power").unwrap_or(0),
                                effect_sprite: extract_string(spell_val, "effect_sprite")
                                    .unwrap_or_default(),
                                pushback_distance: extract_i32(spell_val, "pushback_distance")
                                    .unwrap_or(0),
                                wall_slam_damage_per_tile: extract_i32(
                                    spell_val,
                                    "wall_slam_damage_per_tile",
                                )
                                .unwrap_or(0),
                                description: extract_string(spell_val, "description")
                                    .unwrap_or_default(),
                            });
                    }
                    log::info!(
                        "Received {} scroll spell definitions",
                        state.scroll_spell_definitions.len()
                    );
                }
            }
        }
        "unlockedSpellsSync" => {
            if let Some(value) = data {
                if let Some(ids_arr) = extract_array(value, "spell_ids") {
                    state.unlocked_spells.clear();
                    for id_val in ids_arr {
                        if let Some(id) = id_val.as_str() {
                            state.unlocked_spells.insert(id.to_string());
                        }
                    }
                    log::info!("Synced {} unlocked spells", state.unlocked_spells.len());
                }
            }
        }
        "spellUnlocked" => {
            if let Some(value) = data {
                if let Some(spell_id) = extract_string(value, "spell_id") {
                    state.unlocked_spells.insert(spell_id.clone());
                    log::info!("Spell unlocked: {}", spell_id);
                }
            }
        }
        "pushback" => {
            if let Some(value) = data {
                let target_id = extract_string(value, "target_id").unwrap_or_default();
                let to_x = extract_i32(value, "to_x").unwrap_or(0);
                let to_y = extract_i32(value, "to_y").unwrap_or(0);

                // Update entity position to final pushback position with smooth slide
                if let Some(player) = state.players.get_mut(&target_id) {
                    player.server_x = to_x as f32;
                    player.server_y = to_y as f32;
                    player.target_x = to_x as f32;
                    player.target_y = to_y as f32;
                    player.is_dashing = true; // Reuse dash slide for fast pushback interpolation
                }
            }
        }
        "pong" => {
            // Handle ping response - calculate and display latency
            if let Some(sent_at) = state.ping_sent_at.take() {
                let now = macroquad::time::get_time();
                let latency_ms = (now - sent_at) * 1000.0;
                state.ping_stats.record(latency_ms);
                // Only show in chat if it was a manual /ping command
                if state.manual_ping {
                    state.manual_ping = false;
                    state.push_system_chat(format!("Ping: {}ms", latency_ms.round() as i32));
                }
            }
        }
        _ => return false,
    }
    true
}
