use serde::{Deserialize, Serialize};
use crate::render::animation::{PlayerAnimation, AnimationState};
use super::skills::Skills;
use super::item_registry::ItemRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Down = 0,
    Left = 1,
    Up = 2,
    Right = 3,
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
            _ => Direction::Down,
        }
    }
}

impl Direction {
    pub fn from_velocity(dx: f32, dy: f32) -> Self {
        if dx == 0.0 && dy == 0.0 {
            return Direction::Down;
        }

        // 4 quadrants: vertical takes priority when |dy| > |dx|
        if dy.abs() > dx.abs() {
            if dy < 0.0 { Direction::Up } else { Direction::Down }
        } else {
            if dx < 0.0 { Direction::Left } else { Direction::Right }
        }
    }

    pub fn to_unit_vector(&self) -> (f32, f32) {
        match self {
            Direction::Down => (0.0, 1.0),
            Direction::Up => (0.0, -1.0),
            Direction::Left => (-1.0, 0.0),
            Direction::Right => (1.0, 0.0),
        }
    }
}

// Movement speed in tiles per second (must match server: 250ms per tile)
pub const TILES_PER_SECOND: f32 = 4.0;

// Linear interpolation speed - must match server movement rate
// Server: 250ms per tile = 4 tiles per second
const VISUAL_SPEED: f32 = 4.0;

#[derive(Debug, Clone)]
pub struct Player {
    pub id: String,
    pub name: String,

    // Rendered position (smoothly interpolated each frame)
    pub x: f32,
    pub y: f32,

    // Server-authoritative position (for local player reconciliation)
    pub server_x: f32,
    pub server_y: f32,

    // Target for other players' interpolation
    pub target_x: f32,
    pub target_y: f32,

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
    pub gender: String, // "male" or "female"
    pub skin: String,   // "tan", "pale", "brown", "purple", "orc", "ghost", "skeleton"
    pub hair_style: Option<i32>, // 0-2 (or None for bald)
    pub hair_color: Option<i32>, // 0-6 (color variant index)

    // Equipment
    pub equipped_head: Option<String>,   // Item ID of equipped helmet/hat
    pub equipped_body: Option<String>,   // Item ID of equipped body armor
    pub equipped_weapon: Option<String>, // Item ID of equipped weapon (sword/bow/etc)
    pub equipped_back: Option<String>,   // Item ID of equipped back item (cape/quiver/etc)
    pub equipped_feet: Option<String>,   // Item ID of equipped boots
    pub equipped_ring: Option<String>,   // Item ID of equipped ring
    pub equipped_gloves: Option<String>, // Item ID of equipped gloves
    pub equipped_necklace: Option<String>, // Item ID of equipped necklace
    pub equipped_belt: Option<String>,   // Item ID of equipped belt

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

    /// Whether this player is currently in a dash slide
    pub is_dashing: bool,
}

impl Player {
    pub fn new(id: String, name: String, x: f32, y: f32, gender: String, skin: String) -> Self {
        Self {
            id,
            name,
            x,
            y,
            server_x: x,
            server_y: y,
            target_x: x,
            target_y: y,
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
            is_dashing: false,
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

    pub fn respawn(&mut self, x: f32, y: f32, hp: i32) {
        self.is_dead = false;
        self.death_time = 0.0;
        self.hp = hp;
        self.max_hp = hp;
        self.x = x;
        self.y = y;
        self.server_x = x;
        self.server_y = y;
        self.target_x = x;
        self.target_y = y;
        // Reset animation to standing (in case player died while sitting)
        self.animation.state = AnimationState::Idle;
    }

    /// Set server-authoritative position (convenience method for stopped state)
    pub fn set_server_position(&mut self, new_x: f32, new_y: f32) {
        self.set_server_state(new_x, new_y, 0.0, 0.0, self.direction, false);
    }

    /// Set server state - simple server-authoritative model
    /// Server is single source of truth, client just animates smoothly
    /// is_local_player: if true, only update direction when moving (stationary direction controlled locally)
    pub fn set_server_state(&mut self, x: f32, y: f32, vel_x: f32, vel_y: f32, dir: Direction, is_local_player: bool) {
        let old_server_x = self.server_x;
        let old_server_y = self.server_y;
        let old_vel_x = self.vel_x;
        let old_vel_y = self.vel_y;

        self.server_x = x;
        self.server_y = y;
        self.vel_x = vel_x;
        self.vel_y = vel_y;

        // Direction handling:
        // - Remote players: always accept server direction
        // - Local player when moving: accept server direction (confirms movement)
        // - Local player when sitting: accept server direction (chair controls it)
        // - Local player when stationary: keep local direction (Face commands control it)
        let is_moving = vel_x != 0.0 || vel_y != 0.0;
        let is_sitting = matches!(self.animation.state,
            crate::render::animation::AnimationState::SittingChair | crate::render::animation::AnimationState::SittingGround);
        if !is_local_player || is_moving || is_sitting {
            self.direction = dir;
        }

        // If dashing, set target to new position for fast slide interpolation (don't snap)
        if self.is_dashing {
            self.server_x = x;
            self.server_y = y;
            self.target_x = x;
            self.target_y = y;
            self.vel_x = 0.0;
            self.vel_y = 0.0;
            return;
        }

        // Teleport detection (>2 tiles = snap immediately)
        let dist = ((self.x - x).powi(2) + (self.y - y).powi(2)).sqrt();
        if dist > 2.0 {
            self.x = x;
            self.y = y;
            self.target_x = x;
            self.target_y = y;
            return;
        }

        // Server position changed = server confirmed a move
        let server_moved = (x - old_server_x).abs() > 0.01 || (y - old_server_y).abs() > 0.01;

        // Stopped = always update target to server position
        let stopped = vel_x == 0.0 && vel_y == 0.0;

        // At tile center = safe to update target
        let at_tile_center = (self.x - self.x.round()).abs() < 0.1
                          && (self.y - self.y.round()).abs() < 0.1;

        // Detect velocity direction change (intent changed but server hasn't moved yet)
        let vel_changed = (vel_x as i32, vel_y as i32) != (old_vel_x as i32, old_vel_y as i32);

        if vel_changed && !server_moved && !stopped {
            // Direction intent changed during cooldown (server hasn't moved yet).
            // Only predict ahead if visual is close to server position.
            // This prevents drift accumulation from chasing multiple prediction
            // targets during rapid direction changes.
            let drift = (self.x - x).abs().max((self.y - y).abs());
            if drift < 0.5 {
                if vel_x != 0.0 || vel_y != 0.0 {
                    self.target_x = x + vel_x;
                    self.target_y = y + vel_y;
                }
            } else {
                // Visual has drifted too far - converge to server position first
                self.target_x = x;
                self.target_y = y;
            }
        } else if server_moved || stopped || at_tile_center {
            if vel_x != 0.0 || vel_y != 0.0 {
                self.target_x = x + vel_x;
                self.target_y = y + vel_y;
            } else {
                self.target_x = x;
                self.target_y = y;
            }
        }
    }

    /// Smooth visual interpolation toward target position
    /// Uses axis-aligned movement: moves one grid axis at a time to prevent
    /// diagonal visual artifacts. Resolves the smaller displacement first
    /// to reach tile alignment quickly, then moves on the remaining axis.
    pub fn interpolate_visual(&mut self, delta: f32) {
        let dx = self.target_x - self.x;
        let dy = self.target_y - self.y;

        let old_x = self.x;
        let old_y = self.y;

        if dx.abs() < 0.01 && dy.abs() < 0.01 {
            self.x = self.target_x;
            self.y = self.target_y;
            self.is_moving = false;
            self.is_dashing = false; // Dash slide complete
        } else {
            // Use fast speed during dash (24 tiles/sec vs normal 4)
            let speed = if self.is_dashing { 16.0 } else { VISUAL_SPEED };
            let mut budget = speed * delta;

            // Axis-aligned movement: resolve smaller displacement first,
            // then use remaining budget on the larger axis.
            if dx.abs() <= dy.abs() {
                // X is smaller (or equal): resolve X, then Y
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
                // Y is smaller: resolve Y, then X
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
        let movement_dir = if moved_dx.abs() > 0.001 || moved_dy.abs() > 0.001 {
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

        // Sync animation direction only when safe (prevents moonwalking):
        // - When stationary: always safe to sync
        // - When moving: only if movement direction matches player direction
        let should_sync_direction = match movement_dir {
            None => true,  // Stationary - safe to sync
            Some(move_dir) => move_dir == self.direction,  // Moving - only if aligned
        };

        if should_sync_direction {
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
