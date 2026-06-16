use super::*;

impl Renderer {
    pub(super) fn render_npc(
        &self,
        npc: &Npc,
        is_selected: bool,
        is_hovered: bool,
        camera: &Camera,
    ) {
        let center_offset = (npc.size - 1) as f32 * 0.5;
        let (screen_x, screen_y) =
            world_to_screen_z(npc.x + center_offset, npc.y + center_offset, npc.z, camera);
        let zoom = camera.zoom;

        // Don't render if death animation is complete
        if npc.is_death_animation_complete() {
            return;
        }

        // Get death tint color if dying, otherwise white
        let tint_color = npc.get_death_color().unwrap_or(WHITE);

        // Selection highlight (draw first, behind NPC) - skip while dying
        if is_selected && npc.death_timer.is_none() {
            if npc.size > 1 {
                self.render_multi_tile_selection(
                    npc.x,
                    npc.y,
                    npc.z,
                    npc.size,
                    npc.render_offset_y,
                    camera,
                );
            } else {
                self.render_tile_selection(npc.x, npc.y, npc.z, camera);
            }
        }

        // Name color based on NPC type
        let _name_color = if npc.is_hostile() {
            Color::from_rgba(255, 150, 150, 255) // Red for hostile
        } else if npc.is_quest_giver {
            Color::from_rgba(150, 220, 255, 255) // Cyan for quest givers
        } else if npc.is_banker {
            Color::from_rgba(255, 215, 0, 255) // Gold for bankers
        } else if npc.is_merchant {
            Color::from_rgba(150, 255, 150, 255) // Light green for merchants
        } else if npc.station_type.is_some() {
            Color::from_rgba(255, 180, 100, 255) // Warm orange for stations
        } else {
            Color::from_rgba(255, 255, 255, 255) // White for other friendly NPCs
        };

        // Try to render with sprite, fall back to ellipse
        let sprite_height = if let Some((npc_texture, npc_atlas_offset)) =
            self.npc_sprites.get(&npc.entity_type).or_else(|| {
                self.npc_overflow_sprites
                    .get(&npc.entity_type)
                    .map(|t| (t, None))
            }) {
            // Auto-detect frame size from texture
            let (tex_w, tex_h) = self
                .npc_sprites
                .get_dimensions(&npc.entity_type)
                .or_else(|| {
                    self.npc_overflow_sprites
                        .get(&npc.entity_type)
                        .map(|t| (t.width(), t.height()))
                })
                .unwrap_or((npc_texture.width(), npc_texture.height()));
            let total_frames = match npc.animation.layout {
                NpcAnimationLayout::BossWurm => 48.0,
                NpcAnimationLayout::ExplodingRock => 22.0,
                _ => 16.0,
            };
            let frame_width = tex_w / total_frames;
            let frame_height = tex_h;
            let (npc_atlas_x, npc_atlas_y) = npc_atlas_offset.unwrap_or((0.0, 0.0));

            // Get current frame based on animation state and direction
            let has_idle_anim = self.npc_idle_anim_set.contains(&npc.entity_type);
            let frame_index = npc.animation.get_frame_index(npc.direction, has_idle_anim);
            let src_x = npc_atlas_x + frame_index as f32 * frame_width;

            // Flip horizontally for Right/Left directions
            let flip_x = NpcAnimation::should_flip(npc.direction);

            // Position sprite centered horizontally, feet at world position
            // Round to whole pixels to avoid blurry rendering from subpixel positioning
            let scaled_width = (frame_width * zoom).round();
            let scaled_height = (frame_height * zoom).round();
            let draw_x = (screen_x - scaled_width / 2.0).round();
            let draw_y =
                (screen_y - scaled_height + 4.0 * zoom + npc.render_offset_y * zoom).round();

            // Draw shadow (unless disabled)
            if !npc.no_shadow {
                let shadow_scale = (frame_width / 50.0).clamp(0.5, 2.0);
                let size_scale = npc.size as f32;
                draw_ellipse(
                    screen_x,
                    screen_y,
                    16.0 * shadow_scale * zoom * size_scale,
                    6.0 * shadow_scale * zoom * size_scale,
                    0.0,
                    Color::from_rgba(0, 0, 0, 60),
                );
            }

            draw_texture_ex(
                npc_texture,
                draw_x,
                draw_y,
                tint_color,
                DrawTextureParams {
                    source: Some(Rect::new(src_x, npc_atlas_y, frame_width, frame_height)),
                    dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                    flip_x,
                    ..Default::default()
                },
            );

            scaled_height
        } else {
            // No sprite available — render nothing, just return a plausible height
            // so name tags / selection indicators still position correctly.
            40.0 * zoom
        };

        // Skip UI elements (name, health bar, icons) while dying
        if npc.death_timer.is_some() {
            return;
        }

        // Top of NPC for UI elements (account for sprite y offset)
        let top_y = screen_y - sprite_height + 4.0 * zoom + npc.render_offset_y * zoom;

        // Determine icon coords for friendly NPCs (quest givers only)
        let icon_coords: Option<(u32, u32)> =
            if !npc.is_hostile() && npc.is_quest_giver && !npc.can_turn_in_quest {
                Some((8, 3)) // Quest giver icon
            } else {
                None
            };

        // Floating icon indicator - only when NOT hovered (when hovered, icon is in name bar)
        if !is_hovered && !is_selected {
            let icon_size = 24.0;
            let time = macroquad::time::get_time();

            // Use NPC position as offset so icons don't animate in sync
            let phase_offset = (npc.x + npc.y * 1.7) as f64;

            // Pulsing transparency (2 second cycle, 80-100% opacity)
            let alpha_pulse =
                ((time * std::f64::consts::PI + phase_offset).sin() * 0.5 + 0.5) as f32;
            let mut alpha = (204.0 + alpha_pulse * 51.0) as u8; // 204-255 (80-100%)

            // Fade icon out when speech bubble appears, fade back in when it disappears
            if let Some((_, bubble_time)) = &npc.speech_bubble {
                let age = (time - bubble_time) as f32;
                let icon_alpha = if age < 0.5 {
                    // Fade out over first 0.5s as bubble appears
                    ((1.0 - age / 0.5) * 255.0) as u8
                } else if age > 4.0 && age <= 5.0 {
                    // Fade back in during last second as bubble fades out
                    ((age - 4.0) * 255.0) as u8
                } else if age > 5.0 {
                    255 // Fully visible after bubble is gone
                } else {
                    0 // Hidden while bubble is showing
                };
                alpha = alpha.min(icon_alpha);
            }

            // Center icon on world position (screen_x is from world_to_screen, already tile-centered)
            let icon_x = screen_x - (icon_size * zoom) / 2.0;
            let icon_y = top_y - 20.0 * zoom;

            if npc.is_quest_giver && npc.can_turn_in_quest {
                if let Some(ref texture) = self.ui_icons {
                    let src_rect = Rect::new(24.0, 216.0, 24.0, 24.0); // row 10, col 2 (1-based)
                    draw_texture_ex(
                        texture,
                        icon_x,
                        icon_y,
                        Color::from_rgba(255, 255, 255, alpha),
                        DrawTextureParams {
                            source: Some(src_rect),
                            dest_size: Some(Vec2::new(icon_size * zoom, icon_size * zoom)),
                            ..Default::default()
                        },
                    );
                } else {
                    let check_size = 18.0 * zoom;
                    let check_dims = self.measure_text_sharp("✓", check_size);
                    self.draw_text_sharp(
                        "✓",
                        icon_x + (icon_size * zoom - check_dims.width) / 2.0,
                        icon_y + (icon_size * zoom + check_dims.height) / 2.0 - 2.0 * zoom,
                        check_size,
                        Color::from_rgba(120, 255, 140, alpha),
                    );
                }
            } else if let (Some((icon_col, icon_row)), Some(ref texture)) =
                (icon_coords, &self.ui_icons)
            {
                let src_rect = Rect::new(
                    icon_col as f32 * icon_size,
                    icon_row as f32 * icon_size,
                    icon_size,
                    icon_size,
                );

                draw_texture_ex(
                    texture,
                    icon_x,
                    icon_y,
                    Color::from_rgba(255, 255, 255, alpha),
                    DrawTextureParams {
                        source: Some(src_rect),
                        dest_size: Some(Vec2::new(icon_size * zoom, icon_size * zoom)),
                        ..Default::default()
                    },
                );
            }
        }

        // NPC name with level - only show when hovered or selected
        let show_name = is_selected || is_hovered;
        // Name tag drawing is deferred to render_name_tags() so it appears above all map elements

        // Health bar - only show within 3 seconds of taking damage (and when not at full HP)
        let current_time = macroquad::time::get_time();
        let time_since_damage = current_time - npc.last_damage_time;
        let show_health_bar = npc.hp < npc.max_hp && time_since_damage < 3.0;

        if show_health_bar {
            let bar_width = 30.0 * zoom;
            let bar_height = 5.0 * zoom;
            let bar_x = screen_x - bar_width / 2.0;
            // Position health bar above the name box when visible
            // Name box top sits at (top_y - 19*zoom), so place bar above it with a gap
            let bar_y = if show_name {
                top_y - 19.0 * zoom - bar_height - 2.0 * zoom
            } else {
                top_y - 5.0 * zoom
            };
            let hp_ratio = npc.hp as f32 / npc.max_hp.max(1) as f32;

            self.draw_entity_health_bar(bar_x, bar_y, bar_width, bar_height, hp_ratio, zoom);
        }
    }

    pub(super) fn render_ground_item(&self, item: &GroundItem, camera: &Camera, state: &GameState) {
        // Special rendering for gold piles
        if item.item_id == "gold" && item.gold_pile.is_some() {
            self.render_gold_pile(item, camera);
            return;
        }

        // Look up terrain height at item position for Z-aware rendering
        let item_z = {
            let ix = item.x.round() as i32;
            let iy = item.y.round() as i32;
            let coord = crate::game::ChunkCoord::from_world(ix, iy);
            state
                .chunk_manager
                .chunks()
                .get(&coord)
                .map(|chunk| {
                    let (lx, ly) = crate::game::chunk::world_to_local(ix, iy);
                    chunk.get_height(lx, ly) as f32
                })
                .unwrap_or(0.0)
        };
        let (screen_x, screen_y) = world_to_screen_z(item.x, item.y, item_z, camera);
        let zoom = camera.zoom;
        let time = macroquad::time::get_time();
        let elapsed = time - item.animation_time;

        // Animation phase durations (same as gold)
        const ARC_DURATION: f64 = 0.3;
        const BOUNCE_DURATION: f64 = 0.2;
        const SETTLE_DURATION: f64 = 0.1;
        const TOTAL_DURATION: f64 = ARC_DURATION + BOUNCE_DURATION + SETTLE_DURATION;

        // Animation heights
        const ARC_HEIGHT: f32 = 10.0;
        const BOUNCE_HEIGHT: f32 = 4.0;

        // Bob animation (post-settle)
        const BOB_SPEED: f64 = 3.0;
        const BOB_AMPLITUDE: f32 = 2.0;

        // Calculate height offset based on animation phase
        let (height_offset, spawn_progress) = if elapsed < ARC_DURATION {
            // Phase 1: Arc up and down
            let t = (elapsed / ARC_DURATION) as f32;
            let arc = 4.0 * ARC_HEIGHT * t * (1.0 - t);
            (arc, t)
        } else if elapsed < ARC_DURATION + BOUNCE_DURATION {
            // Phase 2: Bounce up
            let t = ((elapsed - ARC_DURATION) / BOUNCE_DURATION) as f32;
            let bounce = 4.0 * BOUNCE_HEIGHT * t * (1.0 - t);
            (bounce, 1.0)
        } else if elapsed < TOTAL_DURATION {
            // Phase 3: Settle
            let t = ((elapsed - ARC_DURATION - BOUNCE_DURATION) / SETTLE_DURATION) as f32;
            let settle = 4.0 * (BOUNCE_HEIGHT * 0.25) * t * (1.0 - t);
            (settle, 1.0)
        } else {
            // Animation complete - gentle bob
            let bob = ((elapsed * BOB_SPEED).sin() as f32) * BOB_AMPLITUDE;
            (bob, 1.0)
        };

        // Shadow rendering - size and alpha respond to height
        const SHADOW_WIDTH: f32 = 14.0;
        const SHADOW_HEIGHT: f32 = 6.0;
        const SHADOW_BASE_ALPHA: f32 = 50.0;

        let height_normalized = height_offset / ARC_HEIGHT; // Normalize to arc height
        let shadow_scale = 1.0 - height_normalized * 0.2;
        let shadow_alpha = ((SHADOW_BASE_ALPHA - height_normalized * 15.0) * spawn_progress)
            .clamp(0.0, 255.0) as u8;

        draw_ellipse(
            screen_x,
            screen_y,
            SHADOW_WIDTH * zoom * shadow_scale,
            SHADOW_HEIGHT * zoom * shadow_scale,
            0.0,
            Color::from_rgba(0, 0, 0, shadow_alpha),
        );

        let item_def = state.item_registry.get_or_placeholder(&item.item_id);
        let item_y = screen_y - 8.0 * zoom - height_offset * zoom;

        // Try to use item sprite, fall back to colored rectangle
        if let Some((texture, source_rect)) = self.item_sprites.get(&item_def.sprite) {
            // Use texture (or atlas region), centered on the ground position
            let (sprite_w, sprite_h) = if let Some(r) = source_rect {
                (r.w, r.h)
            } else {
                (texture.width(), texture.height())
            };
            let icon_width = sprite_w * zoom;
            let icon_height = sprite_h * zoom;

            draw_texture_ex(
                texture,
                screen_x - icon_width / 2.0,
                item_y - icon_height / 2.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(icon_width, icon_height)),
                    source: source_rect,
                    ..Default::default()
                },
            );
        } else {
            // Fallback to colored rectangle
            let color = item_def.category_color();
            draw_rectangle(
                screen_x - 6.0 * zoom,
                item_y - 6.0 * zoom,
                16.0 * zoom,
                12.0 * zoom,
                color,
            );
            draw_rectangle_lines(
                screen_x - 6.0 * zoom,
                item_y - 6.0 * zoom,
                16.0 * zoom,
                12.0 * zoom,
                1.0,
                WHITE,
            );
        }
    }

    /// Render a fishing line from the player's rod tip to a landing point in the water
    pub(super) fn render_fishing_line(&self, player: &Player, camera: &Camera) {
        use crate::game::Direction;
        use crate::render::animation::{get_weapon_frame, get_weapon_offset, Gender};

        let (screen_x, screen_y) = world_to_screen_z(player.x, player.y, player.z, camera);
        let zoom = camera.zoom;
        let time = macroquad::time::get_time();

        // Compute weapon draw position (same as render_player)
        let draw_x = screen_x - SPRITE_WIDTH * zoom / 2.0;
        let draw_y = screen_y - SPRITE_HEIGHT * zoom + 16.0 * zoom;

        // Get player gender for gender-specific offsets
        let player_gender = Gender::from_wire(&player.gender);

        let anim_frame = player.animation.frame as u32;
        let (offset_x, offset_y) = get_weapon_offset(
            player.animation.state,
            player.animation.direction,
            anim_frame,
            player_gender,
        );
        let weapon_frame = get_weapon_frame(
            player.animation.state,
            player.animation.direction,
            anim_frame,
        );
        let flip = weapon_frame.flip_h;

        // Fishing rod frame size (from manifest: 70x86)
        let fw: f32 = 70.0;
        let _fh: f32 = 86.0;

        let weapon_draw_x = draw_x + offset_x * zoom;
        let weapon_draw_y = draw_y + offset_y * zoom;

        // Rod tip position within the weapon frame (in unscaled pixels)
        // These are the approximate pixel positions of the rod tip in each frame
        let (tip_px, tip_py) = match player.animation.direction {
            Direction::Down | Direction::DownLeft => (14.0, 61.0),
            Direction::Left | Direction::UpLeft => (16.0, 38.0),
            Direction::Up | Direction::UpRight => (16.0, 38.0),
            Direction::Right | Direction::DownRight => (10.0, 61.0),
        };

        // Account for horizontal flip
        let tip_in_frame_x = if flip { fw - tip_px } else { tip_px };

        let rod_x = weapon_draw_x + tip_in_frame_x * zoom;
        let rod_y = weapon_draw_y + tip_py * zoom;

        // Landing point: center of a tile 2-3 tiles ahead in the facing direction
        // Use player position as seed for stable per-session random distance
        let seed = player.x * 73.137 + player.y * 37.891;
        let cast_dist = 2.0 + (seed.sin() * 0.5 + 0.5); // range [2.0, 3.0]
        let (tile_dx, tile_dy): (f32, f32) = match player.animation.direction {
            Direction::Down => (0.0, cast_dist),
            Direction::Up => (0.0, -cast_dist),
            Direction::Left => (-cast_dist, 0.0),
            Direction::Right => (cast_dist, 0.0),
            Direction::DownLeft => (-cast_dist * 0.707, cast_dist * 0.707),
            Direction::DownRight => (cast_dist * 0.707, cast_dist * 0.707),
            Direction::UpLeft => (-cast_dist * 0.707, -cast_dist * 0.707),
            Direction::UpRight => (cast_dist * 0.707, -cast_dist * 0.707),
        };

        let (land_base_x, land_base_y) =
            world_to_screen(player.x + tile_dx, player.y + tile_dy, camera);

        // Gentle sway at the landing point
        let sway_x = (time * 0.8).sin() as f32 * 2.0 * zoom;
        let sway_y = (time * 0.6).cos() as f32 * 1.0 * zoom;
        let land_x = land_base_x + sway_x;
        let land_y = land_base_y + sway_y;

        // Draw line as a catenary curve using segments
        let segments = 8;
        let line_color = Color::new(1.0, 1.0, 1.0, 0.85);
        let line_thickness = (1.0 * zoom).max(0.5);

        for i in 0..segments {
            let t0 = i as f32 / segments as f32;
            let t1 = (i + 1) as f32 / segments as f32;

            let x0 = rod_x + (land_x - rod_x) * t0;
            let x1 = rod_x + (land_x - rod_x) * t1;
            let y0_base = rod_y + (land_y - rod_y) * t0;
            let y1_base = rod_y + (land_y - rod_y) * t1;

            // Parabolic droop, max at midpoint
            let droop_amount = 10.0 * zoom;
            let sag0 = droop_amount * 4.0 * t0 * (1.0 - t0);
            let sag1 = droop_amount * 4.0 * t1 * (1.0 - t1);

            // Slight wind ripple increasing toward the end
            let wind = (time * 2.5 + t0 as f64 * 3.0).sin() as f32 * 1.5 * zoom * t0;

            draw_line(
                x0 + wind * 0.5,
                y0_base + sag0,
                x1 + wind * 0.5,
                y1_base + sag1,
                line_thickness,
                line_color,
            );
        }

        // Small bobber at the landing point
        let bobber_bob = (time * 1.5).sin() as f32 * 1.5 * zoom;
        draw_circle(
            land_x,
            land_y + bobber_bob,
            2.0 * zoom,
            Color::new(0.8, 0.2, 0.1, 0.8),
        );
        draw_circle(
            land_x,
            land_y + bobber_bob,
            1.2 * zoom,
            Color::new(1.0, 0.4, 0.2, 0.9),
        );
    }

    /// Check if a farming sprite is the new large format (62x48, 4 frames, no sign)
    /// by looking at sheet height. Legacy sprites are 32px tall, new ones are taller.
    pub(super) fn is_large_farming_sprite(&self, sprite_name: &str) -> bool {
        if let Some((_, h)) = self.farming_sprites.get_dimensions(sprite_name) {
            h > 32.0
        } else {
            false
        }
    }

    /// Render a single farming patch (called from the depth-sorted render loop)
    pub(super) fn render_single_farming_patch(&self, state: &GameState, patch_id: &str) {
        let patch = match state.farming_patches.get(patch_id) {
            Some(p) => p,
            None => return,
        };
        let zoom = state.camera.zoom;
        let time = macroquad::time::get_time();
        let w = patch.width.max(1) as i32;
        let h = patch.height.max(1) as i32;

        if patch.patch_type == "tree" {
            // One sprite, centered on the footprint.
            let cx = patch.x as f32 + (w - 1) as f32 * 0.5;
            let cy = patch.y as f32 + (h - 1) as f32 * 0.5;
            let (sx, sy) = world_to_screen(cx, cy, &state.camera);
            self.draw_patch_at(patch, sx, sy, zoom, time);
        } else {
            // Draw the crop on every footprint tile (filled-bed look).
            for dy in 0..h {
                for dx in 0..w {
                    let (sx, sy) = world_to_screen(
                        (patch.x + dx) as f32,
                        (patch.y + dy) as f32,
                        &state.camera,
                    );
                    self.draw_patch_at(patch, sx, sy, zoom, time);
                }
            }
        }
    }

    /// Draw one patch tile's crop/tree sprite + overlays at the given screen position.
    fn draw_patch_at(
        &self,
        patch: &crate::game::state::FarmingPatch,
        screen_x: f32,
        screen_y: f32,
        zoom: f32,
        time: f64,
    ) {
        // Legacy sprite dimensions
        let legacy_frame_w = 16.0;
        let legacy_frame_h = 32.0;

        // Trees render with tall object sprites (gid-based), not the farming crop atlas.
        let is_tree = patch.patch_type == "tree";
        let sprite_name = if is_tree {
            None
        } else {
            match patch.state.as_str() {
                "growing" | "harvestable" | "diseased" | "dead" => {
                    Some(Self::crop_to_sprite_name(&patch.crop_id))
                }
                _ => None,
            }
        };

        // Diseased crops get a sickly, washed-out tint; dead crops a dark withered
        // brown so the existing sprite reads as a wilted, lifeless plant (no new art).
        let crop_tint = match patch.state.as_str() {
            "diseased" => Color::new(0.55, 0.5, 0.25, 1.0),
            "dead" => Color::new(0.42, 0.3, 0.18, 1.0),
            _ => WHITE,
        };

        // Draw sign behind crop (legacy sprites only)
        if let Some(name) = sprite_name {
            let is_large = self
                .farming_sprites
                .get_dimensions(name)
                .is_some_and(|(_, h)| h > 32.0);
            if !is_large {
                if let Some((farm_texture, farm_atlas_offset)) = self.farming_sprites.get(name) {
                    let sign_frame = 5u32;
                    let (farm_atlas_x, farm_atlas_y) = farm_atlas_offset.unwrap_or((0.0, 0.0));
                    let src_x = farm_atlas_x + sign_frame as f32 * legacy_frame_w;
                    let sign_w = legacy_frame_w * zoom;
                    let sign_h = legacy_frame_h * zoom;
                    let sign_x = screen_x - sign_w / 2.0;
                    let sign_y = screen_y - sign_h - 4.0 * zoom;
                    draw_texture_ex(
                        farm_texture,
                        sign_x,
                        sign_y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                src_x,
                                farm_atlas_y,
                                legacy_frame_w,
                                legacy_frame_h,
                            )),
                            dest_size: Some(Vec2::new(sign_w, sign_h)),
                            ..Default::default()
                        },
                    );
                }
            }
        }

        // Draw crop sprite (or tall tree object sprite for tree patches)
        let drew_sprite = if is_tree {
            self.draw_tree_object_sprite(
                &patch.state,
                patch.growth_stage,
                screen_x,
                screen_y,
                zoom,
                crop_tint,
            )
        } else if let Some(name) = sprite_name {
            if let Some((crop_texture, crop_atlas_offset)) = self.farming_sprites.get(name) {
                let (crop_atlas_x, crop_atlas_y) = crop_atlas_offset.unwrap_or((0.0, 0.0));
                // Check if large format using dimensions (avoids extra lookup)
                let dims = self.farming_sprites.get_dimensions(name);
                let is_large = dims.is_some_and(|(_, h)| h > 32.0);

                if is_large {
                    // New large format: 4 frames, derive frame size from sheet dimensions
                    let (sheet_w, sheet_h) = dims.unwrap();
                    let num_frames = 4u32;
                    let fw = sheet_w / num_frames as f32;
                    let fh = sheet_h;

                    let frame_index = match patch.state.as_str() {
                        "growing" | "diseased" => patch.growth_stage.min(num_frames - 1),
                        // Dead crops show the fully-grown frame, tinted to look withered.
                        "harvestable" | "dead" => num_frames - 1,
                        _ => 0,
                    };

                    let src_x = crop_atlas_x + frame_index as f32 * fw;
                    let draw_w = fw * zoom;
                    let draw_h = fh * zoom;

                    draw_texture_ex(
                        crop_texture,
                        screen_x - draw_w / 2.0,
                        screen_y - draw_h + 8.0 * zoom,
                        crop_tint,
                        DrawTextureParams {
                            source: Some(Rect::new(src_x, crop_atlas_y, fw, fh)),
                            dest_size: Some(Vec2::new(draw_w, draw_h)),
                            ..Default::default()
                        },
                    );
                } else {
                    // Legacy format: 16x32 frames, frame mapping with gaps
                    let frame_index = match patch.state.as_str() {
                        "growing" | "diseased" => match patch.growth_stage {
                            0 => 0,
                            1 => 2,
                            2 => 3,
                            3 => 4,
                            _ => 4,
                        },
                        // Dead crops show the fully-grown frame, tinted to look withered.
                        "harvestable" | "dead" => 4,
                        _ => 0,
                    };

                    let src_x = crop_atlas_x + frame_index as f32 * legacy_frame_w;
                    let draw_w = legacy_frame_w * zoom;
                    let draw_h = legacy_frame_h * zoom;

                    draw_texture_ex(
                        crop_texture,
                        screen_x - draw_w / 2.0,
                        screen_y - draw_h + draw_h * 0.25,
                        crop_tint,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                src_x,
                                crop_atlas_y,
                                legacy_frame_w,
                                legacy_frame_h,
                            )),
                            dest_size: Some(Vec2::new(draw_w, draw_h)),
                            ..Default::default()
                        },
                    );
                }
                true
            } else {
                false
            }
        } else {
            false
        };

        // Fallback: draw colored diamond for empty patches or missing sprites
        if !drew_sprite {
            let half_w = 16.0 * zoom;
            let half_h = 8.0 * zoom;
            // Dead crops render as a dark, withered patch of soil.
            let (base_color, border_color) = if patch.state == "dead" {
                (
                    Color::new(0.15, 0.12, 0.1, 0.7),
                    Color::new(0.28, 0.18, 0.12, 0.85),
                )
            } else {
                (
                    Color::new(0.35, 0.25, 0.15, 0.5),
                    Color::new(0.45, 0.35, 0.2, 0.6),
                )
            };

            draw_triangle(
                Vec2::new(screen_x, screen_y - half_h),
                Vec2::new(screen_x - half_w, screen_y),
                Vec2::new(screen_x, screen_y + half_h),
                base_color,
            );
            draw_triangle(
                Vec2::new(screen_x, screen_y - half_h),
                Vec2::new(screen_x + half_w, screen_y),
                Vec2::new(screen_x, screen_y + half_h),
                base_color,
            );
            draw_line(
                screen_x,
                screen_y - half_h,
                screen_x - half_w,
                screen_y,
                1.0,
                border_color,
            );
            draw_line(
                screen_x - half_w,
                screen_y,
                screen_x,
                screen_y + half_h,
                1.0,
                border_color,
            );
            draw_line(
                screen_x,
                screen_y + half_h,
                screen_x + half_w,
                screen_y,
                1.0,
                border_color,
            );
            draw_line(
                screen_x + half_w,
                screen_y,
                screen_x,
                screen_y - half_h,
                1.0,
                border_color,
            );
        }

        // Draw soft pulsing green overlay on tile for harvestable crops
        if patch.state == "harvestable" {
            let half_w = 16.0 * zoom;
            let half_h = 8.0 * zoom;
            let pulse_alpha = ((time * 1.2).sin() as f32 * 0.05 + 0.13).clamp(0.08, 0.18);
            let glow = Color::new(0.2, 0.7, 0.3, pulse_alpha);
            draw_triangle(
                Vec2::new(screen_x, screen_y - half_h),
                Vec2::new(screen_x - half_w, screen_y),
                Vec2::new(screen_x, screen_y + half_h),
                glow,
            );
            draw_triangle(
                Vec2::new(screen_x, screen_y - half_h),
                Vec2::new(screen_x + half_w, screen_y),
                Vec2::new(screen_x, screen_y + half_h),
                glow,
            );
        }

        // Draw a reddish-brown warning pulse on diseased crops
        if patch.state == "diseased" {
            let half_w = 16.0 * zoom;
            let half_h = 8.0 * zoom;
            let pulse_alpha = ((time * 2.0).sin() as f32 * 0.06 + 0.15).clamp(0.08, 0.22);
            let glow = Color::new(0.65, 0.2, 0.1, pulse_alpha);
            draw_triangle(
                Vec2::new(screen_x, screen_y - half_h),
                Vec2::new(screen_x - half_w, screen_y),
                Vec2::new(screen_x, screen_y + half_h),
                glow,
            );
            draw_triangle(
                Vec2::new(screen_x, screen_y - half_h),
                Vec2::new(screen_x + half_w, screen_y),
                Vec2::new(screen_x, screen_y + half_h),
                glow,
            );
        }
    }

    /// Object-tileset sprite id for a tree patch's current growth stage.
    /// Stage 1->285, stage 2->286, stage 3->288, stage 4 (complete)->287.
    /// None for empty patches (which draw the soil-diamond fallback). Dead trees show
    /// the full tree (287), tinted withered by the caller.
    fn tree_sprite_id(patch_state: &str, growth_stage: u32) -> Option<u32> {
        if !matches!(patch_state, "growing" | "harvestable" | "diseased" | "dead") {
            return None;
        }
        Some(if matches!(patch_state, "harvestable" | "dead") {
            287
        } else {
            match growth_stage {
                0 => 285,
                1 => 286,
                2 => 288,
                _ => 287,
            }
        })
    }

    /// Source height (in texture pixels) of the current tree-stage sprite, if any.
    fn tree_sprite_height(&self, patch_state: &str, growth_stage: u32) -> Option<f32> {
        let gid = super::OBJECTS_FIRSTGID + Self::tree_sprite_id(patch_state, growth_stage)?;
        let (texture, source_rect) = self.get_object_sprite(gid)?;
        Some(source_rect.map(|r| r.h).unwrap_or(texture.height()))
    }

    /// Draw a tree patch using tall object-tileset sprites keyed by growth stage.
    /// Returns false (so the caller draws the soil-diamond fallback) for empty patches
    /// or when the object sprite can't be resolved. Dead trees draw the full tree with
    /// the caller's withered tint.
    fn draw_tree_object_sprite(
        &self,
        patch_state: &str,
        growth_stage: u32,
        screen_x: f32,
        screen_y: f32,
        zoom: f32,
        tint: Color,
    ) -> bool {
        let Some(sprite_id) = Self::tree_sprite_id(patch_state, growth_stage) else {
            return false;
        };
        let gid = super::OBJECTS_FIRSTGID + sprite_id;
        let Some((texture, source_rect)) = self.get_object_sprite(gid) else {
            return false;
        };
        let (w, h) = source_rect
            .map(|r| (r.w, r.h))
            .unwrap_or((texture.width(), texture.height()));
        let draw_w = (w * zoom).round();
        let draw_h = (h * zoom).round();
        // Bottom-center the tree on the patch tile.
        let draw_x = (screen_x - draw_w / 2.0).round();
        let draw_y = (screen_y - draw_h).round();
        draw_texture_ex(
            texture,
            draw_x,
            draw_y,
            tint,
            DrawTextureParams {
                dest_size: Some(Vec2::new(draw_w, draw_h)),
                source: source_rect,
                ..Default::default()
            },
        );
        true
    }

    /// Map crop_id from farming config to sprite sheet name
    pub(super) fn crop_to_sprite_name(crop_id: &str) -> &str {
        match crop_id {
            // Crop id has trailing 's' but sprite file does not
            "tangleroots" => "tangleroot",
            _ => crop_id,
        }
    }

    pub(super) fn render_farming_patch_labels(&self, state: &GameState) {
        if state.current_interior.is_some() {
            return;
        }
        let hovered_tile = match state.hovered_tile {
            Some(t) => t,
            None => return,
        };

        // Check if hovered tile is a farming patch
        let patch_id = match state.farming_patch_positions.get(&hovered_tile) {
            Some(id) => id,
            None => return,
        };
        let patch = match state.farming_patches.get(patch_id) {
            Some(p) => p,
            None => return,
        };

        let (screen_x, screen_y) = world_to_screen(patch.x as f32, patch.y as f32, &state.camera);
        let zoom = state.camera.zoom;

        // Title-case a snake_case id for display, e.g. "tangleroots" -> "Tangleroots"
        let prettify = |id: &str| {
            id.replace('_', " ")
                .split_whitespace()
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        Some(f) => f.to_uppercase().to_string() + c.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        };

        // Friendly name for an empty patch based on its type.
        let patch_label = match patch.patch_type.as_str() {
            "allotment" => "Allotment".to_string(),
            "herb" => "Herb patch".to_string(),
            "cactus" => "Cactus patch".to_string(),
            "tree" => "Tree patch".to_string(),
            other if !other.is_empty() => format!("{} patch", prettify(other)),
            _ => "Allotment".to_string(),
        };

        // Build label text
        let (label, color) = match patch.state.as_str() {
            "empty" => {
                let suffix = if patch.composted { " (Composted)" } else { " (Empty)" };
                (
                    format!("{}{}", patch_label, suffix),
                    Color::new(0.7, 0.6, 0.4, 1.0),
                )
            }
            "growing" => {
                let crop_name = prettify(&patch.crop_id);
                (
                    format!("{} (Stage {}/4)", crop_name, patch.growth_stage),
                    Color::new(0.4, 0.8, 0.3, 1.0),
                )
            }
            "harvestable" => {
                let crop_name = prettify(&patch.crop_id);
                let lives = if patch.lives_remaining > 1 {
                    format!(" x{}", patch.lives_remaining)
                } else {
                    String::new()
                };
                (
                    format!("{} (Ready!{})", crop_name, lives),
                    Color::new(1.0, 0.9, 0.3, 1.0),
                )
            }
            "diseased" => {
                let crop_name = prettify(&patch.crop_id);
                (
                    format!("{} (Diseased!)", crop_name),
                    Color::new(0.9, 0.4, 0.3, 1.0),
                )
            }
            "dead" => {
                let crop_name = prettify(&patch.crop_id);
                (
                    format!("{} (Dead)", crop_name),
                    Color::new(0.6, 0.6, 0.6, 1.0),
                )
            }
            _ => (patch_label.clone(), Color::new(0.7, 0.7, 0.7, 1.0)),
        };

        // Scale text with zoom for readability
        let font_size = 16.0 * zoom;
        let label_width = self.measure_text_sharp(&label, font_size).width;
        let label_x = screen_x - label_width / 2.0;
        // Position label above the sprite - tall sprites need more offset
        let sprite_name = Self::crop_to_sprite_name(&patch.crop_id);
        let label_offset = if patch.patch_type == "tree" {
            // Trees use tall object sprites; clear the canopy.
            match self.tree_sprite_height(&patch.state, patch.growth_stage) {
                Some(h) => (h + 4.0) * zoom,
                None => 16.0 * zoom,
            }
        } else if self.is_large_farming_sprite(sprite_name) {
            // Large sprite: offset by sprite height minus ground anchor
            let (_, sh) = self
                .farming_sprites
                .get_dimensions(sprite_name)
                .unwrap_or((0.0, 48.0));
            (sh - 8.0) * zoom
        } else {
            16.0 * zoom
        };
        let label_y = screen_y - label_offset;

        // Background
        let padding = 4.0 * zoom;
        let bar_height = 18.0 * zoom;
        draw_rectangle(
            label_x - padding,
            label_y - 14.0 * zoom,
            label_width + padding * 2.0,
            bar_height,
            Color::from_rgba(0, 0, 0, 180),
        );

        // Text
        self.draw_text_sharp(&label, label_x, label_y, font_size, color);
    }
}
