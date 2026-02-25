//! Slayer panel rendering - task display and reward shop

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

/// Reward item row height
const REWARD_ROW_HEIGHT: f32 = 44.0;
const REWARD_ROW_SPACING: f32 = 4.0;
/// Tab names for reward shop categories
const TAB_NAMES: [&str; 4] = ["Potions", "Unlocks", "Equipment", "Blocks"];
/// Tab category keys matching server data
const TAB_CATEGORIES: [&str; 4] = ["potion", "unlock", "equipment", "block"];

impl Renderer {
    /// Render a small HUD chip below the stat bars showing current slayer task
    pub(crate) fn render_slayer_task_chip(&self, state: &GameState, x: f32, y: f32) {
        let task = match &state.ui_state.slayer_current_task {
            Some(t) => t,
            None => return,
        };

        let s = state.ui_state.ui_scale;
        let sprite_area = 24.0 * s;
        let font_sz = 16.0;
        let padding = 3.0 * s;
        let count_text = format!("{}/{}", task.kills_current, task.kills_required);
        let count_dims = self.measure_text_sharp(&count_text, font_sz);
        let chip_w = (sprite_area + padding * 2.0).max(count_dims.width + padding * 2.0);
        let chip_h = padding + sprite_area + 2.0 * s + count_dims.height + padding;

        // Semi-transparent dark background
        draw_rectangle(x, y, chip_w, chip_h, Color::from_rgba(0, 0, 0, 180));
        draw_rectangle_lines(x, y, chip_w, chip_h, 1.0, Color::from_rgba(80, 70, 55, 180));

        // Draw NPC sprite (idle frame 0, down-facing)
        if let Some((npc_texture, npc_atlas_offset)) =
            self.npc_sprites.get(&task.monster_id)
        {
            let (tex_w, tex_h): (f32, f32) = self
                .npc_sprites
                .get_dimensions(&task.monster_id)
                .unwrap_or((npc_texture.width(), npc_texture.height()));
            let frame_width = tex_w / 16.0;
            let frame_height = tex_h;
            let (atlas_x, atlas_y): (f32, f32) = npc_atlas_offset.unwrap_or((0.0, 0.0));

            let source = Rect::new(atlas_x, atlas_y, frame_width, frame_height);
            let scale = (sprite_area / frame_width).min(sprite_area / frame_height);
            let draw_w = frame_width * scale;
            let draw_h = frame_height * scale;
            let draw_x = x + (chip_w - draw_w) / 2.0;
            let draw_y = y + padding + (sprite_area - draw_h) / 2.0;

            draw_texture_ex(
                npc_texture,
                draw_x,
                draw_y,
                WHITE,
                DrawTextureParams {
                    source: Some(source),
                    dest_size: Some(Vec2::new(draw_w, draw_h)),
                    ..Default::default()
                },
            );
        }

        // Kill count text centered below sprite
        self.draw_text_sharp(
            &count_text,
            (x + (chip_w - count_dims.width) / 2.0).floor(),
            (y + padding + sprite_area + 2.0 * s + count_dims.height * 0.9).floor(),
            font_sz,
            TEXT_NORMAL,
        );
    }

    /// Render hover tooltip for slayer task chip (called after other overlapping UI)
    pub(crate) fn render_slayer_task_chip_tooltip(&self, state: &GameState, x: f32, y: f32) {
        let task = match &state.ui_state.slayer_current_task {
            Some(t) => t,
            None => return,
        };

        let s = state.ui_state.ui_scale;
        let sprite_area = 24.0 * s;
        let font_sz = 16.0;
        let padding = 3.0 * s;
        let count_text = format!("{}/{}", task.kills_current, task.kills_required);
        let count_dims = self.measure_text_sharp(&count_text, font_sz);
        let chip_w = (sprite_area + padding * 2.0).max(count_dims.width + padding * 2.0);
        let chip_h = padding + sprite_area + 2.0 * s + count_dims.height + padding;

        let (raw_mx, raw_my) = mouse_position();
        let (vw, vh) = virtual_screen_size();
        let mx = raw_mx * vw / screen_width();
        let my = raw_my * vh / screen_height();

        if mx >= x && mx <= x + chip_w && my >= y && my <= y + chip_h {
            let tip_x = x + chip_w + 4.0 * s;
            let tip_y = y;
            let tip_font = 16.0;
            let line_h = 18.0 * s;
            let tip_pad = 6.0 * s;

            let lines = [
                (format!("Task: {}", task.display_name), TEXT_TITLE),
                (format!("Progress: {}/{}", task.kills_current, task.kills_required), TEXT_NORMAL),
                (format!("XP/kill: {}", task.xp_per_kill), TEXT_NORMAL),
                (format!("Points on completion: {}", task.points_on_complete), TEXT_NORMAL),
                (format!("Slayer Points: {}", state.ui_state.slayer_points), TEXT_GOLD),
                (format!("Tasks completed: {}", state.ui_state.slayer_tasks_completed), TEXT_DIM),
            ];

            let tip_w = lines.iter()
                .map(|(text, _)| self.measure_text_sharp(text, tip_font).width)
                .fold(0.0f32, f32::max) + tip_pad * 2.0;
            let tip_h = tip_pad + lines.len() as f32 * line_h + tip_pad;

            draw_rectangle(tip_x, tip_y, tip_w, tip_h, Color::from_rgba(12, 12, 18, 240));
            draw_rectangle_lines(tip_x, tip_y, tip_w, tip_h, 1.0, Color::from_rgba(80, 70, 55, 200));

            for (i, (text, color)) in lines.iter().enumerate() {
                self.draw_text_sharp(
                    text,
                    tip_x + tip_pad,
                    (tip_y + tip_pad + (i as f32 + 0.8) * line_h).floor(),
                    tip_font,
                    *color,
                );
            }
        }
    }

    pub(crate) fn render_slayer_panel(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;

        let panel_width = (500.0 * s).min(sw - 16.0);
        let panel_height = (600.0 * s).min(sh - 16.0);
        let panel_x = (sw - panel_width) / 2.0;
        let panel_y = (sh - panel_height) / 2.0;

        // Semi-transparent overlay
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.85));

        // Panel frame
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        let inner_x = panel_x + FRAME_THICKNESS;
        let inner_w = panel_width - FRAME_THICKNESS * 2.0;
        let padding = 10.0 * s;

        // ===== HEADER SECTION =====
        let header_h = HEADER_HEIGHT * s;
        let header_y = panel_y + FRAME_THICKNESS;

        draw_rectangle(inner_x, header_y, inner_w, header_h, HEADER_BG);
        draw_line(
            inner_x + 10.0 * s,
            header_y + header_h,
            inner_x + inner_w - 10.0 * s,
            header_y + header_h,
            2.0,
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
            inner_x + (inner_w - title_dims.width) / 2.0,
            header_y + header_h * 0.71,
            16.0,
            TEXT_TITLE,
        );

        // Close button (X)
        let is_mobile = cfg!(target_os = "android");
        let close_btn_size = if is_mobile { 32.0 * s } else { 28.0 * s };
        let close_btn_x = inner_x + inner_w - close_btn_size - 6.0 * s;
        let close_btn_y = header_y + (header_h - close_btn_size) / 2.0;
        let close_bounds = Rect::new(close_btn_x, close_btn_y, close_btn_size, close_btn_size);
        layout.add(UiElementId::SlayerCloseButton, close_bounds);

        let is_close_hovered = matches!(hovered, Some(UiElementId::SlayerCloseButton));
        let (close_bg, close_border) = if is_close_hovered {
            (
                Color::new(0.4, 0.15, 0.15, 1.0),
                Color::new(0.6, 0.2, 0.2, 1.0),
            )
        } else {
            (Color::new(0.2, 0.1, 0.1, 1.0), FRAME_MID)
        };
        draw_rectangle(
            close_btn_x,
            close_btn_y,
            close_btn_size,
            close_btn_size,
            close_border,
        );
        draw_rectangle(
            close_btn_x + 1.0,
            close_btn_y + 1.0,
            close_btn_size - 2.0,
            close_btn_size - 2.0,
            close_bg,
        );

        let cx = close_btn_x + close_btn_size / 2.0;
        let cy = close_btn_y + close_btn_size / 2.0;
        let cross = close_btn_size * 0.25;
        let cross_color = if is_close_hovered {
            TEXT_TITLE
        } else {
            TEXT_DIM
        };
        draw_line(cx - cross, cy - cross, cx + cross, cy + cross, 2.0, cross_color);
        draw_line(cx + cross, cy - cross, cx - cross, cy + cross, 2.0, cross_color);

        // ===== SLAYER POINTS DISPLAY =====
        let points_y = header_y + header_h + padding;
        let points_text = format!("{} Slayer Points", state.ui_state.slayer_points);
        let points_dims = self.measure_text_sharp(&points_text, 16.0);
        self.draw_text_sharp(
            &points_text,
            inner_x + (inner_w - points_dims.width) / 2.0,
            points_y + 14.0 * s,
            16.0,
            TEXT_GOLD,
        );

        // ===== CURRENT TASK SECTION =====
        let task_section_y = points_y + 24.0 * s;
        let task_section_h = 90.0 * s;

        // Section background
        draw_rectangle(
            inner_x + padding,
            task_section_y,
            inner_w - padding * 2.0,
            task_section_h,
            Color::new(0.08, 0.08, 0.10, 1.0),
        );
        draw_rectangle_lines(
            inner_x + padding,
            task_section_y,
            inner_w - padding * 2.0,
            task_section_h,
            1.0,
            SLOT_BORDER,
        );

        let task_content_x = inner_x + padding + 10.0 * s;
        let task_content_w = inner_w - padding * 2.0 - 20.0 * s;

        if let Some(ref task) = state.ui_state.slayer_current_task {
            // Task progress
            let progress_text = format!(
                "{}: {}/{} kills",
                task.display_name, task.kills_current, task.kills_required
            );
            self.draw_text_sharp(
                &progress_text,
                task_content_x,
                task_section_y + 20.0 * s,
                16.0,
                TEXT_NORMAL,
            );

            // Task details
            let details_text = format!(
                "{}xp per kill  |  {} pts on completion",
                task.xp_per_kill, task.points_on_complete
            );
            self.draw_text_sharp(
                &details_text,
                task_content_x,
                task_section_y + 40.0 * s,
                16.0,
                TEXT_DIM,
            );

            // Cancel task button (reddish, costs 30 pts)
            let cancel_w = 140.0 * s;
            let cancel_h = 28.0 * s;
            let cancel_x = task_content_x + task_content_w - cancel_w;
            let cancel_y = task_section_y + task_section_h - cancel_h - 8.0 * s;
            let cancel_bounds = Rect::new(cancel_x, cancel_y, cancel_w, cancel_h);
            layout.add(UiElementId::SlayerCancelTaskButton, cancel_bounds);

            let can_cancel = state.ui_state.slayer_points >= 30;
            let is_cancel_hovered = matches!(hovered, Some(UiElementId::SlayerCancelTaskButton));
            let (cancel_bg, cancel_border) = if !can_cancel {
                (
                    Color::new(0.1, 0.1, 0.1, 1.0),
                    Color::new(0.3, 0.3, 0.3, 1.0),
                )
            } else if is_cancel_hovered {
                (
                    Color::new(0.5, 0.15, 0.15, 1.0),
                    Color::new(0.7, 0.25, 0.25, 1.0),
                )
            } else {
                (
                    Color::new(0.35, 0.1, 0.1, 1.0),
                    Color::new(0.5, 0.18, 0.18, 1.0),
                )
            };

            draw_rectangle(cancel_x, cancel_y, cancel_w, cancel_h, cancel_border);
            draw_rectangle(
                cancel_x + 1.0,
                cancel_y + 1.0,
                cancel_w - 2.0,
                cancel_h - 2.0,
                cancel_bg,
            );

            let cancel_text = "Cancel (-30 pts)";
            let cancel_text_color = if can_cancel { WHITE } else { TEXT_DIM };
            let cancel_text_dims = self.measure_text_sharp(cancel_text, 16.0);
            self.draw_text_sharp(
                cancel_text,
                cancel_x + (cancel_w - cancel_text_dims.width) / 2.0,
                cancel_y + cancel_h * 0.71,
                16.0,
                cancel_text_color,
            );
        } else {
            // No active task
            self.draw_text_sharp(
                "No active task",
                task_content_x,
                task_section_y + 24.0 * s,
                16.0,
                TEXT_DIM,
            );

            // Get new task button (gold theme)
            let get_w = 140.0 * s;
            let get_h = 28.0 * s;
            let get_x = task_content_x + task_content_w - get_w;
            let get_y = task_section_y + task_section_h - get_h - 8.0 * s;
            let get_bounds = Rect::new(get_x, get_y, get_w, get_h);
            layout.add(UiElementId::SlayerGetTaskButton, get_bounds);

            let is_get_hovered = matches!(hovered, Some(UiElementId::SlayerGetTaskButton));
            let (get_bg, get_border) = if is_get_hovered {
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

            draw_rectangle(get_x, get_y, get_w, get_h, get_border);
            draw_rectangle(
                get_x + 1.0,
                get_y + 1.0,
                get_w - 2.0,
                get_h - 2.0,
                get_bg,
            );

            let get_text = "Get New Task";
            let get_text_color = if is_get_hovered { TEXT_TITLE } else { TEXT_NORMAL };
            let get_text_dims = self.measure_text_sharp(get_text, 16.0);
            self.draw_text_sharp(
                get_text,
                get_x + (get_w - get_text_dims.width) / 2.0,
                get_y + get_h * 0.71,
                16.0,
                get_text_color,
            );
        }

        // ===== REWARD SHOP SECTION =====
        let shop_y = task_section_y + task_section_h + padding;
        let shop_h = panel_y + panel_height - FRAME_THICKNESS - shop_y - padding;

        // Section label
        let shop_label = "Reward Shop";
        let shop_label_dims = self.measure_text_sharp(shop_label, 16.0);
        self.draw_text_sharp(
            shop_label,
            inner_x + (inner_w - shop_label_dims.width) / 2.0,
            shop_y + 14.0 * s,
            16.0,
            TEXT_TITLE,
        );

        // Tabs
        let tab_y = shop_y + 22.0 * s;
        let tab_h = TAB_HEIGHT * s;
        let tab_area_w = inner_w - padding * 2.0;
        let tab_w = tab_area_w / TAB_NAMES.len() as f32;
        let active_tab = state.ui_state.slayer_reward_tab;

        for (i, tab_name) in TAB_NAMES.iter().enumerate() {
            let tx = inner_x + padding + i as f32 * tab_w;
            let tab_bounds = Rect::new(tx, tab_y, tab_w, tab_h);
            layout.add(UiElementId::SlayerRewardTab(i), tab_bounds);

            let is_selected = i == active_tab;
            let is_tab_hovered =
                matches!(hovered, Some(UiElementId::SlayerRewardTab(idx)) if *idx == i);

            let (tab_bg, tab_border_color) = if is_selected {
                (SLOT_HOVER_BG, SLOT_SELECTED_BORDER)
            } else if is_tab_hovered {
                (Color::new(0.141, 0.141, 0.188, 1.0), SLOT_HOVER_BORDER)
            } else {
                (SLOT_BG_EMPTY, SLOT_BORDER)
            };

            draw_rectangle(tx, tab_y, tab_w, tab_h, tab_border_color);
            draw_rectangle(tx + 1.0, tab_y + 1.0, tab_w - 2.0, tab_h - 2.0, tab_bg);

            // Active tab indicator (gold bottom line)
            if is_selected {
                draw_rectangle(tx + 2.0, tab_y + tab_h - 3.0, tab_w - 4.0, 3.0, FRAME_ACCENT);
            }

            let tab_text_color = if is_selected { TEXT_TITLE } else { TEXT_NORMAL };
            let tab_text_dims = self.measure_text_sharp(tab_name, TAB_FONT_SIZE);
            self.draw_text_sharp(
                tab_name,
                tx + (tab_w - tab_text_dims.width) / 2.0,
                tab_y + tab_h * 0.68,
                TAB_FONT_SIZE,
                tab_text_color,
            );
        }

        // Content area below tabs
        let content_y = tab_y + tab_h + 6.0 * s;
        let content_h = shop_y + shop_h - content_y;
        let content_x = inner_x + padding;
        let content_w = inner_w - padding * 2.0;

        // Register scroll area
        let scroll_rect = Rect::new(content_x, content_y, content_w, content_h);
        layout.add(UiElementId::SlayerScrollArea, scroll_rect);

        // Content background
        draw_rectangle(content_x, content_y, content_w, content_h, Color::new(0.06, 0.06, 0.08, 1.0));
        draw_rectangle_lines(content_x, content_y, content_w, content_h, 1.0, SLOT_BORDER);

        // Filter rewards by active tab category
        let active_category = TAB_CATEGORIES[active_tab];
        let filtered_rewards: Vec<(usize, &crate::game::slayer::SlayerRewardClientData)> = state
            .ui_state
            .slayer_rewards
            .iter()
            .enumerate()
            .filter(|(_, r)| r.category == active_category)
            .collect();

        // Render reward rows with scroll offset
        let row_h = REWARD_ROW_HEIGHT * s;
        let row_sp = REWARD_ROW_SPACING * s;
        let scroll_offset = state.ui_state.slayer_reward_scroll;

        // Scissor clip for scroll area
        let (real_sw, real_sh) = (screen_width(), screen_height());
        let scale_x = real_sw / sw;
        let scale_y = real_sh / sh;
        let clip_x = (content_x * scale_x) as i32;
        let clip_y = (content_y * scale_y) as i32;
        let clip_w = (content_w * scale_x) as i32;
        let clip_h = (content_h * scale_y) as i32;

        unsafe {
            miniquad::gl::glEnable(miniquad::gl::GL_SCISSOR_TEST);
            miniquad::gl::glScissor(clip_x, real_sh as i32 - clip_y - clip_h, clip_w, clip_h);
        }

        if filtered_rewards.is_empty() && active_tab != 2 {
            self.draw_text_sharp(
                "No rewards available",
                content_x + 10.0 * s,
                content_y + 24.0 * s,
                16.0,
                TEXT_DIM,
            );
        } else {
            let mut row_idx = 0;

            // Render reward items
            for (global_idx, reward) in &filtered_rewards {
                let item_y = content_y + 4.0 * s + row_idx as f32 * (row_h + row_sp) - scroll_offset;

                // Skip items outside visible area (but still count them for layout)
                if item_y + row_h >= content_y && item_y <= content_y + content_h {
                    let can_afford = state.ui_state.slayer_points >= reward.cost;

                    // Row background
                    let row_bg = if row_idx % 2 == 0 {
                        Color::new(0.08, 0.08, 0.10, 0.6)
                    } else {
                        Color::new(0.06, 0.06, 0.08, 0.6)
                    };
                    draw_rectangle(content_x + 2.0, item_y, content_w - 4.0, row_h, row_bg);

                    // Reward name
                    let name_color = if can_afford { TEXT_NORMAL } else { TEXT_DIM };
                    self.draw_text_sharp(
                        &reward.display_name,
                        content_x + 10.0 * s,
                        item_y + 16.0 * s,
                        16.0,
                        name_color,
                    );

                    // Description
                    let desc_color = if can_afford {
                        Color::new(0.6, 0.6, 0.65, 1.0)
                    } else {
                        Color::new(0.35, 0.35, 0.38, 1.0)
                    };
                    self.draw_text_sharp(
                        &reward.description,
                        content_x + 10.0 * s,
                        item_y + 34.0 * s,
                        16.0,
                        desc_color,
                    );

                    // Cost (right-aligned)
                    let cost_text = format!("{} pts", reward.cost);
                    let cost_color = if can_afford { TEXT_GOLD } else { TEXT_DIM };
                    let cost_dims = self.measure_text_sharp(&cost_text, 16.0);

                    // Buy button
                    let btn_w = 50.0 * s;
                    let btn_h = 24.0 * s;
                    let btn_x = content_x + content_w - btn_w - 8.0 * s;
                    let btn_y = item_y + (row_h - btn_h) / 2.0;
                    let btn_bounds = Rect::new(btn_x, btn_y, btn_w, btn_h);
                    layout.add(UiElementId::SlayerBuyReward(*global_idx), btn_bounds);

                    let is_buy_hovered =
                        matches!(hovered, Some(UiElementId::SlayerBuyReward(idx)) if *idx == *global_idx);

                    let (btn_bg, btn_border_color) = if !can_afford {
                        (
                            Color::new(0.1, 0.1, 0.1, 1.0),
                            Color::new(0.3, 0.3, 0.3, 1.0),
                        )
                    } else if is_buy_hovered {
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

                    draw_rectangle(btn_x, btn_y, btn_w, btn_h, btn_border_color);
                    draw_rectangle(
                        btn_x + 1.0,
                        btn_y + 1.0,
                        btn_w - 2.0,
                        btn_h - 2.0,
                        btn_bg,
                    );

                    let btn_text_color = if can_afford { TEXT_NORMAL } else { TEXT_DIM };
                    let buy_text = "Buy";
                    let buy_dims = self.measure_text_sharp(buy_text, 16.0);
                    self.draw_text_sharp(
                        buy_text,
                        btn_x + (btn_w - buy_dims.width) / 2.0,
                        btn_y + btn_h * 0.71,
                        16.0,
                        btn_text_color,
                    );

                    // Cost text to the left of the buy button
                    self.draw_text_sharp(
                        &cost_text,
                        btn_x - cost_dims.width - 8.0 * s,
                        item_y + row_h * 0.55,
                        16.0,
                        cost_color,
                    );
                }

                row_idx += 1;
            }

            // For Blocks tab, also show currently blocked monsters with Remove buttons
            if active_tab == 3 {
                // Separator if there are rewards above
                if !filtered_rewards.is_empty() {
                    let sep_y = content_y + 4.0 * s + row_idx as f32 * (row_h + row_sp) - scroll_offset;
                    if sep_y >= content_y && sep_y <= content_y + content_h {
                        draw_line(
                            content_x + 10.0 * s,
                            sep_y + 4.0 * s,
                            content_x + content_w - 10.0 * s,
                            sep_y + 4.0 * s,
                            1.0,
                            HEADER_BORDER,
                        );
                    }
                    row_idx += 1;
                }

                // Blocked monsters header
                let blocked_header_y =
                    content_y + 4.0 * s + row_idx as f32 * (row_h + row_sp) - scroll_offset;
                if blocked_header_y >= content_y && blocked_header_y <= content_y + content_h {
                    let blocked_label = "Currently Blocked:";
                    self.draw_text_sharp(
                        blocked_label,
                        content_x + 10.0 * s,
                        blocked_header_y + 16.0 * s,
                        16.0,
                        TEXT_TITLE,
                    );
                }
                row_idx += 1;

                if state.ui_state.slayer_blocked_monsters.is_empty() {
                    let empty_y =
                        content_y + 4.0 * s + row_idx as f32 * (row_h + row_sp) - scroll_offset;
                    if empty_y >= content_y && empty_y <= content_y + content_h {
                        self.draw_text_sharp(
                            "No blocked monsters",
                            content_x + 10.0 * s,
                            empty_y + 16.0 * s,
                            16.0,
                            TEXT_DIM,
                        );
                    }
                } else {
                    for (i, monster_name) in
                        state.ui_state.slayer_blocked_monsters.iter().enumerate()
                    {
                        let item_y = content_y + 4.0 * s
                            + row_idx as f32 * (row_h + row_sp)
                            - scroll_offset;

                        if item_y + row_h >= content_y && item_y <= content_y + content_h {
                            // Row background
                            let row_bg = if row_idx % 2 == 0 {
                                Color::new(0.08, 0.08, 0.10, 0.6)
                            } else {
                                Color::new(0.06, 0.06, 0.08, 0.6)
                            };
                            draw_rectangle(
                                content_x + 2.0,
                                item_y,
                                content_w - 4.0,
                                row_h,
                                row_bg,
                            );

                            // Monster name
                            self.draw_text_sharp(
                                monster_name,
                                content_x + 10.0 * s,
                                item_y + row_h * 0.55,
                                16.0,
                                TEXT_NORMAL,
                            );

                            // Remove button (reddish)
                            let remove_w = 70.0 * s;
                            let remove_h = 24.0 * s;
                            let remove_x = content_x + content_w - remove_w - 8.0 * s;
                            let remove_y = item_y + (row_h - remove_h) / 2.0;
                            let remove_bounds =
                                Rect::new(remove_x, remove_y, remove_w, remove_h);
                            layout.add(UiElementId::SlayerRemoveBlock(i), remove_bounds);

                            let is_remove_hovered = matches!(
                                hovered,
                                Some(UiElementId::SlayerRemoveBlock(idx)) if *idx == i
                            );

                            let (remove_bg, remove_border) = if is_remove_hovered {
                                (
                                    Color::new(0.5, 0.15, 0.15, 1.0),
                                    Color::new(0.7, 0.25, 0.25, 1.0),
                                )
                            } else {
                                (
                                    Color::new(0.35, 0.1, 0.1, 1.0),
                                    Color::new(0.5, 0.18, 0.18, 1.0),
                                )
                            };

                            draw_rectangle(
                                remove_x,
                                remove_y,
                                remove_w,
                                remove_h,
                                remove_border,
                            );
                            draw_rectangle(
                                remove_x + 1.0,
                                remove_y + 1.0,
                                remove_w - 2.0,
                                remove_h - 2.0,
                                remove_bg,
                            );

                            let remove_text = "Remove";
                            let remove_text_color = if is_remove_hovered {
                                WHITE
                            } else {
                                TEXT_NORMAL
                            };
                            let remove_dims = self.measure_text_sharp(remove_text, 16.0);
                            self.draw_text_sharp(
                                remove_text,
                                remove_x + (remove_w - remove_dims.width) / 2.0,
                                remove_y + remove_h * 0.71,
                                16.0,
                                remove_text_color,
                            );
                        }

                        row_idx += 1;
                    }
                }
            }
        }

        // Disable scissor test
        unsafe {
            miniquad::gl::glDisable(miniquad::gl::GL_SCISSOR_TEST);
        }
    }
}
