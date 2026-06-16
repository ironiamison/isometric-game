use super::*;

impl InputHandler {
    pub(super) fn resolve_gameplay_mode(
        &mut self,
        state: &mut GameState,
        layout: &UiLayout,
        audio: &mut AudioManager,
        frame: ProcessFrame<'_>,
        commands: &mut Vec<InputCommand>,
    ) -> Option<GameplayMode> {
        let mx = frame.mx;
        let my = frame.my;
        let mouse_clicked = frame.mouse_clicked;
        let clicked_element = frame.clicked_element.clone();
        // Handle social panel touch scrolling
        if state.ui_state.social_open {
            let all_touches: Vec<Touch> = touches();

            // Handle ongoing touch drag
            if let Some(tracking_id) = state.social_state.touch_scroll_id {
                if let Some(touch) = all_touches.iter().find(|t| t.id == tracking_id) {
                    match touch.phase {
                        TouchPhase::Moved | TouchPhase::Stationary => {
                            let (_, vy) =
                                screen_to_virtual_coords(touch.position.x, touch.position.y);
                            let dy = state.social_state.touch_last_y - vy;
                            if !state.social_state.touch_dragged {
                                let total_dy = (state.social_state.touch_start_y - vy).abs();
                                if total_dy > 8.0 {
                                    state.social_state.touch_dragged = true;
                                }
                            }
                            if state.social_state.touch_dragged {
                                // Update scroll offset based on active tab
                                match state.social_state.active_tab {
                                    crate::game::SocialTab::Nearby
                                    | crate::game::SocialTab::Online => {
                                        state.social_state.list_scroll_offset =
                                            (state.social_state.list_scroll_offset + dy).max(0.0);
                                    }
                                    crate::game::SocialTab::Friends => {
                                        state.social_state.friends_scroll_offset =
                                            (state.social_state.friends_scroll_offset + dy)
                                                .max(0.0);
                                    }
                                }
                            }
                            state.social_state.touch_last_y = vy;
                        }
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            state.social_state.touch_scroll_id = None;
                        }
                        _ => {}
                    }
                } else {
                    state.social_state.touch_scroll_id = None;
                }
            } else {
                // Start new touch drag on scroll area
                for touch in &all_touches {
                    if touch.phase == TouchPhase::Started {
                        let (vx, vy) = screen_to_virtual_coords(touch.position.x, touch.position.y);
                        let hit = layout.hit_test(vx, vy);
                        if matches!(
                            hit,
                            Some(UiElementId::SocialScrollArea)
                                | Some(UiElementId::SocialPlayerRow(_))
                                | Some(UiElementId::SocialFriendRow(_))
                        ) {
                            state.social_state.touch_scroll_id = Some(touch.id);
                            state.social_state.touch_last_y = vy;
                            state.social_state.touch_start_y = vy;
                            state.social_state.touch_dragged = false;
                            break;
                        }
                    }
                }
            }

            // Handle mouse wheel scrolling
            let (_, wheel_y) = mouse_wheel();
            if wheel_y.abs() > 0.1 {
                let scroll_speed = 30.0;
                let row_height = 32.0 * state.ui_state.ui_scale; // SOCIAL_ROW_HEIGHT * scale
                let visible_h = layout
                    .get_bounds(&UiElementId::SocialScrollArea)
                    .map(|b| b.h)
                    .unwrap_or(200.0);
                match state.social_state.active_tab {
                    crate::game::SocialTab::Nearby => {
                        let count = state.social_state.nearby_players.len();
                        let max_scroll = (count as f32 * row_height - visible_h).max(0.0);
                        state.social_state.list_scroll_offset =
                            (state.social_state.list_scroll_offset - wheel_y * scroll_speed)
                                .clamp(0.0, max_scroll);
                    }
                    crate::game::SocialTab::Online => {
                        let count = state.social_state.online_players.len();
                        let max_scroll = (count as f32 * row_height - visible_h).max(0.0);
                        state.social_state.list_scroll_offset =
                            (state.social_state.list_scroll_offset - wheel_y * scroll_speed)
                                .clamp(0.0, max_scroll);
                    }
                    crate::game::SocialTab::Friends => {
                        let count = state.social_state.friends.len();
                        let max_scroll = (count as f32 * row_height - visible_h).max(0.0);
                        state.social_state.friends_scroll_offset =
                            (state.social_state.friends_scroll_offset - wheel_y * scroll_speed)
                                .clamp(0.0, max_scroll);
                    }
                }
            }
        }

        // Handle add friend input when focused
        if state.social_state.add_friend_focused && state.ui_state.social_open {
            // Escape unfocuses the input
            if is_key_pressed(KeyCode::Escape) {
                state.social_state.add_friend_focused = false;
                #[cfg(target_os = "android")]
                macroquad::miniquad::window::show_keyboard(false);
                return None;
            }

            // Enter sends friend request
            if is_key_pressed(KeyCode::Enter) {
                let name = state.social_state.add_friend_input.trim().to_string();
                if !name.is_empty() {
                    audio.play_sfx("enter");
                    commands.push(InputCommand::SendFriendRequest { target_name: name });
                    state.social_state.add_friend_input.clear();
                }
                state.social_state.add_friend_focused = false;
                #[cfg(target_os = "android")]
                macroquad::miniquad::window::show_keyboard(false);
                return None;
            }

            // Backspace removes last character
            if is_key_pressed(KeyCode::Backspace) {
                state.social_state.add_friend_input.pop();
            }

            // Capture typed characters
            while let Some(c) = get_char_pressed() {
                // Filter control characters
                if c.is_control()
                    || !c.is_ascii_graphic() && !c.is_ascii_whitespace() && !c.is_alphanumeric()
                {
                    continue;
                }
                // Limit input length
                if state.social_state.add_friend_input.len() < 20 {
                    state.social_state.add_friend_input.push(c);
                }
            }

            // Don't process other input while typing in add friend field
            return None;
        }

        // Handle chat input mode (must be before chat_panel_open block so typing works)
        // In modern mode, chat consumes keyboard but allows mouse clicks (attack, auto-path)
        let chat_consuming_keyboard = if state.ui_state.chat_open {
            process_chat_keyboard_input(state, commands, audio)
        } else {
            false
        };

        // Handle chat panel scrolling and block game-world input
        if state.ui_state.chat_panel_open {
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                const SCROLL_SPEED: f32 = 40.0; // Pixels per scroll tick
                let max_scroll = layout
                    .get_max_scroll(&UiElementId::ChatPanelScrollbar)
                    .unwrap_or(0.0);
                let delta = wheel_y * SCROLL_SPEED;
                state.ui_state.chat_message_scroll =
                    (state.ui_state.chat_message_scroll + delta).clamp(0.0, max_scroll);
            }

            // Chat panel scrollbar drag handling
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::ChatPanelScrollbar) {
                let cp_max = layout
                    .get_max_scroll(&UiElementId::ChatPanelScrollbar)
                    .unwrap_or(0.0);
                let cp_content_h = cp_max + track_bounds.h;
                let clicked_on = matches!(clicked_element, Some(UiElementId::ChatPanelScrollbar));
                crate::ui::scroll::handle_scrollbar_drag_ex(
                    &mut state.ui_state.chat_scroll_drag,
                    &mut state.ui_state.chat_message_scroll,
                    cp_max,
                    track_bounds,
                    cp_content_h,
                    my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                    true, // inverted: thumb at bottom when scroll=0
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.chat_scroll_drag.dragging = false;
            }

            return None;
        }

        // Minimap panel is modal while open (M/Escape closes it)
        if state.ui_state.minimap_panel_open {
            if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::M) {
                audio.play_sfx("enter");
                state.ui_state.minimap_panel_open = false;
                state.ui_state.minimap_panel_dragging = false;
                return None;
            }

            let panel_rect = minimap_panel_rect();
            let map_rect = minimap_map_rect(panel_rect);
            let over_map = mx >= map_rect.x
                && mx <= map_rect.x + map_rect.w
                && my >= map_rect.y
                && my <= map_rect.y + map_rect.h;

            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y.abs() > 0.0 && over_map {
                if let Some(world_bounds) = minimap_world_bounds(state) {
                    let old_zoom = state
                        .ui_state
                        .minimap_panel_zoom
                        .clamp(MINIMAP_PANEL_MIN_ZOOM, MINIMAP_PANEL_MAX_ZOOM);
                    let zoom_factor = (1.0 + wheel_y * 0.12).max(0.1);
                    let new_zoom = (old_zoom * zoom_factor)
                        .clamp(MINIMAP_PANEL_MIN_ZOOM, MINIMAP_PANEL_MAX_ZOOM);

                    if (new_zoom - old_zoom).abs() > f32::EPSILON {
                        let view_bounds = minimap_panel_view_bounds(state, world_bounds);
                        let anchor_world = minimap_screen_to_world(view_bounds, map_rect, mx, my);
                        let (new_view_w, new_view_h) = minimap_view_size(world_bounds, new_zoom);
                        let nx = ((mx - map_rect.x) / map_rect.w.max(1.0)).clamp(0.0, 1.0);
                        let ny = ((my - map_rect.y) / map_rect.h.max(1.0)).clamp(0.0, 1.0);
                        let target_center_x = anchor_world.0 - (nx - 0.5) * new_view_w;
                        let target_center_y = anchor_world.1 - (ny - 0.5) * new_view_h;
                        let (center_x, center_y) = minimap_clamp_center(
                            world_bounds,
                            new_view_w,
                            new_view_h,
                            target_center_x,
                            target_center_y,
                        );

                        state.ui_state.minimap_panel_zoom = new_zoom;
                        state.ui_state.minimap_panel_center_x = Some(center_x);
                        state.ui_state.minimap_panel_center_y = Some(center_y);
                    }
                }
            }

            if mouse_clicked && over_map {
                state.ui_state.minimap_panel_dragging = true;
                state.ui_state.minimap_panel_drag_last_x = mx;
                state.ui_state.minimap_panel_drag_last_y = my;
            }

            if state.ui_state.minimap_panel_dragging {
                if is_mouse_button_down(MouseButton::Left) {
                    if let Some(world_bounds) = minimap_world_bounds(state) {
                        let view_bounds = minimap_panel_view_bounds(state, world_bounds);
                        let dx_pixels = mx - state.ui_state.minimap_panel_drag_last_x;
                        let dy_pixels = my - state.ui_state.minimap_panel_drag_last_y;

                        if dx_pixels.abs() > 0.0 || dy_pixels.abs() > 0.0 {
                            let view_w = view_bounds.width();
                            let view_h = view_bounds.height();
                            let world_dx = dx_pixels / map_rect.w.max(1.0) * view_w;
                            let world_dy = dy_pixels / map_rect.h.max(1.0) * view_h;
                            let center_x = (view_bounds.min_x + view_bounds.max_x) * 0.5 - world_dx;
                            let center_y = (view_bounds.min_y + view_bounds.max_y) * 0.5 - world_dy;
                            let (center_x, center_y) = minimap_clamp_center(
                                world_bounds,
                                view_w,
                                view_h,
                                center_x,
                                center_y,
                            );
                            state.ui_state.minimap_panel_center_x = Some(center_x);
                            state.ui_state.minimap_panel_center_y = Some(center_y);
                        }
                    }
                    state.ui_state.minimap_panel_drag_last_x = mx;
                    state.ui_state.minimap_panel_drag_last_y = my;
                } else {
                    state.ui_state.minimap_panel_dragging = false;
                }
            }

            // Don't return — fall through so arrow-key movement still works
        }

        // When minimap panel is open, only allow movement keys (skip chat/world interaction)
        let minimap_panel_blocks_input = state.ui_state.minimap_panel_open;
        let classic = state.ui_state.classic_controls;

        // Hold off the Enter→open-chat shortcut until Enter is released after entering
        // gameplay, so the keypress that confirmed character selection doesn't pop chat open.
        if state.ui_state.suppress_enter_chat_open {
            if !is_key_down(KeyCode::Enter) {
                state.ui_state.suppress_enter_chat_open = false;
            }
        } else if !minimap_panel_blocks_input && !chat_consuming_keyboard {
            // Enter key opens chat (not in classic mode - chat is always open)
            // Don't open chat on System tab (read-only)
            if !classic
                && is_key_pressed(KeyCode::Enter)
                && !matches!(state.ui_state.chat_active_tab, ChatChannel::System)
            {
                state.ui_state.chat_open = true;
                state.ui_state.chat_input.clear();
                state.ui_state.chat_cursor = 0;
                state.ui_state.chat_scroll_offset = 0;
                // Drain any accumulated characters from the queue
                while get_char_pressed().is_some() {}
                return None;
            }
        }

        // Drain character queue when chat is closed to prevent accumulation
        if !chat_consuming_keyboard {
            while get_char_pressed().is_some() {}
        }

        Some(GameplayMode {
            chat_consuming_keyboard,
            minimap_panel_blocks_input,
            classic,
        })
    }
}

impl InputHandler {
    pub(super) fn handle_pathing_and_combat(
        &mut self,
        state: &mut GameState,
        mode: &GameplayMode,
        movement: &MovementInput,
        frame: ProcessFrame<'_>,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        let chat_consuming_keyboard = mode.chat_consuming_keyboard;
        let classic = mode.classic;
        let dx = movement.dx;
        let dy = movement.dy;
        let attack_key_down = movement.attack_key_down;
        let is_attacking = movement.is_attacking;
        let suppress_active = movement.suppress_active;
        let current_time = frame.current_time;
        // Get player position from SERVER state (not visual) to avoid getting ahead of server
        let player_pos = state
            .get_local_player()
            .map(|p| (p.server_x.round() as i32, p.server_y.round() as i32));

        // Chase / auto-action re-pathfinding: if auto-action is active,
        // ensure we have a valid path or are adjacent to the target.
        // Handles NPCs, players, AND resources (trees/rocks).
        // This runs even during attack animations so chase can recover immediately.
        if dx == 0.0 && dy == 0.0 {
            if let (Some(ref aa), Some((player_x, player_y))) =
                (&state.auto_action_state, player_pos)
            {
                let target_pos: Option<(i32, i32)> = auto_action_target_pos(aa, state)
                    .map(|(x, y)| (x.round() as i32, y.round() as i32));

                // Check if target still exists (NPC/player could have died/disconnected)
                let target_gone = match aa.target_type.as_str() {
                    "npc" => !state.npcs.contains_key(&aa.target_id),
                    "player" => !state.players.contains_key(&aa.target_id),
                    _ => false, // resources don't disappear mid-chase (depletion handled by server)
                };
                if target_gone {
                    state.auto_action_state = None;
                    state.auto_path = None;
                }

                if let Some((tx, ty)) = target_pos {
                    let weapon_range = get_local_weapon_range(state);
                    let is_in_range = in_attack_range(player_x, player_y, tx, ty, weapon_range);

                    // If already in range and auto-action not yet sent, send now
                    if is_in_range {
                        let auto_action_data = state.auto_action_state.as_ref().map(|aa| {
                            (
                                aa.confirmed,
                                auto_action_target_settled(aa, state),
                                aa.target_type.clone(),
                                aa.target_id.clone(),
                                aa.action.clone(),
                            )
                        });

                        if auto_action_data.is_some() {
                            // Face the target while in range, but skip during attack
                            // animations — the playerAttack message already set the
                            // authoritative direction and re-computing from visual
                            // positions causes rapid flip-flop on diagonal angles.
                            let in_attack_anim = state.get_local_player().is_some_and(|p| {
                                matches!(
                                    p.animation.state,
                                    AnimationState::Attacking
                                        | AnimationState::Casting
                                        | AnimationState::ShootingBow
                                )
                            });
                            if !in_attack_anim {
                                // Use server (grid) positions for both player and target
                                // to match the server's direction computation and avoid
                                // jitter from visual interpolation.
                                let face_delta = state.get_local_player().map(|player| {
                                    (
                                        tx as f32 - player.server_x.round(),
                                        ty as f32 - player.server_y.round(),
                                    )
                                });
                                if let Some((dx, dy)) = face_delta {
                                    face_target_if_needed(state, commands, dx, dy);
                                }
                            }
                        }

                        if let Some((confirmed, settled, target_type, target_id, action)) =
                            auto_action_data
                        {
                            if !confirmed && settled {
                                commands.push(InputCommand::StartAutoAction {
                                    target_type,
                                    target_id,
                                    action,
                                });
                                state.auto_path = None;
                            }
                        }
                    } else {
                        // Not in range — chase toward target.
                        // Clear any stale auto_path if we just entered range on a
                        // previous tick but target moved back out.
                        {
                            let needs_repath = if let Some(ref path_state) = state.auto_path {
                                // Destination no longer close to the target (target moved).
                                // Use a tolerance of 2 so we don't re-path every time the
                                // NPC moves a single tile — finish the current path first.
                                let dest_dist = (path_state.destination.0 - tx).abs()
                                    + (path_state.destination.1 - ty).abs();
                                dest_dist > 2
                            } else {
                                // No path at all — need one
                                true
                            };

                            // Throttle re-pathing to at most once per 300ms to prevent
                            // jerky direction changes when chasing moving targets.
                            const REPATH_COOLDOWN: f64 = 0.3;
                            let repath_allowed =
                                current_time - state.last_chase_repath_time >= REPATH_COOLDOWN;

                            if needs_repath && repath_allowed {
                                // Exclude chase target from occupied set so the target
                                // doesn't block our path when it moves onto our route.
                                let mut occupied = build_occupied_set(state, true, true);
                                if let Some(ref aa) = state.auto_action_state {
                                    match aa.target_type.as_str() {
                                        "npc" => {
                                            if let Some(npc) = state.npcs.get(&aa.target_id) {
                                                occupied.remove(&(
                                                    npc.server_x.round() as i32,
                                                    npc.server_y.round() as i32,
                                                ));
                                            }
                                        }
                                        "player" => {
                                            if let Some(p) = state.players.get(&aa.target_id) {
                                                occupied.remove(&(
                                                    p.server_x.round() as i32,
                                                    p.server_y.round() as i32,
                                                ));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                const MAX_PATH_DISTANCE: i32 = 32;
                                let path_result = if weapon_range > 1 {
                                    find_path_to_attack_with_optimistic_splice(
                                        state,
                                        (player_x, player_y),
                                        (tx, ty),
                                        &occupied,
                                        MAX_PATH_DISTANCE,
                                        weapon_range,
                                    )
                                } else {
                                    let preferred =
                                        preferred_adjacent_tile_for_target(state, (tx, ty));
                                    find_path_to_adjacent_with_optimistic_splice(
                                        state,
                                        (player_x, player_y),
                                        (tx, ty),
                                        &occupied,
                                        MAX_PATH_DISTANCE,
                                        preferred,
                                    )
                                };
                                if let Some((dest, path)) = path_result {
                                    state.auto_path = Some(PathState {
                                        path,
                                        current_index: 0,
                                        destination: dest,
                                        pickup_target: None,
                                        interact_target: None,
                                        interact_object_target: None,
                                        waystone_target: None,
                                        browse_stall_target: None,
                                    });
                                    state.last_chase_repath_time = current_time;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Follow target re-pathing — continuously follow another player
        // Cancel follow if player started attacking or performing an auto-action
        if state.follow_target.is_some() && state.auto_action_state.is_some() {
            state.follow_target = None;
            state.follow_arrived_target_pos = None;
            state.follow_target_move_time = 0.0;
        }
        if let Some(ref follow_id) = state.follow_target.clone() {
            if let Some(target) = state.players.get(follow_id) {
                let tx = target.server_x.round() as i32;
                let ty = target.server_y.round() as i32;

                if let Some(player) = state.get_local_player() {
                    let px = player.server_x.round() as i32;
                    let py = player.server_y.round() as i32;
                    let dist = (px - tx).abs() + (py - ty).abs();

                    if dist <= 1 {
                        // Adjacent — stop and enter waiting state
                        if state.auto_path.is_some() {
                            state.auto_path = None;
                        }
                        // Record target position so we know when they move
                        if state.follow_arrived_target_pos.is_none() {
                            state.follow_arrived_target_pos = Some((tx, ty));
                            state.follow_target_move_time = 0.0;
                        }
                    } else if let Some((ax, ay)) = state.follow_arrived_target_pos {
                        // We were adjacent but now dist > 1 — target moved away
                        if (tx, ty) == (ax, ay) {
                            // Target hasn't actually moved, we drifted — just re-path immediately
                            state.follow_arrived_target_pos = None;
                            state.follow_target_move_time = 0.0;
                        } else {
                            // Target moved — wait 500ms before following
                            if state.follow_target_move_time == 0.0 {
                                state.follow_target_move_time = current_time;
                            }
                            const FOLLOW_MOVE_DELAY: f64 = 0.5;
                            if current_time - state.follow_target_move_time >= FOLLOW_MOVE_DELAY {
                                // Delay elapsed — clear waiting state and path to target
                                state.follow_arrived_target_pos = None;
                                state.follow_target_move_time = 0.0;
                                let mut occupied = build_occupied_set(state, true, true);
                                if let Some(p) = state.players.get(follow_id) {
                                    occupied.remove(&(
                                        p.server_x.round() as i32,
                                        p.server_y.round() as i32,
                                    ));
                                }
                                const MAX_PATH_DISTANCE: i32 = 32;
                                if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                    (px, py),
                                    (tx, ty),
                                    &state.chunk_manager,
                                    &occupied,
                                    MAX_PATH_DISTANCE,
                                ) {
                                    state.auto_path = Some(PathState {
                                        path,
                                        current_index: 0,
                                        destination: dest,
                                        pickup_target: None,
                                        interact_target: None,
                                        interact_object_target: None,
                                        waystone_target: None,
                                        browse_stall_target: None,
                                    });
                                    state.last_chase_repath_time = current_time;
                                }
                            }
                        }
                    } else {
                        // Not adjacent and not in waiting state — normal follow re-pathing
                        let needs_repath = if let Some(ref path_state) = state.auto_path {
                            let dest_dist = (path_state.destination.0 - tx).abs()
                                + (path_state.destination.1 - ty).abs();
                            dest_dist > 2
                        } else {
                            true
                        };

                        const REPATH_COOLDOWN: f64 = 0.6;
                        let repath_allowed =
                            current_time - state.last_chase_repath_time >= REPATH_COOLDOWN;

                        if needs_repath && repath_allowed {
                            let mut occupied = build_occupied_set(state, true, true);
                            if let Some(p) = state.players.get(follow_id) {
                                occupied.remove(&(
                                    p.server_x.round() as i32,
                                    p.server_y.round() as i32,
                                ));
                            }
                            const MAX_PATH_DISTANCE: i32 = 32;
                            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                (px, py),
                                (tx, ty),
                                &state.chunk_manager,
                                &occupied,
                                MAX_PATH_DISTANCE,
                            ) {
                                state.auto_path = Some(PathState {
                                    path,
                                    current_index: 0,
                                    destination: dest,
                                    pickup_target: None,
                                    interact_target: None,
                                    interact_object_target: None,
                                    waystone_target: None,
                                    browse_stall_target: None,
                                });
                                state.last_chase_repath_time = current_time;
                            }
                        }
                    }
                }
            } else {
                // Target player disconnected or left view — stop following
                state.follow_target = None;
                state.follow_arrived_target_pos = None;
                state.follow_target_move_time = 0.0;
            }
        }

        if state.auto_path.is_none() {
            self.reset_auto_path_motion_state();
        }
        // Reset motion state when a new path has been set (current_index == 0)
        // so stale sent-waypoint tracking from a previous path doesn't suppress
        // the first Move command of the new path.
        if let Some(ref path_state) = state.auto_path {
            if path_state.current_index == 0 {
                self.reset_auto_path_motion_state();
            }
        }

        // Path following - generate movement commands when auto-pathing
        // Only follow path if not manually moving and not attacking.
        // When movement is suppressed for an attack auto-action, treat as no keyboard
        // input so the auto-path can walk us into range.
        let no_manual_move = (dx == 0.0 && dy == 0.0) || suppress_active;
        if state.auto_path.is_some() && (!no_manual_move || is_attacking) {
            macroquad::logging::info!(
                "[AUTOPATH] GATE_BLOCKED no_manual={} is_attacking={} dx={} dy={} suppress={}",
                no_manual_move,
                is_attacking,
                dx,
                dy,
                suppress_active
            );
        }
        if no_manual_move && !is_attacking {
            if let (Some((player_x, player_y)), Some(ref mut path_state)) =
                (player_pos, &mut state.auto_path)
            {
                sync_path_index(path_state, (player_x, player_y));
            }

            // Check if next waypoint is blocked by an entity - if so, cancel path.
            // When chasing a target, exclude that target from the blocked check so
            // the target moving onto our path doesn't cause constant re-pathing.
            let mut path_blocked = false;
            if let (Some((player_x, player_y)), Some(ref path_state)) =
                (player_pos, &state.auto_path)
            {
                if path_state.current_index < path_state.path.len() {
                    let (next_x, next_y) = path_state.path[path_state.current_index];
                    if player_x != next_x || player_y != next_y {
                        let mut occupied = build_occupied_set(state, true, true);
                        // Exclude chase/follow target from blocked check
                        if let Some(ref aa) = state.auto_action_state {
                            match aa.target_type.as_str() {
                                "npc" => {
                                    if let Some(npc) = state.npcs.get(&aa.target_id) {
                                        occupied.remove(&(
                                            npc.server_x.round() as i32,
                                            npc.server_y.round() as i32,
                                        ));
                                    }
                                }
                                "player" => {
                                    if let Some(p) = state.players.get(&aa.target_id) {
                                        occupied.remove(&(
                                            p.server_x.round() as i32,
                                            p.server_y.round() as i32,
                                        ));
                                    }
                                }
                                _ => {}
                            }
                        }
                        if let Some(ref fid) = state.follow_target {
                            if let Some(p) = state.players.get(fid) {
                                occupied.remove(&(
                                    p.server_x.round() as i32,
                                    p.server_y.round() as i32,
                                ));
                            }
                        }
                        if occupied.contains(&(next_x, next_y)) {
                            path_blocked = true;
                        }
                    }
                }
            }

            if path_blocked {
                macroquad::logging::info!("[AUTOPATH] PATH_BLOCKED");
                self.reset_auto_path_motion_state();
                if !rebuild_current_auto_path(state) {
                    if state.auto_action_state.is_some() || state.follow_target.is_some() {
                        // Clear the blocked path — chase/follow re-path will recalculate next frame
                        state.auto_path = None;
                        self.reset_auto_path_motion_state();
                    } else {
                        state.auto_path = None;
                        self.reset_auto_path_motion_state();
                        commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                        return true;
                    }
                }
            }

            if let (Some((player_x, player_y)), Some(ref mut path_state)) =
                (player_pos, &mut state.auto_path)
            {
                // Auto-path is step-driven: send one move when a new waypoint becomes active.
                if path_state.current_index < path_state.path.len() {
                    let (next_x, next_y) = path_state.path[path_state.current_index];
                    let move_dx = (next_x - player_x).signum() as f32;
                    let move_dy = (next_y - player_y).signum() as f32;

                    let desired_dir = if move_dx != 0.0 {
                        Some((move_dx, 0.0))
                    } else if move_dy != 0.0 {
                        Some((0.0, move_dy))
                    } else {
                        None
                    };

                    if let Some((send_dx, send_dy)) = desired_dir {
                        let waypoint_changed =
                            self.auto_path_sent_waypoint != Some((next_x, next_y));
                        let dir_changed = self.auto_path_sent_dir != Some((send_dx, send_dy));
                        let time_elapsed = current_time - self.last_send_time >= self.send_interval;

                        if waypoint_changed || dir_changed || time_elapsed {
                            commands.push(InputCommand::Move {
                                dx: send_dx,
                                dy: send_dy,
                            });
                            self.auto_path_sent_waypoint = Some((next_x, next_y));
                            self.auto_path_sent_dir = Some((send_dx, send_dy));
                            self.last_send_time = current_time;
                        }
                    }
                } else {
                    macroquad::logging::info!(
                        "[AUTOPATH] INDEX_PAST_END idx={} len={}",
                        path_state.current_index,
                        path_state.path.len()
                    );
                    self.reset_auto_path_motion_state();
                }
            }

            // Check if path completed and handle pickup/interact if needed
            if state
                .auto_path
                .as_ref()
                .map(|p| p.current_index >= p.path.len())
                .unwrap_or(false)
            {
                // Path completed - check for pickup target
                if let Some(ref path_state) = state.auto_path {
                    if let Some(ref item_id) = path_state.pickup_target {
                        commands.push(InputCommand::Pickup {
                            item_id: item_id.clone(),
                        });
                    }
                    // Handle interact target (NPC)
                    if let Some(ref npc_id) = path_state.interact_target {
                        // Check if target is an altar or station
                        if let Some(npc) = state.npcs.get(npc_id) {
                            // Crafting stations open their UI locally, but the server requires
                            // an active NPC-interaction grant to authorize crafting. Send
                            // Interact so the grant gets registered.
                            if npc.station_type.is_some() {
                                commands.push(InputCommand::Interact {
                                    npc_id: npc_id.clone(),
                                });
                            }
                            if npc.is_altar {
                                state.ui_state.altar_panel = Some(crate::game::AltarPanelState {
                                    altar_npc_id: npc_id.clone(),
                                    altar_name: npc.display_name.clone(),
                                });
                            } else if npc.station_type.as_deref() == Some("furnace")
                                || npc.station_type.as_deref() == Some("fire_pit")
                            {
                                state.ui_state.furnace_station_type =
                                    npc.station_type.clone().unwrap_or_default();
                                state.ui_state.fletching_open = false;
                                state.ui_state.workbench_open = false;
                                state.ui_state.furnace_open = true;
                                state.ui_state.furnace_tile =
                                    Some((npc.x.round() as i32, npc.y.round() as i32));
                                state.ui_state.furnace_selected_recipe = 0;
                                state.ui_state.furnace_scroll_offset = 0.0;
                                state.ui_state.furnace_quantity = 1;
                                state.ui_state.furnace_tab = 0;
                            } else if npc.station_type.as_deref() == Some("anvil") {
                                state.ui_state.fletching_open = false;
                                state.ui_state.workbench_open = false;
                                state.ui_state.anvil_open = true;
                                state.ui_state.anvil_tile =
                                    Some((npc.x.round() as i32, npc.y.round() as i32));
                                state.ui_state.anvil_selected_recipe = 0;
                                state.ui_state.anvil_scroll_offset = 0.0;
                                state.ui_state.anvil_quantity = 1;
                                state.ui_state.anvil_tab = 0;
                            } else if npc.station_type.as_deref() == Some("alchemy_station") {
                                state.ui_state.fletching_open = false;
                                state.ui_state.workbench_open = false;
                                state.ui_state.alchemy_station_open = true;
                                state.ui_state.alchemy_station_tile =
                                    Some((npc.x.round() as i32, npc.y.round() as i32));
                                state.ui_state.alchemy_station_selected_recipe = 0;
                                state.ui_state.alchemy_station_scroll_offset = 0.0;
                                state.ui_state.alchemy_station_quantity = 1;
                                state.ui_state.alchemy_station_tab = 0;
                            } else if npc.station_type.as_deref() == Some("workbench") {
                                state.ui_state.fletching_open = false;
                                state.ui_state.alchemy_station_open = false;
                                state.ui_state.workbench_open = true;
                                state.ui_state.workbench_tile =
                                    Some((npc.x.round() as i32, npc.y.round() as i32));
                                state.ui_state.workbench_selected_recipe = 0;
                                state.ui_state.workbench_scroll_offset = 0.0;
                                state.ui_state.workbench_quantity = 1;
                                state.ui_state.workbench_tab = 0;
                            } else if npc.is_alive() {
                                commands.push(InputCommand::Interact {
                                    npc_id: npc_id.clone(),
                                });
                            }
                        } else {
                            commands.push(InputCommand::Interact {
                                npc_id: npc_id.clone(),
                            });
                        }
                    }
                    // Handle interact object target (map objects like obelisks)
                    if let Some((obj_x, obj_y)) = path_state.interact_object_target {
                        commands.push(InputCommand::InteractObject { x: obj_x, y: obj_y });
                    }
                    // Handle direct waystone teleport (right-click Teleport)
                    if let Some((ws_x, ws_y)) = path_state.waystone_target {
                        commands.push(InputCommand::UseWaystone { x: ws_x, y: ws_y });
                    }
                    // Handle browse stall target (left-click player with stall)
                    if let Some(ref player_id) = path_state.browse_stall_target {
                        commands.push(InputCommand::StallBrowse {
                            player_id: player_id.clone(),
                        });
                    }
                }
                // Handle chair sit target
                if let Some((cx, cy)) = state.pending_chair_sit.take() {
                    commands.push(InputCommand::SitChair {
                        tile_x: cx,
                        tile_y: cy,
                    });
                }
                // Handle farming harvest target
                if let Some(patch_id) = state.pending_harvest_patch.take() {
                    commands.push(InputCommand::HarvestCrop { patch_id });
                }
                // Handle farming cure/clear/compost targets
                if let Some(patch_id) = state.pending_cure_patch.take() {
                    commands.push(InputCommand::CurePatch { patch_id });
                }
                if let Some(patch_id) = state.pending_clear_patch.take() {
                    commands.push(InputCommand::ClearPatch { patch_id });
                }
                if let Some(patch_id) = state.pending_compost_patch.take() {
                    commands.push(InputCommand::ApplyCompost {
                        patch_id,
                        item_id: "compost".to_string(),
                    });
                }
                // Handle auto-action: send StartAutoAction now that we've arrived
                let auto_action_snapshot = state.auto_action_state.as_ref().map(|aa| {
                    (
                        aa.confirmed,
                        auto_action_target_settled(aa, state),
                        aa.target_type.clone(),
                        aa.target_id.clone(),
                        aa.action.clone(),
                        auto_action_target_pos(aa, state),
                    )
                });

                if let Some((confirmed, settled, target_type, target_id, action, target_pos)) =
                    auto_action_snapshot
                {
                    // Always face the target when we reach the destination.
                    if let Some((tx, ty)) = target_pos {
                        if let Some(local_id) = &state.local_player_id {
                            if let Some(player) = state.players.get(local_id) {
                                let dx = tx - player.x;
                                let dy = ty - player.y;
                                face_target_if_needed(state, commands, dx, dy);
                            }
                        }
                    }

                    if !confirmed && settled {
                        commands.push(InputCommand::StartAutoAction {
                            target_type,
                            target_id,
                            action,
                        });
                    }
                }
                state.auto_path = None;
                self.reset_auto_path_motion_state();

                // Send stop command so we don't keep moving in the last direction
                // (but not during auto-action — that would interrupt it on the server)
                if state.auto_action_state.is_none() {
                    macroquad::logging::info!("[AUTOPATH] PATH_COMPLETE -> STOP");
                    commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                }
            }
        }

        // Jump (Ctrl key, modern only) - send jump command on press
        if !chat_consuming_keyboard
            && !classic
            && (is_key_pressed(KeyCode::LeftControl) || is_key_pressed(KeyCode::RightControl))
            && !state.is_sitting
        {
            commands.push(InputCommand::Jump);
        }

        // Attack (Space/Ctrl key or touch attack button) - holding continues attacking with cooldown
        // If fishing rod equipped and on/near a fishing tile, start gathering instead
        // Also stop movement when attacking (player must stand still)
        let attack_input = attack_key_down || self.touch_controls.attack_pressed();
        if attack_input && !state.is_sitting {
            // Send stop command if we were moving via keyboard or auto-path
            let was_pathing = state.auto_path.is_some();
            if self.last_dx != 0.0 || self.last_dy != 0.0 || was_pathing {
                commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                self.last_dx = 0.0;
                self.last_dy = 0.0;
            }
            // Cancel auto-path and auto-action when manually attacking
            state.clear_auto_path();
            self.reset_auto_path_motion_state();
            if state.auto_action_state.is_some() {
                state.auto_action_state = None;
                commands.push(InputCommand::CancelAutoAction);
            }

            // Client-side send throttle for held attacks/gathering. The server is
            // authoritative for the real cooldown (ATTACK_COOLDOWN_MS = 700ms) and
            // silently ignores requests that arrive early, so we intentionally send
            // several times per cooldown window rather than once. This keeps manual
            // attacks on the SAME 700ms beat as server-driven auto-attacks
            // (auto-retaliate / click-to-attack) instead of a slower 800ms beat, and
            // makes the auto->manual hand-off snappy: a fresh request is always ready
            // when the server's cooldown opens, instead of waiting out a slow,
            // mis-phased client timer. Kept a divisor of the server cooldown (700/4)
            // so accepted swings land on a steady beat rather than aliasing slower.
            let attack_cooldown = 0.175;
            if current_time - self.last_attack_time >= attack_cooldown {
                // Check if we should gather instead of attack
                let should_gather = if let Some(player) = state.get_local_player() {
                    if matches!(
                        player.equipped_weapon.as_deref(),
                        Some("fishing_rod" | "maple_rod")
                    ) {
                        let px = player.x.round() as i32;
                        let py = player.y.round() as i32;
                        let (fdx, fdy) = player.direction.to_unit_vector();
                        let face_x = px + fdx as i32;
                        let face_y = py + fdy as i32;
                        // Check if the tile we're facing is a fishing marker
                        state
                            .gathering_markers
                            .iter()
                            .find(|m| m.skill == "fishing" && m.x == face_x && m.y == face_y)
                            .map(|m| (m.x, m.y))
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Check if we should woodcut instead of attack (axe equipped + facing tree)
                let should_woodcut = if should_gather.is_none() {
                    if let Some(player) = state.get_local_player() {
                        // Check if player has an axe equipped (chop_speed_multiplier > 0)
                        let has_axe = player
                            .equipped_weapon
                            .as_ref()
                            .and_then(|weapon_id| state.item_registry.get(weapon_id))
                            .and_then(|item| item.equipment.as_ref())
                            .map(|eq| eq.chop_speed_multiplier > 0.0)
                            .unwrap_or(false);

                        if has_axe {
                            let px = player.x.round() as i32;
                            let py = player.y.round() as i32;
                            let (fdx, fdy) = player.direction.to_unit_vector();
                            let face_x = px + fdx as i32;
                            let face_y = py + fdy as i32;

                            // Check if facing tile has a tree object and is not depleted
                            if !state.depleted_trees.contains_key(&(face_x, face_y)) {
                                let obj_result =
                                    state.chunk_manager.get_object_at_exact(face_x, face_y);
                                obj_result.map(|obj| (face_x, face_y, obj.gid))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Check if we should mine instead of attack (pickaxe equipped + facing rock)
                let should_mine = if should_gather.is_none() && should_woodcut.is_none() {
                    if let Some(player) = state.get_local_player() {
                        // Check if player has a pickaxe equipped (mine_speed_multiplier > 0)
                        let has_pickaxe = player
                            .equipped_weapon
                            .as_ref()
                            .and_then(|weapon_id| state.item_registry.get(weapon_id))
                            .and_then(|item| item.equipment.as_ref())
                            .map(|eq| eq.mine_speed_multiplier > 0.0)
                            .unwrap_or(false);

                        if has_pickaxe {
                            let px = player.x.round() as i32;
                            let py = player.y.round() as i32;
                            let (fdx, fdy) = player.direction.to_unit_vector();
                            let face_x = px + fdx as i32;
                            let face_y = py + fdy as i32;

                            // Check if facing tile has a rock object and is not depleted
                            if !state.depleted_rocks.contains_key(&(face_x, face_y)) {
                                let obj_result =
                                    state.chunk_manager.get_object_at_exact(face_x, face_y);
                                obj_result.map(|obj| (face_x, face_y, obj.gid))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some((marker_x, marker_y)) = should_gather {
                    if !state.is_gathering {
                        commands.push(InputCommand::StartGathering { marker_x, marker_y });
                        self.last_attack_time = current_time;
                    }
                } else if let Some((tree_x, tree_y, tree_gid)) = should_woodcut {
                    // Send chop command on each attack press when facing a tree with an axe
                    commands.push(InputCommand::ChopTree {
                        tree_x,
                        tree_y,
                        tree_gid,
                    });
                    self.last_attack_time = current_time;
                } else if let Some((rock_x, rock_y, rock_gid)) = should_mine {
                    // Send mine command on each attack press when facing a rock with a pickaxe
                    commands.push(InputCommand::MineRock {
                        rock_x,
                        rock_y,
                        rock_gid,
                    });
                    self.last_attack_time = current_time;
                } else {
                    // Swing animation is server-authoritative (driven by the server's
                    // PlayerAttack echo), so we only send the command here — no local
                    // animation prediction. This avoids a phantom swing animation when
                    // the server rejects this attack for cooldown (e.g. an auto-retaliate
                    // swing already used the cooldown window).
                    commands.push(InputCommand::Attack);
                    self.last_attack_time = current_time;
                }
            }
        }

        false
    }
}
