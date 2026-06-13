use super::*;

impl InputHandler {
    pub(super) fn handle_manual_movement(
        &mut self,
        state: &mut GameState,
        mode: &GameplayMode,
        frame: ProcessFrame<'_>,
        commands: &mut Vec<InputCommand>,
    ) -> MovementInput {
        let chat_consuming_keyboard = mode.chat_consuming_keyboard;
        let classic = mode.classic;
        let current_time = frame.current_time;
        // When chat is consuming keyboard, suppress all movement keys and hotkeys
        // but still allow mouse clicks (attack, auto-path, etc.) to fall through
        let (up, down, left, right) = if chat_consuming_keyboard {
            (false, false, false, false)
        } else {
            // Read which keys are held (in classic mode, only arrow keys - WASD goes to chat)
            (
                if classic {
                    is_key_down(KeyCode::Up)
                } else {
                    is_key_down(KeyCode::W) || is_key_down(KeyCode::Up)
                },
                if classic {
                    is_key_down(KeyCode::Down)
                } else {
                    is_key_down(KeyCode::S) || is_key_down(KeyCode::Down)
                },
                if classic {
                    is_key_down(KeyCode::Left)
                } else {
                    is_key_down(KeyCode::A) || is_key_down(KeyCode::Left)
                },
                if classic {
                    is_key_down(KeyCode::Right)
                } else {
                    is_key_down(KeyCode::D) || is_key_down(KeyCode::Right)
                },
            )
        };

        // Check for newly pressed keys this frame (last-key-wins priority)
        let (up_just, down_just, left_just, right_just) = if chat_consuming_keyboard {
            (false, false, false, false)
        } else {
            (
                if classic {
                    is_key_pressed(KeyCode::Up)
                } else {
                    is_key_pressed(KeyCode::W) || is_key_pressed(KeyCode::Up)
                },
                if classic {
                    is_key_pressed(KeyCode::Down)
                } else {
                    is_key_pressed(KeyCode::S) || is_key_pressed(KeyCode::Down)
                },
                if classic {
                    is_key_pressed(KeyCode::Left)
                } else {
                    is_key_pressed(KeyCode::A) || is_key_pressed(KeyCode::Left)
                },
                if classic {
                    is_key_pressed(KeyCode::Right)
                } else {
                    is_key_pressed(KeyCode::D) || is_key_pressed(KeyCode::Right)
                },
            )
        };

        // Get touch D-pad input (for mobile)
        use crate::input::touch::DPadDirection;
        let dpad_dir = self.touch_controls.get_direction();
        let dpad_released = self.touch_controls.get_just_released_direction();
        let has_dpad_input = dpad_dir != DPadDirection::None;

        // Cancel auto-path if any movement input (keyboard or D-pad)
        if up || down || left || right || has_dpad_input {
            state.clear_auto_path();
            self.reset_auto_path_motion_state();
        }

        // Determine new direction from keyboard - last key pressed wins
        let keyboard_dir = MoveDir::from_keys(
            up,
            down,
            left,
            right,
            up_just,
            down_just,
            left_just,
            right_just,
            self.prev_dir,
        );

        // Combine keyboard and D-pad: D-pad takes priority if active
        let new_dir = if has_dpad_input {
            match dpad_dir {
                DPadDirection::Up => MoveDir::Up,
                DPadDirection::Down => MoveDir::Down,
                DPadDirection::Left => MoveDir::Left,
                DPadDirection::Right => MoveDir::Right,
                // Map diagonals to the axis that differs from prev (cardinal only)
                DPadDirection::UpLeft
                | DPadDirection::UpRight
                | DPadDirection::DownLeft
                | DPadDirection::DownRight => {
                    let (dx, dy) = match dpad_dir {
                        DPadDirection::UpLeft => (true, true),    // left=true, up=true
                        DPadDirection::UpRight => (false, true),  // right, up
                        DPadDirection::DownLeft => (true, false), // left, down
                        DPadDirection::DownRight => (false, false), // right, down
                        _ => unreachable!(),
                    };
                    let prev_is_vertical = matches!(self.prev_dir, MoveDir::Up | MoveDir::Down);
                    if prev_is_vertical {
                        if dx {
                            MoveDir::Left
                        } else {
                            MoveDir::Right
                        }
                    } else if dy {
                        MoveDir::Up
                    } else {
                        MoveDir::Down
                    }
                }
                DPadDirection::None => keyboard_dir,
            }
        } else {
            keyboard_dir
        };

        // Detect direction changes for face vs move logic (keyboard only - D-pad has its own tracking)
        let dir_changed = keyboard_dir != self.prev_dir;

        // Handle keyboard direction key press/release for face vs move
        if dir_changed && !has_dpad_input {
            if keyboard_dir != MoveDir::None && self.prev_dir == MoveDir::None {
                // New direction pressed - record time
                self.dir_press_time = current_time;
                self.move_sent = false;
                self.player_blocked_since = None;
            } else if keyboard_dir == MoveDir::None && self.prev_dir != MoveDir::None {
                self.player_blocked_since = None;
                // Direction released
                if self.move_sent {
                    // Was moving, now stopped - send stop command
                    macroquad::logging::info!(
                        "[MOVE] KEY RELEASED -> STOP (prev={:?})",
                        self.prev_dir
                    );
                    commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                    self.last_dx = 0.0;
                    self.last_dy = 0.0;
                    self.last_send_time = current_time;
                    self.move_sent = false;
                } else {
                    // Never sent a move (quick tap or frame timing edge case) - send Face command
                    // But not if attacking - player must finish attack first
                    let attack_anim = state.get_local_player().is_some_and(|p| {
                        matches!(
                            p.animation.state,
                            AnimationState::Attacking
                                | AnimationState::Casting
                                | AnimationState::ShootingBow
                        )
                    });
                    if !attack_anim && !state.is_sitting {
                        let dir = self.prev_dir.to_direction_u8();
                        queue_face(state, commands, dir);
                        self.last_send_time = current_time;
                    }
                }
            } else if keyboard_dir != MoveDir::None && self.prev_dir != MoveDir::None {
                // Direction changed while holding
                if self.move_sent {
                    // Already moving - continue moving in new direction immediately (no threshold wait)
                    // move_sent stays true, don't reset dir_press_time
                } else {
                    // Wasn't moving yet (still in threshold wait) - restart timer for new direction
                    self.dir_press_time = current_time;
                }
            }
        }

        // Handle D-pad release for tap-to-face
        // Use a longer window for tap detection on release - even if movement started,
        // a quick release (under 300ms total) is treated as a face-only tap.
        const TAP_RELEASE_WINDOW: f64 = 0.30; // 300ms
        if dpad_released != DPadDirection::None {
            let hold_duration = current_time - self.touch_controls.get_dpad_press_time();
            let was_short_tap = hold_duration < TAP_RELEASE_WINDOW;

            if was_short_tap {
                // Short tap - send stop if we were moving, then send Face
                if self.touch_controls.was_dpad_move_sent() {
                    commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                }
                let attack_anim = state.get_local_player().is_some_and(|p| {
                    matches!(
                        p.animation.state,
                        AnimationState::Attacking
                            | AnimationState::Casting
                            | AnimationState::ShootingBow
                    )
                });
                if !attack_anim && !state.is_sitting {
                    let dir = dpad_released.to_direction_u8();
                    queue_face(state, commands, dir);
                    self.last_send_time = current_time;
                }
            } else if self.touch_controls.was_dpad_move_sent() {
                // Long hold that was moving - send stop command
                commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
            }
            self.last_dx = 0.0;
            self.last_dy = 0.0;
            self.last_send_time = current_time;
            self.move_sent = false;
            self.touch_controls.set_dpad_move_sent(false);
        }

        self.prev_dir = keyboard_dir;
        self.current_dir = keyboard_dir;

        // Convert direction to velocity
        let (dx, dy): (f32, f32) = new_dir.to_velocity();

        // Only send Move commands if held past the threshold
        // Don't move while attacking - check both attack key/touch button and animation state
        let attack_key_down = if chat_consuming_keyboard {
            false
        } else if classic {
            is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl)
        } else {
            is_key_down(KeyCode::Space)
        };
        // Only block movement for manual attacks (key/button held), not for
        // auto-action attack animations — those are handled by suppress_move_until
        // and the server-side cast_stall_ticks instead.
        let is_attacking = attack_key_down || self.touch_controls.attack_pressed();

        // Check if we have any movement input (keyboard or D-pad)
        let has_movement_input = new_dir != MoveDir::None;

        // Clear the attack-move suppression on key release or expiry.
        let suppress_active = current_time < self.suppress_move_until;
        if suppress_active && !has_movement_input {
            self.suppress_move_until = 0.0;
        }
        let suppress_active = current_time < self.suppress_move_until;

        // Movement while sitting is handled server-side (direction-validated auto-stand)
        // Just let the move command go through - server will stand up if direction matches

        if has_movement_input && !is_attacking && !suppress_active {
            // Cancel auto-action and follow when player manually moves
            if state.auto_action_state.is_some() {
                state.auto_action_state = None;
                state.auto_path = None;
                commands.push(InputCommand::CancelAutoAction);
            }
            if state.follow_target.is_some() {
                state.follow_target = None;
                state.follow_arrived_target_pos = None;
                state.follow_target_move_time = 0.0;
                state.auto_path = None;
            }

            // Determine hold duration based on input source
            let hold_duration = if has_dpad_input {
                current_time - self.touch_controls.get_dpad_press_time()
            } else {
                current_time - self.dir_press_time
            };
            let past_threshold = hold_duration >= FACE_THRESHOLD;

            if past_threshold {
                let direction_changed =
                    (dx - self.last_dx).abs() > 0.01 || (dy - self.last_dy).abs() > 0.01;
                let time_elapsed = current_time - self.last_send_time >= self.send_interval;
                let should_send = direction_changed || time_elapsed;

                if should_send {
                    // When sitting, only allow movement in the chair's facing direction (to stand up)
                    // Otherwise only gate by static tile walkability and let server handle dynamic collisions.
                    let can_move = if state.is_sitting {
                        if let Some(player) = state.get_local_player() {
                            let move_dir = new_dir.to_direction_u8();
                            let chair_dir = player.direction as u8;
                            move_dir == chair_dir
                        } else {
                            false
                        }
                    } else if let Some(player) = state.get_local_player() {
                        let player_x = player.server_x.round() as i32;
                        let player_y = player.server_y.round() as i32;
                        let player_z = player.server_z.round() as i32;
                        let target_x = player_x + dx as i32;
                        let target_y = player_y + dy as i32;
                        let tile_walkable = state
                            .chunk_manager
                            .is_walkable(target_x as f32, target_y as f32);
                        // Match server: block if target terrain is more than 1 block above player
                        let target_height =
                            state.chunk_manager.get_height(target_x, target_y) as i32;
                        let height_ok = (target_height - player_z) <= 1;

                        // Player ghosting: after being blocked by a player for
                        // 500ms, stop treating players as obstacles so movement
                        // flows through them like normal walking.
                        let ghosting = self
                            .player_blocked_since
                            .is_some_and(|since| current_time - since >= 0.5);
                        let occupied = build_occupied_set(state, false, !ghosting);
                        let not_occupied = !occupied.contains(&(target_x, target_y));

                        if tile_walkable && not_occupied && height_ok {
                            // Check if a player is still at the target — if so,
                            // keep ghosting active so we don't flicker on/off.
                            if ghosting {
                                let in_interior = state.current_interior.is_some();
                                let player_at_target = !in_interior
                                    && state.players.iter().any(|(id, p)| {
                                        state.local_player_id.as_ref() != Some(id)
                                            && !p.is_dead
                                            && p.server_x.round() as i32 == target_x
                                            && p.server_y.round() as i32 == target_y
                                    });
                                if !player_at_target {
                                    self.player_blocked_since = None;
                                }
                            } else {
                                self.player_blocked_since = None;
                            }
                            true
                        } else if tile_walkable && height_ok {
                            // Blocked by something on the tile — check if it's a player
                            let in_interior = state.current_interior.is_some();
                            let is_player = !in_interior
                                && state.players.iter().any(|(id, p)| {
                                    state.local_player_id.as_ref() != Some(id)
                                        && !p.is_dead
                                        && p.server_x.round() as i32 == target_x
                                        && p.server_y.round() as i32 == target_y
                                });
                            if is_player {
                                self.player_blocked_since.get_or_insert(current_time);
                            } else {
                                self.player_blocked_since = None;
                            }
                            false
                        } else {
                            self.player_blocked_since = None;
                            false
                        }
                    } else {
                        self.player_blocked_since = None;
                        false
                    };

                    if can_move {
                        commands.push(InputCommand::Move { dx, dy });
                        self.last_dx = dx;
                        self.last_dy = dy;
                        self.last_send_time = current_time;
                        self.move_sent = true;
                        // Also track D-pad move sent
                        if has_dpad_input {
                            self.touch_controls.set_dpad_move_sent(true);
                        }
                    } else {
                        // Can't move - face that direction instead
                        if self.move_sent || self.touch_controls.was_dpad_move_sent() {
                            // Was moving, send stop
                            commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                            self.move_sent = false;
                            self.touch_controls.set_dpad_move_sent(false);
                        }
                        if !state.is_sitting {
                            let face_dir = new_dir.to_direction_u8();
                            queue_face(state, commands, face_dir);
                            self.last_dx = dx;
                            self.last_dy = dy;
                            self.last_send_time = current_time;
                        }
                    }
                }
            }
        }

        // Handle keyboard release when D-pad not active - send stop command
        if !has_dpad_input && keyboard_dir == MoveDir::None && self.move_sent {
            // Already handled above in dir_changed block
        }

        // Dash: left shift while moving
        if !chat_consuming_keyboard && is_key_pressed(KeyCode::LeftShift) {
            let is_moving = self.last_dx != 0.0 || self.last_dy != 0.0;
            if is_moving && current_time >= state.dash_cooldown_end {
                commands.push(InputCommand::Dash);
                // Match server's DASH_COOLDOWN_TICKS (100 @ 20Hz = 5.0s) so we never
                // attempt a dash the server will silently reject (every-other-dash bug).
                state.dash_cooldown_end = current_time + 5.0;
            }
        }

        MovementInput {
            dx,
            dy,
            attack_key_down,
            is_attacking,
            suppress_active,
        }
    }
}
