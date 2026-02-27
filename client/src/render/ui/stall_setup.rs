//! Player stall setup panel (for the shop owner) — polished medieval fantasy style

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

// Green-themed button colors for Open/Close Shop
const SHOP_BTN_BG: Color = Color::new(0.10, 0.30, 0.15, 1.0);
const SHOP_BTN_BG_HOVER: Color = Color::new(0.14, 0.40, 0.20, 1.0);
const SHOP_BTN_BORDER: Color = Color::new(0.18, 0.50, 0.28, 1.0);
const SHOP_BTN_BORDER_HOVER: Color = Color::new(0.25, 0.60, 0.35, 1.0);
const SHOP_BTN_TEXT: Color = Color::new(0.35, 0.75, 0.45, 1.0);
const SHOP_BTN_TEXT_HOVER: Color = Color::new(0.50, 0.90, 0.55, 1.0);

// Close-shop red variant
const SHOP_CLOSE_BTN_BG: Color = Color::new(0.30, 0.10, 0.10, 1.0);
const SHOP_CLOSE_BTN_BG_HOVER: Color = Color::new(0.40, 0.14, 0.14, 1.0);
const SHOP_CLOSE_BTN_BORDER: Color = Color::new(0.50, 0.18, 0.18, 1.0);
const SHOP_CLOSE_BTN_BORDER_HOVER: Color = Color::new(0.60, 0.25, 0.25, 1.0);
const SHOP_CLOSE_BTN_TEXT: Color = Color::new(0.85, 0.45, 0.45, 1.0);
const SHOP_CLOSE_BTN_TEXT_HOVER: Color = Color::new(0.95, 0.55, 0.55, 1.0);

// Remove button colors
const REMOVE_BTN_BG: Color = Color::new(0.25, 0.10, 0.10, 1.0);
const REMOVE_BTN_BG_HOVER: Color = Color::new(0.40, 0.15, 0.15, 1.0);
const REMOVE_BTN_BORDER: Color = Color::new(0.45, 0.18, 0.18, 1.0);
const REMOVE_BTN_TEXT: Color = Color::new(0.75, 0.35, 0.35, 1.0);
const REMOVE_BTN_TEXT_HOVER: Color = Color::new(0.95, 0.50, 0.50, 1.0);

// Name input area colors
const NAME_INPUT_BG: Color = Color::new(0.086, 0.086, 0.118, 1.0);
const NAME_INPUT_BORDER: Color = Color::new(0.227, 0.212, 0.188, 1.0);

const MAX_STALL_SLOTS: usize = 10;

impl Renderer {
    pub(crate) fn render_stall_setup_panel(&self, state: &GameState, layout: &mut UiLayout) {
        if !state.ui_state.stall_setup_open {
            return;
        }

        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;

        let panel_width = (420.0 * s).min(sw - 16.0);
        // Dynamic height: header + name row + slots + empty hint + footer + padding
        let slot_row_h = 52.0 * s;
        let name_section_h = 40.0 * s;
        let header_h = HEADER_HEIGHT * s;
        let footer_h = FOOTER_HEIGHT * s;
        let slot_count = state.ui_state.stall_my_slots.len().max(1); // at least 1 row for hint
        let slots_area_h = slot_count as f32 * slot_row_h + 8.0 * s;
        // If fewer than max, add hint row
        let hint_row_h = if state.ui_state.stall_my_slots.len() < MAX_STALL_SLOTS {
            slot_row_h
        } else {
            0.0
        };
        let panel_height = (header_h
            + name_section_h
            + slots_area_h
            + hint_row_h
            + footer_h
            + FRAME_THICKNESS * 2.0
            + 16.0 * s)
            .min(sh - 16.0);
        let panel_x = ((sw - panel_width) / 2.0).floor();
        let panel_y = ((sh - panel_height) / 2.0).floor();

        // ===== PANEL FRAME + CORNER ACCENTS =====
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        let inner_x = panel_x + FRAME_THICKNESS;
        let inner_w = panel_width - FRAME_THICKNESS * 2.0;

        // ===== HEADER =====
        let header_y = panel_y + FRAME_THICKNESS;

        draw_rectangle(inner_x, header_y, inner_w, header_h, PANEL_BG_MID);
        draw_line(
            inner_x + 10.0 * s,
            header_y + header_h,
            inner_x + inner_w - 10.0 * s,
            header_y + header_h,
            2.0,
            HEADER_BORDER,
        );

        // Decorative dots along header border
        let dot_spacing = 60.0 * s;
        let num_dots = ((inner_w - 40.0 * s) / dot_spacing) as i32;
        let start_dot_x = inner_x + 20.0 * s;
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
        let title = "YOUR SHOP";
        let title_dims = self.measure_text_sharp(title, 16.0);
        self.draw_text_sharp(
            title,
            inner_x + (inner_w - title_dims.width) / 2.0,
            header_y + header_h * 0.65,
            16.0,
            TEXT_TITLE,
        );

        // Close button (X)
        let is_mobile = cfg!(target_os = "android");
        let close_btn_size = if is_mobile { 32.0 * s } else { 28.0 * s };
        let close_btn_x = inner_x + inner_w - close_btn_size - 6.0 * s;
        let close_btn_y = header_y + (header_h - close_btn_size) / 2.0;
        let close_bounds = Rect::new(close_btn_x, close_btn_y, close_btn_size, close_btn_size);
        layout.add(UiElementId::StallSetupCloseButton, close_bounds);

        let is_close_hovered =
            state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallSetupCloseButton);
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

        // ===== SHOP NAME SECTION =====
        let name_y = header_y + header_h + 4.0 * s;
        let name_pad = 10.0 * s;

        // "Shop Name:" label
        let label = "Shop Name:";
        let label_dims = self.measure_text_sharp(label, 16.0);
        self.draw_text_sharp(
            label,
            inner_x + name_pad,
            name_y + name_section_h * 0.62,
            16.0,
            TEXT_DIM,
        );

        // Name input area
        let input_x = inner_x + name_pad + label_dims.width + 8.0 * s;
        let input_w = inner_w - name_pad * 2.0 - label_dims.width - 8.0 * s;
        let input_h = 26.0 * s;
        let input_y = name_y + (name_section_h - input_h) / 2.0;

        let input_bounds = Rect::new(input_x, input_y, input_w, input_h);
        layout.add(UiElementId::StallSetupNameInput, input_bounds);

        let is_name_hovered =
            state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallSetupNameInput);
        let is_name_editing = state.ui_state.stall_name_editing;
        let input_border_color = if is_name_editing {
            FRAME_ACCENT
        } else if is_name_hovered {
            SLOT_HOVER_BORDER
        } else {
            NAME_INPUT_BORDER
        };

        draw_rectangle(input_x, input_y, input_w, input_h, input_border_color);
        draw_rectangle(
            input_x + 1.0,
            input_y + 1.0,
            input_w - 2.0,
            input_h - 2.0,
            NAME_INPUT_BG,
        );

        // Display current name or placeholder
        let text_x = input_x + 6.0 * s;
        let text_y = input_y + input_h * 0.70;
        let (name_display, name_color) =
            if state.ui_state.stall_my_name.is_empty() && !is_name_editing {
                ("My Shop".to_string(), TEXT_DIM)
            } else {
                (state.ui_state.stall_my_name.clone(), TEXT_NORMAL)
            };
        self.draw_text_sharp(&name_display, text_x, text_y, 16.0, name_color);

        // Blinking cursor when editing
        if is_name_editing {
            let cursor_visible = (macroquad::time::get_time() * 2.0) as i32 % 2 == 0;
            if cursor_visible {
                let text_before: String = state
                    .ui_state
                    .stall_my_name
                    .chars()
                    .take(state.ui_state.stall_name_cursor)
                    .collect();
                let cursor_x = text_x + self.measure_text_sharp(&text_before, 16.0).width;
                draw_rectangle(cursor_x, input_y + 4.0, 2.0, input_h - 8.0, TEXT_NORMAL);
            }
        }

        // Separator line under name section
        draw_line(
            inner_x + 10.0 * s,
            name_y + name_section_h,
            inner_x + inner_w - 10.0 * s,
            name_y + name_section_h,
            1.0,
            SLOT_BORDER,
        );

        // ===== SLOT ROWS =====
        let slots_y = name_y + name_section_h + 4.0 * s;
        let slot_pad = 8.0 * s;
        let row_w = inner_w - slot_pad * 2.0;
        let icon_size = 46.0 * s;

        for (i, slot) in state.ui_state.stall_my_slots.iter().enumerate() {
            let row_y = slots_y + i as f32 * slot_row_h;

            let row_bounds = Rect::new(inner_x + slot_pad, row_y, row_w, slot_row_h - 2.0 * s);
            layout.add(UiElementId::StallSetupSlot(i), row_bounds);

            let is_hovered =
                state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallSetupSlot(i));

            // Row background
            let row_bg = if is_hovered {
                SLOT_HOVER_BG
            } else {
                SLOT_BG_FILLED
            };
            let row_border = if is_hovered {
                SLOT_HOVER_BORDER
            } else {
                SLOT_BORDER
            };

            draw_rectangle(
                row_bounds.x,
                row_bounds.y,
                row_bounds.w,
                row_bounds.h,
                row_border,
            );
            draw_rectangle(
                row_bounds.x + 1.0,
                row_bounds.y + 1.0,
                row_bounds.w - 2.0,
                row_bounds.h - 2.0,
                row_bg,
            );

            // Inner shadow top edge
            draw_line(
                row_bounds.x + 2.0,
                row_bounds.y + 2.0,
                row_bounds.x + row_bounds.w - 2.0,
                row_bounds.y + 2.0,
                1.0,
                SLOT_INNER_SHADOW,
            );

            // Item icon
            let icon_x = row_bounds.x + 4.0 * s;
            let icon_y = row_bounds.y + (row_bounds.h - icon_size) / 2.0;
            self.draw_item_icon(
                &slot.item_id,
                icon_x,
                icon_y,
                icon_size,
                icon_size,
                state,
                false,
            );

            // Item name
            let item_def = state.item_registry.get_or_placeholder(&slot.item_id);
            let name_x = icon_x + icon_size + 6.0 * s;
            let text_y = row_bounds.y + row_bounds.h * 0.45;
            self.draw_text_sharp(&item_def.display_name, name_x, text_y, 16.0, TEXT_NORMAL);

            // Quantity
            let qty_text = format!("x{}", slot.quantity);
            let qty_dims = self.measure_text_sharp(&qty_text, 16.0);
            let qty_x =
                name_x + self.measure_text_sharp(&item_def.display_name, 16.0).width + 6.0 * s;
            self.draw_text_sharp(&qty_text, qty_x, text_y, 16.0, TEXT_DIM);

            // Price
            let price_text = format!("@ {}g", slot.price);
            let price_x = qty_x + qty_dims.width + 8.0 * s;
            self.draw_text_sharp(&price_text, price_x, text_y, 16.0, TEXT_GOLD);

            // Remove button
            let remove_w = 64.0 * s;
            let remove_h = 22.0 * s;
            let remove_x = row_bounds.x + row_bounds.w - remove_w - 4.0 * s;
            let remove_y = row_bounds.y + (row_bounds.h - remove_h) / 2.0;
            let remove_bounds = Rect::new(remove_x, remove_y, remove_w, remove_h);
            layout.add(UiElementId::StallSetupRemove(i), remove_bounds);

            let is_remove_hovered =
                state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallSetupRemove(i));

            let (rm_bg, rm_border, rm_text_color) = if is_remove_hovered {
                (
                    REMOVE_BTN_BG_HOVER,
                    REMOVE_BTN_BORDER,
                    REMOVE_BTN_TEXT_HOVER,
                )
            } else {
                (REMOVE_BTN_BG, REMOVE_BTN_BORDER, REMOVE_BTN_TEXT)
            };

            draw_rectangle(remove_x, remove_y, remove_w, remove_h, rm_border);
            draw_rectangle(
                remove_x + 1.0,
                remove_y + 1.0,
                remove_w - 2.0,
                remove_h - 2.0,
                rm_bg,
            );

            let rm_label = "Remove";
            let rm_dims = self.measure_text_sharp(rm_label, 16.0);
            self.draw_text_sharp(
                rm_label,
                remove_x + (remove_w - rm_dims.width) / 2.0,
                remove_y + remove_h * 0.70,
                16.0,
                rm_text_color,
            );
        }

        // Empty slot hint
        if state.ui_state.stall_my_slots.len() < MAX_STALL_SLOTS {
            let hint_y = slots_y + state.ui_state.stall_my_slots.len() as f32 * slot_row_h;
            let hint_bounds = Rect::new(inner_x + slot_pad, hint_y, row_w, slot_row_h - 2.0 * s);

            // Dashed-border empty slot feel
            draw_rectangle(
                hint_bounds.x,
                hint_bounds.y,
                hint_bounds.w,
                hint_bounds.h,
                SLOT_BG_EMPTY,
            );
            draw_rectangle_lines(
                hint_bounds.x,
                hint_bounds.y,
                hint_bounds.w,
                hint_bounds.h,
                1.0,
                SLOT_BORDER,
            );

            let hint = "Click inventory to add";
            let hint_dims = self.measure_text_sharp(hint, 16.0);
            self.draw_text_sharp(
                hint,
                hint_bounds.x + (hint_bounds.w - hint_dims.width) / 2.0,
                hint_bounds.y + hint_bounds.h * 0.62,
                16.0,
                TEXT_DIM,
            );
        }

        // ===== FOOTER =====
        let footer_y = panel_y + panel_height - FRAME_THICKNESS - footer_h;
        let footer_x = inner_x;
        let footer_w = inner_w;

        draw_rectangle(footer_x, footer_y, footer_w, footer_h, FOOTER_BG);
        draw_line(
            footer_x + 10.0 * s,
            footer_y,
            footer_x + footer_w - 10.0 * s,
            footer_y,
            1.0,
            HEADER_BORDER,
        );

        // Left side: "[Esc] Close"
        self.draw_text_sharp(
            "[Esc] Close",
            footer_x + 10.0 * s,
            footer_y + footer_h * 0.67,
            16.0,
            TEXT_DIM,
        );

        // Right side: Open/Close Shop button
        let shop_btn_w = 130.0 * s;
        let shop_btn_h = 24.0 * s;
        let shop_btn_x = footer_x + footer_w - shop_btn_w - 10.0 * s;
        let shop_btn_y = footer_y + (footer_h - shop_btn_h) / 2.0;

        if state.ui_state.stall_active {
            // Close Shop button (red-themed)
            let btn_bounds = Rect::new(shop_btn_x, shop_btn_y, shop_btn_w, shop_btn_h);
            layout.add(UiElementId::StallSetupOpenButton, btn_bounds);

            let is_btn_hovered =
                state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallSetupOpenButton);

            let (bg, border, text_color) = if is_btn_hovered {
                (
                    SHOP_CLOSE_BTN_BG_HOVER,
                    SHOP_CLOSE_BTN_BORDER_HOVER,
                    SHOP_CLOSE_BTN_TEXT_HOVER,
                )
            } else {
                (
                    SHOP_CLOSE_BTN_BG,
                    SHOP_CLOSE_BTN_BORDER,
                    SHOP_CLOSE_BTN_TEXT,
                )
            };

            draw_rectangle(shop_btn_x, shop_btn_y, shop_btn_w, shop_btn_h, border);
            draw_rectangle(
                shop_btn_x + 1.0,
                shop_btn_y + 1.0,
                shop_btn_w - 2.0,
                shop_btn_h - 2.0,
                bg,
            );

            // Highlight line on top edge
            if is_btn_hovered {
                draw_line(
                    shop_btn_x + 2.0,
                    shop_btn_y + 2.0,
                    shop_btn_x + shop_btn_w - 2.0,
                    shop_btn_y + 2.0,
                    1.0,
                    SHOP_CLOSE_BTN_BORDER_HOVER,
                );
            }

            let label = "[ CLOSE SHOP ]";
            let label_dims = self.measure_text_sharp(label, 16.0);
            self.draw_text_sharp(
                label,
                shop_btn_x + (shop_btn_w - label_dims.width) / 2.0,
                shop_btn_y + shop_btn_h * 0.70,
                16.0,
                text_color,
            );
        } else {
            // Open Shop button (green-themed)
            let btn_bounds = Rect::new(shop_btn_x, shop_btn_y, shop_btn_w, shop_btn_h);
            layout.add(UiElementId::StallSetupOpenButton, btn_bounds);

            let is_btn_hovered =
                state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallSetupOpenButton);

            let (bg, border, text_color) = if is_btn_hovered {
                (
                    SHOP_BTN_BG_HOVER,
                    SHOP_BTN_BORDER_HOVER,
                    SHOP_BTN_TEXT_HOVER,
                )
            } else {
                (SHOP_BTN_BG, SHOP_BTN_BORDER, SHOP_BTN_TEXT)
            };

            draw_rectangle(shop_btn_x, shop_btn_y, shop_btn_w, shop_btn_h, border);
            draw_rectangle(
                shop_btn_x + 1.0,
                shop_btn_y + 1.0,
                shop_btn_w - 2.0,
                shop_btn_h - 2.0,
                bg,
            );

            // Highlight line on top edge
            if is_btn_hovered {
                draw_line(
                    shop_btn_x + 2.0,
                    shop_btn_y + 2.0,
                    shop_btn_x + shop_btn_w - 2.0,
                    shop_btn_y + 2.0,
                    1.0,
                    SHOP_BTN_BORDER_HOVER,
                );
            }

            let label = "[ OPEN SHOP ]";
            let label_dims = self.measure_text_sharp(label, 16.0);
            self.draw_text_sharp(
                label,
                shop_btn_x + (shop_btn_w - label_dims.width) / 2.0,
                shop_btn_y + shop_btn_h * 0.70,
                16.0,
                text_color,
            );
        }
    }
}
