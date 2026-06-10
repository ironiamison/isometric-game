# Magic Rework Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rework magic to add cast stall, armor accuracy penalties, and replace generic blast with 4 elemental blast spells (air, water, earth, fire).

**Architecture:** Three independent changes: (1) Add a `cast_stall_ticks` counter to Player that blocks movement for 5 ticks after damage spells, (2) Add negative `magic_bonus` values to metal armor in equipment.toml, (3) Replace "blast" spell with air/water/earth/fire blast spells scaling in power.

**Tech Stack:** Rust server (game.rs, spell.rs, movement_tick.rs), TOML data (equipment.toml), Rust client (spell.rs, message_handler.rs, renderer.rs)

---

### Task 1: Cast Stall — Add `cast_stall_ticks` to Player

**Files:**
- Modify: `rust-server/src/game.rs` (Player struct ~line 340-570)

**Step 1: Add field to Player struct**

Add after `last_move_tick` (line 377):
```rust
    pub cast_stall_ticks: u64, // Ticks remaining where movement is blocked after casting
```

**Step 2: Initialize in Player::new()**

Add after `last_move_tick: 0,` (line 566):
```rust
            cast_stall_ticks: 0,
```

**Step 3: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 4: Commit**
```
feat: add cast_stall_ticks field to Player struct
```

---

### Task 2: Cast Stall — Block movement during stall

**Files:**
- Modify: `rust-server/src/game/movement_tick.rs` (line 46-54)

**Step 1: Add stall check to movement tick**

In `process_player_movement_tick`, right at the top of the player loop (line 48-55), add a cast stall decrement before the existing move intent check. The loop currently is:

```rust
        {
            let mut players = self.players.write().await;
            for player in players
                .values_mut()
                .filter(|player| player.active && !player.is_dead)
            {
                if (player.move_dx == 0 && player.move_dy == 0) || player.pending_move_seq.is_none()
                {
                    continue;
                }
```

Change to:

```rust
        {
            let mut players = self.players.write().await;
            for player in players
                .values_mut()
                .filter(|player| player.active && !player.is_dead)
            {
                // Decrement cast stall and reject movement while stalled
                if player.cast_stall_ticks > 0 {
                    player.cast_stall_ticks -= 1;
                    player.reject_pending_move();
                    continue;
                }

                if (player.move_dx == 0 && player.move_dy == 0) || player.pending_move_seq.is_none()
                {
                    continue;
                }
```

**Step 2: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 3: Commit**
```
feat: block movement during cast stall in movement tick
```

---

### Task 3: Cast Stall — Set stall on damage spell cast

**Files:**
- Modify: `rust-server/src/game.rs` (line 7242-7245)

**Step 1: Replace `reject_pending_move()` with cast stall**

Find the damage spell cast section (around line 7242-7245):
```rust
                player
                    .spell_cooldowns
                    .insert(spell_def.id.to_string(), current_time);
                // Stop movement when casting
                player.reject_pending_move();
```

Replace with:
```rust
                player
                    .spell_cooldowns
                    .insert(spell_def.id.to_string(), current_time);
                // Stall movement for 5 ticks when casting damage spells
                player.cast_stall_ticks = 5;
```

**Step 2: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 3: Commit**
```
feat: set 5-tick cast stall on damage spell cast
```

---

### Task 4: Armor Penalties — Add negative magic_bonus to equipment.toml

**Files:**
- Modify: `rust-server/data/items/equipment.toml`

**Step 1: Add `magic_bonus` to all metal armor pieces**

Add `magic_bonus = -N` line after `defence_bonus` for each piece. Both male and female body variants get the same value.

| Item ID | magic_bonus |
|---------|------------|
| `bronze_body` / `bronze_body_female` | -3 |
| `bronze_boots` | -1 |
| `bronze_gloves` | -1 |
| `iron_body` / `iron_body_female` | -4 |
| `iron_boots` | -2 |
| `iron_gloves` | -1 |
| `steel_body` / `steel_body_female` | -5 |
| `steel_boots` | -2 |
| `steel_gloves` | -2 |
| `mithril_body` / `mithril_body_female` | -6 |
| `mithril_boots` | -3 |
| `mithril_gloves` | -2 |
| `adamant_body` / `adamant_body_female` | -7 |
| `adamant_boots` | -3 |
| `adamant_gloves` | -3 |
| `rune_body` / `rune_body_female` | -8 |
| `rune_boots` | -4 |
| `rune_gloves` | -3 |

**Step 2: Verify server compiles and loads**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 3: Commit**
```
feat: add negative magic_bonus to metal armor (OSRS-style)
```

---

### Task 5: Elemental Blasts — Replace blast in server spell.rs

**Files:**
- Modify: `rust-server/src/spell.rs` (lines 35-44)

**Step 1: Replace the single blast spell with 4 elemental blasts**

Replace:
```rust
    SpellDef {
        id: "blast",
        name: "Blast",
        spell_type: SpellType::Damage,
        magic_level_req: 1,
        mana_cost: 2,
        cooldown_ms: 1000,
        base_power: 2,
        effect_sprite: "projectile",
    },
```

With:
```rust
    SpellDef {
        id: "air_blast",
        name: "Air Blast",
        spell_type: SpellType::Damage,
        magic_level_req: 1,
        mana_cost: 2,
        cooldown_ms: 1000,
        base_power: 2,
        effect_sprite: "projectile",
    },
    SpellDef {
        id: "water_blast",
        name: "Water Blast",
        spell_type: SpellType::Damage,
        magic_level_req: 5,
        mana_cost: 3,
        cooldown_ms: 1000,
        base_power: 3,
        effect_sprite: "projectile",
    },
    SpellDef {
        id: "earth_blast",
        name: "Earth Blast",
        spell_type: SpellType::Damage,
        magic_level_req: 10,
        mana_cost: 4,
        cooldown_ms: 1200,
        base_power: 4,
        effect_sprite: "projectile",
    },
    SpellDef {
        id: "fire_blast",
        name: "Fire Blast",
        spell_type: SpellType::Damage,
        magic_level_req: 15,
        mana_cost: 5,
        cooldown_ms: 1200,
        base_power: 5,
        effect_sprite: "projectile",
    },
```

**Step 2: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 3: Commit**
```
feat: replace blast with air/water/earth/fire blast spells
```

---

### Task 6: Elemental Blasts — Update server projectile check

**Files:**
- Modify: `rust-server/src/game.rs` (line 7419-7422)

**Step 1: Update projectile type in DamageEvent**

The current code hardcodes `"blast"` as the projectile name:
```rust
            projectile: if spell_def.effect_sprite == "projectile" {
                Some("blast".to_string())
            } else {
                None
            },
```

Change to use the spell's actual ID:
```rust
            projectile: if spell_def.effect_sprite == "projectile" {
                Some(spell_def.id.to_string())
            } else {
                None
            },
```

**Step 2: Verify it compiles**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 3: Commit**
```
feat: use spell id as projectile type for elemental blasts
```

---

### Task 7: Elemental Blasts — Update client spell definitions

**Files:**
- Modify: `client/src/game/spell.rs` (lines 39-100)

**Step 1: Add elemental blast spells to client SPELLS array**

The current array has 6 entries (no blast at all). Replace with 9 entries — insert the 4 elemental blasts after dark_hand (index 0), keeping the rest. Update the array size from `[SpellDef; 6]` to `[SpellDef; 9]`:

```rust
pub const SPELLS: [SpellDef; 9] = [
    SpellDef {
        id: "dark_hand",
        name: "Dark Hand",
        spell_type: SpellType::Damage,
        magic_level_req: 1,
        mana_cost: 3,
        cooldown_ms: 1500,
        description: "A shadowy hand strikes your target",
        effect_sprite: "dark_hand",
    },
    SpellDef {
        id: "air_blast",
        name: "Air Blast",
        spell_type: SpellType::Damage,
        magic_level_req: 1,
        mana_cost: 2,
        cooldown_ms: 1000,
        description: "A gust of wind strikes your target",
        effect_sprite: "projectile",
    },
    SpellDef {
        id: "water_blast",
        name: "Water Blast",
        spell_type: SpellType::Damage,
        magic_level_req: 5,
        mana_cost: 3,
        cooldown_ms: 1000,
        description: "A surge of water strikes your target",
        effect_sprite: "projectile",
    },
    SpellDef {
        id: "earth_blast",
        name: "Earth Blast",
        spell_type: SpellType::Damage,
        magic_level_req: 10,
        mana_cost: 4,
        cooldown_ms: 1200,
        description: "A chunk of earth strikes your target",
        effect_sprite: "projectile",
    },
    SpellDef {
        id: "fire_blast",
        name: "Fire Blast",
        spell_type: SpellType::Damage,
        magic_level_req: 15,
        mana_cost: 5,
        cooldown_ms: 1200,
        description: "A ball of fire strikes your target",
        effect_sprite: "projectile",
    },
    SpellDef {
        id: "lightning_bolt",
        name: "Lightning Bolt",
        spell_type: SpellType::Damage,
        magic_level_req: 7,
        mana_cost: 7,
        cooldown_ms: 2000,
        description: "A bolt of lightning strikes your target",
        effect_sprite: "lightning_bolt",
    },
    SpellDef {
        id: "dark_eater",
        name: "Dark Eater",
        spell_type: SpellType::Damage,
        magic_level_req: 15,
        mana_cost: 15,
        cooldown_ms: 3000,
        description: "A dark entity devours your target",
        effect_sprite: "dark_eater",
    },
    SpellDef {
        id: "rock_fall",
        name: "Rock Fall",
        spell_type: SpellType::Damage,
        magic_level_req: 25,
        mana_cost: 12,
        cooldown_ms: 2500,
        description: "Summon falling rocks to crush your target",
        effect_sprite: "rock_fall",
    },
    SpellDef {
        id: "heal",
        name: "Heal",
        spell_type: SpellType::Heal,
        magic_level_req: 5,
        mana_cost: 10,
        cooldown_ms: 5000,
        description: "Restore your health",
        effect_sprite: "self_heal",
    },
    SpellDef {
        id: "return_home",
        name: "Return Home",
        spell_type: SpellType::Teleport,
        magic_level_req: 0,
        mana_cost: 0,
        cooldown_ms: 900_000,
        description: "Teleport to the village spawn point",
        effect_sprite: "teleport",
    },
];
```

Note: This is actually 10 entries. Update array size to `[SpellDef; 10]`.

**Step 2: Verify it compiles**

Run: `cd client && cargo check 2>&1 | head -20`

**Step 3: Commit**
```
feat: add elemental blast spells to client spell definitions
```

---

### Task 8: Elemental Blasts — Update client projectile handling

**Files:**
- Modify: `client/src/network/message_handler.rs` (lines 1053, 3882-3896)
- Modify: `client/src/render/renderer.rs` (lines 5614, 5810, 5896, 1693)

**Step 1: Update DamageEvent projectile filter**

At line 1053, change:
```rust
                    if projectile_type != "blast" {
```
To:
```rust
                    if !projectile_type.ends_with("_blast") {
```

**Step 2: Update SpellEffect projectile spawning**

At line 3882-3893, change:
```rust
                if spell_id == "blast" {
```
To:
```rust
                if spell_id.ends_with("_blast") {
```

And at line 3893, change:
```rust
                            sprite: "blast".to_string(),
```
To:
```rust
                            sprite: spell_id.clone(),
```

**Step 3: Update renderer projectile rendering**

At line 5614, change:
```rust
            if projectile.sprite == "blast" {
```
To:
```rust
            if projectile.sprite.ends_with("_blast") {
```

**Step 4: Update renderer spell effect skip**

At line 5810, change:
```rust
                "blast" => continue,
```
To match all blast variants. Since this is a match arm, use a guard:
```rust
                s if s.ends_with("_blast") => continue,
```

At line 5896, change:
```rust
            "blast" => return,
```
To:
```rust
            s if s.ends_with("_blast") => return,
```

**Step 5: Update spell icon mappings**

At line 1693, change:
```rust
                ("blast", "blast"),
```
To:
```rust
                ("air_blast", "blast"),
                ("water_blast", "blast"),
                ("earth_blast", "blast"),
                ("fire_blast", "blast"),
```

(All 4 use the same "blast" icon texture for now — unique icons can be added later.)

**Step 6: Verify it compiles**

Run: `cd client && cargo check 2>&1 | head -20`

**Step 7: Commit**
```
feat: update client to handle elemental blast projectiles
```

---

### Task 9: Final Verification

**Step 1: Full build check**

Run: `cd rust-server && cargo check && cd ../client && cargo check`

**Step 2: Commit the plan update**
```
docs: update magic rework plan with implementation details
```
