//! Area banner UI - displays location name during map transitions

use crate::render::Renderer;
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

/// Banner display phase
#[derive(Debug, Clone, PartialEq)]
pub enum BannerPhase {
    Hidden,
    FadingIn,
    Holding,
    FadingOut,
}

/// Timing constants
const FADE_IN_DURATION: f32 = 0.5;
const HOLD_DURATION: f32 = 2.5;
const FADE_OUT_DURATION: f32 = 0.5;

/// Vertical travel of the slide animation, in virtual pixels. The banner rises
/// into place on the way in, then continues rising up and out on the way out.
const SLIDE_DISTANCE: f32 = 22.0;

/// Overworld display name
pub const OVERWORLD_NAME: &str = "Verdant Fields";

/// Area banner state
#[derive(Debug, Clone)]
pub struct AreaBanner {
    pub text: String,
    pub phase: BannerPhase,
    pub timer: f32,
}

impl Default for AreaBanner {
    fn default() -> Self {
        Self {
            text: String::new(),
            phase: BannerPhase::Hidden,
            timer: 0.0,
        }
    }
}

impl AreaBanner {
    /// Trigger the banner with a new area name
    pub fn show(&mut self, name: &str) {
        self.text = name.to_string();
        self.phase = BannerPhase::FadingIn;
        self.timer = FADE_IN_DURATION;
    }

    /// Update the banner timer, transitioning phases as needed
    pub fn update(&mut self, delta: f32) {
        if self.phase == BannerPhase::Hidden {
            return;
        }

        self.timer -= delta;

        if self.timer <= 0.0 {
            match self.phase {
                BannerPhase::FadingIn => {
                    self.phase = BannerPhase::Holding;
                    self.timer = HOLD_DURATION;
                }
                BannerPhase::Holding => {
                    self.phase = BannerPhase::FadingOut;
                    self.timer = FADE_OUT_DURATION;
                }
                BannerPhase::FadingOut => {
                    self.phase = BannerPhase::Hidden;
                    self.timer = 0.0;
                }
                BannerPhase::Hidden => {}
            }
        }
    }

    /// Get current opacity (0.0 to 1.0)
    pub fn opacity(&self) -> f32 {
        match self.phase {
            BannerPhase::Hidden => 0.0,
            BannerPhase::FadingIn => 1.0 - (self.timer / FADE_IN_DURATION),
            BannerPhase::Holding => 1.0,
            BannerPhase::FadingOut => self.timer / FADE_OUT_DURATION,
        }
    }

    /// Vertical slide offset in virtual pixels (positive = lower on screen).
    /// The banner starts below its resting spot and rises up into place while
    /// fading in, holds at rest, then keeps rising up and out while fading out.
    pub fn slide_offset(&self) -> f32 {
        match self.phase {
            BannerPhase::Hidden => 0.0,
            BannerPhase::FadingIn => {
                // progress 0 -> 1; ease-out so it decelerates into place.
                let p = 1.0 - (self.timer / FADE_IN_DURATION).clamp(0.0, 1.0);
                SLIDE_DISTANCE * (1.0 - ease_out_cubic(p))
            }
            BannerPhase::Holding => 0.0,
            BannerPhase::FadingOut => {
                // progress 0 -> 1; ease-in so it accelerates away upward.
                let p = 1.0 - (self.timer / FADE_OUT_DURATION).clamp(0.0, 1.0);
                -SLIDE_DISTANCE * ease_in_cubic(p)
            }
        }
    }

    /// Check if banner should be rendered
    pub fn is_visible(&self) -> bool {
        self.phase != BannerPhase::Hidden
    }
}

/// Cubic ease-out: fast start, gentle settle (1 - (1-t)^3).
fn ease_out_cubic(t: f32) -> f32 {
    let u = 1.0 - t;
    1.0 - u * u * u
}

/// Cubic ease-in: gentle start, accelerating finish (t^3).
fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}

impl Renderer {
    /// Render the area banner (called from main render loop).
    ///
    /// Style: an elegant title flourish with no background box — gold title text
    /// over a soft shadow, with a thin bronze divider beneath flanked by small
    /// diamond ornaments. Matches the game's bronze/gold medieval UI theme.
    pub fn render_area_banner(&self, text: &str, opacity: f32, slide_offset: f32) {
        let (screen_w, screen_h) = virtual_screen_size();

        // Colors (alpha scaled by the fade opacity).
        // Gold title matching the panel TEXT_TITLE accent: rgba(218, 188, 128).
        let title_color = Color::new(0.855, 0.737, 0.502, opacity);
        // Soft dark shadow for legibility over busy world tiles.
        let shadow_color = Color::new(0.05, 0.04, 0.03, opacity * 0.75);
        // Bronze divider matching FRAME_ACCENT: rgba(218, 178, 108).
        let divider_color = Color::new(0.855, 0.698, 0.424, opacity * 0.9);

        // Measure title.
        let font_size = 32.0;
        let text_dims = self.measure_text_sharp(text, font_size);

        let center_x = (screen_w / 2.0).floor();
        let title_top = (screen_h * 0.18 + slide_offset).floor();

        // --- Title text (centered, with a slightly offset shadow for depth) ---
        let text_x = (center_x - text_dims.width / 2.0).floor();
        let text_y = (title_top + text_dims.height).floor();
        self.draw_text_sharp(text, text_x + 2.0, text_y + 2.0, font_size, shadow_color);
        self.draw_text_sharp(text, text_x, text_y, font_size, title_color);

        // --- Divider flourish beneath the title ---
        let divider_y = (text_y + 10.0).floor();
        // Width tracks the title but is capped so short names still look balanced.
        let half_len = (text_dims.width * 0.5 + 24.0).min(220.0);
        let diamond_gap = 12.0; // space between line end and ornament
        let line_left = center_x - half_len;
        let line_right = center_x + half_len;

        // Soft shadow under the divider, then the bronze line itself.
        draw_line(
            line_left,
            divider_y + 1.0,
            line_right,
            divider_y + 1.0,
            1.0,
            shadow_color,
        );
        draw_line(line_left, divider_y, line_right, divider_y, 1.0, divider_color);

        // --- Diamond ornaments flanking the divider ---
        let diamond_r = 3.0;
        for cx in [line_left - diamond_gap, line_right + diamond_gap] {
            self.draw_diamond(cx, divider_y, diamond_r, shadow_color, 1.0); // shadow
            self.draw_diamond(cx, divider_y, diamond_r, divider_color, 0.0);
        }
    }

    /// Draw a small filled diamond (rotated square) centered at (cx, cy).
    /// `offset` nudges the shape for a drop shadow pass.
    fn draw_diamond(&self, cx: f32, cy: f32, r: f32, color: Color, offset: f32) {
        let cx = cx + offset;
        let cy = cy + offset;
        draw_triangle(
            vec2(cx, cy - r),
            vec2(cx + r, cy),
            vec2(cx, cy + r),
            color,
        );
        draw_triangle(
            vec2(cx, cy - r),
            vec2(cx - r, cy),
            vec2(cx, cy + r),
            color,
        );
    }
}
