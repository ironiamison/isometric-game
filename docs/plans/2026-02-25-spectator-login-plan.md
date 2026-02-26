# Spectator Login Screen Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the starry login background with a live spectator view of the game world, with seamless WebSocket upgrade to authenticated gameplay.

**Architecture:** Client connects to a new `/spectate` WebSocket endpoint on launch (no auth). Server streams StateSync + chunks around spawn (WORLD_SPAWN_X=15, WORLD_SPAWN_Y=4). The client renders the world behind the login form with a slow cinematic camera drift. On login, the client matchmakes via HTTP, then sends a SpectatorUpgrade message over the existing WS to transition to an authenticated player — same connection, no reconnect.

**Tech Stack:** Rust server (Axum/Tokio), Rust client (Macroquad), MessagePack protocol, ewebsock

**Scope:** Native builds only (`#[cfg(not(target_arch = "wasm32"))]`). WASM build is unchanged — JavaScript handles matchmaking before WASM loads.

---

### Task 1: Protocol — Add SpectatorUpgrade Client Message

**Files:**
- Modify: `rust-server/src/protocol.rs` (ClientMessage enum, ~line 12)
- Modify: `client/src/network/messages.rs` (ClientMessage enum)
- Modify: `client/src/network/protocol.rs` (encoding)

**Context:** Both server and client have a `ClientMessage` enum. The server decodes incoming client messages via `decode_client_message()` using MessagePack. The client encodes them via `encode_client_message()`. We need a new variant the spectator can send to upgrade its connection.

**Step 1: Add SpectatorUpgrade to server ClientMessage**

In `rust-server/src/protocol.rs`, add to the `ClientMessage` enum:

```rust
#[serde(rename = "spectatorUpgrade")]
SpectatorUpgrade { session_token: String },
```

Also find `decode_client_message()` and ensure it handles the new variant. The existing MessagePack decoder extracts the type string and matches on it — add a `"spectatorUpgrade"` arm that extracts `session_token` from the map.

**Step 2: Add SpectatorUpgrade to client ClientMessage**

In `client/src/network/messages.rs`, add the same variant to the client's `ClientMessage` enum.

In `client/src/network/protocol.rs`, add encoding for `SpectatorUpgrade` in `encode_client_message()` — encode as MessagePack array `[type_id, "spectatorUpgrade", {"sessionToken": token}]`.

**Step 3: Verify both server and client compile**

```bash
cd rust-server && cargo check 2>&1 | tail -5
cd client && cargo check 2>&1 | tail -5
```

**Step 4: Commit**

```bash
git add -A && git commit -m "feat: add SpectatorUpgrade client message to protocol"
```

---

### Task 2: Server — Spectator Tracking in GameRoom

**Files:**
- Modify: `rust-server/src/game.rs` — GameRoom struct (~line 841), `impl GameRoom` (~line 920)

**Context:** GameRoom tracks players via `players: RwLock<HashMap<String, Player>>` and their message channels via `player_senders: RwLock<HashMap<String, mpsc::Sender<Vec<u8>>>>`. Spectators need similar channel tracking but are NOT players — they have no game entity.

**Step 1: Add spectator_senders field to GameRoom**

In the `GameRoom` struct (game.rs ~line 841), add:

```rust
/// Per-spectator message senders (read-only viewers on login screen)
spectator_senders: RwLock<HashMap<String, mpsc::Sender<Vec<u8>>>>,
```

In `GameRoom::new()` (~line 921), initialize it:

```rust
spectator_senders: RwLock::new(HashMap::new()),
```

**Step 2: Add spectator management methods**

Add these methods to `impl GameRoom`:

```rust
/// Register a spectator's message channel. Returns the spectator ID.
pub async fn add_spectator(&self, spectator_id: &str, sender: mpsc::Sender<Vec<u8>>) {
    self.spectator_senders.write().await.insert(spectator_id.to_string(), sender);
}

/// Remove a spectator's message channel.
pub async fn remove_spectator(&self, spectator_id: &str) {
    self.spectator_senders.write().await.remove(spectator_id);
}

/// Get current spectator count.
pub async fn spectator_count(&self) -> usize {
    self.spectator_senders.read().await.len()
}
```

**Step 3: Verify server compiles**

```bash
cd rust-server && cargo check 2>&1 | tail -5
```

**Step 4: Commit**

```bash
git add -A && git commit -m "feat: add spectator tracking to GameRoom"
```

---

### Task 3: Server — Spectator StateSync in Tick Loop

**Files:**
- Modify: `rust-server/src/game.rs` — tick function (search for the overworld StateSync generation section, ~line 14140)

**Context:** The tick loop generates per-player StateSyncs with VIEW_DISTANCE=40 tile culling. For spectators, we generate ONE StateSync centered on the world spawn point and send it to all spectators via their mpsc channels. This is efficient — one encode for all spectators.

**Step 1: Add spectator StateSync generation after overworld player StateSync**

Find the section in the tick function where overworld StateSync is generated and sent to players (after the per-player encoding loop). After that section, add spectator StateSync:

```rust
// === Spectator StateSync ===
// Generate a single StateSync for all spectators, centered on world spawn
let spectator_senders = self.spectator_senders.read().await;
if !spectator_senders.is_empty() {
    // Gather players near spawn with VIEW_DISTANCE culling
    let mut spectator_player_values = Vec::new();
    for p in players_snapshot.values() {
        if !p.active { continue; }
        let dx = (p.x - WORLD_SPAWN_X).abs();
        let dy = (p.y - WORLD_SPAWN_Y).abs();
        if dx <= VIEW_DISTANCE && dy <= VIEW_DISTANCE {
            spectator_player_values.push(p.to_sync_value());
        }
    }

    // Gather NPCs near spawn
    let mut spectator_npc_values = Vec::new();
    for n in npcs_snapshot.values() {
        let dx = (n.x - WORLD_SPAWN_X).abs();
        let dy = (n.y - WORLD_SPAWN_Y).abs();
        if dx <= VIEW_DISTANCE && dy <= VIEW_DISTANCE {
            spectator_npc_values.push(n.to_sync_value());
        }
    }

    // Encode once for all spectators (always full sync, no delta tracking)
    if let Ok(bytes) = crate::protocol::encode_state_sync_from_values(
        current_tick,
        spectator_player_values,
        spectator_npc_values,
        "",
    ) {
        for sender in spectator_senders.values() {
            let _ = sender.try_send(bytes.clone());
        }
    }
}
drop(spectator_senders);
```

Note: Use the existing `players_snapshot` and `npcs_snapshot` variables that should already exist in the tick function scope. Adapt variable names to match what's actually in scope. The key is: filter by distance from WORLD_SPAWN_X/Y, encode once, send to all spectators.

**Step 2: Verify server compiles**

```bash
cd rust-server && cargo check 2>&1 | tail -5
```

**Step 3: Commit**

```bash
git add -A && git commit -m "feat: add spectator StateSync generation to tick loop"
```

---

### Task 4: Server — /spectate WebSocket Endpoint

**Files:**
- Modify: `rust-server/src/main.rs` — router setup, add `spectate_handler` and `handle_spectator` functions

**Context:** The server uses Axum. The existing router is set up in `main()` with routes like `.route("/ws/:room_id", ...)`. We add a new `/spectate` route that accepts WebSocket connections without authentication. The spectator handler sends initial chunks around spawn, then runs send/recv loops. The recv loop ignores all messages except `SpectatorUpgrade`.

**Step 1: Add the /spectate route to the Axum router**

Find where routes are defined in main.rs and add:

```rust
.route("/spectate", get(spectate_handler))
```

**Step 2: Add spectate_handler function**

```rust
const MAX_SPECTATORS: usize = 50;

async fn spectate_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Get or create the main game room
    let room = state.get_or_create_room("game_room").await;

    // Rate limit spectators
    if room.spectator_count().await >= MAX_SPECTATORS {
        return (StatusCode::SERVICE_UNAVAILABLE, "Too many spectators").into_response();
    }

    ws.on_upgrade(move |socket| handle_spectator(socket, state, room))
        .into_response()
}
```

**Step 3: Add handle_spectator function**

This is the main spectator handler. Model it after `handle_socket` but much simpler:

```rust
async fn handle_spectator(
    socket: WebSocket,
    state: AppState,
    room: Arc<GameRoom>,
) {
    let (mut sender, mut receiver) = socket.split();
    let spectator_id = Uuid::new_v4().to_string();

    // Create mpsc channel for this spectator
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(256);

    // Register as spectator
    room.add_spectator(&spectator_id, tx.clone()).await;

    // Send initial chunks around spawn (5x5 grid)
    let spawn_chunk = chunk::ChunkCoord::from_world(WORLD_SPAWN_X, WORLD_SPAWN_Y);
    for dy in -2..=2 {
        for dx in -2..=2 {
            let coord = chunk::ChunkCoord::new(spawn_chunk.x + dx, spawn_chunk.y + dy);
            if let Some(chunk_msg) = room.handle_chunk_request(coord.x, coord.y).await {
                if let Ok(bytes) = protocol::encode_server_message(&chunk_msg) {
                    let _ = sender.send(Message::Binary(bytes)).await;
                }
            }
        }
    }

    // Subscribe to room broadcasts (for cross-cutting events)
    let mut broadcast_rx = room.subscribe();

    // Send loop: forward mpsc (StateSync) + broadcast to WebSocket
    let send_spectator_id = spectator_id.clone();
    let mut send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                Some(msg) = rx.recv() => {
                    if sender.send(Message::Binary(msg)).await.is_err() { break; }
                }
                result = broadcast_rx.recv() => {
                    match result {
                        Ok(bytes) => {
                            if sender.send(Message::Binary(bytes)).await.is_err() { break; }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                else => break,
            }
        }
    });

    // Recv loop: ignore everything except SpectatorUpgrade
    let recv_room = room.clone();
    let recv_state = state.clone();
    let recv_spectator_id = spectator_id.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Binary(data) => {
                    // Try to decode as client message
                    if let Ok(client_msg) = protocol::decode_client_message(&data) {
                        if let ClientMessage::SpectatorUpgrade { session_token } = client_msg {
                            // Attempt upgrade — handle in Task 5
                            handle_spectator_upgrade(
                                &recv_state,
                                &recv_room,
                                &recv_spectator_id,
                                &session_token,
                                tx.clone(),
                                &mut receiver,
                            ).await;
                            return; // Upgrade handler takes over the recv loop
                        }
                    }
                    // All other messages: silently ignored
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    // Cleanup
    room.remove_spectator(&spectator_id).await;
    info!("Spectator {} disconnected", spectator_id);
}
```

Note: The `handle_spectator_upgrade` function is stubbed here and implemented in Task 5. For now, you can create an empty async fn that just logs "upgrade not yet implemented" so the code compiles.

Also note: You'll need to import `WORLD_SPAWN_X` and `WORLD_SPAWN_Y` from `game.rs` — make them `pub const` if they aren't already.

**Step 4: Verify server compiles**

```bash
cd rust-server && cargo check 2>&1 | tail -5
```

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: add /spectate WebSocket endpoint for login screen world view"
```

---

### Task 5: Server — Session Upgrade Handler

**Files:**
- Modify: `rust-server/src/main.rs` — add `handle_spectator_upgrade` function, extract welcome/init helper

**Context:** When a spectator sends `SpectatorUpgrade { session_token }`, the server must:
1. Validate the session token (same as `ws_handler` does)
2. Look up the session (player_id, character data, etc.)
3. Remove spectator registration, register as player
4. Activate the player entity
5. Send welcome message + all definitions + initial data
6. Switch the recv loop to handle normal client messages
7. On disconnect, run full player cleanup (save character data, etc.)

The welcome/definitions sending logic is currently inline in `handle_socket` (lines 1870-2120). Extract the common parts into a helper function that both `handle_socket` and `handle_spectator_upgrade` can call.

**Step 1: Extract a `send_initial_data` helper**

Create a helper function that sends welcome + definitions + initial game data. This extracts the common code from `handle_socket` (lines 1870-2114):

```rust
async fn send_initial_data(
    sender: &mpsc::Sender<Vec<u8>>,
    state: &AppState,
    room: &GameRoom,
    player_id: &str,
    is_new_character: bool,
) {
    // Helper to send via mpsc
    let send = |bytes: Vec<u8>| {
        let _ = sender.try_send(bytes);
    };

    // Welcome message
    let welcome = ServerMessage::Welcome {
        player_id: player_id.to_string(),
        is_new_character,
    };
    if let Ok(bytes) = protocol::encode_server_message(&welcome) { send(bytes); }

    // Entity definitions
    let entity_defs = room.get_entity_definitions();
    if let Ok(bytes) = protocol::encode_server_message(&entity_defs) { send(bytes); }

    // Item definitions
    let item_defs = state.item_registry.to_client_definitions();
    if let Ok(bytes) = protocol::encode_server_message(&item_defs) { send(bytes); }

    // Recipe definitions
    let recipe_defs = state.crafting_registry.to_client_definitions();
    if let Ok(bytes) = protocol::encode_server_message(&recipe_defs) { send(bytes); }

    // ... continue with all the other definition sends from handle_socket
    // (discovered recipes, scroll spells, unlocked spells, gathering markers,
    //  farming patches, farming contract, chairs, chests, prayer state,
    //  initial chunks, existing players, ground items, player joined broadcast,
    //  quest state, inventory, skills, slayer state, friends)
}
```

Note: This is a large extraction. Copy the sends from `handle_socket` lines 1870-2127 and adapt them to use the mpsc sender instead of the direct WebSocket sender. The mpsc sender is already registered as the player's sender by this point, so `send_to_player` would also work for some messages, but for the initial burst it's simpler to send directly via the channel.

Alternatively, keep `handle_socket` as-is and only extract what `handle_spectator_upgrade` needs. The key messages the upgrade handler MUST send are: Welcome, EntityDefs, ItemDefs, RecipeDefs, DiscoveredRecipes, ScrollSpellDefs, UnlockedSpells, GatheringMarkers, FarmingPatches, ChairPositions, ChestPositions, PrayerState, initial chunks, existing players, ground items, PlayerJoined, quest state, inventory, skills, slayer state.

**Step 2: Implement handle_spectator_upgrade**

```rust
async fn handle_spectator_upgrade(
    state: &AppState,
    room: &GameRoom,
    spectator_id: &str,
    session_token: &str,
    tx: mpsc::Sender<Vec<u8>>,
    receiver: &mut SplitStream<WebSocket>,
) {
    // 1. Validate session token (same as ws_handler)
    let session_id = match state.token_signer.validate_token(session_token) {
        Some((sid, _rid)) => sid,
        None => {
            warn!("Spectator upgrade rejected: invalid session token");
            return;
        }
    };

    // 2. Look up session
    let session = match state.sessions.get(&session_id) {
        Some(s) => s.clone(),
        None => {
            warn!("Spectator upgrade rejected: session not found");
            return;
        }
    };

    // 3. Verify auth token still valid
    if !state.auth_sessions.contains_key(&session.auth_token) {
        warn!("Spectator upgrade rejected: auth token expired");
        return;
    }

    let player_id = session.player_id.clone();
    let character_name = session.character_name.clone();
    let character_id = session.character_id;
    let is_new_character = session.is_new_character;

    // 4. Remove spectator, register as player
    room.remove_spectator(spectator_id).await;
    room.register_player_sender(&player_id, tx.clone()).await;

    // 5. Activate player
    let player_name = room.activate_player(&player_id).await;
    info!("Spectator upgraded to player {} ({})", player_name, player_id);

    // 6. Send all initial data (welcome, definitions, chunks, etc.)
    send_initial_data(&tx, state, room, &player_id, is_new_character).await;

    // 7. Broadcast PlayerJoined to other overworld players
    let (x, y) = room.get_player_position(&player_id).await.unwrap_or((0, 0));
    let (gender, skin) = room.get_player_appearance(&player_id).await
        .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
    let (hair_style, hair_color) = room.get_player_hair(&player_id).await.unwrap_or((None, None));
    let player_joined_msg = ServerMessage::PlayerJoined {
        id: player_id.clone(), name: player_name.clone(), x, y,
        gender, skin, hair_style, hair_color,
    };
    room.send_to_overworld_players(player_joined_msg, Some(&player_id)).await;

    // 8. Send friends data + notify friends online
    room.send_friends_data(&player_id, &state.online_characters).await;
    room.broadcast_friend_status(&player_id, true).await;

    // 9. Handle instance reconnect if needed
    if let Some(ref map_id) = session.current_map {
        auto_enter_instance(state, room, &player_id, map_id, session.entrance_x, session.entrance_y).await;
    }

    // 10. Switch to normal message handling loop
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Binary(data) => {
                if let Err(e) = handle_client_message(state, room, &player_id, &data).await {
                    warn!("Error handling message: {}", e);
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // 11. Cleanup — save character data (same as handle_socket cleanup)
    info!("Player {} ({}) disconnected (was spectator upgrade)", character_name, player_id);

    // Save character data - replicate the cleanup from handle_socket (lines 2215-2391)
    let should_save = state.sessions.get(&session_id)
        .map(|s| state.auth_sessions.contains_key(&s.auth_token))
        .unwrap_or(false);

    if should_save {
        let played_time_delta = state.play_time_anchors
            .remove(&character_id)
            .map(|(_, anchor)| anchor.elapsed().as_secs() as i64)
            .unwrap_or(0);

        if let Some(save_data) = room.get_player_save_data(&player_id).await {
            // Save to DB (replicate handle_socket's save logic)
            // ... copy the save_character call and related cleanup
        }
    }

    // Remove player from room and notify others
    room.remove_player(&player_id).await;
    room.unregister_player_sender(&player_id).await;
    // ... broadcast PlayerLeft, cleanup session, etc.
}
```

Note: The cleanup section (step 11) should replicate `handle_socket`'s disconnect cleanup (lines 2215-2391). Consider extracting this into a shared helper too, to avoid duplication. At minimum, extract the character save + cleanup into `cleanup_player_session()`.

**Step 3: Verify server compiles**

```bash
cd rust-server && cargo check 2>&1 | tail -5
```

**Step 4: Commit**

```bash
git add -A && git commit -m "feat: implement spectator-to-player session upgrade"
```

---

### Task 6: Client — SpectatorCamera

**Files:**
- Create: `client/src/game/spectator_camera.rs`
- Modify: `client/src/game/mod.rs` — add module declaration

**Context:** The spectator camera drifts slowly through waypoints near the world spawn (15, 4). The game uses isometric projection with TILE_WIDTH=64, TILE_HEIGHT=32. Camera x/y are in world tile coordinates (floats). The existing Camera struct (state.rs ~line 258) has x, y, zoom, initialized fields.

**Step 1: Create SpectatorCamera struct**

```rust
/// Cinematic camera that drifts between waypoints for the login screen spectator view.
pub struct SpectatorCamera {
    waypoints: Vec<(f32, f32)>,
    current_index: usize,
    progress: f32,       // 0.0 to 1.0 between current and next waypoint
    speed: f32,          // Progress per second (lower = slower drift)
}

impl SpectatorCamera {
    pub fn new() -> Self {
        // Gentle loop around spawn (15, 4) — radius ~12 tiles
        // These waypoints form a smooth circuit through the spawn area
        let waypoints = vec![
            (15.0, 4.0),    // Start at spawn
            (22.0, 8.0),    // Southeast
            (18.0, 14.0),   // South
            (10.0, 10.0),   // Southwest
            (6.0, 4.0),     // West
            (10.0, -2.0),   // Northwest
            (18.0, -1.0),   // North
        ];
        Self {
            waypoints,
            current_index: 0,
            progress: 0.0,
            speed: 0.04, // ~25 seconds per segment
        }
    }

    /// Advance camera and return current (x, y) position.
    pub fn update(&mut self, dt: f32) -> (f32, f32) {
        self.progress += self.speed * dt;

        if self.progress >= 1.0 {
            self.progress -= 1.0;
            self.current_index = (self.current_index + 1) % self.waypoints.len();
        }

        let (x0, y0) = self.waypoints[self.current_index];
        let next = (self.current_index + 1) % self.waypoints.len();
        let (x1, y1) = self.waypoints[next];

        // Smooth interpolation (ease in-out)
        let t = smooth_step(self.progress);
        let x = x0 + (x1 - x0) * t;
        let y = y0 + (y1 - y0) * t;

        (x, y)
    }

    /// Get current position without advancing.
    pub fn position(&self) -> (f32, f32) {
        let (x0, y0) = self.waypoints[self.current_index];
        let next = (self.current_index + 1) % self.waypoints.len();
        let (x1, y1) = self.waypoints[next];
        let t = smooth_step(self.progress);
        (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
    }
}

fn smooth_step(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
```

**Step 2: Add module to game/mod.rs**

```rust
pub mod spectator_camera;
pub use spectator_camera::SpectatorCamera;
```

**Step 3: Verify client compiles**

```bash
cd client && cargo check 2>&1 | tail -5
```

**Step 4: Commit**

```bash
git add -A && git commit -m "feat: add SpectatorCamera with waypoint drift for login screen"
```

---

### Task 7: Client — Spectator Network Connection

**Files:**
- Modify: `client/src/network/client.rs` — add `new_spectator()`, `connect_spectator()`, `send_spectator_upgrade()`, modify `poll()` for spectator reconnection

**Context:** NetworkClient currently has constructors `new_guest()` and `new_authenticated()` which both call `start_matchmaking()`. For spectator mode, we skip matchmaking and connect directly to `/spectate`. The client uses the `ewebsock` crate for WebSocket connections.

**Step 1: Add spectator_mode field to NetworkClient**

Add `spectator_mode: bool` to the NetworkClient struct (~line 54). Initialize to `false` in all existing constructors.

**Step 2: Add new_spectator constructor**

```rust
/// Create a spectator client that connects to /spectate for login screen world view.
/// No auth needed. Connection is read-only until upgraded.
#[cfg(not(target_arch = "wasm32"))]
pub fn new_spectator(base_url: &str) -> Self {
    let mut client = Self {
        sender: None,
        receiver: None,
        base_url: base_url.to_string(),
        player_name: String::new(),
        connection_state: ConnectionState::Disconnected,
        reconnect_timer: 0.0,
        room_id: None,
        session_token: None,
        auth_token: None,
        character_id: None,
        reconnect_attempts: 0,
        was_connected: false,
        spectator_mode: true,
    };
    client.connect_spectator();
    client
}
```

**Step 3: Add connect_spectator method**

```rust
#[cfg(not(target_arch = "wasm32"))]
fn connect_spectator(&mut self) {
    let http_base = self.base_url
        .replace("ws://", "http://")
        .replace("wss://", "https://");
    // Convert to ws:// or wss:// for WebSocket
    let ws_base = &self.base_url;
    let ws_url = format!("{}/spectate", ws_base);
    log::info!("Connecting spectator WebSocket: {}", ws_url);

    self.connection_state = ConnectionState::Connecting;
    match ewebsock::connect(&ws_url, ewebsock::Options::default()) {
        Ok((sender, receiver)) => {
            log::info!("Spectator WebSocket connection initiated");
            self.sender = Some(sender);
            self.receiver = Some(receiver);
        }
        Err(e) => {
            log::warn!("Spectator WebSocket connection failed: {}", e);
            self.connection_state = ConnectionState::Disconnected;
        }
    }
}
```

**Step 4: Add send_spectator_upgrade method**

```rust
/// Send upgrade message to transition from spectator to authenticated player.
/// Call this after HTTP matchmake succeeds.
pub fn send_spectator_upgrade(&mut self, session_token: &str) {
    let msg = ClientMessage::SpectatorUpgrade {
        session_token: session_token.to_string(),
    };
    if let Some(sender) = &mut self.sender {
        if let Ok(bytes) = super::protocol::encode_client_message(&msg) {
            sender.send(ewebsock::WsMessage::Binary(bytes));
            self.spectator_mode = false;
            log::info!("Sent spectator upgrade message");
        }
    }
}
```

**Step 5: Modify poll() for spectator reconnection**

In `poll()` (~line 252), in the `ConnectionState::Disconnected` branch, add spectator-specific reconnection before the existing logic:

```rust
ConnectionState::Disconnected => {
    if self.spectator_mode {
        // Spectator reconnection - just retry /spectate connection, no matchmaking
        self.reconnect_timer += 1.0 / 60.0;
        if self.reconnect_timer > 3.0 {
            self.reconnect_timer = 0.0;
            self.connect_spectator();
        }
        return;
    }
    // ... existing reconnection logic unchanged
}
```

**Step 6: Add is_spectator() and is_spectator_connected() getters**

```rust
pub fn is_spectator(&self) -> bool {
    self.spectator_mode
}

pub fn is_spectator_connected(&self) -> bool {
    self.spectator_mode && self.connection_state == ConnectionState::Connected
}
```

**Step 7: Verify client compiles**

```bash
cd client && cargo check 2>&1 | tail -5
```

**Step 8: Commit**

```bash
git add -A && git commit -m "feat: add spectator mode to NetworkClient"
```

---

### Task 8: Client — SpectatorState and LoginScreen Integration

**Files:**
- Modify: `client/src/main.rs` — add SpectatorState struct, modify AppState, update main loop
- Modify: `client/src/ui/screens.rs` — modify LoginScreen rendering to support world backdrop
- Modify: `client/src/game/state.rs` — add `spectator_mode` flag, modify `is_world_ready()`

**Context:** This is the biggest integration task. We need to:
1. Create a `SpectatorState` that holds the spectator's GameState + NetworkClient + Camera
2. Thread it through AppState variants (Login, CharacterSelect, CharacterCreate)
3. Update + render the spectator world behind each screen
4. Implement crossfade from starry sky to live world

**Step 1: Add spectator_mode to GameState**

In `client/src/game/state.rs`, add to GameState struct:

```rust
pub spectator_mode: bool,
```

Initialize to `false` in `GameState::new()`.

Modify `is_world_ready()` for spectator mode:

```rust
pub fn is_world_ready(&self) -> bool {
    if self.spectator_mode {
        // In spectator mode, check if spawn chunk is loaded (no local player)
        let spawn_chunk = crate::game::chunk::ChunkCoord::from_world(15, 4);
        return self.chunk_manager.chunks().contains_key(&spawn_chunk);
    }
    // ... existing logic unchanged
}
```

**Step 2: Create SpectatorState struct in main.rs**

```rust
#[cfg(not(target_arch = "wasm32"))]
struct SpectatorState {
    game_state: GameState,
    network: NetworkClient,
    camera: game::SpectatorCamera,
    crossfade_alpha: f32,   // 0.0 = stars, 1.0 = world fully visible
    world_ready: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl SpectatorState {
    fn new() -> Self {
        let mut game_state = GameState::new();
        game_state.spectator_mode = true;
        let network = NetworkClient::new_spectator(WS_URL);
        Self {
            game_state,
            network,
            camera: game::SpectatorCamera::new(),
            crossfade_alpha: 0.0,
            world_ready: false,
        }
    }

    fn update(&mut self, dt: f32) {
        // Poll network messages into game state
        self.network.poll(&mut self.game_state);

        // Update spectator camera
        let (cx, cy) = self.camera.update(dt);
        self.game_state.camera.x = cx;
        self.game_state.camera.y = cy;
        self.game_state.camera.zoom = 1.0;
        self.game_state.camera.initialized = true;

        // Check world readiness and drive crossfade
        if !self.world_ready && self.game_state.is_world_ready() {
            self.world_ready = true;
        }

        if self.world_ready {
            // Fade in over ~1.5 seconds
            self.crossfade_alpha = (self.crossfade_alpha + dt / 1.5).min(1.0);
        }
    }
}
```

**Step 3: Modify AppState to carry SpectatorState**

```rust
#[cfg(not(target_arch = "wasm32"))]
enum AppState {
    Login(LoginScreen, Option<SpectatorState>),
    CharacterSelect(CharacterSelectScreen, Option<SpectatorState>),
    CharacterCreate(CharacterCreateScreen, Option<SpectatorState>),
    Playing {
        game_state: GameState,
        network: NetworkClient,
        input_handler: InputHandler,
        _session: AuthSession,
    },
    GuestMode {
        game_state: GameState,
        network: NetworkClient,
        input_handler: InputHandler,
    },
}
```

**Step 4: Update main loop — initialization**

Where `LoginScreen` is first created (~line 149), also create SpectatorState:

```rust
let spectator = SpectatorState::new();
let mut app_state = AppState::Login(login_screen, Some(spectator));
```

**Step 5: Update main loop — Login state handling**

In the `AppState::Login(screen, spectator)` match arm, update spectator before rendering:

```rust
AppState::Login(screen, spectator) => {
    // Update spectator state
    let dt = get_frame_time();
    if let Some(spec) = spectator.as_mut() {
        spec.update(dt);
    }

    // Render spectator world behind login if ready
    if let Some(spec) = spectator.as_ref() {
        if spec.crossfade_alpha > 0.0 {
            // Render world
            renderer.render_spectator(&spec.game_state);
            // Overlay with semi-transparent dark to keep login form readable
            // (The login form's existing panel background handles this)
        }
    }

    let result = screen.update(&audio);

    // Pass crossfade alpha to login screen so it can dim stars
    let stars_alpha = spectator.as_ref()
        .map(|s| 1.0 - s.crossfade_alpha)
        .unwrap_or(1.0);
    screen.render_with_stars_alpha(stars_alpha);

    match result {
        ScreenState::ToCharacterSelect(session) => {
            audio.play_sfx("login_success");
            let mut char_screen = CharacterSelectScreen::new(session, SERVER_URL);
            // ... existing setup ...
            // Pass spectator through to character select
            app_state = AppState::CharacterSelect(char_screen, spectator.take());
        }
        // ... other transitions carry spectator through too
    }
}
```

**Step 6: Update CharacterSelect state handling similarly**

Thread the spectator state through. Update and render the world behind the character select screen too.

In the `ScreenState::StartGame` transition (where Playing state is created), this is where the **upgrade** happens:

```rust
ScreenState::StartGame { session, character_id, character_name } => {
    if let Some(mut spec) = spectator.take() {
        // Upgrade spectator connection to authenticated player
        // First, do HTTP matchmake to get session_token
        // (This happens synchronously via ureq, same as NetworkClient::start_matchmaking)
        let http_url = WS_URL.replace("ws://", "http://").replace("wss://", "https://");
        let matchmake_url = format!("{}/matchmake/joinOrCreate/game_room", http_url);
        let options = serde_json::json!({"characterId": character_id});
        let result = ureq::post(&matchmake_url)
            .set("Authorization", &format!("Bearer {}", session.token))
            .set("Content-Type", "application/json")
            .send_json(&options);

        match result {
            Ok(response) => {
                if let Ok(data) = response.into_json::<serde_json::Value>() {
                    if let Some(token) = data["session_token"].as_str() {
                        // Send upgrade over existing spectator WS
                        spec.network.send_spectator_upgrade(token);

                        // Reuse spectator's game state (chunks already loaded!)
                        let mut game_state = spec.game_state;
                        game_state.spectator_mode = false;
                        game_state.selected_character_name = Some(character_name);
                        // ... sync audio/UI settings (same as existing code) ...

                        let input_handler = InputHandler::new();
                        // input_handler.load_touch_icons().await;

                        audio.play_music("assets/audio/start.ogg").await;

                        app_state = AppState::Playing {
                            game_state,
                            network: spec.network, // Reuse the same connection!
                            input_handler,
                            _session: session,
                        };
                    }
                }
            }
            Err(e) => {
                log::error!("Matchmake for upgrade failed: {}", e);
                // Fallback: create fresh connection (like the existing flow)
                let network = NetworkClient::new_authenticated(WS_URL, &session.token, character_id);
                // ... existing Playing state creation ...
            }
        }
    } else {
        // No spectator state — use existing flow unchanged
        let network = NetworkClient::new_authenticated(WS_URL, &session.token, character_id);
        // ... existing code ...
    }
}
```

**Step 7: Modify LoginScreen render for stars alpha**

In `client/src/ui/screens.rs`, add a method or parameter to control stars alpha:

Add a `stars_alpha: f32` field to LoginScreen, defaulting to 1.0. Add a setter method `set_stars_alpha(&mut self, alpha: f32)`. In the render method, multiply star/gradient alpha values by `self.stars_alpha`.

Alternatively, add a `render_with_stars_alpha(&self, alpha: f32)` method that wraps the existing render but modulates the background alpha. If stars_alpha is 0.0, skip the starry background entirely.

**Step 8: Add render_spectator to Renderer**

In `client/src/render/renderer.rs`, add a simplified render method for spectator mode that renders the world without UI:

```rust
pub fn render_spectator(&self, state: &GameState) {
    // Render ground tiles, entities, effects — but skip all UI
    // Reuse the existing render pipeline up to entity rendering
    // Skip: chat bubbles, chat log, inventory, all panels
    self.render_ground_tiles(state);
    if state.is_world_ready() {
        self.render_entities(state);
    }
}
```

The exact implementation depends on how modular the existing render methods are. If `render()` is monolithic, you may need to extract the world rendering portion. At minimum, you can call the existing `render()` and it will work — it already checks `is_world_ready()` and won't render UI panels unless they're open (and in spectator mode none are open). The main thing to skip is chat-related rendering.

**Step 9: Verify client compiles**

```bash
cd client && cargo check 2>&1 | tail -5
```

**Step 10: Commit**

```bash
git add -A && git commit -m "feat: integrate spectator world view into login and character select screens"
```

---

### Task 9: Client — Camera Transition on Upgrade

**Files:**
- Modify: `client/src/game/state.rs` — add camera transition state
- Modify: `client/src/main.rs` — trigger camera transition on Playing state entry

**Context:** When upgrading from spectator to playing, the camera should smoothly pan from its current spectator drift position to the player's actual position. The existing camera directly follows the player each frame (`camera.x = player.x, camera.y = player.y`). We need a temporary transition mode.

**Step 1: Add camera transition fields to GameState or Camera**

In the Camera struct (state.rs ~line 258), add:

```rust
pub transition_from: Option<(f32, f32)>,  // Starting position of transition
pub transition_progress: f32,              // 0.0 to 1.0
```

**Step 2: Modify camera update to use transition**

In the GameState update where camera follows player (~line 2005), add transition handling:

```rust
if let Some(local_id) = &self.local_player_id {
    if let Some(player) = self.players.get(local_id) {
        if let Some((from_x, from_y)) = self.camera.transition_from {
            // Smooth transition from spectator position to player
            self.camera.transition_progress += dt * 1.5; // ~0.67 seconds
            if self.camera.transition_progress >= 1.0 {
                self.camera.transition_from = None;
                self.camera.x = player.x;
                self.camera.y = player.y;
            } else {
                let t = smooth_step(self.camera.transition_progress);
                self.camera.x = from_x + (player.x - from_x) * t;
                self.camera.y = from_y + (player.y - from_y) * t;
            }
        } else {
            self.camera.x = player.x;
            self.camera.y = player.y;
        }
        self.camera.initialized = true;
    }
}
```

**Step 3: Set transition start in upgrade flow**

In main.rs, when transitioning from spectator to Playing, capture the spectator camera position:

```rust
// Before creating Playing state:
let (cam_x, cam_y) = spec.camera.position();
game_state.camera.transition_from = Some((cam_x, cam_y));
game_state.camera.transition_progress = 0.0;
```

**Step 4: Verify client compiles**

```bash
cd client && cargo check 2>&1 | tail -5
```

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: smooth camera transition from spectator to player on login"
```

---

## Implementation Notes

### Things to watch for:
- **WASM builds:** All spectator code should be behind `#[cfg(not(target_arch = "wasm32"))]`. The WASM build path in main.rs is completely separate and unchanged.
- **The renderer:** The existing `Renderer::render()` takes `&GameState` and renders everything. For spectator mode, it should work as-is since no UI panels are open and there's no chat to render. If chat bubbles still appear (from player chat in the world), that's fine — the design says skip chat log/input, not chat bubbles.
- **Message handler:** The existing message handler in `client/src/network/` already processes StateSync and ChunkData messages. It will work for spectator mode without changes since it updates `GameState` fields that exist in spectator mode.
- **`local_player_id` is None in spectator mode:** Many places check `self.local_player_id`. In spectator mode this is None. Make sure this doesn't cause panics — it should be fine since most code uses `if let Some(id) = ...` patterns.
- **`get_frame_time()` vs `dt`:** Macroquad's `get_frame_time()` returns delta time for the current frame. Use this for the spectator camera update.

### Testing approach:
1. Start the server locally
2. Launch the client — should see starry sky, then world fades in
3. Walk around on another client — should see the player moving in the spectator view
4. Log in — should see smooth camera transition to player position
5. Verify normal gameplay works after upgrade
6. Test with server down — should see starry sky only, no errors
7. Test spectator cap (connect 50+ spectators)
