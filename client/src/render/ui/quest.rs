//! Quest UI rendering (quest log, tracker, completion notifications)

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

impl Renderer {
    fn quest_tracker_height(&self, state: &GameState, tracker_width: f32) -> f32 {
        if state.ui_state.active_quests.is_empty() {
            return 0.0;
        }

        let line_height = 18.0;
        let objective_line_height = line_height - 2.0;
        let title_wrap_width = (tracker_width + 58.0).max(140.0);
        let detail_wrap_width = (tracker_width + 42.0).max(132.0);
        let mut height = line_height; // header

        for quest in state.ui_state.active_quests.iter().take(2) {
            let title_lines = self.wrap_text(&quest.name, title_wrap_width, 16.0);
            height += title_lines.len().max(1) as f32 * line_height;

            for obj in &quest.objectives {
                let obj_text = format!("{} ({}/{})", obj.description, obj.current, obj.target);
                let wrapped = self.wrap_text(&obj_text, detail_wrap_width, 16.0);
                height += wrapped.len().max(1) as f32 * objective_line_height;
            }

            height += 8.0;
        }

        if state.ui_state.active_quests.len() > 2 {
            let more = format!(
                "...and {} more (Q to view)",
                state.ui_state.active_quests.len() - 2
            );
            let more_lines = self.wrap_text(&more, title_wrap_width, 16.0);
            height += more_lines.len().max(1) as f32 * line_height;
        }

        height
    }

    pub(crate) fn render_quest_log(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;

        let panel_width = (380.0 * s).min(sw - 16.0);
        let panel_height = (420.0 * s).min(sh - 16.0);
        let panel_x = (sw - panel_width) / 2.0;
        let panel_y = (sh - panel_height) / 2.0;

        let line_height = 17.0 * s;
        let objective_spacing = 16.0 * s;
        let entry_padding = 6.0 * s;
        let header_h = HEADER_HEIGHT * s;
        let footer_h = FOOTER_HEIGHT * s;

        // Semi-transparent overlay
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.588));

        // Draw themed panel frame with corner accents
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        // ===== HEADER SECTION =====
        let header_x = panel_x + FRAME_THICKNESS;
        let header_y = panel_y + FRAME_THICKNESS;
        let header_w = panel_width - FRAME_THICKNESS * 2.0;

        // Header background
        draw_rectangle(header_x, header_y, header_w, header_h, HEADER_BG);

        // Header bottom separator with decorative dots
        draw_line(
            header_x + 10.0 * s,
            header_y + header_h,
            header_x + header_w - 10.0 * s,
            header_y + header_h,
            2.0,
            HEADER_BORDER,
        );

        let dot_spacing = 50.0 * s;
        let num_dots = ((header_w - 40.0 * s) / dot_spacing) as i32;
        let start_dot_x = header_x + 20.0 * s;
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
        self.draw_text_sharp(
            "QUEST LOG",
            header_x + 12.0 * s,
            header_y + header_h * 0.65,
            16.0,
            TEXT_TITLE,
        );

        // ===== CONTENT AREA =====
        let content_x = panel_x + FRAME_THICKNESS + 8.0 * s;
        let content_y = panel_y + FRAME_THICKNESS + header_h + 8.0 * s;
        let content_w = panel_width - FRAME_THICKNESS * 2.0 - 16.0 * s;
        let content_h = panel_height - FRAME_THICKNESS * 2.0 - header_h - footer_h - 16.0 * s;

        // Quest list panel with inset effect
        draw_rectangle(content_x, content_y, content_w, content_h, SLOT_BORDER);
        draw_rectangle(
            content_x + 1.0,
            content_y + 1.0,
            content_w - 2.0,
            content_h - 2.0,
            SLOT_BG_EMPTY,
        );

        // Inner shadow (top/left)
        draw_line(
            content_x + 2.0,
            content_y + 2.0,
            content_x + content_w - 2.0,
            content_y + 2.0,
            2.0,
            SLOT_INNER_SHADOW,
        );
        draw_line(
            content_x + 2.0,
            content_y + 2.0,
            content_x + 2.0,
            content_y + content_h - 2.0,
            2.0,
            SLOT_INNER_SHADOW,
        );

        // Register scroll area for mouse wheel handling
        layout.add(
            UiElementId::QuestLogScrollArea,
            Rect::new(content_x, content_y, content_w, content_h),
        );

        if state.ui_state.active_quests.is_empty() {
            let y = content_y + 10.0 * s;
            // Empty state with themed styling
            self.draw_text_sharp(
                "No Active Quests",
                content_x + 12.0 * s,
                y + 10.0 * s,
                16.0,
                TEXT_DIM,
            );
            let y = y + line_height + 8.0 * s;
            self.draw_text_sharp(
                "Talk to NPCs with",
                content_x + 12.0 * s,
                y + 10.0 * s,
                16.0,
                Color::new(0.392, 0.392, 0.431, 1.0),
            );
            self.draw_text_sharp("!", content_x + 140.0 * s, y + 10.0 * s, 16.0, TEXT_GOLD);
            self.draw_text_sharp(
                "above their heads",
                content_x + 155.0 * s,
                y + 10.0 * s,
                16.0,
                Color::new(0.392, 0.392, 0.431, 1.0),
            );
        } else {
            // Calculate total content height for scrolling
            let mut total_content_h = 10.0 * s; // top padding
            for (i, quest) in state.ui_state.active_quests.iter().enumerate() {
                total_content_h += entry_padding
                    + line_height
                    + 4.0 * s
                    + quest.objectives.len() as f32 * objective_spacing
                    + entry_padding;
                if i < state.ui_state.active_quests.len() - 1 {
                    total_content_h += 8.0 * s; // separator
                }
            }
            total_content_h += 10.0 * s; // bottom padding

            let max_scroll = (total_content_h - content_h).max(0.0);
            let scroll_offset = state.ui_state.quest_log_scroll.clamp(0.0, max_scroll);
            let needs_scroll = max_scroll > 0.0;

            // Enable scissor clipping for scrollable content
            if needs_scroll {
                let physical_w = screen_width();
                let physical_h = screen_height();
                let scale_x = physical_w / sw;
                let scale_y = physical_h / sh;
                let mut gl = unsafe { macroquad::window::get_internal_gl() };
                gl.flush();
                gl.quad_gl.scissor(Some((
                    ((content_x + 2.0) * scale_x) as i32,
                    ((content_y + 2.0) * scale_y) as i32,
                    ((content_w - 4.0) * scale_x) as i32,
                    ((content_h - 4.0) * scale_y) as i32,
                )));
            }

            let mut y = content_y + 10.0 * s - scroll_offset;

            for (quest_idx, quest) in state.ui_state.active_quests.iter().enumerate() {
                // Calculate entry height
                let title_height = line_height;
                let objectives_height = quest.objectives.len() as f32 * objective_spacing;
                let entry_height =
                    entry_padding + title_height + 4.0 * s + objectives_height + entry_padding;

                let entry_start_y = y;

                // Skip entries fully outside visible area
                if y + entry_height < content_y || y > content_y + content_h {
                    y += entry_height;
                    if quest_idx < state.ui_state.active_quests.len() - 1 {
                        y += 8.0 * s;
                    }
                    continue;
                }

                // Register quest entry bounds for hover detection (clamped to visible area)
                let vis_top = entry_start_y.max(content_y);
                let vis_bottom = (entry_start_y + entry_height).min(content_y + content_h);
                if vis_bottom > vis_top {
                    let bounds = Rect::new(
                        content_x + 4.0 * s,
                        vis_top,
                        content_w - 8.0 * s,
                        vis_bottom - vis_top,
                    );
                    layout.add(UiElementId::QuestLogEntry(quest_idx), bounds);
                }

                // Check if this quest is hovered
                let is_hovered =
                    matches!(hovered, Some(UiElementId::QuestLogEntry(idx)) if *idx == quest_idx);

                // Draw quest entry background with slot-like styling
                if is_hovered {
                    draw_rectangle(
                        content_x + 4.0 * s,
                        entry_start_y,
                        content_w - 8.0 * s,
                        entry_height,
                        SLOT_HOVER_BORDER,
                    );
                    draw_rectangle(
                        content_x + 5.0 * s,
                        entry_start_y + 1.0,
                        content_w - 10.0 * s,
                        entry_height - 2.0,
                        SLOT_HOVER_BG,
                    );
                }

                // Move y inside the entry box with padding
                y += entry_padding;

                // Quest name with star icon
                let name_color = if is_hovered { TEXT_TITLE } else { TEXT_NORMAL };
                self.draw_text_sharp("*", content_x + 12.0 * s, y + 12.0 * s, 16.0, TEXT_GOLD);
                self.draw_text_sharp(&quest.name, content_x + 28.0 * s, y + 12.0 * s, 16.0, name_color);
                y += title_height + 4.0 * s;

                // Objectives with styled checkmarks
                for obj in &quest.objectives {
                    let (check_icon, status_color) = if obj.completed {
                        ("[+]", Color::new(0.392, 0.784, 0.392, 1.0))
                    } else {
                        ("[ ]", Color::new(0.502, 0.502, 0.541, 1.0))
                    };

                    self.draw_text_sharp(
                        check_icon,
                        content_x + 20.0 * s,
                        y + 12.0 * s,
                        16.0,
                        status_color,
                    );

                    let obj_text = format!("{} ({}/{})", obj.description, obj.current, obj.target);
                    let text_color = if obj.completed {
                        Color::new(0.392, 0.627, 0.392, 1.0)
                    } else {
                        TEXT_DIM
                    };
                    self.draw_text_sharp(&obj_text, content_x + 52.0 * s, y + 12.0 * s, 16.0, text_color);
                    y += objective_spacing;
                }

                // Move past bottom padding
                y += entry_padding;

                // Decorative separator between quests
                if quest_idx < state.ui_state.active_quests.len() - 1 {
                    draw_line(
                        content_x + 20.0 * s,
                        y + 2.0 * s,
                        content_x + content_w - 20.0 * s,
                        y + 2.0 * s,
                        1.0,
                        SLOT_BORDER,
                    );
                    y += 8.0 * s;
                }
            }

            // Disable scissor clipping
            if needs_scroll {
                let mut gl = unsafe { macroquad::window::get_internal_gl() };
                gl.flush();
                gl.quad_gl.scissor(None);

                // Draw scrollbar
                let scrollbar_w = 4.0 * s;
                let scrollbar_x = content_x + content_w - scrollbar_w - 3.0 * s;
                let track_h = content_h - 4.0;
                let track_y = content_y + 2.0;
                let thumb_ratio = content_h / total_content_h;
                let thumb_h = (track_h * thumb_ratio).max(20.0 * s);
                let scroll_ratio = if max_scroll > 0.0 {
                    scroll_offset / max_scroll
                } else {
                    0.0
                };
                let thumb_y = track_y + (track_h - thumb_h) * scroll_ratio;

                // Track
                draw_rectangle(
                    scrollbar_x,
                    track_y,
                    scrollbar_w,
                    track_h,
                    Color::new(1.0, 1.0, 1.0, 0.08),
                );
                // Thumb
                draw_rectangle(
                    scrollbar_x,
                    thumb_y,
                    scrollbar_w,
                    thumb_h,
                    Color::new(1.0, 1.0, 1.0, 0.3),
                );
            }
        }

        // ===== FOOTER SECTION =====
        let footer_x = panel_x + FRAME_THICKNESS;
        let footer_y = panel_y + panel_height - FRAME_THICKNESS - footer_h;
        let footer_w = panel_width - FRAME_THICKNESS * 2.0;

        draw_rectangle(footer_x, footer_y, footer_w, footer_h, FOOTER_BG);
        draw_line(
            footer_x + 10.0 * s,
            footer_y,
            footer_x + footer_w - 10.0 * s,
            footer_y,
            1.0,
            HEADER_BORDER,
        );

        let quest_count = state.ui_state.active_quests.len();
        let count_text = format!("{} Active", quest_count);
        let count_width = self.measure_text_sharp(&count_text, 16.0).width;
        self.draw_text_sharp(
            &count_text,
            footer_x + footer_w - count_width - 10.0 * s,
            footer_y + footer_h * 0.67,
            16.0,
            FRAME_MID,
        );
    }

    pub(crate) fn render_quest_tracker(
        &self,
        state: &GameState,
        tracker_x: f32,
        tracker_y: f32,
        tracker_width: f32,
    ) {
        if state.ui_state.active_quests.is_empty() {
            return;
        }

        let line_height = 18.0;
        let objective_line_height = line_height - 2.0;
        let title_wrap_width = (tracker_width + 58.0).max(140.0);
        let detail_wrap_width = (tracker_width + 42.0).max(132.0);
        let right_edge = tracker_x + tracker_width;
        let draw_right = |renderer: &Renderer, text: &str, y: f32, color: Color| {
            let text_width = renderer.measure_text_sharp(text, 16.0).width;
            renderer.draw_text_sharp(text, (right_edge - text_width).floor(), y, 16.0, color);
        };

        let mut y = tracker_y;

        // Header
        draw_right(self, "QUESTS", y, Color::from_rgba(255, 220, 100, 255));
        y += line_height;

        // Only show first 2 active quests
        for quest in state.ui_state.active_quests.iter().take(2) {
            let title_lines = self.wrap_text(&quest.name, title_wrap_width, 16.0);
            for line in title_lines.iter().take(2) {
                draw_right(self, line, y, WHITE);
                y += line_height;
            }

            for obj in &quest.objectives {
                let status_color = if obj.completed {
                    Color::from_rgba(100, 255, 100, 255)
                } else {
                    Color::from_rgba(200, 200, 200, 255)
                };

                let check = if obj.completed { "[x]" } else { "[ ]" };
                let obj_text = format!("{} ({}/{})", obj.description, obj.current, obj.target);
                let wrapped = self.wrap_text(&obj_text, detail_wrap_width, 16.0);
                for (idx, line) in wrapped.iter().enumerate() {
                    let render_line = if idx == 0 {
                        format!("{} {}", check, line)
                    } else {
                        format!("    {}", line)
                    };
                    draw_right(self, &render_line, y, status_color);
                    y += objective_line_height;
                }
            }

            y += 8.0;
        }

        if state.ui_state.active_quests.len() > 2 {
            let more = format!(
                "...and {} more (Q to view)",
                state.ui_state.active_quests.len() - 2
            );
            for line in self.wrap_text(&more, title_wrap_width, 16.0).iter().take(2) {
                draw_right(self, line, y, LIGHTGRAY);
                y += line_height;
            }
        }
    }

    /// Render farming contract tracker (left-aligned, below stat bars)
    pub(crate) fn render_farming_contract_tracker(
        &self,
        state: &GameState,
        x: f32,
        y_start: f32,
        max_width: f32,
    ) {
        let contract = match &state.farming_contract {
            Some(c) => c,
            None => return,
        };

        let line_height = 18.0;
        let objective_line_height = line_height - 2.0;
        let mut y = y_start;

        // Header
        self.draw_text_sharp("CONTRACT", x, y, 16.0, Color::from_rgba(180, 220, 130, 255));
        y += line_height;

        // Contract info: "Easy: Harvest potatoes"
        let title = format!("{}: Harvest {}", contract.difficulty, contract.crop_name);
        for line in self.wrap_text(&title, max_width, 16.0).iter().take(2) {
            self.draw_text_sharp(line, x, y, 16.0, WHITE);
            y += line_height;
        }

        // Progress: "[x] 3/5 harvested" or "[x] 5/5 harvested" (complete)
        let complete = contract.amount_harvested >= contract.amount_required;
        let (check, status_color) = if complete {
            ("[x]", Color::from_rgba(100, 255, 100, 255))
        } else {
            ("[ ]", Color::from_rgba(200, 200, 200, 255))
        };
        let progress_text = format!(
            "{} {}/{} harvested",
            check, contract.amount_harvested, contract.amount_required
        );
        for line in self
            .wrap_text(&progress_text, max_width, 16.0)
            .iter()
            .take(2)
        {
            self.draw_text_sharp(line, x, y, 16.0, status_color);
            y += objective_line_height;
        }

        if complete {
            let return_text = "[ ] Return to Master Farmer";
            for line in self.wrap_text(return_text, max_width, 16.0).iter().take(2) {
                self.draw_text_sharp(line, x, y, 16.0, Color::from_rgba(200, 200, 200, 255));
                y += objective_line_height;
            }
        }
    }

    pub(crate) fn render_quest_completed(&self, state: &GameState) {
        let current_time = macroquad::time::get_time();
        let (sw, _sh) = virtual_screen_size();

        for event in &state.ui_state.quest_completed_events {
            let age = (current_time - event.time) as f32;
            if age > 4.0 {
                continue;
            }

            let alpha = if age > 3.0 {
                ((4.0 - age) * 255.0) as u8
            } else {
                255
            };

            let scale = if age < 0.3 {
                let t = age / 0.3;
                1.3 - 0.3 * t * t
            } else {
                1.0
            };

            let float_offset = (age * 10.0).min(30.0);
            let base_y = 120.0 - float_offset;

            if let Some(texture) = &self.quest_complete_texture {
                let tex_width = texture.width() * scale;
                let tex_height = texture.height() * scale;
                let x = (sw - tex_width) / 2.0;
                let y = base_y - tex_height / 2.0;

                draw_texture_ex(
                    texture,
                    x,
                    y,
                    Color::from_rgba(255, 255, 255, alpha),
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(tex_width, tex_height)),
                        ..Default::default()
                    },
                );

                let name_width = self.measure_text_sharp(&event.quest_name, 16.0).width;
                self.draw_text_sharp(
                    &event.quest_name,
                    (sw - name_width) / 2.0,
                    y + tex_height + 8.0,
                    16.0,
                    Color::from_rgba(255, 255, 255, alpha),
                );

                let rewards = format!("+{} EXP  +{} Gold", event.exp_reward, event.gold_reward);
                let rewards_width = self.measure_text_sharp(&rewards, 16.0).width;
                self.draw_text_sharp(
                    &rewards,
                    (sw - rewards_width) / 2.0,
                    y + tex_height + 28.0,
                    16.0,
                    Color::from_rgba(100, 255, 100, alpha),
                );
            } else {
                let title = "QUEST COMPLETE!";
                let title_width = self.measure_text_sharp(title, 32.0).width;
                let x = (sw - title_width) / 2.0;

                let outline_color = Color::from_rgba(0, 0, 0, alpha);
                for ox in [-2.0, 2.0] {
                    for oy in [-2.0, 2.0] {
                        self.draw_text_sharp(title, x + ox, base_y + oy, 32.0, outline_color);
                    }
                }

                self.draw_text_sharp(title, x, base_y, 32.0, Color::from_rgba(255, 215, 0, alpha));

                let name_width = self.measure_text_sharp(&event.quest_name, 16.0).width;
                self.draw_text_sharp(
                    &event.quest_name,
                    (sw - name_width) / 2.0,
                    base_y + 25.0,
                    16.0,
                    Color::from_rgba(255, 255, 255, alpha),
                );

                let rewards = format!("+{} EXP  +{} Gold", event.exp_reward, event.gold_reward);
                let rewards_width = self.measure_text_sharp(&rewards, 16.0).width;
                self.draw_text_sharp(
                    &rewards,
                    (sw - rewards_width) / 2.0,
                    base_y + 45.0,
                    16.0,
                    Color::from_rgba(100, 255, 100, alpha),
                );
            }
        }
    }
}
