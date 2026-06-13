//! Animated night-sky background (twinkling stars + shooting stars) shared by
//! the login and character-select screens.

use macroquad::prelude::*;

struct ShootingStar {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max_life: f32,
    length: f32, // trail length in pixels
}

/// Owns the star field state. Call `update(dt, sw, sh)` each frame and
/// `draw(sw, sh, alpha)` during render.
pub struct StarfieldBackground {
    frame_counter: f32,
    stars: Vec<(f32, f32, f32)>, // (x fraction, y fraction, phase)
    shooting_stars: Vec<ShootingStar>,
}

impl StarfieldBackground {
    pub fn new() -> Self {
        let mut stars = Vec::with_capacity(60);
        for i in 0..60 {
            let fi = i as f32;
            let x = ((fi * 137.5) % 1000.0) / 1000.0;
            let y = ((fi * 97.3 + 23.0) % 1000.0) / 1000.0;
            let phase = ((fi * 53.7) % 1000.0) / 1000.0 * std::f32::consts::TAU;
            stars.push((x, y, phase));
        }
        Self {
            frame_counter: 0.0,
            stars,
            shooting_stars: Vec::with_capacity(4),
        }
    }

    /// Advance animation. `dt` in seconds; `sw`/`sh` virtual screen size.
    pub fn update(&mut self, dt: f32, sw: f32, sh: f32) {
        self.frame_counter += dt;

        self.shooting_stars.retain_mut(|s| {
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            s.life -= dt / s.max_life;
            s.life > 0.0
        });

        if self.shooting_stars.len() < 2 {
            let pseudo = (self.frame_counter * 173.0) as u32;
            if pseudo.is_multiple_of(200) {
                let start_x = ((pseudo as f32 * 0.371) % 0.6 + 0.1) * sw;
                let start_y = ((pseudo as f32 * 0.529) % 0.2 + 0.02) * sh;
                let angle = 0.4 + ((pseudo as f32 * 0.213) % 0.4);
                let speed = 200.0 + ((pseudo as f32 * 0.617) % 150.0);
                let life = 0.6 + ((pseudo as f32 * 0.823) % 0.6);
                self.shooting_stars.push(ShootingStar {
                    x: start_x,
                    y: start_y,
                    vx: angle.cos() * speed,
                    vy: angle.sin() * speed,
                    life: 1.0,
                    max_life: life,
                    length: 20.0 + ((pseudo as f32 * 0.419) % 20.0),
                });
            }
        }
    }

    /// Draw the sky gradient + stars. `alpha` (0..1) fades the whole field;
    /// values <= 0.001 skip drawing entirely.
    pub fn draw(&self, sw: f32, sh: f32, alpha: f32) {
        if alpha <= 0.001 {
            return;
        }
        let t = self.frame_counter;

        let sky_steps = 20;
        for i in 0..sky_steps {
            let frac = i as f32 / sky_steps as f32;
            let r = (10.0 + frac * 15.0) as u8;
            let g = (12.0 + frac * 8.0) as u8;
            let b = (40.0 - frac * 10.0) as u8;
            let y = frac * sh;
            let h = sh / sky_steps as f32 + 1.0;
            draw_rectangle(
                0.0,
                y,
                sw,
                h,
                Color::from_rgba(r, g, b, (255.0 * alpha) as u8),
            );
        }

        for &(sx, sy, phase) in &self.stars {
            let a = (((t * 1.5 + phase).sin() * 0.5 + 0.5) * 0.9 + 0.1) * alpha;
            let size = if a > 0.7 * alpha { 2.0 } else { 1.0 };
            draw_rectangle(sx * sw, sy * sh, size, size, Color::new(1.0, 1.0, 0.95, a));
        }

        for s in &self.shooting_stars {
            let a = s.life.min(1.0) * alpha;
            let speed = (s.vx * s.vx + s.vy * s.vy).sqrt();
            let dx = -s.vx / speed * s.length;
            let dy = -s.vy / speed * s.length;
            draw_line(
                s.x,
                s.y,
                s.x + dx * 0.3,
                s.y + dy * 0.3,
                2.0,
                Color::new(1.0, 1.0, 1.0, a),
            );
            draw_line(
                s.x + dx * 0.3,
                s.y + dy * 0.3,
                s.x + dx,
                s.y + dy,
                1.0,
                Color::new(0.8, 0.85, 1.0, a * 0.4),
            );
        }
    }
}

impl Default for StarfieldBackground {
    fn default() -> Self {
        Self::new()
    }
}
