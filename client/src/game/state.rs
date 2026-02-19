use super::chunk::ChunkManager;
use super::entities::Player;
use super::item::{GroundItem, Inventory, RecipeDefinition};
use super::item_registry::ItemRegistry;
use super::npc::Npc;
use super::pathfinding::PathState;
use super::shop::{ShopData, ShopSubTab};
use super::tilemap::Tilemap;
use crate::render::AreaBanner;
use crate::render::XpGlobesManager;
use crate::ui::UiElementId;
use std::collections::{HashMap, HashSet};

const MAX_CHAT_LOG_MESSAGES: usize = 120;

/// State of a map transition (fade effect)
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionState {
    None,
    FadingOut,
    Loading,
    FadingIn,
}

/// Tracks an in-progress map transition
#[derive(Debug, Clone)]
pub struct MapTransition {
    pub state: TransitionState,
    pub progress: f32,
    pub target_map_type: String,
    pub target_map_id: String,
    pub target_spawn_x: f32,
    pub target_spawn_y: f32,
    pub instance_id: String,
}

impl Default for MapTransition {
    fn default() -> Self {
        Self {
            state: TransitionState::None,
            progress: 0.0,
            target_map_type: String::new(),
            target_map_id: String::new(),
            target_spawn_x: 0.0,
            target_spawn_y: 0.0,
            instance_id: String::new(),
        }
    }
}

/// Frame timing diagnostics for performance analysis
#[derive(Clone)]
pub struct FrameTimings {
    pub network_ms: f64,
    pub render_total_ms: f64,
    pub render_ground_ms: f64,
    pub render_entities_ms: f64,
    pub render_overhead_ms: f64,
    pub render_ui_ms: f64,
    pub render_effects_ms: f64,
    pub update_ms: f64,
    pub total_ms: f64,
    pub entity_count: usize,
    pub chunk_count: usize,
    pub tiles_rendered: usize,
    // Frame delta tracking (rolling window)
    pub delta_ms: f64,
    pub delta_min_ms: f64,
    pub delta_max_ms: f64,
    delta_samples: [f64; 60],
    delta_idx: usize,
    // Optional FPS cap (None = uncapped)
    pub fps_cap: Option<u32>,
    // Time spent in next_frame().await (for diagnosing variance)
    pub next_frame_ms: f64,
    pub next_frame_min_ms: f64,
    pub next_frame_max_ms: f64,
    next_frame_samples: [f64; 60],
    next_frame_idx: usize,
    // Smoothed delta for visual interpolation (0.0 = no smoothing, 1.0 = max smoothing)
    pub delta_smoothing: f32,
    pub smoothed_delta: f32,
}

impl Default for FrameTimings {
    fn default() -> Self {
        Self {
            network_ms: 0.0,
            render_total_ms: 0.0,
            render_ground_ms: 0.0,
            render_entities_ms: 0.0,
            render_overhead_ms: 0.0,
            render_ui_ms: 0.0,
            render_effects_ms: 0.0,
            update_ms: 0.0,
            total_ms: 0.0,
            entity_count: 0,
            chunk_count: 0,
            tiles_rendered: 0,
            delta_ms: 0.0,
            delta_min_ms: 0.0,
            delta_max_ms: 0.0,
            delta_samples: [0.0; 60],
            delta_idx: 0,
            fps_cap: Some(144), // High-but-stable default pacing on most displays
            next_frame_ms: 0.0,
            next_frame_min_ms: 0.0,
            next_frame_max_ms: 0.0,
            next_frame_samples: [0.0; 60],
            next_frame_idx: 0,
            delta_smoothing: 0.8,  // Default smoothing to reduce visible pacing jitter
            smoothed_delta: 1.0 / 120.0, // Start near high-refresh frame pacing
        }
    }
}

impl FrameTimings {
    pub fn record_next_frame(&mut self, ms: f64) {
        self.next_frame_ms = ms;
        self.next_frame_samples[self.next_frame_idx] = ms;
        self.next_frame_idx = (self.next_frame_idx + 1) % 60;

        // Calculate min/max over the window
        self.next_frame_min_ms = f64::MAX;
        self.next_frame_max_ms = f64::MIN;
        for &sample in &self.next_frame_samples {
            if sample > 0.0 {
                self.next_frame_min_ms = self.next_frame_min_ms.min(sample);
                self.next_frame_max_ms = self.next_frame_max_ms.max(sample);
            }
        }
        if self.next_frame_min_ms == f64::MAX {
            self.next_frame_min_ms = ms;
        }
        if self.next_frame_max_ms == f64::MIN {
            self.next_frame_max_ms = ms;
        }
    }
}

impl FrameTimings {
    pub fn record_delta(&mut self, delta_ms: f64) {
        self.delta_ms = delta_ms;
        self.delta_samples[self.delta_idx] = delta_ms;
        self.delta_idx = (self.delta_idx + 1) % 60;

        // Calculate min/max over the window
        self.delta_min_ms = f64::MAX;
        self.delta_max_ms = f64::MIN;
        for &sample in &self.delta_samples {
            if sample > 0.0 {
                self.delta_min_ms = self.delta_min_ms.min(sample);
                self.delta_max_ms = self.delta_max_ms.max(sample);
            }
        }
        if self.delta_min_ms == f64::MAX {
            self.delta_min_ms = delta_ms;
        }
        if self.delta_max_ms == f64::MIN {
            self.delta_max_ms = delta_ms;
        }

        // Update smoothed delta for visual interpolation.
        // Clamp extreme outliers so one hitch doesn't create a large visual jump.
        let delta_secs = ((delta_ms / 1000.0) as f32).clamp(1.0 / 240.0, 1.0 / 30.0);
        if self.delta_smoothing > 0.0 {
            self.smoothed_delta = self.smoothed_delta * self.delta_smoothing
                + delta_secs * (1.0 - self.delta_smoothing);
        } else {
            self.smoothed_delta = delta_secs;
        }
    }
}

/// Rolling ping statistics for debug display
pub struct PingStats {
    /// Rolling window of recent ping samples
    samples: [f64; 20],
    idx: usize,
    filled: usize,
    /// When the last auto-ping was sent
    pub last_auto_ping: f64,
    /// Current/latest ping
    pub current_ms: f64,
    /// Rolling average
    pub avg_ms: f64,
    /// Rolling min
    pub min_ms: f64,
    /// Rolling max
    pub max_ms: f64,
}

impl Default for PingStats {
    fn default() -> Self {
        Self {
            samples: [0.0; 20],
            idx: 0,
            filled: 0,
            last_auto_ping: 0.0,
            current_ms: 0.0,
            avg_ms: 0.0,
            min_ms: 0.0,
            max_ms: 0.0,
        }
    }
}

impl PingStats {
    pub fn record(&mut self, ms: f64) {
        self.current_ms = ms;
        self.samples[self.idx] = ms;
        self.idx = (self.idx + 1) % 20;
        if self.filled < 20 {
            self.filled += 1;
        }

        // Recalculate stats over filled samples
        let mut sum = 0.0;
        let mut min = f64::MAX;
        let mut max = f64::MIN;
        for i in 0..self.filled {
            let s = self.samples[i];
            sum += s;
            min = min.min(s);
            max = max.max(s);
        }
        self.avg_ms = sum / self.filled as f64;
        self.min_ms = min;
        self.max_ms = max;
    }

    pub fn has_data(&self) -> bool {
        self.filled > 0
    }
}

pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
    pub initialized: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
            initialized: false,
        }
    }
}

/// Chat channel types for different message sources
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatChannel {
    Local,  // Nearby players only (current default)
    Global, // Server-wide player chat
    System, // XP gains, quest completions, shop transactions
            // Future:
            // Party,
            // Guild,
}

pub struct ChatMessage {
    pub sender_name: String,
    pub text: String,
    pub timestamp: f64,
    pub channel: ChatChannel,
}

impl ChatMessage {
    /// Create a player chat message (defaults to Local channel)
    pub fn player(sender_name: String, text: String) -> Self {
        ChatMessage {
            sender_name,
            text,
            timestamp: macroquad::time::get_time(),
            channel: ChatChannel::Local,
        }
    }

    /// Create a system message
    pub fn system(text: String) -> Self {
        ChatMessage {
            sender_name: "[System]".to_string(),
            text,
            timestamp: macroquad::time::get_time(),
            channel: ChatChannel::System,
        }
    }
}

/// Floating damage/healing number for combat feedback
/// - Positive damage = damage dealt (red)
/// - Negative damage = healing (green, displayed as +X)
/// - Zero = miss (gray, displayed as "MISS")
pub struct DamageEvent {
    pub x: f32,
    pub y: f32,
    pub damage: i32,
    pub time: f64,
    /// Entity ID to look up sprite height at render time
    pub target_id: String,
    pub source_id: Option<String>,
    pub projectile: Option<String>,
}

/// Pending spell effect received from server (to be rendered by Task 9)
pub struct SpellEffect {
    pub caster_id: String,
    pub target_id: Option<String>,
    pub spell_id: String,
    pub target_x: i32,
    pub target_y: i32,
    pub time: f64,
}

/// Active projectile for ranged attack visualization
pub struct Projectile {
    pub sprite: String,
    pub start_x: f32,
    pub start_y: f32,
    pub end_x: f32,
    pub end_y: f32,
    pub start_time: f64,
    pub duration: f64,
}

impl Projectile {
    /// Get current position (0.0 to 1.0 progress)
    pub fn progress(&self, current_time: f64) -> f32 {
        let elapsed = current_time - self.start_time;
        (elapsed / self.duration).min(1.0) as f32
    }

    /// Check if projectile animation is complete
    pub fn is_complete(&self, current_time: f64) -> bool {
        current_time - self.start_time >= self.duration
    }

    /// Get current world position
    pub fn current_pos(&self, current_time: f64) -> (f32, f32) {
        let t = self.progress(current_time);
        let x = self.start_x + (self.end_x - self.start_x) * t;
        let y = self.start_y + (self.end_y - self.start_y) * t;
        (x, y)
    }
}

/// Floating level up text
pub struct LevelUpEvent {
    pub x: f32,
    pub y: f32,
    pub skill: String,
    pub new_level: i32,
    pub time: f64,
}

/// Floating skill XP gain text
pub struct SkillXpEvent {
    pub x: f32,
    pub y: f32,
    pub skill: String,
    pub xp_gained: i64,
    pub time: f64,
}

/// Info about a depleted tree for respawn timer display
pub struct DepletedTreeInfo {
    pub gid: u32,
    pub depleted_at: f64, // Client time when depleted
    pub respawn_at: f64,  // Client time when it will respawn
}

/// Info about a depleted rock for respawn timer display
pub struct DepletedRockInfo {
    pub gid: u32,
    pub depleted_at: f64, // Client time when depleted
    pub respawn_at: f64,  // Client time when it will respawn
}

/// Tree shake effect when being chopped
pub struct TreeShakeEffect {
    pub x: i32,
    pub y: i32,
    pub started_at: f64,
    pub intensity: f32,
}

impl TreeShakeEffect {
    pub const DURATION: f64 = 0.25; // seconds

    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            started_at: macroquad::time::get_time(),
            intensity: 1.0,
        }
    }

    /// Returns the current horizontal offset for the shake effect
    pub fn get_offset(&self) -> f32 {
        let elapsed = macroquad::time::get_time() - self.started_at;
        if elapsed > Self::DURATION {
            return 0.0;
        }
        let progress = elapsed / Self::DURATION;
        let decay = 1.0 - progress as f32;
        let shake = (elapsed * 40.0).sin() as f32; // Fast oscillation
        shake * decay * self.intensity * 3.0 // Max 3 pixel offset
    }

    pub fn is_finished(&self) -> bool {
        macroquad::time::get_time() - self.started_at > Self::DURATION
    }
}

/// Rock shake effect when being mined
pub struct RockShakeEffect {
    pub x: i32,
    pub y: i32,
    pub started_at: f64,
    pub intensity: f32,
}

impl RockShakeEffect {
    pub const DURATION: f64 = 0.20;

    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            started_at: macroquad::time::get_time(),
            intensity: 1.0,
        }
    }

    pub fn get_offset(&self) -> f32 {
        let elapsed = macroquad::time::get_time() - self.started_at;
        if elapsed > Self::DURATION {
            return 0.0;
        }
        let progress = elapsed / Self::DURATION;
        let decay = 1.0 - progress as f32;
        let shake = (elapsed * 50.0).sin() as f32;
        shake * decay * self.intensity * 2.0
    }

    pub fn is_finished(&self) -> bool {
        macroquad::time::get_time() - self.started_at > Self::DURATION
    }
}

/// A falling leaf particle
pub struct LeafParticle {
    pub tile_x: f32,     // World tile X position
    pub tile_y: f32,     // World tile Y position
    pub height: f32,     // Height above ground in screen pixels (starts high, falls to 0)
    pub drift_x: f32,    // Horizontal drift velocity
    pub fall_speed: f32, // Fall speed in pixels per second
    pub rotation: f32,
    pub rotation_speed: f32,
    pub size: f32,
    pub color: macroquad::color::Color,
    pub started_at: f64,
    pub on_ground: bool, // True when leaf has landed
}

impl LeafParticle {
    pub const DURATION: f64 = 3.0; // seconds (longer so leaves pile up visibly)
    pub const GROUND_LINGER: f64 = 1.5; // How long leaves stay on ground before fading

    /// Create a new leaf at the top of a tree
    pub fn new_at_tree(tile_x: i32, tile_y: i32, tree_height: f32) -> Self {
        use macroquad::rand::gen_range;

        // Random leaf colors (greens, yellows, oranges)
        let color = match gen_range(0, 5) {
            0 => macroquad::color::Color::new(0.2, 0.55, 0.2, 0.95), // Dark green
            1 => macroquad::color::Color::new(0.3, 0.65, 0.25, 0.95), // Green
            2 => macroquad::color::Color::new(0.5, 0.6, 0.2, 0.95),  // Yellow-green
            3 => macroquad::color::Color::new(0.6, 0.5, 0.15, 0.95), // Orange-brown
            _ => macroquad::color::Color::new(0.4, 0.55, 0.2, 0.95), // Olive
        };

        Self {
            tile_x: tile_x as f32 + gen_range(-0.3, 0.3),
            tile_y: tile_y as f32 + gen_range(-0.2, 0.2),
            height: tree_height + gen_range(-10.0, 10.0), // Near top of tree
            drift_x: gen_range(-15.0, 15.0),
            fall_speed: gen_range(25.0, 45.0),
            rotation: gen_range(0.0, std::f32::consts::TAU),
            rotation_speed: gen_range(-4.0, 4.0),
            size: gen_range(3.0, 5.0),
            color,
            started_at: macroquad::time::get_time(),
            on_ground: false,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if !self.on_ground {
            // Gentle swaying while falling
            let sway = (macroquad::time::get_time() * 4.0 + self.rotation as f64).sin() as f32;
            self.drift_x += sway * 30.0 * dt;
            self.drift_x *= 0.95; // Damping

            // Apply drift to tile position (small amount)
            self.tile_x += self.drift_x * 0.002 * dt;

            // Fall down
            self.height -= self.fall_speed * dt;

            // Check if landed
            if self.height <= 0.0 {
                self.height = 0.0;
                self.on_ground = true;
            }

            self.rotation += self.rotation_speed * dt;
        } else {
            // On ground - slow down rotation
            self.rotation_speed *= 0.95;
            self.rotation += self.rotation_speed * dt;
        }
    }

    pub fn get_alpha(&self) -> f32 {
        let elapsed = macroquad::time::get_time() - self.started_at;

        if self.on_ground {
            // Fade out after lingering on ground
            let ground_time = elapsed - (Self::DURATION - Self::GROUND_LINGER);
            if ground_time > 0.0 {
                let fade_progress = (ground_time / Self::GROUND_LINGER) as f32;
                return (1.0 - fade_progress).max(0.0);
            }
        }
        1.0
    }

    pub fn is_finished(&self) -> bool {
        macroquad::time::get_time() - self.started_at > Self::DURATION
    }
}

/// A rock debris particle (flies off when mining)
pub struct RockParticle {
    pub tile_x: f32,
    pub tile_y: f32,
    pub height: f32,
    pub drift_x: f32,
    pub fall_speed: f32,
    pub rotation: f32,
    pub rotation_speed: f32,
    pub size: f32,
    pub color: macroquad::color::Color,
    pub started_at: f64,
    pub on_ground: bool,
}

impl RockParticle {
    pub const DURATION: f64 = 2.0;
    pub const GROUND_LINGER: f64 = 1.0;

    pub fn new_at_rock(tile_x: i32, tile_y: i32, rock_height: f32) -> Self {
        use macroquad::rand::gen_range;

        let color = match gen_range(0, 5) {
            0 => macroquad::color::Color::new(0.35, 0.33, 0.30, 0.95),
            1 => macroquad::color::Color::new(0.55, 0.53, 0.50, 0.95),
            2 => macroquad::color::Color::new(0.70, 0.68, 0.65, 0.95),
            3 => macroquad::color::Color::new(0.60, 0.55, 0.40, 0.95),
            _ => macroquad::color::Color::new(0.55, 0.40, 0.25, 0.95),
        };

        Self {
            tile_x: tile_x as f32 + gen_range(-0.2, 0.2),
            tile_y: tile_y as f32 + gen_range(-0.2, 0.2),
            height: rock_height + gen_range(-5.0, 5.0),
            drift_x: gen_range(-20.0, 20.0),
            fall_speed: gen_range(40.0, 70.0),
            rotation: gen_range(0.0, std::f32::consts::TAU),
            rotation_speed: gen_range(-3.0, 3.0),
            size: gen_range(2.0, 4.0),
            color,
            started_at: macroquad::time::get_time(),
            on_ground: false,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if !self.on_ground {
            self.drift_x *= 0.97;
            self.tile_x += self.drift_x * 0.002 * dt;
            self.height -= self.fall_speed * dt;

            if self.height <= 0.0 {
                self.height = 0.0;
                self.on_ground = true;
            }

            self.rotation += self.rotation_speed * dt;
        } else {
            self.rotation_speed *= 0.9;
            self.rotation += self.rotation_speed * dt;
        }
    }

    pub fn get_alpha(&self) -> f32 {
        let elapsed = macroquad::time::get_time() - self.started_at;

        if self.on_ground {
            let ground_time = elapsed - (Self::DURATION - Self::GROUND_LINGER);
            if ground_time > 0.0 {
                let fade_progress = (ground_time / Self::GROUND_LINGER) as f32;
                return (1.0 - fade_progress).max(0.0);
            }
        }
        1.0
    }

    pub fn is_finished(&self) -> bool {
        macroquad::time::get_time() - self.started_at > Self::DURATION
    }
}

/// A tree that's falling down after being chopped
pub struct FallingTreeEffect {
    pub x: i32,
    pub y: i32,
    pub gid: u32,
    pub started_at: f64,
    pub fall_direction: f32, // 1.0 = fall right, -1.0 = fall left
}

impl FallingTreeEffect {
    pub const DURATION: f64 = 1.5; // seconds

    pub fn new(x: i32, y: i32, gid: u32) -> Self {
        // Random fall direction
        let fall_direction = if macroquad::rand::gen_range(0, 2) == 0 {
            -1.0
        } else {
            1.0
        };
        Self {
            x,
            y,
            gid,
            started_at: macroquad::time::get_time(),
            fall_direction,
        }
    }

    /// Returns (rotation_angle, alpha, y_offset)
    pub fn get_transform(&self) -> (f32, f32, f32) {
        let elapsed = macroquad::time::get_time() - self.started_at;
        let progress = (elapsed / Self::DURATION).min(1.0) as f32;

        // Ease-in rotation (accelerating fall)
        let ease = progress * progress;
        let angle = ease * std::f32::consts::FRAC_PI_2 * self.fall_direction * 0.8; // Max ~72 degrees

        // Fade out in second half
        let alpha = if progress > 0.5 {
            1.0 - ((progress - 0.5) * 2.0)
        } else {
            1.0
        };

        // Slight downward movement as it falls
        let y_offset = ease * 10.0;

        (angle, alpha, y_offset)
    }

    pub fn is_finished(&self) -> bool {
        macroquad::time::get_time() - self.started_at > Self::DURATION
    }
}

/// A rock fragment that splits off when a rock is fully mined
pub struct RockFragment {
    pub grid_x: u8,           // 0 or 1 (column in 2x2 grid)
    pub grid_y: u8,           // 0 or 1 (row in 2x2 grid)
    pub drift_x: f32,         // drift in pixels per second
    pub drift_y: f32,
    pub rotation_speed: f32,  // radians per second
}

/// A rock splitting apart after being fully mined
pub struct CrumblingRockEffect {
    pub x: i32,
    pub y: i32,
    pub gid: u32,
    pub started_at: f64,
    pub fragments: Vec<RockFragment>,
}

impl CrumblingRockEffect {
    pub const DURATION: f64 = 1.2; // slightly longer for scatter to settle

    pub fn new(x: i32, y: i32, gid: u32) -> Self {
        use macroquad::rand::gen_range;

        // Create 4 fragments in a 2x2 grid, each drifting INWARD (collapsing)
        let drift = 12.0; // gentle inward drift speed in pixels/sec
        let fragments = vec![
            RockFragment {
                grid_x: 0, grid_y: 0, // top-left piece drifts toward center (right & down)
                drift_x: gen_range(drift - 4.0, drift + 4.0),
                drift_y: gen_range(drift - 4.0, drift + 4.0),
                rotation_speed: gen_range(-1.5, -0.3),
            },
            RockFragment {
                grid_x: 1, grid_y: 0, // top-right piece drifts toward center (left & down)
                drift_x: gen_range(-drift - 4.0, -drift + 4.0),
                drift_y: gen_range(drift - 4.0, drift + 4.0),
                rotation_speed: gen_range(0.3, 1.5),
            },
            RockFragment {
                grid_x: 0, grid_y: 1, // bottom-left piece drifts toward center (right & up)
                drift_x: gen_range(drift - 4.0, drift + 4.0),
                drift_y: gen_range(-drift - 4.0, -drift + 4.0),
                rotation_speed: gen_range(-1.5, -0.3),
            },
            RockFragment {
                grid_x: 1, grid_y: 1, // bottom-right piece drifts toward center (left & up)
                drift_x: gen_range(-drift - 4.0, -drift + 4.0),
                drift_y: gen_range(-drift - 4.0, -drift + 4.0),
                rotation_speed: gen_range(0.3, 1.5),
            },
        ];

        Self {
            x,
            y,
            gid,
            started_at: macroquad::time::get_time(),
            fragments,
        }
    }

    /// Returns (progress 0-1, alpha 0-1, scale 0-1) for the overall effect
    pub fn get_progress_alpha(&self) -> (f32, f32, f32) {
        let elapsed = macroquad::time::get_time() - self.started_at;
        let progress = (elapsed / Self::DURATION).min(1.0) as f32;

        // Fade out in the last 50%
        let alpha = if progress > 0.5 {
            1.0 - ((progress - 0.5) / 0.5)
        } else {
            1.0
        };

        // Shrink fragments as they collapse: 1.0 → 0.5
        let scale = 1.0 - progress * 0.5;

        (progress, alpha, scale)
    }

    pub fn is_finished(&self) -> bool {
        macroquad::time::get_time() - self.started_at > Self::DURATION
    }
}

/// An XP drop notification that floats up and fades out in the HUD
pub struct XpDrop {
    pub skill_type: super::SkillType,
    pub xp_gained: i64,
    pub time: f64,
}

/// Manages the XP drop feed displayed below the player stats panel
#[derive(Default)]
pub struct XpDropFeed {
    pub drops: Vec<XpDrop>,
}

impl XpDropFeed {
    pub fn new() -> Self {
        Self { drops: Vec::new() }
    }

    pub fn push(&mut self, skill_type: super::SkillType, xp_gained: i64) {
        self.drops.push(XpDrop {
            skill_type,
            xp_gained,
            time: macroquad::time::get_time(),
        });
    }

    /// No-op — positioning is purely time-based in the renderer.
    pub fn update(&mut self, _dt: f32) {}
}

/// Chat bubble displayed above a player's head
pub struct ChatBubble {
    pub player_id: String,
    pub text: String,
    pub time: f64,
}

/// A choice in a dialogue box
#[derive(Clone, Debug)]
pub struct DialogueChoice {
    pub id: String,
    pub text: String,
}

/// Active dialogue being shown to the player
#[derive(Clone, Debug)]
pub struct ActiveDialogue {
    pub quest_id: String,
    pub npc_id: String,
    pub speaker: String,
    pub text: String,
    pub choices: Vec<DialogueChoice>,
    pub show_time: f64,
}

/// A quest objective with progress tracking
#[derive(Clone, Debug)]
pub struct QuestObjective {
    pub id: String,
    pub description: String,
    pub current: i32,
    pub target: i32,
    pub completed: bool,
}

/// An active quest with its objectives
#[derive(Clone, Debug)]
pub struct ActiveQuest {
    pub id: String,
    pub name: String,
    pub objectives: Vec<QuestObjective>,
}

/// Quest completion notification
#[derive(Clone, Debug)]
pub struct QuestCompletedEvent {
    pub quest_id: String,
    pub quest_name: String,
    pub exp_reward: i32,
    pub gold_reward: i32,
    pub time: f64,
}

/// Server-wide announcement from admin
#[derive(Clone, Debug)]
pub struct Announcement {
    pub text: String,
    pub time: f64,
}

/// A farming patch in the world
#[derive(Debug, Clone)]
pub struct FarmingPatch {
    pub patch_id: String,
    pub x: i32,
    pub y: i32,
    pub state: String, // "empty", "growing", "harvestable"
    pub crop_id: String,
    pub growth_stage: u32,
    pub owner_id: String,
}

/// Active farming contract info received from server
#[derive(Debug, Clone)]
pub struct FarmingContractInfo {
    pub difficulty: String,
    pub crop_name: String,
    pub amount_required: i32,
    pub amount_harvested: i32,
}

/// A gathering marker tile in the world (fishing spot, mining node, etc.)
#[derive(Debug, Clone)]
pub struct GatheringMarker {
    pub x: i32,
    pub y: i32,
    pub zone_id: String,
    pub skill: String,
}

/// An active bonus tile event (glowing spot that gives 2x gather speed)
#[derive(Debug, Clone)]
pub struct BonusTile {
    pub x: i32,
    pub y: i32,
    pub zone_id: String,
    pub spawn_time: f64,
    pub telegraph_duration: f64,
}

/// An active gathering buff on a player
#[derive(Debug, Clone)]
pub struct GatheringBuff {
    pub buff_type: String,
    pub start_time: f64,
    pub duration: f64,
}

/// Target for context menu - what was right-clicked
#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    InventorySlot(usize),
    EquipmentSlot(String),
    Gold,
}

/// Context menu for right-clicking items
#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub target: ContextMenuTarget,
    pub x: f32,
    pub y: f32,
}

/// Dialog for entering gold drop amount
#[derive(Debug, Clone)]
pub struct GoldDropDialog {
    pub input: String,
    pub cursor: usize,
}

/// What action the bank quantity dialog will perform on confirm
#[derive(Debug, Clone, PartialEq)]
pub enum BankQuantityAction {
    DepositItem,
    WithdrawItem,
    DepositGold,
    WithdrawGold,
}

/// Dialog for entering a custom quantity in the bank UI (Ctrl+Click)
#[derive(Debug, Clone)]
pub struct BankQuantityDialog {
    pub input: String,
    pub cursor: usize,
    pub action: BankQuantityAction,
    pub item_id: Option<String>,
    pub max_quantity: i32,
}

/// Source of a drag operation
#[derive(Debug, Clone, PartialEq)]
pub enum DragSource {
    Inventory(usize),  // Inventory slot index
    Equipment(String), // Equipment slot type ("body", "feet")
}

/// Drag state for inventory/equipment rearrangement
#[derive(Debug, Clone)]
pub struct DragState {
    pub source: DragSource,
    pub item_id: String,
    pub quantity: i32,
}

/// Double-click tracking for inventory slots
#[derive(Debug, Clone)]
pub struct DoubleClickState {
    pub last_click_slot: Option<usize>,
    pub last_click_time: f64,
}

// ============================================================================
// Friend System State
// ============================================================================

/// Friend info for the friends list
#[derive(Debug, Clone)]
pub struct FriendInfo {
    pub id: i64,
    pub name: String,
    pub online: bool,
}

/// Pending friend request info
#[derive(Debug, Clone)]
pub struct PendingRequestInfo {
    pub from_id: i64,
    pub from_name: String,
}

/// Online player info for the social panel
#[derive(Debug, Clone)]
pub struct OnlinePlayerInfo {
    pub id: i64,
    pub name: String,
    pub is_friend: bool,
}

/// Social panel tab
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SocialTab {
    Nearby,
    Online,
    Friends,
}

impl Default for SocialTab {
    fn default() -> Self {
        Self::Nearby
    }
}

/// Social panel state
#[derive(Debug, Clone, Default)]
pub struct SocialState {
    /// Currently selected tab
    pub active_tab: SocialTab,
    /// Friends list (loaded from server)
    pub friends: Vec<FriendInfo>,
    /// Pending friend requests (loaded from server)
    pub pending_requests: Vec<PendingRequestInfo>,
    /// Online players list (refreshed on demand)
    pub online_players: Vec<OnlinePlayerInfo>,
    /// Nearby players (players in same room/instance)
    pub nearby_players: Vec<OnlinePlayerInfo>,
    /// Number of pending friend requests (for badge)
    pub pending_request_count: usize,
    /// "Add by name" input field text
    pub add_friend_input: String,
    /// Whether the add friend input is focused
    pub add_friend_focused: bool,
    /// Scroll offset for player list
    pub list_scroll_offset: f32,
    /// Scroll offset for friends list
    pub friends_scroll_offset: f32,
    /// Touch scroll tracking
    pub touch_scroll_id: Option<u64>,
    pub touch_last_y: f32,
    pub touch_start_y: f32,
    pub touch_dragged: bool,
}

#[derive(Debug, Clone)]
pub struct AltarPanelState {
    pub altar_npc_id: String,
    pub altar_name: String,
}

pub struct UiState {
    pub chat_open: bool,
    pub chat_input: String,
    pub chat_cursor: usize, // Cursor position in chat_input (character index)
    pub chat_scroll_offset: usize, // Scroll offset for long messages (character index)
    pub chat_message_scroll: f32, // Scroll offset for message list (in pixels from bottom)
    pub chat_key_repeat_time: f64, // Last time a repeated key action fired
    pub chat_key_initial_delay: bool, // Whether we're still in initial delay
    pub chat_messages: Vec<ChatMessage>,
    pub chat_revision: u64, // Increments whenever chat content changes (for render cache invalidation)
    pub inventory_open: bool,
    // Quest UI state
    pub active_dialogue: Option<ActiveDialogue>,
    pub active_quests: Vec<ActiveQuest>,
    pub completed_quest_ids: HashSet<String>,
    pub adventurer_selected_tab: usize,
    pub adventurer_selected_tier: usize,
    pub quest_completed_events: Vec<QuestCompletedEvent>,
    pub quest_log_open: bool,
    pub quest_log_scroll: f32,
    // Crafting UI state
    pub crafting_open: bool,
    pub crafting_selected_category: usize,
    pub crafting_selected_recipe: usize,
    pub crafting_scroll_offset: f32,
    pub crafting_npc_id: Option<String>,
    // Crafting progress state (timed crafting)
    pub crafting_in_progress: bool,
    pub crafting_recipe_id: Option<String>,
    pub crafting_progress: f32,
    pub crafting_duration_ms: u64,
    pub crafting_started_at: Option<f64>,
    // Crafting completion animation: (recipe_id, timer_0_to_1)
    pub crafting_complete_animation: Option<(String, f32)>,
    // Shop UI state
    pub shop_data: Option<ShopData>,
    pub shop_npc_id: Option<String>,
    pub shop_sub_tab: ShopSubTab,
    pub shop_main_tab: usize, // 0=Recipes, 1=Shop
    pub shop_selected_buy_index: usize,
    pub shop_selected_sell_index: usize,
    pub shop_buy_quantity: i32,
    pub shop_sell_quantity: i32,
    pub shop_buy_scroll: f32,  // Scroll offset for buy list (pixels)
    pub shop_sell_scroll: f32, // Scroll offset for sell list (pixels)
    // Touch drag scroll tracking for shop lists
    pub shop_touch_scroll_id: Option<u64>,
    pub shop_touch_scroll_column: u8, // 0=buy, 1=sell
    pub shop_touch_last_y: f32,
    pub shop_touch_start_y: f32,
    pub shop_touch_dragged: bool,
    // Hold-to-repeat for quantity +/- buttons
    pub shop_quantity_hold_element: Option<UiElementId>,
    pub shop_quantity_hold_start: f64,
    pub shop_quantity_hold_last_repeat: f64,
    // Bank UI state
    pub bank_open: bool,
    pub bank_slots: Vec<Option<(String, i32)>>, // (item_id, quantity) per slot
    pub bank_gold: i32,
    pub bank_max_slots: u32,
    pub bank_scroll: f32,
    pub bank_inv_scroll: f32,
    // Escape menu state
    pub escape_menu_open: bool,
    // Audio settings (synced with AudioManager)
    pub audio_volume: f32,
    pub audio_sfx_volume: f32,
    pub audio_muted: bool,
    // UI scale (0.75 to 1.25, default 1.0 on desktop, 0.75 on mobile)
    pub ui_scale: f32,
    // Input settings
    pub shift_drop_enabled: bool,
    // Menu button panel states
    pub social_open: bool,
    pub skills_open: bool,
    pub character_panel_open: bool,
    pub prayer_book_open: bool,
    pub minimap_panel_open: bool,
    pub minimap_panel_zoom: f32,
    pub minimap_panel_center_x: Option<f32>,
    pub minimap_panel_center_y: Option<f32>,
    pub minimap_panel_dragging: bool,
    pub minimap_panel_drag_last_x: f32,
    pub minimap_panel_drag_last_y: f32,
    // Mouse hover state for UI elements
    pub hovered_element: Option<UiElementId>,
    // Context menu state
    pub context_menu: Option<ContextMenu>,
    // Gold drop dialog state
    pub gold_drop_dialog: Option<GoldDropDialog>,
    // Bank quantity dialog state (Ctrl+Click in bank)
    pub bank_quantity_dialog: Option<BankQuantityDialog>,
    // Bank help overlay open
    pub bank_help_open: bool,
    // Altar offering panel state
    pub altar_panel: Option<AltarPanelState>,
    // Drag state for inventory slot rearrangement
    pub drag_state: Option<DragState>,
    // Double-click tracking for equipping items
    pub double_click_state: DoubleClickState,
    // Server announcements
    pub announcements: Vec<Announcement>,
    // Chat log visibility (hidden by default on mobile)
    pub chat_log_visible: bool,
    // Chat log semi-transparent background
    pub chat_log_background: bool,
    // Mobile chat panel
    pub chat_panel_open: bool,
    pub chat_active_tab: ChatChannel,
    // Last read message timestamp per tab (for unread tab highlighting)
    pub chat_last_seen_local: f64,
    pub chat_last_seen_global: f64,
    pub chat_last_seen_system: f64,
    // Tap-to-pathfind (enabled by default on mobile, disabled on desktop)
    pub tap_to_pathfind: bool,
    // Use joystick instead of D-pad for mobile controls
    pub use_joystick: bool,
    // Dialogue scroll offset and touch scroll tracking
    pub dialogue_scroll_offset: f32,
    pub dialogue_touch_scroll_id: Option<u64>,
    pub dialogue_touch_last_y: f32,
    pub dialogue_touch_start_y: f32,
    pub dialogue_touch_dragged: bool,
    pub dialogue_scrollbar_dragging: bool,
    pub dialogue_scrollbar_drag_last_y: f32,
    // Inventory grid scroll offset (for small screens where not all rows fit)
    pub inventory_scroll_offset: f32,
    // Touch drag scroll tracking for inventory grid
    pub inventory_touch_scroll_id: Option<u64>,
    pub inventory_touch_last_y: f32,
    pub inventory_scrollbar_dragging: bool,
    pub inventory_scrollbar_drag_last_y: f32,
    /// Which settings slider is currently being dragged (if any)
    pub settings_slider_dragging: Option<crate::ui::UiElementId>,
    // Control scheme: false = Modern (WASD+Space+Enter), true = Classic (Arrows+Ctrl+always-on chat)
    pub classic_controls: bool,
    /// Active tab in the prayer/spell panel: 0 = Prayers, 1 = Spells
    pub prayer_spell_tab: usize,
    /// Whether the spell bar is active (true) or item bar (false)
    pub spell_bar_active: bool,
    /// Whether prayer help overlay is open
    pub prayer_help_open: bool,
    /// Whether spell help overlay is open
    pub spell_help_open: bool,
    /// Graphics quality: true = low (no water shaders), false = high
    pub graphics_low: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            chat_open: false,
            chat_input: String::new(),
            chat_cursor: 0,
            chat_scroll_offset: 0,
            chat_message_scroll: 0.0,
            chat_key_repeat_time: 0.0,
            chat_key_initial_delay: true,
            chat_messages: Vec::new(),
            chat_revision: 0,
            inventory_open: false,
            active_dialogue: None,
            active_quests: Vec::new(),
            completed_quest_ids: HashSet::new(),
            adventurer_selected_tab: 0,
            adventurer_selected_tier: 0,
            quest_completed_events: Vec::new(),
            quest_log_open: false,
            quest_log_scroll: 0.0,
            crafting_open: false,
            crafting_selected_category: 0,
            crafting_selected_recipe: 0,
            crafting_scroll_offset: 0.0,
            crafting_npc_id: None,
            crafting_in_progress: false,
            crafting_recipe_id: None,
            crafting_progress: 0.0,
            crafting_duration_ms: 0,
            crafting_started_at: None,
            crafting_complete_animation: None,
            shop_data: None,
            shop_npc_id: None,
            shop_sub_tab: ShopSubTab::Buy,
            shop_main_tab: 0,
            shop_selected_buy_index: 0,
            shop_selected_sell_index: 0,
            shop_buy_quantity: 1,
            shop_sell_quantity: 1,
            shop_buy_scroll: 0.0,
            shop_sell_scroll: 0.0,
            shop_touch_scroll_id: None,
            shop_touch_scroll_column: 0,
            shop_touch_last_y: 0.0,
            shop_touch_start_y: 0.0,
            shop_touch_dragged: false,
            shop_quantity_hold_element: None,
            shop_quantity_hold_start: 0.0,
            shop_quantity_hold_last_repeat: 0.0,
            bank_open: false,
            bank_slots: Vec::new(),
            bank_gold: 0,
            bank_max_slots: 48,
            bank_scroll: 0.0,
            bank_inv_scroll: 0.0,
            escape_menu_open: false,
            audio_volume: 0.7,
            audio_sfx_volume: 0.7,
            audio_muted: false,
            #[cfg(target_os = "android")]
            ui_scale: 0.75,
            #[cfg(not(target_os = "android"))]
            ui_scale: 1.0,
            shift_drop_enabled: true,
            social_open: false,
            skills_open: false,
            character_panel_open: false,
            prayer_book_open: false,
            minimap_panel_open: false,
            minimap_panel_zoom: 1.0,
            minimap_panel_center_x: None,
            minimap_panel_center_y: None,
            minimap_panel_dragging: false,
            minimap_panel_drag_last_x: 0.0,
            minimap_panel_drag_last_y: 0.0,
            hovered_element: None,
            context_menu: None,
            gold_drop_dialog: None,
            bank_quantity_dialog: None,
            bank_help_open: false,
            altar_panel: None,
            drag_state: None,
            double_click_state: DoubleClickState {
                last_click_slot: None,
                last_click_time: 0.0,
            },
            announcements: Vec::new(),
            #[cfg(target_os = "android")]
            chat_log_visible: false,
            #[cfg(not(target_os = "android"))]
            chat_log_visible: true,
            chat_log_background: true,
            chat_panel_open: false,
            chat_active_tab: ChatChannel::Local,
            chat_last_seen_local: 0.0,
            chat_last_seen_global: 0.0,
            chat_last_seen_system: 0.0,
            #[cfg(target_os = "android")]
            tap_to_pathfind: false,
            #[cfg(not(target_os = "android"))]
            tap_to_pathfind: true,
            use_joystick: false,
            dialogue_scroll_offset: 0.0,
            dialogue_touch_scroll_id: None,
            dialogue_touch_last_y: 0.0,
            dialogue_touch_start_y: 0.0,
            dialogue_touch_dragged: false,
            dialogue_scrollbar_dragging: false,
            dialogue_scrollbar_drag_last_y: 0.0,
            inventory_scroll_offset: 0.0,
            inventory_touch_scroll_id: None,
            inventory_touch_last_y: 0.0,
            inventory_scrollbar_dragging: false,
            inventory_scrollbar_drag_last_y: 0.0,
            settings_slider_dragging: None,
            classic_controls: false,
            prayer_spell_tab: 0,
            spell_bar_active: false,
            prayer_help_open: false,
            spell_help_open: false,
            graphics_low: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
}

pub struct GameState {
    // Connection
    pub connection_status: ConnectionStatus,
    pub local_player_id: Option<String>,
    /// Short lock to prevent stale server snapshots from briefly reverting
    /// a just-issued local face direction.
    pub local_face_lock_dir: Option<super::entities::Direction>,
    /// Client time when local_face_lock_dir expires.
    pub local_face_lock_until: f64,
    pub selected_character_name: Option<String>,
    pub disconnect_requested: bool,
    pub reconnection_failed: bool,

    // World
    pub tilemap: Tilemap,
    pub chunk_manager: ChunkManager,
    pub players: HashMap<String, Player>,
    pub npcs: HashMap<String, Npc>,
    pub ground_items: HashMap<String, GroundItem>,
    /// Items waiting to spawn (with spawn time) - delays loot appearance until after death animation
    pub pending_ground_items: Vec<(GroundItem, f64)>,
    /// Farming patches received from server
    pub farming_patches: HashMap<String, FarmingPatch>,
    /// Farming patch lookup by position
    pub farming_patch_positions: HashMap<(i32, i32), String>,
    /// Which farming plots the player has unlocked
    pub unlocked_farming_plots: Vec<u32>,
    /// Active farming contract (if any)
    pub farming_contract: Option<FarmingContractInfo>,
    /// Ground tile overrides from server (farming plot tiles: locked=65, unlocked=62)
    pub ground_tile_overrides: HashMap<(i32, i32), u32>,
    /// Gathering marker positions received from server
    pub gathering_markers: Vec<GatheringMarker>,
    /// Whether the local player is currently gathering
    pub is_gathering: bool,
    /// Whether the local player is currently sitting on a chair
    pub is_sitting: bool,
    /// Chair positions on the map (received from server)
    pub chair_positions: Vec<(i32, i32)>,
    /// Pending chair to sit on after pathfinding completes
    pub pending_chair_sit: Option<(i32, i32)>,
    pub pending_harvest_patch: Option<String>,
    /// Timestamp when gathering started (for cast animation delay)
    pub gathering_started_at: f64,
    /// Active bonus tile events
    pub bonus_tiles: Vec<BonusTile>,
    /// Active gathering buff on local player
    pub gathering_buff: Option<GatheringBuff>,

    /// Depleted trees (position -> info for respawn timer)
    pub depleted_trees: HashMap<(i32, i32), DepletedTreeInfo>,
    /// Depleted rocks (position -> info for respawn timer)
    pub depleted_rocks: HashMap<(i32, i32), DepletedRockInfo>,
    /// Local dash cooldown tracking (game time when dash becomes available again)
    pub dash_cooldown_end: f64,
    /// Whether the local player is currently woodcutting
    pub is_woodcutting: bool,
    /// Timestamp when woodcutting started
    pub woodcutting_started_at: f64,
    /// Whether the local player is currently mining
    pub is_mining: bool,
    /// Timestamp when mining started
    pub mining_started_at: f64,
    /// Tree shake effects (when being chopped)
    pub tree_shake_effects: Vec<TreeShakeEffect>,
    /// Falling leaf particles
    pub leaf_particles: Vec<LeafParticle>,
    /// Trees falling down after being chopped
    pub falling_trees: Vec<FallingTreeEffect>,
    /// Rock shake effects (when being mined)
    pub rock_shake_effects: Vec<RockShakeEffect>,
    /// Rock debris particles
    pub rock_particles: Vec<RockParticle>,
    /// Rocks crumbling after being fully mined
    pub crumbling_rocks: Vec<CrumblingRockEffect>,

    // Targeting
    pub selected_entity_id: Option<String>,

    // Combat feedback
    pub damage_events: Vec<DamageEvent>,
    pub level_up_events: Vec<LevelUpEvent>,
    /// Pending sound effects to play (queued by message handler, played by main loop)
    pub pending_sfx: Vec<String>,
    /// Pending attack sounds (bool = has_weapon) queued by message handler
    pub pending_attack_sounds: Vec<bool>,
    pub skill_xp_events: Vec<SkillXpEvent>,
    pub xp_globes: XpGlobesManager,
    pub xp_drop_feed: XpDropFeed,
    pub projectiles: Vec<Projectile>,

    // Chat bubbles above players
    pub chat_bubbles: Vec<ChatBubble>,

    // Inventory
    pub inventory: Inventory,

    // Item registry (loaded from server)
    pub item_registry: ItemRegistry,

    // Crafting
    pub recipe_definitions: Vec<RecipeDefinition>,
    pub discovered_recipes: HashSet<String>,

    // Camera and UI
    pub camera: Camera,
    pub ui_state: UiState,

    // Server tick (for ordering)
    pub server_tick: u64,

    // Debug
    pub debug_mode: bool,

    // Tile hover state (world coordinates of tile under mouse)
    pub hovered_tile: Option<(i32, i32)>,

    // Entity hover state (ID of entity under mouse cursor)
    pub hovered_entity_id: Option<String>,

    // Automated pathfinding state
    pub auto_path: Option<PathState>,

    // Performance diagnostics (visible in debug mode)
    pub frame_timings: FrameTimings,

    // Map transition state
    pub map_transition: MapTransition,

    /// Current interior map ID if in an interior (None = overworld)
    pub current_interior: Option<String>,
    /// Current instance ID if in an instance
    pub current_instance: Option<String>,
    /// Pending portal to enter (set when player walks onto a portal)
    pub pending_portal_id: Option<String>,
    /// Last tile position checked for portal (to avoid triggering on spawn)
    pub last_portal_check_pos: Option<(i32, i32)>,
    /// Portal tile to ignore until player steps off it (prevents flip-flop on transitions).
    /// Set to spawn tile after any map transition; cleared when player moves to a different tile.
    pub portal_ignore_tile: Option<(i32, i32)>,
    /// Area banner for displaying location names during transitions
    pub area_banner: AreaBanner,

    /// Social/Friends system state
    pub social_state: SocialState,

    // Spell system state
    /// Active spell effects for rendering
    pub spell_effects: Vec<SpellEffect>,
    /// Spell cooldowns tracked on client for UI feedback
    pub spell_cooldowns: std::collections::HashMap<String, f64>, // spell_id -> time when cooldown expires

    // Prayer system state
    /// Current prayer points
    pub prayer_points: i32,
    /// Maximum prayer points (based on prayer level)
    pub max_prayer_points: i32,
    /// Currently active prayers (by prayer ID)
    pub active_prayers: Vec<String>,

    /// Timestamp when last ping was sent (for latency measurement)
    pub ping_sent_at: Option<f64>,

    /// Continuous ping tracking (for debug menu)
    pub ping_stats: PingStats,

    /// Fade-in progress when world first becomes ready (1.0 = fully black, 0.0 = done)
    pub world_fade_in: f32,
    /// Whether the world has ever been ready (to trigger fade-in once)
    pub world_was_ready: bool,
}

impl GameState {
    pub fn new() -> Self {
        // Create a test tilemap (32x32 tiles) - kept for compatibility
        let tilemap = Tilemap::new_test_map(32, 32);

        Self {
            connection_status: ConnectionStatus::Disconnected,
            local_player_id: None,
            local_face_lock_dir: None,
            local_face_lock_until: 0.0,
            selected_character_name: None,
            disconnect_requested: false,
            reconnection_failed: false,
            tilemap,
            chunk_manager: ChunkManager::new(),
            players: HashMap::new(),
            npcs: HashMap::new(),
            ground_items: HashMap::new(),
            pending_ground_items: Vec::new(),
            farming_patches: HashMap::new(),
            farming_patch_positions: HashMap::new(),
            unlocked_farming_plots: vec![1],
            farming_contract: None,
            ground_tile_overrides: HashMap::new(),
            gathering_markers: Vec::new(),
            is_gathering: false,
            is_sitting: false,
            chair_positions: Vec::new(),
            pending_chair_sit: None,
            pending_harvest_patch: None,
            gathering_started_at: 0.0,
            bonus_tiles: Vec::new(),
            gathering_buff: None,
            dash_cooldown_end: 0.0,
            depleted_trees: HashMap::new(),
            depleted_rocks: HashMap::new(),
            is_woodcutting: false,
            woodcutting_started_at: 0.0,
            is_mining: false,
            mining_started_at: 0.0,
            tree_shake_effects: Vec::new(),
            leaf_particles: Vec::new(),
            falling_trees: Vec::new(),
            rock_shake_effects: Vec::new(),
            rock_particles: Vec::new(),
            crumbling_rocks: Vec::new(),
            selected_entity_id: None,
            damage_events: Vec::new(),
            level_up_events: Vec::new(),
            pending_sfx: Vec::new(),
            pending_attack_sounds: Vec::new(),
            skill_xp_events: Vec::new(),
            xp_globes: XpGlobesManager::new(),
            xp_drop_feed: XpDropFeed::new(),
            projectiles: Vec::new(),
            chat_bubbles: Vec::new(),
            inventory: Inventory::new(),
            item_registry: ItemRegistry::new(),
            recipe_definitions: Vec::new(),
            discovered_recipes: HashSet::new(),
            camera: Camera::default(),
            ui_state: UiState::default(),
            server_tick: 0,
            debug_mode: false,
            hovered_tile: None,
            hovered_entity_id: None,
            auto_path: None,
            frame_timings: FrameTimings::default(),
            map_transition: MapTransition::default(),
            current_interior: None,
            current_instance: None,
            pending_portal_id: None,
            last_portal_check_pos: None,
            portal_ignore_tile: None,
            area_banner: AreaBanner::default(),
            social_state: SocialState::default(),
            spell_effects: Vec::new(),
            spell_cooldowns: std::collections::HashMap::new(),
            prayer_points: 0,
            max_prayer_points: 1,
            active_prayers: Vec::new(),
            ping_sent_at: None,
            ping_stats: PingStats::default(),
            world_fade_in: 0.0,
            world_was_ready: false,
        }
    }

    /// Clear the current auto-path (e.g., when player presses movement keys)
    pub fn clear_auto_path(&mut self) {
        self.auto_path = None;
    }

    /// Append a chat message and bump revision so renderer cache invalidates once.
    pub fn push_chat_message(&mut self, message: ChatMessage) {
        self.ui_state.chat_messages.push(message);
        self.ui_state.chat_revision = self.ui_state.chat_revision.wrapping_add(1);
    }

    /// Update all players in a server-authoritative step model.
    pub fn update(&mut self, delta: f32) {
        // Trigger fade-in when world first becomes ready
        if !self.world_was_ready && self.is_world_ready() {
            self.world_was_ready = true;
            self.world_fade_in = 1.0;
        }

        // Tick down fade-in overlay
        if self.world_fade_in > 0.0 {
            self.world_fade_in = (self.world_fade_in - delta * 3.0).max(0.0); // ~0.33s fade
        }

        // Keep chat history bounded regardless of source channel.
        if self.ui_state.chat_messages.len() > MAX_CHAT_LOG_MESSAGES {
            let to_remove = self.ui_state.chat_messages.len() - MAX_CHAT_LOG_MESSAGES;
            self.ui_state.chat_messages.drain(0..to_remove);
            self.ui_state.chat_revision = self.ui_state.chat_revision.wrapping_add(1);
        }

        // Use smoothed delta for visual interpolation (reduces jitter from frame variance)
        let visual_delta = self.frame_timings.smoothed_delta;
        // Keep local movement tightly synced to real frame time to reduce
        // drift/corrections during rapid directional changes.
        let local_visual_delta = delta.clamp(1.0 / 240.0, 1.0 / 30.0);
        let local_id = self.local_player_id.clone();

        // Update all players (smooth interpolation toward server positions)
        // Note: woodcutting animations are now driven by server WoodcuttingSwing messages
        for (player_id, player) in self.players.iter_mut() {
            let step_delta = if local_id.as_ref() == Some(player_id) {
                local_visual_delta
            } else {
                visual_delta
            };
            player.interpolate_visual(step_delta);
        }

        // Update camera to follow local player
        if let Some(local_id) = &self.local_player_id {
            if let Some(player) = self.players.get(local_id) {
                self.camera.x = player.x;
                self.camera.y = player.y;
                self.camera.initialized = true;
            }
        }

        // Update NPCs (interpolation toward server positions)
        for npc in self.npcs.values_mut() {
            npc.update(visual_delta);
        }

        // Process pending ground items (spawn them after delay)
        let current_time = macroquad::time::get_time();
        let mut i = 0;
        while i < self.pending_ground_items.len() {
            if current_time >= self.pending_ground_items[i].1 {
                let (item, _) = self.pending_ground_items.swap_remove(i);
                self.ground_items.insert(item.id.clone(), item);
            } else {
                i += 1;
            }
        }

        // Clean up old damage events (older than 1.2 seconds)
        self.damage_events
            .retain(|event| current_time - event.time < 1.2);

        // Clean up old level up events (older than 2.0 seconds)
        self.level_up_events
            .retain(|event| current_time - event.time < 1.2);

        // Update tree effects
        self.tree_shake_effects.retain(|e| !e.is_finished());
        self.falling_trees.retain(|e| !e.is_finished());

        // Update leaf particles
        for leaf in &mut self.leaf_particles {
            leaf.update(delta);
        }
        self.leaf_particles.retain(|p| !p.is_finished());

        // Update rock effects
        self.rock_shake_effects.retain(|e| !e.is_finished());
        self.crumbling_rocks.retain(|e| !e.is_finished());
        for particle in &mut self.rock_particles {
            particle.update(delta);
        }
        self.rock_particles.retain(|p| !p.is_finished());

        // Clean up old skill XP events (older than 1.5 seconds)
        self.skill_xp_events
            .retain(|event| current_time - event.time < 1.5);

        // Update and clean up XP drops
        self.xp_drop_feed.update(delta);
        self.xp_drop_feed
            .drops
            .retain(|drop| current_time - drop.time < 2.0);

        // Clean up old chat bubbles (older than 5.0 seconds)
        self.chat_bubbles
            .retain(|bubble| current_time - bubble.time < 5.0);

        // Clean up expired NPC speech bubbles
        for npc in self.npcs.values_mut() {
            if let Some((_, time)) = &npc.speech_bubble {
                if current_time - time > 5.0 {
                    npc.speech_bubble = None;
                }
            }
        }

        // Clean up completed projectiles
        self.projectiles.retain(|p| !p.is_complete(current_time));

        // Clean up finished spell effects (max 3 seconds as fallback)
        self.spell_effects.retain(|effect| {
            let elapsed = current_time - effect.time;
            elapsed < 3.0
        });

        // Clean up old quest completion events (older than 4 seconds)
        self.ui_state
            .quest_completed_events
            .retain(|event| current_time - event.time < 4.0);

        // Clean up old announcements (older than 8 seconds)
        self.ui_state
            .announcements
            .retain(|ann| current_time - ann.time < 8.0);

        // Update crafting progress (Task 14)
        if self.ui_state.crafting_in_progress {
            if let Some(started) = self.ui_state.crafting_started_at {
                let elapsed = ((macroquad::time::get_time() - started) * 1000.0) as f32;
                let duration = self.ui_state.crafting_duration_ms as f32;
                if duration > 0.0 {
                    self.ui_state.crafting_progress = (elapsed / duration).min(1.0);
                }
            }
        }

        // Update crafting completion animation timer (Task 20)
        if let Some((_, ref mut timer)) = self.ui_state.crafting_complete_animation {
            *timer += delta; // ~1 second animation
            if *timer >= 1.0 {
                // Animation done
            }
        }
        if self
            .ui_state
            .crafting_complete_animation
            .as_ref()
            .map_or(false, |(_, t)| *t >= 1.0)
        {
            self.ui_state.crafting_complete_animation = None;
        }

        // Update area banner timer
        self.area_banner.update(delta);

        // Update XP globes (fade timers, hover detection)
        // Calculate globe position to match renderer
        let margin = 12.0;
        let base_y = 25.0;
        let tag_height = 22.0;
        let bar_width = 120.0_f32.max(140.0);
        let (vw, _) = crate::util::virtual_screen_size();
        let bar_x = (vw - bar_width - margin).floor();
        let globe_stats_y = base_y + tag_height / 2.0 + 8.0;
        self.xp_globes.update(bar_x, globe_stats_y);
    }

    pub fn get_local_player(&self) -> Option<&Player> {
        self.local_player_id
            .as_ref()
            .and_then(|id| self.players.get(id))
    }

    /// Get recipes filtered by the current shop's crafting categories.
    /// Returns all recipes if no shop is open.
    pub fn shop_filtered_recipes(&self) -> Vec<RecipeDefinition> {
        if let Some(ref shop) = self.ui_state.shop_data {
            if shop.crafting_categories.is_empty() {
                Vec::new()
            } else {
                self.recipe_definitions
                    .iter()
                    .filter(|r| shop.crafting_categories.contains(&r.category))
                    .cloned()
                    .collect()
            }
        } else {
            self.recipe_definitions.clone()
        }
    }

    /// Returns true when the world is ready to render (player exists and their chunk is loaded)
    pub fn is_world_ready(&self) -> bool {
        if let Some(player) = self.get_local_player() {
            // Check if the player's current chunk is loaded
            let chunk_coord = crate::game::chunk::ChunkCoord::from_world_f32(player.x, player.y);
            self.chunk_manager.chunks().contains_key(&chunk_coord)
        } else {
            false
        }
    }

    /// Update map transition animation
    pub fn update_transition(&mut self, delta: f32) {
        const FADE_DURATION: f32 = 0.25;

        match self.map_transition.state {
            TransitionState::FadingOut => {
                self.map_transition.progress += delta / FADE_DURATION;
                if self.map_transition.progress >= 1.0 {
                    self.map_transition.progress = 1.0;
                    self.map_transition.state = TransitionState::Loading;
                }
            }
            TransitionState::FadingIn => {
                self.map_transition.progress -= delta / FADE_DURATION;
                if self.map_transition.progress <= 0.0 {
                    self.map_transition.progress = 0.0;
                    self.map_transition.state = TransitionState::None;
                }
            }
            _ => {}
        }
    }

    /// Start a map transition
    pub fn start_transition(
        &mut self,
        map_type: String,
        map_id: String,
        spawn_x: f32,
        spawn_y: f32,
        instance_id: String,
    ) {
        self.map_transition = MapTransition {
            state: TransitionState::FadingOut,
            progress: 0.0,
            target_map_type: map_type,
            target_map_id: map_id,
            target_spawn_x: spawn_x,
            target_spawn_y: spawn_y,
            instance_id,
        };
    }

    /// Check if input should be blocked due to transition
    pub fn is_transitioning(&self) -> bool {
        self.map_transition.state != TransitionState::None
    }
}
