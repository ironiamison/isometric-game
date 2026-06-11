use super::*;

impl InputHandler {
    pub(super) fn handle_fletching(
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
        // ===== FLETCHING PANEL (tool-based, no station) =====
        if state.ui_state.fletching_open {
            // Handle mouse clicks on fletching panel elements
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::FletchingCloseButton => {
                            state.ui_state.fletching_open = false;
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            state.pending_sfx.push("enter".to_string());
                            return true;
                        }
                        UiElementId::FletchingRecipeItem(idx) => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.fletching_selected_recipe = *idx;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return true;
                        }
                        UiElementId::FletchingFletchButton => {
                            if !state.ui_state.crafting_in_progress {
                                let tab_sections = crate::render::fletching_sections_for_tab(
                                    state.ui_state.fletching_tab,
                                );
                                let mut fletching_recipes: Vec<_> = state
                                    .recipe_definitions
                                    .iter()
                                    .filter(|r| {
                                        r.category == "fletching"
                                            && r.required_tool.as_deref() == Some("knife")
                                    })
                                    .filter(|r| {
                                        tab_sections.contains(&r.section.as_deref().unwrap_or(""))
                                    })
                                    .collect();
                                fletching_recipes.sort_by_key(|r| r.level_required);
                                if let Some(recipe) =
                                    fletching_recipes.get(state.ui_state.fletching_selected_recipe)
                                {
                                    commands.push(InputCommand::FletchingCraft {
                                        recipe_id: recipe.id.clone(),
                                        quantity: state.ui_state.fletching_quantity,
                                    });
                                }
                            }
                            return true;
                        }
                        UiElementId::FletchingCancelButton => {
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            return true;
                        }
                        UiElementId::FletchingQuantity1 => {
                            state.ui_state.fletching_quantity = 1;
                            return true;
                        }
                        UiElementId::FletchingQuantityX => {
                            state.ui_state.fletching_quantity = 5;
                            return true;
                        }
                        UiElementId::FletchingQuantityAll => {
                            state.ui_state.fletching_quantity = u32::MAX;
                            return true;
                        }
                        UiElementId::FletchingTab(idx) => {
                            if (*idx as u8) < 3 && !state.ui_state.crafting_in_progress {
                                state.ui_state.fletching_tab = *idx as u8;
                                state.ui_state.fletching_selected_recipe = 0;
                                state.ui_state.fletching_scroll_offset = 0.0;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            // Allow chat input while fletching panel is open
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
                state.ui_state.fletching_open = false;
                return true;
            }

            // E key closes fletching panel
            if is_key_pressed(KeyCode::E) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                }
                state.ui_state.fletching_open = false;
                return true;
            }

            if !state.ui_state.crafting_in_progress {
                // Tab key to cycle tabs
                if is_key_pressed(KeyCode::Tab) {
                    state.ui_state.fletching_tab = (state.ui_state.fletching_tab + 1) % 3;
                    state.ui_state.fletching_selected_recipe = 0;
                    state.ui_state.fletching_scroll_offset = 0.0;
                    state.pending_sfx.push("enter".to_string());
                }

                let tab_sections =
                    crate::render::fletching_sections_for_tab(state.ui_state.fletching_tab);
                let mut fletching_recipes: Vec<_> = state
                    .recipe_definitions
                    .iter()
                    .filter(|r| {
                        r.category == "fletching" && r.required_tool.as_deref() == Some("knife")
                    })
                    .filter(|r| tab_sections.contains(&r.section.as_deref().unwrap_or("")))
                    .collect();
                fletching_recipes.sort_by_key(|r| r.level_required);
                let recipe_count = fletching_recipes.len();

                let s = state.ui_state.ui_scale;
                let row_h = 72.0 * s;
                let total_content = recipe_count as f32 * row_h;

                let (_, sh) = crate::util::virtual_screen_size();
                let panel_h = (450.0 * s).min(sh - 16.0);
                let header_h = 40.0 * s;
                let footer_h = 30.0 * s;
                let tab_h = 28.0 * s;
                let frame = 4.0;
                let content_h =
                    panel_h - frame * 2.0 - header_h - 2.0 - tab_h - 4.0 * s - footer_h - 4.0 * s;
                let max_scroll = (total_content - content_h).max(0.0);

                // W/S or Up/Down to navigate recipes
                if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                    if state.ui_state.fletching_selected_recipe > 0 {
                        state.ui_state.fletching_selected_recipe -= 1;
                        let item_top = state.ui_state.fletching_selected_recipe as f32 * row_h;
                        if item_top < state.ui_state.fletching_scroll_offset {
                            state.ui_state.fletching_scroll_offset = item_top;
                        }
                    }
                }
                if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                    if state.ui_state.fletching_selected_recipe < recipe_count.saturating_sub(1) {
                        state.ui_state.fletching_selected_recipe += 1;
                        let item_bottom =
                            (state.ui_state.fletching_selected_recipe + 1) as f32 * row_h;
                        if item_bottom > state.ui_state.fletching_scroll_offset + content_h {
                            state.ui_state.fletching_scroll_offset = item_bottom - content_h;
                        }
                    }
                }

                // 1/X/A for quantity shortcuts
                if is_key_pressed(KeyCode::Key1) {
                    state.ui_state.fletching_quantity = 1;
                }
                if is_key_pressed(KeyCode::X) {
                    state.ui_state.fletching_quantity = 5;
                }
                if is_key_pressed(KeyCode::A) {
                    state.ui_state.fletching_quantity = u32::MAX;
                }

                // Enter or C to craft
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::C) {
                    if let Some(recipe) =
                        fletching_recipes.get(state.ui_state.fletching_selected_recipe)
                    {
                        commands.push(InputCommand::FletchingCraft {
                            recipe_id: recipe.id.clone(),
                            quantity: state.ui_state.fletching_quantity,
                        });
                    }
                }

                // Mouse wheel scrolling
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y != 0.0 {
                    const SCROLL_SPEED: f32 = 30.0;
                    state.ui_state.fletching_scroll_offset =
                        (state.ui_state.fletching_scroll_offset - wheel_y * SCROLL_SPEED)
                            .clamp(0.0, max_scroll);
                }

                // Scrollbar drag handling
                if let Some(track_bounds) = layout.get_bounds(&UiElementId::FletchingScrollbar) {
                    let clicked_on =
                        matches!(clicked_element, Some(UiElementId::FletchingScrollbar));
                    crate::ui::scroll::handle_scrollbar_drag(
                        &mut state.ui_state.fletching_scroll_drag,
                        &mut state.ui_state.fletching_scroll_offset,
                        max_scroll,
                        track_bounds,
                        total_content,
                        my,
                        is_mouse_button_down(MouseButton::Left),
                        mouse_clicked,
                        clicked_on,
                    );
                } else if !is_mouse_button_down(MouseButton::Left) {
                    state.ui_state.fletching_scroll_drag.dragging = false;
                }
            }

            // Don't process other input while fletching panel is open
            return true;
        }

        false
    }
}
