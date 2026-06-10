# Master Crafting Orders Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a crafting commission system to the Adventure Board with two tiers (regular/masterwork), a Commission Marks currency, a Master Artisan prestige shop, and a generalized player title system displayed overhead and in chat.

**Architecture:** Extends the existing Adventure Board and resource contract patterns. Orders use TOML data templates and inventory-check turn-in (no partial progress tracking). Titles are a new generic system with DB persistence, `/title` chat commands, and wire-level propagation via `PlayerUpdate`.

**Tech Stack:** Rust (server: Axum/Tokio/SQLite/rmpv), Rust (client: Macroquad), TOML data files, MessagePack protocol.

---

### Task 1: Database Tables for Titles and Crafting Orders

**Files:**
- Modify: `rust-server/src/db.rs` (in the `migrate` function, after existing table creations ~line 733)

**Step 1: Add the new tables**

In `db.rs` `migrate()`, add after the last `CREATE TABLE IF NOT EXISTS` block:

```rust
// Player titles
sqlx::query(
    r#"
    CREATE TABLE IF NOT EXISTS player_titles (
        character_id INTEGER NOT NULL,
        title_id TEXT NOT NULL,
        unlocked_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (character_id, title_id),
        FOREIGN KEY(character_id) REFERENCES characters(id)
    )
    "#,
)
.execute(pool)
.await?;

// Crafting order daily offers
sqlx::query(
    r#"
    CREATE TABLE IF NOT EXISTS crafting_orders_available (
        character_id INTEGER NOT NULL,
        order_id TEXT NOT NULL,
        generated_date TEXT NOT NULL,
        PRIMARY KEY (character_id, order_id),
        FOREIGN KEY(character_id) REFERENCES characters(id)
    )
    "#,
)
.execute(pool)
.await?;

// Active crafting order (one per player)
sqlx::query(
    r#"
    CREATE TABLE IF NOT EXISTS crafting_orders_active (
        character_id INTEGER PRIMARY KEY,
        order_id TEXT NOT NULL,
        accepted_at INTEGER NOT NULL,
        FOREIGN KEY(character_id) REFERENCES characters(id)
    )
    "#,
)
.execute(pool)
.await?;

// Crafting order stats
sqlx::query(
    r#"
    CREATE TABLE IF NOT EXISTS crafting_order_stats (
        character_id INTEGER PRIMARY KEY,
        orders_completed INTEGER NOT NULL DEFAULT 0,
        masterwork_completed INTEGER NOT NULL DEFAULT 0,
        total_marks_earned INTEGER NOT NULL DEFAULT 0,
        FOREIGN KEY(character_id) REFERENCES characters(id)
    )
    "#,
)
.execute(pool)
.await?;
```

**Step 2: Add columns to characters table**

```rust
// Add active_title and commission_marks to characters
sqlx::query("ALTER TABLE characters ADD COLUMN active_title TEXT DEFAULT NULL")
    .execute(pool)
    .await
    .ok(); // .ok() because column may already exist

sqlx::query("ALTER TABLE characters ADD COLUMN commission_marks INTEGER DEFAULT 0")
    .execute(pool)
    .await
    .ok();
```

**Step 3: Add DB helper methods**

Add these methods to the `Database` impl in `db.rs`:

```rust
// Title DB methods
pub async fn get_player_titles(&self, character_id: i64) -> Vec<String>;
pub async fn unlock_title(&self, character_id: i64, title_id: &str);
pub async fn get_active_title(&self, character_id: i64) -> Option<String>;
pub async fn set_active_title(&self, character_id: i64, title_id: Option<&str>);

// Commission marks
pub async fn get_commission_marks(&self, character_id: i64) -> i32;
pub async fn add_commission_marks(&self, character_id: i64, amount: i32);
pub async fn spend_commission_marks(&self, character_id: i64, amount: i32) -> bool;

// Crafting orders
pub async fn get_available_orders(&self, character_id: i64, date: &str) -> Vec<String>;
pub async fn save_available_orders(&self, character_id: i64, date: &str, order_ids: &[String]);
pub async fn get_active_order(&self, character_id: i64) -> Option<String>;
pub async fn save_active_order(&self, character_id: i64, order_id: &str);
pub async fn remove_active_order(&self, character_id: i64);
pub async fn get_crafting_order_stats(&self, character_id: i64) -> (i32, i32, i32);
pub async fn increment_crafting_order_stats(&self, character_id: i64, is_masterwork: bool, marks: i32);
```

**Step 4: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`
Expected: compiles (warnings OK)

**Step 5: Commit**

```bash
git add rust-server/src/db.rs
git commit -m "feat: add database tables for player titles and crafting orders"
```

---

### Task 2: Player Title System — Server Core

**Files:**
- Create: `rust-server/src/game/titles.rs`
- Modify: `rust-server/src/game.rs` (Player struct, PlayerUpdate struct, mod declaration)
- Modify: `rust-server/src/game/tick_snapshots.rs` (player_update_from_player)
- Modify: `rust-server/src/protocol.rs` (player_update_to_value)

**Step 1: Create titles.rs with title definitions and /title command handler**

Create `rust-server/src/game/titles.rs`:

```rust
use crate::game::GameRoom;

/// All known title definitions. The display text is what players see.
pub struct TitleDef {
    pub id: &'static str,
    pub display: &'static str,
}

pub const TITLES: &[TitleDef] = &[
    // Crafting titles (purchased with Commission Marks)
    TitleDef { id: "artisan_apprentice", display: "Apprentice Artisan" },
    TitleDef { id: "master_smith", display: "Master Smith" },
    TitleDef { id: "master_alchemist", display: "Master Alchemist" },
    TitleDef { id: "master_fletcher", display: "Master Fletcher" },
    TitleDef { id: "master_chef", display: "Master Chef" },
    TitleDef { id: "grandmaster_artisan", display: "Grandmaster Artisan" },
    // Arena titles (unlocked by milestones)
    TitleDef { id: "arena_novice", display: "Brawler" },
    TitleDef { id: "arena_fighter", display: "Fighter" },
    TitleDef { id: "arena_veteran", display: "Veteran" },
    TitleDef { id: "arena_champion", display: "Champion" },
    TitleDef { id: "arena_legend", display: "Legend" },
];

pub fn title_display(title_id: &str) -> Option<&'static str> {
    TITLES.iter().find(|t| t.id == title_id).map(|t| t.display)
}

impl GameRoom {
    pub async fn handle_title_command(&self, player_id: &str, args: &[&str]) {
        match args.first().copied() {
            Some("list") => {
                let character_id = self.player_character_id(player_id).await;
                let unlocked = self.db.get_player_titles(character_id).await;
                if unlocked.is_empty() {
                    self.send_system_message(player_id, "You have no titles unlocked yet.").await;
                } else {
                    let list: Vec<String> = unlocked.iter().filter_map(|id| {
                        title_display(id).map(|d| format!("  {} ({})", d, id))
                    }).collect();
                    let msg = format!("Your titles:\n{}", list.join("\n"));
                    self.send_system_message(player_id, &msg).await;
                }
            }
            Some("set") => {
                let title_id = args.get(1).copied().unwrap_or("");
                if title_id.is_empty() {
                    self.send_system_message(player_id, "Usage: /title set <title_id>").await;
                    return;
                }
                let character_id = self.player_character_id(player_id).await;
                let unlocked = self.db.get_player_titles(character_id).await;
                if !unlocked.contains(&title_id.to_string()) {
                    self.send_system_message(player_id, "You haven't unlocked that title.").await;
                    return;
                }
                let display = match title_display(title_id) {
                    Some(d) => d,
                    None => { self.send_system_message(player_id, "Unknown title.").await; return; }
                };
                self.db.set_active_title(character_id, Some(title_id)).await;
                // Update in-memory Player
                {
                    let mut players = self.players.write().await;
                    if let Some(p) = players.get_mut(player_id) {
                        p.active_title = Some(display.to_string());
                    }
                }
                self.send_system_message(player_id, &format!("Title set: {}", display)).await;
            }
            Some("clear") => {
                let character_id = self.player_character_id(player_id).await;
                self.db.set_active_title(character_id, None).await;
                {
                    let mut players = self.players.write().await;
                    if let Some(p) = players.get_mut(player_id) {
                        p.active_title = None;
                    }
                }
                self.send_system_message(player_id, "Title cleared.").await;
            }
            _ => {
                self.send_system_message(player_id, "Usage: /title list | /title set <id> | /title clear").await;
            }
        }
    }
}
```

**Step 2: Add `active_title` field to Player struct**

In `rust-server/src/game.rs`, add to the `Player` struct (after `is_admin` field ~line 433):

```rust
pub active_title: Option<String>, // Display text of equipped title
```

In `Player::new()`, initialize:
```rust
active_title: None,
```

**Step 3: Add `title` field to PlayerUpdate struct**

In `rust-server/src/game.rs`, add to `PlayerUpdate` (~line 1140, after `combat_style`):

```rust
pub title: Option<String>,
```

**Step 4: Wire title into player_update_from_player**

In `rust-server/src/game/tick_snapshots.rs`, add to the `PlayerUpdate` construction (~line 84, after `combat_style`):

```rust
title: player.active_title.clone(),
```

Also update the test `player_update_from_player_preserves_ack_and_active_stall` to include `active_title: None` when constructing the player (if needed) and assert on the update's title field.

**Step 5: Wire title into protocol serialization**

In `rust-server/src/protocol.rs` `player_update_to_value()`, add before the final `Value::Map(pmap)` (~line 2130):

```rust
pmap.push((
    Value::String("title".into()),
    match &p.title {
        Some(t) => Value::String(t.clone().into()),
        None => Value::Nil,
    },
));
```

**Step 6: Add `mod titles;` to game.rs and wire /title command in chat.rs**

In `rust-server/src/game.rs`, add `pub mod titles;` with the other mod declarations.

In `rust-server/src/game/chat.rs` `handle_chat_command()`, add a match arm:

```rust
"/title" => {
    self.handle_title_command(player_id, &parts[1..]).await;
}
```

**Step 7: Load active_title at login**

Find where Player is loaded from DB on login (likely in `db.rs` or the login/character-select flow). Load `active_title` from the `characters` table and convert the stored `title_id` to display text using `title_display()`. Set `player.active_title` accordingly.

**Step 8: Include title in chat sender_name**

In `rust-server/src/game/chat.rs` `handle_chat()`, after getting `sender_name` (~line 56), append the title:

```rust
let sender_name = if let Some(ref title) = sender.active_title {
    format!("{} ({})", sender.name, title)
} else {
    sender.name.clone()
};
```

**Step 9: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 10: Commit**

```bash
git add rust-server/src/game/titles.rs rust-server/src/game.rs rust-server/src/game/tick_snapshots.rs rust-server/src/protocol.rs rust-server/src/game/chat.rs rust-server/src/db.rs
git commit -m "feat: add player title system with /title commands and wire format"
```

---

### Task 3: Player Title System — Client Rendering

**Files:**
- Modify: `client/src/game/entities.rs` (Player struct — add `title` field)
- Modify: `client/src/network/message_handler.rs` (deserialize title from PlayerUpdate and ChatMessage)
- Modify: `client/src/render/renderer.rs` (render title in name tags)

**Step 1: Add `title` field to client Player struct**

In `client/src/game/entities.rs`, add to the Player struct (~line 96):

```rust
pub title: Option<String>,
```

Initialize to `None` in `Player::new()` or wherever players are constructed.

**Step 2: Deserialize title from StateSync PlayerUpdate**

In `client/src/network/message_handler.rs` `handle_state_sync()` (~line 356), after extracting existing fields, add:

```rust
let title = extract_string(player_value, "title");
```

Set `player.title = title;` when constructing/updating the client Player.

**Step 3: Render title in overhead name tags**

In `client/src/render/renderer.rs` `render_name_tags()` (~line 5443), modify the name display. Currently it shows `name` + `" (Lvl N)"`. Change to include title:

```rust
// After line 5443 where level_text is built:
let title_text = player.title.as_ref().map(|t| format!(" ({})", t)).unwrap_or_default();
let title_width = if title_text.is_empty() {
    0.0
} else {
    self.measure_text_sharp(&title_text, font_size).width
};
// Update total_width calculation to include title_width
let total_width = trophy_width + name_width + title_width + level_width;
```

Then draw the title between name and level, in a gold color:

```rust
// After drawing the player name (~line 5497-5503):
if !title_text.is_empty() {
    let title_color = Color::from_rgba(255, 215, 100, 255); // gold
    self.draw_text_sharp(
        &title_text,
        name_x + trophy_width + name_width,
        name_y,
        font_size,
        title_color,
    );
}
// Adjust level text x position to account for title
self.draw_text_sharp(
    &level_text,
    name_x + trophy_width + name_width + title_width,
    name_y,
    font_size,
    level_color,
);
```

**Step 4: Verify it compiles**

Run: `cd client && cargo check 2>&1 | head -20`

**Step 5: Commit**

```bash
git add client/src/game/entities.rs client/src/network/message_handler.rs client/src/render/renderer.rs
git commit -m "feat: render player titles overhead and in chat on client"
```

---

### Task 4: Crafting Order Data Templates

**Files:**
- Create: `rust-server/data/orders/smithing.toml`
- Create: `rust-server/data/orders/alchemy.toml`
- Create: `rust-server/data/orders/cooking.toml`
- Create: `rust-server/data/orders/fletching.toml`
- Create: `rust-server/data/orders/mining.toml`
- Create: `rust-server/data/orders/woodcutting.toml`
- Create: `rust-server/data/orders/fishing.toml`
- Create: `rust-server/data/orders/leatherworking.toml`

**Step 1: Create the data/orders/ directory and template files**

Before writing templates, check existing items and recipes to use real item IDs. Look at:
- `rust-server/data/items/*.toml` for valid item IDs
- `rust-server/data/recipes/*.toml` for what players can craft
- `rust-server/data/resources/*.toml` for gatherable items

Each TOML file follows this format:

```toml
[[orders]]
id = "unique_order_id"
tier = "regular"      # or "masterwork"
skill = "smithing"    # primary skill
min_level = 15
items = [{ id = "iron_dagger", quantity = 20 }]
rewards = { gold = 500, xp = { smithing = 300 } }

[[orders]]
id = "masterwork_example"
tier = "masterwork"
skill = "smithing"
min_level = 40
items = [{ id = "mithril_longsword", quantity = 5 }]
rewards = { gold = 2000, xp = { smithing = 800, mining = 400 }, marks = 3 }
```

Create 3-5 regular orders and 2-3 masterwork orders per skill file. Use real item IDs from the existing data files. Regular orders should target items that a single skill can produce. Masterwork orders should target items that require outputs from 2-3 skills.

**Step 2: Commit**

```bash
git add rust-server/data/orders/
git commit -m "feat: add crafting order TOML templates for all skill families"
```

---

### Task 5: Crafting Order System — Server Core

**Files:**
- Create: `rust-server/src/game/crafting_orders.rs`
- Modify: `rust-server/src/game.rs` (add mod declaration, add CraftingOrderManager to GameRoom)

**Step 1: Create crafting_orders.rs with data loading and order logic**

```rust
// Key types:
pub struct OrderTemplate {
    pub id: String,
    pub tier: OrderTier,       // Regular or Masterwork
    pub skill: String,
    pub min_level: i32,
    pub items: Vec<OrderItem>, // item_id + quantity
    pub rewards: OrderRewards, // gold, xp map, optional marks
}

pub enum OrderTier { Regular, Masterwork }

pub struct OrderItem { pub id: String, pub quantity: i32 }

pub struct OrderRewards {
    pub gold: i32,
    pub xp: HashMap<String, i64>,
    pub marks: i32,
}

pub struct CraftingOrderRegistry {
    pub orders: Vec<OrderTemplate>,
}
```

Implement:
- `CraftingOrderRegistry::load(data_path: &str)` — reads all `data/orders/*.toml` files, parses into `OrderTemplate` vec
- `CraftingOrderRegistry::generate_daily_orders(player_skills: &HashMap<String, i32>, date: &str) -> Vec<String>` — filters eligible orders by player skill levels, picks 3-5 randomly, ensures at least 1 regular and 1 masterwork if eligible
- `CraftingOrderRegistry::get_order(id: &str) -> Option<&OrderTemplate>` — lookup by ID

**Step 2: Add order accept/turn-in/abandon handlers on GameRoom**

```rust
impl GameRoom {
    pub async fn handle_accept_crafting_order(&self, player_id: &str, order_id: &str);
    pub async fn handle_claim_crafting_order(&self, player_id: &str);
    pub async fn handle_abandon_crafting_order(&self, player_id: &str);
}
```

Accept: validate player has no active order, validate skill level, save to DB.
Claim: check inventory has required items, remove items, grant gold + XP + marks, increment stats, remove active order.
Abandon: remove active order from DB.

**Step 3: Add mod declaration and field to GameRoom**

In `rust-server/src/game.rs`:
- Add `pub mod crafting_orders;`
- Add `pub crafting_order_registry: crafting_orders::CraftingOrderRegistry` to `GameRoom`
- Load it in `GameRoom::new()` alongside other data loading

**Step 4: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 5: Commit**

```bash
git add rust-server/src/game/crafting_orders.rs rust-server/src/game.rs
git commit -m "feat: add crafting order registry and accept/claim/abandon handlers"
```

---

### Task 6: Adventure Board — Orders Tab Integration

**Files:**
- Modify: `rust-server/src/protocol.rs` (add CraftingOrdersState message types)
- Modify: `rust-server/src/game/resource_contracts.rs` (extend adventure board to include orders tab data)
- Modify: `rust-server/src/game.rs` (wire new dialogue choices for orders)

**Step 1: Add protocol types for orders tab**

In `rust-server/src/protocol.rs`, add:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CraftingOrderOfferData {
    pub order_id: String,
    pub tier: String,        // "regular" or "masterwork"
    pub skill: String,
    pub min_level: i32,
    pub items: Vec<CraftingOrderItemData>,
    pub reward_gold: i32,
    pub reward_xp: HashMap<String, i64>,
    pub reward_marks: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CraftingOrderItemData {
    pub item_id: String,
    pub item_name: String,
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CraftingOrderActiveData {
    pub order_id: String,
    pub tier: String,
    pub skill: String,
    pub items: Vec<CraftingOrderItemData>,
    pub reward_gold: i32,
    pub reward_marks: i32,
    pub can_claim: bool,  // true if player has all items in inventory
}

#[derive(Debug, Clone, Serialize)]
pub struct CraftingOrderStatsData {
    pub orders_completed: i32,
    pub masterwork_completed: i32,
    pub commission_marks: i32,
}
```

**Step 2: Extend AdventureBoardState to include crafting orders**

Either add fields to the existing `AdventureBoardState` ServerMessage variant, or send a separate `CraftingOrdersState` message when the board is opened. The simpler approach is to add to the existing message:

Add to `ServerMessage::AdventureBoardState`:
```rust
crafting_orders: Vec<CraftingOrderOfferData>,
crafting_order_active: Option<CraftingOrderActiveData>,
crafting_order_stats: CraftingOrderStatsData,
```

Update `show_adventure_board_dialogue()` to populate these fields by calling into the crafting order system.

**Step 3: Wire dialogue choices for orders tab**

In `rust-server/src/game.rs`, where board dialogue choices are handled (~line 5728), add handling for:
- `"order_accept:<order_id>"` → `handle_accept_crafting_order()`
- `"order_claim"` → `handle_claim_crafting_order()`
- `"order_abandon"` → `handle_abandon_crafting_order()`

After each action, refresh the board state by re-sending `AdventureBoardState`.

**Step 4: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 5: Commit**

```bash
git add rust-server/src/protocol.rs rust-server/src/game/resource_contracts.rs rust-server/src/game.rs
git commit -m "feat: integrate crafting orders into adventure board protocol and dialogue"
```

---

### Task 7: Adventure Board — Client Orders Tab UI

**Files:**
- Modify: `client/src/render/ui/adventure_board.rs` (add Orders tab rendering)
- Modify: `client/src/network/message_handler.rs` (deserialize crafting order data)
- Modify: `client/src/input/handler.rs` (handle Orders tab interactions)
- Modify: `client/src/game/state.rs` or wherever `AdventureBoardPanelState` lives (add crafting order fields)

**Step 1: Extend client state to hold crafting order data**

Add to the adventure board panel state:

```rust
pub crafting_orders: Vec<CraftingOrderOffer>,
pub crafting_order_active: Option<CraftingOrderActive>,
pub crafting_order_stats: CraftingOrderStats,
pub board_tab: BoardTab, // Contracts | Orders
```

**Step 2: Deserialize crafting order data from AdventureBoardState**

In `message_handler.rs`, extend the `AdventureBoardState` handler to parse the new fields.

**Step 3: Add tab switcher to adventure board UI**

In `adventure_board.rs`, add a tab bar at the top of the panel with two tabs: "CONTRACTS" and "ORDERS". The existing contract UI renders when "CONTRACTS" is selected. A new `render_orders_tab()` function renders when "ORDERS" is selected.

**Step 4: Implement render_orders_tab()**

Layout similar to existing contracts tab:
- Left column: list of available orders (skill icon, tier badge, item names)
- Center: selected order detail (items required with quantities, rewards breakdown)
- Right: active order panel (if any) with claim/abandon buttons
- Bottom: stats bar (orders completed, masterwork completed, marks balance)

Use the existing `kind_accent()` color function for skill-based coloring. Add accent colors for new skills if needed.

For the "Accept" button, emit dialogue choice `"order_accept:<order_id>"`.
For "Claim", emit `"order_claim"`.
For "Abandon", emit `"order_abandon"`.

**Step 5: Verify it compiles**

Run: `cd client && cargo check 2>&1 | head -20`

**Step 6: Commit**

```bash
git add client/src/render/ui/adventure_board.rs client/src/network/message_handler.rs client/src/input/handler.rs client/src/game/
git commit -m "feat: add Orders tab to adventure board client UI"
```

---

### Task 8: Master Artisan NPC — Prestige Shop

**Files:**
- Create: `rust-server/data/npcs/master_artisan.toml` (or add to existing NPC data)
- Create: `rust-server/data/shops/master_artisan.toml` (or equivalent shop data)
- Modify: `rust-server/src/game/shop.rs` (support Commission Marks as currency)
- Modify: server shop/NPC interaction code to handle marks-based purchases

**Step 1: Define the Master Artisan NPC entity**

Add to the NPC data files (check existing NPC TOML format in `rust-server/data/npcs/`). Place the NPC near the village crafting area.

**Step 2: Define the prestige shop inventory**

The shop sells titles and cosmetics for Commission Marks. Items:

| Item | Cost (Marks) | Type |
|------|-------------|------|
| Apprentice Artisan title | 10 | title unlock |
| Master Smith title | 30 | title unlock |
| Master Alchemist title | 30 | title unlock |
| Master Fletcher title | 30 | title unlock |
| Master Chef title | 30 | title unlock |
| Grandmaster Artisan title | 100 | title unlock |
| Gilded tool skin | 20 | cosmetic |
| Artisan's Cape | 50 | cosmetic equipment |
| Bonus order slot | 40 | unlock |

**Step 3: Handle marks-based purchases**

When a player buys from the Master Artisan shop:
- Check `commission_marks >= cost`
- Deduct marks via `db.spend_commission_marks()`
- For title items: call `db.unlock_title()` and send system message
- For equipment/cosmetic items: add to inventory
- For the bonus order slot: set a flag on the player (persisted in DB)

This may require a new shop type or a special handler. Check if the existing shop system can be parameterized to use a different currency, or add a special-case handler for this NPC.

**Step 4: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 5: Commit**

```bash
git add rust-server/data/npcs/ rust-server/data/shops/ rust-server/src/game/shop.rs rust-server/src/game/crafting_orders.rs
git commit -m "feat: add Master Artisan NPC with Commission Marks prestige shop"
```

---

### Task 9: Load Title at Login and End-to-End Testing

**Files:**
- Modify: `rust-server/src/db.rs` or character loading code (load active_title on login)
- Modify: wherever character data is loaded into Player struct at session start

**Step 1: Load active_title when player logs in**

Find the character loading flow (search for where `Player::new()` is called with DB data). After loading, query `characters.active_title` and convert to display text:

```rust
if let Some(title_id) = db_active_title {
    player.active_title = crate::game::titles::title_display(&title_id).map(|s| s.to_string());
}
```

Also load `commission_marks` from the characters table if it's used in the Player struct.

**Step 2: Manual end-to-end test plan**

1. Start server, create character, log in
2. Open Adventure Board → verify "Orders" tab appears
3. Check that orders are generated based on skill levels
4. Accept a regular order → verify it shows as active
5. Gather/craft required items → return to board → claim → verify gold + XP
6. Accept a masterwork order → complete → verify marks awarded
7. Visit Master Artisan NPC → buy a title with marks
8. Use `/title set <id>` → verify title appears overhead and in chat
9. `/title list` → verify unlocked titles shown
10. `/title clear` → verify title removed
11. Log out and back in → verify title persists

**Step 3: Commit any remaining fixes**

```bash
git add -u
git commit -m "feat: load player titles at login, finalize crafting orders integration"
```

---

### Task Summary

| # | Task | Scope |
|---|------|-------|
| 1 | Database tables | Server DB |
| 2 | Title system — server core | Server logic + wire format |
| 3 | Title system — client rendering | Client UI |
| 4 | Order data templates | TOML data files |
| 5 | Order system — server core | Server logic |
| 6 | Adventure Board — orders protocol | Server protocol + handlers |
| 7 | Adventure Board — client UI | Client UI |
| 8 | Master Artisan NPC shop | Server NPC + shop |
| 9 | Login loading + end-to-end test | Integration |

Tasks 1-3 (titles) can be built and tested independently before tasks 4-8 (orders). Task 9 ties everything together.
