//! Player stall setup panel (for the shop owner)

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
const BUTTON_BG: Color = Color::new(0.141, 0.125, 0.165, 1.0);
const BUTTON_HOVER: Color = Color::new(0.188, 0.188, 0.282, 1.0);

impl super::super::Renderer {
    pub(crate) fn render_stall_setup_panel(&self, state: &GameState, layout: &mut UiLayout) {
        if !state.ui_state.stall_setup_open {
            return;
        }

        let (sw, sh) = virtual_screen_size();
        let panel_w = 340.0_f32.min(sw - 20.0);
        let panel_h = 380.0_f32.min(sh - 40.0);
        let panel_x = ((sw - panel_w) / 2.0).floor();
        let panel_y = ((sh - panel_h) / 2.0).floor();

        draw_rectangle(panel_x, panel_y, panel_w, panel_h, PANEL_BG);
        draw_rectangle_lines(panel_x, panel_y, panel_w, panel_h, 2.0, PANEL_BORDER);

        // Title
        let title = if state.ui_state.stall_active {
            format!("Your Shop: {}", state.ui_state.stall_my_name)
        } else {
            "Set Up Shop".to_string()
        };
        let title_w = self.measure_text_sharp(&title, 16.0).width;
        self.draw_text_sharp(&title, (panel_x + (panel_w - title_w) / 2.0).floor(), panel_y + 20.0, 16.0, TEXT_TITLE);

        // Stall slots
        let slot_h = 30.0;
        let mut y = panel_y + 40.0;
        for (i, slot) in state.ui_state.stall_my_slots.iter().enumerate() {
            let bounds = Rect::new(panel_x + 8.0, y, panel_w - 16.0, slot_h);
            layout.add(UiElementId::StallSetupSlot(i), bounds);

            let hovered = state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallSetupSlot(i));
            draw_rectangle(bounds.x, bounds.y, bounds.w, bounds.h, if hovered { SLOT_HOVER } else { SLOT_BG });

            let item_def = state.item_registry.get_or_placeholder(&slot.item_id);
            let label = format!("{} x{} @ {}g", item_def.display_name, slot.quantity, slot.price);
            self.draw_text_sharp(&label, bounds.x + 4.0, y + 20.0, 13.0, TEXT_NORMAL);

            // Remove button
            let remove_bounds = Rect::new(bounds.x + bounds.w - 60.0, y + 3.0, 55.0, slot_h - 6.0);
            layout.add(UiElementId::StallSetupRemove(i), remove_bounds);
            let remove_hovered = state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallSetupRemove(i));
            draw_rectangle(remove_bounds.x, remove_bounds.y, remove_bounds.w, remove_bounds.h,
                if remove_hovered { BUTTON_HOVER } else { BUTTON_BG });
            self.draw_text_sharp("Remove", remove_bounds.x + 2.0, remove_bounds.y + 16.0, 12.0, TEXT_NORMAL);

            y += slot_h + 2.0;
        }

        // Empty slots hint
        if state.ui_state.stall_my_slots.len() < 10 {
            let hint = "Click inventory to add items";
            let hint_w = self.measure_text_sharp(hint, 12.0).width;
            self.draw_text_sharp(hint, (panel_x + (panel_w - hint_w) / 2.0).floor(), y + 16.0, 12.0, TEXT_DIM);
        }

        // Bottom: Open/Close button
        let btn_y = panel_y + panel_h - 40.0;
        if state.ui_state.stall_active {
            let close_bounds = Rect::new(panel_x + panel_w / 2.0 - 50.0, btn_y, 100.0, 28.0);
            layout.add(UiElementId::StallSetupCloseButton, close_bounds);
            let hovered = state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallSetupCloseButton);
            draw_rectangle(close_bounds.x, close_bounds.y, close_bounds.w, close_bounds.h,
                if hovered { BUTTON_HOVER } else { BUTTON_BG });
            self.draw_text_sharp("Close Shop", close_bounds.x + 12.0, close_bounds.y + 19.0, 14.0, Color::new(0.9, 0.3, 0.3, 1.0));
        } else {
            let open_bounds = Rect::new(panel_x + panel_w / 2.0 - 50.0, btn_y, 100.0, 28.0);
            layout.add(UiElementId::StallSetupOpenButton, open_bounds);
            let hovered = state.ui_state.hovered_element.as_ref() == Some(&UiElementId::StallSetupOpenButton);
            draw_rectangle(open_bounds.x, open_bounds.y, open_bounds.w, open_bounds.h,
                if hovered { BUTTON_HOVER } else { BUTTON_BG });
            self.draw_text_sharp("Open Shop", open_bounds.x + 12.0, open_bounds.y + 19.0, 14.0, Color::new(0.2, 0.9, 0.2, 1.0));
        }
    }
}
