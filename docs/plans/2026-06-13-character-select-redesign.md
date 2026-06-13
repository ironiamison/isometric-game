# Character Select Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Redesign the character select screen to match the login screen's bronze-bevel medieval theme — a card-based roster with level chips, a persistent "create" row, an attached conditional action bar, an inviting empty state, and the shared animated starfield background.

**Architecture:** Extract login's stateless frame/corner-accent drawing and a stateful starfield into shared modules so login and character-select share one source of truth (no drift). Rewrite `CharacterSelectScreen::render` + `update` around a single computed layout struct (mirroring login's `LoginLayout` pattern) so hit-testing and drawing can't diverge. Pure formatting/color helpers are unit-tested with TDD; the rendering is verified by build + manual visual check.

**Tech Stack:** Rust, Macroquad (immediate-mode drawing), BitmapFont pixel font. Client crate at `client/`.

**Design doc:** `docs/plans/2026-06-13-character-select-redesign-design.md`

---

## Conventions for every task

- All paths are relative to the repo root `/Users/samson/projects/isometric-game/.worktrees/character-select-redesign`.
- Run all `cargo` commands from `client/`.
- Build check command (fast, used after every rendering task): `cargo build`
- Unit-test command: `cargo test <test_name> -- --nocolor`
- Commit after each task with the message shown.
- The screens submodules (`login.rs`, `character_select.rs`) bring shared imports in via `use super::*;` (the parent `src/ui/screens.rs` re-exports macroquad prelude, `CharacterInfo`, `BitmapFont`, `draw_character_preview`, `get_input_state`, `point_in_rect`, `screen_to_virtual`, `virtual_screen_size`, `ScreenState`, `Screen`, etc.). Color constants come from `crate::render::ui::common`.

---

## Task 1: Pure helper — `format_played_time`

Formats `played_time` seconds into the compact `1m` / `9h 51m` / `149h 44m` form shown on each card (no "played" suffix — that's a separate dim label).

**Files:**
- Modify: `client/src/ui/screens/character_select.rs` (add helper + test module near the bottom of the file, before the `// Character Create Screen` divider at line ~890)

**Step 1: Write the failing test**

Add at the end of `character_select.rs` (before the `// ===` Character Create divider):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn played_time_formats() {
        assert_eq!(format_played_time(0), "0m");
        assert_eq!(format_played_time(59), "0m");
        assert_eq!(format_played_time(60), "1m");
        assert_eq!(format_played_time(9 * 3600 + 51 * 60), "9h 51m");
        assert_eq!(format_played_time(149 * 3600 + 44 * 60), "149h 44m");
        assert_eq!(format_played_time(3600), "1h 0m");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test played_time_formats -- --nocolor`
Expected: FAIL — `cannot find function format_played_time`.

**Step 3: Write minimal implementation**

Add as a free function in `character_select.rs` (after the `const MAX_CHARACTERS` line ~4):

```rust
/// Format played-time seconds as a compact `1m` / `9h 51m` / `149h 44m` string.
fn format_played_time(seconds: i64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test played_time_formats -- --nocolor`
Expected: PASS.

**Step 5: Commit**

```bash
git add client/src/ui/screens/character_select.rs
git commit -m "feat: add format_played_time helper for character cards"
```

---

## Task 2: Pure helper — `level_chip_color` (bronze→gold ramp)

The level chip's fill tints from dark bronze (low level) to bright gold (high level). Use a clamped linear interpolation between `FRAME_OUTER` (rgba 82,62,42) at level 1 and `FRAME_ACCENT` (rgba 218,178,108) at level 100+. This keeps the chip fully inside the theme palette.

**Files:**
- Modify: `client/src/ui/screens/character_select.rs`

**Step 1: Write the failing test**

Add to the `tests` module:

```rust
#[test]
fn level_chip_ramp_endpoints_and_clamp() {
    // Level 1 → bronze (FRAME_OUTER); level >=100 → gold (FRAME_ACCENT)
    let low = level_chip_color(1);
    assert!((low.r - 0.322).abs() < 0.01 && (low.g - 0.243).abs() < 0.01);
    let high = level_chip_color(100);
    assert!((high.r - 0.855).abs() < 0.01 && (high.g - 0.698).abs() < 0.01);
    // Clamps below 1 and above 100
    let clamped_low = level_chip_color(0);
    assert!((clamped_low.r - low.r).abs() < 0.001);
    let clamped_high = level_chip_color(126);
    assert!((clamped_high.r - high.r).abs() < 0.001);
    // Midpoint is strictly between the two
    let mid = level_chip_color(50);
    assert!(mid.r > low.r && mid.r < high.r);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test level_chip_ramp -- --nocolor`
Expected: FAIL — `cannot find function level_chip_color`.

**Step 3: Write minimal implementation**

Ensure `FRAME_OUTER` and `FRAME_ACCENT` are imported. At the top of `character_select.rs`, add (below `use super::*;` which is implicit — add an explicit `use`):

```rust
use crate::render::ui::common::{FRAME_ACCENT, FRAME_OUTER};
```

Then add the helper:

```rust
/// Fill color for a level chip: bronze at low levels warming to gold at high
/// levels. Linear ramp from level 1 (FRAME_OUTER) to level 100+ (FRAME_ACCENT).
fn level_chip_color(level: i32) -> Color {
    let t = ((level - 1) as f32 / 99.0).clamp(0.0, 1.0);
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    Color::new(
        lerp(FRAME_OUTER.r, FRAME_ACCENT.r),
        lerp(FRAME_OUTER.g, FRAME_ACCENT.g),
        lerp(FRAME_OUTER.b, FRAME_ACCENT.b),
        1.0,
    )
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test level_chip_ramp -- --nocolor`
Expected: PASS. Also run `cargo test -- --nocolor` to confirm Task 1 + 2 both pass.

**Step 5: Commit**

```bash
git add client/src/ui/screens/character_select.rs
git commit -m "feat: add level_chip_color bronze-to-gold ramp helper"
```

---

## Task 3: Extract shared panel-frame + corner-accent drawing into `common.rs`

Login defines `draw_panel_frame` and `draw_corner_accents` as private static methods (`client/src/ui/screens/login.rs:459` and `:512`). Move them verbatim into `crate::render::ui::common` as free functions so both screens share them. (The renderer has same-named `&self` methods in `render/renderer/ui_primitives.rs` — those are a separate concern; do NOT touch them. Reconciling all three is out of scope.)

**Files:**
- Modify: `client/src/render/ui/common.rs` (add `use macroquad::prelude::*;` drawing imports + two free fns)
- Modify: `client/src/ui/screens/login.rs` (delete the two static methods, import + call the common ones)

**Step 1: Add the free functions to `common.rs`**

At the top of `common.rs`, the only import is `use macroquad::prelude::Color;`. Replace it with `use macroquad::prelude::*;` so `draw_rectangle`/`draw_line` are available. Then append at the end of the file:

```rust
// ============================================================================
// Shared screen chrome (used by login + character select)
// ============================================================================

/// Multi-layer bronze/gold panel frame with an opaque dark backing — the
/// "front door" look used by full-screen menus (login, character select).
pub fn draw_panel_frame(x: f32, y: f32, w: f32, h: f32) {
    // Drop shadow for depth against the night sky
    draw_rectangle(x - 3.0, y - 3.0, w + 6.0, h + 6.0, Color::new(0.0, 0.0, 0.0, 0.5));
    // Dark bronze outer frame
    draw_rectangle(x, y, w, h, FRAME_OUTER);
    // Mid bronze frame (inset 2px)
    draw_rectangle(x + 2.0, y + 2.0, w - 4.0, h - 4.0, FRAME_MID);
    // Main panel backing (inset by frame thickness) — solid dark "front door"
    draw_rectangle(
        x + FRAME_THICKNESS,
        y + FRAME_THICKNESS,
        w - FRAME_THICKNESS * 2.0,
        h - FRAME_THICKNESS * 2.0,
        PANEL_BG_DARK,
    );
    // Inner highlight (top + left)
    draw_line(x + FRAME_THICKNESS, y + FRAME_THICKNESS, x + w - FRAME_THICKNESS, y + FRAME_THICKNESS, 1.0, FRAME_INNER);
    draw_line(x + FRAME_THICKNESS, y + FRAME_THICKNESS, x + FRAME_THICKNESS, y + h - FRAME_THICKNESS, 1.0, FRAME_INNER);
    // Inner shadow (bottom + right)
    let shadow = Color::new(0.0, 0.0, 0.0, 0.235);
    draw_line(x + FRAME_THICKNESS + 1.0, y + h - FRAME_THICKNESS - 1.0, x + w - FRAME_THICKNESS, y + h - FRAME_THICKNESS - 1.0, 1.0, shadow);
    draw_line(x + w - FRAME_THICKNESS - 1.0, y + FRAME_THICKNESS + 1.0, x + w - FRAME_THICKNESS - 1.0, y + h - FRAME_THICKNESS, 1.0, shadow);
}

/// Gold L-shaped corner accents for the panel frame.
pub fn draw_corner_accents(x: f32, y: f32, w: f32, h: f32) {
    let s = CORNER_ACCENT_SIZE + 4.0;
    // Top-left
    draw_rectangle(x, y, s, 2.0, FRAME_ACCENT);
    draw_rectangle(x, y, 2.0, s, FRAME_ACCENT);
    // Top-right
    draw_rectangle(x + w - s, y, s, 2.0, FRAME_ACCENT);
    draw_rectangle(x + w - 2.0, y, 2.0, s, FRAME_ACCENT);
    // Bottom-left
    draw_rectangle(x, y + h - 2.0, s, 2.0, FRAME_ACCENT);
    draw_rectangle(x, y + h - s, 2.0, s, FRAME_ACCENT);
    // Bottom-right
    draw_rectangle(x + w - s, y + h - 2.0, s, 2.0, FRAME_ACCENT);
    draw_rectangle(x + w - 2.0, y + h - s, 2.0, s, FRAME_ACCENT);
}
```

**Step 2: Switch login.rs to the shared functions**

In `client/src/ui/screens/login.rs`:
- Delete the `fn draw_panel_frame(...)` method (lines ~457–509) and the `fn draw_corner_accents(...)` method (lines ~511–526).
- Add `draw_panel_frame` and `draw_corner_accents` to the `use crate::render::ui::common::{...}` import block (lines ~2–4).
- Change the two call sites (lines ~885–886) from `Self::draw_panel_frame(...)` / `Self::draw_corner_accents(...)` to `draw_panel_frame(...)` / `draw_corner_accents(...)`.

**Step 3: Build to verify no behavior change**

Run: `cargo build`
Expected: compiles clean. Login screen is byte-for-byte visually identical (same code, relocated).

**Step 4: Commit**

```bash
git add client/src/render/ui/common.rs client/src/ui/screens/login.rs
git commit -m "refactor: extract shared panel-frame + corner-accent drawing to common"
```

---

## Task 4: Extract the animated starfield into a shared `StarfieldBackground`

Pull login's star/shooting-star state + update + draw into a reusable struct so character select can render the same sky. Login keeps identical behavior.

**Files:**
- Create: `client/src/ui/screens/starfield.rs`
- Modify: `client/src/ui/screens.rs` (add `mod starfield;` + re-export)
- Modify: `client/src/ui/screens/login.rs` (replace inline star state/update/draw with the struct)

**Step 1: Create the starfield module**

Create `client/src/ui/screens/starfield.rs`:

```rust
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
    length: f32,
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
        let sa = alpha;
        if sa <= 0.001 {
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
            draw_rectangle(0.0, y, sw, h, Color::from_rgba(r, g, b, (255.0 * sa) as u8));
        }

        for &(sx, sy, phase) in &self.stars {
            let a = (((t * 1.5 + phase).sin() * 0.5 + 0.5) * 0.9 + 0.1) * sa;
            let size = if a > 0.7 * sa { 2.0 } else { 1.0 };
            draw_rectangle(sx * sw, sy * sh, size, size, Color::new(1.0, 1.0, 0.95, a));
        }

        for s in &self.shooting_stars {
            let a = s.life.min(1.0) * sa;
            let speed = (s.vx * s.vx + s.vy * s.vy).sqrt();
            let dx = -s.vx / speed * s.length;
            let dy = -s.vy / speed * s.length;
            draw_line(s.x, s.y, s.x + dx * 0.3, s.y + dy * 0.3, 2.0, Color::new(1.0, 1.0, 1.0, a));
            draw_line(s.x + dx * 0.3, s.y + dy * 0.3, s.x + dx, s.y + dy, 1.0, Color::new(0.8, 0.85, 1.0, a * 0.4));
        }
    }
}
```

**Step 2: Register the module**

In `client/src/ui/screens.rs`, near the other `mod` lines (~392–394) add:

```rust
mod starfield;
pub use starfield::StarfieldBackground;
```

**Step 3: Rewire login.rs to use it**

In `client/src/ui/screens/login.rs`:
- Remove the `struct ShootingStar { ... }` definition (lines ~40–48).
- Replace the three fields `frame_counter: f32`, `stars: Vec<(f32,f32,f32)>`, `shooting_stars: Vec<ShootingStar>` (lines ~86–88) with a single field `starfield: StarfieldBackground,` — BUT note `frame_counter` is also used elsewhere in login for ping timing (`last_ping_time` comparisons at lines ~698, ~733) and `t` in render. To avoid a larger refactor, KEEP `frame_counter` as its own field on `LoginScreen` and only move the star data. So:
  - Remove `stars` and `shooting_stars` fields; ADD `starfield: StarfieldBackground`.
  - Keep `frame_counter`.
- In `new()` (lines ~122–153): remove the star-generation loop and the `stars` / `shooting_stars` initializers; add `starfield: StarfieldBackground::new(),`.
- In `update()` (lines ~654–684): keep `self.frame_counter += dt;` (still needed for pings), and replace the shooting-star retain/spawn block with `self.starfield.update(dt, sw, sh);`. (Confirm `sw`/`sh` are in scope there; they are computed near the top of `update`. If not, compute `let (sw, sh) = virtual_screen_size();`.)
- In `render()` (lines ~822–876): replace the entire `let sa = self.stars_alpha; if sa > 0.001 { ... }` block with `self.starfield.draw(sw, sh, self.stars_alpha);`.
- Add `use super::StarfieldBackground;` (or rely on `use super::*;` — `screens.rs` re-exports it, so `use super::*;` already covers it; no new import line needed).

**Step 4: Build and verify login unchanged**

Run: `cargo build`
Expected: compiles clean. Login's animated sky behaves exactly as before.

**Step 5: Commit**

```bash
git add client/src/ui/screens/starfield.rs client/src/ui/screens.rs client/src/ui/screens/login.rs
git commit -m "refactor: extract StarfieldBackground shared by login + char select"
```

---

## Task 5: Add a shared screen-button helper + danger color

A reusable button used by the action bar (Play / Delete / Logout) and the empty-state Create button, with three variants matching the design: Primary (gold-on-brown, like login's button), Danger (restrained red-brown), Neutral (navy + bronze border).

**Files:**
- Modify: `client/src/render/ui/common.rs` (add `ButtonVariant` enum + `draw_screen_button` free fn; add danger color consts)

**Step 1: Add danger color constants**

In `common.rs`, in the text/frame color area, add:

```rust
// Danger button (restrained red-brown — used by destructive actions)
pub const DANGER_BG: Color = Color::new(0.247, 0.106, 0.106, 1.0); // rgba(63, 27, 27)
pub const DANGER_BG_HOVER: Color = Color::new(0.353, 0.149, 0.149, 1.0); // rgba(90, 38, 38)
pub const DANGER_BORDER: Color = Color::new(0.667, 0.290, 0.290, 1.0); // rgba(170, 74, 74)
pub const DANGER_TEXT: Color = Color::new(0.847, 0.467, 0.467, 1.0); // rgba(216, 119, 119)
```

**Step 2: Add the button helper**

`draw_screen_button` needs the font to draw centered text, so it takes a `&crate::render::BitmapFont`. Append to `common.rs`:

```rust
#[derive(Clone, Copy, PartialEq)]
pub enum ButtonVariant {
    Primary, // gold on dark brown (default action)
    Danger,  // restrained red-brown (destructive)
    Neutral, // navy + bronze border (low emphasis)
}

/// Draw a labelled menu button with bronze-theme styling. Text is centered and
/// drawn at native 16px for crisp pixels.
pub fn draw_screen_button(
    font: &crate::render::BitmapFont,
    rect: macroquad::prelude::Rect,
    label: &str,
    hovered: bool,
    variant: ButtonVariant,
) {
    let (bg, border, text) = match variant {
        ButtonVariant::Primary => (
            if hovered { Color::from_rgba(64, 50, 28, 255) } else { Color::from_rgba(44, 34, 18, 255) },
            if hovered { TEXT_GOLD } else { FRAME_ACCENT },
            TEXT_TITLE,
        ),
        ButtonVariant::Danger => (
            if hovered { DANGER_BG_HOVER } else { DANGER_BG },
            DANGER_BORDER,
            DANGER_TEXT,
        ),
        ButtonVariant::Neutral => (
            if hovered { Color::from_rgba(36, 36, 52, 255) } else { Color::from_rgba(24, 24, 36, 255) },
            if hovered { FRAME_INNER } else { FRAME_OUTER },
            TEXT_NORMAL,
        ),
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, border);
    let tw = font.measure_text(label, 16.0).width;
    font.draw_text(
        label,
        (rect.x + (rect.w - tw) / 2.0).floor(),
        (rect.y + rect.h / 2.0 + 6.0).floor(),
        16.0,
        text,
    );
}
```

(Confirm `BitmapFont` is re-exported from `crate::render` — login uses `crate::render::BitmapFont`. Confirm `BitmapFont::measure_text(&self, text, size) -> TextDimensions` and `draw_text(&self, text, x, y, size, color)` signatures match the calls in `character_select.rs:220-225`. They do.)

**Step 3: Build**

Run: `cargo build`
Expected: compiles clean (helper unused so far — that's fine; if the unused-warning is denied anywhere, it isn't: the crate has many warnings).

**Step 4: Commit**

```bash
git add client/src/render/ui/common.rs
git commit -m "feat: add shared draw_screen_button with Primary/Danger/Neutral variants"
```

---

## Task 6: Introduce a computed `CharacterSelectLayout`

Mirror login's `LoginLayout` pattern: compute every rect/baseline once from screen size so `update` (hit-testing) and `render` (drawing) share identical geometry. This replaces the current scattered, duplicated layout math (e.g. `list_w`/`list_x`/`inst_y` computed separately in both methods) and is the backbone for the new card + action-bar layout.

**Files:**
- Modify: `client/src/ui/screens/character_select.rs`

**Step 1: Define the layout struct + compute fn**

Add near the top of `character_select.rs` (after the helpers from Tasks 1–2):

```rust
const CARD_HEIGHT: f32 = 64.0;
const CARD_GAP: f32 = 6.0;
const ACTION_BAR_H: f32 = 44.0;

struct CharSelectLayout {
    panel: Rect,        // bronze-framed roster panel
    list_x: f32,        // inner content x (inside frame padding)
    list_w: f32,        // inner content width
    list_top: f32,      // first card y (inside frame)
    list_visible_h: f32,// clip height for the scrollable list
    action_bar: Rect,   // full action-bar row below the panel
    has_characters: bool,
}

impl CharSelectLayout {
    fn compute(sw: f32, sh: f32, has_characters: bool) -> Self {
        let panel_w = 540.0_f32.min(sw - 24.0);
        let panel_x = (sw - panel_w) / 2.0;
        let panel_top = 56.0; // below the header row
        let action_h = ACTION_BAR_H;
        let action_gap = 12.0;
        let bottom_margin = 36.0; // room for hint line
        let panel_bottom = sh - bottom_margin - action_h - action_gap;
        let panel_h = (panel_bottom - panel_top).max(160.0);
        let panel = Rect::new(panel_x, panel_top, panel_w, panel_h);

        let pad = FRAME_THICKNESS + 10.0;
        let list_x = panel_x + pad;
        let list_w = panel_w - pad * 2.0;
        let list_top = panel_top + pad;
        let list_visible_h = panel_h - pad * 2.0;

        let action_bar = Rect::new(panel_x, panel.y + panel.h + action_gap, panel_w, action_h);

        Self { panel, list_x, list_w, list_top, list_visible_h, action_bar, has_characters }
    }
}
```

Add `use crate::render::ui::common::{... , FRAME_THICKNESS, ...};` and `use macroquad::prelude::Rect;` if not already pulled by `use super::*;` (it is — `Rect` comes via macroquad prelude).

**Step 2: Build**

Run: `cargo build`
Expected: compiles clean (struct unused until Task 7 — acceptable warning).

**Step 3: Commit**

```bash
git add client/src/ui/screens/character_select.rs
git commit -m "feat: add CharSelectLayout for shared char-select geometry"
```

---

## Task 7: Rewrite `render` — header, framed panel, animated background

Replace the `render` body. This task draws everything EXCEPT the cards and action bar (done in Tasks 8–9) so we can verify the frame in isolation. Add a `starfield: StarfieldBackground` field to the struct first.

**Files:**
- Modify: `client/src/ui/screens/character_select.rs`

**Step 1: Add starfield field + init**

- Add field to `CharacterSelectScreen` struct: `starfield: StarfieldBackground,`
- In `new()`, add `starfield: StarfieldBackground::new(),`
- (`StarfieldBackground` is in scope via `use super::*;`.)

**Step 2: Replace the background + title block in `render`**

At the top of `render`, replace the existing background block (lines ~521–543) with:

```rust
let (sw, sh) = virtual_screen_size();
let (input_pos, _, _) = get_input_state();
let (mx, my) = (input_pos.x, input_pos.y);
let l = CharSelectLayout::compute(sw, sh, !self.characters.is_empty());

// Background
if self.has_spectator_backdrop {
    draw_rectangle(0.0, 0.0, sw, sh, Color::from_rgba(15, 15, 25, 160));
} else {
    self.starfield.draw(sw, sh, 1.0);
}

// Header row: username (left) + centered title
let header_y = 32.0;
self.draw_text_sharp(
    &format!("{}", self.session.username),
    24.0,
    header_y,
    16.0,
    TEXT_DIM,
);
let title = "SELECT CHARACTER";
let title_w = self.measure_text_sharp(title, 16.0).width;
self.draw_text_sharp(title, ((sw - title_w) / 2.0).floor(), header_y, 16.0, TEXT_TITLE);

// Bronze-framed roster panel
draw_panel_frame(l.panel.x, l.panel.y, l.panel.w, l.panel.h);
draw_corner_accents(l.panel.x, l.panel.y, l.panel.w, l.panel.h);
```

Add the needed imports at the top of the file:
`use crate::render::ui::common::{draw_panel_frame, draw_corner_accents, draw_screen_button, ButtonVariant, TEXT_TITLE, TEXT_DIM, TEXT_NORMAL, FRAME_ACCENT, FRAME_OUTER, FRAME_INNER, FRAME_MID, FRAME_THICKNESS, PANEL_BG_DARK};`

**Step 3: Stub out the rest temporarily**

Comment out / remove the remainder of the old `render` body (the old list loop, old buttons, scrollbar, old delete overlay) for now — it will be rebuilt in Tasks 8–10. Keep the delete-confirmation overlay block intact for now (Task 10 reskins it) by leaving it after the new code, but it must compile (it references `inst_y`, `list_x`, etc. that no longer exist). To keep the build green, the simplest path: leave the old `update` untouched (Task 11 rewrites it) and in `render`, after the frame, early-stub: draw nothing else yet. Delete the old body lines that reference removed locals.

> Implementation note for the executor: it's cleanest to rewrite `render` wholesale across Tasks 7→8→9→10 in one editing pass but COMMIT at each checkpoint. If a partial state won't compile, keep the old `update` and only fully swap `render` at Task 9. Prefer: do Tasks 7–10 edits together, build once, then make the per-task commits describe the layered change. Either way, end each task on a clean `cargo build`.

**Step 4: Build + visual check**

Run: `cargo build`
Then run the app to see the framed empty panel + header (see "Manual verification" at the end). Expected: bronze frame with gold corner brackets, animated stars behind, centered gold title, username top-left.

**Step 5: Commit**

```bash
git add client/src/ui/screens/character_select.rs
git commit -m "feat: char-select header + bronze panel + starfield background"
```

---

## Task 8: Draw character cards (portrait, name, level chip, meta, played time)

Render the scrollable list of cards inside the panel, preserving scissor-clipping + scrollbar + scroll offset. Each card uses the gold-lit selected state, hover brighten, and default recessed fill.

**Files:**
- Modify: `client/src/ui/screens/character_select.rs`

**Step 1: Add a `draw_character_card` method**

```rust
/// Draw one roster card at the given rect. Returns nothing; pure drawing.
fn draw_character_card(&self, rect: Rect, character: &CharacterInfo, selected: bool, hovered: bool) {
    // Card fill
    let fill = if selected {
        Color::from_rgba(46, 38, 22, 255) // warm gold-tinted recess
    } else if hovered {
        Color::from_rgba(30, 30, 42, 255)
    } else {
        PANEL_BG_DARK
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);

    // Border: gold when selected/focused, faint bronze otherwise
    if selected {
        // faint inner glow
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, Color::new(0.855, 0.698, 0.424, 0.06));
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, FRAME_ACCENT);
    } else if hovered {
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, FRAME_INNER);
    } else {
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, FRAME_OUTER);
    }

    // Portrait inset (recessed dark square with bronze edge)
    let inset = rect.h - 12.0;
    let inset_x = (rect.x + 8.0).floor();
    let inset_y = (rect.y + 6.0).floor();
    draw_rectangle(inset_x, inset_y, inset, inset, Color::from_rgba(12, 12, 18, 255));
    draw_rectangle_lines(inset_x, inset_y, inset, inset, 1.0, FRAME_OUTER);

    // Composite character sprite, centered in the inset
    let preview_x = (inset_x + (inset - SPRITE_WIDTH) / 2.0).floor();
    let preview_y = (inset_y + (inset - SPRITE_HEIGHT) / 2.0).floor();
    draw_character_preview(
        &self.player_sprites,
        &self.hair_sprites,
        &self.equipment_sprites,
        &character.gender,
        &character.skin,
        character.hair_style,
        character.hair_color.unwrap_or(0),
        character.sprite_body.as_deref(),
        character.sprite_back.as_deref(),
        character.sprite_feet.as_deref(),
        preview_x,
        preview_y,
    );

    // Text column
    let text_x = inset_x + inset + 12.0;
    let name_color = if selected { TEXT_TITLE } else { TEXT_NORMAL };
    self.draw_text_sharp(&character.name, text_x, rect.y + 26.0, 16.0, name_color);

    // Level chip
    let chip_label = format!("Lv {}", character.level);
    let chip_text_w = self.measure_text_sharp(&chip_label, 16.0).width;
    let chip_w = chip_text_w + 14.0;
    let chip_h = 18.0;
    let chip_x = text_x;
    let chip_y = rect.y + 36.0;
    draw_rectangle(chip_x, chip_y, chip_w, chip_h, level_chip_color(character.level));
    // dark text on the chip for contrast
    self.draw_text_sharp(&chip_label, chip_x + 7.0, chip_y + 14.0, 16.0, Color::from_rgba(20, 16, 10, 255));

    // Meta: gender + race/skin, after the chip
    let meta = format!("{} {}", title_case(&character.gender), title_case(&character.skin));
    self.draw_text_sharp(&meta, chip_x + chip_w + 8.0, chip_y + 14.0, 16.0, TEXT_DIM);

    // Played time (right-aligned), with dim "played" label beneath
    let time_str = format_played_time(character.played_time);
    let tw = self.measure_text_sharp(&time_str, 16.0).width;
    let right = rect.x + rect.w - 12.0;
    self.draw_text_sharp(&time_str, right - tw, rect.y + 26.0, 16.0, TEXT_NORMAL);
    let pl = "played";
    let plw = self.measure_text_sharp(pl, 16.0).width;
    self.draw_text_sharp(pl, right - plw, rect.y + 46.0, 16.0, TEXT_DIM);
}
```

Add a small `title_case` free helper (capitalizes first letter) and a unit test:

```rust
fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
```

Test (add to `tests` mod):
```rust
#[test]
fn title_case_caps_first() {
    assert_eq!(title_case("male"), "Male");
    assert_eq!(title_case("orc"), "Orc");
    assert_eq!(title_case(""), "");
}
```
Run: `cargo test title_case_caps_first -- --nocolor` → PASS.

**Step 2: Draw the list with clipping in `render`**

After the frame in `render`, when `!self.characters.is_empty()`, add scissor-clipped iteration over cards using `l.list_x`, `l.list_top`, `l.list_w`, `l.list_visible_h`, the existing `scroll_offset` math, and a per-card rect of height `CARD_HEIGHT` spaced by `CARD_HEIGHT + CARD_GAP`. Reuse the existing scissor + scrollbar code (lines ~568–687) but feed it the new layout values, and call `self.draw_character_card(card_rect, character, is_selected, is_hovered)` per visible card. The `+ Create new character` row is the next item after the last character (Task 9).

**Step 3: Build + visual check**

Run: `cargo build` then run the app with the `test` account (3 characters). Expected: cards with portraits, gold name on selected, bronze→gold level chips, `Male Tan` meta, right-aligned `9h 51m` / `played`.

**Step 4: Commit**

```bash
git add client/src/ui/screens/character_select.rs
git commit -m "feat: render character cards with portrait, level chip, meta, played time"
```

---

## Task 9: Persistent "+ Create new character" row + conditional action bar + empty state

**Files:**
- Modify: `client/src/ui/screens/character_select.rs`

**Step 1: Create row (in the list)**

After the character cards (still inside the clipped list), draw a create row at index `characters.len()` with the same card footprint but a dashed/faint bronze outline and centered dim-gold `+ Create new character`. Since Macroquad has no dashed-line primitive, approximate with a series of short `draw_line` segments along each edge, or use a single `draw_rectangle_lines` at 1px in `FRAME_OUTER` plus the `+` glyph — pick the dashed approximation:

```rust
fn draw_dashed_rect(x: f32, y: f32, w: f32, h: f32, color: Color) {
    let dash = 6.0;
    let gap = 4.0;
    let mut dx = x;
    while dx < x + w { let e = (dx + dash).min(x + w); draw_line(dx, y, e, y, 1.0, color); draw_line(dx, y + h, e, y + h, 1.0, color); dx += dash + gap; }
    let mut dy = y;
    while dy < y + h { let e = (dy + dash).min(y + h); draw_line(x, dy, x, e, 1.0, color); draw_line(x + w, dy, x + w, e, 1.0, color); dy += dash + gap; }
}
```

The create row is focusable: it is selectable index `== characters.len()` (Task 11 wires navigation/Enter to it). When focused, draw its outline in `FRAME_ACCENT` instead of `FRAME_OUTER`.

Note: total scrollable height must now include the create row: `(characters.len()+1) as f32 * (CARD_HEIGHT + CARD_GAP)`.

**Step 2: Action bar (below panel)**

Below the panel, in `l.action_bar`, lay out buttons with `draw_screen_button`:
- If `has_characters`: three buttons split across the bar width — `▶ Play` (Primary), `Delete` (Danger), `Logout` (Neutral). Compute three equal rects with small gaps. (Use plain text `Play` / `Delete` / `Logout`; the monogram font may not have glyphs for ▶/🗑/⎋ — verify, and if missing, use text-only labels. Default to text-only to be safe.)
- If empty: only a single `Logout` (Neutral) button, right-aligned within the bar (matches the mockup's bottom-right Logout).

**Step 3: Empty state (inside panel)**

When `self.characters.is_empty()`, instead of the list, draw a centered invitation inside the panel:
- A dashed circle (`draw_dashed_circle` approximation, or a simple `draw_circle_lines` in `FRAME_OUTER`) ~28px radius with a small `+`/person glyph centered.
- Gold headline `Your story begins here` (centered, 16px, `TEXT_TITLE`).
- Two dim lines: `No heroes yet. Create your first character` / `to set foot in the realm of Aeven.` (`TEXT_DIM`).
- One `+ Create Character` Primary button centered below.

**Step 4: Hint line (bottom, dim)**

Below the action bar, centered: `[W/S] navigate · [Enter] play · [N] new` when characters exist, else `[N] create character`. Use `TEXT_DIM`. Gate the W/S hint on `#[cfg(not(target_os = "android"))]` like the old code.

**Step 5: Build + visual check (both states)**

Run: `cargo build`. Verify State 1 (account `test`, 3 chars: full action bar + create row) and State 2 (new account, no chars: empty invitation + Logout-only). Use the two test accounts from the mockup or create a throwaway empty account.

**Step 6: Commit**

```bash
git add client/src/ui/screens/character_select.rs
git commit -m "feat: create row, conditional action bar, and empty-state invitation"
```

---

## Task 10: Reskin the delete-confirmation dialog to the bronze theme

**Files:**
- Modify: `client/src/ui/screens/character_select.rs`

**Step 1: Replace the overlay drawing**

In `render`, replace the existing confirm-delete overlay (lines ~709–775) so it uses `draw_panel_frame` + `draw_corner_accents` for the dialog box, `TEXT_TITLE`/`TEXT_NORMAL` for text, a Danger-variant confirm button (`Yes, delete`) and a Neutral cancel button (`No, cancel`) drawn via `draw_screen_button`. Keep the same box size/position (`box_w`, `box_h`, centered) and the same Yes/No hit rects so Task 11's `update` matches. Keep the `return;` after drawing the overlay.

**Step 2: Build + visual check**

Run: `cargo build`. Select a character, press Delete/X → bronze-framed dialog with red-brown confirm. Press N/Esc to cancel.

**Step 3: Commit**

```bash
git add client/src/ui/screens/character_select.rs
git commit -m "feat: reskin delete-confirm dialog to bronze theme"
```

---

## Task 11: Rewrite `update` hit-testing against the new layout

Align all mouse/keyboard/touch handling with the new geometry, the create row as a focusable list item, and the conditional action bar.

**Files:**
- Modify: `client/src/ui/screens/character_select.rs`

**Step 1: Recompute layout in `update`**

At the top of `update` (after the WASM polling block), replace the old layout constants (lines ~270–283) with `let l = CharSelectLayout::compute(sw, sh, !self.characters.is_empty());` and derive `item_height = CARD_HEIGHT + CARD_GAP`, `list_visible_height = l.list_visible_h`, etc. Update touch-scroll + wheel regions to use `l.list_x`/`l.list_top`/`l.list_w`/`l.list_visible_h`. Scrollable total height includes the +1 create row.

**Step 2: Selection range includes the create row**

`selected_index` now ranges over `0..=characters.len()` where the last value is the create row. Update:
- Up/Down (W/S) navigation clamps to `characters.len()` (inclusive) so focus can land on the create row.
- `Enter`: if `selected_index == characters.len()` → `ScreenState::ToCharacterCreate` (respecting `MAX_CHARACTERS`); else StartGame with the selected character.
- Clicking a card selects it; clicking the create row (or already-selected card) triggers its action. Double-click-to-play behavior preserved for character cards.
- Guard `StartGame`/Delete paths with `selected_index < characters.len()` so they never index the create row.

**Step 3: Action-bar hit rects**

Replace the old fixed button hit rects (lines ~434–469) with rects derived from `l.action_bar` exactly matching Task 9's layout (same three-equal-rects math, or single Logout when empty). Wire: Play→StartGame (only if a character is selected, i.e. `selected_index < characters.len()` and non-empty), Delete→`confirm_delete=true`, Logout→logout + `ToLogin`.

**Step 4: Keep keyboard shortcuts**

`N`→create (if `< MAX_CHARACTERS`), `Delete`/`X`→confirm delete (only if a real character is selected), `Esc`→logout, and the delete-confirm `Y`/`N`/`Esc` handling unchanged. Ensure `MAX_CHARACTERS` gating still hides create when full (the create row should not render / not be focusable when `characters.len() >= MAX_CHARACTERS` — skip it in both render and the selection range; clamp `selected_index` accordingly).

**Step 5: Build + full manual pass**

Run: `cargo build`. Then exercise every control (see Manual verification). Confirm focus wraps/clamps correctly, the create row is reachable by keyboard and mouse, action bar buttons work, empty state shows only Logout + the in-panel Create.

**Step 6: Commit**

```bash
git add client/src/ui/screens/character_select.rs
git commit -m "feat: rewrite char-select input handling for new layout + create row"
```

---

## Task 12: Cleanup, warnings, and final verification

**Files:**
- Modify: `client/src/ui/screens/character_select.rs` (remove any dead code / unused locals introduced during the rewrite)

**Steps:**
1. Run `cargo build` and check there are no NEW warnings attributable to this work (the crate has ~113 pre-existing warnings; don't add more — remove unused imports/locals you introduced).
2. Run `cargo test -- --nocolor` — all helper tests pass.
3. Full manual verification pass (below) on both states + delete dialog + WASM-agnostic paths.
4. Run `cargo build --target wasm32-unknown-unknown` if the WASM toolchain is set up (the screen has `#[cfg(target_arch = "wasm32")]` branches that must still compile). If the target isn't installed, note it and skip.
5. Commit:

```bash
git add -A
git commit -m "chore: tidy char-select redesign, remove dead code"
```

---

## Manual verification (used in Tasks 7–12)

Build and run the native client from `client/`:

```bash
cargo run
```

Log in with the `test` account (has characters) and verify **State 1**:
- Animated starfield behind a bronze panel with gold corner brackets.
- Username top-left, gold `SELECT CHARACTER` centered.
- Three character cards: portrait in a recessed inset, gold name when selected (faint warm glow + gold border), `Lv N` chip tinted by level (low=bronze, Lv 126=bright gold), `Male Tan`-style meta, right-aligned `9h 51m` over dim `played`.
- A dashed `+ Create new character` row at the bottom of the list, focusable.
- Attached action bar: Play / Delete (red-brown) / Logout.
- Hint line `[W/S] navigate · [Enter] play · [N] new`.
- Keyboard W/S moves focus through cards + create row; Enter plays / creates; N creates; Delete opens the bronze confirm dialog; Esc logs out. Mouse click selects, double-click plays, wheel scrolls; scrollbar appears when overflowing.

Log in with a fresh account (no characters) and verify **State 2**:
- Panel shows the centered invitation: dashed circle glyph, gold `Your story begins here`, dim two-line subtext, one `+ Create Character` button.
- Action bar shows ONLY Logout (bottom-right). Hint line `[N] create character`.

If anything looks off, capture it and iterate within the relevant task before moving on.

---

## Notes / Out of scope

- No server or protocol changes. No location/zone line (design decision).
- Do NOT modify the renderer's `&self` `draw_panel_frame`/`draw_corner_accents` in `render/renderer/ui_primitives.rs` — separate concern.
- Glyphs ▶/🗑/⎋ may be absent from the monogram font — default to text-only button labels; only add glyphs if verified present.
