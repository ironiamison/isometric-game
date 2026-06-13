use super::*;

impl InputHandler {
    pub(super) fn handle_menu_and_escape(
        &mut self,
        state: &mut GameState,
        layout: &UiLayout,
        audio: &mut AudioManager,
        frame: ProcessFrame<'_>,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        let mx = frame.mx;
        let my = frame.my;
        let mouse_clicked = frame.mouse_clicked;
        let clicked_element = frame.clicked_element.clone();
        // Handle menu button clicks (always visible, handle before modal UIs)
        if mouse_clicked {
            if let Some(ref element) = clicked_element {
                match element {
                    UiElementId::MenuButtonInventory => {
                        audio.play_sfx("enter");
                        // Toggle inventory panel, close others if opening
                        if state.ui_state.inventory_open {
                            state.ui_state.inventory_open = false;
                        } else {
                            state.ui_state.inventory_open = true;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.prayer_book_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.escape_menu_open = false;
                            state.ui_state.close_quest_log();
                            state.ui_state.close_collection_log();
                        }
                        return true;
                    }
                    UiElementId::MenuButtonCharacter => {
                        audio.play_sfx("enter");
                        // Toggle character panel, close others if opening
                        if state.ui_state.character_panel_open {
                            state.ui_state.character_panel_open = false;
                        } else {
                            state.ui_state.character_panel_open = true;
                            state.ui_state.inventory_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.prayer_book_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.escape_menu_open = false;
                            state.ui_state.close_quest_log();
                            state.ui_state.close_collection_log();
                        }
                        return true;
                    }
                    UiElementId::MenuButtonSocial => {
                        audio.play_sfx("enter");
                        // Toggle social panel, close others if opening
                        if state.ui_state.social_open {
                            state.ui_state.social_open = false;
                            state.social_state.add_friend_focused = false;
                        } else {
                            state.ui_state.social_open = true;
                            state.ui_state.inventory_open = false;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.prayer_book_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.escape_menu_open = false;
                            state.ui_state.close_quest_log();
                            state.ui_state.close_collection_log();
                            // Request online players list when opening panel
                            commands.push(InputCommand::GetOnlinePlayers);
                        }
                        return true;
                    }
                    UiElementId::MenuButtonSkills => {
                        audio.play_sfx("enter");
                        // Toggle skills panel, close others if opening
                        if state.ui_state.skills_open {
                            state.ui_state.skills_open = false;
                        } else {
                            state.ui_state.skills_open = true;
                            state.ui_state.inventory_open = false;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.prayer_book_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.escape_menu_open = false;
                            state.ui_state.close_quest_log();
                            state.ui_state.close_collection_log();
                        }
                        return true;
                    }
                    UiElementId::MenuButtonPrayer => {
                        audio.play_sfx("enter");
                        // Toggle prayer book, close others if opening
                        if state.ui_state.prayer_book_open {
                            state.ui_state.prayer_book_open = false;
                        } else {
                            state.ui_state.prayer_book_open = true;
                            state.ui_state.inventory_open = false;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.escape_menu_open = false;
                            state.ui_state.close_quest_log();
                            state.ui_state.close_collection_log();
                        }
                        return true;
                    }
                    UiElementId::MenuButtonQuest => {
                        audio.play_sfx("enter");
                        // Toggle quest log, close others if opening
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
                            state.ui_state.escape_menu_open = false;
                            state.ui_state.close_collection_log();
                        }
                        return true;
                    }
                    UiElementId::MenuButtonSettings => {
                        audio.play_sfx("enter");
                        // Toggle settings panel, close others if opening
                        if state.ui_state.escape_menu_open {
                            state.ui_state.escape_menu_open = false;
                        } else {
                            state.ui_state.escape_menu_open = true;
                            state.ui_state.inventory_open = false;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.prayer_book_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.close_quest_log();
                            state.ui_state.close_collection_log();
                        }
                        return true;
                    }
                    UiElementId::MenuButtonToggle => {
                        audio.play_sfx("enter");
                        state.ui_state.mobile_menu_expanded = !state.ui_state.mobile_menu_expanded;
                        return true;
                    }
                    UiElementId::ChatButton => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_panel_open = !state.ui_state.chat_panel_open;
                        if state.ui_state.chat_panel_open {
                            state.ui_state.chat_active_tab = ChatChannel::Local;
                            mark_chat_channel_as_read(state, ChatChannel::Local);
                            state.ui_state.chat_message_scroll = 0.0;
                            // Close other panels
                            state.ui_state.inventory_open = false;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.prayer_book_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.close_quest_log();
                        }
                    }
                    UiElementId::MinimapToggle => {
                        // Click on minimap preview: pathfind to the clicked world position
                        // Use the inner map rect (excluding title bar) to match renderer
                        let s = state.ui_state.ui_scale;
                        let preview_rect = minimap_preview_rect(s);
                        let map_rect = Rect::new(
                            preview_rect.x + 6.0 * s,
                            preview_rect.y + 24.0 * s,
                            preview_rect.w - 12.0 * s,
                            preview_rect.h - 30.0 * s,
                        );

                        // Only pathfind if click is within the actual map area
                        let in_map = mx >= map_rect.x
                            && mx <= map_rect.x + map_rect.w
                            && my >= map_rect.y
                            && my <= map_rect.y + map_rect.h;

                        if in_map {
                            if let Some(player) = state.get_local_player() {
                                let player_x = player.server_x.round() as i32;
                                let player_y = player.server_y.round() as i32;

                                // Use server position for bounds so coordinate mapping
                                // is consistent with pathfinding start point
                                let half_span =
                                    CHUNK_SIZE as f32 * (MINIMAP_VISIBLE_CHUNK_RADIUS + 0.5);
                                let bounds = MinimapBounds {
                                    min_x: player.server_x - half_span,
                                    min_y: player.server_y - half_span,
                                    max_x: player.server_x + half_span,
                                    max_y: player.server_y + half_span,
                                };

                                let (world_x, world_y) =
                                    minimap_screen_to_world(bounds, map_rect, mx, my);
                                let tile_x = world_x.round() as i32;
                                let tile_y = world_y.round() as i32;

                                let dist = (tile_x - player_x).abs().max((tile_y - player_y).abs());

                                if dist > 0
                                    && state
                                        .chunk_manager
                                        .is_walkable(tile_x as f32, tile_y as f32)
                                {
                                    // Cancel any current auto-action
                                    if state.auto_action_state.is_some() {
                                        state.auto_action_state = None;
                                        commands.push(InputCommand::CancelAutoAction);
                                    }
                                    self.suppress_move_until = 0.0;

                                    let occupied = build_occupied_set(state, true, true);
                                    let path_limit = dist.min(64);

                                    // Use splice-aware pathfinding to preserve in-progress step
                                    if let Some(path) = find_path_with_committed_step_splice(
                                        state,
                                        (player_x, player_y),
                                        (tile_x, tile_y),
                                        &occupied,
                                        path_limit,
                                    ) {
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
                                        self.reset_auto_path_motion_state();
                                    }
                                }
                            }
                        }
                        return true;
                    }
                    UiElementId::MinimapClose => {
                        audio.play_sfx("enter");
                        state.ui_state.minimap_panel_open = false;
                        state.ui_state.minimap_panel_dragging = false;
                        return true;
                    }
                    UiElementId::MinimapPanel | UiElementId::MinimapMarker(_) => {
                        // Handled by dedicated minimap modal logic below.
                    }
                    UiElementId::ChatTabLocal => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_active_tab = ChatChannel::Local;
                        mark_chat_channel_as_read(state, ChatChannel::Local);
                        state.ui_state.chat_message_scroll = 0.0;
                    }
                    UiElementId::ChatTabGlobal => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_active_tab = ChatChannel::Global;
                        mark_chat_channel_as_read(state, ChatChannel::Global);
                        state.ui_state.chat_message_scroll = 0.0;
                    }
                    UiElementId::ChatTabSystem => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_active_tab = ChatChannel::System;
                        mark_chat_channel_as_read(state, ChatChannel::System);
                        state.ui_state.chat_message_scroll = 0.0;
                    }
                    UiElementId::ChatSendButton => {
                        let text = state.ui_state.chat_input.trim().to_string();
                        // Determine channel: ~ prefix forces global, otherwise match active tab
                        // System tab sends to public channel
                        let (send_text, channel) = if let Some(global_text) = text.strip_prefix('~')
                        {
                            let trimmed = global_text.trim().to_string();
                            (trimmed, "global".to_string())
                        } else {
                            let ch = match state.ui_state.chat_active_tab {
                                ChatChannel::Global => "global",
                                _ => "public",
                            };
                            (text.clone(), ch.to_string())
                        };
                        if !send_text.is_empty() {
                            audio.play_sfx("send_message");
                            commands.push(InputCommand::Chat {
                                text: send_text,
                                channel,
                            });
                        }
                        state.ui_state.chat_input.clear();
                        state.ui_state.chat_cursor = 0;
                    }
                    UiElementId::ChatInputField => {
                        state.ui_state.chat_open = true;
                        #[cfg(target_os = "android")]
                        macroquad::miniquad::window::show_keyboard(true);
                    }
                    UiElementId::ChatCloseButton => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_panel_open = false;
                        state.ui_state.chat_open = false;
                        #[cfg(target_os = "android")]
                        macroquad::miniquad::window::show_keyboard(false);
                    }
                    UiElementId::ChatPanelBackground => {
                        // Tapping outside the panel content closes the chat panel
                        state.ui_state.chat_panel_open = false;
                        state.ui_state.chat_open = false;
                        #[cfg(target_os = "android")]
                        macroquad::miniquad::window::show_keyboard(false);
                    }
                    // Social panel scroll area - handle touch scrolling
                    UiElementId::SocialScrollArea => {
                        // Touch scroll handled below, just suppress click
                    }
                    // Social panel handlers
                    UiElementId::SocialTabNearby => {
                        audio.play_sfx("enter");
                        state.social_state.active_tab = crate::game::SocialTab::Nearby;
                    }
                    UiElementId::SocialTabOnline => {
                        audio.play_sfx("enter");
                        state.social_state.active_tab = crate::game::SocialTab::Online;
                        // Request online players list
                        commands.push(InputCommand::GetOnlinePlayers);
                    }
                    UiElementId::SocialTabFriends => {
                        audio.play_sfx("enter");
                        state.social_state.active_tab = crate::game::SocialTab::Friends;
                    }
                    UiElementId::SocialPlayerRow(idx) => {
                        // Send friend request to this player (from nearby or online list)
                        audio.play_sfx("enter");
                        let player_name = match state.social_state.active_tab {
                            crate::game::SocialTab::Nearby => {
                                // Get player from nearby list (state.players minus local player)
                                let local_id = state.local_player_id.as_ref();
                                let nearby: Vec<_> = state
                                    .players
                                    .values()
                                    .filter(|p| Some(&p.id) != local_id)
                                    .collect();
                                nearby.get(*idx).map(|p| p.name.clone())
                            }
                            crate::game::SocialTab::Online => state
                                .social_state
                                .online_players
                                .get(*idx)
                                .map(|p| p.name.clone()),
                            _ => None,
                        };
                        if let Some(name) = player_name {
                            commands.push(InputCommand::SendFriendRequest { target_name: name });
                        }
                    }
                    UiElementId::SocialRequestAccept(idx) => {
                        audio.play_sfx("enter");
                        if let Some(request) =
                            state.social_state.pending_requests.get(*idx).cloned()
                        {
                            let requester_id = request.from_id;
                            let requester_name = request.from_name.clone();
                            commands.push(InputCommand::AcceptFriendRequest { requester_id });
                            // Remove from pending list immediately for responsive UI
                            state.social_state.pending_requests.remove(*idx);
                            state.social_state.pending_request_count =
                                state.social_state.pending_requests.len();
                            // Also add to friends list immediately (they're online since they sent the request)
                            if !state
                                .social_state
                                .friends
                                .iter()
                                .any(|f| f.id == requester_id)
                            {
                                state.social_state.friends.push(crate::game::FriendInfo {
                                    id: requester_id,
                                    name: requester_name,
                                    online: true,
                                });
                                // Sort friends list (online first)
                                state.social_state.friends.sort_by(|a, b| {
                                    match (a.online, b.online) {
                                        (true, false) => std::cmp::Ordering::Less,
                                        (false, true) => std::cmp::Ordering::Greater,
                                        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                                    }
                                });
                            }
                        }
                    }
                    UiElementId::SocialRequestDecline(idx) => {
                        audio.play_sfx("enter");
                        if let Some(request) = state.social_state.pending_requests.get(*idx) {
                            let requester_id = request.from_id;
                            commands.push(InputCommand::DeclineFriendRequest { requester_id });
                            // Remove from local list immediately
                            state.social_state.pending_requests.remove(*idx);
                            state.social_state.pending_request_count =
                                state.social_state.pending_requests.len();
                        }
                    }
                    UiElementId::SocialRemoveFriend(idx) => {
                        audio.play_sfx("enter");
                        if let Some(friend) = state.social_state.friends.get(*idx) {
                            let friend_id = friend.id;
                            commands.push(InputCommand::RemoveFriend { friend_id });
                            // Remove from local list immediately
                            state.social_state.friends.remove(*idx);
                        }
                    }
                    UiElementId::SocialAddFriendButton => {
                        // Send friend request by name
                        let name = state.social_state.add_friend_input.trim().to_string();
                        if !name.is_empty() {
                            audio.play_sfx("enter");
                            commands.push(InputCommand::SendFriendRequest { target_name: name });
                            state.social_state.add_friend_input.clear();
                            state.social_state.add_friend_focused = false;
                            #[cfg(target_os = "android")]
                            macroquad::miniquad::window::show_keyboard(false);
                        }
                    }
                    UiElementId::SocialAddFriendInput => {
                        // Focus the input for typing
                        state.social_state.add_friend_focused = true;
                        #[cfg(target_os = "android")]
                        macroquad::miniquad::window::show_keyboard(true);
                    }
                    // Skills panel - clicking Prayer skill opens prayer book
                    UiElementId::SkillSlot(5) => {
                        // Index 5 is Prayer skill - open prayer book on Prayers tab
                        audio.play_sfx("enter");
                        state.ui_state.prayer_book_open = !state.ui_state.prayer_book_open;
                        if state.ui_state.prayer_book_open {
                            state.ui_state.prayer_spell_tab = 0; // Open to prayers tab
                            state.ui_state.skills_open = false;
                        }
                    }
                    UiElementId::SkillSlot(6) => {
                        // Index 6 is Magic skill - open prayer/spell panel on Spells tab
                        audio.play_sfx("enter");
                        state.ui_state.prayer_book_open = !state.ui_state.prayer_book_open;
                        if state.ui_state.prayer_book_open {
                            state.ui_state.prayer_spell_tab = 1; // Open to spells tab
                            state.ui_state.skills_open = false;
                        }
                    }
                    // Prayer/Spell help buttons
                    UiElementId::PrayerHelpButton => {
                        audio.play_sfx("enter");
                        state.ui_state.prayer_help_open = true;
                    }
                    UiElementId::SpellHelpButton => {
                        audio.play_sfx("enter");
                        state.ui_state.spell_help_open = true;
                    }
                    UiElementId::PrayerHelpClose => {
                        audio.play_sfx("enter");
                        state.ui_state.prayer_help_open = false;
                    }
                    UiElementId::SpellHelpClose => {
                        audio.play_sfx("enter");
                        state.ui_state.spell_help_open = false;
                    }
                    // Prayer/Spell tab switching
                    UiElementId::PrayerSpellTab(tab_idx) => {
                        audio.play_sfx("enter");
                        state.ui_state.prayer_spell_tab = *tab_idx;
                        state.ui_state.prayer_help_open = false;
                        state.ui_state.spell_help_open = false;
                    }
                    // Spell slot handlers (spell panel — click to assign)
                    UiElementId::SpellSlot(_slot_idx) => {
                        audio.play_sfx("enter");
                    }
                    // Hotkey bar preset cycling and settings
                    UiElementId::HotkeyPresetUp => {
                        audio.play_sfx("enter");
                        state.ui_state.hotkey_bar.cycle_up();
                        save_current_ui_settings(state);
                    }
                    UiElementId::HotkeyPresetDown => {
                        audio.play_sfx("enter");
                        state.ui_state.hotkey_bar.cycle_down();
                        save_current_ui_settings(state);
                    }
                    UiElementId::HotkeySettingsCog => {
                        audio.play_sfx("enter");
                        state.ui_state.hotkey_settings_open = !state.ui_state.hotkey_settings_open;
                    }
                    UiElementId::HotkeySettingsPresetTab(tab_idx) => {
                        audio.play_sfx("enter");
                        state.ui_state.hotkey_bar.active_preset = *tab_idx;
                        save_current_ui_settings(state);
                    }
                    UiElementId::HotkeySettingsSlotClear(slot_idx) => {
                        audio.play_sfx("enter");
                        state.ui_state.hotkey_bar.active_mut().slots[*slot_idx] =
                            crate::game::hotkey::HotkeySlotBinding::Empty;
                        save_current_ui_settings(state);
                    }
                    // Prayer panel handlers
                    UiElementId::PrayerSlot(slot_idx) => {
                        // Toggle prayer at this slot
                        if *slot_idx < crate::game::prayer::PRAYERS.len() {
                            let prayer = &crate::game::prayer::PRAYERS[*slot_idx];
                            let prayer_level = state
                                .get_local_player()
                                .map(|p| p.skills.prayer.level)
                                .unwrap_or(1);

                            // Check if player meets level requirement
                            if prayer_level >= prayer.level_req {
                                // Check if we have prayer points (can only activate if we have points)
                                let is_active =
                                    state.active_prayers.contains(&prayer.id.to_string());
                                if is_active || state.prayer_points > 0 {
                                    audio.play_sfx("enter");
                                    commands.push(InputCommand::TogglePrayer {
                                        prayer_id: prayer.id.to_string(),
                                    });
                                } else {
                                    // No prayer points, play error sound
                                    audio.play_sfx("error");
                                }
                            } else {
                                // Level too low, play error sound
                                audio.play_sfx("error");
                            }
                        }
                    }
                    UiElementId::QuestLogEntry(idx) => {
                        audio.play_sfx("enter");
                        // Rebuild sorted quest list matching render_quest_log order
                        let mut sorted: Vec<&QuestCatalogEntry> =
                            state.ui_state.quest_catalog.iter().collect();
                        sorted.sort_by(|a, b| {
                            let sa = quest_status_order(&a.quest_id, &state.ui_state);
                            let sb = quest_status_order(&b.quest_id, &state.ui_state);
                            sa.cmp(&sb).then(a.name.cmp(&b.name))
                        });
                        if let Some(entry) = sorted.get(*idx) {
                            state.ui_state.selected_quest_id = Some(entry.quest_id.clone());
                            state.ui_state.quest_log_scroll = 0.0;
                        }
                    }
                    UiElementId::QuestDetailBack => {
                        audio.play_sfx("enter");
                        state.ui_state.selected_quest_id = None;
                        state.ui_state.quest_log_scroll = 0.0;
                    }
                    UiElementId::CollectionLogLink => {
                        audio.play_sfx("enter");
                        state.ui_state.close_quest_log();
                        state.ui_state.inventory_open = false;
                        state.ui_state.character_panel_open = false;
                        state.ui_state.social_open = false;
                        state.ui_state.skills_open = false;
                        state.ui_state.prayer_book_open = false;
                        state.ui_state.minimap_panel_open = false;
                        state.ui_state.collection_log_open = true;
                    }
                    UiElementId::CollectionLogClose => {
                        audio.play_sfx("enter");
                        state.ui_state.close_collection_log();
                    }
                    UiElementId::CollectionLogCategoryHeader(idx) => {
                        audio.play_sfx("enter");
                        let categories =
                            ["monster_drops", "boss_rewards", "skilling", "quest_rewards"];
                        if let Some(cat) = categories.get(*idx) {
                            if state.ui_state.collection_log_selected_category.as_deref()
                                == Some(*cat)
                            {
                                // Collapse - deselect
                                state.ui_state.collection_log_selected_category = None;
                                state.ui_state.collection_log_selected_subcategory = None;
                            } else {
                                // Expand
                                state.ui_state.collection_log_selected_category =
                                    Some(cat.to_string());
                                state.ui_state.collection_log_selected_subcategory = None;
                            }
                            state.ui_state.collection_log_grid_scroll = 0.0;
                        }
                    }
                    UiElementId::CollectionLogSubcategoryEntry(idx) => {
                        audio.play_sfx("enter");
                        // Rebuild sorted subcategory list matching render order
                        if let Some(ref category) =
                            state.ui_state.collection_log_selected_category.clone()
                        {
                            let mut subcats_map: std::collections::HashMap<&str, usize> =
                                std::collections::HashMap::new();
                            for (_, src, detail) in &state.ui_state.collection_log_definitions {
                                if src == category {
                                    *subcats_map.entry(detail.as_str()).or_insert(0) += 1;
                                }
                            }
                            let mut sorted: Vec<&str> = subcats_map.keys().copied().collect();
                            sorted.sort();
                            if let Some(name) = sorted.get(*idx) {
                                state.ui_state.collection_log_selected_subcategory =
                                    Some(name.to_string());
                                state.ui_state.collection_log_grid_scroll = 0.0;
                            }
                        }
                    }
                    UiElementId::CollectionLogGridItem(_) => {
                        // No action on click - hover tooltip only
                    }
                    _ => {
                        // Clicking elsewhere unfocuses the add friend input
                        if state.social_state.add_friend_focused {
                            state.social_state.add_friend_focused = false;
                            #[cfg(target_os = "android")]
                            macroquad::miniquad::window::show_keyboard(false);
                        }
                    }
                }
            }
        }

        // Handle escape menu
        if state.ui_state.escape_menu_open {
            // Handle slider dragging - continue updating while mouse is held
            if state.ui_state.settings_slider_dragging.is_some() {
                if is_mouse_button_down(MouseButton::Left) {
                    let (mouse_x, _) = mouse_position();
                    match state.ui_state.settings_slider_dragging {
                        Some(UiElementId::EscapeMenuMusicSlider) => {
                            if let Some(slider_elem) = layout
                                .elements
                                .iter()
                                .find(|e| e.id == UiElementId::EscapeMenuMusicSlider)
                            {
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let volume = (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.audio_volume = volume;
                                audio.set_music_volume(volume);
                            }
                        }
                        Some(UiElementId::EscapeMenuSfxSlider) => {
                            if let Some(slider_elem) = layout
                                .elements
                                .iter()
                                .find(|e| e.id == UiElementId::EscapeMenuSfxSlider)
                            {
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let volume = (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.audio_sfx_volume = volume;
                                audio.set_sfx_volume(volume);
                            }
                        }
                        Some(UiElementId::EscapeMenuUiScaleSlider) => {
                            if let Some(slider_elem) = layout
                                .elements
                                .iter()
                                .find(|e| e.id == UiElementId::EscapeMenuUiScaleSlider)
                            {
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let normalized =
                                    (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.ui_scale = 0.75 + normalized * 1.25;
                            }
                        }
                        _ => {}
                    }
                    return true;
                } else {
                    // Mouse released - stop dragging and save settings
                    save_current_ui_settings(state);
                    state.ui_state.settings_slider_dragging = None;
                }
            }

            // Handle mouse clicks on escape menu elements
            if let Some(ref element) = clicked_element {
                if mouse_clicked {
                    match element {
                        UiElementId::EscapeMenuZoom05x => {
                            audio.play_sfx("enter");
                            state.camera.zoom = 0.5;
                            save_current_ui_settings(state);
                            return true;
                        }
                        UiElementId::EscapeMenuZoom1x => {
                            audio.play_sfx("enter");
                            state.camera.zoom = 1.0;
                            save_current_ui_settings(state);
                            return true;
                        }
                        UiElementId::EscapeMenuZoom2x => {
                            audio.play_sfx("enter");
                            state.camera.zoom = 2.0;
                            save_current_ui_settings(state);
                            return true;
                        }
                        UiElementId::EscapeMenuMusicSlider => {
                            // Start dragging and set initial value
                            state.ui_state.settings_slider_dragging =
                                Some(UiElementId::EscapeMenuMusicSlider);
                            if let Some(slider_elem) = layout
                                .elements
                                .iter()
                                .find(|e| e.id == UiElementId::EscapeMenuMusicSlider)
                            {
                                let (mouse_x, _) = mouse_position();
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let volume = (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.audio_volume = volume;
                                audio.set_music_volume(volume);
                            }
                            return true;
                        }
                        UiElementId::EscapeMenuSfxSlider => {
                            // Start dragging and set initial value
                            state.ui_state.settings_slider_dragging =
                                Some(UiElementId::EscapeMenuSfxSlider);
                            if let Some(slider_elem) = layout
                                .elements
                                .iter()
                                .find(|e| e.id == UiElementId::EscapeMenuSfxSlider)
                            {
                                let (mouse_x, _) = mouse_position();
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let volume = (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.audio_sfx_volume = volume;
                                audio.set_sfx_volume(volume);
                            }
                            return true;
                        }
                        UiElementId::EscapeMenuUiScaleSlider => {
                            // Start dragging and set initial value
                            state.ui_state.settings_slider_dragging =
                                Some(UiElementId::EscapeMenuUiScaleSlider);
                            if let Some(slider_elem) = layout
                                .elements
                                .iter()
                                .find(|e| e.id == UiElementId::EscapeMenuUiScaleSlider)
                            {
                                let (mouse_x, _) = mouse_position();
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let normalized =
                                    (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.ui_scale = 0.75 + normalized * 1.25;
                            }
                            return true;
                        }
                        UiElementId::EscapeMenuMusicMuteToggle => {
                            audio.play_sfx("enter");
                            audio.toggle_music_mute();
                            state.ui_state.music_muted = audio.is_music_muted();
                            return true;
                        }
                        UiElementId::EscapeMenuSfxMuteToggle => {
                            audio.play_sfx("enter");
                            audio.toggle_sfx_mute();
                            state.ui_state.sfx_muted = audio.is_sfx_muted();
                            return true;
                        }
                        UiElementId::EscapeMenuShiftDropToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.shift_drop_enabled = !state.ui_state.shift_drop_enabled;
                            save_current_ui_settings(state);
                            return true;
                        }
                        UiElementId::EscapeMenuChatLogToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.chat_log_visible = !state.ui_state.chat_log_visible;
                            save_current_ui_settings(state);
                            return true;
                        }
                        UiElementId::EscapeMenuChatBgToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.chat_log_background =
                                !state.ui_state.chat_log_background;
                            save_current_ui_settings(state);
                            return true;
                        }
                        UiElementId::EscapeMenuTapPathfindToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.tap_to_pathfind = !state.ui_state.tap_to_pathfind;
                            save_current_ui_settings(state);
                            return true;
                        }
                        UiElementId::EscapeMenuJoystickToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.use_joystick = !state.ui_state.use_joystick;
                            save_current_ui_settings(state);
                            return true;
                        }
                        UiElementId::EscapeMenuGraphicsToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.graphics_low = !state.ui_state.graphics_low;
                            save_current_ui_settings(state);
                            return true;
                        }
                        UiElementId::EscapeMenuControlSchemeToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.classic_controls = !state.ui_state.classic_controls;
                            if state.ui_state.classic_controls {
                                state.ui_state.chat_open = true;
                                state.ui_state.chat_cursor =
                                    state.ui_state.chat_input.chars().count();
                            } else {
                                state.ui_state.chat_open = false;
                            }
                            crate::settings::save_classic_controls(state.ui_state.classic_controls);
                            return true;
                        }
                        UiElementId::EscapeMenuDisconnect => {
                            audio.play_sfx("enter");
                            state.disconnect_requested = true;
                            state.ui_state.escape_menu_open = false;
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            // Escape closes settings panel
            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.escape_menu_open = false;
                return true;
            }
        }

        false
    }
}
