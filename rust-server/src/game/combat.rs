use super::*;

impl GameRoom {
    pub async fn handle_attack(
        &self,
        player_id: &str,
        direction_override: Option<Direction>,
        forced_target_id: Option<&str>,
    ) {
        // Determine attacker's instance context (None = overworld)
        let attacker_instance = self.player_instances.read().await.get(player_id).cloned();

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Atomically check the attack cooldown and claim the swing slot in a single
        // lock acquisition. This is the one choke point that prevents double-swings:
        // a manual attack and an auto-retaliate action can both reach handle_attack
        // within the same cooldown window, and without an atomic check-and-set both
        // would read the old last_attack_time (across the many .await points below)
        // and each fire a swing. Claiming here means whichever arrives first wins and
        // the other is rejected.
        {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) => p,
                None => return,
            };
            if player.is_dead {
                return;
            }
            let weapon_type = player
                .equipped_weapon
                .as_ref()
                .and_then(|wid| self.item_registry.get(wid))
                .and_then(|def| def.equipment.as_ref())
                .map(|e| e.weapon_type)
                .unwrap_or(WeaponType::Melee);
            let cooldown = if weapon_type == WeaponType::Ranged {
                RANGED_ATTACK_COOLDOWN_MS
            } else {
                ATTACK_COOLDOWN_MS
            };
            if current_time.saturating_sub(player.last_attack_time) < cooldown {
                return;
            }
            player.last_attack_time = current_time;
            // Stop movement when attacking (player must stand still to attack). Done
            // here so it happens exactly when the swing is claimed.
            player.reject_pending_move();
            // Manual attacks (not from auto-action) reset the auto-retaliate idle timer
            if forced_target_id.is_none() {
                player.last_activity_time = current_time;
            }
        }

        // Get attacker info including combat stats
        // When direction_override is provided (auto-action), atomically set and read the
        // direction in the same lock to prevent race conditions with client Face commands.
        let (
            attacker_name,
            attacker_x,
            attacker_y,
            attacker_dir,
            attack_level,
            strength_level,
            attack_bonus,
            strength_bonus,
            equipped_head,
            equipped_back,
            combat_style,
            equip_ranged_str_bonus,
            equipped_items,
        ) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) => p,
                None => {
                    tracing::warn!("Attack failed: player {} not found", player_id);
                    return;
                }
            };

            // Dead players can't attack
            if player.is_dead {
                return;
            }

            // Apply direction override atomically before reading direction
            if let Some(dir) = direction_override {
                player.direction = dir;
            }

            let base_atk_bonus = player.attack_bonus(&self.item_registry);
            let base_str_bonus = player.strength_bonus(&self.item_registry);
            let ranged_str = player.ranged_strength_bonus(&self.item_registry);

            // Collect equipped item IDs for type bonus lookups
            let equipped: Vec<Option<String>> =
                player.all_equipped().iter().map(|s| (*s).clone()).collect();

            // Apply prayer bonuses to attack and strength
            let active_ids: Vec<String> = player.active_prayers.iter().cloned().collect();
            let prayer_effects = self.prayer_registry.calculate_effects(&active_ids);
            let atk_bonus = prayer_effects.apply_attack_bonus(base_atk_bonus);
            let str_bonus = prayer_effects.apply_strength_bonus(base_str_bonus);

            (
                player.name.clone(),
                player.x,
                player.y,
                player.direction,
                player.skills.attack.level,
                player.skills.strength.level,
                atk_bonus,
                str_bonus,
                player.equipped_head.clone(),
                player.equipped_back.clone(),
                player.combat_style,
                ranged_str,
                equipped,
            )
        };

        // Helper: compute type bonus strength % from equipped items against NPC tags
        let calc_type_bonus_str = |npc_tags: &[String]| -> f32 {
            let mut total = 0.0f32;
            for slot in &equipped_items {
                if let Some(item_id) = slot
                    && let Some(def) = self.item_registry.get(item_id)
                    && let Some(equip) = &def.equipment
                {
                    for tb in &equip.type_bonuses {
                        if npc_tags.contains(&tb.tag) {
                            total += tb.strength_percent;
                        }
                    }
                }
            }
            total
        };

        // Get weapon range and type (needed before cooldown check)
        let (mut weapon_range, weapon_type) = {
            let players = self.players.read().await;
            if let Some(player) = players.get(player_id) {
                if let Some(ref weapon_id) = player.equipped_weapon {
                    if let Some(item_def) = self.item_registry.get(weapon_id) {
                        if let Some(ref equip) = item_def.equipment {
                            (equip.range, equip.weapon_type)
                        } else {
                            (1, WeaponType::Melee)
                        }
                    } else {
                        (1, WeaponType::Melee)
                    }
                } else {
                    (1, WeaponType::Melee) // Unarmed = melee range 1
                }
            } else {
                return;
            }
        };

        // Cooldown was already checked and claimed atomically at the top of this
        // function (see the claim block), so we can proceed straight to the swing.

        // For ranged weapons, override attack/strength with ranged level and apply style bonuses
        let (attack_level, strength_level) = if weapon_type == WeaponType::Ranged {
            let ranged_level = {
                let players = self.players.read().await;
                players
                    .get(player_id)
                    .map(|p| p.skills.ranged.level)
                    .unwrap_or(1)
            };
            // Accurate style: +3 to effective ranged level for accuracy
            let effective_ranged = if combat_style == CombatStyle::Accurate {
                ranged_level + 3
            } else {
                ranged_level
            };
            // Longrange style: +2 to weapon range
            if combat_style == CombatStyle::Longrange {
                weapon_range += 2;
            }
            // Ranged uses ranged_level for both accuracy and max hit
            (effective_ranged, ranged_level)
        } else {
            (attack_level, strength_level)
        };

        // For ranged weapons, check the player has arrows (but don't consume yet —
        // arrows are only consumed when an actual target is found)
        if weapon_type == WeaponType::Ranged {
            let players = self.players.read().await;
            if let Some(player) = players.get(player_id) {
                let has_arrows = player
                    .inventory
                    .slots
                    .iter()
                    .any(|slot| slot.as_ref().is_some_and(|s| s.item_id.ends_with("_arrow")));
                if !has_arrows {
                    drop(players);
                    self.send_to_player(
                        player_id,
                        ServerMessage::AttackResult {
                            success: false,
                            reason: Some("no_arrows".to_string()),
                        },
                    )
                    .await;
                    return;
                }
            } else {
                return;
            }
        }

        // Broadcast attack animation to nearby clients (plays even if no target hit)
        let attack_type = match weapon_type {
            WeaponType::Ranged => "ranged",
            WeaponType::Melee => "melee",
        };
        self.broadcast_to_zone(
            player_id,
            ServerMessage::PlayerAttack {
                player_id: player_id.to_string(),
                attack_type: attack_type.to_string(),
                direction: attacker_dir as u8,
            },
        )
        .await;

        // last_attack_time and movement were already updated in the claim block above.

        // Find target based on weapon range
        let mut target_id: Option<String> = None;
        let mut is_npc = false;
        let mut is_instance_npc = false;
        let mut target_tile_x = attacker_x;
        let mut target_tile_y = attacker_y;

        if let Some(forced_id) = forced_target_id {
            // Auto-action: directly target the known entity (bypasses directional scan
            // which can miss targets not on a cardinal/diagonal line)
            if attacker_instance.is_none() {
                let npcs = self.npcs.read().await;
                if let Some(npc) = npcs.get(forced_id)
                    && npc.is_alive()
                    && npc.is_attackable()
                {
                    target_id = Some(forced_id.to_string());
                    is_npc = true;
                    target_tile_x = npc.x;
                    target_tile_y = npc.y;
                }
            } else if let Some(ref inst_id) = attacker_instance
                && let Some(instance) = self.instance_manager.get_by_instance_id(inst_id)
            {
                let npcs = instance.npcs.read().await;
                if let Some(npc) = npcs.get(forced_id)
                    && npc.is_alive()
                    && npc.is_attackable()
                {
                    target_id = Some(forced_id.to_string());
                    is_npc = true;
                    is_instance_npc = true;
                    target_tile_x = npc.x;
                    target_tile_y = npc.y;
                }
            }
            // Also check players as forced target
            if target_id.is_none() {
                let players = self.players.read().await;
                if let Some(player) = players.get(forced_id)
                    && player.active
                    && player.hp > 0
                {
                    let instances = self.player_instances.read().await;
                    let target_instance = instances.get(forced_id).cloned();
                    drop(instances);
                    // PVP must be allowed at attacker's location (overworld zone or instance flag)
                    let pvp_ok = self.is_pvp_allowed(player_id, attacker_x, attacker_y).await;
                    if target_instance == attacker_instance && pvp_ok {
                        target_id = Some(forced_id.to_string());
                        is_npc = false;
                        target_tile_x = player.x;
                        target_tile_y = player.y;
                    }
                }
            }
        } else {
            // Manual attack: scan tiles in facing direction up to weapon range
            let (dir_dx, dir_dy): (i32, i32) = match attacker_dir {
                Direction::Up => (0, -1),
                Direction::Down => (0, 1),
                Direction::Left => (-1, 0),
                Direction::Right => (1, 0),
                Direction::UpLeft => (-1, -1),
                Direction::UpRight => (1, -1),
                Direction::DownLeft => (-1, 1),
                Direction::DownRight => (1, 1),
            };

            for dist in 1..=weapon_range {
                let check_x = attacker_x + dir_dx * dist;
                let check_y = attacker_y + dir_dy * dist;

                // For ranged weapons, check line of sight
                if weapon_range > 1
                    && !self
                        .world
                        .has_line_of_sight(attacker_x, attacker_y, check_x, check_y)
                        .await
                {
                    tracing::debug!(
                        "{} ranged attack blocked by wall at ({}, {})",
                        attacker_name,
                        check_x,
                        check_y
                    );
                    break;
                }

                // Check NPCs at this tile
                if attacker_instance.is_none() {
                    // Overworld NPCs
                    let npcs = self.npcs.read().await;
                    for (npc_id, npc) in npcs.iter() {
                        if npc.is_alive()
                            && npc.is_attackable()
                            && npc
                                .occupied_tiles()
                                .any(|(tx, ty)| tx == check_x && ty == check_y)
                        {
                            target_id = Some(npc_id.clone());
                            is_npc = true;
                            target_tile_x = check_x;
                            target_tile_y = check_y;
                            tracing::info!(
                                "{} found NPC target: {} at ({}, {}) range {}",
                                attacker_name,
                                npc.name(),
                                check_x,
                                check_y,
                                dist
                            );
                            break;
                        }
                    }
                } else if let Some(ref inst_id) = attacker_instance {
                    // Instance NPCs
                    if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                        let npcs = instance.npcs.read().await;
                        for (npc_id, npc) in npcs.iter() {
                            if npc.is_alive()
                                && npc.is_attackable()
                                && npc
                                    .occupied_tiles()
                                    .any(|(tx, ty)| tx == check_x && ty == check_y)
                            {
                                target_id = Some(npc_id.clone());
                                is_npc = true;
                                is_instance_npc = true;
                                target_tile_x = check_x;
                                target_tile_y = check_y;
                                tracing::info!(
                                    "{} found instance NPC target: {} at ({}, {}) range {}",
                                    attacker_name,
                                    npc.name(),
                                    check_x,
                                    check_y,
                                    dist
                                );
                                break;
                            }
                        }
                    }
                }
                if target_id.is_some() {
                    break;
                }

                // Check players at this tile (must be in same instance context)
                {
                    let players = self.players.read().await;
                    let instances = self.player_instances.read().await;
                    for (pid, player) in players.iter() {
                        if pid != player_id
                            && player.active
                            && player.hp > 0
                            && player.x == check_x
                            && player.y == check_y
                        {
                            // Only target players in the same context (both overworld, or same instance)
                            let target_instance = instances.get(pid.as_str()).cloned();
                            if target_instance != attacker_instance {
                                continue;
                            }
                            // PVP must be allowed at attacker's location
                            if !self.is_pvp_allowed(player_id, attacker_x, attacker_y).await {
                                continue;
                            }
                            target_id = Some(pid.clone());
                            is_npc = false;
                            target_tile_x = check_x;
                            target_tile_y = check_y;
                            tracing::info!(
                                "{} found player target: {} at ({}, {}) range {}",
                                attacker_name,
                                player.name,
                                check_x,
                                check_y,
                                dist
                            );
                            break;
                        }
                    }
                }
                if target_id.is_some() {
                    break;
                }
            }
        }

        // No valid target found
        let target_id = match target_id {
            Some(id) => id,
            None => {
                tracing::debug!(
                    "{} attack missed - no target in range {} facing {:?}",
                    attacker_name,
                    weapon_range,
                    attacker_dir
                );
                return;
            }
        };

        // Stall movement for 8 ticks after ranged attacks that find a target
        if weapon_type == WeaponType::Ranged {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.cast_stall_ticks = 8;
            }
        }

        // In slayer-only areas, players can only attack NPCs matching their active slayer task
        if is_npc
            && let Some(ref inst_id) = attacker_instance
            && let Some(instance) = self.instance_manager.get_by_instance_id(inst_id)
            && let Some(interior) = self.interior_registry.get(&instance.map_id)
            && interior.requires_slayer_task
        {
            let slayer_state = self.get_player_slayer_state(player_id).await;
            let npc_prototype = if is_instance_npc {
                let npcs = instance.npcs.read().await;
                npcs.get(&target_id).map(|n| n.prototype_id.clone())
            } else {
                None
            };
            if let Some(proto_id) = npc_prototype {
                let allowed = match &slayer_state.current_task {
                    Some(task) => {
                        proto_id == task.monster_id
                            || proto_id.starts_with(&format!("{}_", task.monster_id))
                    }
                    None => false,
                };
                if !allowed {
                    self.send_system_message(
                        player_id,
                        "You can only attack your slayer task monster in this area.",
                    )
                    .await;
                    return;
                }
            }
        }

        // Now that we have a confirmed target, consume 1 arrow for ranged weapons
        // and add the arrow's ranged_strength bonus to damage
        let (arrow_strength_bonus, used_arrow_id) = if weapon_type == WeaponType::Ranged {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                let arrow_id = player.inventory.slots.iter().find_map(|slot| {
                    slot.as_ref()
                        .filter(|s| s.item_id.ends_with("_arrow"))
                        .map(|s| s.item_id.clone())
                });
                if let Some(arrow_id) = arrow_id {
                    // Check if equipped back item has ammo save chance (attractor)
                    let saved = if let Some(ref back_id) = equipped_back {
                        if let Some(back_def) = self.item_registry.get(back_id) {
                            if let Some(ref equip) = back_def.equipment {
                                equip.ammo_save_chance > 0.0
                                    && rand::random::<f32>() < equip.ammo_save_chance
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if !saved {
                        player.inventory.remove_item(&arrow_id, 1);
                        let inv_update = player.inventory.to_update();
                        let gold = player.inventory.gold;
                        drop(players);
                        self.send_to_player(
                            player_id,
                            ServerMessage::InventoryUpdate {
                                player_id: player_id.to_string(),
                                slots: inv_update,
                                gold,
                            },
                        )
                        .await;
                    } else {
                        drop(players);
                    }
                    // Look up arrow's ranged_strength bonus
                    let bonus = self
                        .item_registry
                        .get(&arrow_id)
                        .map(|def| def.ranged_strength)
                        .unwrap_or(0);
                    (bonus, Some(arrow_id))
                } else {
                    // Arrows ran out between check and consumption (unlikely but safe)
                    return;
                }
            } else {
                return;
            }
        } else {
            (0, None)
        };
        let strength_bonus = strength_bonus + arrow_strength_bonus + equip_ranged_str_bonus;

        // Fetch slayer state for helmet damage boost check (only if wearing slayer helmet)
        let slayer_task_monster = if equipped_head.as_deref() == Some("slayer_helmet") {
            let slayer_state = self.get_player_slayer_state(player_id).await;
            slayer_state.current_task.map(|t| t.monster_id)
        } else {
            None
        };

        // Apply damage to target using hit/miss mechanics
        // 1. Roll attack vs defence to determine if we hit
        // 2. If hit, calculate max hit from strength and roll damage
        let (target_hp, target_name, target_died, actual_damage) = if is_npc && is_instance_npc {
            // Instance NPC combat
            let instance = self
                .instance_manager
                .get_by_instance_id(attacker_instance.as_ref().unwrap());
            if let Some(inst) = instance {
                let mut npcs = inst.npcs.write().await;
                if let Some(npc) = npcs.get_mut(&target_id) {
                    // Invulnerable NPCs (e.g. boss underground) cannot be hit
                    if npc.invulnerable {
                        let name = npc.name();
                        (npc.hp, name, false, 0)
                    } else {
                        let npc_defence_level = npc.level;
                        let npc_defence_bonus = npc.stats.defence_bonus;

                        if !calculate_hit(
                            attack_level,
                            attack_bonus,
                            npc_defence_level,
                            npc_defence_bonus,
                        ) {
                            npc.take_damage(0, current_time, Some(player_id));
                            let name = npc.name();
                            tracing::info!(
                                "{} misses instance NPC {} (atk {} + {} vs def {} + {})",
                                attacker_name,
                                name,
                                attack_level,
                                attack_bonus,
                                npc_defence_level,
                                npc_defence_bonus
                            );
                            (npc.hp, name, false, 0)
                        } else {
                            let mut max_hit = calculate_max_hit(strength_level, strength_bonus);
                            // Slayer helmet: 15% damage boost against current slayer task
                            if let Some(ref task_monster) = slayer_task_monster {
                                let proto = &npc.prototype_id;
                                if proto == task_monster
                                    || proto.starts_with(&format!("{}_", task_monster))
                                {
                                    max_hit = ((max_hit as f32) * 1.15).floor() as i32;
                                }
                            }
                            // Equipment type bonuses (e.g. +15% vs desert enemies)
                            let type_str_pct = calc_type_bonus_str(&npc.stats.tags);
                            if type_str_pct > 0.0 {
                                max_hit = ((max_hit as f32) * (1.0 + type_str_pct / 100.0)).floor()
                                    as i32;
                            }
                            let damage = roll_damage(max_hit).min(npc.hp);
                            let died = npc.take_damage(damage, current_time, Some(player_id));
                            let name = npc.name();
                            tracing::info!(
                                "{} hits instance NPC {} for {} damage (max: {}, HP: {})",
                                attacker_name,
                                name,
                                damage,
                                max_hit,
                                npc.hp
                            );
                            // Track damage dealer for boss loot distribution
                            if damage > 0
                                && let Some(ref inst_id) = attacker_instance
                            {
                                let mut boss_states = self.boss_states.write().await;
                                if let Some(boss) = boss_states.get_mut(inst_id) {
                                    boss.damage_dealers.insert(player_id.to_string());
                                }
                            }
                            (npc.hp, name, died, damage)
                        }
                    } // end invulnerable else
                } else {
                    return;
                }
            } else {
                return;
            }
        } else if is_npc {
            // Overworld NPC combat
            let mut npcs = self.npcs.write().await;
            if let Some(npc) = npcs.get_mut(&target_id) {
                // NPC's defence = level, no equipment bonus
                let npc_defence_level = npc.level;
                let npc_defence_bonus = npc.stats.defence_bonus;

                // Check if attack hits (attack_level for accuracy)
                if !calculate_hit(
                    attack_level,
                    attack_bonus,
                    npc_defence_level,
                    npc_defence_bonus,
                ) {
                    // Miss - deal 0 damage
                    // Still register aggro so attack attempts interrupt wandering/pathing.
                    npc.take_damage(0, current_time, Some(player_id));
                    let name = npc.name();
                    tracing::info!(
                        "{} misses {} (atk {} + {} vs def {} + {})",
                        attacker_name,
                        name,
                        attack_level,
                        attack_bonus,
                        npc_defence_level,
                        npc_defence_bonus
                    );
                    (npc.hp, name, false, 0)
                } else {
                    // Hit - calculate and apply damage
                    let mut max_hit = calculate_max_hit(strength_level, strength_bonus);
                    // Slayer helmet: 15% damage boost against current slayer task
                    if let Some(ref task_monster) = slayer_task_monster {
                        let proto = &npc.prototype_id;
                        if proto == task_monster || proto.starts_with(&format!("{}_", task_monster))
                        {
                            max_hit = ((max_hit as f32) * 1.15).floor() as i32;
                        }
                    }
                    // Equipment type bonuses (e.g. +15% vs desert enemies)
                    let type_str_pct = calc_type_bonus_str(&npc.stats.tags);
                    if type_str_pct > 0.0 {
                        max_hit = ((max_hit as f32) * (1.0 + type_str_pct / 100.0)).floor() as i32;
                    }
                    let damage = roll_damage(max_hit).min(npc.hp);
                    let died = npc.take_damage(damage, current_time, Some(player_id));
                    let name = npc.name();
                    tracing::info!(
                        "{} hits {} for {} damage (max: {}, HP: {})",
                        attacker_name,
                        name,
                        damage,
                        max_hit,
                        npc.hp
                    );
                    (npc.hp, name, died, damage)
                }
            } else {
                return;
            }
        } else {
            // Players have defence from skills and equipment
            let mut players = self.players.write().await;
            if let Some(target) = players.get_mut(&target_id) {
                if target.is_dead {
                    return; // Already dead
                }
                // God mode prevents all damage
                if target.is_god_mode {
                    return;
                }

                // Get target's defence stats
                let target_defence_level = target.skills.defence.level;
                let base_defence_bonus = target.defence_bonus(&self.item_registry);

                // Apply prayer bonuses to target's defence
                let target_active_ids: Vec<String> =
                    target.active_prayers.iter().cloned().collect();
                let target_prayer_effects =
                    self.prayer_registry.calculate_effects(&target_active_ids);
                let target_defence_bonus =
                    target_prayer_effects.apply_defence_bonus(base_defence_bonus);

                // Check if attack hits
                if !calculate_hit(
                    attack_level,
                    attack_bonus,
                    target_defence_level,
                    target_defence_bonus,
                ) {
                    // Miss - deal 0 damage
                    let name = target.name.clone();
                    tracing::info!(
                        "{} misses {} (atk {} + {} vs def {} + {})",
                        attacker_name,
                        name,
                        attack_level,
                        attack_bonus,
                        target_defence_level,
                        target_defence_bonus
                    );
                    (target.hp, name, false, 0)
                } else {
                    // Hit - calculate and apply damage
                    let max_hit = calculate_max_hit(strength_level, strength_bonus);
                    let raw_damage = roll_damage(max_hit);
                    // Apply prayer damage reduction, then clamp to remaining HP
                    let damage = target_prayer_effects
                        .apply_damage_reduction(raw_damage)
                        .min(target.hp);
                    target.hp -= damage;
                    let name = target.name.clone();
                    let died = target.hp <= 0;
                    if died {
                        target.die(current_time);
                    }
                    tracing::info!(
                        "{} hits {} for {} damage (max: {}, raw: {}, HP: {})",
                        attacker_name,
                        name,
                        damage,
                        max_hit,
                        raw_damage,
                        target.hp
                    );
                    (target.hp, name, died, damage)
                }
            } else {
                return;
            }
        };

        // Use actual target position for damage event (important for ranged projectiles)
        let target_x = target_tile_x as f32;
        let target_y = target_tile_y as f32;

        // Determine projectile type for ranged attacks (use actual arrow item id)
        let projectile = used_arrow_id;

        // Broadcast damage event to players in the same zone (instance or overworld)
        let damage_msg = ServerMessage::DamageEvent {
            source_id: player_id.to_string(),
            target_id: target_id.clone(),
            damage: actual_damage,
            target_hp,
            target_x,
            target_y,
            projectile,
        };
        self.broadcast_to_zone(player_id, damage_msg).await;

        // Send success result to attacker
        let result_msg = ServerMessage::AttackResult {
            success: true,
            reason: None,
        };
        self.send_to_player(player_id, result_msg).await;

        // Award combat XP on every successful hit (OSRS style: XP per damage dealt)
        if actual_damage > 0 {
            let xp_results = {
                let mut players = self.players.write().await;
                if let Some(attacker) = players.get_mut(player_id) {
                    let style = attacker.combat_style;
                    Some(attacker.award_combat_xp(actual_damage, style, weapon_type))
                } else {
                    None
                }
            };

            if let Some(results) = xp_results {
                let mut progression_needs_sync = false;
                for (skill_type, xp_gained, total_xp, level, leveled_up) in results {
                    self.send_to_player(
                        player_id,
                        ServerMessage::SkillXp {
                            player_id: player_id.to_string(),
                            skill: skill_type.as_str().to_string(),
                            xp_gained,
                            total_xp,
                            level,
                        },
                    )
                    .await;

                    if leveled_up {
                        tracing::info!(
                            "Player {} leveled up {} to {}",
                            player_id,
                            skill_type.as_str(),
                            level
                        );
                        self.broadcast_skill_level_up(player_id, skill_type.as_str(), level)
                            .await;
                        progression_needs_sync = true;
                    }
                }

                if progression_needs_sync {
                    self.process_quest_progression_snapshot(player_id).await;
                }
            }
        }

        // Interrupt crafting if target is a player who took damage
        if !is_npc && actual_damage > 0 {
            self.cancel_crafting(&target_id, "interrupted").await;
        }

        // Handle death
        if target_died {
            tracing::info!("{} killed {}", attacker_name, target_name);
            if is_npc {
                // Get NPC info for exp and loot
                let (prototype_id, npc_level) = if is_instance_npc {
                    let inst = self
                        .instance_manager
                        .get_by_instance_id(attacker_instance.as_ref().unwrap());
                    if let Some(inst) = inst {
                        let npcs = inst.npcs.read().await;
                        npcs.get(&target_id)
                            .map(|n| (n.prototype_id.clone(), n.level))
                            .unwrap_or(("unknown".to_string(), 1))
                    } else {
                        ("unknown".to_string(), 1)
                    }
                } else {
                    let npcs = self.npcs.read().await;
                    npcs.get(&target_id)
                        .map(|n| (n.prototype_id.clone(), n.level))
                        .unwrap_or(("unknown".to_string(), 1))
                };

                // Broadcast NPC death (scoped to zone)
                let death_msg = ServerMessage::NpcDied {
                    id: target_id.clone(),
                    killer_id: player_id.to_string(),
                };
                self.broadcast_to_zone(player_id, death_msg).await;

                // Clear attacker's auto-action since target is dead
                self.clear_auto_action(player_id, "target_dead").await;

                // Persist monster kill count for stats leaderboards.
                self.record_monster_kill(player_id).await;

                // Process quest kill event
                self.process_quest_kill(player_id, &prototype_id).await;

                // Process slayer kill event
                self.process_slayer_kill(player_id, &prototype_id).await;

                // Check KOTH NPC death
                if let Some(ref inst_id) = attacker_instance {
                    let ct = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    self.check_koth_npc_death(&target_id, inst_id, ct).await;

                    // Check boss minion death (player killed a minion via combat)
                    self.check_boss_minion_death(
                        &target_id,
                        inst_id,
                        target_x as i32,
                        target_y as i32,
                        ct,
                    )
                    .await;

                    // Check pharaoh minion death (player killed a pharaoh minion via combat)
                    self.check_pharaoh_minion_death(&target_id, inst_id, ct)
                        .await;

                    // Check boss NPC death (player killed the boss)
                    self.check_boss_npc_death(&target_id, inst_id, Some(player_id), ct)
                        .await;
                }

                // Skip loot drops in boss arena (rewards come from battle master)
                let in_boss_arena = attacker_instance
                    .as_ref()
                    .map(|id| id.contains(crate::game::boss_tick::BOSS_MAP_ID))
                    .unwrap_or(false);

                // Spawn item drops from prototype loot table
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                let drops = if in_boss_arena {
                    vec![]
                } else {
                    // Get killer's current instance for loot zone tracking
                    let killer_instance = {
                        let instances = self.player_instances.read().await;
                        instances.get(player_id).cloned()
                    };

                    if let Some(prototype) = self.entity_registry.get(&prototype_id) {
                        crate::entity::generate_loot_from_prototype(
                            prototype,
                            target_x,
                            target_y,
                            player_id,
                            current_time,
                            npc_level,
                            killer_instance,
                        )
                    } else {
                        vec![]
                    }
                };

                for item in drops {
                    let mut items = self.ground_items.write().await;

                    // For gold, try to combine with existing pile at same tile
                    if item.item_id == "gold" {
                        let tile_x = item.x.floor() as i32;
                        let tile_y = item.y.floor() as i32;

                        // Find existing gold at same tile with same owner
                        let existing_gold_id = items
                            .iter()
                            .find(|(_, existing)| {
                                existing.item_id == "gold"
                                    && existing.x.floor() as i32 == tile_x
                                    && existing.y.floor() as i32 == tile_y
                                    && existing.owner_id == item.owner_id
                            })
                            .map(|(id, _)| id.clone());

                        if let Some(existing_id) = existing_gold_id {
                            // Combine with existing pile
                            if let Some(existing) = items.get_mut(&existing_id) {
                                existing.quantity += item.quantity;
                                let update_msg = ServerMessage::ItemQuantityUpdated {
                                    id: existing_id.clone(),
                                    quantity: existing.quantity,
                                };
                                drop(items); // Release lock before broadcast
                                self.broadcast_to_zone(player_id, update_msg).await;
                            }
                            continue;
                        }
                    }

                    // No existing pile to combine with - create new item
                    let drop_msg = ServerMessage::ItemDropped {
                        id: item.id.clone(),
                        item_id: item.item_id.clone(),
                        x: item.x,
                        y: item.y,
                        quantity: item.quantity,
                    };
                    items.insert(item.id.clone(), item);
                    drop(items); // Release lock before broadcast
                    self.broadcast_to_zone(player_id, drop_msg).await;
                }
            } else {
                // Check if this is an arena fight death
                let arena_death = {
                    let arena = self.arena_manager.read().await;
                    arena.is_fighting() && arena.is_in_ring(&target_id)
                };

                if arena_death {
                    // Arena death: notify arena, teleport to spectator zone
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

                    // Teleport dead player to spectator spawn instead of normal death
                    {
                        let spectator_spawn = {
                            let arena = self.arena_manager.read().await;
                            arena.active_spectator_spawn()
                        };
                        let mut players = self.players.write().await;
                        if let Some(p) = players.get_mut(&target_id) {
                            p.hp = p.skills.hitpoints.level; // Revive
                            p.is_dead = false;
                            p.x = spectator_spawn.0;
                            p.y = spectator_spawn.1;
                        }
                    }

                    // Broadcast elimination
                    self.broadcast_to_arena(ServerMessage::ArenaPlayerEliminated {
                        player_id: target_id.clone(),
                        player_name: eliminated_name,
                        killer_id: player_id.to_string(),
                        killer_name,
                        remaining,
                    })
                    .await;

                    // Check if match should end
                    let should_end = {
                        let arena = self.arena_manager.read().await;
                        tracing::info!(
                            "[ARENA] After death: active_fighters={:?}, state={:?}, check_match_end={}",
                            arena.active_fighters,
                            arena.state,
                            arena.check_match_end()
                        );
                        arena.check_match_end()
                    };
                    if should_end {
                        let current_time = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64;
                        let placements = {
                            let mut arena = self.arena_manager.write().await;
                            arena.end_match(current_time)
                        };
                        tracing::info!("[ARENA] Match ended! {} placements", placements.len());

                        // Distribute rewards
                        {
                            let mut players = self.players.write().await;
                            for placement in &placements {
                                if placement.gold_reward > 0
                                    && let Some(p) = players.get_mut(&placement.player_id)
                                {
                                    p.inventory.gold = item::checked_gold_credit(
                                        p.inventory.gold,
                                        placement.gold_reward,
                                    )
                                    .unwrap_or(item::MAX_GOLD);
                                }
                            }
                        }

                        let placement_data: Vec<crate::protocol::ArenaPlacementData> = placements
                            .iter()
                            .map(|p| crate::protocol::ArenaPlacementData {
                                rank: p.rank,
                                player_id: p.player_id.clone(),
                                player_name: p.player_name.clone(),
                                kills: p.kills,
                                gold_reward: p.gold_reward,
                            })
                            .collect();

                        self.broadcast_to_arena(ServerMessage::ArenaMatchEnd {
                            placements: placement_data,
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

                        // Teleport all fighters (including winner) to spectator spawn
                        {
                            let spectator_spawn = {
                                let arena = self.arena_manager.read().await;
                                arena.active_spectator_spawn()
                            };
                            let mut players = self.players.write().await;
                            for placement in &placements {
                                if let Some(p) = players.get_mut(&placement.player_id) {
                                    p.x = spectator_spawn.0;
                                    p.y = spectator_spawn.1;
                                    if p.is_dead {
                                        p.hp = p.skills.hitpoints.level;
                                        p.is_dead = false;
                                    }
                                }
                            }
                        }

                        // Send inventory updates for gold rewards
                        for placement in &placements {
                            if placement.gold_reward > 0 {
                                let update = {
                                    let players = self.players.read().await;
                                    players
                                        .get(&placement.player_id)
                                        .map(|p| (p.inventory.to_update(), p.inventory.gold))
                                };
                                if let Some((slots, gold)) = update {
                                    self.send_to_player(
                                        &placement.player_id,
                                        ServerMessage::InventoryUpdate {
                                            player_id: placement.player_id.clone(),
                                            slots,
                                            gold,
                                        },
                                    )
                                    .await;
                                }
                            }
                        }

                        // Save arena stats to DB
                        if let Some(ref db) = self.db {
                            for placement in &placements {
                                if let Some(char_id) = placement
                                    .player_id
                                    .strip_prefix("char_")
                                    .and_then(|s| s.parse::<i64>().ok())
                                {
                                    let won = placement.rank == 1;
                                    let died = placement.rank > 1;
                                    if let Err(e) = db
                                        .update_arena_stats(
                                            char_id,
                                            won,
                                            placement.kills,
                                            died,
                                            placement.gold_reward,
                                        )
                                        .await
                                    {
                                        tracing::warn!(
                                            "Failed to save arena stats for {}: {}",
                                            placement.player_id,
                                            e
                                        );
                                    }
                                }
                            }
                        }

                        // Notify winner
                        if let Some(winner) = placements.iter().find(|p| p.rank == 1) {
                            self.send_system_message(
                                &winner.player_id,
                                &format!("You won the arena match! +{} gold", winner.gold_reward),
                            )
                            .await;
                        }
                    }
                } else {
                    // Normal player death
                    let death_msg = ServerMessage::PlayerDied {
                        id: target_id.clone(),
                        killer_id: player_id.to_string(),
                    };
                    self.broadcast_to_zone(player_id, death_msg).await;

                    self.clear_auto_action(&target_id, "player_died").await;

                    // Send prayer state update to dying player (prayers cleared on death)
                    let (points, max_points) = {
                        let players = self.players.read().await;
                        if let Some(p) = players.get(&target_id) {
                            (p.prayer_points, p.max_prayer_points())
                        } else {
                            (0, 1)
                        }
                    };
                    self.send_to_player(
                        &target_id,
                        ServerMessage::PrayerStateUpdate {
                            points,
                            max_points,
                            active_prayers: vec![], // Cleared on death
                        },
                    )
                    .await;
                }
            }
        }
    }

    fn directions_match(dir1: Direction, dir2: Direction) -> bool {
        // Convert to numeric for comparison
        let d1 = dir1 as i32;
        let d2 = dir2 as i32;
        let diff = (d1 - d2).abs();
        // Directions match if they're the same or adjacent (with wraparound)
        diff <= 1 || diff == 7
    }

    pub async fn handle_target(&self, player_id: &str, target_id: &str) {
        tracing::info!(
            "Target request: player {} -> target '{}'",
            player_id,
            target_id
        );

        // Validate target exists (can be player or NPC)
        let valid_target = {
            if target_id.is_empty() {
                true // Clear target
            } else if target_id == player_id {
                false // Can't target self
            } else {
                // Check if target is a player
                let players = self.players.read().await;
                let is_player = players.get(target_id).map(|p| p.active).unwrap_or(false);
                drop(players);

                if is_player {
                    true
                } else {
                    // Check if target is an NPC (overworld first, then instance)
                    let npcs = self.npcs.read().await;
                    let is_overworld_npc =
                        npcs.get(target_id).map(|n| n.is_alive()).unwrap_or(false);
                    drop(npcs);

                    if is_overworld_npc {
                        true
                    } else {
                        // Check instance NPCs
                        let player_inst =
                            self.player_instances.read().await.get(player_id).cloned();
                        if let Some(inst_id) = player_inst {
                            if let Some(instance) =
                                self.instance_manager.get_by_instance_id(&inst_id)
                            {
                                let inst_npcs = instance.npcs.read().await;
                                inst_npcs
                                    .get(target_id)
                                    .map(|n| n.is_alive())
                                    .unwrap_or(false)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                }
            }
        };

        if valid_target {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                let new_target = if target_id.is_empty() {
                    None
                } else {
                    Some(target_id.to_string())
                };
                player.target_id = new_target.clone();
                tracing::info!("{} now targeting {:?}", player.name, new_target);

                // Broadcast target change to nearby clients.
                let msg = ServerMessage::TargetChanged {
                    player_id: player_id.to_string(),
                    target_id: new_target,
                };
                drop(players); // Release lock before broadcast
                self.broadcast_to_zone(player_id, msg).await;
            }
        }
    }

    pub async fn handle_pickup(&self, player_id: &str, item_id: &str) {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Get player position
        let (player_x, player_y) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => (p.x as f32, p.y as f32),
                _ => return, // Player not found, inactive, or dead
            }
        };
        let player_instance = self.player_instances.read().await.get(player_id).cloned();

        // Check if item exists and can be picked up
        let (item_info, protection_remaining) = {
            let items = self.ground_items.read().await;
            match items.get(item_id) {
                Some(item) => {
                    if item.instance_id != player_instance {
                        tracing::warn!(
                            "Rejected cross-instance pickup from {} for {}",
                            player_id,
                            item_id
                        );
                        return;
                    }
                    let dx = item.x - player_x;
                    let dy = item.y - player_y;
                    let distance = (dx * dx + dy * dy).sqrt();

                    if distance > 2.0 {
                        (None, None)
                    } else if !item.can_pickup(player_id, current_time) {
                        let elapsed = current_time.saturating_sub(item.drop_time);
                        let remaining_ms = 10000u64.saturating_sub(elapsed);
                        let remaining_secs = remaining_ms.div_ceil(1000);
                        (None, Some(remaining_secs))
                    } else {
                        (Some((item.item_id.clone(), item.quantity)), None)
                    }
                }
                None => (None, None),
            }
        };

        if let Some(secs) = protection_remaining {
            self.send_system_message(
                player_id,
                &format!(
                    "That item is protected for {} more second{}.",
                    secs,
                    if secs == 1 { "" } else { "s" }
                ),
            )
            .await;
            return;
        }

        if let Some((picked_item_id, quantity)) = item_info {
            // Check if player has inventory space before removing from ground
            let has_space = {
                let players = self.players.read().await;
                match players.get(player_id) {
                    Some(player) => player.inventory.has_space_for(
                        &picked_item_id,
                        quantity,
                        &self.item_registry,
                    ),
                    None => return,
                }
            };

            if !has_space {
                self.send_system_message(player_id, "Your inventory is full.")
                    .await;
                return;
            }

            // Remove item from ground
            let removed = {
                let mut items = self.ground_items.write().await;
                items.remove(item_id).is_some()
            };

            if removed {
                // Check if this was a persistent ground spawn
                {
                    let mut gsm = self.ground_spawn_manager.write().await;
                    gsm.mark_picked_up(item_id);
                }

                // Get display name from registry for logging
                let display_name = self
                    .item_registry
                    .get(&picked_item_id)
                    .map(|def| def.display_name.as_str())
                    .unwrap_or(&picked_item_id);
                tracing::debug!(
                    "Player {} picked up {} x{}",
                    player_id,
                    display_name,
                    quantity
                );

                // Add to player's inventory
                let (inventory_update, gold) = {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        player
                            .inventory
                            .add_item(&picked_item_id, quantity, &self.item_registry);
                        (player.inventory.to_update(), player.inventory.gold)
                    } else {
                        return;
                    }
                };

                // Process quest item collection
                self.process_quest_item_collect(player_id, &picked_item_id, quantity)
                    .await;

                // Broadcast pickup to players in same zone
                let pickup_msg = ServerMessage::ItemPickedUp {
                    item_id: item_id.to_string(),
                    player_id: player_id.to_string(),
                };
                self.broadcast_to_zone(player_id, pickup_msg).await;

                // SECURITY: Unicast inventory update (private - only this player receives)
                let inv_msg = ServerMessage::InventoryUpdate {
                    player_id: player_id.to_string(),
                    slots: inventory_update,
                    gold,
                };
                self.send_to_player(player_id, inv_msg).await;
            }
        }
    }
}
