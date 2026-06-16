use super::*;

impl GameRoom {
    pub(super) async fn process_auto_action_tick(&self, current_time: u64) {
        // Collect players with active auto-actions
        let auto_action_players: Vec<(String, AutoAction)> = {
            let players = self.players.read().await;
            players
                .iter()
                .filter_map(|(id, p)| {
                    if p.active && !p.is_dead {
                        p.auto_action.as_ref().map(|a| (id.clone(), a.clone()))
                    } else {
                        None
                    }
                })
                .collect()
        };

        for (pid, auto_action) in auto_action_players {
            match (&auto_action.target, &auto_action.action) {
                (AutoActionTarget::Npc { npc_id }, AutoActionType::Attack) => {
                    let player_inst = self.player_instances.read().await.get(&pid).cloned();

                    // Validate NPC target is still alive (check instance or overworld)
                    let (npc_alive, npc_pos) = if let Some(ref inst_id) = player_inst {
                        if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                            let npcs = instance.npcs.read().await;
                            npcs.get(npc_id).map_or((false, None), |n| {
                                (
                                    n.is_alive(),
                                    Some((n.x, n.y, n.stats.size, n.tile_offset())),
                                )
                            })
                        } else {
                            (false, None)
                        }
                    } else {
                        let npcs = self.npcs.read().await;
                        npcs.get(npc_id).map_or((false, None), |n| {
                            (
                                n.is_alive(),
                                Some((n.x, n.y, n.stats.size, n.tile_offset())),
                            )
                        })
                    };

                    if !npc_alive {
                        self.clear_auto_action(&pid, "target_dead").await;
                        continue;
                    }

                    // Check if in range, cooldown ready, and standing still
                    let (in_range, cooldown_ready, is_still) =
                        if let Some((npc_x, npc_y, npc_size, (npc_off_x, npc_off_y))) = npc_pos {
                            let players = self.players.read().await;
                            if let Some(player) = players.get(&pid) {
                                let closest_x = player
                                    .x
                                    .clamp(npc_x + npc_off_x, npc_x + npc_off_x + npc_size - 1);
                                let closest_y = player
                                    .y
                                    .clamp(npc_y + npc_off_y, npc_y + npc_off_y + npc_size - 1);
                                let dx = (player.x - closest_x).abs();
                                let dy = (player.y - closest_y).abs();
                                let (weapon_range, weapon_is_ranged) =
                                    if let Some(ref weapon_id) = player.equipped_weapon {
                                        if let Some(item_def) = self.item_registry.get(weapon_id) {
                                            item_def.equipment.as_ref().map_or((1, false), |e| {
                                                (e.range, e.weapon_type == WeaponType::Ranged)
                                            })
                                        } else {
                                            (1, false)
                                        }
                                    } else {
                                        (1, false)
                                    };
                                let in_range = if weapon_range == 1 {
                                    (dx + dy) == 1
                                } else {
                                    (dx + dy) <= weapon_range && (dx > 0 || dy > 0)
                                };
                                let cd = if weapon_is_ranged {
                                    RANGED_ATTACK_COOLDOWN_MS
                                } else {
                                    ATTACK_COOLDOWN_MS
                                };
                                let cooldown_ready = current_time - player.last_attack_time >= cd;
                                let is_still = player.move_dx == 0
                                    && player.move_dy == 0
                                    && player.pending_move_seq.is_none()
                                    && current_time.saturating_sub(player.last_move_input_ms)
                                        >= 500;
                                (in_range, cooldown_ready, is_still)
                            } else {
                                (false, false, false)
                            }
                        } else {
                            (false, false, false)
                        };

                    if in_range && cooldown_ready && is_still {
                        // Compute facing direction toward NPC target
                        let face_dir =
                            if let Some((npc_x, npc_y, npc_size, (npc_off_x, npc_off_y))) = npc_pos
                            {
                                let players = self.players.read().await;
                                if let Some(player) = players.get(&pid) {
                                    let closest_x = player
                                        .x
                                        .clamp(npc_x + npc_off_x, npc_x + npc_off_x + npc_size - 1);
                                    let closest_y = player
                                        .y
                                        .clamp(npc_y + npc_off_y, npc_y + npc_off_y + npc_size - 1);
                                    let dx = closest_x - player.x;
                                    let dy = closest_y - player.y;
                                    Some(direction_from_delta(dx, dy))
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                        self.handle_attack(&pid, face_dir, Some(npc_id)).await;
                    }
                }

                (
                    AutoActionTarget::Player {
                        player_id: target_pid,
                    },
                    AutoActionType::Attack,
                ) => {
                    // Validate player target is still alive and connected
                    let target_valid = {
                        let players = self.players.read().await;
                        players
                            .get(target_pid.as_str())
                            .is_some_and(|p| p.active && !p.is_dead)
                    };
                    if !target_valid {
                        self.clear_auto_action(&pid, "target_dead").await;
                        continue;
                    }

                    // Check same instance context
                    let (same_context, _attacker_instance) = {
                        let instances = self.player_instances.read().await;
                        let ai = instances.get(pid.as_str()).cloned();
                        let ti = instances.get(target_pid.as_str()).cloned();
                        (ai == ti, ai)
                    };
                    if !same_context {
                        self.clear_auto_action(&pid, "interrupted").await;
                        continue;
                    }

                    // Stop PvP auto-action if attacker is not in a PvP-allowed area
                    {
                        let attacker_pos = {
                            let players = self.players.read().await;
                            players.get(pid.as_str()).map(|p| (p.x, p.y))
                        };
                        if let Some((ax, ay)) = attacker_pos
                            && !self.is_pvp_allowed(&pid, ax, ay).await
                        {
                            self.clear_auto_action(&pid, "interrupted").await;
                            continue;
                        }
                    }

                    // Check range, cooldown, and standing still
                    let (in_range, cooldown_ready, is_still) = {
                        let players = self.players.read().await;
                        if let (Some(attacker), Some(target)) =
                            (players.get(&pid), players.get(target_pid.as_str()))
                        {
                            let dx = (attacker.x - target.x).abs();
                            let dy = (attacker.y - target.y).abs();
                            let (weapon_range, weapon_is_ranged) =
                                if let Some(ref weapon_id) = attacker.equipped_weapon {
                                    if let Some(item_def) = self.item_registry.get(weapon_id) {
                                        item_def.equipment.as_ref().map_or((1, false), |e| {
                                            (e.range, e.weapon_type == WeaponType::Ranged)
                                        })
                                    } else {
                                        (1, false)
                                    }
                                } else {
                                    (1, false)
                                };
                            // Manhattan distance for all ranges (diamond shape)
                            let in_range = if weapon_range == 1 {
                                (dx + dy) == 1
                            } else {
                                (dx + dy) <= weapon_range && (dx > 0 || dy > 0)
                            };
                            let cd = if weapon_is_ranged {
                                RANGED_ATTACK_COOLDOWN_MS
                            } else {
                                ATTACK_COOLDOWN_MS
                            };
                            let cooldown_ready = current_time - attacker.last_attack_time >= cd;
                            let is_still = attacker.move_dx == 0
                                && attacker.move_dy == 0
                                && attacker.pending_move_seq.is_none()
                                && current_time.saturating_sub(attacker.last_move_input_ms) >= 500;
                            (in_range, cooldown_ready, is_still)
                        } else {
                            (false, false, false)
                        }
                    };

                    if in_range && cooldown_ready && is_still {
                        // Compute facing direction toward player target
                        let face_dir = {
                            let players = self.players.read().await;
                            match (players.get(&pid), players.get(target_pid.as_str())) {
                                (Some(attacker), Some(target)) => {
                                    let dx = target.x - attacker.x;
                                    let dy = target.y - attacker.y;
                                    Some(direction_from_delta(dx, dy))
                                }
                                _ => None,
                            }
                        };
                        // Direction override is applied atomically inside handle_attack
                        self.handle_attack(&pid, face_dir, Some(target_pid)).await;
                    }
                }

                (AutoActionTarget::Resource { x, y, gid }, AutoActionType::Mine) => {
                    // Check if rock is depleted
                    let player_inst = self.player_instances.read().await.get(&pid).cloned();
                    let is_depleted = {
                        let mining = self.mining.read().await;
                        mining.is_rock_depleted(player_inst.as_deref(), *x, *y)
                    };
                    if is_depleted {
                        self.clear_auto_action(&pid, "target_depleted").await;
                        continue;
                    }

                    // Check cardinal adjacency, cooldown, and inventory space
                    let (adjacent, cooldown_ready, inventory_full) = {
                        let mining = self.mining.read().await;
                        let ore_item_id = mining.get_ore_type(*gid).map(|c| c.ore_item_id.clone());
                        let players = self.players.read().await;
                        if let Some(player) = players.get(&pid) {
                            let dx = (player.x - x).abs();
                            let dy = (player.y - y).abs();
                            let adjacent = (dx + dy) == 1;
                            let cooldown_ready =
                                current_time - player.last_attack_time >= ATTACK_COOLDOWN_MS;
                            let inventory_full = if let Some(ref item_id) = ore_item_id {
                                !player
                                    .inventory
                                    .has_space_for(item_id, 1, &self.item_registry)
                            } else {
                                false
                            };
                            (adjacent, cooldown_ready, inventory_full)
                        } else {
                            (false, false, false)
                        }
                    };

                    if inventory_full {
                        self.clear_auto_action(&pid, "inventory_full").await;
                        continue;
                    }

                    if adjacent && cooldown_ready {
                        // Auto-face toward resource
                        {
                            let mut players = self.players.write().await;
                            if let Some(player) = players.get_mut(&pid) {
                                let ddx = x - player.x;
                                let ddy = y - player.y;
                                player.direction = direction_from_delta(ddx, ddy);
                            }
                        }
                        self.handle_mine_rock(&pid, *x, *y, *gid).await;
                    }
                }

                (AutoActionTarget::Resource { x, y, gid }, AutoActionType::Chop) => {
                    // Check if tree is depleted
                    let player_inst = self.player_instances.read().await.get(&pid).cloned();
                    let is_depleted = {
                        let woodcutting = self.woodcutting.read().await;
                        woodcutting.is_tree_depleted(player_inst.as_deref(), *x, *y)
                    };
                    if is_depleted {
                        self.clear_auto_action(&pid, "target_depleted").await;
                        continue;
                    }

                    // Check cardinal adjacency, cooldown, and inventory space
                    let (adjacent, cooldown_ready, inventory_full) = {
                        let woodcutting = self.woodcutting.read().await;
                        let log_item_id = woodcutting
                            .get_tree_type(*gid)
                            .map(|c| c.log_item_id.clone());
                        let players = self.players.read().await;
                        if let Some(player) = players.get(&pid) {
                            let dx = (player.x - x).abs();
                            let dy = (player.y - y).abs();
                            let adjacent = (dx + dy) == 1;
                            let cooldown_ready =
                                current_time - player.last_attack_time >= ATTACK_COOLDOWN_MS;
                            let inventory_full = if let Some(ref item_id) = log_item_id {
                                !player
                                    .inventory
                                    .has_space_for(item_id, 1, &self.item_registry)
                            } else {
                                false
                            };
                            (adjacent, cooldown_ready, inventory_full)
                        } else {
                            (false, false, false)
                        }
                    };

                    if inventory_full {
                        self.clear_auto_action(&pid, "inventory_full").await;
                        continue;
                    }

                    if adjacent && cooldown_ready {
                        // Auto-face toward resource
                        {
                            let mut players = self.players.write().await;
                            if let Some(player) = players.get_mut(&pid) {
                                let ddx = x - player.x;
                                let ddy = y - player.y;
                                player.direction = direction_from_delta(ddx, ddy);
                            }
                        }
                        self.handle_chop_tree(&pid, *x, *y, *gid).await;
                    }
                }

                (AutoActionTarget::FarmTree { patch_id }, AutoActionType::Chop) => {
                    // Validate the patch is still a mature, healthy tree.
                    let footprint = {
                        let farming = self.farming.read().await;
                        let key = (patch_id.clone(), pid.clone());
                        let is_tree = farming
                            .patches
                            .get(patch_id)
                            .map(|p| p.patch_type == "tree")
                            .unwrap_or(false);
                        let mature = farming
                            .player_states
                            .get(&key)
                            .map(|s| s.is_harvestable(&farming.crops, current_time))
                            .unwrap_or(false);
                        if is_tree && mature {
                            farming
                                .patches
                                .get(patch_id)
                                .map(|p| (p.x, p.y, p.width as i32, p.height as i32))
                        } else {
                            None
                        }
                    };
                    let Some((px0, py0, w, h)) = footprint else {
                        self.clear_auto_action(&pid, "target_depleted").await;
                        continue;
                    };

                    let (adjacent, cooldown_ready, face_x, face_y) = {
                        let players = self.players.read().await;
                        if let Some(player) = players.get(&pid) {
                            // Nearest footprint tile, then cardinal-adjacency to it.
                            let cx = player.x.clamp(px0, px0 + w - 1);
                            let cy = player.y.clamp(py0, py0 + h - 1);
                            let adjacent =
                                (player.x - cx).abs() + (player.y - cy).abs() == 1;
                            let cooldown_ready =
                                current_time - player.last_attack_time >= ATTACK_COOLDOWN_MS;
                            (adjacent, cooldown_ready, cx, cy)
                        } else {
                            (false, false, px0, py0)
                        }
                    };

                    if adjacent && cooldown_ready {
                        {
                            let mut players = self.players.write().await;
                            if let Some(player) = players.get_mut(&pid) {
                                let ddx = face_x - player.x;
                                let ddy = face_y - player.y;
                                player.direction = direction_from_delta(ddx, ddy);
                            }
                        }
                        self.handle_chop_farm_tree(&pid, patch_id).await;
                    }
                }

                // Invalid combinations (e.g. Attack on Resource) — just clear
                _ => {
                    self.clear_auto_action(&pid, "interrupted").await;
                }
            }
        }
    }
}
