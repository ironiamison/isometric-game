//! Trade window UI panel — medieval fantasy styled two-column layout

use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

// Status colors
const STATUS_ACCEPTED: Color = Color::new(0.35, 0.75, 0.45, 1.0);
const STATUS_PENDING: Color = Color::new(0.784, 0.55, 0.20, 1.0);

impl super::super::Renderer {
    pub(crate) fn render_trade_panel(&self, state: &GameState, layout: &mut UiLayout) {
        if !state.ui_state.trade_open {
            return;
        }

        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;

        let panel_width = (520.0 * s).min(sw - 16.0);
        let panel_height = (500.0 * s).min(sh - 16.0);
        let panel_x = (sw - panel_width) / 2.0;
        let panel_y = (sh - panel_height) / 2.0;

        let header_h = HEADER_HEIGHT * s;
        let footer_h = FOOTER_HEIGHT * s;

        // Panel frame + corner accents
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        // ===== HEADER =====
        let header_x = panel_x + FRAME_THICKNESS;
        let header_y = panel_y + FRAME_THICKNESS;
        let header_w = panel_width - FRAME_THICKNESS * 2.0;

        draw_rectangle(header_x, header_y, header_w, header_h, PANEL_BG_MID);
        draw_line(
            header_x + 10.0 * s,
            header_y + header_h,
            header_x + header_w - 10.0 * s,
            header_y + header_h,
            2.0,
            HEADER_BORDER,
        );

        // Decorative dots along header border
        let dot_spacing = 60.0 * s;
        let num_dots = ((header_w - 40.0 * s) / dot_spacing) as i32;
        let start_dot_x = header_x + 20.0 * s;
        for i in 0..num_dots {
            let dot_x = start_dot_x + i as f32 * dot_spacing;
            draw_rectangle(
                dot_x - 1.5,
                header_y + header_h - 1.5,
                3.0,
                3.0,
                FRAME_ACCENT,
            );
        }

        // Title
        let partner_name = state
            .ui_state
            .trade_partner_name
            .as_deref()
            .unwrap_or("Player");
        let title = format!("TRADE WITH {}", partner_name.to_uppercase());
        let title_dims = self.measure_text_sharp(&title, 16.0);
        self.draw_text_sharp(
            &title,
            header_x + (header_w - title_dims.width) / 2.0,
            header_y + header_h * 0.65,
            16.0,
            TEXT_TITLE,
        );

        // Close button (X) — mapped to TradeCancelButton
        let is_mobile = cfg!(target_os = "android");
        let close_btn_size = if is_mobile { 32.0 * s } else { 28.0 * s };
        let close_btn_x = header_x + header_w - close_btn_size - 6.0 * s;
        let close_btn_y = header_y + (header_h - close_btn_size) / 2.0;
        let close_bounds = Rect::new(close_btn_x, close_btn_y, close_btn_size, close_btn_size);
        layout.add(UiElementId::TradeCancelButton, close_bounds);

        let is_close_hovered =
            state.ui_state.hovered_element.as_ref() == Some(&UiElementId::TradeCancelButton);
        let (close_bg, close_border) = if is_close_hovered {
            (
                Color::new(0.4, 0.15, 0.15, 1.0),
                Color::new(0.6, 0.2, 0.2, 1.0),
            )
        } else {
            (Color::new(0.2, 0.1, 0.1, 1.0), FRAME_MID)
        };
        draw_rectangle(
            close_btn_x,
            close_btn_y,
            close_btn_size,
            close_btn_size,
            close_border,
        );
        draw_rectangle(
            close_btn_x + 1.0,
            close_btn_y + 1.0,
            close_btn_size - 2.0,
            close_btn_size - 2.0,
            close_bg,
        );
        let cx = close_btn_x + close_btn_size / 2.0;
        let cy = close_btn_y + close_btn_size / 2.0;
        let cross = close_btn_size * 0.25;
        let cross_color = if is_close_hovered {
            TEXT_TITLE
        } else {
            TEXT_DIM
        };
        draw_line(
            cx - cross,
            cy - cross,
            cx + cross,
            cy + cross,
            2.0,
            cross_color,
        );
        draw_line(
            cx + cross,
            cy - cross,
            cx - cross,
            cy + cross,
            2.0,
            cross_color,
        );

        // ===== CONTENT AREA =====
        let content_x = panel_x + FRAME_THICKNESS + 8.0 * s;
        let content_y = header_y + header_h + 6.0 * s;
        let content_w = panel_width - FRAME_THICKNESS * 2.0 - 16.0 * s;
        let footer_y = panel_y + panel_height - FRAME_THICKNESS - footer_h;
        let content_h = footer_y - content_y - 4.0 * s;

        // Two columns
        let col_gap = 8.0 * s;
        let col_w = (content_w - col_gap) / 2.0;
        let left_col_x = content_x;
        let right_col_x = content_x + col_w + col_gap;

        // Column sub-header height
        let col_header_h = 24.0 * s;
        let slot_h = 52.0 * s;
        let slot_gap = 3.0 * s;
        let max_slots = 6;
        let gold_row_h = 32.0 * s;

        // ===== LEFT COLUMN: YOUR OFFER =====
        // Column header background
        draw_rectangle(left_col_x, content_y, col_w, col_header_h, PANEL_BG_MID);
        draw_line(
            left_col_x + 4.0 * s,
            content_y + col_header_h,
            left_col_x + col_w - 4.0 * s,
            content_y + col_header_h,
            1.0,
            HEADER_BORDER,
        );
        let left_title = "YOUR OFFER";
        let left_title_dims = self.measure_text_sharp(left_title, 16.0);
        self.draw_text_sharp(
            left_title,
            left_col_x + (col_w - left_title_dims.width) / 2.0,
            content_y + col_header_h * 0.68,
            16.0,
            TEXT_TITLE,
        );

        // ===== RIGHT COLUMN: THEIR OFFER =====
        draw_rectangle(right_col_x, content_y, col_w, col_header_h, PANEL_BG_MID);
        draw_line(
            right_col_x + 4.0 * s,
            content_y + col_header_h,
            right_col_x + col_w - 4.0 * s,
            content_y + col_header_h,
            1.0,
            HEADER_BORDER,
        );
        let right_title = "THEIR OFFER";
        let right_title_dims = self.measure_text_sharp(right_title, 16.0);
        self.draw_text_sharp(
            right_title,
            right_col_x + (col_w - right_title_dims.width) / 2.0,
            content_y + col_header_h * 0.68,
            16.0,
            TEXT_TITLE,
        );

        // Vertical divider between columns
        let divider_x = content_x + col_w + col_gap / 2.0;
        draw_line(
            divider_x,
            content_y + col_header_h + 4.0 * s,
            divider_x,
            content_y + content_h - 4.0 * s,
            1.0,
            SLOT_BORDER,
        );

        // ===== LEFT COLUMN ITEMS =====
        let items_start_y = content_y + col_header_h + 4.0 * s;
        let icon_size = 46.0 * s;

        for i in 0..max_slots {
            let slot_y = items_start_y + i as f32 * (slot_h + slot_gap);

            if i < state.ui_state.trade_my_items.len() {
                let item = &state.ui_state.trade_my_items[i];
                let item_def = state.item_registry.get_or_placeholder(&item.item_id);

                let bounds = Rect::new(left_col_x, slot_y, col_w, slot_h);
                layout.add(UiElementId::TradeOfferSlot(i), bounds);

                let is_hovered = state.ui_state.hovered_element.as_ref()
                    == Some(&UiElementId::TradeOfferSlot(i));

                // Slot background + border
                let (slot_bg, slot_border) = if is_hovered {
                    (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
                } else {
                    (SLOT_BG_FILLED, SLOT_BORDER)
                };
                draw_rectangle(left_col_x, slot_y, col_w, slot_h, slot_border);
                draw_rectangle(
                    left_col_x + 1.0,
                    slot_y + 1.0,
                    col_w - 2.0,
                    slot_h - 2.0,
                    slot_bg,
                );
                // Inner shadow top
                draw_line(
                    left_col_x + 2.0,
                    slot_y + 2.0,
                    left_col_x + col_w - 2.0,
                    slot_y + 2.0,
                    1.0,
                    SLOT_INNER_SHADOW,
                );

                // Item icon
                let icon_x = left_col_x + 4.0 * s;
                let icon_y = slot_y + (slot_h - icon_size) / 2.0;
                self.draw_item_icon(
                    &item.item_id,
                    icon_x,
                    icon_y,
                    icon_size,
                    icon_size,
                    state,
                    false,
                );

                // Item name
                let text_x = icon_x + icon_size + 6.0 * s;
                let name_color = if is_hovered { TEXT_TITLE } else { TEXT_NORMAL };
                self.draw_text_sharp(
                    &item_def.display_name,
                    text_x,
                    slot_y + slot_h * 0.42,
                    16.0,
                    name_color,
                );

                // Quantity
                let qty_text = format!("x{}", item.quantity);
                self.draw_text_sharp(&qty_text, text_x, slot_y + slot_h * 0.78, 16.0, TEXT_DIM);
            } else {
                // Empty slot
                draw_rectangle(left_col_x, slot_y, col_w, slot_h, SLOT_BORDER);
                draw_rectangle(
                    left_col_x + 1.0,
                    slot_y + 1.0,
                    col_w - 2.0,
                    slot_h - 2.0,
                    SLOT_BG_EMPTY,
                );
                draw_line(
                    left_col_x + 2.0,
                    slot_y + 2.0,
                    left_col_x + col_w - 2.0,
                    slot_y + 2.0,
                    1.0,
                    SLOT_INNER_SHADOW,
                );
            }
        }

        // Left gold row
        let gold_y = items_start_y + max_slots as f32 * (slot_h + slot_gap) + 2.0 * s;
        let gold_bounds = Rect::new(left_col_x, gold_y, col_w, gold_row_h);
        layout.add(UiElementId::TradeGoldInput, gold_bounds);

        let gold_hovered =
            state.ui_state.hovered_element.as_ref() == Some(&UiElementId::TradeGoldInput);
        let (gold_bg, gold_border) = if gold_hovered {
            (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
        } else {
            (SLOT_BG_FILLED, SLOT_BORDER)
        };
        draw_rectangle(left_col_x, gold_y, col_w, gold_row_h, gold_border);
        draw_rectangle(
            left_col_x + 1.0,
            gold_y + 1.0,
            col_w - 2.0,
            gold_row_h - 2.0,
            gold_bg,
        );

        let gold_label = format!("Gold: {}g", state.ui_state.trade_my_gold);
        let gold_dims = self.measure_text_sharp(&gold_label, 16.0);
        self.draw_text_sharp(
            &gold_label,
            left_col_x + (col_w - gold_dims.width) / 2.0,
            gold_y + gold_row_h * 0.68,
            16.0,
            TEXT_GOLD,
        );

        // ===== RIGHT COLUMN ITEMS =====
        for i in 0..max_slots {
            let slot_y = items_start_y + i as f32 * (slot_h + slot_gap);

            if i < state.ui_state.trade_partner_items.len() {
                let item = &state.ui_state.trade_partner_items[i];
                let item_def = state.item_registry.get_or_placeholder(&item.item_id);

                let bounds = Rect::new(right_col_x, slot_y, col_w, slot_h);
                layout.add(UiElementId::TradePartnerSlot(i), bounds);

                let is_hovered = state.ui_state.hovered_element.as_ref()
                    == Some(&UiElementId::TradePartnerSlot(i));

                let (slot_bg, slot_border) = if is_hovered {
                    (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
                } else {
                    (SLOT_BG_FILLED, SLOT_BORDER)
                };
                draw_rectangle(right_col_x, slot_y, col_w, slot_h, slot_border);
                draw_rectangle(
                    right_col_x + 1.0,
                    slot_y + 1.0,
                    col_w - 2.0,
                    slot_h - 2.0,
                    slot_bg,
                );
                draw_line(
                    right_col_x + 2.0,
                    slot_y + 2.0,
                    right_col_x + col_w - 2.0,
                    slot_y + 2.0,
                    1.0,
                    SLOT_INNER_SHADOW,
                );

                // Item icon
                let icon_x = right_col_x + 4.0 * s;
                let icon_y = slot_y + (slot_h - icon_size) / 2.0;
                self.draw_item_icon(
                    &item.item_id,
                    icon_x,
                    icon_y,
                    icon_size,
                    icon_size,
                    state,
                    false,
                );

                // Item name
                let text_x = icon_x + icon_size + 6.0 * s;
                let name_color = if is_hovered { TEXT_TITLE } else { TEXT_NORMAL };
                self.draw_text_sharp(
                    &item_def.display_name,
                    text_x,
                    slot_y + slot_h * 0.42,
                    16.0,
                    name_color,
                );

                // Quantity
                let qty_text = format!("x{}", item.quantity);
                self.draw_text_sharp(&qty_text, text_x, slot_y + slot_h * 0.78, 16.0, TEXT_DIM);
            } else {
                // Empty slot
                draw_rectangle(right_col_x, slot_y, col_w, slot_h, SLOT_BORDER);
                draw_rectangle(
                    right_col_x + 1.0,
                    slot_y + 1.0,
                    col_w - 2.0,
                    slot_h - 2.0,
                    SLOT_BG_EMPTY,
                );
                draw_line(
                    right_col_x + 2.0,
                    slot_y + 2.0,
                    right_col_x + col_w - 2.0,
                    slot_y + 2.0,
                    1.0,
                    SLOT_INNER_SHADOW,
                );
            }
        }

        // Right gold row (display only, no interaction)
        let partner_gold_label = format!("Gold: {}g", state.ui_state.trade_partner_gold);
        draw_rectangle(right_col_x, gold_y, col_w, gold_row_h, SLOT_BORDER);
        draw_rectangle(
            right_col_x + 1.0,
            gold_y + 1.0,
            col_w - 2.0,
            gold_row_h - 2.0,
            SLOT_BG_FILLED,
        );
        let pg_dims = self.measure_text_sharp(&partner_gold_label, 16.0);
        self.draw_text_sharp(
            &partner_gold_label,
            right_col_x + (col_w - pg_dims.width) / 2.0,
            gold_y + gold_row_h * 0.68,
            16.0,
            TEXT_GOLD,
        );

        // ===== FOOTER =====
        let footer_x = panel_x + FRAME_THICKNESS;
        let footer_w = panel_width - FRAME_THICKNESS * 2.0;

        draw_rectangle(footer_x, footer_y, footer_w, footer_h, FOOTER_BG);
        draw_line(
            footer_x + 10.0 * s,
            footer_y,
            footer_x + footer_w - 10.0 * s,
            footer_y,
            1.0,
            HEADER_BORDER,
        );

        // Status text in footer (centered)
        let my_status_text = if state.ui_state.trade_my_accepted {
            "You: Accepted"
        } else {
            "You: Pending"
        };
        let partner_status_text = if state.ui_state.trade_partner_accepted {
            "Partner: Accepted"
        } else {
            "Partner: Pending"
        };
        let my_status_color = if state.ui_state.trade_my_accepted {
            STATUS_ACCEPTED
        } else {
            STATUS_PENDING
        };
        let partner_status_color = if state.ui_state.trade_partner_accepted {
            STATUS_ACCEPTED
        } else {
            STATUS_PENDING
        };

        // Status on left side of footer
        let status_x = footer_x + 10.0 * s;
        let status_y = footer_y + footer_h * 0.67;
        self.draw_text_sharp(my_status_text, status_x, status_y, 16.0, my_status_color);
        let my_w = self.measure_text_sharp(my_status_text, 16.0).width;
        // Separator dot
        self.draw_text_sharp(" | ", status_x + my_w, status_y, 16.0, TEXT_DIM);
        let sep_w = self.measure_text_sharp(" | ", 16.0).width;
        self.draw_text_sharp(
            partner_status_text,
            status_x + my_w + sep_w,
            status_y,
            16.0,
            partner_status_color,
        );

        // Accept button (right side of footer)
        let accept_btn_w = 90.0 * s;
        let accept_btn_h = 24.0 * s;
        let accept_btn_x = footer_x + footer_w - accept_btn_w - 8.0 * s;
        let accept_btn_y = footer_y + (footer_h - accept_btn_h) / 2.0;

        let accept_bounds = Rect::new(accept_btn_x, accept_btn_y, accept_btn_w, accept_btn_h);
        layout.add(UiElementId::TradeAcceptButton, accept_bounds);

        let is_accept_hovered =
            state.ui_state.hovered_element.as_ref() == Some(&UiElementId::TradeAcceptButton);
        let already_accepted = state.ui_state.trade_my_accepted;

        let (btn_bg, btn_border) = if already_accepted {
            // Already accepted: green tint
            (
                Color::new(0.15, 0.4, 0.15, 1.0),
                Color::new(0.25, 0.6, 0.25, 1.0),
            )
        } else if is_accept_hovered {
            (
                Color::new(0.2, 0.5, 0.2, 1.0),
                Color::new(0.3, 0.7, 0.3, 1.0),
            )
        } else {
            (SLOT_BG_EMPTY, SLOT_BORDER)
        };

        draw_rectangle(
            accept_btn_x,
            accept_btn_y,
            accept_btn_w,
            accept_btn_h,
            btn_border,
        );
        draw_rectangle(
            accept_btn_x + 1.0,
            accept_btn_y + 1.0,
            accept_btn_w - 2.0,
            accept_btn_h - 2.0,
            btn_bg,
        );

        // Highlight line on top of button
        if already_accepted || is_accept_hovered {
            draw_line(
                accept_btn_x + 2.0,
                accept_btn_y + 2.0,
                accept_btn_x + accept_btn_w - 2.0,
                accept_btn_y + 2.0,
                1.0,
                Color::new(0.3, 0.7, 0.3, 1.0),
            );
        }

        let accept_label = if already_accepted {
            "ACCEPTED"
        } else {
            "ACCEPT"
        };
        let accept_label_dims = self.measure_text_sharp(accept_label, 16.0);
        let accept_text_color = if already_accepted {
            STATUS_ACCEPTED
        } else if is_accept_hovered {
            WHITE
        } else {
            TEXT_NORMAL
        };
        self.draw_text_sharp(
            accept_label,
            accept_btn_x + (accept_btn_w - accept_label_dims.width) / 2.0,
            accept_btn_y + accept_btn_h * 0.71,
            16.0,
            accept_text_color,
        );
    }

    pub(crate) fn render_trade_request_popup(&self, state: &GameState, layout: &mut UiLayout) {
        if let Some((_, ref name)) = state.ui_state.trade_pending_request {
            let (sw, _sh) = virtual_screen_size();
            let s = state.ui_state.ui_scale;

            let popup_w = (300.0 * s).min(sw - 16.0);
            let popup_h = 110.0 * s;
            let popup_x = ((sw - popup_w) / 2.0).floor();
            let popup_y = 40.0 * s;

            // Semi-transparent backdrop scrim (subtle, since this is a popup not full modal)
            draw_rectangle(
                0.0,
                0.0,
                sw,
                popup_y + popup_h + 20.0,
                Color::new(0.0, 0.0, 0.0, 0.3),
            );

            // Panel frame + corner accents
            self.draw_panel_frame(popup_x, popup_y, popup_w, popup_h);
            self.draw_corner_accents(popup_x, popup_y, popup_w, popup_h);

            // Header area
            let header_x = popup_x + FRAME_THICKNESS;
            let header_y = popup_y + FRAME_THICKNESS;
            let header_w = popup_w - FRAME_THICKNESS * 2.0;
            let header_h = 32.0 * s;

            draw_rectangle(header_x, header_y, header_w, header_h, PANEL_BG_MID);
            draw_line(
                header_x + 8.0 * s,
                header_y + header_h,
                header_x + header_w - 8.0 * s,
                header_y + header_h,
                2.0,
                HEADER_BORDER,
            );

            // Decorative dots
            let dot_spacing = 50.0 * s;
            let num_dots = ((header_w - 30.0 * s) / dot_spacing) as i32;
            let start_dot_x = header_x + 15.0 * s;
            for i in 0..num_dots {
                let dot_x = start_dot_x + i as f32 * dot_spacing;
                draw_rectangle(
                    dot_x - 1.5,
                    header_y + header_h - 1.5,
                    3.0,
                    3.0,
                    FRAME_ACCENT,
                );
            }

            let title = "TRADE REQUEST";
            let title_dims = self.measure_text_sharp(title, 16.0);
            self.draw_text_sharp(
                title,
                header_x + (header_w - title_dims.width) / 2.0,
                header_y + header_h * 0.65,
                16.0,
                TEXT_TITLE,
            );

            // Body text
            let body_y = header_y + header_h + 8.0 * s;
            let text = format!("{} wants to trade with you", name);
            let text_dims = self.measure_text_sharp(&text, 16.0);
            self.draw_text_sharp(
                &text,
                popup_x + (popup_w - text_dims.width) / 2.0,
                body_y + 14.0 * s,
                16.0,
                TEXT_NORMAL,
            );

            // Buttons
            let btn_w = 100.0 * s;
            let btn_h = 26.0 * s;
            let btn_y = body_y + 30.0 * s;
            let btn_gap = 12.0 * s;

            // Accept button
            let accept_x = popup_x + popup_w / 2.0 - btn_w - btn_gap / 2.0;
            let accept_bounds = Rect::new(accept_x, btn_y, btn_w, btn_h);
            layout.add(UiElementId::TradeRequestAccept, accept_bounds);

            let is_accept_hovered =
                state.ui_state.hovered_element.as_ref() == Some(&UiElementId::TradeRequestAccept);
            let (accept_bg, accept_border) = if is_accept_hovered {
                (
                    Color::new(0.2, 0.5, 0.2, 1.0),
                    Color::new(0.3, 0.7, 0.3, 1.0),
                )
            } else {
                (
                    Color::new(0.15, 0.4, 0.15, 1.0),
                    Color::new(0.25, 0.6, 0.25, 1.0),
                )
            };

            draw_rectangle(accept_x, btn_y, btn_w, btn_h, accept_border);
            draw_rectangle(
                accept_x + 1.0,
                btn_y + 1.0,
                btn_w - 2.0,
                btn_h - 2.0,
                accept_bg,
            );
            if is_accept_hovered {
                draw_line(
                    accept_x + 2.0,
                    btn_y + 2.0,
                    accept_x + btn_w - 2.0,
                    btn_y + 2.0,
                    1.0,
                    Color::new(0.3, 0.7, 0.3, 1.0),
                );
            }
            let accept_text = "ACCEPT";
            let accept_dims = self.measure_text_sharp(accept_text, 16.0);
            let accept_color = if is_accept_hovered {
                WHITE
            } else {
                Color::new(0.35, 0.75, 0.45, 1.0)
            };
            self.draw_text_sharp(
                accept_text,
                accept_x + (btn_w - accept_dims.width) / 2.0,
                btn_y + btn_h * 0.71,
                16.0,
                accept_color,
            );

            // Decline button
            let decline_x = popup_x + popup_w / 2.0 + btn_gap / 2.0;
            let decline_bounds = Rect::new(decline_x, btn_y, btn_w, btn_h);
            layout.add(UiElementId::TradeRequestDecline, decline_bounds);

            let is_decline_hovered =
                state.ui_state.hovered_element.as_ref() == Some(&UiElementId::TradeRequestDecline);
            let (decline_bg, decline_border) = if is_decline_hovered {
                (
                    Color::new(0.5, 0.15, 0.15, 1.0),
                    Color::new(0.7, 0.2, 0.2, 1.0),
                )
            } else {
                (
                    Color::new(0.35, 0.12, 0.12, 1.0),
                    Color::new(0.5, 0.18, 0.18, 1.0),
                )
            };

            draw_rectangle(decline_x, btn_y, btn_w, btn_h, decline_border);
            draw_rectangle(
                decline_x + 1.0,
                btn_y + 1.0,
                btn_w - 2.0,
                btn_h - 2.0,
                decline_bg,
            );
            if is_decline_hovered {
                draw_line(
                    decline_x + 2.0,
                    btn_y + 2.0,
                    decline_x + btn_w - 2.0,
                    btn_y + 2.0,
                    1.0,
                    Color::new(0.7, 0.2, 0.2, 1.0),
                );
            }
            let decline_text = "DECLINE";
            let decline_dims = self.measure_text_sharp(decline_text, 16.0);
            let decline_color = if is_decline_hovered {
                WHITE
            } else {
                Color::new(0.85, 0.4, 0.4, 1.0)
            };
            self.draw_text_sharp(
                decline_text,
                decline_x + (btn_w - decline_dims.width) / 2.0,
                btn_y + btn_h * 0.71,
                16.0,
                decline_color,
            );
        }
    }
}
