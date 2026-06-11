use super::*;

impl InputHandler {
    pub(super) fn handle_dialogue_and_altar(
        &mut self,
        state: &mut GameState,
        layout: &UiLayout,
        audio: &mut AudioManager,
        frame: ProcessFrame<'_>,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        let my = frame.my;
        let mouse_clicked = frame.mouse_clicked;
        let clicked_element = frame.clicked_element.clone();
        if Self::handle_modal_panels(
            state,
            layout,
            clicked_element.as_ref(),
            mouse_clicked,
            commands,
        ) {
            return true;
        }

        // Handle altar panel input
        if state.ui_state.altar_panel.is_some() {
            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.altar_panel = None;
                return true;
            }

            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::AltarOfferAll(idx) => {
                            let altar_npc_id = state
                                .ui_state
                                .altar_panel
                                .as_ref()
                                .unwrap()
                                .altar_npc_id
                                .clone();
                            // Build bone rows to find item_id at index (mirrors renderer logic: dedup by item_id)
                            let mut bone_items: Vec<String> = Vec::new();
                            for slot in state.inventory.slots.iter().flatten() {
                                if !slot.item_id.contains("bones") {
                                    continue;
                                }
                                let item_def =
                                    state.item_registry.get_or_placeholder(&slot.item_id);
                                if item_def.prayer_xp <= 0 {
                                    continue;
                                }
                                if !bone_items.contains(&slot.item_id) {
                                    bone_items.push(slot.item_id.clone());
                                }
                            }
                            if let Some(item_id) = bone_items.get(*idx) {
                                commands.push(InputCommand::OfferAllBones {
                                    item_id: item_id.clone(),
                                    altar_id: altar_npc_id,
                                });
                                audio.play_sfx("item_put");
                                state.ui_state.altar_panel = None;
                            }
                        }
                        UiElementId::AltarPray => {
                            let altar_npc_id = state
                                .ui_state
                                .altar_panel
                                .as_ref()
                                .unwrap()
                                .altar_npc_id
                                .clone();
                            commands.push(InputCommand::PrayAtAltar {
                                altar_id: altar_npc_id,
                            });
                            audio.play_sfx("enter");
                        }
                        UiElementId::AltarClose => {
                            state.ui_state.altar_panel = None;
                            audio.play_sfx("enter");
                        }
                        _ => {
                            // Click outside panel elements - close
                            state.ui_state.altar_panel = None;
                        }
                    }
                } else {
                    // Click with no UI element - close
                    state.ui_state.altar_panel = None;
                }
                return true;
            }
            return true;
        }

        // Handle dialogue mode - intercept input when dialogue is open
        if let Some(dialogue) = &state.ui_state.active_dialogue {
            if is_adventure_board_dialogue(dialogue) {
                if mouse_clicked {
                    if let Some(ref element) = clicked_element {
                        match element {
                            UiElementId::AdventureBoardTabContracts => {
                                state.ui_state.adventure_board_tab = 0;
                                return true;
                            }
                            UiElementId::AdventureBoardTabOrders => {
                                state.ui_state.adventure_board_tab = 1;
                                return true;
                            }
                            UiElementId::CraftingOrder(idx) => {
                                state.ui_state.adventure_board_selected_order = *idx;
                                return true;
                            }
                            UiElementId::CraftingOrderAccept => {
                                if let Some(board) = state.ui_state.adventure_board.as_ref() {
                                    let selected = state.ui_state.adventure_board_selected_order;
                                    if let Some(order) = board.crafting_orders.get(selected) {
                                        if board.crafting_order_active.is_none() {
                                            commands.push(InputCommand::DialogueChoice {
                                                quest_id: dialogue.quest_id.clone(),
                                                choice_id: format!(
                                                    "order_accept:{}",
                                                    order.order_id
                                                ),
                                            });
                                            return true;
                                        }
                                    }
                                }
                            }
                            UiElementId::CraftingOrderClaim => {
                                if state
                                    .ui_state
                                    .adventure_board
                                    .as_ref()
                                    .and_then(|b| b.crafting_order_active.as_ref())
                                    .is_some_and(|o| o.can_claim)
                                {
                                    commands.push(InputCommand::DialogueChoice {
                                        quest_id: dialogue.quest_id.clone(),
                                        choice_id: "order_claim".to_string(),
                                    });
                                    return true;
                                }
                            }
                            UiElementId::CraftingOrderAbandon => {
                                if state
                                    .ui_state
                                    .adventure_board
                                    .as_ref()
                                    .and_then(|b| b.crafting_order_active.as_ref())
                                    .is_some()
                                {
                                    commands.push(InputCommand::DialogueChoice {
                                        quest_id: dialogue.quest_id.clone(),
                                        choice_id: "order_abandon".to_string(),
                                    });
                                    return true;
                                }
                            }
                            UiElementId::AdventureBoardOffer(idx) => {
                                if let Some(board) = state.ui_state.adventure_board.as_ref() {
                                    if *idx < board.offers.len() {
                                        state.ui_state.adventure_board_selected_offer = *idx;
                                    }
                                }
                                return true;
                            }
                            UiElementId::AdventureBoardDifficulty(idx) => {
                                if let Some(board) = state.ui_state.adventure_board.as_ref() {
                                    let offer_idx = state
                                        .ui_state
                                        .adventure_board_selected_offer
                                        .min(board.offers.len().saturating_sub(1));
                                    if let Some(offer) = board.offers.get(offer_idx) {
                                        if let Some(difficulty) = offer.difficulties.get(*idx) {
                                            if difficulty.unlocked
                                                && board.active_contract.is_none()
                                            {
                                                commands.push(InputCommand::DialogueChoice {
                                                    quest_id: dialogue.quest_id.clone(),
                                                    choice_id: format!(
                                                        "board_accept:{}:{}",
                                                        offer.kind_id, difficulty.difficulty_id
                                                    ),
                                                });
                                                return true;
                                            }
                                        }
                                    }
                                }
                            }
                            UiElementId::AdventureBoardClaim => {
                                if state
                                    .ui_state
                                    .adventure_board
                                    .as_ref()
                                    .and_then(|board| board.active_contract.as_ref())
                                    .is_some_and(|contract| contract.can_claim)
                                {
                                    commands.push(InputCommand::DialogueChoice {
                                        quest_id: dialogue.quest_id.clone(),
                                        choice_id: "board_claim".to_string(),
                                    });
                                    return true;
                                }
                            }
                            UiElementId::AdventureBoardAbandon => {
                                if state
                                    .ui_state
                                    .adventure_board
                                    .as_ref()
                                    .and_then(|board| board.active_contract.as_ref())
                                    .is_some()
                                {
                                    commands.push(InputCommand::DialogueChoice {
                                        quest_id: dialogue.quest_id.clone(),
                                        choice_id: "board_abandon".to_string(),
                                    });
                                    return true;
                                }
                            }
                            UiElementId::DialogueClose => {
                                commands.push(InputCommand::CloseDialogue);
                                state.ui_state.active_dialogue = None;
                                state.ui_state.adventure_board = None;
                                state.ui_state.adventure_board_selected_offer = 0;
                                state.ui_state.adventure_board_tab = 0;
                                state.ui_state.adventure_board_selected_order = 0;
                                state.pending_sfx.push("enter".to_string());
                                return true;
                            }
                            _ => {}
                        }
                    }
                }

                if is_key_pressed(KeyCode::Escape) {
                    commands.push(InputCommand::CloseDialogue);
                    state.ui_state.active_dialogue = None;
                    state.ui_state.adventure_board = None;
                    state.ui_state.adventure_board_selected_offer = 0;
                    state.ui_state.adventure_board_tab = 0;
                    state.ui_state.adventure_board_selected_order = 0;
                    return true;
                }

                return true;
            }

            let is_guide_dialogue = is_adventurer_guide_dialogue(&dialogue.speaker);
            let dialogue_has_choices = !dialogue.choices.is_empty();
            let guide_actions_locked = is_guide_dialogue && adventurer_guide_actions_locked(state);
            let guide_selected_active_tier =
                is_guide_dialogue && is_selected_adventurer_guide_tier_active(state);
            let guide_selected_tier_completable =
                is_guide_dialogue && is_selected_adventurer_guide_tier_completable(state);

            // Touch drag scrolling for dialogue choices on mobile
            let all_touches: Vec<Touch> = touches();
            if let Some(tracking_id) = state.ui_state.dialogue_touch_scroll_id {
                if let Some(touch) = all_touches.iter().find(|t| t.id == tracking_id) {
                    match touch.phase {
                        TouchPhase::Moved | TouchPhase::Stationary => {
                            let (_, vy) =
                                screen_to_virtual_coords(touch.position.x, touch.position.y);
                            let dy = state.ui_state.dialogue_touch_last_y - vy;
                            if !state.ui_state.dialogue_touch_dragged {
                                let total_dy = (state.ui_state.dialogue_touch_start_y - vy).abs();
                                if total_dy > 8.0 {
                                    state.ui_state.dialogue_touch_dragged = true;
                                }
                            }
                            if state.ui_state.dialogue_touch_dragged {
                                state.ui_state.dialogue_scroll_offset =
                                    (state.ui_state.dialogue_scroll_offset + dy).max(0.0);
                            }
                            state.ui_state.dialogue_touch_last_y = vy;
                        }
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            state.ui_state.dialogue_touch_scroll_id = None;
                        }
                        _ => {}
                    }
                } else {
                    state.ui_state.dialogue_touch_scroll_id = None;
                }
            } else {
                for touch in &all_touches {
                    if touch.phase == TouchPhase::Started {
                        let (vx, vy) = screen_to_virtual_coords(touch.position.x, touch.position.y);
                        let hit = layout.hit_test(vx, vy);
                        let over_scrollable = matches!(
                            hit,
                            Some(UiElementId::DialogueChoice(_))
                                | Some(UiElementId::DialogueScrollbar)
                        );
                        if over_scrollable {
                            state.ui_state.dialogue_touch_scroll_id = Some(touch.id);
                            state.ui_state.dialogue_touch_last_y = vy;
                            state.ui_state.dialogue_touch_start_y = vy;
                            state.ui_state.dialogue_touch_dragged = false;
                            break;
                        }
                    }
                }
            }

            // Handle mouse scrollbar dragging (generic system)
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::DialogueScrollbar) {
                let choice_spacing: f32 = if cfg!(target_os = "android") {
                    38.0
                } else {
                    32.0
                };
                let total_content = dialogue.choices.len() as f32 * choice_spacing;
                let max_scroll = (total_content - track_bounds.h).max(0.0);
                let clicked_on = matches!(clicked_element, Some(UiElementId::DialogueScrollbar));
                crate::ui::scroll::handle_scrollbar_drag(
                    &mut state.ui_state.dialogue_scroll_drag,
                    &mut state.ui_state.dialogue_scroll_offset,
                    max_scroll,
                    track_bounds,
                    total_content,
                    my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.dialogue_scroll_drag.dragging = false;
            }

            // Handle mouse/touch clicks on dialogue elements
            // Skip if touch was a drag (scroll gesture) or scrollbar interaction
            let was_touch_drag = state.ui_state.dialogue_touch_dragged
                && state.ui_state.dialogue_touch_scroll_id.is_none();
            if was_touch_drag {
                state.ui_state.dialogue_touch_dragged = false;
            }
            let was_scrollbar = state.ui_state.dialogue_scroll_drag.dragging;

            if !was_touch_drag && !was_scrollbar && mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::AdventurerTab(idx) => {
                            state.ui_state.adventurer_selected_tab = *idx;
                            state.ui_state.adventurer_selected_tier = 0;
                            sync_adventurer_guide_dialogue_target(state);
                            return true;
                        }
                        UiElementId::AdventurerTier(idx) => {
                            state.ui_state.adventurer_selected_tier = *idx;
                            sync_adventurer_guide_dialogue_target(state);
                            if should_auto_open_selected_combat_tier_offer(
                                state,
                                is_guide_dialogue,
                                dialogue_has_choices,
                            ) {
                                if let Some(quest_id) = adventurer_guide_tier_id(
                                    state.ui_state.adventurer_selected_tab,
                                    state.ui_state.adventurer_selected_tier,
                                ) {
                                    commands.push(InputCommand::DialogueChoice {
                                        quest_id: quest_id.to_string(),
                                        choice_id: "__continue__".to_string(),
                                    });
                                }
                            }
                            return true;
                        }
                        UiElementId::DialogueChoice(idx) => {
                            if guide_actions_locked || guide_selected_active_tier {
                                return true;
                            }
                            if *idx < dialogue.choices.len() {
                                let choice = &dialogue.choices[*idx];
                                commands.push(InputCommand::DialogueChoice {
                                    quest_id: dialogue.quest_id.clone(),
                                    choice_id: choice.id.clone(),
                                });
                                return true;
                            }
                        }
                        UiElementId::DialogueContinue => {
                            if guide_actions_locked {
                                return true;
                            }
                            commands.push(InputCommand::DialogueChoice {
                                quest_id: dialogue.quest_id.clone(),
                                choice_id: "__continue__".to_string(),
                            });
                            return true;
                        }
                        UiElementId::DialogueClose => {
                            if dialogue.quest_id != "__control_scheme__"
                                && dialogue.quest_id != "__tutorial__"
                            {
                                commands.push(InputCommand::CloseDialogue);
                                state.ui_state.active_dialogue = None;
                                state.ui_state.adventure_board = None;
                                state.ui_state.adventure_board_tab = 0;
                                state.ui_state.adventure_board_selected_order = 0;
                                state.pending_sfx.push("enter".to_string());
                                return true;
                            }
                        }
                        _ => {}
                    }
                }
            }

            if !dialogue.choices.is_empty() {
                // Dialogue with choices - Escape cancels, number keys select
                // Don't allow closing the control scheme choice dialogue with Escape
                if is_key_pressed(KeyCode::Escape)
                    && dialogue.quest_id != "__control_scheme__"
                    && dialogue.quest_id != "__tutorial__"
                {
                    commands.push(InputCommand::CloseDialogue);
                    state.ui_state.active_dialogue = None;
                    state.ui_state.adventure_board = None;
                    state.ui_state.adventure_board_tab = 0;
                    state.ui_state.adventure_board_selected_order = 0;
                    return true;
                }

                // Number keys (1-4) select dialogue choices
                if !guide_actions_locked && !guide_selected_active_tier {
                    let choice_keys = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4];
                    for (i, key) in choice_keys.iter().enumerate() {
                        if i < dialogue.choices.len() && is_key_pressed(*key) {
                            let choice = &dialogue.choices[i];
                            commands.push(InputCommand::DialogueChoice {
                                quest_id: dialogue.quest_id.clone(),
                                choice_id: choice.id.clone(),
                            });
                            // Don't clear dialogue here - wait for server response
                            return true;
                        }
                    }
                }
                // Handle scroll wheel for dialogue choices
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y.abs() > 0.0 {
                    let max_scroll = layout
                        .get_max_scroll(&UiElementId::DialogueScrollbar)
                        .unwrap_or(0.0);
                    state.ui_state.dialogue_scroll_offset = (state.ui_state.dialogue_scroll_offset
                        - wheel_y * 20.0)
                        .clamp(0.0, max_scroll);
                }
            } else {
                // No choices - Escape, Enter, or Space to continue/close
                if is_key_pressed(KeyCode::Escape)
                    && dialogue.quest_id != "__control_scheme__"
                    && is_adventurer_guide_dialogue(&dialogue.speaker)
                {
                    commands.push(InputCommand::CloseDialogue);
                    state.ui_state.active_dialogue = None;
                    state.ui_state.adventure_board = None;
                    state.ui_state.adventure_board_tab = 0;
                    state.ui_state.adventure_board_selected_order = 0;
                    return true;
                }

                // Send __continue__ to server so Lua script can resume execution
                // Don't clear dialogue here - wait for server response (either new dialogue or close)
                if !guide_actions_locked
                    && (is_key_pressed(KeyCode::Enter)
                        || is_key_pressed(KeyCode::Space)
                        || is_key_pressed(KeyCode::Escape))
                {
                    commands.push(InputCommand::DialogueChoice {
                        quest_id: dialogue.quest_id.clone(),
                        choice_id: "__continue__".to_string(),
                    });
                    return true;
                }
            }

            // Don't process other input while dialogue is open
            return true;
        }

        false
    }
}
