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

        let s = state.ui_state.ui_scale;
        let line_height = 18.0 * s;
        let objective_line_height = line_height - 2.0 * s;
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

            height += 8.0 * s;
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

        let frame_thickness = FRAME_THICKNESS * s;
        let panel_width = (INV_WIDTH * s).min(sw - 16.0);
        let panel_height_full = 314.0 * s;
        let button_area_height = bottom_ui_height(s);
        let min_panel_y = 4.0;
        let max_available_height = sh - button_area_height - 8.0 - min_panel_y;
        let panel_height = panel_height_full.min(max_available_height);
        let panel_x = sw - panel_width - 8.0;
        let panel_y = sh - button_area_height - panel_height - 8.0;

        let line_height = 17.0 * s;
        let entry_padding = 4.0 * s;
        let header_h = 24.0 * s;
        let footer_h = FOOTER_HEIGHT * s;

        // Draw themed panel frame with corner accents
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        // ===== HEADER SECTION =====
        let header_x = panel_x + frame_thickness;
        let header_y = panel_y + frame_thickness;
        let header_w = panel_width - frame_thickness * 2.0;

        // Header background
        draw_rectangle(header_x, header_y, header_w, header_h, HEADER_BG);

        // Header bottom separator
        draw_line(
            header_x,
            header_y + header_h,
            header_x + header_w,
            header_y + header_h,
            1.0,
            HEADER_BORDER,
        );

        // Title centered in header
        let title = "Quests";
        let title_width = self.measure_text_sharp(title, 16.0).width;
        self.draw_text_sharp(
            title,
            (header_x + (header_w - title_width) / 2.0).floor(),
            (header_y + 17.0 * s).floor(),
            16.0,
            TEXT_TITLE,
        );

        // ===== DETAIL VIEW =====
        if state.ui_state.selected_quest_id.is_some() {
            self.render_quest_detail(
                state,
                hovered,
                layout,
                panel_x,
                panel_y,
                panel_width,
                panel_height,
            );
            // Still draw footer
            let footer_x = panel_x + frame_thickness;
            let footer_y = panel_y + panel_height - frame_thickness - footer_h;
            let footer_w = panel_width - frame_thickness * 2.0;
            draw_rectangle(footer_x, footer_y, footer_w, footer_h, FOOTER_BG);
            draw_line(
                footer_x,
                footer_y,
                footer_x + footer_w,
                footer_y,
                1.0,
                HEADER_BORDER,
            );
            return;
        }

        // ===== CONTENT AREA =====
        let content_x = panel_x + frame_thickness + 8.0 * s;
        let content_y = panel_y + frame_thickness + header_h + 8.0 * s;
        let content_w = panel_width - frame_thickness * 2.0 - 16.0 * s;
        let content_h = panel_height - frame_thickness * 2.0 - header_h - footer_h - 16.0 * s;

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

        if state.ui_state.quest_catalog.is_empty() {
            // Empty state
            self.draw_text_sharp(
                "No Quests Available",
                content_x + 12.0 * s,
                content_y + 20.0 * s,
                16.0,
                TEXT_DIM,
            );
        } else {
            // ===== BUILD SORTED QUEST LIST =====
            // Determine status for each catalog entry:
            // 0 = in-progress (yellow), 1 = not started (red), 2 = completed (green)
            let mut quest_entries: Vec<(usize, &str, &str, u8)> = state
                .ui_state
                .quest_catalog
                .iter()
                .enumerate()
                .map(|(idx, entry)| {
                    let status = if state.ui_state.completed_quest_ids.contains(&entry.quest_id) {
                        2u8 // completed
                    } else if state
                        .ui_state
                        .active_quests
                        .iter()
                        .any(|q| q.id == entry.quest_id)
                    {
                        0u8 // in-progress
                    } else {
                        1u8 // not started
                    };
                    (idx, entry.name.as_str(), entry.quest_id.as_str(), status)
                })
                .collect();

            // Sort: in-progress first, not-started second, completed last; alphabetical within group
            quest_entries.sort_by(|a, b| a.3.cmp(&b.3).then_with(|| a.1.cmp(b.1)));

            // Status colors
            let color_in_progress = Color::new(1.0, 0.843, 0.0, 1.0); // gold/yellow #FFD700
            let color_not_started = Color::new(1.0, 0.267, 0.267, 1.0); // red #FF4444
            let color_completed = Color::new(0.0, 0.8, 0.0, 1.0); // green #00CC00

            // Wrap widths for text
            let text_x_offset = 12.0 * s;
            let right_pad = 8.0 * s;
            let wrap_w = content_w - text_x_offset - right_pad;

            // Calculate total content height for scrolling (with wrapping)
            let mut total_content_h = 6.0 * s; // top padding
            for (_, name, _, _) in &quest_entries {
                let lines = self.wrap_text(name, wrap_w, 16.0);
                let num_lines = lines.len().max(1);
                total_content_h += entry_padding + num_lines as f32 * line_height + entry_padding;
            }
            total_content_h += 6.0 * s; // bottom padding

            let max_scroll = (total_content_h - content_h).max(0.0);
            let scroll_offset = state.ui_state.quest_log_scroll.clamp(0.0, max_scroll);
            let needs_scroll = max_scroll > 0.0;
            layout.set_max_scroll(UiElementId::QuestLogScrollbar, max_scroll);

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

            let mut y = content_y + 6.0 * s - scroll_offset;

            for (display_idx, (_catalog_idx, name, _quest_id, status)) in
                quest_entries.iter().enumerate()
            {
                let name_lines = self.wrap_text(name, wrap_w, 16.0);
                let num_lines = name_lines.len().max(1);
                let entry_height = entry_padding + num_lines as f32 * line_height + entry_padding;
                let entry_start_y = y;

                // Skip entries fully outside visible area
                if y + entry_height < content_y || y > content_y + content_h {
                    y += entry_height;
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
                    layout.add(UiElementId::QuestLogEntry(display_idx), bounds);
                }

                // Check if this quest is hovered
                let is_hovered = matches!(
                    hovered,
                    Some(UiElementId::QuestLogEntry(idx)) if *idx == display_idx
                );

                // Draw hover background
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

                // Determine quest name color based on status
                let name_color = match status {
                    0 => color_in_progress,
                    1 => color_not_started,
                    _ => color_completed,
                };

                // Draw quest name (wrapped lines)
                for line in &name_lines {
                    self.draw_text_sharp(
                        line,
                        content_x + text_x_offset,
                        y + 12.0 * s,
                        16.0,
                        name_color,
                    );
                    y += line_height;
                }

                // Move past bottom padding
                y += entry_padding;
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

                let is_dragging = state.ui_state.quest_log_scroll_drag.dragging;
                let is_hovered = matches!(hovered, Some(UiElementId::QuestLogScrollbar));
                let thumb_alpha = if is_dragging {
                    0.5
                } else if is_hovered {
                    0.4
                } else {
                    0.3
                };

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
                    Color::new(1.0, 1.0, 1.0, thumb_alpha),
                );
                // Register scrollbar bounds for drag interaction
                layout.add_scrollbar(
                    UiElementId::QuestLogScrollbar,
                    Rect::new(scrollbar_x, track_y, scrollbar_w, track_h),
                );
            }
        }

        // ===== FOOTER SECTION =====
        let footer_x = panel_x + frame_thickness;
        let footer_y = panel_y + panel_height - frame_thickness - footer_h;
        let footer_w = panel_width - frame_thickness * 2.0;

        draw_rectangle(footer_x, footer_y, footer_w, footer_h, FOOTER_BG);
        draw_line(
            footer_x,
            footer_y,
            footer_x + footer_w,
            footer_y,
            1.0,
            HEADER_BORDER,
        );

        // Collection Log link in footer
        let clog_text = "Collection Log";
        let clog_hovered = matches!(hovered, Some(UiElementId::CollectionLogLink));
        let clog_color = if clog_hovered { TEXT_TITLE } else { TEXT_DIM };
        self.draw_text_sharp(
            clog_text,
            footer_x + 10.0 * s,
            footer_y + footer_h * 0.67,
            16.0,
            clog_color,
        );
        let clog_w = self.measure_text_sharp(clog_text, 16.0).width;
        layout.add(
            UiElementId::CollectionLogLink,
            Rect::new(footer_x + 10.0 * s, footer_y, clog_w + 4.0 * s, footer_h),
        );

        let completed_count = state.ui_state.completed_quest_ids.len();
        let total_count = state.ui_state.quest_catalog.len();
        let count_text = format!("{} / {} Complete", completed_count, total_count);
        let count_width = self.measure_text_sharp(&count_text, 16.0).width;
        self.draw_text_sharp(
            &count_text,
            footer_x + footer_w - count_width - 10.0 * s,
            footer_y + footer_h * 0.67,
            16.0,
            FRAME_MID,
        );
    }

    fn render_quest_detail(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        panel_x: f32,
        panel_y: f32,
        panel_width: f32,
        panel_height: f32,
    ) {
        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;
        let frame_thickness = FRAME_THICKNESS * s;
        let header_h = 24.0 * s;
        let footer_h = FOOTER_HEIGHT * s;
        let content_x = panel_x + frame_thickness + 8.0 * s;
        let content_y = panel_y + frame_thickness + header_h + 8.0 * s;
        let content_w = panel_width - frame_thickness * 2.0 - 16.0 * s;
        let content_h = panel_height - frame_thickness * 2.0 - header_h - footer_h - 16.0 * s;

        // Inset background
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

        // Register scroll area
        layout.add(
            UiElementId::QuestLogScrollArea,
            Rect::new(content_x, content_y, content_w, content_h),
        );

        // Find the selected quest in catalog
        let selected_id = match &state.ui_state.selected_quest_id {
            Some(id) => id,
            None => return,
        };

        let entry = match state
            .ui_state
            .quest_catalog
            .iter()
            .find(|e| e.quest_id == *selected_id)
        {
            Some(e) => e,
            None => {
                self.draw_text_sharp(
                    "Quest not found",
                    content_x + 12.0 * s,
                    content_y + 20.0 * s,
                    16.0,
                    TEXT_DIM,
                );
                return;
            }
        };

        // Determine quest status
        let is_completed = state.ui_state.completed_quest_ids.contains(selected_id);
        let is_active = state
            .ui_state
            .active_quests
            .iter()
            .any(|q| q.id == *selected_id);

        let color_in_progress = Color::new(1.0, 0.843, 0.0, 1.0);
        let color_not_started = Color::new(1.0, 0.267, 0.267, 1.0);
        let color_completed = Color::new(0.0, 0.8, 0.0, 1.0);

        let status_color = if is_completed {
            color_completed
        } else if is_active {
            color_in_progress
        } else {
            color_not_started
        };

        // Layout constants
        let line_height = 17.0 * s;
        let text_x_offset = 12.0 * s;
        let right_pad = 8.0 * s;
        let wrap_w = content_w - text_x_offset - right_pad;
        let section_gap = 10.0 * s;

        // Calculate total content height for scrolling
        let mut total_h = 6.0 * s; // top padding

        // Back button
        total_h += line_height + 4.0 * s;

        // Quest name (wrapped)
        let name_lines = self.wrap_text(&entry.name, wrap_w, 16.0);
        total_h += name_lines.len().max(1) as f32 * line_height + section_gap;

        // Separator
        total_h += 8.0 * s;

        // Description (wrapped)
        let desc_lines = self.wrap_text(&entry.description, wrap_w, 16.0);
        total_h += desc_lines.len().max(1) as f32 * line_height + section_gap;

        // Separator
        total_h += 8.0 * s;

        // Requirements section
        total_h += line_height; // "Start: NPC"
        if entry.level_required > 0 {
            total_h += line_height;
        }
        if entry.required_quest_name.is_some() {
            total_h += line_height;
        }

        // Objectives section
        if !entry.objectives.is_empty() {
            total_h += 8.0 * s; // separator
            total_h += line_height; // "Objectives:" label
            let active_quest = state
                .ui_state
                .active_quests
                .iter()
                .find(|q| q.id == *selected_id);
            for cat_obj in &entry.objectives {
                let (current, target) = if let Some(aq) = active_quest {
                    aq.objectives
                        .iter()
                        .find(|o| o.id == cat_obj.id)
                        .map(|o| (o.current, o.target))
                        .unwrap_or((0, cat_obj.target))
                } else {
                    (0, cat_obj.target)
                };
                let obj_text = format!("[+] {} ({}/{})", cat_obj.description, current, target);
                let obj_lines = self.wrap_text(&obj_text, wrap_w, 16.0);
                total_h += obj_lines.len().max(1) as f32 * line_height;
            }
        }

        total_h += 6.0 * s; // bottom padding

        let max_scroll = (total_h - content_h).max(0.0);
        let scroll_offset = state.ui_state.quest_log_scroll.clamp(0.0, max_scroll);
        let needs_scroll = max_scroll > 0.0;
        layout.set_max_scroll(UiElementId::QuestLogScrollbar, max_scroll);

        // Enable scissor clipping
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

        let mut y = content_y + 6.0 * s - scroll_offset;

        // ----- "< Back" button -----
        let back_text = "< Back";
        let back_color = if matches!(hovered, Some(UiElementId::QuestDetailBack)) {
            TEXT_TITLE
        } else {
            TEXT_DIM
        };
        self.draw_text_sharp(
            back_text,
            content_x + text_x_offset,
            y + 12.0 * s,
            16.0,
            back_color,
        );
        let back_dims = self.measure_text_sharp(back_text, 16.0);
        let back_bounds = Rect::new(
            content_x + text_x_offset,
            y,
            back_dims.width + 8.0 * s,
            line_height,
        );
        layout.add(UiElementId::QuestDetailBack, back_bounds);
        y += line_height + 4.0 * s;

        // ----- Quest name -----
        for line in &name_lines {
            self.draw_text_sharp(
                line,
                content_x + text_x_offset,
                y + 12.0 * s,
                16.0,
                status_color,
            );
            y += line_height;
        }
        y += section_gap;

        // ----- Separator -----
        draw_line(
            content_x + text_x_offset,
            y,
            content_x + content_w - right_pad,
            y,
            1.0,
            SLOT_BORDER,
        );
        y += 8.0 * s;

        // ----- Description -----
        for line in &desc_lines {
            self.draw_text_sharp(
                line,
                content_x + text_x_offset,
                y + 12.0 * s,
                16.0,
                TEXT_DIM,
            );
            y += line_height;
        }
        y += section_gap;

        // ----- Separator -----
        draw_line(
            content_x + text_x_offset,
            y,
            content_x + content_w - right_pad,
            y,
            1.0,
            SLOT_BORDER,
        );
        y += 8.0 * s;

        // ----- Requirements section -----
        // Start: NPC name
        let start_label = "Start: ";
        let start_label_w = self.measure_text_sharp(start_label, 16.0).width;
        self.draw_text_sharp(
            start_label,
            content_x + text_x_offset,
            y + 12.0 * s,
            16.0,
            TEXT_DIM,
        );
        self.draw_text_sharp(
            &entry.giver_npc_name,
            content_x + text_x_offset + start_label_w,
            y + 12.0 * s,
            16.0,
            TEXT_NORMAL,
        );
        y += line_height;

        // Level requirement
        if entry.level_required > 0 {
            let level_text = format!("Level: {}", entry.level_required);
            self.draw_text_sharp(
                &level_text,
                content_x + text_x_offset,
                y + 12.0 * s,
                16.0,
                TEXT_DIM,
            );
            y += line_height;
        }

        // Required quest
        if let Some(req_quest_name) = &entry.required_quest_name {
            let req_label = "Requires: ";
            let req_label_w = self.measure_text_sharp(req_label, 16.0).width;
            self.draw_text_sharp(
                req_label,
                content_x + text_x_offset,
                y + 12.0 * s,
                16.0,
                TEXT_DIM,
            );
            // Color based on whether the required quest is completed
            let req_color = if let Some(req_id) = &entry.required_quest_id {
                if state.ui_state.completed_quest_ids.contains(req_id) {
                    color_completed
                } else {
                    color_not_started
                }
            } else {
                TEXT_NORMAL
            };
            self.draw_text_sharp(
                req_quest_name,
                content_x + text_x_offset + req_label_w,
                y + 12.0 * s,
                16.0,
                req_color,
            );
            y += line_height;
        }

        // ----- Objectives section -----
        if !entry.objectives.is_empty() {
            // Separator
            draw_line(
                content_x + text_x_offset,
                y + 4.0 * s,
                content_x + content_w - right_pad,
                y + 4.0 * s,
                1.0,
                SLOT_BORDER,
            );
            y += 8.0 * s;

            // "Objectives:" label
            self.draw_text_sharp(
                "Objectives:",
                content_x + text_x_offset,
                y + 12.0 * s,
                16.0,
                TEXT_DIM,
            );
            y += line_height;

            let obj_complete_check = Color::new(0.392, 0.784, 0.392, 1.0);
            let obj_complete_text = Color::new(0.392, 0.627, 0.392, 1.0);
            let obj_incomplete_check = Color::new(0.502, 0.502, 0.541, 1.0);

            let active_quest = state
                .ui_state
                .active_quests
                .iter()
                .find(|q| q.id == *selected_id);

            for cat_obj in &entry.objectives {
                // Overlay progress from active quest if available
                let (current, target, completed) = if let Some(aq) = active_quest {
                    aq.objectives
                        .iter()
                        .find(|o| o.id == cat_obj.id)
                        .map(|o| (o.current, o.target, o.completed))
                        .unwrap_or((0, cat_obj.target, false))
                } else if is_completed {
                    (cat_obj.target, cat_obj.target, true)
                } else {
                    (0, cat_obj.target, false)
                };

                let check = if completed { "[+]" } else { "[ ]" };
                let check_color = if completed {
                    obj_complete_check
                } else {
                    obj_incomplete_check
                };
                let text_color = if completed {
                    obj_complete_text
                } else {
                    TEXT_DIM
                };

                let obj_text =
                    format!("{} {} ({}/{})", check, cat_obj.description, current, target);
                let obj_lines = self.wrap_text(&obj_text, wrap_w, 16.0);

                for (idx, line) in obj_lines.iter().enumerate() {
                    if idx == 0 {
                        let check_w = self.measure_text_sharp(check, 16.0).width;
                        self.draw_text_sharp(
                            check,
                            content_x + text_x_offset,
                            y + 12.0 * s,
                            16.0,
                            check_color,
                        );
                        let rest = &line[check.len()..];
                        self.draw_text_sharp(
                            rest,
                            content_x + text_x_offset + check_w,
                            y + 12.0 * s,
                            16.0,
                            text_color,
                        );
                    } else {
                        self.draw_text_sharp(
                            line,
                            content_x + text_x_offset,
                            y + 12.0 * s,
                            16.0,
                            text_color,
                        );
                    }
                    y += line_height;
                }
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
            let thumb_ratio = content_h / total_h;
            let thumb_h = (track_h * thumb_ratio).max(20.0 * s);
            let scroll_ratio = if max_scroll > 0.0 {
                scroll_offset / max_scroll
            } else {
                0.0
            };
            let thumb_y = track_y + (track_h - thumb_h) * scroll_ratio;

            let is_dragging = state.ui_state.quest_log_scroll_drag.dragging;
            let is_hovered = matches!(hovered, Some(UiElementId::QuestLogScrollbar));
            let thumb_alpha = if is_dragging {
                0.5
            } else if is_hovered {
                0.4
            } else {
                0.3
            };

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
                Color::new(1.0, 1.0, 1.0, thumb_alpha),
            );
            // Register scrollbar bounds for drag interaction
            layout.add_scrollbar(
                UiElementId::QuestLogScrollbar,
                Rect::new(scrollbar_x, track_y, scrollbar_w, track_h),
            );
        }
    }

    pub(crate) fn render_quest_tracker(
        &self,
        state: &GameState,
        _tracker_x: f32,
        tracker_y: f32,
        tracker_width: f32,
    ) -> Option<macroquad::math::Rect> {
        if state.ui_state.active_quests.is_empty() {
            return None;
        }

        // Minimized: just show "Quests (<)" right-aligned
        if state.ui_state.quest_tracker_minimized {
            let font_size = 16.0;
            let text = "Quests (<)";
            let right_anchor = _tracker_x + tracker_width;
            let text_width = self.measure_text_sharp(text, font_size).width;
            let text_x = (right_anchor - text_width).floor();
            self.draw_text_sharp(
                text,
                text_x,
                tracker_y,
                font_size,
                Color::from_rgba(255, 220, 100, 255),
            );
            // On Android, use a larger hit area for easier tapping
            let hit_pad = if cfg!(target_os = "android") {
                12.0
            } else {
                0.0
            };
            let text_dims = self.measure_text_sharp(text, font_size);
            return Some(macroquad::math::Rect::new(
                text_x - hit_pad,
                tracker_y - hit_pad,
                text_width + hit_pad * 2.0,
                text_dims.height + hit_pad * 2.0,
            ));
        }

        let s = state.ui_state.ui_scale;
        let line_height = 18.0 * s;
        let objective_line_height = line_height - 2.0 * s;
        let font_size = 16.0;

        // First pass: collect all rendered lines.
        struct TrackerLine {
            text: String,
            color: Color,
            height: f32,
            underline: bool,
        }

        let mut lines: Vec<TrackerLine> = Vec::new();

        // Header
        let header = "Quests";
        lines.push(TrackerLine {
            text: header.to_string(),
            color: Color::from_rgba(255, 220, 100, 255),
            height: line_height,
            underline: false,
        });

        // Quest content (no wrapping — measure actual widths)
        for quest in state.ui_state.active_quests.iter().take(2) {
            lines.push(TrackerLine {
                text: quest.name.clone(),
                color: WHITE,
                height: line_height,
                underline: true,
            });

            for obj in &quest.objectives {
                let status_color = if obj.completed {
                    Color::from_rgba(100, 255, 100, 255)
                } else {
                    Color::from_rgba(200, 200, 200, 255)
                };
                let check = if obj.completed { "[x]" } else { "[ ]" };
                let obj_text = format!(
                    "{} {} ({}/{})",
                    check, obj.description, obj.current, obj.target
                );
                lines.push(TrackerLine {
                    text: obj_text,
                    color: status_color,
                    height: objective_line_height,
                    underline: false,
                });
            }

            // Spacing between quests (empty line)
            lines.push(TrackerLine {
                text: String::new(),
                color: WHITE,
                height: 8.0 * s,
                underline: false,
            });
        }

        // Remove trailing spacer
        if lines.last().map(|l| l.text.is_empty()).unwrap_or(false) {
            lines.pop();
        }

        if state.ui_state.active_quests.len() > 2 {
            let more = format!(
                "...and {} more (Q to view)",
                state.ui_state.active_quests.len() - 2
            );
            lines.push(TrackerLine {
                text: more,
                color: LIGHTGRAY,
                height: line_height,
                underline: false,
            });
        }

        // Position: right-aligned to the caller's right edge
        let right_anchor = _tracker_x + tracker_width;
        let right_edge = right_anchor;

        // Draw all lines right-aligned (no background)
        let mut y = tracker_y;
        let mut min_x = right_edge;
        for line in &lines {
            if !line.text.is_empty() {
                let text_width = self.measure_text_sharp(&line.text, font_size).width;
                let text_x = (right_edge - text_width).floor();
                min_x = min_x.min(text_x);
                self.draw_text_sharp(&line.text, text_x, y, font_size, line.color);

                if line.underline {
                    let underline_y = y + 3.0 * s;
                    draw_line(
                        text_x,
                        underline_y,
                        right_edge,
                        underline_y,
                        2.0,
                        Color::new(1.0, 1.0, 1.0, 0.75),
                    );
                }
            }
            y += line.height;
        }
        Some(macroquad::math::Rect::new(
            min_x,
            tracker_y,
            right_edge - min_x,
            y - tracker_y,
        ))
    }

    /// Render resource contract tracker (left-aligned, below stat bars)
    pub(crate) fn render_resource_contract_tracker(
        &self,
        state: &GameState,
        x: f32,
        y_start: f32,
        max_width: f32,
    ) {
        let contract = match &state.resource_contract {
            Some(c) => c,
            None => return,
        };

        let line_height = 18.0;
        let objective_line_height = line_height - 2.0;
        let mut y = y_start;

        // Header
        self.draw_text_sharp("CONTRACT", x, y, 16.0, Color::from_rgba(180, 220, 130, 255));
        y += line_height;

        let title = format!("{} ({})", contract.task_text, contract.difficulty);
        for line in self.wrap_text(&title, max_width, 16.0).iter().take(3) {
            self.draw_text_sharp(line, x, y, 16.0, WHITE);
            y += line_height;
        }

        let complete = contract.amount_completed >= contract.amount_required;
        let (check, status_color) = if complete {
            ("[x]", Color::from_rgba(100, 255, 100, 255))
        } else {
            ("[ ]", Color::from_rgba(200, 200, 200, 255))
        };
        let progress_text = format!(
            "{} {}/{} {}",
            check, contract.amount_completed, contract.amount_required, contract.progress_label
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
            let return_text = format!("[ ] Return to {}", contract.giver_name);
            for line in self.wrap_text(&return_text, max_width, 16.0).iter().take(2) {
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
