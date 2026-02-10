# Combat Balance Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rebalance combat formulas and data to create progressive difficulty where endgame players dominate low content while fresh players can challenge higher-level mobs with risk.

**Architecture:** Update combat formulas in `skills.rs`, add attack/defence bonuses to mob prototypes, update NPC combat to use bonuses, rebalance all gear stats.

**Tech Stack:** Rust server, TOML data files

---

## Task 1: Update Combat Formulas

**Files:**
- Modify: `rust-server/src/skills.rs:273-317`

**Step 1: Update calculate_hit formula**

Change base constant from 64 to 20:

```rust
/// Calculate whether an attack hits using attack roll vs defence roll.
/// Returns true if the attack hits.
///
/// Formula: Roll attacker's combat (0 to combat_level * (attack_bonus + 20))
///          Roll defender's combat (0 to combat_level * (defence_bonus + 20))
///          Hit if attack_roll > defence_roll
pub fn calculate_hit(
    attacker_combat_level: i32,
    attack_bonus: i32,
    defender_combat_level: i32,
    defence_bonus: i32,
) -> bool {
    let mut rng = rand::thread_rng();

    let attack_max = attacker_combat_level * (attack_bonus + 20);
    let defence_max = defender_combat_level * (defence_bonus + 20);

    let attack_roll = rng.gen_range(0..=attack_max.max(1));
    let defence_roll = rng.gen_range(0..=defence_max.max(1));

    attack_roll > defence_roll
}
```

**Step 2: Update calculate_max_hit formula**

Change to new formula with minimum 1 damage:

```rust
/// Calculate maximum hit based on combat level and equipment bonus.
///
/// Formula: 1 + (combat_level / 8) + (strength_bonus / 4)
/// This gives roughly:
/// - Level 1, no bonus: 1
/// - Level 30, +10 bonus: 6
/// - Level 70, +25 bonus: 15
pub fn calculate_max_hit(combat_level: i32, strength_bonus: i32) -> i32 {
    let base = 1.0 + (combat_level as f64 / 8.0) + (strength_bonus as f64 / 4.0);
    (base.floor() as i32).max(1)
}
```

**Step 3: Update roll_damage to have minimum 1**

```rust
/// Roll damage between 1 and max_hit (inclusive).
/// Minimum damage on a hit is 1.
pub fn roll_damage(max_hit: i32) -> i32 {
    if max_hit <= 1 {
        return 1;
    }
    rand::thread_rng().gen_range(1..=max_hit)
}
```

**Step 4: Build and verify**

Run: `cd rust-server && cargo build`
Expected: Compiles with no errors

**Step 5: Commit**

```bash
git add rust-server/src/skills.rs
git commit -m "feat(combat): rebalance hit and damage formulas

- Reduce base constant from 64 to 20 (gear matters more)
- New max_hit: 1 + (level/8) + (strength_bonus/4)
- Minimum 1 damage on successful hit"
```

---

## Task 2: Add Mob Attack/Defence Bonus Fields

**Files:**
- Modify: `rust-server/src/entity/prototype.rs:39-51` (RawEntityStats)
- Modify: `rust-server/src/entity/prototype.rs:200-212` (ResolvedStats)
- Modify: `rust-server/src/entity/prototype.rs:214-227` (Default impl)

**Step 1: Add fields to RawEntityStats**

```rust
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RawEntityStats {
    pub level: Option<i32>,
    pub max_hp: Option<i32>,
    pub damage: Option<i32>,
    pub attack_bonus: Option<i32>,   // NEW
    pub defence_bonus: Option<i32>,  // NEW
    pub attack_range: Option<i32>,
    pub aggro_range: Option<i32>,
    pub chase_range: Option<i32>,
    pub move_cooldown_ms: Option<u64>,
    pub attack_cooldown_ms: Option<u64>,
    pub respawn_time_ms: Option<u64>,
    pub hp_regen_percent_per_sec: Option<f32>,
}
```

**Step 2: Add fields to ResolvedStats**

```rust
#[derive(Debug, Clone)]
pub struct ResolvedStats {
    pub level: i32,
    pub max_hp: i32,
    pub damage: i32,
    pub attack_bonus: i32,   // NEW
    pub defence_bonus: i32,  // NEW
    pub attack_range: i32,
    pub aggro_range: i32,
    pub chase_range: i32,
    pub move_cooldown_ms: u64,
    pub attack_cooldown_ms: u64,
    pub respawn_time_ms: u64,
    pub hp_regen_percent_per_sec: f32,
}
```

**Step 3: Update Default impl**

```rust
impl Default for ResolvedStats {
    fn default() -> Self {
        Self {
            level: 1,
            max_hp: 100,
            damage: 10,
            attack_bonus: 0,   // NEW
            defence_bonus: 0,  // NEW
            attack_range: 1,
            aggro_range: 5,
            chase_range: 8,
            move_cooldown_ms: 500,
            attack_cooldown_ms: 800,
            respawn_time_ms: 10000,
            hp_regen_percent_per_sec: 2.0,
        }
    }
}
```

**Step 4: Build (will fail - registry needs update)**

Run: `cd rust-server && cargo build`
Expected: Error about missing fields in registry.rs

**Step 5: Commit partial progress**

```bash
git add rust-server/src/entity/prototype.rs
git commit -m "feat(mobs): add attack_bonus and defence_bonus to prototype stats"
```

---

## Task 3: Update Entity Registry Resolution

**Files:**
- Modify: `rust-server/src/entity/registry.rs:162-194`

**Step 1: Add bonus resolution in resolve_prototype**

Find the stats resolution block and add the new fields:

```rust
        // Merge stats with parent (child overrides parent)
        let stats = ResolvedStats {
            level: raw.stats.level
                .or_else(|| parent.map(|p| p.stats.level))
                .unwrap_or(1),
            max_hp: raw.stats.max_hp
                .or_else(|| parent.map(|p| p.stats.max_hp))
                .unwrap_or(100),
            damage: raw.stats.damage
                .or_else(|| parent.map(|p| p.stats.damage))
                .unwrap_or(10),
            attack_bonus: raw.stats.attack_bonus
                .or_else(|| parent.map(|p| p.stats.attack_bonus))
                .unwrap_or(0),
            defence_bonus: raw.stats.defence_bonus
                .or_else(|| parent.map(|p| p.stats.defence_bonus))
                .unwrap_or(0),
            attack_range: raw.stats.attack_range
                .or_else(|| parent.map(|p| p.stats.attack_range))
                .unwrap_or(1),
            aggro_range: raw.stats.aggro_range
                .or_else(|| parent.map(|p| p.stats.aggro_range))
                .unwrap_or(5),
            chase_range: raw.stats.chase_range
                .or_else(|| parent.map(|p| p.stats.chase_range))
                .unwrap_or(8),
            move_cooldown_ms: raw.stats.move_cooldown_ms
                .or_else(|| parent.map(|p| p.stats.move_cooldown_ms))
                .unwrap_or(500),
            attack_cooldown_ms: raw.stats.attack_cooldown_ms
                .or_else(|| parent.map(|p| p.stats.attack_cooldown_ms))
                .unwrap_or(800),
            respawn_time_ms: raw.stats.respawn_time_ms
                .or_else(|| parent.map(|p| p.stats.respawn_time_ms))
                .unwrap_or(10000),
            hp_regen_percent_per_sec: raw.stats.hp_regen_percent_per_sec
                .or_else(|| parent.map(|p| p.stats.hp_regen_percent_per_sec))
                .unwrap_or(2.0),
        };
```

**Step 2: Build (will fail - npc.rs needs update)**

Run: `cd rust-server && cargo build`
Expected: Error in npc.rs about PrototypeStats

**Step 3: Commit**

```bash
git add rust-server/src/entity/registry.rs
git commit -m "feat(mobs): resolve attack/defence bonuses with inheritance"
```

---

## Task 4: Update NPC PrototypeStats

**Files:**
- Modify: `rust-server/src/npc.rs:40-62` (PrototypeStats struct)
- Modify: `rust-server/src/npc.rs:115-136` (from_prototype stats initialization)

**Step 1: Add fields to PrototypeStats**

```rust
/// Stats from prototype used for AI behavior
#[derive(Debug, Clone)]
pub struct PrototypeStats {
    pub display_name: String,
    pub sprite: String,
    pub damage: i32,
    pub attack_bonus: i32,   // NEW
    pub defence_bonus: i32,  // NEW
    pub attack_range: i32,
    pub aggro_range: i32,
    pub chase_range: i32,
    pub move_cooldown_ms: u64,
    pub attack_cooldown_ms: u64,
    pub respawn_time_ms: u64,
    pub exp_base: i32,
    pub hostile: bool,
    pub is_quest_giver: bool,
    pub is_merchant: bool,
    pub is_altar: bool,
    pub wander_enabled: bool,
    pub wander_radius: i32,
    pub wander_pause_min_ms: u64,
    pub wander_pause_max_ms: u64,
    pub hp_regen_percent_per_sec: f32,
}
```

**Step 2: Update from_prototype to copy bonuses**

Find the stats initialization in from_prototype and add:

```rust
        let stats = PrototypeStats {
            display_name: prototype.display_name.clone(),
            sprite: prototype.sprite.clone(),
            damage: scale_damage(prototype.stats.damage, level),
            attack_bonus: prototype.stats.attack_bonus,   // NEW
            defence_bonus: prototype.stats.defence_bonus, // NEW
            attack_range: prototype.stats.attack_range,
            // ... rest unchanged
        };
```

**Step 3: Build and verify**

Run: `cd rust-server && cargo build`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add rust-server/src/npc.rs
git commit -m "feat(mobs): copy attack/defence bonuses to NPC instance"
```

---

## Task 5: Update NPC Combat to Use Bonuses

**Files:**
- Modify: `rust-server/src/game.rs:6529-6531` (NPC attacking player)
- Modify: `rust-server/src/game.rs:2326-2328` (Player attacking NPC)
- Modify: `rust-server/src/game.rs:8136-8137` (Spell attacking NPC)

**Step 1: Update NPC attack calculation (line ~6529)**

Find the NPC attack section and update:

```rust
                    // NPC uses its level and bonuses for attack
                    let npc_attack_level = npc_level;
                    let npc_attack_bonus = npc.stats.attack_bonus;  // CHANGED from 0
```

**Step 2: Update player attacking NPC (line ~2326)**

Find the player attack section and update:

```rust
                // NPC's defence = level + bonus
                let npc_defence_level = npc.level;
                let npc_defence_bonus = npc.stats.defence_bonus;  // CHANGED from 0
```

**Step 3: Update spell attacking NPC (line ~8136)**

Find the spell attack section and update:

```rust
                let npc_defence_level = npc.level;
                let npc_defence_bonus = npc.stats.defence_bonus;  // CHANGED from 0
```

**Step 4: Build and verify**

Run: `cd rust-server && cargo build`
Expected: Compiles successfully

**Step 5: Commit**

```bash
git add rust-server/src/game.rs
git commit -m "feat(combat): use mob attack/defence bonuses in combat calculations"
```

---

## Task 6: Update Monster TOML Files

**Files:**
- Modify: `rust-server/data/entities/monsters/forest_creatures.toml`
- Modify: `rust-server/data/entities/monsters/dangerous_creatures.toml`
- Modify: `rust-server/data/entities/monsters/pig.toml`
- Modify: `rust-server/data/entities/monsters/corrupted_creatures.toml`

**Step 1: Update forest_creatures.toml**

Add attack_bonus and defence_bonus to each creature. Examples:

```toml
[slime.stats]
level = 1
max_hp = 12
damage = 2
attack_bonus = 0
defence_bonus = 0
# ... rest unchanged

[snail.stats]
level = 1
max_hp = 20
damage = 1
attack_bonus = -2
defence_bonus = 5
# Tank archetype - hard to hit, low damage

[hedgehog.stats]
level = 2
max_hp = 25
damage = 4
attack_bonus = 3
defence_bonus = 2
# Balanced archetype

[springs.stats]
level = 2
max_hp = 18
damage = 4
attack_bonus = 5
defence_bonus = -2
# Glass cannon archetype
```

**Step 2: Update dangerous_creatures.toml**

```toml
[spider.stats]
level = 3
max_hp = 35
damage = 6
attack_bonus = 8
defence_bonus = 0
# Glass cannon - hits hard, easy to hit

[crow.stats]
level = 1
max_hp = 5
damage = 2
attack_bonus = 2
defence_bonus = -3
# Swarm type - weak but hits decently

[reaper.stats]
level = 5
max_hp = 150
damage = 15
attack_bonus = 12
defence_bonus = 10
# Boss archetype - dangerous all around
```

**Step 3: Update pig.toml**

```toml
[pig.stats]
level = 1
max_hp = 15
damage = 2
attack_bonus = 0
defence_bonus = 0
# Basic starter mob

[pig_king.stats]
max_hp = 500
damage = 25
attack_bonus = 15
defence_bonus = 20
# Boss - very dangerous
```

**Step 4: Update corrupted_creatures.toml**

Add appropriate bonuses based on creature type.

**Step 5: Commit**

```bash
git add rust-server/data/entities/monsters/
git commit -m "feat(mobs): add attack/defence bonuses to all monsters

- Starter mobs: +0/+0
- Glass cannons (spider): high attack, low defence
- Tanks (snail): low attack, high defence
- Bosses: high both"
```

---

## Task 7: Rebalance Equipment Stats

**Files:**
- Modify: `rust-server/data/items/equipment.toml`

**Step 1: Plan tier bonuses**

Reference from design doc:
| Tier | Level Req | Attack | Strength | Defence |
|------|-----------|--------|----------|---------|
| Starter | 1 | +1-4 | +1-3 | +1-5 |
| Mid | 20 | +6-12 | +4-8 | +6-12 |
| Late | 40 | +14-20 | +10-15 | +14-22 |
| Endgame | 60 | +22-30 | +16-22 | +24-35 |

**Step 2: Update starter tier (level 1-10)**

Example adjustments:

```toml
[training_boots]
# ... unchanged metadata
[training_boots.equipment]
slot_type = "feet"
defence_level_required = 1
attack_bonus = 1
strength_bonus = 1
defence_bonus = 1

[salvaged_tunic]
[salvaged_tunic.equipment]
slot_type = "body"
defence_level_required = 1
attack_bonus = 0
strength_bonus = 1
defence_bonus = 3
```

**Step 3: Update mid tier (level 20)**

Reduce current high bonuses to fit new scale.

**Step 4: Update late tier (level 40)**

Adjust bonuses to +14-22 range.

**Step 5: Update endgame tier (level 60)**

Cap at +30 attack, +22 strength, +35 defence max.

**Step 6: Remove overpowered items or adjust**

Items like `admin_robes` with +100 bonuses should be:
- Removed entirely, OR
- Marked as debug/GM only, OR
- Reduced to reasonable endgame values

**Step 7: Commit**

```bash
git add rust-server/data/items/equipment.toml
git commit -m "feat(gear): rebalance all equipment to 4-tier system

- Starter (1): +1-5 bonuses
- Mid (20): +6-12 bonuses
- Late (40): +14-22 bonuses
- Endgame (60): +22-35 bonuses"
```

---

## Task 8: Final Build and Test

**Step 1: Full rebuild**

Run: `cd rust-server && cargo build --release`
Expected: Compiles with no errors

**Step 2: Run server**

Run: `cd rust-server && cargo run`
Expected: Server starts, loads all entities and items

**Step 3: Manual testing checklist**

- [ ] Fresh player (Combat 3) can kill level 1 slimes
- [ ] Fresh player vs level 10 mob is dangerous but survivable with food
- [ ] Mid player (Combat 30) kills slimes in 2-3 hits
- [ ] Endgame player (Combat 70) one-shots slimes
- [ ] Mobs with high defence_bonus are harder to hit
- [ ] Mobs with high attack_bonus hit more often

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat(combat): complete combat balance overhaul

Summary:
- Hit formula base: 64 → 20
- Max hit formula: 1 + (lvl/8) + (str_bonus/4)
- Minimum 1 damage on hit
- Mobs have attack/defence bonuses
- Gear rebalanced to 4 tiers (1/20/40/60)"
```

---

## Summary of Changes

| File | Change |
|------|--------|
| `skills.rs` | New hit formula (base 20), new max_hit, min 1 damage |
| `entity/prototype.rs` | Add attack_bonus, defence_bonus fields |
| `entity/registry.rs` | Resolve new bonus fields with inheritance |
| `npc.rs` | Add bonuses to PrototypeStats, copy in from_prototype |
| `game.rs` | Use mob bonuses in 3 combat locations |
| `monsters/*.toml` | Add attack_bonus, defence_bonus to all mobs |
| `equipment.toml` | Rebalance all gear to 4-tier system |
