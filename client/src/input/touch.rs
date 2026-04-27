// Touch input handling for mobile devices
// Provides virtual joystick and touch buttons

use crate::mobile_scale::VIRTUAL_WIDTH;
use macroquad::prelude::*;

/// Convert screen coordinates to virtual coordinates (for Android scaling)
#[cfg(target_os = "android")]
fn to_virtual_coords(x: f32, y: f32) -> (f32, f32) {
    let screen_w = screen_width();
    let screen_h = screen_height();
    let (vw, vh) = virtual_screen_size();

    let vx = x * vw / screen_w;
    let vy = y * vh / screen_h;
    (vx, vy)
}

#[cfg(not(target_os = "android"))]
fn to_virtual_coords(x: f32, y: f32) -> (f32, f32) {
    (x, y)
}

/// Get virtual screen dimensions
fn virtual_screen_size() -> (f32, f32) {
    #[cfg(target_os = "android")]
    {
        // Calculate virtual height to match screen aspect ratio
        let screen_w = screen_width();
        let screen_h = screen_height();
        let aspect = screen_h / screen_w;
        let virtual_height = (VIRTUAL_WIDTH * aspect).round();
        (VIRTUAL_WIDTH, virtual_height)
    }
    #[cfg(not(target_os = "android"))]
    {
        (screen_width(), screen_height())
    }
}

/// D-pad direction (cardinal + diagonal)
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DPadDirection {
    None,
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}

impl DPadDirection {
    pub fn to_direction_u8(self) -> u8 {
        // Must match server Direction enum
        match self {
            DPadDirection::Down => 0,
            DPadDirection::Left => 1,
            DPadDirection::Up => 2,
            DPadDirection::Right => 3,
            DPadDirection::DownLeft => 4,
            DPadDirection::DownRight => 5,
            DPadDirection::UpLeft => 6,
            DPadDirection::UpRight => 7,
            DPadDirection::None => 0, // Default to down
        }
    }

    pub fn to_velocity(self) -> (f32, f32) {
        match self {
            DPadDirection::Up => (0.0, -1.0),
            DPadDirection::Down => (0.0, 1.0),
            DPadDirection::Left => (-1.0, 0.0),
            DPadDirection::Right => (1.0, 0.0),
            DPadDirection::UpLeft => (-1.0, -1.0),
            DPadDirection::UpRight => (1.0, -1.0),
            DPadDirection::DownLeft => (-1.0, 1.0),
            DPadDirection::DownRight => (1.0, 1.0),
            DPadDirection::None => (0.0, 0.0),
        }
    }
}

/// Virtual D-pad for mobile controls
/// Supports tap-to-face (quick tap) vs hold-to-move (like keyboard)
pub struct VirtualDPad {
    /// Center position of the D-pad
    center: Vec2,
    /// Size of each direction button
    button_size: f32,
    /// Gap between buttons
    gap: f32,
    /// Currently pressed direction
    current_dir: DPadDirection,
    /// Touch ID tracking input
    touch_id: Option<u64>,
    /// Time when direction was first pressed
    press_time: f64,
    /// Whether a move command has been sent (held past threshold)
    move_sent: bool,
    /// Direction that was just released (for face command)
    just_released_dir: DPadDirection,
}

impl VirtualDPad {
    pub fn new() -> Self {
        let (screen_w, screen_h) = virtual_screen_size();
        // Position in bottom-left area — on Android, use smaller buttons
        #[cfg(target_os = "android")]
        let (center_x, center_y, button_size) = (80.0, screen_h - 75.0, 42.0);
        #[cfg(not(target_os = "android"))]
        let (center_x, center_y, button_size) = (90.0, screen_h - 100.0, 52.0);

        Self {
            center: vec2(center_x, center_y),
            button_size,
            gap: 2.0,
            current_dir: DPadDirection::None,
            touch_id: None,
            press_time: 0.0,
            move_sent: false,
            just_released_dir: DPadDirection::None,
        }
    }

    /// Update D-pad position for current screen size
    pub fn update_position(&mut self) {
        let (_, screen_h) = virtual_screen_size();
        #[cfg(target_os = "android")]
        {
            self.center = vec2(80.0, screen_h - 75.0);
        }
        #[cfg(not(target_os = "android"))]
        {
            self.center = vec2(90.0, screen_h - 100.0);
        }
    }

    /// Get the button rect for a direction
    fn get_button_rect(&self, dir: DPadDirection) -> (f32, f32, f32, f32) {
        let half = self.button_size / 2.0;
        let offset = self.button_size + self.gap;
        let diag_size = self.button_size * 0.75;
        let diag_half = diag_size / 2.0;

        match dir {
            DPadDirection::Up => (
                self.center.x - half,
                self.center.y - offset - half,
                self.button_size,
                self.button_size,
            ),
            DPadDirection::Down => (
                self.center.x - half,
                self.center.y + offset - half,
                self.button_size,
                self.button_size,
            ),
            DPadDirection::Left => (
                self.center.x - offset - half,
                self.center.y - half,
                self.button_size,
                self.button_size,
            ),
            DPadDirection::Right => (
                self.center.x + offset - half,
                self.center.y - half,
                self.button_size,
                self.button_size,
            ),
            DPadDirection::UpLeft => (
                self.center.x - offset - diag_half,
                self.center.y - offset - diag_half,
                diag_size,
                diag_size,
            ),
            DPadDirection::UpRight => (
                self.center.x + offset - diag_half,
                self.center.y - offset - diag_half,
                diag_size,
                diag_size,
            ),
            DPadDirection::DownLeft => (
                self.center.x - offset - diag_half,
                self.center.y + offset - diag_half,
                diag_size,
                diag_size,
            ),
            DPadDirection::DownRight => (
                self.center.x + offset - diag_half,
                self.center.y + offset - diag_half,
                diag_size,
                diag_size,
            ),
            DPadDirection::None => (
                self.center.x - half,
                self.center.y - half,
                self.button_size,
                self.button_size,
            ),
        }
    }

    /// Check which direction a point is in
    fn hit_test(&self, x: f32, y: f32) -> DPadDirection {
        // Check diagonals first (smaller, in corners)
        for dir in [
            DPadDirection::UpLeft,
            DPadDirection::UpRight,
            DPadDirection::DownLeft,
            DPadDirection::DownRight,
        ] {
            let (rx, ry, rw, rh) = self.get_button_rect(dir);
            if x >= rx && x < rx + rw && y >= ry && y < ry + rh {
                return dir;
            }
        }
        // Then check cardinals
        for dir in [
            DPadDirection::Up,
            DPadDirection::Down,
            DPadDirection::Left,
            DPadDirection::Right,
        ] {
            let (rx, ry, rw, rh) = self.get_button_rect(dir);
            if x >= rx && x < rx + rw && y >= ry && y < ry + rh {
                return dir;
            }
        }
        DPadDirection::None
    }

    /// Update D-pad state based on touch input
    /// Returns true if this D-pad consumed the touch
    pub fn update(&mut self, touches: &[Touch], current_time: f64) -> bool {
        let (screen_w, _) = virtual_screen_size();
        self.just_released_dir = DPadDirection::None;

        // If we're tracking a touch, update or release it
        if let Some(tracking_id) = self.touch_id {
            let tracked = touches.iter().find(|t| t.id == tracking_id);

            match tracked {
                Some(touch) => {
                    match touch.phase {
                        TouchPhase::Moved | TouchPhase::Stationary => {
                            let (vx, vy) = to_virtual_coords(touch.position.x, touch.position.y);
                            let new_dir = self.hit_test(vx, vy);

                            // Direction changed while touching
                            if new_dir != self.current_dir {
                                if new_dir != DPadDirection::None {
                                    // Moved to new direction
                                    self.current_dir = new_dir;
                                    if !self.move_sent {
                                        // Restart timer if we haven't started moving yet
                                        self.press_time = current_time;
                                    }
                                } else {
                                    // Moved off all buttons but still touching
                                    // Keep current direction for now
                                }
                            }
                        }
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            self.just_released_dir = self.current_dir;
                            self.release();
                        }
                        _ => {}
                    }
                }
                None => {
                    self.just_released_dir = self.current_dir;
                    self.release();
                }
            }
            return true;
        }

        // Look for a new touch on the D-pad (left side of screen)
        for touch in touches {
            if touch.phase == TouchPhase::Started {
                let (x, y) = to_virtual_coords(touch.position.x, touch.position.y);

                // Only respond to touches on left half of screen
                if x < screen_w * 0.5 {
                    let dir = self.hit_test(x, y);
                    if dir != DPadDirection::None {
                        self.touch_id = Some(touch.id);
                        self.current_dir = dir;
                        self.press_time = current_time;
                        self.move_sent = false;

                        // Check if this same touch also ended in this frame (very quick tap)
                        // This happens when Started and Ended occur in the same frame
                        let also_ended = touches.iter().any(|t| {
                            t.id == touch.id
                                && matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled)
                        });
                        if also_ended {
                            self.just_released_dir = dir;
                            self.release();
                        }

                        return true;
                    }
                }
            }
        }

        false
    }

    /// Release the D-pad
    fn release(&mut self) {
        self.touch_id = None;
        self.current_dir = DPadDirection::None;
        // move_sent is intentionally NOT cleared here - the input handler
        // reads it on release to decide whether to send a stop command.
    }

    /// Get current direction (None if not pressed)
    pub fn get_direction(&self) -> DPadDirection {
        self.current_dir
    }

    /// Get the direction that was just released (for face command detection)
    pub fn get_just_released(&self) -> DPadDirection {
        self.just_released_dir
    }

    /// Get press time for threshold checking
    pub fn get_press_time(&self) -> f64 {
        self.press_time
    }

    /// Mark that a move command was sent (held past threshold)
    pub fn set_move_sent(&mut self, sent: bool) {
        self.move_sent = sent;
    }

    /// Check if move was sent (held past threshold)
    pub fn was_move_sent(&self) -> bool {
        self.move_sent
    }

    /// Check if D-pad is active
    pub fn is_active(&self) -> bool {
        self.touch_id.is_some()
    }

    /// Render the D-pad
    pub fn render(&self) {
        let all_directions = [
            (DPadDirection::Up, "^"),
            (DPadDirection::Down, "v"),
            (DPadDirection::Left, "<"),
            (DPadDirection::Right, ">"),
            (DPadDirection::UpLeft, "\\"),
            (DPadDirection::UpRight, "/"),
            (DPadDirection::DownLeft, "/"),
            (DPadDirection::DownRight, "\\"),
        ];

        for (dir, arrow) in &all_directions {
            let (x, y, w, h) = self.get_button_rect(*dir);
            let is_pressed = self.current_dir == *dir;

            // Button background
            let bg_color = if is_pressed {
                Color::new(1.0, 1.0, 1.0, 0.4)
            } else {
                Color::new(1.0, 1.0, 1.0, 0.15)
            };
            draw_rectangle(x, y, w, h, bg_color);

            // Button border
            let border_color = if is_pressed {
                Color::new(1.0, 1.0, 1.0, 0.8)
            } else {
                Color::new(1.0, 1.0, 1.0, 0.3)
            };
            draw_rectangle_lines(x, y, w, h, 2.0, border_color);

            // Arrow
            let font_size = 16.0;
            let text_dim = measure_text(arrow, None, font_size as u16, 1.0);
            let text_color = if is_pressed {
                WHITE
            } else {
                Color::new(0.95, 0.95, 0.95, 0.9)
            };
            draw_text(
                arrow,
                x + (w - text_dim.width) / 2.0,
                y + (h + text_dim.height) / 2.0,
                font_size,
                text_color,
            );
        }
    }
}

/// Virtual joystick for mobile controls
/// Touch-drag from a center point to determine direction
pub struct VirtualJoystick {
    /// Center position (set on touch start)
    center: Vec2,
    /// Current touch position
    touch_pos: Vec2,
    /// Dead zone radius (below this, no direction)
    dead_zone: f32,
    /// Display radius for the outer circle
    outer_radius: f32,
    /// Touch ID tracking input
    touch_id: Option<u64>,
    /// Current direction output
    current_dir: DPadDirection,
    /// Time when direction was first pressed
    press_time: f64,
    /// Whether a move command has been sent
    move_sent: bool,
    /// Direction that was just released
    just_released_dir: DPadDirection,
}

impl VirtualJoystick {
    pub fn new() -> Self {
        Self {
            center: Vec2::ZERO,
            touch_pos: Vec2::ZERO,
            dead_zone: 15.0,
            outer_radius: 60.0,
            touch_id: None,
            current_dir: DPadDirection::None,
            press_time: 0.0,
            move_sent: false,
            just_released_dir: DPadDirection::None,
        }
    }

    /// Map an angle to an 8-way direction
    fn angle_to_direction(dx: f32, dy: f32) -> DPadDirection {
        let angle = dy.atan2(dx);
        let pi = std::f32::consts::PI;
        // Divide circle into 8 sectors of 45 degrees each
        let sector = ((angle + pi) / (pi / 4.0)).floor() as i32 % 8;
        match sector {
            0 => DPadDirection::Left,      // -180 to -135
            1 => DPadDirection::UpLeft,    // -135 to -90
            2 => DPadDirection::Up,        // -90 to -45
            3 => DPadDirection::UpRight,   // -45 to 0
            4 => DPadDirection::Right,     // 0 to 45
            5 => DPadDirection::DownRight, // 45 to 90
            6 => DPadDirection::Down,      // 90 to 135
            7 => DPadDirection::DownLeft,  // 135 to 180
            _ => DPadDirection::None,
        }
    }

    /// Update joystick state based on touch input
    /// Returns true if this joystick consumed the touch
    pub fn update(&mut self, touches: &[Touch], current_time: f64) -> bool {
        let (screen_w, _) = virtual_screen_size();
        self.just_released_dir = DPadDirection::None;

        // If we're tracking a touch, update or release it
        if let Some(tracking_id) = self.touch_id {
            let tracked = touches.iter().find(|t| t.id == tracking_id);

            match tracked {
                Some(touch) => match touch.phase {
                    TouchPhase::Moved | TouchPhase::Stationary => {
                        let (vx, vy) = to_virtual_coords(touch.position.x, touch.position.y);
                        self.touch_pos = vec2(vx, vy);
                        let delta = self.touch_pos - self.center;
                        let dist = delta.length();
                        if dist > self.dead_zone {
                            let new_dir = Self::angle_to_direction(delta.x, delta.y);
                            if new_dir != self.current_dir && !self.move_sent {
                                self.press_time = current_time;
                            }
                            self.current_dir = new_dir;
                        } else {
                            self.current_dir = DPadDirection::None;
                        }
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        self.just_released_dir = self.current_dir;
                        self.release();
                    }
                    _ => {}
                },
                None => {
                    self.just_released_dir = self.current_dir;
                    self.release();
                }
            }
            return true;
        }

        // Look for a new touch on left half of screen
        for touch in touches {
            if touch.phase == TouchPhase::Started {
                let (x, y) = to_virtual_coords(touch.position.x, touch.position.y);
                if x < screen_w * 0.5 {
                    self.touch_id = Some(touch.id);
                    self.center = vec2(x, y);
                    self.touch_pos = vec2(x, y);
                    self.current_dir = DPadDirection::None;
                    self.press_time = current_time;
                    self.move_sent = false;

                    // Check for same-frame end
                    let also_ended = touches.iter().any(|t| {
                        t.id == touch.id
                            && matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled)
                    });
                    if also_ended {
                        self.just_released_dir = self.current_dir;
                        self.release();
                    }

                    return true;
                }
            }
        }

        false
    }

    fn release(&mut self) {
        self.touch_id = None;
        self.current_dir = DPadDirection::None;
        // move_sent is intentionally NOT cleared here - the input handler
        // reads it on release to decide whether to send a stop command.
    }

    pub fn get_direction(&self) -> DPadDirection {
        self.current_dir
    }

    pub fn get_just_released(&self) -> DPadDirection {
        self.just_released_dir
    }

    pub fn get_press_time(&self) -> f64 {
        self.press_time
    }

    pub fn set_move_sent(&mut self, sent: bool) {
        self.move_sent = sent;
    }

    pub fn was_move_sent(&self) -> bool {
        self.move_sent
    }

    pub fn is_active(&self) -> bool {
        self.touch_id.is_some()
    }

    /// Render the joystick (only when active)
    pub fn render(&self) {
        if let Some(_) = self.touch_id {
            // Outer circle
            draw_circle(
                self.center.x,
                self.center.y,
                self.outer_radius,
                Color::new(1.0, 1.0, 1.0, 0.1),
            );
            draw_circle_lines(
                self.center.x,
                self.center.y,
                self.outer_radius,
                2.0,
                Color::new(1.0, 1.0, 1.0, 0.25),
            );

            // Inner circle (thumb position, clamped to outer radius)
            let delta = self.touch_pos - self.center;
            let dist = delta.length();
            let clamped = if dist > self.outer_radius {
                self.center + delta.normalize() * self.outer_radius
            } else {
                self.touch_pos
            };
            let inner_alpha = if dist > self.dead_zone { 0.5 } else { 0.25 };
            draw_circle(
                clamped.x,
                clamped.y,
                20.0,
                Color::new(1.0, 1.0, 1.0, inner_alpha),
            );
            draw_circle_lines(
                clamped.x,
                clamped.y,
                20.0,
                2.0,
                Color::new(1.0, 1.0, 1.0, 0.6),
            );
        }
    }
}

/// Touch button for actions like attack
pub struct TouchButton {
    /// Position of the button
    pub position: Vec2,
    /// Radius of the button
    pub radius: f32,
    /// Whether the button is currently pressed
    pressed: bool,
    /// Whether the button was just pressed this frame
    just_pressed: bool,
    /// Touch ID tracking this button
    touch_id: Option<u64>,
    /// Label to display
    pub label: String,
    /// Optional icon texture
    pub icon: Option<Texture2D>,
    /// Optional source rect for atlas-based sprites
    pub icon_source: Option<Rect>,
    /// Sub-label drawn below the icon (e.g. "ATTACK")
    pub sub_label: Option<String>,
}

impl TouchButton {
    pub fn new(x: f32, y: f32, radius: f32, label: &str) -> Self {
        Self {
            position: vec2(x, y),
            radius,
            pressed: false,
            just_pressed: false,
            touch_id: None,
            label: label.to_string(),
            icon: None,
            icon_source: None,
            sub_label: None,
        }
    }

    /// Update button position (call each frame to handle screen resize)
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.position = vec2(x, y);
    }

    /// Update button state based on touch input
    /// Returns true if this button consumed the touch
    pub fn update(&mut self, touches: &[Touch]) -> bool {
        self.just_pressed = false;

        // If we're tracking a touch, check if it's still valid
        if let Some(tracking_id) = self.touch_id {
            let tracked = touches.iter().find(|t| t.id == tracking_id);

            match tracked {
                Some(touch) => match touch.phase {
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        self.pressed = false;
                        self.touch_id = None;
                    }
                    _ => {}
                },
                None => {
                    self.pressed = false;
                    self.touch_id = None;
                }
            }
            return self.pressed;
        }

        // Look for a new touch on this button
        for touch in touches {
            if touch.phase == TouchPhase::Started {
                let (vx, vy) = to_virtual_coords(touch.position.x, touch.position.y);
                let touch_pos = vec2(vx, vy);
                let distance = (touch_pos - self.position).length();

                if distance <= self.radius {
                    self.touch_id = Some(touch.id);
                    self.pressed = true;
                    self.just_pressed = true;
                    return true;
                }
            }
        }

        false
    }

    /// Check if button is currently pressed
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Check if button was just pressed this frame
    pub fn just_pressed(&self) -> bool {
        self.just_pressed
    }

    /// Render the button (themed to match game UI)
    pub fn render(&self) {
        let base_alpha = if self.pressed { 0.55 } else { 0.4 };

        let (bg, border, highlight) = if self.pressed {
            (
                Color::new(0.180, 0.200, 0.145, base_alpha),
                Color::new(0.855, 0.698, 0.424, base_alpha + 0.1),
                Color::new(0.855, 0.698, 0.424, 0.2),
            )
        } else {
            (
                Color::new(0.086, 0.086, 0.118, base_alpha),
                Color::new(0.227, 0.212, 0.188, base_alpha),
                Color::new(0.0, 0.0, 0.0, 0.0),
            )
        };

        // Outer border circle
        draw_circle(self.position.x, self.position.y, self.radius, border);
        // Inner background
        draw_circle(self.position.x, self.position.y, self.radius - 2.0, bg);
        // Press highlight
        if self.pressed {
            draw_circle(
                self.position.x,
                self.position.y,
                self.radius - 4.0,
                highlight,
            );
        }

        let content_alpha = if self.pressed { 0.7 } else { 0.55 };

        // Draw icon if available, otherwise fall back to text label
        if let Some(tex) = &self.icon {
            let has_sub = self.sub_label.is_some();
            let icon_size = if has_sub {
                self.radius * 1.0
            } else {
                self.radius * 1.2
            };
            let icon_y_offset = if has_sub { -8.0 } else { 0.0 };
            draw_texture_ex(
                tex,
                self.position.x - icon_size / 2.0,
                self.position.y - icon_size / 2.0 + icon_y_offset,
                Color::new(1.0, 1.0, 1.0, content_alpha),
                DrawTextureParams {
                    dest_size: Some(vec2(icon_size, icon_size)),
                    source: self.icon_source.map(|r| Rect::new(r.x, r.y, r.w, r.h)),
                    ..Default::default()
                },
            );

            // Draw sub-label inside the circle, below the icon
            if let Some(sub) = &self.sub_label {
                let font_size = 16.0;
                let text_dim = measure_text(sub, None, font_size as u16, 1.0);
                let text_color = if self.pressed {
                    Color::new(0.855, 0.698, 0.424, content_alpha)
                } else {
                    Color::new(0.780, 0.740, 0.680, content_alpha)
                };
                draw_text(
                    sub,
                    self.position.x - text_dim.width / 2.0,
                    self.position.y + self.radius - 14.0,
                    font_size,
                    text_color,
                );
            }
        } else {
            let font_size = 16.0;
            let text_dim = measure_text(&self.label, None, font_size as u16, 1.0);
            let text_color = if self.pressed {
                Color::new(0.855, 0.698, 0.424, content_alpha)
            } else {
                Color::new(0.780, 0.740, 0.680, content_alpha)
            };
            draw_text(
                &self.label,
                self.position.x - text_dim.width / 2.0,
                self.position.y + text_dim.height / 4.0,
                font_size,
                text_color,
            );
        }
    }
}

/// Container for all touch controls
pub struct TouchControls {
    pub dpad: VirtualDPad,
    pub joystick: VirtualJoystick,
    pub attack_button: TouchButton,
    pub interact_button: TouchButton,
    /// Whether touch controls are enabled (typically only on Android)
    pub enabled: bool,
    /// Whether any touch was consumed by controls this frame
    touch_consumed: bool,
}

impl TouchControls {
    pub fn new() -> Self {
        let (screen_w, screen_h) = virtual_screen_size();

        // Position buttons on the right side of the screen
        #[cfg(target_os = "android")]
        let (attack_x, attack_y, interact_x, interact_y) = (
            screen_w - 42.0,
            screen_h - 120.0,
            screen_w - 100.0,
            screen_h - 72.0,
        );
        #[cfg(not(target_os = "android"))]
        let (attack_x, attack_y, interact_x, interact_y) = (
            screen_w - 55.0,
            screen_h - 130.0,
            screen_w - 115.0,
            screen_h - 85.0,
        );

        Self {
            dpad: VirtualDPad::new(),
            joystick: VirtualJoystick::new(),
            attack_button: TouchButton::new(attack_x, attack_y, 40.0, "ATTACK"),
            interact_button: TouchButton::new(interact_x, interact_y, 32.0, "USE"),
            #[cfg(target_os = "android")]
            enabled: true,
            #[cfg(not(target_os = "android"))]
            enabled: false,
            touch_consumed: false,
        }
    }

    /// Load icon textures for touch buttons (fallback only — weapon icon is set dynamically)
    pub async fn load_icons(&mut self) {
        // No static attack icon — it's set dynamically from equipped weapon
    }

    /// Update the attack button icon to show the currently equipped weapon
    pub fn update_attack_icon(
        &mut self,
        weapon_id: Option<&str>,
        item_sprites: &crate::render::SpriteStore,
    ) {
        if let Some(id) = weapon_id {
            if let Some((tex, source_rect)) = item_sprites.get(id) {
                self.attack_button.icon = Some(tex.clone());
                self.attack_button.icon_source = source_rect;
                self.attack_button.sub_label = Some("ATTACK".to_string());
                return;
            }
        }
        // No weapon equipped or sprite not found — clear icon, show label
        self.attack_button.icon = None;
        self.attack_button.icon_source = None;
        self.attack_button.sub_label = None;
    }

    /// Update all touch controls
    /// Set hide_action_buttons to true when panels like inventory are open
    pub fn update(
        &mut self,
        current_time: f64,
        hide_action_buttons: bool,
        hide_all_controls: bool,
        use_joystick: bool,
    ) {
        self.touch_consumed = false;

        if !self.enabled {
            return;
        }

        // Update positions for current virtual screen size
        let (screen_w, screen_h) = virtual_screen_size();
        #[cfg(target_os = "android")]
        {
            self.attack_button
                .set_position(screen_w - 42.0, screen_h - 120.0);
            self.interact_button
                .set_position(screen_w - 100.0, screen_h - 72.0);
        }
        #[cfg(not(target_os = "android"))]
        {
            self.attack_button
                .set_position(screen_w - 55.0, screen_h - 130.0);
            self.interact_button
                .set_position(screen_w - 115.0, screen_h - 85.0);
        }
        self.dpad.update_position();

        // Get all current touches
        let touches: Vec<Touch> = touches();

        // Always update controls so they can process touch-end events and clear state.
        // Without this, hiding controls while a touch is active leaves them permanently
        // "active", causing touch_consumed to stay true and blocking all UI clicks.
        let attack_consumed = self.attack_button.update(&touches);
        let interact_consumed = self.interact_button.update(&touches);
        let direction_consumed = if use_joystick {
            self.joystick.update(&touches, current_time)
        } else {
            self.dpad.update(&touches, current_time)
        };

        // Only count as consumed when the controls are visible
        self.touch_consumed = (!hide_action_buttons
            && (attack_consumed
                || interact_consumed
                || self.attack_button.is_pressed()
                || self.interact_button.is_pressed()))
            || (!hide_all_controls
                && (direction_consumed || self.dpad.is_active() || self.joystick.is_active()));
    }

    /// Check if touch input was consumed by controls this frame
    /// Use this to prevent touch from triggering map clicks
    pub fn consumed_touch(&self) -> bool {
        self.touch_consumed
    }

    /// Render all touch controls
    /// Set hide_action_buttons to true when panels like inventory are open
    pub fn render(&self, hide_action_buttons: bool, hide_all_controls: bool, use_joystick: bool) {
        if !self.enabled {
            return;
        }

        if !hide_all_controls {
            if use_joystick {
                self.joystick.render();
            } else {
                self.dpad.render();
            }
        }
        if !hide_action_buttons {
            self.attack_button.render();
            self.interact_button.render();
        }
    }

    /// Get current direction (from whichever input mode is active)
    pub fn get_direction(&self) -> DPadDirection {
        if !self.enabled {
            return DPadDirection::None;
        }
        // Return from whichever is active
        if self.joystick.is_active() {
            self.joystick.get_direction()
        } else {
            self.dpad.get_direction()
        }
    }

    /// Get direction that was just released (for tap-to-face)
    pub fn get_just_released_direction(&self) -> DPadDirection {
        if !self.enabled {
            return DPadDirection::None;
        }
        let joy_rel = self.joystick.get_just_released();
        if joy_rel != DPadDirection::None {
            return joy_rel;
        }
        self.dpad.get_just_released()
    }

    /// Get press time for threshold checking
    pub fn get_dpad_press_time(&self) -> f64 {
        if self.joystick.is_active() {
            self.joystick.get_press_time()
        } else {
            self.dpad.get_press_time()
        }
    }

    /// Mark that a move command was sent (only on the active control)
    pub fn set_dpad_move_sent(&mut self, sent: bool) {
        if self.joystick.get_direction() != DPadDirection::None {
            self.joystick.set_move_sent(sent);
        } else {
            self.dpad.set_move_sent(sent);
        }
    }

    /// Check if move was sent (held past threshold)
    pub fn was_dpad_move_sent(&self) -> bool {
        self.dpad.was_move_sent() || self.joystick.was_move_sent()
    }

    /// Check if attack button is held (for continuous attacking like space bar)
    pub fn attack_pressed(&self) -> bool {
        self.enabled && self.attack_button.is_pressed()
    }

    /// Check if interact was just pressed
    pub fn interact_pressed(&self) -> bool {
        self.enabled && self.interact_button.just_pressed()
    }
}
