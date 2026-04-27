use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;
use std::collections::HashMap;

impl Renderer {
    pub(crate) fn render_collection_log(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;
        let frame_thickness = FRAME_THICKNESS * s;

        // Panel dimensions - centered, large popup
        let panel_width = (480.0 * s).min(sw - 32.0);
        let panel_height = (360.0 * s).min(sh - 64.0);
        let panel_x = ((sw - panel_width) / 2.0).floor();
        let panel_y = ((sh - panel_height) / 2.0).floor();

        // Draw dimmed background overlay
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.4));

        // Draw panel frame
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        // ===== HEADER =====
        let header_h = 24.0 * s;
        let header_x = panel_x + frame_thickness;
        let header_y = panel_y + frame_thickness;
        let header_w = panel_width - frame_thickness * 2.0;

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
        let title = "Collection Log";
        let title_w = self.measure_text_sharp(title, 16.0).width;
        self.draw_text_sharp(
            title,
            (header_x + (header_w - title_w) / 2.0).floor(),
            (header_y + 17.0 * s).floor(),
            16.0,
            TEXT_TITLE,
        );

        // Close button (X) in top-right
        let close_text = "X";
        let close_w = self.measure_text_sharp(close_text, 16.0).width;
        let close_x = header_x + header_w - close_w - 10.0 * s;
        let close_hovered = matches!(hovered, Some(UiElementId::CollectionLogClose));
        let close_color = if close_hovered {
            Color::new(1.0, 0.3, 0.3, 1.0)
        } else {
            TEXT_DIM
        };
        self.draw_text_sharp(
            close_text,
            close_x,
            (header_y + 17.0 * s).floor(),
            16.0,
            close_color,
        );
        layout.add(
            UiElementId::CollectionLogClose,
            Rect::new(close_x - 4.0 * s, header_y, close_w + 8.0 * s, header_h),
        );

        // ===== FOOTER =====
        let footer_h = FOOTER_HEIGHT * s;
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

        let total = state.ui_state.collection_log_definitions.len();
        let got = state.ui_state.collection_log.len();
        let footer_text = format!("{} / {} Collected", got, total);
        let fw = self.measure_text_sharp(&footer_text, 16.0).width;
        self.draw_text_sharp(
            &footer_text,
            footer_x + footer_w - fw - 10.0 * s,
            footer_y + footer_h * 0.67,
            16.0,
            TEXT_DIM,
        );

        // ===== CONTENT AREA =====
        let content_x = panel_x + frame_thickness;
        let content_y = header_y + header_h + 1.0;
        let content_w = panel_width - frame_thickness * 2.0;
        let content_h = footer_y - content_y - 1.0;

        // Sidebar / grid split
        let sidebar_w = (content_w * 0.35).floor();
        let grid_x = content_x + sidebar_w;
        let grid_w = content_w - sidebar_w;

        // Vertical divider
        draw_line(
            grid_x,
            content_y,
            grid_x,
            content_y + content_h,
            1.0,
            HEADER_BORDER,
        );

        // ===== LEFT SIDEBAR =====
        self.render_collection_sidebar(
            state, hovered, layout, content_x, content_y, sidebar_w, content_h,
        );

        // ===== RIGHT GRID =====
        self.render_collection_grid(state, hovered, layout, grid_x, content_y, grid_w, content_h);
    }

    fn render_collection_sidebar(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        let s = state.ui_state.ui_scale;
        let line_height = 17.0 * s;
        let indent = 16.0 * s;
        let pad = 8.0 * s;
        let defs = &state.ui_state.collection_log_definitions;
        let obtained = &state.ui_state.collection_log;

        // Background
        draw_rectangle(x, y, w, h, SLOT_BG_EMPTY);

        // Register scroll area FIRST so category/subcategory entries (registered later) take priority in hit_test
        layout.add(
            UiElementId::CollectionLogSidebarScrollArea,
            Rect::new(x, y, w, h),
        );

        let categories = [
            ("monster_drops", "Monster Drops"),
            ("boss_rewards", "Boss Rewards"),
            ("skilling", "Skilling"),
            ("quest_rewards", "Quest Rewards"),
        ];

        // Calculate total content height for scroll clamping
        let mut total_content_h = pad;
        for (_, (key, _)) in categories.iter().enumerate() {
            total_content_h += line_height + 6.0 * s; // category header
            let is_expanded =
                state.ui_state.collection_log_selected_category.as_deref() == Some(*key);
            if is_expanded {
                let subcat_count = {
                    let mut seen = std::collections::HashSet::new();
                    for (_, src, detail) in defs.iter() {
                        if src == key {
                            seen.insert(detail.as_str());
                        }
                    }
                    seen.len()
                };
                total_content_h += subcat_count as f32 * (line_height + 2.0 * s) + 4.0 * s;
            }
        }
        total_content_h += pad;
        let max_scroll = (total_content_h - h).max(0.0);
        layout.set_max_scroll(UiElementId::CollectionLogSidebarScrollbar, max_scroll);

        // Enable scissor clipping for scrollable sidebar
        let (sw_screen, sh_screen) = (screen_width(), screen_height());
        let (vw, _vh) = virtual_screen_size();
        let scale_x = sw_screen / vw;
        let scale_y = sh_screen / _vh;

        let mut gl = unsafe { macroquad::window::get_internal_gl() };
        gl.flush();
        gl.quad_gl.scissor(Some((
            (x * scale_x) as i32,
            (y * scale_y) as i32,
            (w * scale_x) as i32,
            (h * scale_y) as i32,
        )));

        let scroll_offset = state
            .ui_state
            .collection_log_sidebar_scroll
            .clamp(0.0, max_scroll);
        let mut cur_y = y + pad - scroll_offset;
        let mut global_subcat_idx = 0usize;

        for (cat_idx, (key, label)) in categories.iter().enumerate() {
            // Category header
            let total: usize = defs.iter().filter(|(_, src, _)| src == key).count();
            let got: usize = defs
                .iter()
                .filter(|(item_id, src, _)| {
                    src == key && obtained.contains_key(&(item_id.clone(), src.clone()))
                })
                .count();

            let is_expanded =
                state.ui_state.collection_log_selected_category.as_deref() == Some(*key);
            let cat_hovered = matches!(hovered, Some(UiElementId::CollectionLogCategoryHeader(idx)) if *idx == cat_idx);

            // Draw category row
            if cat_hovered {
                draw_rectangle(
                    x + 2.0,
                    cur_y - 2.0,
                    w - 4.0,
                    line_height + 4.0,
                    SLOT_HOVER_BG,
                );
            }

            let arrow = if is_expanded { "v" } else { ">" };
            let all_done = got == total && total > 0;
            let cat_color = if all_done {
                Color::new(0.0, 0.8, 0.0, 1.0)
            } else if cat_hovered {
                TEXT_TITLE
            } else {
                TEXT_NORMAL
            };
            let header_text = format!("{} {} ({}/{})", arrow, label, got, total);
            self.draw_text_sharp(&header_text, x + pad, cur_y + 12.0 * s, 16.0, cat_color);

            let cat_rect = Rect::new(x, cur_y - 2.0, w, line_height + 4.0);
            layout.add(UiElementId::CollectionLogCategoryHeader(cat_idx), cat_rect);

            cur_y += line_height + 6.0 * s;

            // Subcategories (if expanded)
            if is_expanded {
                let mut subcats: HashMap<&str, (usize, usize)> = HashMap::new();
                for (item_id, src, detail) in defs.iter() {
                    if src == key {
                        let entry = subcats.entry(detail.as_str()).or_insert((0, 0));
                        entry.0 += 1;
                        if obtained.contains_key(&(item_id.clone(), src.clone())) {
                            entry.1 += 1;
                        }
                    }
                }
                let mut sorted_subcats: Vec<(&str, usize, usize)> = subcats
                    .into_iter()
                    .map(|(name, (total, got))| (name, total, got))
                    .collect();
                sorted_subcats.sort_by(|a, b| a.0.cmp(b.0));

                for (name, sub_total, sub_got) in &sorted_subcats {
                    let is_selected = state
                        .ui_state
                        .collection_log_selected_subcategory
                        .as_deref()
                        == Some(*name);
                    let sub_hovered = matches!(hovered, Some(UiElementId::CollectionLogSubcategoryEntry(idx)) if *idx == global_subcat_idx);

                    if is_selected {
                        draw_rectangle(
                            x + 2.0,
                            cur_y - 2.0,
                            w - 4.0,
                            line_height + 4.0,
                            SLOT_HOVER_BORDER,
                        );
                        draw_rectangle(
                            x + 3.0,
                            cur_y - 1.0,
                            w - 6.0,
                            line_height + 2.0,
                            SLOT_HOVER_BG,
                        );
                    } else if sub_hovered {
                        draw_rectangle(
                            x + 2.0,
                            cur_y - 2.0,
                            w - 4.0,
                            line_height + 4.0,
                            SLOT_HOVER_BG,
                        );
                    }

                    let sub_all_done = *sub_got == *sub_total && *sub_total > 0;
                    let sub_color = if sub_all_done {
                        Color::new(0.0, 0.8, 0.0, 1.0)
                    } else if is_selected {
                        TEXT_TITLE
                    } else {
                        TEXT_DIM
                    };
                    let display_name = state
                        .ui_state
                        .collection_log_display_names
                        .get(*name)
                        .cloned()
                        .unwrap_or_else(|| name.replace('_', " "));
                    let sub_text = format!("{} ({}/{})", display_name, sub_got, sub_total);
                    self.draw_text_sharp(
                        &sub_text,
                        x + pad + indent,
                        cur_y + 12.0 * s,
                        16.0,
                        sub_color,
                    );

                    let sub_rect = Rect::new(x, cur_y - 2.0, w, line_height + 4.0);
                    layout.add(
                        UiElementId::CollectionLogSubcategoryEntry(global_subcat_idx),
                        sub_rect,
                    );

                    cur_y += line_height + 2.0 * s;
                    global_subcat_idx += 1;
                }

                cur_y += 4.0 * s; // spacing after subcategories
            }
        }

        // Disable scissor
        let mut gl = unsafe { macroquad::window::get_internal_gl() };
        gl.flush();
        gl.quad_gl.scissor(None);
    }

    fn render_collection_grid(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        let s = state.ui_state.ui_scale;
        let pad = 8.0 * s;
        let slot_size = (INV_SLOT_SIZE * s).max(MIN_SLOT_SIZE);
        let slot_gap = 4.0 * s;
        let defs = &state.ui_state.collection_log_definitions;
        let obtained = &state.ui_state.collection_log;

        // Background
        draw_rectangle(x, y, w, h, SLOT_BG_EMPTY);

        // Register scroll area FIRST so grid items (registered later) take priority in hit_test
        layout.add(
            UiElementId::CollectionLogGridScrollArea,
            Rect::new(x, y, w, h),
        );

        let category = state.ui_state.collection_log_selected_category.as_deref();
        let subcategory = state
            .ui_state
            .collection_log_selected_subcategory
            .as_deref();

        if subcategory.is_none() || category.is_none() {
            // No subcategory selected — show placeholder
            let placeholder = "Select a category";
            let pw = self.measure_text_sharp(placeholder, 16.0).width;
            self.draw_text_sharp(placeholder, x + (w - pw) / 2.0, y + h / 2.0, 16.0, TEXT_DIM);
            return;
        }

        let category = category.unwrap();
        let subcategory = subcategory.unwrap();

        // Get items for this subcategory
        let mut items: Vec<&str> = defs
            .iter()
            .filter(|(_, src, detail)| src == category && detail == subcategory)
            .map(|(item_id, _, _)| item_id.as_str())
            .collect();
        items.sort();

        // Title - subcategory name
        let display_name = state
            .ui_state
            .collection_log_display_names
            .get(subcategory)
            .cloned()
            .unwrap_or_else(|| subcategory.replace('_', " "));
        let sub_got = items
            .iter()
            .filter(|id| obtained.contains_key(&(id.to_string(), category.to_string())))
            .count();
        let title_text = format!("{} ({}/{})", display_name, sub_got, items.len());
        self.draw_text_sharp(&title_text, x + pad, y + pad + 12.0 * s, 16.0, TEXT_TITLE);

        let grid_y_start = y + pad + 20.0 * s + pad;
        let grid_area_w = w - pad * 2.0;
        let cols = ((grid_area_w + slot_gap) / (slot_size + slot_gap)).floor() as usize;
        let cols = cols.max(1);

        // Enable scissor for grid scrolling
        let (sw_screen, sh_screen) = (screen_width(), screen_height());
        let (vw, vh) = virtual_screen_size();
        let scale_x = sw_screen / vw;
        let scale_y = sh_screen / vh;

        let grid_clip_y = grid_y_start;
        let grid_clip_h = y + h - grid_y_start;

        // Calculate total grid content height for scroll clamping
        let rows = (items.len() + cols - 1) / cols;
        let total_grid_h = rows as f32 * (slot_size + slot_gap);
        let max_grid_scroll = (total_grid_h - grid_clip_h).max(0.0);
        layout.set_max_scroll(UiElementId::CollectionLogGridScrollbar, max_grid_scroll);

        let mut gl = unsafe { macroquad::window::get_internal_gl() };
        gl.flush();
        gl.quad_gl.scissor(Some((
            (x * scale_x) as i32,
            (grid_clip_y * scale_y) as i32,
            (w * scale_x) as i32,
            (grid_clip_h * scale_y) as i32,
        )));

        let scroll = state
            .ui_state
            .collection_log_grid_scroll
            .clamp(0.0, max_grid_scroll);

        for (i, item_id) in items.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let slot_x = x + pad + col as f32 * (slot_size + slot_gap);
            let slot_y = grid_y_start + row as f32 * (slot_size + slot_gap) - scroll;

            // Skip if outside visible area
            if slot_y + slot_size < grid_clip_y || slot_y > grid_clip_y + grid_clip_h {
                continue;
            }

            let is_obtained = obtained.contains_key(&(item_id.to_string(), category.to_string()));
            let item_hovered =
                matches!(hovered, Some(UiElementId::CollectionLogGridItem(idx)) if *idx == i);

            // Draw slot background
            let slot_bg = if item_hovered {
                SLOT_HOVER_BG
            } else if is_obtained {
                SLOT_BG_FILLED
            } else {
                SLOT_BG_EMPTY
            };
            draw_rectangle(slot_x, slot_y, slot_size, slot_size, SLOT_BORDER);
            draw_rectangle(
                slot_x + 1.0,
                slot_y + 1.0,
                slot_size - 2.0,
                slot_size - 2.0,
                slot_bg,
            );

            if is_obtained {
                // Draw full-color item icon
                self.draw_item_icon(
                    item_id,
                    slot_x + 1.0,
                    slot_y + 1.0,
                    slot_size - 2.0,
                    slot_size - 2.0,
                    state,
                    false,
                );
            } else {
                // Draw greyed-out silhouette
                self.draw_item_icon_tinted(
                    item_id,
                    slot_x + 1.0,
                    slot_y + 1.0,
                    slot_size - 2.0,
                    slot_size - 2.0,
                    state,
                    Color::new(0.25, 0.25, 0.25, 0.6),
                );
            }

            // Draw hover border
            if item_hovered {
                draw_rectangle(slot_x, slot_y, slot_size, 1.0, SLOT_HOVER_BORDER);
                draw_rectangle(
                    slot_x,
                    slot_y + slot_size - 1.0,
                    slot_size,
                    1.0,
                    SLOT_HOVER_BORDER,
                );
                draw_rectangle(slot_x, slot_y, 1.0, slot_size, SLOT_HOVER_BORDER);
                draw_rectangle(
                    slot_x + slot_size - 1.0,
                    slot_y,
                    1.0,
                    slot_size,
                    SLOT_HOVER_BORDER,
                );
            }

            // Register grid item for hover
            layout.add(
                UiElementId::CollectionLogGridItem(i),
                Rect::new(slot_x, slot_y, slot_size, slot_size),
            );
        }

        // Disable scissor
        let mut gl = unsafe { macroquad::window::get_internal_gl() };
        gl.flush();
        gl.quad_gl.scissor(None);

        // Draw tooltip for hovered item
        if let Some(UiElementId::CollectionLogGridItem(idx)) = hovered {
            if let Some(item_id) = items.get(*idx) {
                let is_obtained =
                    obtained.contains_key(&(item_id.to_string(), category.to_string()));
                let display = state.item_registry.get_display_name(item_id).to_string();
                let (mx, my) = mouse_position();
                let vx = mx / scale_x;
                let vy = my / scale_y;
                let tw = self.measure_text_sharp(&display, 16.0).width;
                let tp = 6.0 * s;
                let tx = (vx + 12.0).min(vw - tw - tp * 2.0);
                let ty = vy - 24.0 * s;
                draw_rectangle(tx - tp, ty - 14.0 * s, tw + tp * 2.0, 20.0 * s, TOOLTIP_BG);
                draw_rectangle(tx - tp, ty - 14.0 * s, tw + tp * 2.0, 1.0, TOOLTIP_FRAME);
                draw_rectangle(
                    tx - tp,
                    ty - 14.0 * s + 20.0 * s - 1.0,
                    tw + tp * 2.0,
                    1.0,
                    TOOLTIP_FRAME,
                );
                draw_rectangle(tx - tp, ty - 14.0 * s, 1.0, 20.0 * s, TOOLTIP_FRAME);
                draw_rectangle(
                    tx - tp + tw + tp * 2.0 - 1.0,
                    ty - 14.0 * s,
                    1.0,
                    20.0 * s,
                    TOOLTIP_FRAME,
                );
                let text_color = if is_obtained { TEXT_NORMAL } else { TEXT_DIM };
                self.draw_text_sharp(&display, tx, ty, 16.0, text_color);
            }
        }
    }
}
