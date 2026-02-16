use macroquad::prelude::*;
use std::collections::HashSet;
use crate::game::{GameState, ChatChannel, ContextMenu, ContextMenuTarget, DragState, DragSource, GoldDropDialog, BankQuantityDialog, BankQuantityAction, PathState, pathfinding};
use crate::render::animation::AnimationState;
use crate::render::isometric::screen_to_world;
use crate::ui::{UiElementId, UiLayout};
use crate::network::messages::ClientMessage;
use crate::audio::AudioManager;
use crate::util::virtual_screen_size;
use crate::settings::{UiSettings, save_ui_settings};
use super::touch::TouchControls;

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

/// Build set of tiles occupied by entities (other players + NPCs) for pathfinding
fn build_occupied_set(state: &GameState) -> HashSet<(i32, i32)> {
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
                occupied.insert((player.server_x.round() as i32, player.server_y.round() as i32));
            }
        }
    }

    // NPCs are NOT added to the occupied set — the server displaces NPCs
    // when a player walks onto their tile, so paths can route through them.

    occupied
}

/// Input commands that can be sent to the server
#[derive(Debug, Clone)]
pub enum InputCommand {
    Move { dx: f32, dy: f32 },
    Face { direction: u8 },
    Attack,
    Target { entity_id: String },
    ClearTarget,
    Chat { text: String, channel: String },
    Pickup { item_id: String },
    UseItem { slot_index: u8 },
    // Quest commands
    Interact { npc_id: String },
    DialogueChoice { quest_id: String, choice_id: String },
    CloseDialogue,
    // Crafting commands
    Craft { recipe_id: String },
    CancelCraft,
    // Equipment commands
    Equip { slot_index: u8 },
    Unequip { slot_type: String, target_slot: Option<u8> },
    // Inventory commands
    DropItem { slot_index: u8, quantity: u32, target_x: Option<i32>, target_y: Option<i32> },
    DropGold { amount: i32 },
    SwapSlots { from_slot: u8, to_slot: u8 },
    // Shop commands
    ShopBuy { npc_id: String, item_id: String, quantity: u32 },
    ShopSell { npc_id: String, item_id: String, quantity: u32 },
    // Bank commands
    BankDeposit { item_id: String, quantity: i32 },
    BankWithdraw { item_id: String, quantity: i32 },
    BankDepositGold { amount: i32 },
    BankWithdrawGold { amount: i32 },
    // Portal commands
    EnterPortal { portal_id: String },
    // Gathering commands
    StartGathering { marker_x: i32, marker_y: i32 },
    StopGathering,
    // Woodcutting commands
    ChopTree { tree_x: i32, tree_y: i32, tree_gid: u32 },
    // Chair commands
    SitChair { tile_x: i32, tile_y: i32 },
    StandUp,
    // Farming commands
    PlantSeed { patch_id: String, item_id: String },
    HarvestCrop { patch_id: String },
    // Friend system commands
    SendFriendRequest { target_name: String },
    AcceptFriendRequest { requester_id: i64 },
    DeclineFriendRequest { requester_id: i64 },
    RemoveFriend { friend_id: i64 },
    GetOnlinePlayers,
    // Prayer commands
    TogglePrayer { prayer_id: String },
    BuryBones { slot: u8 },
    // Altar commands
    OfferBones { slot: u8, altar_id: String },
    OfferAllBones { item_id: String, altar_id: String },
    PrayAtAltar { altar_id: String },
    // Spell commands
    CastSpell { spell_id: String },
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

pub struct InputHandler {
    // Track last sent velocity to detect changes
    last_dx: f32,
    last_dy: f32,
    // Track which direction was pressed first (for priority)
    current_dir: CardinalDir,
    // Track previous direction for detecting key release
    prev_dir: CardinalDir,
    // Send commands at server tick rate
    last_send_time: f64,
    send_interval: f64,
    // Attack cooldown tracking (matches server cooldown)
    last_attack_time: f64,
    attack_cooldown: f64,
    // Track when current direction key was pressed (for face vs move)
    dir_press_time: f64,
    // Track if we've sent a move command for the current key press
    move_sent: bool,
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
            send_interval: 0.05, // 50ms = 20Hz (matches server tick rate)
            last_attack_time: 0.0,
            attack_cooldown: 0.8, // 800 ms (matches server ATTACK_COOLDOWN_MS)
            dir_press_time: 0.0,
            move_sent: false,
            touch_controls: TouchControls::new(),
        }
    }

    /// Load touch control icons (call once after creation in async context)
    pub async fn load_touch_icons(&mut self) {
        self.touch_controls.load_icons().await;
    }

    pub fn process(&mut self, state: &mut GameState, layout: &UiLayout, audio: &mut AudioManager) -> Vec<InputCommand> {
        let mut commands = Vec::new();
        let current_time = get_time();

        // Update touch controls (for mobile)
        let in_dialogue = state.ui_state.active_dialogue.is_some();
        let any_panel_open = state.ui_state.inventory_open
            || state.ui_state.character_panel_open
            || state.ui_state.skills_open
            || state.ui_state.prayer_book_open
            || state.ui_state.escape_menu_open
            || state.ui_state.crafting_open
            || state.ui_state.shop_data.is_some()
            || state.ui_state.bank_open
            || state.ui_state.quest_log_open
            || state.ui_state.social_open
            || state.ui_state.chat_panel_open
            || in_dialogue;
        let hide_action_buttons = any_panel_open;
        let hide_direction_controls = state.ui_state.escape_menu_open
            || state.ui_state.crafting_open
            || state.ui_state.shop_data.is_some()
            || state.ui_state.bank_open
            || state.ui_state.quest_log_open
            || in_dialogue;
        self.touch_controls.update(current_time, hide_action_buttons, hide_direction_controls, state.ui_state.use_joystick);

        // Get current mouse/touch position in virtual coordinates (for UI hit detection)
        let (raw_mx, raw_my) = mouse_position();
        let (mx, my) = screen_to_virtual_coords(raw_mx, raw_my);

        // Update hover state for visual feedback (used by renderer next frame)
        state.ui_state.hovered_element = layout.hit_test(mx, my).cloned();

        // Update hovered tile based on mouse position (only when not hovering UI or using touch controls)
        // Use round() instead of floor() because tile sprites are visually centered
        // at integer world coordinates, forming diamonds that span [-0.5, 0.5) around each point
        let touch_active = self.touch_controls.consumed_touch();
        if state.ui_state.hovered_element.is_none() && !touch_active {
            let (world_x, world_y) = screen_to_world(mx, my, &state.camera);
            let tile_x = world_x.round() as i32;
            let tile_y = world_y.round() as i32;
            state.hovered_tile = Some((tile_x, tile_y));

            // Check for entity hover (players and NPCs within hover radius)
            let hover_radius = 0.6; // World units - slightly larger than tile size for easier targeting
            let mut hovered_entity: Option<String> = None;

            // Check NPCs first (they're usually more important to interact with)
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

            // Check players if no NPC is hovered
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

        // For click detection, do a fresh hit-test at the moment of click
        // This ensures we detect what's actually under the mouse when clicked
        // On mobile, don't count touches that were consumed by touch controls as map clicks
        let touch_consumed = self.touch_controls.consumed_touch();
        let mouse_clicked = is_mouse_button_pressed(MouseButton::Left) && !touch_consumed;
        let mouse_right_clicked = is_mouse_button_pressed(MouseButton::Right);
        let mouse_released = is_mouse_button_released(MouseButton::Left) && !touch_consumed;
        let clicked_element = if mouse_clicked || mouse_right_clicked || mouse_released {
            layout.hit_test(mx, my).cloned()
        } else {
            None
        };

        // Toggle debug mode
        if is_key_pressed(KeyCode::F3) {
            // Debug toggle handled in main loop
        }

        // Handle drag and drop for inventory slot rearrangement and equipment
        if mouse_released {
            if let Some(drag) = state.ui_state.drag_state.take() {
                // Drag completed - check if we're over a valid drop target
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::InventorySlot(to_idx) => {
                            match &drag.source {
                                DragSource::Inventory(from_idx) => {
                                    // Swap inventory slots if dropping on a different slot
                                    if *from_idx != *to_idx {
                                        // Optimistic update: immediately swap locally
                                        state.inventory.swap_slots(*from_idx, *to_idx);
                                        audio.play_sfx("item_put");

                                        commands.push(InputCommand::SwapSlots {
                                            from_slot: *from_idx as u8,
                                            to_slot: *to_idx as u8,
                                        });
                                    }
                                }
                                DragSource::Equipment(slot_type) => {
                                    // Dragging from equipment to inventory - unequip to specific slot
                                    // Optimistic update: immediately move to inventory and clear equipment
                                    if state.inventory.slots.get(*to_idx).map(|s| s.is_none()).unwrap_or(false) {
                                        state.inventory.set_slot(*to_idx, drag.item_id.clone(), drag.quantity);

                                        // Update player's equipped state optimistically
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
                            }
                        }
                        UiElementId::QuickSlot(_slot_idx) => {
                            // Quick slots are now fixed to inventory positions or spell bar;
                            // no drag-drop assignment needed.
                        }
                        UiElementId::EquipmentSlot(target_slot_type) => {
                            match &drag.source {
                                DragSource::Inventory(from_idx) => {
                                    // Dragging from inventory to equipment - equip if valid slot type
                                    // First check if player meets requirements before optimistic update
                                    let item_def = state.item_registry.get_or_placeholder(&drag.item_id);
                                    let can_equip = if let Some(ref equip) = item_def.equipment {
                                        // Check slot type matches target
                                        let slot_matches = equip.slot_type == *target_slot_type;
                                        // Check level requirement - combat level covers all requirements
                                        let level_required = equip.attack_level_required.max(equip.defence_level_required);
                                        let level_ok = state.get_local_player()
                                            .map(|p| p.skills.combat.level >= level_required)
                                            .unwrap_or(false);
                                        slot_matches && level_ok
                                    } else {
                                        false // Not equippable
                                    };

                                    if can_equip {
                                        // Optimistic update: immediately equip and clear inventory slot
                                        if let Some(local_id) = &state.local_player_id.clone() {
                                            if let Some(player) = state.players.get_mut(local_id) {
                                                match target_slot_type.as_str() {
                                                    "head" => player.equipped_head = Some(drag.item_id.clone()),
                                                    "body" => player.equipped_body = Some(drag.item_id.clone()),
                                                    "weapon" => player.equipped_weapon = Some(drag.item_id.clone()),
                                                    "back" => player.equipped_back = Some(drag.item_id.clone()),
                                                    "feet" => player.equipped_feet = Some(drag.item_id.clone()),
                                                    "ring" => player.equipped_ring = Some(drag.item_id.clone()),
                                                    "gloves" => player.equipped_gloves = Some(drag.item_id.clone()),
                                                    "necklace" => player.equipped_necklace = Some(drag.item_id.clone()),
                                                    "belt" => player.equipped_belt = Some(drag.item_id.clone()),
                                                    _ => {}
                                                }
                                            }
                                        }
                                        state.inventory.clear_slot(*from_idx);
                                        audio.play_sfx("item_put");

                                        commands.push(InputCommand::Equip { slot_index: *from_idx as u8 });
                                    }
                                    // If can't equip, drag is cancelled - item stays in inventory
                                }
                                DragSource::Equipment(source_slot_type) => {
                                    // Dragging from equipment slot to another equipment slot
                                    // Only makes sense if they're different types, otherwise no-op
                                    if source_slot_type != target_slot_type {
                                        // Can't swap different equipment slot types directly
                                        // Would need unequip + equip, which isn't supported
                                    }
                                }
                            }
                        }
                        _ => {
                            // Dropped on non-inventory UI element, cancel drag
                        }
                    }
                } else {
                    // No UI element under cursor - check for world tile drop
                    // Use the already-computed hovered_tile for consistency with visual feedback
                    if let DragSource::Inventory(from_idx) = &drag.source {
                        if let Some((tile_x, tile_y)) = state.hovered_tile {
                            // Get player position
                            if let Some(player) = state.get_local_player() {
                                let player_x = player.x.round() as i32;
                                let player_y = player.y.round() as i32;

                                // Check Chebyshev distance (must be exactly 1 - adjacent/diagonal)
                                let dx = (tile_x - player_x).abs();
                                let dy = (tile_y - player_y).abs();
                                let is_adjacent = dx <= 1 && dy <= 1 && (dx > 0 || dy > 0);

                                if is_adjacent {
                                    // Check if dropping a seed onto a farming patch
                                    let is_seed_on_patch = if let Some(patch_id) = state.farming_patch_positions.get(&(tile_x, tile_y)) {
                                        if let Some(patch) = state.farming_patches.get(patch_id) {
                                            if patch.state == "empty" {
                                                // Check if dragged item is a seed
                                                if let Some(Some(slot)) = state.inventory.slots.get(*from_idx) {
                                                    if slot.item_id.ends_with("_seed") {
                                                        commands.push(InputCommand::PlantSeed {
                                                            patch_id: patch_id.clone(),
                                                            item_id: slot.item_id.clone(),
                                                        });
                                                        audio.play_sfx("item_put");
                                                        true
                                                    } else { false }
                                                } else { false }
                                            } else { false }
                                        } else { false }
                                    } else { false };

                                    // Check if dropping bones onto an altar NPC
                                    let is_bones_on_altar = if !is_seed_on_patch {
                                        if let Some(Some(slot)) = state.inventory.slots.get(*from_idx) {
                                            if slot.item_id.contains("bones") {
                                                // Find altar NPC at this tile
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
                                                } else { false }
                                            } else { false }
                                        } else { false }
                                    } else { false };

                                    if !is_seed_on_patch && !is_bones_on_altar {
                                        // Check for Ctrl/Cmd modifier for single item drop
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
                    // Equipment drag to world is not supported - just cancel
                }
                // Drag ended (either completed swap or cancelled)
                return commands;
            }
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
                            // Check for shift+click to drop (if enabled)
                            let shift_held = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
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
                            let is_double_click = state.ui_state.double_click_state.last_click_slot == Some(*idx)
                                && current_time - state.ui_state.double_click_state.last_click_time < DOUBLE_CLICK_THRESHOLD;

                            if is_double_click {
                                // Reset double-click state
                                state.ui_state.double_click_state.last_click_slot = None;
                                state.ui_state.double_click_state.last_click_time = 0.0;

                                // Check if item is equippable
                                let item_def = state.item_registry.get_or_placeholder(&slot.item_id);
                                if item_def.equipment.is_some() {
                                    // Equip the item
                                    commands.push(InputCommand::Equip { slot_index: *idx as u8 });
                                    return commands;
                                } else {
                                    // Not equippable - use the item instead (e.g., health potion)
                                    commands.push(InputCommand::UseItem { slot_index: *idx as u8 });
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
                        // In item mode, quick slots map directly to inventory slots 0-4
                        if !state.ui_state.spell_bar_active {
                            let inv_idx = *idx;
                            if let Some(Some(slot)) = state.inventory.slots.get(inv_idx) {
                                // Check for shift+click to drop (if enabled)
                                let shift_held = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                                if shift_held && state.ui_state.shift_drop_enabled {
                                    commands.push(InputCommand::DropItem {
                                        slot_index: inv_idx as u8,
                                        quantity: slot.quantity as u32,
                                        target_x: None,
                                        target_y: None,
                                    });
                                    audio.play_sfx("item_put");
                                    return commands;
                                }

                                // Start drag from the inventory slot
                                state.ui_state.drag_state = Some(DragState {
                                    source: DragSource::Inventory(inv_idx),
                                    item_id: slot.item_id.clone(),
                                    quantity: slot.quantity,
                                });
                                audio.play_sfx("item_grab");
                                return commands;
                            }
                        }
                        // In spell mode, no drag from spell bar
                    }
                    UiElementId::EquipmentSlot(slot_type) => {
                        // Check if equipment slot has an item
                        let equipped_item = match slot_type.as_str() {
                            "head" => state.get_local_player().and_then(|p| p.equipped_head.clone()),
                            "body" => state.get_local_player().and_then(|p| p.equipped_body.clone()),
                            "weapon" => state.get_local_player().and_then(|p| p.equipped_weapon.clone()),
                            "back" => state.get_local_player().and_then(|p| p.equipped_back.clone()),
                            "feet" => state.get_local_player().and_then(|p| p.equipped_feet.clone()),
                            "ring" => state.get_local_player().and_then(|p| p.equipped_ring.clone()),
                            "gloves" => state.get_local_player().and_then(|p| p.equipped_gloves.clone()),
                            "necklace" => state.get_local_player().and_then(|p| p.equipped_necklace.clone()),
                            "belt" => state.get_local_player().and_then(|p| p.equipped_belt.clone()),
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
            let padding = 8.0;
            let header_height = 24.0;
            let option_height = 28.0;
            let menu_width = 120.0;

            // Calculate number of options (same logic as render_context_menu)
            let num_options = match &menu.target {
                ContextMenuTarget::EquipmentSlot(_) => 1, // Unequip only
                ContextMenuTarget::Gold => 1, // Drop only
                ContextMenuTarget::InventorySlot(slot_index) => {
                    let (is_equippable, is_bones) = state.inventory.slots.get(*slot_index)
                        .and_then(|s| s.as_ref())
                        .map(|slot| {
                            let item_def = state.item_registry.get_or_placeholder(&slot.item_id);
                            let equippable = item_def.equipment.is_some();
                            let bones = slot.item_id.contains("bones");
                            (equippable, bones)
                        })
                        .unwrap_or((false, false));
                    // [Equip?] [Bury?] Drop
                    1 + if is_equippable { 1 } else { 0 } + if is_bones { 1 } else { 0 }
                }
            };

            let content_height = num_options as f32 * option_height + padding;
            let menu_height = header_height + content_height + padding;

            // Apply same screen clamping as render_context_menu
            let mut menu_x = menu.x.floor();
            let mut menu_y = menu.y.floor();

            let screen_w = screen_width();
            let screen_h = screen_height();

            if menu_x + menu_width > screen_w {
                menu_x = (screen_w - menu_width - 5.0).floor();
            }
            if menu_y + menu_height > screen_h {
                menu_y = (screen_h - menu_height - 5.0).floor();
            }

            // Add some margin for easier interaction
            let margin = 4.0;
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
                                    let (is_equippable, is_bones) = state.inventory.slots.get(*slot_index)
                                        .and_then(|s| s.as_ref())
                                        .map(|slot| {
                                            let item_def = state.item_registry.get_or_placeholder(&slot.item_id);
                                            let equippable = item_def.equipment.is_some();
                                            let bones = slot.item_id.contains("bones");
                                            (equippable, bones)
                                        })
                                        .unwrap_or((false, false));

                                    // Build option index mapping: [Equip?] [Bury?] Drop
                                    let mut current_idx = 0usize;
                                    let equip_idx = if is_equippable { let idx = current_idx; current_idx += 1; Some(idx) } else { None };
                                    let bury_idx = if is_bones { let idx = current_idx; current_idx += 1; Some(idx) } else { None };
                                    let drop_idx = current_idx;

                                    if Some(*option_idx) == equip_idx {
                                        commands.push(InputCommand::Equip { slot_index: *slot_index as u8 });
                                    } else if Some(*option_idx) == bury_idx {
                                        commands.push(InputCommand::BuryBones { slot: *slot_index as u8 });
                                    } else if *option_idx == drop_idx {
                                        if let Some(slot) = state.inventory.slots.get(*slot_index).and_then(|s| s.as_ref()) {
                                            commands.push(InputCommand::DropItem { slot_index: *slot_index as u8, quantity: slot.quantity as u32, target_x: None, target_y: None });
                                        }
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
                        }
                        return commands;
                    }
                    UiElementId::MenuButtonSettings => {
                        audio.play_sfx("enter");
                        // Toggle escape/settings menu
                        state.ui_state.escape_menu_open = !state.ui_state.escape_menu_open;
                        return commands;
                    }
                    UiElementId::ChatButton => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_panel_open = !state.ui_state.chat_panel_open;
                        if state.ui_state.chat_panel_open {
                            state.ui_state.chat_active_tab = ChatChannel::Local;
                            state.ui_state.chat_message_scroll = 0.0;
                            // Close other panels
                            state.ui_state.inventory_open = false;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.social_open = false;
                            state.ui_state.prayer_book_open = false;
                        }
                    }
                    UiElementId::ChatTabLocal => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_active_tab = ChatChannel::Local;
                        state.ui_state.chat_message_scroll = 0.0;
                    }
                    UiElementId::ChatTabGlobal => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_active_tab = ChatChannel::Global;
                        state.ui_state.chat_message_scroll = 0.0;
                    }
                    UiElementId::ChatTabSystem => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_active_tab = ChatChannel::System;
                        state.ui_state.chat_message_scroll = 0.0;
                    }
                    UiElementId::ChatSendButton => {
                        // Block sending on System tab
                        if matches!(state.ui_state.chat_active_tab, ChatChannel::System) {
                            state.ui_state.chat_input.clear();
                            state.ui_state.chat_cursor = 0;
                        } else {
                            let text = state.ui_state.chat_input.trim().to_string();
                            // Determine channel: ~ prefix forces global, otherwise match active tab
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
                                commands.push(InputCommand::Chat { text: send_text, channel });
                            }
                            state.ui_state.chat_input.clear();
                            state.ui_state.chat_cursor = 0;
                        }
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
                                let nearby: Vec<_> = state.players.values()
                                    .filter(|p| Some(&p.id) != local_id)
                                    .collect();
                                nearby.get(*idx).map(|p| p.name.clone())
                            }
                            crate::game::SocialTab::Online => {
                                state.social_state.online_players.get(*idx).map(|p| p.name.clone())
                            }
                            _ => None,
                        };
                        if let Some(name) = player_name {
                            commands.push(InputCommand::SendFriendRequest { target_name: name });
                        }
                    }
                    UiElementId::SocialRequestAccept(idx) => {
                        audio.play_sfx("enter");
                        if let Some(request) = state.social_state.pending_requests.get(*idx).cloned() {
                            let requester_id = request.from_id;
                            let requester_name = request.from_name.clone();
                            commands.push(InputCommand::AcceptFriendRequest { requester_id });
                            // Remove from pending list immediately for responsive UI
                            state.social_state.pending_requests.remove(*idx);
                            state.social_state.pending_request_count = state.social_state.pending_requests.len();
                            // Also add to friends list immediately (they're online since they sent the request)
                            if !state.social_state.friends.iter().any(|f| f.id == requester_id) {
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
                            state.social_state.pending_request_count = state.social_state.pending_requests.len();
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
                    // Spell slot handlers (spell panel — info only, no drag)
                    UiElementId::SpellSlot(_slot_idx) => {
                        audio.play_sfx("enter");
                    }
                    // Spell/Item bar toggle button
                    UiElementId::SpellBarToggle => {
                        audio.play_sfx("enter");
                        state.ui_state.spell_bar_active = !state.ui_state.spell_bar_active;
                    }
                    // Prayer panel handlers
                    UiElementId::PrayerSlot(slot_idx) => {
                        // Toggle prayer at this slot
                        if *slot_idx < crate::game::prayer::PRAYERS.len() {
                            let prayer = &crate::game::prayer::PRAYERS[*slot_idx];
                            let prayer_level = state.get_local_player()
                                .map(|p| p.skills.prayer.level)
                                .unwrap_or(1);

                            // Check if player meets level requirement
                            if prayer_level >= prayer.level_req {
                                // Check if we have prayer points (can only activate if we have points)
                                let is_active = state.active_prayers.contains(&prayer.id.to_string());
                                if is_active || state.prayer_points > 0 {
                                    audio.play_sfx("enter");
                                    commands.push(InputCommand::TogglePrayer { prayer_id: prayer.id.to_string() });
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
                            if let Some(slider_elem) = layout.elements.iter().find(|e| e.id == UiElementId::EscapeMenuMusicSlider) {
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let volume = (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.audio_volume = volume;
                                audio.set_music_volume(volume);
                            }
                        }
                        Some(UiElementId::EscapeMenuSfxSlider) => {
                            if let Some(slider_elem) = layout.elements.iter().find(|e| e.id == UiElementId::EscapeMenuSfxSlider) {
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let volume = (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.audio_sfx_volume = volume;
                                audio.set_sfx_volume(volume);
                            }
                        }
                        Some(UiElementId::EscapeMenuUiScaleSlider) => {
                            if let Some(slider_elem) = layout.elements.iter().find(|e| e.id == UiElementId::EscapeMenuUiScaleSlider) {
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let normalized = (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.ui_scale = 0.75 + normalized * 0.5;
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
                            state.ui_state.escape_menu_open = false;
                            return commands;
                        }
                        UiElementId::EscapeMenuZoom1x => {
                            audio.play_sfx("enter");
                            state.camera.zoom = 1.0;
                            save_current_ui_settings(state);
                            state.ui_state.escape_menu_open = false;
                            return commands;
                        }
                        UiElementId::EscapeMenuZoom2x => {
                            audio.play_sfx("enter");
                            state.camera.zoom = 2.0;
                            save_current_ui_settings(state);
                            state.ui_state.escape_menu_open = false;
                            return commands;
                        }
                        UiElementId::EscapeMenuMusicSlider => {
                            // Start dragging and set initial value
                            state.ui_state.settings_slider_dragging = Some(UiElementId::EscapeMenuMusicSlider);
                            if let Some(slider_elem) = layout.elements.iter().find(|e| e.id == UiElementId::EscapeMenuMusicSlider) {
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
                            state.ui_state.settings_slider_dragging = Some(UiElementId::EscapeMenuSfxSlider);
                            if let Some(slider_elem) = layout.elements.iter().find(|e| e.id == UiElementId::EscapeMenuSfxSlider) {
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
                            state.ui_state.settings_slider_dragging = Some(UiElementId::EscapeMenuUiScaleSlider);
                            if let Some(slider_elem) = layout.elements.iter().find(|e| e.id == UiElementId::EscapeMenuUiScaleSlider) {
                                let (mouse_x, _) = mouse_position();
                                let relative_x = mouse_x - slider_elem.bounds.x;
                                let normalized = (relative_x / slider_elem.bounds.w).clamp(0.0, 1.0);
                                state.ui_state.ui_scale = 0.75 + normalized * 0.5;
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
                            state.ui_state.chat_log_background = !state.ui_state.chat_log_background;
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
                                state.ui_state.chat_cursor = state.ui_state.chat_input.chars().count();
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
                        _ => {
                            // Clicked somewhere else, close menu
                            state.ui_state.escape_menu_open = false;
                        }
                    }
                }
            } else if mouse_clicked {
                // Clicked outside any element, close menu
                state.ui_state.escape_menu_open = false;
            }

            // Escape closes escape menu
            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.escape_menu_open = false;
                return commands;
            }

            // Don't process other input while escape menu is open
            return commands;
        }

        // Handle gold drop dialog
        if state.ui_state.gold_drop_dialog.is_some() {
            // Handle button clicks
            if mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::GoldDropConfirm => {
                            // Parse amount and validate
                            let dialog = state.ui_state.gold_drop_dialog.as_ref().unwrap();
                            if let Ok(amount) = dialog.input.parse::<i32>() {
                                if amount > 0 && amount <= state.inventory.gold {
                                    commands.push(InputCommand::DropGold { amount });
                                    state.ui_state.gold_drop_dialog = None;
                                }
                            }
                            return commands;
                        }
                        UiElementId::GoldDropCancel => {
                            state.ui_state.gold_drop_dialog = None;
                            return commands;
                        }
                        _ => {}
                    }
                }
            }

            // Handle keyboard input
            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.gold_drop_dialog = None;
                return commands;
            }

            if is_key_pressed(KeyCode::Enter) {
                // Confirm with Enter key
                let dialog = state.ui_state.gold_drop_dialog.as_ref().unwrap();
                if let Ok(amount) = dialog.input.parse::<i32>() {
                    if amount > 0 && amount <= state.inventory.gold {
                        commands.push(InputCommand::DropGold { amount });
                        state.ui_state.gold_drop_dialog = None;
                    }
                }
                return commands;
            }

            // Number key input
            let number_keys = [
                (KeyCode::Key0, '0'), (KeyCode::Key1, '1'), (KeyCode::Key2, '2'),
                (KeyCode::Key3, '3'), (KeyCode::Key4, '4'), (KeyCode::Key5, '5'),
                (KeyCode::Key6, '6'), (KeyCode::Key7, '7'), (KeyCode::Key8, '8'),
                (KeyCode::Key9, '9'),
                (KeyCode::Kp0, '0'), (KeyCode::Kp1, '1'), (KeyCode::Kp2, '2'),
                (KeyCode::Kp3, '3'), (KeyCode::Kp4, '4'), (KeyCode::Kp5, '5'),
                (KeyCode::Kp6, '6'), (KeyCode::Kp7, '7'), (KeyCode::Kp8, '8'),
                (KeyCode::Kp9, '9'),
            ];

            for (key, digit) in &number_keys {
                if is_key_pressed(*key) {
                    let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                    // Limit input length (max 10 digits for gold amounts)
                    if dialog.input.len() < 10 {
                        dialog.input.insert(dialog.cursor, *digit);
                        dialog.cursor += 1;
                    }
                }
            }

            // Backspace
            if is_key_pressed(KeyCode::Backspace) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                if dialog.cursor > 0 {
                    dialog.input.remove(dialog.cursor - 1);
                    dialog.cursor -= 1;
                }
            }

            // Delete
            if is_key_pressed(KeyCode::Delete) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                if dialog.cursor < dialog.input.len() {
                    dialog.input.remove(dialog.cursor);
                }
            }

            // Left/Right arrow navigation
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

            // Home/End
            if is_key_pressed(KeyCode::Home) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                dialog.cursor = 0;
            }
            if is_key_pressed(KeyCode::End) {
                let dialog = state.ui_state.gold_drop_dialog.as_mut().unwrap();
                dialog.cursor = dialog.input.len();
            }

            // Drain character queue to prevent ghost characters
            while get_char_pressed().is_some() {}

            // Don't process other input while gold drop dialog is open
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
                            let altar_npc_id = state.ui_state.altar_panel.as_ref().unwrap().altar_npc_id.clone();
                            // Build bone rows to find item_id at index (mirrors renderer logic: dedup by item_id)
                            let mut bone_items: Vec<String> = Vec::new();
                            for slot in state.inventory.slots.iter().flatten() {
                                if !slot.item_id.contains("bones") { continue; }
                                let item_def = state.item_registry.get_or_placeholder(&slot.item_id);
                                if item_def.prayer_xp <= 0 { continue; }
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
                            let altar_npc_id = state.ui_state.altar_panel.as_ref().unwrap().altar_npc_id.clone();
                            commands.push(InputCommand::PrayAtAltar { altar_id: altar_npc_id });
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
            // Touch drag scrolling for dialogue choices on mobile
            let all_touches: Vec<Touch> = touches();
            if let Some(tracking_id) = state.ui_state.dialogue_touch_scroll_id {
                if let Some(touch) = all_touches.iter().find(|t| t.id == tracking_id) {
                    match touch.phase {
                        TouchPhase::Moved | TouchPhase::Stationary => {
                            let (_, vy) = screen_to_virtual_coords(touch.position.x, touch.position.y);
                            let dy = state.ui_state.dialogue_touch_last_y - vy;
                            if !state.ui_state.dialogue_touch_dragged {
                                let total_dy = (state.ui_state.dialogue_touch_start_y - vy).abs();
                                if total_dy > 8.0 {
                                    state.ui_state.dialogue_touch_dragged = true;
                                }
                            }
                            if state.ui_state.dialogue_touch_dragged {
                                state.ui_state.dialogue_scroll_offset = (state.ui_state.dialogue_scroll_offset + dy).max(0.0);
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
                            Some(UiElementId::DialogueChoice(_)) | Some(UiElementId::DialogueScrollbar)
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

            // Handle mouse scrollbar dragging (relative/delta-based)
            if state.ui_state.dialogue_scrollbar_dragging {
                if is_mouse_button_down(MouseButton::Left) {
                    let dy = my - state.ui_state.dialogue_scrollbar_drag_last_y;
                    if let Some(track_bounds) = layout.get_bounds(&UiElementId::DialogueScrollbar) {
                        let choice_spacing: f32 = if cfg!(target_os = "android") { 38.0 } else { 32.0 };
                        let total_content = dialogue.choices.len() as f32 * choice_spacing;
                        let scale = total_content / track_bounds.h;
                        state.ui_state.dialogue_scroll_offset = (state.ui_state.dialogue_scroll_offset + dy * scale).max(0.0);
                    }
                    state.ui_state.dialogue_scrollbar_drag_last_y = my;
                } else {
                    state.ui_state.dialogue_scrollbar_dragging = false;
                }
            } else if mouse_clicked {
                if matches!(clicked_element, Some(UiElementId::DialogueScrollbar)) {
                    state.ui_state.dialogue_scrollbar_dragging = true;
                    state.ui_state.dialogue_scrollbar_drag_last_y = my;
                }
            }

            // Handle mouse/touch clicks on dialogue elements
            // Skip if touch was a drag (scroll gesture) or scrollbar interaction
            let was_touch_drag = state.ui_state.dialogue_touch_dragged && state.ui_state.dialogue_touch_scroll_id.is_none();
            if was_touch_drag {
                state.ui_state.dialogue_touch_dragged = false;
            }
            let was_scrollbar = state.ui_state.dialogue_scrollbar_dragging;

            if !was_touch_drag && !was_scrollbar && mouse_clicked {
                if let Some(ref element) = clicked_element {
                    match element {
                        UiElementId::DialogueChoice(idx) => {
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
                            commands.push(InputCommand::DialogueChoice {
                                quest_id: dialogue.quest_id.clone(),
                                choice_id: "__continue__".to_string(),
                            });
                            return commands;
                        }
                        UiElementId::DialogueClose => {
                            if dialogue.quest_id != "__control_scheme__" {
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
                if is_key_pressed(KeyCode::Escape) && dialogue.quest_id != "__control_scheme__" {
                    commands.push(InputCommand::CloseDialogue);
                    state.ui_state.active_dialogue = None;
                    return commands;
                }

                // Number keys (1-4) select dialogue choices
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
                // Handle scroll wheel for dialogue choices
                let (_wheel_x, wheel_y) = mouse_wheel();
                if wheel_y.abs() > 0.0 {
                    state.ui_state.dialogue_scroll_offset = (state.ui_state.dialogue_scroll_offset - wheel_y * 20.0).max(0.0);
                }
            } else {
                // No choices - Escape, Enter, or Space to continue/close
                // Send __continue__ to server so Lua script can resume execution
                // Don't clear dialogue here - wait for server response (either new dialogue or close)
                if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
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
            if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
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
                                            commands.push(InputCommand::BankWithdrawGold { amount });
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
                (KeyCode::Key0, '0'), (KeyCode::Key1, '1'), (KeyCode::Key2, '2'),
                (KeyCode::Key3, '3'), (KeyCode::Key4, '4'), (KeyCode::Key5, '5'),
                (KeyCode::Key6, '6'), (KeyCode::Key7, '7'), (KeyCode::Key8, '8'),
                (KeyCode::Key9, '9'),
                (KeyCode::Kp0, '0'), (KeyCode::Kp1, '1'), (KeyCode::Kp2, '2'),
                (KeyCode::Kp3, '3'), (KeyCode::Kp4, '4'), (KeyCode::Kp5, '5'),
                (KeyCode::Kp6, '6'), (KeyCode::Kp7, '7'), (KeyCode::Kp8, '8'),
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
                        return commands;
                    }
                }
            }

            // Mouse wheel scrolling
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                const SCROLL_SPEED: f32 = 30.0;
                let row_height = 48.0 + 4.0; // INV_SLOT_SIZE + SLOT_SPACING

                match &state.ui_state.hovered_element {
                    Some(UiElementId::BankScrollArea) | Some(UiElementId::BankSlot(_)) => {
                        let total_rows = (state.ui_state.bank_slots.len() + 5) / 6; // BANK_COLS=6
                        let total_h = total_rows as f32 * row_height;
                        let max_scroll = (total_h - 200.0).max(0.0);
                        state.ui_state.bank_scroll = (state.ui_state.bank_scroll - wheel_y * SCROLL_SPEED).clamp(0.0, max_scroll);
                    }
                    Some(UiElementId::BankInvScrollArea) | Some(UiElementId::BankInventorySlot(_)) => {
                        let total_rows = (20 + 3) / 4; // 20 slots, INV_COLS=4
                        let total_h = total_rows as f32 * row_height;
                        let max_scroll = (total_h - 200.0).max(0.0);
                        state.ui_state.bank_inv_scroll = (state.ui_state.bank_inv_scroll - wheel_y * SCROLL_SPEED).clamp(0.0, max_scroll);
                    }
                    _ => {}
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
                            state.pending_sfx.push("enter".to_string());
                            return commands;
                        }
                        UiElementId::BankInventorySlot(slot_idx) => {
                            // Deposit item from inventory to bank
                            if let Some(Some(inv_slot)) = state.inventory.slots.get(*slot_idx) {
                                let ctrl_held = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
                                let shift_held = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                                if shift_held {
                                    // Shift+Click = deposit all
                                    commands.push(InputCommand::BankDeposit {
                                        item_id: inv_slot.item_id.clone(),
                                        quantity: inv_slot.quantity,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                } else if ctrl_held {
                                    // Ctrl+Click = deposit 1
                                    commands.push(InputCommand::BankDeposit {
                                        item_id: inv_slot.item_id.clone(),
                                        quantity: 1,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                } else {
                                    // Click = open quantity dialog
                                    state.ui_state.bank_quantity_dialog = Some(BankQuantityDialog {
                                        input: String::new(),
                                        cursor: 0,
                                        action: BankQuantityAction::DepositItem,
                                        item_id: Some(inv_slot.item_id.clone()),
                                        max_quantity: inv_slot.quantity,
                                    });
                                }
                            }
                            return commands;
                        }
                        UiElementId::BankSlot(idx) => {
                            // Withdraw item from bank to inventory
                            if let Some(Some((item_id, qty))) = state.ui_state.bank_slots.get(*idx) {
                                let ctrl_held = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
                                let shift_held = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                                if shift_held {
                                    // Shift+Click = withdraw all
                                    commands.push(InputCommand::BankWithdraw {
                                        item_id: item_id.clone(),
                                        quantity: *qty,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                } else if ctrl_held {
                                    // Ctrl+Click = withdraw 1
                                    commands.push(InputCommand::BankWithdraw {
                                        item_id: item_id.clone(),
                                        quantity: 1,
                                    });
                                    state.pending_sfx.push("enter".to_string());
                                } else {
                                    // Click = open quantity dialog
                                    state.ui_state.bank_quantity_dialog = Some(BankQuantityDialog {
                                        input: String::new(),
                                        cursor: 0,
                                        action: BankQuantityAction::WithdrawItem,
                                        item_id: Some(item_id.clone()),
                                        max_quantity: *qty,
                                    });
                                }
                            }
                            return commands;
                        }
                        UiElementId::BankDepositGoldButton => {
                            if state.inventory.gold > 0 {
                                let ctrl_held = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
                                let shift_held = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                                if shift_held {
                                    commands.push(InputCommand::BankDepositGold { amount: state.inventory.gold });
                                    state.pending_sfx.push("enter".to_string());
                                } else if ctrl_held {
                                    commands.push(InputCommand::BankDepositGold { amount: 1 });
                                    state.pending_sfx.push("enter".to_string());
                                } else {
                                    state.ui_state.bank_quantity_dialog = Some(BankQuantityDialog {
                                        input: String::new(),
                                        cursor: 0,
                                        action: BankQuantityAction::DepositGold,
                                        item_id: None,
                                        max_quantity: state.inventory.gold,
                                    });
                                }
                            }
                            return commands;
                        }
                        UiElementId::BankWithdrawGoldButton => {
                            if state.ui_state.bank_gold > 0 {
                                let ctrl_held = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
                                let shift_held = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                                if shift_held {
                                    commands.push(InputCommand::BankWithdrawGold { amount: state.ui_state.bank_gold });
                                    state.pending_sfx.push("enter".to_string());
                                } else if ctrl_held {
                                    commands.push(InputCommand::BankWithdrawGold { amount: 1 });
                                    state.pending_sfx.push("enter".to_string());
                                } else {
                                    state.ui_state.bank_quantity_dialog = Some(BankQuantityDialog {
                                        input: String::new(),
                                        cursor: 0,
                                        action: BankQuantityAction::WithdrawGold,
                                        item_id: None,
                                        max_quantity: state.ui_state.bank_gold,
                                    });
                                }
                            }
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
                        macroquad::input::TouchPhase::Moved | macroquad::input::TouchPhase::Stationary => {
                            let (_, vy) = screen_to_virtual_coords(touch.position.x, touch.position.y);
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
                                    let max_scroll = state.ui_state.shop_data.as_ref()
                                        .map(|d| ((d.stock.len() as f32) * item_height - 200.0).max(0.0))
                                        .unwrap_or(0.0);
                                    state.ui_state.shop_buy_scroll = (state.ui_state.shop_buy_scroll + dy).clamp(0.0, max_scroll);
                                } else {
                                    let inventory_count = state.inventory.slots.iter().filter(|s| s.is_some()).count();
                                    let max_scroll = ((inventory_count as f32) * item_height - 200.0).max(0.0);
                                    state.ui_state.shop_sell_scroll = (state.ui_state.shop_sell_scroll + dy).clamp(0.0, max_scroll);
                                }
                            }
                            state.ui_state.shop_touch_last_y = vy;
                        }
                        macroquad::input::TouchPhase::Ended | macroquad::input::TouchPhase::Cancelled => {
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
                        let buy_area = matches!(hit, Some(UiElementId::ShopBuyScrollArea) | Some(UiElementId::ShopBuyItem(_)));
                        let sell_area = matches!(hit, Some(UiElementId::ShopSellScrollArea) | Some(UiElementId::ShopSellItem(_)));
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
            let was_shop_touch_drag = state.ui_state.shop_touch_dragged && state.ui_state.shop_touch_scroll_id.is_none();
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
                        let categories: Vec<String> = {
                            let mut cats: Vec<String> = state.recipe_definitions.iter()
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
                        let selected_idx = state.ui_state.crafting_selected_category.min(categories.len().saturating_sub(1));
                        let current_category = categories.get(selected_idx).map(|s| s.as_str()).unwrap_or("supplies");
                        let recipes_in_category: Vec<&crate::game::RecipeDefinition> = state.recipe_definitions.iter()
                            .filter(|r| {
                                let cat_match = if current_category == "supplies" {
                                    r.category == "consumables" || r.category == "materials"
                                } else {
                                    r.category == current_category
                                };
                                // Only include discovered recipes (matching renderer)
                                let is_discovered = !r.requires_discovery || state.discovered_recipes.contains(&r.id);
                                cat_match && is_discovered
                            })
                            .collect();
                        if let Some(recipe) = recipes_in_category.get(state.ui_state.crafting_selected_recipe) {
                            log::info!("Crafting (click): {}", recipe.id);
                            commands.push(InputCommand::Craft { recipe_id: recipe.id.clone() });
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
                        state.ui_state.shop_quantity_hold_element = Some(UiElementId::ShopBuyQuantityMinus);
                        state.ui_state.shop_quantity_hold_start = current_time;
                        state.ui_state.shop_quantity_hold_last_repeat = current_time;
                        return commands;
                    }
                    UiElementId::ShopBuyQuantityPlus => {
                        state.ui_state.shop_buy_quantity += 1;
                        state.ui_state.shop_quantity_hold_element = Some(UiElementId::ShopBuyQuantityPlus);
                        state.ui_state.shop_quantity_hold_start = current_time;
                        state.ui_state.shop_quantity_hold_last_repeat = current_time;
                        return commands;
                    }
                    UiElementId::ShopSellQuantityMinus => {
                        if state.ui_state.shop_sell_quantity > 1 {
                            state.ui_state.shop_sell_quantity -= 1;
                        }
                        state.ui_state.shop_quantity_hold_element = Some(UiElementId::ShopSellQuantityMinus);
                        state.ui_state.shop_quantity_hold_start = current_time;
                        state.ui_state.shop_quantity_hold_last_repeat = current_time;
                        return commands;
                    }
                    UiElementId::ShopSellQuantityPlus => {
                        state.ui_state.shop_sell_quantity += 1;
                        state.ui_state.shop_quantity_hold_element = Some(UiElementId::ShopSellQuantityPlus);
                        state.ui_state.shop_quantity_hold_start = current_time;
                        state.ui_state.shop_quantity_hold_last_repeat = current_time;
                        return commands;
                    }
                    UiElementId::ShopSellQuantityMax => {
                        let inventory_items: Vec<_> = state.inventory.slots.iter()
                            .filter_map(|slot| slot.as_ref())
                            .collect();
                        if let Some(inv_slot) = inventory_items.get(state.ui_state.shop_selected_sell_index) {
                            state.ui_state.shop_sell_quantity = inv_slot.quantity.max(1);
                        }
                        return commands;
                    }
                    UiElementId::ShopBuyConfirmButton => {
                        if let Some(ref shop_data) = state.ui_state.shop_data {
                            if let Some(ref npc_id) = state.ui_state.shop_npc_id {
                                if let Some(stock_item) = shop_data.stock.get(state.ui_state.shop_selected_buy_index) {
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
                            let inventory_items: Vec<_> = state.inventory.slots.iter()
                                .filter_map(|slot| slot.as_ref())
                                .collect();
                            if let Some(inv_slot) = inventory_items.get(state.ui_state.shop_selected_sell_index) {
                                commands.push(InputCommand::ShopSell {
                                    npc_id: npc_id.clone(),
                                    item_id: inv_slot.item_id.clone(),
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
                            let since_last = current_time - state.ui_state.shop_quantity_hold_last_repeat;
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
                state.ui_state.shop_main_tab = if state.ui_state.shop_main_tab == 0 { 1 } else { 0 };
            }

            if state.ui_state.shop_main_tab == 0 {
                // Recipes tab keyboard controls
                // Get unique categories from recipes, merging consumables and materials
                let categories: Vec<String> = {
                    let mut cats: Vec<String> = state.recipe_definitions.iter()
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
                        if state.ui_state.crafting_selected_category < categories.len().saturating_sub(1) {
                            state.ui_state.crafting_selected_category += 1;
                            state.ui_state.crafting_selected_recipe = 0;
                            state.ui_state.crafting_scroll_offset = 0.0;
                        }
                    }

                    // Get discovered recipes for current category (matches renderer filtering)
                    let selected_idx = state.ui_state.crafting_selected_category.min(categories.len().saturating_sub(1));
                    let current_category = categories.get(selected_idx).map(|s| s.as_str()).unwrap_or("supplies");
                    let recipes_in_category: Vec<&crate::game::RecipeDefinition> = state.recipe_definitions.iter()
                        .filter(|r| {
                            let cat_match = if current_category == "supplies" {
                                r.category == "consumables" || r.category == "materials"
                            } else {
                                r.category == current_category
                            };
                            // Only include discovered recipes (matching renderer)
                            let is_discovered = !r.requires_discovery || state.discovered_recipes.contains(&r.id);
                            cat_match && is_discovered
                        })
                        .collect();

                    // Up/Down navigate recipes
                    let mut key_navigated = false;
                    if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                        if state.ui_state.crafting_selected_recipe > 0 {
                            state.ui_state.crafting_selected_recipe -= 1;
                            key_navigated = true;
                        }
                    }
                    if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                        if state.ui_state.crafting_selected_recipe < recipes_in_category.len().saturating_sub(1) {
                            state.ui_state.crafting_selected_recipe += 1;
                            key_navigated = true;
                        }
                    }

                    // Only auto-scroll when keyboard navigated, not every frame
                    if key_navigated {
                        let craft_line_h = 28.0_f32;
                        // Count the actual row position including undiscovered "????" entries
                        // The renderer shows ALL recipes (discovered + undiscovered) in order,
                        // but selection index only counts discovered ones
                        let all_in_category: Vec<&crate::game::RecipeDefinition> = state.recipe_definitions.iter()
                            .filter(|r| {
                                if current_category == "supplies" {
                                    r.category == "consumables" || r.category == "materials"
                                } else {
                                    r.category == current_category
                                }
                            })
                            .collect();
                        let mut row = 0usize;
                        let mut discovered_idx = 0usize;
                        for r in &all_in_category {
                            let is_disc = !r.requires_discovery || state.discovered_recipes.contains(&r.id);
                            if is_disc {
                                if discovered_idx == state.ui_state.crafting_selected_recipe {
                                    break;
                                }
                                discovered_idx += 1;
                            }
                            row += 1;
                        }
                        let item_top = row as f32 * craft_line_h;
                        let item_bottom = item_top + craft_line_h;
                        if item_top < state.ui_state.crafting_scroll_offset {
                            state.ui_state.crafting_scroll_offset = item_top;
                        }
                        // Match renderer: list_content_height = panel_height - 172
                        let (_, sh) = crate::util::virtual_screen_size();
                        let panel_h = (450.0_f32).min(sh - 16.0);
                        let visible_h = panel_h - 172.0;
                        if item_bottom > state.ui_state.crafting_scroll_offset + visible_h {
                            state.ui_state.crafting_scroll_offset = item_bottom - visible_h;
                        }
                    }

                    // Enter or C crafts selected recipe
                    if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::C) {
                        if let Some(recipe) = recipes_in_category.get(state.ui_state.crafting_selected_recipe) {
                            log::info!("Crafting: {}", recipe.id);
                            commands.push(InputCommand::Craft { recipe_id: recipe.id.clone() });
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
                    const SCROLL_SPEED: f32 = 30.0;
                    let line_height = 28.0;
                    // Count all recipes in category (discovered + undiscovered) to match renderer
                    let sel_idx = state.ui_state.crafting_selected_category.min(categories.len().saturating_sub(1));
                    let cur_cat = categories.get(sel_idx).map(|s| s.as_str()).unwrap_or("supplies");
                    let total_visible: usize = state.recipe_definitions.iter()
                        .filter(|r| if cur_cat == "supplies" { r.category == "consumables" || r.category == "materials" } else { r.category == cur_cat })
                        .count();
                    // Match renderer: list_content_height = list_height - 34, list_height = content_height - tab_height - 20
                    // content_height = panel_height - FRAME*2 - HEADER - FOOTER - 12, tab_height = 28
                    let (_, sh) = crate::util::virtual_screen_size();
                    let panel_height = (450.0_f32).min(sh - 16.0);
                    let content_height = panel_height - 8.0 - 32.0 - 28.0 - 12.0; // FRAME*2=8, HEADER=32, FOOTER=28
                    let list_height = content_height - 28.0 - 20.0; // tab_height=28
                    let list_content_height = list_height - 34.0;
                    let total_content = total_visible as f32 * line_height;
                    let max_scroll = (total_content - list_content_height).max(0.0);
                    state.ui_state.crafting_scroll_offset = (state.ui_state.crafting_scroll_offset - wheel_y * SCROLL_SPEED).clamp(0.0, max_scroll);
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
                        Some(UiElementId::ShopBuyScrollArea) | Some(UiElementId::ShopBuyItem(_)) => {
                            if let Some(ref shop_data) = state.ui_state.shop_data {
                                let max_scroll = ((shop_data.stock.len() as f32) * item_height - 200.0).max(0.0);
                                state.ui_state.shop_buy_scroll = (state.ui_state.shop_buy_scroll - wheel_y * SCROLL_SPEED).clamp(0.0, max_scroll);
                            }
                        }
                        Some(UiElementId::ShopSellScrollArea) | Some(UiElementId::ShopSellItem(_)) => {
                            let inventory_count = state.inventory.slots.iter().filter(|s| s.is_some()).count();
                            let max_scroll = ((inventory_count as f32) * item_height - 200.0).max(0.0);
                            state.ui_state.shop_sell_scroll = (state.ui_state.shop_sell_scroll - wheel_y * SCROLL_SPEED).clamp(0.0, max_scroll);
                        }
                        _ => {}
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
                        let item_count = state.ui_state.shop_data.as_ref()
                            .map(|d| d.stock.len())
                            .unwrap_or(0);

                        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                            if state.ui_state.shop_selected_buy_index > 0 {
                                state.ui_state.shop_selected_buy_index -= 1;
                                state.ui_state.shop_buy_quantity = 1;
                            }
                        }
                        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                            if state.ui_state.shop_selected_buy_index < item_count.saturating_sub(1) {
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
                                    if let Some(stock_item) = shop_data.stock.get(state.ui_state.shop_selected_buy_index) {
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
                        let inventory_items: Vec<_> = state.inventory.slots.iter()
                            .filter_map(|slot| slot.as_ref())
                            .collect();
                        let item_count = inventory_items.len();

                        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                            if state.ui_state.shop_selected_sell_index > 0 {
                                state.ui_state.shop_selected_sell_index -= 1;
                                state.ui_state.shop_sell_quantity = 1;
                            }
                        }
                        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                            if state.ui_state.shop_selected_sell_index < item_count.saturating_sub(1) {
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
                                if let Some(inv_slot) = inventory_items.get(state.ui_state.shop_selected_sell_index) {
                                    commands.push(InputCommand::ShopSell {
                                        npc_id: npc_id.clone(),
                                        item_id: inv_slot.item_id.clone(),
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

        // Handle social panel touch scrolling
        if state.ui_state.social_open {
            let all_touches: Vec<Touch> = touches();

            // Handle ongoing touch drag
            if let Some(tracking_id) = state.social_state.touch_scroll_id {
                if let Some(touch) = all_touches.iter().find(|t| t.id == tracking_id) {
                    match touch.phase {
                        TouchPhase::Moved | TouchPhase::Stationary => {
                            let (_, vy) = screen_to_virtual_coords(touch.position.x, touch.position.y);
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
                                    crate::game::SocialTab::Nearby | crate::game::SocialTab::Online => {
                                        state.social_state.list_scroll_offset = (state.social_state.list_scroll_offset + dy).max(0.0);
                                    }
                                    crate::game::SocialTab::Friends => {
                                        state.social_state.friends_scroll_offset = (state.social_state.friends_scroll_offset + dy).max(0.0);
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
                        if matches!(hit, Some(UiElementId::SocialScrollArea) | Some(UiElementId::SocialPlayerRow(_)) | Some(UiElementId::SocialFriendRow(_))) {
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
                match state.social_state.active_tab {
                    crate::game::SocialTab::Nearby | crate::game::SocialTab::Online => {
                        state.social_state.list_scroll_offset = (state.social_state.list_scroll_offset - wheel_y * scroll_speed).max(0.0);
                    }
                    crate::game::SocialTab::Friends => {
                        state.social_state.friends_scroll_offset = (state.social_state.friends_scroll_offset - wheel_y * scroll_speed).max(0.0);
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
                if c.is_control() || !c.is_ascii_graphic() && !c.is_ascii_whitespace() && !c.is_alphanumeric() {
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
            let classic = state.ui_state.classic_controls;

            // Helper to convert character index to byte index
            let char_to_byte_index = |s: &str, char_idx: usize| -> usize {
                s.char_indices()
                    .nth(char_idx)
                    .map(|(byte_idx, _)| byte_idx)
                    .unwrap_or(s.len())
            };

            // Key repeat timing constants
            const INITIAL_DELAY: f64 = 0.4; // 400ms before repeat starts
            const REPEAT_RATE: f64 = 0.035; // ~28 repeats per second

            let current_time = macroquad::time::get_time();

            // Escape cancels chat (in classic mode, Escape opens ESC menu instead - don't close chat)
            if is_key_pressed(KeyCode::Escape) {
                if classic {
                    // In classic mode, Escape toggles the ESC menu, chat stays open
                    state.ui_state.escape_menu_open = !state.ui_state.escape_menu_open;
                    return commands;
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
                return commands;
            }

            // Enter sends message
            if is_key_pressed(KeyCode::Enter) {
                // Block sending on System tab
                if matches!(state.ui_state.chat_active_tab, ChatChannel::System) {
                    state.ui_state.chat_input.clear();
                    state.ui_state.chat_cursor = 0;
                    state.ui_state.chat_scroll_offset = 0;
                } else {
                    let text = state.ui_state.chat_input.trim().to_string();
                    // Determine channel: ~ prefix forces global, otherwise match active tab
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
                        commands.push(InputCommand::Chat { text: send_text, channel });
                    }
                    state.ui_state.chat_input.clear();
                    state.ui_state.chat_cursor = 0;
                    state.ui_state.chat_scroll_offset = 0;
                }
                if classic {
                    // In classic mode, chat stays open after sending
                } else {
                    // Close keyboard input but keep chat panel open if it's showing
                    state.ui_state.chat_open = false;
                }
                #[cfg(target_os = "android")]
                macroquad::miniquad::window::show_keyboard(false);
                return commands;
            }

            let char_count = state.ui_state.chat_input.chars().count();

            // Check if any repeatable key is held
            let repeatable_keys = if classic {
                // In classic mode, arrow keys are for movement, not chat cursor
                vec![KeyCode::Backspace, KeyCode::Delete]
            } else {
                vec![KeyCode::Left, KeyCode::Right, KeyCode::Backspace, KeyCode::Delete]
            };
            let any_repeatable_held = repeatable_keys.iter().any(|k| is_key_down(*k));

            // Reset repeat state if no repeatable keys held
            if !any_repeatable_held {
                state.ui_state.chat_key_initial_delay = true;
            }

            // Helper to check if we should fire a key action (initial press or repeat)
            let should_fire = |key: KeyCode, state: &mut GameState, current_time: f64| -> bool {
                if is_key_pressed(key) {
                    // Initial press - always fire and start repeat timer
                    state.ui_state.chat_key_repeat_time = current_time;
                    state.ui_state.chat_key_initial_delay = true;
                    return true;
                } else if is_key_down(key) {
                    // Key held - check repeat timing
                    let delay = if state.ui_state.chat_key_initial_delay { INITIAL_DELAY } else { REPEAT_RATE };
                    if current_time - state.ui_state.chat_key_repeat_time >= delay {
                        state.ui_state.chat_key_repeat_time = current_time;
                        state.ui_state.chat_key_initial_delay = false;
                        return true;
                    }
                }
                false
            };

            // Arrow key navigation (drain char queue after to prevent ghost characters)
            // In classic mode, arrow keys are used for movement, not chat cursor
            if !classic {
                if should_fire(KeyCode::Left, state, current_time) {
                    if state.ui_state.chat_cursor > 0 {
                        state.ui_state.chat_cursor -= 1;
                    }
                    while get_char_pressed().is_some() {}
                }
                if should_fire(KeyCode::Right, state, current_time) {
                    let char_count = state.ui_state.chat_input.chars().count();
                    if state.ui_state.chat_cursor < char_count {
                        state.ui_state.chat_cursor += 1;
                    }
                    while get_char_pressed().is_some() {}
                }
            }
            // Home/End for quick navigation (no repeat needed)
            if is_key_pressed(KeyCode::Home) {
                state.ui_state.chat_cursor = 0;
                while get_char_pressed().is_some() {}
            }
            if is_key_pressed(KeyCode::End) {
                state.ui_state.chat_cursor = char_count;
                while get_char_pressed().is_some() {}
            }

            // Backspace removes character before cursor
            if should_fire(KeyCode::Backspace, state, current_time) {
                if state.ui_state.chat_cursor > 0 {
                    let byte_idx = char_to_byte_index(&state.ui_state.chat_input, state.ui_state.chat_cursor - 1);
                    state.ui_state.chat_input.remove(byte_idx);
                    state.ui_state.chat_cursor -= 1;
                }
            }

            // Delete removes character at cursor
            if should_fire(KeyCode::Delete, state, current_time) {
                let char_count = state.ui_state.chat_input.chars().count();
                if state.ui_state.chat_cursor < char_count {
                    let byte_idx = char_to_byte_index(&state.ui_state.chat_input, state.ui_state.chat_cursor);
                    state.ui_state.chat_input.remove(byte_idx);
                }
            }

            // Capture typed characters - insert at cursor position
            while let Some(c) = get_char_pressed() {
                // Filter control characters and non-printable special chars (like arrow key ghosts)
                if c.is_control() || !c.is_ascii_graphic() && !c.is_ascii_whitespace() && !c.is_alphanumeric() {
                    continue;
                }
                // Limit chat message length (by character count)
                if state.ui_state.chat_input.chars().count() < 200 {
                    let byte_idx = char_to_byte_index(&state.ui_state.chat_input, state.ui_state.chat_cursor);
                    state.ui_state.chat_input.insert(byte_idx, c);
                    state.ui_state.chat_cursor += 1;
                }
            }

            // In classic mode, don't return - fall through to movement/attack handling
            if !classic {
                return commands;
            }
        }

        // Handle chat panel scrolling and block game-world input
        if state.ui_state.chat_panel_open {
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 {
                const SCROLL_SPEED: f32 = 40.0; // Pixels per scroll tick
                let delta = wheel_y * SCROLL_SPEED;
                state.ui_state.chat_message_scroll = (state.ui_state.chat_message_scroll + delta).max(0.0);
            }
            return commands;
        }

        let classic = state.ui_state.classic_controls;

        // Enter key opens chat (not in classic mode - chat is always open)
        // Don't open chat on System tab (read-only)
        if !classic && is_key_pressed(KeyCode::Enter) && !matches!(state.ui_state.chat_active_tab, ChatChannel::System) {
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
        let up = if classic { is_key_down(KeyCode::Up) } else { is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) };
        let down = if classic { is_key_down(KeyCode::Down) } else { is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) };
        let left = if classic { is_key_down(KeyCode::Left) } else { is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) };
        let right = if classic { is_key_down(KeyCode::Right) } else { is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) };

        // Check for newly pressed keys this frame (last-key-wins priority)
        let up_just = if classic { is_key_pressed(KeyCode::Up) } else { is_key_pressed(KeyCode::W) || is_key_pressed(KeyCode::Up) };
        let down_just = if classic { is_key_pressed(KeyCode::Down) } else { is_key_pressed(KeyCode::S) || is_key_pressed(KeyCode::Down) };
        let left_just = if classic { is_key_pressed(KeyCode::Left) } else { is_key_pressed(KeyCode::A) || is_key_pressed(KeyCode::Left) };
        let right_just = if classic { is_key_pressed(KeyCode::Right) } else { is_key_pressed(KeyCode::D) || is_key_pressed(KeyCode::Right) };

        // Get touch D-pad input (for mobile)
        use crate::input::touch::DPadDirection;
        let dpad_dir = self.touch_controls.get_direction();
        let dpad_released = self.touch_controls.get_just_released_direction();
        let has_dpad_input = dpad_dir != DPadDirection::None;

        // Cancel auto-path if any movement input (keyboard or D-pad)
        if up || down || left || right || has_dpad_input {
            state.clear_auto_path();
        }

        // Determine new direction from keyboard - only one direction at a time
        // Newly pressed keys override current direction (last-key-wins),
        // then keep current direction if still held, then fall back to any held key
        let keyboard_dir = if up_just { CardinalDir::Up }
            else if down_just { CardinalDir::Down }
            else if left_just { CardinalDir::Left }
            else if right_just { CardinalDir::Right }
            else { match self.current_dir {
                CardinalDir::Up if up => CardinalDir::Up,
                CardinalDir::Down if down => CardinalDir::Down,
                CardinalDir::Left if left => CardinalDir::Left,
                CardinalDir::Right if right => CardinalDir::Right,
                _ => {
                    if up { CardinalDir::Up }
                    else if down { CardinalDir::Down }
                    else if left { CardinalDir::Left }
                    else if right { CardinalDir::Right }
                    else { CardinalDir::None }
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
                let hold_duration = current_time - self.dir_press_time;
                log::info!("[INPUT] Key released: hold_duration={:.3}s, threshold={:.3}s, move_sent={}",
                    hold_duration, FACE_THRESHOLD, self.move_sent);
                if self.move_sent {
                    // Was moving, now stopped - send stop command
                    commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                    self.last_dx = 0.0;
                    self.last_dy = 0.0;
                    self.last_send_time = current_time;
                } else {
                    // Never sent a move (quick tap or frame timing edge case) - send Face command
                    // But not if attacking - player must finish attack first
                    let attack_anim = state.get_local_player().map_or(false, |p| {
                        matches!(
                            p.animation.state,
                            AnimationState::Attacking | AnimationState::Casting | AnimationState::ShootingBow
                        )
                    });
                    if !attack_anim && !state.is_sitting {
                        let dir = self.prev_dir.to_direction_u8();
                        log::info!("[INPUT] Sending Face command: direction={} (prev_dir={:?})", dir, self.prev_dir);
                        commands.push(InputCommand::Face { direction: dir });
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
                        AnimationState::Attacking | AnimationState::Casting | AnimationState::ShootingBow
                    )
                });
                if !attack_anim && !state.is_sitting {
                    let dir = dpad_released.to_direction_u8();
                    log::info!("[INPUT] D-pad tap - sending Face command: direction={} (hold={:.0}ms)", dir, hold_duration * 1000.0);
                    commands.push(InputCommand::Face { direction: dir });
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
        let is_attacking = attack_key_down || self.touch_controls.attack_pressed() || state.get_local_player().map_or(false, |p| {
            matches!(
                p.animation.state,
                AnimationState::Attacking | AnimationState::Casting | AnimationState::ShootingBow
            )
        });

        // Check if we have any movement input (keyboard or D-pad)
        let has_movement_input = new_dir != CardinalDir::None;

        // Movement while sitting is handled server-side (direction-validated auto-stand)
        // Just let the move command go through - server will stand up if direction matches

        if has_movement_input && !is_attacking {
            // Determine hold duration based on input source
            let hold_duration = if has_dpad_input {
                current_time - self.touch_controls.get_dpad_press_time()
            } else {
                current_time - self.dir_press_time
            };
            let past_threshold = hold_duration >= FACE_THRESHOLD;

            if past_threshold {
                // Past threshold - check if target tile is walkable before sending movement
                // When sitting, only allow movement in the chair's facing direction (to stand up)
                let can_move = if state.is_sitting {
                    // Allow standing up by moving in the chair's facing direction
                    // The player's direction matches the chair's direction when sitting
                    if let Some(player) = state.get_local_player() {
                        let move_dir = new_dir.to_direction_u8();
                        let chair_dir = player.direction as u8;
                        move_dir == chair_dir
                    } else {
                        false
                    }
                } else if let Some(player) = state.get_local_player() {
                    // Use authoritative player tile, not interpolated visual position.
                    let player_x = player.server_x.round() as i32;
                    let player_y = player.server_y.round() as i32;
                    let target_x = player_x + dx as i32;
                    let target_y = player_y + dy as i32;

                    // Check static tile collision
                    let tile_walkable = state.chunk_manager.is_walkable(target_x as f32, target_y as f32);

                    // Check entity collision
                    let occupied = build_occupied_set(state);
                    let not_occupied = !occupied.contains(&(target_x, target_y));

                    tile_walkable && not_occupied
                } else {
                    false
                };

                let direction_changed = (dx - self.last_dx).abs() > 0.01 || (dy - self.last_dy).abs() > 0.01;
                let time_elapsed = current_time - self.last_send_time >= self.send_interval;
                let should_send = direction_changed || time_elapsed;

                if can_move {
                    if should_send {
                        commands.push(InputCommand::Move { dx, dy });
                        self.last_dx = dx;
                        self.last_dy = dy;
                        self.last_send_time = current_time;
                        self.move_sent = true;
                        // Also track D-pad move sent
                        if has_dpad_input {
                            self.touch_controls.set_dpad_move_sent(true);
                        }
                    }
                } else {
                    // Can't move - face that direction instead
                    if self.move_sent || self.touch_controls.was_dpad_move_sent() {
                        // Was moving, send stop
                        commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                        self.move_sent = false;
                        self.touch_controls.set_dpad_move_sent(false);
                    }
                    if should_send && !state.is_sitting {
                        let face_dir = new_dir.to_direction_u8();
                        commands.push(InputCommand::Face { direction: face_dir });
                        self.last_dx = dx;
                        self.last_dy = dy;
                        self.last_send_time = current_time;
                    }
                }
            }
        }

        // Handle keyboard release when D-pad not active - send stop command
        if !has_dpad_input && keyboard_dir == CardinalDir::None && self.move_sent {
            // Already handled above in dir_changed block
        }

        // Path following - generate movement commands when auto-pathing
        // Only follow path if not manually moving and not attacking
        if dx == 0.0 && dy == 0.0 && !is_attacking {
            // Get player position from SERVER state (not visual) to avoid getting ahead of server
            let player_pos = state.get_local_player().map(|p| (p.server_x.round() as i32, p.server_y.round() as i32));

            // Check if next waypoint is blocked by an entity - if so, cancel path
            let mut path_blocked = false;
            if let (Some((player_x, player_y)), Some(ref path_state)) = (player_pos, &state.auto_path) {
                if path_state.current_index < path_state.path.len() {
                    let (next_x, next_y) = path_state.path[path_state.current_index];

                    // Check if we need to move to reach the waypoint
                    if player_x != next_x || player_y != next_y {
                        let occupied = build_occupied_set(state);
                        if occupied.contains(&(next_x, next_y)) {
                            path_blocked = true;
                        }
                    }
                }
            }

            // If path is blocked by entity, cancel it and stop
            if path_blocked {
                state.auto_path = None;
                commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
                return commands;
            }

            if let (Some((player_x, player_y)), Some(ref mut path_state)) = (player_pos, &mut state.auto_path) {
                // Check if we've reached the current waypoint
                if path_state.current_index < path_state.path.len() {
                    let (target_x, target_y) = path_state.path[path_state.current_index];

                    if player_x == target_x && player_y == target_y {
                        // Move to next waypoint
                        path_state.current_index += 1;
                    }

                    // Generate movement toward current waypoint
                    if path_state.current_index < path_state.path.len() {
                        let (next_x, next_y) = path_state.path[path_state.current_index];
                        let move_dx = (next_x - player_x).signum() as f32;
                        let move_dy = (next_y - player_y).signum() as f32;

                        // Only move in one direction at a time (grid-based movement)
                        if move_dx != 0.0 {
                            commands.push(InputCommand::Move { dx: move_dx, dy: 0.0 });
                        } else if move_dy != 0.0 {
                            commands.push(InputCommand::Move { dx: 0.0, dy: move_dy });
                        }
                    }
                }
            }

            // Check if path completed and handle pickup/interact if needed
            if state.auto_path.as_ref().map(|p| p.current_index >= p.path.len()).unwrap_or(false) {
                // Path completed - check for pickup target
                if let Some(ref path_state) = state.auto_path {
                    if let Some(ref item_id) = path_state.pickup_target {
                        commands.push(InputCommand::Pickup { item_id: item_id.clone() });
                    }
                    // Handle interact target (NPC)
                    if let Some(ref npc_id) = path_state.interact_target {
                        // Check if target is an altar
                        if let Some(npc) = state.npcs.get(npc_id) {
                            if npc.is_altar {
                                state.ui_state.altar_panel = Some(crate::game::AltarPanelState {
                                    altar_npc_id: npc_id.clone(),
                                    altar_name: npc.display_name.clone(),
                                });
                            } else if npc.is_alive() {
                                commands.push(InputCommand::Interact { npc_id: npc_id.clone() });
                            }
                        } else {
                            commands.push(InputCommand::Interact { npc_id: npc_id.clone() });
                        }
                    }
                }
                // Handle chair sit target
                if let Some((cx, cy)) = state.pending_chair_sit.take() {
                    commands.push(InputCommand::SitChair { tile_x: cx, tile_y: cy });
                }
                // Handle farming harvest target
                if let Some(patch_id) = state.pending_harvest_patch.take() {
                    commands.push(InputCommand::HarvestCrop { patch_id });
                }
                state.auto_path = None;

                // Send stop command so we don't keep moving in the last direction
                commands.push(InputCommand::Move { dx: 0.0, dy: 0.0 });
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
            // Cancel auto-path when attacking
            state.clear_auto_path();

            if current_time - self.last_attack_time >= self.attack_cooldown {
                // Check if we should gather instead of attack
                let should_gather = if let Some(player) = state.get_local_player() {
                    if player.equipped_weapon.as_deref() == Some("fishing_rod") {
                        let px = player.x.round() as i32;
                        let py = player.y.round() as i32;
                        let (fdx, fdy) = player.direction.to_unit_vector();
                        let face_x = px + fdx as i32;
                        let face_y = py + fdy as i32;
                        // Check if the tile we're facing is a fishing marker
                        state.gathering_markers.iter().find(|m| {
                            m.skill == "fishing" && m.x == face_x && m.y == face_y
                        }).map(|m| (m.x, m.y))
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
                        let (has_axe, chop_speed) = if let Some(ref weapon_id) = player.equipped_weapon {
                            let speed = state.item_registry.get(weapon_id)
                                .and_then(|item| item.equipment.as_ref())
                                .map(|eq| eq.chop_speed_multiplier)
                                .unwrap_or(0.0);
                            log::info!("Woodcutting check: weapon={} chop_speed={}", weapon_id, speed);
                            (speed > 0.0, speed)
                        } else {
                            log::info!("Woodcutting check: no weapon equipped");
                            (false, 0.0)
                        };

                        if has_axe {
                            let px = player.x.round() as i32;
                            let py = player.y.round() as i32;
                            let (fdx, fdy) = player.direction.to_unit_vector();
                            let face_x = px + fdx as i32;
                            let face_y = py + fdy as i32;

                            log::info!("Axe detected (speed={}), checking tile ({}, {})", chop_speed, face_x, face_y);

                            // Check if facing tile has a tree object and is not depleted
                            if !state.depleted_trees.contains_key(&(face_x, face_y)) {
                                let obj_result = state.chunk_manager.get_object_at_exact(face_x, face_y);
                                if let Some(obj) = obj_result {
                                    log::info!("Found object at ({}, {}): gid={}", face_x, face_y, obj.gid);
                                    Some((face_x, face_y, obj.gid))
                                } else {
                                    log::info!("No object found at ({}, {})", face_x, face_y);
                                    None
                                }
                            } else {
                                log::info!("Tree at ({}, {}) is depleted", face_x, face_y);
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
                        log::info!("Fishing rod equipped near fishing spot - sending StartGathering");
                        commands.push(InputCommand::StartGathering { marker_x, marker_y });
                        self.last_attack_time = current_time;
                    }
                } else if let Some((tree_x, tree_y, tree_gid)) = should_woodcut {
                    // Send chop command on each attack press when facing a tree with an axe
                    log::info!("Axe equipped near tree - sending ChopTree");
                    commands.push(InputCommand::ChopTree { tree_x, tree_y, tree_gid });
                    self.last_attack_time = current_time;
                } else {
                    log::info!("Space held - sending Attack command");
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
                        if state.ui_state.spell_bar_active {
                            // Spell mode: cast the spell at this index
                            let magic_level = state.get_local_player()
                                .map(|p| p.skills.magic.level)
                                .unwrap_or(1);
                            let unlocked_spells: Vec<_> = crate::game::spell::SPELLS.iter()
                                .filter(|s| magic_level >= s.magic_level_req)
                                .collect();
                            if let Some(spell_def) = unlocked_spells.get(*idx) {
                                commands.push(InputCommand::CastSpell { spell_id: spell_def.id.to_string() });
                                let cooldown_end = macroquad::time::get_time() + (spell_def.cooldown_ms as f64 / 1000.0);
                                state.spell_cooldowns.insert(spell_def.id.to_string(), cooldown_end);
                            }
                        } else {
                            // Item mode: use/equip item at inventory slot idx
                            let slot_idx = *idx;
                            if let Some(Some(slot)) = state.inventory.slots.get(slot_idx) {
                                let item_def = state.item_registry.get_or_placeholder(&slot.item_id);
                                if item_def.equipment.is_some() {
                                    commands.push(InputCommand::Equip { slot_index: slot_idx as u8 });
                                } else {
                                    commands.push(InputCommand::UseItem { slot_index: slot_idx as u8 });
                                }
                            }
                        }
                    } else if mouse_right_clicked {
                        // Right-click on quick slot opens context menu (item mode only)
                        if !state.ui_state.spell_bar_active {
                            let inv_idx = *idx;
                            if state.inventory.slots.get(inv_idx).and_then(|s| s.as_ref()).is_some() {
                                state.ui_state.context_menu = Some(ContextMenu {
                                    target: ContextMenuTarget::InventorySlot(inv_idx),
                                    x: mx,
                                    y: my,
                                });
                            }
                        }
                    }
                    return commands;
                }
                UiElementId::InventorySlot(idx) => {
                    if mouse_right_clicked {
                        // Right-click opens context menu (if item exists)
                        if state.inventory.slots.get(*idx).and_then(|s| s.as_ref()).is_some() {
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
                            "head" => state.get_local_player().and_then(|p| p.equipped_head.as_ref()).is_some(),
                            "body" => state.get_local_player().and_then(|p| p.equipped_body.as_ref()).is_some(),
                            "weapon" => state.get_local_player().and_then(|p| p.equipped_weapon.as_ref()).is_some(),
                            "back" => state.get_local_player().and_then(|p| p.equipped_back.as_ref()).is_some(),
                            "feet" => state.get_local_player().and_then(|p| p.equipped_feet.as_ref()).is_some(),
                            "ring" => state.get_local_player().and_then(|p| p.equipped_ring.as_ref()).is_some(),
                            "gloves" => state.get_local_player().and_then(|p| p.equipped_gloves.as_ref()).is_some(),
                            "necklace" => state.get_local_player().and_then(|p| p.equipped_necklace.as_ref()).is_some(),
                            "belt" => state.get_local_player().and_then(|p| p.equipped_belt.as_ref()).is_some(),
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
                                        commands.push(InputCommand::Pickup { item_id: item_id.clone() });
                                    } else {
                                        // Out of range - path to an adjacent tile
                                        let player_x = player.x.round() as i32;
                                        let player_y = player.y.round() as i32;
                                        let item_x = ground_item.x.round() as i32;
                                        let item_y = ground_item.y.round() as i32;

                                        // Build occupied set (other players + NPCs)
                                        let occupied = build_occupied_set(state);

                                        const MAX_PATH_DISTANCE: i32 = 32;
                                        if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                                            (player_x, player_y),
                                            (item_x, item_y),
                                            &state.chunk_manager,
                                            &occupied,
                                            MAX_PATH_DISTANCE,
                                        ) {
                                            state.auto_path = Some(PathState {
                                                path,
                                                current_index: 0,
                                                destination: dest,
                                                pickup_target: Some(item_id.clone()),
                                                interact_target: None,
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
                // Check if NPC is hostile (monster) or friendly (shop/quest)
                let is_hostile = state.npcs.get(&npc_id).map(|n| n.is_hostile()).unwrap_or(true);

                if is_hostile {
                    // Hostile NPC (monster) - target them for combat
                    commands.push(InputCommand::Target { entity_id: npc_id });
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
                                    // Check if NPC is an altar - open altar panel instead of dialogue
                                    if npc.is_altar {
                                        state.ui_state.altar_panel = Some(crate::game::AltarPanelState {
                                            altar_npc_id: npc_id.clone(),
                                            altar_name: npc.display_name.clone(),
                                        });
                                    } else {
                                        commands.push(InputCommand::Interact { npc_id });
                                    }
                                } else {
                                    // Out of range - pathfind to adjacent tile
                                    let player_x = player.x.round() as i32;
                                    let player_y = player.y.round() as i32;
                                    let npc_x = npc.x.round() as i32;
                                    let npc_y = npc.y.round() as i32;

                                    // Build occupied set (other players + NPCs)
                                    let occupied = build_occupied_set(state);

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
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            } else if let Some(entity_id) = clicked_player {
                // Player clicked - target them
                commands.push(InputCommand::Target { entity_id });
            } else if let Some(patch_id) = state.farming_patch_positions.get(&(clicked_tile_x, clicked_tile_y)).cloned() {
                // Clicked on a farming patch
                if let Some(patch) = state.farming_patches.get(&patch_id) {
                    if patch.state == "harvestable" {
                        if let Some(local_id) = &state.local_player_id {
                            if let Some(player) = state.players.get(local_id) {
                                let px = player.x.round() as i32;
                                let py = player.y.round() as i32;
                                let cdx = (px - clicked_tile_x).abs();
                                let cdy = (py - clicked_tile_y).abs();
                                if cdx <= 1 && cdy <= 1 {
                                    commands.push(InputCommand::HarvestCrop { patch_id });
                                } else {
                                    // Out of range - pathfind to adjacent tile
                                    let occupied = build_occupied_set(state);
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
                                        });
                                        state.pending_harvest_patch = Some(patch_id);
                                    }
                                }
                            }
                        }
                    }
                }
            } else if state.chair_positions.contains(&(clicked_tile_x, clicked_tile_y)) {
                // Clicked on a chair - try to sit
                if !state.is_sitting {
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(player) = state.players.get(local_id) {
                            let px = player.x.round() as i32;
                            let py = player.y.round() as i32;
                            let cdx = (px - clicked_tile_x).abs();
                            let cdy = (py - clicked_tile_y).abs();
                            if cdx <= 1 && cdy <= 1 {
                                // Within range - sit immediately
                                commands.push(InputCommand::SitChair { tile_x: clicked_tile_x, tile_y: clicked_tile_y });
                            } else {
                                // Out of range - pathfind to adjacent tile, then sit
                                let occupied = build_occupied_set(state);
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
                                    });
                                    state.pending_chair_sit = Some((clicked_tile_x, clicked_tile_y));
                                }
                            }
                        }
                    }
                }
            } else if state.ui_state.tap_to_pathfind {
                // Clicked on empty space - try to path there (if tap-to-pathfind enabled)
                let tile_x = world_x.round() as i32;
                let tile_y = world_y.round() as i32;

                // Only path if within range and walkable
                const MAX_PATH_DISTANCE: i32 = 32;

                if let Some(player) = state.get_local_player() {
                    let player_x = player.x.round() as i32;
                    let player_y = player.y.round() as i32;
                    let dist = (tile_x - player_x).abs().max((tile_y - player_y).abs());

                    if dist <= MAX_PATH_DISTANCE && state.chunk_manager.is_walkable(tile_x as f32, tile_y as f32) {
                        // Build occupied set (other players + NPCs)
                        let occupied = build_occupied_set(state);

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

        // Escape key - close any open panel first, then clear target, then open escape menu
        if is_key_pressed(KeyCode::Escape) {
            // Check if any panel is open and close it
            if state.ui_state.inventory_open || state.ui_state.character_panel_open
                || state.ui_state.social_open || state.ui_state.skills_open
                || state.ui_state.prayer_book_open {
                audio.play_sfx("enter");
                state.ui_state.inventory_open = false;
                state.ui_state.character_panel_open = false;
                state.ui_state.social_open = false;
                state.ui_state.skills_open = false;
                state.ui_state.prayer_book_open = false;
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
                let bg_padding = 6.0;
                let bar_bottom_offset = 8.0 * scale + 6.0; // EXP_BAR_GAP * scale + margin
                let box_bottom = chat_sh - bar_bottom_offset;
                let line_height = 18.0;
                let max_visible_lines: usize = 9;
                let chat_area_h = max_visible_lines as f32 * line_height;
                let chat_bottom_y = box_bottom - bg_padding;
                let chat_top_y = chat_bottom_y - chat_area_h + line_height;
                let over_chat = vmx >= 10.0 - bg_padding && vmx <= 10.0 + 400.0 + bg_padding
                    && vmy >= chat_top_y - bg_padding && vmy <= box_bottom;
                if over_chat {
                    const SCROLL_SPEED: f32 = 40.0; // Pixels per scroll tick
                    let delta = wheel_y * SCROLL_SPEED;
                    state.ui_state.chat_message_scroll = (state.ui_state.chat_message_scroll + delta).max(0.0);
                }
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
                    state.ui_state.inventory_scroll_offset = (state.ui_state.inventory_scroll_offset - wheel_y * SCROLL_SPEED).max(0.0);
                    // Max scroll will be clamped during rendering
                }
            }

            // Mouse scrollbar dragging (relative/delta-based)
            if state.ui_state.inventory_scrollbar_dragging {
                if is_mouse_button_down(MouseButton::Left) {
                    let dy = my - state.ui_state.inventory_scrollbar_drag_last_y;
                    if let Some(track_bounds) = layout.get_bounds(&UiElementId::InventoryScrollbar) {
                        // Scale: moving across the full track scrolls all content
                        // Use a reasonable estimate for total content height
                        let scale = 500.0 / track_bounds.h;
                        state.ui_state.inventory_scroll_offset = (state.ui_state.inventory_scroll_offset + dy * scale).max(0.0);
                    }
                    state.ui_state.inventory_scrollbar_drag_last_y = my;
                } else {
                    state.ui_state.inventory_scrollbar_dragging = false;
                }
            } else if mouse_clicked {
                if matches!(clicked_element, Some(UiElementId::InventoryScrollbar)) {
                    state.ui_state.inventory_scrollbar_dragging = true;
                    state.ui_state.inventory_scrollbar_drag_last_y = my;
                }
            }

            // Touch drag scrolling for mobile
            let all_touches: Vec<Touch> = touches();
            if let Some(tracking_id) = state.ui_state.inventory_touch_scroll_id {
                // We're tracking a touch - update or release
                if let Some(touch) = all_touches.iter().find(|t| t.id == tracking_id) {
                    match touch.phase {
                        TouchPhase::Moved | TouchPhase::Stationary => {
                            let (_, vy) = screen_to_virtual_coords(touch.position.x, touch.position.y);
                            let dy = state.ui_state.inventory_touch_last_y - vy;
                            state.ui_state.inventory_scroll_offset = (state.ui_state.inventory_scroll_offset + dy).max(0.0);
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
                            Some(UiElementId::InventoryGridArea) | Some(UiElementId::InventorySlot(_)) | Some(UiElementId::InventoryScrollbar)
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
            state.ui_state.inventory_scrollbar_dragging = false;
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
            }
        }

        // Use/equip items or cast spells (1-5 keys for quick slots, disabled in classic mode)
        let quick_slot_keys = [
            (KeyCode::Key1, 0usize),
            (KeyCode::Key2, 1usize),
            (KeyCode::Key3, 2usize),
            (KeyCode::Key4, 3usize),
            (KeyCode::Key5, 4usize),
        ];
        for (key, slot_idx) in quick_slot_keys {
            if !classic && is_key_pressed(key) {
                if state.ui_state.spell_bar_active {
                    // Spell mode: cast the spell at this index
                    let magic_level = state.get_local_player()
                        .map(|p| p.skills.magic.level)
                        .unwrap_or(1);
                    let unlocked_spells: Vec<_> = crate::game::spell::SPELLS.iter()
                        .filter(|s| magic_level >= s.magic_level_req)
                        .collect();
                    if let Some(spell_def) = unlocked_spells.get(slot_idx) {
                        commands.push(InputCommand::CastSpell { spell_id: spell_def.id.to_string() });
                        let cooldown_end = macroquad::time::get_time() + (spell_def.cooldown_ms as f64 / 1000.0);
                        state.spell_cooldowns.insert(spell_def.id.to_string(), cooldown_end);
                    }
                } else {
                    // Item mode: use/equip from inventory slot directly
                    if let Some(Some(slot)) = state.inventory.slots.get(slot_idx) {
                        let item_def = state.item_registry.get_or_placeholder(&slot.item_id);
                        if item_def.equipment.is_some() {
                            commands.push(InputCommand::Equip { slot_index: slot_idx as u8 });
                        } else {
                            commands.push(InputCommand::UseItem { slot_index: slot_idx as u8 });
                        }
                    }
                }
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
        let interact_pressed = (!classic && is_key_pressed(KeyCode::E)) || self.touch_controls.interact_pressed();
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
                        commands.push(InputCommand::SitChair { tile_x: cx, tile_y: cy });
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
                        // Check if NPC is an altar - open altar panel instead of dialogue
                        if let Some(npc) = state.npcs.get(&npc_id) {
                            if npc.is_altar {
                                state.ui_state.altar_panel = Some(crate::game::AltarPanelState {
                                    altar_npc_id: npc_id.clone(),
                                    altar_name: npc.display_name.clone(),
                                });
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
            }
        }

        // Toggle quest log (Q key)
        if !classic && is_key_pressed(KeyCode::Q) {
            state.ui_state.quest_log_open = !state.ui_state.quest_log_open;
        }

        commands
    }

    /// Get current movement direction (for client-side prediction)
    pub fn get_movement(&self) -> (f32, f32) {
        (self.last_dx, self.last_dy)
    }

    /// Render touch controls overlay (call after all other rendering)
    /// Set hide_action_buttons to true when panels like inventory are open
    pub fn render_touch_controls(&self, hide_action_buttons: bool, hide_all_controls: bool, use_joystick: bool) {
        self.touch_controls.render(hide_action_buttons, hide_all_controls, use_joystick);
    }

    /// Update attack button to show the currently equipped weapon sprite
    pub fn update_attack_button_icon(&mut self, weapon_id: Option<&str>, item_sprites: &crate::render::SpriteStore) {
        self.touch_controls.update_attack_icon(weapon_id, item_sprites);
    }
}
