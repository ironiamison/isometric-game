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

        // Content height is the interior (header + grouped sections + disconnect).
        // Layout below must keep the accumulated `y` in sync with these totals.
        // Audio mute lives on per-channel icons now, so the old "Mute" toggle row
        // is gone. Desktop matches the Social panel's 314px footprint again; the
        // freed space becomes balanced breathing room (~16 above / ~15 below) for
        // the Disconnect button.
        //   Desktop: header/pad(32) + ZOOM(44) + AUDIO(58) + DISPLAY(39)
        //            + TOGGLES(74) + gap(16) + disconnect(28) + pad(15) = 306
        //   Android: header/pad(32) + ZOOM(44) + AUDIO(58) + TOGGLES(74)
        //            + gap(16) + disconnect(28) + pad(15) = 267
        #[cfg(not(target_os = "android"))]
        let content_height = 306.0 * s;
        #[cfg(target_os = "android")]
        let content_height = 267.0 * s;

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
        let btn_height = 24.0 * s; // segmented controls
        let toggle_height = 24.0 * s; // independent on/off rows
        let toggle_gap = 3.0 * s;
        let slider_height = 16.0 * s;
        let slider_gap = 3.0 * s;
        let label_height = 16.0 * s; // section header advance
        let section_gap = 4.0 * s; // breathing room before a new section
        let inner_width = menu_width - frame_thickness * 2.0 - 16.0 * s;

        // Helper to check hover
        let is_hovered = |bounds: Rect| -> bool {
            mouse_x >= bounds.x
                && mouse_x <= bounds.x + bounds.w
                && mouse_y >= bounds.y
                && mouse_y <= bounds.y + bounds.h
        };

        // ===== ZOOM =====
        self.draw_section_label("Zoom", content_x, y, s);
        y += label_height;

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
        y += btn_height + 4.0 * s;

        // ===== AUDIO =====
        y += section_gap;
        self.draw_section_label("Audio", content_x, y, s);
        y += label_height;

        // Music + SFX sliders stacked, each with a bg-less speaker/mute icon
        // button at the right edge that toggles that channel independently.
        // 16px native icons drawn at the slider height keep them pixel-crisp.
        let icon_size = slider_height;
        let icon_gap = 6.0 * s;
        let slider_label_w = 42.0 * s;
        let audio_slider_x = content_x + slider_label_w;
        let audio_slider_w = inner_width - slider_label_w - icon_size - icon_gap;
        let mute_icon_x = content_x + inner_width - icon_size;
        let mute_icon_dy = ((slider_height - icon_size) / 2.0).round();

        // Music row
        let music_bounds = Rect::new(audio_slider_x, y, audio_slider_w, slider_height);
        layout.add(UiElementId::EscapeMenuMusicSlider, music_bounds);
        self.draw_compact_slider(
            "Music",
            content_x,
            audio_slider_x,
            y,
            audio_slider_w,
            slider_height,
            state.ui_state.audio_volume,
            state.ui_state.music_muted,
            is_hovered(music_bounds),
        );
        let music_mute_bounds = Rect::new(mute_icon_x, y + mute_icon_dy, icon_size, icon_size);
        layout.add(UiElementId::EscapeMenuMusicMuteToggle, music_mute_bounds);
        self.draw_mute_icon(
            mute_icon_x,
            y + mute_icon_dy,
            icon_size,
            state.ui_state.music_muted,
            is_hovered(music_mute_bounds),
        );
        y += slider_height + slider_gap;

        // SFX row
        let sfx_bounds = Rect::new(audio_slider_x, y, audio_slider_w, slider_height);
        layout.add(UiElementId::EscapeMenuSfxSlider, sfx_bounds);
        self.draw_compact_slider(
            "SFX",
            content_x,
            audio_slider_x,
            y,
            audio_slider_w,
            slider_height,
            state.ui_state.audio_sfx_volume,
            state.ui_state.sfx_muted,
            is_hovered(sfx_bounds),
        );
        let sfx_mute_bounds = Rect::new(mute_icon_x, y + mute_icon_dy, icon_size, icon_size);
        layout.add(UiElementId::EscapeMenuSfxMuteToggle, sfx_mute_bounds);
        self.draw_mute_icon(
            mute_icon_x,
            y + mute_icon_dy,
            icon_size,
            state.ui_state.sfx_muted,
            is_hovered(sfx_mute_bounds),
        );
        y += slider_height + slider_gap;

        // ===== DISPLAY (desktop only) =====
        #[cfg(not(target_os = "android"))]
        {
            y += section_gap;
            self.draw_section_label("Display", content_x, y, s);
            y += label_height;

            // Graphics quality (GFX High / GFX Low) — hidden for now.
            // let gfx_half = inner_width / 2.0;
            // let gfx_high_bounds = Rect::new(content_x, y, gfx_half, btn_height);
            // let gfx_low_bounds = Rect::new(content_x + gfx_half, y, gfx_half, btn_height);
            // layout.add(UiElementId::EscapeMenuGraphicsToggle, gfx_high_bounds);
            // layout.add(UiElementId::EscapeMenuGraphicsToggle, gfx_low_bounds);
            // let gfx_low = state.ui_state.graphics_low;
            // self.draw_segmented_control(
            //     content_x,
            //     y,
            //     inner_width,
            //     btn_height,
            //     &[
            //         ("GFX High", !gfx_low, is_hovered(gfx_high_bounds)),
            //         ("GFX Low", gfx_low, is_hovered(gfx_low_bounds)),
            //     ],
            // );
            // y += btn_height + 4.0 * s;

            // UI scale slider
            let ui_slider_width = inner_width - 50.0 * s;
            let ui_slider_x = content_x + 42.0 * s;
            let scale_bounds = Rect::new(ui_slider_x, y, ui_slider_width, slider_height);
            layout.add(UiElementId::EscapeMenuUiScaleSlider, scale_bounds);
            // While dragging, reflect the pending (not-yet-applied) value so the
            // thumb tracks the cursor even though the panel isn't rescaling yet.
            let displayed_scale = state
                .ui_state
                .ui_scale_pending
                .unwrap_or(state.ui_state.ui_scale);
            let scale_normalized = (displayed_scale - 0.75) / 1.25; // 0.75-2.0 range
            self.draw_compact_slider(
                "Scale",
                content_x,
                ui_slider_x,
                y,
                ui_slider_width,
                slider_height,
                scale_normalized,
                false,
                is_hovered(scale_bounds),
            );
            y += slider_height + slider_gap;
        }

        // ===== TOGGLES (2-column grid of independent on/off switches) =====
        y += section_gap;
        self.draw_section_label("Toggles", content_x, y, s);
        y += label_height;

        // Tap/click to walk wording matches the platform's primary input.
        #[cfg(target_os = "android")]
        let walk_label = "Tap walk";
        #[cfg(not(target_os = "android"))]
        let walk_label = "Click walk";

        // Build the toggle list (platform-specific entries via cfg), then lay it
        // out two-per-row. Short labels keep each cell readable at half width.
        // (Shift-to-drop is intentionally omitted — it stays silently enabled.)
        let mut toggles: Vec<(UiElementId, &str, bool)> = Vec::new();
        #[cfg(not(target_os = "android"))]
        toggles.push((
            UiElementId::EscapeMenuControlSchemeToggle,
            "Modern",
            !state.ui_state.classic_controls,
        ));
        toggles.push((
            UiElementId::EscapeMenuChatLogToggle,
            "Chat",
            state.ui_state.chat_log_visible,
        ));
        #[cfg(not(target_os = "android"))]
        toggles.push((
            UiElementId::EscapeMenuChatBgToggle,
            "Chat BG",
            state.ui_state.chat_log_background,
        ));
        toggles.push((
            UiElementId::EscapeMenuTapPathfindToggle,
            walk_label,
            state.ui_state.tap_to_pathfind,
        ));
        #[cfg(target_os = "android")]
        toggles.push((
            UiElementId::EscapeMenuJoystickToggle,
            "Joystick",
            state.ui_state.use_joystick,
        ));

        let col_gap = 6.0 * s;
        let toggle_w = (inner_width - col_gap) / 2.0;
        for (i, (id, label, is_on)) in toggles.iter().enumerate() {
            let col = (i % 2) as f32;
            let row = (i / 2) as f32;
            let tx = content_x + col * (toggle_w + col_gap);
            let ty = y + row * (toggle_height + toggle_gap);
            let bounds = Rect::new(tx, ty, toggle_w, toggle_height);
            layout.add(id.clone(), bounds);
            self.draw_toggle_row(
                tx,
                ty,
                toggle_w,
                toggle_height,
                label,
                *is_on,
                is_hovered(bounds),
                s,
            );
        }
        // (Disconnect is now bottom-anchored, so the toggle grid no longer needs
        // to advance `y` past its last row.)

        // ===== DISCONNECT BUTTON =====
        // Anchored to the bottom of the panel with the same padding the content
        // has at the top (8px below the header), rather than flowing with `y`.
        let disconnect_width = inner_width;
        let disconnect_height = 28.0 * s;
        let disconnect_x = content_x;
        let disconnect_y =
            menu_y + menu_height - frame_thickness - 8.0 * s - disconnect_height;
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

    /// Draw a background-less speaker/mute icon button (just the sprite).
    /// Uses the dedicated sound.png / mute.png textures; brightens on hover.
    fn draw_mute_icon(&self, x: f32, y: f32, size: f32, muted: bool, hovered: bool) {
        let texture = if muted {
            &self.mute_icon
        } else {
            &self.sound_icon
        };
        let Some(texture) = texture.as_ref() else {
            return;
        };
        let tint = if hovered {
            WHITE
        } else {
            Color::new(0.85, 0.85, 0.88, 1.0)
        };
        draw_texture_ex(
            texture,
            x,
            y,
            tint,
            DrawTextureParams {
                dest_size: Some(Vec2::new(size, size)),
                ..Default::default()
            },
        );
    }

    /// Draw a section header that groups the controls beneath it — the
    /// labelled groups in the redesigned panel. White so the groups read
    /// clearly against the dim control labels.
    fn draw_section_label(&self, text: &str, x: f32, y: f32, s: f32) {
        let white = Color::new(0.94, 0.94, 0.96, 1.0);
        self.draw_text_sharp(
            text,
            x.floor(),
            (y + 11.0 * s).floor(),
            14.0,
            white,
        );
    }

    /// Draw a full-width independent on/off toggle: a labelled row that lights
    /// up (gold-tinted, bright label, green check) when on and reads dim with a
    /// dash when off. Distinct from the segmented "pick one" controls.
    #[allow(clippy::too_many_arguments)]
    fn draw_toggle_row(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        label: &str,
        is_on: bool,
        is_hovered: bool,
        s: f32,
    ) {
        let border = if is_on {
            FRAME_MID
        } else if is_hovered {
            SLOT_HOVER_BORDER
        } else {
            SLOT_BORDER
        };
        let bg = if is_hovered {
            SLOT_HOVER_BG
        } else if is_on {
            SLOT_BG_FILLED
        } else {
            SLOT_BG_EMPTY
        };

        draw_rectangle(x, y, w, h, border);
        draw_rectangle(x + 1.0, y + 1.0, w - 2.0, h - 2.0, bg);

        // Subtle top highlight reinforces the "lit" state.
        if is_on {
            draw_line(x + 2.0, y + 1.5, x + w - 2.0, y + 1.5, 1.0, FRAME_INNER);
        }

        // Label, left aligned.
        let label_color = if is_on { TEXT_NORMAL } else { TEXT_DIM };
        self.draw_text_sharp(
            label,
            (x + 10.0 * s).floor(),
            (y + h * 0.68).floor(),
            16.0,
            label_color,
        );

        // State indicator on the right: green check when on, dim dash when off.
        let cx = x + w - 16.0 * s;
        let cy = y + h * 0.5;
        if is_on {
            let green = Color::new(0.45, 0.82, 0.45, 1.0);
            let t = 2.0 * s;
            draw_line(cx - 5.0 * s, cy, cx - 1.0 * s, cy + 4.0 * s, t, green);
            draw_line(cx - 1.0 * s, cy + 4.0 * s, cx + 6.0 * s, cy - 5.0 * s, t, green);
        } else {
            draw_line(cx - 5.0 * s, cy, cx + 5.0 * s, cy, 2.0 * s, TEXT_DIM);
        }
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

    /// Draw a compact slider with a left-aligned label in the gutter at `label_x`.
    #[allow(clippy::too_many_arguments)]
    fn draw_compact_slider(
        &self,
        label: &str,
        label_x: f32,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        value: f32,
        muted: bool,
        hovered: bool,
    ) {
        // Label left-aligned in its gutter (cleaner than right-hugging the track).
        self.draw_text_sharp(
            label,
            label_x.floor(),
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
