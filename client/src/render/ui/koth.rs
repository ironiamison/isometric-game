//! KOTH (King of the Hill) wave survival minigame UI

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

impl Renderer {
    /// Render the KOTH wave HUD (top-center, always visible during KOTH)
    pub(crate) fn render_koth_hud(&self, state: &GameState) {
        let koth = match &state.koth {
            Some(k) => k,
            None => return,
        };

        let s = state.ui_state.ui_scale;
        let (sw, _sh) = virtual_screen_size();

        let hud_width = 220.0 * s;
        let hud_height = 70.0 * s;
        let hud_x = (sw - hud_width) / 2.0;
        let hud_y = 8.0 * s;

        // Background
        draw_rectangle(
            hud_x,
            hud_y,
            hud_width,
            hud_height,
            Color::new(0.0, 0.0, 0.0, 0.7),
        );
        draw_rectangle_lines(hud_x, hud_y, hud_width, hud_height, 2.0, FRAME_ACCENT);

        let cx = hud_x + hud_width / 2.0;

        // Phase-specific display
        match koth.phase.as_str() {
            "countdown" => {
                let secs = (koth.countdown_ms as f32 / 1000.0).ceil() as u32;
                let wave_text = format!("Wave {}", koth.wave);
                let countdown_text = format!("Starting in {}...", secs);

                let wave_dims = self.measure_text_sharp(&wave_text, 16.0);
                self.draw_text_sharp(
                    &wave_text,
                    cx - wave_dims.width / 2.0,
                    hud_y + 22.0 * s,
                    16.0,
                    TEXT_TITLE,
                );
                let cd_dims = self.measure_text_sharp(&countdown_text, 16.0);
                self.draw_text_sharp(
                    &countdown_text,
                    cx - cd_dims.width / 2.0,
                    hud_y + 42.0 * s,
                    16.0,
                    YELLOW,
                );
                let pts_text = format!("Points: {}", koth.points);
                let pts_dims = self.measure_text_sharp(&pts_text, 16.0);
                self.draw_text_sharp(
                    &pts_text,
                    cx - pts_dims.width / 2.0,
                    hud_y + 60.0 * s,
                    16.0,
                    TEXT_DIM,
                );
            }
            "active" => {
                let wave_text = format!("Wave {}", koth.wave);
                let wave_dims = self.measure_text_sharp(&wave_text, 16.0);
                self.draw_text_sharp(
                    &wave_text,
                    cx - wave_dims.width / 2.0,
                    hud_y + 22.0 * s,
                    16.0,
                    TEXT_TITLE,
                );

                // Enemy progress bar
                let bar_width = hud_width - 20.0 * s;
                let bar_x = hud_x + 10.0 * s;
                let bar_y = hud_y + 30.0 * s;
                let bar_height = 12.0 * s;
                let progress = if koth.enemies_total > 0 {
                    1.0 - (koth.enemies_alive as f32 / koth.enemies_total as f32)
                } else {
                    0.0
                };

                draw_rectangle(bar_x, bar_y, bar_width, bar_height, Color::new(0.2, 0.1, 0.1, 0.8));
                draw_rectangle(bar_x, bar_y, bar_width * progress, bar_height, Color::new(0.8, 0.2, 0.2, 0.9));
                draw_rectangle_lines(bar_x, bar_y, bar_width, bar_height, 1.0, FRAME_OUTER);

                let enemy_text = format!(
                    "Enemies: {}/{}",
                    koth.enemies_total - koth.enemies_alive,
                    koth.enemies_total
                );
                let enemy_dims = self.measure_text_sharp(&enemy_text, 16.0);
                self.draw_text_sharp(
                    &enemy_text,
                    cx - enemy_dims.width / 2.0,
                    hud_y + 56.0 * s,
                    16.0,
                    TEXT_NORMAL,
                );

                let pts_text = format!("Points: {}", koth.points);
                let pts_dims = self.measure_text_sharp(&pts_text, 16.0);
                self.draw_text_sharp(
                    &pts_text,
                    cx - pts_dims.width / 2.0,
                    hud_y + 68.0 * s,
                    16.0,
                    TEXT_DIM,
                );
            }
            "wave_complete" => {
                let text = "Wave Complete!";
                let dims = self.measure_text_sharp(text, 16.0);
                self.draw_text_sharp(text, cx - dims.width / 2.0, hud_y + 30.0 * s, 16.0, GREEN);

                let pts_text = format!("Points: {}", koth.points);
                let pts_dims = self.measure_text_sharp(&pts_text, 16.0);
                self.draw_text_sharp(
                    &pts_text,
                    cx - pts_dims.width / 2.0,
                    hud_y + 52.0 * s,
                    16.0,
                    TEXT_NORMAL,
                );
            }
            "checkpoint" => {
                let text = "Checkpoint!";
                let dims = self.measure_text_sharp(text, 16.0);
                self.draw_text_sharp(text, cx - dims.width / 2.0, hud_y + 30.0 * s, 16.0, YELLOW);

                let pts_text = format!("Points: {}", koth.points);
                let pts_dims = self.measure_text_sharp(&pts_text, 16.0);
                self.draw_text_sharp(
                    &pts_text,
                    cx - pts_dims.width / 2.0,
                    hud_y + 52.0 * s,
                    16.0,
                    TEXT_NORMAL,
                );
            }
            _ => {}
        }
    }

    /// Render the KOTH checkpoint dialog (modal, centered)
    pub(crate) fn render_koth_checkpoint(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        if !state.koth_checkpoint_open {
            return;
        }

        let info = match &state.koth_checkpoint_info {
            Some(i) => i,
            None => return,
        };

        let s = state.ui_state.ui_scale;
        let (sw, sh) = virtual_screen_size();

        // Semi-transparent overlay
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.45));

        let box_width = 300.0 * s;
        let reward_rows = info.rewards.len().max(1) as f32;
        let box_height = (160.0 + reward_rows * 22.0) * s;
        let box_x = (sw - box_width) / 2.0;
        let box_y = (sh - box_height) / 2.0;

        // Panel
        self.draw_panel_frame(box_x, box_y, box_width, box_height);
        self.draw_corner_accents(box_x, box_y, box_width, box_height);

        let cx = box_x + box_width / 2.0;
        let mut y = box_y + 24.0 * s;

        // Title
        let title = format!("Wave {} Complete!", info.wave);
        let title_dims = self.measure_text_sharp(&title, 16.0);
        self.draw_text_sharp(&title, cx - title_dims.width / 2.0, y, 16.0, TEXT_TITLE);
        y += 24.0 * s;

        // Points
        let pts = format!("Points: {}", info.points);
        let pts_dims = self.measure_text_sharp(&pts, 16.0);
        self.draw_text_sharp(&pts, cx - pts_dims.width / 2.0, y, 16.0, YELLOW);
        y += 24.0 * s;

        // Rewards
        let rewards_label = "Rewards:";
        let rl_dims = self.measure_text_sharp(rewards_label, 16.0);
        self.draw_text_sharp(rewards_label, cx - rl_dims.width / 2.0, y, 16.0, TEXT_NORMAL);
        y += 18.0 * s;

        for reward in &info.rewards {
            let item_name = state
                .item_registry
                .get_or_placeholder(&reward.item_id)
                .display_name
                .clone();
            let reward_text = format!("{} x{}", item_name, reward.quantity);
            let r_dims = self.measure_text_sharp(&reward_text, 16.0);
            self.draw_text_sharp(&reward_text, cx - r_dims.width / 2.0, y, 16.0, TEXT_DIM);
            y += 22.0 * s;
        }

        y += 8.0 * s;

        // Next wave info
        let next_text = format!("Next wave: {} enemies", info.next_wave_enemy_count);
        let next_dims = self.measure_text_sharp(&next_text, 16.0);
        self.draw_text_sharp(&next_text, cx - next_dims.width / 2.0, y, 16.0, TEXT_DIM);
        y += 28.0 * s;

        // Buttons
        let button_width = 120.0 * s;
        let button_height = 30.0 * s;
        let button_gap = 16.0 * s;

        // "Claim Rewards" button (green)
        let leave_x = cx - button_width - button_gap / 2.0;
        let is_leave_hovered = matches!(hovered, Some(UiElementId::KothLeaveButton));
        let leave_color = if is_leave_hovered {
            Color::new(0.2, 0.7, 0.2, 1.0)
        } else {
            Color::new(0.15, 0.5, 0.15, 1.0)
        };
        draw_rectangle(leave_x, y, button_width, button_height, leave_color);
        draw_rectangle_lines(leave_x, y, button_width, button_height, 1.0, FRAME_OUTER);
        let leave_text = "Claim Rewards";
        let lt_dims = self.measure_text_sharp(leave_text, 16.0);
        self.draw_text_sharp(
            leave_text,
            leave_x + (button_width - lt_dims.width) / 2.0,
            y + (button_height + lt_dims.height) / 2.0,
            16.0,
            WHITE,
        );
        layout.add(
            UiElementId::KothLeaveButton,
            Rect::new(leave_x, y, button_width, button_height),
        );

        // "Keep Fighting" button (orange)
        let fight_x = cx + button_gap / 2.0;
        let is_fight_hovered = matches!(hovered, Some(UiElementId::KothContinueButton));
        let fight_color = if is_fight_hovered {
            Color::new(0.9, 0.6, 0.1, 1.0)
        } else {
            Color::new(0.7, 0.45, 0.05, 1.0)
        };
        draw_rectangle(fight_x, y, button_width, button_height, fight_color);
        draw_rectangle_lines(fight_x, y, button_width, button_height, 1.0, FRAME_OUTER);
        let fight_text = "Keep Fighting";
        let ft_dims = self.measure_text_sharp(fight_text, 16.0);
        self.draw_text_sharp(
            fight_text,
            fight_x + (button_width - ft_dims.width) / 2.0,
            y + (button_height + ft_dims.height) / 2.0,
            16.0,
            WHITE,
        );
        layout.add(
            UiElementId::KothContinueButton,
            Rect::new(fight_x, y, button_width, button_height),
        );
    }

    /// Render the KOTH game over screen (modal)
    pub(crate) fn render_koth_game_over(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let info = match &state.koth_game_over {
            Some(i) => i,
            None => return,
        };

        let s = state.ui_state.ui_scale;
        let (sw, sh) = virtual_screen_size();

        // Semi-transparent overlay
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));

        let box_width = 280.0 * s;
        let reward_rows = info.rewards.len().max(1) as f32;
        let box_height = (160.0 + reward_rows * 22.0) * s;
        let box_x = (sw - box_width) / 2.0;
        let box_y = (sh - box_height) / 2.0;

        self.draw_panel_frame(box_x, box_y, box_width, box_height);
        self.draw_corner_accents(box_x, box_y, box_width, box_height);

        let cx = box_x + box_width / 2.0;
        let mut y = box_y + 28.0 * s;

        // Title
        let title = if info.victory { "Victory!" } else { "Defeated!" };
        let title_color = if info.victory { GREEN } else { RED };
        let title_dims = self.measure_text_sharp(title, 16.0);
        self.draw_text_sharp(title, cx - title_dims.width / 2.0, y, 16.0, title_color);
        y += 28.0 * s;

        // Stats
        let waves = format!("Waves: {}", info.waves_completed);
        let waves_dims = self.measure_text_sharp(&waves, 16.0);
        self.draw_text_sharp(&waves, cx - waves_dims.width / 2.0, y, 16.0, TEXT_NORMAL);
        y += 22.0 * s;

        let pts = format!("Points: {}", info.total_points);
        let pts_dims = self.measure_text_sharp(&pts, 16.0);
        self.draw_text_sharp(&pts, cx - pts_dims.width / 2.0, y, 16.0, YELLOW);
        y += 24.0 * s;

        // Rewards
        if !info.rewards.is_empty() {
            let rewards_label = "Rewards:";
            let rl_dims = self.measure_text_sharp(rewards_label, 16.0);
            self.draw_text_sharp(rewards_label, cx - rl_dims.width / 2.0, y, 16.0, TEXT_NORMAL);
            y += 18.0 * s;

            for reward in &info.rewards {
                let item_name = state
                    .item_registry
                    .get_or_placeholder(&reward.item_id)
                    .display_name
                    .clone();
                let reward_text = format!("{} x{}", item_name, reward.quantity);
                let r_dims = self.measure_text_sharp(&reward_text, 16.0);
                self.draw_text_sharp(&reward_text, cx - r_dims.width / 2.0, y, 16.0, TEXT_DIM);
                y += 22.0 * s;
            }
        }

        y += 10.0 * s;

        // Auto-return timer
        let elapsed = get_time() - info.shown_at;
        let remaining = (10.0 - elapsed).max(0.0);
        if remaining > 0.0 {
            let timer_text = format!("Returning in {:.0}s...", remaining.ceil());
            let timer_dims = self.measure_text_sharp(&timer_text, 16.0);
            self.draw_text_sharp(
                &timer_text,
                cx - timer_dims.width / 2.0,
                y,
                16.0,
                TEXT_DIM,
            );
        }

        // Dismiss button
        y += 20.0 * s;
        let button_width = 100.0 * s;
        let button_height = 28.0 * s;
        let btn_x = cx - button_width / 2.0;
        let is_hovered = matches!(hovered, Some(UiElementId::KothGameOverDismiss));
        let btn_color = if is_hovered {
            Color::new(0.3, 0.3, 0.4, 1.0)
        } else {
            Color::new(0.2, 0.2, 0.3, 1.0)
        };
        draw_rectangle(btn_x, y, button_width, button_height, btn_color);
        draw_rectangle_lines(btn_x, y, button_width, button_height, 1.0, FRAME_OUTER);
        let ok_text = "OK";
        let ok_dims = self.measure_text_sharp(ok_text, 16.0);
        self.draw_text_sharp(
            ok_text,
            btn_x + (button_width - ok_dims.width) / 2.0,
            y + (button_height + ok_dims.height) / 2.0,
            16.0,
            WHITE,
        );
        layout.add(
            UiElementId::KothGameOverDismiss,
            Rect::new(btn_x, y, button_width, button_height),
        );
    }
}
