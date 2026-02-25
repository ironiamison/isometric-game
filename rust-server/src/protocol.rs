use serde::{Deserialize, Serialize};

use crate::game::PlayerUpdate;
use crate::npc::NpcUpdate;

// ============================================================================
// Client -> Server Messages
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "move")]
    Move {
        dx: f32,
        dy: f32,
        #[serde(default)]
        seq: Option<u32>,
    },

    #[serde(rename = "dash")]
    Dash,

    #[serde(rename = "face")]
    Face { direction: u8 },

    #[serde(rename = "chat")]
    Chat { text: String, #[serde(default)] channel: String },

    #[serde(rename = "attack")]
    Attack,

    #[serde(rename = "target")]
    Target { entity_id: String },

    #[serde(rename = "pickup")]
    Pickup { item_id: String },

    #[serde(rename = "useItem")]
    UseItem { slot_index: u8 },

    #[serde(rename = "auth")]
    Auth { username: String, password: String },

    #[serde(rename = "register")]
    Register { username: String, password: String },

    #[serde(rename = "requestChunk")]
    RequestChunk { chunk_x: i32, chunk_y: i32 },

    /// Interact with an NPC (quest giver, merchant, etc.)
    #[serde(rename = "interact")]
    Interact { npc_id: String },

    /// Interact with a world map object (obelisk, etc.)
    #[serde(rename = "interactObject")]
    InteractObject { x: i32, y: i32 },

    /// Direct waystone teleport (no dialogue)
    #[serde(rename = "useWaystone")]
    UseWaystone { x: i32, y: i32 },

    /// Player selected a dialogue choice
    #[serde(rename = "dialogueChoice")]
    DialogueChoiceMsg { quest_id: String, choice_id: String },

    /// Player accepts a quest
    #[serde(rename = "acceptQuest")]
    AcceptQuest { quest_id: String },

    /// Player abandons a quest
    #[serde(rename = "abandonQuest")]
    AbandonQuest { quest_id: String },

    /// Player requests to craft an item (legacy instant craft)
    #[serde(rename = "craft")]
    Craft { recipe_id: String },

    /// Player starts a timed craft
    #[serde(rename = "startCraft")]
    StartCraft { recipe_id: String },

    /// Player cancels an active craft
    #[serde(rename = "cancelCraft")]
    CancelCraft,

    /// Equip item from inventory slot
    #[serde(rename = "equip")]
    Equip { slot_index: u8 },

    /// Unequip item from equipment slot
    #[serde(rename = "unequip")]
    Unequip { slot_type: String },

    /// Drop item from inventory slot (optionally at a target tile)
    #[serde(rename = "dropItem")]
    DropItem {
        slot_index: u8,
        quantity: u32,
        target_x: Option<i32>,
        target_y: Option<i32>,
    },

    /// Drop gold to the ground
    #[serde(rename = "dropGold")]
    DropGold { amount: i32 },

    /// Swap two inventory slots
    #[serde(rename = "swapSlots")]
    SwapSlots { from_slot: u8, to_slot: u8 },

    /// Buy item from shop
    #[serde(rename = "shopBuy")]
    ShopBuy {
        npc_id: String,
        item_id: String,
        quantity: i32,
    },

    /// Sell item to shop
    #[serde(rename = "shopSell")]
    ShopSell {
        npc_id: String,
        item_id: String,
        quantity: i32,
    },

    /// Enter a portal to transition to another map
    #[serde(rename = "enterPortal")]
    EnterPortal { portal_id: String },

    /// Start gathering at a marker tile
    #[serde(rename = "startGathering")]
    StartGathering { marker_x: i32, marker_y: i32 },

    /// Stop gathering
    #[serde(rename = "stopGathering")]
    StopGathering,

    /// Request to sit on a chair
    #[serde(rename = "sitChair")]
    SitChair { tile_x: i32, tile_y: i32 },

    /// Stand up from chair
    #[serde(rename = "standUp")]
    StandUp,

    /// Plant a seed in a farming patch
    #[serde(rename = "plantSeed")]
    PlantSeed { patch_id: String, item_id: String },

    /// Harvest a crop from a farming patch
    #[serde(rename = "harvestCrop")]
    HarvestCrop { patch_id: String },

    // ===== Friend System Messages =====
    /// Send a friend request to a player by name
    #[serde(rename = "sendFriendRequest")]
    SendFriendRequest { target_name: String },

    /// Accept a pending friend request
    #[serde(rename = "acceptFriendRequest")]
    AcceptFriendRequest { requester_id: i64 },

    /// Decline a pending friend request
    #[serde(rename = "declineFriendRequest")]
    DeclineFriendRequest { requester_id: i64 },

    /// Remove a friend from your friends list
    #[serde(rename = "removeFriend")]
    RemoveFriend { friend_id: i64 },

    /// Request list of all online players
    #[serde(rename = "getOnlinePlayers")]
    GetOnlinePlayers,

    // ===== Prayer System Messages =====
    /// Toggle a prayer on/off
    #[serde(rename = "togglePrayer")]
    TogglePrayer { prayer_id: String },

    /// Bury bones from inventory slot
    #[serde(rename = "buryBones")]
    BuryBones { slot: usize },

    /// Use bones at an altar for bonus XP
    #[serde(rename = "offerBones")]
    OfferBones { slot: usize, altar_id: String },

    /// Offer ALL bones of a type at an altar for bonus XP
    #[serde(rename = "offerAllBones")]
    OfferAllBones { item_id: String, altar_id: String },

    /// Pray at an altar to restore prayer points
    #[serde(rename = "prayAtAltar")]
    PrayAtAltar { altar_id: String },

    // ===== Spell System Messages =====
    /// Cast a spell
    #[serde(rename = "castSpell")]
    CastSpell { spell_id: String },

    // ===== Woodcutting System Messages =====
    /// Chop a tree once (player-initiated, one chop per attack)
    #[serde(rename = "chopTree")]
    ChopTree {
        tree_x: i32,
        tree_y: i32,
        tree_gid: u32,
    },

    // ===== Mining System Messages =====
    /// Mine a rock once (player-initiated, one swing per attack)
    #[serde(rename = "mineRock")]
    MineRock {
        rock_x: i32,
        rock_y: i32,
        rock_gid: u32,
    },

    // ===== Bank System Messages =====
    /// Deposit item from inventory into bank
    #[serde(rename = "bankDeposit")]
    BankDeposit { item_id: String, quantity: i32 },

    /// Withdraw item from bank into inventory
    #[serde(rename = "bankWithdraw")]
    BankWithdraw { item_id: String, quantity: i32 },

    /// Deposit gold into bank
    #[serde(rename = "bankDepositGold")]
    BankDepositGold { amount: i32 },

    /// Withdraw gold from bank
    #[serde(rename = "bankWithdrawGold")]
    BankWithdrawGold { amount: i32 },

    /// Deposit all inventory items into bank
    #[serde(rename = "bankDepositAll")]
    BankDepositAll,

    /// Swap (or merge) two bank slots
    #[serde(rename = "bankSwapSlots")]
    BankSwapSlots { slot_a: u32, slot_b: u32 },

    /// Auto-sort entire bank by category then alphabetically
    #[serde(rename = "bankSort")]
    BankSort,

    /// Player starts a batch craft (furnace smelting with quantity)
    #[serde(rename = "startCraftBatch")]
    StartCraftBatch { recipe_id: String, quantity: u32 },

    // ===== Utility Messages =====
    /// Ping for latency measurement - server responds with pong
    #[serde(rename = "ping")]
    Ping { timestamp: f64 },

    // ===== Slayer System Messages =====
    /// Request a new slayer task from a master
    #[serde(rename = "slayerGetTask")]
    SlayerGetTask { master_id: String },

    /// Cancel current slayer task
    #[serde(rename = "slayerCancelTask")]
    SlayerCancelTask,

    /// Buy a slayer reward
    #[serde(rename = "slayerBuyReward")]
    SlayerBuyReward { reward_id: String, #[serde(default)] target_monster_id: Option<String> },

    /// Remove a blocked monster
    #[serde(rename = "slayerRemoveBlock")]
    SlayerRemoveBlock { monster_id: String },

    // ===== Auto-Action System Messages =====
    /// Start repeating an action on a target (attack, mine, chop)
    #[serde(rename = "startAutoAction")]
    StartAutoAction {
        target_type: String,
        target_id: String,
        action: String,
    },

    /// Cancel current auto-action
    #[serde(rename = "cancelAutoAction")]
    CancelAutoAction,

    // ===== Chest System Messages =====
    /// Open a chest at position
    #[serde(rename = "openChest")]
    OpenChest { x: i32, y: i32 },

    /// Take item from a chest slot
    #[serde(rename = "chestTake")]
    ChestTake { chest_id: String, slot: u8 },

    /// Deposit item from inventory into chest
    #[serde(rename = "chestDeposit")]
    ChestDeposit { chest_id: String, inventory_slot: u8 },
}

impl ClientMessage {
    pub fn name(&self) -> &'static str {
        match self {
            ClientMessage::Move { .. } => "Move",
            ClientMessage::Dash => "Dash",
            ClientMessage::Face { .. } => "Face",
            ClientMessage::Chat { .. } => "Chat",
            ClientMessage::Attack => "Attack",
            ClientMessage::Target { .. } => "Target",
            ClientMessage::Pickup { .. } => "Pickup",
            ClientMessage::UseItem { .. } => "UseItem",
            ClientMessage::Auth { .. } => "Auth",
            ClientMessage::Register { .. } => "Register",
            ClientMessage::RequestChunk { .. } => "RequestChunk",
            ClientMessage::Interact { .. } => "Interact",
            ClientMessage::InteractObject { .. } => "InteractObject",
            ClientMessage::UseWaystone { .. } => "UseWaystone",
            ClientMessage::DialogueChoiceMsg { .. } => "DialogueChoice",
            ClientMessage::AcceptQuest { .. } => "AcceptQuest",
            ClientMessage::AbandonQuest { .. } => "AbandonQuest",
            ClientMessage::Craft { .. } => "Craft",
            ClientMessage::StartCraft { .. } => "StartCraft",
            ClientMessage::CancelCraft => "CancelCraft",
            ClientMessage::Equip { .. } => "Equip",
            ClientMessage::Unequip { .. } => "Unequip",
            ClientMessage::DropItem { .. } => "DropItem",
            ClientMessage::DropGold { .. } => "DropGold",
            ClientMessage::SwapSlots { .. } => "SwapSlots",
            ClientMessage::ShopBuy { .. } => "ShopBuy",
            ClientMessage::ShopSell { .. } => "ShopSell",
            ClientMessage::EnterPortal { .. } => "EnterPortal",
            ClientMessage::StartGathering { .. } => "StartGathering",
            ClientMessage::StopGathering => "StopGathering",
            ClientMessage::SitChair { .. } => "SitChair",
            ClientMessage::StandUp => "StandUp",
            ClientMessage::PlantSeed { .. } => "PlantSeed",
            ClientMessage::HarvestCrop { .. } => "HarvestCrop",
            ClientMessage::SendFriendRequest { .. } => "SendFriendRequest",
            ClientMessage::AcceptFriendRequest { .. } => "AcceptFriendRequest",
            ClientMessage::DeclineFriendRequest { .. } => "DeclineFriendRequest",
            ClientMessage::RemoveFriend { .. } => "RemoveFriend",
            ClientMessage::GetOnlinePlayers => "GetOnlinePlayers",
            ClientMessage::TogglePrayer { .. } => "TogglePrayer",
            ClientMessage::BuryBones { .. } => "BuryBones",
            ClientMessage::OfferBones { .. } => "OfferBones",
            ClientMessage::OfferAllBones { .. } => "OfferAllBones",
            ClientMessage::PrayAtAltar { .. } => "PrayAtAltar",
            ClientMessage::CastSpell { .. } => "CastSpell",
            ClientMessage::ChopTree { .. } => "ChopTree",
            ClientMessage::MineRock { .. } => "MineRock",
            ClientMessage::BankDeposit { .. } => "BankDeposit",
            ClientMessage::BankWithdraw { .. } => "BankWithdraw",
            ClientMessage::BankDepositGold { .. } => "BankDepositGold",
            ClientMessage::BankWithdrawGold { .. } => "BankWithdrawGold",
            ClientMessage::BankDepositAll => "BankDepositAll",
            ClientMessage::BankSwapSlots { .. } => "BankSwapSlots",
            ClientMessage::BankSort => "BankSort",
            ClientMessage::StartCraftBatch { .. } => "StartCraftBatch",
            ClientMessage::Ping { .. } => "Ping",
            ClientMessage::SlayerGetTask { .. } => "slayerGetTask",
            ClientMessage::SlayerCancelTask => "slayerCancelTask",
            ClientMessage::SlayerBuyReward { .. } => "slayerBuyReward",
            ClientMessage::SlayerRemoveBlock { .. } => "slayerRemoveBlock",
            ClientMessage::StartAutoAction { .. } => "startAutoAction",
            ClientMessage::CancelAutoAction => "cancelAutoAction",
            ClientMessage::OpenChest { .. } => "OpenChest",
            ClientMessage::ChestTake { .. } => "ChestTake",
            ClientMessage::ChestDeposit { .. } => "ChestDeposit",
        }
    }
}

// ============================================================================
// Chest Data Structs
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct ChestSlotUpdate {
    pub slot: u8,
    pub item_id: String,
    pub quantity: i32,
    pub value: i32,
}

// ============================================================================
// Slayer Data Structs
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct SlayerTaskData {
    pub monster_id: String,
    pub display_name: String,
    pub kills_current: i32,
    pub kills_required: i32,
    pub xp_per_kill: i64,
    pub master_id: String,
    pub points_on_complete: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SlayerRewardData {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub cost: i32,
    pub category: String,
    pub target_id: Option<String>,
    pub quantity: i32,
}

// ============================================================================
// Server -> Client Messages
// ============================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ServerMessage {
    Welcome {
        player_id: String,
        is_new_character: bool,
    },
    PlayerJoined {
        id: String,
        name: String,
        x: i32,
        y: i32,
        gender: String,
        skin: String,
        hair_style: Option<i32>,
        hair_color: Option<i32>,
    },
    PlayerLeft {
        id: String,
    },
    StateSync {
        tick: u64,
        players: Vec<PlayerUpdate>,
        npcs: Vec<NpcUpdate>,
        /// Instance ID this sync belongs to (empty string = overworld).
        /// Clients use this to discard stale syncs from a previous map context.
        instance_id: String,
    },
    ChatMessage {
        #[serde(rename = "senderId")]
        sender_id: String,
        #[serde(rename = "senderName")]
        sender_name: String,
        text: String,
        timestamp: u64,
        channel: String,
    },
    TargetChanged {
        player_id: String,
        target_id: Option<String>,
    },
    PlayerAttack {
        player_id: String,
        attack_type: String, // "melee", "ranged", "spell"
    },
    DamageEvent {
        source_id: String,
        target_id: String,
        damage: i32,
        target_hp: i32,
        target_x: f32,
        target_y: f32,
        projectile: Option<String>,
    },
    AttackResult {
        success: bool,
        reason: Option<String>,
    },
    NpcDied {
        id: String,
        killer_id: String,
    },
    NpcRespawned {
        id: String,
        x: i32,
        y: i32,
    },
    PlayerDied {
        id: String,
        killer_id: String,
    },
    PlayerRespawned {
        id: String,
        x: i32,
        y: i32,
        hp: i32,
    },
    SkillXp {
        player_id: String,
        skill: String,
        xp_gained: i64,
        total_xp: i64,
        level: i32,
    },
    SkillLevelUp {
        player_id: String,
        skill: String,
        new_level: i32,
    },
    /// Sync all skills for a player (sent on login)
    SkillsSync {
        player_id: String,
        hitpoints_level: i32,
        hitpoints_xp: i64,
        combat_level: i32,
        combat_xp: i64,
        fishing_level: i32,
        fishing_xp: i64,
        farming_level: i32,
        farming_xp: i64,
        smithing_level: i32,
        smithing_xp: i64,
        prayer_level: i32,
        prayer_xp: i64,
        magic_level: i32,
        magic_xp: i64,
        woodcutting_level: i32,
        woodcutting_xp: i64,
        alchemy_level: i32,
        alchemy_xp: i64,
        mining_level: i32,
        mining_xp: i64,
        slayer_level: i32,
        slayer_xp: i64,
    },
    ItemDropped {
        id: String,
        item_id: String,
        x: f32,
        y: f32,
        quantity: i32,
    },
    ItemPickedUp {
        item_id: String,
        player_id: String,
    },
    ItemDespawned {
        item_id: String,
    },
    ItemQuantityUpdated {
        id: String,
        quantity: i32,
    },
    InventoryUpdate {
        player_id: String,
        slots: Vec<crate::item::InventorySlotUpdate>,
        gold: i32,
    },
    ItemUsed {
        player_id: String,
        slot: u8,
        item_id: String,
        effect: String, // e.g., "heal:30"
    },
    // Quest-related messages
    QuestAccepted {
        quest_id: String,
        quest_name: String,
        objectives: Vec<QuestObjectiveData>,
    },
    QuestObjectiveProgress {
        quest_id: String,
        objective_id: String,
        current: i32,
        target: i32,
    },
    QuestCompleted {
        quest_id: String,
        quest_name: String,
        rewards_exp: i32,
        rewards_gold: i32,
    },
    QuestStateSync {
        completed_quest_ids: Vec<String>,
    },
    QuestCatalog {
        quests: Vec<QuestCatalogEntryData>,
    },
    ShowDialogue {
        quest_id: String,
        npc_id: String,
        speaker: String,
        text: String,
        choices: Vec<DialogueChoice>,
    },
    Error {
        code: u32,
        message: String,
    },
    ChunkData {
        chunk_x: i32,
        chunk_y: i32,
        layers: Vec<ChunkLayerData>,
        collision: Vec<u8>,            // Packed collision bits
        objects: Vec<ChunkObjectData>, // Map objects (trees, rocks, etc.)
        walls: Vec<ChunkWallData>,     // Edge-aligned walls
        portals: Vec<ChunkPortalData>, // Portals to other maps
    },
    ChunkNotFound {
        chunk_x: i32,
        chunk_y: i32,
    },
    /// Sent on connect: all entity definitions for client-side registry
    EntityDefinitions {
        entities: Vec<ClientEntityDef>,
    },
    /// Sent on connect: all item definitions for client-side registry
    ItemDefinitions {
        items: Vec<ClientItemDef>,
    },
    /// Tell client to close the dialogue UI
    DialogueClosed,
    /// Sent on connect: all recipe definitions for client-side registry
    RecipeDefinitions {
        recipes: Vec<ClientRecipeDef>,
    },
    /// Result of a crafting attempt
    CraftResult {
        success: bool,
        recipe_id: String,
        error: Option<String>,
        items_gained: Vec<RecipeResult>,
    },
    /// Tell client to open the shop/crafting UI for a merchant NPC
    ShopOpen {
        npc_id: String,
    },
    /// Send shop data to client
    ShopData {
        npc_id: String,
        shop: ShopData,
    },
    /// Result of a shop buy/sell action
    ShopResult {
        success: bool,
        action: String,
        item_id: String,
        quantity: i32,
        gold_change: i32,
        error: Option<String>,
    },
    /// Broadcast shop stock update to nearby players
    ShopStockUpdate {
        npc_id: String,
        item_id: String,
        new_quantity: i32,
    },
    /// Broadcast equipment change to all players
    EquipmentUpdate {
        player_id: String,
        equipped_head: Option<String>,
        equipped_body: Option<String>,
        equipped_weapon: Option<String>,
        equipped_back: Option<String>,
        equipped_feet: Option<String>,
        equipped_ring: Option<String>,
        equipped_gloves: Option<String>,
        equipped_necklace: Option<String>,
        equipped_belt: Option<String>,
    },
    /// Result of equip/unequip action sent to the acting player
    EquipResult {
        success: bool,
        slot_type: String,
        item_id: Option<String>,
        error: Option<String>,
    },
    /// Server-wide announcement from admin
    Announcement {
        text: String,
    },
    NpcSpeech {
        npc_id: String,
        message: String,
    },
    // Arena messages
    ArenaStateUpdate {
        state: String,
        countdown_remaining: Option<u32>,
        queued_count: u32,
        fighter_count: u32,
        entry_fee: i32,
    },
    ArenaMatchStart {
        fighter_ids: Vec<String>,
    },
    ArenaPlayerEliminated {
        player_id: String,
        player_name: String,
        killer_id: String,
        killer_name: String,
        remaining: u32,
    },
    ArenaMatchEnd {
        placements: Vec<ArenaPlacementData>,
    },
    ArenaStatsUpdate {
        wins: i32,
        kills: i32,
        deaths: i32,
        current_streak: i32,
        best_streak: i32,
    },
    /// Tell client to transition to a different map (interior or world)
    MapTransition {
        map_type: String, // "interior" or "world"
        map_id: String,   // Interior ID or "world_0"
        spawn_x: f32,
        spawn_y: f32,
        instance_id: String, // Unique instance identifier
    },
    /// Full interior map data sent when entering an interior
    InteriorData {
        map_id: String,
        name: String,
        instance_id: String,
        width: u32,
        height: u32,
        spawn_x: f32,
        spawn_y: f32,
        layers: Vec<ChunkLayerData>,
        collision: Vec<u8>,
        portals: Vec<ChunkPortalData>,
        objects: Vec<ChunkObjectData>,
        walls: Vec<ChunkWallData>,
    },
    // Gathering messages
    GatheringMarkers {
        markers: Vec<GatheringMarkerData>,
    },
    GatheringStarted {
        player_id: String,
        marker_x: i32,
        marker_y: i32,
        zone_id: String,
    },
    GatheringResult {
        player_id: String,
        item_id: String,
        xp_gained: i64,
    },
    GatheringStopped {
        player_id: String,
        reason: String, // "cancelled", "moved", "inventory_full"
    },
    BonusTileSpawned {
        x: i32,
        y: i32,
        zone_id: String,
        telegraph_duration: u64,
    },
    BonusTileClaimed {
        x: i32,
        y: i32,
        player_id: String,
    },
    BonusTileExpired {
        x: i32,
        y: i32,
    },
    BuffApplied {
        player_id: String,
        buff_type: String,
        duration: u64,
    },
    BuffExpired {
        player_id: String,
        buff_type: String,
    },
    ChairPositions {
        positions: Vec<(i32, i32)>,
    },
    ChestPositions {
        positions: Vec<(i32, i32)>,
    },
    SitResult {
        success: bool,
        tile_x: i32,
        tile_y: i32,
        direction: u8,
    },
    /// Send all farming patch states (on connect/area load)
    FarmingPatchStates {
        patches: Vec<FarmingPatchData>,
        unlocked_plots: Vec<u32>,
        tile_overrides: Vec<TileOverride>,
    },
    /// Update a single farming patch state
    PatchStateUpdate {
        patch_id: String,
        state: String,
        crop_id: String,
        growth_stage: u32,
        owner_id: String,
    },

    // ===== Friend System Messages =====
    /// Sent when someone sends you a friend request
    FriendRequestReceived {
        from_id: i64,
        from_name: String,
    },
    /// Sent when your friend request is accepted
    FriendRequestAccepted {
        friend_id: i64,
        friend_name: String,
    },
    /// Sent when your friend request is declined
    FriendRequestDeclined {
        by_id: i64,
    },
    /// Sent when a friend removes you (or you remove them)
    FriendRemoved {
        friend_id: i64,
    },
    /// Full friends list sent on connect
    FriendsList {
        friends: Vec<FriendInfo>,
    },
    /// Pending friend requests sent on connect
    PendingFriendRequests {
        requests: Vec<PendingRequestInfo>,
    },
    /// List of online players (response to GetOnlinePlayers)
    OnlinePlayersList {
        players: Vec<OnlinePlayerInfo>,
    },
    /// Sent when a friend's online status changes
    FriendStatusChanged {
        friend_id: i64,
        online: bool,
    },
    /// Result of a friend action (success/error feedback)
    FriendActionResult {
        action: String,
        success: bool,
        error: Option<String>,
    },

    // ===== Crafting System Messages =====
    /// Sent on connect: player's discovered recipe IDs
    DiscoveredRecipes {
        recipes: Vec<String>,
    },
    /// Sent when a new recipe is discovered
    RecipeDiscovered {
        recipe_id: String,
    },
    /// Timed crafting has started
    CraftingStarted {
        recipe_id: String,
        duration_ms: u64,
    },
    /// Timed crafting was cancelled or interrupted
    CraftingCancelled {
        reason: String,
    },
    /// Timed crafting completed successfully
    CraftingCompleted {
        recipe_id: String,
        items_gained: Vec<(String, u32)>,
        xp_gained: u32,
    },
    /// Batch crafting progress update
    CraftingBatchProgress {
        completed: u32,
        total: u32,
    },

    // ===== Prayer System Messages =====
    /// Update client on prayer state (points and active prayers)
    PrayerStateUpdate {
        points: i32,
        max_points: i32,
        active_prayers: Vec<String>,
    },

    // ===== Spell System Messages =====
    /// Spell visual effect notification
    SpellEffect {
        caster_id: String,
        target_id: Option<String>,
        spell_id: String,
        target_x: i32,
        target_y: i32,
    },
    /// Spell cast result (sent only on failure)
    SpellResult {
        success: bool,
        reason: Option<String>,
    },

    // ===== Woodcutting System Messages =====
    /// Player started chopping a tree
    WoodcuttingStarted {
        player_id: String,
        tree_x: i32,
        tree_y: i32,
        tree_type: String,
    },
    /// Player swung their axe (triggers animation, may or may not get a log)
    WoodcuttingSwing {
        player_id: String,
        tree_x: i32,
        tree_y: i32,
    },
    /// Player chopped a log (successful swing)
    WoodcuttingResult {
        player_id: String,
        item_id: String,
        xp_gained: i64,
    },
    /// Player stopped chopping
    WoodcuttingStopped {
        player_id: String,
        reason: String,
    },
    /// A tree was chopped down (depleted)
    TreeDepleted {
        x: i32,
        y: i32,
        gid: u32,
        respawn_delay_ms: u64,
    },
    /// A tree respawned
    TreeRespawned {
        x: i32,
        y: i32,
        gid: u32,
    },
    /// Sync all depleted trees on chunk load
    DepletedTreesSync {
        trees: Vec<DepletedTreeData>,
    },
    // ===== Mining System Messages =====
    /// Player started mining a rock
    MiningStarted {
        player_id: String,
        rock_x: i32,
        rock_y: i32,
        rock_type: String,
    },
    /// Player swung their pickaxe (triggers animation)
    MiningSwing {
        player_id: String,
        rock_x: i32,
        rock_y: i32,
    },
    /// Player mined ore (successful swing)
    MiningResult {
        player_id: String,
        item_id: String,
        xp_gained: i64,
    },
    /// Player stopped mining
    MiningStopped {
        player_id: String,
        reason: String,
    },
    /// A rock was mined out (depleted)
    RockDepleted {
        x: i32,
        y: i32,
        gid: u32,
        respawn_delay_ms: u64,
    },
    /// A rock respawned
    RockRespawned {
        x: i32,
        y: i32,
        gid: u32,
    },
    /// Sync all depleted rocks on chunk load
    DepletedRocksSync {
        rocks: Vec<DepletedRockData>,
    },
    /// Response to ping for latency measurement
    Pong {
        timestamp: f64,
    },
    // ===== Bank System Messages =====
    /// Full bank state sent when opening bank
    BankOpen {
        slots: Vec<crate::item::InventorySlotUpdate>,
        gold: i32,
        max_slots: u32,
    },
    /// Bank state update after deposit/withdraw
    BankUpdate {
        slots: Vec<crate::item::InventorySlotUpdate>,
        gold: i32,
    },
    /// Result feedback for bank operations
    BankResult {
        success: bool,
        action: String,
        error: Option<String>,
    },

    /// Update client on farming contract state (sent on connect, accept, harvest, claim, abandon)
    FarmingContractUpdate {
        /// None = no active contract, Some = active contract details
        active: bool,
        difficulty: String,
        crop_name: String,
        amount_required: i32,
        amount_harvested: i32,
    },

    // ===== Slayer System Messages =====
    SlayerPanelOpen {
        master_id: String,
        master_name: String,
        current_task: Option<SlayerTaskData>,
        points: i32,
        tasks_completed: i32,
        rewards: Vec<SlayerRewardData>,
        blocked_monsters: Vec<String>,
        unlocked_monsters: Vec<String>,
    },
    SlayerTaskProgress {
        monster_id: String,
        display_name: String,
        kills_current: i32,
        kills_required: i32,
    },
    SlayerTaskComplete {
        monster_id: String,
        display_name: String,
        points_awarded: i32,
        total_points: i32,
    },
    SlayerResult {
        success: bool,
        action: String,
        message: String,
        task: Option<SlayerTaskData>,
        points: Option<i32>,
    },
    SlayerStateSync {
        current_task: Option<SlayerTaskData>,
        points: i32,
        tasks_completed: i32,
        blocked_monsters: Vec<String>,
        unlocked_monsters: Vec<String>,
    },

    // ===== Auto-Action System Messages =====
    /// Confirms auto-action is now active on the server
    AutoActionStarted {
        target_type: String,
        target_id: String,
        action: String,
    },
    /// Auto-action ended (with reason)
    AutoActionStopped {
        reason: String,
    },

    // ===== Scroll Spell System Messages =====
    /// Sent on connect: all scroll spell definitions
    ScrollSpellDefinitions {
        spells: Vec<ScrollSpellDefData>,
    },
    /// Sent on connect: player's unlocked scroll spell IDs
    UnlockedSpellsSync {
        spell_ids: Vec<String>,
    },
    /// Notification when a scroll teaches a spell
    SpellUnlocked {
        spell_id: String,
    },
    /// Pushback effect on a target (from Tornado etc.)
    Pushback {
        target_id: String,
        from_x: i32,
        from_y: i32,
        to_x: i32,
        to_y: i32,
        wall_slam: bool,
        bonus_damage: i32,
    },

    // ===== Chest System Messages =====
    /// Full chest state sent when opening
    ChestOpen {
        chest_id: String,
        name: String,
        slots: Vec<ChestSlotUpdate>,
        total_value: i32,
    },
    /// Chest state update (item taken/deposited/respawned)
    ChestUpdate {
        chest_id: String,
        slots: Vec<ChestSlotUpdate>,
        total_value: i32,
    },
}

/// Scroll spell definition sent to clients
#[derive(Debug, Clone, Serialize)]
pub struct ScrollSpellDefData {
    pub id: String,
    pub name: String,
    pub spell_type: String,
    pub mana_cost: i32,
    pub cooldown_ms: u64,
    pub base_power: i32,
    pub effect_sprite: String,
    pub pushback_distance: i32,
    pub wall_slam_damage_per_tile: i32,
    pub description: String,
}

/// Ground tile override for farming plots (locked vs unlocked appearance)
#[derive(Debug, Clone, Serialize)]
pub struct TileOverride {
    pub x: i32,
    pub y: i32,
    pub tile_id: u32,
}

/// Farming patch data for client synchronization
#[derive(Debug, Clone, Serialize)]
pub struct FarmingPatchData {
    pub patch_id: String,
    pub x: i32,
    pub y: i32,
    pub state: String, // "empty", "growing", "harvestable"
    pub crop_id: String,
    pub growth_stage: u32,
    pub owner_id: String,
}

/// Depleted tree data for client synchronization
#[derive(Debug, Clone, Serialize)]
pub struct DepletedTreeData {
    pub x: i32,
    pub y: i32,
    pub gid: u32,
}

/// Depleted rock data for client synchronization
#[derive(Debug, Clone, Serialize)]
pub struct DepletedRockData {
    pub x: i32,
    pub y: i32,
    pub gid: u32,
}

/// Friend info for friends list
#[derive(Debug, Clone, Serialize)]
pub struct FriendInfo {
    pub id: i64,
    pub name: String,
    pub online: bool,
}

/// Pending friend request info
#[derive(Debug, Clone, Serialize)]
pub struct PendingRequestInfo {
    pub from_id: i64,
    pub from_name: String,
}

/// Online player info for the social panel
#[derive(Debug, Clone, Serialize)]
pub struct OnlinePlayerInfo {
    pub id: i64,
    pub name: String,
    pub is_friend: bool,
}

/// Gathering marker position sent to clients
#[derive(Debug, Clone, Serialize)]
pub struct GatheringMarkerData {
    pub x: i32,
    pub y: i32,
    pub zone_id: String,
    pub skill: String,
}

/// Arena match placement data
#[derive(Debug, Clone, Serialize)]
pub struct ArenaPlacementData {
    pub rank: u32,
    pub player_id: String,
    pub player_name: String,
    pub kills: i32,
    pub gold_reward: i32,
}

/// Layer data for chunk transmission
#[derive(Debug, Clone, Serialize)]
pub struct ChunkLayerData {
    pub layer_type: u8, // 0=Ground, 1=Objects, 2=Overhead
    pub tiles: Vec<u32>,
}

/// Map object data for chunk transmission (trees, rocks, decorations)
#[derive(Debug, Clone, Serialize)]
pub struct ChunkObjectData {
    pub gid: u32,    // Global tile ID from objects.tsx
    pub tile_x: i32, // World tile X coordinate
    pub tile_y: i32, // World tile Y coordinate
    pub width: u32,  // Sprite width in pixels
    pub height: u32, // Sprite height in pixels
}

/// Wall data for chunk transmission
#[derive(Debug, Clone, Serialize)]
pub struct ChunkWallData {
    pub gid: u32,
    pub tile_x: i32,
    pub tile_y: i32,
    pub edge: String, // "down" or "right"
}

/// Portal data for chunk transmission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkPortalData {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub target_map: String,
    pub target_spawn: String,
}

/// Entity definition for client-side registry
#[derive(Debug, Clone, Serialize)]
pub struct ClientEntityDef {
    pub id: String,
    pub display_name: String,
    pub sprite: String,
    pub animation_type: String, // "blob", "humanoid", "quadruped", "flying"
    pub max_hp: i32,
}

/// Item definition for client-side registry
#[derive(Debug, Clone, Serialize)]
pub struct ClientItemDef {
    pub id: String,
    pub display_name: String,
    pub sprite: String,
    pub category: String, // "consumable", "material", "equipment", "quest"
    pub max_stack: i32,
    pub description: String,
    pub base_price: i32,
    pub sellable: bool,
    // Equipment-specific fields (None for non-equipment items)
    pub equipment_slot: Option<String>,
    pub attack_level_required: Option<i32>,
    pub defence_level_required: Option<i32>,
    pub woodcutting_level_required: Option<i32>,
    pub mining_level_required: Option<i32>,
    pub attack_bonus: Option<i32>,
    pub strength_bonus: Option<i32>,
    pub defence_bonus: Option<i32>,
    pub magic_bonus: Option<i32>,
    pub magic_level_required: Option<i32>,
    pub weapon_type: Option<String>,
    pub range: Option<i32>,
    pub chop_speed_multiplier: Option<f32>,
    pub mine_speed_multiplier: Option<f32>,
    pub prayer_xp: i32,
}

/// A dialogue choice for branching dialogue
#[derive(Debug, Clone, Serialize)]
pub struct DialogueChoice {
    pub id: String,
    pub text: String,
}

/// Quest objective data for QuestAccepted message
#[derive(Debug, Clone, Serialize)]
pub struct QuestObjectiveData {
    pub id: String,
    pub description: String,
    pub current: i32,
    pub target: i32,
    pub completed: bool,
}

/// Quest catalog entry for sending all quest info to client
#[derive(Debug, Clone, Serialize)]
pub struct QuestCatalogEntryData {
    pub quest_id: String,
    pub name: String,
    pub description: String,
    pub giver_npc_name: String,
    pub level_required: i32,
    pub required_quest_id: Option<String>,
    pub required_quest_name: Option<String>,
    pub objectives: Vec<QuestObjectiveData>,
}

/// Recipe ingredient for client sync
#[derive(Debug, Clone, Serialize)]
pub struct RecipeIngredient {
    pub item_id: String,
    pub item_name: String,
    pub count: i32,
}

/// Recipe result for client sync
#[derive(Debug, Clone, Serialize)]
pub struct RecipeResult {
    pub item_id: String,
    pub item_name: String,
    pub count: i32,
}

/// Recipe definition for client-side registry
#[derive(Debug, Clone, Serialize)]
pub struct ClientRecipeDef {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub section: Option<String>,
    pub level_required: i32,
    pub ingredients: Vec<RecipeIngredient>,
    pub results: Vec<RecipeResult>,
    pub station: Option<String>,
    pub craft_time_ms: u64,
    pub xp: u32,
    pub requires_discovery: bool,
}

/// Shop data for client synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopData {
    pub shop_id: String,
    pub display_name: String,
    pub buy_multiplier: f32,
    pub sell_multiplier: f32,
    pub crafting_categories: Vec<String>,
    pub crafting_stations: Vec<String>,
    pub stock: Vec<ShopStockItemData>,
}

/// Shop stock item data for client synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopStockItemData {
    pub item_id: String,
    pub quantity: i32,
    pub price: i32,
}

impl ServerMessage {
    pub fn msg_type(&self) -> &'static str {
        match self {
            ServerMessage::Welcome { .. } => "welcome",
            ServerMessage::PlayerJoined { .. } => "playerJoined",
            ServerMessage::PlayerLeft { .. } => "playerLeft",
            ServerMessage::StateSync { .. } => "stateSync",
            ServerMessage::ChatMessage { .. } => "chatMessage",
            ServerMessage::TargetChanged { .. } => "targetChanged",
            ServerMessage::PlayerAttack { .. } => "playerAttack",
            ServerMessage::DamageEvent { .. } => "damageEvent",
            ServerMessage::AttackResult { .. } => "attackResult",
            ServerMessage::NpcDied { .. } => "npcDied",
            ServerMessage::NpcRespawned { .. } => "npcRespawned",
            ServerMessage::PlayerDied { .. } => "playerDied",
            ServerMessage::PlayerRespawned { .. } => "playerRespawned",
            ServerMessage::SkillXp { .. } => "skillXp",
            ServerMessage::SkillLevelUp { .. } => "skillLevelUp",
            ServerMessage::SkillsSync { .. } => "skillsSync",
            ServerMessage::ItemDropped { .. } => "itemDropped",
            ServerMessage::ItemPickedUp { .. } => "itemPickedUp",
            ServerMessage::ItemDespawned { .. } => "itemDespawned",
            ServerMessage::ItemQuantityUpdated { .. } => "itemQuantityUpdated",
            ServerMessage::InventoryUpdate { .. } => "inventoryUpdate",
            ServerMessage::ItemUsed { .. } => "itemUsed",
            ServerMessage::QuestAccepted { .. } => "questAccepted",
            ServerMessage::QuestObjectiveProgress { .. } => "questObjectiveProgress",
            ServerMessage::QuestCompleted { .. } => "questCompleted",
            ServerMessage::QuestStateSync { .. } => "questStateSync",
            ServerMessage::QuestCatalog { .. } => "questCatalog",
            ServerMessage::ShowDialogue { .. } => "showDialogue",
            ServerMessage::Error { .. } => "error",
            ServerMessage::ChunkData { .. } => "chunkData",
            ServerMessage::ChunkNotFound { .. } => "chunkNotFound",
            ServerMessage::EntityDefinitions { .. } => "entityDefinitions",
            ServerMessage::ItemDefinitions { .. } => "itemDefinitions",
            ServerMessage::DialogueClosed => "dialogueClosed",
            ServerMessage::RecipeDefinitions { .. } => "recipeDefinitions",
            ServerMessage::CraftResult { .. } => "craftResult",
            ServerMessage::ShopOpen { .. } => "shopOpen",
            ServerMessage::ShopData { .. } => "shopData",
            ServerMessage::ShopResult { .. } => "shopResult",
            ServerMessage::ShopStockUpdate { .. } => "shopStockUpdate",
            ServerMessage::EquipmentUpdate { .. } => "equipmentUpdate",
            ServerMessage::EquipResult { .. } => "equipResult",
            ServerMessage::ArenaStateUpdate { .. } => "arenaStateUpdate",
            ServerMessage::ArenaMatchStart { .. } => "arenaMatchStart",
            ServerMessage::ArenaPlayerEliminated { .. } => "arenaPlayerEliminated",
            ServerMessage::ArenaMatchEnd { .. } => "arenaMatchEnd",
            ServerMessage::ArenaStatsUpdate { .. } => "arenaStatsUpdate",
            ServerMessage::Announcement { .. } => "announcement",
            ServerMessage::NpcSpeech { .. } => "npcSpeech",
            ServerMessage::MapTransition { .. } => "mapTransition",
            ServerMessage::InteriorData { .. } => "interiorData",
            ServerMessage::GatheringMarkers { .. } => "gatheringMarkers",
            ServerMessage::GatheringStarted { .. } => "gatheringStarted",
            ServerMessage::GatheringResult { .. } => "gatheringResult",
            ServerMessage::GatheringStopped { .. } => "gatheringStopped",
            ServerMessage::BonusTileSpawned { .. } => "bonusTileSpawned",
            ServerMessage::BonusTileClaimed { .. } => "bonusTileClaimed",
            ServerMessage::BonusTileExpired { .. } => "bonusTileExpired",
            ServerMessage::BuffApplied { .. } => "buffApplied",
            ServerMessage::BuffExpired { .. } => "buffExpired",
            ServerMessage::ChairPositions { .. } => "chairPositions",
            ServerMessage::ChestPositions { .. } => "chestPositions",
            ServerMessage::SitResult { .. } => "sitResult",
            ServerMessage::FarmingPatchStates { .. } => "farmingPatchStates",
            ServerMessage::PatchStateUpdate { .. } => "patchStateUpdate",
            // Friend system messages
            ServerMessage::FriendRequestReceived { .. } => "friendRequestReceived",
            ServerMessage::FriendRequestAccepted { .. } => "friendRequestAccepted",
            ServerMessage::FriendRequestDeclined { .. } => "friendRequestDeclined",
            ServerMessage::FriendRemoved { .. } => "friendRemoved",
            ServerMessage::FriendsList { .. } => "friendsList",
            ServerMessage::PendingFriendRequests { .. } => "pendingFriendRequests",
            ServerMessage::OnlinePlayersList { .. } => "onlinePlayersList",
            ServerMessage::FriendStatusChanged { .. } => "friendStatusChanged",
            ServerMessage::FriendActionResult { .. } => "friendActionResult",
            // Crafting system messages
            ServerMessage::DiscoveredRecipes { .. } => "discoveredRecipes",
            ServerMessage::RecipeDiscovered { .. } => "recipeDiscovered",
            ServerMessage::CraftingStarted { .. } => "craftingStarted",
            ServerMessage::CraftingCancelled { .. } => "craftingCancelled",
            ServerMessage::CraftingCompleted { .. } => "craftingCompleted",
            ServerMessage::CraftingBatchProgress { .. } => "craftingBatchProgress",
            // Prayer system messages
            ServerMessage::PrayerStateUpdate { .. } => "prayerStateUpdate",
            // Spell system messages
            ServerMessage::SpellEffect { .. } => "spellEffect",
            ServerMessage::SpellResult { .. } => "spellResult",
            // Woodcutting system messages
            ServerMessage::WoodcuttingStarted { .. } => "woodcuttingStarted",
            ServerMessage::WoodcuttingSwing { .. } => "woodcuttingSwing",
            ServerMessage::WoodcuttingResult { .. } => "woodcuttingResult",
            ServerMessage::WoodcuttingStopped { .. } => "woodcuttingStopped",
            ServerMessage::TreeDepleted { .. } => "treeDepleted",
            ServerMessage::TreeRespawned { .. } => "treeRespawned",
            ServerMessage::DepletedTreesSync { .. } => "depletedTreesSync",
            // Mining system messages
            ServerMessage::MiningStarted { .. } => "miningStarted",
            ServerMessage::MiningSwing { .. } => "miningSwing",
            ServerMessage::MiningResult { .. } => "miningResult",
            ServerMessage::MiningStopped { .. } => "miningStopped",
            ServerMessage::RockDepleted { .. } => "rockDepleted",
            ServerMessage::RockRespawned { .. } => "rockRespawned",
            ServerMessage::DepletedRocksSync { .. } => "depletedRocksSync",
            ServerMessage::Pong { .. } => "pong",
            ServerMessage::BankOpen { .. } => "bankOpen",
            ServerMessage::BankUpdate { .. } => "bankUpdate",
            ServerMessage::BankResult { .. } => "bankResult",
            ServerMessage::FarmingContractUpdate { .. } => "farmingContractUpdate",
            // Slayer system messages
            ServerMessage::SlayerPanelOpen { .. } => "slayerPanelOpen",
            ServerMessage::SlayerTaskProgress { .. } => "slayerTaskProgress",
            ServerMessage::SlayerTaskComplete { .. } => "slayerTaskComplete",
            ServerMessage::SlayerResult { .. } => "slayerResult",
            ServerMessage::SlayerStateSync { .. } => "slayerStateSync",
            // Auto-action system messages
            ServerMessage::AutoActionStarted { .. } => "autoActionStarted",
            ServerMessage::AutoActionStopped { .. } => "autoActionStopped",
            // Scroll spell system messages
            ServerMessage::ScrollSpellDefinitions { .. } => "scrollSpellDefinitions",
            ServerMessage::UnlockedSpellsSync { .. } => "unlockedSpellsSync",
            ServerMessage::SpellUnlocked { .. } => "spellUnlocked",
            ServerMessage::Pushback { .. } => "pushback",
            // Chest system messages
            ServerMessage::ChestOpen { .. } => "chestOpen",
            ServerMessage::ChestUpdate { .. } => "chestUpdate",
        }
    }
}

// ============================================================================
// Encoding/Decoding
// ============================================================================

/// Pre-encode a PlayerUpdate to rmpv::Value for reuse across per-player StateSync messages.
pub fn player_update_to_value(p: &PlayerUpdate) -> rmpv::Value {
    use rmpv::Value;
    let mut pmap = Vec::with_capacity(30);
    pmap.push((
        Value::String("id".into()),
        Value::String(p.id.clone().into()),
    ));
    pmap.push((
        Value::String("name".into()),
        Value::String(p.name.clone().into()),
    ));
    pmap.push((
        Value::String("x".into()),
        Value::Integer((p.x as i64).into()),
    ));
    pmap.push((
        Value::String("y".into()),
        Value::Integer((p.y as i64).into()),
    ));
    pmap.push((
        Value::String("direction".into()),
        Value::Integer((p.direction as i64).into()),
    ));
    pmap.push((
        Value::String("velX".into()),
        Value::Integer((p.vel_x as i64).into()),
    ));
    pmap.push((
        Value::String("velY".into()),
        Value::Integer((p.vel_y as i64).into()),
    ));
    if let Some(seq) = p.move_ack_seq {
        pmap.push((
            Value::String("moveAckSeq".into()),
            Value::Integer((seq as i64).into()),
        ));
    }
    pmap.push((
        Value::String("hp".into()),
        Value::Integer((p.hp as i64).into()),
    ));
    pmap.push((
        Value::String("maxHp".into()),
        Value::Integer((p.max_hp as i64).into()),
    ));
    pmap.push((
        Value::String("combatLevel".into()),
        Value::Integer((p.combat_level as i64).into()),
    ));
    pmap.push((
        Value::String("hitpointsLevel".into()),
        Value::Integer((p.hitpoints_level as i64).into()),
    ));
    pmap.push((
        Value::String("combatSkillLevel".into()),
        Value::Integer((p.combat_skill_level as i64).into()),
    ));
    pmap.push((
        Value::String("gold".into()),
        Value::Integer((p.gold as i64).into()),
    ));
    pmap.push((
        Value::String("gender".into()),
        Value::String(p.gender.clone().into()),
    ));
    pmap.push((
        Value::String("skin".into()),
        Value::String(p.skin.clone().into()),
    ));
    pmap.push((
        Value::String("hair_style".into()),
        match p.hair_style {
            Some(v) => Value::Integer((v as i64).into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("hair_color".into()),
        match p.hair_color {
            Some(v) => Value::Integer((v as i64).into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_head".into()),
        match &p.equipped_head {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_body".into()),
        match &p.equipped_body {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_weapon".into()),
        match &p.equipped_weapon {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_back".into()),
        match &p.equipped_back {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_feet".into()),
        match &p.equipped_feet {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_ring".into()),
        match &p.equipped_ring {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_gloves".into()),
        match &p.equipped_gloves {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_necklace".into()),
        match &p.equipped_necklace {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_belt".into()),
        match &p.equipped_belt {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((Value::String("is_admin".into()), Value::Boolean(p.is_admin)));
    pmap.push((Value::String("sitting".into()), Value::Boolean(p.sitting)));
    pmap.push((
        Value::String("is_gathering".into()),
        Value::Boolean(p.is_gathering),
    ));
    pmap.push((Value::String("dashing".into()), Value::Boolean(p.dashing)));
    pmap.push((
        Value::String("mp".into()),
        Value::Integer((p.mp as i64).into()),
    ));
    pmap.push((
        Value::String("maxMp".into()),
        Value::Integer((p.max_mp as i64).into()),
    ));
    Value::Map(pmap)
}

/// Pre-encode an NpcUpdate to rmpv::Value for reuse across per-player StateSync messages.
pub fn npc_update_to_value(n: &NpcUpdate) -> rmpv::Value {
    use rmpv::Value;
    let mut nmap = Vec::with_capacity(20);
    nmap.push((
        Value::String("id".into()),
        Value::String(n.id.clone().into()),
    ));
    nmap.push((
        Value::String("entity_type".into()),
        Value::String(n.entity_type.clone().into()),
    ));
    nmap.push((
        Value::String("prototype_id".into()),
        Value::String(n.prototype_id.clone().into()),
    ));
    nmap.push((
        Value::String("display_name".into()),
        Value::String(n.display_name.clone().into()),
    ));
    nmap.push((
        Value::String("x".into()),
        Value::Integer((n.x as i64).into()),
    ));
    nmap.push((
        Value::String("y".into()),
        Value::Integer((n.y as i64).into()),
    ));
    nmap.push((
        Value::String("direction".into()),
        Value::Integer((n.direction as i64).into()),
    ));
    nmap.push((
        Value::String("hp".into()),
        Value::Integer((n.hp as i64).into()),
    ));
    nmap.push((
        Value::String("max_hp".into()),
        Value::Integer((n.max_hp as i64).into()),
    ));
    nmap.push((
        Value::String("level".into()),
        Value::Integer((n.level as i64).into()),
    ));
    nmap.push((
        Value::String("state".into()),
        Value::Integer((n.state as i64).into()),
    ));
    nmap.push((Value::String("hostile".into()), Value::Boolean(n.hostile)));
    nmap.push((
        Value::String("is_quest_giver".into()),
        Value::Boolean(n.is_quest_giver),
    ));
    nmap.push((
        Value::String("can_turn_in_quest".into()),
        Value::Boolean(n.can_turn_in_quest),
    ));
    nmap.push((
        Value::String("is_merchant".into()),
        Value::Boolean(n.is_merchant),
    ));
    nmap.push((Value::String("is_altar".into()), Value::Boolean(n.is_altar)));
    nmap.push((Value::String("is_banker".into()), Value::Boolean(n.is_banker)));
    nmap.push((Value::String("is_slayer_master".into()), Value::Boolean(n.is_slayer_master)));
    nmap.push((Value::String("is_friendly".into()), Value::Boolean(n.is_friendly)));
    if let Some(ref st) = n.station_type {
        nmap.push((Value::String("station_type".into()), Value::String(st.clone().into())));
    }
    nmap.push((Value::String("move_speed".into()), Value::F32(n.move_speed)));
    nmap.push((
        Value::String("just_attacked".into()),
        Value::Boolean(n.just_attacked),
    ));
    nmap.push((
        Value::String("no_shadow".into()),
        Value::Boolean(n.no_shadow),
    ));
    nmap.push((
        Value::String("render_offset_y".into()),
        Value::F32(n.render_offset_y),
    ));
    Value::Map(nmap)
}

/// Encode a SlayerTaskData to rmpv::Value (or Nil if None).
fn slayer_task_to_value(task: &Option<SlayerTaskData>) -> rmpv::Value {
    use rmpv::Value;
    match task {
        Some(t) => {
            let mut map = Vec::new();
            map.push((Value::String("monster_id".into()), Value::String(t.monster_id.clone().into())));
            map.push((Value::String("display_name".into()), Value::String(t.display_name.clone().into())));
            map.push((Value::String("kills_current".into()), Value::Integer((t.kills_current as i64).into())));
            map.push((Value::String("kills_required".into()), Value::Integer((t.kills_required as i64).into())));
            map.push((Value::String("xp_per_kill".into()), Value::Integer(t.xp_per_kill.into())));
            map.push((Value::String("master_id".into()), Value::String(t.master_id.clone().into())));
            map.push((Value::String("points_on_complete".into()), Value::Integer((t.points_on_complete as i64).into())));
            Value::Map(map)
        }
        None => Value::Nil,
    }
}

/// Encode a SlayerRewardData to rmpv::Value.
fn slayer_reward_to_value(r: &SlayerRewardData) -> rmpv::Value {
    use rmpv::Value;
    let mut map = Vec::new();
    map.push((Value::String("id".into()), Value::String(r.id.clone().into())));
    map.push((Value::String("display_name".into()), Value::String(r.display_name.clone().into())));
    map.push((Value::String("description".into()), Value::String(r.description.clone().into())));
    map.push((Value::String("cost".into()), Value::Integer((r.cost as i64).into())));
    map.push((Value::String("category".into()), Value::String(r.category.clone().into())));
    map.push((Value::String("target_id".into()), match &r.target_id {
        Some(id) => Value::String(id.clone().into()),
        None => Value::Nil,
    }));
    map.push((Value::String("quantity".into()), Value::Integer((r.quantity as i64).into())));
    Value::Map(map)
}

/// Encode a StateSync message from pre-built rmpv::Values (avoids re-encoding per player).
pub fn encode_state_sync_from_values(
    tick: u64,
    player_values: Vec<rmpv::Value>,
    npc_values: Vec<rmpv::Value>,
    instance_id: &str,
) -> Result<Vec<u8>, String> {
    use rmpv::Value;
    let mut map = Vec::new();
    map.push((Value::String("tick".into()), Value::Integer(tick.into())));
    if !instance_id.is_empty() {
        map.push((
            Value::String("instanceId".into()),
            Value::String(instance_id.into()),
        ));
    }
    map.push((Value::String("players".into()), Value::Array(player_values)));
    map.push((Value::String("npcs".into()), Value::Array(npc_values)));

    let array = Value::Array(vec![
        Value::Integer(13.into()),
        Value::String("stateSync".into()),
        Value::Map(map),
    ]);

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &array)
        .map_err(|e| format!("Failed to encode message: {}", e))?;
    Ok(buf)
}

/// Encode a delta StateSync message with optional removed entity lists.
/// When `full` is true, this is a complete snapshot (same as encode_state_sync_from_values).
/// When `full` is false, only changed entities + removal lists are included.
pub fn encode_delta_state_sync(
    tick: u64,
    player_values: Vec<rmpv::Value>,
    npc_values: Vec<rmpv::Value>,
    instance_id: &str,
    full: bool,
    removed_players: &[String],
    removed_npcs: &[String],
) -> Result<Vec<u8>, String> {
    use rmpv::Value;
    let mut map = Vec::new();
    map.push((Value::String("tick".into()), Value::Integer(tick.into())));
    if !instance_id.is_empty() {
        map.push((
            Value::String("instanceId".into()),
            Value::String(instance_id.into()),
        ));
    }
    map.push((Value::String("players".into()), Value::Array(player_values)));
    map.push((Value::String("npcs".into()), Value::Array(npc_values)));

    if !full {
        map.push((Value::String("full".into()), Value::Boolean(false)));
        if !removed_players.is_empty() {
            map.push((
                Value::String("removedPlayers".into()),
                Value::Array(removed_players.iter().map(|id| Value::String(id.clone().into())).collect()),
            ));
        }
        if !removed_npcs.is_empty() {
            map.push((
                Value::String("removedNpcs".into()),
                Value::Array(removed_npcs.iter().map(|id| Value::String(id.clone().into())).collect()),
            ));
        }
    }

    let array = Value::Array(vec![
        Value::Integer(13.into()),
        Value::String("stateSync".into()),
        Value::Map(map),
    ]);

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &array)
        .map_err(|e| format!("Failed to encode message: {}", e))?;
    Ok(buf)
}

/// Encode a server message to MessagePack format
/// Format: [13, "msg_type", {data}] (matching Colyseus ROOM_DATA protocol)
pub fn encode_server_message(msg: &ServerMessage) -> Result<Vec<u8>, String> {
    use rmpv::Value;

    let msg_type = msg.msg_type();

    // Convert message to rmpv::Value
    let data = match msg {
        ServerMessage::Welcome { player_id, is_new_character } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("is_new_character".into()),
                Value::Boolean(*is_new_character),
            ));
            Value::Map(map)
        }
        ServerMessage::PlayerJoined {
            id,
            name,
            x,
            y,
            gender,
            skin,
            hair_style,
            hair_color,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            map.push((
                Value::String("name".into()),
                Value::String(name.clone().into()),
            ));
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            map.push((
                Value::String("gender".into()),
                Value::String(gender.clone().into()),
            ));
            map.push((
                Value::String("skin".into()),
                Value::String(skin.clone().into()),
            ));
            map.push((
                Value::String("hair_style".into()),
                match hair_style {
                    Some(style) => Value::Integer((*style as i64).into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("hair_color".into()),
                match hair_color {
                    Some(color) => Value::Integer((*color as i64).into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::PlayerLeft { id } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            Value::Map(map)
        }
        ServerMessage::StateSync {
            tick,
            players,
            npcs,
            instance_id,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("tick".into()), Value::Integer((*tick).into())));
            if !instance_id.is_empty() {
                map.push((
                    Value::String("instanceId".into()),
                    Value::String(instance_id.clone().into()),
                ));
            }

            let player_values: Vec<Value> = players
                .iter()
                .map(|p| {
                    let mut pmap = Vec::new();
                    pmap.push((
                        Value::String("id".into()),
                        Value::String(p.id.clone().into()),
                    ));
                    pmap.push((
                        Value::String("name".into()),
                        Value::String(p.name.clone().into()),
                    ));
                    pmap.push((
                        Value::String("x".into()),
                        Value::Integer((p.x as i64).into()),
                    ));
                    pmap.push((
                        Value::String("y".into()),
                        Value::Integer((p.y as i64).into()),
                    ));
                    pmap.push((
                        Value::String("direction".into()),
                        Value::Integer((p.direction as i64).into()),
                    ));
                    // Include velocity for client-side prediction
                    pmap.push((
                        Value::String("velX".into()),
                        Value::Integer((p.vel_x as i64).into()),
                    ));
                    pmap.push((
                        Value::String("velY".into()),
                        Value::Integer((p.vel_y as i64).into()),
                    ));
                    pmap.push((
                        Value::String("hp".into()),
                        Value::Integer((p.hp as i64).into()),
                    ));
                    pmap.push((
                        Value::String("maxHp".into()),
                        Value::Integer((p.max_hp as i64).into()),
                    ));
                    pmap.push((
                        Value::String("combatLevel".into()),
                        Value::Integer((p.combat_level as i64).into()),
                    ));
                    // Individual skill levels
                    pmap.push((
                        Value::String("hitpointsLevel".into()),
                        Value::Integer((p.hitpoints_level as i64).into()),
                    ));
                    pmap.push((
                        Value::String("combatSkillLevel".into()),
                        Value::Integer((p.combat_skill_level as i64).into()),
                    ));
                    pmap.push((
                        Value::String("gold".into()),
                        Value::Integer((p.gold as i64).into()),
                    ));
                    pmap.push((
                        Value::String("gender".into()),
                        Value::String(p.gender.clone().into()),
                    ));
                    pmap.push((
                        Value::String("skin".into()),
                        Value::String(p.skin.clone().into()),
                    ));
                    pmap.push((
                        Value::String("hair_style".into()),
                        match p.hair_style {
                            Some(style) => Value::Integer((style as i64).into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("hair_color".into()),
                        match p.hair_color {
                            Some(color) => Value::Integer((color as i64).into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_head".into()),
                        match &p.equipped_head {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_body".into()),
                        match &p.equipped_body {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_weapon".into()),
                        match &p.equipped_weapon {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_back".into()),
                        match &p.equipped_back {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_feet".into()),
                        match &p.equipped_feet {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_ring".into()),
                        match &p.equipped_ring {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_gloves".into()),
                        match &p.equipped_gloves {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_necklace".into()),
                        match &p.equipped_necklace {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_belt".into()),
                        match &p.equipped_belt {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((Value::String("is_admin".into()), Value::Boolean(p.is_admin)));
                    pmap.push((Value::String("sitting".into()), Value::Boolean(p.sitting)));
                    pmap.push((
                        Value::String("is_gathering".into()),
                        Value::Boolean(p.is_gathering),
                    ));
                    pmap.push((Value::String("dashing".into()), Value::Boolean(p.dashing)));
                    pmap.push((
                        Value::String("mp".into()),
                        Value::Integer((p.mp as i64).into()),
                    ));
                    pmap.push((
                        Value::String("maxMp".into()),
                        Value::Integer((p.max_mp as i64).into()),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((Value::String("players".into()), Value::Array(player_values)));

            let npc_values: Vec<Value> = npcs
                .iter()
                .map(|n| {
                    let mut nmap = Vec::new();
                    nmap.push((
                        Value::String("id".into()),
                        Value::String(n.id.clone().into()),
                    ));
                    nmap.push((
                        Value::String("entity_type".into()),
                        Value::String(n.entity_type.clone().into()),
                    ));
                    nmap.push((
                        Value::String("prototype_id".into()),
                        Value::String(n.prototype_id.clone().into()),
                    ));
                    nmap.push((
                        Value::String("display_name".into()),
                        Value::String(n.display_name.clone().into()),
                    ));
                    nmap.push((
                        Value::String("x".into()),
                        Value::Integer((n.x as i64).into()),
                    ));
                    nmap.push((
                        Value::String("y".into()),
                        Value::Integer((n.y as i64).into()),
                    ));
                    nmap.push((
                        Value::String("direction".into()),
                        Value::Integer((n.direction as i64).into()),
                    ));
                    nmap.push((
                        Value::String("hp".into()),
                        Value::Integer((n.hp as i64).into()),
                    ));
                    nmap.push((
                        Value::String("max_hp".into()),
                        Value::Integer((n.max_hp as i64).into()),
                    ));
                    nmap.push((
                        Value::String("level".into()),
                        Value::Integer((n.level as i64).into()),
                    ));
                    nmap.push((
                        Value::String("state".into()),
                        Value::Integer((n.state as i64).into()),
                    ));
                    nmap.push((Value::String("hostile".into()), Value::Boolean(n.hostile)));
                    nmap.push((
                        Value::String("is_quest_giver".into()),
                        Value::Boolean(n.is_quest_giver),
                    ));
                    nmap.push((
                        Value::String("can_turn_in_quest".into()),
                        Value::Boolean(n.can_turn_in_quest),
                    ));
                    nmap.push((
                        Value::String("is_merchant".into()),
                        Value::Boolean(n.is_merchant),
                    ));
                    nmap.push((Value::String("is_altar".into()), Value::Boolean(n.is_altar)));
                    nmap.push((Value::String("is_banker".into()), Value::Boolean(n.is_banker)));
                    nmap.push((Value::String("is_slayer_master".into()), Value::Boolean(n.is_slayer_master)));
                    nmap.push((Value::String("is_friendly".into()), Value::Boolean(n.is_friendly)));
                    if let Some(ref st) = n.station_type {
                        nmap.push((Value::String("station_type".into()), Value::String(st.clone().into())));
                    }
                    nmap.push((Value::String("move_speed".into()), Value::F32(n.move_speed)));
                    nmap.push((
                        Value::String("just_attacked".into()),
                        Value::Boolean(n.just_attacked),
                    ));
                    Value::Map(nmap)
                })
                .collect();
            map.push((Value::String("npcs".into()), Value::Array(npc_values)));

            Value::Map(map)
        }
        ServerMessage::ChatMessage {
            sender_id,
            sender_name,
            text,
            timestamp,
            channel,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("senderId".into()),
                Value::String(sender_id.clone().into()),
            ));
            map.push((
                Value::String("senderName".into()),
                Value::String(sender_name.clone().into()),
            ));
            map.push((
                Value::String("text".into()),
                Value::String(text.clone().into()),
            ));
            map.push((
                Value::String("timestamp".into()),
                Value::Integer((*timestamp).into()),
            ));
            map.push((
                Value::String("channel".into()),
                Value::String(channel.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::TargetChanged {
            player_id,
            target_id,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("target_id".into()),
                match target_id {
                    Some(id) => Value::String(id.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::PlayerAttack {
            player_id,
            attack_type,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("attack_type".into()),
                Value::String(attack_type.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::DamageEvent {
            source_id,
            target_id,
            damage,
            target_hp,
            target_x,
            target_y,
            projectile,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("source_id".into()),
                Value::String(source_id.clone().into()),
            ));
            map.push((
                Value::String("target_id".into()),
                Value::String(target_id.clone().into()),
            ));
            map.push((
                Value::String("damage".into()),
                Value::Integer((*damage as i64).into()),
            ));
            map.push((
                Value::String("target_hp".into()),
                Value::Integer((*target_hp as i64).into()),
            ));
            map.push((
                Value::String("target_x".into()),
                Value::F64(*target_x as f64),
            ));
            map.push((
                Value::String("target_y".into()),
                Value::F64(*target_y as f64),
            ));
            map.push((
                Value::String("projectile".into()),
                match projectile {
                    Some(p) => Value::String(p.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::AttackResult { success, reason } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("reason".into()),
                match reason {
                    Some(r) => Value::String(r.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::NpcDied { id, killer_id } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            map.push((
                Value::String("killer_id".into()),
                Value::String(killer_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::NpcRespawned { id, x, y } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::PlayerDied { id, killer_id } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            map.push((
                Value::String("killer_id".into()),
                Value::String(killer_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::PlayerRespawned { id, x, y, hp } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            map.push((
                Value::String("hp".into()),
                Value::Integer((*hp as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::SkillXp {
            player_id,
            skill,
            xp_gained,
            total_xp,
            level,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("skill".into()),
                Value::String(skill.clone().into()),
            ));
            map.push((
                Value::String("xp_gained".into()),
                Value::Integer((*xp_gained).into()),
            ));
            map.push((
                Value::String("total_xp".into()),
                Value::Integer((*total_xp).into()),
            ));
            map.push((
                Value::String("level".into()),
                Value::Integer((*level as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::SkillLevelUp {
            player_id,
            skill,
            new_level,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("skill".into()),
                Value::String(skill.clone().into()),
            ));
            map.push((
                Value::String("new_level".into()),
                Value::Integer((*new_level as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::SkillsSync {
            player_id,
            hitpoints_level,
            hitpoints_xp,
            combat_level,
            combat_xp,
            fishing_level,
            fishing_xp,
            farming_level,
            farming_xp,
            smithing_level,
            smithing_xp,
            prayer_level,
            prayer_xp,
            magic_level,
            magic_xp,
            woodcutting_level,
            woodcutting_xp,
            alchemy_level,
            alchemy_xp,
            mining_level,
            mining_xp,
            slayer_level,
            slayer_xp,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("hitpoints_level".into()),
                Value::Integer((*hitpoints_level as i64).into()),
            ));
            map.push((
                Value::String("hitpoints_xp".into()),
                Value::Integer((*hitpoints_xp).into()),
            ));
            map.push((
                Value::String("combat_level".into()),
                Value::Integer((*combat_level as i64).into()),
            ));
            map.push((
                Value::String("combat_xp".into()),
                Value::Integer((*combat_xp).into()),
            ));
            map.push((
                Value::String("fishing_level".into()),
                Value::Integer((*fishing_level as i64).into()),
            ));
            map.push((
                Value::String("fishing_xp".into()),
                Value::Integer((*fishing_xp).into()),
            ));
            map.push((
                Value::String("farming_level".into()),
                Value::Integer((*farming_level as i64).into()),
            ));
            map.push((
                Value::String("farming_xp".into()),
                Value::Integer((*farming_xp).into()),
            ));
            map.push((
                Value::String("smithing_level".into()),
                Value::Integer((*smithing_level as i64).into()),
            ));
            map.push((
                Value::String("smithing_xp".into()),
                Value::Integer((*smithing_xp).into()),
            ));
            map.push((
                Value::String("prayer_level".into()),
                Value::Integer((*prayer_level as i64).into()),
            ));
            map.push((
                Value::String("prayer_xp".into()),
                Value::Integer((*prayer_xp).into()),
            ));
            map.push((
                Value::String("magic_level".into()),
                Value::Integer((*magic_level as i64).into()),
            ));
            map.push((
                Value::String("magic_xp".into()),
                Value::Integer((*magic_xp).into()),
            ));
            map.push((
                Value::String("woodcutting_level".into()),
                Value::Integer((*woodcutting_level as i64).into()),
            ));
            map.push((
                Value::String("woodcutting_xp".into()),
                Value::Integer((*woodcutting_xp).into()),
            ));
            map.push((
                Value::String("alchemy_level".into()),
                Value::Integer((*alchemy_level as i64).into()),
            ));
            map.push((
                Value::String("alchemy_xp".into()),
                Value::Integer((*alchemy_xp).into()),
            ));
            map.push((
                Value::String("mining_level".into()),
                Value::Integer((*mining_level as i64).into()),
            ));
            map.push((
                Value::String("mining_xp".into()),
                Value::Integer((*mining_xp).into()),
            ));
            map.push((
                Value::String("slayer_level".into()),
                Value::Integer((*slayer_level as i64).into()),
            ));
            map.push((
                Value::String("slayer_xp".into()),
                Value::Integer((*slayer_xp).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ItemDropped {
            id,
            item_id,
            x,
            y,
            quantity,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((Value::String("x".into()), Value::F64(*x as f64)));
            map.push((Value::String("y".into()), Value::F64(*y as f64)));
            map.push((
                Value::String("quantity".into()),
                Value::Integer((*quantity as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ItemPickedUp { item_id, player_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ItemDespawned { item_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ItemQuantityUpdated { id, quantity } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            map.push((
                Value::String("quantity".into()),
                Value::Integer((*quantity as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::InventoryUpdate {
            player_id,
            slots,
            gold,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));

            let slot_values: Vec<Value> = slots
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((
                        Value::String("slot".into()),
                        Value::Integer((s.slot as i64).into()),
                    ));
                    smap.push((
                        Value::String("item_id".into()),
                        Value::String(s.item_id.clone().into()),
                    ));
                    smap.push((
                        Value::String("quantity".into()),
                        Value::Integer((s.quantity as i64).into()),
                    ));
                    Value::Map(smap)
                })
                .collect();

            map.push((Value::String("slots".into()), Value::Array(slot_values)));
            map.push((
                Value::String("gold".into()),
                Value::Integer((*gold as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ItemUsed {
            player_id,
            slot,
            item_id,
            effect,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("slot".into()),
                Value::Integer((*slot as i64).into()),
            ));
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("effect".into()),
                Value::String(effect.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::QuestAccepted {
            quest_id,
            quest_name,
            objectives,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("quest_id".into()),
                Value::String(quest_id.clone().into()),
            ));
            map.push((
                Value::String("quest_name".into()),
                Value::String(quest_name.clone().into()),
            ));

            let obj_values: Vec<Value> = objectives
                .iter()
                .map(|obj| {
                    let mut omap = Vec::new();
                    omap.push((
                        Value::String("id".into()),
                        Value::String(obj.id.clone().into()),
                    ));
                    omap.push((
                        Value::String("description".into()),
                        Value::String(obj.description.clone().into()),
                    ));
                    omap.push((
                        Value::String("current".into()),
                        Value::Integer((obj.current as i64).into()),
                    ));
                    omap.push((
                        Value::String("target".into()),
                        Value::Integer((obj.target as i64).into()),
                    ));
                    omap.push((
                        Value::String("completed".into()),
                        Value::Boolean(obj.completed),
                    ));
                    Value::Map(omap)
                })
                .collect();
            map.push((Value::String("objectives".into()), Value::Array(obj_values)));

            Value::Map(map)
        }
        ServerMessage::QuestObjectiveProgress {
            quest_id,
            objective_id,
            current,
            target,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("quest_id".into()),
                Value::String(quest_id.clone().into()),
            ));
            map.push((
                Value::String("objective_id".into()),
                Value::String(objective_id.clone().into()),
            ));
            map.push((
                Value::String("current".into()),
                Value::Integer((*current as i64).into()),
            ));
            map.push((
                Value::String("target".into()),
                Value::Integer((*target as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::QuestCompleted {
            quest_id,
            quest_name,
            rewards_exp,
            rewards_gold,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("quest_id".into()),
                Value::String(quest_id.clone().into()),
            ));
            map.push((
                Value::String("quest_name".into()),
                Value::String(quest_name.clone().into()),
            ));
            map.push((
                Value::String("rewards_exp".into()),
                Value::Integer((*rewards_exp as i64).into()),
            ));
            map.push((
                Value::String("rewards_gold".into()),
                Value::Integer((*rewards_gold as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::QuestStateSync { completed_quest_ids } => {
            let mut map = Vec::new();
            map.push((
                Value::String("completed_quest_ids".into()),
                Value::Array(
                    completed_quest_ids
                        .iter()
                        .map(|id| Value::String(id.clone().into()))
                        .collect(),
                ),
            ));
            Value::Map(map)
        }
        ServerMessage::QuestCatalog { quests } => {
            let mut map = Vec::new();
            let quest_values: Vec<Value> = quests
                .iter()
                .map(|q| {
                    let mut qmap = Vec::new();
                    qmap.push((
                        Value::String("quest_id".into()),
                        Value::String(q.quest_id.clone().into()),
                    ));
                    qmap.push((
                        Value::String("name".into()),
                        Value::String(q.name.clone().into()),
                    ));
                    qmap.push((
                        Value::String("description".into()),
                        Value::String(q.description.clone().into()),
                    ));
                    qmap.push((
                        Value::String("giver_npc_name".into()),
                        Value::String(q.giver_npc_name.clone().into()),
                    ));
                    qmap.push((
                        Value::String("level_required".into()),
                        Value::Integer((q.level_required as i64).into()),
                    ));
                    if let Some(ref req_id) = q.required_quest_id {
                        qmap.push((
                            Value::String("required_quest_id".into()),
                            Value::String(req_id.clone().into()),
                        ));
                    }
                    if let Some(ref req_name) = q.required_quest_name {
                        qmap.push((
                            Value::String("required_quest_name".into()),
                            Value::String(req_name.clone().into()),
                        ));
                    }
                    let obj_values: Vec<Value> = q.objectives.iter().map(|obj| {
                        let mut omap = Vec::new();
                        omap.push((Value::String("id".into()), Value::String(obj.id.clone().into())));
                        omap.push((Value::String("description".into()), Value::String(obj.description.clone().into())));
                        omap.push((Value::String("target".into()), Value::Integer((obj.target as i64).into())));
                        Value::Map(omap)
                    }).collect();
                    qmap.push((Value::String("objectives".into()), Value::Array(obj_values)));
                    Value::Map(qmap)
                })
                .collect();
            map.push((Value::String("quests".into()), Value::Array(quest_values)));
            Value::Map(map)
        }
        ServerMessage::ShowDialogue {
            quest_id,
            npc_id,
            speaker,
            text,
            choices,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("quest_id".into()),
                Value::String(quest_id.clone().into()),
            ));
            map.push((
                Value::String("npc_id".into()),
                Value::String(npc_id.clone().into()),
            ));
            map.push((
                Value::String("speaker".into()),
                Value::String(speaker.clone().into()),
            ));
            map.push((
                Value::String("text".into()),
                Value::String(text.clone().into()),
            ));

            let choice_values: Vec<Value> = choices
                .iter()
                .map(|c| {
                    let mut cmap = Vec::new();
                    cmap.push((
                        Value::String("id".into()),
                        Value::String(c.id.clone().into()),
                    ));
                    cmap.push((
                        Value::String("text".into()),
                        Value::String(c.text.clone().into()),
                    ));
                    Value::Map(cmap)
                })
                .collect();
            map.push((Value::String("choices".into()), Value::Array(choice_values)));

            Value::Map(map)
        }
        ServerMessage::Error { code, message } => {
            let mut map = Vec::new();
            map.push((
                Value::String("code".into()),
                Value::Integer((*code as i64).into()),
            ));
            map.push((
                Value::String("message".into()),
                Value::String(message.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ChunkData {
            chunk_x,
            chunk_y,
            layers,
            collision,
            objects,
            walls,
            portals,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("chunkX".into()),
                Value::Integer((*chunk_x as i64).into()),
            ));
            map.push((
                Value::String("chunkY".into()),
                Value::Integer((*chunk_y as i64).into()),
            ));

            // Encode layers
            let layer_values: Vec<Value> = layers
                .iter()
                .map(|l| {
                    let mut lmap = Vec::new();
                    lmap.push((
                        Value::String("layerType".into()),
                        Value::Integer((l.layer_type as i64).into()),
                    ));
                    let tiles: Vec<Value> = l
                        .tiles
                        .iter()
                        .map(|&t| Value::Integer((t as i64).into()))
                        .collect();
                    lmap.push((Value::String("tiles".into()), Value::Array(tiles)));
                    Value::Map(lmap)
                })
                .collect();
            map.push((Value::String("layers".into()), Value::Array(layer_values)));

            // Encode collision as binary
            let collision_bytes: Vec<Value> = collision
                .iter()
                .map(|&b| Value::Integer((b as i64).into()))
                .collect();
            map.push((
                Value::String("collision".into()),
                Value::Array(collision_bytes),
            ));

            // Encode map objects
            let object_values: Vec<Value> = objects
                .iter()
                .map(|o| {
                    let mut omap = Vec::new();
                    omap.push((
                        Value::String("gid".into()),
                        Value::Integer((o.gid as i64).into()),
                    ));
                    omap.push((
                        Value::String("tileX".into()),
                        Value::Integer((o.tile_x as i64).into()),
                    ));
                    omap.push((
                        Value::String("tileY".into()),
                        Value::Integer((o.tile_y as i64).into()),
                    ));
                    omap.push((
                        Value::String("width".into()),
                        Value::Integer((o.width as i64).into()),
                    ));
                    omap.push((
                        Value::String("height".into()),
                        Value::Integer((o.height as i64).into()),
                    ));
                    Value::Map(omap)
                })
                .collect();
            map.push((Value::String("objects".into()), Value::Array(object_values)));

            // Encode walls
            let wall_values: Vec<Value> = walls
                .iter()
                .map(|w| {
                    let mut wmap = Vec::new();
                    wmap.push((
                        Value::String("gid".into()),
                        Value::Integer((w.gid as i64).into()),
                    ));
                    wmap.push((
                        Value::String("tileX".into()),
                        Value::Integer((w.tile_x as i64).into()),
                    ));
                    wmap.push((
                        Value::String("tileY".into()),
                        Value::Integer((w.tile_y as i64).into()),
                    ));
                    wmap.push((
                        Value::String("edge".into()),
                        Value::String(w.edge.clone().into()),
                    ));
                    Value::Map(wmap)
                })
                .collect();
            map.push((Value::String("walls".into()), Value::Array(wall_values)));

            // Encode portals
            let portal_values: Vec<Value> = portals
                .iter()
                .map(|p| {
                    let mut pmap = Vec::new();
                    pmap.push((
                        Value::String("id".into()),
                        Value::String(p.id.clone().into()),
                    ));
                    pmap.push((
                        Value::String("x".into()),
                        Value::Integer((p.x as i64).into()),
                    ));
                    pmap.push((
                        Value::String("y".into()),
                        Value::Integer((p.y as i64).into()),
                    ));
                    pmap.push((
                        Value::String("width".into()),
                        Value::Integer((p.width as i64).into()),
                    ));
                    pmap.push((
                        Value::String("height".into()),
                        Value::Integer((p.height as i64).into()),
                    ));
                    pmap.push((
                        Value::String("targetMap".into()),
                        Value::String(p.target_map.clone().into()),
                    ));
                    pmap.push((
                        Value::String("targetSpawn".into()),
                        Value::String(p.target_spawn.clone().into()),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((Value::String("portals".into()), Value::Array(portal_values)));

            Value::Map(map)
        }
        ServerMessage::ChunkNotFound { chunk_x, chunk_y } => {
            let mut map = Vec::new();
            map.push((
                Value::String("chunkX".into()),
                Value::Integer((*chunk_x as i64).into()),
            ));
            map.push((
                Value::String("chunkY".into()),
                Value::Integer((*chunk_y as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::EntityDefinitions { entities } => {
            let mut map = Vec::new();
            let entity_values: Vec<Value> = entities
                .iter()
                .map(|e| {
                    let mut emap = Vec::new();
                    emap.push((
                        Value::String("id".into()),
                        Value::String(e.id.clone().into()),
                    ));
                    emap.push((
                        Value::String("displayName".into()),
                        Value::String(e.display_name.clone().into()),
                    ));
                    emap.push((
                        Value::String("sprite".into()),
                        Value::String(e.sprite.clone().into()),
                    ));
                    emap.push((
                        Value::String("animationType".into()),
                        Value::String(e.animation_type.clone().into()),
                    ));
                    emap.push((
                        Value::String("maxHp".into()),
                        Value::Integer((e.max_hp as i64).into()),
                    ));
                    Value::Map(emap)
                })
                .collect();
            map.push((
                Value::String("entities".into()),
                Value::Array(entity_values),
            ));
            Value::Map(map)
        }
        ServerMessage::ItemDefinitions { items } => {
            let mut map = Vec::new();
            let item_values: Vec<Value> = items
                .iter()
                .map(|i| {
                    let mut imap = Vec::new();
                    imap.push((
                        Value::String("id".into()),
                        Value::String(i.id.clone().into()),
                    ));
                    imap.push((
                        Value::String("displayName".into()),
                        Value::String(i.display_name.clone().into()),
                    ));
                    imap.push((
                        Value::String("sprite".into()),
                        Value::String(i.sprite.clone().into()),
                    ));
                    imap.push((
                        Value::String("category".into()),
                        Value::String(i.category.clone().into()),
                    ));
                    imap.push((
                        Value::String("maxStack".into()),
                        Value::Integer((i.max_stack as i64).into()),
                    ));
                    imap.push((
                        Value::String("description".into()),
                        Value::String(i.description.clone().into()),
                    ));
                    imap.push((
                        Value::String("basePrice".into()),
                        Value::Integer((i.base_price as i64).into()),
                    ));
                    imap.push((Value::String("sellable".into()), Value::Boolean(i.sellable)));
                    // Add equipment fields if present
                    if let Some(ref slot) = i.equipment_slot {
                        imap.push((
                            Value::String("equipment_slot".into()),
                            Value::String(slot.clone().into()),
                        ));
                    }
                    if let Some(level) = i.attack_level_required {
                        imap.push((
                            Value::String("attack_level_required".into()),
                            Value::Integer((level as i64).into()),
                        ));
                    }
                    if let Some(level) = i.defence_level_required {
                        imap.push((
                            Value::String("defence_level_required".into()),
                            Value::Integer((level as i64).into()),
                        ));
                    }
                    if let Some(bonus) = i.attack_bonus {
                        imap.push((
                            Value::String("attack_bonus".into()),
                            Value::Integer((bonus as i64).into()),
                        ));
                    }
                    if let Some(bonus) = i.strength_bonus {
                        imap.push((
                            Value::String("strength_bonus".into()),
                            Value::Integer((bonus as i64).into()),
                        ));
                    }
                    if let Some(def) = i.defence_bonus {
                        imap.push((
                            Value::String("defence_bonus".into()),
                            Value::Integer((def as i64).into()),
                        ));
                    }
                    if let Some(bonus) = i.magic_bonus {
                        imap.push((
                            Value::String("magic_bonus".into()),
                            Value::Integer((bonus as i64).into()),
                        ));
                    }
                    if let Some(level) = i.magic_level_required {
                        imap.push((
                            Value::String("magic_level_required".into()),
                            Value::Integer((level as i64).into()),
                        ));
                    }
                    if let Some(ref wtype) = i.weapon_type {
                        imap.push((
                            Value::String("weapon_type".into()),
                            Value::String(wtype.clone().into()),
                        ));
                    }
                    if let Some(r) = i.range {
                        imap.push((
                            Value::String("range".into()),
                            Value::Integer((r as i64).into()),
                        ));
                    }
                    if i.prayer_xp > 0 {
                        imap.push((
                            Value::String("prayer_xp".into()),
                            Value::Integer((i.prayer_xp as i64).into()),
                        ));
                    }
                    // Woodcutting-specific fields
                    if let Some(level) = i.woodcutting_level_required {
                        imap.push((
                            Value::String("woodcutting_level_required".into()),
                            Value::Integer((level as i64).into()),
                        ));
                    }
                    if let Some(speed) = i.chop_speed_multiplier {
                        imap.push((
                            Value::String("chop_speed_multiplier".into()),
                            Value::F32(speed),
                        ));
                    }
                    // Mining-specific fields
                    if let Some(level) = i.mining_level_required {
                        imap.push((
                            Value::String("mining_level_required".into()),
                            Value::Integer((level as i64).into()),
                        ));
                    }
                    if let Some(speed) = i.mine_speed_multiplier {
                        imap.push((
                            Value::String("mine_speed_multiplier".into()),
                            Value::F32(speed),
                        ));
                    }
                    Value::Map(imap)
                })
                .collect();
            map.push((Value::String("items".into()), Value::Array(item_values)));
            Value::Map(map)
        }
        ServerMessage::DialogueClosed => {
            // Empty map - just the message type signals closure
            Value::Map(Vec::new())
        }
        ServerMessage::RecipeDefinitions { recipes } => {
            let mut map = Vec::new();
            let recipe_values: Vec<Value> = recipes
                .iter()
                .map(|r| {
                    let mut rmap = Vec::new();
                    rmap.push((
                        Value::String("id".into()),
                        Value::String(r.id.clone().into()),
                    ));
                    rmap.push((
                        Value::String("display_name".into()),
                        Value::String(r.display_name.clone().into()),
                    ));
                    rmap.push((
                        Value::String("description".into()),
                        Value::String(r.description.clone().into()),
                    ));
                    rmap.push((
                        Value::String("category".into()),
                        Value::String(r.category.clone().into()),
                    ));
                    if let Some(ref s) = r.section {
                        rmap.push((
                            Value::String("section".into()),
                            Value::String(s.clone().into()),
                        ));
                    }
                    rmap.push((
                        Value::String("level_required".into()),
                        Value::Integer((r.level_required as i64).into()),
                    ));

                    let ingredient_values: Vec<Value> = r
                        .ingredients
                        .iter()
                        .map(|i| {
                            let mut imap = Vec::new();
                            imap.push((
                                Value::String("item_id".into()),
                                Value::String(i.item_id.clone().into()),
                            ));
                            imap.push((
                                Value::String("item_name".into()),
                                Value::String(i.item_name.clone().into()),
                            ));
                            imap.push((
                                Value::String("count".into()),
                                Value::Integer((i.count as i64).into()),
                            ));
                            Value::Map(imap)
                        })
                        .collect();
                    rmap.push((
                        Value::String("ingredients".into()),
                        Value::Array(ingredient_values),
                    ));

                    let result_values: Vec<Value> = r
                        .results
                        .iter()
                        .map(|res| {
                            let mut resmap = Vec::new();
                            resmap.push((
                                Value::String("item_id".into()),
                                Value::String(res.item_id.clone().into()),
                            ));
                            resmap.push((
                                Value::String("item_name".into()),
                                Value::String(res.item_name.clone().into()),
                            ));
                            resmap.push((
                                Value::String("count".into()),
                                Value::Integer((res.count as i64).into()),
                            ));
                            Value::Map(resmap)
                        })
                        .collect();
                    rmap.push((Value::String("results".into()), Value::Array(result_values)));

                    // Extended recipe fields
                    match &r.station {
                        Some(s) => rmap.push((
                            Value::String("station".into()),
                            Value::String(s.clone().into()),
                        )),
                        None => rmap.push((Value::String("station".into()), Value::Nil)),
                    }
                    rmap.push((
                        Value::String("craft_time_ms".into()),
                        Value::Integer((r.craft_time_ms as i64).into()),
                    ));
                    rmap.push((
                        Value::String("xp".into()),
                        Value::Integer((r.xp as i64).into()),
                    ));
                    rmap.push((
                        Value::String("requires_discovery".into()),
                        Value::Boolean(r.requires_discovery),
                    ));

                    Value::Map(rmap)
                })
                .collect();
            map.push((Value::String("recipes".into()), Value::Array(recipe_values)));
            Value::Map(map)
        }
        ServerMessage::CraftResult {
            success,
            recipe_id,
            error,
            items_gained,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("recipeId".into()),
                Value::String(recipe_id.clone().into()),
            ));
            map.push((
                Value::String("error".into()),
                match error {
                    Some(e) => Value::String(e.clone().into()),
                    None => Value::Nil,
                },
            ));

            let item_values: Vec<Value> = items_gained
                .iter()
                .map(|item| {
                    let mut imap = Vec::new();
                    imap.push((
                        Value::String("itemId".into()),
                        Value::String(item.item_id.clone().into()),
                    ));
                    imap.push((
                        Value::String("count".into()),
                        Value::Integer((item.count as i64).into()),
                    ));
                    Value::Map(imap)
                })
                .collect();
            map.push((
                Value::String("itemsGained".into()),
                Value::Array(item_values),
            ));

            Value::Map(map)
        }
        ServerMessage::ShopOpen { npc_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("npc_id".into()),
                Value::String(npc_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ShopData { npc_id, shop } => {
            let mut map = Vec::new();
            map.push((
                Value::String("npcId".into()),
                Value::String(npc_id.clone().into()),
            ));

            let mut shop_map = Vec::new();
            shop_map.push((
                Value::String("shopId".into()),
                Value::String(shop.shop_id.clone().into()),
            ));
            shop_map.push((
                Value::String("displayName".into()),
                Value::String(shop.display_name.clone().into()),
            ));
            shop_map.push((
                Value::String("buyMultiplier".into()),
                Value::F64(shop.buy_multiplier as f64),
            ));
            shop_map.push((
                Value::String("sellMultiplier".into()),
                Value::F64(shop.sell_multiplier as f64),
            ));
            let cat_values: Vec<Value> = shop.crafting_categories.iter()
                .map(|c| Value::String(c.clone().into()))
                .collect();
            shop_map.push((
                Value::String("craftingCategories".into()),
                Value::Array(cat_values),
            ));
            let station_values: Vec<Value> = shop.crafting_stations.iter()
                .map(|s| Value::String(s.clone().into()))
                .collect();
            shop_map.push((
                Value::String("craftingStations".into()),
                Value::Array(station_values),
            ));

            let stock_values: Vec<Value> = shop
                .stock
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((
                        Value::String("itemId".into()),
                        Value::String(s.item_id.clone().into()),
                    ));
                    smap.push((
                        Value::String("quantity".into()),
                        Value::Integer((s.quantity as i64).into()),
                    ));
                    smap.push((
                        Value::String("price".into()),
                        Value::Integer((s.price as i64).into()),
                    ));
                    Value::Map(smap)
                })
                .collect();
            shop_map.push((Value::String("stock".into()), Value::Array(stock_values)));

            map.push((Value::String("shop".into()), Value::Map(shop_map)));
            Value::Map(map)
        }
        ServerMessage::ShopResult {
            success,
            action,
            item_id,
            quantity,
            gold_change,
            error,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("action".into()),
                Value::String(action.clone().into()),
            ));
            map.push((
                Value::String("itemId".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("quantity".into()),
                Value::Integer((*quantity as i64).into()),
            ));
            map.push((
                Value::String("goldChange".into()),
                Value::Integer((*gold_change as i64).into()),
            ));
            map.push((
                Value::String("error".into()),
                match error {
                    Some(e) => Value::String(e.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::ShopStockUpdate {
            npc_id,
            item_id,
            new_quantity,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("npcId".into()),
                Value::String(npc_id.clone().into()),
            ));
            map.push((
                Value::String("itemId".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("newQuantity".into()),
                Value::Integer((*new_quantity as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::EquipmentUpdate {
            player_id,
            equipped_head,
            equipped_body,
            equipped_weapon,
            equipped_back,
            equipped_feet,
            equipped_ring,
            equipped_gloves,
            equipped_necklace,
            equipped_belt,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("equipped_head".into()),
                match equipped_head {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_body".into()),
                match equipped_body {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_weapon".into()),
                match equipped_weapon {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_back".into()),
                match equipped_back {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_feet".into()),
                match equipped_feet {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_ring".into()),
                match equipped_ring {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_gloves".into()),
                match equipped_gloves {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_necklace".into()),
                match equipped_necklace {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_belt".into()),
                match equipped_belt {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::EquipResult {
            success,
            slot_type,
            item_id,
            error,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("slot_type".into()),
                Value::String(slot_type.clone().into()),
            ));
            map.push((
                Value::String("item_id".into()),
                match item_id {
                    Some(id) => Value::String(id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("error".into()),
                match error {
                    Some(e) => Value::String(e.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::Announcement { text } => {
            let mut map = Vec::new();
            map.push((
                Value::String("text".into()),
                Value::String(text.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::NpcSpeech { npc_id, message } => {
            let mut map = Vec::new();
            map.push((
                Value::String("npcId".into()),
                Value::String(npc_id.clone().into()),
            ));
            map.push((
                Value::String("message".into()),
                Value::String(message.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::MapTransition {
            map_type,
            map_id,
            spawn_x,
            spawn_y,
            instance_id,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("mapType".into()),
                Value::String(map_type.clone().into()),
            ));
            map.push((
                Value::String("mapId".into()),
                Value::String(map_id.clone().into()),
            ));
            map.push((Value::String("spawnX".into()), Value::F64(*spawn_x as f64)));
            map.push((Value::String("spawnY".into()), Value::F64(*spawn_y as f64)));
            map.push((
                Value::String("instanceId".into()),
                Value::String(instance_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ArenaStateUpdate {
            state,
            countdown_remaining,
            queued_count,
            fighter_count,
            entry_fee,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("state".into()),
                Value::String(state.clone().into()),
            ));
            map.push((
                Value::String("countdownRemaining".into()),
                match countdown_remaining {
                    Some(r) => Value::Integer((*r as i64).into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("queuedCount".into()),
                Value::Integer((*queued_count as i64).into()),
            ));
            map.push((
                Value::String("fighterCount".into()),
                Value::Integer((*fighter_count as i64).into()),
            ));
            map.push((
                Value::String("entryFee".into()),
                Value::Integer((*entry_fee as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ArenaMatchStart { fighter_ids } => {
            let mut map = Vec::new();
            let ids: Vec<Value> = fighter_ids
                .iter()
                .map(|id| Value::String(id.clone().into()))
                .collect();
            map.push((Value::String("fighterIds".into()), Value::Array(ids)));
            Value::Map(map)
        }
        ServerMessage::ArenaPlayerEliminated {
            player_id,
            player_name,
            killer_id,
            killer_name,
            remaining,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("playerId".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("playerName".into()),
                Value::String(player_name.clone().into()),
            ));
            map.push((
                Value::String("killerId".into()),
                Value::String(killer_id.clone().into()),
            ));
            map.push((
                Value::String("killerName".into()),
                Value::String(killer_name.clone().into()),
            ));
            map.push((
                Value::String("remaining".into()),
                Value::Integer((*remaining as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ArenaMatchEnd { placements } => {
            let mut map = Vec::new();
            let placement_values: Vec<Value> = placements
                .iter()
                .map(|p| {
                    let mut pmap = Vec::new();
                    pmap.push((
                        Value::String("rank".into()),
                        Value::Integer((p.rank as i64).into()),
                    ));
                    pmap.push((
                        Value::String("playerId".into()),
                        Value::String(p.player_id.clone().into()),
                    ));
                    pmap.push((
                        Value::String("playerName".into()),
                        Value::String(p.player_name.clone().into()),
                    ));
                    pmap.push((
                        Value::String("kills".into()),
                        Value::Integer((p.kills as i64).into()),
                    ));
                    pmap.push((
                        Value::String("goldReward".into()),
                        Value::Integer((p.gold_reward as i64).into()),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((
                Value::String("placements".into()),
                Value::Array(placement_values),
            ));
            Value::Map(map)
        }
        ServerMessage::ArenaStatsUpdate {
            wins,
            kills,
            deaths,
            current_streak,
            best_streak,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("wins".into()),
                Value::Integer((*wins as i64).into()),
            ));
            map.push((
                Value::String("kills".into()),
                Value::Integer((*kills as i64).into()),
            ));
            map.push((
                Value::String("deaths".into()),
                Value::Integer((*deaths as i64).into()),
            ));
            map.push((
                Value::String("currentStreak".into()),
                Value::Integer((*current_streak as i64).into()),
            ));
            map.push((
                Value::String("bestStreak".into()),
                Value::Integer((*best_streak as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::InteriorData {
            map_id,
            name,
            instance_id,
            width,
            height,
            spawn_x,
            spawn_y,
            layers,
            collision,
            portals,
            objects,
            walls,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("mapId".into()),
                Value::String(map_id.clone().into()),
            ));
            map.push((
                Value::String("name".into()),
                Value::String(name.clone().into()),
            ));
            map.push((
                Value::String("instanceId".into()),
                Value::String(instance_id.clone().into()),
            ));
            map.push((
                Value::String("width".into()),
                Value::Integer((*width as i64).into()),
            ));
            map.push((
                Value::String("height".into()),
                Value::Integer((*height as i64).into()),
            ));
            map.push((Value::String("spawnX".into()), Value::F64(*spawn_x as f64)));
            map.push((Value::String("spawnY".into()), Value::F64(*spawn_y as f64)));

            // Encode layers (same format as ChunkData)
            let layer_values: Vec<Value> = layers
                .iter()
                .map(|l| {
                    let mut lmap = Vec::new();
                    lmap.push((
                        Value::String("layerType".into()),
                        Value::Integer((l.layer_type as i64).into()),
                    ));
                    let tiles: Vec<Value> = l
                        .tiles
                        .iter()
                        .map(|&t| Value::Integer((t as i64).into()))
                        .collect();
                    lmap.push((Value::String("tiles".into()), Value::Array(tiles)));
                    Value::Map(lmap)
                })
                .collect();
            map.push((Value::String("layers".into()), Value::Array(layer_values)));

            // Encode collision as binary array
            let collision_bytes: Vec<Value> = collision
                .iter()
                .map(|&b| Value::Integer((b as i64).into()))
                .collect();
            map.push((
                Value::String("collision".into()),
                Value::Array(collision_bytes),
            ));

            // Encode portals
            let portal_values: Vec<Value> = portals
                .iter()
                .map(|p| {
                    let mut pmap = Vec::new();
                    pmap.push((
                        Value::String("id".into()),
                        Value::String(p.id.clone().into()),
                    ));
                    pmap.push((
                        Value::String("x".into()),
                        Value::Integer((p.x as i64).into()),
                    ));
                    pmap.push((
                        Value::String("y".into()),
                        Value::Integer((p.y as i64).into()),
                    ));
                    pmap.push((
                        Value::String("width".into()),
                        Value::Integer((p.width as i64).into()),
                    ));
                    pmap.push((
                        Value::String("height".into()),
                        Value::Integer((p.height as i64).into()),
                    ));
                    pmap.push((
                        Value::String("targetMap".into()),
                        Value::String(p.target_map.clone().into()),
                    ));
                    pmap.push((
                        Value::String("targetSpawn".into()),
                        Value::String(p.target_spawn.clone().into()),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((Value::String("portals".into()), Value::Array(portal_values)));

            // Encode objects (trees, rocks, decorations)
            let object_values: Vec<Value> = objects
                .iter()
                .map(|o| {
                    let mut omap = Vec::new();
                    omap.push((
                        Value::String("gid".into()),
                        Value::Integer((o.gid as i64).into()),
                    ));
                    omap.push((
                        Value::String("tileX".into()),
                        Value::Integer((o.tile_x as i64).into()),
                    ));
                    omap.push((
                        Value::String("tileY".into()),
                        Value::Integer((o.tile_y as i64).into()),
                    ));
                    omap.push((
                        Value::String("width".into()),
                        Value::Integer((o.width as i64).into()),
                    ));
                    omap.push((
                        Value::String("height".into()),
                        Value::Integer((o.height as i64).into()),
                    ));
                    Value::Map(omap)
                })
                .collect();
            map.push((Value::String("objects".into()), Value::Array(object_values)));

            // Encode walls
            let wall_values: Vec<Value> = walls
                .iter()
                .map(|w| {
                    let mut wmap = Vec::new();
                    wmap.push((
                        Value::String("gid".into()),
                        Value::Integer((w.gid as i64).into()),
                    ));
                    wmap.push((
                        Value::String("tileX".into()),
                        Value::Integer((w.tile_x as i64).into()),
                    ));
                    wmap.push((
                        Value::String("tileY".into()),
                        Value::Integer((w.tile_y as i64).into()),
                    ));
                    wmap.push((
                        Value::String("edge".into()),
                        Value::String(w.edge.clone().into()),
                    ));
                    Value::Map(wmap)
                })
                .collect();
            map.push((Value::String("walls".into()), Value::Array(wall_values)));

            Value::Map(map)
        }
        ServerMessage::GatheringMarkers { markers } => {
            let mut map = Vec::new();
            let marker_values: Vec<Value> = markers
                .iter()
                .map(|m| {
                    let mut mmap = Vec::new();
                    mmap.push((
                        Value::String("x".into()),
                        Value::Integer((m.x as i64).into()),
                    ));
                    mmap.push((
                        Value::String("y".into()),
                        Value::Integer((m.y as i64).into()),
                    ));
                    mmap.push((
                        Value::String("zone_id".into()),
                        Value::String(m.zone_id.clone().into()),
                    ));
                    mmap.push((
                        Value::String("skill".into()),
                        Value::String(m.skill.clone().into()),
                    ));
                    Value::Map(mmap)
                })
                .collect();
            map.push((Value::String("markers".into()), Value::Array(marker_values)));
            Value::Map(map)
        }
        ServerMessage::GatheringStarted {
            player_id,
            marker_x,
            marker_y,
            zone_id,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("marker_x".into()),
                Value::Integer((*marker_x as i64).into()),
            ));
            map.push((
                Value::String("marker_y".into()),
                Value::Integer((*marker_y as i64).into()),
            ));
            map.push((
                Value::String("zone_id".into()),
                Value::String(zone_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::GatheringResult {
            player_id,
            item_id,
            xp_gained,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("xp_gained".into()),
                Value::Integer((*xp_gained).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::GatheringStopped { player_id, reason } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("reason".into()),
                Value::String(reason.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::BonusTileSpawned {
            x,
            y,
            zone_id,
            telegraph_duration,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            map.push((
                Value::String("zone_id".into()),
                Value::String(zone_id.clone().into()),
            ));
            map.push((
                Value::String("telegraph_duration".into()),
                Value::Integer((*telegraph_duration as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::BonusTileClaimed { x, y, player_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::BonusTileExpired { x, y } => {
            let mut map = Vec::new();
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::BuffApplied {
            player_id,
            buff_type,
            duration,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("buff_type".into()),
                Value::String(buff_type.clone().into()),
            ));
            map.push((
                Value::String("duration".into()),
                Value::Integer((*duration as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::BuffExpired {
            player_id,
            buff_type,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("buff_type".into()),
                Value::String(buff_type.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ChairPositions { positions } => {
            let mut map = Vec::new();
            let pos_values: Vec<Value> = positions
                .iter()
                .map(|(x, y)| {
                    let mut pmap = Vec::new();
                    pmap.push((
                        Value::String("x".into()),
                        Value::Integer((*x as i64).into()),
                    ));
                    pmap.push((
                        Value::String("y".into()),
                        Value::Integer((*y as i64).into()),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((Value::String("positions".into()), Value::Array(pos_values)));
            Value::Map(map)
        }
        ServerMessage::ChestPositions { positions } => {
            let mut map = Vec::new();
            let pos_values: Vec<Value> = positions
                .iter()
                .map(|(x, y)| {
                    let mut pmap = Vec::new();
                    pmap.push((
                        Value::String("x".into()),
                        Value::Integer((*x as i64).into()),
                    ));
                    pmap.push((
                        Value::String("y".into()),
                        Value::Integer((*y as i64).into()),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((Value::String("positions".into()), Value::Array(pos_values)));
            Value::Map(map)
        }
        ServerMessage::SitResult {
            success,
            tile_x,
            tile_y,
            direction,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("tileX".into()),
                Value::Integer((*tile_x as i64).into()),
            ));
            map.push((
                Value::String("tileY".into()),
                Value::Integer((*tile_y as i64).into()),
            ));
            map.push((
                Value::String("direction".into()),
                Value::Integer((*direction as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::FarmingPatchStates {
            patches,
            unlocked_plots,
            tile_overrides,
        } => {
            let mut map = Vec::new();
            let patch_values: Vec<Value> = patches
                .iter()
                .map(|p| {
                    let mut pmap = Vec::new();
                    pmap.push((
                        Value::String("patch_id".into()),
                        Value::String(p.patch_id.clone().into()),
                    ));
                    pmap.push((
                        Value::String("x".into()),
                        Value::Integer((p.x as i64).into()),
                    ));
                    pmap.push((
                        Value::String("y".into()),
                        Value::Integer((p.y as i64).into()),
                    ));
                    pmap.push((
                        Value::String("state".into()),
                        Value::String(p.state.clone().into()),
                    ));
                    pmap.push((
                        Value::String("crop_id".into()),
                        Value::String(p.crop_id.clone().into()),
                    ));
                    pmap.push((
                        Value::String("growth_stage".into()),
                        Value::Integer((p.growth_stage as i64).into()),
                    ));
                    pmap.push((
                        Value::String("owner_id".into()),
                        Value::String(p.owner_id.clone().into()),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((Value::String("patches".into()), Value::Array(patch_values)));
            let plot_values: Vec<Value> = unlocked_plots
                .iter()
                .map(|p| Value::Integer((*p as i64).into()))
                .collect();
            map.push((
                Value::String("unlocked_plots".into()),
                Value::Array(plot_values),
            ));
            let tile_override_values: Vec<Value> = tile_overrides
                .iter()
                .map(|t| {
                    let mut tmap = Vec::new();
                    tmap.push((
                        Value::String("x".into()),
                        Value::Integer((t.x as i64).into()),
                    ));
                    tmap.push((
                        Value::String("y".into()),
                        Value::Integer((t.y as i64).into()),
                    ));
                    tmap.push((
                        Value::String("tile_id".into()),
                        Value::Integer((t.tile_id as i64).into()),
                    ));
                    Value::Map(tmap)
                })
                .collect();
            map.push((
                Value::String("tile_overrides".into()),
                Value::Array(tile_override_values),
            ));
            Value::Map(map)
        }
        ServerMessage::PatchStateUpdate {
            patch_id,
            state,
            crop_id,
            growth_stage,
            owner_id,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("patch_id".into()),
                Value::String(patch_id.clone().into()),
            ));
            map.push((
                Value::String("state".into()),
                Value::String(state.clone().into()),
            ));
            map.push((
                Value::String("crop_id".into()),
                Value::String(crop_id.clone().into()),
            ));
            map.push((
                Value::String("growth_stage".into()),
                Value::Integer((*growth_stage as i64).into()),
            ));
            map.push((
                Value::String("owner_id".into()),
                Value::String(owner_id.clone().into()),
            ));
            Value::Map(map)
        }
        // Friend system messages
        ServerMessage::FriendRequestReceived { from_id, from_name } => {
            let mut map = Vec::new();
            map.push((
                Value::String("from_id".into()),
                Value::Integer((*from_id).into()),
            ));
            map.push((
                Value::String("from_name".into()),
                Value::String(from_name.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::FriendRequestAccepted {
            friend_id,
            friend_name,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("friend_id".into()),
                Value::Integer((*friend_id).into()),
            ));
            map.push((
                Value::String("friend_name".into()),
                Value::String(friend_name.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::FriendRequestDeclined { by_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("by_id".into()),
                Value::Integer((*by_id).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::FriendRemoved { friend_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("friend_id".into()),
                Value::Integer((*friend_id).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::FriendsList { friends } => {
            let mut map = Vec::new();
            let friend_values: Vec<Value> = friends
                .iter()
                .map(|f| {
                    let mut fmap = Vec::new();
                    fmap.push((Value::String("id".into()), Value::Integer(f.id.into())));
                    fmap.push((
                        Value::String("name".into()),
                        Value::String(f.name.clone().into()),
                    ));
                    fmap.push((Value::String("online".into()), Value::Boolean(f.online)));
                    Value::Map(fmap)
                })
                .collect();
            map.push((Value::String("friends".into()), Value::Array(friend_values)));
            Value::Map(map)
        }
        ServerMessage::PendingFriendRequests { requests } => {
            let mut map = Vec::new();
            let request_values: Vec<Value> = requests
                .iter()
                .map(|r| {
                    let mut rmap = Vec::new();
                    rmap.push((
                        Value::String("from_id".into()),
                        Value::Integer(r.from_id.into()),
                    ));
                    rmap.push((
                        Value::String("from_name".into()),
                        Value::String(r.from_name.clone().into()),
                    ));
                    Value::Map(rmap)
                })
                .collect();
            map.push((
                Value::String("requests".into()),
                Value::Array(request_values),
            ));
            Value::Map(map)
        }
        ServerMessage::OnlinePlayersList { players } => {
            let mut map = Vec::new();
            let player_values: Vec<Value> = players
                .iter()
                .map(|p| {
                    let mut pmap = Vec::new();
                    pmap.push((Value::String("id".into()), Value::Integer(p.id.into())));
                    pmap.push((
                        Value::String("name".into()),
                        Value::String(p.name.clone().into()),
                    ));
                    pmap.push((
                        Value::String("is_friend".into()),
                        Value::Boolean(p.is_friend),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((Value::String("players".into()), Value::Array(player_values)));
            Value::Map(map)
        }
        ServerMessage::FriendStatusChanged { friend_id, online } => {
            let mut map = Vec::new();
            map.push((
                Value::String("friend_id".into()),
                Value::Integer((*friend_id).into()),
            ));
            map.push((Value::String("online".into()), Value::Boolean(*online)));
            Value::Map(map)
        }
        ServerMessage::FriendActionResult {
            action,
            success,
            error,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("action".into()),
                Value::String(action.clone().into()),
            ));
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            if let Some(err) = error {
                map.push((
                    Value::String("error".into()),
                    Value::String(err.clone().into()),
                ));
            }
            Value::Map(map)
        }
        // Crafting system messages
        ServerMessage::DiscoveredRecipes { recipes } => {
            let mut map = Vec::new();
            let recipe_values: Vec<Value> = recipes
                .iter()
                .map(|r| Value::String(r.clone().into()))
                .collect();
            map.push((Value::String("recipes".into()), Value::Array(recipe_values)));
            Value::Map(map)
        }
        ServerMessage::RecipeDiscovered { recipe_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("recipe_id".into()),
                Value::String(recipe_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::CraftingStarted {
            recipe_id,
            duration_ms,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("recipe_id".into()),
                Value::String(recipe_id.clone().into()),
            ));
            map.push((
                Value::String("duration_ms".into()),
                Value::Integer((*duration_ms as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::CraftingCancelled { reason } => {
            let mut map = Vec::new();
            map.push((
                Value::String("reason".into()),
                Value::String(reason.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::CraftingCompleted {
            recipe_id,
            items_gained,
            xp_gained,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("recipe_id".into()),
                Value::String(recipe_id.clone().into()),
            ));
            let item_values: Vec<Value> = items_gained
                .iter()
                .map(|(item_id, count)| {
                    let mut imap = Vec::new();
                    imap.push((
                        Value::String("item_id".into()),
                        Value::String(item_id.clone().into()),
                    ));
                    imap.push((
                        Value::String("count".into()),
                        Value::Integer((*count as i64).into()),
                    ));
                    Value::Map(imap)
                })
                .collect();
            map.push((
                Value::String("items_gained".into()),
                Value::Array(item_values),
            ));
            map.push((
                Value::String("xp_gained".into()),
                Value::Integer((*xp_gained as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::PrayerStateUpdate {
            points,
            max_points,
            active_prayers,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("points".into()),
                Value::Integer((*points as i64).into()),
            ));
            map.push((
                Value::String("max_points".into()),
                Value::Integer((*max_points as i64).into()),
            ));
            let prayer_values: Vec<Value> = active_prayers
                .iter()
                .map(|p| Value::String(p.clone().into()))
                .collect();
            map.push((
                Value::String("active_prayers".into()),
                Value::Array(prayer_values),
            ));
            Value::Map(map)
        }
        ServerMessage::SpellEffect {
            caster_id,
            target_id,
            spell_id,
            target_x,
            target_y,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("caster_id".into()),
                Value::String(caster_id.clone().into()),
            ));
            match target_id {
                Some(tid) => map.push((
                    Value::String("target_id".into()),
                    Value::String(tid.clone().into()),
                )),
                None => map.push((Value::String("target_id".into()), Value::Nil)),
            }
            map.push((
                Value::String("spell_id".into()),
                Value::String(spell_id.clone().into()),
            ));
            map.push((
                Value::String("target_x".into()),
                Value::Integer((*target_x as i64).into()),
            ));
            map.push((
                Value::String("target_y".into()),
                Value::Integer((*target_y as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::SpellResult { success, reason } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            match reason {
                Some(r) => map.push((
                    Value::String("reason".into()),
                    Value::String(r.clone().into()),
                )),
                None => map.push((Value::String("reason".into()), Value::Nil)),
            }
            Value::Map(map)
        }
        // Woodcutting system messages
        ServerMessage::WoodcuttingStarted {
            player_id,
            tree_x,
            tree_y,
            tree_type,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("tree_x".into()),
                Value::Integer((*tree_x as i64).into()),
            ));
            map.push((
                Value::String("tree_y".into()),
                Value::Integer((*tree_y as i64).into()),
            ));
            map.push((
                Value::String("tree_type".into()),
                Value::String(tree_type.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::WoodcuttingSwing {
            player_id,
            tree_x,
            tree_y,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("tree_x".into()),
                Value::Integer((*tree_x as i64).into()),
            ));
            map.push((
                Value::String("tree_y".into()),
                Value::Integer((*tree_y as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::WoodcuttingResult {
            player_id,
            item_id,
            xp_gained,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("xp_gained".into()),
                Value::Integer((*xp_gained).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::WoodcuttingStopped { player_id, reason } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("reason".into()),
                Value::String(reason.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::TreeDepleted {
            x,
            y,
            gid,
            respawn_delay_ms,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            map.push((
                Value::String("gid".into()),
                Value::Integer((*gid as i64).into()),
            ));
            map.push((
                Value::String("respawn_delay_ms".into()),
                Value::Integer((*respawn_delay_ms as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::TreeRespawned { x, y, gid } => {
            let mut map = Vec::new();
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            map.push((
                Value::String("gid".into()),
                Value::Integer((*gid as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::DepletedTreesSync { trees } => {
            let tree_values: Vec<Value> = trees
                .iter()
                .map(|t| {
                    let mut tree_map = Vec::new();
                    tree_map.push((
                        Value::String("x".into()),
                        Value::Integer((t.x as i64).into()),
                    ));
                    tree_map.push((
                        Value::String("y".into()),
                        Value::Integer((t.y as i64).into()),
                    ));
                    tree_map.push((
                        Value::String("gid".into()),
                        Value::Integer((t.gid as i64).into()),
                    ));
                    Value::Map(tree_map)
                })
                .collect();
            let mut map = Vec::new();
            map.push((Value::String("trees".into()), Value::Array(tree_values)));
            Value::Map(map)
        }
        // Mining system messages
        ServerMessage::MiningStarted {
            player_id,
            rock_x,
            rock_y,
            rock_type,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("rock_x".into()),
                Value::Integer((*rock_x as i64).into()),
            ));
            map.push((
                Value::String("rock_y".into()),
                Value::Integer((*rock_y as i64).into()),
            ));
            map.push((
                Value::String("rock_type".into()),
                Value::String(rock_type.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::MiningSwing {
            player_id,
            rock_x,
            rock_y,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("rock_x".into()),
                Value::Integer((*rock_x as i64).into()),
            ));
            map.push((
                Value::String("rock_y".into()),
                Value::Integer((*rock_y as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::MiningResult {
            player_id,
            item_id,
            xp_gained,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("xp_gained".into()),
                Value::Integer((*xp_gained).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::MiningStopped { player_id, reason } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("reason".into()),
                Value::String(reason.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::RockDepleted {
            x,
            y,
            gid,
            respawn_delay_ms,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            map.push((
                Value::String("gid".into()),
                Value::Integer((*gid as i64).into()),
            ));
            map.push((
                Value::String("respawn_delay_ms".into()),
                Value::Integer((*respawn_delay_ms as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::RockRespawned { x, y, gid } => {
            let mut map = Vec::new();
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            map.push((
                Value::String("gid".into()),
                Value::Integer((*gid as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::DepletedRocksSync { rocks } => {
            let rock_values: Vec<Value> = rocks
                .iter()
                .map(|r| {
                    let mut rock_map = Vec::new();
                    rock_map.push((
                        Value::String("x".into()),
                        Value::Integer((r.x as i64).into()),
                    ));
                    rock_map.push((
                        Value::String("y".into()),
                        Value::Integer((r.y as i64).into()),
                    ));
                    rock_map.push((
                        Value::String("gid".into()),
                        Value::Integer((r.gid as i64).into()),
                    ));
                    Value::Map(rock_map)
                })
                .collect();
            let mut map = Vec::new();
            map.push((Value::String("rocks".into()), Value::Array(rock_values)));
            Value::Map(map)
        }
        ServerMessage::Pong { timestamp } => {
            let mut map = Vec::new();
            map.push((Value::String("timestamp".into()), Value::F64(*timestamp)));
            Value::Map(map)
        }
        ServerMessage::FarmingContractUpdate {
            active,
            difficulty,
            crop_name,
            amount_required,
            amount_harvested,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("active".into()), Value::Boolean(*active)));
            map.push((
                Value::String("difficulty".into()),
                Value::String(difficulty.clone().into()),
            ));
            map.push((
                Value::String("crop_name".into()),
                Value::String(crop_name.clone().into()),
            ));
            map.push((
                Value::String("amount_required".into()),
                Value::Integer((*amount_required as i64).into()),
            ));
            map.push((
                Value::String("amount_harvested".into()),
                Value::Integer((*amount_harvested as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::BankOpen { slots, gold, max_slots } => {
            let mut map = Vec::new();

            let slot_values: Vec<Value> = slots
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((
                        Value::String("slot".into()),
                        Value::Integer((s.slot as i64).into()),
                    ));
                    smap.push((
                        Value::String("item_id".into()),
                        Value::String(s.item_id.clone().into()),
                    ));
                    smap.push((
                        Value::String("quantity".into()),
                        Value::Integer((s.quantity as i64).into()),
                    ));
                    Value::Map(smap)
                })
                .collect();

            map.push((Value::String("slots".into()), Value::Array(slot_values)));
            map.push((
                Value::String("gold".into()),
                Value::Integer((*gold as i64).into()),
            ));
            map.push((
                Value::String("max_slots".into()),
                Value::Integer((*max_slots as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::BankUpdate { slots, gold } => {
            let mut map = Vec::new();

            let slot_values: Vec<Value> = slots
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((
                        Value::String("slot".into()),
                        Value::Integer((s.slot as i64).into()),
                    ));
                    smap.push((
                        Value::String("item_id".into()),
                        Value::String(s.item_id.clone().into()),
                    ));
                    smap.push((
                        Value::String("quantity".into()),
                        Value::Integer((s.quantity as i64).into()),
                    ));
                    Value::Map(smap)
                })
                .collect();

            map.push((Value::String("slots".into()), Value::Array(slot_values)));
            map.push((
                Value::String("gold".into()),
                Value::Integer((*gold as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::BankResult { success, action, error } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("action".into()),
                Value::String(action.clone().into()),
            ));
            map.push((
                Value::String("error".into()),
                match error {
                    Some(e) => Value::String(e.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::CraftingBatchProgress { completed, total } => {
            let mut map = Vec::new();
            map.push((
                Value::String("completed".into()),
                Value::Integer((*completed as i64).into()),
            ));
            map.push((
                Value::String("total".into()),
                Value::Integer((*total as i64).into()),
            ));
            Value::Map(map)
        }
        // ===== Slayer System Messages =====
        ServerMessage::SlayerPanelOpen {
            master_id,
            master_name,
            current_task,
            points,
            tasks_completed,
            rewards,
            blocked_monsters,
            unlocked_monsters,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("master_id".into()), Value::String(master_id.clone().into())));
            map.push((Value::String("master_name".into()), Value::String(master_name.clone().into())));
            map.push((Value::String("current_task".into()), slayer_task_to_value(current_task)));
            map.push((Value::String("points".into()), Value::Integer((*points as i64).into())));
            map.push((Value::String("tasks_completed".into()), Value::Integer((*tasks_completed as i64).into())));
            let reward_values: Vec<Value> = rewards.iter().map(|r| slayer_reward_to_value(r)).collect();
            map.push((Value::String("rewards".into()), Value::Array(reward_values)));
            let blocked: Vec<Value> = blocked_monsters.iter().map(|s| Value::String(s.clone().into())).collect();
            map.push((Value::String("blocked_monsters".into()), Value::Array(blocked)));
            let unlocked: Vec<Value> = unlocked_monsters.iter().map(|s| Value::String(s.clone().into())).collect();
            map.push((Value::String("unlocked_monsters".into()), Value::Array(unlocked)));
            Value::Map(map)
        }
        ServerMessage::SlayerTaskProgress {
            monster_id,
            display_name,
            kills_current,
            kills_required,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("monster_id".into()), Value::String(monster_id.clone().into())));
            map.push((Value::String("display_name".into()), Value::String(display_name.clone().into())));
            map.push((Value::String("kills_current".into()), Value::Integer((*kills_current as i64).into())));
            map.push((Value::String("kills_required".into()), Value::Integer((*kills_required as i64).into())));
            Value::Map(map)
        }
        ServerMessage::SlayerTaskComplete {
            monster_id,
            display_name,
            points_awarded,
            total_points,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("monster_id".into()), Value::String(monster_id.clone().into())));
            map.push((Value::String("display_name".into()), Value::String(display_name.clone().into())));
            map.push((Value::String("points_awarded".into()), Value::Integer((*points_awarded as i64).into())));
            map.push((Value::String("total_points".into()), Value::Integer((*total_points as i64).into())));
            Value::Map(map)
        }
        ServerMessage::SlayerResult {
            success,
            action,
            message,
            task,
            points,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((Value::String("action".into()), Value::String(action.clone().into())));
            map.push((Value::String("message".into()), Value::String(message.clone().into())));
            map.push((Value::String("task".into()), slayer_task_to_value(task)));
            map.push((Value::String("points".into()), match points {
                Some(p) => Value::Integer((*p as i64).into()),
                None => Value::Nil,
            }));
            Value::Map(map)
        }
        ServerMessage::SlayerStateSync {
            current_task,
            points,
            tasks_completed,
            blocked_monsters,
            unlocked_monsters,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("current_task".into()), slayer_task_to_value(current_task)));
            map.push((Value::String("points".into()), Value::Integer((*points as i64).into())));
            map.push((Value::String("tasks_completed".into()), Value::Integer((*tasks_completed as i64).into())));
            let blocked: Vec<Value> = blocked_monsters.iter().map(|s| Value::String(s.clone().into())).collect();
            map.push((Value::String("blocked_monsters".into()), Value::Array(blocked)));
            let unlocked: Vec<Value> = unlocked_monsters.iter().map(|s| Value::String(s.clone().into())).collect();
            map.push((Value::String("unlocked_monsters".into()), Value::Array(unlocked)));
            Value::Map(map)
        }
        ServerMessage::AutoActionStarted {
            target_type,
            target_id,
            action,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("target_type".into()),
                Value::String(target_type.clone().into()),
            ));
            map.push((
                Value::String("target_id".into()),
                Value::String(target_id.clone().into()),
            ));
            map.push((
                Value::String("action".into()),
                Value::String(action.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::AutoActionStopped { reason } => {
            let mut map = Vec::new();
            map.push((
                Value::String("reason".into()),
                Value::String(reason.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ScrollSpellDefinitions { spells } => {
            let mut map = Vec::new();
            let spell_values: Vec<Value> = spells
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((Value::String("id".into()), Value::String(s.id.clone().into())));
                    smap.push((Value::String("name".into()), Value::String(s.name.clone().into())));
                    smap.push((Value::String("spell_type".into()), Value::String(s.spell_type.clone().into())));
                    smap.push((Value::String("mana_cost".into()), Value::Integer((s.mana_cost as i64).into())));
                    smap.push((Value::String("cooldown_ms".into()), Value::Integer((s.cooldown_ms as i64).into())));
                    smap.push((Value::String("base_power".into()), Value::Integer((s.base_power as i64).into())));
                    smap.push((Value::String("effect_sprite".into()), Value::String(s.effect_sprite.clone().into())));
                    smap.push((Value::String("pushback_distance".into()), Value::Integer((s.pushback_distance as i64).into())));
                    smap.push((Value::String("wall_slam_damage_per_tile".into()), Value::Integer((s.wall_slam_damage_per_tile as i64).into())));
                    smap.push((Value::String("description".into()), Value::String(s.description.clone().into())));
                    Value::Map(smap)
                })
                .collect();
            map.push((Value::String("spells".into()), Value::Array(spell_values)));
            Value::Map(map)
        }
        ServerMessage::UnlockedSpellsSync { spell_ids } => {
            let mut map = Vec::new();
            let ids: Vec<Value> = spell_ids
                .iter()
                .map(|s| Value::String(s.clone().into()))
                .collect();
            map.push((Value::String("spell_ids".into()), Value::Array(ids)));
            Value::Map(map)
        }
        ServerMessage::SpellUnlocked { spell_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("spell_id".into()),
                Value::String(spell_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::Pushback {
            target_id,
            from_x,
            from_y,
            to_x,
            to_y,
            wall_slam,
            bonus_damage,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("target_id".into()), Value::String(target_id.clone().into())));
            map.push((Value::String("from_x".into()), Value::Integer((*from_x as i64).into())));
            map.push((Value::String("from_y".into()), Value::Integer((*from_y as i64).into())));
            map.push((Value::String("to_x".into()), Value::Integer((*to_x as i64).into())));
            map.push((Value::String("to_y".into()), Value::Integer((*to_y as i64).into())));
            map.push((Value::String("wall_slam".into()), Value::Boolean(*wall_slam)));
            map.push((Value::String("bonus_damage".into()), Value::Integer((*bonus_damage as i64).into())));
            Value::Map(map)
        }
        ServerMessage::ChestOpen { chest_id, name, slots, total_value } => {
            let mut map = Vec::new();
            map.push((Value::String("chest_id".into()), Value::String(chest_id.clone().into())));
            map.push((Value::String("name".into()), Value::String(name.clone().into())));

            let slot_values: Vec<Value> = slots.iter().map(|s| {
                let mut smap = Vec::new();
                smap.push((Value::String("slot".into()), Value::Integer((s.slot as i64).into())));
                smap.push((Value::String("item_id".into()), Value::String(s.item_id.clone().into())));
                smap.push((Value::String("quantity".into()), Value::Integer((s.quantity as i64).into())));
                smap.push((Value::String("value".into()), Value::Integer((s.value as i64).into())));
                Value::Map(smap)
            }).collect();

            map.push((Value::String("slots".into()), Value::Array(slot_values)));
            map.push((Value::String("total_value".into()), Value::Integer((*total_value as i64).into())));
            Value::Map(map)
        }
        ServerMessage::ChestUpdate { chest_id, slots, total_value } => {
            let mut map = Vec::new();
            map.push((Value::String("chest_id".into()), Value::String(chest_id.clone().into())));

            let slot_values: Vec<Value> = slots.iter().map(|s| {
                let mut smap = Vec::new();
                smap.push((Value::String("slot".into()), Value::Integer((s.slot as i64).into())));
                smap.push((Value::String("item_id".into()), Value::String(s.item_id.clone().into())));
                smap.push((Value::String("quantity".into()), Value::Integer((s.quantity as i64).into())));
                smap.push((Value::String("value".into()), Value::Integer((s.value as i64).into())));
                Value::Map(smap)
            }).collect();

            map.push((Value::String("slots".into()), Value::Array(slot_values)));
            map.push((Value::String("total_value".into()), Value::Integer((*total_value as i64).into())));
            Value::Map(map)
        }
    };

    // Encode as [13, "msg_type", data] - matching Colyseus ROOM_DATA format
    let array = Value::Array(vec![
        Value::Integer(13.into()), // Protocol.RoomData
        Value::String(msg_type.into()),
        data,
    ]);

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &array)
        .map_err(|e| format!("Failed to encode message: {}", e))?;

    Ok(buf)
}

/// Decode a client message from MessagePack format
/// Expected format: [13, "msg_type", {data}]
pub fn decode_client_message(data: &[u8]) -> Result<ClientMessage, String> {
    use rmpv::Value;
    use std::io::Cursor;

    let mut cursor = Cursor::new(data);
    let value = rmpv::decode::read_value(&mut cursor)
        .map_err(|e| format!("Failed to decode MessagePack: {}", e))?;

    let array = value.as_array().ok_or("Expected array")?;

    if array.len() < 2 {
        return Err("Array too short".to_string());
    }

    let protocol = array[0].as_u64().ok_or("Protocol code must be integer")? as u8;

    if protocol != 13 {
        return Err(format!("Unexpected protocol code: {}", protocol));
    }

    let msg_type = array[1].as_str().ok_or("Message type must be string")?;

    let msg_data = if array.len() > 2 {
        &array[2]
    } else {
        &Value::Nil
    };

    match msg_type {
        "move" => {
            let dx = extract_f32(msg_data, "dx").unwrap_or(0.0);
            let dy = extract_f32(msg_data, "dy").unwrap_or(0.0);
            let seq = extract_u32(msg_data, "seq");
            Ok(ClientMessage::Move { dx, dy, seq })
        }
        "dash" => Ok(ClientMessage::Dash),
        "face" => {
            let direction = msg_data
                .as_map()
                .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("direction")))
                .and_then(|(_, v)| v.as_u64().map(|u| u as u8))
                .unwrap_or(0);
            Ok(ClientMessage::Face { direction })
        }
        "chat" => {
            let text = extract_string(msg_data, "text").unwrap_or_default();
            let channel = extract_string(msg_data, "channel").unwrap_or_default();
            Ok(ClientMessage::Chat { text, channel })
        }
        "attack" => Ok(ClientMessage::Attack),
        "target" => {
            let entity_id = extract_string(msg_data, "entity_id").unwrap_or_default();
            Ok(ClientMessage::Target { entity_id })
        }
        "pickup" => {
            let item_id = extract_string(msg_data, "item_id").unwrap_or_default();
            Ok(ClientMessage::Pickup { item_id })
        }
        "useItem" => {
            let slot_index = msg_data
                .as_map()
                .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("slot_index")))
                .and_then(|(_, v)| v.as_u64().map(|u| u as u8))
                .unwrap_or(0);
            Ok(ClientMessage::UseItem { slot_index })
        }
        "auth" => {
            let username = extract_string(msg_data, "username").unwrap_or_default();
            let password = extract_string(msg_data, "password").unwrap_or_default();
            Ok(ClientMessage::Auth { username, password })
        }
        "register" => {
            let username = extract_string(msg_data, "username").unwrap_or_default();
            let password = extract_string(msg_data, "password").unwrap_or_default();
            Ok(ClientMessage::Register { username, password })
        }
        "requestChunk" => {
            let chunk_x = extract_i32(msg_data, "chunkX").unwrap_or(0);
            let chunk_y = extract_i32(msg_data, "chunkY").unwrap_or(0);
            Ok(ClientMessage::RequestChunk { chunk_x, chunk_y })
        }
        "interact" => {
            let npc_id = extract_string(msg_data, "npc_id").unwrap_or_default();
            Ok(ClientMessage::Interact { npc_id })
        }
        "dialogueChoice" => {
            let quest_id = extract_string(msg_data, "quest_id").unwrap_or_default();
            let choice_id = extract_string(msg_data, "choice_id").unwrap_or_default();
            Ok(ClientMessage::DialogueChoiceMsg {
                quest_id,
                choice_id,
            })
        }
        "acceptQuest" => {
            let quest_id = extract_string(msg_data, "quest_id").unwrap_or_default();
            Ok(ClientMessage::AcceptQuest { quest_id })
        }
        "abandonQuest" => {
            let quest_id = extract_string(msg_data, "quest_id").unwrap_or_default();
            Ok(ClientMessage::AbandonQuest { quest_id })
        }
        "craft" => {
            let recipe_id = extract_string(msg_data, "recipe_id").unwrap_or_default();
            Ok(ClientMessage::Craft { recipe_id })
        }
        "startCraft" => {
            let recipe_id = extract_string(msg_data, "recipe_id").unwrap_or_default();
            Ok(ClientMessage::StartCraft { recipe_id })
        }
        "cancelCraft" => Ok(ClientMessage::CancelCraft),
        "equip" => {
            let slot_index = msg_data
                .as_map()
                .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("slot_index")))
                .and_then(|(_, v)| v.as_u64().map(|u| u as u8))
                .unwrap_or(0);
            Ok(ClientMessage::Equip { slot_index })
        }
        "unequip" => {
            let slot_type = extract_string(msg_data, "slot_type").unwrap_or_default();
            Ok(ClientMessage::Unequip { slot_type })
        }
        "dropItem" => {
            let slot_index = msg_data
                .as_map()
                .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("slot_index")))
                .and_then(|(_, v)| v.as_u64().map(|u| u as u8))
                .unwrap_or(0);
            let quantity = msg_data
                .as_map()
                .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("quantity")))
                .and_then(|(_, v)| v.as_u64().map(|u| u as u32))
                .unwrap_or(1);
            let target_x = msg_data
                .as_map()
                .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("target_x")))
                .and_then(|(_, v)| v.as_i64().map(|i| i as i32));
            let target_y = msg_data
                .as_map()
                .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("target_y")))
                .and_then(|(_, v)| v.as_i64().map(|i| i as i32));
            Ok(ClientMessage::DropItem {
                slot_index,
                quantity,
                target_x,
                target_y,
            })
        }
        "dropGold" => {
            let amount = extract_i32(msg_data, "amount").unwrap_or(0);
            Ok(ClientMessage::DropGold { amount })
        }
        "swapSlots" => {
            let from_slot = msg_data
                .as_map()
                .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("from_slot")))
                .and_then(|(_, v)| v.as_u64().map(|u| u as u8))
                .unwrap_or(0);
            let to_slot = msg_data
                .as_map()
                .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("to_slot")))
                .and_then(|(_, v)| v.as_u64().map(|u| u as u8))
                .unwrap_or(0);
            Ok(ClientMessage::SwapSlots { from_slot, to_slot })
        }
        "shopBuy" => {
            let npc_id = extract_string(msg_data, "npcId").unwrap_or_default();
            let item_id = extract_string(msg_data, "itemId").unwrap_or_default();
            let quantity = extract_i32(msg_data, "quantity").unwrap_or(0);
            Ok(ClientMessage::ShopBuy {
                npc_id,
                item_id,
                quantity,
            })
        }
        "shopSell" => {
            let npc_id = extract_string(msg_data, "npcId").unwrap_or_default();
            let item_id = extract_string(msg_data, "itemId").unwrap_or_default();
            let quantity = extract_i32(msg_data, "quantity").unwrap_or(0);
            Ok(ClientMessage::ShopSell {
                npc_id,
                item_id,
                quantity,
            })
        }
        "enterPortal" => {
            let portal_id = extract_string(msg_data, "portalId").unwrap_or_default();
            Ok(ClientMessage::EnterPortal { portal_id })
        }
        "startGathering" => {
            let marker_x = extract_i32(msg_data, "marker_x").unwrap_or(0);
            let marker_y = extract_i32(msg_data, "marker_y").unwrap_or(0);
            Ok(ClientMessage::StartGathering { marker_x, marker_y })
        }
        "stopGathering" => Ok(ClientMessage::StopGathering),
        "sitChair" => {
            let tile_x = extract_i32(msg_data, "tile_x").unwrap_or(0);
            let tile_y = extract_i32(msg_data, "tile_y").unwrap_or(0);
            Ok(ClientMessage::SitChair { tile_x, tile_y })
        }
        "standUp" => Ok(ClientMessage::StandUp),
        "plantSeed" => {
            let patch_id = extract_string(msg_data, "patch_id").unwrap_or_default();
            let item_id = extract_string(msg_data, "item_id").unwrap_or_default();
            Ok(ClientMessage::PlantSeed { patch_id, item_id })
        }
        "harvestCrop" => {
            let patch_id = extract_string(msg_data, "patch_id").unwrap_or_default();
            Ok(ClientMessage::HarvestCrop { patch_id })
        }
        // Friend system messages
        "sendFriendRequest" => {
            let target_name = extract_string(msg_data, "target_name").unwrap_or_default();
            Ok(ClientMessage::SendFriendRequest { target_name })
        }
        "acceptFriendRequest" => {
            let requester_id = extract_i64(msg_data, "requester_id").unwrap_or(0);
            Ok(ClientMessage::AcceptFriendRequest { requester_id })
        }
        "declineFriendRequest" => {
            let requester_id = extract_i64(msg_data, "requester_id").unwrap_or(0);
            Ok(ClientMessage::DeclineFriendRequest { requester_id })
        }
        "removeFriend" => {
            let friend_id = extract_i64(msg_data, "friend_id").unwrap_or(0);
            Ok(ClientMessage::RemoveFriend { friend_id })
        }
        "getOnlinePlayers" => Ok(ClientMessage::GetOnlinePlayers),
        // Prayer system messages
        "togglePrayer" => {
            let prayer_id = extract_string(msg_data, "prayer_id").unwrap_or_default();
            Ok(ClientMessage::TogglePrayer { prayer_id })
        }
        "buryBones" => {
            let slot = extract_i64(msg_data, "slot").unwrap_or(0) as usize;
            Ok(ClientMessage::BuryBones { slot })
        }
        "offerBones" => {
            let slot = extract_i64(msg_data, "slot").unwrap_or(0) as usize;
            let altar_id = extract_string(msg_data, "altar_id").unwrap_or_default();
            Ok(ClientMessage::OfferBones { slot, altar_id })
        }
        "offerAllBones" => {
            let item_id = extract_string(msg_data, "item_id").unwrap_or_default();
            let altar_id = extract_string(msg_data, "altar_id").unwrap_or_default();
            Ok(ClientMessage::OfferAllBones { item_id, altar_id })
        }
        "prayAtAltar" => {
            let altar_id = extract_string(msg_data, "altar_id").unwrap_or_default();
            Ok(ClientMessage::PrayAtAltar { altar_id })
        }
        // Spell system messages
        "castSpell" => {
            let spell_id = extract_string(msg_data, "spell_id").unwrap_or_default();
            Ok(ClientMessage::CastSpell { spell_id })
        }
        // Woodcutting messages
        "chopTree" => {
            let tree_x = extract_i64(msg_data, "tree_x").unwrap_or(0) as i32;
            let tree_y = extract_i64(msg_data, "tree_y").unwrap_or(0) as i32;
            let tree_gid = extract_i64(msg_data, "tree_gid").unwrap_or(0) as u32;
            Ok(ClientMessage::ChopTree {
                tree_x,
                tree_y,
                tree_gid,
            })
        }
        // Mining messages
        "mineRock" => {
            let rock_x = extract_i64(msg_data, "rock_x").unwrap_or(0) as i32;
            let rock_y = extract_i64(msg_data, "rock_y").unwrap_or(0) as i32;
            let rock_gid = extract_i64(msg_data, "rock_gid").unwrap_or(0) as u32;
            Ok(ClientMessage::MineRock {
                rock_x,
                rock_y,
                rock_gid,
            })
        }
        // Utility messages
        "ping" => {
            let timestamp = extract_f64(msg_data, "timestamp").unwrap_or(0.0);
            Ok(ClientMessage::Ping { timestamp })
        }
        // Bank messages
        "bankDeposit" => {
            let item_id = extract_string(msg_data, "item_id").unwrap_or_default();
            let quantity = extract_i32(msg_data, "quantity").unwrap_or(1);
            Ok(ClientMessage::BankDeposit { item_id, quantity })
        }
        "bankWithdraw" => {
            let item_id = extract_string(msg_data, "item_id").unwrap_or_default();
            let quantity = extract_i32(msg_data, "quantity").unwrap_or(1);
            Ok(ClientMessage::BankWithdraw { item_id, quantity })
        }
        "bankDepositGold" => {
            let amount = extract_i32(msg_data, "amount").unwrap_or(0);
            Ok(ClientMessage::BankDepositGold { amount })
        }
        "bankWithdrawGold" => {
            let amount = extract_i32(msg_data, "amount").unwrap_or(0);
            Ok(ClientMessage::BankWithdrawGold { amount })
        }
        "bankDepositAll" => Ok(ClientMessage::BankDepositAll),
        "bankSwapSlots" => {
            let slot_a = extract_u32(msg_data, "slot_a").unwrap_or(0);
            let slot_b = extract_u32(msg_data, "slot_b").unwrap_or(0);
            Ok(ClientMessage::BankSwapSlots { slot_a, slot_b })
        }
        "bankSort" => Ok(ClientMessage::BankSort),
        "startCraftBatch" => {
            let recipe_id = extract_string(msg_data, "recipe_id").unwrap_or_default();
            let quantity = extract_u32(msg_data, "quantity").unwrap_or(1);
            Ok(ClientMessage::StartCraftBatch { recipe_id, quantity })
        }
        "slayerGetTask" => {
            let master_id = extract_string(msg_data, "master_id").unwrap_or_default();
            Ok(ClientMessage::SlayerGetTask { master_id })
        }
        "slayerCancelTask" => Ok(ClientMessage::SlayerCancelTask),
        "slayerBuyReward" => {
            let reward_id = extract_string(msg_data, "reward_id").unwrap_or_default();
            let target_monster_id = extract_string(msg_data, "target_monster_id");
            Ok(ClientMessage::SlayerBuyReward { reward_id, target_monster_id })
        }
        "slayerRemoveBlock" => {
            let monster_id = extract_string(msg_data, "monster_id").unwrap_or_default();
            Ok(ClientMessage::SlayerRemoveBlock { monster_id })
        }
        "startAutoAction" => {
            let target_type = extract_string(msg_data, "target_type").unwrap_or_default();
            let target_id = extract_string(msg_data, "target_id").unwrap_or_default();
            let action = extract_string(msg_data, "action").unwrap_or_default();
            Ok(ClientMessage::StartAutoAction { target_type, target_id, action })
        }
        "cancelAutoAction" => Ok(ClientMessage::CancelAutoAction),
        "interactObject" => {
            let x = extract_i32(msg_data, "x").unwrap_or(0);
            let y = extract_i32(msg_data, "y").unwrap_or(0);
            Ok(ClientMessage::InteractObject { x, y })
        }
        "useWaystone" => {
            let x = extract_i32(msg_data, "x").unwrap_or(0);
            let y = extract_i32(msg_data, "y").unwrap_or(0);
            Ok(ClientMessage::UseWaystone { x, y })
        }
        "openChest" => {
            let x = extract_i32(msg_data, "x").unwrap_or(0);
            let y = extract_i32(msg_data, "y").unwrap_or(0);
            Ok(ClientMessage::OpenChest { x, y })
        }
        "chestTake" => {
            let chest_id = extract_string(msg_data, "chest_id").unwrap_or_default();
            let slot = extract_i32(msg_data, "slot").unwrap_or(0) as u8;
            Ok(ClientMessage::ChestTake { chest_id, slot })
        }
        "chestDeposit" => {
            let chest_id = extract_string(msg_data, "chest_id").unwrap_or_default();
            let inventory_slot = extract_i32(msg_data, "inventory_slot").unwrap_or(0) as u8;
            Ok(ClientMessage::ChestDeposit { chest_id, inventory_slot })
        }
        _ => Err(format!("Unknown message type: {}", msg_type)),
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn extract_string(value: &rmpv::Value, key: &str) -> Option<String> {
    value.as_map().and_then(|map| {
        map.iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .and_then(|(_, v)| v.as_str().map(|s| s.to_string()))
    })
}

fn extract_f64(value: &rmpv::Value, key: &str) -> Option<f64> {
    value.as_map().and_then(|map| {
        map.iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .and_then(|(_, v)| v.as_f64())
    })
}

fn extract_f32(value: &rmpv::Value, key: &str) -> Option<f32> {
    value.as_map().and_then(|map| {
        map.iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .and_then(|(_, v)| {
                v.as_f64()
                    .map(|f| f as f32)
                    .or_else(|| v.as_i64().map(|i| i as f32))
                    .or_else(|| v.as_u64().map(|u| u as f32))
            })
    })
}

fn extract_i32(value: &rmpv::Value, key: &str) -> Option<i32> {
    value.as_map().and_then(|map| {
        map.iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .and_then(|(_, v)| {
                v.as_i64()
                    .map(|i| i as i32)
                    .or_else(|| v.as_u64().map(|u| u as i32))
            })
    })
}

fn extract_u32(value: &rmpv::Value, key: &str) -> Option<u32> {
    value.as_map().and_then(|map| {
        map.iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .and_then(|(_, v)| {
                v.as_u64()
                    .map(|u| u as u32)
                    .or_else(|| v.as_i64().map(|i| i as u32))
            })
    })
}

fn extract_i64(value: &rmpv::Value, key: &str) -> Option<i64> {
    value.as_map().and_then(|map| {
        map.iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .and_then(|(_, v)| v.as_i64().or_else(|| v.as_u64().map(|u| u as i64)))
    })
}

// ============================================================================
// Binary Compression (for StateSync bandwidth reduction)
// ============================================================================

const COMPRESSION_THRESHOLD: usize = 128;

/// Wrap a MessagePack payload with a compression prefix.
/// - 0x00 prefix: uncompressed data follows
/// - 0x01 prefix: deflate-compressed data follows
/// Only compresses if the payload exceeds COMPRESSION_THRESHOLD bytes.
pub fn maybe_compress(data: Vec<u8>) -> Vec<u8> {
    use flate2::write::DeflateEncoder;
    use flate2::Compression;
    use std::io::Write;

    if data.len() <= COMPRESSION_THRESHOLD {
        let mut out = Vec::with_capacity(1 + data.len());
        out.push(0x00);
        out.extend_from_slice(&data);
        return out;
    }

    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
    if encoder.write_all(&data).is_ok() {
        if let Ok(compressed) = encoder.finish() {
            if compressed.len() < data.len() {
                let mut out = Vec::with_capacity(1 + compressed.len());
                out.push(0x01);
                out.extend_from_slice(&compressed);
                return out;
            }
        }
    }

    // Fallback: uncompressed
    let mut out = Vec::with_capacity(1 + data.len());
    out.push(0x00);
    out.extend_from_slice(&data);
    out
}
