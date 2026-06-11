//! Crafting panel rendering

use super::super::Renderer;
use super::common::*;
use crate::game::{GameState, RecipeDefinition};
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

mod progress;
mod recipes;

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
        let footer_h = if cfg!(target_os = "android") {
            0.0
        } else {
            FOOTER_HEIGHT * s
        };
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
}
