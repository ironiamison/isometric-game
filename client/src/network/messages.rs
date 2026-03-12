use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// === Client -> Server Messages ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "auth")]
    Auth { username: String, password: String },

    #[serde(rename = "register")]
    Register { username: String, password: String },

    #[serde(rename = "move")]
    Move { dx: f32, dy: f32, seq: u32 },

    #[serde(rename = "dash")]
    Dash,

    #[serde(rename = "jump")]
    Jump,

    #[serde(rename = "face")]
    Face { direction: u8 },

    #[serde(rename = "moveTo")]
    MoveTo { x: f32, y: f32 },

    #[serde(rename = "target")]
    Target { entity_id: String },

    #[serde(rename = "attack")]
    Attack,

    #[serde(rename = "chat")]
    Chat { text: String, channel: String },

    #[serde(rename = "pickup")]
    Pickup { item_id: String },

    #[serde(rename = "useItem")]
    UseItem { slot_index: u32 },

    #[serde(rename = "requestChunk")]
    RequestChunk { chunk_x: i32, chunk_y: i32 },

    // Quest-related messages
    #[serde(rename = "interact")]
    Interact { npc_id: String },

    /// Interact with a world map object (obelisk, etc.)
    #[serde(rename = "interactObject")]
    InteractObject { x: i32, y: i32 },

    /// Direct waystone teleport (no dialogue)
    #[serde(rename = "useWaystone")]
    UseWaystone { x: i32, y: i32 },

    #[serde(rename = "dialogueChoice")]
    DialogueChoice { quest_id: String, choice_id: String },

    #[serde(rename = "acceptQuest")]
    AcceptQuest { quest_id: String },

    #[serde(rename = "abandonQuest")]
    AbandonQuest { quest_id: String },

    #[serde(rename = "craft")]
    Craft { recipe_id: String },

    #[serde(rename = "startCraft")]
    StartCraft { recipe_id: String },

    #[serde(rename = "cancelCraft")]
    CancelCraft,

    #[serde(rename = "equip")]
    Equip { slot_index: u8 },

    #[serde(rename = "unequip")]
    Unequip {
        slot_type: String,
        target_slot: Option<u8>,
    },

    #[serde(rename = "dropItem")]
    DropItem {
        slot_index: u8,
        quantity: u32,
        target_x: Option<i32>,
        target_y: Option<i32>,
    },

    #[serde(rename = "dropGold")]
    DropGold { amount: i32 },

    #[serde(rename = "swapSlots")]
    SwapSlots { from_slot: u8, to_slot: u8 },

    #[serde(rename = "shopBuy")]
    ShopBuy {
        npc_id: String,
        item_id: String,
        quantity: u32,
    },

    #[serde(rename = "shopSell")]
    ShopSell {
        npc_id: String,
        item_id: String,
        quantity: u32,
    },

    // Bank commands
    #[serde(rename = "bankDeposit")]
    BankDeposit { item_id: String, quantity: i32 },

    #[serde(rename = "bankWithdraw")]
    BankWithdraw { item_id: String, quantity: i32 },

    #[serde(rename = "bankDepositGold")]
    BankDepositGold { amount: i32 },

    #[serde(rename = "bankWithdrawGold")]
    BankWithdrawGold { amount: i32 },

    #[serde(rename = "bankDepositAll")]
    BankDepositAll,

    #[serde(rename = "bankSwapSlots")]
    BankSwapSlots { slot_a: u32, slot_b: u32 },

    #[serde(rename = "bankSort")]
    BankSort,

    #[serde(rename = "enterPortal")]
    EnterPortal { portal_id: String },

    // Gathering commands
    #[serde(rename = "startGathering")]
    StartGathering { marker_x: i32, marker_y: i32 },

    #[serde(rename = "stopGathering")]
    StopGathering,

    // Woodcutting commands
    #[serde(rename = "chopTree")]
    ChopTree {
        tree_x: i32,
        tree_y: i32,
        tree_gid: u32,
    },

    // Mining commands
    #[serde(rename = "mineRock")]
    MineRock {
        rock_x: i32,
        rock_y: i32,
        rock_gid: u32,
    },

    // Chair commands
    #[serde(rename = "sitChair")]
    SitChair { tile_x: i32, tile_y: i32 },

    #[serde(rename = "standUp")]
    StandUp,

    // Farming commands
    #[serde(rename = "plantSeed")]
    PlantSeed { patch_id: String, item_id: String },

    #[serde(rename = "harvestCrop")]
    HarvestCrop { patch_id: String },

    // Friend system commands
    #[serde(rename = "sendFriendRequest")]
    SendFriendRequest { target_name: String },

    #[serde(rename = "acceptFriendRequest")]
    AcceptFriendRequest { requester_id: i64 },

    #[serde(rename = "declineFriendRequest")]
    DeclineFriendRequest { requester_id: i64 },

    #[serde(rename = "removeFriend")]
    RemoveFriend { friend_id: i64 },

    #[serde(rename = "getOnlinePlayers")]
    GetOnlinePlayers,

    // Prayer commands
    #[serde(rename = "togglePrayer")]
    TogglePrayer { prayer_id: String },

    #[serde(rename = "buryBones")]
    BuryBones { slot: usize },

    // Altar commands
    #[serde(rename = "offerBones")]
    OfferBones { slot: usize, altar_id: String },

    #[serde(rename = "offerAllBones")]
    OfferAllBones { item_id: String, altar_id: String },

    #[serde(rename = "prayAtAltar")]
    PrayAtAltar { altar_id: String },

    // Spell system commands
    #[serde(rename = "castSpell")]
    CastSpell { spell_id: String },

    #[serde(rename = "startCraftBatch")]
    StartCraftBatch { recipe_id: String, quantity: u32 },

    // Slayer commands
    #[serde(rename = "slayerGetTask")]
    SlayerGetTask { master_id: String },

    #[serde(rename = "slayerCancelTask")]
    SlayerCancelTask,

    #[serde(rename = "slayerBuyReward")]
    SlayerBuyReward {
        reward_id: String,
        target_monster_id: Option<String>,
    },

    #[serde(rename = "slayerRemoveBlock")]
    SlayerRemoveBlock { monster_id: String },

    // Auto-action commands (click-to-act chase system)
    // Chest commands
    #[serde(rename = "openChest")]
    OpenChest { x: i32, y: i32 },

    #[serde(rename = "chestTake")]
    ChestTake { chest_id: String, slot: u8 },

    #[serde(rename = "chestDeposit")]
    ChestDeposit {
        chest_id: String,
        inventory_slot: u8,
    },

    #[serde(rename = "startAutoAction")]
    StartAutoAction {
        target_type: String,
        target_id: String,
        action: String,
    },

    #[serde(rename = "cancelAutoAction")]
    CancelAutoAction,

    // Ping for latency measurement
    #[serde(rename = "ping")]
    Ping { timestamp: f64 },

    // ===== Trade System =====
    #[serde(rename = "tradeRequest")]
    TradeRequest { target_id: String },
    #[serde(rename = "tradeAcceptRequest")]
    TradeAcceptRequest { requester_id: String },
    #[serde(rename = "tradeDeclineRequest")]
    TradeDeclineRequest { requester_id: String },
    #[serde(rename = "tradeOfferItem")]
    TradeOfferItem { slot_index: u8, quantity: i32 },
    #[serde(rename = "tradeRemoveItem")]
    TradeRemoveItem { offer_index: u8 },
    #[serde(rename = "tradeOfferGold")]
    TradeOfferGold { amount: i32 },
    #[serde(rename = "tradeAccept")]
    TradeAccept,
    #[serde(rename = "tradeCancel")]
    TradeCancel,

    // ===== Player Stall System =====
    #[serde(rename = "stallOpen")]
    StallOpen { name: String },
    #[serde(rename = "stallClose")]
    StallClose,
    #[serde(rename = "stallSetItem")]
    StallSetItem {
        inventory_slot: u8,
        quantity: i32,
        price: i32,
    },
    #[serde(rename = "stallRemoveItem")]
    StallRemoveItem { stall_slot: u8 },
    #[serde(rename = "stallUpdatePrice")]
    StallUpdatePrice { stall_slot: u8, price: i32 },
    #[serde(rename = "stallBrowse")]
    StallBrowse { player_id: String },
    #[serde(rename = "stallBuy")]
    StallBuy {
        seller_id: String,
        stall_slot: u8,
        quantity: i32,
    },

    /// Spectator upgrades to a full player session
    #[serde(rename = "spectatorUpgrade")]
    SpectatorUpgrade { session_token: String },

    /// Set combat style (accurate, aggressive, defensive, controlled)
    #[serde(rename = "setCombatStyle")]
    SetCombatStyle { style: String },
}

impl ClientMessage {
    /// Convert message to Colyseus protocol format (type, data)
    pub fn to_protocol(&self) -> (&'static str, HashMap<String, rmpv::Value>) {
        use rmpv::Value;

        let mut data = HashMap::new();

        let msg_type = match self {
            ClientMessage::Auth { username, password } => {
                data.insert("username".into(), Value::String(username.clone().into()));
                data.insert("password".into(), Value::String(password.clone().into()));
                "auth"
            }
            ClientMessage::Register { username, password } => {
                data.insert("username".into(), Value::String(username.clone().into()));
                data.insert("password".into(), Value::String(password.clone().into()));
                "register"
            }
            ClientMessage::Move { dx, dy, seq } => {
                data.insert("dx".into(), Value::F64(*dx as f64));
                data.insert("dy".into(), Value::F64(*dy as f64));
                data.insert("seq".into(), Value::Integer((*seq as i64).into()));
                "move"
            }
            ClientMessage::Dash => "dash",
            ClientMessage::Jump => "jump",
            ClientMessage::Face { direction } => {
                data.insert(
                    "direction".into(),
                    Value::Integer((*direction as i64).into()),
                );
                "face"
            }
            ClientMessage::MoveTo { x, y } => {
                data.insert("x".into(), Value::F64(*x as f64));
                data.insert("y".into(), Value::F64(*y as f64));
                "moveTo"
            }
            ClientMessage::Target { entity_id } => {
                data.insert("entity_id".into(), Value::String(entity_id.clone().into()));
                "target"
            }
            ClientMessage::Attack => "attack",
            ClientMessage::Chat { text, channel } => {
                data.insert("text".into(), Value::String(text.clone().into()));
                data.insert("channel".into(), Value::String(channel.clone().into()));
                "chat"
            }
            ClientMessage::Pickup { item_id } => {
                data.insert("item_id".into(), Value::String(item_id.clone().into()));
                "pickup"
            }
            ClientMessage::UseItem { slot_index } => {
                data.insert(
                    "slot_index".into(),
                    Value::Integer((*slot_index as i64).into()),
                );
                "useItem"
            }
            ClientMessage::RequestChunk { chunk_x, chunk_y } => {
                data.insert("chunkX".into(), Value::Integer((*chunk_x as i64).into()));
                data.insert("chunkY".into(), Value::Integer((*chunk_y as i64).into()));
                "requestChunk"
            }
            ClientMessage::Interact { npc_id } => {
                data.insert("npc_id".into(), Value::String(npc_id.clone().into()));
                "interact"
            }
            ClientMessage::InteractObject { x, y } => {
                data.insert("x".into(), Value::Integer((*x as i64).into()));
                data.insert("y".into(), Value::Integer((*y as i64).into()));
                "interactObject"
            }
            ClientMessage::UseWaystone { x, y } => {
                data.insert("x".into(), Value::Integer((*x as i64).into()));
                data.insert("y".into(), Value::Integer((*y as i64).into()));
                "useWaystone"
            }
            ClientMessage::DialogueChoice {
                quest_id,
                choice_id,
            } => {
                data.insert("quest_id".into(), Value::String(quest_id.clone().into()));
                data.insert("choice_id".into(), Value::String(choice_id.clone().into()));
                "dialogueChoice"
            }
            ClientMessage::AcceptQuest { quest_id } => {
                data.insert("quest_id".into(), Value::String(quest_id.clone().into()));
                "acceptQuest"
            }
            ClientMessage::AbandonQuest { quest_id } => {
                data.insert("quest_id".into(), Value::String(quest_id.clone().into()));
                "abandonQuest"
            }
            ClientMessage::Craft { recipe_id } => {
                data.insert("recipe_id".into(), Value::String(recipe_id.clone().into()));
                "craft"
            }
            ClientMessage::StartCraft { recipe_id } => {
                data.insert("recipe_id".into(), Value::String(recipe_id.clone().into()));
                "startCraft"
            }
            ClientMessage::CancelCraft => "cancelCraft",
            ClientMessage::Equip { slot_index } => {
                data.insert(
                    "slot_index".into(),
                    Value::Integer((*slot_index as i64).into()),
                );
                "equip"
            }
            ClientMessage::Unequip {
                slot_type,
                target_slot,
            } => {
                data.insert("slot_type".into(), Value::String(slot_type.clone().into()));
                if let Some(slot) = target_slot {
                    data.insert("target_slot".into(), Value::Integer((*slot as i64).into()));
                }
                "unequip"
            }
            ClientMessage::DropItem {
                slot_index,
                quantity,
                target_x,
                target_y,
            } => {
                data.insert(
                    "slot_index".into(),
                    Value::Integer((*slot_index as i64).into()),
                );
                data.insert("quantity".into(), Value::Integer((*quantity as i64).into()));
                if let Some(x) = target_x {
                    data.insert("target_x".into(), Value::Integer((*x as i64).into()));
                }
                if let Some(y) = target_y {
                    data.insert("target_y".into(), Value::Integer((*y as i64).into()));
                }
                "dropItem"
            }
            ClientMessage::DropGold { amount } => {
                data.insert("amount".into(), Value::Integer((*amount as i64).into()));
                "dropGold"
            }
            ClientMessage::SwapSlots { from_slot, to_slot } => {
                data.insert(
                    "from_slot".into(),
                    Value::Integer((*from_slot as i64).into()),
                );
                data.insert("to_slot".into(), Value::Integer((*to_slot as i64).into()));
                "swapSlots"
            }
            ClientMessage::ShopBuy {
                npc_id,
                item_id,
                quantity,
            } => {
                data.insert("npcId".into(), Value::String(npc_id.clone().into()));
                data.insert("itemId".into(), Value::String(item_id.clone().into()));
                data.insert("quantity".into(), Value::Integer((*quantity as i64).into()));
                "shopBuy"
            }
            ClientMessage::ShopSell {
                npc_id,
                item_id,
                quantity,
            } => {
                data.insert("npcId".into(), Value::String(npc_id.clone().into()));
                data.insert("itemId".into(), Value::String(item_id.clone().into()));
                data.insert("quantity".into(), Value::Integer((*quantity as i64).into()));
                "shopSell"
            }
            ClientMessage::BankDeposit { item_id, quantity } => {
                data.insert("item_id".into(), Value::String(item_id.clone().into()));
                data.insert("quantity".into(), Value::Integer((*quantity as i64).into()));
                "bankDeposit"
            }
            ClientMessage::BankWithdraw { item_id, quantity } => {
                data.insert("item_id".into(), Value::String(item_id.clone().into()));
                data.insert("quantity".into(), Value::Integer((*quantity as i64).into()));
                "bankWithdraw"
            }
            ClientMessage::BankDepositGold { amount } => {
                data.insert("amount".into(), Value::Integer((*amount as i64).into()));
                "bankDepositGold"
            }
            ClientMessage::BankWithdrawGold { amount } => {
                data.insert("amount".into(), Value::Integer((*amount as i64).into()));
                "bankWithdrawGold"
            }
            ClientMessage::BankDepositAll => "bankDepositAll",
            ClientMessage::BankSwapSlots { slot_a, slot_b } => {
                data.insert("slot_a".into(), Value::Integer((*slot_a as i64).into()));
                data.insert("slot_b".into(), Value::Integer((*slot_b as i64).into()));
                "bankSwapSlots"
            }
            ClientMessage::BankSort => "bankSort",
            ClientMessage::EnterPortal { portal_id } => {
                data.insert("portalId".into(), Value::String(portal_id.clone().into()));
                "enterPortal"
            }
            ClientMessage::StartGathering { marker_x, marker_y } => {
                data.insert("marker_x".into(), Value::Integer((*marker_x as i64).into()));
                data.insert("marker_y".into(), Value::Integer((*marker_y as i64).into()));
                "startGathering"
            }
            ClientMessage::StopGathering => "stopGathering",
            ClientMessage::ChopTree {
                tree_x,
                tree_y,
                tree_gid,
            } => {
                data.insert("tree_x".into(), Value::Integer((*tree_x as i64).into()));
                data.insert("tree_y".into(), Value::Integer((*tree_y as i64).into()));
                data.insert("tree_gid".into(), Value::Integer((*tree_gid as i64).into()));
                "chopTree"
            }
            ClientMessage::MineRock {
                rock_x,
                rock_y,
                rock_gid,
            } => {
                data.insert("rock_x".into(), Value::Integer((*rock_x as i64).into()));
                data.insert("rock_y".into(), Value::Integer((*rock_y as i64).into()));
                data.insert("rock_gid".into(), Value::Integer((*rock_gid as i64).into()));
                "mineRock"
            }
            ClientMessage::SitChair { tile_x, tile_y } => {
                data.insert("tile_x".into(), Value::Integer((*tile_x as i64).into()));
                data.insert("tile_y".into(), Value::Integer((*tile_y as i64).into()));
                "sitChair"
            }
            ClientMessage::StandUp => "standUp",
            ClientMessage::PlantSeed { patch_id, item_id } => {
                data.insert("patch_id".into(), Value::String(patch_id.clone().into()));
                data.insert("item_id".into(), Value::String(item_id.clone().into()));
                "plantSeed"
            }
            ClientMessage::HarvestCrop { patch_id } => {
                data.insert("patch_id".into(), Value::String(patch_id.clone().into()));
                "harvestCrop"
            }
            ClientMessage::SendFriendRequest { target_name } => {
                data.insert(
                    "target_name".into(),
                    Value::String(target_name.clone().into()),
                );
                "sendFriendRequest"
            }
            ClientMessage::AcceptFriendRequest { requester_id } => {
                data.insert(
                    "requester_id".into(),
                    Value::Integer((*requester_id).into()),
                );
                "acceptFriendRequest"
            }
            ClientMessage::DeclineFriendRequest { requester_id } => {
                data.insert(
                    "requester_id".into(),
                    Value::Integer((*requester_id).into()),
                );
                "declineFriendRequest"
            }
            ClientMessage::RemoveFriend { friend_id } => {
                data.insert("friend_id".into(), Value::Integer((*friend_id).into()));
                "removeFriend"
            }
            ClientMessage::GetOnlinePlayers => "getOnlinePlayers",
            ClientMessage::TogglePrayer { prayer_id } => {
                data.insert("prayer_id".into(), Value::String(prayer_id.clone().into()));
                "togglePrayer"
            }
            ClientMessage::BuryBones { slot } => {
                data.insert("slot".into(), Value::Integer((*slot as i64).into()));
                "buryBones"
            }
            ClientMessage::OfferBones { slot, altar_id } => {
                data.insert("slot".into(), Value::Integer((*slot as i64).into()));
                data.insert("altar_id".into(), Value::String(altar_id.clone().into()));
                "offerBones"
            }
            ClientMessage::OfferAllBones { item_id, altar_id } => {
                data.insert("item_id".into(), Value::String(item_id.clone().into()));
                data.insert("altar_id".into(), Value::String(altar_id.clone().into()));
                "offerAllBones"
            }
            ClientMessage::PrayAtAltar { altar_id } => {
                data.insert("altar_id".into(), Value::String(altar_id.clone().into()));
                "prayAtAltar"
            }
            ClientMessage::CastSpell { spell_id } => {
                data.insert("spell_id".into(), Value::String(spell_id.clone().into()));
                "castSpell"
            }
            ClientMessage::StartCraftBatch {
                recipe_id,
                quantity,
            } => {
                data.insert("recipe_id".into(), Value::String(recipe_id.clone().into()));
                data.insert("quantity".into(), Value::from(*quantity as i64));
                "startCraftBatch"
            }
            ClientMessage::SlayerGetTask { master_id } => {
                data.insert("master_id".into(), Value::String(master_id.clone().into()));
                "slayerGetTask"
            }
            ClientMessage::SlayerCancelTask => "slayerCancelTask",
            ClientMessage::SlayerBuyReward {
                reward_id,
                target_monster_id,
            } => {
                data.insert("reward_id".into(), Value::String(reward_id.clone().into()));
                if let Some(target) = target_monster_id {
                    data.insert(
                        "target_monster_id".into(),
                        Value::String(target.clone().into()),
                    );
                }
                "slayerBuyReward"
            }
            ClientMessage::SlayerRemoveBlock { monster_id } => {
                data.insert(
                    "monster_id".into(),
                    Value::String(monster_id.clone().into()),
                );
                "slayerRemoveBlock"
            }
            ClientMessage::OpenChest { x, y } => {
                data.insert("x".into(), Value::Integer((*x as i64).into()));
                data.insert("y".into(), Value::Integer((*y as i64).into()));
                "openChest"
            }
            ClientMessage::ChestTake { chest_id, slot } => {
                data.insert("chest_id".into(), Value::String(chest_id.clone().into()));
                data.insert("slot".into(), Value::Integer((*slot as i64).into()));
                "chestTake"
            }
            ClientMessage::ChestDeposit {
                chest_id,
                inventory_slot,
            } => {
                data.insert("chest_id".into(), Value::String(chest_id.clone().into()));
                data.insert(
                    "inventory_slot".into(),
                    Value::Integer((*inventory_slot as i64).into()),
                );
                "chestDeposit"
            }
            ClientMessage::StartAutoAction {
                target_type,
                target_id,
                action,
            } => {
                data.insert(
                    "target_type".into(),
                    Value::String(target_type.clone().into()),
                );
                data.insert("target_id".into(), Value::String(target_id.clone().into()));
                data.insert("action".into(), Value::String(action.clone().into()));
                "startAutoAction"
            }
            ClientMessage::CancelAutoAction => "cancelAutoAction",
            ClientMessage::Ping { timestamp } => {
                data.insert("timestamp".into(), Value::F64(*timestamp));
                "ping"
            }
            // Trade system
            ClientMessage::TradeRequest { target_id } => {
                data.insert("target_id".into(), Value::String(target_id.clone().into()));
                "tradeRequest"
            }
            ClientMessage::TradeAcceptRequest { requester_id } => {
                data.insert(
                    "requester_id".into(),
                    Value::String(requester_id.clone().into()),
                );
                "tradeAcceptRequest"
            }
            ClientMessage::TradeDeclineRequest { requester_id } => {
                data.insert(
                    "requester_id".into(),
                    Value::String(requester_id.clone().into()),
                );
                "tradeDeclineRequest"
            }
            ClientMessage::TradeOfferItem {
                slot_index,
                quantity,
            } => {
                data.insert(
                    "slot_index".into(),
                    Value::Integer((*slot_index as i64).into()),
                );
                data.insert("quantity".into(), Value::Integer((*quantity as i64).into()));
                "tradeOfferItem"
            }
            ClientMessage::TradeRemoveItem { offer_index } => {
                data.insert(
                    "offer_index".into(),
                    Value::Integer((*offer_index as i64).into()),
                );
                "tradeRemoveItem"
            }
            ClientMessage::TradeOfferGold { amount } => {
                data.insert("amount".into(), Value::Integer((*amount as i64).into()));
                "tradeOfferGold"
            }
            ClientMessage::TradeAccept => "tradeAccept",
            ClientMessage::TradeCancel => "tradeCancel",
            // Stall system
            ClientMessage::StallOpen { name } => {
                data.insert("name".into(), Value::String(name.clone().into()));
                "stallOpen"
            }
            ClientMessage::StallClose => "stallClose",
            ClientMessage::StallSetItem {
                inventory_slot,
                quantity,
                price,
            } => {
                data.insert(
                    "inventory_slot".into(),
                    Value::Integer((*inventory_slot as i64).into()),
                );
                data.insert("quantity".into(), Value::Integer((*quantity as i64).into()));
                data.insert("price".into(), Value::Integer((*price as i64).into()));
                "stallSetItem"
            }
            ClientMessage::StallRemoveItem { stall_slot } => {
                data.insert(
                    "stall_slot".into(),
                    Value::Integer((*stall_slot as i64).into()),
                );
                "stallRemoveItem"
            }
            ClientMessage::StallUpdatePrice { stall_slot, price } => {
                data.insert(
                    "stall_slot".into(),
                    Value::Integer((*stall_slot as i64).into()),
                );
                data.insert("price".into(), Value::Integer((*price as i64).into()));
                "stallUpdatePrice"
            }
            ClientMessage::StallBrowse { player_id } => {
                data.insert("player_id".into(), Value::String(player_id.clone().into()));
                "stallBrowse"
            }
            ClientMessage::StallBuy {
                seller_id,
                stall_slot,
                quantity,
            } => {
                data.insert("seller_id".into(), Value::String(seller_id.clone().into()));
                data.insert(
                    "stall_slot".into(),
                    Value::Integer((*stall_slot as i64).into()),
                );
                data.insert("quantity".into(), Value::Integer((*quantity as i64).into()));
                "stallBuy"
            }
            ClientMessage::SpectatorUpgrade { session_token } => {
                data.insert(
                    "sessionToken".into(),
                    Value::String(session_token.clone().into()),
                );
                "spectatorUpgrade"
            }
            ClientMessage::SetCombatStyle { style } => {
                data.insert("style".into(), Value::String(style.clone().into()));
                "setCombatStyle"
            }
        };

        (msg_type, data)
    }
}

/// Helper function to create an enter portal message
pub fn enter_portal(portal_id: &str) -> ClientMessage {
    ClientMessage::EnterPortal {
        portal_id: portal_id.to_string(),
    }
}

// Note: Server messages are handled via the protocol module's MessagePack decoder
// The ServerMessage enum below is kept for reference but is not directly used

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "welcome")]
    Welcome { player_id: String },

    #[serde(rename = "error")]
    Error { code: u32, message: String },

    #[serde(rename = "playerJoined")]
    PlayerJoined {
        id: String,
        name: String,
        x: f32,
        y: f32,
    },

    #[serde(rename = "playerLeft")]
    PlayerLeft { id: String },

    #[serde(rename = "stateSync")]
    StateSync {
        tick: u64,
        players: Vec<PlayerUpdate>,
    },

    #[serde(rename = "chatMessage")]
    ChatMessage {
        sender_name: String,
        text: String,
        timestamp: f64,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerUpdate {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hp: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "maxHp")]
    pub max_hp: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "expToNextLevel")]
    pub exp_to_next_level: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gold: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_moving: Option<bool>,
}
