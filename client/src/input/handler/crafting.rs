use super::*;

impl InputHandler {
    pub(super) fn handle_crafting(
        &mut self,
        state: &mut GameState,
        layout: &UiLayout,
        audio: &mut AudioManager,
        frame: ProcessFrame<'_>,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        let current_time = frame.current_time;
        let my = frame.my;
        let mouse_clicked = frame.mouse_clicked;
        let clicked_element = frame.clicked_element.clone();
        // Handle crafting mode
        if state.ui_state.crafting_open {
            // Touch drag scrolling for shop lists on mobile
            let all_touches: Vec<macroquad::input::Touch> = macroquad::input::touches();
            if let Some(tracking_id) = state.ui_state.shop_touch_scroll_id {
                if let Some(touch) = all_touches.iter().find(|t| t.id == tracking_id) {
                    match touch.phase {
                        macroquad::input::TouchPhase::Moved
                        | macroquad::input::TouchPhase::Stationary => {
                            let (_, vy) =
                                screen_to_virtual_coords(touch.position.x, touch.position.y);
                            let dy = state.ui_state.shop_touch_last_y - vy;
                            if !state.ui_state.shop_touch_dragged {
                                let total_dy = (state.ui_state.shop_touch_start_y - vy).abs();
                                if total_dy > 8.0 {
                                    state.ui_state.shop_touch_dragged = true;
                                }
                            }
                            if state.ui_state.shop_touch_dragged {
                                let item_height = 48.0 + 4.0; // SHOP_ITEM_HEIGHT + SHOP_ITEM_SPACING
                                if state.ui_state.shop_touch_scroll_column == 0 {
                                    let max_scroll = state
                                        .ui_state
                                        .shop_data
                                        .as_ref()
                                        .map(|d| {
                                            ((d.stock.len() as f32) * item_height - 200.0).max(0.0)
                                        })
                                        .unwrap_or(0.0);
                                    state.ui_state.shop_buy_scroll =
                                        (state.ui_state.shop_buy_scroll + dy)
                                            .clamp(0.0, max_scroll);
                                } else {
                                    let inventory_count = state.inventory.aggregate_items().len();
                                    let max_scroll =
                                        ((inventory_count as f32) * item_height - 200.0).max(0.0);
                                    state.ui_state.shop_sell_scroll =
                                        (state.ui_state.shop_sell_scroll + dy)
                                            .clamp(0.0, max_scroll);
                                }
                            }
                            state.ui_state.shop_touch_last_y = vy;
                        }
                        macroquad::input::TouchPhase::Ended
                        | macroquad::input::TouchPhase::Cancelled => {
                            state.ui_state.shop_touch_scroll_id = None;
                        }
                        _ => {}
                    }
                } else {
                    state.ui_state.shop_touch_scroll_id = None;
                }
            } else {
                for touch in &all_touches {
                    if touch.phase == macroquad::input::TouchPhase::Started {
                        let (vx, vy) = screen_to_virtual_coords(touch.position.x, touch.position.y);
                        let hit = layout.hit_test(vx, vy);
                        let buy_area = matches!(
                            hit,
                            Some(UiElementId::ShopBuyScrollArea)
                                | Some(UiElementId::ShopBuyItem(_))
                        );
                        let sell_area = matches!(
                            hit,
                            Some(UiElementId::ShopSellScrollArea)
                                | Some(UiElementId::ShopSellItem(_))
                        );
                        if buy_area || sell_area {
                            state.ui_state.shop_touch_scroll_id = Some(touch.id);
                            state.ui_state.shop_touch_scroll_column = if buy_area { 0 } else { 1 };
                            state.ui_state.shop_touch_last_y = vy;
                            state.ui_state.shop_touch_start_y = vy;
                            state.ui_state.shop_touch_dragged = false;
                            break;
                        }
                    }
                }
            }

            // Suppress click actions if the touch was a scroll drag
            let was_shop_touch_drag =
                state.ui_state.shop_touch_dragged && state.ui_state.shop_touch_scroll_id.is_none();
            if was_shop_touch_drag {
                state.ui_state.shop_touch_dragged = false;
            }

            // Handle mouse clicks on crafting elements (only on mouse down, not release)
            if mouse_clicked && !was_shop_touch_drag {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::ShopCraftingCloseButton => {
                            state.ui_state.crafting_open = false;
                            state.ui_state.crafting_npc_id = None;
                            state.ui_state.shop_data = None;
                            state.ui_state.shop_quantity_hold_element = None;
                            state.pending_sfx.push("enter".to_string());
                            return true;
                        }
                        UiElementId::MainTab(idx) => {
                            state.ui_state.shop_main_tab = *idx;
                            state.pending_sfx.push("enter".to_string());
                            return true;
                        }
                        UiElementId::CraftingCategoryTab(idx) => {
                            // Disable category switching during crafting
                            if !state.ui_state.crafting_in_progress {
                                if *idx != state.ui_state.crafting_selected_category {
                                    state.ui_state.crafting_selected_category = *idx;
                                    state.ui_state.crafting_selected_recipe = 0;
                                    state.ui_state.crafting_scroll_offset = 0.0;
                                    state.pending_sfx.push("enter".to_string());
                                }
                            }
                            return true;
                        }
                        UiElementId::CraftingRecipeItem(idx) => {
                            // Disable recipe selection during crafting
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.crafting_selected_recipe = *idx;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return true;
                        }
                        UiElementId::CraftingButton => {
                            // Don't allow crafting while already in progress
                            if state.ui_state.crafting_in_progress {
                                return true;
                            }
                            // Get unique categories from recipes (matching renderer grouping)
                            let click_filtered = state.shop_filtered_recipes();
                            let categories: Vec<String> = {
                                let mut cats: Vec<String> = click_filtered
                                    .iter()
                                    .map(|r| {
                                        if r.category == "materials" || r.category == "consumables"
                                        {
                                            "supplies".to_string()
                                        } else {
                                            r.category.clone()
                                        }
                                    })
                                    .collect();
                                cats.sort();
                                cats.dedup();
                                cats
                            };
                            let selected_idx = state
                                .ui_state
                                .crafting_selected_category
                                .min(categories.len().saturating_sub(1));
                            let current_category = categories
                                .get(selected_idx)
                                .map(|s| s.as_str())
                                .unwrap_or("supplies");
                            let mut recipes_in_category: Vec<&crate::game::RecipeDefinition> =
                                click_filtered
                                    .iter()
                                    .filter(|r| {
                                        let cat_match = if current_category == "supplies" {
                                            r.category == "consumables" || r.category == "materials"
                                        } else {
                                            r.category == current_category
                                        };
                                        // Only include discovered recipes (matching renderer)
                                        let is_discovered = !r.requires_discovery
                                            || state.discovered_recipes.contains(&r.id);
                                        cat_match && is_discovered
                                    })
                                    .collect();
                            // Sort to match renderer order (section → level → name)
                            recipes_in_category.sort_by(|a, b| {
                                let a_sec = a.section.as_deref().unwrap_or("");
                                let b_sec = b.section.as_deref().unwrap_or("");
                                section_sort_key(a_sec)
                                    .cmp(&section_sort_key(b_sec))
                                    .then(a.level_required.cmp(&b.level_required))
                                    .then(a.display_name.cmp(&b.display_name))
                            });
                            if let Some(recipe) =
                                recipes_in_category.get(state.ui_state.crafting_selected_recipe)
                            {
                                log::info!("Crafting (click): {}", recipe.id);
                                commands.push(InputCommand::Craft {
                                    recipe_id: recipe.id.clone(),
                                });
                            }
                            return true;
                        }
                        UiElementId::CraftingCancelButton => {
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            return true;
                        }
                        UiElementId::ShopBuyItem(idx) => {
                            state.ui_state.shop_selected_buy_index = *idx;
                            state.ui_state.shop_buy_quantity = 1;
                            state.pending_sfx.push("enter".to_string());
                            return true;
                        }
                        UiElementId::ShopSellItem(idx) => {
                            state.ui_state.shop_selected_sell_index = *idx;
                            state.ui_state.shop_sell_quantity = 1;
                            state.pending_sfx.push("enter".to_string());
                            return true;
                        }
                        UiElementId::ShopBuyQuantityMinus => {
                            if state.ui_state.shop_buy_quantity > 1 {
                                state.ui_state.shop_buy_quantity -= 1;
                            }
                            state.ui_state.shop_quantity_hold_element =
                                Some(UiElementId::ShopBuyQuantityMinus);
                            state.ui_state.shop_quantity_hold_start = current_time;
                            state.ui_state.shop_quantity_hold_last_repeat = current_time;
                            return true;
                        }
                        UiElementId::ShopBuyQuantityPlus => {
                            state.ui_state.shop_buy_quantity += 1;
                            state.ui_state.shop_quantity_hold_element =
                                Some(UiElementId::ShopBuyQuantityPlus);
                            state.ui_state.shop_quantity_hold_start = current_time;
                            state.ui_state.shop_quantity_hold_last_repeat = current_time;
                            return true;
                        }
                        UiElementId::ShopSellQuantityMinus => {
                            if state.ui_state.shop_sell_quantity > 1 {
                                state.ui_state.shop_sell_quantity -= 1;
                            }
                            state.ui_state.shop_quantity_hold_element =
                                Some(UiElementId::ShopSellQuantityMinus);
                            state.ui_state.shop_quantity_hold_start = current_time;
                            state.ui_state.shop_quantity_hold_last_repeat = current_time;
                            return true;
                        }
                        UiElementId::ShopSellQuantityPlus => {
                            state.ui_state.shop_sell_quantity += 1;
                            state.ui_state.shop_quantity_hold_element =
                                Some(UiElementId::ShopSellQuantityPlus);
                            state.ui_state.shop_quantity_hold_start = current_time;
                            state.ui_state.shop_quantity_hold_last_repeat = current_time;
                            return true;
                        }
                        UiElementId::ShopSellQuantityMax => {
                            let inventory_items = state.inventory.aggregate_items();
                            if let Some(agg_item) =
                                inventory_items.get(state.ui_state.shop_selected_sell_index)
                            {
                                state.ui_state.shop_sell_quantity = agg_item.total_quantity.max(1);
                            }
                            return true;
                        }
                        UiElementId::ShopBuyConfirmButton => {
                            if let Some(ref shop_data) = state.ui_state.shop_data {
                                if let Some(ref npc_id) = state.ui_state.shop_npc_id {
                                    if let Some(stock_item) =
                                        shop_data.stock.get(state.ui_state.shop_selected_buy_index)
                                    {
                                        audio.play_sfx("buy");
                                        commands.push(InputCommand::ShopBuy {
                                            npc_id: npc_id.clone(),
                                            item_id: stock_item.item_id.clone(),
                                            quantity: state.ui_state.shop_buy_quantity as u32,
                                        });
                                    }
                                }
                            }
                            return true;
                        }
                        UiElementId::ShopSellConfirmButton => {
                            if let Some(ref npc_id) = state.ui_state.shop_npc_id {
                                let inventory_items = state.inventory.aggregate_items();
                                if let Some(agg_item) =
                                    inventory_items.get(state.ui_state.shop_selected_sell_index)
                                {
                                    commands.push(InputCommand::ShopSell {
                                        npc_id: npc_id.clone(),
                                        item_id: agg_item.item_id.clone(),
                                        quantity: state.ui_state.shop_sell_quantity as u32,
                                    });
                                }
                            }
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            // Hold-to-repeat for quantity +/- buttons
            if is_mouse_button_down(MouseButton::Left) {
                if let Some(ref hold_elem) = state.ui_state.shop_quantity_hold_element {
                    // Check if still hovering the same button
                    let still_hovering = state.ui_state.hovered_element.as_ref() == Some(hold_elem);
                    if still_hovering {
                        const INITIAL_DELAY: f64 = 0.4;
                        const REPEAT_INTERVAL: f64 = 0.06;
                        let held_duration = current_time - state.ui_state.shop_quantity_hold_start;
                        if held_duration >= INITIAL_DELAY {
                            let since_last =
                                current_time - state.ui_state.shop_quantity_hold_last_repeat;
                            if since_last >= REPEAT_INTERVAL {
                                state.ui_state.shop_quantity_hold_last_repeat = current_time;
                                match hold_elem {
                                    UiElementId::ShopBuyQuantityMinus => {
                                        if state.ui_state.shop_buy_quantity > 1 {
                                            state.ui_state.shop_buy_quantity -= 1;
                                        }
                                    }
                                    UiElementId::ShopBuyQuantityPlus => {
                                        state.ui_state.shop_buy_quantity += 1;
                                    }
                                    UiElementId::ShopSellQuantityMinus => {
                                        if state.ui_state.shop_sell_quantity > 1 {
                                            state.ui_state.shop_sell_quantity -= 1;
                                        }
                                    }
                                    UiElementId::ShopSellQuantityPlus => {
                                        state.ui_state.shop_sell_quantity += 1;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    } else {
                        state.ui_state.shop_quantity_hold_element = None;
                    }
                }
            } else {
                state.ui_state.shop_quantity_hold_element = None;
            }

            // Allow chat input while shop panel is open
            if state.ui_state.chat_open && !state.ui_state.classic_controls {
                process_chat_keyboard_input(state, commands, audio);
                return true;
            }

            // Escape: if crafting in progress, cancel craft; otherwise close menu
            if is_key_pressed(KeyCode::Escape) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                    return true;
                }
                state.ui_state.crafting_open = false;
                state.ui_state.crafting_npc_id = None;
                state.ui_state.shop_data = None;
                state.ui_state.shop_quantity_hold_element = None;
                return true;
            }

            // Q switches between Recipes/Shop main tabs
            if is_key_pressed(KeyCode::Q) {
                state.ui_state.shop_main_tab = if state.ui_state.shop_main_tab == 0 {
                    1
                } else {
                    0
                };
            }

            if state.ui_state.shop_main_tab == 0 {
                // Recipes tab keyboard controls
                // Get recipes filtered by this shop's categories
                let filtered_recipes = state.shop_filtered_recipes();
                // Get unique categories from recipes, merging consumables and materials
                let categories: Vec<String> = {
                    let mut cats: Vec<String> = filtered_recipes
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
                };

                // Disable navigation during crafting
                if !state.ui_state.crafting_in_progress {
                    // Left/Right navigate categories
                    if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
                        if state.ui_state.crafting_selected_category > 0 {
                            state.ui_state.crafting_selected_category -= 1;
                            state.ui_state.crafting_selected_recipe = 0;
                            state.ui_state.crafting_scroll_offset = 0.0;
                        }
                    }
                    if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
                        if state.ui_state.crafting_selected_category
                            < categories.len().saturating_sub(1)
                        {
                            state.ui_state.crafting_selected_category += 1;
                            state.ui_state.crafting_selected_recipe = 0;
                            state.ui_state.crafting_scroll_offset = 0.0;
                        }
                    }

                    // Get discovered recipes for current category (matches renderer filtering)
                    let selected_idx = state
                        .ui_state
                        .crafting_selected_category
                        .min(categories.len().saturating_sub(1));
                    let current_category = categories
                        .get(selected_idx)
                        .map(|s| s.as_str())
                        .unwrap_or("supplies");
                    let mut recipes_in_category: Vec<&crate::game::RecipeDefinition> =
                        filtered_recipes
                            .iter()
                            .filter(|r| {
                                let cat_match = if current_category == "supplies" {
                                    r.category == "consumables" || r.category == "materials"
                                } else {
                                    r.category == current_category
                                };
                                // Only include discovered recipes (matching renderer)
                                let is_discovered = !r.requires_discovery
                                    || state.discovered_recipes.contains(&r.id);
                                cat_match && is_discovered
                            })
                            .collect();
                    // Sort to match renderer order (section → level → name)
                    recipes_in_category.sort_by(|a, b| {
                        let a_sec = a.section.as_deref().unwrap_or("");
                        let b_sec = b.section.as_deref().unwrap_or("");
                        section_sort_key(a_sec)
                            .cmp(&section_sort_key(b_sec))
                            .then(a.level_required.cmp(&b.level_required))
                            .then(a.display_name.cmp(&b.display_name))
                    });

                    // Up/Down navigate recipes
                    let mut key_navigated = false;
                    if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                        if state.ui_state.crafting_selected_recipe > 0 {
                            state.ui_state.crafting_selected_recipe -= 1;
                            key_navigated = true;
                        }
                    }
                    if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                        if state.ui_state.crafting_selected_recipe
                            < recipes_in_category.len().saturating_sub(1)
                        {
                            state.ui_state.crafting_selected_recipe += 1;
                            key_navigated = true;
                        }
                    }

                    // Only auto-scroll when keyboard navigated, not every frame
                    if key_navigated {
                        let s = state.ui_state.ui_scale;
                        let craft_line_h = 28.0 * s;
                        let section_h = SECTION_HEADER_HEIGHT * s;
                        // Count the actual row position including undiscovered "????" entries
                        // and section headers (must match renderer layout)
                        let mut all_in_category: Vec<&crate::game::RecipeDefinition> =
                            filtered_recipes
                                .iter()
                                .filter(|r| {
                                    if current_category == "supplies" {
                                        r.category == "consumables" || r.category == "materials"
                                    } else {
                                        r.category == current_category
                                    }
                                })
                                .collect();
                        // Sort by section to match renderer order
                        all_in_category.sort_by(|a, b| {
                            let sa = a.section.as_deref().unwrap_or("");
                            let sb = b.section.as_deref().unwrap_or("");
                            section_sort_key(sa)
                                .cmp(&section_sort_key(sb))
                                .then_with(|| a.level_required.cmp(&b.level_required))
                                .then_with(|| a.display_name.cmp(&b.display_name))
                        });
                        // Walk through items tracking pixel position with section headers
                        let mut pixel_y = 0.0_f32;
                        let mut current_section: Option<&str> = None;
                        let mut discovered_idx = 0usize;
                        let mut item_top = 0.0_f32;
                        for r in &all_in_category {
                            let recipe_section = r.section.as_deref().unwrap_or("");
                            if !recipe_section.is_empty() && current_section != Some(recipe_section)
                            {
                                current_section = Some(recipe_section);
                                pixel_y += section_h;
                            }
                            let is_disc =
                                !r.requires_discovery || state.discovered_recipes.contains(&r.id);
                            if is_disc {
                                if discovered_idx == state.ui_state.crafting_selected_recipe {
                                    item_top = pixel_y;
                                    break;
                                }
                                discovered_idx += 1;
                            }
                            pixel_y += craft_line_h;
                        }
                        let item_bottom = item_top + craft_line_h;
                        if item_top < state.ui_state.crafting_scroll_offset {
                            state.ui_state.crafting_scroll_offset = item_top;
                        }
                        // Calculate visible height matching renderer layout (scaled)
                        let (_, sh) = crate::util::virtual_screen_size();
                        let panel_h = (450.0 * s).min(sh - 16.0);
                        let content_height = panel_h - 8.0 - 40.0 * s - 30.0 * s - 12.0 * s;
                        let has_tabs = categories.len() > 1;
                        let list_height = if has_tabs {
                            content_height - 28.0 * s - 20.0 * s
                        } else {
                            content_height - 10.0 * s
                        };
                        let visible_h = list_height - 8.0 * s;
                        if item_bottom > state.ui_state.crafting_scroll_offset + visible_h {
                            state.ui_state.crafting_scroll_offset = item_bottom - visible_h;
                        }
                    }

                    // Enter or C crafts selected recipe
                    if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::C) {
                        if let Some(recipe) =
                            recipes_in_category.get(state.ui_state.crafting_selected_recipe)
                        {
                            log::info!("Crafting: {}", recipe.id);
                            commands.push(InputCommand::Craft {
                                recipe_id: recipe.id.clone(),
                            });
                        }
                    }
                } else {
                    // While crafting is in progress, X key cancels
                    if is_key_pressed(KeyCode::X) {
                        commands.push(InputCommand::CancelCraft);
                        return true;
                    }
                }

                // Mouse wheel scrolling for crafting recipe list (same logic as shop tab)
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y != 0.0 {
                    let s = state.ui_state.ui_scale;
                    const SCROLL_SPEED: f32 = 30.0;
                    let line_height = 28.0 * s;
                    // Count all recipes in category (discovered + undiscovered) to match renderer
                    let sel_idx = state
                        .ui_state
                        .crafting_selected_category
                        .min(categories.len().saturating_sub(1));
                    let cur_cat = categories
                        .get(sel_idx)
                        .map(|sc| sc.as_str())
                        .unwrap_or("supplies");
                    let recipes_in_cat: Vec<&crate::game::RecipeDefinition> = filtered_recipes
                        .iter()
                        .filter(|r| {
                            if cur_cat == "supplies" {
                                r.category == "consumables" || r.category == "materials"
                            } else {
                                r.category == cur_cat
                            }
                        })
                        .collect();
                    let total_visible = recipes_in_cat.len();
                    // Count distinct sections for header height
                    let num_scroll_sections = {
                        let mut seen = std::collections::HashSet::new();
                        for r in &recipes_in_cat {
                            if let Some(ref sec) = r.section {
                                if !sec.is_empty() {
                                    seen.insert(sec.as_str());
                                }
                            }
                        }
                        seen.len()
                    };
                    // Match renderer layout constants (scaled by ui_scale)
                    let (_, sh) = crate::util::virtual_screen_size();
                    let panel_height = (450.0 * s).min(sh - 16.0);
                    let content_height = panel_height - 8.0 - 40.0 * s - 30.0 * s - 12.0 * s;
                    let has_tabs = categories.len() > 1;
                    let list_height = if has_tabs {
                        content_height - 28.0 * s - 20.0 * s
                    } else {
                        content_height - 10.0 * s
                    };
                    let list_content_height = list_height - 8.0 * s;
                    let total_content = total_visible as f32 * line_height
                        + num_scroll_sections as f32 * SECTION_HEADER_HEIGHT * s;
                    let max_scroll = (total_content - list_content_height).max(0.0);
                    state.ui_state.crafting_scroll_offset = (state.ui_state.crafting_scroll_offset
                        - wheel_y * SCROLL_SPEED)
                        .clamp(0.0, max_scroll);
                }

                // Crafting scrollbar drag handling
                if let Some(track_bounds) = layout.get_bounds(&UiElementId::CraftingScrollbar) {
                    let s = state.ui_state.ui_scale;
                    let line_height = if cfg!(target_os = "android") {
                        36.0 * s
                    } else {
                        28.0 * s
                    };
                    let sel_idx = state
                        .ui_state
                        .crafting_selected_category
                        .min(categories.len().saturating_sub(1));
                    let cur_cat = categories
                        .get(sel_idx)
                        .map(|sc| sc.as_str())
                        .unwrap_or("supplies");
                    let recipes_in_cat: Vec<&crate::game::RecipeDefinition> = filtered_recipes
                        .iter()
                        .filter(|r| {
                            if cur_cat == "supplies" {
                                r.category == "consumables" || r.category == "materials"
                            } else {
                                r.category == cur_cat
                            }
                        })
                        .collect();
                    let drag_total_visible = recipes_in_cat.len();
                    let drag_num_sections = {
                        let mut seen = std::collections::HashSet::new();
                        for r in &recipes_in_cat {
                            if let Some(ref sec) = r.section {
                                if !sec.is_empty() {
                                    seen.insert(sec.as_str());
                                }
                            }
                        }
                        seen.len()
                    };
                    let total_content = drag_total_visible as f32 * line_height
                        + drag_num_sections as f32 * SECTION_HEADER_HEIGHT * s;
                    let max_scroll = (total_content - track_bounds.h).max(0.0);
                    let clicked_on =
                        matches!(clicked_element, Some(UiElementId::CraftingScrollbar));
                    crate::ui::scroll::handle_scrollbar_drag(
                        &mut state.ui_state.crafting_scroll_drag,
                        &mut state.ui_state.crafting_scroll_offset,
                        max_scroll,
                        track_bounds,
                        total_content,
                        my,
                        is_mouse_button_down(MouseButton::Left),
                        mouse_clicked,
                        clicked_on,
                    );
                } else if !is_mouse_button_down(MouseButton::Left) {
                    state.ui_state.crafting_scroll_drag.dragging = false;
                }
            } else if state.ui_state.shop_main_tab == 1 {
                // Shop tab - side-by-side Buy/Sell layout
                // Mouse wheel scrolling based on which scroll area the mouse is hovering over
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y != 0.0 {
                    const SCROLL_SPEED: f32 = 30.0;
                    let item_height = 48.0 + 4.0; // height + spacing

                    // Check which area is being hovered
                    match &state.ui_state.hovered_element {
                        Some(UiElementId::ShopBuyScrollArea)
                        | Some(UiElementId::ShopBuyItem(_)) => {
                            if let Some(ref shop_data) = state.ui_state.shop_data {
                                let max_scroll =
                                    ((shop_data.stock.len() as f32) * item_height - 200.0).max(0.0);
                                state.ui_state.shop_buy_scroll = (state.ui_state.shop_buy_scroll
                                    - wheel_y * SCROLL_SPEED)
                                    .clamp(0.0, max_scroll);
                            }
                        }
                        Some(UiElementId::ShopSellScrollArea)
                        | Some(UiElementId::ShopSellItem(_)) => {
                            let inventory_count = state.inventory.aggregate_items().len();
                            let max_scroll =
                                ((inventory_count as f32) * item_height - 200.0).max(0.0);
                            state.ui_state.shop_sell_scroll = (state.ui_state.shop_sell_scroll
                                - wheel_y * SCROLL_SPEED)
                                .clamp(0.0, max_scroll);
                        }
                        _ => {}
                    }
                }

                // Scrollbar drag handling for shop buy/sell
                {
                    let item_height = 48.0 + 4.0;
                    // Buy scrollbar
                    if let Some(track_bounds) = layout.get_bounds(&UiElementId::ShopBuyScrollbar) {
                        let max_scroll = state
                            .ui_state
                            .shop_data
                            .as_ref()
                            .map(|d| ((d.stock.len() as f32) * item_height - 200.0).max(0.0))
                            .unwrap_or(0.0);
                        let clicked_on =
                            matches!(clicked_element, Some(UiElementId::ShopBuyScrollbar));
                        crate::ui::scroll::handle_scrollbar_drag(
                            &mut state.ui_state.shop_buy_scroll_drag,
                            &mut state.ui_state.shop_buy_scroll,
                            max_scroll,
                            track_bounds,
                            max_scroll + 200.0,
                            my,
                            is_mouse_button_down(MouseButton::Left),
                            mouse_clicked,
                            clicked_on,
                        );
                    } else if !is_mouse_button_down(MouseButton::Left) {
                        state.ui_state.shop_buy_scroll_drag.dragging = false;
                    }
                    // Sell scrollbar
                    if let Some(track_bounds) = layout.get_bounds(&UiElementId::ShopSellScrollbar) {
                        let inventory_count = state.inventory.aggregate_items().len();
                        let max_scroll = ((inventory_count as f32) * item_height - 200.0).max(0.0);
                        let clicked_on =
                            matches!(clicked_element, Some(UiElementId::ShopSellScrollbar));
                        crate::ui::scroll::handle_scrollbar_drag(
                            &mut state.ui_state.shop_sell_scroll_drag,
                            &mut state.ui_state.shop_sell_scroll,
                            max_scroll,
                            track_bounds,
                            max_scroll + 200.0,
                            my,
                            is_mouse_button_down(MouseButton::Left),
                            mouse_clicked,
                            clicked_on,
                        );
                    } else if !is_mouse_button_down(MouseButton::Left) {
                        state.ui_state.shop_sell_scroll_drag.dragging = false;
                    }
                }

                // Keyboard controls for shop
                use crate::game::ShopSubTab;

                // Left/Right or A/D to switch between Buy and Sell panels
                if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
                    state.ui_state.shop_sub_tab = ShopSubTab::Buy;
                }
                if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
                    state.ui_state.shop_sub_tab = ShopSubTab::Sell;
                }
                // Tab to toggle between panels
                if is_key_pressed(KeyCode::Tab) {
                    state.ui_state.shop_sub_tab = match state.ui_state.shop_sub_tab {
                        ShopSubTab::Buy => ShopSubTab::Sell,
                        ShopSubTab::Sell => ShopSubTab::Buy,
                    };
                }

                // Up/Down or W/S to navigate items in the active panel
                match state.ui_state.shop_sub_tab {
                    ShopSubTab::Buy => {
                        let item_count = state
                            .ui_state
                            .shop_data
                            .as_ref()
                            .map(|d| d.stock.len())
                            .unwrap_or(0);

                        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                            if state.ui_state.shop_selected_buy_index > 0 {
                                state.ui_state.shop_selected_buy_index -= 1;
                                state.ui_state.shop_buy_quantity = 1;
                            }
                        }
                        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                            if state.ui_state.shop_selected_buy_index < item_count.saturating_sub(1)
                            {
                                state.ui_state.shop_selected_buy_index += 1;
                                state.ui_state.shop_buy_quantity = 1;
                            }
                        }

                        // +/- to adjust quantity
                        if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
                            state.ui_state.shop_buy_quantity += 1;
                        }
                        if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
                            if state.ui_state.shop_buy_quantity > 1 {
                                state.ui_state.shop_buy_quantity -= 1;
                            }
                        }

                        // Enter to confirm buy
                        if is_key_pressed(KeyCode::Enter) {
                            if let Some(ref shop_data) = state.ui_state.shop_data {
                                if let Some(ref npc_id) = state.ui_state.shop_npc_id {
                                    if let Some(stock_item) =
                                        shop_data.stock.get(state.ui_state.shop_selected_buy_index)
                                    {
                                        audio.play_sfx("buy");
                                        commands.push(InputCommand::ShopBuy {
                                            npc_id: npc_id.clone(),
                                            item_id: stock_item.item_id.clone(),
                                            quantity: state.ui_state.shop_buy_quantity as u32,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    ShopSubTab::Sell => {
                        let inventory_items = state.inventory.aggregate_items();
                        let item_count = inventory_items.len();

                        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                            if state.ui_state.shop_selected_sell_index > 0 {
                                state.ui_state.shop_selected_sell_index -= 1;
                                state.ui_state.shop_sell_quantity = 1;
                            }
                        }
                        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                            if state.ui_state.shop_selected_sell_index
                                < item_count.saturating_sub(1)
                            {
                                state.ui_state.shop_selected_sell_index += 1;
                                state.ui_state.shop_sell_quantity = 1;
                            }
                        }

                        // +/- to adjust quantity
                        if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
                            state.ui_state.shop_sell_quantity += 1;
                        }
                        if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
                            if state.ui_state.shop_sell_quantity > 1 {
                                state.ui_state.shop_sell_quantity -= 1;
                            }
                        }

                        // Enter to confirm sell
                        if is_key_pressed(KeyCode::Enter) {
                            if let Some(ref npc_id) = state.ui_state.shop_npc_id {
                                if let Some(agg_item) =
                                    inventory_items.get(state.ui_state.shop_selected_sell_index)
                                {
                                    commands.push(InputCommand::ShopSell {
                                        npc_id: npc_id.clone(),
                                        item_id: agg_item.item_id.clone(),
                                        quantity: state.ui_state.shop_sell_quantity as u32,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // Don't process other input while crafting is open
            return true;
        }

        false
    }
}

impl InputHandler {
    pub(super) fn handle_alchemy(
        &mut self,
        state: &mut GameState,
        layout: &UiLayout,
        audio: &mut AudioManager,
        frame: ProcessFrame<'_>,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        let my = frame.my;
        let mouse_clicked = frame.mouse_clicked;
        let clicked_element = frame.clicked_element.clone();
        // Handle alchemy station mode
        if state.ui_state.alchemy_station_open {
            // Handle mouse clicks on alchemy station elements
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::AlchemyCloseButton => {
                            state.ui_state.alchemy_station_open = false;
                            state.ui_state.alchemy_station_tile = None;
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            state.pending_sfx.push("enter".to_string());
                            return true;
                        }
                        UiElementId::AlchemyRecipeItem(idx) => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.alchemy_station_selected_recipe = *idx;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return true;
                        }
                        UiElementId::AlchemyBrewButton => {
                            if !state.ui_state.crafting_in_progress {
                                let tab_sections =
                                    sections_for_tab(state.ui_state.alchemy_station_tab);
                                let mut alchemy_recipes: Vec<_> = state
                                    .recipe_definitions
                                    .iter()
                                    .filter(|r| r.station.as_deref() == Some("alchemy_station"))
                                    .filter(|r| {
                                        !r.requires_discovery
                                            || state.discovered_recipes.contains(&r.id)
                                    })
                                    .filter(|r| {
                                        tab_sections.contains(&r.section.as_deref().unwrap_or(""))
                                    })
                                    .collect();
                                alchemy_recipes.sort_by(|a, b| {
                                    let sa = a.section.as_deref().unwrap_or("");
                                    let sb = b.section.as_deref().unwrap_or("");
                                    section_sort_key(sa)
                                        .cmp(&section_sort_key(sb))
                                        .then(a.level_required.cmp(&b.level_required))
                                });
                                if let Some(recipe) = alchemy_recipes
                                    .get(state.ui_state.alchemy_station_selected_recipe)
                                {
                                    commands.push(InputCommand::AlchemyCraft {
                                        recipe_id: recipe.id.clone(),
                                        quantity: state.ui_state.alchemy_station_quantity,
                                    });
                                }
                            }
                            return true;
                        }
                        UiElementId::AlchemyCancelButton => {
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            return true;
                        }
                        UiElementId::AlchemyQuantityMinus => {
                            if state.ui_state.alchemy_station_quantity > 1 {
                                state.ui_state.alchemy_station_quantity -= 1;
                            }
                            return true;
                        }
                        UiElementId::AlchemyQuantityPlus => {
                            state.ui_state.alchemy_station_quantity =
                                (state.ui_state.alchemy_station_quantity + 1).min(99);
                            return true;
                        }
                        UiElementId::AlchemyQuantityMax => {
                            let tab_sections = sections_for_tab(state.ui_state.alchemy_station_tab);
                            let mut alchemy_recipes: Vec<_> = state
                                .recipe_definitions
                                .iter()
                                .filter(|r| r.station.as_deref() == Some("alchemy_station"))
                                .filter(|r| {
                                    !r.requires_discovery
                                        || state.discovered_recipes.contains(&r.id)
                                })
                                .filter(|r| {
                                    tab_sections.contains(&r.section.as_deref().unwrap_or(""))
                                })
                                .collect();
                            alchemy_recipes.sort_by(|a, b| {
                                let sa = a.section.as_deref().unwrap_or("");
                                let sb = b.section.as_deref().unwrap_or("");
                                section_sort_key(sa)
                                    .cmp(&section_sort_key(sb))
                                    .then(a.level_required.cmp(&b.level_required))
                            });
                            if let Some(recipe) =
                                alchemy_recipes.get(state.ui_state.alchemy_station_selected_recipe)
                            {
                                let mut max_possible = 99i32;
                                for ing in &recipe.ingredients {
                                    let have = state.inventory.count_item_by_id(&ing.item_id);
                                    if ing.count > 0 {
                                        max_possible = max_possible.min(have / ing.count);
                                    }
                                }
                                state.ui_state.alchemy_station_quantity =
                                    (max_possible.max(1) as u32).min(99);
                            }
                            return true;
                        }
                        UiElementId::AlchemyTab(idx) => {
                            // Allow switching to tabs that have content (0=Potions, 1=Scrolls)
                            if (*idx == 0 || *idx == 1) && !state.ui_state.crafting_in_progress {
                                state.ui_state.alchemy_station_tab = *idx as u8;
                                state.ui_state.alchemy_station_selected_recipe = 0;
                                state.ui_state.alchemy_station_scroll_offset = 0.0;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            // Allow chat input while alchemy station panel is open
            if state.ui_state.chat_open && !state.ui_state.classic_controls {
                process_chat_keyboard_input(state, commands, audio);
                return true;
            }

            // Escape: cancel if crafting, otherwise close
            if is_key_pressed(KeyCode::Escape) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                    return true;
                }
                state.ui_state.alchemy_station_open = false;
                state.ui_state.alchemy_station_tile = None;
                return true;
            }

            // E key closes alchemy station
            if is_key_pressed(KeyCode::E) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                }
                state.ui_state.alchemy_station_open = false;
                state.ui_state.alchemy_station_tile = None;
                return true;
            }

            if !state.ui_state.crafting_in_progress {
                let tab_sections = sections_for_tab(state.ui_state.alchemy_station_tab);
                let mut alchemy_recipes: Vec<_> = state
                    .recipe_definitions
                    .iter()
                    .filter(|r| r.station.as_deref() == Some("alchemy_station"))
                    .filter(|r| !r.requires_discovery || state.discovered_recipes.contains(&r.id))
                    .filter(|r| tab_sections.contains(&r.section.as_deref().unwrap_or("")))
                    .collect();
                alchemy_recipes.sort_by(|a, b| {
                    let sa = a.section.as_deref().unwrap_or("");
                    let sb = b.section.as_deref().unwrap_or("");
                    section_sort_key(sa)
                        .cmp(&section_sort_key(sb))
                        .then(a.level_required.cmp(&b.level_required))
                });
                let recipe_count = alchemy_recipes.len();

                // Must match renderer layout in alchemy_station.rs
                let s = state.ui_state.ui_scale;
                let row_h = 56.0 * s;
                let section_header_h = 22.0 * s;
                let section_count = {
                    let mut sections = std::collections::HashSet::new();
                    for r in &alchemy_recipes {
                        sections.insert(r.section.as_deref().unwrap_or(""));
                    }
                    sections.len()
                };
                let total_content =
                    recipe_count as f32 * row_h + section_count as f32 * section_header_h;

                let (_, sh) = crate::util::virtual_screen_size();
                let panel_h = (520.0 * s).min(sh - 16.0);
                let header_h = 40.0 * s;
                let footer_h = 30.0 * s;
                let tab_h = 28.0 * s;
                let skill_bar_h = 24.0 * s;
                let frame = 4.0; // FRAME_THICKNESS
                                 // content_y = panel_y + frame + header_h + 2 + tab_h + 2 + skill_bar_h + 4*s
                                 // footer_y = panel_y + panel_h - frame - footer_h
                let total_content_h = panel_h
                    - frame * 2.0
                    - header_h
                    - 2.0
                    - tab_h
                    - 2.0
                    - skill_bar_h
                    - 4.0 * s
                    - 4.0 * s
                    - footer_h;

                // Dynamic detail panel height based on selected recipe's ingredient count
                let ingredient_count = alchemy_recipes
                    .get(state.ui_state.alchemy_station_selected_recipe)
                    .map(|r| r.ingredients.len())
                    .unwrap_or(1);
                let detail_h = (8.0 * s
                    + 40.0 * s
                    + 8.0 * s
                    + 6.0 * s
                    + ingredient_count as f32 * 28.0 * s
                    + 10.0 * s
                    + 26.0 * s
                    + 6.0 * s)
                    .min(total_content_h * 0.65);
                let recipe_list_h = total_content_h - detail_h - 4.0 * s;
                let max_scroll = (total_content - recipe_list_h).max(0.0);

                // W/S or Up/Down to navigate recipes
                if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                    if state.ui_state.alchemy_station_selected_recipe > 0 {
                        state.ui_state.alchemy_station_selected_recipe -= 1;
                        let item_top =
                            state.ui_state.alchemy_station_selected_recipe as f32 * row_h;
                        if item_top < state.ui_state.alchemy_station_scroll_offset {
                            state.ui_state.alchemy_station_scroll_offset = item_top;
                        }
                    }
                }
                if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                    if state.ui_state.alchemy_station_selected_recipe
                        < recipe_count.saturating_sub(1)
                    {
                        state.ui_state.alchemy_station_selected_recipe += 1;
                        let item_bottom =
                            (state.ui_state.alchemy_station_selected_recipe + 1) as f32 * row_h;
                        if item_bottom
                            > state.ui_state.alchemy_station_scroll_offset + recipe_list_h
                        {
                            state.ui_state.alchemy_station_scroll_offset =
                                item_bottom - recipe_list_h;
                        }
                    }
                }

                // +/- to adjust quantity
                if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
                    state.ui_state.alchemy_station_quantity =
                        (state.ui_state.alchemy_station_quantity + 1).min(99);
                }
                if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
                    if state.ui_state.alchemy_station_quantity > 1 {
                        state.ui_state.alchemy_station_quantity -= 1;
                    }
                }

                // Enter or C to brew
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::C) {
                    if let Some(recipe) =
                        alchemy_recipes.get(state.ui_state.alchemy_station_selected_recipe)
                    {
                        commands.push(InputCommand::AlchemyCraft {
                            recipe_id: recipe.id.clone(),
                            quantity: state.ui_state.alchemy_station_quantity,
                        });
                    }
                }

                // Mouse wheel scrolling
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y != 0.0 {
                    const SCROLL_SPEED: f32 = 30.0;
                    state.ui_state.alchemy_station_scroll_offset =
                        (state.ui_state.alchemy_station_scroll_offset - wheel_y * SCROLL_SPEED)
                            .clamp(0.0, max_scroll);
                }

                // Scrollbar drag handling
                if let Some(track_bounds) = layout.get_bounds(&UiElementId::AlchemyScrollbar) {
                    let clicked_on = matches!(clicked_element, Some(UiElementId::AlchemyScrollbar));
                    crate::ui::scroll::handle_scrollbar_drag(
                        &mut state.ui_state.alchemy_station_scroll_drag,
                        &mut state.ui_state.alchemy_station_scroll_offset,
                        max_scroll,
                        track_bounds,
                        total_content,
                        my,
                        is_mouse_button_down(MouseButton::Left),
                        mouse_clicked,
                        clicked_on,
                    );
                } else if !is_mouse_button_down(MouseButton::Left) {
                    state.ui_state.alchemy_station_scroll_drag.dragging = false;
                }
            }

            // Don't process other input while alchemy station is open
            return true;
        }

        false
    }
}
