# Potion System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a complete potion crafting pipeline: Alchemy skill, herb farming/foraging, potion brewing with tiered recipes, buff system, and a Witch NPC merchant.

**Architecture:** Extends existing systems (Skills, Crafting, Farming, Gathering, Shops) with minimal new code. New Alchemy skill + recipe category routes through existing crafting infrastructure. Buff system adds `active_buffs` to Player with tick-based expiry. All data is TOML-driven following established patterns.

**Tech Stack:** Rust server (Axum/Tokio), TOML data files, existing MessagePack protocol

**Design doc:** `docs/plans/2026-02-10-potion-system-design.md`

---

### Task 1: Add Alchemy Skill

**Files:**
- Modify: `rust-server/src/skills.rs`

**Step 1: Add Alchemy to SkillType enum**

In `rust-server/src/skills.rs`, add `Alchemy` variant to `SkillType` enum (line 20-29):

```rust
pub enum SkillType {
    Hitpoints,
    Combat,
    Fishing,
    Farming,
    Smithing,
    Prayer,
    Magic,
    Woodcutting,
    Alchemy,
}
```

**Step 2: Add Alchemy to all SkillType match arms**

In `as_str()` (line 32-43), add:
```rust
SkillType::Alchemy => "alchemy",
```

In `from_str()` (line 45-57), add:
```rust
"alchemy" => Some(SkillType::Alchemy),
```

In `all()` (line 59-70), add `SkillType::Alchemy` to the array.

**Step 3: Add alchemy field to Skills struct**

In `Skills` struct (line 164-180), add:
```rust
#[serde(default)]
pub alchemy: Skill,
```

In `Skills::new()` (line 189-201), add:
```rust
alchemy: Skill::new(1),
```

In `Skills::get()` (line 210-221), add:
```rust
SkillType::Alchemy => &self.alchemy,
```

In `Skills::get_mut()` (line 224-235), add:
```rust
SkillType::Alchemy => &mut self.alchemy,
```

In `Skills::total_level()` (line 238-239), add `+ self.alchemy.level`.

**Step 4: Update test fixtures**

In `test_total_level()` (line 390-394), update expected total from 19 to 20 (new skill at level 1).

In `test_combat_level()` max_skills fixture (line 375-384), add:
```rust
alchemy: Skill::new(1),
```

**Step 5: Verify tests pass**

Run: `cd rust-server && cargo test skills`
Expected: All tests pass

**Step 6: Verify compilation**

Run: `cd rust-server && cargo check 2>&1 | grep error`
Expected: No errors

**Step 7: Commit**

```bash
git add rust-server/src/skills.rs
git commit -m "feat: add Alchemy skill to skills system"
```

---

### Task 2: Add Alchemy Recipe Category & Update Craft Handlers

**Files:**
- Modify: `rust-server/src/crafting/definition.rs`
- Modify: `rust-server/src/game.rs`

**Step 1: Add Alchemy to RecipeCategory enum**

In `rust-server/src/crafting/definition.rs` (line 11-17), add `Alchemy` variant:

```rust
pub enum RecipeCategory {
    Consumables,
    Materials,
    Equipment,
    Tools,
    Smithing,
    Alchemy,
}
```

In `RecipeCategory::as_str()` (line 26-35), add:
```rust
RecipeCategory::Alchemy => "alchemy",
```

**Step 2: Update `handle_start_craft` skill routing**

In `rust-server/src/game.rs` at lines 4214-4226, the level check currently routes Smithing to `skills.smithing.level` and everything else to `combat_level()`. Add Alchemy routing:

```rust
// Check level requirement - route to correct skill
let level_check_passed = match recipe.category {
    RecipeCategory::Smithing => player.skills.smithing.level >= recipe.level_required,
    RecipeCategory::Alchemy => player.skills.alchemy.level >= recipe.level_required,
    _ => player.combat_level() >= recipe.level_required,
};

if !level_check_passed {
    let skill_name = match recipe.category {
        RecipeCategory::Smithing => "Smithing",
        RecipeCategory::Alchemy => "Alchemy",
        _ => "Combat",
    };
```

**Step 3: Update `handle_craft` skill routing**

In `rust-server/src/game.rs` at lines 4067-4081, apply the same routing logic:

```rust
// Check level requirement - route to correct skill
let level_check_passed = match recipe.category {
    RecipeCategory::Smithing => player.skills.smithing.level >= recipe.level_required,
    RecipeCategory::Alchemy => player.skills.alchemy.level >= recipe.level_required,
    _ => player.combat_level() >= recipe.level_required,
};

if !level_check_passed {
    let skill_name = match recipe.category {
        RecipeCategory::Smithing => "Smithing",
        RecipeCategory::Alchemy => "Alchemy",
        _ => "Combat",
    };
    drop(players);
    self.send_to_player(
        player_id,
        ServerMessage::CraftResult {
            success: false,
            recipe_id: recipe_id.to_string(),
            error: Some(format!("Requires {} level {}", skill_name, recipe.level_required)),
            items_gained: vec![],
        },
    )
    .await;
    return;
}
```

**Step 4: Add Alchemy XP grant on successful craft**

In `handle_craft`, after adding results to inventory (after line 4137), add XP grant for alchemy recipes:

```rust
// Grant alchemy XP if this is an alchemy recipe
if recipe.category == RecipeCategory::Alchemy && recipe.xp > 0 {
    let leveled = player.skills.alchemy.add_xp(recipe.xp as i64);
    if leveled {
        // Send level up notification (pattern from other skill grants)
    }
}
```

Do the same in the timed craft completion path in `handle_start_craft`.

**Step 5: Verify compilation**

Run: `cd rust-server && cargo check 2>&1 | grep error`
Expected: No errors

**Step 6: Commit**

```bash
git add rust-server/src/crafting/definition.rs rust-server/src/game.rs
git commit -m "feat: add Alchemy recipe category with skill-based level routing"
```

---

### Task 3: Add Herb & Vial Item Definitions

**Files:**
- Modify: `rust-server/data/items/materials.toml`

**Step 1: Add herb items and vial to materials.toml**

Add a new section after the Farming Produce section in `rust-server/data/items/materials.toml`:

```toml
# =============================================================================
# Herbs (Alchemy Ingredients)
# =============================================================================

[greenleaf]
display_name = "Greenleaf"
sprite = "greenleaf"
description = "A common herb with mild restorative properties. Used in basic alchemy."
category = "material"
max_stack = 99
base_price = 10
sellable = true

[tangleroots]
display_name = "Tangleroots"
sprite = "tangleroots"
description = "Twisted roots with potent energy. A versatile alchemy ingredient."
category = "material"
max_stack = 99
base_price = 20
sellable = true

[marshbloom]
display_name = "Marshbloom"
sprite = "marshbloom"
description = "A swamp flower that thrives in damp conditions. Essential for mid-tier potions."
category = "material"
max_stack = 99
base_price = 35
sellable = true

[ashveil]
display_name = "Ashveil"
sprite = "ashveil"
description = "A rare herb that grows in volcanic soil. Used in powerful potions."
category = "material"
max_stack = 99
base_price = 60
sellable = true

[nightthorn]
display_name = "Nightthorn"
sprite = "nightthorn"
description = "A thorny herb that only blooms at night. Prized by alchemists."
category = "material"
max_stack = 99
base_price = 100
sellable = true

[bloodcap]
display_name = "Bloodcap"
sprite = "bloodcap"
description = "A crimson mushroom of legendary potency. The finest alchemy ingredient."
category = "material"
max_stack = 99
base_price = 180
sellable = true

[vial_of_water]
display_name = "Vial of Water"
sprite = "vial_of_water"
description = "A glass vial filled with clean water. Base container for all potions."
category = "material"
max_stack = 99
base_price = 5
sellable = true
```

**Step 2: Verify server starts**

Run: `cd rust-server && cargo run` (briefly, Ctrl+C after startup logs)
Expected: No errors loading items

**Step 3: Commit**

```bash
git add rust-server/data/items/materials.toml
git commit -m "feat: add herb items and vial of water for alchemy"
```

---

### Task 4: Add Herb Seeds & Farming Crops

**Files:**
- Modify: `rust-server/data/items/seeds.toml`
- Modify: `rust-server/data/farming_patches.toml`

**Step 1: Add herb seed items to seeds.toml**

Append to `rust-server/data/items/seeds.toml`:

```toml
# =============================================================================
# Herb Seeds (Alchemy)
# =============================================================================

[greenleaf_seed]
display_name = "Greenleaf Seed"
sprite = "greenleaf_seed"
description = "A seed that grows into a greenleaf herb. Requires Farming level 1."
category = "material"
max_stack = 99
base_price = 8
sellable = true

[tangleroots_seed]
display_name = "Tangleroots Seed"
sprite = "tangleroots_seed"
description = "A seed that grows into tangleroots. Requires Farming level 10."
category = "material"
max_stack = 99
base_price = 18
sellable = true

[marshbloom_seed]
display_name = "Marshbloom Seed"
sprite = "marshbloom_seed"
description = "A seed that grows into marshbloom. Requires Farming level 20."
category = "material"
max_stack = 99
base_price = 30
sellable = true

[ashveil_seed]
display_name = "Ashveil Seed"
sprite = "ashveil_seed"
description = "A seed that grows into ashveil. Requires Farming level 35."
category = "material"
max_stack = 99
base_price = 55
sellable = true

[nightthorn_seed]
display_name = "Nightthorn Seed"
sprite = "nightthorn_seed"
description = "A seed that grows into nightthorn. Requires Farming level 50."
category = "material"
max_stack = 99
base_price = 90
sellable = true

[bloodcap_seed]
display_name = "Bloodcap Seed"
sprite = "bloodcap_seed"
description = "A rare spore that grows into a bloodcap mushroom. Requires Farming level 70."
category = "material"
max_stack = 99
base_price = 160
sellable = true
```

**Step 2: Add herb crops to farming_patches.toml**

Append to `rust-server/data/farming_patches.toml`:

```toml
# =============================================================================
# Herbs (Alchemy)
# =============================================================================

[greenleaf]
seed_item = "greenleaf_seed"
produce_item = "greenleaf"
level_required = 1
growth_time_minutes = 5
growth_stages = 4
harvest_amount_min = 1
harvest_amount_max = 2
xp_planting = 3
xp_per_harvest = 10
seed_return_chance = 0.15

[tangleroots]
seed_item = "tangleroots_seed"
produce_item = "tangleroots"
level_required = 10
growth_time_minutes = 8
growth_stages = 4
harvest_amount_min = 1
harvest_amount_max = 2
xp_planting = 5
xp_per_harvest = 18
seed_return_chance = 0.15

[marshbloom]
seed_item = "marshbloom_seed"
produce_item = "marshbloom"
level_required = 20
growth_time_minutes = 12
growth_stages = 4
harvest_amount_min = 1
harvest_amount_max = 2
xp_planting = 8
xp_per_harvest = 28
seed_return_chance = 0.15

[ashveil]
seed_item = "ashveil_seed"
produce_item = "ashveil"
level_required = 35
growth_time_minutes = 15
growth_stages = 4
harvest_amount_min = 1
harvest_amount_max = 2
xp_planting = 12
xp_per_harvest = 40
seed_return_chance = 0.15

[nightthorn]
seed_item = "nightthorn_seed"
produce_item = "nightthorn"
level_required = 50
growth_time_minutes = 20
growth_stages = 4
harvest_amount_min = 1
harvest_amount_max = 2
xp_planting = 18
xp_per_harvest = 55
seed_return_chance = 0.15

[bloodcap]
seed_item = "bloodcap_seed"
produce_item = "bloodcap"
level_required = 70
growth_time_minutes = 25
growth_stages = 4
harvest_amount_min = 1
harvest_amount_max = 1
xp_planting = 25
xp_per_harvest = 75
seed_return_chance = 0.10
```

**Step 3: Add herb seeds to existing Seed Shop**

Append to `rust-server/data/shops/seed_shop.toml`:

```toml
# Herb Seeds
[[stock]]
item_id = "greenleaf_seed"
max_quantity = 15
restock_rate = 3
restock_interval_minutes = 5

[[stock]]
item_id = "tangleroots_seed"
max_quantity = 10
restock_rate = 2
restock_interval_minutes = 5

[[stock]]
item_id = "marshbloom_seed"
max_quantity = 8
restock_rate = 2
restock_interval_minutes = 5

[[stock]]
item_id = "ashveil_seed"
max_quantity = 5
restock_rate = 1
restock_interval_minutes = 5

[[stock]]
item_id = "nightthorn_seed"
max_quantity = 3
restock_rate = 1
restock_interval_minutes = 5

[[stock]]
item_id = "bloodcap_seed"
max_quantity = 2
restock_rate = 1
restock_interval_minutes = 5
```

**Step 4: Verify server starts**

Run: `cd rust-server && cargo run` (briefly)
Expected: No errors, farming system loads new crops

**Step 5: Commit**

```bash
git add rust-server/data/items/seeds.toml rust-server/data/farming_patches.toml rust-server/data/shops/seed_shop.toml
git commit -m "feat: add herb seeds and farming crops for alchemy pipeline"
```

---

### Task 5: Add Wild Herb Gathering Zones

**Files:**
- Modify: `rust-server/data/gathering_zones.toml`
- Modify: `rust-server/data/loot_tables.toml`

**Step 1: Add herb gathering zones**

Append to `rust-server/data/gathering_zones.toml`:

```toml
# =============================================================================
# Herb Gathering Zones (Farming skill)
# =============================================================================

[zones.forest_herbs]
skill = "farming"
level_required = 1
loot_table = "herbs_beginner"
bonus_spawn_frequency = 60
base_gather_speed = 6.0
base_xp = 8

[zones.swamp_herbs]
skill = "farming"
level_required = 15
loot_table = "herbs_intermediate"
bonus_spawn_frequency = 55
base_gather_speed = 5.5
base_xp = 20
```

**Step 2: Add herb loot tables**

Append to `rust-server/data/loot_tables.toml`:

```toml
# =============================================================================
# Herb Loot Tables (Farming)
# =============================================================================

[herbs_beginner]
skill = "farming"

[herbs_beginner.tiers.common]
base_weight = 70
level_scaling = -0.5
items = [
    { id = "greenleaf", level = 1, weight = 10, xp_bonus = 0 },
]

[herbs_beginner.tiers.uncommon]
base_weight = 25
level_scaling = 0.3
items = [
    { id = "tangleroots", level = 10, weight = 10, xp_bonus = 8 },
]

[herbs_beginner.tiers.rare]
base_weight = 5
level_scaling = 0.2
items = [
    { id = "marshbloom", level = 20, weight = 10, xp_bonus = 18 },
    { id = "greenleaf_seed", level = 1, weight = 3, xp_bonus = 5 },
    { id = "tangleroots_seed", level = 10, weight = 2, xp_bonus = 10 },
]

[herbs_intermediate]
skill = "farming"

[herbs_intermediate.tiers.common]
base_weight = 60
level_scaling = -0.5
items = [
    { id = "tangleroots", level = 10, weight = 10, xp_bonus = 0 },
    { id = "marshbloom", level = 20, weight = 8, xp_bonus = 5 },
]

[herbs_intermediate.tiers.uncommon]
base_weight = 30
level_scaling = 0.3
items = [
    { id = "marshbloom", level = 20, weight = 10, xp_bonus = 10 },
]

[herbs_intermediate.tiers.rare]
base_weight = 10
level_scaling = 0.2
items = [
    { id = "tangleroots_seed", level = 10, weight = 3, xp_bonus = 8 },
    { id = "marshbloom_seed", level = 20, weight = 2, xp_bonus = 15 },
]
```

**Step 3: Verify server starts**

Run: `cd rust-server && cargo run` (briefly)
Expected: No errors, gathering zones load

**Step 4: Commit**

```bash
git add rust-server/data/gathering_zones.toml rust-server/data/loot_tables.toml
git commit -m "feat: add wild herb gathering zones with farming-based loot tables"
```

---

### Task 6: Add Potion Item Definitions

**Files:**
- Modify: `rust-server/data/items/consumables.toml`

**Step 1: Add all potion items**

Append to `rust-server/data/items/consumables.toml`:

```toml
# ============================================================================
# Alchemy Potions - Restoration
# ============================================================================

[weak_health_potion]
display_name = "Weak Health Potion"
sprite = "item_weak_health_potion"
description = "A basic herbal remedy. Restores 15 HP."
category = "consumable"
max_stack = 10
base_price = 15
sellable = true

[weak_health_potion.use_effect]
type = "heal"
amount = 15

[weak_mana_potion]
display_name = "Weak Mana Potion"
sprite = "item_weak_mana_potion"
description = "A mild arcane infusion. Restores 10 MP."
category = "consumable"
max_stack = 10
base_price = 18
sellable = true

[weak_mana_potion.use_effect]
type = "restore_mana"
amount = 10

[mana_potion]
display_name = "Mana Potion"
sprite = "item_mana_potion"
description = "Restores 20 MP when consumed."
category = "consumable"
max_stack = 10
base_price = 35
sellable = true

[mana_potion.use_effect]
type = "restore_mana"
amount = 20

[strong_health_potion]
display_name = "Strong Health Potion"
sprite = "item_strong_health_potion"
description = "A powerful restorative brew. Restores 50 HP."
category = "consumable"
max_stack = 10
base_price = 60
sellable = true

[strong_health_potion.use_effect]
type = "heal"
amount = 50

[strong_mana_potion]
display_name = "Strong Mana Potion"
sprite = "item_strong_mana_potion"
description = "A potent arcane elixir. Restores 35 MP."
category = "consumable"
max_stack = 10
base_price = 75
sellable = true

[strong_mana_potion.use_effect]
type = "restore_mana"
amount = 35

[strong_prayer_potion]
display_name = "Strong Prayer Potion"
sprite = "item_strong_prayer_potion"
description = "A blessed elixir. Restores 12 + floor(level/3) prayer points."
category = "consumable"
max_stack = 10
base_price = 80
sellable = true

[strong_prayer_potion.use_effect]
type = "restore_prayer"
amount = 12

# ============================================================================
# Alchemy Potions - Stat Buffs
# ============================================================================

[attack_potion]
display_name = "Attack Potion"
sprite = "item_attack_potion"
description = "Temporarily boosts attack bonus by 5 for 60 seconds."
category = "consumable"
max_stack = 10
base_price = 40
sellable = true

[attack_potion.use_effect]
type = "buff"
stat = "attack"
amount = 5
duration_ms = 60000

[strength_potion]
display_name = "Strength Potion"
sprite = "item_strength_potion"
description = "Temporarily boosts strength bonus by 5 for 60 seconds."
category = "consumable"
max_stack = 10
base_price = 40
sellable = true

[strength_potion.use_effect]
type = "buff"
stat = "strength"
amount = 5
duration_ms = 60000

[defence_potion]
display_name = "Defence Potion"
sprite = "item_defence_potion"
description = "Temporarily boosts defence bonus by 5 for 60 seconds."
category = "consumable"
max_stack = 10
base_price = 40
sellable = true

[defence_potion.use_effect]
type = "buff"
stat = "defence"
amount = 5
duration_ms = 60000

[super_attack_potion]
display_name = "Super Attack Potion"
sprite = "item_super_attack_potion"
description = "Greatly boosts attack bonus by 10 for 90 seconds."
category = "consumable"
max_stack = 10
base_price = 100
sellable = true

[super_attack_potion.use_effect]
type = "buff"
stat = "attack"
amount = 10
duration_ms = 90000

[super_strength_potion]
display_name = "Super Strength Potion"
sprite = "item_super_strength_potion"
description = "Greatly boosts strength bonus by 10 for 90 seconds."
category = "consumable"
max_stack = 10
base_price = 100
sellable = true

[super_strength_potion.use_effect]
type = "buff"
stat = "strength"
amount = 10
duration_ms = 90000

[super_defence_potion]
display_name = "Super Defence Potion"
sprite = "item_super_defence_potion"
description = "Greatly boosts defence bonus by 10 for 90 seconds."
category = "consumable"
max_stack = 10
base_price = 100
sellable = true

[super_defence_potion.use_effect]
type = "buff"
stat = "defence"
amount = 10
duration_ms = 90000

# ============================================================================
# Alchemy Potions - Utility
# ============================================================================

[antidote]
display_name = "Antidote"
sprite = "item_antidote"
description = "Cures poison and grants brief immunity. A wise precaution in dangerous lands."
category = "consumable"
max_stack = 10
base_price = 25
sellable = true

[antidote.use_effect]
type = "heal"
amount = 0
```

**Note:** The existing `health_potion` (30 HP) stays as-is — it becomes the mid-tier potion sold by the Alchemist. The new `weak_health_potion` (15 HP) is the entry-level one. The `antidote` uses a placeholder heal(0) effect until poison is implemented. The `strong_prayer_potion` uses `restore_prayer` with amount 12 — the formula `amount + level/N` is already implemented in the handler (currently uses `/4`, the strong version will need a separate handler or the same formula applies).

**Step 2: Verify server starts**

Run: `cd rust-server && cargo run` (briefly)
Expected: Items load without errors

**Step 3: Commit**

```bash
git add rust-server/data/items/consumables.toml
git commit -m "feat: add potion item definitions for alchemy system"
```

---

### Task 7: Add Potion Crafting Recipes

**Files:**
- Create: `rust-server/data/recipes/alchemy.toml`

**Step 1: Create alchemy recipes file**

Create `rust-server/data/recipes/alchemy.toml`:

```toml
# =============================================================================
# Alchemy Recipes - Potions
# =============================================================================

# --- Restoration: Weak Tier (Herb + Vial) ---

[weak_health_potion]
display_name = "Weak Health Potion"
description = "Brew a basic health potion from greenleaf."
category = "alchemy"
level_required = 1
xp = 25

[[weak_health_potion.ingredients]]
item_id = "greenleaf"
count = 1

[[weak_health_potion.ingredients]]
item_id = "vial_of_water"
count = 1

[[weak_health_potion.results]]
item_id = "weak_health_potion"
count = 1

# ---

[weak_mana_potion]
display_name = "Weak Mana Potion"
description = "Brew a basic mana potion from tangleroots."
category = "alchemy"
level_required = 5
xp = 30

[[weak_mana_potion.ingredients]]
item_id = "tangleroots"
count = 1

[[weak_mana_potion.ingredients]]
item_id = "vial_of_water"
count = 1

[[weak_mana_potion.results]]
item_id = "weak_mana_potion"
count = 1

# --- Utility ---

[antidote]
display_name = "Antidote"
description = "Brew an antidote from greenleaf and hedgehog spine."
category = "alchemy"
level_required = 10
xp = 35

[[antidote.ingredients]]
item_id = "greenleaf"
count = 1

[[antidote.ingredients]]
item_id = "hedgehog_spine"
count = 1

[[antidote.ingredients]]
item_id = "vial_of_water"
count = 1

[[antidote.results]]
item_id = "antidote"
count = 1

# --- Stat Buffs: Basic Tier (Herb + Secondary + Vial) ---

[attack_potion]
display_name = "Attack Potion"
description = "Brew a potion that temporarily boosts attack."
category = "alchemy"
level_required = 15
xp = 50

[[attack_potion.ingredients]]
item_id = "tangleroots"
count = 1

[[attack_potion.ingredients]]
item_id = "spider_fang"
count = 1

[[attack_potion.ingredients]]
item_id = "vial_of_water"
count = 1

[[attack_potion.results]]
item_id = "attack_potion"
count = 1

# --- Restoration: Mid Tier (Herb + Secondary + Vial) ---

[health_potion_recipe]
display_name = "Health Potion"
description = "Brew a health potion from marshbloom and slime core."
category = "alchemy"
level_required = 20
xp = 60

[[health_potion_recipe.ingredients]]
item_id = "marshbloom"
count = 1

[[health_potion_recipe.ingredients]]
item_id = "slime_core"
count = 1

[[health_potion_recipe.ingredients]]
item_id = "vial_of_water"
count = 1

[[health_potion_recipe.results]]
item_id = "health_potion"
count = 1

[strength_potion]
display_name = "Strength Potion"
description = "Brew a potion that temporarily boosts strength."
category = "alchemy"
level_required = 20
xp = 60

[[strength_potion.ingredients]]
item_id = "marshbloom"
count = 1

[[strength_potion.ingredients]]
item_id = "slime_core"
count = 1

[[strength_potion.ingredients]]
item_id = "vial_of_water"
count = 1

[[strength_potion.results]]
item_id = "strength_potion"
count = 1

[mana_potion_recipe]
display_name = "Mana Potion"
description = "Brew a mana potion from marshbloom and worm segment."
category = "alchemy"
level_required = 25
xp = 65

[[mana_potion_recipe.ingredients]]
item_id = "marshbloom"
count = 1

[[mana_potion_recipe.ingredients]]
item_id = "worm_segment"
count = 1

[[mana_potion_recipe.ingredients]]
item_id = "vial_of_water"
count = 1

[[mana_potion_recipe.results]]
item_id = "mana_potion"
count = 1

[defence_potion]
display_name = "Defence Potion"
description = "Brew a potion that temporarily boosts defence."
category = "alchemy"
level_required = 25
xp = 65

[[defence_potion.ingredients]]
item_id = "marshbloom"
count = 1

[[defence_potion.ingredients]]
item_id = "hedgehog_spine"
count = 1

[[defence_potion.ingredients]]
item_id = "vial_of_water"
count = 1

[[defence_potion.results]]
item_id = "defence_potion"
count = 1

[prayer_potion_recipe]
display_name = "Prayer Potion"
description = "Brew a prayer potion from tangleroots and crow feather."
category = "alchemy"
level_required = 30
xp = 70

[[prayer_potion_recipe.ingredients]]
item_id = "tangleroots"
count = 1

[[prayer_potion_recipe.ingredients]]
item_id = "crow_feather"
count = 1

[[prayer_potion_recipe.ingredients]]
item_id = "vial_of_water"
count = 1

[[prayer_potion_recipe.results]]
item_id = "prayer_potion"
count = 1

# --- Restoration: Strong Tier (High Herb + Rare Secondary + Vial) ---

[strong_health_potion]
display_name = "Strong Health Potion"
description = "Brew a powerful health potion from ashveil and spider fang."
category = "alchemy"
level_required = 45
xp = 100

[[strong_health_potion.ingredients]]
item_id = "ashveil"
count = 1

[[strong_health_potion.ingredients]]
item_id = "spider_fang"
count = 1

[[strong_health_potion.ingredients]]
item_id = "vial_of_water"
count = 1

[[strong_health_potion.results]]
item_id = "strong_health_potion"
count = 1

[strong_mana_potion]
display_name = "Strong Mana Potion"
description = "Brew a potent mana elixir from ashveil and dark essence."
category = "alchemy"
level_required = 50
xp = 110

[[strong_mana_potion.ingredients]]
item_id = "ashveil"
count = 1

[[strong_mana_potion.ingredients]]
item_id = "dark_essence"
count = 1

[[strong_mana_potion.ingredients]]
item_id = "vial_of_water"
count = 1

[[strong_mana_potion.results]]
item_id = "strong_mana_potion"
count = 1

# --- Stat Buffs: Super Tier ---

[super_attack_potion]
display_name = "Super Attack Potion"
description = "Brew a powerful attack potion from nightthorn and spider fang."
category = "alchemy"
level_required = 55
xp = 110

[[super_attack_potion.ingredients]]
item_id = "nightthorn"
count = 1

[[super_attack_potion.ingredients]]
item_id = "spider_fang"
count = 1

[[super_attack_potion.ingredients]]
item_id = "vial_of_water"
count = 1

[[super_attack_potion.results]]
item_id = "super_attack_potion"
count = 1

[strong_prayer_potion]
display_name = "Strong Prayer Potion"
description = "Brew a blessed elixir from nightthorn and crow feather."
category = "alchemy"
level_required = 55
xp = 120

[[strong_prayer_potion.ingredients]]
item_id = "nightthorn"
count = 1

[[strong_prayer_potion.ingredients]]
item_id = "crow_feather"
count = 1

[[strong_prayer_potion.ingredients]]
item_id = "vial_of_water"
count = 1

[[strong_prayer_potion.results]]
item_id = "strong_prayer_potion"
count = 1

[super_strength_potion]
display_name = "Super Strength Potion"
description = "Brew a powerful strength potion from nightthorn and dark essence."
category = "alchemy"
level_required = 60
xp = 120

[[super_strength_potion.ingredients]]
item_id = "nightthorn"
count = 1

[[super_strength_potion.ingredients]]
item_id = "dark_essence"
count = 1

[[super_strength_potion.ingredients]]
item_id = "vial_of_water"
count = 1

[[super_strength_potion.results]]
item_id = "super_strength_potion"
count = 1

[super_defence_potion]
display_name = "Super Defence Potion"
description = "Brew a powerful defence potion from bloodcap and hedgehog spine."
category = "alchemy"
level_required = 65
xp = 130

[[super_defence_potion.ingredients]]
item_id = "bloodcap"
count = 1

[[super_defence_potion.ingredients]]
item_id = "hedgehog_spine"
count = 1

[[super_defence_potion.ingredients]]
item_id = "vial_of_water"
count = 1

[[super_defence_potion.results]]
item_id = "super_defence_potion"
count = 1
```

**Step 2: Verify server loads recipes**

Run: `cd rust-server && cargo run` (briefly)
Expected: Recipe registry loads alchemy.toml without errors

**Step 3: Commit**

```bash
git add rust-server/data/recipes/alchemy.toml
git commit -m "feat: add alchemy potion crafting recipes"
```

---

### Task 8: Add Witch NPC & Shop

**Files:**
- Create: `rust-server/data/shops/witch.toml`
- Modify: `rust-server/data/entities/npcs/villagers.toml`

**Step 1: Create witch shop**

Create `rust-server/data/shops/witch.toml`:

```toml
id = "witch"
display_name = "The Witch's Cauldron"

[[stock]]
item_id = "vial_of_water"
max_quantity = 50
restock_rate = 10

[[stock]]
item_id = "weak_health_potion"
max_quantity = 10
restock_rate = 2

[[stock]]
item_id = "weak_mana_potion"
max_quantity = 10
restock_rate = 2

[[stock]]
item_id = "antidote"
max_quantity = 5
restock_rate = 1

[[stock]]
item_id = "greenleaf_seed"
max_quantity = 10
restock_rate = 3

[[stock]]
item_id = "tangleroots_seed"
max_quantity = 5
restock_rate = 2
```

**Step 2: Add witch NPC prototype**

Append to `rust-server/data/entities/npcs/villagers.toml`:

```toml
# ============================================================================
# Witch Hazel - Alchemy Merchant
# ============================================================================
[witch_hazel]
display_name = "Witch Hazel"
sprite = "witch"
animation_type = "humanoid"
description = "A mysterious witch who brews potions and sells alchemy supplies. Her cauldron bubbles with strange concoctions."

[witch_hazel.stats]
max_hp = 120
damage = 0
attack_range = 0
aggro_range = 0
chase_range = 0
move_cooldown_ms = 700
attack_cooldown_ms = 0
respawn_time_ms = 0

[witch_hazel.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[witch_hazel.behaviors]
hostile = false
merchant = true
wander_enabled = true
wander_radius = 2
wander_pause_min_ms = 6000
wander_pause_max_ms = 12000

[witch_hazel.merchant]
shop_id = "witch"
buy_multiplier = 0.4
sell_multiplier = 1.2
restock_interval_minutes = 5

[witch_hazel.speech]
radius = 6
interval_min_ms = 20000
interval_max_ms = 40000
messages = [
    "Bubble, bubble... need a potion, dearie?",
    "Herbs and vials, everything an alchemist needs...",
    "My potions are the finest in the land.",
    "Bring me rare herbs and I might teach you a thing or two.",
    "The swamp provides all the ingredients I need.",
    "Don't touch that cauldron! It bites.",
]

[witch_hazel.dialogue]
greeting = "Welcome to my little shop, dearie. Looking for potions or supplies?"
shop_open = "Take a peek at what I've got brewing."
```

**Step 3: Verify server loads**

Run: `cd rust-server && cargo run` (briefly)
Expected: Shop and NPC prototype load without errors

**Note:** The witch NPC will need to be placed in the world via the map editor. This task only creates the prototype and shop data. Place her at a thematic location (swamp or forest edge).

**Step 4: Commit**

```bash
git add rust-server/data/shops/witch.toml rust-server/data/entities/npcs/villagers.toml
git commit -m "feat: add Witch Hazel NPC merchant with alchemy shop"
```

---

### Task 9: Implement Temporary Buff System

This is the most complex task — adds `active_buffs` to Player, tick processing, combat integration, and wires up the existing `UseEffect::Buff`.

**Files:**
- Modify: `rust-server/src/game.rs`

**Step 1: Add ActiveBuff struct and player field**

Near the Player struct definition (around line 206), add:

```rust
#[derive(Debug, Clone)]
pub struct ActiveBuff {
    pub stat: String,      // "attack", "strength", "defence"
    pub amount: i32,
    pub expires_at: u64,   // game tick when buff expires
}
```

Add to Player struct:
```rust
pub active_buffs: Vec<ActiveBuff>,
```

Initialize in Player::new (or wherever Player is constructed) as:
```rust
active_buffs: Vec::new(),
```

**Step 2: Add buff helper methods to Player**

Add methods to Player impl:

```rust
/// Get total buff bonus for a stat
pub fn buff_bonus(&self, stat: &str) -> i32 {
    self.active_buffs
        .iter()
        .filter(|b| b.stat == stat)
        .map(|b| b.amount)
        .sum()
}

/// Apply a buff, replacing any existing buff for the same stat
pub fn apply_buff(&mut self, stat: String, amount: i32, duration_ticks: u64, current_tick: u64) {
    self.active_buffs.retain(|b| b.stat != stat);
    self.active_buffs.push(ActiveBuff {
        stat,
        amount,
        expires_at: current_tick + duration_ticks,
    });
}

/// Remove expired buffs, returns true if any were removed
pub fn tick_buffs(&mut self, current_tick: u64) -> bool {
    let before = self.active_buffs.len();
    self.active_buffs.retain(|b| b.expires_at > current_tick);
    self.active_buffs.len() != before
}
```

**Step 3: Integrate buffs into combat calculations**

In `Player::attack_bonus()` (line 349-362), add buff bonus at the end:
```rust
pub fn attack_bonus(&self, item_registry: &ItemRegistry) -> i32 {
    let mut bonus = 0;
    for equipped in self.all_equipped() {
        if let Some(item_id) = equipped {
            if let Some(def) = item_registry.get(item_id) {
                if let Some(equip) = &def.equipment {
                    bonus += equip.attack_bonus;
                }
            }
        }
    }
    bonus + self.buff_bonus("attack")
}
```

Do the same for `strength_bonus()` and `defence_bonus()`:
```rust
bonus + self.buff_bonus("strength")
// and
bonus + self.buff_bonus("defence")
```

**Step 4: Wire up UseEffect::Buff in handle_use_item**

In `handle_use_item` (line 3903-3906), replace the placeholder:

```rust
Some(UseEffect::Buff { stat, amount, duration_ms }) => {
    // Convert duration from ms to ticks (20Hz = 50ms per tick)
    let duration_ticks = duration_ms / 50;
    let current_tick = self.tick_counter.load(std::sync::atomic::Ordering::Relaxed);
    player.apply_buff(stat.clone(), *amount, duration_ticks, current_tick);
    format!("buff:{}:{}:{}", stat, amount, duration_ms)
}
```

Note: Check how `tick_counter` is accessed in the GameRoom — it may be a field or computed from elapsed time. Adapt the current tick source to match existing patterns (search for `tick_counter` or similar in game.rs).

**Step 5: Add buff expiry to tick loop**

In the main tick loop (search for where player HP regen happens), add buff tick processing:

```rust
// Tick buffs for all players
for player in players.values_mut() {
    player.tick_buffs(current_tick);
}
```

**Step 6: Add active_buffs to save/load**

Make sure `active_buffs` is initialized as empty `Vec::new()` when loading from database (buffs don't persist across sessions — they expire on logout). This is the simplest approach.

**Step 7: Verify compilation and tests**

Run: `cd rust-server && cargo check 2>&1 | grep error`
Run: `cd rust-server && cargo test`
Expected: No errors, all tests pass

**Step 8: Commit**

```bash
git add rust-server/src/game.rs
git commit -m "feat: implement temporary buff system with combat integration"
```

---

### Task 10: Implement Mana Restore Effect

The `RestoreMana` UseEffect exists but the handler just formats a string without actually restoring MP. The Player struct already has `mp` field.

**Files:**
- Modify: `rust-server/src/game.rs`

**Step 1: Fix RestoreMana handler**

In `handle_use_item` (line 3890-3892), replace:

```rust
Some(UseEffect::RestoreMana { amount }) => {
    let max_mp = player.max_mana_points();
    let old_mp = player.mp;
    player.mp = (player.mp + amount).min(max_mp);
    let actual_restored = player.mp - old_mp;
    format!("mana:{}", actual_restored)
}
```

Check that `max_mana_points()` method exists on Player. If not, add it based on the pattern `10 + skills.magic.level * 2` (from the design doc memory).

**Step 2: Verify compilation**

Run: `cd rust-server && cargo check 2>&1 | grep error`
Expected: No errors

**Step 3: Commit**

```bash
git add rust-server/src/game.rs
git commit -m "feat: implement mana restore effect for mana potions"
```

---

### Task 11: Remove Old Alchemist Shop (Cleanup)

The existing `alchemist.toml` shop sells `health_potion`, `mana_potion`, and `antidote` without proper item definitions for all of them. Now that the Witch handles potions, decide whether to keep both or replace.

**Recommended:** Keep the Alchemist as-is for now (it sells mid-tier health/prayer potions for convenience) and let the Witch be the dedicated alchemy supplier. No code changes needed — just verify both shops load correctly.

**Step 1: Verify both shops coexist**

Run: `cd rust-server && cargo run` (briefly)
Expected: Both `alchemist` and `witch` shops load

**Step 2: Commit (if any changes needed)**

No commit needed if both work as-is.

---

### Task 12: Final Integration Test

**Step 1: Full compilation check**

Run: `cd rust-server && cargo build 2>&1 | grep error`
Expected: Clean build

**Step 2: Run all tests**

Run: `cd rust-server && cargo test`
Expected: All tests pass

**Step 3: Manual smoke test**

Start the server and verify:
1. Alchemy skill appears in skills
2. Herb items exist in item registry
3. Alchemy recipes appear in crafting UI
4. Witch NPC loads with shop
5. Herb seeds plantable in farm patches
6. Buff potions apply temporary combat bonuses

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: complete potion system with alchemy skill, herbs, recipes, witch NPC, and buff system"
```
