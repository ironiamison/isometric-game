use super::*;

impl GameRoom {
    pub async fn handle_chunk_request(&self, chunk_x: i32, chunk_y: i32) -> Option<ServerMessage> {
        use crate::chunk::WallEdge;
        use crate::protocol::{ChunkLayerData, ChunkObjectData, ChunkPortalData, ChunkWallData};

        let coord = ChunkCoord::new(chunk_x, chunk_y);
        if let Some(chunk) = self.world.get_chunk_data(coord).await {
            let layers: Vec<ChunkLayerData> = chunk
                .layers
                .iter()
                .map(|layer| ChunkLayerData {
                    layer_type: layer.layer_type as u8,
                    tiles: layer.tiles.clone(),
                })
                .collect();

            let collision = chunk.pack_collision();

            let objects: Vec<ChunkObjectData> = chunk
                .objects
                .iter()
                .map(|obj| ChunkObjectData {
                    gid: obj.gid,
                    tile_x: obj.tile_x,
                    tile_y: obj.tile_y,
                    width: obj.width,
                    height: obj.height,
                })
                .collect();

            let portals: Vec<ChunkPortalData> = chunk
                .portals
                .iter()
                .map(|p| ChunkPortalData {
                    id: p.id.clone(),
                    x: p.x,
                    y: p.y,
                    width: p.width,
                    height: p.height,
                    target_map: p.target_map.clone(),
                    target_spawn: p.target_spawn.clone(),
                })
                .collect();

            Some(ServerMessage::ChunkData {
                chunk_x,
                chunk_y,
                layers,
                collision,
                objects,
                walls: chunk
                    .walls
                    .iter()
                    .map(|w| ChunkWallData {
                        gid: w.gid,
                        tile_x: w.tile_x,
                        tile_y: w.tile_y,
                        edge: match w.edge {
                            WallEdge::Down => "down".to_string(),
                            WallEdge::Right => "right".to_string(),
                        },
                    })
                    .collect(),
                portals,
                heightmap: chunk.height_data.as_ref().map(|h| h.heights.clone()),
                block_types_down: chunk
                    .height_data
                    .as_ref()
                    .map(|h| h.block_types_down.clone()),
                block_types_right: chunk
                    .height_data
                    .as_ref()
                    .map(|h| h.block_types_right.clone()),
            })
        } else {
            Some(ServerMessage::ChunkNotFound { chunk_x, chunk_y })
        }
    }

    pub fn world(&self) -> &Arc<World> {
        &self.world
    }

    pub async fn update_player_chunk(&self, player_id: &str, new_chunk: ChunkCoord) -> bool {
        let mut chunks = self.player_chunks.write().await;
        let old_chunk = chunks.get(player_id).copied();
        if old_chunk != Some(new_chunk) {
            chunks.insert(player_id.to_string(), new_chunk);
            return true;
        }
        false
    }

    pub fn get_entity_definitions(&self) -> ServerMessage {
        use crate::protocol::ClientEntityDef;

        let entities: Vec<ClientEntityDef> = self
            .entity_registry
            .all()
            .map(|proto| ClientEntityDef {
                id: proto.id.clone(),
                display_name: proto.display_name.clone(),
                sprite: proto.sprite.clone(),
                animation_type: format!("{:?}", proto.animation_type).to_lowercase(),
                max_hp: proto.stats.max_hp,
            })
            .collect();

        ServerMessage::EntityDefinitions { entities }
    }

    pub async fn handle_cast_spell(&self, player_id: &str, spell_id: &str) {
        // 1. Resolve spell: check static spells first, then scroll spell registry
        let resolved = if let Some(s) = crate::spell::get_spell(spell_id) {
            ResolvedSpell {
                id: s.id.to_string(),
                spell_type: s.spell_type,
                magic_level_req: Some(s.magic_level_req),
                mana_cost: s.mana_cost,
                cooldown_ms: s.cooldown_ms,
                base_power: s.base_power,
                effect_sprite: s.effect_sprite.to_string(),
                pushback_distance: 0,
                wall_slam_damage_per_tile: 0,
                is_scroll_spell: false,
            }
        } else if let Some(s) = self.scroll_spell_registry.get(spell_id) {
            ResolvedSpell {
                id: s.id.clone(),
                spell_type: s.spell_type,
                magic_level_req: None, // Scroll spells skip magic level checks
                mana_cost: s.mana_cost,
                cooldown_ms: s.cooldown_ms,
                base_power: s.base_power,
                effect_sprite: s.effect_sprite.clone(),
                pushback_distance: s.pushback_distance,
                wall_slam_damage_per_tile: s.wall_slam_damage_per_tile,
                is_scroll_spell: true,
            }
        } else {
            self.send_to_player(
                player_id,
                ServerMessage::SpellResult {
                    success: false,
                    reason: Some("Unknown spell".to_string()),
                    spell_id: Some(spell_id.to_string()),
                },
            )
            .await;
            return;
        };

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // 2. Validate under read lock first
        {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) if !p.is_dead && p.active => p,
                _ => return,
            };

            // Scroll spells require unlock instead of magic level
            if resolved.is_scroll_spell {
                if !player.unlocked_spells.contains(&resolved.id) {
                    self.send_to_player(
                        player_id,
                        ServerMessage::SpellResult {
                            success: false,
                            reason: Some("You haven't learned this spell".to_string()),
                            spell_id: Some(resolved.id.clone()),
                        },
                    )
                    .await;
                    return;
                }
            } else if let Some(req) = resolved.magic_level_req {
                // Check magic level for static spells
                if player.skills.magic.level < req {
                    self.send_to_player(
                        player_id,
                        ServerMessage::SpellResult {
                            success: false,
                            reason: Some("Magic level too low".to_string()),
                            spell_id: Some(resolved.id.clone()),
                        },
                    )
                    .await;
                    return;
                }
            }
            // Check mana
            if player.mp < resolved.mana_cost {
                self.send_to_player(
                    player_id,
                    ServerMessage::SpellResult {
                        success: false,
                        reason: Some("Not enough mana".to_string()),
                        spell_id: Some(resolved.id.clone()),
                    },
                )
                .await;
                return;
            }
            // Check cooldown
            if let Some(&last_cast) = player.spell_cooldowns.get(&resolved.id)
                && current_time < last_cast + resolved.cooldown_ms
            {
                self.send_to_player(
                    player_id,
                    ServerMessage::SpellResult {
                        success: false,
                        reason: Some("Spell on cooldown".to_string()),
                        spell_id: Some(resolved.id.clone()),
                    },
                )
                .await;
                return;
            }
        }

        // 3. Dispatch based on spell type
        match resolved.spell_type {
            crate::spell::SpellType::Damage => {
                self.cast_damage_spell_resolved(player_id, &resolved, current_time)
                    .await
            }
            crate::spell::SpellType::Heal => {
                self.cast_heal_spell_resolved(player_id, &resolved, current_time)
                    .await
            }
            crate::spell::SpellType::Teleport => {
                // For static teleport spells (Return Home), delegate to existing handler
                if let Some(spell_def) = crate::spell::get_spell(spell_id) {
                    self.cast_return_home_spell(player_id, spell_def, current_time)
                        .await;
                }
            }
        }
    }

    async fn cast_damage_spell_resolved(
        &self,
        player_id: &str,
        spell_def: &ResolvedSpell,
        current_time: u64,
    ) {
        // 1. Get attacker info and target
        let (
            caster_name,
            caster_x,
            caster_y,
            target_id_opt,
            magic_level,
            attack_level,
            magic_bonus,
        ) = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) if !p.is_dead && p.active => p,
                _ => return,
            };
            (
                player.name.clone(),
                player.x,
                player.y,
                player.target_id.clone(),
                player.skills.magic.level,
                player.skills.attack.level,
                player.magic_bonus(&self.item_registry),
            )
        };

        // Effective attack level for spells: blend of attack and magic
        let effective_level = (attack_level + magic_level) / 2;

        // Must have a target
        let target_id = match target_id_opt {
            Some(id) => id,
            None => {
                self.send_to_player(
                    player_id,
                    ServerMessage::SpellResult {
                        success: false,
                        reason: Some("No target selected".to_string()),
                        spell_id: Some(spell_def.id.clone()),
                    },
                )
                .await;
                return;
            }
        };

        // Determine caster's instance context (None = overworld)
        let caster_instance = self.player_instances.read().await.get(player_id).cloned();

        // 2. Resolve target: check NPCs first, then players (same pattern as handle_attack)
        let mut is_npc = false;
        let mut is_instance_npc = false;
        let mut target_x: i32 = 0;
        let mut target_y: i32 = 0;
        let mut target_exists = false;

        // Check NPCs - instance NPCs if in instance, overworld NPCs if in overworld
        if let Some(ref inst_id) = caster_instance {
            if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                let npcs = instance.npcs.read().await;
                if let Some(npc) = npcs.get(&target_id)
                    && npc.is_alive()
                    && npc.is_attackable()
                {
                    is_npc = true;
                    is_instance_npc = true;
                    target_x = npc.x;
                    target_y = npc.y;
                    target_exists = true;
                }
            }
        } else {
            let npcs = self.npcs.read().await;
            if let Some(npc) = npcs.get(&target_id)
                && npc.is_alive()
                && npc.is_attackable()
            {
                is_npc = true;
                target_x = npc.x;
                target_y = npc.y;
                target_exists = true;
            }
        }

        // Check players if not an NPC (must be in same instance context)
        if !target_exists {
            let players = self.players.read().await;
            let instances = self.player_instances.read().await;
            let target_instance = instances.get(target_id.as_str()).cloned();
            // PVP must be allowed at caster's location
            let pvp_ok = self.is_pvp_allowed(player_id, caster_x, caster_y).await;
            if pvp_ok
                && let Some(target) = players.get(&target_id)
                && target.active
                && target.hp > 0
                && !target.is_dead
                && target_instance == caster_instance
            {
                is_npc = false;
                target_x = target.x;
                target_y = target.y;
                target_exists = true;
            }
        }

        if !target_exists {
            self.send_to_player(
                player_id,
                ServerMessage::SpellResult {
                    success: false,
                    reason: Some("Invalid target".to_string()),
                    spell_id: Some(spell_def.id.clone()),
                },
            )
            .await;
            return;
        }

        // 3. Check range (Chebyshev distance, 5 tiles for spells)
        let dx = (caster_x - target_x).abs();
        let dy = (caster_y - target_y).abs();
        let distance = dx.max(dy);
        if distance > 5 {
            self.send_to_player(
                player_id,
                ServerMessage::SpellResult {
                    success: false,
                    reason: Some("Target out of range".to_string()),
                    spell_id: Some(spell_def.id.clone()),
                },
            )
            .await;
            return;
        }

        // 4. Deduct mana and set cooldown
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.mp -= spell_def.mana_cost;
                player
                    .spell_cooldowns
                    .insert(spell_def.id.to_string(), current_time);
                // Stall movement for 10 ticks when casting damage spells
                player.cast_stall_ticks = 10;
            }
        }

        // 5. Broadcast casting animation
        let face_dir = direction_from_delta(target_x - caster_x, target_y - caster_y);
        self.broadcast_to_zone(
            player_id,
            ServerMessage::PlayerAttack {
                player_id: player_id.to_string(),
                attack_type: "spell".to_string(),
                direction: face_dir as u8,
            },
        )
        .await;

        // 6. Calculate hit/miss using blended combat+magic level
        let attack_bonus = magic_bonus; // Spells use magic bonus for accuracy

        // Helper closure-like macro for NPC spell damage (used for both overworld and instance NPCs)
        macro_rules! apply_spell_to_npc {
            ($npc:expr) => {{
                let npc_defence_level = $npc.level;
                let npc_defence_bonus = $npc.stats.defence_bonus;

                if !crate::skills::calculate_hit(
                    effective_level,
                    attack_bonus,
                    npc_defence_level,
                    npc_defence_bonus,
                ) {
                    // Miss
                    $npc.take_damage(0, current_time, Some(player_id));
                    let name = $npc.name();
                    tracing::info!(
                        "{} spell misses {} (eff {} [atk{}+mag{}] vs def {})",
                        caster_name,
                        name,
                        effective_level,
                        attack_level,
                        magic_level,
                        npc_defence_level
                    );
                    ($npc.hp, name, false, 0)
                } else {
                    // Hit
                    let max_hit =
                        crate::spell::calculate_spell_max_hit(magic_level, spell_def.base_power);
                    let damage = crate::spell::roll_spell_damage(max_hit);
                    let died = $npc.take_damage(damage, current_time, Some(player_id));
                    let name = $npc.name();
                    tracing::info!(
                        "{} spell hits {} for {} damage (max: {}, HP: {})",
                        caster_name,
                        name,
                        damage,
                        max_hit,
                        $npc.hp
                    );
                    ($npc.hp, name, died, damage)
                }
            }};
        }

        let (target_hp, target_name, target_died, actual_damage) = if is_npc {
            if is_instance_npc {
                let inst_id = caster_instance.as_ref().unwrap();
                if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                    let mut npcs = instance.npcs.write().await;
                    if let Some(npc) = npcs.get_mut(&target_id) {
                        apply_spell_to_npc!(npc)
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            } else {
                let mut npcs = self.npcs.write().await;
                if let Some(npc) = npcs.get_mut(&target_id) {
                    apply_spell_to_npc!(npc)
                } else {
                    return;
                }
            }
        } else {
            let mut players = self.players.write().await;
            if let Some(target) = players.get_mut(&target_id) {
                if target.is_dead {
                    return;
                }
                if target.is_god_mode {
                    return;
                }

                let target_defence_level = target.skills.defence.level;
                let base_defence_bonus = target.defence_bonus(&self.item_registry);

                // Apply prayer bonuses to target's defence
                let target_active_ids: Vec<String> =
                    target.active_prayers.iter().cloned().collect();
                let target_prayer_effects =
                    self.prayer_registry.calculate_effects(&target_active_ids);
                let target_defence_bonus =
                    target_prayer_effects.apply_defence_bonus(base_defence_bonus);

                if !crate::skills::calculate_hit(
                    effective_level,
                    attack_bonus,
                    target_defence_level,
                    target_defence_bonus,
                ) {
                    // Miss
                    let name = target.name.clone();
                    tracing::info!(
                        "{} spell misses {} (eff {} [atk{}+mag{}] vs def {} + {})",
                        caster_name,
                        name,
                        effective_level,
                        attack_level,
                        magic_level,
                        target_defence_level,
                        target_defence_bonus
                    );
                    (target.hp, name, false, 0)
                } else {
                    // Hit
                    let max_hit =
                        crate::spell::calculate_spell_max_hit(magic_level, spell_def.base_power);
                    let raw_damage = crate::spell::roll_spell_damage(max_hit);
                    let damage = target_prayer_effects.apply_damage_reduction(raw_damage);
                    target.hp = (target.hp - damage).max(0);
                    let name = target.name.clone();
                    let died = target.hp <= 0;
                    if died {
                        target.die(current_time);
                    }
                    tracing::info!(
                        "{} spell hits {} for {} damage (max: {}, raw: {}, HP: {})",
                        caster_name,
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

        // 7. Broadcast SpellEffect to nearby players in the zone
        self.broadcast_to_zone(
            player_id,
            ServerMessage::SpellEffect {
                caster_id: player_id.to_string(),
                target_id: Some(target_id.clone()),
                spell_id: spell_def.id.to_string(),
                target_x,
                target_y,
            },
        )
        .await;

        // 8. Broadcast DamageEvent
        let damage_msg = ServerMessage::DamageEvent {
            source_id: player_id.to_string(),
            target_id: target_id.clone(),
            damage: actual_damage,
            target_hp,
            target_x: target_x as f32,
            target_y: target_y as f32,
            projectile: if spell_def.effect_sprite == "projectile" {
                Some(spell_def.id.to_string())
            } else {
                None
            },
        };
        self.broadcast_to_zone(player_id, damage_msg).await;

        // 8b. Apply pushback if the spell has it and the target was hit
        if spell_def.pushback_distance > 0 && actual_damage > 0 && !target_died {
            let dx = target_x - caster_x;
            let dy = target_y - caster_y;
            // Normalize direction (sign only)
            let dir_x = if dx != 0 { dx.signum() } else { 0 };
            let dir_y = if dy != 0 { dy.signum() } else { 0 };
            // If caster and target are on the same tile, push down as fallback
            let (dir_x, dir_y) = if dir_x == 0 && dir_y == 0 {
                (0, 1)
            } else {
                (dir_x, dir_y)
            };

            let mut final_x = target_x;
            let mut final_y = target_y;
            let mut blocked_tiles = 0;
            let mut wall_slam = false;

            for i in 1..=spell_def.pushback_distance {
                let next_x = target_x + dir_x * i;
                let next_y = target_y + dir_y * i;

                if !self.world.is_tile_walkable(next_x, next_y).await {
                    // Hit a wall - wall slam!
                    wall_slam = true;
                    blocked_tiles = spell_def.pushback_distance - (i - 1);
                    break;
                }
                final_x = next_x;
                final_y = next_y;
            }

            // Apply wall slam bonus damage
            let wall_slam_bonus = if wall_slam {
                blocked_tiles * spell_def.wall_slam_damage_per_tile
            } else {
                0
            };

            // Move the target to the final position
            if is_npc {
                if is_instance_npc {
                    let inst_id = caster_instance.as_ref().unwrap();
                    if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                        let mut npcs = instance.npcs.write().await;
                        if let Some(npc) = npcs.get_mut(&target_id) {
                            npc.x = final_x;
                            npc.y = final_y;
                            if wall_slam_bonus > 0 {
                                npc.take_damage(wall_slam_bonus, current_time, Some(player_id));
                            }
                        }
                    }
                } else {
                    let mut npcs = self.npcs.write().await;
                    if let Some(npc) = npcs.get_mut(&target_id) {
                        npc.x = final_x;
                        npc.y = final_y;
                        if wall_slam_bonus > 0 {
                            npc.take_damage(wall_slam_bonus, current_time, Some(player_id));
                        }
                    }
                }
            } else {
                let mut players = self.players.write().await;
                if let Some(target) = players.get_mut(&target_id) {
                    target.x = final_x;
                    target.y = final_y;
                    target.move_dx = 0;
                    target.move_dy = 0;
                    if wall_slam_bonus > 0 {
                        target.hp = (target.hp - wall_slam_bonus).max(0);
                    }
                }
            }

            // Send Pushback message
            self.broadcast_to_zone(
                player_id,
                ServerMessage::Pushback {
                    target_id: target_id.clone(),
                    from_x: target_x,
                    from_y: target_y,
                    to_x: final_x,
                    to_y: final_y,
                    wall_slam,
                    bonus_damage: wall_slam_bonus,
                },
            )
            .await;

            // Send DamageEvent for wall slam bonus
            if wall_slam_bonus > 0 {
                let slam_hp = if is_npc {
                    if is_instance_npc {
                        let inst_id = caster_instance.as_ref().unwrap();
                        if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                            let npcs = instance.npcs.read().await;
                            npcs.get(&target_id).map(|n| n.hp).unwrap_or(0)
                        } else {
                            0
                        }
                    } else {
                        let npcs = self.npcs.read().await;
                        npcs.get(&target_id).map(|n| n.hp).unwrap_or(0)
                    }
                } else {
                    let players = self.players.read().await;
                    players.get(&target_id).map(|p| p.hp).unwrap_or(0)
                };
                self.broadcast_to_zone(
                    player_id,
                    ServerMessage::DamageEvent {
                        source_id: player_id.to_string(),
                        target_id: target_id.clone(),
                        damage: wall_slam_bonus,
                        target_hp: slam_hp,
                        target_x: final_x as f32,
                        target_y: final_y as f32,
                        projectile: None,
                    },
                )
                .await;
            }
        }

        // 9. Award Magic XP and Hitpoints XP
        if actual_damage > 0 {
            let xp_results = {
                let mut players = self.players.write().await;
                if let Some(attacker) = players.get_mut(player_id) {
                    let magic_xp =
                        (actual_damage as f64 * crate::skills::MAGIC_XP_PER_DAMAGE) as i64;
                    let hp_xp =
                        (actual_damage as f64 * crate::skills::HITPOINTS_XP_PER_DAMAGE) as i64;

                    let mut results = Vec::new();

                    // Award Magic XP
                    let magic_leveled = attacker.skills.magic.add_xp(magic_xp);
                    results.push((
                        SkillType::Magic,
                        magic_xp,
                        attacker.skills.magic.xp,
                        attacker.skills.magic.level,
                        magic_leveled,
                    ));

                    // Award Hitpoints XP
                    let old_hp_level = attacker.skills.hitpoints.level;
                    let hp_leveled = attacker.skills.hitpoints.add_xp(hp_xp);
                    if hp_leveled {
                        let new_max = attacker.skills.hitpoints.level;
                        attacker.hp += new_max - old_hp_level;
                    }
                    results.push((
                        SkillType::Hitpoints,
                        hp_xp,
                        attacker.skills.hitpoints.xp,
                        attacker.skills.hitpoints.level,
                        hp_leveled,
                    ));

                    Some(results)
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

        // 10. Interrupt crafting if target is a player who took damage
        if !is_npc && actual_damage > 0 {
            self.cancel_crafting(&target_id, "interrupted").await;
        }

        // 11. Handle death
        if target_died {
            tracing::info!(
                "{} killed {} with spell {}",
                caster_name,
                target_name,
                spell_def.id
            );
            if is_npc {
                // Get NPC info for exp and loot
                let (prototype_id, npc_level) = if is_instance_npc {
                    let inst_id = caster_instance.as_ref().unwrap();
                    if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                        let npcs = instance.npcs.read().await;
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

                // Broadcast NPC death to nearby players.
                self.broadcast_to_zone(
                    player_id,
                    ServerMessage::NpcDied {
                        id: target_id.clone(),
                        killer_id: player_id.to_string(),
                    },
                )
                .await;

                // Persist monster kill count for stats leaderboards.
                self.record_monster_kill(player_id).await;

                // Process quest kill event
                self.process_quest_kill(player_id, &prototype_id).await;

                // Process slayer kill event
                self.process_slayer_kill(player_id, &prototype_id).await;

                // Skip loot drops in boss arena (rewards come from battle master)
                let in_boss_arena = caster_instance
                    .as_ref()
                    .map(|id| id.contains(crate::game::boss_tick::BOSS_MAP_ID))
                    .unwrap_or(false);

                // Spawn item drops from prototype loot table
                let drop_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                let killer_instance = {
                    let instances = self.player_instances.read().await;
                    instances.get(player_id).cloned()
                };

                let drops = if in_boss_arena {
                    vec![]
                } else if let Some(prototype) = self.entity_registry.get(&prototype_id) {
                    crate::entity::generate_loot_from_prototype(
                        prototype,
                        target_x as f32,
                        target_y as f32,
                        player_id,
                        drop_time,
                        npc_level,
                        killer_instance,
                    )
                } else {
                    vec![]
                };

                // Record collection log entries for monster drops (skip gold)
                for item in &drops {
                    if item.item_id != "gold" {
                        self.record_collection_entry(
                            player_id,
                            &item.item_id,
                            "monster_drops",
                            &prototype_id,
                        )
                        .await;
                    }
                }

                for item in drops {
                    let mut items = self.ground_items.write().await;

                    // For gold, try to combine with existing pile at same tile
                    if item.item_id == "gold" {
                        let tile_x = item.x.floor() as i32;
                        let tile_y = item.y.floor() as i32;

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
                            if let Some(existing) = items.get_mut(&existing_id) {
                                existing.quantity += item.quantity;
                                let update_msg = ServerMessage::ItemQuantityUpdated {
                                    id: existing_id.clone(),
                                    quantity: existing.quantity,
                                };
                                drop(items);
                                self.broadcast_to_zone(player_id, update_msg).await;
                            }
                            continue;
                        }
                    }

                    let drop_msg = ServerMessage::ItemDropped {
                        id: item.id.clone(),
                        item_id: item.item_id.clone(),
                        x: item.x,
                        y: item.y,
                        quantity: item.quantity,
                    };
                    items.insert(item.id.clone(), item);
                    drop(items);
                    self.broadcast_to_zone(player_id, drop_msg).await;
                }
            } else {
                // Player death from spell
                let arena_death = {
                    let arena = self.arena_manager.read().await;
                    arena.is_fighting() && arena.is_in_ring(&target_id)
                };

                if arena_death {
                    // Arena death handling
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
                        let end_time = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64;
                        let placements = {
                            let mut arena = self.arena_manager.write().await;
                            arena.end_match(end_time)
                        };

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

                        // Teleport all fighters to spectator spawn
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
                    self.broadcast_to_zone(
                        player_id,
                        ServerMessage::PlayerDied {
                            id: target_id.clone(),
                            killer_id: player_id.to_string(),
                        },
                    )
                    .await;

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
                            active_prayers: vec![],
                        },
                    )
                    .await;
                }
            }
        }
    }

    async fn cast_heal_spell_resolved(
        &self,
        player_id: &str,
        spell_def: &ResolvedSpell,
        current_time: u64,
    ) {
        // Get caster info
        let (caster_x, caster_y, caster_direction, magic_level, current_hp, max_hp) = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) if !p.is_dead && p.active => p,
                _ => return,
            };
            (
                player.x,
                player.y,
                player.direction,
                player.skills.magic.level,
                player.hp,
                player.max_hp(),
            )
        };

        // 1. Deduct mana and set cooldown
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.mp -= spell_def.mana_cost;
                player
                    .spell_cooldowns
                    .insert(spell_def.id.to_string(), current_time);
            }
        }

        // 2. Calculate heal amount
        let heal_amount = crate::spell::calculate_heal_amount(magic_level, spell_def.base_power);
        let actual_heal = heal_amount.min(max_hp - current_hp); // Clamp to not exceed max HP

        // 3. Apply heal
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.hp = (player.hp + heal_amount).min(player.max_hp());
            }
        }

        // 4. Broadcast SpellEffect (target_id = None, target position = caster position)
        self.broadcast_to_zone(
            player_id,
            ServerMessage::SpellEffect {
                caster_id: player_id.to_string(),
                target_id: None,
                spell_id: spell_def.id.to_string(),
                target_x: caster_x,
                target_y: caster_y,
            },
        )
        .await;

        // 5. Broadcast casting animation
        self.broadcast_to_zone(
            player_id,
            ServerMessage::PlayerAttack {
                player_id: player_id.to_string(),
                attack_type: "spell".to_string(),
                direction: caster_direction as u8,
            },
        )
        .await;

        // 6. Award Magic XP based on amount healed
        if actual_heal > 0 {
            let xp_results = {
                let mut players = self.players.write().await;
                if let Some(caster) = players.get_mut(player_id) {
                    let magic_xp = (actual_heal as f64 * crate::skills::MAGIC_XP_PER_HEAL) as i64;

                    let magic_leveled = caster.skills.magic.add_xp(magic_xp);
                    Some((
                        SkillType::Magic,
                        magic_xp,
                        caster.skills.magic.xp,
                        caster.skills.magic.level,
                        magic_leveled,
                    ))
                } else {
                    None
                }
            };

            if let Some((skill_type, xp_gained, total_xp, level, leveled_up)) = xp_results {
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
                    self.process_quest_progression_snapshot(player_id).await;
                }
            }
        }

        tracing::info!(
            "Player {} healed for {} HP with spell {}",
            player_id,
            actual_heal,
            spell_def.id
        );
    }

    pub async fn cast_return_home_spell(
        &self,
        player_id: &str,
        spell_def: &crate::spell::SpellDef,
        current_time: u64,
    ) -> bool {
        // Validate and set cooldown under write lock
        {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) if !p.is_dead && p.active => p,
                _ => return false,
            };

            // Check cooldown
            if let Some(&last_cast) = player.spell_cooldowns.get(spell_def.id)
                && current_time < last_cast + spell_def.cooldown_ms
            {
                drop(players);
                self.send_to_player(
                    player_id,
                    ServerMessage::SpellResult {
                        success: false,
                        reason: Some("Spell on cooldown".to_string()),
                        spell_id: Some(spell_def.id.to_string()),
                    },
                )
                .await;
                return false;
            }

            // Set cooldown and move player to spawn
            player
                .spell_cooldowns
                .insert(spell_def.id.to_string(), current_time);
            player.x = WORLD_SPAWN_X;
            player.y = WORLD_SPAWN_Y;
        }

        // Send success result
        self.send_to_player(
            player_id,
            ServerMessage::SpellResult {
                success: true,
                reason: None,
                spell_id: Some(spell_def.id.to_string()),
            },
        )
        .await;

        // Send spell effect to the player
        self.send_to_player(
            player_id,
            ServerMessage::SpellEffect {
                caster_id: player_id.to_string(),
                target_id: None,
                spell_id: spell_def.id.to_string(),
                target_x: WORLD_SPAWN_X,
                target_y: WORLD_SPAWN_Y,
            },
        )
        .await;

        tracing::info!(
            "Player {} cast Return Home, teleporting to ({}, {})",
            player_id,
            WORLD_SPAWN_X,
            WORLD_SPAWN_Y
        );
        true
    }
}
