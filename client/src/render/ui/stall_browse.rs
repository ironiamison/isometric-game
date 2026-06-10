//! Stall browse panel (for buyers viewing another player's shop)
//! Polished medieval fantasy style matching anvil and alchemy station panels.

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

impl Renderer {
    pub(crate) fn render_stall_browse_panel(&self, state: &GameState, layout: &mut UiLayout) {
        let browse = match &state.ui_state.stall_browse {
            Some(b) => b,
            None => return,
        };

        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;

        // Panel sizing — compact shop panel
        let panel_width = (380.0 * s).min(sw - 16.0);
        let max_rows = 8;
        let row_h = 54.0 * s;
        let header_h = HEADER_HEIGHT * s;
        let footer_h = 52.0 * s; // taller footer for buy controls
        let content_h = max_rows as f32 * row_h;
        let panel_height =
            (header_h + content_h + footer_h + FRAME_THICKNESS * 2.0 + 4.0).min(sh - 16.0);
        let panel_x = (sw - panel_width) / 2.0;
        let panel_y = (sh - panel_height) / 2.0;

        // Panel frame + corner accents
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        // ===== HEADER =====
        let header_x = panel_x + FRAME_THICKNESS;
        let header_y_pos = panel_y + FRAME_THICKNESS;
        let header_w = panel_width - FRAME_THICKNESS * 2.0;

        draw_rectangle(header_x, header_y_pos, header_w, header_h, PANEL_BG_MID);
        draw_line(
            header_x + 10.0 * s,
            header_y_pos + header_h,
            header_x + header_w - 10.0 * s,
            header_y_pos + header_h,
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
                header_y_pos + header_h - 1.5,
                3.0,
                3.0,
                FRAME_ACCENT,
            );
        }

        // Title: seller's name + stall name
        let title = format!("{}'s \"{}\"", browse.seller_name, browse.stall_name);
        let title_dims = self.measure_text_sharp(&title, 16.0);
        self.draw_text_sharp(
            &title,
            header_x + (header_w - title_dims.width) / 2.0,
            header_y_pos + header_h * 0.65,
            16.0,
            TEXT_TITLE,
        );

        // Close button (X) — in header, right side
        let is_mobile = cfg!(target_os = "android");
        let close_btn_size = if is_mobile { 32.0 * s } else { 28.0 * s };
        let close_btn_x = header_x + header_w - close_btn_size - 6.0 * s;
        let close_btn_y = header_y_pos + (header_h - close_btn_size) / 2.0;
        let close_bounds = Rect::new(close_btn_x, close_btn_y, close_btn_size, close_btn_size);
        layout.add(UiElementId::StallBrowseCloseButton, close_bounds);

        let is_close_hovered =
            state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallBrowseCloseButton);
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

        // ===== CONTENT: Item rows =====
        let content_x = panel_x + FRAME_THICKNESS + 8.0 * s;
        let content_y = header_y_pos + header_h + 4.0 * s;
        let content_w = panel_width - FRAME_THICKNESS * 2.0 - 16.0 * s;
        let footer_y = panel_y + panel_height - FRAME_THICKNESS - footer_h;

        let icon_size = 46.0 * s;
        let icon_pad = 8.0 * s;
        let text_x_offset = icon_pad + icon_size + 8.0 * s;

        if browse.items.is_empty() {
            // Empty state
            let empty_msg = "No items for sale";
            let empty_dims = self.measure_text_sharp(empty_msg, 16.0);
            self.draw_text_sharp(
                empty_msg,
                content_x + (content_w - empty_dims.width) / 2.0,
                content_y + 40.0 * s,
                16.0,
                TEXT_DIM,
            );
        } else {
            for (i, item) in browse.items.iter().enumerate() {
                let y = content_y + i as f32 * row_h;

                // Skip rows that would overflow into footer
                if y + row_h > footer_y {
                    break;
                }

                let bounds = Rect::new(content_x, y, content_w, row_h - 2.0 * s);
                layout.add(UiElementId::StallBrowseItem(i), bounds);

                let is_selected = state.ui_state.stall_browse_selected == i;
                let is_hovered = state.ui_state.hovered_element.as_ref()
                    == Some(&UiElementId::StallBrowseItem(i));

                // Row background
                let row_bg = if is_selected {
                    SLOT_HOVER_BG
                } else if is_hovered {
                    Color::new(0.141, 0.141, 0.188, 1.0)
                } else {
                    Color::new(0.0, 0.0, 0.0, 0.0)
                };

                if is_selected || is_hovered {
                    draw_rectangle(bounds.x, bounds.y, bounds.w, bounds.h, row_bg);
                }

                // Selected row: left accent bar (gold theme for commerce)
                if is_selected {
                    draw_rectangle(content_x, y + 4.0 * s, 3.0, row_h - 10.0 * s, FRAME_ACCENT);
                }

                // Item icon
                let icon_x = content_x + icon_pad;
                let icon_y = y + (row_h - 2.0 * s - icon_size) / 2.0;
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
                let item_def = state.item_registry.get_or_placeholder(&item.item_id);
                let name_color = if is_selected { TEXT_TITLE } else { TEXT_NORMAL };
                self.draw_text_sharp(
                    &item_def.display_name,
                    content_x + text_x_offset,
                    y + row_h * 0.38,
                    16.0,
                    name_color,
                );

                // Quantity: "x[available_qty]" below name
                let qty_text = format!("x{}", item.quantity);
                self.draw_text_sharp(
                    &qty_text,
                    content_x + text_x_offset,
                    y + row_h * 0.72,
                    16.0,
                    TEXT_DIM,
                );

                // Price right-aligned: "[price]g each"
                let price_text = format!("{}g each", item.price);
                let price_dims = self.measure_text_sharp(&price_text, 16.0);
                self.draw_text_sharp(
                    &price_text,
                    content_x + content_w - price_dims.width - 4.0 * s,
                    y + row_h * 0.55,
                    16.0,
                    TEXT_GOLD,
                );

                // Row separator
                draw_line(
                    content_x + 10.0 * s,
                    y + row_h - 2.0 * s,
                    content_x + content_w - 10.0 * s,
                    y + row_h - 2.0 * s,
                    1.0,
                    Color::new(0.15, 0.15, 0.20, 1.0),
                );
            }
        }

        // ===== FOOTER: Buy controls =====
        let footer_w = panel_width - FRAME_THICKNESS * 2.0;

        draw_rectangle(header_x, footer_y, footer_w, footer_h, FOOTER_BG);
        draw_line(
            header_x + 10.0 * s,
            footer_y,
            header_x + footer_w - 10.0 * s,
            footer_y,
            1.0,
            HEADER_BORDER,
        );

        // Selected item info
        let selected_item = browse.items.get(state.ui_state.stall_browse_selected);
        let total_price = selected_item
            .and_then(|item| item.price.checked_mul(state.ui_state.stall_buy_quantity))
            .unwrap_or(i32::MAX);
        let can_afford =
            state.inventory.gold >= total_price && selected_item.is_some() && total_price > 0;

        let ctrl_y = footer_y + (footer_h - 26.0 * s) / 2.0;
        let btn_h = 26.0 * s;
        let qty_btn_size = btn_h;
        let pad_left = header_x + 10.0 * s;

        // [-] button
        let minus_x = pad_left;
        let minus_bounds = Rect::new(minus_x, ctrl_y, qty_btn_size, qty_btn_size);
        layout.add(UiElementId::StallBrowseQuantityMinus, minus_bounds);
        let is_minus_hovered =
            state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallBrowseQuantityMinus);
        let (minus_bg, minus_border) = if is_minus_hovered {
            (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
        } else {
            (SLOT_BG_EMPTY, SLOT_BORDER)
        };
        draw_rectangle(minus_x, ctrl_y, qty_btn_size, qty_btn_size, minus_border);
        draw_rectangle(
            minus_x + 1.0,
            ctrl_y + 1.0,
            qty_btn_size - 2.0,
            qty_btn_size - 2.0,
            minus_bg,
        );
        let minus_dims = self.measure_text_sharp("-", 16.0);
        self.draw_text_sharp(
            "-",
            minus_x + (qty_btn_size - minus_dims.width) / 2.0,
            ctrl_y + qty_btn_size * 0.73,
            16.0,
            if is_minus_hovered {
                TEXT_TITLE
            } else {
                TEXT_NORMAL
            },
        );

        // Quantity display
        let qty_text = format!("{}", state.ui_state.stall_buy_quantity);
        let qty_dims = self.measure_text_sharp(&qty_text, 16.0);
        let qty_display_x = minus_x + qty_btn_size + 4.0 * s;
        let qty_display_w = 28.0 * s;
        self.draw_text_sharp(
            &qty_text,
            qty_display_x + (qty_display_w - qty_dims.width) / 2.0,
            ctrl_y + qty_btn_size * 0.73,
            16.0,
            TEXT_TITLE,
        );

        // [+] button
        let plus_x = qty_display_x + qty_display_w + 4.0 * s;
        let plus_bounds = Rect::new(plus_x, ctrl_y, qty_btn_size, qty_btn_size);
        layout.add(UiElementId::StallBrowseQuantityPlus, plus_bounds);
        let is_plus_hovered =
            state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallBrowseQuantityPlus);
        let (plus_bg, plus_border) = if is_plus_hovered {
            (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
        } else {
            (SLOT_BG_EMPTY, SLOT_BORDER)
        };
        draw_rectangle(plus_x, ctrl_y, qty_btn_size, qty_btn_size, plus_border);
        draw_rectangle(
            plus_x + 1.0,
            ctrl_y + 1.0,
            qty_btn_size - 2.0,
            qty_btn_size - 2.0,
            plus_bg,
        );
        let plus_dims = self.measure_text_sharp("+", 16.0);
        self.draw_text_sharp(
            "+",
            plus_x + (qty_btn_size - plus_dims.width) / 2.0,
            ctrl_y + qty_btn_size * 0.73,
            16.0,
            if is_plus_hovered {
                TEXT_TITLE
            } else {
                TEXT_NORMAL
            },
        );

        // Total price label (center area)
        let total_text = format!("Total: {}g", total_price);
        let total_dims = self.measure_text_sharp(&total_text, 16.0);
        let total_x_center = plus_x + qty_btn_size + 8.0 * s;
        let buy_btn_w = 100.0 * s;
        let buy_btn_x = header_x + footer_w - buy_btn_w - 10.0 * s;
        let total_region_w = buy_btn_x - total_x_center;
        self.draw_text_sharp(
            &total_text,
            total_x_center + (total_region_w - total_dims.width) / 2.0,
            ctrl_y + qty_btn_size * 0.73,
            16.0,
            TEXT_GOLD,
        );

        // [ BUY ] button (right-aligned, green when affordable)
        let buy_btn_h = btn_h;
        let buy_bounds = Rect::new(buy_btn_x, ctrl_y, buy_btn_w, buy_btn_h);
        if can_afford {
            layout.add(UiElementId::StallBrowseBuyButton, buy_bounds);
        }

        let is_buy_hovered = can_afford
            && state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallBrowseBuyButton);
        let (buy_bg, buy_border) = if !can_afford {
            (Color::new(0.12, 0.08, 0.06, 1.0), SLOT_BORDER)
        } else if is_buy_hovered {
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

        draw_rectangle(buy_btn_x, ctrl_y, buy_btn_w, buy_btn_h, buy_border);
        draw_rectangle(
            buy_btn_x + 1.0,
            ctrl_y + 1.0,
            buy_btn_w - 2.0,
            buy_btn_h - 2.0,
            buy_bg,
        );

        // Highlight line on top of buy button when affordable
        if can_afford {
            draw_line(
                buy_btn_x + 2.0,
                ctrl_y + 2.0,
                buy_btn_x + buy_btn_w - 2.0,
                ctrl_y + 2.0,
                1.0,
                Color::new(0.3, 0.7, 0.3, 1.0),
            );
        }

        let buy_text = if can_afford { "[ BUY ]" } else { "Can't Buy" };
        let buy_text_w = self.measure_text_sharp(buy_text, 16.0).width;
        let buy_text_color = if !can_afford {
            Color::new(0.5, 0.3, 0.3, 1.0)
        } else if is_buy_hovered {
            WHITE
        } else {
            Color::new(0.3, 0.7, 0.3, 1.0)
        };
        self.draw_text_sharp(
            buy_text,
            buy_btn_x + (buy_btn_w - buy_text_w) / 2.0,
            ctrl_y + buy_btn_h * 0.69,
            16.0,
            buy_text_color,
        );
    }
}
