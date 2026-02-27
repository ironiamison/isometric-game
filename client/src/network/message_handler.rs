use super::protocol::{
    extract_array, extract_bool, extract_f32, extract_i32, extract_string, extract_u32,
    extract_u64, extract_u8,
};
use crate::game::npc::{Npc, NpcState};
use crate::game::{
    ActiveDialogue, ActiveQuest, BonusTile, ChatBubble, ChatChannel, ChatMessage, ConnectionStatus,
    DamageEvent, DialogueChoice, Direction, EquipmentStats, FarmingPatch, FriendInfo, GameState,
    GatheringBuff, GatheringMarker, GroundItem, InventorySlot, ItemDefinition, LevelUpEvent,
    MapObject, OnlinePlayerInfo, PendingRequestInfo, Player, Portal, CatalogObjective, QuestCatalogEntry,
    QuestCompletedEvent, QuestObjective, RecipeDefinition, RecipeIngredient, RecipeResult,
    ShopData, ShopStockItem,
    SkillType, SkillXpEvent, SpellEffect, TransitionState, Wall, WallEdge,
};
use crate::render::OVERWORLD_NAME;

/// Max tile distance from local player to play other players' SFX.
/// Roughly matches the visible screen area so you don't hear off-screen actions.
const SFX_AUDIBLE_RANGE: f32 = 20.0;

fn adventurer_guide_tier_id(tab_idx: usize, tier_idx: usize) -> Option<&'static str> {
    match (tab_idx, tier_idx) {
        (0, 0) => Some("adventurer_tier_1"),
        (0, 1) => Some("adventurer_tier_2"),
        (0, 2) => Some("adventurer_tier_3"),
        (1, 0) => Some("skilling_tier_1"),
        (1, 1) => Some("skilling_tier_2"),
        (1, 2) => Some("skilling_tier_3"),
        _ => None,
    }
}

fn reset_adventurer_guide_dialogue(state: &mut GameState) -> bool {
    let is_guide = state
        .ui_state
        .active_dialogue
        .as_ref()
        .map(|d| d.speaker.eq_ignore_ascii_case("Adventurer Guide"))
        .unwrap_or(false);

    if !is_guide {
        return false;
    }

    if let Some(dialogue) = state.ui_state.active_dialogue.as_mut() {
        if let Some(tier_id) = adventurer_guide_tier_id(
            state.ui_state.adventurer_selected_tab,
            state.ui_state.adventurer_selected_tier,
        ) {
            dialogue.quest_id = tier_id.to_string();
        } else {
            dialogue.quest_id.clear();
        }
        dialogue.choices.clear();
        dialogue.text =
            "Select a tier to review progress. Talk to the guide to start or complete tiers."
                .to_string();
    }
    state.ui_state.dialogue_scroll_offset = 0.0;
    state.ui_state.dialogue_touch_scroll_id = None;
    state.ui_state.dialogue_touch_dragged = false;
    true
}

/// Extract a nested map value from an rmpv map by key.
fn extract_map_field<'a>(value: &'a rmpv::Value, key: &str) -> Option<&'a rmpv::Value> {
    value.as_map().and_then(|map| {
        map.iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .map(|(_, v)| v)
    })
}

fn extract_i64(value: &rmpv::Value, key: &str) -> Option<i64> {
    value.as_map().and_then(|map| {
        map.iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .and_then(|(_, v)| v.as_i64().or_else(|| v.as_u64().map(|u| u as i64)))
    })
}

fn extract_slayer_task(value: &rmpv::Value, key: &str) -> Option<crate::game::slayer::SlayerTaskClientData> {
    let task_val = extract_map_field(value, key)?;
    if task_val.is_nil() {
        return None;
    }
    Some(crate::game::slayer::SlayerTaskClientData {
        monster_id: extract_string(task_val, "monster_id").unwrap_or_default(),
        display_name: extract_string(task_val, "display_name").unwrap_or_default(),
        kills_current: extract_i32(task_val, "kills_current").unwrap_or(0),
        kills_required: extract_i32(task_val, "kills_required").unwrap_or(0),
        xp_per_kill: extract_i64(task_val, "xp_per_kill").unwrap_or(0),
        master_id: extract_string(task_val, "master_id").unwrap_or_default(),
        points_on_complete: extract_i32(task_val, "points_on_complete").unwrap_or(0),
    })
}

fn extract_slayer_rewards(value: &rmpv::Value, key: &str) -> Vec<crate::game::slayer::SlayerRewardClientData> {
    let mut rewards = Vec::new();
    if let Some(arr) = extract_map_field(value, key) {
        if let rmpv::Value::Array(ref items) = *arr {
            for item in items {
                rewards.push(crate::game::slayer::SlayerRewardClientData {
                    id: extract_string(item, "id").unwrap_or_default(),
                    display_name: extract_string(item, "display_name").unwrap_or_default(),
                    description: extract_string(item, "description").unwrap_or_default(),
                    cost: extract_i32(item, "cost").unwrap_or(0),
                    category: extract_string(item, "category").unwrap_or_default(),
                    target_id: extract_string(item, "target_id"),
                    quantity: extract_i32(item, "quantity").unwrap_or(1),
                });
            }
        }
    }
    rewards
}

fn extract_string_array(value: &rmpv::Value, key: &str) -> Vec<String> {
    let mut result = Vec::new();
    if let Some(arr) = extract_map_field(value, key) {
        if let rmpv::Value::Array(ref items) = *arr {
            for item in items {
                if let Some(s) = item.as_str() {
                    result.push(s.to_string());
                }
            }
        }
    }
    result
}

pub fn handle_room_data(msg_type: &str, data: Option<&rmpv::Value>, state: &mut GameState) {
    match msg_type {
        "welcome" => {
            if let Some(value) = data {
                if let Some(player_id) = extract_string(value, "player_id") {
                    log::info!("Welcome! Player ID: {}", player_id);
                    state.local_player_id = Some(player_id);
                    state.reset_move_sequence_state();
                    state.connection_status = ConnectionStatus::Connected;

                    // Check if this is a new character (for tutorial)
                    let is_new = extract_bool(value, "is_new_character").unwrap_or(false);
                    let tutorial_done = crate::settings::load_tutorial_completed();
                    log::warn!("TUTORIAL: Welcome data={:?}", value);
                    log::warn!("TUTORIAL: is_new={}, tutorial_done={}", is_new, tutorial_done);
                    if is_new && !tutorial_done {
                        log::info!("Tutorial: setting tutorial_pending = true");
                        state.tutorial_pending = true;
                    }
                }
            }
        }

        "playerJoined" => {
            if let Some(value) = data {
                let id = extract_string(value, "id").unwrap_or_default();
                let name = extract_string(value, "name").unwrap_or_default();
                // Server sends i32 grid positions
                let x = extract_i32(value, "x").unwrap_or(0) as f32;
                let y = extract_i32(value, "y").unwrap_or(0) as f32;
                // Appearance
                let gender = extract_string(value, "gender").unwrap_or_else(|| "male".to_string());
                let skin = extract_string(value, "skin").unwrap_or_else(|| "tan".to_string());
                let hair_style = extract_i32(value, "hair_style");
                let hair_color = extract_i32(value, "hair_color");
                // Equipment (filter empty strings to None)
                let equipped_head =
                    extract_string(value, "equipped_head").filter(|s| !s.is_empty());
                let equipped_body =
                    extract_string(value, "equipped_body").filter(|s| !s.is_empty());
                let equipped_weapon =
                    extract_string(value, "equipped_weapon").filter(|s| !s.is_empty());
                let equipped_back =
                    extract_string(value, "equipped_back").filter(|s| !s.is_empty());
                let equipped_feet =
                    extract_string(value, "equipped_feet").filter(|s| !s.is_empty());
                let equipped_ring =
                    extract_string(value, "equipped_ring").filter(|s| !s.is_empty());
                let equipped_gloves =
                    extract_string(value, "equipped_gloves").filter(|s| !s.is_empty());
                let equipped_necklace =
                    extract_string(value, "equipped_necklace").filter(|s| !s.is_empty());
                let equipped_belt =
                    extract_string(value, "equipped_belt").filter(|s| !s.is_empty());
                // Admin status
                let is_admin = extract_bool(value, "is_admin").unwrap_or(false);

                log::info!(
                    "Player joined: {} at ({}, {}) [{}/{}]",
                    name,
                    x,
                    y,
                    gender,
                    skin
                );
                let mut player = Player::new(id.clone(), name, x, y, gender, skin);
                player.hair_style = hair_style;
                player.hair_color = hair_color;
                player.equipped_head = equipped_head;
                player.equipped_body = equipped_body;
                player.equipped_weapon = equipped_weapon;
                player.equipped_back = equipped_back;
                player.equipped_feet = equipped_feet;
                player.equipped_ring = equipped_ring;
                player.equipped_gloves = equipped_gloves;
                player.equipped_necklace = equipped_necklace;
                player.equipped_belt = equipped_belt;
                player.is_admin = is_admin;
                state.players.insert(id, player);
            }
        }

        "playerLeft" => {
            if let Some(value) = data {
                if let Some(id) = extract_string(value, "id") {
                    log::info!("Player left: {}", id);
                    state.players.remove(&id);
                }
            }
        }

        "stateSync" => {
            if let Some(value) = data {
                let tick = extract_u64(value, "tick").unwrap_or(0);

                // Discard stale StateSyncs from a different map context.
                // This prevents instance NPCs (e.g. Elder Mara) from appearing
                // in the overworld when a StateSync races with a map transition.
                let sync_instance = extract_string(value, "instanceId").unwrap_or_default();
                let current_instance = state.current_instance.clone().unwrap_or_default();
                if sync_instance != current_instance {
                    // Context mismatch — skip this entire StateSync
                    return;
                }

                // Ignore out-of-order snapshots to prevent positional rewinds/jitter.
                if tick < state.server_tick {
                    log::trace!(
                        "Dropping stale stateSync tick {} (latest {})",
                        tick,
                        state.server_tick
                    );
                    return;
                }
                state.server_tick = tick;

                // Delta sync: "full" field absent or true = full snapshot, false = delta
                let is_full_sync = extract_bool(value, "full").unwrap_or(true);

                // Update players (grid positions from server)
                let mut player_regen_events: Vec<(String, f32, f32, i32)> = Vec::new();
                let mut synced_player_ids: Vec<String> = Vec::new();
                if let Some(players) = extract_array(value, "players") {
                    for player_value in players {
                        let id = extract_string(player_value, "id").unwrap_or_default();
                        synced_player_ids.push(id.clone());
                        let name = extract_string(player_value, "name").unwrap_or_default();
                        // Server sends i32 grid positions
                        let x = extract_i32(player_value, "x");
                        let y = extract_i32(player_value, "y");
                        let direction = extract_i32(player_value, "direction");
                        let hp = extract_i32(player_value, "hp");
                        let max_hp = extract_i32(player_value, "maxHp");
                        let mp = extract_i32(player_value, "mp");
                        let max_mp = extract_i32(player_value, "maxMp");
                        // Skill levels (consolidated combat system)
                        let hitpoints_level = extract_i32(player_value, "hitpointsLevel");
                        let combat_skill_level = extract_i32(player_value, "combatSkillLevel");
                        let gold = extract_i32(player_value, "gold");
                        let gender = extract_string(player_value, "gender")
                            .unwrap_or_else(|| "male".to_string());
                        let skin = extract_string(player_value, "skin")
                            .unwrap_or_else(|| "tan".to_string());
                        let hair_style = extract_i32(player_value, "hair_style");
                        let hair_color = extract_i32(player_value, "hair_color");
                        let equipped_head =
                            extract_string(player_value, "equipped_head").filter(|s| !s.is_empty());
                        let equipped_body =
                            extract_string(player_value, "equipped_body").filter(|s| !s.is_empty());
                        let equipped_weapon = extract_string(player_value, "equipped_weapon")
                            .filter(|s| !s.is_empty());
                        let equipped_back =
                            extract_string(player_value, "equipped_back").filter(|s| !s.is_empty());
                        let equipped_feet =
                            extract_string(player_value, "equipped_feet").filter(|s| !s.is_empty());
                        let equipped_ring =
                            extract_string(player_value, "equipped_ring").filter(|s| !s.is_empty());
                        let equipped_gloves = extract_string(player_value, "equipped_gloves")
                            .filter(|s| !s.is_empty());
                        let equipped_necklace = extract_string(player_value, "equipped_necklace")
                            .filter(|s| !s.is_empty());
                        let equipped_belt =
                            extract_string(player_value, "equipped_belt").filter(|s| !s.is_empty());
                        let is_admin = extract_bool(player_value, "is_admin").unwrap_or(false);
                        let has_stall = extract_bool(player_value, "has_stall").unwrap_or(false);
                        let stall_name = extract_string(player_value, "stall_name");
                        let move_ack_seq = extract_u32(player_value, "moveAckSeq");

                        let is_local_player = state.local_player_id.as_ref() == Some(&id);
                        if is_local_player {
                            if let Some(ack_seq) = move_ack_seq {
                                state.acknowledge_move_sequence(ack_seq);
                            }
                        }
                        let has_pending_local_moves =
                            is_local_player && state.has_pending_move_sequences();
                        let catchup_softness = state.sync_catchup_softness();
                        let catchup_lead_scale = state.sync_catchup_lead_scale();
                        let local_lead_scale = if is_local_player {
                            state.local_prediction_lead_scale() * catchup_lead_scale
                        } else {
                            1.0
                        };
                        let local_reconciliation_softness = if is_local_player {
                            state.local_reconciliation_softness() * catchup_softness
                        } else {
                            catchup_softness
                        };

                        if let Some(player) = state.players.get_mut(&id) {
                            // Read velocity (movement intent) from server
                            let vel_x = extract_i32(player_value, "velX").unwrap_or(0) as f32;
                            let vel_y = extract_i32(player_value, "velY").unwrap_or(0) as f32;
                            // Direction from server
                            let dir = direction
                                .map(|d| Direction::from_u8(d as u8))
                                .unwrap_or(player.direction);

                            // Check if player is dashing (set before set_server_state so it handles interpolation)
                            let dashing = extract_bool(player_value, "dashing").unwrap_or(false);
                            if dashing {
                                player.is_dashing = true;
                            }

                            if let (Some(x), Some(y)) = (x, y) {
                                // Set server state - local player direction only updates when moving
                                player.set_server_state(
                                    x as f32,
                                    y as f32,
                                    vel_x,
                                    vel_y,
                                    dir,
                                    is_local_player,
                                    has_pending_local_moves,
                                    local_lead_scale,
                                    local_reconciliation_softness,
                                );
                            } else if direction.is_some() && !is_local_player {
                                // Direction-only update for remote players
                                // Local player direction is controlled locally when stationary
                                player.direction = dir;
                            }
                            if let Some(hp) = hp {
                                // Update last_damage_time if HP decreased (ensures HP bar shows)
                                if hp < player.hp {
                                    player.last_damage_time = macroquad::time::get_time();
                                } else if hp > player.hp && player.hp > 0 {
                                    // HP increased (regen) - record for floating text
                                    let heal_amount = hp - player.hp;
                                    player_regen_events.push((
                                        id.clone(),
                                        player.x,
                                        player.y,
                                        heal_amount,
                                    ));
                                }
                                player.hp = hp;
                            }
                            if let Some(max_hp) = max_hp {
                                player.max_hp = max_hp;
                            }
                            if let Some(mp) = mp {
                                player.mp = mp;
                            }
                            if let Some(max_mp) = max_mp {
                                player.max_mp = max_mp;
                            }
                            // Update skill levels
                            if let Some(level) = hitpoints_level {
                                player.skills.hitpoints.level = level;
                            }
                            if let Some(level) = combat_skill_level {
                                player.skills.combat.level = level;
                            }
                            // Update hair
                            player.hair_style = hair_style;
                            player.hair_color = hair_color;
                            // Update equipment
                            player.equipped_head = equipped_head.clone();
                            player.equipped_body = equipped_body.clone();
                            player.equipped_weapon = equipped_weapon.clone();
                            player.equipped_back = equipped_back.clone();
                            player.equipped_feet = equipped_feet.clone();
                            player.equipped_ring = equipped_ring.clone();
                            player.equipped_gloves = equipped_gloves.clone();
                            player.equipped_necklace = equipped_necklace.clone();
                            player.equipped_belt = equipped_belt.clone();
                            // Update admin status
                            player.is_admin = is_admin;
                            // Update stall status
                            player.has_stall = has_stall;
                            player.stall_name = stall_name.clone();
                            // Update sitting state
                            let sitting = extract_bool(player_value, "sitting").unwrap_or(false);
                            if sitting
                                && player.animation.state
                                    != crate::render::animation::AnimationState::SittingChair
                            {
                                // Snap position immediately so remote players don't "walk" to the chair
                                if !is_local_player {
                                    player.x = player.target_x;
                                    player.y = player.target_y;
                                    player.server_x = player.target_x;
                                    player.server_y = player.target_y;
                                }
                                // Force direction to match chair direction from server
                                // (bypass the face command grace period when sitting)
                                if let Some(d) = direction {
                                    let chair_dir = Direction::from_u8(d as u8);
                                    player.direction = chair_dir;
                                    player.animation.direction = chair_dir;
                                }
                                player.sit_chair();
                                if is_local_player {
                                    state.is_sitting = true;
                                }
                            } else if !sitting
                                && player.animation.state
                                    == crate::render::animation::AnimationState::SittingChair
                            {
                                player.stand_up();
                                if is_local_player {
                                    state.is_sitting = false;
                                }
                            }

                            // Update gathering state (for players who started fishing before we logged in)
                            let server_is_gathering =
                                extract_bool(player_value, "is_gathering").unwrap_or(false);
                            if server_is_gathering && !player.is_gathering {
                                player.is_gathering = true;
                                player.gathering_started_at = macroquad::time::get_time();
                                // Don't play attack animation - they're already in gathering pose
                                if is_local_player {
                                    state.is_gathering = true;
                                    state.gathering_started_at = macroquad::time::get_time();
                                }
                            } else if !server_is_gathering && player.is_gathering {
                                // Don't reset if gathering was started very recently (grace period to avoid race condition)
                                let recently_started =
                                    macroquad::time::get_time() - player.gathering_started_at < 1.0;
                                if !recently_started {
                                    player.is_gathering = false;
                                    if is_local_player {
                                        state.is_gathering = false;
                                    }
                                }
                            }
                        } else if state.local_player_id.as_ref() != Some(&id) && !id.is_empty() {
                            // Player not in our map - create them from stateSync data
                            // This handles players re-appearing after map transitions
                            if let (Some(px), Some(py)) = (x, y) {
                                log::info!(
                                    "Creating player from stateSync: {} at ({}, {})",
                                    name,
                                    px,
                                    py
                                );
                                let mut new_player = Player::new(
                                    id.clone(),
                                    name.clone(),
                                    px as f32,
                                    py as f32,
                                    gender,
                                    skin,
                                );
                                new_player.hair_style = hair_style;
                                new_player.hair_color = hair_color;
                                new_player.equipped_head = equipped_head;
                                new_player.equipped_body = equipped_body;
                                new_player.equipped_weapon = equipped_weapon;
                                new_player.equipped_back = equipped_back;
                                new_player.equipped_feet = equipped_feet;
                                new_player.equipped_ring = equipped_ring;
                                new_player.equipped_gloves = equipped_gloves;
                                new_player.equipped_necklace = equipped_necklace;
                                new_player.equipped_belt = equipped_belt;
                                new_player.is_admin = is_admin;
                                new_player.has_stall = has_stall;
                                new_player.stall_name = stall_name;
                                let sitting =
                                    extract_bool(player_value, "sitting").unwrap_or(false);
                                if sitting {
                                    new_player.sit_chair();
                                }
                                let is_gathering =
                                    extract_bool(player_value, "is_gathering").unwrap_or(false);
                                if is_gathering {
                                    new_player.is_gathering = true;
                                    new_player.gathering_started_at = macroquad::time::get_time();
                                }
                                let is_woodcutting =
                                    extract_bool(player_value, "is_woodcutting").unwrap_or(false);
                                if is_woodcutting {
                                    new_player.is_woodcutting = true;
                                    new_player.woodcutting_started_at = macroquad::time::get_time();
                                }
                                let is_mining =
                                    extract_bool(player_value, "is_mining").unwrap_or(false);
                                if is_mining {
                                    new_player.is_mining = true;
                                    new_player.mining_started_at = macroquad::time::get_time();
                                }
                                let dashing =
                                    extract_bool(player_value, "dashing").unwrap_or(false);
                                if dashing {
                                    new_player.is_dashing = true;
                                }
                                if let Some(hp_val) = hp {
                                    new_player.hp = hp_val;
                                }
                                if let Some(max_hp_val) = max_hp {
                                    new_player.max_hp = max_hp_val;
                                }
                                if let Some(mp_val) = mp {
                                    new_player.mp = mp_val;
                                }
                                if let Some(max_mp_val) = max_mp {
                                    new_player.max_mp = max_mp_val;
                                }
                                if let Some(dir) = direction {
                                    let new_dir = Direction::from_u8(dir as u8);
                                    new_player.direction = new_dir;
                                    new_player.animation.direction = new_dir;
                                }
                                state.players.insert(id.clone(), new_player);
                            }
                        }

                        // Update inventory gold for local player
                        if state.local_player_id.as_ref() == Some(&id) {
                            if let Some(gold) = gold {
                                state.inventory.gold = gold;
                            }
                        }
                    }
                }

                // Reconcile: remove players who are no longer in this StateSync.
                // In instances, the server sends ALL players in the group, so anyone
                // missing has left. This prevents ghost players from lingering when a
                // PlayerLeft message races with a StateSync that still included them.
                // Only reconcile on full syncs (delta syncs use explicit removal lists).
                if is_full_sync && !sync_instance.is_empty() {
                    let local_id = state.local_player_id.clone().unwrap_or_default();
                    state
                        .players
                        .retain(|id, _| *id == local_id || synced_player_ids.contains(id));
                }

                // Delta sync: process explicit removal lists
                if !is_full_sync {
                    if let Some(removed) = extract_array(value, "removedPlayers") {
                        for rv in removed {
                            if let Some(id) = rv.as_str() {
                                if state.local_player_id.as_deref() != Some(id) {
                                    state.players.remove(id);
                                }
                            }
                        }
                    }
                }

                // Check if local player walked onto a portal (auto-trigger)
                // Uses server-authoritative position (not interpolated visual position)
                // to detect portals immediately when the server confirms the move
                if let Some(local_id) = &state.local_player_id {
                    if let Some(player) = state.players.get(local_id) {
                        let current_tile = (
                            player.server_x.floor() as i32,
                            player.server_y.floor() as i32,
                        );
                        let prev_tile = state.last_portal_check_pos;

                        // Only check for portal if we moved to a different tile
                        let moved_tiles = prev_tile.map_or(false, |prev| prev != current_tile);

                        // Clear the ignored portal tile once the player steps off it
                        if moved_tiles {
                            if let Some(ignored) = state.portal_ignore_tile {
                                if current_tile != ignored {
                                    state.portal_ignore_tile = None;
                                }
                            }
                        }

                        if moved_tiles
                            && state.pending_portal_id.is_none()
                            && state
                                .portal_ignore_tile
                                .map_or(true, |ignored| current_tile != ignored)
                            && matches!(state.map_transition.state, TransitionState::None)
                        {
                            if let Some(portal) = state
                                .chunk_manager
                                .get_portal_at(player.server_x, player.server_y)
                            {
                                state.pending_portal_id = Some(portal.id.clone());
                            }
                        }

                        // Always update last checked position
                        state.last_portal_check_pos = Some(current_tile);
                    }
                }

                // Push player regen events as healing numbers (negative damage = green +X)
                let current_time = macroquad::time::get_time();
                for (target_id, x, y, heal_amount) in player_regen_events {
                    state.damage_events.push(DamageEvent {
                        x,
                        y,
                        damage: -heal_amount, // Negative = healing
                        time: current_time,
                        target_id,
                        source_id: None,
                        projectile: None,
                    });
                }

                // Update NPCs (grid positions from server, converted to f32 for interpolation)
                let mut npc_regen_events: Vec<(String, f32, f32, i32)> = Vec::new();
                if let Some(npcs) = extract_array(value, "npcs") {
                    for npc_value in npcs {
                        let id = extract_string(npc_value, "id").unwrap_or_default();
                        let npc_type = extract_u8(npc_value, "npc_type").unwrap_or(0);
                        let entity_type = extract_string(npc_value, "entity_type")
                            .unwrap_or_else(|| "pig".to_string());
                        let display_name = extract_string(npc_value, "display_name")
                            .unwrap_or_else(|| "???".to_string());
                        // Server sends i32 grid positions
                        let x = extract_i32(npc_value, "x").unwrap_or(0) as f32;
                        let y = extract_i32(npc_value, "y").unwrap_or(0) as f32;
                        let direction = extract_u8(npc_value, "direction").unwrap_or(0);
                        let hp = extract_i32(npc_value, "hp").unwrap_or(50);
                        let max_hp = extract_i32(npc_value, "max_hp").unwrap_or(50);
                        let level = extract_i32(npc_value, "level").unwrap_or(1);
                        let npc_state = extract_u8(npc_value, "state").unwrap_or(0);
                        let hostile = extract_bool(npc_value, "hostile").unwrap_or(true);
                        let is_quest_giver =
                            extract_bool(npc_value, "is_quest_giver").unwrap_or(false);
                        let can_turn_in_quest =
                            extract_bool(npc_value, "can_turn_in_quest").unwrap_or(false);
                        let is_merchant = extract_bool(npc_value, "is_merchant").unwrap_or(false);
                        let is_altar = extract_bool(npc_value, "is_altar").unwrap_or(false);
                        let is_banker = extract_bool(npc_value, "is_banker").unwrap_or(false);
                        let is_slayer_master = extract_bool(npc_value, "is_slayer_master").unwrap_or(false);
                        let is_friendly = extract_bool(npc_value, "is_friendly").unwrap_or(false);
                        let station_type = extract_string(npc_value, "station_type");
                        let move_speed = extract_f32(npc_value, "move_speed").unwrap_or(2.0);
                        let no_shadow = extract_bool(npc_value, "no_shadow").unwrap_or(false);
                        let render_offset_y =
                            extract_f32(npc_value, "render_offset_y").unwrap_or(0.0);

                        if let Some(npc) = state.npcs.get_mut(&id) {
                            // Update existing NPC - interpolate toward new grid position
                            npc.set_server_position(x, y);
                            npc.direction = Direction::from_u8(direction);
                            // Update last_damage_time if HP decreased (ensures HP bar shows)
                            if hp < npc.hp {
                                npc.last_damage_time = macroquad::time::get_time();
                            } else if hp > npc.hp && npc.hp > 0 {
                                // HP increased (regen) - record for floating text
                                let heal_amount = hp - npc.hp;
                                npc_regen_events.push((id.clone(), npc.x, npc.y, heal_amount));
                            }
                            npc.hp = hp;
                            npc.max_hp = max_hp;
                            npc.level = level;
                            // Handle state transitions
                            let new_state = NpcState::from_u8(npc_state);
                            if new_state != NpcState::Dead {
                                // NPC is alive - clear death state if it was dying
                                npc.death_timer = None;
                                npc.pending_death = false;
                                npc.state = new_state;
                            } else if npc.death_timer.is_none() && !npc.pending_death {
                                // Server says dead, start death sequence if not already
                                npc.start_death();
                            }
                            // If death_timer is Some and new_state is Dead, let animation continue
                            // Update display name in case it changed
                            npc.display_name = display_name;
                            npc.hostile = hostile;
                            npc.is_quest_giver = is_quest_giver;
                            npc.can_turn_in_quest = can_turn_in_quest;
                            npc.is_merchant = is_merchant;
                            npc.is_altar = is_altar;
                            npc.is_banker = is_banker;
                            npc.is_slayer_master = is_slayer_master;
                            npc.is_friendly = is_friendly;
                            npc.station_type = station_type;
                            npc.move_speed = move_speed;
                            npc.no_shadow = no_shadow;
                            npc.render_offset_y = render_offset_y;
                        } else {
                            // New NPC - add to state
                            let mut npc = Npc::new(id.clone(), entity_type, x, y);
                            npc.display_name = display_name;
                            npc.direction = Direction::from_u8(direction);
                            npc.hp = hp;
                            npc.max_hp = max_hp;
                            npc.level = level;
                            npc.state = NpcState::from_u8(npc_state);
                            npc.hostile = hostile;
                            npc.is_quest_giver = is_quest_giver;
                            npc.can_turn_in_quest = can_turn_in_quest;
                            npc.is_merchant = is_merchant;
                            npc.is_altar = is_altar;
                            npc.is_banker = is_banker;
                            npc.is_slayer_master = is_slayer_master;
                            npc.is_friendly = is_friendly;
                            npc.station_type = station_type;
                            npc.move_speed = move_speed;
                            npc.no_shadow = no_shadow;
                            npc.render_offset_y = render_offset_y;
                            state.npcs.insert(id, npc);
                        }
                    }
                }

                // Delta sync: process explicit NPC removal list
                if !is_full_sync {
                    if let Some(removed) = extract_array(value, "removedNpcs") {
                        for rv in removed {
                            if let Some(id) = rv.as_str() {
                                state.npcs.remove(id);
                            }
                        }
                    }
                }

                // Push NPC regen events as healing numbers (negative damage = green +X)
                for (target_id, x, y, heal_amount) in npc_regen_events {
                    state.damage_events.push(DamageEvent {
                        x,
                        y,
                        damage: -heal_amount, // Negative = healing
                        time: current_time,
                        target_id,
                        source_id: None,
                        projectile: None,
                    });
                }
            }
        }

        "chatMessage" => {
            if let Some(value) = data {
                let sender_name = extract_string(value, "senderName").unwrap_or_default();
                let text = extract_string(value, "text").unwrap_or_default();
                let extracted_ts = extract_u64(value, "timestamp").unwrap_or(0) as f64;
                let timestamp = if extracted_ts > 0.0 {
                    extracted_ts
                } else {
                    macroquad::time::get_time()
                };
                let channel_str = extract_string(value, "channel").unwrap_or_default();

                let channel = match channel_str.as_str() {
                    "global" => ChatChannel::Global,
                    "system" => ChatChannel::System,
                    _ => ChatChannel::Local, // "public" or unknown defaults to Local
                };

                // Add to chat log
                state.push_chat_message(ChatMessage {
                    sender_name: sender_name.clone(),
                    text: text.clone(),
                    timestamp,
                    channel,
                });
                state.pending_sfx.push("message_add".to_string());

                // Chat bubbles only for public/nearby messages
                if matches!(channel, ChatChannel::Local) {
                    if let Some((player_id, _)) =
                        state.players.iter().find(|(_, p)| p.name == sender_name)
                    {
                        let player_id = player_id.clone();
                        state.chat_bubbles.retain(|b| b.player_id != player_id);
                        state.chat_bubbles.push(ChatBubble {
                            player_id,
                            text,
                            time: macroquad::time::get_time(),
                        });
                    }
                }
            }
        }

        "npcSpeech" => {
            if let Some(value) = data {
                let npc_id = extract_string(value, "npcId").unwrap_or_default();
                let message = extract_string(value, "message").unwrap_or_default();

                if let Some(npc) = state.npcs.get_mut(&npc_id) {
                    npc.speech_bubble = Some((message, macroquad::time::get_time()));
                }
            }
        }

        "targetChanged" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let target_id = extract_string(value, "target_id");

                // Update local selection if this is our player
                if state.local_player_id.as_ref() == Some(&player_id) {
                    state.selected_entity_id = target_id.clone();
                    log::debug!("Target changed to: {:?}", state.selected_entity_id);
                }
            }
        }

        "playerAttack" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let attack_type =
                    extract_string(value, "attack_type").unwrap_or_else(|| "melee".to_string());

                let is_local = state.local_player_id.as_ref() == Some(&player_id);

                // Check if already in attack animation BEFORE calling play_attack.
                // Manual attacks play animation+sound locally; auto-attacks rely on
                // this server event. set_state is idempotent (no-op if already in
                // same state), so always triggering the animation is safe.
                let already_attacking = is_local && state.players.get(&player_id)
                    .map_or(false, |p| matches!(p.animation.state,
                        crate::render::animation::AnimationState::Attacking
                        | crate::render::animation::AnimationState::ShootingBow
                        | crate::render::animation::AnimationState::Casting));

                if let Some(player) = state.players.get_mut(&player_id) {
                    match attack_type.as_str() {
                        "ranged" => player.play_shoot_bow(),
                        "spell" => player.play_cast(),
                        _ => player.play_attack(),
                    }
                }

                // Play attack sound for server-driven attacks (auto-attacks).
                // Manual attacks already played the sound locally, so skip if the
                // player was already mid-animation when this event arrived.
                if is_local && !already_attacking {
                    if let Some(player) = state.players.get(&player_id) {
                        let sound_type = if attack_type == "ranged" {
                            crate::game::state::AttackSoundType::Ranged
                        } else if player.equipped_weapon.is_some() {
                            crate::game::state::AttackSoundType::Melee
                        } else {
                            crate::game::state::AttackSoundType::Unarmed
                        };
                        state.pending_attack_sounds.push(sound_type);
                    }
                }
            }
        }

        "damageEvent" => {
            if let Some(value) = data {
                let source_id = extract_string(value, "source_id");
                let target_id = extract_string(value, "target_id").unwrap_or_default();
                let damage = extract_i32(value, "damage").unwrap_or(0);
                let target_hp = extract_i32(value, "target_hp").unwrap_or(0);
                let target_x = extract_f32(value, "target_x").unwrap_or(0.0);
                let target_y = extract_f32(value, "target_y").unwrap_or(0.0);
                let projectile = extract_string(value, "projectile");

                log::debug!(
                    "Damage event: {} took {} damage from {:?} (HP: {})",
                    target_id,
                    damage,
                    source_id,
                    target_hp
                );

                // Trigger attack animation for NPCs (players use playerAttack event)
                if let Some(ref src_id) = source_id {
                    if let Some(npc) = state.npcs.get_mut(src_id) {
                        npc.trigger_attack_animation();
                    }
                }

                // Update last damage time (could be player or NPC)
                // NOTE: We intentionally do NOT update hp here. The StateSync snapshot
                // is taken BEFORE combat in the tick loop, so it contains stale pre-damage HP.
                // If we set hp = target_hp here, the subsequent stale StateSync would see
                // hp_from_sync > entity.hp and falsely detect regen (showing green +X numbers).
                // Letting StateSync be the sole authority for HP state avoids this race.
                let current_time = macroquad::time::get_time();
                if let Some(player) = state.players.get_mut(&target_id) {
                    player.last_damage_time = current_time;
                } else if let Some(npc) = state.npcs.get_mut(&target_id) {
                    npc.last_damage_time = current_time;
                }

                // Play hit sound when our player gets attacked (including misses)
                if state.local_player_id.as_deref() == Some(&target_id) {
                    state.pending_sfx.push("unarmed".to_string());
                }

                // Create floating damage number with target_id for height lookup at render time
                state.damage_events.push(DamageEvent {
                    x: target_x,
                    y: target_y,
                    damage,
                    time: macroquad::time::get_time(),
                    target_id,
                    source_id: source_id.clone(),
                    projectile: projectile.clone(),
                });

                // Spawn projectile for ranged attacks
                if let Some(ref projectile_type) = projectile {
                    if let Some(ref source_id) = source_id {
                        // Get source tile center (rounded to ensure straight isometric lines)
                        let source_pos = if let Some(player) = state.players.get(source_id) {
                            Some((player.x.round(), player.y.round()))
                        } else {
                            None
                        };

                        if let Some((src_x, src_y)) = source_pos {
                            // Target tile center (rounded for straight isometric lines)
                            let end_x = target_x.round();
                            let end_y = target_y.round();

                            state.projectiles.push(crate::game::Projectile {
                                sprite: projectile_type.clone(),
                                start_x: src_x,
                                start_y: src_y,
                                end_x,
                                end_y,
                                start_time: current_time,
                                duration: 0.15, // Fast arrow travel
                            });
                        }
                    }
                }
            }
        }

        "npcDied" => {
            if let Some(value) = data {
                let npc_id = extract_string(value, "npc_id").unwrap_or_default();
                log::debug!("NPC died: {}", npc_id);

                if let Some(npc) = state.npcs.get_mut(&npc_id) {
                    npc.start_death();
                }

                // Clear selection if we had this NPC targeted
                if state.selected_entity_id.as_ref() == Some(&npc_id) {
                    state.selected_entity_id = None;
                }

                // Close shop if this NPC was the merchant
                if let Some(shop_npc_id) = &state.ui_state.shop_npc_id {
                    if shop_npc_id == &npc_id {
                        state.ui_state.crafting_open = false;
                        state.ui_state.shop_data = None;
                    }
                }
            }
        }

        "npcRespawned" => {
            if let Some(value) = data {
                let npc_id = extract_string(value, "id").unwrap_or_default();
                // Server sends i32 grid positions
                let x = extract_i32(value, "x").unwrap_or(0) as f32;
                let y = extract_i32(value, "y").unwrap_or(0) as f32;
                let hp = extract_i32(value, "hp").unwrap_or(50);
                log::debug!("NPC respawned: {} at ({}, {})", npc_id, x, y);

                if let Some(npc) = state.npcs.get_mut(&npc_id) {
                    npc.state = NpcState::Idle;
                    npc.hp = hp;
                    npc.max_hp = hp;
                    npc.x = x;
                    npc.y = y;
                    npc.target_x = x;
                    npc.target_y = y;
                }
            }
        }

        "playerDied" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "id").unwrap_or_default();
                let killer_id = extract_string(value, "killer_id").unwrap_or_default();
                log::info!("Player {} was killed by {}", player_id, killer_id);

                if let Some(player) = state.players.get_mut(&player_id) {
                    player.die();
                }

                // Local player death: clear combat/movement state
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.pending_sfx.push("death".to_string());
                    state.auto_action_state = None;
                    state.auto_path = None;
                    state.follow_target = None;
                    state.follow_arrived_target_pos = None;
                    state.follow_target_move_time = 0.0;
                }

                // Clear selection if we had this player targeted
                if state.selected_entity_id.as_ref() == Some(&player_id) {
                    state.selected_entity_id = None;
                }
            }
        }

        "playerRespawned" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "id").unwrap_or_default();
                // Server sends i32 grid positions
                let x = extract_i32(value, "x").unwrap_or(0) as f32;
                let y = extract_i32(value, "y").unwrap_or(0) as f32;
                let hp = extract_i32(value, "hp").unwrap_or(100);
                log::info!("Player {} respawned at ({}, {})", player_id, x, y);

                if let Some(player) = state.players.get_mut(&player_id) {
                    player.respawn(x, y, hp);
                }

                // Reset local player state on respawn
                if state.local_player_id.as_ref() == Some(&player_id) {
                    state.is_sitting = false;
                    // Clear stale pending move sequences so interpolation starts clean
                    state.clear_pending_moves();
                    // Defensively clear interior mode in case we died in an instance
                    // (mapTransition should also handle this, but belt-and-suspenders)
                    if state.chunk_manager.is_interior() {
                        state.chunk_manager.clear_interior();
                        state.current_interior = None;
                        state.current_instance = None;
                        state.npcs.clear();
                        state.ground_items.clear();
                    }
                }
            }
        }

        "attackResult" => {
            if let Some(value) = data {
                let success = value
                    .as_map()
                    .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("success")))
                    .and_then(|(_, v)| v.as_bool())
                    .unwrap_or(false);
                let reason = extract_string(value, "reason");

                if !success {
                    if let Some(reason) = reason {
                        log::debug!("Attack failed: {}", reason);
                    }
                }
            }
        }

        "skillXp" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let skill_name = extract_string(value, "skill").unwrap_or_default();
                let xp_gained = extract_i32(value, "xp_gained").unwrap_or(0) as i64;
                let total_xp = extract_i32(value, "total_xp").unwrap_or(0) as i64;
                let level = extract_i32(value, "level").unwrap_or(1);

                log::debug!(
                    "Player {} gained {} {} XP (total: {}, level: {})",
                    player_id,
                    xp_gained,
                    skill_name,
                    total_xp,
                    level
                );

                if let Some(player) = state.players.get_mut(&player_id) {
                    // Update the specific skill
                    if let Some(skill_type) = SkillType::from_str(&skill_name) {
                        let skill = player.skills.get_mut(skill_type);
                        skill.xp = total_xp;
                        skill.level = level;

                        // Update max_hp if hitpoints changed
                        if skill_type == SkillType::Hitpoints {
                            player.max_hp = level;
                        }
                    }

                    // Create floating XP event and system message for local player
                    if state.local_player_id.as_ref() == Some(&player_id) {
                        // Add system chat message (system-only, no Local mirror — too spammy)
                        state.ui_state.chat_messages.push_system_only(ChatMessage::system(format!(
                                "+{} {} XP",
                                xp_gained, skill_name
                            )));
                        state.ui_state.chat_revision = state.ui_state.chat_revision.wrapping_add(1);

                        state.skill_xp_events.push(SkillXpEvent {
                            x: player.x,
                            y: player.y,
                            skill: skill_name.clone(),
                            xp_gained,
                            time: macroquad::time::get_time(),
                        });

                        // Update XP globes and drop feed
                        if let Some(skill_type) = SkillType::from_str(&skill_name) {
                            let xp_for_next = crate::game::skills::total_xp_for_level(level + 1);
                            state
                                .xp_globes
                                .on_xp_gain(skill_type, total_xp, xp_for_next, level);
                            state.xp_drop_feed.push(skill_type, xp_gained);
                        }
                    }
                }
            }
        }

        "skillsSync" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                log::info!("skillsSync received for player_id: {}, local_player_id: {:?}, players in state: {:?}",
                    player_id, state.local_player_id, state.players.keys().collect::<Vec<_>>());

                // Only update skills for the local player
                if state.local_player_id.as_ref() == Some(&player_id) {
                    if let Some(player) = state.players.get_mut(&player_id) {
                        // Update all skills
                        if let Some(level) = extract_i32(value, "hitpoints_level") {
                            player.skills.hitpoints.level = level;
                            player.max_hp = level;
                        }
                        if let Some(xp) = extract_i32(value, "hitpoints_xp") {
                            player.skills.hitpoints.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "combat_level") {
                            player.skills.combat.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "combat_xp") {
                            player.skills.combat.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "fishing_level") {
                            player.skills.fishing.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "fishing_xp") {
                            player.skills.fishing.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "farming_level") {
                            player.skills.farming.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "farming_xp") {
                            player.skills.farming.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "smithing_level") {
                            player.skills.smithing.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "smithing_xp") {
                            player.skills.smithing.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "prayer_level") {
                            player.skills.prayer.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "prayer_xp") {
                            player.skills.prayer.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "magic_level") {
                            player.skills.magic.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "magic_xp") {
                            player.skills.magic.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "woodcutting_level") {
                            player.skills.woodcutting.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "woodcutting_xp") {
                            player.skills.woodcutting.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "alchemy_level") {
                            player.skills.alchemy.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "alchemy_xp") {
                            player.skills.alchemy.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "mining_level") {
                            player.skills.mining.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "mining_xp") {
                            player.skills.mining.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "slayer_level") {
                            player.skills.slayer.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "slayer_xp") {
                            player.skills.slayer.xp = xp as i64;
                        }
                        if let Some(level) = extract_i32(value, "survivalist_level") {
                            player.skills.survivalist.level = level;
                        }
                        if let Some(xp) = extract_i32(value, "survivalist_xp") {
                            player.skills.survivalist.xp = xp as i64;
                        }

                        log::info!("Skills synced for player {}: HP {}, Combat {}, Fishing {}, Farming {}, Smithing {}, Prayer {}, Magic {}, Woodcutting {}, Alchemy {}, Mining {}, Slayer {}, Survivalist {}",
                            player_id,
                            player.skills.hitpoints.level,
                            player.skills.combat.level,
                            player.skills.fishing.level,
                            player.skills.farming.level,
                            player.skills.smithing.level,
                            player.skills.prayer.level,
                            player.skills.magic.level,
                            player.skills.woodcutting.level,
                            player.skills.alchemy.level,
                            player.skills.mining.level,
                            player.skills.slayer.level,
                            player.skills.survivalist.level
                        );
                    } else {
                        log::warn!(
                            "skillsSync: player {} not found in state.players",
                            player_id
                        );
                    }
                } else {
                    log::warn!(
                        "skillsSync: player_id {} doesn't match local_player_id {:?}",
                        player_id,
                        state.local_player_id
                    );
                }
            }
        }

        "skillLevelUp" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let skill_name = extract_string(value, "skill").unwrap_or_default();
                let new_level = extract_i32(value, "new_level").unwrap_or(1);

                log::info!(
                    "Player {} leveled up {} to {}!",
                    player_id,
                    skill_name,
                    new_level
                );

                // Get player position for floating text
                if let Some(player) = state.players.get_mut(&player_id) {
                    // Update the specific skill level
                    if let Some(skill_type) = SkillType::from_str(&skill_name) {
                        let skill = player.skills.get_mut(skill_type);
                        skill.level = new_level;

                        // Update max_hp and current HP if hitpoints leveled up
                        if skill_type == SkillType::Hitpoints {
                            let old_max = player.max_hp;
                            player.max_hp = new_level;
                            // Heal the difference (new HP from the level)
                            player.hp += new_level - old_max;
                        }
                    }

                    // Create floating level up event and system message for local player
                    if state.local_player_id.as_ref() == Some(&player_id) {
                        state.ui_state.chat_messages.push(ChatMessage::system(format!(
                                "{} leveled up to {}!",
                                skill_name, new_level
                            )));
                        state.ui_state.chat_revision = state.ui_state.chat_revision.wrapping_add(1);
                        state.pending_sfx.push("level_up".to_string());
                    }

                    let now = macroquad::time::get_time();
                    let px = player.x;
                    let py = player.y;

                    state.level_up_events.push(LevelUpEvent {
                        x: px,
                        y: py,
                        skill: skill_name,
                        new_level,
                        time: now,
                    });
                }
            }
        }

        "itemDropped" => {
            if let Some(value) = data {
                let id = extract_string(value, "id").unwrap_or_default();
                let item_id =
                    extract_string(value, "item_id").unwrap_or_else(|| "unknown".to_string());
                let x = extract_f32(value, "x").unwrap_or(0.0);
                let y = extract_f32(value, "y").unwrap_or(0.0);
                let quantity = extract_i32(value, "quantity").unwrap_or(1);

                log::debug!("Item dropped: {} ({}) at ({}, {})", id, item_id, x, y);

                let item = if item_id == "gold" {
                    GroundItem::new_gold(id.clone(), x, y, quantity)
                } else {
                    GroundItem::new(id.clone(), item_id, x, y, quantity)
                };

                // Check if there's a dying NPC near this drop location
                let near_dying_npc = state.npcs.values().any(|npc| {
                    let dx = npc.x - x;
                    let dy = npc.y - y;
                    let dist_sq = dx * dx + dy * dy;
                    npc.is_dying() && dist_sq < 2.0 // Within ~1.4 tiles
                });

                if near_dying_npc {
                    // Delay item appearance by 0.6s to let death animation complete
                    let spawn_time = macroquad::time::get_time() + 0.6;
                    state.pending_ground_items.push((item, spawn_time));
                } else {
                    // Spawn immediately (player drop, etc.)
                    state.ground_items.insert(id, item);
                }
            }
        }

        "itemPickedUp" => {
            if let Some(value) = data {
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                let player_id = extract_string(value, "player_id").unwrap_or_default();

                log::debug!("Item {} picked up by {}", item_id, player_id);
                state.ground_items.remove(&item_id);
            }
        }

        "itemDespawned" => {
            if let Some(value) = data {
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                log::debug!("Item {} despawned", item_id);
                state.ground_items.remove(&item_id);
            }
        }

        "itemQuantityUpdated" => {
            if let Some(value) = data {
                let id = extract_string(value, "id").unwrap_or_default();
                let quantity = extract_i32(value, "quantity").unwrap_or(1);

                log::debug!("Item {} quantity updated to {}", id, quantity);

                if let Some(item) = state.ground_items.get_mut(&id) {
                    item.quantity = quantity;
                    // Regenerate gold pile with new quantity
                    if item.item_id == "gold" {
                        item.gold_pile = Some(crate::game::item::GoldPileState::new(
                            quantity,
                            macroquad::time::get_time(),
                        ));
                    }
                }
            }
        }

        "inventoryUpdate" => {
            // Server sends this only to the owning player (unicast)
            if let Some(value) = data {
                // Clear current inventory
                for slot in state.inventory.slots.iter_mut() {
                    *slot = None;
                }

                // Update slots
                if let Some(slots) = extract_array(value, "slots") {
                    for slot_value in slots {
                        let slot_idx = extract_u8(slot_value, "slot").unwrap_or(0) as usize;
                        let item_id = extract_string(slot_value, "item_id").unwrap_or_default();
                        let quantity = extract_i32(slot_value, "quantity").unwrap_or(0);

                        if slot_idx < state.inventory.slots.len()
                            && !item_id.is_empty()
                            && quantity > 0
                        {
                            state.inventory.slots[slot_idx] =
                                Some(InventorySlot::new(item_id, quantity));
                        }
                    }
                }

                // Update gold
                if let Some(gold) = extract_i32(value, "gold") {
                    state.inventory.gold = gold;
                }

                log::debug!(
                    "Inventory updated: {} gold, {} items",
                    state.inventory.gold,
                    state.inventory.slots.iter().filter(|s| s.is_some()).count()
                );
            }
        }

        "itemUsed" => {
            // Server sends this only to the owning player (unicast)
            if let Some(value) = data {
                let slot = extract_u8(value, "slot").unwrap_or(0);
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                let effect = extract_string(value, "effect").unwrap_or_default();
                log::debug!(
                    "Item used: slot {} item {} effect {}",
                    slot,
                    item_id,
                    effect
                );
            }
        }

        "chunkData" => {
            if let Some(value) = data {
                let chunk_x = extract_i32(value, "chunkX").unwrap_or(0);
                let chunk_y = extract_i32(value, "chunkY").unwrap_or(0);

                // Parse layers array
                let mut layers: Vec<(u8, Vec<u32>)> = Vec::new();
                if let Some(layers_arr) = extract_array(value, "layers") {
                    for layer_value in layers_arr {
                        let layer_type = extract_u8(layer_value, "layerType").unwrap_or(0);
                        let tiles: Vec<u32> = extract_array(layer_value, "tiles")
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_u64().map(|u| u as u32))
                                    .collect()
                            })
                            .unwrap_or_default();
                        layers.push((layer_type, tiles));
                    }
                }

                // Parse collision bytes
                let collision: Vec<u8> = extract_array(value, "collision")
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_u64().map(|u| u as u8))
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse map objects
                let mut objects: Vec<MapObject> = Vec::new();
                if let Some(objects_arr) = extract_array(value, "objects") {
                    for obj_value in objects_arr {
                        let gid = obj_value["gid"].as_u64().unwrap_or(0) as u32;
                        let tile_x = obj_value["tileX"].as_i64().unwrap_or(0) as i32;
                        let tile_y = obj_value["tileY"].as_i64().unwrap_or(0) as i32;
                        let width = obj_value["width"].as_u64().unwrap_or(0) as u32;
                        let height = obj_value["height"].as_u64().unwrap_or(0) as u32;
                        objects.push(MapObject {
                            gid,
                            tile_x,
                            tile_y,
                            width,
                            height,
                        });
                    }
                }

                // Parse walls from server message
                let mut walls: Vec<Wall> = Vec::new();
                if let Some(walls_arr) = extract_array(value, "walls") {
                    for w in walls_arr {
                        let gid = w["gid"].as_u64().unwrap_or(0) as u32;
                        let tile_x = w["tileX"].as_i64().unwrap_or(0) as i32;
                        let tile_y = w["tileY"].as_i64().unwrap_or(0) as i32;
                        let edge_str = w["edge"].as_str().unwrap_or("down");
                        let edge = match edge_str {
                            "right" => WallEdge::Right,
                            _ => WallEdge::Down,
                        };
                        walls.push(Wall {
                            gid,
                            tile_x,
                            tile_y,
                            edge,
                        });
                    }
                }

                // Parse portals from server message
                let mut portals: Vec<Portal> = Vec::new();
                if let Some(portals_arr) = extract_array(value, "portals") {
                    for p in portals_arr {
                        let id = extract_string(p, "id").unwrap_or_default();
                        let x = extract_i32(p, "x").unwrap_or(0);
                        let y = extract_i32(p, "y").unwrap_or(0);
                        let width = extract_i32(p, "width").unwrap_or(1);
                        let height = extract_i32(p, "height").unwrap_or(1);
                        let target_map = extract_string(p, "targetMap").unwrap_or_default();
                        let target_spawn = extract_string(p, "targetSpawn").unwrap_or_default();
                        portals.push(Portal {
                            id,
                            x,
                            y,
                            width,
                            height,
                            target_map,
                            target_spawn,
                        });
                    }
                }

                log::debug!("Received chunk data: ({}, {}) with {} layers, {} collision bytes, {} objects, {} walls, {} portals",
                    chunk_x, chunk_y, layers.len(), collision.len(), objects.len(), walls.len(), portals.len());

                state.chunk_manager.load_chunk(
                    chunk_x, chunk_y, layers, &collision, objects, walls, portals,
                );
            }
        }

        "chunkNotFound" => {
            if let Some(value) = data {
                let chunk_x = extract_i32(value, "chunkX").unwrap_or(0);
                let chunk_y = extract_i32(value, "chunkY").unwrap_or(0);
                log::warn!("Chunk not found: ({}, {})", chunk_x, chunk_y);
            }
        }

        // ========== Quest System Messages ==========
        "showDialogue" => {
            if let Some(value) = data {
                let quest_id = extract_string(value, "quest_id").unwrap_or_default();
                let npc_id = extract_string(value, "npc_id").unwrap_or_default();
                let mut speaker = extract_string(value, "speaker").unwrap_or_default();
                let text = extract_string(value, "text").unwrap_or_default();

                // If the NPC is an adventurer guide, always use its canonical
                // speaker name so the custom guide UI renders instead of the
                // generic dialogue box.
                if let Some(npc) = state.npcs.get(&npc_id) {
                    if npc.entity_type == "adventurer_guide" {
                        speaker = "Adventurer Guide".to_string();
                    }
                }

                // Parse choices array
                let mut choices = Vec::new();
                if let Some(choices_arr) = extract_array(value, "choices") {
                    for choice_value in choices_arr {
                        let id = extract_string(choice_value, "id").unwrap_or_default();
                        let choice_text = extract_string(choice_value, "text").unwrap_or_default();
                        choices.push(DialogueChoice {
                            id,
                            text: choice_text,
                        });
                    }
                }

                // If Old Thomas's tutorial dialogue but tutorial is already done,
                // show a friendly post-tutorial greeting instead
                let (quest_id, text, choices) = if quest_id == "__tutorial__"
                    && (crate::settings::load_tutorial_completed()
                        || state.tutorial.as_ref().map_or(false, |t| t.is_done()))
                {
                    (
                        String::new(),
                        "Good to see you again, friend! You're doing great out there. Remember, the Adventurer Guide can help you find your next challenge!".to_string(),
                        vec![DialogueChoice {
                            id: "close".to_string(),
                            text: "Thanks, Old Thomas!".to_string(),
                        }],
                    )
                } else {
                    (quest_id, text, choices)
                };

                log::info!(
                    "Showing dialogue from {}: {} ({} choices)",
                    speaker,
                    text,
                    choices.len()
                );

                let already_open = state
                    .ui_state
                    .active_dialogue
                    .as_ref()
                    .map(|d| d.npc_id == npc_id)
                    .unwrap_or(false);

                state.ui_state.dialogue_scroll_offset = 0.0;
                state.ui_state.dialogue_touch_scroll_id = None;
                state.ui_state.dialogue_touch_dragged = false;
                state.ui_state.active_dialogue = Some(ActiveDialogue {
                    quest_id,
                    npc_id,
                    speaker,
                    text,
                    choices,
                    show_time: macroquad::time::get_time(),
                });

                // Keep the custom Adventurer Guide panel focused on the quest currently discussed.
                if let Some(dialogue) = &state.ui_state.active_dialogue {
                    if dialogue.speaker.eq_ignore_ascii_case("Adventurer Guide") {
                        let selected = match dialogue.quest_id.as_str() {
                            "adventurer_tier_1" => Some((0, 0)),
                            "adventurer_tier_2" => Some((0, 1)),
                            "adventurer_tier_3" => Some((0, 2)),
                            "skilling_tier_1" | "woodcutting_tier_1" | "fishing_tier_1"
                            | "alchemy_tier_1" => Some((1, 0)),
                            "skilling_tier_2" | "woodcutting_tier_2" | "fishing_tier_2"
                            | "alchemy_tier_2" => Some((1, 1)),
                            "skilling_tier_3" | "woodcutting_tier_3" | "fishing_tier_3"
                            | "alchemy_tier_3" => Some((1, 2)),
                            _ => None,
                        };
                        if let Some((tab_idx, tier_idx)) = selected {
                            state.ui_state.adventurer_selected_tab = tab_idx;
                            state.ui_state.adventurer_selected_tier = tier_idx;
                        }
                    }
                }

                if !already_open {
                    state.pending_sfx.push("ui_open".to_string());
                }
            }
        }

        "questAccepted" => {
            if let Some(value) = data {
                let quest_id = extract_string(value, "quest_id").unwrap_or_default();
                let quest_name = extract_string(value, "quest_name").unwrap_or_default();
                let accepted_id = quest_id.clone();

                // Parse objectives
                let mut objectives = Vec::new();
                if let Some(obj_arr) = extract_array(value, "objectives") {
                    for obj_value in obj_arr {
                        let id = extract_string(obj_value, "id").unwrap_or_default();
                        let description =
                            extract_string(obj_value, "description").unwrap_or_default();
                        let current = extract_i32(obj_value, "current").unwrap_or(0);
                        let target = extract_i32(obj_value, "target").unwrap_or(1);
                        objectives.push(QuestObjective {
                            id,
                            description,
                            current,
                            target,
                            completed: current >= target,
                        });
                    }
                }

                log::info!("Quest accepted: {} - {}", quest_id, quest_name);

                // Add to active quests (or update if exists)
                if let Some(existing) = state
                    .ui_state
                    .active_quests
                    .iter_mut()
                    .find(|q| q.id == quest_id)
                {
                    existing.objectives = objectives;
                } else {
                    state.ui_state.active_quests.push(ActiveQuest {
                        id: quest_id,
                        name: quest_name,
                        objectives,
                    });
                }
                state.ui_state.completed_quest_ids.remove(&accepted_id);

                // Don't close dialogue here - let user read the quest acceptance message
                // Dialogue will close when user presses continue or server sends dialogueClosed
            }
        }

        "questStateSync" => {
            if let Some(value) = data {
                state.ui_state.completed_quest_ids.clear();
                if let Some(ids) = extract_array(value, "completed_quest_ids") {
                    for id_value in ids {
                        if let Some(id) = id_value.as_str() {
                            state.ui_state.completed_quest_ids.insert(id.to_string());
                        }
                    }
                }
            }
        }

        "questCatalog" => {
            if let Some(value) = data {
                state.ui_state.quest_catalog.clear();
                if let Some(quests) = extract_array(value, "quests") {
                    for q in quests {
                        let quest_id = extract_string(q, "quest_id").unwrap_or_default();
                        let name = extract_string(q, "name").unwrap_or_default();
                        let description = extract_string(q, "description").unwrap_or_default();
                        let giver_npc_name = extract_string(q, "giver_npc_name").unwrap_or_default();
                        let level_required = extract_i32(q, "level_required").unwrap_or(0);
                        let required_quest_id = extract_string(q, "required_quest_id");
                        let required_quest_name = extract_string(q, "required_quest_name");
                        let mut objectives = Vec::new();
                        if let Some(obj_arr) = extract_array(q, "objectives") {
                            for obj in obj_arr {
                                let id = extract_string(obj, "id").unwrap_or_default();
                                let description = extract_string(obj, "description").unwrap_or_default();
                                let target = extract_i32(obj, "target").unwrap_or(1);
                                objectives.push(CatalogObjective { id, description, target });
                            }
                        }
                        state.ui_state.quest_catalog.push(QuestCatalogEntry {
                            quest_id,
                            name,
                            description,
                            giver_npc_name,
                            level_required,
                            required_quest_id,
                            required_quest_name,
                            objectives,
                        });
                    }
                }
                log::info!("Received quest catalog with {} quests", state.ui_state.quest_catalog.len());
            }
        }

        "questObjectiveProgress" => {
            if let Some(value) = data {
                let quest_id = extract_string(value, "quest_id").unwrap_or_default();
                let objective_id = extract_string(value, "objective_id").unwrap_or_default();
                let current = extract_i32(value, "current").unwrap_or(0);
                let target = extract_i32(value, "target").unwrap_or(1);

                log::debug!(
                    "Quest objective progress: {}:{} = {}/{}",
                    quest_id,
                    objective_id,
                    current,
                    target
                );

                // Update the objective in the active quest
                if let Some(quest) = state
                    .ui_state
                    .active_quests
                    .iter_mut()
                    .find(|q| q.id == quest_id)
                {
                    if let Some(obj) = quest.objectives.iter_mut().find(|o| o.id == objective_id) {
                        obj.current = current;
                        obj.target = target;
                        obj.completed = current >= target;
                    }
                }
            }
        }

        "questCompleted" => {
            if let Some(value) = data {
                let quest_id = extract_string(value, "quest_id").unwrap_or_default();
                let quest_name = extract_string(value, "quest_name").unwrap_or_default();
                let exp_reward = extract_i32(value, "rewards_exp").unwrap_or(0);
                let gold_reward = extract_i32(value, "rewards_gold").unwrap_or(0);
                let completed_id = quest_id.clone();

                log::info!(
                    "Quest completed: {} - {} (EXP: {}, Gold: {})",
                    quest_id,
                    quest_name,
                    exp_reward,
                    gold_reward
                );

                // Add system chat message
                state.push_system_chat(format!(
                        "Quest '{}' complete!",
                        quest_name
                    ));

                // Remove from active quests
                state.ui_state.active_quests.retain(|q| q.id != quest_id);

                // Play quest complete sound
                state.pending_sfx.push("quest_complete".to_string());

                // Add completion notification
                state
                    .ui_state
                    .quest_completed_events
                    .push(QuestCompletedEvent {
                        quest_id,
                        quest_name,
                        exp_reward,
                        gold_reward,
                        time: macroquad::time::get_time(),
                    });
                state.ui_state.completed_quest_ids.insert(completed_id);

                // Keep Adventurer Guide UI open and reset it after completion.
                if !reset_adventurer_guide_dialogue(state) {
                    state.ui_state.active_dialogue = None;
                }
            }
        }

        "dialogueClosed" => {
            // Keep Adventurer Guide panel open and reset to its initial state.
            if !reset_adventurer_guide_dialogue(state) {
                state.ui_state.active_dialogue = None;
            }
        }

        // ========== Item Definition Messages ==========
        "itemDefinitions" => {
            if let Some(value) = data {
                let mut items = Vec::new();

                if let Some(items_arr) = extract_array(value, "items") {
                    for item_value in items_arr {
                        let id = extract_string(item_value, "id").unwrap_or_default();
                        let display_name =
                            extract_string(item_value, "displayName").unwrap_or_default();
                        let sprite = extract_string(item_value, "sprite").unwrap_or_default();
                        let category = extract_string(item_value, "category")
                            .unwrap_or_else(|| "material".to_string());
                        let max_stack = extract_i32(item_value, "maxStack").unwrap_or(99);
                        let description =
                            extract_string(item_value, "description").unwrap_or_default();
                        let base_price = extract_i32(item_value, "basePrice").unwrap_or(0);
                        let sellable = extract_bool(item_value, "sellable").unwrap_or(false);

                        // Parse equipment stats if present
                        let equipment =
                            extract_string(item_value, "equipment_slot").map(|slot_type| {
                                let chop_speed =
                                    extract_f32(item_value, "chop_speed_multiplier").unwrap_or(0.0);
                                if chop_speed > 0.0 {
                                    log::info!(
                                        "Loaded item {} with chop_speed_multiplier={}",
                                        id,
                                        chop_speed
                                    );
                                }
                                let mine_speed =
                                    extract_f32(item_value, "mine_speed_multiplier").unwrap_or(0.0);
                                if mine_speed > 0.0 {
                                    log::info!(
                                        "Loaded item {} with mine_speed_multiplier={}",
                                        id,
                                        mine_speed
                                    );
                                }
                                EquipmentStats {
                                    slot_type,
                                    attack_level_required: extract_i32(
                                        item_value,
                                        "attack_level_required",
                                    )
                                    .unwrap_or(1),
                                    defence_level_required: extract_i32(
                                        item_value,
                                        "defence_level_required",
                                    )
                                    .unwrap_or(1),
                                    attack_bonus: extract_i32(item_value, "attack_bonus")
                                        .unwrap_or(0),
                                    strength_bonus: extract_i32(item_value, "strength_bonus")
                                        .unwrap_or(0),
                                    defence_bonus: extract_i32(item_value, "defence_bonus")
                                        .unwrap_or(0),
                                    magic_bonus: extract_i32(item_value, "magic_bonus")
                                        .unwrap_or(0),
                                    magic_level_required: extract_i32(
                                        item_value,
                                        "magic_level_required",
                                    )
                                    .unwrap_or(0),
                                    woodcutting_level_required: extract_i32(
                                        item_value,
                                        "woodcutting_level_required",
                                    )
                                    .unwrap_or(1),
                                    chop_speed_multiplier: chop_speed,
                                    mining_level_required: extract_i32(
                                        item_value,
                                        "mining_level_required",
                                    )
                                    .unwrap_or(1),
                                    mine_speed_multiplier: mine_speed,
                                }
                            });

                        // Parse weapon fields
                        let weapon_type = extract_string(item_value, "weapon_type");
                        let range = extract_i32(item_value, "range");

                        items.push(ItemDefinition {
                            id,
                            display_name,
                            sprite,
                            category,
                            max_stack,
                            description,
                            base_price,
                            sellable,
                            equipment,
                            weapon_type,
                            range,
                            prayer_xp: extract_i32(item_value, "prayer_xp").unwrap_or(0),
                            use_effect: extract_string(item_value, "use_effect_type"),
                        });
                    }
                }

                state.item_registry.load_from_server(items);
            }
        }

        // ========== Crafting System Messages ==========
        "recipeDefinitions" => {
            if let Some(value) = data {
                state.recipe_definitions.clear();

                if let Some(recipes_arr) = extract_array(value, "recipes") {
                    for recipe_value in recipes_arr {
                        let id = extract_string(recipe_value, "id").unwrap_or_default();
                        let display_name =
                            extract_string(recipe_value, "display_name").unwrap_or_default();
                        let description =
                            extract_string(recipe_value, "description").unwrap_or_default();
                        let category = extract_string(recipe_value, "category")
                            .unwrap_or_else(|| "consumables".to_string());
                        let section = extract_string(recipe_value, "section");
                        let level_required =
                            extract_i32(recipe_value, "level_required").unwrap_or(1);
                        let station = extract_string(recipe_value, "station");
                        let craft_time_ms = extract_u64(recipe_value, "craft_time_ms").unwrap_or(0);
                        let xp = extract_u32(recipe_value, "xp").unwrap_or(0);
                        let requires_discovery =
                            extract_bool(recipe_value, "requires_discovery").unwrap_or(false);
                        let required_tool = extract_string(recipe_value, "required_tool");
                        let burn_result = extract_string(recipe_value, "burn_result");
                        let burn_stop_level =
                            extract_i32(recipe_value, "burn_stop_level");

                        // Parse ingredients
                        let mut ingredients = Vec::new();
                        if let Some(ing_arr) = extract_array(recipe_value, "ingredients") {
                            for ing_value in ing_arr {
                                let item_id =
                                    extract_string(ing_value, "item_id").unwrap_or_default();
                                let item_name =
                                    extract_string(ing_value, "item_name").unwrap_or_default();
                                let count = extract_i32(ing_value, "count").unwrap_or(1);
                                ingredients.push(RecipeIngredient {
                                    item_id,
                                    item_name,
                                    count,
                                });
                            }
                        }

                        // Parse results
                        let mut results = Vec::new();
                        if let Some(res_arr) = extract_array(recipe_value, "results") {
                            for res_value in res_arr {
                                let item_id =
                                    extract_string(res_value, "item_id").unwrap_or_default();
                                let item_name =
                                    extract_string(res_value, "item_name").unwrap_or_default();
                                let count = extract_i32(res_value, "count").unwrap_or(1);
                                results.push(RecipeResult {
                                    item_id,
                                    item_name,
                                    count,
                                });
                            }
                        }

                        state.recipe_definitions.push(RecipeDefinition {
                            id,
                            display_name,
                            description,
                            category,
                            section,
                            level_required,
                            ingredients,
                            results,
                            station,
                            craft_time_ms,
                            xp,
                            requires_discovery,
                            required_tool,
                            burn_result,
                            burn_stop_level,
                        });
                    }
                }

                log::info!(
                    "Received {} recipe definitions",
                    state.recipe_definitions.len()
                );
            }
        }

        "shopOpen" => {
            if let Some(value) = data {
                let npc_id = extract_string(value, "npc_id").unwrap_or_default();
                log::info!("Opening shop for NPC: {}", npc_id);

                state.ui_state.crafting_open = true;
                state.ui_state.crafting_npc_id = Some(npc_id);
                state.ui_state.crafting_selected_category = 0;
                state.ui_state.crafting_selected_recipe = 0;
            }
        }

        "craftResult" => {
            if let Some(value) = data {
                let success = value
                    .as_map()
                    .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("success")))
                    .and_then(|(_, v)| v.as_bool())
                    .unwrap_or(false);
                let recipe_id = extract_string(value, "recipe_id").unwrap_or_default();
                let error = extract_string(value, "error");

                if success {
                    log::info!("Crafting success: {}", recipe_id);
                    // Inventory update will come separately
                } else {
                    log::warn!("Crafting failed: {} - {:?}", recipe_id, error);
                    if let Some(err) = error {
                        state.push_system_chat(format!("Crafting failed: {}", err));
                    }
                }
            }
        }

        "discoveredRecipes" => {
            if let Some(value) = data {
                if let Some(recipes_arr) = extract_array(value, "recipes") {
                    state.discovered_recipes.clear();
                    for recipe_value in recipes_arr {
                        if let Some(recipe_id) = recipe_value.as_str() {
                            state.discovered_recipes.insert(recipe_id.to_string());
                        }
                    }
                    log::info!(
                        "Received {} discovered recipes",
                        state.discovered_recipes.len()
                    );
                }
            }
        }

        "recipeDiscovered" => {
            if let Some(value) = data {
                let recipe_id = extract_string(value, "recipe_id").unwrap_or_default();
                if !recipe_id.is_empty() {
                    state.discovered_recipes.insert(recipe_id.clone());

                    // Look up display name from recipe definitions
                    let display_name = state
                        .recipe_definitions
                        .iter()
                        .find(|r| r.id == recipe_id)
                        .map(|r| r.display_name.clone())
                        .unwrap_or_else(|| recipe_id.clone());

                    state.push_system_chat(format!(
                            "Recipe learned: {}",
                            display_name
                        ));
                    log::info!("Recipe discovered: {}", recipe_id);
                }
            }
        }

        "craftingStarted" => {
            if let Some(value) = data {
                let recipe_id = extract_string(value, "recipe_id").unwrap_or_default();
                let duration_ms = extract_u64(value, "duration_ms").unwrap_or(0);

                log::info!("Crafting started: {} ({}ms)", recipe_id, duration_ms);
                state.ui_state.crafting_in_progress = true;
                state.ui_state.crafting_recipe_id = Some(recipe_id);
                state.ui_state.crafting_duration_ms = duration_ms;
                state.ui_state.crafting_started_at = Some(macroquad::time::get_time());
                state.ui_state.crafting_progress = 0.0;
            }
        }

        "craftingCancelled" => {
            if let Some(value) = data {
                let reason = extract_string(value, "reason").unwrap_or_default();

                log::info!("Crafting cancelled: {}", reason);
                state.ui_state.crafting_in_progress = false;
                state.ui_state.crafting_recipe_id = None;
                state.ui_state.crafting_started_at = None;
                state.ui_state.crafting_progress = 0.0;

                if !reason.is_empty() {
                    state.push_system_chat(format!(
                            "Crafting cancelled: {}",
                            reason
                        ));
                }
            }
        }

        "craftingCompleted" => {
            if let Some(value) = data {
                let recipe_id = extract_string(value, "recipe_id").unwrap_or_default();
                let xp_gained = extract_u32(value, "xp_gained").unwrap_or(0);

                log::info!("Crafting completed: {} (+{}xp)", recipe_id, xp_gained);

                // Clear crafting progress state
                state.ui_state.crafting_in_progress = false;
                state.ui_state.crafting_recipe_id = None;
                state.ui_state.crafting_started_at = None;
                state.ui_state.crafting_progress = 0.0;

                // Trigger completion animation (starts at 0.0, ticks up to 1.0)
                state.ui_state.crafting_complete_animation = Some((recipe_id.clone(), 0.0));

                // Look up display name from recipe definitions
                let display_name = state
                    .recipe_definitions
                    .iter()
                    .find(|r| r.id == recipe_id)
                    .map(|r| r.display_name.clone())
                    .unwrap_or_else(|| recipe_id.clone());

                let station = state
                    .recipe_definitions
                    .iter()
                    .find(|r| r.id == recipe_id)
                    .and_then(|r| r.station.as_deref())
                    .map(|s| s.to_string());

                if state.ui_state.batch_total > 1 {
                    state.push_system_chat(format!(
                        "{} ({}/{})",
                        display_name,
                        state.ui_state.batch_completed,
                        state.ui_state.batch_total
                    ));
                } else {
                    let verb = match station.as_deref() {
                        Some("furnace") => "Smelted",
                        Some("alchemy_station") => "Brewed",
                        Some("fire_pit") => "Cooked",
                        _ => "Crafted",
                    };
                    state.push_system_chat(format!("{}: {}", verb, display_name));
                }

                // Play furnace sound on successful smelt/craft
                state.pending_sfx.push("furnace".to_string());

                // Inventory update and XP will come via separate messages
            }
        }

        "craftingBatchProgress" => {
            if let Some(value) = data {
                let completed = extract_u32(value, "completed").unwrap_or(0);
                let total = extract_u32(value, "total").unwrap_or(0);
                state.ui_state.batch_completed = completed;
                state.ui_state.batch_total = total;
                log::info!("Batch progress: {}/{}", completed, total);
            }
        }

        // ========== Equipment Messages ==========
        "equipmentUpdate" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let equipped_head =
                    extract_string(value, "equipped_head").filter(|s| !s.is_empty());
                let equipped_body =
                    extract_string(value, "equipped_body").filter(|s| !s.is_empty());
                let equipped_weapon =
                    extract_string(value, "equipped_weapon").filter(|s| !s.is_empty());
                let equipped_back =
                    extract_string(value, "equipped_back").filter(|s| !s.is_empty());
                let equipped_feet =
                    extract_string(value, "equipped_feet").filter(|s| !s.is_empty());
                let equipped_ring =
                    extract_string(value, "equipped_ring").filter(|s| !s.is_empty());
                let equipped_gloves =
                    extract_string(value, "equipped_gloves").filter(|s| !s.is_empty());
                let equipped_necklace =
                    extract_string(value, "equipped_necklace").filter(|s| !s.is_empty());
                let equipped_belt =
                    extract_string(value, "equipped_belt").filter(|s| !s.is_empty());

                if let Some(player) = state.players.get_mut(&player_id) {
                    player.equipped_head = equipped_head.clone();
                    player.equipped_body = equipped_body.clone();
                    player.equipped_weapon = equipped_weapon.clone();
                    player.equipped_back = equipped_back.clone();
                    player.equipped_feet = equipped_feet.clone();
                    player.equipped_ring = equipped_ring.clone();
                    player.equipped_gloves = equipped_gloves.clone();
                    player.equipped_necklace = equipped_necklace.clone();
                    player.equipped_belt = equipped_belt.clone();
                    log::info!("Player {} equipment updated", player_id);
                }
            }
        }

        "equipResult" => {
            if let Some(value) = data {
                let success = value
                    .as_map()
                    .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("success")))
                    .and_then(|(_, v)| v.as_bool())
                    .unwrap_or(false);
                let slot_type = extract_string(value, "slot_type").unwrap_or_default();
                let item_id = extract_string(value, "item_id");
                let error = extract_string(value, "error");

                if success {
                    log::info!("Equipment {} success: {:?}", slot_type, item_id);
                } else {
                    log::warn!("Equipment {} failed: {:?}", slot_type, error);
                    // TODO: Show error message in UI
                }
            }
        }

        // ========== Admin Messages ==========
        "announcement" => {
            if let Some(value) = data {
                let text = extract_string(value, "text").unwrap_or_default();
                log::info!("Server announcement: {}", text);
                state
                    .ui_state
                    .announcements
                    .push(crate::game::Announcement {
                        text,
                        time: macroquad::time::get_time(),
                    });
                state.pending_sfx.push("announce".to_string());
            }
        }

        // ========== Shop System Messages ==========

        // ========== Bank System Messages ==========
        "bankOpen" => {
            if let Some(value) = data {
                let mut slots = Vec::new();
                if let Some(slots_arr) = extract_array(value, "slots") {
                    for slot_value in slots_arr {
                        let slot = extract_i32(slot_value, "slot").unwrap_or(0) as u8;
                        let item_id = extract_string(slot_value, "item_id").unwrap_or_default();
                        let quantity = extract_i32(slot_value, "quantity").unwrap_or(0);
                        if !item_id.is_empty() && quantity > 0 {
                            slots.push((slot, item_id, quantity));
                        }
                    }
                }
                let gold = extract_i32(value, "gold").unwrap_or(0);
                let max_slots = extract_i32(value, "max_slots").unwrap_or(48) as u32;

                log::info!(
                    "Bank opened: {} items, {}g, {} max slots",
                    slots.len(),
                    gold,
                    max_slots
                );
                state.ui_state.bank_open = true;
                state.ui_state.bank_slots = vec![None; max_slots as usize];
                for (slot, item_id, quantity) in slots {
                    if (slot as usize) < state.ui_state.bank_slots.len() {
                        state.ui_state.bank_slots[slot as usize] = Some((item_id, quantity));
                    }
                }
                state.ui_state.bank_gold = gold;
                state.ui_state.bank_max_slots = max_slots;
                state.pending_sfx.push("ui_open".to_string());
            }
        }

        "bankUpdate" => {
            if let Some(value) = data {
                // Rebuild slots from server data
                let mut new_slots = vec![None; state.ui_state.bank_max_slots as usize];
                if let Some(slots_arr) = extract_array(value, "slots") {
                    for slot_value in slots_arr {
                        let slot = extract_i32(slot_value, "slot").unwrap_or(0) as u8;
                        let item_id = extract_string(slot_value, "item_id").unwrap_or_default();
                        let quantity = extract_i32(slot_value, "quantity").unwrap_or(0);
                        if !item_id.is_empty() && quantity > 0 && (slot as usize) < new_slots.len()
                        {
                            new_slots[slot as usize] = Some((item_id, quantity));
                        }
                    }
                }
                state.ui_state.bank_slots = new_slots;
                state.ui_state.bank_gold =
                    extract_i32(value, "gold").unwrap_or(state.ui_state.bank_gold);
            }
        }

        "bankResult" => {
            if let Some(value) = data {
                let success = extract_bool(value, "success").unwrap_or(false);
                let error = extract_string(value, "error");

                if !success {
                    if let Some(err) = error {
                        state.push_system_chat(format!("Bank: {}", err));
                    }
                }
            }
        }

        "chestOpen" => {
            if let Some(value) = data {
                let chest_id = extract_string(value, "chest_id").unwrap_or_default();
                let chest_name = extract_string(value, "name").unwrap_or_else(|| "Chest".to_string());
                let total_value = extract_i32(value, "total_value").unwrap_or(0);
                let mut slots = Vec::new();
                if let Some(slots_arr) = extract_array(value, "slots") {
                    for slot_value in slots_arr {
                        let slot = extract_i32(slot_value, "slot").unwrap_or(0) as u8;
                        let item_id = extract_string(slot_value, "item_id").unwrap_or_default();
                        let quantity = extract_i32(slot_value, "quantity").unwrap_or(0);
                        let value = extract_i32(slot_value, "value").unwrap_or(0);
                        if !item_id.is_empty() && quantity > 0 {
                            slots.push((slot, item_id, quantity, value));
                        }
                    }
                }

                log::info!("Chest opened: '{}', {} items, total value {}g", chest_id, slots.len(), total_value);

                // Determine slot count from the max slot index
                let max_slot = slots.iter().map(|(s, _, _, _)| *s as usize).max().unwrap_or(0);
                let num_slots = (max_slot + 1).max(10);

                state.ui_state.chest_open = true;
                state.ui_state.inventory_open = true;
                state.ui_state.chest_id = chest_id;
                state.ui_state.chest_name = chest_name;
                state.ui_state.chest_slots = vec![None; num_slots];
                for (slot, item_id, quantity, value) in slots {
                    if (slot as usize) < state.ui_state.chest_slots.len() {
                        state.ui_state.chest_slots[slot as usize] = Some((item_id, quantity, value));
                    }
                }
                state.ui_state.chest_total_value = total_value;
                state.ui_state.chest_scroll = 0.0;
                state.pending_sfx.push("ui_open".to_string());
            }
        }

        "chestUpdate" => {
            if let Some(value) = data {
                let chest_id = extract_string(value, "chest_id").unwrap_or_default();
                if chest_id == state.ui_state.chest_id {
                    let total_value = extract_i32(value, "total_value").unwrap_or(0);
                    let mut new_slots = vec![None; state.ui_state.chest_slots.len()];
                    if let Some(slots_arr) = extract_array(value, "slots") {
                        for slot_value in slots_arr {
                            let slot = extract_i32(slot_value, "slot").unwrap_or(0) as u8;
                            let item_id = extract_string(slot_value, "item_id").unwrap_or_default();
                            let quantity = extract_i32(slot_value, "quantity").unwrap_or(0);
                            let value = extract_i32(slot_value, "value").unwrap_or(0);
                            if !item_id.is_empty() && quantity > 0 && (slot as usize) < new_slots.len() {
                                new_slots[slot as usize] = Some((item_id, quantity, value));
                            }
                        }
                    }
                    state.ui_state.chest_slots = new_slots;
                    state.ui_state.chest_total_value = total_value;
                }
            }
        }

        "shopData" => {
            if let Some(value) = data {
                // Extract npcId from top level (camelCase from server)
                let npc_id = extract_string(value, "npcId").unwrap_or_default();

                // Extract shop data from nested "shop" field
                let shop_value = value
                    .as_map()
                    .and_then(|m| {
                        m.iter()
                            .find(|(k, _)| k.as_str() == Some("shop"))
                            .map(|(_, v)| v)
                    })
                    .unwrap_or(value);

                let shop_id = extract_string(shop_value, "shopId").unwrap_or_default();
                let display_name =
                    extract_string(shop_value, "displayName").unwrap_or_else(|| "Shop".to_string());
                let buy_multiplier = extract_f32(shop_value, "buyMultiplier").unwrap_or(0.5);
                let sell_multiplier = extract_f32(shop_value, "sellMultiplier").unwrap_or(1.0);

                // Parse crafting categories from server
                let crafting_categories: Vec<String> = extract_array(shop_value, "craftingCategories")
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let crafting_stations: Vec<String> = extract_array(shop_value, "craftingStations")
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let show_crafting = !crafting_categories.is_empty();

                let mut stock = Vec::new();
                if let Some(stock_arr) = extract_array(shop_value, "stock") {
                    for item_value in stock_arr {
                        let item_id = extract_string(item_value, "itemId").unwrap_or_default();
                        let quantity = extract_i32(item_value, "quantity").unwrap_or(0);
                        let price = extract_i32(item_value, "price").unwrap_or(0);

                        stock.push(ShopStockItem {
                            item_id,
                            quantity,
                            price,
                        });
                    }
                }

                log::info!(
                    "Shop data received: {} items from {} (npc: {})",
                    stock.len(),
                    display_name,
                    npc_id
                );
                state.ui_state.shop_npc_id = Some(npc_id);
                state.ui_state.shop_data = Some(ShopData {
                    shop_id,
                    display_name,
                    buy_multiplier,
                    sell_multiplier,
                    show_crafting,
                    crafting_categories,
                    crafting_stations,
                    stock,
                });
                state.ui_state.crafting_open = true; // Open crafting window (which has shop tab)
                state.ui_state.shop_main_tab = 1; // Switch to Shop tab
                state.pending_sfx.push("ui_open".to_string());
            }
        }

        "shopResult" => {
            if let Some(value) = data {
                let success = value
                    .as_map()
                    .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("success")))
                    .and_then(|(_, v)| v.as_bool())
                    .unwrap_or(false);
                let action = extract_string(value, "action").unwrap_or_default();
                let item_id = extract_string(value, "itemId").unwrap_or_default();
                let quantity = extract_i32(value, "quantity").unwrap_or(0);
                let gold_change = extract_i32(value, "goldChange").unwrap_or(0);
                let error = extract_string(value, "error");

                if success {
                    log::info!("Shop transaction successful");

                    // Get item display name from registry
                    let item_name = state
                        .item_registry
                        .get(&item_id)
                        .map(|def| def.display_name.clone())
                        .unwrap_or_else(|| item_id.clone());

                    // Add system chat message
                    let message = if action == "buy" {
                        format!(
                            "Bought {}x {} for {}g",
                            quantity,
                            item_name,
                            gold_change.abs()
                        )
                    } else {
                        format!(
                            "Sold {}x {} for {}g",
                            quantity,
                            item_name,
                            gold_change.abs()
                        )
                    };
                    state.push_system_chat(message);
                } else if let Some(err) = error {
                    log::warn!("Shop transaction failed: {}", err);
                    // Show error in system chat
                    state.push_system_chat(format!("Transaction failed: {}", err));
                }
            }
        }

        "shopStockUpdate" => {
            if let Some(value) = data {
                let item_id = extract_string(value, "itemId").unwrap_or_default();
                let new_quantity = extract_i32(value, "newQuantity").unwrap_or(0);

                // Update the stock in the current shop data if it's open
                if let Some(shop_data) = &mut state.ui_state.shop_data {
                    if let Some(item) = shop_data.stock.iter_mut().find(|i| i.item_id == item_id) {
                        item.quantity = new_quantity;
                        log::debug!(
                            "Shop stock updated: {} now has {} in stock",
                            item_id,
                            new_quantity
                        );
                    }
                }
            }
        }

        "mapTransition" => {
            if let Some(value) = data {
                let map_type = extract_string(value, "mapType").unwrap_or_default();
                let map_id = extract_string(value, "mapId").unwrap_or_default();
                let spawn_x = extract_f32(value, "spawnX").unwrap_or(0.0);
                let spawn_y = extract_f32(value, "spawnY").unwrap_or(0.0);
                let instance_id = extract_string(value, "instanceId").unwrap_or_default();

                if map_type == "overworld" {
                    // Returning to overworld from interior

                    // Trigger area banner for overworld
                    state.area_banner.show(OVERWORLD_NAME);

                    // Clear interior mode
                    state.chunk_manager.clear_interior();
                    state.current_interior = None;
                    state.current_instance = None;

                    // Clear interior NPCs and ground items (will be repopulated by stateSync)
                    state.npcs.clear();
                    state.ground_items.clear();

                    // Reset portal check and ignore the spawn tile until player steps off
                    state.last_portal_check_pos = None;
                    let spawn_tile = (spawn_x.floor() as i32, spawn_y.floor() as i32);
                    state.portal_ignore_tile = Some(spawn_tile);

                    // Update player position (both visual and server-authoritative)
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(player) = state.players.get_mut(local_id) {
                            player.x = spawn_x;
                            player.y = spawn_y;
                            player.server_x = spawn_x;
                            player.server_y = spawn_y;
                            player.target_x = spawn_x;
                            player.target_y = spawn_y;
                        }
                    }

                    // Start fade-in transition directly (no loading needed)
                    state.map_transition.state = crate::game::state::TransitionState::FadingIn;
                    state.map_transition.progress = 1.0;
                } else {
                    // Transitioning to interior - wait for interiorData
                    state.start_transition(map_type, map_id, spawn_x, spawn_y, instance_id);
                }
            }
        }

        "interiorData" => {
            if let Some(value) = data {
                let map_id = extract_string(value, "mapId").unwrap_or_default();
                let instance_id = extract_string(value, "instanceId").unwrap_or_default();
                let width = extract_u32(value, "width").unwrap_or(32);
                let height = extract_u32(value, "height").unwrap_or(32);
                let spawn_x = extract_f32(value, "spawnX").unwrap_or(0.0);
                let spawn_y = extract_f32(value, "spawnY").unwrap_or(0.0);

                // Extract interior name (fallback to map_id if missing)
                let name = extract_string(value, "name").unwrap_or(map_id.clone());

                // Trigger area banner
                state.area_banner.show(&name);

                // Parse layers
                let mut layers: Vec<(u8, Vec<u32>)> = Vec::new();
                if let Some(layers_arr) = extract_array(value, "layers") {
                    for layer_data in layers_arr {
                        let layer_type = extract_u8(layer_data, "layerType").unwrap_or(0);
                        let tiles: Vec<u32> = extract_array(layer_data, "tiles")
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_u64().map(|n| n as u32))
                                    .collect()
                            })
                            .unwrap_or_default();
                        layers.push((layer_type, tiles));
                    }
                }

                // Parse collision
                let collision: Vec<u8> = extract_array(value, "collision")
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_u64().map(|n| n as u8))
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse portals
                let mut portals: Vec<Portal> = Vec::new();
                if let Some(portals_arr) = extract_array(value, "portals") {
                    for p in portals_arr {
                        portals.push(Portal {
                            id: extract_string(p, "id").unwrap_or_default(),
                            x: extract_i32(p, "x").unwrap_or(0),
                            y: extract_i32(p, "y").unwrap_or(0),
                            width: extract_i32(p, "width").unwrap_or(1),
                            height: extract_i32(p, "height").unwrap_or(1),
                            target_map: extract_string(p, "targetMap").unwrap_or_default(),
                            target_spawn: extract_string(p, "targetSpawn").unwrap_or_default(),
                        });
                    }
                }

                // Parse objects (trees, rocks, decorations)
                let mut objects: Vec<MapObject> = Vec::new();
                if let Some(objects_arr) = extract_array(value, "objects") {
                    for o in objects_arr {
                        objects.push(MapObject {
                            gid: extract_u32(o, "gid").unwrap_or(0),
                            tile_x: extract_i32(o, "tileX").unwrap_or(0),
                            tile_y: extract_i32(o, "tileY").unwrap_or(0),
                            width: extract_u32(o, "width").unwrap_or(32),
                            height: extract_u32(o, "height").unwrap_or(32),
                        });
                    }
                }

                // Parse walls
                let mut walls: Vec<Wall> = Vec::new();
                if let Some(walls_arr) = extract_array(value, "walls") {
                    for w in walls_arr {
                        let edge_str = extract_string(w, "edge").unwrap_or_default();
                        let edge = match edge_str.as_str() {
                            "right" | "Right" => WallEdge::Right,
                            _ => WallEdge::Down,
                        };
                        walls.push(Wall {
                            gid: extract_u32(w, "gid").unwrap_or(0),
                            tile_x: extract_i32(w, "tileX").unwrap_or(0),
                            tile_y: extract_i32(w, "tileY").unwrap_or(0),
                            edge,
                        });
                    }
                }

                // Clear world data when entering interior
                state.npcs.clear();
                state.ground_items.clear();
                state.chair_positions.clear();
                state.chest_positions.clear();
                state.gathering_markers.clear();
                state.pending_chair_sit = None;

                // Clear other players (keep only local player) to avoid ghost collisions
                if let Some(local_id) = &state.local_player_id {
                    let local_player = state.players.remove(local_id);
                    state.players.clear();
                    if let Some(player) = local_player {
                        state.players.insert(local_id.clone(), player);
                    }
                } else {
                    state.players.clear();
                }

                // Load the interior
                state
                    .chunk_manager
                    .load_interior(width, height, layers, &collision, portals, objects, walls);
                state.current_interior = Some(map_id.clone());
                state.current_instance = Some(instance_id);

                // Reset portal check and ignore the spawn tile until player steps off
                state.last_portal_check_pos = None;
                let spawn_tile = (spawn_x.floor() as i32, spawn_y.floor() as i32);
                state.portal_ignore_tile = Some(spawn_tile);

                // Update player position (both visual and server-authoritative)
                if let Some(local_id) = &state.local_player_id {
                    if let Some(player) = state.players.get_mut(local_id) {
                        player.x = spawn_x;
                        player.y = spawn_y;
                        player.server_x = spawn_x;
                        player.server_y = spawn_y;
                        player.target_x = spawn_x;
                        player.target_y = spawn_y;
                    }
                }

                // Complete the transition (fade in)
                // Handle both Loading (normal case) and FadingOut (data arrived quickly)
                match state.map_transition.state {
                    crate::game::state::TransitionState::Loading => {
                        state.map_transition.state = crate::game::state::TransitionState::FadingIn;
                    }
                    crate::game::state::TransitionState::FadingOut => {
                        // Data arrived before fade out completed - skip to fade in
                        state.map_transition.progress = 1.0;
                        state.map_transition.state = crate::game::state::TransitionState::FadingIn;
                    }
                    _ => {}
                }
            }
        }

        "chairPositions" => {
            if let Some(value) = data {
                if let Some(positions_arr) = extract_array(value, "positions") {
                    let mut positions = Vec::new();
                    for p in positions_arr {
                        let x = extract_i32(p, "x").unwrap_or(0);
                        let y = extract_i32(p, "y").unwrap_or(0);
                        positions.push((x, y));
                    }
                    log::info!("Received {} chair positions", positions.len());
                    state.chair_positions = positions;
                }
            }
        }

        "chestPositions" => {
            if let Some(value) = data {
                if let Some(positions_arr) = extract_array(value, "positions") {
                    let mut positions = Vec::new();
                    for p in positions_arr {
                        let x = extract_i32(p, "x").unwrap_or(0);
                        let y = extract_i32(p, "y").unwrap_or(0);
                        positions.push((x, y));
                    }
                    log::info!("Received {} chest positions", positions.len());
                    state.chest_positions = positions;
                }
            }
        }

        "sitResult" => {
            if let Some(value) = data {
                let success = extract_bool(value, "success").unwrap_or(false);
                if success {
                    let tile_x = extract_i32(value, "tileX").unwrap_or(0);
                    let tile_y = extract_i32(value, "tileY").unwrap_or(0);
                    let direction = extract_i32(value, "direction").unwrap_or(0) as u8;
                    state.is_sitting = true;
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(player) = state.players.get_mut(local_id) {
                            player.x = tile_x as f32;
                            player.y = tile_y as f32;
                            player.server_x = tile_x as f32;
                            player.server_y = tile_y as f32;
                            player.target_x = tile_x as f32;
                            player.target_y = tile_y as f32;
                            player.direction = Direction::from_u8(direction);
                            player.animation.direction = Direction::from_u8(direction);
                            player.sit_chair();
                        }
                    }
                }
            }
        }

        "gatheringMarkers" => {
            if let Some(value) = data {
                if let Some(markers_arr) = extract_array(value, "markers") {
                    let mut markers = Vec::new();
                    for m in markers_arr {
                        let x = extract_i32(m, "x").unwrap_or(0);
                        let y = extract_i32(m, "y").unwrap_or(0);
                        let zone_id = extract_string(m, "zone_id").unwrap_or_default();
                        let skill = extract_string(m, "skill").unwrap_or_default();
                        markers.push(GatheringMarker {
                            x,
                            y,
                            zone_id,
                            skill,
                        });
                    }
                    log::info!("Received {} gathering markers", markers.len());
                    state.gathering_markers = markers;
                }
            }
        }

        "gatheringStarted" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let zone_id = extract_string(value, "zone_id").unwrap_or_default();
                log::info!(
                    "Gathering started for player {} in zone {}",
                    player_id,
                    zone_id
                );
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.is_gathering = true;
                    player.gathering_started_at = macroquad::time::get_time();
                    player.play_attack();
                }
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.is_gathering = true;
                    state.gathering_started_at = macroquad::time::get_time();
                }
            }
        }

        "gatheringResult" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                let xp_gained = extract_i32(value, "xp_gained").unwrap_or(0) as i64;
                log::info!(
                    "Gathering result: player {} got {} (+{}xp)",
                    player_id,
                    item_id,
                    xp_gained
                );
                if state.local_player_id.as_deref() == Some(&player_id) {
                    let item_name = state
                        .item_registry
                        .get(&item_id)
                        .map(|d| d.display_name.clone())
                        .unwrap_or(item_id.clone());
                    // Add XP event for floating text
                    if let Some(player) = state.players.get(&player_id) {
                        state.skill_xp_events.push(SkillXpEvent {
                            x: player.x,
                            y: player.y,
                            skill: "Fishing".to_string(),
                            xp_gained,
                            time: macroquad::time::get_time(),
                        });
                    }
                    // Add chat message about the catch
                    state.push_system_chat(format!("You caught a {}!", item_name));
                }
            }
        }

        "gatheringStopped" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let reason = extract_string(value, "reason").unwrap_or_default();
                log::info!("Gathering stopped for player {}: {}", player_id, reason);
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.is_gathering = false;
                }
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.is_gathering = false;
                    if reason == "inventory_full" {
                        state.push_system_chat("Your inventory is full!".to_string());
                        state.pending_sfx.push("error".to_string());
                    }
                }
            }
        }

        "bonusTileSpawned" => {
            if let Some(value) = data {
                let x = extract_i32(value, "x").unwrap_or(0);
                let y = extract_i32(value, "y").unwrap_or(0);
                let zone_id = extract_string(value, "zone_id").unwrap_or_default();
                let telegraph_duration =
                    extract_u64(value, "telegraph_duration").unwrap_or(5) as f64;
                log::info!("Bonus tile spawned at ({}, {}) in zone {}", x, y, zone_id);
                state.bonus_tiles.push(BonusTile {
                    x,
                    y,
                    zone_id,
                    spawn_time: macroquad::time::get_time(),
                    telegraph_duration,
                });
            }
        }

        "bonusTileClaimed" => {
            if let Some(value) = data {
                let x = extract_i32(value, "x").unwrap_or(0);
                let y = extract_i32(value, "y").unwrap_or(0);
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                log::info!("Bonus tile at ({}, {}) claimed by {}", x, y, player_id);
                // Remove the bonus tile
                state.bonus_tiles.retain(|t| t.x != x || t.y != y);
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.push_chat_message(ChatMessage::system(
                        "You claimed the bonus spot! 2x gathering speed for 30s!".to_string(),
                    ));
                }
            }
        }

        "bonusTileExpired" => {
            if let Some(value) = data {
                let x = extract_i32(value, "x").unwrap_or(0);
                let y = extract_i32(value, "y").unwrap_or(0);
                log::info!("Bonus tile at ({}, {}) expired", x, y);
                state.bonus_tiles.retain(|t| t.x != x || t.y != y);
            }
        }

        "buffApplied" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let buff_type = extract_string(value, "buff_type").unwrap_or_default();
                let duration = extract_u64(value, "duration").unwrap_or(30) as f64;
                log::info!(
                    "Buff {} applied to player {} for {}s",
                    buff_type,
                    player_id,
                    duration
                );
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.gathering_buff = Some(GatheringBuff {
                        buff_type,
                        start_time: macroquad::time::get_time(),
                        duration,
                    });
                }
            }
        }

        "buffExpired" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let buff_type = extract_string(value, "buff_type").unwrap_or_default();
                log::info!("Buff {} expired for player {}", buff_type, player_id);
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.gathering_buff = None;
                }
            }
        }

        // =====================================================================
        // Woodcutting Messages
        // =====================================================================
        "woodcuttingSwing" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let tree_x = extract_i32(value, "tree_x").unwrap_or(0);
                let tree_y = extract_i32(value, "tree_y").unwrap_or(0);

                // Check if the action is close enough to the local player to hear
                let is_local = state.local_player_id.as_deref() == Some(&player_id);
                let in_audio_range = is_local || {
                    state.local_player_id.as_ref().and_then(|id| state.players.get(id)).map(|lp| {
                        let dx = (lp.x - tree_x as f32).abs();
                        let dy = (lp.y - tree_y as f32).abs();
                        dx.max(dy) <= SFX_AUDIBLE_RANGE
                    }).unwrap_or(false)
                };

                // Server says player swung - play attack animation (always) and sound (if in range)
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.play_attack();
                }
                if in_audio_range {
                    state.pending_attack_sounds.push(crate::game::state::AttackSoundType::Melee);

                    // Play woodcutting sound effect
                    state.pending_sfx.push("woodcut".to_string());
                }

                // Add tree shake effect
                state
                    .tree_shake_effects
                    .push(crate::game::state::TreeShakeEffect::new(tree_x, tree_y));

                // Spawn leaf particles at the top of the tree (skip on low graphics)
                if !state.ui_state.graphics_low {
                    let tree_height = 60.0;
                    for _ in 0..3 {
                        state
                            .leaf_particles
                            .push(crate::game::state::LeafParticle::new_at_tree(
                                tree_x,
                                tree_y,
                                tree_height,
                            ));
                    }
                }
            }
        }

        "woodcuttingResult" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                log::info!("Woodcutting result: player {} got {}", player_id, item_id);

                // XP display is handled by the separate skillXp message
                // This handler just shows the item feedback

                if state.local_player_id.as_deref() == Some(player_id.as_str()) {
                    let item_name = state
                        .item_registry
                        .get(&item_id)
                        .map(|d| d.display_name.clone())
                        .unwrap_or(item_id.clone());

                    // Add chat message about the chop
                    state.push_system_chat(format!(
                            "You chopped some {}!",
                            item_name
                        ));
                }
            }
        }

        "treeDepleted" => {
            if let Some(value) = data {
                let x = extract_i32(value, "x").unwrap_or(0);
                let y = extract_i32(value, "y").unwrap_or(0);
                let gid = extract_u32(value, "gid").unwrap_or(0);
                let respawn_delay_ms = extract_u64(value, "respawn_delay_ms").unwrap_or(7500);
                let now = macroquad::time::get_time();
                log::info!(
                    "Tree depleted at ({}, {}), respawn in {}ms",
                    x,
                    y,
                    respawn_delay_ms
                );

                // Add falling tree effect
                state
                    .falling_trees
                    .push(crate::game::state::FallingTreeEffect::new(x, y, gid));

                // Spawn a burst of leaves when tree falls (skip on low graphics)
                if !state.ui_state.graphics_low {
                    let tree_height = 60.0;
                    for _ in 0..10 {
                        state
                            .leaf_particles
                            .push(crate::game::state::LeafParticle::new_at_tree(
                                x,
                                y,
                                tree_height,
                            ));
                    }
                }

                // Mark tree as depleted (hides the static tree, shows respawn timer)
                state.depleted_trees.insert(
                    (x, y),
                    crate::game::state::DepletedTreeInfo {
                        gid,
                        depleted_at: now,
                        respawn_at: now + (respawn_delay_ms as f64 / 1000.0),
                    },
                );
            }
        }

        "treeRespawned" => {
            if let Some(value) = data {
                let x = extract_i32(value, "x").unwrap_or(0);
                let y = extract_i32(value, "y").unwrap_or(0);
                log::info!("Tree respawned at ({}, {})", x, y);
                state.depleted_trees.remove(&(x, y));
            }
        }

        "depletedTreesSync" => {
            if let Some(value) = data {
                if let Some(trees_arr) = extract_array(value, "trees") {
                    state.depleted_trees.clear();
                    let now = macroquad::time::get_time();
                    for tree in trees_arr {
                        let x = extract_i32(tree, "x").unwrap_or(0);
                        let y = extract_i32(tree, "y").unwrap_or(0);
                        let gid = extract_u32(tree, "gid").unwrap_or(0);
                        // For sync, we don't know exact respawn time, use a short default
                        state.depleted_trees.insert(
                            (x, y),
                            crate::game::state::DepletedTreeInfo {
                                gid,
                                depleted_at: now,
                                respawn_at: now + 5.0, // Default 5 seconds remaining
                            },
                        );
                    }
                    log::info!("Synced {} depleted trees", state.depleted_trees.len());
                }
            }
        }

        // =====================================================================
        // Mining Messages
        // =====================================================================
        "miningStarted" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                log::info!("Mining started for player {}", player_id);
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.is_mining = true;
                    player.mining_started_at = macroquad::time::get_time();
                    player.play_attack();
                }
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.is_mining = true;
                    state.mining_started_at = macroquad::time::get_time();
                }
            }
        }

        "miningStopped" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let reason = extract_string(value, "reason").unwrap_or_default();
                log::info!("Mining stopped for player {}: {}", player_id, reason);
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.is_mining = false;
                }
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.is_mining = false;
                    if reason == "inventory_full" {
                        state.push_system_chat("Your inventory is full!".to_string());
                        state.pending_sfx.push("error".to_string());
                    }
                }
            }
        }

        "miningSwing" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let rock_x = extract_i32(value, "rock_x").unwrap_or(0);
                let rock_y = extract_i32(value, "rock_y").unwrap_or(0);

                // Check if the action is close enough to the local player to hear
                let is_local = state.local_player_id.as_deref() == Some(&player_id);
                let in_audio_range = is_local || {
                    state.local_player_id.as_ref().and_then(|id| state.players.get(id)).map(|lp| {
                        let dx = (lp.x - rock_x as f32).abs();
                        let dy = (lp.y - rock_y as f32).abs();
                        dx.max(dy) <= SFX_AUDIBLE_RANGE
                    }).unwrap_or(false)
                };

                // Server says player swung - play attack animation (always) and sound (if in range)
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.play_attack();
                }
                if in_audio_range {
                    state.pending_attack_sounds.push(crate::game::state::AttackSoundType::Melee);

                    // Play mining sound effect
                    state.pending_sfx.push("mining".to_string());
                }

                // Add rock shake effect
                state
                    .rock_shake_effects
                    .push(crate::game::state::RockShakeEffect::new(rock_x, rock_y));

                // Spawn rock debris particles (skip on low graphics)
                if !state.ui_state.graphics_low {
                    let rock_height = 30.0;
                    for _ in 0..4 {
                        state
                            .rock_particles
                            .push(crate::game::state::RockParticle::new_at_rock(
                                rock_x,
                                rock_y,
                                rock_height,
                            ));
                    }
                }
            }
        }

        "miningResult" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                log::info!("Mining result: player {} got {}", player_id, item_id);

                // XP display is handled by the separate skillXp message
                // This handler just shows the item feedback

                if state.local_player_id.as_deref() == Some(player_id.as_str()) {
                    let item_name = state
                        .item_registry
                        .get(&item_id)
                        .map(|d| d.display_name.clone())
                        .unwrap_or(item_id.clone());

                    // Add chat message about the mine
                    state.push_system_chat(format!(
                            "You mined some {}!",
                            item_name
                        ));
                }
            }
        }

        "rockDepleted" => {
            if let Some(value) = data {
                let x = extract_i32(value, "x").unwrap_or(0);
                let y = extract_i32(value, "y").unwrap_or(0);
                let gid = extract_u32(value, "gid").unwrap_or(0);
                let respawn_delay_ms = extract_u64(value, "respawn_delay_ms").unwrap_or(7500);
                let now = macroquad::time::get_time();
                log::info!(
                    "Rock depleted at ({}, {}), respawn in {}ms",
                    x,
                    y,
                    respawn_delay_ms
                );

                // Add crumbling rock effect
                state
                    .crumbling_rocks
                    .push(crate::game::state::CrumblingRockEffect::new(x, y, gid));

                // Spawn a burst of rock debris when rock crumbles (skip on low graphics)
                if !state.ui_state.graphics_low {
                    let rock_height = 30.0;
                    for _ in 0..12 {
                        state
                            .rock_particles
                            .push(crate::game::state::RockParticle::new_at_rock(
                                x,
                                y,
                                rock_height,
                            ));
                    }
                }

                // Mark rock as depleted (hides the static rock, shows respawn timer)
                state.depleted_rocks.insert(
                    (x, y),
                    crate::game::state::DepletedRockInfo {
                        gid,
                        depleted_at: now,
                        respawn_at: now + (respawn_delay_ms as f64 / 1000.0),
                    },
                );
            }
        }

        "rockRespawned" => {
            if let Some(value) = data {
                let x = extract_i32(value, "x").unwrap_or(0);
                let y = extract_i32(value, "y").unwrap_or(0);
                log::info!("Rock respawned at ({}, {})", x, y);
                state.depleted_rocks.remove(&(x, y));
            }
        }

        "depletedRocksSync" => {
            if let Some(value) = data {
                if let Some(rocks_arr) = extract_array(value, "rocks") {
                    state.depleted_rocks.clear();
                    let now = macroquad::time::get_time();
                    for rock in rocks_arr {
                        let x = extract_i32(rock, "x").unwrap_or(0);
                        let y = extract_i32(rock, "y").unwrap_or(0);
                        let gid = extract_u32(rock, "gid").unwrap_or(0);
                        // For sync, we don't know exact respawn time, use a short default
                        state.depleted_rocks.insert(
                            (x, y),
                            crate::game::state::DepletedRockInfo {
                                gid,
                                depleted_at: now,
                                respawn_at: now + 5.0, // Default 5 seconds remaining
                            },
                        );
                    }
                    log::info!("Synced {} depleted rocks", state.depleted_rocks.len());
                }
            }
        }

        "farmingPatchStates" => {
            if let Some(value) = data {
                if let Some(patches_arr) = extract_array(value, "patches") {
                    state.farming_patches.clear();
                    state.farming_patch_positions.clear();
                    for p in patches_arr {
                        let patch_id = extract_string(p, "patch_id").unwrap_or_default();
                        let x = extract_i32(p, "x").unwrap_or(0);
                        let y = extract_i32(p, "y").unwrap_or(0);
                        let patch_state =
                            extract_string(p, "state").unwrap_or_else(|| "empty".to_string());
                        let crop_id = extract_string(p, "crop_id").unwrap_or_default();
                        let growth_stage = extract_u32(p, "growth_stage").unwrap_or(0);
                        let owner_id = extract_string(p, "owner_id").unwrap_or_default();
                        state
                            .farming_patch_positions
                            .insert((x, y), patch_id.clone());
                        state.farming_patches.insert(
                            patch_id.clone(),
                            FarmingPatch {
                                patch_id,
                                x,
                                y,
                                state: patch_state,
                                crop_id,
                                growth_stage,
                                owner_id,
                            },
                        );
                    }
                    // Parse unlocked plots
                    if let Some(plots_arr) = extract_array(value, "unlocked_plots") {
                        state.unlocked_farming_plots = plots_arr
                            .iter()
                            .filter_map(|v| v.as_u64().map(|u| u as u32))
                            .collect();
                    } else {
                        state.unlocked_farming_plots = vec![1];
                    }

                    // Parse ground tile overrides (farming plot tiles)
                    state.ground_tile_overrides.clear();
                    if let Some(overrides_arr) = extract_array(value, "tile_overrides") {
                        for t in overrides_arr {
                            let x = extract_i32(t, "x").unwrap_or(0);
                            let y = extract_i32(t, "y").unwrap_or(0);
                            let tile_id = extract_u32(t, "tile_id").unwrap_or(0);
                            state.ground_tile_overrides.insert((x, y), tile_id);
                        }
                    }

                    log::info!(
                        "Received {} farming patches, {} tile overrides",
                        state.farming_patches.len(),
                        state.ground_tile_overrides.len()
                    );
                }
            }
        }

        "patchStateUpdate" => {
            if let Some(value) = data {
                let patch_id = extract_string(value, "patch_id").unwrap_or_default();
                let patch_state =
                    extract_string(value, "state").unwrap_or_else(|| "empty".to_string());
                let crop_id = extract_string(value, "crop_id").unwrap_or_default();
                let growth_stage = extract_u32(value, "growth_stage").unwrap_or(0);
                let owner_id = extract_string(value, "owner_id").unwrap_or_default();

                if let Some(patch) = state.farming_patches.get_mut(&patch_id) {
                    // Detect harvest: was harvestable, now empty
                    if patch.state == "harvestable" && patch_state == "empty" {
                        state.pending_sfx.push("pop".to_string());
                    }
                    patch.state = patch_state;
                    patch.crop_id = crop_id;
                    patch.growth_stage = growth_stage;
                    patch.owner_id = owner_id;
                }
            }
        }

        "farmingContractUpdate" => {
            if let Some(value) = data {
                let active = extract_bool(value, "active").unwrap_or(false);
                if active {
                    state.farming_contract = Some(crate::game::FarmingContractInfo {
                        difficulty: extract_string(value, "difficulty").unwrap_or_default(),
                        crop_name: extract_string(value, "crop_name").unwrap_or_default(),
                        amount_required: extract_i32(value, "amount_required").unwrap_or(0),
                        amount_harvested: extract_i32(value, "amount_harvested").unwrap_or(0),
                    });
                } else {
                    state.farming_contract = None;
                }
            }
        }

        // =====================================================================
        // Friend System Messages
        // =====================================================================
        "friendsList" => {
            if let Some(value) = data {
                if let Some(friends_array) = extract_array(value, "friends") {
                    state.social_state.friends.clear();
                    for friend_value in friends_array {
                        let id = extract_i32(friend_value, "id").unwrap_or(0) as i64;
                        let name = extract_string(friend_value, "name").unwrap_or_default();
                        let online = extract_bool(friend_value, "online").unwrap_or(false);
                        state.social_state.friends.push(crate::game::FriendInfo {
                            id,
                            name,
                            online,
                        });
                    }
                    // Sort: online friends first, then alphabetical
                    state
                        .social_state
                        .friends
                        .sort_by(|a, b| match (a.online, b.online) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                        });
                    log::info!(
                        "Received friends list: {} friends",
                        state.social_state.friends.len()
                    );
                }
            }
        }

        "pendingFriendRequests" => {
            if let Some(value) = data {
                if let Some(requests_array) = extract_array(value, "requests") {
                    state.social_state.pending_requests.clear();
                    for req_value in requests_array {
                        let from_id = extract_i32(req_value, "from_id").unwrap_or(0) as i64;
                        let from_name = extract_string(req_value, "from_name").unwrap_or_default();
                        state
                            .social_state
                            .pending_requests
                            .push(crate::game::PendingRequestInfo { from_id, from_name });
                    }
                    state.social_state.pending_request_count =
                        state.social_state.pending_requests.len();
                    log::info!(
                        "Received {} pending friend requests",
                        state.social_state.pending_request_count
                    );
                }
            }
        }

        "onlinePlayersList" => {
            if let Some(value) = data {
                if let Some(players_array) = extract_array(value, "players") {
                    state.social_state.online_players.clear();
                    for player_value in players_array {
                        let id = extract_i32(player_value, "id").unwrap_or(0) as i64;
                        let name = extract_string(player_value, "name").unwrap_or_default();
                        let is_friend = extract_bool(player_value, "is_friend").unwrap_or(false);
                        state
                            .social_state
                            .online_players
                            .push(crate::game::OnlinePlayerInfo {
                                id,
                                name,
                                is_friend,
                            });
                    }
                    // Sort alphabetically
                    state
                        .social_state
                        .online_players
                        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                    log::info!(
                        "Received online players list: {} players",
                        state.social_state.online_players.len()
                    );
                }
            }
        }

        "friendRequestReceived" => {
            if let Some(value) = data {
                let from_id = extract_i32(value, "from_id").unwrap_or(0) as i64;
                let from_name = extract_string(value, "from_name").unwrap_or_default();

                // Add to pending requests if not already there
                if !state
                    .social_state
                    .pending_requests
                    .iter()
                    .any(|r| r.from_id == from_id)
                {
                    state
                        .social_state
                        .pending_requests
                        .push(crate::game::PendingRequestInfo {
                            from_id,
                            from_name: from_name.clone(),
                        });
                    state.social_state.pending_request_count =
                        state.social_state.pending_requests.len();
                }
                log::info!("Received friend request from {}", from_name);
            }
        }

        "friendRequestAccepted" => {
            if let Some(value) = data {
                let friend_id = extract_i32(value, "friend_id").unwrap_or(0) as i64;
                let friend_name = extract_string(value, "friend_name").unwrap_or_default();

                // Add to friends list if not already there
                if !state.social_state.friends.iter().any(|f| f.id == friend_id) {
                    state.social_state.friends.push(crate::game::FriendInfo {
                        id: friend_id,
                        name: friend_name.clone(),
                        online: true, // They just accepted, so they're online
                    });
                    // Re-sort friends list
                    state
                        .social_state
                        .friends
                        .sort_by(|a, b| match (a.online, b.online) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                        });
                }
                log::info!("Friend request accepted by {}", friend_name);
            }
        }

        "friendRequestDeclined" => {
            if let Some(value) = data {
                let by_id = extract_i32(value, "by_id").unwrap_or(0) as i64;
                log::info!("Friend request declined by character {}", by_id);
            }
        }

        "friendRemoved" => {
            if let Some(value) = data {
                let friend_id = extract_i32(value, "friend_id").unwrap_or(0) as i64;
                state.social_state.friends.retain(|f| f.id != friend_id);
                log::info!("Friend removed: {}", friend_id);
            }
        }

        "friendStatusChanged" => {
            if let Some(value) = data {
                let friend_id = extract_i32(value, "friend_id").unwrap_or(0) as i64;
                let online = extract_bool(value, "online").unwrap_or(false);

                // Update friend's online status
                if let Some(friend) = state
                    .social_state
                    .friends
                    .iter_mut()
                    .find(|f| f.id == friend_id)
                {
                    friend.online = online;
                    log::info!(
                        "Friend {} is now {}",
                        friend.name,
                        if online { "online" } else { "offline" }
                    );
                }

                // Re-sort friends list
                state
                    .social_state
                    .friends
                    .sort_by(|a, b| match (a.online, b.online) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    });
            }
        }

        "friendActionResult" => {
            if let Some(value) = data {
                let action = extract_string(value, "action").unwrap_or_default();
                let success = extract_bool(value, "success").unwrap_or(false);
                let error = extract_string(value, "error");

                if success {
                    log::info!("Friend action '{}' succeeded", action);
                } else if let Some(err) = error {
                    log::warn!("Friend action '{}' failed: {}", action, err);
                    // Add error to chat as system message
                    state.push_chat_message(ChatMessage::system(err));
                }
            }
        }

        // =====================================================================
        // Prayer System Messages
        // =====================================================================
        "prayerStateUpdate" => {
            if let Some(value) = data {
                let points = extract_i32(value, "points").unwrap_or(0);
                let max_points = extract_i32(value, "max_points").unwrap_or(1);

                // Parse active prayers array
                let mut active_prayers = Vec::new();
                if let Some(prayers_arr) = extract_array(value, "active_prayers") {
                    for prayer_value in prayers_arr {
                        if let Some(prayer_id) = prayer_value.as_str() {
                            active_prayers.push(prayer_id.to_string());
                        }
                    }
                }

                log::info!(
                    "Prayer state update: {}/{} points, {} active prayers",
                    points,
                    max_points,
                    active_prayers.len()
                );

                state.prayer_points = points;
                state.max_prayer_points = max_points;
                state.active_prayers = active_prayers;
            }
        }

        "spellEffect" => {
            if let Some(value) = data {
                let caster_id = extract_string(value, "caster_id").unwrap_or_default();
                let target_id = extract_string(value, "target_id");
                let spell_id = extract_string(value, "spell_id").unwrap_or_default();
                let target_x = extract_i32(value, "target_x").unwrap_or(0);
                let target_y = extract_i32(value, "target_y").unwrap_or(0);

                log::info!(
                    "Spell effect: {} cast {} at ({}, {}), target: {:?}",
                    caster_id,
                    spell_id,
                    target_x,
                    target_y,
                    target_id
                );

                // Trigger casting animation on caster
                if let Some(player) = state.players.get_mut(&caster_id) {
                    player.play_cast();
                }

                // Store for rendering (Task 9)
                state.spell_effects.push(SpellEffect {
                    caster_id,
                    target_id,
                    spell_id,
                    target_x,
                    target_y,
                    time: macroquad::time::get_time(),
                });
            }
        }

        "spellResult" => {
            if let Some(value) = data {
                let success = extract_bool(value, "success").unwrap_or(false);
                let reason = extract_string(value, "reason");

                if !success {
                    if let Some(reason) = &reason {
                        log::info!("Spell cast failed: {}", reason);
                        // Add system chat message for failure feedback
                        state.push_system_chat(format!("Spell failed: {}", reason));
                    }
                }
            }
        }

        "scrollSpellDefinitions" => {
            if let Some(value) = data {
                if let Some(spells_arr) = extract_array(value, "spells") {
                    state.scroll_spell_definitions.clear();
                    for spell_val in spells_arr {
                        let id = extract_string(spell_val, "id").unwrap_or_default();
                        let name = extract_string(spell_val, "name").unwrap_or_default();
                        let spell_type_str = extract_string(spell_val, "spell_type").unwrap_or_default();
                        let spell_type = match spell_type_str.as_str() {
                            "damage" => crate::game::spell::SpellType::Damage,
                            "heal" => crate::game::spell::SpellType::Heal,
                            "teleport" => crate::game::spell::SpellType::Teleport,
                            _ => crate::game::spell::SpellType::Damage,
                        };
                        state.scroll_spell_definitions.push(crate::game::spell::ScrollSpellDef {
                            id,
                            name,
                            spell_type,
                            mana_cost: extract_i32(spell_val, "mana_cost").unwrap_or(0),
                            cooldown_ms: extract_i32(spell_val, "cooldown_ms").unwrap_or(0) as u64,
                            base_power: extract_i32(spell_val, "base_power").unwrap_or(0),
                            effect_sprite: extract_string(spell_val, "effect_sprite").unwrap_or_default(),
                            pushback_distance: extract_i32(spell_val, "pushback_distance").unwrap_or(0),
                            wall_slam_damage_per_tile: extract_i32(spell_val, "wall_slam_damage_per_tile").unwrap_or(0),
                            description: extract_string(spell_val, "description").unwrap_or_default(),
                        });
                    }
                    log::info!("Received {} scroll spell definitions", state.scroll_spell_definitions.len());
                }
            }
        }

        "unlockedSpellsSync" => {
            if let Some(value) = data {
                if let Some(ids_arr) = extract_array(value, "spell_ids") {
                    state.unlocked_spells.clear();
                    for id_val in ids_arr {
                        if let Some(id) = id_val.as_str() {
                            state.unlocked_spells.insert(id.to_string());
                        }
                    }
                    log::info!("Synced {} unlocked spells", state.unlocked_spells.len());
                }
            }
        }

        "spellUnlocked" => {
            if let Some(value) = data {
                if let Some(spell_id) = extract_string(value, "spell_id") {
                    state.unlocked_spells.insert(spell_id.clone());
                    log::info!("Spell unlocked: {}", spell_id);
                }
            }
        }

        "pushback" => {
            if let Some(value) = data {
                let target_id = extract_string(value, "target_id").unwrap_or_default();
                let to_x = extract_i32(value, "to_x").unwrap_or(0);
                let to_y = extract_i32(value, "to_y").unwrap_or(0);

                // Update entity position to final pushback position with smooth slide
                if let Some(player) = state.players.get_mut(&target_id) {
                    player.server_x = to_x as f32;
                    player.server_y = to_y as f32;
                    player.target_x = to_x as f32;
                    player.target_y = to_y as f32;
                    player.is_dashing = true; // Reuse dash slide for fast pushback interpolation
                }
            }
        }

        "pong" => {
            // Handle ping response - calculate and display latency
            if let Some(sent_at) = state.ping_sent_at.take() {
                let now = macroquad::time::get_time();
                let latency_ms = (now - sent_at) * 1000.0;
                state.ping_stats.record(latency_ms);
                state.refresh_high_ping_movement_mode();
                // Only show in chat if it was a manual /ping (not auto-ping)
                if !state.debug_mode {
                    state.push_system_chat(format!(
                            "Ping: {}ms",
                            latency_ms.round() as i32
                        ));
                }
            }
        }

        "slayerPanelOpen" => {
            if let Some(value) = data {
                state.ui_state.slayer_master_id = extract_string(value, "master_id");
                state.ui_state.slayer_master_name = extract_string(value, "master_name");
                state.ui_state.slayer_current_task = extract_slayer_task(value, "current_task");
                state.ui_state.slayer_points = extract_i32(value, "points").unwrap_or(0);
                state.ui_state.slayer_tasks_completed = extract_i32(value, "tasks_completed").unwrap_or(0);
                state.ui_state.slayer_rewards = extract_slayer_rewards(value, "rewards");
                state.ui_state.slayer_blocked_monsters = extract_string_array(value, "blocked_monsters");
                state.ui_state.slayer_unlocked_monsters = extract_string_array(value, "unlocked_monsters");
                state.ui_state.slayer_panel_open = true;
                state.ui_state.slayer_reward_tab = 0;
                state.ui_state.slayer_reward_scroll = 0.0;
                state.pending_sfx.push("ui_open".to_string());
            }
        }

        "slayerTaskProgress" => {
            if let Some(value) = data {
                if let Some(ref mut task) = state.ui_state.slayer_current_task {
                    if let Some(kills) = extract_i32(value, "kills_current") {
                        task.kills_current = kills;
                    }
                    if let Some(kills) = extract_i32(value, "kills_required") {
                        task.kills_required = kills;
                    }
                }
            }
        }

        "slayerTaskComplete" => {
            if let Some(value) = data {
                let display_name = extract_string(value, "display_name").unwrap_or_default();
                let points_awarded = extract_i32(value, "points_awarded").unwrap_or(0);
                let total_points = extract_i32(value, "total_points").unwrap_or(0);
                state.ui_state.slayer_current_task = None;
                state.ui_state.slayer_points = total_points;
                // Add a system message about task completion
                state.push_system_chat(format!(
                        "Slayer task complete! {} - earned {} points (total: {}).",
                        display_name, points_awarded, total_points
                    ));
            }
        }

        "slayerResult" => {
            if let Some(value) = data {
                let _success = extract_bool(value, "success").unwrap_or(false);
                let message = extract_string(value, "message");
                if let Some(task) = extract_slayer_task(value, "task") {
                    state.ui_state.slayer_current_task = Some(task);
                }
                if let Some(points) = extract_i32(value, "points") {
                    state.ui_state.slayer_points = points;
                }
                // Show message in chat
                if let Some(msg) = message {
                    state.push_system_chat(msg);
                }
            }
        }

        "slayerStateSync" => {
            if let Some(value) = data {
                state.ui_state.slayer_current_task = extract_slayer_task(value, "current_task");
                state.ui_state.slayer_points = extract_i32(value, "points").unwrap_or(0);
                state.ui_state.slayer_tasks_completed = extract_i32(value, "tasks_completed").unwrap_or(0);
                state.ui_state.slayer_blocked_monsters = extract_string_array(value, "blocked_monsters");
                state.ui_state.slayer_unlocked_monsters = extract_string_array(value, "unlocked_monsters");
            }
        }

        "autoActionStarted" => {
            if let Some(value) = data {
                let target_type = extract_string(value, "target_type").unwrap_or_default();
                let target_id = extract_string(value, "target_id").unwrap_or_default();
                let action = extract_string(value, "action").unwrap_or_default();
                if let Some(ref mut aa) = state.auto_action_state {
                    aa.confirmed = true;
                }
                log::debug!("Auto-action started: {} {} {}", action, target_type, target_id);
            }
        }

        "autoActionStopped" => {
            if let Some(value) = data {
                let reason = extract_string(value, "reason").unwrap_or_default();
                state.auto_action_state = None;
                // Also clear auto-path if we were chasing
                state.auto_path = None;
                log::debug!("Auto-action stopped: {}", reason);
            }
        }

        "error" => {
            if let Some(value) = data {
                let message = extract_string(value, "message").unwrap_or_default();
                log::warn!("Server error: {}", message);
                state.push_system_chat(message);
                state.pending_sfx.push("error".to_string());
            }
        }

        // ===== Trade System Messages =====
        "tradeRequestReceived" => {
            if let Some(value) = data {
                let requester_id = extract_string(value, "requester_id").unwrap_or_default();
                let requester_name = extract_string(value, "requester_name").unwrap_or_default();
                state.ui_state.trade_pending_request = Some((requester_id, requester_name.clone()));
                state.push_system_chat(format!("{} wants to trade with you.", requester_name));
            }
        }

        "tradeOpened" => {
            if let Some(value) = data {
                let partner_id = extract_string(value, "partner_id").unwrap_or_default();
                let partner_name = extract_string(value, "partner_name").unwrap_or_default();
                state.ui_state.trade_open = true;
                state.ui_state.inventory_open = true;
                state.ui_state.trade_partner_id = Some(partner_id);
                state.ui_state.trade_partner_name = Some(partner_name);
                state.ui_state.trade_my_items.clear();
                state.ui_state.trade_my_gold = 0;
                state.ui_state.trade_my_accepted = false;
                state.ui_state.trade_partner_items.clear();
                state.ui_state.trade_partner_gold = 0;
                state.ui_state.trade_partner_accepted = false;
                state.ui_state.trade_pending_request = None;
                state.pending_sfx.push("ui_open".to_string());
            }
        }

        "tradeOfferUpdate" => {
            if let Some(value) = data {
                // Parse partner's items
                let mut items = Vec::new();
                if let Some(arr) = extract_map_field(value, "partner_items") {
                    if let rmpv::Value::Array(ref list) = *arr {
                        for item in list {
                            let slot_index = extract_u8(item, "slot_index").unwrap_or(0);
                            let item_id = extract_string(item, "item_id").unwrap_or_default();
                            let quantity = extract_i32(item, "quantity").unwrap_or(1);
                            items.push(crate::game::TradeOfferItem { slot_index, item_id, quantity });
                        }
                    }
                }
                state.ui_state.trade_partner_items = items;
                state.ui_state.trade_partner_gold = extract_i32(value, "partner_gold").unwrap_or(0);
                state.ui_state.trade_partner_accepted = extract_bool(value, "partner_accepted").unwrap_or(false);
                // If partner changed offer, reset our accepted state
                state.ui_state.trade_my_accepted = false;
            }
        }

        "tradeMyOfferUpdate" => {
            if let Some(value) = data {
                let mut items = Vec::new();
                if let Some(arr) = extract_map_field(value, "my_items") {
                    if let rmpv::Value::Array(ref list) = *arr {
                        for item in list {
                            let slot_index = extract_u8(item, "slot_index").unwrap_or(0);
                            let item_id = extract_string(item, "item_id").unwrap_or_default();
                            let quantity = extract_i32(item, "quantity").unwrap_or(1);
                            items.push(crate::game::TradeOfferItem { slot_index, item_id, quantity });
                        }
                    }
                }
                state.ui_state.trade_my_items = items;
                state.ui_state.trade_my_gold = extract_i32(value, "my_gold").unwrap_or(0);
                state.ui_state.trade_my_accepted = extract_bool(value, "my_accepted").unwrap_or(false);
            }
        }

        "tradeCompleted" => {
            state.ui_state.trade_open = false;
            state.ui_state.trade_partner_id = None;
            state.ui_state.trade_partner_name = None;
            state.ui_state.trade_my_items.clear();
            state.ui_state.trade_partner_items.clear();
            state.push_system_chat("Trade completed successfully!".to_string());
            state.pending_sfx.push("ui_close".to_string());
        }

        "tradeCancelled" => {
            if let Some(value) = data {
                let reason = extract_string(value, "reason").unwrap_or_else(|| "Trade cancelled.".to_string());
                state.push_system_chat(reason);
            }
            state.ui_state.trade_open = false;
            state.ui_state.trade_partner_id = None;
            state.ui_state.trade_partner_name = None;
            state.ui_state.trade_my_items.clear();
            state.ui_state.trade_partner_items.clear();
            state.ui_state.trade_pending_request = None;
            state.pending_sfx.push("ui_close".to_string());
        }

        // ===== Stall System Messages =====
        "stallOpened" => {
            if let Some(value) = data {
                let name = extract_string(value, "name").unwrap_or_default();
                state.ui_state.stall_active = true;
                state.ui_state.stall_my_name = name;
                state.ui_state.stall_setup_open = true;
                state.ui_state.inventory_open = true;
                state.push_system_chat("Your shop is now open!".to_string());
            }
        }

        "stallClosed" => {
            state.ui_state.stall_active = false;
            state.ui_state.stall_my_slots.clear();
            state.ui_state.stall_setup_open = false;
            state.push_system_chat("Your shop has been closed.".to_string());
        }

        "stallUpdate" => {
            if let Some(value) = data {
                let mut slots = Vec::new();
                if let Some(arr) = extract_map_field(value, "slots") {
                    if let rmpv::Value::Array(ref list) = *arr {
                        for item in list {
                            let slot = extract_u8(item, "slot").unwrap_or(0);
                            let item_id = extract_string(item, "item_id").unwrap_or_default();
                            let quantity = extract_i32(item, "quantity").unwrap_or(1);
                            let price = extract_i32(item, "price").unwrap_or(0);
                            slots.push(crate::game::StallSlotInfo { slot, item_id, quantity, price });
                        }
                    }
                }
                state.ui_state.stall_my_slots = slots;
            }
        }

        "stallBrowseData" => {
            if let Some(value) = data {
                let seller_id = extract_string(value, "seller_id").unwrap_or_default();
                let seller_name = extract_string(value, "seller_name").unwrap_or_default();
                let stall_name = extract_string(value, "stall_name").unwrap_or_default();
                let mut items = Vec::new();
                if let Some(arr) = extract_map_field(value, "items") {
                    if let rmpv::Value::Array(ref list) = *arr {
                        for item in list {
                            let slot = extract_u8(item, "slot").unwrap_or(0);
                            let item_id = extract_string(item, "item_id").unwrap_or_default();
                            let quantity = extract_i32(item, "quantity").unwrap_or(1);
                            let price = extract_i32(item, "price").unwrap_or(0);
                            items.push(crate::game::StallSlotInfo { slot, item_id, quantity, price });
                        }
                    }
                }
                state.ui_state.stall_browse = Some(crate::game::StallBrowseInfo {
                    seller_id,
                    seller_name,
                    stall_name,
                    items,
                });
                state.ui_state.stall_buy_quantity = 1;
                state.ui_state.stall_browse_selected = 0;
                state.pending_sfx.push("ui_open".to_string());
            }
        }

        "stallBuyResult" => {
            if let Some(value) = data {
                let success = extract_bool(value, "success").unwrap_or(false);
                let message = extract_string(value, "message").unwrap_or_default();
                if success {
                    state.push_system_chat(format!("Purchase successful: {}", message));
                } else {
                    state.push_system_chat(format!("Purchase failed: {}", message));
                }
            }
        }

        "stallSaleNotification" => {
            if let Some(value) = data {
                let buyer_name = extract_string(value, "buyer_name").unwrap_or_default();
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                let quantity = extract_i32(value, "quantity").unwrap_or(1);
                let gold_earned = extract_i32(value, "gold_earned").unwrap_or(0);
                let item_def = state.item_registry.get_or_placeholder(&item_id);
                state.push_system_chat(format!(
                    "{} bought {}x {} from your shop for {}g!",
                    buyer_name, quantity, item_def.display_name, gold_earned
                ));
            }
        }

        "stallItemUpdate" => {
            if let Some(value) = data {
                let slot = extract_u8(value, "slot").unwrap_or(0);
                let quantity = extract_i32(value, "quantity").unwrap_or(0);
                // Update browse data if open
                if let Some(ref mut browse) = state.ui_state.stall_browse {
                    if let Some(item) = browse.items.iter_mut().find(|i| i.slot == slot) {
                        if quantity <= 0 {
                            browse.items.retain(|i| i.slot != slot);
                        } else {
                            item.quantity = quantity;
                        }
                    }
                }
            }
        }

        _ => {
            log::debug!("Unhandled message type: {}", msg_type);
        }
    }
}
