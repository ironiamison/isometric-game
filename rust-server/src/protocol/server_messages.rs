use super::*;

// ============================================================================
// Chest Data Structs
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct PotionBuffEntry {
    pub stat: String,
    pub amount: i32,
    pub remaining_ms: u64,
    pub source_item_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChestSlotUpdate {
    pub slot: u8,
    pub item_id: String,
    pub quantity: i32,
    pub value: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradeOfferItemData {
    pub slot_index: u8,
    pub item_id: String,
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct StallSlotData {
    pub slot: u8,
    pub item_id: String,
    pub quantity: i32,
    pub price: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldMapChunkSampleData {
    pub chunk_x: i32,
    pub chunk_y: i32,
    pub low_tiles: Vec<u32>,
    pub high_tiles: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldMapPoiData {
    pub x: f32,
    pub y: f32,
    pub label: String,
    pub icon_index: u8,
    pub kind: u8,
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
        direction: u8,
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
        attack_level: i32,
        attack_xp: i64,
        strength_level: i32,
        strength_xp: i64,
        defence_level: i32,
        defence_xp: i64,
        ranged_level: i32,
        ranged_xp: i64,
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
        survivalist_level: i32,
        survivalist_xp: i64,
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
        collision: Vec<u8>,                  // Packed collision bits
        objects: Vec<ChunkObjectData>,       // Map objects (trees, rocks, etc.)
        walls: Vec<ChunkWallData>,           // Edge-aligned walls
        portals: Vec<ChunkPortalData>,       // Portals to other maps
        heightmap: Option<Vec<u8>>,          // Optional height data (CHUNK_SIZE^2 bytes)
        block_types_down: Option<Vec<u16>>,  // Wall sprite IDs for down (+Y) side face
        block_types_right: Option<Vec<u16>>, // Wall sprite IDs for right (+X) side face
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
    // KOTH (King of the Hill) messages
    KothStateUpdate {
        phase: String,
        wave: u32,
        points: u32,
        enemies_alive: u32,
        enemies_total: u32,
        countdown_ms: u32,
    },
    KothCheckpoint {
        wave: u32,
        points: u32,
        rewards: Vec<KothRewardData>,
        next_wave_enemy_count: u32,
    },
    KothGameOver {
        waves_completed: u32,
        total_points: u32,
        rewards: Vec<KothRewardData>,
        victory: bool,
    },
    // Boss fight messages
    BossStateUpdate {
        boss_id: String,
        hp: i32,
        max_hp: i32,
        phase: String,
        wurm_state: String,
    },
    AoeWarning {
        tiles: Vec<(i32, i32)>,
        delay_ms: u64,
        effect: String,
    },
    AoeDamage {
        tiles: Vec<(i32, i32)>,
        damage: i32,
        effect: String,
    },
    Explosion {
        x: i32,
        y: i32,
        radius: i32,
        damage: i32,
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
        heightmap: Option<Vec<u8>>,
        block_types_down: Option<Vec<u16>>,
        block_types_right: Option<Vec<u16>>,
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

    BuffApplied {
        player_id: String,
        buff_type: String,
        duration: u64,
    },
    BuffExpired {
        player_id: String,
        buff_type: String,
    },
    PotionBuffsSync {
        player_id: String,
        buffs: Vec<PotionBuffEntry>,
    },
    ChairPositions {
        positions: Vec<(i32, i32)>,
    },
    ChestPositions {
        positions: Vec<(i32, i32)>,
    },
    WorldMapData {
        min_x: i32,
        min_y: i32,
        max_x: i32,
        max_y: i32,
        low_sample_dim: u8,
        high_sample_dim: u8,
        chunks: Vec<WorldMapChunkSampleData>,
        pois: Vec<WorldMapPoiData>,
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
        spell_id: Option<String>,
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
    ResourceContractUpdate {
        active: bool,
        contract_kind: String,
        difficulty: String,
        task_text: String,
        progress_label: String,
        amount_required: i32,
        amount_completed: i32,
        giver_name: String,
    },
    AdventureBoardState {
        npc_id: String,
        offers: Vec<AdventureBoardOfferData>,
        active_contract: Option<AdventureBoardActiveContractData>,
        stats: AdventureBoardStatsData,
        crafting_orders: Vec<CraftingOrderOfferData>,
        crafting_order_active: Option<CraftingOrderActiveData>,
        crafting_order_stats: CraftingOrderStatsData,
        daily_contracts_completed: i32,
        daily_contract_limit: i32,
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
        blockable_monsters: Vec<(String, String)>,
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
    /// Auto-retaliate setting changed
    AutoRetaliateChanged {
        enabled: bool,
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

    // ===== Trade System Messages =====
    /// Incoming trade request notification
    TradeRequestReceived {
        requester_id: String,
        requester_name: String,
    },
    /// Trade window opened
    TradeOpened {
        trade_id: String,
        partner_id: String,
        partner_name: String,
    },
    /// Partner's offer updated
    TradeOfferUpdate {
        partner_items: Vec<TradeOfferItemData>,
        partner_gold: i32,
        partner_accepted: bool,
    },
    /// Server confirms your offer
    TradeMyOfferUpdate {
        my_items: Vec<TradeOfferItemData>,
        my_gold: i32,
        my_accepted: bool,
    },
    /// Trade completed successfully
    TradeCompleted {
        items_received: Vec<TradeOfferItemData>,
        gold_received: i32,
    },
    /// Trade cancelled
    TradeCancelled {
        reason: String,
    },

    // ===== Player Stall System Messages =====
    /// Confirms stall opened (to owner)
    StallOpened {
        name: String,
        slots: Vec<StallSlotData>,
    },
    /// Stall closed (to owner)
    StallClosed {
        reason: String,
    },
    /// Stall contents changed (to owner)
    StallUpdate {
        slots: Vec<StallSlotData>,
    },
    /// Browse data sent to buyer
    StallBrowseData {
        seller_id: String,
        seller_name: String,
        stall_name: String,
        items: Vec<StallSlotData>,
    },
    /// Purchase result
    StallBuyResult {
        success: bool,
        item_id: String,
        quantity: i32,
        total_price: i32,
        error: Option<String>,
    },
    /// Sale notification to seller
    StallSaleNotification {
        item_id: String,
        quantity: i32,
        gold_received: i32,
        buyer_name: String,
    },
    /// Live update for browsers when stall item changes
    StallItemUpdate {
        seller_id: String,
        stall_slot: u8,
        new_quantity: i32,
    },
    /// Notify clients of the all-time top total level players (for trophy icons)
    TopPlayerChanged {
        player_name: Option<String>,
        second_player_name: Option<String>,
    },
    // Collection log messages
    CollectionLogDefinitions {
        entries: Vec<(String, String, String)>,
        /// source_detail_id -> display_name (e.g., "pig" -> "Pig", "axe_to_grind" -> "Axe to Grind")
        display_names: Vec<(String, String)>,
    },
    CollectionLogSync {
        entries: Vec<(String, String, String, String)>,
    },
    CollectionLogEntry {
        item_id: String,
        source: String,
        source_detail: String,
        obtained_at: String,
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

/// Reward item data for KOTH messages
#[derive(Debug, Clone, Serialize)]
pub struct KothRewardData {
    pub item_id: String,
    pub quantity: u32,
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
    pub ranged_level_required: Option<i32>,
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
    /// Ranged strength bonus for ammunition (arrows)
    pub ranged_strength: i32,
    /// Ranged strength bonus from equipment (necklaces, belts, etc.)
    pub ranged_strength_bonus: Option<i32>,
    /// Use effect type string (e.g. "dig", "heal") - lets client show context menu actions
    pub use_effect_type: Option<String>,
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
    pub required_tool: Option<String>,
    pub burn_result: Option<String>,
    pub burn_stop_level: Option<i32>,
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

#[derive(Debug, Clone, Serialize)]
pub struct AdventureBoardDifficultyData {
    pub difficulty_id: String,
    pub difficulty_name: String,
    pub level_required: i32,
    pub unlocked: bool,
    pub reward_xp: i64,
    pub reward_gold: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdventureBoardOfferData {
    pub kind_id: String,
    pub kind_name: String,
    pub description: String,
    pub skill_level: i32,
    pub difficulties: Vec<AdventureBoardDifficultyData>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CraftingOrderOfferData {
    pub order_id: String,
    pub tier: String,
    pub skill: String,
    pub min_level: i32,
    pub items: Vec<CraftingOrderItemData>,
    pub reward_gold: i32,
    pub reward_xp: Vec<(String, i64)>,
    pub reward_marks: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CraftingOrderItemData {
    pub item_id: String,
    pub item_name: String,
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CraftingOrderActiveData {
    pub order_id: String,
    pub tier: String,
    pub skill: String,
    pub items: Vec<CraftingOrderItemData>,
    pub reward_gold: i32,
    pub reward_marks: i32,
    pub can_claim: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CraftingOrderStatsData {
    pub orders_completed: i32,
    pub masterwork_completed: i32,
    pub commission_marks: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdventureBoardActiveContractData {
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

#[derive(Debug, Clone, Serialize)]
pub struct AdventureBoardStatsData {
    pub contracts_completed: i32,
    pub total_gold_earned: i32,
    pub total_xp_earned: i64,
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
            ServerMessage::KothStateUpdate { .. } => "kothStateUpdate",
            ServerMessage::KothCheckpoint { .. } => "kothCheckpoint",
            ServerMessage::KothGameOver { .. } => "kothGameOver",
            ServerMessage::BossStateUpdate { .. } => "bossStateUpdate",
            ServerMessage::AoeWarning { .. } => "aoeWarning",
            ServerMessage::AoeDamage { .. } => "aoeDamage",
            ServerMessage::Explosion { .. } => "explosion",
            ServerMessage::Announcement { .. } => "announcement",
            ServerMessage::NpcSpeech { .. } => "npcSpeech",
            ServerMessage::MapTransition { .. } => "mapTransition",
            ServerMessage::InteriorData { .. } => "interiorData",
            ServerMessage::GatheringMarkers { .. } => "gatheringMarkers",
            ServerMessage::GatheringStarted { .. } => "gatheringStarted",
            ServerMessage::GatheringResult { .. } => "gatheringResult",
            ServerMessage::GatheringStopped { .. } => "gatheringStopped",

            ServerMessage::BuffApplied { .. } => "buffApplied",
            ServerMessage::BuffExpired { .. } => "buffExpired",
            ServerMessage::PotionBuffsSync { .. } => "potionBuffsSync",
            ServerMessage::ChairPositions { .. } => "chairPositions",
            ServerMessage::ChestPositions { .. } => "chestPositions",
            ServerMessage::WorldMapData { .. } => "worldMapData",
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
            ServerMessage::ResourceContractUpdate { .. } => "resourceContractUpdate",
            ServerMessage::AdventureBoardState { .. } => "adventureBoardState",
            // Slayer system messages
            ServerMessage::SlayerPanelOpen { .. } => "slayerPanelOpen",
            ServerMessage::SlayerTaskProgress { .. } => "slayerTaskProgress",
            ServerMessage::SlayerTaskComplete { .. } => "slayerTaskComplete",
            ServerMessage::SlayerResult { .. } => "slayerResult",
            ServerMessage::SlayerStateSync { .. } => "slayerStateSync",
            // Auto-action system messages
            ServerMessage::AutoActionStarted { .. } => "autoActionStarted",
            ServerMessage::AutoActionStopped { .. } => "autoActionStopped",
            ServerMessage::AutoRetaliateChanged { .. } => "autoRetaliateChanged",
            // Scroll spell system messages
            ServerMessage::ScrollSpellDefinitions { .. } => "scrollSpellDefinitions",
            ServerMessage::UnlockedSpellsSync { .. } => "unlockedSpellsSync",
            ServerMessage::SpellUnlocked { .. } => "spellUnlocked",
            ServerMessage::Pushback { .. } => "pushback",
            // Chest system messages
            ServerMessage::ChestOpen { .. } => "chestOpen",
            ServerMessage::ChestUpdate { .. } => "chestUpdate",
            // Trade system messages
            ServerMessage::TradeRequestReceived { .. } => "tradeRequestReceived",
            ServerMessage::TradeOpened { .. } => "tradeOpened",
            ServerMessage::TradeOfferUpdate { .. } => "tradeOfferUpdate",
            ServerMessage::TradeMyOfferUpdate { .. } => "tradeMyOfferUpdate",
            ServerMessage::TradeCompleted { .. } => "tradeCompleted",
            ServerMessage::TradeCancelled { .. } => "tradeCancelled",
            // Stall system messages
            ServerMessage::StallOpened { .. } => "stallOpened",
            ServerMessage::StallClosed { .. } => "stallClosed",
            ServerMessage::StallUpdate { .. } => "stallUpdate",
            ServerMessage::StallBrowseData { .. } => "stallBrowseData",
            ServerMessage::StallBuyResult { .. } => "stallBuyResult",
            ServerMessage::StallSaleNotification { .. } => "stallSaleNotification",
            ServerMessage::StallItemUpdate { .. } => "stallItemUpdate",
            ServerMessage::TopPlayerChanged { .. } => "topPlayerChanged",
            // Collection log
            ServerMessage::CollectionLogDefinitions { .. } => "collectionLogDefinitions",
            ServerMessage::CollectionLogSync { .. } => "collectionLogSync",
            ServerMessage::CollectionLogEntry { .. } => "collectionLogEntry",
        }
    }
}
