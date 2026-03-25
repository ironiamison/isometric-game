//! Crafting panel rendering

use super::super::Renderer;
use super::common::*;
use crate::game::{GameState, RecipeDefinition};
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

/// Height of a section header row in the blueprint list
pub const SECTION_HEADER_HEIGHT: f32 = 22.0;

/// Sort key for section ordering within a category.
/// Values overlap across categories (e.g. "materials"=0, "restoration"=0) because
/// sections from different categories never appear in the same list.
pub fn section_sort_key(section: &str) -> usize {
    match section {
        "materials" => 0,
        "equipment" => 1,
        "accessories" => 2,
        "ammunition" => 3,
        "restoration" => 0,
        "stat_buffs" => 1,
        "utility" => 2,
        "scrolls" => 3,
        "cooking" => 0,
        _ => 99,
    }
}

/// Display name for a section
fn section_display_name(section: &str) -> &str {
    match section {
        "materials" => "Materials",
        "equipment" => "Equipment",
        "accessories" => "Accessories",
        "ammunition" => "Ammunition",
        "restoration" => "Restoration",
        "stat_buffs" => "Stat Buffs",
        "utility" => "Utility",
        "scrolls" => "Scrolls",
        "cooking" => "Cooking",
        _ => section,
    }
}

/// Helper to build the category list from recipes, grouping materials/consumables into "supplies"
fn build_categories(recipes: &[RecipeDefinition]) -> Vec<String> {
    let mut cats: Vec<String> = recipes
        .iter()
        .map(|r| {
            if r.category == "materials" || r.category == "consumables" {
                "supplies".to_string()
            } else {
                r.category.clone()
            }
        })
        .collect();
    cats.sort();
    cats.dedup();
    cats
}

/// Helper to filter recipes for a given category (matching the supplies grouping)
fn recipes_for_category<'a>(
    recipes: &'a [RecipeDefinition],
    category: &str,
) -> Vec<&'a RecipeDefinition> {
    recipes
        .iter()
        .filter(|r| {
            if category == "supplies" {
                r.category == "consumables" || r.category == "materials"
            } else {
                r.category == category
            }
        })
        .collect()
}

impl Renderer {
    pub(crate) fn render_crafting(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;

        let (panel_x, panel_y, panel_width, panel_height) = if cfg!(target_os = "android") {
            (0.0, 0.0, sw, sh)
        } else {
            let pw = (650.0 * s).min(sw - 16.0);
            let ph = (450.0 * s).min(sh - 16.0);
            ((sw - pw) / 2.0, (sh - ph) / 2.0, pw, ph)
        };

        // Semi-transparent overlay
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.588));

        // Draw themed panel frame with corner accents
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        // ===== HEADER SECTION =====
        let header_h = HEADER_HEIGHT * s;
        let footer_h = if cfg!(target_os = "android") { 0.0 } else { FOOTER_HEIGHT * s };
        let header_x = panel_x + FRAME_THICKNESS;
        let header_y = panel_y + FRAME_THICKNESS;
        let header_w = panel_width - FRAME_THICKNESS * 2.0;

        draw_rectangle(header_x, header_y, header_w, header_h, HEADER_BG);

        draw_line(
            header_x + 10.0 * s,
            header_y + header_h,
            header_x + header_w - 10.0 * s,
            header_y + header_h,
            2.0,
            HEADER_BORDER,
        );

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

        // Main tabs: Shop / Crafting
        let main_tab_y = header_y + 6.0 * s;
        let main_tab_height = TAB_HEIGHT * s;
        let main_tab_width = 100.0 * s;
        let mut main_tab_x = header_x + 12.0 * s;

        // Shop Tab
        let is_shop_selected = state.ui_state.shop_main_tab == 1;
        let shop_bounds = Rect::new(main_tab_x, main_tab_y, main_tab_width, main_tab_height);
        layout.add(UiElementId::MainTab(1), shop_bounds);

        let is_shop_hovered = matches!(hovered, Some(UiElementId::MainTab(1)));
        let (shop_bg, shop_border) = if is_shop_selected {
            (SLOT_HOVER_BG, SLOT_SELECTED_BORDER)
        } else if is_shop_hovered {
            (Color::new(0.141, 0.141, 0.188, 1.0), SLOT_HOVER_BORDER)
        } else {
            (SLOT_BG_EMPTY, SLOT_BORDER)
        };

        draw_rectangle(
            main_tab_x,
            main_tab_y,
            main_tab_width,
            main_tab_height,
            shop_border,
        );
        draw_rectangle(
            main_tab_x + 1.0,
            main_tab_y + 1.0,
            main_tab_width - 2.0,
            main_tab_height - 2.0,
            shop_bg,
        );

        let shop_text_color = if is_shop_selected {
            TEXT_TITLE
        } else if is_shop_hovered {
            TEXT_NORMAL
        } else {
            TEXT_DIM
        };
        let shop_dims = self.measure_text_sharp("Shop", TAB_FONT_SIZE);
        let shop_text_x = main_tab_x + (main_tab_width - shop_dims.width) / 2.0;
        self.draw_text_sharp(
            "Shop",
            shop_text_x,
            main_tab_y + main_tab_height * 0.68,
            TAB_FONT_SIZE,
            shop_text_color,
        );

        main_tab_x += main_tab_width + 4.0 * s;

        // Crafting Tab (formerly Recipes) - only show if shop allows it
        let show_crafting = state
            .ui_state
            .shop_data
            .as_ref()
            .map_or(true, |s| s.show_crafting);
        if show_crafting {
            let is_recipes_selected = state.ui_state.shop_main_tab == 0;
            let recipes_bounds = Rect::new(main_tab_x, main_tab_y, main_tab_width, main_tab_height);
            layout.add(UiElementId::MainTab(0), recipes_bounds);

            let is_recipes_hovered = matches!(hovered, Some(UiElementId::MainTab(0)));
            let (recipes_bg, recipes_border) = if is_recipes_selected {
                (SLOT_HOVER_BG, SLOT_SELECTED_BORDER)
            } else if is_recipes_hovered {
                (Color::new(0.141, 0.141, 0.188, 1.0), SLOT_HOVER_BORDER)
            } else {
                (SLOT_BG_EMPTY, SLOT_BORDER)
            };

            draw_rectangle(
                main_tab_x,
                main_tab_y,
                main_tab_width,
                main_tab_height,
                recipes_border,
            );
            draw_rectangle(
                main_tab_x + 1.0,
                main_tab_y + 1.0,
                main_tab_width - 2.0,
                main_tab_height - 2.0,
                recipes_bg,
            );

            let recipes_text_color = if is_recipes_selected {
                TEXT_TITLE
            } else if is_recipes_hovered {
                TEXT_NORMAL
            } else {
                TEXT_DIM
            };
            let recipes_dims = self.measure_text_sharp("Crafting", TAB_FONT_SIZE);
            let recipes_text_x = main_tab_x + (main_tab_width - recipes_dims.width) / 2.0;
            self.draw_text_sharp(
                "Crafting",
                recipes_text_x,
                main_tab_y + main_tab_height * 0.68,
                TAB_FONT_SIZE,
                recipes_text_color,
            );
        }

        // Close button (X) - same style as dialogue close button
        let is_mobile = cfg!(target_os = "android");
        let close_btn_size = if is_mobile { 32.0 * s } else { 28.0 * s };
        let close_btn_x = header_x + header_w - close_btn_size - 6.0 * s;
        let close_btn_y = header_y + (header_h - close_btn_size) / 2.0;
        let close_bounds = Rect::new(close_btn_x, close_btn_y, close_btn_size, close_btn_size);
        layout.add(UiElementId::ShopCraftingCloseButton, close_bounds);

        let is_close_hovered = matches!(hovered, Some(UiElementId::ShopCraftingCloseButton));
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

        // ===== CONTENT AREA =====
        let content_y = panel_y + FRAME_THICKNESS + header_h + 4.0 * s;
        let content_height = panel_height - FRAME_THICKNESS * 2.0 - header_h - footer_h - 12.0 * s;
        let content_width = panel_width - FRAME_THICKNESS * 2.0;

        // Render appropriate tab content
        match state.ui_state.shop_main_tab {
            0 if show_crafting => self.render_recipes_tab(
                state,
                hovered,
                layout,
                panel_x,
                content_y,
                content_width,
                content_height,
            ),
            1 => self.render_shop_tab(
                state,
                hovered,
                layout,
                panel_x,
                content_y,
                content_width,
                content_height,
            ),
            _ => {}
        }

        // ===== FOOTER SECTION =====
        if !cfg!(target_os = "android") {
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

        if state.ui_state.shop_main_tab == 0 {
            if state.ui_state.crafting_in_progress {
                // Show cancel hint while crafting
                self.draw_text_sharp(
                    "[Esc] Cancel",
                    footer_x + 10.0 * s,
                    footer_y + footer_h * 0.67,
                    16.0,
                    TEXT_DIM,
                );
            } else {
                // Recipes tab controls
                self.draw_text_sharp(
                    "[Q] Tab",
                    footer_x + 10.0 * s,
                    footer_y + footer_h * 0.67,
                    16.0,
                    TEXT_DIM,
                );

                let has_multiple_categories =
                    build_categories(&state.shop_filtered_recipes()).len() > 1;

                if has_multiple_categories {
                    self.draw_text_sharp(
                        "[A/D] Category",
                        footer_x + 100.0 * s,
                        footer_y + footer_h * 0.67,
                        16.0,
                        TEXT_DIM,
                    );
                    self.draw_text_sharp(
                        "[W/S] Select",
                        footer_x + 230.0 * s,
                        footer_y + footer_h * 0.67,
                        16.0,
                        TEXT_DIM,
                    );
                    self.draw_text_sharp(
                        "[C] Craft",
                        footer_x + 340.0 * s,
                        footer_y + footer_h * 0.67,
                        16.0,
                        TEXT_DIM,
                    );
                } else {
                    self.draw_text_sharp(
                        "[W/S] Select",
                        footer_x + 100.0 * s,
                        footer_y + footer_h * 0.67,
                        16.0,
                        TEXT_DIM,
                    );
                    self.draw_text_sharp(
                        "[C] Craft",
                        footer_x + 210.0 * s,
                        footer_y + footer_h * 0.67,
                        16.0,
                        TEXT_DIM,
                    );
                }
            }
        } else {
            // Shop tab controls
            self.draw_text_sharp(
                "[Q] Tab",
                footer_x + 10.0 * s,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_DIM,
            );
            self.draw_text_sharp(
                "[Tab] Buy/Sell",
                footer_x + 100.0 * s,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_DIM,
            );
            self.draw_text_sharp(
                "[W/S] Select",
                footer_x + 230.0 * s,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_DIM,
            );
            self.draw_text_sharp(
                "[+/-] Qty",
                footer_x + 340.0 * s,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_DIM,
            );

            // Gold display - right-aligned in footer
            let gold_text = format!("{}g", state.inventory.gold);
            let gold_text_w = self.measure_text_sharp(&gold_text, 16.0).width;
            let icon_size = 12.0 * s;
            let icon_margin = 4.0 * s;
            let total_gold_w = icon_size + icon_margin + gold_text_w;
            let gold_x = footer_x + footer_w - total_gold_w - 10.0 * s;
            if let Some(texture) = &self.gold_nugget_texture {
                draw_texture_ex(
                    texture,
                    gold_x,
                    footer_y + footer_h * 0.33,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(icon_size, icon_size)),
                        ..Default::default()
                    },
                );
            }
            self.draw_text_sharp(
                &gold_text,
                gold_x + icon_size + icon_margin,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_GOLD,
            );
        }
        } // end !android footer
    }

    fn render_recipes_tab(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        panel_x: f32,
        content_y: f32,
        content_width: f32,
        content_height: f32,
    ) {
        let s = self.font_scale.get();

        // Filter recipes by the shop's crafting categories
        let filtered_recipes = state.shop_filtered_recipes();
        let categories = build_categories(&filtered_recipes);

        if categories.is_empty() {
            self.draw_text_sharp(
                "No recipes available",
                panel_x + FRAME_THICKNESS + 20.0 * s,
                content_y + 40.0 * s,
                16.0,
                TEXT_DIM,
            );
            return;
        }

        // ===== CATEGORY TABS =====
        // If we only have one category, we don't need tabs and can stretch the list higher
        let show_tabs = categories.len() > 1;
        let tab_y = content_y;
        let tab_height = if show_tabs { 28.0 * s } else { 0.0 };
        let mut tab_x = panel_x + FRAME_THICKNESS + 10.0 * s;

        if show_tabs {
            for (i, category) in categories.iter().enumerate() {
                let is_selected = i == state.ui_state.crafting_selected_category;
                let display_name: String = category
                    .chars()
                    .enumerate()
                    .map(|(idx, c)| if idx == 0 { c.to_ascii_uppercase() } else { c })
                    .collect();
                let tab_width = self.measure_text_sharp(&display_name, 16.0).width + 24.0 * s;

                let bounds = Rect::new(tab_x, tab_y, tab_width, tab_height);
                layout.add(UiElementId::CraftingCategoryTab(i), bounds);

                let is_hovered =
                    matches!(hovered, Some(UiElementId::CraftingCategoryTab(idx)) if *idx == i);

                let (bg_color, border_color) = if is_selected {
                    (SLOT_HOVER_BG, SLOT_SELECTED_BORDER)
                } else if is_hovered {
                    (Color::new(0.141, 0.141, 0.188, 1.0), SLOT_HOVER_BORDER)
                } else {
                    (SLOT_BG_EMPTY, SLOT_BORDER)
                };

                draw_rectangle(tab_x, tab_y, tab_width, tab_height, border_color);
                draw_rectangle(
                    tab_x + 1.0,
                    tab_y + 1.0,
                    tab_width - 2.0,
                    tab_height - 2.0,
                    bg_color,
                );

                if is_selected {
                    draw_line(
                        tab_x + 2.0,
                        tab_y + 2.0,
                        tab_x + tab_width - 2.0,
                        tab_y + 2.0,
                        1.0,
                        FRAME_INNER,
                    );
                    draw_line(
                        tab_x + 2.0,
                        tab_y + 2.0,
                        tab_x + 2.0,
                        tab_y + tab_height - 2.0,
                        1.0,
                        FRAME_INNER,
                    );
                }

                let text_color = if is_selected {
                    TEXT_TITLE
                } else if is_hovered {
                    TEXT_NORMAL
                } else {
                    TEXT_DIM
                };
                self.draw_text_sharp(
                    &display_name,
                    tab_x + 12.0 * s,
                    tab_y + tab_height * 0.68,
                    16.0,
                    text_color,
                );

                tab_x += tab_width + 4.0 * s;
            }
        }

        let selected_idx = state
            .ui_state
            .crafting_selected_category
            .min(categories.len().saturating_sub(1));
        let current_category = categories
            .get(selected_idx)
            .map(|s| s.as_str())
            .unwrap_or("supplies");

        // Get all recipes for this category
        let all_recipes = recipes_for_category(&filtered_recipes, current_category);

        // Filter recipes by discovery status and sort by section then level
        let mut visible_recipes: Vec<(usize, &RecipeDefinition, bool)> = all_recipes
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let is_discovered =
                    !r.requires_discovery || state.discovered_recipes.contains(&r.id);
                (i, *r, is_discovered)
            })
            .collect();

        // Sort by section order, then level, then name
        visible_recipes.sort_by(|a, b| {
            let a_sec = a.1.section.as_deref().unwrap_or("");
            let b_sec = b.1.section.as_deref().unwrap_or("");
            section_sort_key(a_sec)
                .cmp(&section_sort_key(b_sec))
                .then(a.1.level_required.cmp(&b.1.level_required))
                .then(a.1.display_name.cmp(&b.1.display_name))
        });

        // ===== RECIPE LIST (left side) =====
        let list_width = (220.0 * s).min((content_width - 32.0 * s) * 0.38);
        let list_x = panel_x + FRAME_THICKNESS + 10.0 * s;
        let list_y = if show_tabs {
            tab_y + tab_height + 12.0 * s
        } else {
            content_y + 2.0 * s
        };
        let list_height = if show_tabs {
            content_height - tab_height - 20.0 * s
        } else {
            content_height - 10.0 * s
        };

        draw_rectangle(list_x, list_y, list_width, list_height, SLOT_BORDER);
        draw_rectangle(
            list_x + 1.0,
            list_y + 1.0,
            list_width - 2.0,
            list_height - 2.0,
            SLOT_BG_EMPTY,
        );

        draw_line(
            list_x + 2.0,
            list_y + 2.0,
            list_x + list_width - 2.0,
            list_y + 2.0,
            2.0,
            SLOT_INNER_SHADOW,
        );
        draw_line(
            list_x + 2.0,
            list_y + 2.0,
            list_x + 2.0,
            list_y + list_height - 2.0,
            2.0,
            SLOT_INNER_SHADOW,
        );

        let line_height = 28.0 * s;
        let section_header_h = SECTION_HEADER_HEIGHT * s;
        let list_content_y = list_y + 4.0 * s;
        let list_content_height = list_height - 8.0 * s;

        // Count distinct non-empty sections for header height
        let num_sections = {
            let mut seen = std::collections::HashSet::new();
            for (_, r, _) in &visible_recipes {
                if let Some(ref s) = r.section {
                    if !s.is_empty() {
                        seen.insert(s.as_str());
                    }
                }
            }
            seen.len()
        };
        // Calculate total content height and clamp scroll offset
        let total_content =
            visible_recipes.len() as f32 * line_height + num_sections as f32 * section_header_h;
        let max_scroll = (total_content - list_content_height).max(0.0);
        let scroll_offset = state.ui_state.crafting_scroll_offset.clamp(0.0, max_scroll);

        // Scissor rect for clipping the recipe list
        let physical_w = screen_width();
        let physical_h = screen_height();
        let (vw, _vh) = virtual_screen_size();
        let scale_x = physical_w / vw;
        let scale_y = physical_h / _vh;
        {
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            gl.quad_gl.scissor(Some((
                ((list_x + 2.0) * scale_x) as i32,
                ((list_content_y) * scale_y) as i32,
                ((list_width - 4.0) * scale_x) as i32,
                ((list_content_height) * scale_y) as i32,
            )));
        }

        let mut y = list_content_y - scroll_offset;

        // Track the index of selectable recipes (discovered only)
        let mut selectable_index = 0usize;
        let mut current_section: Option<&str> = None;

        for (_orig_idx, recipe, is_discovered) in &visible_recipes {
            // Check if we need a section header
            let recipe_section = recipe.section.as_deref().unwrap_or("");
            if !recipe_section.is_empty() && current_section != Some(recipe_section) {
                current_section = Some(recipe_section);

                // Render section header if visible
                let header_bottom = y + section_header_h;
                if header_bottom >= list_content_y && y <= list_content_y + list_content_height {
                    let display = section_display_name(recipe_section);
                    self.draw_text_sharp(
                        display,
                        list_x + 8.0 * s,
                        y + section_header_h * 0.73,
                        16.0,
                        FRAME_ACCENT,
                    );
                    draw_line(
                        list_x + 8.0 * s,
                        y + section_header_h - 2.0,
                        list_x + list_width - 8.0 * s,
                        y + section_header_h - 2.0,
                        1.0,
                        Color::new(0.25, 0.22, 0.15, 1.0),
                    );
                }
                y += section_header_h;
            }

            let item_bottom = y + line_height;
            let item_top = y;

            // Skip items fully above or below the visible area
            if item_bottom < list_content_y || item_top > list_content_y + list_content_height {
                if *is_discovered {
                    selectable_index += 1;
                }
                y += line_height;
                continue;
            }

            if *is_discovered {
                let is_selected = selectable_index == state.ui_state.crafting_selected_recipe;

                let item_bounds =
                    Rect::new(list_x + 4.0 * s, y, list_width - 8.0 * s, line_height - 2.0);
                layout.add(
                    UiElementId::CraftingRecipeItem(selectable_index),
                    item_bounds,
                );

                let is_hovered = matches!(hovered, Some(UiElementId::CraftingRecipeItem(idx)) if *idx == selectable_index);

                if is_selected {
                    draw_rectangle(
                        list_x + 4.0 * s,
                        y,
                        list_width - 8.0 * s,
                        line_height - 2.0,
                        SLOT_HOVER_BG,
                    );
                } else if is_hovered {
                    draw_rectangle(
                        list_x + 4.0 * s,
                        y,
                        list_width - 8.0 * s,
                        line_height - 2.0,
                        Color::new(0.125, 0.125, 0.173, 1.0),
                    );
                }

                let text_color = if is_selected {
                    TEXT_TITLE
                } else if is_hovered {
                    TEXT_NORMAL
                } else {
                    TEXT_DIM
                };

                let prefix = if is_selected { "> " } else { "  " };
                let text_y = y + (line_height - 2.0) / 2.0 + line_height * 0.18;
                self.draw_text_sharp(
                    &format!("{}{}", prefix, recipe.display_name),
                    list_x + 8.0 * s,
                    text_y,
                    16.0,
                    text_color,
                );

                if recipe.level_required > 1 {
                    let level_text = format!("Lv{}", recipe.level_required);
                    let level_width = self.measure_text_sharp(&level_text, 16.0).width;
                    self.draw_text_sharp(
                        &level_text,
                        list_x + list_width - level_width - 12.0 * s,
                        text_y,
                        16.0,
                        FRAME_MID,
                    );
                }

                selectable_index += 1;
            } else {
                // Undiscovered recipe - show grayed out "????" (non-selectable)
                let text_y = y + (line_height - 2.0) / 2.0 + line_height * 0.18;
                self.draw_text_sharp(
                    "  ????",
                    list_x + 8.0 * s,
                    text_y,
                    16.0,
                    Color::new(0.35, 0.35, 0.4, 1.0),
                );
            }

            y += line_height;
        }

        // Disable scissor clipping
        {
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            gl.quad_gl.scissor(None);
        }

        // Draw scroll indicator if content overflows
        if max_scroll > 0.0 {
            let scrollbar_track_h = list_content_height - 4.0 * s;
            let scrollbar_x = list_x + list_width - 8.0 * s;
            let scrollbar_y = list_content_y + 2.0 * s;
            let scrollbar_w = 4.0 * s;

            // Track
            draw_rectangle(
                scrollbar_x,
                scrollbar_y,
                scrollbar_w,
                scrollbar_track_h,
                Color::new(0.1, 0.1, 0.13, 1.0),
            );

            // Thumb
            let visible_ratio = (list_content_height / total_content).min(1.0);
            let thumb_h = (scrollbar_track_h * visible_ratio).max(16.0 * s);
            let scroll_ratio = if max_scroll > 0.0 {
                scroll_offset / max_scroll
            } else {
                0.0
            };
            let thumb_y = scrollbar_y + scroll_ratio * (scrollbar_track_h - thumb_h);
            let is_dragging = state.ui_state.crafting_scroll_drag.dragging;
            let is_hovered = matches!(hovered, Some(UiElementId::CraftingScrollbar));
            let thumb_color = if is_dragging || is_hovered {
                FRAME_ACCENT
            } else {
                FRAME_MID
            };
            draw_rectangle(scrollbar_x, thumb_y, scrollbar_w, thumb_h, thumb_color);
            layout.add_scrollbar(
                UiElementId::CraftingScrollbar,
                Rect::new(scrollbar_x, scrollbar_y, scrollbar_w, scrollbar_track_h),
            );
        }

        // Build the list of discovered (selectable) recipes for detail panel
        let discovered_recipes: Vec<&RecipeDefinition> = visible_recipes
            .iter()
            .filter(|(_, _, discovered)| *discovered)
            .map(|(_, r, _)| *r)
            .collect();

        // ===== DETAIL PANEL (right side) =====
        let detail_x = list_x + list_width + 12.0 * s;
        let detail_width = content_width - list_width - 32.0 * s;
        let detail_y = list_y;
        let detail_height = list_height;

        draw_rectangle(detail_x, detail_y, detail_width, detail_height, SLOT_BORDER);
        draw_rectangle(
            detail_x + 1.0,
            detail_y + 1.0,
            detail_width - 2.0,
            detail_height - 2.0,
            Color::new(0.094, 0.094, 0.125, 1.0),
        );

        draw_line(
            detail_x + 2.0,
            detail_y + 2.0,
            detail_x + detail_width - 2.0,
            detail_y + 2.0,
            2.0,
            SLOT_INNER_SHADOW,
        );
        draw_line(
            detail_x + 2.0,
            detail_y + 2.0,
            detail_x + 2.0,
            detail_y + detail_height - 2.0,
            2.0,
            SLOT_INNER_SHADOW,
        );

        // Task 14: If crafting is in progress, show progress overlay instead of normal detail
        if state.ui_state.crafting_in_progress {
            self.render_crafting_progress(
                state,
                hovered,
                layout,
                detail_x,
                detail_y,
                detail_width,
                detail_height,
            );
            return;
        }

        // Task 20: If completion animation is active, show it overlaid
        if let Some((ref recipe_id, timer)) = state.ui_state.crafting_complete_animation {
            if timer < 1.0 {
                self.render_crafting_complete(
                    state,
                    recipe_id,
                    timer,
                    detail_x,
                    detail_y,
                    detail_width,
                    detail_height,
                );
                // Still show normal detail panel underneath, but render overlay on top
                // (the overlay is semi-transparent at the end, so we render both)
            }
        }

        if let Some(recipe) = discovered_recipes.get(state.ui_state.crafting_selected_recipe) {
            // Draw larger sprite preview of the result item in the header area
            let header_icon_size = 48.0 * s;
            let header_icon_x = detail_x + 12.0 * s;
            let header_icon_y = detail_y + 6.0 * s;
            if let Some(result) = recipe.results.first() {
                self.draw_item_icon(
                    &result.item_id,
                    header_icon_x,
                    header_icon_y,
                    header_icon_size,
                    header_icon_size,
                    state,
                    true,
                );
            }

            // Item name next to icon — vertically centered with icon top half
            let text_offset_x = header_icon_size + 8.0 * s;
            let icon_center_y = header_icon_y + header_icon_size / 2.0;
            let name_y = icon_center_y - 6.0 * s;
            self.draw_text_sharp(
                &recipe.display_name,
                detail_x + 12.0 * s + text_offset_x,
                name_y,
                16.0,
                TEXT_TITLE,
            );

            // Station requirement top-right, aligned with name
            if let Some(ref station) = recipe.station {
                let station_display: String = station
                    .chars()
                    .enumerate()
                    .map(|(idx, c)| if idx == 0 { c.to_ascii_uppercase() } else { c })
                    .collect();
                let station_w = self.measure_text_sharp(&station_display, 16.0).width;
                self.draw_text_sharp(
                    &station_display,
                    detail_x + detail_width - station_w - 12.0 * s,
                    name_y,
                    16.0,
                    TEXT_DIM,
                );
            }

            // Craft time + XP on same line below name
            let info_y = icon_center_y + 10.0 * s;
            let info_x = detail_x + 12.0 * s + text_offset_x;
            let mut cursor_x = info_x;
            if recipe.craft_time_ms > 0 {
                let seconds = recipe.craft_time_ms as f32 / 1000.0;
                let time_text = if seconds == seconds.floor() {
                    format!("{}s", seconds as u32)
                } else {
                    format!("{:.1}s", seconds)
                };
                self.draw_text_sharp(&time_text, cursor_x, info_y, 16.0, TEXT_DIM);
                cursor_x += self.measure_text_sharp(&time_text, 16.0).width;
                if recipe.xp > 0 {
                    self.draw_text_sharp(" | ", cursor_x, info_y, 16.0, TEXT_DIM);
                    cursor_x += self.measure_text_sharp(" | ", 16.0).width;
                }
            }
            if recipe.xp > 0 {
                let xp_text = format!("{} XP", recipe.xp);
                self.draw_text_sharp(&xp_text, cursor_x, info_y, 16.0, TEXT_GOLD);
            }

            let desc_start_y = detail_y + 6.0 * s + header_icon_size + 4.0 * s;
            draw_line(
                detail_x + 10.0 * s,
                desc_start_y - 4.0 * s,
                detail_x + detail_width - 10.0 * s,
                desc_start_y - 4.0 * s,
                1.0,
                HEADER_BORDER,
            );

            let desc_height = self.draw_text_wrapped(
                &recipe.description,
                detail_x + 12.0 * s,
                desc_start_y + 14.0 * s,
                16.0,
                TEXT_NORMAL,
                detail_width - 24.0 * s,
                20.0 * s,
            );

            let mut section_y = desc_start_y + 14.0 * s + desc_height + 12.0 * s;

            if recipe.level_required > 1 {
                let skill_name = match recipe.category.as_str() {
                    "smithing" => "Smithing",
                    "alchemy" => "Alchemy",
                    "cooking" | "fletching" | "leatherworking" => "Survivalist",
                    _ => "Combat",
                };
                let (level_color, level_icon) = if let Some(player) = state.get_local_player() {
                    let player_level = match recipe.category.as_str() {
                        "smithing" => player.skills.smithing.level,
                        "alchemy" => player.skills.alchemy.level,
                        "cooking" | "fletching" | "leatherworking" => {
                            player.skills.survivalist.level
                        }
                        _ => player.combat_level(),
                    };
                    if player_level >= recipe.level_required {
                        (Color::new(0.392, 0.784, 0.392, 1.0), "[OK]")
                    } else {
                        (Color::new(0.784, 0.314, 0.314, 1.0), "[!!]")
                    }
                } else {
                    (TEXT_DIM, "[??]")
                };
                self.draw_text_sharp(
                    &format!(
                        "{} Requires {} Level {}",
                        level_icon, skill_name, recipe.level_required
                    ),
                    detail_x + 12.0 * s,
                    section_y,
                    16.0,
                    level_color,
                );
                section_y += 22.0 * s;
            }

            // Show required tool if any
            if let Some(ref tool) = recipe.required_tool {
                let tool_display = match tool.as_str() {
                    "knife" => "Knife",
                    other => other,
                };
                let has_tool = state
                    .inventory
                    .slots
                    .iter()
                    .flatten()
                    .any(|slot| slot.item_id == *tool);
                let (tool_color, tool_icon) = if has_tool {
                    (Color::new(0.392, 0.784, 0.392, 1.0), "[OK]")
                } else {
                    (Color::new(0.784, 0.314, 0.314, 1.0), "[!!]")
                };
                self.draw_text_sharp(
                    &format!("{} Requires: {}", tool_icon, tool_display),
                    detail_x + 12.0 * s,
                    section_y,
                    16.0,
                    tool_color,
                );
                section_y += 22.0 * s;
            }

            self.draw_text_sharp(
                "Materials Required:",
                detail_x + 12.0 * s,
                section_y,
                16.0,
                FRAME_INNER,
            );
            section_y += 22.0 * s;

            let mut craft_blocked_reason: Option<&str> = None;

            // Check level requirement
            if recipe.level_required > 1 {
                if let Some(player) = state.get_local_player() {
                    let player_level = match recipe.category.as_str() {
                        "smithing" => player.skills.smithing.level,
                        "alchemy" => player.skills.alchemy.level,
                        "cooking" | "fletching" | "leatherworking" => {
                            player.skills.survivalist.level
                        }
                        _ => player.combat_level(),
                    };
                    if player_level < recipe.level_required {
                        craft_blocked_reason = Some("Requirements Not Met");
                    }
                }
            }

            // Check required tool
            if craft_blocked_reason.is_none() {
                if let Some(ref tool) = recipe.required_tool {
                    let has_tool = state
                        .inventory
                        .slots
                        .iter()
                        .flatten()
                        .any(|slot| slot.item_id == *tool);
                    if !has_tool {
                        craft_blocked_reason = Some("Missing Required Tool");
                    }
                }
            }

            // Check inventory space
            if craft_blocked_reason.is_none() {
                let free_slots = state.inventory.slots.iter().filter(|s| s.is_none()).count();
                if free_slots == 0 {
                    craft_blocked_reason = Some("Inventory Full");
                }
            }

            let mut missing_ingredients = false;

            for ingredient in &recipe.ingredients {
                let have_count = state.inventory.count_item_by_id(&ingredient.item_id);
                let need_count = ingredient.count;
                let has_enough = have_count >= need_count;

                if !has_enough {
                    missing_ingredients = true;
                }

                let (marker, color) = if has_enough {
                    ("[+]", Color::new(0.392, 0.784, 0.392, 1.0))
                } else {
                    ("[-]", Color::new(0.784, 0.314, 0.314, 1.0))
                };

                // Draw ingredient sprite icon
                let ing_icon_size = 28.0 * s;
                let ing_icon_x = detail_x + 20.0 * s;
                let ing_icon_y = section_y - 14.0 * s;
                self.draw_item_icon(
                    &ingredient.item_id,
                    ing_icon_x,
                    ing_icon_y,
                    ing_icon_size,
                    ing_icon_size,
                    state,
                    false,
                );

                let display_name = state.item_registry.get_display_name(&ingredient.item_id);
                let text = format!(
                    "{} {} ({}/{})",
                    marker, display_name, have_count, need_count
                );
                self.draw_text_sharp(
                    &text,
                    detail_x + 20.0 * s + ing_icon_size + 6.0 * s,
                    section_y + 2.0 * s,
                    16.0,
                    color,
                );
                section_y += 32.0 * s;
            }

            if missing_ingredients && craft_blocked_reason.is_none() {
                craft_blocked_reason = Some("Missing Materials");
            }
            let can_craft = craft_blocked_reason.is_none();

            // ===== CRAFT BUTTON =====
            let btn_height = 28.0 * s;
            let btn_y = detail_y + detail_height - 38.0 * s;
            let btn_width = 140.0 * s;
            let btn_x = detail_x + (detail_width - btn_width) / 2.0;

            if can_craft {
                let bounds = Rect::new(btn_x, btn_y, btn_width, btn_height);
                layout.add(UiElementId::CraftingButton, bounds);
            }

            let is_btn_hovered = can_craft && matches!(hovered, Some(UiElementId::CraftingButton));

            if can_craft {
                let (btn_bg, btn_border) = if is_btn_hovered {
                    (
                        Color::new(0.2, 0.5, 0.2, 1.0),
                        Color::new(0.3, 0.7, 0.3, 1.0),
                    )
                } else {
                    (
                        Color::new(0.15, 0.4, 0.15, 1.0),
                        Color::new(0.25, 0.6, 0.25, 1.0),
                    )
                };

                draw_rectangle(btn_x, btn_y, btn_width, btn_height, btn_border);
                draw_rectangle(
                    btn_x + 1.0,
                    btn_y + 1.0,
                    btn_width - 2.0,
                    btn_height - 2.0,
                    btn_bg,
                );

                draw_line(
                    btn_x + 2.0,
                    btn_y + 2.0,
                    btn_x + btn_width - 2.0,
                    btn_y + 2.0,
                    1.0,
                    Color::new(0.4, 0.8, 0.4, 1.0),
                );
                draw_line(
                    btn_x + 2.0,
                    btn_y + 2.0,
                    btn_x + 2.0,
                    btn_y + btn_height - 2.0,
                    1.0,
                    Color::new(0.4, 0.8, 0.4, 1.0),
                );

                let craft_text = "[ CRAFT ]";
                let text_w = self.measure_text_sharp(craft_text, 16.0).width;
                self.draw_text_sharp(
                    craft_text,
                    btn_x + (btn_width - text_w) / 2.0,
                    btn_y + btn_height * 0.68,
                    16.0,
                    WHITE,
                );
            } else {
                draw_rectangle(btn_x, btn_y, btn_width, btn_height, SLOT_BORDER);
                draw_rectangle(
                    btn_x + 1.0,
                    btn_y + 1.0,
                    btn_width - 2.0,
                    btn_height - 2.0,
                    Color::new(0.125, 0.094, 0.094, 1.0),
                );

                let text = craft_blocked_reason.unwrap_or("Missing Materials");
                let text_w = self.measure_text_sharp(text, 16.0).width;
                self.draw_text_sharp(
                    text,
                    btn_x + (btn_width - text_w) / 2.0,
                    btn_y + btn_height * 0.68,
                    16.0,
                    Color::new(0.502, 0.314, 0.314, 1.0),
                );
            }
        } else {
            self.draw_text_sharp(
                "Select a recipe",
                detail_x + 12.0 * s,
                detail_y + 24.0 * s,
                16.0,
                TEXT_DIM,
            );
        }
    }

    /// Task 14: Render crafting progress overlay on the detail panel
    fn render_crafting_progress(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        detail_x: f32,
        detail_y: f32,
        detail_width: f32,
        detail_height: f32,
    ) {
        let s = self.font_scale.get();
        let progress = state.ui_state.crafting_progress;

        // "CRAFTING..." text centered, with pulsing ellipsis
        let time = get_time() as f32;
        let dots = match ((time * 2.0) as i32) % 4 {
            0 => "CRAFTING",
            1 => "CRAFTING.",
            2 => "CRAFTING..",
            _ => "CRAFTING...",
        };
        let crafting_dims = self.measure_text_sharp(dots, 16.0);
        let text_x = detail_x + (detail_width - crafting_dims.width) / 2.0;
        self.draw_text_sharp(dots, text_x, detail_y + 40.0 * s, 16.0, TEXT_TITLE);

        // Show the result item name and sprite if we can find the recipe
        if let Some(ref recipe_id) = state.ui_state.crafting_recipe_id {
            if let Some(recipe) = state.recipe_definitions.iter().find(|r| &r.id == recipe_id) {
                // Draw a centered result item sprite (48x48) below the CRAFTING text
                let progress_icon_size = 48.0 * s;
                if let Some(result) = recipe.results.first() {
                    let icon_x = detail_x + (detail_width - progress_icon_size) / 2.0;
                    let icon_y = detail_y + 50.0 * s;
                    self.draw_item_icon(
                        &result.item_id,
                        icon_x,
                        icon_y,
                        progress_icon_size,
                        progress_icon_size,
                        state,
                        true,
                    );
                }

                // Recipe name below the sprite
                let name_dims = self.measure_text_sharp(&recipe.display_name, 16.0);
                let name_x = detail_x + (detail_width - name_dims.width) / 2.0;
                // Pulsing effect on the item name
                let pulse = (time * 3.0).sin() * 0.15 + 0.85;
                let pulse_color = Color::new(
                    CATEGORY_EQUIPMENT.r * pulse,
                    CATEGORY_EQUIPMENT.g * pulse,
                    CATEGORY_EQUIPMENT.b * pulse,
                    1.0,
                );
                self.draw_text_sharp(
                    &recipe.display_name,
                    name_x,
                    detail_y + 50.0 * s + progress_icon_size + 16.0 * s,
                    16.0,
                    pulse_color,
                );

                // Show what it creates
                if let Some(result) = recipe.results.first() {
                    let result_name = state.item_registry.get_display_name(&result.item_id);
                    let result_text = format!("Creating: {} x{}", result_name, result.count);
                    let result_dims = self.measure_text_sharp(&result_text, 16.0);
                    let result_x = detail_x + (detail_width - result_dims.width) / 2.0;
                    self.draw_text_sharp(
                        &result_text,
                        result_x,
                        detail_y + 50.0 * s + progress_icon_size + 36.0 * s,
                        16.0,
                        TEXT_NORMAL,
                    );
                }
            }
        }

        // Progress bar
        let bar_width = detail_width - 40.0 * s;
        let bar_height = 20.0 * s;
        let bar_x = detail_x + 20.0 * s;
        let bar_y = detail_y + detail_height / 2.0 - bar_height / 2.0 + 10.0 * s;

        // Bar background
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

        // Bar fill
        let fill_width = (bar_width - 4.0) * progress;
        if fill_width > 0.0 {
            // Gradient-like fill using two rectangles
            let fill_x = bar_x + 2.0;
            let fill_y = bar_y + 2.0;
            let fill_h = bar_height - 4.0;

            draw_rectangle(
                fill_x,
                fill_y,
                fill_width,
                fill_h,
                Color::new(0.15, 0.4, 0.15, 1.0),
            );
            // Brighter top half
            draw_rectangle(
                fill_x,
                fill_y,
                fill_width,
                fill_h / 2.0,
                Color::new(0.2, 0.55, 0.2, 1.0),
            );
            // Highlight line at top
            draw_line(
                fill_x,
                fill_y,
                fill_x + fill_width,
                fill_y,
                1.0,
                Color::new(0.35, 0.75, 0.35, 1.0),
            );
        }

        // Percentage text below bar
        let pct_text = format!("{}%", (progress * 100.0) as i32);
        let pct_dims = self.measure_text_sharp(&pct_text, 16.0);
        let pct_x = detail_x + (detail_width - pct_dims.width) / 2.0;
        self.draw_text_sharp(
            &pct_text,
            pct_x,
            bar_y + bar_height + 20.0 * s,
            16.0,
            TEXT_NORMAL,
        );

        // CANCEL button
        let cancel_btn_width = 120.0 * s;
        let cancel_btn_height = 28.0 * s;
        let cancel_btn_x = detail_x + (detail_width - cancel_btn_width) / 2.0;
        let cancel_btn_y = detail_y + detail_height - 42.0 * s;

        let cancel_bounds = Rect::new(
            cancel_btn_x,
            cancel_btn_y,
            cancel_btn_width,
            cancel_btn_height,
        );
        layout.add(UiElementId::CraftingCancelButton, cancel_bounds);

        let is_cancel_hovered = matches!(hovered, Some(UiElementId::CraftingCancelButton));
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

        draw_rectangle(
            cancel_btn_x,
            cancel_btn_y,
            cancel_btn_width,
            cancel_btn_height,
            cancel_border,
        );
        draw_rectangle(
            cancel_btn_x + 1.0,
            cancel_btn_y + 1.0,
            cancel_btn_width - 2.0,
            cancel_btn_height - 2.0,
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
            cancel_btn_x + (cancel_btn_width - cancel_text_w) / 2.0,
            cancel_btn_y + cancel_btn_height * 0.68,
            16.0,
            cancel_text_color,
        );
    }

    /// Task 20: Render crafting completion animation overlay
    fn render_crafting_complete(
        &self,
        state: &GameState,
        recipe_id: &str,
        timer: f32,
        detail_x: f32,
        detail_y: f32,
        detail_width: f32,
        detail_height: f32,
    ) {
        let s_ui = self.font_scale.get();
        // timer goes from 0.0 to 1.0 over ~1 second
        let alpha = 1.0 - timer; // fade out

        // "Crafted!" text with scale-up pop effect
        let scale = if timer < 0.2 {
            // Pop in: scale from 0.5 to 1.2
            0.5 + (timer / 0.2) * 0.7
        } else if timer < 0.35 {
            // Settle: scale from 1.2 to 1.0
            1.2 - ((timer - 0.2) / 0.15) * 0.2
        } else {
            1.0
        };

        let crafted_text = "Crafted!";
        let font_size = 16.0 * scale;
        let crafted_dims = self.measure_text_sharp(crafted_text, font_size);
        let crafted_x = detail_x + (detail_width - crafted_dims.width) / 2.0;
        let crafted_y = detail_y + detail_height / 2.0 - 20.0 * s_ui;

        let text_color = Color::new(0.392, 0.784, 0.392, alpha);
        self.draw_text_sharp(crafted_text, crafted_x, crafted_y, font_size, text_color);

        // Show the item name below
        if let Some(recipe) = state.recipe_definitions.iter().find(|r| r.id == recipe_id) {
            let name_dims = self.measure_text_sharp(&recipe.display_name, 16.0);
            let name_x = detail_x + (detail_width - name_dims.width) / 2.0;
            let name_color = Color::new(
                CATEGORY_EQUIPMENT.r,
                CATEGORY_EQUIPMENT.g,
                CATEGORY_EQUIPMENT.b,
                alpha,
            );
            self.draw_text_sharp(
                &recipe.display_name,
                name_x,
                crafted_y + 25.0 * s_ui,
                16.0,
                name_color,
            );
        }
    }
}
