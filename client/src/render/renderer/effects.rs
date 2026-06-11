use super::*;

impl Renderer {
    pub(super) fn render_click_effects(&self, state: &GameState) {
        // Temporarily disabled
        return;
        use crate::game::state::ClickEffectKind;

        for effect in &state.click_effects {
            let texture = match effect.kind {
                ClickEffectKind::Walk => &self.click_walk_texture,
                ClickEffectKind::Attack => &self.click_attack_texture,
                ClickEffectKind::Interact => &self.click_interact_texture,
            };
            let Some(tex) = texture.as_ref() else {
                continue;
            };

            let frame = effect.frame();
            let frame_size = crate::game::state::ClickEffect::FRAME_SIZE;
            let source_rect = Rect::new(frame as f32 * frame_size, 0.0, frame_size, frame_size);

            // Convert exact world position to screen space
            let (screen_x, screen_y) = world_to_screen(effect.tile_x, effect.tile_y, &state.camera);

            let zoom = state.camera.zoom;
            let draw_size = frame_size * zoom;

            // Fade out over the last quarter of the animation
            let alpha = if effect.elapsed > crate::game::state::ClickEffect::DURATION * 0.75 {
                let t = (effect.elapsed - crate::game::state::ClickEffect::DURATION * 0.75)
                    / (crate::game::state::ClickEffect::DURATION * 0.25);
                1.0 - t
            } else {
                1.0
            };

            draw_texture_ex(
                tex,
                screen_x - draw_size * 0.5,
                screen_y - draw_size * 0.5,
                Color::new(1.0, 1.0, 1.0, alpha),
                DrawTextureParams {
                    source: Some(source_rect),
                    dest_size: Some(Vec2::new(draw_size, draw_size)),
                    ..Default::default()
                },
            );
        }
    }

    pub(super) fn render_projectiles(&self, state: &GameState) {
        let current_time = macroquad::time::get_time();

        for projectile in &state.projectiles {
            let (world_x, world_y, world_z) = projectile.current_pos(current_time);
            let (screen_x, screen_y_raw) =
                world_to_screen_z(world_x, world_y, world_z, &state.camera);

            // Sprite-based projectile (blast spell)
            if projectile.sprite.ends_with("_blast") {
                if let Some((texture, atlas_offset)) =
                    self.spell_effect_textures.get(&projectile.sprite)
                {
                    let (tex_w, tex_h) = self
                        .spell_effect_textures
                        .get_dimensions(&projectile.sprite)
                        .unwrap_or((texture.width(), texture.height()));
                    let frame_count = 4usize;
                    let frame_w = tex_w / frame_count as f32;
                    let frame_h = tex_h;

                    // Animate: cycle through frames
                    let elapsed = current_time - projectile.start_time;
                    let fps = 10.0;
                    let frame_idx = ((elapsed * fps) as usize) % frame_count;

                    let (offset_x, offset_y) = atlas_offset.unwrap_or((0.0, 0.0));
                    let source_rect = Rect::new(
                        offset_x + frame_idx as f32 * frame_w,
                        offset_y,
                        frame_w,
                        frame_h,
                    );

                    let zoom = state.camera.zoom;
                    let draw_w = frame_w * zoom;
                    let draw_h = frame_h * zoom;
                    let y_offset = -24.0 * zoom;

                    draw_texture_ex(
                        texture,
                        screen_x - draw_w / 2.0,
                        screen_y_raw + y_offset - draw_h / 2.0,
                        WHITE,
                        DrawTextureParams {
                            source: Some(source_rect),
                            dest_size: Some(Vec2::new(draw_w, draw_h)),
                            ..Default::default()
                        },
                    );
                }
                continue;
            }

            // Drawn arrow projectile — follows arc tangent angle
            let arrow_y_offset = -24.0 * state.camera.zoom;
            let screen_y = screen_y_raw + arrow_y_offset;

            // Use arc tangent for direction so arrow follows the arc
            let current_time = macroquad::time::get_time();
            let (vel_x, vel_y, vel_z) = projectile.current_direction(current_time);

            // Convert velocity to screen space
            let sv_x = (vel_x - vel_y) * 32.0; // TILE_WIDTH/2
            let sv_y = (vel_x + vel_y) * 16.0 - vel_z * 32.0; // TILE_HEIGHT/2, Z offset
            let len = (sv_x * sv_x + sv_y * sv_y).sqrt().max(0.001);
            let dir_x = sv_x / len;
            let dir_y = sv_y / len;

            // Perpendicular vector for arrow width
            let perp_x = -dir_y;
            let perp_y = dir_x;

            // Arrow dimensions
            let shaft_length = 18.0;
            let shaft_width = 2.0;
            let head_length = 6.0;
            let head_width = 5.0;
            let fletch_length = 4.0;
            let fletch_width = 3.0;

            // Colors
            let shaft_color = Color::new(0.55, 0.35, 0.15, 1.0); // Wood brown
            let head_color = Color::new(0.45, 0.45, 0.5, 1.0); // Metal gray
            let fletch_color = Color::new(0.85, 0.85, 0.8, 1.0); // Light feathers

            // Arrow positions
            let tip_x = screen_x + dir_x * (shaft_length / 2.0 + head_length);
            let tip_y = screen_y + dir_y * (shaft_length / 2.0 + head_length);
            let back_x = screen_x - dir_x * shaft_length / 2.0;
            let back_y = screen_y - dir_y * shaft_length / 2.0;

            // Draw shaft (thick line)
            draw_line(
                back_x,
                back_y,
                screen_x + dir_x * shaft_length / 2.0,
                screen_y + dir_y * shaft_length / 2.0,
                shaft_width,
                shaft_color,
            );

            // Draw arrowhead (triangle pointing forward)
            let head_base_x = screen_x + dir_x * shaft_length / 2.0;
            let head_base_y = screen_y + dir_y * shaft_length / 2.0;
            draw_triangle(
                Vec2::new(tip_x, tip_y),
                Vec2::new(
                    head_base_x + perp_x * head_width / 2.0,
                    head_base_y + perp_y * head_width / 2.0,
                ),
                Vec2::new(
                    head_base_x - perp_x * head_width / 2.0,
                    head_base_y - perp_y * head_width / 2.0,
                ),
                head_color,
            );

            // Draw fletching (two small triangles at the back)
            let fletch_base_x = back_x + dir_x * fletch_length;
            let fletch_base_y = back_y + dir_y * fletch_length;

            // Left fletch
            draw_triangle(
                Vec2::new(
                    back_x + perp_x * shaft_width / 2.0,
                    back_y + perp_y * shaft_width / 2.0,
                ),
                Vec2::new(
                    fletch_base_x + perp_x * shaft_width / 2.0,
                    fletch_base_y + perp_y * shaft_width / 2.0,
                ),
                Vec2::new(
                    back_x + perp_x * fletch_width,
                    back_y + perp_y * fletch_width,
                ),
                fletch_color,
            );

            // Right fletch
            draw_triangle(
                Vec2::new(
                    back_x - perp_x * shaft_width / 2.0,
                    back_y - perp_y * shaft_width / 2.0,
                ),
                Vec2::new(
                    fletch_base_x - perp_x * shaft_width / 2.0,
                    fletch_base_y - perp_y * shaft_width / 2.0,
                ),
                Vec2::new(
                    back_x - perp_x * fletch_width,
                    back_y - perp_y * fletch_width,
                ),
                fletch_color,
            );
        }
    }

    pub(super) fn render_spell_effects(&self, state: &GameState) {
        let current_time = macroquad::time::get_time();

        for effect in &state.spell_effects {
            let elapsed = current_time - effect.time;

            // Look up the effect sprite based on spell_id
            // Skip rocks_aoe — it's depth-sorted with entities
            let sprite_name = match effect.spell_id.as_str() {
                "dark_hand" => "dark_hand",
                "lightning_bolt" => "lightning_bolt",
                "dark_eater" => "dark_eater",
                "rock_fall" => "rock_fall",
                "heal" => "self_heal",
                "teleport" | "return_home" => "bubbles_warp",
                "tornado" => "tornado",
                s if s.ends_with("_blast") => continue,
                "rocks_aoe" => continue,
                _ => continue,
            };

            let (texture, atlas_offset) = match self.spell_effect_textures.get(sprite_name) {
                Some(t) => t,
                None => continue,
            };

            // Get sprite dimensions (from atlas rect or texture size)
            let (tex_w, tex_h) = self
                .spell_effect_textures
                .get_dimensions(sprite_name)
                .unwrap_or((texture.width(), texture.height()));
            let frame_count = match sprite_name {
                "rocks_aoe" => 8usize,
                _ => 5usize,
            };
            let frame_w = tex_w / frame_count as f32;
            let frame_h = tex_h;
            let fps = 10.0_f64;
            let total_duration = frame_count as f64 / fps;

            if elapsed > total_duration {
                continue; // Animation finished
            }

            let frame_idx = ((elapsed * fps) as usize).min(frame_count - 1);

            // Calculate screen position from world coordinates
            let (screen_x, screen_y) = world_to_screen(
                effect.target_x as f32,
                effect.target_y as f32,
                &state.camera,
            );

            // Viewport culling - skip off-screen spell effects
            let (sw, sh) = virtual_screen_size();
            let zoom = state.camera.zoom;
            let margin = 100.0 * zoom;
            if screen_x < -margin
                || screen_x > sw + margin
                || screen_y < -margin
                || screen_y > sh + margin
            {
                continue;
            }

            // Draw the current frame, centered on the tile
            let draw_w = frame_w * zoom;
            let draw_h = frame_h * zoom;
            // Apply atlas offset if present
            let (offset_x, offset_y) = atlas_offset.unwrap_or((0.0, 0.0));
            let source_rect = Rect::new(
                offset_x + frame_idx as f32 * frame_w,
                offset_y,
                frame_w,
                frame_h,
            );

            // Align sprite so its bottom edge sits at the tile center
            draw_texture_ex(
                texture,
                screen_x - draw_w / 2.0,
                screen_y - draw_h,
                WHITE,
                DrawTextureParams {
                    source: Some(source_rect),
                    dest_size: Some(Vec2::new(draw_w, draw_h)),
                    ..Default::default()
                },
            );
        }
    }

    /// Render a single spell effect by index (used for depth-sorted effects like rocks_aoe).
    pub(super) fn render_single_spell_effect(&self, state: &GameState, idx: usize) {
        let effect = match state.spell_effects.get(idx) {
            Some(e) => e,
            None => return,
        };
        let current_time = macroquad::time::get_time();
        let elapsed = current_time - effect.time;

        let sprite_name = match effect.spell_id.as_str() {
            s if s.ends_with("_blast") => return,
            "rocks_aoe" => "rocks_aoe",
            other => other,
        };

        let (texture, atlas_offset) = match self.spell_effect_textures.get(sprite_name) {
            Some(t) => t,
            None => return,
        };

        let (tex_w, tex_h) = self
            .spell_effect_textures
            .get_dimensions(sprite_name)
            .unwrap_or((texture.width(), texture.height()));
        let frame_count = match sprite_name {
            "rocks_aoe" => 8usize,
            _ => 5usize,
        };
        let frame_w = tex_w / frame_count as f32;
        let frame_h = tex_h;
        let fps = 10.0_f64;
        let total_duration = frame_count as f64 / fps;

        if elapsed > total_duration {
            return;
        }

        let frame_idx = ((elapsed * fps) as usize).min(frame_count - 1);
        let zoom = state.camera.zoom;

        let (screen_x, screen_y) = world_to_screen(
            effect.target_x as f32,
            effect.target_y as f32,
            &state.camera,
        );

        let draw_w = frame_w * zoom;
        let draw_h = frame_h * zoom;
        let (offset_x, offset_y) = atlas_offset.unwrap_or((0.0, 0.0));
        let source_rect = Rect::new(
            offset_x + frame_idx as f32 * frame_w,
            offset_y,
            frame_w,
            frame_h,
        );

        draw_texture_ex(
            texture,
            screen_x - draw_w / 2.0,
            screen_y - draw_h,
            WHITE,
            DrawTextureParams {
                source: Some(source_rect),
                dest_size: Some(Vec2::new(draw_w, draw_h)),
                ..Default::default()
            },
        );
    }

    pub(super) fn render_damage_numbers(&self, state: &GameState) {
        let current_time = macroquad::time::get_time();
        let zoom = state.camera.zoom;
        const DURATION: f32 = 1.2;
        let font_size = 16.0 * zoom;

        for event in &state.damage_events {
            let age = (current_time - event.time) as f32;
            if age > DURATION {
                continue;
            }

            let t = age / DURATION;

            // Steady float upward - round to whole pixels for crisp movement, scale with zoom
            let float_offset = (age * 40.0 * zoom).round();

            // Compute height offset based on entity type and actual sprite size
            let height_offset = if state.players.contains_key(&event.target_id) {
                (SPRITE_HEIGHT - 8.0) * zoom / 2.0 // Center of player sprite
            } else if let Some(npc) = state.npcs.get(&event.target_id) {
                // Use actual sprite height if available, otherwise fallback to ellipse size
                if let Some((_, h)) =
                    self.npc_sprites
                        .get_dimensions(&npc.entity_type)
                        .or_else(|| {
                            self.npc_overflow_sprites
                                .get(&npc.entity_type)
                                .map(|t| (t.width(), t.height()))
                        })
                {
                    h * zoom / 2.0 // Center of NPC sprite
                } else {
                    12.0 * zoom // Center of fallback ellipse
                }
            } else {
                25.0 * zoom // Fallback for unknown entities
            };

            // For multi-tile NPCs, center damage numbers on the footprint
            let (dmg_x, dmg_y) = if let Some(npc) = state.npcs.get(&event.target_id) {
                let center_offset = (npc.size - 1) as f32 * 0.5;
                (event.x + center_offset, event.y + center_offset)
            } else {
                (event.x, event.y)
            };
            let (screen_x, screen_y) = world_to_screen(dmg_x, dmg_y, &state.camera);

            // Viewport culling - skip off-screen damage numbers
            let (sw, sh) = virtual_screen_size();
            let margin = 100.0 * zoom;
            if screen_x < -margin
                || screen_x > sw + margin
                || screen_y < -margin
                || screen_y > sh + margin
            {
                continue;
            }

            // Round all positions to whole pixels
            let final_y = (screen_y - height_offset - float_offset).round();

            // Fade: visible for first half, then fade out
            let alpha = if t < 0.5 { 1.0 } else { 1.0 - (t - 0.5) * 2.0 };

            // Text and color
            let (text, base_color) = if event.damage > 0 {
                (
                    format!("-{}", event.damage),
                    Color::new(1.0, 0.3, 0.2, alpha),
                )
            } else if event.damage < 0 {
                (
                    format!("+{}", -event.damage),
                    Color::new(0.3, 1.0, 0.4, alpha),
                )
            } else {
                ("MISS".to_string(), Color::new(0.6, 0.6, 0.6, alpha))
            };

            let text_dims = self.measure_text_sharp(&text, font_size);
            // Round center position to whole pixels
            let draw_x = (screen_x - text_dims.width / 2.0).round();

            // Outline/shadow for readability
            let outline_offset = 1.0 * zoom;
            let outline_color = Color::new(0.0, 0.0, 0.0, alpha * 0.9);
            if state.ui_state.graphics_low {
                // Single shadow offset (2 draws total instead of 5)
                self.draw_text_sharp(
                    &text,
                    draw_x + outline_offset,
                    final_y + outline_offset,
                    font_size,
                    outline_color,
                );
            } else {
                for &(ox, oy) in &[
                    (-outline_offset, -outline_offset),
                    (outline_offset, -outline_offset),
                    (-outline_offset, outline_offset),
                    (outline_offset, outline_offset),
                ] {
                    self.draw_text_sharp(
                        &text,
                        draw_x + ox,
                        final_y + oy,
                        font_size,
                        outline_color,
                    );
                }
            }

            self.draw_text_sharp(&text, draw_x, final_y, font_size, base_color);
        }
    }
}
