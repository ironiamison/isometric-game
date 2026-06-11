use super::*;

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
                draw_rectangle(0.0, y, sw, h, Color::from_rgba(r, g, b, (255.0 * sa) as u8));
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
