use std::collections::HashMap;

// ---------------------------------------------------------------------------
// KOTH Phase state machine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum KothPhase {
    /// 3s countdown before wave starts
    Countdown { ends_at: u64 },
    /// Enemies spawning + alive
    WaveActive,
    /// Brief 2s intermission after wave cleared
    WaveComplete { ends_at: u64 },
    /// Every 5 waves - player decides to leave or continue
    Checkpoint,
    /// Player died or left
    GameOver,
}

// ---------------------------------------------------------------------------
// Pending spawn entry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PendingSpawn {
    pub prototype_id: String,
    pub level: i32,
    pub x: i32,
    pub y: i32,
}

// ---------------------------------------------------------------------------
// Reward item
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct KothReward {
    pub item_id: String,
    pub quantity: u32,
}

// ---------------------------------------------------------------------------
// KOTH state per-instance
// ---------------------------------------------------------------------------

pub struct KothState {
    pub instance_id: String,
    pub player_id: String,
    pub phase: KothPhase,
    pub current_wave: u32,
    pub points: u32,
    pub enemies_alive: u32,
    pub enemies_total: u32,
    pub spawn_queue: Vec<PendingSpawn>,
    pub last_spawn_time: u64,
    pub wave_start_time: u64,
    pub last_checkpoint_wave: u32,
    pub kills_this_wave: u32,
    pub map_width: u32,
    pub map_height: u32,
    /// Overworld position where the player entered from (for return teleport)
    pub entrance_x: i32,
    pub entrance_y: i32,
}

// ---------------------------------------------------------------------------
// Events emitted for GameRoom to process
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum KothEvent {
    /// Spawn an NPC in the instance
    SpawnNpc {
        instance_id: String,
        npc_id: String,
        prototype_id: String,
        level: i32,
        x: i32,
        y: i32,
    },
    /// Send state update to player
    StateUpdate {
        player_id: String,
        phase: String,
        wave: u32,
        points: u32,
        enemies_alive: u32,
        enemies_total: u32,
        countdown_ms: u32,
    },
    /// Send checkpoint info to player
    CheckpointReached {
        player_id: String,
        wave: u32,
        points: u32,
        rewards: Vec<KothReward>,
        next_wave_enemy_count: u32,
    },
    /// Game over - grant rewards and clean up
    GameOver {
        player_id: String,
        instance_id: String,
        waves_completed: u32,
        total_points: u32,
        rewards: Vec<KothReward>,
        victory: bool,
    },
}

// ---------------------------------------------------------------------------
// Wave configuration
// ---------------------------------------------------------------------------

/// Number of enemies for a given wave
fn enemies_for_wave(wave: u32) -> u32 {
    (3 + wave).min(25)
}

/// Enemy level based on wave and player level.
/// Starts low (~5) so any player can enjoy the first few waves, then ramps
/// toward and eventually past the player's effective level.
fn enemy_level_for_wave(wave: u32, player_level: i32) -> i32 {
    let level = match wave {
        1 => 5,
        2 => 8,
        3 => 12,
        4 => 16,
        5 => 20,
        // Waves 6+: climb by 4 per wave, eventually overtaking the player
        _ => 20 + (wave as i32 - 5) * 4,
    };
    // Cap so enemies never exceed player_level + wave/3 (keeps scaling sane)
    level.min(player_level + wave as i32 / 3).max(1)
}

/// Pick enemy prototype based on wave number
fn enemy_prototype_for_wave(wave: u32) -> &'static str {
    if wave <= 5 {
        "koth_slime"
    } else if wave <= 10 {
        "koth_skeleton"
    } else if wave <= 15 {
        "koth_spider"
    } else {
        "koth_demon"
    }
}

/// Spawn positions on the outer 2-tile ring of the map
fn generate_spawn_positions(map_width: u32, map_height: u32, count: u32) -> Vec<(i32, i32)> {
    let mut positions = Vec::new();
    let w = map_width as i32;
    let h = map_height as i32;

    // Collect all tiles in the outer 2-tile ring (avoiding corners blocked by collision)
    let mut ring_tiles = Vec::new();
    for x in 2..w - 2 {
        ring_tiles.push((x, 2)); // top edge
        ring_tiles.push((x, h - 3)); // bottom edge
    }
    for y in 2..h - 2 {
        ring_tiles.push((2, y)); // left edge
        ring_tiles.push((w - 3, y)); // right edge
    }

    if ring_tiles.is_empty() {
        return positions;
    }

    // Distribute spawns evenly around the ring
    let step = ring_tiles.len().max(1) / (count as usize).max(1);
    for i in 0..count as usize {
        let idx = (i * step) % ring_tiles.len();
        positions.push(ring_tiles[idx]);
    }

    positions
}

// ---------------------------------------------------------------------------
// Points calculation
// ---------------------------------------------------------------------------

fn wave_points(wave: u32) -> u32 {
    wave * 10
}

fn speed_bonus(wave_start_time: u64, current_time: u64) -> u32 {
    let elapsed_ms = current_time.saturating_sub(wave_start_time);
    if elapsed_ms < 30_000 { 20 } else { 0 }
}

// ---------------------------------------------------------------------------
// Reward tiers
// ---------------------------------------------------------------------------

fn checkpoint_rewards(wave: u32) -> Vec<KothReward> {
    match wave {
        // Wave 5: starter supplies - potions and food to keep you going
        1..=5 => vec![
            KothReward {
                item_id: "gold_coins".to_string(),
                quantity: 75,
            },
            KothReward {
                item_id: "health_potion".to_string(),
                quantity: 3,
            },
            KothReward {
                item_id: "weak_mana_potion".to_string(),
                quantity: 2,
            },
            KothReward {
                item_id: "cooked_trout".to_string(),
                quantity: 5,
            },
        ],
        // Wave 10: better consumables + gems
        6..=10 => vec![
            KothReward {
                item_id: "gold_coins".to_string(),
                quantity: 200,
            },
            KothReward {
                item_id: "strong_health_potion".to_string(),
                quantity: 2,
            },
            KothReward {
                item_id: "mana_potion".to_string(),
                quantity: 3,
            },
            KothReward {
                item_id: "prayer_potion".to_string(),
                quantity: 2,
            },
            KothReward {
                item_id: "uncut_sapphire".to_string(),
                quantity: 2,
            },
        ],
        // Wave 15: first ancient fragments + good potions
        11..=15 => vec![
            KothReward {
                item_id: "gold_coins".to_string(),
                quantity: 400,
            },
            KothReward {
                item_id: "ancient_fragment".to_string(),
                quantity: 2,
            },
            KothReward {
                item_id: "strong_health_potion".to_string(),
                quantity: 3,
            },
            KothReward {
                item_id: "strong_mana_potion".to_string(),
                quantity: 2,
            },
            KothReward {
                item_id: "uncut_ruby".to_string(),
                quantity: 1,
            },
        ],
        // Wave 20: solid fragment haul + top-tier supplies
        16..=20 => vec![
            KothReward {
                item_id: "gold_coins".to_string(),
                quantity: 750,
            },
            KothReward {
                item_id: "ancient_fragment".to_string(),
                quantity: 4,
            },
            KothReward {
                item_id: "strong_health_potion".to_string(),
                quantity: 5,
            },
            KothReward {
                item_id: "strong_mana_potion".to_string(),
                quantity: 3,
            },
            KothReward {
                item_id: "uncut_diamond".to_string(),
                quantity: 1,
            },
        ],
        // Wave 25+: endgame fragment farming
        _ => vec![
            KothReward {
                item_id: "gold_coins".to_string(),
                quantity: 1000,
            },
            KothReward {
                item_id: "ancient_fragment".to_string(),
                quantity: 6,
            },
            KothReward {
                item_id: "strong_health_potion".to_string(),
                quantity: 5,
            },
            KothReward {
                item_id: "strong_mana_potion".to_string(),
                quantity: 5,
            },
            KothReward {
                item_id: "strong_prayer_potion".to_string(),
                quantity: 2,
            },
            KothReward {
                item_id: "uncut_diamond".to_string(),
                quantity: 2,
            },
        ],
    }
}

fn consolation_rewards(waves_completed: u32) -> Vec<KothReward> {
    if waves_completed == 0 {
        return vec![];
    }
    let mut rewards = vec![KothReward {
        item_id: "gold_coins".to_string(),
        quantity: waves_completed * 15,
    }];
    // Give a health potion if they made it past wave 2
    if waves_completed >= 2 {
        rewards.push(KothReward {
            item_id: "health_potion".to_string(),
            quantity: 1,
        });
    }
    // Give an ancient fragment if they died past wave 10
    if waves_completed >= 10 {
        rewards.push(KothReward {
            item_id: "ancient_fragment".to_string(),
            quantity: 1,
        });
    }
    rewards
}

// ---------------------------------------------------------------------------
// KothState implementation
// ---------------------------------------------------------------------------

impl KothState {
    pub fn new(
        instance_id: String,
        player_id: String,
        map_width: u32,
        map_height: u32,
        current_time: u64,
        entrance_x: i32,
        entrance_y: i32,
    ) -> Self {
        Self {
            instance_id,
            player_id,
            phase: KothPhase::Countdown {
                ends_at: current_time + 3000,
            },
            current_wave: 1,
            points: 0,
            enemies_alive: 0,
            enemies_total: 0,
            spawn_queue: Vec::new(),
            last_spawn_time: 0,
            wave_start_time: 0,
            last_checkpoint_wave: 0,
            kills_this_wave: 0,
            map_width,
            map_height,
            entrance_x,
            entrance_y,
        }
    }

    /// Main tick - returns events for the GameRoom to process
    pub fn tick(&mut self, current_time: u64, player_level: i32) -> Vec<KothEvent> {
        let mut events = Vec::new();

        match self.phase.clone() {
            KothPhase::Countdown { ends_at } => {
                if current_time >= ends_at {
                    self.start_wave(current_time, player_level, &mut events);
                } else {
                    let remaining = (ends_at - current_time) as u32;
                    events.push(KothEvent::StateUpdate {
                        player_id: self.player_id.clone(),
                        phase: "countdown".to_string(),
                        wave: self.current_wave,
                        points: self.points,
                        enemies_alive: 0,
                        enemies_total: enemies_for_wave(self.current_wave),
                        countdown_ms: remaining,
                    });
                }
            }
            KothPhase::WaveActive => {
                // Staggered spawning: spawn a batch every 2 seconds
                if !self.spawn_queue.is_empty()
                    && current_time.saturating_sub(self.last_spawn_time) >= 2000
                {
                    let batch_size = 4.min(self.spawn_queue.len());
                    let batch: Vec<PendingSpawn> = self.spawn_queue.drain(..batch_size).collect();
                    self.last_spawn_time = current_time;

                    for spawn in batch {
                        let npc_id = format!(
                            "koth_{}_w{}_{}",
                            self.instance_id,
                            self.current_wave,
                            self.enemies_total - self.spawn_queue.len() as u32
                        );
                        events.push(KothEvent::SpawnNpc {
                            instance_id: self.instance_id.clone(),
                            npc_id,
                            prototype_id: spawn.prototype_id,
                            level: spawn.level,
                            x: spawn.x,
                            y: spawn.y,
                        });
                    }
                }

                // Send periodic state updates (every tick is fine, client will throttle display)
                events.push(KothEvent::StateUpdate {
                    player_id: self.player_id.clone(),
                    phase: "active".to_string(),
                    wave: self.current_wave,
                    points: self.points,
                    enemies_alive: self.enemies_alive,
                    enemies_total: self.enemies_total,
                    countdown_ms: 0,
                });
            }
            KothPhase::WaveComplete { ends_at } => {
                if current_time >= ends_at {
                    // Calculate wave completion points
                    self.points += wave_points(self.current_wave);
                    self.points += speed_bonus(self.wave_start_time, current_time);

                    if self.current_wave % 5 == 0 {
                        // Checkpoint
                        self.phase = KothPhase::Checkpoint;
                        let rewards = checkpoint_rewards(self.current_wave);
                        let next_count = enemies_for_wave(self.current_wave + 1);
                        events.push(KothEvent::CheckpointReached {
                            player_id: self.player_id.clone(),
                            wave: self.current_wave,
                            points: self.points,
                            rewards,
                            next_wave_enemy_count: next_count,
                        });
                    } else {
                        // Next wave countdown
                        self.current_wave += 1;
                        self.phase = KothPhase::Countdown {
                            ends_at: current_time + 3000,
                        };
                    }
                } else {
                    events.push(KothEvent::StateUpdate {
                        player_id: self.player_id.clone(),
                        phase: "wave_complete".to_string(),
                        wave: self.current_wave,
                        points: self.points,
                        enemies_alive: 0,
                        enemies_total: self.enemies_total,
                        countdown_ms: (ends_at - current_time) as u32,
                    });
                }
            }
            KothPhase::Checkpoint => {
                // Wait for player input (KothContinue or KothLeave)
                events.push(KothEvent::StateUpdate {
                    player_id: self.player_id.clone(),
                    phase: "checkpoint".to_string(),
                    wave: self.current_wave,
                    points: self.points,
                    enemies_alive: 0,
                    enemies_total: 0,
                    countdown_ms: 0,
                });
            }
            KothPhase::GameOver => {
                // Already handled
            }
        }

        events
    }

    /// Start a new wave
    fn start_wave(&mut self, current_time: u64, player_level: i32, events: &mut Vec<KothEvent>) {
        let count = enemies_for_wave(self.current_wave);
        let level = enemy_level_for_wave(self.current_wave, player_level);
        let prototype = enemy_prototype_for_wave(self.current_wave);
        let positions = generate_spawn_positions(self.map_width, self.map_height, count);

        self.enemies_alive = count;
        self.enemies_total = count;
        self.kills_this_wave = 0;
        self.wave_start_time = current_time;
        self.last_spawn_time = current_time;

        // Queue all spawns - first batch spawns immediately
        self.spawn_queue.clear();
        for (i, (x, y)) in positions.into_iter().enumerate() {
            self.spawn_queue.push(PendingSpawn {
                prototype_id: prototype.to_string(),
                level,
                x,
                y,
            });

            // Spawn first batch immediately
            if i < 4 {
                let spawn = self.spawn_queue.last().unwrap().clone();
                let npc_id = format!("koth_{}_w{}_{}", self.instance_id, self.current_wave, i);
                events.push(KothEvent::SpawnNpc {
                    instance_id: self.instance_id.clone(),
                    npc_id,
                    prototype_id: spawn.prototype_id,
                    level: spawn.level,
                    x: spawn.x,
                    y: spawn.y,
                });
            }
        }
        // Remove already-spawned from queue
        let spawned = 4.min(self.spawn_queue.len());
        self.spawn_queue.drain(..spawned);

        self.phase = KothPhase::WaveActive;

        events.push(KothEvent::StateUpdate {
            player_id: self.player_id.clone(),
            phase: "active".to_string(),
            wave: self.current_wave,
            points: self.points,
            enemies_alive: self.enemies_alive,
            enemies_total: self.enemies_total,
            countdown_ms: 0,
        });
    }

    /// Called when an NPC dies in this KOTH instance
    pub fn on_npc_killed(&mut self, current_time: u64) -> Vec<KothEvent> {
        let mut events = Vec::new();

        if self.phase != KothPhase::WaveActive {
            return events;
        }

        self.enemies_alive = self.enemies_alive.saturating_sub(1);
        self.kills_this_wave += 1;
        self.points += 5; // Kill bonus

        if self.enemies_alive == 0 && self.spawn_queue.is_empty() {
            // Wave complete
            self.phase = KothPhase::WaveComplete {
                ends_at: current_time + 2000,
            };
            events.push(KothEvent::StateUpdate {
                player_id: self.player_id.clone(),
                phase: "wave_complete".to_string(),
                wave: self.current_wave,
                points: self.points,
                enemies_alive: 0,
                enemies_total: self.enemies_total,
                countdown_ms: 2000,
            });
        }

        events
    }

    /// Player chose to continue at checkpoint
    pub fn on_continue(&mut self, current_time: u64) -> Vec<KothEvent> {
        if self.phase != KothPhase::Checkpoint {
            return vec![];
        }

        self.last_checkpoint_wave = self.current_wave;
        self.current_wave += 1;
        self.phase = KothPhase::Countdown {
            ends_at: current_time + 3000,
        };
        vec![]
    }

    /// Player chose to leave at checkpoint
    pub fn on_leave(&mut self) -> Vec<KothEvent> {
        let rewards = checkpoint_rewards(self.current_wave);
        self.phase = KothPhase::GameOver;

        vec![KothEvent::GameOver {
            player_id: self.player_id.clone(),
            instance_id: self.instance_id.clone(),
            waves_completed: self.current_wave,
            total_points: self.points,
            rewards,
            victory: true,
        }]
    }

    /// Player died during KOTH
    pub fn on_player_died(&mut self) -> Vec<KothEvent> {
        self.phase = KothPhase::GameOver;

        // Grant rewards from last checkpoint, or consolation
        let rewards = if self.last_checkpoint_wave > 0 {
            checkpoint_rewards(self.last_checkpoint_wave)
        } else {
            consolation_rewards(self.current_wave.saturating_sub(1))
        };

        vec![KothEvent::GameOver {
            player_id: self.player_id.clone(),
            instance_id: self.instance_id.clone(),
            waves_completed: self.current_wave.saturating_sub(1),
            total_points: self.points,
            rewards,
            victory: false,
        }]
    }

    /// Check if this KOTH session is over
    pub fn is_game_over(&self) -> bool {
        self.phase == KothPhase::GameOver
    }
}

/// Maps instance_id -> KothState for all active KOTH sessions
pub type KothStates = HashMap<String, KothState>;
