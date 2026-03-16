# Desert Wurm Boss Design

## Overview

A giant desert wurm boss encounter in a group instance arena, accessible via a portal in the desert overworld. Recommended combat level 50-70, fight duration ~3-5 minutes.

**Entry:** Desert overworld portal leads to a sand arena (public group instance). Multiple players can enter the same instance.

**Core Mechanics:**
- Melee attacks when on the surface and players are adjacent
- Dig underground and re-emerge at a new position
- Rock throw AOE on emergence - telegraphed danger zones appear ~1.5s before rocks land
- Explosive minion spawns - slower-than-player chasers that explode in a 3x3 area on contact or when one-hit killed. Can damage the boss if it's in the blast radius

**Loot (full RNG per kill):**
- Cactus seeds (new item)
- Ancient fragments
- Wurm blade (new rare weapon)

---

## Phase Design

### Phase 1: "The Hunt" (100% - 66% HP)
- Wurm roams the surface, chasing and meleeing the nearest player
- Occasionally digs underground (~every 15s), repositions, and emerges with a small rock throw (3-4 rocks, telegraphed zones)
- Spawns 1 explosive minion every ~20s
- Teaches players the core mechanics at a manageable pace

### Phase 2: "The Storm" (66% - 33% HP)
- Digs more frequently (~every 10s)
- Rock throw on emergence increases to 5-7 rocks
- Spawns 2 explosive minions every ~15s
- Melee attacks deal more damage
- More dodging, more minion management

### Phase 3: "The Frenzy" (33% - 0% HP)
- Digs very frequently (~every 7s)
- Rock throw hits 8-10 tiles across the arena
- Spawns 3 explosive minions every ~10s
- Fastest melee attack speed
- Using minion explosions on the boss becomes a key strategy to finish it off

---

## Server Architecture

### New NPC States

Extend `NpcState` enum with boss-specific states:
- `Digging` - Wurm is burrowing underground (invulnerable, untargetable)
- `Emerging` - Wurm is surfacing at a new position (triggers rock throw AOE)
- `SpawningMinions` - Brief state when spawning explosive chasers

### Boss Tick Logic

New `boss_tick.rs` file (similar to `koth_tick.rs`) that runs per-instance when a boss is active:
- Track current phase based on HP thresholds (66%, 33%)
- Manage dig/emerge cycle timers per phase
- Handle rock throw zone selection and delayed damage
- Spawn minion NPCs into the instance
- Detect minion explosion events (death or player contact) and apply 3x3 AOE damage to all entities in range

### Explosive Minions

Regular instance NPCs with special prototype flags:
- `is_explosive: true` - explodes on death or player contact
- `explosion_radius: 1` (1 tile in each direction = 3x3)
- `explosion_damage` - flat damage to players AND NPCs (including the boss)
- HP of 1 so any hit pops them
- Uses existing chase AI with slower `move_cooldown_ms`

### Rock Throw AOE

Server-driven telegraphing:
1. Server picks random tiles around emerge point
2. Sends `AoeWarning { tiles, delay_ms }` message to instance players
3. After delay, sends `AoeDamage { tiles, damage }` and damages anyone still on those tiles
4. Client renders warning indicators on marked tiles during the delay window

---

## Protocol - New Messages

### Server -> Client

```
AoeWarning {
    tiles: Vec<(i32, i32)>,
    delay_ms: u64,
    effect: String,        // "rock_throw", etc.
}

AoeDamage {
    tiles: Vec<(i32, i32)>,
    damage: i32,
}

BossState {
    boss_id: String,
    hp: i32,
    max_hp: i32,
    phase: u8,
    state: String,         // "surface", "digging", "emerging"
}

Explosion {
    x: i32,
    y: i32,
    radius: i32,
    damage: i32,
}
```

---

## Client Rendering

- **Boss HP bar** - Large bar at top of screen showing name, HP, and current phase
- **AOE warning tiles** - Pulsing red/orange overlay on threatened tiles during delay window
- **Rock impact** - Dust/debris sprite on landing tiles
- **Minion explosion** - 3x3 blast effect centered on the minion's tile
- **Dig/emerge animations** - Use existing spritesheet animations

---

## New Items

- **Wurm Blade** - New melee weapon, stats TBD (level 50-60 tier)
- **Cactus Seeds** - New farming seed item, plantable in farming patches

---

## Implementation Order

1. Arena interior map + portal entry from desert overworld
2. Wurm boss NPC prototype (stats, sprite, animations)
3. New NPC states (Digging, Emerging, SpawningMinions)
4. Boss tick logic - phase tracking, dig/emerge cycle, melee
5. AOE warning/damage system (rock throw)
6. Explosive minion prototype + explosion mechanic (3x3 AOE)
7. Protocol messages (AoeWarning, AoeDamage, BossState, Explosion)
8. Client: boss HP bar UI
9. Client: AOE warning tile rendering
10. Client: explosion and rock impact effects
11. New items: wurm blade, cactus seeds
12. Loot table and drop RNG
13. Balancing and playtesting
