use super::*;
use macroquad::miniquad::{self, EventHandler};
use std::sync::OnceLock;

/// True when a key event is a clipboard-paste combo (Ctrl+V on Windows/Linux, Cmd+V on macOS).
///
/// This is decided from the modifier flags carried on the *event itself*, which every miniquad
/// backend derives from live OS state (`GetKeyState` on Windows, `modifierFlags` on macOS,
/// `event.ctrlKey/metaKey` on web). That sidesteps a cross-platform bug: none of the backends
/// reset keyboard state on focus loss, so a Ctrl/Cmd held while the window loses focus (alt-tab,
/// Cmd-tab, browser tab switch) leaves the key stuck "down" in the polled `is_key_down` state
/// forever — making every later `V` press look like paste. The event's own mods are never stuck.
pub(super) fn is_paste_combo(keycode: miniquad::KeyCode, mods: miniquad::KeyMods) -> bool {
    keycode == miniquad::KeyCode::V && (mods.ctrl || mods.logo)
}

/// Replays this frame's raw input events looking for a genuine paste combo.
#[derive(Default)]
struct PasteWatcher {
    paste_requested: bool,
}

impl EventHandler for PasteWatcher {
    fn update(&mut self) {}
    fn draw(&mut self) {}
    fn key_down_event(
        &mut self,
        keycode: miniquad::KeyCode,
        mods: miniquad::KeyMods,
        _repeat: bool,
    ) {
        if is_paste_combo(keycode, mods) {
            self.paste_requested = true;
        }
    }
}

/// Returns whether a Ctrl/Cmd+V was pressed since the previous call. Lazily registers a single
/// shared input subscriber. Call once per frame.
pub(super) fn poll_paste_request() -> bool {
    static SUBSCRIBER: OnceLock<usize> = OnceLock::new();
    let subscriber = *SUBSCRIBER.get_or_init(macroquad::input::utils::register_input_subscriber);
    let mut watcher = PasteWatcher::default();
    macroquad::input::utils::repeat_all_miniquad_input(&mut watcher, subscriber);
    watcher.paste_requested
}

/// Convert a character index to a byte index in a UTF-8 string.
pub(super) fn char_to_byte_index(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(byte_idx, _)| byte_idx)
        .unwrap_or(s.len())
}

/// Check if a key should fire (initial press or auto-repeat).
pub(super) fn chat_key_should_fire(key: KeyCode, state: &mut GameState, current_time: f64) -> bool {
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
pub(super) fn process_chat_keyboard_input(
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
            let (send_text, channel) = if let Some(global_text) = text.strip_prefix('~') {
                let trimmed = global_text.trim().to_string();
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

    // Paste from clipboard (Ctrl+V / Cmd+V). Driven by the event's live modifier flags
    // (see poll_paste_request) rather than is_key_down, which the OS leaves stuck "down" after
    // focus loss with the key held — otherwise every plain `v` would paste.
    if state.ui_state.paste_requested {
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
    if chat_key_should_fire(KeyCode::Backspace, state, current_time)
        && state.ui_state.chat_cursor > 0
    {
        let byte_idx =
            char_to_byte_index(&state.ui_state.chat_input, state.ui_state.chat_cursor - 1);
        state.ui_state.chat_input.remove(byte_idx);
        state.ui_state.chat_cursor -= 1;
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
pub(super) const OBELISK_GID: u32 = 1958;

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
/// Assign an item or spell to the first available mobile hotkey slot (0-2).
/// If all slots are full, replaces slot 0 (cycles).
pub(super) fn assign_to_mobile_hotkey(
    state: &mut GameState,
    binding: crate::game::hotkey::HotkeySlotBinding,
) {
    use crate::game::hotkey::HotkeySlotBinding;
    let preset = state.ui_state.hotkey_bar.active_mut();
    // Find first empty slot among the 3 mobile slots
    for i in 0..3 {
        if matches!(preset.slots[i], HotkeySlotBinding::Empty) {
            preset.slots[i] = binding;
            save_current_ui_settings(state);
            return;
        }
    }
    // All full — replace slot 0
    preset.slots[0] = binding;
    save_current_ui_settings(state);
}

pub(super) fn save_current_ui_settings(state: &GameState) {
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
        quest_tracker_minimized: state.ui_state.quest_tracker_minimized,
        hide_system_in_public: state.ui_state.hide_system_in_public,
    };
    save_ui_settings(&settings);
}

/// Convert screen coordinates to virtual coordinates for UI hit detection
pub(super) fn screen_to_virtual_coords(x: f32, y: f32) -> (f32, f32) {
    let (vw, vh) = virtual_screen_size();
    let screen_w = screen_width();
    let screen_h = screen_height();
    (x * vw / screen_w, y * vh / screen_h)
}

pub(super) fn latest_chat_timestamp_for_channel(state: &GameState, channel: ChatChannel) -> f64 {
    state.ui_state.chat_messages.latest_timestamp(&channel)
}

pub(super) fn mark_chat_channel_as_read(state: &mut GameState, channel: ChatChannel) {
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

pub(super) const AUTO_ACTION_NPC_SETTLE_EPS: f32 = 0.06;

pub(super) fn queue_face(state: &mut GameState, commands: &mut Vec<InputCommand>, direction: u8) {
    let dir = crate::game::Direction::from_u8(direction).to_cardinal();
    commands.push(InputCommand::Face {
        direction: dir as u8,
    });
    if let Some(local_id) = &state.local_player_id {
        if let Some(player) = state.players.get_mut(local_id) {
            player.direction = dir;
            player.animation.direction = dir;
        }
    }
}

pub(super) fn auto_action_target_settled(
    aa: &crate::game::AutoActionState,
    state: &GameState,
) -> bool {
    if aa.target_type != "npc" {
        return true;
    }
    state.npcs.get(&aa.target_id).is_none_or(|npc| {
        let dx = (npc.x - npc.target_x).abs();
        let dy = (npc.y - npc.target_y).abs();
        dx.max(dy) <= AUTO_ACTION_NPC_SETTLE_EPS
    })
}

/// Activate a hotkey slot binding — returns InputCommand(s) to execute
pub(super) fn activate_hotkey_slot(state: &mut GameState, slot_idx: usize) -> Vec<InputCommand> {
    use crate::game::hotkey::HotkeySlotBinding;

    let binding = state.ui_state.hotkey_bar.active().slots[slot_idx].clone();
    match binding {
        HotkeySlotBinding::Empty => vec![],
        HotkeySlotBinding::Item { ref item_id } => {
            if let Some(inv_idx) = state.inventory.find_slot_by_item_id(item_id) {
                let item_def = state.item_registry.get_or_placeholder(item_id);
                if item_id.ends_with("_seed") {
                    // Seeds: plant on the farming patch the player is standing on
                    if let Some(player) = state.get_local_player() {
                        let px = player.x.round() as i32;
                        let py = player.y.round() as i32;
                        if let Some(patch_id) = state.farming_patch_positions.get(&(px, py)) {
                            if let Some(patch) = state.farming_patches.get(patch_id) {
                                if patch.state == "empty" {
                                    vec![InputCommand::PlantSeed {
                                        patch_id: patch_id.clone(),
                                        item_id: item_id.clone(),
                                    }]
                                } else {
                                    state.push_system_chat(
                                        "This patch already has something planted.".to_string(),
                                    );
                                    vec![]
                                }
                            } else {
                                vec![]
                            }
                        } else {
                            state.push_system_chat(
                                "Stand on a farming patch to plant seeds.".to_string(),
                            );
                            vec![]
                        }
                    } else {
                        vec![]
                    }
                } else if item_def.equipment.is_some() {
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
                .is_some_and(|&t| now < t);
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

pub(super) fn is_adventurer_guide_dialogue(speaker: &str) -> bool {
    speaker.eq_ignore_ascii_case("Adventurer Guide")
}

pub(super) fn is_adventure_board_dialogue(dialogue: &ActiveDialogue) -> bool {
    dialogue.quest_id.starts_with("adventure_board:")
        || dialogue.speaker.eq_ignore_ascii_case("Adventure Board")
}

pub(super) fn adventurer_guide_tier_id(tab_idx: usize, tier_idx: usize) -> Option<&'static str> {
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

pub(super) fn is_adventurer_guide_tier_id(quest_id: &str) -> bool {
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

pub(super) fn has_active_adventurer_guide_task(state: &GameState) -> bool {
    state
        .ui_state
        .active_quests
        .iter()
        .any(|q| is_adventurer_guide_tier_id(&q.id))
}

pub(super) fn is_selected_adventurer_guide_tier_active(state: &GameState) -> bool {
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

pub(super) fn is_selected_adventurer_guide_tier_completable(state: &GameState) -> bool {
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

pub(super) fn adventurer_guide_actions_locked(state: &GameState) -> bool {
    has_active_adventurer_guide_task(state) && !is_selected_adventurer_guide_tier_active(state)
}

pub(super) fn is_combat_tier_unlocked(state: &GameState, tier_idx: usize) -> bool {
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

pub(super) fn should_auto_open_selected_combat_tier_offer(
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

pub(super) fn sync_adventurer_guide_dialogue_target(state: &mut GameState) {
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
pub(super) fn auto_action_target_pos(
    aa: &crate::game::AutoActionState,
    state: &GameState,
) -> Option<(f32, f32)> {
    match aa.target_type.as_str() {
        // Use server-authoritative positions for chase logic to avoid interpolation mismatches
        "npc" => state.npcs.get(&aa.target_id).and_then(|n| {
            // For multi-tile NPCs, target the nearest tile of the footprint
            let player = state.get_local_player()?;
            let px = player.server_x.round() as i32;
            let py = player.server_y.round() as i32;
            let nx = n.server_x.round() as i32;
            let ny = n.server_y.round() as i32;
            let closest_x = px.clamp(nx, nx + n.size - 1);
            let closest_y = py.clamp(ny, ny + n.size - 1);
            Some((closest_x as f32, closest_y as f32))
        }),
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

#[cfg(test)]
mod tests {
    use super::is_paste_combo;
    use macroquad::miniquad::{KeyCode, KeyMods};

    fn mods(ctrl: bool, logo: bool) -> KeyMods {
        KeyMods {
            shift: false,
            ctrl,
            alt: false,
            logo,
        }
    }

    #[test]
    fn ctrl_v_and_cmd_v_are_paste() {
        assert!(is_paste_combo(KeyCode::V, mods(true, false))); // Ctrl+V (Win/Linux)
        assert!(is_paste_combo(KeyCode::V, mods(false, true))); // Cmd+V (macOS)
    }

    #[test]
    fn plain_v_is_not_paste() {
        // The bug case: V pressed with no live modifier flags must not paste, even if the
        // persistent key-down state thinks Ctrl/Cmd is held.
        assert!(!is_paste_combo(KeyCode::V, mods(false, false)));
    }

    #[test]
    fn modifier_without_v_is_not_paste() {
        assert!(!is_paste_combo(KeyCode::C, mods(true, false)));
    }
}
