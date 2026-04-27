//! Desert Wurm boss fight HUD - HP bar and phase indicator

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

impl Renderer {
    pub(crate) fn render_boss_hud(&self, state: &GameState) {
        let boss = match &state.boss {
            Some(b) => b,
            None => return,
        };

        let s = state.ui_state.ui_scale;
        let (sw, _sh) = virtual_screen_size();

        let panel_width = 320.0 * s;
        let panel_height = 50.0 * s;
        let panel_x = (sw - panel_width) / 2.0;
        let panel_y = 6.0 * s;

        // Background
        draw_rectangle(
            panel_x,
            panel_y,
            panel_width,
            panel_height,
            Color::new(0.0, 0.0, 0.0, 0.75),
        );
        draw_rectangle_lines(
            panel_x,
            panel_y,
            panel_width,
            panel_height,
            2.0,
            FRAME_ACCENT,
        );

        // Boss name
        let name = "Desert Wurm";
        let name_dims = self.measure_text_sharp(name, 16.0);
        self.draw_text_sharp(
            name,
            (sw - name_dims.width) / 2.0,
            panel_y + 16.0 * s,
            16.0,
            TEXT_TITLE,
        );

        // HP bar
        let bar_width = panel_width - 20.0 * s;
        let bar_height = 16.0 * s;
        let bar_x = panel_x + 10.0 * s;
        let bar_y = panel_y + 24.0 * s;

        let hp_pct = if boss.max_hp > 0 {
            boss.hp as f32 / boss.max_hp as f32
        } else {
            0.0
        };

        let bar_color = match boss.phase.as_str() {
            "hunt" => Color::new(0.2, 0.8, 0.2, 0.9),
            "storm" => Color::new(0.9, 0.7, 0.1, 0.9),
            "frenzy" => Color::new(0.9, 0.2, 0.1, 0.9),
            _ => Color::new(0.8, 0.2, 0.2, 0.9),
        };

        draw_rectangle(
            bar_x,
            bar_y,
            bar_width,
            bar_height,
            Color::new(0.15, 0.1, 0.1, 0.9),
        );
        draw_rectangle(bar_x, bar_y, bar_width * hp_pct, bar_height, bar_color);
        draw_rectangle_lines(bar_x, bar_y, bar_width, bar_height, 1.0, FRAME_OUTER);

        let hp_text = format!("{} / {}", boss.hp, boss.max_hp);
        let hp_dims = self.measure_text_sharp(&hp_text, 16.0);
        self.draw_text_sharp(
            &hp_text,
            bar_x + (bar_width - hp_dims.width) / 2.0,
            bar_y + (bar_height + hp_dims.height) / 2.0,
            16.0,
            WHITE,
        );
    }
}
