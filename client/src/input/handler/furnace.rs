use super::*;

impl InputHandler {
    pub(super) fn handle_furnace(
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
        // Handle furnace mode
        if state.ui_state.furnace_open {
            // Handle mouse clicks on furnace elements
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::FurnaceCloseButton => {
                            state.ui_state.furnace_open = false;
                            state.ui_state.furnace_tile = None;
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            state.pending_sfx.push("enter".to_string());
                            return true;
                        }
                        UiElementId::FurnaceRecipeItem(idx) => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.furnace_selected_recipe = *idx;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return true;
                        }
                        UiElementId::FurnaceSmeltButton => {
                            if !state.ui_state.crafting_in_progress {
                                let station = state.ui_state.furnace_station_type.as_str();
                                let is_fire_pit = station == "fire_pit";
                                let section_filter = if is_fire_pit {
                                    "fish"
                                } else if state.ui_state.furnace_tab == 0 {
                                    "materials"
                                } else {
                                    "jewelry"
                                };
                                let mut furnace_recipes: Vec<_> = state
                                    .recipe_definitions
                                    .iter()
                                    .filter(|r| r.station.as_deref() == Some(station))
                                    .filter(|r| {
                                        if is_fire_pit {
                                            true
                                        } else {
                                            r.section.as_deref() == Some(section_filter)
                                        }
                                    })
                                    .filter(|r| {
                                        !r.requires_discovery
                                            || state.discovered_recipes.contains(&r.id)
                                    })
                                    .collect();
                                furnace_recipes.sort_by_key(|r| r.level_required);
                                if let Some(recipe) =
                                    furnace_recipes.get(state.ui_state.furnace_selected_recipe)
                                {
                                    commands.push(InputCommand::FurnaceCraft {
                                        recipe_id: recipe.id.clone(),
                                        quantity: state.ui_state.furnace_quantity,
                                    });
                                }
                            }
                            return true;
                        }
                        UiElementId::FurnaceCancelButton => {
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            return true;
                        }
                        UiElementId::FurnaceQuantity1 => {
                            state.ui_state.furnace_quantity = 1;
                            return true;
                        }
                        UiElementId::FurnaceQuantityX => {
                            // Toggle to a reasonable default (5) or cycle
                            state.ui_state.furnace_quantity =
                                if state.ui_state.furnace_quantity == 5 {
                                    10
                                } else {
                                    5
                                };
                            return true;
                        }
                        UiElementId::FurnaceQuantityAll => {
                            state.ui_state.furnace_quantity = u32::MAX;
                            return true;
                        }
                        UiElementId::FurnaceTabSmelting => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.furnace_tab = 0;
                                state.ui_state.furnace_selected_recipe = 0;
                                state.ui_state.furnace_scroll_offset = 0.0;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return true;
                        }
                        UiElementId::FurnaceTabJewelry => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.furnace_tab = 1;
                                state.ui_state.furnace_selected_recipe = 0;
                                state.ui_state.furnace_scroll_offset = 0.0;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            // Allow chat input while furnace panel is open
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
                state.ui_state.furnace_open = false;
                state.ui_state.furnace_tile = None;
                return true;
            }

            // E key closes furnace
            if is_key_pressed(KeyCode::E) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                }
                state.ui_state.furnace_open = false;
                state.ui_state.furnace_tile = None;
                return true;
            }

            if !state.ui_state.crafting_in_progress {
                // Tab key to cycle tabs
                if is_key_pressed(KeyCode::Tab) {
                    state.ui_state.furnace_tab = (state.ui_state.furnace_tab + 1) % 2;
                    state.ui_state.furnace_selected_recipe = 0;
                    state.ui_state.furnace_scroll_offset = 0.0;
                    state.pending_sfx.push("enter".to_string());
                }

                let station = state.ui_state.furnace_station_type.as_str();
                let is_fire_pit = station == "fire_pit";
                let section_filter = if is_fire_pit {
                    "fish"
                } else if state.ui_state.furnace_tab == 0 {
                    "materials"
                } else {
                    "jewelry"
                };
                let mut furnace_recipes: Vec<_> = state
                    .recipe_definitions
                    .iter()
                    .filter(|r| r.station.as_deref() == Some(station))
                    .filter(|r| {
                        if is_fire_pit {
                            true
                        } else {
                            r.section.as_deref() == Some(section_filter)
                        }
                    })
                    .filter(|r| !r.requires_discovery || state.discovered_recipes.contains(&r.id))
                    .collect();
                furnace_recipes.sort_by_key(|r| r.level_required);
                let recipe_count = furnace_recipes.len();

                // W/S or Up/Down to navigate recipes
                if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                    if state.ui_state.furnace_selected_recipe > 0 {
                        state.ui_state.furnace_selected_recipe -= 1;
                        // Auto-scroll to keep selected in view
                        let s = state.ui_state.ui_scale;
                        let row_h = 72.0 * s;
                        let item_top = state.ui_state.furnace_selected_recipe as f32 * row_h;
                        if item_top < state.ui_state.furnace_scroll_offset {
                            state.ui_state.furnace_scroll_offset = item_top;
                        }
                    }
                }
                if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                    if state.ui_state.furnace_selected_recipe < recipe_count.saturating_sub(1) {
                        state.ui_state.furnace_selected_recipe += 1;
                        // Auto-scroll to keep selected in view
                        let s = state.ui_state.ui_scale;
                        let row_h = 72.0 * s;
                        let item_bottom =
                            (state.ui_state.furnace_selected_recipe + 1) as f32 * row_h;
                        let (_, sh) = crate::util::virtual_screen_size();
                        let is_fire_pit = state.ui_state.furnace_station_type == "fire_pit";
                        let panel_h = (450.0 * s).min(sh - 16.0);
                        let content_h = if is_fire_pit {
                            panel_h - 10.0 - 78.0 * s
                        } else {
                            panel_h - 10.0 - 106.0 * s
                        };
                        if item_bottom > state.ui_state.furnace_scroll_offset + content_h {
                            state.ui_state.furnace_scroll_offset = item_bottom - content_h;
                        }
                    }
                }

                // Quantity keys: 1, X, A
                if is_key_pressed(KeyCode::Key1) {
                    state.ui_state.furnace_quantity = 1;
                }
                if is_key_pressed(KeyCode::X) {
                    state.ui_state.furnace_quantity = if state.ui_state.furnace_quantity == 5 {
                        10
                    } else {
                        5
                    };
                }
                if is_key_pressed(KeyCode::A) {
                    state.ui_state.furnace_quantity = u32::MAX;
                }

                // Enter or C to smelt
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::C) {
                    if let Some(recipe) =
                        furnace_recipes.get(state.ui_state.furnace_selected_recipe)
                    {
                        commands.push(InputCommand::FurnaceCraft {
                            recipe_id: recipe.id.clone(),
                            quantity: state.ui_state.furnace_quantity,
                        });
                    }
                }

                // Mouse wheel scrolling
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y != 0.0 {
                    const SCROLL_SPEED: f32 = 30.0;
                    let s = state.ui_state.ui_scale;
                    let row_h = 72.0 * s;
                    let total_content = recipe_count as f32 * row_h;
                    let (_, sh) = crate::util::virtual_screen_size();
                    let is_fire_pit = state.ui_state.furnace_station_type == "fire_pit";
                    let panel_h = (450.0 * s).min(sh - 16.0);
                    // Match renderer: content_h = panel_h - 10.0 - (header+tab+footer+gaps)*s
                    let content_h = if is_fire_pit {
                        panel_h - 10.0 - 78.0 * s
                    } else {
                        panel_h - 10.0 - 106.0 * s
                    };
                    let max_scroll = (total_content - content_h).max(0.0);
                    state.ui_state.furnace_scroll_offset = (state.ui_state.furnace_scroll_offset
                        - wheel_y * SCROLL_SPEED)
                        .clamp(0.0, max_scroll);
                }

                // Scrollbar drag handling
                if let Some(track_bounds) = layout.get_bounds(&UiElementId::FurnaceScrollbar) {
                    let s = state.ui_state.ui_scale;
                    let row_h = 72.0 * s;
                    let total_content = recipe_count as f32 * row_h;
                    let (_, sh) = crate::util::virtual_screen_size();
                    let is_fire_pit = state.ui_state.furnace_station_type == "fire_pit";
                    let panel_h = (450.0 * s).min(sh - 16.0);
                    let content_h = if is_fire_pit {
                        panel_h - 10.0 - 78.0 * s
                    } else {
                        panel_h - 10.0 - 106.0 * s
                    };
                    let max_scroll = (total_content - content_h).max(0.0);
                    let clicked_on = matches!(clicked_element, Some(UiElementId::FurnaceScrollbar));
                    crate::ui::scroll::handle_scrollbar_drag(
                        &mut state.ui_state.furnace_scroll_drag,
                        &mut state.ui_state.furnace_scroll_offset,
                        max_scroll,
                        track_bounds,
                        total_content,
                        my,
                        is_mouse_button_down(MouseButton::Left),
                        mouse_clicked,
                        clicked_on,
                    );
                } else if !is_mouse_button_down(MouseButton::Left) {
                    state.ui_state.furnace_scroll_drag.dragging = false;
                }
            }

            // Don't process other input while furnace is open
            return true;
        }

        false
    }
}
