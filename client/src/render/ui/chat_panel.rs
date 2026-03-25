//! Mobile chat panel rendering - fullscreen overlay with tabs

use super::super::Renderer;
use super::common::*;
use crate::game::{ChatChannel, GameState};
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;
use macroquad::window::get_internal_gl;

impl Renderer {
    /// Render the fullscreen chat panel overlay
    pub(crate) fn render_chat_panel(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        if !state.ui_state.chat_panel_open {
            return;
        }

        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;

        // Semi-transparent overlay (blocks game interaction)
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.6));
        layout.add(
            UiElementId::ChatPanelBackground,
            macroquad::prelude::Rect::new(0.0, 0.0, sw, sh),
        );

        // Panel dimensions — fullscreen on Android, small margin on desktop
        let margin = if cfg!(target_os = "android") { 0.0 } else { 10.0 };
        let panel_x = margin;
        let panel_y = margin;
        let panel_w = sw - margin * 2.0;
        let panel_h = sh - margin * 2.0;

        // Panel frame
        self.draw_panel_frame(panel_x, panel_y, panel_w, panel_h);

        // === CLOSE BUTTON (top-right corner) ===
        let close_size = 32.0;
        let close_x = panel_x + panel_w - close_size - FRAME_THICKNESS - 4.0;
        let close_y = panel_y + FRAME_THICKNESS + 4.0;

        let close_bounds = macroquad::prelude::Rect::new(close_x, close_y, close_size, close_size);
        layout.add(UiElementId::ChatCloseButton, close_bounds);

        let is_close_hovered = hovered.as_ref() == Some(&UiElementId::ChatCloseButton);
        let (btn_bg, btn_border) = if is_close_hovered {
            (
                Color::new(0.4, 0.15, 0.15, 1.0),
                Color::new(0.6, 0.2, 0.2, 1.0),
            )
        } else {
            (Color::new(0.2, 0.1, 0.1, 1.0), FRAME_MID)
        };
        draw_rectangle(close_x, close_y, close_size, close_size, btn_bg);
        draw_rectangle_lines(close_x, close_y, close_size, close_size, 1.0, btn_border);

        // X icon
        let x_margin = 8.0;
        let x_color = if is_close_hovered {
            Color::new(1.0, 0.4, 0.4, 1.0)
        } else {
            TEXT_DIM
        };
        draw_line(
            close_x + x_margin,
            close_y + x_margin,
            close_x + close_size - x_margin,
            close_y + close_size - x_margin,
            2.0,
            x_color,
        );
        draw_line(
            close_x + close_size - x_margin,
            close_y + x_margin,
            close_x + x_margin,
            close_y + close_size - x_margin,
            2.0,
            x_color,
        );

        // === TAB BAR ===
        let tab_y = panel_y + FRAME_THICKNESS;
        let tab_w = (panel_w - FRAME_THICKNESS * 2.0 - close_size - 8.0) / 3.0;
        let tab_h = TAB_HEIGHT * s;
        let tab_x_start = panel_x + FRAME_THICKNESS;

        let tabs = [
            (UiElementId::ChatTabLocal, "Public", ChatChannel::Local),
            (UiElementId::ChatTabGlobal, "Global", ChatChannel::Global),
            (UiElementId::ChatTabSystem, "System", ChatChannel::System),
        ];
        let latest_local_ts = state
            .ui_state
            .chat_messages
            .latest_timestamp(&ChatChannel::Local);
        let latest_global_ts = state
            .ui_state
            .chat_messages
            .latest_timestamp(&ChatChannel::Global);
        let latest_system_ts = state
            .ui_state
            .chat_messages
            .latest_timestamp(&ChatChannel::System);

        for (i, (id, label, channel)) in tabs.iter().enumerate() {
            let tx = tab_x_start + i as f32 * tab_w;
            let is_active = std::mem::discriminant(&state.ui_state.chat_active_tab)
                == std::mem::discriminant(channel);
            let is_hovered = hovered.as_ref() == Some(id);
            let has_unread = match channel {
                ChatChannel::Local => latest_local_ts > state.ui_state.chat_last_seen_local,
                ChatChannel::Global => latest_global_ts > state.ui_state.chat_last_seen_global,
                ChatChannel::System => latest_system_ts > state.ui_state.chat_last_seen_system,
            };

            let bg = if is_active {
                HEADER_BG
            } else if is_hovered {
                SLOT_HOVER_BG
            } else {
                PANEL_BG_DARK
            };

            draw_rectangle(tx, tab_y, tab_w, tab_h, bg);
            draw_rectangle_lines(tx, tab_y, tab_w, tab_h, 1.0, HEADER_BORDER);

            if is_active {
                // Gold underline for active tab
                draw_rectangle(
                    tx + 2.0,
                    tab_y + tab_h - 2.0,
                    tab_w - 4.0,
                    2.0,
                    FRAME_ACCENT,
                );
            }

            let text_w = self.measure_text_sharp(label, TAB_FONT_SIZE).width;
            self.draw_text_sharp(
                label,
                (tx + (tab_w - text_w) / 2.0).floor(),
                (tab_y + tab_h * 0.68).floor(),
                TAB_FONT_SIZE,
                if is_active {
                    TEXT_TITLE
                } else if has_unread {
                    Color::new(0.92, 0.92, 0.92, 1.0)
                } else {
                    TEXT_DIM
                },
            );

            layout.add(
                id.clone(),
                macroquad::prelude::Rect::new(tx, tab_y, tab_w, tab_h),
            );
        }

        // === MESSAGE LIST ===
        let messages_y = tab_y + tab_h + 4.0 * s;
        let input_bar_h = 48.0 * s;
        let messages_h = panel_y + panel_h - FRAME_THICKNESS - input_bar_h - 4.0 * s - messages_y;
        let messages_x = panel_x + FRAME_THICKNESS + 8.0;
        let messages_w = panel_w - FRAME_THICKNESS * 2.0 - 16.0;

        // Message area background
        draw_rectangle(
            panel_x + FRAME_THICKNESS,
            messages_y,
            panel_w - FRAME_THICKNESS * 2.0,
            messages_h,
            PANEL_BG_DARK,
        );

        // Register message area for scroll input
        layout.add(
            UiElementId::ChatMessageArea,
            macroquad::prelude::Rect::new(
                panel_x + FRAME_THICKNESS,
                messages_y,
                panel_w - FRAME_THICKNESS * 2.0,
                messages_h,
            ),
        );

        // Filter and render messages
        let font_size = 16.0;
        let line_height = 20.0 * s;
        let max_lines = (messages_h / line_height) as usize;

        let filtered: Vec<_> = state
            .ui_state
            .chat_messages
            .channel(&state.ui_state.chat_active_tab)
            .iter()
            .collect();

        // Build all wrapped lines with their colors
        let mut all_lines: Vec<(String, Color)> = Vec::new();
        for msg in filtered.iter() {
            let (color, text) = match msg.channel {
                ChatChannel::Local => (WHITE, format!("{}: {}", msg.sender_name, msg.text)),
                ChatChannel::Global => (SKYBLUE, format!("[G] {}: {}", msg.sender_name, msg.text)),
                ChatChannel::System => (
                    Color::from_rgba(255, 220, 100, 255),
                    format!("{} {}", msg.sender_name, msg.text),
                ),
            };
            let wrapped = self.wrap_text(&text, messages_w, font_size);
            for line in wrapped {
                all_lines.push((line, color));
            }
        }

        // Apply smooth pixel-based scroll offset
        let total_lines = all_lines.len();
        let total_content_height = total_lines as f32 * line_height;
        let max_scroll_px = (total_content_height - messages_h).max(0.0);
        let scroll_px = state.ui_state.chat_message_scroll.min(max_scroll_px);
        layout.set_max_scroll(UiElementId::ChatPanelScrollbar, max_scroll_px);

        // Calculate which lines are visible and the sub-pixel offset
        let scroll_lines = scroll_px / line_height;
        let fractional_offset = (scroll_lines.fract()) * line_height;
        let scroll_lines_int = scroll_lines.floor() as usize;

        // We need one extra line for smooth scrolling (partially visible at top/bottom)
        let visible_lines = max_lines + 1;
        let end = total_lines.saturating_sub(scroll_lines_int);
        let start = end.saturating_sub(visible_lines);

        // Scissor clip to message area so text doesn't overflow into tabs/input
        let physical_w = macroquad::window::screen_width();
        let physical_h = macroquad::window::screen_height();
        let scale_x = physical_w / sw;
        let scale_y = physical_h / sh;
        {
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            gl.quad_gl.scissor(Some((
                ((panel_x + FRAME_THICKNESS) * scale_x) as i32,
                (messages_y * scale_y) as i32,
                ((panel_w - FRAME_THICKNESS * 2.0) * scale_x) as i32,
                (messages_h * scale_y) as i32,
            )));
        }

        let mut y = messages_y + messages_h - line_height + fractional_offset;
        for i in (start..end).rev() {
            if y >= messages_y - line_height && y <= messages_y + messages_h {
                let (ref line, color) = all_lines[i];
                self.draw_text_sharp(line, messages_x, y, font_size, color);
            }
            y -= line_height;
        }

        // Draw scroll indicator if there are messages above
        if start > 0 {
            let indicator = format!("▲ {} more lines", start);
            let ind_w = self.measure_text_sharp(&indicator, 14.0).width;
            self.draw_text_sharp(
                &indicator,
                messages_x + (messages_w - ind_w) / 2.0,
                messages_y + 14.0,
                14.0,
                TEXT_DIM,
            );
        }

        // Disable scissor clipping
        {
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            gl.quad_gl.scissor(None);
        }

        // Draw scrollbar
        if max_scroll_px > 0.0 {
            let scrollbar_w: f32 = if cfg!(target_os = "android") {
                12.0
            } else {
                8.0
            };
            let track_x = messages_x + messages_w - scrollbar_w;
            let track_y = messages_y;
            let track_h = messages_h;

            layout.add_scrollbar(
                UiElementId::ChatPanelScrollbar,
                Rect::new(track_x, track_y, scrollbar_w, track_h),
            );

            // Track
            draw_rectangle(
                track_x,
                track_y,
                scrollbar_w,
                track_h,
                Color::new(0.1, 0.09, 0.12, 0.6),
            );

            // Thumb (inverted: bottom = scroll 0, top = max scroll)
            let visible_ratio = (messages_h / total_content_height).min(1.0);
            let thumb_h = (track_h * visible_ratio).max(16.0);
            let scroll_ratio = if max_scroll_px > 0.0 {
                scroll_px / max_scroll_px
            } else {
                0.0
            };
            let thumb_y = track_y + (track_h - thumb_h) * (1.0 - scroll_ratio);

            let is_dragging = state.ui_state.chat_scroll_drag.dragging;
            let thumb_color = if is_dragging {
                Color::new(0.5, 0.45, 0.55, 0.9)
            } else {
                Color::new(0.35, 0.32, 0.40, 0.7)
            };
            draw_rectangle(
                track_x + 1.0,
                thumb_y,
                scrollbar_w - 2.0,
                thumb_h,
                thumb_color,
            );
        }

        // === INPUT BAR ===
        {
            let input_y = panel_y + panel_h - FRAME_THICKNESS - input_bar_h;
            let send_btn_w = 60.0 * s;
            let input_w = panel_w - FRAME_THICKNESS * 2.0 - send_btn_w - 12.0;
            let input_x = panel_x + FRAME_THICKNESS + 4.0;

            // Input field background
            draw_rectangle(input_x, input_y, input_w, input_bar_h, SLOT_BG_EMPTY);
            draw_rectangle_lines(input_x, input_y, input_w, input_bar_h, 1.0, SLOT_BORDER);

            // Input text
            let text_y = input_y + input_bar_h * 0.6;
            let display_text = if state.ui_state.chat_input.is_empty() {
                "Tap to chat..."
            } else {
                &state.ui_state.chat_input
            };
            let text_color = if state.ui_state.chat_input.is_empty() {
                TEXT_DIM
            } else {
                TEXT_NORMAL
            };
            self.draw_text_sharp(display_text, input_x + 8.0, text_y, font_size, text_color);

            layout.add(
                UiElementId::ChatInputField,
                macroquad::prelude::Rect::new(input_x, input_y, input_w, input_bar_h),
            );

            // Send button
            let send_x = input_x + input_w + 8.0;
            let is_send_hovered = hovered.as_ref() == Some(&UiElementId::ChatSendButton);
            let send_bg = if is_send_hovered {
                SLOT_HOVER_BG
            } else {
                HEADER_BG
            };
            draw_rectangle(send_x, input_y, send_btn_w, input_bar_h, send_bg);
            draw_rectangle_lines(send_x, input_y, send_btn_w, input_bar_h, 1.0, FRAME_MID);

            let send_label = "Send";
            let send_w = self.measure_text_sharp(send_label, font_size).width;
            self.draw_text_sharp(
                send_label,
                (send_x + (send_btn_w - send_w) / 2.0).floor(),
                text_y,
                font_size,
                TEXT_TITLE,
            );

            layout.add(
                UiElementId::ChatSendButton,
                macroquad::prelude::Rect::new(send_x, input_y, send_btn_w, input_bar_h),
            );
        }
    }
}
