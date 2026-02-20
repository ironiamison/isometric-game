//! Anvil panel rendering — grid-based smithing UI

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

// Progress bar colors (steel-blue theme for smithing)
const ANVIL_PROGRESS_DARK: Color = Color::new(0.20, 0.30, 0.50, 1.0);
const ANVIL_PROGRESS_MID: Color = Color::new(0.30, 0.45, 0.65, 1.0);
const ANVIL_PROGRESS_LIGHT: Color = Color::new(0.45, 0.60, 0.80, 1.0);

impl Renderer {
    pub(crate) fn render_anvil(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (sw, sh) = virtual_screen_size();

        let panel_width = (560.0_f32).min(sw - 16.0);
        let panel_height = (480.0_f32).min(sh - 16.0);
        let panel_x = (sw - panel_width) / 2.0;
        let panel_y = (sh - panel_height) / 2.0;

        // Semi-transparent overlay
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.588));

        // Panel frame + corner accents
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        // ===== HEADER =====
        let header_x = panel_x + FRAME_THICKNESS;
        let header_y = panel_y + FRAME_THICKNESS;
        let header_w = panel_width - FRAME_THICKNESS * 2.0;

        draw_rectangle(header_x, header_y, header_w, HEADER_HEIGHT, PANEL_BG_MID);
        draw_line(
            header_x + 10.0,
            header_y + HEADER_HEIGHT,
            header_x + header_w - 10.0,
            header_y + HEADER_HEIGHT,
            2.0,
            HEADER_BORDER,
        );

        // Decorative dots along header border
        let dot_spacing = 60.0;
        let num_dots = ((header_w - 40.0) / dot_spacing) as i32;
        let start_dot_x = header_x + 20.0;
        for i in 0..num_dots {
            let dot_x = start_dot_x + i as f32 * dot_spacing;
            draw_rectangle(
                dot_x - 1.5,
                header_y + HEADER_HEIGHT - 1.5,
                3.0,
                3.0,
                FRAME_ACCENT,
            );
        }

        // Title
        let title = "ANVIL";
        let title_dims = self.measure_text_sharp(title, 16.0);
        self.draw_text_sharp(
            title,
            header_x + (header_w - title_dims.width) / 2.0,
            header_y + 26.0,
            16.0,
            TEXT_TITLE,
        );

        // Close button (X)
        let is_mobile = cfg!(target_os = "android");
        let close_btn_size = if is_mobile { 32.0 } else { 28.0 };
        let close_btn_x = header_x + header_w - close_btn_size - 6.0;
        let close_btn_y = header_y + (HEADER_HEIGHT - close_btn_size) / 2.0;
        let close_bounds = Rect::new(close_btn_x, close_btn_y, close_btn_size, close_btn_size);
        layout.add(UiElementId::AnvilCloseButton, close_bounds);

        let is_close_hovered = matches!(hovered, Some(UiElementId::AnvilCloseButton));
        let (close_bg, close_border) = if is_close_hovered {
            (
                Color::new(0.4, 0.15, 0.15, 1.0),
                Color::new(0.6, 0.2, 0.2, 1.0),
            )
        } else {
            (Color::new(0.2, 0.1, 0.1, 1.0), FRAME_MID)
        };
        draw_rectangle(close_btn_x, close_btn_y, close_btn_size, close_btn_size, close_border);
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
        let cross_color = if is_close_hovered { TEXT_TITLE } else { TEXT_DIM };
        draw_line(cx - cross, cy - cross, cx + cross, cy + cross, 2.0, cross_color);
        draw_line(cx + cross, cy - cross, cx - cross, cy + cross, 2.0, cross_color);

        // ===== TABS =====
        let tab_y = header_y + HEADER_HEIGHT + 2.0;
        let tab_h = TAB_HEIGHT;
        let tab_w = header_w / 2.0;
        let active_tab = state.ui_state.anvil_tab;

        let tab_labels = ["Materials", "Equipment"];
        let tab_ids = [UiElementId::AnvilTabMaterials, UiElementId::AnvilTabEquipment];

        for (idx, (label, id)) in tab_labels.iter().zip(tab_ids.iter()).enumerate() {
            let tx = header_x + idx as f32 * tab_w;
            let is_active = idx as u8 == active_tab;

            let bounds = Rect::new(tx, tab_y, tab_w, tab_h);
            layout.add(id.clone(), bounds);

            let (tab_bg, tab_text_color) = if is_active {
                (PANEL_BG_MID, TEXT_TITLE)
            } else {
                (SLOT_BG_EMPTY, TEXT_DIM)
            };

            draw_rectangle(tx, tab_y, tab_w, tab_h, tab_bg);

            if idx > 0 {
                draw_line(tx, tab_y + 4.0, tx, tab_y + tab_h - 4.0, 1.0, SLOT_BORDER);
            }

            if is_active {
                draw_line(tx + 4.0, tab_y + tab_h - 1.0, tx + tab_w - 4.0, tab_y + tab_h - 1.0, 2.0, SLOT_SELECTED_BORDER);
            } else {
                draw_line(tx + 4.0, tab_y + tab_h - 1.0, tx + tab_w - 4.0, tab_y + tab_h - 1.0, 1.0, SLOT_BORDER);
            }

            let label_dims = self.measure_text_sharp(label, TAB_FONT_SIZE);
            self.draw_text_sharp(
                label,
                tx + (tab_w - label_dims.width) / 2.0,
                tab_y + 19.0,
                TAB_FONT_SIZE,
                tab_text_color,
            );
        }

        // ===== CONTENT AREA =====
        let content_x = panel_x + FRAME_THICKNESS + 8.0;
        let content_y = tab_y + tab_h + 4.0;
        let content_w = panel_width - FRAME_THICKNESS * 2.0 - 16.0;
        let controls_strip_h = 44.0; // space reserved for controls strip above footer
        let full_content_h = panel_y + panel_height - FRAME_THICKNESS - FOOTER_HEIGHT - 4.0 - content_y;

        if state.ui_state.crafting_in_progress {
            self.render_anvil_progress(state, hovered, layout, content_x, content_y, content_w, full_content_h);
        } else {
            let content_h = full_content_h - controls_strip_h;
            self.render_anvil_recipe_grid(state, hovered, layout, content_x, content_y, content_w, content_h);
        }

        // ===== FOOTER =====
        let footer_x = panel_x + FRAME_THICKNESS;
        let footer_y = panel_y + panel_height - FRAME_THICKNESS - FOOTER_HEIGHT;
        let footer_w = panel_width - FRAME_THICKNESS * 2.0;

        draw_rectangle(footer_x, footer_y, footer_w, FOOTER_HEIGHT, FOOTER_BG);
        draw_line(
            footer_x + 10.0,
            footer_y,
            footer_x + footer_w - 10.0,
            footer_y,
            1.0,
            HEADER_BORDER,
        );

        if state.ui_state.crafting_in_progress {
            self.draw_text_sharp("[Esc] Cancel", footer_x + 10.0, footer_y + 20.0, 16.0, TEXT_DIM);
        } else {
            self.draw_text_sharp("[Tab] Tab", footer_x + 10.0, footer_y + 20.0, 16.0, TEXT_DIM);
            self.draw_text_sharp("[Arrows] Select", footer_x + 105.0, footer_y + 20.0, 16.0, TEXT_DIM);
            self.draw_text_sharp("[1/X/A] Qty", footer_x + 240.0, footer_y + 20.0, 16.0, TEXT_DIM);
            self.draw_text_sharp("[Enter] Smith", footer_x + 355.0, footer_y + 20.0, 16.0, TEXT_DIM);
        }
    }

    /// Render the grid of anvil recipes
    fn render_anvil_recipe_grid(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        content_x: f32,
        content_y: f32,
        content_w: f32,
        content_h: f32,
    ) {
        let section_filter = if state.ui_state.anvil_tab == 0 { "materials" } else { "equipment" };
        let mut anvil_recipes: Vec<_> = state
            .recipe_definitions
            .iter()
            .filter(|r| r.station.as_deref() == Some("anvil"))
            .filter(|r| r.section.as_deref() == Some(section_filter))
            .filter(|r| !r.requires_discovery || state.discovered_recipes.contains(&r.id))
            .collect();
        anvil_recipes.sort_by_key(|r| r.level_required);

        if anvil_recipes.is_empty() {
            let msg = if state.ui_state.anvil_tab == 0 {
                "No material recipes available"
            } else {
                "No equipment recipes available"
            };
            self.draw_text_sharp(msg, content_x + 20.0, content_y + 40.0, 16.0, TEXT_DIM);
            return;
        }

        // Grid layout: 4 columns
        let columns = 4;
        let gap = 6.0;
        let cell_w = (content_w - (columns as f32 - 1.0) * gap) / columns as f32;
        let cell_h = 90.0;

        let rows = (anvil_recipes.len() + columns - 1) / columns;
        let total_content = rows as f32 * (cell_h + gap);
        let max_scroll = (total_content - content_h).max(0.0);
        let scroll_offset = state.ui_state.anvil_scroll_offset.clamp(0.0, max_scroll);

        // Register scroll area
        let scroll_bounds = Rect::new(content_x, content_y, content_w, content_h);
        layout.add(UiElementId::AnvilScrollArea, scroll_bounds);

        // Scissor clipping
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

        for (i, recipe) in anvil_recipes.iter().enumerate() {
            let col = i % columns;
            let row = i / columns;

            let cell_x = content_x + col as f32 * (cell_w + gap);
            let cell_y = content_y + row as f32 * (cell_h + gap) - scroll_offset;

            // Skip cells outside visible area
            if cell_y + cell_h < content_y || cell_y > content_y + content_h {
                continue;
            }

            let is_selected = i == state.ui_state.anvil_selected_recipe;
            let is_hovered = matches!(hovered, Some(UiElementId::AnvilRecipeCell(idx)) if *idx == i);

            // Cell background
            let cell_bg = if is_selected {
                SLOT_HOVER_BG
            } else if is_hovered {
                Color::new(0.141, 0.141, 0.188, 1.0)
            } else {
                SLOT_BG_EMPTY
            };

            let cell_border = if is_selected {
                SLOT_SELECTED_BORDER
            } else if is_hovered {
                SLOT_HOVER_BORDER
            } else {
                SLOT_BORDER
            };

            // Draw cell border + background
            draw_rectangle(cell_x, cell_y, cell_w, cell_h, cell_border);
            draw_rectangle(cell_x + 1.0, cell_y + 1.0, cell_w - 2.0, cell_h - 2.0, cell_bg);

            // Inner shadow top
            draw_line(
                cell_x + 2.0,
                cell_y + 2.0,
                cell_x + cell_w - 2.0,
                cell_y + 2.0,
                1.0,
                SLOT_INNER_SHADOW,
            );

            // Register click area
            let cell_bounds = Rect::new(cell_x, cell_y, cell_w, cell_h);
            layout.add(UiElementId::AnvilRecipeCell(i), cell_bounds);

            // Smithing level badge (top-left corner)
            if recipe.level_required > 0 {
                let badge_icon_size = 14.0;
                let badge_x = cell_x + 3.0;
                let badge_y = cell_y + 3.0;

                // Draw small smithing icon from spritesheet
                let drew_icon = if let Some(ref texture) = self.ui_icons {
                    let src_x = 5.0 * 24.0; // Smithing = col 5
                    let src_y = 6.0 * 24.0; // row 6
                    draw_texture_ex(
                        texture,
                        badge_x,
                        badge_y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(src_x, src_y, 24.0, 24.0)),
                            dest_size: Some(Vec2::new(badge_icon_size, badge_icon_size)),
                            ..Default::default()
                        },
                    );
                    true
                } else {
                    false
                };

                if !drew_icon {
                    // Fallback: colored "Sm" letter
                    self.draw_text_sharp("Sm", badge_x, badge_y + 11.0, 16.0, Color::new(0.7, 0.5, 0.2, 1.0));
                }

                // Level number next to icon
                let lvl_text = format!("{}", recipe.level_required);
                self.draw_text_sharp(&lvl_text, badge_x + badge_icon_size + 1.0, badge_y + 12.0, 16.0, TEXT_DIM);
            }

            // Icon (centered horizontally, near top)
            let icon_size = 40.0;
            let icon_x = cell_x + (cell_w - icon_size) / 2.0;
            let icon_y = cell_y + 6.0;
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

            // Item name (centered, below icon)
            let name = &recipe.display_name;
            let name_dims = self.measure_text_sharp(name, 16.0);
            let name_color = if is_selected { TEXT_TITLE } else { TEXT_NORMAL };
            // Truncate name if too wide
            let display_name = if name_dims.width > cell_w - 8.0 {
                let mut truncated = name.clone();
                while self.measure_text_sharp(&truncated, 16.0).width > cell_w - 16.0 && truncated.len() > 3 {
                    truncated.pop();
                }
                truncated.push_str("..");
                truncated
            } else {
                name.clone()
            };
            let display_dims = self.measure_text_sharp(&display_name, 16.0);
            self.draw_text_sharp(
                &display_name,
                cell_x + (cell_w - display_dims.width) / 2.0,
                cell_y + icon_size + 18.0,
                16.0,
                name_color,
            );

            // Ingredient cost (centered, below name)
            let mut can_craft = true;
            let mut ing_parts: Vec<String> = Vec::new();
            for ing in &recipe.ingredients {
                let have = state.inventory.count_item_by_id(&ing.item_id);
                let name = state.item_registry.get_display_name(&ing.item_id);
                ing_parts.push(format!("{}x {}", ing.count, name));
                if have < ing.count {
                    can_craft = false;
                }
            }
            // Check level requirement
            if recipe.level_required > 1 {
                if let Some(player) = state.get_local_player() {
                    if player.skills.smithing.level < recipe.level_required {
                        can_craft = false;
                    }
                }
            }
            let ing_text = ing_parts.join(", ");
            let ing_color = if can_craft {
                Color::new(0.392, 0.784, 0.392, 1.0)
            } else {
                Color::new(0.784, 0.314, 0.314, 1.0)
            };
            // Truncate ingredients if too wide
            let ing_display = if self.measure_text_sharp(&ing_text, 16.0).width > cell_w - 8.0 {
                let mut truncated = ing_text.clone();
                while self.measure_text_sharp(&truncated, 16.0).width > cell_w - 16.0 && truncated.len() > 3 {
                    truncated.pop();
                }
                truncated.push_str("..");
                truncated
            } else {
                ing_text
            };
            let ing_dims = self.measure_text_sharp(&ing_display, 16.0);
            self.draw_text_sharp(
                &ing_display,
                cell_x + (cell_w - ing_dims.width) / 2.0,
                cell_y + icon_size + 34.0,
                16.0,
                ing_color,
            );
        }

        // Disable scissor
        {
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            gl.quad_gl.scissor(None);
        }

        // Scrollbar
        if max_scroll > 0.0 {
            let scrollbar_track_h = content_h - 4.0;
            let scrollbar_x = content_x + content_w - 8.0;
            let scrollbar_y = content_y + 2.0;

            draw_rectangle(scrollbar_x, scrollbar_y, 4.0, scrollbar_track_h, Color::new(0.1, 0.08, 0.06, 1.0));

            let visible_ratio = (content_h / total_content).min(1.0);
            let thumb_h = (scrollbar_track_h * visible_ratio).max(16.0);
            let scroll_ratio = if max_scroll > 0.0 { scroll_offset / max_scroll } else { 0.0 };
            let thumb_y = scrollbar_y + scroll_ratio * (scrollbar_track_h - thumb_h);
            draw_rectangle(scrollbar_x, thumb_y, 4.0, thumb_h, SLOT_BORDER);
        }

        // ===== CONTROLS (below grid) =====
        // Only show when not crafting
        self.render_anvil_controls(state, hovered, layout, &anvil_recipes, content_x, content_y + content_h + 2.0, content_w);
    }

    /// Render quantity buttons and SMITH button below the grid
    fn render_anvil_controls(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        anvil_recipes: &[&crate::game::RecipeDefinition],
        area_x: f32,
        area_y: f32,
        _area_w: f32,
    ) {
        // Controls are placed in the footer area, but we need space
        // Actually let's place them inside the content area at the bottom
        // The footer already has keybind hints. Put controls just above footer.

        let controls_h = 34.0;
        let controls_y = area_y - controls_h - 2.0;

        // Background strip behind controls
        draw_rectangle(area_x, controls_y - 4.0, _area_w, controls_h + 8.0, PANEL_BG_DARK);
        draw_line(area_x + 8.0, controls_y - 4.0, area_x + _area_w - 8.0, controls_y - 4.0, 1.0, SLOT_BORDER);

        // Quantity buttons: [1] [X] [All]
        let qty_btn_w = 32.0;
        let qty_btn_h = 24.0;
        let qty_labels = ["1", "X", "All"];
        let qty_ids = [
            UiElementId::AnvilQuantity1,
            UiElementId::AnvilQuantityX,
            UiElementId::AnvilQuantityAll,
        ];

        let controls_start_x = area_x + 8.0;

        for (j, (label, id)) in qty_labels.iter().zip(qty_ids.iter()).enumerate() {
            let bx = controls_start_x + j as f32 * (qty_btn_w + 4.0);
            let bounds = Rect::new(bx, controls_y, qty_btn_w, qty_btn_h);
            layout.add(id.clone(), bounds);

            let is_qty_active = match j {
                0 => state.ui_state.anvil_quantity == 1,
                2 => state.ui_state.anvil_quantity == u32::MAX,
                _ => state.ui_state.anvil_quantity != 1 && state.ui_state.anvil_quantity != u32::MAX,
            };
            let is_qty_hovered = hovered.as_ref() == Some(id);

            let (bg, border) = if is_qty_active {
                (Color::new(0.188, 0.188, 0.282, 1.0), SLOT_SELECTED_BORDER)
            } else if is_qty_hovered {
                (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
            } else {
                (SLOT_BG_EMPTY, SLOT_BORDER)
            };

            draw_rectangle(bx, controls_y, qty_btn_w, qty_btn_h, border);
            draw_rectangle(bx + 1.0, controls_y + 1.0, qty_btn_w - 2.0, qty_btn_h - 2.0, bg);

            let label_dims = self.measure_text_sharp(label, 16.0);
            let text_color = if is_qty_active { TEXT_TITLE } else { TEXT_DIM };
            self.draw_text_sharp(
                label,
                bx + (qty_btn_w - label_dims.width) / 2.0,
                controls_y + 17.0,
                16.0,
                text_color,
            );
        }

        // Quantity display
        let qty_display = if state.ui_state.anvil_quantity == u32::MAX {
            "All".to_string()
        } else {
            format!("x{}", state.ui_state.anvil_quantity)
        };
        let qty_disp_x = controls_start_x + 3.0 * (qty_btn_w + 4.0) + 4.0;
        self.draw_text_sharp(&qty_display, qty_disp_x, controls_y + 17.0, 16.0, TEXT_NORMAL);

        // Check if selected recipe can be crafted
        let mut can_craft = false;
        if let Some(recipe) = anvil_recipes.get(state.ui_state.anvil_selected_recipe) {
            can_craft = true;
            for ing in &recipe.ingredients {
                let have = state.inventory.count_item_by_id(&ing.item_id);
                if have < ing.count {
                    can_craft = false;
                    break;
                }
            }
            if recipe.level_required > 1 {
                if let Some(player) = state.get_local_player() {
                    if player.skills.smithing.level < recipe.level_required {
                        can_craft = false;
                    }
                }
            }
        }

        // SMITH button
        let smith_btn_w = 110.0;
        let smith_btn_h = 26.0;
        let smith_btn_x = qty_disp_x + 50.0;
        let smith_btn_y = controls_y - 1.0;

        if can_craft {
            let bounds = Rect::new(smith_btn_x, smith_btn_y, smith_btn_w, smith_btn_h);
            layout.add(UiElementId::AnvilSmithButton, bounds);
        }

        let is_smith_hovered = can_craft && matches!(hovered, Some(UiElementId::AnvilSmithButton));
        let (btn_bg, btn_border) = if !can_craft {
            (Color::new(0.12, 0.08, 0.06, 1.0), SLOT_BORDER)
        } else if is_smith_hovered {
            (Color::new(0.15, 0.35, 0.55, 1.0), Color::new(0.25, 0.50, 0.70, 1.0))
        } else {
            (Color::new(0.12, 0.28, 0.45, 1.0), Color::new(0.20, 0.42, 0.60, 1.0))
        };

        draw_rectangle(smith_btn_x, smith_btn_y, smith_btn_w, smith_btn_h, btn_border);
        draw_rectangle(
            smith_btn_x + 1.0,
            smith_btn_y + 1.0,
            smith_btn_w - 2.0,
            smith_btn_h - 2.0,
            btn_bg,
        );

        if can_craft {
            draw_line(
                smith_btn_x + 2.0,
                smith_btn_y + 2.0,
                smith_btn_x + smith_btn_w - 2.0,
                smith_btn_y + 2.0,
                1.0,
                Color::new(0.30, 0.55, 0.75, 1.0),
            );
        }

        let smith_text = if can_craft { "[ SMITH ]" } else { "Can't Smith" };
        let smith_text_w = self.measure_text_sharp(smith_text, 16.0).width;
        let smith_text_color = if !can_craft {
            Color::new(0.5, 0.3, 0.3, 1.0)
        } else if is_smith_hovered {
            WHITE
        } else {
            Color::new(0.35, 0.60, 0.80, 1.0)
        };
        self.draw_text_sharp(
            smith_text,
            smith_btn_x + (smith_btn_w - smith_text_w) / 2.0,
            smith_btn_y + 18.0,
            16.0,
            smith_text_color,
        );
    }

    /// Render smithing progress overlay
    fn render_anvil_progress(
        &self,
        state: &GameState,
        _hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        area_x: f32,
        area_y: f32,
        area_w: f32,
        area_h: f32,
    ) {
        let progress = state.ui_state.crafting_progress;
        let time = get_time() as f32;

        // "SMITHING..." text with pulsing ellipsis
        let dots = match ((time * 2.0) as i32) % 4 {
            0 => "SMITHING",
            1 => "SMITHING.",
            2 => "SMITHING..",
            _ => "SMITHING...",
        };
        let dots_dims = self.measure_text_sharp(dots, 16.0);
        self.draw_text_sharp(
            dots,
            area_x + (area_w - dots_dims.width) / 2.0,
            area_y + 36.0,
            16.0,
            TEXT_TITLE,
        );

        // Show result item icon + name
        if let Some(ref recipe_id) = state.ui_state.crafting_recipe_id {
            if let Some(recipe) = state.recipe_definitions.iter().find(|r| &r.id == recipe_id) {
                let icon_size = 48.0;
                if let Some(result) = recipe.results.first() {
                    let icon_x = area_x + (area_w - icon_size) / 2.0;
                    self.draw_item_icon(
                        &result.item_id,
                        icon_x,
                        area_y + 46.0,
                        icon_size,
                        icon_size,
                        state,
                        true,
                    );
                }

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
                    area_y + 46.0 + icon_size + 16.0,
                    16.0,
                    pulse_color,
                );
            }
        }

        // Batch counter
        if state.ui_state.batch_total > 1 {
            let batch_text = format!(
                "{}/{}",
                state.ui_state.batch_completed,
                state.ui_state.batch_total
            );
            let batch_dims = self.measure_text_sharp(&batch_text, 16.0);
            self.draw_text_sharp(
                &batch_text,
                area_x + (area_w - batch_dims.width) / 2.0,
                area_y + 130.0,
                16.0,
                TEXT_NORMAL,
            );
        }

        // Progress bar (steel-blue theme)
        let bar_width = area_w - 60.0;
        let bar_height = 20.0;
        let bar_x = area_x + 30.0;
        let bar_y = area_y + area_h / 2.0 - bar_height / 2.0 + 10.0;

        draw_rectangle(bar_x, bar_y, bar_width, bar_height, SLOT_BORDER);
        draw_rectangle(bar_x + 1.0, bar_y + 1.0, bar_width - 2.0, bar_height - 2.0, SLOT_BG_EMPTY);
        draw_line(bar_x + 2.0, bar_y + 2.0, bar_x + bar_width - 2.0, bar_y + 2.0, 1.0, SLOT_INNER_SHADOW);

        let fill_width = (bar_width - 4.0) * progress;
        if fill_width > 0.0 {
            let fill_x = bar_x + 2.0;
            let fill_y = bar_y + 2.0;
            let fill_h = bar_height - 4.0;

            draw_rectangle(fill_x, fill_y, fill_width, fill_h, ANVIL_PROGRESS_DARK);
            draw_rectangle(fill_x, fill_y, fill_width, fill_h / 2.0, ANVIL_PROGRESS_MID);
            draw_line(fill_x, fill_y, fill_x + fill_width, fill_y, 1.0, ANVIL_PROGRESS_LIGHT);
        }

        // Percentage
        let pct_text = format!("{}%", (progress * 100.0) as i32);
        let pct_dims = self.measure_text_sharp(&pct_text, 16.0);
        self.draw_text_sharp(
            &pct_text,
            area_x + (area_w - pct_dims.width) / 2.0,
            bar_y + bar_height + 20.0,
            16.0,
            TEXT_NORMAL,
        );

        // Cancel button
        let cancel_w = 120.0;
        let cancel_h = 28.0;
        let cancel_x = area_x + (area_w - cancel_w) / 2.0;
        let cancel_y = area_y + area_h - 42.0;

        let cancel_bounds = Rect::new(cancel_x, cancel_y, cancel_w, cancel_h);
        layout.add(UiElementId::AnvilCancelButton, cancel_bounds);

        let is_cancel_hovered = matches!(_hovered, Some(UiElementId::AnvilCancelButton));
        let (cancel_bg, cancel_border) = if is_cancel_hovered {
            (Color::new(0.45, 0.15, 0.15, 1.0), Color::new(0.6, 0.2, 0.2, 1.0))
        } else {
            (Color::new(0.35, 0.12, 0.12, 1.0), Color::new(0.5, 0.18, 0.18, 1.0))
        };

        draw_rectangle(cancel_x, cancel_y, cancel_w, cancel_h, cancel_border);
        draw_rectangle(cancel_x + 1.0, cancel_y + 1.0, cancel_w - 2.0, cancel_h - 2.0, cancel_bg);

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
            cancel_y + 19.0,
            16.0,
            cancel_text_color,
        );
    }
}
