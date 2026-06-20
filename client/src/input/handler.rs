use super::touch::TouchControls;
use crate::audio::AudioManager;
use crate::game::state::{ClickEffect, ClickEffectKind};
use crate::game::{
    pathfinding, quest_status_order, ActiveDialogue, BankDrag, BankQuantityAction,
    BankQuantityDialog, ChatChannel, ContextMenu, ContextMenuTarget, DragSource, DragState,
    GameState, GoldDropDialog, PathState, QuestCatalogEntry, StallPriceDialog, CHUNK_SIZE,
};
use crate::render::animation::AnimationState;
use crate::render::isometric::screen_to_world;
use crate::render::{section_sort_key, sections_for_tab, SECTION_HEADER_HEIGHT};
use crate::settings::{save_ui_settings, UiSettings};
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;
use std::collections::HashSet;

mod banking;
mod crafting;
mod dialogue;
mod drag_context;
mod drag_drop;
mod drag_start;
mod fletching;
mod furnace;
mod gameplay;
mod grand_exchange;
mod helpers;
mod lifecycle;
mod menus;
mod modal;
mod movement;
mod pathfinding_helpers;
mod smithing;
mod ui_actions;
mod workbench;
mod world;

use helpers::*;
pub use helpers::{get_map_object_name, is_obelisk_gid};
use pathfinding_helpers::*;

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
    Jump,
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
    UseItemOnEntity {
        slot_index: u8,
        npc_id: String,
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
        expected_price: i32,
    },
    // Combat style
    SetCombatStyle {
        style: String,
    },
    // Auto-retaliate toggle
    SetAutoRetaliate {
        enabled: bool,
    },
    // KOTH commands
    KothContinue,
    KothLeave,
    // Grand Exchange
    GeOpen,
    GePlaceOffer {
        side: String,
        item_id: String,
        price: i64,
        quantity: i64,
    },
    GeCancelOffer {
        offer_id: i64,
    },
    GeCollect {
        offer_id: i64,
    },
}

/// Movement directions for isometric movement (cardinal + diagonal)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MoveDir {
    None,
    Up,
    Down,
    Left,
    Right,
}

impl MoveDir {
    /// Convert to server direction enum value (matches Direction enum)
    fn to_direction_u8(self) -> u8 {
        match self {
            MoveDir::Down => 0,
            MoveDir::Left => 1,
            MoveDir::Up => 2,
            MoveDir::Right => 3,
            MoveDir::None => 0, // Default to down
        }
    }

    /// Determine direction from held keys + just-pressed info.
    /// Last key pressed wins. When released, falls back to whatever is still held.
    fn from_keys(
        up: bool,
        down: bool,
        left: bool,
        right: bool,
        up_just: bool,
        down_just: bool,
        left_just: bool,
        right_just: bool,
        active: MoveDir,
    ) -> Self {
        // If a key was just pressed this frame, it takes priority
        // (check in reverse order so last-processed wins if multiple pressed same frame)
        let mut new_active = active;
        if up_just {
            new_active = MoveDir::Up;
        }
        if down_just {
            new_active = MoveDir::Down;
        }
        if left_just {
            new_active = MoveDir::Left;
        }
        if right_just {
            new_active = MoveDir::Right;
        }

        // If the active direction's key is still held, use it
        let active_held = match new_active {
            MoveDir::Up => up,
            MoveDir::Down => down,
            MoveDir::Left => left,
            MoveDir::Right => right,
            MoveDir::None => false,
        };
        if active_held {
            return new_active;
        }

        // Active key was released — fall back to whatever is still held
        // (pick first found, priority doesn't matter since only one should remain)
        if up {
            return MoveDir::Up;
        }
        if down {
            return MoveDir::Down;
        }
        if left {
            return MoveDir::Left;
        }
        if right {
            return MoveDir::Right;
        }

        MoveDir::None
    }

    /// Convert to velocity vector
    fn to_velocity(self) -> (f32, f32) {
        match self {
            MoveDir::Up => (0.0, -1.0),
            MoveDir::Down => (0.0, 1.0),
            MoveDir::Left => (-1.0, 0.0),
            MoveDir::Right => (1.0, 0.0),
            MoveDir::None => (0.0, 0.0),
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

const MINIMAP_PREVIEW_WIDTH: f32 = 188.0;
const MINIMAP_PREVIEW_HEIGHT: f32 = 140.0;
const MINIMAP_PREVIEW_Y: f32 = 8.0;
const MINIMAP_PREVIEW_MARGIN: f32 = 12.0;
const MINIMAP_VISIBLE_CHUNK_RADIUS: f32 = 0.8;

fn minimap_preview_rect(ui_scale: f32) -> Rect {
    let (sw, _) = virtual_screen_size();
    let s = ui_scale;
    let width = MINIMAP_PREVIEW_WIDTH * s;
    let height = MINIMAP_PREVIEW_HEIGHT * s;
    let margin = MINIMAP_PREVIEW_MARGIN * s;
    let y = MINIMAP_PREVIEW_Y * s;
    Rect::new((sw - width - margin).floor(), y.floor(), width, height)
}

fn minimap_preview_bounds(state: &GameState) -> Option<MinimapBounds> {
    let player = state.get_local_player()?;
    let half_span = CHUNK_SIZE as f32 * (MINIMAP_VISIBLE_CHUNK_RADIUS + 0.5);
    Some(MinimapBounds {
        min_x: player.x - half_span,
        min_y: player.y - half_span,
        max_x: player.x + half_span,
        max_y: player.y + half_span,
    })
}

fn minimap_world_bounds(state: &GameState) -> Option<MinimapBounds> {
    let mut bounds = if let Some((width, height)) = state.chunk_manager.get_interior_size() {
        MinimapBounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: width as f32,
            max_y: height as f32,
        }
    } else if let Some(snapshot) = state.world_map_snapshot.as_ref() {
        MinimapBounds {
            min_x: snapshot.bounds.min_x,
            min_y: snapshot.bounds.min_y,
            max_x: snapshot.bounds.max_x,
            max_y: snapshot.bounds.max_y,
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
    current_dir: MoveDir,
    // Track previous direction for detecting key release
    prev_dir: MoveDir,
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
    // Timestamp until which keyboard movement is suppressed so an attack
    // auto-action can fire. Cleared on key release or expiry (max ~600ms).
    suppress_move_until: f64,
    // Touch controls for mobile devices
    pub touch_controls: TouchControls,
    // Long-press tracking for right-click on mobile
    long_press_start: f64,
    long_press_pos: (f32, f32),
    long_press_active: bool,
    long_press_fired: bool,
    /// Timestamp when we first started being blocked by another player.
    /// After 500ms, we ghost through them (client-side collision skip).
    player_blocked_since: Option<f64>,
}

#[derive(Clone, Copy)]
struct ProcessFrame<'a> {
    current_time: f64,
    mx: f32,
    my: f32,
    mouse_clicked: bool,
    mouse_right_clicked: bool,
    mouse_released: bool,
    clicked_element: &'a Option<UiElementId>,
}

struct GameplayMode {
    chat_consuming_keyboard: bool,
    minimap_panel_blocks_input: bool,
    classic: bool,
}

struct MovementInput {
    dx: f32,
    dy: f32,
    attack_key_down: bool,
    is_attacking: bool,
    suppress_active: bool,
}

impl InputHandler {
    pub fn process(
        &mut self,
        state: &mut GameState,
        layout: &UiLayout,
        audio: &mut AudioManager,
    ) -> Vec<InputCommand> {
        let mut commands = Vec::new();
        let current_time = get_time();

        // Detect Ctrl/Cmd+V from this frame's key events (live modifier flags) before any handler
        // reads it. Avoids is_key_down, which the OS leaves stuck "down" after focus loss with the
        // key held — a cross-platform quirk that otherwise makes every plain `v` paste.
        state.ui_state.paste_requested = helpers::poll_paste_request();

        self.update_touch_controls(state, current_time);

        // Get current mouse/touch position in virtual coordinates (for UI hit detection)
        let (raw_mx, raw_my) = mouse_position();
        let (mx, my) = screen_to_virtual_coords(raw_mx, raw_my);

        self.update_hover_state(state, layout, mx, my);
        let (mouse_clicked, mouse_right_clicked, mouse_released, clicked_element) =
            self.current_click_target(layout, mx, my);
        let frame = ProcessFrame {
            current_time,
            mx,
            my,
            mouse_clicked,
            mouse_right_clicked,
            mouse_released,
            clicked_element: &clicked_element,
        };

        // Toggle debug mode
        if is_key_pressed(KeyCode::F3) {
            // Debug toggle handled in main loop
        }

        if mouse_released
            && self.handle_drag_drop(state, clicked_element.as_ref(), audio, &mut commands)
        {
            return commands;
        }

        if self.handle_drag_start(state, audio, frame, &mut commands) {
            return commands;
        }

        if self.handle_context_menu(state, frame, &mut commands) {
            return commands;
        }

        if self.handle_menu_and_escape(state, layout, audio, frame, &mut commands) {
            return commands;
        }

        if self.handle_dialogue_and_altar(state, layout, audio, frame, &mut commands) {
            return commands;
        }

        if self.handle_grand_exchange(state, layout, frame, &mut commands) {
            return commands;
        }

        if self.handle_banking(state, layout, frame, &mut commands) {
            return commands;
        }

        if self.handle_crafting(state, layout, audio, frame, &mut commands) {
            return commands;
        }

        if self.handle_furnace(state, layout, audio, frame, &mut commands) {
            return commands;
        }

        if self.handle_smithing(state, layout, audio, frame, &mut commands) {
            return commands;
        }

        if self.handle_alchemy(state, layout, audio, frame, &mut commands) {
            return commands;
        }

        if self.handle_workbench(state, layout, audio, frame, &mut commands) {
            return commands;
        }

        if self.handle_fletching(state, layout, audio, frame, &mut commands) {
            return commands;
        }

        let Some(mode) = self.resolve_gameplay_mode(state, layout, audio, frame, &mut commands)
        else {
            return commands;
        };

        let movement = self.handle_manual_movement(state, &mode, frame, &mut commands);

        // Minimap panel open: movement keys processed above, skip everything else
        if mode.minimap_panel_blocks_input {
            return commands;
        }

        if self.handle_pathing_and_combat(state, &mode, &movement, frame, &mut commands) {
            return commands;
        }

        if self.handle_clickable_ui(state, audio, frame, &mut commands) {
            return commands;
        }

        if self.handle_world_selection(state, frame, &mut commands) {
            return commands;
        }

        if self.handle_world_context(state, frame, &mut commands) {
            return commands;
        }

        if self.handle_shortcuts_and_scrolling(state, layout, audio, &mode, frame, &mut commands) {
            return commands;
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
        let s = state.ui_state.ui_scale;
        let columns = 4;
        let cell_h = 106.0 * s;
        let gap = 6.0 * s;
        let row = state.ui_state.anvil_selected_recipe / columns;
        let item_top = row as f32 * (cell_h + gap);
        let item_bottom = item_top + cell_h;

        let (_, sh) = crate::util::virtual_screen_size();
        let bottom_bar_h = 53.0 * s + 8.0;
        let panel_h = (500.0 * s).min(sh - bottom_bar_h - 8.0);
        let content_h = panel_h - 10.0 - 142.0 * s;

        if item_top < state.ui_state.anvil_scroll_offset {
            state.ui_state.anvil_scroll_offset = item_top;
        }
        if item_bottom > state.ui_state.anvil_scroll_offset + content_h {
            state.ui_state.anvil_scroll_offset = item_bottom - content_h;
        }
    }
}
