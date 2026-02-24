//! Right-click context menu rendering

use super::super::Renderer;
use super::common::*;
use crate::game::{ContextMenu, ContextMenuTarget, GameState};
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

/// Helper to push an option and auto-assign its index
fn push_option<'a>(options: &mut Vec<(&'a str, UiElementId)>, label: &'a str) {
    options.push((label, UiElementId::ContextMenuOption(options.len())));
}

impl Renderer {
    /// Render the right-click context menu
    pub(crate) fn render_context_menu(
        &self,
        menu: &ContextMenu,
        state: &GameState,
        layout: &mut UiLayout,
    ) {
        let padding = 4.0;
        let option_height = 22.0;
        let font_size = 16.0;
        let h_pad = 6.0; // horizontal padding inside menu

        // Determine which options to show and header title
        let mut options: Vec<(&str, UiElementId)> = Vec::new();
        let header_title: String = match &menu.target {
            ContextMenuTarget::EquipmentSlot(_) => {
                push_option(&mut options, "Unequip");
                "Equipment".to_string()
            }
            ContextMenuTarget::Gold => {
                push_option(&mut options, "Drop");
                "Gold".to_string()
            }
            ContextMenuTarget::InventorySlot(slot_index) => {
                if let Some(slot) = state
                    .inventory
                    .slots
                    .get(*slot_index)
                    .and_then(|s| s.as_ref())
                {
                    let item_def = state.item_registry.get_or_placeholder(&slot.item_id);
                    if item_def.equipment.is_some() {
                        push_option(&mut options, "Equip");
                    }
                    if slot.item_id.contains("bones") {
                        push_option(&mut options, "Bury");
                    }
                }
                push_option(&mut options, "Drop");
                "Item".to_string()
            }
            ContextMenuTarget::Player { id } => {
                let name = state
                    .players
                    .get(id)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "Player".to_string());
                push_option(&mut options, "Attack");
                push_option(&mut options, "Follow");
                push_option(&mut options, "Add Friend");
                push_option(&mut options, "Examine");
                name
            }
            ContextMenuTarget::Npc { id } => {
                if let Some(npc) = state.npcs.get(id) {
                    let name = npc.name();
                    if npc.is_attackable() {
                        push_option(&mut options, "Attack");
                        push_option(&mut options, "Target");
                        push_option(&mut options, "Examine");
                    } else {
                        if npc.is_altar {
                            push_option(&mut options, "Pray");
                            push_option(&mut options, "Offer Bones");
                            push_option(&mut options, "Examine");
                        } else if npc.station_type.is_some() {
                            push_option(&mut options, "Use");
                            push_option(&mut options, "Examine");
                        } else if npc.is_merchant {
                            push_option(&mut options, "Talk-to");
                            push_option(&mut options, "Trade");
                            push_option(&mut options, "Examine");
                        } else if npc.is_banker {
                            push_option(&mut options, "Bank");
                            push_option(&mut options, "Examine");
                        } else if npc.is_slayer_master {
                            push_option(&mut options, "Get Task");
                            push_option(&mut options, "Examine");
                        } else {
                            push_option(&mut options, "Talk-to");
                            push_option(&mut options, "Examine");
                        }
                    }
                    name
                } else {
                    push_option(&mut options, "Examine");
                    "NPC".to_string()
                }
            }
            ContextMenuTarget::Tree { gid, .. } => {
                let name = crate::game::tree_types::get_tree_info(*gid)
                    .map(|info| format!("{} Tree", info.name))
                    .unwrap_or_else(|| "Tree".to_string());
                push_option(&mut options, "Chop");
                push_option(&mut options, "Examine");
                name
            }
            ContextMenuTarget::Rock { gid, .. } => {
                let name = crate::game::ore_types::get_ore_info(*gid)
                    .map(|info| format!("{} Rock", info.name))
                    .unwrap_or_else(|| "Rock".to_string());
                push_option(&mut options, "Mine");
                push_option(&mut options, "Examine");
                name
            }
            ContextMenuTarget::GatheringSpot { marker_index } => {
                let name = state
                    .gathering_markers
                    .get(*marker_index)
                    .map(|m| {
                        let action = if m.skill == "fishing" { "Fish" } else { "Gather" };
                        push_option(&mut options, action);
                        m.zone_id.clone()
                    })
                    .unwrap_or_else(|| "Gathering Spot".to_string());
                push_option(&mut options, "Examine");
                name
            }
            ContextMenuTarget::GroundItem { id } => {
                let name = state
                    .ground_items
                    .get(id)
                    .map(|item| {
                        let item_def = state.item_registry.get_or_placeholder(&item.item_id);
                        if item.quantity > 1 {
                            format!("{} ({})", item_def.display_name, item.quantity)
                        } else {
                            item_def.display_name.clone()
                        }
                    })
                    .unwrap_or_else(|| "Item".to_string());
                push_option(&mut options, "Pick up");
                push_option(&mut options, "Examine");
                name
            }
            ContextMenuTarget::FarmingPatch { patch_id } => {
                if let Some(patch) = state.farming_patches.get(patch_id) {
                    if patch.state == "harvestable" {
                        push_option(&mut options, "Harvest");
                    } else if patch.state == "empty" {
                        push_option(&mut options, "Plant");
                    }
                }
                push_option(&mut options, "Examine");
                "Farming Patch".to_string()
            }
            ContextMenuTarget::Tile { .. } => {
                push_option(&mut options, "Walk here");
                "Tile".to_string()
            }
            ContextMenuTarget::HotkeySlot(_) => {
                push_option(&mut options, "Clear Slot");
                "Hotkey Slot".to_string()
            }
            ContextMenuTarget::Spell { spell_id } => {
                // Spell context menu — title only, assign buttons rendered separately
                let name = crate::game::spell::SPELLS
                    .iter()
                    .find(|s| s.id == spell_id)
                    .map(|s| s.name.to_string())
                    .unwrap_or_else(|| "Spell".to_string());
                name
            }
        };

        // For contexts that support hotkey assignment, we'll render inline buttons
        let show_hotkey_assign = matches!(
            &menu.target,
            ContextMenuTarget::InventorySlot(_) | ContextMenuTarget::Spell { .. }
        );

        if options.is_empty() && !show_hotkey_assign {
            return;
        }

        // Measure widest option to size the menu
        let mut max_text_w: f32 = self.measure_text_sharp(&header_title, font_size).width;
        for (label, _) in &options {
            let w = self.measure_text_sharp(label, font_size).width;
            if w > max_text_w {
                max_text_w = w;
            }
        }
        // Hotkey assign row needs space for "Hotkey: [1][2][3][4][5]"
        let hotkey_row_width = if show_hotkey_assign { 160.0 } else { 0.0 };
        let menu_width = (max_text_w + h_pad * 2.0 + 8.0).max(80.0).max(hotkey_row_width).floor();

        let header_height = option_height; // header is same height as an option row
        let hotkey_row_height = if show_hotkey_assign { option_height + 2.0 } else { 0.0 };
        let menu_height = header_height + options.len() as f32 * option_height + hotkey_row_height + padding;

        // Position at cursor, clamped to screen
        let (sw, sh) = virtual_screen_size();
        let mut menu_x = menu.x.floor();
        let mut menu_y = menu.y.floor();
        if menu_x + menu_width > sw {
            menu_x = (sw - menu_width - 2.0).floor();
        }
        if menu_y + menu_height > sh {
            menu_y = (sh - menu_height - 2.0).floor();
        }

        // Background — single dark rect with thin 1px border
        let bg = Color::new(0.08, 0.08, 0.12, 0.95);
        let border = Color::new(0.25, 0.25, 0.30, 0.8);
        draw_rectangle(menu_x - 1.0, menu_y - 1.0, menu_width + 2.0, menu_height + 2.0, border);
        draw_rectangle(menu_x, menu_y, menu_width, menu_height, bg);

        // Header — just the title text, slightly dimmer
        let header_color = Color::new(0.75, 0.72, 0.65, 1.0);
        self.draw_text_sharp(
            &header_title,
            (menu_x + h_pad).floor(),
            (menu_y + font_size).floor(),
            font_size,
            header_color,
        );

        // Thin separator line under header
        let sep_y = (menu_y + header_height).floor();
        draw_line(menu_x + 2.0, sep_y, menu_x + menu_width - 2.0, sep_y, 1.0, border);

        // Options
        let (mouse_x, mouse_y) = mouse_position();
        let mut y = sep_y + padding * 0.5;

        for (_i, (label, element_id)) in options.iter().enumerate() {
            let option_bounds = Rect::new(menu_x, y, menu_width, option_height);
            layout.add(element_id.clone(), option_bounds);

            let is_hovered = mouse_x >= option_bounds.x
                && mouse_x <= option_bounds.x + option_bounds.w
                && mouse_y >= option_bounds.y
                && mouse_y <= option_bounds.y + option_bounds.h;

            if is_hovered {
                let hover_bg = Color::new(0.20, 0.20, 0.28, 0.9);
                draw_rectangle(option_bounds.x, option_bounds.y, option_bounds.w, option_bounds.h, hover_bg);
            }

            let text_color = if is_hovered {
                TEXT_TITLE
            } else {
                TEXT_NORMAL
            };
            self.draw_text_sharp(
                label,
                (menu_x + h_pad).floor(),
                (y + font_size).floor(),
                font_size,
                text_color,
            );

            y += option_height;
        }

        // Render inline hotkey assign buttons: "Hotkey: [1][2][3][4][5]"
        if show_hotkey_assign {
            let (mouse_x, mouse_y) = mouse_position();
            let assign_y = y + 2.0;
            let label = "Hotkey:";
            let label_color = Color::new(0.55, 0.55, 0.60, 1.0);
            self.draw_text_sharp(
                label,
                (menu_x + h_pad).floor(),
                (assign_y + font_size - 2.0).floor(),
                font_size,
                label_color,
            );
            let label_w = self.measure_text_sharp(label, font_size).width;
            let btn_size = 20.0;
            let btn_gap = 3.0;
            let mut bx = menu_x + h_pad + label_w + 6.0;
            for i in 0..5 {
                let btn_rect = Rect::new(bx, assign_y, btn_size, btn_size);
                layout.add(UiElementId::HotkeyAssignButton(i), btn_rect);

                let is_btn_hovered = mouse_x >= btn_rect.x
                    && mouse_x <= btn_rect.x + btn_rect.w
                    && mouse_y >= btn_rect.y
                    && mouse_y <= btn_rect.y + btn_rect.h;

                let btn_bg = if is_btn_hovered {
                    SLOT_HOVER_BG
                } else {
                    SLOT_BG_FILLED
                };
                let btn_border = if is_btn_hovered {
                    SLOT_HOVER_BORDER
                } else {
                    SLOT_BORDER
                };
                draw_rectangle(bx, assign_y, btn_size, btn_size, btn_bg);
                draw_rectangle_lines(bx, assign_y, btn_size, btn_size, 1.0, btn_border);

                let num_text = (i + 1).to_string();
                let num_w = self.measure_text_sharp(&num_text, font_size).width;
                self.draw_text_sharp(
                    &num_text,
                    (bx + (btn_size - num_w) / 2.0).floor(),
                    (assign_y + font_size - 2.0).floor(),
                    font_size,
                    if is_btn_hovered { TEXT_TITLE } else { TEXT_NORMAL },
                );
                bx += btn_size + btn_gap;
            }
        }
    }
}
