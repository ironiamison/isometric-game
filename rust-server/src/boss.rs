use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DIG_DURATION_MS: u64 = 3000;
const SUBMERGE_ANIM_MS: u64 = 750;
const EMERGE_DURATION_MS: u64 = 1500;
const AOE_WARNING_DELAY_MS: u64 = 1250;
pub const MINION_EXPLOSION_DAMAGE: i32 = 10;
/// Maximum number of live minions allowed per boss instance
const MAX_MINIONS: u32 = 4;

// ---------------------------------------------------------------------------
// Boss phase (based on HP thresholds)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum BossPhase {
    Hunt,
    Storm,
    Frenzy,
}

// ---------------------------------------------------------------------------
// Wurm movement state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum WurmState {
    Surface,
    Submerging { ends_at: u64 },
    Digging { ends_at: u64, target_x: i32, target_y: i32 },
    Emerging { ends_at: u64, target_x: i32, target_y: i32 },
    Dead,
}

// ---------------------------------------------------------------------------
// AoE zone tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AoeZone {
    pub tiles: Vec<(i32, i32)>,
    pub damage: i32,
    pub lands_at: u64,
    pub sent_warning: bool,
}

// ---------------------------------------------------------------------------
// Phase configuration (private)
// ---------------------------------------------------------------------------

struct PhaseConfig {
    dig_interval: u64,
    rock_count: u32,
    minion_count: u32,
    minion_interval: u64,
    melee_dmg_mult: f32,
}

fn phase_config(phase: &BossPhase) -> PhaseConfig {
    match phase {
        BossPhase::Hunt => PhaseConfig {
            dig_interval: 15_000,
            rock_count: 3,
            minion_count: 1,
            minion_interval: 25_000,
            melee_dmg_mult: 1.0,
        },
        BossPhase::Storm => PhaseConfig {
            dig_interval: 18_000,
            rock_count: 5,
            minion_count: 1,
            minion_interval: 20_000,
            melee_dmg_mult: 1.3,
        },
        BossPhase::Frenzy => PhaseConfig {
            dig_interval: 15_000,
            rock_count: 7,
            minion_count: 2,
            minion_interval: 15_000,
            melee_dmg_mult: 1.6,
        },
    }
}

// ---------------------------------------------------------------------------
// Events emitted for GameRoom to process
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum BossEvent {
    StateUpdate {
        instance_id: String,
        boss_hp: i32,
        boss_max_hp: i32,
        phase: String,
        wurm_state: String,
    },
    SpawnMinion {
        instance_id: String,
        npc_id: String,
        x: i32,
        y: i32,
    },
    AoeWarning {
        instance_id: String,
        tiles: Vec<(i32, i32)>,
        delay_ms: u64,
        effect: String,
    },
    AoeDamage {
        instance_id: String,
        tiles: Vec<(i32, i32)>,
        damage: i32,
        effect: String,
    },
    Explosion {
        instance_id: String,
        x: i32,
        y: i32,
        radius: i32,
        damage: i32,
    },
    BossDied {
        instance_id: String,
        killer_id: Option<String>,
    },
    MoveBoss {
        instance_id: String,
        npc_id: String,
        x: i32,
        y: i32,
    },
    SetBossInvulnerable {
        instance_id: String,
        npc_id: String,
        invulnerable: bool,
    },
    SetBossNpcState {
        instance_id: String,
        npc_id: String,
        state: u8,
    },
    HideBoss {
        instance_id: String,
        npc_id: String,
        hidden: bool,
    },
    Announcement {
        instance_id: String,
        message: String,
    },
    TeleportOut {
        instance_id: String,
    },
}

// ---------------------------------------------------------------------------
// Boss state per-instance
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
    pub map_width: i32,
    pub map_height: i32,
    pub last_dig_time: u64,
    pub last_minion_spawn_time: u64,
    pub minion_counter: u32,
    pub aoe_zones: Vec<AoeZone>,
    pub player_ids: Vec<String>,
    pub live_minion_count: u32,
    /// Last time the boss moved a tile while burrowing
    pub last_burrow_move_time: u64,
    /// Timestamp when the boss died (0 = alive)
    pub death_time: u64,
    /// Countdown seconds already announced (3, 2, 1)
    pub countdown_sent: u8,
    /// Players who dealt damage to the boss (for loot distribution)
    pub damage_dealers: std::collections::HashSet<String>,
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

// Arena bounds — all spawns, effects, and movement are clamped to this region
const ARENA_MIN: i32 = 7;
const ARENA_MAX: i32 = 27;

/// Clamp a coordinate pair to the arena bounds.
fn clamp_arena(x: i32, y: i32) -> (i32, i32) {
    (x.clamp(ARENA_MIN, ARENA_MAX), y.clamp(ARENA_MIN, ARENA_MAX))
}

/// Generate random rock-throw target tiles spread across the arena.
fn generate_rock_targets(
    boss_x: i32,
    boss_y: i32,
    count: u32,
    _map_w: i32,
    _map_h: i32,
) -> Vec<(i32, i32)> {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let arena_w = (ARENA_MAX - ARENA_MIN + 1) as u64;
    let mut targets = Vec::new();
    for i in 0..count {
        let hash = seed
            .wrapping_mul(31)
            .wrapping_add((boss_x as u64).wrapping_mul(17))
            .wrapping_add((boss_y as u64).wrapping_mul(7))
            .wrapping_add(i as u64 * 53);
        let tx = ARENA_MIN + (hash % arena_w) as i32;
        let ty = ARENA_MIN + ((hash / arena_w) % arena_w) as i32;
        targets.push((tx, ty));
    }
    targets
}

/// Pick a random-ish emerge position within the arena bounds.
fn pick_emerge_position(_map_w: i32, _map_h: i32, seed: u64) -> (i32, i32) {
    let range = (ARENA_MAX - ARENA_MIN + 1) as u64;
    let x = ARENA_MIN + (seed.wrapping_mul(7) % range) as i32;
    let y = ARENA_MIN + ((seed / 3).wrapping_mul(13) % range) as i32;
    clamp_arena(x, y)
}

/// Pick a minion spawn position on the arena edges.
fn pick_minion_spawn(_map_w: i32, _map_h: i32, seed: u64) -> (i32, i32) {
    let side_len = ARENA_MAX - ARENA_MIN + 1;
    let perimeter = 4 * side_len - 4;
    let pos = (seed % perimeter.max(1) as u64) as i32;

    if pos < side_len {
        // Top edge
        (ARENA_MIN + pos, ARENA_MIN)
    } else if pos < 2 * side_len - 1 {
        // Right edge
        (ARENA_MAX, ARENA_MIN + (pos - side_len + 1))
    } else if pos < 3 * side_len - 2 {
        // Bottom edge
        (ARENA_MAX - (pos - 2 * side_len + 2), ARENA_MAX)
    } else {
        // Left edge
        (ARENA_MIN, ARENA_MAX - (pos - 3 * side_len + 3))
    }
}

// ---------------------------------------------------------------------------
// BossState implementation
// ---------------------------------------------------------------------------

impl BossState {
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
            wurm_state: WurmState::Surface,
            boss_hp,
            boss_max_hp,
            boss_x,
            boss_y,
            map_width,
            map_height,
            last_dig_time: current_time,
            last_minion_spawn_time: current_time,
            minion_counter: 0,
            aoe_zones: Vec::new(),
            player_ids: Vec::new(),
            live_minion_count: 0,
            last_burrow_move_time: 0,
            death_time: 0,
            countdown_sent: 0,
            damage_dealers: std::collections::HashSet::new(),
        }
    }

    /// Main tick - returns events for the GameRoom to process
    pub fn tick(&mut self, current_time: u64) -> Vec<BossEvent> {
        let mut events = Vec::new();

        if self.wurm_state == WurmState::Dead {
            return events;
        }

        // Update phase from HP
        self.update_phase();

        let config = phase_config(&self.phase);

        match self.wurm_state.clone() {
            WurmState::Surface => {
                // Check dig timer
                if current_time.saturating_sub(self.last_dig_time) >= config.dig_interval {
                    self.last_dig_time = current_time;
                    self.wurm_state = WurmState::Submerging {
                        ends_at: current_time + SUBMERGE_ANIM_MS,
                    };

                    // Boss becomes invulnerable immediately
                    events.push(BossEvent::SetBossInvulnerable {
                        instance_id: self.instance_id.clone(),
                        npc_id: self.boss_npc_id.clone(),
                        invulnerable: true,
                    });

                    // Tell client to play submerge animation
                    events.push(BossEvent::SetBossNpcState {
                        instance_id: self.instance_id.clone(),
                        npc_id: self.boss_npc_id.clone(),
                        state: 6,
                    });
                }

                // Check minion spawn timer (capped at MAX_MINIONS alive)
                if current_time.saturating_sub(self.last_minion_spawn_time)
                    >= config.minion_interval
                    && self.live_minion_count < MAX_MINIONS
                {
                    self.last_minion_spawn_time = current_time;
                    let can_spawn = (MAX_MINIONS - self.live_minion_count).min(config.minion_count);
                    for i in 0..can_spawn {
                        self.minion_counter += 1;
                        self.live_minion_count += 1;
                        let seed = current_time
                            .wrapping_add(self.minion_counter as u64)
                            .wrapping_add(i as u64 * 37);
                        let (x, y) = pick_minion_spawn(self.map_width, self.map_height, seed);
                        let npc_id = format!(
                            "boss_minion_{}_{}",
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
            }
            WurmState::Submerging { ends_at } => {
                if current_time >= ends_at {
                    // Submerge animation finished — pick emerge target and start burrowing toward it
                    let seed = current_time.wrapping_mul(31).wrapping_add(self.boss_hp as u64);
                    let (tx, ty) = pick_emerge_position(self.map_width, self.map_height, seed);

                    self.wurm_state = WurmState::Digging {
                        ends_at: current_time + DIG_DURATION_MS,
                        target_x: tx,
                        target_y: ty,
                    };

                    // Set burrowing animation (state 8) — boss moves tile-by-tile during Digging
                    events.push(BossEvent::SetBossNpcState {
                        instance_id: self.instance_id.clone(),
                        npc_id: self.boss_npc_id.clone(),
                        state: 8,
                    });

                    self.last_burrow_move_time = current_time;
                }
            }
            WurmState::Digging { ends_at: _, target_x, target_y } => {
                let tx = target_x;
                let ty = target_y;
                let at_target = self.boss_x == tx && self.boss_y == ty;

                if at_target {
                    // Arrived at target — emerge
                    let rock_targets = generate_rock_targets(
                        tx,
                        ty,
                        config.rock_count,
                        self.map_width,
                        self.map_height,
                    );

                    let mut all_tiles = std::collections::HashSet::new();
                    for &(rx, ry) in &rock_targets {
                        // + cross pattern: center + 4 cardinal neighbours
                        for &(dx, dy) in &[(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
                            let (cx, cy) = clamp_arena(rx + dx, ry + dy);
                            all_tiles.insert((cx, cy));
                        }
                    }
                    self.aoe_zones.push(AoeZone {
                        tiles: all_tiles.into_iter().collect(),
                        damage: 8 + (config.melee_dmg_mult * 4.0) as i32,
                        lands_at: current_time + AOE_WARNING_DELAY_MS,
                        sent_warning: false,
                    });

                    self.wurm_state = WurmState::Emerging {
                        ends_at: current_time + EMERGE_DURATION_MS,
                        target_x: tx,
                        target_y: ty,
                    };

                    events.push(BossEvent::SetBossNpcState {
                        instance_id: self.instance_id.clone(),
                        npc_id: self.boss_npc_id.clone(),
                        state: 7,
                    });
                } else {
                    // Move one tile toward target every 150ms
                    const BURROW_MOVE_MS: u64 = 150;
                    if current_time.saturating_sub(self.last_burrow_move_time) >= BURROW_MOVE_MS {
                        self.last_burrow_move_time = current_time;

                        let dx = tx - self.boss_x;
                        let dy = ty - self.boss_y;

                        // Move along the axis with greater distance
                        let (step_x, step_y) = if dx.abs() >= dy.abs() {
                            (dx.signum(), 0)
                        } else {
                            (0, dy.signum())
                        };

                        let new_x = self.boss_x + step_x;
                        let new_y = self.boss_y + step_y;
                        self.boss_x = new_x;
                        self.boss_y = new_y;

                        events.push(BossEvent::MoveBoss {
                            instance_id: self.instance_id.clone(),
                            npc_id: self.boss_npc_id.clone(),
                            x: new_x,
                            y: new_y,
                        });
                    }
                }
            }
            WurmState::Emerging {
                ends_at,
                target_x,
                target_y,
            } => {
                if current_time >= ends_at {
                    // Update internal position tracking
                    self.boss_x = target_x;
                    self.boss_y = target_y;

                    // Boss becomes vulnerable again
                    events.push(BossEvent::SetBossInvulnerable {
                        instance_id: self.instance_id.clone(),
                        npc_id: self.boss_npc_id.clone(),
                        invulnerable: false,
                    });

                    // Back to idle animation
                    events.push(BossEvent::SetBossNpcState {
                        instance_id: self.instance_id.clone(),
                        npc_id: self.boss_npc_id.clone(),
                        state: 0,
                    });

                    self.wurm_state = WurmState::Surface;
                }
            }
            WurmState::Dead => {
                // Already handled above
            }
        }

        // Process AoE zones: send warnings, apply damage when landed
        let mut completed_indices = Vec::new();
        for (i, zone) in self.aoe_zones.iter_mut().enumerate() {
            if !zone.sent_warning {
                zone.sent_warning = true;
                let remaining = zone.lands_at.saturating_sub(current_time);
                events.push(BossEvent::AoeWarning {
                    instance_id: self.instance_id.clone(),
                    tiles: zone.tiles.clone(),
                    delay_ms: remaining,
                    effect: "rock_throw".to_string(),
                });
            }

            if current_time >= zone.lands_at {
                events.push(BossEvent::AoeDamage {
                    instance_id: self.instance_id.clone(),
                    tiles: zone.tiles.clone(),
                    damage: zone.damage,
                    effect: "rocks_aoe".to_string(),
                });
                completed_indices.push(i);
            }
        }

        // Remove completed zones in reverse order to preserve indices
        for i in completed_indices.into_iter().rev() {
            self.aoe_zones.swap_remove(i);
        }

        // Always emit state update
        let phase_str = match self.phase {
            BossPhase::Hunt => "hunt",
            BossPhase::Storm => "storm",
            BossPhase::Frenzy => "frenzy",
        };
        let wurm_str = match &self.wurm_state {
            WurmState::Surface => "surface",
            WurmState::Submerging { .. } => "submerging",
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

    /// Update phase based on HP percentage
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

    /// Called when the boss takes damage
    pub fn on_boss_damaged(&mut self, damage: i32, attacker_id: Option<String>) -> Vec<BossEvent> {
        self.boss_hp = (self.boss_hp - damage).max(0);
        self.update_phase();

        let mut events = Vec::new();

        if self.boss_hp <= 0 {
            self.wurm_state = WurmState::Dead;
            events.push(BossEvent::BossDied {
                instance_id: self.instance_id.clone(),
                killer_id: attacker_id,
            });
        }

        events
    }

    /// Called when a minion explodes (dies near a player)
    pub fn on_minion_exploded(&mut self, x: i32, y: i32) -> Vec<BossEvent> {
        self.live_minion_count = self.live_minion_count.saturating_sub(1);
        vec![BossEvent::Explosion {
            instance_id: self.instance_id.clone(),
            x,
            y,
            radius: 1,
            damage: MINION_EXPLOSION_DAMAGE,
        }]
    }

    /// Add a player to this boss fight
    pub fn add_player(&mut self, player_id: String) {
        if !self.player_ids.contains(&player_id) {
            self.player_ids.push(player_id);
        }
    }

    /// Remove a player from this boss fight
    pub fn remove_player(&mut self, player_id: &str) {
        self.player_ids.retain(|id| id != player_id);
    }

    /// Check if the boss is dead
    pub fn is_dead(&self) -> bool {
        self.wurm_state == WurmState::Dead
    }

    /// Check if the boss is currently invulnerable (underground)
    pub fn is_invulnerable(&self) -> bool {
        matches!(
            self.wurm_state,
            WurmState::Submerging { .. } | WurmState::Digging { .. } | WurmState::Emerging { .. }
        )
    }
}

/// Maps instance_id -> BossState for all active boss fights
pub type BossStates = HashMap<String, BossState>;
