//! Grand Exchange UI — a SOLST-priced order-book market.
//!
//! Left column: the live market book. Right column: a create-offer form (with an
//! inventory item picker + numeric price/quantity fields) on top, and the
//! player's own offers (with collect / cancel actions) below.

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::game::state::GeEditField;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

const ROW_H: f32 = 30.0;
const INV_COLS: usize = 10;

/// Format SOLST base units into a human string (e.g. 1500000 -> "1.5").
pub(crate) fn format_solst(base: i64, decimals: u8) -> String {
    let d = 10i64.pow(decimals as u32);
    if d <= 1 {
        return base.to_string();
    }
    let neg = base < 0;
    let abs = base.abs();
    let whole = abs / d;
    let frac = abs % d;
    let body = if frac == 0 {
        whole.to_string()
    } else {
        let s = format!("{:0width$}", frac, width = decimals as usize);
        let trimmed = s.trim_end_matches('0');
        let shown = &trimmed[..trimmed.len().min(3)];
        format!("{}.{}", whole, shown)
    };
    if neg { format!("-{}", body) } else { body }
}

/// Parse a decimal SOLST string into base units.
pub(crate) fn parse_price_to_base(input: &str, decimals: u8) -> Option<i64> {
    let t = input.trim();
    if t.is_empty() {
        return None;
    }
    let mut parts = t.splitn(2, '.');
    let whole_str = parts.next().unwrap_or("");
    let frac_str = parts.next().unwrap_or("");
    let whole: i64 = if whole_str.is_empty() {
        0
    } else {
        whole_str.parse().ok()?
    };
    let d = 10i64.pow(decimals as u32);
    let mut frac: i64 = 0;
    if !frac_str.is_empty() {
        let mut fs = frac_str.to_string();
        if fs.len() > decimals as usize {
            fs.truncate(decimals as usize);
        }
        while fs.len() < decimals as usize {
            fs.push('0');
        }
        frac = fs.parse().ok()?;
    }
    whole.checked_mul(d)?.checked_add(frac)
}

impl Renderer {
    pub(crate) fn render_grand_exchange(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;
        let ge = &state.ui_state.ge;

        let panel_w = (760.0 * s).min(sw - 16.0);
        let panel_h = (540.0 * s).min(sh - 16.0);
        let panel_x = (sw - panel_w) / 2.0;
        let panel_y = (sh - panel_h) / 2.0;

        // Dim + frame.
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.62));
        self.draw_panel_frame(panel_x, panel_y, panel_w, panel_h);
        self.draw_corner_accents(panel_x, panel_y, panel_w, panel_h);

        let header_h = 30.0 * s;
        let inner_x = panel_x + FRAME_THICKNESS;
        let inner_y = panel_y + FRAME_THICKNESS;
        let inner_w = panel_w - FRAME_THICKNESS * 2.0;

        // ===== Header =====
        draw_rectangle(inner_x, inner_y, inner_w, header_h, HEADER_BG);
        draw_line(
            inner_x,
            inner_y + header_h,
            inner_x + inner_w,
            inner_y + header_h,
            1.0,
            HEADER_BORDER,
        );
        let title = "Grand Exchange";
        let td = self.measure_text_sharp(title, 16.0);
        self.draw_text_sharp(
            title,
            inner_x + (inner_w - td.width) / 2.0,
            inner_y + header_h * 0.7,
            16.0,
            TEXT_TITLE,
        );

        // Balance (right side of header).
        let bal_text = format!("{} SOLST", format_solst(ge.balance, ge.decimals.max(1)));
        let bd = self.measure_text_sharp(&bal_text, 14.0);
        self.draw_text_sharp(
            &bal_text,
            inner_x + inner_w - bd.width - 40.0 * s,
            inner_y + header_h * 0.7,
            14.0,
            TEXT_GOLD,
        );

        // Close button.
        let close_sz = 20.0 * s;
        let close_rect = Rect::new(
            inner_x + inner_w - close_sz - 6.0 * s,
            inner_y + (header_h - close_sz) / 2.0,
            close_sz,
            close_sz,
        );
        layout.add(UiElementId::GeCloseButton, close_rect);
        let close_hov = matches!(hovered, Some(UiElementId::GeCloseButton));
        draw_rectangle(
            close_rect.x,
            close_rect.y,
            close_rect.w,
            close_rect.h,
            if close_hov {
                Color::new(0.5, 0.2, 0.2, 1.0)
            } else {
                Color::new(0.2, 0.13, 0.13, 1.0)
            },
        );
        let xd = self.measure_text_sharp("X", 14.0);
        self.draw_text_sharp(
            "X",
            close_rect.x + (close_sz - xd.width) / 2.0,
            close_rect.y + close_sz * 0.72,
            14.0,
            TEXT_NORMAL,
        );

        let content_y = inner_y + header_h + 8.0 * s;
        let content_h = panel_y + panel_h - FRAME_THICKNESS - content_y - 6.0 * s;
        let pad = 10.0 * s;
        let col_gap = 10.0 * s;
        let left_w = (inner_w - pad * 2.0 - col_gap) * 0.46;
        let right_w = inner_w - pad * 2.0 - col_gap - left_w;
        let left_x = inner_x + pad;
        let right_x = left_x + left_w + col_gap;

        self.render_ge_market(state, hovered, layout, left_x, content_y, left_w, content_h);

        let form_h = content_h * 0.52;
        self.render_ge_form(
            state, hovered, layout, right_x, content_y, right_w, form_h,
        );
        self.render_ge_offers(
            state,
            hovered,
            layout,
            right_x,
            content_y + form_h + 8.0 * s,
            right_w,
            content_h - form_h - 8.0 * s,
        );
    }

    fn render_ge_market(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        let s = state.ui_state.ui_scale;
        let ge = &state.ui_state.ge;
        draw_rectangle(x, y, w, h, PANEL_BG_DARK);
        self.draw_text_sharp("Market", x + 6.0 * s, y + 16.0 * s, 13.0, TEXT_TITLE);

        let list_y = y + 24.0 * s;
        let list_h = h - 24.0 * s;
        layout.add(UiElementId::GeMarketScrollArea, Rect::new(x, list_y, w, list_h));

        let row_h = ROW_H * s;
        let max_rows = (list_h / row_h).floor().max(0.0) as usize;
        let scroll = (ge.market_scroll / row_h).floor().max(0.0) as usize;

        if ge.market.is_empty() {
            self.draw_text_sharp(
                "No active offers yet.",
                x + 6.0 * s,
                list_y + 20.0 * s,
                12.0,
                TEXT_DIM,
            );
            return;
        }

        for (vis, idx) in (scroll..ge.market.len()).take(max_rows).enumerate() {
            let row = &ge.market[idx];
            let ry = list_y + vis as f32 * row_h;
            let rect = Rect::new(x, ry, w, row_h - 2.0 * s);
            layout.add(UiElementId::GeMarketRow(idx), rect);
            let hov = matches!(hovered, Some(UiElementId::GeMarketRow(i)) if *i == idx);
            draw_rectangle(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                if hov { SLOT_HOVER_BG } else { SLOT_BG_FILLED },
            );

            // Side tag.
            let is_sell = row.side == "sell";
            let tag = if is_sell { "SELL" } else { "BUY" };
            let tag_color = if is_sell {
                Color::new(0.95, 0.5, 0.4, 1.0)
            } else {
                Color::new(0.5, 0.85, 0.55, 1.0)
            };
            self.draw_text_sharp(tag, rect.x + 4.0 * s, rect.y + row_h * 0.62, 11.0, tag_color);

            // Icon + name.
            let icon = 22.0 * s;
            self.draw_item_icon(
                &row.item_id,
                rect.x + 38.0 * s,
                rect.y + (row_h - icon) / 2.0 - 1.0 * s,
                icon,
                icon,
                state,
                false,
            );
            let name = state
                .item_registry
                .get(&row.item_id)
                .map(|d| d.display_name.clone())
                .unwrap_or_else(|| row.item_id.clone());
            self.draw_text_sharp(&name, rect.x + 64.0 * s, rect.y + row_h * 0.62, 12.0, TEXT_NORMAL);

            // Price x qty (right aligned).
            let info = format!("{} x{}", format_solst(row.price, ge.decimals.max(1)), row.quantity);
            let id = self.measure_text_sharp(&info, 12.0);
            self.draw_text_sharp(
                &info,
                rect.x + w - id.width - 6.0 * s,
                rect.y + row_h * 0.62,
                12.0,
                TEXT_GOLD,
            );
        }
    }

    fn render_ge_form(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        let s = state.ui_state.ui_scale;
        let ge = &state.ui_state.ge;
        draw_rectangle(x, y, w, h, PANEL_BG_DARK);
        self.draw_text_sharp("Create Offer", x + 6.0 * s, y + 16.0 * s, 13.0, TEXT_TITLE);

        // Buy / Sell toggle.
        let btn_w = (w - 18.0 * s) / 2.0;
        let btn_h = 24.0 * s;
        let toggle_y = y + 24.0 * s;
        let buy_rect = Rect::new(x + 6.0 * s, toggle_y, btn_w, btn_h);
        let sell_rect = Rect::new(x + 12.0 * s + btn_w, toggle_y, btn_w, btn_h);
        layout.add(UiElementId::GeSideBuy, buy_rect);
        layout.add(UiElementId::GeSideSell, sell_rect);
        self.ge_tab(buy_rect, "Buy", !ge.side_sell, hovered, UiElementId::GeSideBuy, s);
        self.ge_tab(
            sell_rect,
            "Sell",
            ge.side_sell,
            hovered,
            UiElementId::GeSideSell,
            s,
        );

        // Selected item display.
        let sel_y = toggle_y + btn_h + 6.0 * s;
        let icon = 26.0 * s;
        self.draw_item_icon_or_blank(state, ge.selected_item.as_deref(), x + 6.0 * s, sel_y, icon);
        let sel_name = match &ge.selected_item {
            Some(id) => state
                .item_registry
                .get(id)
                .map(|d| d.display_name.clone())
                .unwrap_or_else(|| id.clone()),
            None => "Pick an item below".to_string(),
        };
        self.draw_text_sharp(
            &sel_name,
            x + 12.0 * s + icon,
            sel_y + icon * 0.7,
            13.0,
            if ge.selected_item.is_some() {
                TEXT_NORMAL
            } else {
                TEXT_DIM
            },
        );

        // Price + quantity fields.
        let field_y = sel_y + icon + 6.0 * s;
        let field_h = 22.0 * s;
        let field_w = (w - 18.0 * s) / 2.0;
        let price_rect = Rect::new(x + 6.0 * s, field_y, field_w, field_h);
        let qty_rect = Rect::new(x + 12.0 * s + field_w, field_y, field_w, field_h);
        layout.add(UiElementId::GePriceField, price_rect);
        layout.add(UiElementId::GeQuantityField, qty_rect);
        self.ge_field(
            price_rect,
            "Price (SOLST)",
            &ge.price_input,
            ge.editing == GeEditField::Price,
            hovered,
            UiElementId::GePriceField,
            s,
        );
        self.ge_field(
            qty_rect,
            "Quantity",
            &ge.qty_input,
            ge.editing == GeEditField::Quantity,
            hovered,
            UiElementId::GeQuantityField,
            s,
        );

        // Inventory picker strip.
        let inv_y = field_y + field_h + 6.0 * s;
        let inv_label_h = 14.0 * s;
        self.draw_text_sharp("Inventory:", x + 6.0 * s, inv_y + 11.0 * s, 11.0, TEXT_DIM);
        let grid_y = inv_y + inv_label_h;
        let cell = ((w - 12.0 * s) / INV_COLS as f32).min(28.0 * s);
        layout.add(
            UiElementId::GeInvScrollArea,
            Rect::new(x + 6.0 * s, grid_y, w - 12.0 * s, cell * 2.0),
        );
        for (i, slot) in state.inventory.slots.iter().enumerate() {
            let col = i % INV_COLS;
            let r = i / INV_COLS;
            if r >= 2 {
                break;
            }
            let cx = x + 6.0 * s + col as f32 * cell;
            let cy = grid_y + r as f32 * cell;
            let rect = Rect::new(cx, cy, cell - 2.0 * s, cell - 2.0 * s);
            layout.add(UiElementId::GeInventorySlot(i), rect);
            let hov = matches!(hovered, Some(UiElementId::GeInventorySlot(j)) if *j == i);
            let selected = slot
                .as_ref()
                .map(|sl| Some(&sl.item_id) == ge.selected_item.as_ref())
                .unwrap_or(false);
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, SLOT_BG_EMPTY);
            if let Some(sl) = slot {
                self.draw_item_icon(&sl.item_id, rect.x, rect.y, rect.w, rect.h, state, false);
            }
            let border = if selected {
                SLOT_SELECTED_BORDER
            } else if hov {
                SLOT_HOVER_BORDER
            } else {
                SLOT_BORDER
            };
            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.5 * s, border);
        }

        // Confirm button.
        let confirm_y = grid_y + cell * 2.0 + 6.0 * s;
        let confirm_rect = Rect::new(x + 6.0 * s, confirm_y, w - 12.0 * s, 26.0 * s);
        layout.add(UiElementId::GeConfirmButton, confirm_rect);
        let hov = matches!(hovered, Some(UiElementId::GeConfirmButton));
        let base = if ge.side_sell {
            Color::new(0.45, 0.22, 0.18, 1.0)
        } else {
            Color::new(0.2, 0.4, 0.24, 1.0)
        };
        draw_rectangle(
            confirm_rect.x,
            confirm_rect.y,
            confirm_rect.w,
            confirm_rect.h,
            if hov {
                Color::new(base.r + 0.1, base.g + 0.1, base.b + 0.1, 1.0)
            } else {
                base
            },
        );
        let label = if ge.side_sell {
            "Place Sell Offer"
        } else {
            "Place Buy Offer"
        };
        let ld = self.measure_text_sharp(label, 13.0);
        self.draw_text_sharp(
            label,
            confirm_rect.x + (confirm_rect.w - ld.width) / 2.0,
            confirm_rect.y + 17.0 * s,
            13.0,
            TEXT_NORMAL,
        );

        // Status message.
        if !ge.status_msg.is_empty() {
            self.draw_text_sharp(
                &ge.status_msg,
                x + 6.0 * s,
                confirm_y + 40.0 * s,
                11.0,
                TEXT_DIM,
            );
        }
    }

    fn render_ge_offers(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        let s = state.ui_state.ui_scale;
        let ge = &state.ui_state.ge;
        draw_rectangle(x, y, w, h, PANEL_BG_DARK);
        self.draw_text_sharp("Your Offers", x + 6.0 * s, y + 16.0 * s, 13.0, TEXT_TITLE);

        let list_y = y + 24.0 * s;
        let list_h = h - 24.0 * s;
        layout.add(
            UiElementId::GeOffersScrollArea,
            Rect::new(x, list_y, w, list_h),
        );

        if ge.offers.is_empty() {
            self.draw_text_sharp(
                "You have no offers.",
                x + 6.0 * s,
                list_y + 18.0 * s,
                12.0,
                TEXT_DIM,
            );
            return;
        }

        let row_h = 40.0 * s;
        let max_rows = (list_h / row_h).floor().max(0.0) as usize;
        let scroll = (ge.offers_scroll / row_h).floor().max(0.0) as usize;

        for (vis, idx) in (scroll..ge.offers.len()).take(max_rows).enumerate() {
            let offer = &ge.offers[idx];
            let ry = list_y + vis as f32 * row_h;
            draw_rectangle(x, ry, w, row_h - 3.0 * s, SLOT_BG_FILLED);

            let is_sell = offer.side == "sell";
            let tag = if is_sell { "SELL" } else { "BUY" };
            let tag_color = if is_sell {
                Color::new(0.95, 0.5, 0.4, 1.0)
            } else {
                Color::new(0.5, 0.85, 0.55, 1.0)
            };
            self.draw_text_sharp(tag, x + 4.0 * s, ry + 14.0 * s, 10.0, tag_color);

            let icon = 18.0 * s;
            self.draw_item_icon(&offer.item_id, x + 34.0 * s, ry + 3.0 * s, icon, icon, state, false);
            let name = state
                .item_registry
                .get(&offer.item_id)
                .map(|d| d.display_name.clone())
                .unwrap_or_else(|| offer.item_id.clone());
            self.draw_text_sharp(&name, x + 56.0 * s, ry + 14.0 * s, 11.0, TEXT_NORMAL);

            let filled = offer.quantity - offer.remaining;
            let progress = format!(
                "{}/{} @ {}",
                filled,
                offer.quantity,
                format_solst(offer.price, ge.decimals.max(1))
            );
            self.draw_text_sharp(&progress, x + 56.0 * s, ry + 30.0 * s, 10.0, TEXT_DIM);

            // Action buttons (right side).
            let btn_h = 16.0 * s;
            let btn_w = 56.0 * s;
            let bx = x + w - btn_w - 4.0 * s;
            if offer.collect_items > 0 {
                let r = Rect::new(bx, ry + 3.0 * s, btn_w, btn_h);
                layout.add(UiElementId::GeOfferCollect(idx), r);
                let hov = matches!(hovered, Some(UiElementId::GeOfferCollect(i)) if *i == idx);
                draw_rectangle(
                    r.x,
                    r.y,
                    r.w,
                    r.h,
                    if hov {
                        Color::new(0.25, 0.5, 0.3, 1.0)
                    } else {
                        Color::new(0.18, 0.38, 0.22, 1.0)
                    },
                );
                let lbl = format!("Get {}", offer.collect_items);
                let d = self.measure_text_sharp(&lbl, 10.0);
                self.draw_text_sharp(&lbl, r.x + (btn_w - d.width) / 2.0, r.y + 12.0 * s, 10.0, TEXT_NORMAL);
            }
            if offer.remaining > 0 {
                let r = Rect::new(bx, ry + 21.0 * s, btn_w, btn_h);
                layout.add(UiElementId::GeOfferCancel(idx), r);
                let hov = matches!(hovered, Some(UiElementId::GeOfferCancel(i)) if *i == idx);
                draw_rectangle(
                    r.x,
                    r.y,
                    r.w,
                    r.h,
                    if hov {
                        Color::new(0.5, 0.25, 0.25, 1.0)
                    } else {
                        Color::new(0.38, 0.18, 0.18, 1.0)
                    },
                );
                let d = self.measure_text_sharp("Cancel", 10.0);
                self.draw_text_sharp(
                    "Cancel",
                    r.x + (btn_w - d.width) / 2.0,
                    r.y + 12.0 * s,
                    10.0,
                    TEXT_NORMAL,
                );
            }
        }
    }

    fn ge_tab(
        &self,
        rect: Rect,
        label: &str,
        active: bool,
        hovered: &Option<UiElementId>,
        id: UiElementId,
        s: f32,
    ) {
        let hov = hovered.as_ref() == Some(&id);
        let bg = if active {
            Color::new(0.3, 0.27, 0.2, 1.0)
        } else if hov {
            SLOT_HOVER_BG
        } else {
            SLOT_BG_FILLED
        };
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
        let border = if active { SLOT_SELECTED_BORDER } else { SLOT_BORDER };
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.5 * s, border);
        let d = self.measure_text_sharp(label, 13.0);
        self.draw_text_sharp(
            label,
            rect.x + (rect.w - d.width) / 2.0,
            rect.y + rect.h * 0.68,
            13.0,
            if active { TEXT_TITLE } else { TEXT_NORMAL },
        );
    }

    fn ge_field(
        &self,
        rect: Rect,
        placeholder: &str,
        value: &str,
        editing: bool,
        hovered: &Option<UiElementId>,
        id: UiElementId,
        s: f32,
    ) {
        let hov = hovered.as_ref() == Some(&id);
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, SLOT_BG_EMPTY);
        let border = if editing {
            SLOT_SELECTED_BORDER
        } else if hov {
            SLOT_HOVER_BORDER
        } else {
            SLOT_BORDER
        };
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.5 * s, border);
        let (text, color) = if value.is_empty() {
            (placeholder.to_string(), TEXT_DIM)
        } else {
            (value.to_string(), TEXT_NORMAL)
        };
        let shown = if editing {
            format!("{}_", value)
        } else {
            text
        };
        self.draw_text_sharp(&shown, rect.x + 5.0 * s, rect.y + rect.h * 0.68, 12.0, color);
    }

    fn draw_item_icon_or_blank(
        &self,
        state: &GameState,
        item_id: Option<&str>,
        x: f32,
        y: f32,
        size: f32,
    ) {
        draw_rectangle(x, y, size, size, SLOT_BG_EMPTY);
        if let Some(id) = item_id {
            self.draw_item_icon(id, x, y, size, size, state, false);
        }
        draw_rectangle_lines(x, y, size, size, 1.5, SLOT_BORDER);
    }
}
