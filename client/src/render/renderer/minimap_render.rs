use super::*;

impl Renderer {
    /// Draws the static minimap raster (background, sampled ground tiles, chunk
    /// outlines) for the given bounds. This is the expensive part — separated so
    /// callers can cache it into a render target instead of redrawing thousands
    /// of tile rectangles every frame.
    pub(super) fn draw_minimap_raster(
        &self,
        state: &GameState,
        bounds: &MinimapBounds,
        map_rect: Rect,
        tile_budget: usize,
        use_world_map_snapshot: bool,
    ) {
        draw_rectangle(
            map_rect.x,
            map_rect.y,
            map_rect.w,
            map_rect.h,
            Color::new(0.045, 0.065, 0.075, 0.95),
        );

        let base_step_x = (bounds.width() / map_rect.w.max(1.0)).ceil().max(1.0) as usize;
        let base_step_y = (bounds.height() / map_rect.h.max(1.0)).ceil().max(1.0) as usize;
        let visible_tiles =
            (bounds.width().ceil().max(1.0) * bounds.height().ceil().max(1.0)) as usize;
        let budget_step = ((visible_tiles as f32 / tile_budget.max(1) as f32)
            .sqrt()
            .ceil()
            .max(1.0)) as usize;
        let sample_step_x = base_step_x.max(budget_step);
        let sample_step_y = base_step_y.max(budget_step);

        let interior_size = state.chunk_manager.get_interior_size();
        // Draw a primitive world raster by sampling the ground tile color per tile.
        if use_world_map_snapshot && interior_size.is_none() {
            if let Some(snapshot) = state.world_map_snapshot.as_ref() {
                self.draw_world_map_snapshot(bounds, map_rect, snapshot);
            }
        } else if !state.chunk_manager.chunks().is_empty() {
            for (coord, chunk) in state.chunk_manager.chunks().iter() {
                let Some(ground_layer) = chunk
                    .layers
                    .iter()
                    .find(|layer| layer.layer_type == ChunkLayerType::Ground)
                else {
                    continue;
                };

                let (tile_w, tile_h, base_x, base_y) = if let Some((w, h)) = interior_size {
                    if coord.x != 0 || coord.y != 0 {
                        continue;
                    }
                    (w as usize, h as usize, 0i32, 0i32)
                } else {
                    (
                        CHUNK_SIZE as usize,
                        CHUNK_SIZE as usize,
                        coord.x * CHUNK_SIZE as i32,
                        coord.y * CHUNK_SIZE as i32,
                    )
                };
                let chunk_min_x = base_x as f32;
                let chunk_min_y = base_y as f32;
                let chunk_max_x = chunk_min_x + tile_w as f32;
                let chunk_max_y = chunk_min_y + tile_h as f32;
                if chunk_max_x <= bounds.min_x
                    || chunk_min_x >= bounds.max_x
                    || chunk_max_y <= bounds.min_y
                    || chunk_min_y >= bounds.max_y
                {
                    continue;
                }

                for y in (0..tile_h).step_by(sample_step_y) {
                    let row_start = y * tile_w;
                    if row_start >= ground_layer.tiles.len() {
                        break;
                    }
                    for x in (0..tile_w).step_by(sample_step_x) {
                        let idx = row_start + x;
                        if idx >= ground_layer.tiles.len() {
                            break;
                        }
                        let tile_id = ground_layer.tiles[idx];
                        if tile_id == 0 {
                            continue;
                        }

                        let world_x = base_x + x as i32;
                        let world_y = base_y + y as i32;
                        let tile_span_x = sample_step_x.min(tile_w.saturating_sub(x).max(1));
                        let tile_span_y = sample_step_y.min(tile_h.saturating_sub(y).max(1));
                        let tile_min_x = world_x as f32;
                        let tile_min_y = world_y as f32;
                        let tile_max_x = (world_x + tile_span_x as i32) as f32;
                        let tile_max_y = (world_y + tile_span_y as i32) as f32;
                        if tile_max_x <= bounds.min_x
                            || tile_min_x >= bounds.max_x
                            || tile_max_y <= bounds.min_y
                            || tile_min_y >= bounds.max_y
                        {
                            continue;
                        }
                        let (sx1, sy1) =
                            self.minimap_world_to_screen(bounds, map_rect, tile_min_x, tile_min_y);
                        let (sx2, sy2) =
                            self.minimap_world_to_screen(bounds, map_rect, tile_max_x, tile_max_y);
                        let rect_x = sx1.min(sx2);
                        let rect_y = sy1.min(sy2);
                        let rect_w = (sx2 - sx1).abs().max(1.0);
                        let rect_h = (sy2 - sy1).abs().max(1.0);

                        draw_rectangle(
                            rect_x,
                            rect_y,
                            rect_w,
                            rect_h,
                            self.minimap_tile_color(tile_id),
                        );
                    }
                }
            }
        } else {
            // Fallback to legacy local tilemap if chunk streaming has not initialized yet.
            if let Some(layer) = state
                .tilemap
                .layers
                .iter()
                .find(|l| l.layer_type == LayerType::Ground)
            {
                let width = state.tilemap.width as usize;
                let height = state.tilemap.height as usize;
                for y in (0..height).step_by(sample_step_y) {
                    for x in (0..width).step_by(sample_step_x) {
                        let idx = y * width + x;
                        let tile_id = layer.tiles.get(idx).copied().unwrap_or(0);
                        if tile_id == 0 {
                            continue;
                        }

                        let tile_span_x = sample_step_x.min(width.saturating_sub(x).max(1));
                        let tile_span_y = sample_step_y.min(height.saturating_sub(y).max(1));
                        let tile_min_x = x as f32;
                        let tile_min_y = y as f32;
                        let tile_max_x = (x + tile_span_x) as f32;
                        let tile_max_y = (y + tile_span_y) as f32;
                        if tile_max_x <= bounds.min_x
                            || tile_min_x >= bounds.max_x
                            || tile_max_y <= bounds.min_y
                            || tile_min_y >= bounds.max_y
                        {
                            continue;
                        }
                        let (sx1, sy1) =
                            self.minimap_world_to_screen(bounds, map_rect, tile_min_x, tile_min_y);
                        let (sx2, sy2) =
                            self.minimap_world_to_screen(bounds, map_rect, tile_max_x, tile_max_y);
                        let rect_x = sx1.min(sx2);
                        let rect_y = sy1.min(sy2);
                        let rect_w = (sx2 - sx1).abs().max(1.0);
                        let rect_h = (sy2 - sy1).abs().max(1.0);

                        draw_rectangle(
                            rect_x,
                            rect_y,
                            rect_w,
                            rect_h,
                            self.minimap_tile_color(tile_id),
                        );
                    }
                }
            }
        }

        if use_world_map_snapshot && interior_size.is_none() {
            if let Some(snapshot) = state.world_map_snapshot.as_ref() {
                self.draw_world_map_chunk_outlines(bounds, map_rect, snapshot);
            }
        } else {
            for coord in state.chunk_manager.chunks().keys() {
                let (chunk_x, chunk_y, chunk_w, chunk_h) = if let Some((w, h)) = interior_size {
                    if coord.x != 0 || coord.y != 0 {
                        continue;
                    }
                    (0.0, 0.0, w as f32, h as f32)
                } else {
                    (
                        (coord.x * CHUNK_SIZE as i32) as f32,
                        (coord.y * CHUNK_SIZE as i32) as f32,
                        CHUNK_SIZE as f32,
                        CHUNK_SIZE as f32,
                    )
                };
                if chunk_x + chunk_w <= bounds.min_x
                    || chunk_x >= bounds.max_x
                    || chunk_y + chunk_h <= bounds.min_y
                    || chunk_y >= bounds.max_y
                {
                    continue;
                }

                let (sx1, sy1) = self.minimap_world_to_screen(bounds, map_rect, chunk_x, chunk_y);
                let (sx2, sy2) = self.minimap_world_to_screen(
                    bounds,
                    map_rect,
                    chunk_x + chunk_w,
                    chunk_y + chunk_h,
                );
                let rect_x = sx1.min(sx2);
                let rect_y = sy1.min(sy2);
                let rect_w = (sx2 - sx1).abs().max(1.0);
                let rect_h = (sy2 - sy1).abs().max(1.0);

                draw_rectangle(
                    rect_x,
                    rect_y,
                    rect_w,
                    rect_h,
                    Color::new(0.0, 0.0, 0.0, 0.08),
                );
                draw_rectangle_lines(
                    rect_x,
                    rect_y,
                    rect_w,
                    rect_h,
                    1.0,
                    Color::new(0.35, 0.50, 0.40, 0.30),
                );
            }
        }
    }

    /// Draws the full minimap (cached-able raster + live markers) directly to the
    /// current render target. Used by the expanded map panel; the small HUD
    /// preview uses a cached-raster fast path (see `render_minimap_preview`).
    pub(super) fn draw_minimap_contents(
        &self,
        state: &GameState,
        bounds: &MinimapBounds,
        markers: &[MinimapMarker],
        map_rect: Rect,
        marker_scale: f32,
        hovered_marker: Option<usize>,
        capture_hitboxes: bool,
        tile_budget: usize,
        use_world_map_snapshot: bool,
    ) -> Vec<(usize, Rect)> {
        self.draw_minimap_raster(state, bounds, map_rect, tile_budget, use_world_map_snapshot);
        self.draw_minimap_markers(
            state,
            bounds,
            markers,
            map_rect,
            marker_scale,
            hovered_marker,
            capture_hitboxes,
            true,
        )
    }

    /// Draws the dynamic minimap markers (players, NPCs, items, destination flag)
    /// on top of an already-drawn raster. Returns marker hitboxes when requested.
    pub(super) fn draw_minimap_markers(
        &self,
        state: &GameState,
        bounds: &MinimapBounds,
        markers: &[MinimapMarker],
        map_rect: Rect,
        marker_scale: f32,
        hovered_marker: Option<usize>,
        capture_hitboxes: bool,
        // When false, clip icon markers in source space instead of via a GL scissor.
        // A scissor costs two gl.flush() (mid-frame batch submits); the always-on HUD
        // preview can't afford that every frame, while the on-demand panel keeps it.
        use_scissor: bool,
    ) -> Vec<(usize, Rect)> {
        let mut hitboxes: Vec<(usize, Rect)> =
            Vec::with_capacity(if capture_hitboxes { markers.len() } else { 0 });

        // Scissor clip markers to the map rect so icons don't bleed over bevels
        if use_scissor {
            let physical_w = screen_width();
            let physical_h = screen_height();
            let (vw, vh) = virtual_screen_size();
            let clip_sx = physical_w / vw;
            let clip_sy = physical_h / vh;
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            gl.quad_gl.scissor(Some((
                (map_rect.x * clip_sx) as i32,
                (map_rect.y * clip_sy) as i32,
                (map_rect.w * clip_sx) as i32,
                (map_rect.h * clip_sy) as i32,
            )));
        }

        // Draw player markers in a second pass so they always stay above other marker types.
        for draw_player_pass in [false, true] {
            for (idx, marker) in markers.iter().enumerate() {
                let is_player = marker.kind == MinimapMarkerKind::Player;
                if is_player != draw_player_pass {
                    continue;
                }
                if marker.x < bounds.min_x
                    || marker.x > bounds.max_x
                    || marker.y < bounds.min_y
                    || marker.y > bounds.max_y
                {
                    continue;
                }
                let (sx, sy) = self.minimap_world_to_screen(bounds, map_rect, marker.x, marker.y);
                let (color, base_radius) = Self::minimap_marker_style(marker.kind);
                let hovered = hovered_marker == Some(idx);

                let use_icon = marker.icon_index < 10 && self.map_icons.is_some();
                let radius;
                if use_icon {
                    let tex = self.map_icons.as_ref().unwrap();
                    let icon_size = 16.0;
                    radius = icon_size * 0.5;
                    let src = Rect::new(marker.icon_index as f32 * 16.0, 0.0, 16.0, 16.0);
                    let dest_x = sx - icon_size * 0.5;
                    let dest_y = sy - icon_size * 0.5;
                    if use_scissor {
                        draw_texture_ex(
                            tex,
                            dest_x,
                            dest_y,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(macroquad::math::Vec2::new(icon_size, icon_size)),
                                source: Some(src),
                                ..Default::default()
                            },
                        );
                    } else {
                        self.draw_texture_box_clipped(
                            tex, dest_x, dest_y, icon_size, icon_size, src, map_rect,
                        );
                    }
                    if hovered {
                        if let Some(outline_tex) = &self.map_icons_outlines {
                            let outline_src =
                                Rect::new(marker.icon_index as f32 * 18.0, 0.0, 18.0, 18.0);
                            draw_texture_ex(
                                outline_tex,
                                dest_x - 1.0,
                                dest_y - 1.0,
                                WHITE,
                                DrawTextureParams {
                                    dest_size: Some(macroquad::math::Vec2::new(18.0, 18.0)),
                                    source: Some(outline_src),
                                    ..Default::default()
                                },
                            );
                        }
                    }
                } else {
                    radius = base_radius * marker_scale + if hovered { 1.4 } else { 0.0 };
                    draw_circle(sx, sy, radius + 1.2, Color::new(0.0, 0.0, 0.0, 0.65));
                    draw_circle(sx, sy, radius, color);
                    if hovered {
                        draw_circle_lines(
                            sx,
                            sy,
                            radius + 1.6,
                            1.0,
                            Color::new(1.0, 1.0, 1.0, 0.9),
                        );
                    }
                }

                if capture_hitboxes {
                    hitboxes.push((
                        idx,
                        Rect::new(
                            sx - radius - 3.0,
                            sy - radius - 3.0,
                            (radius + 3.0) * 2.0,
                            (radius + 3.0) * 2.0,
                        ),
                    ));
                }
            }
        }

        // Draw destination flag for active auto-path
        if let Some(ref path_state) = state.auto_path {
            if let Some(flag_tex) = &self.destination_flag {
                let dest_x = path_state.destination.0 as f32 + 0.5;
                let dest_y = path_state.destination.1 as f32 + 0.5;
                if dest_x >= bounds.min_x
                    && dest_x <= bounds.max_x
                    && dest_y >= bounds.min_y
                    && dest_y <= bounds.max_y
                {
                    let (sx, sy) = self.minimap_world_to_screen(bounds, map_rect, dest_x, dest_y);
                    let flag_w = flag_tex.width();
                    let flag_h = flag_tex.height();
                    // Anchor the flag's bottom-center pole to the destination point
                    if use_scissor {
                        draw_texture_ex(
                            flag_tex,
                            sx - flag_w * 0.5,
                            sy - flag_h,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(macroquad::math::Vec2::new(flag_w, flag_h)),
                                ..Default::default()
                            },
                        );
                    } else {
                        self.draw_texture_box_clipped(
                            flag_tex,
                            sx - flag_w * 0.5,
                            sy - flag_h,
                            flag_w,
                            flag_h,
                            Rect::new(0.0, 0.0, flag_w, flag_h),
                            map_rect,
                        );
                    }
                }
            }
        }

        // Disable scissor clipping
        if use_scissor {
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            gl.quad_gl.scissor(None);
        }

        hitboxes
    }

    pub(super) fn draw_world_map_snapshot(
        &self,
        bounds: &MinimapBounds,
        map_rect: Rect,
        snapshot: &WorldMapSnapshot,
    ) {
        let chunk_screen_size = (map_rect.w * CHUNK_SIZE as f32 / bounds.width())
            .min(map_rect.h * CHUNK_SIZE as f32 / bounds.height());
        let use_high_detail = snapshot.high_sample_dim > snapshot.low_sample_dim
            && chunk_screen_size / snapshot.high_sample_dim as f32 >= 2.0;
        let sample_dim = if use_high_detail {
            snapshot.high_sample_dim.max(1)
        } else {
            snapshot.low_sample_dim.max(1)
        };
        let tile_span = CHUNK_SIZE as f32 / sample_dim as f32;
        for chunk in &snapshot.chunks {
            let base_x = chunk.chunk_x * CHUNK_SIZE as i32;
            let base_y = chunk.chunk_y * CHUNK_SIZE as i32;
            let tiles = if use_high_detail {
                &chunk.high_tiles
            } else {
                &chunk.low_tiles
            };
            for sample_y in 0..sample_dim {
                for sample_x in 0..sample_dim {
                    let idx = sample_y * sample_dim + sample_x;
                    let tile_id = tiles.get(idx).copied().unwrap_or(0);
                    if tile_id == 0 {
                        continue;
                    }
                    let tile_min_x = base_x as f32 + sample_x as f32 * tile_span;
                    let tile_min_y = base_y as f32 + sample_y as f32 * tile_span;
                    let tile_max_x = tile_min_x + tile_span;
                    let tile_max_y = tile_min_y + tile_span;
                    if tile_max_x <= bounds.min_x
                        || tile_min_x >= bounds.max_x
                        || tile_max_y <= bounds.min_y
                        || tile_min_y >= bounds.max_y
                    {
                        continue;
                    }
                    let (sx1, sy1) =
                        self.minimap_world_to_screen(bounds, map_rect, tile_min_x, tile_min_y);
                    let (sx2, sy2) =
                        self.minimap_world_to_screen(bounds, map_rect, tile_max_x, tile_max_y);
                    draw_rectangle(
                        sx1.min(sx2),
                        sy1.min(sy2),
                        (sx2 - sx1).abs().max(1.0),
                        (sy2 - sy1).abs().max(1.0),
                        self.minimap_tile_color(tile_id),
                    );
                }
            }
        }
    }

    pub(super) fn draw_world_map_chunk_outlines(
        &self,
        bounds: &MinimapBounds,
        map_rect: Rect,
        snapshot: &WorldMapSnapshot,
    ) {
        for chunk in &snapshot.chunks {
            let chunk_x = (chunk.chunk_x * CHUNK_SIZE as i32) as f32;
            let chunk_y = (chunk.chunk_y * CHUNK_SIZE as i32) as f32;
            let chunk_w = CHUNK_SIZE as f32;
            let chunk_h = CHUNK_SIZE as f32;
            if chunk_x + chunk_w <= bounds.min_x
                || chunk_x >= bounds.max_x
                || chunk_y + chunk_h <= bounds.min_y
                || chunk_y >= bounds.max_y
            {
                continue;
            }
            let (sx1, sy1) = self.minimap_world_to_screen(bounds, map_rect, chunk_x, chunk_y);
            let (sx2, sy2) = self.minimap_world_to_screen(
                bounds,
                map_rect,
                chunk_x + chunk_w,
                chunk_y + chunk_h,
            );
            draw_rectangle(
                sx1.min(sx2),
                sy1.min(sy2),
                (sx2 - sx1).abs().max(1.0),
                (sy2 - sy1).abs().max(1.0),
                Color::new(0.0, 0.0, 0.0, 0.08),
            );
            draw_rectangle_lines(
                sx1.min(sx2),
                sy1.min(sy2),
                (sx2 - sx1).abs().max(1.0),
                (sy2 - sy1).abs().max(1.0),
                1.0,
                Color::new(0.40, 0.52, 0.46, 0.14),
            );
        }
    }

    pub(super) fn render_minimap_preview(&self, state: &GameState) {
        let s = self.font_scale.get();
        let preview_rect = self.minimap_preview_rect();
        self.draw_minimap_preview_frame(
            preview_rect.x,
            preview_rect.y,
            preview_rect.w,
            preview_rect.h,
        );

        let title = "Minimap [M]";
        self.draw_text_sharp(
            title,
            preview_rect.x + 8.0 * s,
            preview_rect.y + 17.0 * s,
            MINIMAP_WORLD_TEXT_SIZE,
            TEXT_TITLE,
        );

        let map_rect = Rect::new(
            preview_rect.x + 6.0 * s,
            preview_rect.y + 24.0 * s,
            preview_rect.w - 12.0 * s,
            preview_rect.h - 30.0 * s,
        );

        if let Some(player) = state.get_local_player() {
            // Quantize the view center to the nearest tile so the cached raster only
            // regenerates when the player crosses a tile boundary (not on every
            // sub-pixel interpolation step). Markers are still drawn live against
            // these same bounds, so the raster and markers never drift apart.
            let center_tx = player.x.round() as i32;
            let center_ty = player.y.round() as i32;
            let half_span = CHUNK_SIZE as f32 * (MINIMAP_VISIBLE_CHUNK_RADIUS + 0.5);
            let bounds = MinimapBounds {
                min_x: center_tx as f32 - half_span,
                min_y: center_ty as f32 - half_span,
                max_x: center_tx as f32 + half_span,
                max_y: center_ty as f32 + half_span,
            };
            let key = (center_tx, center_ty, state.chunk_manager.revision());

            // Lazily create the off-screen raster buffer. sample_count: 0 skips the
            // resolve path (glDrawBuffers), which is unavailable on WebGL 1.
            {
                let mut rt_opt = self.minimap_preview_rt.borrow_mut();
                if rt_opt.is_none() {
                    let rt = render_target_ex(
                        MINIMAP_PREVIEW_RT_W,
                        MINIMAP_PREVIEW_RT_H,
                        RenderTargetParams {
                            sample_count: 0,
                            depth: false,
                        },
                    );
                    rt.texture.set_filter(FilterMode::Nearest);
                    *rt_opt = Some(rt);
                }
            }
            let rt = self.minimap_preview_rt.borrow().as_ref().unwrap().clone();

            // Re-rasterize only when the cache key changed. This is the expensive
            // part (thousands of tile rects) — skipping it while standing still
            // takes the HUD minimap from ~3.9ms/frame to a single textured blit.
            if self.minimap_preview_key.get() != Some(key) {
                let rt_rect =
                    Rect::new(0.0, 0.0, MINIMAP_PREVIEW_RT_W as f32, MINIMAP_PREVIEW_RT_H as f32);
                set_camera(&Camera2D {
                    render_target: Some(rt.clone()),
                    ..Camera2D::from_display_rect(rt_rect)
                });
                clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
                self.draw_minimap_raster(
                    state,
                    &bounds,
                    rt_rect,
                    MINIMAP_PREVIEW_TILE_BUDGET,
                    false,
                );
                set_default_camera();
                self.minimap_preview_key.set(Some(key));
            }

            // Blit the cached raster into the preview, then draw live markers on top.
            draw_texture_ex(
                &rt.texture,
                map_rect.x,
                map_rect.y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(map_rect.w, map_rect.h)),
                    flip_y: true,
                    ..Default::default()
                },
            );
            let markers = self.collect_minimap_markers(state, Some(&bounds), false);
            // use_scissor=false: source-clip icons instead, avoiding two gl.flush()/frame.
            self.draw_minimap_markers(state, &bounds, &markers, map_rect, 0.8, None, false, false);
        } else {
            draw_rectangle(
                map_rect.x,
                map_rect.y,
                map_rect.w,
                map_rect.h,
                Color::new(0.05, 0.05, 0.07, 0.85),
            );
            self.draw_text_sharp(
                "Loading map...",
                map_rect.x + 10.0 * s,
                map_rect.y + 24.0 * s,
                16.0,
                TEXT_DIM,
            );
        }
    }

    pub(super) fn render_minimap_overlay(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        if state.get_local_player().is_none() || state.current_instance.is_some() {
            return;
        }

        if self.minimap_preview_enabled(state) {
            layout.add(UiElementId::MinimapToggle, self.minimap_preview_rect());
        }

        if !state.ui_state.minimap_panel_open {
            return;
        }

        let (sw, sh) = virtual_screen_size();
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.45));
        layout.add(UiElementId::MinimapPanel, Rect::new(0.0, 0.0, sw, sh));

        let panel_rect = self.minimap_panel_rect();
        let (px, py, pw, ph) = (panel_rect.x, panel_rect.y, panel_rect.w, panel_rect.h);
        // Solid, premium dark shell. A focused full-screen map wants to feel
        // grounded, not translucent: a soft drop shadow gives depth and separates
        // it from the world, over an opaque cool dark fill — no edge borders.
        draw_rectangle(
            px - 4.0,
            py - 4.0,
            pw + 8.0,
            ph + 8.0,
            Color::new(0.0, 0.0, 0.0, 0.22),
        );
        draw_rectangle(
            px - 2.0,
            py - 2.0,
            pw + 4.0,
            ph + 4.0,
            Color::new(0.0, 0.0, 0.0, 0.30),
        );
        draw_rectangle(px, py, pw, ph, Color::new(0.055, 0.063, 0.078, 0.985));

        // Title floats over the translucent header zone (no filled strip / divider).
        let title_band = 30.0;
        let title = "World Map";
        let title_w = self
            .measure_text_sharp(title, MINIMAP_WORLD_TEXT_SIZE)
            .width;
        self.draw_text_sharp(
            title,
            (panel_rect.x + (panel_rect.w - title_w) * 0.5).floor(),
            (panel_rect.y + 21.0).floor(),
            MINIMAP_WORLD_TEXT_SIZE,
            TEXT_TITLE,
        );

        // Bare "X" close glyph — no bezel/box, just the line-drawn mark that
        // brightens to red on hover.
        let close_size = 22.0;
        let close_rect = Rect::new(
            panel_rect.x + panel_rect.w - close_size - 8.0,
            panel_rect.y + (title_band - close_size) * 0.5,
            close_size,
            close_size,
        );
        let close_hovered = matches!(hovered, Some(UiElementId::MinimapClose));
        let x_color = if close_hovered {
            Color::new(1.0, 0.4, 0.4, 1.0)
        } else {
            TEXT_DIM
        };
        let x_margin = close_size * 0.3;
        draw_line(
            close_rect.x + x_margin,
            close_rect.y + x_margin,
            close_rect.x + close_size - x_margin,
            close_rect.y + close_size - x_margin,
            2.0,
            x_color,
        );
        draw_line(
            close_rect.x + close_size - x_margin,
            close_rect.y + x_margin,
            close_rect.x + x_margin,
            close_rect.y + close_size - x_margin,
            2.0,
            x_color,
        );
        layout.add(UiElementId::MinimapClose, close_rect);

        let map_rect = Rect::new(
            panel_rect.x + 14.0,
            panel_rect.y + title_band + 6.0,
            panel_rect.w - 28.0,
            panel_rect.h - title_band - 6.0 - 52.0,
        );

        let hovered_marker_idx = match hovered {
            Some(UiElementId::MinimapMarker(idx)) => Some(*idx),
            _ => None,
        };

        if let Some(world_bounds) = self.minimap_bounds(state) {
            let view_bounds = self.minimap_panel_view_bounds(state, world_bounds);
            let markers = self.declutter_minimap_markers(
                &self.collect_minimap_markers(state, Some(&view_bounds), true),
                &view_bounds,
                map_rect,
            );
            let marker_hitboxes = self.draw_minimap_contents(
                state,
                &view_bounds,
                &markers,
                map_rect,
                1.0,
                hovered_marker_idx,
                true,
                MINIMAP_PANEL_TILE_BUDGET,
                true,
            );

            for (marker_idx, hitbox) in marker_hitboxes {
                layout.add(UiElementId::MinimapMarker(marker_idx), hitbox);
            }

            let footer_left = panel_rect.x + 14.0;
            let footer_width = panel_rect.w - 28.0;
            let footer_text_size = MINIMAP_WORLD_TEXT_SIZE;
            let status_y = panel_rect.y + panel_rect.h - 34.0;
            let legend_y = panel_rect.y + panel_rect.h - 14.0;
            // Legend items: (icon_index, label)
            let legend_items: [(u8, &str); 6] = [
                (7, "Teleport"),
                (8, "Enemy"),
                (1, "Tree"),
                (6, "Quest"),
                (255, "Service"),
                (9, "Chest"),
            ];
            let slot_width = footer_width / legend_items.len() as f32;
            let legend_icon_size = 10.0;
            let icon_gap = 4.0;

            for (idx, (icon_idx, label)) in legend_items.iter().enumerate() {
                let label_w = self.measure_text_sharp(label, footer_text_size).width;
                let slot_center_x = footer_left + slot_width * (idx as f32 + 0.5);
                let group_w = legend_icon_size + icon_gap + label_w;
                let group_left = slot_center_x - group_w / 2.0;
                let icon_x = group_left;
                let text_x = icon_x + legend_icon_size + icon_gap;

                if *icon_idx < 10 {
                    if let Some(tex) = &self.map_icons {
                        let src = Rect::new(*icon_idx as f32 * 16.0, 0.0, 16.0, 16.0);
                        draw_texture_ex(
                            tex,
                            icon_x,
                            legend_y - legend_icon_size + 1.0,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(macroquad::math::Vec2::new(
                                    legend_icon_size,
                                    legend_icon_size,
                                )),
                                source: Some(src),
                                ..Default::default()
                            },
                        );
                    } else {
                        let (color, _) = Self::minimap_marker_style(match *icon_idx {
                            7 => MinimapMarkerKind::Teleport,
                            8 => MinimapMarkerKind::Enemy,
                            6 => MinimapMarkerKind::Quest,
                            9 => MinimapMarkerKind::Chest,
                            _ => MinimapMarkerKind::Tree,
                        });
                        draw_circle(icon_x + 3.0, legend_y - 4.0, 3.0, color);
                    }
                } else {
                    let (color, _) = Self::minimap_marker_style(MinimapMarkerKind::Service);
                    draw_circle(icon_x + 3.0, legend_y - 4.0, 3.0, color);
                }
                self.draw_text_sharp(label, text_x, legend_y, footer_text_size, TEXT_NORMAL);
            }

            let zoom = state
                .ui_state
                .minimap_panel_zoom
                .clamp(MINIMAP_PANEL_MIN_ZOOM, MINIMAP_PANEL_MAX_ZOOM);
            let (status_text, status_color) = if let Some(idx) = hovered_marker_idx {
                if let Some(marker) = markers.get(idx) {
                    (format!("Selected: {}", marker.label), TEXT_TITLE)
                } else {
                    (
                        format!("Zoom {:.1}x | Scroll to zoom | Drag to pan", zoom),
                        TEXT_DIM,
                    )
                }
            } else {
                (
                    format!("Zoom {:.1}x | Scroll to zoom | Drag to pan", zoom),
                    TEXT_DIM,
                )
            };
            let status_w = self
                .measure_text_sharp(&status_text, footer_text_size)
                .width;
            self.draw_text_sharp(
                &status_text,
                panel_rect.x + (panel_rect.w - status_w) * 0.5,
                status_y,
                footer_text_size,
                status_color,
            );
        } else {
            draw_rectangle(
                map_rect.x,
                map_rect.y,
                map_rect.w,
                map_rect.h,
                Color::new(0.05, 0.05, 0.07, 0.85),
            );
            self.draw_text_sharp(
                "Map data not loaded yet.",
                map_rect.x + 12.0,
                map_rect.y + 28.0,
                MINIMAP_WORLD_TEXT_SIZE,
                TEXT_DIM,
            );
        }
    }
}
