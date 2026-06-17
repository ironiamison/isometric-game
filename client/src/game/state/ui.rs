use super::*;

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

/// A quest from the server catalog (static info for all quests)
pub struct QuestCatalogEntry {
    pub quest_id: String,
    pub name: String,
    pub description: String,
    pub giver_npc_name: String,
    pub level_required: i32,
    pub required_quest_id: Option<String>,
    pub required_quest_name: Option<String>,
    pub objectives: Vec<CatalogObjective>,
}

/// Objective definition from the quest catalog (static, no progress)
pub struct CatalogObjective {
    pub id: String,
    pub description: String,
    pub target: i32,
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
    pub state: String, // "empty", "growing", "harvestable", "diseased", "dead"
    pub crop_id: String,
    pub growth_stage: u32,
    pub owner_id: String,
    pub health: String,
    pub lives_remaining: u32,
    pub composted: bool,
    pub patch_type: String,
    pub width: u32,
    pub height: u32,
    pub capacity: u32,
}

/// Active resource contract info received from server
#[derive(Debug, Clone)]
pub struct ResourceContractInfo {
    pub contract_kind: String,
    pub difficulty: String,
    pub task_text: String,
    pub progress_label: String,
    /// Item the player must collect — rendered as the contract chip's icon.
    pub target_item_id: String,
    pub amount_required: i32,
    pub amount_completed: i32,
    pub giver_name: String,
}

#[derive(Debug, Clone)]
pub struct AdventureBoardDifficultyInfo {
    pub difficulty_id: String,
    pub difficulty_name: String,
    pub level_required: i32,
    pub unlocked: bool,
    pub reward_xp: i64,
    pub reward_gold: i32,
}

#[derive(Debug, Clone)]
pub struct AdventureBoardOfferInfo {
    pub kind_id: String,
    pub kind_name: String,
    pub description: String,
    pub skill_level: i32,
    pub difficulties: Vec<AdventureBoardDifficultyInfo>,
}

#[derive(Debug, Clone)]
pub struct AdventureBoardActiveContractInfo {
    pub kind_id: String,
    pub kind_name: String,
    pub difficulty_name: String,
    pub task_text: String,
    pub progress_label: String,
    pub amount_required: i32,
    pub amount_completed: i32,
    pub giver_name: String,
    pub reward_xp: i64,
    pub reward_gold: i32,
    pub bonus_item_text: String,
    pub can_claim: bool,
}

#[derive(Debug, Clone)]
pub struct AdventureBoardStatsInfo {
    pub contracts_completed: i32,
    pub total_gold_earned: i32,
    pub total_xp_earned: i64,
}

#[derive(Debug, Clone)]
pub struct AdventureBoardPanelState {
    pub npc_id: String,
    pub offers: Vec<AdventureBoardOfferInfo>,
    pub active_contract: Option<AdventureBoardActiveContractInfo>,
    pub stats: AdventureBoardStatsInfo,
    pub crafting_orders: Vec<CraftingOrderOfferInfo>,
    pub crafting_order_active: Option<CraftingOrderActiveInfo>,
    pub crafting_order_stats: CraftingOrderStatsInfo,
    pub daily_contracts_completed: i32,
    pub daily_contract_limit: i32,
}

#[derive(Debug, Clone)]
pub struct CraftingOrderOfferInfo {
    pub order_id: String,
    pub tier: String,
    pub skill: String,
    pub min_level: i32,
    pub items: Vec<CraftingOrderItemInfo>,
    pub reward_gold: i32,
    pub reward_xp: Vec<(String, i64)>,
    pub reward_marks: i32,
}

#[derive(Debug, Clone)]
pub struct CraftingOrderItemInfo {
    pub item_id: String,
    pub item_name: String,
    pub quantity: i32,
}

#[derive(Debug, Clone)]
pub struct CraftingOrderActiveInfo {
    pub order_id: String,
    pub tier: String,
    pub skill: String,
    pub items: Vec<CraftingOrderItemInfo>,
    pub reward_gold: i32,
    pub reward_marks: i32,
    pub can_claim: bool,
}

#[derive(Debug, Clone)]
pub struct CraftingOrderStatsInfo {
    pub orders_completed: i32,
    pub masterwork_completed: i32,
    pub commission_marks: i32,
}

/// A gathering marker tile in the world (fishing spot, mining node, etc.)
#[derive(Debug, Clone)]
pub struct GatheringMarker {
    pub x: i32,
    pub y: i32,
    pub zone_id: String,
    pub skill: String,
}

/// An active gathering buff on a player
#[derive(Debug, Clone)]
pub struct GatheringBuff {
    pub buff_type: String,
    pub start_time: f64,
    pub duration: f64,
}

/// An active potion buff (attack/strength/defence boost)
#[derive(Debug, Clone)]
pub struct ActivePotionBuff {
    pub stat: String,
    pub amount: i32,
    pub expires_at: f64, // local macroquad time when buff expires
    pub source_item_id: String,
}

/// Target for context menu - what was right-clicked
#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    // UI targets
    InventorySlot(usize),
    EquipmentSlot(String),
    Gold,
    // World targets
    Player { id: String },
    Npc { id: String },
    Tree { tile_x: i32, tile_y: i32, gid: u32 },
    Rock { tile_x: i32, tile_y: i32, gid: u32 },
    MapObject { tile_x: i32, tile_y: i32, gid: u32 },
    GatheringSpot { marker_index: usize },
    GroundItem { id: String },
    FarmingPatch { patch_id: String },
    Tile { x: i32, y: i32 },
    HotkeySlot(usize),
    Spell(String),
    QuestTracker,
    ChatTab,
    BankSlot(usize),
    BankInventorySlot(usize),
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

/// Dialog for entering stall item price
#[derive(Debug, Clone)]
pub struct StallPriceDialog {
    pub input: String,
    pub cursor: usize,
    pub inventory_slot: u8,
    pub quantity: i32,
    pub item_id: String,
}

/// What action the bank quantity dialog will perform on confirm
#[derive(Debug, Clone, PartialEq)]
pub enum BankQuantityAction {
    DepositItem,
    WithdrawItem,
    DepositGold,
    WithdrawGold,
}

/// Dialog for entering a custom quantity in the bank UI (Ctrl+Click)
#[derive(Debug, Clone)]
pub struct BankQuantityDialog {
    pub input: String,
    pub cursor: usize,
    pub action: BankQuantityAction,
    pub item_id: Option<String>,
    pub max_quantity: i32,
}

/// Tracks an active drag operation within the bank grid
pub struct BankDrag {
    pub from_slot: usize,
    pub mouse_start_x: f32,
    pub mouse_start_y: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub active: bool,
}

/// Source of a drag operation
#[derive(Debug, Clone, PartialEq)]
pub enum DragSource {
    Inventory(usize),  // Inventory slot index
    Equipment(String), // Equipment slot type ("body", "feet")
    Spell(String),     // Spell ID for drag-to-hotkey
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
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SocialTab {
    #[default]
    Nearby,
    Online,
    Friends,
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

/// Boss fight client state
#[derive(Debug, Clone)]
pub struct BossClientState {
    pub boss_id: String,
    pub hp: i32,
    pub max_hp: i32,
    pub phase: String,
    pub wurm_state: String,
}

/// Active Mark of Death (Reaper boss). The marked player must reach a Soul Ward
/// before it expires or the Reaper claims their soul.
#[derive(Debug, Clone)]
pub struct ReaperMarkState {
    pub player_id: String,
    /// Client time (seconds) when the mark was received.
    pub created_at: f64,
    /// How long the player has to cleanse, in milliseconds.
    pub duration_ms: u64,
}

/// AOE warning zone being displayed
#[derive(Debug, Clone)]
pub struct AoeWarningZone {
    pub tiles: Vec<(i32, i32)>,
    pub created_at: f64,
    pub delay_ms: u64,
    pub effect: String,
}

/// Active explosion effect
#[derive(Debug, Clone)]
pub struct ExplosionEffect {
    pub x: i32,
    pub y: i32,
    pub radius: i32,
    pub created_at: f64,
}

/// KOTH minigame client state
#[derive(Debug, Clone)]
pub struct KothClientState {
    pub phase: String,
    pub wave: u32,
    pub points: u32,
    pub enemies_alive: u32,
    pub enemies_total: u32,
    pub countdown_ms: u32,
}

/// KOTH checkpoint reward preview
#[derive(Debug, Clone)]
pub struct KothCheckpointInfo {
    pub wave: u32,
    pub points: u32,
    pub rewards: Vec<KothRewardPreview>,
    pub next_wave_enemy_count: u32,
}

/// KOTH game over info
#[derive(Debug, Clone)]
pub struct KothGameOverInfo {
    pub waves_completed: u32,
    pub total_points: u32,
    pub rewards: Vec<KothRewardPreview>,
    pub victory: bool,
    pub shown_at: f64,
}

/// A reward item preview
#[derive(Debug, Clone)]
pub struct KothRewardPreview {
    pub item_id: String,
    pub quantity: u32,
}

/// A single item in a trade offer
#[derive(Debug, Clone)]
pub struct TradeOfferItem {
    pub slot_index: u8,
    pub item_id: String,
    pub quantity: i32,
}

/// A stall slot with item, quantity and price
#[derive(Debug, Clone)]
pub struct StallSlotInfo {
    pub slot: u8,
    pub item_id: String,
    pub quantity: i32,
    pub price: i32,
}

/// Data for browsing another player's stall
#[derive(Debug, Clone)]
pub struct StallBrowseInfo {
    pub seller_id: String,
    pub seller_name: String,
    pub stall_name: String,
    pub items: Vec<StallSlotInfo>,
}

pub struct UiState {
    pub chat_open: bool,
    /// When true, the Enter→open-chat shortcut is ignored until Enter is released.
    /// Set when entering gameplay so the Enter press used to log in on the character
    /// select screen doesn't immediately pop the chat input open.
    pub suppress_enter_chat_open: bool,
    pub chat_input: String,
    pub chat_cursor: usize, // Cursor position in chat_input (character index)
    pub chat_scroll_offset: usize, // Scroll offset for long messages (character index)
    pub chat_message_scroll: f32, // Scroll offset for message list (in pixels from bottom)
    pub chat_scroll_drag: crate::ui::scroll::ScrollDragState,
    pub chat_key_repeat_time: f64, // Last time a repeated key action fired
    pub chat_key_initial_delay: bool, // Whether we're still in initial delay
    /// Set once per frame when a genuine Ctrl/Cmd+V was pressed (from the key event's live
    /// modifier flags). Avoids relying on is_key_down, which the OS leaves stuck "down" after
    /// focus loss with the key held, making every plain `v` paste.
    pub paste_requested: bool,
    pub chat_messages: ChatLog,
    pub chat_revision: u64, // Increments whenever chat content changes (for render cache invalidation)
    pub inventory_open: bool,
    // Quest UI state
    pub active_dialogue: Option<ActiveDialogue>,
    pub adventure_board: Option<AdventureBoardPanelState>,
    pub adventure_board_selected_offer: usize,
    pub adventure_board_tab: u8, // 0 = Contracts, 1 = Orders
    pub adventure_board_selected_order: usize,
    pub active_quests: Vec<ActiveQuest>,
    pub completed_quest_ids: HashSet<String>,
    pub adventurer_selected_tab: usize,
    pub adventurer_selected_tier: usize,
    pub quest_completed_events: Vec<QuestCompletedEvent>,
    pub quest_log_open: bool,
    pub quest_tracker_minimized: bool,
    pub quest_tracker_rect: std::cell::Cell<Option<macroquad::math::Rect>>,
    pub quest_log_scroll: f32,
    pub quest_catalog: Vec<QuestCatalogEntry>,
    pub selected_quest_id: Option<String>,
    // Collection log UI state
    pub collection_log_open: bool,
    /// Static definitions: Vec of (item_id, source, source_detail)
    pub collection_log_definitions: Vec<(String, String, String)>,
    /// Player's obtained items: HashMap of (item_id, source) -> obtained_at
    pub collection_log: std::collections::HashMap<(String, String), String>,
    /// Display names for source_detail IDs (e.g., "pig" -> "Pig", "fishing" -> "Fishing")
    pub collection_log_display_names: std::collections::HashMap<String, String>,
    pub collection_log_selected_category: Option<String>,
    pub collection_log_selected_subcategory: Option<String>,
    pub collection_log_sidebar_scroll: f32,
    pub collection_log_grid_scroll: f32,
    pub collection_scroll_drag: crate::ui::scroll::ScrollDragState,
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
    // Furnace / Fire Pit UI state (shared — fire pit reuses furnace UI)
    pub furnace_open: bool,
    pub furnace_tile: Option<(i32, i32)>,
    pub furnace_selected_recipe: usize,
    pub furnace_scroll_offset: f32,
    pub furnace_quantity: u32,
    pub furnace_tab: u8,
    pub furnace_station_type: String, // "furnace" or "fire_pit"
    // Anvil UI state
    pub anvil_open: bool,
    pub anvil_tile: Option<(i32, i32)>,
    pub anvil_selected_recipe: usize,
    pub anvil_scroll_offset: f32,
    pub anvil_quantity: u32,
    pub anvil_tab: u8, // 0=Materials, 1=Equipment
    // Alchemy Station UI state
    pub alchemy_station_open: bool,
    pub alchemy_station_tile: Option<(i32, i32)>,
    pub alchemy_station_selected_recipe: usize,
    pub alchemy_station_scroll_offset: f32,
    pub alchemy_station_quantity: u32,
    pub alchemy_station_tab: u8,
    // Workbench UI state
    pub workbench_open: bool,
    pub workbench_tile: Option<(i32, i32)>,
    pub workbench_selected_recipe: usize,
    pub workbench_scroll_offset: f32,
    pub workbench_quantity: u32,
    pub workbench_tab: u8,
    // Fletching panel (tool-based, no station)
    pub fletching_open: bool,
    pub fletching_selected_recipe: usize,
    pub fletching_scroll_offset: f32,
    pub fletching_quantity: u32,
    pub fletching_tab: u8,
    // Batch progress (shared between furnace and regular crafting)
    pub batch_completed: u32,
    pub batch_total: u32,
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
    // Bank UI state
    pub bank_open: bool,
    pub bank_slots: Vec<Option<(String, i32)>>, // (item_id, quantity) per slot
    pub bank_gold: i32,
    pub bank_max_slots: u32,
    pub bank_scroll: f32,
    pub bank_inv_scroll: f32,
    pub bank_scroll_drag: crate::ui::scroll::ScrollDragState,
    pub bank_inv_scroll_drag: crate::ui::scroll::ScrollDragState,
    pub bank_drag: Option<BankDrag>,
    // Chest UI state
    pub chest_open: bool,
    pub chest_id: String,
    pub chest_name: String,
    pub chest_slots: Vec<Option<(String, i32, i32)>>, // (item_id, quantity, value) per slot
    pub chest_total_value: i32,
    pub chest_scroll: f32,
    // Escape menu state
    pub escape_menu_open: bool,
    // Mobile menu toggle (collapsed/expanded)
    pub mobile_menu_expanded: bool,
    // Audio settings (synced with AudioManager)
    pub audio_volume: f32,
    pub audio_sfx_volume: f32,
    pub music_muted: bool,
    pub sfx_muted: bool,
    // UI scale (0.75 to 2.0, default 1.0; fixed at 1.0 on Android)
    pub ui_scale: f32,
    // Pending scale while dragging the settings slider. The live `ui_scale` is
    // only updated on mouse release, so the panel doesn't rescale mid-drag.
    pub ui_scale_pending: Option<f32>,
    // Input settings
    pub shift_drop_enabled: bool,
    // Menu button panel states
    pub social_open: bool,
    pub skills_open: bool,
    pub character_panel_open: bool,
    pub prayer_book_open: bool,
    pub minimap_panel_open: bool,
    pub minimap_panel_zoom: f32,
    pub minimap_panel_center_x: Option<f32>,
    pub minimap_panel_center_y: Option<f32>,
    pub minimap_panel_dragging: bool,
    pub minimap_panel_drag_last_x: f32,
    pub minimap_panel_drag_last_y: f32,
    // Mouse hover state for UI elements
    pub hovered_element: Option<UiElementId>,
    // Context menu state
    pub context_menu: Option<ContextMenu>,
    // Gold drop dialog state
    pub gold_drop_dialog: Option<GoldDropDialog>,
    // Bank quantity dialog state (Ctrl+Click in bank)
    pub bank_quantity_dialog: Option<BankQuantityDialog>,
    // Bank help overlay open
    pub bank_help_open: bool,
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
    // Chat log semi-transparent background
    pub chat_log_background: bool,
    // Mobile chat panel
    pub chat_panel_open: bool,
    pub chat_active_tab: ChatChannel,
    // Last read message timestamp per tab (for unread tab highlighting)
    pub chat_last_seen_local: f64,
    pub chat_last_seen_global: f64,
    pub chat_last_seen_system: f64,
    // Hide system messages in the Public/Local chat tab
    pub hide_system_in_public: bool,
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
    pub dialogue_scroll_drag: crate::ui::scroll::ScrollDragState,
    // Selected inventory slot (for "use item on entity" flow)
    pub selected_inventory_slot: Option<usize>,
    // Inventory grid scroll offset (for small screens where not all rows fit)
    pub inventory_scroll_offset: f32,
    // Touch drag scroll tracking for inventory grid
    pub inventory_touch_scroll_id: Option<u64>,
    pub inventory_touch_last_y: f32,
    pub inventory_scroll_drag: crate::ui::scroll::ScrollDragState,
    // Scrollbar drag states for panels
    pub shop_buy_scroll_drag: crate::ui::scroll::ScrollDragState,
    pub shop_sell_scroll_drag: crate::ui::scroll::ScrollDragState,
    pub quest_log_scroll_drag: crate::ui::scroll::ScrollDragState,
    pub furnace_scroll_drag: crate::ui::scroll::ScrollDragState,
    pub anvil_scroll_drag: crate::ui::scroll::ScrollDragState,
    pub alchemy_station_scroll_drag: crate::ui::scroll::ScrollDragState,
    pub workbench_scroll_drag: crate::ui::scroll::ScrollDragState,
    pub fletching_scroll_drag: crate::ui::scroll::ScrollDragState,
    pub crafting_scroll_drag: crate::ui::scroll::ScrollDragState,
    /// Which settings slider is currently being dragged (if any)
    pub settings_slider_dragging: Option<crate::ui::UiElementId>,
    // Control scheme: false = Modern (WASD+Space+Enter), true = Classic (Arrows+Ctrl+always-on chat)
    pub classic_controls: bool,
    /// Active tab in the prayer/spell panel: 0 = Prayers, 1 = Spells
    pub prayer_spell_tab: usize,
    /// Unified hotkey bar configuration (presets + bindings)
    pub hotkey_bar: HotkeyBarConfig,
    /// Whether the hotkey settings popup is open
    pub hotkey_settings_open: bool,
    /// Whether prayer help overlay is open
    pub prayer_help_open: bool,
    /// Whether spell help overlay is open
    pub spell_help_open: bool,
    /// Whether combat style selector panel is open
    pub combat_style_open: bool,
    /// Graphics quality: true = low (no water shaders), false = high
    pub graphics_low: bool,
    // Slayer panel
    pub slayer_panel_open: bool,
    pub slayer_master_id: Option<String>,
    pub slayer_master_name: Option<String>,
    pub slayer_current_task: Option<crate::game::slayer::SlayerTaskClientData>,
    pub slayer_points: i32,
    pub slayer_tasks_completed: i32,
    pub slayer_rewards: Vec<crate::game::slayer::SlayerRewardClientData>,
    pub slayer_blocked_monsters: Vec<String>,
    pub slayer_unlocked_monsters: Vec<String>,
    pub slayer_blockable_monsters: Vec<(String, String)>,
    pub slayer_selected_block_monster: Option<usize>,
    pub slayer_reward_tab: usize,
    pub slayer_reward_scroll: f32,
    pub slayer_block_scroll_offset: f32,
    pub slayer_block_scroll_drag: crate::ui::scroll::ScrollDragState,

    // ===== Trade System =====
    /// Whether trade window is open
    pub trade_open: bool,
    /// Trade partner's player ID
    pub trade_partner_id: Option<String>,
    /// Trade partner's name
    pub trade_partner_name: Option<String>,
    /// Our trade offer items
    pub trade_my_items: Vec<TradeOfferItem>,
    /// Our gold offer
    pub trade_my_gold: i32,
    /// Whether we accepted
    pub trade_my_accepted: bool,
    /// Partner's offered items
    pub trade_partner_items: Vec<TradeOfferItem>,
    /// Partner's gold offer
    pub trade_partner_gold: i32,
    /// Whether partner accepted
    pub trade_partner_accepted: bool,
    /// Pending trade request (requester_id, requester_name)
    pub trade_pending_request: Option<(String, String)>,

    // ===== Player Stall System =====
    /// Whether the stall setup panel is open (owner)
    pub stall_setup_open: bool,
    /// Our stall slots (when we own a stall)
    pub stall_my_slots: Vec<StallSlotInfo>,
    /// Our stall name
    pub stall_my_name: String,
    /// Whether we're editing the stall name
    pub stall_name_editing: bool,
    /// Cursor position in stall name input
    pub stall_name_cursor: usize,
    /// Whether we have an active stall
    pub stall_active: bool,
    /// Stall browse data (when browsing another player's stall)
    pub stall_browse: Option<StallBrowseInfo>,
    /// Selected stall browse buy quantity
    pub stall_buy_quantity: i32,
    /// Selected stall browse slot index
    pub stall_browse_selected: usize,
    /// Stall price input dialog
    pub stall_price_dialog: Option<StallPriceDialog>,
    /// Last prices entered per item_id (for auto-fill)
    pub stall_last_prices: std::collections::HashMap<String, i32>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            chat_open: false,
            suppress_enter_chat_open: false,
            chat_input: String::new(),
            chat_cursor: 0,
            chat_scroll_offset: 0,
            chat_message_scroll: 0.0,
            chat_scroll_drag: Default::default(),
            chat_key_repeat_time: 0.0,
            chat_key_initial_delay: true,
            paste_requested: false,
            chat_messages: ChatLog::new(),
            chat_revision: 0,
            inventory_open: false,
            active_dialogue: None,
            adventure_board: None,
            adventure_board_selected_offer: 0,
            adventure_board_tab: 0,
            adventure_board_selected_order: 0,
            active_quests: Vec::new(),
            completed_quest_ids: HashSet::new(),
            adventurer_selected_tab: 0,
            adventurer_selected_tier: 0,
            quest_completed_events: Vec::new(),
            quest_log_open: false,
            quest_tracker_minimized: cfg!(target_os = "android"),
            quest_tracker_rect: std::cell::Cell::new(None),
            quest_log_scroll: 0.0,
            quest_catalog: Vec::new(),
            selected_quest_id: None,
            collection_log_open: false,
            collection_log_definitions: Vec::new(),
            collection_log: std::collections::HashMap::new(),
            collection_log_display_names: std::collections::HashMap::new(),
            collection_log_selected_category: None,
            collection_log_selected_subcategory: None,
            collection_log_sidebar_scroll: 0.0,
            collection_log_grid_scroll: 0.0,
            collection_scroll_drag: Default::default(),
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
            furnace_open: false,
            furnace_tile: None,
            furnace_selected_recipe: 0,
            furnace_scroll_offset: 0.0,
            furnace_quantity: 1,
            furnace_tab: 0,
            furnace_station_type: "furnace".to_string(),
            anvil_open: false,
            anvil_tile: None,
            anvil_selected_recipe: 0,
            anvil_scroll_offset: 0.0,
            anvil_quantity: 1,
            anvil_tab: 0,
            alchemy_station_open: false,
            alchemy_station_tile: None,
            alchemy_station_selected_recipe: 0,
            alchemy_station_scroll_offset: 0.0,
            alchemy_station_quantity: 1,
            alchemy_station_tab: 0,
            workbench_open: false,
            workbench_tile: None,
            workbench_selected_recipe: 0,
            workbench_scroll_offset: 0.0,
            workbench_quantity: 1,
            workbench_tab: 0,
            fletching_open: false,
            fletching_selected_recipe: 0,
            fletching_scroll_offset: 0.0,
            fletching_quantity: 1,
            fletching_tab: 0,
            batch_completed: 0,
            batch_total: 0,
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
            bank_open: false,
            bank_slots: Vec::new(),
            bank_gold: 0,
            bank_max_slots: 50,
            bank_scroll: 0.0,
            bank_inv_scroll: 0.0,
            bank_scroll_drag: Default::default(),
            bank_inv_scroll_drag: Default::default(),
            bank_drag: None,
            chest_open: false,
            chest_id: String::new(),
            chest_name: String::new(),
            chest_slots: Vec::new(),
            chest_total_value: 0,
            chest_scroll: 0.0,
            escape_menu_open: false,
            mobile_menu_expanded: cfg!(target_os = "android"),
            audio_volume: 0.5,
            audio_sfx_volume: 0.5,
            music_muted: false,
            sfx_muted: false,
            ui_scale: 1.0,
            ui_scale_pending: None,
            shift_drop_enabled: true,
            social_open: false,
            skills_open: false,
            character_panel_open: false,
            prayer_book_open: false,
            minimap_panel_open: false,
            minimap_panel_zoom: 1.0,
            minimap_panel_center_x: None,
            minimap_panel_center_y: None,
            minimap_panel_dragging: false,
            minimap_panel_drag_last_x: 0.0,
            minimap_panel_drag_last_y: 0.0,
            hovered_element: None,
            context_menu: None,
            gold_drop_dialog: None,
            bank_quantity_dialog: None,
            bank_help_open: false,
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
            chat_log_background: true,
            chat_panel_open: false,
            chat_active_tab: ChatChannel::Local,
            chat_last_seen_local: 0.0,
            chat_last_seen_global: 0.0,
            chat_last_seen_system: 0.0,
            hide_system_in_public: true,
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
            dialogue_scroll_drag: Default::default(),
            selected_inventory_slot: None,
            inventory_scroll_offset: 0.0,
            inventory_touch_scroll_id: None,
            inventory_touch_last_y: 0.0,
            inventory_scroll_drag: Default::default(),
            shop_buy_scroll_drag: Default::default(),
            shop_sell_scroll_drag: Default::default(),
            quest_log_scroll_drag: Default::default(),
            furnace_scroll_drag: Default::default(),
            anvil_scroll_drag: Default::default(),
            alchemy_station_scroll_drag: Default::default(),
            workbench_scroll_drag: Default::default(),
            fletching_scroll_drag: Default::default(),
            crafting_scroll_drag: Default::default(),
            settings_slider_dragging: None,
            classic_controls: false,
            prayer_spell_tab: 1,
            hotkey_bar: HotkeyBarConfig::default(),
            hotkey_settings_open: false,
            prayer_help_open: false,
            spell_help_open: false,
            combat_style_open: false,
            #[cfg(target_os = "android")]
            graphics_low: true,
            #[cfg(not(target_os = "android"))]
            graphics_low: false,
            slayer_panel_open: false,
            slayer_master_id: None,
            slayer_master_name: None,
            slayer_current_task: None,
            slayer_points: 0,
            slayer_tasks_completed: 0,
            slayer_rewards: Vec::new(),
            slayer_blocked_monsters: Vec::new(),
            slayer_unlocked_monsters: Vec::new(),
            slayer_blockable_monsters: Vec::new(),
            slayer_selected_block_monster: None,
            slayer_reward_tab: 0,
            slayer_reward_scroll: 0.0,
            slayer_block_scroll_offset: 0.0,
            slayer_block_scroll_drag: Default::default(),
            // Trade system
            trade_open: false,
            trade_partner_id: None,
            trade_partner_name: None,
            trade_my_items: Vec::new(),
            trade_my_gold: 0,
            trade_my_accepted: false,
            trade_partner_items: Vec::new(),
            trade_partner_gold: 0,
            trade_partner_accepted: false,
            trade_pending_request: None,
            // Stall system
            stall_setup_open: false,
            stall_my_slots: Vec::new(),
            stall_my_name: String::new(),
            stall_name_editing: false,
            stall_name_cursor: 0,
            stall_active: false,
            stall_browse: None,
            stall_buy_quantity: 1,
            stall_browse_selected: 0,
            stall_price_dialog: None,
            stall_last_prices: std::collections::HashMap::new(),
        }
    }
}

impl UiState {
    pub fn close_quest_log(&mut self) {
        self.quest_log_open = false;
        self.quest_log_scroll = 0.0;
        self.selected_quest_id = None;
    }

    pub fn close_collection_log(&mut self) {
        self.collection_log_open = false;
        self.collection_log_selected_category = None;
        self.collection_log_selected_subcategory = None;
        self.collection_log_sidebar_scroll = 0.0;
        self.collection_log_grid_scroll = 0.0;
    }
}

/// Returns sort order for quest status: 0 = in-progress, 1 = not started, 2 = completed
pub fn quest_status_order(quest_id: &str, ui_state: &UiState) -> u8 {
    if ui_state.completed_quest_ids.contains(quest_id) {
        2
    } else if ui_state.active_quests.iter().any(|q| q.id == quest_id) {
        0
    } else {
        1
    }
}
