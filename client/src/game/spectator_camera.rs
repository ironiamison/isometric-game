/// Cinematic camera that drifts between waypoints for the login screen spectator view.
pub struct SpectatorCamera {
    waypoints: Vec<(f32, f32)>,
    current_index: usize,
    progress: f32, // 0.0 to 1.0 between current and next waypoint
    speed: f32,    // Progress per second (lower = slower drift)
}

impl SpectatorCamera {
    pub fn new() -> Self {
        // Gentle loop around spawn (15, 4) — radius ~12 tiles
        let waypoints = vec![
            (15.0, 4.0),  // Start at spawn
            (22.0, 8.0),  // Southeast
            (18.0, 14.0), // South
            (10.0, 10.0), // Southwest
            (6.0, 4.0),   // West
            (10.0, -2.0), // Northwest
            (18.0, -1.0), // North
        ];
        Self {
            waypoints,
            current_index: 0,
            progress: 0.0,
            speed: 0.04, // ~25 seconds per segment
        }
    }

    /// Advance camera and return current (x, y) position.
    pub fn update(&mut self, dt: f32) -> (f32, f32) {
        self.progress += self.speed * dt;

        if self.progress >= 1.0 {
            self.progress -= 1.0;
            self.current_index = (self.current_index + 1) % self.waypoints.len();
        }

        let (x0, y0) = self.waypoints[self.current_index];
        let next = (self.current_index + 1) % self.waypoints.len();
        let (x1, y1) = self.waypoints[next];

        // Smooth interpolation (ease in-out)
        let t = smooth_step(self.progress);
        (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
    }

    /// Get current position without advancing.
    pub fn position(&self) -> (f32, f32) {
        let (x0, y0) = self.waypoints[self.current_index];
        let next = (self.current_index + 1) % self.waypoints.len();
        let (x1, y1) = self.waypoints[next];
        let t = smooth_step(self.progress);
        (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
    }
}

fn smooth_step(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
