// Library crate for Android builds

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
use macroquad::prelude::*;
#[cfg(target_os = "android")]
use std::time::{Duration, Instant};

pub mod util;
pub use util::asset_path;

pub mod mobile_scale;
pub use mobile_scale::MobileScaler;

mod app;
pub mod audio;
pub mod auth;
pub mod config;
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
mod desktop;
pub mod game;
mod gameplay;
pub mod input;
pub mod network;
pub mod render;
pub mod settings;
mod spectator;
pub mod ui;

pub use app::window_conf;

#[cfg(target_os = "android")]
use app::AppState;
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
use audio::AudioManager;
#[cfg(target_arch = "wasm32")]
use auth::AuthSession;
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
use config::{SERVER_URL, WS_URL};
#[cfg(target_arch = "wasm32")]
use game::GameState;
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
use gameplay::run_game_frame;
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
use input::InputHandler;
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
use network::NetworkClient;
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
use render::Renderer;
#[cfg(target_arch = "wasm32")]
use spectator::SpectatorState;
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
use ui::{CharacterCreateScreen, CharacterSelectScreen, LoginScreen, Screen, ScreenState};

// For Android, we need to export quad_main as the entry point
// miniquad's JNI code (in MainActivity.java) spawns a thread that calls quad_main
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn quad_main() {
    macroquad::Window::from_config(window_conf(), async_main());
}

// WASM entry point - miniquad's JS bundle calls the exported `main` function
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn main() {
    macroquad::Window::from_config(window_conf(), async_main());
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub async fn run_desktop() {
    desktop::run().await;
}

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
async fn async_main() {
    // Initialize logging (skip on Android - use logcat instead)
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
    env_logger::init();

    // Set panic hook for native builds
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
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
    #[cfg(target_os = "android")]
    let scaler = MobileScaler::new();

    // Native build with auth flow
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Start menu music
        audio.play_music("assets/audio/menu.ogg").await;

        let mut login_screen = LoginScreen::new(SERVER_URL);
        login_screen.use_renderer_font(renderer.font().clone());
        login_screen.load_font().await;
        let mut app_state = AppState::Login(login_screen);
        let mut last_next_frame_ms: f64 = 0.0;

        loop {
            let frame_start = Instant::now();

            // Begin scaled rendering for mobile
            scaler.begin_frame();

            // Record last frame's next_frame() time into game state
            if let AppState::Playing { game_state, .. } | AppState::GuestMode { game_state, .. } =
                &mut app_state
            {
                game_state
                    .frame_timings
                    .record_next_frame(last_next_frame_ms);
            }

            match &mut app_state {
                AppState::Login(screen) => {
                    let result = screen.update(&audio);
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
                                renderer.weapon_sprites().clone(),
                                renderer.weapon_frame_sizes().clone(),
                            );
                            char_screen.load_font().await;
                            app_state = AppState::CharacterSelect(char_screen);
                        }
                        ScreenState::StartGuestMode => {
                            let game_state = app::new_game_state(&audio, None);
                            let network = NetworkClient::new_guest(WS_URL);
                            let mut input_handler = InputHandler::new();
                            input_handler.load_touch_icons().await;

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

                AppState::CharacterSelect(screen) => {
                    let result = screen.update(&audio);
                    screen.render();

                    match result {
                        ScreenState::StartGame {
                            session,
                            character_id,
                            character_name,
                        } => {
                            let game_state = app::new_game_state(&audio, Some(character_name));

                            let network = NetworkClient::new_authenticated(
                                WS_URL,
                                &session.token,
                                character_id,
                            );
                            let mut input_handler = InputHandler::new();
                            input_handler.load_touch_icons().await;

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
                            app_state = AppState::CharacterCreate(create_screen);
                        }
                        ScreenState::ToLogin => {
                            let mut login_screen = LoginScreen::new(SERVER_URL);
                            login_screen.load_font().await;
                            app_state = AppState::Login(login_screen);
                        }
                        _ => {}
                    }
                }

                AppState::CharacterCreate(screen) => {
                    let result = screen.update(&audio);
                    screen.render();

                    match result {
                        ScreenState::ToCharacterSelect(session) => {
                            let mut char_screen = CharacterSelectScreen::new(session, SERVER_URL);
                            char_screen.use_renderer_assets(
                                renderer.font().clone(),
                                renderer.player_sprites().clone(),
                                renderer.hair_sprites().clone(),
                                renderer.equipment_sprites().clone(),
                                renderer.weapon_sprites().clone(),
                                renderer.weapon_frame_sizes().clone(),
                            );
                            char_screen.load_font().await;
                            app_state = AppState::CharacterSelect(char_screen);
                        }
                        _ => {}
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

                    if game_state.disconnect_requested {
                        audio.play_music("assets/audio/menu.ogg").await;
                        network.disconnect();
                        let mut login_screen = LoginScreen::new(SERVER_URL);
                        login_screen.load_font().await;
                        app_state = AppState::Login(login_screen);
                        continue;
                    }

                    if game_state.reconnection_failed {
                        log::info!("Reconnection failed, returning to login screen");
                        audio.play_music("assets/audio/menu.ogg").await;
                        network.disconnect();
                        let mut login_screen = LoginScreen::new(SERVER_URL);
                        login_screen.load_font().await;
                        app_state = AppState::Login(login_screen);
                        continue;
                    }
                }
            }

            // End scaled rendering for mobile (draws scaled result to screen)
            scaler.end_frame();

            // Apply optional FPS cap
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

            let next_frame_start = Instant::now();
            next_frame().await;
            last_next_frame_ms = next_frame_start.elapsed().as_secs_f64() * 1000.0;
        }
    }

    // WASM build - full auth flow
    #[cfg(target_arch = "wasm32")]
    {
        use crate::auth::AuthResult;

        // Start menu music
        audio.play_music("assets/audio/menu.ogg").await;

        let mut login_screen = LoginScreen::new(SERVER_URL);
        login_screen.use_renderer_font(renderer.font().clone());
        login_screen.load_font().await;

        enum WasmAppState {
            Login(LoginScreen),
            CharacterSelect(CharacterSelectScreen),
            CharacterCreate(CharacterCreateScreen),
            Matchmaking {
                auth_client: crate::auth::AuthClient,
                session: AuthSession,
                character_name: String,
            },
            Playing {
                game_state: GameState,
                network: NetworkClient,
                input_handler: InputHandler,
            },
            GuestMode {
                game_state: GameState,
                network: NetworkClient,
                input_handler: InputHandler,
            },
        }

        // Live world preview streamed behind the login/character screens. Upgraded in
        // place to a full player session when the game starts (matches native).
        let mut spectator: Option<SpectatorState> = Some(SpectatorState::new());
        let mut app_state = WasmAppState::Login(login_screen);

        loop {
            match &mut app_state {
                WasmAppState::Login(screen) => {
                    let dt = get_frame_time();

                    // Update spectator world view behind login screen
                    if let Some(spec) = spectator.as_mut() {
                        spec.update(dt);
                        screen.set_stars_alpha(1.0 - spec.crossfade_alpha);
                    }

                    let result = screen.update(&audio);

                    // Render: world backdrop first (if spectator ready), then login on top
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
                                renderer.weapon_sprites().clone(),
                                renderer.weapon_frame_sizes().clone(),
                            );
                            char_screen.load_font().await;
                            app_state = WasmAppState::CharacterSelect(char_screen);
                        }
                        ScreenState::StartGuestMode => {
                            // Disconnect spectator if active
                            if let Some(mut spec) = spectator.take() {
                                spec.network.disconnect();
                            }
                            let game_state = app::new_game_state(&audio, None);
                            let network = NetworkClient::new_guest(WS_URL);
                            let mut input_handler = InputHandler::new();
                            input_handler.load_touch_icons().await;

                            audio.play_music("assets/audio/start.ogg").await;

                            app_state = WasmAppState::GuestMode {
                                game_state,
                                network,
                                input_handler,
                            };
                        }
                        _ => {}
                    }
                }

                WasmAppState::CharacterSelect(screen) => {
                    let dt = get_frame_time();

                    // Update spectator world view behind character select
                    if let Some(spec) = spectator.as_mut() {
                        spec.update(dt);
                    }

                    let result = screen.update(&audio);
                    screen.load_equipment_if_needed().await;

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
                            // Start matchmaking via auth client
                            let mut auth_client = crate::auth::AuthClient::new(SERVER_URL);
                            auth_client.start_matchmake(&session.token, character_id, "game_room");
                            app_state = WasmAppState::Matchmaking {
                                auth_client,
                                session,
                                character_name,
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
                            app_state = WasmAppState::CharacterCreate(create_screen);
                        }
                        ScreenState::ToLogin => {
                            let mut login_screen = LoginScreen::new(SERVER_URL);
                            login_screen.use_renderer_font(renderer.font().clone());
                            login_screen.load_font().await;
                            app_state = WasmAppState::Login(login_screen);
                        }
                        _ => {}
                    }
                }

                WasmAppState::CharacterCreate(screen) => {
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

                    match result {
                        ScreenState::ToCharacterSelect(session) => {
                            let mut char_screen = CharacterSelectScreen::new(session, SERVER_URL);
                            char_screen.use_renderer_assets(
                                renderer.font().clone(),
                                renderer.player_sprites().clone(),
                                renderer.hair_sprites().clone(),
                                renderer.equipment_sprites().clone(),
                                renderer.weapon_sprites().clone(),
                                renderer.weapon_frame_sizes().clone(),
                            );
                            char_screen.load_font().await;
                            app_state = WasmAppState::CharacterSelect(char_screen);
                        }
                        _ => {}
                    }
                }

                WasmAppState::Matchmaking {
                    auth_client,
                    session,
                    character_name,
                } => {
                    let dt = get_frame_time();

                    // Keep streaming the world so chunks are ready for a seamless upgrade
                    if let Some(spec) = spectator.as_mut() {
                        spec.update(dt);
                    }

                    // Render: live world backdrop if ready, else a solid background
                    let world_shown = if let Some(spec) = spectator.as_mut() {
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
                    if !world_shown {
                        clear_background(Color::from_rgba(25, 25, 35, 255));
                    }

                    // "Connecting..." overlay
                    let (sw, sh) = (screen_width(), screen_height());
                    let dot_count = ((macroquad::time::get_time() * 3.0) as usize % 4) as usize;
                    let dots = &"..."[..dot_count];
                    let conn_text = format!("Connecting{}", dots);
                    let font_size = 32.0;
                    let dims = renderer.measure_text_sharp(&conn_text, font_size);
                    let tx = ((sw - dims.width) / 2.0).floor();
                    let ty = (sh / 2.0).floor();
                    renderer.draw_text_sharp(&conn_text, tx, ty, font_size, WHITE);

                    if let Some(result) = auth_client.poll() {
                        match result {
                            AuthResult::Matchmake(Ok((room_id, session_token))) => {
                                // Store matchmaking results in localStorage so the network
                                // client can reconnect to the correct room if dropped.
                                {
                                    let storage = &mut quad_storage::STORAGE.lock().unwrap();
                                    storage.set("roomId", &room_id);
                                    storage.set("sessionToken", &session_token);
                                }

                                let mut input_handler = InputHandler::new();
                                input_handler.load_touch_icons().await;

                                if let Some(mut spec) = spectator.take() {
                                    // Upgrade the existing spectator socket in place, reusing
                                    // the chunks it has already streamed for a smooth transition.
                                    spec.network.send_spectator_upgrade(&session_token);

                                    let (cam_x, cam_y) = spec.camera.position();
                                    let mut game_state = spec.game_state;
                                    game_state.camera.transition_from = Some((cam_x, cam_y));
                                    game_state.camera.transition_progress = 0.0;
                                    game_state.spectator_mode = false;
                                    app::configure_game_state(
                                        &mut game_state,
                                        &audio,
                                        Some(character_name.clone()),
                                    );

                                    audio.play_music("assets/audio/start.ogg").await;

                                    app_state = WasmAppState::Playing {
                                        game_state,
                                        network: spec.network,
                                        input_handler,
                                    };
                                } else {
                                    // Fallback: fresh connection (no spectator available).
                                    // new_authenticated reads roomId/sessionToken from localStorage.
                                    let game_state =
                                        app::new_game_state(&audio, Some(character_name.clone()));
                                    let network =
                                        NetworkClient::new_authenticated(WS_URL, &session.token, 0);

                                    audio.play_music("assets/audio/start.ogg").await;

                                    app_state = WasmAppState::Playing {
                                        game_state,
                                        network,
                                        input_handler,
                                    };
                                }
                            }
                            AuthResult::Matchmake(Err(e)) => {
                                log::error!("Matchmaking failed: {}", e);
                                // Go back to character select with error message
                                let mut char_screen =
                                    CharacterSelectScreen::new(session.clone(), SERVER_URL);
                                char_screen.use_renderer_assets(
                                    renderer.font().clone(),
                                    renderer.player_sprites().clone(),
                                    renderer.hair_sprites().clone(),
                                    renderer.equipment_sprites().clone(),
                                    renderer.weapon_sprites().clone(),
                                    renderer.weapon_frame_sizes().clone(),
                                );
                                char_screen.set_error(format!("{}", e));
                                char_screen.load_font().await;
                                app_state = WasmAppState::CharacterSelect(char_screen);
                            }
                            _ => {}
                        }
                    }
                }

                WasmAppState::Playing {
                    game_state,
                    network,
                    input_handler,
                }
                | WasmAppState::GuestMode {
                    game_state,
                    network,
                    input_handler,
                } => {
                    run_game_frame(game_state, network, input_handler, &renderer, &mut audio);

                    if game_state.disconnect_requested || game_state.reconnection_failed {
                        audio.play_music("assets/audio/menu.ogg").await;
                        network.disconnect();
                        let mut login_screen = LoginScreen::new(SERVER_URL);
                        login_screen.use_renderer_font(renderer.font().clone());
                        login_screen.load_font().await;
                        spectator = Some(SpectatorState::new());
                        app_state = WasmAppState::Login(login_screen);
                    }
                }
            }

            next_frame().await;
        }
    }
}
