//! Altar bone offering panel

use macroquad::prelude::*;
use crate::game::{GameState, AltarPanelState};
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use super::super::Renderer;
use super::common::*;

/// Info about a bone type in inventory for display
struct BoneRow {
    item_id: String,
    display_name: String,
    quantity: i32,
    xp_per_bone: i32,
}

/// Get altar XP for a bone item (hardcoded values matching server)
fn altar_xp_for_bone(item_id: &str, prayer_xp: i32) -> i32 {
    match item_id {
        "regular_bones" => 12,
        "big_bones" => 37,
        "dragon_bones" => 180,
        _ => (prayer_xp as f32 * 2.5) as i32,
    }
}

impl Renderer {
    pub(crate) fn render_altar_panel(&self, panel: &AltarPanelState, state: &GameState, hovered: &Option<UiElementId>, layout: &mut UiLayout) {
        let (sw, sh) = virtual_screen_size();

        // Collect bone types from inventory
        let mut bone_rows: Vec<BoneRow> = Vec::new();
        for slot in state.inventory.slots.iter().flatten() {
            if !slot.item_id.contains("bones") {
                continue;
            }
            let item_def = state.item_registry.get_or_placeholder(&slot.item_id);
            if item_def.prayer_xp <= 0 {
                continue;
            }
            // Merge with existing row or create new (dedup by item_id)
            if let Some(row) = bone_rows.iter_mut().find(|r| r.item_id == slot.item_id) {
                row.quantity += slot.quantity;
            } else {
                bone_rows.push(BoneRow {
                    item_id: slot.item_id.clone(),
                    display_name: item_def.display_name.clone(),
                    quantity: slot.quantity,
                    xp_per_bone: altar_xp_for_bone(&slot.item_id, item_def.prayer_xp),
                });
            }
        }

        // Semi-transparent overlay
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.45));

        let row_height = 40.0;
        let header_height = 24.0;
        let pray_button_height = 32.0;
        let padding = 8.0;
        let box_width = 320.0;
        let content_rows_height = if bone_rows.is_empty() { 24.0 } else { bone_rows.len() as f32 * row_height };
        let box_height = header_height + content_rows_height + pray_button_height + padding * 3.0;
        let box_x = (sw - box_width) / 2.0;
        let box_y = (sh - box_height) / 2.0;

        // Panel frame
        self.draw_panel_frame(box_x, box_y, box_width, box_height);
        self.draw_corner_accents(box_x, box_y, box_width, box_height);

        // Title tab
        let title_text = &panel.altar_name;
        let title_width = self.measure_text_sharp(title_text, 16.0).width + 28.0;
        let title_x = box_x + (box_width - title_width) / 2.0;
        let title_y = box_y - 8.0;
        let title_h = 26.0;

        draw_rectangle(title_x - 1.0, title_y - 1.0, title_width + 2.0, title_h + 2.0, FRAME_OUTER);
        draw_rectangle(title_x, title_y, title_width, title_h, HEADER_BG);
        draw_rectangle(title_x + 1.0, title_y + 1.0, title_width - 2.0, title_h - 2.0, Color::new(0.165, 0.149, 0.188, 1.0));
        draw_line(title_x + 2.0, title_y + 2.0, title_x + title_width - 2.0, title_y + 2.0, 1.0, FRAME_INNER);
        self.draw_text_sharp(title_text, title_x + 14.0, title_y + 18.0, 16.0, TEXT_TITLE);

        // Close button (X) top-right
        let close_size = 20.0;
        let close_x = box_x + box_width - close_size - FRAME_THICKNESS - 4.0;
        let close_y = box_y + FRAME_THICKNESS + 4.0;
        let close_bounds = Rect::new(close_x, close_y, close_size, close_size);
        layout.add(UiElementId::AltarClose, close_bounds);

        let close_hovered = matches!(hovered, Some(UiElementId::AltarClose));
        let close_color = if close_hovered { Color::new(0.9, 0.3, 0.3, 1.0) } else { TEXT_DIM };
        self.draw_text_sharp("X", close_x + 4.0, close_y + 15.0, 16.0, close_color);

        // Content area
        let content_x = box_x + FRAME_THICKNESS + padding;
        let content_y = box_y + FRAME_THICKNESS + header_height;
        let content_width = box_width - FRAME_THICKNESS * 2.0 - padding * 2.0;

        if bone_rows.is_empty() {
            self.draw_text_sharp("You have no bones to offer.", content_x, content_y + 20.0, 16.0, TEXT_DIM);
        } else {
            for (i, row) in bone_rows.iter().enumerate() {
                let row_y = content_y + (i as f32 * row_height);

                // Row background (subtle alternating)
                if i % 2 == 0 {
                    draw_rectangle(content_x - 4.0, row_y, content_width + 8.0, row_height, Color::new(0.1, 0.1, 0.12, 0.3));
                }

                // Bone name and quantity
                let name_text = format!("{} x{}", row.display_name, row.quantity);
                self.draw_text_sharp(&name_text, content_x, row_y + 16.0, 16.0, TEXT_NORMAL);

                // XP info
                let total_xp = row.xp_per_bone as i64 * row.quantity as i64;
                let xp_text = format!("{}xp ea / {}xp total", row.xp_per_bone, total_xp);
                self.draw_text_sharp(&xp_text, content_x, row_y + 32.0, 16.0, TEXT_DIM);

                // Offer All button (right side)
                let btn_width = 76.0;
                let btn_height = 28.0;
                let btn_x = content_x + content_width - btn_width;
                let btn_y = row_y + (row_height - btn_height) / 2.0;
                let btn_bounds = Rect::new(btn_x, btn_y, btn_width, btn_height);
                layout.add(UiElementId::AltarOfferAll(i), btn_bounds);

                let btn_hovered = matches!(hovered, Some(UiElementId::AltarOfferAll(idx)) if *idx == i);
                let (btn_bg, btn_border) = if btn_hovered {
                    (Color::new(0.235, 0.204, 0.141, 1.0), FRAME_ACCENT)
                } else {
                    (Color::new(0.157, 0.141, 0.110, 1.0), FRAME_MID)
                };

                draw_rectangle(btn_x, btn_y, btn_width, btn_height, btn_border);
                draw_rectangle(btn_x + 1.0, btn_y + 1.0, btn_width - 2.0, btn_height - 2.0, btn_bg);

                let btn_text_color = if btn_hovered { TEXT_TITLE } else { TEXT_NORMAL };
                let offer_text = "Offer All";
                let offer_w = self.measure_text_sharp(offer_text, 16.0).width;
                self.draw_text_sharp(offer_text, btn_x + (btn_width - offer_w) / 2.0, btn_y + 18.0, 16.0, btn_text_color);
            }
        }

        // Pray button at bottom
        let pray_y = content_y + content_rows_height + padding;
        let pray_width = content_width;
        let pray_bounds = Rect::new(content_x, pray_y, pray_width, pray_button_height);
        layout.add(UiElementId::AltarPray, pray_bounds);

        let pray_hovered = matches!(hovered, Some(UiElementId::AltarPray));
        let (pray_bg, pray_border) = if pray_hovered {
            (Color::new(0.141, 0.180, 0.235, 1.0), Color::new(0.4, 0.6, 0.9, 1.0))
        } else {
            (Color::new(0.110, 0.130, 0.157, 1.0), FRAME_MID)
        };

        draw_rectangle(content_x, pray_y, pray_width, pray_button_height, pray_border);
        draw_rectangle(content_x + 1.0, pray_y + 1.0, pray_width - 2.0, pray_button_height - 2.0, pray_bg);

        if pray_hovered {
            draw_line(content_x + 2.0, pray_y + 2.0, content_x + pray_width - 2.0, pray_y + 2.0, 1.0, FRAME_INNER);
        }

        let pray_text_color = if pray_hovered { Color::new(0.7, 0.85, 1.0, 1.0) } else { TEXT_NORMAL };
        let pray_text = "Pray (Restore Prayer Points)";
        let pray_text_w = self.measure_text_sharp(pray_text, 16.0).width;
        self.draw_text_sharp(pray_text, content_x + (pray_width - pray_text_w) / 2.0, pray_y + 21.0, 16.0, pray_text_color);
    }
}
