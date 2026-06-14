use macroquad::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::audio::AudioManager;
use crate::game::GameState;
use crate::input::{InputCommand, InputHandler};
use crate::network::{self, NetworkClient};
use crate::render::animation::AnimationState;
use crate::render::Renderer;
use crate::{game, settings};

static FULLSCREEN: AtomicBool = AtomicBool::new(false);

/// Run a single frame of gameplay
pub(crate) fn run_game_frame(
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

    // Toggle fullscreen (borderless on Windows)
    if is_key_pressed(KeyCode::F11) {
        let new_state = !FULLSCREEN.load(Ordering::Relaxed);
        FULLSCREEN.store(new_state, Ordering::Relaxed);
        set_fullscreen(new_state);
        log::info!("Fullscreen: {}", new_state);
    }

    // Cycle FPS cap with F4: 60 -> 140 -> 60
    if is_key_pressed(KeyCode::F4) {
        game_state.frame_timings.fps_cap = match game_state.frame_timings.fps_cap {
            Some(60) => Some(140),
            _ => Some(60),
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

                // F8 to toggle/cycle animation viewer
                if is_key_pressed(KeyCode::F8) {
                    use crate::render::animation::AnimationState;
                    let anim_states = [
                        AnimationState::Idle,
                        AnimationState::Walking,
                        AnimationState::Attacking,
                        AnimationState::SittingGround,
                        AnimationState::SittingChair,
                        AnimationState::Casting,
                        AnimationState::ShootingBow,
                    ];
                    game_state.debug_anim_viewer = match game_state.debug_anim_viewer {
                        None => {
                            player.animation.set_state(anim_states[0]);
                            player.animation.frame = 0.0;
                            Some((0, true))
                        }
                        Some((idx, paused)) => {
                            let next = idx + 1;
                            if next >= anim_states.len() {
                                // Restore normal animation
                                player.animation.set_state(AnimationState::Idle);
                                None
                            } else {
                                player.animation.set_state(anim_states[next]);
                                player.animation.frame = 0.0;
                                Some((next, paused))
                            }
                        }
                    };
                }

                // F9/F10 to step frames when animation viewer is active
                if let Some((_, ref mut paused)) = game_state.debug_anim_viewer {
                    // F10 to toggle pause/play
                    if is_key_pressed(KeyCode::F10) {
                        *paused = !*paused;
                    }
                    // F9 to step forward one frame (auto-pauses)
                    if is_key_pressed(KeyCode::F9) {
                        let config =
                            crate::render::animation::get_animation_config(player.animation.state);
                        player.animation.frame =
                            ((player.animation.frame as u32 + 1) % config.frame_count) as f32;
                        *paused = true;
                    }
                }
            }
        }
    }

    // 1. Poll network messages
    let network_start = get_time();
    network.poll(game_state);
    let network_ms = (get_time() - network_start) * 1000.0;

    // 1.5. Play any pending sound effects queued by message handlers
    // If we were backgrounded (tab inactive), the delta will be huge — clear queues
    // instead of playing all accumulated sounds at once
    if delta > 0.5 {
        game_state.pending_sfx.clear();
        game_state.pending_attack_sounds.clear();
    } else {
        for sfx_name in game_state.pending_sfx.drain(..) {
            audio.play_sfx(&sfx_name);
        }
        for attack_type in game_state.pending_attack_sounds.drain(..) {
            audio.play_attack_sound(attack_type);
        }
    }

    // Play pending music track change
    if let Some(music_path) = game_state.pending_music.take() {
        audio.play_music_preloaded(&music_path);
    }

    // 2. Render and get UI layout for hit detection
    clear_background(Color::from_rgba(30, 30, 40, 255));
    let (layout, render_timings) = renderer.render(game_state);

    // 3. Handle input with UI layout and send commands
    let commands = input_handler.process(game_state, &layout, audio);
    for cmd in &commands {
        use network::messages::ClientMessage;
        let msg = match cmd {
            InputCommand::Move { dx, dy } => {
                // Notify tutorial of player movement
                if *dx != 0.0 || *dy != 0.0 {
                    if let Some(tutorial) = &mut game_state.tutorial {
                        tutorial.on_player_moved();
                    }
                }
                let seq = game_state.next_move_sequence(*dx, *dy);
                ClientMessage::Move {
                    dx: *dx,
                    dy: *dy,
                    seq: Some(seq),
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
                        let new_dir = game::Direction::from_u8(*direction);
                        player.direction = new_dir;
                        player.animation.direction = new_dir; // Also update animation direction for rendering
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
                // The swing animation and sound are server-authoritative: they are
                // driven by the server's PlayerAttack echo, not predicted locally.
                // This prevents phantom/double swings when a manual attack and an
                // auto-retaliate swing overlap (the client can't know the server will
                // reject a swing for cooldown until the echo, or lack of one, arrives).
                // We still validate arrows client-side so we can give immediate
                // feedback and avoid sending an attack that can't land.
                if let Some(local_id) = &game_state.local_player_id {
                    let is_ranged = game_state
                        .players
                        .get(local_id)
                        .and_then(|p| p.equipped_weapon.as_ref())
                        .and_then(|weapon_id| game_state.item_registry.get(weapon_id))
                        .map(|item_def| item_def.weapon_type.as_deref() == Some("ranged"))
                        .unwrap_or(false);

                    if is_ranged {
                        let has_arrows = game_state.inventory.slots.iter().any(|slot| {
                            slot.as_ref().is_some_and(|s| s.item_id.ends_with("_arrow"))
                        });
                        if !has_arrows {
                            // No arrows - play error sound and show message
                            game_state.pending_sfx.push("error".to_string());
                            game_state.push_system_chat("You have no arrows!".to_string());
                            continue;
                        }
                    }
                }
                ClientMessage::Attack
            }
            InputCommand::Jump => ClientMessage::Jump,
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
                    game_state.manual_ping = true;
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
                slot_index: *slot_index,
            },
            InputCommand::UseItemOnEntity { slot_index, npc_id } => ClientMessage::UseItemOn {
                slot_index: *slot_index,
                target_npc_id: npc_id.clone(),
            },
            // Quest-related commands
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
                    settings::save_classic_controls(classic);
                    settings::save_control_scheme_chosen();
                    game_state.ui_state.active_dialogue = None;
                    continue;
                }
                if quest_id == "__tutorial__" {
                    game_state.ui_state.active_dialogue = None;
                    // Create TutorialManager if it doesn't exist yet
                    if game_state.tutorial.is_none() {
                        game_state.tutorial = Some(game::tutorial::TutorialManager::new(
                            game_state.ui_state.classic_controls,
                        ));
                    }
                    if let Some(tutorial) = &mut game_state.tutorial {
                        if tutorial.phase == game::tutorial::TutorialPhase::AwaitingAccept {
                            if choice_id == "accept" {
                                tutorial.advance(); // -> Movement
                                tutorial.pending_dialogue = true;
                            } else {
                                tutorial.skip();
                                settings::save_tutorial_completed();
                            }
                        }
                    }
                    continue;
                }
                // Start port travel fade if selecting a port destination
                if quest_id.starts_with("port:") && choice_id.starts_with("port_dest_") {
                    game_state.map_transition = game::state::MapTransition {
                        state: game::state::TransitionState::FadingOut,
                        progress: 0.0,
                        target_map_type: String::new(),
                        target_map_id: String::new(),
                        target_spawn_x: 0.0,
                        target_spawn_y: 0.0,
                        instance_id: String::new(),
                    };
                }
                ClientMessage::DialogueChoiceMsg {
                    quest_id: quest_id.clone(),
                    choice_id: choice_id.clone(),
                }
            }
            InputCommand::CloseDialogue => {
                // Just close locally, no server message needed
                continue;
            }
            // Crafting command
            InputCommand::Craft { recipe_id } => ClientMessage::StartCraft {
                recipe_id: recipe_id.clone(),
            },
            InputCommand::CancelCraft => ClientMessage::CancelCraft,
            // Equipment commands
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
            // Inventory commands
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
            // Shop commands
            InputCommand::ShopBuy {
                npc_id,
                item_id,
                quantity,
            } => ClientMessage::ShopBuy {
                npc_id: npc_id.clone(),
                item_id: item_id.clone(),
                quantity: (*quantity).min(i32::MAX as u32) as i32,
            },
            InputCommand::ShopSell {
                npc_id,
                item_id,
                quantity,
            } => ClientMessage::ShopSell {
                npc_id: npc_id.clone(),
                item_id: item_id.clone(),
                quantity: (*quantity).min(i32::MAX as u32) as i32,
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
            // Portal commands
            InputCommand::EnterPortal { portal_id } => ClientMessage::EnterPortal {
                portal_id: portal_id.clone(),
            },
            InputCommand::StartGathering { marker_x, marker_y } => {
                // Swing animation is server-authoritative (driven by the gatheringStarted
                // echo), so no local prediction here — keeps it consistent with combat
                // and gathering and avoids double animations.
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
                // Swing animation is server-authoritative (driven by the woodcuttingSwing
                // echo), so no local prediction here.
                // Optimistically show the woodcutting indicator. Woodcutting has
                // no server "started" session, so we drive the indicator off swing
                // messages (and time it out in GameState::update); setting it here
                // makes the bar appear immediately instead of after the first swing
                // round-trips.
                game_state.is_woodcutting = true;
                game_state.woodcutting_started_at = get_time();
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
                // Swing animation is server-authoritative (driven by the miningSwing
                // echo), so no local prediction here.
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
            // Workbench commands
            InputCommand::WorkbenchCraft {
                recipe_id,
                quantity,
            } => ClientMessage::StartCraftBatch {
                recipe_id: recipe_id.clone(),
                quantity: *quantity,
            },
            // Fletching commands
            InputCommand::FletchingCraft {
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
            InputCommand::ChestDeposit {
                chest_id,
                inventory_slot,
            } => ClientMessage::ChestDeposit {
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
            InputCommand::InteractObject { x, y } => ClientMessage::InteractObject { x: *x, y: *y },
            InputCommand::UseWaystone { x, y } => ClientMessage::UseWaystone { x: *x, y: *y },
            // Trade commands
            InputCommand::TradeRequest { target_id } => ClientMessage::TradeRequest {
                target_id: target_id.clone(),
            },
            InputCommand::TradeAcceptRequest { requester_id } => {
                ClientMessage::TradeAcceptRequest {
                    requester_id: requester_id.clone(),
                }
            }
            InputCommand::TradeDeclineRequest { requester_id } => {
                ClientMessage::TradeDeclineRequest {
                    requester_id: requester_id.clone(),
                }
            }
            InputCommand::TradeOfferItem {
                slot_index,
                quantity,
            } => ClientMessage::TradeOfferItem {
                slot_index: *slot_index,
                quantity: *quantity,
            },
            InputCommand::TradeRemoveItem { offer_index } => ClientMessage::TradeRemoveItem {
                offer_index: *offer_index,
            },
            InputCommand::TradeOfferGold { amount } => {
                ClientMessage::TradeOfferGold { amount: *amount }
            }
            InputCommand::TradeAccept => ClientMessage::TradeAccept,
            InputCommand::TradeCancel => ClientMessage::TradeCancel,
            // Stall commands
            InputCommand::StallOpen { name } => ClientMessage::StallOpen { name: name.clone() },
            InputCommand::StallClose => ClientMessage::StallClose,
            InputCommand::StallSetItem {
                inventory_slot,
                quantity,
                price,
            } => ClientMessage::StallSetItem {
                inventory_slot: *inventory_slot,
                quantity: *quantity,
                price: *price,
            },
            InputCommand::StallRemoveItem { stall_slot } => ClientMessage::StallRemoveItem {
                stall_slot: *stall_slot,
            },
            InputCommand::StallBrowse { player_id } => ClientMessage::StallBrowse {
                player_id: player_id.clone(),
            },
            InputCommand::StallBuy {
                seller_id,
                stall_slot,
                quantity,
                expected_price,
            } => ClientMessage::StallBuy {
                seller_id: seller_id.clone(),
                stall_slot: *stall_slot,
                quantity: *quantity,
                expected_price: *expected_price,
            },
            InputCommand::SetCombatStyle { style } => ClientMessage::SetCombatStyle {
                style: style.clone(),
            },
            InputCommand::KothContinue => ClientMessage::KothContinue,
            InputCommand::KothLeave => ClientMessage::KothLeave,
            InputCommand::SetAutoRetaliate { enabled } => {
                ClientMessage::SetAutoRetaliate { enabled: *enabled }
            }
        };
        network.send(&msg);
    }

    // Process pending portal trigger (auto-triggered by walking onto portal)
    if let Some(portal_id) = game_state.pending_portal_id.take() {
        network.send(&network::messages::ClientMessage::EnterPortal { portal_id });
    }

    // Auto-ping: 2s in debug mode (for stats), 20s otherwise (keepalive)
    if network.is_connected() && game_state.ping_sent_at.is_none() {
        let interval = if game_state.debug_mode { 2.0 } else { 20.0 };
        let now = get_time();
        if now - game_state.ping_stats.last_auto_ping >= interval {
            game_state.ping_stats.last_auto_ping = now;
            game_state.ping_sent_at = Some(now);
            network.send(&network::messages::ClientMessage::Ping { timestamp: now });
        }
    }

    // Record delta for diagnostics
    game_state.frame_timings.record_delta(delta as f64 * 1000.0);

    // 3.5. Tutorial: check if we should start, and update phase progress
    // maybe_start_tutorial
    if game_state.tutorial_pending && game_state.ui_state.active_dialogue.is_none() {
        log::warn!("TUTORIAL: auto-starting tutorial now!");
        game_state.tutorial_pending = false;
        let mut tutorial =
            game::tutorial::TutorialManager::new(game_state.ui_state.classic_controls);
        if let Some(dialogue) = tutorial.phase_dialogue() {
            game_state.ui_state.active_dialogue = Some(dialogue);
        }
        tutorial.hint_visible = false;
        game_state.tutorial = Some(tutorial);
    }
    // update_tutorial
    if let Some(tutorial) = &mut game_state.tutorial {
        if !tutorial.is_done() {
            if tutorial.hint_visible
                && game_state.ui_state.active_dialogue.is_none()
                && is_key_pressed(KeyCode::Escape)
            {
                tutorial.skip();
                settings::save_tutorial_completed();
            } else {
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
                if tutorial.phase == game::tutorial::TutorialPhase::Handoff
                    && game_state.ui_state.active_dialogue.is_none()
                    && !tutorial.pending_dialogue
                {
                    tutorial.advance();
                    tutorial.hint_visible = false;
                    settings::save_tutorial_completed();
                }
            }
        }
    }

    // 4. Update game state
    // Save debug animation state before update overwrites it
    let debug_anim_freeze = if let Some((idx, true)) = game_state.debug_anim_viewer {
        if let Some(local_id) = &game_state.local_player_id {
            game_state
                .players
                .get(local_id)
                .map(|p| (idx, p.animation.state, p.animation.frame))
        } else {
            None
        }
    } else {
        None
    };

    let update_start = get_time();
    game_state.update(delta);
    game_state.update_transition(delta);
    let update_ms = (get_time() - update_start) * 1000.0;

    // Restore frozen animation after update
    if let Some((_idx, frozen_state, frozen_frame)) = debug_anim_freeze {
        if let Some(local_id) = &game_state.local_player_id.clone() {
            if let Some(player) = game_state.players.get_mut(local_id) {
                player.animation.state = frozen_state;
                player.animation.frame = frozen_frame;
            }
        }
    }

    // 4b. Request chunks around player position
    if let Some(player) = game_state.get_local_player() {
        let chunks_to_request = game_state
            .chunk_manager
            .update_player_position(player.server_x, player.server_y);
        for coord in chunks_to_request {
            network.send(&network::messages::ClientMessage::RequestChunk {
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

    // 5. Debug info (render after game state update to show current frame data)
    if game_state.debug_mode {
        // Position below the stat bars (name tag + HP/MP/Prayer bars)
        let debug_y_offset = 115.0;
        let y = |base: f32| base + debug_y_offset;

        // Semi-transparent background for readability
        draw_rectangle(
            4.0,
            debug_y_offset + 8.0,
            340.0,
            440.0,
            Color::from_rgba(0, 0, 0, 140),
        );

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
        } else {
            " | Ping: waiting...".to_string()
        };
        let connected_ping = format!("Connected: {}{}", network.is_connected(), ping_str);
        let ping_color =
            if game_state.ping_stats.has_data() && game_state.ping_stats.current_ms > 200.0 {
                Color::from_rgba(255, 100, 100, 255)
            } else if game_state.ping_stats.has_data() && game_state.ping_stats.current_ms > 120.0 {
                Color::from_rgba(255, 200, 100, 255)
            } else {
                WHITE
            };
        renderer.draw_text_sharp(&connected_ping, 10.0, y(60.0), 16.0, ping_color);

        // Show position and chunk info
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
            // Appearance debug info
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

            // Animation viewer info
            if let Some((idx, paused)) = game_state.debug_anim_viewer {
                let anim_names = [
                    "Idle",
                    "Walking",
                    "Attacking",
                    "SittingGround",
                    "SittingChair",
                    "Casting",
                    "ShootingBow",
                ];
                let config = crate::render::animation::get_animation_config(player.animation.state);
                let pause_str = if paused { "PAUSED" } else { "PLAYING" };
                renderer.draw_text_sharp(
                    &format!(
                        "Anim: {} frame {}/{} {} [F8:next F9:step F10:pause]",
                        anim_names[idx],
                        player.animation.frame as u32,
                        config.frame_count,
                        pause_str,
                    ),
                    10.0,
                    y(160.0),
                    16.0,
                    Color::from_rgba(255, 200, 100, 255),
                );
            } else {
                renderer.draw_text_sharp(
                    "Anim viewer: off [F8 to start]",
                    10.0,
                    y(160.0),
                    16.0,
                    Color::from_rgba(150, 150, 150, 255),
                );
            }
        }

        // Frame timing breakdown
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

        // Render breakdown with spike highlighting (>0.5ms highlighted)
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

        // Delta variance (key indicator of frame pacing issues)
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

        // next_frame() timing (helps diagnose where variance comes from)
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

        // Delta smoothing setting
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

    // 6. Render touch controls where the platform exposes them.
    let weapon_sprite_key = game_state
        .get_local_player()
        .and_then(|player| player.equipped_weapon.as_deref())
        .map(|id| game_state.item_registry.get_sprite_key(id));
    input_handler.update_attack_button_icon(weapon_sprite_key, &renderer.item_sprites);

    let in_dialogue = game_state.ui_state.active_dialogue.is_some();
    let any_panel_open = game_state.ui_state.inventory_open
        || game_state.ui_state.character_panel_open
        || game_state.ui_state.skills_open
        || game_state.ui_state.prayer_book_open
        || game_state.ui_state.escape_menu_open
        || game_state.ui_state.crafting_open
        || game_state.ui_state.furnace_open
        || game_state.ui_state.anvil_open
        || game_state.ui_state.fletching_open
        || game_state.ui_state.bank_open
        || game_state.ui_state.chest_open
        || game_state.ui_state.shop_data.is_some()
        || game_state.ui_state.quest_log_open
        || game_state.ui_state.social_open
        || game_state.ui_state.chat_panel_open
        || in_dialogue;
    let hide_direction_controls = if cfg!(target_os = "android") {
        game_state.ui_state.crafting_open
            || game_state.ui_state.furnace_open
            || game_state.ui_state.anvil_open
            || game_state.ui_state.fletching_open
            || game_state.ui_state.bank_open
            || game_state.ui_state.chest_open
            || game_state.ui_state.shop_data.is_some()
            || game_state.ui_state.chat_panel_open
            || in_dialogue
    } else {
        game_state.ui_state.escape_menu_open
            || game_state.ui_state.crafting_open
            || game_state.ui_state.shop_data.is_some()
            || game_state.ui_state.quest_log_open
            || game_state.ui_state.chat_panel_open
            || in_dialogue
    };
    input_handler.render_touch_controls(
        any_panel_open,
        hide_direction_controls,
        game_state.ui_state.use_joystick,
    );

    // 7. Render overlays last so transitions consistently cover every platform UI.
    renderer.render_world_fade_in(game_state);
    renderer.render_transition_overlay(game_state);
    renderer.render_tutorial_hint(game_state);
}
