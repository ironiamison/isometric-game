//! Slayer Master panel UI - task assignment and reward shop

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

const HEADER_H: f32 = 28.0;
const ROW_HEIGHT: f32 = 52.0;
const ROW_SPACING: f32 = 4.0;
const TAB_COUNT: usize = 3;

impl Renderer {
    pub(crate) fn render_slayer_panel(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        if !state.ui_state.slayer_panel_open {
            return;
        }

        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;

        let panel_width = (380.0 * s).min(sw - 16.0);
        let panel_height = (480.0 * s).min(sh - 16.0);
        let panel_x = (sw - panel_width) / 2.0;
        let panel_y = (sh - panel_height) / 2.0;

        let header_h = HEADER_H * s;
        let padding = 10.0 * s;

        // Semi-transparent overlay
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.588));

        // Draw themed panel frame
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        // ===== HEADER =====
        let header_x = panel_x + FRAME_THICKNESS;
        let header_y = panel_y + FRAME_THICKNESS;
        let header_w = panel_width - FRAME_THICKNESS * 2.0;

        draw_rectangle(header_x, header_y, header_w, header_h, HEADER_BG);
        draw_line(
            header_x,
            header_y + header_h,
            header_x + header_w,
            header_y + header_h,
            1.0,
            HEADER_BORDER,
        );

        // Title
        let master_name = state
            .ui_state
            .slayer_master_name
            .as_deref()
            .unwrap_or("Slayer Master");
        let title = format!("Slayer Master: {}", master_name);
        let title_dims = self.measure_text_sharp(&title, 16.0);
        self.draw_text_sharp(
            &title,
            header_x + (header_w - title_dims.width) / 2.0,
            header_y + header_h * 0.71,
            16.0,
            TEXT_TITLE,
        );

        // Close button
        let close_size = 20.0 * s;
        let close_x = header_x + header_w - close_size - 6.0 * s;
        let close_y = header_y + (header_h - close_size) / 2.0;
        let close_rect = Rect::new(close_x, close_y, close_size, close_size);
        layout.add(UiElementId::SlayerCloseButton, close_rect);
        let close_hovered = matches!(hovered, Some(UiElementId::SlayerCloseButton));
        let close_color = if close_hovered { TEXT_GOLD } else { TEXT_DIM };
        self.draw_text_sharp(
            "X",
            close_x + (close_size - self.measure_text_sharp("X", 16.0).width) / 2.0,
            close_y + close_size * 0.71,
            16.0,
            close_color,
        );

        // ===== CONTENT AREA =====
        let content_x = panel_x + FRAME_THICKNESS + padding;
        let content_y = header_y + header_h + 6.0 * s;
        let content_w = panel_width - FRAME_THICKNESS * 2.0 - padding * 2.0;

        // ===== POINTS DISPLAY =====
        let points_text = format!("{} Slayer Points", state.ui_state.slayer_points);
        let points_dims = self.measure_text_sharp(&points_text, 16.0);
        self.draw_text_sharp(
            &points_text,
            content_x + (content_w - points_dims.width) / 2.0,
            content_y + 14.0 * s,
            16.0,
            TEXT_GOLD,
        );

        let mut y = content_y + 26.0 * s;

        // Separator
        draw_line(
            content_x,
            y,
            content_x + content_w,
            y,
            1.0,
            HEADER_BORDER,
        );
        y += 6.0 * s;

        // ===== CURRENT TASK SECTION =====
        self.render_slayer_task_section(state, hovered, layout, content_x, y, content_w, s);

        // Calculate task section height
        let task_section_h = if state.ui_state.slayer_current_task.is_some() {
            80.0 * s
        } else {
            50.0 * s
        };
        y += task_section_h;

        // Separator
        draw_line(
            content_x,
            y,
            content_x + content_w,
            y,
            1.0,
            HEADER_BORDER,
        );
        y += 6.0 * s;

        // ===== REWARD SHOP SECTION =====
        let reward_area_h = panel_y + panel_height - FRAME_THICKNESS - y - 6.0 * s;
        self.render_slayer_reward_section(
            state, hovered, layout, content_x, y, content_w, reward_area_h, s,
        );
    }

    fn render_slayer_task_section(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        x: f32,
        y: f32,
        w: f32,
        s: f32,
    ) {
        // Section title
        self.draw_text_sharp("Current Task", x, y + 14.0 * s, 16.0, TEXT_TITLE);

        if let Some(task) = &state.ui_state.slayer_current_task {
            // Monster name and progress
            let progress_text = format!(
                "{}: {}/{}",
                task.display_name, task.kills_current, task.kills_required
            );
            self.draw_text_sharp(&progress_text, x, y + 34.0 * s, 16.0, TEXT_NORMAL);

            // XP per kill and points on completion
            let detail_text = format!(
                "{}xp/kill  |  {} pts on complete",
                task.xp_per_kill, task.points_on_complete
            );
            self.draw_text_sharp(&detail_text, x, y + 52.0 * s, 16.0, TEXT_DIM);

            // Cancel Task button
            let btn_w = 140.0 * s;
            let btn_h = 24.0 * s;
            let btn_x = x + w - btn_w;
            let btn_y = y + 36.0 * s;
            let btn_rect = Rect::new(btn_x, btn_y, btn_w, btn_h);
            layout.add(UiElementId::SlayerCancelTaskButton, btn_rect);

            let cancel_hovered = matches!(hovered, Some(UiElementId::SlayerCancelTaskButton));
            let cancel_bg = if cancel_hovered {
                Color::new(0.235, 0.141, 0.141, 1.0)
            } else {
                Color::new(0.157, 0.110, 0.110, 1.0)
            };
            let cancel_border = if cancel_hovered {
                Color::new(0.8, 0.4, 0.4, 1.0)
            } else {
                FRAME_MID
            };

            draw_rectangle(btn_x, btn_y, btn_w, btn_h, cancel_border);
            draw_rectangle(
                btn_x + 1.0,
                btn_y + 1.0,
                btn_w - 2.0,
                btn_h - 2.0,
                cancel_bg,
            );

            let cancel_text = "Cancel Task (30 pts)";
            let cancel_dims = self.measure_text_sharp(cancel_text, 16.0);
            let cancel_color = if cancel_hovered {
                TEXT_TITLE
            } else {
                TEXT_NORMAL
            };
            self.draw_text_sharp(
                cancel_text,
                btn_x + (btn_w - cancel_dims.width) / 2.0,
                btn_y + btn_h * 0.71,
                16.0,
                cancel_color,
            );
        } else {
            // No active task
            self.draw_text_sharp("No active task", x, y + 34.0 * s, 16.0, TEXT_DIM);

            // Get New Task button
            let btn_w = 120.0 * s;
            let btn_h = 24.0 * s;
            let btn_x = x + w - btn_w;
            let btn_y = y + 18.0 * s;
            let btn_rect = Rect::new(btn_x, btn_y, btn_w, btn_h);
            layout.add(UiElementId::SlayerGetTaskButton, btn_rect);

            let get_hovered = matches!(hovered, Some(UiElementId::SlayerGetTaskButton));
            let get_bg = if get_hovered {
                Color::new(0.235, 0.204, 0.141, 1.0)
            } else {
                Color::new(0.157, 0.141, 0.110, 1.0)
            };
            let get_border = if get_hovered {
                FRAME_ACCENT
            } else {
                FRAME_MID
            };

            draw_rectangle(btn_x, btn_y, btn_w, btn_h, get_border);
            draw_rectangle(
                btn_x + 1.0,
                btn_y + 1.0,
                btn_w - 2.0,
                btn_h - 2.0,
                get_bg,
            );

            let get_text = "Get New Task";
            let get_dims = self.measure_text_sharp(get_text, 16.0);
            let get_color = if get_hovered { TEXT_TITLE } else { TEXT_NORMAL };
            self.draw_text_sharp(
                get_text,
                btn_x + (btn_w - get_dims.width) / 2.0,
                btn_y + btn_h * 0.71,
                16.0,
                get_color,
            );
        }
    }

    fn render_slayer_reward_section(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        s: f32,
    ) {
        // Section title
        self.draw_text_sharp("Reward Shop", x, y + 14.0 * s, 16.0, TEXT_TITLE);

        let tab_y = y + 22.0 * s;
        let tab_h = TAB_HEIGHT * s;
        let tab_names = ["Potions", "Unlocks", "Blocks"];
        let tab_w = w / TAB_COUNT as f32;
        let current_tab = state.ui_state.slayer_reward_tab;

        // Draw tabs
        for (i, name) in tab_names.iter().enumerate() {
            let tx = x + i as f32 * tab_w;
            let tab_rect = Rect::new(tx, tab_y, tab_w, tab_h);
            layout.add(UiElementId::SlayerRewardTab(i), tab_rect);

            let is_selected = i == current_tab;
            let is_hovered = matches!(hovered, Some(UiElementId::SlayerRewardTab(idx)) if *idx == i);

            let tab_bg = if is_selected {
                HEADER_BG
            } else if is_hovered {
                Color::new(0.13, 0.12, 0.16, 1.0)
            } else {
                PANEL_BG_DARK
            };
            let tab_border = if is_selected {
                FRAME_ACCENT
            } else {
                SLOT_BORDER
            };

            // Tab background
            draw_rectangle(tx, tab_y, tab_w, tab_h, tab_border);
            draw_rectangle(tx + 1.0, tab_y + 1.0, tab_w - 2.0, tab_h - 2.0, tab_bg);

            // Active tab bottom highlight
            if is_selected {
                draw_rectangle(tx + 1.0, tab_y + tab_h - 2.0, tab_w - 2.0, 2.0, FRAME_ACCENT);
            }

            let name_dims = self.measure_text_sharp(name, 16.0);
            let name_color = if is_selected {
                TEXT_TITLE
            } else if is_hovered {
                TEXT_NORMAL
            } else {
                TEXT_DIM
            };
            self.draw_text_sharp(
                name,
                tx + (tab_w - name_dims.width) / 2.0,
                tab_y + tab_h * 0.71,
                16.0,
                name_color,
            );
        }

        // List area below tabs
        let list_y = tab_y + tab_h + 4.0 * s;
        let list_h = h - (list_y - y) - 4.0 * s;

        // Inset list background
        draw_rectangle(x, list_y, w, list_h, SLOT_BORDER);
        draw_rectangle(x + 1.0, list_y + 1.0, w - 2.0, list_h - 2.0, SLOT_BG_EMPTY);
        // Inner shadow
        draw_line(x + 2.0, list_y + 2.0, x + w - 2.0, list_y + 2.0, 1.0, SLOT_INNER_SHADOW);
        draw_line(x + 2.0, list_y + 2.0, x + 2.0, list_y + list_h - 2.0, 1.0, SLOT_INNER_SHADOW);

        // Register scroll area
        let scroll_rect = Rect::new(x, list_y, w, list_h);
        layout.add(UiElementId::SlayerScrollArea, scroll_rect);

        // Filter rewards by current tab category
        let tab_category = match current_tab {
            0 => "potions",
            1 => "unlocks",
            2 => "blocks",
            _ => "potions",
        };

        let filtered_rewards: Vec<(usize, &crate::game::slayer::SlayerRewardClientData)> = state
            .ui_state
            .slayer_rewards
            .iter()
            .enumerate()
            .filter(|(_, r)| r.category == tab_category)
            .collect();

        // Calculate scroll
        let row_h = ROW_HEIGHT * s;
        let row_gap = ROW_SPACING * s;

        // For blocks tab, add space for blocked monsters list
        let blocked_extra = if current_tab == 2 {
            let blocked_count = state.ui_state.slayer_blocked_monsters.len();
            if blocked_count > 0 {
                20.0 * s + blocked_count as f32 * (26.0 * s)
            } else {
                0.0
            }
        } else {
            0.0
        };

        let total_content_h =
            filtered_rewards.len() as f32 * (row_h + row_gap) + blocked_extra;
        let max_scroll = (total_content_h - list_h).max(0.0);
        let scroll_offset = state.ui_state.slayer_reward_scroll.clamp(0.0, max_scroll);

        // Scissor clipping for scrollable content
        let needs_scroll = max_scroll > 0.0;
        if needs_scroll {
            let physical_w = screen_width();
            let physical_h = screen_height();
            let (vsw, vsh) = virtual_screen_size();
            let scale_x = physical_w / vsw;
            let scale_y = physical_h / vsh;
            let mut gl = unsafe { macroquad::window::get_internal_gl() };
            gl.flush();
            gl.quad_gl.scissor(Some((
                ((x + 2.0) * scale_x) as i32,
                ((list_y + 2.0) * scale_y) as i32,
                ((w - 4.0) * scale_x) as i32,
                ((list_h - 4.0) * scale_y) as i32,
            )));
        }

        let mut ry = list_y + 4.0 * s - scroll_offset;

        // Draw reward items
        for (global_idx, reward) in &filtered_rewards {
            // Skip items outside visible area
            if ry + row_h < list_y || ry > list_y + list_h {
                ry += row_h + row_gap;
                continue;
            }

            let row_x = x + 6.0 * s;
            let row_w = w - 12.0 * s;

            // Row background
            let is_row_hovered =
                matches!(hovered, Some(UiElementId::SlayerBuyReward(idx)) if *idx == *global_idx);
            let row_bg = if is_row_hovered {
                SLOT_HOVER_BG
            } else {
                PANEL_BG_MID
            };
            let row_border = if is_row_hovered {
                SLOT_HOVER_BORDER
            } else {
                SLOT_BORDER
            };
            draw_rectangle(row_x, ry, row_w, row_h, row_border);
            draw_rectangle(row_x + 1.0, ry + 1.0, row_w - 2.0, row_h - 2.0, row_bg);

            // Reward name
            self.draw_text_sharp(
                &reward.display_name,
                row_x + 8.0 * s,
                ry + 18.0 * s,
                16.0,
                TEXT_NORMAL,
            );

            // Description
            self.draw_text_sharp(
                &reward.description,
                row_x + 8.0 * s,
                ry + 34.0 * s,
                16.0,
                TEXT_DIM,
            );

            // Cost and Buy button
            let cost_text = format!("{} pts", reward.cost);
            let cost_dims = self.measure_text_sharp(&cost_text, 16.0);
            let can_afford = state.ui_state.slayer_points >= reward.cost;

            let btn_w = 50.0 * s;
            let btn_h = 22.0 * s;
            let btn_x = row_x + row_w - btn_w - 6.0 * s;
            let btn_y = ry + (row_h - btn_h) / 2.0;

            // Cost text (to the left of the buy button)
            let cost_color = if can_afford { TEXT_GOLD } else { TEXT_DIM };
            self.draw_text_sharp(
                &cost_text,
                btn_x - cost_dims.width - 8.0 * s,
                ry + row_h * 0.56,
                16.0,
                cost_color,
            );

            // Register buy button only if fully visible
            if ry >= list_y - 1.0 && ry + row_h <= list_y + list_h + 1.0 {
                let buy_rect = Rect::new(btn_x, btn_y, btn_w, btn_h);
                layout.add(UiElementId::SlayerBuyReward(*global_idx), buy_rect);
            }

            let buy_hovered =
                matches!(hovered, Some(UiElementId::SlayerBuyReward(idx)) if *idx == *global_idx);
            let (buy_bg, buy_border_color) = if !can_afford {
                (
                    Color::new(0.10, 0.10, 0.12, 1.0),
                    Color::new(0.20, 0.20, 0.22, 1.0),
                )
            } else if buy_hovered {
                (
                    Color::new(0.235, 0.204, 0.141, 1.0),
                    FRAME_ACCENT,
                )
            } else {
                (
                    Color::new(0.157, 0.141, 0.110, 1.0),
                    FRAME_MID,
                )
            };

            draw_rectangle(btn_x, btn_y, btn_w, btn_h, buy_border_color);
            draw_rectangle(
                btn_x + 1.0,
                btn_y + 1.0,
                btn_w - 2.0,
                btn_h - 2.0,
                buy_bg,
            );

            let buy_text = "Buy";
            let buy_dims = self.measure_text_sharp(buy_text, 16.0);
            let buy_text_color = if !can_afford {
                TEXT_DIM
            } else if buy_hovered {
                TEXT_TITLE
            } else {
                TEXT_NORMAL
            };
            self.draw_text_sharp(
                buy_text,
                btn_x + (btn_w - buy_dims.width) / 2.0,
                btn_y + btn_h * 0.71,
                16.0,
                buy_text_color,
            );

            ry += row_h + row_gap;
        }

        // For Blocks tab: show currently blocked monsters
        if current_tab == 2 && !state.ui_state.slayer_blocked_monsters.is_empty() {
            ry += 6.0 * s;
            self.draw_text_sharp("Blocked Monsters:", x + 8.0 * s, ry + 14.0 * s, 16.0, TEXT_TITLE);
            ry += 20.0 * s;

            for (i, monster_name) in state.ui_state.slayer_blocked_monsters.iter().enumerate() {
                if ry + 24.0 * s < list_y || ry > list_y + list_h {
                    ry += 26.0 * s;
                    continue;
                }

                let row_x = x + 8.0 * s;

                // Monster name
                self.draw_text_sharp(monster_name, row_x, ry + 16.0 * s, 16.0, TEXT_NORMAL);

                // Remove button
                let remove_w = 60.0 * s;
                let remove_h = 20.0 * s;
                let remove_x = x + w - remove_w - 12.0 * s;
                let remove_y = ry + 1.0 * s;

                if ry >= list_y - 1.0 && ry + 24.0 * s <= list_y + list_h + 1.0 {
                    let remove_rect = Rect::new(remove_x, remove_y, remove_w, remove_h);
                    layout.add(UiElementId::SlayerRemoveBlock(i), remove_rect);
                }

                let remove_hovered =
                    matches!(hovered, Some(UiElementId::SlayerRemoveBlock(idx)) if *idx == i);
                let remove_bg = if remove_hovered {
                    Color::new(0.235, 0.141, 0.141, 1.0)
                } else {
                    Color::new(0.157, 0.110, 0.110, 1.0)
                };
                let remove_border = if remove_hovered {
                    Color::new(0.8, 0.4, 0.4, 1.0)
                } else {
                    FRAME_MID
                };

                draw_rectangle(remove_x, remove_y, remove_w, remove_h, remove_border);
                draw_rectangle(
                    remove_x + 1.0,
                    remove_y + 1.0,
                    remove_w - 2.0,
                    remove_h - 2.0,
                    remove_bg,
                );

                let remove_text = "Remove";
                let remove_dims = self.measure_text_sharp(remove_text, 16.0);
                let remove_color = if remove_hovered {
                    TEXT_TITLE
                } else {
                    TEXT_NORMAL
                };
                self.draw_text_sharp(
                    remove_text,
                    remove_x + (remove_w - remove_dims.width) / 2.0,
                    remove_y + remove_h * 0.71,
                    16.0,
                    remove_color,
                );

                ry += 26.0 * s;
            }
        }

        // Disable scissor clipping
        if needs_scroll {
            let mut gl = unsafe { macroquad::window::get_internal_gl() };
            gl.flush();
            gl.quad_gl.scissor(None);

            // Draw scrollbar
            let scrollbar_w = 4.0 * s;
            let scrollbar_x = x + w - scrollbar_w - 3.0 * s;
            let track_h = list_h - 4.0;
            let track_y = list_y + 2.0;
            let thumb_ratio = list_h / total_content_h;
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

        // Empty state
        if filtered_rewards.is_empty() && (current_tab != 2 || state.ui_state.slayer_blocked_monsters.is_empty()) {
            self.draw_text_sharp(
                "No rewards available",
                x + 8.0 * s,
                list_y + 20.0 * s,
                16.0,
                TEXT_DIM,
            );
        }
    }
}
