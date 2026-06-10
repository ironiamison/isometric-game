# Collection Log Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Track unique items players have obtained, organized by source (Monster Drops, Boss Rewards, Skilling, Quest Rewards), with an in-game tabbed UI sharing the quest panel.

**Architecture:** New SQLite table for persistence, in-memory HashSet on Player struct for fast lookups, static TOML definitions for what's obtainable, 3 new protocol messages, and a tabbed quest/collection panel on the client.

**Tech Stack:** Rust (server: Axum/Tokio/SQLx, client: Macroquad), MessagePack protocol, SQLite, TOML config

---

### Task 1: Database Table + CRUD

**Files:**
- Modify: `rust-server/src/db.rs:728` (before `tracing::info!("Database migrations complete")`)

**Step 1: Add CREATE TABLE to migrate()**

After the `bans` table creation (line 728), before `tracing::info!`, add:

```rust
        // Collection log table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS collection_log (
                character_id INTEGER NOT NULL,
                item_id TEXT NOT NULL,
                source TEXT NOT NULL,
                source_detail TEXT,
                obtained_at TEXT NOT NULL,
                PRIMARY KEY (character_id, item_id, source)
            )
            "#,
        )
        .execute(pool)
        .await?;
```

**Step 2: Add CRUD methods**

After the existing DB methods (end of `impl Database`), add:

```rust
    // =========================================================================
    // Collection Log
    // =========================================================================

    pub async fn save_collection_entry(
        &self,
        character_id: i64,
        item_id: &str,
        source: &str,
        source_detail: &str,
        obtained_at: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO collection_log (character_id, item_id, source, source_detail, obtained_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(character_id)
        .bind(item_id)
        .bind(source)
        .bind(source_detail)
        .bind(obtained_at)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn load_collection_log(
        &self,
        character_id: i64,
    ) -> Result<Vec<(String, String, String, String)>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT item_id, source, source_detail, obtained_at FROM collection_log WHERE character_id = ?",
        )
        .bind(character_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                (
                    row.get("item_id"),
                    row.get("source"),
                    row.get::<String, _>("source_detail"),
                    row.get("obtained_at"),
                )
            })
            .collect())
    }
```

**Step 3: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`
Expected: No new errors

**Step 4: Commit**

```bash
git add rust-server/src/db.rs
git commit -m "feat: add collection_log database table and CRUD methods"
```

---

### Task 2: Static Collection Log Definitions (TOML)

**Files:**
- Create: `rust-server/data/collection_log.toml`
- Create: `rust-server/src/collection_log.rs`
- Modify: `rust-server/src/main.rs` (add module)

**Step 1: Create the TOML data file**

Create `rust-server/data/collection_log.toml`. Populate it by cross-referencing the existing loot tables, entity prototypes, skill gathering tables, and quest reward definitions in `rust-server/data/`. The format:

```toml
[monster_drops]
pig = ["piglet", "health_potion", "potato_seed", "greenleaf", "bronze_sword", "regular_bones"]
# ... add all monsters from data/entities/monsters/ that have loot entries

[boss_rewards]
# ... add all bosses from data/entities/ boss files

[skilling]
fishing = ["raw_shrimp", "raw_sardine", "raw_trout", "raw_salmon"]
mining = ["copper_ore", "tin_ore", "iron_ore"]
# ... add items from data/loot_tables.toml fishing/mining/etc tables
# ... add recipe results from data/recipes/ for cooking/smithing/alchemy

[quest_rewards]
# ... add reward items from each quest TOML in data/quests/
```

**Important:** To populate this accurately, read through:
- `rust-server/data/entities/monsters/*.toml` — each monster's `loot` and `loot_tables` entries
- `rust-server/data/loot_tables.toml` — fishing, mining, herb gathering tables
- `rust-server/data/recipes/*.toml` — crafting output items (cooking, smithing, alchemy)
- `rust-server/data/quests/**/*.toml` — `[rewards.items]` sections

**Step 2: Create the Rust module to load it**

Create `rust-server/src/collection_log.rs`:

```rust
use std::collections::HashMap;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct CollectionLogDefinitions {
    pub monster_drops: HashMap<String, Vec<String>>,
    pub boss_rewards: HashMap<String, Vec<String>>,
    pub skilling: HashMap<String, Vec<String>>,
    pub quest_rewards: HashMap<String, Vec<String>>,
}

impl CollectionLogDefinitions {
    pub fn load(path: &str) -> Self {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read collection_log.toml: {}", e));
        toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse collection_log.toml: {}", e))
    }

    /// Get all (item_id, source, source_detail) triples for protocol transmission
    pub fn all_entries(&self) -> Vec<(String, String, String)> {
        let mut entries = Vec::new();
        for (monster, items) in &self.monster_drops {
            for item in items {
                entries.push((item.clone(), "monster_drops".to_string(), monster.clone()));
            }
        }
        for (boss, items) in &self.boss_rewards {
            for item in items {
                entries.push((item.clone(), "boss_rewards".to_string(), boss.clone()));
            }
        }
        for (skill, items) in &self.skilling {
            for item in items {
                entries.push((item.clone(), "skilling".to_string(), skill.clone()));
            }
        }
        for (quest, items) in &self.quest_rewards {
            for item in items {
                entries.push((item.clone(), "quest_rewards".to_string(), quest.clone()));
            }
        }
        entries
    }
}
```

**Step 3: Register the module**

In `rust-server/src/main.rs`, add `mod collection_log;` alongside the other module declarations at the top.

Load it in AppState initialization, alongside the other registry loads. Add a `collection_log_defs: Arc<collection_log::CollectionLogDefinitions>` field to `AppState` and load it with `CollectionLogDefinitions::load("data/collection_log.toml")`.

**Step 4: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 5: Commit**

```bash
git add rust-server/data/collection_log.toml rust-server/src/collection_log.rs rust-server/src/main.rs
git commit -m "feat: add collection log definitions TOML and loader"
```

---

### Task 3: Protocol Messages

**Files:**
- Modify: `rust-server/src/protocol.rs`

**Step 1: Add 3 new ServerMessage variants**

Add before the closing `}` of the `ServerMessage` enum (after `TopPlayerChanged` at line ~1425):

```rust
    // Collection log messages
    CollectionLogDefinitions {
        /// Vec of (item_id, source, source_detail)
        entries: Vec<(String, String, String)>,
    },
    CollectionLogSync {
        /// Vec of (item_id, source, source_detail, obtained_at)
        entries: Vec<(String, String, String, String)>,
    },
    CollectionLogEntry {
        item_id: String,
        source: String,
        source_detail: String,
        obtained_at: String,
    },
```

**Step 2: Add msg_type() matches**

In `msg_type()` (after `TopPlayerChanged` match at line ~1894), add:

```rust
            ServerMessage::CollectionLogDefinitions { .. } => "collectionLogDefinitions",
            ServerMessage::CollectionLogSync { .. } => "collectionLogSync",
            ServerMessage::CollectionLogEntry { .. } => "collectionLogEntry",
```

**Step 3: Add encode_server_message() matches**

In `encode_server_message()` (after `TopPlayerChanged` encoding at line ~6745), add:

```rust
        ServerMessage::CollectionLogDefinitions { entries } => {
            let items: Vec<Value> = entries
                .iter()
                .map(|(item_id, source, source_detail)| {
                    Value::Array(vec![
                        Value::String(item_id.clone().into()),
                        Value::String(source.clone().into()),
                        Value::String(source_detail.clone().into()),
                    ])
                })
                .collect();
            let mut map = Vec::new();
            map.push((
                Value::String("entries".into()),
                Value::Array(items),
            ));
            Value::Map(map)
        }
        ServerMessage::CollectionLogSync { entries } => {
            let items: Vec<Value> = entries
                .iter()
                .map(|(item_id, source, source_detail, obtained_at)| {
                    Value::Array(vec![
                        Value::String(item_id.clone().into()),
                        Value::String(source.clone().into()),
                        Value::String(source_detail.clone().into()),
                        Value::String(obtained_at.clone().into()),
                    ])
                })
                .collect();
            let mut map = Vec::new();
            map.push((
                Value::String("entries".into()),
                Value::Array(items),
            ));
            Value::Map(map)
        }
        ServerMessage::CollectionLogEntry {
            item_id,
            source,
            source_detail,
            obtained_at,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("source".into()),
                Value::String(source.clone().into()),
            ));
            map.push((
                Value::String("source_detail".into()),
                Value::String(source_detail.clone().into()),
            ));
            map.push((
                Value::String("obtained_at".into()),
                Value::String(obtained_at.clone().into()),
            ));
            Value::Map(map)
        }
```

**Step 4: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 5: Commit**

```bash
git add rust-server/src/protocol.rs
git commit -m "feat: add collection log protocol messages"
```

---

### Task 4: In-Memory State + Recording Helper

**Files:**
- Modify: `rust-server/src/game.rs` (Player struct + GameRoom)

**Step 1: Add collection_log field to Player struct**

After `pub stall: Option<PlayerStall>,` (line ~449), add:

```rust
    /// Collection log: set of (item_id, source) pairs this player has obtained
    pub collection_log: HashSet<(String, String)>,
```

Initialize it in the Player constructor (wherever `Player { ... }` is constructed) with `collection_log: HashSet::new(),`.

**Step 2: Add helper method to GameRoom**

Add a method to GameRoom for recording collection entries:

```rust
    /// Record a collection log entry for a player. Returns true if this was a new entry.
    pub async fn record_collection_entry(
        &self,
        player_id: &str,
        item_id: &str,
        source: &str,
        source_detail: &str,
    ) -> bool {
        let key = (item_id.to_string(), source.to_string());

        // Check + insert in-memory
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                if !player.collection_log.insert(key) {
                    return false; // Already had it
                }
            } else {
                return false;
            }
        }

        // Persist to DB
        let obtained_at = chrono::Utc::now().to_rfc3339();
        if let Some(ref db) = self.db {
            let account_id = {
                let players = self.players.read().await;
                players.get(player_id).map(|p| p.account_id)
            };
            if let Some(account_id) = account_id {
                if let Err(e) = db
                    .save_collection_entry(account_id, item_id, source, source_detail, &obtained_at)
                    .await
                {
                    tracing::warn!("Failed to save collection entry for {}: {}", player_id, e);
                }
            }
        }

        // Send real-time notification to client
        self.send_to_player(
            player_id,
            ServerMessage::CollectionLogEntry {
                item_id: item_id.to_string(),
                source: source.to_string(),
                source_detail: source_detail.to_string(),
                obtained_at,
            },
        )
        .await;

        true
    }
```

Also add a getter for loading on connect:

```rust
    pub async fn get_player_collection_log(
        &self,
        player_id: &str,
    ) -> HashSet<(String, String)> {
        let players = self.players.read().await;
        players
            .get(player_id)
            .map(|p| p.collection_log.clone())
            .unwrap_or_default()
    }

    pub async fn set_player_collection_log(
        &self,
        player_id: &str,
        log: HashSet<(String, String)>,
    ) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.collection_log = log;
        }
    }
```

**Step 3: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 4: Commit**

```bash
git add rust-server/src/game.rs
git commit -m "feat: add collection log in-memory state and recording helper"
```

---

### Task 5: Load on Connect + Send Definitions & Sync

**Files:**
- Modify: `rust-server/src/main.rs` (matchmaking handler ~line 1457, WebSocket handler ~line 2292)

**Step 1: Load collection log from DB on matchmake**

After the unlocked spells loading block (around line 1479), add:

```rust
    // Load collection log from database
    match state.db.load_collection_log(character_id).await {
        Ok(entries) => {
            let count = entries.len();
            let log_set: std::collections::HashSet<(String, String)> = entries
                .iter()
                .map(|(item_id, source, _, _)| (item_id.clone(), source.clone()))
                .collect();
            room.set_player_collection_log(&player_id, log_set).await;
            if count > 0 {
                info!(
                    "Loaded {} collection log entries for {}",
                    count, character_data.name
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load collection log for character {}: {}",
                character_id,
                e
            );
        }
    }
```

**Step 2: Send definitions and sync on WebSocket connect**

After the prayer state send (around line 2333), add:

```rust
                            // Collection log definitions
                            let clog_defs_msg = ServerMessage::CollectionLogDefinitions {
                                entries: recv_state.collection_log_defs.all_entries(),
                            };
                            if let Ok(bytes) = protocol::encode_server_message(&clog_defs_msg) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Collection log sync (player's obtained entries)
                            if let Some(ref db) = recv_state.db {
                                // Grab from player's in-memory state
                                let log_entries = {
                                    let players = recv_room.players.read().await;
                                    if let Some(player) = players.get(&player_id) {
                                        // Need to fetch full entries with timestamps from DB
                                        drop(players);
                                        match db.load_collection_log(recv_character_id).await {
                                            Ok(entries) => entries,
                                            Err(_) => vec![],
                                        }
                                    } else {
                                        drop(players);
                                        vec![]
                                    }
                                };
                                let clog_sync = ServerMessage::CollectionLogSync {
                                    entries: log_entries,
                                };
                                if let Ok(bytes) = protocol::encode_server_message(&clog_sync) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }
```

Note: `recv_character_id` may need to be captured in the closure — check how `character_id` is available in that scope and adjust accordingly.

**Step 3: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 4: Commit**

```bash
git add rust-server/src/main.rs
git commit -m "feat: load and send collection log on player connect"
```

---

### Task 6: Hook Monster Drops

**Files:**
- Modify: `rust-server/src/game.rs` (~line 5087, item pickup)

**Step 1: Add collection recording after item pickup**

Find the monster loot item pickup code (around line 5087-5092 where `add_item` is called for picked-up ground items). The key is to identify when an item was dropped by a monster kill vs. player drop. 

Look for where `NpcDied` is handled and loot is generated. After loot items are spawned on the ground (around line 4612-4619 `generate_loot_from_prototype`), record each item. The ideal hook is right after the loot generation loop:

```rust
// After each loot item is generated from an NPC kill:
self.record_collection_entry(
    player_id,
    &item_id,
    "monster_drops",
    &prototype_id,
)
.await;
```

Find the exact spot where individual loot items are determined (item_id and quantity) from the NPC prototype, and add the recording call there. The `prototype_id` (NPC type like "pig") serves as `source_detail`.

**Step 2: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 3: Commit**

```bash
git add rust-server/src/game.rs
git commit -m "feat: record collection log entries for monster drops"
```

---

### Task 7: Hook Boss Rewards

**Files:**
- Modify: `rust-server/src/game/boss_tick.rs` (~line 629-645, loot rolling)

**Step 1: Add collection recording after boss loot is rolled**

In the `BossDied` handler where loot is rolled for each damage dealer (around line 629-645), after each item is determined for a player, record it:

```rust
// After each boss loot item is rolled for a player:
// (This needs to be done outside the sync context — collect items first, record after)
```

Since boss loot uses a pending rewards system, the best hook may be where rewards are actually claimed and added to bank (around line 1180-1187). After `bank.add_item()`:

```rust
self.record_collection_entry(
    player_id,
    item_id,
    "boss_rewards",
    &boss_prototype_id,
)
.await;
```

Find the boss prototype ID that's available in scope — it may be stored in the pending rewards or accessible from the NPC/dialogue context.

**Step 2: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 3: Commit**

```bash
git add rust-server/src/game/boss_tick.rs
git commit -m "feat: record collection log entries for boss rewards"
```

---

### Task 8: Hook Skilling (Fishing, Mining, Woodcutting, Crafting)

**Files:**
- Modify: `rust-server/src/game/resources.rs` (fishing ~755, mining ~524, woodcutting ~296)
- Modify: `rust-server/src/game/crafting.rs` (~697-705)

**Step 1: Hook fishing**

After the fishing `add_item` call (around line 762 in resources.rs):

```rust
// After: player.inventory.add_item(&result.item_id, 1, &self.item_registry);
// Need to record outside the players write lock — collect player_id + item_id, then record after dropping lock
```

**Important pattern:** Since `record_collection_entry` needs to acquire the players write lock, you CANNOT call it while already holding the lock. Collect the (player_id, item_id) pairs from the gather results, drop the players lock, then loop and record:

```rust
// After the players write lock block:
for result in &gather_results {
    self.record_collection_entry(
        &result.pid,
        &result.item_id,
        "skilling",
        "fishing",
    )
    .await;
}
```

**Step 2: Hook mining**

Same pattern after `handle_mine_rock` adds ore to inventory (around line 529):

```rust
// After dropping the players write lock:
self.record_collection_entry(player_id, &result.ore_item_id, "skilling", "mining").await;
```

**Step 3: Hook woodcutting**

Same pattern after `handle_chop_tree` adds logs (around line 301):

```rust
// After dropping the players write lock:
self.record_collection_entry(player_id, &result.log_item_id, "skilling", "woodcutting").await;
```

**Step 4: Hook crafting (cooking/smithing/alchemy)**

In `crafting.rs`, after successful craft results are added to inventory (around line 701-705). Determine the skill from the recipe's `station_type` or category:

```rust
// After dropping the players write lock:
let skill_name = match recipe.station_type.as_str() {
    "fire_pit" | "range" => "cooking",
    "furnace" => "smithing",
    "alchemy_station" => "alchemy",
    _ => "crafting",
};
for result in &recipe.results {
    self.record_collection_entry(player_id, &result.item_id, "skilling", skill_name).await;
}
```

**Step 5: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 6: Commit**

```bash
git add rust-server/src/game/resources.rs rust-server/src/game/crafting.rs
git commit -m "feat: record collection log entries for skilling activities"
```

---

### Task 9: Hook Quest Rewards

**Files:**
- Modify: `rust-server/src/game/quests.rs` (~line 313-319)

**Step 1: Add collection recording after quest reward items**

After the quest reward items loop (around line 319, after `add_item`), and after the players write lock is dropped:

```rust
// After granting rewards and dropping lock:
for item_reward in &quest.rewards.items {
    self.record_collection_entry(
        player_id,
        &item_reward.item_id,
        "quest_rewards",
        quest_id,
    )
    .await;
}
```

**Step 2: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 3: Commit**

```bash
git add rust-server/src/game/quests.rs
git commit -m "feat: record collection log entries for quest rewards"
```

---

### Task 10: Client — State + Message Handling

**Files:**
- Modify: `client/src/game/state.rs` (~line 1476)
- Modify: `client/src/network/message_handler.rs`

**Step 1: Add collection log state fields**

After `selected_quest_id` (line 1476 in state.rs), add:

```rust
    // Collection log UI state
    pub collection_tab_active: bool,  // false = quests tab, true = collection tab
    /// Static definitions: Vec of (item_id, source, source_detail)
    pub collection_log_definitions: Vec<(String, String, String)>,
    /// Player's obtained items: HashMap of (item_id, source) -> obtained_at
    pub collection_log: std::collections::HashMap<(String, String), String>,
    pub collection_category: Option<String>,      // "monster_drops", "boss_rewards", "skilling", "quest_rewards"
    pub collection_subcategory: Option<String>,    // e.g., "pig", "fishing", "axe_to_grind"
    pub collection_scroll: f32,
    pub collection_scroll_drag: crate::ui::scroll::ScrollDragState,
```

Initialize them with defaults in the UiState constructor (find the `Default` impl or constructor):

```rust
    collection_tab_active: false,
    collection_log_definitions: Vec::new(),
    collection_log: std::collections::HashMap::new(),
    collection_category: None,
    collection_subcategory: None,
    collection_scroll: 0.0,
    collection_scroll_drag: Default::default(),
```

**Step 2: Handle incoming messages in message_handler.rs**

Add three new match arms in the message routing (alongside `"questCompleted"` etc.):

```rust
"collectionLogDefinitions" => {
    if let Some(value) = data {
        if let Some(entries_val) = extract_map_field(value, "entries") {
            if let rmpv::Value::Array(arr) = entries_val {
                let mut defs = Vec::new();
                for entry in arr {
                    if let rmpv::Value::Array(fields) = entry {
                        if fields.len() >= 3 {
                            let item_id = fields[0].as_str().unwrap_or("").to_string();
                            let source = fields[1].as_str().unwrap_or("").to_string();
                            let source_detail = fields[2].as_str().unwrap_or("").to_string();
                            defs.push((item_id, source, source_detail));
                        }
                    }
                }
                log::info!("Received {} collection log definitions", defs.len());
                state.ui_state.collection_log_definitions = defs;
            }
        }
    }
}
"collectionLogSync" => {
    if let Some(value) = data {
        if let Some(entries_val) = extract_map_field(value, "entries") {
            if let rmpv::Value::Array(arr) = entries_val {
                let mut log = std::collections::HashMap::new();
                for entry in arr {
                    if let rmpv::Value::Array(fields) = entry {
                        if fields.len() >= 4 {
                            let item_id = fields[0].as_str().unwrap_or("").to_string();
                            let source = fields[1].as_str().unwrap_or("").to_string();
                            let obtained_at = fields[3].as_str().unwrap_or("").to_string();
                            log.insert((item_id, source), obtained_at);
                        }
                    }
                }
                log::info!("Synced {} collection log entries", log.len());
                state.ui_state.collection_log = log;
            }
        }
    }
}
"collectionLogEntry" => {
    if let Some(value) = data {
        let item_id = extract_string(value, "item_id").unwrap_or_default();
        let source = extract_string(value, "source").unwrap_or_default();
        let obtained_at = extract_string(value, "obtained_at").unwrap_or_default();

        log::info!("New collection log entry: {} from {}", item_id, source);
        state.ui_state.collection_log.insert(
            (item_id.clone(), source),
            obtained_at,
        );

        // Show notification
        state.push_system_chat(format!("New collection log entry: {}!", item_id));
        state.pending_sfx.push("enter".to_string());
    }
}
```

**Step 3: Verify it compiles**

Run: `cd client && cargo check 2>&1 | head -20`

**Step 4: Commit**

```bash
git add client/src/game/state.rs client/src/network/message_handler.rs
git commit -m "feat: add collection log client state and message handling"
```

---

### Task 11: Client — Tabbed Quest/Collection Panel UI

**Files:**
- Modify: `client/src/render/ui/quest.rs` (main rendering)
- Modify: `client/src/ui/layout.rs` (new UiElementId variants)
- Modify: `client/src/input/handler.rs` (click handling)

**Step 1: Add UiElementId variants**

In `client/src/ui/layout.rs`, add after the quest-related variants (around line 38):

```rust
    // Collection Log
    CollectionLogTab,
    QuestsTab,
    CollectionLogCategory(usize),
    CollectionLogSubcategory(usize),
    CollectionLogBack,
    CollectionLogScrollArea,
    CollectionLogScrollbar,
```

**Step 2: Modify render_quest_log to add tabs**

In `client/src/render/ui/quest.rs`, modify `render_quest_log` to:

1. Replace the hardcoded "Quests" title (line 95) with two tab buttons:

```rust
        // Tab bar
        let tab_w = header_w / 2.0;
        let quests_tab_hovered = matches!(hovered, Some(UiElementId::QuestsTab));
        let clog_tab_hovered = matches!(hovered, Some(UiElementId::CollectionLogTab));

        let quests_active = !state.ui_state.collection_tab_active;
        let clog_active = state.ui_state.collection_tab_active;

        // Quests tab
        let qtab_bg = if quests_active { HEADER_BG } else if quests_tab_hovered { Color::new(0.25, 0.22, 0.18, 1.0) } else { Color::new(0.15, 0.13, 0.10, 1.0) };
        draw_rectangle(header_x, header_y, tab_w, header_h, qtab_bg);
        let qt_text = "Quests";
        let qt_w = self.measure_text_sharp(qt_text, 16.0).width;
        self.draw_text_sharp(
            qt_text,
            (header_x + (tab_w - qt_w) / 2.0).floor(),
            (header_y + 17.0 * s).floor(),
            16.0,
            if quests_active { TEXT_TITLE } else { TEXT_DIM },
        );
        layout.register(UiElementId::QuestsTab, macroquad::math::Rect::new(header_x, header_y, tab_w, header_h));

        // Collection Log tab
        let ctab_bg = if clog_active { HEADER_BG } else if clog_tab_hovered { Color::new(0.25, 0.22, 0.18, 1.0) } else { Color::new(0.15, 0.13, 0.10, 1.0) };
        draw_rectangle(header_x + tab_w, header_y, tab_w, header_h, ctab_bg);
        let ct_text = "Collection";
        let ct_w = self.measure_text_sharp(ct_text, 16.0).width;
        self.draw_text_sharp(
            ct_text,
            (header_x + tab_w + (tab_w - ct_w) / 2.0).floor(),
            (header_y + 17.0 * s).floor(),
            16.0,
            if clog_active { TEXT_TITLE } else { TEXT_DIM },
        );
        layout.register(UiElementId::CollectionLogTab, macroquad::math::Rect::new(header_x + tab_w, header_y, tab_w, header_h));
```

2. After the header, branch on `collection_tab_active`:

```rust
        if state.ui_state.collection_tab_active {
            self.render_collection_log(state, hovered, layout, panel_x, panel_y, panel_width, panel_height, header_h, footer_h, line_height, entry_padding);
        } else {
            // Existing quest list/detail rendering (move existing code here)
        }
```

**Step 3: Implement render_collection_log**

Add a new method to the renderer. This is the biggest piece of UI work. It should follow the existing quest list rendering pattern (scroll area, entries, click targets):

```rust
fn render_collection_log(
    &self,
    state: &GameState,
    hovered: &Option<UiElementId>,
    layout: &mut UiLayout,
    panel_x: f32, panel_y: f32, panel_width: f32, panel_height: f32,
    header_h: f32, footer_h: f32, line_height: f32, entry_padding: f32,
) {
    let s = state.ui_state.ui_scale;
    let frame_thickness = FRAME_THICKNESS * s;
    let content_x = panel_x + frame_thickness + 8.0 * s;
    let content_y = panel_y + frame_thickness + header_h + 4.0 * s;
    let content_w = panel_width - frame_thickness * 2.0 - 16.0 * s;
    let content_h = panel_height - frame_thickness * 2.0 - header_h - footer_h - 8.0 * s;

    let defs = &state.ui_state.collection_log_definitions;
    let obtained = &state.ui_state.collection_log;

    if let Some(ref subcat) = state.ui_state.collection_subcategory {
        // Item list view — show items within a subcategory
        self.render_collection_items(state, hovered, layout, content_x, content_y, content_w, content_h, line_height, entry_padding);
    } else if let Some(ref category) = state.ui_state.collection_category {
        // Subcategory view — show sub-sections within a category
        self.render_collection_subcategories(state, hovered, layout, content_x, content_y, content_w, content_h, line_height, entry_padding);
    } else {
        // Top-level category view
        self.render_collection_categories(state, hovered, layout, content_x, content_y, content_w, content_h, line_height, entry_padding);
    }

    // Back button (if drilled in)
    if state.ui_state.collection_category.is_some() {
        // Draw "< Back" at top of content area
        let back_text = "< Back";
        let back_w = self.measure_text_sharp(back_text, 14.0).width + 8.0 * s;
        let back_rect = macroquad::math::Rect::new(content_x, content_y - line_height, back_w, line_height);
        let back_hovered = matches!(hovered, Some(UiElementId::CollectionLogBack));
        if back_hovered {
            draw_rectangle(back_rect.x, back_rect.y, back_rect.w, back_rect.h, Color::new(0.3, 0.25, 0.2, 0.5));
        }
        self.draw_text_sharp(back_text, content_x + 4.0 * s, content_y - 3.0 * s, 14.0, TEXT_HIGHLIGHT);
        layout.register(UiElementId::CollectionLogBack, back_rect);
    }

    // Footer with total completion
    let total_possible = defs.len();
    let total_obtained = obtained.len();
    let footer_text = format!("{} / {} Collected", total_obtained, total_possible);
    // Draw footer (follow existing quest footer pattern)
}
```

**The category, subcategory, and item renderers** should each:
1. Build a list of entries from `collection_log_definitions`, grouped appropriately
2. Count obtained vs total for each
3. Render scrollable entries using the same pattern as quest list (scissor clipping, scroll state, hover effects)
4. Register click targets with appropriate `UiElementId` variants

**Step 4: Handle clicks in input/handler.rs**

Add click handlers for the new UI elements:

```rust
UiElementId::QuestsTab => {
    audio.play_sfx("enter");
    state.ui_state.collection_tab_active = false;
}
UiElementId::CollectionLogTab => {
    audio.play_sfx("enter");
    state.ui_state.collection_tab_active = true;
    state.ui_state.collection_category = None;
    state.ui_state.collection_subcategory = None;
    state.ui_state.collection_scroll = 0.0;
}
UiElementId::CollectionLogCategory(idx) => {
    audio.play_sfx("enter");
    let categories = ["monster_drops", "boss_rewards", "skilling", "quest_rewards"];
    if let Some(cat) = categories.get(*idx) {
        state.ui_state.collection_category = Some(cat.to_string());
        state.ui_state.collection_subcategory = None;
        state.ui_state.collection_scroll = 0.0;
    }
}
UiElementId::CollectionLogSubcategory(idx) => {
    audio.play_sfx("enter");
    // Determine subcategory from current category + index
    // Build sorted subcategory list matching render order, select by idx
    state.ui_state.collection_scroll = 0.0;
}
UiElementId::CollectionLogBack => {
    audio.play_sfx("enter");
    if state.ui_state.collection_subcategory.is_some() {
        state.ui_state.collection_subcategory = None;
    } else {
        state.ui_state.collection_category = None;
    }
    state.ui_state.collection_scroll = 0.0;
}
```

**Step 5: Reset collection state when closing panel**

In `close_quest_log()`, add:

```rust
    self.collection_tab_active = false;
    self.collection_category = None;
    self.collection_subcategory = None;
    self.collection_scroll = 0.0;
```

**Step 6: Verify it compiles**

Run: `cd client && cargo check 2>&1 | head -20`

**Step 7: Commit**

```bash
git add client/src/render/ui/quest.rs client/src/ui/layout.rs client/src/input/handler.rs
git commit -m "feat: add tabbed collection log UI to quest panel"
```

---

### Task 12: Populate collection_log.toml with Real Data

**Files:**
- Modify: `rust-server/data/collection_log.toml`

**Step 1: Cross-reference all data sources**

Go through each data directory and populate the TOML completely:

1. Read every file in `rust-server/data/entities/monsters/` — extract all items from `loot` entries and `loot_tables` references
2. Read boss entity files — extract boss reward items
3. Read `rust-server/data/loot_tables.toml` — extract all fishing/mining/herb gathering items
4. Read `rust-server/data/recipes/*.toml` — extract all crafting output items, categorized by station type
5. Read `rust-server/data/quests/**/*.toml` — extract all `[rewards.items]` sections

**Step 2: Verify the TOML parses**

Run: `cd rust-server && cargo test -- --test-threads=1 2>&1 | head -20` or add a quick parse test.

**Step 3: Commit**

```bash
git add rust-server/data/collection_log.toml
git commit -m "feat: populate collection_log.toml with all obtainable items"
```

---

### Task 13: Integration Test

**Step 1: Manual testing checklist**

- [ ] Server starts without errors
- [ ] Login sends `collectionLogDefinitions` and `collectionLogSync` messages
- [ ] Killing a monster records a new entry and sends `collectionLogEntry`
- [ ] Fishing/mining/etc records entries
- [ ] Quest completion records reward entries
- [ ] Collection log tab appears in quest panel
- [ ] Tab switching works between Quests and Collection
- [ ] Categories show correct counts
- [ ] Drilling into categories shows subcategories
- [ ] Drilling into subcategories shows items (obtained vs ???)
- [ ] Back button navigates correctly
- [ ] Scrolling works in all views
- [ ] Relogging preserves collection state

**Step 2: Final commit**

```bash
git add -A
git commit -m "feat: collection log - complete implementation"
```
