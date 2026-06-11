use super::*;

impl InputHandler {
    pub fn new() -> Self {
        Self {
            last_dx: 0.0,
            last_dy: 0.0,
            current_dir: MoveDir::None,
            prev_dir: MoveDir::None,
            last_send_time: 0.0,
            send_interval: 0.05, // 50ms keeps facing/move intent responsive
            last_attack_time: 0.0,
            dir_press_time: 0.0,
            move_sent: false,
            auto_path_sent_waypoint: None,
            auto_path_sent_dir: None,
            suppress_move_until: 0.0,
            touch_controls: TouchControls::new(),
            long_press_start: 0.0,
            long_press_pos: (0.0, 0.0),
            long_press_active: false,
            long_press_fired: false,
            player_blocked_since: None,
        }
    }

    /// Load touch control icons (call once after creation in async context)
    pub async fn load_touch_icons(&mut self) {
        self.touch_controls.load_icons().await;
    }

    pub(super) fn reset_auto_path_motion_state(&mut self) {
        self.auto_path_sent_waypoint = None;
        self.auto_path_sent_dir = None;
    }

    pub(super) fn update_touch_controls(&mut self, state: &GameState, current_time: f64) {
        let in_dialogue = state.ui_state.active_dialogue.is_some();
        let any_panel_open = state.ui_state.inventory_open
            || state.ui_state.character_panel_open
            || state.ui_state.skills_open
            || state.ui_state.prayer_book_open
            || state.ui_state.minimap_panel_open
            || state.ui_state.escape_menu_open
            || state.ui_state.crafting_open
            || state.ui_state.furnace_open
            || state.ui_state.anvil_open
            || state.ui_state.fletching_open
            || state.ui_state.shop_data.is_some()
            || state.ui_state.bank_open
            || state.ui_state.chest_open
            || state.ui_state.social_open
            || state.ui_state.chat_panel_open
            || state.ui_state.slayer_panel_open
            || in_dialogue;
        let hide_action_buttons = any_panel_open;
        let hide_direction_controls = state.ui_state.crafting_open
            || state.ui_state.furnace_open
            || state.ui_state.anvil_open
            || state.ui_state.fletching_open
            || state.ui_state.shop_data.is_some()
            || state.ui_state.bank_open
            || state.ui_state.chest_open
            || state.ui_state.minimap_panel_open
            || in_dialogue;
        self.touch_controls.update(
            current_time,
            hide_action_buttons,
            hide_direction_controls,
            state.ui_state.use_joystick,
        );
    }

    pub(super) fn update_hover_state(
        &self,
        state: &mut GameState,
        layout: &UiLayout,
        mx: f32,
        my: f32,
    ) {
        state.ui_state.hovered_element = layout.hit_test(mx, my).cloned();
        mark_chat_channel_as_read(state, state.ui_state.chat_active_tab);

        let touch_active = self.touch_controls.consumed_touch();
        if state.ui_state.hovered_element.is_none() && !touch_active {
            // Pick tile accounting for elevation (elevated tiles take priority)
            let (tile_x, tile_y, tile_z) =
                state
                    .chunk_manager
                    .pick_tile_at_screen(mx, my, &state.camera);
            state.hovered_tile = Some((tile_x, tile_y));
            state.hovered_tile_z = tile_z;

            // Entity hover: compare in screen space so Z-elevated entities
            // are hovered when the cursor visually overlaps them.
            let hover_radius_px =
                0.6 * crate::render::isometric::TILE_WIDTH * state.camera.zoom * 0.5;
            let hover_radius_sq = hover_radius_px * hover_radius_px;
            let mut hovered_entity: Option<String> = None;

            'npc_loop: for npc in state.npcs.values() {
                if npc.state != crate::game::npc::NpcState::Dead {
                    // Check each tile in the NPC's footprint
                    for dy in 0..npc.size {
                        for dx in 0..npc.size {
                            let (sx, sy) = crate::render::isometric::world_to_screen_z_exact(
                                npc.x + dx as f32,
                                npc.y + dy as f32,
                                npc.z,
                                &state.camera,
                            );
                            let dmx = mx - sx;
                            let dmy = my - sy;
                            if dmx * dmx + dmy * dmy < hover_radius_sq {
                                hovered_entity = Some(npc.id.clone());
                                break 'npc_loop;
                            }
                        }
                    }
                }
            }

            if hovered_entity.is_none() {
                for player in state.players.values() {
                    if !player.is_dead {
                        let (sx, sy) = crate::render::isometric::world_to_screen_z_exact(
                            player.x,
                            player.y,
                            player.z,
                            &state.camera,
                        );
                        let dx = mx - sx;
                        let dy = my - sy;
                        if dx * dx + dy * dy < hover_radius_sq {
                            hovered_entity = Some(player.id.clone());
                            break;
                        }
                    }
                }
            }

            state.hovered_entity_id = hovered_entity;
        } else {
            state.hovered_tile = None;
            state.hovered_entity_id = None;
        }
    }

    pub(super) fn current_click_target(
        &mut self,
        layout: &UiLayout,
        mx: f32,
        my: f32,
    ) -> (bool, bool, bool, Option<UiElementId>) {
        let touch_consumed = self.touch_controls.consumed_touch();
        let mouse_clicked = is_mouse_button_pressed(MouseButton::Left) && !touch_consumed;
        let mut mouse_right_clicked = is_mouse_button_pressed(MouseButton::Right);
        let mouse_released = is_mouse_button_released(MouseButton::Left) && !touch_consumed;

        // Long-press detection for right-click on mobile
        if cfg!(target_os = "android") {
            let now = macroquad::time::get_time();
            if mouse_clicked {
                self.long_press_start = now;
                self.long_press_pos = (mx, my);
                self.long_press_active = true;
                self.long_press_fired = false;
            }
            if self.long_press_active && is_mouse_button_down(MouseButton::Left) {
                let dx = mx - self.long_press_pos.0;
                let dy = my - self.long_press_pos.1;
                if dx * dx + dy * dy > 400.0 {
                    // Moved too far (>20px) — cancel long press
                    self.long_press_active = false;
                } else if !self.long_press_fired && now - self.long_press_start > 0.4 {
                    // Long press triggered — fire right click
                    mouse_right_clicked = true;
                    self.long_press_fired = true;
                }
            }
            if mouse_released || !is_mouse_button_down(MouseButton::Left) {
                self.long_press_active = false;
            }
        }

        let clicked_element = if mouse_clicked || mouse_right_clicked || mouse_released {
            layout.hit_test(mx, my).cloned()
        } else {
            None
        };

        (
            mouse_clicked,
            mouse_right_clicked,
            mouse_released,
            clicked_element,
        )
    }
}
