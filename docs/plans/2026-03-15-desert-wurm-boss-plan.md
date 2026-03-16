# Desert Wurm Boss Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a three-phase desert wurm boss fight with dig/emerge mechanics, AOE rock throws, explosive minions, and a boss HP bar UI — all inside a public group instance.

**Architecture:** Follow the KOTH pattern exactly: a pure state machine (`boss.rs`) emits events, a tick handler (`boss_tick.rs`) processes them in `GameRoom`. New `ServerMessage` variants drive the client. The boss is an NPC in the instance with special states; minions are lightweight 1-HP NPCs with an `is_explosive` flag.

**Tech Stack:** Rust server (Axum/Tokio), Macroquad client, MessagePack protocol, TOML entity data, JSON interior maps.

---

### Task 1: Boss State Machine (`rust-server/src/boss.rs`)

**Files:**
- Create: `rust-server/src/boss.rs`
- Modify: `rust-server/src/main.rs:39` (add `mod boss;` near `mod koth;`)

**Step 1: Create the boss state machine module**

Model after `rust-server/src/koth.rs`. The boss has phases, timers, and emits events for GameRoom to process.

```rust
use std::collections::HashMap;
use rand::Rng;

// ---------------------------------------------------------------------------
// Boss Phase
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum BossPhase {
    /// Phase 1: 100%-66% HP - "The Hunt"
    Hunt,
    /// Phase 2: 66%-33% HP - "The Storm"
    Storm,
    /// Phase 3: 33%-0% HP - "The Frenzy"
    Frenzy,
}

// ---------------------------------------------------------------------------
// Wurm Surface State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum WurmState {
    /// On the surface, chasing/meleeing players
    Surface,
    /// Burrowing underground (invulnerable)
    Digging { ends_at: u64 },
    /// Emerging at new position, triggers rock throw
    Emerging { ends_at: u64, target_x: i32, target_y: i32 },
    /// Boss is dead
    Dead,
}

// ---------------------------------------------------------------------------
// Pending minion spawn
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PendingMinionSpawn {
    pub x: i32,
    pub y: i32,
}

// ---------------------------------------------------------------------------
// AOE warning zone (rocks about to land)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AoeZone {
    pub tiles: Vec<(i32, i32)>,
    pub damage: i32,
    pub lands_at: u64,
    pub sent_warning: bool,
}

// ---------------------------------------------------------------------------
// Boss State per-instance
// ---------------------------------------------------------------------------

pub struct BossState {
    pub instance_id: String,
    pub boss_npc_id: String,
    pub phase: BossPhase,
    pub wurm_state: WurmState,
    pub boss_hp: i32,
    pub boss_max_hp: i32,
    pub boss_x: i32,
    pub boss_y: i32,
    pub map_width: u32,
    pub map_height: u32,
    /// Tracks time since last dig cycle
    pub last_dig_time: u64,
    /// Tracks time since last minion spawn
    pub last_minion_spawn_time: u64,
    /// Minion counter for unique IDs
    pub minion_counter: u32,
    /// Active AOE zones waiting to land
    pub aoe_zones: Vec<AoeZone>,
    /// Players in this boss fight (for sending messages)
    pub player_ids: Vec<String>,
}

// ---------------------------------------------------------------------------
// Events emitted for GameRoom to process
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum BossEvent {
    /// Send boss state update to all players in instance
    StateUpdate {
        instance_id: String,
        boss_hp: i32,
        boss_max_hp: i32,
        phase: String,
        wurm_state: String,
    },
    /// Spawn an explosive minion NPC
    SpawnMinion {
        instance_id: String,
        npc_id: String,
        x: i32,
        y: i32,
    },
    /// Send AOE warning to clients (tiles about to be hit)
    AoeWarning {
        instance_id: String,
        tiles: Vec<(i32, i32)>,
        delay_ms: u64,
        effect: String,
    },
    /// Apply AOE damage to entities on tiles
    AoeDamage {
        instance_id: String,
        tiles: Vec<(i32, i32)>,
        damage: i32,
    },
    /// Minion exploded - apply 3x3 AOE damage
    Explosion {
        instance_id: String,
        x: i32,
        y: i32,
        radius: i32,
        damage: i32,
    },
    /// Boss died - trigger loot and cleanup
    BossDied {
        instance_id: String,
        killer_id: String,
    },
    /// Update boss NPC position (after emerge)
    MoveBoss {
        instance_id: String,
        npc_id: String,
        x: i32,
        y: i32,
    },
    /// Make boss invulnerable/vulnerable
    SetBossInvulnerable {
        instance_id: String,
        npc_id: String,
        invulnerable: bool,
    },
}

// ---------------------------------------------------------------------------
// Phase configuration
// ---------------------------------------------------------------------------

struct PhaseConfig {
    dig_interval_ms: u64,
    rock_count: u32,
    minion_count: u32,
    minion_interval_ms: u64,
    melee_damage_multiplier: f32,
}

fn phase_config(phase: &BossPhase) -> PhaseConfig {
    match phase {
        BossPhase::Hunt => PhaseConfig {
            dig_interval_ms: 15_000,
            rock_count: 4,
            minion_count: 1,
            minion_interval_ms: 20_000,
            melee_damage_multiplier: 1.0,
        },
        BossPhase::Storm => PhaseConfig {
            dig_interval_ms: 10_000,
            rock_count: 6,
            minion_count: 2,
            minion_interval_ms: 15_000,
            melee_damage_multiplier: 1.3,
        },
        BossPhase::Frenzy => PhaseConfig {
            dig_interval_ms: 7_000,
            rock_count: 9,
            minion_count: 3,
            minion_interval_ms: 10_000,
            melee_damage_multiplier: 1.6,
        },
    }
}

// ---------------------------------------------------------------------------
// Rock throw target generation
// ---------------------------------------------------------------------------

fn generate_rock_targets(
    boss_x: i32,
    boss_y: i32,
    count: u32,
    map_width: u32,
    map_height: u32,
) -> Vec<(i32, i32)> {
    let mut rng = rand::thread_rng();
    let mut tiles = Vec::new();
    for _ in 0..count {
        // Rocks land within 8 tiles of emerge point, biased toward center
        let dx = rng.gen_range(-8..=8);
        let dy = rng.gen_range(-8..=8);
        let tx = (boss_x + dx).clamp(1, map_width as i32 - 2);
        let ty = (boss_y + dy).clamp(1, map_height as i32 - 2);
        tiles.push((tx, ty));
    }
    tiles
}

/// Pick a random non-edge position for the wurm to emerge
fn pick_emerge_position(map_width: u32, map_height: u32) -> (i32, i32) {
    let mut rng = rand::thread_rng();
    let x = rng.gen_range(4..map_width as i32 - 4);
    let y = rng.gen_range(4..map_height as i32 - 4);
    (x, y)
}

/// Pick spawn positions for minions (edges of the arena)
fn pick_minion_spawn(map_width: u32, map_height: u32) -> (i32, i32) {
    let mut rng = rand::thread_rng();
    let edge = rng.gen_range(0..4);
    match edge {
        0 => (rng.gen_range(3..map_width as i32 - 3), 3),                    // top
        1 => (rng.gen_range(3..map_width as i32 - 3), map_height as i32 - 4), // bottom
        2 => (3, rng.gen_range(3..map_height as i32 - 3)),                    // left
        _ => (map_width as i32 - 4, rng.gen_range(3..map_height as i32 - 3)), // right
    }
}

// ---------------------------------------------------------------------------
// BossState implementation
// ---------------------------------------------------------------------------

/// Duration the wurm spends underground (ms)
const DIG_DURATION_MS: u64 = 3000;
/// Duration the emerge animation takes before rocks land
const EMERGE_DURATION_MS: u64 = 1500;
/// Rock throw AOE warning delay (time between warning and damage)
const AOE_WARNING_DELAY_MS: u64 = 1500;
/// Explosive minion damage
const MINION_EXPLOSION_DAMAGE: i32 = 15;

impl BossState {
    pub fn new(
        instance_id: String,
        boss_npc_id: String,
        boss_hp: i32,
        boss_x: i32,
        boss_y: i32,
        map_width: u32,
        map_height: u32,
        current_time: u64,
        player_ids: Vec<String>,
    ) -> Self {
        Self {
            instance_id,
            boss_npc_id,
            phase: BossPhase::Hunt,
            wurm_state: WurmState::Surface,
            boss_hp,
            boss_max_hp: boss_hp,
            boss_x,
            boss_y,
            map_width,
            map_height,
            last_dig_time: current_time,
            last_minion_spawn_time: current_time,
            minion_counter: 0,
            aoe_zones: Vec::new(),
            player_ids,
        }
    }

    /// Update phase based on HP thresholds
    fn update_phase(&mut self) {
        let hp_pct = self.boss_hp as f32 / self.boss_max_hp as f32;
        let new_phase = if hp_pct > 0.66 {
            BossPhase::Hunt
        } else if hp_pct > 0.33 {
            BossPhase::Storm
        } else {
            BossPhase::Frenzy
        };
        self.phase = new_phase;
    }

    /// Main tick - returns events for GameRoom to process
    pub fn tick(&mut self, current_time: u64) -> Vec<BossEvent> {
        let mut events = Vec::new();

        if self.boss_hp <= 0 {
            self.wurm_state = WurmState::Dead;
            return events;
        }

        self.update_phase();
        let config = phase_config(&self.phase);

        match self.wurm_state.clone() {
            WurmState::Surface => {
                // Check if it's time to dig
                if current_time - self.last_dig_time >= config.dig_interval_ms {
                    self.wurm_state = WurmState::Digging {
                        ends_at: current_time + DIG_DURATION_MS,
                    };
                    self.last_dig_time = current_time;

                    // Boss becomes invulnerable while underground
                    events.push(BossEvent::SetBossInvulnerable {
                        instance_id: self.instance_id.clone(),
                        npc_id: self.boss_npc_id.clone(),
                        invulnerable: true,
                    });
                }

                // Check if it's time to spawn minions
                if current_time - self.last_minion_spawn_time >= config.minion_interval_ms {
                    self.last_minion_spawn_time = current_time;
                    for _ in 0..config.minion_count {
                        let (sx, sy) = pick_minion_spawn(self.map_width, self.map_height);
                        self.minion_counter += 1;
                        let npc_id = format!(
                            "boss_minion_{}_{}",
                            self.instance_id, self.minion_counter
                        );
                        events.push(BossEvent::SpawnMinion {
                            instance_id: self.instance_id.clone(),
                            npc_id,
                            x: sx,
                            y: sy,
                        });
                    }
                }
            }

            WurmState::Digging { ends_at } => {
                if current_time >= ends_at {
                    // Pick emerge position and start emerging
                    let (ex, ey) = pick_emerge_position(self.map_width, self.map_height);
                    self.wurm_state = WurmState::Emerging {
                        ends_at: current_time + EMERGE_DURATION_MS,
                        target_x: ex,
                        target_y: ey,
                    };

                    // Generate rock throw AOE
                    let rock_tiles = generate_rock_targets(
                        ex,
                        ey,
                        config.rock_count,
                        self.map_width,
                        self.map_height,
                    );

                    // Queue AOE zone
                    self.aoe_zones.push(AoeZone {
                        tiles: rock_tiles.clone(),
                        damage: 10 + (self.phase != BossPhase::Hunt) as i32 * 5,
                        lands_at: current_time + AOE_WARNING_DELAY_MS,
                        sent_warning: false,
                    });
                }
            }

            WurmState::Emerging { ends_at, target_x, target_y } => {
                if current_time >= ends_at {
                    // Emerge complete - move boss to new position, become vulnerable
                    self.boss_x = target_x;
                    self.boss_y = target_y;
                    self.wurm_state = WurmState::Surface;

                    events.push(BossEvent::MoveBoss {
                        instance_id: self.instance_id.clone(),
                        npc_id: self.boss_npc_id.clone(),
                        x: target_x,
                        y: target_y,
                    });
                    events.push(BossEvent::SetBossInvulnerable {
                        instance_id: self.instance_id.clone(),
                        npc_id: self.boss_npc_id.clone(),
                        invulnerable: false,
                    });
                }
            }

            WurmState::Dead => {
                // Handled above
            }
        }

        // Process AOE zones
        let mut landed_indices = Vec::new();
        for (i, zone) in self.aoe_zones.iter_mut().enumerate() {
            // Send warning if not sent yet
            if !zone.sent_warning {
                zone.sent_warning = true;
                events.push(BossEvent::AoeWarning {
                    instance_id: self.instance_id.clone(),
                    tiles: zone.tiles.clone(),
                    delay_ms: AOE_WARNING_DELAY_MS,
                    effect: "rock_throw".to_string(),
                });
            }

            // Check if rocks have landed
            if current_time >= zone.lands_at {
                events.push(BossEvent::AoeDamage {
                    instance_id: self.instance_id.clone(),
                    tiles: zone.tiles.clone(),
                    damage: zone.damage,
                });
                landed_indices.push(i);
            }
        }

        // Remove landed zones (reverse order to preserve indices)
        for i in landed_indices.into_iter().rev() {
            self.aoe_zones.remove(i);
        }

        // Always send state update
        let phase_str = match self.phase {
            BossPhase::Hunt => "hunt",
            BossPhase::Storm => "storm",
            BossPhase::Frenzy => "frenzy",
        };
        let wurm_str = match &self.wurm_state {
            WurmState::Surface => "surface",
            WurmState::Digging { .. } => "digging",
            WurmState::Emerging { .. } => "emerging",
            WurmState::Dead => "dead",
        };
        events.push(BossEvent::StateUpdate {
            instance_id: self.instance_id.clone(),
            boss_hp: self.boss_hp,
            boss_max_hp: self.boss_max_hp,
            phase: phase_str.to_string(),
            wurm_state: wurm_str.to_string(),
        });

        events
    }

    /// Called when boss takes damage (from player attack or minion explosion)
    pub fn on_boss_damaged(&mut self, damage: i32) {
        self.boss_hp = (self.boss_hp - damage).max(0);
        self.update_phase();
    }

    /// Called when a minion explodes
    pub fn on_minion_exploded(&mut self, x: i32, y: i32) -> Vec<BossEvent> {
        vec![BossEvent::Explosion {
            instance_id: self.instance_id.clone(),
            x,
            y,
            radius: 1, // 3x3 area
            damage: MINION_EXPLOSION_DAMAGE,
        }]
    }

    /// Add a player to the fight
    pub fn add_player(&mut self, player_id: String) {
        if !self.player_ids.contains(&player_id) {
            self.player_ids.push(player_id);
        }
    }

    /// Remove a player from the fight
    pub fn remove_player(&mut self, player_id: &str) {
        self.player_ids.retain(|id| id != player_id);
    }

    /// Check if boss is dead
    pub fn is_dead(&self) -> bool {
        self.boss_hp <= 0
    }

    /// Check if boss is currently invulnerable (underground)
    pub fn is_invulnerable(&self) -> bool {
        matches!(self.wurm_state, WurmState::Digging { .. })
    }
}

/// Maps instance_id -> BossState for all active boss fights
pub type BossStates = HashMap<String, BossState>;
```

**Step 2: Register the module in main.rs**

Add `mod boss;` at `rust-server/src/main.rs:39` (after `mod koth;`):

```rust
mod koth;
mod boss;
```

**Step 3: Commit**

```bash
git add rust-server/src/boss.rs rust-server/src/main.rs
git commit -m "feat: add desert wurm boss state machine"
```

---

### Task 2: Boss Entity Data (TOML prototypes)

**Files:**
- Create: `rust-server/data/entities/monsters/desert_boss.toml`

**Step 1: Create boss and minion prototypes**

Follow the format in `rust-server/data/entities/monsters/koth_enemies.toml`:

```toml
# ============================================================================
# Desert Wurm Boss
# ============================================================================

[desert_wurm]
display_name = "Desert Wurm"
sprite = "desert_wurm"
animation_type = "standard"
description = "A massive wurm that burrows through the desert sands."

[desert_wurm.stats]
level = 60
max_hp = 500
damage = 12
attack_bonus = 20
defence_bonus = 15
attack_range = 1
aggro_range = 999
chase_range = 999
move_cooldown_ms = 600
attack_cooldown_ms = 1200
respawn_time_ms = 0

[desert_wurm.rewards]
exp_base = 500
gold_min = 200
gold_max = 500

[[desert_wurm.loot]]
item_id = "ancient_fragment"
drop_chance = 0.40
quantity_min = 1
quantity_max = 3

[[desert_wurm.loot]]
item_id = "cactus_seed"
drop_chance = 0.50
quantity_min = 2
quantity_max = 5

[[desert_wurm.loot]]
item_id = "wurm_blade"
drop_chance = 0.05
quantity_min = 1
quantity_max = 1

[desert_wurm.behaviors]
hostile = true
wander_enabled = false

# ============================================================================
# Explosive Minion (1 HP, chases player, explodes on contact/death)
# ============================================================================

[wurm_minion]
display_name = "Sand Scarab"
sprite = "sand_scarab"
animation_type = "standard"
description = "A volatile scarab that detonates on contact."

[wurm_minion.stats]
level = 30
max_hp = 1
damage = 0
attack_bonus = 0
defence_bonus = 0
attack_range = 1
aggro_range = 999
chase_range = 999
move_cooldown_ms = 350
attack_cooldown_ms = 9999999
respawn_time_ms = 0

[wurm_minion.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[wurm_minion.behaviors]
hostile = true
wander_enabled = false
```

**Step 2: Commit**

```bash
git add rust-server/data/entities/monsters/desert_boss.toml
git commit -m "feat: add desert wurm and sand scarab entity prototypes"
```

---

### Task 3: New Protocol Messages

**Files:**
- Modify: `rust-server/src/protocol.rs:868` (add new ServerMessage variants after KOTH messages)

**Step 1: Add boss-related ServerMessage variants**

Add after the KOTH messages (around line 888):

```rust
    // Boss fight messages
    BossStateUpdate {
        boss_id: String,
        hp: i32,
        max_hp: i32,
        phase: String,
        wurm_state: String,
    },
    AoeWarning {
        tiles: Vec<(i32, i32)>,
        delay_ms: u64,
        effect: String,
    },
    AoeDamage {
        tiles: Vec<(i32, i32)>,
        damage: i32,
    },
    Explosion {
        x: i32,
        y: i32,
        radius: i32,
        damage: i32,
    },
```

**Step 2: Verify the serde encoding works**

The existing `#[serde(untagged)]` on `ServerMessage` means each variant serializes as its own object. The `(i32, i32)` tuples will serialize as `[x, y]` arrays in MessagePack, which is fine for the client to parse.

**Step 3: Commit**

```bash
git add rust-server/src/protocol.rs
git commit -m "feat: add boss fight protocol messages (AoeWarning, AoeDamage, BossStateUpdate, Explosion)"
```

---

### Task 4: Boss Tick Handler (`rust-server/src/game/boss_tick.rs`)

**Files:**
- Create: `rust-server/src/game/boss_tick.rs`
- Modify: `rust-server/src/game.rs:31` (add `mod boss_tick;`)
- Modify: `rust-server/src/game.rs:1132` (add `boss_states` field to GameRoom)
- Modify: `rust-server/src/game.rs:1654` (initialize `boss_states` in GameRoom::new)
- Modify: `rust-server/src/game.rs:6269` (call `process_boss_tick` in tick loop)

**Step 1: Add boss_states field to GameRoom**

At `rust-server/src/game.rs:1132` (after `koth_states`), add:

```rust
    /// Active boss fight sessions: instance_id -> BossState
    boss_states: RwLock<crate::boss::BossStates>,
```

Initialize it in `GameRoom::new` (after `koth_states` init around line 1654):

```rust
    boss_states: RwLock::new(std::collections::HashMap::new()),
```

Add module declaration at line 31 (after `pub(crate) mod koth_tick;`):

```rust
mod boss_tick;
```

**Step 2: Create the boss tick handler**

Model after `rust-server/src/game/koth_tick.rs`:

```rust
use super::GameRoom;
use crate::boss::{BossEvent, BossState};
use crate::npc::Npc;
use crate::protocol::ServerMessage;

/// The desert wurm arena interior map ID
pub const DESERT_WURM_MAP_ID: &str = "desert_wurm_arena";

/// Explosive minion prototype ID
const MINION_PROTOTYPE_ID: &str = "wurm_minion";

impl GameRoom {
    /// Process all active boss fight sessions each tick
    pub(in crate::game) async fn process_boss_tick(&self, current_time: u64) {
        let mut boss_states = self.boss_states.write().await;
        let mut finished_instances: Vec<String> = Vec::new();
        let mut all_events: Vec<BossEvent> = Vec::new();

        for (instance_id, boss) in boss_states.iter_mut() {
            if boss.is_dead() {
                finished_instances.push(instance_id.clone());
                continue;
            }

            let events = boss.tick(current_time);
            all_events.extend(events);
        }

        // Remove finished fights
        for id in &finished_instances {
            boss_states.remove(id);
        }

        drop(boss_states);

        // Process events
        for event in all_events {
            self.handle_boss_event(event, current_time).await;
        }
    }

    /// Handle a single boss event
    async fn handle_boss_event(&self, event: BossEvent, _current_time: u64) {
        match event {
            BossEvent::StateUpdate {
                instance_id,
                boss_hp,
                boss_max_hp,
                phase,
                wurm_state,
            } => {
                self.send_to_instance(
                    &instance_id,
                    ServerMessage::BossStateUpdate {
                        boss_id: "desert_wurm".to_string(),
                        hp: boss_hp,
                        max_hp: boss_max_hp,
                        phase,
                        wurm_state,
                    },
                )
                .await;
            }

            BossEvent::SpawnMinion {
                instance_id,
                npc_id,
                x,
                y,
            } => {
                if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                    if let Some(prototype) = self.entity_registry.get(MINION_PROTOTYPE_ID) {
                        let npc = Npc::from_prototype(
                            &npc_id,
                            MINION_PROTOTYPE_ID,
                            prototype,
                            x,
                            y,
                            30, // minion level
                            None,
                        );
                        let mut npcs = instance.npcs.write().await;
                        npcs.insert(npc_id, npc);
                    }
                }
            }

            BossEvent::AoeWarning {
                instance_id,
                tiles,
                delay_ms,
                effect,
            } => {
                self.send_to_instance(
                    &instance_id,
                    ServerMessage::AoeWarning {
                        tiles,
                        delay_ms,
                        effect,
                    },
                )
                .await;
            }

            BossEvent::AoeDamage {
                instance_id,
                tiles,
                damage,
            } => {
                // Damage all players standing on AOE tiles
                let tile_set: std::collections::HashSet<(i32, i32)> =
                    tiles.iter().cloned().collect();

                let mut players = self.players.write().await;
                let instances = self.player_instances.read().await;

                for (player_id, player) in players.iter_mut() {
                    // Only affect players in this instance
                    if instances.get(player_id).map(|id| id.as_str()) != Some(&instance_id) {
                        continue;
                    }
                    let px = player.x.round() as i32;
                    let py = player.y.round() as i32;
                    if tile_set.contains(&(px, py)) {
                        player.hp = (player.hp - damage).max(0);
                        // Send damage event
                        // (player death is handled by existing tick loop checks)
                    }
                }

                // Also send visual effect to clients
                self.send_to_instance(
                    &instance_id,
                    ServerMessage::AoeDamage {
                        tiles,
                        damage,
                    },
                )
                .await;
            }

            BossEvent::Explosion {
                instance_id,
                x,
                y,
                radius,
                damage,
            } => {
                // Calculate 3x3 (radius=1) affected tiles
                let mut affected_tiles = Vec::new();
                for dx in -radius..=radius {
                    for dy in -radius..=radius {
                        affected_tiles.push((x + dx, y + dy));
                    }
                }
                let tile_set: std::collections::HashSet<(i32, i32)> =
                    affected_tiles.iter().cloned().collect();

                // Damage players in blast zone
                {
                    let mut players = self.players.write().await;
                    let instances = self.player_instances.read().await;
                    for (player_id, player) in players.iter_mut() {
                        if instances.get(player_id).map(|id| id.as_str()) != Some(&instance_id) {
                            continue;
                        }
                        let px = player.x.round() as i32;
                        let py = player.y.round() as i32;
                        if tile_set.contains(&(px, py)) {
                            player.hp = (player.hp - damage).max(0);
                        }
                    }
                }

                // Damage boss if in blast zone
                {
                    let mut boss_states = self.boss_states.write().await;
                    if let Some(boss) = boss_states.get_mut(&instance_id) {
                        if tile_set.contains(&(boss.boss_x, boss.boss_y)) {
                            boss.on_boss_damaged(damage);
                        }
                    }
                }

                // Damage other minion NPCs in blast zone (chain reaction!)
                if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                    let npcs = instance.npcs.read().await;
                    let mut minions_to_kill = Vec::new();
                    for (npc_id, npc) in npcs.iter() {
                        if npc_id.starts_with("boss_minion_") && npc.is_alive() {
                            let nx = npc.x.round() as i32;
                            let ny = npc.y.round() as i32;
                            if tile_set.contains(&(nx, ny)) {
                                minions_to_kill.push(npc_id.clone());
                            }
                        }
                    }
                    drop(npcs);

                    // Kill chain-reaction minions (they will trigger their own explosions)
                    for npc_id in minions_to_kill {
                        let mut npcs = instance.npcs.write().await;
                        if let Some(npc) = npcs.get_mut(&npc_id) {
                            npc.hp = 0;
                        }
                    }
                }

                // Send explosion visual to clients
                self.send_to_instance(
                    &instance_id,
                    ServerMessage::Explosion {
                        x,
                        y,
                        radius,
                        damage,
                    },
                )
                .await;
            }

            BossEvent::MoveBoss {
                instance_id,
                npc_id,
                x,
                y,
            } => {
                if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                    let mut npcs = instance.npcs.write().await;
                    if let Some(npc) = npcs.get_mut(&npc_id) {
                        npc.x = x as f32;
                        npc.y = y as f32;
                        npc.spawn_x = x;
                        npc.spawn_y = y;
                    }
                }
            }

            BossEvent::SetBossInvulnerable {
                instance_id,
                npc_id,
                invulnerable,
            } => {
                if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                    let mut npcs = instance.npcs.write().await;
                    if let Some(npc) = npcs.get_mut(&npc_id) {
                        npc.invulnerable = invulnerable;
                    }
                }
            }

            BossEvent::BossDied {
                instance_id,
                killer_id,
            } => {
                tracing::info!(
                    "Desert Wurm defeated in instance {} by {}",
                    instance_id,
                    killer_id
                );
                // Loot is handled by existing NPC death -> loot generation pipeline
                // Clean up minions
                if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                    let mut npcs = instance.npcs.write().await;
                    npcs.retain(|id, _| !id.starts_with("boss_minion_"));
                }
            }
        }
    }

    /// Helper: send a message to all players in a specific instance
    async fn send_to_instance(&self, instance_id: &str, msg: ServerMessage) {
        let instances = self.player_instances.read().await;
        let player_ids: Vec<String> = instances
            .iter()
            .filter(|(_, iid)| iid.as_str() == instance_id)
            .map(|(pid, _)| pid.clone())
            .collect();
        drop(instances);

        for pid in player_ids {
            self.send_to_player(&pid, msg.clone()).await;
        }
    }

    /// Start a boss fight session when players enter the arena
    pub async fn start_boss_session(
        &self,
        instance_id: &str,
        boss_npc_id: &str,
        boss_hp: i32,
        boss_x: i32,
        boss_y: i32,
        map_width: u32,
        map_height: u32,
        current_time: u64,
        player_ids: Vec<String>,
    ) {
        let boss = BossState::new(
            instance_id.to_string(),
            boss_npc_id.to_string(),
            boss_hp,
            boss_x,
            boss_y,
            map_width,
            map_height,
            current_time,
            player_ids,
        );
        let mut states = self.boss_states.write().await;
        states.insert(instance_id.to_string(), boss);
        tracing::info!(
            "Desert Wurm boss fight started in instance {}",
            instance_id
        );
    }

    /// Called when an NPC dies in an instance - checks if it's a boss minion
    pub(in crate::game) async fn check_boss_minion_death(
        &self,
        npc_id: &str,
        instance_id: &str,
        npc_x: i32,
        npc_y: i32,
        current_time: u64,
    ) {
        if !npc_id.starts_with("boss_minion_") {
            return;
        }

        // Minion died -> trigger explosion
        let mut boss_states = self.boss_states.write().await;
        if let Some(boss) = boss_states.get_mut(instance_id) {
            let events = boss.on_minion_exploded(npc_x, npc_y);
            drop(boss_states);

            for event in events {
                self.handle_boss_event(event, current_time).await;
            }
        }
    }

    /// Called when the boss NPC itself dies
    pub(in crate::game) async fn check_boss_npc_death(
        &self,
        npc_id: &str,
        instance_id: &str,
        killer_id: &str,
        current_time: u64,
    ) {
        let mut boss_states = self.boss_states.write().await;
        if let Some(boss) = boss_states.get_mut(instance_id) {
            if boss.boss_npc_id == npc_id {
                boss.wurm_state = crate::boss::WurmState::Dead;
                let events = vec![BossEvent::BossDied {
                    instance_id: instance_id.to_string(),
                    killer_id: killer_id.to_string(),
                }];
                drop(boss_states);

                for event in events {
                    self.handle_boss_event(event, current_time).await;
                }
            }
        }
    }
}
```

**Step 3: Wire into the tick loop**

At `rust-server/src/game.rs:6269` (after `process_koth_tick`), add:

```rust
        // Boss fight tick
        self.process_boss_tick(current_time).await;
```

**Step 4: Commit**

```bash
git add rust-server/src/game/boss_tick.rs rust-server/src/game.rs
git commit -m "feat: add boss tick handler and wire into game loop"
```

---

### Task 5: NPC Invulnerability Flag

**Files:**
- Modify: `rust-server/src/npc.rs:40-60` (add `invulnerable` field to Npc struct)

**Step 1: Add invulnerable field to Npc**

Find the `pub struct Npc` definition and add:

```rust
    /// Boss mechanic: when true, NPC cannot take damage
    pub invulnerable: bool,
```

Initialize it to `false` in `Npc::from_prototype` and `Npc::new` (wherever Npc is constructed).

**Step 2: Guard damage in the combat handler**

Find where NPC `take_damage` is called in `game.rs` (the `handle_attack` function) and add a guard:

```rust
if npc.invulnerable {
    // Show a "0" or miss indicator to the player
    return;
}
```

**Step 3: Commit**

```bash
git add rust-server/src/npc.rs rust-server/src/game.rs
git commit -m "feat: add invulnerable flag to NPCs for boss dig mechanic"
```

---

### Task 6: Explosive Minion Contact Detection

**Files:**
- Modify: `rust-server/src/game/instance_npc_tick.rs` (detect when minion reaches a player)

**Step 1: Add contact explosion check**

In `process_instance_npc_tick()`, after NPC movement updates, add a check: if an NPC's prototype is `wurm_minion` and it's adjacent to (or on the same tile as) a player, trigger its death (which triggers the explosion via `check_boss_minion_death`).

```rust
// Check explosive minion contact
if npc.prototype_id == "wurm_minion" && npc.is_alive() {
    for (player_id, player) in &instance_players {
        let px = player.x.round() as i32;
        let py = player.y.round() as i32;
        let nx = npc.x.round() as i32;
        let ny = npc.y.round() as i32;
        if (px - nx).abs() <= 1 && (py - ny).abs() <= 1 {
            // Contact! Kill the minion (triggers explosion in boss_tick)
            npc.hp = 0;
            contact_explosions.push((npc_id.clone(), instance_id.clone(), nx, ny));
            break;
        }
    }
}
```

Return these contact explosions so `game.rs` can call `check_boss_minion_death` for each.

**Step 2: Commit**

```bash
git add rust-server/src/game/instance_npc_tick.rs
git commit -m "feat: detect explosive minion contact with players"
```

---

### Task 7: Arena Interior Map

**Files:**
- Create: `rust-server/maps/interiors/desert_wurm_arena.json`

**Step 1: Create the arena map**

Follow the format of `rust-server/maps/interiors/koth_arena.json`. Create a 32x32 sand arena:

```json
{
  "id": "desert_wurm_arena",
  "name": "Desert Wurm Lair",
  "instance_type": "public",
  "size": { "width": 32, "height": 32 },
  "spawn_points": {
    "entrance": { "x": 16.0, "y": 28.0 }
  },
  "portals": [
    {
      "id": "wurm_exit",
      "x": 15,
      "y": 30,
      "width": 3,
      "height": 2,
      "target_map": "overworld",
      "target_x": 0.0,
      "target_y": 0.0
    }
  ],
  "entities": [
    {
      "entity_id": "desert_wurm",
      "x": 16,
      "y": 10,
      "level": 60,
      "unique_id": "desert_wurm_boss",
      "respawn": false
    }
  ],
  "layers": { "ground": [] },
  "collision": [],
  "objects": [],
  "walls": [],
  "heightmap": []
}
```

**Note:** The `layers.ground`, `collision`, `objects`, `walls`, and `heightmap` arrays need to be filled with actual tile data (32x32 = 1024 entries). The ground layer should use sand tile GIDs. Collision should block the edges and leave the interior open. This will likely need to be created/edited in the mapper tool.

**Step 2: Commit**

```bash
git add rust-server/maps/interiors/desert_wurm_arena.json
git commit -m "feat: add desert wurm arena interior map"
```

---

### Task 8: Hook Boss Session Start to Instance Entry

**Files:**
- Modify: `rust-server/src/game/travel.rs` or wherever instance entry is handled

**Step 1: Find where KOTH session starts on instance entry**

Search for `start_koth_session` calls to find the pattern. When a player enters the `desert_wurm_arena` instance, call `start_boss_session` with the boss NPC's stats.

```rust
// After player enters desert_wurm_arena instance:
if map_id == "desert_wurm_arena" {
    // Find the boss NPC in the instance
    if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
        let npcs = instance.npcs.read().await;
        for (npc_id, npc) in npcs.iter() {
            if npc.prototype_id == "desert_wurm" {
                // Only start if no active boss state exists
                let boss_states = self.boss_states.read().await;
                if !boss_states.contains_key(&instance_id) {
                    drop(boss_states);
                    self.start_boss_session(
                        &instance_id,
                        npc_id,
                        npc.max_hp,
                        npc.x as i32,
                        npc.y as i32,
                        instance.map_width,
                        instance.map_height,
                        current_time,
                        vec![player_id.to_string()],
                    ).await;
                } else {
                    // Boss fight already active, add player
                    drop(boss_states);
                    let mut boss_states = self.boss_states.write().await;
                    if let Some(boss) = boss_states.get_mut(&instance_id) {
                        boss.add_player(player_id.to_string());
                    }
                }
                break;
            }
        }
    }
}
```

**Step 2: Commit**

```bash
git add rust-server/src/game/travel.rs
git commit -m "feat: start boss session when players enter desert wurm arena"
```

---

### Task 9: Wire NPC Death to Boss Checks

**Files:**
- Modify: `rust-server/src/game.rs` (wherever NPC death is processed)

**Step 1: Find existing NPC death handler**

Look for where `check_koth_npc_death` is called. Add parallel calls to `check_boss_minion_death` and `check_boss_npc_death`:

```rust
// After existing NPC death processing:
self.check_boss_minion_death(npc_id, instance_id, npc_x, npc_y, current_time).await;
self.check_boss_npc_death(npc_id, instance_id, killer_id, current_time).await;
```

**Step 2: Commit**

```bash
git add rust-server/src/game.rs
git commit -m "feat: wire NPC death events to boss minion/death handlers"
```

---

### Task 10: Client Boss State & Message Handling

**Files:**
- Modify: `client/src/game/state.rs:1182` (add boss client state structs near KOTH structs)
- Modify: `client/src/game/state.rs:1908` (add boss fields to GameState)
- Modify: `client/src/network/message_handler.rs` (handle new messages)

**Step 1: Add client-side boss state structs**

Near the `KothClientState` definition at line 1182:

```rust
/// Boss fight client state
#[derive(Debug, Clone)]
pub struct BossClientState {
    pub boss_id: String,
    pub hp: i32,
    pub max_hp: i32,
    pub phase: String,
    pub wurm_state: String,
}

/// AOE warning zone being displayed
#[derive(Debug, Clone)]
pub struct AoeWarningZone {
    pub tiles: Vec<(i32, i32)>,
    pub created_at: f64,
    pub delay_ms: u64,
    pub effect: String,
}

/// Active explosion effect
#[derive(Debug, Clone)]
pub struct ExplosionEffect {
    pub x: i32,
    pub y: i32,
    pub radius: i32,
    pub created_at: f64,
}
```

**Step 2: Add fields to GameState**

Near the KOTH fields (around line 1911):

```rust
    /// Boss fight state (active when in boss arena)
    pub boss: Option<BossClientState>,
    /// Active AOE warning zones
    pub aoe_warnings: Vec<AoeWarningZone>,
    /// Active explosion effects
    pub explosions: Vec<ExplosionEffect>,
```

Initialize them in `GameState::new()` / `Default`:

```rust
    boss: None,
    aoe_warnings: Vec::new(),
    explosions: Vec::new(),
```

**Step 3: Handle messages in message_handler.rs**

Add handlers for `BossStateUpdate`, `AoeWarning`, `AoeDamage`, `Explosion` similar to the KOTH message handlers:

```rust
// BossStateUpdate
"BossStateUpdate" | _ if msg_type_matches("BossStateUpdate") => {
    state.boss = Some(BossClientState {
        boss_id: extract_string(&map, "boss_id").unwrap_or_default(),
        hp: extract_i32(&map, "hp").unwrap_or(0),
        max_hp: extract_i32(&map, "max_hp").unwrap_or(1),
        phase: extract_string(&map, "phase").unwrap_or_default(),
        wurm_state: extract_string(&map, "wurm_state").unwrap_or_default(),
    });
}

// AoeWarning
"AoeWarning" => {
    let tiles = extract_tile_vec(&map, "tiles");
    let delay_ms = extract_u64(&map, "delay_ms").unwrap_or(1500);
    let effect = extract_string(&map, "effect").unwrap_or_default();
    state.aoe_warnings.push(AoeWarningZone {
        tiles,
        created_at: get_time(),
        delay_ms,
        effect,
    });
}

// AoeDamage - remove corresponding warnings (visual only, damage is server-side)
"AoeDamage" => {
    // Warnings auto-expire, this is just for visual rock impact effects
}

// Explosion
"Explosion" => {
    let x = extract_i32(&map, "x").unwrap_or(0);
    let y = extract_i32(&map, "y").unwrap_or(0);
    let radius = extract_i32(&map, "radius").unwrap_or(1);
    state.explosions.push(ExplosionEffect {
        x,
        y,
        radius,
        created_at: get_time(),
    });
}
```

**Step 4: Clean up expired effects in the game update loop**

```rust
// Clean up expired AOE warnings (remove after delay_ms + small buffer)
state.aoe_warnings.retain(|w| {
    let elapsed = (get_time() - w.created_at) * 1000.0;
    elapsed < (w.delay_ms as f64 + 500.0)
});

// Clean up expired explosions (show for ~1 second)
state.explosions.retain(|e| {
    get_time() - e.created_at < 1.0
});
```

**Step 5: Commit**

```bash
git add client/src/game/state.rs client/src/network/message_handler.rs
git commit -m "feat: add boss fight client state and message handling"
```

---

### Task 11: Client Boss HP Bar UI

**Files:**
- Create: `client/src/render/ui/boss_hud.rs`
- Modify: `client/src/render/ui/mod.rs` (add `pub mod boss_hud;`)
- Modify: `client/src/render/renderer.rs` (call `render_boss_hud` in render loop)

**Step 1: Create the boss HUD**

Follow the pattern of `client/src/render/ui/koth.rs`:

```rust
//! Desert Wurm boss fight HUD - HP bar and phase indicator

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

impl Renderer {
    /// Render the boss HP bar at the top of the screen
    pub(crate) fn render_boss_hud(&self, state: &GameState) {
        let boss = match &state.boss {
            Some(b) => b,
            None => return,
        };

        let s = state.ui_state.ui_scale;
        let (sw, _sh) = virtual_screen_size();

        let bar_width = 300.0 * s;
        let bar_height = 24.0 * s;
        let bar_x = (sw - bar_width) / 2.0;
        let bar_y = 12.0 * s;

        // Background panel
        let panel_width = bar_width + 20.0 * s;
        let panel_height = 50.0 * s;
        let panel_x = (sw - panel_width) / 2.0;
        let panel_y = 6.0 * s;
        draw_rectangle(panel_x, panel_y, panel_width, panel_height, Color::new(0.0, 0.0, 0.0, 0.75));
        draw_rectangle_lines(panel_x, panel_y, panel_width, panel_height, 2.0, FRAME_ACCENT);

        // Boss name
        let name = "Desert Wurm";
        let name_dims = self.measure_text_sharp(name, 16.0);
        self.draw_text_sharp(
            name,
            (sw - name_dims.width) / 2.0,
            panel_y + 16.0 * s,
            16.0,
            TEXT_TITLE,
        );

        // HP bar
        let hp_pct = if boss.max_hp > 0 {
            boss.hp as f32 / boss.max_hp as f32
        } else {
            0.0
        };

        // Bar color changes by phase
        let bar_color = match boss.phase.as_str() {
            "hunt" => Color::new(0.2, 0.8, 0.2, 0.9),
            "storm" => Color::new(0.9, 0.7, 0.1, 0.9),
            "frenzy" => Color::new(0.9, 0.2, 0.1, 0.9),
            _ => Color::new(0.8, 0.2, 0.2, 0.9),
        };

        let bar_y = panel_y + 24.0 * s;
        draw_rectangle(bar_x, bar_y, bar_width, bar_height, Color::new(0.15, 0.1, 0.1, 0.9));
        draw_rectangle(bar_x, bar_y, bar_width * hp_pct, bar_height, bar_color);
        draw_rectangle_lines(bar_x, bar_y, bar_width, bar_height, 1.0, FRAME_OUTER);

        // HP text
        let hp_text = format!("{} / {}", boss.hp, boss.max_hp);
        let hp_dims = self.measure_text_sharp(&hp_text, 16.0);
        self.draw_text_sharp(
            &hp_text,
            (sw - hp_dims.width) / 2.0,
            bar_y + (bar_height + hp_dims.height) / 2.0,
            16.0,
            WHITE,
        );
    }
}
```

**Step 2: Register the module**

In `client/src/render/ui/mod.rs`, add:

```rust
pub mod boss_hud;
```

**Step 3: Call in render loop**

In `client/src/render/renderer.rs`, find where `render_koth_hud` is called and add nearby:

```rust
self.render_boss_hud(state);
```

**Step 4: Commit**

```bash
git add client/src/render/ui/boss_hud.rs client/src/render/ui/mod.rs client/src/render/renderer.rs
git commit -m "feat: add boss HP bar UI with phase-colored health bar"
```

---

### Task 12: Client AOE Warning Tile Rendering

**Files:**
- Modify: `client/src/render/renderer.rs` (add AOE warning and explosion rendering)

**Step 1: Render AOE warning zones**

Add a function that renders pulsing danger overlays on warned tiles. Call it during the tile rendering pass (after ground, before entities):

```rust
/// Render AOE warning zones as pulsing red overlays
fn render_aoe_warnings(&self, state: &GameState) {
    let current_time = get_time();

    for warning in &state.aoe_warnings {
        let elapsed = (current_time - warning.created_at) * 1000.0;
        if elapsed > warning.delay_ms as f64 {
            continue; // Already landed
        }

        // Pulsing alpha (faster as landing approaches)
        let progress = elapsed / warning.delay_ms as f64;
        let pulse_speed = 4.0 + progress * 8.0;
        let alpha = 0.2 + 0.3 * (elapsed as f32 * pulse_speed).sin().abs();

        for &(tx, ty) in &warning.tiles {
            let (sx, sy) = world_to_screen(tx as f32, ty as f32, 0);
            // Draw red overlay on the tile
            draw_rectangle(
                sx - TILE_WIDTH as f32 / 2.0,
                sy - TILE_HEIGHT as f32 / 2.0,
                TILE_WIDTH as f32,
                TILE_HEIGHT as f32,
                Color::new(1.0, 0.2, 0.0, alpha),
            );
        }
    }
}

/// Render explosion effects
fn render_explosions(&self, state: &GameState) {
    let current_time = get_time();

    for explosion in &state.explosions {
        let elapsed = current_time - explosion.created_at;
        if elapsed > 1.0 {
            continue;
        }

        let alpha = (1.0 - elapsed as f32).max(0.0);
        let scale = 1.0 + elapsed as f32 * 0.5; // Expanding circle

        for dx in -explosion.radius..=explosion.radius {
            for dy in -explosion.radius..=explosion.radius {
                let (sx, sy) = world_to_screen(
                    (explosion.x + dx) as f32,
                    (explosion.y + dy) as f32,
                    0,
                );
                draw_rectangle(
                    sx - TILE_WIDTH as f32 * scale / 2.0,
                    sy - TILE_HEIGHT as f32 * scale / 2.0,
                    TILE_WIDTH as f32 * scale,
                    TILE_HEIGHT as f32 * scale,
                    Color::new(1.0, 0.6, 0.0, alpha * 0.6),
                );
            }
        }
    }
}
```

**Step 2: Call these in the main render function**

After ground rendering, before entity rendering.

**Step 3: Commit**

```bash
git add client/src/render/renderer.rs
git commit -m "feat: add AOE warning tile overlays and explosion effects"
```

---

### Task 13: New Items (Wurm Blade & Cactus Seeds)

**Files:**
- Modify: `rust-server/data/items/equipment.toml` (add wurm_blade)
- Modify: `rust-server/data/items/seeds.toml` or appropriate seeds file (add cactus_seed)

**Step 1: Add wurm blade to equipment**

Find the weapons section in `rust-server/data/items/equipment.toml` and add:

```toml
[wurm_blade]
display_name = "Wurm Blade"
sprite = "wurm_blade"
description = "A razor-sharp blade carved from a desert wurm's fang."
category = "equipment"
max_stack = 1
base_price = 5000
sellable = true

[wurm_blade.equipment]
slot = "weapon"
weapon_type = "melee"
attack_bonus = 35
strength_bonus = 30
defence_bonus = 0
level_required = 55
```

**Step 2: Add cactus seeds**

Find the seeds file and add:

```toml
[cactus_seed]
display_name = "Cactus Seed"
sprite = "cactus_seed"
description = "A prickly seed from the deep desert. Plant it in a farming patch."
category = "seed"
max_stack = 50
base_price = 100
sellable = true
```

**Step 3: Commit**

```bash
git add rust-server/data/items/equipment.toml rust-server/data/items/seeds.toml
git commit -m "feat: add wurm blade weapon and cactus seed items"
```

---

### Task 14: Integration Testing & Cleanup

**Files:**
- All modified files

**Step 1: Build the server**

```bash
cd rust-server && cargo build 2>&1 | head -50
```

Fix any compilation errors. Common issues:
- Missing imports in `boss.rs` or `boss_tick.rs`
- Field mismatches between protocol variants and serde
- Missing `invulnerable` field initializations in Npc constructors

**Step 2: Build the client**

```bash
cd client && cargo build 2>&1 | head -50
```

Fix any compilation errors. Common issues:
- Missing struct fields in GameState initialization
- Missing message handler branches
- Import path issues for new types

**Step 3: Verify no regressions**

Run the server and verify:
- Existing KOTH still works
- Normal combat still works
- Instance entry/exit still works

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: desert wurm boss fight - complete implementation"
```
