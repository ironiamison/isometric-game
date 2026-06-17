use super::entities::Direction;
use crate::render::animation::{NpcAnimation, NpcAnimationLayout, NpcAnimationState};

// ============================================================================
// NPC State
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcState {
    Idle = 0,
    Chasing = 1,
    Attacking = 2,
    Returning = 3,
    Dead = 4,
    Submerging = 6,
    Emerging = 7,
    Burrowing = 8,
}

impl NpcState {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => NpcState::Idle,
            1 => NpcState::Chasing,
            2 => NpcState::Attacking,
            3 => NpcState::Returning,
            4 => NpcState::Dead,
            6 => NpcState::Submerging,
            7 => NpcState::Emerging,
            8 => NpcState::Burrowing,
            _ => NpcState::Idle,
        }
    }
}

// ============================================================================
// NPC Entity
// ============================================================================

#[derive(Debug, Clone)]
pub struct Npc {
    pub id: String,
    /// Entity prototype ID (e.g., "pig", "elder_villager") - determines sprite
    pub entity_type: String,
    /// Display name from server (e.g., "Piggy Lv.1")
    pub display_name: String,
    /// Visual position (smoothly interpolated)
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Server-authoritative position
    pub server_x: f32,
    pub server_y: f32,
    pub server_z: f32,
    /// Target for interpolation
    pub target_x: f32,
    pub target_y: f32,
    pub target_z: f32,
    pub direction: Direction,
    pub hp: i32,
    pub max_hp: i32,
    pub level: i32,
    pub state: NpcState,
    pub animation: NpcAnimation,
    /// Whether this NPC is hostile
    pub hostile: bool,
    /// Whether this NPC offers quests
    pub is_quest_giver: bool,
    /// Whether this quest giver currently has a quest ready to turn in
    pub can_turn_in_quest: bool,
    /// Whether this NPC is a merchant
    pub is_merchant: bool,
    /// Whether this NPC is an altar
    pub is_altar: bool,
    /// Whether this NPC is a banker
    pub is_banker: bool,
    /// Movement speed in tiles per second (from server, for interpolation)
    pub move_speed: f32,
    /// Last time this NPC took damage (for health bar visibility)
    pub last_damage_time: f64,
    /// Death animation timer - Some(t) means dying, None means alive
    pub death_timer: Option<f32>,
    /// NPC will die after reaching target position
    pub pending_death: bool,
    /// Speech bubble text and timestamp
    pub speech_bubble: Option<(String, f64)>,
    /// Whether to hide the shadow under this NPC
    pub no_shadow: bool,
    /// Vertical pixel offset for rendering (positive = down)
    pub render_offset_y: f32,
    /// Whether this NPC is a slayer master
    pub is_slayer_master: bool,
    /// Whether this NPC is friendly (non-attackable, no level shown)
    pub is_friendly: bool,
    /// Whether this NPC is a port master (offers travel between locations)
    pub is_port_master: bool,
    /// Station type (e.g. "furnace", "anvil") if this NPC is a crafting station
    pub station_type: Option<String>,
    /// Delay before death animation starts (so killing blow attack animation is visible)
    pub death_delay: Option<f32>,
    /// NPC footprint size (1 = single tile, 2 = 2x2, etc.)
    pub size: i32,
    /// Client time (seconds) this NPC was first seen — used for spawn fade-in
    /// (e.g. Reaper soul-wraiths).
    pub spawned_at: f64,
}

impl Npc {
    pub fn new(id: String, entity_type: String, x: f32, y: f32) -> Self {
        Self {
            id,
            entity_type,
            display_name: String::new(), // Set by server
            x,
            y,
            z: 0.0,
            target_x: x,
            target_y: y,
            target_z: 0.0,
            server_x: x,
            server_y: y,
            server_z: 0.0,
            direction: Direction::Down,
            hp: 1,
            max_hp: 1,
            level: 1,
            state: NpcState::Idle,
            animation: NpcAnimation::default(),
            hostile: true,
            is_quest_giver: false,
            can_turn_in_quest: false,
            is_merchant: false,
            is_altar: false,
            is_banker: false,
            is_slayer_master: false,
            is_friendly: false,
            is_port_master: false,
            move_speed: 2.0, // Default, will be set by server
            last_damage_time: 0.0,
            death_timer: None,
            pending_death: false,
            speech_bubble: None,
            no_shadow: false,
            render_offset_y: 0.0,
            station_type: None,
            death_delay: None,
            size: 1,
            spawned_at: macroquad::time::get_time(),
        }
    }

    pub fn name(&self) -> String {
        // Don't show level for friendly NPCs
        if self.is_friendly
            || self.is_quest_giver
            || self.is_merchant
            || self.is_altar
            || self.is_banker
            || self.is_slayer_master
            || self.is_port_master
            || self.station_type.is_some()
        {
            self.display_name.clone()
        } else {
            format!("{} Lv.{}", self.display_name, self.level)
        }
    }

    pub fn is_hostile(&self) -> bool {
        self.hostile
    }

    /// Returns true if this NPC can be attacked/targeted by players.
    /// Friendly NPCs, quest givers, merchants, altars, and bankers cannot be attacked.
    pub fn is_attackable(&self) -> bool {
        !self.is_friendly
            && !self.is_quest_giver
            && !self.is_merchant
            && !self.is_altar
            && !self.is_banker
            && !self.is_slayer_master
            && !self.is_port_master
            && self.station_type.is_none()
    }

    pub fn is_alive(&self) -> bool {
        self.state != NpcState::Dead
    }

    /// Check if NPC is in the process of dying (dead but animation not complete)
    pub fn is_dying(&self) -> bool {
        self.state == NpcState::Dead && !self.is_death_animation_complete()
    }

    /// Start the death sequence - NPC will finish moving to target then play death animation.
    pub fn start_death(&mut self) {
        self.state = NpcState::Dead;
        self.hp = 0;
        if self.animation.layout == NpcAnimationLayout::ExplodingRock {
            // Exploding rocks start death animation immediately (no walk-to-target)
            self.death_timer = Some(0.0);
            self.pending_death = false;
        } else {
            self.pending_death = true;
            // death_timer starts when NPC reaches target position (in update())
        }
    }

    /// Check if death animation is complete
    pub fn is_death_animation_complete(&self) -> bool {
        let duration = if self.animation.layout == NpcAnimationLayout::ExplodingRock {
            0.6 // 6 frames at 10fps
        } else {
            0.5
        };
        self.death_timer.map(|t| t >= duration).unwrap_or(false)
    }

    /// Get death animation color tint (fade to red while fading out)
    pub fn get_death_color(&self) -> Option<macroquad::color::Color> {
        use macroquad::color::Color;

        // Exploding rocks use their own explosion frames, no color tint
        if self.animation.layout == NpcAnimationLayout::ExplodingRock {
            return self
                .death_timer
                .map(|_| Color::from_rgba(255, 255, 255, 255));
        }

        self.death_timer.map(|t| {
            let progress = (t / 0.5).min(1.0);
            // Fade to red: green/blue go from 1.0 to 0.3
            let gb = 1.0 - 0.7 * progress;
            // Fade out: alpha goes from 1.0 to 0.0
            let alpha = 1.0 - progress;
            Color::new(1.0, gb, gb, alpha)
        })
    }

    /// Trigger attack animation - called when damage event is received
    pub fn trigger_attack_animation(&mut self) {
        self.animation.set_state(NpcAnimationState::Attacking);
    }

    /// Update position from server - matches player logic exactly
    pub fn set_server_position(&mut self, new_x: f32, new_y: f32) {
        self.server_x = new_x;
        self.server_y = new_y;

        let dx = self.x - new_x;
        let dy = self.y - new_y;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist > 4.0 {
            // Too far - hard snap
            self.x = new_x;
            self.y = new_y;
            self.target_x = new_x;
            self.target_y = new_y;
        } else {
            // Update target to server position
            self.target_x = new_x;
            self.target_y = new_y;
        }
    }

    /// Smooth visual interpolation - constant speed linear movement
    /// Speed is based on NPC's move_speed to match server timing
    pub fn update(&mut self, delta: f32) {
        // Update death animation timer
        if let Some(ref mut t) = self.death_timer {
            *t += delta;
            // For exploding rocks, play the explosion animation during death
            if self.animation.layout == NpcAnimationLayout::ExplodingRock {
                self.animation.set_state(NpcAnimationState::Exploding);
                self.animation.update(delta);
            }
            return; // Don't update position/animation while dying
        }

        // Tick death delay (lets killing blow animation play before death starts)
        if let Some(ref mut delay) = self.death_delay {
            *delay -= delta;
            if *delay <= 0.0 {
                self.death_delay = None;
                // Delay done, now proceed with pending_death logic below
            } else {
                return; // Still waiting for delay
            }
        }

        // If pending death but no timer yet, we need to finish moving first
        let is_pending_death = self.pending_death;

        // Skip normal updates for dead NPCs (unless pending death - they still need to move)
        if self.state == NpcState::Dead && !is_pending_death {
            return;
        }

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

        let dx = self.target_x - self.x;
        let dy = self.target_y - self.y;
        let dist = (dx * dx + dy * dy).sqrt();

        let actually_moving;

        if dist < 0.01 {
            // Reached target - snap exactly
            self.x = self.target_x;
            self.y = self.target_y;
            actually_moving = false;

            // If pending death and we've reached target, start death animation
            if is_pending_death {
                self.death_timer = Some(0.0);
                self.pending_death = false;
                return;
            }
        } else {
            // Linear interpolation - constant speed movement
            // Move faster when far behind to avoid visible teleport snaps
            // while still converging quickly to authoritative state.
            let catchup = (dist * 0.5).clamp(1.0, 3.0);
            let is_wraith = self.entity_type == "wraith";
            let base_speed = if self.state == NpcState::Burrowing {
                // Burrowing boss moves fast underground (matches server 150ms/tile)
                6.67
            } else if is_wraith {
                // Reaper soul: slow, ominous drift (matches server WRAITH_MOVE_MS ~600ms/tile)
                1.67
            } else {
                self.move_speed * 1.2
            };
            // Souls glide below the normal 2.0 floor so the drift reads slow.
            let floor = if is_wraith { 0.5 } else { 2.0 };
            let speed = (base_speed * catchup).max(floor);
            let move_dist = speed * delta;

            if dist <= move_dist {
                // Close enough - snap to target
                self.x = self.target_x;
                self.y = self.target_y;
            } else {
                // Move at constant speed toward target
                self.x += (dx / dist) * move_dist;
                self.y += (dy / dist) * move_dist;
            }

            actually_moving = true;
        }

        // Handle animation states (skip if pending death)
        if is_pending_death {
            // Keep walking animation while moving to death spot
            if actually_moving {
                self.animation.set_state(NpcAnimationState::Walking);
            }
            self.animation.update(delta);
            return;
        }

        // Handle special NPC states (submerging/emerging)
        if self.state == NpcState::Submerging {
            self.animation.set_state(NpcAnimationState::Submerging);
            self.animation.update(delta);
            return;
        }
        if self.state == NpcState::Burrowing {
            self.animation.set_state(NpcAnimationState::Burrowing);
            self.animation.update(delta);
            return;
        }
        if self.state == NpcState::Emerging {
            if self.animation.state != NpcAnimationState::Idle {
                self.animation.set_state(NpcAnimationState::Emerging);
                self.animation.update(delta);
                if self.animation.is_finished() {
                    self.animation.set_state(NpcAnimationState::Idle);
                }
            }
            return;
        }

        // Attack animation is triggered by trigger_attack_animation() when damage event received
        if self.animation.state == NpcAnimationState::Attacking {
            // Let attack animation play through
            if self.animation.is_finished() {
                // Animation done, return to normal
                if actually_moving {
                    self.animation.set_state(NpcAnimationState::Walking);
                } else {
                    self.animation.set_state(NpcAnimationState::Idle);
                }
            }
        } else {
            // Normal movement animation (not in attack animation)
            if actually_moving {
                self.animation.set_state(NpcAnimationState::Walking);
            } else {
                self.animation.set_state(NpcAnimationState::Idle);
            }
        }

        self.animation.update(delta);
    }
}
