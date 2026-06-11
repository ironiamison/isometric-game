use super::*;

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
        "jump" => Ok(ClientMessage::Jump),
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
        "useItemOn" => {
            let slot_index = msg_data
                .as_map()
                .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("slot_index")))
                .and_then(|(_, v)| v.as_u64().map(|u| u as u8))
                .unwrap_or(0);
            let target_npc_id = extract_string(msg_data, "target_npc_id").unwrap_or_default();
            Ok(ClientMessage::UseItemOn {
                slot_index,
                target_npc_id,
            })
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
            Ok(ClientMessage::StartCraftBatch {
                recipe_id,
                quantity,
            })
        }
        "slayerGetTask" => {
            let master_id = extract_string(msg_data, "master_id").unwrap_or_default();
            Ok(ClientMessage::SlayerGetTask { master_id })
        }
        "slayerCancelTask" => Ok(ClientMessage::SlayerCancelTask),
        "slayerBuyReward" => {
            let reward_id = extract_string(msg_data, "reward_id").unwrap_or_default();
            let target_monster_id = extract_string(msg_data, "target_monster_id");
            Ok(ClientMessage::SlayerBuyReward {
                reward_id,
                target_monster_id,
            })
        }
        "slayerRemoveBlock" => {
            let monster_id = extract_string(msg_data, "monster_id").unwrap_or_default();
            Ok(ClientMessage::SlayerRemoveBlock { monster_id })
        }
        "startAutoAction" => {
            let target_type = extract_string(msg_data, "target_type").unwrap_or_default();
            let target_id = extract_string(msg_data, "target_id").unwrap_or_default();
            let action = extract_string(msg_data, "action").unwrap_or_default();
            Ok(ClientMessage::StartAutoAction {
                target_type,
                target_id,
                action,
            })
        }
        "cancelAutoAction" => Ok(ClientMessage::CancelAutoAction),
        "setAutoRetaliate" => {
            let enabled = extract_bool(msg_data, "enabled").unwrap_or(true);
            Ok(ClientMessage::SetAutoRetaliate { enabled })
        }
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
            Ok(ClientMessage::ChestDeposit {
                chest_id,
                inventory_slot,
            })
        }
        "spectatorUpgrade" => {
            let session_token = extract_string(msg_data, "sessionToken").unwrap_or_default();
            Ok(ClientMessage::SpectatorUpgrade { session_token })
        }
        // ===== Trade System Messages =====
        "tradeRequest" => {
            let target_id = extract_string(msg_data, "target_id").unwrap_or_default();
            Ok(ClientMessage::TradeRequest { target_id })
        }
        "tradeAcceptRequest" => {
            let requester_id = extract_string(msg_data, "requester_id").unwrap_or_default();
            Ok(ClientMessage::TradeAcceptRequest { requester_id })
        }
        "tradeDeclineRequest" => {
            let requester_id = extract_string(msg_data, "requester_id").unwrap_or_default();
            Ok(ClientMessage::TradeDeclineRequest { requester_id })
        }
        "tradeOfferItem" => {
            let slot_index = extract_i32(msg_data, "slot_index").unwrap_or(0) as u8;
            let quantity = extract_i32(msg_data, "quantity").unwrap_or(1);
            Ok(ClientMessage::TradeOfferItem {
                slot_index,
                quantity,
            })
        }
        "tradeRemoveItem" => {
            let offer_index = extract_i32(msg_data, "offer_index").unwrap_or(0) as u8;
            Ok(ClientMessage::TradeRemoveItem { offer_index })
        }
        "tradeOfferGold" => {
            let amount = extract_i32(msg_data, "amount").unwrap_or(0);
            Ok(ClientMessage::TradeOfferGold { amount })
        }
        "tradeAccept" => Ok(ClientMessage::TradeAccept),
        "tradeCancel" => Ok(ClientMessage::TradeCancel),
        // ===== Player Stall System Messages =====
        "stallOpen" => {
            let name = extract_string(msg_data, "name").unwrap_or_default();
            Ok(ClientMessage::StallOpen { name })
        }
        "stallClose" => Ok(ClientMessage::StallClose),
        "stallSetItem" => {
            let inventory_slot = extract_i32(msg_data, "inventory_slot").unwrap_or(0) as u8;
            let quantity = extract_i32(msg_data, "quantity").unwrap_or(1);
            let price = extract_i32(msg_data, "price").unwrap_or(0);
            Ok(ClientMessage::StallSetItem {
                inventory_slot,
                quantity,
                price,
            })
        }
        "stallRemoveItem" => {
            let stall_slot = extract_i32(msg_data, "stall_slot").unwrap_or(0) as u8;
            Ok(ClientMessage::StallRemoveItem { stall_slot })
        }
        "stallUpdatePrice" => {
            let stall_slot = extract_i32(msg_data, "stall_slot").unwrap_or(0) as u8;
            let price = extract_i32(msg_data, "price").unwrap_or(0);
            Ok(ClientMessage::StallUpdatePrice { stall_slot, price })
        }
        "stallBrowse" => {
            let player_id = extract_string(msg_data, "player_id").unwrap_or_default();
            Ok(ClientMessage::StallBrowse { player_id })
        }
        "stallBuy" => {
            let seller_id = extract_string(msg_data, "seller_id").unwrap_or_default();
            let stall_slot = extract_i32(msg_data, "stall_slot").unwrap_or(0) as u8;
            let quantity = extract_i32(msg_data, "quantity").unwrap_or(1);
            let expected_price = extract_i32(msg_data, "expected_price").unwrap_or(-1);
            Ok(ClientMessage::StallBuy {
                seller_id,
                stall_slot,
                quantity,
                expected_price,
            })
        }
        "setCombatStyle" => {
            let style = extract_string(msg_data, "style").unwrap_or_default();
            Ok(ClientMessage::SetCombatStyle { style })
        }
        "kothContinue" => Ok(ClientMessage::KothContinue),
        "kothLeave" => Ok(ClientMessage::KothLeave),
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

fn extract_bool(value: &rmpv::Value, key: &str) -> Option<bool> {
    value.as_map().and_then(|map| {
        map.iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .and_then(|(_, v)| v.as_bool())
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
                    .and_then(|i| i32::try_from(i).ok())
                    .or_else(|| v.as_u64().and_then(|u| i32::try_from(u).ok()))
            })
    })
}

fn extract_u32(value: &rmpv::Value, key: &str) -> Option<u32> {
    value.as_map().and_then(|map| {
        map.iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .and_then(|(_, v)| {
                v.as_u64()
                    .and_then(|u| u32::try_from(u).ok())
                    .or_else(|| v.as_i64().and_then(|i| u32::try_from(i).ok()))
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
