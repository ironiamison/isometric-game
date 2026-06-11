use super::*;

impl Renderer {
    pub(super) fn render_tilemap_layer(&self, state: &GameState, layer_type: LayerType) {
        // Don't render anything until world is ready (player exists and their chunk is loaded)
        // This prevents showing the fallback test tilemap or empty world during login
        if !state.is_world_ready() {
            return;
        }

        // Convert LayerType to ChunkLayerType for chunk rendering
        let chunk_layer_type = match layer_type {
            LayerType::Ground => ChunkLayerType::Ground,
            LayerType::Objects => ChunkLayerType::Objects,
            LayerType::Overhead => ChunkLayerType::Overhead,
        };

        // Try to render from chunks if any are loaded
        let chunks = state.chunk_manager.chunks();
        if !chunks.is_empty() {
            // Screen bounds for culling (use virtual size for mobile scaling)
            let (screen_w, screen_h) = virtual_screen_size();
            let margin = TILE_WIDTH * 4.0; // Extra margin for chunk edges

            // Check if we're in interior mode
            let interior_size = state.chunk_manager.get_interior_size();

            // Render from chunk manager
            for (coord, chunk) in chunks.iter() {
                // For interiors, the chunk is at (0,0) and uses interior dimensions
                // For overworld, use standard CHUNK_SIZE
                let (tile_width, tile_height) = interior_size.unwrap_or((CHUNK_SIZE, CHUNK_SIZE));

                let chunk_offset_x = coord.x * CHUNK_SIZE as i32;
                let chunk_offset_y = coord.y * CHUNK_SIZE as i32;

                // CHUNK-LEVEL CULLING: Check if chunk is visible before iterating tiles
                // In isometric projection, a chunk forms a diamond. Check all 4 corners.
                let corners = [
                    (chunk_offset_x as f32, chunk_offset_y as f32), // top
                    (
                        chunk_offset_x as f32 + tile_width as f32,
                        chunk_offset_y as f32,
                    ), // right
                    (
                        chunk_offset_x as f32,
                        chunk_offset_y as f32 + tile_height as f32,
                    ), // left
                    (
                        chunk_offset_x as f32 + tile_width as f32,
                        chunk_offset_y as f32 + tile_height as f32,
                    ), // bottom
                ];

                // Get screen bounds of the chunk
                let mut min_sx = f32::MAX;
                let mut max_sx = f32::MIN;
                let mut min_sy = f32::MAX;
                let mut max_sy = f32::MIN;

                for (wx, wy) in corners {
                    let (sx, sy) = world_to_screen(wx, wy, &state.camera);
                    min_sx = min_sx.min(sx);
                    max_sx = max_sx.max(sx);
                    min_sy = min_sy.min(sy);
                    max_sy = max_sy.max(sy);
                }

                // Skip entire chunk if completely off-screen
                if max_sx < -margin
                    || min_sx > screen_w + margin
                    || max_sy < -margin
                    || min_sy > screen_h + margin
                {
                    continue;
                }

                // Find the layer
                for layer in &chunk.layers {
                    if layer.layer_type != chunk_layer_type {
                        continue;
                    }

                    // Compute base screen position for chunk origin, then step incrementally
                    // In isometric: +1 world_x = (+dx, +dy), +1 world_y = (-dx, +dy)
                    // Use _exact (no rounding) for the base to avoid double-rounding jitter
                    let zoom = state.camera.zoom;
                    let dx = (TILE_WIDTH / 2.0) * zoom;
                    let dy = (TILE_HEIGHT / 2.0) * zoom;
                    let (base_sx, base_sy) = world_to_screen_exact(
                        chunk_offset_x as f32,
                        chunk_offset_y as f32,
                        &state.camera,
                    );
                    let water_effects = !state.ui_state.graphics_low;

                    // Tile-level culling bounds
                    let tile_margin = TILE_WIDTH * 2.0;
                    let cull_left = -tile_margin;
                    let cull_right = screen_w + tile_margin;
                    let cull_top = -tile_margin;
                    let cull_bottom = screen_h + tile_margin;

                    // For large interiors, limit tile iteration to visible world-space bounds
                    let (min_local_x, max_local_x, min_local_y, max_local_y) =
                        if interior_size.is_some() {
                            let corners = [
                                screen_to_world(cull_left, cull_top, &state.camera),
                                screen_to_world(cull_right, cull_top, &state.camera),
                                screen_to_world(cull_left, cull_bottom, &state.camera),
                                screen_to_world(cull_right, cull_bottom, &state.camera),
                            ];
                            let mut min_world_x = f32::MAX;
                            let mut max_world_x = f32::MIN;
                            let mut min_world_y = f32::MAX;
                            let mut max_world_y = f32::MIN;
                            for (wx, wy) in corners {
                                min_world_x = min_world_x.min(wx);
                                max_world_x = max_world_x.max(wx);
                                min_world_y = min_world_y.min(wy);
                                max_world_y = max_world_y.max(wy);
                            }

                            // Extra margin (in tiles) to avoid edge pop-in
                            let world_margin = 2.0;
                            let min_world_x = (min_world_x - world_margin).floor() as i32;
                            let max_world_x = (max_world_x + world_margin).ceil() as i32;
                            let min_world_y = (min_world_y - world_margin).floor() as i32;
                            let max_world_y = (max_world_y + world_margin).ceil() as i32;

                            let tile_width_i = tile_width as i32;
                            let tile_height_i = tile_height as i32;

                            let min_local_x = (min_world_x - chunk_offset_x)
                                .clamp(0, tile_width_i.saturating_sub(1));
                            let max_local_x = (max_world_x - chunk_offset_x)
                                .clamp(0, tile_width_i.saturating_sub(1));
                            let min_local_y = (min_world_y - chunk_offset_y)
                                .clamp(0, tile_height_i.saturating_sub(1));
                            let max_local_y = (max_world_y - chunk_offset_y)
                                .clamp(0, tile_height_i.saturating_sub(1));

                            (min_local_x, max_local_x, min_local_y, max_local_y)
                        } else {
                            (
                                0,
                                tile_width.saturating_sub(1) as i32,
                                0,
                                tile_height.saturating_sub(1) as i32,
                            )
                        };

                    if max_local_x < min_local_x || max_local_y < min_local_y {
                        continue;
                    }

                    // Render tiles in isometric order
                    for local_y in min_local_y..=max_local_y {
                        let row_sx = base_sx - local_y as f32 * dx;
                        let row_sy = base_sy + local_y as f32 * dy;

                        for local_x in min_local_x..=max_local_x {
                            let idx = ((local_y as u32) * tile_width + (local_x as u32)) as usize;
                            let tile_id = layer.tiles.get(idx).copied().unwrap_or(0);

                            if tile_id == 0 {
                                continue;
                            }

                            // Skip elevated ground tiles - they render in the depth-sorted entity pass
                            // so they can properly occlude entities behind them
                            if chunk_layer_type == ChunkLayerType::Ground {
                                let tile_height_z =
                                    chunk.get_height(local_x as u32, local_y as u32);
                                if tile_height_z > 0 {
                                    continue;
                                }
                            }

                            let screen_x = (row_sx + local_x as f32 * dx).round();
                            let screen_y = (row_sy + local_x as f32 * dy).round();

                            // Tile-level culling (still needed for partially visible chunks)
                            if screen_x < cull_left
                                || screen_x > cull_right
                                || screen_y < cull_top
                                || screen_y > cull_bottom
                            {
                                continue;
                            }

                            let world_x = chunk_offset_x + local_x as i32;
                            let world_y = chunk_offset_y + local_y as i32;

                            // Apply ground tile overrides (e.g. farming plot tiles)
                            let tile_id = if chunk_layer_type == ChunkLayerType::Ground {
                                state
                                    .ground_tile_overrides
                                    .get(&(world_x, world_y))
                                    .copied()
                                    .unwrap_or(tile_id)
                            } else {
                                tile_id
                            };

                            // Ambient occlusion: darken tiles when a neighbor behind them
                            // (north/west in world = -X/-Y) is taller, simulating shadow
                            let ao_tint = if chunk_layer_type == ChunkLayerType::Ground
                                && chunk.heights.is_some()
                            {
                                let h = chunk.get_height(local_x as u32, local_y as u32);
                                let h_nx = if local_x > 0 {
                                    chunk.get_height((local_x - 1) as u32, local_y as u32)
                                } else {
                                    0
                                };
                                let h_ny = if local_y > 0 {
                                    chunk.get_height(local_x as u32, (local_y - 1) as u32)
                                } else {
                                    0
                                };
                                let max_diff =
                                    (h_nx as i32 - h as i32).max(h_ny as i32 - h as i32).max(0);
                                if max_diff > 0 {
                                    let brightness = (1.0 - max_diff as f32 * 0.15).max(0.5);
                                    Some(Color::new(brightness, brightness, brightness, 1.0))
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            // Draw tile sprite with optional AO tint baked in
                            if let Some(tint) = ao_tint {
                                self.draw_tile_sprite_tinted(
                                    screen_x,
                                    screen_y,
                                    tile_id,
                                    zoom,
                                    Some((world_x as f32, world_y as f32)),
                                    water_effects,
                                    tint,
                                );
                            } else {
                                self.draw_tile_sprite(
                                    screen_x,
                                    screen_y,
                                    tile_id,
                                    zoom,
                                    Some((world_x as f32, world_y as f32)),
                                    water_effects,
                                );
                            }

                            // Draw collision indicator in debug mode
                            if state.debug_mode
                                && chunk.collision.get(idx).copied().unwrap_or(false)
                            {
                                self.draw_collision_indicator(screen_x, screen_y, zoom);
                            }
                        }
                    }
                }
            }
            return;
        }

        // Fallback: render from old tilemap if no chunks loaded
        let tilemap = &state.tilemap;

        for layer in &tilemap.layers {
            if layer.layer_type != layer_type {
                continue;
            }

            // Render in isometric order (back to front)
            let (vw, vh) = virtual_screen_size();
            let zoom = state.camera.zoom;
            let dx = (TILE_WIDTH / 2.0) * zoom;
            let dy = (TILE_HEIGHT / 2.0) * zoom;
            let (base_sx, base_sy) = world_to_screen_exact(0.0, 0.0, &state.camera);
            let water_effects = !state.ui_state.graphics_low;
            let margin = TILE_WIDTH * 2.0;

            for y in 0..tilemap.height {
                let row_sx = base_sx - y as f32 * dx;
                let row_sy = base_sy + y as f32 * dy;

                for x in 0..tilemap.width {
                    let idx = (y * tilemap.width + x) as usize;
                    let tile_id = layer.tiles.get(idx).copied().unwrap_or(0);

                    if tile_id == 0 {
                        continue; // Skip empty tiles
                    }

                    let screen_x = (row_sx + x as f32 * dx).round();
                    let screen_y = (row_sy + x as f32 * dy).round();

                    // Culling: skip tiles outside viewport
                    if screen_x < -margin
                        || screen_x > vw + margin
                        || screen_y < -margin
                        || screen_y > vh + margin
                    {
                        continue;
                    }

                    // Draw tile sprite (or fallback to colored tile)
                    self.draw_tile_sprite(
                        screen_x,
                        screen_y,
                        tile_id,
                        zoom,
                        Some((x as f32, y as f32)),
                        water_effects,
                    );

                    // Draw collision indicator in debug mode
                    if state.debug_mode && tilemap.collision.get(idx).copied().unwrap_or(false) {
                        self.draw_collision_indicator(screen_x, screen_y, zoom);
                    }
                }
            }
        }
    }

    /// Draw block side faces when a tile is elevated above its neighbors.
    /// In isometric view, we see the south (+X) and east (+Y) faces.
    pub(super) fn draw_block_sides(
        &self,
        chunk: &crate::game::chunk::Chunk,
        local_x: i32,
        local_y: i32,
        tile_height: u8,
        block_type_down: u16,
        block_type_right: u16,
        screen_x: f32,
        screen_y: f32,
        zoom: f32,
        state: &GameState,
        coord: &crate::game::ChunkCoord,
        skip_right: bool,
        skip_down: bool,
    ) {
        let hw = TILE_WIDTH / 2.0 * zoom; // half width
        let hh = TILE_HEIGHT / 2.0 * zoom; // half height

        // Diamond vertices of this tile (centered at screen_x, screen_y)
        let right = (screen_x + hw, screen_y);
        let bottom = (screen_x, screen_y + hh);
        let left = (screen_x - hw, screen_y);

        // Helper to get neighbor height (handles chunk boundaries)
        let get_neighbor = |nx: i32, ny: i32| -> u8 {
            if nx >= 0 && nx < CHUNK_SIZE as i32 && ny >= 0 && ny < CHUNK_SIZE as i32 {
                chunk.get_height(nx as u32, ny as u32)
            } else {
                let world_x = coord.x * CHUNK_SIZE as i32 + nx;
                let world_y = coord.y * CHUNK_SIZE as i32 + ny;
                let neighbor_coord = crate::game::ChunkCoord::from_world(world_x, world_y);
                state
                    .chunk_manager
                    .chunks()
                    .get(&neighbor_coord)
                    .map(|c| {
                        let (lx, ly) = crate::game::chunk::world_to_local(world_x, world_y);
                        c.get_height(lx, ly)
                    })
                    .unwrap_or(0)
            }
        };

        // Each face has its own wall sprite ID. 0 = no sprite (fall back to colored triangles).
        let down_gid = if block_type_down > 0 {
            WALLS_FIRSTGID + block_type_down as u32
        } else {
            0
        };
        let right_gid = if block_type_right > 0 {
            WALLS_FIRSTGID + block_type_right as u32
        } else {
            0
        };
        let right_sprite = if right_gid > 0 {
            self.get_wall_sprite(right_gid)
        } else {
            None
        };
        let down_sprite = if down_gid > 0 {
            self.get_wall_sprite(down_gid)
        } else {
            None
        };

        // Directional face tints — light comes from top-right, so:
        //   +X face (SE, catches more light) = slightly darkened
        //   +Y face (SW, faces away from light) = more darkened
        let right_tint = Color::new(0.82, 0.82, 0.82, 1.0);
        let down_tint = Color::new(0.65, 0.65, 0.65, 1.0);

        // +X face (south/front-right): sprite left edge at bottom vertex
        let nh = get_neighbor(local_x + 1, local_y);
        if !skip_right && tile_height > nh {
            let units = (tile_height - nh) as usize;
            if let Some((tex, source_rect)) = right_sprite {
                let face_h = units as f32 * hh;
                let src_w = source_rect.map_or(tex.width(), |r| r.w);
                let src_h = source_rect.map_or(tex.height(), |r| r.h);
                let scaled_w = (src_w * zoom).round();
                let sprite_h = (src_h * zoom).round();
                // Overlap sprites by hh to tile parallelogram shapes seamlessly
                let effective_h = (sprite_h - hh).max(1.0);
                let count = (face_h / effective_h).ceil() as i32;
                // Allow hh above bottom vertex for the parallelogram's slanted top edge;
                // the tile surface covers that triangle via depth sorting
                let clip_top = bottom.1 - hh;
                let face_bottom = bottom.1 + face_h;
                for i in 0..count {
                    let draw_x = bottom.0;
                    let mut draw_y = face_bottom - sprite_h - i as f32 * effective_h;
                    let mut src = source_rect;
                    let mut dest_h = sprite_h;
                    // Clip sprite above face parallelogram bounds
                    if draw_y < clip_top {
                        let clip = clip_top - draw_y;
                        let clip_src = clip / zoom;
                        draw_y = clip_top;
                        dest_h -= clip;
                        src = Some(match src {
                            Some(r) => Rect::new(r.x, r.y + clip_src, r.w, r.h - clip_src),
                            None => Rect::new(0.0, clip_src, src_w, src_h - clip_src),
                        });
                    }
                    if dest_h <= 0.0 {
                        continue;
                    }
                    draw_texture_ex(
                        tex,
                        draw_x.round(),
                        draw_y.round(),
                        right_tint,
                        DrawTextureParams {
                            dest_size: Some(Vec2::new(scaled_w, dest_h)),
                            source: src,
                            ..Default::default()
                        },
                    );
                }
            } else {
                let d = units as f32 * hh;
                let color = Color::from_rgba(90, 70, 45, 255);
                draw_triangle(
                    vec2(right.0, right.1),
                    vec2(bottom.0, bottom.1),
                    vec2(bottom.0, bottom.1 + d),
                    color,
                );
                draw_triangle(
                    vec2(right.0, right.1),
                    vec2(bottom.0, bottom.1 + d),
                    vec2(right.0, right.1 + d),
                    color,
                );
            }
        }

        // +Y face (east/front-left): sprite right edge at bottom vertex
        let nh = get_neighbor(local_x, local_y + 1);
        if !skip_down && tile_height > nh {
            let units = (tile_height - nh) as usize;
            if let Some((tex, source_rect)) = down_sprite {
                let face_h = units as f32 * hh;
                let src_w = source_rect.map_or(tex.width(), |r| r.w);
                let src_h = source_rect.map_or(tex.height(), |r| r.h);
                let scaled_w = (src_w * zoom).round();
                let sprite_h = (src_h * zoom).round();
                // Overlap sprites by hh to tile parallelogram shapes seamlessly
                let effective_h = (sprite_h - hh).max(1.0);
                let count = (face_h / effective_h).ceil() as i32;
                // Allow hh above bottom vertex for the parallelogram's slanted top edge;
                // the tile surface covers that triangle via depth sorting
                let clip_top = bottom.1 - hh;
                let face_bottom = bottom.1 + face_h;
                for i in 0..count {
                    let draw_x = bottom.0 - scaled_w;
                    let mut draw_y = face_bottom - sprite_h - i as f32 * effective_h;
                    let mut src = source_rect;
                    let mut dest_h = sprite_h;
                    // Clip sprite above face parallelogram bounds
                    if draw_y < clip_top {
                        let clip = clip_top - draw_y;
                        let clip_src = clip / zoom;
                        draw_y = clip_top;
                        dest_h -= clip;
                        src = Some(match src {
                            Some(r) => Rect::new(r.x, r.y + clip_src, r.w, r.h - clip_src),
                            None => Rect::new(0.0, clip_src, src_w, src_h - clip_src),
                        });
                    }
                    if dest_h <= 0.0 {
                        continue;
                    }
                    draw_texture_ex(
                        tex,
                        draw_x.round(),
                        draw_y.round(),
                        down_tint,
                        DrawTextureParams {
                            dest_size: Some(Vec2::new(scaled_w, dest_h)),
                            source: src,
                            ..Default::default()
                        },
                    );
                }
            } else {
                let d = units as f32 * hh;
                let color = Color::from_rgba(49, 38, 25, 255);
                draw_triangle(
                    vec2(left.0, left.1),
                    vec2(bottom.0, bottom.1),
                    vec2(bottom.0, bottom.1 + d),
                    color,
                );
                draw_triangle(
                    vec2(left.0, left.1),
                    vec2(bottom.0, bottom.1 + d),
                    vec2(left.0, left.1 + d),
                    color,
                );
            }
        }

        // Back faces (-X, -Y) are not drawn — in isometric view the tile surface
        // occludes them, and depth sorting handles entity occlusion correctly.
    }

    pub(super) fn draw_collision_indicator(&self, screen_x: f32, screen_y: f32, zoom: f32) {
        let half_w = TILE_WIDTH * zoom / 4.0;
        let half_h = TILE_HEIGHT * zoom / 4.0;
        draw_rectangle_lines(
            screen_x - half_w,
            screen_y - half_h,
            half_w * 2.0,
            half_h * 2.0,
            2.0 * zoom,
            Color::from_rgba(255, 0, 0, 150),
        );
    }

    pub(super) fn draw_isometric_object(
        &self,
        screen_x: f32,
        screen_y: f32,
        tile_id: u32,
        zoom: f32,
    ) {
        // Draw shadow ellipse for objects
        draw_ellipse(
            screen_x,
            screen_y + 4.0 * zoom,
            24.0 * zoom,
            16.0 * zoom,
            0.0,
            Color::from_rgba(0, 0, 0, 50),
        );

        // Draw object tile sprite (slightly elevated)
        let elevated_y = screen_y - TILE_HEIGHT * zoom * 0.25;
        self.draw_tile_sprite(screen_x, elevated_y, tile_id, zoom, None, false);
    }

    pub(super) fn draw_isometric_tile(
        &self,
        screen_x: f32,
        screen_y: f32,
        color: Color,
        zoom: f32,
    ) {
        // Draw a diamond-shaped tile
        let half_w = TILE_WIDTH * zoom / 2.0;
        let half_h = TILE_HEIGHT * zoom / 2.0;

        // Diamond vertices (clockwise from top)
        let top = (screen_x, screen_y - half_h);
        let right = (screen_x + half_w, screen_y);
        let bottom = (screen_x, screen_y + half_h);
        let left = (screen_x - half_w, screen_y);

        // Draw as two triangles
        draw_triangle(
            Vec2::new(top.0, top.1),
            Vec2::new(right.0, right.1),
            Vec2::new(bottom.0, bottom.1),
            color,
        );
        draw_triangle(
            Vec2::new(top.0, top.1),
            Vec2::new(bottom.0, bottom.1),
            Vec2::new(left.0, left.1),
            color,
        );

        // Draw outline
        let outline_color = Color::from_rgba(80, 80, 90, 255);
        draw_line(top.0, top.1, right.0, right.1, 1.0, outline_color);
        draw_line(right.0, right.1, bottom.0, bottom.1, 1.0, outline_color);
        draw_line(bottom.0, bottom.1, left.0, left.1, 1.0, outline_color);
        draw_line(left.0, left.1, top.0, top.1, 1.0, outline_color);
    }

    /// Draw a selection highlight around the tile at the given world position
    pub(super) fn render_tile_selection(
        &self,
        world_x: f32,
        world_y: f32,
        world_z: f32,
        camera: &Camera,
    ) {
        // Get the tile the entity is standing on (floor to get tile coords)
        let tile_x = world_x.floor();
        let tile_y = world_y.floor();

        // Get the center of that tile in screen space, accounting for Z elevation
        let (center_x, center_y) = world_to_screen_z(tile_x + 0.5, tile_y + 0.5, world_z, camera);
        let center_y = center_y - TILE_HEIGHT * camera.zoom / 2.0;

        // Tile dimensions (half-sizes for diamond corners), scaled by zoom
        let half_w = TILE_WIDTH * camera.zoom / 2.0;
        let half_h = TILE_HEIGHT * camera.zoom / 2.0;

        // Diamond corners (isometric tile shape)
        let top = (center_x, center_y - half_h);
        let right = (center_x + half_w, center_y);
        let bottom = (center_x, center_y + half_h);
        let left = (center_x - half_w, center_y);

        // Pulsing effect
        let pulse = (macroquad::time::get_time() * 3.0).sin() as f32 * 0.3 + 0.7;
        let alpha = (pulse * 255.0) as u8;
        let color = Color::from_rgba(255, 255, 0, alpha);

        // Draw yellow diamond outline
        let line_width = 2.0 * camera.zoom;
        draw_line(top.0, top.1, right.0, right.1, line_width, color);
        draw_line(right.0, right.1, bottom.0, bottom.1, line_width, color);
        draw_line(bottom.0, bottom.1, left.0, left.1, line_width, color);
        draw_line(left.0, left.1, top.0, top.1, line_width, color);
    }

    /// Draw a single large diamond selection outline covering an NxN tile footprint.
    pub(super) fn render_multi_tile_selection(
        &self,
        world_x: f32,
        world_y: f32,
        world_z: f32,
        size: i32,
        render_offset_y: f32,
        camera: &Camera,
    ) {
        let s = size as f32;
        let y_off = -render_offset_y * camera.zoom;
        // The 4 corners of the NxN diamond in world space:
        // top = (x, y), right = (x+s, y), bottom = (x+s, y+s), left = (x, y+s)
        let top = world_to_screen_z(world_x, world_y, world_z, camera);
        let right = world_to_screen_z(world_x + s, world_y, world_z, camera);
        let bottom = world_to_screen_z(world_x + s, world_y + s, world_z, camera);
        let left = world_to_screen_z(world_x, world_y + s, world_z, camera);
        let top = (top.0, top.1 + y_off);
        let right = (right.0, right.1 + y_off);
        let bottom = (bottom.0, bottom.1 + y_off);
        let left = (left.0, left.1 + y_off);

        let pulse = (macroquad::time::get_time() * 3.0).sin() as f32 * 0.3 + 0.7;
        let alpha = (pulse * 255.0) as u8;
        let color = Color::from_rgba(255, 255, 0, alpha);
        let line_width = 2.0 * camera.zoom;

        draw_line(top.0, top.1, right.0, right.1, line_width, color);
        draw_line(right.0, right.1, bottom.0, bottom.1, line_width, color);
        draw_line(bottom.0, bottom.1, left.0, left.1, line_width, color);
        draw_line(left.0, left.1, top.0, top.1, line_width, color);
    }

    /// Draw corner indicators for the hovered tile
    pub(crate) fn render_tile_hover(&self, tile_x: i32, tile_y: i32, tile_z: i32, camera: &Camera) {
        // Get the center of the tile in screen space, accounting for Z elevation
        let (center_x, center_y) = world_to_screen_z(
            tile_x as f32 + 0.5,
            tile_y as f32 + 0.5,
            tile_z as f32,
            camera,
        );
        let center_y = center_y - TILE_HEIGHT * camera.zoom / 2.0;

        // Tile dimensions (half-sizes for diamond corners), scaled by zoom
        let half_w = TILE_WIDTH * camera.zoom / 2.0;
        let half_h = TILE_HEIGHT * camera.zoom / 2.0;

        // Diamond corners (isometric tile shape)
        let top = (center_x, center_y - half_h);
        let right = (center_x + half_w, center_y);
        let bottom = (center_x, center_y + half_h);
        let left = (center_x - half_w, center_y);

        // Corner size as fraction of edge length
        let t = 0.2;

        // Thin white lines with some transparency
        let color = Color::from_rgba(255, 255, 255, 180);
        let line_width = 1.0 * camera.zoom;

        // Top corner
        draw_line(
            top.0,
            top.1,
            top.0 + (left.0 - top.0) * t,
            top.1 + (left.1 - top.1) * t,
            line_width,
            color,
        );
        draw_line(
            top.0,
            top.1,
            top.0 + (right.0 - top.0) * t,
            top.1 + (right.1 - top.1) * t,
            line_width,
            color,
        );

        // Right corner
        draw_line(
            right.0,
            right.1,
            right.0 + (top.0 - right.0) * t,
            right.1 + (top.1 - right.1) * t,
            line_width,
            color,
        );
        draw_line(
            right.0,
            right.1,
            right.0 + (bottom.0 - right.0) * t,
            right.1 + (bottom.1 - right.1) * t,
            line_width,
            color,
        );

        // Bottom corner
        draw_line(
            bottom.0,
            bottom.1,
            bottom.0 + (right.0 - bottom.0) * t,
            bottom.1 + (right.1 - bottom.1) * t,
            line_width,
            color,
        );
        draw_line(
            bottom.0,
            bottom.1,
            bottom.0 + (left.0 - bottom.0) * t,
            bottom.1 + (left.1 - bottom.1) * t,
            line_width,
            color,
        );

        // Left corner
        draw_line(
            left.0,
            left.1,
            left.0 + (bottom.0 - left.0) * t,
            left.1 + (bottom.1 - left.1) * t,
            line_width,
            color,
        );
        draw_line(
            left.0,
            left.1,
            left.0 + (top.0 - left.0) * t,
            left.1 + (top.1 - left.1) * t,
            line_width,
            color,
        );
    }

    /// Draw a dim overlay on the auto-path destination tile
    pub(super) fn render_destination_tile(
        &self,
        tile_x: i32,
        tile_y: i32,
        tile_z: i32,
        camera: &Camera,
    ) {
        let (center_x, center_y) = world_to_screen_z(
            tile_x as f32 + 0.5,
            tile_y as f32 + 0.5,
            tile_z as f32,
            camera,
        );
        let center_y = center_y - TILE_HEIGHT * camera.zoom / 2.0;

        let half_w = TILE_WIDTH * camera.zoom / 2.0;
        let half_h = TILE_HEIGHT * camera.zoom / 2.0;

        // Diamond vertices
        let top = Vec2::new(center_x, center_y - half_h);
        let right = Vec2::new(center_x + half_w, center_y);
        let bottom = Vec2::new(center_x, center_y + half_h);
        let left = Vec2::new(center_x - half_w, center_y);

        let color = Color::new(0.0, 0.0, 0.0, 0.18);

        // Fill the diamond with two triangles
        draw_triangle(top, right, bottom, color);
        draw_triangle(top, bottom, left, color);
    }

    /// Draw a green drop zone indicator for a tile (when dragging items)
    pub(crate) fn render_drop_zone(
        &self,
        tile_x: i32,
        tile_y: i32,
        tile_z: i32,
        camera: &Camera,
        is_hovered: bool,
    ) {
        // Get the center of the tile in screen space, accounting for Z elevation
        let (center_x, center_y) = world_to_screen_z(
            tile_x as f32 + 0.5,
            tile_y as f32 + 0.5,
            tile_z as f32,
            camera,
        );
        let center_y = center_y - TILE_HEIGHT * camera.zoom / 2.0;

        // Tile dimensions (half-sizes for diamond corners), scaled by zoom
        let half_w = TILE_WIDTH * camera.zoom / 2.0;
        let half_h = TILE_HEIGHT * camera.zoom / 2.0;

        // Diamond corners (isometric tile shape)
        let top = (center_x, center_y - half_h);
        let right = (center_x + half_w, center_y);
        let bottom = (center_x, center_y + half_h);
        let left = (center_x - half_w, center_y);

        // Green color - brighter when hovered
        let color = if is_hovered {
            Color::from_rgba(100, 255, 100, 120)
        } else {
            Color::from_rgba(100, 200, 100, 60)
        };

        // Draw filled diamond shape
        draw_triangle(
            Vec2::new(top.0, top.1),
            Vec2::new(right.0, right.1),
            Vec2::new(bottom.0, bottom.1),
            color,
        );
        draw_triangle(
            Vec2::new(top.0, top.1),
            Vec2::new(bottom.0, bottom.1),
            Vec2::new(left.0, left.1),
            color,
        );

        // Draw border if hovered
        if is_hovered {
            let border_color = Color::from_rgba(150, 255, 150, 200);
            let line_width = 1.0 * camera.zoom;
            draw_line(top.0, top.1, right.0, right.1, line_width, border_color);
            draw_line(
                right.0,
                right.1,
                bottom.0,
                bottom.1,
                line_width,
                border_color,
            );
            draw_line(bottom.0, bottom.1, left.0, left.1, line_width, border_color);
            draw_line(left.0, left.1, top.0, top.1, line_width, border_color);
        }
    }
}
