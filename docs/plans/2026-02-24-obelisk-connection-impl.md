# Obelisk Connection Quest - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a quest where a player meets a scholar at an obelisk, travels north to fix a broken waystone connection (by digging up and killing a hedgehog), and unlocks fast-travel between two obelisks.

**Architecture:** Data-driven design using existing TOML + Lua quest system. Three new server-side systems (persistent ground spawns, dig sites, waystones) loaded from TOML configs. New `InteractObject` protocol message for clicking map objects. Client gets a new `MapObject` context menu target.

**Tech Stack:** Rust server (Axum/Tokio), Rust client (Macroquad), MessagePack protocol, TOML configs, Lua scripting

**Design Doc:** `docs/plans/2026-02-24-obelisk-connection-quest-design.md`

---

## Task 1: Create Data Files (Items, NPC, Quest Definition)

Pure data — no Rust code changes. Sets up all TOML/Lua content.

**Files:**
- Create: `rust-server/data/items/tools.toml`
- Create: `rust-server/data/quests/exploration/obelisk_connection.toml`
- Create: `rust-server/data/scripts/quests/exploration/obelisk_connection.lua`
- Create: `rust-server/data/ground_spawns.toml`
- Create: `rust-server/data/dig_sites.toml`
- Create: `rust-server/data/waystones.toml`
- Modify: `rust-server/data/entities/npcs/villagers.toml` (append Researcher Orin)
- Modify: `rust-server/data/quest_locations.toml` (add north_obelisk)

**Step 1: Create `rust-server/data/items/tools.toml`**

```toml
[shovel]
display_name = "Old Shovel"
sprite = "shovel"
description = "A worn but sturdy shovel. Useful for digging into soft ground."
category = "quest"
max_stack = 1
base_price = 5
sellable = false

[shovel.use_effect]
type = "dig"
```

Note: The item registry at `rust-server/src/data/item_registry.rs:20-57` auto-discovers all `.toml` files in `data/items/`, so no registration code needed.

**Step 2: Create `rust-server/data/quests/exploration/obelisk_connection.toml`**

Create directory `rust-server/data/quests/exploration/` first. Content from design doc section 1 (the full quest TOML with objectives: reach_north_obelisk, kill_hedgehog, return_to_orin).

**Step 3: Create `rust-server/data/scripts/quests/exploration/obelisk_connection.lua`**

Create directory `rust-server/data/scripts/quests/exploration/` first. Content from design doc section 2 (the full Lua script with on_interact, show_offer_dialogue, etc).

**Step 4: Append Researcher Orin to `rust-server/data/entities/npcs/villagers.toml`**

Add the full NPC definition block from the design doc section 4 at the end of the file. Uses `sprite = "jackson"`, `quest_giver = true`, `available_quests = ["obelisk_connection"]`.

**Step 5: Add quest location to `rust-server/data/quest_locations.toml`**

Append:
```toml
[north_obelisk]
x = 92
y = -163
radius = 3
```

**Step 6: Create `rust-server/data/ground_spawns.toml`**

```toml
# Persistent ground item spawns
# Items that always exist at a location and respawn after pickup

[[spawns]]
id = "obelisk_shovel"
item_id = "shovel"
x = 93.0
y = -162.0
quantity = 1
respawn_seconds = 30
```

**Step 7: Create `rust-server/data/dig_sites.toml`**

```toml
# Dig sites - locations where using a shovel triggers quest events

[[sites]]
id = "obelisk_blockage"
x = 94
y = -160
radius = 1
quest_id = "obelisk_connection"
quest_objective_id = "kill_hedgehog"
spawn_entity = "hedgehog"
spawn_level = 6
```

**Step 8: Create `rust-server/data/waystones.toml`**

```toml
# Waystone teleport network

[[waystones]]
id = "south_obelisk"
name = "Southern Obelisk"
x = 88
y = 34
linked_to = "north_obelisk"
quest_required = "obelisk_connection"

[[waystones]]
id = "north_obelisk"
name = "Northern Obelisk"
x = 92
y = -163
linked_to = "south_obelisk"
quest_required = "obelisk_connection"
```

**Step 9: Add Researcher Orin spawn to chunk JSON**

Edit `rust-server/maps/world_0/chunk_2_1.json` — add to the `entities` array:
```json
{
  "entityId": "researcher_orin",
  "x": 25,
  "y": 2,
  "facing": "left",
  "uniqueId": "researcher_orin"
}
```

Local coords: global (89, 34) → chunk (2,1) → local (89 - 64, 34 - 32) = (25, 2).

**Step 10: Verify server compiles**

Run: `cd rust-server && cargo check 2>&1 | tail -5`
Expected: Should compile (no Rust changes yet). Confirms existing code is healthy.

**Step 11: Commit**

```bash
git add rust-server/data/items/tools.toml \
  rust-server/data/quests/exploration/obelisk_connection.toml \
  rust-server/data/scripts/quests/exploration/obelisk_connection.lua \
  rust-server/data/ground_spawns.toml \
  rust-server/data/dig_sites.toml \
  rust-server/data/waystones.toml \
  rust-server/data/entities/npcs/villagers.toml \
  rust-server/data/quest_locations.toml \
  rust-server/maps/world_0/chunk_2_1.json
git commit -m "feat: add obelisk connection quest data files"
```

---

## Task 2: Add `UseEffect::Dig` Variant

Add the Dig variant to the UseEffect enum so the shovel item can be deserialized.

**Files:**
- Modify: `rust-server/src/data/item_def.rs:159-180` (UseEffect enum)

**Step 1: Add Dig variant to UseEffect enum**

In `rust-server/src/data/item_def.rs`, add `Dig` after `LearnSpell`:

```rust
// At line 179, after LearnSpell { spell_id: String },
Dig,
```

The full enum becomes:
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UseEffect {
    Heal { amount: i32 },
    RestoreMana { amount: i32 },
    RestorePrayer { amount: i32 },
    Buff { stat: String, amount: i32, duration_ms: u64 },
    Teleport { destination: String },
    LearnSpell { spell_id: String },
    Dig,
}
```

**Step 2: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | grep -E "^error" | head -5`
Expected: No new errors. Existing match arms on UseEffect use `None =>` fallback or will need updating (check next step).

**Step 3: Handle Dig in `handle_use_item` pre-check**

The key issue: `inventory.use_item()` at `game.rs:6400` always decrements quantity. The shovel must NOT be consumed. We intercept Dig in the pre-check section (like recipe scrolls at lines 6359-6387), before `use_item()` is called.

In `rust-server/src/game.rs`, within `handle_use_item` around line 6384, add a new check after the spell scroll check:

```rust
// After the LearnSpell check (around line 6383), add:
if matches!(&def.use_effect, Some(crate::data::UseEffect::Dig)) {
    drop(players);
    self.handle_dig(player_id, slot_index).await;
    return;
}
```

**Step 4: Add placeholder `handle_dig` method**

Add to `GameRoom` impl in `game.rs` (near the other handle_ methods):

```rust
/// Handle using a dig tool (shovel) - checks dig sites near player
async fn handle_dig(&self, player_id: &str, _slot_index: u8) {
    self.send_system_message(player_id, "There's nothing to dig here.")
        .await;
}
```

**Step 5: Verify compilation**

Run: `cd rust-server && cargo check 2>&1 | grep -E "^error" | head -5`
Expected: Compiles with no new errors. May get a warning about unused `_slot_index`.

**Step 6: Commit**

```bash
git add rust-server/src/data/item_def.rs rust-server/src/game.rs
git commit -m "feat: add Dig use effect variant and handler stub"
```

---

## Task 3: Persistent Ground Item Spawns System

Load spawn definitions from TOML, create ground items on startup, respawn them after pickup.

**Files:**
- Create: `rust-server/src/ground_spawn.rs` (new module)
- Modify: `rust-server/src/main.rs` or `rust-server/src/lib.rs` (register module)
- Modify: `rust-server/src/game.rs` (load spawns, tick respawns, mark on pickup)

**Step 1: Create the ground spawn module**

Create `rust-server/src/ground_spawn.rs`:

```rust
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Deserialize)]
pub struct GroundSpawnDef {
    pub id: String,
    pub item_id: String,
    pub x: f32,
    pub y: f32,
    pub quantity: i32,
    pub respawn_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct GroundSpawnsFile {
    spawns: Vec<GroundSpawnDef>,
}

#[derive(Debug)]
pub struct GroundSpawnState {
    pub def: GroundSpawnDef,
    /// None = item is on the ground, Some(when) = picked up, waiting to respawn
    pub picked_up_at: Option<Instant>,
    /// The ground_item id currently active (if any)
    pub active_ground_item_id: Option<String>,
}

pub struct GroundSpawnManager {
    pub spawns: HashMap<String, GroundSpawnState>,
}

impl GroundSpawnManager {
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("ground_spawns.toml");
        let spawns = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<GroundSpawnsFile>(&content) {
                    Ok(file) => {
                        tracing::info!("Loaded {} ground spawn definitions", file.spawns.len());
                        file.spawns
                            .into_iter()
                            .map(|def| {
                                let id = def.id.clone();
                                (id, GroundSpawnState {
                                    def,
                                    picked_up_at: None,
                                    active_ground_item_id: None,
                                })
                            })
                            .collect()
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse ground_spawns.toml: {}", e);
                        HashMap::new()
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to read ground_spawns.toml: {}", e);
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        Self { spawns }
    }

    /// Mark a ground item as picked up by matching its ground_item_id
    pub fn mark_picked_up(&mut self, ground_item_id: &str) {
        for state in self.spawns.values_mut() {
            if state.active_ground_item_id.as_deref() == Some(ground_item_id) {
                state.picked_up_at = Some(Instant::now());
                state.active_ground_item_id = None;
                tracing::debug!("Ground spawn '{}' picked up, will respawn in {}s", state.def.id, state.def.respawn_seconds);
                return;
            }
        }
    }

    /// Check which spawns need respawning. Returns list of (spawn_id, item_id, x, y, quantity).
    pub fn check_respawns(&mut self) -> Vec<(String, String, f32, f32, i32)> {
        let now = Instant::now();
        let mut to_respawn = Vec::new();

        for state in self.spawns.values_mut() {
            if let Some(picked_up_at) = state.picked_up_at {
                if now.duration_since(picked_up_at).as_secs() >= state.def.respawn_seconds {
                    state.picked_up_at = None;
                    to_respawn.push((
                        state.def.id.clone(),
                        state.def.item_id.clone(),
                        state.def.x,
                        state.def.y,
                        state.def.quantity,
                    ));
                }
            }
        }

        to_respawn
    }

    /// Record that a ground item was created for a spawn
    pub fn set_active_ground_item(&mut self, spawn_id: &str, ground_item_id: String) {
        if let Some(state) = self.spawns.get_mut(spawn_id) {
            state.active_ground_item_id = Some(ground_item_id);
        }
    }

    /// Get initial spawns that need ground items created (on server start)
    pub fn get_initial_spawns(&self) -> Vec<(String, String, f32, f32, i32)> {
        self.spawns
            .values()
            .filter(|s| s.picked_up_at.is_none() && s.active_ground_item_id.is_none())
            .map(|s| (s.def.id.clone(), s.def.item_id.clone(), s.def.x, s.def.y, s.def.quantity))
            .collect()
    }
}
```

**Step 2: Register module**

Add `pub mod ground_spawn;` to the server's module declarations (likely `rust-server/src/main.rs` or `rust-server/src/lib.rs` — check where other `mod` declarations are).

**Step 3: Add GroundSpawnManager to GameRoom**

In `rust-server/src/game.rs`, add to the `GameRoom` struct fields:

```rust
ground_spawn_manager: RwLock<GroundSpawnManager>,
```

**Step 4: Load spawns and create initial ground items in GameRoom::new()**

In the GameRoom constructor (around line 935-983 area), after loading other systems:

```rust
// Load persistent ground spawns
let mut ground_spawn_manager = GroundSpawnManager::load(Path::new("data"));

// Create initial ground items for persistent spawns
let current_time = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_millis() as u64;

let initial_ground_items = {
    let initial_spawns = ground_spawn_manager.get_initial_spawns();
    let mut items = Vec::new();
    for (spawn_id, item_id, x, y, quantity) in initial_spawns {
        let ground_item_id = format!("persistent_{}", spawn_id);
        let mut ground_item = GroundItem::new(&ground_item_id, &item_id, x, y, quantity, None, current_time);
        items.push((spawn_id, ground_item_id, ground_item));
    }
    items
};

for (spawn_id, ground_item_id, ground_item) in initial_ground_items {
    ground_spawn_manager.set_active_ground_item(&spawn_id, ground_item_id.clone());
    ground_items.insert(ground_item_id, ground_item);
}
```

**Step 5: Exclude persistent items from expiry in tick**

In `game.rs` around line 12908-12925 (ground item expiry check), modify the filter to skip persistent items:

```rust
// Check for expired items (60 second lifetime) — skip persistent spawns
let persistent_ids: std::collections::HashSet<String> = {
    let gsm = self.ground_spawn_manager.read().await;
    gsm.spawns.values()
        .filter_map(|s| s.active_ground_item_id.clone())
        .collect()
};

let expired_items: Vec<String> = {
    let items = self.ground_items.read().await;
    items
        .iter()
        .filter(|(id, item)| !persistent_ids.contains(id.as_str()) && item.is_expired(current_time))
        .map(|(id, _)| id.clone())
        .collect()
};
```

**Step 6: Mark persistent spawns as picked up in `handle_pickup`**

In `game.rs` `handle_pickup()` (around line 5316-5320), after successfully removing the ground item, notify the spawn manager:

```rust
if removed {
    // Check if this was a persistent ground spawn
    {
        let mut gsm = self.ground_spawn_manager.write().await;
        gsm.mark_picked_up(item_id);
    }
    // ... rest of existing code
}
```

**Step 7: Add respawn check to tick loop**

In `game.rs` tick(), after the expired items check (around line 12925), add:

```rust
// Respawn persistent ground items
{
    let respawns = {
        let mut gsm = self.ground_spawn_manager.write().await;
        gsm.check_respawns()
    };

    if !respawns.is_empty() {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        for (spawn_id, item_id, x, y, quantity) in respawns {
            let ground_item_id = format!("persistent_{}", spawn_id);
            let ground_item = GroundItem::new(&ground_item_id, &item_id, x, y, quantity, None, current_time);
            {
                let mut items = self.ground_items.write().await;
                items.insert(ground_item_id.clone(), ground_item);
            }
            {
                let mut gsm = self.ground_spawn_manager.write().await;
                gsm.set_active_ground_item(&spawn_id, ground_item_id);
            }
            tracing::debug!("Respawned persistent ground item: {}", spawn_id);
        }
    }
}
```

**Step 8: Verify compilation**

Run: `cd rust-server && cargo check 2>&1 | grep -E "^error" | head -5`
Expected: Compiles.

**Step 9: Commit**

```bash
git add rust-server/src/ground_spawn.rs rust-server/src/game.rs rust-server/src/main.rs
git commit -m "feat: add persistent ground item spawn system"
```

---

## Task 4: Dig Sites System

Load dig site definitions, implement the dig handler that checks player position and quest state, spawns a mob.

**Files:**
- Create: `rust-server/src/dig_site.rs` (new module)
- Modify: `rust-server/src/game.rs` (load dig sites, implement handle_dig)

**Step 1: Create the dig site module**

Create `rust-server/src/dig_site.rs`:

```rust
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct DigSiteDef {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub radius: i32,
    pub quest_id: String,
    pub quest_objective_id: String,
    pub spawn_entity: String,
    pub spawn_level: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct DigSitesFile {
    sites: Vec<DigSiteDef>,
}

pub struct DigSiteManager {
    pub sites: Vec<DigSiteDef>,
    /// Track which players have triggered which dig sites: (player_id, site_id)
    pub triggered: HashSet<(String, String)>,
}

impl DigSiteManager {
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("dig_sites.toml");
        let sites = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<DigSitesFile>(&content) {
                    Ok(file) => {
                        tracing::info!("Loaded {} dig site definitions", file.sites.len());
                        file.sites
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse dig_sites.toml: {}", e);
                        Vec::new()
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to read dig_sites.toml: {}", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        Self {
            sites,
            triggered: HashSet::new(),
        }
    }

    /// Find a dig site the player can activate at their current position
    pub fn find_site_at(
        &self,
        player_x: i32,
        player_y: i32,
        player_id: &str,
        active_quest_ids: &[String],
        quest_objective_statuses: &HashMap<String, HashMap<String, bool>>,
    ) -> Option<&DigSiteDef> {
        for site in &self.sites {
            // Check proximity
            let dx = (player_x - site.x).abs();
            let dy = (player_y - site.y).abs();
            if dx > site.radius || dy > site.radius {
                continue;
            }

            // Check not already triggered
            if self.triggered.contains(&(player_id.to_string(), site.id.clone())) {
                continue;
            }

            // Check quest is active
            if !active_quest_ids.contains(&site.quest_id) {
                continue;
            }

            // Check the objective hasn't been completed yet
            if let Some(objectives) = quest_objective_statuses.get(&site.quest_id) {
                if objectives.get(&site.quest_objective_id) == Some(&true) {
                    continue; // Already completed this objective
                }
            }

            return Some(site);
        }
        None
    }

    pub fn mark_triggered(&mut self, player_id: &str, site_id: &str) {
        self.triggered.insert((player_id.to_string(), site_id.to_string()));
    }
}
```

**Step 2: Register module**

Add `pub mod dig_site;` to module declarations.

**Step 3: Add DigSiteManager to GameRoom**

```rust
dig_site_manager: RwLock<DigSiteManager>,
```

Load in constructor:
```rust
let dig_site_manager = DigSiteManager::load(Path::new("data"));
```

**Step 4: Implement `handle_dig`**

Replace the placeholder in `game.rs`:

```rust
async fn handle_dig(&self, player_id: &str, _slot_index: u8) {
    // Get player position and quest state
    let (px, py, active_quests, objective_statuses) = {
        let players = self.players.read().await;
        let player = match players.get(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };
        let active_quest_ids: Vec<String> = player
            .quest_progress
            .iter()
            .filter(|(_, state)| state.status == QuestStatus::InProgress)
            .map(|(id, _)| id.clone())
            .collect();
        let objective_statuses: HashMap<String, HashMap<String, bool>> = player
            .quest_progress
            .iter()
            .map(|(quest_id, state)| {
                let obj_map: HashMap<String, bool> = state
                    .objectives
                    .iter()
                    .map(|(obj_id, progress)| (obj_id.clone(), progress.completed))
                    .collect();
                (quest_id.clone(), obj_map)
            })
            .collect();
        (player.x, player.y, active_quest_ids, objective_statuses)
    };

    // Check dig sites
    let site_match = {
        let dsm = self.dig_site_manager.read().await;
        dsm.find_site_at(px, py, player_id, &active_quests, &objective_statuses)
            .cloned()
    };

    if let Some(site) = site_match {
        // Mark as triggered
        {
            let mut dsm = self.dig_site_manager.write().await;
            dsm.mark_triggered(player_id, &site.id);
        }

        // Spawn the entity
        self.send_system_message(player_id, "You dig into the ground... something is stirring beneath!")
            .await;

        if let Some(prototype) = self.entity_registry.get(&site.spawn_entity) {
            let npc_id = format!("dig_{}_{}", site.id, player_id);
            let npc = crate::npc::Npc::from_prototype(
                &npc_id,
                &site.spawn_entity,
                prototype,
                site.x,
                site.y,
                site.spawn_level,
                None,
            );
            let mut npcs = self.npcs.write().await;
            npcs.insert(npc_id, npc);
        }
    } else {
        self.send_system_message(player_id, "There's nothing to dig here.")
            .await;
    }
}
```

Note: The exact field names for quest progress (`quest_progress`, `QuestStatus`, `objectives`, `completed`) need to match the actual codebase. Check `rust-server/src/quest/state.rs` for the real types. The above is a guide — adapt to the actual struct fields.

**Step 5: Verify compilation**

Run: `cd rust-server && cargo check 2>&1 | grep -E "^error" | head -5`
Expected: Compiles. Adapt field names if needed.

**Step 6: Commit**

```bash
git add rust-server/src/dig_site.rs rust-server/src/game.rs rust-server/src/main.rs
git commit -m "feat: add dig site system with quest-triggered mob spawning"
```

---

## Task 5: InteractObject Protocol Message & Obelisk Interaction

Add a new client→server message for clicking map objects, and server-side handling for obelisk interactions.

**Files:**
- Modify: `rust-server/src/protocol.rs:12` (ClientMessage enum)
- Modify: `rust-server/src/main.rs` (message dispatch for InteractObject)
- Modify: `rust-server/src/game.rs` (handle_interact_object method)
- Modify: `client/src/network/messages.rs` (add InteractObject to client messages)

**Step 1: Add InteractObject to server's ClientMessage**

In `rust-server/src/protocol.rs`, add after `Interact` (around line 53):

```rust
/// Interact with a world map object (obelisk, etc.)
#[serde(rename = "interactObject")]
InteractObject { x: i32, y: i32 },
```

**Step 2: Add dispatch in message handler**

Find where `ClientMessage` variants are matched and dispatched (in `main.rs` or wherever the WebSocket message handler is). Add:

```rust
ClientMessage::InteractObject { x, y } => {
    game_room.handle_interact_object(&player_id, x, y).await;
}
```

**Step 3: Add handle_interact_object to GameRoom**

```rust
/// Handle player interacting with a world map object
pub async fn handle_interact_object(&self, player_id: &str, x: i32, y: i32) {
    // Check waystones first
    {
        let wsm = self.waystone_manager.read().await;
        if let Some(waystone) = wsm.get_at(x, y) {
            self.handle_waystone_interaction(player_id, waystone).await;
            return;
        }
    }

    // Check if this is an obelisk during the quest (before waystone is unlocked)
    // The obelisk at (92, -163) has special quest dialogue
    self.handle_obelisk_quest_interaction(player_id, x, y).await;
}
```

**Step 4: Add InteractObject to client network messages**

In `client/src/network/messages.rs`, add to the client message enum:

```rust
#[serde(rename = "interactObject")]
InteractObject { x: i32, y: i32 },
```

**Step 5: Verify both sides compile**

Run: `cd rust-server && cargo check` and `cd client && cargo check`

**Step 6: Commit**

```bash
git add rust-server/src/protocol.rs rust-server/src/main.rs rust-server/src/game.rs \
  client/src/network/messages.rs
git commit -m "feat: add InteractObject protocol message for map object interaction"
```

---

## Task 6: Waystone System (Fast-Travel)

Load waystone definitions, implement teleportation when clicked after quest completion.

**Files:**
- Create: `rust-server/src/waystone.rs` (new module)
- Modify: `rust-server/src/game.rs` (load waystones, handle waystone interaction + teleport)

**Step 1: Create waystone module**

Create `rust-server/src/waystone.rs`:

```rust
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct WaystoneDef {
    pub id: String,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub linked_to: String,
    pub quest_required: String,
}

#[derive(Debug, Clone, Deserialize)]
struct WaystonesFile {
    waystones: Vec<WaystoneDef>,
}

pub struct WaystoneManager {
    /// Keyed by waystone id
    pub waystones: HashMap<String, WaystoneDef>,
    /// Spatial lookup: (x, y) -> waystone id
    pub by_position: HashMap<(i32, i32), String>,
}

impl WaystoneManager {
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("waystones.toml");
        let waystones_list = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<WaystonesFile>(&content) {
                    Ok(file) => {
                        tracing::info!("Loaded {} waystone definitions", file.waystones.len());
                        file.waystones
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse waystones.toml: {}", e);
                        Vec::new()
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to read waystones.toml: {}", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        let mut waystones = HashMap::new();
        let mut by_position = HashMap::new();

        for ws in waystones_list {
            by_position.insert((ws.x, ws.y), ws.id.clone());
            waystones.insert(ws.id.clone(), ws);
        }

        Self { waystones, by_position }
    }

    /// Get waystone at exact position (or within 1 tile for click tolerance)
    pub fn get_at(&self, x: i32, y: i32) -> Option<&WaystoneDef> {
        // Check exact and adjacent tiles
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(id) = self.by_position.get(&(x + dx, y + dy)) {
                    return self.waystones.get(id);
                }
            }
        }
        None
    }

    /// Get the linked waystone destination
    pub fn get_destination(&self, waystone_id: &str) -> Option<&WaystoneDef> {
        self.waystones
            .get(waystone_id)
            .and_then(|ws| self.waystones.get(&ws.linked_to))
    }
}
```

**Step 2: Register module, add to GameRoom, load in constructor**

Same pattern as previous modules.

**Step 3: Implement handle_waystone_interaction**

```rust
async fn handle_waystone_interaction(&self, player_id: &str, waystone: &WaystoneDef) {
    // Check if player has completed the required quest
    let quest_completed = {
        let players = self.players.read().await;
        if let Some(player) = players.get(player_id) {
            player.completed_quest_ids.contains(&waystone.quest_required)
            // Adapt to actual field name for completed quests
        } else {
            false
        }
    };

    if quest_completed {
        // Get destination
        let destination = {
            let wsm = self.waystone_manager.read().await;
            wsm.get_destination(&waystone.id).cloned()
        };

        if let Some(dest) = destination {
            // Show teleport confirmation dialogue
            // Use the dialogue system to ask Yes/No
            let dialogue = ServerMessage::Dialogue {
                speaker: waystone.name.clone(),
                text: format!("The waystone hums with energy. Travel to the {}?", dest.name),
                choices: vec![
                    DialogueChoice { id: "teleport".to_string(), text: "Yes, teleport me.".to_string() },
                    DialogueChoice { id: "cancel".to_string(), text: "Not now.".to_string() },
                ],
                quest_id: None,
            };
            self.send_to_player(player_id, dialogue).await;
            // Store pending waystone teleport so DialogueChoice can resolve it
        }
    } else {
        // Quest not completed - show lore text
        self.send_system_message(player_id, "The ancient stone stands silent. Perhaps someone nearby knows more about it.")
            .await;
    }
}
```

Note: The exact ServerMessage::Dialogue format and DialogueChoice struct need to match the actual codebase. Check `protocol.rs` for the real dialogue message types. Adapt accordingly.

**Step 4: Implement handle_obelisk_quest_interaction**

```rust
async fn handle_obelisk_quest_interaction(&self, player_id: &str, x: i32, y: i32) {
    // Check if this is the northern obelisk during the quest
    let north_obelisk = (92, -163);
    let dx = (x - north_obelisk.0).abs();
    let dy = (y - north_obelisk.1).abs();

    if dx > 2 || dy > 2 {
        return; // Not near the northern obelisk
    }

    // Check quest state
    let quest_state = {
        let players = self.players.read().await;
        if let Some(player) = players.get(player_id) {
            // Check if player is on obelisk_connection quest
            player.quest_progress.get("obelisk_connection").map(|state| {
                let hedgehog_done = state.objectives
                    .get("kill_hedgehog")
                    .map(|o| o.completed)
                    .unwrap_or(false);
                let reach_done = state.objectives
                    .get("reach_north_obelisk")
                    .map(|o| o.completed)
                    .unwrap_or(false);
                (reach_done, hedgehog_done)
            })
        } else {
            None
        }
    };

    match quest_state {
        Some((true, false)) => {
            // Reached obelisk but hasn't killed hedgehog yet
            let dialogue = ServerMessage::Dialogue {
                speaker: "Ancient Obelisk".to_string(),
                text: "The stone hums faintly... something buried beneath is disrupting the flow of energy. You'll need to dig it out. Perhaps there's a tool lying nearby...".to_string(),
                choices: vec![],
                quest_id: None,
            };
            self.send_to_player(player_id, dialogue).await;
        }
        Some((true, true)) => {
            // Hedgehog killed - restore connection
            let dialogue = ServerMessage::Dialogue {
                speaker: "Ancient Obelisk".to_string(),
                text: "The stone pulses with renewed energy. You feel the connection snap into place, reaching far to the south. The waystone is restored!".to_string(),
                choices: vec![],
                quest_id: None,
            };
            self.send_to_player(player_id, dialogue).await;
            // Advance quest - this should trigger the "return to Orin" objective
        }
        _ => {
            // Not on quest or wrong state
            self.send_system_message(player_id, "An ancient stone covered in faded runes. It seems dormant.")
                .await;
        }
    }
}
```

**Step 5: Verify compilation**

Run: `cd rust-server && cargo check`

**Step 6: Commit**

```bash
git add rust-server/src/waystone.rs rust-server/src/game.rs rust-server/src/main.rs
git commit -m "feat: add waystone fast-travel system with quest gating"
```

---

## Task 7: Client-Side Map Object Interaction

Add MapObject as a context menu target, detect obelisk clicks, send InteractObject message.

**Files:**
- Modify: `client/src/game/state.rs:984-999` (ContextMenuTarget enum)
- Modify: `client/src/input/handler.rs:7612-7629` (map object click detection)
- Modify: `client/src/render/ui/context_menu.rs:162-170` (context menu rendering)

**Step 1: Add MapObject variant to ContextMenuTarget**

In `client/src/game/state.rs`, add after the `Rock` variant:

```rust
MapObject { tile_x: i32, tile_y: i32, gid: u32 },
```

**Step 2: Detect obelisk GID on right-click**

In `client/src/input/handler.rs` around line 7629 (after the Rock check in the map objects section), add a fallback for unrecognized map objects:

```rust
// After the Rock check, add:
// Unrecognized map object (obelisks, etc.) - offer generic interaction
break 'find_target ContextMenuTarget::MapObject {
    tile_x: clicked_tile_x,
    tile_y: clicked_tile_y,
    gid: obj_gid,
};
```

**Step 3: Add context menu rendering for MapObject**

In `client/src/render/ui/context_menu.rs`, add a match arm before the `Tile` variant:

```rust
ContextMenuTarget::MapObject { tile_x, tile_y, gid } => {
    push_option(&mut options, "Interact");
    push_option(&mut options, "Examine");
    "Object".to_string()
}
```

**Step 4: Handle context menu action → send InteractObject**

Find where context menu option selections are processed (in the input handler). When "Interact" is selected for a MapObject target, send:

```rust
commands.push(InputCommand::InteractObject { x: tile_x, y: tile_y });
```

This requires adding `InteractObject { x: i32, y: i32 }` to the `InputCommand` enum as well, and wiring it to send the network message.

**Step 5: Also handle left-click on map objects**

In the left-click handler section, when clicking on an unrecognized map object, pathfind to it and send InteractObject. Follow the same pattern as NPC interaction (pathfind_and_interact_npc).

**Step 6: Verify client compiles**

Run: `cd client && cargo check`

**Step 7: Commit**

```bash
git add client/src/game/state.rs client/src/input/handler.rs \
  client/src/render/ui/context_menu.rs client/src/network/messages.rs
git commit -m "feat: add client-side map object interaction and context menu"
```

---

## Task 8: Handle Waystone Teleport via Dialogue Choice + Right-Click Quick Teleport

Wire up the dialogue choice for waystone teleportation, and add right-click "Teleport" as a direct option.

**Files:**
- Modify: `rust-server/src/game.rs` (handle waystone dialogue choice → teleport player)
- Modify: `client/src/render/ui/context_menu.rs` (add "Teleport" for known waystone objects)

**Step 1: Server - handle waystone teleport confirmation**

When the player selects "teleport" in the waystone dialogue, the server needs to actually move them. This requires storing a "pending waystone" interaction per player and resolving it when the dialogue choice comes in.

Add to the player struct or a temporary map:
```rust
/// Pending waystone teleport destination (set when dialogue shown, cleared on choice)
pending_waystone_destination: Option<(i32, i32)>,
```

In `handle_waystone_interaction`, after showing dialogue, store the destination coords.

In the dialogue choice handler, when `choice_id == "teleport"` and player has a pending waystone:
```rust
player.x = dest_x;
player.y = dest_y;
player.move_dx = 0;
player.move_dy = 0;
// Clear pending
player.pending_waystone_destination = None;
// Send position update to player
```

**Step 2: Client - add "Teleport" to right-click for waystone map objects**

In the context menu rendering for `MapObject`, check if the GID matches a waystone (obelisk GID). This requires the client to know which objects are waystones. Options:
- Server sends waystone info on connect (cleanest)
- Client hardcodes the obelisk GID and checks completed quests

For now, in the `MapObject` context menu, check if the player has completed the quest:
```rust
ContextMenuTarget::MapObject { tile_x, tile_y, gid } => {
    // Check if this is a known waystone and quest is complete
    if state.is_waystone_unlocked(*tile_x, *tile_y) {
        push_option(&mut options, "Teleport");
    }
    push_option(&mut options, "Interact");
    push_option(&mut options, "Examine");
    "Obelisk".to_string()
}
```

The `is_waystone_unlocked` check needs waystone data on the client. The server should send unlocked waystone info as part of login data or quest completion.

**Step 3: Handle "Teleport" right-click action**

When "Teleport" is selected from context menu, send `InteractObject { x, y }` — the server will detect it's a waystone and teleport directly (skip dialogue for right-click, or auto-confirm).

Alternative: Add a separate `ClientMessage::UseTeleport { waystone_id }` for the direct teleport path.

**Step 4: Verify compilation**

Run both: `cd rust-server && cargo check` and `cd client && cargo check`

**Step 5: Commit**

```bash
git add rust-server/src/game.rs client/src/render/ui/context_menu.rs \
  client/src/game/state.rs
git commit -m "feat: add waystone teleportation via dialogue and right-click"
```

---

## Task 9: End-to-End Integration & Testing

Manual testing checklist — run the server and client, verify the full quest flow.

**Step 1: Start the server**

Run: `cd rust-server && cargo run`
Verify in logs:
- "Loaded N ground spawn definitions"
- "Loaded N dig site definitions"
- "Loaded N waystone definitions"
- "Spawning researcher_orin at (89, 34)"

**Step 2: Test Phase 1 — Meet Researcher Orin**

1. Walk to (89, 34) — see Researcher Orin with speech bubbles
2. Right-click → "Talk-to" → see quest offer dialogue
3. Accept quest → quest appears in quest tracker
4. Check quest log (Q key) → shows "Find the northern obelisk"

**Step 3: Test Phase 2 — Northern Obelisk**

1. Walk north to (92, -163)
2. Quest objective "reach_north_obelisk" should complete on arrival
3. Click the obelisk → see dialogue about buried blockage
4. Find the shovel on the ground at (93, -162) → pick it up
5. Walk to dig site at (94, -160) → double-click shovel in inventory
6. See "something is stirring" message → hedgehog spawns
7. Kill the hedgehog → "kill_hedgehog" objective completes
8. Click obelisk again → see "connection restored" dialogue

**Step 4: Test Phase 3 — Return**

1. Walk back to Researcher Orin at (89, 34)
2. Talk to Orin → quest completes, rewards given
3. Both obelisks should now be fast-travel points

**Step 5: Test Phase 4 — Fast Travel**

1. Click either obelisk → teleport dialogue appears → select Yes → teleported
2. Right-click obelisk → "Teleport" option → instant teleport
3. Verify round-trip works both directions

**Step 6: Test Edge Cases**

1. Dig at wrong location → "Nothing to dig here"
2. Dig without being on quest → "Nothing to dig here"
3. Click obelisk without quest → lore text
4. Shovel respawns 30s after pickup
5. Multiple players can independently do the quest

**Step 7: Final commit**

```bash
git add -A
git commit -m "feat: obelisk connection quest - complete implementation"
```
