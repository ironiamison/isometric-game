#![forbid(unsafe_code)]

mod client_message;
mod wire;

pub use client_message::ClientMessage;
pub use wire::{
    MAX_CLIENT_MESSAGE_BYTES, PROTOCOL_VERSION, ROOM_DATA_PROTOCOL_CODE, decode_client_message,
    encode_client_message,
};
