//! Character panel rendering - separate equipment slots panel

use super::super::Renderer;
use super::common::*;
use crate::game::{DragSource, GameState};
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

/// Character panel dimensions
const CHARACTER_PANEL_PADDING: f32 = 12.0;
const CHARACTER_HEADER_HEIGHT: f32 = 24.0;
const CHARACTER_GRID_HEIGHT: f32 = 4.0 * EQUIP_SLOT_SIZE + 3.0 * EQUIP_SLOT_SPACING; // 164
const CHARACTER_PANEL_WIDTH: f32 = 240.0; // Unified width to match inventory and other UI panels
const SHOP_BUTTON_HEIGHT: f32 = 26.0;
const COMBAT_STYLE_ROW_HEIGHT: f32 = 26.0;
const GRID_STYLE_GAP: f32 = 6.0; // Small gap between equipment grid and style row
const AUTO_RETALIATE_BTN_HEIGHT: f32 = 60.0; // Icon + two lines of text
const CHARACTER_PANEL_HEIGHT: f32 = FRAME_THICKNESS * 2.0
    + CHARACTER_HEADER_HEIGHT
    + CHARACTER_PANEL_PADDING
    + CHARACTER_GRID_HEIGHT
    + GRID_STYLE_GAP
    + COMBAT_STYLE_ROW_HEIGHT
    + CHARACTER_PANEL_PADDING
    + SHOP_BUTTON_HEIGHT
    + CHARACTER_PANEL_PADDING;

/// Tile size of the shared UI icon atlas (`ui_icons`), matching the skills panel.
const UI_ICON_SIZE: f32 = 24.0;

/// Value color for a negative stat bonus — penalties read red so a player
/// instantly sees a loadout is hurting a stat. Matches the "OFF" red elsewhere.
const STAT_NEGATIVE: Color = Color::new(0.85, 0.38, 0.38, 1.0);

/// Combat styles: (id, button label, tooltip title, effect description, trained skill).
const MELEE_COMBAT_STYLES: [(&str, &str, &str, &str, &str); 4] = [
    (
        "accurate",
        "Acc",
        "Accurate",
        "Boosts attack accuracy",
        "Attack",
    ),
    (
        "aggressive",
        "Agg",
        "Aggressive",
        "Boosts damage dealt",
        "Strength",
    ),
    (
        "defensive",
        "Def",
        "Defensive",
        "Boosts your defence",
        "Defence",
    ),
    (
        "controlled",
        "Ctrl",
        "Controlled",
        "Trains all combat evenly",
        "Att/Str/Def",
    ),
];

/// Ranged combat styles: same tuple shape as the melee set.
const RANGED_COMBAT_STYLES: [(&str, &str, &str, &str, &str); 3] = [
    (
        "accurate",
        "Acc",
        "Accurate",
        "Boosts ranged accuracy",
        "Ranged",
    ),
    ("rapid", "Rapid", "Rapid", "Faster attack speed", "Ranged"),
    (
        "longrange",
        "Long",
        "Long range",
        "Adds range and defence",
        "Ranged/Defence",
    ),
];

/// Format an equipment stat bonus with an explicit sign: "+5", "0", "-22".
/// Blindly prefixing "+" produced "+-22" for negative bonuses; this fixes it.
fn format_stat_bonus(v: i32) -> String {
    if v > 0 {
        format!("+{}", v)
    } else {
        // Negative values already carry their own "-"; zero shows as "0".
        format!("{}", v)
    }
}

/// Color a stat *value* by sign so penalties stand out: positive keeps the
/// stat's identity color, negative is red, zero is dim.
fn stat_value_color(v: i32, identity: Color) -> Color {
    if v < 0 {
        STAT_NEGATIVE
    } else if v == 0 {
        TEXT_DIM
    } else {
        identity
    }
}

/// Get the combat styles for the currently equipped weapon type
fn get_combat_styles_for_weapon(
    state: &GameState,
) -> &'static [(
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
)] {
    let is_ranged = state
        .get_local_player()
        .and_then(|p| p.equipped_weapon.as_ref())
        .and_then(|wid| state.item_registry.get(wid))
        .and_then(|def| def.weapon_type.as_ref())
        .map(|wt| wt == "ranged")
        .unwrap_or(false);

    if is_ranged {
        &RANGED_COMBAT_STYLES
    } else {
        &MELEE_COMBAT_STYLES
    }
}

impl Renderer {
    /// Render the character panel when open
    pub(crate) fn render_character_panel(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        if !state.ui_state.character_panel_open {
            return;
        }

        let (screen_w, screen_h) = virtual_screen_size();
        let scale = state.ui_state.ui_scale;

        // Scaled dimensions
        let panel_width = CHARACTER_PANEL_WIDTH * scale;
        let panel_height = CHARACTER_PANEL_HEIGHT * scale;
        let frame_thickness = FRAME_THICKNESS * scale;
        let header_height = CHARACTER_HEADER_HEIGHT * scale;
        let panel_padding = CHARACTER_PANEL_PADDING * scale;
        let slot_size = (EQUIP_SLOT_SIZE * scale).max(MIN_SLOT_SIZE); // Ensure icons fit
        let slot_spacing = EQUIP_SLOT_SPACING * scale;
        let _button_size = MENU_BUTTON_SIZE * scale;
        let _exp_bar_gap = EXP_BAR_GAP * scale;

        // Position panel on right side, above menu buttons
        let panel_x = screen_w - panel_width - 8.0;
        let button_area_height = bottom_ui_height(scale);
        let panel_top = if cfg!(target_os = "android") {
            2.0
        } else {
            45.0
        };
        let panel_bottom = screen_h - button_area_height - 8.0;
        // On Android, recalculate height without header, shop button, and stats for compact fit
        let panel_height = if cfg!(target_os = "android") {
            let h = FRAME_THICKNESS * 2.0
                + CHARACTER_PANEL_PADDING * 0.5
                + CHARACTER_GRID_HEIGHT
                + CHARACTER_PANEL_PADDING * 0.5
                + COMBAT_STYLE_ROW_HEIGHT
                + CHARACTER_PANEL_PADDING * 0.5;
            (h * scale).min(panel_bottom - panel_top)
        } else {
            (panel_bottom - panel_top).min(panel_height)
        };
        let panel_y = panel_bottom - panel_height;

        // Draw panel frame
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        // Header (skip on Android for more space)
        let header_x = panel_x + frame_thickness;
        let header_y = panel_y + frame_thickness;
        let header_w = panel_width - frame_thickness * 2.0;

        let content_y = if cfg!(target_os = "android") {
            header_y
        } else {
            draw_rectangle(header_x, header_y, header_w, header_height, HEADER_BG);
            draw_line(
                header_x,
                header_y + header_height,
                header_x + header_w,
                header_y + header_height,
                1.0,
                HEADER_BORDER,
            );

            let header_text = "Character";
            let text_dims = self.measure_text_sharp(header_text, 16.0);
            let text_x = header_x + (header_w - text_dims.width) / 2.0;
            self.draw_text_sharp(
                header_text,
                text_x,
                (header_y + 17.0 * scale).floor(),
                16.0,
                TEXT_TITLE,
            );
            header_y + header_height
        };

        // Grid area
        let grid_x = panel_x + frame_thickness + panel_padding;
        let grid_y = content_y + panel_padding;

        let slot_step = slot_size + slot_spacing;

        // Equipment slots - arranged in body-shaped layout
        // Same layout as was in inventory.rs
        let equipment_slots: [(&str, i32, i32); 9] = [
            ("head", 1, 0),
            ("back", 0, 1),
            ("body", 1, 1),
            ("weapon", 2, 1),
            ("gloves", 0, 2),
            ("ring", 2, 2),
            ("necklace", 0, 3),
            ("feet", 1, 3),
            ("belt", 2, 3),
        ];

        for (slot_type, col, row) in equipment_slots.iter() {
            let slot_x = grid_x + (*col as f32) * slot_step;
            let slot_y = grid_y + (*row as f32) * slot_step;

            let bounds = Rect::new(slot_x, slot_y, slot_size, slot_size);
            layout.add(UiElementId::EquipmentSlot(slot_type.to_string()), bounds);

            let is_hovered =
                matches!(hovered, Some(UiElementId::EquipmentSlot(s)) if s == *slot_type);
            let is_dragging = matches!(&state.ui_state.drag_state, Some(drag) if matches!(&drag.source, DragSource::Equipment(s) if s == *slot_type));

            let has_item = state
                .get_local_player()
                .map(|p| match *slot_type {
                    "head" => p.equipped_head.is_some(),
                    "body" => p.equipped_body.is_some(),
                    "weapon" => p.equipped_weapon.is_some(),
                    "back" => p.equipped_back.is_some(),
                    "feet" => p.equipped_feet.is_some(),
                    "ring" => p.equipped_ring.is_some(),
                    "gloves" => p.equipped_gloves.is_some(),
                    "necklace" => p.equipped_necklace.is_some(),
                    "belt" => p.equipped_belt.is_some(),
                    _ => false,
                })
                .unwrap_or(false);

            self.draw_equipment_slot(
                slot_x,
                slot_y,
                slot_size,
                slot_type,
                has_item,
                is_hovered,
                is_dragging,
            );

            if !is_dragging {
                if let Some(local_player) = state.get_local_player() {
                    let item_id = match *slot_type {
                        "head" => local_player.equipped_head.as_ref(),
                        "body" => local_player.equipped_body.as_ref(),
                        "weapon" => local_player.equipped_weapon.as_ref(),
                        "back" => local_player.equipped_back.as_ref(),
                        "feet" => local_player.equipped_feet.as_ref(),
                        "ring" => local_player.equipped_ring.as_ref(),
                        "gloves" => local_player.equipped_gloves.as_ref(),
                        "necklace" => local_player.equipped_necklace.as_ref(),
                        "belt" => local_player.equipped_belt.as_ref(),
                        _ => None,
                    };
                    if let Some(id) = item_id {
                        self.draw_item_icon(id, slot_x, slot_y, slot_size, slot_size, state, false);
                    }
                }
            }
        }

        // Stats section - to the right of equipment grid (desktop only)
        if !cfg!(target_os = "android") {
            let stats_y = grid_y;

            // Right-hand column (stats box + retaliate button), sized to its content
            // and right-anchored against the panel's inner edge so it stops crowding
            // the equipment grid. Width = the wider of the stat rows (icon + label +
            // value) and the "Retaliate" caption.
            let col_icon = 14.0 * scale;
            let col_icon_gap = 4.0 * scale;
            let col_lv_gap = 6.0 * scale; // label -> value gap (previously a loose ~14px)
            let col_hpad = 6.0 * scale; // horizontal inner padding
            let col_label_w = self.measure_text_sharp("MAG", 16.0).width;
            let col_value_w = self.measure_text_sharp("+99", 16.0).width;
            let stat_content_w = col_icon + col_icon_gap + col_label_w + col_lv_gap + col_value_w;
            let retaliate_content_w = self.measure_text_sharp("Retaliate", 16.0).width;
            let col_w = stat_content_w.max(retaliate_content_w) + col_hpad * 2.0;
            let col_x = panel_x + panel_width - frame_thickness - panel_padding - col_w;

            if let Some(player) = state.get_local_player() {
                let atk_bonus = player.attack_bonus(&state.item_registry);
                let str_bonus = player.strength_bonus(&state.item_registry);
                let def_bonus = player.defence_bonus(&state.item_registry);
                let mag_bonus = player.magic_bonus(&state.item_registry);
                let rng_bonus = player.ranged_strength_bonus(&state.item_registry);

                let mag_color = Color::new(0.650, 0.400, 0.850, 1.0);
                let rng_color = Color::new(0.900, 0.600, 0.250, 1.0);

                // Each row: (label, value, identity color, atlas tile coords matching
                // the skills panel so ATK/STR/DEF/MAG/RNG are scannable by glyph).
                let rows: [(&str, i32, Color, (i32, i32)); 5] = [
                    ("ATK", atk_bonus, CATEGORY_EQUIPMENT, (5, 5)),
                    ("STR", str_bonus, CATEGORY_CONSUMABLE, (2, 6)),
                    ("DEF", def_bonus, CATEGORY_MATERIAL, (4, 5)),
                    ("MAG", mag_bonus, mag_color, (6, 6)),
                    ("RNG", rng_bonus, rng_color, (3, 5)),
                ];

                // Framed "your totals" sub-panel: gold-language 1px border + recessed
                // dark fill wrapping the five rows so they read as one coherent block.
                let row_h = 17.0 * scale;
                let vpad = 4.0 * scale; // vertical inner padding (kept tight)
                let box_h = vpad * 2.0 + rows.len() as f32 * row_h;
                // Border matches the equipment slots so the stats box reads as part
                // of the same set rather than a separate gold-framed element.
                draw_rectangle(col_x, stats_y, col_w, box_h, SLOT_BORDER);
                draw_rectangle(
                    col_x + 1.0,
                    stats_y + 1.0,
                    col_w - 2.0,
                    box_h - 2.0,
                    SLOT_BG_EMPTY,
                );

                let icon_x = col_x + col_hpad;
                let label_x = icon_x + col_icon + col_icon_gap;
                // Right-align values to the content's natural right edge so the
                // label -> value gap stays tight regardless of the box width.
                let value_right = label_x + col_label_w + col_lv_gap + col_value_w;

                for (i, (label, val, identity, (ic, ir))) in rows.iter().enumerate() {
                    let row_top = stats_y + vpad + i as f32 * row_h;
                    let baseline = (row_top + 13.0 * scale).floor();

                    // Glyph from the shared UI icon atlas
                    if let Some(tex) = &self.ui_icons {
                        let src = Rect::new(
                            *ic as f32 * UI_ICON_SIZE,
                            *ir as f32 * UI_ICON_SIZE,
                            UI_ICON_SIZE,
                            UI_ICON_SIZE,
                        );
                        draw_texture_ex(
                            tex,
                            icon_x,
                            (row_top + (row_h - col_icon) / 2.0).floor(),
                            WHITE,
                            DrawTextureParams {
                                source: Some(src),
                                dest_size: Some(Vec2::new(col_icon, col_icon)),
                                ..Default::default()
                            },
                        );
                    }

                    // Label keeps its identity color (scannable); value is sign-colored.
                    self.draw_text_sharp(label, label_x, baseline, 16.0, *identity);
                    let val_text = format_stat_bonus(*val);
                    let val_w = self.measure_text_sharp(&val_text, 16.0).width;
                    self.draw_text_sharp(
                        &val_text,
                        value_right - val_w,
                        baseline,
                        16.0,
                        stat_value_color(*val, *identity),
                    );
                }
            }

            // Auto-retaliate toggle button (in stats column, bottom-aligned with belt row)
            let ar_h = AUTO_RETALIATE_BTN_HEIGHT * scale;
            let grid_bottom_local = grid_y + 3.0 * slot_step + slot_size; // bottom of belt row
            let ar_y = grid_bottom_local - ar_h;
            // Same content-sized, right-anchored column as the stats box above.
            let ar_w = col_w;
            let ar_x = col_x;

            let ar_bounds = Rect::new(ar_x, ar_y, ar_w, ar_h);
            layout.add(UiElementId::AutoRetaliateToggle, ar_bounds);

            let ar_enabled = state.auto_retaliate;
            let ar_hovered = matches!(hovered, Some(UiElementId::AutoRetaliateToggle));

            let ar_bg = if ar_hovered {
                SLOT_HOVER_BG
            } else {
                SLOT_BG_EMPTY
            };
            // Enabled = gold-lit border (matches the "active = gold" convention used
            // everywhere else); hover brightens; off is the plain slot border.
            let ar_border = if ar_enabled {
                SLOT_SELECTED_BORDER
            } else if ar_hovered {
                SLOT_HOVER_BORDER
            } else {
                SLOT_BORDER
            };

            draw_rectangle(ar_x, ar_y, ar_w, ar_h, ar_border);
            draw_rectangle(ar_x + 1.0, ar_y + 1.0, ar_w - 2.0, ar_h - 2.0, ar_bg);

            // Draw icon centered at top
            if let Some(icon) = &self.auto_retaliate_icon {
                let icon_size = 24.0 * scale;
                let icon_x = ar_x + (ar_w - icon_size) / 2.0;
                let icon_y = ar_y + 4.0 * scale;
                draw_texture_ex(
                    icon,
                    icon_x,
                    icon_y,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(icon_size, icon_size)),
                        ..Default::default()
                    },
                );
            }

            // Label: "Retaliate" caption + an unmistakable ON / OFF state word.
            let line1 = "Retaliate";
            let line2 = if ar_enabled { "ON" } else { "OFF" };
            let state_color = if ar_enabled {
                Color::new(0.35, 0.92, 0.4, 1.0)
            } else {
                Color::new(0.85, 0.38, 0.38, 1.0)
            };
            let l1w = self.measure_text_sharp(line1, 16.0).width;
            let l2w = self.measure_text_sharp(line2, 16.0).width;
            let text_start = ar_y + 4.0 * scale + 24.0 * scale + 2.0 * scale;
            self.draw_text_sharp(
                line1,
                ar_x + (ar_w - l1w) / 2.0,
                text_start + 10.0 * scale,
                16.0,
                TEXT_NORMAL,
            );
            self.draw_text_sharp(
                line2,
                ar_x + (ar_w - l2w) / 2.0,
                text_start + 22.0 * scale,
                16.0,
                state_color,
            );
        }

        // ===== COMBAT STYLE SELECTOR =====
        let style_row_height = COMBAT_STYLE_ROW_HEIGHT * scale;
        let style_area_width = panel_width - frame_thickness * 2.0 - panel_padding * 2.0;
        let style_x = panel_x + frame_thickness + panel_padding;
        let grid_bottom = grid_y + 4.0 * slot_step;
        let style_y = grid_bottom + GRID_STYLE_GAP * scale;

        // Label
        let label = "Style:";
        let label_w = self.measure_text_sharp(label, 16.0).width;
        self.draw_text_sharp(label, style_x, style_y + 17.0 * scale, 16.0, TEXT_DIM);

        // Dynamic style buttons filling the remaining width
        let combat_styles = get_combat_styles_for_weapon(state);
        let buttons_x = style_x + label_w + 6.0;
        let buttons_width = style_area_width - label_w - 6.0;
        let gap = 3.0 * scale;
        let num_styles = combat_styles.len() as f32;
        let btn_w = ((buttons_width - gap * (num_styles - 1.0)) / num_styles).floor();

        let current_style = state
            .get_local_player()
            .map(|p| p.combat_style.clone())
            .unwrap_or_else(|| "accurate".to_string());

        for (i, (style_id, style_label, _, _, _)) in combat_styles.iter().enumerate() {
            let bx = buttons_x + i as f32 * (btn_w + gap);
            let by = style_y;
            let bounds = Rect::new(bx, by, btn_w, style_row_height);
            layout.add(UiElementId::CombatStyleButton(i), bounds);

            let is_active = current_style == *style_id;
            let is_hovered =
                matches!(hovered, Some(UiElementId::CombatStyleButton(idx)) if *idx == i);

            let (bg, border) = if is_active {
                (SLOT_HOVER_BG, SLOT_SELECTED_BORDER)
            } else if is_hovered {
                (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
            } else {
                (SLOT_BG_EMPTY, SLOT_BORDER)
            };

            draw_rectangle(bx, by, btn_w, style_row_height, border);
            draw_rectangle(bx + 1.0, by + 1.0, btn_w - 2.0, style_row_height - 2.0, bg);

            let text_color = if is_active {
                TEXT_GOLD
            } else if is_hovered {
                TEXT_TITLE
            } else {
                TEXT_NORMAL
            };
            let tw = self.measure_text_sharp(style_label, 16.0).width;
            self.draw_text_sharp(
                style_label,
                bx + (btn_w - tw) / 2.0,
                by + 17.0 * scale,
                16.0,
                text_color,
            );
        }

        // ===== OPEN SHOP BUTTON (desktop only) =====
        if cfg!(target_os = "android") {
            return;
        }
        let btn_height = 26.0 * scale;
        let btn_width = panel_width - frame_thickness * 2.0 - panel_padding * 2.0;
        let btn_x = panel_x + frame_thickness + panel_padding;
        let btn_y = panel_y + panel_height - frame_thickness - panel_padding - btn_height;

        let btn_bounds = Rect::new(btn_x, btn_y, btn_width, btn_height);
        layout.add(UiElementId::CharacterOpenShopButton, btn_bounds);

        let btn_hovered = matches!(hovered, Some(UiElementId::CharacterOpenShopButton));
        let btn_label = if state.ui_state.stall_setup_open || state.ui_state.stall_active {
            "Close Shop"
        } else {
            "Open Shop"
        };

        let (btn_bg, btn_border) = if btn_hovered {
            (Color::new(0.235, 0.204, 0.141, 1.0), FRAME_ACCENT)
        } else {
            (Color::new(0.157, 0.141, 0.110, 1.0), FRAME_MID)
        };

        draw_rectangle(btn_x, btn_y, btn_width, btn_height, btn_border);
        draw_rectangle(
            btn_x + 1.0,
            btn_y + 1.0,
            btn_width - 2.0,
            btn_height - 2.0,
            btn_bg,
        );

        if btn_hovered {
            draw_line(
                btn_x + 2.0,
                btn_y + 2.0,
                btn_x + btn_width - 2.0,
                btn_y + 2.0,
                1.0,
                FRAME_INNER,
            );
        }

        let btn_text_color = if btn_hovered { TEXT_TITLE } else { TEXT_NORMAL };
        let btn_text_w = self.measure_text_sharp(btn_label, 16.0).width;
        self.draw_text_sharp(
            btn_label,
            btn_x + (btn_width - btn_text_w) / 2.0,
            btn_y + 18.0 * scale,
            16.0,
            btn_text_color,
        );
    }

    /// Hover tooltip for the combat style buttons: what the style does and which
    /// skill it trains (shown in parentheses), since Acc/Agg/Def/Ctrl are opaque.
    pub(crate) fn render_combat_style_tooltip(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
    ) {
        if !state.ui_state.character_panel_open {
            return;
        }
        let idx = match hovered {
            Some(UiElementId::CombatStyleButton(i)) => *i,
            _ => return,
        };
        let styles = get_combat_styles_for_weapon(state);
        let (_, _, title, desc, trains) = match styles.get(idx) {
            Some(s) => *s,
            None => return,
        };
        let title_text = format!("{} ({})", title, trains);

        let (mouse_x, mouse_y) = mouse_position();
        let padding = 8.0;
        let line_height = 20.0;
        let font_size = 16.0;

        let max_width = self
            .measure_text_sharp(&title_text, font_size)
            .width
            .max(self.measure_text_sharp(desc, font_size).width);
        let tooltip_width = max_width + padding * 2.0;
        let tooltip_height = padding * 2.0 + line_height * 2.0;

        let (sw, sh) = virtual_screen_size();
        let tooltip_x = (mouse_x + 16.0).min(sw - tooltip_width - 8.0);
        let tooltip_y = (mouse_y + 16.0).min(sh - tooltip_height - 8.0);

        draw_rectangle(
            tooltip_x - 1.0,
            tooltip_y - 1.0,
            tooltip_width + 2.0,
            tooltip_height + 2.0,
            TOOLTIP_FRAME,
        );
        draw_rectangle(
            tooltip_x,
            tooltip_y,
            tooltip_width,
            tooltip_height,
            TOOLTIP_BG,
        );

        let mut text_y = tooltip_y + padding + 14.0;
        self.draw_text_sharp(
            &title_text,
            tooltip_x + padding,
            text_y,
            font_size,
            TEXT_GOLD,
        );
        text_y += line_height;
        self.draw_text_sharp(desc, tooltip_x + padding, text_y, font_size, TEXT_NORMAL);
    }
}
