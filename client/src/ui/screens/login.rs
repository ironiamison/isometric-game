use super::*;
use crate::render::ui::common::{
    CORNER_ACCENT_SIZE, FRAME_ACCENT, FRAME_INNER, FRAME_MID, FRAME_OUTER, FRAME_THICKNESS,
    PANEL_BG_DARK, TEXT_DIM, TEXT_GOLD, TEXT_NORMAL, TEXT_TITLE,
};

// Placeholder external links — swap in the real Discord invite / news URL when ready.
const DISCORD_URL: &str = "https://discord.gg/VHB9qSyhUF";
#[allow(dead_code)] // News is "coming soon" — wired for layout, not yet clickable.
const NEWS_URL: &str = "https://example.com/news";

/// Open an external URL in the system browser. Platform-specific:
/// - Desktop: the `open` crate launches the default browser.
/// - WASM: `window.open` via the JS plugin in `web/auth.js`.
/// - Android: no-op for now (needs JNI plumbing — see TODO).
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn open_external_url(url: &str) {
    if let Err(e) = open::that(url) {
        log::warn!("failed to open url {url}: {e}");
    }
}

#[cfg(target_arch = "wasm32")]
fn open_external_url(url: &str) {
    use sapp_jsutils::JsObject;
    extern "C" {
        fn open_url(url: JsObject);
    }
    let js = JsObject::string(url);
    unsafe { open_url(js) };
}

#[cfg(target_os = "android")]
fn open_external_url(url: &str) {
    // TODO: Android URL open requires JNI plumbing; no-op for now.
    let _ = url;
}

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

/// Precomputed positions for every element of the login panel. Computed once
/// from the screen size and shared by both `update` (hit-testing) and `render`
/// (drawing) so the two can never drift apart.
struct LoginLayout {
    panel: Rect,
    content_x: f32,
    content_w: f32,
    title_baseline: f32,
    error_baseline: f32,
    tab_login: Rect,
    tab_register: Rect,
    tab_underline_y: f32,
    username_label_baseline: f32,
    username_field: Rect,
    password_label_baseline: f32,
    password_field: Rect,
    remember_box: Rect,
    remember_row: Rect,
    button: Rect,
    divider_y: f32,
    footer_center_y: f32,
    discord_icon: Rect,
    news_icon: Rect,
}

pub struct LoginScreen {
    username: String,
    password: String,
    active_field: LoginField,
    mode: LoginMode,
    error_message: Option<String>,
    auth_client: AuthClient,
    font: BitmapFont,
    discord_icon: Option<Texture2D>,
    news_icon: Option<Texture2D>,
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
            discord_icon: None,
            news_icon: None,
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

    /// Load font and textures asynchronously - call this after creating the screen
    pub async fn load_font(&mut self) {
        // Font may already be set via use_renderer_font()
        let font_already_loaded = !self.font.is_empty();

        if !font_already_loaded {
            self.font =
                BitmapFont::load_or_default("assets/fonts/monogram/ttf/monogram-extended.ttf")
                    .await;
        }

        // Footer social/news icons (fall back to primitive shapes if missing)
        if let Ok(texture) = load_texture(&asset_path("assets/ui/discord.png")).await {
            texture.set_filter(FilterMode::Nearest);
            self.discord_icon = Some(texture);
        }
        if let Ok(texture) = load_texture(&asset_path("assets/ui/docs.png")).await {
            texture.set_filter(FilterMode::Nearest);
            self.news_icon = Some(texture);
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

    /// Compute every element rect/baseline for the centered login panel.
    /// Shared by `update` (hit-testing) and `render` (drawing).
    fn compute_layout(sw: f32, sh: f32) -> LoginLayout {
        let compact = sh < 480.0;

        // Inner content column (fields, button, tabs all share this width).
        let content_w = (sw - 48.0).min(320.0).max(220.0).floor();
        let pad_x = 24.0;
        let pad_top = 22.0;
        let pad_bottom = 18.0;
        let panel_w = (content_w + pad_x * 2.0).floor();

        // Section heights
        let title_h = 36.0;
        let tab_h = 30.0;
        let label_h = 18.0;
        let field_h = if compact { 32.0 } else { 40.0 };
        let remember_h = 22.0;
        let button_h = 42.0;
        let footer_h = 24.0;
        let error_h = 18.0; // reserved row for the validation/error message

        // Gaps (title/tab gaps kept tight so the error row fits without growth)
        let g_after_title = if compact { 8.0 } else { 12.0 };
        let g_after_tabs = if compact { 10.0 } else { 14.0 };
        let g_label_field = 4.0;
        let g_after_field = if compact { 10.0 } else { 16.0 };
        let g_before_error = 6.0;
        let g_after_error = 6.0;
        let g_footer_top = 22.0; // extra headroom below divider for the "soon" label
        let g_after_remember = if compact { 10.0 } else { 16.0 };
        let g_after_button = if compact { 10.0 } else { 16.0 };
        let g_divider = 12.0;

        let content_h: f32 = title_h
            + g_after_title
            + tab_h
            + g_after_tabs
            + label_h
            + g_label_field
            + field_h
            + g_after_field
            + label_h
            + g_label_field
            + field_h
            + g_before_error
            + error_h
            + g_after_error
            + remember_h
            + g_after_remember
            + button_h
            + g_after_button
            + g_divider
            + 1.0
            + g_footer_top
            + footer_h;

        let panel_h = (content_h + pad_top + pad_bottom).floor();
        let panel_x = ((sw - panel_w) / 2.0).floor();
        let panel_y = ((sh - panel_h) / 2.0).max(8.0).floor();
        let content_x = (panel_x + pad_x).floor();

        let mut y = panel_y + pad_top;

        // Title (baseline sits near the bottom of its reserved band)
        let title_baseline = (y + title_h - 8.0).floor();
        y += title_h + g_after_title;

        // Tabs
        let tab_w = (content_w / 2.0).floor();
        let tab_login = Rect::new(content_x, y, tab_w, tab_h);
        let tab_register = Rect::new(content_x + tab_w, y, content_w - tab_w, tab_h);
        let tab_underline_y = y + tab_h;
        y += tab_h + g_after_tabs;

        // Username
        let username_label_baseline = (y + label_h - 2.0).floor();
        y += label_h + g_label_field;
        let username_field = Rect::new(content_x, y.floor(), content_w, field_h);
        y += field_h + g_after_field;

        // Password
        let password_label_baseline = (y + label_h - 2.0).floor();
        y += label_h + g_label_field;
        let password_field = Rect::new(content_x, y.floor(), content_w, field_h);
        y += field_h + g_before_error;

        // Error/validation message row (always reserved so layout never shifts)
        let error_baseline = (y + error_h - 4.0).floor();
        y += error_h + g_after_error;

        // Remember me
        let cb_size = 18.0;
        let remember_row = Rect::new(content_x, y.floor(), content_w, remember_h);
        let remember_box = Rect::new(
            content_x,
            (y + (remember_h - cb_size) / 2.0).floor(),
            cb_size,
            cb_size,
        );
        y += remember_h + g_after_remember;

        // Primary button
        let button = Rect::new(content_x, y.floor(), content_w, button_h);
        y += button_h + g_after_button;

        // Divider
        y += g_divider;
        let divider_y = y.floor();
        y += 1.0 + g_footer_top;

        // Footer (status left, icons right)
        let footer_center_y = (y + footer_h / 2.0).floor();
        let icon_size = 24.0; // 1:1 with the 24x24 source PNGs for crisp pixels
        let icon_y = (footer_center_y - icon_size / 2.0).floor();
        let news_icon = Rect::new(
            content_x + content_w - icon_size,
            icon_y,
            icon_size,
            icon_size,
        );
        let discord_icon = Rect::new(news_icon.x - icon_size - 12.0, icon_y, icon_size, icon_size);

        LoginLayout {
            panel: Rect::new(panel_x, panel_y, panel_w, panel_h),
            content_x,
            content_w,
            title_baseline,
            error_baseline,
            tab_login,
            tab_register,
            tab_underline_y,
            username_label_baseline,
            username_field,
            password_label_baseline,
            password_field,
            remember_box,
            remember_row,
            button,
            divider_y,
            footer_center_y,
            discord_icon,
            news_icon,
        }
    }

    /// Validate the form and attempt login/register. Returns a screen
    /// transition on a synchronous (native) success; otherwise sets the error
    /// message or kicks off an async (WASM) request and returns None.
    fn submit(&mut self) -> Option<ScreenState> {
        if self.username.len() < 3 {
            self.error_message = Some("Username must be at least 3 characters".to_string());
            return None;
        }
        if self.password.len() < 6 {
            self.error_message = Some("Password must be at least 6 characters".to_string());
            return None;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let result = match self.mode {
                LoginMode::Login => self.auth_client.login(&self.username, &self.password),
                LoginMode::Register => self.auth_client.register(&self.username, &self.password),
            };
            match result {
                Ok(session) => {
                    self.save_remember_me();
                    return Some(ScreenState::ToCharacterSelect(session));
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
                LoginMode::Login => self.auth_client.start_login(&self.username, &self.password),
                LoginMode::Register => {
                    self.auth_client.start_register(&self.username, &self.password)
                }
            }
        }

        None
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

    /// Draw the multi-layer bronze/gold panel frame (mirrors the renderer's
    /// `draw_panel_frame`, with a more opaque dark backing for the login screen).
    fn draw_panel_frame(x: f32, y: f32, w: f32, h: f32) {
        // Drop shadow for depth against the night sky
        draw_rectangle(x - 3.0, y - 3.0, w + 6.0, h + 6.0, Color::new(0.0, 0.0, 0.0, 0.5));
        // Dark bronze outer frame
        draw_rectangle(x, y, w, h, FRAME_OUTER);
        // Mid bronze frame (inset 2px)
        draw_rectangle(x + 2.0, y + 2.0, w - 4.0, h - 4.0, FRAME_MID);
        // Main panel backing (inset by frame thickness) — solid dark "front door"
        draw_rectangle(
            x + FRAME_THICKNESS,
            y + FRAME_THICKNESS,
            w - FRAME_THICKNESS * 2.0,
            h - FRAME_THICKNESS * 2.0,
            PANEL_BG_DARK,
        );
        // Inner highlight (top + left)
        draw_line(
            x + FRAME_THICKNESS,
            y + FRAME_THICKNESS,
            x + w - FRAME_THICKNESS,
            y + FRAME_THICKNESS,
            1.0,
            FRAME_INNER,
        );
        draw_line(
            x + FRAME_THICKNESS,
            y + FRAME_THICKNESS,
            x + FRAME_THICKNESS,
            y + h - FRAME_THICKNESS,
            1.0,
            FRAME_INNER,
        );
        // Inner shadow (bottom + right)
        let shadow = Color::new(0.0, 0.0, 0.0, 0.235);
        draw_line(
            x + FRAME_THICKNESS + 1.0,
            y + h - FRAME_THICKNESS - 1.0,
            x + w - FRAME_THICKNESS,
            y + h - FRAME_THICKNESS - 1.0,
            1.0,
            shadow,
        );
        draw_line(
            x + w - FRAME_THICKNESS - 1.0,
            y + FRAME_THICKNESS + 1.0,
            x + w - FRAME_THICKNESS - 1.0,
            y + h - FRAME_THICKNESS,
            1.0,
            shadow,
        );
    }

    /// Gold L-shaped corner accents (mirrors the renderer's `draw_corner_accents`).
    fn draw_corner_accents(x: f32, y: f32, w: f32, h: f32) {
        let s = CORNER_ACCENT_SIZE + 4.0; // slightly bolder for the front door
        // Top-left
        draw_rectangle(x, y, s, 2.0, FRAME_ACCENT);
        draw_rectangle(x, y, 2.0, s, FRAME_ACCENT);
        // Top-right
        draw_rectangle(x + w - s, y, s, 2.0, FRAME_ACCENT);
        draw_rectangle(x + w - 2.0, y, 2.0, s, FRAME_ACCENT);
        // Bottom-left
        draw_rectangle(x, y + h - 2.0, s, 2.0, FRAME_ACCENT);
        draw_rectangle(x, y + h - s, 2.0, s, FRAME_ACCENT);
        // Bottom-right
        draw_rectangle(x + w - s, y + h - 2.0, s, 2.0, FRAME_ACCENT);
        draw_rectangle(x + w - 2.0, y + h - s, 2.0, s, FRAME_ACCENT);
    }

    /// Draw a labelled text field with gold-on-focus borders (no blue fill).
    fn draw_field(
        &self,
        rect: Rect,
        label: &str,
        label_baseline: f32,
        active: bool,
        display: &str,
        placeholder: &str,
        font_size: f32,
    ) {
        // Label (native 16px to stay crisp)
        let label_color = if active { TEXT_TITLE } else { TEXT_DIM };
        self.draw_text_sharp(label, rect.x, label_baseline, 16.0, label_color);

        // Recessed dark fill (consistent whether focused or not — focus = border)
        let fill = if active {
            Color::from_rgba(22, 22, 32, 250)
        } else {
            Color::from_rgba(14, 14, 20, 235)
        };
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);

        // Gold border when active, dim bronze when not
        let border = if active { FRAME_ACCENT } else { FRAME_OUTER };
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, border);

        // Text or placeholder
        let (text, color) = if display.is_empty() && !active {
            (placeholder.to_string(), TEXT_DIM)
        } else {
            (display.to_string(), TEXT_NORMAL)
        };
        self.draw_text_sharp(
            &text,
            rect.x + 10.0,
            rect.y + (rect.h + font_size) / 2.0,
            font_size,
            color,
        );
    }

    /// Draw a footer icon button. Uses the texture if loaded, otherwise a
    /// primitive fallback shape. `enabled` dims the icon (used for "soon" news).
    fn draw_footer_icon(
        &self,
        rect: Rect,
        texture: &Option<Texture2D>,
        hovered: bool,
        enabled: bool,
        is_book: bool,
    ) {
        if hovered && enabled {
            // Subtle gold hit-box highlight on hover
            draw_rectangle(
                rect.x - 3.0,
                rect.y - 3.0,
                rect.w + 6.0,
                rect.h + 6.0,
                Color::new(FRAME_ACCENT.r, FRAME_ACCENT.g, FRAME_ACCENT.b, 0.18),
            );
        }

        match texture {
            Some(tex) => {
                let alpha = if enabled { 1.0 } else { 0.4 };
                draw_texture_ex(
                    tex,
                    rect.x,
                    rect.y,
                    Color::new(1.0, 1.0, 1.0, alpha),
                    DrawTextureParams {
                        dest_size: Some(vec2(rect.w, rect.h)),
                        ..Default::default()
                    },
                );
            }
            None => {
                let c = if !enabled {
                    TEXT_DIM
                } else if hovered {
                    FRAME_ACCENT
                } else {
                    Color::new(0.78, 0.78, 0.85, 1.0)
                };
                if is_book {
                    // Book/scroll: outline + spine + two "text" lines
                    draw_rectangle_lines(rect.x + 2.0, rect.y + 2.0, rect.w - 4.0, rect.h - 4.0, 2.0, c);
                    let mid = rect.x + rect.w / 2.0;
                    draw_line(mid, rect.y + 3.0, mid, rect.y + rect.h - 3.0, 1.0, c);
                    draw_line(rect.x + 5.0, rect.y + 8.0, mid - 2.0, rect.y + 8.0, 1.0, c);
                    draw_line(rect.x + 5.0, rect.y + 13.0, mid - 2.0, rect.y + 13.0, 1.0, c);
                } else {
                    // Speech bubble: rounded body + tail + two eyes
                    draw_rectangle(rect.x + 1.0, rect.y + 3.0, rect.w - 2.0, rect.h - 9.0, c);
                    draw_rectangle(rect.x + 3.0, rect.y + 1.0, rect.w - 6.0, rect.h - 5.0, c);
                    draw_triangle(
                        vec2(rect.x + 5.0, rect.y + rect.h - 5.0),
                        vec2(rect.x + 11.0, rect.y + rect.h - 5.0),
                        vec2(rect.x + 5.0, rect.y + rect.h),
                        c,
                    );
                    let eye = Color::new(0.07, 0.07, 0.09, 1.0);
                    draw_rectangle(rect.x + 6.0, rect.y + 7.0, 3.0, 3.0, eye);
                    draw_rectangle(rect.x + 13.0, rect.y + 7.0, 3.0, 3.0, eye);
                }
            }
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
            if pseudo.is_multiple_of(200) {
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

        // Handle clicks against the shared layout
        if clicked {
            let l = Self::compute_layout(sw, sh);
            let hit = |r: Rect| point_in_rect(mx, my, r.x, r.y, r.w, r.h);

            if hit(l.username_field) {
                self.active_field = LoginField::Username;
                show_keyboard(true);
            } else if hit(l.password_field) {
                self.active_field = LoginField::Password;
                show_keyboard(true);
            } else if hit(l.tab_login) {
                if self.mode != LoginMode::Login {
                    audio.play_sfx("enter");
                }
                self.mode = LoginMode::Login;
                self.error_message = None;
            } else if hit(l.tab_register) {
                if self.mode != LoginMode::Register {
                    audio.play_sfx("enter");
                }
                self.mode = LoginMode::Register;
                self.error_message = None;
            } else if hit(l.remember_row) {
                self.remember_me = !self.remember_me;
            } else if hit(l.button) {
                show_keyboard(false);
                if let Some(state) = self.submit() {
                    return state;
                }
            } else if hit(l.discord_icon) {
                open_external_url(DISCORD_URL);
            } else if hit(l.news_icon) {
                // News is coming soon — no action yet.
            } else {
                // Tapped empty space — dismiss the on-screen keyboard
                show_keyboard(false);
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

        // Toggle between login/register (keyboard shortcut)
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
            if let Some(state) = self.submit() {
                return state;
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

        // === FORM PANEL ===

        let l = Self::compute_layout(sw, sh);
        let font_size = 16.0;
        let hit = |r: Rect| point_in_rect(mx, my, r.x, r.y, r.w, r.h);

        // Gold-bevel frame + corner accents (the "front door")
        Self::draw_panel_frame(l.panel.x, l.panel.y, l.panel.w, l.panel.h);
        Self::draw_corner_accents(l.panel.x, l.panel.y, l.panel.w, l.panel.h);

        // Title wordmark — drawn as text at a native font size (32, falling back
        // to 24 if it would overflow) to stay pixel-crisp. The logo.png is
        // 384x244 and can't downscale into this panel without blurring.
        let title = "NEW AEVEN";
        let title_size = if self.measure_text_sharp(title, 32.0).width <= l.content_w {
            32.0
        } else {
            24.0
        };
        let title_w = self.measure_text_sharp(title, title_size).width;
        self.draw_text_sharp(
            title,
            (l.panel.x + (l.panel.w - title_w) / 2.0).floor(),
            l.title_baseline,
            title_size,
            TEXT_TITLE,
        );

        // === TABS ===
        let draw_tab = |this: &Self, rect: Rect, label: &str, active: bool| {
            let hovered = hit(rect);
            let color = if active {
                TEXT_TITLE
            } else if hovered {
                TEXT_NORMAL
            } else {
                TEXT_DIM
            };
            let tw = this.measure_text_sharp(label, font_size).width;
            this.draw_text_sharp(
                label,
                (rect.x + (rect.w - tw) / 2.0).floor(),
                (rect.y + rect.h / 2.0 + 6.0).floor(),
                font_size,
                color,
            );
        };
        draw_tab(self, l.tab_login, "Login", self.mode == LoginMode::Login);
        draw_tab(
            self,
            l.tab_register,
            "Register",
            self.mode == LoginMode::Register,
        );
        // Underline: faint full-width baseline + bright gold under the active tab
        draw_line(
            l.content_x,
            l.tab_underline_y,
            l.content_x + l.content_w,
            l.tab_underline_y,
            1.0,
            FRAME_OUTER,
        );
        let active_tab = if self.mode == LoginMode::Login {
            l.tab_login
        } else {
            l.tab_register
        };
        draw_rectangle(active_tab.x, l.tab_underline_y - 1.0, active_tab.w, 2.0, FRAME_ACCENT);

        // === FIELDS ===
        let username_active = self.active_field == LoginField::Username;
        let cursor_on = (get_time() * 2.0) as i32 % 2 == 0;
        let username_display = if self.username.is_empty() && !username_active {
            String::new()
        } else {
            let cursor = if username_active && cursor_on { "|" } else { "" };
            format!("{}{}", self.username, cursor)
        };
        self.draw_field(
            l.username_field,
            "USERNAME",
            l.username_label_baseline,
            username_active,
            &username_display,
            "Enter username...",
            font_size,
        );

        let password_active = self.active_field == LoginField::Password;
        let password_display = if self.password.is_empty() && !password_active {
            String::new()
        } else {
            let masked: String = "•".repeat(self.password.len());
            let cursor = if password_active && cursor_on { "|" } else { "" };
            format!("{}{}", masked, cursor)
        };
        self.draw_field(
            l.password_field,
            "PASSWORD",
            l.password_label_baseline,
            password_active,
            &password_display,
            "Enter password...",
            font_size,
        );

        // === REMEMBER ME ===
        let cb = l.remember_box;
        draw_rectangle(cb.x, cb.y, cb.w, cb.h, Color::from_rgba(14, 14, 20, 235));
        draw_rectangle_lines(
            cb.x,
            cb.y,
            cb.w,
            cb.h,
            2.0,
            if self.remember_me { FRAME_ACCENT } else { FRAME_OUTER },
        );
        if self.remember_me {
            draw_line(cb.x + 4.0, cb.y + 9.0, cb.x + 7.0, cb.y + 13.0, 2.0, FRAME_ACCENT);
            draw_line(cb.x + 7.0, cb.y + 13.0, cb.x + 14.0, cb.y + 5.0, 2.0, FRAME_ACCENT);
        }
        self.draw_text_sharp(
            "Remember me",
            cb.x + cb.w + 8.0,
            cb.y + cb.h - 3.0,
            font_size,
            TEXT_NORMAL,
        );

        // === PRIMARY BUTTON ===
        let btn = l.button;
        let btn_hovered = hit(btn);
        let btn_bg = if btn_hovered {
            Color::from_rgba(64, 50, 28, 255)
        } else {
            Color::from_rgba(44, 34, 18, 255)
        };
        let btn_border = if btn_hovered { TEXT_GOLD } else { FRAME_ACCENT };
        draw_rectangle(btn.x, btn.y, btn.w, btn.h, btn_bg);
        draw_rectangle_lines(btn.x, btn.y, btn.w, btn.h, 2.0, btn_border);
        let btn_text = match self.mode {
            LoginMode::Login => "Enter Aeven",
            LoginMode::Register => "Create Account",
        };
        let bt_w = self.measure_text_sharp(btn_text, font_size).width;
        self.draw_text_sharp(
            btn_text,
            (btn.x + (btn.w - bt_w) / 2.0).floor(),
            (btn.y + btn.h / 2.0 + 6.0).floor(),
            font_size,
            TEXT_TITLE,
        );

        // Error message — in the reserved row under the password field
        if let Some(ref error) = self.error_message {
            self.draw_text_sharp(error, l.content_x, l.error_baseline, 16.0, RED);
        }

        // === FOOTER ===
        draw_line(
            l.content_x,
            l.divider_y,
            l.content_x + l.content_w,
            l.divider_y,
            1.0,
            FRAME_OUTER,
        );

        // Server status (left)
        let status_color = if self.server_online {
            Color::from_rgba(80, 200, 80, 255)
        } else {
            Color::from_rgba(200, 60, 60, 255)
        };
        let status_text = if self.server_online { "Online" } else { "Offline" };
        draw_circle(l.content_x + 4.0, l.footer_center_y, 4.0, status_color);
        self.draw_text_sharp(
            status_text,
            l.content_x + 14.0,
            l.footer_center_y + 5.0,
            font_size,
            TEXT_NORMAL,
        );

        // Footer icons (right): Discord (clickable) + News (soon)
        self.draw_footer_icon(l.discord_icon, &self.discord_icon, hit(l.discord_icon), true, false);
        self.draw_footer_icon(l.news_icon, &self.news_icon, hit(l.news_icon), false, true);
        // "soon" tag above the news icon
        let soon = "soon";
        let soon_w = self.measure_text_sharp(soon, 16.0).width;
        self.draw_text_sharp(
            soon,
            (l.news_icon.x + (l.news_icon.w - soon_w) / 2.0).floor(),
            l.news_icon.y - 5.0,
            16.0,
            TEXT_DIM,
        );

        // Version (bottom right of screen)
        let version_text = format!("v{}", env!("CARGO_PKG_VERSION"));
        let version_w = self.measure_text_sharp(&version_text, 16.0).width;
        self.draw_text_sharp(
            &version_text,
            (sw - version_w - 10.0).floor(),
            sh - 10.0,
            16.0,
            TEXT_DIM,
        );
    }
}

// ============================================================================
// Character Select Screen
// ============================================================================
