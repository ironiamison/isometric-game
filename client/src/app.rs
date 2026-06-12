use macroquad::prelude::*;

use crate::game::tutorial::TutorialManager;
use crate::game::GameState;
#[cfg(target_os = "android")]
use crate::input::InputHandler;
#[cfg(target_os = "android")]
use crate::network::NetworkClient;

#[cfg(target_os = "android")]
use crate::auth::AuthSession;
#[cfg(target_os = "android")]
use crate::ui::{CharacterCreateScreen, CharacterSelectScreen, LoginScreen};

#[cfg(not(target_os = "android"))]
pub fn maybe_show_control_scheme_dialogue(game_state: &mut GameState) {
    if !crate::settings::load_control_scheme_chosen() {
        game_state.ui_state.active_dialogue = Some(crate::game::state::ActiveDialogue {
            quest_id: "__control_scheme__".to_string(),
            npc_id: String::new(),
            speaker: "Control Scheme".to_string(),
            text: "Welcome! Choose your control scheme:\n\nModern: WASD to move, Space to attack, Ctrl to jump, Enter to chat\n\nClassic: Arrow keys to move, Ctrl to attack, always-on chat input".to_string(),
            choices: vec![
                crate::game::state::DialogueChoice {
                    id: "modern".to_string(),
                    text: "Modern (WASD + Space Attack + Enter)".to_string(),
                },
                crate::game::state::DialogueChoice {
                    id: "classic".to_string(),
                    text: "Classic (Arrows + Ctrl Attack + Always-on Chat)".to_string(),
                },
            ],
            show_time: get_time(),
        });
    }
}

pub fn maybe_start_tutorial(game_state: &mut GameState) {
    if !game_state.tutorial_pending || game_state.ui_state.active_dialogue.is_some() {
        return;
    }
    game_state.tutorial_pending = false;

    let mut tutorial = TutorialManager::new(game_state.ui_state.classic_controls);
    if let Some(dialogue) = tutorial.phase_dialogue() {
        game_state.ui_state.active_dialogue = Some(dialogue);
    }
    tutorial.hint_visible = false;
    game_state.tutorial = Some(tutorial);
}

pub fn update_tutorial(game_state: &mut GameState) {
    let Some(tutorial) = &mut game_state.tutorial else {
        return;
    };
    if tutorial.is_done() {
        return;
    }

    if tutorial.hint_visible
        && game_state.ui_state.active_dialogue.is_none()
        && is_key_pressed(KeyCode::Escape)
    {
        tutorial.skip();
        crate::settings::save_tutorial_completed();
        return;
    }

    if tutorial.pending_dialogue && game_state.ui_state.active_dialogue.is_none() {
        tutorial.pending_dialogue = false;
        tutorial.hint_visible = true;
        if let Some(dialogue) = tutorial.phase_dialogue() {
            game_state.ui_state.active_dialogue = Some(dialogue);
        }
    }

    if game_state.ui_state.inventory_open {
        tutorial.on_inventory_opened();
    }
    if game_state.ui_state.skills_open {
        tutorial.on_skills_opened();
    }

    if tutorial.phase == crate::game::tutorial::TutorialPhase::Handoff
        && game_state.ui_state.active_dialogue.is_none()
        && !tutorial.pending_dialogue
    {
        tutorial.advance();
        tutorial.hint_visible = false;
        crate::settings::save_tutorial_completed();
    }
}

pub fn window_conf() -> Conf {
    Conf {
        window_title: "New Aeven".to_string(),
        window_width: 1280,
        window_height: 720,
        fullscreen: false,
        icon: load_icon(),
        platform: miniquad::conf::Platform {
            swap_interval: Some(0),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn load_icon() -> Option<miniquad::conf::Icon> {
    [
        "assets/app-icon.png",
        "assets/ui/app-icon.png",
        "assets/logo.png",
        "assets/ui/logo.png",
        "assets/favicon.png",
    ]
    .into_iter()
    .find_map(|path| {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| image::load_from_memory(&bytes).ok())
            .map(icon_from_image)
    })
}

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
fn load_icon() -> Option<miniquad::conf::Icon> {
    None
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn icon_from_image(image: image::DynamicImage) -> miniquad::conf::Icon {
    let image = image.to_rgba8();
    let small = image::imageops::resize(&image, 16, 16, image::imageops::FilterType::Lanczos3);
    let medium = image::imageops::resize(&image, 32, 32, image::imageops::FilterType::Lanczos3);
    let big = image::imageops::resize(&image, 64, 64, image::imageops::FilterType::Lanczos3);

    miniquad::conf::Icon {
        small: small.into_raw().try_into().unwrap_or([0; 16 * 16 * 4]),
        medium: medium.into_raw().try_into().unwrap_or([0; 32 * 32 * 4]),
        big: big.into_raw().try_into().unwrap_or([0; 64 * 64 * 4]),
    }
}

#[cfg(target_os = "android")]
pub enum AppState {
    Login(LoginScreen),
    CharacterSelect(CharacterSelectScreen),
    CharacterCreate(CharacterCreateScreen),
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
