use super::*;

impl Renderer {
    pub(super) fn render_player(
        &self,
        player: &Player,
        is_local: bool,
        is_selected: bool,
        is_hovered: bool,
        camera: &Camera,
        item_registry: &crate::game::item_registry::ItemRegistry,
        ground_z: f32,
        // When set, every appearance layer is drawn with this colour (a spectral
        // tint with built-in alpha) and decorations (shadow/selection) are skipped.
        // Used to render Reaper soul-wraiths as a ghost-copy of the player.
        tint_override: Option<Color>,
    ) {
        let (screen_x, screen_y) = world_to_screen_z(player.x, player.y, player.z, camera);
        let zoom = camera.zoom;

        // Scaled sprite dimensions
        let scaled_sprite_width = SPRITE_WIDTH * zoom;
        let scaled_sprite_height = SPRITE_HEIGHT * zoom;

        // Dead players are faded
        let alpha = if player.is_dead { 100 } else { 255 };

        // Selection highlight (draw first, behind player, at ground level)
        if is_selected && !player.is_dead && tint_override.is_none() {
            self.render_tile_selection(player.x, player.y, player.z, camera);
        }

        // Vertical offset for sitting on chair (shift up to center on tile)
        let sit_offset_y =
            if player.animation.state == crate::render::animation::AnimationState::SittingChair {
                10.0 * zoom
            } else {
                0.0
            };

        // Draw shadow at ground level, scaled by height above ground (skipped for ghosts)
        if tint_override.is_none() {
            let height_above_ground = (player.z - ground_z).max(0.0);
            let shadow_scale = (1.0 - height_above_ground * 0.15).clamp(0.4, 1.0);
            let shadow_alpha = ((60.0 - height_above_ground * 12.0).clamp(15.0, 60.0)) as u8;
            let (shadow_sx, shadow_sy) = world_to_screen_z(player.x, player.y, ground_z, camera);
            draw_ellipse(
                shadow_sx,
                shadow_sy + 4.0 * zoom,
                16.0 * zoom * shadow_scale,
                7.0 * zoom * shadow_scale,
                0.0,
                Color::from_rgba(0, 0, 0, shadow_alpha),
            );
        }

        // Try to render sprite based on player's appearance, fall back to colored circle
        if let Some((player_texture, player_offset)) =
            self.get_player_sprite(&player.gender, &player.skin)
        {
            let coords = player.animation.get_sprite_coords();
            let (player_atlas_x, player_atlas_y) = player_offset.unwrap_or((0.0, 0.0));
            let (src_x, src_y, src_w, src_h) = coords.to_source_rect();

            // Tint for local player distinction (slight green tint), or the
            // spectral ghost tint when rendering a soul-wraith.
            let tint = if let Some(c) = tint_override {
                c
            } else if is_local {
                Color::from_rgba(220, 255, 220, alpha)
            } else {
                Color::from_rgba(255, 255, 255, alpha)
            };

            // Position sprite so feet are at screen_y
            let draw_x = screen_x - scaled_sprite_width / 2.0;
            let draw_y = screen_y - scaled_sprite_height + 16.0 * zoom + sit_offset_y; // Offset to align feet with tile (8px base + 8px centering adjustment)

            // Get player gender for gender-specific offsets
            let player_gender = Gender::from_wire(&player.gender);

            // Calculate weapon frame info if weapon is equipped (hidden when sitting)
            let is_sitting = matches!(
                player.animation.state,
                crate::render::animation::AnimationState::SittingChair
                    | crate::render::animation::AnimationState::SittingGround
            );
            let weapon_info = player
                .equipped_weapon
                .as_ref()
                .filter(|_| !is_sitting)
                .and_then(|weapon_id| {
                    let sprite_key = item_registry.get_sprite_key(weapon_id);
                    self.weapon_sprites
                        .get(sprite_key)
                        .map(|(tex, atlas_offset)| {
                            let anim_frame = player.animation.frame as u32;
                            let weapon_frame = get_weapon_frame(
                                player.animation.state,
                                player.animation.direction,
                                anim_frame,
                            );
                            let (offset_x, offset_y) = get_weapon_offset(
                                player.animation.state,
                                player.animation.direction,
                                anim_frame,
                                player_gender,
                            );
                            // Get weapon frame size from manifest, fallback to default
                            let (fw, fh) = self
                                .weapon_frame_sizes
                                .get(sprite_key)
                                .copied()
                                .unwrap_or((WEAPON_SPRITE_WIDTH, WEAPON_SPRITE_HEIGHT));
                            (tex, atlas_offset, weapon_frame, offset_x, offset_y, fw, fh)
                        })
                });

            // Scaled weapon dimensions (per-weapon)
            let (scaled_weapon_width, scaled_weapon_height, wf_width, wf_height) = weapon_info
                .as_ref()
                .map(|(_, _, _, _, _, fw, fh)| (*fw * zoom, *fh * zoom, *fw, *fh))
                .unwrap_or((
                    WEAPON_SPRITE_WIDTH * zoom,
                    WEAPON_SPRITE_HEIGHT * zoom,
                    WEAPON_SPRITE_WIDTH,
                    WEAPON_SPRITE_HEIGHT,
                ));

            // Draw weapon under-layer (before player sprite)
            if let Some((weapon_sprite, atlas_offset, ref weapon_frame, offset_x, offset_y, _, _)) =
                weapon_info
            {
                let (atlas_x, atlas_y) = atlas_offset.unwrap_or((0.0, 0.0));
                let weapon_src_x = atlas_x + weapon_frame.frame_under as f32 * wf_width;
                let weapon_draw_x = draw_x + offset_x * zoom;
                let weapon_draw_y = draw_y + offset_y * zoom;

                draw_texture_ex(
                    weapon_sprite,
                    weapon_draw_x,
                    weapon_draw_y,
                    tint,
                    DrawTextureParams {
                        source: Some(Rect::new(weapon_src_x, atlas_y, wf_width, wf_height)),
                        dest_size: Some(Vec2::new(scaled_weapon_width, scaled_weapon_height)),
                        flip_x: weapon_frame.flip_h,
                        ..Default::default()
                    },
                );
            }

            // Draw back static items BEHIND player (for down/right directions - tip peeks out)
            if let Some(ref back_item_id) = player.equipped_back {
                let back_sprite_key = item_registry.get_sprite_key(back_item_id);
                if let Some((equip_texture, equip_offset)) =
                    self.equipment_sprites.get(back_sprite_key)
                {
                    // Check if this is an offhand item based on dimensions
                    let (equip_w, equip_h) = self
                        .equipment_sprites
                        .get_dimensions(back_sprite_key)
                        .unwrap_or((equip_texture.width(), equip_texture.height()));
                    let is_offhand = equip_w > equip_h * 8.0;
                    if !is_offhand {
                        let anim_frame = player.animation.frame as u32;
                        let back_frame = get_back_static_frame(player.animation.direction);
                        if back_frame.render_behind {
                            let (back_offset_x, back_offset_y) = get_back_static_offset(
                                player.animation.state,
                                player.animation.direction,
                                anim_frame,
                                player_gender,
                            );
                            let (atlas_x, atlas_y) = equip_offset.unwrap_or((0.0, 0.0));
                            let back_src_x =
                                atlas_x + back_frame.frame as f32 * BACK_STATIC_SPRITE_WIDTH;
                            let scaled_back_width = BACK_STATIC_SPRITE_WIDTH * zoom;
                            let scaled_back_height = BACK_STATIC_SPRITE_HEIGHT * zoom;
                            let back_draw_x = draw_x + back_offset_x * zoom;
                            let back_draw_y = draw_y + back_offset_y * zoom;

                            draw_texture_ex(
                                equip_texture,
                                back_draw_x,
                                back_draw_y,
                                tint,
                                DrawTextureParams {
                                    source: Some(Rect::new(
                                        back_src_x,
                                        atlas_y,
                                        BACK_STATIC_SPRITE_WIDTH,
                                        BACK_STATIC_SPRITE_HEIGHT,
                                    )),
                                    dest_size: Some(Vec2::new(
                                        scaled_back_width,
                                        scaled_back_height,
                                    )),
                                    flip_x: back_frame.flip_h,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }
            }

            // Draw player sprite
            draw_texture_ex(
                player_texture,
                draw_x,
                draw_y,
                tint,
                DrawTextureParams {
                    source: Some(Rect::new(
                        player_atlas_x + src_x,
                        player_atlas_y + src_y,
                        src_w,
                        src_h,
                    )),
                    dest_size: Some(Vec2::new(scaled_sprite_width, scaled_sprite_height)),
                    flip_x: coords.flip_h,
                    ..Default::default()
                },
            );

            // Draw hair and head equipment (after base sprite, before body armor)
            // Check if player has head equipment that we can render with shader
            let head_item_id_ref = player.equipped_head.as_ref();
            let head_sprite_data = head_item_id_ref.and_then(|head_item_id| {
                let head_sprite_key = item_registry.get_sprite_key(head_item_id);
                let (tex, offset) = self.equipment_sprites.get(head_sprite_key)?;
                let (w, h) = self.equipment_sprites.get_dimensions(head_sprite_key)?;
                Some((tex, offset, w, h))
            });

            let has_shader = self.head_hair_material.is_some();

            if let Some((head_texture, head_offset, _head_rect_w, _head_rect_h)) = head_sprite_data
            {
                // Player has head equipment - use shader compositing if available
                if has_shader {
                    if let (Some(style), Some(color)) = (player.hair_style, player.hair_color) {
                        let hair_key = format!("{}_{}", player.gender, style);
                        if let Some((hair_texture, hair_atlas_offset)) =
                            self.hair_sprites.get(&hair_key)
                        {
                            // For UV calculations, we need the FULL texture dimensions, not sprite rect dimensions
                            // get_dimensions() returns sprite rect size in atlas mode, but UVs need full texture size
                            let hair_full_tex_w = hair_texture.width();
                            let hair_full_tex_h = hair_texture.height();
                            let head_full_tex_w = head_texture.width();
                            let head_full_tex_h = head_texture.height();

                            // Get atlas offsets (0,0 if not using atlas)
                            let (hair_atlas_x, hair_atlas_y) =
                                hair_atlas_offset.unwrap_or((0.0, 0.0));
                            let (head_atlas_x, head_atlas_y) = head_offset.unwrap_or((0.0, 0.0));

                            // Calculate hair frame info
                            let is_back = matches!(
                                player.animation.direction,
                                Direction::Up | Direction::Left
                            );
                            let frame_index = color * 2 + if is_back { 1 } else { 0 };
                            let hair_src_x = frame_index as f32 * HAIR_SPRITE_WIDTH;

                            // Calculate hair offsets using gender-aware function
                            let anim_frame = player.animation.frame as u32;
                            let (hair_pos_x, hair_pos_y) = get_hair_offset(
                                player.animation.state,
                                player.animation.direction,
                                anim_frame,
                                player_gender,
                                coords.flip_h,
                            );

                            // Calculate head frame info
                            let head_frame = get_head_frame(player.animation.direction);
                            let (head_pos_x, head_pos_y) = get_head_offset(
                                player.animation.state,
                                player.animation.direction,
                                anim_frame,
                                player_gender,
                            );
                            let head_src_x = head_frame.frame as f32 * HEAD_SPRITE_WIDTH;

                            // Calculate pixel offset from head origin to hair origin (in unscaled pixels)
                            // Hair is centered: hair_x = (SPRITE_WIDTH - HAIR_SPRITE_WIDTH) / 2 + hair_pos_x = 3 + hair_pos_x
                            // Head uses head_pos_x directly
                            let hair_base_x = (SPRITE_WIDTH - HAIR_SPRITE_WIDTH) / 2.0 + hair_pos_x;
                            let hair_base_y = hair_pos_y; // sit offset already included in get_hair_offset
                            let delta_x = hair_base_x - head_pos_x;
                            let delta_y = hair_base_y - head_pos_y;

                            // Compute UV transform for shader
                            // The shader needs to transform head UV to hair UV
                            // UVs are in full-texture coords [0,1], so we use full texture dimensions

                            // Head source rect in normalized UV (including atlas offset)
                            let head_uv_x = (head_atlas_x + head_src_x) / head_full_tex_w;
                            let head_uv_y = head_atlas_y / head_full_tex_h;
                            let _head_uv_w = HEAD_SPRITE_WIDTH / head_full_tex_w;
                            let _head_uv_h = HEAD_SPRITE_HEIGHT / head_full_tex_h;

                            // Hair source rect in normalized UV (including atlas offset)
                            let hair_uv_x = (hair_atlas_x + hair_src_x) / hair_full_tex_w;
                            let hair_uv_y = hair_atlas_y / hair_full_tex_h;
                            let hair_uv_w = HAIR_SPRITE_WIDTH / hair_full_tex_w;
                            let hair_uv_h = HAIR_SPRITE_HEIGHT / hair_full_tex_h;

                            // The transform: given head UV (u, v) in full texture coords
                            // 1. Normalize to head frame: local = (u - head_uv_x) / head_uv_w, (v - head_uv_y) / head_uv_h
                            // 2. To pixels: pixel = local * (HEAD_SPRITE_WIDTH, HEAD_SPRITE_HEIGHT)
                            // 3. Offset: hair_pixel = pixel - (delta_x, delta_y)
                            // 4. To hair local: hair_local = hair_pixel / (HAIR_SPRITE_WIDTH, HAIR_SPRITE_HEIGHT)
                            // 5. To hair UV: hair_uv = hair_uv_x + hair_local.x * hair_uv_w, hair_uv_y + hair_local.y * hair_uv_h

                            // Combining and simplifying (see derivation in comments above):
                            // hair_uv.x = offset_x + u * scale_x
                            // hair_uv.y = offset_y + v * scale_y

                            let scale_x = head_full_tex_w * hair_uv_w / HAIR_SPRITE_WIDTH;
                            let scale_y = head_full_tex_h * hair_uv_h / HAIR_SPRITE_HEIGHT;
                            let offset_x = hair_uv_x
                                - head_uv_x * scale_x
                                - delta_x * hair_uv_w / HAIR_SPRITE_WIDTH;
                            let offset_y = hair_uv_y
                                - head_uv_y * scale_y
                                - delta_y * hair_uv_h / HAIR_SPRITE_HEIGHT;

                            // Set up shader
                            let material = self.head_hair_material.as_ref().unwrap();
                            material.set_texture("HairTexture", hair_texture.clone());
                            material.set_uniform(
                                "HairUvTransform",
                                [offset_x, offset_y, scale_x, scale_y],
                            );
                            material.set_uniform("Tint", [1.0f32, 1.0f32, 1.0f32, 1.0f32]);
                            gl_use_material(material);

                            // Draw head with shader active
                            let scaled_head_width = HEAD_SPRITE_WIDTH * zoom;
                            let scaled_head_height = HEAD_SPRITE_HEIGHT * zoom;
                            let head_draw_x = draw_x + head_pos_x * zoom;
                            let head_draw_y = draw_y + head_pos_y * zoom;

                            draw_texture_ex(
                                head_texture,
                                head_draw_x,
                                head_draw_y,
                                tint,
                                DrawTextureParams {
                                    source: Some(Rect::new(
                                        head_atlas_x + head_src_x,
                                        head_atlas_y,
                                        HEAD_SPRITE_WIDTH,
                                        HEAD_SPRITE_HEIGHT,
                                    )),
                                    dest_size: Some(Vec2::new(
                                        scaled_head_width,
                                        scaled_head_height,
                                    )),
                                    flip_x: head_frame.flip_h,
                                    ..Default::default()
                                },
                            );

                            gl_use_default_material();
                        }
                    } else {
                        // No hair, just draw head normally
                        let anim_frame = player.animation.frame as u32;
                        let head_frame = get_head_frame(player.animation.direction);
                        let (head_pos_offset_x, head_pos_offset_y) = get_head_offset(
                            player.animation.state,
                            player.animation.direction,
                            anim_frame,
                            player_gender,
                        );
                        let (head_atlas_x, head_atlas_y) = head_offset.unwrap_or((0.0, 0.0));
                        let head_src_x = head_frame.frame as f32 * HEAD_SPRITE_WIDTH;
                        let scaled_head_width = HEAD_SPRITE_WIDTH * zoom;
                        let scaled_head_height = HEAD_SPRITE_HEIGHT * zoom;
                        let head_draw_x = draw_x + head_pos_offset_x * zoom;
                        let head_draw_y = draw_y + head_pos_offset_y * zoom;

                        draw_texture_ex(
                            head_texture,
                            head_draw_x,
                            head_draw_y,
                            tint,
                            DrawTextureParams {
                                source: Some(Rect::new(
                                    head_atlas_x + head_src_x,
                                    head_atlas_y,
                                    HEAD_SPRITE_WIDTH,
                                    HEAD_SPRITE_HEIGHT,
                                )),
                                dest_size: Some(Vec2::new(scaled_head_width, scaled_head_height)),
                                flip_x: head_frame.flip_h,
                                ..Default::default()
                            },
                        );
                    }
                } else {
                    // No shader available, draw hair then head (hair will show through transparent areas)
                    // Draw hair first
                    if let (Some(style), Some(color)) = (player.hair_style, player.hair_color) {
                        let hair_key = format!("{}_{}", player.gender, style);
                        if let Some((hair_tex, hair_atlas_offset)) =
                            self.hair_sprites.get(&hair_key)
                        {
                            let is_back = matches!(
                                player.animation.direction,
                                Direction::Up | Direction::Left
                            );
                            let frame_index = color * 2 + if is_back { 1 } else { 0 };
                            let (hair_atlas_x, hair_atlas_y) =
                                hair_atlas_offset.unwrap_or((0.0, 0.0));
                            let hair_src_x = hair_atlas_x + frame_index as f32 * HAIR_SPRITE_WIDTH;
                            let scaled_hair_width = HAIR_SPRITE_WIDTH * zoom;
                            let scaled_hair_height = HAIR_SPRITE_HEIGHT * zoom;

                            // Calculate hair offsets using gender-aware function
                            let anim_frame = player.animation.frame as u32;
                            let (hair_pos_offset_x, hair_pos_offset_y) = get_hair_offset(
                                player.animation.state,
                                player.animation.direction,
                                anim_frame,
                                player_gender,
                                coords.flip_h,
                            );

                            let hair_draw_x = draw_x
                                + (scaled_sprite_width - scaled_hair_width) / 2.0
                                + hair_pos_offset_x * zoom;
                            let hair_draw_y = draw_y + hair_pos_offset_y * zoom;

                            draw_texture_ex(
                                hair_tex,
                                hair_draw_x,
                                hair_draw_y,
                                tint,
                                DrawTextureParams {
                                    source: Some(Rect::new(
                                        hair_src_x,
                                        hair_atlas_y,
                                        HAIR_SPRITE_WIDTH,
                                        HAIR_SPRITE_HEIGHT,
                                    )),
                                    dest_size: Some(Vec2::new(
                                        scaled_hair_width,
                                        scaled_hair_height,
                                    )),
                                    flip_x: coords.flip_h,
                                    ..Default::default()
                                },
                            );
                        }
                    }

                    // Then draw head on top
                    let anim_frame = player.animation.frame as u32;
                    let head_frame = get_head_frame(player.animation.direction);
                    let (head_pos_offset_x, head_pos_offset_y) = get_head_offset(
                        player.animation.state,
                        player.animation.direction,
                        anim_frame,
                        player_gender,
                    );
                    let (head_atlas_x, head_atlas_y) = head_offset.unwrap_or((0.0, 0.0));
                    let head_src_x = head_atlas_x + head_frame.frame as f32 * HEAD_SPRITE_WIDTH;
                    let scaled_head_width = HEAD_SPRITE_WIDTH * zoom;
                    let scaled_head_height = HEAD_SPRITE_HEIGHT * zoom;
                    let head_draw_x = draw_x + head_pos_offset_x * zoom;
                    let head_draw_y = draw_y + head_pos_offset_y * zoom;

                    draw_texture_ex(
                        head_texture,
                        head_draw_x,
                        head_draw_y,
                        tint,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                head_src_x,
                                head_atlas_y,
                                HEAD_SPRITE_WIDTH,
                                HEAD_SPRITE_HEIGHT,
                            )),
                            dest_size: Some(Vec2::new(scaled_head_width, scaled_head_height)),
                            flip_x: head_frame.flip_h,
                            ..Default::default()
                        },
                    );
                }
            }
            // Hair without head equipment is drawn after body armor (see below)

            // Draw equipment overlay (body armor)
            if let Some(ref body_item_id) = player.equipped_body {
                let body_sprite_key = item_registry.get_sprite_key(body_item_id);
                if let Some((body_texture, body_atlas_offset)) =
                    self.equipment_sprites.get(body_sprite_key)
                {
                    // Check if this is a new-style single-row body armor sprite (width > height * 2)
                    // Body armor sprites are wider (16 frames) so use a more aggressive ratio check
                    let (body_w, body_h) = self
                        .equipment_sprites
                        .get_dimensions(body_sprite_key)
                        .unwrap_or((body_texture.width(), body_texture.height()));
                    let is_single_row = body_w > body_h * 2.0;
                    let (body_atlas_x, body_atlas_y) = body_atlas_offset.unwrap_or((0.0, 0.0));

                    if is_single_row {
                        // New single-row body armor format
                        let anim_frame = player.animation.frame as u32;
                        let armor_frame = get_body_armor_frame(
                            player.animation.state,
                            player.animation.direction,
                            anim_frame,
                        );
                        let (armor_offset_x, armor_offset_y) = get_body_armor_offset(
                            player.animation.state,
                            player.animation.direction,
                            anim_frame,
                            player_gender,
                        );

                        let armor_src_x =
                            body_atlas_x + armor_frame.frame as f32 * BODY_ARMOR_SPRITE_WIDTH;
                        let scaled_armor_width = BODY_ARMOR_SPRITE_WIDTH * zoom;
                        let scaled_armor_height = BODY_ARMOR_SPRITE_HEIGHT * zoom;

                        let armor_draw_x = draw_x + armor_offset_x * zoom;
                        let armor_draw_y = draw_y + armor_offset_y * zoom;

                        draw_texture_ex(
                            body_texture,
                            armor_draw_x,
                            armor_draw_y,
                            tint,
                            DrawTextureParams {
                                source: Some(Rect::new(
                                    armor_src_x,
                                    body_atlas_y,
                                    BODY_ARMOR_SPRITE_WIDTH,
                                    BODY_ARMOR_SPRITE_HEIGHT,
                                )),
                                dest_size: Some(Vec2::new(scaled_armor_width, scaled_armor_height)),
                                flip_x: armor_frame.flip_h,
                                ..Default::default()
                            },
                        );
                    } else {
                        // Old grid-style body armor format (matches player sprite sheet layout)
                        draw_texture_ex(
                            body_texture,
                            draw_x,
                            draw_y,
                            tint, // Same tint as player
                            DrawTextureParams {
                                source: Some(Rect::new(
                                    body_atlas_x + src_x,
                                    body_atlas_y + src_y,
                                    src_w,
                                    src_h,
                                )),
                                dest_size: Some(Vec2::new(
                                    scaled_sprite_width,
                                    scaled_sprite_height,
                                )),
                                flip_x: coords.flip_h,
                                ..Default::default()
                            },
                        );
                    }
                }
            }

            // Draw hair on top of body armor (when no head equipment)
            if player.equipped_head.is_none() {
                if let (Some(style), Some(color)) = (player.hair_style, player.hair_color) {
                    let hair_key = format!("{}_{}", player.gender, style);
                    if let Some((hair_tex, hair_atlas_offset)) = self.hair_sprites.get(&hair_key) {
                        let is_back = crate::render::animation::is_up_or_left_direction(
                            player.animation.direction,
                        );
                        let frame_index = color * 2 + if is_back { 1 } else { 0 };
                        let (hair_atlas_x, hair_atlas_y) = hair_atlas_offset.unwrap_or((0.0, 0.0));
                        let hair_src_x = hair_atlas_x + frame_index as f32 * HAIR_SPRITE_WIDTH;
                        let scaled_hair_width = HAIR_SPRITE_WIDTH * zoom;
                        let scaled_hair_height = HAIR_SPRITE_HEIGHT * zoom;

                        // Calculate hair offsets using gender-aware function
                        let anim_frame = player.animation.frame as u32;
                        let (hair_pos_offset_x, hair_pos_offset_y) = get_hair_offset(
                            player.animation.state,
                            player.animation.direction,
                            anim_frame,
                            player_gender,
                            coords.flip_h,
                        );

                        let hair_draw_x = draw_x
                            + (scaled_sprite_width - scaled_hair_width) / 2.0
                            + hair_pos_offset_x * zoom;
                        let hair_draw_y = draw_y + hair_pos_offset_y * zoom;

                        draw_texture_ex(
                            hair_tex,
                            hair_draw_x,
                            hair_draw_y,
                            tint,
                            DrawTextureParams {
                                source: Some(Rect::new(
                                    hair_src_x,
                                    hair_atlas_y,
                                    HAIR_SPRITE_WIDTH,
                                    HAIR_SPRITE_HEIGHT,
                                )),
                                dest_size: Some(Vec2::new(scaled_hair_width, scaled_hair_height)),
                                flip_x: coords.flip_h,
                                ..Default::default()
                            },
                        );
                    }
                }
            }

            // Draw equipment overlay (boots)
            if let Some(ref feet_item_id) = player.equipped_feet {
                let feet_sprite_key = item_registry.get_sprite_key(feet_item_id);
                if let Some((feet_texture, feet_atlas_offset)) =
                    self.equipment_sprites.get(feet_sprite_key)
                {
                    // Check if this is a new-style single-row boot sprite (width > height)
                    let (feet_w, feet_h) = self
                        .equipment_sprites
                        .get_dimensions(feet_sprite_key)
                        .unwrap_or((feet_texture.width(), feet_texture.height()));
                    let is_single_row = feet_w > feet_h;
                    let (feet_atlas_x, feet_atlas_y) = feet_atlas_offset.unwrap_or((0.0, 0.0));

                    if is_single_row {
                        // New single-row boot format
                        let anim_frame = player.animation.frame as u32;
                        let boot_frame = get_boot_frame(
                            player.animation.state,
                            player.animation.direction,
                            anim_frame,
                        );
                        let (boot_offset_x, boot_offset_y) = get_boot_offset(
                            player.animation.state,
                            player.animation.direction,
                            anim_frame,
                            player_gender,
                        );

                        let boot_src_x = feet_atlas_x + boot_frame.frame as f32 * BOOT_SPRITE_WIDTH;
                        let scaled_boot_width = BOOT_SPRITE_WIDTH * zoom;
                        let scaled_boot_height = BOOT_SPRITE_HEIGHT * zoom;

                        let boot_draw_x = draw_x + boot_offset_x * zoom;
                        let boot_draw_y = draw_y + boot_offset_y * zoom;

                        draw_texture_ex(
                            feet_texture,
                            boot_draw_x,
                            boot_draw_y,
                            tint,
                            DrawTextureParams {
                                source: Some(Rect::new(
                                    boot_src_x,
                                    feet_atlas_y,
                                    BOOT_SPRITE_WIDTH,
                                    BOOT_SPRITE_HEIGHT,
                                )),
                                dest_size: Some(Vec2::new(scaled_boot_width, scaled_boot_height)),
                                flip_x: boot_frame.flip_h,
                                ..Default::default()
                            },
                        );
                    } else {
                        // Old grid-style boot format (matches player sprite sheet layout)
                        draw_texture_ex(
                            feet_texture,
                            draw_x,
                            draw_y,
                            tint,
                            DrawTextureParams {
                                source: Some(Rect::new(
                                    feet_atlas_x + src_x,
                                    feet_atlas_y + src_y,
                                    src_w,
                                    src_h,
                                )),
                                dest_size: Some(Vec2::new(
                                    scaled_sprite_width,
                                    scaled_sprite_height,
                                )),
                                flip_x: coords.flip_h,
                                ..Default::default()
                            },
                        );
                    }
                }
            }

            // Draw back slot equipment (quiver, shield, etc.)
            if let Some(ref back_item_id) = player.equipped_back {
                let back_sprite_key = item_registry.get_sprite_key(back_item_id);
                if let Some((back_texture, back_atlas_offset)) =
                    self.equipment_sprites.get(back_sprite_key)
                {
                    // Detect sprite type by dimensions:
                    // - 16-frame offhand (shield): width > height * 8 (very wide strip)
                    // - 2-frame static back (quiver): width < height * 4 (narrow strip)
                    let (back_w, back_h) = self
                        .equipment_sprites
                        .get_dimensions(back_sprite_key)
                        .unwrap_or((back_texture.width(), back_texture.height()));
                    let is_offhand = back_w > back_h * 8.0;
                    let (back_atlas_x, back_atlas_y) = back_atlas_offset.unwrap_or((0.0, 0.0));

                    if is_offhand {
                        // 16-frame offhand item (shield)
                        let anim_frame = player.animation.frame as u32;
                        let offhand_frame = get_offhand_frame(
                            player.animation.state,
                            player.animation.direction,
                            anim_frame,
                        );
                        let (offhand_offset_x, offhand_offset_y) = get_offhand_offset(
                            player.animation.state,
                            player.animation.direction,
                            anim_frame,
                            player_gender,
                        );

                        let offhand_src_x =
                            back_atlas_x + offhand_frame.frame as f32 * OFFHAND_SPRITE_WIDTH;
                        let scaled_offhand_width = OFFHAND_SPRITE_WIDTH * zoom;
                        let scaled_offhand_height = OFFHAND_SPRITE_HEIGHT * zoom;

                        let offhand_draw_x = draw_x + offhand_offset_x * zoom;
                        let offhand_draw_y = draw_y + offhand_offset_y * zoom;

                        draw_texture_ex(
                            back_texture,
                            offhand_draw_x,
                            offhand_draw_y,
                            tint,
                            DrawTextureParams {
                                source: Some(Rect::new(
                                    offhand_src_x,
                                    back_atlas_y,
                                    OFFHAND_SPRITE_WIDTH,
                                    OFFHAND_SPRITE_HEIGHT,
                                )),
                                dest_size: Some(Vec2::new(
                                    scaled_offhand_width,
                                    scaled_offhand_height,
                                )),
                                flip_x: offhand_frame.flip_h,
                                ..Default::default()
                            },
                        );
                    } else {
                        // 2-frame static back item (quiver, cape)
                        let anim_frame = player.animation.frame as u32;
                        let back_frame = get_back_static_frame(player.animation.direction);

                        // Only render here if visible and NOT rendering behind player
                        // (behind rendering happens before player sprite)
                        if back_frame.visible && !back_frame.render_behind {
                            let (back_pos_offset_x, back_pos_offset_y) = get_back_static_offset(
                                player.animation.state,
                                player.animation.direction,
                                anim_frame,
                                player_gender,
                            );

                            let back_src_x =
                                back_atlas_x + back_frame.frame as f32 * BACK_STATIC_SPRITE_WIDTH;
                            let scaled_back_width = BACK_STATIC_SPRITE_WIDTH * zoom;
                            let scaled_back_height = BACK_STATIC_SPRITE_HEIGHT * zoom;

                            let back_draw_x = draw_x + back_pos_offset_x * zoom;
                            let back_draw_y = draw_y + back_pos_offset_y * zoom;

                            draw_texture_ex(
                                back_texture,
                                back_draw_x,
                                back_draw_y,
                                tint,
                                DrawTextureParams {
                                    source: Some(Rect::new(
                                        back_src_x,
                                        back_atlas_y,
                                        BACK_STATIC_SPRITE_WIDTH,
                                        BACK_STATIC_SPRITE_HEIGHT,
                                    )),
                                    dest_size: Some(Vec2::new(
                                        scaled_back_width,
                                        scaled_back_height,
                                    )),
                                    flip_x: back_frame.flip_h,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }
            }

            // Draw weapon over-layer (after equipment, for attack frame 2 front overlay)
            if let Some((weapon_sprite, atlas_offset, ref weapon_frame, offset_x, offset_y, _, _)) =
                weapon_info
            {
                if let Some(frame_over) = weapon_frame.frame_over {
                    let (atlas_x, atlas_y) = atlas_offset.unwrap_or((0.0, 0.0));
                    let weapon_src_x = atlas_x + frame_over as f32 * wf_width;
                    let weapon_draw_x = draw_x + offset_x * zoom;
                    let weapon_draw_y = draw_y + offset_y * zoom;

                    draw_texture_ex(
                        weapon_sprite,
                        weapon_draw_x,
                        weapon_draw_y,
                        tint,
                        DrawTextureParams {
                            source: Some(Rect::new(weapon_src_x, atlas_y, wf_width, wf_height)),
                            dest_size: Some(Vec2::new(scaled_weapon_width, scaled_weapon_height)),
                            flip_x: weapon_frame.flip_h,
                            ..Default::default()
                        },
                    );
                }
            }
        } else {
            // Fallback: colored circle
            let base_color = if is_local {
                self.local_player_color
            } else {
                self.player_color
            };
            let color = Color::from_rgba(
                (base_color.r * 255.0) as u8,
                (base_color.g * 255.0) as u8,
                (base_color.b * 255.0) as u8,
                alpha,
            );

            let radius = 12.0 * zoom;
            draw_circle(screen_x, screen_y - radius, radius, color);

            // Direction indicator
            let (dx, dy) = player.direction.to_unit_vector();
            let indicator_len = 15.0 * zoom;
            draw_line(
                screen_x,
                screen_y - radius,
                screen_x + dx * indicator_len,
                screen_y - radius + dy * indicator_len * 0.5, // Flatten for isometric
                2.0 * zoom,
                WHITE,
            );
        }

        // Player name (positioned just above head) - only show when hovered or selected
        let has_sprite = self
            .get_player_sprite(&player.gender, &player.skin)
            .is_some();
        let name_y_offset = if has_sprite {
            scaled_sprite_height - 8.0 * zoom
        } else {
            24.0 * zoom
        };

        let show_name = is_selected || is_hovered;
        // Name tag drawing is deferred to render_name_tags() so it appears above all map elements

        // Health bar - only show within 3 seconds of taking damage (and when not at full HP)
        let current_time = macroquad::time::get_time();
        let time_since_damage = current_time - player.last_damage_time;
        let show_health_bar = player.hp < player.max_hp && time_since_damage < 3.0;

        if show_health_bar {
            let bar_width = 32.0 * zoom;
            let bar_height = 6.0 * zoom;
            let bar_x = screen_x - bar_width / 2.0;
            // Position health bar where name would be if name isn't showing, otherwise above the name
            let bar_y = if show_name {
                screen_y - name_y_offset - 16.0 * zoom
            } else {
                screen_y - name_y_offset
            };
            let hp_ratio = player.hp as f32 / player.max_hp.max(1) as f32;

            self.draw_entity_health_bar(bar_x, bar_y, bar_width, bar_height, hp_ratio, zoom);
        }
    }

    /// Renders a semi-transparent silhouette of the player that's always visible.
    /// Composites all layers at full opacity onto an off-screen render target first,
    /// then draws the result with low alpha so equipment properly occludes skin.
    pub(super) fn render_player_silhouette(
        &self,
        player: &Player,
        camera: &Camera,
        item_registry: &crate::game::item_registry::ItemRegistry,
    ) {
        if player.is_dead {
            return;
        }

        // Skip silhouette on Android — render target switches are expensive on mobile GPUs
        if cfg!(target_os = "android") {
            return;
        }

        // Lazily create the render target
        {
            let mut rt_opt = self.silhouette_rt.borrow_mut();
            if rt_opt.is_none() {
                // Use sample_count: 0 to skip the resolve-texture path, which
                // calls glDrawBuffers — unavailable on WebGL 1.
                let rt = render_target_ex(
                    SILHOUETTE_RT_SIZE,
                    SILHOUETTE_RT_SIZE,
                    RenderTargetParams {
                        sample_count: 0,
                        depth: false,
                    },
                );
                rt.texture.set_filter(FilterMode::Nearest);
                *rt_opt = Some(rt);
            }
        }
        let rt = self.silhouette_rt.borrow().as_ref().unwrap().clone();

        // --- Phase 1: Composite all layers at full opacity onto the render target ---
        set_camera(&Camera2D {
            render_target: Some(rt.clone()),
            ..Camera2D::from_display_rect(Rect::new(
                0.0,
                0.0,
                SILHOUETTE_RT_SIZE as f32,
                SILHOUETTE_RT_SIZE as f32,
            ))
        });
        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));

        if let Some((player_texture, player_offset)) =
            self.get_player_sprite(&player.gender, &player.skin)
        {
            let coords = player.animation.get_sprite_coords();
            let (src_x, src_y, src_w, src_h) = coords.to_source_rect();
            let (player_atlas_x, player_atlas_y) = player_offset.unwrap_or((0.0, 0.0));

            // Draw at anchor position in the RT (1x scale, no zoom)
            let draw_x = SILHOUETTE_ANCHOR_X;
            let draw_y = SILHOUETTE_ANCHOR_Y;
            let player_gender = Gender::from_wire(&player.gender);

            // Calculate weapon frame info (hidden when sitting)
            let is_sitting_sil = matches!(
                player.animation.state,
                crate::render::animation::AnimationState::SittingChair
                    | crate::render::animation::AnimationState::SittingGround
            );
            let weapon_info = player
                .equipped_weapon
                .as_ref()
                .filter(|_| !is_sitting_sil)
                .and_then(|weapon_id| {
                    let sprite_key = item_registry.get_sprite_key(weapon_id);
                    self.weapon_sprites
                        .get(sprite_key)
                        .map(|(tex, atlas_offset)| {
                            let anim_frame = player.animation.frame as u32;
                            let weapon_frame = get_weapon_frame(
                                player.animation.state,
                                player.animation.direction,
                                anim_frame,
                            );
                            let (offset_x, offset_y) = get_weapon_offset(
                                player.animation.state,
                                player.animation.direction,
                                anim_frame,
                                player_gender,
                            );
                            let (fw, fh) = self
                                .weapon_frame_sizes
                                .get(sprite_key)
                                .copied()
                                .unwrap_or((WEAPON_SPRITE_WIDTH, WEAPON_SPRITE_HEIGHT));
                            (tex, atlas_offset, weapon_frame, offset_x, offset_y, fw, fh)
                        })
                });

            let (wf_width, wf_height) = weapon_info
                .as_ref()
                .map(|(_, _, _, _, _, fw, fh)| (*fw, *fh))
                .unwrap_or((WEAPON_SPRITE_WIDTH, WEAPON_SPRITE_HEIGHT));

            // Weapon under-layer
            if let Some((weapon_sprite, atlas_offset, ref weapon_frame, offset_x, offset_y, _, _)) =
                weapon_info
            {
                let (atlas_x, atlas_y) = atlas_offset.unwrap_or((0.0, 0.0));
                let weapon_src_x = atlas_x + weapon_frame.frame_under as f32 * wf_width;
                draw_texture_ex(
                    weapon_sprite,
                    draw_x + offset_x,
                    draw_y + offset_y,
                    WHITE,
                    DrawTextureParams {
                        source: Some(Rect::new(weapon_src_x, atlas_y, wf_width, wf_height)),
                        dest_size: Some(Vec2::new(wf_width, wf_height)),
                        flip_x: weapon_frame.flip_h,
                        ..Default::default()
                    },
                );
            }

            // Player base sprite
            draw_texture_ex(
                player_texture,
                draw_x,
                draw_y,
                WHITE,
                DrawTextureParams {
                    source: Some(Rect::new(
                        player_atlas_x + src_x,
                        player_atlas_y + src_y,
                        src_w,
                        src_h,
                    )),
                    dest_size: Some(Vec2::new(SPRITE_WIDTH, SPRITE_HEIGHT)),
                    flip_x: coords.flip_h,
                    ..Default::default()
                },
            );

            // Body armor
            if let Some(ref body_item_id) = player.equipped_body {
                let body_sprite_key = item_registry.get_sprite_key(body_item_id);
                if let Some((body_texture, body_atlas_offset)) =
                    self.equipment_sprites.get(body_sprite_key)
                {
                    let (body_w, body_h) = self
                        .equipment_sprites
                        .get_dimensions(body_sprite_key)
                        .unwrap_or((body_texture.width(), body_texture.height()));
                    let is_single_row = body_w > body_h * 2.0;
                    let (body_atlas_x, body_atlas_y) = body_atlas_offset.unwrap_or((0.0, 0.0));

                    if is_single_row {
                        let anim_frame = player.animation.frame as u32;
                        let armor_frame = get_body_armor_frame(
                            player.animation.state,
                            player.animation.direction,
                            anim_frame,
                        );
                        let (armor_offset_x, armor_offset_y) = get_body_armor_offset(
                            player.animation.state,
                            player.animation.direction,
                            anim_frame,
                            player_gender,
                        );
                        let armor_src_x =
                            body_atlas_x + armor_frame.frame as f32 * BODY_ARMOR_SPRITE_WIDTH;
                        draw_texture_ex(
                            body_texture,
                            draw_x + armor_offset_x,
                            draw_y + armor_offset_y,
                            WHITE,
                            DrawTextureParams {
                                source: Some(Rect::new(
                                    armor_src_x,
                                    body_atlas_y,
                                    BODY_ARMOR_SPRITE_WIDTH,
                                    BODY_ARMOR_SPRITE_HEIGHT,
                                )),
                                dest_size: Some(Vec2::new(
                                    BODY_ARMOR_SPRITE_WIDTH,
                                    BODY_ARMOR_SPRITE_HEIGHT,
                                )),
                                flip_x: armor_frame.flip_h,
                                ..Default::default()
                            },
                        );
                    } else {
                        draw_texture_ex(
                            body_texture,
                            draw_x,
                            draw_y,
                            WHITE,
                            DrawTextureParams {
                                source: Some(Rect::new(
                                    body_atlas_x + src_x,
                                    body_atlas_y + src_y,
                                    src_w,
                                    src_h,
                                )),
                                dest_size: Some(Vec2::new(SPRITE_WIDTH, SPRITE_HEIGHT)),
                                flip_x: coords.flip_h,
                                ..Default::default()
                            },
                        );
                    }
                }
            }

            // Hair (when no head equipment) or headgear
            if let Some(ref head_item_id) = player.equipped_head {
                let head_sprite_key = item_registry.get_sprite_key(head_item_id);
                if let Some((head_texture, head_atlas_offset)) =
                    self.equipment_sprites.get(head_sprite_key)
                {
                    let anim_frame = player.animation.frame as u32;
                    let head_frame = get_head_frame(player.animation.direction);
                    let (head_pos_offset_x, head_pos_offset_y) = get_head_offset(
                        player.animation.state,
                        player.animation.direction,
                        anim_frame,
                        player_gender,
                    );
                    let (head_atlas_x, head_atlas_y) = head_atlas_offset.unwrap_or((0.0, 0.0));
                    let head_src_x = head_atlas_x + head_frame.frame as f32 * HEAD_SPRITE_WIDTH;

                    draw_texture_ex(
                        head_texture,
                        draw_x + head_pos_offset_x,
                        draw_y + head_pos_offset_y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                head_src_x,
                                head_atlas_y,
                                HEAD_SPRITE_WIDTH,
                                HEAD_SPRITE_HEIGHT,
                            )),
                            dest_size: Some(Vec2::new(HEAD_SPRITE_WIDTH, HEAD_SPRITE_HEIGHT)),
                            flip_x: head_frame.flip_h,
                            ..Default::default()
                        },
                    );
                }
            } else if let (Some(style), Some(color)) = (player.hair_style, player.hair_color) {
                let hair_key = format!("{}_{}", player.gender, style);
                if let Some((hair_tex, hair_atlas_offset)) = self.hair_sprites.get(&hair_key) {
                    let is_back = crate::render::animation::is_up_or_left_direction(
                        player.animation.direction,
                    );
                    let frame_index = color * 2 + if is_back { 1 } else { 0 };
                    let (hair_atlas_x, hair_atlas_y) = hair_atlas_offset.unwrap_or((0.0, 0.0));
                    let hair_src_x = hair_atlas_x + frame_index as f32 * HAIR_SPRITE_WIDTH;

                    let anim_frame = player.animation.frame as u32;
                    let (hair_pos_offset_x, hair_pos_offset_y) = get_hair_offset(
                        player.animation.state,
                        player.animation.direction,
                        anim_frame,
                        player_gender,
                        coords.flip_h,
                    );
                    let hair_draw_x =
                        draw_x + (SPRITE_WIDTH - HAIR_SPRITE_WIDTH) / 2.0 + hair_pos_offset_x;
                    let hair_draw_y = draw_y + hair_pos_offset_y;

                    draw_texture_ex(
                        hair_tex,
                        hair_draw_x,
                        hair_draw_y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                hair_src_x,
                                hair_atlas_y,
                                HAIR_SPRITE_WIDTH,
                                HAIR_SPRITE_HEIGHT,
                            )),
                            dest_size: Some(Vec2::new(HAIR_SPRITE_WIDTH, HAIR_SPRITE_HEIGHT)),
                            flip_x: coords.flip_h,
                            ..Default::default()
                        },
                    );
                }
            }

            // Boots
            if let Some(ref feet_item_id) = player.equipped_feet {
                let feet_sprite_key = item_registry.get_sprite_key(feet_item_id);
                if let Some((feet_texture, feet_atlas_offset)) =
                    self.equipment_sprites.get(feet_sprite_key)
                {
                    let (feet_w, feet_h) = self
                        .equipment_sprites
                        .get_dimensions(feet_sprite_key)
                        .unwrap_or((feet_texture.width(), feet_texture.height()));
                    let is_single_row = feet_w > feet_h;
                    let (feet_atlas_x, feet_atlas_y) = feet_atlas_offset.unwrap_or((0.0, 0.0));

                    if is_single_row {
                        let anim_frame = player.animation.frame as u32;
                        let boot_frame = get_boot_frame(
                            player.animation.state,
                            player.animation.direction,
                            anim_frame,
                        );
                        let (boot_offset_x, boot_offset_y) = get_boot_offset(
                            player.animation.state,
                            player.animation.direction,
                            anim_frame,
                            player_gender,
                        );
                        let boot_src_x = feet_atlas_x + boot_frame.frame as f32 * BOOT_SPRITE_WIDTH;
                        draw_texture_ex(
                            feet_texture,
                            draw_x + boot_offset_x,
                            draw_y + boot_offset_y,
                            WHITE,
                            DrawTextureParams {
                                source: Some(Rect::new(
                                    boot_src_x,
                                    feet_atlas_y,
                                    BOOT_SPRITE_WIDTH,
                                    BOOT_SPRITE_HEIGHT,
                                )),
                                dest_size: Some(Vec2::new(BOOT_SPRITE_WIDTH, BOOT_SPRITE_HEIGHT)),
                                flip_x: boot_frame.flip_h,
                                ..Default::default()
                            },
                        );
                    } else {
                        draw_texture_ex(
                            feet_texture,
                            draw_x,
                            draw_y,
                            WHITE,
                            DrawTextureParams {
                                source: Some(Rect::new(
                                    feet_atlas_x + src_x,
                                    feet_atlas_y + src_y,
                                    src_w,
                                    src_h,
                                )),
                                dest_size: Some(Vec2::new(SPRITE_WIDTH, SPRITE_HEIGHT)),
                                flip_x: coords.flip_h,
                                ..Default::default()
                            },
                        );
                    }
                }
            }

            // Weapon over-layer
            if let Some((weapon_sprite, atlas_offset, ref weapon_frame, offset_x, offset_y, _, _)) =
                weapon_info
            {
                if let Some(frame_over) = weapon_frame.frame_over {
                    let (atlas_x, atlas_y) = atlas_offset.unwrap_or((0.0, 0.0));
                    let weapon_src_x = atlas_x + frame_over as f32 * wf_width;
                    draw_texture_ex(
                        weapon_sprite,
                        draw_x + offset_x,
                        draw_y + offset_y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(weapon_src_x, atlas_y, wf_width, wf_height)),
                            dest_size: Some(Vec2::new(wf_width, wf_height)),
                            flip_x: weapon_frame.flip_h,
                            ..Default::default()
                        },
                    );
                }
            }
        }

        // --- Phase 2: Draw the composited RT to screen with silhouette tint ---
        set_default_camera();

        let (screen_x, screen_y) = world_to_screen_z(player.x, player.y, player.z, camera);
        let zoom = camera.zoom;
        let sit_offset_y =
            if player.animation.state == crate::render::animation::AnimationState::SittingChair {
                10.0 * zoom
            } else {
                0.0
            };
        let player_draw_x = screen_x - SPRITE_WIDTH * zoom / 2.0;
        let player_draw_y = screen_y - SPRITE_HEIGHT * zoom + 16.0 * zoom + sit_offset_y;

        let rt_screen_x = player_draw_x - SILHOUETTE_ANCHOR_X * zoom;
        let rt_screen_y = player_draw_y - SILHOUETTE_ANCHOR_Y * zoom;

        draw_texture_ex(
            &rt.texture,
            rt_screen_x,
            rt_screen_y,
            Color::from_rgba(255, 255, 255, 50),
            DrawTextureParams {
                dest_size: Some(Vec2::new(
                    SILHOUETTE_RT_SIZE as f32 * zoom,
                    SILHOUETTE_RT_SIZE as f32 * zoom,
                )),
                flip_y: true,
                ..Default::default()
            },
        );
    }
}
