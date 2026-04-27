use super::item_registry::ItemRegistry;
use super::skills::Skills;
use crate::render::animation::{AnimationState, PlayerAnimation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Down = 0,
    Left = 1,
    Up = 2,
    Right = 3,
    DownLeft = 4,
    DownRight = 5,
    UpLeft = 6,
    UpRight = 7,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Down
    }
}

impl Direction {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Direction::Down,
            1 => Direction::Left,
            2 => Direction::Up,
            3 => Direction::Right,
            4 => Direction::DownLeft,
            5 => Direction::DownRight,
            6 => Direction::UpLeft,
            7 => Direction::UpRight,
            _ => Direction::Down,
        }
    }

    pub fn from_velocity(dx: f32, dy: f32) -> Self {
        if dx == 0.0 && dy == 0.0 {
            return Direction::Down;
        }
        // Cardinal only — pick the dominant axis, break ties with vertical
        if dx.abs() > dy.abs() {
            if dx > 0.0 {
                Direction::Right
            } else {
                Direction::Left
            }
        } else {
            if dy > 0.0 {
                Direction::Down
            } else {
                Direction::Up
            }
        }
    }

    /// Snap any diagonal to its nearest cardinal direction.
    pub fn to_cardinal(self) -> Self {
        match self {
            Direction::UpLeft | Direction::Up => Direction::Up,
            Direction::UpRight | Direction::Right => Direction::Right,
            Direction::DownRight | Direction::Down => Direction::Down,
            Direction::DownLeft | Direction::Left => Direction::Left,
        }
    }

    pub fn to_unit_vector(&self) -> (f32, f32) {
        match self {
            Direction::Down => (0.0, 1.0),
            Direction::Up => (0.0, -1.0),
            Direction::Left => (-1.0, 0.0),
            Direction::Right => (1.0, 0.0),
            Direction::DownLeft => (-1.0, 1.0),
            Direction::DownRight => (1.0, 1.0),
            Direction::UpLeft => (-1.0, -1.0),
            Direction::UpRight => (1.0, -1.0),
        }
    }
}

// Movement speed in tiles per second (must match server: 250ms per tile)
pub const TILES_PER_SECOND: f32 = 4.0;

// Linear interpolation speed - matches server movement rate
// Server: 250ms per tile = 4 tiles per second
const VISUAL_SPEED: f32 = 4.0;
// Threshold for considering visual position "at tile center"
const TILE_CENTER_THRESHOLD: f32 = 0.15;
// Ignore tiny/ambiguous per-frame displacement for facing updates.
const DIRECTION_FRAME_DELTA_EPS: f32 = 0.01;
const DIRECTION_AMBIGUITY_EPS: f32 = 0.01;

#[derive(Debug, Clone)]
pub struct Player {
    pub id: String,
    pub name: String,

    // Rendered position (smoothly interpolated each frame)
    pub x: f32,
    pub y: f32,
    pub z: f32,

    // Server-authoritative position (for local player reconciliation)
    pub server_x: f32,
    pub server_y: f32,
    pub server_z: f32,

    // Target for other players' interpolation
    pub target_x: f32,
    pub target_y: f32,
    pub target_z: f32,

    // Movement velocity (-1, 0, or 1 per axis, normalized for diagonal)
    pub vel_x: f32,
    pub vel_y: f32,

    pub direction: Direction,
    pub is_moving: bool,

    // Stats
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub skills: Skills,

    // Death state
    pub is_dead: bool,
    pub death_time: f64, // When the player died (game time)

    // Appearance
    pub gender: String,          // "male" or "female"
    pub skin: String,            // "tan", "pale", "brown", "purple", "orc", "ghost", "skeleton"
    pub hair_style: Option<i32>, // 0-2 (or None for bald)
    pub hair_color: Option<i32>, // 0-6 (color variant index)

    // Equipment
    pub equipped_head: Option<String>, // Item ID of equipped helmet/hat
    pub equipped_body: Option<String>, // Item ID of equipped body armor
    pub equipped_weapon: Option<String>, // Item ID of equipped weapon (sword/bow/etc)
    pub equipped_back: Option<String>, // Item ID of equipped back item (cape/quiver/etc)
    pub equipped_feet: Option<String>, // Item ID of equipped boots
    pub equipped_ring: Option<String>, // Item ID of equipped ring
    pub equipped_gloves: Option<String>, // Item ID of equipped gloves
    pub equipped_necklace: Option<String>, // Item ID of equipped necklace
    pub equipped_belt: Option<String>, // Item ID of equipped belt

    // Admin status
    pub is_admin: bool,

    // Animation
    pub animation: PlayerAnimation,

    // Last time this player took damage (for health bar visibility)
    pub last_damage_time: f64,

    // Gathering state (for fishing line rendering)
    pub is_gathering: bool,
    pub gathering_started_at: f64,

    // Woodcutting state
    pub is_woodcutting: bool,
    pub woodcutting_started_at: f64,
    pub last_woodcutting_anim: f64,

    // Mining state
    pub is_mining: bool,
    pub mining_started_at: f64,
    pub last_mining_anim: f64,

    /// Whether this player is currently in a dash slide
    pub is_dashing: bool,
    /// Whether this player has an active stall
    pub has_stall: bool,
    /// Name of the player's stall (if has_stall)
    pub stall_name: Option<String>,
    /// Combat style: "accurate", "aggressive", "defensive", "controlled"
    pub combat_style: String,
    /// True for the local player (set from state sync calls)
    is_local_player: bool,
    /// Last server tick this player appeared in a StateSync (for staleness detection)
    pub last_sync_tick: u64,
}

impl Player {
    pub fn new(id: String, name: String, x: f32, y: f32, gender: String, skin: String) -> Self {
        Self {
            id,
            name,
            x,
            y,
            z: 0.0,
            server_x: x,
            server_y: y,
            server_z: 0.0,
            target_x: x,
            target_y: y,
            target_z: 0.0,
            vel_x: 0.0,
            vel_y: 0.0,
            direction: Direction::Down,
            is_moving: false,
            hp: 10,
            max_hp: 10,
            mp: 50,
            max_mp: 50,
            skills: Skills::new(),
            is_dead: false,
            death_time: 0.0,
            gender,
            skin,
            hair_style: None,
            hair_color: None,
            equipped_head: None,
            equipped_body: None,
            equipped_weapon: None,
            equipped_back: None,
            equipped_feet: None,
            equipped_ring: None,
            equipped_gloves: None,
            equipped_necklace: None,
            equipped_belt: None,
            is_admin: false,
            animation: PlayerAnimation::new(),
            last_damage_time: 0.0,
            is_gathering: false,
            gathering_started_at: 0.0,
            is_woodcutting: false,
            woodcutting_started_at: 0.0,
            last_woodcutting_anim: 0.0,
            is_mining: false,
            mining_started_at: 0.0,
            last_mining_anim: 0.0,
            is_dashing: false,
            has_stall: false,
            stall_name: None,
            combat_style: "accurate".to_string(),
            is_local_player: false,
            last_sync_tick: 0,
        }
    }

    pub fn die(&mut self) {
        self.is_dead = true;
        self.death_time = macroquad::time::get_time();
        self.hp = 0;
    }

    /// Get the combat level (calculated from skills)
    pub fn combat_level(&self) -> i32 {
        self.skills.combat_level()
    }

    /// Get all equipped item IDs as an array for iteration
    fn all_equipped(&self) -> [&Option<String>; 9] {
        [
            &self.equipped_head,
            &self.equipped_body,
            &self.equipped_weapon,
            &self.equipped_back,
            &self.equipped_feet,
            &self.equipped_ring,
            &self.equipped_gloves,
            &self.equipped_necklace,
            &self.equipped_belt,
        ]
    }

    /// Calculate total attack bonus from all equipped items
    pub fn attack_bonus(&self, item_registry: &ItemRegistry) -> i32 {
        let mut bonus = 0;
        for equipped in self.all_equipped() {
            if let Some(item_id) = equipped {
                let def = item_registry.get_or_placeholder(item_id);
                if let Some(equip) = &def.equipment {
                    bonus += equip.attack_bonus;
                }
            }
        }
        bonus
    }

    /// Calculate total strength bonus from all equipped items
    pub fn strength_bonus(&self, item_registry: &ItemRegistry) -> i32 {
        let mut bonus = 0;
        for equipped in self.all_equipped() {
            if let Some(item_id) = equipped {
                let def = item_registry.get_or_placeholder(item_id);
                if let Some(equip) = &def.equipment {
                    bonus += equip.strength_bonus;
                }
            }
        }
        bonus
    }

    /// Calculate total defence bonus from all equipped items
    pub fn defence_bonus(&self, item_registry: &ItemRegistry) -> i32 {
        let mut bonus = 0;
        for equipped in self.all_equipped() {
            if let Some(item_id) = equipped {
                let def = item_registry.get_or_placeholder(item_id);
                if let Some(equip) = &def.equipment {
                    bonus += equip.defence_bonus;
                }
            }
        }
        bonus
    }

    /// Calculate total magic bonus from all equipped items
    pub fn magic_bonus(&self, item_registry: &ItemRegistry) -> i32 {
        let mut bonus = 0;
        for equipped in self.all_equipped() {
            if let Some(item_id) = equipped {
                let def = item_registry.get_or_placeholder(item_id);
                if let Some(equip) = &def.equipment {
                    bonus += equip.magic_bonus;
                }
            }
        }
        bonus
    }

    /// Calculate total ranged strength bonus from all equipped items
    pub fn ranged_strength_bonus(&self, item_registry: &ItemRegistry) -> i32 {
        let mut bonus = 0;
        for equipped in self.all_equipped() {
            if let Some(item_id) = equipped {
                let def = item_registry.get_or_placeholder(item_id);
                if let Some(equip) = &def.equipment {
                    bonus += equip.ranged_strength_bonus;
                }
            }
        }
        bonus
    }

    pub fn respawn(&mut self, x: f32, y: f32, hp: i32) {
        self.is_dead = false;
        self.death_time = 0.0;
        self.hp = hp;
        self.max_hp = hp;
        self.x = x;
        self.y = y;
        self.z = 0.0;
        self.server_x = x;
        self.server_y = y;
        self.server_z = 0.0;
        self.target_x = x;
        self.target_y = y;
        self.target_z = 0.0;
        self.vel_x = 0.0;
        self.vel_y = 0.0;
        // Reset animation to standing (in case player died while sitting)
        self.animation.state = AnimationState::Idle;
    }

    /// Set server-authoritative position (convenience method for stopped state)
    pub fn set_server_position(&mut self, new_x: f32, new_y: f32) {
        self.set_server_state(
            new_x,
            new_y,
            self.server_z as i32,
            0.0,
            0.0,
            self.direction,
            false,
            false,
        );
    }

    /// Set server state - strict tile-center-to-tile-center model.
    /// Target is ALWAYS a tile center. Direction changes queue until arrival.
    pub fn set_server_state(
        &mut self,
        x: f32,
        y: f32,
        z: i32,
        vel_x: f32,
        vel_y: f32,
        dir: Direction,
        is_local_player: bool,
        has_pending_local_moves: bool,
    ) {
        let old_server_x = self.server_x;
        let old_server_y = self.server_y;
        self.is_local_player = is_local_player;

        self.server_x = x;
        self.server_y = y;
        self.server_z = z as f32;
        self.target_z = z as f32;
        self.vel_x = vel_x;
        self.vel_y = vel_y;

        let step_dx = x - old_server_x;
        let step_dy = y - old_server_y;
        let server_moved = step_dx.abs() > 0.01 || step_dy.abs() > 0.01;

        // Remote players always follow server facing.
        // Local player keeps input-facing unless sitting.
        let is_sitting = matches!(
            self.animation.state,
            crate::render::animation::AnimationState::SittingChair
                | crate::render::animation::AnimationState::SittingGround
        );
        if !is_local_player || is_sitting {
            self.direction = dir;
        } else if server_moved {
            let in_attack = matches!(
                self.animation.state,
                crate::render::animation::AnimationState::Attacking
                    | crate::render::animation::AnimationState::Casting
                    | crate::render::animation::AnimationState::ShootingBow
            );
            if in_attack {
                self.direction = dir;
            } else {
                let adx = step_dx.abs();
                let ady = step_dy.abs();
                if (adx - ady).abs() > DIRECTION_AMBIGUITY_EPS {
                    self.direction = Direction::from_velocity(step_dx, step_dy);
                }
            }
        }

        // Dashing: slide to target without prediction
        if self.is_dashing {
            self.target_x = x;
            self.target_y = y;
            self.vel_x = 0.0;
            self.vel_y = 0.0;
            return;
        }

        // Teleport detection: snap if too far away
        let dist = ((self.x - x).powi(2) + (self.y - y).powi(2)).sqrt();
        if dist > 3.0 {
            self.x = x;
            self.y = y;
            self.target_x = x;
            self.target_y = y;
            return;
        }

        // ── Strict tile-center model ──
        // Target is always a tile center (integer coords).
        // Only predict from server position — never re-predict from a
        // predicted-ahead tile, which would cause wrong-direction glitches
        // on direction changes.
        let stopped = vel_x == 0.0 && vel_y == 0.0;
        let at_server_pos = (self.x - x).abs() < TILE_CENTER_THRESHOLD
            && (self.y - y).abs() < TILE_CENTER_THRESHOLD;

        if stopped || (is_local_player && !has_pending_local_moves) {
            // No movement intent: target = server position
            self.target_x = x;
            self.target_y = y;
        } else if server_moved || at_server_pos {
            // Server confirmed a tile step, or visual is at server pos
            // (e.g. first move from standstill). Predict next tile center.
            self.target_x = x + vel_x;
            self.target_y = y + vel_y;
        }
        // else: mid-interpolation or at predicted tile waiting for server
        // confirmation. Keep current target.
    }

    /// Smooth visual interpolation toward target position
    /// Uses axis-aligned movement to stay grid-true and avoid diagonal artifacts.
    pub fn interpolate_visual(&mut self, delta: f32) {
        let dx = self.target_x - self.x;
        let dy = self.target_y - self.y;
        let old_x = self.x;
        let old_y = self.y;

        // Z interpolation
        let dz = self.target_z - self.z;
        if dz.abs() < 0.01 {
            self.z = self.target_z;
        } else {
            // Base speed 8 blocks/sec; falling scales with drop distance
            let z_speed = if dz < 0.0 {
                8.0 * dz.abs().max(1.0)
            } else {
                8.0
            };
            let z_step = z_speed * delta;
            if z_step >= dz.abs() {
                self.z = self.target_z;
            } else {
                self.z += dz.signum() * z_step;
            }
        }

        if dx.abs() < 0.01 && dy.abs() < 0.01 {
            self.x = self.target_x;
            self.y = self.target_y;
            // Keep walking animation alive while server indicates movement,
            // even during the brief positional pause waiting for the next
            // server confirmation. Prevents walk→idle flicker at tile centers.
            let has_vel = self.vel_x != 0.0 || self.vel_y != 0.0;
            self.is_moving = has_vel;
            self.is_dashing = false; // Dash slide complete
        } else {
            // Use fast speed during dash (normal movement remains 4 tiles/sec).
            let speed = if self.is_dashing { 16.0 } else { VISUAL_SPEED };
            let mut budget = speed * delta;

            // Resolve smaller displacement axis first, then the larger axis.
            if dx.abs() <= dy.abs() {
                if dx.abs() > 0.01 {
                    let step = budget.min(dx.abs());
                    self.x += dx.signum() * step;
                    budget -= step;
                }
                if budget > 0.01 && dy.abs() > 0.01 {
                    let step = budget.min(dy.abs());
                    self.y += dy.signum() * step;
                }
            } else {
                if dy.abs() > 0.01 {
                    let step = budget.min(dy.abs());
                    self.y += dy.signum() * step;
                    budget -= step;
                }
                if budget > 0.01 && dx.abs() > 0.01 {
                    let step = budget.min(dx.abs());
                    self.x += dx.signum() * step;
                }
            }
            self.is_moving = true;
        }

        // Compute movement direction from actual frame displacement
        let moved_dx = self.x - old_x;
        let moved_dy = self.y - old_y;
        let abs_dx = moved_dx.abs();
        let abs_dy = moved_dy.abs();
        let movement_dir = if abs_dx.max(abs_dy) > DIRECTION_FRAME_DELTA_EPS
            && (abs_dx - abs_dy).abs() > DIRECTION_AMBIGUITY_EPS
        {
            Some(Direction::from_velocity(moved_dx, moved_dy))
        } else {
            None
        };

        self.update_animation(delta, movement_dir);
    }

    /// Update animation state and frame
    /// Only syncs animation direction when movement aligns (prevents moonwalking)
    fn update_animation(&mut self, delta: f32, movement_dir: Option<Direction>) {
        // During dash, freeze on walking frame 1 (mid-stride)
        if self.is_dashing {
            self.animation.state = AnimationState::Walking;
            self.animation.frame = 1.0;
            self.animation.direction = self.direction;
            return;
        }

        // Handle action animations (attack, cast, etc) - they take priority
        let in_action = self.animation.state == AnimationState::Attacking
            || self.animation.state == AnimationState::Casting
            || self.animation.state == AnimationState::ShootingBow;

        if in_action {
            self.animation.update(delta);
            if self.animation.is_finished() {
                if self.is_moving {
                    self.animation.set_state(AnimationState::Walking);
                } else {
                    self.animation.set_state(AnimationState::Idle);
                }
            }
            return;
        }

        // Local animation facing follows actual rendered movement to avoid
        // moonwalk during prediction; logical facing still commits from
        // authoritative server tile steps in set_server_state().
        if self.is_moving {
            if self.is_local_player {
                self.animation.direction = movement_dir.unwrap_or(self.direction);
            } else if let Some(move_dir) = movement_dir {
                // Keep logical facing in sync with actual movement so idle/attack
                // frames don't snap back to a stale pre-turn direction.
                self.direction = move_dir;
                self.animation.direction = move_dir;
            } else {
                self.animation.direction = self.direction;
            }
        } else {
            self.animation.direction = self.direction;
        }

        // Handle movement animations
        if self.is_moving {
            self.animation.set_state(AnimationState::Walking);
            self.animation.update(delta);
        } else {
            // Only go to idle if not in a sitting state
            if self.animation.state != AnimationState::SittingGround
                && self.animation.state != AnimationState::SittingChair
            {
                self.animation.set_state(AnimationState::Idle);
            }
        }
    }

    /// Trigger attack animation
    pub fn play_attack(&mut self) {
        // Sync animation direction to player direction before attacking
        // This ensures attack faces the current intended direction, even if
        // the player just changed direction while standing still
        self.animation.direction = self.direction;
        self.animation.set_state(AnimationState::Attacking);
    }

    /// Trigger spell casting animation
    pub fn play_cast(&mut self) {
        self.animation.direction = self.direction;
        self.animation.set_state(AnimationState::Casting);
    }

    /// Trigger bow shooting animation
    pub fn play_shoot_bow(&mut self) {
        self.animation.direction = self.direction;
        self.animation.set_state(AnimationState::ShootingBow);
    }

    /// Sit on ground
    pub fn sit_ground(&mut self) {
        self.animation.set_state(AnimationState::SittingGround);
    }

    /// Sit on chair
    pub fn sit_chair(&mut self) {
        self.animation.set_state(AnimationState::SittingChair);
    }

    /// Stand up from sitting - move to tile in front of chair
    pub fn stand_up(&mut self) {
        self.animation.set_state(AnimationState::Idle);
        let (dx, dy) = match self.direction {
            Direction::Up => (0.0, -1.0),
            Direction::Down => (0.0, 1.0),
            Direction::Left => (-1.0, 0.0),
            Direction::Right => (1.0, 0.0),
            _ => (0.0, 0.0),
        };
        self.x += dx;
        self.y += dy;
        self.target_x = self.x;
        self.target_y = self.y;
    }

    /// Smooth interpolation toward target position
    pub fn update(&mut self, delta: f32) {
        self.interpolate_visual(delta);
    }
}
