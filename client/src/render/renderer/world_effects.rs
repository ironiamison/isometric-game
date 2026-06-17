use super::*;

impl Renderer {
    pub(super) fn render_gathering_markers(&self, state: &GameState) {
        if !state.debug_mode {
            return;
        }
        let zoom = state.camera.zoom;
        for marker in &state.gathering_markers {
            // Map skill type to sprite name
            let sprite_id = match marker.skill.as_str() {
                "fishing" => "trout",
                _ => continue,
            };

            let (screen_x, screen_y) =
                world_to_screen(marker.x as f32, marker.y as f32, &state.camera);

            if let Some((texture, source_rect)) = self.item_sprites.get(sprite_id) {
                let (sprite_w, sprite_h) = if let Some(r) = source_rect {
                    (r.w, r.h)
                } else {
                    (texture.width(), texture.height())
                };
                let icon_width = sprite_w * zoom;
                let icon_height = sprite_h * zoom;

                let alpha = Color::new(1.0, 1.0, 1.0, 0.7);
                draw_texture_ex(
                    texture,
                    screen_x - icon_width / 2.0,
                    screen_y - icon_height / 2.0,
                    alpha,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(icon_width, icon_height)),
                        source: source_rect,
                        ..Default::default()
                    },
                );
            }
        }
    }

    /// Render AOE warning zones as pulsing overlays on tiles
    pub(super) fn render_aoe_warnings(&self, state: &GameState) {
        let zoom = state.camera.zoom;
        let time = macroquad::time::get_time();
        let (sw, sh) = virtual_screen_size();

        for warning in &state.aoe_warnings {
            let elapsed_ms = (time - warning.created_at) * 1000.0;
            let progress = (elapsed_ms / warning.delay_ms as f64).min(1.0) as f32;

            // Pulse faster as landing time approaches
            let pulse_speed = 4.0 + progress as f64 * 12.0;
            let pulse = ((time * pulse_speed).sin() as f32 * 0.5 + 0.5) * 0.4 + 0.15;

            // Color based on effect type - default red for damage
            let base_color = match warning.effect.as_str() {
                "sandstorm" => Color::new(0.9, 0.7, 0.2, pulse * progress),
                // Soul Ward (Reaper): a "stand here to cleanse" safe zone. Green,
                // and it fades IN as the timer runs down (inverse of a danger zone).
                "soul_ward" => Color::new(0.25, 0.95, 0.45, pulse * (1.0 - progress) + 0.12),
                _ => Color::new(0.9, 0.2, 0.1, pulse * progress),
            };

            for &(tx, ty) in &warning.tiles {
                let (screen_x, screen_y) = world_to_screen(tx as f32, ty as f32, &state.camera);

                // Viewport culling
                let margin = 64.0 * zoom;
                if screen_x < -margin
                    || screen_x > sw + margin
                    || screen_y < -margin
                    || screen_y > sh + margin
                {
                    continue;
                }

                // Draw isometric diamond highlight
                let half_w = (TILE_WIDTH / 2.0) * zoom * 0.5;
                let half_h = (TILE_HEIGHT / 2.0) * zoom * 0.5;

                draw_triangle(
                    Vec2::new(screen_x, screen_y - half_h),
                    Vec2::new(screen_x + half_w, screen_y),
                    Vec2::new(screen_x, screen_y + half_h),
                    base_color,
                );
                draw_triangle(
                    Vec2::new(screen_x, screen_y - half_h),
                    Vec2::new(screen_x - half_w, screen_y),
                    Vec2::new(screen_x, screen_y + half_h),
                    base_color,
                );
            }
        }
    }

    /// Render Reaper "Soul Wraiths" as translucent, drifting ghost-copies of the
    /// player whose mark failed — full equipped appearance, drawn with a spectral
    /// tint. The source player id is encoded in the wraith NPC id after "::".
    pub(super) fn render_soul_wraiths(&self, state: &GameState) {
        use crate::game::Direction;
        use crate::render::animation::AnimationState;

        let time = macroquad::time::get_time();
        let zoom = state.camera.zoom;

        // The Reaper's position — souls face/drift toward it.
        let reaper_pos = state
            .npcs
            .values()
            .find(|n| n.entity_type == "reaper")
            .map(|b| (b.x, b.y));

        const FADE_IN: f32 = 0.5;
        const FADE_OUT: f32 = 0.6;

        for npc in state.npcs.values() {
            if npc.entity_type != "wraith" || npc.is_death_animation_complete() {
                continue;
            }

            // Whose soul is this? Decode the source player (after "::"), falling
            // back to the local player (so a solo player always sees their own).
            let Some(source) = npc
                .id
                .split("::")
                .nth(1)
                .and_then(|id| state.players.get(id))
                .or_else(|| {
                    state
                        .local_player_id
                        .as_deref()
                        .and_then(|id| state.players.get(id))
                })
            else {
                continue;
            };

            // Fade in on spawn, fade out while dying (killed or consumed).
            let fade_in = (((time - npc.spawned_at) as f32) / FADE_IN).clamp(0.0, 1.0);
            let fade_out = match npc.death_timer {
                Some(t) => (1.0 - t / FADE_OUT).clamp(0.0, 1.0),
                None => 1.0,
            };
            let fade = fade_in * fade_out;
            if fade <= 0.0 {
                continue;
            }

            // Clone the player's full appearance (body, hair, equipment) and pose
            // it as a soul FLOATING toward the Reaper — idle stance (no walk) with
            // a gentle hover + bob. Keep the lift SMALL so the body stays over its
            // real tile and remains click-targetable (picking is tile-based).
            let mut ghost = source.clone();
            ghost.x = npc.x;
            ghost.y = npc.y;
            let bob = (time * 2.2).sin() as f32 * 0.08;
            ghost.z = npc.z + 0.22 + bob; // subtle hover + bob
            ghost.is_dead = false;
            let facing = match reaper_pos {
                Some((bx, by)) => Direction::from_velocity(bx - npc.x, by - npc.y),
                None => Direction::Up,
            };
            ghost.direction = facing;
            ghost.animation.set_state(AnimationState::Idle);
            ghost.animation.direction = facing;
            ghost.animation.frame = 0.0;

            // Soft spectral glow pooled at the feet — brighter when hovered/targeted
            // so players can see it's a clickable target.
            let targeted = state.hovered_entity_id.as_deref() == Some(npc.id.as_str())
                || state.selected_entity_id.as_deref() == Some(npc.id.as_str());
            let (sx, sy) = world_to_screen(npc.x, npc.y, &state.camera);
            let glow_a = if targeted { 0.45 } else { 0.2 };
            let glow_r = if targeted { 16.0 } else { 13.0 };
            draw_ellipse(
                sx,
                sy + 2.0 * zoom,
                glow_r * zoom,
                (glow_r * 0.4) * zoom,
                0.0,
                Color::new(0.5, 0.9, 1.0, glow_a * fade),
            );

            // Full equipped appearance, drawn with a translucent spectral tint.
            let pulse = (time * 3.0).sin() as f32 * 0.06 + 0.5;
            let ghost_tint = Color::new(0.72, 0.92, 1.0, pulse * fade);
            self.render_player(
                &ghost,
                false,
                false,
                false,
                &state.camera,
                &state.item_registry,
                0.0,
                Some(ghost_tint),
            );
        }
    }

    /// Render the Mark of Death indicator above the currently-marked player
    /// (Reaper boss). A pulsing soul-purple ring with a countdown.
    pub(super) fn render_reaper_mark(&self, state: &GameState) {
        let mark = match &state.reaper_mark {
            Some(m) => m,
            None => return,
        };
        let time = macroquad::time::get_time();
        let remaining_ms = mark.duration_ms as f64 - (time - mark.created_at) * 1000.0;
        if remaining_ms <= 0.0 {
            return;
        }
        let player = match state.players.get(&mark.player_id) {
            Some(p) => p,
            None => return,
        };

        let zoom = state.camera.zoom;
        let (sx, sy) = world_to_screen(player.x, player.y, &state.camera);
        let cy = sy - 86.0 * zoom; // float just above the head

        let pulse = (time * 6.0).sin() as f32 * 0.5 + 0.5;
        let radius = (10.0 + pulse * 2.0) * zoom;
        let ring = Color::new(0.85, 0.1, 0.9, 0.85); // soul-purple

        draw_circle(sx, cy, radius, Color::new(0.05, 0.0, 0.08, 0.8));
        draw_circle_lines(sx, cy, radius, (2.0 * zoom).max(1.0), ring);

        // Downward pointer toward the player's head
        draw_triangle(
            Vec2::new(sx - 4.0 * zoom, cy + radius),
            Vec2::new(sx + 4.0 * zoom, cy + radius),
            Vec2::new(sx, cy + radius + 5.0 * zoom),
            ring,
        );

        // Countdown seconds — native 16px bitmap size for crisp text
        let secs = (remaining_ms / 1000.0).ceil() as i32;
        let txt = secs.to_string();
        let dims = self.measure_text_sharp(&txt, 16.0);
        self.draw_text_sharp(
            &txt,
            sx - dims.width / 2.0,
            cy + dims.height / 2.0,
            16.0,
            WHITE,
        );
    }

    /// DEBUG: Render occupied tile footprints for multi-tile NPCs
    pub(super) fn render_npc_debug_tiles(&self, state: &GameState) {
        let zoom = state.camera.zoom;
        for npc in state.npcs.values() {
            if npc.size <= 1 {
                continue;
            }
            // Show the raw anchor footprint (green) — tiles (x,y) to (x+size-1, y+size-1)
            for dy in 0..npc.size {
                for dx in 0..npc.size {
                    let tx = npc.x + dx as f32;
                    let ty = npc.y + dy as f32;
                    let (sx, sy) = world_to_screen(tx, ty, &state.camera);
                    let half_w = (TILE_WIDTH / 2.0) * zoom * 0.5;
                    let half_h = (TILE_HEIGHT / 2.0) * zoom * 0.5;
                    let color = Color::new(0.0, 1.0, 0.0, 0.3);
                    draw_triangle(
                        Vec2::new(sx, sy - half_h),
                        Vec2::new(sx + half_w, sy),
                        Vec2::new(sx, sy + half_h),
                        color,
                    );
                    draw_triangle(
                        Vec2::new(sx, sy - half_h),
                        Vec2::new(sx - half_w, sy),
                        Vec2::new(sx, sy + half_h),
                        color,
                    );
                }
            }
            // Show sprite center (blue dot)
            let center_offset = (npc.size - 1) as f32 * 0.5;
            let (cx, cy) =
                world_to_screen(npc.x + center_offset, npc.y + center_offset, &state.camera);
            draw_circle(cx, cy, 4.0 * zoom, Color::new(0.0, 0.5, 1.0, 0.8));
        }
    }

    /// Render explosion effects as expanding/fading circles on tile areas
    pub(super) fn render_explosions(&self, state: &GameState) {
        let zoom = state.camera.zoom;
        let time = macroquad::time::get_time();
        let (sw, sh) = virtual_screen_size();

        for explosion in &state.explosions {
            let elapsed = (time - explosion.created_at) as f32;
            if elapsed > 1.0 {
                continue;
            }

            // Fade out over 1 second
            let alpha = (1.0 - elapsed).max(0.0) * 0.6;
            // Expand slightly
            let scale = 1.0 + elapsed * 0.3;

            let (center_x, center_y) =
                world_to_screen(explosion.x as f32, explosion.y as f32, &state.camera);

            // Viewport culling
            let radius_px = explosion.radius as f32 * TILE_WIDTH * zoom * scale;
            if center_x + radius_px < 0.0
                || center_x - radius_px > sw
                || center_y + radius_px < 0.0
                || center_y - radius_px > sh
            {
                continue;
            }

            // Draw colored overlay on each tile in the radius
            let r = explosion.radius;
            for dx in -r..=r {
                for dy in -r..=r {
                    let tx = explosion.x + dx;
                    let ty = explosion.y + dy;
                    let (sx, sy) = world_to_screen(tx as f32, ty as f32, &state.camera);

                    let half_w = (TILE_WIDTH / 2.0) * zoom * 0.5 * scale;
                    let half_h = (TILE_HEIGHT / 2.0) * zoom * 0.5 * scale;

                    // Orange-red explosion color
                    let color = Color::new(1.0, 0.4 + elapsed * 0.3, 0.1, alpha);

                    draw_triangle(
                        Vec2::new(sx, sy - half_h),
                        Vec2::new(sx + half_w, sy),
                        Vec2::new(sx, sy + half_h),
                        color,
                    );
                    draw_triangle(
                        Vec2::new(sx, sy - half_h),
                        Vec2::new(sx - half_w, sy),
                        Vec2::new(sx, sy + half_h),
                        color,
                    );
                }
            }
        }
    }

    /// Render exit portal arrows on interior map edges
    pub(super) fn render_exit_portal_arrows(&self, state: &GameState) {
        // Only render in interior mode
        let (width, height) = match state.chunk_manager.get_interior_size() {
            Some(size) => size,
            None => return,
        };

        // Get interior chunk (always at 0,0)
        let coord = crate::game::ChunkCoord::new(0, 0);
        let chunk = match state.chunk_manager.chunks().get(&coord) {
            Some(c) => c,
            None => return,
        };

        // Pulsing opacity (70-100%, 2-second cycle)
        let time = macroquad::time::get_time();
        let alpha = (0.7 + 0.3 * (time * std::f64::consts::PI).sin() as f32).clamp(0.0, 1.0);
        let color = Color::new(1.0, 1.0, 1.0, alpha);

        let zoom = state.camera.zoom;
        let arrow_w = 64.0 * zoom;
        let arrow_h = 32.0 * zoom;

        // Track min/max positions for portals on each edge
        // (min_pos, max_pos) where pos is Y for left/right edges, X for top/bottom
        let mut left_span: Option<(i32, i32)> = None;
        let mut right_span: Option<(i32, i32)> = None;
        let mut top_span: Option<(i32, i32)> = None;
        let mut bottom_span: Option<(i32, i32)> = None;

        // Group portals by edge and find spans
        // Use else-if to ensure each portal only counts for ONE edge (priority: bottom > top > right > left)
        for portal in &chunk.portals {
            if portal.y + portal.height >= height as i32 {
                // Bottom edge
                let min_x = portal.x;
                let max_x = portal.x + portal.width;
                bottom_span = Some(match bottom_span {
                    Some((cur_min, cur_max)) => (cur_min.min(min_x), cur_max.max(max_x)),
                    None => (min_x, max_x),
                });
            } else if portal.y == 0 {
                // Top edge
                let min_x = portal.x;
                let max_x = portal.x + portal.width;
                top_span = Some(match top_span {
                    Some((cur_min, cur_max)) => (cur_min.min(min_x), cur_max.max(max_x)),
                    None => (min_x, max_x),
                });
            } else if portal.x + portal.width >= width as i32 {
                // Right edge
                let min_y = portal.y;
                let max_y = portal.y + portal.height;
                right_span = Some(match right_span {
                    Some((cur_min, cur_max)) => (cur_min.min(min_y), cur_max.max(max_y)),
                    None => (min_y, max_y),
                });
            } else if portal.x == 0 {
                // Left edge
                let min_y = portal.y;
                let max_y = portal.y + portal.height;
                left_span = Some(match left_span {
                    Some((cur_min, cur_max)) => (cur_min.min(min_y), cur_max.max(max_y)),
                    None => (min_y, max_y),
                });
            }
        }

        // Draw arrow for each edge that has portals, centered on the span
        if let Some((min_y, max_y)) = left_span {
            if let Some(ref tex) = self.exit_arrow_left {
                let center_y = (min_y + max_y) as f32 / 2.0;
                let (sx, sy) = world_to_screen(-0.5, center_y, &state.camera);
                draw_texture_ex(
                    tex,
                    sx - arrow_w / 2.0,
                    sy - arrow_h / 2.0,
                    color,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(arrow_w, arrow_h)),
                        ..Default::default()
                    },
                );
            }
        }
        if let Some((min_y, max_y)) = right_span {
            if let Some(ref tex) = self.exit_arrow_right {
                let center_y = (min_y + max_y) as f32 / 2.0;
                let (sx, sy) = world_to_screen(width as f32 + 0.5, center_y, &state.camera);
                draw_texture_ex(
                    tex,
                    sx - arrow_w / 2.0,
                    sy - arrow_h / 2.0,
                    color,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(arrow_w, arrow_h)),
                        ..Default::default()
                    },
                );
            }
        }
        if let Some((min_x, max_x)) = top_span {
            if let Some(ref tex) = self.exit_arrow_up {
                let center_x = (min_x + max_x) as f32 / 2.0;
                let (sx, sy) = world_to_screen(center_x, -0.5, &state.camera);
                draw_texture_ex(
                    tex,
                    sx - arrow_w / 2.0,
                    sy - arrow_h / 2.0,
                    color,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(arrow_w, arrow_h)),
                        ..Default::default()
                    },
                );
            }
        }
        if let Some((min_x, max_x)) = bottom_span {
            if let Some(ref tex) = self.exit_arrow_down {
                let center_x = (min_x + max_x) as f32 / 2.0;
                let (sx, sy) = world_to_screen(center_x, height as f32 + 0.5, &state.camera);
                draw_texture_ex(
                    tex,
                    sx - arrow_w / 2.0,
                    sy - arrow_h / 2.0,
                    color,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(arrow_w, arrow_h)),
                        ..Default::default()
                    },
                );
            }
        }
    }

    /// Render gathering buff timer indicator (top-center HUD)
    pub(super) fn render_gathering_buff(&self, state: &GameState) {
        if !state.is_gathering {
            return;
        }
        if let Some(ref buff) = state.gathering_buff {
            let time = macroquad::time::get_time();
            let elapsed = time - buff.start_time;
            let remaining = (buff.duration - elapsed).max(0.0);
            if remaining <= 0.0 {
                return;
            }
            let progress = (remaining / buff.duration) as f32;

            let sw = screen_width();
            let bar_w = 120.0;
            let bar_h = 14.0;
            let x = (sw - bar_w) / 2.0;
            let y = 40.0;

            // Background
            draw_rectangle(
                x - 1.0,
                y - 1.0,
                bar_w + 2.0,
                bar_h + 2.0,
                Color::new(0.0, 0.0, 0.0, 0.6),
            );
            // Fill
            let fill_color = Color::new(1.0, 0.85, 0.2, 0.8);
            draw_rectangle(x, y, bar_w * progress, bar_h, fill_color);
            // Text
            let label = format!("2x Gather {:.0}s", remaining);
            let font_size = 10.0;
            let text_w = self.font.measure_text(&label, font_size).width;
            self.draw_text_sharp(
                &label,
                x + (bar_w - text_w) / 2.0,
                y + 11.0,
                font_size,
                WHITE,
            );
        }
    }

    /// Render a gold pile with multiple animated nuggets
    pub(super) fn render_gold_pile(&self, item: &GroundItem, camera: &Camera) {
        let (screen_x, screen_y) = world_to_screen(item.x, item.y, camera);
        let zoom = camera.zoom;
        let time = macroquad::time::get_time();

        let pile = match &item.gold_pile {
            Some(p) => p,
            None => return,
        };

        let texture = match &self.gold_nugget_texture {
            Some(t) => t,
            None => return,
        };

        let elapsed = time - pile.spawn_time;

        // Animation phase durations
        const ARC_DURATION: f64 = 0.3; // Phase 1: arc outward
        const BOUNCE_DURATION: f64 = 0.2; // Phase 2: bounce up
        const SETTLE_DURATION: f64 = 0.1; // Phase 3: settle down
        const TOTAL_DURATION: f64 = ARC_DURATION + BOUNCE_DURATION + SETTLE_DURATION;
        const STAGGER_DELAY: f64 = 0.03;

        // Animation heights
        const ARC_HEIGHT: f32 = 10.0; // Peak height during arc
        const BOUNCE_HEIGHT: f32 = 4.0; // Peak height during bounce

        // Bob animation (post-settle)
        const BOB_SPEED: f64 = 2.5;
        const BOB_AMPLITUDE: f32 = 1.5;

        // Shadow constants
        const SHADOW_WIDTH: f32 = 18.0;
        const SHADOW_HEIGHT: f32 = 8.0;
        const SHADOW_BASE_ALPHA: f32 = 50.0;

        // Calculate overall spawn progress for shadow fade-in
        let overall_spawn_t = (elapsed / TOTAL_DURATION).clamp(0.0, 1.0) as f32;

        // Calculate average bob for shadow pulse (only after nuggets mostly settled)
        let avg_bob = if overall_spawn_t > 0.7 {
            let bob_strength = ((overall_spawn_t - 0.7) / 0.3).min(1.0);
            let sum: f32 = pile
                .nuggets
                .iter()
                .map(|n| ((time * BOB_SPEED + n.phase_offset).sin() as f32) * BOB_AMPLITUDE * zoom)
                .sum();
            (sum / pile.nuggets.len() as f32) * bob_strength
        } else {
            0.0
        };

        // Shadow size and alpha respond to average bob
        let bob_normalized = avg_bob / (BOB_AMPLITUDE * zoom);
        let shadow_scale = 1.0 - bob_normalized * 0.15;
        let shadow_alpha =
            ((SHADOW_BASE_ALPHA - bob_normalized * 10.0) * overall_spawn_t).clamp(0.0, 255.0) as u8;

        draw_ellipse(
            screen_x,
            screen_y,
            SHADOW_WIDTH * zoom * shadow_scale,
            SHADOW_HEIGHT * zoom * shadow_scale,
            0.0,
            Color::from_rgba(0, 0, 0, shadow_alpha),
        );

        // Sort nuggets by Y offset for proper depth (back to front)
        let mut sorted_indices: Vec<usize> = (0..pile.nuggets.len()).collect();
        sorted_indices.sort_by(|&a, &b| {
            pile.nuggets[a]
                .target_y
                .partial_cmp(&pile.nuggets[b].target_y)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Render each nugget
        for (render_idx, &nugget_idx) in sorted_indices.iter().enumerate() {
            let nugget = &pile.nuggets[nugget_idx];

            // Calculate elapsed time for this nugget (with stagger)
            let nugget_elapsed = elapsed - (render_idx as f64 * STAGGER_DELAY);
            if nugget_elapsed < 0.0 {
                continue; // Nugget hasn't spawned yet
            }

            // Calculate position and height based on animation phase
            let (current_x, current_y, height_offset) = if nugget_elapsed < ARC_DURATION {
                // Phase 1: Arc outward from center to target
                let t = (nugget_elapsed / ARC_DURATION) as f32;
                let ease_t = 1.0 - (1.0 - t).powi(2); // Ease-out quadratic for position

                let x = nugget.target_x * ease_t;
                let y = nugget.target_y * ease_t;
                // Parabolic arc: height = 4 * peak * t * (1 - t)
                let arc = 4.0 * ARC_HEIGHT * t * (1.0 - t);

                (x, y, arc)
            } else if nugget_elapsed < ARC_DURATION + BOUNCE_DURATION {
                // Phase 2: Bounce up from target position
                let t = ((nugget_elapsed - ARC_DURATION) / BOUNCE_DURATION) as f32;
                // Parabolic bounce
                let bounce = 4.0 * BOUNCE_HEIGHT * t * (1.0 - t);

                (nugget.target_x, nugget.target_y, bounce)
            } else if nugget_elapsed < TOTAL_DURATION {
                // Phase 3: Settle down
                let t =
                    ((nugget_elapsed - ARC_DURATION - BOUNCE_DURATION) / SETTLE_DURATION) as f32;
                // Small settling bounce (quarter height of main bounce)
                let settle = 4.0 * (BOUNCE_HEIGHT * 0.25) * t * (1.0 - t);

                (nugget.target_x, nugget.target_y, settle)
            } else {
                // Animation complete - apply bob
                let bob = ((time * BOB_SPEED + nugget.phase_offset).sin() as f32) * BOB_AMPLITUDE;
                (nugget.target_x, nugget.target_y, bob)
            };

            // Calculate final screen position
            let nugget_x = screen_x + current_x * zoom;
            let nugget_y = screen_y + current_y * zoom - height_offset * zoom - 4.0 * zoom;

            // Draw nugget sprite
            let width = texture.width() * zoom;
            let height = texture.height() * zoom;

            draw_texture_ex(
                texture,
                nugget_x - width / 2.0,
                nugget_y - height / 2.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(width, height)),
                    ..Default::default()
                },
            );
        }
    }
}
