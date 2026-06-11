use super::*;

impl Renderer {
    pub(super) fn render_map_object(&self, obj: &MapObject, tile_z: f32, camera: &Camera) {
        // Get screen position for the tile CENTER (add 0.5 to tile coords)
        let (screen_x, screen_y) = world_to_screen_z(
            obj.tile_x as f32 + 0.5,
            obj.tile_y as f32 + 0.5,
            tile_z,
            camera,
        );
        let zoom = camera.zoom;

        // Try to get the sprite for this gid
        if let Some((texture, source_rect)) = self.get_object_sprite(obj.gid) {
            // Check if this is an animated sprite
            let sprite_id = obj.gid.wrapping_sub(OBJECTS_FIRSTGID);
            let (source_rect, tex_width, tex_height) =
                if let Some(&frames) = self.animated_objects.get(&sprite_id) {
                    let (anim_rect, frame_w) = Self::get_animated_source_rect(source_rect, frames);
                    let h = source_rect.map(|r| r.h).unwrap_or(texture.height());
                    (anim_rect, frame_w, h)
                } else {
                    let (w, h) = if let Some(r) = source_rect {
                        (r.w, r.h)
                    } else {
                        (texture.width(), texture.height())
                    };
                    (source_rect, w, h)
                };

            // Scale the sprite (round to avoid fractional scaling artifacts)
            let scaled_width = (tex_width * zoom).round();
            let scaled_height = (tex_height * zoom).round();

            // Position sprite so its bottom-center aligns with the tile center
            // Round to pixel grid to avoid subpixel rendering artifacts
            let draw_x = (screen_x - scaled_width / 2.0).round();
            let draw_y = (screen_y - scaled_height).round();

            draw_texture_ex(
                texture,
                draw_x,
                draw_y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                    source: source_rect,
                    ..Default::default()
                },
            );
        } else {
            // Fallback: draw a colored placeholder rectangle
            let placeholder_width = (32.0 * zoom).round();
            let placeholder_height = (64.0 * zoom).round();
            let draw_x = (screen_x - placeholder_width / 2.0).round();
            let draw_y = (screen_y - placeholder_height).round();

            draw_rectangle(
                draw_x,
                draw_y,
                placeholder_width,
                placeholder_height,
                Color::from_rgba(100, 150, 100, 200),
            );
            draw_rectangle_lines(
                draw_x,
                draw_y,
                placeholder_width,
                placeholder_height,
                2.0,
                Color::from_rgba(50, 100, 50, 255),
            );
        }
    }

    /// Render a map object with a horizontal shake offset (for trees being chopped)
    pub(super) fn render_map_object_shaking(
        &self,
        obj: &MapObject,
        shake_offset: f32,
        tile_z: f32,
        camera: &Camera,
    ) {
        let (screen_x, screen_y) = world_to_screen_z(
            obj.tile_x as f32 + 0.5,
            obj.tile_y as f32 + 0.5,
            tile_z,
            camera,
        );
        let zoom = camera.zoom;

        if let Some((texture, source_rect)) = self.get_object_sprite(obj.gid) {
            // Check if this is an animated sprite
            let sprite_id = obj.gid.wrapping_sub(OBJECTS_FIRSTGID);
            let (source_rect, tex_width, tex_height) =
                if let Some(&frames) = self.animated_objects.get(&sprite_id) {
                    let (anim_rect, frame_w) = Self::get_animated_source_rect(source_rect, frames);
                    let h = source_rect.map(|r| r.h).unwrap_or(texture.height());
                    (anim_rect, frame_w, h)
                } else {
                    let (w, h) = if let Some(r) = source_rect {
                        (r.w, r.h)
                    } else {
                        (texture.width(), texture.height())
                    };
                    (source_rect, w, h)
                };

            let scaled_width = (tex_width * zoom).round();
            let scaled_height = (tex_height * zoom).round();

            // Apply shake offset to x position
            let draw_x = (screen_x - scaled_width / 2.0 + shake_offset * zoom).round();
            let draw_y = (screen_y - scaled_height).round();

            draw_texture_ex(
                texture,
                draw_x,
                draw_y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                    source: source_rect,
                    ..Default::default()
                },
            );
        }
    }

    /// Render a falling tree (tree that was just chopped down)
    pub(super) fn render_falling_tree(
        &self,
        gid: u32,
        tile_x: i32,
        tile_y: i32,
        tile_z: f32,
        angle: f32,
        alpha: f32,
        _y_offset: f32,
        camera: &Camera,
    ) {
        // The pivot point (tree base) should stay fixed at pivot_x, pivot_y
        let (pivot_x, pivot_y) =
            world_to_screen_z(tile_x as f32 + 0.5, tile_y as f32 + 0.5, tile_z, camera);
        let zoom = camera.zoom;

        if let Some((texture, source_rect)) = self.get_object_sprite(gid) {
            // Check if this is an animated sprite
            let sprite_id = gid.wrapping_sub(OBJECTS_FIRSTGID);
            let (source_rect, tex_width, tex_height) =
                if let Some(&frames) = self.animated_objects.get(&sprite_id) {
                    let (anim_rect, frame_w) = Self::get_animated_source_rect(source_rect, frames);
                    let h = source_rect.map(|r| r.h).unwrap_or(texture.height());
                    (anim_rect, frame_w, h)
                } else {
                    let (w, h) = if let Some(r) = source_rect {
                        (r.w, r.h)
                    } else {
                        (texture.width(), texture.height())
                    };
                    (source_rect, w, h)
                };

            let w = tex_width * zoom;
            let h = tex_height * zoom;

            // Rotate each corner around the pivot (bottom-center of tree)
            // Corners relative to pivot: TL, TR, BR, BL
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let rotate = |rx: f32, ry: f32| -> Vec3 {
                Vec3::new(
                    pivot_x + rx * cos_a - ry * sin_a,
                    pivot_y + rx * sin_a + ry * cos_a,
                    0.0,
                )
            };

            let tl = rotate(-w / 2.0, -h);
            let tr = rotate(w / 2.0, -h);
            let br = rotate(w / 2.0, 0.0);
            let bl = rotate(-w / 2.0, 0.0);

            // UV coordinates
            let (u0, v0, u1, v1) = if let Some(r) = source_rect {
                (
                    r.x / texture.width(),
                    r.y / texture.height(),
                    (r.x + r.w) / texture.width(),
                    (r.y + r.h) / texture.height(),
                )
            } else {
                (0.0, 0.0, 1.0, 1.0)
            };

            let color_arr: [u8; 4] = [255, 255, 255, (alpha * 255.0) as u8];

            // Build mesh with 4 vertices and 2 triangles
            let mesh = Mesh {
                vertices: vec![
                    Vertex {
                        position: tl,
                        uv: Vec2::new(u0, v0),
                        color: color_arr,
                        normal: Vec4::ZERO,
                    },
                    Vertex {
                        position: tr,
                        uv: Vec2::new(u1, v0),
                        color: color_arr,
                        normal: Vec4::ZERO,
                    },
                    Vertex {
                        position: br,
                        uv: Vec2::new(u1, v1),
                        color: color_arr,
                        normal: Vec4::ZERO,
                    },
                    Vertex {
                        position: bl,
                        uv: Vec2::new(u0, v1),
                        color: color_arr,
                        normal: Vec4::ZERO,
                    },
                ],
                indices: vec![0, 1, 2, 0, 2, 3],
                texture: Some(texture.clone()),
            };

            draw_mesh(&mesh);
        }
    }

    /// Render a crumbling rock — simple shrink + fade in place
    pub(super) fn render_crumbling_rock(
        &self,
        gid: u32,
        tile_x: i32,
        tile_y: i32,
        tile_z: f32,
        scale: f32,
        alpha: f32,
        camera: &Camera,
    ) {
        let (base_x, base_y) =
            world_to_screen_z(tile_x as f32 + 0.5, tile_y as f32 + 0.5, tile_z, camera);
        let zoom = camera.zoom;

        if let Some((texture, source_rect)) = self.get_object_sprite(gid) {
            // Check if this is an animated sprite
            let sprite_id = gid.wrapping_sub(OBJECTS_FIRSTGID);
            let (source_rect, tex_width, tex_height) =
                if let Some(&frames) = self.animated_objects.get(&sprite_id) {
                    let (anim_rect, frame_w) = Self::get_animated_source_rect(source_rect, frames);
                    let h = source_rect.map(|r| r.h).unwrap_or(texture.height());
                    (anim_rect, frame_w, h)
                } else {
                    let (w, h) = if let Some(r) = source_rect {
                        (r.w, r.h)
                    } else {
                        (texture.width(), texture.height())
                    };
                    (source_rect, w, h)
                };

            let scaled_width = (tex_width * zoom * scale).round();
            let scaled_height = (tex_height * zoom * scale).round();

            // Anchor at base center — rock shrinks downward into ground
            let draw_x = (base_x - scaled_width / 2.0).round();
            let draw_y = (base_y - scaled_height).round();

            draw_texture_ex(
                texture,
                draw_x,
                draw_y,
                Color::new(1.0, 1.0, 1.0, alpha),
                DrawTextureParams {
                    dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                    source: source_rect,
                    ..Default::default()
                },
            );
        }
    }

    /// Render a wall on a tile edge
    pub(super) fn render_wall(&self, wall: &Wall, tile_z: f32, camera: &Camera) {
        let zoom = camera.zoom;

        // Get the tile's top vertex screen position (same as mapper)
        // Use exact coordinates to avoid rounding errors
        let (sx, sy) = world_to_screen_exact(wall.tile_x as f32, wall.tile_y as f32, camera);
        // Apply Z offset to raise wall to tile elevation
        let screen_x = sx;
        let screen_y = sy - tile_z * (TILE_HEIGHT / 2.0) * zoom;

        // Tiles are centered on their world_to_screen position, so
        // bottom vertex is at center + half tile height (not full height)
        // Round to pixel grid to avoid subpixel jitter
        let bottom_vertex_x = screen_x.round();
        let bottom_vertex_y = (screen_y + (TILE_HEIGHT / 2.0) * zoom).round();

        // Try to get the wall sprite for this gid
        if let Some((texture, source_rect)) = self.get_wall_sprite(wall.gid) {
            // Check if this is an animated wall sprite
            let sprite_id = wall.gid.wrapping_sub(WALLS_FIRSTGID);
            let (source_rect, tex_width, tex_height) =
                if let Some(&frames) = self.animated_walls.get(&sprite_id) {
                    let (anim_rect, frame_w) = Self::get_animated_source_rect(source_rect, frames);
                    let h = source_rect.map(|r| r.h).unwrap_or(texture.height());
                    (anim_rect, frame_w, h)
                } else {
                    let (w, h) = if let Some(r) = source_rect {
                        (r.w, r.h)
                    } else {
                        (texture.width(), texture.height())
                    };
                    (source_rect, w, h)
                };

            let scaled_width = (tex_width * zoom).round();
            let scaled_height = (tex_height * zoom).round();

            let (draw_x, draw_y) = match wall.edge {
                WallEdge::Down => {
                    // Bottom-right corner of sprite at bottom vertex
                    (
                        bottom_vertex_x - scaled_width,
                        bottom_vertex_y - scaled_height,
                    )
                }
                WallEdge::Right => {
                    // Bottom-left corner of sprite at bottom vertex
                    (bottom_vertex_x, bottom_vertex_y - scaled_height)
                }
            };

            draw_texture_ex(
                texture,
                draw_x.round(),
                draw_y.round(),
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                    source: source_rect,
                    ..Default::default()
                },
            );
        }
    }
}
