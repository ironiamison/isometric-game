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
            if dy < 0.0 {
                Direction::Up
            } else {
                Direction::Down
            }
        } else {
            if dx < 0.0 {
                Direction::Left
            } else {
                Direction::Right
            }
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
// Keep walking loop alive briefly between step confirmations to avoid frame restarts.
const WALK_ANIM_GRACE_SECS: f64 = 0.10;

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
    /// Short grace window to keep walk loop continuous between step snapshots.
    walk_anim_grace_until: f64,
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
            is_mining: false,
            mining_started_at: 0.0,
            last_mining_anim: 0.0,
            is_dashing: false,
            walk_anim_grace_until: 0.0,
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
        self.set_server_state(new_x, new_y, 0.0, 0.0, self.direction);
    }

    /// Set server state in a strict step-confirmed model.
    /// Facing only changes on confirmed movement, or explicit face updates while idle.
    pub fn set_server_state(&mut self, x: f32, y: f32, vel_x: f32, vel_y: f32, dir: Direction) {
        let old_server_x = self.server_x;
        let old_server_y = self.server_y;

        self.server_x = x;
        self.server_y = y;
        self.vel_x = vel_x;
        self.vel_y = vel_y;

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
            self.direction = dir;
            self.animation.direction = dir;
            self.is_moving = false;
            return;
        }

        // Server position changed = server confirmed a tile step.
        let step_dx = x - old_server_x;
        let step_dy = y - old_server_y;
        let server_moved = step_dx.abs() > 0.01 || step_dy.abs() > 0.01;

        if server_moved {
            self.target_x = x;
            self.target_y = y;
            self.direction = Direction::from_velocity(step_dx, step_dy);
            return;
        }

        // No confirmed step: keep current movement target to avoid pre-turns.
        // Allow explicit face updates only when idle.
        let idle_intent = vel_x == 0.0 && vel_y == 0.0;
        if idle_intent {
            self.target_x = x;
            self.target_y = y;
            self.direction = dir;
        }
    }

    /// Smooth visual interpolation toward target position
    /// Linear constant-speed interpolation keeps stepping smooth without prediction.
    pub fn interpolate_visual(&mut self, delta: f32) {
        let dx = self.target_x - self.x;
        let dy = self.target_y - self.y;
        let dist = (dx * dx + dy * dy).sqrt();

        let old_x = self.x;
        let old_y = self.y;

        if dist < 0.01 {
            self.x = self.target_x;
            self.y = self.target_y;
            self.is_moving = false;
            self.is_dashing = false; // Dash slide complete
        } else {
            // Use fast speed during dash (normal movement remains 4 tiles/sec).
            let speed = if self.is_dashing { 16.0 } else { VISUAL_SPEED };
            let move_dist = speed * delta;

            if dist <= move_dist {
                self.x = self.target_x;
                self.y = self.target_y;
            } else {
                self.x += (dx / dist) * move_dist;
                self.y += (dy / dist) * move_dist;
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

        let now = macroquad::time::get_time();
        if self.is_moving {
            self.walk_anim_grace_until = now + WALK_ANIM_GRACE_SECS;
        }
        let should_walk_loop = self.is_moving || now < self.walk_anim_grace_until;

        // Handle action animations (attack, cast, etc) - they take priority
        let in_action = self.animation.state == AnimationState::Attacking
            || self.animation.state == AnimationState::Casting
            || self.animation.state == AnimationState::ShootingBow;

        if in_action {
            self.animation.update(delta);
            if self.animation.is_finished() {
                if should_walk_loop {
                    self.animation.set_state(AnimationState::Walking);
                } else {
                    self.animation.set_state(AnimationState::Idle);
                }
            }
            return;
        }

        // Sitting state is controlled by explicit sit/stand events.
        if self.animation.state == AnimationState::SittingGround
            || self.animation.state == AnimationState::SittingChair
        {
            return;
        }

        // Animation direction follows actual movement to prevent moonwalking.
        // While moving, only update direction when we actually moved this frame.
        // During grace-only frames, keep previous animation direction to avoid flips.
        if should_walk_loop {
            if let Some(move_dir) = movement_dir {
                self.animation.direction = move_dir;
            }
        } else {
            self.animation.direction = self.direction;
        }

        // Handle movement animations
        if should_walk_loop {
            self.animation.set_state(AnimationState::Walking);
            self.animation.update(delta);
        } else {
            self.animation.set_state(AnimationState::Idle);
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
