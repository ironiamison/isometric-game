use super::*;

impl Renderer {
    pub(super) fn render_recipes_tab(
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
}
