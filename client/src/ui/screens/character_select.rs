use super::*;

/// Maximum characters per account
const MAX_CHARACTERS: usize = 3;

/// Format played-time seconds as a compact `1m` / `9h 51m` / `149h 44m` string.
fn format_played_time(seconds: i64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

pub struct CharacterSelectScreen {
    session: AuthSession,
    characters: Vec<CharacterInfo>,
    selected_index: usize,
    error_message: Option<String>,
    auth_client: AuthClient,
    font: BitmapFont,
    confirm_delete: bool,
    player_sprites: SpritesheetStore,
    hair_sprites: SpritesheetStore,
    equipment_sprites: SpritesheetStore,
    // Scroll state for character list on small screens
    list_scroll_offset: f32,
    touch_scroll_id: Option<u64>,
    touch_scroll_last_y: f32,
    /// When true, spectator world is rendered behind this screen — use dark overlay instead of solid bg
    pub has_spectator_backdrop: bool,
    #[cfg(target_arch = "wasm32")]
    loading: bool,
    #[cfg(target_arch = "wasm32")]
    needs_initial_load: bool,
    #[cfg(target_arch = "wasm32")]
    needs_equipment_load: bool,
}

impl CharacterSelectScreen {
    pub fn new(session: AuthSession, server_url: &str) -> Self {
        let auth_client = AuthClient::new(server_url);

        // Use characters from login response (no separate request needed)
        let characters = session.characters.clone();

        Self {
            session,
            characters,
            selected_index: 0,
            error_message: None,
            auth_client,
            font: BitmapFont::default(),
            confirm_delete: false,
            player_sprites: SpritesheetStore::Individual(HashMap::new()),
            hair_sprites: SpritesheetStore::Individual(HashMap::new()),
            equipment_sprites: SpritesheetStore::Individual(HashMap::new()),
            list_scroll_offset: 0.0,
            touch_scroll_id: None,
            touch_scroll_last_y: 0.0,
            has_spectator_backdrop: false,
            #[cfg(target_arch = "wasm32")]
            loading: false,
            #[cfg(target_arch = "wasm32")]
            needs_initial_load: true,
            #[cfg(target_arch = "wasm32")]
            needs_equipment_load: false,
        }
    }

    /// Use pre-loaded assets from the renderer (avoids duplicate loading)
    pub fn use_renderer_assets(
        &mut self,
        font: BitmapFont,
        player: SpritesheetStore,
        hair: SpritesheetStore,
        equipment: SpritesheetStore,
    ) {
        self.font = font;
        self.player_sprites = player;
        self.hair_sprites = hair;
        self.equipment_sprites = equipment;
    }

    /// Load font and sprites asynchronously - call this after creating the screen
    pub async fn load_font(&mut self) {
        // If assets were pre-loaded via use_renderer_assets(), skip loading
        if !self.player_sprites.is_empty() {
            return;
        }

        self.font =
            BitmapFont::load_or_default("assets/fonts/monogram/ttf/monogram-extended.ttf").await;

        // Load sprites from atlas or individually
        let manifest = SpriteManifest::load().await;

        if let Some(ref atlas_info) = manifest.players_atlas {
            if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                self.player_sprites = SpritesheetStore::Atlas {
                    texture: tex,
                    rects,
                };
            }
        }
        if self.player_sprites.is_empty() {
            self.player_sprites = load_player_sprites().await;
        }

        if let Some(ref atlas_info) = manifest.hair_atlas {
            if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                self.hair_sprites = SpritesheetStore::Atlas {
                    texture: tex,
                    rects,
                };
            }
        }
        if self.hair_sprites.is_empty() {
            let mut hair_map = HashMap::new();
            for style in 0..6i32 {
                let path = asset_path(&format!("assets/sprites/hair/hair_{}.png", style));
                if let Ok(tex) = load_texture(&path).await {
                    tex.set_filter(FilterMode::Nearest);
                    hair_map.insert(format!("male_{}", style), tex);
                }
                let path = asset_path(&format!("assets/sprites/hair/hair_female_{}.png", style));
                if let Ok(tex) = load_texture(&path).await {
                    tex.set_filter(FilterMode::Nearest);
                    hair_map.insert(format!("female_{}", style), tex);
                }
            }
            self.hair_sprites = SpritesheetStore::Individual(hair_map);
        }

        self.load_equipment_sprites(&manifest).await;
    }

    async fn load_equipment_sprites(&mut self, manifest: &SpriteManifest) {
        // Try atlas first
        if let Some(ref atlas_info) = manifest.equipment_atlas {
            if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                self.equipment_sprites = SpritesheetStore::Atlas {
                    texture: tex,
                    rects,
                };
                return;
            }
        }

        // Fallback: load individual equipment sprites
        let mut sprite_keys: Vec<String> = Vec::new();
        for c in &self.characters {
            for key in [
                &c.sprite_head,
                &c.sprite_body,
                &c.sprite_weapon,
                &c.sprite_back,
                &c.sprite_feet,
            ]
            .into_iter()
            .flatten()
            {
                if !sprite_keys.contains(key) && !self.equipment_sprites.contains(key) {
                    sprite_keys.push(key.clone());
                }
            }
        }

        if sprite_keys.is_empty() {
            return;
        }

        let mut equip_map = HashMap::new();
        for sprite_key in &sprite_keys {
            for entry in &manifest.equipment {
                let key = entry.rsplit('/').next().unwrap_or(entry);
                if key == sprite_key {
                    let path = asset_path(&format!("assets/sprites/{}.png", entry));
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        equip_map.insert(sprite_key.clone(), tex);
                    }
                    break;
                }
            }
        }
        self.equipment_sprites = SpritesheetStore::Individual(equip_map);
    }

    /// Load equipment sprites if characters have been loaded (WASM only)
    #[cfg(target_arch = "wasm32")]
    pub async fn load_equipment_if_needed(&mut self) {
        // Skip if equipment already loaded from renderer
        if self.equipment_sprites.len() > 0 {
            self.needs_equipment_load = false;
            return;
        }
        if self.needs_equipment_load {
            self.needs_equipment_load = false;
            let manifest = SpriteManifest::load().await;
            self.load_equipment_sprites(&manifest).await;
        }
    }

    /// Set an error message to display
    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
    }

    /// Refresh the character list from the server
    pub fn refresh_characters(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Ok(chars) = self.auth_client.get_characters(&self.session.token) {
                self.characters = chars;
                if self.selected_index >= self.characters.len() && !self.characters.is_empty() {
                    self.selected_index = self.characters.len() - 1;
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        if !self.auth_client.is_busy() {
            self.loading = true;
            self.auth_client.start_get_characters(&self.session.token);
        }
    }

    /// Draw text with pixel font for sharp rendering
    fn draw_text_sharp(&self, text: &str, x: f32, y: f32, font_size: f32, color: Color) {
        self.font.draw_text(text, x, y, font_size, color);
    }

    fn measure_text_sharp(&self, text: &str, font_size: f32) -> TextDimensions {
        self.font.measure_text(text, font_size)
    }
}

impl Screen for CharacterSelectScreen {
    fn update(&mut self, _audio: &AudioManager) -> ScreenState {
        // WASM: poll pending requests (characters now come with login response)
        #[cfg(target_arch = "wasm32")]
        {
            // Skip initial load - characters are included in login response
            if self.needs_initial_load {
                self.needs_initial_load = false;
            }
            if let Some(result) = self.auth_client.poll() {
                self.loading = false;
                match result {
                    AuthResult::Characters(Ok(chars)) => {
                        self.characters = chars;
                        if self.selected_index >= self.characters.len()
                            && !self.characters.is_empty()
                        {
                            self.selected_index = self.characters.len() - 1;
                        }
                        self.needs_equipment_load = true;
                    }
                    AuthResult::Characters(Err(e)) => {
                        self.error_message = Some(e.to_string());
                    }
                    AuthResult::CharacterDeleted(Ok(())) => {
                        // Refresh after delete
                        self.refresh_characters();
                    }
                    AuthResult::CharacterDeleted(Err(e)) => {
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
        let list_w = 500.0_f32.min(sw - 20.0);
        let list_x = (sw - list_w) / 2.0;
        let list_y = 44.0;
        let item_height = 70.0;
        let button_area_height = 48.0;
        let inst_y = sh - button_area_height;

        // Scrollable list area: from list_y down to the button area (with padding)
        let list_button_gap = 4.0;
        let list_visible_height = (inst_y - 10.0 - list_button_gap - list_y).min(400.0);
        let total_list_height = self.characters.len() as f32 * item_height;
        let max_scroll = (total_list_height - list_visible_height).max(0.0);
        self.list_scroll_offset = self.list_scroll_offset.clamp(0.0, max_scroll);

        // Touch drag scrolling
        let all_touches: Vec<Touch> = touches();
        if let Some(tracking_id) = self.touch_scroll_id {
            if let Some(touch) = all_touches.iter().find(|t| t.id == tracking_id) {
                match touch.phase {
                    TouchPhase::Moved | TouchPhase::Stationary => {
                        let (_, vy) = screen_to_virtual(touch.position.x, touch.position.y);
                        let dy = self.touch_scroll_last_y - vy;
                        self.list_scroll_offset =
                            (self.list_scroll_offset + dy).clamp(0.0, max_scroll);
                        self.touch_scroll_last_y = vy;
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        self.touch_scroll_id = None;
                    }
                    _ => {}
                }
            } else {
                self.touch_scroll_id = None;
            }
        } else if !self.confirm_delete {
            for touch in &all_touches {
                if touch.phase == TouchPhase::Started {
                    let (vx, vy) = screen_to_virtual(touch.position.x, touch.position.y);
                    // Only start scroll if touch is in the list area
                    if vx >= list_x
                        && vx <= list_x + list_w
                        && vy >= list_y
                        && vy <= list_y + list_visible_height
                    {
                        self.touch_scroll_id = Some(touch.id);
                        self.touch_scroll_last_y = vy;
                        break;
                    }
                }
            }
        }

        // Mouse wheel scrolling
        let (_wheel_x, wheel_y) = mouse_wheel();
        if wheel_y != 0.0 && my >= list_y && my <= list_y + list_visible_height {
            self.list_scroll_offset =
                (self.list_scroll_offset - wheel_y * 30.0).clamp(0.0, max_scroll);
        }

        // Delete confirmation mode
        if self.confirm_delete {
            // Keyboard shortcuts
            if is_key_pressed(KeyCode::Y) {
                if !self.characters.is_empty() {
                    let char_id = self.characters[self.selected_index].id;
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if self
                            .auth_client
                            .delete_character(&self.session.token, char_id)
                            .is_ok()
                        {
                            self.refresh_characters();
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    if !self.auth_client.is_busy() {
                        self.loading = true;
                        self.auth_client
                            .start_delete_character(&self.session.token, char_id);
                    }
                }
                self.confirm_delete = false;
                return ScreenState::Continue;
            }
            if is_key_pressed(KeyCode::N) || is_key_pressed(KeyCode::Escape) {
                self.confirm_delete = false;
                return ScreenState::Continue;
            }

            // Mouse clicks on Yes/No buttons
            if clicked {
                let box_w = 450.0_f32.min(sw - 20.0);
                let box_h = 150.0;
                let box_x = (sw - box_w) / 2.0;
                let box_y = (sh - box_h) / 2.0;

                // Yes button area (left side of button text)
                let yes_x = box_x + 70.0;
                let yes_y = box_y + 85.0;
                if point_in_rect(mx, my, yes_x, yes_y, 100.0, 30.0) {
                    if !self.characters.is_empty() {
                        let char_id = self.characters[self.selected_index].id;
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            if self
                                .auth_client
                                .delete_character(&self.session.token, char_id)
                                .is_ok()
                            {
                                self.refresh_characters();
                            }
                        }
                        #[cfg(target_arch = "wasm32")]
                        if !self.auth_client.is_busy() {
                            self.loading = true;
                            self.auth_client
                                .start_delete_character(&self.session.token, char_id);
                        }
                    }
                    self.confirm_delete = false;
                    return ScreenState::Continue;
                }

                // No button area (right side of button text)
                let no_x = box_x + 250.0;
                if point_in_rect(mx, my, no_x, yes_y, 100.0, 30.0) {
                    self.confirm_delete = false;
                    return ScreenState::Continue;
                }
            }

            return ScreenState::Continue;
        }

        // Mouse: Click on character rows (accounting for scroll and clipping)
        if clicked && !self.characters.is_empty() {
            for i in 0..self.characters.len() {
                let y = list_y + i as f32 * item_height - self.list_scroll_offset;
                // Only allow clicking visible rows
                if y + item_height - 5.0 < list_y || y > list_y + list_visible_height {
                    continue;
                }
                if point_in_rect(mx, my, list_x, y, list_w, item_height - 5.0) {
                    // Ensure click is within the visible list area
                    if my >= list_y && my <= list_y + list_visible_height {
                        if self.selected_index == i {
                            let character = &self.characters[self.selected_index];
                            return ScreenState::StartGame {
                                session: self.session.clone(),
                                character_id: character.id,
                                character_name: character.name.clone(),
                            };
                        } else {
                            self.selected_index = i;
                        }
                        break;
                    }
                }
            }
        }

        // Mouse: Click on buttons at bottom
        if clicked {
            // Play button
            if point_in_rect(mx, my, list_x, inst_y - 10.0, 100.0, 30.0)
                && !self.characters.is_empty()
            {
                let character = &self.characters[self.selected_index];
                return ScreenState::StartGame {
                    session: self.session.clone(),
                    character_id: character.id,
                    character_name: character.name.clone(),
                };
            }

            // New button
            if self.characters.len() < MAX_CHARACTERS
                && point_in_rect(mx, my, list_x + 120.0, inst_y - 10.0, 70.0, 30.0)
            {
                return ScreenState::ToCharacterCreate(self.session.clone());
            }

            // Delete button
            if point_in_rect(mx, my, list_x + 210.0, inst_y - 10.0, 90.0, 30.0)
                && !self.characters.is_empty()
            {
                self.confirm_delete = true;
            }

            // Logout button
            if point_in_rect(mx, my, list_x + 330.0, inst_y - 10.0, 100.0, 30.0) {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let _ = self.auth_client.logout(&self.session.token);
                }
                return ScreenState::ToLogin;
            }
        }

        // Keyboard: Navigate characters
        if (is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W)) && self.selected_index > 0 {
            self.selected_index -= 1;
        }
        if (is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S))
            && self.selected_index < self.characters.len().saturating_sub(1)
        {
            self.selected_index += 1;
        }

        // Keyboard: Create new character
        if is_key_pressed(KeyCode::N) && self.characters.len() < MAX_CHARACTERS {
            return ScreenState::ToCharacterCreate(self.session.clone());
        }

        // Keyboard: Delete character
        if (is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::X))
            && !self.characters.is_empty()
        {
            self.confirm_delete = true;
        }

        // Keyboard: Logout
        if is_key_pressed(KeyCode::Escape) {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = self.auth_client.logout(&self.session.token);
            }
            return ScreenState::ToLogin;
        }

        // Keyboard: Select character and start game
        if is_key_pressed(KeyCode::Enter) && !self.characters.is_empty() {
            let character = &self.characters[self.selected_index];
            return ScreenState::StartGame {
                session: self.session.clone(),
                character_id: character.id,
                character_name: character.name.clone(),
            };
        }

        ScreenState::Continue
    }

    fn render(&self) {
        let (sw, sh) = virtual_screen_size();
        let (input_pos, _, _) = get_input_state();
        let mx = input_pos.x;
        let my = input_pos.y;

        // Background
        if self.has_spectator_backdrop {
            // Dark overlay over the live spectator world
            draw_rectangle(0.0, 0.0, sw, sh, Color::from_rgba(15, 15, 25, 160));
        } else {
            clear_background(Color::from_rgba(25, 25, 35, 255));

            // Draw decorative elements (only without spectator backdrop)
            for i in 0..15 {
                let alpha = 0.03 + (i as f32 * 0.005);
                let color = Color::new(0.2, 0.3, 0.4, alpha);
                draw_line(0.0, i as f32 * 50.0, sw, i as f32 * 50.0, 1.0, color);
            }
        }

        // Title (aligned vertically with account info, horizontally centered)
        let title = "SELECT CHARACTER";
        let title_width = self.measure_text_sharp(title, 16.0).width;
        self.draw_text_sharp(title, (sw - title_width) / 2.0, 24.0, 16.0, WHITE);

        // Account info
        let account_text = format!("Logged in as: {}", self.session.username);
        self.draw_text_sharp(&account_text, 20.0, 24.0, 16.0, LIGHTGRAY);

        // Layout
        let list_w = 500.0_f32.min(sw - 20.0);
        let list_x = (sw - list_w) / 2.0;
        let list_y = 44.0;
        let item_height = 70.0;
        let button_area_height = 48.0;
        let inst_y = sh - button_area_height;
        let list_visible_height = (inst_y - 10.0 - list_y).min(400.0);
        let total_list_height = self.characters.len() as f32 * item_height;
        let max_scroll = (total_list_height - list_visible_height).max(0.0);
        let scroll_offset = self.list_scroll_offset.clamp(0.0, max_scroll);
        let needs_scroll = max_scroll > 0.0;

        if self.characters.is_empty() {
            self.draw_text_sharp("No characters yet!", list_x, list_y + 30.0, 16.0, GRAY);
            self.draw_text_sharp(
                "Press [N] to create your first character",
                list_x,
                list_y + 55.0,
                16.0,
                LIGHTGRAY,
            );
        } else {
            // Set up scissor clipping for the list area
            if needs_scroll {
                let physical_w = screen_width();
                let physical_h = screen_height();
                let scale_x = physical_w / sw;
                let scale_y = physical_h / sh;
                let mut gl = unsafe { macroquad::window::get_internal_gl() };
                gl.flush();
                gl.quad_gl.scissor(Some((
                    (list_x * scale_x) as i32,
                    (list_y * scale_y) as i32,
                    (list_w * scale_x) as i32,
                    (list_visible_height * scale_y) as i32,
                )));
            }

            for (i, character) in self.characters.iter().enumerate() {
                let y = list_y + i as f32 * item_height - scroll_offset;

                // Skip rows fully outside visible area
                if needs_scroll && (y + item_height < list_y || y > list_y + list_visible_height) {
                    continue;
                }

                let is_selected = i == self.selected_index;
                let is_hovered = point_in_rect(mx, my, list_x, y, list_w, item_height - 5.0)
                    && my >= list_y
                    && my <= list_y + list_visible_height;

                // Background
                let bg_color = if is_selected {
                    Color::from_rgba(60, 80, 120, 255)
                } else if is_hovered {
                    Color::from_rgba(50, 55, 80, 255)
                } else {
                    Color::from_rgba(40, 40, 60, 255)
                };
                draw_rectangle(list_x, y, list_w, item_height - 5.0, bg_color);

                if is_selected {
                    draw_rectangle_lines(list_x, y, list_w, item_height - 5.0, 2.0, WHITE);
                } else if is_hovered {
                    draw_rectangle_lines(list_x, y, list_w, item_height - 5.0, 1.0, GRAY);
                }

                // Character preview sprite (floor to avoid subpixel stretching)
                let preview_x = (list_x + 10.0).floor();
                let preview_y = (y + (item_height - 5.0 - SPRITE_HEIGHT) / 2.0).floor();
                draw_character_preview(
                    &self.player_sprites,
                    &self.hair_sprites,
                    &self.equipment_sprites,
                    &character.gender,
                    &character.skin,
                    character.hair_style,
                    character.hair_color.unwrap_or(0),
                    character.sprite_body.as_deref(),
                    character.sprite_back.as_deref(),
                    character.sprite_feet.as_deref(),
                    preview_x,
                    preview_y,
                );

                // Character info (shifted right to make room for preview)
                let text_x = list_x + 50.0;
                self.draw_text_sharp(&character.name, text_x, y + 26.0, 16.0, WHITE);
                let class_info = format!(
                    "Level {} {} {}",
                    character.level, character.gender, character.skin
                );
                self.draw_text_sharp(&class_info, text_x, y + 48.0, 16.0, LIGHTGRAY);

                // Played time
                let hours = character.played_time / 3600;
                let minutes = (character.played_time % 3600) / 60;
                let time_str = if hours > 0 {
                    format!("{}h {}m played", hours, minutes)
                } else {
                    format!("{}m played", minutes)
                };
                let time_x = (list_x + list_w - 160.0).max(text_x + 120.0);
                self.draw_text_sharp(&time_str, time_x, y + 36.0, 16.0, LIGHTGRAY);
            }

            // Disable scissor clipping
            if needs_scroll {
                let mut gl = unsafe { macroquad::window::get_internal_gl() };
                gl.flush();
                gl.quad_gl.scissor(None);

                // Draw scrollbar
                let scrollbar_w = 4.0;
                let scrollbar_x = list_x + list_w - scrollbar_w - 2.0;
                let track_h = list_visible_height;
                let thumb_ratio = list_visible_height / total_list_height;
                let thumb_h = (track_h * thumb_ratio).max(20.0);
                let scroll_ratio = if max_scroll > 0.0 {
                    scroll_offset / max_scroll
                } else {
                    0.0
                };
                let thumb_y = list_y + (track_h - thumb_h) * scroll_ratio;

                // Track
                draw_rectangle(
                    scrollbar_x,
                    list_y,
                    scrollbar_w,
                    track_h,
                    Color::new(1.0, 1.0, 1.0, 0.08),
                );
                // Thumb
                draw_rectangle(
                    scrollbar_x,
                    thumb_y,
                    scrollbar_w,
                    thumb_h,
                    Color::new(1.0, 1.0, 1.0, 0.3),
                );
            }
        }

        // Background below the list to cleanly separate from buttons
        // (skip when spectator backdrop is active — the full-screen overlay already covers it)
        if !self.has_spectator_backdrop {
            let button_zone_y = list_y + list_visible_height;
            draw_rectangle(
                0.0,
                button_zone_y,
                sw,
                sh - button_zone_y,
                Color::from_rgba(25, 25, 35, 255),
            );
        }

        // Error message
        if let Some(ref error) = self.error_message {
            let error_y = inst_y - 25.0;
            self.draw_text_sharp(error, list_x, error_y, 16.0, RED);
        }

        // Delete confirmation overlay
        if self.confirm_delete {
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.7));

            let box_w = 450.0_f32.min(sw - 20.0);
            let box_h = 150.0;
            let box_x = (sw - box_w) / 2.0;
            let box_y = (sh - box_h) / 2.0;

            draw_rectangle(
                box_x,
                box_y,
                box_w,
                box_h,
                Color::from_rgba(60, 40, 40, 255),
            );
            draw_rectangle_lines(box_x, box_y, box_w, box_h, 2.0, RED);

            if !self.characters.is_empty() {
                let char_name = &self.characters[self.selected_index].name;
                let delete_text = format!("Delete '{}'?", char_name);
                let delete_width = self.measure_text_sharp(&delete_text, 16.0).width;
                self.draw_text_sharp(
                    &delete_text,
                    box_x + (box_w - delete_width) / 2.0,
                    box_y + 50.0,
                    16.0,
                    WHITE,
                );
            }

            // Touch-friendly Yes/No buttons
            let button_w = 100.0;
            let button_h = 30.0;
            let yes_x = box_x + 70.0;
            let yes_y = box_y + 85.0;
            let no_x = box_x + 250.0;

            // Yes button
            let yes_hovered = point_in_rect(mx, my, yes_x, yes_y, button_w, button_h);
            let yes_bg = if yes_hovered {
                Color::from_rgba(140, 60, 60, 255)
            } else {
                Color::from_rgba(100, 40, 40, 255)
            };
            let yes_border = if yes_hovered {
                Color::from_rgba(255, 100, 100, 255)
            } else {
                RED
            };
            draw_rectangle(yes_x, yes_y, button_w, button_h, yes_bg);
            draw_rectangle_lines(yes_x, yes_y, button_w, button_h, 2.0, yes_border);
            self.draw_text_sharp("Yes, delete", yes_x + 8.0, yes_y + 20.0, 16.0, WHITE);

            // No button
            let no_hovered = point_in_rect(mx, my, no_x, yes_y, button_w, button_h);
            let no_bg = if no_hovered {
                Color::from_rgba(80, 80, 110, 255)
            } else {
                Color::from_rgba(60, 60, 80, 255)
            };
            let no_border = if no_hovered { WHITE } else { LIGHTGRAY };
            draw_rectangle(no_x, yes_y, button_w, button_h, no_bg);
            draw_rectangle_lines(no_x, yes_y, button_w, button_h, 2.0, no_border);
            self.draw_text_sharp("No, cancel", no_x + 8.0, yes_y + 20.0, 16.0, WHITE);
            return;
        }

        // Buttons at bottom
        let button_height = 30.0;

        // Play button
        let play_hovered = point_in_rect(mx, my, list_x, inst_y - 10.0, 100.0, button_height);
        let play_bg = if play_hovered {
            Color::from_rgba(60, 140, 90, 255)
        } else {
            Color::from_rgba(40, 100, 60, 255)
        };
        let play_border = if play_hovered {
            Color::from_rgba(100, 255, 150, 255)
        } else {
            GREEN
        };
        draw_rectangle(list_x, inst_y - 10.0, 100.0, button_height, play_bg);
        draw_rectangle_lines(
            list_x,
            inst_y - 10.0,
            100.0,
            button_height,
            2.0,
            play_border,
        );
        self.draw_text_sharp("Play", list_x + 10.0, inst_y + 10.0, 16.0, WHITE);

        // New button
        if self.characters.len() < MAX_CHARACTERS {
            let new_hovered =
                point_in_rect(mx, my, list_x + 120.0, inst_y - 10.0, 70.0, button_height);
            let new_bg = if new_hovered {
                Color::from_rgba(120, 120, 60, 255)
            } else {
                Color::from_rgba(80, 80, 40, 255)
            };
            let new_border = if new_hovered {
                Color::from_rgba(255, 255, 100, 255)
            } else {
                YELLOW
            };
            draw_rectangle(list_x + 120.0, inst_y - 10.0, 70.0, button_height, new_bg);
            draw_rectangle_lines(
                list_x + 120.0,
                inst_y - 10.0,
                70.0,
                button_height,
                2.0,
                new_border,
            );
            self.draw_text_sharp("New", list_x + 130.0, inst_y + 10.0, 16.0, WHITE);
        }

        // Delete button
        let delete_hovered =
            point_in_rect(mx, my, list_x + 210.0, inst_y - 10.0, 90.0, button_height);
        let delete_bg = if delete_hovered {
            Color::from_rgba(140, 60, 60, 255)
        } else {
            Color::from_rgba(100, 40, 40, 255)
        };
        let delete_border = if delete_hovered {
            Color::from_rgba(255, 100, 100, 255)
        } else {
            RED
        };
        draw_rectangle(
            list_x + 210.0,
            inst_y - 10.0,
            90.0,
            button_height,
            delete_bg,
        );
        draw_rectangle_lines(
            list_x + 210.0,
            inst_y - 10.0,
            90.0,
            button_height,
            2.0,
            delete_border,
        );
        self.draw_text_sharp("Delete", list_x + 220.0, inst_y + 10.0, 16.0, WHITE);

        // Logout button
        let logout_hovered =
            point_in_rect(mx, my, list_x + 330.0, inst_y - 10.0, 100.0, button_height);
        let logout_bg = if logout_hovered {
            Color::from_rgba(80, 80, 110, 255)
        } else {
            Color::from_rgba(60, 60, 80, 255)
        };
        let logout_border = if logout_hovered { WHITE } else { LIGHTGRAY };
        draw_rectangle(
            list_x + 330.0,
            inst_y - 10.0,
            100.0,
            button_height,
            logout_bg,
        );
        draw_rectangle_lines(
            list_x + 330.0,
            inst_y - 10.0,
            100.0,
            button_height,
            2.0,
            logout_border,
        );
        self.draw_text_sharp("Logout", list_x + 340.0, inst_y + 10.0, 16.0, WHITE);

        #[cfg(not(target_os = "android"))]
        self.draw_text_sharp("[W/S] Navigate", list_x, inst_y + 28.0, 16.0, DARKGRAY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn played_time_formats() {
        assert_eq!(format_played_time(0), "0m");
        assert_eq!(format_played_time(59), "0m");
        assert_eq!(format_played_time(60), "1m");
        assert_eq!(format_played_time(9 * 3600 + 51 * 60), "9h 51m");
        assert_eq!(format_played_time(149 * 3600 + 44 * 60), "149h 44m");
        assert_eq!(format_played_time(3600), "1h 0m");
    }
}

// ============================================================================
// Character Create Screen
// ============================================================================
