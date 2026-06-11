use super::*;

impl InputHandler {
    pub(super) fn handle_modal_panels(
        state: &mut GameState,
        layout: &UiLayout,
        clicked_element: Option<&UiElementId>,
        mouse_clicked: bool,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        if state.ui_state.gold_drop_dialog.is_some() {
            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::GoldDropConfirm => {
                            let dialog = state.ui_state.gold_drop_dialog.as_ref().unwrap();
                            if let Ok(amount) = dialog.input.parse::<i32>() {
                                if amount > 0 && amount <= state.inventory.gold {
                                    if state.ui_state.trade_open {
                                        commands.push(InputCommand::TradeOfferGold { amount });
                                    } else {
                                        commands.push(InputCommand::DropGold { amount });
                                    }
                                    state.ui_state.gold_drop_dialog = None;
                                }
                            }
                            return true;
                        }
                        UiElementId::GoldDropCancel => {
                            state.ui_state.gold_drop_dialog = None;
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.gold_drop_dialog = None;
                return true;
            }

            if is_key_pressed(KeyCode::Enter) {
                let dialog = state.ui_state.gold_drop_dialog.as_ref().unwrap();
                if let Ok(amount) = dialog.input.parse::<i32>() {
                    if amount > 0 && amount <= state.inventory.gold {
                        if state.ui_state.trade_open {
                            commands.push(InputCommand::TradeOfferGold { amount });
                        } else {
                            commands.push(InputCommand::DropGold { amount });
                        }
                        state.ui_state.gold_drop_dialog = None;
                    }
                }
                return true;
            }

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
                    let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                    if dialog.input.len() < 10 {
                        dialog.input.insert(dialog.cursor, *digit);
                        dialog.cursor += 1;
                    }
                }
            }

            if is_key_pressed(KeyCode::Backspace) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                if dialog.cursor > 0 {
                    dialog.input.remove(dialog.cursor - 1);
                    dialog.cursor -= 1;
                }
            }

            if is_key_pressed(KeyCode::Delete) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                if dialog.cursor < dialog.input.len() {
                    dialog.input.remove(dialog.cursor);
                }
            }

            if is_key_pressed(KeyCode::Left) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                if dialog.cursor > 0 {
                    dialog.cursor -= 1;
                }
            }
            if is_key_pressed(KeyCode::Right) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                if dialog.cursor < dialog.input.len() {
                    dialog.cursor += 1;
                }
            }

            if is_key_pressed(KeyCode::Home) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                dialog.cursor = 0;
            }
            if is_key_pressed(KeyCode::End) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                dialog.cursor = dialog.input.len();
            }

            while get_char_pressed().is_some() {}
            return true;
        }

        if state.ui_state.stall_price_dialog.is_some() {
            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::StallPriceConfirm => {
                            let dialog = state.ui_state.stall_price_dialog.as_ref().unwrap();
                            if let Ok(price) = dialog.input.parse::<i32>() {
                                if price > 0 {
                                    let item_id = dialog.item_id.clone();
                                    commands.push(InputCommand::StallSetItem {
                                        inventory_slot: dialog.inventory_slot,
                                        quantity: dialog.quantity,
                                        price,
                                    });
                                    state.ui_state.stall_last_prices.insert(item_id, price);
                                    state.ui_state.stall_price_dialog = None;
                                }
                            }
                            return true;
                        }
                        UiElementId::StallPriceCancel => {
                            state.ui_state.stall_price_dialog = None;
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.stall_price_dialog = None;
                return true;
            }

            if is_key_pressed(KeyCode::Enter) {
                let dialog = state.ui_state.stall_price_dialog.as_ref().unwrap();
                if let Ok(price) = dialog.input.parse::<i32>() {
                    if price > 0 {
                        let item_id = dialog.item_id.clone();
                        commands.push(InputCommand::StallSetItem {
                            inventory_slot: dialog.inventory_slot,
                            quantity: dialog.quantity,
                            price,
                        });
                        state.ui_state.stall_last_prices.insert(item_id, price);
                        state.ui_state.stall_price_dialog = None;
                    }
                }
                return true;
            }

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
            ];
            for (key, digit) in &number_keys {
                if is_key_pressed(*key) {
                    let dialog = state.ui_state.stall_price_dialog.as_mut().unwrap();
                    if dialog.input.len() < 10 {
                        dialog.input.insert(dialog.cursor, *digit);
                        dialog.cursor += 1;
                    }
                }
            }

            if is_key_pressed(KeyCode::Backspace) {
                let dialog = state.ui_state.stall_price_dialog.as_mut().unwrap();
                if dialog.cursor > 0 {
                    dialog.input.remove(dialog.cursor - 1);
                    dialog.cursor -= 1;
                }
            }

            if is_key_pressed(KeyCode::Delete) {
                let dialog = state.ui_state.stall_price_dialog.as_mut().unwrap();
                if dialog.cursor < dialog.input.len() {
                    dialog.input.remove(dialog.cursor);
                }
            }

            if is_key_pressed(KeyCode::Left) {
                let dialog = state.ui_state.stall_price_dialog.as_mut().unwrap();
                if dialog.cursor > 0 {
                    dialog.cursor -= 1;
                }
            }
            if is_key_pressed(KeyCode::Right) {
                let dialog = state.ui_state.stall_price_dialog.as_mut().unwrap();
                if dialog.cursor < dialog.input.len() {
                    dialog.cursor += 1;
                }
            }

            if is_key_pressed(KeyCode::Home) {
                let dialog = state.ui_state.stall_price_dialog.as_mut().unwrap();
                dialog.cursor = 0;
            }
            if is_key_pressed(KeyCode::End) {
                let dialog = state.ui_state.stall_price_dialog.as_mut().unwrap();
                dialog.cursor = dialog.input.len();
            }

            while get_char_pressed().is_some() {}
            return true;
        }

        if state.ui_state.chest_open {
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                if let Some(UiElementId::ChestScrollArea) = &state.ui_state.hovered_element {
                    let max_scroll = layout
                        .get_max_scroll(&UiElementId::ChestScrollArea)
                        .unwrap_or(0.0);
                    state.ui_state.chest_scroll =
                        (state.ui_state.chest_scroll - wheel_y * 30.0).clamp(0.0, max_scroll);
                }
            }

            let mut chest_handled = false;
            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::ChestClose => {
                            state.ui_state.chest_open = false;
                            state.pending_sfx.push("enter".to_string());
                            chest_handled = true;
                        }
                        UiElementId::ChestSlot(idx) => {
                            if (*idx as usize) < state.ui_state.chest_slots.len() {
                                if state.ui_state.chest_slots[*idx as usize].is_some() {
                                    commands.push(InputCommand::ChestTake {
                                        chest_id: state.ui_state.chest_id.clone(),
                                        slot: *idx,
                                    });
                                }
                            }
                            chest_handled = true;
                        }
                        _ => {}
                    }
                }
            }

            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.chest_open = false;
                return true;
            }

            if chest_handled {
                return true;
            }
        }

        if state.ui_state.slayer_panel_open {
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                if let Some(UiElementId::SlayerScrollArea) = &state.ui_state.hovered_element {
                    state.ui_state.slayer_reward_scroll =
                        (state.ui_state.slayer_reward_scroll - wheel_y * 30.0).max(0.0);
                } else if matches!(
                    &state.ui_state.hovered_element,
                    Some(UiElementId::SlayerBlockScrollArea)
                        | Some(UiElementId::SlayerBlockMonsterSelect(_))
                        | Some(UiElementId::SlayerRemoveBlock(_))
                        | Some(UiElementId::SlayerBlockScrollbar)
                ) {
                    state.ui_state.slayer_block_scroll_offset =
                        (state.ui_state.slayer_block_scroll_offset - wheel_y * 30.0).max(0.0);
                }
            }

            // Block list scrollbar drag handling
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::SlayerBlockScrollbar) {
                let s = state.ui_state.ui_scale;
                let compact_h = 24.0 * s;
                let compact_sp = 2.0 * s;

                let available_count = state
                    .ui_state
                    .slayer_blockable_monsters
                    .iter()
                    .filter(|(id, _)| !state.ui_state.slayer_blocked_monsters.contains(id))
                    .count();

                let mut total_h = 0.0_f32;
                if available_count > 0 {
                    total_h += compact_h + compact_sp;
                    total_h += available_count as f32 * (compact_h + compact_sp);
                    total_h += 4.0 * s;
                }
                total_h += compact_h + compact_sp;
                if state.ui_state.slayer_blocked_monsters.is_empty() {
                    total_h += compact_h + compact_sp;
                } else {
                    total_h += state.ui_state.slayer_blocked_monsters.len() as f32
                        * (compact_h + compact_sp);
                }

                let max_scroll = (total_h - track_bounds.h).max(0.0);
                let clicked_on = matches!(clicked_element, Some(UiElementId::SlayerBlockScrollbar));
                let (_, raw_my) = mouse_position();
                let (_, virt_my) = screen_to_virtual_coords(0.0, raw_my);
                crate::ui::scroll::handle_scrollbar_drag(
                    &mut state.ui_state.slayer_block_scroll_drag,
                    &mut state.ui_state.slayer_block_scroll_offset,
                    max_scroll,
                    track_bounds,
                    total_h,
                    virt_my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.slayer_block_scroll_drag.dragging = false;
            }

            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::SlayerCloseButton => {
                            state.ui_state.slayer_panel_open = false;
                            state.pending_sfx.push("enter".to_string());
                        }
                        UiElementId::SlayerGetTaskButton => {
                            if let Some(ref master_id) = state.ui_state.slayer_master_id.clone() {
                                commands.push(InputCommand::SlayerGetTask {
                                    master_id: master_id.clone(),
                                });
                            }
                        }
                        UiElementId::SlayerCancelTaskButton => {
                            commands.push(InputCommand::SlayerCancelTask);
                        }
                        UiElementId::SlayerRewardTab(idx) => {
                            state.ui_state.slayer_reward_tab = *idx;
                            state.ui_state.slayer_reward_scroll = 0.0;
                            state.ui_state.slayer_block_scroll_offset = 0.0;
                        }
                        UiElementId::SlayerBuyReward(idx) => {
                            if let Some(reward) = state.ui_state.slayer_rewards.get(*idx) {
                                if state.ui_state.slayer_points >= reward.cost {
                                    let target = if reward.category == "block" {
                                        state.ui_state.slayer_selected_block_monster.and_then(|i| {
                                            state
                                                .ui_state
                                                .slayer_blockable_monsters
                                                .get(i)
                                                .map(|(id, _)| id.clone())
                                        })
                                    } else {
                                        reward.target_id.clone()
                                    };
                                    commands.push(InputCommand::SlayerBuyReward {
                                        reward_id: reward.id.clone(),
                                        target_monster_id: target,
                                    });
                                }
                            }
                        }
                        UiElementId::SlayerRemoveBlock(idx) => {
                            if let Some(monster_name) =
                                state.ui_state.slayer_blocked_monsters.get(*idx)
                            {
                                commands.push(InputCommand::SlayerRemoveBlock {
                                    monster_id: monster_name.clone(),
                                });
                            }
                        }
                        UiElementId::SlayerBlockMonsterSelect(idx) => {
                            state.ui_state.slayer_selected_block_monster = Some(*idx);
                        }
                        _ => {}
                    }
                }
            }

            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.slayer_panel_open = false;
                return true;
            }

            return true;
        }

        if state.ui_state.trade_open {
            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::TradeOfferSlot(i) => {
                            commands.push(InputCommand::TradeRemoveItem {
                                offer_index: *i as u8,
                            });
                        }
                        UiElementId::TradeGoldInput => {
                            state.ui_state.gold_drop_dialog = Some(GoldDropDialog {
                                input: String::new(),
                                cursor: 0,
                            });
                        }
                        UiElementId::TradeAcceptButton => {
                            commands.push(InputCommand::TradeAccept);
                        }
                        UiElementId::TradeCancelButton => {
                            commands.push(InputCommand::TradeCancel);
                        }
                        UiElementId::InventorySlot(slot_idx) => {
                            if let Some(slot) = state
                                .inventory
                                .slots
                                .get(*slot_idx)
                                .and_then(|s| s.as_ref())
                            {
                                commands.push(InputCommand::TradeOfferItem {
                                    slot_index: *slot_idx as u8,
                                    quantity: slot.quantity,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }

            if is_key_pressed(KeyCode::Escape) {
                commands.push(InputCommand::TradeCancel);
                return true;
            }

            return true;
        }

        if state.ui_state.trade_pending_request.is_some() && mouse_clicked {
            if let Some(element) = clicked_element {
                match element {
                    UiElementId::TradeRequestAccept => {
                        if let Some((ref requester_id, _)) = state.ui_state.trade_pending_request {
                            commands.push(InputCommand::TradeAcceptRequest {
                                requester_id: requester_id.clone(),
                            });
                        }
                        state.ui_state.trade_pending_request = None;
                    }
                    UiElementId::TradeRequestDecline => {
                        if let Some((ref requester_id, _)) = state.ui_state.trade_pending_request {
                            commands.push(InputCommand::TradeDeclineRequest {
                                requester_id: requester_id.clone(),
                            });
                        }
                        state.ui_state.trade_pending_request = None;
                    }
                    _ => {}
                }
            }
        }

        if state.ui_state.stall_setup_open {
            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::StallSetupNameInput => {
                            state.ui_state.stall_name_editing = true;
                            state.ui_state.stall_name_cursor = state.ui_state.stall_my_name.len();
                        }
                        UiElementId::StallSetupRemove(i) => {
                            state.ui_state.stall_name_editing = false;
                            if let Some(slot) = state.ui_state.stall_my_slots.get(*i) {
                                commands.push(InputCommand::StallRemoveItem {
                                    stall_slot: slot.slot,
                                });
                            }
                        }
                        UiElementId::StallSetupOpenButton => {
                            state.ui_state.stall_name_editing = false;
                            if state.ui_state.stall_active {
                                commands.push(InputCommand::StallClose);
                            } else {
                                let name = if state.ui_state.stall_my_name.is_empty() {
                                    "My Shop".to_string()
                                } else {
                                    state.ui_state.stall_my_name.clone()
                                };
                                commands.push(InputCommand::StallOpen { name });
                            }
                        }
                        UiElementId::StallSetupCloseButton => {
                            state.ui_state.stall_name_editing = false;
                            state.ui_state.stall_setup_open = false;
                        }
                        UiElementId::InventorySlot(slot_idx) => {
                            state.ui_state.stall_name_editing = false;
                            if let Some(slot) = state
                                .inventory
                                .slots
                                .get(*slot_idx)
                                .and_then(|s| s.as_ref())
                            {
                                commands.push(InputCommand::StallSetItem {
                                    inventory_slot: *slot_idx as u8,
                                    quantity: slot.quantity,
                                    price: 1,
                                });
                            }
                        }
                        _ => {
                            state.ui_state.stall_name_editing = false;
                        }
                    }
                } else {
                    state.ui_state.stall_name_editing = false;
                }
            }

            if state.ui_state.stall_name_editing {
                if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Enter) {
                    state.ui_state.stall_name_editing = false;
                    while get_char_pressed().is_some() {}
                    return true;
                }

                if is_key_pressed(KeyCode::Backspace) {
                    if state.ui_state.stall_name_cursor > 0 {
                        state.ui_state.stall_name_cursor -= 1;
                        state
                            .ui_state
                            .stall_my_name
                            .remove(state.ui_state.stall_name_cursor);
                    }
                }
                if is_key_pressed(KeyCode::Delete) {
                    if state.ui_state.stall_name_cursor < state.ui_state.stall_my_name.len() {
                        state
                            .ui_state
                            .stall_my_name
                            .remove(state.ui_state.stall_name_cursor);
                    }
                }
                if is_key_pressed(KeyCode::Left) && state.ui_state.stall_name_cursor > 0 {
                    state.ui_state.stall_name_cursor -= 1;
                }
                if is_key_pressed(KeyCode::Right)
                    && state.ui_state.stall_name_cursor < state.ui_state.stall_my_name.len()
                {
                    state.ui_state.stall_name_cursor += 1;
                }
                if is_key_pressed(KeyCode::Home) {
                    state.ui_state.stall_name_cursor = 0;
                }
                if is_key_pressed(KeyCode::End) {
                    state.ui_state.stall_name_cursor = state.ui_state.stall_my_name.len();
                }

                while let Some(ch) = get_char_pressed() {
                    if ch.is_control() {
                        continue;
                    }
                    if state.ui_state.stall_my_name.len() < 24 {
                        state
                            .ui_state
                            .stall_my_name
                            .insert(state.ui_state.stall_name_cursor, ch);
                        state.ui_state.stall_name_cursor += 1;
                    }
                }

                return true;
            }

            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.stall_setup_open = false;
                state.ui_state.stall_name_editing = false;
                return true;
            }

            return true;
        }

        if state.ui_state.stall_browse.is_some() {
            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::StallBrowseItem(i) => {
                            state.ui_state.stall_browse_selected = *i;
                            state.ui_state.stall_buy_quantity = 1;
                        }
                        UiElementId::StallBrowseQuantityMinus => {
                            if state.ui_state.stall_buy_quantity > 1 {
                                state.ui_state.stall_buy_quantity -= 1;
                            }
                        }
                        UiElementId::StallBrowseQuantityPlus => {
                            state.ui_state.stall_buy_quantity += 1;
                            if let Some(ref browse) = state.ui_state.stall_browse {
                                if let Some(item) =
                                    browse.items.get(state.ui_state.stall_browse_selected)
                                {
                                    if state.ui_state.stall_buy_quantity > item.quantity {
                                        state.ui_state.stall_buy_quantity = item.quantity;
                                    }
                                }
                            }
                        }
                        UiElementId::StallBrowseBuyButton => {
                            if let Some(ref browse) = state.ui_state.stall_browse {
                                if let Some(item) =
                                    browse.items.get(state.ui_state.stall_browse_selected)
                                {
                                    commands.push(InputCommand::StallBuy {
                                        seller_id: browse.seller_id.clone(),
                                        stall_slot: item.slot,
                                        quantity: state.ui_state.stall_buy_quantity,
                                        expected_price: item.price,
                                    });
                                }
                            }
                        }
                        UiElementId::StallBrowseCloseButton => {
                            state.ui_state.stall_browse = None;
                            state.pending_sfx.push("ui_close".to_string());
                        }
                        _ => {}
                    }
                }
            }

            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.stall_browse = None;
                return true;
            }

            return true;
        }

        // KOTH game over dialog
        if state.koth_game_over.is_some() {
            if mouse_clicked {
                if let Some(element) = clicked_element {
                    if matches!(element, UiElementId::KothGameOverDismiss) {
                        state.koth_game_over = None;
                    }
                }
            }
            return true;
        }

        // KOTH checkpoint dialog
        if state.koth_checkpoint_open {
            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::KothContinueButton => {
                            state.koth_checkpoint_open = false;
                            state.koth_checkpoint_info = None;
                            commands.push(InputCommand::KothContinue);
                        }
                        UiElementId::KothLeaveButton => {
                            state.koth_checkpoint_open = false;
                            state.koth_checkpoint_info = None;
                            commands.push(InputCommand::KothLeave);
                        }
                        _ => {}
                    }
                }
            }
            return true;
        }

        false
    }
}
