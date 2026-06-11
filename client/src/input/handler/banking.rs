use super::*;

impl InputHandler {
    pub(super) fn handle_banking(
        &mut self,
        state: &mut GameState,
        layout: &UiLayout,
        frame: ProcessFrame<'_>,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        let mx = frame.mx;
        let my = frame.my;
        let mouse_clicked = frame.mouse_clicked;
        let mouse_right_clicked = frame.mouse_right_clicked;
        let mouse_released = frame.mouse_released;
        let clicked_element = frame.clicked_element.clone();
        // Handle bank help overlay (blocks other bank input while open)
        if state.ui_state.bank_help_open && state.ui_state.bank_open {
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    if matches!(element, UiElementId::BankHelpClose) {
                        state.ui_state.bank_help_open = false;
                        return true;
                    }
                }
            }
            if is_key_pressed(KeyCode::Escape)
                || is_key_pressed(KeyCode::Enter)
                || is_key_pressed(KeyCode::Space)
            {
                state.ui_state.bank_help_open = false;
                return true;
            }
            return true;
        }

        // Handle bank quantity dialog (blocks other bank input while open)
        if state.ui_state.bank_quantity_dialog.is_some() {
            // Handle button clicks
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::BankQuantityConfirm => {
                            let dialog = state.ui_state.bank_quantity_dialog.as_ref().unwrap();
                            if let Ok(amount) = dialog.input.parse::<i32>() {
                                if amount > 0 && amount <= dialog.max_quantity {
                                    match dialog.action {
                                        BankQuantityAction::DepositItem => {
                                            if let Some(ref item_id) = dialog.item_id {
                                                commands.push(InputCommand::BankDeposit {
                                                    item_id: item_id.clone(),
                                                    quantity: amount,
                                                });
                                            }
                                        }
                                        BankQuantityAction::WithdrawItem => {
                                            if let Some(ref item_id) = dialog.item_id {
                                                commands.push(InputCommand::BankWithdraw {
                                                    item_id: item_id.clone(),
                                                    quantity: amount,
                                                });
                                            }
                                        }
                                        BankQuantityAction::DepositGold => {
                                            commands.push(InputCommand::BankDepositGold { amount });
                                        }
                                        BankQuantityAction::WithdrawGold => {
                                            commands
                                                .push(InputCommand::BankWithdrawGold { amount });
                                        }
                                    }
                                    state.pending_sfx.push("enter".to_string());
                                    state.ui_state.bank_quantity_dialog = None;
                                }
                            }
                            return true;
                        }
                        UiElementId::BankQuantityCancel => {
                            state.ui_state.bank_quantity_dialog = None;
                            return true;
                        }
                        UiElementId::BankQuantityMax => {
                            let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                            let max_str = dialog.max_quantity.to_string();
                            dialog.cursor = max_str.len();
                            dialog.input = max_str;
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            // Keyboard input
            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.bank_quantity_dialog = None;
                return true;
            }

            if is_key_pressed(KeyCode::Enter) {
                let dialog = state.ui_state.bank_quantity_dialog.as_ref().unwrap();
                if let Ok(amount) = dialog.input.parse::<i32>() {
                    if amount > 0 && amount <= dialog.max_quantity {
                        match dialog.action {
                            BankQuantityAction::DepositItem => {
                                if let Some(ref item_id) = dialog.item_id {
                                    commands.push(InputCommand::BankDeposit {
                                        item_id: item_id.clone(),
                                        quantity: amount,
                                    });
                                }
                            }
                            BankQuantityAction::WithdrawItem => {
                                if let Some(ref item_id) = dialog.item_id {
                                    commands.push(InputCommand::BankWithdraw {
                                        item_id: item_id.clone(),
                                        quantity: amount,
                                    });
                                }
                            }
                            BankQuantityAction::DepositGold => {
                                commands.push(InputCommand::BankDepositGold { amount });
                            }
                            BankQuantityAction::WithdrawGold => {
                                commands.push(InputCommand::BankWithdrawGold { amount });
                            }
                        }
                        state.pending_sfx.push("enter".to_string());
                        state.ui_state.bank_quantity_dialog = None;
                    }
                }
                return true;
            }

            // Number key input
            let number_keys = [
                (KeyCode::Key0, '0'),
                (KeyCode::Key1, '1'),
                (KeyCode::Key2, '2'),
                (KeyCode::Key3, '3'),
                (KeyCode::Key4, '4'),
                (KeyCode::Key5, '5'),
                (KeyCode::Key6, '6'),
                (KeyCode::Key7, '7'),
                (KeyCode::Key8, '8'),
                (KeyCode::Key9, '9'),
                (KeyCode::Kp0, '0'),
                (KeyCode::Kp1, '1'),
                (KeyCode::Kp2, '2'),
                (KeyCode::Kp3, '3'),
                (KeyCode::Kp4, '4'),
                (KeyCode::Kp5, '5'),
                (KeyCode::Kp6, '6'),
                (KeyCode::Kp7, '7'),
                (KeyCode::Kp8, '8'),
                (KeyCode::Kp9, '9'),
            ];

            for (key, digit) in &number_keys {
                if is_key_pressed(*key) {
                    let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                    if dialog.input.len() < 10 {
                        dialog.input.insert(dialog.cursor, *digit);
                        dialog.cursor += 1;
                    }
                }
            }

            if is_key_pressed(KeyCode::Backspace) {
                let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                if dialog.cursor > 0 {
                    dialog.input.remove(dialog.cursor - 1);
                    dialog.cursor -= 1;
                }
            }

            if is_key_pressed(KeyCode::Delete) {
                let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                if dialog.cursor < dialog.input.len() {
                    dialog.input.remove(dialog.cursor);
                }
            }

            if is_key_pressed(KeyCode::Left) {
                let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                if dialog.cursor > 0 {
                    dialog.cursor -= 1;
                }
            }
            if is_key_pressed(KeyCode::Right) {
                let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                if dialog.cursor < dialog.input.len() {
                    dialog.cursor += 1;
                }
            }

            if is_key_pressed(KeyCode::Home) {
                let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                dialog.cursor = 0;
            }
            if is_key_pressed(KeyCode::End) {
                let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                dialog.cursor = dialog.input.len();
            }

            // Drain character queue to prevent ghost characters
            while get_char_pressed().is_some() {}

            return true;
        }

        // Handle bank mode
        if state.ui_state.bank_open {
            // Auto-close if player moved too far from banker
            if let Some(local_id) = &state.local_player_id {
                if let Some(player) = state.players.get(local_id) {
                    let px = player.server_x;
                    let py = player.server_y;
                    let near_banker = state.npcs.values().any(|npc| {
                        npc.is_banker && (npc.x - px).abs() <= 3.0 && (npc.y - py).abs() <= 3.0
                    });
                    if !near_banker {
                        state.ui_state.bank_open = false;
                        state.ui_state.bank_slots.clear();
                        state.ui_state.bank_quantity_dialog = None;
                        state.ui_state.bank_help_open = false;
                        state.ui_state.bank_drag = None;
                        return true;
                    }
                }
            }

            // Mouse wheel scrolling
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                const SCROLL_SPEED: f32 = 30.0;

                match &state.ui_state.hovered_element {
                    Some(UiElementId::BankScrollArea) | Some(UiElementId::BankSlot(_)) => {
                        let max_scroll = layout
                            .get_max_scroll(&UiElementId::BankScrollbar)
                            .unwrap_or(0.0);
                        state.ui_state.bank_scroll = (state.ui_state.bank_scroll
                            - wheel_y * SCROLL_SPEED)
                            .clamp(0.0, max_scroll);
                    }
                    Some(UiElementId::BankInvScrollArea)
                    | Some(UiElementId::BankInventorySlot(_)) => {
                        let max_scroll = layout
                            .get_max_scroll(&UiElementId::BankInvScrollbar)
                            .unwrap_or(0.0);
                        state.ui_state.bank_inv_scroll = (state.ui_state.bank_inv_scroll
                            - wheel_y * SCROLL_SPEED)
                            .clamp(0.0, max_scroll);
                    }
                    _ => {}
                }
            }

            // Bank scrollbar drag handling
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::BankScrollbar) {
                let bank_max = layout
                    .get_max_scroll(&UiElementId::BankScrollbar)
                    .unwrap_or(0.0);
                let bank_content_h = bank_max + track_bounds.h;
                let clicked_on = matches!(clicked_element, Some(UiElementId::BankScrollbar));
                crate::ui::scroll::handle_scrollbar_drag(
                    &mut state.ui_state.bank_scroll_drag,
                    &mut state.ui_state.bank_scroll,
                    bank_max,
                    track_bounds,
                    bank_content_h,
                    my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.bank_scroll_drag.dragging = false;
            }

            // Bank inventory scrollbar drag handling
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::BankInvScrollbar) {
                let inv_max = layout
                    .get_max_scroll(&UiElementId::BankInvScrollbar)
                    .unwrap_or(0.0);
                let inv_content_h = inv_max + track_bounds.h;
                let clicked_on = matches!(clicked_element, Some(UiElementId::BankInvScrollbar));
                crate::ui::scroll::handle_scrollbar_drag(
                    &mut state.ui_state.bank_inv_scroll_drag,
                    &mut state.ui_state.bank_inv_scroll,
                    inv_max,
                    track_bounds,
                    inv_content_h,
                    my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.bank_inv_scroll_drag.dragging = false;
            }

            // === Bank drag state machine ===
            let (cur_mx, cur_my) = mouse_position();
            if state.ui_state.bank_drag.is_some() {
                let drag = state.ui_state.bank_drag.as_ref().unwrap();
                let from_slot = drag.from_slot;
                let active = drag.active;

                // Cancel on right-click or Escape
                if is_mouse_button_pressed(MouseButton::Right) || is_key_pressed(KeyCode::Escape) {
                    state.ui_state.bank_drag = None;
                    return true;
                }

                if active {
                    // Active drag: check for drop on mouse release
                    if mouse_released {
                        if let Some(UiElementId::BankSlot(target_idx)) =
                            &state.ui_state.hovered_element
                        {
                            let target = *target_idx;
                            if target != from_slot {
                                commands.push(InputCommand::BankSwapSlots {
                                    slot_a: from_slot as u32,
                                    slot_b: target as u32,
                                });
                                state.pending_sfx.push("enter".to_string());
                            }
                        }
                        state.ui_state.bank_drag = None;
                        return true;
                    }
                    // Active drag consumes input - don't process clicks below
                    // (fall through to click handling is blocked by the else-if below)
                } else {
                    // Pending drag: check dead zone or release
                    if mouse_released {
                        // Mouse released within dead zone => treat as normal click
                        state.ui_state.bank_drag = None;
                        // Process as withdraw click
                        if let Some(Some((item_id, qty))) = state.ui_state.bank_slots.get(from_slot)
                        {
                            let ctrl_held = is_key_down(KeyCode::LeftControl)
                                || is_key_down(KeyCode::RightControl);
                            let shift_held =
                                is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                            if shift_held {
                                commands.push(InputCommand::BankWithdraw {
                                    item_id: item_id.clone(),
                                    quantity: *qty,
                                });
                                state.pending_sfx.push("enter".to_string());
                            } else if ctrl_held && *qty > 1 {
                                state.ui_state.bank_quantity_dialog = Some(BankQuantityDialog {
                                    input: String::new(),
                                    cursor: 0,
                                    action: BankQuantityAction::WithdrawItem,
                                    item_id: Some(item_id.clone()),
                                    max_quantity: *qty,
                                });
                            } else {
                                commands.push(InputCommand::BankWithdraw {
                                    item_id: item_id.clone(),
                                    quantity: 1,
                                });
                                state.pending_sfx.push("enter".to_string());
                            }
                        }
                        return true;
                    }

                    // Check dead zone (4px = squared distance > 16.0)
                    let dx = cur_mx - drag.mouse_start_x;
                    let dy = cur_my - drag.mouse_start_y;
                    if dx * dx + dy * dy > 16.0 {
                        // Promote to active drag
                        state.ui_state.bank_drag.as_mut().unwrap().active = true;
                    }
                }

                // If we have an active drag, consume input and skip click handling
                if state
                    .ui_state
                    .bank_drag
                    .as_ref()
                    .map(|d| d.active)
                    .unwrap_or(false)
                {
                    return true;
                }
            }

            // Right-click context menu for bank slots and inventory slots
            if mouse_right_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::BankSlot(idx) => {
                            if let Some(Some(_)) = state.ui_state.bank_slots.get(*idx) {
                                state.ui_state.context_menu = Some(ContextMenu {
                                    target: ContextMenuTarget::BankSlot(*idx),
                                    x: mx,
                                    y: my,
                                });
                            }
                            return true;
                        }
                        UiElementId::BankInventorySlot(idx) => {
                            if let Some(Some(_)) = state.inventory.slots.get(*idx) {
                                state.ui_state.context_menu = Some(ContextMenu {
                                    target: ContextMenuTarget::BankInventorySlot(*idx),
                                    x: mx,
                                    y: my,
                                });
                            }
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            // Initiate bank drag on mouse_clicked over a BankSlot with an item
            if mouse_clicked {
                if let Some(UiElementId::BankSlot(idx)) = &clicked_element {
                    let idx = *idx;
                    if let Some(Some(_)) = state.ui_state.bank_slots.get(idx) {
                        // Start a pending drag
                        state.ui_state.bank_drag = Some(BankDrag {
                            from_slot: idx,
                            mouse_start_x: cur_mx,
                            mouse_start_y: cur_my,
                            offset_x: 0.0,
                            offset_y: 0.0,
                            active: false,
                        });
                        // Don't fall through to the normal BankSlot click handler
                        return true;
                    }
                }
            }

            // Click handling
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::BankHelpButton => {
                            state.ui_state.bank_help_open = true;
                            return true;
                        }
                        UiElementId::BankCloseButton => {
                            state.ui_state.bank_open = false;
                            state.ui_state.bank_slots.clear();
                            state.ui_state.bank_quantity_dialog = None;
                            state.ui_state.bank_help_open = false;
                            state.ui_state.bank_drag = None;
                            state.pending_sfx.push("enter".to_string());
                            return true;
                        }
                        UiElementId::BankInventorySlot(slot_idx) => {
                            // Deposit item from inventory to bank
                            if let Some(Some(inv_slot)) = state.inventory.slots.get(*slot_idx) {
                                let ctrl_held = is_key_down(KeyCode::LeftControl)
                                    || is_key_down(KeyCode::RightControl);
                                let shift_held = is_key_down(KeyCode::LeftShift)
                                    || is_key_down(KeyCode::RightShift);
                                if shift_held {
                                    // Shift+Click = deposit all
                                    commands.push(InputCommand::BankDeposit {
                                        item_id: inv_slot.item_id.clone(),
                                        quantity: inv_slot.quantity,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                } else if ctrl_held && inv_slot.quantity > 1 {
                                    // Ctrl+Click = open quantity dialog (only if stack > 1)
                                    state.ui_state.bank_quantity_dialog =
                                        Some(BankQuantityDialog {
                                            input: String::new(),
                                            cursor: 0,
                                            action: BankQuantityAction::DepositItem,
                                            item_id: Some(inv_slot.item_id.clone()),
                                            max_quantity: inv_slot.quantity,
                                        });
                                } else {
                                    // Click = deposit 1
                                    commands.push(InputCommand::BankDeposit {
                                        item_id: inv_slot.item_id.clone(),
                                        quantity: 1,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                }
                            }
                            return true;
                        }
                        UiElementId::BankSlot(idx) => {
                            // Withdraw item from bank to inventory
                            if let Some(Some((item_id, qty))) = state.ui_state.bank_slots.get(*idx)
                            {
                                let ctrl_held = is_key_down(KeyCode::LeftControl)
                                    || is_key_down(KeyCode::RightControl);
                                let shift_held = is_key_down(KeyCode::LeftShift)
                                    || is_key_down(KeyCode::RightShift);
                                if shift_held {
                                    // Shift+Click = withdraw all
                                    commands.push(InputCommand::BankWithdraw {
                                        item_id: item_id.clone(),
                                        quantity: *qty,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                } else if ctrl_held && *qty > 1 {
                                    // Ctrl+Click = open quantity dialog (only if stack > 1)
                                    state.ui_state.bank_quantity_dialog =
                                        Some(BankQuantityDialog {
                                            input: String::new(),
                                            cursor: 0,
                                            action: BankQuantityAction::WithdrawItem,
                                            item_id: Some(item_id.clone()),
                                            max_quantity: *qty,
                                        });
                                } else {
                                    // Click = withdraw 1
                                    commands.push(InputCommand::BankWithdraw {
                                        item_id: item_id.clone(),
                                        quantity: 1,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                }
                            }
                            return true;
                        }
                        UiElementId::BankDepositGoldButton => {
                            if state.inventory.gold > 0 {
                                let ctrl_held = is_key_down(KeyCode::LeftControl)
                                    || is_key_down(KeyCode::RightControl);
                                let shift_held = is_key_down(KeyCode::LeftShift)
                                    || is_key_down(KeyCode::RightShift);
                                if shift_held {
                                    commands.push(InputCommand::BankDepositGold {
                                        amount: state.inventory.gold,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                } else if ctrl_held {
                                    state.ui_state.bank_quantity_dialog =
                                        Some(BankQuantityDialog {
                                            input: String::new(),
                                            cursor: 0,
                                            action: BankQuantityAction::DepositGold,
                                            item_id: None,
                                            max_quantity: state.inventory.gold,
                                        });
                                } else {
                                    commands.push(InputCommand::BankDepositGold { amount: 1 });
                                    state.pending_sfx.push("enter".to_string());
                                }
                            }
                            return true;
                        }
                        UiElementId::BankWithdrawGoldButton => {
                            if state.ui_state.bank_gold > 0 {
                                let ctrl_held = is_key_down(KeyCode::LeftControl)
                                    || is_key_down(KeyCode::RightControl);
                                let shift_held = is_key_down(KeyCode::LeftShift)
                                    || is_key_down(KeyCode::RightShift);
                                if shift_held {
                                    commands.push(InputCommand::BankWithdrawGold {
                                        amount: state.ui_state.bank_gold,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                } else if ctrl_held {
                                    state.ui_state.bank_quantity_dialog =
                                        Some(BankQuantityDialog {
                                            input: String::new(),
                                            cursor: 0,
                                            action: BankQuantityAction::WithdrawGold,
                                            item_id: None,
                                            max_quantity: state.ui_state.bank_gold,
                                        });
                                } else {
                                    commands.push(InputCommand::BankWithdrawGold { amount: 1 });
                                    state.pending_sfx.push("enter".to_string());
                                }
                            }
                            return true;
                        }
                        UiElementId::BankDepositAllButton => {
                            commands.push(InputCommand::BankDepositAll);
                            state.pending_sfx.push("enter".to_string());
                            return true;
                        }
                        UiElementId::BankSortButton => {
                            commands.push(InputCommand::BankSort);
                            state.pending_sfx.push("enter".to_string());
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            // Escape to close
            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.bank_open = false;
                state.ui_state.bank_slots.clear();
                state.ui_state.bank_quantity_dialog = None;
                state.ui_state.bank_help_open = false;
                state.ui_state.bank_drag = None;
                return true;
            }

            return true;
        }

        false
    }
}
