use serde::{Deserialize, Serialize};

// ============================================================================
// Client -> Server Messages
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    #[serde(rename = "jump")]
    Jump,

    #[serde(rename = "face")]
    Face { direction: u8 },

    #[serde(rename = "chat")]
    Chat {
        text: String,
        #[serde(default)]
        channel: String,
    },

    #[serde(rename = "attack")]
    Attack,

    #[serde(rename = "target")]
    Target { entity_id: String },

    #[serde(rename = "pickup")]
    Pickup { item_id: String },

    #[serde(rename = "useItem")]
    UseItem { slot_index: u8 },

    UseItemOn {
        slot_index: u8,
        target_npc_id: String,
    },

    #[serde(rename = "auth")]
    Auth { username: String, password: String },

    #[serde(rename = "register")]
    Register { username: String, password: String },

    #[serde(rename = "requestChunk")]
    RequestChunk {
        #[serde(rename = "chunkX", alias = "chunk_x")]
        chunk_x: i32,
        #[serde(rename = "chunkY", alias = "chunk_y")]
        chunk_y: i32,
    },

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
    Unequip {
        slot_type: String,
        #[serde(default)]
        target_slot: Option<u8>,
    },

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
        #[serde(rename = "npcId", alias = "npc_id")]
        npc_id: String,
        #[serde(rename = "itemId", alias = "item_id")]
        item_id: String,
        quantity: i32,
    },

    /// Sell item to shop
    #[serde(rename = "shopSell")]
    ShopSell {
        #[serde(rename = "npcId", alias = "npc_id")]
        npc_id: String,
        #[serde(rename = "itemId", alias = "item_id")]
        item_id: String,
        quantity: i32,
    },

    /// Enter a portal to transition to another map
    #[serde(rename = "enterPortal")]
    EnterPortal {
        #[serde(rename = "portalId", alias = "portal_id")]
        portal_id: String,
    },

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

    /// Apply compost to a farming patch (reduces disease, adds a harvest life)
    #[serde(rename = "applyCompost")]
    ApplyCompost { patch_id: String, item_id: String },

    /// Cure a diseased farming patch
    #[serde(rename = "curePatch")]
    CurePatch { patch_id: String },

    /// Clear a dead crop from a farming patch
    #[serde(rename = "clearPatch")]
    ClearPatch { patch_id: String },

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
    SlayerBuyReward {
        reward_id: String,
        #[serde(default)]
        target_monster_id: Option<String>,
    },

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

    /// Toggle auto-retaliate on/off
    #[serde(rename = "setAutoRetaliate")]
    SetAutoRetaliate { enabled: bool },

    // ===== Chest System Messages =====
    /// Open a chest at position
    #[serde(rename = "openChest")]
    OpenChest { x: i32, y: i32 },

    /// Take item from a chest slot
    #[serde(rename = "chestTake")]
    ChestTake { chest_id: String, slot: u8 },

    /// Deposit item from inventory into chest
    #[serde(rename = "chestDeposit")]
    ChestDeposit {
        chest_id: String,
        inventory_slot: u8,
    },

    /// Spectator upgrades to a full player session
    #[serde(rename = "spectatorUpgrade")]
    SpectatorUpgrade { session_token: String },

    // ===== Trade System Messages =====
    /// Request to trade with another player
    #[serde(rename = "tradeRequest")]
    TradeRequest { target_id: String },

    /// Accept an incoming trade request
    #[serde(rename = "tradeAcceptRequest")]
    TradeAcceptRequest { requester_id: String },

    /// Decline an incoming trade request
    #[serde(rename = "tradeDeclineRequest")]
    TradeDeclineRequest { requester_id: String },

    /// Add inventory item to trade offer
    #[serde(rename = "tradeOfferItem")]
    TradeOfferItem { slot_index: u8, quantity: i32 },

    /// Remove item from trade offer
    #[serde(rename = "tradeRemoveItem")]
    TradeRemoveItem { offer_index: u8 },

    /// Set gold amount in trade offer
    #[serde(rename = "tradeOfferGold")]
    TradeOfferGold { amount: i32 },

    /// Accept current trade offers
    #[serde(rename = "tradeAccept")]
    TradeAccept,

    /// Cancel active trade
    #[serde(rename = "tradeCancel")]
    TradeCancel,

    // ===== Player Stall System Messages =====
    /// Open a player stall with a custom name
    #[serde(rename = "stallOpen")]
    StallOpen { name: String },

    /// Close player stall
    #[serde(rename = "stallClose")]
    StallClose,

    /// Move item from inventory to stall slot with price
    #[serde(rename = "stallSetItem")]
    StallSetItem {
        inventory_slot: u8,
        quantity: i32,
        price: i32,
    },

    /// Remove item from stall back to inventory
    #[serde(rename = "stallRemoveItem")]
    StallRemoveItem { stall_slot: u8 },

    /// Update price of a stall slot
    #[serde(rename = "stallUpdatePrice")]
    StallUpdatePrice { stall_slot: u8, price: i32 },

    /// Browse another player's stall
    #[serde(rename = "stallBrowse")]
    StallBrowse { player_id: String },

    /// Buy from another player's stall
    #[serde(rename = "stallBuy")]
    StallBuy {
        seller_id: String,
        stall_slot: u8,
        quantity: i32,
        expected_price: i32,
    },

    /// Set the player's combat style
    #[serde(rename = "setCombatStyle")]
    SetCombatStyle { style: String },

    // ===== KOTH Messages =====
    /// Player continues fighting at KOTH checkpoint
    #[serde(rename = "kothContinue")]
    KothContinue,

    /// Player leaves KOTH and claims rewards
    #[serde(rename = "kothLeave")]
    KothLeave,
}

impl ClientMessage {
    pub fn name(&self) -> &'static str {
        match self {
            ClientMessage::Move { .. } => "Move",
            ClientMessage::Dash => "Dash",
            ClientMessage::Jump => "Jump",
            ClientMessage::Face { .. } => "Face",
            ClientMessage::Chat { .. } => "Chat",
            ClientMessage::Attack => "Attack",
            ClientMessage::Target { .. } => "Target",
            ClientMessage::Pickup { .. } => "Pickup",
            ClientMessage::UseItem { .. } => "UseItem",
            ClientMessage::UseItemOn { .. } => "UseItemOn",
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
            ClientMessage::ApplyCompost { .. } => "ApplyCompost",
            ClientMessage::CurePatch { .. } => "CurePatch",
            ClientMessage::ClearPatch { .. } => "ClearPatch",
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
            ClientMessage::SetAutoRetaliate { .. } => "setAutoRetaliate",
            ClientMessage::OpenChest { .. } => "OpenChest",
            ClientMessage::ChestTake { .. } => "ChestTake",
            ClientMessage::ChestDeposit { .. } => "ChestDeposit",
            // Trade system
            ClientMessage::TradeRequest { .. } => "TradeRequest",
            ClientMessage::TradeAcceptRequest { .. } => "TradeAcceptRequest",
            ClientMessage::TradeDeclineRequest { .. } => "TradeDeclineRequest",
            ClientMessage::TradeOfferItem { .. } => "TradeOfferItem",
            ClientMessage::TradeRemoveItem { .. } => "TradeRemoveItem",
            ClientMessage::TradeOfferGold { .. } => "TradeOfferGold",
            ClientMessage::TradeAccept => "TradeAccept",
            ClientMessage::TradeCancel => "TradeCancel",
            // Stall system
            ClientMessage::StallOpen { .. } => "StallOpen",
            ClientMessage::StallClose => "StallClose",
            ClientMessage::StallSetItem { .. } => "StallSetItem",
            ClientMessage::StallRemoveItem { .. } => "StallRemoveItem",
            ClientMessage::StallUpdatePrice { .. } => "StallUpdatePrice",
            ClientMessage::StallBrowse { .. } => "StallBrowse",
            ClientMessage::StallBuy { .. } => "StallBuy",
            ClientMessage::SpectatorUpgrade { .. } => "SpectatorUpgrade",
            ClientMessage::SetCombatStyle { .. } => "SetCombatStyle",
            ClientMessage::KothContinue => "KothContinue",
            ClientMessage::KothLeave => "KothLeave",
        }
    }
}
