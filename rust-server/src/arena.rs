use serde::Serialize;
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Zone bounds
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ZoneBounds {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl ZoneBounds {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }
}

// ---------------------------------------------------------------------------
// Ring configuration — one per physical arena zone on the map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RingConfig {
    pub name: String,
    pub max_players: usize,
    pub ring_zone: ZoneBounds,
    pub spectator_zone: ZoneBounds,
    pub ring_spawn_points: Vec<(i32, i32)>,
    pub spectator_spawn: (i32, i32),
}

// ---------------------------------------------------------------------------
// Arena configuration — shared queue zone + multiple rings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ArenaConfig {
    pub map_id: String,
    pub entry_fee: i32,
    pub countdown_duration_ms: u64,
    pub results_duration_ms: u64,
    pub queue_zone: ZoneBounds,
    /// Rings sorted by max_players ascending — the server picks the smallest
    /// ring that fits the queue.
    pub rings: Vec<RingConfig>,
}

impl Default for ArenaConfig {
    fn default() -> Self {
        Self {
            map_id: "duel_arena".to_string(),
            entry_fee: 50,
            countdown_duration_ms: 10_000,
            results_duration_ms: 10_000,
            // Queue / waiting area
            queue_zone: ZoneBounds {
                min_x: 13,
                min_y: 2,
                max_x: 30,
                max_y: 13,
            },
            rings: vec![
                // Small 1v1 ring (3,3 - 11,11)
                RingConfig {
                    name: "1v1 Ring".to_string(),
                    max_players: 2,
                    ring_zone: ZoneBounds {
                        min_x: 3,
                        min_y: 3,
                        max_x: 11,
                        max_y: 11,
                    },
                    spectator_zone: ZoneBounds {
                        min_x: 3,
                        min_y: 12,
                        max_x: 11,
                        max_y: 13,
                    },
                    ring_spawn_points: vec![(5, 5), (9, 9)],
                    spectator_spawn: (7, 12),
                },
                // Larger FFA ring (15,15 - 29,29)
                RingConfig {
                    name: "FFA Ring".to_string(),
                    max_players: 8,
                    ring_zone: ZoneBounds {
                        min_x: 15,
                        min_y: 15,
                        max_x: 29,
                        max_y: 29,
                    },
                    spectator_zone: ZoneBounds {
                        min_x: 13,
                        min_y: 14,
                        max_x: 30,
                        max_y: 14,
                    },
                    ring_spawn_points: vec![
                        (17, 19),
                        (27, 19),
                        (22, 17),
                        (22, 27),
                        (17, 17),
                        (27, 17),
                        (17, 27),
                        (27, 27),
                    ],
                    spectator_spawn: (22, 14),
                },
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// Arena state machine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ArenaState {
    Idle,
    Countdown { ends_at: u64 },
    Fighting,
    Results { ends_at: u64 },
}

// ---------------------------------------------------------------------------
// Events emitted by the arena for the GameRoom to act on
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub enum ArenaEvent {
    StateChanged { state: String },
    FightStarted { fighters: Vec<(String, (i32, i32))> },
    MatchEnded { placements: Vec<ArenaPlacement> },
    ResultsExpired,
}

// ---------------------------------------------------------------------------
// Match results
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ArenaPlacement {
    pub rank: u32,
    pub player_id: String,
    pub player_name: String,
    pub kills: i32,
    pub gold_reward: i32,
}

// ---------------------------------------------------------------------------
// Per-match statistics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct MatchStats {
    pub kills: HashMap<String, i32>,
    pub death_order: Vec<String>,
    pub fighter_names: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// ArenaManager
// ---------------------------------------------------------------------------

pub struct ArenaManager {
    pub state: ArenaState,
    pub config: ArenaConfig,
    pub queued_players: Vec<String>,
    pub active_fighters: Vec<String>,
    pub spectators: Vec<String>,
    pub escrow: HashMap<String, i32>,
    pub match_stats: MatchStats,
    pub all_arena_players: Vec<String>,
    /// Index into config.rings for the ring selected for the current match
    pub active_ring_index: Option<usize>,
    /// Players rejected from queue (e.g. insufficient gold) - prevents spam
    pub queue_rejected: HashSet<String>,
}

impl ArenaManager {
    pub fn new(config: ArenaConfig) -> Self {
        Self {
            state: ArenaState::Idle,
            config,
            queued_players: Vec::new(),
            active_fighters: Vec::new(),
            spectators: Vec::new(),
            escrow: HashMap::new(),
            match_stats: MatchStats::default(),
            all_arena_players: Vec::new(),
            active_ring_index: None,
            queue_rejected: HashSet::new(),
        }
    }

    /// Get the currently active ring config (only valid during Countdown/Fighting/Results)
    pub fn active_ring(&self) -> Option<&RingConfig> {
        self.active_ring_index
            .and_then(|i| self.config.rings.get(i))
    }

    /// Pick the best ring for a given player count.
    /// Selects the smallest ring whose max_players >= count.
    /// Falls back to the largest ring if all are too small.
    fn select_ring(&self, player_count: usize) -> usize {
        // Rings are sorted by max_players ascending in the default config.
        // Find the first ring that fits.
        for (i, ring) in self.config.rings.iter().enumerate() {
            if player_count <= ring.max_players {
                return i;
            }
        }
        // Fallback: largest ring (last one)
        self.config.rings.len().saturating_sub(1)
    }

    // -----------------------------------------------------------------------
    // Tick
    // -----------------------------------------------------------------------

    pub fn tick(&mut self, current_time: u64) -> Vec<ArenaEvent> {
        let mut events = Vec::new();

        match self.state.clone() {
            ArenaState::Countdown { ends_at } => {
                if current_time >= ends_at {
                    let spawn_assignments = self.start_fight();
                    events.push(ArenaEvent::StateChanged {
                        state: "fighting".to_string(),
                    });
                    events.push(ArenaEvent::FightStarted {
                        fighters: spawn_assignments,
                    });
                }
            }
            ArenaState::Results { ends_at } => {
                if current_time >= ends_at {
                    self.reset_to_idle();
                    events.push(ArenaEvent::ResultsExpired);
                    events.push(ArenaEvent::StateChanged {
                        state: "idle".to_string(),
                    });
                }
            }
            _ => {}
        }

        events
    }

    // -----------------------------------------------------------------------
    // Queue management
    // -----------------------------------------------------------------------

    pub fn queue_player(
        &mut self,
        player_id: &str,
        player_name: &str,
        gold: i32,
    ) -> Result<(), String> {
        if self.state != ArenaState::Idle {
            return Err("Arena is not accepting new players right now.".to_string());
        }

        if self.queued_players.contains(&player_id.to_string()) {
            return Err("You are already in the queue.".to_string());
        }

        if gold < self.config.entry_fee {
            return Err(format!(
                "Not enough gold. Entry fee is {} but you only have {}.",
                self.config.entry_fee, gold
            ));
        }

        self.queued_players.push(player_id.to_string());
        self.match_stats
            .fighter_names
            .insert(player_id.to_string(), player_name.to_string());

        if !self.all_arena_players.contains(&player_id.to_string()) {
            self.all_arena_players.push(player_id.to_string());
        }

        Ok(())
    }

    pub fn dequeue_player(&mut self, player_id: &str) -> Option<i32> {
        self.queued_players.retain(|id| id != player_id);
        self.all_arena_players.retain(|id| id != player_id);
        self.match_stats.fighter_names.remove(player_id);
        self.escrow.remove(player_id)
    }

    // -----------------------------------------------------------------------
    // State transitions
    // -----------------------------------------------------------------------

    pub fn start_countdown(
        &mut self,
        current_time: u64,
        custom_duration_ms: Option<u64>,
    ) -> Result<Vec<(String, i32)>, String> {
        if self.state != ArenaState::Idle {
            return Err("Arena is not idle.".to_string());
        }
        if self.queued_players.len() < 2 {
            return Err("Need at least 2 players to start.".to_string());
        }

        if self.config.rings.is_empty() {
            return Err("No rings configured.".to_string());
        }

        // Select ring based on queue size
        self.active_ring_index = Some(self.select_ring(self.queued_players.len()));

        let ring = &self.config.rings[self.active_ring_index.unwrap()];
        tracing::info!(
            "Arena: selected '{}' for {} players (max {})",
            ring.name,
            self.queued_players.len(),
            ring.max_players
        );

        let duration = custom_duration_ms.unwrap_or(self.config.countdown_duration_ms);
        let ends_at = current_time + duration;

        let mut charges: Vec<(String, i32)> = Vec::new();
        for pid in &self.queued_players {
            self.escrow.insert(pid.clone(), self.config.entry_fee);
            charges.push((pid.clone(), self.config.entry_fee));
        }

        self.state = ArenaState::Countdown { ends_at };
        Ok(charges)
    }

    pub fn start_fight(&mut self) -> Vec<(String, (i32, i32))> {
        self.active_fighters = self.queued_players.clone();
        self.queued_players.clear();

        self.match_stats.kills.clear();
        self.match_stats.death_order.clear();
        for pid in &self.active_fighters {
            self.match_stats.kills.insert(pid.clone(), 0);
        }

        let ring = &self.config.rings[self.active_ring_index.unwrap_or(0)];
        let spawns = &ring.ring_spawn_points;
        let assignments: Vec<(String, (i32, i32))> = self
            .active_fighters
            .iter()
            .enumerate()
            .map(|(i, pid)| {
                let point = spawns[i % spawns.len()];
                (pid.clone(), point)
            })
            .collect();

        self.state = ArenaState::Fighting;
        assignments
    }

    // -----------------------------------------------------------------------
    // Combat tracking
    // -----------------------------------------------------------------------

    pub fn on_player_death(&mut self, player_id: &str, killer_id: Option<&str>) {
        if let Some(kid) = killer_id {
            *self.match_stats.kills.entry(kid.to_string()).or_insert(0) += 1;
        }

        if !self
            .match_stats
            .death_order
            .contains(&player_id.to_string())
        {
            self.match_stats.death_order.push(player_id.to_string());
        }

        self.active_fighters.retain(|id| id != player_id);
        if !self.spectators.contains(&player_id.to_string()) {
            self.spectators.push(player_id.to_string());
        }
    }

    pub fn check_match_end(&self) -> bool {
        self.active_fighters.len() <= 1
    }

    pub fn end_match(&mut self, current_time: u64) -> Vec<ArenaPlacement> {
        let total_pot: i32 = self.escrow.values().sum();
        let num_entrants = self.escrow.len();

        let mut ranked_ids: Vec<String> = Vec::new();

        if let Some(winner) = self.active_fighters.first() {
            ranked_ids.push(winner.clone());
        }

        for pid in self.match_stats.death_order.iter().rev() {
            ranked_ids.push(pid.clone());
        }

        let payouts = Self::calculate_payouts(total_pot, num_entrants, ranked_ids.len());

        let placements: Vec<ArenaPlacement> = ranked_ids
            .iter()
            .enumerate()
            .map(|(i, pid)| {
                let name = self
                    .match_stats
                    .fighter_names
                    .get(pid)
                    .cloned()
                    .unwrap_or_else(|| "Unknown".to_string());
                let kills = self.match_stats.kills.get(pid).copied().unwrap_or(0);
                let reward = payouts.get(i).copied().unwrap_or(0);

                ArenaPlacement {
                    rank: (i + 1) as u32,
                    player_id: pid.clone(),
                    player_name: name,
                    kills,
                    gold_reward: reward,
                }
            })
            .collect();

        self.state = ArenaState::Results {
            ends_at: current_time + self.config.results_duration_ms,
        };

        self.escrow.clear();
        placements
    }

    pub fn cancel(&mut self) -> Vec<(String, i32)> {
        let refunds: Vec<(String, i32)> = self.escrow.drain().collect();
        self.reset_to_idle();
        refunds
    }

    // -----------------------------------------------------------------------
    // Disconnection handling
    // -----------------------------------------------------------------------

    pub fn on_player_disconnect(&mut self, player_id: &str) -> Option<(String, Option<String>)> {
        let was_fighting = self.active_fighters.contains(&player_id.to_string());

        self.queued_players.retain(|id| id != player_id);
        self.spectators.retain(|id| id != player_id);
        self.all_arena_players.retain(|id| id != player_id);

        if was_fighting {
            self.on_player_death(player_id, None);
            Some((player_id.to_string(), None))
        } else {
            self.escrow.remove(player_id);
            None
        }
    }

    // -----------------------------------------------------------------------
    // Zone / state queries
    // -----------------------------------------------------------------------

    pub fn is_in_queue_zone(&self, x: i32, y: i32) -> bool {
        self.config.queue_zone.contains(x, y)
    }

    pub fn is_in_ring(&self, player_id: &str) -> bool {
        self.active_fighters.contains(&player_id.to_string())
    }

    /// Get the ring zone bounds for the active match (for movement blocking)
    pub fn active_ring_zone(&self) -> Option<&ZoneBounds> {
        self.active_ring().map(|r| &r.ring_zone)
    }

    /// Get the spectator spawn for the active ring
    pub fn active_spectator_spawn(&self) -> (i32, i32) {
        self.active_ring()
            .map(|r| r.spectator_spawn)
            .unwrap_or((16, 4))
    }

    pub fn is_fighting(&self) -> bool {
        matches!(self.state, ArenaState::Fighting)
    }

    pub fn set_entry_fee(&mut self, fee: i32) {
        self.config.entry_fee = fee;
    }

    #[allow(dead_code)]
    pub fn state_name(&self) -> &'static str {
        match self.state {
            ArenaState::Idle => "idle",
            ArenaState::Countdown { .. } => "countdown",
            ArenaState::Fighting => "fighting",
            ArenaState::Results { .. } => "results",
        }
    }

    pub fn get_status_text(&self) -> String {
        match &self.state {
            ArenaState::Idle => {
                let queued = self.queued_players.len();
                format!(
                    "Arena is idle. {} player{} queued. Entry fee: {} gold.",
                    queued,
                    if queued == 1 { "" } else { "s" },
                    self.config.entry_fee
                )
            }
            ArenaState::Countdown { ends_at } => {
                let ring_name = self
                    .active_ring()
                    .map(|r| r.name.as_str())
                    .unwrap_or("unknown");
                format!(
                    "Fight starting in {}! {} fighters → {}.",
                    ends_at,
                    self.queued_players.len(),
                    ring_name
                )
            }
            ArenaState::Fighting => {
                let ring_name = self
                    .active_ring()
                    .map(|r| r.name.as_str())
                    .unwrap_or("unknown");
                format!(
                    "Fight in progress ({})! {} fighter{} remaining.",
                    ring_name,
                    self.active_fighters.len(),
                    if self.active_fighters.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                )
            }
            ArenaState::Results { .. } => "Match complete. Results on display.".to_string(),
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn reset_to_idle(&mut self) {
        self.state = ArenaState::Idle;
        self.queued_players.clear();
        self.active_fighters.clear();
        self.spectators.clear();
        self.escrow.clear();
        self.match_stats = MatchStats::default();
        self.all_arena_players.clear();
        self.active_ring_index = None;
    }

    fn calculate_payouts(total_pot: i32, num_entrants: usize, num_ranked: usize) -> Vec<i32> {
        if num_ranked == 0 {
            return Vec::new();
        }

        if num_entrants <= 2 {
            let mut payouts = vec![total_pot];
            for _ in 1..num_ranked {
                payouts.push(0);
            }
            return payouts;
        }

        // 3+ players: 60% / 25% / 15%
        let first = (total_pot as f64 * 0.60).floor() as i32;
        let second = (total_pot as f64 * 0.25).floor() as i32;
        let third = (total_pot as f64 * 0.15).floor() as i32;
        let remainder = total_pot - first - second - third;
        let first = first + remainder;

        let mut payouts = Vec::with_capacity(num_ranked);
        for i in 0..num_ranked {
            match i {
                0 => payouts.push(first),
                1 => payouts.push(second),
                2 => payouts.push(third),
                _ => payouts.push(0),
            }
        }
        payouts
    }
}
