use super::*;

impl Renderer {
    pub(super) fn render_tree_timer(
        &self,
        tile_x: i32,
        tile_y: i32,
        tile_z: f32,
        progress: f32,
        camera: &Camera,
    ) {
        let zoom = camera.zoom;

        // Convert tile position to screen position (center of tile)
        let (screen_x, mut screen_y) =
            world_to_screen_z(tile_x as f32 + 0.5, tile_y as f32 + 0.5, tile_z, camera);
        // Adjust Y to center on tile (world_to_screen gives bottom of tile)
        screen_y -= 16.0 * zoom;

        // Draw pie chart timer (15% more opaque for visibility)
        let radius = 12.0 * zoom;
        let bg_color = Color::new(0.0, 0.0, 0.0, 0.50);
        let fill_color = Color::new(0.2, 0.8, 0.2, 0.60);
        let border_color = Color::new(0.1, 0.4, 0.1, 0.75);

        // Draw background circle
        draw_circle(screen_x, screen_y, radius, bg_color);

        // Draw filled pie slice showing progress
        if progress > 0.0 {
            let segments = 32;
            let start_angle = -std::f32::consts::FRAC_PI_2; // Start from top

            // Draw pie as triangle fan
            for i in 0..segments {
                let t1 = i as f32 / segments as f32;
                let t2 = (i + 1) as f32 / segments as f32;
                let angle1 = start_angle + t1 * progress * std::f32::consts::TAU;
                let angle2 = start_angle + t2 * progress * std::f32::consts::TAU;

                let x1 = screen_x + angle1.cos() * radius;
                let y1 = screen_y + angle1.sin() * radius;
                let x2 = screen_x + angle2.cos() * radius;
                let y2 = screen_y + angle2.sin() * radius;

                draw_triangle(
                    Vec2::new(screen_x, screen_y),
                    Vec2::new(x1, y1),
                    Vec2::new(x2, y2),
                    fill_color,
                );
            }
        }

        // Draw border circle
        draw_circle_lines(screen_x, screen_y, radius, 2.0, border_color);
    }

    pub(super) fn render_level_up_events(&self, state: &GameState) {
        let current_time = macroquad::time::get_time();
        const DURATION: f32 = 1.2;
        const FONT_SIZE: f32 = 16.0;

        for event in &state.level_up_events {
            let age = (current_time - event.time) as f32;
            if age > DURATION {
                continue;
            }

            let t = age / DURATION;
            let float_offset = (age * 40.0).round();

            let (screen_x, screen_y) = world_to_screen(event.x, event.y, &state.camera);
            let height_offset = (SPRITE_HEIGHT - 8.0) / 2.0;
            let final_y = (screen_y - height_offset - float_offset).round();

            let alpha = if t < 0.5 { 1.0 } else { 1.0 - (t - 0.5) * 2.0 };

            let text = format!("LEVEL UP! ({})", event.new_level);
            let text_dims = self.measure_text_sharp(&text, FONT_SIZE);
            let draw_x = (screen_x - text_dims.width / 2.0).round();

            let outline_color = Color::new(0.0, 0.0, 0.0, alpha * 0.9);
            for &(ox, oy) in &[(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                self.draw_text_sharp(&text, draw_x + ox, final_y + oy, FONT_SIZE, outline_color);
            }

            let base_color = Color::new(1.0, 1.0, 0.0, alpha);
            self.draw_text_sharp(&text, draw_x, final_y, FONT_SIZE, base_color);
        }
    }

    /// Create a mesh for a rounded rectangle with optional tail (no overlapping geometry)
    pub(super) fn create_rounded_rect_mesh(
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        color: Color,
    ) -> Mesh {
        Self::create_bubble_mesh(x, y, w, h, r, color, None)
    }

    /// Create a mesh for a chat bubble with tail (no overlapping geometry)
    pub(super) fn create_bubble_mesh(
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        color: Color,
        _tail: Option<(f32, f32, f32)>,
    ) -> Mesh {
        let color_arr = [
            (color.r * 255.0) as u8,
            (color.g * 255.0) as u8,
            (color.b * 255.0) as u8,
            (color.a * 255.0) as u8,
        ];

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Helper to add a vertex
        let mut add_vertex = |px: f32, py: f32| -> u16 {
            let idx = vertices.len() as u16;
            vertices.push(Vertex {
                position: Vec3::new(px, py, 0.0),
                uv: Vec2::ZERO,
                color: color_arr,
                normal: Vec4::ZERO,
            });
            idx
        };

        // Corner circle segment count
        let segments = 8;

        // Center rectangle vertices (4 corners where the rounded corners meet)
        let c_tl = add_vertex(x + r, y + r); // top-left inner corner
        let c_tr = add_vertex(x + w - r, y + r); // top-right inner corner
        let c_bl = add_vertex(x + r, y + h - r); // bottom-left inner corner
        let c_br = add_vertex(x + w - r, y + h - r); // bottom-right inner corner

        // Center rectangle (2 triangles)
        indices.extend_from_slice(&[c_tl, c_tr, c_br, c_tl, c_br, c_bl]);

        // Top edge strip
        let t_tl = add_vertex(x + r, y);
        let t_tr = add_vertex(x + w - r, y);
        indices.extend_from_slice(&[t_tl, t_tr, c_tr, t_tl, c_tr, c_tl]);

        // Bottom edge strip
        let b_bl = add_vertex(x + r, y + h);
        let b_br = add_vertex(x + w - r, y + h);
        indices.extend_from_slice(&[c_bl, c_br, b_br, c_bl, b_br, b_bl]);

        // Left edge strip
        let l_tl = add_vertex(x, y + r);
        let l_bl = add_vertex(x, y + h - r);
        indices.extend_from_slice(&[l_tl, c_tl, c_bl, l_tl, c_bl, l_bl]);

        // Right edge strip
        let r_tr = add_vertex(x + w, y + r);
        let r_br = add_vertex(x + w, y + h - r);
        indices.extend_from_slice(&[c_tr, r_tr, r_br, c_tr, r_br, c_br]);

        // Corner fans (quarter circles)
        use std::f32::consts::PI;

        // Top-left corner (180° to 270°)
        let mut prev = l_tl; // starts at left edge
        for i in 1..=segments {
            let angle = PI + (PI / 2.0) * (i as f32 / segments as f32);
            let px = x + r + r * angle.cos();
            let py = y + r + r * angle.sin();
            let curr = add_vertex(px, py);
            indices.extend_from_slice(&[c_tl, prev, curr]);
            prev = curr;
        }

        // Top-right corner (270° to 360°)
        prev = t_tr; // starts at top edge
        for i in 1..=segments {
            let angle = PI * 1.5 + (PI / 2.0) * (i as f32 / segments as f32);
            let px = x + w - r + r * angle.cos();
            let py = y + r + r * angle.sin();
            let curr = add_vertex(px, py);
            indices.extend_from_slice(&[c_tr, prev, curr]);
            prev = curr;
        }

        // Bottom-right corner (0° to 90°)
        prev = r_br; // starts at right edge
        for i in 1..=segments {
            let angle = (PI / 2.0) * (i as f32 / segments as f32);
            let px = x + w - r + r * angle.cos();
            let py = y + h - r + r * angle.sin();
            let curr = add_vertex(px, py);
            indices.extend_from_slice(&[c_br, prev, curr]);
            prev = curr;
        }

        // Bottom-left corner (90° to 180°)
        prev = b_bl; // starts at bottom edge
        for i in 1..=segments {
            let angle = PI / 2.0 + (PI / 2.0) * (i as f32 / segments as f32);
            let px = x + r + r * angle.cos();
            let py = y + h - r + r * angle.sin();
            let curr = add_vertex(px, py);
            indices.extend_from_slice(&[c_bl, prev, curr]);
            prev = curr;
        }

        Mesh {
            vertices,
            indices,
            texture: None,
        }
    }

    /// Render name tags for all hovered/selected players and NPCs.
    /// Called after overhead tiles so names always appear above all map elements.
    pub(super) fn render_name_tags(&self, state: &GameState) {
        // Player name tags
        for player in state.players.values() {
            let is_selected = state.selected_entity_id.as_ref() == Some(&player.id);
            let is_hovered = state.hovered_entity_id.as_ref() == Some(&player.id);
            if !is_selected && !is_hovered {
                continue;
            }

            let (screen_x, screen_y) =
                world_to_screen_z(player.x, player.y, player.z, &state.camera);
            let zoom = state.camera.zoom;
            let font_size = 16.0 * zoom;
            let scaled_sprite_height = SPRITE_HEIGHT * zoom;
            let has_sprite = self
                .get_player_sprite(&player.gender, &player.skin)
                .is_some();
            let name_y_offset = if has_sprite {
                scaled_sprite_height - 8.0 * zoom
            } else {
                24.0 * zoom
            };

            let combat_level = player.combat_level();
            let level_text = format!(" (Lvl {})", combat_level);
            let has_title = player.title.is_some();
            let title_text = player.title.as_deref().unwrap_or("");
            let name_width = self.measure_text_sharp(&player.name, font_size).width;
            let level_width = self.measure_text_sharp(&level_text, font_size).width - 2.0 * zoom;
            let is_top_player =
                state.top_level_player_name.as_deref() == Some(player.name.as_str());
            let is_second_player = !is_top_player
                && state.second_level_player_name.as_deref() == Some(player.name.as_str());
            let has_trophy = (is_top_player || is_second_player) && self.ui_icons.is_some();
            let trophy_icon_size = 16.0 * zoom;
            let trophy_gap = 4.0 * zoom;
            let trophy_width = if has_trophy {
                trophy_icon_size + trophy_gap
            } else {
                0.0
            };

            // Title row (above name): centered independently
            let title_width = if has_title {
                self.measure_text_sharp(title_text, font_size).width
            } else {
                0.0
            };
            let row_height = 16.0 * zoom;
            let title_row_offset = if has_title { row_height } else { 0.0 };

            // Name row: trophy + name + level
            let name_row_width = trophy_width + name_width + level_width;
            let name_x = screen_x - name_row_width / 2.0;
            let name_y = screen_y - name_y_offset + 2.0 * zoom;

            let padding = 4.0 * zoom;
            let bar_height = if has_title {
                18.0 * zoom + title_row_offset
            } else {
                18.0 * zoom
            };
            let bg_width = name_row_width.max(title_width);
            let bg_x = screen_x - bg_width / 2.0;
            let bg_top = name_y - 14.0 * zoom - title_row_offset;
            draw_rectangle(
                bg_x - padding,
                bg_top,
                bg_width + padding * 2.0,
                bar_height,
                Color::from_rgba(0, 0, 0, 180),
            );

            // Draw title above name
            if has_title {
                let title_color = Color::from_rgba(255, 215, 100, 255);
                let title_x = screen_x - title_width / 2.0;
                self.draw_text_sharp(
                    title_text,
                    title_x,
                    name_y - title_row_offset,
                    font_size,
                    title_color,
                );
            }

            // Draw trophy icon: gold for #1, silver for #2
            if has_trophy {
                if let Some(ref texture) = self.ui_icons {
                    let src_rect = if is_top_player {
                        Rect::new(24.0, 48.0, 24.0, 24.0)
                    } else {
                        Rect::new(0.0, 48.0, 24.0, 24.0)
                    };
                    let icon_y = (name_y - 14.0 * zoom) + (18.0 * zoom - trophy_icon_size) / 2.0;
                    draw_texture_ex(
                        texture,
                        name_x,
                        icon_y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(src_rect),
                            dest_size: Some(Vec2::new(trophy_icon_size, trophy_icon_size)),
                            ..Default::default()
                        },
                    );
                }
            }

            self.draw_text_sharp(
                &player.name,
                name_x + trophy_width,
                name_y,
                font_size,
                WHITE,
            );
            let level_color = Color::from_rgba(180, 220, 255, 255);
            self.draw_text_sharp(
                &level_text,
                name_x + trophy_width + name_width,
                name_y,
                font_size,
                level_color,
            );
        }

        // NPC name tags
        for npc in state.npcs.values() {
            if npc.death_timer.is_some() || npc.is_death_animation_complete() {
                continue;
            }

            let is_selected = state.selected_entity_id.as_ref() == Some(&npc.id);
            let is_hovered = state.hovered_entity_id.as_ref() == Some(&npc.id);
            if !is_selected && !is_hovered {
                continue;
            }

            let center_offset = (npc.size - 1) as f32 * 0.5;
            let (screen_x, screen_y) = world_to_screen_z(
                npc.x + center_offset,
                npc.y + center_offset,
                npc.z,
                &state.camera,
            );
            let zoom = state.camera.zoom;

            // Compute sprite height to find top_y
            let sprite_height = if let Some((_, h)) = self
                .npc_sprites
                .get_dimensions(&npc.entity_type)
                .or_else(|| {
                    self.npc_overflow_sprites
                        .get(&npc.entity_type)
                        .map(|t| (t.width(), t.height()))
                }) {
                (h * zoom).round()
            } else {
                // Fallback ellipse sizing
                let time = macroquad::time::get_time() as f32;
                let wobble = (time * 2.0 + npc.x + npc.y).sin() * 0.5 + 0.5;
                let radius = (10.0 + wobble * 1.5) * zoom;
                let height_offset = (8.0 + wobble * 2.0) * zoom;
                (height_offset + radius) * 2.0
            };
            let top_y = screen_y - sprite_height + 4.0 * zoom + npc.render_offset_y * zoom;

            let name_color = if npc.is_hostile() {
                Color::from_rgba(255, 150, 150, 255)
            } else if npc.is_quest_giver {
                Color::from_rgba(150, 220, 255, 255)
            } else if npc.is_banker {
                Color::from_rgba(255, 215, 0, 255)
            } else if npc.is_merchant {
                Color::from_rgba(150, 255, 150, 255)
            } else if npc.station_type.is_some() {
                Color::from_rgba(255, 180, 100, 255)
            } else {
                Color::from_rgba(255, 255, 255, 255)
            };

            let font_size = 16.0 * zoom;
            let name = npc.name();
            let name_width = self.measure_text_sharp(&name, font_size).width;
            let name_y = top_y - 5.0 * zoom;
            let padding = 4.0 * zoom;

            let show_turn_in_check = npc.is_quest_giver && npc.can_turn_in_quest;
            let small_icon: Option<&Texture2D> = if npc.is_quest_giver && !show_turn_in_check {
                self.chat_small_icon.as_ref()
            } else {
                None
            };
            let check_icon_width = if show_turn_in_check { 16.0 * zoom } else { 0.0 };

            let icon_gap = 4.0 * zoom;
            let (total_width, icon_width) = if let Some(tex) = small_icon {
                let w = tex.width() * zoom;
                (w + icon_gap + name_width, w)
            } else if show_turn_in_check {
                (check_icon_width + icon_gap + name_width, check_icon_width)
            } else {
                (name_width, 0.0)
            };
            let content_x = screen_x - total_width / 2.0;

            let bar_height = 18.0 * zoom;
            draw_rectangle(
                content_x - padding,
                name_y - 14.0 * zoom,
                total_width + padding * 2.0,
                bar_height,
                Color::from_rgba(0, 0, 0, 180),
            );

            if let Some(tex) = small_icon {
                let icon_h = tex.height() * zoom;
                let bar_top = name_y - 14.0 * zoom;
                let icon_y = bar_top + (bar_height - icon_h) / 2.0;
                draw_texture_ex(
                    tex,
                    content_x,
                    icon_y,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(tex.width() * zoom, icon_h)),
                        ..Default::default()
                    },
                );
            } else if show_turn_in_check {
                if let Some(ref texture) = self.ui_icons {
                    let src_rect = Rect::new(24.0, 216.0, 24.0, 24.0); // row 10, col 2 (1-based)
                    let icon_size = 16.0 * zoom;
                    let bar_top = name_y - 14.0 * zoom;
                    let icon_y = bar_top + (bar_height - icon_size) / 2.0;
                    draw_texture_ex(
                        texture,
                        content_x,
                        icon_y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(src_rect),
                            dest_size: Some(Vec2::new(icon_size, icon_size)),
                            ..Default::default()
                        },
                    );
                } else {
                    self.draw_text_sharp(
                        "✓",
                        content_x,
                        name_y,
                        font_size,
                        Color::from_rgba(120, 255, 140, 255),
                    );
                }
            }

            let text_x = if small_icon.is_some() || show_turn_in_check {
                content_x + icon_width + icon_gap
            } else {
                content_x
            };

            self.draw_text_sharp(&name, text_x, name_y, font_size, name_color);
        }
    }

    /// Render overhead stall indicator for players with active stalls
    pub(super) fn render_stall_indicators(&self, state: &GameState) {
        let zoom = state.camera.zoom;
        let font_size = 16.0 * zoom;
        let scaled_sprite_height = SPRITE_HEIGHT * zoom;

        for player in state.players.values() {
            if !player.has_stall {
                continue;
            }
            // Show for local player too (so they see their shop is open)

            let (screen_x, screen_y) =
                world_to_screen_z(player.x, player.y, player.z, &state.camera);
            let has_sprite = self
                .get_player_sprite(&player.gender, &player.skin)
                .is_some();
            let name_y_offset = if has_sprite {
                scaled_sprite_height - 8.0 * zoom
            } else {
                24.0 * zoom
            };

            // Position above the name tag area
            let stall_label = player.stall_name.as_deref().unwrap_or("Shop");
            let label_w = self.measure_text_sharp(stall_label, font_size).width;
            let padding = 4.0 * zoom;
            let bar_height = 18.0 * zoom;
            let tag_x = (screen_x - (label_w + padding * 2.0) / 2.0).floor();
            let name_y = (screen_y - name_y_offset + 2.0 * zoom).floor();
            let tag_y = (name_y - 14.0 * zoom - bar_height - 2.0 * zoom).floor();

            // Green background bar
            draw_rectangle(
                tag_x,
                tag_y,
                label_w + padding * 2.0,
                bar_height,
                Color::new(0.1, 0.45, 0.1, 0.85),
            );
            self.draw_text_sharp(
                stall_label,
                (tag_x + padding).floor(),
                (tag_y + bar_height - 4.0 * zoom).floor(),
                font_size,
                Color::new(0.9, 1.0, 0.9, 1.0),
            );
        }
    }

    /// Render name tag for hovered tree showing name and level requirement
    pub(super) fn render_tree_name_tag(&self, state: &GameState) {
        // Only show if we're hovering over a tile
        let Some((tile_x, tile_y)) = state.hovered_tile else {
            return;
        };

        // Check if this tile is depleted (don't show for stumps)
        if state.depleted_trees.contains_key(&(tile_x, tile_y)) {
            return;
        }

        // Check if there's an object at this exact tile (no tall-object extension)
        let Some(obj) = state.chunk_manager.get_object_at_exact(tile_x, tile_y) else {
            return;
        };

        // Check if this object is a tree (by GID)
        let Some(tree_info) = get_tree_info(obj.gid) else {
            return;
        };

        // Get player's woodcutting level
        let player_wc_level = state
            .get_local_player()
            .map(|p| p.skills.get(crate::game::SkillType::Woodcutting).level)
            .unwrap_or(1);

        let can_chop = player_wc_level >= tree_info.level_required;

        // Get screen position (center of tile, raised up)
        let (screen_x, screen_y) =
            world_to_screen(tile_x as f32 + 0.5, tile_y as f32 + 0.5, &state.camera);
        let zoom = state.camera.zoom;

        // Get actual sprite height for this tree
        let sprite_height = if let Some((texture, source_rect)) = self.get_object_sprite(obj.gid) {
            let tex_height = if let Some(r) = source_rect {
                r.h
            } else {
                texture.height()
            };
            tex_height * zoom
        } else {
            80.0 * zoom // Fallback if sprite not found
        };

        // Position the tag above the tree sprite
        let tag_y = screen_y - sprite_height - 5.0 * zoom;

        // Format text: "Oak Tree (Lvl 1)"
        let text = format!("{} (Lvl {})", tree_info.name, tree_info.level_required);
        let font_size = 16.0 * zoom;
        let text_dims = self.measure_text_sharp(&text, font_size);

        // Choose color based on whether player can chop
        let level_color = if can_chop {
            Color::from_rgba(100, 255, 100, 255) // Green
        } else {
            Color::from_rgba(255, 100, 100, 255) // Red
        };

        // Draw background
        let padding = 4.0 * zoom;
        let bar_height = 18.0 * zoom;
        let bar_x = screen_x - text_dims.width / 2.0 - padding;
        let bar_y = tag_y - 14.0 * zoom;

        draw_rectangle(
            bar_x,
            bar_y,
            text_dims.width + padding * 2.0,
            bar_height,
            Color::from_rgba(0, 0, 0, 180),
        );

        // Draw text
        let text_x = screen_x - text_dims.width / 2.0;
        self.draw_text_sharp(&text, text_x, tag_y, font_size, level_color);
    }

    /// Render name tag for hovered rock/ore showing name and level requirement
    pub(super) fn render_ore_name_tag(&self, state: &GameState) {
        let Some((tile_x, tile_y)) = state.hovered_tile else {
            return;
        };

        // Don't show for depleted rocks
        if state.depleted_rocks.contains_key(&(tile_x, tile_y)) {
            return;
        }

        // Check if there's an object at this tile
        let Some(obj) = state.chunk_manager.get_object_at_exact(tile_x, tile_y) else {
            return;
        };

        // Check if this object is an ore rock (by GID)
        let Some(ore_info) = get_ore_info(obj.gid) else {
            return;
        };

        // Get player's mining level
        let player_mining_level = state
            .get_local_player()
            .map(|p| p.skills.get(crate::game::SkillType::Mining).level)
            .unwrap_or(1);

        let can_mine = player_mining_level >= ore_info.level_required;

        // Get screen position
        let (screen_x, screen_y) =
            world_to_screen(tile_x as f32 + 0.5, tile_y as f32 + 0.5, &state.camera);
        let zoom = state.camera.zoom;

        // Get actual sprite height for this rock
        let sprite_height = if let Some((texture, source_rect)) = self.get_object_sprite(obj.gid) {
            let tex_height = if let Some(r) = source_rect {
                r.h
            } else {
                texture.height()
            };
            tex_height * zoom
        } else {
            40.0 * zoom
        };

        // Position the tag above the rock sprite
        let tag_y = screen_y - sprite_height - 5.0 * zoom;

        let text = format!("{} (Lvl {})", ore_info.name, ore_info.level_required);
        let font_size = 16.0 * zoom;
        let text_dims = self.measure_text_sharp(&text, font_size);

        let level_color = if can_mine {
            Color::from_rgba(100, 255, 100, 255)
        } else {
            Color::from_rgba(255, 100, 100, 255)
        };

        let padding = 4.0 * zoom;
        let bar_height = 18.0 * zoom;
        let bar_x = screen_x - text_dims.width / 2.0 - padding;
        let bar_y = tag_y - 14.0 * zoom;

        draw_rectangle(
            bar_x,
            bar_y,
            text_dims.width + padding * 2.0,
            bar_height,
            Color::from_rgba(0, 0, 0, 180),
        );

        let text_x = screen_x - text_dims.width / 2.0;
        self.draw_text_sharp(&text, text_x, tag_y, font_size, level_color);
    }

    /// Render name tag for hovered map objects (obelisks, etc.)
    pub(super) fn render_map_object_name_tag(&self, state: &GameState) {
        let Some((tile_x, tile_y)) = state.hovered_tile else {
            return;
        };

        let Some(obj) = state.chunk_manager.get_object_at_exact(tile_x, tile_y) else {
            return;
        };

        // Skip trees and rocks (they have their own name tags)
        if get_tree_info(obj.gid).is_some() || get_ore_info(obj.gid).is_some() {
            return;
        }

        let name: String = if let Some(n) = crate::input::handler::get_map_object_name(obj.gid) {
            n.to_string()
        } else if state.chest_positions.contains(&(tile_x, tile_y)) {
            "Chest".to_string()
        } else {
            return;
        };

        let (screen_x, screen_y) =
            world_to_screen(tile_x as f32 + 0.5, tile_y as f32 + 0.5, &state.camera);
        let zoom = state.camera.zoom;

        let sprite_height = if let Some((texture, source_rect)) = self.get_object_sprite(obj.gid) {
            let tex_height = if let Some(r) = source_rect {
                r.h
            } else {
                texture.height()
            };
            tex_height * zoom
        } else {
            40.0 * zoom
        };

        let tag_y = screen_y - sprite_height - 5.0 * zoom;
        let font_size = 16.0 * zoom;
        let text_dims = self.measure_text_sharp(&name, font_size);
        let label_color = Color::from_rgba(255, 215, 0, 255); // Gold color

        let padding = 4.0 * zoom;
        let bar_height = 18.0 * zoom;
        let bar_x = screen_x - text_dims.width / 2.0 - padding;
        let bar_y = tag_y - 14.0 * zoom;

        draw_rectangle(
            bar_x,
            bar_y,
            text_dims.width + padding * 2.0,
            bar_height,
            Color::from_rgba(0, 0, 0, 180),
        );

        let text_x = screen_x - text_dims.width / 2.0;
        self.draw_text_sharp(&name, text_x, tag_y, font_size, label_color);
    }

    /// Render chat bubbles above players' heads
    pub(super) fn render_chat_bubbles(&self, state: &GameState) {
        let current_time = macroquad::time::get_time();

        for bubble in &state.chat_bubbles {
            let age = (current_time - bubble.time) as f32;
            if age > 5.0 {
                continue;
            }

            // Find the player this bubble belongs to
            let Some(player) = state.players.get(&bubble.player_id) else {
                continue;
            };

            // Get player screen position
            let (screen_x, screen_y) =
                world_to_screen_z(player.x, player.y, player.z, &state.camera);

            // Fade out in the last 1 second (age 4-5)
            let alpha = if age > 4.0 {
                ((5.0 - age) * 255.0) as u8
            } else {
                255
            };

            // Word wrap the text - scale with zoom for readability
            let zoom = state.camera.zoom;
            let font_size = 16.0 * zoom;
            let line_height = 18.0 * zoom;
            let max_bubble_width = 220.0 * zoom;
            let padding_h = 4.0 * zoom;
            let padding_v = 1.0 * zoom;
            let tail_height = 6.0 * zoom;
            let corner_radius = 5.0 * zoom;

            let lines = self.wrap_text(&bubble.text, max_bubble_width - padding_h * 2.0, font_size);
            let num_lines = lines.len().max(1);

            // Calculate bubble dimensions
            let mut max_line_width = 0.0f32;
            for line in &lines {
                let width = self.measure_text_sharp(line, font_size).width;
                max_line_width = max_line_width.max(width);
            }

            let bubble_width = (max_line_width + padding_h * 2.0).max(18.0 * zoom);
            let bubble_height = num_lines as f32 * line_height + padding_v * 2.0;

            // Position bubble above player's head
            // Base offset: sprite height (78) minus feet offset (8) = 70, scaled by zoom
            let base_offset = (SPRITE_HEIGHT - 8.0) * zoom;

            // Check if name tag is showing (hovered or selected) - need extra space
            let is_hovered = state.hovered_entity_id.as_ref() == Some(&bubble.player_id);
            let is_selected = state.selected_entity_id.as_ref() == Some(&bubble.player_id);
            let name_offset = if is_hovered || is_selected {
                16.0 * zoom
            } else {
                0.0
            };

            let bubble_x = screen_x - bubble_width / 2.0;
            let bubble_y = screen_y - base_offset - name_offset - bubble_height - tail_height;

            // Colors with alpha - off-white paper/comic book style
            let bg_alpha = (alpha as f32 * 0.8) as u8; // 80% opacity for background
            let bg_color = Color::from_rgba(255, 250, 240, bg_alpha); // Warm off-white/cream
            let border_color = Color::from_rgba(60, 50, 40, alpha); // Dark brown border
            let text_color = Color::from_rgba(30, 25, 20, alpha); // Dark brown text

            // Draw rounded rectangle bubble body using mesh (no overlapping geometry)
            let r = corner_radius;
            let bx = bubble_x.floor();
            let by = bubble_y.floor();
            let bw = bubble_width.floor();
            let bh = bubble_height.floor();

            // Draw border first (slightly larger rounded rect)
            let border_mesh = Self::create_rounded_rect_mesh(
                bx - 1.0,
                by - 1.0,
                bw + 2.0,
                bh + 2.0,
                r + 1.0,
                border_color,
            );
            draw_mesh(&border_mesh);

            // Draw fill on top using mesh (no overlapping = no alpha stacking)
            let fill_mesh = Self::create_rounded_rect_mesh(bx, by, bw, bh, r, bg_color);
            draw_mesh(&fill_mesh);

            // Draw tail
            let tail_x = screen_x.floor();
            let tail_top_y = by + bh;
            let tail_bottom_y = tail_top_y + tail_height;
            let tail_half_width = 4.0 * zoom;

            // Tail border
            draw_triangle(
                Vec2::new(tail_x - tail_half_width - 1.0, tail_top_y),
                Vec2::new(tail_x + tail_half_width + 1.0, tail_top_y),
                Vec2::new(tail_x, tail_bottom_y + 1.0),
                border_color,
            );
            // Tail fill - use a mesh vertex approach to match the bubble's alpha exactly
            // Create a small mesh for just the tail triangle
            let tail_color_arr = [
                (bg_color.r * 255.0) as u8,
                (bg_color.g * 255.0) as u8,
                (bg_color.b * 255.0) as u8,
                (bg_color.a * 255.0) as u8,
            ];
            let tail_mesh = Mesh {
                vertices: vec![
                    Vertex {
                        position: Vec3::new(tail_x - tail_half_width, tail_top_y, 0.0),
                        uv: Vec2::ZERO,
                        color: tail_color_arr,
                        normal: Vec4::ZERO,
                    },
                    Vertex {
                        position: Vec3::new(tail_x + tail_half_width, tail_top_y, 0.0),
                        uv: Vec2::ZERO,
                        color: tail_color_arr,
                        normal: Vec4::ZERO,
                    },
                    Vertex {
                        position: Vec3::new(tail_x, tail_bottom_y, 0.0),
                        uv: Vec2::ZERO,
                        color: tail_color_arr,
                        normal: Vec4::ZERO,
                    },
                ],
                indices: vec![0, 1, 2],
                texture: None,
            };
            draw_mesh(&tail_mesh);

            // Tail border lines
            draw_line(
                tail_x - tail_half_width,
                tail_top_y,
                tail_x,
                tail_bottom_y,
                1.0,
                border_color,
            );
            draw_line(
                tail_x + tail_half_width,
                tail_top_y,
                tail_x,
                tail_bottom_y,
                1.0,
                border_color,
            );

            // Draw text lines (centered)
            let bubble_center_x = bx + bw / 2.0;
            let mut text_y = by + padding_v + font_size * 0.85;

            for line in &lines {
                let line_width = self.measure_text_sharp(line, font_size).width;
                let text_x = bubble_center_x - line_width / 2.0;
                self.draw_text_sharp(line, text_x, text_y, font_size, text_color);
                text_y += line_height;
            }
        }

        // Render NPC speech bubbles
        for npc in state.npcs.values() {
            if npc.state == NpcState::Dead {
                continue;
            }

            let Some((ref text, time)) = npc.speech_bubble else {
                continue;
            };

            let age = (current_time - time) as f32;
            if age > 5.0 {
                continue;
            }

            // Get NPC screen position
            let (screen_x, screen_y) = world_to_screen_z(npc.x, npc.y, npc.z, &state.camera);

            // Fade out in the last 1 second (age 4-5)
            let alpha = if age > 4.0 {
                ((5.0 - age) * 255.0) as u8
            } else {
                255
            };

            // Word wrap the text (same params as player bubbles) - scale with zoom
            let zoom = state.camera.zoom;
            let font_size = 16.0 * zoom;
            let line_height = 18.0 * zoom;
            let max_bubble_width = 220.0 * zoom;
            let padding_h = 4.0 * zoom;
            let padding_v = 1.0 * zoom;
            let tail_height = 6.0 * zoom;
            let corner_radius = 5.0 * zoom;

            let lines = self.wrap_text(text, max_bubble_width - padding_h * 2.0, font_size);
            let num_lines = lines.len().max(1);

            let mut max_line_width = 0.0f32;
            for line in &lines {
                let width = self.measure_text_sharp(line, font_size).width;
                max_line_width = max_line_width.max(width);
            }

            let bubble_width = (max_line_width + padding_h * 2.0).max(18.0 * zoom);
            let bubble_height = num_lines as f32 * line_height + padding_v * 2.0;

            // Position bubble above NPC's head
            let base_offset = (SPRITE_HEIGHT - 8.0) * zoom;

            let is_hovered = state.hovered_entity_id.as_ref() == Some(&npc.id);
            let is_selected = state.selected_entity_id.as_ref() == Some(&npc.id);
            let name_offset = if is_hovered || is_selected {
                16.0 * zoom
            } else {
                0.0
            };

            let bubble_x = screen_x - bubble_width / 2.0;
            let bubble_y = screen_y - base_offset - name_offset - bubble_height - tail_height;

            // Colors with alpha - off-white paper/comic book style
            let bg_alpha = (alpha as f32 * 0.8) as u8;
            let bg_color = Color::from_rgba(255, 250, 240, bg_alpha);
            let border_color = Color::from_rgba(60, 50, 40, alpha);
            let text_color = Color::from_rgba(30, 25, 20, alpha);

            let r = corner_radius;
            let bx = bubble_x.floor();
            let by = bubble_y.floor();
            let bw = bubble_width.floor();
            let bh = bubble_height.floor();

            let border_mesh = Self::create_rounded_rect_mesh(
                bx - 1.0,
                by - 1.0,
                bw + 2.0,
                bh + 2.0,
                r + 1.0,
                border_color,
            );
            draw_mesh(&border_mesh);

            let fill_mesh = Self::create_rounded_rect_mesh(bx, by, bw, bh, r, bg_color);
            draw_mesh(&fill_mesh);

            // Draw tail
            let tail_x = screen_x.floor();
            let tail_top_y = by + bh;
            let tail_bottom_y = tail_top_y + tail_height;
            let tail_half_width = 4.0 * zoom;

            draw_triangle(
                Vec2::new(tail_x - tail_half_width - 1.0, tail_top_y),
                Vec2::new(tail_x + tail_half_width + 1.0, tail_top_y),
                Vec2::new(tail_x, tail_bottom_y + 1.0),
                border_color,
            );

            let tail_color_arr = [
                (bg_color.r * 255.0) as u8,
                (bg_color.g * 255.0) as u8,
                (bg_color.b * 255.0) as u8,
                (bg_color.a * 255.0) as u8,
            ];
            let tail_mesh = Mesh {
                vertices: vec![
                    Vertex {
                        position: Vec3::new(tail_x - tail_half_width, tail_top_y, 0.0),
                        uv: Vec2::ZERO,
                        color: tail_color_arr,
                        normal: Vec4::ZERO,
                    },
                    Vertex {
                        position: Vec3::new(tail_x + tail_half_width, tail_top_y, 0.0),
                        uv: Vec2::ZERO,
                        color: tail_color_arr,
                        normal: Vec4::ZERO,
                    },
                    Vertex {
                        position: Vec3::new(tail_x, tail_bottom_y, 0.0),
                        uv: Vec2::ZERO,
                        color: tail_color_arr,
                        normal: Vec4::ZERO,
                    },
                ],
                indices: vec![0, 1, 2],
                texture: None,
            };
            draw_mesh(&tail_mesh);

            draw_line(
                tail_x - tail_half_width,
                tail_top_y,
                tail_x,
                tail_bottom_y,
                1.0,
                border_color,
            );
            draw_line(
                tail_x + tail_half_width,
                tail_top_y,
                tail_x,
                tail_bottom_y,
                1.0,
                border_color,
            );

            // Draw text lines (centered)
            let bubble_center_x = bx + bw / 2.0;
            let mut text_y = by + padding_v + font_size * 0.85;

            for line in &lines {
                let line_width = self.measure_text_sharp(line, font_size).width;
                let text_x = bubble_center_x - line_width / 2.0;
                self.draw_text_sharp(line, text_x, text_y, font_size, text_color);
                text_y += line_height;
            }
        }
    }
}
