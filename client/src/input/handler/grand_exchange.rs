use super::*;
use crate::game::state::GeEditField;
use crate::render::ui::grand_exchange::{format_solst, parse_price_to_base};

impl InputHandler {
    pub(super) fn handle_grand_exchange(
        &mut self,
        state: &mut GameState,
        _layout: &UiLayout,
        frame: ProcessFrame<'_>,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        // When closed, allow the open hotkey (G) during normal gameplay.
        if !state.ui_state.ge.open {
            let blocked = state.ui_state.chat_open
                || state.ui_state.bank_open
                || state.ui_state.crafting_open
                || state.ui_state.escape_menu_open
                || state.ui_state.active_dialogue.is_some();
            if !blocked && is_key_pressed(KeyCode::G) {
                state.ui_state.ge.open = true;
                state.ui_state.ge.status_msg.clear();
                commands.push(InputCommand::GeOpen);
                return true;
            }
            return false;
        }

        // ===== Open: full modal, consumes all input =====
        if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::G) {
            state.ui_state.ge.open = false;
            state.ui_state.ge.editing = GeEditField::None;
            return true;
        }

        // Numeric field typing.
        let editing = state.ui_state.ge.editing;
        if editing != GeEditField::None {
            while let Some(c) = get_char_pressed() {
                let ge = &mut state.ui_state.ge;
                let allow_dot = editing == GeEditField::Price;
                let buf = match editing {
                    GeEditField::Price => &mut ge.price_input,
                    GeEditField::Quantity => &mut ge.qty_input,
                    GeEditField::None => break,
                };
                if buf.len() >= 16 {
                    continue;
                }
                if c.is_ascii_digit() {
                    buf.push(c);
                } else if c == '.' && allow_dot && !buf.contains('.') {
                    buf.push(c);
                }
            }
            if is_key_pressed(KeyCode::Backspace) {
                let ge = &mut state.ui_state.ge;
                match editing {
                    GeEditField::Price => {
                        ge.price_input.pop();
                    }
                    GeEditField::Quantity => {
                        ge.qty_input.pop();
                    }
                    GeEditField::None => {}
                }
            }
            if is_key_pressed(KeyCode::Enter) {
                state.ui_state.ge.editing = GeEditField::None;
            }
        }

        // Mouse-wheel scrolling for the two lists.
        let (_, wheel_y) = mouse_wheel();
        if wheel_y != 0.0 {
            let s = state.ui_state.ui_scale;
            match &state.ui_state.hovered_element {
                Some(UiElementId::GeMarketScrollArea) | Some(UiElementId::GeMarketRow(_)) => {
                    let len = state.ui_state.ge.market.len() as f32;
                    let step = 30.0 * s;
                    let max = (len - 1.0).max(0.0) * step;
                    let sc = &mut state.ui_state.ge.market_scroll;
                    *sc = (*sc - wheel_y.signum() * step).clamp(0.0, max);
                }
                Some(UiElementId::GeOffersScrollArea)
                | Some(UiElementId::GeOfferCollect(_))
                | Some(UiElementId::GeOfferCancel(_)) => {
                    let len = state.ui_state.ge.offers.len() as f32;
                    let step = 40.0 * s;
                    let max = (len - 1.0).max(0.0) * step;
                    let sc = &mut state.ui_state.ge.offers_scroll;
                    *sc = (*sc - wheel_y.signum() * step).clamp(0.0, max);
                }
                _ => {}
            }
        }

        // Clicks.
        if frame.mouse_clicked {
            if let Some(element) = frame.clicked_element.clone() {
                match element {
                    UiElementId::GeCloseButton => {
                        state.ui_state.ge.open = false;
                        state.ui_state.ge.editing = GeEditField::None;
                    }
                    UiElementId::GeSideBuy => {
                        state.ui_state.ge.side_sell = false;
                    }
                    UiElementId::GeSideSell => {
                        state.ui_state.ge.side_sell = true;
                    }
                    UiElementId::GePriceField => {
                        state.ui_state.ge.editing = GeEditField::Price;
                    }
                    UiElementId::GeQuantityField => {
                        state.ui_state.ge.editing = GeEditField::Quantity;
                    }
                    UiElementId::GeInventorySlot(i) => {
                        if let Some(Some(slot)) = state.inventory.slots.get(i) {
                            state.ui_state.ge.selected_item = Some(slot.item_id.clone());
                        }
                    }
                    UiElementId::GeMarketRow(i) => {
                        if let Some(row) = state.ui_state.ge.market.get(i).cloned() {
                            let decimals = state.ui_state.ge.decimals.max(1);
                            let ge = &mut state.ui_state.ge;
                            ge.selected_item = Some(row.item_id.clone());
                            ge.price_input = format_solst(row.price, decimals);
                            // Take the opposite side of a resting order.
                            ge.side_sell = row.side == "buy";
                        }
                    }
                    UiElementId::GeConfirmButton => {
                        self.ge_submit_offer(state, commands);
                    }
                    UiElementId::GeOfferCollect(i) => {
                        if let Some(offer) = state.ui_state.ge.offers.get(i) {
                            commands.push(InputCommand::GeCollect {
                                offer_id: offer.id,
                            });
                        }
                    }
                    UiElementId::GeOfferCancel(i) => {
                        if let Some(offer) = state.ui_state.ge.offers.get(i) {
                            commands.push(InputCommand::GeCancelOffer {
                                offer_id: offer.id,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        // Drain stray characters so they don't leak elsewhere.
        while get_char_pressed().is_some() {}

        // Modal: swallow everything while open.
        true
    }

    fn ge_submit_offer(&mut self, state: &mut GameState, commands: &mut Vec<InputCommand>) {
        let ge = &state.ui_state.ge;
        let Some(item_id) = ge.selected_item.clone() else {
            state.ui_state.ge.status_msg = "Select an item first.".to_string();
            return;
        };
        let decimals = ge.decimals.max(1);
        let price = parse_price_to_base(&ge.price_input, decimals);
        let qty = ge.qty_input.trim().parse::<i64>().ok();
        let side_sell = ge.side_sell;

        match (price, qty) {
            (Some(p), Some(q)) if p > 0 && q > 0 => {
                commands.push(InputCommand::GePlaceOffer {
                    side: if side_sell { "sell" } else { "buy" }.to_string(),
                    item_id,
                    price: p,
                    quantity: q,
                });
                let ge = &mut state.ui_state.ge;
                ge.qty_input.clear();
                ge.editing = GeEditField::None;
            }
            _ => {
                state.ui_state.ge.status_msg = "Enter a valid price and quantity.".to_string();
            }
        }
    }
}
