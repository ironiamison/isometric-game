use crate::game::Direction;
use rand::Rng;
use serde::Serialize;
use std::collections::HashSet;

// ============================================================================
// Level Scaling
// ============================================================================

/// Scale NPC HP based on level (10% increase per level above 1)
fn scale_hp(base_hp: i32, level: i32) -> i32 {
    let multiplier = 1.0 + 0.10 * (level - 1).max(0) as f64;
    (base_hp as f64 * multiplier).round() as i32
}

/// Scale NPC damage based on level (15% increase per level above 1)
fn scale_damage(base_damage: i32, level: i32) -> i32 {
    let multiplier = 1.0 + 0.15 * (level - 1).max(0) as f64;
    (base_damage as f64 * multiplier).round() as i32
}

// ============================================================================
// NPC State
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcState {
    Idle,      // 0
    Chasing,   // 1
    Attacking, // 2
    Returning, // 3
    Dead,      // 4
    Wandering,  // 5 - added at end to preserve existing state values
    Submerging, // 6
    Emerging,   // 7
}

// ============================================================================
// NPC Entity
// ============================================================================

/// Stats from prototype used for AI behavior
#[derive(Debug, Clone)]
pub struct PrototypeStats {
    pub display_name: String,
    pub sprite: String,
    pub damage: i32,
    pub attack_bonus: i32,
    pub defence_bonus: i32,
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
    pub is_banker: bool,
    pub is_slayer_master: bool,
    pub is_friendly: bool,
    pub is_port_master: bool,
    pub wander_enabled: bool,
    pub wander_radius: i32,
    pub wander_pause_min_ms: u64,
    pub wander_pause_max_ms: u64,
    pub no_shadow: bool,
    pub render_offset_y: f32,
    pub hp_regen_percent_per_sec: f32,
    pub station_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Npc {
    pub id: String,
    /// Reference to entity prototype ID (e.g., "pig", "elder_villager")
    pub prototype_id: String,
    /// Stats from prototype
    pub stats: PrototypeStats,
    // Grid position (server authoritative)
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub spawn_x: i32,
    pub spawn_y: i32,
    pub direction: Direction,
    pub spawn_direction: Direction,
    pub hp: i32,
    pub max_hp: i32,
    pub level: i32,
    pub state: NpcState,
    pub target_id: Option<String>, // Player it's aggro'd on
    pub last_attack_time: u64,
    pub last_move_time: u64, // For grid movement cooldown
    pub death_time: u64,     // When the NPC died (for respawn)
    /// Set to true on the tick when this NPC attacks, for client animation sync
    pub just_attacked: bool,
    /// Target position for wandering
    pub wander_target: Option<(i32, i32)>,
    /// Timestamp until which the NPC should remain idle before wandering
    pub idle_until: u64,
    /// Last time HP regen was applied
    pub last_regen_time: u64,
    /// Speech bubble config (None = NPC never speaks)
    pub speech_messages: Option<Vec<String>>,
    pub speech_radius: i32,
    pub speech_interval_min_ms: u64,
    pub speech_interval_max_ms: u64,
    /// Timestamp when this NPC should next speak
    pub next_speech_at: u64,
    /// Best distance achieved toward current wander target (for stuck detection)
    wander_best_distance: i32,
    /// Number of move attempts without getting closer to wander target
    wander_stuck_count: u8,
    /// Boss mechanic: when true, NPC cannot take damage
    pub invulnerable: bool,
    /// When true, NPC is hidden from state sync (e.g. boss underground)
    pub hidden: bool,
}

impl Npc {
    /// Create an NPC from an entity prototype
    /// `facing_override` takes priority over the prototype's behaviors.facing
    pub fn from_prototype(
        id: &str,
        prototype_id: &str,
        prototype: &crate::entity::EntityPrototype,
        x: i32,
        y: i32,
        level: i32,
        facing_override: Option<&str>,
    ) -> Self {
        let stats = PrototypeStats {
            display_name: prototype.display_name.clone(),
            sprite: prototype.sprite.clone(),
            damage: scale_damage(prototype.stats.damage, level),
            attack_bonus: prototype.stats.attack_bonus,
            defence_bonus: prototype.stats.defence_bonus,
            attack_range: prototype.stats.attack_range,
            aggro_range: prototype.stats.aggro_range,
            chase_range: prototype.stats.chase_range,
            move_cooldown_ms: prototype.stats.move_cooldown_ms,
            attack_cooldown_ms: prototype.stats.attack_cooldown_ms,
            respawn_time_ms: prototype.stats.respawn_time_ms,
            exp_base: prototype.rewards.exp_base,
            hostile: prototype.behaviors.hostile,
            is_quest_giver: prototype.behaviors.quest_giver,
            is_merchant: prototype.behaviors.merchant,
            is_altar: prototype.behaviors.altar,
            is_banker: prototype.behaviors.banker,
            is_slayer_master: prototype.behaviors.slayer_master,
            is_friendly: prototype.behaviors.friendly,
            is_port_master: prototype.behaviors.port_master,
            wander_enabled: prototype.behaviors.wander_enabled,
            wander_radius: prototype.behaviors.wander_radius,
            wander_pause_min_ms: prototype.behaviors.wander_pause_min_ms,
            wander_pause_max_ms: prototype.behaviors.wander_pause_max_ms,
            hp_regen_percent_per_sec: prototype.stats.hp_regen_percent_per_sec,
            no_shadow: prototype.behaviors.no_shadow,
            render_offset_y: prototype.behaviors.render_offset_y,
            station_type: prototype.behaviors.station_type.clone(),
        };

        Self {
            id: id.to_string(),
            prototype_id: prototype_id.to_string(),
            x,
            y,
            z: 0,
            spawn_x: x,
            spawn_y: y,
            direction: facing_override
                .or(prototype.behaviors.facing.as_deref())
                .map(Direction::from_str)
                .unwrap_or(Direction::Down),
            spawn_direction: facing_override
                .or(prototype.behaviors.facing.as_deref())
                .map(Direction::from_str)
                .unwrap_or(Direction::Down),
            hp: scale_hp(prototype.stats.max_hp, level),
            max_hp: scale_hp(prototype.stats.max_hp, level),
            level,
            state: NpcState::Idle,
            target_id: None,
            last_attack_time: 0,
            last_move_time: 0,
            death_time: 0,
            just_attacked: false,
            wander_target: None,
            idle_until: 0,
            last_regen_time: 0,
            speech_messages: prototype.speech.as_ref().map(|s| s.messages.clone()),
            speech_radius: prototype.speech.as_ref().map(|s| s.radius).unwrap_or(0),
            speech_interval_min_ms: prototype
                .speech
                .as_ref()
                .map(|s| s.interval_min_ms)
                .unwrap_or(15000),
            speech_interval_max_ms: prototype
                .speech
                .as_ref()
                .map(|s| s.interval_max_ms)
                .unwrap_or(45000),
            next_speech_at: 0,
            wander_best_distance: i32::MAX,
            wander_stuck_count: 0,
            invulnerable: false,
            hidden: false,
            stats,
        }
    }

    // Helper methods to get stats
    fn get_damage(&self) -> i32 {
        self.stats.damage
    }

    fn get_attack_range(&self) -> i32 {
        self.stats.attack_range
    }

    fn get_aggro_range(&self) -> i32 {
        self.stats.aggro_range
    }

    fn get_chase_range(&self) -> i32 {
        self.stats.chase_range
    }

    fn get_move_cooldown_ms(&self) -> u64 {
        self.stats.move_cooldown_ms
    }

    fn get_attack_cooldown_ms(&self) -> u64 {
        self.stats.attack_cooldown_ms
    }

    pub fn get_respawn_time_ms(&self) -> u64 {
        self.stats.respawn_time_ms
    }

    pub fn is_hostile(&self) -> bool {
        self.stats.hostile
    }

    pub fn is_quest_giver(&self) -> bool {
        self.stats.is_quest_giver
    }

    pub fn is_merchant(&self) -> bool {
        self.stats.is_merchant
    }

    pub fn is_altar(&self) -> bool {
        self.stats.is_altar
    }

    pub fn is_banker(&self) -> bool {
        self.stats.is_banker
    }

    pub fn is_slayer_master(&self) -> bool {
        self.stats.is_slayer_master
    }

    pub fn is_friendly(&self) -> bool {
        self.stats.is_friendly
    }

    pub fn is_port_master(&self) -> bool {
        self.stats.is_port_master
    }

    pub fn station_type(&self) -> Option<&str> {
        self.stats.station_type.as_deref()
    }

    /// Returns true if this NPC can be attacked by players.
    /// Friendly NPCs, quest givers, merchants, altars, bankers, and stations cannot be attacked.
    pub fn is_attackable(&self) -> bool {
        !self.stats.is_friendly
            && !self.stats.is_quest_giver
            && !self.stats.is_merchant
            && !self.stats.is_altar
            && !self.stats.is_banker
            && !self.stats.is_slayer_master
            && !self.stats.is_port_master
            && self.stats.station_type.is_none()
    }

    pub fn name(&self) -> String {
        format!("{} Lv.{}", self.stats.display_name, self.level)
    }

    pub fn exp_reward(&self) -> i32 {
        // Scale EXP by NPC level
        self.stats.exp_base * self.level
    }

    pub fn is_alive(&self) -> bool {
        self.state != NpcState::Dead
    }

    /// Take damage and return true if the NPC died.
    /// If attacker_id is provided and NPC survives, it will start chasing the attacker.
    pub fn take_damage(
        &mut self,
        damage: i32,
        current_time: u64,
        attacker_id: Option<&str>,
    ) -> bool {
        if self.invulnerable {
            return false;
        }
        self.hp = (self.hp - damage).max(0);
        if self.hp <= 0 {
            self.state = NpcState::Dead;
            self.death_time = current_time;
            self.target_id = None;
            true
        } else {
            // Being damaged should immediately interrupt current behavior and chase attacker.
            if self.is_attackable() {
                if let Some(attacker) = attacker_id {
                    self.target_id = Some(attacker.to_string());
                    self.state = NpcState::Chasing;
                    self.wander_target = None;
                    self.wander_best_distance = i32::MAX;
                    self.wander_stuck_count = 0;
                }
            }
            false
        }
    }

    /// Check if ready to respawn (respawn_time_ms == 0 means no respawn)
    pub fn ready_to_respawn(&self, current_time: u64) -> bool {
        if self.state != NpcState::Dead {
            return false;
        }
        let respawn_ms = self.get_respawn_time_ms();
        if respawn_ms == 0 {
            return false;
        }
        current_time - self.death_time >= respawn_ms
    }

    /// Respawn the NPC at its spawn point
    pub fn respawn(&mut self) {
        self.x = self.spawn_x;
        self.y = self.spawn_y;
        self.direction = self.spawn_direction;
        self.hp = self.max_hp;
        self.state = NpcState::Idle;
        self.target_id = None;
        self.last_attack_time = 0;
        self.last_move_time = 0;
        self.wander_target = None;
        self.idle_until = 0;
        self.last_regen_time = 0;
    }

    /// Apply passive HP regeneration based on prototype stats
    pub fn apply_regen(&mut self, current_time: u64) {
        const REGEN_INTERVAL_MS: u64 = 30000;
        if self.state == NpcState::Dead {
            return;
        }
        // First tick after spawn/respawn - just initialize timer, don't regen yet
        if self.last_regen_time == 0 {
            self.last_regen_time = current_time;
            return;
        }
        if current_time - self.last_regen_time >= REGEN_INTERVAL_MS {
            self.last_regen_time = current_time;
            if self.hp < self.max_hp && self.hp > 0 {
                let regen = ((self.max_hp as f32 * self.stats.hp_regen_percent_per_sec) / 100.0)
                    .ceil()
                    .max(1.0) as i32;
                self.hp = (self.hp + regen).min(self.max_hp);
            }
        }
    }

    /// Pick a random wander target within radius of spawn point
    fn pick_wander_target(&self, walkable_check: &dyn Fn(i32, i32) -> bool) -> (i32, i32) {
        let mut rng = rand::thread_rng();
        let radius = self.stats.wander_radius;
        // Try a few times to find a walkable target
        for _ in 0..8 {
            let dx = rng.gen_range(-radius..=radius);
            let dy = rng.gen_range(-radius..=radius);
            let tx = self.spawn_x + dx;
            let ty = self.spawn_y + dy;
            if walkable_check(tx, ty) {
                return (tx, ty);
            }
        }
        // Fallback to current position (will be rejected as same-position in caller)
        (self.x, self.y)
    }

    /// Set a random idle pause duration
    fn set_random_idle_pause(&mut self, current_time: u64) {
        let mut rng = rand::thread_rng();
        let pause = rng.gen_range(self.stats.wander_pause_min_ms..=self.stats.wander_pause_max_ms);
        self.idle_until = current_time + pause;
    }

    /// Calculate grid distance (Chebyshev - allows diagonal)
    fn grid_distance(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
        (x1 - x2).abs().max((y1 - y2).abs())
    }

    /// Check if target is within attack range AND in a cardinal direction (not diagonal)
    fn is_in_attack_range(x1: i32, y1: i32, x2: i32, y2: i32, range: i32) -> bool {
        let dx = (x1 - x2).abs();
        let dy = (y1 - y2).abs();
        // Must be cardinal (one axis is 0) and within range
        (dx == 0 || dy == 0) && (dx + dy) <= range
    }

    /// Try to move one tile toward target position (grid-based)
    /// Returns true if moved
    fn try_move_toward(
        &mut self,
        target_x: i32,
        target_y: i32,
        current_time: u64,
        occupied_tiles: &HashSet<(i32, i32)>,
        walkable_check: &dyn Fn(i32, i32) -> bool,
        height_check: &dyn Fn(i32, i32) -> i32,
    ) -> bool {
        // Check movement cooldown
        if current_time - self.last_move_time < self.get_move_cooldown_ms() {
            return false;
        }

        let dx = target_x - self.x;
        let dy = target_y - self.y;

        // Already at target
        if dx == 0 && dy == 0 {
            return false;
        }

        // Build list of candidate moves in priority order
        let mut candidates: Vec<(i32, i32)> = Vec::with_capacity(4);

        // Primary direction: move along axis with greater distance
        if dx.abs() > dy.abs() {
            candidates.push((dx.signum(), 0)); // Primary: horizontal
            if dy != 0 {
                candidates.push((0, dy.signum())); // Secondary: vertical toward target
            }
        } else if dy.abs() > dx.abs() {
            candidates.push((0, dy.signum())); // Primary: vertical
            if dx != 0 {
                candidates.push((dx.signum(), 0)); // Secondary: horizontal toward target
            }
        } else {
            // Equal distance on both axes - try both
            if dx != 0 {
                candidates.push((dx.signum(), 0));
            }
            if dy != 0 {
                candidates.push((0, dy.signum()));
            }
        }

        // Add perpendicular moves as last resort (to go around obstacles)
        // These don't move us closer but allow pathfinding around blockers
        if dy == 0 && dx != 0 {
            // Moving horizontally, try vertical sidesteps
            candidates.push((0, 1));
            candidates.push((0, -1));
        } else if dx == 0 && dy != 0 {
            // Moving vertically, try horizontal sidesteps
            candidates.push((1, 0));
            candidates.push((-1, 0));
        }

        // Try each candidate move
        for (move_x, move_y) in candidates {
            let new_x = self.x + move_x;
            let new_y = self.y + move_y;

            // Check if tile is walkable (collision check)
            if !walkable_check(new_x, new_y) {
                continue; // Tile has collision
            }

            // Check height difference - NPCs can auto-step up 1 block but not more
            let target_height = height_check(new_x, new_y);
            let height_diff = target_height - self.z;
            if height_diff > 1 {
                continue; // Too high to step up
            }

            // Check if tile is occupied by another NPC or player
            if occupied_tiles.contains(&(new_x, new_y)) {
                continue; // Try next candidate
            }

            // Found a valid move
            self.x = new_x;
            self.y = new_y;
            self.z = target_height;
            self.last_move_time = current_time;

            // Update facing direction
            self.direction = crate::game::Direction::from_velocity(move_x as f32, move_y as f32);

            return true;
        }

        false // All moves blocked
    }

    /// Update NPC AI state and movement
    /// Returns Some((target_id, damage)) if the NPC attacks a player
    pub fn update(
        &mut self,
        _delta: f32,                               // Not used for grid movement
        players: &[(String, i32, i32, i32)],       // (id, x, y, hp) - grid positions
        other_npc_positions: &HashSet<(i32, i32)>, // positions of other NPCs and players (excluding self)
        current_time: u64,
        walkable_check: &dyn Fn(i32, i32) -> bool,
        height_check: &dyn Fn(i32, i32) -> i32,
    ) -> Option<(String, i32)> {
        // Reset attack flag each tick - will be set to true if we attack this tick
        self.just_attacked = false;

        if self.state == NpcState::Dead {
            return None;
        }

        let mut attack_result = None;

        match self.state {
            NpcState::Idle => {
                // Hostile NPCs look for players in aggro range
                if self.is_hostile() {
                    let aggro_range = self.get_aggro_range();
                    if aggro_range > 0 {
                        let mut nearest: Option<(String, i32)> = None;
                        for (player_id, px, py, hp) in players {
                            if *hp <= 0 {
                                continue; // Skip dead players
                            }
                            let dist = Self::grid_distance(self.x, self.y, *px, *py);
                            if dist <= aggro_range {
                                if nearest.is_none() || dist < nearest.as_ref().unwrap().1 {
                                    nearest = Some((player_id.clone(), dist));
                                }
                            }
                        }

                        if let Some((target_id, _)) = nearest {
                            self.target_id = Some(target_id);
                            self.state = NpcState::Chasing;
                            return None; // State changed, skip wandering check
                        }
                    }
                }

                // Check if wandering is enabled and idle pause has elapsed
                if self.stats.wander_enabled && current_time >= self.idle_until {
                    let target = self.pick_wander_target(walkable_check);
                    // Only wander if target is different from current position
                    if target.0 != self.x || target.1 != self.y {
                        self.wander_target = Some(target);
                        self.wander_best_distance =
                            Self::grid_distance(self.x, self.y, target.0, target.1);
                        self.wander_stuck_count = 0;
                        self.state = NpcState::Wandering;
                    } else {
                        // Pick another pause if we'd wander to same spot
                        self.set_random_idle_pause(current_time);
                    }
                }
            }

            NpcState::Wandering => {
                // Hostile NPCs interrupt wandering immediately if player in aggro range
                if self.is_hostile() {
                    let aggro_range = self.get_aggro_range();
                    if aggro_range > 0 {
                        for (player_id, px, py, hp) in players {
                            if *hp <= 0 {
                                continue;
                            }
                            let dist = Self::grid_distance(self.x, self.y, *px, *py);
                            if dist <= aggro_range {
                                self.target_id = Some(player_id.clone());
                                self.state = NpcState::Chasing;
                                self.wander_target = None;
                                return None;
                            }
                        }
                    }
                }

                // Move toward wander target
                if let Some((tx, ty)) = self.wander_target {
                    if self.x == tx && self.y == ty {
                        // Reached target, go idle with random pause
                        self.state = NpcState::Idle;
                        self.wander_target = None;
                        self.wander_best_distance = i32::MAX;
                        self.wander_stuck_count = 0;
                        self.set_random_idle_pause(current_time);
                    } else {
                        let moved = self.try_move_toward(
                            tx,
                            ty,
                            current_time,
                            other_npc_positions,
                            walkable_check,
                            height_check,
                        );

                        if moved {
                            // Check if we made progress toward target
                            let current_dist = Self::grid_distance(self.x, self.y, tx, ty);
                            if current_dist < self.wander_best_distance {
                                // Making progress - reset stuck counter
                                self.wander_best_distance = current_dist;
                                self.wander_stuck_count = 0;
                            } else {
                                // Moved but didn't get closer (sidestep or backtrack)
                                self.wander_stuck_count += 1;

                                // If stuck for too many moves, abandon this target
                                if self.wander_stuck_count >= 4 {
                                    self.state = NpcState::Idle;
                                    self.wander_target = None;
                                    self.wander_best_distance = i32::MAX;
                                    self.wander_stuck_count = 0;
                                    self.set_random_idle_pause(current_time);
                                }
                            }
                        }
                    }
                } else {
                    // No target, go idle
                    self.state = NpcState::Idle;
                    self.set_random_idle_pause(current_time);
                }
            }

            NpcState::Chasing => {
                // Check if target is still valid
                let target_pos = self.target_id.as_ref().and_then(|tid| {
                    players
                        .iter()
                        .find(|(id, _, _, hp)| id == tid && *hp > 0)
                        .map(|(_, x, y, _)| (*x, *y))
                });

                if let Some((tx, ty)) = target_pos {
                    let spawn_dist =
                        Self::grid_distance(self.x, self.y, self.spawn_x, self.spawn_y);
                    let target_dist = Self::grid_distance(self.x, self.y, tx, ty);
                    let movement_done =
                        current_time - self.last_move_time >= self.get_move_cooldown_ms();

                    if spawn_dist > self.get_chase_range() || target_dist > self.get_chase_range() {
                        // Too far from spawn OR target got too far away, return home
                        self.state = NpcState::Returning;
                        self.target_id = None;
                    } else if Self::is_in_attack_range(
                        self.x,
                        self.y,
                        tx,
                        ty,
                        self.get_attack_range(),
                    ) && movement_done
                    {
                        // In attack range (cardinal direction only) and movement completed
                        self.state = NpcState::Attacking;
                    } else if !Self::is_in_attack_range(
                        self.x,
                        self.y,
                        tx,
                        ty,
                        self.get_attack_range(),
                    ) {
                        // Not in range, move toward target (one tile at a time)
                        self.try_move_toward(
                            tx,
                            ty,
                            current_time,
                            other_npc_positions,
                            walkable_check,
                            height_check,
                        );
                    }
                    // If in range but movement not done, stay in Chasing and wait
                } else {
                    // Target lost (died or disconnected)
                    self.state = NpcState::Returning;
                    self.target_id = None;
                }
            }

            NpcState::Attacking => {
                // Check if target is still in range
                let target_info = self.target_id.as_ref().and_then(|tid| {
                    players
                        .iter()
                        .find(|(id, _, _, hp)| id == tid && *hp > 0)
                        .map(|(id, x, y, _)| (id.clone(), *x, *y))
                });

                if let Some((target_id, tx, ty)) = target_info {
                    if !Self::is_in_attack_range(self.x, self.y, tx, ty, self.get_attack_range()) {
                        // Target moved out of range or not in cardinal direction, chase again
                        self.state = NpcState::Chasing;
                    } else {
                        // Face target
                        let dx = tx - self.x;
                        let dy = ty - self.y;
                        if dx != 0 || dy != 0 {
                            self.direction =
                                crate::game::Direction::from_velocity(dx as f32, dy as f32);
                        }

                        // Attack only if movement has completed (movement cooldown expired)
                        // and attack cooldown is ready
                        let movement_done =
                            current_time - self.last_move_time >= self.get_move_cooldown_ms();
                        let attack_ready =
                            current_time - self.last_attack_time >= self.get_attack_cooldown_ms();

                        if movement_done && attack_ready {
                            self.last_attack_time = current_time;
                            self.just_attacked = true; // Signal client to play animation
                            attack_result = Some((target_id, self.get_damage()));
                        }
                    }
                } else {
                    // Target lost
                    self.state = NpcState::Returning;
                    self.target_id = None;
                }
            }

            NpcState::Returning => {
                let dist = Self::grid_distance(self.x, self.y, self.spawn_x, self.spawn_y);

                if dist == 0 {
                    // Reached spawn, go idle (HP regenerates passively over time)
                    self.state = NpcState::Idle;
                    self.wander_target = None;
                    // Set idle pause before wandering again
                    if self.stats.wander_enabled {
                        self.set_random_idle_pause(current_time);
                    }
                } else {
                    // Move toward spawn (one tile at a time)
                    self.try_move_toward(
                        self.spawn_x,
                        self.spawn_y,
                        current_time,
                        other_npc_positions,
                        walkable_check,
                        height_check,
                    );
                }
            }

            NpcState::Dead => {}
            NpcState::Submerging | NpcState::Emerging => {
                // Handled by boss_tick; no generic AI behavior
            }
        }

        attack_result
    }
}

// ============================================================================
// NPC Update for Network Sync
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NpcUpdate {
    pub id: String,
    /// Entity prototype ID (e.g., "pig", "elder_villager") for client-side lookup
    pub entity_type: String,
    /// NPC prototype ID used by quest definitions (giver_npc)
    pub prototype_id: String,
    /// Display name to show above NPC
    pub display_name: String,
    pub x: i32, // Grid position
    pub y: i32, // Grid position
    pub z: i32, // Height position
    pub direction: u8,
    pub hp: i32,
    pub max_hp: i32,
    pub level: i32,
    pub state: u8,
    /// Whether this NPC is hostile
    pub hostile: bool,
    /// Whether this NPC offers quests
    pub is_quest_giver: bool,
    /// True when this NPC currently has a quest ready to turn in for the receiving player
    pub can_turn_in_quest: bool,
    /// Whether this NPC is a merchant
    pub is_merchant: bool,
    /// Whether this NPC is an altar
    pub is_altar: bool,
    /// Whether this NPC is a banker
    pub is_banker: bool,
    /// Whether this NPC is a slayer master
    pub is_slayer_master: bool,
    /// Whether this NPC is friendly (non-attackable, no level shown)
    pub is_friendly: bool,
    /// Whether this NPC is a port master (travel services)
    pub is_port_master: bool,
    /// Movement speed in tiles per second (for client interpolation)
    pub move_speed: f32,
    /// True only on the tick when this NPC attacks (for animation sync)
    pub just_attacked: bool,
    /// Whether to hide the shadow under this NPC
    pub no_shadow: bool,
    /// Vertical pixel offset for rendering (positive = down)
    pub render_offset_y: f32,
    /// Station type (e.g. "furnace", "anvil") if this NPC is a crafting station
    pub station_type: Option<String>,
}

impl From<&Npc> for NpcUpdate {
    fn from(npc: &Npc) -> Self {
        // Convert move_cooldown_ms to tiles per second
        // e.g., 500ms per tile = 2.0 tiles/sec, 250ms = 4.0 tiles/sec
        let move_speed = if npc.stats.move_cooldown_ms > 0 {
            1000.0 / npc.stats.move_cooldown_ms as f32
        } else {
            0.0 // Non-moving NPCs (villagers, etc.)
        };

        Self {
            id: npc.id.clone(),
            entity_type: npc.stats.sprite.clone(),
            prototype_id: npc.prototype_id.clone(),
            display_name: npc.stats.display_name.clone(),
            x: npc.x,
            y: npc.y,
            z: npc.z,
            direction: npc.direction as u8,
            hp: npc.hp,
            max_hp: npc.max_hp,
            level: npc.level,
            state: npc.state as u8,
            hostile: npc.is_hostile(),
            is_quest_giver: npc.is_quest_giver(),
            can_turn_in_quest: false,
            is_merchant: npc.is_merchant(),
            is_altar: npc.is_altar(),
            is_banker: npc.is_banker(),
            is_slayer_master: npc.is_slayer_master(),
            is_friendly: npc.is_friendly(),
            is_port_master: npc.is_port_master(),
            move_speed,
            just_attacked: npc.just_attacked,
            no_shadow: npc.stats.no_shadow,
            render_offset_y: npc.stats.render_offset_y,
            station_type: npc.station_type().map(|s| s.to_string()),
        }
    }
}
