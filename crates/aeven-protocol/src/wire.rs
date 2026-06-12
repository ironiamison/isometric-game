use crate::ClientMessage;
use rmpv::Value;
use std::io::Cursor;

pub const PROTOCOL_VERSION: u16 = 1;
pub const ROOM_DATA_PROTOCOL_CODE: u8 = 13;
pub const MAX_CLIENT_MESSAGE_BYTES: usize = 64 * 1024;

pub fn encode_client_message(message: &ClientMessage) -> Result<Vec<u8>, String> {
    let tagged = serde_json::to_value(message)
        .map_err(|error| format!("failed to serialize client message: {error}"))?;
    let serde_json::Value::Object(entries) = tagged else {
        return Err("client message did not serialize to an object".to_string());
    };

    let mut message_type = None;
    let mut payload = Vec::with_capacity(entries.len().saturating_sub(1));
    for (key, value) in entries {
        if key == "type" {
            message_type = value.as_str().map(str::to_owned);
        } else {
            payload.push((Value::from(key), json_to_msgpack(value)?));
        }
    }
    let message_type =
        message_type.ok_or_else(|| "client message is missing its type tag".to_string())?;

    encode_envelope(&message_type, payload)
}

fn encode_envelope(message_type: &str, payload: Vec<(Value, Value)>) -> Result<Vec<u8>, String> {
    let envelope = Value::Array(vec![
        Value::from(ROOM_DATA_PROTOCOL_CODE),
        Value::from(message_type),
        Value::Map(payload),
    ]);
    let mut bytes = Vec::new();
    rmpv::encode::write_value(&mut bytes, &envelope)
        .map_err(|error| format!("failed to encode client message: {error}"))?;
    Ok(bytes)
}

fn json_to_msgpack(value: serde_json::Value) -> Result<Value, String> {
    match value {
        serde_json::Value::Null => Ok(Value::Nil),
        serde_json::Value::Bool(value) => Ok(Value::Boolean(value)),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Ok(Value::from(value))
            } else if let Some(value) = value.as_u64() {
                Ok(Value::from(value))
            } else if let Some(value) = value.as_f64() {
                Ok(Value::from(value))
            } else {
                Err("client message contains an unsupported number".to_string())
            }
        }
        serde_json::Value::String(value) => Ok(Value::from(value)),
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(json_to_msgpack)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        serde_json::Value::Object(values) => values
            .into_iter()
            .map(|(key, value)| Ok((Value::from(key), json_to_msgpack(value)?)))
            .collect::<Result<Vec<_>, String>>()
            .map(Value::Map),
    }
}

pub fn decode_client_message(bytes: &[u8]) -> Result<ClientMessage, String> {
    if bytes.is_empty() {
        return Err("client message is empty".to_string());
    }
    if bytes.len() > MAX_CLIENT_MESSAGE_BYTES {
        return Err(format!(
            "client message exceeds {MAX_CLIENT_MESSAGE_BYTES} byte limit"
        ));
    }

    let mut cursor = Cursor::new(bytes);
    let value = rmpv::decode::read_value(&mut cursor)
        .map_err(|error| format!("failed to decode MessagePack: {error}"))?;
    if cursor.position() != bytes.len() as u64 {
        return Err("client message contains trailing bytes".to_string());
    }

    let Value::Array(mut envelope) = value else {
        return Err("client message envelope must be an array".to_string());
    };
    if envelope.len() != 3 {
        return Err("client message envelope must contain exactly three values".to_string());
    }

    let payload = envelope.pop().expect("length checked");
    let message_type = envelope
        .pop()
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or_else(|| "client message type must be a string".to_string())?;
    let protocol_code = envelope
        .pop()
        .and_then(|value| value.as_u64())
        .ok_or_else(|| "client message protocol code must be an integer".to_string())?;
    if protocol_code != u64::from(ROOM_DATA_PROTOCOL_CODE) {
        return Err(format!("unsupported protocol code: {protocol_code}"));
    }

    let Value::Map(mut fields) = payload else {
        return Err("client message payload must be a map".to_string());
    };
    fields.push((Value::from("type"), Value::from(message_type)));
    rmpv::ext::from_value(Value::Map(fields))
        .map_err(|error| format!("invalid client message payload: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(message: ClientMessage) {
        let bytes = encode_client_message(&message).unwrap();
        assert_eq!(decode_client_message(&bytes).unwrap(), message);
    }

    #[test]
    fn representative_commands_round_trip() {
        round_trip(ClientMessage::Move {
            dx: 0.5,
            dy: -1.0,
            seq: Some(42),
        });
        round_trip(ClientMessage::RequestChunk {
            chunk_x: -3,
            chunk_y: 7,
        });
        round_trip(ClientMessage::ShopBuy {
            npc_id: "merchant".to_string(),
            item_id: "potion".to_string(),
            quantity: 3,
        });
        round_trip(ClientMessage::KothContinue);
    }

    #[test]
    fn malformed_commands_are_rejected_instead_of_defaulted() {
        let envelope = Value::Array(vec![
            Value::from(ROOM_DATA_PROTOCOL_CODE),
            Value::from("move"),
            Value::Map(vec![(Value::from("dx"), Value::from(1.0))]),
        ]);
        let mut bytes = Vec::new();
        rmpv::encode::write_value(&mut bytes, &envelope).unwrap();

        assert!(decode_client_message(&bytes).is_err());
    }

    #[test]
    fn legacy_snake_case_aliases_remain_compatible() {
        let envelope = Value::Array(vec![
            Value::from(ROOM_DATA_PROTOCOL_CODE),
            Value::from("requestChunk"),
            Value::Map(vec![
                (Value::from("chunk_x"), Value::from(2)),
                (Value::from("chunk_y"), Value::from(-4)),
            ]),
        ]);
        let mut bytes = Vec::new();
        rmpv::encode::write_value(&mut bytes, &envelope).unwrap();

        assert_eq!(
            decode_client_message(&bytes).unwrap(),
            ClientMessage::RequestChunk {
                chunk_x: 2,
                chunk_y: -4,
            }
        );
    }
}
