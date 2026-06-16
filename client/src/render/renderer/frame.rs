use super::*;

impl Renderer {
    pub(super) fn render_loading_screen(&self, state: &GameState) {
        let sw = screen_width();
        let sh = screen_height();

        // Determine status message based on connection state
        let status = if state.connection_status == ConnectionStatus::Disconnected {
            "Connecting"
        } else if state.local_player_id.is_none() {
            "Logging in"
        } else if state.get_local_player().is_none() {
            "Loading character"
        } else {
            "Loading world"
        };

        // Animated dots (cycles every 1s)
        let dot_count = ((get_time() * 3.0) as usize % 4) as usize;
        let dots = &"..."[..dot_count];
        let text = format!("{}{}", status, dots);

        let font_size = 32.0;
        let dims = self.measure_text_sharp(&text, font_size);
        let x = ((sw - dims.width) / 2.0).floor();
        let y = ((sh) / 2.0).floor();

        self.draw_text_sharp(&text, x, y, font_size, Color::from_rgba(200, 200, 200, 255));
    }

    pub fn render(&self, state: &GameState) -> (UiLayout, RenderTimings) {
        let render_start = get_time();
        let mut timings = RenderTimings::default();

        // Reset font scale to 1.0 for world rendering (player names, damage, etc.)
        self.font_scale.set(1.0);

        // 1. Render ground layer tiles
        let t0 = get_time();
        self.render_tilemap_layer(state, LayerType::Ground);

        // (Drop zone highlights are now rendered in the depth-sorted pass)
        // (Farming patches are now rendered in the depth-sorted pass)

        // 1.8. Render gathering marker overlays (fishing spots, etc.)
        self.render_gathering_markers(state);

        // 1.9. Render AOE warning zones and explosion effects (boss fight)
        self.render_aoe_warnings(state);
        self.render_explosions(state);

        // // DEBUG: Render NPC occupied tile footprints for multi-tile NPCs
        // self.render_npc_debug_tiles(state);

        timings.ground_ms = (get_time() - t0) * 1000.0;

        // Skip entity/world rendering until world is ready
        let world_ready = state.is_world_ready();

        // 2. Collect renderable items (players + NPCs + items + object tiles + map objects) for depth sorting
        let t1 = get_time();
        #[derive(Clone)]
        enum Renderable<'a> {
            Player(&'a Player, bool),
            Npc(&'a Npc),
            Item(&'a GroundItem),
            Tile {
                x: u32,
                y: u32,
                tile_id: u32,
            },
            /// Tile hover highlight - depth-sorted between elevated tiles and entities
            TileHover {
                tile_x: i32,
                tile_y: i32,
                tile_z: i32,
            },
            /// Destination tile dim overlay for auto-path target
            DestinationTile {
                tile_x: i32,
                tile_y: i32,
                tile_z: i32,
            },
            /// Drop zone highlight - depth-sorted to render on top of elevated blocks
            DropZone {
                tile_x: i32,
                tile_y: i32,
                tile_z: i32,
                is_hovered: bool,
            },
            /// Elevated ground tile (z > 0) - depth-sorted with entities for proper occlusion
            ElevatedTile {
                screen_x: f32,
                screen_y: f32,
                tile_id: u32,
                height: u8,
                local_x: i32,
                local_y: i32,
                chunk_coord: crate::game::ChunkCoord,
            },
            /// Block side face - each face (+X right, +Y down) is pushed as a
            /// separate renderable with direction-aware depth so that entities
            /// in front sort correctly while lower tiles don't clip the cliff.
            BlockSide {
                screen_x: f32,
                screen_y: f32,
                height: u8,
                block_type_down: u16,
                block_type_right: u16,
                skip_right: bool,
                skip_down: bool,
                local_x: i32,
                local_y: i32,
                chunk_coord: crate::game::ChunkCoord,
            },
            ChunkObject(&'a MapObject, f32), // Object with tile_z
            ChunkObjectShaking(&'a MapObject, f32, f32), // Object with shake offset and tile_z
            ChunkWall(&'a Wall, f32),        // Wall with tile_z
            TreeTimer {
                tile_x: i32,
                tile_y: i32,
                tile_z: f32,
                progress: f32,
            },
            FallingTree {
                gid: u32,
                tile_x: i32,
                tile_y: i32,
                tile_z: f32,
                angle: f32,
                alpha: f32,
                y_offset: f32,
            },
            CrumblingRock {
                gid: u32,
                tile_x: i32,
                tile_y: i32,
                tile_z: f32,
                scale: f32,
                alpha: f32,
            },
            RockTimer {
                tile_x: i32,
                tile_y: i32,
                tile_z: f32,
                progress: f32,
            },
            SpellEffect {
                effect_idx: usize,
            },
            FarmingPatch {
                patch_id: &'a str,
            },
        }

        // Pre-allocate with estimated capacity to reduce allocations
        let chunk_object_estimate: usize = state
            .chunk_manager
            .chunks()
            .values()
            .map(|c| c.objects.len() + c.walls.len())
            .sum();
        let estimated_capacity = state.players.len()
            + state.npcs.len()
            + state.ground_items.len()
            + chunk_object_estimate
            + 100;
        let mut renderables: Vec<(f32, Renderable)> = Vec::with_capacity(estimated_capacity);

        // Only collect world entities when world is ready
        if !world_ready {
            // Show loading screen instead of empty world
            self.render_loading_screen(state);

            timings.entities_ms = (get_time() - t1) * 1000.0;

            // 8. Render UI (non-interactive elements) — skip in spectator mode
            let t4 = get_time();
            if !state.spectator_mode {
                self.font_scale.set(state.ui_state.ui_scale);
                self.render_ui(state);
            }

            // 9. Render interactive UI elements and return layout for hit detection
            let layout = if state.spectator_mode {
                UiLayout::default()
            } else {
                self.render_interactive_ui(state)
            };

            // 10. Render XP drops above interactive UI overlays
            if !state.spectator_mode {
                self.render_deferred_xp_drops(state);
            }
            timings.ui_ms = (get_time() - t4) * 1000.0;

            timings.total_ms = (get_time() - render_start) * 1000.0;
            return (layout, timings);
        }

        // Compute visible world-space AABB from screen corners (avoids per-object world_to_screen)
        let (cull_screen_w, cull_screen_h) = virtual_screen_size();
        let corners_world = [
            screen_to_world(0.0, 0.0, &state.camera),
            screen_to_world(cull_screen_w, 0.0, &state.camera),
            screen_to_world(0.0, cull_screen_h, &state.camera),
            screen_to_world(cull_screen_w, cull_screen_h, &state.camera),
        ];
        // Margin in world tiles for tall objects and edge effects
        let world_cull_margin = 8.0;
        let vis_min_x =
            corners_world.iter().map(|c| c.0).fold(f32::MAX, f32::min) - world_cull_margin;
        let vis_max_x =
            corners_world.iter().map(|c| c.0).fold(f32::MIN, f32::max) + world_cull_margin;
        let vis_min_y =
            corners_world.iter().map(|c| c.1).fold(f32::MAX, f32::min) - world_cull_margin;
        let vis_max_y =
            corners_world.iter().map(|c| c.1).fold(f32::MIN, f32::max) + world_cull_margin;
        let is_visible_world = |wx: f32, wy: f32| {
            wx >= vis_min_x && wx <= vis_max_x && wy >= vis_min_y && wy <= vis_max_y
        };

        // Add ground items (depth-sorted with entities, accounting for elevation)
        for item in state.ground_items.values() {
            if !is_visible_world(item.x, item.y) {
                continue;
            }
            let item_z = state
                .chunk_manager
                .get_height(item.x.round() as i32, item.y.round() as i32)
                as f32;
            let depth = calculate_depth_z(item.x, item.y, item_z, 1) + 0.01;
            renderables.push((depth, Renderable::Item(item)));
        }

        // Add farming patches (depth-sorted with entities)
        if state.current_interior.is_none() {
            for (id, patch) in &state.farming_patches {
                // Sort multi-tile patches by their centered footprint position (like NPCs).
                let wx = patch.x as f32 + (patch.width.max(1) as f32 - 1.0) * 0.5;
                let wy = patch.y as f32 + (patch.height.max(1) as f32 - 1.0) * 0.5;
                if !is_visible_world(wx, wy) {
                    continue;
                }
                let patch_z = state.chunk_manager.get_height(patch.x, patch.y) as f32;
                let depth = calculate_depth_z(wx, wy, patch_z, 1) + 0.01;
                renderables.push((
                    depth,
                    Renderable::FarmingPatch {
                        patch_id: id.as_str(),
                    },
                ));
            }
        }

        // Add elevated ground tiles (z > 0) for depth-sorted rendering with entities
        {
            let zoom = state.camera.zoom;
            let dx_step = (TILE_WIDTH / 2.0) * zoom;
            let dy_step = (TILE_HEIGHT / 2.0) * zoom;
            for (coord, chunk) in state.chunk_manager.chunks().iter() {
                if chunk.heights.is_none() {
                    continue;
                }
                let chunk_offset_x = coord.x * CHUNK_SIZE as i32;
                let chunk_offset_y = coord.y * CHUNK_SIZE as i32;
                let (base_sx, base_sy) = world_to_screen_exact(
                    chunk_offset_x as f32,
                    chunk_offset_y as f32,
                    &state.camera,
                );
                for local_y in 0..CHUNK_SIZE as i32 {
                    for local_x in 0..CHUNK_SIZE as i32 {
                        let h = chunk.get_height(local_x as u32, local_y as u32);
                        if h == 0 {
                            continue;
                        }
                        let wx = (chunk_offset_x + local_x) as f32;
                        let wy = (chunk_offset_y + local_y) as f32;
                        if !is_visible_world(wx, wy) {
                            continue;
                        }
                        let screen_x =
                            (base_sx + local_x as f32 * dx_step - local_y as f32 * dx_step).round();
                        let screen_y =
                            (base_sy + local_x as f32 * dy_step + local_y as f32 * dy_step
                                - h as f32 * (TILE_HEIGHT / 2.0) * zoom)
                                .round();

                        let tile_id = {
                            let idx = (local_y as u32 * CHUNK_SIZE + local_x as u32) as usize;
                            let base_id = chunk
                                .layers
                                .iter()
                                .find(|l| l.layer_type == ChunkLayerType::Ground)
                                .and_then(|l| l.tiles.get(idx).copied())
                                .unwrap_or(0);
                            state
                                .ground_tile_overrides
                                .get(&(chunk_offset_x + local_x, chunk_offset_y + local_y))
                                .copied()
                                .unwrap_or(base_id)
                        };
                        if tile_id == 0 {
                            continue;
                        }
                        // Depth: same x+y plane as entities, but use z for proper ordering
                        let depth = calculate_depth_z(wx, wy, h as f32, 1);
                        renderables.push((
                            depth,
                            Renderable::ElevatedTile {
                                screen_x,
                                screen_y,
                                tile_id,
                                height: h,
                                local_x,
                                local_y,
                                chunk_coord: *coord,
                            },
                        ));
                        // Block sides extend downward from the tile surface.
                        // Each face is pushed separately with depth based on its
                        // neighbor's height: sort just BELOW the neighbor's tile
                        // surface (-0.12) so the neighbor covers the face's bottom
                        // edge, while the face stays above things further behind.
                        let bt_down = chunk.get_block_type_down(local_x as u32, local_y as u32);
                        let bt_right = chunk.get_block_type_right(local_x as u32, local_y as u32);

                        // Helper to get neighbor height (handles chunk boundaries)
                        let get_nh = |nx: i32, ny: i32| -> u8 {
                            if nx >= 0
                                && nx < CHUNK_SIZE as i32
                                && ny >= 0
                                && ny < CHUNK_SIZE as i32
                            {
                                chunk.get_height(nx as u32, ny as u32)
                            } else {
                                let nwx = chunk_offset_x + nx;
                                let nwy = chunk_offset_y + ny;
                                let nc = crate::game::ChunkCoord::from_world(nwx, nwy);
                                state
                                    .chunk_manager
                                    .chunks()
                                    .get(&nc)
                                    .map(|c| {
                                        let (lx, ly) = crate::game::chunk::world_to_local(nwx, nwy);
                                        c.get_height(lx, ly)
                                    })
                                    .unwrap_or(0)
                            }
                        };

                        // Right (+X) face: sits between (x,y) and (x+1,y)
                        let right_nh = get_nh(local_x + 1, local_y);
                        if h > right_nh {
                            let rd = calculate_depth_z(wx + 1.0, wy, right_nh as f32, 1) - 0.12;
                            renderables.push((
                                rd,
                                Renderable::BlockSide {
                                    screen_x,
                                    screen_y,
                                    height: h,
                                    block_type_down: bt_down,
                                    block_type_right: bt_right,
                                    skip_right: false,
                                    skip_down: true,
                                    local_x,
                                    local_y,
                                    chunk_coord: *coord,
                                },
                            ));
                        }

                        // Down (+Y) face: sits between (x,y) and (x,y+1)
                        let down_nh = get_nh(local_x, local_y + 1);
                        if h > down_nh {
                            let dd = calculate_depth_z(wx, wy + 1.0, down_nh as f32, 1) - 0.12;
                            renderables.push((
                                dd,
                                Renderable::BlockSide {
                                    screen_x,
                                    screen_y,
                                    height: h,
                                    block_type_down: bt_down,
                                    block_type_right: bt_right,
                                    skip_right: true,
                                    skip_down: false,
                                    local_x,
                                    local_y,
                                    chunk_coord: *coord,
                                },
                            ));
                        }
                    }
                }
            }
        }

        // Add players
        for player in state.players.values() {
            if !is_visible_world(player.x, player.y) {
                continue;
            }
            let is_local = state.local_player_id.as_ref() == Some(&player.id);
            // Use ceil() on the interpolated position for depth so that during
            // movement the player sorts at the higher of the two tiles they're
            // between. This prevents both source and destination tiles from
            // rendering on top of the player mid-step. Max with target_depth
            // handles the forward-movement case as an extra safety net.
            // When descending away from camera (-x or -y), sort behind the
            // cliff edge. When descending toward camera (+x or +y), keep
            // normal depth so the player stays in front of the edge face.
            let descending_away = player.target_z < player.z
                && player.target_x <= player.x.floor()
                && player.target_y <= player.y.floor();
            let mut depth = if descending_away {
                // Use visual Z so depth decreases gradually as player falls,
                // rather than instantly dropping to ground-level depth
                calculate_depth_z(player.x.floor(), player.y.floor(), player.z, 1) - 0.02
            } else {
                let ceil_depth = calculate_depth_z(player.x.ceil(), player.y.ceil(), player.z, 1);
                let target_depth =
                    calculate_depth_z(player.target_x, player.target_y, player.target_z, 1);
                ceil_depth.max(target_depth) + 0.25
            };
            // Sitting players render on top of the chair object at the same tile
            if player.animation.state == crate::render::animation::AnimationState::SittingChair {
                depth += 0.5;
            }
            renderables.push((depth, Renderable::Player(player, is_local)));
        }

        // Add NPCs
        for npc in state.npcs.values() {
            let center_offset = (npc.size - 1) as f32 * 0.5;
            let cx = npc.x + center_offset;
            let cy = npc.y + center_offset;
            if !is_visible_world(cx, cy) {
                continue;
            }
            let descending_away = npc.target_z < npc.z
                && npc.target_x <= npc.x.floor()
                && npc.target_y <= npc.y.floor();
            let depth = if descending_away {
                calculate_depth_z(cx.floor(), cy.floor(), npc.z, 1) - 0.02
            } else {
                let ceil_depth = calculate_depth_z(cx.ceil(), cy.ceil(), npc.z, 1);
                let target_depth = calculate_depth_z(npc.target_x, npc.target_y, npc.target_z, 1);
                ceil_depth.max(target_depth) + 0.25
            };
            renderables.push((depth, Renderable::Npc(npc)));
        }

        // Add spell effects as depth-sorted renderables
        {
            let current_time = macroquad::time::get_time();
            for (idx, effect) in state.spell_effects.iter().enumerate() {
                let elapsed = current_time - effect.time;
                let sprite_name = match effect.spell_id.as_str() {
                    "dark_hand" => "dark_hand",
                    "lightning_bolt" => "lightning_bolt",
                    "dark_eater" => "dark_eater",
                    "rock_fall" => "rock_fall",
                    "heal" => "self_heal",
                    "teleport" | "return_home" => "bubbles_warp",
                    "tornado" => "tornado",
                    "rocks_aoe" => "rocks_aoe",
                    _ => continue,
                };
                let frame_count = match sprite_name {
                    "rocks_aoe" => 8usize,
                    _ => 5usize,
                };
                let fps = 10.0_f64;
                let total_duration = frame_count as f64 / fps;
                if elapsed > total_duration {
                    continue;
                }
                let ex = effect.target_x as f32;
                let ey = effect.target_y as f32;
                if !is_visible_world(ex, ey) {
                    continue;
                }
                let depth = calculate_depth(ex, ey, 1) + 0.25;
                renderables.push((depth, Renderable::SpellEffect { effect_idx: idx }));
            }
        }

        // Add legacy object-layer tiles only when chunk data is unavailable.
        // In streamed worlds, chunk objects/walls are the source of truth.
        if state.chunk_manager.chunks().is_empty() {
            for layer in &state.tilemap.layers {
                if layer.layer_type == LayerType::Objects {
                    for y in 0..state.tilemap.height {
                        for x in 0..state.tilemap.width {
                            let wx = x as f32;
                            let wy = y as f32;
                            if wx < vis_min_x || wx > vis_max_x || wy < vis_min_y || wy > vis_max_y
                            {
                                continue;
                            }
                            let idx = (y * state.tilemap.width + x) as usize;
                            let tile_id = layer.tiles.get(idx).copied().unwrap_or(0);
                            if tile_id > 0 {
                                let depth = calculate_depth(wx, wy, 1);
                                renderables.push((depth, Renderable::Tile { x, y, tile_id }));
                            }
                        }
                    }
                }
            }
        }

        // Reuse struct-level lookup tables for tree/rock effects (clear + rebuild avoids allocation)
        {
            let mut ftp = self.falling_tree_positions.borrow_mut();
            ftp.clear();
            ftp.extend(state.falling_trees.iter().map(|ft| (ft.x, ft.y)));
        }
        {
            let mut tso = self.tree_shake_offsets.borrow_mut();
            tso.clear();
            tso.extend(
                state
                    .tree_shake_effects
                    .iter()
                    .map(|shake| ((shake.x, shake.y), shake.get_offset())),
            );
        }
        {
            let mut crp = self.crumbling_rock_positions.borrow_mut();
            crp.clear();
            crp.extend(state.crumbling_rocks.iter().map(|cr| (cr.x, cr.y)));
        }
        {
            let mut rso = self.rock_shake_offsets.borrow_mut();
            rso.clear();
            rso.extend(
                state
                    .rock_shake_effects
                    .iter()
                    .map(|shake| ((shake.x, shake.y), shake.get_offset())),
            );
        }
        let falling_tree_positions = self.falling_tree_positions.borrow();
        let tree_shake_offsets = self.tree_shake_offsets.borrow();
        let crumbling_rock_positions = self.crumbling_rock_positions.borrow();
        let rock_shake_offsets = self.rock_shake_offsets.borrow();

        // Add map objects and walls from loaded chunks with chunk-level pre-culling
        let interior_dims = state.chunk_manager.get_interior_size();
        let chunk_size = CHUNK_SIZE as f32;
        for (coord, chunk) in state.chunk_manager.chunks().iter() {
            // Chunk-level AABB check: skip entire chunk if outside visible area
            // For interiors, the single chunk at (0,0) covers the full map dimensions
            let (chunk_min_x, chunk_min_y, chunk_max_x, chunk_max_y) =
                if let Some((w, h)) = interior_dims {
                    (0.0, 0.0, w as f32, h as f32)
                } else {
                    let min_x = (coord.x * CHUNK_SIZE as i32) as f32;
                    let min_y = (coord.y * CHUNK_SIZE as i32) as f32;
                    (min_x, min_y, min_x + chunk_size, min_y + chunk_size)
                };
            if chunk_max_x < vis_min_x
                || chunk_min_x > vis_max_x
                || chunk_max_y < vis_min_y
                || chunk_min_y > vis_max_y
            {
                continue;
            }

            for obj in &chunk.objects {
                let wx = obj.tile_x as f32;
                let wy = obj.tile_y as f32;
                if wx < vis_min_x || wx > vis_max_x || wy < vis_min_y || wy > vis_max_y {
                    continue;
                }
                // Skip depleted trees (they're hidden until respawn)
                if state.depleted_trees.contains_key(&(obj.tile_x, obj.tile_y)) {
                    continue;
                }
                // Skip trees that are currently falling (we render them with the fall animation)
                if falling_tree_positions.contains(&(obj.tile_x, obj.tile_y)) {
                    continue;
                }
                // Skip depleted rocks (they're hidden until respawn)
                if state.depleted_rocks.contains_key(&(obj.tile_x, obj.tile_y)) {
                    continue;
                }
                // Skip rocks that are currently crumbling
                if crumbling_rock_positions.contains(&(obj.tile_x, obj.tile_y)) {
                    continue;
                }
                let (lx, ly) = crate::game::chunk::world_to_local(obj.tile_x, obj.tile_y);
                let tile_z = chunk.get_height(lx, ly) as f32;
                let depth = calculate_depth_z(wx, wy, tile_z, 1);
                // Check if object is shaking (tree or rock) and apply offset
                let tree_shake = tree_shake_offsets.get(&(obj.tile_x, obj.tile_y)).copied();
                let rock_shake = rock_shake_offsets.get(&(obj.tile_x, obj.tile_y)).copied();
                if let Some(offset) = tree_shake.or(rock_shake) {
                    renderables.push((depth, Renderable::ChunkObjectShaking(obj, offset, tile_z)));
                } else {
                    renderables.push((depth, Renderable::ChunkObject(obj, tile_z)));
                }
            }
            for wall in &chunk.walls {
                let wx = wall.tile_x as f32;
                let wy = wall.tile_y as f32;
                if wx < vis_min_x || wx > vis_max_x || wy < vis_min_y || wy > vis_max_y {
                    continue;
                }
                let (lx, ly) = crate::game::chunk::world_to_local(wall.tile_x, wall.tile_y);
                let tile_z = chunk.get_height(lx, ly) as f32;
                // Walls are tall sprites extending upward from the tile surface.
                // Add a small depth boost so they sort above elevated tiles at the
                // same effective depth, but still below entities (+0.25).
                let depth = calculate_depth_z(wx, wy, tile_z, 1) + 0.2;
                renderables.push((depth, Renderable::ChunkWall(wall, tile_z)));
            }
        }

        // Add depleted tree respawn timers (depth-sorted with other objects)
        let current_time = macroquad::time::get_time();
        for ((tile_x, tile_y), info) in &state.depleted_trees {
            let wx = *tile_x as f32;
            let wy = *tile_y as f32;
            if wx < vis_min_x || wx > vis_max_x || wy < vis_min_y || wy > vis_max_y {
                continue;
            }
            let total_duration = info.respawn_at - info.depleted_at;
            if total_duration <= 0.0 {
                continue;
            }
            let elapsed = current_time - info.depleted_at;
            let progress = (elapsed / total_duration).clamp(0.0, 1.0) as f32;
            let tile_z = state.chunk_manager.get_height(*tile_x, *tile_y) as f32;
            let depth = calculate_depth_z(wx, wy, tile_z, 1);
            renderables.push((
                depth,
                Renderable::TreeTimer {
                    tile_x: *tile_x,
                    tile_y: *tile_y,
                    tile_z,
                    progress,
                },
            ));
        }

        // Add depleted rock respawn timers
        for ((tile_x, tile_y), info) in &state.depleted_rocks {
            let wx = *tile_x as f32;
            let wy = *tile_y as f32;
            if wx < vis_min_x || wx > vis_max_x || wy < vis_min_y || wy > vis_max_y {
                continue;
            }
            let total_duration = info.respawn_at - info.depleted_at;
            if total_duration <= 0.0 {
                continue;
            }
            let elapsed = current_time - info.depleted_at;
            let progress = (elapsed / total_duration).clamp(0.0, 1.0) as f32;
            let tile_z = state.chunk_manager.get_height(*tile_x, *tile_y) as f32;
            let depth = calculate_depth_z(wx, wy, tile_z, 1);
            renderables.push((
                depth,
                Renderable::RockTimer {
                    tile_x: *tile_x,
                    tile_y: *tile_y,
                    tile_z,
                    progress,
                },
            ));
        }

        // Add falling trees (trees that were just chopped down)
        for ft in &state.falling_trees {
            let wx = ft.x as f32;
            let wy = ft.y as f32;
            if wx < vis_min_x || wx > vis_max_x || wy < vis_min_y || wy > vis_max_y {
                continue;
            }
            let (angle, alpha, y_offset) = ft.get_transform();
            let tile_z = state.chunk_manager.get_height(ft.x, ft.y) as f32;
            let depth = calculate_depth_z(wx, wy, tile_z, 1);
            renderables.push((
                depth,
                Renderable::FallingTree {
                    gid: ft.gid,
                    tile_x: ft.x,
                    tile_y: ft.y,
                    tile_z,
                    angle,
                    alpha,
                    y_offset,
                },
            ));
        }

        // Add crumbling rocks
        for cr in &state.crumbling_rocks {
            let wx = cr.x as f32;
            let wy = cr.y as f32;
            if wx < vis_min_x || wx > vis_max_x || wy < vis_min_y || wy > vis_max_y {
                continue;
            }
            let (scale, alpha) = cr.get_transform();
            let tile_z = state.chunk_manager.get_height(cr.x, cr.y) as f32;
            let depth = calculate_depth_z(wx, wy, tile_z, 1);
            renderables.push((
                depth,
                Renderable::CrumblingRock {
                    gid: cr.gid,
                    tile_x: cr.x,
                    tile_y: cr.y,
                    tile_z,
                    scale,
                    alpha,
                },
            ));
        }

        // Add drop zone highlights as depth-sorted renderables (draws above blocks, below entities)
        if let Some(ref drag) = state.ui_state.drag_state {
            if matches!(drag.source, DragSource::Inventory(_)) {
                if let Some(player) = state.get_local_player() {
                    let player_x = player.x.round() as i32;
                    let player_y = player.y.round() as i32;

                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let tile_x = player_x + dx;
                            let tile_y = player_y + dy;
                            let tile_z = state.chunk_manager.get_height(tile_x, tile_y) as i32;
                            let is_hovered = state.hovered_tile == Some((tile_x, tile_y));
                            let depth =
                                calculate_depth_z(tile_x as f32, tile_y as f32, tile_z as f32, 1)
                                    + 0.02;
                            renderables.push((
                                depth,
                                Renderable::DropZone {
                                    tile_x,
                                    tile_y,
                                    tile_z,
                                    is_hovered,
                                },
                            ));
                        }
                    }
                }

                // When dragging a patch-targeting item (seed/compost/cure) over a plot it
                // can be used on — and standing next to that plot — light up the WHOLE
                // footprint as droppable.
                let hovered_patch = state
                    .hovered_tile
                    .and_then(|h| state.farming_patch_positions.get(&h))
                    .and_then(|id| state.farming_patches.get(id));
                if let (Some(patch), Some(player)) = (hovered_patch, state.get_local_player()) {
                    let px = player.x.round() as i32;
                    let py = player.y.round() as i32;
                    let cx = px.clamp(patch.x, patch.x + patch.width.max(1) as i32 - 1);
                    let cy = py.clamp(patch.y, patch.y + patch.height.max(1) as i32 - 1);
                    let adjacent = (px - cx).abs() <= 1 && (py - cy).abs() <= 1;
                    let item_id = drag.item_id.as_str();
                    let valid = adjacent
                        && if item_id.ends_with("_seed") {
                        patch.state == "empty"
                    } else if item_id == "compost" {
                        !patch.composted
                            && matches!(patch.state.as_str(), "empty" | "growing" | "harvestable")
                    } else if item_id == "plant_cure_potion" {
                        patch.state == "diseased"
                    } else {
                        false
                    };
                    if valid {
                        for dy in 0..patch.height.max(1) as i32 {
                            for dx in 0..patch.width.max(1) as i32 {
                                let tile_x = patch.x + dx;
                                let tile_y = patch.y + dy;
                                let tile_z = state.chunk_manager.get_height(tile_x, tile_y) as i32;
                                let depth = calculate_depth_z(
                                    tile_x as f32,
                                    tile_y as f32,
                                    tile_z as f32,
                                    1,
                                ) + 0.02;
                                // The whole bed reads as droppable (faint green); the tile
                                // directly under the cursor gets the bright hover highlight.
                                let is_hovered = state.hovered_tile == Some((tile_x, tile_y));
                                renderables.push((
                                    depth,
                                    Renderable::DropZone {
                                        tile_x,
                                        tile_y,
                                        tile_z,
                                        is_hovered,
                                    },
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Add tile hover highlight as a depth-sorted renderable (draws above tile surface, below objects/entities)
        if let Some((tile_x, tile_y)) = state.hovered_tile {
            let z = state.hovered_tile_z;
            // Elevated tiles at +0.0, objects at +0.0 (but pushed after tiles so stable sort
            // puts them on top), entities at +0.25. Use -0.01 to render just below objects.
            let depth = calculate_depth_z(tile_x as f32, tile_y as f32, z as f32, 1) - 0.01;
            renderables.push((
                depth,
                Renderable::TileHover {
                    tile_x,
                    tile_y,
                    tile_z: z,
                },
            ));
        }

        // Add destination tile highlight for active auto-path
        if let Some(ref path_state) = state.auto_path {
            let (dest_x, dest_y) = path_state.destination;
            let z = state.chunk_manager.get_height(dest_x, dest_y) as i32;
            let depth = calculate_depth_z(dest_x as f32, dest_y as f32, z as f32, 1) - 0.01;
            renderables.push((
                depth,
                Renderable::DestinationTile {
                    tile_x: dest_x,
                    tile_y: dest_y,
                    tile_z: z,
                },
            ));
        }

        // Sort by depth (painter's algorithm)
        // Must use stable sort: items at the same depth (e.g. walls on tiles
        // with equal x+y) must keep a consistent order to avoid flickering.
        renderables.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // 3. Render sorted entities
        for (_, renderable) in renderables {
            match renderable {
                Renderable::TileHover {
                    tile_x,
                    tile_y,
                    tile_z,
                } => {
                    self.render_tile_hover(tile_x, tile_y, tile_z, &state.camera);
                }
                Renderable::DestinationTile {
                    tile_x,
                    tile_y,
                    tile_z,
                } => {
                    self.render_destination_tile(tile_x, tile_y, tile_z, &state.camera);
                }
                Renderable::DropZone {
                    tile_x,
                    tile_y,
                    tile_z,
                    is_hovered,
                } => {
                    self.render_drop_zone(tile_x, tile_y, tile_z, &state.camera, is_hovered);
                }
                Renderable::Item(item) => {
                    self.render_ground_item(item, &state.camera, state);
                }
                Renderable::Player(player, is_local) => {
                    let is_selected = state.selected_entity_id.as_ref() == Some(&player.id);
                    let is_hovered = state.hovered_entity_id.as_ref() == Some(&player.id);
                    // Interpolate ground height at player position for smooth
                    // shadow movement during height transitions
                    let ground_z = {
                        let chunks = state.chunk_manager.chunks();
                        let get_h = |wx: i32, wy: i32| -> f32 {
                            let c = crate::game::ChunkCoord::from_world(wx, wy);
                            let (lx, ly) = crate::game::chunk::world_to_local(wx, wy);
                            chunks
                                .get(&c)
                                .map(|ch| ch.get_height(lx, ly) as f32)
                                .unwrap_or(0.0)
                        };
                        let px = player.x.floor() as i32;
                        let py = player.y.floor() as i32;
                        let fx = player.x - player.x.floor();
                        let fy = player.y - player.y.floor();
                        let h00 = get_h(px, py);
                        let h10 = get_h(px + 1, py);
                        let h01 = get_h(px, py + 1);
                        let h11 = get_h(px + 1, py + 1);
                        h00 * (1.0 - fx) * (1.0 - fy)
                            + h10 * fx * (1.0 - fy)
                            + h01 * (1.0 - fx) * fy
                            + h11 * fx * fy
                    };
                    self.render_player(
                        player,
                        is_local,
                        is_selected,
                        is_hovered,
                        &state.camera,
                        &state.item_registry,
                        ground_z,
                    );
                }
                Renderable::Npc(npc) => {
                    let is_selected = state.selected_entity_id.as_ref() == Some(&npc.id);
                    let is_hovered = state.hovered_entity_id.as_ref() == Some(&npc.id);
                    self.render_npc(npc, is_selected, is_hovered, &state.camera);
                }
                Renderable::Tile { x, y, tile_id } => {
                    let (screen_x, screen_y) = world_to_screen(x as f32, y as f32, &state.camera);
                    self.draw_isometric_object(screen_x, screen_y, tile_id, state.camera.zoom);
                }
                Renderable::ElevatedTile {
                    screen_x,
                    screen_y,
                    tile_id,
                    height,
                    local_x,
                    local_y,
                    chunk_coord,
                } => {
                    let zoom = state.camera.zoom;
                    // Tint based on height for depth perception
                    let mut brightness = 1.0 + height as f32 * 0.06;

                    // Ambient occlusion: darken when a neighbor behind (-X/-Y) is taller
                    if let Some(chunk) = state.chunk_manager.chunks().get(&chunk_coord) {
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
                        let max_diff = (h_nx as i32 - height as i32)
                            .max(h_ny as i32 - height as i32)
                            .max(0);
                        if max_diff > 0 {
                            brightness -= max_diff as f32 * 0.15;
                            brightness = brightness.max(0.5);
                        }
                    }

                    let c = (brightness * 255.0).min(255.0) as u8;
                    let tint = Color::from_rgba(c, c, c, 255);
                    self.draw_tile_sprite_tinted(
                        screen_x,
                        screen_y,
                        tile_id,
                        zoom,
                        Some((
                            (chunk_coord.x * CHUNK_SIZE as i32 + local_x) as f32,
                            (chunk_coord.y * CHUNK_SIZE as i32 + local_y) as f32,
                        )),
                        false,
                        tint,
                    );
                    // Side faces are drawn separately via BlockSide renderable
                    // at a lower depth to prevent occluding same-height entities.
                }
                Renderable::BlockSide {
                    screen_x,
                    screen_y,
                    height,
                    block_type_down,
                    block_type_right,
                    skip_right,
                    skip_down,
                    local_x,
                    local_y,
                    chunk_coord,
                } => {
                    let zoom = state.camera.zoom;
                    if let Some(chunk) = state.chunk_manager.chunks().get(&chunk_coord) {
                        self.draw_block_sides(
                            chunk,
                            local_x,
                            local_y,
                            height,
                            block_type_down,
                            block_type_right,
                            screen_x,
                            screen_y,
                            zoom,
                            state,
                            &chunk_coord,
                            skip_right,
                            skip_down,
                        );
                    }
                }
                Renderable::ChunkObject(obj, tile_z) => {
                    self.render_map_object(obj, tile_z, &state.camera);
                }
                Renderable::ChunkObjectShaking(obj, offset, tile_z) => {
                    self.render_map_object_shaking(obj, offset, tile_z, &state.camera);
                }
                Renderable::ChunkWall(wall, tile_z) => {
                    self.render_wall(wall, tile_z, &state.camera);
                }
                Renderable::TreeTimer {
                    tile_x,
                    tile_y,
                    tile_z,
                    progress,
                } => {
                    self.render_tree_timer(tile_x, tile_y, tile_z, progress, &state.camera);
                }
                Renderable::FallingTree {
                    gid,
                    tile_x,
                    tile_y,
                    tile_z,
                    angle,
                    alpha,
                    y_offset,
                } => {
                    self.render_falling_tree(
                        gid,
                        tile_x,
                        tile_y,
                        tile_z,
                        angle,
                        alpha,
                        y_offset,
                        &state.camera,
                    );
                }
                Renderable::CrumblingRock {
                    gid,
                    tile_x,
                    tile_y,
                    tile_z,
                    scale,
                    alpha,
                } => {
                    self.render_crumbling_rock(
                        gid,
                        tile_x,
                        tile_y,
                        tile_z,
                        scale,
                        alpha,
                        &state.camera,
                    );
                }
                Renderable::RockTimer {
                    tile_x,
                    tile_y,
                    tile_z,
                    progress,
                } => {
                    // Reuse tree timer rendering — same pie chart style
                    self.render_tree_timer(tile_x, tile_y, tile_z, progress, &state.camera);
                }
                Renderable::SpellEffect { effect_idx } => {
                    self.render_single_spell_effect(state, effect_idx);
                }
                Renderable::FarmingPatch { patch_id } => {
                    self.render_single_farming_patch(state, patch_id);
                }
            }
        }

        // Render fishing lines on top of all world objects (walls, piers, etc.)
        for player in state.players.values() {
            if player.is_gathering {
                let elapsed = macroquad::time::get_time() - player.gathering_started_at;
                if elapsed > 0.2 {
                    self.render_fishing_line(player, &state.camera);
                }
            }
        }

        // Render leaf particles (world-space, after depth-sorted objects)
        // Skip all particles when graphics_low to save draw calls and trig on mobile
        if !state.ui_state.graphics_low {
            for leaf in &state.leaf_particles {
                if !is_visible_world(leaf.tile_x, leaf.tile_y) {
                    continue;
                }

                // Convert tile coords to screen coords
                let (screen_x, base_screen_y) =
                    world_to_screen(leaf.tile_x, leaf.tile_y, &state.camera);

                // Offset upward by height (height is in unscaled pixels, apply zoom)
                let screen_y = base_screen_y - leaf.height * state.camera.zoom;

                let alpha = leaf.get_alpha();
                let color = Color::new(
                    leaf.color.r,
                    leaf.color.g,
                    leaf.color.b,
                    leaf.color.a * alpha,
                );
                let size = leaf.size * state.camera.zoom;

                // Draw a simple leaf shape (small rotated diamond)
                let cos_r = leaf.rotation.cos();
                let sin_r = leaf.rotation.sin();

                // Draw as a small diamond/leaf shape
                let hw = size * 0.5;
                let hh = size * 0.8;

                let points = [
                    (
                        screen_x + cos_r * 0.0 - sin_r * (-hh),
                        screen_y + sin_r * 0.0 + cos_r * (-hh),
                    ), // top
                    (
                        screen_x + cos_r * hw - sin_r * 0.0,
                        screen_y + sin_r * hw + cos_r * 0.0,
                    ), // right
                    (
                        screen_x + cos_r * 0.0 - sin_r * hh,
                        screen_y + sin_r * 0.0 + cos_r * hh,
                    ), // bottom
                    (
                        screen_x + cos_r * (-hw) - sin_r * 0.0,
                        screen_y + sin_r * (-hw) + cos_r * 0.0,
                    ), // left
                ];

                // Draw as two triangles
                draw_triangle(
                    Vec2::new(points[0].0, points[0].1),
                    Vec2::new(points[1].0, points[1].1),
                    Vec2::new(points[2].0, points[2].1),
                    color,
                );
                draw_triangle(
                    Vec2::new(points[0].0, points[0].1),
                    Vec2::new(points[2].0, points[2].1),
                    Vec2::new(points[3].0, points[3].1),
                    color,
                );
            }

            // Render rock debris particles (world-space, after depth-sorted objects)
            for particle in &state.rock_particles {
                if !is_visible_world(particle.tile_x, particle.tile_y) {
                    continue;
                }

                let (screen_x, base_screen_y) =
                    world_to_screen(particle.tile_x, particle.tile_y, &state.camera);
                let screen_y = base_screen_y - particle.height * state.camera.zoom;

                let alpha = particle.get_alpha();
                let color = Color::new(
                    particle.color.r,
                    particle.color.g,
                    particle.color.b,
                    particle.color.a * alpha,
                );
                let size = particle.size * state.camera.zoom;

                // Draw as a small rotated square (chunkier than leaf diamonds)
                let cos_r = particle.rotation.cos();
                let sin_r = particle.rotation.sin();
                let hs = size * 0.5;

                let points = [
                    (
                        screen_x + cos_r * (-hs) - sin_r * (-hs),
                        screen_y + sin_r * (-hs) + cos_r * (-hs),
                    ),
                    (
                        screen_x + cos_r * hs - sin_r * (-hs),
                        screen_y + sin_r * hs + cos_r * (-hs),
                    ),
                    (
                        screen_x + cos_r * hs - sin_r * hs,
                        screen_y + sin_r * hs + cos_r * hs,
                    ),
                    (
                        screen_x + cos_r * (-hs) - sin_r * hs,
                        screen_y + sin_r * (-hs) + cos_r * hs,
                    ),
                ];

                draw_triangle(
                    Vec2::new(points[0].0, points[0].1),
                    Vec2::new(points[1].0, points[1].1),
                    Vec2::new(points[2].0, points[2].1),
                    color,
                );
                draw_triangle(
                    Vec2::new(points[0].0, points[0].1),
                    Vec2::new(points[2].0, points[2].1),
                    Vec2::new(points[3].0, points[3].1),
                    color,
                );
            }

            // Render fishing bubble particles (small rising circles)
            let bubble_time = get_time();
            for bubble in &state.fishing_bubbles {
                if !is_visible_world(bubble.tile_x, bubble.tile_y) {
                    continue;
                }

                let (screen_x, base_screen_y) =
                    world_to_screen(bubble.tile_x, bubble.tile_y, &state.camera);
                let screen_y = base_screen_y - bubble.height * state.camera.zoom;

                let alpha = bubble.get_alpha(bubble_time);
                let size = bubble.size * state.camera.zoom;

                // Subtle white/light-blue bubble
                let color = Color::new(0.8, 0.9, 1.0, alpha * 0.35);
                draw_circle(screen_x, screen_y, size, color);

                // Tiny bright highlight on the bubble
                let highlight = Color::new(1.0, 1.0, 1.0, alpha * 0.2);
                draw_circle(
                    screen_x - size * 0.25,
                    screen_y - size * 0.25,
                    size * 0.35,
                    highlight,
                );
            }
        } // end if !graphics_low (particle rendering)

        timings.entities_ms = (get_time() - t1) * 1000.0;

        // 3.9. Render click effects on the ground
        self.render_click_effects(state);

        // 4. Render overhead layer (always on top)
        let t2 = get_time();
        self.render_tilemap_layer(state, LayerType::Overhead);

        // 4.1. Render exit portal arrows on interior maps
        self.render_exit_portal_arrows(state);

        timings.overhead_ms = (get_time() - t2) * 1000.0;

        // 4.2. Render local player silhouette (on top of overhead, visible through trees)
        if let Some(ref local_id) = state.local_player_id {
            if let Some(local_player) = state.players.get(local_id) {
                self.render_player_silhouette(local_player, &state.camera, &state.item_registry);
            }
        }

        // 4.5. Render name tags above all map elements (overhead, walls, objects, etc.)
        self.render_name_tags(state);
        self.render_stall_indicators(state);
        self.render_tree_name_tag(state);
        self.render_ore_name_tag(state);
        self.render_map_object_name_tag(state);
        self.render_farming_patch_labels(state);

        // 5. Render floating damage numbers
        let t3 = get_time();
        self.render_damage_numbers(state);

        // 6. Render floating level up text
        self.render_level_up_events(state);

        // 7. Render chat bubbles above players
        self.render_chat_bubbles(state);

        // 7.5. Render projectiles
        self.render_projectiles(state);

        // 7.6. Render spell effects (animated sprite sheets)
        self.render_spell_effects(state);
        timings.effects_ms = (get_time() - t3) * 1000.0;

        // 8. Render UI (non-interactive elements) — skip in spectator mode
        let t4 = get_time();
        if !state.spectator_mode {
            self.font_scale.set(state.ui_state.ui_scale);
            self.render_ui(state);
        }

        // 9. Render interactive UI elements and return layout for hit detection
        let layout = if state.spectator_mode {
            UiLayout::default()
        } else {
            self.render_interactive_ui(state)
        };

        // 10. Render XP drops above interactive UI overlays (e.g. crafting fade)
        if !state.spectator_mode {
            self.render_deferred_xp_drops(state);
        }
        timings.ui_ms = (get_time() - t4) * 1000.0;

        timings.total_ms = (get_time() - render_start) * 1000.0;
        (layout, timings)
    }
}
