//! Stall browse panel (for buyers viewing another player's shop)

use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

// Local color aliases
const PANEL_BG: Color = Color::new(0.071, 0.071, 0.094, 0.96);
const PANEL_BORDER: Color = Color::new(0.322, 0.243, 0.165, 1.0);
const SLOT_BG: Color = Color::new(0.125, 0.125, 0.173, 1.0);
const SLOT_HOVER: Color = Color::new(0.188, 0.188, 0.282, 1.0);
const SLOT_SELECTED: Color = Color::new(0.22, 0.22, 0.35, 1.0);
const BUTTON_BG: Color = Color::new(0.141, 0.125, 0.165, 1.0);
const BUTTON_HOVER: Color = Color::new(0.188, 0.188, 0.282, 1.0);

impl super::super::Renderer {
    pub(crate) fn render_stall_browse_panel(&self, state: &GameState, layout: &mut UiLayout) {
        let browse = match &state.ui_state.stall_browse {
            Some(b) => b,
            None => return,
        };

        let (sw, sh) = virtual_screen_size();
        let panel_w = 340.0_f32.min(sw - 20.0);
        let panel_h = 360.0_f32.min(sh - 40.0);
        let panel_x = ((sw - panel_w) / 2.0).floor();
        let panel_y = ((sh - panel_h) / 2.0).floor();

        draw_rectangle(panel_x, panel_y, panel_w, panel_h, PANEL_BG);
        draw_rectangle_lines(panel_x, panel_y, panel_w, panel_h, 2.0, PANEL_BORDER);

        // Title
        let title = format!("{}'s \"{}\"", browse.seller_name, browse.stall_name);
        let title_w = self.measure_text_sharp(&title, 15.0).width;
        self.draw_text_sharp(&title, (panel_x + (panel_w - title_w) / 2.0).floor(), panel_y + 20.0, 15.0, TEXT_TITLE);

        // Items list
        let slot_h = 30.0;
        let mut y = panel_y + 38.0;
        for (i, item) in browse.items.iter().enumerate() {
            let bounds = Rect::new(panel_x + 8.0, y, panel_w - 16.0, slot_h);
            layout.add(UiElementId::StallBrowseItem(i), bounds);

            let selected = state.ui_state.stall_browse_selected == i;
            let hovered = state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallBrowseItem(i));
            let bg = if selected { SLOT_SELECTED } else if hovered { SLOT_HOVER } else { SLOT_BG };
            draw_rectangle(bounds.x, bounds.y, bounds.w, bounds.h, bg);

            let item_def = state.item_registry.get_or_placeholder(&item.item_id);
            let label = format!("{} x{}  {}g each", item_def.display_name, item.quantity, item.price);
            self.draw_text_sharp(&label, bounds.x + 4.0, y + 20.0, 13.0, TEXT_NORMAL);

            y += slot_h + 2.0;
        }

        if browse.items.is_empty() {
            self.draw_text_sharp("No items for sale", panel_x + panel_w / 2.0 - 50.0, y + 20.0, 13.0, TEXT_DIM);
        }

        // Buy controls at bottom
        let bottom_y = panel_y + panel_h - 70.0;

        // Quantity controls
        let qty_label = format!("Qty: {}", state.ui_state.stall_buy_quantity);
        self.draw_text_sharp(&qty_label, panel_x + panel_w / 2.0 - 20.0, bottom_y + 18.0, 14.0, TEXT_NORMAL);

        let minus_bounds = Rect::new(panel_x + panel_w / 2.0 - 60.0, bottom_y, 28.0, 26.0);
        layout.add(UiElementId::StallBrowseQuantityMinus, minus_bounds);
        let minus_hovered = state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallBrowseQuantityMinus);
        draw_rectangle(minus_bounds.x, minus_bounds.y, minus_bounds.w, minus_bounds.h,
            if minus_hovered { BUTTON_HOVER } else { BUTTON_BG });
        self.draw_text_sharp("-", minus_bounds.x + 10.0, minus_bounds.y + 18.0, 14.0, TEXT_NORMAL);

        let plus_bounds = Rect::new(panel_x + panel_w / 2.0 + 40.0, bottom_y, 28.0, 26.0);
        layout.add(UiElementId::StallBrowseQuantityPlus, plus_bounds);
        let plus_hovered = state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallBrowseQuantityPlus);
        draw_rectangle(plus_bounds.x, plus_bounds.y, plus_bounds.w, plus_bounds.h,
            if plus_hovered { BUTTON_HOVER } else { BUTTON_BG });
        self.draw_text_sharp("+", plus_bounds.x + 10.0, plus_bounds.y + 18.0, 14.0, TEXT_NORMAL);

        // Total price
        let selected_item = browse.items.get(state.ui_state.stall_browse_selected);
        let total = selected_item.map_or(0, |item| item.price * state.ui_state.stall_buy_quantity);
        let total_label = format!("Total: {}g", total);
        let total_w = self.measure_text_sharp(&total_label, 14.0).width;
        self.draw_text_sharp(&total_label, (panel_x + (panel_w - total_w) / 2.0).floor(), bottom_y + 44.0, 14.0, TEXT_GOLD);

        // Buy button
        let buy_bounds = Rect::new(panel_x + panel_w - 80.0, bottom_y + 30.0, 68.0, 28.0);
        layout.add(UiElementId::StallBrowseBuyButton, buy_bounds);
        let buy_hovered = state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallBrowseBuyButton);
        draw_rectangle(buy_bounds.x, buy_bounds.y, buy_bounds.w, buy_bounds.h,
            if buy_hovered { BUTTON_HOVER } else { BUTTON_BG });
        self.draw_text_sharp("Buy", buy_bounds.x + 20.0, buy_bounds.y + 19.0, 14.0, Color::new(0.2, 0.9, 0.2, 1.0));

        // Close button
        let close_bounds = Rect::new(panel_x + 8.0, bottom_y + 30.0, 68.0, 28.0);
        layout.add(UiElementId::StallBrowseCloseButton, close_bounds);
        let close_hovered = state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallBrowseCloseButton);
        draw_rectangle(close_bounds.x, close_bounds.y, close_bounds.w, close_bounds.h,
            if close_hovered { BUTTON_HOVER } else { BUTTON_BG });
        self.draw_text_sharp("Close", close_bounds.x + 14.0, close_bounds.y + 19.0, 14.0, TEXT_NORMAL);
    }
}
