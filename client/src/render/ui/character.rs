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
const CHARACTER_GRID_WIDTH: f32 = 3.0 * EQUIP_SLOT_SIZE + 2.0 * EQUIP_SLOT_SPACING; // 122
const CHARACTER_GRID_HEIGHT: f32 = 4.0 * EQUIP_SLOT_SIZE + 3.0 * EQUIP_SLOT_SPACING + 26.0; // 190
const CHARACTER_PANEL_WIDTH: f32 = 240.0; // Unified width to match inventory and other UI panels
const SHOP_BUTTON_HEIGHT: f32 = 26.0;
const COMBAT_STYLE_ROW_HEIGHT: f32 = 26.0;
const CHARACTER_PANEL_HEIGHT: f32 = FRAME_THICKNESS * 2.0
    + CHARACTER_HEADER_HEIGHT
    + CHARACTER_PANEL_PADDING
    + CHARACTER_GRID_HEIGHT
    + CHARACTER_PANEL_PADDING
    + COMBAT_STYLE_ROW_HEIGHT
    + CHARACTER_PANEL_PADDING
    + SHOP_BUTTON_HEIGHT
    + CHARACTER_PANEL_PADDING; // ~338
const STATS_SECTION_GAP: f32 = 8.0; // Gap between equipment grid and stats

/// Combat style labels and short forms
const COMBAT_STYLES: [(&str, &str); 4] = [
    ("accurate", "Acc"),
    ("aggressive", "Agg"),
    ("defensive", "Def"),
    ("controlled", "Ctrl"),
];

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
        let button_size = MENU_BUTTON_SIZE * scale;
        let exp_bar_gap = EXP_BAR_GAP * scale;
        let stats_gap = STATS_SECTION_GAP * scale;
        let grid_width = CHARACTER_GRID_WIDTH * scale;

        // Position panel on right side, top-aligned with HP bar, bottom above menu buttons
        let panel_x = screen_w - panel_width - 8.0;
        let button_area_height = button_size + exp_bar_gap;
        let panel_top = 45.0; // Align with top of HP bar (below name tag)
        let panel_bottom = screen_h - button_area_height - 8.0;
        let panel_height = (panel_bottom - panel_top).min(panel_height);
        let panel_y = panel_bottom - panel_height;

        // Draw panel frame
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        // Header
        let header_x = panel_x + frame_thickness;
        let header_y = panel_y + frame_thickness;
        let header_w = panel_width - frame_thickness * 2.0;

        draw_rectangle(header_x, header_y, header_w, header_height, HEADER_BG);
        draw_line(
            header_x,
            header_y + header_height,
            header_x + header_w,
            header_y + header_height,
            1.0,
            HEADER_BORDER,
        );

        // Header text (native font size for crisp rendering)
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

        // Grid area
        let grid_x = panel_x + frame_thickness + panel_padding;
        let grid_y = header_y + header_height + panel_padding;

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

        // Stats section - to the right of equipment grid
        let stats_x = grid_x + grid_width + stats_gap;
        let stats_y = grid_y;

        // Get player stats (native font size for crisp rendering)
        if let Some(player) = state.get_local_player() {
            let line_height = 24.0 * scale;
            // Center stats horizontally in the space right of the equipment grid
            let available_width = panel_x + panel_width - frame_thickness - stats_x;
            let label_w = self.measure_text_sharp("DEF", 16.0).width;
            let gap = 4.0;
            // Estimate widest value (e.g. "+99")
            let value_w = self.measure_text_sharp("+99", 16.0).width;
            let total_stats_w = label_w + gap + value_w;
            let label_x = stats_x + (available_width - total_stats_w) / 2.0 + 6.0;
            let value_x = label_x + label_w + gap;
            let mut text_y = stats_y + 18.0 * scale;

            // Equipment bonuses
            let atk_bonus = player.attack_bonus(&state.item_registry);
            let str_bonus = player.strength_bonus(&state.item_registry);
            let def_bonus = player.defence_bonus(&state.item_registry);

            // Stats list (font stays at native 16.0 for crisp text)
            self.draw_text_sharp("ATK", label_x, text_y, 16.0, CATEGORY_EQUIPMENT);
            let atk_val = format!("+{}", atk_bonus);
            self.draw_text_sharp(&atk_val, value_x, text_y, 16.0, CATEGORY_EQUIPMENT);
            text_y += line_height;

            self.draw_text_sharp("STR", label_x, text_y, 16.0, CATEGORY_CONSUMABLE);
            let str_val = format!("+{}", str_bonus);
            self.draw_text_sharp(&str_val, value_x, text_y, 16.0, CATEGORY_CONSUMABLE);
            text_y += line_height;

            self.draw_text_sharp("DEF", label_x, text_y, 16.0, CATEGORY_MATERIAL);
            let def_val = format!("+{}", def_bonus);
            self.draw_text_sharp(&def_val, value_x, text_y, 16.0, CATEGORY_MATERIAL);
        }

        // ===== COMBAT STYLE SELECTOR =====
        let style_row_height = COMBAT_STYLE_ROW_HEIGHT * scale;
        let style_area_width = panel_width - frame_thickness * 2.0 - panel_padding * 2.0;
        let style_x = panel_x + frame_thickness + panel_padding;
        let style_y = panel_y + panel_height
            - frame_thickness
            - panel_padding
            - SHOP_BUTTON_HEIGHT * scale
            - panel_padding
            - style_row_height;

        // Label
        let label = "Style:";
        let label_w = self.measure_text_sharp(label, 16.0).width;
        self.draw_text_sharp(
            label,
            style_x,
            style_y + 17.0 * scale,
            16.0,
            TEXT_DIM,
        );

        // 4 style buttons filling the remaining width
        let buttons_x = style_x + label_w + 6.0;
        let buttons_width = style_area_width - label_w - 6.0;
        let gap = 3.0 * scale;
        let btn_w = ((buttons_width - gap * 3.0) / 4.0).floor();

        let current_style = state
            .get_local_player()
            .map(|p| p.combat_style.clone())
            .unwrap_or_else(|| "accurate".to_string());

        for (i, (style_id, style_label)) in COMBAT_STYLES.iter().enumerate() {
            let bx = buttons_x + i as f32 * (btn_w + gap);
            let by = style_y;
            let bounds = Rect::new(bx, by, btn_w, style_row_height);
            layout.add(UiElementId::CombatStyleButton(i), bounds);

            let is_active = current_style == *style_id;
            let is_hovered = matches!(hovered, Some(UiElementId::CombatStyleButton(idx)) if *idx == i);

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

        // ===== OPEN SHOP BUTTON =====
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
}
