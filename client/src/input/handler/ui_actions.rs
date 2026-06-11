use super::*;

impl InputHandler {
    pub(super) fn handle_clickable_ui(
        &mut self,
        state: &mut GameState,
        audio: &mut AudioManager,
        frame: ProcessFrame<'_>,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        let mx = frame.mx;
        let my = frame.my;
        let mouse_clicked = frame.mouse_clicked;
        let mouse_right_clicked = frame.mouse_right_clicked;
        let clicked_element = frame.clicked_element.clone();
        // Handle right-click on chat tab
        if mouse_right_clicked {
            if let Some(ref element) = clicked_element {
                if matches!(element, UiElementId::ChatTabLocal) {
                    state.ui_state.context_menu = Some(ContextMenu {
                        target: ContextMenuTarget::ChatTab,
                        x: mx,
                        y: my,
                    });
                    return true;
                }
            }
        }

        // Handle mouse clicks on quick slots and inventory (always visible when open)
        if let Some(ref element) = clicked_element {
            match element {
                UiElementId::QuickSlot(idx) => {
                    if mouse_clicked {
                        // Unified hotkey bar: activate the binding
                        let cmds = activate_hotkey_slot(state, *idx);
                        commands.extend(cmds);
                    } else if mouse_right_clicked {
                        // Right-click opens context menu for hotkey slot
                        state.ui_state.context_menu = Some(ContextMenu {
                            target: ContextMenuTarget::HotkeySlot(*idx),
                            x: mx,
                            y: my,
                        });
                    }
                    return true;
                }
                UiElementId::InventorySlot(idx) => {
                    if mouse_right_clicked {
                        // Right-click opens context menu (if item exists)
                        if state
                            .inventory
                            .slots
                            .get(*idx)
                            .and_then(|s| s.as_ref())
                            .is_some()
                        {
                            state.ui_state.context_menu = Some(ContextMenu {
                                target: ContextMenuTarget::InventorySlot(*idx),
                                x: mx,
                                y: my,
                            });
                        }
                    }
                    return true;
                }
                UiElementId::SpellSlot(slot_idx) => {
                    if mouse_right_clicked {
                        // Right-click on spell opens context menu for hotkey assignment
                        let spell_id = if *slot_idx < crate::game::spell::SPELLS.len() {
                            Some(crate::game::spell::SPELLS[*slot_idx].id.to_string())
                        } else {
                            let scroll_idx = *slot_idx - crate::game::spell::SPELLS.len();
                            state
                                .scroll_spell_definitions
                                .get(scroll_idx)
                                .filter(|s| state.unlocked_spells.contains(&s.id))
                                .map(|s| s.id.clone())
                        };
                        if let Some(id) = spell_id {
                            state.ui_state.context_menu = Some(ContextMenu {
                                target: ContextMenuTarget::Spell(id),
                                x: mx,
                                y: my,
                            });
                        }
                    }
                    return true;
                }
                UiElementId::EquipmentSlot(slot_type) => {
                    if mouse_right_clicked {
                        // Right-click on equipment slot opens context menu (if something is equipped)
                        let has_item = match slot_type.as_str() {
                            "head" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_head.as_ref())
                                .is_some(),
                            "body" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_body.as_ref())
                                .is_some(),
                            "weapon" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_weapon.as_ref())
                                .is_some(),
                            "back" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_back.as_ref())
                                .is_some(),
                            "feet" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_feet.as_ref())
                                .is_some(),
                            "ring" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_ring.as_ref())
                                .is_some(),
                            "gloves" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_gloves.as_ref())
                                .is_some(),
                            "necklace" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_necklace.as_ref())
                                .is_some(),
                            "belt" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_belt.as_ref())
                                .is_some(),
                            _ => false,
                        };
                        if has_item {
                            state.ui_state.context_menu = Some(ContextMenu {
                                target: ContextMenuTarget::EquipmentSlot(slot_type.clone()),
                                x: mx,
                                y: my,
                            });
                        }
                    }
                    return true;
                }
                UiElementId::CombatStyleButton(idx) => {
                    if mouse_clicked {
                        // Dynamic styles based on equipped weapon type
                        let is_ranged = state
                            .get_local_player()
                            .and_then(|p| p.equipped_weapon.as_ref())
                            .and_then(|wid| state.item_registry.get(wid))
                            .and_then(|def| def.weapon_type.as_ref())
                            .map(|wt| wt == "ranged")
                            .unwrap_or(false);
                        let styles: &[&str] = if is_ranged {
                            &["accurate", "rapid", "longrange"]
                        } else {
                            &["accurate", "aggressive", "defensive", "controlled"]
                        };
                        if let Some(style) = styles.get(*idx) {
                            audio.play_sfx("click");
                            commands.push(InputCommand::SetCombatStyle {
                                style: style.to_string(),
                            });
                            // Optimistically update local state
                            if let Some(local_id) = state.local_player_id.clone() {
                                if let Some(player) = state.players.get_mut(&local_id) {
                                    player.combat_style = style.to_string();
                                }
                            }
                        }
                    }
                    return true;
                }
                UiElementId::AutoRetaliateToggle => {
                    if mouse_clicked {
                        audio.play_sfx("click");
                        let new_val = !state.auto_retaliate;
                        state.auto_retaliate = new_val;
                        commands.push(InputCommand::SetAutoRetaliate { enabled: new_val });
                    }
                    return true;
                }
                UiElementId::CharacterOpenShopButton => {
                    if mouse_clicked {
                        audio.play_sfx("enter");
                        state.ui_state.stall_setup_open = !state.ui_state.stall_setup_open;
                        if state.ui_state.stall_setup_open {
                            state.ui_state.inventory_open = true;
                            state.ui_state.character_panel_open = false;
                        }
                    }
                    return true;
                }
                UiElementId::GoldDisplay => {
                    if mouse_right_clicked && state.inventory.gold > 0 {
                        // Right-click on gold display opens context menu
                        state.ui_state.context_menu = Some(ContextMenu {
                            target: ContextMenuTarget::Gold,
                            x: mx,
                            y: my,
                        });
                    }
                    return true;
                }
                UiElementId::GroundItem(item_id) => {
                    if mouse_clicked {
                        // Left-click on ground item - attempt pickup if within range, or path to it
                        if let Some(local_id) = &state.local_player_id {
                            if let Some(player) = state.players.get(local_id) {
                                if let Some(ground_item) = state.ground_items.get(item_id) {
                                    let dx = ground_item.x - player.x;
                                    let dy = ground_item.y - player.y;
                                    let dist = (dx * dx + dy * dy).sqrt();

                                    const PICKUP_RANGE: f32 = 2.0;
                                    if dist < PICKUP_RANGE {
                                        commands.push(InputCommand::Pickup {
                                            item_id: item_id.clone(),
                                        });
                                    } else {
                                        // Out of range - path to an adjacent tile
                                        let player_x = player.x.round() as i32;
                                        let player_y = player.y.round() as i32;
                                        let item_x = ground_item.x.round() as i32;
                                        let item_y = ground_item.y.round() as i32;

                                        // Build occupied set (other players + NPCs)
                                        let occupied = build_occupied_set(state, true, true);

                                        const MAX_PATH_DISTANCE: i32 = 32;
                                        if let Some((dest, path)) =
                                            pathfinding::find_path_to_adjacent(
                                                (player_x, player_y),
                                                (item_x, item_y),
                                                &state.chunk_manager,
                                                &occupied,
                                                MAX_PATH_DISTANCE,
                                            )
                                        {
                                            state.auto_path = Some(PathState {
                                                path,
                                                current_index: 0,
                                                destination: dest,
                                                pickup_target: Some(item_id.clone()),
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
                    }
                    return true;
                }
                _ => {}
            }
        }

        false
    }
}

impl InputHandler {
    pub(super) fn handle_shortcuts_and_scrolling(
        &mut self,
        state: &mut GameState,
        layout: &UiLayout,
        audio: &mut AudioManager,
        mode: &GameplayMode,
        frame: ProcessFrame<'_>,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        let chat_consuming_keyboard = mode.chat_consuming_keyboard;
        let classic = mode.classic;
        let mx = frame.mx;
        let my = frame.my;
        let mouse_clicked = frame.mouse_clicked;
        let clicked_element = frame.clicked_element.clone();
        // Escape key - clear item selection first, then close panels, then clear target, then open escape menu
        if is_key_pressed(KeyCode::Escape) {
            // Clear inventory item selection first
            if state.ui_state.selected_inventory_slot.is_some() {
                state.ui_state.selected_inventory_slot = None;
                return true;
            }
            // Close hotkey settings popup first
            if state.ui_state.hotkey_settings_open {
                audio.play_sfx("enter");
                state.ui_state.hotkey_settings_open = false;
            } else
            // Check if any panel is open and close it
            if state.ui_state.collection_log_open {
                audio.play_sfx("enter");
                state.ui_state.close_collection_log();
            } else if state.ui_state.inventory_open
                || state.ui_state.character_panel_open
                || state.ui_state.social_open
                || state.ui_state.skills_open
                || state.ui_state.prayer_book_open
                || state.ui_state.quest_log_open
            {
                audio.play_sfx("enter");
                state.ui_state.inventory_open = false;
                state.ui_state.selected_inventory_slot = None;
                state.ui_state.character_panel_open = false;
                state.ui_state.social_open = false;
                state.ui_state.skills_open = false;
                state.ui_state.prayer_book_open = false;
                state.ui_state.close_quest_log();
                // Reset social panel input state
                state.social_state.add_friend_focused = false;
            } else if state.selected_entity_id.is_some() {
                commands.push(InputCommand::ClearTarget);
            } else {
                // No target selected and no panels open - open escape menu
                audio.play_sfx("enter");
                state.ui_state.escape_menu_open = true;
            }
        }

        // Toggle inventory (I key) with mutual exclusivity
        // In classic mode, letter/number keys go to chat input, not hotkeys
        if !classic && !chat_consuming_keyboard && is_key_pressed(KeyCode::I) {
            audio.play_sfx("enter");
            if state.ui_state.inventory_open {
                state.ui_state.inventory_open = false;
            } else {
                state.ui_state.inventory_open = true;
                state.ui_state.character_panel_open = false;
                state.ui_state.social_open = false;
                state.ui_state.skills_open = false;
                state.ui_state.prayer_book_open = false;
                state.ui_state.minimap_panel_open = false;
                state.ui_state.close_quest_log();
                state.ui_state.close_collection_log();
            }
        }

        // Toggle skills panel (T key) with mutual exclusivity
        if !classic && !chat_consuming_keyboard && is_key_pressed(KeyCode::T) {
            audio.play_sfx("enter");
            if state.ui_state.skills_open {
                state.ui_state.skills_open = false;
            } else {
                state.ui_state.skills_open = true;
                state.ui_state.inventory_open = false;
                state.ui_state.character_panel_open = false;
                state.ui_state.social_open = false;
                state.ui_state.prayer_book_open = false;
                state.ui_state.minimap_panel_open = false;
                state.ui_state.close_quest_log();
                state.ui_state.close_collection_log();
            }
        }

        // Chat log scrolling (mouse wheel on desktop) - uses direct bounds check
        // since chat log is not registered for hit detection (allows click-through)
        if state.ui_state.chat_log_visible {
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                let (mx, my) = mouse_position();
                let (vmx, vmy) = screen_to_virtual_coords(mx, my);
                let (_, chat_sh) = virtual_screen_size();
                let scale = state.ui_state.ui_scale;
                let bg_padding = 6.0 * scale;
                let box_bottom = chat_sh - 8.0 * scale; // EXP_BAR_GAP * scale
                let line_height = 18.0 * scale;
                let max_chat_width = if scale >= 2.0 {
                    400.0 * scale - 260.0
                } else {
                    360.0 * scale
                };
                let max_visible_lines: usize = if scale >= 2.0 { 6 } else { 7 };
                let chat_area_h = max_visible_lines as f32 * line_height;
                let chat_bottom_y = box_bottom - bg_padding;
                let chat_top_y = chat_bottom_y - chat_area_h + line_height;
                let over_chat = vmx >= 10.0 - bg_padding
                    && vmx <= 10.0 + max_chat_width + bg_padding
                    && vmy >= chat_top_y - bg_padding
                    && vmy <= box_bottom;
                if over_chat {
                    const SCROLL_SPEED: f32 = 40.0; // Pixels per scroll tick
                    let max_scroll = layout
                        .get_max_scroll(&UiElementId::ChatLogScrollbar)
                        .unwrap_or(0.0);
                    let delta = wheel_y * SCROLL_SPEED;
                    state.ui_state.chat_message_scroll =
                        (state.ui_state.chat_message_scroll + delta).clamp(0.0, max_scroll);
                }
            }
        }

        // Chat log scrollbar drag handling
        if state.ui_state.chat_log_visible {
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::ChatLogScrollbar) {
                let chat_max = layout
                    .get_max_scroll(&UiElementId::ChatLogScrollbar)
                    .unwrap_or(0.0);
                let chat_content_h = chat_max + track_bounds.h;
                let clicked_on = matches!(clicked_element, Some(UiElementId::ChatLogScrollbar));
                crate::ui::scroll::handle_scrollbar_drag_ex(
                    &mut state.ui_state.chat_scroll_drag,
                    &mut state.ui_state.chat_message_scroll,
                    chat_max,
                    track_bounds,
                    chat_content_h,
                    my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                    true, // inverted: thumb at bottom when scroll=0
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.chat_scroll_drag.dragging = false;
            }
        }

        // Inventory grid scrolling (mouse wheel / touch drag)
        if state.ui_state.inventory_open {
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                // Check if hovering over inventory grid or any inventory slot
                let over_inventory = matches!(
                    &state.ui_state.hovered_element,
                    Some(UiElementId::InventoryGridArea) | Some(UiElementId::InventorySlot(_))
                );
                if over_inventory {
                    const SCROLL_SPEED: f32 = 30.0;
                    let max_scroll = layout
                        .get_max_scroll(&UiElementId::InventoryScrollbar)
                        .unwrap_or(0.0);
                    state.ui_state.inventory_scroll_offset =
                        (state.ui_state.inventory_scroll_offset - wheel_y * SCROLL_SPEED)
                            .clamp(0.0, max_scroll);
                }
            }

            // Mouse scrollbar dragging (generic system)
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::InventoryScrollbar) {
                let inv_max_scroll = layout
                    .get_max_scroll(&UiElementId::InventoryScrollbar)
                    .unwrap_or(0.0);
                let inv_content_h = inv_max_scroll + track_bounds.h;
                let clicked_on = matches!(clicked_element, Some(UiElementId::InventoryScrollbar));
                crate::ui::scroll::handle_scrollbar_drag(
                    &mut state.ui_state.inventory_scroll_drag,
                    &mut state.ui_state.inventory_scroll_offset,
                    inv_max_scroll,
                    track_bounds,
                    inv_content_h,
                    my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.inventory_scroll_drag.dragging = false;
            }

            // Touch drag scrolling for mobile
            let all_touches: Vec<Touch> = touches();
            if let Some(tracking_id) = state.ui_state.inventory_touch_scroll_id {
                // We're tracking a touch - update or release
                if let Some(touch) = all_touches.iter().find(|t| t.id == tracking_id) {
                    match touch.phase {
                        TouchPhase::Moved | TouchPhase::Stationary => {
                            let (_, vy) =
                                screen_to_virtual_coords(touch.position.x, touch.position.y);
                            let dy = state.ui_state.inventory_touch_last_y - vy;
                            state.ui_state.inventory_scroll_offset =
                                (state.ui_state.inventory_scroll_offset + dy).max(0.0);
                            state.ui_state.inventory_touch_last_y = vy;
                        }
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            state.ui_state.inventory_touch_scroll_id = None;
                        }
                        _ => {}
                    }
                } else {
                    state.ui_state.inventory_touch_scroll_id = None;
                }
            } else {
                // Look for new touch starting in the inventory grid area
                for touch in &all_touches {
                    if touch.phase == TouchPhase::Started {
                        let (vx, vy) = screen_to_virtual_coords(touch.position.x, touch.position.y);
                        let over_grid = matches!(
                            layout.hit_test(vx, vy),
                            Some(UiElementId::InventoryGridArea)
                                | Some(UiElementId::InventorySlot(_))
                                | Some(UiElementId::InventoryScrollbar)
                        );
                        if over_grid {
                            state.ui_state.inventory_touch_scroll_id = Some(touch.id);
                            state.ui_state.inventory_touch_last_y = vy;
                            break;
                        }
                    }
                }
            }
        } else {
            // Reset tracking when inventory closes
            state.ui_state.inventory_touch_scroll_id = None;
            state.ui_state.inventory_scroll_drag.dragging = false;
        }

        // Toggle character panel (C key) with mutual exclusivity
        if !classic && !chat_consuming_keyboard && is_key_pressed(KeyCode::C) {
            audio.play_sfx("enter");
            if state.ui_state.character_panel_open {
                state.ui_state.character_panel_open = false;
            } else {
                state.ui_state.character_panel_open = true;
                state.ui_state.inventory_open = false;
                state.ui_state.social_open = false;
                state.ui_state.skills_open = false;
                state.ui_state.prayer_book_open = false;
                state.ui_state.minimap_panel_open = false;
            }
        }

        // Toggle prayer book (P key) with mutual exclusivity
        if !classic && !chat_consuming_keyboard && is_key_pressed(KeyCode::P) {
            audio.play_sfx("enter");
            if state.ui_state.prayer_book_open {
                state.ui_state.prayer_book_open = false;
            } else {
                state.ui_state.prayer_book_open = true;
                state.ui_state.inventory_open = false;
                state.ui_state.character_panel_open = false;
                state.ui_state.social_open = false;
                state.ui_state.skills_open = false;
                state.ui_state.minimap_panel_open = false;
            }
        }

        // Toggle expanded minimap panel (M key) — disabled in instances/interiors
        if !classic
            && !chat_consuming_keyboard
            && is_key_pressed(KeyCode::M)
            && state.current_instance.is_none()
        {
            audio.play_sfx("enter");
            state.ui_state.minimap_panel_open = !state.ui_state.minimap_panel_open;
            if state.ui_state.minimap_panel_open {
                state.ui_state.inventory_open = false;
                state.ui_state.character_panel_open = false;
                state.ui_state.social_open = false;
                state.ui_state.skills_open = false;
                state.ui_state.prayer_book_open = false;
                state.ui_state.close_quest_log();
                state.ui_state.chat_panel_open = false;
                state.ui_state.chat_open = false;
                state.ui_state.minimap_panel_zoom = 1.0;
                state.ui_state.minimap_panel_center_x = None;
                state.ui_state.minimap_panel_center_y = None;
            }
            state.ui_state.minimap_panel_dragging = false;
            return true;
        }

        // Use/equip items or cast spells via unified hotkey bar (1-5 keys, disabled in classic mode)
        let quick_slot_keys = [
            (KeyCode::Key1, 0usize),
            (KeyCode::Key2, 1usize),
            (KeyCode::Key3, 2usize),
            (KeyCode::Key4, 3usize),
            (KeyCode::Key5, 4usize),
        ];
        for (key, slot_idx) in quick_slot_keys {
            if !classic && !chat_consuming_keyboard && is_key_pressed(key) {
                let cmds = activate_hotkey_slot(state, slot_idx);
                commands.extend(cmds);
            }
        }

        // Pickup nearest item (F key or touch interact when no NPC nearby)
        let pickup_pressed = !classic && !chat_consuming_keyboard && is_key_pressed(KeyCode::F);
        if pickup_pressed {
            // Get local player position
            if let Some(local_id) = &state.local_player_id {
                if let Some(player) = state.players.get(local_id) {
                    // Find nearest item within pickup range (2 tiles)
                    const PICKUP_RANGE: f32 = 2.0;
                    let mut nearest_item: Option<(String, f32)> = None;

                    for (id, item) in &state.ground_items {
                        let dx = item.x - player.x;
                        let dy = item.y - player.y;
                        let dist = (dx * dx + dy * dy).sqrt();

                        if dist < PICKUP_RANGE {
                            if nearest_item.is_none() || dist < nearest_item.as_ref().unwrap().1 {
                                nearest_item = Some((id.clone(), dist));
                            }
                        }
                    }

                    if let Some((item_id, _)) = nearest_item {
                        commands.push(InputCommand::Pickup { item_id });
                    }
                }
            }
        }

        // Interact with nearest NPC (E key or touch interact button)
        // Touch interact button also picks up items if no NPC nearby
        let interact_pressed = (!classic && !chat_consuming_keyboard && is_key_pressed(KeyCode::E))
            || self.touch_controls.interact_pressed();
        if interact_pressed {
            // If sitting, stand up
            if state.is_sitting {
                commands.push(InputCommand::StandUp);
                state.is_sitting = false;
                if let Some(local_id) = &state.local_player_id {
                    if let Some(player) = state.players.get_mut(local_id) {
                        player.stand_up();
                    }
                }
            } else if let Some(local_id) = &state.local_player_id {
                // Check for nearby chairs first, then NPCs
                let mut sat_on_chair = false;
                if let Some(player) = state.players.get(local_id) {
                    let px = player.x.round() as i32;
                    let py = player.y.round() as i32;
                    let mut nearest_chair: Option<((i32, i32), i32)> = None;
                    for &(cx, cy) in &state.chair_positions {
                        let cdx = (px - cx).abs();
                        let cdy = (py - cy).abs();
                        let dist = cdx.max(cdy);
                        if dist <= 1 {
                            if nearest_chair.is_none() || dist < nearest_chair.unwrap().1 {
                                nearest_chair = Some(((cx, cy), dist));
                            }
                        }
                    }
                    if let Some(((cx, cy), _)) = nearest_chair {
                        commands.push(InputCommand::SitChair {
                            tile_x: cx,
                            tile_y: cy,
                        });
                        sat_on_chair = true;
                    }
                }
                if !sat_on_chair {
                    if let Some(player) = state.players.get(local_id) {
                        // Find nearest NPC within interaction range (2.5 tiles)
                        const INTERACT_RANGE: f32 = 2.5;
                        let mut nearest_npc: Option<(String, f32)> = None;

                        for (id, npc) in &state.npcs {
                            // Only interact with alive NPCs
                            if !npc.is_alive() {
                                continue;
                            }

                            let dx = npc.x - player.x;
                            let dy = npc.y - player.y;
                            let dist = (dx * dx + dy * dy).sqrt();

                            if dist < INTERACT_RANGE {
                                if nearest_npc.is_none() || dist < nearest_npc.as_ref().unwrap().1 {
                                    nearest_npc = Some((id.clone(), dist));
                                }
                            }
                        }

                        if let Some((npc_id, _)) = nearest_npc {
                            log::info!("Interacting with NPC: {}", npc_id);
                            // Check if NPC is an altar or station
                            if let Some(npc) = state.npcs.get(&npc_id) {
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
                                } else {
                                    commands.push(InputCommand::Interact { npc_id });
                                }
                            } else {
                                commands.push(InputCommand::Interact { npc_id });
                            }
                        } else if self.touch_controls.interact_pressed() {
                            // Touch interact fallback: pickup item if no NPC nearby
                            const PICKUP_RANGE: f32 = 2.0;
                            let mut nearest_item: Option<(String, f32)> = None;
                            for (id, item) in &state.ground_items {
                                let dx = item.x - player.x;
                                let dy = item.y - player.y;
                                let dist = (dx * dx + dy * dy).sqrt();
                                if dist < PICKUP_RANGE {
                                    if nearest_item.is_none()
                                        || dist < nearest_item.as_ref().unwrap().1
                                    {
                                        nearest_item = Some((id.clone(), dist));
                                    }
                                }
                            }
                            if let Some((item_id, _)) = nearest_item {
                                commands.push(InputCommand::Pickup { item_id });
                            }
                        }
                    }
                }
            }
        }

        // Toggle quest log (Q key) with mutual exclusivity
        if !classic && !chat_consuming_keyboard && is_key_pressed(KeyCode::Q) {
            audio.play_sfx("enter");
            if state.ui_state.quest_log_open {
                state.ui_state.close_quest_log();
            } else {
                state.ui_state.quest_log_open = true;
                state.ui_state.quest_log_scroll = 0.0;
                state.ui_state.selected_quest_id = None;
                state.ui_state.inventory_open = false;
                state.ui_state.character_panel_open = false;
                state.ui_state.social_open = false;
                state.ui_state.skills_open = false;
                state.ui_state.prayer_book_open = false;
                state.ui_state.minimap_panel_open = false;
                state.ui_state.close_collection_log();
            }
        }

        // Toggle collection log (V key) with mutual exclusivity
        if !classic && !chat_consuming_keyboard && is_key_pressed(KeyCode::V) {
            audio.play_sfx("enter");
            if state.ui_state.collection_log_open {
                state.ui_state.close_collection_log();
            } else {
                state.ui_state.close_quest_log();
                state.ui_state.inventory_open = false;
                state.ui_state.character_panel_open = false;
                state.ui_state.social_open = false;
                state.ui_state.skills_open = false;
                state.ui_state.prayer_book_open = false;
                state.ui_state.minimap_panel_open = false;
                state.ui_state.collection_log_open = true;
            }
        }

        // Collection log scrolling
        if state.ui_state.collection_log_open {
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                const SCROLL_SPEED: f32 = 30.0;
                match &state.ui_state.hovered_element {
                    Some(UiElementId::CollectionLogSidebarScrollArea)
                    | Some(UiElementId::CollectionLogCategoryHeader(_))
                    | Some(UiElementId::CollectionLogSubcategoryEntry(_)) => {
                        let max_scroll = layout
                            .get_max_scroll(&UiElementId::CollectionLogSidebarScrollbar)
                            .unwrap_or(2000.0);
                        state.ui_state.collection_log_sidebar_scroll =
                            (state.ui_state.collection_log_sidebar_scroll - wheel_y * SCROLL_SPEED)
                                .clamp(0.0, max_scroll);
                    }
                    Some(UiElementId::CollectionLogGridScrollArea)
                    | Some(UiElementId::CollectionLogGridItem(_)) => {
                        let max_scroll = layout
                            .get_max_scroll(&UiElementId::CollectionLogGridScrollbar)
                            .unwrap_or(2000.0);
                        state.ui_state.collection_log_grid_scroll =
                            (state.ui_state.collection_log_grid_scroll - wheel_y * SCROLL_SPEED)
                                .clamp(0.0, max_scroll);
                    }
                    _ => {}
                }
            }
        }

        // Quest log scrolling
        if state.ui_state.quest_log_open {
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                const SCROLL_SPEED: f32 = 30.0;
                let max_scroll = layout
                    .get_max_scroll(&UiElementId::QuestLogScrollbar)
                    .unwrap_or(0.0);
                state.ui_state.quest_log_scroll = (state.ui_state.quest_log_scroll
                    - wheel_y * SCROLL_SPEED)
                    .clamp(0.0, max_scroll);
            }

            // Scrollbar drag handling
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::QuestLogScrollbar) {
                let ql_max_scroll = layout
                    .get_max_scroll(&UiElementId::QuestLogScrollbar)
                    .unwrap_or(0.0);
                let ql_content_h = ql_max_scroll + track_bounds.h;
                let clicked_on = matches!(clicked_element, Some(UiElementId::QuestLogScrollbar));
                crate::ui::scroll::handle_scrollbar_drag(
                    &mut state.ui_state.quest_log_scroll_drag,
                    &mut state.ui_state.quest_log_scroll,
                    ql_max_scroll,
                    track_bounds,
                    ql_content_h,
                    my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.quest_log_scroll_drag.dragging = false;
            }
        } else {
            state.ui_state.quest_log_scroll_drag.dragging = false;
        }
        false
    }
}
