# Chat Tab System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Public/Global/System chat tabs with proximity-based public chat, global broadcast, and filtered system messages.

**Architecture:** Add a `channel` field to the chat protocol (client→server and server→client). Server routes public chat to nearby players (Chebyshev ≤ 40), global chat to all. Client filters display by active tab and determines send channel from active tab. Desktop gets clickable tabs above chat log; mobile already has tab infrastructure.

**Tech Stack:** Rust (server: Axum/Tokio, client: Macroquad), MessagePack protocol

---

### Task 1: Server Protocol — Add channel field to ChatMessage

**Files:**
- Modify: `rust-server/src/protocol.rs:19-20` (ClientMessage::Chat)
- Modify: `rust-server/src/protocol.rs:284-291` (ServerMessage::ChatMessage)

**Step 1: Add channel to ClientMessage::Chat**

In `rust-server/src/protocol.rs`, change:
```rust
#[serde(rename = "chat")]
Chat { text: String },
```
to:
```rust
#[serde(rename = "chat")]
Chat { text: String, #[serde(default)] channel: String },
```

The `#[serde(default)]` ensures backward compatibility — old clients that don't send `channel` will default to empty string, which we'll treat as "public".

**Step 2: Add channel to ServerMessage::ChatMessage**

In `rust-server/src/protocol.rs`, change:
```rust
ChatMessage {
    #[serde(rename = "senderId")]
    sender_id: String,
    #[serde(rename = "senderName")]
    sender_name: String,
    text: String,
    timestamp: u64,
},
```
to:
```rust
ChatMessage {
    #[serde(rename = "senderId")]
    sender_id: String,
    #[serde(rename = "senderName")]
    sender_name: String,
    text: String,
    timestamp: u64,
    channel: String,
},
```

**Step 3: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | grep "error"`

This will show errors in `game.rs` wherever `ServerMessage::ChatMessage` is constructed without the new `channel` field — that's expected and fixed in Task 2.

**Step 4: Commit**

```bash
git add rust-server/src/protocol.rs
git commit -m "feat(chat): add channel field to chat protocol messages"
```

---

### Task 2: Server — Route chat by channel

**Files:**
- Modify: `rust-server/src/game.rs:1779-1806` (handle_chat)
- Modify: `rust-server/src/game.rs:2366-2377` (send_system_message)
- Modify: `rust-server/src/main.rs:2496-2498` (message dispatch)

**Step 1: Update message dispatch in main.rs**

In `rust-server/src/main.rs`, change:
```rust
ClientMessage::Chat { text } => {
    room.handle_chat(player_id, &text).await;
}
```
to:
```rust
ClientMessage::Chat { text, channel } => {
    room.handle_chat(player_id, &text, &channel).await;
}
```

**Step 2: Rewrite handle_chat with channel routing**

In `rust-server/src/game.rs`, replace the `handle_chat` method (lines 1779-1806) with:

```rust
pub async fn handle_chat(&self, player_id: &str, text: &str, channel: &str) {
    let sanitized = text.trim().chars().take(200).collect::<String>();
    if sanitized.is_empty() {
        return;
    }

    // Check for commands (messages starting with /)
    if sanitized.starts_with('/') {
        self.handle_chat_command(player_id, &sanitized).await;
        return;
    }

    let players = self.players.read().await;
    let sender = match players.get(player_id) {
        Some(p) => p,
        None => return,
    };
    let sender_name = sender.name.clone();
    let sender_x = sender.x as i32;
    let sender_y = sender.y as i32;
    drop(players);

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    if channel == "global" {
        let msg = ServerMessage::ChatMessage {
            sender_id: player_id.to_string(),
            sender_name,
            text: sanitized,
            timestamp,
            channel: "global".to_string(),
        };
        self.broadcast(msg).await;
    } else {
        // Public (nearby) chat — send to players within VIEW_DISTANCE
        let msg = ServerMessage::ChatMessage {
            sender_id: player_id.to_string(),
            sender_name,
            text: sanitized,
            timestamp,
            channel: "public".to_string(),
        };

        let player_instances = self.player_instances.read().await;
        let sender_instance = player_instances.get(player_id).cloned();
        let all_players = self.players.read().await;
        let senders = self.player_senders.read().await;

        if let Ok(bytes) = crate::protocol::encode_server_message(&msg) {
            for (pid, sender_ch) in senders.iter() {
                // Must be in same instance
                if player_instances.get(pid).cloned() != sender_instance {
                    continue;
                }
                // Always send to self
                if pid == player_id {
                    let _ = sender_ch.try_send(bytes.clone());
                    continue;
                }
                // Check distance
                if let Some(other) = all_players.get(pid) {
                    let dx = (other.x as i32 - sender_x).abs();
                    let dy = (other.y as i32 - sender_y).abs();
                    if dx.max(dy) <= VIEW_DISTANCE {
                        let _ = sender_ch.try_send(bytes.clone());
                    }
                }
            }
        }
    }
}
```

**Step 3: Update send_system_message to include channel**

In `rust-server/src/game.rs`, update `send_system_message` (lines 2366-2377) to include the channel field:

```rust
async fn send_system_message(&self, player_id: &str, text: &str) {
    let msg = ServerMessage::ChatMessage {
        sender_id: "system".to_string(),
        sender_name: "[System]".to_string(),
        text: text.to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        channel: "system".to_string(),
    };
    self.send_to_player(player_id, msg).await;
}
```

**Step 4: Fix any remaining ChatMessage constructions**

Search for other `ServerMessage::ChatMessage` constructions and add `channel` field. Check:
- Any admin `/announce` command that builds a ChatMessage directly

Run: `cd rust-server && cargo check 2>&1 | grep "error"` — should compile cleanly.

**Step 5: Commit**

```bash
git add rust-server/src/game.rs rust-server/src/main.rs
git commit -m "feat(chat): route public chat by proximity, global to all"
```

---

### Task 3: Client Protocol — Add channel to ClientMessage::Chat

**Files:**
- Modify: `client/src/input/handler.rs:77` (InputCommand enum)
- Modify: `client/src/network/messages.rs:192-195` (ClientMessage encoding)
- Modify: `client/src/app.rs:224-232` (InputCommand → ClientMessage mapping)
- Modify: `client/src/main.rs:467-475` (same mapping, WASM entry point)

**Step 1: Add channel to InputCommand::Chat**

In `client/src/input/handler.rs`, change:
```rust
Chat { text: String },
```
to:
```rust
Chat { text: String, channel: String },
```

**Step 2: Add channel to ClientMessage::Chat**

In `client/src/network/messages.rs` (or wherever `ClientMessage` is defined on the client), ensure the Chat variant includes channel. Find the `ClientMessage` enum definition and add the field. Then update the encoding at lines 192-195:

```rust
ClientMessage::Chat { text, channel } => {
    data.insert("text".into(), Value::String(text.clone().into()));
    data.insert("channel".into(), Value::String(channel.clone().into()));
    "chat"
}
```

**Step 3: Update InputCommand→ClientMessage mapping in app.rs**

In `client/src/app.rs`, update the chat mapping (lines 224-232):
```rust
InputCommand::Chat { text, channel } => {
    // Handle /ping command
    if text.trim().eq_ignore_ascii_case("/ping") {
        let timestamp = get_time();
        game_state.ping_sent_at = Some(timestamp);
        ClientMessage::Ping { timestamp }
    } else {
        ClientMessage::Chat { text: text.clone(), channel: channel.clone() }
    }
},
```

**Step 4: Update the same mapping in main.rs (WASM entry)**

In `client/src/main.rs`, apply the same change at lines 467-475.

**Step 5: Fix all InputCommand::Chat call sites**

Every place that pushes `InputCommand::Chat { text }` needs updating to include a channel. For now, use a placeholder — we'll wire up the actual tab-based channel in Task 5. Set all to `"public".to_string()` for now.

There are call sites in `client/src/input/handler.rs` at approximately lines 871 and 2329.

**Step 6: Verify compilation**

Run: `cd client && cargo check 2>&1 | grep "error"`

**Step 7: Commit**

```bash
git add client/src/input/handler.rs client/src/network/messages.rs client/src/app.rs client/src/main.rs
git commit -m "feat(chat): add channel field to client chat messages"
```

---

### Task 4: Client — Parse channel from incoming ChatMessage

**Files:**
- Modify: `client/src/network/message_handler.rs:461-496`

**Step 1: Read channel field and map to ChatChannel**

Replace the chatMessage handler (lines 461-496) with:

```rust
"chatMessage" => {
    if let Some(value) = data {
        let sender_name = extract_string(value, "senderName").unwrap_or_default();
        let text = extract_string(value, "text").unwrap_or_default();
        let timestamp = extract_u64(value, "timestamp").unwrap_or(0) as f64;
        let channel_str = extract_string(value, "channel").unwrap_or_default();

        let channel = match channel_str.as_str() {
            "global" => ChatChannel::Global,
            "system" => ChatChannel::System,
            _ => ChatChannel::Local, // "public" or unknown defaults to Local
        };

        // Add to chat log
        state.ui_state.chat_messages.push(ChatMessage {
            sender_name: sender_name.clone(),
            text: text.clone(),
            timestamp,
            channel,
        });
        state.pending_sfx.push("message_add".to_string());

        if state.ui_state.chat_messages.len() > 75 {
            state.ui_state.chat_messages.remove(0);
        }

        // Chat bubbles only for public/nearby messages
        if matches!(channel, ChatChannel::Local) {
            if let Some((player_id, _)) = state.players.iter().find(|(_, p)| p.name == sender_name) {
                let player_id = player_id.clone();
                state.chat_bubbles.retain(|b| b.player_id != player_id);
                state.chat_bubbles.push(ChatBubble {
                    player_id,
                    text,
                    time: macroquad::time::get_time(),
                });
            }
        }
    }
}
```

**Step 2: Verify compilation**

Run: `cd client && cargo check 2>&1 | grep "error"`

**Step 3: Commit**

```bash
git add client/src/network/message_handler.rs
git commit -m "feat(chat): parse channel from server messages, bubbles only for public"
```

---

### Task 5: Client — Wire up channel from active tab + ~ prefix

**Files:**
- Modify: `client/src/input/handler.rs:2324-2329` (desktop Enter-send)
- Modify: `client/src/input/handler.rs:868-871` (mobile send button)

**Step 1: Create a helper to determine channel from state**

At each `InputCommand::Chat` push site, determine the channel:

```rust
// Determine channel: ~ prefix forces global, otherwise match active tab
let (send_text, channel) = if text.starts_with('~') {
    (text[1..].trim().to_string(), "global".to_string())
} else {
    let ch = match state.ui_state.chat_active_tab {
        ChatChannel::Global => "global",
        _ => "public",
    };
    (text, ch.to_string())
};
```

Apply this logic at both InputCommand::Chat push sites (desktop at ~line 2329 and mobile at ~line 871). Use `send_text` as the text and `channel` as the channel. Skip sending if `send_text` is empty after stripping `~`.

**Step 2: Block sending on System tab**

At both send sites, add an early return if active tab is System:
```rust
if matches!(state.ui_state.chat_active_tab, ChatChannel::System) {
    // System tab is read-only
    state.ui_state.chat_input.clear();
    return commands;
}
```

**Step 3: Verify compilation**

Run: `cd client && cargo check 2>&1 | grep "error"`

**Step 4: Commit**

```bash
git add client/src/input/handler.rs
git commit -m "feat(chat): send channel based on active tab, ~ prefix for global"
```

---

### Task 6: Desktop UI — Add clickable tabs above chat log

**Files:**
- Modify: `client/src/render/renderer.rs:5249-5330` (chat log rendering)
- Modify: `client/src/ui/mod.rs` or wherever `UiElementId` is defined (add tab element IDs)
- Modify: `client/src/input/handler.rs` (handle tab clicks)

**Step 1: Add UiElementId variants for desktop chat tabs**

Add to the `UiElementId` enum (check if `ChatTabLocal`, `ChatTabGlobal`, `ChatTabSystem` already exist — they're used by mobile panel). If they exist, reuse them. If not, add:
```rust
ChatTabLocalDesktop,
ChatTabGlobalDesktop,
ChatTabSystemDesktop,
```

**Step 2: Render tabs above the chat log**

In `renderer.rs`, inside the `if state.ui_state.chat_log_visible` block, before the message rendering, add tab rendering. Insert just after the `clip_h` calculation (around line 5274):

```rust
// Tab bar above chat log
let tab_h = 18.0 * zoom;
let tab_names = ["Public", "Global", "System"];
let tab_channels = [ChatChannel::Local, ChatChannel::Global, ChatChannel::System];
let tab_ids = [UiElementId::ChatTabLocal, UiElementId::ChatTabGlobal, UiElementId::ChatTabSystem];
let tab_w = (max_chat_width / 3.0).floor();
let tab_bar_y = clip_y - tab_h;

for i in 0..3 {
    let tx = chat_x + i as f32 * tab_w;
    let is_active = std::mem::discriminant(&state.ui_state.chat_active_tab) == std::mem::discriminant(&tab_channels[i]);
    let is_hovered = state.ui_state.hovered_element.as_ref() == Some(&tab_ids[i]);

    let bg = if is_active {
        Color::new(0.15, 0.15, 0.2, 0.85)
    } else if is_hovered {
        Color::new(0.1, 0.1, 0.15, 0.7)
    } else {
        Color::new(0.05, 0.05, 0.08, 0.65)
    };

    draw_rectangle(tx, tab_bar_y, tab_w, tab_h, bg);

    if is_active {
        // Gold underline
        draw_rectangle(tx + 2.0, tab_bar_y + tab_h - 2.0, tab_w - 4.0, 2.0, Color::new(0.76, 0.60, 0.23, 1.0));
    }

    let label_size = 13.0 * zoom;
    let text_w = self.measure_text_sharp(tab_names[i], label_size).width;
    self.draw_text_sharp(
        tab_names[i],
        (tx + (tab_w - text_w) / 2.0).floor(),
        (tab_bar_y + tab_h / 2.0 + label_size * 0.35).floor(),
        label_size,
        if is_active { WHITE } else { Color::new(0.6, 0.6, 0.6, 1.0) },
    );

    layout.add(tab_ids[i].clone(), Rect::new(tx, tab_bar_y, tab_w, tab_h));
}
```

Note: This requires `layout` to be available in this rendering method. Check if the chat log rendering function receives a `layout` parameter. If not, the tabs will need hit-testing handled differently (e.g., via mouse position checks in the input handler, similar to how `chat_log_background` click area works).

**Step 3: Filter chat messages by active tab**

In the chat line building loop (around line 5315), add a filter:

Change:
```rust
for msg in state.ui_state.chat_messages.iter() {
```
to:
```rust
for msg in state.ui_state.chat_messages.iter().filter(|m| {
    std::mem::discriminant(&m.channel) == std::mem::discriminant(&state.ui_state.chat_active_tab)
}) {
```

**Step 4: Include active tab in the cache key**

The chat lines cache needs to invalidate when the tab changes. Add the active tab to the `ChatLinesCacheKey` struct and the `cache_key` construction. Add a field like:
```rust
active_tab: u8, // 0=Local, 1=Global, 2=System
```

And set it from:
```rust
active_tab: match state.ui_state.chat_active_tab {
    ChatChannel::Local => 0,
    ChatChannel::Global => 1,
    ChatChannel::System => 2,
},
```

**Step 5: Handle tab clicks in input handler**

In the input handler where UI clicks are processed, add handlers for the tab UiElementIds:
```rust
UiElementId::ChatTabLocal => {
    state.ui_state.chat_active_tab = ChatChannel::Local;
    state.ui_state.chat_message_scroll = 0.0;
}
UiElementId::ChatTabGlobal => {
    state.ui_state.chat_active_tab = ChatChannel::Global;
    state.ui_state.chat_message_scroll = 0.0;
}
UiElementId::ChatTabSystem => {
    state.ui_state.chat_active_tab = ChatChannel::System;
    state.ui_state.chat_message_scroll = 0.0;
}
```

**Step 6: Verify compilation**

Run: `cd client && cargo check 2>&1 | grep "error"`

**Step 7: Commit**

```bash
git add client/src/render/renderer.rs client/src/input/handler.rs client/src/ui/
git commit -m "feat(chat): add clickable desktop chat tabs with filtering"
```

---

### Task 7: Desktop UI — Channel indicator next to input

**Files:**
- Modify: `client/src/render/renderer.rs` (wherever the chat input field is rendered)

**Step 1: Find chat input rendering**

Search for where the chat input text/cursor is drawn on desktop. Add a channel indicator prefix before the input text:

```rust
if state.ui_state.chat_open && !matches!(state.ui_state.chat_active_tab, ChatChannel::System) {
    let indicator = match state.ui_state.chat_active_tab {
        ChatChannel::Local => "[Public] ",
        ChatChannel::Global => "[Global] ",
        ChatChannel::System => "",
    };
    let indicator_color = match state.ui_state.chat_active_tab {
        ChatChannel::Local => WHITE,
        ChatChannel::Global => SKYBLUE,
        ChatChannel::System => YELLOW,
    };
    // Draw indicator before the input text
    // ... position it at the start of the input line
}
```

If the System tab is active and the input opens, either prevent it from opening or show a "(read-only)" indicator.

**Step 2: Verify compilation and test visually**

Run: `cd client && cargo check 2>&1 | grep "error"`

**Step 3: Commit**

```bash
git add client/src/render/renderer.rs
git commit -m "feat(chat): show channel indicator next to desktop chat input"
```

---

### Task 8: Mobile chat panel — Wire up channel sending

**Files:**
- Modify: `client/src/render/ui/chat_panel.rs` (tab labels)

**Step 1: Update tab labels**

In `client/src/render/ui/chat_panel.rs` line 65, rename "Local" to "Public":

```rust
let tabs = [
    (UiElementId::ChatTabLocal, "Public", ChatChannel::Local),
    (UiElementId::ChatTabGlobal, "Global", ChatChannel::Global),
    (UiElementId::ChatTabSystem, "System", ChatChannel::System),
];
```

The mobile panel already filters messages by active tab (line 126-128) and hides input on System tab (line 102/179-180). The channel field in InputCommand::Chat is already wired up from Task 5 — the mobile send button at handler.rs:871 pushes the same InputCommand, so channel routing is automatic.

**Step 2: Verify compilation**

Run: `cd client && cargo check 2>&1 | grep "error"`

**Step 3: Commit**

```bash
git add client/src/render/ui/chat_panel.rs
git commit -m "feat(chat): rename mobile Local tab to Public"
```

---

### Task 9: Build and manual test

**Step 1: Build server**

Run: `cd rust-server && cargo build 2>&1 | tail -5`
Expected: compiles with warnings only (no errors).

**Step 2: Build client**

Run: `cd client && cargo build 2>&1 | tail -5`
Expected: compiles with warnings only (no errors).

**Step 3: Manual test checklist**

- [ ] Start server, connect two clients
- [ ] Default tab is Public — messages sent only to nearby players
- [ ] Switch to Global tab — messages broadcast to all
- [ ] Type `~hello` on Public tab — message appears in Global channel for all
- [ ] System messages (e.g., XP gains) appear only in System tab
- [ ] Chat bubbles appear only for Public messages
- [ ] Desktop tabs are clickable, filter correctly
- [ ] Mobile tabs work, input hidden on System tab
- [ ] Scrolling works per-tab (resets on tab switch)

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: chat tab system with public/global/system channels"
```
