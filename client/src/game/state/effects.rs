#[derive(Debug, Clone, Copy)]
pub enum AttackSoundType {
    Unarmed,
    Melee,
    Ranged,
}

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
    pub render_ui_chat_ms: f64,
    pub render_ui_hud_ms: f64,
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
            render_ui_chat_ms: 0.0,
            render_ui_hud_ms: 0.0,
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
            fps_cap: Some(140), // Default cap, toggle with F4
            next_frame_ms: 0.0,
            next_frame_min_ms: 0.0,
            next_frame_max_ms: 0.0,
            next_frame_samples: [0.0; 60],
            next_frame_idx: 0,
            delta_smoothing: 0.8, // Default smoothing to reduce visible pacing jitter
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

/// Tracks client-side auto-action chase state (OSRS-style click-to-act)
#[derive(Debug, Clone)]
pub struct AutoActionState {
    pub target_type: String, // "npc", "player", "resource"
    pub target_id: String,   // entity id or "x,y,gid"
    pub action: String,      // "attack", "mine", "chop"
    pub confirmed: bool,     // true after server sends AutoActionStarted
}

pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub zoom: f32,
    pub initialized: bool,
    pub transition_from: Option<(f32, f32)>, // Starting position of transition
    pub transition_progress: f32,            // 0.0 to 1.0
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            zoom: 1.0,
            initialized: false,
            transition_from: None,
            transition_progress: 0.0,
        }
    }
}

/// Smooth-step interpolation (ease in-out) for camera transitions.
pub(super) fn smooth_step(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
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

/// Per-channel chat storage so that system message spam doesn't evict public/global messages.
pub struct ChatLog {
    local: Vec<ChatMessage>,
    global: Vec<ChatMessage>,
    system: Vec<ChatMessage>,
}

impl Default for ChatLog {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatLog {
    pub fn new() -> Self {
        Self {
            local: Vec::new(),
            global: Vec::new(),
            system: Vec::new(),
        }
    }

    pub fn push(&mut self, message: ChatMessage) {
        // Mirror system messages into Local tab too (so players see them in public chat).
        // Keep the System channel on the copy so it renders with gold color.
        if matches!(message.channel, ChatChannel::System) {
            self.local.push(ChatMessage {
                sender_name: message.sender_name.clone(),
                text: message.text.clone(),
                timestamp: message.timestamp,
                channel: ChatChannel::System,
            });
            if self.local.len() > MAX_CHAT_LOG_MESSAGES {
                let to_remove = self.local.len() - MAX_CHAT_LOG_MESSAGES;
                self.local.drain(0..to_remove);
            }
        }

        let vec = match message.channel {
            ChatChannel::Local => &mut self.local,
            ChatChannel::Global => &mut self.global,
            ChatChannel::System => &mut self.system,
        };
        vec.push(message);
        if vec.len() > MAX_CHAT_LOG_MESSAGES {
            let to_remove = vec.len() - MAX_CHAT_LOG_MESSAGES;
            vec.drain(0..to_remove);
        }
    }

    /// Push a message to the System tab only (no Local mirror). Used for high-frequency
    /// messages like XP gains that would spam the Local tab.
    pub fn push_system_only(&mut self, message: ChatMessage) {
        self.system.push(message);
        if self.system.len() > MAX_CHAT_LOG_MESSAGES {
            let to_remove = self.system.len() - MAX_CHAT_LOG_MESSAGES;
            self.system.drain(0..to_remove);
        }
    }

    /// Get messages for a specific channel.
    pub fn channel(&self, channel: &ChatChannel) -> &[ChatMessage] {
        match channel {
            ChatChannel::Local => &self.local,
            ChatChannel::Global => &self.global,
            ChatChannel::System => &self.system,
        }
    }

    /// Get the latest timestamp for a given channel, or 0.0 if empty.
    pub fn latest_timestamp(&self, channel: &ChatChannel) -> f64 {
        self.channel(channel)
            .last()
            .map(|m| m.timestamp)
            .unwrap_or(0.0)
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
    pub start_z: f32,
    pub end_x: f32,
    pub end_y: f32,
    pub end_z: f32,
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

    /// Get current world position with Z.
    /// Blast projectiles arc upward (parabolic Z offset peaking at t=0.5).
    pub fn current_pos(&self, current_time: f64) -> (f32, f32, f32) {
        let t = self.progress(current_time);
        let x = self.start_x + (self.end_x - self.start_x) * t;
        let y = self.start_y + (self.end_y - self.start_y) * t;
        let base_z = self.start_z + (self.end_z - self.start_z) * t;

        // Parabolic arc: peaks at t=0.5, height scales with distance
        let dx = self.end_x - self.start_x;
        let dy = self.end_y - self.start_y;
        let dist = (dx * dx + dy * dy).sqrt();
        let arc_height = (dist * 0.3).max(1.0); // 30% of distance, minimum 1 tile
        let arc_offset = 4.0 * arc_height * t * (1.0 - t); // parabola: 0 at t=0, peak at t=0.5, 0 at t=1
        let z = base_z + arc_offset;

        (x, y, z)
    }

    /// Get the instantaneous velocity direction (tangent of the arc trajectory).
    /// Returns (dx/dt, dy/dt, dz/dt) in world space — use for orienting sprites along the arc.
    pub fn current_direction(&self, current_time: f64) -> (f32, f32, f32) {
        let t = self.progress(current_time);
        let vel_x = self.end_x - self.start_x;
        let vel_y = self.end_y - self.start_y;
        let base_vel_z = self.end_z - self.start_z;

        // Derivative of arc: d/dt [4 * h * t * (1-t)] = 4 * h * (1 - 2t)
        let dist = (vel_x * vel_x + vel_y * vel_y).sqrt();
        let arc_height = (dist * 0.3).max(1.0);
        let arc_vel_z = 4.0 * arc_height * (1.0 - 2.0 * t);

        (vel_x, vel_y, base_vel_z + arc_vel_z)
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

/// The kind of click effect to display
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClickEffectKind {
    Walk,
    Attack,
    Interact,
}

/// An animated click effect shown at a world tile when the player clicks
pub struct ClickEffect {
    pub tile_x: f32,
    pub tile_y: f32,
    pub kind: ClickEffectKind,
    pub elapsed: f32,
}

impl ClickEffect {
    /// Total animation duration in seconds (4 frames)
    pub const DURATION: f32 = 0.4;
    pub const FRAME_COUNT: u32 = 4;
    pub const FRAME_SIZE: f32 = 16.0;

    pub fn new(tile_x: f32, tile_y: f32, kind: ClickEffectKind) -> Self {
        Self {
            tile_x,
            tile_y,
            kind,
            elapsed: 0.0,
        }
    }

    pub fn update(&mut self, delta: f32) {
        self.elapsed += delta;
    }

    pub fn is_finished(&self) -> bool {
        self.elapsed >= Self::DURATION
    }

    /// Returns the current frame index (0..3)
    pub fn frame(&self) -> u32 {
        let f = (self.elapsed / Self::DURATION * Self::FRAME_COUNT as f32) as u32;
        f.min(Self::FRAME_COUNT - 1)
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
            tile_x: tile_x as f32 + gen_range(-0.35, 0.35),
            tile_y: tile_y as f32 + gen_range(-0.15, 0.25),
            height: rock_height * gen_range(0.25, 0.55),
            drift_x: gen_range(-40.0, 40.0),
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

/// A water bubble particle for fishing spot indicators
pub struct BubbleParticle {
    pub tile_x: f32,
    pub tile_y: f32,
    pub height: f32,  // Rises from 0 upward
    pub drift_x: f32, // Small horizontal wobble
    pub rise_speed: f32,
    pub size: f32,
    pub started_at: f64,
}

impl BubbleParticle {
    pub const DURATION: f64 = 1.2;

    pub fn new(tile_x: f32, tile_y: f32) -> Self {
        use macroquad::rand::gen_range;
        Self {
            tile_x: tile_x + gen_range(-0.25, 0.25),
            tile_y: tile_y + gen_range(-0.15, 0.15),
            height: 0.0,
            drift_x: gen_range(-8.0, 8.0),
            rise_speed: gen_range(12.0, 22.0),
            size: gen_range(1.0, 2.5),
            started_at: macroquad::time::get_time(),
        }
    }

    pub fn update(&mut self, dt: f32, now: f64) {
        self.height += self.rise_speed * dt;
        let sway = (now * 5.0 + self.drift_x as f64).sin() as f32;
        self.tile_x += sway * 0.3 * dt;
    }

    pub fn get_alpha(&self, now: f64) -> f32 {
        let elapsed = now - self.started_at;
        let progress = (elapsed / Self::DURATION) as f32;
        // Fade in quickly, fade out in last 40%
        if progress < 0.2 {
            progress / 0.2
        } else if progress > 0.6 {
            1.0 - (progress - 0.6) / 0.4
        } else {
            1.0
        }
    }

    pub fn is_finished(&self, now: f64) -> bool {
        now - self.started_at > Self::DURATION
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
/// A rock crumbling after being fully mined (simple shrink + fade)
pub struct CrumblingRockEffect {
    pub x: i32,
    pub y: i32,
    pub gid: u32,
    pub started_at: f64,
}

impl CrumblingRockEffect {
    pub const DURATION: f64 = 0.6; // quick pop — particles do the heavy lifting

    pub fn new(x: i32, y: i32, gid: u32) -> Self {
        Self {
            x,
            y,
            gid,
            started_at: macroquad::time::get_time(),
        }
    }

    /// Returns (scale, alpha) for the shrink + fade
    pub fn get_transform(&self) -> (f32, f32) {
        let elapsed = macroquad::time::get_time() - self.started_at;
        let progress = (elapsed / Self::DURATION).min(1.0) as f32;

        // Ease-in: accelerating shrink (starts slow, speeds up)
        let ease = progress * progress;

        // Shrink: 1.0 → 0.3
        let scale = 1.0 - ease * 0.7;

        // Fade: starts at 40% progress, fully gone by end
        let alpha = if progress > 0.4 {
            1.0 - ((progress - 0.4) / 0.6)
        } else {
            1.0
        };

        (scale, alpha)
    }

    pub fn is_finished(&self) -> bool {
        macroquad::time::get_time() - self.started_at > Self::DURATION
    }
}

/// An XP drop notification that floats up and fades out in the HUD
pub struct XpDrop {
    pub skill_type: crate::game::SkillType,
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

    pub fn push(&mut self, skill_type: crate::game::SkillType, xp_gained: i64) {
        self.drops.push(XpDrop {
            skill_type,
            xp_gained,
            time: macroquad::time::get_time(),
        });
    }

    /// No-op — positioning is purely time-based in the renderer.
    pub fn update(&mut self, _dt: f32) {}
}
