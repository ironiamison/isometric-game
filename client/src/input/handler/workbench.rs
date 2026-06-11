use super::*;

impl InputHandler {
    pub(super) fn handle_workbench(
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
        // ===== WORKBENCH PANEL =====
        if state.ui_state.workbench_open {
            // Handle mouse clicks on workbench elements
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::WorkbenchCloseButton => {
                            state.ui_state.workbench_open = false;
                            state.ui_state.workbench_tile = None;
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            state.pending_sfx.push("enter".to_string());
                            return true;
                        }
                        UiElementId::WorkbenchRecipeItem(idx) => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.workbench_selected_recipe = *idx;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return true;
                        }
                        UiElementId::WorkbenchCraftButton => {
                            if !state.ui_state.crafting_in_progress {
                                let tab_sections = crate::render::workbench_sections_for_tab(
                                    state.ui_state.workbench_tab,
                                );
                                let mut workbench_recipes: Vec<_> = state
                                    .recipe_definitions
                                    .iter()
                                    .filter(|r| r.station.as_deref() == Some("workbench"))
                                    .filter(|r| {
                                        !r.requires_discovery
                                            || state.discovered_recipes.contains(&r.id)
                                    })
                                    .filter(|r| {
                                        tab_sections.contains(&r.section.as_deref().unwrap_or(""))
                                    })
                                    .collect();
                                workbench_recipes.sort_by(|a, b| {
                                    let sa = a.section.as_deref().unwrap_or("");
                                    let sb = b.section.as_deref().unwrap_or("");
                                    section_sort_key(sa)
                                        .cmp(&section_sort_key(sb))
                                        .then(a.level_required.cmp(&b.level_required))
                                });
                                if let Some(recipe) =
                                    workbench_recipes.get(state.ui_state.workbench_selected_recipe)
                                {
                                    commands.push(InputCommand::WorkbenchCraft {
                                        recipe_id: recipe.id.clone(),
                                        quantity: state.ui_state.workbench_quantity,
                                    });
                                }
                            }
                            return true;
                        }
                        UiElementId::WorkbenchCancelButton => {
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            return true;
                        }
                        UiElementId::WorkbenchQuantityMinus => {
                            if state.ui_state.workbench_quantity > 1 {
                                state.ui_state.workbench_quantity -= 1;
                            }
                            return true;
                        }
                        UiElementId::WorkbenchQuantityPlus => {
                            state.ui_state.workbench_quantity =
                                (state.ui_state.workbench_quantity + 1).min(99);
                            return true;
                        }
                        UiElementId::WorkbenchTab(idx) => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.workbench_tab = *idx as u8;
                                state.ui_state.workbench_selected_recipe = 0;
                                state.ui_state.workbench_scroll_offset = 0.0;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            // Allow chat input while workbench panel is open
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
                state.ui_state.workbench_open = false;
                state.ui_state.workbench_tile = None;
                return true;
            }

            // E key closes workbench
            if is_key_pressed(KeyCode::E) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                }
                state.ui_state.workbench_open = false;
                state.ui_state.workbench_tile = None;
                return true;
            }

            if !state.ui_state.crafting_in_progress {
                // Tab key to cycle tabs
                if is_key_pressed(KeyCode::Tab) {
                    state.ui_state.workbench_tab = (state.ui_state.workbench_tab + 1) % 3;
                    state.ui_state.workbench_selected_recipe = 0;
                    state.ui_state.workbench_scroll_offset = 0.0;
                    state.pending_sfx.push("enter".to_string());
                }

                let tab_sections =
                    crate::render::workbench_sections_for_tab(state.ui_state.workbench_tab);
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
                let recipe_count = workbench_recipes.len();

                // Must match renderer layout in workbench.rs
                let s = state.ui_state.ui_scale;
                let row_h = 56.0 * s;
                let section_header_h = 22.0 * s;
                let section_count = {
                    let mut sections = std::collections::HashSet::new();
                    for r in &workbench_recipes {
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
                let ingredient_count = workbench_recipes
                    .get(state.ui_state.workbench_selected_recipe)
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
                    if state.ui_state.workbench_selected_recipe > 0 {
                        state.ui_state.workbench_selected_recipe -= 1;
                        let item_top = state.ui_state.workbench_selected_recipe as f32 * row_h;
                        if item_top < state.ui_state.workbench_scroll_offset {
                            state.ui_state.workbench_scroll_offset = item_top;
                        }
                    }
                }
                if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                    if state.ui_state.workbench_selected_recipe < recipe_count.saturating_sub(1) {
                        state.ui_state.workbench_selected_recipe += 1;
                        let item_bottom =
                            (state.ui_state.workbench_selected_recipe + 1) as f32 * row_h;
                        if item_bottom > state.ui_state.workbench_scroll_offset + recipe_list_h {
                            state.ui_state.workbench_scroll_offset = item_bottom - recipe_list_h;
                        }
                    }
                }

                // +/- to adjust quantity
                if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
                    state.ui_state.workbench_quantity =
                        (state.ui_state.workbench_quantity + 1).min(99);
                }
                if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
                    if state.ui_state.workbench_quantity > 1 {
                        state.ui_state.workbench_quantity -= 1;
                    }
                }

                // Enter or C to craft
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::C) {
                    if let Some(recipe) =
                        workbench_recipes.get(state.ui_state.workbench_selected_recipe)
                    {
                        commands.push(InputCommand::WorkbenchCraft {
                            recipe_id: recipe.id.clone(),
                            quantity: state.ui_state.workbench_quantity,
                        });
                    }
                }

                // Mouse wheel scrolling
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y != 0.0 {
                    const SCROLL_SPEED: f32 = 30.0;
                    state.ui_state.workbench_scroll_offset =
                        (state.ui_state.workbench_scroll_offset - wheel_y * SCROLL_SPEED)
                            .clamp(0.0, max_scroll);
                }

                // Scrollbar drag handling
                if let Some(track_bounds) = layout.get_bounds(&UiElementId::WorkbenchScrollbar) {
                    let clicked_on =
                        matches!(clicked_element, Some(UiElementId::WorkbenchScrollbar));
                    crate::ui::scroll::handle_scrollbar_drag(
                        &mut state.ui_state.workbench_scroll_drag,
                        &mut state.ui_state.workbench_scroll_offset,
                        max_scroll,
                        track_bounds,
                        total_content,
                        my,
                        is_mouse_button_down(MouseButton::Left),
                        mouse_clicked,
                        clicked_on,
                    );
                } else if !is_mouse_button_down(MouseButton::Left) {
                    state.ui_state.workbench_scroll_drag.dragging = false;
                }
            }

            // Don't process other input while workbench is open
            return true;
        }

        false
    }
}
