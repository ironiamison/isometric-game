//! NPC dialogue panel rendering

use macroquad::prelude::*;
use macroquad::window::get_internal_gl;
use crate::game::{ActiveDialogue, GameState};
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use super::super::Renderer;
use super::common::*;

#[derive(Clone, Copy)]
struct GuideObjectiveTemplate {
    id: &'static str,
    label: &'static str,
    target: i32,
}

#[derive(Clone, Copy)]
struct GuideTierTemplate {
    id: &'static str,
    title: &'static str,
    subtitle: &'static str,
    description: &'static str,
    reward_exp: i32,
    reward_gold: i32,
    reward_items: &'static [&'static str],
    objectives: &'static [GuideObjectiveTemplate],
}

const GUIDE_T1_OBJECTIVES: [GuideObjectiveTemplate; 3] = [
    GuideObjectiveTemplate { id: "kill_crows", label: "Defeat crows", target: 8 },
    GuideObjectiveTemplate { id: "reach_combat_8", label: "Reach Combat level", target: 8 },
    GuideObjectiveTemplate { id: "gather_gold_150", label: "Accumulate gold", target: 150 },
];
const GUIDE_T2_OBJECTIVES: [GuideObjectiveTemplate; 3] = [
    GuideObjectiveTemplate { id: "kill_blue_slimes", label: "Defeat blue slimes", target: 12 },
    GuideObjectiveTemplate { id: "reach_woodcutting_5", label: "Reach Woodcutting level", target: 5 },
    GuideObjectiveTemplate { id: "gather_gold_400", label: "Accumulate gold", target: 400 },
];
const GUIDE_T3_OBJECTIVES: [GuideObjectiveTemplate; 3] = [
    GuideObjectiveTemplate { id: "kill_pigs", label: "Defeat pigs", target: 15 },
    GuideObjectiveTemplate { id: "reach_farming_8", label: "Reach Farming level", target: 8 },
    GuideObjectiveTemplate { id: "gather_gold_900", label: "Accumulate gold", target: 900 },
];

const GUIDE_T1_REWARDS: [&str; 1] = ["3x Weak Health Potion"];
const GUIDE_T2_REWARDS: [&str; 2] = ["2x Health Potion", "2x Weak Mana Potion"];
const GUIDE_T3_REWARDS: [&str; 2] = ["2x Strong Health Potion", "2x Prayer Potion"];

const GUIDE_TIERS: [GuideTierTemplate; 3] = [
    GuideTierTemplate {
        id: "adventurer_tier_1",
        title: "Getting a Grip on It",
        subtitle: "Tier I",
        description: "Build your baseline with combat, monster clears, and early money control.",
        reward_exp: 300,
        reward_gold: 120,
        reward_items: &GUIDE_T1_REWARDS,
        objectives: &GUIDE_T1_OBJECTIVES,
    },
    GuideTierTemplate {
        id: "adventurer_tier_2",
        title: "Building Consistency",
        subtitle: "Tier II",
        description: "Balance combat and gathering while keeping momentum across objectives.",
        reward_exp: 450,
        reward_gold: 220,
        reward_items: &GUIDE_T2_REWARDS,
        objectives: &GUIDE_T2_OBJECTIVES,
    },
    GuideTierTemplate {
        id: "adventurer_tier_3",
        title: "Early Mastery",
        subtitle: "Tier III",
        description: "Prove discipline across combat, farming growth, and wealth management.",
        reward_exp: 700,
        reward_gold: 400,
        reward_items: &GUIDE_T3_REWARDS,
        objectives: &GUIDE_T3_OBJECTIVES,
    },
];

fn is_adventurer_guide_dialogue(dialogue: &ActiveDialogue) -> bool {
    dialogue.speaker.eq_ignore_ascii_case("Adventurer Guide")
}

impl Renderer {
    pub(crate) fn render_dialogue(&self, state: &GameState, dialogue: &ActiveDialogue, hovered: &Option<UiElementId>, layout: &mut UiLayout, scroll_offset: f32, scrollbar_dragging: bool) {
        if is_adventurer_guide_dialogue(dialogue) {
            self.render_adventurer_guide_dialogue(state, dialogue, hovered, layout);
            return;
        }
        let (sw, sh) = virtual_screen_size();

        let is_mobile = cfg!(target_os = "android");

        // Semi-transparent overlay to focus attention
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.45));

        // Responsive width: cap at 620, with 10px margin each side
        let box_width = sw.min(620.0 + 20.0) - 20.0;

        // Mobile-aware sizing
        let (choice_btn_height, choice_spacing) = if is_mobile {
            (30.0, 38.0)
        } else {
            (26.0, 32.0)
        };

        let bottom_margin = if is_mobile { 20.0 } else { 60.0 };

        let choice_area_height = if dialogue.choices.is_empty() {
            40.0 // space for the Continue button
        } else {
            dialogue.choices.len() as f32 * choice_spacing + 36.0
        };
        let text_margin_bottom = 12.0;

        // Pre-compute text line count for dynamic height
        let text_line_count = {
            let mut count = 0u32;
            let temp_max_width = box_width - FRAME_THICKNESS * 2.0 - 24.0;
            for paragraph in dialogue.text.split('\n') {
                let words: Vec<&str> = paragraph.split_whitespace().collect();
                if words.is_empty() {
                    count += 1;
                    continue;
                }
                let mut cur = String::new();
                for word in words {
                    let test = if cur.is_empty() { word.to_string() } else { format!("{} {}", cur, word) };
                    let w = self.measure_text_sharp(&test, 16.0).width;
                    if w > temp_max_width && !cur.is_empty() {
                        count += 1;
                        cur = word.to_string();
                    } else {
                        cur = test;
                    }
                }
                if !cur.is_empty() {
                    count += 1;
                }
            }
            count.max(1)
        };
        let text_height = text_line_count as f32 * 22.0;
        let ideal_box_height = 50.0 + text_height + text_margin_bottom + choice_area_height;

        // Clamp height to screen bounds (leave 40px top margin minimum)
        let max_box_height = sh - 40.0 - bottom_margin;
        let box_height = ideal_box_height.min(max_box_height);
        let is_clamped = ideal_box_height > max_box_height;

        let box_x = (sw - box_width) / 2.0;
        let box_y = sh - box_height - bottom_margin;

        // Draw themed panel frame with corner accents
        self.draw_panel_frame(box_x, box_y, box_width, box_height);
        self.draw_corner_accents(box_x, box_y, box_width, box_height);

        // ===== CLOSE BUTTON (top-right corner) =====
        if !dialogue.choices.is_empty() {
            let close_size = if is_mobile { 32.0 } else { 24.0 };
            let close_x = box_x + box_width - close_size - FRAME_THICKNESS - 4.0;
            let close_y = box_y + FRAME_THICKNESS + 4.0;

            let bounds = Rect::new(close_x, close_y, close_size, close_size);
            layout.add(UiElementId::DialogueClose, bounds);

            let is_hovered = matches!(hovered, Some(UiElementId::DialogueClose));
            let (btn_bg, btn_border) = if is_hovered {
                (Color::new(0.4, 0.15, 0.15, 1.0), Color::new(0.6, 0.2, 0.2, 1.0))
            } else {
                (Color::new(0.2, 0.1, 0.1, 1.0), FRAME_MID)
            };

            draw_rectangle(close_x, close_y, close_size, close_size, btn_border);
            draw_rectangle(close_x + 1.0, close_y + 1.0, close_size - 2.0, close_size - 2.0, btn_bg);

            let cx = close_x + close_size / 2.0;
            let cy = close_y + close_size / 2.0;
            let cross = close_size * 0.25;
            let cross_color = if is_hovered { TEXT_TITLE } else { TEXT_DIM };
            draw_line(cx - cross, cy - cross, cx + cross, cy + cross, 2.0, cross_color);
            draw_line(cx + cross, cy - cross, cx - cross, cy + cross, 2.0, cross_color);
        }

        // ===== SPEAKER NAME TAB =====
        let speaker_text = dialogue.speaker.to_uppercase();
        let speaker_width = self.measure_text_sharp(&speaker_text, 16.0).width + 28.0;
        let speaker_x = box_x + 20.0;
        let speaker_y = box_y - 8.0;
        let speaker_h = 26.0;

        // Speaker tab with beveled effect
        draw_rectangle(speaker_x - 1.0, speaker_y - 1.0, speaker_width + 2.0, speaker_h + 2.0, FRAME_OUTER);
        draw_rectangle(speaker_x, speaker_y, speaker_width, speaker_h, HEADER_BG);
        draw_rectangle(speaker_x + 1.0, speaker_y + 1.0, speaker_width - 2.0, speaker_h - 2.0, Color::new(0.165, 0.149, 0.188, 1.0));

        // Speaker tab inner highlight
        draw_line(speaker_x + 2.0, speaker_y + 2.0, speaker_x + speaker_width - 2.0, speaker_y + 2.0, 1.0, FRAME_INNER);

        // Speaker name in gold
        self.draw_text_sharp(&speaker_text, speaker_x + 14.0, speaker_y + 18.0, 16.0, TEXT_TITLE);

        // Small decorative accent on speaker tab corners
        draw_rectangle(speaker_x, speaker_y, 3.0, 1.0, FRAME_ACCENT);
        draw_rectangle(speaker_x + speaker_width - 3.0, speaker_y, 3.0, 1.0, FRAME_ACCENT);

        // ===== DIALOGUE CONTENT AREA =====
        let content_x = box_x + FRAME_THICKNESS + 12.0;
        let content_y = box_y + FRAME_THICKNESS + 20.0;
        let content_width = box_width - FRAME_THICKNESS * 2.0 - 24.0;

        // Decorative line under speaker area (shortened when close button is present)
        let line_end = if !dialogue.choices.is_empty() {
            let close_size = if is_mobile { 32.0 } else { 24.0 };
            box_x + box_width - close_size - FRAME_THICKNESS - 4.0 - 8.0
        } else {
            content_x + content_width
        };
        draw_line(content_x, content_y, line_end, content_y, 1.0, HEADER_BORDER);

        // Dialogue text with word wrap
        let text_x = content_x;
        let text_y = content_y + 28.0;
        let max_line_width = content_width;

        let mut current_line = String::new();
        let mut line_y = text_y;

        for paragraph in dialogue.text.split('\n') {
            let words: Vec<&str> = paragraph.split_whitespace().collect();
            if words.is_empty() {
                // Empty line — advance by line height
                line_y += 22.0;
                continue;
            }
            for word in words {
                let test_line = if current_line.is_empty() {
                    word.to_string()
                } else {
                    format!("{} {}", current_line, word)
                };

                let line_width = self.measure_text_sharp(&test_line, 16.0).width;
                if line_width > max_line_width && !current_line.is_empty() {
                    self.draw_text_sharp(&current_line, text_x, line_y, 16.0, TEXT_NORMAL);
                    line_y += 22.0;
                    current_line = word.to_string();
                } else {
                    current_line = test_line;
                }
            }
            if !current_line.is_empty() {
                self.draw_text_sharp(&current_line, text_x, line_y, 16.0, TEXT_NORMAL);
                line_y += 22.0;
                current_line.clear();
            }
        }

        // ===== CHOICES / CONTINUE =====
        if dialogue.choices.is_empty() {
            let hint = "[ Continue ]";
            let hint_width = self.measure_text_sharp(hint, 16.0).width + 20.0;
            let hint_x = box_x + box_width - hint_width - FRAME_THICKNESS - 15.0;
            let hint_y = box_y + box_height - FRAME_THICKNESS - 32.0;

            let bounds = Rect::new(hint_x, hint_y, hint_width, 24.0);
            layout.add(UiElementId::DialogueContinue, bounds);

            let is_hovered = matches!(hovered, Some(UiElementId::DialogueContinue));

            let (btn_bg, btn_border) = if is_hovered {
                (Color::new(0.235, 0.204, 0.141, 1.0), FRAME_ACCENT)
            } else {
                (Color::new(0.157, 0.141, 0.110, 1.0), FRAME_MID)
            };

            draw_rectangle(hint_x, hint_y, hint_width, 24.0, btn_border);
            draw_rectangle(hint_x + 1.0, hint_y + 1.0, hint_width - 2.0, 22.0, btn_bg);

            if is_hovered {
                draw_line(hint_x + 2.0, hint_y + 2.0, hint_x + hint_width - 2.0, hint_y + 2.0, 1.0, FRAME_INNER);
            }

            let text_color = if is_hovered { TEXT_TITLE } else { TEXT_NORMAL };
            self.draw_text_sharp(hint, hint_x + 10.0, hint_y + 17.0, 16.0, text_color);

            self.draw_text_sharp("[Enter]", box_x + FRAME_THICKNESS + 15.0, hint_y + 17.0, 16.0, TEXT_DIM);
        } else {
            // ===== CHOICE BUTTONS =====
            let choice_start_y = box_y + FRAME_THICKNESS + 70.0 + text_margin_bottom;

            // Calculate visible area for choices when clamped
            let choice_area_top = choice_start_y;
            let choice_area_bottom = box_y + box_height - FRAME_THICKNESS - 20.0;
            let visible_choice_height = choice_area_bottom - choice_area_top;

            // Calculate max scroll
            let total_choice_content = dialogue.choices.len() as f32 * choice_spacing;
            let max_scroll = (total_choice_content - visible_choice_height).max(0.0);
            let needs_scroll = max_scroll > 0.0;
            let clamped_scroll = scroll_offset.clamp(0.0, max_scroll);

            // Scrollbar margin
            let scrollbar_width: f32 = if is_mobile { 20.0 } else { 14.0 };

            // Apply scissor clipping when choices overflow the visible area
            if needs_scroll {
                let physical_w = screen_width();
                let physical_h = screen_height();
                let scale_x = physical_w / sw;
                let scale_y = physical_h / sh;
                let mut gl = unsafe { get_internal_gl() };
                gl.flush();
                let clip_width = content_width - scrollbar_width - 4.0;
                gl.quad_gl.scissor(Some((
                    (content_x * scale_x) as i32,
                    (choice_area_top * scale_y) as i32,
                    (clip_width * scale_x) as i32,
                    (visible_choice_height * scale_y) as i32,
                )));
            }

            for (i, choice) in dialogue.choices.iter().enumerate() {
                let choice_y = choice_start_y + (i as f32 * choice_spacing) - clamped_scroll;

                // Skip rendering if outside visible area
                if needs_scroll && (choice_y + choice_btn_height < choice_area_top || choice_y > choice_area_bottom) {
                    continue;
                }

                let choice_width = if needs_scroll { content_width - scrollbar_width - 4.0 } else { content_width };
                let choice_x = content_x;

                let bounds = Rect::new(choice_x, choice_y, choice_width, choice_btn_height);
                layout.add(UiElementId::DialogueChoice(i), bounds);

                let is_hovered = matches!(hovered, Some(UiElementId::DialogueChoice(idx)) if *idx == i);

                let (bg_color, border_color) = if is_hovered {
                    (SLOT_HOVER_BG, SLOT_SELECTED_BORDER)
                } else {
                    (SLOT_BG_EMPTY, SLOT_BORDER)
                };

                draw_rectangle(choice_x, choice_y, choice_width, choice_btn_height, border_color);
                draw_rectangle(choice_x + 1.0, choice_y + 1.0, choice_width - 2.0, choice_btn_height - 2.0, bg_color);

                if is_hovered {
                    draw_line(choice_x + 2.0, choice_y + 2.0, choice_x + choice_width - 2.0, choice_y + 2.0, 1.0, FRAME_INNER);
                    draw_line(choice_x + 2.0, choice_y + 2.0, choice_x + 2.0, choice_y + choice_btn_height - 2.0, 1.0, FRAME_INNER);
                }

                let num_text = format!("[{}]", i + 1);
                let num_color = if is_hovered { TEXT_GOLD } else { FRAME_MID };
                self.draw_text_sharp(&num_text, choice_x + 8.0, choice_y + choice_btn_height * 0.65, 16.0, num_color);

                let text_color = if is_hovered { TEXT_TITLE } else { TEXT_NORMAL };
                self.draw_text_sharp(&choice.text, choice_x + 40.0, choice_y + choice_btn_height * 0.65, 16.0, text_color);
            }

            if needs_scroll {
                let mut gl = unsafe { get_internal_gl() };
                gl.flush();
                gl.quad_gl.scissor(None);

                // Scrollbar track and thumb
                let track_x = content_x + content_width - scrollbar_width;
                let track_y = choice_area_top;
                let track_h = visible_choice_height;

                // Register scrollbar hit area
                layout.add(UiElementId::DialogueScrollbar, Rect::new(track_x, track_y, scrollbar_width, track_h));

                // Draw track background
                draw_rectangle(track_x, track_y, scrollbar_width, track_h, Color::new(0.1, 0.09, 0.12, 0.6));

                // Draw thumb
                let thumb_ratio = visible_choice_height / total_choice_content;
                let thumb_h = (track_h * thumb_ratio).max(20.0);
                let scroll_ratio = if max_scroll > 0.0 { clamped_scroll / max_scroll } else { 0.0 };
                let thumb_y = track_y + scroll_ratio * (track_h - thumb_h);

                let thumb_color = if scrollbar_dragging {
                    FRAME_ACCENT
                } else if matches!(hovered, Some(UiElementId::DialogueScrollbar)) {
                    FRAME_MID
                } else {
                    Color::new(0.3, 0.27, 0.35, 0.8)
                };
                draw_rectangle(track_x + 2.0, thumb_y, scrollbar_width - 4.0, thumb_h, thumb_color);
            }

            let hint_y = box_y + box_height - FRAME_THICKNESS - 10.0;
            self.draw_text_sharp("[1-4] Select", content_x, hint_y, 16.0, TEXT_DIM);
            self.draw_text_sharp("[Esc] Close", content_x + content_width - 75.0, hint_y, 16.0, TEXT_DIM);
        }
    }

    fn render_adventurer_guide_dialogue(
        &self,
        state: &GameState,
        dialogue: &ActiveDialogue,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (sw, sh) = virtual_screen_size();
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));

        let panel_w = sw.min(780.0);
        let panel_h = sh.min(470.0);
        let panel_x = ((sw - panel_w) * 0.5).floor();
        let panel_y = ((sh - panel_h) * 0.5).floor();
        self.draw_panel_frame(panel_x, panel_y, panel_w, panel_h);
        self.draw_corner_accents(panel_x, panel_y, panel_w, panel_h);

        self.draw_text_sharp("ADVENTURE PATHS", panel_x + 18.0, panel_y + 28.0, 18.0, TEXT_TITLE);
        self.draw_text_sharp("Custom progression milestones", panel_x + 220.0, panel_y + 28.0, 16.0, TEXT_DIM);

        let close_x = panel_x + panel_w - 32.0;
        let close_y = panel_y + 8.0;
        let close_bounds = Rect::new(close_x, close_y, 20.0, 16.0);
        layout.add(UiElementId::DialogueClose, close_bounds);
        let close_hovered = matches!(hovered, Some(UiElementId::DialogueClose));
        draw_rectangle(close_x, close_y, 20.0, 16.0, if close_hovered { SLOT_SELECTED_BORDER } else { FRAME_MID });
        draw_rectangle(close_x + 1.0, close_y + 1.0, 18.0, 14.0, if close_hovered { SLOT_HOVER_BG } else { SLOT_BG_EMPTY });
        self.draw_text_sharp("X", close_x + 7.0, close_y + 12.0, 16.0, if close_hovered { TEXT_TITLE } else { TEXT_DIM });

        let left_w = 220.0;
        let left_x = panel_x + 12.0;
        let left_y = panel_y + 42.0;
        let left_h = panel_h - 54.0;
        draw_rectangle(left_x, left_y, left_w, left_h, SLOT_BORDER);
        draw_rectangle(left_x + 1.0, left_y + 1.0, left_w - 2.0, left_h - 2.0, SLOT_BG_EMPTY);

        let selected_idx = state.ui_state.adventurer_selected_tier.min(GUIDE_TIERS.len().saturating_sub(1));

        let mut row_y = left_y + 8.0;
        for (idx, tier) in GUIDE_TIERS.iter().enumerate() {
            let is_selected = idx == selected_idx;
            let completed = state.ui_state.completed_quest_ids.contains(tier.id);
            let is_active = state.ui_state.active_quests.iter().any(|q| q.id == tier.id);
            let unlocked = idx == 0
                || state.ui_state.completed_quest_ids.contains(GUIDE_TIERS[idx - 1].id)
                || is_active
                || completed;

            let row_h = 52.0;
            let row_bounds = Rect::new(left_x + 6.0, row_y, left_w - 12.0, row_h);
            layout.add(UiElementId::AdventurerTier(idx), row_bounds);
            let row_hovered = matches!(hovered, Some(UiElementId::AdventurerTier(i)) if *i == idx);

            let row_bg = if is_selected {
                SLOT_HOVER_BG
            } else if row_hovered {
                Color::new(0.14, 0.14, 0.20, 1.0)
            } else {
                Color::new(0.10, 0.10, 0.14, 1.0)
            };
            let row_border = if is_selected { SLOT_SELECTED_BORDER } else { SLOT_BORDER };
            draw_rectangle(row_bounds.x, row_bounds.y, row_bounds.w, row_bounds.h, row_border);
            draw_rectangle(row_bounds.x + 1.0, row_bounds.y + 1.0, row_bounds.w - 2.0, row_bounds.h - 2.0, row_bg);

            let status = if completed {
                "COMPLETED"
            } else if is_active {
                "ACTIVE"
            } else if unlocked {
                "AVAILABLE"
            } else {
                "LOCKED"
            };
            let status_color = if completed {
                Color::new(0.40, 0.80, 0.40, 1.0)
            } else if is_active {
                TEXT_GOLD
            } else if unlocked {
                TEXT_NORMAL
            } else {
                TEXT_DIM
            };
            self.draw_text_sharp(tier.subtitle, row_bounds.x + 8.0, row_bounds.y + 16.0, 15.0, TEXT_DIM);
            self.draw_text_sharp(tier.title, row_bounds.x + 8.0, row_bounds.y + 32.0, 16.0, if unlocked { TEXT_NORMAL } else { TEXT_DIM });
            self.draw_text_sharp(status, row_bounds.x + row_bounds.w - 84.0, row_bounds.y + 32.0, 14.0, status_color);

            row_y += row_h + 8.0;
        }

        let right_x = left_x + left_w + 12.0;
        let right_y = left_y;
        let right_w = panel_x + panel_w - right_x - 12.0;
        let right_h = left_h;
        draw_rectangle(right_x, right_y, right_w, right_h, SLOT_BORDER);
        draw_rectangle(right_x + 1.0, right_y + 1.0, right_w - 2.0, right_h - 2.0, Color::new(0.09, 0.09, 0.13, 1.0));

        let tier = GUIDE_TIERS[selected_idx];
        let completed = state.ui_state.completed_quest_ids.contains(tier.id);
        let active_quest = state.ui_state.active_quests.iter().find(|q| q.id == tier.id);

        self.draw_text_sharp(tier.title, right_x + 12.0, right_y + 26.0, 20.0, TEXT_TITLE);
        self.draw_text_sharp(tier.subtitle, right_x + right_w - 82.0, right_y + 26.0, 16.0, FRAME_MID);

        let mut desc_y = right_y + 48.0;
        for line in self.wrap_text(tier.description, right_w - 24.0, 16.0).iter().take(3) {
            self.draw_text_sharp(line, right_x + 12.0, desc_y, 16.0, TEXT_NORMAL);
            desc_y += 18.0;
        }
        for line in self.wrap_text(&dialogue.text, right_w - 24.0, 16.0).iter().take(2) {
            self.draw_text_sharp(line, right_x + 12.0, desc_y, 16.0, TEXT_DIM);
            desc_y += 17.0;
        }

        draw_line(right_x + 12.0, desc_y + 4.0, right_x + right_w - 12.0, desc_y + 4.0, 1.0, HEADER_BORDER);
        let mut y = desc_y + 24.0;

        self.draw_text_sharp("Objectives", right_x + 12.0, y, 16.0, FRAME_INNER);
        y += 18.0;

        let mut completed_count = 0;
        for objective in tier.objectives {
            let (mut current, mut target, mut done) = (0, objective.target, false);
            if completed {
                current = objective.target;
                done = true;
            } else if let Some(quest) = active_quest {
                if let Some(obj) = quest.objectives.iter().find(|o| o.id == objective.id) {
                    current = obj.current;
                    target = obj.target;
                    done = obj.completed;
                }
            }

            if done {
                completed_count += 1;
            }

            let line = format!(
                "{} {} ({}/{})",
                if done { "[+]" } else { "[ ]" },
                objective.label,
                current,
                target
            );
            self.draw_text_sharp(
                &line,
                right_x + 14.0,
                y,
                16.0,
                if done { Color::new(0.42, 0.82, 0.42, 1.0) } else { TEXT_NORMAL },
            );
            y += 18.0;
        }

        let progress = format!("Progress: {}/{}", completed_count, tier.objectives.len());
        self.draw_text_sharp(&progress, right_x + 12.0, y + 6.0, 16.0, TEXT_GOLD);
        y += 26.0;

        self.draw_text_sharp("Rewards", right_x + 12.0, y, 16.0, FRAME_INNER);
        y += 18.0;
        self.draw_text_sharp(
            &format!("EXP: {}   Gold: {}", tier.reward_exp, tier.reward_gold),
            right_x + 14.0,
            y,
            16.0,
            TEXT_NORMAL,
        );
        y += 18.0;
        for reward in tier.reward_items.iter().take(3) {
            self.draw_text_sharp(&format!("* {}", reward), right_x + 14.0, y, 16.0, TEXT_DIM);
            y += 16.0;
        }

        let action_base_y = right_y + right_h - 72.0;
        if dialogue.quest_id == tier.id {
            if dialogue.choices.is_empty() {
                let btn = Rect::new(right_x + right_w - 170.0, action_base_y, 150.0, 30.0);
                layout.add(UiElementId::DialogueContinue, btn);
                let hovered_continue = matches!(hovered, Some(UiElementId::DialogueContinue));
                draw_rectangle(btn.x, btn.y, btn.w, btn.h, if hovered_continue { SLOT_SELECTED_BORDER } else { FRAME_MID });
                draw_rectangle(btn.x + 1.0, btn.y + 1.0, btn.w - 2.0, btn.h - 2.0, if hovered_continue { SLOT_HOVER_BG } else { SLOT_BG_EMPTY });
                self.draw_text_sharp("Continue", btn.x + 46.0, btn.y + 20.0, 16.0, if hovered_continue { TEXT_TITLE } else { TEXT_NORMAL });
            } else {
                let mut btn_y = action_base_y - ((dialogue.choices.len().saturating_sub(1) as f32) * 34.0);
                for (i, choice) in dialogue.choices.iter().enumerate() {
                    let btn = Rect::new(right_x + right_w - 260.0, btn_y, 240.0, 30.0);
                    layout.add(UiElementId::DialogueChoice(i), btn);
                    let btn_hovered = matches!(hovered, Some(UiElementId::DialogueChoice(idx)) if *idx == i);
                    draw_rectangle(btn.x, btn.y, btn.w, btn.h, if btn_hovered { SLOT_SELECTED_BORDER } else { FRAME_MID });
                    draw_rectangle(btn.x + 1.0, btn.y + 1.0, btn.w - 2.0, btn.h - 2.0, if btn_hovered { SLOT_HOVER_BG } else { SLOT_BG_EMPTY });
                    self.draw_text_sharp(&choice.text, btn.x + 12.0, btn.y + 20.0, 16.0, if btn_hovered { TEXT_TITLE } else { TEXT_NORMAL });
                    btn_y += 34.0;
                }
            }
        } else {
            self.draw_text_sharp(
                "Select this tier in dialogue to take action.",
                right_x + right_w - 280.0,
                action_base_y + 20.0,
                16.0,
                TEXT_DIM,
            );
        }
    }
}
