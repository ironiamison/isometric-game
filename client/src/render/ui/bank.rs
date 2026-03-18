//! Bank vault UI - two-column grid layout: Bank (left) and Inventory (right)

use super::super::Renderer;
use super::common::*;
use crate::game::{BankQuantityAction, BankQuantityDialog, GameState};
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

const COLUMN_GAP: f32 = 10.0;
const HEADER_HEIGHT: f32 = 28.0;
const GOLD_BAR_HEIGHT: f32 = 36.0;
const BANK_COLS: usize = 6;
const INV_COLS: usize = 4;

/// Format large quantities with K/M/B abbreviations for bank slot display.
fn format_bank_quantity(qty: i32) -> String {
    if qty < 100_000 {
        qty.to_string()
    } else if qty < 10_000_000 {
        let k = qty / 1000;
        let remainder = (qty % 1000) / 100;
        if remainder > 0 {
            format!("{}.{}K", k, remainder)
        } else {
            format!("{}K", k)
        }
    } else if qty < 1_000_000_000 {
        let m = qty / 1_000_000;
        let remainder = (qty % 1_000_000) / 100_000;
        if remainder > 0 {
            format!("{}.{}M", m, remainder)
        } else {
            format!("{}M", m)
        }
    } else {
        let b = qty / 1_000_000_000;
        let remainder = (qty % 1_000_000_000) / 100_000_000;
        if remainder > 0 {
            format!("{}.{}B", b, remainder)
        } else {
            format!("{}B", b)
        }
    }
}

impl Renderer {
    pub(crate) fn render_bank(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;

        let slot_size = INV_SLOT_SIZE * s;
        let slot_gap = SLOT_SPACING * s;

        // Calculate panel size from grid dimensions
        let bank_grid_w = BANK_COLS as f32 * (slot_size + slot_gap) - slot_gap;
        let inv_grid_w = INV_COLS as f32 * (slot_size + slot_gap) - slot_gap;
        let padding = 12.0 * s;
        let panel_width =
            (padding * 2.0 + bank_grid_w + COLUMN_GAP * s + inv_grid_w + FRAME_THICKNESS * 2.0)
                .min(sw - 16.0);
        let panel_height = (500.0 * s).min(sh - 16.0);
        let panel_x = (sw - panel_width) / 2.0;
        let panel_y = (sh - panel_height) / 2.0;

        let header_h = HEADER_HEIGHT * s;
        let gold_bar_h = GOLD_BAR_HEIGHT * s;

        // Semi-transparent overlay
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.588));

        // Draw themed panel frame
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        // ===== HEADER =====
        let header_x = panel_x + FRAME_THICKNESS;
        let header_y = panel_y + FRAME_THICKNESS;
        let header_w = panel_width - FRAME_THICKNESS * 2.0;

        draw_rectangle(header_x, header_y, header_w, header_h, HEADER_BG);
        draw_line(
            header_x,
            header_y + header_h,
            header_x + header_w,
            header_y + header_h,
            1.0,
            HEADER_BORDER,
        );

        let title = "Bank Vault";
        let title_dims = self.measure_text_sharp(title, 16.0);
        self.draw_text_sharp(
            title,
            header_x + (header_w - title_dims.width) / 2.0,
            header_y + header_h * 0.71,
            16.0,
            TEXT_TITLE,
        );

        // Help button (?) on the left side of header
        let help_size = 20.0 * s;
        let help_x = header_x + 6.0 * s;
        let help_y = header_y + (header_h - help_size) / 2.0;
        let help_rect = Rect::new(help_x, help_y, help_size, help_size);
        layout.add(UiElementId::BankHelpButton, help_rect);
        let help_hovered = matches!(hovered, Some(UiElementId::BankHelpButton));
        let help_bg = if help_hovered {
            Color::new(0.25, 0.22, 0.30, 1.0)
        } else {
            Color::new(0.15, 0.13, 0.18, 1.0)
        };
        let help_border = if help_hovered {
            Color::new(0.6, 0.55, 0.7, 1.0)
        } else {
            Color::new(0.35, 0.32, 0.40, 1.0)
        };
        draw_rectangle(help_x, help_y, help_size, help_size, help_border);
        draw_rectangle(
            help_x + 1.0,
            help_y + 1.0,
            help_size - 2.0,
            help_size - 2.0,
            help_bg,
        );
        let q_dims = self.measure_text_sharp("?", 16.0);
        let help_text_color = if help_hovered {
            TEXT_GOLD
        } else {
            Color::new(0.7, 0.65, 0.5, 1.0)
        };
        self.draw_text_sharp(
            "?",
            help_x + (help_size - q_dims.width) / 2.0,
            help_y + help_size * 0.71,
            16.0,
            help_text_color,
        );

        // Sort button (between help and close)
        let sort_size = 20.0 * s;
        let sort_x = header_x + header_w - 20.0 * s - sort_size - 14.0 * s;
        let sort_y = header_y + (header_h - sort_size) / 2.0;
        let sort_rect = Rect::new(sort_x, sort_y, sort_size, sort_size);
        layout.add(UiElementId::BankSortButton, sort_rect);
        let sort_hovered = matches!(hovered, Some(UiElementId::BankSortButton));
        let sort_bg = if sort_hovered {
            Color::new(0.25, 0.22, 0.30, 1.0)
        } else {
            Color::new(0.15, 0.13, 0.18, 1.0)
        };
        let sort_border = if sort_hovered {
            Color::new(0.6, 0.55, 0.7, 1.0)
        } else {
            Color::new(0.35, 0.32, 0.40, 1.0)
        };
        draw_rectangle(sort_x, sort_y, sort_size, sort_size, sort_border);
        draw_rectangle(
            sort_x + 1.0,
            sort_y + 1.0,
            sort_size - 2.0,
            sort_size - 2.0,
            sort_bg,
        );
        let sort_text_color = if sort_hovered {
            TEXT_GOLD
        } else {
            Color::new(0.7, 0.65, 0.5, 1.0)
        };
        let s_dims = self.measure_text_sharp("S", 14.0);
        self.draw_text_sharp(
            "S",
            sort_x + (sort_size - s_dims.width) / 2.0,
            sort_y + sort_size * 0.71,
            14.0,
            sort_text_color,
        );

        // Close button
        let close_size = 20.0 * s;
        let close_x = header_x + header_w - close_size - 6.0 * s;
        let close_y = header_y + (header_h - close_size) / 2.0;
        let close_rect = Rect::new(close_x, close_y, close_size, close_size);
        layout.add(UiElementId::BankCloseButton, close_rect);
        let close_hovered = matches!(hovered, Some(UiElementId::BankCloseButton));
        let close_color = if close_hovered { TEXT_GOLD } else { TEXT_DIM };
        self.draw_text_sharp(
            "X",
            close_x + (close_size - self.measure_text_sharp("X", 16.0).width) / 2.0,
            close_y + close_size * 0.71,
            16.0,
            close_color,
        );

        // Content area
        let content_x = panel_x + FRAME_THICKNESS + padding;
        let content_y = header_y + header_h + 4.0 * s;
        let content_height = panel_height - FRAME_THICKNESS * 2.0 - header_h - 4.0 * s;

        let col_header_h = 26.0 * s;
        let grid_y = content_y + col_header_h + 4.0 * s;
        let grid_height = content_height - col_header_h - 4.0 * s - gold_bar_h - 12.0 * s;
        let gold_y = grid_y + grid_height + 6.0 * s;

        let right_x = content_x + bank_grid_w + COLUMN_GAP * s;

        // Render grids
        self.render_bank_grid(
            state,
            hovered,
            layout,
            content_x,
            grid_y,
            bank_grid_w,
            grid_height,
            slot_size,
            slot_gap,
        );
        self.render_bank_inv_grid(
            state,
            hovered,
            layout,
            right_x,
            grid_y,
            inv_grid_w,
            grid_height,
            slot_size,
            slot_gap,
        );

        // Column headers (drawn ON TOP to mask overflow)
        // BANK header
        draw_rectangle(
            content_x,
            content_y,
            bank_grid_w,
            col_header_h,
            PANEL_BG_MID,
        );
        draw_rectangle(
            content_x,
            content_y,
            bank_grid_w,
            col_header_h - 2.0,
            SLOT_BORDER,
        );
        draw_rectangle(
            content_x + 1.0,
            content_y + 1.0,
            bank_grid_w - 2.0,
            col_header_h - 4.0,
            PANEL_BG_MID,
        );
        let hdr = "BANK";
        let hdr_dims = self.measure_text_sharp(hdr, 16.0);
        self.draw_text_sharp(
            hdr,
            content_x + (bank_grid_w - hdr_dims.width) / 2.0,
            content_y + col_header_h * 0.65,
            16.0,
            TEXT_TITLE,
        );

        // INVENTORY header
        draw_rectangle(right_x, content_y, inv_grid_w, col_header_h, PANEL_BG_MID);
        draw_rectangle(
            right_x,
            content_y,
            inv_grid_w,
            col_header_h - 2.0,
            SLOT_BORDER,
        );
        draw_rectangle(
            right_x + 1.0,
            content_y + 1.0,
            inv_grid_w - 2.0,
            col_header_h - 4.0,
            PANEL_BG_MID,
        );
        let hdr_pad = 6.0 * s;
        let hdr_text_y = content_y + col_header_h * 0.65;

        let hdr2 = "INVENTORY";
        self.draw_text_sharp(hdr2, right_x + hdr_pad, hdr_text_y, 16.0, TEXT_TITLE);

        // Deposit All text button (right-aligned with matching padding)
        let da_text = "Deposit All";
        let da_text_dims = self.measure_text_sharp(da_text, 16.0);
        let da_text_x = right_x + inv_grid_w - da_text_dims.width - hdr_pad;
        let da_rect = Rect::new(
            da_text_x - 2.0,
            content_y,
            da_text_dims.width + 4.0,
            col_header_h,
        );
        layout.add(UiElementId::BankDepositAllButton, da_rect);
        let da_hovered = matches!(hovered, Some(UiElementId::BankDepositAllButton));
        self.draw_text_sharp(
            da_text,
            da_text_x,
            hdr_text_y,
            16.0,
            if da_hovered { TEXT_GOLD } else { TEXT_DIM },
        );

        // Divider
        let divider_x = content_x + bank_grid_w + COLUMN_GAP * s / 2.0;
        draw_line(
            divider_x,
            content_y,
            divider_x,
            gold_y + gold_bar_h,
            1.0,
            HEADER_BORDER,
        );

        // Gold bar (bottom)
        self.render_bank_gold_bar(
            state,
            hovered,
            layout,
            content_x,
            right_x,
            gold_y,
            bank_grid_w,
            inv_grid_w,
        );

        // Render floating dragged item (on top of everything)
        if let Some(drag) = &state.ui_state.bank_drag {
            if drag.active {
                if let Some(Some((item_id, quantity))) =
                    state.ui_state.bank_slots.get(drag.from_slot)
                {
                    let (mx, my) = mouse_position();
                    let drag_slot_size = slot_size;

                    // Draw the item icon centered on cursor at 80% opacity
                    let sprite_key = state.item_registry.get_sprite_key(item_id);
                    if let Some((texture, source_rect)) = self.item_sprites.get(sprite_key) {
                        let (icon_width, icon_height) = if let Some(r) = source_rect {
                            (r.w, r.h)
                        } else {
                            (texture.width(), texture.height())
                        };
                        let ix = mx - icon_width / 2.0;
                        let iy = my - icon_height / 2.0;

                        draw_texture_ex(
                            texture,
                            ix,
                            iy,
                            Color::new(1.0, 1.0, 1.0, 0.8),
                            DrawTextureParams {
                                source: source_rect,
                                ..Default::default()
                            },
                        );
                    } else {
                        // Fallback: colored rectangle
                        let item_def = state.item_registry.get_or_placeholder(item_id);
                        let mut color = item_def.category_color();
                        color.a = 0.8;
                        let icon_size = 32.0;
                        draw_rectangle(
                            mx - icon_size / 2.0,
                            my - icon_size / 2.0,
                            icon_size,
                            icon_size,
                            color,
                        );
                    }

                    // Draw quantity below cursor if > 1
                    if *quantity > 1 {
                        let qty_text = format_bank_quantity(*quantity);
                        self.draw_text_sharp(
                            &qty_text,
                            mx - drag_slot_size / 2.0 + 3.0 * s,
                            my + drag_slot_size / 2.0 - 4.0 * s,
                            16.0,
                            Color::new(0.0, 0.0, 0.0, 0.6),
                        );
                        self.draw_text_sharp(
                            &qty_text,
                            mx - drag_slot_size / 2.0 + 2.0 * s,
                            my + drag_slot_size / 2.0 - 5.0 * s,
                            16.0,
                            Color::new(TEXT_NORMAL.r, TEXT_NORMAL.g, TEXT_NORMAL.b, 0.8),
                        );
                    }
                }
            }
        }
    }

    fn render_bank_grid(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        x: f32,
        grid_y: f32,
        _width: f32,
        grid_height: f32,
        slot_size: f32,
        slot_gap: f32,
    ) {
        let s = self.font_scale.get();
        let total_slots = state.ui_state.bank_slots.len();
        if total_slots == 0 {
            self.draw_text_sharp("Empty", x + 10.0 * s, grid_y + 20.0 * s, 16.0, TEXT_DIM);
            return;
        }

        let row_height = slot_size + slot_gap;
        let total_rows = (total_slots + BANK_COLS - 1) / BANK_COLS;
        let total_grid_height = total_rows as f32 * row_height - slot_gap;
        let needs_scroll = total_grid_height > grid_height;

        let max_scroll = (total_grid_height - grid_height).max(0.0);
        let scroll_offset = state.ui_state.bank_scroll.clamp(0.0, max_scroll);
        layout.set_max_scroll(UiElementId::BankScrollbar, max_scroll);

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
            let has_item = state
                .ui_state
                .bank_slots
                .get(i)
                .map(|s| s.is_some())
                .unwrap_or(false);

            // Check if this slot is the drag source
            let is_drag_source = state
                .ui_state
                .bank_drag
                .as_ref()
                .map(|d| d.active && d.from_slot == i)
                .unwrap_or(false);

            // Check if this slot is the drag target (hovered during active drag)
            let is_drag_target = is_hovered
                && state
                    .ui_state
                    .bank_drag
                    .as_ref()
                    .map(|d| d.active && d.from_slot != i)
                    .unwrap_or(false);

            let slot_state = if is_hovered {
                SlotState::Hovered
            } else {
                SlotState::Normal
            };
            self.draw_inventory_slot(sx, sy, slot_size, has_item, slot_state);

            if let Some(Some((item_id, quantity))) = state.ui_state.bank_slots.get(i) {
                if is_drag_source {
                    // Dim the source slot: draw item at reduced opacity
                    self.draw_item_icon(item_id, sx, sy, slot_size, slot_size, state, false);
                    // Overlay a dark rectangle to dim to ~30% visibility
                    draw_rectangle(
                        sx + 1.0,
                        sy + 1.0,
                        slot_size - 2.0,
                        slot_size - 2.0,
                        Color::new(0.08, 0.08, 0.11, 0.7),
                    );
                } else {
                    self.draw_item_icon(item_id, sx, sy, slot_size, slot_size, state, false);
                }

                if *quantity > 1 {
                    let qty_text = format_bank_quantity(*quantity);
                    let qty_alpha = if is_drag_source { 0.3 } else { 1.0 };
                    self.draw_text_sharp(
                        &qty_text,
                        sx + 3.0 * s,
                        sy + slot_size - 4.0 * s,
                        16.0,
                        Color::new(0.0, 0.0, 0.0, 0.8 * qty_alpha),
                    );
                    self.draw_text_sharp(
                        &qty_text,
                        sx + 2.0 * s,
                        sy + slot_size - 5.0 * s,
                        16.0,
                        Color::new(
                            TEXT_NORMAL.r,
                            TEXT_NORMAL.g,
                            TEXT_NORMAL.b,
                            TEXT_NORMAL.a * qty_alpha,
                        ),
                    );
                }
            }

            // Gold highlight border on drag target
            if is_drag_target {
                let border_color = TEXT_GOLD;
                // Draw 2px gold border
                draw_rectangle(sx, sy, slot_size, 2.0, border_color);
                draw_rectangle(sx, sy + slot_size - 2.0, slot_size, 2.0, border_color);
                draw_rectangle(sx, sy, 2.0, slot_size, border_color);
                draw_rectangle(sx + slot_size - 2.0, sy, 2.0, slot_size, border_color);

                // If target slot has the same item as source, show merge hint "+"
                if let Some(drag) = &state.ui_state.bank_drag {
                    if let Some(Some((target_item_id, _))) = state.ui_state.bank_slots.get(i) {
                        if let Some(Some((source_item_id, _))) =
                            state.ui_state.bank_slots.get(drag.from_slot)
                        {
                            if target_item_id == source_item_id {
                                let plus_dims = self.measure_text_sharp("+", 16.0);
                                self.draw_text_sharp(
                                    "+",
                                    sx + slot_size - plus_dims.width - 2.0 * s,
                                    sy + 12.0 * s,
                                    16.0,
                                    TEXT_GOLD,
                                );
                            }
                        }
                    }
                }
            }
        }

        // Scrollbar
        if needs_scroll {
            let scrollbar_w: f32 = if cfg!(target_os = "android") {
                12.0
            } else {
                8.0
            };
            let track_x = x + grid_w + 2.0;
            let track_y = grid_y;
            let track_h = grid_height;

            layout.add_scrollbar(
                UiElementId::BankScrollbar,
                Rect::new(track_x, track_y, scrollbar_w, track_h),
            );

            // Track
            draw_rectangle(
                track_x,
                track_y,
                scrollbar_w,
                track_h,
                Color::new(0.1, 0.09, 0.12, 0.6),
            );

            // Thumb
            let thumb_ratio = grid_height / total_grid_height;
            let thumb_h = (track_h * thumb_ratio).max(16.0);
            let scroll_ratio = if max_scroll > 0.0 {
                scroll_offset / max_scroll
            } else {
                0.0
            };
            let thumb_y = track_y + scroll_ratio * (track_h - thumb_h);

            let is_dragging = state.ui_state.bank_scroll_drag.dragging;
            let is_hovered = matches!(hovered, Some(UiElementId::BankScrollbar));
            let thumb_color = if is_dragging || is_hovered {
                Color::new(0.5, 0.45, 0.55, 0.9)
            } else {
                Color::new(0.35, 0.32, 0.40, 0.7)
            };
            draw_rectangle(
                track_x + 1.0,
                thumb_y,
                scrollbar_w - 2.0,
                thumb_h,
                thumb_color,
            );
        }
    }

    fn render_bank_inv_grid(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        x: f32,
        grid_y: f32,
        _width: f32,
        grid_height: f32,
        slot_size: f32,
        slot_gap: f32,
    ) {
        let s = self.font_scale.get();
        let row_height = slot_size + slot_gap;
        let total_slots = 20; // Standard inventory size
        let total_rows = (total_slots + INV_COLS - 1) / INV_COLS;
        let total_grid_height = total_rows as f32 * row_height - slot_gap;
        let needs_scroll = total_grid_height > grid_height;

        let max_scroll = (total_grid_height - grid_height).max(0.0);
        let scroll_offset = state.ui_state.bank_inv_scroll.clamp(0.0, max_scroll);
        layout.set_max_scroll(UiElementId::BankInvScrollbar, max_scroll);

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

            let is_hovered =
                matches!(hovered, Some(UiElementId::BankInventorySlot(idx)) if *idx == i);
            let has_item = state
                .inventory
                .slots
                .get(i)
                .map(|s| s.is_some())
                .unwrap_or(false);

            let slot_state = if is_hovered {
                SlotState::Hovered
            } else {
                SlotState::Normal
            };
            self.draw_inventory_slot(sx, sy, slot_size, has_item, slot_state);

            if let Some(Some(inv_slot)) = state.inventory.slots.get(i) {
                self.draw_item_icon(
                    &inv_slot.item_id,
                    sx,
                    sy,
                    slot_size,
                    slot_size,
                    state,
                    false,
                );

                if inv_slot.quantity > 1 {
                    let qty_text = format_bank_quantity(inv_slot.quantity);
                    self.draw_text_sharp(
                        &qty_text,
                        sx + 3.0 * s,
                        sy + slot_size - 4.0 * s,
                        16.0,
                        Color::new(0.0, 0.0, 0.0, 0.8),
                    );
                    self.draw_text_sharp(
                        &qty_text,
                        sx + 2.0 * s,
                        sy + slot_size - 5.0 * s,
                        16.0,
                        TEXT_NORMAL,
                    );
                }
            }
        }

        // Scrollbar
        if needs_scroll {
            let scrollbar_w: f32 = if cfg!(target_os = "android") {
                12.0
            } else {
                8.0
            };
            let track_x = x + grid_w + 2.0;
            let track_y = grid_y;
            let track_h = grid_height;

            layout.add_scrollbar(
                UiElementId::BankInvScrollbar,
                Rect::new(track_x, track_y, scrollbar_w, track_h),
            );

            // Track
            draw_rectangle(
                track_x,
                track_y,
                scrollbar_w,
                track_h,
                Color::new(0.1, 0.09, 0.12, 0.6),
            );

            // Thumb
            let thumb_ratio = grid_height / total_grid_height;
            let thumb_h = (track_h * thumb_ratio).max(16.0);
            let scroll_ratio = if max_scroll > 0.0 {
                scroll_offset / max_scroll
            } else {
                0.0
            };
            let thumb_y = track_y + scroll_ratio * (track_h - thumb_h);

            let is_dragging = state.ui_state.bank_inv_scroll_drag.dragging;
            let is_hovered = matches!(hovered, Some(UiElementId::BankInvScrollbar));
            let thumb_color = if is_dragging || is_hovered {
                Color::new(0.5, 0.45, 0.55, 0.9)
            } else {
                Color::new(0.35, 0.32, 0.40, 0.7)
            };
            draw_rectangle(
                track_x + 1.0,
                thumb_y,
                scrollbar_w - 2.0,
                thumb_h,
                thumb_color,
            );
        }
    }

    fn render_bank_gold_bar(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        left_x: f32,
        right_x: f32,
        gold_y: f32,
        bank_w: f32,
        inv_w: f32,
    ) {
        let s = self.font_scale.get();
        let gold_bar_h = GOLD_BAR_HEIGHT * s;

        // Bank gold (left side)
        draw_rectangle(left_x, gold_y, bank_w, gold_bar_h, SLOT_BORDER);
        draw_rectangle(
            left_x + 1.0,
            gold_y + 1.0,
            bank_w - 2.0,
            gold_bar_h - 2.0,
            PANEL_BG_MID,
        );

        let gold_icon_size = 14.0 * s;
        if let Some(texture) = &self.gold_nugget_texture {
            draw_texture_ex(
                texture,
                left_x + 8.0 * s,
                gold_y + 10.0 * s,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(gold_icon_size, gold_icon_size)),
                    ..Default::default()
                },
            );
        }
        let bank_gold_text = format!("{}g", state.ui_state.bank_gold);
        self.draw_text_sharp(
            &bank_gold_text,
            left_x + 8.0 * s + gold_icon_size + 4.0 * s,
            gold_y + gold_bar_h * 0.64,
            16.0,
            TEXT_GOLD,
        );

        // Withdraw gold button
        let btn_w = 76.0 * s;
        let btn_h = 24.0 * s;
        let withdraw_x = left_x + bank_w - btn_w - 6.0 * s;
        let withdraw_y = gold_y + (gold_bar_h - btn_h) / 2.0;
        let withdraw_rect = Rect::new(withdraw_x, withdraw_y, btn_w, btn_h);
        layout.add(UiElementId::BankWithdrawGoldButton, withdraw_rect);
        let w_hovered = matches!(hovered, Some(UiElementId::BankWithdrawGoldButton));
        let w_bg = if w_hovered {
            Color::new(0.3, 0.25, 0.15, 1.0)
        } else {
            Color::new(0.2, 0.17, 0.1, 1.0)
        };
        draw_rectangle(withdraw_x, withdraw_y, btn_w, btn_h, SLOT_BORDER);
        draw_rectangle(
            withdraw_x + 1.0,
            withdraw_y + 1.0,
            btn_w - 2.0,
            btn_h - 2.0,
            w_bg,
        );
        let w_text = "Withdraw";
        let w_dims = self.measure_text_sharp(w_text, 16.0);
        self.draw_text_sharp(
            w_text,
            withdraw_x + (btn_w - w_dims.width) / 2.0,
            withdraw_y + btn_h * 0.71,
            16.0,
            TEXT_NORMAL,
        );

        // Inventory gold (right side)
        draw_rectangle(right_x, gold_y, inv_w, gold_bar_h, SLOT_BORDER);
        draw_rectangle(
            right_x + 1.0,
            gold_y + 1.0,
            inv_w - 2.0,
            gold_bar_h - 2.0,
            PANEL_BG_MID,
        );

        if let Some(texture) = &self.gold_nugget_texture {
            draw_texture_ex(
                texture,
                right_x + 8.0 * s,
                gold_y + 10.0 * s,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(gold_icon_size, gold_icon_size)),
                    ..Default::default()
                },
            );
        }
        let inv_gold_text = format!("{}g", state.inventory.gold);
        self.draw_text_sharp(
            &inv_gold_text,
            right_x + 8.0 * s + gold_icon_size + 4.0 * s,
            gold_y + gold_bar_h * 0.64,
            16.0,
            TEXT_GOLD,
        );

        // Deposit gold button
        let deposit_x = right_x + inv_w - btn_w - 6.0 * s;
        let deposit_y = gold_y + (gold_bar_h - btn_h) / 2.0;
        let deposit_rect = Rect::new(deposit_x, deposit_y, btn_w, btn_h);
        layout.add(UiElementId::BankDepositGoldButton, deposit_rect);
        let d_hovered = matches!(hovered, Some(UiElementId::BankDepositGoldButton));
        let d_bg = if d_hovered {
            Color::new(0.15, 0.25, 0.3, 1.0)
        } else {
            Color::new(0.1, 0.17, 0.2, 1.0)
        };
        draw_rectangle(deposit_x, deposit_y, btn_w, btn_h, SLOT_BORDER);
        draw_rectangle(
            deposit_x + 1.0,
            deposit_y + 1.0,
            btn_w - 2.0,
            btn_h - 2.0,
            d_bg,
        );
        let d_text = "Deposit";
        let d_dims = self.measure_text_sharp(d_text, 16.0);
        self.draw_text_sharp(
            d_text,
            deposit_x + (btn_w - d_dims.width) / 2.0,
            deposit_y + btn_h * 0.71,
            16.0,
            TEXT_NORMAL,
        );
    }

    /// Render the bank quantity input dialog (Ctrl+Click)
    pub(crate) fn render_bank_quantity_dialog(
        &self,
        dialog: &BankQuantityDialog,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (sw, sh) = virtual_screen_size();
        let s = self.font_scale.get();

        // Semi-transparent overlay to focus attention
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.45));

        let box_width = 280.0 * s;
        let box_height = 140.0 * s;
        let box_x = (sw - box_width) / 2.0;
        let box_y = (sh - box_height) / 2.0;

        // Draw themed panel frame with corner accents
        self.draw_panel_frame(box_x, box_y, box_width, box_height);
        self.draw_corner_accents(box_x, box_y, box_width, box_height);

        // ===== TITLE TAB =====
        let title_text = match dialog.action {
            BankQuantityAction::DepositItem => "DEPOSIT ITEM",
            BankQuantityAction::WithdrawItem => "WITHDRAW ITEM",
            BankQuantityAction::DepositGold => "DEPOSIT GOLD",
            BankQuantityAction::WithdrawGold => "WITHDRAW GOLD",
        };
        let title_width = self.measure_text_sharp(title_text, 16.0).width + 28.0 * s;
        let title_x = box_x + (box_width - title_width) / 2.0;
        let title_y = box_y - 8.0 * s;
        let title_h = 26.0 * s;

        // Title tab with beveled effect
        draw_rectangle(
            title_x - 1.0,
            title_y - 1.0,
            title_width + 2.0,
            title_h + 2.0,
            FRAME_OUTER,
        );
        draw_rectangle(title_x, title_y, title_width, title_h, HEADER_BG);
        draw_rectangle(
            title_x + 1.0,
            title_y + 1.0,
            title_width - 2.0,
            title_h - 2.0,
            Color::new(0.165, 0.149, 0.188, 1.0),
        );

        // Title tab inner highlight
        draw_line(
            title_x + 2.0,
            title_y + 2.0,
            title_x + title_width - 2.0,
            title_y + 2.0,
            1.0,
            FRAME_INNER,
        );

        // Title text in gold
        self.draw_text_sharp(
            title_text,
            title_x + 14.0 * s,
            title_y + title_h * 0.69,
            16.0,
            TEXT_TITLE,
        );

        // Small decorative accent on title tab corners
        draw_rectangle(title_x, title_y, 3.0, 1.0, FRAME_ACCENT);
        draw_rectangle(title_x + title_width - 3.0, title_y, 3.0, 1.0, FRAME_ACCENT);

        // ===== CONTENT AREA =====
        let content_x = box_x + FRAME_THICKNESS + 12.0 * s;
        let content_y = box_y + FRAME_THICKNESS + 16.0 * s;
        let content_width = box_width - FRAME_THICKNESS * 2.0 - 24.0 * s;

        // Available quantity display
        let available_text = if matches!(
            dialog.action,
            BankQuantityAction::DepositGold | BankQuantityAction::WithdrawGold
        ) {
            format!("Available: {}g", dialog.max_quantity)
        } else {
            format!("Available: {}", dialog.max_quantity)
        };
        self.draw_text_sharp(
            &available_text,
            content_x,
            content_y + 16.0 * s,
            16.0,
            TEXT_GOLD,
        );

        // ===== INPUT FIELD =====
        let input_y = content_y + 36.0 * s;
        let input_height = 28.0 * s;
        let input_width = content_width;

        // Input field background
        draw_rectangle(content_x, input_y, input_width, input_height, SLOT_BORDER);
        draw_rectangle(
            content_x + 1.0,
            input_y + 1.0,
            input_width - 2.0,
            input_height - 2.0,
            SLOT_BG_EMPTY,
        );

        // Inner shadow
        draw_line(
            content_x + 2.0,
            input_y + 2.0,
            content_x + input_width - 2.0,
            input_y + 2.0,
            1.0,
            SLOT_INNER_SHADOW,
        );
        draw_line(
            content_x + 2.0,
            input_y + 2.0,
            content_x + 2.0,
            input_y + input_height - 2.0,
            1.0,
            SLOT_INNER_SHADOW,
        );

        // Input text
        let input_text_x = content_x + 8.0 * s;
        let input_text_y = input_y + input_height * 0.68;

        if dialog.input.is_empty() {
            self.draw_text_sharp(
                "Enter amount...",
                input_text_x,
                input_text_y,
                16.0,
                TEXT_DIM,
            );
        } else {
            self.draw_text_sharp(&dialog.input, input_text_x, input_text_y, 16.0, TEXT_NORMAL);
        }

        // Blinking cursor
        let cursor_visible = (macroquad::time::get_time() * 2.0) as i32 % 2 == 0;
        if cursor_visible {
            let text_before_cursor: String = dialog.input.chars().take(dialog.cursor).collect();
            let cursor_x = input_text_x + self.measure_text_sharp(&text_before_cursor, 16.0).width;
            draw_rectangle(
                cursor_x,
                input_y + 6.0 * s,
                2.0,
                input_height - 12.0 * s,
                TEXT_NORMAL,
            );
        }

        // ===== BUTTONS =====
        let button_y = input_y + input_height + 12.0 * s;
        let gap = 8.0 * s;
        let button_width = (content_width - gap * 2.0) / 3.0;
        let button_height = 28.0 * s;

        // Confirm button
        let confirm_x = content_x;
        let confirm_bounds = Rect::new(confirm_x, button_y, button_width, button_height);
        layout.add(UiElementId::BankQuantityConfirm, confirm_bounds);

        let confirm_hovered = matches!(hovered, Some(UiElementId::BankQuantityConfirm));
        let (confirm_bg, confirm_border) = if confirm_hovered {
            (Color::new(0.235, 0.204, 0.141, 1.0), FRAME_ACCENT)
        } else {
            (Color::new(0.157, 0.141, 0.110, 1.0), FRAME_MID)
        };

        draw_rectangle(
            confirm_x,
            button_y,
            button_width,
            button_height,
            confirm_border,
        );
        draw_rectangle(
            confirm_x + 1.0,
            button_y + 1.0,
            button_width - 2.0,
            button_height - 2.0,
            confirm_bg,
        );

        if confirm_hovered {
            draw_line(
                confirm_x + 2.0,
                button_y + 2.0,
                confirm_x + button_width - 2.0,
                button_y + 2.0,
                1.0,
                FRAME_INNER,
            );
        }

        let confirm_text_color = if confirm_hovered {
            TEXT_TITLE
        } else {
            TEXT_NORMAL
        };
        let confirm_text = "Confirm";
        let confirm_text_width = self.measure_text_sharp(confirm_text, 16.0).width;
        self.draw_text_sharp(
            confirm_text,
            confirm_x + (button_width - confirm_text_width) / 2.0,
            button_y + button_height * 0.68,
            16.0,
            confirm_text_color,
        );

        // Max button
        let max_x = content_x + button_width + gap;
        let max_bounds = Rect::new(max_x, button_y, button_width, button_height);
        layout.add(UiElementId::BankQuantityMax, max_bounds);

        let max_hovered = matches!(hovered, Some(UiElementId::BankQuantityMax));
        let (max_bg, max_border) = if max_hovered {
            (Color::new(0.141, 0.204, 0.235, 1.0), Color::new(0.4, 0.7, 0.9, 1.0))
        } else {
            (Color::new(0.110, 0.141, 0.157, 1.0), FRAME_MID)
        };

        draw_rectangle(max_x, button_y, button_width, button_height, max_border);
        draw_rectangle(
            max_x + 1.0,
            button_y + 1.0,
            button_width - 2.0,
            button_height - 2.0,
            max_bg,
        );

        if max_hovered {
            draw_line(
                max_x + 2.0,
                button_y + 2.0,
                max_x + button_width - 2.0,
                button_y + 2.0,
                1.0,
                FRAME_INNER,
            );
        }

        let max_text_color = if max_hovered {
            TEXT_TITLE
        } else {
            TEXT_NORMAL
        };
        let max_text = "Max";
        let max_text_width = self.measure_text_sharp(max_text, 16.0).width;
        self.draw_text_sharp(
            max_text,
            max_x + (button_width - max_text_width) / 2.0,
            button_y + button_height * 0.68,
            16.0,
            max_text_color,
        );

        // Cancel button
        let cancel_x = content_x + (button_width + gap) * 2.0;
        let cancel_bounds = Rect::new(cancel_x, button_y, button_width, button_height);
        layout.add(UiElementId::BankQuantityCancel, cancel_bounds);

        let cancel_hovered = matches!(hovered, Some(UiElementId::BankQuantityCancel));
        let (cancel_bg, cancel_border) = if cancel_hovered {
            (
                Color::new(0.235, 0.141, 0.141, 1.0),
                Color::new(0.8, 0.4, 0.4, 1.0),
            )
        } else {
            (Color::new(0.157, 0.110, 0.110, 1.0), FRAME_MID)
        };

        draw_rectangle(
            cancel_x,
            button_y,
            button_width,
            button_height,
            cancel_border,
        );
        draw_rectangle(
            cancel_x + 1.0,
            button_y + 1.0,
            button_width - 2.0,
            button_height - 2.0,
            cancel_bg,
        );

        if cancel_hovered {
            draw_line(
                cancel_x + 2.0,
                button_y + 2.0,
                cancel_x + button_width - 2.0,
                button_y + 2.0,
                1.0,
                FRAME_INNER,
            );
        }

        let cancel_text_color = if cancel_hovered {
            TEXT_TITLE
        } else {
            TEXT_NORMAL
        };
        let cancel_text = "Cancel";
        let cancel_text_width = self.measure_text_sharp(cancel_text, 16.0).width;
        self.draw_text_sharp(
            cancel_text,
            cancel_x + (button_width - cancel_text_width) / 2.0,
            button_y + button_height * 0.68,
            16.0,
            cancel_text_color,
        );
    }

    /// Render the bank help overlay explaining controls
    pub(crate) fn render_bank_help_overlay(
        &self,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (screen_w, screen_h) = virtual_screen_size();
        let s = self.font_scale.get();

        let overlay_w = (240.0 * s).min(screen_w - 20.0);
        let font_size = 16.0;
        let line_height = 20.0 * s;
        let padding = 10.0 * s;
        let section_gap = 6.0 * s;

        let lines: Vec<(&str, Color)> = vec![
            ("Bank Controls", TEXT_TITLE),
            ("", TEXT_NORMAL),
            ("Items & Gold", TEXT_GOLD),
            ("Click - Deposit/Withdraw 1", TEXT_NORMAL),
            ("Ctrl+Click - Enter custom amount", TEXT_NORMAL),
            ("Shift+Click - Deposit/Withdraw all", TEXT_NORMAL),
        ];

        let content_height = lines.len() as f32 * line_height + section_gap;
        let close_btn_height = 22.0 * s;
        let overlay_h = padding * 2.0 + content_height + close_btn_height + 4.0;

        let overlay_x = ((screen_w - overlay_w) / 2.0).floor();
        let overlay_y = ((screen_h - overlay_h) / 2.0).floor();

        // Dim background
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.0, 0.0, 0.0, 0.5));

        // Panel frame
        let accent = TEXT_GOLD;
        draw_rectangle(
            overlay_x - 2.0,
            overlay_y - 2.0,
            overlay_w + 4.0,
            overlay_h + 4.0,
            accent,
        );
        draw_rectangle(
            overlay_x - 1.0,
            overlay_y - 1.0,
            overlay_w + 2.0,
            overlay_h + 2.0,
            TOOLTIP_FRAME,
        );
        draw_rectangle(overlay_x, overlay_y, overlay_w, overlay_h, TOOLTIP_BG);

        // Draw text lines
        let mut text_y = overlay_y + padding + 12.0 * s;
        for (text, color) in &lines {
            if !text.is_empty() {
                self.draw_text_sharp(
                    text,
                    (overlay_x + padding).floor(),
                    text_y.floor(),
                    font_size,
                    *color,
                );
            }
            text_y += line_height;
        }
        text_y += section_gap;

        // Close button
        let close_text = "Got it!";
        let close_dims = self.measure_text_sharp(close_text, 16.0);
        let close_btn_w = (close_dims.width + 20.0 * s).min(overlay_w - padding * 2.0);
        let close_btn_x = overlay_x + (overlay_w - close_btn_w) / 2.0;
        let close_btn_y = text_y;

        layout.add(
            UiElementId::BankHelpClose,
            Rect::new(close_btn_x, close_btn_y, close_btn_w, close_btn_height),
        );

        let close_hovered = matches!(hovered, Some(UiElementId::BankHelpClose));
        let close_bg = if close_hovered {
            accent
        } else {
            Color::new(0.15, 0.13, 0.18, 1.0)
        };
        let close_text_color = if close_hovered {
            Color::new(0.05, 0.05, 0.07, 1.0)
        } else {
            accent
        };

        draw_rectangle(
            close_btn_x,
            close_btn_y,
            close_btn_w,
            close_btn_height,
            Color::new(0.3, 0.28, 0.35, 1.0),
        );
        draw_rectangle(
            close_btn_x + 1.0,
            close_btn_y + 1.0,
            close_btn_w - 2.0,
            close_btn_height - 2.0,
            close_bg,
        );
        let close_text_x = close_btn_x + (close_btn_w - close_dims.width) / 2.0;
        self.draw_text_sharp(
            close_text,
            close_text_x.floor(),
            (close_btn_y + close_btn_height * 0.71).floor(),
            16.0,
            close_text_color,
        );
    }
}
