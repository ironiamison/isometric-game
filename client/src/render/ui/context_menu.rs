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
            ContextMenuTarget::MapObject { gid, .. } => {
                let name = crate::input::handler::get_map_object_name(*gid);
                if crate::input::handler::is_obelisk_gid(*gid) {
                    push_option(&mut options, "Teleport");
                } else {
                    push_option(&mut options, "Interact");
                }
                push_option(&mut options, "Examine");
                name.unwrap_or("Object").to_string()
            }
            ContextMenuTarget::Tile { .. } => {
                push_option(&mut options, "Walk here");
                "Tile".to_string()
            }
            ContextMenuTarget::HotkeySlot(_) => {
                push_option(&mut options, "Clear Slot");
                "Hotkey Slot".to_string()
            }
        };

        if options.is_empty() {
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
        let menu_width = (max_text_w + h_pad * 2.0 + 8.0).max(80.0).floor();

        let header_height = option_height;
        let menu_height = header_height + options.len() as f32 * option_height + padding;

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

    }
}
