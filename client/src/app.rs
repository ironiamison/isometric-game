// Shared application code between desktop and Android builds

use macroquad::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

use crate::audio::AudioManager;
use crate::game::GameState;
use crate::input::{InputCommand, InputHandler};
use crate::network::NetworkClient;
use crate::render::animation::AnimationState;
use crate::render::Renderer;

#[cfg(not(target_arch = "wasm32"))]
use crate::auth::AuthSession;
#[cfg(not(target_arch = "wasm32"))]
use crate::ui::{CharacterCreateScreen, CharacterSelectScreen, LoginScreen, Screen, ScreenState};

use crate::game::tutorial::TutorialManager;

// pub const SERVER_URL: &str = "https://aeven.xyz";
// pub const WS_URL: &str = "wss://aeven.xyz";

pub const SERVER_URL: &str = "http://localhost:2567";
pub const WS_URL: &str = "ws://localhost:2567";

// Development mode - enables guest login
// Set to false for production builds
pub const DEV_MODE: bool = false;

/// Show the control scheme choice dialogue if the player hasn't chosen yet.
/// Skipped on Android (no keyboard).
#[cfg(not(target_os = "android"))]
pub fn maybe_show_control_scheme_dialogue(game_state: &mut GameState) {
    if !crate::settings::load_control_scheme_chosen() {
        game_state.ui_state.active_dialogue = Some(crate::game::state::ActiveDialogue {
            quest_id: "__control_scheme__".to_string(),
            npc_id: String::new(),
            speaker: "Control Scheme".to_string(),
            text: "Welcome! Choose your control scheme:\n\nModern: WASD to move, Space to attack, Enter to chat\n\nClassic: Arrow keys to move, Ctrl to attack, always-on chat input".to_string(),
            choices: vec![
                crate::game::state::DialogueChoice {
                    id: "modern".to_string(),
                    text: "Modern (WASD + Space + Enter)".to_string(),
                },
                crate::game::state::DialogueChoice {
                    id: "classic".to_string(),
                    text: "Classic (Arrows + Ctrl + Always-on Chat)".to_string(),
                },
            ],
            show_time: get_time(),
        });
    }
}

/// Start the tutorial if the player is new and hasn't completed it yet.
/// Called each frame — will fire only when tutorial_pending is true and no dialogue is open.
pub fn maybe_start_tutorial(game_state: &mut GameState) {
    if !game_state.tutorial_pending {
        return;
    }
    // Wait for any active dialogue (e.g. control scheme) to close first
    if game_state.ui_state.active_dialogue.is_some() {
        log::warn!("TUTORIAL: pending but dialogue is open, waiting...");
        return;
    }
    log::warn!("TUTORIAL: starting tutorial now!");
    game_state.tutorial_pending = false;

    // Create the tutorial manager
    let mut tutorial = TutorialManager::new(game_state.ui_state.classic_controls);

    // Show Old Thomas's initial greeting
    if let Some(dialogue) = tutorial.phase_dialogue() {
        game_state.ui_state.active_dialogue = Some(dialogue);
    }
    tutorial.hint_visible = false; // No hint during the greeting
    game_state.tutorial = Some(tutorial);
}

/// Process tutorial phase completion checks. Called each frame during gameplay.
pub fn update_tutorial(game_state: &mut GameState) {
    let Some(tutorial) = &mut game_state.tutorial else {
        return;
    };
    if tutorial.is_done() {
        return;
    }

    // Escape key skips the tutorial while the hint bar is visible (not during dialogue)
    if tutorial.hint_visible
        && game_state.ui_state.active_dialogue.is_none()
        && is_key_pressed(KeyCode::Escape)
    {
        tutorial.skip();
        crate::settings::save_tutorial_completed();
        return;
    }

    // If there's a pending dialogue to show and no active dialogue, show it
    if tutorial.pending_dialogue && game_state.ui_state.active_dialogue.is_none() {
        tutorial.pending_dialogue = false;
        tutorial.hint_visible = true;
        if let Some(dialogue) = tutorial.phase_dialogue() {
            game_state.ui_state.active_dialogue = Some(dialogue);
        }
    }

    // Check if inventory was opened (phase 4)
    if game_state.ui_state.inventory_open {
        tutorial.on_inventory_opened();
    }

    // Check if skills panel was opened (phase 5)
    if game_state.ui_state.skills_open {
        tutorial.on_skills_opened();
    }

    // Auto-complete handoff phase after the dialogue is dismissed
    if tutorial.phase == crate::game::tutorial::TutorialPhase::Handoff
        && game_state.ui_state.active_dialogue.is_none()
        && !tutorial.pending_dialogue
    {
        tutorial.advance(); // -> Done
        tutorial.hint_visible = false;
        crate::settings::save_tutorial_completed();
    }
}

pub fn window_conf() -> Conf {
    let icon = load_icon().map(|icon| Some(icon)).unwrap_or(None);
    Conf {
        window_title: "New Aeven".to_string(),
        window_width: 1280,
        window_height: 720,
        fullscreen: false,
        icon,
        platform: miniquad::conf::Platform {
            swap_interval: Some(0),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn load_icon() -> Option<miniquad::conf::Icon> {
    let candidates = [
        "assets/app-icon.png",
        "assets/ui/app-icon.png",
        "assets/logo.png",
        "assets/ui/logo.png",
        "assets/favicon.png",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(image) = image::load_from_memory(&bytes) {
                return Some(icon_from_image(image));
            }
        }
    }
    None
}

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
fn load_icon() -> Option<miniquad::conf::Icon> {
    None
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn icon_from_image(image: image::DynamicImage) -> miniquad::conf::Icon {
    let img = image.to_rgba8();
    let icon16 = image::imageops::resize(&img, 16, 16, image::imageops::FilterType::Lanczos3);
    let icon32 = image::imageops::resize(&img, 32, 32, image::imageops::FilterType::Lanczos3);
    let icon64 = image::imageops::resize(&img, 64, 64, image::imageops::FilterType::Lanczos3);

    miniquad::conf::Icon {
        small: icon16.into_raw().try_into().unwrap_or([0; 16 * 16 * 4]),
        medium: icon32.into_raw().try_into().unwrap_or([0; 32 * 32 * 4]),
        big: icon64.into_raw().try_into().unwrap_or([0; 64 * 64 * 4]),
    }
}

/// Application state for native builds
#[cfg(not(target_arch = "wasm32"))]
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

/// Run a single frame of gameplay
pub fn run_game_frame(
    game_state: &mut GameState,
    network: &mut NetworkClient,
    input_handler: &mut InputHandler,
    renderer: &Renderer,
    audio: &mut AudioManager,
) {
    let frame_start = get_time();
    let delta = get_frame_time();

    // Toggle debug mode with F3
    if is_key_pressed(KeyCode::F3) {
        game_state.debug_mode = !game_state.debug_mode;
    }

    // Cycle FPS cap with F4: Uncapped -> 60 -> 144 -> 240 -> 300 -> Uncapped
    if is_key_pressed(KeyCode::F4) {
        game_state.frame_timings.fps_cap = match game_state.frame_timings.fps_cap {
            None => Some(60),
            Some(60) => Some(144),
            Some(144) => Some(240),
            Some(240) => Some(300),
            _ => None,
        };
        log::info!("FPS cap: {:?}", game_state.frame_timings.fps_cap);
    }

    // Cycle delta smoothing with F7: 0 -> 0.5 -> 0.8 -> 0.9 -> 0
    if is_key_pressed(KeyCode::F7) {
        game_state.frame_timings.delta_smoothing = match game_state.frame_timings.delta_smoothing {
            x if x < 0.1 => 0.5,
            x if x < 0.6 => 0.8,
            x if x < 0.85 => 0.9,
            _ => 0.0,
        };
        log::info!(
            "Delta smoothing: {}",
            game_state.frame_timings.delta_smoothing
        );
    }

    // Debug controls for appearance cycling (only in debug mode)
    if game_state.debug_mode {
        if let Some(local_id) = &game_state.local_player_id.clone() {
            if let Some(player) = game_state.players.get_mut(local_id) {
                // F5 to cycle gender
                if is_key_pressed(KeyCode::F5) {
                    player.gender = match player.gender.as_str() {
                        "male" => "female".to_string(),
                        _ => "male".to_string(),
                    };
                    log::info!("Debug: Changed gender to {}", player.gender);
                }

                // F6 to cycle skin
                if is_key_pressed(KeyCode::F6) {
                    let skins = ["tan", "pale", "brown", "purple", "orc", "ghost", "skeleton"];
                    let current_idx = skins.iter().position(|&s| s == player.skin).unwrap_or(0);
                    let next_idx = (current_idx + 1) % skins.len();
                    player.skin = skins[next_idx].to_string();
                    log::info!("Debug: Changed skin to {}", player.skin);
                }
            }
        }
    }

    // 1. Poll network messages
    let network_start = get_time();
    network.poll(game_state);
    let network_ms = (get_time() - network_start) * 1000.0;

    // 1.5. Play any pending sound effects queued by message handlers
    for sfx_name in game_state.pending_sfx.drain(..) {
        audio.play_sfx(&sfx_name);
    }
    for attack_type in game_state.pending_attack_sounds.drain(..) {
        audio.play_attack_sound(attack_type);
    }

    // 2. Render and get UI layout for hit detection
    clear_background(Color::from_rgba(30, 30, 40, 255));
    let (layout, render_timings) = renderer.render(game_state);

    // 3. Handle input with UI layout and send commands
    let commands = input_handler.process(game_state, &layout, audio);
    for cmd in &commands {
        use crate::network::messages::ClientMessage;
        let msg = match cmd {
            InputCommand::Move { dx, dy } => {
                // Notify tutorial of player movement
                if (*dx != 0.0 || *dy != 0.0) {
                    if let Some(tutorial) = &mut game_state.tutorial {
                        tutorial.on_player_moved();
                    }
                }
                let seq = game_state.next_move_sequence(*dx, *dy);
                ClientMessage::Move {
                    dx: *dx,
                    dy: *dy,
                    seq,
                }
            }
            InputCommand::Face { direction } => {
                // Skip direction update if sitting or attacking
                if game_state.is_sitting {
                    continue;
                }
                if let Some(local_id) = &game_state.local_player_id {
                    if let Some(player) = game_state.players.get(local_id) {
                        let is_attacking = matches!(
                            player.animation.state,
                            AnimationState::Attacking
                                | AnimationState::Casting
                                | AnimationState::ShootingBow
                        );
                        if is_attacking {
                            continue;
                        }
                    }
                }
                // Optimistic local face update for responsiveness.
                // Server state sync remains authoritative.
                if let Some(local_id) = &game_state.local_player_id {
                    if let Some(player) = game_state.players.get_mut(local_id) {
                        let new_dir = crate::game::Direction::from_u8(*direction);
                        player.direction = new_dir;
                        player.animation.direction = new_dir;
                    }
                }
                ClientMessage::Face {
                    direction: *direction,
                }
            }
            InputCommand::Attack => {
                // Notify tutorial of combat action
                if let Some(tutorial) = &mut game_state.tutorial {
                    tutorial.on_combat_action();
                }
                // Trigger attack animation and sound on local player
                if let Some(local_id) = &game_state.local_player_id {
                    let is_ranged = game_state
                        .players
                        .get(local_id)
                        .and_then(|p| p.equipped_weapon.as_ref())
                        .and_then(|weapon_id| game_state.item_registry.get(weapon_id))
                        .map(|item_def| item_def.weapon_type.as_deref() == Some("ranged"))
                        .unwrap_or(false);

                    if let Some(player) = game_state.players.get_mut(local_id) {
                        if is_ranged {
                            player.play_shoot_bow();
                        } else {
                            player.play_attack();
                        }
                        let sound_type = if is_ranged {
                            crate::game::state::AttackSoundType::Ranged
                        } else if player.equipped_weapon.is_some() {
                            crate::game::state::AttackSoundType::Melee
                        } else {
                            crate::game::state::AttackSoundType::Unarmed
                        };
                        audio.play_attack_sound(sound_type);
                    }
                }
                ClientMessage::Attack
            }
            InputCommand::Target { entity_id } => ClientMessage::Target {
                entity_id: entity_id.clone(),
            },
            InputCommand::ClearTarget => ClientMessage::Target {
                entity_id: String::new(),
            },
            InputCommand::Chat { text, channel } => {
                // Handle /ping command
                if text.trim().eq_ignore_ascii_case("/ping") {
                    let timestamp = get_time();
                    game_state.ping_sent_at = Some(timestamp);
                    ClientMessage::Ping { timestamp }
                } else {
                    ClientMessage::Chat {
                        text: text.clone(),
                        channel: channel.clone(),
                    }
                }
            }
            InputCommand::Pickup { item_id } => ClientMessage::Pickup {
                item_id: item_id.clone(),
            },
            InputCommand::UseItem { slot_index } => ClientMessage::UseItem {
                slot_index: *slot_index as u32,
            },
            InputCommand::Interact { npc_id } => {
                // Notify tutorial of NPC interaction
                if let Some(tutorial) = &mut game_state.tutorial {
                    tutorial.on_dialogue_opened();
                }
                ClientMessage::Interact {
                    npc_id: npc_id.clone(),
                }
            }
            InputCommand::DialogueChoice {
                quest_id,
                choice_id,
            } => {
                audio.play_sfx("enter");
                if quest_id == "__control_scheme__" {
                    let classic = choice_id == "classic";
                    game_state.ui_state.classic_controls = classic;
                    if classic {
                        game_state.ui_state.chat_open = true;
                    }
                    crate::settings::save_classic_controls(classic);
                    crate::settings::save_control_scheme_chosen();
                    game_state.ui_state.active_dialogue = None;
                    continue;
                }
                if quest_id == "__tutorial__" {
                    game_state.ui_state.active_dialogue = None;
                    // Create TutorialManager if it doesn't exist yet
                    // (e.g. player clicked Old Thomas directly instead of auto-start)
                    if game_state.tutorial.is_none() {
                        game_state.tutorial = Some(TutorialManager::new(
                            game_state.ui_state.classic_controls,
                        ));
                    }
                    if let Some(tutorial) = &mut game_state.tutorial {
                        if tutorial.phase == crate::game::tutorial::TutorialPhase::AwaitingAccept {
                            if choice_id == "accept" {
                                tutorial.advance(); // -> Movement
                                tutorial.pending_dialogue = true;
                            } else {
                                tutorial.skip();
                                crate::settings::save_tutorial_completed();
                            }
                        }
                        // For all other phases, the dialogue "ok" just closes it
                        // and the hint bar takes over
                    }
                    continue;
                }
                ClientMessage::DialogueChoice {
                    quest_id: quest_id.clone(),
                    choice_id: choice_id.clone(),
                }
            }
            InputCommand::CloseDialogue => {
                continue;
            }
            InputCommand::Craft { recipe_id } => ClientMessage::StartCraft {
                recipe_id: recipe_id.clone(),
            },
            InputCommand::CancelCraft => ClientMessage::CancelCraft,
            InputCommand::Equip { slot_index } => ClientMessage::Equip {
                slot_index: *slot_index,
            },
            InputCommand::Unequip {
                slot_type,
                target_slot,
            } => ClientMessage::Unequip {
                slot_type: slot_type.clone(),
                target_slot: *target_slot,
            },
            InputCommand::DropItem {
                slot_index,
                quantity,
                target_x,
                target_y,
            } => ClientMessage::DropItem {
                slot_index: *slot_index,
                quantity: *quantity,
                target_x: *target_x,
                target_y: *target_y,
            },
            InputCommand::DropGold { amount } => ClientMessage::DropGold { amount: *amount },
            InputCommand::SwapSlots { from_slot, to_slot } => ClientMessage::SwapSlots {
                from_slot: *from_slot,
                to_slot: *to_slot,
            },
            InputCommand::ShopBuy {
                npc_id,
                item_id,
                quantity,
            } => ClientMessage::ShopBuy {
                npc_id: npc_id.clone(),
                item_id: item_id.clone(),
                quantity: *quantity,
            },
            InputCommand::ShopSell {
                npc_id,
                item_id,
                quantity,
            } => ClientMessage::ShopSell {
                npc_id: npc_id.clone(),
                item_id: item_id.clone(),
                quantity: *quantity,
            },
            // Bank commands
            InputCommand::BankDeposit { item_id, quantity } => ClientMessage::BankDeposit {
                item_id: item_id.clone(),
                quantity: *quantity,
            },
            InputCommand::BankWithdraw { item_id, quantity } => ClientMessage::BankWithdraw {
                item_id: item_id.clone(),
                quantity: *quantity,
            },
            InputCommand::BankDepositGold { amount } => {
                ClientMessage::BankDepositGold { amount: *amount }
            }
            InputCommand::BankWithdrawGold { amount } => {
                ClientMessage::BankWithdrawGold { amount: *amount }
            }
            InputCommand::BankDepositAll => ClientMessage::BankDepositAll,
            InputCommand::BankSwapSlots { slot_a, slot_b } => ClientMessage::BankSwapSlots {
                slot_a: *slot_a,
                slot_b: *slot_b,
            },
            InputCommand::BankSort => ClientMessage::BankSort,
            InputCommand::EnterPortal { portal_id } => ClientMessage::EnterPortal {
                portal_id: portal_id.clone(),
            },
            InputCommand::StartGathering { marker_x, marker_y } => {
                // Play attack animation so it looks like the player is casting/throwing
                if let Some(local_id) = &game_state.local_player_id {
                    if let Some(player) = game_state.players.get_mut(local_id) {
                        player.play_attack();
                    }
                }
                ClientMessage::StartGathering {
                    marker_x: *marker_x,
                    marker_y: *marker_y,
                }
            }
            InputCommand::StopGathering => ClientMessage::StopGathering,
            InputCommand::ChopTree {
                tree_x,
                tree_y,
                tree_gid,
            } => {
                // Play attack animation for chopping (server will also broadcast this)
                if let Some(local_id) = &game_state.local_player_id {
                    if let Some(player) = game_state.players.get_mut(local_id) {
                        player.play_attack();
                    }
                }
                ClientMessage::ChopTree {
                    tree_x: *tree_x,
                    tree_y: *tree_y,
                    tree_gid: *tree_gid,
                }
            }
            InputCommand::MineRock {
                rock_x,
                rock_y,
                rock_gid,
            } => {
                // Play attack animation for mining (server will also broadcast this)
                if let Some(local_id) = &game_state.local_player_id {
                    if let Some(player) = game_state.players.get_mut(local_id) {
                        player.play_attack();
                    }
                }
                ClientMessage::MineRock {
                    rock_x: *rock_x,
                    rock_y: *rock_y,
                    rock_gid: *rock_gid,
                }
            }
            InputCommand::SitChair { tile_x, tile_y } => ClientMessage::SitChair {
                tile_x: *tile_x,
                tile_y: *tile_y,
            },
            InputCommand::StandUp => ClientMessage::StandUp,
            InputCommand::PlantSeed { patch_id, item_id } => ClientMessage::PlantSeed {
                patch_id: patch_id.clone(),
                item_id: item_id.clone(),
            },
            InputCommand::HarvestCrop { patch_id } => ClientMessage::HarvestCrop {
                patch_id: patch_id.clone(),
            },
            // Friend system commands
            InputCommand::SendFriendRequest { target_name } => ClientMessage::SendFriendRequest {
                target_name: target_name.clone(),
            },
            InputCommand::AcceptFriendRequest { requester_id } => {
                ClientMessage::AcceptFriendRequest {
                    requester_id: *requester_id,
                }
            }
            InputCommand::DeclineFriendRequest { requester_id } => {
                ClientMessage::DeclineFriendRequest {
                    requester_id: *requester_id,
                }
            }
            InputCommand::RemoveFriend { friend_id } => ClientMessage::RemoveFriend {
                friend_id: *friend_id,
            },
            InputCommand::GetOnlinePlayers => ClientMessage::GetOnlinePlayers,
            // Prayer commands
            InputCommand::TogglePrayer { prayer_id } => ClientMessage::TogglePrayer {
                prayer_id: prayer_id.clone(),
            },
            InputCommand::BuryBones { slot } => ClientMessage::BuryBones {
                slot: *slot as usize,
            },
            // Altar commands
            InputCommand::OfferBones { slot, altar_id } => ClientMessage::OfferBones {
                slot: *slot as usize,
                altar_id: altar_id.clone(),
            },
            InputCommand::OfferAllBones { item_id, altar_id } => ClientMessage::OfferAllBones {
                item_id: item_id.clone(),
                altar_id: altar_id.clone(),
            },
            InputCommand::PrayAtAltar { altar_id } => ClientMessage::PrayAtAltar {
                altar_id: altar_id.clone(),
            },
            // Spell commands
            InputCommand::CastSpell { spell_id } => ClientMessage::CastSpell {
                spell_id: spell_id.clone(),
            },
            InputCommand::Dash => ClientMessage::Dash,
            // Furnace commands
            InputCommand::FurnaceCraft {
                recipe_id,
                quantity,
            } => ClientMessage::StartCraftBatch {
                recipe_id: recipe_id.clone(),
                quantity: *quantity,
            },
            // Anvil commands
            InputCommand::AnvilCraft {
                recipe_id,
                quantity,
            } => ClientMessage::StartCraftBatch {
                recipe_id: recipe_id.clone(),
                quantity: *quantity,
            },
            // Alchemy Station commands
            InputCommand::AlchemyCraft {
                recipe_id,
                quantity,
            } => ClientMessage::StartCraftBatch {
                recipe_id: recipe_id.clone(),
                quantity: *quantity,
            },
            // Slayer commands
            InputCommand::SlayerGetTask { master_id } => ClientMessage::SlayerGetTask {
                master_id: master_id.clone(),
            },
            InputCommand::SlayerCancelTask => ClientMessage::SlayerCancelTask,
            InputCommand::SlayerBuyReward {
                reward_id,
                target_monster_id,
            } => ClientMessage::SlayerBuyReward {
                reward_id: reward_id.clone(),
                target_monster_id: target_monster_id.clone(),
            },
            InputCommand::SlayerRemoveBlock { monster_id } => ClientMessage::SlayerRemoveBlock {
                monster_id: monster_id.clone(),
            },
            // Chest commands
            InputCommand::ChestTake { chest_id, slot } => ClientMessage::ChestTake {
                chest_id: chest_id.clone(),
                slot: *slot,
            },
            InputCommand::ChestDeposit { chest_id, inventory_slot } => ClientMessage::ChestDeposit {
                chest_id: chest_id.clone(),
                inventory_slot: *inventory_slot,
            },
            InputCommand::StartAutoAction {
                target_type,
                target_id,
                action,
            } => ClientMessage::StartAutoAction {
                target_type: target_type.clone(),
                target_id: target_id.clone(),
                action: action.clone(),
            },
            InputCommand::CancelAutoAction => ClientMessage::CancelAutoAction,
            InputCommand::InteractObject { x, y } => ClientMessage::InteractObject {
                x: *x,
                y: *y,
            },
            InputCommand::UseWaystone { x, y } => ClientMessage::UseWaystone {
                x: *x,
                y: *y,
            },
        };
        network.send(&msg);
    }

    // Process pending portal trigger
    if let Some(portal_id) = game_state.pending_portal_id.take() {
        network.send(&crate::network::messages::ClientMessage::EnterPortal { portal_id });
    }

    // Auto-ping every 2 seconds when debug mode is active
    if game_state.debug_mode && network.is_connected() && game_state.ping_sent_at.is_none() {
        let now = get_time();
        if now - game_state.ping_stats.last_auto_ping >= 2.0 {
            game_state.ping_stats.last_auto_ping = now;
            game_state.ping_sent_at = Some(now);
            network.send(&crate::network::messages::ClientMessage::Ping { timestamp: now });
        }
    }

    // Record delta for diagnostics
    game_state.frame_timings.record_delta(delta as f64 * 1000.0);

    // 3.5. Tutorial: check if we should start, and update phase progress
    maybe_start_tutorial(game_state);
    update_tutorial(game_state);

    // 4. Update game state
    let update_start = get_time();
    game_state.update(delta);
    game_state.update_transition(delta);
    let update_ms = (get_time() - update_start) * 1000.0;

    // 4b. Request chunks around player position
    if let Some(player) = game_state.get_local_player() {
        let chunks_to_request = game_state
            .chunk_manager
            .update_player_position(player.server_x, player.server_y);
        for coord in chunks_to_request {
            network.send(&crate::network::messages::ClientMessage::RequestChunk {
                chunk_x: coord.x,
                chunk_y: coord.y,
            });
        }
        game_state.chunk_manager.unload_distant_chunks();
    }

    // Store frame timings
    let total_ms = (get_time() - frame_start) * 1000.0;
    game_state.frame_timings.network_ms = network_ms;
    game_state.frame_timings.render_total_ms = render_timings.total_ms;
    game_state.frame_timings.render_ground_ms = render_timings.ground_ms;
    game_state.frame_timings.render_entities_ms = render_timings.entities_ms;
    game_state.frame_timings.render_overhead_ms = render_timings.overhead_ms;
    game_state.frame_timings.render_effects_ms = render_timings.effects_ms;
    game_state.frame_timings.render_ui_ms = render_timings.ui_ms;
    game_state.frame_timings.update_ms = update_ms;
    game_state.frame_timings.total_ms = total_ms;
    game_state.frame_timings.entity_count =
        game_state.players.len() + game_state.npcs.len() + game_state.ground_items.len();
    game_state.frame_timings.chunk_count = game_state.chunk_manager.chunks().len();

    // 5. Debug info
    if game_state.debug_mode {
        let debug_y_offset = 20.0;
        let y = |base: f32| base + debug_y_offset;
        let fps_cap_str = match game_state.frame_timings.fps_cap {
            Some(cap) => format!(" (cap: {})", cap),
            None => " (uncapped)".to_string(),
        };
        renderer.draw_text_sharp(
            &format!("FPS: {}{} [F4]", get_fps(), fps_cap_str),
            10.0,
            y(20.0),
            16.0,
            WHITE,
        );
        renderer.draw_text_sharp(
            &format!("Players: {}", game_state.players.len()),
            10.0,
            y(40.0),
            16.0,
            WHITE,
        );
        let ping_str = if game_state.ping_stats.has_data() {
            let ps = &game_state.ping_stats;
            format!(
                " | Ping: {}ms (avg:{} min:{} max:{})",
                ps.current_ms.round() as i32,
                ps.avg_ms.round() as i32,
                ps.min_ms.round() as i32,
                ps.max_ms.round() as i32
            )
        } else if game_state.debug_mode {
            " | Ping: waiting...".to_string()
        } else {
            String::new()
        };
        renderer.draw_text_sharp(
            &format!("Connected: {}{}", network.is_connected(), ping_str),
            10.0,
            y(60.0),
            16.0,
            WHITE,
        );

        if let Some(player) = game_state.get_local_player() {
            let chunk_x = (player.x / 32.0).floor() as i32;
            let chunk_y = (player.y / 32.0).floor() as i32;
            renderer.draw_text_sharp(
                &format!("Pos: ({:.1}, {:.1})", player.x, player.y),
                10.0,
                y(80.0),
                16.0,
                YELLOW,
            );
            renderer.draw_text_sharp(
                &format!("Chunk: ({}, {})", chunk_x, chunk_y),
                10.0,
                y(100.0),
                16.0,
                YELLOW,
            );
            renderer.draw_text_sharp(
                &format!("NPCs: {}", game_state.npcs.len()),
                10.0,
                y(120.0),
                16.0,
                WHITE,
            );
            renderer.draw_text_sharp(
                &format!(
                    "Appearance: {} {} (F5/F6 to cycle)",
                    player.gender, player.skin
                ),
                10.0,
                y(140.0),
                16.0,
                Color::from_rgba(150, 200, 255, 255),
            );
        }

        let t = &game_state.frame_timings;
        let timing_color = Color::from_rgba(100, 255, 150, 255);
        let spike_color = Color::from_rgba(255, 100, 100, 255);
        renderer.draw_text_sharp(
            "--- Frame Timing (ms) ---",
            10.0,
            y(170.0),
            16.0,
            timing_color,
        );
        renderer.draw_text_sharp(
            &format!("Network:  {:.2}", t.network_ms),
            10.0,
            y(190.0),
            16.0,
            timing_color,
        );

        let ground_color = if t.render_ground_ms > 0.5 {
            spike_color
        } else {
            timing_color
        };
        let entities_color = if t.render_entities_ms > 0.5 {
            spike_color
        } else {
            timing_color
        };
        let overhead_color = if t.render_overhead_ms > 0.5 {
            spike_color
        } else {
            timing_color
        };
        let effects_color = if t.render_effects_ms > 0.5 {
            spike_color
        } else {
            timing_color
        };
        let ui_color = if t.render_ui_ms > 0.5 {
            spike_color
        } else {
            timing_color
        };

        renderer.draw_text_sharp(
            &format!("Render:   {:.2} (total)", t.render_total_ms),
            10.0,
            y(210.0),
            16.0,
            timing_color,
        );
        renderer.draw_text_sharp(
            &format!("  Ground:   {:.2}", t.render_ground_ms),
            10.0,
            y(230.0),
            16.0,
            ground_color,
        );
        renderer.draw_text_sharp(
            &format!("  Entities: {:.2}", t.render_entities_ms),
            10.0,
            y(250.0),
            16.0,
            entities_color,
        );
        renderer.draw_text_sharp(
            &format!("  Overhead: {:.2}", t.render_overhead_ms),
            10.0,
            y(270.0),
            16.0,
            overhead_color,
        );
        renderer.draw_text_sharp(
            &format!("  Effects:  {:.2}", t.render_effects_ms),
            10.0,
            y(290.0),
            16.0,
            effects_color,
        );
        renderer.draw_text_sharp(
            &format!("  UI:       {:.2}", t.render_ui_ms),
            10.0,
            y(310.0),
            16.0,
            ui_color,
        );

        renderer.draw_text_sharp(
            &format!("Update:   {:.2}", t.update_ms),
            10.0,
            y(330.0),
            16.0,
            timing_color,
        );
        renderer.draw_text_sharp(
            &format!("Total:    {:.2}", t.total_ms),
            10.0,
            y(350.0),
            16.0,
            timing_color,
        );
        renderer.draw_text_sharp(
            &format!("Entities: {} | Chunks: {}", t.entity_count, t.chunk_count),
            10.0,
            y(370.0),
            16.0,
            timing_color,
        );

        let delta_variance = t.delta_max_ms - t.delta_min_ms;
        let variance_color = if delta_variance > 5.0 {
            spike_color
        } else {
            timing_color
        };
        renderer.draw_text_sharp(
            &format!(
                "Delta: {:.1}ms (range: {:.1}-{:.1}, var: {:.1})",
                t.delta_ms, t.delta_min_ms, t.delta_max_ms, delta_variance
            ),
            10.0,
            y(390.0),
            16.0,
            variance_color,
        );

        let nf_variance = t.next_frame_max_ms - t.next_frame_min_ms;
        let nf_color = if nf_variance > 5.0 {
            spike_color
        } else {
            timing_color
        };
        renderer.draw_text_sharp(
            &format!(
                "next_frame(): {:.1}ms (range: {:.1}-{:.1}, var: {:.1})",
                t.next_frame_ms, t.next_frame_min_ms, t.next_frame_max_ms, nf_variance
            ),
            10.0,
            y(410.0),
            16.0,
            nf_color,
        );

        let smooth_str = if t.delta_smoothing > 0.0 {
            format!("{:.1}", t.delta_smoothing)
        } else {
            "off".to_string()
        };
        renderer.draw_text_sharp(
            &format!(
                "Smoothing: {} [F7] (smoothed: {:.1}ms)",
                smooth_str,
                t.smoothed_delta * 1000.0
            ),
            10.0,
            y(430.0),
            16.0,
            timing_color,
        );
    }

    // 6. Render overlays
    renderer.render_world_fade_in(game_state);
    renderer.render_transition_overlay(game_state);
    renderer.render_tutorial_hint(game_state);

    // 7. Render touch controls (mobile only)
    // Update attack button icon to show equipped weapon
    let weapon_sprite_key = game_state
        .get_local_player()
        .and_then(|p| p.equipped_weapon.as_deref())
        .map(|id| game_state.item_registry.get_sprite_key(id));
    input_handler.update_attack_button_icon(weapon_sprite_key, &renderer.item_sprites);

    // Hide controls when any panel is open
    let in_dialogue = game_state.ui_state.active_dialogue.is_some();
    let any_panel_open = game_state.ui_state.inventory_open
        || game_state.ui_state.character_panel_open
        || game_state.ui_state.skills_open
        || game_state.ui_state.prayer_book_open
        || game_state.ui_state.escape_menu_open
        || game_state.ui_state.crafting_open
        || game_state.ui_state.shop_data.is_some()
        || game_state.ui_state.quest_log_open
        || game_state.ui_state.social_open
        || game_state.ui_state.chat_panel_open
        || in_dialogue;
    let hide_direction_controls = game_state.ui_state.escape_menu_open
        || game_state.ui_state.crafting_open
        || game_state.ui_state.shop_data.is_some()
        || game_state.ui_state.quest_log_open
        || game_state.ui_state.chat_panel_open
        || in_dialogue;
    input_handler.render_touch_controls(
        any_panel_open,
        hide_direction_controls,
        game_state.ui_state.use_joystick,
    );
}
