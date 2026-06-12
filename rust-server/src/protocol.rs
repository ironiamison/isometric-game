use serde::{Deserialize, Serialize};

use crate::game::PlayerUpdate;
use crate::npc::NpcUpdate;

mod encoding;
mod server_messages;
mod state_sync;

pub use aeven_protocol::{ClientMessage, PROTOCOL_VERSION, decode_client_message};
pub use encoding::encode_server_message;
pub use server_messages::*;
pub use state_sync::*;
// ============================================================================
// Binary Compression (for StateSync bandwidth reduction)
// ============================================================================

// Deflate setup overhead is noticeable for tiny high-frequency deltas.
// Keep small payloads uncompressed to reduce server-side sync CPU spikes.
const COMPRESSION_THRESHOLD: usize = 1024;

/// Wrap a MessagePack payload with a compression prefix.
///
/// - 0x00 prefix: uncompressed data follows
/// - 0x01 prefix: deflate-compressed data follows
///
/// Only compresses if the payload exceeds COMPRESSION_THRESHOLD bytes.
pub fn maybe_compress(data: Vec<u8>) -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;

    if data.len() <= COMPRESSION_THRESHOLD {
        let mut out = Vec::with_capacity(1 + data.len());
        out.push(0x00);
        out.extend_from_slice(&data);
        return out;
    }

    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
    if encoder.write_all(&data).is_ok()
        && let Ok(compressed) = encoder.finish()
        && compressed.len() < data.len()
    {
        let mut out = Vec::with_capacity(1 + compressed.len());
        out.push(0x01);
        out.extend_from_slice(&compressed);
        return out;
    }

    // Fallback: uncompressed
    let mut out = Vec::with_capacity(1 + data.len());
    out.push(0x00);
    out.extend_from_slice(&data);
    out
}
