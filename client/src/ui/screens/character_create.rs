use super::*;

pub struct CharacterCreateScreen {
    session: AuthSession,
    name: String,
    gender_index: usize,
    skin_index: usize,
    hair_style_index: Option<usize>, // None = bald, Some(0-2) = style
    hair_color_index: usize,         // 0-6
    error_message: Option<String>,
    auth_client: AuthClient,
    font: BitmapFont,
    active_field: CreateField,
    player_sprites: SpritesheetStore,
    hair_sprites: SpritesheetStore,
    scroll_y: f32,
    last_touch_y: Option<f32>,
    touch_detected: bool,
    #[cfg(target_arch = "wasm32")]
    loading: bool,
}

const HAIR_STYLES: usize = 6; // 0-5
const HAIR_COLORS: usize = 10; // 0-9 (20 frames / 2 front-back pairs)

#[derive(PartialEq, Clone, Copy)]
enum CreateField {
    Name,
    Gender,
    Skin,
    HairStyle,
    HairColor,
}

impl CharacterCreateScreen {
    pub fn new(session: AuthSession, server_url: &str) -> Self {
        Self {
            session,
            name: String::new(),
            gender_index: 0,
            skin_index: 0,
            hair_style_index: None,
            hair_color_index: 0,
            error_message: None,
            auth_client: AuthClient::new(server_url),
            font: BitmapFont::default(),
            active_field: CreateField::Name,
            player_sprites: SpritesheetStore::Individual(HashMap::new()),
            hair_sprites: SpritesheetStore::Individual(HashMap::new()),
            scroll_y: 0.0,
            last_touch_y: None,
            touch_detected: false,
            #[cfg(target_arch = "wasm32")]
            loading: false,
        }
    }

    /// Use pre-loaded assets from the renderer (avoids duplicate loading)
    pub fn use_renderer_assets(
        &mut self,
        font: BitmapFont,
        player: SpritesheetStore,
        hair: SpritesheetStore,
    ) {
        self.font = font;
        self.player_sprites = player;
        self.hair_sprites = hair;
    }

    pub async fn load_font(&mut self) {
        // If assets were pre-loaded via use_renderer_assets(), skip loading
        if self.player_sprites.len() > 0 {
            return;
        }

        {
            self.font =
                BitmapFont::load_or_default("assets/fonts/monogram/ttf/monogram-extended.ttf")
                    .await;
            self.player_sprites = load_player_sprites().await;

            let mut hair_map = HashMap::new();
            for style in 0..HAIR_STYLES as i32 {
                // Male hair sprites
                let path = asset_path(&format!("assets/sprites/hair/hair_{}.png", style));
                match load_texture(&path).await {
                    Ok(tex) => {
                        tex.set_filter(FilterMode::Nearest);
                        hair_map.insert(format!("male_{}", style), tex);
                    }
                    Err(e) => {
                        log::warn!("Failed to load hair sprite {}: {}", path, e);
                    }
                }
                // Female hair sprites
                let path = asset_path(&format!("assets/sprites/hair/hair_female_{}.png", style));
                match load_texture(&path).await {
                    Ok(tex) => {
                        tex.set_filter(FilterMode::Nearest);
                        hair_map.insert(format!("female_{}", style), tex);
                    }
                    Err(e) => {
                        log::warn!("Failed to load female hair sprite {}: {}", path, e);
                    }
                }
            }
            self.hair_sprites = SpritesheetStore::Individual(hair_map);
        }
    }

    fn draw_text_sharp(&self, text: &str, x: f32, y: f32, font_size: f32, color: Color) {
        self.font.draw_text(text, x, y, font_size, color);
    }

    fn measure_text_sharp(&self, text: &str, font_size: f32) -> TextDimensions {
        self.font.measure_text(text, font_size)
    }

    fn handle_name_input(&mut self) {
        while let Some(c) = get_char_pressed() {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                if self.name.len() < 16 {
                    self.name.push(c);
                    self.error_message = None;
                }
            }
        }

        if is_key_pressed(KeyCode::Backspace) {
            self.name.pop();
            self.error_message = None;
        }
    }
}

impl Screen for CharacterCreateScreen {
    fn update(&mut self, audio: &AudioManager) -> ScreenState {
        // WASM: poll pending requests
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(result) = self.auth_client.poll() {
                self.loading = false;
                match result {
                    AuthResult::CharacterCreated(Ok(char_info)) => {
                        self.session.characters.push(char_info);
                        return ScreenState::ToCharacterSelect(self.session.clone());
                    }
                    AuthResult::CharacterCreated(Err(e)) => {
                        self.error_message = Some(e.to_string());
                    }
                    _ => {}
                }
            }
        }

        let (sw, sh) = virtual_screen_size();
        let (input_pos, clicked, _is_touch) = get_input_state();
        let mx = input_pos.x;
        let my = input_pos.y;

        // Layout constants (must match render)
        let total_width = 460.0;
        let content_x = (sw - total_width) / 2.0;
        let field_height = 70.0;
        let content_height: f32 = 330.0;
        let max_content_height = content_height.min(sh - 80.0);
        let form_total_h: f32 = field_height * 4.0 + 10.0 + 36.0; // fields + button gap + button height
        let max_scroll = (form_total_h - max_content_height).max(0.0);

        // Handle scroll via mouse wheel
        let (_wheel_x, wheel_y) = mouse_wheel();
        if wheel_y != 0.0 {
            self.scroll_y = (self.scroll_y - wheel_y * 30.0).clamp(0.0, max_scroll);
        }

        // Handle touch drag scrolling (mobile)
        let touch_list: Vec<Touch> = touches();
        if !touch_list.is_empty() {
            self.touch_detected = true;
        }
        if let Some(touch) = touch_list.first() {
            let (_, vy) = screen_to_virtual(touch.position.x, touch.position.y);
            match touch.phase {
                TouchPhase::Started => {
                    self.last_touch_y = Some(vy);
                }
                TouchPhase::Moved => {
                    if let Some(last_y) = self.last_touch_y {
                        let delta = last_y - vy;
                        self.scroll_y = (self.scroll_y + delta).clamp(0.0, max_scroll);
                    }
                    self.last_touch_y = Some(vy);
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    self.last_touch_y = None;
                }
                _ => {}
            }
        } else {
            self.last_touch_y = None;
        }

        // Preview stays fixed, only form fields scroll
        let fixed_y = ((sh - max_content_height) / 2.0).max(50.0);
        let content_y = fixed_y - self.scroll_y;
        let preview_w = 140.0;
        let form_x = content_x + preview_w + 20.0;
        let form_w = 300.0;
        let half_width = (form_w - 10.0) / 2.0;

        // Handle name input when name field is active
        if self.active_field == CreateField::Name {
            self.handle_name_input();
        }

        // Mouse: Click on fields to focus
        if clicked {
            // Name field box
            let name_box_y = content_y + 20.0;
            if point_in_rect(mx, my, form_x, name_box_y, form_w, 36.0) {
                self.active_field = CreateField::Name;
                show_keyboard(true);
            }

            // Gender field box
            let gender_box_y = content_y + field_height + 20.0;
            if point_in_rect(mx, my, form_x, gender_box_y, form_w, 36.0) {
                self.active_field = CreateField::Gender;
                show_keyboard(false);

                // Check if clicked on left arrow area
                if point_in_rect(mx, my, form_x, gender_box_y, 50.0, 36.0) {
                    self.gender_index = if self.gender_index == 0 {
                        GENDERS.len() - 1
                    } else {
                        self.gender_index - 1
                    };
                }
                // Check if clicked on right arrow area
                if point_in_rect(mx, my, form_x + form_w - 50.0, gender_box_y, 50.0, 36.0) {
                    self.gender_index = (self.gender_index + 1) % GENDERS.len();
                }
            }

            // Skin field box
            let skin_box_y = content_y + field_height * 2.0 + 20.0;
            if point_in_rect(mx, my, form_x, skin_box_y, form_w, 36.0) {
                self.active_field = CreateField::Skin;
                show_keyboard(false);

                // Check if clicked on left arrow area
                if point_in_rect(mx, my, form_x, skin_box_y, 50.0, 36.0) {
                    self.skin_index = if self.skin_index == 0 {
                        SKINS.len() - 1
                    } else {
                        self.skin_index - 1
                    };
                }
                // Check if clicked on right arrow area
                if point_in_rect(mx, my, form_x + form_w - 50.0, skin_box_y, 50.0, 36.0) {
                    self.skin_index = (self.skin_index + 1) % SKINS.len();
                }
            }

            // Hair style field box (left half of hair row)
            let hair_box_y = content_y + field_height * 3.0 + 20.0;
            if point_in_rect(mx, my, form_x, hair_box_y, half_width, 36.0) {
                self.active_field = CreateField::HairStyle;
                show_keyboard(false);

                // Check if clicked on left arrow area
                if point_in_rect(mx, my, form_x, hair_box_y, 35.0, 36.0) {
                    self.hair_style_index = match self.hair_style_index {
                        None => Some(HAIR_STYLES - 1),
                        Some(0) => None,
                        Some(i) => Some(i - 1),
                    };
                }
                // Check if clicked on right arrow area
                if point_in_rect(mx, my, form_x + half_width - 35.0, hair_box_y, 35.0, 36.0) {
                    self.hair_style_index = match self.hair_style_index {
                        None => Some(0),
                        Some(i) if i >= HAIR_STYLES - 1 => None,
                        Some(i) => Some(i + 1),
                    };
                }
            }

            // Hair color field box (right half of hair row, only if hair style selected)
            let hair_color_x = form_x + half_width + 10.0;
            if self.hair_style_index.is_some()
                && point_in_rect(mx, my, hair_color_x, hair_box_y, half_width, 36.0)
            {
                self.active_field = CreateField::HairColor;
                show_keyboard(false);

                // Check if clicked on left arrow area
                if point_in_rect(mx, my, hair_color_x, hair_box_y, 35.0, 36.0) {
                    self.hair_color_index = if self.hair_color_index == 0 {
                        HAIR_COLORS - 1
                    } else {
                        self.hair_color_index - 1
                    };
                }
                // Check if clicked on right arrow area
                if point_in_rect(
                    mx,
                    my,
                    hair_color_x + half_width - 35.0,
                    hair_box_y,
                    35.0,
                    36.0,
                ) {
                    self.hair_color_index = (self.hair_color_index + 1) % HAIR_COLORS;
                }
            }

            // Buttons row
            let buttons_y = content_y + field_height * 4.0 + 10.0;
            let button_w = (form_w - 10.0) / 2.0;

            // Create button
            if point_in_rect(mx, my, form_x, buttons_y, button_w, 36.0) {
                show_keyboard(false);
                let name = self.name.trim();
                if name.len() < 2 {
                    self.error_message = Some("Name must be at least 2 characters".to_string());
                    return ScreenState::Continue;
                }
                if name.len() > 16 {
                    self.error_message = Some("Name must be at most 16 characters".to_string());
                    return ScreenState::Continue;
                }

                let gender = GENDERS[self.gender_index];
                let skin = SKINS[self.skin_index];
                let hair_style = self.hair_style_index.map(|i| i as i32);
                let hair_color = if self.hair_style_index.is_some() {
                    Some(self.hair_color_index as i32)
                } else {
                    None
                };

                #[cfg(not(target_arch = "wasm32"))]
                match self.auth_client.create_character(
                    &self.session.token,
                    name,
                    gender,
                    skin,
                    hair_style,
                    hair_color,
                ) {
                    Ok(char_info) => {
                        self.session.characters.push(char_info);
                        return ScreenState::ToCharacterSelect(self.session.clone());
                    }
                    Err(e) => {
                        self.error_message = Some(e.to_string());
                    }
                }
                #[cfg(target_arch = "wasm32")]
                if !self.auth_client.is_busy() {
                    self.loading = true;
                    self.auth_client.start_create_character(
                        &self.session.token,
                        name,
                        gender,
                        skin,
                        hair_style,
                        hair_color,
                    );
                }
            }

            // Cancel button
            let cancel_x = form_x + button_w + 10.0;
            if point_in_rect(mx, my, cancel_x, buttons_y, button_w, 36.0) {
                show_keyboard(false);
                return ScreenState::ToCharacterSelect(self.session.clone());
            }
        }

        // Keyboard: Navigate between fields with Tab or Up/Down
        if is_key_pressed(KeyCode::Tab) || is_key_pressed(KeyCode::Down) {
            audio.play_sfx("enter");
            self.active_field = match self.active_field {
                CreateField::Name => CreateField::Gender,
                CreateField::Gender => CreateField::Skin,
                CreateField::Skin => CreateField::HairStyle,
                CreateField::HairStyle => {
                    if self.hair_style_index.is_some() {
                        CreateField::HairColor
                    } else {
                        CreateField::Name
                    }
                }
                CreateField::HairColor => CreateField::Name,
            };
        }
        if is_key_pressed(KeyCode::Up) {
            audio.play_sfx("enter");
            self.active_field = match self.active_field {
                CreateField::Name => {
                    if self.hair_style_index.is_some() {
                        CreateField::HairColor
                    } else {
                        CreateField::HairStyle
                    }
                }
                CreateField::Gender => CreateField::Name,
                CreateField::Skin => CreateField::Gender,
                CreateField::HairStyle => CreateField::Skin,
                CreateField::HairColor => CreateField::HairStyle,
            };
        }

        // Keyboard: Left/Right or A/D to cycle options
        if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
            match self.active_field {
                CreateField::Gender => {
                    self.gender_index = if self.gender_index == 0 {
                        GENDERS.len() - 1
                    } else {
                        self.gender_index - 1
                    };
                }
                CreateField::Skin => {
                    self.skin_index = if self.skin_index == 0 {
                        SKINS.len() - 1
                    } else {
                        self.skin_index - 1
                    };
                }
                CreateField::HairStyle => {
                    self.hair_style_index = match self.hair_style_index {
                        None => Some(HAIR_STYLES - 1),
                        Some(0) => None,
                        Some(i) => Some(i - 1),
                    };
                }
                CreateField::HairColor => {
                    self.hair_color_index = if self.hair_color_index == 0 {
                        HAIR_COLORS - 1
                    } else {
                        self.hair_color_index - 1
                    };
                }
                _ => {}
            }
        }
        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
            match self.active_field {
                CreateField::Gender => {
                    self.gender_index = (self.gender_index + 1) % GENDERS.len();
                }
                CreateField::Skin => {
                    self.skin_index = (self.skin_index + 1) % SKINS.len();
                }
                CreateField::HairStyle => {
                    self.hair_style_index = match self.hair_style_index {
                        None => Some(0),
                        Some(i) if i >= HAIR_STYLES - 1 => None,
                        Some(i) => Some(i + 1),
                    };
                }
                CreateField::HairColor => {
                    self.hair_color_index = (self.hair_color_index + 1) % HAIR_COLORS;
                }
                _ => {}
            }
        }

        // Keyboard: Cancel
        if is_key_pressed(KeyCode::Escape) {
            show_keyboard(false);
            return ScreenState::ToCharacterSelect(self.session.clone());
        }

        // Keyboard: Create character
        if is_key_pressed(KeyCode::Enter) {
            audio.play_sfx("enter");
            let name = self.name.trim();
            if name.len() < 2 {
                self.error_message = Some("Name must be at least 2 characters".to_string());
                return ScreenState::Continue;
            }
            if name.len() > 16 {
                self.error_message = Some("Name must be at most 16 characters".to_string());
                return ScreenState::Continue;
            }

            let gender = GENDERS[self.gender_index];
            let skin = SKINS[self.skin_index];
            let hair_style = self.hair_style_index.map(|i| i as i32);
            let hair_color = if self.hair_style_index.is_some() {
                Some(self.hair_color_index as i32)
            } else {
                None
            };

            #[cfg(not(target_arch = "wasm32"))]
            match self.auth_client.create_character(
                &self.session.token,
                name,
                gender,
                skin,
                hair_style,
                hair_color,
            ) {
                Ok(_) => {
                    return ScreenState::ToCharacterSelect(self.session.clone());
                }
                Err(e) => {
                    self.error_message = Some(e.to_string());
                }
            }
            #[cfg(target_arch = "wasm32")]
            if !self.auth_client.is_busy() {
                self.loading = true;
                self.auth_client.start_create_character(
                    &self.session.token,
                    name,
                    gender,
                    skin,
                    hair_style,
                    hair_color,
                );
            }
        }

        ScreenState::Continue
    }

    fn render(&self) {
        let (sw, sh) = virtual_screen_size();
        let (input_pos, _, _) = get_input_state();
        let mx = input_pos.x;
        let my = input_pos.y;

        // Background
        clear_background(Color::from_rgba(25, 25, 35, 255));

        // Draw decorative elements
        for i in 0..15 {
            let alpha = 0.03 + (i as f32 * 0.005);
            let color = Color::new(0.2, 0.3, 0.4, alpha);
            draw_line(0.0, i as f32 * 50.0, sw, i as f32 * 50.0, 1.0, color);
        }

        // Title
        let title = "CREATE CHARACTER";
        let title_width = self.measure_text_sharp(title, 16.0).width;
        self.draw_text_sharp(title, (sw - title_width) / 2.0, 30.0, 16.0, WHITE);

        // Layout: Preview on left (fixed), form on right (scrollable)
        let total_width = 460.0; // Preview (140) + gap (20) + form (300)
        let content_x = (sw - total_width) / 2.0;
        let content_height: f32 = 330.0; // 4 fields * 70 + buttons area
        let max_content_height = content_height.min(sh - 80.0); // leave room for title + padding
        let fixed_y = ((sh - max_content_height) / 2.0).max(50.0);
        let form_y = fixed_y - self.scroll_y; // Form fields scroll

        // === LEFT SIDE: Character Preview (fixed) ===
        let preview_w = 140.0;
        let preview_h = 200.0;

        // Decorative frame around preview
        let frame_padding = 8.0;
        let frame_x = content_x - frame_padding;
        let frame_y = fixed_y - frame_padding;
        let frame_w = preview_w + frame_padding * 2.0;
        let frame_h = preview_h + frame_padding * 2.0;

        // Outer glow
        draw_rectangle(
            frame_x - 2.0,
            frame_y - 2.0,
            frame_w + 4.0,
            frame_h + 4.0,
            Color::from_rgba(60, 80, 120, 100),
        );
        // Frame background
        draw_rectangle(
            frame_x,
            frame_y,
            frame_w,
            frame_h,
            Color::from_rgba(40, 45, 60, 255),
        );
        // Inner preview area
        draw_rectangle(
            content_x,
            fixed_y,
            preview_w,
            preview_h,
            Color::from_rgba(20, 22, 30, 255),
        );
        // Frame border
        draw_rectangle_lines(
            frame_x,
            frame_y,
            frame_w,
            frame_h,
            2.0,
            Color::from_rgba(80, 100, 140, 255),
        );
        // Corner accents
        let accent_size = 8.0;
        let accent_color = Color::from_rgba(100, 140, 200, 255);
        draw_line(
            frame_x,
            frame_y + accent_size,
            frame_x + accent_size,
            frame_y,
            2.0,
            accent_color,
        );
        draw_line(
            frame_x + frame_w - accent_size,
            frame_y,
            frame_x + frame_w,
            frame_y + accent_size,
            2.0,
            accent_color,
        );
        draw_line(
            frame_x,
            frame_y + frame_h - accent_size,
            frame_x + accent_size,
            frame_y + frame_h,
            2.0,
            accent_color,
        );
        draw_line(
            frame_x + frame_w - accent_size,
            frame_y + frame_h,
            frame_x + frame_w,
            frame_y + frame_h - accent_size,
            2.0,
            accent_color,
        );

        // Draw character sprite preview (native pixel size, floor to avoid subpixel stretching)
        let sprite_x = (content_x + (preview_w - SPRITE_WIDTH) / 2.0).floor();
        let sprite_y = (fixed_y + (preview_h - SPRITE_HEIGHT) / 2.0 - 10.0).floor();
        let empty_equip = SpritesheetStore::Individual(HashMap::new());
        draw_character_preview(
            &self.player_sprites,
            &self.hair_sprites,
            &empty_equip,
            GENDERS[self.gender_index],
            SKINS[self.skin_index],
            self.hair_style_index.map(|i| i as i32),
            self.hair_color_index as i32,
            None,
            None,
            None,
            sprite_x,
            sprite_y,
        );

        // Preview label below character
        let preview_label = format!("{} {}", GENDERS[self.gender_index], SKINS[self.skin_index]);
        let label_width = self.measure_text_sharp(&preview_label, 16.0).width;
        self.draw_text_sharp(
            &preview_label,
            content_x + (preview_w - label_width) / 2.0,
            fixed_y + preview_h - 30.0,
            16.0,
            LIGHTGRAY,
        );

        // === RIGHT SIDE: Form Fields (clipped to preview area) ===
        let form_x = content_x + preview_w + 20.0;
        let form_w = 300.0;
        let field_height = 70.0;
        let clip_top = fixed_y;
        let clip_bottom = fixed_y + max_content_height;

        // Helper: check if a field row is visible in the clipped area
        // field_top is the y of the label, field extends to field_top + field_height
        let is_visible = |field_top: f32, height: f32| -> bool {
            field_top + height > clip_top && field_top < clip_bottom
        };

        // Name field
        let name_active = self.active_field == CreateField::Name;
        let name_y = form_y;
        if is_visible(name_y, field_height) {
            self.draw_text_sharp(
                "Name",
                form_x,
                name_y,
                16.0,
                if name_active { WHITE } else { GRAY },
            );

            let name_box_color = if name_active {
                Color::from_rgba(80, 120, 180, 255)
            } else {
                Color::from_rgba(60, 60, 80, 255)
            };
            draw_rectangle(form_x, name_y + 20.0, form_w, 36.0, name_box_color);
            draw_rectangle_lines(
                form_x,
                name_y + 20.0,
                form_w,
                36.0,
                2.0,
                if name_active { WHITE } else { GRAY },
            );

            let cursor = if name_active && (get_time() * 2.0) as i32 % 2 == 0 {
                "|"
            } else {
                ""
            };
            let name_display = if self.name.is_empty() && !name_active {
                "Enter name...".to_string()
            } else {
                format!("{}{}", self.name, cursor)
            };
            let text_color = if self.name.is_empty() && !name_active {
                DARKGRAY
            } else {
                WHITE
            };
            self.draw_text_sharp(
                &name_display,
                form_x + 10.0,
                name_y + 44.0,
                16.0,
                text_color,
            );
        }

        // Gender field
        let gender_active = self.active_field == CreateField::Gender;
        let gender_y = form_y + field_height;
        if is_visible(gender_y, field_height) {
            self.draw_text_sharp(
                "Gender",
                form_x,
                gender_y,
                16.0,
                if gender_active { WHITE } else { GRAY },
            );

            let gender_box_color = if gender_active {
                Color::from_rgba(80, 120, 180, 255)
            } else {
                Color::from_rgba(60, 60, 80, 255)
            };
            draw_rectangle(form_x, gender_y + 20.0, form_w, 36.0, gender_box_color);
            draw_rectangle_lines(
                form_x,
                gender_y + 20.0,
                form_w,
                36.0,
                2.0,
                if gender_active { WHITE } else { GRAY },
            );

            self.draw_text_sharp(
                "<",
                form_x + 15.0,
                gender_y + 44.0,
                16.0,
                if gender_active { YELLOW } else { DARKGRAY },
            );
            let gender_text = GENDERS[self.gender_index];
            let gender_width = self.measure_text_sharp(gender_text, 16.0).width;
            self.draw_text_sharp(
                gender_text,
                form_x + form_w / 2.0 - gender_width / 2.0,
                gender_y + 44.0,
                16.0,
                WHITE,
            );
            self.draw_text_sharp(
                ">",
                form_x + form_w - 25.0,
                gender_y + 44.0,
                16.0,
                if gender_active { YELLOW } else { DARKGRAY },
            );
        }

        // Skin field
        let skin_active = self.active_field == CreateField::Skin;
        let skin_y = form_y + field_height * 2.0;
        if is_visible(skin_y, field_height) {
            self.draw_text_sharp(
                "Skin",
                form_x,
                skin_y,
                16.0,
                if skin_active { WHITE } else { GRAY },
            );

            let skin_box_color = if skin_active {
                Color::from_rgba(80, 120, 180, 255)
            } else {
                Color::from_rgba(60, 60, 80, 255)
            };
            draw_rectangle(form_x, skin_y + 20.0, form_w, 36.0, skin_box_color);
            draw_rectangle_lines(
                form_x,
                skin_y + 20.0,
                form_w,
                36.0,
                2.0,
                if skin_active { WHITE } else { GRAY },
            );

            self.draw_text_sharp(
                "<",
                form_x + 15.0,
                skin_y + 44.0,
                16.0,
                if skin_active { YELLOW } else { DARKGRAY },
            );
            let skin_text = SKINS[self.skin_index];
            let skin_width = self.measure_text_sharp(skin_text, 16.0).width;
            self.draw_text_sharp(
                skin_text,
                form_x + form_w / 2.0 - skin_width / 2.0,
                skin_y + 44.0,
                16.0,
                WHITE,
            );
            self.draw_text_sharp(
                ">",
                form_x + form_w - 25.0,
                skin_y + 44.0,
                16.0,
                if skin_active { YELLOW } else { DARKGRAY },
            );
        }

        // Hair row: Style and Color side by side
        let hair_y = form_y + field_height * 3.0;
        let half_width = (form_w - 10.0) / 2.0; // 10px gap between
        if is_visible(hair_y, field_height) {
            // Hair Style (left half)
            let hair_style_active = self.active_field == CreateField::HairStyle;
            self.draw_text_sharp(
                "Style",
                form_x,
                hair_y,
                16.0,
                if hair_style_active { WHITE } else { GRAY },
            );

            let hair_style_box_color = if hair_style_active {
                Color::from_rgba(80, 120, 180, 255)
            } else {
                Color::from_rgba(60, 60, 80, 255)
            };
            draw_rectangle(
                form_x,
                hair_y + 20.0,
                half_width,
                36.0,
                hair_style_box_color,
            );
            draw_rectangle_lines(
                form_x,
                hair_y + 20.0,
                half_width,
                36.0,
                2.0,
                if hair_style_active { WHITE } else { GRAY },
            );

            self.draw_text_sharp(
                "<",
                form_x + 10.0,
                hair_y + 44.0,
                16.0,
                if hair_style_active { YELLOW } else { DARKGRAY },
            );
            let hair_style_string;
            let hair_style_text = match self.hair_style_index {
                None => "Bald",
                Some(i) => {
                    hair_style_string = format!("{}", i + 1);
                    &hair_style_string
                }
            };
            let hair_style_width = self.measure_text_sharp(hair_style_text, 16.0).width;
            self.draw_text_sharp(
                hair_style_text,
                form_x + half_width / 2.0 - hair_style_width / 2.0,
                hair_y + 44.0,
                16.0,
                WHITE,
            );
            self.draw_text_sharp(
                ">",
                form_x + half_width - 20.0,
                hair_y + 44.0,
                16.0,
                if hair_style_active { YELLOW } else { DARKGRAY },
            );

            // Hair Color (right half) - only enabled if hair style selected
            let hair_color_x = form_x + half_width + 10.0;
            let hair_color_active = self.active_field == CreateField::HairColor;
            let has_hair = self.hair_style_index.is_some();

            self.draw_text_sharp(
                "Color",
                hair_color_x,
                hair_y,
                16.0,
                if has_hair {
                    if hair_color_active {
                        WHITE
                    } else {
                        GRAY
                    }
                } else {
                    DARKGRAY
                },
            );

            let hair_color_box_color = if !has_hair {
                Color::from_rgba(40, 40, 50, 255) // Disabled look
            } else if hair_color_active {
                Color::from_rgba(80, 120, 180, 255)
            } else {
                Color::from_rgba(60, 60, 80, 255)
            };
            draw_rectangle(
                hair_color_x,
                hair_y + 20.0,
                half_width,
                36.0,
                hair_color_box_color,
            );
            draw_rectangle_lines(
                hair_color_x,
                hair_y + 20.0,
                half_width,
                36.0,
                2.0,
                if has_hair {
                    if hair_color_active {
                        WHITE
                    } else {
                        GRAY
                    }
                } else {
                    DARKGRAY
                },
            );

            if has_hair {
                self.draw_text_sharp(
                    "<",
                    hair_color_x + 10.0,
                    hair_y + 44.0,
                    16.0,
                    if hair_color_active { YELLOW } else { DARKGRAY },
                );
                let hair_color_text = format!("{}", self.hair_color_index + 1);
                let hair_color_width = self.measure_text_sharp(&hair_color_text, 16.0).width;
                self.draw_text_sharp(
                    &hair_color_text,
                    hair_color_x + half_width / 2.0 - hair_color_width / 2.0,
                    hair_y + 44.0,
                    16.0,
                    WHITE,
                );
                self.draw_text_sharp(
                    ">",
                    hair_color_x + half_width - 20.0,
                    hair_y + 44.0,
                    16.0,
                    if hair_color_active { YELLOW } else { DARKGRAY },
                );
            } else {
                let dash_width = self.measure_text_sharp("-", 16.0).width;
                self.draw_text_sharp(
                    "-",
                    hair_color_x + half_width / 2.0 - dash_width / 2.0,
                    hair_y + 44.0,
                    16.0,
                    DARKGRAY,
                );
            }
        }

        // Buttons row
        let buttons_y = form_y + field_height * 4.0 + 10.0;
        let button_w = (form_w - 10.0) / 2.0;
        if is_visible(buttons_y, 36.0) {
            // Create button
            let create_hovered = point_in_rect(mx, my, form_x, buttons_y, button_w, 36.0);
            let create_bg = if create_hovered {
                Color::from_rgba(60, 140, 90, 255)
            } else {
                Color::from_rgba(40, 100, 60, 255)
            };
            let create_border = if create_hovered {
                Color::from_rgba(100, 200, 120, 255)
            } else {
                Color::from_rgba(60, 140, 80, 255)
            };
            draw_rectangle(form_x, buttons_y, button_w, 36.0, create_bg);
            draw_rectangle_lines(form_x, buttons_y, button_w, 36.0, 2.0, create_border);
            let create_text = "Create";
            let create_width = self.measure_text_sharp(create_text, 16.0).width;
            self.draw_text_sharp(
                create_text,
                form_x + button_w / 2.0 - create_width / 2.0,
                buttons_y + 24.0,
                16.0,
                WHITE,
            );

            // Cancel button
            let cancel_x = form_x + button_w + 10.0;
            let cancel_hovered = point_in_rect(mx, my, cancel_x, buttons_y, button_w, 36.0);
            let cancel_bg = if cancel_hovered {
                Color::from_rgba(120, 80, 80, 255)
            } else {
                Color::from_rgba(80, 60, 60, 255)
            };
            let cancel_border = if cancel_hovered {
                Color::from_rgba(180, 120, 120, 255)
            } else {
                Color::from_rgba(120, 80, 80, 255)
            };
            draw_rectangle(cancel_x, buttons_y, button_w, 36.0, cancel_bg);
            draw_rectangle_lines(cancel_x, buttons_y, button_w, 36.0, 2.0, cancel_border);
            let cancel_text = "Cancel";
            let cancel_width = self.measure_text_sharp(cancel_text, 16.0).width;
            self.draw_text_sharp(
                cancel_text,
                cancel_x + button_w / 2.0 - cancel_width / 2.0,
                buttons_y + 24.0,
                16.0,
                WHITE,
            );
        }

        // Error message
        if let Some(ref error) = self.error_message {
            let error_y = buttons_y + 50.0;
            if error_y < clip_bottom {
                let error_width = self.measure_text_sharp(error, 16.0).width;
                self.draw_text_sharp(
                    error,
                    form_x + (form_w - error_width) / 2.0,
                    error_y,
                    16.0,
                    RED,
                );
            }
        }

        // Scroll indicator (only when content is scrollable)
        let field_height = 70.0;
        let form_total_h: f32 = field_height * 4.0 + 10.0 + 36.0;
        let max_scroll = (form_total_h - max_content_height).max(0.0);
        if max_scroll > 0.0 {
            let track_x = form_x + form_w + 8.0;
            let track_y = fixed_y;
            let track_h = max_content_height;
            let track_w = 4.0;

            // Track
            draw_rectangle(
                track_x,
                track_y,
                track_w,
                track_h,
                Color::from_rgba(50, 50, 70, 150),
            );

            // Thumb
            let thumb_ratio = (max_content_height / form_total_h).min(1.0);
            let thumb_h = (track_h * thumb_ratio).max(20.0);
            let scroll_ratio = self.scroll_y / max_scroll;
            let thumb_y = track_y + (track_h - thumb_h) * scroll_ratio;
            draw_rectangle(
                track_x,
                thumb_y,
                track_w,
                thumb_h,
                Color::from_rgba(120, 140, 180, 200),
            );
        }

        // Keyboard hints at bottom (hide on touch devices)
        if !self.touch_detected {
            let hints_y = sh - 30.0;
            self.draw_text_sharp(
                "[Tab] Next field    [A/D] Change    [Enter] Create    [Esc] Cancel",
                (sw - 450.0) / 2.0,
                hints_y,
                16.0,
                DARKGRAY,
            );
        }
    }
}
