use super::*;

impl GameRoom {
    pub async fn handle_move(&self, player_id: &str, dx: f32, dy: f32, seq: Option<u32>) {
        // NOTE: Movement does NOT cancel auto-action. The client sends an explicit
        // CancelAutoAction message when the player manually moves (keyboard/dpad),
        // manually attacks, or clicks empty ground. Chase-follow movements must NOT
        // interrupt auto-action, otherwise the player can never catch a moving target.

        let mut chair_to_free: Option<(i32, i32)> = None;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                // Reset auto-retaliate idle timer on any movement input
                player.last_activity_time = now_ms;

                let move_seq =
                    seq.unwrap_or_else(|| player.last_received_move_seq.saturating_add(1));
                let prev_seq = player.last_received_move_seq;
                if move_seq <= player.last_received_move_seq {
                    self.movement_anomalies
                        .stale_packets_ignored
                        .fetch_add(1, Ordering::Relaxed);
                    // Out-of-order/duplicate move packet. This is especially useful when
                    // diagnosing "continued movement after key-up" reports.
                    if now_ms.saturating_sub(player.last_move_input_warn_ms)
                        >= MOVE_INPUT_WARN_THROTTLE_MS
                    {
                        let is_stop = dx.abs() <= 0.1 && dy.abs() <= 0.1;
                        tracing::warn!(
                            "Ignoring stale move packet for {} (seq={} <= last={}, stop={}, intent=({}, {}) pos=({}, {}))",
                            player_id,
                            move_seq,
                            player.last_received_move_seq,
                            is_stop,
                            player.move_dx,
                            player.move_dy,
                            player.x,
                            player.y
                        );
                        player.last_move_input_warn_ms = now_ms;
                    }
                    return;
                }
                player.last_received_move_seq = move_seq;

                // Detect missing move packets from client/network path.
                let seq_gap = move_seq.saturating_sub(prev_seq);
                if seq_gap > 4 {
                    self.movement_anomalies
                        .seq_gap_events
                        .fetch_add(1, Ordering::Relaxed);
                    if now_ms.saturating_sub(player.last_move_input_warn_ms)
                        >= MOVE_INPUT_WARN_THROTTLE_MS
                    {
                        tracing::warn!(
                            "Move seq gap {} for {} (prev={} recv={} pos=({}, {}) intent=({}, {}))",
                            seq_gap,
                            player_id,
                            prev_seq,
                            move_seq,
                            player.x,
                            player.y,
                            player.move_dx,
                            player.move_dy
                        );
                        player.last_move_input_warn_ms = now_ms;
                    }
                }

                // Track movement-input cadence for diagnostics.
                if player.last_move_input_ms > 0 && (player.move_dx != 0 || player.move_dy != 0) {
                    let gap_ms = now_ms.saturating_sub(player.last_move_input_ms);
                    if gap_ms > MOVE_INPUT_GAP_WARN_MS {
                        self.movement_anomalies
                            .input_gap_events
                            .fetch_add(1, Ordering::Relaxed);
                        if now_ms.saturating_sub(player.last_move_input_warn_ms)
                            >= MOVE_INPUT_WARN_THROTTLE_MS
                        {
                            tracing::warn!(
                                "Move input gap {}ms for {} (seq={} pos=({}, {}) intent=({}, {}))",
                                gap_ms,
                                player_id,
                                move_seq,
                                player.x,
                                player.y,
                                player.move_dx,
                                player.move_dy
                            );
                            player.last_move_input_warn_ms = now_ms;
                        }
                    }
                }
                player.last_move_input_ms = now_ms;

                // Block movement while stall is active
                if player.stall.as_ref().map_or(false, |s| s.active) {
                    return;
                }

                // Auto-stand when trying to move while sitting (only in chair facing direction)
                if let Some(pos) = player.sitting_at {
                    // Determine intended movement direction
                    let move_dir = if dx.abs() > dy.abs() {
                        if dx > 0.1 {
                            Some(Direction::Right)
                        } else if dx < -0.1 {
                            Some(Direction::Left)
                        } else {
                            None
                        }
                    } else if dy.abs() > 0.1 {
                        if dy > 0.1 {
                            Some(Direction::Down)
                        } else {
                            Some(Direction::Up)
                        }
                    } else {
                        None
                    };

                    // Only allow standing up when moving in the chair's facing direction
                    if move_dir == Some(player.direction) {
                        let (fdx, fdy) = match player.direction {
                            Direction::Up => (0, -1),
                            Direction::Down => (0, 1),
                            Direction::Left => (-1, 0),
                            Direction::Right => (1, 0),
                            _ => (0, 0),
                        };
                        player.x = pos.0 + fdx;
                        player.y = pos.1 + fdy;
                        player.sitting_at = None;
                        chair_to_free = Some(pos);
                    }
                    // Either way, don't process actual movement while sitting
                    player.mark_move_seq_processed(move_seq);
                    player.clear_move_intent();
                } else {
                    // Convert to grid movement (-1, 0, or 1)
                    // Supports diagonal movement (both axes non-zero)
                    let move_dx = if dx > 0.1 {
                        1
                    } else if dx < -0.1 {
                        -1
                    } else {
                        0
                    };
                    let move_dy = if dy > 0.1 {
                        1
                    } else if dy < -0.1 {
                        -1
                    } else {
                        0
                    };

                    // Queue movement intent only. Facing updates when a move is
                    // actually applied in the tick loop.
                    if move_dx == 0 && move_dy == 0 {
                        // Stop intent: clear everything including last-move vel.
                        player.mark_move_seq_processed(move_seq);
                        player.stop_moving();
                    } else {
                        player.move_dx = move_dx;
                        player.move_dy = move_dy;
                        player.pending_move_seq = Some(move_seq);
                    }
                }
            }
        }
        // Free chair outside of players lock
        if let Some((tx, ty)) = chair_to_free {
            let mut chairs = self.chairs.write().await;
            if let Some(chair) = chairs.get_mut(&(tx, ty)) {
                if chair.occupied_by.as_deref() == Some(player_id) {
                    chair.occupied_by = None;
                }
            }
        }

        // Stop gathering immediately when movement starts (don't wait for arrival)
        if dx.abs() > 0.1 || dy.abs() > 0.1 {
            self.handle_stop_gathering(player_id).await;
        }

        // Close chest if player moved
        self.close_player_chest(player_id).await;
    }

    pub async fn handle_dash(&self, player_id: &str) {
        let current_tick = *self.tick.read().await;

        // Get player state and validate
        let (px, py, direction, last_dash_tick, is_sitting, is_dead, active) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) => (
                    p.x,
                    p.y,
                    p.direction,
                    p.last_dash_tick,
                    p.sitting_at.is_some(),
                    p.is_dead,
                    p.active,
                ),
                None => return,
            }
        };

        if !active || is_dead || is_sitting {
            return;
        }

        // Check cooldown
        if current_tick.saturating_sub(last_dash_tick) < DASH_COOLDOWN_TICKS {
            return;
        }

        // Get direction vector
        let (dx, dy) = match direction {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
            _ => return,
        };

        // Snapshot collision data
        let player_inst = self.player_instances.read().await.get(player_id).cloned();
        let is_overworld = player_inst.is_none();

        let (overworld_player_pos, npc_positions, chair_positions) = {
            let players = self.players.read().await;
            let instances = self.player_instances.read().await;
            let overworld: std::collections::HashSet<(i32, i32)> = players
                .values()
                .filter(|p| {
                    p.active && !p.is_dead && p.id != player_id && !instances.contains_key(&p.id)
                })
                .map(|p| (p.x, p.y))
                .collect();
            drop(instances);
            drop(players);

            let npcs = self.npcs.read().await;
            let npc_pos: std::collections::HashSet<(i32, i32)> = npcs
                .values()
                .filter(|n| n.is_alive())
                .flat_map(|n| n.occupied_tiles())
                .collect();
            drop(npcs);

            let chairs = self.chairs.read().await;
            let chair_pos: std::collections::HashSet<(i32, i32)> = chairs.keys().cloned().collect();
            drop(chairs);

            (overworld, npc_pos, chair_pos)
        };

        // Snapshot instance collision data if player is in an instance
        let (inst_collision, inst_width, inst_height, inst_npc_pos, inst_player_pos) =
            if let Some(ref inst_id) = player_inst {
                if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                    let collision = instance.collision.read().await.clone();
                    let width = instance.map_width;
                    let height = instance.map_height;

                    let npcs = instance.npcs.read().await;
                    let npc_pos: std::collections::HashSet<(i32, i32)> = npcs
                        .values()
                        .filter(|n| n.is_alive())
                        .flat_map(|n| n.occupied_tiles())
                        .collect();

                    let players = self.players.read().await;
                    let instances = self.player_instances.read().await;
                    let player_pos: std::collections::HashSet<(i32, i32)> = players
                        .values()
                        .filter(|p| {
                            p.active
                                && !p.is_dead
                                && p.id != player_id
                                && instances.get(&p.id).map(|i| i.as_str())
                                    == Some(inst_id.as_str())
                        })
                        .map(|p| (p.x, p.y))
                        .collect();

                    (
                        Some(collision),
                        width,
                        height,
                        Some(npc_pos),
                        Some(player_pos),
                    )
                } else {
                    (None, 0, 0, None, None)
                }
            } else {
                (None, 0, 0, None, None)
            };

        // Walk up to DASH_DISTANCE tiles, stopping at first collision
        let chunks_guard = self.world.chunks_read().await;
        let mut final_x = px;
        let mut final_y = py;

        for step in 1..=DASH_DISTANCE {
            let check_x = px + dx * step;
            let check_y = py + dy * step;

            if is_overworld {
                // Check tile walkability
                let coord = crate::chunk::ChunkCoord::from_world(check_x, check_y);
                let walkable = if let Some(chunk) = chunks_guard.get(&coord) {
                    let (lx, ly) = crate::chunk::world_to_local(check_x, check_y);
                    chunk.is_walkable_local(lx, ly)
                } else {
                    false
                };
                if !walkable {
                    break;
                }

                if overworld_player_pos.contains(&(check_x, check_y)) {
                    break;
                }
                if npc_positions.contains(&(check_x, check_y)) {
                    break;
                }
                if chair_positions.contains(&(check_x, check_y)) {
                    break;
                }
            } else {
                // Instance collision checks
                if let Some(ref collision) = inst_collision {
                    if check_x < 0
                        || check_y < 0
                        || check_x >= inst_width as i32
                        || check_y >= inst_height as i32
                    {
                        break;
                    }
                    let idx = (check_y as u32 * inst_width + check_x as u32) as usize;
                    if collision.get(idx).copied().unwrap_or(true) {
                        break;
                    }
                } else {
                    break; // Instance not found - stop dash for safety
                }
                if let Some(ref player_pos) = inst_player_pos {
                    if player_pos.contains(&(check_x, check_y)) {
                        break;
                    }
                }
                if let Some(ref npc_pos) = inst_npc_pos {
                    if npc_pos.contains(&(check_x, check_y)) {
                        break;
                    }
                }
            }

            final_x = check_x;
            final_y = check_y;
        }
        drop(chunks_guard);

        // Only dash if we can move at least 1 tile
        if final_x == px && final_y == py {
            return;
        }

        // Apply the dash
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.x = final_x;
                player.y = final_y;
                player.last_dash_tick = current_tick;
                player.last_move_tick = current_tick;
                player.is_dashing = true;
                player.reject_pending_move();
            }
        }
    }

    pub async fn handle_jump(&self, player_id: &str) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            if player.grounded && !player.is_dead && player.active {
                player.grounded = false;
                player.jump_ticks = 6; // 6 ticks = 300ms airtime at 20Hz
            }
        }
    }

    pub async fn handle_face(&self, player_id: &str, direction: u8) {
        let (player_x, player_y, face_dir) = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                // Don't allow direction changes while sitting
                if player.sitting_at.is_some() {
                    return;
                }
                let face_dir = Direction::from_u8(direction).to_cardinal();
                player.direction = face_dir;
                // Ensure player is not moving when just facing
                player.reject_pending_move();
                (player.x, player.y, face_dir)
            } else {
                tracing::warn!("handle_face: player not found: {}", player_id);
                return;
            }
        };

        // Determine gathering interruption outside players lock.
        let should_stop_gathering = {
            let gathering = self.gathering.read().await;
            if !gathering.is_gathering(player_id) {
                false
            } else {
                let (fdx, fdy): (i32, i32) = match face_dir {
                    Direction::Down => (0, 1),
                    Direction::Up => (0, -1),
                    Direction::Left => (-1, 0),
                    Direction::Right => (1, 0),
                    Direction::DownLeft => (-1, 1),
                    Direction::DownRight => (1, 1),
                    Direction::UpLeft => (-1, -1),
                    Direction::UpRight => (1, -1),
                };
                let face_x = player_x + fdx;
                let face_y = player_y + fdy;
                let facing_marker = gathering
                    .markers
                    .iter()
                    .any(|m| m.x == face_x && m.y == face_y);
                !facing_marker
            }
        };

        if should_stop_gathering {
            self.handle_stop_gathering(player_id).await;
        }
    }
}
