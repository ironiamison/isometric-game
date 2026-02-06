//! Prayer Book / Spell Book panel rendering - displays prayers or spells in a grid
//! Shows prayer/spell icons with locked/available/active states
//! Click to toggle prayers (sends TogglePrayer message)
//! Tabs switch between Prayers and Spells views

use macroquad::prelude::*;
use crate::game::GameState;
use crate::game::prayer::{PrayerCategory, PrayerDef, PRAYERS};
use crate::game::spell::{SpellDef, SPELLS};
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
const PRAYER_TAB_HEIGHT: f32 = 22.0;
const PRAYER_POINTS_HEIGHT: f32 = 20.0;
const PRAYER_PANEL_WIDTH: f32 = PRAYER_GRID_WIDTH + PRAYER_PANEL_PADDING * 2.0 + FRAME_THICKNESS * 2.0;
const PRAYER_PANEL_HEIGHT: f32 = FRAME_THICKNESS * 2.0 + PRAYER_HEADER_HEIGHT + PRAYER_TAB_HEIGHT + PRAYER_PANEL_PADDING +
    (PRAYER_GRID_ROWS as f32 * PRAYER_SLOT_SIZE + (PRAYER_GRID_ROWS - 1) as f32 * PRAYER_SLOT_SPACING) +
    PRAYER_PANEL_PADDING + PRAYER_POINTS_HEIGHT + PRAYER_PANEL_PADDING;

/// Spell theme color (purple/arcane)
const SPELL_COLOR: Color = Color::new(0.6, 0.4, 0.9, 1.0);

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
    /// Render the prayer/spell book panel when open
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
        let tab_height = PRAYER_TAB_HEIGHT * scale;
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

        // Header text - show active tab name
        let active_tab = state.ui_state.prayer_spell_tab;
        let header_text = if active_tab == 0 { "Prayer Book" } else { "Spell Book" };
        let text_dims = self.measure_text_sharp(header_text, 16.0);
        let text_x = header_x + (header_w - text_dims.width) / 2.0;
        self.draw_text_sharp(header_text, text_x, header_y + (header_height + 12.0) / 2.0, 16.0, TEXT_TITLE);

        // Tab bar below header
        let tab_y = header_y + header_height;
        let tab_w = header_w / 2.0;
        let tab_labels = ["Prayers", "Spells"];

        for (i, label) in tab_labels.iter().enumerate() {
            let tx = header_x + i as f32 * tab_w;
            let is_active_tab = active_tab == i;
            let is_hovered_tab = matches!(hovered, Some(UiElementId::PrayerSpellTab(idx)) if *idx == i);

            // Tab background
            let tab_bg = if is_active_tab {
                Color::new(0.16, 0.14, 0.20, 1.0)
            } else if is_hovered_tab {
                Color::new(0.12, 0.11, 0.16, 1.0)
            } else {
                Color::new(0.08, 0.08, 0.10, 1.0)
            };
            draw_rectangle(tx, tab_y, tab_w, tab_height, tab_bg);

            // Active tab bottom highlight
            if is_active_tab {
                let accent = if i == 0 {
                    Color::new(0.2, 0.7, 0.8, 1.0) // Cyan for prayers
                } else {
                    SPELL_COLOR // Purple for spells
                };
                draw_rectangle(tx, tab_y + tab_height - 2.0 * scale, tab_w, 2.0 * scale, accent);
            }

            // Tab text
            let label_dims = self.measure_text_sharp(label, 16.0);
            let label_x = tx + (tab_w - label_dims.width) / 2.0;
            let label_color = if is_active_tab { TEXT_NORMAL } else { TEXT_DIM };
            self.draw_text_sharp(label, label_x, tab_y + (tab_height + 12.0) / 2.0, 16.0, label_color);

            // Register tab bounds
            layout.add(UiElementId::PrayerSpellTab(i), Rect::new(tx, tab_y, tab_w, tab_height));
        }

        // Separator line below tabs
        draw_line(
            header_x + 6.0 * scale,
            tab_y + tab_height,
            header_x + header_w - 6.0 * scale,
            tab_y + tab_height,
            1.0,
            HEADER_BORDER,
        );

        // Grid area - starts below tab bar
        let grid_x = panel_x + frame_thickness + panel_padding;
        let grid_y = tab_y + tab_height + panel_padding;

        // Render content based on active tab
        if active_tab == 0 {
            // === PRAYERS TAB ===
            let prayer_level = state.get_local_player()
                .map(|p| p.skills.prayer.level)
                .unwrap_or(1);

            // Draw prayer slots
            for (i, prayer) in PRAYERS.iter().enumerate() {
                let row = i / PRAYER_GRID_COLS;
                let col = i % PRAYER_GRID_COLS;
                let slot_x = grid_x + col as f32 * (slot_size + slot_spacing);
                let slot_y = grid_y + row as f32 * (slot_size + slot_spacing);

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
        } else {
            // === SPELLS TAB ===
            let magic_level = state.get_local_player()
                .map(|p| p.skills.magic.level)
                .unwrap_or(1);

            // Draw spell slots
            self.render_spell_grid(grid_x, grid_y, slot_size, slot_spacing, magic_level, state, hovered, layout, scale);

            // Mana bar (replaces prayer points bar)
            let points_y = grid_y + PRAYER_GRID_ROWS as f32 * (slot_size + slot_spacing) - slot_spacing + panel_padding;
            let points_bar_width = panel_width - frame_thickness * 2.0 - panel_padding * 2.0;
            self.draw_mana_bar(grid_x, points_y, points_bar_width, points_height, state, scale);
        }
    }

    /// Render spell grid in the Spells tab
    fn render_spell_grid(
        &self,
        grid_x: f32,
        grid_y: f32,
        slot_size: f32,
        slot_spacing: f32,
        magic_level: i32,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        scale: f32,
    ) {
        for (i, spell) in SPELLS.iter().enumerate() {
            let row = i / PRAYER_GRID_COLS;
            let col = i % PRAYER_GRID_COLS;
            let slot_x = grid_x + col as f32 * (slot_size + slot_spacing);
            let slot_y = grid_y + row as f32 * (slot_size + slot_spacing);

            let bounds = Rect::new(slot_x, slot_y, slot_size, slot_size);
            layout.add(UiElementId::SpellSlot(i), bounds);

            let is_hovered = matches!(hovered, Some(UiElementId::SpellSlot(idx)) if *idx == i);
            let is_locked = magic_level < spell.magic_level_req;

            self.draw_spell_slot(slot_x, slot_y, slot_size, spell, is_locked, is_hovered, scale);
        }

        // Fill remaining empty slots in the grid for visual consistency
        let _total_slots = PRAYER_GRID_COLS * PRAYER_GRID_ROWS;
        for i in SPELLS.len()..(PRAYER_GRID_COLS * PRAYER_GRID_ROWS) {
            let row = i / PRAYER_GRID_COLS;
            let col = i % PRAYER_GRID_COLS;
            let slot_x = grid_x + col as f32 * (slot_size + slot_spacing);
            let slot_y = grid_y + row as f32 * (slot_size + slot_spacing);

            // Draw empty slot
            draw_rectangle(slot_x, slot_y, slot_size, slot_size, Color::new(0.15, 0.14, 0.13, 1.0));
            draw_rectangle(slot_x + 1.0, slot_y + 1.0, slot_size - 2.0, slot_size - 2.0, Color::new(0.055, 0.055, 0.075, 1.0));
            draw_line(slot_x + 2.0, slot_y + 2.0, slot_x + slot_size - 2.0, slot_y + 2.0, 2.0, SLOT_INNER_SHADOW);
            draw_line(slot_x + 2.0, slot_y + 2.0, slot_x + 2.0, slot_y + slot_size - 2.0, 2.0, SLOT_INNER_SHADOW);
        }
    }

    /// Draw a single spell slot
    fn draw_spell_slot(
        &self,
        x: f32,
        y: f32,
        size: f32,
        spell: &SpellDef,
        is_locked: bool,
        is_hovered: bool,
        scale: f32,
    ) {
        // Background color based on state
        let (bg_color, border_color) = if is_locked {
            // Locked - dark and muted (same as locked prayers)
            (
                Color::new(0.055, 0.055, 0.075, 1.0),
                Color::new(0.15, 0.14, 0.13, 1.0),
            )
        } else if is_hovered {
            // Hovered - purple tint highlight
            (
                Color::new(0.15, 0.12, 0.25, 1.0),
                Color::new(0.6, 0.45, 0.8, 1.0),
            )
        } else {
            // Available - slightly purple tint
            (
                Color::new(0.10, 0.08, 0.16, 1.0),
                Color::new(0.30, 0.25, 0.40, 1.0),
            )
        };

        // Draw slot background
        draw_rectangle(x, y, size, size, border_color);
        draw_rectangle(x + 1.0, y + 1.0, size - 2.0, size - 2.0, bg_color);

        // Inner shadow
        draw_line(x + 2.0, y + 2.0, x + size - 2.0, y + 2.0, 2.0, SLOT_INNER_SHADOW);
        draw_line(x + 2.0, y + 2.0, x + 2.0, y + size - 2.0, 2.0, SLOT_INNER_SHADOW);

        // Draw spell icon - colored square with initials as fallback
        let icon_size = 20.0 * scale;
        let icon_x = x + (size - icon_size) / 2.0;
        let icon_y = y + (size - icon_size) / 2.0;

        let icon_color = if is_locked {
            Color::new(0.3, 0.28, 0.35, 1.0)
        } else {
            SPELL_COLOR
        };

        // Draw colored square background for the icon
        let icon_bg = if is_locked {
            Color::new(0.08, 0.07, 0.12, 1.0)
        } else {
            Color::new(0.15, 0.10, 0.30, 1.0)
        };
        draw_rectangle(icon_x, icon_y, icon_size, icon_size, icon_bg);

        // Draw the spell's initials as icon
        let initials: String = spell.name.split_whitespace()
            .map(|w| w.chars().next().unwrap_or('?'))
            .collect();
        let initial_dims = self.measure_text_sharp(&initials, 14.0);
        let initial_x = icon_x + (icon_size - initial_dims.width) / 2.0;
        let initial_y = icon_y + (icon_size + 10.0) / 2.0;
        self.draw_text_sharp(&initials, initial_x, initial_y, 14.0, icon_color);

        // Mana cost in bottom-right corner
        let cost_text = format!("{}", spell.mana_cost);
        let cost_dims = self.measure_text_sharp(&cost_text, 12.0);
        let cost_x = x + size - cost_dims.width - 2.0;
        let cost_y = y + size - 2.0;

        // Shadow
        self.draw_text_sharp(&cost_text, cost_x + 1.0, cost_y + 1.0, 12.0, Color::new(0.0, 0.0, 0.0, 0.8));
        // Mana cost (blue for available, red for locked)
        let cost_color = if is_locked {
            Color::new(0.8, 0.3, 0.3, 1.0)
        } else {
            Color::new(0.4, 0.5, 0.9, 1.0)
        };
        self.draw_text_sharp(&cost_text, cost_x, cost_y, 12.0, cost_color);

        // Level requirement in top-left corner (if locked)
        if is_locked {
            let level_text = format!("{}", spell.magic_level_req);
            let level_dims = self.measure_text_sharp(&level_text, 16.0);
            let level_x = x + size - level_dims.width - 2.0;
            let level_y = y + 14.0;

            // Shadow
            self.draw_text_sharp(&level_text, level_x + 1.0, level_y + 1.0, 16.0, Color::new(0.0, 0.0, 0.0, 0.8));
            // Text (red for locked)
            self.draw_text_sharp(&level_text, level_x, level_y, 16.0, Color::new(0.8, 0.3, 0.3, 1.0));
        }
    }

    /// Draw the mana bar (shown in Spells tab instead of prayer points)
    fn draw_mana_bar(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        state: &GameState,
        _scale: f32,
    ) {
        let (mp, max_mp) = state.get_local_player()
            .map(|p| (p.mp, p.max_mp))
            .unwrap_or((0, 1));

        // Background
        draw_rectangle(x, y, width, height, Color::new(0.1, 0.1, 0.15, 1.0));
        draw_rectangle(x + 1.0, y + 1.0, width - 2.0, height - 2.0, Color::new(0.05, 0.05, 0.08, 1.0));

        // Fill bar
        let fill_ratio = if max_mp > 0 {
            (mp as f32 / max_mp as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };

        if fill_ratio > 0.0 {
            let fill_width = (width - 4.0) * fill_ratio;
            // Blue/purple for mana
            let fill_color = Color::new(0.3, 0.2, 0.8, 1.0);
            let fill_highlight = Color::new(0.5, 0.35, 0.95, 1.0);

            draw_rectangle(x + 2.0, y + 2.0, fill_width, height - 4.0, fill_color);
            // Highlight on top
            draw_rectangle(x + 2.0, y + 2.0, fill_width, (height - 4.0) * 0.3, fill_highlight);
        }

        // Text
        let mana_text = format!("{}/{}", mp, max_mp);
        let text_dims = self.measure_text_sharp(&mana_text, 16.0);
        let text_x = x + (width - text_dims.width) / 2.0;
        let text_y = y + (height + 12.0) / 2.0;

        // Shadow
        self.draw_text_sharp(&mana_text, text_x + 1.0, text_y + 1.0, 16.0, Color::new(0.0, 0.0, 0.0, 0.8));
        // Text
        self.draw_text_sharp(&mana_text, text_x, text_y, 16.0, TEXT_NORMAL);
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

        // Draw prayer icon from sprite sheet
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

        // Draw prayer icon from individual texture
        self.draw_prayer_icon(icon_x, icon_y, icon_size, prayer.id, icon_color);

        // Level requirement in corner (if locked)
        if is_locked {
            let level_text = format!("{}", prayer.level_req);
            let text_dims = self.measure_text_sharp(&level_text, 16.0);
            let level_x = x + size - text_dims.width - 2.0;
            let level_y = y + size - 2.0;

            // Shadow
            self.draw_text_sharp(&level_text, level_x + 1.0, level_y + 1.0, 16.0, Color::new(0.0, 0.0, 0.0, 0.8));
            // Text (red for locked)
            self.draw_text_sharp(&level_text, level_x, level_y, 16.0, Color::new(0.8, 0.3, 0.3, 1.0));
        }
    }

    /// Draw prayer icon from individual texture file
    fn draw_prayer_icon(&self, x: f32, y: f32, size: f32, prayer_id: &str, color: Color) {
        if let Some(texture) = self.prayer_icons.get(prayer_id) {
            draw_texture_ex(
                texture,
                x,
                y,
                color,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(size, size)),
                    ..Default::default()
                },
            );
        } else {
            // Fallback: draw "?" if texture not found
            let cx = x + size / 2.0;
            let cy = y + size / 2.0;
            self.draw_text_sharp("?", cx - 4.0, cy + 4.0, 16.0, color);
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
        let text_dims = self.measure_text_sharp(&points_text, 16.0);
        let text_x = x + (width - text_dims.width) / 2.0;
        let text_y = y + (height + 12.0) / 2.0;

        // Shadow
        self.draw_text_sharp(&points_text, text_x + 1.0, text_y + 1.0, 16.0, Color::new(0.0, 0.0, 0.0, 0.8));
        // Text
        self.draw_text_sharp(&points_text, text_x, text_y, 16.0, TEXT_NORMAL);
    }

    /// Render tooltip when hovering a prayer or spell slot
    pub(crate) fn render_prayer_tooltip(&self, state: &GameState, hovered: &Option<UiElementId>) {
        // Check for prayer slot hover
        if let Some(UiElementId::PrayerSlot(i)) = hovered {
            if *i < PRAYERS.len() {
                self.render_prayer_slot_tooltip(state, *i);
                return;
            }
        }

        // Check for spell slot hover
        if let Some(UiElementId::SpellSlot(i)) = hovered {
            if *i < SPELLS.len() {
                self.render_spell_tooltip(state, *i);
                return;
            }
        }
    }

    /// Render tooltip for a prayer slot
    fn render_prayer_slot_tooltip(&self, state: &GameState, slot_index: usize) {
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
        let line_height = 20.0;
        let font_size = 16.0;

        let name_dims = self.measure_text_sharp(name, font_size);
        let level_dims = self.measure_text_sharp(&level_text, font_size);
        let desc_dims = self.measure_text_sharp(prayer.description, font_size);
        let status_dims = self.measure_text_sharp(&status_text, font_size);

        let max_width = name_dims.width
            .max(level_dims.width)
            .max(desc_dims.width)
            .max(status_dims.width);

        let tooltip_width = (max_width + padding * 2.0).floor();
        let tooltip_height = (padding * 2.0 + line_height * 4.0).floor();

        // Position tooltip (floor to avoid subpixel rendering)
        let (sw, sh) = virtual_screen_size();
        let tooltip_x = (mouse_x + 16.0).min(sw - tooltip_width - 8.0).floor();
        let tooltip_y = (mouse_y + 16.0).min(sh - tooltip_height - 8.0).floor();

        // Draw tooltip background
        draw_rectangle(tooltip_x - 1.0, tooltip_y - 1.0, tooltip_width + 2.0, tooltip_height + 2.0, TOOLTIP_FRAME);
        draw_rectangle(tooltip_x, tooltip_y, tooltip_width, tooltip_height, TOOLTIP_BG);

        // Draw text
        let mut text_y = (tooltip_y + padding + 14.0).floor();

        // Prayer name (colored by category)
        let name_color = if is_locked { TEXT_DIM } else { category_color(prayer.category) };
        self.draw_text_sharp(name, (tooltip_x + padding).floor(), text_y, font_size, name_color);
        text_y += line_height;

        // Level
        let level_color = if is_locked { Color::new(0.8, 0.3, 0.3, 1.0) } else { TEXT_NORMAL };
        self.draw_text_sharp(&level_text, (tooltip_x + padding).floor(), text_y, font_size, level_color);
        text_y += line_height;

        // Description
        self.draw_text_sharp(prayer.description, (tooltip_x + padding).floor(), text_y, font_size, TEXT_NORMAL);
        text_y += line_height;

        // Status
        let status_color = if is_active { Color::new(0.3, 0.8, 0.3, 1.0) } else { TEXT_DIM };
        self.draw_text_sharp(&status_text, (tooltip_x + padding).floor(), text_y, font_size, status_color);
    }

    /// Render tooltip for a spell slot
    fn render_spell_tooltip(&self, state: &GameState, slot_index: usize) {
        let spell = &SPELLS[slot_index];
        let player = match state.get_local_player() {
            Some(p) => p,
            None => return,
        };

        let magic_level = player.skills.magic.level;
        let is_locked = magic_level < spell.magic_level_req;

        let (mouse_x, mouse_y) = mouse_position();

        // Tooltip content
        let name = spell.name;
        let level_text = format!("Magic Level: {}", spell.magic_level_req);
        let mana_text = format!("Mana Cost: {}", spell.mana_cost);
        let cooldown_text = format!("Cooldown: {:.1}s", spell.cooldown_ms as f64 / 1000.0);
        let desc = spell.description;

        // Calculate tooltip size
        let padding = 8.0;
        let line_height = 20.0;
        let font_size = 16.0;

        let name_dims = self.measure_text_sharp(name, font_size);
        let level_dims = self.measure_text_sharp(&level_text, font_size);
        let mana_dims = self.measure_text_sharp(&mana_text, font_size);
        let cooldown_dims = self.measure_text_sharp(&cooldown_text, font_size);
        let desc_dims = self.measure_text_sharp(desc, font_size);

        let max_width = name_dims.width
            .max(level_dims.width)
            .max(mana_dims.width)
            .max(cooldown_dims.width)
            .max(desc_dims.width);

        let tooltip_width = (max_width + padding * 2.0).floor();
        let tooltip_height = (padding * 2.0 + line_height * 5.0).floor();

        // Position tooltip (floor to avoid subpixel rendering)
        let (sw, sh) = virtual_screen_size();
        let tooltip_x = (mouse_x + 16.0).min(sw - tooltip_width - 8.0).floor();
        let tooltip_y = (mouse_y + 16.0).min(sh - tooltip_height - 8.0).floor();

        // Draw tooltip background
        draw_rectangle(tooltip_x - 1.0, tooltip_y - 1.0, tooltip_width + 2.0, tooltip_height + 2.0, TOOLTIP_FRAME);
        draw_rectangle(tooltip_x, tooltip_y, tooltip_width, tooltip_height, TOOLTIP_BG);

        // Draw text
        let mut text_y = (tooltip_y + padding + 14.0).floor();

        // Spell name (purple if available, grey if locked)
        let name_color = if is_locked { TEXT_DIM } else { SPELL_COLOR };
        self.draw_text_sharp(name, (tooltip_x + padding).floor(), text_y, font_size, name_color);
        text_y += line_height;

        // Magic level requirement
        let level_color = if is_locked { Color::new(0.8, 0.3, 0.3, 1.0) } else { TEXT_NORMAL };
        self.draw_text_sharp(&level_text, (tooltip_x + padding).floor(), text_y, font_size, level_color);
        text_y += line_height;

        // Mana cost
        let mana_color = Color::new(0.4, 0.5, 0.9, 1.0);
        self.draw_text_sharp(&mana_text, (tooltip_x + padding).floor(), text_y, font_size, mana_color);
        text_y += line_height;

        // Cooldown
        self.draw_text_sharp(&cooldown_text, (tooltip_x + padding).floor(), text_y, font_size, TEXT_NORMAL);
        text_y += line_height;

        // Description
        self.draw_text_sharp(desc, (tooltip_x + padding).floor(), text_y, font_size, TEXT_NORMAL);
    }
}
