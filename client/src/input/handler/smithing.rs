use super::*;

impl InputHandler {
    pub(super) fn handle_smithing(
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
        // Handle anvil mode
        if state.ui_state.anvil_open {
            // Handle mouse clicks on anvil elements
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::AnvilCloseButton => {
                            state.ui_state.anvil_open = false;
                            state.ui_state.anvil_tile = None;
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            state.pending_sfx.push("enter".to_string());
                            return true;
                        }
                        UiElementId::AnvilRecipeCell(idx) => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.anvil_selected_recipe = *idx;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return true;
                        }
                        UiElementId::AnvilSmithButton => {
                            if !state.ui_state.crafting_in_progress {
                                let section_filter = if state.ui_state.anvil_tab == 0 {
                                    "materials"
                                } else {
                                    "equipment"
                                };
                                let mut anvil_recipes: Vec<_> = state
                                    .recipe_definitions
                                    .iter()
                                    .filter(|r| r.station.as_deref() == Some("anvil"))
                                    .filter(|r| r.section.as_deref() == Some(section_filter))
                                    .filter(|r| {
                                        !r.requires_discovery
                                            || state.discovered_recipes.contains(&r.id)
                                    })
                                    .collect();
                                anvil_recipes.sort_by_key(|r| r.level_required);
                                if let Some(recipe) =
                                    anvil_recipes.get(state.ui_state.anvil_selected_recipe)
                                {
                                    commands.push(InputCommand::AnvilCraft {
                                        recipe_id: recipe.id.clone(),
                                        quantity: state.ui_state.anvil_quantity,
                                    });
                                }
                            }
                            return true;
                        }
                        UiElementId::AnvilCancelButton => {
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            return true;
                        }
                        UiElementId::AnvilQuantity1 => {
                            state.ui_state.anvil_quantity = 1;
                            return true;
                        }
                        UiElementId::AnvilQuantityX => {
                            state.ui_state.anvil_quantity = if state.ui_state.anvil_quantity == 5 {
                                10
                            } else {
                                5
                            };
                            return true;
                        }
                        UiElementId::AnvilQuantityAll => {
                            state.ui_state.anvil_quantity = u32::MAX;
                            return true;
                        }
                        UiElementId::AnvilTabMaterials => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.anvil_tab = 0;
                                state.ui_state.anvil_selected_recipe = 0;
                                state.ui_state.anvil_scroll_offset = 0.0;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return true;
                        }
                        UiElementId::AnvilTabEquipment => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.anvil_tab = 1;
                                state.ui_state.anvil_selected_recipe = 0;
                                state.ui_state.anvil_scroll_offset = 0.0;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            // Allow chat input while anvil panel is open
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
                state.ui_state.anvil_open = false;
                state.ui_state.anvil_tile = None;
                return true;
            }

            // E key closes anvil
            if is_key_pressed(KeyCode::E) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                }
                state.ui_state.anvil_open = false;
                state.ui_state.anvil_tile = None;
                return true;
            }

            if !state.ui_state.crafting_in_progress {
                // Tab key to cycle tabs
                if is_key_pressed(KeyCode::Tab) {
                    state.ui_state.anvil_tab = (state.ui_state.anvil_tab + 1) % 2;
                    state.ui_state.anvil_selected_recipe = 0;
                    state.ui_state.anvil_scroll_offset = 0.0;
                    state.pending_sfx.push("enter".to_string());
                }

                let section_filter = if state.ui_state.anvil_tab == 0 {
                    "materials"
                } else {
                    "equipment"
                };
                let columns = 4;

                // Get recipe count (drop borrow before navigation)
                let recipe_count = {
                    let mut count = 0;
                    for r in &state.recipe_definitions {
                        if r.station.as_deref() == Some("anvil")
                            && r.section.as_deref() == Some(section_filter)
                            && (!r.requires_discovery || state.discovered_recipes.contains(&r.id))
                        {
                            count += 1;
                        }
                    }
                    count
                };

                // Grid navigation: Up/Down moves by row (±columns), Left/Right moves by ±1
                if (is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W))
                    && state.ui_state.anvil_selected_recipe >= columns
                {
                    state.ui_state.anvil_selected_recipe -= columns;
                    self.auto_scroll_anvil_grid(state);
                }
                if (is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S))
                    && state.ui_state.anvil_selected_recipe + columns < recipe_count
                {
                    state.ui_state.anvil_selected_recipe += columns;
                    self.auto_scroll_anvil_grid(state);
                }
                if (is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A))
                    && state.ui_state.anvil_selected_recipe > 0
                {
                    state.ui_state.anvil_selected_recipe -= 1;
                    self.auto_scroll_anvil_grid(state);
                }
                if (is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D))
                    && state.ui_state.anvil_selected_recipe < recipe_count.saturating_sub(1)
                {
                    state.ui_state.anvil_selected_recipe += 1;
                    self.auto_scroll_anvil_grid(state);
                }

                // Quantity keys: 1, X, A (not left/right since those navigate grid)
                if is_key_pressed(KeyCode::Key1) {
                    state.ui_state.anvil_quantity = 1;
                }
                if is_key_pressed(KeyCode::X) {
                    state.ui_state.anvil_quantity = if state.ui_state.anvil_quantity == 5 {
                        10
                    } else {
                        5
                    };
                }

                // Enter or C to smith
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::C) {
                    let mut anvil_recipes: Vec<_> = state
                        .recipe_definitions
                        .iter()
                        .filter(|r| r.station.as_deref() == Some("anvil"))
                        .filter(|r| r.section.as_deref() == Some(section_filter))
                        .filter(|r| {
                            !r.requires_discovery || state.discovered_recipes.contains(&r.id)
                        })
                        .collect();
                    anvil_recipes.sort_by_key(|r| r.level_required);
                    if let Some(recipe) = anvil_recipes.get(state.ui_state.anvil_selected_recipe) {
                        commands.push(InputCommand::AnvilCraft {
                            recipe_id: recipe.id.clone(),
                            quantity: state.ui_state.anvil_quantity,
                        });
                    }
                }

                // Mouse wheel scrolling
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y != 0.0 {
                    const SCROLL_SPEED: f32 = 30.0;
                    let s = state.ui_state.ui_scale;
                    let cell_h = 106.0 * s;
                    let gap = 6.0 * s;
                    let rows = recipe_count.div_ceil(columns);
                    let total_content = rows as f32 * (cell_h + gap);
                    let (_, sh) = crate::util::virtual_screen_size();
                    // Match renderer: bottom_bar_h reserved, panel capped
                    let bottom_bar_h = 53.0 * s + 8.0;
                    let panel_h = (500.0 * s).min(sh - bottom_bar_h - 8.0);
                    // content_h = panel_h - frame/header/tabs/footer/gaps - controls_strip
                    let content_h = panel_h - 10.0 - 142.0 * s;
                    let max_scroll = (total_content - content_h).max(0.0);
                    state.ui_state.anvil_scroll_offset = (state.ui_state.anvil_scroll_offset
                        - wheel_y * SCROLL_SPEED)
                        .clamp(0.0, max_scroll);
                }

                // Scrollbar drag handling
                if let Some(track_bounds) = layout.get_bounds(&UiElementId::AnvilScrollbar) {
                    let s = state.ui_state.ui_scale;
                    let cell_h = 106.0 * s;
                    let gap = 6.0 * s;
                    let rows = recipe_count.div_ceil(columns);
                    let total_content = rows as f32 * (cell_h + gap);
                    let (_, sh) = crate::util::virtual_screen_size();
                    let bottom_bar_h = 53.0 * s + 8.0;
                    let panel_h = (500.0 * s).min(sh - bottom_bar_h - 8.0);
                    let content_h = panel_h - 10.0 - 142.0 * s;
                    let max_scroll = (total_content - content_h).max(0.0);
                    let clicked_on = matches!(clicked_element, Some(UiElementId::AnvilScrollbar));
                    crate::ui::scroll::handle_scrollbar_drag(
                        &mut state.ui_state.anvil_scroll_drag,
                        &mut state.ui_state.anvil_scroll_offset,
                        max_scroll,
                        track_bounds,
                        total_content,
                        my,
                        is_mouse_button_down(MouseButton::Left),
                        mouse_clicked,
                        clicked_on,
                    );
                } else if !is_mouse_button_down(MouseButton::Left) {
                    state.ui_state.anvil_scroll_drag.dragging = false;
                }
            }

            // Don't process other input while anvil is open
            return true;
        }

        false
    }
}
