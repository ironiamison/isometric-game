//! Fletching panel rendering — tool-based crafting UI (knife)

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

// Green-brown progress bar colors for woodworking theme
const FLETCH_PROGRESS_DARK: Color = Color::new(0.22, 0.40, 0.15, 1.0);
const FLETCH_PROGRESS_MID: Color = Color::new(0.32, 0.55, 0.22, 1.0);
const FLETCH_PROGRESS_LIGHT: Color = Color::new(0.45, 0.70, 0.30, 1.0);

/// Returns which recipe sections belong to each fletching tab.
pub fn fletching_sections_for_tab(tab: u8) -> &'static [&'static str] {
    match tab {
        0 => &["shafts"],
        1 => &["arrows"],
        2 => &["bows"],
        _ => &[],
    }
}

impl Renderer {
    pub(crate) fn render_fletching(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;

        let panel_width = (500.0 * s).min(sw - 16.0);
        let panel_height = (450.0 * s).min(sh - 16.0);
        let panel_x = (sw - panel_width) / 2.0;
        let panel_y = (sh - panel_height) / 2.0;

        // No background darkening — fletching is a quick tool-based panel

        // Panel frame + corner accents
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        let header_h = HEADER_HEIGHT * s;
        let footer_h = FOOTER_HEIGHT * s;

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
        let title = "FLETCHING";
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
        layout.add(UiElementId::FletchingCloseButton, close_bounds);

        let is_close_hovered = matches!(hovered, Some(UiElementId::FletchingCloseButton));
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
        let tab_h = 28.0 * s;
        let tab_count = 3;
        let tab_w = header_w / tab_count as f32;
        let tab_labels = ["Shafts", "Arrows", "Bows"];

        for (idx, label) in tab_labels.iter().enumerate() {
            let tx = header_x + idx as f32 * tab_w;
            let is_active = idx as u8 == state.ui_state.fletching_tab;
            let has_content = !fletching_sections_for_tab(idx as u8).is_empty();

            let bounds = Rect::new(tx, tab_y, tab_w, tab_h);
            layout.add(UiElementId::FletchingTab(idx), bounds);

            let (tab_bg, tab_text_color) = if is_active {
                (PANEL_BG_MID, TEXT_TITLE)
            } else if has_content {
                (SLOT_BG_EMPTY, Color::new(0.50, 0.50, 0.55, 0.8))
            } else {
                (SLOT_BG_EMPTY, Color::new(0.35, 0.35, 0.40, 0.6))
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

            // Active tab bottom accent (green for fletching)
            if is_active {
                draw_line(
                    tx + 4.0 * s,
                    tab_y + tab_h - 1.0,
                    tx + tab_w - 4.0 * s,
                    tab_y + tab_h - 1.0,
                    2.0,
                    Color::new(0.35, 0.75, 0.45, 1.0),
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

        // ===== CONTENT AREA =====
        let content_x = panel_x + FRAME_THICKNESS + 8.0 * s;
        let content_y = tab_y + tab_h + 4.0 * s;
        let content_w = panel_width - FRAME_THICKNESS * 2.0 - 16.0 * s;
        let content_h = panel_y + panel_height - FRAME_THICKNESS - footer_h - 4.0 * s - content_y;

        // If crafting is in progress, show progress overlay
        if state.ui_state.crafting_in_progress {
            self.render_fletching_progress(
                state, hovered, layout, content_x, content_y, content_w, content_h,
            );
        } else {
            self.render_fletching_recipe_list(
                state, hovered, layout, content_x, content_y, content_w, content_h,
            );
        }

        // ===== FOOTER =====
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

        if state.ui_state.crafting_in_progress {
            self.draw_text_sharp(
                "[Esc] Cancel",
                footer_x + 10.0 * s,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_DIM,
            );
        } else {
            self.draw_text_sharp(
                "[W/S] Select",
                footer_x + 10.0 * s,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_DIM,
            );
            self.draw_text_sharp(
                "[1/X/A] Qty",
                footer_x + 110.0 * s,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_DIM,
            );
            self.draw_text_sharp(
                "[Tab] Tab",
                footer_x + 210.0 * s,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_DIM,
            );
            self.draw_text_sharp(
                "[Enter] Fletch",
                footer_x + 310.0 * s,
                footer_y + footer_h * 0.67,
                16.0,
                TEXT_DIM,
            );
        }
    }

    /// Render the list of fletching recipes
    fn render_fletching_recipe_list(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        content_x: f32,
        content_y: f32,
        content_w: f32,
        content_h: f32,
    ) {
        // Filter recipes: fletching category with knife tool, filtered by active tab
        let tab_sections = fletching_sections_for_tab(state.ui_state.fletching_tab);
        let mut fletching_recipes: Vec<_> = state
            .recipe_definitions
            .iter()
            .filter(|r| r.category == "fletching" && r.required_tool.as_deref() == Some("knife"))
            .filter(|r| tab_sections.contains(&r.section.as_deref().unwrap_or("")))
            .collect();
        fletching_recipes.sort_by_key(|r| r.level_required);

        let s = self.font_scale.get();

        if fletching_recipes.is_empty() {
            self.draw_text_sharp(
                "No fletching recipes available",
                content_x + 20.0 * s,
                content_y + 40.0 * s,
                16.0,
                TEXT_DIM,
            );
            return;
        }

        // Register scroll area
        let scroll_bounds = Rect::new(content_x, content_y, content_w, content_h);
        layout.add(UiElementId::FletchingScrollArea, scroll_bounds);

        let row_height = 72.0 * s;
        let total_content = fletching_recipes.len() as f32 * row_height;
        let max_scroll = (total_content - content_h).max(0.0);
        let scroll_offset = state
            .ui_state
            .fletching_scroll_offset
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

        for (i, recipe) in fletching_recipes.iter().enumerate() {
            let row_top = y;
            let row_bottom = y + row_height;

            // Skip rows outside visible area
            if row_bottom < content_y || row_top > content_y + content_h {
                y += row_height;
                continue;
            }

            let is_selected = i == state.ui_state.fletching_selected_recipe;
            let is_hovered =
                matches!(hovered, Some(UiElementId::FletchingRecipeItem(idx)) if *idx == i);

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
                // Left accent bar
                draw_rectangle(
                    content_x + 2.0,
                    y + 4.0,
                    3.0,
                    row_height - 8.0,
                    SLOT_SELECTED_BORDER,
                );
            }

            // Register click area
            let row_bounds = Rect::new(content_x + 2.0, y + 1.0, content_w - 4.0, row_height - 2.0);
            layout.add(UiElementId::FletchingRecipeItem(i), row_bounds);

            // Icon (left side)
            let icon_size = 40.0 * s;
            let icon_x = content_x + 12.0 * s;
            let icon_y = y + (row_height - icon_size) / 2.0 - 2.0;
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
            self.draw_text_sharp(&recipe.display_name, text_x, y + 18.0 * s, 16.0, name_color);

            // Ingredients line
            let mut ing_parts: Vec<String> = Vec::new();
            let mut can_craft = true;
            for ing in &recipe.ingredients {
                let have = state.inventory.count_item_by_id(&ing.item_id);
                let name = state.item_registry.get_display_name(&ing.item_id);
                ing_parts.push(format!("{}x {}", ing.count, name));
                if have < ing.count {
                    can_craft = false;
                }
            }
            let ing_text = ing_parts.join(" + ");
            let ing_color = if can_craft {
                Color::new(0.392, 0.784, 0.392, 1.0)
            } else {
                Color::new(0.784, 0.314, 0.314, 1.0)
            };
            self.draw_text_sharp(&ing_text, text_x, y + 36.0 * s, 16.0, ing_color);

            // Level, XP, time info
            let mut info_x = text_x;
            let info_y = y + 52.0 * s;

            if recipe.level_required > 1 {
                // Determine level color: green if met, red if not
                let level_color = if let Some(player) = state.get_local_player() {
                    let player_level = player.skills.survivalist.level;
                    if player_level >= recipe.level_required {
                        Color::new(0.392, 0.784, 0.392, 1.0)
                    } else {
                        Color::new(0.784, 0.314, 0.314, 1.0)
                    }
                } else {
                    TEXT_DIM
                };

                // Draw small survivalist skill icon from spritesheet
                let icon_size = 14.0 * s;
                if let Some(ref texture) = self.ui_icons {
                    let (icon_col, icon_row) = (1.0, 5.0); // Survivalist icon
                    let src_x = icon_col * 24.0;
                    let src_y = icon_row * 24.0;
                    draw_texture_ex(
                        texture,
                        info_x,
                        info_y - icon_size + 2.0 * s,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(src_x, src_y, 24.0, 24.0)),
                            dest_size: Some(Vec2::new(icon_size, icon_size)),
                            ..Default::default()
                        },
                    );
                    info_x += icon_size + 2.0 * s;
                }

                let lvl_text = format!("Lv{}", recipe.level_required);
                self.draw_text_sharp(&lvl_text, info_x, info_y, 16.0, level_color);
                info_x += self.measure_text_sharp(&lvl_text, 16.0).width;
            }

            let mut extra_parts: Vec<String> = Vec::new();
            if recipe.xp > 0 {
                extra_parts.push(format!("{}xp", recipe.xp));
            }
            if recipe.craft_time_ms > 0 {
                let secs = recipe.craft_time_ms as f32 / 1000.0;
                if secs == secs.floor() {
                    extra_parts.push(format!("{}s", secs as u32));
                } else {
                    extra_parts.push(format!("{:.1}s", secs));
                }
            }
            if !extra_parts.is_empty() {
                let separator = if recipe.level_required > 1 {
                    " · "
                } else {
                    ""
                };
                let extra_text = format!("{}{}", separator, extra_parts.join(" · "));
                self.draw_text_sharp(&extra_text, info_x, info_y, 16.0, TEXT_DIM);
            }

            // Level check - show warning if too low
            if recipe.level_required > 1 {
                if let Some(player) = state.get_local_player() {
                    let player_level = player.skills.survivalist.level;
                    if player_level < recipe.level_required {
                        can_craft = false;
                    }
                }
            }

            // Right side: quantity buttons + fletch button (only for selected recipe)
            if is_selected {
                let right_x = content_x + content_w - 170.0 * s;

                // Quantity buttons: [1] [X] [All]
                let qty_btn_w = 32.0 * s;
                let qty_btn_h = 22.0 * s;
                let qty_y = y + 6.0 * s;
                let qty_labels = ["1", "X", "All"];
                let qty_ids = [
                    UiElementId::FletchingQuantity1,
                    UiElementId::FletchingQuantityX,
                    UiElementId::FletchingQuantityAll,
                ];

                for (j, (label, id)) in qty_labels.iter().zip(qty_ids.iter()).enumerate() {
                    let bx = right_x + j as f32 * (qty_btn_w + 4.0 * s);
                    let bounds = Rect::new(bx, qty_y, qty_btn_w, qty_btn_h);
                    layout.add(id.clone(), bounds);

                    let is_qty_active = match j {
                        0 => state.ui_state.fletching_quantity == 1,
                        2 => state.ui_state.fletching_quantity == u32::MAX,
                        _ => {
                            state.ui_state.fletching_quantity != 1
                                && state.ui_state.fletching_quantity != u32::MAX
                        }
                    };
                    let is_qty_hovered = hovered.as_ref() == Some(id);

                    let (bg, border) = if is_qty_active {
                        (Color::new(0.188, 0.188, 0.282, 1.0), SLOT_SELECTED_BORDER)
                    } else if is_qty_hovered {
                        (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
                    } else {
                        (SLOT_BG_EMPTY, SLOT_BORDER)
                    };

                    draw_rectangle(bx, qty_y, qty_btn_w, qty_btn_h, border);
                    draw_rectangle(bx + 1.0, qty_y + 1.0, qty_btn_w - 2.0, qty_btn_h - 2.0, bg);

                    let label_dims = self.measure_text_sharp(label, 16.0);
                    let text_color = if is_qty_active { TEXT_TITLE } else { TEXT_DIM };
                    self.draw_text_sharp(
                        label,
                        bx + (qty_btn_w - label_dims.width) / 2.0,
                        qty_y + qty_btn_h * 0.73,
                        16.0,
                        text_color,
                    );
                }

                // Quantity display
                let qty_display = if state.ui_state.fletching_quantity == u32::MAX {
                    "All".to_string()
                } else {
                    format!("x{}", state.ui_state.fletching_quantity)
                };
                let qty_disp_x = right_x + 3.0 * (qty_btn_w + 4.0 * s) + 4.0 * s;
                self.draw_text_sharp(
                    &qty_display,
                    qty_disp_x,
                    qty_y + qty_btn_h * 0.73,
                    16.0,
                    TEXT_NORMAL,
                );

                // FLETCH button (green)
                let fletch_btn_w = 100.0 * s;
                let fletch_btn_h = 26.0 * s;
                let fletch_btn_x = right_x + 4.0 * s;
                let fletch_btn_y = y + row_height - fletch_btn_h - 8.0 * s;

                if can_craft {
                    let bounds = Rect::new(fletch_btn_x, fletch_btn_y, fletch_btn_w, fletch_btn_h);
                    layout.add(UiElementId::FletchingFletchButton, bounds);
                }

                let is_fletch_hovered =
                    can_craft && matches!(hovered, Some(UiElementId::FletchingFletchButton));
                let (btn_bg, btn_border) = if !can_craft {
                    (Color::new(0.12, 0.08, 0.06, 1.0), SLOT_BORDER)
                } else if is_fletch_hovered {
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

                draw_rectangle(
                    fletch_btn_x,
                    fletch_btn_y,
                    fletch_btn_w,
                    fletch_btn_h,
                    btn_border,
                );
                draw_rectangle(
                    fletch_btn_x + 1.0,
                    fletch_btn_y + 1.0,
                    fletch_btn_w - 2.0,
                    fletch_btn_h - 2.0,
                    btn_bg,
                );

                if can_craft {
                    // Top highlight
                    draw_line(
                        fletch_btn_x + 2.0,
                        fletch_btn_y + 2.0,
                        fletch_btn_x + fletch_btn_w - 2.0,
                        fletch_btn_y + 2.0,
                        1.0,
                        Color::new(0.3, 0.7, 0.3, 1.0),
                    );
                }

                let fletch_text = if can_craft {
                    "[ FLETCH ]"
                } else {
                    "Can't Fletch"
                };
                let fletch_text_w = self.measure_text_sharp(fletch_text, 16.0).width;
                let fletch_text_color = if !can_craft {
                    Color::new(0.5, 0.3, 0.3, 1.0)
                } else if is_fletch_hovered {
                    WHITE
                } else {
                    Color::new(0.3, 0.7, 0.3, 1.0)
                };
                self.draw_text_sharp(
                    fletch_text,
                    fletch_btn_x + (fletch_btn_w - fletch_text_w) / 2.0,
                    fletch_btn_y + fletch_btn_h * 0.69,
                    16.0,
                    fletch_text_color,
                );
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
            let is_dragging = state.ui_state.fletching_scroll_drag.dragging;
            let is_hovered = matches!(hovered, Some(UiElementId::FletchingScrollbar));
            let thumb_color = if is_dragging || is_hovered {
                SLOT_HOVER_BORDER
            } else {
                SLOT_BORDER
            };
            draw_rectangle(scrollbar_x, thumb_y, scrollbar_w, thumb_h, thumb_color);
            layout.add_scrollbar(
                UiElementId::FletchingScrollbar,
                Rect::new(scrollbar_x, scrollbar_y, scrollbar_w, scrollbar_track_h),
            );
        }
    }

    /// Render fletching progress overlay
    fn render_fletching_progress(
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

        // Progress text with pulsing ellipsis
        let base_text = "FLETCHING";
        let dots = match ((time * 2.0) as i32) % 4 {
            0 => format!("{}", base_text),
            1 => format!("{}.", base_text),
            2 => format!("{}..", base_text),
            _ => format!("{}...", base_text),
        };
        let dots_dims = self.measure_text_sharp(&dots, 16.0);
        self.draw_text_sharp(
            &dots,
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

        // Batch counter (if batch > 1)
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

        // Progress bar
        let bar_width = area_w - 60.0 * s;
        let bar_height = 20.0 * s;
        let bar_x = area_x + 30.0 * s;
        let bar_y = area_y + area_h / 2.0 - bar_height / 2.0 + 10.0;

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

        // Bar fill (green-brown gradient for woodworking)
        let fill_width = (bar_width - 4.0) * progress;
        if fill_width > 0.0 {
            let fill_x = bar_x + 2.0;
            let fill_y = bar_y + 2.0;
            let fill_h = bar_height - 4.0;

            draw_rectangle(fill_x, fill_y, fill_width, fill_h, FLETCH_PROGRESS_DARK);
            draw_rectangle(
                fill_x,
                fill_y,
                fill_width,
                fill_h / 2.0,
                FLETCH_PROGRESS_MID,
            );
            draw_line(
                fill_x,
                fill_y,
                fill_x + fill_width,
                fill_y,
                1.0,
                FLETCH_PROGRESS_LIGHT,
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
        layout.add(UiElementId::FletchingCancelButton, cancel_bounds);

        let is_cancel_hovered = matches!(_hovered, Some(UiElementId::FletchingCancelButton));
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
