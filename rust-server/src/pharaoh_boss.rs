use std::collections::{HashMap, HashSet};

use crate::boss::{BossEvent, BossPhase};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_MINIONS: u32 = 8;

/// Arena bounds — same convention as the Wurm boss.
const ARENA_MIN: i32 = 7;
const ARENA_MAX: i32 = 27;

// ---------------------------------------------------------------------------
// Pharaoh state (simple: alive or dead)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum PharaohState {
    Active,
    Dead,
}

// ---------------------------------------------------------------------------
// Phase-specific configuration
// ---------------------------------------------------------------------------

struct PhaseConfig {
    projectile_interval: u64,
    projectile_damage: i32,
    minion_count: u32,
    minion_interval: u64,
    arena_shrink_interval: u64,
}

fn phase_config(phase: &BossPhase) -> PhaseConfig {
    match phase {
        BossPhase::Hunt => PhaseConfig {
            projectile_interval: 2_000,
            projectile_damage: 8,
            minion_count: 2,
            minion_interval: 15_000,
            arena_shrink_interval: 0, // no shrink in phase 1
        },
        BossPhase::Storm => PhaseConfig {
            projectile_interval: 1_500,
            projectile_damage: 8,
            minion_count: 3,
            minion_interval: 12_000,
            arena_shrink_interval: 0, // no shrink in phase 2
        },
        BossPhase::Frenzy => PhaseConfig {
            projectile_interval: 1_000,
            projectile_damage: 12,
            minion_count: 4,
            minion_interval: 10_000,
            arena_shrink_interval: 20_000,
        },
    }
}

// ---------------------------------------------------------------------------
// Helper: pick a minion spawn position on the arena edge
// ---------------------------------------------------------------------------

fn pick_minion_spawn(seed: u64) -> (i32, i32) {
    let side_len = ARENA_MAX - ARENA_MIN + 1;
    let perimeter = 4 * side_len - 4;
    let pos = (seed % perimeter.max(1) as u64) as i32;

    if pos < side_len {
        (ARENA_MIN + pos, ARENA_MIN)
    } else if pos < 2 * side_len - 1 {
        (ARENA_MAX, ARENA_MIN + (pos - side_len + 1))
    } else if pos < 3 * side_len - 2 {
        (ARENA_MAX - (pos - 2 * side_len + 2), ARENA_MAX)
    } else {
        (ARENA_MIN, ARENA_MAX - (pos - 3 * side_len + 3))
    }
}

/// Compute the outer ring of tiles for a given shrink layer.
/// Layer 0 = outermost ring (ARENA_MIN), layer 1 = next ring in, etc.
fn arena_shrink_tiles(layer: u32) -> Vec<(i32, i32)> {
    let l = layer as i32;
    let min = ARENA_MIN + l;
    let max = ARENA_MAX - l;
    if min >= max {
        return Vec::new();
    }
    let mut tiles = Vec::new();
    for x in min..=max {
        tiles.push((x, min));
        tiles.push((x, max));
    }
    for y in (min + 1)..max {
        tiles.push((min, y));
        tiles.push((max, y));
    }
    tiles
}

// ---------------------------------------------------------------------------
// Minion prototype helper
// ---------------------------------------------------------------------------

fn minion_prototype(phase: &BossPhase, index: u32) -> &'static str {
    match phase {
        BossPhase::Hunt => "pharaoh_mummy",
        BossPhase::Storm => "pharaoh_skeleton",
        BossPhase::Frenzy => {
            if index.is_multiple_of(2) {
                "pharaoh_mummy"
            } else {
                "pharaoh_skeleton"
            }
        }
    }
}

// ---------------------------------------------------------------------------
// PharaohBossState
// ---------------------------------------------------------------------------

pub struct PharaohBossState {
    pub instance_id: String,
    pub boss_npc_id: String,
    pub phase: BossPhase,
    pub state: PharaohState,
    pub boss_hp: i32,
    pub boss_max_hp: i32,
    pub boss_x: i32,
    pub boss_y: i32,
    pub map_width: i32,
    pub map_height: i32,
    pub last_projectile_time: u64,
    pub last_minion_spawn_time: u64,
    pub last_arena_shrink_time: u64,
    pub minion_counter: u32,
    pub live_minion_count: u32,
    pub player_ids: Vec<String>,
    pub death_time: u64,
    pub countdown_sent: u8,
    pub damage_dealers: HashSet<String>,
    pub arena_shrink_layer: u32,
}

impl PharaohBossState {
    pub fn new(
        instance_id: String,
        boss_npc_id: String,
        boss_hp: i32,
        boss_max_hp: i32,
        boss_x: i32,
        boss_y: i32,
        map_width: i32,
        map_height: i32,
        current_time: u64,
    ) -> Self {
        Self {
            instance_id,
            boss_npc_id,
            phase: BossPhase::Hunt,
            state: PharaohState::Active,
            boss_hp,
            boss_max_hp,
            boss_x,
            boss_y,
            map_width,
            map_height,
            last_projectile_time: current_time,
            last_minion_spawn_time: current_time,
            last_arena_shrink_time: current_time,
            minion_counter: 0,
            live_minion_count: 0,
            player_ids: Vec::new(),
            death_time: 0,
            countdown_sent: 0,
            damage_dealers: HashSet::new(),
            arena_shrink_layer: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Main tick — returns events for the GameRoom to process
    // -----------------------------------------------------------------------

    pub fn tick(&mut self, current_time: u64) -> Vec<BossEvent> {
        let mut events = Vec::new();

        if self.state == PharaohState::Dead {
            return events;
        }

        // Track previous phase so we can announce transitions
        let prev_phase = self.phase.clone();
        self.update_phase();

        // Announce phase transitions
        if self.phase != prev_phase {
            let msg = match self.phase {
                BossPhase::Hunt => "The Cursed Pharaoh stirs...",
                BossPhase::Storm => "The Pharaoh's wrath intensifies!",
                BossPhase::Frenzy => "The Pharaoh enters a desperate frenzy!",
            };
            events.push(BossEvent::Announcement {
                instance_id: self.instance_id.clone(),
                message: msg.to_string(),
            });
        }

        let config = phase_config(&self.phase);

        // --- Projectile firing ---
        if current_time.saturating_sub(self.last_projectile_time) >= config.projectile_interval {
            self.last_projectile_time = current_time;

            // Emit a single-tile AoeWarning; boss_tick will resolve the actual target
            events.push(BossEvent::AoeWarning {
                instance_id: self.instance_id.clone(),
                tiles: vec![(self.boss_x, self.boss_y)],
                delay_ms: 0,
                effect: format!("pharaoh_projectile:{}", config.projectile_damage),
            });
        }

        // --- Minion spawning ---
        if current_time.saturating_sub(self.last_minion_spawn_time) >= config.minion_interval
            && self.live_minion_count < MAX_MINIONS
        {
            self.last_minion_spawn_time = current_time;
            let can_spawn = (MAX_MINIONS - self.live_minion_count).min(config.minion_count);

            for i in 0..can_spawn {
                self.minion_counter += 1;
                self.live_minion_count += 1;

                let seed = current_time
                    .wrapping_add(self.minion_counter as u64)
                    .wrapping_add(i as u64 * 41);
                let (x, y) = pick_minion_spawn(seed);

                let _prototype = minion_prototype(&self.phase, i);
                let npc_id = format!(
                    "pharaoh_minion_{}_{}",
                    self.instance_id, self.minion_counter
                );

                events.push(BossEvent::SpawnMinion {
                    instance_id: self.instance_id.clone(),
                    npc_id,
                    x,
                    y,
                });
            }
        }

        // --- Arena shrink (Phase 3 / Frenzy only) ---
        if self.phase == BossPhase::Frenzy
            && config.arena_shrink_interval > 0
            && current_time.saturating_sub(self.last_arena_shrink_time)
                >= config.arena_shrink_interval
        {
            self.last_arena_shrink_time = current_time;
            let tiles = arena_shrink_tiles(self.arena_shrink_layer);
            if !tiles.is_empty() {
                // Warning first
                events.push(BossEvent::AoeWarning {
                    instance_id: self.instance_id.clone(),
                    tiles: tiles.clone(),
                    delay_ms: 2_000,
                    effect: "arena_shrink".to_string(),
                });
                // Then damage
                events.push(BossEvent::AoeDamage {
                    instance_id: self.instance_id.clone(),
                    tiles,
                    damage: 5,
                    effect: "arena_shrink".to_string(),
                });
                self.arena_shrink_layer += 1;
            }
        }

        // --- State update (every tick) ---
        let phase_str = match self.phase {
            BossPhase::Hunt => "hunt",
            BossPhase::Storm => "storm",
            BossPhase::Frenzy => "frenzy",
        };
        events.push(BossEvent::StateUpdate {
            instance_id: self.instance_id.clone(),
            boss_hp: self.boss_hp,
            boss_max_hp: self.boss_max_hp,
            phase: phase_str.to_string(),
            wurm_state: "active".to_string(), // pharaoh is always stationary
        });

        events
    }

    // -----------------------------------------------------------------------
    // Phase logic
    // -----------------------------------------------------------------------

    fn update_phase(&mut self) {
        if self.boss_max_hp <= 0 {
            return;
        }
        let hp_pct = (self.boss_hp as f64 / self.boss_max_hp as f64) * 100.0;
        self.phase = if hp_pct > 66.0 {
            BossPhase::Hunt
        } else if hp_pct > 33.0 {
            BossPhase::Storm
        } else {
            BossPhase::Frenzy
        };
    }

    // -----------------------------------------------------------------------
    // Damage handling
    // -----------------------------------------------------------------------

    pub fn on_boss_damaged(&mut self, damage: i32, attacker_id: Option<String>) -> Vec<BossEvent> {
        self.boss_hp = (self.boss_hp - damage).max(0);

        if let Some(ref id) = attacker_id {
            self.damage_dealers.insert(id.clone());
        }

        self.update_phase();

        let mut events = Vec::new();
        if self.boss_hp <= 0 {
            self.state = PharaohState::Dead;
            events.push(BossEvent::BossDied {
                instance_id: self.instance_id.clone(),
                killer_id: attacker_id,
            });
        }
        events
    }

    // -----------------------------------------------------------------------
    // Minion lifecycle
    // -----------------------------------------------------------------------

    pub fn on_minion_died(&mut self) {
        self.live_minion_count = self.live_minion_count.saturating_sub(1);
    }

    // -----------------------------------------------------------------------
    // Player management
    // -----------------------------------------------------------------------

    pub fn add_player(&mut self, player_id: String) {
        if !self.player_ids.contains(&player_id) {
            self.player_ids.push(player_id);
        }
    }

    pub fn remove_player(&mut self, player_id: &str) {
        self.player_ids.retain(|id| id != player_id);
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    pub fn is_dead(&self) -> bool {
        self.state == PharaohState::Dead
    }
}

/// Maps instance_id -> PharaohBossState for all active pharaoh boss fights.
pub type PharaohBossStates = HashMap<String, PharaohBossState>;
