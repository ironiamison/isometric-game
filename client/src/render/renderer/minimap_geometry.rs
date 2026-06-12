use super::*;

impl Renderer {
    pub(super) fn minimap_preview_rect(&self) -> Rect {
        let (sw, _) = virtual_screen_size();
        let s = self.font_scale.get();
        let width = MINIMAP_PREVIEW_WIDTH * s;
        let height = MINIMAP_PREVIEW_HEIGHT * s;
        let margin = MINIMAP_MARGIN * s;
        let y = MINIMAP_PREVIEW_Y * s;
        Rect::new((sw - width - margin).floor(), y.floor(), width, height)
    }

    pub(super) fn minimap_preview_enabled(&self, state: &GameState) -> bool {
        !state.ui_state.graphics_low && state.current_instance.is_none()
    }

    pub(super) fn minimap_panel_rect(&self) -> Rect {
        let (sw, sh) = virtual_screen_size();
        let panel_w = (sw * 0.72).clamp(420.0, 760.0);
        let panel_h = (sh * 0.72).clamp(320.0, 620.0);
        Rect::new(
            ((sw - panel_w) * 0.5).floor(),
            ((sh - panel_h) * 0.5).floor(),
            panel_w,
            panel_h,
        )
    }

    pub(super) fn local_name_tag_position(&self, _state: &GameState) -> (f32, f32) {
        #[cfg(target_os = "android")]
        {
            (10.0, 46.0)
        }
        #[cfg(not(target_os = "android"))]
        {
            // Inset the content so the unified cluster frame (drawn around the name
            // tag + stat bars) lands a few px from the screen corner. Keep in sync
            // with the `cpad` used when drawing the frame in ui_frame.rs.
            let s = self.font_scale.get();
            let inset = FRAME_THICKNESS + 4.0 * s;
            (6.0 + inset, 6.0 + inset)
        }
    }

    pub(super) fn minimap_stats_stack_position(
        &self,
        state: &GameState,
        bar_width: f32,
    ) -> (f32, f32) {
        let _ = bar_width;
        let s = self.font_scale.get();
        let (name_tag_x, name_tag_y) = self.local_name_tag_position(state);
        // Desktop reserves a taller header (portrait + name + level) before the bars;
        // android keeps the original compact single-line name tag.
        #[cfg(target_os = "android")]
        let header_block = 22.0 * s + 4.0 * s;
        #[cfg(not(target_os = "android"))]
        let header_block = 26.0 * s + 4.0 * s;
        (name_tag_x.floor(), (name_tag_y + header_block).floor())
    }

    /// Width of the top-left HUD stat bars / cluster. Driven by the name+level text
    /// so longer names widen the cluster. Shared by the draw pass and the hit-test
    /// registration so they never drift apart.
    pub(super) fn hud_bar_width(&self, state: &GameState) -> f32 {
        let s = self.font_scale.get();
        let font_size = 16.0;
        let padding = 6.0;
        if let Some(player) = state.get_local_player() {
            let name_w = self.measure_text_sharp(&player.name, font_size).width;
            let level_text = format!(" Lv.{}", player.skills.total_level());
            let level_w = self.measure_text_sharp(&level_text, font_size).width;
            // Wider HUD cluster so the bars + style row read long and roomy.
            (name_w + level_w + padding * 2.0).max(200.0 * s)
        } else {
            200.0 * s
        }
    }

    /// Extra vertical space the desktop stat cluster reserves below the bars for the
    /// embedded combat-style selector + the frame's bottom padding. Transient HUD
    /// indicators (gathering/stall/dash/chips/trackers) anchor below this. Zero on
    /// android (no embedded selector / cluster frame).
    pub(super) fn hud_below_bars_offset(&self) -> f32 {
        if cfg!(target_os = "android") {
            0.0
        } else {
            let s = self.font_scale.get();
            // Just the cluster's bottom frame padding (cpad=4 + FRAME_THICKNESS).
            4.0 * s + FRAME_THICKNESS
        }
    }

    pub(super) fn draw_minimap_preview_frame(&self, x: f32, y: f32, w: f32, h: f32) {
        // Lightweight treatment matching the redesigned HUD: a thin border + a
        // translucent fill that lets the world bleed through, instead of the old
        // bronze multi-layer bevel, solid header strip and gold corner accents.
        // Keeps the minimap reading as part of the modern, low-intrusion HUD
        // family (portrait cluster, chat, menu bars) rather than a heavy panel.
        self.draw_hud_cluster_frame(x, y, w, h);
    }

    pub(super) fn minimap_bounds(&self, state: &GameState) -> Option<MinimapBounds> {
        let mut bounds = if let Some((width, height)) = state.chunk_manager.get_interior_size() {
            MinimapBounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: width as f32,
                max_y: height as f32,
            }
        } else if let Some(snapshot) = state.world_map_snapshot.as_ref() {
            MinimapBounds {
                min_x: snapshot.bounds.min_x,
                min_y: snapshot.bounds.min_y,
                max_x: snapshot.bounds.max_x,
                max_y: snapshot.bounds.max_y,
            }
        } else if !state.chunk_manager.chunks().is_empty() {
            let mut min_x = f32::MAX;
            let mut min_y = f32::MAX;
            let mut max_x = f32::MIN;
            let mut max_y = f32::MIN;

            for coord in state.chunk_manager.chunks().keys() {
                let chunk_x = (coord.x * CHUNK_SIZE as i32) as f32;
                let chunk_y = (coord.y * CHUNK_SIZE as i32) as f32;
                min_x = min_x.min(chunk_x);
                min_y = min_y.min(chunk_y);
                max_x = max_x.max(chunk_x + CHUNK_SIZE as f32);
                max_y = max_y.max(chunk_y + CHUNK_SIZE as f32);
            }

            MinimapBounds {
                min_x,
                min_y,
                max_x,
                max_y,
            }
        } else if let Some(player) = state.get_local_player() {
            let radius = 24.0;
            MinimapBounds {
                min_x: player.x - radius,
                min_y: player.y - radius,
                max_x: player.x + radius,
                max_y: player.y + radius,
            }
        } else {
            return None;
        };

        if let Some(player) = state.get_local_player() {
            bounds.min_x = bounds.min_x.min(player.x);
            bounds.min_y = bounds.min_y.min(player.y);
            bounds.max_x = bounds.max_x.max(player.x);
            bounds.max_y = bounds.max_y.max(player.y);
        }

        let padding = 2.0;
        bounds.min_x -= padding;
        bounds.min_y -= padding;
        bounds.max_x += padding;
        bounds.max_y += padding;
        if bounds.max_x <= bounds.min_x {
            bounds.max_x = bounds.min_x + 1.0;
        }
        if bounds.max_y <= bounds.min_y {
            bounds.max_y = bounds.min_y + 1.0;
        }
        Some(bounds)
    }

    pub(super) fn minimap_preview_bounds(&self, state: &GameState) -> Option<MinimapBounds> {
        let player = state.get_local_player()?;
        let half_span = CHUNK_SIZE as f32 * (MINIMAP_VISIBLE_CHUNK_RADIUS + 0.5);

        Some(MinimapBounds {
            min_x: player.x - half_span,
            min_y: player.y - half_span,
            max_x: player.x + half_span,
            max_y: player.y + half_span,
        })
    }

    pub(super) fn clamp_minimap_panel_center(
        world_bounds: MinimapBounds,
        view_w: f32,
        view_h: f32,
        center_x: f32,
        center_y: f32,
    ) -> (f32, f32) {
        let half_w = view_w * 0.5;
        let half_h = view_h * 0.5;
        let min_cx = world_bounds.min_x + half_w;
        let max_cx = world_bounds.max_x - half_w;
        let min_cy = world_bounds.min_y + half_h;
        let max_cy = world_bounds.max_y - half_h;
        (
            center_x.clamp(min_cx, max_cx),
            center_y.clamp(min_cy, max_cy),
        )
    }

    pub(super) fn minimap_panel_view_bounds(
        &self,
        state: &GameState,
        world_bounds: MinimapBounds,
    ) -> MinimapBounds {
        let zoom = state
            .ui_state
            .minimap_panel_zoom
            .clamp(MINIMAP_PANEL_MIN_ZOOM, MINIMAP_PANEL_MAX_ZOOM);
        let view_w = (world_bounds.width() / zoom).clamp(1.0, world_bounds.width());
        let view_h = (world_bounds.height() / zoom).clamp(1.0, world_bounds.height());

        let default_center = state.get_local_player().map(|p| (p.x, p.y)).unwrap_or((
            (world_bounds.min_x + world_bounds.max_x) * 0.5,
            (world_bounds.min_y + world_bounds.max_y) * 0.5,
        ));
        let center_x = state
            .ui_state
            .minimap_panel_center_x
            .unwrap_or(default_center.0);
        let center_y = state
            .ui_state
            .minimap_panel_center_y
            .unwrap_or(default_center.1);
        let (center_x, center_y) =
            Self::clamp_minimap_panel_center(world_bounds, view_w, view_h, center_x, center_y);
        let half_w = view_w * 0.5;
        let half_h = view_h * 0.5;

        MinimapBounds {
            min_x: center_x - half_w,
            min_y: center_y - half_h,
            max_x: center_x + half_w,
            max_y: center_y + half_h,
        }
    }

    pub(super) fn minimap_marker_style(kind: MinimapMarkerKind) -> (Color, f32) {
        match kind {
            MinimapMarkerKind::Player => (Color::new(0.95, 0.95, 1.0, 1.0), 3.6),
            MinimapMarkerKind::Teleport => (Color::new(0.35, 0.85, 1.0, 1.0), 3.0),
            MinimapMarkerKind::Enemy => (Color::new(0.95, 0.35, 0.35, 1.0), 2.7),
            MinimapMarkerKind::Tree => (Color::new(0.35, 0.85, 0.45, 1.0), 2.4),
            MinimapMarkerKind::Quest => (Color::new(1.0, 0.82, 0.35, 1.0), 3.1),
            MinimapMarkerKind::Service => (Color::new(1.0, 0.70, 0.40, 1.0), 3.0),
            MinimapMarkerKind::Chest => (Color::new(1.0, 0.88, 0.45, 1.0), 3.0),
        }
    }

    pub(super) fn world_map_poi_marker_kind(kind: u8) -> MinimapMarkerKind {
        match kind {
            WORLD_MAP_POI_KIND_TELEPORT => MinimapMarkerKind::Teleport,
            WORLD_MAP_POI_KIND_QUEST => MinimapMarkerKind::Quest,
            WORLD_MAP_POI_KIND_SERVICE => MinimapMarkerKind::Service,
            WORLD_MAP_POI_KIND_CHEST => MinimapMarkerKind::Chest,
            WORLD_MAP_POI_KIND_TREE => MinimapMarkerKind::Tree,
            _ => MinimapMarkerKind::Tree,
        }
    }

    pub(super) fn format_map_display_name(target_map: &str) -> String {
        let raw = target_map.trim();
        if raw.is_empty() {
            return "Unknown".to_string();
        }

        // Support encoded forms such as "interior:old_house" or "maps/interiors/old_house".
        let scoped = raw.rsplit(':').next().unwrap_or(raw);
        let id = scoped.rsplit('/').next().unwrap_or(scoped).trim();
        if id.is_empty() {
            return "Unknown".to_string();
        }

        if id.eq_ignore_ascii_case("overworld") {
            return "Overworld".to_string();
        }

        let mut out = String::new();
        for (i, word) in id
            .split(['_', '-', ' '])
            .filter(|w| !w.is_empty())
            .enumerate()
        {
            if i > 0 {
                out.push(' ');
            }
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                out.push(first.to_ascii_uppercase());
                for c in chars {
                    out.push(c.to_ascii_lowercase());
                }
            }
        }

        if out.is_empty() {
            "Unknown".to_string()
        } else {
            out
        }
    }

    pub(super) fn sample_tileset_tile_color(image: &Image, tile_id: u32) -> Option<Color> {
        if tile_id == 0 {
            return None;
        }

        let id = tile_id - 1;
        let col = id % TILESET_COLUMNS;
        let row = id / TILESET_COLUMNS;
        let x0 = col * TILESET_TILE_WIDTH as u32;
        let y0 = row * TILESET_TILE_HEIGHT as u32;

        let img_w = image.width as u32;
        let img_h = image.height as u32;
        if x0 >= img_w || y0 >= img_h {
            return None;
        }

        let sample_offsets = [
            (0.50, 0.50),
            (0.50, 0.66),
            (0.36, 0.52),
            (0.64, 0.52),
            (0.50, 0.36),
        ];

        let mut sum_r = 0.0;
        let mut sum_g = 0.0;
        let mut sum_b = 0.0;
        let mut count = 0.0;

        for (fx, fy) in sample_offsets {
            let sx = (x0 as f32 + TILESET_TILE_WIDTH * fx).floor() as u32;
            let sy = (y0 as f32 + TILESET_TILE_HEIGHT * fy).floor() as u32;
            if sx >= img_w || sy >= img_h {
                continue;
            }
            let c = image.get_pixel(sx, sy);
            if c.a <= 0.05 {
                continue;
            }
            sum_r += c.r;
            sum_g += c.g;
            sum_b += c.b;
            count += 1.0;
        }

        if count <= 0.0 {
            return None;
        }

        Some(Color::new(sum_r / count, sum_g / count, sum_b / count, 1.0))
    }

    pub(super) fn is_debug_purple(color: Color) -> bool {
        // Catch the legacy debug-fallback tone (roughly rgb(100, 50, 100)) and close variants.
        color.r > color.g + 0.10
            && color.b > color.g + 0.10
            && (color.r - color.b).abs() < 0.10
            && ((color.r + color.g + color.b) / 3.0) < 0.55
    }

    pub(super) fn minimap_tile_color(&self, tile_id: u32) -> Color {
        if let Some(cached) = self
            .minimap_tile_color_cache
            .borrow()
            .get(&tile_id)
            .copied()
        {
            return cached;
        }

        if self.tileset_image_cache.borrow().is_none() {
            if let Some(tileset) = &self.tileset {
                // One-time GPU->CPU copy used for minimap color sampling.
                *self.tileset_image_cache.borrow_mut() = Some(tileset.get_texture_data());
            }
        }

        let sampled = self
            .tileset_image_cache
            .borrow()
            .as_ref()
            .and_then(|img| Self::sample_tileset_tile_color(img, tile_id));

        // Keep minimap grounded in world colors. Avoid the debug-purple fallback for unknown ids.
        let base = sampled.unwrap_or_else(|| {
            if tile_id <= 8 {
                get_tile_color(tile_id)
            } else {
                Color::from_rgba(58, 92, 64, 255)
            }
        });
        let base = if Self::is_debug_purple(base) {
            Color::from_rgba(58, 92, 64, 255)
        } else {
            base
        };

        let tuned = Color::new(
            (base.r * 0.88 + 0.03).clamp(0.0, 1.0),
            (base.g * 0.88 + 0.03).clamp(0.0, 1.0),
            (base.b * 0.88 + 0.03).clamp(0.0, 1.0),
            0.90,
        );

        self.minimap_tile_color_cache
            .borrow_mut()
            .insert(tile_id, tuned);
        tuned
    }

    pub(super) fn minimap_world_to_screen(
        &self,
        bounds: &MinimapBounds,
        map_rect: Rect,
        world_x: f32,
        world_y: f32,
    ) -> (f32, f32) {
        let nx = ((world_x - bounds.min_x) / bounds.width()).clamp(0.0, 1.0);
        let ny = ((world_y - bounds.min_y) / bounds.height()).clamp(0.0, 1.0);
        (map_rect.x + nx * map_rect.w, map_rect.y + ny * map_rect.h)
    }

    pub(super) fn collect_minimap_markers(
        &self,
        state: &GameState,
        bounds: Option<&MinimapBounds>,
        use_world_map_snapshot: bool,
    ) -> Vec<MinimapMarker> {
        let mut markers: Vec<MinimapMarker> = Vec::new();
        let player_pos = state.get_local_player().map(|p| (p.x, p.y));
        let bounds = bounds.copied();
        let bounds_margin = CHUNK_SIZE as f32 * 0.5;
        let loaded_chunk_coords: HashSet<(i32, i32)> = state
            .chunk_manager
            .chunks()
            .keys()
            .map(|coord| (coord.x, coord.y))
            .collect();

        let distance_sq = |x: f32, y: f32| -> f32 {
            if let Some((px, py)) = player_pos {
                let dx = x - px;
                let dy = y - py;
                dx * dx + dy * dy
            } else {
                0.0
            }
        };
        let in_bounds = |x: f32, y: f32| -> bool {
            if let Some(b) = bounds {
                x >= b.min_x - bounds_margin
                    && x <= b.max_x + bounds_margin
                    && y >= b.min_y - bounds_margin
                    && y <= b.max_y + bounds_margin
            } else {
                true
            }
        };
        let is_interior = state.chunk_manager.get_interior_size().is_some();
        let npc_in_loaded_chunk = |x: f32, y: f32| -> bool {
            if is_interior {
                // In interiors, all data is in chunk (0,0) regardless of world position
                loaded_chunk_coords.contains(&(0, 0))
            } else {
                let chunk_x = (x.floor() as i32).div_euclid(CHUNK_SIZE as i32);
                let chunk_y = (y.floor() as i32).div_euclid(CHUNK_SIZE as i32);
                loaded_chunk_coords.contains(&(chunk_x, chunk_y))
            }
        };

        if let Some(player) = state.get_local_player() {
            markers.push(MinimapMarker {
                kind: MinimapMarkerKind::Player,
                x: player.x,
                y: player.y,
                label: "You".to_string(),
                icon_index: 255, // Player uses dot, not icon
            });
        }

        if use_world_map_snapshot {
            if let Some(snapshot) = state.world_map_snapshot.as_ref() {
                for poi in snapshot.pois.iter().filter(|poi| in_bounds(poi.x, poi.y)) {
                    markers.push(MinimapMarker {
                        kind: Self::world_map_poi_marker_kind(poi.kind),
                        x: poi.x,
                        y: poi.y,
                        label: poi.label.clone(),
                        icon_index: poi.icon_index,
                    });
                }
            }

            let mut enemy_markers: Vec<(f32, MinimapMarker)> = Vec::new();
            for npc in state.npcs.values() {
                if !npc.is_alive() || !npc_in_loaded_chunk(npc.x, npc.y) || !in_bounds(npc.x, npc.y)
                {
                    continue;
                }
                if npc.is_hostile() {
                    enemy_markers.push((
                        distance_sq(npc.x, npc.y),
                        MinimapMarker {
                            kind: MinimapMarkerKind::Enemy,
                            x: npc.x,
                            y: npc.y,
                            label: format!("Enemy, {}", npc.display_name),
                            icon_index: 8,
                        },
                    ));
                }
            }
            enemy_markers.sort_by(|a, b| {
                a.0.partial_cmp(&b.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.1.label.cmp(&b.1.label))
            });
            const MAX_ENEMY_MARKERS: usize = 120;
            for (_, marker) in enemy_markers.into_iter().take(MAX_ENEMY_MARKERS) {
                markers.push(marker);
            }

            return markers;
        }

        let mut teleport_markers: Vec<MinimapMarker> = Vec::new();
        let interior_size = state.chunk_manager.get_interior_size();
        for (coord, chunk) in state.chunk_manager.chunks().iter() {
            let base_x = coord.x * CHUNK_SIZE as i32;
            let base_y = coord.y * CHUNK_SIZE as i32;
            if let Some(b) = bounds {
                let (chunk_min_x, chunk_min_y, chunk_max_x, chunk_max_y) =
                    if let Some((w, h)) = interior_size {
                        (0.0f32, 0.0f32, w as f32, h as f32)
                    } else {
                        let min_x = base_x as f32;
                        let min_y = base_y as f32;
                        (
                            min_x,
                            min_y,
                            min_x + CHUNK_SIZE as f32,
                            min_y + CHUNK_SIZE as f32,
                        )
                    };
                if chunk_max_x < b.min_x - bounds_margin
                    || chunk_min_x > b.max_x + bounds_margin
                    || chunk_max_y < b.min_y - bounds_margin
                    || chunk_min_y > b.max_y + bounds_margin
                {
                    continue;
                }
            }
            for portal in &chunk.portals {
                let world_x = base_x as f32 + portal.x as f32 + portal.width.max(1) as f32 * 0.5;
                let world_y = base_y as f32 + portal.y as f32 + portal.height.max(1) as f32 * 0.5;
                if !in_bounds(world_x, world_y) {
                    continue;
                }
                let target = Self::format_map_display_name(&portal.target_map);
                teleport_markers.push(MinimapMarker {
                    kind: MinimapMarkerKind::Teleport,
                    x: world_x,
                    y: world_y,
                    label: format!("Teleport, {}", target),
                    icon_index: 7,
                });
            }
        }
        teleport_markers.sort_by(|a, b| {
            a.y.partial_cmp(&b.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
        });
        markers.extend(teleport_markers);

        let mut quest_markers: Vec<MinimapMarker> = Vec::new();
        let mut enemy_markers: Vec<(f32, MinimapMarker)> = Vec::new();
        for npc in state.npcs.values() {
            if !npc.is_alive() {
                continue;
            }
            if !npc_in_loaded_chunk(npc.x, npc.y) {
                continue;
            }
            if !in_bounds(npc.x, npc.y) {
                continue;
            }
            if npc.is_banker {
                markers.push(MinimapMarker {
                    kind: MinimapMarkerKind::Service,
                    x: npc.x,
                    y: npc.y,
                    label: format!("Bank, {}", npc.display_name),
                    icon_index: 255,
                });
            } else if npc.is_altar {
                markers.push(MinimapMarker {
                    kind: MinimapMarkerKind::Service,
                    x: npc.x,
                    y: npc.y,
                    label: format!("Altar, {}", npc.display_name),
                    icon_index: 255,
                });
            } else if npc.station_type.is_some() {
                markers.push(MinimapMarker {
                    kind: MinimapMarkerKind::Service,
                    x: npc.x,
                    y: npc.y,
                    label: format!("Service, {}", npc.display_name),
                    icon_index: 255,
                });
            } else if npc.is_quest_giver {
                quest_markers.push(MinimapMarker {
                    kind: MinimapMarkerKind::Quest,
                    x: npc.x,
                    y: npc.y,
                    label: format!("Quest, {}", npc.display_name),
                    icon_index: 6,
                });
            } else if npc.is_hostile() {
                enemy_markers.push((
                    distance_sq(npc.x, npc.y),
                    MinimapMarker {
                        kind: MinimapMarkerKind::Enemy,
                        x: npc.x,
                        y: npc.y,
                        label: format!("Enemy, {}", npc.display_name),
                        icon_index: 8,
                    },
                ));
            }
        }
        quest_markers.sort_by(|a, b| a.label.cmp(&b.label));
        markers.extend(quest_markers);

        enemy_markers.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.label.cmp(&b.1.label))
        });
        const MAX_ENEMY_MARKERS: usize = 120;
        for (_, marker) in enemy_markers.into_iter().take(MAX_ENEMY_MARKERS) {
            markers.push(marker);
        }

        let mut tree_markers: Vec<(f32, MinimapMarker)> = Vec::new();
        for chunk in state.chunk_manager.chunks().values() {
            for obj in &chunk.objects {
                if state.depleted_trees.contains_key(&(obj.tile_x, obj.tile_y)) {
                    continue;
                }
                if let Some(tree_info) = get_tree_info(obj.gid) {
                    let wx = obj.tile_x as f32 + 0.5;
                    let wy = obj.tile_y as f32 + 0.5;
                    if !in_bounds(wx, wy) {
                        continue;
                    }
                    let tree_icon = match tree_info.name {
                        "Willow Tree" => 3,
                        "Maple Tree" => 4,
                        "Yew Tree" => 5,
                        _ => 1, // Oak and any unknown default to oak icon
                    };
                    tree_markers.push((
                        distance_sq(wx, wy),
                        MinimapMarker {
                            kind: MinimapMarkerKind::Tree,
                            x: wx,
                            y: wy,
                            label: format!(
                                "Tree, {} (Lv.{})",
                                tree_info.name, tree_info.level_required
                            ),
                            icon_index: tree_icon,
                        },
                    ));
                }
            }
        }
        tree_markers.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.label.cmp(&b.1.label))
        });
        const MAX_TREE_MARKERS: usize = 180;
        for (_, marker) in tree_markers.into_iter().take(MAX_TREE_MARKERS) {
            markers.push(marker);
        }

        markers
    }

    pub(super) fn declutter_minimap_markers(
        &self,
        markers: &[MinimapMarker],
        bounds: &MinimapBounds,
        map_rect: Rect,
    ) -> Vec<MinimapMarker> {
        if markers.len() < 2 {
            return markers.to_vec();
        }

        let pixels_per_world_unit = (map_rect.w / bounds.width())
            .min(map_rect.h / bounds.height())
            .max(0.01);

        let marker_spacing = |kind: MinimapMarkerKind| -> Option<f32> {
            let base = match kind {
                MinimapMarkerKind::Tree => 18.0,
                MinimapMarkerKind::Enemy => 16.0,
                MinimapMarkerKind::Chest => 14.0,
                _ => return None,
            };
            Some((base / pixels_per_world_unit.max(0.6)).clamp(1.25, base))
        };

        let marker_priority = |kind: MinimapMarkerKind| -> u8 {
            match kind {
                MinimapMarkerKind::Player => 100,
                MinimapMarkerKind::Teleport => 90,
                MinimapMarkerKind::Quest => 80,
                MinimapMarkerKind::Service => 75,
                MinimapMarkerKind::Chest => 60,
                MinimapMarkerKind::Enemy => 50,
                MinimapMarkerKind::Tree => 40,
            }
        };

        let mut order: Vec<usize> = (0..markers.len()).collect();
        order.sort_by(|a, b| {
            marker_priority(markers[*b].kind)
                .cmp(&marker_priority(markers[*a].kind))
                .then_with(|| markers[*a].label.cmp(&markers[*b].label))
                .then_with(|| a.cmp(b))
        });

        let mut kept_indices: Vec<usize> = Vec::with_capacity(markers.len());
        let mut occupied: HashMap<(MinimapMarkerKind, i32, i32), usize> = HashMap::new();

        for idx in order {
            let marker = &markers[idx];
            let Some(spacing_world) = marker_spacing(marker.kind) else {
                kept_indices.push(idx);
                continue;
            };

            let grid_x = (marker.x / spacing_world).floor() as i32;
            let grid_y = (marker.y / spacing_world).floor() as i32;
            let mut blocked = false;

            for cell_y in (grid_y - 1)..=(grid_y + 1) {
                for cell_x in (grid_x - 1)..=(grid_x + 1) {
                    if let Some(other_idx) = occupied.get(&(marker.kind, cell_x, cell_y)) {
                        let other = &markers[*other_idx];
                        let dx = other.x - marker.x;
                        let dy = other.y - marker.y;
                        if dx * dx + dy * dy <= spacing_world * spacing_world {
                            blocked = true;
                            break;
                        }
                    }
                }
                if blocked {
                    break;
                }
            }

            if blocked {
                continue;
            }

            occupied.insert((marker.kind, grid_x, grid_y), idx);
            kept_indices.push(idx);
        }

        kept_indices.sort_unstable();
        kept_indices
            .into_iter()
            .map(|idx| markers[idx].clone())
            .collect()
    }
}
