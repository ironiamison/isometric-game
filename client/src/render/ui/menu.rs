//! Settings panel rendering (bottom-right aligned, like inventory/skills/character)

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

impl Renderer {
    /// Render the settings panel (bottom-right, above menu buttons)
    pub(crate) fn render_escape_menu(&self, state: &GameState, layout: &mut UiLayout) {
        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;

        // Panel sizing - compute height from content
        let frame_thickness = FRAME_THICKNESS * s;
        let menu_width = 240.0 * s;

        // Content height varies by platform:
        //   header(24) + pad(8) + zoom(30) + disconnect(28) + gap(8) + pad(8)
        //   + toggle rows: mute(26) + chat(26) + tapwalk(26) = 78
        //   Desktop: sliders(22+22+26=70) + control(26) + gfx(26) = 122
        //   Android: sliders(22) + joystick(26) = 48
        #[cfg(not(target_os = "android"))]
        let content_height = 306.0 * s;
        #[cfg(target_os = "android")]
        let content_height = 306.0 * s;

        // Position at bottom-right, above menu buttons (matching other panels)
        let _button_size = MENU_BUTTON_SIZE * s;
        let _exp_bar_gap = EXP_BAR_GAP * s;
        let button_area_height = bottom_ui_height(s);
        // Match quest panel sizing: 314 * s clamped to available space
        let min_panel_y = 4.0;
        let max_available_height = sh - button_area_height - 8.0 - min_panel_y;
        let menu_height = (frame_thickness * 2.0 + content_height).min(max_available_height);
        let menu_x = sw - menu_width - 8.0;
        let menu_y = sh - button_area_height - menu_height - 8.0;

        // ===== PANEL FRAME =====
        self.draw_panel_frame(menu_x, menu_y, menu_width, menu_height);
        self.draw_corner_accents(menu_x, menu_y, menu_width, menu_height);

        // ===== HEADER =====
        let header_height = 24.0 * s;
        draw_rectangle(
            menu_x + frame_thickness,
            menu_y + frame_thickness,
            menu_width - frame_thickness * 2.0,
            header_height,
            HEADER_BG,
        );
        draw_line(
            menu_x + frame_thickness,
            menu_y + frame_thickness + header_height,
            menu_x + menu_width - frame_thickness,
            menu_y + frame_thickness + header_height,
            1.0,
            HEADER_BORDER,
        );

        // Title centered in header
        let title = "Settings";
        let title_width = self.measure_text_sharp(title, 16.0).width;
        self.draw_text_sharp(
            title,
            (menu_x + (menu_width - title_width) / 2.0).floor(),
            (menu_y + frame_thickness + 17.0 * s).floor(),
            16.0,
            TEXT_TITLE,
        );

        // Get current mouse position for hover detection
        let (mouse_x, mouse_y) = mouse_position();

        // ===== CONTENT AREA =====
        let content_x = menu_x + frame_thickness + 8.0 * s;
        let mut y = menu_y + frame_thickness + header_height + 8.0 * s;

        // Shared dimensions
        let row_height = 26.0 * s;
        let btn_height = 24.0 * s;
        let slider_height = 16.0 * s;
        let inner_width = menu_width - frame_thickness * 2.0 - 16.0 * s;

        // Helper to draw themed button
        let draw_button = |btn_x: f32,
                           btn_y: f32,
                           btn_w: f32,
                           btn_h: f32,
                           text: &str,
                           is_selected: bool,
                           is_hovered: bool,
                           renderer: &Self| {
            let (bg_color, border_color) = if is_selected {
                // Gold-tinted fill + gold border = the single "active" language
                (Color::new(0.224, 0.190, 0.118, 1.0), FRAME_ACCENT) // gold-tinted (active)
            } else if is_hovered {
                (SLOT_HOVER_BG, SLOT_BORDER)
            } else {
                (SLOT_BG_EMPTY, SLOT_BORDER)
            };

            draw_rectangle(btn_x, btn_y, btn_w, btn_h, border_color);
            draw_rectangle(btn_x + 1.0, btn_y + 1.0, btn_w - 2.0, btn_h - 2.0, bg_color);

            if is_selected || is_hovered {
                draw_line(
                    btn_x + 2.0,
                    btn_y + 2.0,
                    btn_x + btn_w - 2.0,
                    btn_y + 2.0,
                    1.0,
                    FRAME_INNER,
                );
            }

            let text_width = renderer.measure_text_sharp(text, 16.0).width;
            let text_color = if is_selected { TEXT_TITLE } else { TEXT_NORMAL };
            renderer.draw_text_sharp(
                text,
                (btn_x + (btn_w - text_width) / 2.0).floor(),
                (btn_y + btn_h * 0.71).floor(),
                16.0,
                text_color,
            );
        };

        // Helper to check hover
        let is_hovered = |bounds: Rect| -> bool {
            mouse_x >= bounds.x
                && mouse_x <= bounds.x + bounds.w
                && mouse_y >= bounds.y
                && mouse_y <= bounds.y + bounds.h
        };

        // ===== ZOOM ROW (segmented selector) =====
        let zoom_seg_w = inner_width / 3.0;
        let zoom_05x_bounds = Rect::new(content_x, y, zoom_seg_w, btn_height);
        let zoom_1x_bounds = Rect::new(content_x + zoom_seg_w, y, zoom_seg_w, btn_height);
        let zoom_2x_bounds = Rect::new(content_x + zoom_seg_w * 2.0, y, zoom_seg_w, btn_height);
        layout.add(UiElementId::EscapeMenuZoom05x, zoom_05x_bounds);
        layout.add(UiElementId::EscapeMenuZoom1x, zoom_1x_bounds);
        layout.add(UiElementId::EscapeMenuZoom2x, zoom_2x_bounds);

        let is_05x_selected = (state.camera.zoom - 0.5).abs() < 0.1;
        let is_1x_selected = (state.camera.zoom - 1.0).abs() < 0.1;
        let is_2x_selected = (state.camera.zoom - 2.0).abs() < 0.1;
        self.draw_segmented_control(
            content_x,
            y,
            inner_width,
            btn_height,
            &[
                ("0.5x", is_05x_selected, is_hovered(zoom_05x_bounds)),
                ("1x", is_1x_selected, is_hovered(zoom_1x_bounds)),
                ("2x", is_2x_selected, is_hovered(zoom_2x_bounds)),
            ],
        );
        y += row_height + 4.0 * s;

        // ===== AUDIO SLIDERS =====
        // On Android: Music + SFX side by side on one row
        // On desktop: separate rows
        #[cfg(target_os = "android")]
        {
            let half_width = (inner_width - 6.0 * s) / 2.0;
            let label_offset = 30.0 * s;
            let left_slider_x = content_x + label_offset;
            let left_slider_w = half_width - label_offset;
            let right_slider_x = content_x + half_width + 6.0 * s + label_offset;
            let right_slider_w = half_width - label_offset;

            let music_bounds = Rect::new(left_slider_x, y, left_slider_w, slider_height);
            layout.add(UiElementId::EscapeMenuMusicSlider, music_bounds);
            self.draw_compact_slider(
                "Mus",
                left_slider_x,
                y,
                left_slider_w,
                slider_height,
                state.ui_state.audio_volume,
                state.ui_state.audio_muted,
                is_hovered(music_bounds),
            );

            let sfx_bounds = Rect::new(right_slider_x, y, right_slider_w, slider_height);
            layout.add(UiElementId::EscapeMenuSfxSlider, sfx_bounds);
            self.draw_compact_slider(
                "SFX",
                right_slider_x,
                y,
                right_slider_w,
                slider_height,
                state.ui_state.audio_sfx_volume,
                state.ui_state.audio_muted,
                is_hovered(sfx_bounds),
            );
            y += row_height - 4.0 * s;
        }
        #[cfg(not(target_os = "android"))]
        {
            let slider_width = inner_width - 50.0 * s;
            let slider_x = content_x + 42.0 * s;

            let music_bounds = Rect::new(slider_x, y, slider_width, slider_height);
            layout.add(UiElementId::EscapeMenuMusicSlider, music_bounds);
            self.draw_compact_slider(
                "Music",
                slider_x,
                y,
                slider_width,
                slider_height,
                state.ui_state.audio_volume,
                state.ui_state.audio_muted,
                is_hovered(music_bounds),
            );
            y += row_height - 4.0 * s;

            let sfx_bounds = Rect::new(slider_x, y, slider_width, slider_height);
            layout.add(UiElementId::EscapeMenuSfxSlider, sfx_bounds);
            self.draw_compact_slider(
                "SFX",
                slider_x,
                y,
                slider_width,
                slider_height,
                state.ui_state.audio_sfx_volume,
                state.ui_state.audio_muted,
                is_hovered(sfx_bounds),
            );
            y += row_height - 4.0 * s;
        }

        // UI Scale slider (not on Android — mobile is one-size-fits-all)
        #[cfg(not(target_os = "android"))]
        {
            let ui_slider_width = inner_width - 50.0 * s;
            let ui_slider_x = content_x + 42.0 * s;
            let scale_bounds = Rect::new(ui_slider_x, y, ui_slider_width, slider_height);
            layout.add(UiElementId::EscapeMenuUiScaleSlider, scale_bounds);
            let scale_normalized = (state.ui_state.ui_scale - 0.75) / 1.25; // 0.75-2.0 range
            self.draw_compact_slider(
                "Scale",
                ui_slider_x,
                y,
                ui_slider_width,
                slider_height,
                scale_normalized,
                false,
                is_hovered(scale_bounds),
            );
            y += row_height;
        }

        // ===== TOGGLE BUTTONS (2 per row) =====
        let toggle_w = (inner_width - 6.0 * s) / 2.0;

        // Row 1: Mute + Shift-Drop
        let mute_bounds = Rect::new(content_x, y, toggle_w, btn_height);
        let shift_drop_bounds = Rect::new(content_x + toggle_w + 6.0 * s, y, toggle_w, btn_height);
        layout.add(UiElementId::EscapeMenuMuteToggle, mute_bounds);
        layout.add(UiElementId::EscapeMenuShiftDropToggle, shift_drop_bounds);

        let mute_text = if state.ui_state.audio_muted {
            "Muted"
        } else {
            "Mute"
        };
        draw_button(
            mute_bounds.x,
            mute_bounds.y,
            toggle_w,
            btn_height,
            mute_text,
            state.ui_state.audio_muted,
            is_hovered(mute_bounds),
            self,
        );
        let shift_text = "ShftDrp";
        draw_button(
            shift_drop_bounds.x,
            shift_drop_bounds.y,
            toggle_w,
            btn_height,
            shift_text,
            state.ui_state.shift_drop_enabled,
            is_hovered(shift_drop_bounds),
            self,
        );
        y += row_height;

        // Row 2: Chat Log + ChatBG (ChatBG desktop only)
        let chat_bounds = Rect::new(content_x, y, toggle_w, btn_height);
        layout.add(UiElementId::EscapeMenuChatLogToggle, chat_bounds);

        let chat_text = "Chat";
        draw_button(
            chat_bounds.x,
            chat_bounds.y,
            toggle_w,
            btn_height,
            chat_text,
            state.ui_state.chat_log_visible,
            is_hovered(chat_bounds),
            self,
        );

        #[cfg(not(target_os = "android"))]
        {
            let chat_bg_bounds = Rect::new(content_x + toggle_w + 6.0 * s, y, toggle_w, btn_height);
            layout.add(UiElementId::EscapeMenuChatBgToggle, chat_bg_bounds);
            draw_button(
                chat_bg_bounds.x,
                chat_bg_bounds.y,
                toggle_w,
                btn_height,
                "ChatBG",
                state.ui_state.chat_log_background,
                is_hovered(chat_bg_bounds),
                self,
            );
        }
        y += row_height;

        // Row 3: Tap Walk
        let tap_walk_bounds = Rect::new(content_x, y, toggle_w, btn_height);
        layout.add(UiElementId::EscapeMenuTapPathfindToggle, tap_walk_bounds);

        let tap_text = "TapWalk";
        draw_button(
            tap_walk_bounds.x,
            tap_walk_bounds.y,
            toggle_w,
            btn_height,
            tap_text,
            state.ui_state.tap_to_pathfind,
            is_hovered(tap_walk_bounds),
            self,
        );
        y += row_height;

        // Row 3: Joystick toggle (Android only)
        #[cfg(target_os = "android")]
        {
            let joystick_bounds = Rect::new(content_x, y, toggle_w, btn_height);
            layout.add(UiElementId::EscapeMenuJoystickToggle, joystick_bounds);
            let joystick_text = "Joystick";
            draw_button(
                joystick_bounds.x,
                joystick_bounds.y,
                toggle_w,
                btn_height,
                joystick_text,
                state.ui_state.use_joystick,
                is_hovered(joystick_bounds),
                self,
            );
            y += row_height;
        }

        // Row: Control Scheme (Modern / Classic) - desktop only
        #[cfg(not(target_os = "android"))]
        {
            let ctrl_half = inner_width / 2.0;
            let ctrl_modern_bounds = Rect::new(content_x, y, ctrl_half, btn_height);
            let ctrl_classic_bounds = Rect::new(content_x + ctrl_half, y, ctrl_half, btn_height);
            layout.add(
                UiElementId::EscapeMenuControlSchemeToggle,
                ctrl_modern_bounds,
            );
            layout.add(
                UiElementId::EscapeMenuControlSchemeToggle,
                ctrl_classic_bounds,
            );

            let classic = state.ui_state.classic_controls;
            self.draw_segmented_control(
                content_x,
                y,
                inner_width,
                btn_height,
                &[
                    ("Modern", !classic, is_hovered(ctrl_modern_bounds)),
                    ("Classic", classic, is_hovered(ctrl_classic_bounds)),
                ],
            );
            y += row_height;
        }

        // Row: Graphics Quality (High / Low) - desktop only
        #[cfg(not(target_os = "android"))]
        {
            let gfx_half = inner_width / 2.0;
            let gfx_high_bounds = Rect::new(content_x, y, gfx_half, btn_height);
            let gfx_low_bounds = Rect::new(content_x + gfx_half, y, gfx_half, btn_height);
            layout.add(UiElementId::EscapeMenuGraphicsToggle, gfx_high_bounds);
            layout.add(UiElementId::EscapeMenuGraphicsToggle, gfx_low_bounds);

            let gfx_low = state.ui_state.graphics_low;
            self.draw_segmented_control(
                content_x,
                y,
                inner_width,
                btn_height,
                &[
                    ("GFX High", !gfx_low, is_hovered(gfx_high_bounds)),
                    ("GFX Low", gfx_low, is_hovered(gfx_low_bounds)),
                ],
            );
            y += row_height;
        }

        y += 8.0 * s;

        // ===== DISCONNECT BUTTON =====
        let disconnect_width = inner_width;
        let disconnect_height = 28.0 * s;
        let disconnect_x = content_x;
        let disconnect_y = y;
        let disconnect_bounds = Rect::new(
            disconnect_x,
            disconnect_y,
            disconnect_width,
            disconnect_height,
        );
        layout.add(UiElementId::EscapeMenuDisconnect, disconnect_bounds);

        let disconnect_hovered = is_hovered(disconnect_bounds);
        let disconnect_bg = if disconnect_hovered {
            Color::new(0.35, 0.15, 0.15, 1.0)
        } else {
            Color::new(0.25, 0.12, 0.12, 1.0)
        };
        let disconnect_border = Color::new(0.5, 0.2, 0.2, 1.0);

        draw_rectangle(
            disconnect_x,
            disconnect_y,
            disconnect_width,
            disconnect_height,
            disconnect_border,
        );
        draw_rectangle(
            disconnect_x + 1.0,
            disconnect_y + 1.0,
            disconnect_width - 2.0,
            disconnect_height - 2.0,
            disconnect_bg,
        );

        if disconnect_hovered {
            draw_line(
                disconnect_x + 2.0,
                disconnect_y + 2.0,
                disconnect_x + disconnect_width - 2.0,
                disconnect_y + 2.0,
                1.0,
                Color::new(0.6, 0.3, 0.3, 1.0),
            );
        }

        let disconnect_text = "Disconnect";
        let disconnect_text_width = self.measure_text_sharp(disconnect_text, 16.0).width;
        let disconnect_text_color = if disconnect_hovered {
            Color::new(1.0, 0.8, 0.8, 1.0)
        } else {
            Color::new(0.85, 0.7, 0.7, 1.0)
        };
        self.draw_text_sharp(
            disconnect_text,
            (disconnect_x + (disconnect_width - disconnect_text_width) / 2.0).floor(),
            (disconnect_y + disconnect_height * 0.68).floor(),
            16.0,
            disconnect_text_color,
        );
    }

    /// Draw a segmented selector: N mutually-exclusive options sharing one track
    /// with internal dividers (the 0.5x/1x/2x pattern). Visually distinguishes
    /// "pick one" controls from the independent on/off toggle buttons.
    /// Each segment is `(label, is_active, is_hovered)`.
    fn draw_segmented_control(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        segments: &[(&str, bool, bool)],
    ) {
        let n = segments.len();
        if n == 0 {
            return;
        }

        // Outer border (single track edge for the whole control).
        draw_rectangle(x, y, w, h, FRAME_MID);

        let inner_x = x + 1.0;
        let inner_y = y + 1.0;
        let inner_w = w - 2.0;
        let inner_h = h - 2.0;
        let seg_w = inner_w / n as f32;

        for (i, (label, is_active, is_hovered)) in segments.iter().enumerate() {
            let sx = inner_x + i as f32 * seg_w;

            let bg = if *is_active {
                Color::new(0.224, 0.190, 0.118, 1.0) // gold-tinted (active)
            } else if *is_hovered {
                SLOT_HOVER_BG
            } else {
                SLOT_BG_EMPTY
            };
            draw_rectangle(sx, inner_y, seg_w, inner_h, bg);

            if *is_active {
                // Gold top highlight = the shared "active" language.
                draw_line(
                    sx + 1.0,
                    inner_y + 1.0,
                    sx + seg_w - 1.0,
                    inner_y + 1.0,
                    1.0,
                    FRAME_ACCENT,
                );
            }

            // Divider between segments (makes it read as one connected control).
            if i > 0 {
                draw_line(sx, inner_y, sx, inner_y + inner_h, 1.0, FRAME_OUTER);
            }

            let label_w = self.measure_text_sharp(label, 16.0).width;
            let text_color = if *is_active {
                TEXT_TITLE
            } else if *is_hovered {
                TEXT_NORMAL
            } else {
                TEXT_DIM
            };
            self.draw_text_sharp(
                label,
                (sx + (seg_w - label_w) / 2.0).floor(),
                (inner_y + inner_h * 0.71).floor(),
                16.0,
                text_color,
            );
        }
    }

    /// Draw a compact slider with label on left
    fn draw_compact_slider(
        &self,
        label: &str,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        value: f32,
        muted: bool,
        hovered: bool,
    ) {
        let s = self.font_scale.get();
        // Label to the left
        let label_width = self.measure_text_sharp(label, 16.0).width;
        self.draw_text_sharp(
            label,
            (x - label_width - 6.0 * s).floor(),
            (y + height * 0.75).floor(),
            16.0,
            TEXT_DIM,
        );

        // Track
        let track_color = if hovered {
            SLOT_HOVER_BG
        } else {
            SLOT_BG_EMPTY
        };
        draw_rectangle(x, y, width, height, SLOT_BORDER);
        draw_rectangle(x + 1.0, y + 1.0, width - 2.0, height - 2.0, track_color);

        // Fill
        let fill_width = (width - 4.0) * value;
        let fill_color = if muted {
            Color::new(0.3, 0.3, 0.35, 1.0)
        } else {
            Color::new(0.4, 0.55, 0.3, 1.0)
        };
        draw_rectangle(x + 2.0, y + 2.0, fill_width, height - 4.0, fill_color);

        // Handle
        let handle_x = x + 2.0 + fill_width - 3.0;
        let handle_color = if muted {
            Color::new(0.5, 0.5, 0.55, 1.0)
        } else {
            FRAME_ACCENT
        };
        draw_rectangle(
            handle_x.max(x + 2.0),
            y + 2.0,
            6.0,
            height - 4.0,
            handle_color,
        );
    }
}
