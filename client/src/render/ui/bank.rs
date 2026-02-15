//! Bank vault UI - two-column grid layout: Bank (left) and Inventory (right)

use macroquad::prelude::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use super::super::Renderer;
use super::common::*;

const COLUMN_GAP: f32 = 10.0;
const HEADER_HEIGHT: f32 = 28.0;
const GOLD_BAR_HEIGHT: f32 = 36.0;
const BANK_COLS: usize = 6;
const INV_COLS: usize = 4;

impl Renderer {
    pub(crate) fn render_bank(&self, state: &GameState, hovered: &Option<UiElementId>, layout: &mut UiLayout) {
        let (sw, sh) = virtual_screen_size();

        let slot_size = INV_SLOT_SIZE;
        let slot_gap = SLOT_SPACING;

        // Calculate panel size from grid dimensions
        let bank_grid_w = BANK_COLS as f32 * (slot_size + slot_gap) - slot_gap;
        let inv_grid_w = INV_COLS as f32 * (slot_size + slot_gap) - slot_gap;
        let padding = 12.0;
        let panel_width = (padding * 2.0 + bank_grid_w + COLUMN_GAP + inv_grid_w + FRAME_THICKNESS * 2.0).min(sw - 16.0);
        let panel_height = (500.0_f32).min(sh - 16.0);
        let panel_x = (sw - panel_width) / 2.0;
        let panel_y = (sh - panel_height) / 2.0;

        // Semi-transparent overlay
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.588));

        // Draw themed panel frame
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        // ===== HEADER =====
        let header_x = panel_x + FRAME_THICKNESS;
        let header_y = panel_y + FRAME_THICKNESS;
        let header_w = panel_width - FRAME_THICKNESS * 2.0;

        draw_rectangle(header_x, header_y, header_w, HEADER_HEIGHT, HEADER_BG);
        draw_line(header_x, header_y + HEADER_HEIGHT, header_x + header_w, header_y + HEADER_HEIGHT, 1.0, HEADER_BORDER);

        let title = "Bank Vault";
        let title_dims = self.measure_text_sharp(title, 16.0);
        self.draw_text_sharp(title, header_x + (header_w - title_dims.width) / 2.0, header_y + 20.0, 16.0, TEXT_TITLE);

        // Close button
        let close_size = 20.0;
        let close_x = header_x + header_w - close_size - 6.0;
        let close_y = header_y + (HEADER_HEIGHT - close_size) / 2.0;
        let close_rect = Rect::new(close_x, close_y, close_size, close_size);
        layout.add(UiElementId::BankCloseButton, close_rect);
        let close_hovered = matches!(hovered, Some(UiElementId::BankCloseButton));
        let close_color = if close_hovered { TEXT_GOLD } else { TEXT_DIM };
        self.draw_text_sharp("X", close_x + 4.0, close_y + 15.0, 16.0, close_color);

        // Content area
        let content_x = panel_x + FRAME_THICKNESS + padding;
        let content_y = header_y + HEADER_HEIGHT + 4.0;
        let content_height = panel_height - FRAME_THICKNESS * 2.0 - HEADER_HEIGHT - 4.0;

        let col_header_h = 26.0;
        let grid_y = content_y + col_header_h + 4.0;
        let grid_height = content_height - col_header_h - 4.0 - GOLD_BAR_HEIGHT - 12.0;
        let gold_y = grid_y + grid_height + 6.0;

        let right_x = content_x + bank_grid_w + COLUMN_GAP;

        // Render grids
        self.render_bank_grid(state, hovered, layout, content_x, grid_y, bank_grid_w, grid_height, slot_size, slot_gap);
        self.render_bank_inv_grid(state, hovered, layout, right_x, grid_y, inv_grid_w, grid_height, slot_size, slot_gap);

        // Column headers (drawn ON TOP to mask overflow)
        // BANK header
        draw_rectangle(content_x, content_y, bank_grid_w, col_header_h, PANEL_BG_MID);
        draw_rectangle(content_x, content_y, bank_grid_w, col_header_h - 2.0, SLOT_BORDER);
        draw_rectangle(content_x + 1.0, content_y + 1.0, bank_grid_w - 2.0, col_header_h - 4.0, PANEL_BG_MID);
        let hdr = "BANK";
        let hdr_dims = self.measure_text_sharp(hdr, 16.0);
        self.draw_text_sharp(hdr, content_x + (bank_grid_w - hdr_dims.width) / 2.0, content_y + 17.0, 16.0, TEXT_TITLE);

        // INVENTORY header
        draw_rectangle(right_x, content_y, inv_grid_w, col_header_h, PANEL_BG_MID);
        draw_rectangle(right_x, content_y, inv_grid_w, col_header_h - 2.0, SLOT_BORDER);
        draw_rectangle(right_x + 1.0, content_y + 1.0, inv_grid_w - 2.0, col_header_h - 4.0, PANEL_BG_MID);
        let hdr2 = "INVENTORY";
        let hdr2_dims = self.measure_text_sharp(hdr2, 16.0);
        self.draw_text_sharp(hdr2, right_x + (inv_grid_w - hdr2_dims.width) / 2.0, content_y + 17.0, 16.0, TEXT_TITLE);

        // Divider
        let divider_x = content_x + bank_grid_w + COLUMN_GAP / 2.0;
        draw_line(divider_x, content_y, divider_x, gold_y + GOLD_BAR_HEIGHT, 1.0, HEADER_BORDER);

        // Gold bar (bottom)
        self.render_bank_gold_bar(state, hovered, layout, content_x, right_x, gold_y, bank_grid_w, inv_grid_w);
    }

    fn render_bank_grid(&self, state: &GameState, hovered: &Option<UiElementId>, layout: &mut UiLayout, x: f32, grid_y: f32, _width: f32, grid_height: f32, slot_size: f32, slot_gap: f32) {
        let total_slots = state.ui_state.bank_slots.len();
        if total_slots == 0 {
            self.draw_text_sharp("Empty", x + 10.0, grid_y + 20.0, 16.0, TEXT_DIM);
            return;
        }

        let row_height = slot_size + slot_gap;
        let total_rows = (total_slots + BANK_COLS - 1) / BANK_COLS;
        let total_grid_height = total_rows as f32 * row_height - slot_gap;
        let needs_scroll = total_grid_height > grid_height;

        let max_scroll = (total_grid_height - grid_height).max(0.0);
        let scroll_offset = state.ui_state.bank_scroll.clamp(0.0, max_scroll);

        // Register scroll area
        let grid_w = BANK_COLS as f32 * (slot_size + slot_gap) - slot_gap;
        let scroll_area = Rect::new(x, grid_y, grid_w, grid_height);
        layout.add(UiElementId::BankScrollArea, scroll_area);

        for i in 0..total_slots {
            let row = i / BANK_COLS;
            let col = i % BANK_COLS;
            let sx = x + col as f32 * (slot_size + slot_gap);
            let sy = grid_y + row as f32 * row_height - scroll_offset;

            // Skip slots outside visible area
            if sy + slot_size < grid_y || sy > grid_y + grid_height {
                continue;
            }

            // Only register hit if fully visible
            if sy >= grid_y - 1.0 && sy + slot_size <= grid_y + grid_height + 1.0 {
                let bounds = Rect::new(sx, sy, slot_size, slot_size);
                layout.add(UiElementId::BankSlot(i), bounds);
            }

            let is_hovered = matches!(hovered, Some(UiElementId::BankSlot(idx)) if *idx == i);
            let has_item = state.ui_state.bank_slots.get(i).map(|s| s.is_some()).unwrap_or(false);

            let slot_state = if is_hovered { SlotState::Hovered } else { SlotState::Normal };
            self.draw_inventory_slot(sx, sy, slot_size, has_item, slot_state);

            if let Some(Some((item_id, quantity))) = state.ui_state.bank_slots.get(i) {
                self.draw_item_icon(item_id, sx, sy, slot_size, slot_size, state, false);

                if *quantity > 1 {
                    let qty_text = quantity.to_string();
                    self.draw_text_sharp(&qty_text, sx + 3.0, sy + slot_size - 4.0, 16.0, Color::new(0.0, 0.0, 0.0, 0.8));
                    self.draw_text_sharp(&qty_text, sx + 2.0, sy + slot_size - 5.0, 16.0, TEXT_NORMAL);
                }
            }
        }
    }

    fn render_bank_inv_grid(&self, state: &GameState, hovered: &Option<UiElementId>, layout: &mut UiLayout, x: f32, grid_y: f32, _width: f32, grid_height: f32, slot_size: f32, slot_gap: f32) {
        let row_height = slot_size + slot_gap;
        let total_slots = 20; // Standard inventory size
        let total_rows = (total_slots + INV_COLS - 1) / INV_COLS;
        let total_grid_height = total_rows as f32 * row_height - slot_gap;
        let needs_scroll = total_grid_height > grid_height;

        let max_scroll = (total_grid_height - grid_height).max(0.0);
        let scroll_offset = state.ui_state.bank_inv_scroll.clamp(0.0, max_scroll);

        // Register scroll area
        let grid_w = INV_COLS as f32 * (slot_size + slot_gap) - slot_gap;
        let scroll_area = Rect::new(x, grid_y, grid_w, grid_height);
        layout.add(UiElementId::BankInvScrollArea, scroll_area);

        for i in 0..total_slots {
            let row = i / INV_COLS;
            let col = i % INV_COLS;
            let sx = x + col as f32 * (slot_size + slot_gap);
            let sy = grid_y + row as f32 * row_height - scroll_offset;

            if sy + slot_size < grid_y || sy > grid_y + grid_height {
                continue;
            }

            if sy >= grid_y - 1.0 && sy + slot_size <= grid_y + grid_height + 1.0 {
                let bounds = Rect::new(sx, sy, slot_size, slot_size);
                layout.add(UiElementId::BankInventorySlot(i), bounds);
            }

            let is_hovered = matches!(hovered, Some(UiElementId::BankInventorySlot(idx)) if *idx == i);
            let has_item = state.inventory.slots.get(i).map(|s| s.is_some()).unwrap_or(false);

            let slot_state = if is_hovered { SlotState::Hovered } else { SlotState::Normal };
            self.draw_inventory_slot(sx, sy, slot_size, has_item, slot_state);

            if let Some(Some(inv_slot)) = state.inventory.slots.get(i) {
                self.draw_item_icon(&inv_slot.item_id, sx, sy, slot_size, slot_size, state, false);

                if inv_slot.quantity > 1 {
                    let qty_text = inv_slot.quantity.to_string();
                    self.draw_text_sharp(&qty_text, sx + 3.0, sy + slot_size - 4.0, 16.0, Color::new(0.0, 0.0, 0.0, 0.8));
                    self.draw_text_sharp(&qty_text, sx + 2.0, sy + slot_size - 5.0, 16.0, TEXT_NORMAL);
                }
            }
        }
    }

    fn render_bank_gold_bar(&self, state: &GameState, hovered: &Option<UiElementId>, layout: &mut UiLayout, left_x: f32, right_x: f32, gold_y: f32, bank_w: f32, inv_w: f32) {
        // Bank gold (left side)
        draw_rectangle(left_x, gold_y, bank_w, GOLD_BAR_HEIGHT, SLOT_BORDER);
        draw_rectangle(left_x + 1.0, gold_y + 1.0, bank_w - 2.0, GOLD_BAR_HEIGHT - 2.0, PANEL_BG_MID);

        let gold_icon_size = 14.0;
        if let Some(texture) = &self.gold_nugget_texture {
            draw_texture_ex(texture, left_x + 8.0, gold_y + 10.0, WHITE, DrawTextureParams {
                dest_size: Some(vec2(gold_icon_size, gold_icon_size)),
                ..Default::default()
            });
        }
        let bank_gold_text = format!("{}g", state.ui_state.bank_gold);
        self.draw_text_sharp(&bank_gold_text, left_x + 8.0 + gold_icon_size + 4.0, gold_y + 23.0, 16.0, TEXT_GOLD);

        // Withdraw gold button
        let btn_w = 76.0;
        let btn_h = 24.0;
        let withdraw_x = left_x + bank_w - btn_w - 6.0;
        let withdraw_y = gold_y + (GOLD_BAR_HEIGHT - btn_h) / 2.0;
        let withdraw_rect = Rect::new(withdraw_x, withdraw_y, btn_w, btn_h);
        layout.add(UiElementId::BankWithdrawGoldButton, withdraw_rect);
        let w_hovered = matches!(hovered, Some(UiElementId::BankWithdrawGoldButton));
        let w_bg = if w_hovered { Color::new(0.3, 0.25, 0.15, 1.0) } else { Color::new(0.2, 0.17, 0.1, 1.0) };
        draw_rectangle(withdraw_x, withdraw_y, btn_w, btn_h, SLOT_BORDER);
        draw_rectangle(withdraw_x + 1.0, withdraw_y + 1.0, btn_w - 2.0, btn_h - 2.0, w_bg);
        let w_text = "Withdraw";
        let w_dims = self.measure_text_sharp(w_text, 16.0);
        self.draw_text_sharp(w_text, withdraw_x + (btn_w - w_dims.width) / 2.0, withdraw_y + 17.0, 16.0, TEXT_NORMAL);

        // Inventory gold (right side)
        draw_rectangle(right_x, gold_y, inv_w, GOLD_BAR_HEIGHT, SLOT_BORDER);
        draw_rectangle(right_x + 1.0, gold_y + 1.0, inv_w - 2.0, GOLD_BAR_HEIGHT - 2.0, PANEL_BG_MID);

        if let Some(texture) = &self.gold_nugget_texture {
            draw_texture_ex(texture, right_x + 8.0, gold_y + 10.0, WHITE, DrawTextureParams {
                dest_size: Some(vec2(gold_icon_size, gold_icon_size)),
                ..Default::default()
            });
        }
        let inv_gold_text = format!("{}g", state.inventory.gold);
        self.draw_text_sharp(&inv_gold_text, right_x + 8.0 + gold_icon_size + 4.0, gold_y + 23.0, 16.0, TEXT_GOLD);

        // Deposit gold button
        let deposit_x = right_x + inv_w - btn_w - 6.0;
        let deposit_y = gold_y + (GOLD_BAR_HEIGHT - btn_h) / 2.0;
        let deposit_rect = Rect::new(deposit_x, deposit_y, btn_w, btn_h);
        layout.add(UiElementId::BankDepositGoldButton, deposit_rect);
        let d_hovered = matches!(hovered, Some(UiElementId::BankDepositGoldButton));
        let d_bg = if d_hovered { Color::new(0.15, 0.25, 0.3, 1.0) } else { Color::new(0.1, 0.17, 0.2, 1.0) };
        draw_rectangle(deposit_x, deposit_y, btn_w, btn_h, SLOT_BORDER);
        draw_rectangle(deposit_x + 1.0, deposit_y + 1.0, btn_w - 2.0, btn_h - 2.0, d_bg);
        let d_text = "Deposit";
        let d_dims = self.measure_text_sharp(d_text, 16.0);
        self.draw_text_sharp(d_text, deposit_x + (btn_w - d_dims.width) / 2.0, deposit_y + 17.0, 16.0, TEXT_NORMAL);
    }
}
