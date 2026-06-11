use super::*;

impl InputHandler {
    pub(super) fn handle_drag_start(
        &mut self,
        state: &mut GameState,
        audio: &mut AudioManager,
        frame: ProcessFrame<'_>,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        let current_time = frame.current_time;
        let mouse_clicked = frame.mouse_clicked;
        let clicked_element = frame.clicked_element.clone();
        // Double-click detection threshold (300ms)
        const DOUBLE_CLICK_THRESHOLD: f64 = 0.3;

        // Start drag on left click on inventory slot with item
        // But first check for double-click to equip
        if mouse_clicked && state.ui_state.drag_state.is_none() {
            if let Some(ref element) = clicked_element {
                match element {
                    UiElementId::InventorySlot(idx) => {
                        // Check if slot has an item
                        if let Some(Some(slot)) = state.inventory.slots.get(*idx) {
                            // If trade is open, add item to trade offer instead of dragging
                            if state.ui_state.trade_open {
                                commands.push(InputCommand::TradeOfferItem {
                                    slot_index: *idx as u8,
                                    quantity: slot.quantity,
                                });
                                return true;
                            }

                            // If stall setup is open, open price dialog before adding
                            if state.ui_state.stall_setup_open {
                                let item_id = slot.item_id.clone();
                                let last_price = state
                                    .ui_state
                                    .stall_last_prices
                                    .get(&item_id)
                                    .copied()
                                    .unwrap_or(0);
                                let prefill = if last_price > 0 {
                                    last_price.to_string()
                                } else {
                                    String::new()
                                };
                                let cursor = prefill.len();
                                state.ui_state.stall_price_dialog = Some(StallPriceDialog {
                                    input: prefill,
                                    cursor,
                                    inventory_slot: *idx as u8,
                                    quantity: slot.quantity,
                                    item_id,
                                });
                                return true;
                            }

                            // Check for shift+click to drop (if enabled)
                            let shift_held =
                                is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                            if shift_held && state.ui_state.shift_drop_enabled {
                                // Drop the entire stack at player position
                                commands.push(InputCommand::DropItem {
                                    slot_index: *idx as u8,
                                    quantity: slot.quantity as u32,
                                    target_x: None,
                                    target_y: None,
                                });
                                audio.play_sfx("item_put");
                                return true;
                            }

                            // Check for double-click
                            let is_double_click = state.ui_state.double_click_state.last_click_slot
                                == Some(*idx)
                                && current_time - state.ui_state.double_click_state.last_click_time
                                    < DOUBLE_CLICK_THRESHOLD;

                            if is_double_click {
                                // Reset double-click state
                                state.ui_state.double_click_state.last_click_slot = None;
                                state.ui_state.double_click_state.last_click_time = 0.0;

                                // Check if item is equippable
                                let item_def =
                                    state.item_registry.get_or_placeholder(&slot.item_id);
                                if item_def.equipment.is_some() {
                                    // Equip the item
                                    commands.push(InputCommand::Equip {
                                        slot_index: *idx as u8,
                                    });
                                    return true;
                                } else {
                                    // Not equippable - use the item instead (e.g., health potion)
                                    commands.push(InputCommand::UseItem {
                                        slot_index: *idx as u8,
                                    });
                                    return true;
                                }
                            } else {
                                // First click - record for potential double-click
                                state.ui_state.double_click_state.last_click_slot = Some(*idx);
                                state.ui_state.double_click_state.last_click_time = current_time;

                                // Single click on inventory slot: clear any selection
                                // (item selection for use-on-entity is disabled for now)
                                state.ui_state.selected_inventory_slot = None;

                                // Start drag from inventory
                                state.ui_state.drag_state = Some(DragState {
                                    source: DragSource::Inventory(*idx),
                                    item_id: slot.item_id.clone(),
                                    quantity: slot.quantity,
                                });
                                audio.play_sfx("item_grab");
                                // Don't process other input while starting drag
                                return true;
                            }
                        }
                    }
                    UiElementId::QuickSlot(idx) => {
                        // Unified hotkey bar: activate on click
                        let cmds = activate_hotkey_slot(state, *idx);
                        commands.extend(cmds);
                        return true;
                    }
                    UiElementId::SpellSlot(slot_idx) => {
                        // Start drag from spell panel
                        if *slot_idx < crate::game::spell::SPELLS.len() {
                            let spell = &crate::game::spell::SPELLS[*slot_idx];
                            state.ui_state.drag_state = Some(DragState {
                                source: DragSource::Spell(spell.id.to_string()),
                                item_id: spell.id.to_string(),
                                quantity: 0,
                            });
                            audio.play_sfx("item_grab");
                            return true;
                        } else {
                            // Scroll spell slot - only allow drag if unlocked
                            let scroll_idx = *slot_idx - crate::game::spell::SPELLS.len();
                            if let Some(scroll_spell) =
                                state.scroll_spell_definitions.get(scroll_idx)
                            {
                                if state.unlocked_spells.contains(&scroll_spell.id) {
                                    let id = scroll_spell.id.clone();
                                    state.ui_state.drag_state = Some(DragState {
                                        source: DragSource::Spell(id.clone()),
                                        item_id: id,
                                        quantity: 0,
                                    });
                                    audio.play_sfx("item_grab");
                                    return true;
                                }
                            }
                        }
                    }
                    UiElementId::EquipmentSlot(slot_type) => {
                        // Check if equipment slot has an item
                        let equipped_item = match slot_type.as_str() {
                            "head" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_head.clone()),
                            "body" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_body.clone()),
                            "weapon" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_weapon.clone()),
                            "back" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_back.clone()),
                            "feet" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_feet.clone()),
                            "ring" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_ring.clone()),
                            "gloves" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_gloves.clone()),
                            "necklace" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_necklace.clone()),
                            "belt" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_belt.clone()),
                            _ => None,
                        };
                        if let Some(item_id) = equipped_item {
                            // Start drag from equipment slot
                            state.ui_state.drag_state = Some(DragState {
                                source: DragSource::Equipment(slot_type.clone()),
                                item_id,
                                quantity: 1, // Equipment is always quantity 1
                            });
                            audio.play_sfx("item_grab");
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }

        false
    }
}
