use macroquad::miniquad::window::show_keyboard;
use macroquad::prelude::*;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;

use crate::audio::AudioManager;
#[cfg(target_arch = "wasm32")]
use crate::auth::AuthResult;
use crate::auth::{AuthClient, AuthSession, CharacterInfo};
use crate::render::{BitmapFont, SpritesheetStore};
use crate::util::{asset_path, virtual_screen_size, SpriteManifest};

// Sprite sheet constants for character preview
const SPRITE_WIDTH: f32 = 34.0;
const SPRITE_HEIGHT: f32 = 78.0;

// Equipment sprite constants (matching renderer)
const BODY_ARMOR_SPRITE_WIDTH: f32 = 34.0;
const BODY_ARMOR_SPRITE_HEIGHT: f32 = 77.0;
const BOOT_SPRITE_WIDTH: f32 = 34.0;
const BOOT_SPRITE_HEIGHT: f32 = 27.0;
const BACK_STATIC_SPRITE_WIDTH: f32 = 50.0;
const BACK_STATIC_SPRITE_HEIGHT: f32 = 63.0;
const OFFHAND_SPRITE_WIDTH: f32 = 38.5;
const OFFHAND_SPRITE_HEIGHT: f32 = 38.0;

/// Convert screen coordinates to virtual coordinates (for Android scaling)
fn screen_to_virtual(x: f32, y: f32) -> (f32, f32) {
    let (vw, vh) = virtual_screen_size();
    let screen_w = screen_width();
    let screen_h = screen_height();

    // On desktop, virtual == screen, so this is a no-op
    let vx = x * vw / screen_w;
    let vy = y * vh / screen_h;
    (vx, vy)
}

/// Get input position and click state from either mouse or touch
/// Returns (position, just_clicked, is_touching)
fn get_input_state() -> (Vec2, bool, bool) {
    let touches: Vec<Touch> = touches();

    // Check for touch input first (mobile)
    for touch in &touches {
        if touch.phase == TouchPhase::Started {
            let (vx, vy) = screen_to_virtual(touch.position.x, touch.position.y);
            return (vec2(vx, vy), true, true);
        }
    }

    // Check for any active touch (for position tracking)
    if let Some(touch) = touches.first() {
        let (vx, vy) = screen_to_virtual(touch.position.x, touch.position.y);
        return (vec2(vx, vy), false, true);
    }

    // Fall back to mouse input (desktop)
    let (mx, my) = mouse_position();
    let (vx, vy) = screen_to_virtual(mx, my);
    let clicked = is_mouse_button_pressed(MouseButton::Left);
    (vec2(vx, vy), clicked, false)
}

/// Load all player sprite textures (gender x skin combinations) as individual files
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
async fn load_player_sprites() -> SpritesheetStore {
    let mut sprites = HashMap::new();
    let genders = ["male", "female"];

    for gender in &genders {
        for skin in &SKINS {
            let path = asset_path(&format!(
                "assets/sprites/players/player_{}_{}.png",
                gender, skin
            ));
            if let Ok(texture) = load_texture(&path).await {
                texture.set_filter(FilterMode::Nearest);
                let key = format!("{}_{}", gender, skin);
                sprites.insert(key, texture);
            }
        }
    }

    SpritesheetStore::Individual(sprites)
}

/// Load a spritesheet atlas texture and return the texture + rect mappings
async fn load_spritesheet_atlas(
    atlas_info: &crate::util::SpriteAtlasInfo,
) -> Option<(Texture2D, HashMap<String, Rect>)> {
    let path = asset_path(&format!("assets/{}", atlas_info.file));
    match load_texture(&path).await {
        Ok(tex) => {
            tex.set_filter(FilterMode::Nearest);
            let rects = atlas_info
                .sprites
                .iter()
                .map(|(key, sr)| {
                    (
                        key.clone(),
                        Rect::new(sr.x as f32, sr.y as f32, sr.w as f32, sr.h as f32),
                    )
                })
                .collect();
            log::info!(
                "Loaded spritesheet atlas {} ({}x{}, {} sprites)",
                atlas_info.file,
                tex.width(),
                tex.height(),
                atlas_info.sprites.len()
            );
            Some((tex, rects))
        }
        Err(e) => {
            log::warn!("Failed to load spritesheet atlas {}: {}", path, e);
            None
        }
    }
}

/// Check if a point is inside a rectangle
fn point_in_rect(px: f32, py: f32, rx: f32, ry: f32, rw: f32, rh: f32) -> bool {
    px >= rx && px < rx + rw && py >= ry && py < ry + rh
}

// Hair sprite dimensions (different from player sprites)
const HAIR_SPRITE_WIDTH: f32 = 28.0;
const HAIR_SPRITE_HEIGHT: f32 = 54.0;

/// Draw a character preview sprite at the given position.
/// Renders at native pixel size (no scaling) for crisp pixel art.
fn draw_character_preview(
    sprites: &SpritesheetStore,
    hair_sprites: &SpritesheetStore,
    equipment_sprites: &SpritesheetStore,
    gender: &str,
    skin: &str,
    hair_style: Option<i32>,
    hair_color: i32,
    sprite_body: Option<&str>,
    sprite_back: Option<&str>,
    sprite_feet: Option<&str>,
    x: f32,
    y: f32,
) {
    let key = format!("{}_{}", gender, skin);
    if let Some((texture, player_offset)) = sprites.get(&key) {
        let (player_atlas_x, player_atlas_y) = player_offset.unwrap_or((0.0, 0.0));

        // 1. Draw back items behind player (quiver/cape - frame 1 for "down" direction)
        if let Some(back_id) = sprite_back {
            if let Some((equip_sprite, equip_offset)) = equipment_sprites.get(back_id) {
                let (equip_atlas_x, equip_atlas_y) = equip_offset.unwrap_or((0.0, 0.0));
                let (equip_w, _equip_h) = equipment_sprites
                    .get_dimensions(back_id)
                    .unwrap_or((equip_sprite.width(), equip_sprite.height()));
                let is_offhand = equip_w > 8.0 * BACK_STATIC_SPRITE_WIDTH;
                if !is_offhand {
                    let back_src_x = equip_atlas_x + 1.0 * BACK_STATIC_SPRITE_WIDTH;
                    draw_texture_ex(
                        equip_sprite,
                        x,
                        y - 15.0,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                back_src_x,
                                equip_atlas_y,
                                BACK_STATIC_SPRITE_WIDTH,
                                BACK_STATIC_SPRITE_HEIGHT,
                            )),
                            flip_x: true,
                            ..Default::default()
                        },
                    );
                }
            }
        }

        // 2. Draw base character sprite (idle frame 0,0)
        draw_texture_ex(
            texture,
            x,
            y,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(
                    player_atlas_x,
                    player_atlas_y,
                    SPRITE_WIDTH,
                    SPRITE_HEIGHT,
                )),
                ..Default::default()
            },
        );

        // 3. Draw body armor (frame 0 for idle/down, offset y=-3)
        if let Some(body_id) = sprite_body {
            if let Some((equip_sprite, equip_offset)) = equipment_sprites.get(body_id) {
                let (equip_atlas_x, equip_atlas_y) = equip_offset.unwrap_or((0.0, 0.0));
                let (equip_w, equip_h) = equipment_sprites
                    .get_dimensions(body_id)
                    .unwrap_or((equip_sprite.width(), equip_sprite.height()));
                let is_single_row = equip_w > equip_h * 2.0;
                if is_single_row {
                    draw_texture_ex(
                        equip_sprite,
                        x,
                        y - 3.0,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                equip_atlas_x,
                                equip_atlas_y,
                                BODY_ARMOR_SPRITE_WIDTH,
                                BODY_ARMOR_SPRITE_HEIGHT,
                            )),
                            ..Default::default()
                        },
                    );
                } else {
                    // Old grid-style format - same layout as player sprite
                    draw_texture_ex(
                        equip_sprite,
                        x,
                        y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                equip_atlas_x,
                                equip_atlas_y,
                                SPRITE_WIDTH,
                                SPRITE_HEIGHT,
                            )),
                            ..Default::default()
                        },
                    );
                }
            }
        }

        // 4. Draw hair (after body armor so it appears on top)
        if let Some(style) = hair_style {
            let hair_key = format!("{}_{}", gender, style);
            if let Some((hair_tex, hair_offset)) = hair_sprites.get(&hair_key) {
                let (hair_atlas_x, hair_atlas_y) = hair_offset.unwrap_or((0.0, 0.0));
                let hair_frame_index = hair_color * 2; // front frame
                let hair_src_x = hair_atlas_x + hair_frame_index as f32 * HAIR_SPRITE_WIDTH;
                // Center hair on player: (34 - 28) / 2 = 3, then offset -1
                let hair_x = x + (SPRITE_WIDTH - HAIR_SPRITE_WIDTH) / 2.0 - 1.0;
                let hair_y = y - 3.0;

                draw_texture_ex(
                    hair_tex,
                    hair_x,
                    hair_y,
                    WHITE,
                    DrawTextureParams {
                        source: Some(Rect::new(
                            hair_src_x,
                            hair_atlas_y,
                            HAIR_SPRITE_WIDTH,
                            HAIR_SPRITE_HEIGHT,
                        )),
                        ..Default::default()
                    },
                );
            }
        }

        // 5. Draw boots (frame 0 for idle/down)
        if let Some(feet_id) = sprite_feet {
            if let Some((equip_sprite, equip_offset)) = equipment_sprites.get(feet_id) {
                let (equip_atlas_x, equip_atlas_y) = equip_offset.unwrap_or((0.0, 0.0));
                let (equip_w, equip_h) = equipment_sprites
                    .get_dimensions(feet_id)
                    .unwrap_or((equip_sprite.width(), equip_sprite.height()));
                let is_single_row = equip_w > equip_h;
                if is_single_row {
                    draw_texture_ex(
                        equip_sprite,
                        x,
                        y + 46.0,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                equip_atlas_x,
                                equip_atlas_y,
                                BOOT_SPRITE_WIDTH,
                                BOOT_SPRITE_HEIGHT,
                            )),
                            ..Default::default()
                        },
                    );
                } else {
                    draw_texture_ex(
                        equip_sprite,
                        x,
                        y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                equip_atlas_x,
                                equip_atlas_y,
                                SPRITE_WIDTH,
                                SPRITE_HEIGHT,
                            )),
                            ..Default::default()
                        },
                    );
                }
            }
        }

        // 6. Draw back items in front of player (offhand/shield - frame 0 for idle/down)
        if let Some(back_id) = sprite_back {
            if let Some((equip_sprite, equip_offset)) = equipment_sprites.get(back_id) {
                let (equip_atlas_x, equip_atlas_y) = equip_offset.unwrap_or((0.0, 0.0));
                let (equip_w, _equip_h) = equipment_sprites
                    .get_dimensions(back_id)
                    .unwrap_or((equip_sprite.width(), equip_sprite.height()));
                let is_offhand = equip_w > 8.0 * BACK_STATIC_SPRITE_WIDTH;
                if is_offhand {
                    draw_texture_ex(
                        equip_sprite,
                        x - 2.0,
                        y + 20.0,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                equip_atlas_x,
                                equip_atlas_y,
                                OFFHAND_SPRITE_WIDTH,
                                OFFHAND_SPRITE_HEIGHT,
                            )),
                            ..Default::default()
                        },
                    );
                }
            }
        }
    } else {
        draw_rectangle(
            x,
            y,
            SPRITE_WIDTH,
            SPRITE_HEIGHT,
            Color::from_rgba(100, 100, 100, 255),
        );
    }
}

/// Result of screen update - tells main loop what to do next
pub enum ScreenState {
    /// Stay on current screen
    Continue,
    /// Move to character select with auth session
    ToCharacterSelect(AuthSession),
    /// Move to character creation screen
    ToCharacterCreate(AuthSession),
    /// Start the game with the selected character
    StartGame {
        session: AuthSession,
        character_id: i64,
        character_name: String,
    },
    /// Guest mode (dev only)
    StartGuestMode,
    /// Go back to login
    ToLogin,
}

pub trait Screen {
    fn update(&mut self, audio: &AudioManager) -> ScreenState;
    fn render(&self);
}

// ============================================================================
// Login Screen
// ============================================================================

/// A shooting star streak across the night sky
struct ShootingStar {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max_life: f32,
    length: f32, // trail length in pixels
}

pub struct LoginScreen {
    username: String,
    password: String,
    active_field: LoginField,
    mode: LoginMode,
    error_message: Option<String>,
    auth_client: AuthClient,
    font: BitmapFont,
    logo: Option<Texture2D>,
    // Animation state
    frame_counter: f32,
    stars: Vec<(f32, f32, f32)>, // (x, y, phase)
    shooting_stars: Vec<ShootingStar>,
    // Backspace key-repeat
    backspace_held_time: f32,
    backspace_repeat_timer: f32,
    // Server status
    server_url: String,
    server_online: bool,
    last_ping_time: f32,
    #[cfg(not(target_arch = "wasm32"))]
    health_rx: Option<mpsc::Receiver<bool>>,
    #[cfg(target_arch = "wasm32")]
    loading: bool,
    // Remember me
    remember_me: bool,
    // Auto-focus: show keyboard on first frame
    keyboard_shown: bool,
    // Alpha for the starry background (1.0 = full stars, 0.0 = no stars, world shows through)
    stars_alpha: f32,
}

#[derive(PartialEq, Clone, Copy)]
enum LoginField {
    Username,
    Password,
}

#[derive(PartialEq, Clone, Copy)]
enum LoginMode {
    Login,
    Register,
}

impl LoginScreen {
    pub fn new(server_url: &str) -> Self {
        // Generate stars with deterministic pseudo-random positions
        let mut stars = Vec::with_capacity(60);
        for i in 0..60 {
            let fi = i as f32;
            let x = ((fi * 137.5) % 1000.0) / 1000.0; // fraction of screen width
            let y = ((fi * 97.3 + 23.0) % 1000.0) / 1000.0; // full screen
            let phase = ((fi * 53.7) % 1000.0) / 1000.0 * std::f32::consts::TAU;
            stars.push((x, y, phase));
        }

        let (saved_username, remember_me) = match crate::auth::credentials::load_username() {
            Some(u) => (u, true),
            None => (String::new(), false),
        };

        Self {
            username: saved_username,
            password: String::new(),
            active_field: if remember_me {
                LoginField::Password
            } else {
                LoginField::Username
            },
            mode: LoginMode::Login,
            error_message: None,
            auth_client: AuthClient::new(server_url),
            font: BitmapFont::default(),
            logo: None,
            frame_counter: 0.0,
            stars,
            shooting_stars: Vec::with_capacity(4),
            backspace_held_time: 0.0,
            backspace_repeat_timer: 0.0,
            server_url: server_url.to_string(),
            server_online: false,
            last_ping_time: -10.0, // trigger immediate ping
            #[cfg(not(target_arch = "wasm32"))]
            health_rx: None,
            #[cfg(target_arch = "wasm32")]
            loading: false,
            remember_me,
            keyboard_shown: false,
            stars_alpha: 1.0,
        }
    }

    /// Use pre-loaded font from the renderer (avoids duplicate loading)
    pub fn use_renderer_font(&mut self, font: BitmapFont) {
        self.font = font;
    }

    /// Set the alpha for the starry background (1.0 = full stars, 0.0 = hidden for world backdrop)
    pub fn set_stars_alpha(&mut self, alpha: f32) {
        self.stars_alpha = alpha;
    }

    /// Load font and logo asynchronously - call this after creating the screen
    pub async fn load_font(&mut self) {
        // Font may already be set via use_renderer_font()
        let font_already_loaded = !self.font.is_empty();

        if !font_already_loaded {
            self.font =
                BitmapFont::load_or_default("assets/fonts/monogram/ttf/monogram-extended.ttf")
                    .await;
        }

        // Load logo texture
        if let Ok(texture) = load_texture(&asset_path("assets/ui/logo.png")).await {
            texture.set_filter(FilterMode::Nearest);
            self.logo = Some(texture);
        }
    }

    /// Draw text with pixel font for sharp rendering
    fn draw_text_sharp(&self, text: &str, x: f32, y: f32, font_size: f32, color: Color) {
        self.font.draw_text(text, x, y, font_size, color);
    }

    fn measure_text_sharp(&self, text: &str, font_size: f32) -> TextDimensions {
        self.font.measure_text(text, font_size)
    }

    fn save_remember_me(&self) {
        if self.remember_me {
            crate::auth::credentials::save_username(&self.username);
        } else {
            crate::auth::credentials::clear_username();
        }
    }

    fn handle_text_input(&mut self) {
        // Handle character input
        while let Some(c) = get_char_pressed() {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                let field = match self.active_field {
                    LoginField::Username => &mut self.username,
                    LoginField::Password => &mut self.password,
                };
                if field.len() < 20 {
                    field.push(c);
                }
            }
        }

        // Handle backspace with key-repeat
        if is_key_down(KeyCode::Backspace) {
            let dt = get_frame_time();
            let mut delete = false;

            if is_key_pressed(KeyCode::Backspace) {
                // First press: delete immediately and reset timers
                delete = true;
                self.backspace_held_time = 0.0;
                self.backspace_repeat_timer = 0.0;
            } else {
                self.backspace_held_time += dt;
                // After 0.4s initial delay, repeat every 0.05s
                if self.backspace_held_time > 0.4 {
                    self.backspace_repeat_timer += dt;
                    if self.backspace_repeat_timer >= 0.05 {
                        self.backspace_repeat_timer -= 0.05;
                        delete = true;
                    }
                }
            }

            if delete {
                let field = match self.active_field {
                    LoginField::Username => &mut self.username,
                    LoginField::Password => &mut self.password,
                };
                field.pop();
            }
        } else {
            self.backspace_held_time = 0.0;
            self.backspace_repeat_timer = 0.0;
        }
    }
}

impl Screen for LoginScreen {
    fn update(&mut self, audio: &AudioManager) -> ScreenState {
        let (sw, sh) = virtual_screen_size();
        let (input_pos, clicked, _is_touch) = get_input_state();
        let mx = input_pos.x;
        let my = input_pos.y;

        // Auto-focus: activate keyboard input on first frame so typing works immediately
        if !self.keyboard_shown {
            self.keyboard_shown = true;
            show_keyboard(true);
        }

        // Update animation
        let dt = get_frame_time();
        self.frame_counter += dt;

        // Update shooting stars
        self.shooting_stars.retain_mut(|s| {
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            s.life -= dt / s.max_life;
            s.life > 0.0
        });

        // Spawn shooting stars occasionally
        if self.shooting_stars.len() < 2 {
            let pseudo = (self.frame_counter * 173.0) as u32;
            // ~1 every 3-5 seconds on average
            if pseudo % 200 == 0 {
                let start_x = ((pseudo as f32 * 0.371) % 0.6 + 0.1) * sw;
                let start_y = ((pseudo as f32 * 0.529) % 0.2 + 0.02) * sh;
                let angle = 0.4 + ((pseudo as f32 * 0.213) % 0.4); // downward-right angle
                let speed = 200.0 + ((pseudo as f32 * 0.617) % 150.0);
                let life = 0.6 + ((pseudo as f32 * 0.823) % 0.6);
                self.shooting_stars.push(ShootingStar {
                    x: start_x,
                    y: start_y,
                    vx: angle.cos() * speed,
                    vy: angle.sin() * speed,
                    life: 1.0,
                    max_life: life,
                    length: 20.0 + ((pseudo as f32 * 0.419) % 20.0),
                });
            }
        }

        // Ping server every 5 seconds (non-blocking)
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Check for result from previous health check
            if let Some(rx) = &self.health_rx {
                if let Ok(online) = rx.try_recv() {
                    self.server_online = online;
                    self.health_rx = None;
                }
            }

            // Start a new health check if none is in flight
            if self.health_rx.is_none() && self.frame_counter - self.last_ping_time > 5.0 {
                self.last_ping_time = self.frame_counter;
                let (tx, rx) = mpsc::channel();
                let health_url = format!("{}/health", self.server_url);
                std::thread::spawn(move || {
                    let online = ureq::get(&health_url)
                        .timeout(std::time::Duration::from_secs(2))
                        .call()
                        .is_ok();
                    let _ = tx.send(online);
                });
                self.health_rx = Some(rx);
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            // Poll pending auth requests
            if let Some(result) = self.auth_client.poll() {
                self.loading = false;
                match result {
                    AuthResult::Login(Ok(session)) | AuthResult::Register(Ok(session)) => {
                        self.save_remember_me();
                        return ScreenState::ToCharacterSelect(session);
                    }
                    AuthResult::Login(Err(e)) | AuthResult::Register(Err(e)) => {
                        self.error_message = Some(e.to_string());
                    }
                    AuthResult::HealthCheck(online) => {
                        self.server_online = online;
                    }
                    _ => {}
                }
            }
            // Periodic health check
            if self.frame_counter - self.last_ping_time > 5.0 && !self.auth_client.is_busy() {
                self.last_ping_time = self.frame_counter;
                self.auth_client.start_health_check();
            }
        }

        // Layout constants (must match render)
        let compact = sh < 400.0;
        let box_width = sw.min(340.0);
        let box_height = if compact { 32.0 } else { 40.0 };
        let box_x = (sw - box_width) / 2.0;
        let btn_height = 36.0;
        let spacing = if compact { 4.0 } else { 10.0 };

        // Calculate form height (subtitle + inputs + buttons, excluding logo)
        let logo_h = 46.0;
        let logo_margin = 4.0;
        let subtitle_h = 16.0;
        let form_gap = if compact { 2.0 } else { 8.0 };
        let label_h = 12.0;
        let field_gap = if compact { 14.0 } else { spacing + 4.0 };
        let buttons_gap = spacing + 14.0;

        let checkbox_row_h = 20.0;
        let checkbox_gap = if compact { 4.0 } else { 8.0 };

        let form_content_h = subtitle_h + form_gap
            + label_h + box_height  // username
            + field_gap + label_h + box_height  // password
            + checkbox_gap + checkbox_row_h  // remember me
            + spacing + btn_height; // buttons

        // Center the form content vertically, then place logo above it
        let form_content_top = ((sh - form_content_h) / 2.0).max(logo_h + logo_margin + 6.0);

        let username_y = form_content_top + subtitle_h + form_gap;
        let username_field_y = username_y + label_h;
        let password_y = username_field_y + box_height + field_gap;
        let password_field_y = password_y + label_h;
        let remember_y = password_field_y + box_height + checkbox_gap;
        let buttons_y = remember_y + checkbox_row_h + spacing;

        // Handle touch/click on input fields and buttons
        if clicked {
            // Username field (clickable box area)
            if point_in_rect(mx, my, box_x, username_field_y, box_width, box_height) {
                self.active_field = LoginField::Username;
                show_keyboard(true);
            }
            // Password field (clickable box area)
            else if point_in_rect(mx, my, box_x, password_field_y, box_width, box_height) {
                self.active_field = LoginField::Password;
                show_keyboard(true);
            }
            // Remember me checkbox
            else if point_in_rect(mx, my, box_x, remember_y, box_width, checkbox_row_h) {
                self.remember_me = !self.remember_me;
            }
            // Tapped elsewhere - hide keyboard
            else {
                show_keyboard(false);
            }

            // Login/Register button (left side)
            let login_btn_w = (box_width - spacing) / 2.0;
            if point_in_rect(mx, my, box_x, buttons_y, login_btn_w, btn_height) {
                show_keyboard(false);
                if self.username.len() >= 3 && self.password.len() >= 6 {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let result = match self.mode {
                            LoginMode::Login => {
                                self.auth_client.login(&self.username, &self.password)
                            }
                            LoginMode::Register => {
                                self.auth_client.register(&self.username, &self.password)
                            }
                        };

                        match result {
                            Ok(session) => {
                                self.save_remember_me();
                                return ScreenState::ToCharacterSelect(session);
                            }
                            Err(e) => {
                                self.error_message = Some(e.to_string());
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    if !self.auth_client.is_busy() {
                        self.loading = true;
                        match self.mode {
                            LoginMode::Login => {
                                self.auth_client.start_login(&self.username, &self.password)
                            }
                            LoginMode::Register => self
                                .auth_client
                                .start_register(&self.username, &self.password),
                        }
                    }
                } else if self.username.len() < 3 {
                    self.error_message = Some("Username min 3 chars".to_string());
                } else {
                    self.error_message = Some("Password min 6 chars".to_string());
                }
            }

            // Toggle mode button (right side, same row)
            let toggle_x = box_x + login_btn_w + spacing;
            if point_in_rect(mx, my, toggle_x, buttons_y, login_btn_w, btn_height) {
                self.mode = if self.mode == LoginMode::Register {
                    LoginMode::Login
                } else {
                    LoginMode::Register
                };
                self.error_message = None;
            }
        }

        // Tab to switch fields (check before handle_text_input consumes it)
        if is_key_pressed(KeyCode::Tab) {
            audio.play_sfx("enter");
            self.active_field = match self.active_field {
                LoginField::Username => LoginField::Password,
                LoginField::Password => LoginField::Username,
            };
        }

        self.handle_text_input();

        // Clear error on any input
        if is_key_pressed(KeyCode::Backspace) || get_char_pressed().is_some() {
            self.error_message = None;
        }

        // Toggle between login/register
        if is_key_pressed(KeyCode::F1) {
            self.mode = if self.mode == LoginMode::Register {
                LoginMode::Login
            } else {
                LoginMode::Register
            };
            self.error_message = None;
        }

        // Submit on Enter
        if is_key_pressed(KeyCode::Enter) {
            if self.username.len() < 3 {
                self.error_message = Some("Username must be at least 3 characters".to_string());
                return ScreenState::Continue;
            }
            if self.password.len() < 6 {
                self.error_message = Some("Password must be at least 6 characters".to_string());
                return ScreenState::Continue;
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                let result = match self.mode {
                    LoginMode::Login => self.auth_client.login(&self.username, &self.password),
                    LoginMode::Register => {
                        self.auth_client.register(&self.username, &self.password)
                    }
                };

                match result {
                    Ok(session) => {
                        self.save_remember_me();
                        return ScreenState::ToCharacterSelect(session);
                    }
                    Err(e) => {
                        self.error_message = Some(e.to_string());
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            if !self.auth_client.is_busy() {
                self.loading = true;
                match self.mode {
                    LoginMode::Login => {
                        self.auth_client.start_login(&self.username, &self.password)
                    }
                    LoginMode::Register => self
                        .auth_client
                        .start_register(&self.username, &self.password),
                }
            }
        }

        ScreenState::Continue
    }

    fn render(&self) {
        let (sw, sh) = virtual_screen_size();
        let (input_pos, _, _) = get_input_state();
        let mx = input_pos.x;
        let my = input_pos.y;
        let t = self.frame_counter;

        // === ANIMATED BACKGROUND SCENE ===

        let sa = self.stars_alpha;
        if sa > 0.001 {
            // Night sky gradient (full screen)
            let sky_steps = 20;
            for i in 0..sky_steps {
                let frac = i as f32 / sky_steps as f32;
                let r = (10.0 + frac * 15.0) as u8;
                let g = (12.0 + frac * 8.0) as u8;
                let b = (40.0 - frac * 10.0) as u8;
                let y = frac * sh;
                let h = sh / sky_steps as f32 + 1.0;
                draw_rectangle(
                    0.0,
                    y,
                    sw,
                    h,
                    Color::from_rgba(r, g, b, (255.0 * sa) as u8),
                );
            }

            // Twinkling stars
            for &(sx, sy, phase) in &self.stars {
                let alpha = (((t * 1.5 + phase).sin() * 0.5 + 0.5) * 0.9 + 0.1) * sa;
                let size = if alpha > 0.7 * sa { 2.0 } else { 1.0 };
                draw_rectangle(
                    sx * sw,
                    sy * sh,
                    size,
                    size,
                    Color::new(1.0, 1.0, 0.95, alpha),
                );
            }

            // Shooting stars
            for s in &self.shooting_stars {
                let alpha = s.life.min(1.0) * sa;
                let speed = (s.vx * s.vx + s.vy * s.vy).sqrt();
                let dx = -s.vx / speed * s.length;
                let dy = -s.vy / speed * s.length;
                // Bright head
                draw_line(
                    s.x,
                    s.y,
                    s.x + dx * 0.3,
                    s.y + dy * 0.3,
                    2.0,
                    Color::new(1.0, 1.0, 1.0, alpha),
                );
                // Fading tail
                draw_line(
                    s.x + dx * 0.3,
                    s.y + dy * 0.3,
                    s.x + dx,
                    s.y + dy,
                    1.0,
                    Color::new(0.8, 0.85, 1.0, alpha * 0.4),
                );
            }
        }

        // === FORM OVERLAY ===

        // Layout constants (must match update) - all floored to avoid subpixel rendering
        let compact = sh < 400.0;
        let box_width = sw.min(340.0).floor();
        let box_height = if compact { 32.0 } else { 40.0 };
        let box_x = ((sw - box_width) / 2.0).floor();
        let btn_height = 36.0;
        let spacing = if compact { 4.0 } else { 10.0 };
        let font_size = 16.0;

        let logo_h = 46.0;
        let logo_margin = 4.0;
        let subtitle_h = 16.0;
        let form_gap = if compact { 2.0 } else { 8.0 };
        let label_h = 12.0;
        let field_gap = if compact { 14.0 } else { spacing + 4.0 };
        let buttons_gap = spacing + 14.0;

        let checkbox_row_h = 20.0;
        let checkbox_gap = if compact { 4.0 } else { 8.0 };

        let form_content_h = subtitle_h
            + form_gap
            + label_h
            + box_height
            + field_gap
            + label_h
            + box_height
            + checkbox_gap
            + checkbox_row_h
            + spacing
            + btn_height;

        let form_content_top = ((sh - form_content_h) / 2.0)
            .max(logo_h + logo_margin + 6.0)
            .floor();

        // Semi-transparent panel behind the form
        let panel_padding = 20.0;
        let panel_x = (box_x - panel_padding).floor();
        let panel_y = (form_content_top - panel_padding).floor();
        let panel_w = (box_width + panel_padding * 2.0).floor();
        let panel_h = (form_content_h + panel_padding * 2.0).floor();

        // Panel background (no border)
        draw_rectangle(
            panel_x,
            panel_y,
            panel_w,
            panel_h,
            Color::from_rgba(20, 20, 35, 180),
        );

        // Logo (placed above the panel)
        let logo_y = panel_y - logo_margin - logo_h;
        if let Some(logo) = &self.logo {
            let logo_scale = 0.25;
            let logo_w = logo.width() * logo_scale;
            let logo_actual_h = logo.height() * logo_scale;
            let logo_x = (sw - logo_w) / 2.0;
            draw_texture_ex(
                logo,
                logo_x.floor(),
                logo_y.max(4.0).floor(),
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(logo_w.floor(), logo_actual_h.floor())),
                    ..Default::default()
                },
            );
        } else {
            let title = "NEW AEVEN";
            let title_width = self.measure_text_sharp(title, 16.0).width;
            self.draw_text_sharp(
                title,
                (sw - title_width) / 2.0,
                logo_y.max(4.0) + 22.0,
                16.0,
                WHITE,
            );
        }

        // Subtitle
        let subtitle = match self.mode {
            LoginMode::Login => "Login",
            LoginMode::Register => "Register",
        };
        self.draw_text_sharp(subtitle, box_x, form_content_top, 16.0, GRAY);

        // Username field
        let username_y = (form_content_top + subtitle_h + form_gap).floor();
        let username_active = self.active_field == LoginField::Username;
        let username_color = if username_active {
            Color::from_rgba(60, 90, 140, 200)
        } else {
            Color::from_rgba(40, 40, 60, 180)
        };

        self.draw_text_sharp("Username", box_x, username_y, font_size, LIGHTGRAY);
        let field_y = (username_y + label_h).floor();
        draw_rectangle(box_x, field_y, box_width, box_height, username_color);
        draw_rectangle_lines(
            box_x,
            field_y,
            box_width,
            box_height,
            2.0,
            if username_active { WHITE } else { GRAY },
        );

        let username_display = if self.username.is_empty() && !username_active {
            "Enter username...".to_string()
        } else {
            let cursor = if username_active && (get_time() * 2.0) as i32 % 2 == 0 {
                "|"
            } else {
                ""
            };
            format!("{}{}", self.username, cursor)
        };
        let text_color = if self.username.is_empty() && !username_active {
            DARKGRAY
        } else {
            WHITE
        };
        self.draw_text_sharp(
            &username_display,
            box_x + 10.0,
            field_y + (box_height + font_size) / 2.0,
            font_size,
            text_color,
        );

        // Password field
        let password_y = (field_y + box_height + field_gap).floor();
        let password_active = self.active_field == LoginField::Password;
        let password_color = if password_active {
            Color::from_rgba(60, 90, 140, 200)
        } else {
            Color::from_rgba(40, 40, 60, 180)
        };

        self.draw_text_sharp("Password", box_x, password_y, font_size, LIGHTGRAY);
        let pass_field_y = (password_y + label_h).floor();
        draw_rectangle(box_x, pass_field_y, box_width, box_height, password_color);
        draw_rectangle_lines(
            box_x,
            pass_field_y,
            box_width,
            box_height,
            2.0,
            if password_active { WHITE } else { GRAY },
        );

        let password_display = if self.password.is_empty() && !password_active {
            "Enter password...".to_string()
        } else {
            let masked: String = "*".repeat(self.password.len());
            let cursor = if password_active && (get_time() * 2.0) as i32 % 2 == 0 {
                "|"
            } else {
                ""
            };
            format!("{}{}", masked, cursor)
        };
        let text_color = if self.password.is_empty() && !password_active {
            DARKGRAY
        } else {
            WHITE
        };
        self.draw_text_sharp(
            &password_display,
            box_x + 10.0,
            pass_field_y + (box_height + font_size) / 2.0,
            font_size,
            text_color,
        );

        // Remember me checkbox
        let remember_y = (pass_field_y + box_height + checkbox_gap).floor();
        let cb_size = 16.0;
        let cb_x = box_x;
        let cb_y = remember_y + (checkbox_row_h - cb_size) / 2.0;
        draw_rectangle(
            cb_x,
            cb_y,
            cb_size,
            cb_size,
            Color::from_rgba(40, 40, 60, 180),
        );
        draw_rectangle_lines(cb_x, cb_y, cb_size, cb_size, 2.0, GRAY);
        if self.remember_me {
            // Draw checkmark as two lines
            draw_line(cb_x + 3.0, cb_y + 8.0, cb_x + 6.0, cb_y + 12.0, 2.0, GREEN);
            draw_line(cb_x + 6.0, cb_y + 12.0, cb_x + 13.0, cb_y + 4.0, 2.0, GREEN);
        }
        self.draw_text_sharp(
            "Remember me",
            cb_x + cb_size + 6.0,
            remember_y + 17.0,
            font_size,
            LIGHTGRAY,
        );

        // Buttons row
        let buttons_y = (remember_y + checkbox_row_h + spacing).floor();
        let btn_w = ((box_width - spacing) / 2.0).floor();

        // Login/Register button (left)
        let enter_text = match self.mode {
            LoginMode::Login => "Login",
            LoginMode::Register => "Register",
        };
        let login_hovered = point_in_rect(mx, my, box_x, buttons_y, btn_w, btn_height);
        let login_bg = if login_hovered {
            Color::from_rgba(60, 140, 90, 255)
        } else {
            Color::from_rgba(40, 100, 60, 255)
        };
        let login_border = if login_hovered {
            Color::from_rgba(100, 255, 150, 255)
        } else {
            GREEN
        };
        draw_rectangle(box_x, buttons_y, btn_w, btn_height, login_bg);
        draw_rectangle_lines(box_x, buttons_y, btn_w, btn_height, 2.0, login_border);
        // Double-line border trick for rounded look
        draw_rectangle_lines(
            box_x + 1.0,
            buttons_y + 1.0,
            btn_w - 2.0,
            btn_height - 2.0,
            1.0,
            Color::new(1.0, 1.0, 1.0, 0.1),
        );
        let enter_w = self.measure_text_sharp(enter_text, font_size).width;
        self.draw_text_sharp(
            enter_text,
            (box_x + (btn_w - enter_w) / 2.0).floor(),
            buttons_y + 24.0,
            font_size,
            WHITE,
        );

        // Toggle mode button (right)
        let toggle_text = match self.mode {
            LoginMode::Login => "Register",
            LoginMode::Register => "Login",
        };
        let toggle_x = (box_x + btn_w + spacing).floor();
        let toggle_hovered = point_in_rect(mx, my, toggle_x, buttons_y, btn_w, btn_height);
        let toggle_bg = if toggle_hovered {
            Color::from_rgba(120, 120, 60, 255)
        } else {
            Color::from_rgba(80, 80, 40, 255)
        };
        let toggle_border = if toggle_hovered {
            Color::from_rgba(255, 255, 100, 255)
        } else {
            YELLOW
        };
        draw_rectangle(toggle_x, buttons_y, btn_w, btn_height, toggle_bg);
        draw_rectangle_lines(toggle_x, buttons_y, btn_w, btn_height, 2.0, toggle_border);
        draw_rectangle_lines(
            toggle_x + 1.0,
            buttons_y + 1.0,
            btn_w - 2.0,
            btn_height - 2.0,
            1.0,
            Color::new(1.0, 1.0, 1.0, 0.1),
        );
        let toggle_w = self.measure_text_sharp(toggle_text, font_size).width;
        self.draw_text_sharp(
            toggle_text,
            (toggle_x + (btn_w - toggle_w) / 2.0).floor(),
            buttons_y + 24.0,
            font_size,
            WHITE,
        );

        // Error message (below buttons)
        if let Some(ref error) = self.error_message {
            self.draw_text_sharp(error, box_x, buttons_y + btn_height + 14.0, 16.0, RED);
        }

        // Version (bottom right)
        let version_text = format!("v{}", env!("CARGO_PKG_VERSION"));
        let version_w = self.measure_text_sharp(&version_text, 16.0).width;
        self.draw_text_sharp(
            &version_text,
            (sw - version_w - 10.0).floor(),
            sh - 10.0,
            16.0,
            WHITE,
        );

        // Server status (bottom left)
        let status_dot_color = if self.server_online {
            Color::from_rgba(80, 200, 80, 255)
        } else {
            Color::from_rgba(200, 60, 60, 255)
        };
        let status_text = if self.server_online {
            "Connected"
        } else {
            "Disconnected"
        };
        let status_y = sh - 10.0;
        draw_rectangle(10.0, status_y - 6.0, 6.0, 6.0, status_dot_color);
        self.draw_text_sharp(status_text, 20.0, status_y, 16.0, WHITE);
    }
}

// ============================================================================
// Character Select Screen
// ============================================================================

/// Maximum characters per account
const MAX_CHARACTERS: usize = 3;

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
        if self.player_sprites.len() > 0 {
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
        if self.player_sprites.len() == 0 {
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
        if self.hair_sprites.len() == 0 {
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
            for slot in [
                &c.sprite_head,
                &c.sprite_body,
                &c.sprite_weapon,
                &c.sprite_back,
                &c.sprite_feet,
            ] {
                if let Some(key) = slot {
                    if !sprite_keys.contains(key) && !self.equipment_sprites.contains(key) {
                        sprite_keys.push(key.clone());
                    }
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
            if point_in_rect(mx, my, list_x, inst_y - 10.0, 100.0, 30.0) {
                if !self.characters.is_empty() {
                    let character = &self.characters[self.selected_index];
                    return ScreenState::StartGame {
                        session: self.session.clone(),
                        character_id: character.id,
                        character_name: character.name.clone(),
                    };
                }
            }

            // New button
            if self.characters.len() < MAX_CHARACTERS {
                if point_in_rect(mx, my, list_x + 120.0, inst_y - 10.0, 70.0, 30.0) {
                    return ScreenState::ToCharacterCreate(self.session.clone());
                }
            }

            // Delete button
            if point_in_rect(mx, my, list_x + 210.0, inst_y - 10.0, 90.0, 30.0) {
                if !self.characters.is_empty() {
                    self.confirm_delete = true;
                }
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
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            }
        }
        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
            if self.selected_index < self.characters.len().saturating_sub(1) {
                self.selected_index += 1;
            }
        }

        // Keyboard: Create new character
        if is_key_pressed(KeyCode::N) && self.characters.len() < MAX_CHARACTERS {
            return ScreenState::ToCharacterCreate(self.session.clone());
        }

        // Keyboard: Delete character
        if is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::X) {
            if !self.characters.is_empty() {
                self.confirm_delete = true;
            }
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
        if is_key_pressed(KeyCode::Enter) {
            if !self.characters.is_empty() {
                let character = &self.characters[self.selected_index];
                return ScreenState::StartGame {
                    session: self.session.clone(),
                    character_id: character.id,
                    character_name: character.name.clone(),
                };
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

        // Solid background below the list to cleanly separate from buttons
        let button_zone_y = list_y + list_visible_height;
        draw_rectangle(
            0.0,
            button_zone_y,
            sw,
            sh - button_zone_y,
            Color::from_rgba(25, 25, 35, 255),
        );

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

// ============================================================================
// Character Create Screen
// ============================================================================

const GENDERS: [&str; 2] = ["male", "female"];

const SKINS: [&str; 7] = ["tan", "pale", "brown", "fish", "orc", "panda", "skeleton"];

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
