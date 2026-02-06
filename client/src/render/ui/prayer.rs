//! Prayer Book panel rendering - displays all prayers in a grid
//! Shows prayer icons with locked/available/active states
//! Click to toggle prayers (sends TogglePrayer message)

use macroquad::prelude::*;
use crate::game::GameState;
use crate::game::prayer::{PrayerCategory, PrayerDef, PRAYERS};
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use super::super::Renderer;
use super::common::*;

/// Prayer panel dimensions
const PRAYER_PANEL_PADDING: f32 = 8.0;
const PRAYER_GRID_COLS: usize = 4;
const PRAYER_GRID_ROWS: usize = 4; // 14 prayers + 2 empty slots
const PRAYER_SLOT_SIZE: f32 = 36.0;
const PRAYER_SLOT_SPACING: f32 = 4.0;
const PRAYER_GRID_WIDTH: f32 = PRAYER_GRID_COLS as f32 * PRAYER_SLOT_SIZE + (PRAYER_GRID_COLS - 1) as f32 * PRAYER_SLOT_SPACING;
const PRAYER_HEADER_HEIGHT: f32 = 24.0;
const PRAYER_POINTS_HEIGHT: f32 = 20.0;
const PRAYER_PANEL_WIDTH: f32 = PRAYER_GRID_WIDTH + PRAYER_PANEL_PADDING * 2.0 + FRAME_THICKNESS * 2.0;
const PRAYER_PANEL_HEIGHT: f32 = FRAME_THICKNESS * 2.0 + PRAYER_HEADER_HEIGHT + PRAYER_PANEL_PADDING +
    (PRAYER_GRID_ROWS as f32 * PRAYER_SLOT_SIZE + (PRAYER_GRID_ROWS - 1) as f32 * PRAYER_SLOT_SPACING) +
    PRAYER_PANEL_PADDING + PRAYER_POINTS_HEIGHT + PRAYER_PANEL_PADDING;

/// Get the color for a prayer category
fn category_color(category: PrayerCategory) -> Color {
    match category {
        PrayerCategory::Attack => Color::new(0.9, 0.3, 0.3, 1.0),      // Red
        PrayerCategory::Defence => Color::new(0.3, 0.5, 0.9, 1.0),     // Blue
        PrayerCategory::Strength => Color::new(0.3, 0.8, 0.3, 1.0),    // Green
        PrayerCategory::Gathering => Color::new(0.8, 0.7, 0.2, 1.0),   // Gold
        PrayerCategory::HpRegen => Color::new(0.9, 0.5, 0.7, 1.0),     // Pink
        PrayerCategory::Protection => Color::new(0.9, 0.9, 0.5, 1.0),  // Light yellow
    }
}

impl Renderer {
    /// Render the prayer book panel when open
    pub(crate) fn render_prayer_panel(&self, state: &GameState, hovered: &Option<UiElementId>, layout: &mut UiLayout) {
        if !state.ui_state.prayer_book_open {
            return;
        }

        let (screen_w, screen_h) = virtual_screen_size();
        let scale = state.ui_state.ui_scale;

        // Scaled dimensions
        let panel_width = PRAYER_PANEL_WIDTH * scale;
        let panel_height = PRAYER_PANEL_HEIGHT * scale;
        let frame_thickness = FRAME_THICKNESS * scale;
        let header_height = PRAYER_HEADER_HEIGHT * scale;
        let panel_padding = PRAYER_PANEL_PADDING * scale;
        let slot_size = PRAYER_SLOT_SIZE * scale;
        let slot_spacing = PRAYER_SLOT_SPACING * scale;
        let button_size = MENU_BUTTON_SIZE * scale;
        let exp_bar_gap = EXP_BAR_GAP * scale;
        let points_height = PRAYER_POINTS_HEIGHT * scale;

        // Position panel on right side, above the menu buttons
        let panel_x = screen_w - panel_width - 8.0;
        let button_area_height = button_size + exp_bar_gap;
        let panel_y = screen_h - button_area_height - panel_height - 8.0;

        // Draw panel frame
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        // Header
        let header_x = panel_x + frame_thickness;
        let header_y = panel_y + frame_thickness;
        let header_w = panel_width - frame_thickness * 2.0;

        draw_rectangle(header_x, header_y, header_w, header_height, HEADER_BG);
        draw_line(
            header_x + 6.0 * scale,
            header_y + header_height,
            header_x + header_w - 6.0 * scale,
            header_y + header_height,
            1.0,
            HEADER_BORDER,
        );

        // Header text
        let header_text = "Prayer Book";
        let text_dims = self.measure_text_sharp(header_text, 16.0);
        let text_x = header_x + (header_w - text_dims.width) / 2.0;
        self.draw_text_sharp(header_text, text_x, header_y + (header_height + 12.0) / 2.0, 16.0, TEXT_TITLE);

        // Get player's prayer level
        let prayer_level = state.get_local_player()
            .map(|p| p.skills.prayer.level)
            .unwrap_or(1);

        // Grid area
        let grid_x = panel_x + frame_thickness + panel_padding;
        let grid_y = header_y + header_height + panel_padding;

        // Draw prayer slots
        for (i, prayer) in PRAYERS.iter().enumerate() {
            let row = i / PRAYER_GRID_COLS;
            let col = i % PRAYER_GRID_COLS;
            let slot_x = grid_x + col as f32 * (slot_size + slot_spacing);
            let slot_y = grid_y + row as f32 * (slot_size + slot_spacing);

            // Register bounds for hit detection
            let bounds = Rect::new(slot_x, slot_y, slot_size, slot_size);
            layout.add(UiElementId::PrayerSlot(i), bounds);

            let is_hovered = matches!(hovered, Some(UiElementId::PrayerSlot(idx)) if *idx == i);
            let is_active = state.active_prayers.contains(&prayer.id.to_string());
            let is_locked = prayer_level < prayer.level_req;
            let has_points = state.prayer_points > 0;

            self.draw_prayer_slot(slot_x, slot_y, slot_size, prayer, is_locked, is_active, is_hovered, has_points, scale);
        }

        // Prayer points bar
        let points_y = grid_y + PRAYER_GRID_ROWS as f32 * (slot_size + slot_spacing) - slot_spacing + panel_padding;
        let points_bar_width = panel_width - frame_thickness * 2.0 - panel_padding * 2.0;
        self.draw_prayer_points_bar(grid_x, points_y, points_bar_width, points_height, state, scale);
    }

    /// Draw a single prayer slot
    fn draw_prayer_slot(
        &self,
        x: f32,
        y: f32,
        size: f32,
        prayer: &PrayerDef,
        is_locked: bool,
        is_active: bool,
        is_hovered: bool,
        has_points: bool,
        scale: f32,
    ) {
        // Background color based on state
        let (bg_color, border_color) = if is_locked {
            // Locked - dark and muted
            (
                Color::new(0.055, 0.055, 0.075, 1.0),
                Color::new(0.15, 0.14, 0.13, 1.0),
            )
        } else if is_active {
            // Active - glowing highlight based on category
            let cat_color = category_color(prayer.category);
            (
                Color::new(cat_color.r * 0.3, cat_color.g * 0.3, cat_color.b * 0.3, 1.0),
                Color::new(cat_color.r, cat_color.g, cat_color.b, 1.0),
            )
        } else if is_hovered && has_points {
            // Hovered - highlighted
            (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
        } else {
            // Normal - available
            (SLOT_BG_EMPTY, SLOT_BORDER)
        };

        // Draw slot background
        draw_rectangle(x, y, size, size, border_color);
        draw_rectangle(x + 1.0, y + 1.0, size - 2.0, size - 2.0, bg_color);

        // Inner shadow
        draw_line(x + 2.0, y + 2.0, x + size - 2.0, y + 2.0, 2.0, SLOT_INNER_SHADOW);
        draw_line(x + 2.0, y + 2.0, x + 2.0, y + size - 2.0, 2.0, SLOT_INNER_SHADOW);

        // Active glow effect
        if is_active {
            let cat_color = category_color(prayer.category);
            let glow_alpha = 0.3 + 0.1 * ((get_time() * 2.0).sin() as f32); // Pulsing glow
            draw_rectangle(
                x + 2.0,
                y + 2.0,
                size - 4.0,
                size - 4.0,
                Color::new(cat_color.r, cat_color.g, cat_color.b, glow_alpha),
            );
        }

        // Draw prayer icon (category symbol for now)
        let icon_size = 20.0 * scale;
        let icon_x = x + (size - icon_size) / 2.0;
        let icon_y = y + (size - icon_size) / 2.0;

        let icon_color = if is_locked {
            Color::new(0.3, 0.28, 0.25, 1.0)
        } else if is_active {
            category_color(prayer.category)
        } else {
            Color::new(0.7, 0.7, 0.7, 1.0)
        };

        // Draw a simple symbol based on category
        self.draw_prayer_icon(icon_x, icon_y, icon_size, prayer.category, icon_color);

        // Level requirement in corner (if locked)
        if is_locked {
            let level_text = format!("{}", prayer.level_req);
            let text_dims = self.measure_text_sharp(&level_text, 12.0);
            let level_x = x + size - text_dims.width - 2.0;
            let level_y = y + size - 2.0;

            // Shadow
            self.draw_text_sharp(&level_text, level_x + 1.0, level_y + 1.0, 12.0, Color::new(0.0, 0.0, 0.0, 0.8));
            // Text (red for locked)
            self.draw_text_sharp(&level_text, level_x, level_y, 12.0, Color::new(0.8, 0.3, 0.3, 1.0));
        }
    }

    /// Draw prayer icon based on category
    fn draw_prayer_icon(&self, x: f32, y: f32, size: f32, category: PrayerCategory, color: Color) {
        let cx = x + size / 2.0;
        let cy = y + size / 2.0;
        let s = size * 0.4;

        match category {
            PrayerCategory::Attack => {
                // Sword/blade shape
                draw_rectangle(cx - s * 0.15, cy - s, s * 0.3, s * 1.6, color);
                draw_rectangle(cx - s * 0.5, cy + s * 0.3, s, s * 0.25, color);
            }
            PrayerCategory::Defence => {
                // Shield shape
                draw_rectangle(cx - s * 0.5, cy - s * 0.6, s, s * 1.2, color);
                draw_rectangle(cx - s * 0.3, cy + s * 0.4, s * 0.6, s * 0.3, color);
            }
            PrayerCategory::Strength => {
                // Fist/arm shape
                draw_rectangle(cx - s * 0.4, cy - s * 0.3, s * 0.8, s * 0.6, color);
                draw_rectangle(cx + s * 0.1, cy - s * 0.6, s * 0.3, s * 0.4, color);
            }
            PrayerCategory::Gathering => {
                // Pickaxe shape
                draw_rectangle(cx - s * 0.15, cy - s * 0.7, s * 0.3, s * 1.4, color);
                draw_rectangle(cx - s * 0.5, cy - s * 0.7, s * 0.7, s * 0.3, color);
            }
            PrayerCategory::HpRegen => {
                // Heart/cross shape
                draw_rectangle(cx - s * 0.15, cy - s * 0.5, s * 0.3, s, color);
                draw_rectangle(cx - s * 0.5, cy - s * 0.15, s, s * 0.3, color);
            }
            PrayerCategory::Protection => {
                // Circle/holy symbol
                let r = s * 0.5;
                // Draw a simple diamond
                draw_rectangle(cx - r * 0.2, cy - r, r * 0.4, r * 0.8, color);
                draw_rectangle(cx - r, cy - r * 0.2, r * 2.0, r * 0.4, color);
                draw_rectangle(cx - r * 0.2, cy + r * 0.2, r * 0.4, r * 0.8, color);
            }
        }
    }

    /// Draw the prayer points bar
    fn draw_prayer_points_bar(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        state: &GameState,
        _scale: f32,
    ) {
        // Background
        draw_rectangle(x, y, width, height, Color::new(0.1, 0.1, 0.15, 1.0));
        draw_rectangle(x + 1.0, y + 1.0, width - 2.0, height - 2.0, Color::new(0.05, 0.05, 0.08, 1.0));

        // Fill bar
        let fill_ratio = if state.max_prayer_points > 0 {
            (state.prayer_points as f32 / state.max_prayer_points as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };

        if fill_ratio > 0.0 {
            let fill_width = (width - 4.0) * fill_ratio;
            // Cyan/turquoise for prayer points
            let fill_color = Color::new(0.2, 0.7, 0.8, 1.0);
            let fill_highlight = Color::new(0.4, 0.85, 0.95, 1.0);

            draw_rectangle(x + 2.0, y + 2.0, fill_width, height - 4.0, fill_color);
            // Highlight on top
            draw_rectangle(x + 2.0, y + 2.0, fill_width, (height - 4.0) * 0.3, fill_highlight);
        }

        // Text
        let points_text = format!("{}/{}", state.prayer_points, state.max_prayer_points);
        let text_dims = self.measure_text_sharp(&points_text, 14.0);
        let text_x = x + (width - text_dims.width) / 2.0;
        let text_y = y + (height + 10.0) / 2.0;

        // Shadow
        self.draw_text_sharp(&points_text, text_x + 1.0, text_y + 1.0, 14.0, Color::new(0.0, 0.0, 0.0, 0.8));
        // Text
        self.draw_text_sharp(&points_text, text_x, text_y, 14.0, TEXT_NORMAL);
    }

    /// Render prayer tooltip when hovering a prayer slot
    pub(crate) fn render_prayer_tooltip(&self, state: &GameState, hovered: &Option<UiElementId>) {
        let slot_index = match hovered {
            Some(UiElementId::PrayerSlot(i)) if *i < PRAYERS.len() => *i,
            _ => return,
        };

        let prayer = &PRAYERS[slot_index];
        let player = match state.get_local_player() {
            Some(p) => p,
            None => return,
        };

        let prayer_level = player.skills.prayer.level;
        let is_locked = prayer_level < prayer.level_req;
        let is_active = state.active_prayers.contains(&prayer.id.to_string());

        let (mouse_x, mouse_y) = mouse_position();

        // Tooltip content
        let name = prayer.name;
        let level_text = format!("Level: {}", prayer.level_req);
        let status_text = if is_locked {
            format!("Requires level {}", prayer.level_req)
        } else if is_active {
            "Active".to_string()
        } else {
            "Click to activate".to_string()
        };

        // Calculate tooltip size
        let padding = 8.0;
        let line_height = 18.0;
        let font_size = 14.0;

        let name_dims = self.measure_text_sharp(name, 16.0);
        let level_dims = self.measure_text_sharp(&level_text, font_size);
        let desc_dims = self.measure_text_sharp(prayer.description, font_size);
        let status_dims = self.measure_text_sharp(&status_text, font_size);

        let max_width = name_dims.width
            .max(level_dims.width)
            .max(desc_dims.width)
            .max(status_dims.width);

        let tooltip_width = max_width + padding * 2.0;
        let tooltip_height = padding * 2.0 + line_height * 4.0;

        // Position tooltip
        let (sw, sh) = virtual_screen_size();
        let tooltip_x = (mouse_x + 16.0).min(sw - tooltip_width - 8.0);
        let tooltip_y = (mouse_y + 16.0).min(sh - tooltip_height - 8.0);

        // Draw tooltip background
        draw_rectangle(tooltip_x - 1.0, tooltip_y - 1.0, tooltip_width + 2.0, tooltip_height + 2.0, TOOLTIP_FRAME);
        draw_rectangle(tooltip_x, tooltip_y, tooltip_width, tooltip_height, TOOLTIP_BG);

        // Draw text
        let mut text_y = tooltip_y + padding + 12.0;

        // Prayer name (colored by category)
        let name_color = if is_locked { TEXT_DIM } else { category_color(prayer.category) };
        self.draw_text_sharp(name, tooltip_x + padding, text_y, 16.0, name_color);
        text_y += line_height;

        // Level
        let level_color = if is_locked { Color::new(0.8, 0.3, 0.3, 1.0) } else { TEXT_NORMAL };
        self.draw_text_sharp(&level_text, tooltip_x + padding, text_y, font_size, level_color);
        text_y += line_height;

        // Description
        self.draw_text_sharp(prayer.description, tooltip_x + padding, text_y, font_size, TEXT_NORMAL);
        text_y += line_height;

        // Status
        let status_color = if is_active { Color::new(0.3, 0.8, 0.3, 1.0) } else { TEXT_DIM };
        self.draw_text_sharp(&status_text, tooltip_x + padding, text_y, font_size, status_color);
    }
}
