use super::*;
#[cfg(target_arch = "wasm32")]
use crate::auth::{
    is_wallet_available, poll_wallet_sign, start_wallet_sign, AuthResult, WalletSignPoll,
};
use crate::render::ui::common::{
    draw_corner_accents, draw_panel_frame, FRAME_ACCENT, FRAME_OUTER, TEXT_DIM, TEXT_GOLD,
    TEXT_NORMAL, TEXT_TITLE,
};

const DISCORD_URL: &str = "https://discord.gg/VHB9qSyhUF";

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
    let _ = url;
}

struct LoginLayout {
    panel: Rect,
    content_x: f32,
    content_w: f32,
    title_baseline: f32,
    subtitle_baseline: f32,
    error_baseline: f32,
    guest_button: Rect,
    wallet_button: Rect,
    divider_y: f32,
    footer_center_y: f32,
    discord_icon: Rect,
}

pub struct LoginScreen {
    error_message: Option<String>,
    auth_client: AuthClient,
    font: BitmapFont,
    discord_icon: Option<Texture2D>,
    frame_counter: f32,
    starfield: StarfieldBackground,
    server_url: String,
    server_online: bool,
    last_ping_time: f32,
    #[cfg(not(target_arch = "wasm32"))]
    health_rx: Option<mpsc::Receiver<bool>>,
    #[cfg(target_arch = "wasm32")]
    loading: bool,
    #[cfg(target_arch = "wasm32")]
    wallet_sign_id: Option<i32>,
    #[cfg(target_arch = "wasm32")]
    wallet_nonce: Option<String>,
    keyboard_shown: bool,
    stars_alpha: f32,
}

impl LoginScreen {
    pub fn new(server_url: &str) -> Self {
        Self {
            error_message: None,
            auth_client: AuthClient::new(server_url),
            font: BitmapFont::default(),
            discord_icon: None,
            frame_counter: 0.0,
            starfield: StarfieldBackground::new(),
            server_url: server_url.to_string(),
            server_online: false,
            last_ping_time: -10.0,
            #[cfg(not(target_arch = "wasm32"))]
            health_rx: None,
            #[cfg(target_arch = "wasm32")]
            loading: false,
            #[cfg(target_arch = "wasm32")]
            wallet_sign_id: None,
            #[cfg(target_arch = "wasm32")]
            wallet_nonce: None,
            keyboard_shown: false,
            stars_alpha: 1.0,
        }
    }

    pub fn use_renderer_font(&mut self, font: BitmapFont) {
        self.font = font;
    }

    pub fn set_stars_alpha(&mut self, alpha: f32) {
        self.stars_alpha = alpha;
    }

    pub async fn load_font(&mut self) {
        if self.font.is_empty() {
            self.font =
                BitmapFont::load_or_default("assets/fonts/monogram/ttf/monogram-extended.ttf")
                    .await;
        }
        if let Ok(texture) = load_texture(&asset_path("assets/ui/discord.png")).await {
            texture.set_filter(FilterMode::Nearest);
            self.discord_icon = Some(texture);
        }
    }

    fn draw_text_sharp(&self, text: &str, x: f32, y: f32, font_size: f32, color: Color) {
        self.font.draw_text(text, x, y, font_size, color);
    }

    fn measure_text_sharp(&self, text: &str, font_size: f32) -> TextDimensions {
        self.font.measure_text(text, font_size)
    }

    fn compute_layout(sw: f32, sh: f32) -> LoginLayout {
        let content_w = (sw - 48.0).clamp(240.0, 340.0).floor();
        let pad_x = 24.0;
        let pad_top = 28.0;
        let pad_bottom = 20.0;
        let panel_w = (content_w + pad_x * 2.0).floor();

        let title_h = 40.0;
        let subtitle_h = 36.0;
        let error_h = 20.0;
        let button_h = 46.0;
        let footer_h = 24.0;
        let g = 14.0;

        let content_h: f32 =
            title_h + g + subtitle_h + g + error_h + g + button_h + g + button_h + 24.0 + footer_h;
        let panel_h = (content_h + pad_top + pad_bottom).floor();
        let panel_x = ((sw - panel_w) / 2.0).floor();
        let panel_y = ((sh - panel_h) / 2.0).max(8.0).floor();
        let content_x = (panel_x + pad_x).floor();

        let mut y = panel_y + pad_top;
        let title_baseline = (y + title_h - 10.0).floor();
        y += title_h + g;
        let subtitle_baseline = (y + subtitle_h - 8.0).floor();
        y += subtitle_h + g;
        let error_baseline = (y + error_h - 4.0).floor();
        y += error_h + g;
        let guest_button = Rect::new(content_x, y.floor(), content_w, button_h);
        y += button_h + g;
        let wallet_button = Rect::new(content_x, y.floor(), content_w, button_h);
        y += button_h + 24.0;
        let divider_y = y.floor();
        y += 12.0;
        let footer_center_y = (y + footer_h / 2.0).floor();
        let icon_size = 24.0;
        let icon_y = (footer_center_y - icon_size / 2.0).floor();
        let discord_icon = Rect::new(
            content_x + content_w - icon_size,
            icon_y,
            icon_size,
            icon_size,
        );

        LoginLayout {
            panel: Rect::new(panel_x, panel_y, panel_w, panel_h),
            content_x,
            content_w,
            title_baseline,
            subtitle_baseline,
            error_baseline,
            guest_button,
            wallet_button,
            divider_y,
            footer_center_y,
            discord_icon,
        }
    }

    fn play_as_guest(&mut self) -> Option<ScreenState> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match self.auth_client.guest() {
                Ok(session) => return Some(ScreenState::ToCharacterSelect(session)),
                Err(e) => self.error_message = Some(e.to_string()),
            }
        }

        #[cfg(target_arch = "wasm32")]
        if !self.auth_client.is_busy() {
            self.loading = true;
            self.auth_client.start_guest();
        }

        None
    }

    fn connect_wallet(&mut self) -> Option<ScreenState> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.error_message = Some(
                "Wallet login works in the browser — open /play/ with Phantom installed."
                    .to_string(),
            );
        }

        #[cfg(target_arch = "wasm32")]
        {
            if !is_wallet_available() {
                self.error_message =
                    Some("Install Phantom wallet, or use Play as Guest.".to_string());
                return None;
            }
            if !self.auth_client.is_busy() && self.wallet_sign_id.is_none() {
                self.loading = true;
                self.error_message = None;
                self.auth_client.start_wallet_challenge();
            }
        }

        None
    }

    fn draw_button(&self, rect: Rect, label: &str, hovered: bool, enabled: bool) {
        let bg = if !enabled {
            Color::from_rgba(28, 28, 36, 255)
        } else if hovered {
            Color::from_rgba(64, 50, 28, 255)
        } else {
            Color::from_rgba(44, 34, 18, 255)
        };
        let border = if !enabled {
            FRAME_OUTER
        } else if hovered {
            TEXT_GOLD
        } else {
            FRAME_ACCENT
        };
        let text_color = if enabled { TEXT_TITLE } else { TEXT_DIM };

        draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, border);
        let tw = self.measure_text_sharp(label, 16.0).width;
        self.draw_text_sharp(
            label,
            (rect.x + (rect.w - tw) / 2.0).floor(),
            (rect.y + rect.h / 2.0 + 6.0).floor(),
            16.0,
            text_color,
        );
    }
}

impl Screen for LoginScreen {
    fn update(&mut self, audio: &AudioManager) -> ScreenState {
        let (sw, sh) = virtual_screen_size();
        let (input_pos, clicked, _) = get_input_state();
        let mx = input_pos.x;
        let my = input_pos.y;

        if !self.keyboard_shown {
            self.keyboard_shown = true;
            show_keyboard(false);
        }

        let dt = get_frame_time();
        self.frame_counter += dt;
        self.starfield.update(dt, sw, sh);

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(rx) = &self.health_rx {
                if let Ok(online) = rx.try_recv() {
                    self.server_online = online;
                    self.health_rx = None;
                }
            }
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
            if let Some(sign_id) = self.wallet_sign_id {
                match poll_wallet_sign(sign_id) {
                    WalletSignPoll::Pending => {}
                    WalletSignPoll::Done(result) => {
                        self.wallet_sign_id = None;
                        if let Some(nonce) = self.wallet_nonce.take() {
                            if !self.auth_client.is_busy() {
                                self.loading = true;
                                self.auth_client.start_wallet_login(
                                    &result.pubkey,
                                    &result.signature,
                                    &nonce,
                                );
                            }
                        } else {
                            self.loading = false;
                            self.error_message =
                                Some("Wallet sign-in state lost — try again.".to_string());
                        }
                    }
                    WalletSignPoll::Failed(err) => {
                        self.wallet_sign_id = None;
                        self.loading = false;
                        self.error_message = Some(err);
                    }
                }
            }

            if let Some(result) = self.auth_client.poll() {
                match result {
                    AuthResult::Guest(Ok(session)) => {
                        self.loading = false;
                        return ScreenState::ToCharacterSelect(session);
                    }
                    AuthResult::Guest(Err(e)) => {
                        self.loading = false;
                        self.error_message = Some(e.to_string());
                    }
                    AuthResult::WalletChallenge(Ok(challenge)) => {
                        self.wallet_nonce = Some(challenge.nonce);
                        self.wallet_sign_id = Some(start_wallet_sign(&challenge.message));
                    }
                    AuthResult::WalletChallenge(Err(e)) => {
                        self.loading = false;
                        self.error_message = Some(e.to_string());
                    }
                    AuthResult::WalletLogin(Ok(session)) => {
                        self.loading = false;
                        return ScreenState::ToCharacterSelect(session);
                    }
                    AuthResult::WalletLogin(Err(e)) => {
                        self.loading = false;
                        self.error_message = Some(e.to_string());
                    }
                    AuthResult::HealthCheck(online) => {
                        self.server_online = online;
                    }
                    _ => {}
                }
            }
            if self.frame_counter - self.last_ping_time > 5.0
                && !self.auth_client.is_busy()
                && self.wallet_sign_id.is_none()
            {
                self.last_ping_time = self.frame_counter;
                self.auth_client.start_health_check();
            }
        }

        if clicked {
            let l = Self::compute_layout(sw, sh);
            let hit = |r: Rect| point_in_rect(mx, my, r.x, r.y, r.w, r.h);

            if hit(l.guest_button) {
                audio.play_sfx("enter");
                self.error_message = None;
                if let Some(state) = self.play_as_guest() {
                    return state;
                }
            } else if hit(l.wallet_button) {
                audio.play_sfx("enter");
                self.error_message = None;
                if let Some(state) = self.connect_wallet() {
                    return state;
                }
            } else if hit(l.discord_icon) {
                open_external_url(DISCORD_URL);
            }
        }

        if is_key_pressed(KeyCode::Enter) {
            if let Some(state) = self.play_as_guest() {
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

        self.starfield.draw(sw, sh, self.stars_alpha);

        let l = Self::compute_layout(sw, sh);
        let hit = |r: Rect| point_in_rect(mx, my, r.x, r.y, r.w, r.h);

        draw_panel_frame(l.panel.x, l.panel.y, l.panel.w, l.panel.h);
        draw_corner_accents(l.panel.x, l.panel.y, l.panel.w, l.panel.h);

        let title = "SOLSTEAD";
        let title_size = 32.0;
        let title_w = self.measure_text_sharp(title, title_size).width;
        self.draw_text_sharp(
            title,
            (l.panel.x + (l.panel.w - title_w) / 2.0).floor(),
            l.title_baseline,
            title_size,
            TEXT_TITLE,
        );

        let subtitle = "Play as guest or connect your wallet";
        let sub_size = 14.0;
        let sub_w = self.measure_text_sharp(subtitle, sub_size).width;
        self.draw_text_sharp(
            subtitle,
            (l.panel.x + (l.panel.w - sub_w) / 2.0).floor(),
            l.subtitle_baseline,
            sub_size,
            TEXT_DIM,
        );

        if let Some(ref error) = self.error_message {
            self.draw_text_sharp(error, l.content_x, l.error_baseline, 14.0, RED);
        }

        self.draw_button(l.guest_button, "Play as Guest", hit(l.guest_button), true);
        #[cfg(target_arch = "wasm32")]
        let wallet_enabled = is_wallet_available();
        #[cfg(not(target_arch = "wasm32"))]
        let wallet_enabled = true;
        self.draw_button(
            l.wallet_button,
            "Connect Wallet",
            hit(l.wallet_button),
            wallet_enabled,
        );

        draw_line(
            l.content_x,
            l.divider_y,
            l.content_x + l.content_w,
            l.divider_y,
            1.0,
            FRAME_OUTER,
        );

        let status_color = if self.server_online {
            Color::from_rgba(80, 200, 80, 255)
        } else {
            Color::from_rgba(200, 60, 60, 255)
        };
        let status_text = if self.server_online {
            "Online"
        } else {
            "Offline"
        };
        draw_circle(l.content_x + 4.0, l.footer_center_y, 4.0, status_color);
        self.draw_text_sharp(
            status_text,
            l.content_x + 14.0,
            l.footer_center_y + 5.0,
            16.0,
            TEXT_NORMAL,
        );

        if let Some(ref tex) = self.discord_icon {
            let r = l.discord_icon;
            if hit(r) {
                draw_rectangle(
                    r.x - 3.0,
                    r.y - 3.0,
                    r.w + 6.0,
                    r.h + 6.0,
                    Color::new(FRAME_ACCENT.r, FRAME_ACCENT.g, FRAME_ACCENT.b, 0.18),
                );
            }
            draw_texture_ex(
                tex,
                r.x,
                r.y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(r.w, r.h)),
                    ..Default::default()
                },
            );
        }

        let version_text = format!("v{}", env!("CARGO_PKG_VERSION"));
        let version_w = self.measure_text_sharp(&version_text, 16.0).width;
        self.draw_text_sharp(
            &version_text,
            (sw - version_w - 10.0).floor(),
            sh - 10.0,
            16.0,
            TEXT_DIM,
        );

        #[cfg(target_arch = "wasm32")]
        if self.loading {
            let loading = "Connecting...";
            let lw = self.measure_text_sharp(loading, 16.0).width;
            self.draw_text_sharp(
                loading,
                (l.panel.x + (l.panel.w - lw) / 2.0).floor(),
                l.wallet_button.y + l.wallet_button.h + 8.0,
                16.0,
                TEXT_GOLD,
            );
        }
    }
}
