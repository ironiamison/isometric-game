use super::*;

pub(super) fn handle(msg_type: &str, data: Option<&rmpv::Value>, state: &mut GameState) -> bool {
    match msg_type {
        "welcome" => {
            if let Some(value) = data {
                handle_welcome(value, state);
            }
        }
        "playerJoined" => {
            if let Some(value) = data {
                handle_player_joined(value, state);
            }
        }
        "playerLeft" => {
            if let Some(value) = data {
                handle_player_left(value, state);
            }
        }
        "stateSync" => {
            if let Some(value) = data {
                handle_state_sync(value, state);
            }
        }
        "chatMessage" => {
            if let Some(value) = data {
                handle_chat_message(value, state);
            }
        }
        "npcSpeech" => {
            if let Some(value) = data {
                let npc_id = extract_string(value, "npcId").unwrap_or_default();
                let message = extract_string(value, "message").unwrap_or_default();

                if let Some(npc) = state.npcs.get_mut(&npc_id) {
                    npc.speech_bubble = Some((message, macroquad::time::get_time()));
                }
            }
        }
        "targetChanged" => {
            if let Some(value) = data {
                handle_target_changed(value, state);
            }
        }
        "playerAttack" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let attack_type =
                    extract_string(value, "attack_type").unwrap_or_else(|| "melee".to_string());
                let direction = extract_u8(value, "direction");

                let is_local = state.local_player_id.as_ref() == Some(&player_id);

                // Check if already in attack animation BEFORE calling play_attack.
                // Manual attacks play animation+sound locally; auto-attacks rely on
                // this server event. set_state is idempotent (no-op if already in
                // same state), so always triggering the animation is safe.
                let already_attacking = is_local
                    && state.players.get(&player_id).map_or(false, |p| {
                        matches!(
                            p.animation.state,
                            crate::render::animation::AnimationState::Attacking
                                | crate::render::animation::AnimationState::ShootingBow
                                | crate::render::animation::AnimationState::Casting
                        )
                    });

                if let Some(player) = state.players.get_mut(&player_id) {
                    // Update facing direction from server (fixes visual mismatch
                    // during auto-action ranged/spell attacks)
                    if let Some(dir) = direction {
                        let new_dir = crate::game::Direction::from_u8(dir).to_cardinal();
                        player.direction = new_dir;
                        player.animation.direction = new_dir;
                    }

                    match attack_type.as_str() {
                        "ranged" => player.play_shoot_bow(),
                        "spell" => player.play_cast(),
                        _ => player.play_attack(),
                    }
                }

                // Play attack sound for server-driven attacks (auto-attacks).
                // Manual attacks already played the sound locally, so skip if the
                // player was already mid-animation when this event arrived.
                if is_local && !already_attacking {
                    if let Some(player) = state.players.get(&player_id) {
                        let sound_type = if attack_type == "ranged" {
                            crate::game::state::AttackSoundType::Ranged
                        } else if player.equipped_weapon.is_some() {
                            crate::game::state::AttackSoundType::Melee
                        } else {
                            crate::game::state::AttackSoundType::Unarmed
                        };
                        state.pending_attack_sounds.push(sound_type);
                    }
                }
            }
        }
        "damageEvent" => {
            if let Some(value) = data {
                let source_id = extract_string(value, "source_id");
                let target_id = extract_string(value, "target_id").unwrap_or_default();
                let damage = extract_i32(value, "damage").unwrap_or(0);
                let target_hp = extract_i32(value, "target_hp").unwrap_or(0);
                let target_x = extract_f32(value, "target_x").unwrap_or(0.0);
                let target_y = extract_f32(value, "target_y").unwrap_or(0.0);
                let projectile = extract_string(value, "projectile");

                log::debug!(
                    "Damage event: {} took {} damage from {:?} (HP: {})",
                    target_id,
                    damage,
                    source_id,
                    target_hp
                );

                // Trigger attack animation for NPCs (players use playerAttack event)
                if let Some(ref src_id) = source_id {
                    if let Some(npc) = state.npcs.get_mut(src_id) {
                        npc.trigger_attack_animation();
                    }
                }

                // Update last damage time (could be player or NPC)
                // NOTE: We intentionally do NOT update hp here. The StateSync snapshot
                // is taken BEFORE combat in the tick loop, so it contains stale pre-damage HP.
                // If we set hp = target_hp here, the subsequent stale StateSync would see
                // hp_from_sync > entity.hp and falsely detect regen (showing green +X numbers).
                // Letting StateSync be the sole authority for HP state avoids this race.
                let current_time = macroquad::time::get_time();
                if let Some(player) = state.players.get_mut(&target_id) {
                    player.last_damage_time = current_time;
                } else if let Some(npc) = state.npcs.get_mut(&target_id) {
                    npc.last_damage_time = current_time;
                }

                // Play hit sound when our player gets attacked (including misses)
                if state.local_player_id.as_deref() == Some(&target_id) {
                    state.pending_sfx.push("unarmed".to_string());
                }

                // Create floating damage number with target_id for height lookup at render time
                state.damage_events.push(DamageEvent {
                    x: target_x,
                    y: target_y,
                    damage,
                    time: macroquad::time::get_time(),
                    target_id,
                    source_id: source_id.clone(),
                    projectile: projectile.clone(),
                });

                // Spawn projectile for ranged attacks (blast handled by spellEffect)
                if let Some(ref projectile_type) = projectile {
                    if !projectile_type.ends_with("_blast") {
                        if let Some(ref source_id) = source_id {
                            // Get source position + Z (check players then NPCs)
                            let source_pos = if let Some(player) = state.players.get(source_id) {
                                Some((player.x.round(), player.y.round(), player.z))
                            } else if let Some(npc) = state.npcs.get(source_id) {
                                Some((npc.x.round(), npc.y.round(), npc.z))
                            } else {
                                None
                            };

                            if let Some((src_x, src_y, src_z)) = source_pos {
                                // Target tile center (rounded for straight isometric lines)
                                let end_x = target_x.round();
                                let end_y = target_y.round();
                                // Look up target Z from terrain height
                                let end_z =
                                    state.chunk_manager.get_height(end_x as i32, end_y as i32)
                                        as f32;
                                let dx = end_x - src_x;
                                let dy = end_y - src_y;
                                let dist = (dx * dx + dy * dy).sqrt();
                                let duration = (dist as f64 * 0.12).clamp(0.25, 0.5); // ~0.12s per tile

                                state.projectiles.push(crate::game::Projectile {
                                    sprite: projectile_type.clone(),
                                    start_x: src_x,
                                    start_y: src_y,
                                    start_z: src_z,
                                    end_x,
                                    end_y,
                                    end_z,
                                    start_time: current_time,
                                    duration,
                                });
                            }
                        }
                    }
                }
            }
        }
        "npcDied" => {
            if let Some(value) = data {
                let npc_id = extract_string(value, "npc_id").unwrap_or_default();
                log::debug!("NPC died: {}", npc_id);

                if let Some(npc) = state.npcs.get_mut(&npc_id) {
                    npc.start_death();
                }

                // Clear selection if we had this NPC targeted
                if state.selected_entity_id.as_ref() == Some(&npc_id) {
                    state.selected_entity_id = None;
                }

                // Close shop if this NPC was the merchant
                if let Some(shop_npc_id) = &state.ui_state.shop_npc_id {
                    if shop_npc_id == &npc_id {
                        state.ui_state.crafting_open = false;
                        state.ui_state.shop_data = None;
                    }
                }
            }
        }
        "npcRespawned" => {
            if let Some(value) = data {
                let npc_id = extract_string(value, "id").unwrap_or_default();
                // Server sends i32 grid positions
                let x = extract_i32(value, "x").unwrap_or(0) as f32;
                let y = extract_i32(value, "y").unwrap_or(0) as f32;
                let hp = extract_i32(value, "hp").unwrap_or(50);
                log::debug!("NPC respawned: {} at ({}, {})", npc_id, x, y);

                if let Some(npc) = state.npcs.get_mut(&npc_id) {
                    npc.state = NpcState::Idle;
                    npc.hp = hp;
                    npc.max_hp = hp;
                    npc.x = x;
                    npc.y = y;
                    npc.target_x = x;
                    npc.target_y = y;
                }
            }
        }
        "playerDied" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "id").unwrap_or_default();
                let killer_id = extract_string(value, "killer_id").unwrap_or_default();
                log::info!("Player {} was killed by {}", player_id, killer_id);

                if let Some(player) = state.players.get_mut(&player_id) {
                    player.die();
                }

                // Local player death: clear combat/movement state
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.pending_sfx.push("death".to_string());
                    state.auto_action_state = None;
                    state.auto_path = None;
                    state.follow_target = None;
                    state.follow_arrived_target_pos = None;
                    state.follow_target_move_time = 0.0;
                }

                // Clear selection if we had this player targeted
                if state.selected_entity_id.as_ref() == Some(&player_id) {
                    state.selected_entity_id = None;
                }
            }
        }
        "playerRespawned" => {
            if let Some(value) = data {
                handle_player_respawned(value, state);
            }
        }
        "attackResult" => {
            if let Some(value) = data {
                let success = value
                    .as_map()
                    .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("success")))
                    .and_then(|(_, v)| v.as_bool())
                    .unwrap_or(false);
                let reason = extract_string(value, "reason");

                if !success {
                    if let Some(ref reason) = reason {
                        log::debug!("Attack failed: {}", reason);
                        if reason == "no_arrows" {
                            state.pending_sfx.push("error".to_string());
                            state.push_system_chat("You have no arrows!".to_string());
                        }
                    }
                }
            }
        }
        "skillXp" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let skill_name = extract_string(value, "skill").unwrap_or_default();
                let xp_gained = extract_i32(value, "xp_gained").unwrap_or(0) as i64;
                let total_xp = extract_i32(value, "total_xp").unwrap_or(0) as i64;
                let level = extract_i32(value, "level").unwrap_or(1);

                log::debug!(
                    "Player {} gained {} {} XP (total: {}, level: {})",
                    player_id,
                    xp_gained,
                    skill_name,
                    total_xp,
                    level
                );

                if let Some(player) = state.players.get_mut(&player_id) {
                    // Update the specific skill
                    if let Some(skill_type) = SkillType::from_str(&skill_name) {
                        let skill = player.skills.get_mut(skill_type);
                        skill.xp = total_xp;
                        skill.level = level;

                        // Update max_hp if hitpoints changed
                        if skill_type == SkillType::Hitpoints {
                            player.max_hp = level;
                        }
                    }

                    // Create floating XP event and system message for local player
                    if state.local_player_id.as_ref() == Some(&player_id) {
                        // Add system chat message (system-only, no Local mirror — too spammy)
                        state
                            .ui_state
                            .chat_messages
                            .push_system_only(ChatMessage::system(format!(
                                "+{} {} XP",
                                xp_gained, skill_name
                            )));
                        state.ui_state.chat_revision = state.ui_state.chat_revision.wrapping_add(1);

                        state.skill_xp_events.push(SkillXpEvent {
                            x: player.x,
                            y: player.y,
                            skill: skill_name.clone(),
                            xp_gained,
                            time: macroquad::time::get_time(),
                        });

                        // Update XP globes and drop feed
                        if let Some(skill_type) = SkillType::from_str(&skill_name) {
                            let xp_for_next = crate::game::skills::total_xp_for_level(level + 1);
                            state
                                .xp_globes
                                .on_xp_gain(skill_type, total_xp, xp_for_next, level);
                            state.xp_drop_feed.push(skill_type, xp_gained);
                        }
                    }
                }
            }
        }
        "skillsSync" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                log::info!("skillsSync received for player_id: {}, local_player_id: {:?}, players in state: {:?}",
                    player_id, state.local_player_id, state.players.keys().collect::<Vec<_>>());

                // Only update skills for the local player
                if state.local_player_id.as_ref() == Some(&player_id) {
                    if let Some(player) = state.players.get_mut(&player_id) {
                        // Update all skills
                        if let Some(level) = extract_i32(value, "hitpoints_level") {
                            player.skills.hitpoints.level = level;
                            player.max_hp = level;
                        }
                        if let Some(xp) = extract_i32(value, "hitpoints_xp") {
                            player.skills.hitpoints.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "attack_level") {
                            player.skills.attack.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "attack_xp") {
                            player.skills.attack.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "strength_level") {
                            player.skills.strength.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "strength_xp") {
                            player.skills.strength.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "defence_level") {
                            player.skills.defence.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "defence_xp") {
                            player.skills.defence.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "ranged_level") {
                            player.skills.ranged.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "ranged_xp") {
                            player.skills.ranged.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "fishing_level") {
                            player.skills.fishing.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "fishing_xp") {
                            player.skills.fishing.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "farming_level") {
                            player.skills.farming.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "farming_xp") {
                            player.skills.farming.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "smithing_level") {
                            player.skills.smithing.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "smithing_xp") {
                            player.skills.smithing.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "prayer_level") {
                            player.skills.prayer.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "prayer_xp") {
                            player.skills.prayer.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "magic_level") {
                            player.skills.magic.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "magic_xp") {
                            player.skills.magic.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "woodcutting_level") {
                            player.skills.woodcutting.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "woodcutting_xp") {
                            player.skills.woodcutting.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "alchemy_level") {
                            player.skills.alchemy.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "alchemy_xp") {
                            player.skills.alchemy.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "mining_level") {
                            player.skills.mining.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "mining_xp") {
                            player.skills.mining.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "slayer_level") {
                            player.skills.slayer.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "slayer_xp") {
                            player.skills.slayer.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "survivalist_level") {
                            player.skills.survivalist.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "survivalist_xp") {
                            player.skills.survivalist.xp = xp as i64;
                        }

                        log::info!("Skills synced for player {}: HP {}, Atk {}, Str {}, Def {}, Ranged {}, Fishing {}, Farming {}, Smithing {}, Prayer {}, Magic {}, Woodcutting {}, Alchemy {}, Mining {}, Slayer {}, Survivalist {}",
                            player_id,
                            player.skills.hitpoints.level,
                            player.skills.attack.level,
                            player.skills.strength.level,
                            player.skills.defence.level,
                            player.skills.ranged.level,
                            player.skills.fishing.level,
                            player.skills.farming.level,
                            player.skills.smithing.level,
                            player.skills.prayer.level,
                            player.skills.magic.level,
                            player.skills.woodcutting.level,
                            player.skills.alchemy.level,
                            player.skills.mining.level,
                            player.skills.slayer.level,
                            player.skills.survivalist.level
                        );
                    } else {
                        log::warn!(
                            "skillsSync: player {} not found in state.players",
                            player_id
                        );
                    }
                } else {
                    log::warn!(
                        "skillsSync: player_id {} doesn't match local_player_id {:?}",
                        player_id,
                        state.local_player_id
                    );
                }
            }
        }
        "skillLevelUp" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let skill_name = extract_string(value, "skill").unwrap_or_default();
                let new_level = extract_i32(value, "new_level").unwrap_or(1);

                log::info!(
                    "Player {} leveled up {} to {}!",
                    player_id,
                    skill_name,
                    new_level
                );

                // Get player position for floating text
                if let Some(player) = state.players.get_mut(&player_id) {
                    // Update the specific skill level
                    if let Some(skill_type) = SkillType::from_str(&skill_name) {
                        let skill = player.skills.get_mut(skill_type);
                        skill.level = new_level;

                        // Update max_hp and current HP if hitpoints leveled up
                        if skill_type == SkillType::Hitpoints {
                            let old_max = player.max_hp;
                            player.max_hp = new_level;
                            // Heal the difference (new HP from the level)
                            player.hp += new_level - old_max;
                        }
                    }

                    // Create floating level up event and system message for local player
                    if state.local_player_id.as_ref() == Some(&player_id) {
                        state
                            .ui_state
                            .chat_messages
                            .push(ChatMessage::system(format!(
                                "{} leveled up to {}!",
                                skill_name, new_level
                            )));
                        state.ui_state.chat_revision = state.ui_state.chat_revision.wrapping_add(1);
                        state.pending_sfx.push("level_up".to_string());
                    }

                    let now = macroquad::time::get_time();
                    let px = player.x;
                    let py = player.y;

                    state.level_up_events.push(LevelUpEvent {
                        x: px,
                        y: py,
                        skill: skill_name,
                        new_level,
                        time: now,
                    });
                }
            }
        }
        _ => return false,
    }
    true
}
