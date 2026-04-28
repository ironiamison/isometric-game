# Crafting Crates Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add lootbox crate items rewarded from crafting orders that can be opened from inventory for a single random loot roll with level-scaled tables.

**Architecture:** New `UseEffect::OpenCrate` variant routes crate items to a dedicated handler. Loot tables are TOML-defined with rarity tiers and level brackets. Crates are granted on crafting order claim. Opening removes the crate and adds a random reward.

**Tech Stack:** Rust (server: Axum/Tokio/SQLite), TOML data files, MessagePack protocol.

---

### Task 1: Crate Item Definitions

**Files:**
- Create: `rust-server/data/items/crates.toml`

**Step 1: Create crate item definitions**

Create `rust-server/data/items/crates.toml` with 6 crate items. All are stackable, bankable, not sellable. Each has a `use_effect` of type `open_crate` with `tier` and `bracket` fields.

Use this format (matching existing items in `data/items/consumables.toml`):

```toml
[artisans_crate_low]
display_name = "Artisan's Crate (Beginner)"
sprite = "crate"
description = "A sturdy crate packed by the Artisan's Guild. Contains beginner-tier rewards."
category = "misc"
max_stack = 100
base_price = 0
sellable = false

[artisans_crate_low.use_effect]
type = "open_crate"
tier = "artisan"
bracket = "low"

[artisans_crate_mid]
display_name = "Artisan's Crate (Intermediate)"
sprite = "crate"
description = "A sturdy crate packed by the Artisan's Guild. Contains intermediate-tier rewards."
category = "misc"
max_stack = 100
base_price = 0
sellable = false

[artisans_crate_mid.use_effect]
type = "open_crate"
tier = "artisan"
bracket = "mid"

[artisans_crate_high]
display_name = "Artisan's Crate (Advanced)"
sprite = "crate"
description = "A sturdy crate packed by the Artisan's Guild. Contains advanced-tier rewards."
category = "misc"
max_stack = 100
base_price = 0
sellable = false

[artisans_crate_high.use_effect]
type = "open_crate"
tier = "artisan"
bracket = "high"

[masters_crate_low]
display_name = "Master's Crate (Beginner)"
sprite = "crate_gold"
description = "A gilded crate reserved for master craftsmen. Contains beginner-tier exceptional rewards."
category = "misc"
max_stack = 100
base_price = 0
sellable = false

[masters_crate_low.use_effect]
type = "open_crate"
tier = "master"
bracket = "low"

[masters_crate_mid]
display_name = "Master's Crate (Intermediate)"
sprite = "crate_gold"
description = "A gilded crate reserved for master craftsmen. Contains intermediate-tier exceptional rewards."
category = "misc"
max_stack = 100
base_price = 0
sellable = false

[masters_crate_mid.use_effect]
type = "open_crate"
tier = "master"
bracket = "mid"

[masters_crate_high]
display_name = "Master's Crate (Advanced)"
sprite = "crate_gold"
description = "A gilded crate reserved for master craftsmen. Contains advanced-tier exceptional rewards."
category = "misc"
max_stack = 100
base_price = 0
sellable = false

[masters_crate_high.use_effect]
type = "open_crate"
tier = "master"
bracket = "high"
```

**Step 2: Commit**

```bash
git add rust-server/data/items/crates.toml
git commit -m "feat: add crate item definitions for crafting order rewards"
```

---

### Task 2: UseEffect::OpenCrate Variant and Routing

**Files:**
- Modify: `rust-server/src/data/item_def.rs` (~line 218, add OpenCrate variant to UseEffect enum)
- Modify: `rust-server/src/game/inventory.rs` (~line 19, add Crate to ItemUseRoute; ~line 33, add routing in classify_item_use)

**Step 1: Add OpenCrate variant to UseEffect**

In `rust-server/src/data/item_def.rs`, add to the `UseEffect` enum (after `Dig` ~line 218):

```rust
OpenCrate {
    tier: String,
    bracket: String,
},
```

**Step 2: Add Crate route to ItemUseRoute**

In `rust-server/src/game/inventory.rs`, add `Crate` variant to `ItemUseRoute` enum (~line 23):

```rust
enum ItemUseRoute {
    RecipeScroll,
    SpellScroll,
    DigTool,
    Crate,
    Consumable,
}
```

**Step 3: Add routing in classify_item_use**

In `classify_item_use()` (~line 33), add a match arm for `OpenCrate` before the default `Consumable` fallthrough:

```rust
fn classify_item_use(item_id: &str, use_effect: Option<&UseEffect>) -> ItemUseRoute {
    if item_id.starts_with("recipe_") {
        return ItemUseRoute::RecipeScroll;
    }
    match use_effect {
        Some(UseEffect::LearnSpell { .. }) => ItemUseRoute::SpellScroll,
        Some(UseEffect::Dig) => ItemUseRoute::DigTool,
        Some(UseEffect::OpenCrate { .. }) => ItemUseRoute::Crate,
        _ => ItemUseRoute::Consumable,
    }
}
```

**Step 4: Add the dispatch case in handle_use_item**

In `handle_use_item()` in inventory.rs, find the match on `classify_item_use()` result. Add:

```rust
ItemUseRoute::Crate => {
    self.handle_open_crate(player_id, slot_index).await;
}
```

This calls a method that will be implemented in Task 3.

**Step 5: Verify compilation**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 6: Commit**

```bash
git add rust-server/src/data/item_def.rs rust-server/src/game/inventory.rs
git commit -m "feat: add OpenCrate use effect variant and item routing"
```

---

### Task 3: Loot Table Data and Loading

**Files:**
- Create: `rust-server/data/crate_loot/artisan.toml`
- Create: `rust-server/data/crate_loot/master.toml`
- Create: `rust-server/src/game/crate_loot.rs`
- Modify: `rust-server/src/game.rs` (add mod declaration and field to GameRoom)

**Step 1: Create artisan loot table TOML**

Create `rust-server/data/crate_loot/artisan.toml`:

```toml
# Artisan's Crate loot tables
# Single roll: pick rarity, then pick item from bracket pool

[rarity_weights]
common = 60
uncommon = 30
rare = 10

# ---- LOW BRACKET (order min_level 1-19) ----

[[low.common]]
item_id = "copper_ore"
quantity_min = 15
quantity_max = 25

[[low.common]]
item_id = "tin_ore"
quantity_min = 15
quantity_max = 25

[[low.common]]
item_id = "oak_log"
quantity_min = 10
quantity_max = 20

[[low.common]]
item_id = "raw_shrimp"
quantity_min = 15
quantity_max = 25

[[low.common]]
item_id = "raw_sardine"
quantity_min = 10
quantity_max = 20

[[low.uncommon]]
item_id = "weak_health_potion"
quantity_min = 3
quantity_max = 5

[[low.uncommon]]
item_id = "bronze_bar"
quantity_min = 5
quantity_max = 10

[[low.uncommon]]
item_id = "potato_seed"
quantity_min = 3
quantity_max = 6

[[low.uncommon]]
item_id = "onion_seed"
quantity_min = 3
quantity_max = 6

[[low.rare]]
item_id = "iron_bar"
quantity_min = 3
quantity_max = 5

[[low.rare]]
item_id = "commission_marks"
quantity_min = 3
quantity_max = 3

# ---- MID BRACKET (order min_level 20-39) ----

[[mid.common]]
item_id = "iron_ore"
quantity_min = 10
quantity_max = 20

[[mid.common]]
item_id = "coal_ore"
quantity_min = 8
quantity_max = 15

[[mid.common]]
item_id = "willow_log"
quantity_min = 10
quantity_max = 15

[[mid.common]]
item_id = "raw_salmon"
quantity_min = 10
quantity_max = 20

[[mid.common]]
item_id = "raw_lobster"
quantity_min = 8
quantity_max = 15

[[mid.uncommon]]
item_id = "health_potion"
quantity_min = 2
quantity_max = 4

[[mid.uncommon]]
item_id = "steel_bar"
quantity_min = 3
quantity_max = 5

[[mid.uncommon]]
item_id = "strawberry_seed"
quantity_min = 2
quantity_max = 4

[[mid.uncommon]]
item_id = "sweetcorn_seed"
quantity_min = 2
quantity_max = 3

[[mid.rare]]
item_id = "mithril_bar"
quantity_min = 2
quantity_max = 4

[[mid.rare]]
item_id = "commission_marks"
quantity_min = 5
quantity_max = 5

# ---- HIGH BRACKET (order min_level 40+) ----

[[high.common]]
item_id = "mithril_ore"
quantity_min = 8
quantity_max = 15

[[high.common]]
item_id = "adamant_ore"
quantity_min = 5
quantity_max = 10

[[high.common]]
item_id = "yew_log"
quantity_min = 8
quantity_max = 12

[[high.common]]
item_id = "raw_swordfish"
quantity_min = 8
quantity_max = 15

[[high.uncommon]]
item_id = "super_attack_potion"
quantity_min = 2
quantity_max = 3

[[high.uncommon]]
item_id = "super_strength_potion"
quantity_min = 2
quantity_max = 3

[[high.uncommon]]
item_id = "adamant_bar"
quantity_min = 3
quantity_max = 5

[[high.uncommon]]
item_id = "watermelon_seed"
quantity_min = 1
quantity_max = 3

[[high.rare]]
item_id = "rune_bar"
quantity_min = 1
quantity_max = 2

[[high.rare]]
item_id = "commission_marks"
quantity_min = 8
quantity_max = 8
```

**Step 2: Create master loot table TOML**

Create `rust-server/data/crate_loot/master.toml` with better drop rates and an epic tier:

```toml
# Master's Crate loot tables
# Better rarity weights + epic tier

[rarity_weights]
common = 40
uncommon = 40
rare = 15
epic = 5

# ---- LOW BRACKET ----

[[low.common]]
item_id = "iron_ore"
quantity_min = 15
quantity_max = 25

[[low.common]]
item_id = "oak_log"
quantity_min = 15
quantity_max = 25

[[low.common]]
item_id = "raw_trout"
quantity_min = 15
quantity_max = 25

[[low.uncommon]]
item_id = "health_potion"
quantity_min = 3
quantity_max = 5

[[low.uncommon]]
item_id = "iron_bar"
quantity_min = 5
quantity_max = 8

[[low.uncommon]]
item_id = "strawberry_seed"
quantity_min = 3
quantity_max = 5

[[low.rare]]
item_id = "steel_bar"
quantity_min = 3
quantity_max = 6

[[low.rare]]
item_id = "commission_marks"
quantity_min = 5
quantity_max = 5

[[low.epic]]
item_id = "commission_marks"
quantity_min = 15
quantity_max = 15

# ---- MID BRACKET ----

[[mid.common]]
item_id = "mithril_ore"
quantity_min = 10
quantity_max = 18

[[mid.common]]
item_id = "maple_log"
quantity_min = 10
quantity_max = 18

[[mid.common]]
item_id = "raw_lobster"
quantity_min = 10
quantity_max = 20

[[mid.uncommon]]
item_id = "super_strength_potion"
quantity_min = 2
quantity_max = 4

[[mid.uncommon]]
item_id = "mithril_bar"
quantity_min = 3
quantity_max = 6

[[mid.uncommon]]
item_id = "watermelon_seed"
quantity_min = 2
quantity_max = 3

[[mid.rare]]
item_id = "adamant_bar"
quantity_min = 3
quantity_max = 5

[[mid.rare]]
item_id = "commission_marks"
quantity_min = 10
quantity_max = 10

[[mid.epic]]
item_id = "commission_marks"
quantity_min = 25
quantity_max = 25

# ---- HIGH BRACKET ----

[[high.common]]
item_id = "adamant_ore"
quantity_min = 8
quantity_max = 15

[[high.common]]
item_id = "yew_log"
quantity_min = 10
quantity_max = 18

[[high.common]]
item_id = "raw_swordfish"
quantity_min = 10
quantity_max = 18

[[high.uncommon]]
item_id = "super_defence_potion"
quantity_min = 2
quantity_max = 4

[[high.uncommon]]
item_id = "adamant_bar"
quantity_min = 4
quantity_max = 8

[[high.uncommon]]
item_id = "nightthorn_seed"
quantity_min = 1
quantity_max = 2

[[high.rare]]
item_id = "rune_bar"
quantity_min = 2
quantity_max = 4

[[high.rare]]
item_id = "commission_marks"
quantity_min = 15
quantity_max = 15

[[high.epic]]
item_id = "commission_marks"
quantity_min = 25
quantity_max = 25
```

**Step 3: Create crate_loot.rs with data loading and roll logic**

Create `rust-server/src/game/crate_loot.rs`:

```rust
use rand::Rng;
use serde::Deserialize;
use std::collections::HashMap;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct LootEntry {
    pub item_id: String,
    pub quantity_min: i32,
    pub quantity_max: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RarityWeights {
    pub common: u32,
    pub uncommon: u32,
    pub rare: u32,
    #[serde(default)]
    pub epic: u32,
}

/// Raw TOML structure for a bracket (low/mid/high)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct BracketLoot {
    #[serde(default)]
    pub common: Vec<LootEntry>,
    #[serde(default)]
    pub uncommon: Vec<LootEntry>,
    #[serde(default)]
    pub rare: Vec<LootEntry>,
    #[serde(default)]
    pub epic: Vec<LootEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrateLootFile {
    pub rarity_weights: RarityWeights,
    #[serde(default)]
    pub low: BracketLoot,
    #[serde(default)]
    pub mid: BracketLoot,
    #[serde(default)]
    pub high: BracketLoot,
}

// ============================================================================
// Registry
// ============================================================================

pub struct CrateLootRegistry {
    tables: HashMap<String, CrateLootFile>, // tier -> loot file
}

impl CrateLootRegistry {
    pub fn load(data_path: &str) -> Self {
        let mut tables = HashMap::new();
        let dir = format!("{}/crate_loot", data_path);
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Could not read crate_loot directory {}: {}", dir, e);
                return Self { tables };
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let tier = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            match std::fs::read_to_string(&path) {
                Ok(contents) => match toml::from_str::<CrateLootFile>(&contents) {
                    Ok(loot_file) => {
                        tracing::info!("Loaded crate loot table: {}", tier);
                        tables.insert(tier, loot_file);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse crate loot {:?}: {}", path, e);
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read crate loot {:?}: {}", path, e);
                }
            }
        }

        Self { tables }
    }

    /// Roll a single loot drop from a crate.
    /// Returns (item_id, quantity) or None if the table is missing/empty.
    pub fn roll(&self, tier: &str, bracket: &str) -> Option<(String, i32)> {
        let table = self.tables.get(tier)?;
        let bracket_loot = match bracket {
            "low" => &table.low,
            "mid" => &table.mid,
            "high" => &table.high,
            _ => &table.mid,
        };

        let weights = &table.rarity_weights;
        let total = weights.common + weights.uncommon + weights.rare + weights.epic;
        if total == 0 {
            return None;
        }

        let mut rng = rand::thread_rng();
        let roll = rng.gen_range(0..total);

        let pool = if roll < weights.common {
            &bracket_loot.common
        } else if roll < weights.common + weights.uncommon {
            &bracket_loot.uncommon
        } else if roll < weights.common + weights.uncommon + weights.rare {
            &bracket_loot.rare
        } else {
            // Epic — fall back to rare if no epic entries
            if bracket_loot.epic.is_empty() {
                &bracket_loot.rare
            } else {
                &bracket_loot.epic
            }
        };

        if pool.is_empty() {
            return None;
        }

        let entry = &pool[rng.gen_range(0..pool.len())];
        let quantity = rng.gen_range(entry.quantity_min..=entry.quantity_max);
        Some((entry.item_id.clone(), quantity))
    }
}
```

**Step 4: Register module and field on GameRoom**

In `rust-server/src/game.rs`:
- Add `pub(crate) mod crate_loot;`
- Add `pub crate_loot_registry: crate_loot::CrateLootRegistry` to GameRoom struct
- In `GameRoom::new()`, load it: `crate_loot_registry: crate_loot::CrateLootRegistry::load("data")`

**Step 5: Verify compilation**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 6: Commit**

```bash
git add rust-server/data/crate_loot/ rust-server/src/game/crate_loot.rs rust-server/src/game.rs
git commit -m "feat: add crate loot tables and registry with roll logic"
```

---

### Task 4: Crate Opening Handler

**Files:**
- Modify: `rust-server/src/game/crate_loot.rs` (add handle_open_crate on GameRoom)
- Modify: `rust-server/src/game/inventory.rs` (if the Crate dispatch was stubbed, wire it up)

**Step 1: Implement handle_open_crate on GameRoom**

Add to `rust-server/src/game/crate_loot.rs`:

```rust
use super::GameRoom;
use crate::protocol::ServerMessage;
use crate::data::item_def::UseEffect;

impl GameRoom {
    pub(in crate::game) async fn handle_open_crate(
        &self,
        player_id: &str,
        slot_index: usize,
    ) {
        // 1. Get the item from the slot, extract tier+bracket from UseEffect
        let (item_id, tier, bracket) = {
            let players = self.players.read().await;
            let Some(player) = players.get(player_id) else { return; };
            let Some(slot) = player.inventory.slots.get(slot_index) else { return; };
            let Some(ref slot_item) = slot else { return; };
            let item_id = slot_item.item_id.clone();
            let Some(def) = self.item_registry.get(&item_id) else { return; };
            match &def.use_effect {
                Some(UseEffect::OpenCrate { tier, bracket }) => {
                    (item_id, tier.clone(), bracket.clone())
                }
                _ => return,
            }
        };

        // 2. Roll loot
        let Some((reward_id, reward_qty)) = self.crate_loot_registry.roll(&tier, &bracket) else {
            self.send_system_message(player_id, "The crate was empty...").await;
            return;
        };

        // 3. Special case: commission_marks reward
        if reward_id == "commission_marks" {
            // Remove crate, grant marks via DB
            let (inventory_update, gold) = {
                let mut players = self.players.write().await;
                let Some(player) = players.get_mut(player_id) else { return; };
                player.inventory.remove_item(&item_id, 1);
                (player.inventory.to_update(), player.inventory.gold)
            };
            if let Some(ref db) = self.db {
                if let Some(character_id) = Self::parse_character_id(player_id) {
                    let _ = db.add_commission_marks(character_id, reward_qty).await;
                }
            }
            self.send_to_player(player_id, ServerMessage::InventoryUpdate {
                slots: inventory_update,
                gold,
            }).await;
            self.send_system_message(
                player_id,
                &format!("You open a crate and find: {} Commission Marks!", reward_qty),
            ).await;
            return;
        }

        // 4. Remove crate, add reward item
        let (inventory_update, gold, reward_name) = {
            let mut players = self.players.write().await;
            let Some(player) = players.get_mut(player_id) else { return; };
            player.inventory.remove_item(&item_id, 1);
            player.inventory.add_item(&reward_id, reward_qty, &self.item_registry);
            let reward_name = self.item_registry
                .get(&reward_id)
                .map(|d| d.display_name.clone())
                .unwrap_or_else(|| reward_id.clone());
            (player.inventory.to_update(), player.inventory.gold, reward_name)
        };

        // 5. Send updates
        self.send_to_player(player_id, ServerMessage::InventoryUpdate {
            slots: inventory_update,
            gold,
        }).await;

        self.send_system_message(
            player_id,
            &format!("You open a crate and find: {}x {}!", reward_qty, reward_name),
        ).await;
    }
}
```

**Step 2: Verify compilation**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 3: Commit**

```bash
git add rust-server/src/game/crate_loot.rs rust-server/src/game/inventory.rs
git commit -m "feat: implement crate opening handler with loot roll"
```

---

### Task 5: Grant Crates on Crafting Order Claim

**Files:**
- Modify: `rust-server/src/game/crafting_orders.rs` (in handle_claim_crafting_order, add crate to inventory)

**Step 1: Add crate reward to claim handler**

In `handle_claim_crafting_order()`, inside the write lock block where items are removed and gold is granted, after granting gold, add:

```rust
// Determine crate type and bracket
let bracket = if template.min_level >= 40 {
    "high"
} else if template.min_level >= 20 {
    "mid"
} else {
    "low"
};
let crate_id = if template.tier == "masterwork" {
    format!("masters_crate_{}", bracket)
} else {
    format!("artisans_crate_{}", bracket)
};
player.inventory.add_item(&crate_id, 1, &self.item_registry);
```

Also update the system message to mention the crate reward:

```rust
let crate_name = self.item_registry
    .get(&crate_id)
    .map(|d| d.display_name.clone())
    .unwrap_or(crate_id);
// Include in the existing rewards message
```

Note: `self.item_registry` is accessible because `self` is available in the outer function scope. The crate_id and crate_name need to be captured inside the write lock block and passed out with the other result data.

**Step 2: Verify compilation**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 3: Commit**

```bash
git add rust-server/src/game/crafting_orders.rs
git commit -m "feat: grant crate rewards on crafting order completion"
```

---

### Task Summary

| # | Task | Scope |
|---|------|-------|
| 1 | Crate item definitions | TOML data (6 items) |
| 2 | UseEffect::OpenCrate + routing | Server item_def + inventory dispatch |
| 3 | Loot tables + registry | TOML data + server loading/roll logic |
| 4 | Crate opening handler | Server game logic |
| 5 | Grant crates on order claim | Server crafting_orders integration |

Tasks are sequential — each builds on the previous.
