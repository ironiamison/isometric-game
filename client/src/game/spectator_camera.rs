/// Cinematic camera that drifts smoothly through waypoints for the login screen spectator view.
/// Uses Catmull-Rom spline interpolation for continuous, jitter-free motion.
pub struct SpectatorCamera {
    waypoints: Vec<(f32, f32)>,
    current_index: usize,
    progress: f32, // 0.0 to 1.0 between current and next waypoint
    speed: f32,    // Progress per second (lower = slower drift)
}

impl Default for SpectatorCamera {
    fn default() -> Self {
        Self::new()
    }
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
            speed: 0.03, // ~33 seconds per segment (slower = smoother feel)
        }
    }

    /// Advance camera and return current (x, y) position.
    pub fn update(&mut self, dt: f32) -> (f32, f32) {
        self.progress += self.speed * dt;

        if self.progress >= 1.0 {
            self.progress -= 1.0;
            self.current_index = (self.current_index + 1) % self.waypoints.len();
        }

        self.catmull_rom_position(self.current_index, self.progress)
    }

    /// Get current position without advancing.
    pub fn position(&self) -> (f32, f32) {
        self.catmull_rom_position(self.current_index, self.progress)
    }

    /// Catmull-Rom spline interpolation through waypoints.
    /// Produces smooth, continuous curves with no velocity jumps at waypoint boundaries.
    fn catmull_rom_position(&self, index: usize, t: f32) -> (f32, f32) {
        let len = self.waypoints.len();
        let p0 = self.waypoints[(index + len - 1) % len];
        let p1 = self.waypoints[index];
        let p2 = self.waypoints[(index + 1) % len];
        let p3 = self.waypoints[(index + 2) % len];

        let t2 = t * t;
        let t3 = t2 * t;

        // Catmull-Rom basis functions
        let x = 0.5
            * ((2.0 * p1.0)
                + (-p0.0 + p2.0) * t
                + (2.0 * p0.0 - 5.0 * p1.0 + 4.0 * p2.0 - p3.0) * t2
                + (-p0.0 + 3.0 * p1.0 - 3.0 * p2.0 + p3.0) * t3);

        let y = 0.5
            * ((2.0 * p1.1)
                + (-p0.1 + p2.1) * t
                + (2.0 * p0.1 - 5.0 * p1.1 + 4.0 * p2.1 - p3.1) * t2
                + (-p0.1 + 3.0 * p1.1 - 3.0 * p2.1 + p3.1) * t3);

        (x, y)
    }
}
