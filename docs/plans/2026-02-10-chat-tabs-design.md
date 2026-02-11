# Chat Tab System Design

## Overview

Add a tabbed chat system with three channels: **Public** (nearby players), **Global** (server-wide), and **System** (XP gains, quest notifications, etc.). Messages are filtered by active tab, and the active tab determines the send channel.

## Decisions

- **Nearby range**: 40 tiles (Chebyshev distance, matches VIEW_DISTANCE)
- **Default send channel**: matches active tab
- **`~` prefix**: forces global regardless of active tab
- **Desktop UI**: small clickable tabs above chat log
- **Chat bubbles**: Public messages only
- **System tab**: hides input field

## Protocol Changes

### ClientMessage::Chat
Add `channel` field:
```rust
Chat { text: String, channel: String }  // "public" or "global"
```

### ServerMessage::ChatMessage
Add `channel` field:
```rust
ChatMessage {
    sender_id: String,
    sender_name: String,
    text: String,
    timestamp: u64,
    channel: String,  // "public", "global", or "system"
}
```

## Server Changes (`game.rs`)

### handle_chat()
- Read `channel` from incoming message
- **Global**: `broadcast()` to all players (current behavior)
- **Public**: filter players by Chebyshev distance ≤ 40 from sender, `send_to_player()` to each nearby player (always include sender)
- System messages unchanged — already unicast via `send_to_player()`

## Client Changes

### Message Handler (`message_handler.rs`)
- Read `channel` field from incoming ChatMessage
- Map: `"public"` → `ChatChannel::Local`, `"global"` → `ChatChannel::Global`, `"system"` → `ChatChannel::System`
- Chat bubbles only created for Public channel messages

### Input Logic (`handler.rs` + `messages.rs`)
- Send channel matches `chat_active_tab`: Local → "public", Global → "global"
- System tab: input hidden/disabled
- `~` prefix: strip it, force channel to "global"
- Update `ClientMessage::Chat` encoding to include channel

### Filtering
- All messages stored in single `chat_messages` Vec
- Filtering at render time based on `chat_active_tab`

### Desktop UI (`renderer.rs`)
- Three clickable tabs above chat log: `Public | Global | System`
- Active tab gets highlight background
- Chat log filters messages by active tab
- Channel indicator next to input field (e.g. `[Public]`)
- Input hidden when System tab active

### Mobile UI (`chat_panel.rs`)
- Already has tab infrastructure
- Update to send with correct channel field
- Filter incoming messages by channel field
