//! XP drop feed - floating "+X XP" notifications with skill icons

use macroquad::prelude::*;
use crate::game::state::XpDropFeed;
use crate::game::SkillType;

const DROP_LIFETIME: f64 = 2.0;
const FLOAT_DISTANCE: f32 = 120.0; // How far drops travel upward
const FADE_START: f64 = 0.6; // When fading begins
const ROW_HEIGHT: f32 = 20.0;
const ICON_SIZE: f32 = 16.0;
const UI_ICON_SIZE: f32 = 24.0;

use super::super::Renderer;

impl Renderer {
    /// Render XP drop feed: each drop shows a skill icon + "+X XP", floating upward and fading out
    pub fn render_xp_drop_feed(&self, feed: &XpDropFeed, right_edge_x: f32, bar_width: f32, start_y: f32) {
        let current_time = macroquad::time::get_time();

        // Sort drops by time so we can detect groups and assign slot offsets
        let mut sorted: Vec<_> = feed.drops.iter()
            .filter(|d| current_time - d.time < DROP_LIFETIME)
            .collect();
        sorted.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());

        // Assign slot indices: drops within 0.05s of each other are in the same group
        const GROUP_THRESHOLD: f64 = 0.05;
        let mut slot: u32 = 0;
        let mut group_start_time = f64::NEG_INFINITY;

        for drop in sorted.iter() {
            if drop.time - group_start_time > GROUP_THRESHOLD {
                group_start_time = drop.time;
                slot = 0;
            }

            let age = current_time - drop.time;
            let t = age / DROP_LIFETIME;
            let y_offset = -(t as f32 * FLOAT_DISTANCE) + slot as f32 * ROW_HEIGHT;

            let opacity = if age < FADE_START {
                1.0
            } else {
                ((DROP_LIFETIME - age) / (DROP_LIFETIME - FADE_START)) as f32
            };

            let y = start_y + y_offset;
            let text = format!("+{} XP", drop.xp_gained);
            let text_w = self.measure_text_sharp(&text, 16.0).width;
            let total_w = ICON_SIZE + 4.0 + text_w;

            let x = right_edge_x + bar_width - total_w;

            self.draw_xp_drop_icon(drop.skill_type, x, y - ICON_SIZE / 2.0 - 2.0, opacity);

            // Black outline
            let tx = x + ICON_SIZE + 4.0;
            let outline = Color::new(0.0, 0.0, 0.0, opacity);
            for &(dx, dy) in &[(-1.0_f32, 0.0_f32), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                self.draw_text_sharp(&text, tx + dx, y + dy, 16.0, outline);
            }
            self.draw_text_sharp(&text, tx, y, 16.0, Color::new(1.0, 1.0, 1.0, opacity));

            slot += 1;
        }
    }

    fn draw_xp_drop_icon(&self, skill_type: SkillType, x: f32, y: f32, opacity: f32) {
        let tint = Color::new(1.0, 1.0, 1.0, opacity);

        if skill_type == SkillType::Fishing {
            if let Some(ref tex) = self.fishing_skill_icon {
                draw_texture_ex(tex, x, y, tint, DrawTextureParams {
                    dest_size: Some(Vec2::new(ICON_SIZE, ICON_SIZE)),
                    ..Default::default()
                });
                return;
            }
        }

        if let Some(ref texture) = self.ui_icons {
            let (icon_col, icon_row) = match skill_type {
                SkillType::Hitpoints => (0, 6),
                SkillType::Combat => (2, 6),
                SkillType::Fishing => unreachable!(),
            };
            let src_x = icon_col as f32 * UI_ICON_SIZE;
            let src_y = icon_row as f32 * UI_ICON_SIZE;

            draw_texture_ex(texture, x, y, tint, DrawTextureParams {
                source: Some(Rect::new(src_x, src_y, UI_ICON_SIZE, UI_ICON_SIZE)),
                dest_size: Some(Vec2::new(ICON_SIZE, ICON_SIZE)),
                ..Default::default()
            });
            return;
        }

        // Fallback: colored letter
        let color = self.get_xp_drop_skill_color(skill_type);
        let letter = match skill_type {
            SkillType::Hitpoints => "H",
            SkillType::Combat => "C",
            SkillType::Fishing => "F",
        };
        self.draw_text_sharp(letter, x, y + 12.0, 16.0, Color::new(color.r, color.g, color.b, opacity));
    }

    fn get_xp_drop_skill_color(&self, skill_type: SkillType) -> Color {
        match skill_type {
            SkillType::Hitpoints => Color::new(0.8, 0.2, 0.2, 1.0),
            SkillType::Combat => Color::new(0.85, 0.65, 0.15, 1.0),
            SkillType::Fishing => Color::new(0.2, 0.6, 0.85, 1.0),
        }
    }
}
