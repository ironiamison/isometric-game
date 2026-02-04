# Monster Material Crafting Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement station-based crafting with a new Smithing skill, recipe discovery, and an interactive crafting UI with progress bars.

**Architecture:** Extend the existing crafting system with: (1) new Smithing skill in skills registry, (2) station types and player proximity checks, (3) recipe discovery/unlock tracking per player, (4) timed crafting with progress and interruption handling, (5) enhanced UI with progress bars and sprite previews.

**Tech Stack:** Rust server (TOML configs, async handlers), Rust client (macroquad UI), existing protocol patterns.

---

## Phase 1: Data Foundation

### Task 1: Add Smithing Skill Type

**Files:**
- Modify: `rust-server/src/skills.rs`
- Modify: `client/src/game/skills.rs`

**Step 1: Add Smithing to server SkillType enum**

In `rust-server/src/skills.rs`, find the `SkillType` enum and add Smithing:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillType {
    Hitpoints,
    Combat,
    Fishing,
    Farming,
    Smithing,  // ADD THIS
}
```

**Step 2: Add smithing field to Skills struct**

Find the `Skills` struct and add:

```rust
pub struct Skills {
    pub hitpoints: Skill,
    pub combat: Skill,
    pub fishing: Skill,
    pub farming: Skill,
    pub smithing: Skill,  // ADD THIS
}
```

**Step 3: Update Skills::new() default**

```rust
impl Skills {
    pub fn new() -> Self {
        Self {
            hitpoints: Skill::new(),
            combat: Skill::new(),
            fishing: Skill::new(),
            farming: Skill::new(),
            smithing: Skill::new(),  // ADD THIS
        }
    }
}
```

**Step 4: Update Skills::get() match**

```rust
pub fn get(&self, skill_type: SkillType) -> &Skill {
    match skill_type {
        SkillType::Hitpoints => &self.hitpoints,
        SkillType::Combat => &self.combat,
        SkillType::Fishing => &self.fishing,
        SkillType::Farming => &self.farming,
        SkillType::Smithing => &self.smithing,  // ADD THIS
    }
}
```

**Step 5: Update Skills::get_mut() match**

```rust
pub fn get_mut(&mut self, skill_type: SkillType) -> &mut Skill {
    match skill_type {
        SkillType::Hitpoints => &mut self.hitpoints,
        SkillType::Combat => &mut self.combat,
        SkillType::Fishing => &mut self.fishing,
        SkillType::Farming => &mut self.farming,
        SkillType::Smithing => &mut self.smithing,  // ADD THIS
    }
}
```

**Step 6: Mirror changes in client skills.rs**

Apply the same changes to `client/src/game/skills.rs` - add Smithing to SkillType enum, Skills struct, and all match statements.

**Step 7: Verify compilation**

Run: `cd rust-server && cargo check`
Run: `cd client && cargo check`
Expected: Both compile without errors

**Step 8: Commit**

```bash
git add rust-server/src/skills.rs client/src/game/skills.rs
git commit -m "feat: add Smithing skill type"
```

---

### Task 2: Add Component Materials

**Files:**
- Modify: `rust-server/data/items/materials.toml`

**Step 1: Add crafting component items**

Append to `rust-server/data/items/materials.toml`:

```toml
# =============================================================================
# Crafting Components
# =============================================================================

[scrap_leather]
display_name = "Scrap Leather"
sprite = "scrap_leather"
description = "Crudely tanned leather from forest creatures."
category = "material"
max_stack = 99
base_price = 15
sellable = true

[bone_fragment]
display_name = "Bone Fragment"
sprite = "bone_fragment"
description = "Bits of shell and bone bound together."
category = "material"
max_stack = 99
base_price = 18
sellable = true

[crude_binding]
display_name = "Crude Binding"
sprite = "crude_binding"
description = "Fibrous material for holding things together."
category = "material"
max_stack = 99
base_price = 20
sellable = true

[shell_plate]
display_name = "Shell Plate"
sprite = "shell_plate"
description = "Hardened shell pieces, surprisingly sturdy."
category = "material"
max_stack = 99
base_price = 22
sellable = true

[blade_shard]
display_name = "Blade Shard"
sprite = "blade_shard"
description = "Sharp spines formed into a crude blade edge."
category = "material"
max_stack = 99
base_price = 25
sellable = true

[bow_stave]
display_name = "Bow Stave"
sprite = "bow_stave"
description = "A flexible frame ready to be strung."
category = "material"
max_stack = 99
base_price = 25
sellable = true

# =============================================================================
# Ammunition
# =============================================================================

[bone_arrow]
display_name = "Bone Arrow"
sprite = "bone_arrow"
description = "Arrows crafted from bone and feather."
category = "material"
max_stack = 999
base_price = 2
sellable = true

# =============================================================================
# Recipe Scrolls
# =============================================================================

[recipe_spine_blade]
display_name = "Recipe: Spine Blade"
sprite = "recipe_scroll"
description = "Teaches how to craft a Spine Blade."
category = "material"
max_stack = 1
base_price = 50
sellable = true

[recipe_scavenger_bow]
display_name = "Recipe: Scavenger Bow"
sprite = "recipe_scroll"
description = "Teaches how to craft a Scavenger Bow."
category = "material"
max_stack = 1
base_price = 50
sellable = true
```

**Step 2: Verify server loads items**

Run: `cd rust-server && cargo check`
Expected: Compiles (item loading happens at runtime)

**Step 3: Commit**

```bash
git add rust-server/data/items/materials.toml
git commit -m "feat: add crafting components and recipe scrolls"
```

---

### Task 3: Add Scavenger Equipment

**Files:**
- Modify: `rust-server/data/items/equipment.toml`

**Step 1: Add Scavenger Set equipment**

Append to `rust-server/data/items/equipment.toml`:

```toml
# =============================================================================
# SCAVENGER SET (Craftable early-game gear)
# =============================================================================

[scavenger_vest]
display_name = "Scavenger Vest"
sprite = "scavenger_vest"
description = "Cobbled-together armor from forest creature scraps."
category = "equipment"
max_stack = 1
base_price = 45
sellable = true
[scavenger_vest.equipment]
slot_type = "body"
defence_level_required = 1
attack_bonus = 0
strength_bonus = 0
defence_bonus = 3

[scavenger_boots]
display_name = "Scavenger Boots"
sprite = "scavenger_boots"
description = "Sturdy boots patched together from leather scraps."
category = "equipment"
max_stack = 1
base_price = 25
sellable = true
[scavenger_boots.equipment]
slot_type = "feet"
defence_level_required = 1
attack_bonus = 0
strength_bonus = 0
defence_bonus = 2

[bone_ring]
display_name = "Bone Ring"
sprite = "bone_ring"
description = "A ring carved from creature bones."
category = "equipment"
max_stack = 1
base_price = 35
sellable = true
[bone_ring.equipment]
slot_type = "ring"
defence_level_required = 1
attack_bonus = 1
strength_bonus = 1
defence_bonus = 1

[shell_pendant]
display_name = "Shell Pendant"
sprite = "shell_pendant"
description = "A pendant made from polished shell plates."
category = "equipment"
max_stack = 1
base_price = 40
sellable = true
[shell_pendant.equipment]
slot_type = "necklace"
defence_level_required = 1
attack_bonus = 0
strength_bonus = 0
defence_bonus = 3

[spine_blade]
display_name = "Spine Blade"
sprite = "spine_blade"
description = "A crude blade fashioned from hedgehog spines."
category = "equipment"
max_stack = 1
base_price = 55
sellable = true
[spine_blade.equipment]
slot_type = "weapon"
attack_level_required = 1
attack_bonus = 4
strength_bonus = 6
defence_bonus = 0

[scavenger_bow]
display_name = "Scavenger Bow"
sprite = "scavenger_bow"
description = "A makeshift bow cobbled together from springy materials."
category = "equipment"
max_stack = 1
base_price = 65
sellable = true
[scavenger_bow.equipment]
slot_type = "weapon"
weapon_type = "ranged"
range = 6
attack_level_required = 1
attack_bonus = 8
strength_bonus = 5
defence_bonus = 0
```

**Step 2: Verify compilation**

Run: `cd rust-server && cargo check`
Expected: Compiles

**Step 3: Commit**

```bash
git add rust-server/data/items/equipment.toml
git commit -m "feat: add Scavenger Set equipment"
```

---

### Task 4: Add Smithing Recipes

**Files:**
- Create: `rust-server/data/recipes/smithing.toml`

**Step 1: Create smithing recipes file**

Create `rust-server/data/recipes/smithing.toml`:

```toml
# Smithing recipes - components and gear crafted at stations
# smithing_level = skill requirement (0 = no requirement, for arrows)
# station = required station type ("anvil" or "workbench")
# craft_time_ms = crafting duration in milliseconds
# xp = smithing XP gained on completion

# =============================================================================
# Components (crafted at Anvil, Smithing level 1)
# =============================================================================

[scrap_leather]
display_name = "Scrap Leather"
description = "Crudely tanned leather from forest creatures."
category = "smithing"
level_required = 1
station = "anvil"
craft_time_ms = 1500
xp = 10

[[scrap_leather.ingredients]]
item_id = "piglet"
count = 3

[[scrap_leather.ingredients]]
item_id = "slime_core"
count = 2

[[scrap_leather.results]]
item_id = "scrap_leather"
count = 1

[bone_fragment]
display_name = "Bone Fragment"
description = "Bits of shell and bone bound together."
category = "smithing"
level_required = 1
station = "anvil"
craft_time_ms = 1500
xp = 10

[[bone_fragment.ingredients]]
item_id = "snail_shell"
count = 4

[[bone_fragment.ingredients]]
item_id = "hedgehog_spine"
count = 2

[[bone_fragment.results]]
item_id = "bone_fragment"
count = 1

[crude_binding]
display_name = "Crude Binding"
description = "Fibrous material for holding things together."
category = "smithing"
level_required = 1
station = "anvil"
craft_time_ms = 1500
xp = 10

[[crude_binding.ingredients]]
item_id = "worm_segment"
count = 3

[[crude_binding.ingredients]]
item_id = "spider_silk"
count = 2

[[crude_binding.results]]
item_id = "crude_binding"
count = 1

[shell_plate]
display_name = "Shell Plate"
description = "Hardened shell pieces, surprisingly sturdy."
category = "smithing"
level_required = 1
station = "anvil"
craft_time_ms = 1500
xp = 10

[[shell_plate.ingredients]]
item_id = "snail_shell"
count = 5

[[shell_plate.results]]
item_id = "shell_plate"
count = 1

[blade_shard]
display_name = "Blade Shard"
description = "Sharp spines formed into a crude blade edge."
category = "smithing"
level_required = 1
station = "anvil"
craft_time_ms = 1500
xp = 10

[[blade_shard.ingredients]]
item_id = "hedgehog_spine"
count = 4

[[blade_shard.ingredients]]
item_id = "spring_coil"
count = 1

[[blade_shard.results]]
item_id = "blade_shard"
count = 1

[bow_stave]
display_name = "Bow Stave"
description = "A flexible frame ready to be strung."
category = "smithing"
level_required = 1
station = "workbench"
craft_time_ms = 1500
xp = 10

[[bow_stave.ingredients]]
item_id = "spring_coil"
count = 3

[[bow_stave.ingredients]]
item_id = "worm_segment"
count = 2

[[bow_stave.results]]
item_id = "bow_stave"
count = 1

# =============================================================================
# Ammunition (crafted at Workbench, no Smithing requirement)
# =============================================================================

[bone_arrow]
display_name = "Bone Arrow"
description = "Craft arrows from bone and feather."
category = "smithing"
level_required = 0
station = "workbench"
craft_time_ms = 1500
xp = 5

[[bone_arrow.ingredients]]
item_id = "hedgehog_spine"
count = 1

[[bone_arrow.ingredients]]
item_id = "crow_feather"
count = 3

[[bone_arrow.results]]
item_id = "bone_arrow"
count = 15

# =============================================================================
# Armor (crafted at Anvil)
# =============================================================================

[scavenger_vest]
display_name = "Scavenger Vest"
description = "Cobble together armor from forest creature scraps."
category = "smithing"
level_required = 1
station = "anvil"
craft_time_ms = 5000
xp = 35

[[scavenger_vest.ingredients]]
item_id = "scrap_leather"
count = 2

[[scavenger_vest.ingredients]]
item_id = "shell_plate"
count = 1

[[scavenger_vest.ingredients]]
item_id = "crude_binding"
count = 1

[[scavenger_vest.results]]
item_id = "scavenger_vest"
count = 1

[scavenger_boots]
display_name = "Scavenger Boots"
description = "Patch together sturdy boots from leather scraps."
category = "smithing"
level_required = 1
station = "anvil"
craft_time_ms = 3000
xp = 20

[[scavenger_boots.ingredients]]
item_id = "scrap_leather"
count = 1

[[scavenger_boots.ingredients]]
item_id = "crude_binding"
count = 1

[[scavenger_boots.results]]
item_id = "scavenger_boots"
count = 1

# =============================================================================
# Accessories (crafted at Workbench, Smithing level 5)
# =============================================================================

[bone_ring]
display_name = "Bone Ring"
description = "Carve a ring from creature bones."
category = "smithing"
level_required = 5
station = "workbench"
craft_time_ms = 3000
xp = 25

[[bone_ring.ingredients]]
item_id = "bone_fragment"
count = 1

[[bone_ring.results]]
item_id = "bone_ring"
count = 1

[shell_pendant]
display_name = "Shell Pendant"
description = "Polish shell plates into a pendant."
category = "smithing"
level_required = 5
station = "workbench"
craft_time_ms = 3000
xp = 25

[[shell_pendant.ingredients]]
item_id = "shell_plate"
count = 1

[[shell_pendant.ingredients]]
item_id = "crude_binding"
count = 1

[[shell_pendant.results]]
item_id = "shell_pendant"
count = 1

# =============================================================================
# Weapons (crafted at Anvil, Smithing level 5, requires recipe scroll)
# =============================================================================

[spine_blade]
display_name = "Spine Blade"
description = "Forge a blade from sharpened spines."
category = "smithing"
level_required = 5
station = "anvil"
craft_time_ms = 5000
xp = 40
requires_discovery = true

[[spine_blade.ingredients]]
item_id = "blade_shard"
count = 1

[[spine_blade.ingredients]]
item_id = "bone_fragment"
count = 1

[[spine_blade.results]]
item_id = "spine_blade"
count = 1

[scavenger_bow]
display_name = "Scavenger Bow"
description = "Assemble a makeshift bow from springy materials."
category = "smithing"
level_required = 5
station = "workbench"
craft_time_ms = 5000
xp = 45
requires_discovery = true

[[scavenger_bow.ingredients]]
item_id = "bow_stave"
count = 1

[[scavenger_bow.ingredients]]
item_id = "crude_binding"
count = 1

[[scavenger_bow.ingredients]]
item_id = "crow_feather"
count = 3

[[scavenger_bow.results]]
item_id = "scavenger_bow"
count = 1
```

**Step 2: Verify file is valid TOML**

Run: `cd rust-server && cargo check`
Expected: Compiles (TOML loaded at runtime)

**Step 3: Commit**

```bash
git add rust-server/data/recipes/smithing.toml
git commit -m "feat: add smithing recipes for Scavenger Set"
```

---

### Task 5: Add Recipe Scroll Drops to Monsters

**Files:**
- Modify: `rust-server/data/entities/monsters/dangerous_creatures.toml`
- Modify: `rust-server/data/entities/monsters/forest_creatures.toml`

**Step 1: Add spine blade recipe drop to spider**

In `rust-server/data/entities/monsters/dangerous_creatures.toml`, find the `[spider]` section and add a new loot entry after the existing ones:

```toml
[[spider.loot]]
item_id = "recipe_spine_blade"
drop_chance = 0.05
quantity_min = 1
quantity_max = 1
```

**Step 2: Add scavenger bow recipe drop to crow**

In the same file, find `[crow]` and add:

```toml
[[crow.loot]]
item_id = "recipe_scavenger_bow"
drop_chance = 0.03
quantity_min = 1
quantity_max = 1
```

**Step 3: Add scavenger bow recipe drop to springy**

In `rust-server/data/entities/monsters/forest_creatures.toml`, find `[springs]` and add:

```toml
[[springs.loot]]
item_id = "recipe_scavenger_bow"
drop_chance = 0.03
quantity_min = 1
quantity_max = 1
```

**Step 4: Verify compilation**

Run: `cd rust-server && cargo check`
Expected: Compiles

**Step 5: Commit**

```bash
git add rust-server/data/entities/monsters/dangerous_creatures.toml rust-server/data/entities/monsters/forest_creatures.toml
git commit -m "feat: add recipe scroll drops to spider, crow, springy"
```

---

## Phase 2: Server-Side Crafting Logic

### Task 6: Extend Recipe Definition with New Fields

**Files:**
- Modify: `rust-server/src/crafting/definition.rs`

**Step 1: Add new fields to RawRecipeDefinition**

Find the `RawRecipeDefinition` struct and add the new optional fields:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct RawRecipeDefinition {
    pub display_name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub level_required: Option<u32>,
    pub ingredients: Vec<Ingredient>,
    pub results: Option<Vec<CraftResult>>,
    // NEW FIELDS
    pub station: Option<String>,
    pub craft_time_ms: Option<u64>,
    pub xp: Option<u32>,
    pub requires_discovery: Option<bool>,
}
```

**Step 2: Add new fields to RecipeDefinition**

Find the `RecipeDefinition` struct and add:

```rust
#[derive(Debug, Clone)]
pub struct RecipeDefinition {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub category: RecipeCategory,
    pub level_required: u32,
    pub ingredients: Vec<Ingredient>,
    pub results: Vec<CraftResult>,
    // NEW FIELDS
    pub station: Option<String>,
    pub craft_time_ms: u64,
    pub xp: u32,
    pub requires_discovery: bool,
}
```

**Step 3: Update from_raw() to handle new fields**

Find the `from_raw` function and update it:

```rust
pub fn from_raw(id: String, raw: RawRecipeDefinition) -> Self {
    Self {
        id,
        display_name: raw.display_name,
        description: raw.description.unwrap_or_default(),
        category: raw.category
            .map(|c| RecipeCategory::from_str(&c))
            .unwrap_or(RecipeCategory::Materials),
        level_required: raw.level_required.unwrap_or(1),
        ingredients: raw.ingredients,
        results: raw.results.unwrap_or_default(),
        // NEW FIELDS with defaults
        station: raw.station,
        craft_time_ms: raw.craft_time_ms.unwrap_or(0),
        xp: raw.xp.unwrap_or(0),
        requires_discovery: raw.requires_discovery.unwrap_or(false),
    }
}
```

**Step 4: Verify compilation**

Run: `cd rust-server && cargo check`
Expected: Compiles

**Step 5: Commit**

```bash
git add rust-server/src/crafting/definition.rs
git commit -m "feat: extend recipe definition with station, time, xp, discovery"
```

---

### Task 7: Update Protocol for Extended Recipes

**Files:**
- Modify: `rust-server/src/protocol.rs`
- Modify: `client/src/network/messages.rs`
- Modify: `client/src/game/item.rs`

**Step 1: Add fields to ClientRecipeDef in server protocol**

In `rust-server/src/protocol.rs`, find `ClientRecipeDef` and add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRecipeDef {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub level_required: u32,
    pub ingredients: Vec<RecipeIngredient>,
    pub results: Vec<RecipeResult>,
    // NEW FIELDS
    pub station: Option<String>,
    pub craft_time_ms: u64,
    pub xp: u32,
    pub requires_discovery: bool,
}
```

**Step 2: Update CraftingRegistry::to_client_definitions()**

In `rust-server/src/crafting/registry.rs`, find where `ClientRecipeDef` is constructed and add the new fields:

```rust
ClientRecipeDef {
    id: recipe.id.clone(),
    display_name: recipe.display_name.clone(),
    description: recipe.description.clone(),
    category: recipe.category.to_string(),
    level_required: recipe.level_required,
    ingredients: recipe.ingredients.iter().map(|i| RecipeIngredient {
        item_id: i.item_id.clone(),
        count: i.count,
    }).collect(),
    results: recipe.results.iter().map(|r| RecipeResult {
        item_id: r.item_id.clone(),
        count: r.count,
    }).collect(),
    // NEW FIELDS
    station: recipe.station.clone(),
    craft_time_ms: recipe.craft_time_ms,
    xp: recipe.xp,
    requires_discovery: recipe.requires_discovery,
}
```

**Step 3: Mirror changes in client messages.rs**

In `client/src/network/messages.rs`, find the `ClientRecipeDef` struct (if it exists there) or in `client/src/game/item.rs` find `RecipeDefinition` and add the same fields:

```rust
pub struct RecipeDefinition {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub level_required: u32,
    pub ingredients: Vec<RecipeIngredient>,
    pub results: Vec<RecipeResult>,
    // NEW FIELDS
    pub station: Option<String>,
    pub craft_time_ms: u64,
    pub xp: u32,
    pub requires_discovery: bool,
}
```

**Step 4: Update client message parsing**

In `client/src/network/message_handler.rs`, ensure the new fields are parsed when receiving `RecipeDefinitions`.

**Step 5: Verify compilation**

Run: `cd rust-server && cargo check`
Run: `cd client && cargo check`
Expected: Both compile

**Step 6: Commit**

```bash
git add rust-server/src/protocol.rs rust-server/src/crafting/registry.rs client/src/network/messages.rs client/src/game/item.rs client/src/network/message_handler.rs
git commit -m "feat: extend protocol with station, time, xp, discovery fields"
```

---

### Task 8: Add Crafting Station Definitions

**Files:**
- Create: `rust-server/data/crafting_stations.toml`
- Modify: `rust-server/src/crafting/mod.rs`
- Create: `rust-server/src/crafting/stations.rs`

**Step 1: Create station definitions file**

Create `rust-server/data/crafting_stations.toml`:

```toml
# Crafting station definitions
# Each station has an ID that matches the "station" field in recipes

[anvil]
display_name = "Anvil"
description = "A sturdy anvil for metalworking and armor crafting."
interaction_range = 2

[workbench]
display_name = "Workbench"
description = "A wooden workbench for crafting accessories and ammunition."
interaction_range = 2
```

**Step 2: Create stations module**

Create `rust-server/src/crafting/stations.rs`:

```rust
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct StationDefinition {
    pub display_name: String,
    pub description: String,
    pub interaction_range: u32,
}

#[derive(Debug, Default)]
pub struct StationRegistry {
    stations: HashMap<String, StationDefinition>,
}

impl StationRegistry {
    pub fn new() -> Self {
        Self {
            stations: HashMap::new(),
        }
    }

    pub fn load_from_file(&mut self, path: &Path) -> Result<(), String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read stations file: {}", e))?;

        let stations: HashMap<String, StationDefinition> = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse stations TOML: {}", e))?;

        self.stations = stations;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&StationDefinition> {
        self.stations.get(id)
    }

    pub fn exists(&self, id: &str) -> bool {
        self.stations.contains_key(id)
    }
}
```

**Step 3: Add stations module to crafting/mod.rs**

In `rust-server/src/crafting/mod.rs`, add:

```rust
pub mod stations;
pub use stations::StationRegistry;
```

**Step 4: Verify compilation**

Run: `cd rust-server && cargo check`
Expected: Compiles

**Step 5: Commit**

```bash
git add rust-server/data/crafting_stations.toml rust-server/src/crafting/stations.rs rust-server/src/crafting/mod.rs
git commit -m "feat: add crafting station registry"
```

---

### Task 9: Add Player Recipe Discovery Tracking

**Files:**
- Modify: `rust-server/src/db.rs` (or player data structure)
- Modify: `rust-server/src/protocol.rs`

**Step 1: Add discovered_recipes to player data**

Find where player data is stored (likely in db.rs or a player struct) and add a field to track discovered recipes:

```rust
pub discovered_recipes: HashSet<String>,
```

**Step 2: Add protocol message for discovered recipes**

In `rust-server/src/protocol.rs`, add to `ServerMessage`:

```rust
DiscoveredRecipes {
    recipes: Vec<String>,
},
RecipeDiscovered {
    recipe_id: String,
},
```

**Step 3: Send discovered recipes on player connect**

After sending `RecipeDefinitions`, also send `DiscoveredRecipes` with the player's unlocked recipe IDs.

**Step 4: Verify compilation**

Run: `cd rust-server && cargo check`
Expected: Compiles

**Step 5: Commit**

```bash
git add rust-server/src/db.rs rust-server/src/protocol.rs
git commit -m "feat: add player recipe discovery tracking"
```

---

### Task 10: Implement Timed Crafting with Interruption

**Files:**
- Modify: `rust-server/src/game.rs`
- Modify: `rust-server/src/protocol.rs`

**Step 1: Add crafting state tracking**

Add to player state (in game.rs or player struct):

```rust
pub struct CraftingState {
    pub recipe_id: String,
    pub started_at: Instant,
    pub duration_ms: u64,
}
```

**Step 2: Add protocol messages for crafting progress**

In `rust-server/src/protocol.rs`, add:

```rust
// Client -> Server
StartCraft { recipe_id: String },
CancelCraft,

// Server -> Client
CraftingStarted {
    recipe_id: String,
    duration_ms: u64,
},
CraftingProgress {
    progress: f32,  // 0.0 to 1.0
},
CraftingCancelled {
    reason: String,  // "cancelled", "interrupted", "moved"
},
CraftingCompleted {
    recipe_id: String,
    items_gained: Vec<RecipeResult>,
    xp_gained: u32,
},
```

**Step 3: Implement handle_start_craft()**

In `rust-server/src/game.rs`, add a new handler:

```rust
pub async fn handle_start_craft(&self, player_id: &str, recipe_id: &str) {
    // 1. Get recipe
    // 2. Validate: player alive, has materials, meets level req, at correct station
    // 3. Check recipe discovery if requires_discovery
    // 4. Start crafting timer
    // 5. Send CraftingStarted message
}
```

**Step 4: Implement crafting tick in game loop**

In the game loop, check active crafting states and:
- Send progress updates
- Complete crafting when timer expires
- Cancel if player takes damage or moves

**Step 5: Implement handle_cancel_craft()**

```rust
pub async fn handle_cancel_craft(&self, player_id: &str) {
    // 1. Check if player is crafting
    // 2. Cancel crafting state
    // 3. Refund materials (they weren't consumed yet)
    // 4. Send CraftingCancelled message
}
```

**Step 6: Verify compilation**

Run: `cd rust-server && cargo check`
Expected: Compiles

**Step 7: Commit**

```bash
git add rust-server/src/game.rs rust-server/src/protocol.rs
git commit -m "feat: implement timed crafting with interruption handling"
```

---

### Task 11: Add Smithing XP Rewards

**Files:**
- Modify: `rust-server/src/game.rs`

**Step 1: Grant Smithing XP on craft completion**

In the crafting completion logic, after adding items to inventory:

```rust
// Grant smithing XP
if recipe.xp > 0 {
    let skills = player.skills_mut();
    let leveled_up = skills.smithing.add_xp(recipe.xp);

    // Send XP drop notification
    self.send_to_player(player_id, ServerMessage::XpDrop {
        skill: "smithing".to_string(),
        amount: recipe.xp,
    }).await;

    if leveled_up {
        self.send_to_player(player_id, ServerMessage::LevelUp {
            skill: "smithing".to_string(),
            new_level: skills.smithing.level,
        }).await;
    }
}
```

**Step 2: Verify compilation**

Run: `cd rust-server && cargo check`
Expected: Compiles

**Step 3: Commit**

```bash
git add rust-server/src/game.rs
git commit -m "feat: grant smithing XP on successful craft"
```

---

## Phase 3: Client-Side UI

### Task 12: Add Client Crafting State

**Files:**
- Modify: `client/src/game/state.rs`
- Modify: `client/src/ui/mod.rs` (or ui_state)

**Step 1: Add crafting progress state**

In the UI state struct, add:

```rust
pub crafting_in_progress: bool,
pub crafting_recipe_id: Option<String>,
pub crafting_progress: f32,  // 0.0 to 1.0
pub crafting_duration_ms: u64,
pub crafting_started_at: Option<Instant>,
```

**Step 2: Add discovered recipes to game state**

In `GameState`, add:

```rust
pub discovered_recipes: HashSet<String>,
```

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Compiles

**Step 4: Commit**

```bash
git add client/src/game/state.rs client/src/ui/mod.rs
git commit -m "feat: add client crafting progress state"
```

---

### Task 13: Handle New Protocol Messages on Client

**Files:**
- Modify: `client/src/network/message_handler.rs`

**Step 1: Handle DiscoveredRecipes message**

```rust
ServerMessage::DiscoveredRecipes { recipes } => {
    state.discovered_recipes = recipes.into_iter().collect();
}
```

**Step 2: Handle RecipeDiscovered message**

```rust
ServerMessage::RecipeDiscovered { recipe_id } => {
    state.discovered_recipes.insert(recipe_id.clone());
    // Show notification: "Recipe Learned: {recipe_name}"
}
```

**Step 3: Handle CraftingStarted message**

```rust
ServerMessage::CraftingStarted { recipe_id, duration_ms } => {
    ui_state.crafting_in_progress = true;
    ui_state.crafting_recipe_id = Some(recipe_id);
    ui_state.crafting_duration_ms = duration_ms;
    ui_state.crafting_started_at = Some(Instant::now());
    ui_state.crafting_progress = 0.0;
}
```

**Step 4: Handle CraftingCancelled message**

```rust
ServerMessage::CraftingCancelled { reason } => {
    ui_state.crafting_in_progress = false;
    ui_state.crafting_recipe_id = None;
    // Show notification based on reason
}
```

**Step 5: Handle CraftingCompleted message**

```rust
ServerMessage::CraftingCompleted { recipe_id, items_gained, xp_gained } => {
    ui_state.crafting_in_progress = false;
    ui_state.crafting_recipe_id = None;
    // Show completion animation/notification
    // XP drop handled separately
}
```

**Step 6: Verify compilation**

Run: `cd client && cargo check`
Expected: Compiles

**Step 7: Commit**

```bash
git add client/src/network/message_handler.rs
git commit -m "feat: handle crafting protocol messages on client"
```

---

### Task 14: Update Crafting UI with Progress Bar

**Files:**
- Modify: `client/src/render/ui/crafting.rs`

**Step 1: Add progress bar rendering during crafting**

When `ui_state.crafting_in_progress` is true, replace the normal crafting panel content with:

```rust
fn render_crafting_progress(
    d: &mut RaylibDrawHandle,
    ui_state: &UiState,
    state: &GameState,
    bounds: Rectangle,
) {
    // Get the recipe being crafted
    let recipe = state.recipe_definitions.iter()
        .find(|r| Some(&r.id) == ui_state.crafting_recipe_id.as_ref());

    if let Some(recipe) = recipe {
        // Center content
        let center_x = bounds.x + bounds.width / 2.0;
        let center_y = bounds.y + bounds.height / 2.0;

        // "CRAFTING..." text
        draw_text_centered(d, "CRAFTING...", center_x, center_y - 80.0, 24, Color::WHITE);

        // Large sprite preview (pulsing)
        let sprite_size = 64.0;
        let pulse = (get_time() * 3.0).sin() * 0.1 + 1.0;  // Subtle pulse
        // Draw recipe result sprite scaled by pulse

        // Progress bar
        let bar_width = 200.0;
        let bar_height = 20.0;
        let bar_x = center_x - bar_width / 2.0;
        let bar_y = center_y + 40.0;

        // Background
        d.draw_rectangle(bar_x as i32, bar_y as i32, bar_width as i32, bar_height as i32, Color::DARKGRAY);

        // Fill
        let fill_width = bar_width * ui_state.crafting_progress;
        d.draw_rectangle(bar_x as i32, bar_y as i32, fill_width as i32, bar_height as i32, Color::GREEN);

        // Border
        d.draw_rectangle_lines(bar_x as i32, bar_y as i32, bar_width as i32, bar_height as i32, Color::WHITE);

        // Percentage text
        let percent = (ui_state.crafting_progress * 100.0) as i32;
        draw_text_centered(d, &format!("{}%", percent), center_x, bar_y + bar_height + 20.0, 18, Color::WHITE);

        // Cancel button
        let cancel_bounds = Rectangle::new(center_x - 60.0, bar_y + 60.0, 120.0, 36.0);
        if draw_button(d, "CANCEL", cancel_bounds, ui_state) {
            // Send cancel message
        }
    }
}
```

**Step 2: Update progress each frame**

In the crafting UI update logic:

```rust
if ui_state.crafting_in_progress {
    if let Some(started) = ui_state.crafting_started_at {
        let elapsed = started.elapsed().as_millis() as f32;
        let duration = ui_state.crafting_duration_ms as f32;
        ui_state.crafting_progress = (elapsed / duration).min(1.0);
    }
}
```

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Compiles

**Step 4: Commit**

```bash
git add client/src/render/ui/crafting.rs
git commit -m "feat: add crafting progress bar UI"
```

---

### Task 15: Filter Recipes by Discovery Status

**Files:**
- Modify: `client/src/render/ui/crafting.rs`

**Step 1: Update recipe filtering**

When filtering recipes for display, also check discovery status:

```rust
let recipes: Vec<&RecipeDefinition> = state.recipe_definitions.iter()
    .filter(|r| {
        // Category filter
        let category_match = if current_category == "supplies" {
            r.category == "consumables" || r.category == "materials"
        } else if current_category == "smithing" {
            r.category == "smithing"
        } else {
            r.category == current_category
        };

        // Discovery filter - hide undiscovered recipes that require discovery
        let discovery_ok = if r.requires_discovery {
            state.discovered_recipes.contains(&r.id)
        } else {
            true
        };

        category_match && discovery_ok
    })
    .collect();
```

**Step 2: Show locked recipes as "????" (optional)**

Alternatively, show locked recipes but grayed out with "????" name:

```rust
// In recipe list rendering
if r.requires_discovery && !state.discovered_recipes.contains(&r.id) {
    draw_text(d, "????", x, y, 16, Color::GRAY);
} else {
    draw_text(d, &r.display_name, x, y, 16, Color::WHITE);
}
```

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Compiles

**Step 4: Commit**

```bash
git add client/src/render/ui/crafting.rs
git commit -m "feat: filter crafting recipes by discovery status"
```

---

### Task 16: Add Station Requirement Display

**Files:**
- Modify: `client/src/render/ui/crafting.rs`

**Step 1: Show station requirement in recipe detail**

In the recipe detail panel, add station requirement display:

```rust
// After level requirement
if let Some(station) = &recipe.station {
    let station_name = match station.as_str() {
        "anvil" => "Anvil",
        "workbench" => "Workbench",
        _ => station,
    };

    // Check if player is near the required station
    let near_station = is_player_near_station(state, station);
    let color = if near_station { Color::GREEN } else { Color::RED };
    let icon = if near_station { "[+]" } else { "[-]" };

    draw_text(d, &format!("{} Station: {}", icon, station_name), x, y, 16, color);
}
```

**Step 2: Disable craft button if not at station**

```rust
let can_craft = has_all_materials
    && meets_level_req
    && (recipe.station.is_none() || is_player_near_station(state, recipe.station.as_ref().unwrap()));
```

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Compiles

**Step 4: Commit**

```bash
git add client/src/render/ui/crafting.rs
git commit -m "feat: show station requirement in crafting UI"
```

---

### Task 17: Add Smithing Skill to Skills Panel

**Files:**
- Modify: `client/src/render/ui/skills.rs`

**Step 1: Add Smithing to skills grid**

Find where skills are rendered in the skills panel and add Smithing:

```rust
// In the skills list/grid
let skills = [
    ("Hitpoints", state.skills.hitpoints.level),
    ("Combat", state.skills.combat.level),
    ("Fishing", state.skills.fishing.level),
    ("Farming", state.skills.farming.level),
    ("Smithing", state.skills.smithing.level),  // ADD THIS
];
```

**Step 2: Verify compilation**

Run: `cd client && cargo check`
Expected: Compiles

**Step 3: Commit**

```bash
git add client/src/render/ui/skills.rs
git commit -m "feat: add Smithing to skills panel"
```

---

## Phase 4: Recipe Scroll Usage

### Task 18: Implement Recipe Scroll Consumption

**Files:**
- Modify: `rust-server/src/game.rs`

**Step 1: Add UseItem handler for recipe scrolls**

When a player uses a recipe scroll item:

```rust
pub async fn handle_use_item(&self, player_id: &str, item_id: &str, slot: usize) {
    // Check if item is a recipe scroll
    if item_id.starts_with("recipe_") {
        let recipe_id = item_id.strip_prefix("recipe_").unwrap();

        // Check if recipe exists
        if self.crafting_registry.get(recipe_id).is_none() {
            self.send_error(player_id, "Unknown recipe").await;
            return;
        }

        // Check if already discovered
        let player = self.get_player_mut(player_id).await;
        if player.discovered_recipes.contains(recipe_id) {
            self.send_error(player_id, "You already know this recipe").await;
            return;
        }

        // Consume scroll
        player.inventory.remove_item(slot, 1);

        // Add to discovered recipes
        player.discovered_recipes.insert(recipe_id.to_string());

        // Save to database
        self.save_player_recipes(player_id).await;

        // Notify client
        self.send_to_player(player_id, ServerMessage::RecipeDiscovered {
            recipe_id: recipe_id.to_string(),
        }).await;

        self.send_to_player(player_id, ServerMessage::InventoryUpdate {
            inventory: player.inventory.to_client(),
        }).await;
    }
}
```

**Step 2: Verify compilation**

Run: `cd rust-server && cargo check`
Expected: Compiles

**Step 3: Commit**

```bash
git add rust-server/src/game.rs
git commit -m "feat: implement recipe scroll consumption"
```

---

## Phase 5: Integration & Polish

### Task 19: Add Smithing Category Tab to Crafting UI

**Files:**
- Modify: `client/src/render/ui/crafting.rs`

**Step 1: Add "Smithing" as a category tab**

Find where category tabs are defined and add:

```rust
let categories = ["supplies", "smithing"];  // Add smithing
```

**Step 2: Handle smithing category filtering**

Already handled in Task 15, but verify the category filter includes:

```rust
} else if current_category == "smithing" {
    r.category == "smithing"
}
```

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Compiles

**Step 4: Commit**

```bash
git add client/src/render/ui/crafting.rs
git commit -m "feat: add Smithing category tab to crafting UI"
```

---

### Task 20: Add Crafting Completion Animation

**Files:**
- Modify: `client/src/render/ui/crafting.rs`

**Step 1: Add completion state**

```rust
pub crafting_complete_animation: Option<(String, Instant)>,  // (recipe_id, started_at)
```

**Step 2: Trigger animation on completion**

When `CraftingCompleted` is received:

```rust
ui_state.crafting_complete_animation = Some((recipe_id.clone(), Instant::now()));
```

**Step 3: Render completion animation**

```rust
if let Some((recipe_id, started)) = &ui_state.crafting_complete_animation {
    let elapsed = started.elapsed().as_secs_f32();
    if elapsed < 1.0 {
        // Draw "Crafted!" text with fade and scale
        let alpha = (1.0 - elapsed) * 255.0;
        let scale = 1.0 + elapsed * 0.5;
        // Render item sprite with pop effect
    } else {
        ui_state.crafting_complete_animation = None;
    }
}
```

**Step 4: Verify compilation**

Run: `cd client && cargo check`
Expected: Compiles

**Step 5: Commit**

```bash
git add client/src/render/ui/crafting.rs
git commit -m "feat: add crafting completion animation"
```

---

### Task 21: Final Integration Test

**Step 1: Start server**

```bash
cd rust-server && cargo run
```

**Step 2: Start client**

```bash
cd client && cargo run
```

**Step 3: Manual test checklist**

- [ ] Smithing skill appears in skills panel at level 1
- [ ] Smithing recipes appear in crafting UI under "Smithing" tab
- [ ] Recipes requiring discovery show as "????" until learned
- [ ] Recipe scroll can be used to learn recipes
- [ ] Crafting starts with progress bar when clicking Craft
- [ ] Progress bar fills over correct duration
- [ ] Cancel button works and refunds materials
- [ ] Taking damage interrupts crafting
- [ ] Crafting completes and grants items + XP
- [ ] XP drop appears for Smithing
- [ ] Level up notification works for Smithing

**Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: integration test fixes"
```

---

### Task 22: Final Commit and Branch Ready

**Step 1: Ensure all changes committed**

```bash
git status
```

**Step 2: Create summary commit if needed**

```bash
git log --oneline -10  # Review commits
```

**Step 3: Branch is ready for review/merge**

The `feature/monster-crafting` branch is complete and ready for:
- Code review
- Merge to master
- Or further iteration

---

## Summary

**Total Tasks:** 22
**Estimated Time:** 4-6 hours of focused implementation

**Key Files Modified:**
- `rust-server/src/skills.rs` - Smithing skill
- `rust-server/src/crafting/` - Extended recipe system
- `rust-server/src/game.rs` - Timed crafting logic
- `rust-server/src/protocol.rs` - New messages
- `rust-server/data/recipes/smithing.toml` - Recipes
- `rust-server/data/items/*.toml` - New items
- `client/src/render/ui/crafting.rs` - Progress UI
- `client/src/game/state.rs` - Client state

**Key Features Delivered:**
1. New Smithing skill with XP progression
2. 14 new recipes (6 components, 6 gear, 2 ammo variants)
3. Station-based crafting (anvil/workbench)
4. Recipe discovery via NPC teaching + scroll drops
5. Timed crafting with progress bar
6. Cancellation and interruption handling
7. Interactive crafting UI with previews
