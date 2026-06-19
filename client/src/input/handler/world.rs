use super::*;

impl InputHandler {
    pub(super) fn handle_world_selection(
        &mut self,
        state: &mut GameState,
        frame: ProcessFrame<'_>,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        let current_time = frame.current_time;
        let mouse_clicked = frame.mouse_clicked;
        let clicked_element = frame.clicked_element.clone();
        // Quest tracker: a left-click (or tap) toggles minimize/expand. Handle this before
        // the click-to-move logic below so clicking the tracker doesn't also move the player.
        if mouse_clicked {
            if let Some(tracker_rect) = state.ui_state.quest_tracker_rect.get() {
                let (raw_x, raw_y) = mouse_position();
                let (mouse_vx, mouse_vy) = screen_to_virtual_coords(raw_x, raw_y);
                if tracker_rect.contains(macroquad::math::Vec2::new(mouse_vx, mouse_vy)) {
                    state.ui_state.quest_tracker_minimized =
                        !state.ui_state.quest_tracker_minimized;
                    save_current_ui_settings(state);
                    return true;
                }
            }
        }
        // Target selection (left click) - only if not clicking on UI
        if mouse_clicked && clicked_element.is_none() {
            let (raw_x, raw_y) = mouse_position();
            let (mouse_x, mouse_y) = screen_to_virtual_coords(raw_x, raw_y);
            let (click_world_x, click_world_y) = screen_to_world(mouse_x, mouse_y, &state.camera);

            // Get the clicked tile coordinates (elevation-aware)
            let (clicked_tile_x, clicked_tile_y, _clicked_tile_z) = state
                .chunk_manager
                .pick_tile_at_screen(mouse_x, mouse_y, &state.camera);

            // Find entity on the exact clicked tile
            let mut clicked_player: Option<String> = None;
            let mut clicked_npc: Option<String> = None;

            // Check players - must be on the exact clicked tile
            for (id, player) in &state.players {
                // Don't allow targeting self
                if state.local_player_id.as_ref() == Some(id) {
                    continue;
                }

                let player_tile_x = player.x.round() as i32;
                let player_tile_y = player.y.round() as i32;

                if player_tile_x == clicked_tile_x && player_tile_y == clicked_tile_y {
                    clicked_player = Some(id.clone());
                    break;
                }
            }

            // Check NPCs - must be on the exact clicked tile
            for (id, npc) in &state.npcs {
                // Only allow interacting with alive NPCs
                if !npc.is_alive() {
                    continue;
                }

                let npc_tile_x = npc.x.round() as i32;
                let npc_tile_y = npc.y.round() as i32;

                let mut on_footprint = false;
                for dy in 0..npc.size {
                    for dx in 0..npc.size {
                        if npc_tile_x + dx == clicked_tile_x && npc_tile_y + dy == clicked_tile_y {
                            on_footprint = true;
                        }
                    }
                }
                if on_footprint {
                    clicked_npc = Some(id.clone());
                    break;
                }
            }

            // Prioritize NPC interaction over player targeting
            if let Some(npc_id) = clicked_npc {
                // Check if NPC can be targeted for combat (not a merchant/quest giver/banker/altar)
                let is_attackable = state
                    .npcs
                    .get(&npc_id)
                    .map(|n| n.is_attackable())
                    .unwrap_or(true);

                if is_attackable {
                    // Attackable NPC - target it and set up auto-action chase
                    state.click_effects.clear();
                    state.click_effects.push(ClickEffect::new(
                        click_world_x,
                        click_world_y,
                        ClickEffectKind::Attack,
                    ));
                    // Cancel any existing server-side auto-action before starting a new one
                    commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                    state.auto_path = None;
                    self.reset_auto_path_motion_state();
                    if state.auto_action_state.is_some() {
                        commands.push(InputCommand::CancelAutoAction);
                    }
                    commands.push(InputCommand::Target {
                        entity_id: npc_id.clone(),
                    });
                    state.auto_action_state = Some(crate::game::AutoActionState {
                        target_type: "npc".to_string(),
                        target_id: npc_id.clone(),
                        action: "attack".to_string(),
                        confirmed: false,
                    });
                    // Check range — only stop + suppress keyboard if in range
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(player) = state.players.get(local_id) {
                            if let Some(npc) = state.npcs.get(&npc_id) {
                                let player_x = player.server_x.round() as i32;
                                let player_y = player.server_y.round() as i32;
                                let npc_x = npc.server_x.round() as i32;
                                let npc_y = npc.server_y.round() as i32;
                                let weapon_range = get_local_weapon_range(state);
                                if !in_attack_range(player_x, player_y, npc_x, npc_y, weapon_range)
                                {
                                    let occupied = build_occupied_set(state, true, true);
                                    const MAX_PATH_DISTANCE: i32 = 32;
                                    let path_result = if weapon_range > 1 {
                                        pathfinding::find_path_within_range(
                                            (player_x, player_y),
                                            (npc_x, npc_y),
                                            &state.chunk_manager,
                                            &occupied,
                                            MAX_PATH_DISTANCE,
                                            weapon_range,
                                        )
                                    } else {
                                        pathfinding::find_path_to_adjacent(
                                            (player_x, player_y),
                                            (npc_x, npc_y),
                                            &state.chunk_manager,
                                            &occupied,
                                            MAX_PATH_DISTANCE,
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
                                    } else {
                                        // Pathfinding failed but we're close — send auto-action
                                        // anyway and let the server handle range checking.
                                        let dir = crate::game::Direction::from_velocity(
                                            npc_x as f32 - player_x as f32,
                                            npc_y as f32 - player_y as f32,
                                        );
                                        queue_face(state, commands, dir as u8);
                                        commands.push(InputCommand::StartAutoAction {
                                            target_type: "npc".to_string(),
                                            target_id: npc_id.clone(),
                                            action: "attack".to_string(),
                                        });
                                    }
                                } else {
                                    // Already in range — stop movement and suppress keyboard
                                    // so the attack fires without held keys cancelling it.
                                    self.last_dx = 0.0;
                                    self.last_dy = 0.0;
                                    self.move_sent = false;
                                    self.suppress_move_until = current_time + 0.6;
                                    let dir = crate::game::Direction::from_velocity(
                                        npc_x as f32 - player_x as f32,
                                        npc_y as f32 - player_y as f32,
                                    );
                                    queue_face(state, commands, dir as u8);
                                    commands.push(InputCommand::StartAutoAction {
                                        target_type: "npc".to_string(),
                                        target_id: npc_id.clone(),
                                        action: "attack".to_string(),
                                    });
                                }
                            }
                        }
                    }
                } else {
                    // Friendly NPC - interact or pathfind-to-interact
                    state.click_effects.clear();
                    state.click_effects.push(ClickEffect::new(
                        click_world_x,
                        click_world_y,
                        ClickEffectKind::Interact,
                    ));
                    const INTERACT_RANGE: f32 = 2.5;
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(player) = state.players.get(local_id) {
                            if let Some(npc) = state.npcs.get(&npc_id) {
                                let dx = npc.x - player.x;
                                let dy = npc.y - player.y;
                                let dist_to_player = (dx * dx + dy * dy).sqrt();

                                // If an inventory item is selected, use it on this NPC instead of interacting
                                if let Some(selected_slot) = state.ui_state.selected_inventory_slot
                                {
                                    if dist_to_player < INTERACT_RANGE {
                                        commands.push(InputCommand::UseItemOnEntity {
                                            slot_index: selected_slot as u8,
                                            npc_id: npc_id.clone(),
                                        });
                                        state.ui_state.selected_inventory_slot = None;
                                    }
                                    return true;
                                }

                                if dist_to_player < INTERACT_RANGE {
                                    // Crafting stations open their UI locally, but the server
                                    // requires an active NPC-interaction grant to authorize
                                    // crafting. Send Interact so the grant gets registered.
                                    if npc.station_type.is_some() {
                                        commands.push(InputCommand::Interact {
                                            npc_id: npc_id.clone(),
                                        });
                                    }
                                    // Check if NPC is an altar or station
                                    if npc.is_altar {
                                        state.ui_state.altar_panel =
                                            Some(crate::game::AltarPanelState {
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
                                    } else if npc.station_type.as_deref() == Some("alchemy_station")
                                    {
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
                                    } else {
                                        commands.push(InputCommand::Interact { npc_id });
                                    }
                                } else {
                                    // Out of range - pathfind to adjacent tile
                                    let player_x = player.server_x.round() as i32;
                                    let player_y = player.server_y.round() as i32;
                                    let npc_x = npc.server_x.round() as i32;
                                    let npc_y = npc.server_y.round() as i32;

                                    // Build occupied set (other players + NPCs)
                                    let occupied = build_occupied_set(state, true, true);

                                    const MAX_PATH_DISTANCE: i32 = 32;
                                    if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                        (player_x, player_y),
                                        (npc_x, npc_y),
                                        &state.chunk_manager,
                                        &occupied,
                                        MAX_PATH_DISTANCE,
                                    ) {
                                        state.auto_path = Some(PathState {
                                            path,
                                            current_index: 0,
                                            destination: dest,
                                            pickup_target: None,
                                            interact_target: Some(npc_id),
                                            interact_object_target: None,
                                            waystone_target: None,
                                            browse_stall_target: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            } else if let Some(entity_id) = clicked_player {
                // Check if clicked player has an open stall - browse instead of attack
                let target_has_stall = state.players.get(&entity_id).is_some_and(|p| p.has_stall);

                if target_has_stall {
                    // Player has a stall - pathfind to them and browse their shop
                    state.click_effects.clear();
                    state.click_effects.push(ClickEffect::new(
                        click_world_x,
                        click_world_y,
                        ClickEffectKind::Interact,
                    ));
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(local_player) = state.players.get(local_id) {
                            if let Some(target_player) = state.players.get(&entity_id) {
                                let player_x = local_player.server_x.round() as i32;
                                let player_y = local_player.server_y.round() as i32;
                                let target_x = target_player.server_x.round() as i32;
                                let target_y = target_player.server_y.round() as i32;
                                let cdx = (player_x - target_x).abs();
                                let cdy = (player_y - target_y).abs();
                                if (cdx + cdy) <= 3 {
                                    // Already in range - browse immediately
                                    commands.push(InputCommand::StallBrowse {
                                        player_id: entity_id.clone(),
                                    });
                                } else {
                                    // Pathfind to adjacent tile, then browse on arrival
                                    let occupied = build_occupied_set(state, true, true);
                                    const MAX_PATH_DISTANCE: i32 = 32;
                                    if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                        (player_x, player_y),
                                        (target_x, target_y),
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
                                            browse_stall_target: Some(entity_id.clone()),
                                        });
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Normal player click - target and set up auto-action chase
                    state.click_effects.clear();
                    state.click_effects.push(ClickEffect::new(
                        click_world_x,
                        click_world_y,
                        ClickEffectKind::Attack,
                    ));
                    // Cancel any existing server-side auto-action before starting a new one
                    commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                    state.auto_path = None;
                    self.reset_auto_path_motion_state();
                    if state.auto_action_state.is_some() {
                        commands.push(InputCommand::CancelAutoAction);
                    }
                    commands.push(InputCommand::Target {
                        entity_id: entity_id.clone(),
                    });
                    state.auto_action_state = Some(crate::game::AutoActionState {
                        target_type: "player".to_string(),
                        target_id: entity_id.clone(),
                        action: "attack".to_string(),
                        confirmed: false,
                    });
                    // Pathfind to within attack range, or send immediately if already in range
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(local_player) = state.players.get(local_id) {
                            if let Some(target_player) = state.players.get(&entity_id) {
                                let player_x = local_player.server_x.round() as i32;
                                let player_y = local_player.server_y.round() as i32;
                                let target_x = target_player.server_x.round() as i32;
                                let target_y = target_player.server_y.round() as i32;
                                let weapon_range = get_local_weapon_range(state);
                                if !in_attack_range(
                                    player_x,
                                    player_y,
                                    target_x,
                                    target_y,
                                    weapon_range,
                                ) {
                                    let occupied = build_occupied_set(state, true, true);
                                    const MAX_PATH_DISTANCE: i32 = 32;
                                    let path_result = if weapon_range > 1 {
                                        pathfinding::find_path_within_range(
                                            (player_x, player_y),
                                            (target_x, target_y),
                                            &state.chunk_manager,
                                            &occupied,
                                            MAX_PATH_DISTANCE,
                                            weapon_range,
                                        )
                                    } else {
                                        pathfinding::find_path_to_adjacent(
                                            (player_x, player_y),
                                            (target_x, target_y),
                                            &state.chunk_manager,
                                            &occupied,
                                            MAX_PATH_DISTANCE,
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
                                    }
                                } else {
                                    // Already in range - face target and send to server immediately
                                    let dir = crate::game::Direction::from_velocity(
                                        target_x as f32 - player_x as f32,
                                        target_y as f32 - player_y as f32,
                                    );
                                    queue_face(state, commands, dir as u8);
                                    commands.push(InputCommand::StartAutoAction {
                                        target_type: "player".to_string(),
                                        target_id: entity_id.clone(),
                                        action: "attack".to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            } else if let Some(ground_item_id) = state
                .ground_items
                .values()
                .find(|item| {
                    let (tile_x, tile_y) = item.tile_coords();
                    tile_x == clicked_tile_x && tile_y == clicked_tile_y
                })
                .map(|item| item.id.clone())
            {
                // Clicked on a tile with a ground item - attempt pickup
                if let Some(local_id) = &state.local_player_id {
                    if let Some(player) = state.players.get(local_id) {
                        if let Some(ground_item) = state.ground_items.get(&ground_item_id) {
                            let dx = ground_item.x - player.x;
                            let dy = ground_item.y - player.y;
                            let dist = (dx * dx + dy * dy).sqrt();

                            const PICKUP_RANGE: f32 = 2.0;
                            if dist < PICKUP_RANGE {
                                commands.push(InputCommand::Pickup {
                                    item_id: ground_item_id,
                                });
                            } else {
                                // Out of range - path to an adjacent tile
                                let player_x = player.server_x.round() as i32;
                                let player_y = player.server_y.round() as i32;
                                let item_x = ground_item.x.floor() as i32;
                                let item_y = ground_item.y.floor() as i32;

                                let occupied = build_occupied_set(state, true, true);

                                const MAX_PATH_DISTANCE: i32 = 32;
                                if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                    (player_x, player_y),
                                    (item_x, item_y),
                                    &state.chunk_manager,
                                    &occupied,
                                    MAX_PATH_DISTANCE,
                                ) {
                                    state.auto_path = Some(PathState {
                                        path,
                                        current_index: 0,
                                        destination: dest,
                                        pickup_target: Some(ground_item_id),
                                        interact_target: None,
                                        interact_object_target: None,
                                        waystone_target: None,
                                        browse_stall_target: None,
                                    });
                                }
                            }
                        }
                    }
                }
            } else if state
                .chair_positions
                .contains(&(clicked_tile_x, clicked_tile_y))
            {
                // Clicked on a chair - try to sit
                if !state.is_sitting {
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(player) = state.players.get(local_id) {
                            let px = player.server_x.round() as i32;
                            let py = player.server_y.round() as i32;
                            let cdx = (px - clicked_tile_x).abs();
                            let cdy = (py - clicked_tile_y).abs();
                            if cdx <= 1 && cdy <= 1 {
                                // Within range - sit immediately
                                commands.push(InputCommand::SitChair {
                                    tile_x: clicked_tile_x,
                                    tile_y: clicked_tile_y,
                                });
                            } else {
                                // Out of range - pathfind to adjacent tile, then sit
                                let occupied = build_occupied_set(state, true, true);
                                const MAX_PATH_DISTANCE: i32 = 32;
                                if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                    (px, py),
                                    (clicked_tile_x, clicked_tile_y),
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
                                    state.pending_chair_sit =
                                        Some((clicked_tile_x, clicked_tile_y));
                                }
                            }
                        }
                    }
                }
            } else if let Some(obj) = state
                .chunk_manager
                .get_object_at_exact(clicked_tile_x, clicked_tile_y)
            {
                // Check if clicked object is a tree or rock for auto-action
                let obj_gid = obj.gid;
                let is_tree = crate::game::tree_types::is_tree_gid(obj_gid);
                let is_rock = crate::game::ore_types::get_ore_info(obj_gid).is_some();

                // Show click effect — attack for resources, interact for other objects
                let click_kind = if is_tree || is_rock {
                    ClickEffectKind::Attack
                } else {
                    ClickEffectKind::Interact
                };
                state.click_effects.clear();
                state.click_effects.push(ClickEffect::new(
                    click_world_x,
                    click_world_y,
                    click_kind,
                ));

                if is_tree
                    && !state
                        .depleted_trees
                        .contains_key(&(clicked_tile_x, clicked_tile_y))
                {
                    // Check if player has axe equipped
                    let has_axe = state
                        .get_local_player()
                        .and_then(|p| p.equipped_weapon.as_ref())
                        .and_then(|weapon_id| state.item_registry.get(weapon_id))
                        .and_then(|item| item.equipment.as_ref())
                        .map(|eq| eq.chop_speed_multiplier > 0.0)
                        .unwrap_or(false);

                    if has_axe {
                        let target_id =
                            format!("{},{},{}", clicked_tile_x, clicked_tile_y, obj_gid);
                        state.auto_action_state = Some(crate::game::AutoActionState {
                            target_type: "resource".to_string(),
                            target_id: target_id.clone(),
                            action: "chop".to_string(),
                            confirmed: false,
                        });
                        // Pathfind to adjacent tile, or send immediately if already adjacent
                        if let Some(player) = state.get_local_player() {
                            let player_x = player.server_x.round() as i32;
                            let player_y = player.server_y.round() as i32;
                            let cdx = (player_x - clicked_tile_x).abs();
                            let cdy = (player_y - clicked_tile_y).abs();
                            // Cardinal adjacency only (no diagonal)
                            if (cdx + cdy) != 1 {
                                let occupied = build_occupied_set(state, true, true);
                                const MAX_PATH_DISTANCE: i32 = 32;
                                if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                    (player_x, player_y),
                                    (clicked_tile_x, clicked_tile_y),
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
                                }
                            } else {
                                // Already cardinal-adjacent - face target and send immediately
                                let dir = crate::game::Direction::from_velocity(
                                    clicked_tile_x as f32 - player_x as f32,
                                    clicked_tile_y as f32 - player_y as f32,
                                );
                                queue_face(state, commands, dir as u8);
                                commands.push(InputCommand::StartAutoAction {
                                    target_type: "resource".to_string(),
                                    target_id,
                                    action: "chop".to_string(),
                                });
                            }
                        }
                    }
                } else if is_rock
                    && !state
                        .depleted_rocks
                        .contains_key(&(clicked_tile_x, clicked_tile_y))
                {
                    // Check if player has pickaxe equipped
                    let has_pickaxe = state
                        .get_local_player()
                        .and_then(|p| p.equipped_weapon.as_ref())
                        .and_then(|weapon_id| state.item_registry.get(weapon_id))
                        .and_then(|item| item.equipment.as_ref())
                        .map(|eq| eq.mine_speed_multiplier > 0.0)
                        .unwrap_or(false);

                    if has_pickaxe {
                        let target_id =
                            format!("{},{},{}", clicked_tile_x, clicked_tile_y, obj_gid);
                        state.auto_action_state = Some(crate::game::AutoActionState {
                            target_type: "resource".to_string(),
                            target_id: target_id.clone(),
                            action: "mine".to_string(),
                            confirmed: false,
                        });
                        // Pathfind to adjacent tile, or send immediately if already adjacent
                        if let Some(player) = state.get_local_player() {
                            let player_x = player.server_x.round() as i32;
                            let player_y = player.server_y.round() as i32;
                            let cdx = (player_x - clicked_tile_x).abs();
                            let cdy = (player_y - clicked_tile_y).abs();
                            // Cardinal adjacency only (no diagonal)
                            if (cdx + cdy) != 1 {
                                let occupied = build_occupied_set(state, true, true);
                                const MAX_PATH_DISTANCE: i32 = 32;
                                if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                    (player_x, player_y),
                                    (clicked_tile_x, clicked_tile_y),
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
                                }
                            } else {
                                // Already cardinal-adjacent - face target and send immediately
                                let dir = crate::game::Direction::from_velocity(
                                    clicked_tile_x as f32 - player_x as f32,
                                    clicked_tile_y as f32 - player_y as f32,
                                );
                                queue_face(state, commands, dir as u8);
                                commands.push(InputCommand::StartAutoAction {
                                    target_type: "resource".to_string(),
                                    target_id,
                                    action: "mine".to_string(),
                                });
                            }
                        }
                    }
                } else if is_obelisk_gid(obj_gid)
                    || state
                        .chest_positions
                        .contains(&(clicked_tile_x, clicked_tile_y))
                {
                    // Clicked on an obelisk or chest — walk to it and interact
                    if let Some(player) = state.get_local_player() {
                        let player_x = player.server_x.round() as i32;
                        let player_y = player.server_y.round() as i32;
                        let cdx = (player_x - clicked_tile_x).abs();
                        let cdy = (player_y - clicked_tile_y).abs();
                        if cdx <= 1 && cdy <= 1 {
                            // Already adjacent — interact immediately
                            commands.push(InputCommand::InteractObject {
                                x: clicked_tile_x,
                                y: clicked_tile_y,
                            });
                        } else {
                            // Pathfind to adjacent tile, then interact
                            let occupied = build_occupied_set(state, true, true);
                            const MAX_PATH_DISTANCE: i32 = 32;
                            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                (player_x, player_y),
                                (clicked_tile_x, clicked_tile_y),
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
                                    interact_object_target: Some((clicked_tile_x, clicked_tile_y)),
                                    waystone_target: None,
                                    browse_stall_target: None,
                                });
                            }
                        }
                    }
                }
            } else if let Some(patch_id) = state
                .farming_patch_positions
                .get(&(clicked_tile_x, clicked_tile_y))
                .cloned()
            {
                // Clicked on a farming patch
                if let Some(patch) = state.farming_patches.get(&patch_id) {
                    if patch.state == "harvestable" {
                        if let Some(local_id) = &state.local_player_id {
                            if let Some(player) = state.players.get(local_id) {
                                let px = player.server_x.round() as i32;
                                let py = player.server_y.round() as i32;
                                let cdx = (px - clicked_tile_x).abs();
                                let cdy = (py - clicked_tile_y).abs();
                                if cdx <= 1 && cdy <= 1 {
                                    commands.push(InputCommand::HarvestCrop { patch_id });
                                } else {
                                    // Out of range - pathfind to adjacent tile
                                    let occupied = build_occupied_set(state, true, true);
                                    const MAX_PATH_DISTANCE: i32 = 32;
                                    if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                        (px, py),
                                        (clicked_tile_x, clicked_tile_y),
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
                                        state.pending_harvest_patch = Some(patch_id);
                                    }
                                }
                            }
                        }
                    }
                }
            } else if state
                .chest_positions
                .contains(&(clicked_tile_x, clicked_tile_y))
            {
                // Clicked on a chest - walk to it and interact
                if let Some(player) = state.get_local_player() {
                    let px = player.server_x.round() as i32;
                    let py = player.server_y.round() as i32;
                    let cdx = (px - clicked_tile_x).abs();
                    let cdy = (py - clicked_tile_y).abs();
                    if cdx <= 1 && cdy <= 1 {
                        // Already adjacent — interact immediately
                        commands.push(InputCommand::InteractObject {
                            x: clicked_tile_x,
                            y: clicked_tile_y,
                        });
                    } else {
                        // Pathfind to adjacent tile, then interact
                        let occupied = build_occupied_set(state, true, true);
                        const MAX_PATH_DISTANCE: i32 = 32;
                        if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                            (px, py),
                            (clicked_tile_x, clicked_tile_y),
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
                                interact_object_target: Some((clicked_tile_x, clicked_tile_y)),
                                waystone_target: None,
                                browse_stall_target: None,
                            });
                        }
                    }
                }
            } else if state.ui_state.tap_to_pathfind {
                // If an inventory item is selected, clear it instead of moving
                if state.ui_state.selected_inventory_slot.is_some() {
                    state.ui_state.selected_inventory_slot = None;
                    return true;
                }
                // Clicked on empty space - cancel auto-action and path there
                if state.auto_action_state.is_some() {
                    state.auto_action_state = None;
                    commands.push(InputCommand::CancelAutoAction);
                }
                // Send stop to cancel any queued Move from the old auto_path
                // (which already ran earlier this frame)
                commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                state.auto_path = None;
                self.reset_auto_path_motion_state();
                self.suppress_move_until = 0.0;

                // Use elevation-aware tile picking for click-to-move
                let tile_x = clicked_tile_x;
                let tile_y = clicked_tile_y;

                // Only path if within range and walkable
                const MAX_PATH_DISTANCE: i32 = 32;

                if let Some(player) = state.get_local_player() {
                    // Use server-authoritative tile for click-to-move.
                    let player_x = player.server_x.round() as i32;
                    let player_y = player.server_y.round() as i32;
                    let dist = (tile_x - player_x).abs().max((tile_y - player_y).abs());

                    if dist <= MAX_PATH_DISTANCE
                        && state
                            .chunk_manager
                            .is_walkable(tile_x as f32, tile_y as f32)
                    {
                        // Build occupied set (other players + NPCs)
                        let occupied = build_occupied_set(state, true, true);

                        // Calculate path using A*
                        if let Some(path) = pathfinding::find_path(
                            (player_x, player_y),
                            (tile_x, tile_y),
                            &state.chunk_manager,
                            &occupied,
                            MAX_PATH_DISTANCE,
                        ) {
                            macroquad::logging::info!(
                                "[CLICK2MOVE] path created len={} from=({},{}) to=({},{})",
                                path.len(),
                                player_x,
                                player_y,
                                tile_x,
                                tile_y
                            );
                            state.click_effects.clear();
                            state.click_effects.push(ClickEffect::new(
                                click_world_x,
                                click_world_y,
                                ClickEffectKind::Walk,
                            ));
                            state.auto_path = Some(PathState {
                                path,
                                current_index: 0,
                                destination: (tile_x, tile_y),
                                pickup_target: None,
                                interact_target: None,
                                interact_object_target: None,
                                waystone_target: None,
                                browse_stall_target: None,
                            });
                        } else {
                            macroquad::logging::info!(
                                "[CLICK2MOVE] pathfind FAILED from=({},{}) to=({},{})",
                                player_x,
                                player_y,
                                tile_x,
                                tile_y
                            );
                        }
                    } else {
                        macroquad::logging::info!(
                            "[CLICK2MOVE] BLOCKED dist={} walkable={}",
                            dist,
                            state
                                .chunk_manager
                                .is_walkable(tile_x as f32, tile_y as f32)
                        );
                    }
                } else {
                    macroquad::logging::info!("[CLICK2MOVE] no local player");
                }

                // Also clear target when clicking empty space
                if state.selected_entity_id.is_some() {
                    commands.push(InputCommand::ClearTarget);
                }
            }
        }

        false
    }
}

impl InputHandler {
    pub(super) fn handle_world_context(
        &mut self,
        state: &mut GameState,
        frame: ProcessFrame<'_>,
        _commands: &mut Vec<InputCommand>,
    ) -> bool {
        let mx = frame.mx;
        let my = frame.my;
        let mouse_right_clicked = frame.mouse_right_clicked;
        let clicked_element = frame.clicked_element.clone();
        // Quest tracker interaction
        if let Some(tracker_rect) = state.ui_state.quest_tracker_rect.get() {
            let (raw_x, raw_y) = mouse_position();
            let (mouse_vx, mouse_vy) = screen_to_virtual_coords(raw_x, raw_y);
            if tracker_rect.contains(macroquad::math::Vec2::new(mouse_vx, mouse_vy)) {
                // Left-click/tap toggling is handled in handle_world_selection (it runs
                // before click-to-move). Here we only open the right-click context menu
                // (desktop, or long-press on mobile).
                if mouse_right_clicked {
                    state.ui_state.context_menu = Some(ContextMenu {
                        target: ContextMenuTarget::QuestTracker,
                        x: mouse_vx,
                        y: mouse_vy,
                    });
                    return true;
                }
            }
        }

        // Right-click world detection - open context menu for world entities
        if mouse_right_clicked && clicked_element.is_none() {
            let (raw_x, raw_y) = mouse_position();
            let (mouse_vx, mouse_vy) = screen_to_virtual_coords(raw_x, raw_y);
            // Use elevation-aware tile picking for right-click
            let (clicked_tile_x, clicked_tile_y, _clicked_tile_z) = state
                .chunk_manager
                .pick_tile_at_screen(mouse_vx, mouse_vy, &state.camera);

            // Determine what's under the cursor, same priority as left-click
            let target = 'find_target: {
                // Check NPCs
                for (id, npc) in &state.npcs {
                    if !npc.is_alive() {
                        continue;
                    }
                    let npc_tile_x = npc.x.round() as i32;
                    let npc_tile_y = npc.y.round() as i32;
                    if npc_tile_x == clicked_tile_x && npc_tile_y == clicked_tile_y {
                        break 'find_target ContextMenuTarget::Npc { id: id.clone() };
                    }
                }

                // Check players (skip self)
                for (id, player) in &state.players {
                    if state.local_player_id.as_ref() == Some(id) {
                        continue;
                    }
                    let player_tile_x = player.x.round() as i32;
                    let player_tile_y = player.y.round() as i32;
                    if player_tile_x == clicked_tile_x && player_tile_y == clicked_tile_y {
                        break 'find_target ContextMenuTarget::Player { id: id.clone() };
                    }
                }

                // Check ground items
                for item in state.ground_items.values() {
                    let (ix, iy) = item.tile_coords();
                    if ix == clicked_tile_x && iy == clicked_tile_y {
                        break 'find_target ContextMenuTarget::GroundItem {
                            id: item.id.clone(),
                        };
                    }
                }

                // Check map objects (trees/rocks)
                if let Some(obj) = state
                    .chunk_manager
                    .get_object_at_exact(clicked_tile_x, clicked_tile_y)
                {
                    let obj_gid = obj.gid;
                    if crate::game::tree_types::is_tree_gid(obj_gid) {
                        break 'find_target ContextMenuTarget::Tree {
                            tile_x: clicked_tile_x,
                            tile_y: clicked_tile_y,
                            gid: obj_gid,
                        };
                    }
                    if crate::game::ore_types::get_ore_info(obj_gid).is_some() {
                        break 'find_target ContextMenuTarget::Rock {
                            tile_x: clicked_tile_x,
                            tile_y: clicked_tile_y,
                            gid: obj_gid,
                        };
                    }
                    // Generic map object (obelisks, waystones, etc.)
                    break 'find_target ContextMenuTarget::MapObject {
                        tile_x: clicked_tile_x,
                        tile_y: clicked_tile_y,
                        gid: obj_gid,
                    };
                }

                // Check gathering markers
                for (i, marker) in state.gathering_markers.iter().enumerate() {
                    if marker.x == clicked_tile_x && marker.y == clicked_tile_y {
                        break 'find_target ContextMenuTarget::GatheringSpot { marker_index: i };
                    }
                }

                // Check farming patches
                if let Some(patch_id) = state
                    .farming_patch_positions
                    .get(&(clicked_tile_x, clicked_tile_y))
                    .cloned()
                {
                    break 'find_target ContextMenuTarget::FarmingPatch { patch_id };
                }

                // Default: empty tile
                ContextMenuTarget::Tile {
                    x: clicked_tile_x,
                    y: clicked_tile_y,
                }
            };

            state.ui_state.context_menu = Some(ContextMenu {
                target,
                x: mx,
                y: my,
            });
        }

        false
    }
}
