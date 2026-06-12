use super::state_sync::{slayer_reward_to_value, slayer_task_to_value};
use super::*;
use rmpv::Value;

// Wire maps are appended in protocol order and may include conditional fields.
#[allow(clippy::vec_init_then_push)]
mod core {
    include!("encoding_core.rs");
}
#[allow(clippy::vec_init_then_push)]
mod combat {
    include!("encoding_combat.rs");
}
#[allow(clippy::vec_init_then_push)]
mod items {
    include!("encoding_items.rs");
}
#[allow(clippy::vec_init_then_push)]
mod quests {
    include!("encoding_quests.rs");
}
#[allow(clippy::vec_init_then_push)]
mod world {
    include!("encoding_world.rs");
}
#[allow(clippy::vec_init_then_push)]
mod commerce {
    include!("encoding_commerce.rs");
}
#[allow(clippy::vec_init_then_push)]
mod gathering {
    include!("encoding_gathering.rs");
}
#[allow(clippy::vec_init_then_push)]
mod events {
    include!("encoding_events.rs");
}

fn encode_data(msg: &ServerMessage) -> Result<Value, String> {
    let encoders: [fn(&ServerMessage) -> Option<Value>; 8] = [
        core::encode,
        combat::encode,
        items::encode,
        quests::encode,
        world::encode,
        commerce::encode,
        gathering::encode,
        events::encode,
    ];
    encoders
        .into_iter()
        .find_map(|encode| encode(msg))
        .ok_or_else(|| format!("No encoder for server message {}", msg.msg_type()))
}

/// Encode a server message to MessagePack format.
pub fn encode_server_message(msg: &ServerMessage) -> Result<Vec<u8>, String> {
    let msg_type = msg.msg_type();
    let data = encode_data(msg)?;
    let array = Value::Array(vec![
        Value::Integer(13.into()),
        Value::String(msg_type.into()),
        data,
    ]);
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &array)
        .map_err(|e| format!("Failed to encode message: {}", e))?;
    Ok(buf)
}
