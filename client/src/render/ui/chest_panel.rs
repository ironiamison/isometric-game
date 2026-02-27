//! Chest panel UI - scrollable list of chest contents with take/deposit actions

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

/// Height of each item row in the chest list
const ROW_HEIGHT: f32 = 36.0;
/// Spacing between rows
const ROW_SPACING: f32 = 2.0;

impl Renderer {
    pub(crate) fn render_chest_panel(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;

        let panel_width = (280.0 * s).min(sw - 16.0);
        let panel_height = (350.0 * s).min(sh - 16.0);
        let panel_x = (sw - panel_width) / 2.0;
        let panel_y = (sh - panel_height) / 2.0;

        // Panel frame
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        let inner_x = panel_x + FRAME_THICKNESS;
        let inner_w = panel_width - FRAME_THICKNESS * 2.0;
        let padding = 10.0 * s;

        // ===== HEADER =====
        let header_h = HEADER_HEIGHT * s;
        let header_y = panel_y + FRAME_THICKNESS;

        draw_rectangle(inner_x, header_y, inner_w, header_h, HEADER_BG);
        draw_line(
            inner_x + 10.0 * s,
            header_y + header_h,
            inner_x + inner_w - 10.0 * s,
            header_y + header_h,
            2.0,
            HEADER_BORDER,
        );

        // Title
        let title = &state.ui_state.chest_name.to_uppercase();
        let title_dims = self.measure_text_sharp(title, 16.0);
        self.draw_text_sharp(
            title,
            inner_x + (inner_w - title_dims.width) / 2.0,
            header_y + header_h * 0.71,
            16.0,
            TEXT_TITLE,
        );

        // Close button (X)
        let is_mobile = cfg!(target_os = "android");
        let close_btn_size = if is_mobile { 32.0 * s } else { 28.0 * s };
        let close_btn_x = inner_x + inner_w - close_btn_size - 6.0 * s;
        let close_btn_y = header_y + (header_h - close_btn_size) / 2.0;
        let close_bounds = Rect::new(close_btn_x, close_btn_y, close_btn_size, close_btn_size);
        layout.add(UiElementId::ChestClose, close_bounds);

        let is_close_hovered = matches!(hovered, Some(UiElementId::ChestClose));
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

        // ===== FOOTER (total value) =====
        let footer_h = FOOTER_HEIGHT * s;
        let footer_y = panel_y + panel_height - FRAME_THICKNESS - footer_h;

        draw_rectangle(inner_x, footer_y, inner_w, footer_h, FOOTER_BG);
        draw_line(
            inner_x + 10.0 * s,
            footer_y,
            inner_x + inner_w - 10.0 * s,
            footer_y,
            1.0,
            HEADER_BORDER,
        );

        let total_text = format!("Total Value: {}g", state.ui_state.chest_total_value);
        let total_dims = self.measure_text_sharp(&total_text, 16.0);
        self.draw_text_sharp(
            &total_text,
            inner_x + (inner_w - total_dims.width) / 2.0,
            footer_y + footer_h * 0.68,
            16.0,
            TEXT_GOLD,
        );

        // ===== CONTENT AREA (scrollable list) =====
        let content_y = header_y + header_h + 4.0 * s;
        let content_h = footer_y - content_y - 4.0 * s;
        let content_x = inner_x + padding;
        let content_w = inner_w - padding * 2.0;

        // Register scroll area
        let scroll_rect = Rect::new(content_x, content_y, content_w, content_h);
        layout.add(UiElementId::ChestScrollArea, scroll_rect);

        // Content background
        draw_rectangle(
            content_x,
            content_y,
            content_w,
            content_h,
            Color::new(0.06, 0.06, 0.08, 1.0),
        );
        draw_rectangle_lines(content_x, content_y, content_w, content_h, 1.0, SLOT_BORDER);

        // Calculate total content height for scroll clamping
        let row_h = ROW_HEIGHT * s;
        let row_sp = ROW_SPACING * s;
        let total_rows = state.ui_state.chest_slots.len();
        let total_content_height = total_rows as f32 * (row_h + row_sp);
        let max_scroll = (total_content_height - content_h).max(0.0);
        layout.set_max_scroll(UiElementId::ChestScrollArea, max_scroll);

        let scroll_offset = state.ui_state.chest_scroll;

        // Scissor clip for scroll area
        let (real_sw, real_sh) = (screen_width(), screen_height());
        let scale_x = real_sw / sw;
        let scale_y = real_sh / sh;
        let clip_x = (content_x * scale_x) as i32;
        let clip_y = (content_y * scale_y) as i32;
        let clip_w = (content_w * scale_x) as i32;
        let clip_h = (content_h * scale_y) as i32;

        unsafe {
            miniquad::gl::glEnable(miniquad::gl::GL_SCISSOR_TEST);
            miniquad::gl::glScissor(clip_x, real_sh as i32 - clip_y - clip_h, clip_w, clip_h);
        }

        // Render each slot row
        for (idx, slot) in state.ui_state.chest_slots.iter().enumerate() {
            let item_y = content_y + 2.0 * s + idx as f32 * (row_h + row_sp) - scroll_offset;

            // Skip items outside visible area
            if item_y + row_h < content_y || item_y > content_y + content_h {
                continue;
            }

            // Register clickable row
            let row_bounds = Rect::new(content_x + 2.0, item_y, content_w - 4.0, row_h);
            layout.add(UiElementId::ChestSlot(idx as u8), row_bounds);

            let is_hovered = matches!(hovered, Some(UiElementId::ChestSlot(i)) if *i == idx as u8);

            // Row background with alternating shade and hover highlight
            let row_bg = if is_hovered {
                SLOT_HOVER_BG
            } else if idx % 2 == 0 {
                Color::new(0.08, 0.08, 0.10, 0.6)
            } else {
                Color::new(0.06, 0.06, 0.08, 0.6)
            };
            draw_rectangle(content_x + 2.0, item_y, content_w - 4.0, row_h, row_bg);

            if is_hovered {
                draw_rectangle_lines(
                    content_x + 2.0,
                    item_y,
                    content_w - 4.0,
                    row_h,
                    1.0,
                    SLOT_HOVER_BORDER,
                );
            }

            if let Some((item_id, quantity, value)) = slot {
                let display_name = state.item_registry.get_display_name(item_id);

                // Draw item icon if available
                let text_start_x;
                let sprite_key = state.item_registry.get_sprite_key(item_id);
                if let Some((texture, source_rect)) = self.item_sprites.get(sprite_key) {
                    let icon_size = 28.0 * s;
                    let icon_x = content_x + 6.0 * s;
                    let icon_y = item_y + (row_h - icon_size) / 2.0;

                    let (icon_width, icon_height) = if let Some(r) = source_rect {
                        (r.w, r.h)
                    } else {
                        (texture.width(), texture.height())
                    };
                    let scale = (icon_size / icon_width).min(icon_size / icon_height);
                    let draw_w = icon_width * scale;
                    let draw_h = icon_height * scale;

                    draw_texture_ex(
                        texture,
                        icon_x + (icon_size - draw_w) / 2.0,
                        icon_y + (icon_size - draw_h) / 2.0,
                        WHITE,
                        DrawTextureParams {
                            source: source_rect,
                            dest_size: Some(Vec2::new(draw_w, draw_h)),
                            ..Default::default()
                        },
                    );
                    text_start_x = content_x + 6.0 * s + icon_size + 4.0 * s;
                } else {
                    text_start_x = content_x + 8.0 * s;
                }

                // Item name (left-aligned)
                let name_color = if is_hovered { TEXT_TITLE } else { TEXT_NORMAL };
                self.draw_text_sharp(
                    display_name,
                    text_start_x,
                    item_y + row_h * 0.6,
                    16.0,
                    name_color,
                );

                // Value (right-aligned)
                let value_text = format!("{}g", value);
                let value_dims = self.measure_text_sharp(&value_text, 16.0);
                self.draw_text_sharp(
                    &value_text,
                    content_x + content_w - value_dims.width - 8.0 * s,
                    item_y + row_h * 0.6,
                    16.0,
                    TEXT_GOLD,
                );

                // Quantity (before value)
                let qty_text = format!("x{}", quantity);
                let qty_dims = self.measure_text_sharp(&qty_text, 16.0);
                self.draw_text_sharp(
                    &qty_text,
                    content_x + content_w - value_dims.width - qty_dims.width - 16.0 * s,
                    item_y + row_h * 0.6,
                    16.0,
                    TEXT_DIM,
                );
            } else {
                // Empty slot indicator
                self.draw_text_sharp(
                    "- Empty -",
                    content_x + 8.0 * s,
                    item_y + row_h * 0.6,
                    16.0,
                    Color::new(0.3, 0.3, 0.35, 0.6),
                );
            }
        }

        // Disable scissor test
        unsafe {
            miniquad::gl::glDisable(miniquad::gl::GL_SCISSOR_TEST);
        }
    }
}
