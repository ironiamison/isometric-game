# NPC Speech Bubbles Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** NPCs randomly say messages when players are nearby, broadcasting speech bubbles to all players within range.

**Architecture:** Add a `[speech]` block to NPC TOML prototypes. Server tracks a per-NPC speech timer that only ticks when players are within radius. On fire, sends an `NpcSpeech` message to nearby players. Client reuses existing chat bubble rendering.

**Tech Stack:** Rust (server + client), TOML configs, MessagePack protocol, Macroquad rendering

---

### Task 1: Add Speech Config to TOML Parsing

**Files:**
- Modify: `rust-server/src/entity/prototype.rs:72-90` (RawEntityBehaviors — but we add a new struct)
- Modify: `rust-server/src/entity/prototype.rs:128-151` (RawEntityPrototype)
- Modify: `rust-server/src/entity/prototype.rs:253-268` (EntityPrototype)
- Modify: `rust-server/src/entity/registry.rs:221-242` (resolve_prototype)

**Step 1: Add RawSpeechConfig and SpeechConfig structs**

In `rust-server/src/entity/prototype.rs`, add near the top (after the existing config structs, around line 90):

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct RawSpeechConfig {
    #[serde(default = "default_speech_radius")]
    pub radius: i32,
    #[serde(default = "default_speech_interval_min")]
    pub interval_min_ms: u64,
    #[serde(default = "default_speech_interval_max")]
    pub interval_max_ms: u64,
    #[serde(default)]
    pub messages: Vec<String>,
}

fn default_speech_radius() -> i32 { 5 }
fn default_speech_interval_min() -> u64 { 15000 }
fn default_speech_interval_max() -> u64 { 45000 }

#[derive(Debug, Clone)]
pub struct SpeechConfig {
    pub radius: i32,
    pub interval_min_ms: u64,
    pub interval_max_ms: u64,
    pub messages: Vec<String>,
}

impl From<&RawSpeechConfig> for SpeechConfig {
    fn from(raw: &RawSpeechConfig) -> Self {
        Self {
            radius: raw.radius,
            interval_min_ms: raw.interval_min_ms,
            interval_max_ms: raw.interval_max_ms,
            messages: raw.messages.clone(),
        }
    }
}
```

**Step 2: Add speech field to RawEntityPrototype**

In `rust-server/src/entity/prototype.rs`, add to `RawEntityPrototype` (after `dialogue` field, line ~150):

```rust
    pub speech: Option<RawSpeechConfig>,
```

**Step 3: Add speech field to EntityPrototype**

In `rust-server/src/entity/prototype.rs`, add to `EntityPrototype` (after `dialogue` field, line ~267):

```rust
    pub speech: Option<SpeechConfig>,
```

**Step 4: Wire up in registry resolver**

In `rust-server/src/entity/registry.rs`, inside `resolve_prototype()`, add after `dialogue` (line ~241):

```rust
            speech: raw.speech.as_ref().map(SpeechConfig::from)
                .or_else(|| parent.and_then(|p| p.speech.clone())),
```

**Step 5: Commit**

```bash
git add rust-server/src/entity/prototype.rs rust-server/src/entity/registry.rs
git commit -m "feat: add speech config to entity prototype TOML parsing"
```

---

### Task 2: Add Speech State to Server NPC

**Files:**
- Modify: `rust-server/src/npc.rs:24-44` (PrototypeStats)
- Modify: `rust-server/src/npc.rs:46-75` (Npc struct)
- Modify: `rust-server/src/npc.rs:79-130` (from_prototype)

**Step 1: Add speech fields to Npc struct**

In `rust-server/src/npc.rs`, add to `Npc` struct (after `last_regen_time`, line ~74):

```rust
    /// Speech bubble config (None = NPC never speaks)
    pub speech_messages: Option<Vec<String>>,
    pub speech_radius: i32,
    pub speech_interval_min_ms: u64,
    pub speech_interval_max_ms: u64,
    /// Timestamp when this NPC should next speak
    pub next_speech_at: u64,
```

**Step 2: Initialize in from_prototype**

In `rust-server/src/npc.rs`, inside `from_prototype`, add to the `Self { ... }` block (after `last_regen_time: 0`, line ~127):

```rust
            speech_messages: prototype.speech.as_ref().map(|s| s.messages.clone()),
            speech_radius: prototype.speech.as_ref().map(|s| s.radius).unwrap_or(0),
            speech_interval_min_ms: prototype.speech.as_ref().map(|s| s.interval_min_ms).unwrap_or(15000),
            speech_interval_max_ms: prototype.speech.as_ref().map(|s| s.interval_max_ms).unwrap_or(45000),
            next_speech_at: 0,
```

**Step 3: Commit**

```bash
git add rust-server/src/npc.rs
git commit -m "feat: add speech state fields to server NPC struct"
```

---

### Task 3: Add NpcSpeech Server Message

**Files:**
- Modify: `rust-server/src/protocol.rs:100-347` (ServerMessage enum)
- Modify: `rust-server/src/protocol.rs:482-530` (msg_type match)

**Step 1: Add NpcSpeech variant to ServerMessage enum**

In `rust-server/src/protocol.rs`, add a new variant to `ServerMessage` (after `Announcement`, around line ~340):

```rust
    NpcSpeech {
        npc_id: String,
        message: String,
    },
```

**Step 2: Add msg_type match arm**

In `rust-server/src/protocol.rs`, add to the `msg_type()` match (after the `Announcement` arm):

```rust
            ServerMessage::NpcSpeech { .. } => "npcSpeech",
```

**Step 3: Commit**

```bash
git add rust-server/src/protocol.rs
git commit -m "feat: add NpcSpeech server message variant"
```

---

### Task 4: Add Speech Logic to Server Game Loop

**Files:**
- Modify: `rust-server/src/game.rs:3608-3656` (NPC tick loop)

**Step 1: Add speech check in NPC tick loop**

In `rust-server/src/game.rs`, after the NPC AI update block (after `npc.apply_regen(current_time);`, line ~3651), and before `npc_updates.push(...)`, add:

```rust
                // Check NPC speech
                if let Some(ref messages) = npc.speech_messages {
                    if !messages.is_empty() && npc.is_alive() {
                        // Check if any player is within speech radius
                        let has_nearby_player = player_positions.iter().any(|(_, px, py, _)| {
                            let dx = (npc.x - px).abs();
                            let dy = (npc.y - py).abs();
                            dx.max(dy) <= npc.speech_radius
                        });

                        if has_nearby_player {
                            if npc.next_speech_at == 0 {
                                // First time a player is nearby — set initial timer
                                let delay = npc.speech_interval_min_ms
                                    + (rand::random::<u64>() % (npc.speech_interval_max_ms - npc.speech_interval_min_ms + 1));
                                npc.next_speech_at = current_time + delay;
                            } else if current_time >= npc.next_speech_at {
                                // Time to speak!
                                let idx = rand::random::<usize>() % messages.len();
                                let message = messages[idx].clone();
                                let npc_id = npc.id.clone();
                                let radius = npc.speech_radius;
                                let npc_x = npc.x;
                                let npc_y = npc.y;

                                // Collect nearby player IDs to send speech to
                                for (pid, px, py, _) in &player_positions {
                                    let dx = (npc_x - px).abs();
                                    let dy = (npc_y - py).abs();
                                    if dx.max(dy) <= radius {
                                        npc_speech_events.push((pid.clone(), npc_id.clone(), message.clone()));
                                    }
                                }

                                // Reset timer
                                let delay = npc.speech_interval_min_ms
                                    + (rand::random::<u64>() % (npc.speech_interval_max_ms - npc.speech_interval_min_ms + 1));
                                npc.next_speech_at = current_time + delay;
                            }
                        } else {
                            // No players nearby — reset timer
                            npc.next_speech_at = 0;
                        }
                    }
                }
```

**Step 2: Declare the speech events vec and send messages**

Before the NPC loop (around line 3607, alongside existing `npc_attacks` vec):

```rust
        let mut npc_speech_events: Vec<(String, String, String)> = Vec::new(); // (player_id, npc_id, message)
```

After the NPC block closes (after line ~3656), add sending logic:

```rust
        // Send NPC speech bubbles to nearby players
        for (player_id, npc_id, message) in npc_speech_events {
            self.send_to_player(&player_id, ServerMessage::NpcSpeech {
                npc_id: npc_id.clone(),
                message: message.clone(),
            }).await;
        }
```

**Step 3: Commit**

```bash
git add rust-server/src/game.rs
git commit -m "feat: add NPC speech timer and broadcast in game tick"
```

---

### Task 5: Add Speech Bubble to Client NPC Struct

**Files:**
- Modify: `client/src/game/npc.rs:34-70` (Npc struct)
- Modify: `client/src/game/npc.rs:72+` (Npc::new)

**Step 1: Add speech_bubble field to client Npc**

In `client/src/game/npc.rs`, add to the Npc struct (after `pending_death`, line ~69):

```rust
    /// Speech bubble text and timestamp
    pub speech_bubble: Option<(String, f64)>,
```

**Step 2: Initialize in Npc::new**

In `client/src/game/npc.rs`, add to the `Self { ... }` in `new()`:

```rust
            speech_bubble: None,
```

**Step 3: Commit**

```bash
git add client/src/game/npc.rs
git commit -m "feat: add speech_bubble field to client NPC struct"
```

---

### Task 6: Handle NpcSpeech Message on Client

**Files:**
- Modify: `client/src/network/message_handler.rs` (add handler near chatMessage handler, line ~367)

**Step 1: Add npcSpeech handler**

In `client/src/network/message_handler.rs`, add a new match arm (after the `chatMessage` handler block):

```rust
        "npcSpeech" => {
            if let Some(value) = data {
                let npc_id = extract_string(value, "npc_id").unwrap_or_default();
                let message = extract_string(value, "message").unwrap_or_default();

                if let Some(npc) = state.npcs.get_mut(&npc_id) {
                    npc.speech_bubble = Some((message, macroquad::time::get_time()));
                }
            }
        }
```

**Step 2: Commit**

```bash
git add client/src/network/message_handler.rs
git commit -m "feat: handle NpcSpeech message on client"
```

---

### Task 7: Render NPC Speech Bubbles

**Files:**
- Modify: `client/src/render/renderer.rs:1052-1179` (render_chat_bubbles function)

**Step 1: Add NPC speech bubble rendering**

In `client/src/render/renderer.rs`, at the end of `render_chat_bubbles()` (after the player bubble loop closes at line ~1178, before the closing `}`), add:

```rust
        // Render NPC speech bubbles
        for npc in state.npcs.values() {
            let Some((ref text, time)) = npc.speech_bubble else {
                continue;
            };

            let age = (current_time - time) as f32;
            if age > 5.0 {
                continue;
            }

            // Get NPC screen position
            let (screen_x, screen_y) = world_to_screen(npc.x, npc.y, &state.camera);

            // Fade out in the last 1 second (age 4-5)
            let alpha = if age > 4.0 {
                ((5.0 - age) * 255.0) as u8
            } else {
                255
            };

            // Word wrap the text (same params as player bubbles)
            let max_bubble_width = 220.0;
            let font_size = 16.0;
            let line_height = 18.0;
            let padding_h = 4.0;
            let padding_v = 1.0;
            let tail_height = 6.0;
            let corner_radius = 5.0;

            let lines = self.wrap_text(text, max_bubble_width - padding_h * 2.0, font_size);
            let num_lines = lines.len().max(1);

            let mut max_line_width = 0.0f32;
            for line in &lines {
                let width = self.measure_text_sharp(line, font_size).width;
                max_line_width = max_line_width.max(width);
            }

            let bubble_width = (max_line_width + padding_h * 2.0).max(18.0);
            let bubble_height = num_lines as f32 * line_height + padding_v * 2.0;

            // Position bubble above NPC's head
            let zoom = state.camera.zoom;
            let base_offset = (SPRITE_HEIGHT - 8.0) * zoom;

            let is_hovered = state.hovered_entity_id.as_ref() == Some(&npc.id);
            let is_selected = state.selected_entity_id.as_ref() == Some(&npc.id);
            let name_offset = if is_hovered || is_selected { 16.0 } else { 0.0 };

            let bubble_x = screen_x - bubble_width / 2.0;
            let bubble_y = screen_y - base_offset - name_offset - bubble_height - tail_height;

            // Colors with alpha - off-white paper/comic book style
            let bg_alpha = (alpha as f32 * 0.8) as u8;
            let bg_color = Color::from_rgba(255, 250, 240, bg_alpha);
            let border_color = Color::from_rgba(60, 50, 40, alpha);
            let text_color = Color::from_rgba(30, 25, 20, alpha);

            let r = corner_radius;
            let bx = bubble_x.floor();
            let by = bubble_y.floor();
            let bw = bubble_width.floor();
            let bh = bubble_height.floor();

            let border_mesh = Self::create_rounded_rect_mesh(bx - 1.0, by - 1.0, bw + 2.0, bh + 2.0, r + 1.0, border_color);
            draw_mesh(&border_mesh);

            let fill_mesh = Self::create_rounded_rect_mesh(bx, by, bw, bh, r, bg_color);
            draw_mesh(&fill_mesh);

            // Draw tail
            let tail_x = screen_x.floor();
            let tail_top_y = by + bh;
            let tail_bottom_y = tail_top_y + tail_height;
            let tail_half_width = 4.0;

            draw_triangle(
                Vec2::new(tail_x - tail_half_width - 1.0, tail_top_y),
                Vec2::new(tail_x + tail_half_width + 1.0, tail_top_y),
                Vec2::new(tail_x, tail_bottom_y + 1.0),
                border_color,
            );

            let tail_color_arr = [
                (bg_color.r * 255.0) as u8,
                (bg_color.g * 255.0) as u8,
                (bg_color.b * 255.0) as u8,
                (bg_color.a * 255.0) as u8,
            ];
            let tail_mesh = Mesh {
                vertices: vec![
                    Vertex { position: Vec3::new(tail_x - tail_half_width, tail_top_y, 0.0), uv: Vec2::ZERO, color: tail_color_arr, normal: Vec4::ZERO },
                    Vertex { position: Vec3::new(tail_x + tail_half_width, tail_top_y, 0.0), uv: Vec2::ZERO, color: tail_color_arr, normal: Vec4::ZERO },
                    Vertex { position: Vec3::new(tail_x, tail_bottom_y, 0.0), uv: Vec2::ZERO, color: tail_color_arr, normal: Vec4::ZERO },
                ],
                indices: vec![0, 1, 2],
                texture: None,
            };
            draw_mesh(&tail_mesh);

            draw_line(tail_x - tail_half_width, tail_top_y, tail_x, tail_bottom_y, 1.0, border_color);
            draw_line(tail_x + tail_half_width, tail_top_y, tail_x, tail_bottom_y, 1.0, border_color);

            // Draw text lines (centered)
            let bubble_center_x = bx + bw / 2.0;
            let mut text_y = by + padding_v + font_size * 0.85;

            for line in &lines {
                let line_width = self.measure_text_sharp(line, font_size).width;
                let text_x = bubble_center_x - line_width / 2.0;
                self.draw_text_sharp(line, text_x, text_y, font_size, text_color);
                text_y += line_height;
            }
        }
```

**Step 2: Commit**

```bash
git add client/src/render/renderer.rs
git commit -m "feat: render NPC speech bubbles using existing bubble system"
```

---

### Task 8: Add Speech Data to NPC TOML Files

**Files:**
- Modify: `rust-server/data/entities/npcs/villagers.toml`

**Step 1: Add speech config to Elder Mara**

Add a `[elder_villager.speech]` block:

```toml
[elder_villager.speech]
radius = 5
interval_min_ms = 20000
interval_max_ms = 40000
messages = [
    "The old forest isn't what it used to be...",
    "I remember when this village was just a few huts.",
    "Adventurer, I could use your help.",
    "The corruption came without warning...",
    "Stay vigilant. These lands are not safe.",
]
```

Add speech to any other NPCs defined in the same file or other NPC TOML files as appropriate.

**Step 2: Commit**

```bash
git add rust-server/data/entities/npcs/
git commit -m "feat: add speech bubble messages to villager NPC definitions"
```

---

### Task 9: Build and Verify

**Step 1: Build the server**

```bash
cd rust-server && cargo build 2>&1
```

Expected: Compiles without errors.

**Step 2: Build the client**

```bash
cd client && cargo build 2>&1
```

Expected: Compiles without errors.

**Step 3: Fix any compilation errors**

Address any type mismatches, missing imports, or field initialization issues.

**Step 4: Commit any fixes**

```bash
git add -A && git commit -m "fix: resolve compilation issues for NPC speech bubbles"
```
