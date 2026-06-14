use super::*;

impl Renderer {
    pub(super) fn get_player_sprite(
        &self,
        gender: &str,
        skin: &str,
    ) -> Option<(&Texture2D, Option<(f32, f32)>)> {
        let key = format!("{}_{}", gender, skin);
        self.player_sprites
            .get(&key)
            // Fallback to male_tan if sprite not found
            .or_else(|| self.player_sprites.get("male_tan"))
    }

    /// Get the sprite texture for a map object by its gid
    /// sprite_id = gid - OBJECTS_FIRSTGID (e.g., gid 1263 → sprite "101")
    pub(super) fn get_object_sprite(&self, gid: u32) -> Option<(&Texture2D, Option<Rect>)> {
        if gid < OBJECTS_FIRSTGID {
            return None;
        }
        let sprite_id = gid - OBJECTS_FIRSTGID;
        let mut buf = [0u8; 12];
        let key = u32_to_str(sprite_id, &mut buf);
        self.object_sprites.get(key)
    }

    /// Get the sprite texture for a wall by its gid
    /// sprite_id = gid - WALLS_FIRSTGID (e.g., gid 102 → sprite "101")
    pub(super) fn get_wall_sprite(&self, gid: u32) -> Option<(&Texture2D, Option<Rect>)> {
        if gid < WALLS_FIRSTGID {
            return None;
        }
        let sprite_id = gid - WALLS_FIRSTGID;
        let mut buf = [0u8; 12];
        let key = u32_to_str(sprite_id, &mut buf);
        self.wall_sprites.get(key)
    }

    /// Draw text with pixel font for sharp rendering
    /// Uses multi-size bitmap font for crisp text at any size.
    /// Font size is scaled by `font_scale` (set to ui_scale during UI rendering).
    pub fn draw_text_sharp(&self, text: &str, x: f32, y: f32, font_size: f32, color: Color) {
        let scaled_size = self.scaled_font_size(font_size);
        self.font
            .draw_text(text, x.floor(), y.floor(), scaled_size, color);
    }

    /// Apply font_scale and snap to nearest multiple of 8 for pixel-perfect rendering
    pub(super) fn scaled_font_size(&self, font_size: f32) -> f32 {
        let scale = self.font_scale.get();
        if (scale - 1.0).abs() < 0.01 {
            font_size
        } else {
            ((font_size * scale) / 8.0).round() * 8.0
        }
    }

    /// Get reference to player sprites for sharing with UI screens
    pub fn player_sprites(&self) -> &SpritesheetStore {
        &self.player_sprites
    }

    /// Get reference to hair sprites for sharing with UI screens
    pub fn hair_sprites(&self) -> &SpritesheetStore {
        &self.hair_sprites
    }

    /// Get reference to equipment sprites for sharing with UI screens
    pub fn equipment_sprites(&self) -> &SpritesheetStore {
        &self.equipment_sprites
    }

    /// Get reference to weapon sprites for sharing with UI screens
    pub fn weapon_sprites(&self) -> &SpritesheetStore {
        &self.weapon_sprites
    }

    /// Get reference to weapon frame sizes for sharing with UI screens
    pub fn weapon_frame_sizes(&self) -> &HashMap<String, (f32, f32)> {
        &self.weapon_frame_sizes
    }

    /// Get reference to font for sharing with UI screens
    pub fn font(&self) -> &BitmapFont {
        &self.font
    }

    /// Measure text with pixel font (scaled by font_scale)
    pub(crate) fn measure_text_sharp(&self, text: &str, font_size: f32) -> TextDimensions {
        let scaled_size = self.scaled_font_size(font_size);
        let font_key = (scaled_size * 100.0).round() as i32;

        if let Some(bucket) = self.text_measure_cache.borrow().get(&font_key) {
            if let Some(cached) = bucket.get(text) {
                return *cached;
            }
        }

        let measured = self.font.measure_text(text, scaled_size);
        let mut cache = self.text_measure_cache.borrow_mut();
        let bucket = cache.entry(font_key).or_default();
        if bucket.len() < TEXT_MEASURE_CACHE_BUCKET_LIMIT {
            bucket.insert(text.to_string(), measured);
        }
        measured
    }

    /// Draw text with word wrapping to fit within max_width
    /// Returns the total height used
    pub(crate) fn draw_text_wrapped(
        &self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: Color,
        max_width: f32,
        line_height: f32,
    ) -> f32 {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut current_line = String::new();
        let mut current_y = y;
        let space_width = self.measure_text_sharp(" ", font_size).width;

        for word in words {
            let word_width = self.measure_text_sharp(word, font_size).width;
            let line_width = if current_line.is_empty() {
                word_width
            } else {
                self.measure_text_sharp(&current_line, font_size).width + space_width + word_width
            };

            if line_width > max_width && !current_line.is_empty() {
                // Draw current line and start new one
                self.draw_text_sharp(&current_line, x, current_y, font_size, color);
                current_y += line_height;
                current_line = word.to_string();
            } else {
                // Add word to current line
                if current_line.is_empty() {
                    current_line = word.to_string();
                } else {
                    current_line.push(' ');
                    current_line.push_str(word);
                }
            }
        }

        // Draw remaining line
        if !current_line.is_empty() {
            self.draw_text_sharp(&current_line, x, current_y, font_size, color);
            current_y += line_height;
        }

        current_y - y
    }

    /// Get the UV rect for a tile ID in the tileset
    /// Tiled uses 1-indexed tile IDs (0 = empty)
    pub(super) fn get_tile_uv(&self, tile_id: u32) -> Option<Rect> {
        if tile_id == 0 {
            return None;
        }

        let tileset = self.tileset.as_ref()?;
        let id = tile_id - 1; // Convert to 0-indexed

        let col = id % TILESET_COLUMNS;
        let row = id / TILESET_COLUMNS;

        let x = col as f32 * TILESET_TILE_WIDTH;
        let y = row as f32 * TILESET_TILE_HEIGHT;

        Some(Rect::new(
            x / tileset.width(),
            y / tileset.height(),
            TILESET_TILE_WIDTH / tileset.width(),
            TILESET_TILE_HEIGHT / tileset.height(),
        ))
    }

    /// Draw a tile sprite from the tileset
    pub(super) fn draw_tile_sprite(
        &self,
        screen_x: f32,
        screen_y: f32,
        tile_id: u32,
        zoom: f32,
        world_pos: Option<(f32, f32)>,
        water_effects: bool,
    ) {
        let scaled_width = TILE_WIDTH * zoom;
        let scaled_height = TILE_HEIGHT * zoom;

        // Apply water shader for water tiles (tile ID 418)
        let is_water = tile_id == 418 && water_effects;
        if is_water {
            if let Some(ref mat) = self.water_material {
                mat.set_uniform("Time", get_time() as f32);
                gl_use_material(mat);
            }
        }

        if let (Some(tileset), Some(uv)) = (&self.tileset, self.get_tile_uv(tile_id)) {
            let draw_x = screen_x - scaled_width / 2.0;
            let draw_y = screen_y - scaled_height / 2.0;
            let source = Rect::new(
                uv.x * tileset.width(),
                uv.y * tileset.height(),
                TILESET_TILE_WIDTH,
                TILESET_TILE_HEIGHT,
            );

            draw_texture_ex(
                tileset,
                draw_x,
                draw_y,
                WHITE,
                DrawTextureParams {
                    source: Some(source),
                    dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                    ..Default::default()
                },
            );

            if is_water && self.water_material.is_some() {
                gl_use_default_material();
            }

            // Draw wave overlay on top of water tiles
            if is_water {
                if let (Some(ref mat), Some((wx, wy))) = (&self.water_overlay_material, world_pos) {
                    mat.set_uniform("Time", get_time() as f32);
                    mat.set_uniform("WorldPos", (wx, wy));
                    gl_use_material(mat);

                    draw_texture_ex(
                        tileset,
                        draw_x,
                        draw_y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(source),
                            dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                            ..Default::default()
                        },
                    );

                    gl_use_default_material();
                }
            }
        } else {
            let color = get_tile_color(tile_id);
            self.draw_isometric_tile(screen_x, screen_y, color, zoom);

            if is_water && self.water_material.is_some() {
                gl_use_default_material();
            }
        }
    }

    /// Draw a tile sprite with a color tint (for height-based lighting)
    pub(super) fn draw_tile_sprite_tinted(
        &self,
        screen_x: f32,
        screen_y: f32,
        tile_id: u32,
        zoom: f32,
        world_pos: Option<(f32, f32)>,
        water_effects: bool,
        tint: Color,
    ) {
        let scaled_width = TILE_WIDTH * zoom;
        let scaled_height = TILE_HEIGHT * zoom;

        let is_water = tile_id == 418 && water_effects;
        if is_water {
            if let Some(ref mat) = self.water_material {
                mat.set_uniform("Time", get_time() as f32);
                gl_use_material(mat);
            }
        }

        if let (Some(tileset), Some(uv)) = (&self.tileset, self.get_tile_uv(tile_id)) {
            let draw_x = screen_x - scaled_width / 2.0;
            let draw_y = screen_y - scaled_height / 2.0;
            let source = Rect::new(
                uv.x * tileset.width(),
                uv.y * tileset.height(),
                TILESET_TILE_WIDTH,
                TILESET_TILE_HEIGHT,
            );

            draw_texture_ex(
                tileset,
                draw_x,
                draw_y,
                tint,
                DrawTextureParams {
                    source: Some(source),
                    dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                    ..Default::default()
                },
            );

            if is_water && self.water_material.is_some() {
                gl_use_default_material();
            }

            if is_water {
                if let (Some(ref mat), Some((wx, wy))) = (&self.water_overlay_material, world_pos) {
                    mat.set_uniform("Time", get_time() as f32);
                    mat.set_uniform("WorldPos", (wx, wy));
                    gl_use_material(mat);
                    draw_texture_ex(
                        tileset,
                        draw_x,
                        draw_y,
                        tint,
                        DrawTextureParams {
                            source: Some(source),
                            dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                            ..Default::default()
                        },
                    );
                    gl_use_default_material();
                }
            }
        } else {
            let color = get_tile_color(tile_id);
            let tinted = Color::new(
                color.r * tint.r,
                color.g * tint.g,
                color.b * tint.b,
                color.a,
            );
            self.draw_isometric_tile(screen_x, screen_y, tinted, zoom);

            if is_water && self.water_material.is_some() {
                gl_use_default_material();
            }
        }
    }
}
