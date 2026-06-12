use macroquad::prelude::*;
use std::time::{Duration, Instant};

use crate::audio::AudioManager;
use crate::auth::AuthSession;
use crate::config::{SERVER_URL, WS_URL};
use crate::game::GameState;
use crate::gameplay::run_game_frame;
use crate::input::InputHandler;
use crate::network::NetworkClient;
use crate::render::Renderer;
use crate::ui::{CharacterCreateScreen, CharacterSelectScreen, LoginScreen, Screen, ScreenState};
use crate::{game, network};

/// Spectator state for login/character select screens — streams the world behind the UI.
#[cfg(not(target_arch = "wasm32"))]
struct SpectatorState {
    game_state: GameState,
    network: NetworkClient,
    camera: game::SpectatorCamera,
    crossfade_alpha: f32, // 0.0 = stars fully visible, 1.0 = world fully visible
    world_ready: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl SpectatorState {
    fn new() -> Self {
        let mut game_state = GameState::new();
        game_state.spectator_mode = true;
        let network = NetworkClient::new_spectator(WS_URL);
        Self {
            game_state,
            network,
            camera: game::SpectatorCamera::new(),
            crossfade_alpha: 0.0,
            world_ready: false,
        }
    }

    fn update(&mut self, dt: f32) {
        // Poll network messages into game state
        self.network.poll(&mut self.game_state);

        // Update spectator camera
        let (cx, cy) = self.camera.update(dt);
        self.game_state.camera.x = cx;
        self.game_state.camera.y = cy;
        self.game_state.camera.zoom = 1.0;
        self.game_state.camera.initialized = true;

        // Request chunks around camera position (spectator has no local player)
        let chunks_to_request = self.game_state.chunk_manager.update_player_position(cx, cy);
        for coord in chunks_to_request {
            self.network
                .send(&network::messages::ClientMessage::RequestChunk {
                    chunk_x: coord.x,
                    chunk_y: coord.y,
                });
        }
        self.game_state.chunk_manager.unload_distant_chunks();

        // Check world readiness and drive crossfade
        if !self.world_ready && self.game_state.is_world_ready() {
            self.world_ready = true;
        }

        if self.world_ready {
            // Fade in over ~1.5 seconds
            self.crossfade_alpha = (self.crossfade_alpha + dt / 1.5).min(1.0);
        }
    }
}

/// Application state for native builds
#[cfg(not(target_arch = "wasm32"))]
enum AppState {
    Login(LoginScreen, Option<SpectatorState>),
    CharacterSelect(CharacterSelectScreen, Option<SpectatorState>),
    CharacterCreate(CharacterCreateScreen, Option<SpectatorState>),
    Playing {
        game_state: GameState,
        network: NetworkClient,
        input_handler: InputHandler,
        _session: AuthSession,
    },
    GuestMode {
        game_state: GameState,
        network: NetworkClient,
        input_handler: InputHandler,
    },
}

pub(crate) async fn run() {
    // Initialize logging
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();

    // Set panic hook for native builds to capture crash info
    #[cfg(not(target_arch = "wasm32"))]
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("PANIC: {}", panic_info);
        if let Some(location) = panic_info.location() {
            eprintln!(
                "  at {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
        }
    }));

    // On WASM, macroquad/miniquad handles panic logging to console

    // Create audio manager first (just initializes, doesn't load yet)
    let mut audio = AudioManager::new_without_preload();
    // Renderer shows loading screen and loads all assets including audio
    let renderer = Renderer::new(&mut audio).await;

    // Native build with auth flow
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Start menu music
        audio.play_music("assets/audio/menu.ogg").await;

        let mut login_screen = LoginScreen::new(SERVER_URL);
        login_screen.use_renderer_font(renderer.font().clone());
        login_screen.load_font().await;
        let spectator = SpectatorState::new();
        let mut app_state = AppState::Login(login_screen, Some(spectator));
        let mut last_next_frame_ms: f64 = 0.0;

        loop {
            let frame_start = Instant::now();

            // Record last frame's next_frame() time into game state
            if let AppState::Playing { game_state, .. } | AppState::GuestMode { game_state, .. } =
                &mut app_state
            {
                game_state
                    .frame_timings
                    .record_next_frame(last_next_frame_ms);
            }

            match &mut app_state {
                AppState::Login(screen, spectator) => {
                    let dt = get_frame_time();

                    // Update spectator world view behind login screen
                    if let Some(spec) = spectator.as_mut() {
                        spec.update(dt);
                        screen.set_stars_alpha(1.0 - spec.crossfade_alpha);
                    }

                    let result = screen.update(&audio);

                    // Render: world backdrop first (if spectator ready), then login screen on top
                    if let Some(spec) = spectator.as_mut() {
                        if spec.world_ready {
                            clear_background(Color::from_rgba(30, 30, 40, 255));
                            renderer.render(&spec.game_state);
                        }
                    }
                    screen.render();

                    match result {
                        ScreenState::ToCharacterSelect(session) => {
                            audio.play_sfx("login_success");
                            let mut char_screen = CharacterSelectScreen::new(session, SERVER_URL);
                            char_screen.use_renderer_assets(
                                renderer.font().clone(),
                                renderer.player_sprites().clone(),
                                renderer.hair_sprites().clone(),
                                renderer.equipment_sprites().clone(),
                            );
                            char_screen.load_font().await;
                            app_state = AppState::CharacterSelect(char_screen, spectator.take());
                        }
                        ScreenState::StartGuestMode => {
                            // Disconnect spectator if active
                            if let Some(mut spec) = spectator.take() {
                                spec.network.disconnect();
                            }
                            let game_state = crate::app::new_game_state(&audio, None);
                            let network = NetworkClient::new_guest(WS_URL);
                            let mut input_handler = InputHandler::new();
                            input_handler.load_touch_icons().await;

                            // Start background music
                            audio.play_music("assets/audio/start.ogg").await;

                            app_state = AppState::GuestMode {
                                game_state,
                                network,
                                input_handler,
                            };
                        }
                        _ => {}
                    }
                }

                AppState::CharacterSelect(screen, spectator) => {
                    let dt = get_frame_time();

                    // Update spectator world view behind character select
                    if let Some(spec) = spectator.as_mut() {
                        spec.update(dt);
                    }

                    let result = screen.update(&audio);

                    // Render: world backdrop first (if spectator ready), then screen on top
                    let has_backdrop = if let Some(spec) = spectator.as_mut() {
                        if spec.world_ready {
                            clear_background(Color::from_rgba(30, 30, 40, 255));
                            renderer.render(&spec.game_state);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    screen.has_spectator_backdrop = has_backdrop;
                    screen.render();

                    match result {
                        ScreenState::StartGame {
                            session,
                            character_id,
                            character_name,
                        } => {
                            // Try to upgrade spectator connection, fall back to fresh connection
                            if let Some(mut spec) = spectator.take() {
                                // HTTP matchmake to get session_token for upgrade
                                let http_url = WS_URL
                                    .replace("ws://", "http://")
                                    .replace("wss://", "https://");
                                let matchmake_url =
                                    format!("{}/matchmake/joinOrCreate/game_room", http_url);
                                let body = serde_json::json!({
                                    "characterId": character_id,
                                });
                                let result = ureq::post(&matchmake_url)
                                    .set("Authorization", &format!("Bearer {}", session.token))
                                    .set("Content-Type", "application/json")
                                    .send_json(&body);

                                match result {
                                    Ok(response) => {
                                        if let Ok(data) = response.into_json::<serde_json::Value>()
                                        {
                                            if let Some(token) = data["sessionToken"].as_str() {
                                                // Send upgrade over existing spectator WS
                                                spec.network.send_spectator_upgrade(token);

                                                // Capture spectator camera position for smooth transition
                                                let (cam_x, cam_y) = spec.camera.position();

                                                // Reuse spectator's game state (chunks already loaded!)
                                                let mut game_state = spec.game_state;
                                                game_state.camera.transition_from =
                                                    Some((cam_x, cam_y));
                                                game_state.camera.transition_progress = 0.0;
                                                game_state.spectator_mode = false;
                                                crate::app::configure_game_state(
                                                    &mut game_state,
                                                    &audio,
                                                    Some(character_name),
                                                );

                                                let mut input_handler = InputHandler::new();
                                                input_handler.load_touch_icons().await;

                                                // Start background music
                                                audio.play_music("assets/audio/start.ogg").await;

                                                app_state = AppState::Playing {
                                                    game_state,
                                                    network: spec.network,
                                                    input_handler,
                                                    _session: session,
                                                };
                                                continue;
                                            }
                                        }
                                        // JSON parse failed — fall through to fresh connection
                                        log::error!("Matchmake for upgrade: bad response format");
                                        spec.network.disconnect();
                                    }
                                    Err(e) => {
                                        log::error!("Matchmake for upgrade failed: {}", e);
                                        spec.network.disconnect();
                                    }
                                }
                            }

                            // Fallback: fresh connection (no spectator or upgrade failed)
                            let game_state =
                                crate::app::new_game_state(&audio, Some(character_name));

                            let network = NetworkClient::new_authenticated(
                                WS_URL,
                                &session.token,
                                character_id,
                            );
                            let mut input_handler = InputHandler::new();
                            input_handler.load_touch_icons().await;

                            // Start background music
                            audio.play_music("assets/audio/start.ogg").await;

                            app_state = AppState::Playing {
                                game_state,
                                network,
                                input_handler,
                                _session: session,
                            };
                        }
                        ScreenState::ToCharacterCreate(session) => {
                            let mut create_screen = CharacterCreateScreen::new(session, SERVER_URL);
                            create_screen.use_renderer_assets(
                                renderer.font().clone(),
                                renderer.player_sprites().clone(),
                                renderer.hair_sprites().clone(),
                            );
                            create_screen.load_font().await;
                            app_state = AppState::CharacterCreate(create_screen, spectator.take());
                        }
                        ScreenState::ToLogin => {
                            let mut login_screen = LoginScreen::new(SERVER_URL);
                            login_screen.use_renderer_font(renderer.font().clone());
                            login_screen.load_font().await;
                            app_state = AppState::Login(login_screen, spectator.take());
                        }
                        _ => {}
                    }
                }

                AppState::CharacterCreate(screen, spectator) => {
                    let dt = get_frame_time();

                    // Update spectator world view behind character create
                    if let Some(spec) = spectator.as_mut() {
                        spec.update(dt);
                    }

                    let result = screen.update(&audio);

                    // Render: world backdrop first (if spectator ready), then screen on top
                    if let Some(spec) = spectator.as_mut() {
                        if spec.world_ready {
                            clear_background(Color::from_rgba(30, 30, 40, 255));
                            renderer.render(&spec.game_state);
                        }
                    }
                    screen.render();

                    if let ScreenState::ToCharacterSelect(session) = result {
                        let mut char_screen = CharacterSelectScreen::new(session, SERVER_URL);
                        char_screen.use_renderer_assets(
                            renderer.font().clone(),
                            renderer.player_sprites().clone(),
                            renderer.hair_sprites().clone(),
                            renderer.equipment_sprites().clone(),
                        );
                        char_screen.load_font().await;
                        app_state = AppState::CharacterSelect(char_screen, spectator.take());
                    }
                }

                AppState::Playing {
                    game_state,
                    network,
                    input_handler,
                    ..
                }
                | AppState::GuestMode {
                    game_state,
                    network,
                    input_handler,
                } => {
                    run_game_frame(game_state, network, input_handler, &renderer, &mut audio);

                    // Check for disconnect request
                    if game_state.disconnect_requested {
                        // Switch to menu music and disconnect from server
                        audio.play_music("assets/audio/menu.ogg").await;
                        network.disconnect();
                        let mut login_screen = LoginScreen::new(SERVER_URL);
                        login_screen.use_renderer_font(renderer.font().clone());
                        login_screen.load_font().await;
                        let spectator = SpectatorState::new();
                        app_state = AppState::Login(login_screen, Some(spectator));
                        continue;
                    }

                    // Check for reconnection failure (server disconnected and retries exhausted)
                    if game_state.reconnection_failed {
                        log::info!("Reconnection failed, returning to login screen");
                        audio.play_music("assets/audio/menu.ogg").await;
                        network.disconnect();
                        let mut login_screen = LoginScreen::new(SERVER_URL);
                        login_screen.use_renderer_font(renderer.font().clone());
                        login_screen.load_font().await;
                        let spectator = SpectatorState::new();
                        app_state = AppState::Login(login_screen, Some(spectator));
                        continue;
                    }
                }
            }

            // Apply optional FPS cap (only when in game)
            let fps_cap = match &app_state {
                AppState::Playing { game_state, .. } | AppState::GuestMode { game_state, .. } => {
                    game_state.frame_timings.fps_cap
                }
                _ => None,
            };

            if let Some(cap) = fps_cap {
                let target_frame_time = Duration::from_secs_f64(1.0 / cap as f64);
                // Subtract previous next_frame() time from our sleep budget so total
                // frame time (work + sleep + next_frame) hits the target. Without this,
                // next_frame() overhead is additive and fast-rendering frames (e.g.
                // small interior maps) sleep too long, causing FPS to drop well below cap.
                let nf_compensation = Duration::from_secs_f64(last_next_frame_ms / 1000.0);
                let effective_target = target_frame_time.saturating_sub(nf_compensation);
                let elapsed = frame_start.elapsed();
                if elapsed < effective_target {
                    // Two-phase pacing: coarse sleep, then short spin to reduce oversleep jitter.
                    // This keeps frame times more consistent than a single sleep() call.
                    let remaining = effective_target - elapsed;
                    if remaining > Duration::from_millis(2) {
                        std::thread::sleep(remaining - Duration::from_millis(1));
                    }
                    while frame_start.elapsed() < effective_target {
                        std::hint::spin_loop();
                    }
                }
            }

            // Measure time spent in next_frame() to diagnose variance
            let next_frame_start = Instant::now();
            next_frame().await;
            last_next_frame_ms = next_frame_start.elapsed().as_secs_f64() * 1000.0;
        }
    }

    // WASM build - networked game mode
    // JavaScript handles matchmaking before loading WASM,
    // storing roomId and sessionToken in localStorage
    #[cfg(target_arch = "wasm32")]
    {
        let mut game_state = GameState::new();
        let mut network = NetworkClient::new_guest(WS_URL);
        let mut input_handler = InputHandler::new();
        input_handler.load_touch_icons().await;

        loop {
            run_game_frame(
                &mut game_state,
                &mut network,
                &mut input_handler,
                &renderer,
                &mut audio,
            );
            next_frame().await;
        }
    }
}
