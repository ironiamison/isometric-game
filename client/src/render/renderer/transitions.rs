use super::*;

impl Renderer {
    pub fn render_world_fade_in(&self, state: &GameState) {
        if state.world_fade_in <= 0.0 {
            return;
        }
        let (sw, sh) = virtual_screen_size();
        let bg = Color::from_rgba(30, 30, 40, 255);
        draw_rectangle(
            0.0,
            0.0,
            sw,
            sh,
            Color::new(bg.r, bg.g, bg.b, state.world_fade_in),
        );
    }

    /// Render transition fade overlay
    pub fn render_transition_overlay(&self, state: &GameState) {
        use crate::game::state::TransitionState;

        if state.map_transition.state == TransitionState::None {
            return;
        }

        let alpha = state.map_transition.progress;
        let (trans_sw, trans_sh) = virtual_screen_size();
        draw_rectangle(
            0.0,
            0.0,
            trans_sw,
            trans_sh,
            Color::new(0.0, 0.0, 0.0, alpha),
        );
    }

    /// Render the tutorial hint bar at the bottom of the screen.
    pub fn render_tutorial_hint(&self, state: &GameState) {
        let Some(tutorial) = &state.tutorial else {
            return;
        };
        if !tutorial.hint_visible || tutorial.is_done() {
            return;
        }

        let hint_text = tutorial.hint_text();
        if hint_text.is_empty() {
            return;
        }

        let (sw, _sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;
        let font_size = 24.0;
        let skip_font_size = 16.0;

        // Fade in based on time since phase started
        let age = get_time() - tutorial.phase_start_time;
        let alpha = (age / 0.4).min(1.0) as f32; // 400ms fade in

        // Measure text
        let hint_dims = self.measure_text_sharp(hint_text, font_size);
        let skip_text = "Press Esc to skip tutorial";
        let skip_dims = self.measure_text_sharp(skip_text, skip_font_size);

        // Bar dimensions
        let padding_x = 20.0 * s;
        let padding_y = 10.0 * s;
        let bar_w = hint_dims.width.max(skip_dims.width) + padding_x * 2.0;
        let bar_h = hint_dims.height + skip_dims.height + padding_y * 3.0;
        let bar_x = ((sw - bar_w) / 2.0).floor();
        let bar_y = 10.0 * s; // Aligned to top edge

        // Background
        draw_rectangle(
            bar_x,
            bar_y,
            bar_w,
            bar_h,
            Color::from_rgba(0, 0, 0, (180.0 * alpha) as u8),
        );

        // Border
        let border_color = Color::from_rgba(200, 180, 120, (180.0 * alpha) as u8);
        draw_rectangle_lines(bar_x, bar_y, bar_w, bar_h, 1.0, border_color);

        // Hint text (centered)
        let hint_x = ((sw - hint_dims.width) / 2.0).floor();
        let hint_y = bar_y + padding_y + hint_dims.height;
        let text_alpha = (255.0 * alpha) as u8;

        // Outline
        for ox in [-1.0_f32, 1.0] {
            for oy in [-1.0_f32, 1.0] {
                self.draw_text_sharp(
                    hint_text,
                    hint_x + ox,
                    hint_y + oy,
                    font_size,
                    Color::from_rgba(0, 0, 0, text_alpha),
                );
            }
        }
        self.draw_text_sharp(
            hint_text,
            hint_x,
            hint_y,
            font_size,
            Color::from_rgba(255, 255, 220, text_alpha),
        );

        // Skip text (centered, dimmer)
        let skip_x = ((sw - skip_dims.width) / 2.0).floor();
        let skip_y = hint_y + padding_y + skip_dims.height;
        self.draw_text_sharp(
            skip_text,
            skip_x,
            skip_y,
            skip_font_size,
            Color::from_rgba(160, 160, 160, (160.0 * alpha) as u8),
        );
    }
}
