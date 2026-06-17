//! The Reaper boss — a soul-economy fight.
//!
//! Unlike the Wurm (hit-and-hide, dodge telegraphed AoE) the Reaper is always
//! present and always vulnerable, but it HEALS whenever it claims a soul:
//!   * a Mark of Death that expires before the player reaches a Soul Ward, and
//!   * a Soul Wraith (spawned by a failed mark) that reaches the boss.
//! Deny souls -> the Reaper dies. Feed it souls -> the fight never ends.
//!
//! State that needs world access (player/NPC positions) is resolved by the
//! boss tick pipeline: `plan(..)` takes position snapshots and returns a list
//! of [`ReaperAction`]s for the pipeline to apply after dropping the lock.

use std::collections::{HashMap, HashSet};

use crate::boss::{BossEvent, BossPhase};

// ---------------------------------------------------------------------------
// Tunables (v1 — Phase 1 vertical slice)
// ---------------------------------------------------------------------------

/// How long the marked player has to reach a Soul Ward.
const MARK_TIMER_MS: u64 = 6_000;
/// How often all live wraiths advance one tile toward the boss.
/// Matches the client's soul drift speed (~1.67 tiles/sec) for a smooth glide.
const WRAITH_MOVE_MS: u64 = 600;
/// Hard cap on concurrent wraiths so a wiped group can't snowball forever.
const MAX_WRAITHS: u32 = 6;
/// Damage dealt to a player whose Mark of Death expires uncleansed.
const FAIL_DAMAGE: i32 = 20;

/// Arena walkable bounds for the 30x30 Oakshore_reaper_Boss map.
const ARENA_MIN: i32 = 2;
const ARENA_MAX: i32 = 27;

/// Candidate Soul Ward centres (sub-quadrant + centre of the arena). The ward
/// is placed at whichever candidate is furthest from the boss, pulling the
/// marked player out of melee — that's the core tension of the fight.
const WARD_SPOTS: [(i32, i32); 5] = [(8, 8), (8, 21), (21, 8), (21, 21), (14, 14)];

/// Mark cadence per phase (escalates as the Reaper loses HP).
fn mark_interval(phase: &BossPhase) -> u64 {
    match phase {
        BossPhase::Hunt => 12_000,
        BossPhase::Storm => 10_000,
        BossPhase::Frenzy => 8_000,
    }
}

fn clamp_arena(x: i32, y: i32) -> (i32, i32) {
    (x.clamp(ARENA_MIN, ARENA_MAX), y.clamp(ARENA_MIN, ARENA_MAX))
}

fn chebyshev(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    (ax - bx).abs().max((ay - by).abs())
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ReaperState {
    Active,
    Dead,
}

/// An active Mark of Death.
#[derive(Debug, Clone)]
struct Mark {
    player_id: String,
    /// The 3x3 Soul Ward the player must stand in to cleanse.
    ward_tiles: Vec<(i32, i32)>,
    expires_at: u64,
}

/// Side effects the boss tick pipeline must apply (it has player/NPC access).
#[derive(Debug)]
pub enum ReaperAction {
    /// Tell the client to display a Mark of Death on a player.
    Mark { player_id: String, duration_ms: u64 },
    /// Show the green "stand here" Soul Ward zone.
    SoulWard { tiles: Vec<(i32, i32)> },
    /// Remove a player's mark indicator (cleansed or resolved).
    ClearMark { player_id: String },
    /// A mark expired uncleansed: damage the player and tear loose a wraith.
    /// The boss only heals if that wraith later reaches it (ConsumeWraith).
    FailMark {
        player_id: String,
        damage: i32,
        /// (npc_id, x, y) of the wraith to spawn, if under the cap.
        wraith: Option<(String, i32, i32)>,
    },
    /// Advance a wraith one tile toward the boss.
    MoveWraith { npc_id: String, x: i32, y: i32 },
    /// A wraith reached the boss: despawn it and heal the boss.
    ConsumeWraith { npc_id: String, heal: i32 },
    /// System chat announcement.
    Announce { message: String },
}

pub struct ReaperBossState {
    pub instance_id: String,
    pub boss_npc_id: String,
    pub phase: BossPhase,
    pub state: ReaperState,
    pub boss_hp: i32,
    pub boss_max_hp: i32,
    pub boss_x: i32,
    pub boss_y: i32,
    pub map_width: i32,
    pub map_height: i32,
    pub player_ids: Vec<String>,
    pub death_time: u64,
    pub countdown_sent: u8,
    pub damage_dealers: HashSet<String>,

    mark: Option<Mark>,
    last_mark_time: u64,
    mark_counter: u32,
    wraith_counter: u32,
    live_wraith_count: u32,
    last_wraith_move_time: u64,
}

impl ReaperBossState {
    #[allow(clippy::too_many_arguments)]
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
            state: ReaperState::Active,
            boss_hp,
            boss_max_hp,
            boss_x,
            boss_y,
            map_width,
            map_height,
            player_ids: Vec::new(),
            death_time: 0,
            countdown_sent: 0,
            damage_dealers: HashSet::new(),
            mark: None,
            // Stagger the first mark so players get a moment to engage.
            last_mark_time: current_time,
            mark_counter: 0,
            wraith_counter: 0,
            live_wraith_count: 0,
            last_wraith_move_time: current_time,
        }
    }

    /// Lightweight per-tick update: phase from HP + a state broadcast.
    /// The soul mechanics are handled by [`Self::plan`] (needs world access).
    pub fn tick(&mut self, _current_time: u64) -> Vec<BossEvent> {
        let mut events = Vec::new();
        if self.state == ReaperState::Dead {
            return events;
        }
        self.update_phase();

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
            wurm_state: "active".to_string(),
        });
        events
    }

    /// Plan mark/wraith/heal side effects using live position snapshots.
    /// `alive_players` and `wraiths` are `(id, x, y)`.
    pub fn plan(
        &mut self,
        current_time: u64,
        alive_players: &[(String, i32, i32)],
        wraiths: &[(String, i32, i32)],
    ) -> Vec<ReaperAction> {
        let mut actions = Vec::new();
        if self.state == ReaperState::Dead {
            return actions;
        }

        // --- Resolve an expiring mark first ---
        if let Some(mark) = self.mark.clone() {
            if current_time >= mark.expires_at {
                self.mark = None;
                actions.push(ReaperAction::ClearMark {
                    player_id: mark.player_id.clone(),
                });

                match alive_players.iter().find(|(id, _, _)| *id == mark.player_id) {
                    Some((_, px, py)) if mark.ward_tiles.contains(&(*px, *py)) => {
                        actions.push(ReaperAction::Announce {
                            message: "A marked soul slips free of the Reaper's grasp.".to_string(),
                        });
                    }
                    Some((_, px, py)) => {
                        // Failed: damage, heal the boss, and tear loose a wraith.
                        let wraith = if self.live_wraith_count < MAX_WRAITHS {
                            self.wraith_counter += 1;
                            self.live_wraith_count += 1;
                            // Encode the source player after "::" so the client can
                            // render the wraith as a ghostly copy of that player.
                            let id = format!(
                                "reaper_wraith_{}_{}::{}",
                                self.instance_id, self.wraith_counter, mark.player_id
                            );
                            Some((id, *px, *py))
                        } else {
                            None
                        };
                        actions.push(ReaperAction::FailMark {
                            player_id: mark.player_id.clone(),
                            damage: FAIL_DAMAGE,
                            wraith,
                        });
                        actions.push(ReaperAction::Announce {
                            message: "The Reaper claims a soul!".to_string(),
                        });
                    }
                    // Player left/died before resolution — no consequence.
                    None => {}
                }
            }
        }

        // --- Schedule a new mark ---
        if self.mark.is_none()
            && current_time.saturating_sub(self.last_mark_time) >= mark_interval(&self.phase)
            && !alive_players.is_empty()
        {
            self.last_mark_time = current_time;
            self.mark_counter += 1;

            // Mark the player furthest from the boss (forces a long run).
            let target = alive_players
                .iter()
                .max_by_key(|(_, x, y)| chebyshev(*x, *y, self.boss_x, self.boss_y))
                .cloned();

            if let Some((player_id, _, _)) = target {
                let ward_tiles = self.pick_ward();
                self.mark = Some(Mark {
                    player_id: player_id.clone(),
                    ward_tiles: ward_tiles.clone(),
                    expires_at: current_time + MARK_TIMER_MS,
                });
                actions.push(ReaperAction::Mark {
                    player_id,
                    duration_ms: MARK_TIMER_MS,
                });
                actions.push(ReaperAction::SoulWard { tiles: ward_tiles });
            }
        }

        // --- Advance wraiths toward the boss ---
        if !wraiths.is_empty()
            && current_time.saturating_sub(self.last_wraith_move_time) >= WRAITH_MOVE_MS
        {
            self.last_wraith_move_time = current_time;
            for (id, wx, wy) in wraiths {
                if chebyshev(*wx, *wy, self.boss_x, self.boss_y) <= 1 {
                    self.live_wraith_count = self.live_wraith_count.saturating_sub(1);
                    actions.push(ReaperAction::ConsumeWraith {
                        npc_id: id.clone(),
                        heal: self.pct_hp(4),
                    });
                } else {
                    let dx = self.boss_x - wx;
                    let dy = self.boss_y - wy;
                    let (sx, sy) = if dx.abs() >= dy.abs() {
                        (dx.signum(), 0)
                    } else {
                        (0, dy.signum())
                    };
                    let (nx, ny) = clamp_arena(wx + sx, wy + sy);
                    actions.push(ReaperAction::MoveWraith {
                        npc_id: id.clone(),
                        x: nx,
                        y: ny,
                    });
                }
            }
        }

        actions
    }

    /// Choose the ward furthest from the boss, varied per mark to avoid repeats.
    fn pick_ward(&self) -> Vec<(i32, i32)> {
        let mut spots = WARD_SPOTS;
        // Rotate the candidate list by the mark counter so ties break differently.
        let rot = (self.mark_counter as usize) % spots.len();
        spots.rotate_left(rot);
        let (cx, cy) = spots
            .iter()
            .max_by_key(|(x, y)| chebyshev(*x, *y, self.boss_x, self.boss_y))
            .copied()
            .unwrap_or((14, 14));

        let mut tiles = Vec::with_capacity(9);
        for dy in -1..=1 {
            for dx in -1..=1 {
                tiles.push(clamp_arena(cx + dx, cy + dy));
            }
        }
        tiles
    }

    fn pct_hp(&self, pct: i32) -> i32 {
        ((self.boss_max_hp * pct) / 100).max(1)
    }

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

    pub fn on_wraith_died(&mut self) {
        self.live_wraith_count = self.live_wraith_count.saturating_sub(1);
    }

    pub fn add_player(&mut self, player_id: String) {
        if !self.player_ids.contains(&player_id) {
            self.player_ids.push(player_id);
        }
    }

    pub fn remove_player(&mut self, player_id: &str) {
        self.player_ids.retain(|id| id != player_id);
    }

    pub fn is_dead(&self) -> bool {
        self.state == ReaperState::Dead
    }
}

/// Maps instance_id -> ReaperBossState for all active reaper fights.
pub type ReaperBossStates = HashMap<String, ReaperBossState>;
