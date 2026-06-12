use super::protocol::{
    extract_array, extract_bool, extract_f32, extract_i32, extract_string, extract_u32,
    extract_u64, extract_u8,
};
use crate::game::npc::{Npc, NpcState};
use crate::game::world_map::{
    WORLD_MAP_POI_KIND_CHEST, WORLD_MAP_POI_KIND_QUEST, WORLD_MAP_POI_KIND_TELEPORT,
    WORLD_MAP_POI_KIND_TREE,
};
use crate::game::{
    ActiveDialogue, ActivePotionBuff, ActiveQuest, AdventureBoardActiveContractInfo,
    AdventureBoardDifficultyInfo, AdventureBoardOfferInfo, AdventureBoardPanelState,
    AdventureBoardStatsInfo, CatalogObjective, ChatBubble, ChatChannel, ChatMessage,
    ConnectionStatus, CraftingOrderActiveInfo, CraftingOrderItemInfo, CraftingOrderOfferInfo,
    CraftingOrderStatsInfo, DamageEvent, DialogueChoice, Direction, EquipmentStats, FarmingPatch,
    GameState, GatheringBuff, GatheringMarker, GroundItem, InventorySlot, ItemDefinition,
    LevelUpEvent, MapObject, Player, Portal, QuestCatalogEntry, QuestCompletedEvent,
    QuestObjective, RecipeDefinition, RecipeIngredient, RecipeResult, ShopData, ShopStockItem,
    SkillType, SkillXpEvent, SpellEffect, TransitionState, Wall, WallEdge, WorldMapBounds,
    WorldMapChunkSample, WorldMapPoi, WorldMapSnapshot,
};
use crate::render::animation::{NpcAnimationLayout, NpcAnimationState};
use crate::render::OVERWORLD_NAME;

/// Max tile distance from local player to play other players' SFX.
/// Roughly matches the visible screen area so you don't hear off-screen actions.
const SFX_AUDIBLE_RANGE: f32 = 20.0;

fn current_time() -> f64 {
    #[cfg(test)]
    {
        0.0
    }

    #[cfg(not(test))]
    {
        macroquad::time::get_time()
    }
}

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

fn clear_adventure_board_dialogue(state: &mut GameState) {
    state.ui_state.adventure_board = None;
    state.ui_state.adventure_board_selected_offer = 0;
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

fn extract_slayer_task(
    value: &rmpv::Value,
    key: &str,
) -> Option<crate::game::slayer::SlayerTaskClientData> {
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

fn extract_slayer_rewards(
    value: &rmpv::Value,
    key: &str,
) -> Vec<crate::game::slayer::SlayerRewardClientData> {
    let mut rewards = Vec::new();
    if let Some(rmpv::Value::Array(items)) = extract_map_field(value, key) {
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
    rewards
}

fn extract_string_array(value: &rmpv::Value, key: &str) -> Vec<String> {
    let mut result = Vec::new();
    if let Some(rmpv::Value::Array(items)) = extract_map_field(value, key) {
        for item in items {
            if let Some(s) = item.as_str() {
                result.push(s.to_string());
            }
        }
    }
    result
}

fn extract_blockable_monsters(value: &rmpv::Value, key: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    if let Some(rmpv::Value::Array(items)) = extract_map_field(value, key) {
        for item in items {
            let id = extract_string(item, "id").unwrap_or_default();
            let name = extract_string(item, "name").unwrap_or_default();
            if !id.is_empty() {
                result.push((id, name));
            }
        }
    }
    result
}

fn handle_welcome(value: &rmpv::Value, state: &mut GameState) {
    let protocol_version = extract_u32(value, "protocol_version").unwrap_or(0);
    if protocol_version != u32::from(aeven_protocol::PROTOCOL_VERSION) {
        log::error!(
            "Incompatible protocol version: server={}, client={}",
            protocol_version,
            aeven_protocol::PROTOCOL_VERSION
        );
        state.connection_status = ConnectionStatus::Disconnected;
        state.disconnect_requested = true;
        return;
    }

    if let Some(player_id) = extract_string(value, "player_id") {
        log::info!("Welcome! Player ID: {}", player_id);
        state.local_player_id = Some(player_id);
        state.reset_move_sequence_state();
        state.connection_status = ConnectionStatus::Connected;

        let is_new = extract_bool(value, "is_new_character").unwrap_or(false);
        let tutorial_done = crate::settings::load_tutorial_completed();
        log::warn!("TUTORIAL: Welcome data={:?}", value);
        log::warn!(
            "TUTORIAL: is_new={}, tutorial_done={}",
            is_new,
            tutorial_done
        );
        if is_new && !tutorial_done {
            log::info!("Tutorial: setting tutorial_pending = true");
            state.tutorial_pending = true;
        }
    }
}

fn handle_player_joined(value: &rmpv::Value, state: &mut GameState) {
    let id = extract_string(value, "id").unwrap_or_default();
    let name = extract_string(value, "name").unwrap_or_default();
    let x = extract_i32(value, "x").unwrap_or(0) as f32;
    let y = extract_i32(value, "y").unwrap_or(0) as f32;
    let gender = extract_string(value, "gender").unwrap_or_else(|| "male".to_string());
    let skin = extract_string(value, "skin").unwrap_or_else(|| "tan".to_string());
    let hair_style = extract_i32(value, "hair_style");
    let hair_color = extract_i32(value, "hair_color");
    let equipped_head = extract_string(value, "equipped_head").filter(|s| !s.is_empty());
    let equipped_body = extract_string(value, "equipped_body").filter(|s| !s.is_empty());
    let equipped_weapon = extract_string(value, "equipped_weapon").filter(|s| !s.is_empty());
    let equipped_back = extract_string(value, "equipped_back").filter(|s| !s.is_empty());
    let equipped_feet = extract_string(value, "equipped_feet").filter(|s| !s.is_empty());
    let equipped_ring = extract_string(value, "equipped_ring").filter(|s| !s.is_empty());
    let equipped_gloves = extract_string(value, "equipped_gloves").filter(|s| !s.is_empty());
    let equipped_necklace = extract_string(value, "equipped_necklace").filter(|s| !s.is_empty());
    let equipped_belt = extract_string(value, "equipped_belt").filter(|s| !s.is_empty());
    let is_admin = extract_bool(value, "is_admin").unwrap_or(false);

    log::info!(
        "Player joined: {} at ({}, {}) [{}/{}]",
        name,
        x,
        y,
        gender,
        skin
    );
    let z = extract_i32(value, "z").unwrap_or(0);
    let mut player = Player::new(id.clone(), name, x, y, gender, skin);
    // Snap Z immediately so players on elevated platforms render correctly
    player.z = z as f32;
    player.server_z = z as f32;
    player.target_z = z as f32;
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

fn handle_player_left(value: &rmpv::Value, state: &mut GameState) {
    if let Some(id) = extract_string(value, "id") {
        log::info!("Player left: {}", id);
        state.players.remove(&id);
    }
}

fn handle_chat_message(value: &rmpv::Value, state: &mut GameState) {
    let sender_name = extract_string(value, "senderName").unwrap_or_default();
    let text = extract_string(value, "text").unwrap_or_default();
    let extracted_ts = extract_u64(value, "timestamp").unwrap_or(0) as f64;
    let timestamp = if extracted_ts > 0.0 {
        extracted_ts
    } else {
        current_time()
    };
    let channel_str = extract_string(value, "channel").unwrap_or_default();

    let channel = match channel_str.as_str() {
        "global" => ChatChannel::Global,
        "system" => ChatChannel::System,
        _ => ChatChannel::Local,
    };

    state.push_chat_message(ChatMessage {
        sender_name: sender_name.clone(),
        text: text.clone(),
        timestamp,
        channel,
    });
    state.pending_sfx.push("message_add".to_string());

    if matches!(channel, ChatChannel::Local) {
        let sender_id = extract_string(value, "senderId").unwrap_or_default();
        if !sender_id.is_empty() && state.players.contains_key(&sender_id) {
            state.chat_bubbles.retain(|b| b.player_id != sender_id);
            state.chat_bubbles.push(ChatBubble {
                player_id: sender_id,
                text,
                time: current_time(),
            });
        }
    }
}

fn handle_target_changed(value: &rmpv::Value, state: &mut GameState) {
    let player_id = extract_string(value, "player_id").unwrap_or_default();
    let target_id = extract_string(value, "target_id");

    if state.local_player_id.as_ref() == Some(&player_id) {
        state.selected_entity_id = target_id.clone();
        log::debug!("Target changed to: {:?}", state.selected_entity_id);
    }
}

fn handle_player_respawned(value: &rmpv::Value, state: &mut GameState) {
    let player_id = extract_string(value, "id").unwrap_or_default();
    let x = extract_i32(value, "x").unwrap_or(0) as f32;
    let y = extract_i32(value, "y").unwrap_or(0) as f32;
    let hp = extract_i32(value, "hp").unwrap_or(100);
    log::info!("Player {} respawned at ({}, {})", player_id, x, y);

    if let Some(player) = state.players.get_mut(&player_id) {
        player.respawn(x, y, hp);
    }

    if state.local_player_id.as_ref() == Some(&player_id) {
        state.is_sitting = false;
        state.clear_pending_moves();
        if state.chunk_manager.is_interior() {
            state.chunk_manager.clear_interior();
            state.current_interior = None;
            state.current_instance = None;
            state.npcs.clear();
            state.ground_items.clear();
        }
    }
}

fn handle_inventory_update(value: &rmpv::Value, state: &mut GameState) {
    for slot in state.inventory.slots.iter_mut() {
        *slot = None;
    }

    if let Some(slots) = extract_array(value, "slots") {
        for slot_value in slots {
            let slot_idx = extract_u8(slot_value, "slot").unwrap_or(0) as usize;
            let item_id = extract_string(slot_value, "item_id").unwrap_or_default();
            let quantity = extract_i32(slot_value, "quantity").unwrap_or(0);

            if slot_idx < state.inventory.slots.len() && !item_id.is_empty() && quantity > 0 {
                state.inventory.slots[slot_idx] = Some(InventorySlot::new(item_id, quantity));
            }
        }
    }

    if let Some(gold) = extract_i32(value, "gold") {
        state.inventory.gold = gold;
    }

    log::debug!(
        "Inventory updated: {} gold, {} items",
        state.inventory.gold,
        state.inventory.slots.iter().filter(|s| s.is_some()).count()
    );
}

fn handle_state_sync(value: &rmpv::Value, state: &mut GameState) {
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
            let z = extract_i32(player_value, "z").unwrap_or(0);
            let direction = extract_i32(player_value, "direction");
            let hp = extract_i32(player_value, "hp");
            let max_hp = extract_i32(player_value, "maxHp");
            let mp = extract_i32(player_value, "mp");
            let max_mp = extract_i32(player_value, "maxMp");
            // Skill levels
            let hitpoints_level = extract_i32(player_value, "hitpointsLevel");
            let attack_level = extract_i32(player_value, "attackLevel");
            let strength_level = extract_i32(player_value, "strengthLevel");
            let defence_level = extract_i32(player_value, "defenceLevel");
            let ranged_level = extract_i32(player_value, "rangedLevel");
            let combat_style = extract_string(player_value, "combatStyle");
            let gold = extract_i32(player_value, "gold");
            let gender =
                extract_string(player_value, "gender").unwrap_or_else(|| "male".to_string());
            let skin = extract_string(player_value, "skin").unwrap_or_else(|| "tan".to_string());
            let hair_style = extract_i32(player_value, "hair_style");
            let hair_color = extract_i32(player_value, "hair_color");
            let equipped_head =
                extract_string(player_value, "equipped_head").filter(|s| !s.is_empty());
            let equipped_body =
                extract_string(player_value, "equipped_body").filter(|s| !s.is_empty());
            let equipped_weapon =
                extract_string(player_value, "equipped_weapon").filter(|s| !s.is_empty());
            let equipped_back =
                extract_string(player_value, "equipped_back").filter(|s| !s.is_empty());
            let equipped_feet =
                extract_string(player_value, "equipped_feet").filter(|s| !s.is_empty());
            let equipped_ring =
                extract_string(player_value, "equipped_ring").filter(|s| !s.is_empty());
            let equipped_gloves =
                extract_string(player_value, "equipped_gloves").filter(|s| !s.is_empty());
            let equipped_necklace =
                extract_string(player_value, "equipped_necklace").filter(|s| !s.is_empty());
            let equipped_belt =
                extract_string(player_value, "equipped_belt").filter(|s| !s.is_empty());
            let is_admin = extract_bool(player_value, "is_admin").unwrap_or(false);
            let title = extract_string(player_value, "title");
            let has_stall = extract_bool(player_value, "has_stall").unwrap_or(false);
            let stall_name = extract_string(player_value, "stall_name");
            let move_ack_seq = extract_u32(player_value, "moveAckSeq");

            let is_local_player = state.local_player_id.as_ref() == Some(&id);
            if is_local_player {
                if let Some(ack_seq) = move_ack_seq {
                    state.acknowledge_move_sequence(ack_seq);
                }
            }
            let has_pending_local_moves = is_local_player && state.has_pending_move_sequences();

            if let Some(player) = state.players.get_mut(&id) {
                // Track last tick this player appeared in a sync (for staleness detection)
                player.last_sync_tick = tick;
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
                    if is_local_player
                        && (vel_x != 0.0
                            || vel_y != 0.0
                            || player.vel_x != 0.0
                            || player.vel_y != 0.0)
                    {
                        macroquad::logging::info!(
                            "[SYNC] pos=({},{}) z={} vel=({},{}) ack={:?}",
                            x,
                            y,
                            z,
                            vel_x,
                            vel_y,
                            move_ack_seq
                        );
                    }
                    player.set_server_state(
                        x as f32,
                        y as f32,
                        z,
                        vel_x,
                        vel_y,
                        dir,
                        is_local_player,
                        has_pending_local_moves,
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
                        player_regen_events.push((id.clone(), player.x, player.y, heal_amount));
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
                if let Some(level) = attack_level {
                    player.skills.attack.level = level;
                }
                if let Some(level) = strength_level {
                    player.skills.strength.level = level;
                }
                if let Some(level) = defence_level {
                    player.skills.defence.level = level;
                }
                if let Some(level) = ranged_level {
                    player.skills.ranged.level = level;
                }
                if let Some(ref style) = combat_style {
                    player.combat_style = style.clone();
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
                // Update title
                player.title = title.clone();
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
                    let mut new_player =
                        Player::new(id.clone(), name.clone(), px as f32, py as f32, gender, skin);
                    // Snap Z immediately so players on elevated platforms don't
                    // briefly appear at ground level when first created.
                    new_player.z = z as f32;
                    new_player.server_z = z as f32;
                    new_player.target_z = z as f32;
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
                    new_player.title = title;
                    new_player.has_stall = has_stall;
                    new_player.stall_name = stall_name;
                    let sitting = extract_bool(player_value, "sitting").unwrap_or(false);
                    if sitting {
                        new_player.sit_chair();
                    }
                    let is_gathering = extract_bool(player_value, "is_gathering").unwrap_or(false);
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
                    let is_mining = extract_bool(player_value, "is_mining").unwrap_or(false);
                    if is_mining {
                        new_player.is_mining = true;
                        new_player.mining_started_at = macroquad::time::get_time();
                    }
                    let dashing = extract_bool(player_value, "dashing").unwrap_or(false);
                    if dashing {
                        new_player.is_dashing = true;
                    }
                    new_player.last_sync_tick = tick;
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

    // Overworld staleness cleanup: remove players that haven't appeared in any
    // StateSync for 40+ ticks (2 seconds). This catches ghost players left behind
    // when a PlayerLeft message is dropped (e.g. channel full during portal transition).
    // We can't do simple full-sync reconciliation in the overworld because view-distance
    // culling means absent players may just be far away, not gone.
    if sync_instance.is_empty() && tick > 40 {
        let stale_threshold = tick - 40;
        let local_id = state.local_player_id.clone().unwrap_or_default();
        state.players.retain(|id, player| {
            if *id == local_id {
                return true;
            }
            if player.last_sync_tick > 0 && player.last_sync_tick < stale_threshold {
                log::info!(
                    "Removing stale player {} (last seen tick {}, current {})",
                    id,
                    player.last_sync_tick,
                    tick
                );
                return false;
            }
            true
        });
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
            let moved_tiles = prev_tile.is_some_and(|prev| prev != current_tile);

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
                && (state.portal_ignore_tile != Some(current_tile))
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
    let mut synced_npc_ids: Vec<String> = Vec::new();
    if let Some(npcs) = extract_array(value, "npcs") {
        for npc_value in npcs {
            let id = extract_string(npc_value, "id").unwrap_or_default();
            synced_npc_ids.push(id.clone());
            let _npc_type = extract_u8(npc_value, "npc_type").unwrap_or(0);
            let entity_type =
                extract_string(npc_value, "entity_type").unwrap_or_else(|| "pig".to_string());
            let display_name =
                extract_string(npc_value, "display_name").unwrap_or_else(|| "???".to_string());
            // Server sends i32 grid positions
            let x = extract_i32(npc_value, "x").unwrap_or(0) as f32;
            let y = extract_i32(npc_value, "y").unwrap_or(0) as f32;
            let npc_z = extract_i32(npc_value, "z").unwrap_or(0) as f32;
            let direction = extract_u8(npc_value, "direction").unwrap_or(0);
            let hp = extract_i32(npc_value, "hp").unwrap_or(50);
            let max_hp = extract_i32(npc_value, "max_hp").unwrap_or(50);
            let level = extract_i32(npc_value, "level").unwrap_or(1);
            let npc_state = extract_u8(npc_value, "state").unwrap_or(0);
            let hostile = extract_bool(npc_value, "hostile").unwrap_or(true);
            let is_quest_giver = extract_bool(npc_value, "is_quest_giver").unwrap_or(false);
            let can_turn_in_quest = extract_bool(npc_value, "can_turn_in_quest").unwrap_or(false);
            let is_merchant = extract_bool(npc_value, "is_merchant").unwrap_or(false);
            let is_altar = extract_bool(npc_value, "is_altar").unwrap_or(false);
            let is_banker = extract_bool(npc_value, "is_banker").unwrap_or(false);
            let is_slayer_master = extract_bool(npc_value, "is_slayer_master").unwrap_or(false);
            let is_friendly = extract_bool(npc_value, "is_friendly").unwrap_or(false);
            let is_port_master = extract_bool(npc_value, "is_port_master").unwrap_or(false);
            let station_type = extract_string(npc_value, "station_type");
            let move_speed = extract_f32(npc_value, "move_speed").unwrap_or(2.0);
            let no_shadow = extract_bool(npc_value, "no_shadow").unwrap_or(false);
            let render_offset_y = extract_f32(npc_value, "render_offset_y").unwrap_or(0.0);
            let size = extract_i32(npc_value, "size").unwrap_or(1);

            if let Some(npc) = state.npcs.get_mut(&id) {
                // Update existing NPC - interpolate toward new grid position
                npc.set_server_position(x, y);
                npc.server_z = npc_z;
                npc.target_z = npc_z;
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
                npc.is_port_master = is_port_master;
                npc.station_type = station_type;
                npc.move_speed = move_speed;
                npc.no_shadow = no_shadow;
                npc.render_offset_y = render_offset_y;
                npc.size = size;
            } else {
                // New NPC - add to state
                let mut npc = Npc::new(id.clone(), entity_type, x, y);
                if npc.entity_type == "big_wurm" {
                    npc.animation.layout = NpcAnimationLayout::BossWurm;
                } else if npc.entity_type == "rockexplode" {
                    npc.animation.layout = NpcAnimationLayout::ExplodingRock;
                }
                // Snap Z immediately so NPCs on elevated platforms render correctly
                npc.z = npc_z;
                npc.server_z = npc_z;
                npc.target_z = npc_z;
                npc.display_name = display_name;
                npc.direction = Direction::from_u8(direction);
                npc.hp = hp;
                npc.max_hp = max_hp;
                npc.level = level;
                npc.state = NpcState::from_u8(npc_state);
                // Sync animation state immediately to avoid a flash of idle frame
                match npc.state {
                    NpcState::Submerging => npc.animation.set_state(NpcAnimationState::Submerging),
                    NpcState::Emerging => npc.animation.set_state(NpcAnimationState::Emerging),
                    NpcState::Burrowing => npc.animation.set_state(NpcAnimationState::Burrowing),
                    NpcState::Dead => {
                        npc.start_death();
                    }
                    _ => {}
                }
                npc.hostile = hostile;
                npc.is_quest_giver = is_quest_giver;
                npc.can_turn_in_quest = can_turn_in_quest;
                npc.is_merchant = is_merchant;
                npc.is_altar = is_altar;
                npc.is_banker = is_banker;
                npc.is_slayer_master = is_slayer_master;
                npc.is_friendly = is_friendly;
                npc.is_port_master = is_port_master;
                npc.station_type = station_type;
                npc.move_speed = move_speed;
                npc.no_shadow = no_shadow;
                npc.render_offset_y = render_offset_y;
                npc.size = size;
                state.npcs.insert(id, npc);
            }
        }
    }

    // Delta sync: process explicit NPC removal list
    if !is_full_sync {
        if let Some(removed) = extract_array(value, "removedNpcs") {
            for rv in removed {
                if let Some(id) = rv.as_str() {
                    // Keep dying NPCs so their death animation can finish
                    let is_dying = state.npcs.get(id).map(|n| n.is_dying()).unwrap_or(false);
                    if !is_dying {
                        state.npcs.remove(id);
                    }
                }
            }
        }
    }

    // Instance full sync: remove NPCs no longer present in the server's list
    if is_full_sync && !sync_instance.is_empty() {
        state
            .npcs
            .retain(|id, npc| synced_npc_ids.contains(id) || npc.is_dying());
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

pub fn handle_room_data(msg_type: &str, data: Option<&rmpv::Value>, state: &mut GameState) {
    let handled = session_combat::handle(msg_type, data, state)
        || inventory_world::handle(msg_type, data, state)
        || quests_content::handle(msg_type, data, state)
        || crafting_commerce::handle(msg_type, data, state)
        || world_gathering::handle(msg_type, data, state)
        || social_magic::handle(msg_type, data, state)
        || activities::handle(msg_type, data, state);
    if !handled {
        log::debug!("Unhandled message type: {}", msg_type);
    }
}

mod activities;
mod crafting_commerce;
mod inventory_world;
mod quests_content;
mod session_combat;
mod social_magic;
mod world_gathering;

#[cfg(test)]
mod tests {
    use super::*;
    use rmpv::Value;

    fn map(entries: Vec<(&str, Value)>) -> Value {
        Value::Map(
            entries
                .into_iter()
                .map(|(key, value)| (Value::from(key), value))
                .collect(),
        )
    }

    fn player(id: &str, name: &str) -> Player {
        Player::new(
            id.to_string(),
            name.to_string(),
            10.0,
            20.0,
            "female".to_string(),
            "pale".to_string(),
        )
    }

    #[test]
    fn welcome_sets_connection_and_resets_move_sequences() {
        let mut state = GameState::new();
        state.next_move_sequence(1.0, 0.0);
        state.next_move_sequence(0.0, 1.0);
        assert!(state.has_pending_move_sequences());

        let welcome = map(vec![
            ("player_id", Value::from("player-1")),
            ("is_new_character", Value::from(false)),
            (
                "protocol_version",
                Value::from(aeven_protocol::PROTOCOL_VERSION),
            ),
        ]);

        handle_room_data("welcome", Some(&welcome), &mut state);

        assert_eq!(state.local_player_id.as_deref(), Some("player-1"));
        assert!(matches!(
            state.connection_status,
            ConnectionStatus::Connected
        ));
        assert_eq!(state.next_move_seq, 0);
        assert_eq!(state.last_acked_move_seq, 0);
        assert!(!state.has_pending_move_sequences());
        assert!(!state.tutorial_pending);
    }

    #[test]
    fn welcome_rejects_an_incompatible_protocol() {
        let mut state = GameState::new();
        let welcome = map(vec![
            ("player_id", Value::from("player-1")),
            ("is_new_character", Value::from(false)),
            ("protocol_version", Value::from(999)),
        ]);

        handle_room_data("welcome", Some(&welcome), &mut state);

        assert!(state.local_player_id.is_none());
        assert!(state.disconnect_requested);
        assert_eq!(state.connection_status, ConnectionStatus::Disconnected);
    }

    #[test]
    fn player_joined_populates_player_appearance_and_equipment() {
        let mut state = GameState::new();
        let joined = map(vec![
            ("id", Value::from("remote-1")),
            ("name", Value::from("Ayla")),
            ("x", Value::from(12)),
            ("y", Value::from(34)),
            ("gender", Value::from("female")),
            ("skin", Value::from("pale")),
            ("hair_style", Value::from(3)),
            ("hair_color", Value::from(5)),
            ("equipped_head", Value::from("wizard_hat")),
            ("equipped_weapon", Value::from("oak_staff")),
            ("equipped_body", Value::from("")),
            ("equipped_back", Value::from("")),
            ("equipped_feet", Value::from("")),
            ("equipped_ring", Value::from("")),
            ("equipped_gloves", Value::from("")),
            ("equipped_necklace", Value::from("")),
            ("equipped_belt", Value::from("")),
            ("is_admin", Value::from(true)),
        ]);

        handle_room_data("playerJoined", Some(&joined), &mut state);

        let player = state.players.get("remote-1").expect("player inserted");
        assert_eq!(player.name, "Ayla");
        assert_eq!(player.x, 12.0);
        assert_eq!(player.y, 34.0);
        assert_eq!(player.gender, "female");
        assert_eq!(player.skin, "pale");
        assert_eq!(player.hair_style, Some(3));
        assert_eq!(player.hair_color, Some(5));
        assert_eq!(player.equipped_head.as_deref(), Some("wizard_hat"));
        assert_eq!(player.equipped_weapon.as_deref(), Some("oak_staff"));
        assert!(player.is_admin);
    }

    #[test]
    fn player_left_removes_existing_player() {
        let mut state = GameState::new();
        state
            .players
            .insert("remote-1".to_string(), player("remote-1", "Ayla"));

        let left = map(vec![("id", Value::from("remote-1"))]);
        handle_room_data("playerLeft", Some(&left), &mut state);

        assert!(!state.players.contains_key("remote-1"));
    }

    #[test]
    fn chat_message_adds_local_log_entry_bubble_and_sfx() {
        let mut state = GameState::new();
        state
            .players
            .insert("remote-1".to_string(), player("remote-1", "Ayla"));

        let chat = map(vec![
            ("senderId", Value::from("remote-1")),
            ("senderName", Value::from("Ayla")),
            ("text", Value::from("Hello there")),
            ("timestamp", Value::from(123_u64)),
            ("channel", Value::from("public")),
        ]);

        handle_room_data("chatMessage", Some(&chat), &mut state);

        let local_messages = state.ui_state.chat_messages.channel(&ChatChannel::Local);
        assert_eq!(local_messages.len(), 1);
        assert_eq!(local_messages[0].sender_name, "Ayla");
        assert_eq!(local_messages[0].text, "Hello there");
        assert_eq!(local_messages[0].timestamp, 123.0);
        assert_eq!(local_messages[0].channel, ChatChannel::Local);
        assert_eq!(state.chat_bubbles.len(), 1);
        assert_eq!(state.chat_bubbles[0].player_id, "remote-1");
        assert_eq!(state.chat_bubbles[0].text, "Hello there");
        assert_eq!(state.pending_sfx, vec!["message_add".to_string()]);
    }

    #[test]
    fn target_changed_only_updates_local_player_selection() {
        let mut state = GameState::new();
        state.local_player_id = Some("local-1".to_string());
        state.selected_entity_id = Some("old-target".to_string());

        let remote_target = map(vec![
            ("player_id", Value::from("remote-1")),
            ("target_id", Value::from("npc-2")),
        ]);
        handle_room_data("targetChanged", Some(&remote_target), &mut state);
        assert_eq!(state.selected_entity_id.as_deref(), Some("old-target"));

        let local_target = map(vec![
            ("player_id", Value::from("local-1")),
            ("target_id", Value::from("npc-3")),
        ]);
        handle_room_data("targetChanged", Some(&local_target), &mut state);
        assert_eq!(state.selected_entity_id.as_deref(), Some("npc-3"));
    }

    #[test]
    fn inventory_update_replaces_slots_and_gold() {
        let mut state = GameState::new();
        state.inventory.slots[0] = Some(InventorySlot::new("old_item".to_string(), 1));
        state.inventory.gold = 5;

        let update = map(vec![
            (
                "slots",
                Value::Array(vec![
                    map(vec![
                        ("slot", Value::from(1)),
                        ("item_id", Value::from("bronze_sword")),
                        ("quantity", Value::from(2)),
                    ]),
                    map(vec![
                        ("slot", Value::from(3)),
                        ("item_id", Value::from("bread")),
                        ("quantity", Value::from(5)),
                    ]),
                ]),
            ),
            ("gold", Value::from(77)),
        ]);

        handle_room_data("inventoryUpdate", Some(&update), &mut state);

        assert!(state.inventory.slots[0].is_none());
        let slot_one = state.inventory.slots[1].as_ref().expect("slot 1 set");
        assert_eq!(slot_one.item_id, "bronze_sword");
        assert_eq!(slot_one.quantity, 2);
        let slot_three = state.inventory.slots[3].as_ref().expect("slot 3 set");
        assert_eq!(slot_three.item_id, "bread");
        assert_eq!(slot_three.quantity, 5);
        assert_eq!(state.inventory.gold, 77);
    }

    #[test]
    fn player_respawned_resets_local_state_and_pending_moves() {
        let mut state = GameState::new();
        state.local_player_id = Some("local-1".to_string());
        state.is_sitting = true;
        state.next_move_sequence(1.0, 0.0);
        state
            .players
            .insert("local-1".to_string(), player("local-1", "Hero"));

        if let Some(player) = state.players.get_mut("local-1") {
            player.is_dead = true;
            player.hp = 0;
        }

        let respawn = map(vec![
            ("id", Value::from("local-1")),
            ("x", Value::from(42)),
            ("y", Value::from(24)),
            ("hp", Value::from(88)),
        ]);

        handle_room_data("playerRespawned", Some(&respawn), &mut state);

        let player = state.players.get("local-1").expect("local player exists");
        assert!(!player.is_dead);
        assert_eq!(player.x, 42.0);
        assert_eq!(player.y, 24.0);
        assert_eq!(player.hp, 88);
        assert_eq!(player.max_hp, 88);
        assert!(!state.is_sitting);
        assert!(!state.has_pending_move_sequences());
    }

    #[test]
    fn state_sync_ignores_mismatched_instance_context() {
        let mut state = GameState::new();
        state.current_instance = Some("instance-a".to_string());
        state.server_tick = 7;
        state
            .players
            .insert("remote-1".to_string(), player("remote-1", "Ayla"));

        let sync = map(vec![
            ("tick", Value::from(8)),
            ("instanceId", Value::from("instance-b")),
            ("players", Value::Array(vec![])),
            ("npcs", Value::Array(vec![])),
        ]);

        handle_room_data("stateSync", Some(&sync), &mut state);

        assert_eq!(state.server_tick, 7);
        assert!(state.players.contains_key("remote-1"));
    }

    #[test]
    fn state_sync_ignores_stale_ticks() {
        let mut state = GameState::new();
        state.current_instance = Some("instance-a".to_string());
        state.server_tick = 12;
        state
            .players
            .insert("remote-1".to_string(), player("remote-1", "Ayla"));

        let stale_sync = map(vec![
            ("tick", Value::from(11)),
            ("instanceId", Value::from("instance-a")),
            ("players", Value::Array(vec![])),
            ("npcs", Value::Array(vec![])),
        ]);

        handle_room_data("stateSync", Some(&stale_sync), &mut state);

        assert_eq!(state.server_tick, 12);
        assert!(state.players.contains_key("remote-1"));
    }
}
