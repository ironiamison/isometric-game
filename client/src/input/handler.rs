use super::touch::TouchControls;
use crate::audio::AudioManager;
use crate::game::{
    pathfinding, quest_status_order, BankDrag, BankQuantityAction, BankQuantityDialog, ChatChannel,
    ContextMenu, ContextMenuTarget, DragSource, DragState, GameState, GoldDropDialog, PathState,
    QuestCatalogEntry, StallPriceDialog, CHUNK_SIZE,
};
use crate::network::messages::ClientMessage;
use crate::render::animation::AnimationState;
use crate::render::isometric::screen_to_world;
use crate::render::{section_sort_key, sections_for_tab, SECTION_HEADER_HEIGHT};
use crate::settings::{save_ui_settings, UiSettings};
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;
use std::collections::HashSet;

/// Convert a character index to a byte index in a UTF-8 string.
fn char_to_byte_index(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(byte_idx, _)| byte_idx)
        .unwrap_or(s.len())
}

/// Check if a key should fire (initial press or auto-repeat).
fn chat_key_should_fire(key: KeyCode, state: &mut GameState, current_time: f64) -> bool {
    const INITIAL_DELAY: f64 = 0.4;
    const REPEAT_RATE: f64 = 0.035;

    if is_key_pressed(key) {
        state.ui_state.chat_key_repeat_time = current_time;
        state.ui_state.chat_key_initial_delay = true;
        return true;
    } else if is_key_down(key) {
        let delay = if state.ui_state.chat_key_initial_delay {
            INITIAL_DELAY
        } else {
            REPEAT_RATE
        };
        if current_time - state.ui_state.chat_key_repeat_time >= delay {
            state.ui_state.chat_key_repeat_time = current_time;
            state.ui_state.chat_key_initial_delay = false;
            return true;
        }
    }
    false
}

/// Process chat keyboard input when chat is open.
/// Returns `true` if the caller should return early (non-classic mode consumes all input).
fn process_chat_keyboard_input(
    state: &mut GameState,
    commands: &mut Vec<InputCommand>,
    audio: &mut AudioManager,
) -> bool {
    let classic = state.ui_state.classic_controls;
    let current_time = macroquad::time::get_time();

    // Escape cancels chat (in classic mode, Escape opens ESC menu instead)
    if is_key_pressed(KeyCode::Escape) {
        if classic {
            state.ui_state.escape_menu_open = !state.ui_state.escape_menu_open;
            return true;
        }
        state.ui_state.chat_open = false;
        state.ui_state.chat_input.clear();
        state.ui_state.chat_cursor = 0;
        state.ui_state.chat_scroll_offset = 0;
        if state.ui_state.chat_panel_open {
            state.ui_state.chat_panel_open = false;
        }
        #[cfg(target_os = "android")]
        macroquad::miniquad::window::show_keyboard(false);
        return true;
    }

    // Enter sends message
    if is_key_pressed(KeyCode::Enter) {
        if matches!(state.ui_state.chat_active_tab, ChatChannel::System) {
            state.ui_state.chat_input.clear();
            state.ui_state.chat_cursor = 0;
            state.ui_state.chat_scroll_offset = 0;
        } else {
            let text = state.ui_state.chat_input.trim().to_string();
            let (send_text, channel) = if text.starts_with('~') {
                let trimmed = text[1..].trim().to_string();
                (trimmed, "global".to_string())
            } else {
                let ch = match state.ui_state.chat_active_tab {
                    ChatChannel::Global => "global",
                    _ => "public",
                };
                (text.clone(), ch.to_string())
            };
            if !send_text.is_empty() {
                audio.play_sfx("send_message");
                commands.push(InputCommand::Chat {
                    text: send_text,
                    channel,
                });
            }
            state.ui_state.chat_input.clear();
            state.ui_state.chat_cursor = 0;
            state.ui_state.chat_scroll_offset = 0;
        }
        if !classic {
            state.ui_state.chat_open = false;
        }
        #[cfg(target_os = "android")]
        macroquad::miniquad::window::show_keyboard(false);
        return true;
    }

    let char_count = state.ui_state.chat_input.chars().count();

    // Check if any repeatable key is held
    let repeatable_keys = if classic {
        vec![KeyCode::Backspace, KeyCode::Delete]
    } else {
        vec![
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Backspace,
            KeyCode::Delete,
        ]
    };
    let any_repeatable_held = repeatable_keys.iter().any(|k| is_key_down(*k));
    if !any_repeatable_held {
        state.ui_state.chat_key_initial_delay = true;
    }

    // Arrow key navigation (non-classic only)
    if !classic {
        if chat_key_should_fire(KeyCode::Left, state, current_time) {
            if state.ui_state.chat_cursor > 0 {
                state.ui_state.chat_cursor -= 1;
            }
            while get_char_pressed().is_some() {}
        }
        if chat_key_should_fire(KeyCode::Right, state, current_time) {
            let char_count = state.ui_state.chat_input.chars().count();
            if state.ui_state.chat_cursor < char_count {
                state.ui_state.chat_cursor += 1;
            }
            while get_char_pressed().is_some() {}
        }
    }
    // Home/End for quick navigation
    if is_key_pressed(KeyCode::Home) {
        state.ui_state.chat_cursor = 0;
        while get_char_pressed().is_some() {}
    }
    if is_key_pressed(KeyCode::End) {
        state.ui_state.chat_cursor = char_count;
        while get_char_pressed().is_some() {}
    }

    // Paste from clipboard (Ctrl+V / Cmd+V)
    let ctrl_held = is_key_down(KeyCode::LeftControl)
        || is_key_down(KeyCode::RightControl)
        || is_key_down(KeyCode::LeftSuper)
        || is_key_down(KeyCode::RightSuper);
    if ctrl_held && is_key_pressed(KeyCode::V) {
        if let Some(text) = macroquad::miniquad::window::clipboard_get() {
            for c in text.chars() {
                if state.ui_state.chat_input.chars().count() >= 200 {
                    break;
                }
                if c.is_control() {
                    continue;
                }
                let byte_idx =
                    char_to_byte_index(&state.ui_state.chat_input, state.ui_state.chat_cursor);
                state.ui_state.chat_input.insert(byte_idx, c);
                state.ui_state.chat_cursor += 1;
            }
        }
        while get_char_pressed().is_some() {}
    }

    // Backspace removes character before cursor
    if chat_key_should_fire(KeyCode::Backspace, state, current_time) {
        if state.ui_state.chat_cursor > 0 {
            let byte_idx =
                char_to_byte_index(&state.ui_state.chat_input, state.ui_state.chat_cursor - 1);
            state.ui_state.chat_input.remove(byte_idx);
            state.ui_state.chat_cursor -= 1;
        }
    }

    // Delete removes character at cursor
    if chat_key_should_fire(KeyCode::Delete, state, current_time) {
        let char_count = state.ui_state.chat_input.chars().count();
        if state.ui_state.chat_cursor < char_count {
            let byte_idx =
                char_to_byte_index(&state.ui_state.chat_input, state.ui_state.chat_cursor);
            state.ui_state.chat_input.remove(byte_idx);
        }
    }

    // Capture typed characters - insert at cursor position
    while let Some(c) = get_char_pressed() {
        if c.is_control()
            || !c.is_ascii_graphic() && !c.is_ascii_whitespace() && !c.is_alphanumeric()
        {
            continue;
        }
        if state.ui_state.chat_input.chars().count() < 200 {
            let byte_idx =
                char_to_byte_index(&state.ui_state.chat_input, state.ui_state.chat_cursor);
            state.ui_state.chat_input.insert(byte_idx, c);
            state.ui_state.chat_cursor += 1;
        }
    }

    // In classic mode, don't return early - fall through to movement/attack handling
    !classic
}

/// GID for obelisk map objects (objects tileset firstgid 1162 + sprite 796)
const OBELISK_GID: u32 = 1958;

/// Check if a GID is an obelisk (interactable map object)
pub fn is_obelisk_gid(gid: u32) -> bool {
    gid == OBELISK_GID
}

/// Get the display name for an interactable map object by GID
pub fn get_map_object_name(gid: u32) -> Option<&'static str> {
    match gid {
        OBELISK_GID => Some("Ancient Obelisk"),
        _ => None,
    }
}

/// Save current UI settings to persistent storage
fn save_current_ui_settings(state: &GameState) {
    let settings = UiSettings {
        zoom: state.camera.zoom,
        ui_scale: state.ui_state.ui_scale,
        shift_drop_enabled: state.ui_state.shift_drop_enabled,
        chat_log_visible: state.ui_state.chat_log_visible,
        tap_to_pathfind: state.ui_state.tap_to_pathfind,
        use_joystick: state.ui_state.use_joystick,
        graphics_low: state.ui_state.graphics_low,
        chat_log_background: state.ui_state.chat_log_background,
        hotkey_bar: state.ui_state.hotkey_bar.clone(),
    };
    save_ui_settings(&settings);
}

/// Convert screen coordinates to virtual coordinates for UI hit detection
fn screen_to_virtual_coords(x: f32, y: f32) -> (f32, f32) {
    let (vw, vh) = virtual_screen_size();
    let screen_w = screen_width();
    let screen_h = screen_height();
    (x * vw / screen_w, y * vh / screen_h)
}

fn latest_chat_timestamp_for_channel(state: &GameState, channel: ChatChannel) -> f64 {
    state.ui_state.chat_messages.latest_timestamp(&channel)
}

fn mark_chat_channel_as_read(state: &mut GameState, channel: ChatChannel) {
    let latest = latest_chat_timestamp_for_channel(state, channel);
    match channel {
        ChatChannel::Local => {
            state.ui_state.chat_last_seen_local = state.ui_state.chat_last_seen_local.max(latest);
        }
        ChatChannel::Global => {
            state.ui_state.chat_last_seen_global = state.ui_state.chat_last_seen_global.max(latest);
        }
        ChatChannel::System => {
            state.ui_state.chat_last_seen_system = state.ui_state.chat_last_seen_system.max(latest);
        }
    }
}

const AUTO_ACTION_NPC_SETTLE_EPS: f32 = 0.06;

fn queue_face(state: &mut GameState, commands: &mut Vec<InputCommand>, direction: u8) {
    commands.push(InputCommand::Face { direction });
    if let Some(local_id) = &state.local_player_id {
        if let Some(player) = state.players.get_mut(local_id) {
            let dir = crate::game::Direction::from_u8(direction);
            player.direction = dir;
            player.animation.direction = dir;
        }
    }
}

fn auto_action_target_settled(aa: &crate::game::AutoActionState, state: &GameState) -> bool {
    if aa.target_type != "npc" {
        return true;
    }
    state.npcs.get(&aa.target_id).map_or(true, |npc| {
        let dx = (npc.x - npc.target_x).abs();
        let dy = (npc.y - npc.target_y).abs();
        dx.max(dy) <= AUTO_ACTION_NPC_SETTLE_EPS
    })
}

/// Activate a hotkey slot binding — returns InputCommand(s) to execute
fn activate_hotkey_slot(state: &mut GameState, slot_idx: usize) -> Vec<InputCommand> {
    use crate::game::hotkey::HotkeySlotBinding;

    let binding = state.ui_state.hotkey_bar.active().slots[slot_idx].clone();
    match binding {
        HotkeySlotBinding::Empty => vec![],
        HotkeySlotBinding::Item { ref item_id } => {
            if let Some(inv_idx) = state.inventory.find_slot_by_item_id(item_id) {
                let item_def = state.item_registry.get_or_placeholder(item_id);
                if item_def.equipment.is_some() {
                    vec![InputCommand::Equip {
                        slot_index: inv_idx as u8,
                    }]
                } else {
                    vec![InputCommand::UseItem {
                        slot_index: inv_idx as u8,
                    }]
                }
            } else {
                // Item not in inventory (ghost state) — no-op
                vec![]
            }
        }
        HotkeySlotBinding::Spell { ref spell_id } => {
            let now = macroquad::time::get_time();
            let on_cooldown = state
                .spell_cooldowns
                .get(spell_id.as_str())
                .map_or(false, |&t| now < t);
            if !on_cooldown {
                // Look up spell def for cooldown duration (check static spells, then scroll spells)
                let cooldown_ms = crate::game::spell::SPELLS
                    .iter()
                    .find(|s| s.id == spell_id)
                    .map(|s| s.cooldown_ms)
                    .or_else(|| {
                        state
                            .scroll_spell_definitions
                            .iter()
                            .find(|s| s.id == *spell_id)
                            .map(|s| s.cooldown_ms)
                    });
                if let Some(cd_ms) = cooldown_ms {
                    let cooldown_end = now + (cd_ms as f64 / 1000.0);
                    state.spell_cooldowns.insert(spell_id.clone(), cooldown_end);
                }
                vec![InputCommand::CastSpell {
                    spell_id: spell_id.clone(),
                }]
            } else {
                vec![]
            }
        }
    }
}

fn is_adventurer_guide_dialogue(speaker: &str) -> bool {
    speaker.eq_ignore_ascii_case("Adventurer Guide")
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

fn is_adventurer_guide_tier_id(quest_id: &str) -> bool {
    matches!(
        quest_id,
        "adventurer_tier_1"
            | "adventurer_tier_2"
            | "adventurer_tier_3"
            | "skilling_tier_1"
            | "skilling_tier_2"
            | "skilling_tier_3"
    )
}

fn has_active_adventurer_guide_task(state: &GameState) -> bool {
    state
        .ui_state
        .active_quests
        .iter()
        .any(|q| is_adventurer_guide_tier_id(&q.id))
}

fn is_selected_adventurer_guide_tier_active(state: &GameState) -> bool {
    let Some(selected_id) = adventurer_guide_tier_id(
        state.ui_state.adventurer_selected_tab,
        state.ui_state.adventurer_selected_tier,
    ) else {
        return false;
    };

    state
        .ui_state
        .active_quests
        .iter()
        .any(|q| q.id == selected_id)
}

fn is_selected_adventurer_guide_tier_completable(state: &GameState) -> bool {
    let Some(selected_id) = adventurer_guide_tier_id(
        state.ui_state.adventurer_selected_tab,
        state.ui_state.adventurer_selected_tier,
    ) else {
        return false;
    };

    state
        .ui_state
        .active_quests
        .iter()
        .find(|q| q.id == selected_id)
        .map(|q| q.objectives.iter().all(|o| o.completed))
        .unwrap_or(false)
}

fn adventurer_guide_actions_locked(state: &GameState) -> bool {
    has_active_adventurer_guide_task(state) && !is_selected_adventurer_guide_tier_active(state)
}

fn is_combat_tier_unlocked(state: &GameState, tier_idx: usize) -> bool {
    let Some(tier_id) = adventurer_guide_tier_id(0, tier_idx) else {
        return false;
    };

    if tier_idx == 0 {
        return true;
    }

    if state.ui_state.completed_quest_ids.contains(tier_id)
        || state.ui_state.active_quests.iter().any(|q| q.id == tier_id)
    {
        return true;
    }

    let Some(prev_id) = adventurer_guide_tier_id(0, tier_idx.saturating_sub(1)) else {
        return false;
    };
    state.ui_state.completed_quest_ids.contains(prev_id)
}

fn should_auto_open_selected_combat_tier_offer(
    state: &GameState,
    is_guide_dialogue: bool,
    dialogue_has_choices: bool,
) -> bool {
    if !is_guide_dialogue {
        return false;
    }
    if state.ui_state.adventurer_selected_tab != 0 {
        return false;
    }
    if has_active_adventurer_guide_task(state) {
        return false;
    }
    if dialogue_has_choices {
        return false;
    }

    let tier_idx = state.ui_state.adventurer_selected_tier;
    let Some(tier_id) = adventurer_guide_tier_id(0, tier_idx) else {
        return false;
    };

    if state.ui_state.completed_quest_ids.contains(tier_id)
        || state.ui_state.active_quests.iter().any(|q| q.id == tier_id)
    {
        return false;
    }

    is_combat_tier_unlocked(state, tier_idx)
}

fn sync_adventurer_guide_dialogue_target(state: &mut GameState) {
    let selected_id = adventurer_guide_tier_id(
        state.ui_state.adventurer_selected_tab,
        state.ui_state.adventurer_selected_tier,
    );

    if let Some(dialogue) = state.ui_state.active_dialogue.as_mut() {
        if is_adventurer_guide_dialogue(&dialogue.speaker) && dialogue.choices.is_empty() {
            if let Some(quest_id) = selected_id {
                dialogue.quest_id = quest_id.to_string();
                dialogue.text = "Select a tier to review progress. Talk to the guide to start or complete tiers.".to_string();
            }
        }
    }
}

/// Get the world position of an auto-action target (for facing direction)
fn auto_action_target_pos(
    aa: &crate::game::AutoActionState,
    state: &GameState,
) -> Option<(f32, f32)> {
    match aa.target_type.as_str() {
        // Use server-authoritative positions for chase logic to avoid interpolation mismatches
        "npc" => state
            .npcs
            .get(&aa.target_id)
            .map(|n| (n.server_x, n.server_y)),
        "player" => state
            .players
            .get(&aa.target_id)
            .map(|p| (p.server_x, p.server_y)),
        "resource" => {
            // target_id format: "x,y,gid"
            let parts: Vec<&str> = aa.target_id.split(',').collect();
            if parts.len() >= 2 {
                if let (Ok(x), Ok(y)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                    return Some((x as f32, y as f32));
                }
            }
            None
        }
        _ => None,
    }
}

fn sync_path_index(path_state: &mut PathState, player_pos: (i32, i32)) {
    while path_state.current_index < path_state.path.len() {
        let (wx, wy) = path_state.path[path_state.current_index];
        if (wx, wy) == player_pos {
            path_state.current_index += 1;
        } else {
            break;
        }
    }

    if path_state.current_index < path_state.path.len() {
        if let Some(found_idx) = path_state
            .path
            .iter()
            .enumerate()
            .skip(path_state.current_index)
            .find_map(|(i, &(wx, wy))| {
                if (wx, wy) == player_pos {
                    Some(i)
                } else {
                    None
                }
            })
        {
            path_state.current_index = found_idx + 1;
        }
    }
}

/// Build set of tiles occupied by entities (other players + NPCs) for pathfinding
fn build_occupied_set(state: &GameState, include_chairs: bool) -> HashSet<(i32, i32)> {
    let mut occupied = HashSet::new();

    // When in interior mode, don't count overworld players as obstacles
    // (they shouldn't be in our instance anyway)
    let in_interior = state.current_interior.is_some();

    // Add other players (not local player)
    // Skip if in interior - we'll only see players in our instance from server updates
    if !in_interior {
        for (id, player) in &state.players {
            if state.local_player_id.as_ref() == Some(id) {
                continue;
            }
            if !player.is_dead {
                // Use server-authoritative coordinates to match server-side collision checks.
                occupied.insert((
                    player.server_x.round() as i32,
                    player.server_y.round() as i32,
                ));
            }
        }
    }

    // Add all alive NPCs
    for npc in state.npcs.values() {
        if npc.is_alive() {
            // Use server-authoritative coordinates to avoid interpolation skew.
            occupied.insert((npc.server_x.round() as i32, npc.server_y.round() as i32));
        }
    }

    if include_chairs {
        for (cx, cy) in &state.chair_positions {
            occupied.insert((*cx, *cy));
        }
    }

    occupied
}

fn preferred_adjacent_tile_for_target(state: &GameState, target: (i32, i32)) -> Option<(i32, i32)> {
    let player = state.get_local_player()?;
    let dx = player.x - target.0 as f32;
    let dy = player.y - target.1 as f32;

    if dx.abs() < 0.01 && dy.abs() < 0.01 {
        return None;
    }

    if dx.abs() >= dy.abs() {
        Some((target.0 + if dx > 0.0 { 1 } else { -1 }, target.1))
    } else {
        Some((target.0, target.1 + if dy > 0.0 { 1 } else { -1 }))
    }
}

#[derive(Clone)]
struct SpliceCandidate {
    pos: (i32, i32),
    steps_to_pos: i32,
    prefix_range: Option<(usize, usize)>,
}

fn splice_candidates(
    state: &GameState,
    start: (i32, i32),
    max_splice_ahead: usize,
) -> Vec<SpliceCandidate> {
    let mut candidates = Vec::new();
    candidates.push(SpliceCandidate {
        pos: start,
        steps_to_pos: 0,
        prefix_range: None,
    });

    let Some(path_state) = state.auto_path.as_ref() else {
        return candidates;
    };

    if path_state.current_index >= path_state.path.len() {
        return candidates;
    }

    let max_idx = (path_state.current_index + max_splice_ahead).min(path_state.path.len() - 1);
    let start_is_next = path_state.path[path_state.current_index] == start;
    let base_steps: i32 = if start_is_next { 0 } else { 1 };

    for i in path_state.current_index..=max_idx {
        let pos = path_state.path[i];
        if pos == start {
            continue;
        }
        let steps_to_pos = base_steps + (i as i32 - path_state.current_index as i32);
        candidates.push(SpliceCandidate {
            pos,
            steps_to_pos,
            prefix_range: Some((path_state.current_index, i)),
        });
    }

    candidates
}

fn find_path_with_optimistic_splice(
    state: &GameState,
    start: (i32, i32),
    goal: (i32, i32),
    occupied: &HashSet<(i32, i32)>,
    max_distance: i32,
) -> Option<Vec<(i32, i32)>> {
    find_path_with_limited_splice(state, start, goal, occupied, max_distance, 6)
}

// For plain click-to-move, only preserve the currently committed next tile.
// Splicing too far ahead into an old route can create loops/backtracking when
// the player rapidly retargets destinations.
fn find_path_with_committed_step_splice(
    state: &GameState,
    start: (i32, i32),
    goal: (i32, i32),
    occupied: &HashSet<(i32, i32)>,
    max_distance: i32,
) -> Option<Vec<(i32, i32)>> {
    find_path_with_limited_splice(state, start, goal, occupied, max_distance, 1)
}

fn find_path_with_limited_splice(
    state: &GameState,
    start: (i32, i32),
    goal: (i32, i32),
    occupied: &HashSet<(i32, i32)>,
    max_distance: i32,
    max_splice_ahead: usize,
) -> Option<Vec<(i32, i32)>> {
    let candidates = splice_candidates(state, start, max_splice_ahead);
    let path_state = state.auto_path.as_ref();

    let mut best: Option<(i32, SpliceCandidate, Vec<(i32, i32)>)> = None;

    for cand in candidates {
        if let Some(path) =
            pathfinding::find_path(cand.pos, goal, &state.chunk_manager, occupied, max_distance)
        {
            let steps_from_pos = path.len().saturating_sub(1) as i32;
            let total_steps = cand.steps_to_pos + steps_from_pos;
            let better = match &best {
                None => true,
                Some((best_total, best_cand, _)) => {
                    total_steps < *best_total
                        || (total_steps == *best_total
                            && cand.steps_to_pos > best_cand.steps_to_pos)
                }
            };
            if better {
                best = Some((total_steps, cand.clone(), path));
            }
        }
    }

    let Some((_, cand, path)) = best else {
        return None;
    };

    if let (Some((start_idx, end_idx)), Some(path_state)) = (cand.prefix_range, path_state) {
        let mut combined = Vec::new();
        combined.extend_from_slice(&path_state.path[start_idx..=end_idx]);
        if path.len() > 1 {
            combined.extend_from_slice(&path[1..]);
        }
        return Some(combined);
    }

    Some(path)
}

/// Get the attack range for the local player's equipped weapon (1 for melee/unarmed, >1 for ranged).
fn get_local_weapon_range(state: &GameState) -> i32 {
    let weapon_id = state
        .local_player_id
        .as_ref()
        .and_then(|id| state.players.get(id))
        .and_then(|p| p.equipped_weapon.as_ref());
    if let Some(weapon_id) = weapon_id {
        if let Some(item_def) = state.item_registry.get(weapon_id) {
            return item_def.range.unwrap_or(1);
        }
    }
    1
}

/// Check if a position is within attack range of a target (matches server logic).
/// Uses Manhattan distance (diamond shape) for all ranges.
fn in_attack_range(px: i32, py: i32, tx: i32, ty: i32, weapon_range: i32) -> bool {
    let dx = (px - tx).abs();
    let dy = (py - ty).abs();
    if weapon_range == 1 {
        (dx + dy) == 1 // Cardinal adjacency for melee
    } else {
        (dx + dy) <= weapon_range && (dx > 0 || dy > 0) // Manhattan for ranged
    }
}

fn find_path_to_attack_with_optimistic_splice(
    state: &GameState,
    start: (i32, i32),
    target: (i32, i32),
    occupied: &HashSet<(i32, i32)>,
    max_distance: i32,
    weapon_range: i32,
) -> Option<((i32, i32), Vec<(i32, i32)>)> {
    if weapon_range <= 1 {
        let preferred = preferred_adjacent_tile_for_target(state, target);
        return find_path_to_adjacent_with_optimistic_splice(
            state, start, target, occupied, max_distance, preferred,
        );
    }

    // For ranged: use optimistic splice candidates with range-based pathfinding
    let candidates = splice_candidates(state, start, 6);

    let mut best: Option<(i32, SpliceCandidate, (i32, i32), Vec<(i32, i32)>)> = None;

    for cand in candidates {
        if let Some((dest, path)) = pathfinding::find_path_within_range(
            cand.pos,
            target,
            &state.chunk_manager,
            occupied,
            max_distance,
            weapon_range,
        ) {
            let steps_from_pos = path.len().saturating_sub(1) as i32;
            let total_steps = cand.steps_to_pos + steps_from_pos;
            let better = match &best {
                None => true,
                Some((best_total, best_cand, _, _)) => {
                    total_steps < *best_total
                        || (total_steps == *best_total
                            && cand.steps_to_pos > best_cand.steps_to_pos)
                }
            };
            if better {
                best = Some((total_steps, cand.clone(), dest, path));
            }
        }
    }

    let Some((_, cand, dest, path)) = best else {
        return None;
    };

    let path_state = state.auto_path.as_ref();
    if let (Some((start_idx, end_idx)), Some(path_state)) = (cand.prefix_range, path_state) {
        let mut combined = Vec::new();
        combined.extend_from_slice(&path_state.path[start_idx..=end_idx]);
        if path.len() > 1 {
            combined.extend_from_slice(&path[1..]);
        }
        return Some((dest, combined));
    }

    Some((dest, path))
}

fn find_path_to_adjacent_with_optimistic_splice(
    state: &GameState,
    start: (i32, i32),
    target: (i32, i32),
    occupied: &HashSet<(i32, i32)>,
    max_distance: i32,
    preferred_adjacent: Option<(i32, i32)>,
) -> Option<((i32, i32), Vec<(i32, i32)>)> {
    let candidates = splice_candidates(state, start, 6);
    let path_state = state.auto_path.as_ref();

    let mut best: Option<(i32, SpliceCandidate, (i32, i32), Vec<(i32, i32)>)> = None;

    for cand in candidates {
        if let Some((dest, path)) = pathfinding::find_path_to_adjacent_prefer(
            cand.pos,
            target,
            &state.chunk_manager,
            occupied,
            max_distance,
            preferred_adjacent,
        ) {
            let steps_from_pos = path.len().saturating_sub(1) as i32;
            let total_steps = cand.steps_to_pos + steps_from_pos;
            let better = match &best {
                None => true,
                Some((best_total, best_cand, _, _)) => {
                    total_steps < *best_total
                        || (total_steps == *best_total
                            && cand.steps_to_pos > best_cand.steps_to_pos)
                }
            };
            if better {
                best = Some((total_steps, cand.clone(), dest, path));
            }
        }
    }

    let Some((_, cand, dest, path)) = best else {
        return None;
    };

    if let (Some((start_idx, end_idx)), Some(path_state)) = (cand.prefix_range, path_state) {
        let mut combined = Vec::new();
        combined.extend_from_slice(&path_state.path[start_idx..=end_idx]);
        if path.len() > 1 {
            combined.extend_from_slice(&path[1..]);
        }
        return Some((dest, combined));
    }

    Some((dest, path))
}

fn face_target_if_needed(
    state: &mut GameState,
    commands: &mut Vec<InputCommand>,
    dx: f32,
    dy: f32,
) {
    let dir = crate::game::Direction::from_velocity(dx, dy);
    if let Some(local_id) = &state.local_player_id {
        if let Some(player) = state.players.get(local_id) {
            if player.direction == dir {
                return;
            }
        }
    }
    queue_face(state, commands, dir as u8);
}

/// Pathfind to within attack range of a player and set up attack, or attack immediately if in range.
fn pathfind_and_attack_player(
    state: &mut GameState,
    commands: &mut Vec<InputCommand>,
    target_id: &str,
) {
    if let Some(local_id) = &state.local_player_id.clone() {
        if let Some(local_player) = state.players.get(local_id) {
            if let Some(target) = state.players.get(target_id) {
                let px = local_player.server_x.round() as i32;
                let py = local_player.server_y.round() as i32;
                let tx = target.server_x.round() as i32;
                let ty = target.server_y.round() as i32;
                let weapon_range = get_local_weapon_range(state);
                if !in_attack_range(px, py, tx, ty, weapon_range) {
                    let occupied = build_occupied_set(state, true);
                    const MAX_PATH_DISTANCE: i32 = 32;
                    if let Some((dest, path)) = find_path_to_attack_with_optimistic_splice(
                        state,
                        (px, py),
                        (tx, ty),
                        &occupied,
                        MAX_PATH_DISTANCE,
                        weapon_range,
                    ) {
                        state.auto_path = Some(PathState {
                            path,
                            current_index: 0,
                            destination: dest,
                            pickup_target: None,
                            interact_target: None,
                            interact_object_target: None,
                            waystone_target: None,
                            browse_stall_target: None,
                        });
                    }
                } else {
                    let dir = crate::game::Direction::from_velocity(
                        target.server_x - local_player.x,
                        target.server_y - local_player.y,
                    );
                    queue_face(state, commands, dir as u8);
                    commands.push(InputCommand::StartAutoAction {
                        target_type: "player".to_string(),
                        target_id: target_id.to_string(),
                        action: "attack".to_string(),
                    });
                }
            }
        }
    }
}

/// Pathfind to within attack range of an NPC and set up attack, or attack immediately if in range.
fn pathfind_and_attack_npc(state: &mut GameState, commands: &mut Vec<InputCommand>, npc_id: &str) {
    if let Some(local_id) = &state.local_player_id.clone() {
        if let Some(player) = state.players.get(local_id) {
            if let Some(npc) = state.npcs.get(npc_id) {
                let px = player.server_x.round() as i32;
                let py = player.server_y.round() as i32;
                let nx = npc.server_x.round() as i32;
                let ny = npc.server_y.round() as i32;
                let weapon_range = get_local_weapon_range(state);
                if !in_attack_range(px, py, nx, ny, weapon_range) {
                    let occupied = build_occupied_set(state, true);
                    const MAX_PATH_DISTANCE: i32 = 32;
                    if let Some((dest, path)) = find_path_to_attack_with_optimistic_splice(
                        state,
                        (px, py),
                        (nx, ny),
                        &occupied,
                        MAX_PATH_DISTANCE,
                        weapon_range,
                    ) {
                        state.auto_path = Some(PathState {
                            path,
                            current_index: 0,
                            destination: dest,
                            pickup_target: None,
                            interact_target: None,
                            interact_object_target: None,
                            waystone_target: None,
                            browse_stall_target: None,
                        });
                    }
                } else {
                    let dir = crate::game::Direction::from_velocity(
                        npc.server_x - player.x,
                        npc.server_y - player.y,
                    );
                    queue_face(state, commands, dir as u8);
                    if let Some(aa) = state.auto_action_state.as_ref() {
                        if auto_action_target_settled(aa, state) {
                            commands.push(InputCommand::StartAutoAction {
                                target_type: "npc".to_string(),
                                target_id: npc_id.to_string(),
                                action: "attack".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Pathfind to NPC and execute an action when in range, or do it immediately if close enough.
fn pathfind_and_interact_npc(
    state: &mut GameState,
    commands: &mut Vec<InputCommand>,
    npc_id: &str,
    on_interact: impl FnOnce(&mut GameState, &mut Vec<InputCommand>, &str),
) {
    const INTERACT_RANGE: f32 = 2.5;
    let should_interact = if let Some(local_id) = &state.local_player_id.clone() {
        if let Some(player) = state.players.get(local_id) {
            if let Some(npc) = state.npcs.get(npc_id) {
                let dx = npc.server_x - player.server_x;
                let dy = npc.server_y - player.server_y;
                let dist = (dx * dx + dy * dy).sqrt();
                Some(dist < INTERACT_RANGE)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    match should_interact {
        Some(true) => {
            on_interact(state, commands, npc_id);
        }
        Some(false) => {
            if let Some(local_id) = &state.local_player_id.clone() {
                if let Some(player) = state.players.get(local_id) {
                    if let Some(npc) = state.npcs.get(npc_id) {
                        let px = player.server_x.round() as i32;
                        let py = player.server_y.round() as i32;
                        let nx = npc.server_x.round() as i32;
                        let ny = npc.server_y.round() as i32;
                        let occupied = build_occupied_set(state, false);
                        const MAX_PATH_DISTANCE: i32 = 32;
                        if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                            (px, py),
                            (nx, ny),
                            &state.chunk_manager,
                            &occupied,
                            MAX_PATH_DISTANCE,
                        ) {
                            let npc_id_owned = npc_id.to_string();
                            state.auto_path = Some(PathState {
                                path,
                                current_index: 0,
                                destination: dest,
                                pickup_target: None,
                                interact_target: Some(npc_id_owned),
                                interact_object_target: None,
                                waystone_target: None,
                                browse_stall_target: None,
                            });
                        }
                    }
                }
            }
        }
        None => {}
    }
}

/// Pathfind to adjacent tile of a resource and start auto-action, or do it immediately if adjacent.
fn pathfind_and_resource(
    state: &mut GameState,
    commands: &mut Vec<InputCommand>,
    tile_x: i32,
    tile_y: i32,
    target_id: &str,
    action: &str,
) {
    if let Some(player) = state.get_local_player() {
        let px = player.server_x.round() as i32;
        let py = player.server_y.round() as i32;
        let cdx = (px - tile_x).abs();
        let cdy = (py - tile_y).abs();
        if (cdx + cdy) != 1 {
            let occupied = build_occupied_set(state, true);
            const MAX_PATH_DISTANCE: i32 = 32;
            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                (px, py),
                (tile_x, tile_y),
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
            ) {
                state.auto_path = Some(PathState {
                    path,
                    current_index: 0,
                    destination: dest,
                    pickup_target: None,
                    interact_target: None,
                    interact_object_target: None,
                    waystone_target: None,
                    browse_stall_target: None,
                });
            }
        } else {
            let dir = crate::game::Direction::from_velocity(
                tile_x as f32 - px as f32,
                tile_y as f32 - py as f32,
            );
            queue_face(state, commands, dir as u8);
            commands.push(InputCommand::StartAutoAction {
                target_type: "resource".to_string(),
                target_id: target_id.to_string(),
                action: action.to_string(),
            });
        }
    }
}

/// Pathfind to a tile.
fn pathfind_to_tile(
    state: &mut GameState,
    commands: &mut Vec<InputCommand>,
    tile_x: i32,
    tile_y: i32,
) {
    // Cancel any existing auto-action or follow
    if state.auto_action_state.is_some() {
        state.auto_action_state = None;
        commands.push(InputCommand::CancelAutoAction);
    }
    state.follow_target = None;
    state.follow_arrived_target_pos = None;
    state.follow_target_move_time = 0.0;

    const MAX_PATH_DISTANCE: i32 = 32;
    if let Some(player) = state.get_local_player() {
        // Start path plans from authoritative server tile to avoid
        // planning from a visual/interpolated future tile.
        let px = player.server_x.round() as i32;
        let py = player.server_y.round() as i32;
        let dist = (tile_x - px).abs().max((tile_y - py).abs());
        if dist <= MAX_PATH_DISTANCE
            && state
                .chunk_manager
                .is_walkable(tile_x as f32, tile_y as f32)
        {
            let occupied = build_occupied_set(state, true);
            if let Some(path) = pathfinding::find_path(
                (px, py),
                (tile_x, tile_y),
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
            ) {
                state.auto_path = Some(PathState {
                    path,
                    current_index: 0,
                    destination: (tile_x, tile_y),
                    pickup_target: None,
                    interact_target: None,
                    interact_object_target: None,
                    waystone_target: None,
                    browse_stall_target: None,
                });
            }
        }
    }
}

fn rebuild_path_state(template: &PathState, path: Vec<(i32, i32)>, destination: (i32, i32)) -> PathState {
    let mut next = template.clone();
    next.path = path;
    next.current_index = 0;
    next.destination = destination;
    next
}

fn rebuild_current_auto_path(state: &mut GameState) -> bool {
    const MAX_PATH_DISTANCE: i32 = 32;

    let Some(template) = state.auto_path.clone() else {
        return false;
    };
    let Some(player) = state.get_local_player() else {
        return false;
    };
    let start = (
        player.server_x.round() as i32,
        player.server_y.round() as i32,
    );

    if let Some(aa) = state.auto_action_state.clone() {
        if let Some((txf, tyf)) = auto_action_target_pos(&aa, state) {
            let target = (txf.round() as i32, tyf.round() as i32);
            let mut occupied = build_occupied_set(state, true);
            match aa.target_type.as_str() {
                "npc" => {
                    occupied.remove(&target);
                }
                "player" => {
                    occupied.remove(&target);
                }
                _ => {}
            }
            let preferred = preferred_adjacent_tile_for_target(state, target);
            if let Some((dest, path)) = pathfinding::find_path_to_adjacent_prefer(
                start,
                target,
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
                preferred,
            ) {
                state.auto_path = Some(rebuild_path_state(&template, path, dest));
                return true;
            }
        }
        return false;
    }

    if let Some(follow_id) = state.follow_target.clone() {
        if let Some(target) = state.players.get(&follow_id) {
            let target_tile = (
                target.server_x.round() as i32,
                target.server_y.round() as i32,
            );
            let mut occupied = build_occupied_set(state, true);
            occupied.remove(&target_tile);
            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                start,
                target_tile,
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
            ) {
                state.auto_path = Some(rebuild_path_state(&template, path, dest));
                return true;
            }
        }
        return false;
    }

    if let Some(item_id) = template.pickup_target.clone() {
        if let Some(item) = state.ground_items.get(&item_id) {
            let target = (item.x.round() as i32, item.y.round() as i32);
            let occupied = build_occupied_set(state, true);
            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                start,
                target,
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
            ) {
                state.auto_path = Some(rebuild_path_state(&template, path, dest));
                return true;
            }
        }
        return false;
    }

    if let Some(npc_id) = template.interact_target.clone() {
        if let Some(npc) = state.npcs.get(&npc_id) {
            let target = (npc.server_x.round() as i32, npc.server_y.round() as i32);
            let occupied = build_occupied_set(state, false);
            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                start,
                target,
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
            ) {
                state.auto_path = Some(rebuild_path_state(&template, path, dest));
                return true;
            }
        }
        return false;
    }

    if let Some(target) = template.interact_object_target {
        let occupied = build_occupied_set(state, true);
        if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
            start,
            target,
            &state.chunk_manager,
            &occupied,
            MAX_PATH_DISTANCE,
        ) {
            state.auto_path = Some(rebuild_path_state(&template, path, dest));
            return true;
        }
        return false;
    }

    if let Some(target) = template.waystone_target {
        let occupied = build_occupied_set(state, true);
        if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
            start,
            target,
            &state.chunk_manager,
            &occupied,
            MAX_PATH_DISTANCE,
        ) {
            state.auto_path = Some(rebuild_path_state(&template, path, dest));
            return true;
        }
        return false;
    }

    if let Some(player_id) = template.browse_stall_target.clone() {
        if let Some(target) = state.players.get(&player_id) {
            let target_tile = (
                target.server_x.round() as i32,
                target.server_y.round() as i32,
            );
            let mut occupied = build_occupied_set(state, true);
            occupied.remove(&target_tile);
            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                start,
                target_tile,
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
            ) {
                state.auto_path = Some(rebuild_path_state(&template, path, dest));
                return true;
            }
        }
        return false;
    }

    if let Some((chair_x, chair_y)) = state.pending_chair_sit {
        let occupied = build_occupied_set(state, true);
        if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
            start,
            (chair_x, chair_y),
            &state.chunk_manager,
            &occupied,
            MAX_PATH_DISTANCE,
        ) {
            state.auto_path = Some(rebuild_path_state(&template, path, dest));
            return true;
        }
        return false;
    }

    if let Some(patch_id) = state.pending_harvest_patch.clone() {
        if let Some(patch) = state.farming_patches.get(&patch_id) {
            let occupied = build_occupied_set(state, true);
            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                start,
                (patch.x, patch.y),
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
            ) {
                state.auto_path = Some(rebuild_path_state(&template, path, dest));
                return true;
            }
        }
        return false;
    }

    let goal = template.destination;
    if !state.chunk_manager.is_walkable(goal.0 as f32, goal.1 as f32) {
        return false;
    }
    let occupied = build_occupied_set(state, true);
    if let Some(path) =
        pathfinding::find_path(start, goal, &state.chunk_manager, &occupied, MAX_PATH_DISTANCE)
    {
        state.auto_path = Some(rebuild_path_state(&template, path, goal));
        return true;
    }

    false
}

/// Returns (combat_level_required, slayer_level_required) for a slayer master entity type.
fn slayer_master_requirements(entity_type: &str) -> (i32, i32) {
    match entity_type {
        "slayer_master_turael" => (0, 1),
        "slayer_master_mazchna" => (20, 30),
        "slayer_master_chaeldar" => (40, 60),
        _ => (0, 1), // Unknown master, let server validate
    }
}

/// Input commands that can be sent to the server
#[derive(Debug, Clone)]
pub enum InputCommand {
    Move {
        dx: f32,
        dy: f32,
    },
    Face {
        direction: u8,
    },
    Attack,
    Target {
        entity_id: String,
    },
    ClearTarget,
    Chat {
        text: String,
        channel: String,
    },
    Pickup {
        item_id: String,
    },
    UseItem {
        slot_index: u8,
    },
    // Quest commands
    Interact {
        npc_id: String,
    },
    DialogueChoice {
        quest_id: String,
        choice_id: String,
    },
    CloseDialogue,
    // Crafting commands
    Craft {
        recipe_id: String,
    },
    CancelCraft,
    // Equipment commands
    Equip {
        slot_index: u8,
    },
    Unequip {
        slot_type: String,
        target_slot: Option<u8>,
    },
    // Inventory commands
    DropItem {
        slot_index: u8,
        quantity: u32,
        target_x: Option<i32>,
        target_y: Option<i32>,
    },
    DropGold {
        amount: i32,
    },
    SwapSlots {
        from_slot: u8,
        to_slot: u8,
    },
    // Shop commands
    ShopBuy {
        npc_id: String,
        item_id: String,
        quantity: u32,
    },
    ShopSell {
        npc_id: String,
        item_id: String,
        quantity: u32,
    },
    // Bank commands
    BankDeposit {
        item_id: String,
        quantity: i32,
    },
    BankWithdraw {
        item_id: String,
        quantity: i32,
    },
    BankDepositGold {
        amount: i32,
    },
    BankWithdrawGold {
        amount: i32,
    },
    BankDepositAll,
    BankSwapSlots {
        slot_a: u32,
        slot_b: u32,
    },
    BankSort,
    // Portal commands
    EnterPortal {
        portal_id: String,
    },
    // Gathering commands
    StartGathering {
        marker_x: i32,
        marker_y: i32,
    },
    StopGathering,
    // Woodcutting commands
    ChopTree {
        tree_x: i32,
        tree_y: i32,
        tree_gid: u32,
    },
    // Mining commands
    MineRock {
        rock_x: i32,
        rock_y: i32,
        rock_gid: u32,
    },
    // Chair commands
    SitChair {
        tile_x: i32,
        tile_y: i32,
    },
    StandUp,
    // Farming commands
    PlantSeed {
        patch_id: String,
        item_id: String,
    },
    HarvestCrop {
        patch_id: String,
    },
    // Friend system commands
    SendFriendRequest {
        target_name: String,
    },
    AcceptFriendRequest {
        requester_id: i64,
    },
    DeclineFriendRequest {
        requester_id: i64,
    },
    RemoveFriend {
        friend_id: i64,
    },
    GetOnlinePlayers,
    // Prayer commands
    TogglePrayer {
        prayer_id: String,
    },
    BuryBones {
        slot: u8,
    },
    // Altar commands
    OfferBones {
        slot: u8,
        altar_id: String,
    },
    OfferAllBones {
        item_id: String,
        altar_id: String,
    },
    PrayAtAltar {
        altar_id: String,
    },
    // Spell commands
    CastSpell {
        spell_id: String,
    },
    // Movement abilities
    Dash,
    // Furnace commands
    FurnaceCraft {
        recipe_id: String,
        quantity: u32,
    },
    // Anvil commands
    AnvilCraft {
        recipe_id: String,
        quantity: u32,
    },
    // Alchemy Station commands
    AlchemyCraft {
        recipe_id: String,
        quantity: u32,
    },
    // Workbench commands
    WorkbenchCraft {
        recipe_id: String,
        quantity: u32,
    },
    // Fletching commands
    FletchingCraft {
        recipe_id: String,
        quantity: u32,
    },
    // Slayer commands
    SlayerGetTask {
        master_id: String,
    },
    SlayerCancelTask,
    SlayerBuyReward {
        reward_id: String,
        target_monster_id: Option<String>,
    },
    SlayerRemoveBlock {
        monster_id: String,
    },
    // Chest commands
    ChestTake {
        chest_id: String,
        slot: u8,
    },
    ChestDeposit {
        chest_id: String,
        inventory_slot: u8,
    },
    // Auto-action commands (click-to-act chase system)
    StartAutoAction {
        target_type: String,
        target_id: String,
        action: String,
    },
    CancelAutoAction,
    // Map object interaction commands
    InteractObject {
        x: i32,
        y: i32,
    },
    // Direct waystone teleport (no dialogue)
    UseWaystone {
        x: i32,
        y: i32,
    },
    // Trade commands
    TradeRequest {
        target_id: String,
    },
    TradeAcceptRequest {
        requester_id: String,
    },
    TradeDeclineRequest {
        requester_id: String,
    },
    TradeOfferItem {
        slot_index: u8,
        quantity: i32,
    },
    TradeRemoveItem {
        offer_index: u8,
    },
    TradeOfferGold {
        amount: i32,
    },
    TradeAccept,
    TradeCancel,
    // Stall commands
    StallOpen {
        name: String,
    },
    StallClose,
    StallSetItem {
        inventory_slot: u8,
        quantity: i32,
        price: i32,
    },
    StallRemoveItem {
        stall_slot: u8,
    },
    StallBrowse {
        player_id: String,
    },
    StallBuy {
        seller_id: String,
        stall_slot: u8,
        quantity: i32,
    },
    // Combat style
    SetCombatStyle {
        style: String,
    },
}

/// Cardinal directions for isometric movement (no diagonals)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CardinalDir {
    None,
    Up,
    Down,
    Left,
    Right,
}

impl CardinalDir {
    /// Convert to server direction enum value (matches Direction enum)
    fn to_direction_u8(self) -> u8 {
        match self {
            CardinalDir::Down => 0,
            CardinalDir::Left => 1,
            CardinalDir::Up => 2,
            CardinalDir::Right => 3,
            CardinalDir::None => 0, // Default to down
        }
    }
}

/// Threshold for distinguishing face vs move (in seconds)
const FACE_THRESHOLD: f64 = 0.15; // 150ms - time to hold before movement starts (taps shorter than this = face only)
const MINIMAP_PANEL_MIN_ZOOM: f32 = 1.0;
const MINIMAP_PANEL_MAX_ZOOM: f32 = 6.0;

#[derive(Clone, Copy, Debug)]
struct MinimapBounds {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

impl MinimapBounds {
    fn width(&self) -> f32 {
        (self.max_x - self.min_x).max(1.0)
    }

    fn height(&self) -> f32 {
        (self.max_y - self.min_y).max(1.0)
    }
}

fn minimap_panel_rect() -> Rect {
    let (sw, sh) = virtual_screen_size();
    let panel_w = (sw * 0.72).clamp(420.0, 760.0);
    let panel_h = (sh * 0.72).clamp(320.0, 620.0);
    Rect::new(
        ((sw - panel_w) * 0.5).floor(),
        ((sh - panel_h) * 0.5).floor(),
        panel_w,
        panel_h,
    )
}

fn minimap_map_rect(panel_rect: Rect) -> Rect {
    Rect::new(
        panel_rect.x + 14.0,
        panel_rect.y + 34.0,
        panel_rect.w - 28.0,
        panel_rect.h - 86.0,
    )
}

fn minimap_world_bounds(state: &GameState) -> Option<MinimapBounds> {
    let mut bounds = if let Some((width, height)) = state.chunk_manager.get_interior_size() {
        MinimapBounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: width as f32,
            max_y: height as f32,
        }
    } else if !state.chunk_manager.chunks().is_empty() {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for coord in state.chunk_manager.chunks().keys() {
            let chunk_x = (coord.x * CHUNK_SIZE as i32) as f32;
            let chunk_y = (coord.y * CHUNK_SIZE as i32) as f32;
            min_x = min_x.min(chunk_x);
            min_y = min_y.min(chunk_y);
            max_x = max_x.max(chunk_x + CHUNK_SIZE as f32);
            max_y = max_y.max(chunk_y + CHUNK_SIZE as f32);
        }

        MinimapBounds {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    } else if let Some(player) = state.get_local_player() {
        let radius = 24.0;
        MinimapBounds {
            min_x: player.x - radius,
            min_y: player.y - radius,
            max_x: player.x + radius,
            max_y: player.y + radius,
        }
    } else {
        return None;
    };

    if let Some(player) = state.get_local_player() {
        bounds.min_x = bounds.min_x.min(player.x);
        bounds.min_y = bounds.min_y.min(player.y);
        bounds.max_x = bounds.max_x.max(player.x);
        bounds.max_y = bounds.max_y.max(player.y);
    }

    let padding = 2.0;
    bounds.min_x -= padding;
    bounds.min_y -= padding;
    bounds.max_x += padding;
    bounds.max_y += padding;
    if bounds.max_x <= bounds.min_x {
        bounds.max_x = bounds.min_x + 1.0;
    }
    if bounds.max_y <= bounds.min_y {
        bounds.max_y = bounds.min_y + 1.0;
    }
    Some(bounds)
}

fn minimap_view_size(world_bounds: MinimapBounds, zoom: f32) -> (f32, f32) {
    let clamped_zoom = zoom.clamp(MINIMAP_PANEL_MIN_ZOOM, MINIMAP_PANEL_MAX_ZOOM);
    (
        (world_bounds.width() / clamped_zoom).clamp(1.0, world_bounds.width()),
        (world_bounds.height() / clamped_zoom).clamp(1.0, world_bounds.height()),
    )
}

fn minimap_clamp_center(
    world_bounds: MinimapBounds,
    view_w: f32,
    view_h: f32,
    center_x: f32,
    center_y: f32,
) -> (f32, f32) {
    let half_w = view_w * 0.5;
    let half_h = view_h * 0.5;
    let min_cx = world_bounds.min_x + half_w;
    let max_cx = world_bounds.max_x - half_w;
    let min_cy = world_bounds.min_y + half_h;
    let max_cy = world_bounds.max_y - half_h;
    (
        center_x.clamp(min_cx, max_cx),
        center_y.clamp(min_cy, max_cy),
    )
}

fn minimap_panel_view_bounds(state: &GameState, world_bounds: MinimapBounds) -> MinimapBounds {
    let (view_w, view_h) = minimap_view_size(world_bounds, state.ui_state.minimap_panel_zoom);
    let default_center = state.get_local_player().map(|p| (p.x, p.y)).unwrap_or((
        (world_bounds.min_x + world_bounds.max_x) * 0.5,
        (world_bounds.min_y + world_bounds.max_y) * 0.5,
    ));
    let center_x = state
        .ui_state
        .minimap_panel_center_x
        .unwrap_or(default_center.0);
    let center_y = state
        .ui_state
        .minimap_panel_center_y
        .unwrap_or(default_center.1);
    let (center_x, center_y) =
        minimap_clamp_center(world_bounds, view_w, view_h, center_x, center_y);
    let half_w = view_w * 0.5;
    let half_h = view_h * 0.5;

    MinimapBounds {
        min_x: center_x - half_w,
        min_y: center_y - half_h,
        max_x: center_x + half_w,
        max_y: center_y + half_h,
    }
}

fn minimap_screen_to_world(
    bounds: MinimapBounds,
    map_rect: Rect,
    screen_x: f32,
    screen_y: f32,
) -> (f32, f32) {
    let nx = ((screen_x - map_rect.x) / map_rect.w.max(1.0)).clamp(0.0, 1.0);
    let ny = ((screen_y - map_rect.y) / map_rect.h.max(1.0)).clamp(0.0, 1.0);
    (
        bounds.min_x + nx * bounds.width(),
        bounds.min_y + ny * bounds.height(),
    )
}

pub struct InputHandler {
    // Track last sent velocity to detect changes
    last_dx: f32,
    last_dy: f32,
    // Track which direction was pressed first (for priority)
    current_dir: CardinalDir,
    // Track previous direction for detecting key release
    prev_dir: CardinalDir,
    // Periodic movement-intent resend interval
    last_send_time: f64,
    send_interval: f64,
    // Attack cooldown tracking (matches server cooldown)
    last_attack_time: f64,
    // Track when current direction key was pressed (for face vs move)
    dir_press_time: f64,
    // Track if we've sent a move command for the current key press
    move_sent: bool,
    // Auto-path movement is step-driven: one move per waypoint transition.
    auto_path_sent_waypoint: Option<(i32, i32)>,
    auto_path_sent_dir: Option<(f32, f32)>,
    // Touch controls for mobile devices
    pub touch_controls: TouchControls,
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            last_dx: 0.0,
            last_dy: 0.0,
            current_dir: CardinalDir::None,
            prev_dir: CardinalDir::None,
            last_send_time: 0.0,
            send_interval: 0.05, // 50ms keeps facing/move intent responsive
            last_attack_time: 0.0,
            dir_press_time: 0.0,
            move_sent: false,
            auto_path_sent_waypoint: None,
            auto_path_sent_dir: None,
            touch_controls: TouchControls::new(),
        }
    }

    /// Load touch control icons (call once after creation in async context)
    pub async fn load_touch_icons(&mut self) {
        self.touch_controls.load_icons().await;
    }

    fn reset_auto_path_motion_state(&mut self) {
        self.auto_path_sent_waypoint = None;
        self.auto_path_sent_dir = None;
    }

    fn update_touch_controls(&mut self, state: &GameState, current_time: f64) {
        let in_dialogue = state.ui_state.active_dialogue.is_some();
        let any_panel_open = state.ui_state.inventory_open
            || state.ui_state.character_panel_open
            || state.ui_state.skills_open
            || state.ui_state.prayer_book_open
            || state.ui_state.minimap_panel_open
            || state.ui_state.escape_menu_open
            || state.ui_state.crafting_open
            || state.ui_state.furnace_open
            || state.ui_state.anvil_open
            || state.ui_state.fletching_open
            || state.ui_state.shop_data.is_some()
            || state.ui_state.bank_open
            || state.ui_state.chest_open
            || state.ui_state.social_open
            || state.ui_state.chat_panel_open
            || state.ui_state.slayer_panel_open
            || in_dialogue;
        let hide_action_buttons = any_panel_open;
        let hide_direction_controls = state.ui_state.crafting_open
            || state.ui_state.furnace_open
            || state.ui_state.anvil_open
            || state.ui_state.fletching_open
            || state.ui_state.shop_data.is_some()
            || state.ui_state.bank_open
            || state.ui_state.chest_open
            || state.ui_state.minimap_panel_open
            || in_dialogue;
        self.touch_controls.update(
            current_time,
            hide_action_buttons,
            hide_direction_controls,
            state.ui_state.use_joystick,
        );
    }

    fn update_hover_state(&self, state: &mut GameState, layout: &UiLayout, mx: f32, my: f32) {
        state.ui_state.hovered_element = layout.hit_test(mx, my).cloned();
        mark_chat_channel_as_read(state, state.ui_state.chat_active_tab);

        let touch_active = self.touch_controls.consumed_touch();
        if state.ui_state.hovered_element.is_none() && !touch_active {
            let (world_x, world_y) = screen_to_world(mx, my, &state.camera);
            let tile_x = world_x.round() as i32;
            let tile_y = world_y.round() as i32;
            state.hovered_tile = Some((tile_x, tile_y));

            let hover_radius = 0.6;
            let mut hovered_entity: Option<String> = None;

            for npc in state.npcs.values() {
                if npc.state != crate::game::npc::NpcState::Dead {
                    let dx = world_x - npc.x;
                    let dy = world_y - npc.y;
                    if dx * dx + dy * dy < hover_radius * hover_radius {
                        hovered_entity = Some(npc.id.clone());
                        break;
                    }
                }
            }

            if hovered_entity.is_none() {
                for player in state.players.values() {
                    if !player.is_dead {
                        let dx = world_x - player.x;
                        let dy = world_y - player.y;
                        if dx * dx + dy * dy < hover_radius * hover_radius {
                            hovered_entity = Some(player.id.clone());
                            break;
                        }
                    }
                }
            }

            state.hovered_entity_id = hovered_entity;
        } else {
            state.hovered_tile = None;
            state.hovered_entity_id = None;
        }
    }

    fn current_click_target(
        &self,
        layout: &UiLayout,
        mx: f32,
        my: f32,
    ) -> (bool, bool, bool, Option<UiElementId>) {
        let touch_consumed = self.touch_controls.consumed_touch();
        let mouse_clicked = is_mouse_button_pressed(MouseButton::Left) && !touch_consumed;
        let mouse_right_clicked = is_mouse_button_pressed(MouseButton::Right);
        let mouse_released = is_mouse_button_released(MouseButton::Left) && !touch_consumed;
        let clicked_element = if mouse_clicked || mouse_right_clicked || mouse_released {
            layout.hit_test(mx, my).cloned()
        } else {
            None
        };

        (
            mouse_clicked,
            mouse_right_clicked,
            mouse_released,
            clicked_element,
        )
    }

    fn handle_drag_drop(
        &self,
        state: &mut GameState,
        clicked_element: Option<&UiElementId>,
        audio: &mut AudioManager,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        let Some(drag) = state.ui_state.drag_state.take() else {
            return false;
        };

        if let Some(element) = clicked_element {
            match element {
                UiElementId::InventorySlot(to_idx) => match &drag.source {
                    DragSource::Inventory(from_idx) => {
                        if *from_idx != *to_idx {
                            state.inventory.swap_slots(*from_idx, *to_idx);
                            audio.play_sfx("item_put");

                            commands.push(InputCommand::SwapSlots {
                                from_slot: *from_idx as u8,
                                to_slot: *to_idx as u8,
                            });
                        }
                    }
                    DragSource::Equipment(slot_type) => {
                        if state
                            .inventory
                            .slots
                            .get(*to_idx)
                            .map(|s| s.is_none())
                            .unwrap_or(false)
                        {
                            state
                                .inventory
                                .set_slot(*to_idx, drag.item_id.clone(), drag.quantity);

                            if let Some(local_id) = &state.local_player_id.clone() {
                                if let Some(player) = state.players.get_mut(local_id) {
                                    match slot_type.as_str() {
                                        "head" => player.equipped_head = None,
                                        "body" => player.equipped_body = None,
                                        "weapon" => player.equipped_weapon = None,
                                        "back" => player.equipped_back = None,
                                        "feet" => player.equipped_feet = None,
                                        "ring" => player.equipped_ring = None,
                                        "gloves" => player.equipped_gloves = None,
                                        "necklace" => player.equipped_necklace = None,
                                        "belt" => player.equipped_belt = None,
                                        _ => {}
                                    }
                                }
                            }
                        }

                        audio.play_sfx("item_put");
                        commands.push(InputCommand::Unequip {
                            slot_type: slot_type.clone(),
                            target_slot: Some(*to_idx as u8),
                        });
                    }
                    DragSource::Spell(_) => {}
                },
                UiElementId::QuickSlot(slot_idx) | UiElementId::HotkeySettingsSlot(slot_idx) => {
                    match &drag.source {
                        DragSource::Inventory(inv_idx) => {
                            if let Some(Some(slot)) = state.inventory.slots.get(*inv_idx) {
                                state.ui_state.hotkey_bar.active_mut().slots[*slot_idx] =
                                    crate::game::hotkey::HotkeySlotBinding::Item {
                                        item_id: slot.item_id.clone(),
                                    };
                                save_current_ui_settings(state);
                                audio.play_sfx("item_put");
                            }
                        }
                        DragSource::Spell(spell_id) => {
                            state.ui_state.hotkey_bar.active_mut().slots[*slot_idx] =
                                crate::game::hotkey::HotkeySlotBinding::Spell {
                                    spell_id: spell_id.clone(),
                                };
                            save_current_ui_settings(state);
                            audio.play_sfx("item_put");
                        }
                        DragSource::Equipment(_) => {}
                    }
                }
                UiElementId::EquipmentSlot(target_slot_type) => match &drag.source {
                    DragSource::Inventory(from_idx) => {
                        let item_def = state.item_registry.get_or_placeholder(&drag.item_id);
                        let can_equip = if let Some(ref equip) = item_def.equipment {
                            let slot_matches = equip.slot_type == *target_slot_type;
                            let level_ok = state
                                .get_local_player()
                                .map(|p| {
                                    p.skills.attack.level >= equip.attack_level_required
                                        && p.skills.defence.level >= equip.defence_level_required
                                        && p.skills.ranged.level >= equip.ranged_level_required
                                })
                                .unwrap_or(false);
                            slot_matches && level_ok
                        } else {
                            false
                        };

                        if can_equip {
                            if let Some(local_id) = &state.local_player_id.clone() {
                                if let Some(player) = state.players.get_mut(local_id) {
                                    match target_slot_type.as_str() {
                                        "head" => player.equipped_head = Some(drag.item_id.clone()),
                                        "body" => player.equipped_body = Some(drag.item_id.clone()),
                                        "weapon" => {
                                            player.equipped_weapon = Some(drag.item_id.clone())
                                        }
                                        "back" => player.equipped_back = Some(drag.item_id.clone()),
                                        "feet" => player.equipped_feet = Some(drag.item_id.clone()),
                                        "ring" => player.equipped_ring = Some(drag.item_id.clone()),
                                        "gloves" => {
                                            player.equipped_gloves = Some(drag.item_id.clone())
                                        }
                                        "necklace" => {
                                            player.equipped_necklace = Some(drag.item_id.clone())
                                        }
                                        "belt" => player.equipped_belt = Some(drag.item_id.clone()),
                                        _ => {}
                                    }
                                }
                            }
                            state.inventory.clear_slot(*from_idx);
                            audio.play_sfx("item_put");

                            commands.push(InputCommand::Equip {
                                slot_index: *from_idx as u8,
                            });
                        }
                    }
                    DragSource::Equipment(source_slot_type) => {
                        if source_slot_type != target_slot_type {}
                    }
                    DragSource::Spell(_) => {}
                },
                UiElementId::ChestSlot(_) | UiElementId::ChestScrollArea => {
                    if state.ui_state.chest_open {
                        if let DragSource::Inventory(from_idx) = &drag.source {
                            commands.push(InputCommand::ChestDeposit {
                                chest_id: state.ui_state.chest_id.clone(),
                                inventory_slot: *from_idx as u8,
                            });
                            audio.play_sfx("item_put");
                        }
                    }
                }
                _ => {}
            }
        } else if let DragSource::Inventory(from_idx) = &drag.source {
            if let Some((tile_x, tile_y)) = state.hovered_tile {
                if let Some(player) = state.get_local_player() {
                    let player_x = player.x.round() as i32;
                    let player_y = player.y.round() as i32;
                    let dx = (tile_x - player_x).abs();
                    let dy = (tile_y - player_y).abs();
                    let is_adjacent = dx <= 1 && dy <= 1;

                    if is_adjacent {
                        let is_seed_on_patch = if let Some(patch_id) =
                            state.farming_patch_positions.get(&(tile_x, tile_y))
                        {
                            if let Some(patch) = state.farming_patches.get(patch_id) {
                                if patch.state == "empty" {
                                    if let Some(Some(slot)) = state.inventory.slots.get(*from_idx) {
                                        if slot.item_id.ends_with("_seed") {
                                            commands.push(InputCommand::PlantSeed {
                                                patch_id: patch_id.clone(),
                                                item_id: slot.item_id.clone(),
                                            });
                                            audio.play_sfx("item_put");
                                            true
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        let is_bones_on_altar = if !is_seed_on_patch {
                            if let Some(Some(slot)) = state.inventory.slots.get(*from_idx) {
                                if slot.item_id.contains("bones") {
                                    let mut altar_id = None;
                                    for (npc_id, npc) in &state.npcs {
                                        if npc.is_altar
                                            && npc.x.round() as i32 == tile_x
                                            && npc.y.round() as i32 == tile_y
                                        {
                                            altar_id = Some(npc_id.clone());
                                            break;
                                        }
                                    }
                                    if let Some(aid) = altar_id {
                                        commands.push(InputCommand::OfferBones {
                                            slot: *from_idx as u8,
                                            altar_id: aid,
                                        });
                                        audio.play_sfx("item_put");
                                        true
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        if !is_seed_on_patch && !is_bones_on_altar {
                            let ctrl_held = is_key_down(KeyCode::LeftControl)
                                || is_key_down(KeyCode::RightControl)
                                || is_key_down(KeyCode::LeftSuper)
                                || is_key_down(KeyCode::RightSuper);

                            let quantity = if ctrl_held { 1 } else { drag.quantity as u32 };

                            commands.push(InputCommand::DropItem {
                                slot_index: *from_idx as u8,
                                quantity,
                                target_x: Some(tile_x),
                                target_y: Some(tile_y),
                            });
                            audio.play_sfx("item_put");
                        }
                    }
                }
            }
        }

        true
    }

    fn handle_modal_panels(
        state: &mut GameState,
        layout: &UiLayout,
        clicked_element: Option<&UiElementId>,
        mouse_clicked: bool,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        if state.ui_state.gold_drop_dialog.is_some() {
            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::GoldDropConfirm => {
                            let dialog = state.ui_state.gold_drop_dialog.as_ref().unwrap();
                            if let Ok(amount) = dialog.input.parse::<i32>() {
                                if amount > 0 && amount <= state.inventory.gold {
                                    if state.ui_state.trade_open {
                                        commands.push(InputCommand::TradeOfferGold { amount });
                                    } else {
                                        commands.push(InputCommand::DropGold { amount });
                                    }
                                    state.ui_state.gold_drop_dialog = None;
                                }
                            }
                            return true;
                        }
                        UiElementId::GoldDropCancel => {
                            state.ui_state.gold_drop_dialog = None;
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.gold_drop_dialog = None;
                return true;
            }

            if is_key_pressed(KeyCode::Enter) {
                let dialog = state.ui_state.gold_drop_dialog.as_ref().unwrap();
                if let Ok(amount) = dialog.input.parse::<i32>() {
                    if amount > 0 && amount <= state.inventory.gold {
                        if state.ui_state.trade_open {
                            commands.push(InputCommand::TradeOfferGold { amount });
                        } else {
                            commands.push(InputCommand::DropGold { amount });
                        }
                        state.ui_state.gold_drop_dialog = None;
                    }
                }
                return true;
            }

            let number_keys = [
                (KeyCode::Key0, '0'),
                (KeyCode::Key1, '1'),
                (KeyCode::Key2, '2'),
                (KeyCode::Key3, '3'),
                (KeyCode::Key4, '4'),
                (KeyCode::Key5, '5'),
                (KeyCode::Key6, '6'),
                (KeyCode::Key7, '7'),
                (KeyCode::Key8, '8'),
                (KeyCode::Key9, '9'),
                (KeyCode::Kp0, '0'),
                (KeyCode::Kp1, '1'),
                (KeyCode::Kp2, '2'),
                (KeyCode::Kp3, '3'),
                (KeyCode::Kp4, '4'),
                (KeyCode::Kp5, '5'),
                (KeyCode::Kp6, '6'),
                (KeyCode::Kp7, '7'),
                (KeyCode::Kp8, '8'),
                (KeyCode::Kp9, '9'),
            ];

            for (key, digit) in &number_keys {
                if is_key_pressed(*key) {
                    let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                    if dialog.input.len() < 10 {
                        dialog.input.insert(dialog.cursor, *digit);
                        dialog.cursor += 1;
                    }
                }
            }

            if is_key_pressed(KeyCode::Backspace) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                if dialog.cursor > 0 {
                    dialog.input.remove(dialog.cursor - 1);
                    dialog.cursor -= 1;
                }
            }

            if is_key_pressed(KeyCode::Delete) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                if dialog.cursor < dialog.input.len() {
                    dialog.input.remove(dialog.cursor);
                }
            }

            if is_key_pressed(KeyCode::Left) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                if dialog.cursor > 0 {
                    dialog.cursor -= 1;
                }
            }
            if is_key_pressed(KeyCode::Right) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                if dialog.cursor < dialog.input.len() {
                    dialog.cursor += 1;
                }
            }

            if is_key_pressed(KeyCode::Home) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                dialog.cursor = 0;
            }
            if is_key_pressed(KeyCode::End) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                dialog.cursor = dialog.input.len();
            }

            while get_char_pressed().is_some() {}
            return true;
        }

        if state.ui_state.stall_price_dialog.is_some() {
            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::StallPriceConfirm => {
                            let dialog = state.ui_state.stall_price_dialog.as_ref().unwrap();
                            if let Ok(price) = dialog.input.parse::<i32>() {
                                if price > 0 {
                                    let item_id = dialog.item_id.clone();
                                    commands.push(InputCommand::StallSetItem {
                                        inventory_slot: dialog.inventory_slot,
                                        quantity: dialog.quantity,
                                        price,
                                    });
                                    state.ui_state.stall_last_prices.insert(item_id, price);
                                    state.ui_state.stall_price_dialog = None;
                                }
                            }
                            return true;
                        }
                        UiElementId::StallPriceCancel => {
                            state.ui_state.stall_price_dialog = None;
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.stall_price_dialog = None;
                return true;
            }

            if is_key_pressed(KeyCode::Enter) {
                let dialog = state.ui_state.stall_price_dialog.as_ref().unwrap();
                if let Ok(price) = dialog.input.parse::<i32>() {
                    if price > 0 {
                        let item_id = dialog.item_id.clone();
                        commands.push(InputCommand::StallSetItem {
                            inventory_slot: dialog.inventory_slot,
                            quantity: dialog.quantity,
                            price,
                        });
                        state.ui_state.stall_last_prices.insert(item_id, price);
                        state.ui_state.stall_price_dialog = None;
                    }
                }
                return true;
            }

            let number_keys = [
                (KeyCode::Key0, '0'),
                (KeyCode::Key1, '1'),
                (KeyCode::Key2, '2'),
                (KeyCode::Key3, '3'),
                (KeyCode::Key4, '4'),
                (KeyCode::Key5, '5'),
                (KeyCode::Key6, '6'),
                (KeyCode::Key7, '7'),
                (KeyCode::Key8, '8'),
                (KeyCode::Key9, '9'),
            ];
            for (key, digit) in &number_keys {
                if is_key_pressed(*key) {
                    let dialog = state.ui_state.stall_price_dialog.as_mut().unwrap();
                    if dialog.input.len() < 10 {
                        dialog.input.insert(dialog.cursor, *digit);
                        dialog.cursor += 1;
                    }
                }
            }

            if is_key_pressed(KeyCode::Backspace) {
                let dialog = state.ui_state.stall_price_dialog.as_mut().unwrap();
                if dialog.cursor > 0 {
                    dialog.input.remove(dialog.cursor - 1);
                    dialog.cursor -= 1;
                }
            }

            if is_key_pressed(KeyCode::Delete) {
                let dialog = state.ui_state.stall_price_dialog.as_mut().unwrap();
                if dialog.cursor < dialog.input.len() {
                    dialog.input.remove(dialog.cursor);
                }
            }

            if is_key_pressed(KeyCode::Left) {
                let dialog = state.ui_state.stall_price_dialog.as_mut().unwrap();
                if dialog.cursor > 0 {
                    dialog.cursor -= 1;
                }
            }
            if is_key_pressed(KeyCode::Right) {
                let dialog = state.ui_state.stall_price_dialog.as_mut().unwrap();
                if dialog.cursor < dialog.input.len() {
                    dialog.cursor += 1;
                }
            }

            if is_key_pressed(KeyCode::Home) {
                let dialog = state.ui_state.stall_price_dialog.as_mut().unwrap();
                dialog.cursor = 0;
            }
            if is_key_pressed(KeyCode::End) {
                let dialog = state.ui_state.stall_price_dialog.as_mut().unwrap();
                dialog.cursor = dialog.input.len();
            }

            while get_char_pressed().is_some() {}
            return true;
        }

        if state.ui_state.chest_open {
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                if let Some(UiElementId::ChestScrollArea) = &state.ui_state.hovered_element {
                    let max_scroll = layout
                        .get_max_scroll(&UiElementId::ChestScrollArea)
                        .unwrap_or(0.0);
                    state.ui_state.chest_scroll =
                        (state.ui_state.chest_scroll - wheel_y * 30.0).clamp(0.0, max_scroll);
                }
            }

            let mut chest_handled = false;
            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::ChestClose => {
                            state.ui_state.chest_open = false;
                            state.pending_sfx.push("enter".to_string());
                            chest_handled = true;
                        }
                        UiElementId::ChestSlot(idx) => {
                            if (*idx as usize) < state.ui_state.chest_slots.len() {
                                if state.ui_state.chest_slots[*idx as usize].is_some() {
                                    commands.push(InputCommand::ChestTake {
                                        chest_id: state.ui_state.chest_id.clone(),
                                        slot: *idx,
                                    });
                                }
                            }
                            chest_handled = true;
                        }
                        _ => {}
                    }
                }
            }

            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.chest_open = false;
                return true;
            }

            if chest_handled {
                return true;
            }
        }

        if state.ui_state.slayer_panel_open {
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                if let Some(UiElementId::SlayerScrollArea) = &state.ui_state.hovered_element {
                    state.ui_state.slayer_reward_scroll =
                        (state.ui_state.slayer_reward_scroll - wheel_y * 30.0).max(0.0);
                }
            }

            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::SlayerCloseButton => {
                            state.ui_state.slayer_panel_open = false;
                            state.pending_sfx.push("enter".to_string());
                        }
                        UiElementId::SlayerGetTaskButton => {
                            if let Some(ref master_id) = state.ui_state.slayer_master_id.clone() {
                                commands.push(InputCommand::SlayerGetTask {
                                    master_id: master_id.clone(),
                                });
                            }
                        }
                        UiElementId::SlayerCancelTaskButton => {
                            commands.push(InputCommand::SlayerCancelTask);
                        }
                        UiElementId::SlayerRewardTab(idx) => {
                            state.ui_state.slayer_reward_tab = *idx;
                            state.ui_state.slayer_reward_scroll = 0.0;
                        }
                        UiElementId::SlayerBuyReward(idx) => {
                            if let Some(reward) = state.ui_state.slayer_rewards.get(*idx) {
                                if state.ui_state.slayer_points >= reward.cost {
                                    commands.push(InputCommand::SlayerBuyReward {
                                        reward_id: reward.id.clone(),
                                        target_monster_id: reward.target_id.clone(),
                                    });
                                }
                            }
                        }
                        UiElementId::SlayerRemoveBlock(idx) => {
                            if let Some(monster_name) =
                                state.ui_state.slayer_blocked_monsters.get(*idx)
                            {
                                commands.push(InputCommand::SlayerRemoveBlock {
                                    monster_id: monster_name.clone(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }

            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.slayer_panel_open = false;
                return true;
            }

            return true;
        }

        if state.ui_state.trade_open {
            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::TradeOfferSlot(i) => {
                            commands.push(InputCommand::TradeRemoveItem {
                                offer_index: *i as u8,
                            });
                        }
                        UiElementId::TradeGoldInput => {
                            state.ui_state.gold_drop_dialog = Some(GoldDropDialog {
                                input: String::new(),
                                cursor: 0,
                            });
                        }
                        UiElementId::TradeAcceptButton => {
                            commands.push(InputCommand::TradeAccept);
                        }
                        UiElementId::TradeCancelButton => {
                            commands.push(InputCommand::TradeCancel);
                        }
                        UiElementId::InventorySlot(slot_idx) => {
                            if let Some(slot) = state
                                .inventory
                                .slots
                                .get(*slot_idx)
                                .and_then(|s| s.as_ref())
                            {
                                commands.push(InputCommand::TradeOfferItem {
                                    slot_index: *slot_idx as u8,
                                    quantity: slot.quantity,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }

            if is_key_pressed(KeyCode::Escape) {
                commands.push(InputCommand::TradeCancel);
                return true;
            }

            return true;
        }

        if state.ui_state.trade_pending_request.is_some() && mouse_clicked {
            if let Some(element) = clicked_element {
                match element {
                    UiElementId::TradeRequestAccept => {
                        if let Some((ref requester_id, _)) = state.ui_state.trade_pending_request {
                            commands.push(InputCommand::TradeAcceptRequest {
                                requester_id: requester_id.clone(),
                            });
                        }
                        state.ui_state.trade_pending_request = None;
                    }
                    UiElementId::TradeRequestDecline => {
                        if let Some((ref requester_id, _)) = state.ui_state.trade_pending_request {
                            commands.push(InputCommand::TradeDeclineRequest {
                                requester_id: requester_id.clone(),
                            });
                        }
                        state.ui_state.trade_pending_request = None;
                    }
                    _ => {}
                }
            }
        }

        if state.ui_state.stall_setup_open {
            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::StallSetupNameInput => {
                            state.ui_state.stall_name_editing = true;
                            state.ui_state.stall_name_cursor = state.ui_state.stall_my_name.len();
                        }
                        UiElementId::StallSetupRemove(i) => {
                            state.ui_state.stall_name_editing = false;
                            if let Some(slot) = state.ui_state.stall_my_slots.get(*i) {
                                commands.push(InputCommand::StallRemoveItem {
                                    stall_slot: slot.slot,
                                });
                            }
                        }
                        UiElementId::StallSetupOpenButton => {
                            state.ui_state.stall_name_editing = false;
                            if state.ui_state.stall_active {
                                commands.push(InputCommand::StallClose);
                            } else {
                                let name = if state.ui_state.stall_my_name.is_empty() {
                                    "My Shop".to_string()
                                } else {
                                    state.ui_state.stall_my_name.clone()
                                };
                                commands.push(InputCommand::StallOpen { name });
                            }
                        }
                        UiElementId::StallSetupCloseButton => {
                            state.ui_state.stall_name_editing = false;
                            state.ui_state.stall_setup_open = false;
                        }
                        UiElementId::InventorySlot(slot_idx) => {
                            state.ui_state.stall_name_editing = false;
                            if let Some(slot) = state
                                .inventory
                                .slots
                                .get(*slot_idx)
                                .and_then(|s| s.as_ref())
                            {
                                commands.push(InputCommand::StallSetItem {
                                    inventory_slot: *slot_idx as u8,
                                    quantity: slot.quantity,
                                    price: 1,
                                });
                            }
                        }
                        _ => {
                            state.ui_state.stall_name_editing = false;
                        }
                    }
                } else {
                    state.ui_state.stall_name_editing = false;
                }
            }

            if state.ui_state.stall_name_editing {
                if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Enter) {
                    state.ui_state.stall_name_editing = false;
                    while get_char_pressed().is_some() {}
                    return true;
                }

                if is_key_pressed(KeyCode::Backspace) {
                    if state.ui_state.stall_name_cursor > 0 {
                        state.ui_state.stall_name_cursor -= 1;
                        state
                            .ui_state
                            .stall_my_name
                            .remove(state.ui_state.stall_name_cursor);
                    }
                }
                if is_key_pressed(KeyCode::Delete) {
                    if state.ui_state.stall_name_cursor < state.ui_state.stall_my_name.len() {
                        state
                            .ui_state
                            .stall_my_name
                            .remove(state.ui_state.stall_name_cursor);
                    }
                }
                if is_key_pressed(KeyCode::Left) && state.ui_state.stall_name_cursor > 0 {
                    state.ui_state.stall_name_cursor -= 1;
                }
                if is_key_pressed(KeyCode::Right)
                    && state.ui_state.stall_name_cursor < state.ui_state.stall_my_name.len()
                {
                    state.ui_state.stall_name_cursor += 1;
                }
                if is_key_pressed(KeyCode::Home) {
                    state.ui_state.stall_name_cursor = 0;
                }
                if is_key_pressed(KeyCode::End) {
                    state.ui_state.stall_name_cursor = state.ui_state.stall_my_name.len();
                }

                while let Some(ch) = get_char_pressed() {
                    if ch.is_control() {
                        continue;
                    }
                    if state.ui_state.stall_my_name.len() < 24 {
                        state
                            .ui_state
                            .stall_my_name
                            .insert(state.ui_state.stall_name_cursor, ch);
                        state.ui_state.stall_name_cursor += 1;
                    }
                }

                return true;
            }

            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.stall_setup_open = false;
                state.ui_state.stall_name_editing = false;
                return true;
            }

            return true;
        }

        if state.ui_state.stall_browse.is_some() {
            if mouse_clicked {
                if let Some(element) = clicked_element {
                    match element {
                        UiElementId::StallBrowseItem(i) => {
                            state.ui_state.stall_browse_selected = *i;
                            state.ui_state.stall_buy_quantity = 1;
                        }
                        UiElementId::StallBrowseQuantityMinus => {
                            if state.ui_state.stall_buy_quantity > 1 {
                                state.ui_state.stall_buy_quantity -= 1;
                            }
                        }
                        UiElementId::StallBrowseQuantityPlus => {
                            state.ui_state.stall_buy_quantity += 1;
                            if let Some(ref browse) = state.ui_state.stall_browse {
                                if let Some(item) =
                                    browse.items.get(state.ui_state.stall_browse_selected)
                                {
                                    if state.ui_state.stall_buy_quantity > item.quantity {
                                        state.ui_state.stall_buy_quantity = item.quantity;
                                    }
                                }
                            }
                        }
                        UiElementId::StallBrowseBuyButton => {
                            if let Some(ref browse) = state.ui_state.stall_browse {
                                if let Some(item) =
                                    browse.items.get(state.ui_state.stall_browse_selected)
                                {
                                    commands.push(InputCommand::StallBuy {
                                        seller_id: browse.seller_id.clone(),
                                        stall_slot: item.slot,
                                        quantity: state.ui_state.stall_buy_quantity,
                                    });
                                }
                            }
                        }
                        UiElementId::StallBrowseCloseButton => {
                            state.ui_state.stall_browse = None;
                            state.pending_sfx.push("ui_close".to_string());
                        }
                        _ => {}
                    }
                }
            }

            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.stall_browse = None;
                return true;
            }

            return true;
        }

        false
    }

    pub fn process(
        &mut self,
        state: &mut GameState,
        layout: &UiLayout,
        audio: &mut AudioManager,
    ) -> Vec<InputCommand> {
        let mut commands = Vec::new();
        let current_time = get_time();

        self.update_touch_controls(state, current_time);

        // Get current mouse/touch position in virtual coordinates (for UI hit detection)
        let (raw_mx, raw_my) = mouse_position();
        let (mx, my) = screen_to_virtual_coords(raw_mx, raw_my);

        self.update_hover_state(state, layout, mx, my);
        let (mouse_clicked, mouse_right_clicked, mouse_released, clicked_element) =
            self.current_click_target(layout, mx, my);

        // Toggle debug mode
        if is_key_pressed(KeyCode::F3) {
            // Debug toggle handled in main loop
        }

        if mouse_released
            && self.handle_drag_drop(state, clicked_element.as_ref(), audio, &mut commands)
        {
            return commands;
        }

        // Double-click detection threshold (300ms)
        const DOUBLE_CLICK_THRESHOLD: f64 = 0.3;

        // Start drag on left click on inventory slot with item
        // But first check for double-click to equip
        if mouse_clicked && state.ui_state.drag_state.is_none() {
            if let Some(ref element) = clicked_element {
                match element {
                    UiElementId::InventorySlot(idx) => {
                        // Check if slot has an item
                        if let Some(Some(slot)) = state.inventory.slots.get(*idx) {
                            // If trade is open, add item to trade offer instead of dragging
                            if state.ui_state.trade_open {
                                commands.push(InputCommand::TradeOfferItem {
                                    slot_index: *idx as u8,
                                    quantity: slot.quantity,
                                });
                                return commands;
                            }

                            // If stall setup is open, open price dialog before adding
                            if state.ui_state.stall_setup_open {
                                let item_id = slot.item_id.clone();
                                let last_price = state
                                    .ui_state
                                    .stall_last_prices
                                    .get(&item_id)
                                    .copied()
                                    .unwrap_or(0);
                                let prefill = if last_price > 0 {
                                    last_price.to_string()
                                } else {
                                    String::new()
                                };
                                let cursor = prefill.len();
                                state.ui_state.stall_price_dialog = Some(StallPriceDialog {
                                    input: prefill,
                                    cursor,
                                    inventory_slot: *idx as u8,
                                    quantity: slot.quantity,
                                    item_id,
                                });
                                return commands;
                            }

                            // Check for shift+click to drop (if enabled)
                            let shift_held =
                                is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                            if shift_held && state.ui_state.shift_drop_enabled {
                                // Drop the entire stack at player position
                                commands.push(InputCommand::DropItem {
                                    slot_index: *idx as u8,
                                    quantity: slot.quantity as u32,
                                    target_x: None,
                                    target_y: None,
                                });
                                audio.play_sfx("item_put");
                                return commands;
                            }

                            // Check for double-click
                            let is_double_click = state.ui_state.double_click_state.last_click_slot
                                == Some(*idx)
                                && current_time - state.ui_state.double_click_state.last_click_time
                                    < DOUBLE_CLICK_THRESHOLD;

                            if is_double_click {
                                // Reset double-click state
                                state.ui_state.double_click_state.last_click_slot = None;
                                state.ui_state.double_click_state.last_click_time = 0.0;

                                // Check if item is equippable
                                let item_def =
                                    state.item_registry.get_or_placeholder(&slot.item_id);
                                if item_def.equipment.is_some() {
                                    // Equip the item
                                    commands.push(InputCommand::Equip {
                                        slot_index: *idx as u8,
                                    });
                                    return commands;
                                } else {
                                    // Not equippable - use the item instead (e.g., health potion)
                                    commands.push(InputCommand::UseItem {
                                        slot_index: *idx as u8,
                                    });
                                    return commands;
                                }
                            } else {
                                // First click - record for potential double-click
                                state.ui_state.double_click_state.last_click_slot = Some(*idx);
                                state.ui_state.double_click_state.last_click_time = current_time;

                                // Start drag from inventory
                                state.ui_state.drag_state = Some(DragState {
                                    source: DragSource::Inventory(*idx),
                                    item_id: slot.item_id.clone(),
                                    quantity: slot.quantity,
                                });
                                audio.play_sfx("item_grab");
                                // Don't process other input while starting drag
                                return commands;
                            }
                        }
                    }
                    UiElementId::QuickSlot(idx) => {
                        // Unified hotkey bar: activate on click
                        let cmds = activate_hotkey_slot(state, *idx);
                        commands.extend(cmds);
                        return commands;
                    }
                    UiElementId::SpellSlot(slot_idx) => {
                        // Start drag from spell panel
                        if *slot_idx < crate::game::spell::SPELLS.len() {
                            let spell = &crate::game::spell::SPELLS[*slot_idx];
                            state.ui_state.drag_state = Some(DragState {
                                source: DragSource::Spell(spell.id.to_string()),
                                item_id: spell.id.to_string(),
                                quantity: 0,
                            });
                            audio.play_sfx("item_grab");
                            return commands;
                        } else {
                            // Scroll spell slot - only allow drag if unlocked
                            let scroll_idx = *slot_idx - crate::game::spell::SPELLS.len();
                            if let Some(scroll_spell) =
                                state.scroll_spell_definitions.get(scroll_idx)
                            {
                                if state.unlocked_spells.contains(&scroll_spell.id) {
                                    let id = scroll_spell.id.clone();
                                    state.ui_state.drag_state = Some(DragState {
                                        source: DragSource::Spell(id.clone()),
                                        item_id: id,
                                        quantity: 0,
                                    });
                                    audio.play_sfx("item_grab");
                                    return commands;
                                }
                            }
                        }
                    }
                    UiElementId::EquipmentSlot(slot_type) => {
                        // Check if equipment slot has an item
                        let equipped_item = match slot_type.as_str() {
                            "head" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_head.clone()),
                            "body" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_body.clone()),
                            "weapon" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_weapon.clone()),
                            "back" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_back.clone()),
                            "feet" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_feet.clone()),
                            "ring" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_ring.clone()),
                            "gloves" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_gloves.clone()),
                            "necklace" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_necklace.clone()),
                            "belt" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_belt.clone()),
                            _ => None,
                        };
                        if let Some(item_id) = equipped_item {
                            // Start drag from equipment slot
                            state.ui_state.drag_state = Some(DragState {
                                source: DragSource::Equipment(slot_type.clone()),
                                item_id,
                                quantity: 1, // Equipment is always quantity 1
                            });
                            audio.play_sfx("item_grab");
                            return commands;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Handle context menu interactions first
        if let Some(ref menu) = state.ui_state.context_menu {
            // Auto-hide context menu when mouse leaves its bounds
            // Use generous estimates — exact size depends on text measurement in renderer,
            // but we just need a rough bounding box to dismiss when mouse wanders far away.
            let option_height = 20.0;
            let num_options = match &menu.target {
                ContextMenuTarget::EquipmentSlot(_) => 1,
                ContextMenuTarget::Gold => 1,
                ContextMenuTarget::InventorySlot(slot_index) => {
                    let (is_equippable, is_bones, is_knife) = state
                        .inventory
                        .slots
                        .get(*slot_index)
                        .and_then(|s| s.as_ref())
                        .map(|slot| {
                            let item_def = state.item_registry.get_or_placeholder(&slot.item_id);
                            let equippable = item_def.equipment.is_some();
                            let bones = slot.item_id.contains("bones");
                            let knife = slot.item_id == "knife";
                            (equippable, bones, knife)
                        })
                        .unwrap_or((false, false, false));
                    let has_deposit = state.ui_state.chest_open;
                    1 + if is_equippable { 1 } else { 0 }
                        + if is_bones { 1 } else { 0 }
                        + if is_knife { 1 } else { 0 }
                        + if has_deposit { 1 } else { 0 }
                }
                ContextMenuTarget::Player { .. } => 4,
                ContextMenuTarget::Npc { id } => state
                    .npcs
                    .get(id)
                    .map(|npc| {
                        if npc.is_attackable() {
                            3
                        } else if npc.is_altar {
                            3
                        } else if npc.is_merchant {
                            3
                        } else {
                            2
                        }
                    })
                    .unwrap_or(1),
                ContextMenuTarget::Tree { .. } => 2,
                ContextMenuTarget::Rock { .. } => 2,
                ContextMenuTarget::MapObject { .. } => 2,
                ContextMenuTarget::GatheringSpot { .. } => 2,
                ContextMenuTarget::GroundItem { .. } => 2,
                ContextMenuTarget::FarmingPatch { patch_id } => state
                    .farming_patches
                    .get(patch_id)
                    .map(|p| {
                        if p.state == "harvestable" || p.state == "empty" {
                            2
                        } else {
                            1
                        }
                    })
                    .unwrap_or(1),
                ContextMenuTarget::Tile { .. } => 1,
                ContextMenuTarget::HotkeySlot(_) => 1, // "Clear Slot"
            };

            let menu_width = 140.0; // generous estimate
            let menu_height = option_height + num_options as f32 * option_height + 4.0;

            let mut menu_x = menu.x.floor();
            let mut menu_y = menu.y.floor();
            let screen_w = screen_width();
            let screen_h = screen_height();
            if menu_x + menu_width > screen_w {
                menu_x = (screen_w - menu_width - 2.0).floor();
            }
            if menu_y + menu_height > screen_h {
                menu_y = (screen_h - menu_height - 2.0).floor();
            }

            let margin = 6.0;
            let is_mouse_inside = mx >= menu_x - margin
                && mx <= menu_x + menu_width + margin
                && my >= menu_y - margin
                && my <= menu_y + menu_height + margin;

            if !is_mouse_inside {
                state.ui_state.context_menu = None;
            }
        }

        if state.ui_state.context_menu.is_some() {
            if let Some(ref element) = clicked_element {
                if mouse_clicked {
                    match element {
                        UiElementId::ContextMenuOption(option_idx) => {
                            // Get menu info before clearing it
                            let menu = state.ui_state.context_menu.take().unwrap();

                            match &menu.target {
                                ContextMenuTarget::EquipmentSlot(slot_type) => {
                                    // Equipment slot context menu - only unequip option
                                    if *option_idx == 0 {
                                        commands.push(InputCommand::Unequip {
                                            slot_type: slot_type.clone(),
                                            target_slot: None, // Use first available slot
                                        });
                                    }
                                }
                                ContextMenuTarget::Gold => {
                                    // Gold context menu - only drop option
                                    if *option_idx == 0 {
                                        // Open gold drop dialog
                                        state.ui_state.gold_drop_dialog = Some(GoldDropDialog {
                                            input: String::new(),
                                            cursor: 0,
                                        });
                                    }
                                }
                                ContextMenuTarget::InventorySlot(slot_index) => {
                                    // Inventory slot context menu
                                    // Determine menu options based on item type
                                    let (is_equippable, is_bones, is_dig, is_knife, has_item) =
                                        state
                                            .inventory
                                            .slots
                                            .get(*slot_index)
                                            .and_then(|s| s.as_ref())
                                            .map(|slot| {
                                                let item_def = state
                                                    .item_registry
                                                    .get_or_placeholder(&slot.item_id);
                                                let equippable = item_def.equipment.is_some();
                                                let bones = slot.item_id.contains("bones");
                                                let dig =
                                                    item_def.use_effect.as_deref() == Some("dig");
                                                let knife = slot.item_id == "knife";
                                                (equippable, bones, dig, knife, true)
                                            })
                                            .unwrap_or((false, false, false, false, false));
                                    let chest_open = state.ui_state.chest_open && has_item;

                                    // Build option index mapping: [Equip?] [Bury?] [Dig?] [Fletch?] [Deposit?] Drop
                                    let mut current_idx = 0usize;
                                    let equip_idx = if is_equippable {
                                        let idx = current_idx;
                                        current_idx += 1;
                                        Some(idx)
                                    } else {
                                        None
                                    };
                                    let bury_idx = if is_bones {
                                        let idx = current_idx;
                                        current_idx += 1;
                                        Some(idx)
                                    } else {
                                        None
                                    };
                                    let dig_idx = if is_dig {
                                        let idx = current_idx;
                                        current_idx += 1;
                                        Some(idx)
                                    } else {
                                        None
                                    };
                                    let fletch_idx = if is_knife {
                                        let idx = current_idx;
                                        current_idx += 1;
                                        Some(idx)
                                    } else {
                                        None
                                    };
                                    let deposit_idx = if chest_open {
                                        let idx = current_idx;
                                        current_idx += 1;
                                        Some(idx)
                                    } else {
                                        None
                                    };
                                    let drop_idx = current_idx;

                                    if Some(*option_idx) == equip_idx {
                                        commands.push(InputCommand::Equip {
                                            slot_index: *slot_index as u8,
                                        });
                                    } else if Some(*option_idx) == bury_idx {
                                        commands.push(InputCommand::BuryBones {
                                            slot: *slot_index as u8,
                                        });
                                    } else if Some(*option_idx) == dig_idx {
                                        commands.push(InputCommand::UseItem {
                                            slot_index: *slot_index as u8,
                                        });
                                    } else if Some(*option_idx) == fletch_idx {
                                        state.ui_state.fletching_open = true;
                                        state.ui_state.fletching_selected_recipe = 0;
                                        state.ui_state.fletching_scroll_offset = 0.0;
                                        state.ui_state.fletching_quantity = 1;
                                        state.ui_state.fletching_tab = 0;
                                        state.pending_sfx.push("ui_open".to_string());
                                    } else if Some(*option_idx) == deposit_idx {
                                        commands.push(InputCommand::ChestDeposit {
                                            chest_id: state.ui_state.chest_id.clone(),
                                            inventory_slot: *slot_index as u8,
                                        });
                                    } else if *option_idx == drop_idx {
                                        if let Some(slot) = state
                                            .inventory
                                            .slots
                                            .get(*slot_index)
                                            .and_then(|s| s.as_ref())
                                        {
                                            commands.push(InputCommand::DropItem {
                                                slot_index: *slot_index as u8,
                                                quantity: slot.quantity as u32,
                                                target_x: None,
                                                target_y: None,
                                            });
                                        }
                                    }
                                }
                                // === World context menu targets ===
                                ContextMenuTarget::Player { id } => {
                                    // Options: 0=Attack, 1=Follow, 2=Trade, [3=Browse Shop if stall], N=Add Friend, N+1=Examine
                                    let player_has_stall =
                                        state.players.get(id).map_or(false, |p| p.has_stall);
                                    let mut ci = 0usize;
                                    let attack_idx = {
                                        let idx = ci;
                                        ci += 1;
                                        idx
                                    };
                                    let follow_idx = {
                                        let idx = ci;
                                        ci += 1;
                                        idx
                                    };
                                    let trade_idx = {
                                        let idx = ci;
                                        ci += 1;
                                        idx
                                    };
                                    let browse_shop_idx = if player_has_stall {
                                        let idx = ci;
                                        ci += 1;
                                        Some(idx)
                                    } else {
                                        None
                                    };
                                    let add_friend_idx = {
                                        let idx = ci;
                                        ci += 1;
                                        idx
                                    };
                                    let examine_idx = ci;

                                    if *option_idx == attack_idx {
                                        commands.push(InputCommand::Target {
                                            entity_id: id.clone(),
                                        });
                                        state.auto_action_state =
                                            Some(crate::game::AutoActionState {
                                                target_type: "player".to_string(),
                                                target_id: id.clone(),
                                                action: "attack".to_string(),
                                                confirmed: false,
                                            });
                                        pathfind_and_attack_player(state, &mut commands, id);
                                    } else if *option_idx == follow_idx {
                                        state.follow_target = Some(id.clone());
                                        if state.auto_action_state.is_some() {
                                            state.auto_action_state = None;
                                            commands.push(InputCommand::CancelAutoAction);
                                        }
                                        if let Some(local_id) = &state.local_player_id.clone() {
                                            if let Some(player) = state.players.get(local_id) {
                                                if let Some(target) = state.players.get(id) {
                                                    let px = player.server_x.round() as i32;
                                                    let py = player.server_y.round() as i32;
                                                    let tx = target.server_x.round() as i32;
                                                    let ty = target.server_y.round() as i32;
                                                    let mut occupied =
                                                        build_occupied_set(state, true);
                                                    occupied.remove(&(tx, ty));
                                                    const MAX_PATH_DISTANCE: i32 = 32;
                                                    if let Some((dest, path)) =
                                                        pathfinding::find_path_to_adjacent(
                                                            (px, py),
                                                            (tx, ty),
                                                            &state.chunk_manager,
                                                            &occupied,
                                                            MAX_PATH_DISTANCE,
                                                        )
                                                    {
                                                        state.auto_path = Some(PathState {
                                                            path,
                                                            current_index: 0,
                                                            destination: dest,
                                                            pickup_target: None,
                                                            interact_target: None,
                                                            interact_object_target: None,
                                                            waystone_target: None,
                                                            browse_stall_target: None,
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    } else if *option_idx == trade_idx {
                                        // Send trade request
                                        commands.push(InputCommand::TradeRequest {
                                            target_id: id.clone(),
                                        });
                                    } else if browse_shop_idx == Some(*option_idx) {
                                        // Browse this player's stall
                                        commands.push(InputCommand::StallBrowse {
                                            player_id: id.clone(),
                                        });
                                    } else if *option_idx == add_friend_idx {
                                        if let Some(player) = state.players.get(id) {
                                            commands.push(InputCommand::SendFriendRequest {
                                                target_name: player.name.clone(),
                                            });
                                        }
                                    } else if *option_idx == examine_idx {
                                        if let Some(player) = state.players.get(id) {
                                            let msg = format!(
                                                "{} (level {})",
                                                player.name,
                                                player.combat_level()
                                            );
                                            state.push_system_chat(msg);
                                        }
                                    }
                                }
                                ContextMenuTarget::Npc { id } => {
                                    if let Some(npc) = state.npcs.get(id) {
                                        let is_attackable = npc.is_attackable();
                                        let is_altar = npc.is_altar;
                                        let is_merchant = npc.is_merchant;
                                        let is_banker = npc.is_banker;
                                        let is_slayer_master = npc.is_slayer_master;
                                        let has_station = npc.station_type.is_some();
                                        let npc_name = npc.display_name.clone();
                                        let npc_level = npc.level;
                                        let npc_entity_type = npc.entity_type.clone();
                                        let npc_id = id.clone();

                                        if is_attackable {
                                            // Options: 0=Attack, 1=Target, 2=Examine
                                            match option_idx {
                                                0 => {
                                                    commands.push(InputCommand::Target {
                                                        entity_id: npc_id.clone(),
                                                    });
                                                    state.auto_action_state =
                                                        Some(crate::game::AutoActionState {
                                                            target_type: "npc".to_string(),
                                                            target_id: npc_id.clone(),
                                                            action: "attack".to_string(),
                                                            confirmed: false,
                                                        });
                                                    pathfind_and_attack_npc(
                                                        state,
                                                        &mut commands,
                                                        &npc_id,
                                                    );
                                                }
                                                1 => {
                                                    // Target only — select without attacking or moving
                                                    commands.push(InputCommand::Target {
                                                        entity_id: npc_id.clone(),
                                                    });
                                                }
                                                2 => {
                                                    let msg = format!(
                                                        "{} (level {})",
                                                        npc_name, npc_level
                                                    );
                                                    state.push_system_chat(msg);
                                                }
                                                _ => {}
                                            }
                                        } else if is_altar {
                                            // Options: 0=Pray, 1=Offer Bones, 2=Examine
                                            match option_idx {
                                                0 => {
                                                    // Pray at altar
                                                    pathfind_and_interact_npc(
                                                        state,
                                                        &mut commands,
                                                        &npc_id,
                                                        |_state, commands, npc_id| {
                                                            commands.push(
                                                                InputCommand::PrayAtAltar {
                                                                    altar_id: npc_id.to_string(),
                                                                },
                                                            );
                                                        },
                                                    );
                                                }
                                                1 => {
                                                    // Offer Bones - open altar panel
                                                    pathfind_and_interact_npc(
                                                        state,
                                                        &mut commands,
                                                        &npc_id,
                                                        |state, _commands, npc_id| {
                                                            if let Some(npc) =
                                                                state.npcs.get(npc_id)
                                                            {
                                                                state.ui_state.altar_panel = Some(
                                                                    crate::game::AltarPanelState {
                                                                        altar_npc_id: npc_id
                                                                            .to_string(),
                                                                        altar_name: npc
                                                                            .display_name
                                                                            .clone(),
                                                                    },
                                                                );
                                                            }
                                                        },
                                                    );
                                                }
                                                2 => {
                                                    let msg =
                                                        format!("An altar dedicated to the gods.");
                                                    state.push_system_chat(msg);
                                                }
                                                _ => {}
                                            }
                                        } else if has_station {
                                            // Options: 0=Use, 1=Examine
                                            match option_idx {
                                                0 => {
                                                    pathfind_and_interact_npc(
                                                        state,
                                                        &mut commands,
                                                        &npc_id,
                                                        |state, _commands, npc_id| {
                                                            if let Some(npc) =
                                                                state.npcs.get(npc_id)
                                                            {
                                                                match npc.station_type.as_deref() {
                                                                    Some("furnace") => {
                                                                        state
                                                                            .ui_state
                                                                            .furnace_station_type =
                                                                            "furnace".to_string();
                                                                        state
                                                                            .ui_state
                                                                            .fletching_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_open = true;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_tile = Some((
                                                                            npc.x.round() as i32,
                                                                            npc.y.round() as i32,
                                                                        ));
                                                                        state.ui_state.furnace_selected_recipe = 0;
                                                                        state.ui_state.furnace_scroll_offset = 0.0;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_quantity = 1;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_tab = 0;
                                                                    }
                                                                    Some("fire_pit") => {
                                                                        state
                                                                            .ui_state
                                                                            .furnace_station_type =
                                                                            "fire_pit".to_string();
                                                                        state
                                                                            .ui_state
                                                                            .fletching_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_open = true;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_tile = Some((
                                                                            npc.x.round() as i32,
                                                                            npc.y.round() as i32,
                                                                        ));
                                                                        state.ui_state.furnace_selected_recipe = 0;
                                                                        state.ui_state.furnace_scroll_offset = 0.0;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_quantity = 1;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_tab = 0;
                                                                    }
                                                                    Some("anvil") => {
                                                                        state
                                                                            .ui_state
                                                                            .fletching_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_open = false;
                                                                        state.ui_state.anvil_open =
                                                                            true;
                                                                        state.ui_state.anvil_tile =
                                                                            Some((
                                                                                npc.x.round()
                                                                                    as i32,
                                                                                npc.y.round()
                                                                                    as i32,
                                                                            ));
                                                                        state.ui_state.anvil_selected_recipe = 0;
                                                                        state
                                                                            .ui_state
                                                                            .anvil_scroll_offset =
                                                                            0.0;
                                                                        state
                                                                            .ui_state
                                                                            .anvil_quantity = 1;
                                                                        state.ui_state.anvil_tab =
                                                                            0;
                                                                    }
                                                                    Some("alchemy_station") => {
                                                                        state
                                                                            .ui_state
                                                                            .fletching_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .alchemy_station_open =
                                                                            true;
                                                                        state
                                                                            .ui_state
                                                                            .alchemy_station_tile =
                                                                            Some((
                                                                                npc.x.round()
                                                                                    as i32,
                                                                                npc.y.round()
                                                                                    as i32,
                                                                            ));
                                                                        state.ui_state.alchemy_station_selected_recipe = 0;
                                                                        state.ui_state.alchemy_station_scroll_offset = 0.0;
                                                                        state.ui_state.alchemy_station_quantity = 1;
                                                                        state
                                                                            .ui_state
                                                                            .alchemy_station_tab =
                                                                            0;
                                                                    }
                                                                    Some("workbench") => {
                                                                        state
                                                                            .ui_state
                                                                            .fletching_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .alchemy_station_open =
                                                                            false;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_open = true;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_tile =
                                                                            Some((
                                                                                npc.x.round()
                                                                                    as i32,
                                                                                npc.y.round()
                                                                                    as i32,
                                                                            ));
                                                                        state.ui_state.workbench_selected_recipe = 0;
                                                                        state.ui_state.workbench_scroll_offset = 0.0;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_quantity = 1;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_tab = 0;
                                                                    }
                                                                    _ => {
                                                                        _commands.push(InputCommand::Interact { npc_id: npc_id.to_string() });
                                                                    }
                                                                }
                                                            }
                                                        },
                                                    );
                                                }
                                                1 => {
                                                    let msg = npc_name;
                                                    state.push_system_chat(msg);
                                                }
                                                _ => {}
                                            }
                                        } else if is_merchant {
                                            // Options: 0=Talk-to, 1=Trade, 2=Examine
                                            match option_idx {
                                                0 | 1 => {
                                                    // Both Talk-to and Trade interact with merchant
                                                    pathfind_and_interact_npc(
                                                        state,
                                                        &mut commands,
                                                        &npc_id,
                                                        |_state, commands, npc_id| {
                                                            commands.push(InputCommand::Interact {
                                                                npc_id: npc_id.to_string(),
                                                            });
                                                        },
                                                    );
                                                }
                                                2 => {
                                                    let msg = npc_name;
                                                    state.push_system_chat(msg);
                                                }
                                                _ => {}
                                            }
                                        } else if is_banker {
                                            // Options: 0=Bank, 1=Examine
                                            match option_idx {
                                                0 => {
                                                    pathfind_and_interact_npc(
                                                        state,
                                                        &mut commands,
                                                        &npc_id,
                                                        |_state, commands, npc_id| {
                                                            commands.push(InputCommand::Interact {
                                                                npc_id: npc_id.to_string(),
                                                            });
                                                        },
                                                    );
                                                }
                                                1 => {
                                                    let msg = npc_name;
                                                    state.push_system_chat(msg);
                                                }
                                                _ => {}
                                            }
                                        } else if is_slayer_master {
                                            // Options: 0=Get Task, 1=Examine
                                            match option_idx {
                                                0 => {
                                                    // Check requirements client-side for instant feedback
                                                    let (combat_req, slayer_req) =
                                                        slayer_master_requirements(
                                                            &npc_entity_type,
                                                        );
                                                    let player_combat = state
                                                        .get_local_player()
                                                        .map(|p| p.combat_level())
                                                        .unwrap_or(0);
                                                    let player_slayer = state
                                                        .get_local_player()
                                                        .map(|p| p.skills.slayer.level)
                                                        .unwrap_or(1);

                                                    if player_combat < combat_req {
                                                        state.push_system_chat(format!(
                                                            "You need combat level {} to get tasks from {}. (You are level {})",
                                                            combat_req, npc_name, player_combat
                                                        ));
                                                    } else if player_slayer < slayer_req {
                                                        state.push_system_chat(format!(
                                                            "You need slayer level {} to get tasks from {}. (You are level {})",
                                                            slayer_req, npc_name, player_slayer
                                                        ));
                                                    } else {
                                                        pathfind_and_interact_npc(
                                                            state,
                                                            &mut commands,
                                                            &npc_id,
                                                            |_state, commands, npc_id| {
                                                                commands.push(
                                                                    InputCommand::SlayerGetTask {
                                                                        master_id: npc_id
                                                                            .to_string(),
                                                                    },
                                                                );
                                                            },
                                                        );
                                                    }
                                                }
                                                1 => {
                                                    let (combat_req, slayer_req) =
                                                        slayer_master_requirements(
                                                            &npc_entity_type,
                                                        );
                                                    if combat_req > 0 || slayer_req > 1 {
                                                        state.push_system_chat(format!(
                                                            "{} - Requires combat level {}, slayer level {}.",
                                                            npc_name, combat_req, slayer_req
                                                        ));
                                                    } else {
                                                        state.push_system_chat(format!(
                                                            "{} - Beginner slayer master.",
                                                            npc_name
                                                        ));
                                                    }
                                                }
                                                _ => {}
                                            }
                                        } else {
                                            // Generic friendly NPC: 0=Talk-to, 1=Examine
                                            match option_idx {
                                                0 => {
                                                    pathfind_and_interact_npc(
                                                        state,
                                                        &mut commands,
                                                        &npc_id,
                                                        |_state, commands, npc_id| {
                                                            commands.push(InputCommand::Interact {
                                                                npc_id: npc_id.to_string(),
                                                            });
                                                        },
                                                    );
                                                }
                                                1 => {
                                                    state.push_system_chat(npc_name);
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                                ContextMenuTarget::Tree {
                                    tile_x,
                                    tile_y,
                                    gid,
                                } => {
                                    // Options: 0=Chop, 1=Examine
                                    match option_idx {
                                        0 => {
                                            let target_id =
                                                format!("{},{},{}", tile_x, tile_y, gid);
                                            state.auto_action_state =
                                                Some(crate::game::AutoActionState {
                                                    target_type: "resource".to_string(),
                                                    target_id: target_id.clone(),
                                                    action: "chop".to_string(),
                                                    confirmed: false,
                                                });
                                            pathfind_and_resource(
                                                state,
                                                &mut commands,
                                                *tile_x,
                                                *tile_y,
                                                &target_id,
                                                "chop",
                                            );
                                        }
                                        1 => {
                                            let name = crate::game::tree_types::get_tree_info(*gid)
                                                .map(|info| info.name)
                                                .unwrap_or("Tree");
                                            state.push_system_chat(format!("{} tree.", name));
                                        }
                                        _ => {}
                                    }
                                }
                                ContextMenuTarget::Rock {
                                    tile_x,
                                    tile_y,
                                    gid,
                                } => {
                                    // Options: 0=Mine, 1=Examine
                                    match option_idx {
                                        0 => {
                                            let target_id =
                                                format!("{},{},{}", tile_x, tile_y, gid);
                                            state.auto_action_state =
                                                Some(crate::game::AutoActionState {
                                                    target_type: "resource".to_string(),
                                                    target_id: target_id.clone(),
                                                    action: "mine".to_string(),
                                                    confirmed: false,
                                                });
                                            pathfind_and_resource(
                                                state,
                                                &mut commands,
                                                *tile_x,
                                                *tile_y,
                                                &target_id,
                                                "mine",
                                            );
                                        }
                                        1 => {
                                            let name = crate::game::ore_types::get_ore_info(*gid)
                                                .map(|info| info.name)
                                                .unwrap_or("Rock");
                                            state.push_system_chat(format!(
                                                "A rock containing {} ore.",
                                                name.to_lowercase()
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                                ContextMenuTarget::MapObject {
                                    tile_x,
                                    tile_y,
                                    gid,
                                } => {
                                    // Obelisks: 0=Teleport, 1=Examine
                                    // Chests: 0=Open, 1=Examine
                                    // Other objects: 0=Interact, 1=Examine
                                    let tx = *tile_x;
                                    let ty = *tile_y;
                                    let is_chest = state.chest_positions.contains(&(tx, ty));
                                    match option_idx {
                                        0 => {
                                            if is_obelisk_gid(*gid) {
                                                // Walk to obelisk, then teleport directly
                                                if let Some(player) = state.get_local_player() {
                                                    let px = player.server_x.round() as i32;
                                                    let py = player.server_y.round() as i32;
                                                    let cdx = (px - tx).abs();
                                                    let cdy = (py - ty).abs();
                                                    if cdx <= 1 && cdy <= 1 {
                                                        commands.push(InputCommand::UseWaystone {
                                                            x: tx,
                                                            y: ty,
                                                        });
                                                    } else {
                                                        let occupied =
                                                            build_occupied_set(state, true);
                                                        const MAX_PATH_DISTANCE: i32 = 32;
                                                        if let Some((dest, path)) =
                                                            pathfinding::find_path_to_adjacent(
                                                                (px, py),
                                                                (tx, ty),
                                                                &state.chunk_manager,
                                                                &occupied,
                                                                MAX_PATH_DISTANCE,
                                                            )
                                                        {
                                                            state.auto_path = Some(PathState {
                                                                path,
                                                                current_index: 0,
                                                                destination: dest,
                                                                pickup_target: None,
                                                                interact_target: None,
                                                                interact_object_target: None,
                                                                waystone_target: Some((tx, ty)),
                                                                browse_stall_target: None,
                                                            });
                                                        }
                                                    }
                                                }
                                            } else if is_chest {
                                                // Walk to chest and open it
                                                if let Some(player) = state.get_local_player() {
                                                    let px = player.server_x.round() as i32;
                                                    let py = player.server_y.round() as i32;
                                                    let cdx = (px - tx).abs();
                                                    let cdy = (py - ty).abs();
                                                    if cdx <= 1 && cdy <= 1 {
                                                        commands.push(
                                                            InputCommand::InteractObject {
                                                                x: tx,
                                                                y: ty,
                                                            },
                                                        );
                                                    } else {
                                                        let occupied =
                                                            build_occupied_set(state, true);
                                                        const MAX_PATH_DISTANCE: i32 = 32;
                                                        if let Some((dest, path)) =
                                                            pathfinding::find_path_to_adjacent(
                                                                (px, py),
                                                                (tx, ty),
                                                                &state.chunk_manager,
                                                                &occupied,
                                                                MAX_PATH_DISTANCE,
                                                            )
                                                        {
                                                            state.auto_path = Some(PathState {
                                                                path,
                                                                current_index: 0,
                                                                destination: dest,
                                                                pickup_target: None,
                                                                interact_target: None,
                                                                interact_object_target: Some((
                                                                    tx, ty,
                                                                )),
                                                                waystone_target: None,
                                                                browse_stall_target: None,
                                                            });
                                                        }
                                                    }
                                                }
                                            } else {
                                                commands.push(InputCommand::InteractObject {
                                                    x: tx,
                                                    y: ty,
                                                });
                                            }
                                        }
                                        1 => {
                                            if is_chest {
                                                state.push_system_chat(
                                                    "A wooden storage chest.".to_string(),
                                                );
                                            } else if is_obelisk_gid(*gid) {
                                                state.push_system_chat("An ancient obelisk humming with magical energy.".to_string());
                                            } else {
                                                match get_map_object_name(*gid) {
                                                    Some(name) => state.push_system_chat(format!(
                                                        "A {}.",
                                                        name.to_lowercase()
                                                    )),
                                                    None => state.push_system_chat(
                                                        "Nothing interesting.".to_string(),
                                                    ),
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                ContextMenuTarget::GatheringSpot { marker_index } => {
                                    // Options: 0=Fish/Gather, 1=Examine
                                    match option_idx {
                                        0 => {
                                            if let Some(marker) =
                                                state.gathering_markers.get(*marker_index)
                                            {
                                                let marker_x = marker.x;
                                                let marker_y = marker.y;
                                                // Pathfind to marker and start gathering
                                                if let Some(player) = state.get_local_player() {
                                                    let px = player.server_x.round() as i32;
                                                    let py = player.server_y.round() as i32;
                                                    let dx = (px - marker_x).abs();
                                                    let dy = (py - marker_y).abs();
                                                    if dx <= 1 && dy <= 1 {
                                                        commands.push(
                                                            InputCommand::StartGathering {
                                                                marker_x,
                                                                marker_y,
                                                            },
                                                        );
                                                    } else {
                                                        let occupied =
                                                            build_occupied_set(state, true);
                                                        const MAX_PATH_DISTANCE: i32 = 32;
                                                        if let Some((dest, path)) =
                                                            pathfinding::find_path_to_adjacent(
                                                                (px, py),
                                                                (marker_x, marker_y),
                                                                &state.chunk_manager,
                                                                &occupied,
                                                                MAX_PATH_DISTANCE,
                                                            )
                                                        {
                                                            state.auto_path = Some(PathState {
                                                                path,
                                                                current_index: 0,
                                                                destination: dest,
                                                                pickup_target: None,
                                                                interact_target: None,
                                                                interact_object_target: None,
                                                                waystone_target: None,
                                                                browse_stall_target: None,
                                                            });
                                                            // Player will need to interact again when they arrive
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        1 => {
                                            if let Some(marker) =
                                                state.gathering_markers.get(*marker_index)
                                            {
                                                state.push_system_chat(format!(
                                                    "A {} spot.",
                                                    marker.skill
                                                ));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                ContextMenuTarget::GroundItem { id } => {
                                    // Options: 0=Pick up, 1=Examine
                                    match option_idx {
                                        0 => {
                                            if let Some(item) = state.ground_items.get(id) {
                                                let item_x = item.x.round() as i32;
                                                let item_y = item.y.round() as i32;
                                                let item_id = item.id.clone();
                                                const PICKUP_RANGE: f32 = 2.0;
                                                if let Some(player) = state.get_local_player() {
                                                    let dx = item.x - player.x;
                                                    let dy = item.y - player.y;
                                                    let dist = (dx * dx + dy * dy).sqrt();
                                                    if dist < PICKUP_RANGE {
                                                        commands
                                                            .push(InputCommand::Pickup { item_id });
                                                    } else {
                                                        // Pathfind to item
                                                        let px = player.server_x.round() as i32;
                                                        let py = player.server_y.round() as i32;
                                                        let occupied =
                                                            build_occupied_set(state, true);
                                                        const MAX_PATH_DISTANCE: i32 = 32;
                                                        if let Some(path) =
                                                            find_path_with_optimistic_splice(
                                                                state,
                                                                (px, py),
                                                                (item_x, item_y),
                                                                &occupied,
                                                                MAX_PATH_DISTANCE,
                                                            )
                                                        {
                                                            state.auto_path = Some(PathState {
                                                                path,
                                                                current_index: 0,
                                                                destination: (item_x, item_y),
                                                                pickup_target: Some(item_id),
                                                                interact_target: None,
                                                                interact_object_target: None,
                                                                waystone_target: None,
                                                                browse_stall_target: None,
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        1 => {
                                            if let Some(item) = state.ground_items.get(id) {
                                                let item_def = state
                                                    .item_registry
                                                    .get_or_placeholder(&item.item_id);
                                                state.push_system_chat(format!(
                                                    "{}: {}",
                                                    item_def.display_name, item_def.description
                                                ));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                ContextMenuTarget::FarmingPatch { patch_id } => {
                                    if let Some(patch) = state.farming_patches.get(patch_id) {
                                        let patch_state = patch.state.clone();
                                        let patch_x = patch.x;
                                        let patch_y = patch.y;
                                        let pid = patch_id.clone();
                                        if patch_state == "harvestable" {
                                            // Options: 0=Harvest, 1=Examine
                                            match option_idx {
                                                0 => {
                                                    if let Some(player) = state.get_local_player() {
                                                        let px = player.server_x.round() as i32;
                                                        let py = player.server_y.round() as i32;
                                                        let cdx = (px - patch_x).abs();
                                                        let cdy = (py - patch_y).abs();
                                                        if cdx <= 1 && cdy <= 1 {
                                                            commands.push(
                                                                InputCommand::HarvestCrop {
                                                                    patch_id: pid,
                                                                },
                                                            );
                                                        } else {
                                                            let occupied =
                                                                build_occupied_set(state, true);
                                                            const MAX_PATH_DISTANCE: i32 = 32;
                                                            if let Some((dest, path)) =
                                                                pathfinding::find_path_to_adjacent(
                                                                    (px, py),
                                                                    (patch_x, patch_y),
                                                                    &state.chunk_manager,
                                                                    &occupied,
                                                                    MAX_PATH_DISTANCE,
                                                                )
                                                            {
                                                                state.auto_path = Some(PathState {
                                                                    path,
                                                                    current_index: 0,
                                                                    destination: dest,
                                                                    pickup_target: None,
                                                                    interact_target: None,
                                                                    interact_object_target: None,
                                                                    waystone_target: None,
                                                                    browse_stall_target: None,
                                                                });
                                                                state.pending_harvest_patch =
                                                                    Some(pid);
                                                            }
                                                        }
                                                    }
                                                }
                                                1 => {
                                                    state.push_system_chat(
                                                        "This crop is ready to harvest."
                                                            .to_string(),
                                                    );
                                                }
                                                _ => {}
                                            }
                                        } else if patch_state == "empty" {
                                            // Options: 0=Plant, 1=Examine
                                            match option_idx {
                                                0 => {
                                                    // TODO: Open seed selection UI or plant first seed
                                                    state.push_system_chat(
                                                        "Use a seed on this patch to plant it."
                                                            .to_string(),
                                                    );
                                                }
                                                1 => {
                                                    state.push_system_chat(
                                                        "An empty farming patch.".to_string(),
                                                    );
                                                }
                                                _ => {}
                                            }
                                        } else {
                                            // Growing state - only Examine
                                            if *option_idx == 0 {
                                                state.push_system_chat(
                                                    "A farming patch with something growing."
                                                        .to_string(),
                                                );
                                            }
                                        }
                                    }
                                }
                                ContextMenuTarget::Tile { x, y } => {
                                    // Options: 0=Walk here
                                    if *option_idx == 0 {
                                        pathfind_to_tile(state, &mut commands, *x, *y);
                                    }
                                }
                                ContextMenuTarget::HotkeySlot(slot_idx) => {
                                    // Options: 0=Clear Slot
                                    if *option_idx == 0 {
                                        state.ui_state.hotkey_bar.active_mut().slots[*slot_idx] =
                                            crate::game::hotkey::HotkeySlotBinding::Empty;
                                        save_current_ui_settings(state);
                                    }
                                }
                            }
                            return commands;
                        }
                        _ => {
                            // Clicked somewhere else, close menu
                            state.ui_state.context_menu = None;
                        }
                    }
                }
            } else if mouse_clicked || mouse_right_clicked {
                // Clicked outside any element, close menu
                state.ui_state.context_menu = None;
            }

            // Escape closes context menu
            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.context_menu = None;
                return commands;
            }
        }

        // Handle menu button clicks (always visible, handle before modal UIs)
        if mouse_clicked {
            if let Some(ref element) = clicked_element {
                match element {
                    UiElementId::MenuButtonInventory => {
                        audio.play_sfx("enter");
                        // Toggle inventory panel, close others if opening
                        if state.ui_state.inventory_open {
                            state.ui_state.inventory_open = false;
                        } else {
                            state.ui_state.inventory_open = true;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.prayer_book_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.escape_menu_open = false;
                            state.ui_state.close_quest_log();
                        }
                        return commands;
                    }
                    UiElementId::MenuButtonCharacter => {
                        audio.play_sfx("enter");
                        // Toggle character panel, close others if opening
                        if state.ui_state.character_panel_open {
                            state.ui_state.character_panel_open = false;
                        } else {
                            state.ui_state.character_panel_open = true;
                            state.ui_state.inventory_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.prayer_book_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.escape_menu_open = false;
                            state.ui_state.close_quest_log();
                        }
                        return commands;
                    }
                    UiElementId::MenuButtonSocial => {
                        audio.play_sfx("enter");
                        // Toggle social panel, close others if opening
                        if state.ui_state.social_open {
                            state.ui_state.social_open = false;
                            state.social_state.add_friend_focused = false;
                        } else {
                            state.ui_state.social_open = true;
                            state.ui_state.inventory_open = false;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.prayer_book_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.escape_menu_open = false;
                            state.ui_state.close_quest_log();
                            // Request online players list when opening panel
                            commands.push(InputCommand::GetOnlinePlayers);
                        }
                        return commands;
                    }
                    UiElementId::MenuButtonSkills => {
                        audio.play_sfx("enter");
                        // Toggle skills panel, close others if opening
                        if state.ui_state.skills_open {
                            state.ui_state.skills_open = false;
                        } else {
                            state.ui_state.skills_open = true;
                            state.ui_state.inventory_open = false;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.prayer_book_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.escape_menu_open = false;
                            state.ui_state.close_quest_log();
                        }
                        return commands;
                    }
                    UiElementId::MenuButtonPrayer => {
                        audio.play_sfx("enter");
                        // Toggle prayer book, close others if opening
                        if state.ui_state.prayer_book_open {
                            state.ui_state.prayer_book_open = false;
                        } else {
                            state.ui_state.prayer_book_open = true;
                            state.ui_state.inventory_open = false;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.escape_menu_open = false;
                            state.ui_state.close_quest_log();
                        }
                        return commands;
                    }
                    UiElementId::MenuButtonQuest => {
                        audio.play_sfx("enter");
                        // Toggle quest log, close others if opening
                        if state.ui_state.quest_log_open {
                            state.ui_state.close_quest_log();
                        } else {
                            state.ui_state.quest_log_open = true;
                            state.ui_state.quest_log_scroll = 0.0;
                            state.ui_state.selected_quest_id = None;
                            state.ui_state.inventory_open = false;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.prayer_book_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.escape_menu_open = false;
                        }
                        return commands;
                    }
                    UiElementId::MenuButtonSettings => {
                        audio.play_sfx("enter");
                        // Toggle settings panel, close others if opening
                        if state.ui_state.escape_menu_open {
                            state.ui_state.escape_menu_open = false;
                        } else {
                            state.ui_state.escape_menu_open = true;
                            state.ui_state.inventory_open = false;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.prayer_book_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.close_quest_log();
                        }
                        return commands;
                    }
                    UiElementId::ChatButton => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_panel_open = !state.ui_state.chat_panel_open;
                        if state.ui_state.chat_panel_open {
                            state.ui_state.chat_active_tab = ChatChannel::Local;
                            mark_chat_channel_as_read(state, ChatChannel::Local);
                            state.ui_state.chat_message_scroll = 0.0;
                            // Close other panels
                            state.ui_state.inventory_open = false;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.prayer_book_open = false;
                            state.ui_state.minimap_panel_open = false;
                            state.ui_state.close_quest_log();
                        }
                    }
                    UiElementId::MinimapToggle => {
                        audio.play_sfx("enter");
                        state.ui_state.minimap_panel_open = !state.ui_state.minimap_panel_open;
                        if state.ui_state.minimap_panel_open {
                            state.ui_state.inventory_open = false;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.prayer_book_open = false;
                            state.ui_state.close_quest_log();
                            state.ui_state.chat_panel_open = false;
                            state.ui_state.chat_open = false;
                            state.ui_state.minimap_panel_zoom = 1.0;
                            state.ui_state.minimap_panel_center_x = None;
                            state.ui_state.minimap_panel_center_y = None;
                            state.ui_state.minimap_panel_dragging = false;
                        }
                        return commands;
                    }
                    UiElementId::MinimapClose => {
                        audio.play_sfx("enter");
                        state.ui_state.minimap_panel_open = false;
                        state.ui_state.minimap_panel_dragging = false;
                        return commands;
                    }
                    UiElementId::MinimapPanel | UiElementId::MinimapMarker(_) => {
                        // Handled by dedicated minimap modal logic below.
                    }
                    UiElementId::ChatTabLocal => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_active_tab = ChatChannel::Local;
                        mark_chat_channel_as_read(state, ChatChannel::Local);
                        state.ui_state.chat_message_scroll = 0.0;
                    }
                    UiElementId::ChatTabGlobal => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_active_tab = ChatChannel::Global;
                        mark_chat_channel_as_read(state, ChatChannel::Global);
                        state.ui_state.chat_message_scroll = 0.0;
                    }
                    UiElementId::ChatTabSystem => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_active_tab = ChatChannel::System;
                        mark_chat_channel_as_read(state, ChatChannel::System);
                        state.ui_state.chat_message_scroll = 0.0;
                    }
                    UiElementId::ChatSendButton => {
                        let text = state.ui_state.chat_input.trim().to_string();
                        // Determine channel: ~ prefix forces global, otherwise match active tab
                        // System tab sends to public channel
                        let (send_text, channel) = if text.starts_with('~') {
                            let trimmed = text[1..].trim().to_string();
                            (trimmed, "global".to_string())
                        } else {
                            let ch = match state.ui_state.chat_active_tab {
                                ChatChannel::Global => "global",
                                _ => "public",
                            };
                            (text.clone(), ch.to_string())
                        };
                        if !send_text.is_empty() {
                            audio.play_sfx("send_message");
                            commands.push(InputCommand::Chat {
                                text: send_text,
                                channel,
                            });
                        }
                        state.ui_state.chat_input.clear();
                        state.ui_state.chat_cursor = 0;
                    }
                    UiElementId::ChatInputField => {
                        state.ui_state.chat_open = true;
                        #[cfg(target_os = "android")]
                        macroquad::miniquad::window::show_keyboard(true);
                    }
                    UiElementId::ChatCloseButton => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_panel_open = false;
                        state.ui_state.chat_open = false;
                        #[cfg(target_os = "android")]
                        macroquad::miniquad::window::show_keyboard(false);
                    }
                    UiElementId::ChatPanelBackground => {
                        // Tapping outside the panel content closes the chat panel
                        state.ui_state.chat_panel_open = false;
                        state.ui_state.chat_open = false;
                        #[cfg(target_os = "android")]
                        macroquad::miniquad::window::show_keyboard(false);
                    }
                    // Social panel scroll area - handle touch scrolling
                    UiElementId::SocialScrollArea => {
                        // Touch scroll handled below, just suppress click
                    }
                    // Social panel handlers
                    UiElementId::SocialTabNearby => {
                        audio.play_sfx("enter");
                        state.social_state.active_tab = crate::game::SocialTab::Nearby;
                    }
                    UiElementId::SocialTabOnline => {
                        audio.play_sfx("enter");
                        state.social_state.active_tab = crate::game::SocialTab::Online;
                        // Request online players list
                        commands.push(InputCommand::GetOnlinePlayers);
                    }
                    UiElementId::SocialTabFriends => {
                        audio.play_sfx("enter");
                        state.social_state.active_tab = crate::game::SocialTab::Friends;
                    }
                    UiElementId::SocialPlayerRow(idx) => {
                        // Send friend request to this player (from nearby or online list)
                        audio.play_sfx("enter");
                        let player_name = match state.social_state.active_tab {
                            crate::game::SocialTab::Nearby => {
                                // Get player from nearby list (state.players minus local player)
                                let local_id = state.local_player_id.as_ref();
                                let nearby: Vec<_> = state
                                    .players
                                    .values()
                                    .filter(|p| Some(&p.id) != local_id)
                                    .collect();
                                nearby.get(*idx).map(|p| p.name.clone())
                            }
                            crate::game::SocialTab::Online => state
                                .social_state
                                .online_players
                                .get(*idx)
                                .map(|p| p.name.clone()),
                            _ => None,
                        };
                        if let Some(name) = player_name {
                            commands.push(InputCommand::SendFriendRequest { target_name: name });
                        }
                    }
                    UiElementId::SocialRequestAccept(idx) => {
                        audio.play_sfx("enter");
                        if let Some(request) =
                            state.social_state.pending_requests.get(*idx).cloned()
                        {
                            let requester_id = request.from_id;
                            let requester_name = request.from_name.clone();
                            commands.push(InputCommand::AcceptFriendRequest { requester_id });
                            // Remove from pending list immediately for responsive UI
                            state.social_state.pending_requests.remove(*idx);
                            state.social_state.pending_request_count =
                                state.social_state.pending_requests.len();
                            // Also add to friends list immediately (they're online since they sent the request)
                            if !state
                                .social_state
                                .friends
                                .iter()
                                .any(|f| f.id == requester_id)
                            {
                                state.social_state.friends.push(crate::game::FriendInfo {
                                    id: requester_id,
                                    name: requester_name,
                                    online: true,
                                });
                                // Sort friends list (online first)
                                state.social_state.friends.sort_by(|a, b| {
                                    match (a.online, b.online) {
                                        (true, false) => std::cmp::Ordering::Less,
                                        (false, true) => std::cmp::Ordering::Greater,
                                        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                                    }
                                });
                            }
                        }
                    }
                    UiElementId::SocialRequestDecline(idx) => {
                        audio.play_sfx("enter");
                        if let Some(request) = state.social_state.pending_requests.get(*idx) {
                            let requester_id = request.from_id;
                            commands.push(InputCommand::DeclineFriendRequest { requester_id });
                            // Remove from local list immediately
                            state.social_state.pending_requests.remove(*idx);
                            state.social_state.pending_request_count =
                                state.social_state.pending_requests.len();
                        }
                    }
                    UiElementId::SocialRemoveFriend(idx) => {
                        audio.play_sfx("enter");
                        if let Some(friend) = state.social_state.friends.get(*idx) {
                            let friend_id = friend.id;
                            commands.push(InputCommand::RemoveFriend { friend_id });
                            // Remove from local list immediately
                            state.social_state.friends.remove(*idx);
                        }
                    }
                    UiElementId::SocialAddFriendButton => {
                        // Send friend request by name
                        let name = state.social_state.add_friend_input.trim().to_string();
                        if !name.is_empty() {
                            audio.play_sfx("enter");
                            commands.push(InputCommand::SendFriendRequest { target_name: name });
                            state.social_state.add_friend_input.clear();
                            state.social_state.add_friend_focused = false;
                            #[cfg(target_os = "android")]
                            macroquad::miniquad::window::show_keyboard(false);
                        }
                    }
                    UiElementId::SocialAddFriendInput => {
                        // Focus the input for typing
                        state.social_state.add_friend_focused = true;
                        #[cfg(target_os = "android")]
                        macroquad::miniquad::window::show_keyboard(true);
                    }
                    // Skills panel - clicking Prayer skill opens prayer book
                    UiElementId::SkillSlot(5) => {
                        // Index 5 is Prayer skill - open prayer book on Prayers tab
                        audio.play_sfx("enter");
                        state.ui_state.prayer_book_open = !state.ui_state.prayer_book_open;
                        if state.ui_state.prayer_book_open {
                            state.ui_state.prayer_spell_tab = 0; // Open to prayers tab
                            state.ui_state.skills_open = false;
                        }
                    }
                    UiElementId::SkillSlot(6) => {
                        // Index 6 is Magic skill - open prayer/spell panel on Spells tab
                        audio.play_sfx("enter");
                        state.ui_state.prayer_book_open = !state.ui_state.prayer_book_open;
                        if state.ui_state.prayer_book_open {
                            state.ui_state.prayer_spell_tab = 1; // Open to spells tab
                            state.ui_state.skills_open = false;
                        }
                    }
                    // Prayer/Spell help buttons
                    UiElementId::PrayerHelpButton => {
                        audio.play_sfx("enter");
                        state.ui_state.prayer_help_open = true;
                    }
                    UiElementId::SpellHelpButton => {
                        audio.play_sfx("enter");
                        state.ui_state.spell_help_open = true;
                    }
                    UiElementId::PrayerHelpClose => {
                        audio.play_sfx("enter");
                        state.ui_state.prayer_help_open = false;
                    }
                    UiElementId::SpellHelpClose => {
                        audio.play_sfx("enter");
                        state.ui_state.spell_help_open = false;
                    }
                    // Prayer/Spell tab switching
                    UiElementId::PrayerSpellTab(tab_idx) => {
                        audio.play_sfx("enter");
                        state.ui_state.prayer_spell_tab = *tab_idx;
                        state.ui_state.prayer_help_open = false;
                        state.ui_state.spell_help_open = false;
                    }
                    // Spell slot handlers (spell panel — click to assign)
                    UiElementId::SpellSlot(_slot_idx) => {
                        audio.play_sfx("enter");
                    }
                    // Hotkey bar preset cycling and settings
                    UiElementId::HotkeyPresetUp => {
                        audio.play_sfx("enter");
                        state.ui_state.hotkey_bar.cycle_up();
                        save_current_ui_settings(state);
                    }
                    UiElementId::HotkeyPresetDown => {
                        audio.play_sfx("enter");
                        state.ui_state.hotkey_bar.cycle_down();
                        save_current_ui_settings(state);
                    }
                    UiElementId::HotkeySettingsCog => {
                        audio.play_sfx("enter");
                        state.ui_state.hotkey_settings_open = !state.ui_state.hotkey_settings_open;
                    }
                    UiElementId::HotkeySettingsPresetTab(tab_idx) => {
                        audio.play_sfx("enter");
                        state.ui_state.hotkey_bar.active_preset = *tab_idx;
                        save_current_ui_settings(state);
                    }
                    UiElementId::HotkeySettingsSlotClear(slot_idx) => {
                        audio.play_sfx("enter");
                        state.ui_state.hotkey_bar.active_mut().slots[*slot_idx] =
                            crate::game::hotkey::HotkeySlotBinding::Empty;
                        save_current_ui_settings(state);
                    }
                    // Prayer panel handlers
                    UiElementId::PrayerSlot(slot_idx) => {
                        // Toggle prayer at this slot
                        if *slot_idx < crate::game::prayer::PRAYERS.len() {
                            let prayer = &crate::game::prayer::PRAYERS[*slot_idx];
                            let prayer_level = state
                                .get_local_player()
                                .map(|p| p.skills.prayer.level)
                                .unwrap_or(1);

                            // Check if player meets level requirement
                            if prayer_level >= prayer.level_req {
                                // Check if we have prayer points (can only activate if we have points)
                                let is_active =
                                    state.active_prayers.contains(&prayer.id.to_string());
                                if is_active || state.prayer_points > 0 {
                                    audio.play_sfx("enter");
                                    commands.push(InputCommand::TogglePrayer {
                                        prayer_id: prayer.id.to_string(),
                                    });
                                } else {
                                    // No prayer points, play error sound
                                    audio.play_sfx("error");
                                }
                            } else {
                                // Level too low, play error sound
                                audio.play_sfx("error");
                            }
                        }
                    }
                    UiElementId::QuestLogEntry(idx) => {
                        audio.play_sfx("enter");
                        // Rebuild sorted quest list matching render_quest_log order
                        let mut sorted: Vec<&QuestCatalogEntry> =
                            state.ui_state.quest_catalog.iter().collect();
                        sorted.sort_by(|a, b| {
                            let sa = quest_status_order(&a.quest_id, &state.ui_state);
                            let sb = quest_status_order(&b.quest_id, &state.ui_state);
                            sa.cmp(&sb).then(a.name.cmp(&b.name))
                        });
                        if let Some(entry) = sorted.get(*idx) {
                            state.ui_state.selected_quest_id = Some(entry.quest_id.clone());
                            state.ui_state.quest_log_scroll = 0.0;
                        }
                    }
                    UiElementId::QuestDetailBack => {
                        audio.play_sfx("enter");
                        state.ui_state.selected_quest_id = None;
                        state.ui_state.quest_log_scroll = 0.0;
                    }
                    _ => {
                        // Clicking elsewhere unfocuses the add friend input
                        if state.social_state.add_friend_focused {
                            state.social_state.add_friend_focused = false;
                            #[cfg(target_os = "android")]
                            macroquad::miniquad::window::show_keyboard(false);
                        }
                    }
                }
            }
        }

        // Handle escape menu
        if state.ui_state.escape_menu_open {
            // Handle slider dragging - continue updating while mouse is held
            if state.ui_state.settings_slider_dragging.is_some() {
                if is_mouse_button_down(MouseButton::Left) {
                    let (mouse_x, _) = mouse_position();
                    match state.ui_state.settings_slider_dragging {
                        Some(UiElementId::EscapeMenuMusicSlider) => {
                            if let Some(slider_elem) = layout
                                .elements
                                .iter()
                                .find(|e| e.id == UiElementId::EscapeMenuMusicSlider)
                            {
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let volume = (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.audio_volume = volume;
                                audio.set_music_volume(volume);
                            }
                        }
                        Some(UiElementId::EscapeMenuSfxSlider) => {
                            if let Some(slider_elem) = layout
                                .elements
                                .iter()
                                .find(|e| e.id == UiElementId::EscapeMenuSfxSlider)
                            {
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let volume = (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.audio_sfx_volume = volume;
                                audio.set_sfx_volume(volume);
                            }
                        }
                        Some(UiElementId::EscapeMenuUiScaleSlider) => {
                            if let Some(slider_elem) = layout
                                .elements
                                .iter()
                                .find(|e| e.id == UiElementId::EscapeMenuUiScaleSlider)
                            {
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let normalized =
                                    (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.ui_scale = 0.75 + normalized * 1.25;
                            }
                        }
                        _ => {}
                    }
                    return commands;
                } else {
                    // Mouse released - stop dragging and save settings
                    save_current_ui_settings(state);
                    state.ui_state.settings_slider_dragging = None;
                }
            }

            // Handle mouse clicks on escape menu elements
            if let Some(ref element) = clicked_element {
                if mouse_clicked {
                    match element {
                        UiElementId::EscapeMenuZoom05x => {
                            audio.play_sfx("enter");
                            state.camera.zoom = 0.5;
                            save_current_ui_settings(state);
                            return commands;
                        }
                        UiElementId::EscapeMenuZoom1x => {
                            audio.play_sfx("enter");
                            state.camera.zoom = 1.0;
                            save_current_ui_settings(state);
                            return commands;
                        }
                        UiElementId::EscapeMenuZoom2x => {
                            audio.play_sfx("enter");
                            state.camera.zoom = 2.0;
                            save_current_ui_settings(state);
                            return commands;
                        }
                        UiElementId::EscapeMenuMusicSlider => {
                            // Start dragging and set initial value
                            state.ui_state.settings_slider_dragging =
                                Some(UiElementId::EscapeMenuMusicSlider);
                            if let Some(slider_elem) = layout
                                .elements
                                .iter()
                                .find(|e| e.id == UiElementId::EscapeMenuMusicSlider)
                            {
                                let (mouse_x, _) = mouse_position();
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let volume = (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.audio_volume = volume;
                                audio.set_music_volume(volume);
                            }
                            return commands;
                        }
                        UiElementId::EscapeMenuSfxSlider => {
                            // Start dragging and set initial value
                            state.ui_state.settings_slider_dragging =
                                Some(UiElementId::EscapeMenuSfxSlider);
                            if let Some(slider_elem) = layout
                                .elements
                                .iter()
                                .find(|e| e.id == UiElementId::EscapeMenuSfxSlider)
                            {
                                let (mouse_x, _) = mouse_position();
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let volume = (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.audio_sfx_volume = volume;
                                audio.set_sfx_volume(volume);
                            }
                            return commands;
                        }
                        UiElementId::EscapeMenuUiScaleSlider => {
                            // Start dragging and set initial value
                            state.ui_state.settings_slider_dragging =
                                Some(UiElementId::EscapeMenuUiScaleSlider);
                            if let Some(slider_elem) = layout
                                .elements
                                .iter()
                                .find(|e| e.id == UiElementId::EscapeMenuUiScaleSlider)
                            {
                                let (mouse_x, _) = mouse_position();
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let normalized =
                                    (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.ui_scale = 0.75 + normalized * 1.25;
                            }
                            return commands;
                        }
                        UiElementId::EscapeMenuMuteToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.audio_muted = !state.ui_state.audio_muted;
                            audio.toggle_mute();
                            return commands;
                        }
                        UiElementId::EscapeMenuShiftDropToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.shift_drop_enabled = !state.ui_state.shift_drop_enabled;
                            save_current_ui_settings(state);
                            return commands;
                        }
                        UiElementId::EscapeMenuChatLogToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.chat_log_visible = !state.ui_state.chat_log_visible;
                            save_current_ui_settings(state);
                            return commands;
                        }
                        UiElementId::EscapeMenuChatBgToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.chat_log_background =
                                !state.ui_state.chat_log_background;
                            save_current_ui_settings(state);
                            return commands;
                        }
                        UiElementId::EscapeMenuTapPathfindToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.tap_to_pathfind = !state.ui_state.tap_to_pathfind;
                            save_current_ui_settings(state);
                            return commands;
                        }
                        UiElementId::EscapeMenuJoystickToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.use_joystick = !state.ui_state.use_joystick;
                            save_current_ui_settings(state);
                            return commands;
                        }
                        UiElementId::EscapeMenuGraphicsToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.graphics_low = !state.ui_state.graphics_low;
                            save_current_ui_settings(state);
                            return commands;
                        }
                        UiElementId::EscapeMenuControlSchemeToggle => {
                            audio.play_sfx("enter");
                            state.ui_state.classic_controls = !state.ui_state.classic_controls;
                            if state.ui_state.classic_controls {
                                state.ui_state.chat_open = true;
                                state.ui_state.chat_cursor =
                                    state.ui_state.chat_input.chars().count();
                            } else {
                                state.ui_state.chat_open = false;
                            }
                            crate::settings::save_classic_controls(state.ui_state.classic_controls);
                            return commands;
                        }
                        UiElementId::EscapeMenuDisconnect => {
                            audio.play_sfx("enter");
                            state.disconnect_requested = true;
                            state.ui_state.escape_menu_open = false;
                            return commands;
                        }
                        _ => {}
                    }
                }
            }

            // Escape closes settings panel
            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.escape_menu_open = false;
                return commands;
            }
        }

        if Self::handle_modal_panels(
            state,
            layout,
            clicked_element.as_ref(),
            mouse_clicked,
            &mut commands,
        ) {
            return commands;
        }

        // Handle altar panel input
        if state.ui_state.altar_panel.is_some() {
            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.altar_panel = None;
                return commands;
            }

            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::AltarOfferAll(idx) => {
                            let altar_npc_id = state
                                .ui_state
                                .altar_panel
                                .as_ref()
                                .unwrap()
                                .altar_npc_id
                                .clone();
                            // Build bone rows to find item_id at index (mirrors renderer logic: dedup by item_id)
                            let mut bone_items: Vec<String> = Vec::new();
                            for slot in state.inventory.slots.iter().flatten() {
                                if !slot.item_id.contains("bones") {
                                    continue;
                                }
                                let item_def =
                                    state.item_registry.get_or_placeholder(&slot.item_id);
                                if item_def.prayer_xp <= 0 {
                                    continue;
                                }
                                if !bone_items.contains(&slot.item_id) {
                                    bone_items.push(slot.item_id.clone());
                                }
                            }
                            if let Some(item_id) = bone_items.get(*idx) {
                                commands.push(InputCommand::OfferAllBones {
                                    item_id: item_id.clone(),
                                    altar_id: altar_npc_id,
                                });
                                audio.play_sfx("item_put");
                                state.ui_state.altar_panel = None;
                            }
                        }
                        UiElementId::AltarPray => {
                            let altar_npc_id = state
                                .ui_state
                                .altar_panel
                                .as_ref()
                                .unwrap()
                                .altar_npc_id
                                .clone();
                            commands.push(InputCommand::PrayAtAltar {
                                altar_id: altar_npc_id,
                            });
                            audio.play_sfx("enter");
                        }
                        UiElementId::AltarClose => {
                            state.ui_state.altar_panel = None;
                            audio.play_sfx("enter");
                        }
                        _ => {
                            // Click outside panel elements - close
                            state.ui_state.altar_panel = None;
                        }
                    }
                } else {
                    // Click with no UI element - close
                    state.ui_state.altar_panel = None;
                }
                return commands;
            }
            return commands;
        }

        // Handle dialogue mode - intercept input when dialogue is open
        if let Some(dialogue) = &state.ui_state.active_dialogue {
            let is_guide_dialogue = is_adventurer_guide_dialogue(&dialogue.speaker);
            let dialogue_has_choices = !dialogue.choices.is_empty();
            let guide_actions_locked = is_guide_dialogue && adventurer_guide_actions_locked(state);
            let guide_selected_active_tier =
                is_guide_dialogue && is_selected_adventurer_guide_tier_active(state);
            let guide_selected_tier_completable =
                is_guide_dialogue && is_selected_adventurer_guide_tier_completable(state);

            // Touch drag scrolling for dialogue choices on mobile
            let all_touches: Vec<Touch> = touches();
            if let Some(tracking_id) = state.ui_state.dialogue_touch_scroll_id {
                if let Some(touch) = all_touches.iter().find(|t| t.id == tracking_id) {
                    match touch.phase {
                        TouchPhase::Moved | TouchPhase::Stationary => {
                            let (_, vy) =
                                screen_to_virtual_coords(touch.position.x, touch.position.y);
                            let dy = state.ui_state.dialogue_touch_last_y - vy;
                            if !state.ui_state.dialogue_touch_dragged {
                                let total_dy = (state.ui_state.dialogue_touch_start_y - vy).abs();
                                if total_dy > 8.0 {
                                    state.ui_state.dialogue_touch_dragged = true;
                                }
                            }
                            if state.ui_state.dialogue_touch_dragged {
                                state.ui_state.dialogue_scroll_offset =
                                    (state.ui_state.dialogue_scroll_offset + dy).max(0.0);
                            }
                            state.ui_state.dialogue_touch_last_y = vy;
                        }
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            state.ui_state.dialogue_touch_scroll_id = None;
                        }
                        _ => {}
                    }
                } else {
                    state.ui_state.dialogue_touch_scroll_id = None;
                }
            } else {
                for touch in &all_touches {
                    if touch.phase == TouchPhase::Started {
                        let (vx, vy) = screen_to_virtual_coords(touch.position.x, touch.position.y);
                        let hit = layout.hit_test(vx, vy);
                        let over_scrollable = matches!(
                            hit,
                            Some(UiElementId::DialogueChoice(_))
                                | Some(UiElementId::DialogueScrollbar)
                        );
                        if over_scrollable {
                            state.ui_state.dialogue_touch_scroll_id = Some(touch.id);
                            state.ui_state.dialogue_touch_last_y = vy;
                            state.ui_state.dialogue_touch_start_y = vy;
                            state.ui_state.dialogue_touch_dragged = false;
                            break;
                        }
                    }
                }
            }

            // Handle mouse scrollbar dragging (generic system)
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::DialogueScrollbar) {
                let choice_spacing: f32 = if cfg!(target_os = "android") {
                    38.0
                } else {
                    32.0
                };
                let total_content = dialogue.choices.len() as f32 * choice_spacing;
                let max_scroll = (total_content - track_bounds.h).max(0.0);
                let clicked_on = matches!(clicked_element, Some(UiElementId::DialogueScrollbar));
                crate::ui::scroll::handle_scrollbar_drag(
                    &mut state.ui_state.dialogue_scroll_drag,
                    &mut state.ui_state.dialogue_scroll_offset,
                    max_scroll,
                    track_bounds,
                    total_content,
                    my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.dialogue_scroll_drag.dragging = false;
            }

            // Handle mouse/touch clicks on dialogue elements
            // Skip if touch was a drag (scroll gesture) or scrollbar interaction
            let was_touch_drag = state.ui_state.dialogue_touch_dragged
                && state.ui_state.dialogue_touch_scroll_id.is_none();
            if was_touch_drag {
                state.ui_state.dialogue_touch_dragged = false;
            }
            let was_scrollbar = state.ui_state.dialogue_scroll_drag.dragging;

            if !was_touch_drag && !was_scrollbar && mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::AdventurerTab(idx) => {
                            state.ui_state.adventurer_selected_tab = *idx;
                            state.ui_state.adventurer_selected_tier = 0;
                            sync_adventurer_guide_dialogue_target(state);
                            return commands;
                        }
                        UiElementId::AdventurerTier(idx) => {
                            state.ui_state.adventurer_selected_tier = *idx;
                            sync_adventurer_guide_dialogue_target(state);
                            if should_auto_open_selected_combat_tier_offer(
                                state,
                                is_guide_dialogue,
                                dialogue_has_choices,
                            ) {
                                if let Some(quest_id) = adventurer_guide_tier_id(
                                    state.ui_state.adventurer_selected_tab,
                                    state.ui_state.adventurer_selected_tier,
                                ) {
                                    commands.push(InputCommand::DialogueChoice {
                                        quest_id: quest_id.to_string(),
                                        choice_id: "__continue__".to_string(),
                                    });
                                }
                            }
                            return commands;
                        }
                        UiElementId::DialogueChoice(idx) => {
                            if guide_actions_locked || guide_selected_active_tier {
                                return commands;
                            }
                            if *idx < dialogue.choices.len() {
                                let choice = &dialogue.choices[*idx];
                                commands.push(InputCommand::DialogueChoice {
                                    quest_id: dialogue.quest_id.clone(),
                                    choice_id: choice.id.clone(),
                                });
                                return commands;
                            }
                        }
                        UiElementId::DialogueContinue => {
                            if guide_actions_locked {
                                return commands;
                            }
                            commands.push(InputCommand::DialogueChoice {
                                quest_id: dialogue.quest_id.clone(),
                                choice_id: "__continue__".to_string(),
                            });
                            return commands;
                        }
                        UiElementId::DialogueClose => {
                            if dialogue.quest_id != "__control_scheme__"
                                && dialogue.quest_id != "__tutorial__"
                            {
                                commands.push(InputCommand::CloseDialogue);
                                state.ui_state.active_dialogue = None;
                                state.pending_sfx.push("enter".to_string());
                                return commands;
                            }
                        }
                        _ => {}
                    }
                }
            }

            if !dialogue.choices.is_empty() {
                // Dialogue with choices - Escape cancels, number keys select
                // Don't allow closing the control scheme choice dialogue with Escape
                if is_key_pressed(KeyCode::Escape)
                    && dialogue.quest_id != "__control_scheme__"
                    && dialogue.quest_id != "__tutorial__"
                {
                    commands.push(InputCommand::CloseDialogue);
                    state.ui_state.active_dialogue = None;
                    return commands;
                }

                // Number keys (1-4) select dialogue choices
                if !guide_actions_locked && !guide_selected_active_tier {
                    let choice_keys = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4];
                    for (i, key) in choice_keys.iter().enumerate() {
                        if i < dialogue.choices.len() && is_key_pressed(*key) {
                            let choice = &dialogue.choices[i];
                            commands.push(InputCommand::DialogueChoice {
                                quest_id: dialogue.quest_id.clone(),
                                choice_id: choice.id.clone(),
                            });
                            // Don't clear dialogue here - wait for server response
                            return commands;
                        }
                    }
                }
                // Handle scroll wheel for dialogue choices
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y.abs() > 0.0 {
                    let max_scroll = layout
                        .get_max_scroll(&UiElementId::DialogueScrollbar)
                        .unwrap_or(0.0);
                    state.ui_state.dialogue_scroll_offset = (state.ui_state.dialogue_scroll_offset
                        - wheel_y * 20.0)
                        .clamp(0.0, max_scroll);
                }
            } else {
                // No choices - Escape, Enter, or Space to continue/close
                if is_key_pressed(KeyCode::Escape)
                    && dialogue.quest_id != "__control_scheme__"
                    && is_adventurer_guide_dialogue(&dialogue.speaker)
                {
                    commands.push(InputCommand::CloseDialogue);
                    state.ui_state.active_dialogue = None;
                    return commands;
                }

                // Send __continue__ to server so Lua script can resume execution
                // Don't clear dialogue here - wait for server response (either new dialogue or close)
                if !guide_actions_locked
                    && (is_key_pressed(KeyCode::Enter)
                        || is_key_pressed(KeyCode::Space)
                        || is_key_pressed(KeyCode::Escape))
                {
                    commands.push(InputCommand::DialogueChoice {
                        quest_id: dialogue.quest_id.clone(),
                        choice_id: "__continue__".to_string(),
                    });
                    return commands;
                }
            }

            // Don't process other input while dialogue is open
            return commands;
        }

        // Handle bank help overlay (blocks other bank input while open)
        if state.ui_state.bank_help_open && state.ui_state.bank_open {
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    if matches!(element, UiElementId::BankHelpClose) {
                        state.ui_state.bank_help_open = false;
                        return commands;
                    }
                }
            }
            if is_key_pressed(KeyCode::Escape)
                || is_key_pressed(KeyCode::Enter)
                || is_key_pressed(KeyCode::Space)
            {
                state.ui_state.bank_help_open = false;
                return commands;
            }
            return commands;
        }

        // Handle bank quantity dialog (blocks other bank input while open)
        if state.ui_state.bank_quantity_dialog.is_some() {
            // Handle button clicks
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::BankQuantityConfirm => {
                            let dialog = state.ui_state.bank_quantity_dialog.as_ref().unwrap();
                            if let Ok(amount) = dialog.input.parse::<i32>() {
                                if amount > 0 && amount <= dialog.max_quantity {
                                    match dialog.action {
                                        BankQuantityAction::DepositItem => {
                                            if let Some(ref item_id) = dialog.item_id {
                                                commands.push(InputCommand::BankDeposit {
                                                    item_id: item_id.clone(),
                                                    quantity: amount,
                                                });
                                            }
                                        }
                                        BankQuantityAction::WithdrawItem => {
                                            if let Some(ref item_id) = dialog.item_id {
                                                commands.push(InputCommand::BankWithdraw {
                                                    item_id: item_id.clone(),
                                                    quantity: amount,
                                                });
                                            }
                                        }
                                        BankQuantityAction::DepositGold => {
                                            commands.push(InputCommand::BankDepositGold { amount });
                                        }
                                        BankQuantityAction::WithdrawGold => {
                                            commands
                                                .push(InputCommand::BankWithdrawGold { amount });
                                        }
                                    }
                                    state.pending_sfx.push("enter".to_string());
                                    state.ui_state.bank_quantity_dialog = None;
                                }
                            }
                            return commands;
                        }
                        UiElementId::BankQuantityCancel => {
                            state.ui_state.bank_quantity_dialog = None;
                            return commands;
                        }
                        _ => {}
                    }
                }
            }

            // Keyboard input
            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.bank_quantity_dialog = None;
                return commands;
            }

            if is_key_pressed(KeyCode::Enter) {
                let dialog = state.ui_state.bank_quantity_dialog.as_ref().unwrap();
                if let Ok(amount) = dialog.input.parse::<i32>() {
                    if amount > 0 && amount <= dialog.max_quantity {
                        match dialog.action {
                            BankQuantityAction::DepositItem => {
                                if let Some(ref item_id) = dialog.item_id {
                                    commands.push(InputCommand::BankDeposit {
                                        item_id: item_id.clone(),
                                        quantity: amount,
                                    });
                                }
                            }
                            BankQuantityAction::WithdrawItem => {
                                if let Some(ref item_id) = dialog.item_id {
                                    commands.push(InputCommand::BankWithdraw {
                                        item_id: item_id.clone(),
                                        quantity: amount,
                                    });
                                }
                            }
                            BankQuantityAction::DepositGold => {
                                commands.push(InputCommand::BankDepositGold { amount });
                            }
                            BankQuantityAction::WithdrawGold => {
                                commands.push(InputCommand::BankWithdrawGold { amount });
                            }
                        }
                        state.pending_sfx.push("enter".to_string());
                        state.ui_state.bank_quantity_dialog = None;
                    }
                }
                return commands;
            }

            // Number key input
            let number_keys = [
                (KeyCode::Key0, '0'),
                (KeyCode::Key1, '1'),
                (KeyCode::Key2, '2'),
                (KeyCode::Key3, '3'),
                (KeyCode::Key4, '4'),
                (KeyCode::Key5, '5'),
                (KeyCode::Key6, '6'),
                (KeyCode::Key7, '7'),
                (KeyCode::Key8, '8'),
                (KeyCode::Key9, '9'),
                (KeyCode::Kp0, '0'),
                (KeyCode::Kp1, '1'),
                (KeyCode::Kp2, '2'),
                (KeyCode::Kp3, '3'),
                (KeyCode::Kp4, '4'),
                (KeyCode::Kp5, '5'),
                (KeyCode::Kp6, '6'),
                (KeyCode::Kp7, '7'),
                (KeyCode::Kp8, '8'),
                (KeyCode::Kp9, '9'),
            ];

            for (key, digit) in &number_keys {
                if is_key_pressed(*key) {
                    let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                    if dialog.input.len() < 10 {
                        dialog.input.insert(dialog.cursor, *digit);
                        dialog.cursor += 1;
                    }
                }
            }

            if is_key_pressed(KeyCode::Backspace) {
                let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                if dialog.cursor > 0 {
                    dialog.input.remove(dialog.cursor - 1);
                    dialog.cursor -= 1;
                }
            }

            if is_key_pressed(KeyCode::Delete) {
                let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                if dialog.cursor < dialog.input.len() {
                    dialog.input.remove(dialog.cursor);
                }
            }

            if is_key_pressed(KeyCode::Left) {
                let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                if dialog.cursor > 0 {
                    dialog.cursor -= 1;
                }
            }
            if is_key_pressed(KeyCode::Right) {
                let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                if dialog.cursor < dialog.input.len() {
                    dialog.cursor += 1;
                }
            }

            if is_key_pressed(KeyCode::Home) {
                let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                dialog.cursor = 0;
            }
            if is_key_pressed(KeyCode::End) {
                let dialog = state.ui_state.bank_quantity_dialog.as_mut().unwrap();
                dialog.cursor = dialog.input.len();
            }

            // Drain character queue to prevent ghost characters
            while get_char_pressed().is_some() {}

            return commands;
        }

        // Handle bank mode
        if state.ui_state.bank_open {
            // Auto-close if player moved too far from banker
            if let Some(local_id) = &state.local_player_id {
                if let Some(player) = state.players.get(local_id) {
                    let px = player.server_x;
                    let py = player.server_y;
                    let near_banker = state.npcs.values().any(|npc| {
                        npc.is_banker && (npc.x - px).abs() <= 3.0 && (npc.y - py).abs() <= 3.0
                    });
                    if !near_banker {
                        state.ui_state.bank_open = false;
                        state.ui_state.bank_slots.clear();
                        state.ui_state.bank_quantity_dialog = None;
                        state.ui_state.bank_help_open = false;
                        state.ui_state.bank_drag = None;
                        return commands;
                    }
                }
            }

            // Mouse wheel scrolling
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                const SCROLL_SPEED: f32 = 30.0;

                match &state.ui_state.hovered_element {
                    Some(UiElementId::BankScrollArea) | Some(UiElementId::BankSlot(_)) => {
                        let max_scroll = layout
                            .get_max_scroll(&UiElementId::BankScrollbar)
                            .unwrap_or(0.0);
                        state.ui_state.bank_scroll = (state.ui_state.bank_scroll
                            - wheel_y * SCROLL_SPEED)
                            .clamp(0.0, max_scroll);
                    }
                    Some(UiElementId::BankInvScrollArea)
                    | Some(UiElementId::BankInventorySlot(_)) => {
                        let max_scroll = layout
                            .get_max_scroll(&UiElementId::BankInvScrollbar)
                            .unwrap_or(0.0);
                        state.ui_state.bank_inv_scroll = (state.ui_state.bank_inv_scroll
                            - wheel_y * SCROLL_SPEED)
                            .clamp(0.0, max_scroll);
                    }
                    _ => {}
                }
            }

            // Bank scrollbar drag handling
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::BankScrollbar) {
                let bank_max = layout
                    .get_max_scroll(&UiElementId::BankScrollbar)
                    .unwrap_or(0.0);
                let bank_content_h = bank_max + track_bounds.h;
                let clicked_on = matches!(clicked_element, Some(UiElementId::BankScrollbar));
                crate::ui::scroll::handle_scrollbar_drag(
                    &mut state.ui_state.bank_scroll_drag,
                    &mut state.ui_state.bank_scroll,
                    bank_max,
                    track_bounds,
                    bank_content_h,
                    my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.bank_scroll_drag.dragging = false;
            }

            // Bank inventory scrollbar drag handling
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::BankInvScrollbar) {
                let inv_max = layout
                    .get_max_scroll(&UiElementId::BankInvScrollbar)
                    .unwrap_or(0.0);
                let inv_content_h = inv_max + track_bounds.h;
                let clicked_on = matches!(clicked_element, Some(UiElementId::BankInvScrollbar));
                crate::ui::scroll::handle_scrollbar_drag(
                    &mut state.ui_state.bank_inv_scroll_drag,
                    &mut state.ui_state.bank_inv_scroll,
                    inv_max,
                    track_bounds,
                    inv_content_h,
                    my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.bank_inv_scroll_drag.dragging = false;
            }

            // === Bank drag state machine ===
            let (cur_mx, cur_my) = mouse_position();
            if state.ui_state.bank_drag.is_some() {
                let drag = state.ui_state.bank_drag.as_ref().unwrap();
                let from_slot = drag.from_slot;
                let active = drag.active;

                // Cancel on right-click or Escape
                if is_mouse_button_pressed(MouseButton::Right) || is_key_pressed(KeyCode::Escape) {
                    state.ui_state.bank_drag = None;
                    return commands;
                }

                if active {
                    // Active drag: check for drop on mouse release
                    if mouse_released {
                        if let Some(UiElementId::BankSlot(target_idx)) =
                            &state.ui_state.hovered_element
                        {
                            let target = *target_idx;
                            if target != from_slot {
                                commands.push(InputCommand::BankSwapSlots {
                                    slot_a: from_slot as u32,
                                    slot_b: target as u32,
                                });
                                state.pending_sfx.push("enter".to_string());
                            }
                        }
                        state.ui_state.bank_drag = None;
                        return commands;
                    }
                    // Active drag consumes input - don't process clicks below
                    // (fall through to click handling is blocked by the else-if below)
                } else {
                    // Pending drag: check dead zone or release
                    if mouse_released {
                        // Mouse released within dead zone => treat as normal click
                        state.ui_state.bank_drag = None;
                        // Process as withdraw click
                        if let Some(Some((item_id, qty))) = state.ui_state.bank_slots.get(from_slot)
                        {
                            let ctrl_held = is_key_down(KeyCode::LeftControl)
                                || is_key_down(KeyCode::RightControl);
                            let shift_held =
                                is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                            if shift_held {
                                commands.push(InputCommand::BankWithdraw {
                                    item_id: item_id.clone(),
                                    quantity: *qty,
                                });
                                state.pending_sfx.push("enter".to_string());
                            } else if ctrl_held && *qty > 1 {
                                state.ui_state.bank_quantity_dialog = Some(BankQuantityDialog {
                                    input: String::new(),
                                    cursor: 0,
                                    action: BankQuantityAction::WithdrawItem,
                                    item_id: Some(item_id.clone()),
                                    max_quantity: *qty,
                                });
                            } else {
                                commands.push(InputCommand::BankWithdraw {
                                    item_id: item_id.clone(),
                                    quantity: 1,
                                });
                                state.pending_sfx.push("enter".to_string());
                            }
                        }
                        return commands;
                    }

                    // Check dead zone (4px = squared distance > 16.0)
                    let dx = cur_mx - drag.mouse_start_x;
                    let dy = cur_my - drag.mouse_start_y;
                    if dx * dx + dy * dy > 16.0 {
                        // Promote to active drag
                        state.ui_state.bank_drag.as_mut().unwrap().active = true;
                    }
                }

                // If we have an active drag, consume input and skip click handling
                if state
                    .ui_state
                    .bank_drag
                    .as_ref()
                    .map(|d| d.active)
                    .unwrap_or(false)
                {
                    return commands;
                }
            }

            // Initiate bank drag on mouse_clicked over a BankSlot with an item
            if mouse_clicked {
                if let Some(UiElementId::BankSlot(idx)) = &clicked_element {
                    let idx = *idx;
                    if let Some(Some(_)) = state.ui_state.bank_slots.get(idx) {
                        // Start a pending drag
                        state.ui_state.bank_drag = Some(BankDrag {
                            from_slot: idx,
                            mouse_start_x: cur_mx,
                            mouse_start_y: cur_my,
                            offset_x: 0.0,
                            offset_y: 0.0,
                            active: false,
                        });
                        // Don't fall through to the normal BankSlot click handler
                        return commands;
                    }
                }
            }

            // Click handling
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::BankHelpButton => {
                            state.ui_state.bank_help_open = true;
                            return commands;
                        }
                        UiElementId::BankCloseButton => {
                            state.ui_state.bank_open = false;
                            state.ui_state.bank_slots.clear();
                            state.ui_state.bank_quantity_dialog = None;
                            state.ui_state.bank_help_open = false;
                            state.ui_state.bank_drag = None;
                            state.pending_sfx.push("enter".to_string());
                            return commands;
                        }
                        UiElementId::BankInventorySlot(slot_idx) => {
                            // Deposit item from inventory to bank
                            if let Some(Some(inv_slot)) = state.inventory.slots.get(*slot_idx) {
                                let ctrl_held = is_key_down(KeyCode::LeftControl)
                                    || is_key_down(KeyCode::RightControl);
                                let shift_held = is_key_down(KeyCode::LeftShift)
                                    || is_key_down(KeyCode::RightShift);
                                if shift_held {
                                    // Shift+Click = deposit all
                                    commands.push(InputCommand::BankDeposit {
                                        item_id: inv_slot.item_id.clone(),
                                        quantity: inv_slot.quantity,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                } else if ctrl_held && inv_slot.quantity > 1 {
                                    // Ctrl+Click = open quantity dialog (only if stack > 1)
                                    state.ui_state.bank_quantity_dialog =
                                        Some(BankQuantityDialog {
                                            input: String::new(),
                                            cursor: 0,
                                            action: BankQuantityAction::DepositItem,
                                            item_id: Some(inv_slot.item_id.clone()),
                                            max_quantity: inv_slot.quantity,
                                        });
                                } else {
                                    // Click = deposit 1
                                    commands.push(InputCommand::BankDeposit {
                                        item_id: inv_slot.item_id.clone(),
                                        quantity: 1,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                }
                            }
                            return commands;
                        }
                        UiElementId::BankSlot(idx) => {
                            // Withdraw item from bank to inventory
                            if let Some(Some((item_id, qty))) = state.ui_state.bank_slots.get(*idx)
                            {
                                let ctrl_held = is_key_down(KeyCode::LeftControl)
                                    || is_key_down(KeyCode::RightControl);
                                let shift_held = is_key_down(KeyCode::LeftShift)
                                    || is_key_down(KeyCode::RightShift);
                                if shift_held {
                                    // Shift+Click = withdraw all
                                    commands.push(InputCommand::BankWithdraw {
                                        item_id: item_id.clone(),
                                        quantity: *qty,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                } else if ctrl_held && *qty > 1 {
                                    // Ctrl+Click = open quantity dialog (only if stack > 1)
                                    state.ui_state.bank_quantity_dialog =
                                        Some(BankQuantityDialog {
                                            input: String::new(),
                                            cursor: 0,
                                            action: BankQuantityAction::WithdrawItem,
                                            item_id: Some(item_id.clone()),
                                            max_quantity: *qty,
                                        });
                                } else {
                                    // Click = withdraw 1
                                    commands.push(InputCommand::BankWithdraw {
                                        item_id: item_id.clone(),
                                        quantity: 1,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                }
                            }
                            return commands;
                        }
                        UiElementId::BankDepositGoldButton => {
                            if state.inventory.gold > 0 {
                                let ctrl_held = is_key_down(KeyCode::LeftControl)
                                    || is_key_down(KeyCode::RightControl);
                                let shift_held = is_key_down(KeyCode::LeftShift)
                                    || is_key_down(KeyCode::RightShift);
                                if shift_held {
                                    commands.push(InputCommand::BankDepositGold {
                                        amount: state.inventory.gold,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                } else if ctrl_held {
                                    state.ui_state.bank_quantity_dialog =
                                        Some(BankQuantityDialog {
                                            input: String::new(),
                                            cursor: 0,
                                            action: BankQuantityAction::DepositGold,
                                            item_id: None,
                                            max_quantity: state.inventory.gold,
                                        });
                                } else {
                                    commands.push(InputCommand::BankDepositGold { amount: 1 });
                                    state.pending_sfx.push("enter".to_string());
                                }
                            }
                            return commands;
                        }
                        UiElementId::BankWithdrawGoldButton => {
                            if state.ui_state.bank_gold > 0 {
                                let ctrl_held = is_key_down(KeyCode::LeftControl)
                                    || is_key_down(KeyCode::RightControl);
                                let shift_held = is_key_down(KeyCode::LeftShift)
                                    || is_key_down(KeyCode::RightShift);
                                if shift_held {
                                    commands.push(InputCommand::BankWithdrawGold {
                                        amount: state.ui_state.bank_gold,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                } else if ctrl_held {
                                    state.ui_state.bank_quantity_dialog =
                                        Some(BankQuantityDialog {
                                            input: String::new(),
                                            cursor: 0,
                                            action: BankQuantityAction::WithdrawGold,
                                            item_id: None,
                                            max_quantity: state.ui_state.bank_gold,
                                        });
                                } else {
                                    commands.push(InputCommand::BankWithdrawGold { amount: 1 });
                                    state.pending_sfx.push("enter".to_string());
                                }
                            }
                            return commands;
                        }
                        UiElementId::BankDepositAllButton => {
                            commands.push(InputCommand::BankDepositAll);
                            state.pending_sfx.push("enter".to_string());
                            return commands;
                        }
                        UiElementId::BankSortButton => {
                            commands.push(InputCommand::BankSort);
                            state.pending_sfx.push("enter".to_string());
                            return commands;
                        }
                        _ => {}
                    }
                }
            }

            // Escape to close
            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.bank_open = false;
                state.ui_state.bank_slots.clear();
                state.ui_state.bank_quantity_dialog = None;
                state.ui_state.bank_help_open = false;
                state.ui_state.bank_drag = None;
                return commands;
            }

            return commands;
        }

        // Handle crafting mode
        if state.ui_state.crafting_open {
            // Touch drag scrolling for shop lists on mobile
            let all_touches: Vec<macroquad::input::Touch> = macroquad::input::touches();
            if let Some(tracking_id) = state.ui_state.shop_touch_scroll_id {
                if let Some(touch) = all_touches.iter().find(|t| t.id == tracking_id) {
                    match touch.phase {
                        macroquad::input::TouchPhase::Moved
                        | macroquad::input::TouchPhase::Stationary => {
                            let (_, vy) =
                                screen_to_virtual_coords(touch.position.x, touch.position.y);
                            let dy = state.ui_state.shop_touch_last_y - vy;
                            if !state.ui_state.shop_touch_dragged {
                                let total_dy = (state.ui_state.shop_touch_start_y - vy).abs();
                                if total_dy > 8.0 {
                                    state.ui_state.shop_touch_dragged = true;
                                }
                            }
                            if state.ui_state.shop_touch_dragged {
                                let item_height = 48.0 + 4.0; // SHOP_ITEM_HEIGHT + SHOP_ITEM_SPACING
                                if state.ui_state.shop_touch_scroll_column == 0 {
                                    let max_scroll = state
                                        .ui_state
                                        .shop_data
                                        .as_ref()
                                        .map(|d| {
                                            ((d.stock.len() as f32) * item_height - 200.0).max(0.0)
                                        })
                                        .unwrap_or(0.0);
                                    state.ui_state.shop_buy_scroll =
                                        (state.ui_state.shop_buy_scroll + dy)
                                            .clamp(0.0, max_scroll);
                                } else {
                                    let inventory_count = state.inventory.aggregate_items().len();
                                    let max_scroll =
                                        ((inventory_count as f32) * item_height - 200.0).max(0.0);
                                    state.ui_state.shop_sell_scroll =
                                        (state.ui_state.shop_sell_scroll + dy)
                                            .clamp(0.0, max_scroll);
                                }
                            }
                            state.ui_state.shop_touch_last_y = vy;
                        }
                        macroquad::input::TouchPhase::Ended
                        | macroquad::input::TouchPhase::Cancelled => {
                            state.ui_state.shop_touch_scroll_id = None;
                        }
                        _ => {}
                    }
                } else {
                    state.ui_state.shop_touch_scroll_id = None;
                }
            } else {
                for touch in &all_touches {
                    if touch.phase == macroquad::input::TouchPhase::Started {
                        let (vx, vy) = screen_to_virtual_coords(touch.position.x, touch.position.y);
                        let hit = layout.hit_test(vx, vy);
                        let buy_area = matches!(
                            hit,
                            Some(UiElementId::ShopBuyScrollArea)
                                | Some(UiElementId::ShopBuyItem(_))
                        );
                        let sell_area = matches!(
                            hit,
                            Some(UiElementId::ShopSellScrollArea)
                                | Some(UiElementId::ShopSellItem(_))
                        );
                        if buy_area || sell_area {
                            state.ui_state.shop_touch_scroll_id = Some(touch.id);
                            state.ui_state.shop_touch_scroll_column = if buy_area { 0 } else { 1 };
                            state.ui_state.shop_touch_last_y = vy;
                            state.ui_state.shop_touch_start_y = vy;
                            state.ui_state.shop_touch_dragged = false;
                            break;
                        }
                    }
                }
            }

            // Suppress click actions if the touch was a scroll drag
            let was_shop_touch_drag =
                state.ui_state.shop_touch_dragged && state.ui_state.shop_touch_scroll_id.is_none();
            if was_shop_touch_drag {
                state.ui_state.shop_touch_dragged = false;
            }

            // Handle mouse clicks on crafting elements (only on mouse down, not release)
            if mouse_clicked && !was_shop_touch_drag {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::ShopCraftingCloseButton => {
                            state.ui_state.crafting_open = false;
                            state.ui_state.crafting_npc_id = None;
                            state.ui_state.shop_data = None;
                            state.ui_state.shop_quantity_hold_element = None;
                            state.pending_sfx.push("enter".to_string());
                            return commands;
                        }
                        UiElementId::MainTab(idx) => {
                            state.ui_state.shop_main_tab = *idx;
                            state.pending_sfx.push("enter".to_string());
                            return commands;
                        }
                        UiElementId::CraftingCategoryTab(idx) => {
                            // Disable category switching during crafting
                            if !state.ui_state.crafting_in_progress {
                                if *idx != state.ui_state.crafting_selected_category {
                                    state.ui_state.crafting_selected_category = *idx;
                                    state.ui_state.crafting_selected_recipe = 0;
                                    state.ui_state.crafting_scroll_offset = 0.0;
                                    state.pending_sfx.push("enter".to_string());
                                }
                            }
                            return commands;
                        }
                        UiElementId::CraftingRecipeItem(idx) => {
                            // Disable recipe selection during crafting
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.crafting_selected_recipe = *idx;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return commands;
                        }
                        UiElementId::CraftingButton => {
                            // Don't allow crafting while already in progress
                            if state.ui_state.crafting_in_progress {
                                return commands;
                            }
                            // Get unique categories from recipes (matching renderer grouping)
                            let click_filtered = state.shop_filtered_recipes();
                            let categories: Vec<String> = {
                                let mut cats: Vec<String> = click_filtered
                                    .iter()
                                    .map(|r| {
                                        if r.category == "materials" || r.category == "consumables"
                                        {
                                            "supplies".to_string()
                                        } else {
                                            r.category.clone()
                                        }
                                    })
                                    .collect();
                                cats.sort();
                                cats.dedup();
                                cats
                            };
                            let selected_idx = state
                                .ui_state
                                .crafting_selected_category
                                .min(categories.len().saturating_sub(1));
                            let current_category = categories
                                .get(selected_idx)
                                .map(|s| s.as_str())
                                .unwrap_or("supplies");
                            let mut recipes_in_category: Vec<&crate::game::RecipeDefinition> =
                                click_filtered
                                    .iter()
                                    .filter(|r| {
                                        let cat_match = if current_category == "supplies" {
                                            r.category == "consumables" || r.category == "materials"
                                        } else {
                                            r.category == current_category
                                        };
                                        // Only include discovered recipes (matching renderer)
                                        let is_discovered = !r.requires_discovery
                                            || state.discovered_recipes.contains(&r.id);
                                        cat_match && is_discovered
                                    })
                                    .collect();
                            // Sort to match renderer order (section → level → name)
                            recipes_in_category.sort_by(|a, b| {
                                let a_sec = a.section.as_deref().unwrap_or("");
                                let b_sec = b.section.as_deref().unwrap_or("");
                                section_sort_key(a_sec)
                                    .cmp(&section_sort_key(b_sec))
                                    .then(a.level_required.cmp(&b.level_required))
                                    .then(a.display_name.cmp(&b.display_name))
                            });
                            if let Some(recipe) =
                                recipes_in_category.get(state.ui_state.crafting_selected_recipe)
                            {
                                log::info!("Crafting (click): {}", recipe.id);
                                commands.push(InputCommand::Craft {
                                    recipe_id: recipe.id.clone(),
                                });
                            }
                            return commands;
                        }
                        UiElementId::CraftingCancelButton => {
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            return commands;
                        }
                        UiElementId::ShopBuyItem(idx) => {
                            state.ui_state.shop_selected_buy_index = *idx;
                            state.ui_state.shop_buy_quantity = 1;
                            state.pending_sfx.push("enter".to_string());
                            return commands;
                        }
                        UiElementId::ShopSellItem(idx) => {
                            state.ui_state.shop_selected_sell_index = *idx;
                            state.ui_state.shop_sell_quantity = 1;
                            state.pending_sfx.push("enter".to_string());
                            return commands;
                        }
                        UiElementId::ShopBuyQuantityMinus => {
                            if state.ui_state.shop_buy_quantity > 1 {
                                state.ui_state.shop_buy_quantity -= 1;
                            }
                            state.ui_state.shop_quantity_hold_element =
                                Some(UiElementId::ShopBuyQuantityMinus);
                            state.ui_state.shop_quantity_hold_start = current_time;
                            state.ui_state.shop_quantity_hold_last_repeat = current_time;
                            return commands;
                        }
                        UiElementId::ShopBuyQuantityPlus => {
                            state.ui_state.shop_buy_quantity += 1;
                            state.ui_state.shop_quantity_hold_element =
                                Some(UiElementId::ShopBuyQuantityPlus);
                            state.ui_state.shop_quantity_hold_start = current_time;
                            state.ui_state.shop_quantity_hold_last_repeat = current_time;
                            return commands;
                        }
                        UiElementId::ShopSellQuantityMinus => {
                            if state.ui_state.shop_sell_quantity > 1 {
                                state.ui_state.shop_sell_quantity -= 1;
                            }
                            state.ui_state.shop_quantity_hold_element =
                                Some(UiElementId::ShopSellQuantityMinus);
                            state.ui_state.shop_quantity_hold_start = current_time;
                            state.ui_state.shop_quantity_hold_last_repeat = current_time;
                            return commands;
                        }
                        UiElementId::ShopSellQuantityPlus => {
                            state.ui_state.shop_sell_quantity += 1;
                            state.ui_state.shop_quantity_hold_element =
                                Some(UiElementId::ShopSellQuantityPlus);
                            state.ui_state.shop_quantity_hold_start = current_time;
                            state.ui_state.shop_quantity_hold_last_repeat = current_time;
                            return commands;
                        }
                        UiElementId::ShopSellQuantityMax => {
                            let inventory_items = state.inventory.aggregate_items();
                            if let Some(agg_item) =
                                inventory_items.get(state.ui_state.shop_selected_sell_index)
                            {
                                state.ui_state.shop_sell_quantity = agg_item.total_quantity.max(1);
                            }
                            return commands;
                        }
                        UiElementId::ShopBuyConfirmButton => {
                            if let Some(ref shop_data) = state.ui_state.shop_data {
                                if let Some(ref npc_id) = state.ui_state.shop_npc_id {
                                    if let Some(stock_item) =
                                        shop_data.stock.get(state.ui_state.shop_selected_buy_index)
                                    {
                                        audio.play_sfx("buy");
                                        commands.push(InputCommand::ShopBuy {
                                            npc_id: npc_id.clone(),
                                            item_id: stock_item.item_id.clone(),
                                            quantity: state.ui_state.shop_buy_quantity as u32,
                                        });
                                    }
                                }
                            }
                            return commands;
                        }
                        UiElementId::ShopSellConfirmButton => {
                            if let Some(ref npc_id) = state.ui_state.shop_npc_id {
                                let inventory_items = state.inventory.aggregate_items();
                                if let Some(agg_item) =
                                    inventory_items.get(state.ui_state.shop_selected_sell_index)
                                {
                                    commands.push(InputCommand::ShopSell {
                                        npc_id: npc_id.clone(),
                                        item_id: agg_item.item_id.clone(),
                                        quantity: state.ui_state.shop_sell_quantity as u32,
                                    });
                                }
                            }
                            return commands;
                        }
                        _ => {}
                    }
                }
            }

            // Hold-to-repeat for quantity +/- buttons
            if is_mouse_button_down(MouseButton::Left) {
                if let Some(ref hold_elem) = state.ui_state.shop_quantity_hold_element {
                    // Check if still hovering the same button
                    let still_hovering = state.ui_state.hovered_element.as_ref() == Some(hold_elem);
                    if still_hovering {
                        const INITIAL_DELAY: f64 = 0.4;
                        const REPEAT_INTERVAL: f64 = 0.06;
                        let held_duration = current_time - state.ui_state.shop_quantity_hold_start;
                        if held_duration >= INITIAL_DELAY {
                            let since_last =
                                current_time - state.ui_state.shop_quantity_hold_last_repeat;
                            if since_last >= REPEAT_INTERVAL {
                                state.ui_state.shop_quantity_hold_last_repeat = current_time;
                                match hold_elem {
                                    UiElementId::ShopBuyQuantityMinus => {
                                        if state.ui_state.shop_buy_quantity > 1 {
                                            state.ui_state.shop_buy_quantity -= 1;
                                        }
                                    }
                                    UiElementId::ShopBuyQuantityPlus => {
                                        state.ui_state.shop_buy_quantity += 1;
                                    }
                                    UiElementId::ShopSellQuantityMinus => {
                                        if state.ui_state.shop_sell_quantity > 1 {
                                            state.ui_state.shop_sell_quantity -= 1;
                                        }
                                    }
                                    UiElementId::ShopSellQuantityPlus => {
                                        state.ui_state.shop_sell_quantity += 1;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    } else {
                        state.ui_state.shop_quantity_hold_element = None;
                    }
                }
            } else {
                state.ui_state.shop_quantity_hold_element = None;
            }

            // Allow chat input while shop panel is open
            if state.ui_state.chat_open && !state.ui_state.classic_controls {
                process_chat_keyboard_input(state, &mut commands, audio);
                return commands;
            }

            // Escape: if crafting in progress, cancel craft; otherwise close menu
            if is_key_pressed(KeyCode::Escape) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                    return commands;
                }
                state.ui_state.crafting_open = false;
                state.ui_state.crafting_npc_id = None;
                state.ui_state.shop_data = None;
                state.ui_state.shop_quantity_hold_element = None;
                return commands;
            }

            // Q switches between Recipes/Shop main tabs
            if is_key_pressed(KeyCode::Q) {
                state.ui_state.shop_main_tab = if state.ui_state.shop_main_tab == 0 {
                    1
                } else {
                    0
                };
            }

            if state.ui_state.shop_main_tab == 0 {
                // Recipes tab keyboard controls
                // Get recipes filtered by this shop's categories
                let filtered_recipes = state.shop_filtered_recipes();
                // Get unique categories from recipes, merging consumables and materials
                let categories: Vec<String> = {
                    let mut cats: Vec<String> = filtered_recipes
                        .iter()
                        .map(|r| {
                            if r.category == "materials" || r.category == "consumables" {
                                "supplies".to_string()
                            } else {
                                r.category.clone()
                            }
                        })
                        .collect();
                    cats.sort();
                    cats.dedup();
                    cats
                };

                // Disable navigation during crafting
                if !state.ui_state.crafting_in_progress {
                    // Left/Right navigate categories
                    if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
                        if state.ui_state.crafting_selected_category > 0 {
                            state.ui_state.crafting_selected_category -= 1;
                            state.ui_state.crafting_selected_recipe = 0;
                            state.ui_state.crafting_scroll_offset = 0.0;
                        }
                    }
                    if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
                        if state.ui_state.crafting_selected_category
                            < categories.len().saturating_sub(1)
                        {
                            state.ui_state.crafting_selected_category += 1;
                            state.ui_state.crafting_selected_recipe = 0;
                            state.ui_state.crafting_scroll_offset = 0.0;
                        }
                    }

                    // Get discovered recipes for current category (matches renderer filtering)
                    let selected_idx = state
                        .ui_state
                        .crafting_selected_category
                        .min(categories.len().saturating_sub(1));
                    let current_category = categories
                        .get(selected_idx)
                        .map(|s| s.as_str())
                        .unwrap_or("supplies");
                    let mut recipes_in_category: Vec<&crate::game::RecipeDefinition> =
                        filtered_recipes
                            .iter()
                            .filter(|r| {
                                let cat_match = if current_category == "supplies" {
                                    r.category == "consumables" || r.category == "materials"
                                } else {
                                    r.category == current_category
                                };
                                // Only include discovered recipes (matching renderer)
                                let is_discovered = !r.requires_discovery
                                    || state.discovered_recipes.contains(&r.id);
                                cat_match && is_discovered
                            })
                            .collect();
                    // Sort to match renderer order (section → level → name)
                    recipes_in_category.sort_by(|a, b| {
                        let a_sec = a.section.as_deref().unwrap_or("");
                        let b_sec = b.section.as_deref().unwrap_or("");
                        section_sort_key(a_sec)
                            .cmp(&section_sort_key(b_sec))
                            .then(a.level_required.cmp(&b.level_required))
                            .then(a.display_name.cmp(&b.display_name))
                    });

                    // Up/Down navigate recipes
                    let mut key_navigated = false;
                    if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                        if state.ui_state.crafting_selected_recipe > 0 {
                            state.ui_state.crafting_selected_recipe -= 1;
                            key_navigated = true;
                        }
                    }
                    if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                        if state.ui_state.crafting_selected_recipe
                            < recipes_in_category.len().saturating_sub(1)
                        {
                            state.ui_state.crafting_selected_recipe += 1;
                            key_navigated = true;
                        }
                    }

                    // Only auto-scroll when keyboard navigated, not every frame
                    if key_navigated {
                        let s = state.ui_state.ui_scale;
                        let craft_line_h = 28.0 * s;
                        let section_h = SECTION_HEADER_HEIGHT * s;
                        // Count the actual row position including undiscovered "????" entries
                        // and section headers (must match renderer layout)
                        let mut all_in_category: Vec<&crate::game::RecipeDefinition> =
                            filtered_recipes
                                .iter()
                                .filter(|r| {
                                    if current_category == "supplies" {
                                        r.category == "consumables" || r.category == "materials"
                                    } else {
                                        r.category == current_category
                                    }
                                })
                                .collect();
                        // Sort by section to match renderer order
                        all_in_category.sort_by(|a, b| {
                            let sa = a.section.as_deref().unwrap_or("");
                            let sb = b.section.as_deref().unwrap_or("");
                            section_sort_key(sa)
                                .cmp(&section_sort_key(sb))
                                .then_with(|| a.level_required.cmp(&b.level_required))
                                .then_with(|| a.display_name.cmp(&b.display_name))
                        });
                        // Walk through items tracking pixel position with section headers
                        let mut pixel_y = 0.0_f32;
                        let mut current_section: Option<&str> = None;
                        let mut discovered_idx = 0usize;
                        let mut item_top = 0.0_f32;
                        for r in &all_in_category {
                            let recipe_section = r.section.as_deref().unwrap_or("");
                            if !recipe_section.is_empty() && current_section != Some(recipe_section)
                            {
                                current_section = Some(recipe_section);
                                pixel_y += section_h;
                            }
                            let is_disc =
                                !r.requires_discovery || state.discovered_recipes.contains(&r.id);
                            if is_disc {
                                if discovered_idx == state.ui_state.crafting_selected_recipe {
                                    item_top = pixel_y;
                                    break;
                                }
                                discovered_idx += 1;
                            }
                            pixel_y += craft_line_h;
                        }
                        let item_bottom = item_top + craft_line_h;
                        if item_top < state.ui_state.crafting_scroll_offset {
                            state.ui_state.crafting_scroll_offset = item_top;
                        }
                        // Calculate visible height matching renderer layout (scaled)
                        let (_, sh) = crate::util::virtual_screen_size();
                        let panel_h = (450.0 * s).min(sh - 16.0);
                        let content_height = panel_h - 8.0 - 40.0 * s - 30.0 * s - 12.0 * s;
                        let has_tabs = categories.len() > 1;
                        let list_height = if has_tabs {
                            content_height - 28.0 * s - 20.0 * s
                        } else {
                            content_height - 10.0 * s
                        };
                        let visible_h = list_height - 8.0 * s;
                        if item_bottom > state.ui_state.crafting_scroll_offset + visible_h {
                            state.ui_state.crafting_scroll_offset = item_bottom - visible_h;
                        }
                    }

                    // Enter or C crafts selected recipe
                    if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::C) {
                        if let Some(recipe) =
                            recipes_in_category.get(state.ui_state.crafting_selected_recipe)
                        {
                            log::info!("Crafting: {}", recipe.id);
                            commands.push(InputCommand::Craft {
                                recipe_id: recipe.id.clone(),
                            });
                        }
                    }
                } else {
                    // While crafting is in progress, X key cancels
                    if is_key_pressed(KeyCode::X) {
                        commands.push(InputCommand::CancelCraft);
                        return commands;
                    }
                }

                // Mouse wheel scrolling for crafting recipe list (same logic as shop tab)
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y != 0.0 {
                    let s = state.ui_state.ui_scale;
                    const SCROLL_SPEED: f32 = 30.0;
                    let line_height = 28.0 * s;
                    // Count all recipes in category (discovered + undiscovered) to match renderer
                    let sel_idx = state
                        .ui_state
                        .crafting_selected_category
                        .min(categories.len().saturating_sub(1));
                    let cur_cat = categories
                        .get(sel_idx)
                        .map(|sc| sc.as_str())
                        .unwrap_or("supplies");
                    let recipes_in_cat: Vec<&crate::game::RecipeDefinition> = filtered_recipes
                        .iter()
                        .filter(|r| {
                            if cur_cat == "supplies" {
                                r.category == "consumables" || r.category == "materials"
                            } else {
                                r.category == cur_cat
                            }
                        })
                        .collect();
                    let total_visible = recipes_in_cat.len();
                    // Count distinct sections for header height
                    let num_scroll_sections = {
                        let mut seen = std::collections::HashSet::new();
                        for r in &recipes_in_cat {
                            if let Some(ref sec) = r.section {
                                if !sec.is_empty() {
                                    seen.insert(sec.as_str());
                                }
                            }
                        }
                        seen.len()
                    };
                    // Match renderer layout constants (scaled by ui_scale)
                    let (_, sh) = crate::util::virtual_screen_size();
                    let panel_height = (450.0 * s).min(sh - 16.0);
                    let content_height = panel_height - 8.0 - 40.0 * s - 30.0 * s - 12.0 * s;
                    let has_tabs = categories.len() > 1;
                    let list_height = if has_tabs {
                        content_height - 28.0 * s - 20.0 * s
                    } else {
                        content_height - 10.0 * s
                    };
                    let list_content_height = list_height - 8.0 * s;
                    let total_content = total_visible as f32 * line_height
                        + num_scroll_sections as f32 * SECTION_HEADER_HEIGHT * s;
                    let max_scroll = (total_content - list_content_height).max(0.0);
                    state.ui_state.crafting_scroll_offset = (state.ui_state.crafting_scroll_offset
                        - wheel_y * SCROLL_SPEED)
                        .clamp(0.0, max_scroll);
                }

                // Crafting scrollbar drag handling
                if let Some(track_bounds) = layout.get_bounds(&UiElementId::CraftingScrollbar) {
                    let s = state.ui_state.ui_scale;
                    let line_height = if cfg!(target_os = "android") {
                        36.0 * s
                    } else {
                        28.0 * s
                    };
                    let sel_idx = state
                        .ui_state
                        .crafting_selected_category
                        .min(categories.len().saturating_sub(1));
                    let cur_cat = categories
                        .get(sel_idx)
                        .map(|sc| sc.as_str())
                        .unwrap_or("supplies");
                    let recipes_in_cat: Vec<&crate::game::RecipeDefinition> = filtered_recipes
                        .iter()
                        .filter(|r| {
                            if cur_cat == "supplies" {
                                r.category == "consumables" || r.category == "materials"
                            } else {
                                r.category == cur_cat
                            }
                        })
                        .collect();
                    let drag_total_visible = recipes_in_cat.len();
                    let drag_num_sections = {
                        let mut seen = std::collections::HashSet::new();
                        for r in &recipes_in_cat {
                            if let Some(ref sec) = r.section {
                                if !sec.is_empty() {
                                    seen.insert(sec.as_str());
                                }
                            }
                        }
                        seen.len()
                    };
                    let total_content = drag_total_visible as f32 * line_height
                        + drag_num_sections as f32 * SECTION_HEADER_HEIGHT * s;
                    let max_scroll = (total_content - track_bounds.h).max(0.0);
                    let clicked_on =
                        matches!(clicked_element, Some(UiElementId::CraftingScrollbar));
                    crate::ui::scroll::handle_scrollbar_drag(
                        &mut state.ui_state.crafting_scroll_drag,
                        &mut state.ui_state.crafting_scroll_offset,
                        max_scroll,
                        track_bounds,
                        total_content,
                        my,
                        is_mouse_button_down(MouseButton::Left),
                        mouse_clicked,
                        clicked_on,
                    );
                } else if !is_mouse_button_down(MouseButton::Left) {
                    state.ui_state.crafting_scroll_drag.dragging = false;
                }
            } else if state.ui_state.shop_main_tab == 1 {
                // Shop tab - side-by-side Buy/Sell layout
                // Mouse wheel scrolling based on which scroll area the mouse is hovering over
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y != 0.0 {
                    const SCROLL_SPEED: f32 = 30.0;
                    let item_height = 48.0 + 4.0; // height + spacing

                    // Check which area is being hovered
                    match &state.ui_state.hovered_element {
                        Some(UiElementId::ShopBuyScrollArea)
                        | Some(UiElementId::ShopBuyItem(_)) => {
                            if let Some(ref shop_data) = state.ui_state.shop_data {
                                let max_scroll =
                                    ((shop_data.stock.len() as f32) * item_height - 200.0).max(0.0);
                                state.ui_state.shop_buy_scroll = (state.ui_state.shop_buy_scroll
                                    - wheel_y * SCROLL_SPEED)
                                    .clamp(0.0, max_scroll);
                            }
                        }
                        Some(UiElementId::ShopSellScrollArea)
                        | Some(UiElementId::ShopSellItem(_)) => {
                            let inventory_count = state.inventory.aggregate_items().len();
                            let max_scroll =
                                ((inventory_count as f32) * item_height - 200.0).max(0.0);
                            state.ui_state.shop_sell_scroll = (state.ui_state.shop_sell_scroll
                                - wheel_y * SCROLL_SPEED)
                                .clamp(0.0, max_scroll);
                        }
                        _ => {}
                    }
                }

                // Scrollbar drag handling for shop buy/sell
                {
                    let item_height = 48.0 + 4.0;
                    // Buy scrollbar
                    if let Some(track_bounds) = layout.get_bounds(&UiElementId::ShopBuyScrollbar) {
                        let max_scroll = state
                            .ui_state
                            .shop_data
                            .as_ref()
                            .map(|d| ((d.stock.len() as f32) * item_height - 200.0).max(0.0))
                            .unwrap_or(0.0);
                        let clicked_on =
                            matches!(clicked_element, Some(UiElementId::ShopBuyScrollbar));
                        crate::ui::scroll::handle_scrollbar_drag(
                            &mut state.ui_state.shop_buy_scroll_drag,
                            &mut state.ui_state.shop_buy_scroll,
                            max_scroll,
                            track_bounds,
                            max_scroll + 200.0,
                            my,
                            is_mouse_button_down(MouseButton::Left),
                            mouse_clicked,
                            clicked_on,
                        );
                    } else if !is_mouse_button_down(MouseButton::Left) {
                        state.ui_state.shop_buy_scroll_drag.dragging = false;
                    }
                    // Sell scrollbar
                    if let Some(track_bounds) = layout.get_bounds(&UiElementId::ShopSellScrollbar) {
                        let inventory_count = state.inventory.aggregate_items().len();
                        let max_scroll = ((inventory_count as f32) * item_height - 200.0).max(0.0);
                        let clicked_on =
                            matches!(clicked_element, Some(UiElementId::ShopSellScrollbar));
                        crate::ui::scroll::handle_scrollbar_drag(
                            &mut state.ui_state.shop_sell_scroll_drag,
                            &mut state.ui_state.shop_sell_scroll,
                            max_scroll,
                            track_bounds,
                            max_scroll + 200.0,
                            my,
                            is_mouse_button_down(MouseButton::Left),
                            mouse_clicked,
                            clicked_on,
                        );
                    } else if !is_mouse_button_down(MouseButton::Left) {
                        state.ui_state.shop_sell_scroll_drag.dragging = false;
                    }
                }

                // Keyboard controls for shop
                use crate::game::ShopSubTab;

                // Left/Right or A/D to switch between Buy and Sell panels
                if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
                    state.ui_state.shop_sub_tab = ShopSubTab::Buy;
                }
                if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
                    state.ui_state.shop_sub_tab = ShopSubTab::Sell;
                }
                // Tab to toggle between panels
                if is_key_pressed(KeyCode::Tab) {
                    state.ui_state.shop_sub_tab = match state.ui_state.shop_sub_tab {
                        ShopSubTab::Buy => ShopSubTab::Sell,
                        ShopSubTab::Sell => ShopSubTab::Buy,
                    };
                }

                // Up/Down or W/S to navigate items in the active panel
                match state.ui_state.shop_sub_tab {
                    ShopSubTab::Buy => {
                        let item_count = state
                            .ui_state
                            .shop_data
                            .as_ref()
                            .map(|d| d.stock.len())
                            .unwrap_or(0);

                        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                            if state.ui_state.shop_selected_buy_index > 0 {
                                state.ui_state.shop_selected_buy_index -= 1;
                                state.ui_state.shop_buy_quantity = 1;
                            }
                        }
                        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                            if state.ui_state.shop_selected_buy_index < item_count.saturating_sub(1)
                            {
                                state.ui_state.shop_selected_buy_index += 1;
                                state.ui_state.shop_buy_quantity = 1;
                            }
                        }

                        // +/- to adjust quantity
                        if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
                            state.ui_state.shop_buy_quantity += 1;
                        }
                        if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
                            if state.ui_state.shop_buy_quantity > 1 {
                                state.ui_state.shop_buy_quantity -= 1;
                            }
                        }

                        // Enter to confirm buy
                        if is_key_pressed(KeyCode::Enter) {
                            if let Some(ref shop_data) = state.ui_state.shop_data {
                                if let Some(ref npc_id) = state.ui_state.shop_npc_id {
                                    if let Some(stock_item) =
                                        shop_data.stock.get(state.ui_state.shop_selected_buy_index)
                                    {
                                        audio.play_sfx("buy");
                                        commands.push(InputCommand::ShopBuy {
                                            npc_id: npc_id.clone(),
                                            item_id: stock_item.item_id.clone(),
                                            quantity: state.ui_state.shop_buy_quantity as u32,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    ShopSubTab::Sell => {
                        let inventory_items = state.inventory.aggregate_items();
                        let item_count = inventory_items.len();

                        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                            if state.ui_state.shop_selected_sell_index > 0 {
                                state.ui_state.shop_selected_sell_index -= 1;
                                state.ui_state.shop_sell_quantity = 1;
                            }
                        }
                        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                            if state.ui_state.shop_selected_sell_index
                                < item_count.saturating_sub(1)
                            {
                                state.ui_state.shop_selected_sell_index += 1;
                                state.ui_state.shop_sell_quantity = 1;
                            }
                        }

                        // +/- to adjust quantity
                        if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
                            state.ui_state.shop_sell_quantity += 1;
                        }
                        if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
                            if state.ui_state.shop_sell_quantity > 1 {
                                state.ui_state.shop_sell_quantity -= 1;
                            }
                        }

                        // Enter to confirm sell
                        if is_key_pressed(KeyCode::Enter) {
                            if let Some(ref npc_id) = state.ui_state.shop_npc_id {
                                if let Some(agg_item) =
                                    inventory_items.get(state.ui_state.shop_selected_sell_index)
                                {
                                    commands.push(InputCommand::ShopSell {
                                        npc_id: npc_id.clone(),
                                        item_id: agg_item.item_id.clone(),
                                        quantity: state.ui_state.shop_sell_quantity as u32,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // Don't process other input while crafting is open
            return commands;
        }

        // Handle furnace mode
        if state.ui_state.furnace_open {
            // Handle mouse clicks on furnace elements
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::FurnaceCloseButton => {
                            state.ui_state.furnace_open = false;
                            state.ui_state.furnace_tile = None;
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            state.pending_sfx.push("enter".to_string());
                            return commands;
                        }
                        UiElementId::FurnaceRecipeItem(idx) => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.furnace_selected_recipe = *idx;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return commands;
                        }
                        UiElementId::FurnaceSmeltButton => {
                            if !state.ui_state.crafting_in_progress {
                                let station = state.ui_state.furnace_station_type.as_str();
                                let is_fire_pit = station == "fire_pit";
                                let section_filter = if is_fire_pit {
                                    "fish"
                                } else if state.ui_state.furnace_tab == 0 {
                                    "materials"
                                } else {
                                    "jewelry"
                                };
                                let mut furnace_recipes: Vec<_> = state
                                    .recipe_definitions
                                    .iter()
                                    .filter(|r| r.station.as_deref() == Some(station))
                                    .filter(|r| {
                                        if is_fire_pit {
                                            true
                                        } else {
                                            r.section.as_deref() == Some(section_filter)
                                        }
                                    })
                                    .filter(|r| {
                                        !r.requires_discovery
                                            || state.discovered_recipes.contains(&r.id)
                                    })
                                    .collect();
                                furnace_recipes.sort_by_key(|r| r.level_required);
                                if let Some(recipe) =
                                    furnace_recipes.get(state.ui_state.furnace_selected_recipe)
                                {
                                    commands.push(InputCommand::FurnaceCraft {
                                        recipe_id: recipe.id.clone(),
                                        quantity: state.ui_state.furnace_quantity,
                                    });
                                }
                            }
                            return commands;
                        }
                        UiElementId::FurnaceCancelButton => {
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            return commands;
                        }
                        UiElementId::FurnaceQuantity1 => {
                            state.ui_state.furnace_quantity = 1;
                            return commands;
                        }
                        UiElementId::FurnaceQuantityX => {
                            // Toggle to a reasonable default (5) or cycle
                            state.ui_state.furnace_quantity =
                                if state.ui_state.furnace_quantity == 5 {
                                    10
                                } else {
                                    5
                                };
                            return commands;
                        }
                        UiElementId::FurnaceQuantityAll => {
                            state.ui_state.furnace_quantity = u32::MAX;
                            return commands;
                        }
                        UiElementId::FurnaceTabSmelting => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.furnace_tab = 0;
                                state.ui_state.furnace_selected_recipe = 0;
                                state.ui_state.furnace_scroll_offset = 0.0;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return commands;
                        }
                        UiElementId::FurnaceTabJewelry => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.furnace_tab = 1;
                                state.ui_state.furnace_selected_recipe = 0;
                                state.ui_state.furnace_scroll_offset = 0.0;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return commands;
                        }
                        _ => {}
                    }
                }
            }

            // Allow chat input while furnace panel is open
            if state.ui_state.chat_open && !state.ui_state.classic_controls {
                process_chat_keyboard_input(state, &mut commands, audio);
                return commands;
            }

            // Escape: cancel if crafting, otherwise close
            if is_key_pressed(KeyCode::Escape) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                    return commands;
                }
                state.ui_state.furnace_open = false;
                state.ui_state.furnace_tile = None;
                return commands;
            }

            // E key closes furnace
            if is_key_pressed(KeyCode::E) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                }
                state.ui_state.furnace_open = false;
                state.ui_state.furnace_tile = None;
                return commands;
            }

            if !state.ui_state.crafting_in_progress {
                // Tab key to cycle tabs
                if is_key_pressed(KeyCode::Tab) {
                    state.ui_state.furnace_tab = (state.ui_state.furnace_tab + 1) % 2;
                    state.ui_state.furnace_selected_recipe = 0;
                    state.ui_state.furnace_scroll_offset = 0.0;
                    state.pending_sfx.push("enter".to_string());
                }

                let station = state.ui_state.furnace_station_type.as_str();
                let is_fire_pit = station == "fire_pit";
                let section_filter = if is_fire_pit {
                    "fish"
                } else if state.ui_state.furnace_tab == 0 {
                    "materials"
                } else {
                    "jewelry"
                };
                let mut furnace_recipes: Vec<_> = state
                    .recipe_definitions
                    .iter()
                    .filter(|r| r.station.as_deref() == Some(station))
                    .filter(|r| {
                        if is_fire_pit {
                            true
                        } else {
                            r.section.as_deref() == Some(section_filter)
                        }
                    })
                    .filter(|r| !r.requires_discovery || state.discovered_recipes.contains(&r.id))
                    .collect();
                furnace_recipes.sort_by_key(|r| r.level_required);
                let recipe_count = furnace_recipes.len();

                // W/S or Up/Down to navigate recipes
                if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                    if state.ui_state.furnace_selected_recipe > 0 {
                        state.ui_state.furnace_selected_recipe -= 1;
                        // Auto-scroll to keep selected in view
                        let row_h = 72.0_f32;
                        let item_top = state.ui_state.furnace_selected_recipe as f32 * row_h;
                        if item_top < state.ui_state.furnace_scroll_offset {
                            state.ui_state.furnace_scroll_offset = item_top;
                        }
                    }
                }
                if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                    if state.ui_state.furnace_selected_recipe < recipe_count.saturating_sub(1) {
                        state.ui_state.furnace_selected_recipe += 1;
                        // Auto-scroll to keep selected in view
                        let row_h = 72.0_f32;
                        let item_bottom =
                            (state.ui_state.furnace_selected_recipe + 1) as f32 * row_h;
                        let (_, sh) = crate::util::virtual_screen_size();
                        let panel_h = (450.0_f32).min(sh - 16.0);
                        // 8.0 = FRAME*2, 40.0 = HEADER, 28.0 = TABS, 30.0 = FOOTER
                        let content_h = panel_h - 8.0 - 40.0 - 28.0 - 30.0 - 16.0;
                        if item_bottom > state.ui_state.furnace_scroll_offset + content_h {
                            state.ui_state.furnace_scroll_offset = item_bottom - content_h;
                        }
                    }
                }

                // Quantity keys: 1, X, A
                if is_key_pressed(KeyCode::Key1) {
                    state.ui_state.furnace_quantity = 1;
                }
                if is_key_pressed(KeyCode::X) {
                    state.ui_state.furnace_quantity = if state.ui_state.furnace_quantity == 5 {
                        10
                    } else {
                        5
                    };
                }
                if is_key_pressed(KeyCode::A) {
                    state.ui_state.furnace_quantity = u32::MAX;
                }

                // Enter or C to smelt
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::C) {
                    if let Some(recipe) =
                        furnace_recipes.get(state.ui_state.furnace_selected_recipe)
                    {
                        commands.push(InputCommand::FurnaceCraft {
                            recipe_id: recipe.id.clone(),
                            quantity: state.ui_state.furnace_quantity,
                        });
                    }
                }

                // Mouse wheel scrolling
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y != 0.0 {
                    const SCROLL_SPEED: f32 = 30.0;
                    let row_h = 72.0_f32;
                    let total_content = recipe_count as f32 * row_h;
                    let (_, sh) = crate::util::virtual_screen_size();
                    let panel_h = (450.0_f32).min(sh - 16.0);
                    // 8.0 = FRAME*2, 40.0 = HEADER, 28.0 = TABS, 30.0 = FOOTER
                    let content_h = panel_h - 8.0 - 40.0 - 28.0 - 30.0 - 16.0;
                    let max_scroll = (total_content - content_h).max(0.0);
                    state.ui_state.furnace_scroll_offset = (state.ui_state.furnace_scroll_offset
                        - wheel_y * SCROLL_SPEED)
                        .clamp(0.0, max_scroll);
                }

                // Scrollbar drag handling
                if let Some(track_bounds) = layout.get_bounds(&UiElementId::FurnaceScrollbar) {
                    let row_h = 72.0_f32;
                    let total_content = recipe_count as f32 * row_h;
                    let (_, sh) = crate::util::virtual_screen_size();
                    let panel_h = (450.0_f32).min(sh - 16.0);
                    let content_h = panel_h - 8.0 - 40.0 - 28.0 - 30.0 - 16.0;
                    let max_scroll = (total_content - content_h).max(0.0);
                    let clicked_on = matches!(clicked_element, Some(UiElementId::FurnaceScrollbar));
                    crate::ui::scroll::handle_scrollbar_drag(
                        &mut state.ui_state.furnace_scroll_drag,
                        &mut state.ui_state.furnace_scroll_offset,
                        max_scroll,
                        track_bounds,
                        total_content,
                        my,
                        is_mouse_button_down(MouseButton::Left),
                        mouse_clicked,
                        clicked_on,
                    );
                } else if !is_mouse_button_down(MouseButton::Left) {
                    state.ui_state.furnace_scroll_drag.dragging = false;
                }
            }

            // Don't process other input while furnace is open
            return commands;
        }

        // Handle anvil mode
        if state.ui_state.anvil_open {
            // Handle mouse clicks on anvil elements
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::AnvilCloseButton => {
                            state.ui_state.anvil_open = false;
                            state.ui_state.anvil_tile = None;
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            state.pending_sfx.push("enter".to_string());
                            return commands;
                        }
                        UiElementId::AnvilRecipeCell(idx) => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.anvil_selected_recipe = *idx;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return commands;
                        }
                        UiElementId::AnvilSmithButton => {
                            if !state.ui_state.crafting_in_progress {
                                let section_filter = if state.ui_state.anvil_tab == 0 {
                                    "materials"
                                } else {
                                    "equipment"
                                };
                                let mut anvil_recipes: Vec<_> = state
                                    .recipe_definitions
                                    .iter()
                                    .filter(|r| r.station.as_deref() == Some("anvil"))
                                    .filter(|r| r.section.as_deref() == Some(section_filter))
                                    .filter(|r| {
                                        !r.requires_discovery
                                            || state.discovered_recipes.contains(&r.id)
                                    })
                                    .collect();
                                anvil_recipes.sort_by_key(|r| r.level_required);
                                if let Some(recipe) =
                                    anvil_recipes.get(state.ui_state.anvil_selected_recipe)
                                {
                                    commands.push(InputCommand::AnvilCraft {
                                        recipe_id: recipe.id.clone(),
                                        quantity: state.ui_state.anvil_quantity,
                                    });
                                }
                            }
                            return commands;
                        }
                        UiElementId::AnvilCancelButton => {
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            return commands;
                        }
                        UiElementId::AnvilQuantity1 => {
                            state.ui_state.anvil_quantity = 1;
                            return commands;
                        }
                        UiElementId::AnvilQuantityX => {
                            state.ui_state.anvil_quantity = if state.ui_state.anvil_quantity == 5 {
                                10
                            } else {
                                5
                            };
                            return commands;
                        }
                        UiElementId::AnvilQuantityAll => {
                            state.ui_state.anvil_quantity = u32::MAX;
                            return commands;
                        }
                        UiElementId::AnvilTabMaterials => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.anvil_tab = 0;
                                state.ui_state.anvil_selected_recipe = 0;
                                state.ui_state.anvil_scroll_offset = 0.0;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return commands;
                        }
                        UiElementId::AnvilTabEquipment => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.anvil_tab = 1;
                                state.ui_state.anvil_selected_recipe = 0;
                                state.ui_state.anvil_scroll_offset = 0.0;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return commands;
                        }
                        _ => {}
                    }
                }
            }

            // Allow chat input while anvil panel is open
            if state.ui_state.chat_open && !state.ui_state.classic_controls {
                process_chat_keyboard_input(state, &mut commands, audio);
                return commands;
            }

            // Escape: cancel if crafting, otherwise close
            if is_key_pressed(KeyCode::Escape) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                    return commands;
                }
                state.ui_state.anvil_open = false;
                state.ui_state.anvil_tile = None;
                return commands;
            }

            // E key closes anvil
            if is_key_pressed(KeyCode::E) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                }
                state.ui_state.anvil_open = false;
                state.ui_state.anvil_tile = None;
                return commands;
            }

            if !state.ui_state.crafting_in_progress {
                // Tab key to cycle tabs
                if is_key_pressed(KeyCode::Tab) {
                    state.ui_state.anvil_tab = (state.ui_state.anvil_tab + 1) % 2;
                    state.ui_state.anvil_selected_recipe = 0;
                    state.ui_state.anvil_scroll_offset = 0.0;
                    state.pending_sfx.push("enter".to_string());
                }

                let section_filter = if state.ui_state.anvil_tab == 0 {
                    "materials"
                } else {
                    "equipment"
                };
                let columns = 4;

                // Get recipe count (drop borrow before navigation)
                let recipe_count = {
                    let mut count = 0;
                    for r in &state.recipe_definitions {
                        if r.station.as_deref() == Some("anvil")
                            && r.section.as_deref() == Some(section_filter)
                            && (!r.requires_discovery || state.discovered_recipes.contains(&r.id))
                        {
                            count += 1;
                        }
                    }
                    count
                };

                // Grid navigation: Up/Down moves by row (±columns), Left/Right moves by ±1
                if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                    if state.ui_state.anvil_selected_recipe >= columns {
                        state.ui_state.anvil_selected_recipe -= columns;
                        self.auto_scroll_anvil_grid(state);
                    }
                }
                if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                    if state.ui_state.anvil_selected_recipe + columns < recipe_count {
                        state.ui_state.anvil_selected_recipe += columns;
                        self.auto_scroll_anvil_grid(state);
                    }
                }
                if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
                    if state.ui_state.anvil_selected_recipe > 0 {
                        state.ui_state.anvil_selected_recipe -= 1;
                        self.auto_scroll_anvil_grid(state);
                    }
                }
                if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
                    if state.ui_state.anvil_selected_recipe < recipe_count.saturating_sub(1) {
                        state.ui_state.anvil_selected_recipe += 1;
                        self.auto_scroll_anvil_grid(state);
                    }
                }

                // Quantity keys: 1, X, A (not left/right since those navigate grid)
                if is_key_pressed(KeyCode::Key1) {
                    state.ui_state.anvil_quantity = 1;
                }
                if is_key_pressed(KeyCode::X) {
                    state.ui_state.anvil_quantity = if state.ui_state.anvil_quantity == 5 {
                        10
                    } else {
                        5
                    };
                }

                // Enter or C to smith
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::C) {
                    let mut anvil_recipes: Vec<_> = state
                        .recipe_definitions
                        .iter()
                        .filter(|r| r.station.as_deref() == Some("anvil"))
                        .filter(|r| r.section.as_deref() == Some(section_filter))
                        .filter(|r| {
                            !r.requires_discovery || state.discovered_recipes.contains(&r.id)
                        })
                        .collect();
                    anvil_recipes.sort_by_key(|r| r.level_required);
                    if let Some(recipe) = anvil_recipes.get(state.ui_state.anvil_selected_recipe) {
                        commands.push(InputCommand::AnvilCraft {
                            recipe_id: recipe.id.clone(),
                            quantity: state.ui_state.anvil_quantity,
                        });
                    }
                }

                // Mouse wheel scrolling
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y != 0.0 {
                    const SCROLL_SPEED: f32 = 30.0;
                    let cell_h = 106.0_f32;
                    let gap = 6.0_f32;
                    let rows = (recipe_count + columns - 1) / columns;
                    let total_content = rows as f32 * (cell_h + gap);
                    let (_, sh) = crate::util::virtual_screen_size();
                    let panel_h = (480.0_f32).min(sh - 16.0);
                    let content_h = panel_h - 8.0 - 40.0 - 28.0 - 30.0 - 16.0 - 44.0;
                    let max_scroll = (total_content - content_h).max(0.0);
                    state.ui_state.anvil_scroll_offset = (state.ui_state.anvil_scroll_offset
                        - wheel_y * SCROLL_SPEED)
                        .clamp(0.0, max_scroll);
                }

                // Scrollbar drag handling
                if let Some(track_bounds) = layout.get_bounds(&UiElementId::AnvilScrollbar) {
                    let cell_h = 106.0_f32;
                    let gap = 6.0_f32;
                    let rows = (recipe_count + columns - 1) / columns;
                    let total_content = rows as f32 * (cell_h + gap);
                    let (_, sh) = crate::util::virtual_screen_size();
                    let panel_h = (480.0_f32).min(sh - 16.0);
                    let content_h = panel_h - 8.0 - 40.0 - 28.0 - 30.0 - 16.0 - 44.0;
                    let max_scroll = (total_content - content_h).max(0.0);
                    let clicked_on = matches!(clicked_element, Some(UiElementId::AnvilScrollbar));
                    crate::ui::scroll::handle_scrollbar_drag(
                        &mut state.ui_state.anvil_scroll_drag,
                        &mut state.ui_state.anvil_scroll_offset,
                        max_scroll,
                        track_bounds,
                        total_content,
                        my,
                        is_mouse_button_down(MouseButton::Left),
                        mouse_clicked,
                        clicked_on,
                    );
                } else if !is_mouse_button_down(MouseButton::Left) {
                    state.ui_state.anvil_scroll_drag.dragging = false;
                }
            }

            // Don't process other input while anvil is open
            return commands;
        }

        // Handle alchemy station mode
        if state.ui_state.alchemy_station_open {
            // Handle mouse clicks on alchemy station elements
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::AlchemyCloseButton => {
                            state.ui_state.alchemy_station_open = false;
                            state.ui_state.alchemy_station_tile = None;
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            state.pending_sfx.push("enter".to_string());
                            return commands;
                        }
                        UiElementId::AlchemyRecipeItem(idx) => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.alchemy_station_selected_recipe = *idx;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return commands;
                        }
                        UiElementId::AlchemyBrewButton => {
                            if !state.ui_state.crafting_in_progress {
                                let tab_sections =
                                    sections_for_tab(state.ui_state.alchemy_station_tab);
                                let mut alchemy_recipes: Vec<_> = state
                                    .recipe_definitions
                                    .iter()
                                    .filter(|r| r.station.as_deref() == Some("alchemy_station"))
                                    .filter(|r| {
                                        !r.requires_discovery
                                            || state.discovered_recipes.contains(&r.id)
                                    })
                                    .filter(|r| {
                                        tab_sections.contains(&r.section.as_deref().unwrap_or(""))
                                    })
                                    .collect();
                                alchemy_recipes.sort_by(|a, b| {
                                    let sa = a.section.as_deref().unwrap_or("");
                                    let sb = b.section.as_deref().unwrap_or("");
                                    section_sort_key(sa)
                                        .cmp(&section_sort_key(sb))
                                        .then(a.level_required.cmp(&b.level_required))
                                });
                                if let Some(recipe) = alchemy_recipes
                                    .get(state.ui_state.alchemy_station_selected_recipe)
                                {
                                    commands.push(InputCommand::AlchemyCraft {
                                        recipe_id: recipe.id.clone(),
                                        quantity: state.ui_state.alchemy_station_quantity,
                                    });
                                }
                            }
                            return commands;
                        }
                        UiElementId::AlchemyCancelButton => {
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            return commands;
                        }
                        UiElementId::AlchemyQuantityMinus => {
                            if state.ui_state.alchemy_station_quantity > 1 {
                                state.ui_state.alchemy_station_quantity -= 1;
                            }
                            return commands;
                        }
                        UiElementId::AlchemyQuantityPlus => {
                            state.ui_state.alchemy_station_quantity =
                                (state.ui_state.alchemy_station_quantity + 1).min(99);
                            return commands;
                        }
                        UiElementId::AlchemyQuantityMax => {
                            let tab_sections =
                                sections_for_tab(state.ui_state.alchemy_station_tab);
                            let mut alchemy_recipes: Vec<_> = state
                                .recipe_definitions
                                .iter()
                                .filter(|r| r.station.as_deref() == Some("alchemy_station"))
                                .filter(|r| {
                                    !r.requires_discovery
                                        || state.discovered_recipes.contains(&r.id)
                                })
                                .filter(|r| {
                                    tab_sections.contains(&r.section.as_deref().unwrap_or(""))
                                })
                                .collect();
                            alchemy_recipes.sort_by(|a, b| {
                                let sa = a.section.as_deref().unwrap_or("");
                                let sb = b.section.as_deref().unwrap_or("");
                                section_sort_key(sa)
                                    .cmp(&section_sort_key(sb))
                                    .then(a.level_required.cmp(&b.level_required))
                            });
                            if let Some(recipe) = alchemy_recipes
                                .get(state.ui_state.alchemy_station_selected_recipe)
                            {
                                let mut max_possible = 99i32;
                                for ing in &recipe.ingredients {
                                    let have = state.inventory.count_item_by_id(&ing.item_id);
                                    if ing.count > 0 {
                                        max_possible = max_possible.min(have / ing.count);
                                    }
                                }
                                state.ui_state.alchemy_station_quantity =
                                    (max_possible.max(1) as u32).min(99);
                            }
                            return commands;
                        }
                        UiElementId::AlchemyTab(idx) => {
                            // Allow switching to tabs that have content (0=Potions, 1=Scrolls)
                            if (*idx == 0 || *idx == 1) && !state.ui_state.crafting_in_progress {
                                state.ui_state.alchemy_station_tab = *idx as u8;
                                state.ui_state.alchemy_station_selected_recipe = 0;
                                state.ui_state.alchemy_station_scroll_offset = 0.0;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return commands;
                        }
                        _ => {}
                    }
                }
            }

            // Allow chat input while alchemy station panel is open
            if state.ui_state.chat_open && !state.ui_state.classic_controls {
                process_chat_keyboard_input(state, &mut commands, audio);
                return commands;
            }

            // Escape: cancel if crafting, otherwise close
            if is_key_pressed(KeyCode::Escape) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                    return commands;
                }
                state.ui_state.alchemy_station_open = false;
                state.ui_state.alchemy_station_tile = None;
                return commands;
            }

            // E key closes alchemy station
            if is_key_pressed(KeyCode::E) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                }
                state.ui_state.alchemy_station_open = false;
                state.ui_state.alchemy_station_tile = None;
                return commands;
            }

            if !state.ui_state.crafting_in_progress {
                let tab_sections = sections_for_tab(state.ui_state.alchemy_station_tab);
                let mut alchemy_recipes: Vec<_> = state
                    .recipe_definitions
                    .iter()
                    .filter(|r| r.station.as_deref() == Some("alchemy_station"))
                    .filter(|r| !r.requires_discovery || state.discovered_recipes.contains(&r.id))
                    .filter(|r| tab_sections.contains(&r.section.as_deref().unwrap_or("")))
                    .collect();
                alchemy_recipes.sort_by(|a, b| {
                    let sa = a.section.as_deref().unwrap_or("");
                    let sb = b.section.as_deref().unwrap_or("");
                    section_sort_key(sa)
                        .cmp(&section_sort_key(sb))
                        .then(a.level_required.cmp(&b.level_required))
                });
                let recipe_count = alchemy_recipes.len();

                // Must match renderer layout in alchemy_station.rs
                let s = state.ui_state.ui_scale;
                let row_h = 56.0 * s;
                let section_header_h = 22.0 * s;
                let section_count = {
                    let mut sections = std::collections::HashSet::new();
                    for r in &alchemy_recipes {
                        sections.insert(r.section.as_deref().unwrap_or(""));
                    }
                    sections.len()
                };
                let total_content =
                    recipe_count as f32 * row_h + section_count as f32 * section_header_h;

                let (_, sh) = crate::util::virtual_screen_size();
                let panel_h = (520.0 * s).min(sh - 16.0);
                let header_h = 40.0 * s;
                let footer_h = 30.0 * s;
                let tab_h = 28.0 * s;
                let skill_bar_h = 24.0 * s;
                let frame = 4.0; // FRAME_THICKNESS
                                 // content_y = panel_y + frame + header_h + 2 + tab_h + 2 + skill_bar_h + 4*s
                                 // footer_y = panel_y + panel_h - frame - footer_h
                let total_content_h = panel_h
                    - frame * 2.0
                    - header_h
                    - 2.0
                    - tab_h
                    - 2.0
                    - skill_bar_h
                    - 4.0 * s
                    - 4.0 * s
                    - footer_h;

                // Dynamic detail panel height based on selected recipe's ingredient count
                let ingredient_count = alchemy_recipes
                    .get(state.ui_state.alchemy_station_selected_recipe)
                    .map(|r| r.ingredients.len())
                    .unwrap_or(1);
                let detail_h = (8.0 * s
                    + 40.0 * s
                    + 8.0 * s
                    + 6.0 * s
                    + ingredient_count as f32 * 28.0 * s
                    + 10.0 * s
                    + 26.0 * s
                    + 6.0 * s)
                    .min(total_content_h * 0.65);
                let recipe_list_h = total_content_h - detail_h - 4.0 * s;
                let max_scroll = (total_content - recipe_list_h).max(0.0);

                // W/S or Up/Down to navigate recipes
                if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                    if state.ui_state.alchemy_station_selected_recipe > 0 {
                        state.ui_state.alchemy_station_selected_recipe -= 1;
                        let item_top =
                            state.ui_state.alchemy_station_selected_recipe as f32 * row_h;
                        if item_top < state.ui_state.alchemy_station_scroll_offset {
                            state.ui_state.alchemy_station_scroll_offset = item_top;
                        }
                    }
                }
                if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                    if state.ui_state.alchemy_station_selected_recipe
                        < recipe_count.saturating_sub(1)
                    {
                        state.ui_state.alchemy_station_selected_recipe += 1;
                        let item_bottom =
                            (state.ui_state.alchemy_station_selected_recipe + 1) as f32 * row_h;
                        if item_bottom
                            > state.ui_state.alchemy_station_scroll_offset + recipe_list_h
                        {
                            state.ui_state.alchemy_station_scroll_offset =
                                item_bottom - recipe_list_h;
                        }
                    }
                }

                // +/- to adjust quantity
                if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
                    state.ui_state.alchemy_station_quantity =
                        (state.ui_state.alchemy_station_quantity + 1).min(99);
                }
                if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
                    if state.ui_state.alchemy_station_quantity > 1 {
                        state.ui_state.alchemy_station_quantity -= 1;
                    }
                }

                // Enter or C to brew
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::C) {
                    if let Some(recipe) =
                        alchemy_recipes.get(state.ui_state.alchemy_station_selected_recipe)
                    {
                        commands.push(InputCommand::AlchemyCraft {
                            recipe_id: recipe.id.clone(),
                            quantity: state.ui_state.alchemy_station_quantity,
                        });
                    }
                }

                // Mouse wheel scrolling
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y != 0.0 {
                    const SCROLL_SPEED: f32 = 30.0;
                    state.ui_state.alchemy_station_scroll_offset =
                        (state.ui_state.alchemy_station_scroll_offset - wheel_y * SCROLL_SPEED)
                            .clamp(0.0, max_scroll);
                }

                // Scrollbar drag handling
                if let Some(track_bounds) = layout.get_bounds(&UiElementId::AlchemyScrollbar) {
                    let clicked_on = matches!(clicked_element, Some(UiElementId::AlchemyScrollbar));
                    crate::ui::scroll::handle_scrollbar_drag(
                        &mut state.ui_state.alchemy_station_scroll_drag,
                        &mut state.ui_state.alchemy_station_scroll_offset,
                        max_scroll,
                        track_bounds,
                        total_content,
                        my,
                        is_mouse_button_down(MouseButton::Left),
                        mouse_clicked,
                        clicked_on,
                    );
                } else if !is_mouse_button_down(MouseButton::Left) {
                    state.ui_state.alchemy_station_scroll_drag.dragging = false;
                }
            }

            // Don't process other input while alchemy station is open
            return commands;
        }

        // ===== WORKBENCH PANEL =====
        if state.ui_state.workbench_open {
            // Handle mouse clicks on workbench elements
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::WorkbenchCloseButton => {
                            state.ui_state.workbench_open = false;
                            state.ui_state.workbench_tile = None;
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            state.pending_sfx.push("enter".to_string());
                            return commands;
                        }
                        UiElementId::WorkbenchRecipeItem(idx) => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.workbench_selected_recipe = *idx;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return commands;
                        }
                        UiElementId::WorkbenchCraftButton => {
                            if !state.ui_state.crafting_in_progress {
                                let tab_sections = crate::render::workbench_sections_for_tab(
                                    state.ui_state.workbench_tab,
                                );
                                let mut workbench_recipes: Vec<_> = state
                                    .recipe_definitions
                                    .iter()
                                    .filter(|r| r.station.as_deref() == Some("workbench"))
                                    .filter(|r| {
                                        !r.requires_discovery
                                            || state.discovered_recipes.contains(&r.id)
                                    })
                                    .filter(|r| {
                                        tab_sections.contains(&r.section.as_deref().unwrap_or(""))
                                    })
                                    .collect();
                                workbench_recipes.sort_by(|a, b| {
                                    let sa = a.section.as_deref().unwrap_or("");
                                    let sb = b.section.as_deref().unwrap_or("");
                                    section_sort_key(sa)
                                        .cmp(&section_sort_key(sb))
                                        .then(a.level_required.cmp(&b.level_required))
                                });
                                if let Some(recipe) =
                                    workbench_recipes.get(state.ui_state.workbench_selected_recipe)
                                {
                                    commands.push(InputCommand::WorkbenchCraft {
                                        recipe_id: recipe.id.clone(),
                                        quantity: state.ui_state.workbench_quantity,
                                    });
                                }
                            }
                            return commands;
                        }
                        UiElementId::WorkbenchCancelButton => {
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            return commands;
                        }
                        UiElementId::WorkbenchQuantityMinus => {
                            if state.ui_state.workbench_quantity > 1 {
                                state.ui_state.workbench_quantity -= 1;
                            }
                            return commands;
                        }
                        UiElementId::WorkbenchQuantityPlus => {
                            state.ui_state.workbench_quantity =
                                (state.ui_state.workbench_quantity + 1).min(99);
                            return commands;
                        }
                        UiElementId::WorkbenchTab(idx) => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.workbench_tab = *idx as u8;
                                state.ui_state.workbench_selected_recipe = 0;
                                state.ui_state.workbench_scroll_offset = 0.0;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return commands;
                        }
                        _ => {}
                    }
                }
            }

            // Allow chat input while workbench panel is open
            if state.ui_state.chat_open && !state.ui_state.classic_controls {
                process_chat_keyboard_input(state, &mut commands, audio);
                return commands;
            }

            // Escape: cancel if crafting, otherwise close
            if is_key_pressed(KeyCode::Escape) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                    return commands;
                }
                state.ui_state.workbench_open = false;
                state.ui_state.workbench_tile = None;
                return commands;
            }

            // E key closes workbench
            if is_key_pressed(KeyCode::E) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                }
                state.ui_state.workbench_open = false;
                state.ui_state.workbench_tile = None;
                return commands;
            }

            if !state.ui_state.crafting_in_progress {
                // Tab key to cycle tabs
                if is_key_pressed(KeyCode::Tab) {
                    state.ui_state.workbench_tab = (state.ui_state.workbench_tab + 1) % 3;
                    state.ui_state.workbench_selected_recipe = 0;
                    state.ui_state.workbench_scroll_offset = 0.0;
                    state.pending_sfx.push("enter".to_string());
                }

                let tab_sections =
                    crate::render::workbench_sections_for_tab(state.ui_state.workbench_tab);
                let mut workbench_recipes: Vec<_> = state
                    .recipe_definitions
                    .iter()
                    .filter(|r| r.station.as_deref() == Some("workbench"))
                    .filter(|r| !r.requires_discovery || state.discovered_recipes.contains(&r.id))
                    .filter(|r| tab_sections.contains(&r.section.as_deref().unwrap_or("")))
                    .collect();
                workbench_recipes.sort_by(|a, b| {
                    let sa = a.section.as_deref().unwrap_or("");
                    let sb = b.section.as_deref().unwrap_or("");
                    section_sort_key(sa)
                        .cmp(&section_sort_key(sb))
                        .then(a.level_required.cmp(&b.level_required))
                });
                let recipe_count = workbench_recipes.len();

                // Must match renderer layout in workbench.rs
                let s = state.ui_state.ui_scale;
                let row_h = 56.0 * s;
                let section_header_h = 22.0 * s;
                let section_count = {
                    let mut sections = std::collections::HashSet::new();
                    for r in &workbench_recipes {
                        sections.insert(r.section.as_deref().unwrap_or(""));
                    }
                    sections.len()
                };
                let total_content =
                    recipe_count as f32 * row_h + section_count as f32 * section_header_h;

                let (_, sh) = crate::util::virtual_screen_size();
                let panel_h = (520.0 * s).min(sh - 16.0);
                let header_h = 40.0 * s;
                let footer_h = 30.0 * s;
                let tab_h = 28.0 * s;
                let skill_bar_h = 24.0 * s;
                let frame = 4.0; // FRAME_THICKNESS
                let total_content_h = panel_h
                    - frame * 2.0
                    - header_h
                    - 2.0
                    - tab_h
                    - 2.0
                    - skill_bar_h
                    - 4.0 * s
                    - 4.0 * s
                    - footer_h;

                // Dynamic detail panel height based on selected recipe's ingredient count
                let ingredient_count = workbench_recipes
                    .get(state.ui_state.workbench_selected_recipe)
                    .map(|r| r.ingredients.len())
                    .unwrap_or(1);
                let detail_h = (8.0 * s
                    + 40.0 * s
                    + 8.0 * s
                    + 6.0 * s
                    + ingredient_count as f32 * 28.0 * s
                    + 10.0 * s
                    + 26.0 * s
                    + 6.0 * s)
                    .min(total_content_h * 0.65);
                let recipe_list_h = total_content_h - detail_h - 4.0 * s;
                let max_scroll = (total_content - recipe_list_h).max(0.0);

                // W/S or Up/Down to navigate recipes
                if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                    if state.ui_state.workbench_selected_recipe > 0 {
                        state.ui_state.workbench_selected_recipe -= 1;
                        let item_top = state.ui_state.workbench_selected_recipe as f32 * row_h;
                        if item_top < state.ui_state.workbench_scroll_offset {
                            state.ui_state.workbench_scroll_offset = item_top;
                        }
                    }
                }
                if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                    if state.ui_state.workbench_selected_recipe < recipe_count.saturating_sub(1) {
                        state.ui_state.workbench_selected_recipe += 1;
                        let item_bottom =
                            (state.ui_state.workbench_selected_recipe + 1) as f32 * row_h;
                        if item_bottom > state.ui_state.workbench_scroll_offset + recipe_list_h {
                            state.ui_state.workbench_scroll_offset = item_bottom - recipe_list_h;
                        }
                    }
                }

                // +/- to adjust quantity
                if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
                    state.ui_state.workbench_quantity =
                        (state.ui_state.workbench_quantity + 1).min(99);
                }
                if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
                    if state.ui_state.workbench_quantity > 1 {
                        state.ui_state.workbench_quantity -= 1;
                    }
                }

                // Enter or C to craft
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::C) {
                    if let Some(recipe) =
                        workbench_recipes.get(state.ui_state.workbench_selected_recipe)
                    {
                        commands.push(InputCommand::WorkbenchCraft {
                            recipe_id: recipe.id.clone(),
                            quantity: state.ui_state.workbench_quantity,
                        });
                    }
                }

                // Mouse wheel scrolling
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y != 0.0 {
                    const SCROLL_SPEED: f32 = 30.0;
                    state.ui_state.workbench_scroll_offset =
                        (state.ui_state.workbench_scroll_offset - wheel_y * SCROLL_SPEED)
                            .clamp(0.0, max_scroll);
                }

                // Scrollbar drag handling
                if let Some(track_bounds) = layout.get_bounds(&UiElementId::WorkbenchScrollbar) {
                    let clicked_on =
                        matches!(clicked_element, Some(UiElementId::WorkbenchScrollbar));
                    crate::ui::scroll::handle_scrollbar_drag(
                        &mut state.ui_state.workbench_scroll_drag,
                        &mut state.ui_state.workbench_scroll_offset,
                        max_scroll,
                        track_bounds,
                        total_content,
                        my,
                        is_mouse_button_down(MouseButton::Left),
                        mouse_clicked,
                        clicked_on,
                    );
                } else if !is_mouse_button_down(MouseButton::Left) {
                    state.ui_state.workbench_scroll_drag.dragging = false;
                }
            }

            // Don't process other input while workbench is open
            return commands;
        }

        // ===== FLETCHING PANEL (tool-based, no station) =====
        if state.ui_state.fletching_open {
            // Handle mouse clicks on fletching panel elements
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::FletchingCloseButton => {
                            state.ui_state.fletching_open = false;
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            state.pending_sfx.push("enter".to_string());
                            return commands;
                        }
                        UiElementId::FletchingRecipeItem(idx) => {
                            if !state.ui_state.crafting_in_progress {
                                state.ui_state.fletching_selected_recipe = *idx;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return commands;
                        }
                        UiElementId::FletchingFletchButton => {
                            if !state.ui_state.crafting_in_progress {
                                let tab_sections = crate::render::fletching_sections_for_tab(
                                    state.ui_state.fletching_tab,
                                );
                                let mut fletching_recipes: Vec<_> = state
                                    .recipe_definitions
                                    .iter()
                                    .filter(|r| {
                                        r.category == "fletching"
                                            && r.required_tool.as_deref() == Some("knife")
                                    })
                                    .filter(|r| {
                                        tab_sections.contains(&r.section.as_deref().unwrap_or(""))
                                    })
                                    .collect();
                                fletching_recipes.sort_by_key(|r| r.level_required);
                                if let Some(recipe) =
                                    fletching_recipes.get(state.ui_state.fletching_selected_recipe)
                                {
                                    commands.push(InputCommand::FletchingCraft {
                                        recipe_id: recipe.id.clone(),
                                        quantity: state.ui_state.fletching_quantity,
                                    });
                                }
                            }
                            return commands;
                        }
                        UiElementId::FletchingCancelButton => {
                            if state.ui_state.crafting_in_progress {
                                commands.push(InputCommand::CancelCraft);
                            }
                            return commands;
                        }
                        UiElementId::FletchingQuantity1 => {
                            state.ui_state.fletching_quantity = 1;
                            return commands;
                        }
                        UiElementId::FletchingQuantityX => {
                            state.ui_state.fletching_quantity = 5;
                            return commands;
                        }
                        UiElementId::FletchingQuantityAll => {
                            state.ui_state.fletching_quantity = u32::MAX;
                            return commands;
                        }
                        UiElementId::FletchingTab(idx) => {
                            if (*idx as u8) < 3 && !state.ui_state.crafting_in_progress {
                                state.ui_state.fletching_tab = *idx as u8;
                                state.ui_state.fletching_selected_recipe = 0;
                                state.ui_state.fletching_scroll_offset = 0.0;
                                state.pending_sfx.push("enter".to_string());
                            }
                            return commands;
                        }
                        _ => {}
                    }
                }
            }

            // Allow chat input while fletching panel is open
            if state.ui_state.chat_open && !state.ui_state.classic_controls {
                process_chat_keyboard_input(state, &mut commands, audio);
                return commands;
            }

            // Escape: cancel if crafting, otherwise close
            if is_key_pressed(KeyCode::Escape) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                    return commands;
                }
                state.ui_state.fletching_open = false;
                return commands;
            }

            // E key closes fletching panel
            if is_key_pressed(KeyCode::E) {
                if state.ui_state.crafting_in_progress {
                    commands.push(InputCommand::CancelCraft);
                }
                state.ui_state.fletching_open = false;
                return commands;
            }

            if !state.ui_state.crafting_in_progress {
                // Tab key to cycle tabs
                if is_key_pressed(KeyCode::Tab) {
                    state.ui_state.fletching_tab = (state.ui_state.fletching_tab + 1) % 3;
                    state.ui_state.fletching_selected_recipe = 0;
                    state.ui_state.fletching_scroll_offset = 0.0;
                    state.pending_sfx.push("enter".to_string());
                }

                let tab_sections =
                    crate::render::fletching_sections_for_tab(state.ui_state.fletching_tab);
                let mut fletching_recipes: Vec<_> = state
                    .recipe_definitions
                    .iter()
                    .filter(|r| {
                        r.category == "fletching" && r.required_tool.as_deref() == Some("knife")
                    })
                    .filter(|r| tab_sections.contains(&r.section.as_deref().unwrap_or("")))
                    .collect();
                fletching_recipes.sort_by_key(|r| r.level_required);
                let recipe_count = fletching_recipes.len();

                let s = state.ui_state.ui_scale;
                let row_h = 72.0 * s;
                let total_content = recipe_count as f32 * row_h;

                let (_, sh) = crate::util::virtual_screen_size();
                let panel_h = (450.0 * s).min(sh - 16.0);
                let header_h = 40.0 * s;
                let footer_h = 30.0 * s;
                let tab_h = 28.0 * s;
                let frame = 4.0;
                let content_h =
                    panel_h - frame * 2.0 - header_h - 2.0 - tab_h - 4.0 * s - footer_h - 4.0 * s;
                let max_scroll = (total_content - content_h).max(0.0);

                // W/S or Up/Down to navigate recipes
                if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                    if state.ui_state.fletching_selected_recipe > 0 {
                        state.ui_state.fletching_selected_recipe -= 1;
                        let item_top = state.ui_state.fletching_selected_recipe as f32 * row_h;
                        if item_top < state.ui_state.fletching_scroll_offset {
                            state.ui_state.fletching_scroll_offset = item_top;
                        }
                    }
                }
                if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                    if state.ui_state.fletching_selected_recipe < recipe_count.saturating_sub(1) {
                        state.ui_state.fletching_selected_recipe += 1;
                        let item_bottom =
                            (state.ui_state.fletching_selected_recipe + 1) as f32 * row_h;
                        if item_bottom > state.ui_state.fletching_scroll_offset + content_h {
                            state.ui_state.fletching_scroll_offset = item_bottom - content_h;
                        }
                    }
                }

                // 1/X/A for quantity shortcuts
                if is_key_pressed(KeyCode::Key1) {
                    state.ui_state.fletching_quantity = 1;
                }
                if is_key_pressed(KeyCode::X) {
                    state.ui_state.fletching_quantity = 5;
                }
                if is_key_pressed(KeyCode::A) {
                    state.ui_state.fletching_quantity = u32::MAX;
                }

                // Enter or C to craft
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::C) {
                    if let Some(recipe) =
                        fletching_recipes.get(state.ui_state.fletching_selected_recipe)
                    {
                        commands.push(InputCommand::FletchingCraft {
                            recipe_id: recipe.id.clone(),
                            quantity: state.ui_state.fletching_quantity,
                        });
                    }
                }

                // Mouse wheel scrolling
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y != 0.0 {
                    const SCROLL_SPEED: f32 = 30.0;
                    state.ui_state.fletching_scroll_offset =
                        (state.ui_state.fletching_scroll_offset - wheel_y * SCROLL_SPEED)
                            .clamp(0.0, max_scroll);
                }

                // Scrollbar drag handling
                if let Some(track_bounds) = layout.get_bounds(&UiElementId::FletchingScrollbar) {
                    let clicked_on =
                        matches!(clicked_element, Some(UiElementId::FletchingScrollbar));
                    crate::ui::scroll::handle_scrollbar_drag(
                        &mut state.ui_state.fletching_scroll_drag,
                        &mut state.ui_state.fletching_scroll_offset,
                        max_scroll,
                        track_bounds,
                        total_content,
                        my,
                        is_mouse_button_down(MouseButton::Left),
                        mouse_clicked,
                        clicked_on,
                    );
                } else if !is_mouse_button_down(MouseButton::Left) {
                    state.ui_state.fletching_scroll_drag.dragging = false;
                }
            }

            // Don't process other input while fletching panel is open
            return commands;
        }

        // Handle social panel touch scrolling
        if state.ui_state.social_open {
            let all_touches: Vec<Touch> = touches();

            // Handle ongoing touch drag
            if let Some(tracking_id) = state.social_state.touch_scroll_id {
                if let Some(touch) = all_touches.iter().find(|t| t.id == tracking_id) {
                    match touch.phase {
                        TouchPhase::Moved | TouchPhase::Stationary => {
                            let (_, vy) =
                                screen_to_virtual_coords(touch.position.x, touch.position.y);
                            let dy = state.social_state.touch_last_y - vy;
                            if !state.social_state.touch_dragged {
                                let total_dy = (state.social_state.touch_start_y - vy).abs();
                                if total_dy > 8.0 {
                                    state.social_state.touch_dragged = true;
                                }
                            }
                            if state.social_state.touch_dragged {
                                // Update scroll offset based on active tab
                                match state.social_state.active_tab {
                                    crate::game::SocialTab::Nearby
                                    | crate::game::SocialTab::Online => {
                                        state.social_state.list_scroll_offset =
                                            (state.social_state.list_scroll_offset + dy).max(0.0);
                                    }
                                    crate::game::SocialTab::Friends => {
                                        state.social_state.friends_scroll_offset =
                                            (state.social_state.friends_scroll_offset + dy)
                                                .max(0.0);
                                    }
                                }
                            }
                            state.social_state.touch_last_y = vy;
                        }
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            state.social_state.touch_scroll_id = None;
                        }
                        _ => {}
                    }
                } else {
                    state.social_state.touch_scroll_id = None;
                }
            } else {
                // Start new touch drag on scroll area
                for touch in &all_touches {
                    if touch.phase == TouchPhase::Started {
                        let (vx, vy) = screen_to_virtual_coords(touch.position.x, touch.position.y);
                        let hit = layout.hit_test(vx, vy);
                        if matches!(
                            hit,
                            Some(UiElementId::SocialScrollArea)
                                | Some(UiElementId::SocialPlayerRow(_))
                                | Some(UiElementId::SocialFriendRow(_))
                        ) {
                            state.social_state.touch_scroll_id = Some(touch.id);
                            state.social_state.touch_last_y = vy;
                            state.social_state.touch_start_y = vy;
                            state.social_state.touch_dragged = false;
                            break;
                        }
                    }
                }
            }

            // Handle mouse wheel scrolling
            let (_, wheel_y) = mouse_wheel();
            if wheel_y.abs() > 0.1 {
                let scroll_speed = 30.0;
                let row_height = 32.0 * state.ui_state.ui_scale; // SOCIAL_ROW_HEIGHT * scale
                let visible_h = layout
                    .get_bounds(&UiElementId::SocialScrollArea)
                    .map(|b| b.h)
                    .unwrap_or(200.0);
                match state.social_state.active_tab {
                    crate::game::SocialTab::Nearby => {
                        let count = state.social_state.nearby_players.len();
                        let max_scroll = (count as f32 * row_height - visible_h).max(0.0);
                        state.social_state.list_scroll_offset =
                            (state.social_state.list_scroll_offset - wheel_y * scroll_speed)
                                .clamp(0.0, max_scroll);
                    }
                    crate::game::SocialTab::Online => {
                        let count = state.social_state.online_players.len();
                        let max_scroll = (count as f32 * row_height - visible_h).max(0.0);
                        state.social_state.list_scroll_offset =
                            (state.social_state.list_scroll_offset - wheel_y * scroll_speed)
                                .clamp(0.0, max_scroll);
                    }
                    crate::game::SocialTab::Friends => {
                        let count = state.social_state.friends.len();
                        let max_scroll = (count as f32 * row_height - visible_h).max(0.0);
                        state.social_state.friends_scroll_offset =
                            (state.social_state.friends_scroll_offset - wheel_y * scroll_speed)
                                .clamp(0.0, max_scroll);
                    }
                }
            }
        }

        // Handle add friend input when focused
        if state.social_state.add_friend_focused && state.ui_state.social_open {
            // Escape unfocuses the input
            if is_key_pressed(KeyCode::Escape) {
                state.social_state.add_friend_focused = false;
                #[cfg(target_os = "android")]
                macroquad::miniquad::window::show_keyboard(false);
                return commands;
            }

            // Enter sends friend request
            if is_key_pressed(KeyCode::Enter) {
                let name = state.social_state.add_friend_input.trim().to_string();
                if !name.is_empty() {
                    audio.play_sfx("enter");
                    commands.push(InputCommand::SendFriendRequest { target_name: name });
                    state.social_state.add_friend_input.clear();
                }
                state.social_state.add_friend_focused = false;
                #[cfg(target_os = "android")]
                macroquad::miniquad::window::show_keyboard(false);
                return commands;
            }

            // Backspace removes last character
            if is_key_pressed(KeyCode::Backspace) {
                state.social_state.add_friend_input.pop();
            }

            // Capture typed characters
            while let Some(c) = get_char_pressed() {
                // Filter control characters
                if c.is_control()
                    || !c.is_ascii_graphic() && !c.is_ascii_whitespace() && !c.is_alphanumeric()
                {
                    continue;
                }
                // Limit input length
                if state.social_state.add_friend_input.len() < 20 {
                    state.social_state.add_friend_input.push(c);
                }
            }

            // Don't process other input while typing in add friend field
            return commands;
        }

        // Handle chat input mode (must be before chat_panel_open block so typing works)
        if state.ui_state.chat_open {
            if process_chat_keyboard_input(state, &mut commands, audio) {
                return commands;
            }
        }

        // Handle chat panel scrolling and block game-world input
        if state.ui_state.chat_panel_open {
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                const SCROLL_SPEED: f32 = 40.0; // Pixels per scroll tick
                let max_scroll = layout
                    .get_max_scroll(&UiElementId::ChatPanelScrollbar)
                    .unwrap_or(0.0);
                let delta = wheel_y * SCROLL_SPEED;
                state.ui_state.chat_message_scroll =
                    (state.ui_state.chat_message_scroll + delta).clamp(0.0, max_scroll);
            }

            // Chat panel scrollbar drag handling
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::ChatPanelScrollbar) {
                let cp_max = layout
                    .get_max_scroll(&UiElementId::ChatPanelScrollbar)
                    .unwrap_or(0.0);
                let cp_content_h = cp_max + track_bounds.h;
                let clicked_on = matches!(clicked_element, Some(UiElementId::ChatPanelScrollbar));
                crate::ui::scroll::handle_scrollbar_drag_ex(
                    &mut state.ui_state.chat_scroll_drag,
                    &mut state.ui_state.chat_message_scroll,
                    cp_max,
                    track_bounds,
                    cp_content_h,
                    my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                    true, // inverted: thumb at bottom when scroll=0
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.chat_scroll_drag.dragging = false;
            }

            return commands;
        }

        // Minimap panel is modal while open (M/Escape closes it)
        if state.ui_state.minimap_panel_open {
            if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::M) {
                audio.play_sfx("enter");
                state.ui_state.minimap_panel_open = false;
                state.ui_state.minimap_panel_dragging = false;
                return commands;
            }

            let panel_rect = minimap_panel_rect();
            let map_rect = minimap_map_rect(panel_rect);
            let over_map = mx >= map_rect.x
                && mx <= map_rect.x + map_rect.w
                && my >= map_rect.y
                && my <= map_rect.y + map_rect.h;

            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y.abs() > 0.0 && over_map {
                if let Some(world_bounds) = minimap_world_bounds(state) {
                    let old_zoom = state
                        .ui_state
                        .minimap_panel_zoom
                        .clamp(MINIMAP_PANEL_MIN_ZOOM, MINIMAP_PANEL_MAX_ZOOM);
                    let zoom_factor = (1.0 + wheel_y * 0.12).max(0.1);
                    let new_zoom = (old_zoom * zoom_factor)
                        .clamp(MINIMAP_PANEL_MIN_ZOOM, MINIMAP_PANEL_MAX_ZOOM);

                    if (new_zoom - old_zoom).abs() > f32::EPSILON {
                        let view_bounds = minimap_panel_view_bounds(state, world_bounds);
                        let anchor_world = minimap_screen_to_world(view_bounds, map_rect, mx, my);
                        let (new_view_w, new_view_h) = minimap_view_size(world_bounds, new_zoom);
                        let nx = ((mx - map_rect.x) / map_rect.w.max(1.0)).clamp(0.0, 1.0);
                        let ny = ((my - map_rect.y) / map_rect.h.max(1.0)).clamp(0.0, 1.0);
                        let target_center_x = anchor_world.0 - (nx - 0.5) * new_view_w;
                        let target_center_y = anchor_world.1 - (ny - 0.5) * new_view_h;
                        let (center_x, center_y) = minimap_clamp_center(
                            world_bounds,
                            new_view_w,
                            new_view_h,
                            target_center_x,
                            target_center_y,
                        );

                        state.ui_state.minimap_panel_zoom = new_zoom;
                        state.ui_state.minimap_panel_center_x = Some(center_x);
                        state.ui_state.minimap_panel_center_y = Some(center_y);
                    }
                }
            }

            if mouse_clicked && over_map {
                state.ui_state.minimap_panel_dragging = true;
                state.ui_state.minimap_panel_drag_last_x = mx;
                state.ui_state.minimap_panel_drag_last_y = my;
            }

            if state.ui_state.minimap_panel_dragging {
                if is_mouse_button_down(MouseButton::Left) {
                    if let Some(world_bounds) = minimap_world_bounds(state) {
                        let view_bounds = minimap_panel_view_bounds(state, world_bounds);
                        let dx_pixels = mx - state.ui_state.minimap_panel_drag_last_x;
                        let dy_pixels = my - state.ui_state.minimap_panel_drag_last_y;

                        if dx_pixels.abs() > 0.0 || dy_pixels.abs() > 0.0 {
                            let view_w = view_bounds.width();
                            let view_h = view_bounds.height();
                            let world_dx = dx_pixels / map_rect.w.max(1.0) * view_w;
                            let world_dy = dy_pixels / map_rect.h.max(1.0) * view_h;
                            let center_x = (view_bounds.min_x + view_bounds.max_x) * 0.5 - world_dx;
                            let center_y = (view_bounds.min_y + view_bounds.max_y) * 0.5 - world_dy;
                            let (center_x, center_y) = minimap_clamp_center(
                                world_bounds,
                                view_w,
                                view_h,
                                center_x,
                                center_y,
                            );
                            state.ui_state.minimap_panel_center_x = Some(center_x);
                            state.ui_state.minimap_panel_center_y = Some(center_y);
                        }
                    }
                    state.ui_state.minimap_panel_drag_last_x = mx;
                    state.ui_state.minimap_panel_drag_last_y = my;
                } else {
                    state.ui_state.minimap_panel_dragging = false;
                }
            }

            return commands;
        }

        let classic = state.ui_state.classic_controls;

        // Enter key opens chat (not in classic mode - chat is always open)
        // Don't open chat on System tab (read-only)
        if !classic
            && is_key_pressed(KeyCode::Enter)
            && !matches!(state.ui_state.chat_active_tab, ChatChannel::System)
        {
            state.ui_state.chat_open = true;
            state.ui_state.chat_input.clear();
            state.ui_state.chat_cursor = 0;
            state.ui_state.chat_scroll_offset = 0;
            // Drain any accumulated characters from the queue
            while get_char_pressed().is_some() {}
            return commands;
        }

        // Drain character queue when chat is closed to prevent accumulation
        while get_char_pressed().is_some() {}

        // Read which keys are held (in classic mode, only arrow keys - WASD goes to chat)
        let up = if classic {
            is_key_down(KeyCode::Up)
        } else {
            is_key_down(KeyCode::W) || is_key_down(KeyCode::Up)
        };
        let down = if classic {
            is_key_down(KeyCode::Down)
        } else {
            is_key_down(KeyCode::S) || is_key_down(KeyCode::Down)
        };
        let left = if classic {
            is_key_down(KeyCode::Left)
        } else {
            is_key_down(KeyCode::A) || is_key_down(KeyCode::Left)
        };
        let right = if classic {
            is_key_down(KeyCode::Right)
        } else {
            is_key_down(KeyCode::D) || is_key_down(KeyCode::Right)
        };

        // Check for newly pressed keys this frame (last-key-wins priority)
        let up_just = if classic {
            is_key_pressed(KeyCode::Up)
        } else {
            is_key_pressed(KeyCode::W) || is_key_pressed(KeyCode::Up)
        };
        let down_just = if classic {
            is_key_pressed(KeyCode::Down)
        } else {
            is_key_pressed(KeyCode::S) || is_key_pressed(KeyCode::Down)
        };
        let left_just = if classic {
            is_key_pressed(KeyCode::Left)
        } else {
            is_key_pressed(KeyCode::A) || is_key_pressed(KeyCode::Left)
        };
        let right_just = if classic {
            is_key_pressed(KeyCode::Right)
        } else {
            is_key_pressed(KeyCode::D) || is_key_pressed(KeyCode::Right)
        };

        // Get touch D-pad input (for mobile)
        use crate::input::touch::DPadDirection;
        let dpad_dir = self.touch_controls.get_direction();
        let dpad_released = self.touch_controls.get_just_released_direction();
        let has_dpad_input = dpad_dir != DPadDirection::None;

        // Cancel auto-path if any movement input (keyboard or D-pad)
        if up || down || left || right || has_dpad_input {
            state.clear_auto_path();
            self.reset_auto_path_motion_state();
        }

        // Determine new direction from keyboard - only one direction at a time
        // Newly pressed keys override current direction (last-key-wins),
        // then keep current direction if still held, then fall back to any held key
        let keyboard_dir = if up_just {
            CardinalDir::Up
        } else if down_just {
            CardinalDir::Down
        } else if left_just {
            CardinalDir::Left
        } else if right_just {
            CardinalDir::Right
        } else {
            match self.current_dir {
                CardinalDir::Up if up => CardinalDir::Up,
                CardinalDir::Down if down => CardinalDir::Down,
                CardinalDir::Left if left => CardinalDir::Left,
                CardinalDir::Right if right => CardinalDir::Right,
                _ => {
                    if up {
                        CardinalDir::Up
                    } else if down {
                        CardinalDir::Down
                    } else if left {
                        CardinalDir::Left
                    } else if right {
                        CardinalDir::Right
                    } else {
                        CardinalDir::None
                    }
                }
            }
        };

        // Combine keyboard and D-pad: D-pad takes priority if active
        let new_dir = if has_dpad_input {
            match dpad_dir {
                DPadDirection::Up => CardinalDir::Up,
                DPadDirection::Down => CardinalDir::Down,
                DPadDirection::Left => CardinalDir::Left,
                DPadDirection::Right => CardinalDir::Right,
                DPadDirection::None => keyboard_dir,
            }
        } else {
            keyboard_dir
        };

        // Detect direction changes for face vs move logic (keyboard only - D-pad has its own tracking)
        let dir_changed = keyboard_dir != self.prev_dir;

        // Handle keyboard direction key press/release for face vs move
        if dir_changed && !has_dpad_input {
            if keyboard_dir != CardinalDir::None && self.prev_dir == CardinalDir::None {
                // New direction pressed - record time
                self.dir_press_time = current_time;
                self.move_sent = false;
            } else if keyboard_dir == CardinalDir::None && self.prev_dir != CardinalDir::None {
                // Direction released
                if self.move_sent {
                    // Was moving, now stopped - send stop command
                    commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                    self.last_dx = 0.0;
                    self.last_dy = 0.0;
                    self.last_send_time = current_time;
                    self.move_sent = false;
                } else {
                    // Never sent a move (quick tap or frame timing edge case) - send Face command
                    // But not if attacking - player must finish attack first
                    let attack_anim = state.get_local_player().map_or(false, |p| {
                        matches!(
                            p.animation.state,
                            AnimationState::Attacking
                                | AnimationState::Casting
                                | AnimationState::ShootingBow
                        )
                    });
                    if !attack_anim && !state.is_sitting {
                        let dir = self.prev_dir.to_direction_u8();
                        queue_face(state, &mut commands, dir);
                        self.last_send_time = current_time;
                    }
                }
            } else if keyboard_dir != CardinalDir::None && self.prev_dir != CardinalDir::None {
                // Direction changed while holding
                if self.move_sent {
                    // Already moving - continue moving in new direction immediately (no threshold wait)
                    // move_sent stays true, don't reset dir_press_time
                } else {
                    // Wasn't moving yet (still in threshold wait) - restart timer for new direction
                    self.dir_press_time = current_time;
                }
            }
        }

        // Handle D-pad release for tap-to-face
        // Use a longer window for tap detection on release - even if movement started,
        // a quick release (under 300ms total) is treated as a face-only tap.
        const TAP_RELEASE_WINDOW: f64 = 0.30; // 300ms
        if dpad_released != DPadDirection::None {
            let hold_duration = current_time - self.touch_controls.get_dpad_press_time();
            let was_short_tap = hold_duration < TAP_RELEASE_WINDOW;

            if was_short_tap {
                // Short tap - send stop if we were moving, then send Face
                if self.touch_controls.was_dpad_move_sent() {
                    commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                }
                let attack_anim = state.get_local_player().map_or(false, |p| {
                    matches!(
                        p.animation.state,
                        AnimationState::Attacking
                            | AnimationState::Casting
                            | AnimationState::ShootingBow
                    )
                });
                if !attack_anim && !state.is_sitting {
                    let dir = dpad_released.to_direction_u8();
                    queue_face(state, &mut commands, dir);
                    self.last_send_time = current_time;
                }
            } else if self.touch_controls.was_dpad_move_sent() {
                // Long hold that was moving - send stop command
                commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
            }
            self.last_dx = 0.0;
            self.last_dy = 0.0;
            self.last_send_time = current_time;
            self.move_sent = false;
            self.touch_controls.set_dpad_move_sent(false);
        }

        self.prev_dir = keyboard_dir;
        self.current_dir = keyboard_dir;

        // Convert direction to velocity
        let (dx, dy): (f32, f32) = match new_dir {
            CardinalDir::Up => (0.0, -1.0),
            CardinalDir::Down => (0.0, 1.0),
            CardinalDir::Left => (-1.0, 0.0),
            CardinalDir::Right => (1.0, 0.0),
            CardinalDir::None => (0.0, 0.0),
        };

        // Only send Move commands if held past the threshold
        // Don't move while attacking - check both attack key/touch button and animation state
        let attack_key_down = if classic {
            is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl)
        } else {
            is_key_down(KeyCode::Space)
        };
        let is_attacking = attack_key_down
            || self.touch_controls.attack_pressed()
            || state.get_local_player().map_or(false, |p| {
                matches!(
                    p.animation.state,
                    AnimationState::Attacking
                        | AnimationState::Casting
                        | AnimationState::ShootingBow
                )
            });

        // Check if we have any movement input (keyboard or D-pad)
        let has_movement_input = new_dir != CardinalDir::None;

        // Movement while sitting is handled server-side (direction-validated auto-stand)
        // Just let the move command go through - server will stand up if direction matches

        if has_movement_input && !is_attacking {
            // Cancel auto-action and follow when player manually moves
            if state.auto_action_state.is_some() {
                state.auto_action_state = None;
                state.auto_path = None;
                commands.push(InputCommand::CancelAutoAction);
            }
            if state.follow_target.is_some() {
                state.follow_target = None;
                state.follow_arrived_target_pos = None;
                state.follow_target_move_time = 0.0;
                state.auto_path = None;
            }

            // Determine hold duration based on input source
            let hold_duration = if has_dpad_input {
                current_time - self.touch_controls.get_dpad_press_time()
            } else {
                current_time - self.dir_press_time
            };
            let past_threshold = hold_duration >= FACE_THRESHOLD;

            if past_threshold {
                let direction_changed =
                    (dx - self.last_dx).abs() > 0.01 || (dy - self.last_dy).abs() > 0.01;
                let time_elapsed = current_time - self.last_send_time >= self.send_interval;
                let should_send = direction_changed || time_elapsed;

                if should_send {
                    // When sitting, only allow movement in the chair's facing direction (to stand up)
                    // Otherwise only gate by static tile walkability and let server handle dynamic collisions.
                    let can_move = if state.is_sitting {
                        if let Some(player) = state.get_local_player() {
                            let move_dir = new_dir.to_direction_u8();
                            let chair_dir = player.direction as u8;
                            move_dir == chair_dir
                        } else {
                            false
                        }
                    } else if let Some(player) = state.get_local_player() {
                        let player_x = player.server_x.round() as i32;
                        let player_y = player.server_y.round() as i32;
                        let target_x = player_x + dx as i32;
                        let target_y = player_y + dy as i32;
                        let tile_walkable = state
                            .chunk_manager
                            .is_walkable(target_x as f32, target_y as f32);
                        let occupied = build_occupied_set(state, false);
                        let not_occupied = !occupied.contains(&(target_x, target_y));
                        tile_walkable && not_occupied
                    } else {
                        false
                    };

                    if can_move {
                        commands.push(InputCommand::Move { dx, dy });
                        self.last_dx = dx;
                        self.last_dy = dy;
                        self.last_send_time = current_time;
                        self.move_sent = true;
                        // Close context menu when player starts moving
                        state.ui_state.context_menu = None;
                        // Also track D-pad move sent
                        if has_dpad_input {
                            self.touch_controls.set_dpad_move_sent(true);
                        }
                    } else {
                        // Can't move - face that direction instead
                        if self.move_sent || self.touch_controls.was_dpad_move_sent() {
                            // Was moving, send stop
                            commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                            self.move_sent = false;
                            self.touch_controls.set_dpad_move_sent(false);
                        }
                        if !state.is_sitting {
                            let face_dir = new_dir.to_direction_u8();
                            queue_face(state, &mut commands, face_dir);
                            self.last_dx = dx;
                            self.last_dy = dy;
                            self.last_send_time = current_time;
                        }
                    }
                }
            }
        }

        // Handle keyboard release when D-pad not active - send stop command
        if !has_dpad_input && keyboard_dir == CardinalDir::None && self.move_sent {
            // Already handled above in dir_changed block
        }

        // Dash: left shift while moving
        if is_key_pressed(KeyCode::LeftShift) {
            let is_moving = self.last_dx != 0.0 || self.last_dy != 0.0;
            if is_moving && current_time >= state.dash_cooldown_end {
                commands.push(InputCommand::Dash);
                state.dash_cooldown_end = current_time + 3.0; // 3 second cooldown
            }
        }

        // Get player position from SERVER state (not visual) to avoid getting ahead of server
        let player_pos = state
            .get_local_player()
            .map(|p| (p.server_x.round() as i32, p.server_y.round() as i32));

        // Chase / auto-action re-pathfinding: if auto-action is active,
        // ensure we have a valid path or are adjacent to the target.
        // Handles NPCs, players, AND resources (trees/rocks).
        // This runs even during attack animations so chase can recover immediately.
        if dx == 0.0 && dy == 0.0 {
            if let (Some(ref aa), Some((player_x, player_y))) =
                (&state.auto_action_state, player_pos)
            {
                let target_pos: Option<(i32, i32)> = auto_action_target_pos(aa, state)
                    .map(|(x, y)| (x.round() as i32, y.round() as i32));

                // Check if target still exists (NPC/player could have died/disconnected)
                let target_gone = match aa.target_type.as_str() {
                    "npc" => !state.npcs.contains_key(&aa.target_id),
                    "player" => !state.players.contains_key(&aa.target_id),
                    _ => false, // resources don't disappear mid-chase (depletion handled by server)
                };
                if target_gone {
                    state.auto_action_state = None;
                    state.auto_path = None;
                }

                if let Some((tx, ty)) = target_pos {
                    let weapon_range = get_local_weapon_range(state);
                    let is_in_range = in_attack_range(player_x, player_y, tx, ty, weapon_range);

                    // If already in range and auto-action not yet sent, send now
                    if is_in_range {
                        let auto_action_data = state.auto_action_state.as_ref().map(|aa| {
                            (
                                aa.confirmed,
                                auto_action_target_settled(aa, state),
                                aa.target_type.clone(),
                                aa.target_id.clone(),
                                aa.action.clone(),
                            )
                        });

                        if auto_action_data.is_some() {
                            // Face the target while in range, but skip during attack
                            // animations — the playerAttack message already set the
                            // authoritative direction and re-computing from visual
                            // positions causes rapid flip-flop on diagonal angles.
                            let in_attack_anim =
                                state.get_local_player().map_or(false, |p| {
                                    matches!(
                                        p.animation.state,
                                        AnimationState::Attacking
                                            | AnimationState::Casting
                                            | AnimationState::ShootingBow
                                    )
                                });
                            if !in_attack_anim {
                                // Use server (grid) positions for both player and target
                                // to match the server's direction computation and avoid
                                // jitter from visual interpolation.
                                let face_delta = state.get_local_player().map(|player| {
                                    (
                                        tx as f32 - player.server_x.round(),
                                        ty as f32 - player.server_y.round(),
                                    )
                                });
                                if let Some((dx, dy)) = face_delta {
                                    face_target_if_needed(state, &mut commands, dx, dy);
                                }
                            }
                        }

                        if let Some((confirmed, settled, target_type, target_id, action)) =
                            auto_action_data
                        {
                            if !confirmed && settled {
                                commands.push(InputCommand::StartAutoAction {
                                    target_type,
                                    target_id,
                                    action,
                                });
                                state.auto_path = None;
                            }
                        }
                    } else {
                        // Not in range — chase toward target.
                        // Clear any stale auto_path if we just entered range on a
                        // previous tick but target moved back out.
                        {
                            let needs_repath = if let Some(ref path_state) = state.auto_path {
                                // Destination no longer close to the target (target moved).
                                // Use a tolerance of 2 so we don't re-path every time the
                                // NPC moves a single tile — finish the current path first.
                                let dest_dist = (path_state.destination.0 - tx).abs()
                                    + (path_state.destination.1 - ty).abs();
                                dest_dist > 2
                            } else {
                                // No path at all — need one
                                true
                            };

                            // Throttle re-pathing to at most once per 300ms to prevent
                            // jerky direction changes when chasing moving targets.
                            const REPATH_COOLDOWN: f64 = 0.3;
                            let repath_allowed =
                                current_time - state.last_chase_repath_time >= REPATH_COOLDOWN;

                            if needs_repath && repath_allowed {
                                // Exclude chase target from occupied set so the target
                                // doesn't block our path when it moves onto our route.
                                let mut occupied = build_occupied_set(state, true);
                                if let Some(ref aa) = state.auto_action_state {
                                    match aa.target_type.as_str() {
                                        "npc" => {
                                            if let Some(npc) = state.npcs.get(&aa.target_id) {
                                                occupied.remove(&(
                                                    npc.server_x.round() as i32,
                                                    npc.server_y.round() as i32,
                                                ));
                                            }
                                        }
                                        "player" => {
                                            if let Some(p) = state.players.get(&aa.target_id) {
                                                occupied.remove(&(
                                                    p.server_x.round() as i32,
                                                    p.server_y.round() as i32,
                                                ));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                const MAX_PATH_DISTANCE: i32 = 32;
                                let path_result = if weapon_range > 1 {
                                    find_path_to_attack_with_optimistic_splice(
                                        state,
                                        (player_x, player_y),
                                        (tx, ty),
                                        &occupied,
                                        MAX_PATH_DISTANCE,
                                        weapon_range,
                                    )
                                } else {
                                    let preferred = preferred_adjacent_tile_for_target(state, (tx, ty));
                                    find_path_to_adjacent_with_optimistic_splice(
                                        state,
                                        (player_x, player_y),
                                        (tx, ty),
                                        &occupied,
                                        MAX_PATH_DISTANCE,
                                        preferred,
                                    )
                                };
                                if let Some((dest, path)) = path_result {
                                    state.auto_path = Some(PathState {
                                        path,
                                        current_index: 0,
                                        destination: dest,
                                        pickup_target: None,
                                        interact_target: None,
                                        interact_object_target: None,
                                        waystone_target: None,
                                        browse_stall_target: None,
                                    });
                                    state.last_chase_repath_time = current_time;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Follow target re-pathing — continuously follow another player
        // Cancel follow if player started attacking or performing an auto-action
        if state.follow_target.is_some() && state.auto_action_state.is_some() {
            state.follow_target = None;
            state.follow_arrived_target_pos = None;
            state.follow_target_move_time = 0.0;
        }
        if let Some(ref follow_id) = state.follow_target.clone() {
            if let Some(target) = state.players.get(follow_id) {
                let tx = target.server_x.round() as i32;
                let ty = target.server_y.round() as i32;

                if let Some(player) = state.get_local_player() {
                    let px = player.server_x.round() as i32;
                    let py = player.server_y.round() as i32;
                    let dist = (px - tx).abs() + (py - ty).abs();

                    if dist <= 1 {
                        // Adjacent — stop and enter waiting state
                        if state.auto_path.is_some() {
                            state.auto_path = None;
                        }
                        // Record target position so we know when they move
                        if state.follow_arrived_target_pos.is_none() {
                            state.follow_arrived_target_pos = Some((tx, ty));
                            state.follow_target_move_time = 0.0;
                        }
                    } else if let Some((ax, ay)) = state.follow_arrived_target_pos {
                        // We were adjacent but now dist > 1 — target moved away
                        if (tx, ty) == (ax, ay) {
                            // Target hasn't actually moved, we drifted — just re-path immediately
                            state.follow_arrived_target_pos = None;
                            state.follow_target_move_time = 0.0;
                        } else {
                            // Target moved — wait 500ms before following
                            if state.follow_target_move_time == 0.0 {
                                state.follow_target_move_time = current_time;
                            }
                            const FOLLOW_MOVE_DELAY: f64 = 0.5;
                            if current_time - state.follow_target_move_time >= FOLLOW_MOVE_DELAY {
                                // Delay elapsed — clear waiting state and path to target
                                state.follow_arrived_target_pos = None;
                                state.follow_target_move_time = 0.0;
                                let mut occupied = build_occupied_set(state, true);
                                if let Some(p) = state.players.get(follow_id) {
                                    occupied.remove(&(
                                        p.server_x.round() as i32,
                                        p.server_y.round() as i32,
                                    ));
                                }
                                const MAX_PATH_DISTANCE: i32 = 32;
                                if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                    (px, py),
                                    (tx, ty),
                                    &state.chunk_manager,
                                    &occupied,
                                    MAX_PATH_DISTANCE,
                                ) {
                                    state.auto_path = Some(PathState {
                                        path,
                                        current_index: 0,
                                        destination: dest,
                                        pickup_target: None,
                                        interact_target: None,
                                        interact_object_target: None,
                                        waystone_target: None,
                                        browse_stall_target: None,
                                    });
                                    state.last_chase_repath_time = current_time;
                                }
                            }
                        }
                    } else {
                        // Not adjacent and not in waiting state — normal follow re-pathing
                        let needs_repath = if let Some(ref path_state) = state.auto_path {
                            let dest_dist = (path_state.destination.0 - tx).abs()
                                + (path_state.destination.1 - ty).abs();
                            dest_dist > 2
                        } else {
                            true
                        };

                        const REPATH_COOLDOWN: f64 = 0.6;
                        let repath_allowed =
                            current_time - state.last_chase_repath_time >= REPATH_COOLDOWN;

                        if needs_repath && repath_allowed {
                            let mut occupied = build_occupied_set(state, true);
                            if let Some(p) = state.players.get(follow_id) {
                                occupied.remove(&(
                                    p.server_x.round() as i32,
                                    p.server_y.round() as i32,
                                ));
                            }
                            const MAX_PATH_DISTANCE: i32 = 32;
                            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                (px, py),
                                (tx, ty),
                                &state.chunk_manager,
                                &occupied,
                                MAX_PATH_DISTANCE,
                            ) {
                                state.auto_path = Some(PathState {
                                    path,
                                    current_index: 0,
                                    destination: dest,
                                    pickup_target: None,
                                    interact_target: None,
                                    interact_object_target: None,
                                    waystone_target: None,
                                    browse_stall_target: None,
                                });
                                state.last_chase_repath_time = current_time;
                            }
                        }
                    }
                }
            } else {
                // Target player disconnected or left view — stop following
                state.follow_target = None;
                state.follow_arrived_target_pos = None;
                state.follow_target_move_time = 0.0;
            }
        }

        if state.auto_path.is_none() {
            self.reset_auto_path_motion_state();
        }

        // Path following - generate movement commands when auto-pathing
        // Only follow path if not manually moving and not attacking
        if dx == 0.0 && dy == 0.0 && !is_attacking {
            if let (Some((player_x, player_y)), Some(ref mut path_state)) =
                (player_pos, &mut state.auto_path)
            {
                sync_path_index(path_state, (player_x, player_y));
            }

            // Check if next waypoint is blocked by an entity - if so, cancel path.
            // When chasing a target, exclude that target from the blocked check so
            // the target moving onto our path doesn't cause constant re-pathing.
            let mut path_blocked = false;
            if let (Some((player_x, player_y)), Some(ref path_state)) =
                (player_pos, &state.auto_path)
            {
                if path_state.current_index < path_state.path.len() {
                    let (next_x, next_y) = path_state.path[path_state.current_index];
                    if player_x != next_x || player_y != next_y {
                        let mut occupied = build_occupied_set(state, true);
                        // Exclude chase/follow target from blocked check
                        if let Some(ref aa) = state.auto_action_state {
                            match aa.target_type.as_str() {
                                "npc" => {
                                    if let Some(npc) = state.npcs.get(&aa.target_id) {
                                        occupied.remove(&(
                                            npc.server_x.round() as i32,
                                            npc.server_y.round() as i32,
                                        ));
                                    }
                                }
                                "player" => {
                                    if let Some(p) = state.players.get(&aa.target_id) {
                                        occupied.remove(&(
                                            p.server_x.round() as i32,
                                            p.server_y.round() as i32,
                                        ));
                                    }
                                }
                                _ => {}
                            }
                        }
                        if let Some(ref fid) = state.follow_target {
                            if let Some(p) = state.players.get(fid) {
                                occupied.remove(&(
                                    p.server_x.round() as i32,
                                    p.server_y.round() as i32,
                                ));
                            }
                        }
                        if occupied.contains(&(next_x, next_y)) {
                            path_blocked = true;
                        }
                    }
                }
            }

            if path_blocked {
                self.reset_auto_path_motion_state();
                if !rebuild_current_auto_path(state) {
                    if state.auto_action_state.is_some() || state.follow_target.is_some() {
                        // Clear the blocked path — chase/follow re-path will recalculate next frame
                        state.auto_path = None;
                        self.reset_auto_path_motion_state();
                    } else {
                        state.auto_path = None;
                        self.reset_auto_path_motion_state();
                        commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                        return commands;
                    }
                }
            }

            if let (Some((player_x, player_y)), Some(ref mut path_state)) =
                (player_pos, &mut state.auto_path)
            {
                // Auto-path is step-driven: send one move when a new waypoint becomes active.
                if path_state.current_index < path_state.path.len() {
                    let (next_x, next_y) = path_state.path[path_state.current_index];
                    let move_dx = (next_x - player_x).signum() as f32;
                    let move_dy = (next_y - player_y).signum() as f32;

                    let desired_dir = if move_dx != 0.0 {
                        Some((move_dx, 0.0))
                    } else if move_dy != 0.0 {
                        Some((0.0, move_dy))
                    } else {
                        None
                    };

                    if let Some((send_dx, send_dy)) = desired_dir {
                        let waypoint_changed =
                            self.auto_path_sent_waypoint != Some((next_x, next_y));
                        let dir_changed = self.auto_path_sent_dir != Some((send_dx, send_dy));

                        if waypoint_changed || dir_changed {
                            commands.push(InputCommand::Move {
                                dx: send_dx,
                                dy: send_dy,
                            });
                            self.auto_path_sent_waypoint = Some((next_x, next_y));
                            self.auto_path_sent_dir = Some((send_dx, send_dy));
                        }
                    }
                } else {
                    self.reset_auto_path_motion_state();
                }
            }

            // Check if path completed and handle pickup/interact if needed
            if state
                .auto_path
                .as_ref()
                .map(|p| p.current_index >= p.path.len())
                .unwrap_or(false)
            {
                // Path completed - check for pickup target
                if let Some(ref path_state) = state.auto_path {
                    if let Some(ref item_id) = path_state.pickup_target {
                        commands.push(InputCommand::Pickup {
                            item_id: item_id.clone(),
                        });
                    }
                    // Handle interact target (NPC)
                    if let Some(ref npc_id) = path_state.interact_target {
                        // Check if target is an altar or station
                        if let Some(npc) = state.npcs.get(npc_id) {
                            if npc.is_altar {
                                state.ui_state.altar_panel = Some(crate::game::AltarPanelState {
                                    altar_npc_id: npc_id.clone(),
                                    altar_name: npc.display_name.clone(),
                                });
                            } else if npc.station_type.as_deref() == Some("furnace")
                                || npc.station_type.as_deref() == Some("fire_pit")
                            {
                                state.ui_state.furnace_station_type =
                                    npc.station_type.clone().unwrap_or_default();
                                state.ui_state.fletching_open = false;
                                state.ui_state.workbench_open = false;
                                state.ui_state.furnace_open = true;
                                state.ui_state.furnace_tile =
                                    Some((npc.x.round() as i32, npc.y.round() as i32));
                                state.ui_state.furnace_selected_recipe = 0;
                                state.ui_state.furnace_scroll_offset = 0.0;
                                state.ui_state.furnace_quantity = 1;
                                state.ui_state.furnace_tab = 0;
                            } else if npc.station_type.as_deref() == Some("anvil") {
                                state.ui_state.fletching_open = false;
                                state.ui_state.workbench_open = false;
                                state.ui_state.anvil_open = true;
                                state.ui_state.anvil_tile =
                                    Some((npc.x.round() as i32, npc.y.round() as i32));
                                state.ui_state.anvil_selected_recipe = 0;
                                state.ui_state.anvil_scroll_offset = 0.0;
                                state.ui_state.anvil_quantity = 1;
                                state.ui_state.anvil_tab = 0;
                            } else if npc.station_type.as_deref() == Some("alchemy_station") {
                                state.ui_state.fletching_open = false;
                                state.ui_state.workbench_open = false;
                                state.ui_state.alchemy_station_open = true;
                                state.ui_state.alchemy_station_tile =
                                    Some((npc.x.round() as i32, npc.y.round() as i32));
                                state.ui_state.alchemy_station_selected_recipe = 0;
                                state.ui_state.alchemy_station_scroll_offset = 0.0;
                                state.ui_state.alchemy_station_quantity = 1;
                                state.ui_state.alchemy_station_tab = 0;
                            } else if npc.station_type.as_deref() == Some("workbench") {
                                state.ui_state.fletching_open = false;
                                state.ui_state.alchemy_station_open = false;
                                state.ui_state.workbench_open = true;
                                state.ui_state.workbench_tile =
                                    Some((npc.x.round() as i32, npc.y.round() as i32));
                                state.ui_state.workbench_selected_recipe = 0;
                                state.ui_state.workbench_scroll_offset = 0.0;
                                state.ui_state.workbench_quantity = 1;
                                state.ui_state.workbench_tab = 0;
                            } else if npc.is_alive() {
                                commands.push(InputCommand::Interact {
                                    npc_id: npc_id.clone(),
                                });
                            }
                        } else {
                            commands.push(InputCommand::Interact {
                                npc_id: npc_id.clone(),
                            });
                        }
                    }
                    // Handle interact object target (map objects like obelisks)
                    if let Some((obj_x, obj_y)) = path_state.interact_object_target {
                        commands.push(InputCommand::InteractObject { x: obj_x, y: obj_y });
                    }
                    // Handle direct waystone teleport (right-click Teleport)
                    if let Some((ws_x, ws_y)) = path_state.waystone_target {
                        commands.push(InputCommand::UseWaystone { x: ws_x, y: ws_y });
                    }
                    // Handle browse stall target (left-click player with stall)
                    if let Some(ref player_id) = path_state.browse_stall_target {
                        commands.push(InputCommand::StallBrowse {
                            player_id: player_id.clone(),
                        });
                    }
                }
                // Handle chair sit target
                if let Some((cx, cy)) = state.pending_chair_sit.take() {
                    commands.push(InputCommand::SitChair {
                        tile_x: cx,
                        tile_y: cy,
                    });
                }
                // Handle farming harvest target
                if let Some(patch_id) = state.pending_harvest_patch.take() {
                    commands.push(InputCommand::HarvestCrop { patch_id });
                }
                // Handle auto-action: send StartAutoAction now that we've arrived
                let auto_action_snapshot = state.auto_action_state.as_ref().map(|aa| {
                    (
                        aa.confirmed,
                        auto_action_target_settled(aa, state),
                        aa.target_type.clone(),
                        aa.target_id.clone(),
                        aa.action.clone(),
                        auto_action_target_pos(aa, state),
                    )
                });

                if let Some((confirmed, settled, target_type, target_id, action, target_pos)) =
                    auto_action_snapshot
                {
                    // Always face the target when we reach the destination.
                    if let Some((tx, ty)) = target_pos {
                        if let Some(local_id) = &state.local_player_id {
                            if let Some(player) = state.players.get(local_id) {
                                let dx = tx - player.x;
                                let dy = ty - player.y;
                                face_target_if_needed(state, &mut commands, dx, dy);
                            }
                        }
                    }

                    if !confirmed && settled {
                        commands.push(InputCommand::StartAutoAction {
                            target_type,
                            target_id,
                            action,
                        });
                    }
                }
                state.auto_path = None;
                self.reset_auto_path_motion_state();

                // Send stop command so we don't keep moving in the last direction
                // (but not during auto-action — that would interrupt it on the server)
                if state.auto_action_state.is_none() {
                    commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                }
            }
        }

        // Attack (Space key or touch attack button) - holding continues attacking with cooldown
        // If fishing rod equipped and on/near a fishing tile, start gathering instead
        // Also stop movement when attacking (player must stand still)
        let attack_input = attack_key_down || self.touch_controls.attack_pressed();
        if attack_input && !state.is_sitting {
            // Send stop command if we were moving via keyboard or auto-path
            let was_pathing = state.auto_path.is_some();
            if self.last_dx != 0.0 || self.last_dy != 0.0 || was_pathing {
                commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                self.last_dx = 0.0;
                self.last_dy = 0.0;
            }
            // Cancel auto-path and auto-action when manually attacking
            state.clear_auto_path();
            self.reset_auto_path_motion_state();
            if state.auto_action_state.is_some() {
                state.auto_action_state = None;
                commands.push(InputCommand::CancelAutoAction);
            }

            // Ranged weapons have a longer cooldown to balance range advantage
            let attack_cooldown = {
                let weapon_range = get_local_weapon_range(state);
                if weapon_range > 1 { 1.1 } else { 0.8 }
            };
            if current_time - self.last_attack_time >= attack_cooldown {
                // Check if we should gather instead of attack
                let should_gather = if let Some(player) = state.get_local_player() {
                    if matches!(
                        player.equipped_weapon.as_deref(),
                        Some("fishing_rod" | "maple_rod")
                    ) {
                        let px = player.x.round() as i32;
                        let py = player.y.round() as i32;
                        let (fdx, fdy) = player.direction.to_unit_vector();
                        let face_x = px + fdx as i32;
                        let face_y = py + fdy as i32;
                        // Check if the tile we're facing is a fishing marker
                        state
                            .gathering_markers
                            .iter()
                            .find(|m| m.skill == "fishing" && m.x == face_x && m.y == face_y)
                            .map(|m| (m.x, m.y))
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Check if we should woodcut instead of attack (axe equipped + facing tree)
                let should_woodcut = if should_gather.is_none() {
                    if let Some(player) = state.get_local_player() {
                        // Check if player has an axe equipped (chop_speed_multiplier > 0)
                        let has_axe = player
                            .equipped_weapon
                            .as_ref()
                            .and_then(|weapon_id| state.item_registry.get(weapon_id))
                            .and_then(|item| item.equipment.as_ref())
                            .map(|eq| eq.chop_speed_multiplier > 0.0)
                            .unwrap_or(false);

                        if has_axe {
                            let px = player.x.round() as i32;
                            let py = player.y.round() as i32;
                            let (fdx, fdy) = player.direction.to_unit_vector();
                            let face_x = px + fdx as i32;
                            let face_y = py + fdy as i32;

                            // Check if facing tile has a tree object and is not depleted
                            if !state.depleted_trees.contains_key(&(face_x, face_y)) {
                                let obj_result =
                                    state.chunk_manager.get_object_at_exact(face_x, face_y);
                                if let Some(obj) = obj_result {
                                    Some((face_x, face_y, obj.gid))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Check if we should mine instead of attack (pickaxe equipped + facing rock)
                let should_mine = if should_gather.is_none() && should_woodcut.is_none() {
                    if let Some(player) = state.get_local_player() {
                        // Check if player has a pickaxe equipped (mine_speed_multiplier > 0)
                        let has_pickaxe = player
                            .equipped_weapon
                            .as_ref()
                            .and_then(|weapon_id| state.item_registry.get(weapon_id))
                            .and_then(|item| item.equipment.as_ref())
                            .map(|eq| eq.mine_speed_multiplier > 0.0)
                            .unwrap_or(false);

                        if has_pickaxe {
                            let px = player.x.round() as i32;
                            let py = player.y.round() as i32;
                            let (fdx, fdy) = player.direction.to_unit_vector();
                            let face_x = px + fdx as i32;
                            let face_y = py + fdy as i32;

                            // Check if facing tile has a rock object and is not depleted
                            if !state.depleted_rocks.contains_key(&(face_x, face_y)) {
                                let obj_result =
                                    state.chunk_manager.get_object_at_exact(face_x, face_y);
                                if let Some(obj) = obj_result {
                                    Some((face_x, face_y, obj.gid))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some((marker_x, marker_y)) = should_gather {
                    if !state.is_gathering {
                        commands.push(InputCommand::StartGathering { marker_x, marker_y });
                        self.last_attack_time = current_time;
                    }
                } else if let Some((tree_x, tree_y, tree_gid)) = should_woodcut {
                    // Send chop command on each attack press when facing a tree with an axe
                    commands.push(InputCommand::ChopTree {
                        tree_x,
                        tree_y,
                        tree_gid,
                    });
                    self.last_attack_time = current_time;
                } else if let Some((rock_x, rock_y, rock_gid)) = should_mine {
                    // Send mine command on each attack press when facing a rock with a pickaxe
                    commands.push(InputCommand::MineRock {
                        rock_x,
                        rock_y,
                        rock_gid,
                    });
                    self.last_attack_time = current_time;
                } else {
                    commands.push(InputCommand::Attack);
                    self.last_attack_time = current_time;

                    // Set attack animation based on weapon type
                    let anim_state = if let Some(player) = state.get_local_player() {
                        if let Some(ref weapon_id) = player.equipped_weapon {
                            if let Some(item_def) = state.item_registry.get(weapon_id) {
                                if item_def.weapon_type.as_deref() == Some("ranged") {
                                    AnimationState::ShootingBow
                                } else {
                                    AnimationState::Attacking
                                }
                            } else {
                                AnimationState::Attacking
                            }
                        } else {
                            AnimationState::Attacking
                        }
                    } else {
                        AnimationState::Attacking
                    };

                    // Now apply the animation to the player
                    if let Some(local_id) = &state.local_player_id.clone() {
                        if let Some(player) = state.players.get_mut(local_id) {
                            player.animation.set_state(anim_state);
                        }
                    }
                }
            }
        }

        // Handle mouse clicks on quick slots and inventory (always visible when open)
        if let Some(ref element) = clicked_element {
            match element {
                UiElementId::QuickSlot(idx) => {
                    if mouse_clicked {
                        // Unified hotkey bar: activate the binding
                        let cmds = activate_hotkey_slot(state, *idx);
                        commands.extend(cmds);
                    } else if mouse_right_clicked {
                        // Right-click opens context menu for hotkey slot
                        state.ui_state.context_menu = Some(ContextMenu {
                            target: ContextMenuTarget::HotkeySlot(*idx),
                            x: mx,
                            y: my,
                        });
                    }
                    return commands;
                }
                UiElementId::InventorySlot(idx) => {
                    if mouse_right_clicked {
                        // Right-click opens context menu (if item exists)
                        if state
                            .inventory
                            .slots
                            .get(*idx)
                            .and_then(|s| s.as_ref())
                            .is_some()
                        {
                            state.ui_state.context_menu = Some(ContextMenu {
                                target: ContextMenuTarget::InventorySlot(*idx),
                                x: mx,
                                y: my,
                            });
                        }
                    }
                    return commands;
                }
                UiElementId::EquipmentSlot(slot_type) => {
                    if mouse_right_clicked {
                        // Right-click on equipment slot opens context menu (if something is equipped)
                        let has_item = match slot_type.as_str() {
                            "head" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_head.as_ref())
                                .is_some(),
                            "body" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_body.as_ref())
                                .is_some(),
                            "weapon" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_weapon.as_ref())
                                .is_some(),
                            "back" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_back.as_ref())
                                .is_some(),
                            "feet" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_feet.as_ref())
                                .is_some(),
                            "ring" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_ring.as_ref())
                                .is_some(),
                            "gloves" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_gloves.as_ref())
                                .is_some(),
                            "necklace" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_necklace.as_ref())
                                .is_some(),
                            "belt" => state
                                .get_local_player()
                                .and_then(|p| p.equipped_belt.as_ref())
                                .is_some(),
                            _ => false,
                        };
                        if has_item {
                            state.ui_state.context_menu = Some(ContextMenu {
                                target: ContextMenuTarget::EquipmentSlot(slot_type.clone()),
                                x: mx,
                                y: my,
                            });
                        }
                    }
                    return commands;
                }
                UiElementId::CombatStyleButton(idx) => {
                    if mouse_clicked {
                        // Dynamic styles based on equipped weapon type
                        let is_ranged = state
                            .get_local_player()
                            .and_then(|p| p.equipped_weapon.as_ref())
                            .and_then(|wid| state.item_registry.get(wid))
                            .and_then(|def| def.weapon_type.as_ref())
                            .map(|wt| wt == "ranged")
                            .unwrap_or(false);
                        let styles: &[&str] = if is_ranged {
                            &["accurate", "rapid", "longrange"]
                        } else {
                            &["accurate", "aggressive", "defensive", "controlled"]
                        };
                        if let Some(style) = styles.get(*idx) {
                            audio.play_sfx("click");
                            commands.push(InputCommand::SetCombatStyle {
                                style: style.to_string(),
                            });
                            // Optimistically update local state
                            if let Some(local_id) = state.local_player_id.clone() {
                                if let Some(player) = state.players.get_mut(&local_id) {
                                    player.combat_style = style.to_string();
                                }
                            }
                        }
                    }
                    return commands;
                }
                UiElementId::CharacterOpenShopButton => {
                    if mouse_clicked {
                        audio.play_sfx("enter");
                        state.ui_state.stall_setup_open = !state.ui_state.stall_setup_open;
                        if state.ui_state.stall_setup_open {
                            state.ui_state.inventory_open = true;
                            state.ui_state.character_panel_open = false;
                        }
                    }
                    return commands;
                }
                UiElementId::GoldDisplay => {
                    if mouse_right_clicked && state.inventory.gold > 0 {
                        // Right-click on gold display opens context menu
                        state.ui_state.context_menu = Some(ContextMenu {
                            target: ContextMenuTarget::Gold,
                            x: mx,
                            y: my,
                        });
                    }
                    return commands;
                }
                UiElementId::GroundItem(item_id) => {
                    if mouse_clicked {
                        // Left-click on ground item - attempt pickup if within range, or path to it
                        if let Some(local_id) = &state.local_player_id {
                            if let Some(player) = state.players.get(local_id) {
                                if let Some(ground_item) = state.ground_items.get(item_id) {
                                    let dx = ground_item.x - player.x;
                                    let dy = ground_item.y - player.y;
                                    let dist = (dx * dx + dy * dy).sqrt();

                                    const PICKUP_RANGE: f32 = 2.0;
                                    if dist < PICKUP_RANGE {
                                        commands.push(InputCommand::Pickup {
                                            item_id: item_id.clone(),
                                        });
                                    } else {
                                        // Out of range - path to an adjacent tile
                                        let player_x = player.x.round() as i32;
                                        let player_y = player.y.round() as i32;
                                        let item_x = ground_item.x.round() as i32;
                                        let item_y = ground_item.y.round() as i32;

                                        // Build occupied set (other players + NPCs)
                                        let occupied = build_occupied_set(state, true);

                                        const MAX_PATH_DISTANCE: i32 = 32;
                                        if let Some((dest, path)) =
                                            pathfinding::find_path_to_adjacent(
                                                (player_x, player_y),
                                                (item_x, item_y),
                                                &state.chunk_manager,
                                                &occupied,
                                                MAX_PATH_DISTANCE,
                                            )
                                        {
                                            state.auto_path = Some(PathState {
                                                path,
                                                current_index: 0,
                                                destination: dest,
                                                pickup_target: Some(item_id.clone()),
                                                interact_target: None,
                                                interact_object_target: None,
                                                waystone_target: None,
                                                browse_stall_target: None,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return commands;
                }
                _ => {}
            }
        }

        // Target selection (left click) - only if not clicking on UI
        if mouse_clicked && clicked_element.is_none() {
            let (raw_x, raw_y) = mouse_position();
            let (mouse_x, mouse_y) = screen_to_virtual_coords(raw_x, raw_y);
            let (world_x, world_y) = screen_to_world(mouse_x, mouse_y, &state.camera);

            // Get the clicked tile coordinates
            let clicked_tile_x = world_x.round() as i32;
            let clicked_tile_y = world_y.round() as i32;

            // Find entity on the exact clicked tile
            let mut clicked_player: Option<String> = None;
            let mut clicked_npc: Option<String> = None;

            // Check players - must be on the exact clicked tile
            for (id, player) in &state.players {
                // Don't allow targeting self
                if state.local_player_id.as_ref() == Some(id) {
                    continue;
                }

                let player_tile_x = player.x.round() as i32;
                let player_tile_y = player.y.round() as i32;

                if player_tile_x == clicked_tile_x && player_tile_y == clicked_tile_y {
                    clicked_player = Some(id.clone());
                    break;
                }
            }

            // Check NPCs - must be on the exact clicked tile
            for (id, npc) in &state.npcs {
                // Only allow interacting with alive NPCs
                if !npc.is_alive() {
                    continue;
                }

                let npc_tile_x = npc.x.round() as i32;
                let npc_tile_y = npc.y.round() as i32;

                if npc_tile_x == clicked_tile_x && npc_tile_y == clicked_tile_y {
                    clicked_npc = Some(id.clone());
                    break;
                }
            }

            // Prioritize NPC interaction over player targeting
            if let Some(npc_id) = clicked_npc {
                // Check if NPC can be targeted for combat (not a merchant/quest giver/banker/altar)
                let is_attackable = state
                    .npcs
                    .get(&npc_id)
                    .map(|n| n.is_attackable())
                    .unwrap_or(true);

                if is_attackable {
                    // Attackable NPC - target it and set up auto-action chase
                    commands.push(InputCommand::Target {
                        entity_id: npc_id.clone(),
                    });
                    state.auto_action_state = Some(crate::game::AutoActionState {
                        target_type: "npc".to_string(),
                        target_id: npc_id.clone(),
                        action: "attack".to_string(),
                        confirmed: false,
                    });
                    // Pathfind to within attack range, or send immediately if already in range
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(player) = state.players.get(local_id) {
                            if let Some(npc) = state.npcs.get(&npc_id) {
                                let player_x = player.server_x.round() as i32;
                                let player_y = player.server_y.round() as i32;
                                let npc_x = npc.server_x.round() as i32;
                                let npc_y = npc.server_y.round() as i32;
                                let weapon_range = get_local_weapon_range(state);
                                if !in_attack_range(player_x, player_y, npc_x, npc_y, weapon_range) {
                                    let occupied = build_occupied_set(state, true);
                                    const MAX_PATH_DISTANCE: i32 = 32;
                                    let path_result = if weapon_range > 1 {
                                        pathfinding::find_path_within_range(
                                            (player_x, player_y),
                                            (npc_x, npc_y),
                                            &state.chunk_manager,
                                            &occupied,
                                            MAX_PATH_DISTANCE,
                                            weapon_range,
                                        )
                                    } else {
                                        pathfinding::find_path_to_adjacent(
                                            (player_x, player_y),
                                            (npc_x, npc_y),
                                            &state.chunk_manager,
                                            &occupied,
                                            MAX_PATH_DISTANCE,
                                        )
                                    };
                                    if let Some((dest, path)) = path_result {
                                        state.auto_path = Some(PathState {
                                            path,
                                            current_index: 0,
                                            destination: dest,
                                            pickup_target: None,
                                            interact_target: None,
                                            interact_object_target: None,
                                            waystone_target: None,
                                            browse_stall_target: None,
                                        });
                                    }
                                } else {
                                    // Already in range - face target and send immediately
                                    let dir = crate::game::Direction::from_velocity(
                                        npc_x as f32 - player_x as f32,
                                        npc_y as f32 - player_y as f32,
                                    );
                                    queue_face(state, &mut commands, dir as u8);
                                    if let Some(aa) = state.auto_action_state.as_ref() {
                                        if auto_action_target_settled(aa, state) {
                                            commands.push(InputCommand::StartAutoAction {
                                                target_type: "npc".to_string(),
                                                target_id: npc_id.clone(),
                                                action: "attack".to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Friendly NPC - interact or pathfind-to-interact
                    const INTERACT_RANGE: f32 = 2.5;
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(player) = state.players.get(local_id) {
                            if let Some(npc) = state.npcs.get(&npc_id) {
                                let dx = npc.x - player.x;
                                let dy = npc.y - player.y;
                                let dist_to_player = (dx * dx + dy * dy).sqrt();

                                if dist_to_player < INTERACT_RANGE {
                                    // Check if NPC is an altar or station
                                    if npc.is_altar {
                                        state.ui_state.altar_panel =
                                            Some(crate::game::AltarPanelState {
                                                altar_npc_id: npc_id.clone(),
                                                altar_name: npc.display_name.clone(),
                                            });
                                    } else if npc.station_type.as_deref() == Some("furnace")
                                        || npc.station_type.as_deref() == Some("fire_pit")
                                    {
                                        state.ui_state.furnace_station_type =
                                            npc.station_type.clone().unwrap_or_default();
                                        state.ui_state.fletching_open = false;
                                        state.ui_state.workbench_open = false;
                                        state.ui_state.furnace_open = true;
                                        state.ui_state.furnace_tile =
                                            Some((npc.x.round() as i32, npc.y.round() as i32));
                                        state.ui_state.furnace_selected_recipe = 0;
                                        state.ui_state.furnace_scroll_offset = 0.0;
                                        state.ui_state.furnace_quantity = 1;
                                        state.ui_state.furnace_tab = 0;
                                    } else if npc.station_type.as_deref() == Some("anvil") {
                                        state.ui_state.fletching_open = false;
                                        state.ui_state.workbench_open = false;
                                        state.ui_state.anvil_open = true;
                                        state.ui_state.anvil_tile =
                                            Some((npc.x.round() as i32, npc.y.round() as i32));
                                        state.ui_state.anvil_selected_recipe = 0;
                                        state.ui_state.anvil_scroll_offset = 0.0;
                                        state.ui_state.anvil_quantity = 1;
                                        state.ui_state.anvil_tab = 0;
                                    } else if npc.station_type.as_deref() == Some("alchemy_station")
                                    {
                                        state.ui_state.fletching_open = false;
                                        state.ui_state.workbench_open = false;
                                        state.ui_state.alchemy_station_open = true;
                                        state.ui_state.alchemy_station_tile =
                                            Some((npc.x.round() as i32, npc.y.round() as i32));
                                        state.ui_state.alchemy_station_selected_recipe = 0;
                                        state.ui_state.alchemy_station_scroll_offset = 0.0;
                                        state.ui_state.alchemy_station_quantity = 1;
                                        state.ui_state.alchemy_station_tab = 0;
                                    } else if npc.station_type.as_deref() == Some("workbench") {
                                        state.ui_state.fletching_open = false;
                                        state.ui_state.alchemy_station_open = false;
                                        state.ui_state.workbench_open = true;
                                        state.ui_state.workbench_tile =
                                            Some((npc.x.round() as i32, npc.y.round() as i32));
                                        state.ui_state.workbench_selected_recipe = 0;
                                        state.ui_state.workbench_scroll_offset = 0.0;
                                        state.ui_state.workbench_quantity = 1;
                                        state.ui_state.workbench_tab = 0;
                                    } else {
                                        commands.push(InputCommand::Interact { npc_id });
                                    }
                                } else {
                                    // Out of range - pathfind to adjacent tile
                                    let player_x = player.server_x.round() as i32;
                                    let player_y = player.server_y.round() as i32;
                                    let npc_x = npc.server_x.round() as i32;
                                    let npc_y = npc.server_y.round() as i32;

                                    // Build occupied set (other players + NPCs)
                                    let occupied = build_occupied_set(state, true);

                                    const MAX_PATH_DISTANCE: i32 = 32;
                                    if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                        (player_x, player_y),
                                        (npc_x, npc_y),
                                        &state.chunk_manager,
                                        &occupied,
                                        MAX_PATH_DISTANCE,
                                    ) {
                                        state.auto_path = Some(PathState {
                                            path,
                                            current_index: 0,
                                            destination: dest,
                                            pickup_target: None,
                                            interact_target: Some(npc_id),
                                            interact_object_target: None,
                                            waystone_target: None,
                                            browse_stall_target: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            } else if let Some(entity_id) = clicked_player {
                // Check if clicked player has an open stall - browse instead of attack
                let target_has_stall = state.players.get(&entity_id).map_or(false, |p| p.has_stall);

                if target_has_stall {
                    // Player has a stall - pathfind to them and browse their shop
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(local_player) = state.players.get(local_id) {
                            if let Some(target_player) = state.players.get(&entity_id) {
                                let player_x = local_player.server_x.round() as i32;
                                let player_y = local_player.server_y.round() as i32;
                                let target_x = target_player.server_x.round() as i32;
                                let target_y = target_player.server_y.round() as i32;
                                let cdx = (player_x - target_x).abs();
                                let cdy = (player_y - target_y).abs();
                                if (cdx + cdy) <= 3 {
                                    // Already in range - browse immediately
                                    commands.push(InputCommand::StallBrowse {
                                        player_id: entity_id.clone(),
                                    });
                                } else {
                                    // Pathfind to adjacent tile, then browse on arrival
                                    let occupied = build_occupied_set(state, true);
                                    const MAX_PATH_DISTANCE: i32 = 32;
                                    if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                        (player_x, player_y),
                                        (target_x, target_y),
                                        &state.chunk_manager,
                                        &occupied,
                                        MAX_PATH_DISTANCE,
                                    ) {
                                        state.auto_path = Some(PathState {
                                            path,
                                            current_index: 0,
                                            destination: dest,
                                            pickup_target: None,
                                            interact_target: None,
                                            interact_object_target: None,
                                            waystone_target: None,
                                            browse_stall_target: Some(entity_id.clone()),
                                        });
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Normal player click - target and set up auto-action chase
                    commands.push(InputCommand::Target {
                        entity_id: entity_id.clone(),
                    });
                    state.auto_action_state = Some(crate::game::AutoActionState {
                        target_type: "player".to_string(),
                        target_id: entity_id.clone(),
                        action: "attack".to_string(),
                        confirmed: false,
                    });
                    // Pathfind to within attack range, or send immediately if already in range
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(local_player) = state.players.get(local_id) {
                            if let Some(target_player) = state.players.get(&entity_id) {
                                let player_x = local_player.server_x.round() as i32;
                                let player_y = local_player.server_y.round() as i32;
                                let target_x = target_player.server_x.round() as i32;
                                let target_y = target_player.server_y.round() as i32;
                                let weapon_range = get_local_weapon_range(state);
                                if !in_attack_range(player_x, player_y, target_x, target_y, weapon_range) {
                                    let occupied = build_occupied_set(state, true);
                                    const MAX_PATH_DISTANCE: i32 = 32;
                                    let path_result = if weapon_range > 1 {
                                        pathfinding::find_path_within_range(
                                            (player_x, player_y),
                                            (target_x, target_y),
                                            &state.chunk_manager,
                                            &occupied,
                                            MAX_PATH_DISTANCE,
                                            weapon_range,
                                        )
                                    } else {
                                        pathfinding::find_path_to_adjacent(
                                            (player_x, player_y),
                                            (target_x, target_y),
                                            &state.chunk_manager,
                                            &occupied,
                                            MAX_PATH_DISTANCE,
                                        )
                                    };
                                    if let Some((dest, path)) = path_result {
                                        state.auto_path = Some(PathState {
                                            path,
                                            current_index: 0,
                                            destination: dest,
                                            pickup_target: None,
                                            interact_target: None,
                                            interact_object_target: None,
                                            waystone_target: None,
                                            browse_stall_target: None,
                                        });
                                    }
                                } else {
                                    // Already in range - face target and send to server immediately
                                    let dir = crate::game::Direction::from_velocity(
                                        target_x as f32 - player_x as f32,
                                        target_y as f32 - player_y as f32,
                                    );
                                    queue_face(state, &mut commands, dir as u8);
                                    commands.push(InputCommand::StartAutoAction {
                                        target_type: "player".to_string(),
                                        target_id: entity_id.clone(),
                                        action: "attack".to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            } else if state
                .chair_positions
                .contains(&(clicked_tile_x, clicked_tile_y))
            {
                // Clicked on a chair - try to sit
                if !state.is_sitting {
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(player) = state.players.get(local_id) {
                            let px = player.server_x.round() as i32;
                            let py = player.server_y.round() as i32;
                            let cdx = (px - clicked_tile_x).abs();
                            let cdy = (py - clicked_tile_y).abs();
                            if cdx <= 1 && cdy <= 1 {
                                // Within range - sit immediately
                                commands.push(InputCommand::SitChair {
                                    tile_x: clicked_tile_x,
                                    tile_y: clicked_tile_y,
                                });
                            } else {
                                // Out of range - pathfind to adjacent tile, then sit
                                let occupied = build_occupied_set(state, true);
                                const MAX_PATH_DISTANCE: i32 = 32;
                                if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                    (px, py),
                                    (clicked_tile_x, clicked_tile_y),
                                    &state.chunk_manager,
                                    &occupied,
                                    MAX_PATH_DISTANCE,
                                ) {
                                    state.auto_path = Some(PathState {
                                        path,
                                        current_index: 0,
                                        destination: dest,
                                        pickup_target: None,
                                        interact_target: None,
                                        interact_object_target: None,
                                        waystone_target: None,
                                        browse_stall_target: None,
                                    });
                                    state.pending_chair_sit =
                                        Some((clicked_tile_x, clicked_tile_y));
                                }
                            }
                        }
                    }
                }
            } else if let Some(obj) = state
                .chunk_manager
                .get_object_at_exact(clicked_tile_x, clicked_tile_y)
            {
                // Check if clicked object is a tree or rock for auto-action
                let obj_gid = obj.gid;
                let is_tree = crate::game::tree_types::is_tree_gid(obj_gid);
                let is_rock = crate::game::ore_types::get_ore_info(obj_gid).is_some();

                if is_tree
                    && !state
                        .depleted_trees
                        .contains_key(&(clicked_tile_x, clicked_tile_y))
                {
                    // Check if player has axe equipped
                    let has_axe = state
                        .get_local_player()
                        .and_then(|p| p.equipped_weapon.as_ref())
                        .and_then(|weapon_id| state.item_registry.get(weapon_id))
                        .and_then(|item| item.equipment.as_ref())
                        .map(|eq| eq.chop_speed_multiplier > 0.0)
                        .unwrap_or(false);

                    if has_axe {
                        let target_id =
                            format!("{},{},{}", clicked_tile_x, clicked_tile_y, obj_gid);
                        state.auto_action_state = Some(crate::game::AutoActionState {
                            target_type: "resource".to_string(),
                            target_id: target_id.clone(),
                            action: "chop".to_string(),
                            confirmed: false,
                        });
                        // Pathfind to adjacent tile, or send immediately if already adjacent
                        if let Some(player) = state.get_local_player() {
                            let player_x = player.server_x.round() as i32;
                            let player_y = player.server_y.round() as i32;
                            let cdx = (player_x - clicked_tile_x).abs();
                            let cdy = (player_y - clicked_tile_y).abs();
                            // Cardinal adjacency only (no diagonal)
                            if (cdx + cdy) != 1 {
                                let occupied = build_occupied_set(state, true);
                                const MAX_PATH_DISTANCE: i32 = 32;
                                if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                    (player_x, player_y),
                                    (clicked_tile_x, clicked_tile_y),
                                    &state.chunk_manager,
                                    &occupied,
                                    MAX_PATH_DISTANCE,
                                ) {
                                    state.auto_path = Some(PathState {
                                        path,
                                        current_index: 0,
                                        destination: dest,
                                        pickup_target: None,
                                        interact_target: None,
                                        interact_object_target: None,
                                        waystone_target: None,
                                        browse_stall_target: None,
                                    });
                                }
                            } else {
                                // Already cardinal-adjacent - face target and send immediately
                                let dir = crate::game::Direction::from_velocity(
                                    clicked_tile_x as f32 - player_x as f32,
                                    clicked_tile_y as f32 - player_y as f32,
                                );
                                queue_face(state, &mut commands, dir as u8);
                                commands.push(InputCommand::StartAutoAction {
                                    target_type: "resource".to_string(),
                                    target_id,
                                    action: "chop".to_string(),
                                });
                            }
                        }
                    }
                } else if is_rock
                    && !state
                        .depleted_rocks
                        .contains_key(&(clicked_tile_x, clicked_tile_y))
                {
                    // Check if player has pickaxe equipped
                    let has_pickaxe = state
                        .get_local_player()
                        .and_then(|p| p.equipped_weapon.as_ref())
                        .and_then(|weapon_id| state.item_registry.get(weapon_id))
                        .and_then(|item| item.equipment.as_ref())
                        .map(|eq| eq.mine_speed_multiplier > 0.0)
                        .unwrap_or(false);

                    if has_pickaxe {
                        let target_id =
                            format!("{},{},{}", clicked_tile_x, clicked_tile_y, obj_gid);
                        state.auto_action_state = Some(crate::game::AutoActionState {
                            target_type: "resource".to_string(),
                            target_id: target_id.clone(),
                            action: "mine".to_string(),
                            confirmed: false,
                        });
                        // Pathfind to adjacent tile, or send immediately if already adjacent
                        if let Some(player) = state.get_local_player() {
                            let player_x = player.server_x.round() as i32;
                            let player_y = player.server_y.round() as i32;
                            let cdx = (player_x - clicked_tile_x).abs();
                            let cdy = (player_y - clicked_tile_y).abs();
                            // Cardinal adjacency only (no diagonal)
                            if (cdx + cdy) != 1 {
                                let occupied = build_occupied_set(state, true);
                                const MAX_PATH_DISTANCE: i32 = 32;
                                if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                    (player_x, player_y),
                                    (clicked_tile_x, clicked_tile_y),
                                    &state.chunk_manager,
                                    &occupied,
                                    MAX_PATH_DISTANCE,
                                ) {
                                    state.auto_path = Some(PathState {
                                        path,
                                        current_index: 0,
                                        destination: dest,
                                        pickup_target: None,
                                        interact_target: None,
                                        interact_object_target: None,
                                        waystone_target: None,
                                        browse_stall_target: None,
                                    });
                                }
                            } else {
                                // Already cardinal-adjacent - face target and send immediately
                                let dir = crate::game::Direction::from_velocity(
                                    clicked_tile_x as f32 - player_x as f32,
                                    clicked_tile_y as f32 - player_y as f32,
                                );
                                queue_face(state, &mut commands, dir as u8);
                                commands.push(InputCommand::StartAutoAction {
                                    target_type: "resource".to_string(),
                                    target_id,
                                    action: "mine".to_string(),
                                });
                            }
                        }
                    }
                } else if is_obelisk_gid(obj_gid)
                    || state
                        .chest_positions
                        .contains(&(clicked_tile_x, clicked_tile_y))
                {
                    // Clicked on an obelisk or chest — walk to it and interact
                    if let Some(player) = state.get_local_player() {
                        let player_x = player.server_x.round() as i32;
                        let player_y = player.server_y.round() as i32;
                        let cdx = (player_x - clicked_tile_x).abs();
                        let cdy = (player_y - clicked_tile_y).abs();
                        if cdx <= 1 && cdy <= 1 {
                            // Already adjacent — interact immediately
                            commands.push(InputCommand::InteractObject {
                                x: clicked_tile_x,
                                y: clicked_tile_y,
                            });
                        } else {
                            // Pathfind to adjacent tile, then interact
                            let occupied = build_occupied_set(state, true);
                            const MAX_PATH_DISTANCE: i32 = 32;
                            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                (player_x, player_y),
                                (clicked_tile_x, clicked_tile_y),
                                &state.chunk_manager,
                                &occupied,
                                MAX_PATH_DISTANCE,
                            ) {
                                state.auto_path = Some(PathState {
                                    path,
                                    current_index: 0,
                                    destination: dest,
                                    pickup_target: None,
                                    interact_target: None,
                                    interact_object_target: Some((clicked_tile_x, clicked_tile_y)),
                                    waystone_target: None,
                                    browse_stall_target: None,
                                });
                            }
                        }
                    }
                }
            } else if let Some(patch_id) = state
                .farming_patch_positions
                .get(&(clicked_tile_x, clicked_tile_y))
                .cloned()
            {
                // Clicked on a farming patch
                if let Some(patch) = state.farming_patches.get(&patch_id) {
                    if patch.state == "harvestable" {
                        if let Some(local_id) = &state.local_player_id {
                            if let Some(player) = state.players.get(local_id) {
                                let px = player.server_x.round() as i32;
                                let py = player.server_y.round() as i32;
                                let cdx = (px - clicked_tile_x).abs();
                                let cdy = (py - clicked_tile_y).abs();
                                if cdx <= 1 && cdy <= 1 {
                                    commands.push(InputCommand::HarvestCrop { patch_id });
                                } else {
                                    // Out of range - pathfind to adjacent tile
                                    let occupied = build_occupied_set(state, true);
                                    const MAX_PATH_DISTANCE: i32 = 32;
                                    if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                        (px, py),
                                        (clicked_tile_x, clicked_tile_y),
                                        &state.chunk_manager,
                                        &occupied,
                                        MAX_PATH_DISTANCE,
                                    ) {
                                        state.auto_path = Some(PathState {
                                            path,
                                            current_index: 0,
                                            destination: dest,
                                            pickup_target: None,
                                            interact_target: None,
                                            interact_object_target: None,
                                            waystone_target: None,
                                            browse_stall_target: None,
                                        });
                                        state.pending_harvest_patch = Some(patch_id);
                                    }
                                }
                            }
                        }
                    }
                }
            } else if state
                .chest_positions
                .contains(&(clicked_tile_x, clicked_tile_y))
            {
                // Clicked on a chest - walk to it and interact
                if let Some(player) = state.get_local_player() {
                    let px = player.server_x.round() as i32;
                    let py = player.server_y.round() as i32;
                    let cdx = (px - clicked_tile_x).abs();
                    let cdy = (py - clicked_tile_y).abs();
                    if cdx <= 1 && cdy <= 1 {
                        // Already adjacent — interact immediately
                        commands.push(InputCommand::InteractObject {
                            x: clicked_tile_x,
                            y: clicked_tile_y,
                        });
                    } else {
                        // Pathfind to adjacent tile, then interact
                        let occupied = build_occupied_set(state, true);
                        const MAX_PATH_DISTANCE: i32 = 32;
                        if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                            (px, py),
                            (clicked_tile_x, clicked_tile_y),
                            &state.chunk_manager,
                            &occupied,
                            MAX_PATH_DISTANCE,
                        ) {
                            state.auto_path = Some(PathState {
                                path,
                                current_index: 0,
                                destination: dest,
                                pickup_target: None,
                                interact_target: None,
                                interact_object_target: Some((clicked_tile_x, clicked_tile_y)),
                                waystone_target: None,
                                browse_stall_target: None,
                            });
                        }
                    }
                }
            } else if state.ui_state.tap_to_pathfind {
                // Clicked on empty space - cancel auto-action and path there
                if state.auto_action_state.is_some() {
                    state.auto_action_state = None;
                    commands.push(InputCommand::CancelAutoAction);
                }

                let tile_x = world_x.round() as i32;
                let tile_y = world_y.round() as i32;

                // Only path if within range and walkable
                const MAX_PATH_DISTANCE: i32 = 32;

                if let Some(player) = state.get_local_player() {
                    // Use server-authoritative tile for click-to-move.
                    let player_x = player.server_x.round() as i32;
                    let player_y = player.server_y.round() as i32;
                    let dist = (tile_x - player_x).abs().max((tile_y - player_y).abs());

                    if dist <= MAX_PATH_DISTANCE
                        && state
                            .chunk_manager
                            .is_walkable(tile_x as f32, tile_y as f32)
                    {
                        // Build occupied set (other players + NPCs)
                        let occupied = build_occupied_set(state, true);

                        // Calculate path using A*
                        if let Some(path) = pathfinding::find_path(
                            (player_x, player_y),
                            (tile_x, tile_y),
                            &state.chunk_manager,
                            &occupied,
                            MAX_PATH_DISTANCE,
                        ) {
                            state.auto_path = Some(PathState {
                                path,
                                current_index: 0,
                                destination: (tile_x, tile_y),
                                pickup_target: None,
                                interact_target: None,
                                interact_object_target: None,
                                waystone_target: None,
                                browse_stall_target: None,
                            });
                        }
                    }
                }

                // Also clear target when clicking empty space
                if state.selected_entity_id.is_some() {
                    commands.push(InputCommand::ClearTarget);
                }
            }
        }

        // Right-click world detection - open context menu for world entities
        if mouse_right_clicked && clicked_element.is_none() {
            let (raw_x, raw_y) = mouse_position();
            let (mouse_vx, mouse_vy) = screen_to_virtual_coords(raw_x, raw_y);
            let (world_x, world_y) = screen_to_world(mouse_vx, mouse_vy, &state.camera);
            let clicked_tile_x = world_x.round() as i32;
            let clicked_tile_y = world_y.round() as i32;

            // Determine what's under the cursor, same priority as left-click
            let target = 'find_target: {
                // Check NPCs
                for (id, npc) in &state.npcs {
                    if !npc.is_alive() {
                        continue;
                    }
                    let npc_tile_x = npc.x.round() as i32;
                    let npc_tile_y = npc.y.round() as i32;
                    if npc_tile_x == clicked_tile_x && npc_tile_y == clicked_tile_y {
                        break 'find_target ContextMenuTarget::Npc { id: id.clone() };
                    }
                }

                // Check players (skip self)
                for (id, player) in &state.players {
                    if state.local_player_id.as_ref() == Some(id) {
                        continue;
                    }
                    let player_tile_x = player.x.round() as i32;
                    let player_tile_y = player.y.round() as i32;
                    if player_tile_x == clicked_tile_x && player_tile_y == clicked_tile_y {
                        break 'find_target ContextMenuTarget::Player { id: id.clone() };
                    }
                }

                // Check ground items
                for (_id, item) in &state.ground_items {
                    let ix = item.x.round() as i32;
                    let iy = item.y.round() as i32;
                    if ix == clicked_tile_x && iy == clicked_tile_y {
                        break 'find_target ContextMenuTarget::GroundItem {
                            id: item.id.clone(),
                        };
                    }
                }

                // Check map objects (trees/rocks)
                if let Some(obj) = state
                    .chunk_manager
                    .get_object_at_exact(clicked_tile_x, clicked_tile_y)
                {
                    let obj_gid = obj.gid;
                    if crate::game::tree_types::is_tree_gid(obj_gid) {
                        break 'find_target ContextMenuTarget::Tree {
                            tile_x: clicked_tile_x,
                            tile_y: clicked_tile_y,
                            gid: obj_gid,
                        };
                    }
                    if crate::game::ore_types::get_ore_info(obj_gid).is_some() {
                        break 'find_target ContextMenuTarget::Rock {
                            tile_x: clicked_tile_x,
                            tile_y: clicked_tile_y,
                            gid: obj_gid,
                        };
                    }
                    // Generic map object (obelisks, waystones, etc.)
                    break 'find_target ContextMenuTarget::MapObject {
                        tile_x: clicked_tile_x,
                        tile_y: clicked_tile_y,
                        gid: obj_gid,
                    };
                }

                // Check gathering markers
                for (i, marker) in state.gathering_markers.iter().enumerate() {
                    if marker.x == clicked_tile_x && marker.y == clicked_tile_y {
                        break 'find_target ContextMenuTarget::GatheringSpot { marker_index: i };
                    }
                }

                // Check farming patches
                if let Some(patch_id) = state
                    .farming_patch_positions
                    .get(&(clicked_tile_x, clicked_tile_y))
                    .cloned()
                {
                    break 'find_target ContextMenuTarget::FarmingPatch { patch_id };
                }

                // Default: empty tile
                ContextMenuTarget::Tile {
                    x: clicked_tile_x,
                    y: clicked_tile_y,
                }
            };

            state.ui_state.context_menu = Some(ContextMenu {
                target,
                x: mx,
                y: my,
            });
        }

        // Escape key - close any open panel first, then clear target, then open escape menu
        if is_key_pressed(KeyCode::Escape) {
            // Close hotkey settings popup first
            if state.ui_state.hotkey_settings_open {
                audio.play_sfx("enter");
                state.ui_state.hotkey_settings_open = false;
            } else
            // Check if any panel is open and close it
            if state.ui_state.inventory_open
                || state.ui_state.character_panel_open
                || state.ui_state.social_open
                || state.ui_state.skills_open
                || state.ui_state.prayer_book_open
                || state.ui_state.quest_log_open
            {
                audio.play_sfx("enter");
                state.ui_state.inventory_open = false;
                state.ui_state.character_panel_open = false;
                state.ui_state.social_open = false;
                state.ui_state.skills_open = false;
                state.ui_state.prayer_book_open = false;
                state.ui_state.close_quest_log();
                // Reset social panel input state
                state.social_state.add_friend_focused = false;
            } else if state.selected_entity_id.is_some() {
                commands.push(InputCommand::ClearTarget);
            } else {
                // No target selected and no panels open - open escape menu
                audio.play_sfx("enter");
                state.ui_state.escape_menu_open = true;
            }
        }

        // Toggle inventory (I key) with mutual exclusivity
        // In classic mode, letter/number keys go to chat input, not hotkeys
        if !classic && is_key_pressed(KeyCode::I) {
            audio.play_sfx("enter");
            if state.ui_state.inventory_open {
                state.ui_state.inventory_open = false;
            } else {
                state.ui_state.inventory_open = true;
                state.ui_state.character_panel_open = false;
                state.ui_state.social_open = false;
                state.ui_state.skills_open = false;
                state.ui_state.prayer_book_open = false;
                state.ui_state.minimap_panel_open = false;
                state.ui_state.close_quest_log();
            }
        }

        // Toggle skills panel (T key) with mutual exclusivity
        if !classic && is_key_pressed(KeyCode::T) {
            audio.play_sfx("enter");
            if state.ui_state.skills_open {
                state.ui_state.skills_open = false;
            } else {
                state.ui_state.skills_open = true;
                state.ui_state.inventory_open = false;
                state.ui_state.character_panel_open = false;
                state.ui_state.social_open = false;
                state.ui_state.prayer_book_open = false;
                state.ui_state.minimap_panel_open = false;
                state.ui_state.close_quest_log();
            }
        }

        // Chat log scrolling (mouse wheel on desktop) - uses direct bounds check
        // since chat log is not registered for hit detection (allows click-through)
        if state.ui_state.chat_log_visible {
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                let (mx, my) = mouse_position();
                let (vmx, vmy) = screen_to_virtual_coords(mx, my);
                let (_, chat_sh) = virtual_screen_size();
                let scale = state.ui_state.ui_scale;
                let bg_padding = 6.0 * scale;
                let box_bottom = chat_sh - 8.0 * scale; // EXP_BAR_GAP * scale
                let line_height = 18.0 * scale;
                let max_chat_width = if scale >= 2.0 {
                    400.0 * scale - 260.0
                } else {
                    360.0 * scale
                };
                let max_visible_lines: usize = if scale >= 2.0 { 6 } else { 7 };
                let chat_area_h = max_visible_lines as f32 * line_height;
                let chat_bottom_y = box_bottom - bg_padding;
                let chat_top_y = chat_bottom_y - chat_area_h + line_height;
                let over_chat = vmx >= 10.0 - bg_padding
                    && vmx <= 10.0 + max_chat_width + bg_padding
                    && vmy >= chat_top_y - bg_padding
                    && vmy <= box_bottom;
                if over_chat {
                    const SCROLL_SPEED: f32 = 40.0; // Pixels per scroll tick
                    let max_scroll = layout
                        .get_max_scroll(&UiElementId::ChatLogScrollbar)
                        .unwrap_or(0.0);
                    let delta = wheel_y * SCROLL_SPEED;
                    state.ui_state.chat_message_scroll =
                        (state.ui_state.chat_message_scroll + delta).clamp(0.0, max_scroll);
                }
            }
        }

        // Chat log scrollbar drag handling
        if state.ui_state.chat_log_visible {
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::ChatLogScrollbar) {
                let chat_max = layout
                    .get_max_scroll(&UiElementId::ChatLogScrollbar)
                    .unwrap_or(0.0);
                let chat_content_h = chat_max + track_bounds.h;
                let clicked_on = matches!(clicked_element, Some(UiElementId::ChatLogScrollbar));
                crate::ui::scroll::handle_scrollbar_drag_ex(
                    &mut state.ui_state.chat_scroll_drag,
                    &mut state.ui_state.chat_message_scroll,
                    chat_max,
                    track_bounds,
                    chat_content_h,
                    my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                    true, // inverted: thumb at bottom when scroll=0
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.chat_scroll_drag.dragging = false;
            }
        }

        // Inventory grid scrolling (mouse wheel / touch drag)
        if state.ui_state.inventory_open {
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                // Check if hovering over inventory grid or any inventory slot
                let over_inventory = matches!(
                    &state.ui_state.hovered_element,
                    Some(UiElementId::InventoryGridArea) | Some(UiElementId::InventorySlot(_))
                );
                if over_inventory {
                    const SCROLL_SPEED: f32 = 30.0;
                    let max_scroll = layout
                        .get_max_scroll(&UiElementId::InventoryScrollbar)
                        .unwrap_or(0.0);
                    state.ui_state.inventory_scroll_offset =
                        (state.ui_state.inventory_scroll_offset - wheel_y * SCROLL_SPEED)
                            .clamp(0.0, max_scroll);
                }
            }

            // Mouse scrollbar dragging (generic system)
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::InventoryScrollbar) {
                let inv_max_scroll = layout
                    .get_max_scroll(&UiElementId::InventoryScrollbar)
                    .unwrap_or(0.0);
                let inv_content_h = inv_max_scroll + track_bounds.h;
                let clicked_on = matches!(clicked_element, Some(UiElementId::InventoryScrollbar));
                crate::ui::scroll::handle_scrollbar_drag(
                    &mut state.ui_state.inventory_scroll_drag,
                    &mut state.ui_state.inventory_scroll_offset,
                    inv_max_scroll,
                    track_bounds,
                    inv_content_h,
                    my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.inventory_scroll_drag.dragging = false;
            }

            // Touch drag scrolling for mobile
            let all_touches: Vec<Touch> = touches();
            if let Some(tracking_id) = state.ui_state.inventory_touch_scroll_id {
                // We're tracking a touch - update or release
                if let Some(touch) = all_touches.iter().find(|t| t.id == tracking_id) {
                    match touch.phase {
                        TouchPhase::Moved | TouchPhase::Stationary => {
                            let (_, vy) =
                                screen_to_virtual_coords(touch.position.x, touch.position.y);
                            let dy = state.ui_state.inventory_touch_last_y - vy;
                            state.ui_state.inventory_scroll_offset =
                                (state.ui_state.inventory_scroll_offset + dy).max(0.0);
                            state.ui_state.inventory_touch_last_y = vy;
                        }
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            state.ui_state.inventory_touch_scroll_id = None;
                        }
                        _ => {}
                    }
                } else {
                    state.ui_state.inventory_touch_scroll_id = None;
                }
            } else {
                // Look for new touch starting in the inventory grid area
                for touch in &all_touches {
                    if touch.phase == TouchPhase::Started {
                        let (vx, vy) = screen_to_virtual_coords(touch.position.x, touch.position.y);
                        let over_grid = matches!(
                            layout.hit_test(vx, vy),
                            Some(UiElementId::InventoryGridArea)
                                | Some(UiElementId::InventorySlot(_))
                                | Some(UiElementId::InventoryScrollbar)
                        );
                        if over_grid {
                            state.ui_state.inventory_touch_scroll_id = Some(touch.id);
                            state.ui_state.inventory_touch_last_y = vy;
                            break;
                        }
                    }
                }
            }
        } else {
            // Reset tracking when inventory closes
            state.ui_state.inventory_touch_scroll_id = None;
            state.ui_state.inventory_scroll_drag.dragging = false;
        }

        // Toggle character panel (C key) with mutual exclusivity
        if !classic && is_key_pressed(KeyCode::C) {
            audio.play_sfx("enter");
            if state.ui_state.character_panel_open {
                state.ui_state.character_panel_open = false;
            } else {
                state.ui_state.character_panel_open = true;
                state.ui_state.inventory_open = false;
                state.ui_state.social_open = false;
                state.ui_state.skills_open = false;
                state.ui_state.prayer_book_open = false;
                state.ui_state.minimap_panel_open = false;
            }
        }

        // Toggle prayer book (P key) with mutual exclusivity
        if !classic && is_key_pressed(KeyCode::P) {
            audio.play_sfx("enter");
            if state.ui_state.prayer_book_open {
                state.ui_state.prayer_book_open = false;
            } else {
                state.ui_state.prayer_book_open = true;
                state.ui_state.inventory_open = false;
                state.ui_state.character_panel_open = false;
                state.ui_state.social_open = false;
                state.ui_state.skills_open = false;
                state.ui_state.minimap_panel_open = false;
            }
        }

        // Toggle expanded minimap panel (M key)
        if !classic && is_key_pressed(KeyCode::M) {
            audio.play_sfx("enter");
            state.ui_state.minimap_panel_open = !state.ui_state.minimap_panel_open;
            if state.ui_state.minimap_panel_open {
                state.ui_state.inventory_open = false;
                state.ui_state.character_panel_open = false;
                state.ui_state.social_open = false;
                state.ui_state.skills_open = false;
                state.ui_state.prayer_book_open = false;
                state.ui_state.close_quest_log();
                state.ui_state.chat_panel_open = false;
                state.ui_state.chat_open = false;
                state.ui_state.minimap_panel_zoom = 1.0;
                state.ui_state.minimap_panel_center_x = None;
                state.ui_state.minimap_panel_center_y = None;
            }
            state.ui_state.minimap_panel_dragging = false;
            return commands;
        }

        // Use/equip items or cast spells via unified hotkey bar (1-5 keys, disabled in classic mode)
        let quick_slot_keys = [
            (KeyCode::Key1, 0usize),
            (KeyCode::Key2, 1usize),
            (KeyCode::Key3, 2usize),
            (KeyCode::Key4, 3usize),
            (KeyCode::Key5, 4usize),
        ];
        for (key, slot_idx) in quick_slot_keys {
            if !classic && is_key_pressed(key) {
                let cmds = activate_hotkey_slot(state, slot_idx);
                commands.extend(cmds);
            }
        }

        // Pickup nearest item (F key or touch interact when no NPC nearby)
        let pickup_pressed = !classic && is_key_pressed(KeyCode::F);
        if pickup_pressed {
            // Get local player position
            if let Some(local_id) = &state.local_player_id {
                if let Some(player) = state.players.get(local_id) {
                    // Find nearest item within pickup range (2 tiles)
                    const PICKUP_RANGE: f32 = 2.0;
                    let mut nearest_item: Option<(String, f32)> = None;

                    for (id, item) in &state.ground_items {
                        let dx = item.x - player.x;
                        let dy = item.y - player.y;
                        let dist = (dx * dx + dy * dy).sqrt();

                        if dist < PICKUP_RANGE {
                            if nearest_item.is_none() || dist < nearest_item.as_ref().unwrap().1 {
                                nearest_item = Some((id.clone(), dist));
                            }
                        }
                    }

                    if let Some((item_id, _)) = nearest_item {
                        commands.push(InputCommand::Pickup { item_id });
                    }
                }
            }
        }

        // Interact with nearest NPC (E key or touch interact button)
        // Touch interact button also picks up items if no NPC nearby
        let interact_pressed =
            (!classic && is_key_pressed(KeyCode::E)) || self.touch_controls.interact_pressed();
        if interact_pressed {
            // If sitting, stand up
            if state.is_sitting {
                commands.push(InputCommand::StandUp);
                state.is_sitting = false;
                if let Some(local_id) = &state.local_player_id {
                    if let Some(player) = state.players.get_mut(local_id) {
                        player.stand_up();
                    }
                }
            } else if let Some(local_id) = &state.local_player_id {
                // Check for nearby chairs first, then NPCs
                let mut sat_on_chair = false;
                if let Some(player) = state.players.get(local_id) {
                    let px = player.x.round() as i32;
                    let py = player.y.round() as i32;
                    let mut nearest_chair: Option<((i32, i32), i32)> = None;
                    for &(cx, cy) in &state.chair_positions {
                        let cdx = (px - cx).abs();
                        let cdy = (py - cy).abs();
                        let dist = cdx.max(cdy);
                        if dist <= 1 {
                            if nearest_chair.is_none() || dist < nearest_chair.unwrap().1 {
                                nearest_chair = Some(((cx, cy), dist));
                            }
                        }
                    }
                    if let Some(((cx, cy), _)) = nearest_chair {
                        commands.push(InputCommand::SitChair {
                            tile_x: cx,
                            tile_y: cy,
                        });
                        sat_on_chair = true;
                    }
                }
                if !sat_on_chair {
                    if let Some(player) = state.players.get(local_id) {
                        // Find nearest NPC within interaction range (2.5 tiles)
                        const INTERACT_RANGE: f32 = 2.5;
                        let mut nearest_npc: Option<(String, f32)> = None;

                        for (id, npc) in &state.npcs {
                            // Only interact with alive NPCs
                            if !npc.is_alive() {
                                continue;
                            }

                            let dx = npc.x - player.x;
                            let dy = npc.y - player.y;
                            let dist = (dx * dx + dy * dy).sqrt();

                            if dist < INTERACT_RANGE {
                                if nearest_npc.is_none() || dist < nearest_npc.as_ref().unwrap().1 {
                                    nearest_npc = Some((id.clone(), dist));
                                }
                            }
                        }

                        if let Some((npc_id, _)) = nearest_npc {
                            log::info!("Interacting with NPC: {}", npc_id);
                            // Check if NPC is an altar or station
                            if let Some(npc) = state.npcs.get(&npc_id) {
                                if npc.is_altar {
                                    state.ui_state.altar_panel =
                                        Some(crate::game::AltarPanelState {
                                            altar_npc_id: npc_id.clone(),
                                            altar_name: npc.display_name.clone(),
                                        });
                                } else if npc.station_type.as_deref() == Some("furnace")
                                    || npc.station_type.as_deref() == Some("fire_pit")
                                {
                                    state.ui_state.furnace_station_type =
                                        npc.station_type.clone().unwrap_or_default();
                                    state.ui_state.fletching_open = false;
                                    state.ui_state.workbench_open = false;
                                    state.ui_state.furnace_open = true;
                                    state.ui_state.furnace_tile =
                                        Some((npc.x.round() as i32, npc.y.round() as i32));
                                    state.ui_state.furnace_selected_recipe = 0;
                                    state.ui_state.furnace_scroll_offset = 0.0;
                                    state.ui_state.furnace_quantity = 1;
                                    state.ui_state.furnace_tab = 0;
                                } else if npc.station_type.as_deref() == Some("anvil") {
                                    state.ui_state.fletching_open = false;
                                    state.ui_state.workbench_open = false;
                                    state.ui_state.anvil_open = true;
                                    state.ui_state.anvil_tile =
                                        Some((npc.x.round() as i32, npc.y.round() as i32));
                                    state.ui_state.anvil_selected_recipe = 0;
                                    state.ui_state.anvil_scroll_offset = 0.0;
                                    state.ui_state.anvil_quantity = 1;
                                    state.ui_state.anvil_tab = 0;
                                } else if npc.station_type.as_deref() == Some("alchemy_station") {
                                    state.ui_state.fletching_open = false;
                                    state.ui_state.workbench_open = false;
                                    state.ui_state.alchemy_station_open = true;
                                    state.ui_state.alchemy_station_tile =
                                        Some((npc.x.round() as i32, npc.y.round() as i32));
                                    state.ui_state.alchemy_station_selected_recipe = 0;
                                    state.ui_state.alchemy_station_scroll_offset = 0.0;
                                    state.ui_state.alchemy_station_quantity = 1;
                                    state.ui_state.alchemy_station_tab = 0;
                                } else if npc.station_type.as_deref() == Some("workbench") {
                                    state.ui_state.fletching_open = false;
                                    state.ui_state.alchemy_station_open = false;
                                    state.ui_state.workbench_open = true;
                                    state.ui_state.workbench_tile =
                                        Some((npc.x.round() as i32, npc.y.round() as i32));
                                    state.ui_state.workbench_selected_recipe = 0;
                                    state.ui_state.workbench_scroll_offset = 0.0;
                                    state.ui_state.workbench_quantity = 1;
                                    state.ui_state.workbench_tab = 0;
                                } else {
                                    commands.push(InputCommand::Interact { npc_id });
                                }
                            } else {
                                commands.push(InputCommand::Interact { npc_id });
                            }
                        } else if self.touch_controls.interact_pressed() {
                            // Touch interact fallback: pickup item if no NPC nearby
                            const PICKUP_RANGE: f32 = 2.0;
                            let mut nearest_item: Option<(String, f32)> = None;
                            for (id, item) in &state.ground_items {
                                let dx = item.x - player.x;
                                let dy = item.y - player.y;
                                let dist = (dx * dx + dy * dy).sqrt();
                                if dist < PICKUP_RANGE {
                                    if nearest_item.is_none()
                                        || dist < nearest_item.as_ref().unwrap().1
                                    {
                                        nearest_item = Some((id.clone(), dist));
                                    }
                                }
                            }
                            if let Some((item_id, _)) = nearest_item {
                                commands.push(InputCommand::Pickup { item_id });
                            }
                        }
                    }
                }
            }
        }

        // Toggle quest log (Q key) with mutual exclusivity
        if !classic && is_key_pressed(KeyCode::Q) {
            audio.play_sfx("enter");
            if state.ui_state.quest_log_open {
                state.ui_state.close_quest_log();
            } else {
                state.ui_state.quest_log_open = true;
                state.ui_state.quest_log_scroll = 0.0;
                state.ui_state.selected_quest_id = None;
                state.ui_state.inventory_open = false;
                state.ui_state.character_panel_open = false;
                state.ui_state.social_open = false;
                state.ui_state.skills_open = false;
                state.ui_state.prayer_book_open = false;
                state.ui_state.minimap_panel_open = false;
            }
        }

        // Quest log scrolling
        if state.ui_state.quest_log_open {
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                const SCROLL_SPEED: f32 = 30.0;
                let max_scroll = layout
                    .get_max_scroll(&UiElementId::QuestLogScrollbar)
                    .unwrap_or(0.0);
                state.ui_state.quest_log_scroll = (state.ui_state.quest_log_scroll
                    - wheel_y * SCROLL_SPEED)
                    .clamp(0.0, max_scroll);
            }

            // Scrollbar drag handling
            if let Some(track_bounds) = layout.get_bounds(&UiElementId::QuestLogScrollbar) {
                let ql_max_scroll = layout
                    .get_max_scroll(&UiElementId::QuestLogScrollbar)
                    .unwrap_or(0.0);
                let ql_content_h = ql_max_scroll + track_bounds.h;
                let clicked_on = matches!(clicked_element, Some(UiElementId::QuestLogScrollbar));
                crate::ui::scroll::handle_scrollbar_drag(
                    &mut state.ui_state.quest_log_scroll_drag,
                    &mut state.ui_state.quest_log_scroll,
                    ql_max_scroll,
                    track_bounds,
                    ql_content_h,
                    my,
                    is_mouse_button_down(MouseButton::Left),
                    mouse_clicked,
                    clicked_on,
                );
            } else if !is_mouse_button_down(MouseButton::Left) {
                state.ui_state.quest_log_scroll_drag.dragging = false;
            }
        } else {
            state.ui_state.quest_log_scroll_drag.dragging = false;
        }

        commands
    }

    /// Render touch controls overlay (call after all other rendering)
    /// Set hide_action_buttons to true when panels like inventory are open
    pub fn render_touch_controls(
        &self,
        hide_action_buttons: bool,
        hide_all_controls: bool,
        use_joystick: bool,
    ) {
        self.touch_controls
            .render(hide_action_buttons, hide_all_controls, use_joystick);
    }

    /// Update attack button to show the currently equipped weapon sprite
    pub fn update_attack_button_icon(
        &mut self,
        weapon_id: Option<&str>,
        item_sprites: &crate::render::SpriteStore,
    ) {
        self.touch_controls
            .update_attack_icon(weapon_id, item_sprites);
    }

    /// Auto-scroll anvil grid to keep selected recipe in view
    fn auto_scroll_anvil_grid(&self, state: &mut crate::game::GameState) {
        let columns = 4;
        let cell_h = 106.0_f32;
        let gap = 6.0_f32;
        let row = state.ui_state.anvil_selected_recipe / columns;
        let item_top = row as f32 * (cell_h + gap);
        let item_bottom = item_top + cell_h;

        let (_, sh) = crate::util::virtual_screen_size();
        let panel_h = (480.0_f32).min(sh - 16.0);
        let content_h = panel_h - 8.0 - 40.0 - 28.0 - 30.0 - 16.0 - 44.0;

        if item_top < state.ui_state.anvil_scroll_offset {
            state.ui_state.anvil_scroll_offset = item_top;
        }
        if item_bottom > state.ui_state.anvil_scroll_offset + content_h {
            state.ui_state.anvil_scroll_offset = item_bottom - content_h;
        }
    }
}
