use std::collections::{HashMap, HashSet};
use super::entities::Player;
use super::item::{GroundItem, Inventory, RecipeDefinition};
use super::item_registry::ItemRegistry;
use super::npc::Npc;
use super::tilemap::Tilemap;
use super::chunk::ChunkManager;
use super::pathfinding::PathState;
use super::shop::{ShopData, ShopSubTab};
use crate::render::animation::AnimationState;
use crate::render::XpGlobesManager;
use crate::ui::UiElementId;
use crate::render::AreaBanner;

/// State of a map transition (fade effect)
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionState {
    None,
    FadingOut,
    Loading,
    FadingIn,
}

/// Tracks an in-progress map transition
#[derive(Debug, Clone)]
pub struct MapTransition {
    pub state: TransitionState,
    pub progress: f32,
    pub target_map_type: String,
    pub target_map_id: String,
    pub target_spawn_x: f32,
    pub target_spawn_y: f32,
    pub instance_id: String,
}

impl Default for MapTransition {
    fn default() -> Self {
        Self {
            state: TransitionState::None,
            progress: 0.0,
            target_map_type: String::new(),
            target_map_id: String::new(),
            target_spawn_x: 0.0,
            target_spawn_y: 0.0,
            instance_id: String::new(),
        }
    }
}

/// Frame timing diagnostics for performance analysis
#[derive(Clone)]
pub struct FrameTimings {
    pub network_ms: f64,
    pub render_total_ms: f64,
    pub render_ground_ms: f64,
    pub render_entities_ms: f64,
    pub render_overhead_ms: f64,
    pub render_ui_ms: f64,
    pub render_effects_ms: f64,
    pub update_ms: f64,
    pub total_ms: f64,
    pub entity_count: usize,
    pub chunk_count: usize,
    pub tiles_rendered: usize,
    // Frame delta tracking (rolling window)
    pub delta_ms: f64,
    pub delta_min_ms: f64,
    pub delta_max_ms: f64,
    delta_samples: [f64; 60],
    delta_idx: usize,
    // Optional FPS cap (None = uncapped)
    pub fps_cap: Option<u32>,
    // Time spent in next_frame().await (for diagnosing variance)
    pub next_frame_ms: f64,
    pub next_frame_min_ms: f64,
    pub next_frame_max_ms: f64,
    next_frame_samples: [f64; 60],
    next_frame_idx: usize,
    // Smoothed delta for visual interpolation (0.0 = no smoothing, 1.0 = max smoothing)
    pub delta_smoothing: f32,
    pub smoothed_delta: f32,
}

impl Default for FrameTimings {
    fn default() -> Self {
        Self {
            network_ms: 0.0,
            render_total_ms: 0.0,
            render_ground_ms: 0.0,
            render_entities_ms: 0.0,
            render_overhead_ms: 0.0,
            render_ui_ms: 0.0,
            render_effects_ms: 0.0,
            update_ms: 0.0,
            total_ms: 0.0,
            entity_count: 0,
            chunk_count: 0,
            tiles_rendered: 0,
            delta_ms: 0.0,
            delta_min_ms: 0.0,
            delta_max_ms: 0.0,
            delta_samples: [0.0; 60],
            delta_idx: 0,
            fps_cap: None, // Uncapped by default
            next_frame_ms: 0.0,
            next_frame_min_ms: 0.0,
            next_frame_max_ms: 0.0,
            next_frame_samples: [0.0; 60],
            next_frame_idx: 0,
            delta_smoothing: 0.0, // 0.0 = disabled, try 0.5-0.9 for smoothing
            smoothed_delta: 0.016, // Start at ~60fps
        }
    }
}

impl FrameTimings {
    pub fn record_next_frame(&mut self, ms: f64) {
        self.next_frame_ms = ms;
        self.next_frame_samples[self.next_frame_idx] = ms;
        self.next_frame_idx = (self.next_frame_idx + 1) % 60;

        // Calculate min/max over the window
        self.next_frame_min_ms = f64::MAX;
        self.next_frame_max_ms = f64::MIN;
        for &sample in &self.next_frame_samples {
            if sample > 0.0 {
                self.next_frame_min_ms = self.next_frame_min_ms.min(sample);
                self.next_frame_max_ms = self.next_frame_max_ms.max(sample);
            }
        }
        if self.next_frame_min_ms == f64::MAX {
            self.next_frame_min_ms = ms;
        }
        if self.next_frame_max_ms == f64::MIN {
            self.next_frame_max_ms = ms;
        }
    }
}

impl FrameTimings {
    pub fn record_delta(&mut self, delta_ms: f64) {
        self.delta_ms = delta_ms;
        self.delta_samples[self.delta_idx] = delta_ms;
        self.delta_idx = (self.delta_idx + 1) % 60;

        // Calculate min/max over the window
        self.delta_min_ms = f64::MAX;
        self.delta_max_ms = f64::MIN;
        for &sample in &self.delta_samples {
            if sample > 0.0 {
                self.delta_min_ms = self.delta_min_ms.min(sample);
                self.delta_max_ms = self.delta_max_ms.max(sample);
            }
        }
        if self.delta_min_ms == f64::MAX {
            self.delta_min_ms = delta_ms;
        }
        if self.delta_max_ms == f64::MIN {
            self.delta_max_ms = delta_ms;
        }

        // Update smoothed delta for visual interpolation
        let delta_secs = (delta_ms / 1000.0) as f32;
        if self.delta_smoothing > 0.0 {
            self.smoothed_delta = self.smoothed_delta * self.delta_smoothing
                + delta_secs * (1.0 - self.delta_smoothing);
        } else {
            self.smoothed_delta = delta_secs;
        }
    }
}

pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
    pub initialized: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
            initialized: false,
        }
    }
}

/// Chat channel types for different message sources
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatChannel {
    Local,      // Nearby players only (current default)
    Global,     // Server-wide player chat
    System,     // XP gains, quest completions, shop transactions
    // Future:
    // Party,
    // Guild,
}

pub struct ChatMessage {
    pub sender_name: String,
    pub text: String,
    pub timestamp: f64,
    pub channel: ChatChannel,
}

impl ChatMessage {
    /// Create a player chat message (defaults to Local channel)
    pub fn player(sender_name: String, text: String) -> Self {
        ChatMessage {
            sender_name,
            text,
            timestamp: macroquad::time::get_time(),
            channel: ChatChannel::Local,
        }
    }

    /// Create a system message
    pub fn system(text: String) -> Self {
        ChatMessage {
            sender_name: "[System]".to_string(),
            text,
            timestamp: macroquad::time::get_time(),
            channel: ChatChannel::System,
        }
    }
}

/// Floating damage/healing number for combat feedback
/// - Positive damage = damage dealt (red)
/// - Negative damage = healing (green, displayed as +X)
/// - Zero = miss (gray, displayed as "MISS")
pub struct DamageEvent {
    pub x: f32,
    pub y: f32,
    pub damage: i32,
    pub time: f64,
    /// Entity ID to look up sprite height at render time
    pub target_id: String,
    pub source_id: Option<String>,
    pub projectile: Option<String>,
}

/// Pending spell effect received from server (to be rendered by Task 9)
pub struct SpellEffect {
    pub caster_id: String,
    pub target_id: Option<String>,
    pub spell_id: String,
    pub target_x: i32,
    pub target_y: i32,
    pub time: f64,
}

/// Active projectile for ranged attack visualization
pub struct Projectile {
    pub sprite: String,
    pub start_x: f32,
    pub start_y: f32,
    pub end_x: f32,
    pub end_y: f32,
    pub start_time: f64,
    pub duration: f64,
}

impl Projectile {
    /// Get current position (0.0 to 1.0 progress)
    pub fn progress(&self, current_time: f64) -> f32 {
        let elapsed = current_time - self.start_time;
        (elapsed / self.duration).min(1.0) as f32
    }

    /// Check if projectile animation is complete
    pub fn is_complete(&self, current_time: f64) -> bool {
        current_time - self.start_time >= self.duration
    }

    /// Get current world position
    pub fn current_pos(&self, current_time: f64) -> (f32, f32) {
        let t = self.progress(current_time);
        let x = self.start_x + (self.end_x - self.start_x) * t;
        let y = self.start_y + (self.end_y - self.start_y) * t;
        (x, y)
    }
}

/// Floating level up text
pub struct LevelUpEvent {
    pub x: f32,
    pub y: f32,
    pub skill: String,
    pub new_level: i32,
    pub time: f64,
}

/// A single firework particle for level-up celebrations
pub struct FireworkParticle {
    /// World origin (where player was)
    pub origin_x: f32,
    pub origin_y: f32,
    /// Screen-pixel offset from origin
    pub ox: f32,
    pub oy: f32,
    /// Screen-pixel velocity
    pub vx: f32,
    pub vy: f32,
    pub color: (u8, u8, u8),
    pub time: f64,
    pub size: f32,
    /// Previous positions for tail (screen offsets)
    pub trail: Vec<(f32, f32)>,
    /// If true, this is a secondary burst particle (no further explosion)
    pub is_spark: bool,
    /// Whether this particle has already burst
    pub has_burst: bool,
}

/// Floating skill XP gain text
pub struct SkillXpEvent {
    pub x: f32,
    pub y: f32,
    pub skill: String,
    pub xp_gained: i64,
    pub time: f64,
}

/// An XP drop notification that floats up and fades out in the HUD
pub struct XpDrop {
    pub skill_type: super::SkillType,
    pub xp_gained: i64,
    pub time: f64,
}

/// Manages the XP drop feed displayed below the player stats panel
#[derive(Default)]
pub struct XpDropFeed {
    pub drops: Vec<XpDrop>,
}

impl XpDropFeed {
    pub fn new() -> Self {
        Self { drops: Vec::new() }
    }

    pub fn push(&mut self, skill_type: super::SkillType, xp_gained: i64) {
        self.drops.push(XpDrop {
            skill_type,
            xp_gained,
            time: macroquad::time::get_time(),
        });
    }

    /// No-op — positioning is purely time-based in the renderer.
    pub fn update(&mut self, _dt: f32) {}
}

/// Chat bubble displayed above a player's head
pub struct ChatBubble {
    pub player_id: String,
    pub text: String,
    pub time: f64,
}

/// A choice in a dialogue box
#[derive(Clone, Debug)]
pub struct DialogueChoice {
    pub id: String,
    pub text: String,
}

/// Active dialogue being shown to the player
#[derive(Clone, Debug)]
pub struct ActiveDialogue {
    pub quest_id: String,
    pub npc_id: String,
    pub speaker: String,
    pub text: String,
    pub choices: Vec<DialogueChoice>,
    pub show_time: f64,
}

/// A quest objective with progress tracking
#[derive(Clone, Debug)]
pub struct QuestObjective {
    pub id: String,
    pub description: String,
    pub current: i32,
    pub target: i32,
    pub completed: bool,
}

/// An active quest with its objectives
#[derive(Clone, Debug)]
pub struct ActiveQuest {
    pub id: String,
    pub name: String,
    pub objectives: Vec<QuestObjective>,
}

/// Quest completion notification
#[derive(Clone, Debug)]
pub struct QuestCompletedEvent {
    pub quest_id: String,
    pub quest_name: String,
    pub exp_reward: i32,
    pub gold_reward: i32,
    pub time: f64,
}

/// Server-wide announcement from admin
#[derive(Clone, Debug)]
pub struct Announcement {
    pub text: String,
    pub time: f64,
}

/// A farming patch in the world
#[derive(Debug, Clone)]
pub struct FarmingPatch {
    pub patch_id: String,
    pub x: i32,
    pub y: i32,
    pub state: String,       // "empty", "growing", "harvestable"
    pub crop_id: String,
    pub growth_stage: u32,
    pub owner_id: String,
}

/// A gathering marker tile in the world (fishing spot, mining node, etc.)
#[derive(Debug, Clone)]
pub struct GatheringMarker {
    pub x: i32,
    pub y: i32,
    pub zone_id: String,
    pub skill: String,
}

/// An active bonus tile event (glowing spot that gives 2x gather speed)
#[derive(Debug, Clone)]
pub struct BonusTile {
    pub x: i32,
    pub y: i32,
    pub zone_id: String,
    pub spawn_time: f64,
    pub telegraph_duration: f64,
}

/// An active gathering buff on a player
#[derive(Debug, Clone)]
pub struct GatheringBuff {
    pub buff_type: String,
    pub start_time: f64,
    pub duration: f64,
}

/// Target for context menu - what was right-clicked
#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    InventorySlot(usize),
    EquipmentSlot(String),
    Gold,
}

/// Context menu for right-clicking items
#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub target: ContextMenuTarget,
    pub x: f32,
    pub y: f32,
}

/// Dialog for entering gold drop amount
#[derive(Debug, Clone)]
pub struct GoldDropDialog {
    pub input: String,
    pub cursor: usize,
}

/// Source of a drag operation
#[derive(Debug, Clone, PartialEq)]
pub enum DragSource {
    Inventory(usize),          // Inventory slot index
    Equipment(String),         // Equipment slot type ("body", "feet")
}

/// Drag state for inventory/equipment rearrangement
#[derive(Debug, Clone)]
pub struct DragState {
    pub source: DragSource,
    pub item_id: String,
    pub quantity: i32,
}

/// Double-click tracking for inventory slots
#[derive(Debug, Clone)]
pub struct DoubleClickState {
    pub last_click_slot: Option<usize>,
    pub last_click_time: f64,
}

// ============================================================================
// Friend System State
// ============================================================================

/// Friend info for the friends list
#[derive(Debug, Clone)]
pub struct FriendInfo {
    pub id: i64,
    pub name: String,
    pub online: bool,
}

/// Pending friend request info
#[derive(Debug, Clone)]
pub struct PendingRequestInfo {
    pub from_id: i64,
    pub from_name: String,
}

/// Online player info for the social panel
#[derive(Debug, Clone)]
pub struct OnlinePlayerInfo {
    pub id: i64,
    pub name: String,
    pub is_friend: bool,
}

/// Social panel tab
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SocialTab {
    Nearby,
    Online,
    Friends,
}

impl Default for SocialTab {
    fn default() -> Self {
        Self::Nearby
    }
}

/// Social panel state
#[derive(Debug, Clone, Default)]
pub struct SocialState {
    /// Currently selected tab
    pub active_tab: SocialTab,
    /// Friends list (loaded from server)
    pub friends: Vec<FriendInfo>,
    /// Pending friend requests (loaded from server)
    pub pending_requests: Vec<PendingRequestInfo>,
    /// Online players list (refreshed on demand)
    pub online_players: Vec<OnlinePlayerInfo>,
    /// Nearby players (players in same room/instance)
    pub nearby_players: Vec<OnlinePlayerInfo>,
    /// Number of pending friend requests (for badge)
    pub pending_request_count: usize,
    /// "Add by name" input field text
    pub add_friend_input: String,
    /// Whether the add friend input is focused
    pub add_friend_focused: bool,
    /// Scroll offset for player list
    pub list_scroll_offset: f32,
    /// Scroll offset for friends list
    pub friends_scroll_offset: f32,
    /// Touch scroll tracking
    pub touch_scroll_id: Option<u64>,
    pub touch_last_y: f32,
    pub touch_start_y: f32,
    pub touch_dragged: bool,
}

#[derive(Debug, Clone)]
pub struct AltarPanelState {
    pub altar_npc_id: String,
    pub altar_name: String,
}

pub struct UiState {
    pub chat_open: bool,
    pub chat_input: String,
    pub chat_cursor: usize,        // Cursor position in chat_input (character index)
    pub chat_scroll_offset: usize, // Scroll offset for long messages (character index)
    pub chat_message_scroll: usize, // Scroll offset for message list (in wrapped lines from bottom)
    pub chat_key_repeat_time: f64, // Last time a repeated key action fired
    pub chat_key_initial_delay: bool, // Whether we're still in initial delay
    pub chat_messages: Vec<ChatMessage>,
    pub inventory_open: bool,
    // Quest UI state
    pub active_dialogue: Option<ActiveDialogue>,
    pub active_quests: Vec<ActiveQuest>,
    pub quest_completed_events: Vec<QuestCompletedEvent>,
    pub quest_log_open: bool,
    // Crafting UI state
    pub crafting_open: bool,
    pub crafting_selected_category: usize,
    pub crafting_selected_recipe: usize,
    pub crafting_scroll_offset: f32,
    pub crafting_npc_id: Option<String>,
    // Crafting progress state (timed crafting)
    pub crafting_in_progress: bool,
    pub crafting_recipe_id: Option<String>,
    pub crafting_progress: f32,
    pub crafting_duration_ms: u64,
    pub crafting_started_at: Option<f64>,
    // Crafting completion animation: (recipe_id, timer_0_to_1)
    pub crafting_complete_animation: Option<(String, f32)>,
    // Shop UI state
    pub shop_data: Option<ShopData>,
    pub shop_npc_id: Option<String>,
    pub shop_sub_tab: ShopSubTab,
    pub shop_main_tab: usize, // 0=Recipes, 1=Shop
    pub shop_selected_buy_index: usize,
    pub shop_selected_sell_index: usize,
    pub shop_buy_quantity: i32,
    pub shop_sell_quantity: i32,
    pub shop_buy_scroll: f32,  // Scroll offset for buy list (pixels)
    pub shop_sell_scroll: f32, // Scroll offset for sell list (pixels)
    // Touch drag scroll tracking for shop lists
    pub shop_touch_scroll_id: Option<u64>,
    pub shop_touch_scroll_column: u8, // 0=buy, 1=sell
    pub shop_touch_last_y: f32,
    pub shop_touch_start_y: f32,
    pub shop_touch_dragged: bool,
    // Hold-to-repeat for quantity +/- buttons
    pub shop_quantity_hold_element: Option<UiElementId>,
    pub shop_quantity_hold_start: f64,
    pub shop_quantity_hold_last_repeat: f64,
    // Escape menu state
    pub escape_menu_open: bool,
    // Audio settings (synced with AudioManager)
    pub audio_volume: f32,
    pub audio_sfx_volume: f32,
    pub audio_muted: bool,
    // UI scale (0.75 to 1.25, default 1.0 on desktop, 0.75 on mobile)
    pub ui_scale: f32,
    // Input settings
    pub shift_drop_enabled: bool,
    // Menu button panel states
    pub social_open: bool,
    pub skills_open: bool,
    pub character_panel_open: bool,
    pub prayer_book_open: bool,
    // Mouse hover state for UI elements
    pub hovered_element: Option<UiElementId>,
    // Context menu state
    pub context_menu: Option<ContextMenu>,
    // Gold drop dialog state
    pub gold_drop_dialog: Option<GoldDropDialog>,
    // Altar offering panel state
    pub altar_panel: Option<AltarPanelState>,
    // Drag state for inventory slot rearrangement
    pub drag_state: Option<DragState>,
    // Double-click tracking for equipping items
    pub double_click_state: DoubleClickState,
    // Server announcements
    pub announcements: Vec<Announcement>,
    // Chat log visibility (hidden by default on mobile)
    pub chat_log_visible: bool,
    // Mobile chat panel
    pub chat_panel_open: bool,
    pub chat_active_tab: ChatChannel,
    // Tap-to-pathfind (enabled by default on mobile, disabled on desktop)
    pub tap_to_pathfind: bool,
    // Use joystick instead of D-pad for mobile controls
    pub use_joystick: bool,
    // Dialogue scroll offset and touch scroll tracking
    pub dialogue_scroll_offset: f32,
    pub dialogue_touch_scroll_id: Option<u64>,
    pub dialogue_touch_last_y: f32,
    pub dialogue_touch_start_y: f32,
    pub dialogue_touch_dragged: bool,
    pub dialogue_scrollbar_dragging: bool,
    pub dialogue_scrollbar_drag_last_y: f32,
    // Inventory grid scroll offset (for small screens where not all rows fit)
    pub inventory_scroll_offset: f32,
    // Touch drag scroll tracking for inventory grid
    pub inventory_touch_scroll_id: Option<u64>,
    pub inventory_touch_last_y: f32,
    pub inventory_scrollbar_dragging: bool,
    pub inventory_scrollbar_drag_last_y: f32,
    // Control scheme: false = Modern (WASD+Space+Enter), true = Classic (Arrows+Ctrl+always-on chat)
    pub classic_controls: bool,
    /// Active tab in the prayer/spell panel: 0 = Prayers, 1 = Spells
    pub prayer_spell_tab: usize,
    /// Whether the spell bar is active (true) or item bar (false)
    pub spell_bar_active: bool,
    /// Whether prayer help overlay is open
    pub prayer_help_open: bool,
    /// Whether spell help overlay is open
    pub spell_help_open: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            chat_open: false,
            chat_input: String::new(),
            chat_cursor: 0,
            chat_scroll_offset: 0,
            chat_message_scroll: 0,
            chat_key_repeat_time: 0.0,
            chat_key_initial_delay: true,
            chat_messages: Vec::new(),
            inventory_open: false,
            active_dialogue: None,
            active_quests: Vec::new(),
            quest_completed_events: Vec::new(),
            quest_log_open: false,
            crafting_open: false,
            crafting_selected_category: 0,
            crafting_selected_recipe: 0,
            crafting_scroll_offset: 0.0,
            crafting_npc_id: None,
            crafting_in_progress: false,
            crafting_recipe_id: None,
            crafting_progress: 0.0,
            crafting_duration_ms: 0,
            crafting_started_at: None,
            crafting_complete_animation: None,
            shop_data: None,
            shop_npc_id: None,
            shop_sub_tab: ShopSubTab::Buy,
            shop_main_tab: 0,
            shop_selected_buy_index: 0,
            shop_selected_sell_index: 0,
            shop_buy_quantity: 1,
            shop_sell_quantity: 1,
            shop_buy_scroll: 0.0,
            shop_sell_scroll: 0.0,
            shop_touch_scroll_id: None,
            shop_touch_scroll_column: 0,
            shop_touch_last_y: 0.0,
            shop_touch_start_y: 0.0,
            shop_touch_dragged: false,
            shop_quantity_hold_element: None,
            shop_quantity_hold_start: 0.0,
            shop_quantity_hold_last_repeat: 0.0,
            escape_menu_open: false,
            audio_volume: 0.7,
            audio_sfx_volume: 0.7,
            audio_muted: false,
            #[cfg(target_os = "android")]
            ui_scale: 0.75,
            #[cfg(not(target_os = "android"))]
            ui_scale: 1.0,
            shift_drop_enabled: true,
            social_open: false,
            skills_open: false,
            character_panel_open: false,
            prayer_book_open: false,
            hovered_element: None,
            context_menu: None,
            gold_drop_dialog: None,
            altar_panel: None,
            drag_state: None,
            double_click_state: DoubleClickState {
                last_click_slot: None,
                last_click_time: 0.0,
            },
            announcements: Vec::new(),
            #[cfg(target_os = "android")]
            chat_log_visible: false,
            #[cfg(not(target_os = "android"))]
            chat_log_visible: true,
            chat_panel_open: false,
            chat_active_tab: ChatChannel::Local,
            #[cfg(target_os = "android")]
            tap_to_pathfind: false,
            #[cfg(not(target_os = "android"))]
            tap_to_pathfind: true,
            use_joystick: false,
            dialogue_scroll_offset: 0.0,
            dialogue_touch_scroll_id: None,
            dialogue_touch_last_y: 0.0,
            dialogue_touch_start_y: 0.0,
            dialogue_touch_dragged: false,
            dialogue_scrollbar_dragging: false,
            dialogue_scrollbar_drag_last_y: 0.0,
            inventory_scroll_offset: 0.0,
            inventory_touch_scroll_id: None,
            inventory_touch_last_y: 0.0,
            inventory_scrollbar_dragging: false,
            inventory_scrollbar_drag_last_y: 0.0,
            classic_controls: false,
            prayer_spell_tab: 0,
            spell_bar_active: false,
            prayer_help_open: false,
            spell_help_open: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
}

pub struct GameState {
    // Connection
    pub connection_status: ConnectionStatus,
    pub local_player_id: Option<String>,
    pub selected_character_name: Option<String>,
    pub disconnect_requested: bool,
    pub reconnection_failed: bool,
    /// Timestamp of last Face command sent (to ignore stale server direction updates)
    pub last_face_command_time: f64,

    // World
    pub tilemap: Tilemap,
    pub chunk_manager: ChunkManager,
    pub players: HashMap<String, Player>,
    pub npcs: HashMap<String, Npc>,
    pub ground_items: HashMap<String, GroundItem>,
    /// Items waiting to spawn (with spawn time) - delays loot appearance until after death animation
    pub pending_ground_items: Vec<(GroundItem, f64)>,
    /// Farming patches received from server
    pub farming_patches: HashMap<String, FarmingPatch>,
    /// Farming patch lookup by position
    pub farming_patch_positions: HashMap<(i32, i32), String>,
    /// Gathering marker positions received from server
    pub gathering_markers: Vec<GatheringMarker>,
    /// Whether the local player is currently gathering
    pub is_gathering: bool,
    /// Whether the local player is currently sitting on a chair
    pub is_sitting: bool,
    /// Chair positions on the map (received from server)
    pub chair_positions: Vec<(i32, i32)>,
    /// Pending chair to sit on after pathfinding completes
    pub pending_chair_sit: Option<(i32, i32)>,
    pub pending_harvest_patch: Option<String>,
    /// Timestamp when gathering started (for cast animation delay)
    pub gathering_started_at: f64,
    /// Active bonus tile events
    pub bonus_tiles: Vec<BonusTile>,
    /// Active gathering buff on local player
    pub gathering_buff: Option<GatheringBuff>,

    // Targeting
    pub selected_entity_id: Option<String>,

    // Combat feedback
    pub damage_events: Vec<DamageEvent>,
    pub level_up_events: Vec<LevelUpEvent>,
    /// Pending sound effects to play (queued by message handler, played by main loop)
    pub pending_sfx: Vec<String>,
    pub firework_particles: Vec<FireworkParticle>,
    pub skill_xp_events: Vec<SkillXpEvent>,
    pub xp_globes: XpGlobesManager,
    pub xp_drop_feed: XpDropFeed,
    pub projectiles: Vec<Projectile>,

    // Chat bubbles above players
    pub chat_bubbles: Vec<ChatBubble>,

    // Inventory
    pub inventory: Inventory,

    // Item registry (loaded from server)
    pub item_registry: ItemRegistry,

    // Crafting
    pub recipe_definitions: Vec<RecipeDefinition>,
    pub discovered_recipes: HashSet<String>,

    // Camera and UI
    pub camera: Camera,
    pub ui_state: UiState,

    // Server tick (for ordering)
    pub server_tick: u64,

    // Debug
    pub debug_mode: bool,

    // Tile hover state (world coordinates of tile under mouse)
    pub hovered_tile: Option<(i32, i32)>,

    // Entity hover state (ID of entity under mouse cursor)
    pub hovered_entity_id: Option<String>,

    // Automated pathfinding state
    pub auto_path: Option<PathState>,

    // Performance diagnostics (visible in debug mode)
    pub frame_timings: FrameTimings,

    // Map transition state
    pub map_transition: MapTransition,

    /// Current interior map ID if in an interior (None = overworld)
    pub current_interior: Option<String>,
    /// Current instance ID if in an instance
    pub current_instance: Option<String>,
    /// Pending portal to enter (set when player walks onto a portal)
    pub pending_portal_id: Option<String>,
    /// Last tile position checked for portal (to avoid triggering on spawn)
    pub last_portal_check_pos: Option<(i32, i32)>,
    /// Area banner for displaying location names during transitions
    pub area_banner: AreaBanner,

    /// Social/Friends system state
    pub social_state: SocialState,

    // Spell system state
    /// Active spell effects for rendering
    pub spell_effects: Vec<SpellEffect>,
    /// Spell cooldowns tracked on client for UI feedback
    pub spell_cooldowns: std::collections::HashMap<String, f64>,  // spell_id -> time when cooldown expires

    // Prayer system state
    /// Current prayer points
    pub prayer_points: i32,
    /// Maximum prayer points (based on prayer level)
    pub max_prayer_points: i32,
    /// Currently active prayers (by prayer ID)
    pub active_prayers: Vec<String>,
}

impl GameState {
    pub fn new() -> Self {
        // Create a test tilemap (32x32 tiles) - kept for compatibility
        let tilemap = Tilemap::new_test_map(32, 32);

        Self {
            connection_status: ConnectionStatus::Disconnected,
            local_player_id: None,
            selected_character_name: None,
            disconnect_requested: false,
            reconnection_failed: false,
            last_face_command_time: 0.0,
            tilemap,
            chunk_manager: ChunkManager::new(),
            players: HashMap::new(),
            npcs: HashMap::new(),
            ground_items: HashMap::new(),
            pending_ground_items: Vec::new(),
            farming_patches: HashMap::new(),
            farming_patch_positions: HashMap::new(),
            gathering_markers: Vec::new(),
            is_gathering: false,
            is_sitting: false,
            chair_positions: Vec::new(),
            pending_chair_sit: None,
            pending_harvest_patch: None,
            gathering_started_at: 0.0,
            bonus_tiles: Vec::new(),
            gathering_buff: None,
            selected_entity_id: None,
            damage_events: Vec::new(),
            level_up_events: Vec::new(),
            pending_sfx: Vec::new(),
            firework_particles: Vec::new(),
            skill_xp_events: Vec::new(),
            xp_globes: XpGlobesManager::new(),
            xp_drop_feed: XpDropFeed::new(),
            projectiles: Vec::new(),
            chat_bubbles: Vec::new(),
            inventory: Inventory::new(),
            item_registry: ItemRegistry::new(),
            recipe_definitions: Vec::new(),
            discovered_recipes: HashSet::new(),
            camera: Camera::default(),
            ui_state: UiState::default(),
            server_tick: 0,
            debug_mode: false,
            hovered_tile: None,
            hovered_entity_id: None,
            auto_path: None,
            frame_timings: FrameTimings::default(),
            map_transition: MapTransition::default(),
            current_interior: None,
            current_instance: None,
            pending_portal_id: None,
            last_portal_check_pos: None,
            area_banner: AreaBanner::default(),
            social_state: SocialState::default(),
            spell_effects: Vec::new(),
            spell_cooldowns: std::collections::HashMap::new(),
            prayer_points: 0,
            max_prayer_points: 1,
            active_prayers: Vec::new(),
        }
    }

    /// Clear the current auto-path (e.g., when player presses movement keys)
    pub fn clear_auto_path(&mut self) {
        self.auto_path = None;
    }

    /// Update all players - simple server-authoritative model
    /// Local player facing is immediate when stationary, movement direction from server
    pub fn update(&mut self, delta: f32, input_dx: f32, input_dy: f32) {
        // Use smoothed delta for visual interpolation (reduces jitter from frame variance)
        let visual_delta = self.frame_timings.smoothed_delta;

        // Update local player facing immediately when stationary (responsive feel)
        // Skip when sitting - chair controls direction
        if let Some(local_id) = &self.local_player_id {
            if let Some(player) = self.players.get_mut(local_id) {
                // Only update direction from input when stationary and not attacking/sitting
                let is_stationary = !player.is_moving && player.vel_x == 0.0 && player.vel_y == 0.0;
                let is_attacking = matches!(
                    player.animation.state,
                    AnimationState::Attacking | AnimationState::Casting | AnimationState::ShootingBow
                );
                let is_sitting = matches!(
                    player.animation.state,
                    AnimationState::SittingChair | AnimationState::SittingGround
                );
                if is_stationary && !is_attacking && !is_sitting && (input_dx != 0.0 || input_dy != 0.0) {
                    let new_dir = super::entities::Direction::from_velocity(input_dx, input_dy);
                    player.direction = new_dir;
                    player.animation.direction = new_dir;
                }
            }
        }

        // Update all players (smooth interpolation toward server positions)
        for player in self.players.values_mut() {
            player.interpolate_visual(visual_delta);
        }

        // Update camera to follow local player
        if let Some(local_id) = &self.local_player_id {
            if let Some(player) = self.players.get(local_id) {
                self.camera.x = player.x;
                self.camera.y = player.y;
                self.camera.initialized = true;
            }
        }

        // Update NPCs (interpolation toward server positions)
        for npc in self.npcs.values_mut() {
            npc.update(visual_delta);
        }

        // Process pending ground items (spawn them after delay)
        let current_time = macroquad::time::get_time();
        let mut i = 0;
        while i < self.pending_ground_items.len() {
            if current_time >= self.pending_ground_items[i].1 {
                let (item, _) = self.pending_ground_items.swap_remove(i);
                self.ground_items.insert(item.id.clone(), item);
            } else {
                i += 1;
            }
        }

        // Clean up old damage events (older than 1.2 seconds)
        self.damage_events.retain(|event| current_time - event.time < 1.2);

        // Clean up old level up events (older than 2.0 seconds)
        self.level_up_events.retain(|event| current_time - event.time < 1.2);

        // Update firework particles (screen-pixel physics)
        let mut new_sparks = Vec::new();
        for p in &mut self.firework_particles {
            // Record trail
            p.trail.push((p.ox, p.oy));
            if p.trail.len() > 5 {
                p.trail.remove(0);
            }

            p.vx *= 0.94;
            p.vy += 300.0 * delta;
            p.ox += p.vx * delta;
            p.oy += p.vy * delta;

            // Main particles burst into sparks when they start falling
            let age = (current_time - p.time) as f32;
            if !p.is_spark && !p.has_burst && age > 0.15 && p.vy > 0.0 {
                p.has_burst = true;
                for j in 0..5 {
                    let a = (j as f32 / 5.0) * std::f32::consts::TAU;
                    let s = 30.0 + (j as f32 % 3.0) * 15.0;
                    new_sparks.push(FireworkParticle {
                        origin_x: p.origin_x,
                        origin_y: p.origin_y,
                        ox: p.ox,
                        oy: p.oy,
                        vx: a.cos() * s,
                        vy: a.sin() * s,
                        color: p.color,
                        time: current_time,
                        size: 1.5,
                        trail: Vec::new(),
                        is_spark: true,
                        has_burst: true,
                    });
                }
            }
        }
        self.firework_particles.extend(new_sparks);
        self.firework_particles.retain(|p| {
            let age = current_time - p.time;
            if p.is_spark { age < 0.5 } else { age < 1.0 }
        });

        // Clean up old skill XP events (older than 1.5 seconds)
        self.skill_xp_events.retain(|event| current_time - event.time < 1.5);

        // Update and clean up XP drops
        self.xp_drop_feed.update(delta);
        self.xp_drop_feed.drops.retain(|drop| current_time - drop.time < 2.0);

        // Clean up old chat bubbles (older than 5.0 seconds)
        self.chat_bubbles.retain(|bubble| current_time - bubble.time < 5.0);

        // Clean up expired NPC speech bubbles
        for npc in self.npcs.values_mut() {
            if let Some((_, time)) = &npc.speech_bubble {
                if current_time - time > 5.0 {
                    npc.speech_bubble = None;
                }
            }
        }

        // Clean up completed projectiles
        self.projectiles.retain(|p| !p.is_complete(current_time));

        // Clean up finished spell effects (max 3 seconds as fallback)
        self.spell_effects.retain(|effect| {
            let elapsed = current_time - effect.time;
            elapsed < 3.0
        });

        // Clean up old quest completion events (older than 4 seconds)
        self.ui_state.quest_completed_events.retain(|event| current_time - event.time < 4.0);

        // Clean up old announcements (older than 8 seconds)
        self.ui_state.announcements.retain(|ann| current_time - ann.time < 8.0);

        // Update crafting progress (Task 14)
        if self.ui_state.crafting_in_progress {
            if let Some(started) = self.ui_state.crafting_started_at {
                let elapsed = ((macroquad::time::get_time() - started) * 1000.0) as f32;
                let duration = self.ui_state.crafting_duration_ms as f32;
                if duration > 0.0 {
                    self.ui_state.crafting_progress = (elapsed / duration).min(1.0);
                }
            }
        }

        // Update crafting completion animation timer (Task 20)
        if let Some((_, ref mut timer)) = self.ui_state.crafting_complete_animation {
            *timer += delta; // ~1 second animation
            if *timer >= 1.0 {
                // Animation done
            }
        }
        if self.ui_state.crafting_complete_animation.as_ref().map_or(false, |(_, t)| *t >= 1.0) {
            self.ui_state.crafting_complete_animation = None;
        }

        // Update area banner timer
        self.area_banner.update(delta);

        // Update XP globes (fade timers, hover detection)
        // Calculate globe position to match renderer
        let margin = 12.0;
        let base_y = 25.0;
        let tag_height = 22.0;
        let bar_width = 120.0_f32.max(140.0);
        let (vw, _) = crate::util::virtual_screen_size();
        let bar_x = (vw - bar_width - margin).floor();
        let globe_stats_y = base_y + tag_height / 2.0 + 8.0;
        self.xp_globes.update(bar_x, globe_stats_y);
    }

    pub fn get_local_player(&self) -> Option<&Player> {
        self.local_player_id.as_ref().and_then(|id| self.players.get(id))
    }

    /// Update map transition animation
    pub fn update_transition(&mut self, delta: f32) {
        const FADE_DURATION: f32 = 0.25;

        match self.map_transition.state {
            TransitionState::FadingOut => {
                self.map_transition.progress += delta / FADE_DURATION;
                if self.map_transition.progress >= 1.0 {
                    self.map_transition.progress = 1.0;
                    self.map_transition.state = TransitionState::Loading;
                }
            }
            TransitionState::FadingIn => {
                self.map_transition.progress -= delta / FADE_DURATION;
                if self.map_transition.progress <= 0.0 {
                    self.map_transition.progress = 0.0;
                    self.map_transition.state = TransitionState::None;
                }
            }
            _ => {}
        }
    }

    /// Start a map transition
    pub fn start_transition(&mut self, map_type: String, map_id: String, spawn_x: f32, spawn_y: f32, instance_id: String) {
        self.map_transition = MapTransition {
            state: TransitionState::FadingOut,
            progress: 0.0,
            target_map_type: map_type,
            target_map_id: map_id,
            target_spawn_x: spawn_x,
            target_spawn_y: spawn_y,
            instance_id,
        };
    }

    /// Check if input should be blocked due to transition
    pub fn is_transitioning(&self) -> bool {
        self.map_transition.state != TransitionState::None
    }
}
