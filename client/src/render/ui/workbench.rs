//! Workbench panel rendering — two-panel leatherworking UI

use super::super::Renderer;
use super::common::*;
use super::crafting::section_sort_key;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

// Progress bar colors (leather brown theme)
const WORKBENCH_PROGRESS_DARK: Color = Color::new(0.40, 0.28, 0.12, 1.0);
const WORKBENCH_PROGRESS_MID: Color = Color::new(0.55, 0.38, 0.18, 1.0);
const WORKBENCH_PROGRESS_LIGHT: Color = Color::new(0.70, 0.50, 0.25, 1.0);

fn workbench_section_name(section: &str) -> &str {
    match section {
        "tanning" => "Tanning",
        "armor" => "Armor",
        "miscellaneous" => "Spinning",
        _ => section,
    }
}

/// Returns which recipe sections belong to each workbench tab.
pub fn workbench_sections_for_tab(tab: u8) -> &'static [&'static str] {
    match tab {
        0 => &["tanning"],
        1 => &["armor"],
        2 => &["miscellaneous"],
        _ => &[],
    }
}

impl Renderer {
    pub(crate) fn render_workbench(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;

        let panel_width = (560.0 * s).min(sw - 16.0);
        let panel_height = (520.0 * s).min(sh - 16.0);
        let panel_x = (sw - panel_width) / 2.0;
        let panel_y = (sh - panel_height) / 2.0;

        // No background darkening — workbench is a station panel like fletching

        // Panel frame + corner accents
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        let header_h = HEADER_HEIGHT * s;
        let footer_h = FOOTER_HEIGHT * s;
        let tab_h_scaled = TAB_HEIGHT * s;

        // ===== HEADER =====
        let header_x = panel_x + FRAME_THICKNESS;
        let header_y = panel_y + FRAME_THICKNESS;
        let header_w = panel_width - FRAME_THICKNESS * 2.0;

        draw_rectangle(header_x, header_y, header_w, header_h, PANEL_BG_MID);
        draw_line(
            header_x + 10.0 * s,
            header_y + header_h,
            header_x + header_w - 10.0 * s,
            header_y + header_h,
            2.0,
            HEADER_BORDER,
        );

        // Decorative dots along header border
        let dot_spacing = 60.0 * s;
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
        let title = "WORKBENCH";
        let title_dims = self.measure_text_sharp(title, 16.0);
        self.draw_text_sharp(
            title,
            header_x + (header_w - title_dims.width) / 2.0,
            header_y + header_h * 0.65,
            16.0,
            TEXT_TITLE,
        );

        // Close button (X)
        let is_mobile = cfg!(target_os = "android");
        let close_btn_size = if is_mobile { 32.0 * s } else { 28.0 * s };
        let close_btn_x = header_x + header_w - close_btn_size - 6.0 * s;
        let close_btn_y = header_y + (header_h - close_btn_size) / 2.0;
        let close_bounds = Rect::new(close_btn_x, close_btn_y, close_btn_size, close_btn_size);
        layout.add(UiElementId::WorkbenchCloseButton, close_bounds);

        let is_close_hovered = matches!(hovered, Some(UiElementId::WorkbenchCloseButton));
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
        draw_line(
            cx - cross,
            cy - cross,
            cx + cross,
            cy + cross,
            2.0,
            cross_color,
        );
        draw_line(
            cx + cross,
            cy - cross,
            cx - cross,
            cy + cross,
            2.0,
            cross_color,
        );

        // ===== TABS =====
        let tab_y = header_y + header_h + 2.0;
        let tab_h = tab_h_scaled;
        let tab_count = 3;
        let tab_w = header_w / tab_count as f32;

        let tab_labels = ["Tanning", "Armor", "Spinning"];

        for (idx, label) in tab_labels.iter().enumerate() {
            let tx = header_x + idx as f32 * tab_w;
            let is_active = idx as u8 == state.ui_state.workbench_tab;

            let bounds = Rect::new(tx, tab_y, tab_w, tab_h);
            layout.add(UiElementId::WorkbenchTab(idx), bounds);

            let (tab_bg, tab_text_color) = if is_active {
                (PANEL_BG_MID, TEXT_TITLE)
            } else {
                (SLOT_BG_EMPTY, Color::new(0.50, 0.50, 0.55, 0.8))
            };

            draw_rectangle(tx, tab_y, tab_w, tab_h, tab_bg);

            // Border between tabs
            if idx > 0 {
                draw_line(
                    tx,
                    tab_y + 4.0 * s,
                    tx,
                    tab_y + tab_h - 4.0 * s,
                    1.0,
                    SLOT_BORDER,
                );
            }

            // Active tab bottom accent (brown for workbench)
            if is_active {
                draw_line(
                    tx + 4.0 * s,
                    tab_y + tab_h - 1.0,
                    tx + tab_w - 4.0 * s,
                    tab_y + tab_h - 1.0,
                    2.0,
                    Color::new(0.70, 0.50, 0.25, 1.0),
                );
            } else {
                draw_line(
                    tx + 4.0 * s,
                    tab_y + tab_h - 1.0,
                    tx + tab_w - 4.0 * s,
                    tab_y + tab_h - 1.0,
                    1.0,
                    SLOT_BORDER,
                );
            }

            let label_dims = self.measure_text_sharp(label, TAB_FONT_SIZE);
            self.draw_text_sharp(
                label,
                tx + (tab_w - label_dims.width) / 2.0,
                tab_y + tab_h * 0.68,
                TAB_FONT_SIZE,
                tab_text_color,
            );
        }

        // ===== SKILL INFO BAR =====
        let skill_bar_y = tab_y + tab_h + 2.0;
        let skill_bar_h = 24.0 * s;
        self.render_workbench_skill_bar(state, header_x, skill_bar_y, header_w, skill_bar_h);

        // ===== CONTENT AREA =====
        let content_x = panel_x + FRAME_THICKNESS + 8.0 * s;
        let content_y = skill_bar_y + skill_bar_h + 4.0 * s;
        let content_w = panel_width - FRAME_THICKNESS * 2.0 - 16.0 * s;
        let footer_y = panel_y + panel_height - FRAME_THICKNESS - footer_h;
        let total_content_h = footer_y - 4.0 * s - content_y;

        // If crafting is in progress, show progress overlay over full content area
        if state.ui_state.crafting_in_progress {
            self.render_workbench_progress(
                state,
                hovered,
                layout,
                content_x,
                content_y,
                content_w,
                total_content_h,
            );
        } else {
            // Compute detail panel height dynamically based on ingredient count
            let ingredient_count = {
                let tab_sections = workbench_sections_for_tab(state.ui_state.workbench_tab);
                let mut recipes: Vec<_> = state
                    .recipe_definitions
                    .iter()
                    .filter(|r| r.station.as_deref() == Some("workbench"))
                    .filter(|r| !r.requires_discovery || state.discovered_recipes.contains(&r.id))
                    .filter(|r| tab_sections.contains(&r.section.as_deref().unwrap_or("")))
                    .collect();
                recipes.sort_by(|a, b| {
                    let sa = a.section.as_deref().unwrap_or("");
                    let sb = b.section.as_deref().unwrap_or("");
                    section_sort_key(sa)
                        .cmp(&section_sort_key(sb))
                        .then(a.level_required.cmp(&b.level_required))
                });
                recipes
                    .get(state.ui_state.workbench_selected_recipe)
                    .map(|r| r.ingredients.len())
                    .unwrap_or(1)
            };
            let top_pad = 8.0 * s;
            let icon_h = 40.0 * s;
            let sep_gap = 8.0 * s;
            let ing_top = 6.0 * s;
            let ing_rows = ingredient_count as f32 * 28.0 * s;
            let ing_bottom_gap = 10.0 * s;
            let btn_h = 26.0 * s;
            let bottom_pad = 6.0 * s;
            let detail_h = (top_pad
                + icon_h
                + sep_gap
                + ing_top
                + ing_rows
                + ing_bottom_gap
                + btn_h
                + bottom_pad)
                .min(total_content_h * 0.65);
            let recipe_list_h = total_content_h - detail_h - 4.0 * s;
            let detail_y = content_y + recipe_list_h + 4.0 * s;

            self.render_workbench_recipe_list(
                state,
                hovered,
                layout,
                content_x,
                content_y,
                content_w,
                recipe_list_h,
            );

            // Divider line between panels
            draw_line(
                content_x + 10.0 * s,
                detail_y - 2.0,
                content_x + content_w - 10.0 * s,
                detail_y - 2.0,
                1.0,
                HEADER_BORDER,
            );

            self.render_workbench_crafting_detail(
                state, hovered, layout, content_x, detail_y, content_w, detail_h,
            );
        }

        // ===== FOOTER =====
        let footer_w = panel_width - FRAME_THICKNESS * 2.0;

        draw_rectangle(header_x, footer_y, footer_w, footer_h, FOOTER_BG);
        draw_line(
            header_x + 10.0 * s,
            footer_y,
            header_x + footer_w - 10.0 * s,
            footer_y,
            1.0,
            HEADER_BORDER,
        );

        if state.ui_state.crafting_in_progress {
            self.draw_text_sharp(
                "[Esc] Cancel",
                header_x + 10.0 * s,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_DIM,
            );
        } else {
            self.draw_text_sharp(
                "[W/S] Select",
                header_x + 10.0 * s,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_DIM,
            );
            self.draw_text_sharp(
                "[+/-] Qty",
                header_x + 125.0 * s,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_DIM,
            );
            self.draw_text_sharp(
                "[Tab] Tab",
                header_x + 230.0 * s,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_DIM,
            );
            self.draw_text_sharp(
                "[Enter] Craft",
                header_x + 330.0 * s,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_DIM,
            );
        }
    }

    /// Render the skill info bar: survivalist level, XP bar, recipe count
    fn render_workbench_skill_bar(&self, state: &GameState, x: f32, y: f32, w: f32, h: f32) {
        let s = state.ui_state.ui_scale;

        draw_rectangle(x, y, w, h, Color::new(0.10, 0.08, 0.06, 1.0));

        let (skill_level, xp_progress, xp_current, xp_needed) =
            if let Some(player) = state.get_local_player() {
                let skill = &player.skills.survivalist;
                let progress = skill.level_progress();
                let current_in_level = if skill.level >= 99 {
                    0
                } else {
                    let total_for_level = crate::game::skills::total_xp_for_level(skill.level);
                    let xp_in_level = skill.xp - total_for_level;
                    xp_in_level.max(0) as u64
                };
                let total_for_next = if skill.level >= 99 {
                    0
                } else {
                    let total_for_level = crate::game::skills::total_xp_for_level(skill.level);
                    let total_for_next = crate::game::skills::total_xp_for_level(skill.level + 1);
                    (total_for_next - total_for_level).max(0) as u64
                };
                (skill.level, progress, current_in_level, total_for_next)
            } else {
                (1, 0.0, 0u64, 0u64)
            };

        // Survivalist icon + level on left
        let icon_size = 16.0 * s;
        let left_x = x + 8.0 * s;
        if let Some(ref texture) = self.ui_icons {
            let src_x = 1.0 * 24.0; // Survivalist = col 1
            let src_y = 5.0 * 24.0; // row 5
            draw_texture_ex(
                texture,
                left_x,
                y + (h - icon_size) / 2.0,
                WHITE,
                DrawTextureParams {
                    source: Some(Rect::new(src_x, src_y, 24.0, 24.0)),
                    dest_size: Some(Vec2::new(icon_size, icon_size)),
                    ..Default::default()
                },
            );
        }
        let level_text = format!("Lv{}", skill_level);
        self.draw_text_sharp(
            &level_text,
            left_x + icon_size + 4.0 * s,
            y + h * 0.70,
            16.0,
            Color::new(0.70, 0.50, 0.25, 1.0),
        );

        // XP progress bar in center
        let bar_x = x + 100.0 * s;
        let bar_w = w - 260.0 * s;
        let bar_h = 12.0 * s;
        let bar_y = y + (h - bar_h) / 2.0;

        draw_rectangle(bar_x, bar_y, bar_w, bar_h, SLOT_BORDER);
        draw_rectangle(
            bar_x + 1.0,
            bar_y + 1.0,
            bar_w - 2.0,
            bar_h - 2.0,
            SLOT_BG_EMPTY,
        );

        let fill_w = (bar_w - 4.0) * xp_progress;
        if fill_w > 0.0 {
            draw_rectangle(
                bar_x + 2.0,
                bar_y + 2.0,
                fill_w,
                bar_h - 4.0,
                WORKBENCH_PROGRESS_DARK,
            );
            draw_rectangle(
                bar_x + 2.0,
                bar_y + 2.0,
                fill_w,
                (bar_h - 4.0) / 2.0,
                WORKBENCH_PROGRESS_MID,
            );
        }

        let xp_text = format!("{}/{}", xp_current, xp_needed);
        let xp_dims = self.measure_text_sharp(&xp_text, 16.0);
        self.draw_text_sharp(
            &xp_text,
            bar_x + (bar_w - xp_dims.width) / 2.0,
            y + h * 0.70,
            16.0,
            TEXT_DIM,
        );

        // Recipe count on right
        let all_workbench: Vec<_> = state
            .recipe_definitions
            .iter()
            .filter(|r| r.station.as_deref() == Some("workbench"))
            .collect();
        let unlocked = all_workbench
            .iter()
            .filter(|r| !r.requires_discovery || state.discovered_recipes.contains(&r.id))
            .count();
        let total = all_workbench.len();
        let recipe_text = format!("Recipes: {}/{}", unlocked, total);
        let recipe_dims = self.measure_text_sharp(&recipe_text, 16.0);
        self.draw_text_sharp(
            &recipe_text,
            x + w - recipe_dims.width - 8.0 * s,
            y + h * 0.70,
            16.0,
            TEXT_DIM,
        );
    }

    /// Render the scrollable recipe list with section headers
    fn render_workbench_recipe_list(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        content_x: f32,
        content_y: f32,
        content_w: f32,
        content_h: f32,
    ) {
        let s = self.font_scale.get();

        // Get workbench recipes filtered by active tab
        let tab_sections = workbench_sections_for_tab(state.ui_state.workbench_tab);
        let mut workbench_recipes: Vec<_> = state
            .recipe_definitions
            .iter()
            .filter(|r| r.station.as_deref() == Some("workbench"))
            .filter(|r| !r.requires_discovery || state.discovered_recipes.contains(&r.id))
            .filter(|r| tab_sections.contains(&r.section.as_deref().unwrap_or("")))
            .collect();

        // Sort by section then level
        workbench_recipes.sort_by(|a, b| {
            let sa = a.section.as_deref().unwrap_or("");
            let sb = b.section.as_deref().unwrap_or("");
            section_sort_key(sa)
                .cmp(&section_sort_key(sb))
                .then(a.level_required.cmp(&b.level_required))
        });

        if workbench_recipes.is_empty() {
            self.draw_text_sharp(
                "No recipes available",
                content_x + 20.0 * s,
                content_y + 40.0 * s,
                16.0,
                TEXT_DIM,
            );
            return;
        }

        // Register scroll area
        let scroll_bounds = Rect::new(content_x, content_y, content_w, content_h);
        layout.add(UiElementId::WorkbenchScrollArea, scroll_bounds);

        // Calculate total content height including section headers
        let row_height = 56.0 * s;
        let section_header_h = 22.0 * s;
        let mut total_content = 0.0_f32;
        let mut prev_section: Option<&str> = None;
        for recipe in &workbench_recipes {
            let section = recipe.section.as_deref().unwrap_or("");
            if prev_section != Some(section) {
                total_content += section_header_h;
                prev_section = Some(section);
            }
            total_content += row_height;
        }

        let max_scroll = (total_content - content_h).max(0.0);
        let scroll_offset = state
            .ui_state
            .workbench_scroll_offset
            .clamp(0.0, max_scroll);

        // Scissor clipping for scrollable area
        let physical_w = screen_width();
        let physical_h = screen_height();
        let (vw, vh) = virtual_screen_size();
        let scale_x = physical_w / vw;
        let scale_y = physical_h / vh;
        {
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            gl.quad_gl.scissor(Some((
                ((content_x) * scale_x) as i32,
                ((content_y) * scale_y) as i32,
                ((content_w) * scale_x) as i32,
                ((content_h) * scale_y) as i32,
            )));
        }

        let mut y = content_y - scroll_offset;
        let mut prev_section: Option<&str> = None;
        let mut recipe_index = 0;

        for recipe in &workbench_recipes {
            let section = recipe.section.as_deref().unwrap_or("");

            // Section header
            if prev_section != Some(section) {
                let header_top = y;
                let header_bottom = y + section_header_h;

                if header_bottom >= content_y && header_top <= content_y + content_h {
                    draw_rectangle(
                        content_x + 2.0,
                        y + 1.0,
                        content_w - 4.0,
                        section_header_h - 2.0,
                        Color::new(0.12, 0.13, 0.14, 1.0),
                    );
                    let section_name = workbench_section_name(section);
                    self.draw_text_sharp(
                        section_name,
                        content_x + 12.0 * s,
                        y + section_header_h * 0.72,
                        16.0,
                        Color::new(0.70, 0.50, 0.25, 0.8),
                    );
                    // Decorative line after section name
                    let name_w = self.measure_text_sharp(section_name, 16.0).width;
                    draw_line(
                        content_x + 12.0 * s + name_w + 8.0 * s,
                        y + section_header_h / 2.0,
                        content_x + content_w - 12.0 * s,
                        y + section_header_h / 2.0,
                        1.0,
                        Color::new(0.25, 0.18, 0.10, 0.6),
                    );
                }

                y += section_header_h;
                prev_section = Some(section);
            }

            let row_top = y;
            let row_bottom = y + row_height;

            // Skip rows outside visible area
            if row_bottom < content_y || row_top > content_y + content_h {
                y += row_height;
                recipe_index += 1;
                continue;
            }

            let is_selected = recipe_index == state.ui_state.workbench_selected_recipe;
            let is_hovered = matches!(hovered, Some(UiElementId::WorkbenchRecipeItem(idx)) if *idx == recipe_index);

            // Row background
            let row_bg = if is_selected {
                SLOT_HOVER_BG
            } else if is_hovered {
                Color::new(0.141, 0.141, 0.188, 1.0)
            } else {
                Color::new(0.0, 0.0, 0.0, 0.0)
            };

            if is_selected || is_hovered {
                draw_rectangle(
                    content_x + 2.0,
                    y + 1.0,
                    content_w - 4.0,
                    row_height - 2.0,
                    row_bg,
                );
            }

            if is_selected {
                // Left accent bar (brown for workbench)
                draw_rectangle(
                    content_x + 2.0,
                    y + 4.0,
                    3.0,
                    row_height - 8.0,
                    Color::new(0.70, 0.50, 0.25, 1.0),
                );
            }

            // Register click area
            let row_bounds = Rect::new(content_x + 2.0, y + 1.0, content_w - 4.0, row_height - 2.0);
            layout.add(UiElementId::WorkbenchRecipeItem(recipe_index), row_bounds);

            // Icon (left side)
            let icon_size = 40.0 * s;
            let icon_x = content_x + 12.0 * s;
            let icon_y = y + (row_height - icon_size) / 2.0;
            if let Some(result) = recipe.results.first() {
                self.draw_item_icon(
                    &result.item_id,
                    icon_x,
                    icon_y,
                    icon_size,
                    icon_size,
                    state,
                    true,
                );
            }

            // Recipe name
            let text_x = icon_x + icon_size + 10.0 * s;
            let name_color = if is_selected { TEXT_TITLE } else { TEXT_NORMAL };
            self.draw_text_sharp(&recipe.display_name, text_x, y + 20.0 * s, 16.0, name_color);

            // Draw each ingredient with individual green/red color
            let max_ing_w = content_w - (text_x - content_x) - 8.0 * s;
            let mut cursor_x = text_x;
            let ing_y = y + 40.0 * s;
            let green = Color::new(0.392, 0.784, 0.392, 0.7);
            let red = Color::new(0.784, 0.314, 0.314, 0.7);
            for (i, ing) in recipe.ingredients.iter().enumerate() {
                let have = state.inventory.count_item_by_id(&ing.item_id);
                let name = state.item_registry.get_display_name(&ing.item_id);
                let ing_color = if have >= ing.count { green } else { red };
                let label = if i < recipe.ingredients.len() - 1 {
                    format!("{}x {}, ", ing.count, name)
                } else {
                    format!("{}x {}", ing.count, name)
                };
                // Stop drawing if we'd exceed available width
                let dims = self.measure_text_sharp(&label, 16.0);
                if cursor_x + dims.width > text_x + max_ing_w {
                    self.draw_text_sharp("...", cursor_x, ing_y, 16.0, ing_color);
                    break;
                }
                self.draw_text_sharp(&label, cursor_x, ing_y, 16.0, ing_color);
                cursor_x += dims.width;
            }

            // Separator line
            draw_line(
                content_x + 10.0 * s,
                y + row_height - 1.0,
                content_x + content_w - 10.0 * s,
                y + row_height - 1.0,
                1.0,
                Color::new(0.15, 0.15, 0.20, 1.0),
            );

            y += row_height;
            recipe_index += 1;
        }

        // Disable scissor
        {
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            gl.quad_gl.scissor(None);
        }

        // Scrollbar
        if max_scroll > 0.0 {
            let scrollbar_track_h = content_h - 4.0 * s;
            let scrollbar_x = content_x + content_w - 8.0 * s;
            let scrollbar_y = content_y + 2.0 * s;
            let scrollbar_w = 4.0 * s;

            draw_rectangle(
                scrollbar_x,
                scrollbar_y,
                scrollbar_w,
                scrollbar_track_h,
                Color::new(0.1, 0.08, 0.06, 1.0),
            );

            let visible_ratio = (content_h / total_content).min(1.0);
            let thumb_h = (scrollbar_track_h * visible_ratio).max(16.0 * s);
            let scroll_ratio = if max_scroll > 0.0 {
                scroll_offset / max_scroll
            } else {
                0.0
            };
            let thumb_y = scrollbar_y + scroll_ratio * (scrollbar_track_h - thumb_h);
            let is_dragging = state.ui_state.workbench_scroll_drag.dragging;
            let is_scrollbar_hovered = matches!(hovered, Some(UiElementId::WorkbenchScrollbar));
            let thumb_color = if is_dragging || is_scrollbar_hovered {
                SLOT_HOVER_BORDER
            } else {
                SLOT_BORDER
            };
            draw_rectangle(scrollbar_x, thumb_y, scrollbar_w, thumb_h, thumb_color);
            layout.add_scrollbar(
                UiElementId::WorkbenchScrollbar,
                Rect::new(scrollbar_x, scrollbar_y, scrollbar_w, scrollbar_track_h),
            );
        }
    }

    /// Render crafting detail panel (bottom section) for selected recipe
    fn render_workbench_crafting_detail(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        let s = self.font_scale.get();

        // Get workbench recipes filtered by active tab
        let tab_sections = workbench_sections_for_tab(state.ui_state.workbench_tab);
        let mut workbench_recipes: Vec<_> = state
            .recipe_definitions
            .iter()
            .filter(|r| r.station.as_deref() == Some("workbench"))
            .filter(|r| !r.requires_discovery || state.discovered_recipes.contains(&r.id))
            .filter(|r| tab_sections.contains(&r.section.as_deref().unwrap_or("")))
            .collect();

        workbench_recipes.sort_by(|a, b| {
            let sa = a.section.as_deref().unwrap_or("");
            let sb = b.section.as_deref().unwrap_or("");
            section_sort_key(sa)
                .cmp(&section_sort_key(sb))
                .then(a.level_required.cmp(&b.level_required))
        });

        let selected = state.ui_state.workbench_selected_recipe;
        let recipe = match workbench_recipes.get(selected) {
            Some(r) => r,
            None => {
                self.draw_text_sharp(
                    "Select a recipe to craft",
                    x + 20.0 * s,
                    y + 30.0 * s,
                    16.0,
                    TEXT_DIM,
                );
                return;
            }
        };

        // Dark background for detail area
        draw_rectangle(x, y, w, h, Color::new(0.06, 0.06, 0.05, 0.5));

        let pad = 12.0 * s;
        let right_edge = x + w - pad;

        // ===== HEADER: Icon + Recipe name + Level/XP/Time =====
        let header_y = y + 8.0 * s;
        let icon_size = 40.0 * s;
        let icon_x = x + pad;

        if let Some(result) = recipe.results.first() {
            self.draw_item_icon(
                &result.item_id,
                icon_x,
                header_y,
                icon_size,
                icon_size,
                state,
                true,
            );
        }

        let text_x = icon_x + icon_size + 10.0 * s;
        self.draw_text_sharp(
            &recipe.display_name,
            text_x,
            header_y + 16.0 * s,
            16.0,
            TEXT_TITLE,
        );

        // Info line: Lv · XP · time
        let mut info_parts = Vec::new();
        if recipe.level_required > 1 {
            info_parts.push(format!("Lv{}", recipe.level_required));
        }
        if recipe.xp > 0 {
            info_parts.push(format!("{}xp", recipe.xp));
        }
        if recipe.craft_time_ms > 0 {
            let t = recipe.craft_time_ms as f32 / 1000.0;
            if t == t.floor() {
                info_parts.push(format!("{}s", t as u32));
            } else {
                info_parts.push(format!("{:.1}s", t));
            }
        }
        if !info_parts.is_empty() {
            let info_text = info_parts.join(" \u{00b7} ");
            let info_color = if recipe.level_required > 1 {
                if let Some(player) = state.get_local_player() {
                    if player.skills.survivalist.level < recipe.level_required {
                        Color::new(0.784, 0.314, 0.314, 1.0)
                    } else {
                        TEXT_DIM
                    }
                } else {
                    TEXT_DIM
                }
            } else {
                TEXT_DIM
            };
            self.draw_text_sharp(&info_text, text_x, header_y + 34.0 * s, 16.0, info_color);
        }

        // ===== SEPARATOR =====
        let sep_y = header_y + icon_size + 8.0 * s;
        draw_line(x + pad, sep_y, right_edge, sep_y, 1.0, HEADER_BORDER);

        // ===== INGREDIENT ROWS =====
        let ing_left = icon_x + 8.0;
        let ing_start_y = sep_y + 6.0 * s;
        let ing_icon_size = 24.0 * s;
        let ing_row_h = 28.0 * s;
        let green = Color::new(0.392, 0.784, 0.392, 1.0);
        let red = Color::new(0.784, 0.314, 0.314, 1.0);

        let mut can_craft = true;

        for (i, ing) in recipe.ingredients.iter().enumerate() {
            let have = state.inventory.count_item_by_id(&ing.item_id);
            let name = state.item_registry.get_display_name(&ing.item_id);
            let has_enough = have >= ing.count;

            if !has_enough {
                can_craft = false;
            }

            let row_y = ing_start_y + i as f32 * ing_row_h;
            let color = if has_enough { green } else { red };

            // Ingredient icon
            let icon_y_centered = row_y + (ing_row_h - ing_icon_size) / 2.0;
            self.draw_item_icon(
                &ing.item_id,
                ing_left,
                icon_y_centered,
                ing_icon_size,
                ing_icon_size,
                state,
                false,
            );

            // Ingredient name
            let name_x = ing_left + ing_icon_size + 18.0;
            self.draw_text_sharp(name, name_x, row_y + ing_row_h * 0.68, 16.0, color);

            // Have/need count (right-aligned)
            let count_text = format!("{} / {}", have, ing.count);
            let count_w = self.measure_text_sharp(&count_text, 16.0).width;
            self.draw_text_sharp(
                &count_text,
                right_edge - count_w,
                row_y + ing_row_h * 0.68,
                16.0,
                color,
            );
        }

        // Level check
        if recipe.level_required > 1 {
            if let Some(player) = state.get_local_player() {
                if player.skills.survivalist.level < recipe.level_required {
                    can_craft = false;
                }
            }
        }

        // ===== BOTTOM ACTION BAR: Quantity (left) + Craft button (right) =====
        let btn_h = 26.0 * s;
        let btn_y = y + h - btn_h - 6.0 * s;
        let qty_btn_size = btn_h;

        // Minus button
        let minus_x = ing_left;
        let minus_bounds = Rect::new(minus_x, btn_y, qty_btn_size, qty_btn_size);
        layout.add(UiElementId::WorkbenchQuantityMinus, minus_bounds);
        let is_minus_hovered = matches!(hovered, Some(UiElementId::WorkbenchQuantityMinus));
        let (minus_bg, minus_border) = if is_minus_hovered {
            (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
        } else {
            (SLOT_BG_EMPTY, SLOT_BORDER)
        };
        draw_rectangle(minus_x, btn_y, qty_btn_size, qty_btn_size, minus_border);
        draw_rectangle(
            minus_x + 1.0,
            btn_y + 1.0,
            qty_btn_size - 2.0,
            qty_btn_size - 2.0,
            minus_bg,
        );
        let minus_dims = self.measure_text_sharp("-", 16.0);
        self.draw_text_sharp(
            "-",
            minus_x + (qty_btn_size - minus_dims.width) / 2.0,
            btn_y + qty_btn_size * 0.73,
            16.0,
            if is_minus_hovered {
                TEXT_TITLE
            } else {
                TEXT_NORMAL
            },
        );

        // Quantity display
        let qty_text = format!("{}", state.ui_state.workbench_quantity);
        let qty_dims = self.measure_text_sharp(&qty_text, 16.0);
        let qty_display_x = minus_x + qty_btn_size + 4.0 * s;
        let qty_display_w = 24.0 * s;
        self.draw_text_sharp(
            &qty_text,
            qty_display_x + (qty_display_w - qty_dims.width) / 2.0,
            btn_y + qty_btn_size * 0.73,
            16.0,
            TEXT_TITLE,
        );

        // Plus button
        let plus_x = qty_display_x + qty_display_w + 4.0 * s;
        let plus_bounds = Rect::new(plus_x, btn_y, qty_btn_size, qty_btn_size);
        layout.add(UiElementId::WorkbenchQuantityPlus, plus_bounds);
        let is_plus_hovered = matches!(hovered, Some(UiElementId::WorkbenchQuantityPlus));
        let (plus_bg, plus_border) = if is_plus_hovered {
            (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
        } else {
            (SLOT_BG_EMPTY, SLOT_BORDER)
        };
        draw_rectangle(plus_x, btn_y, qty_btn_size, qty_btn_size, plus_border);
        draw_rectangle(
            plus_x + 1.0,
            btn_y + 1.0,
            qty_btn_size - 2.0,
            qty_btn_size - 2.0,
            plus_bg,
        );
        let plus_dims = self.measure_text_sharp("+", 16.0);
        self.draw_text_sharp(
            "+",
            plus_x + (qty_btn_size - plus_dims.width) / 2.0,
            btn_y + qty_btn_size * 0.73,
            16.0,
            if is_plus_hovered {
                TEXT_TITLE
            } else {
                TEXT_NORMAL
            },
        );

        // CRAFT button (right-aligned)
        let craft_btn_w = 120.0 * s;
        let craft_btn_h = btn_h;
        let craft_btn_x = right_edge - craft_btn_w;
        let craft_btn_y = btn_y;

        if can_craft {
            let bounds = Rect::new(craft_btn_x, craft_btn_y, craft_btn_w, craft_btn_h);
            layout.add(UiElementId::WorkbenchCraftButton, bounds);
        }

        let is_craft_hovered =
            can_craft && matches!(hovered, Some(UiElementId::WorkbenchCraftButton));
        let (btn_bg, btn_border) = if !can_craft {
            (Color::new(0.12, 0.08, 0.06, 1.0), SLOT_BORDER)
        } else if is_craft_hovered {
            (
                Color::new(0.50, 0.35, 0.15, 1.0),
                Color::new(0.70, 0.50, 0.25, 1.0),
            )
        } else {
            (
                Color::new(0.40, 0.28, 0.12, 1.0),
                Color::new(0.60, 0.42, 0.20, 1.0),
            )
        };

        draw_rectangle(
            craft_btn_x,
            craft_btn_y,
            craft_btn_w,
            craft_btn_h,
            btn_border,
        );
        draw_rectangle(
            craft_btn_x + 1.0,
            craft_btn_y + 1.0,
            craft_btn_w - 2.0,
            craft_btn_h - 2.0,
            btn_bg,
        );

        if can_craft {
            draw_line(
                craft_btn_x + 2.0,
                craft_btn_y + 2.0,
                craft_btn_x + craft_btn_w - 2.0,
                craft_btn_y + 2.0,
                1.0,
                Color::new(0.70, 0.50, 0.25, 1.0),
            );
        }

        let craft_text = if can_craft {
            "[ CRAFT ]"
        } else {
            "Can't Craft"
        };
        let craft_text_w = self.measure_text_sharp(craft_text, 16.0).width;
        let craft_text_color = if !can_craft {
            Color::new(0.5, 0.3, 0.3, 1.0)
        } else if is_craft_hovered {
            WHITE
        } else {
            Color::new(0.70, 0.50, 0.25, 1.0)
        };
        self.draw_text_sharp(
            craft_text,
            craft_btn_x + (craft_btn_w - craft_text_w) / 2.0,
            craft_btn_y + craft_btn_h * 0.69,
            16.0,
            craft_text_color,
        );
    }

    /// Render crafting progress overlay
    fn render_workbench_progress(
        &self,
        state: &GameState,
        _hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        area_x: f32,
        area_y: f32,
        area_w: f32,
        area_h: f32,
    ) {
        let s = self.font_scale.get();
        let progress = state.ui_state.crafting_progress;
        let time = get_time() as f32;

        // "CRAFTING..." text with pulsing ellipsis
        let dots = match ((time * 2.0) as i32) % 4 {
            0 => "CRAFTING",
            1 => "CRAFTING.",
            2 => "CRAFTING..",
            _ => "CRAFTING...",
        };
        let dots_dims = self.measure_text_sharp(dots, 16.0);
        self.draw_text_sharp(
            dots,
            area_x + (area_w - dots_dims.width) / 2.0,
            area_y + 36.0 * s,
            16.0,
            TEXT_TITLE,
        );

        // Show result item icon + name
        if let Some(ref recipe_id) = state.ui_state.crafting_recipe_id {
            if let Some(recipe) = state.recipe_definitions.iter().find(|r| &r.id == recipe_id) {
                let icon_size = 48.0 * s;
                if let Some(result) = recipe.results.first() {
                    let icon_x = area_x + (area_w - icon_size) / 2.0;
                    self.draw_item_icon(
                        &result.item_id,
                        icon_x,
                        area_y + 46.0 * s,
                        icon_size,
                        icon_size,
                        state,
                        true,
                    );
                }

                // Pulsing recipe name
                let pulse = (time * 3.0).sin() * 0.15 + 0.85;
                let pulse_color = Color::new(
                    TEXT_TITLE.r * pulse,
                    TEXT_TITLE.g * pulse,
                    TEXT_TITLE.b * pulse,
                    1.0,
                );
                let name_dims = self.measure_text_sharp(&recipe.display_name, 16.0);
                self.draw_text_sharp(
                    &recipe.display_name,
                    area_x + (area_w - name_dims.width) / 2.0,
                    area_y + 46.0 * s + icon_size + 16.0 * s,
                    16.0,
                    pulse_color,
                );
            }
        }

        // Batch counter
        if state.ui_state.batch_total > 1 {
            let batch_text = format!(
                "{}/{}",
                state.ui_state.batch_completed, state.ui_state.batch_total
            );
            let batch_dims = self.measure_text_sharp(&batch_text, 16.0);
            self.draw_text_sharp(
                &batch_text,
                area_x + (area_w - batch_dims.width) / 2.0,
                area_y + 130.0 * s,
                16.0,
                TEXT_NORMAL,
            );
        }

        // Progress bar (brown workbench theme)
        let bar_width = area_w - 60.0 * s;
        let bar_height = 20.0 * s;
        let bar_x = area_x + 30.0 * s;
        let bar_y = area_y + area_h / 2.0 - bar_height / 2.0 + 10.0;

        draw_rectangle(bar_x, bar_y, bar_width, bar_height, SLOT_BORDER);
        draw_rectangle(
            bar_x + 1.0,
            bar_y + 1.0,
            bar_width - 2.0,
            bar_height - 2.0,
            SLOT_BG_EMPTY,
        );
        draw_line(
            bar_x + 2.0,
            bar_y + 2.0,
            bar_x + bar_width - 2.0,
            bar_y + 2.0,
            1.0,
            SLOT_INNER_SHADOW,
        );

        let fill_width = (bar_width - 4.0) * progress;
        if fill_width > 0.0 {
            let fill_x = bar_x + 2.0;
            let fill_y = bar_y + 2.0;
            let fill_h = bar_height - 4.0;

            draw_rectangle(fill_x, fill_y, fill_width, fill_h, WORKBENCH_PROGRESS_DARK);
            draw_rectangle(
                fill_x,
                fill_y,
                fill_width,
                fill_h / 2.0,
                WORKBENCH_PROGRESS_MID,
            );
            draw_line(
                fill_x,
                fill_y,
                fill_x + fill_width,
                fill_y,
                1.0,
                WORKBENCH_PROGRESS_LIGHT,
            );
        }

        // Percentage
        let pct_text = format!("{}%", (progress * 100.0) as i32);
        let pct_dims = self.measure_text_sharp(&pct_text, 16.0);
        self.draw_text_sharp(
            &pct_text,
            area_x + (area_w - pct_dims.width) / 2.0,
            bar_y + bar_height + 20.0 * s,
            16.0,
            TEXT_NORMAL,
        );

        // Cancel button
        let cancel_w = 120.0 * s;
        let cancel_h = 28.0 * s;
        let cancel_x = area_x + (area_w - cancel_w) / 2.0;
        let cancel_y = area_y + area_h - 42.0 * s;

        let cancel_bounds = Rect::new(cancel_x, cancel_y, cancel_w, cancel_h);
        layout.add(UiElementId::WorkbenchCancelButton, cancel_bounds);

        let is_cancel_hovered = matches!(_hovered, Some(UiElementId::WorkbenchCancelButton));
        let (cancel_bg, cancel_border) = if is_cancel_hovered {
            (
                Color::new(0.45, 0.15, 0.15, 1.0),
                Color::new(0.6, 0.2, 0.2, 1.0),
            )
        } else {
            (
                Color::new(0.35, 0.12, 0.12, 1.0),
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

        let cancel_text = "[ CANCEL ]";
        let cancel_text_w = self.measure_text_sharp(cancel_text, 16.0).width;
        let cancel_text_color = if is_cancel_hovered {
            WHITE
        } else {
            Color::new(0.85, 0.6, 0.6, 1.0)
        };
        self.draw_text_sharp(
            cancel_text,
            cancel_x + (cancel_w - cancel_text_w) / 2.0,
            cancel_y + cancel_h * 0.68,
            16.0,
            cancel_text_color,
        );
    }
}
