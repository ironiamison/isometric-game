//! Quick slots bar rendering - toggleable between items and spells

use macroquad::prelude::*;
use crate::game::GameState;
use crate::game::spell::SPELLS;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use super::super::Renderer;
use super::super::isometric::world_to_screen;
use super::common::*;

impl Renderer {
    pub(crate) fn render_quick_slots(&self, state: &GameState, hovered: &Option<UiElementId>, layout: &mut UiLayout) {
        if state.ui_state.spell_bar_active {
            self.render_spell_bar(state, hovered, layout);
        } else {
            self.render_item_bar(state, hovered, layout);
        }
        // Render the toggle button
        self.render_bar_toggle_button(state, hovered, layout);
    }

    /// Render item bar: slots 0-4 of inventory directly
    fn render_item_bar(&self, state: &GameState, hovered: &Option<UiElementId>, layout: &mut UiLayout) {
        let scale = state.ui_state.ui_scale;
        let slot_size = (QUICK_SLOT_SIZE * scale).max(MIN_SLOT_SIZE);
        let spacing = QUICK_SLOT_SPACING * scale;
        let total_width = 5.0 * slot_size + 4.0 * spacing;

        let (sw, sh) = virtual_screen_size();
        let start_x = (sw - total_width) / 2.0;
        let start_y = sh - EXP_BAR_GAP * scale - slot_size;

        for i in 0..5 {
            let x = start_x + i as f32 * (slot_size + spacing);
            let y = start_y;

            // Register slot bounds for hit detection
            let bounds = Rect::new(x, y, slot_size, slot_size);
            layout.add(UiElementId::QuickSlot(i), bounds);

            let is_hovered = matches!(hovered, Some(UiElementId::QuickSlot(idx)) if *idx == i);

            // Check if this inventory slot is being dragged
            let is_dragging = matches!(
                &state.ui_state.drag_state,
                Some(drag) if matches!(&drag.source, crate::game::DragSource::Inventory(idx) if *idx == i)
            );

            let slot_state = if is_dragging {
                SlotState::Dragging
            } else if is_hovered {
                SlotState::Hovered
            } else {
                SlotState::Normal
            };

            let has_item = state.inventory.slots.get(i).map(|s| s.is_some()).unwrap_or(false);
            self.draw_inventory_slot(x, y, slot_size, has_item, slot_state);

            // Draw item if present (hide if being dragged)
            if let Some(Some(slot)) = state.inventory.slots.get(i) {
                if !is_dragging {
                    self.draw_item_icon(&slot.item_id, x, y, slot_size, slot_size, state, false);

                    // Quantity badge (bottom-left with shadow)
                    if slot.quantity > 1 {
                        let qty_text = slot.quantity.to_string();
                        self.draw_text_sharp(&qty_text, x + 3.0 * scale, y + slot_size - 4.0, 16.0, Color::new(0.0, 0.0, 0.0, 0.8));
                        self.draw_text_sharp(&qty_text, x + 2.0 * scale, y + slot_size - 5.0, 16.0, TEXT_NORMAL);
                    }
                }
            }

            // Shift-drop indicator overlay
            let shift_held = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
            if shift_held && state.ui_state.shift_drop_enabled && has_item && is_hovered {
                draw_rectangle(x + 2.0, y + 2.0, slot_size - 4.0, slot_size - 4.0, Color::new(0.8, 0.2, 0.2, 0.35));
                draw_rectangle_lines(x + 1.0, y + 1.0, slot_size - 2.0, slot_size - 2.0, 2.0, Color::new(0.9, 0.3, 0.3, 0.9));
            }

            // Slot number badge (top-right)
            let num_text = (i + 1).to_string();
            let text_w = self.measure_text_sharp(&num_text, 16.0).width;
            let badge_w = text_w + 2.0;
            let badge_h = 13.0;
            let num_x = x + slot_size - badge_w - 1.0;
            let num_y = y + 1.0;
            draw_rectangle(num_x, num_y, badge_w, badge_h, Color::new(0.0, 0.0, 0.0, 0.5));
            self.draw_text_sharp(&num_text, num_x + 1.0, num_y + 11.0, 16.0, TEXT_NORMAL);
        }
    }

    /// Render spell bar: unlocked spells in up to 5 slots
    fn render_spell_bar(&self, state: &GameState, hovered: &Option<UiElementId>, layout: &mut UiLayout) {
        let scale = state.ui_state.ui_scale;
        let slot_size = (QUICK_SLOT_SIZE * scale).max(MIN_SLOT_SIZE);
        let spacing = QUICK_SLOT_SPACING * scale;
        let total_width = 5.0 * slot_size + 4.0 * spacing;

        let (sw, sh) = virtual_screen_size();
        let start_x = (sw - total_width) / 2.0;
        let start_y = sh - EXP_BAR_GAP * scale - slot_size;

        // Get player magic level
        let magic_level = state.get_local_player()
            .map(|p| p.skills.magic.level)
            .unwrap_or(1);

        // Collect unlocked spells
        let unlocked_spells: Vec<_> = SPELLS.iter()
            .filter(|s| magic_level >= s.magic_level_req)
            .collect();

        let now = macroquad::time::get_time();
        let player_mp = state.get_local_player().map(|p| p.mp).unwrap_or(0);

        for i in 0..5 {
            let x = start_x + i as f32 * (slot_size + spacing);
            let y = start_y;

            // Register slot bounds for hit detection
            let bounds = Rect::new(x, y, slot_size, slot_size);
            layout.add(UiElementId::QuickSlot(i), bounds);

            let is_hovered = matches!(hovered, Some(UiElementId::QuickSlot(idx)) if *idx == i);

            let slot_state = if is_hovered {
                SlotState::Hovered
            } else {
                SlotState::Normal
            };

            if let Some(spell_def) = unlocked_spells.get(i) {
                // Draw slot background (has content)
                self.draw_inventory_slot(x, y, slot_size, true, slot_state);

                // Try to draw spell icon texture, fallback to colored rect with letter
                let icon_key = format!("spell_{}", spell_def.effect_sprite);
                let has_icon = self.item_sprites.get(&icon_key).is_some();

                if has_icon {
                    self.draw_item_icon(&icon_key, x, y, slot_size, slot_size, state, false);
                } else {
                    // Fallback: colored rectangle with spell's first letter
                    let color = match spell_def.spell_type {
                        crate::game::spell::SpellType::Damage => Color::new(0.6, 0.15, 0.15, 0.9),
                        crate::game::spell::SpellType::Heal => Color::new(0.15, 0.5, 0.15, 0.9),
                    };
                    let pad = 4.0;
                    draw_rectangle(x + pad, y + pad, slot_size - pad * 2.0, slot_size - pad * 2.0, color);

                    // Draw spell first letter centered
                    let letter = &spell_def.name[..1];
                    let letter_size = 22.0;
                    let letter_w = self.measure_text_sharp(letter, letter_size).width;
                    self.draw_text_sharp(
                        letter,
                        x + (slot_size - letter_w) / 2.0,
                        y + (slot_size + letter_size * 0.6) / 2.0,
                        letter_size,
                        WHITE,
                    );
                }

                // Mana cost badge (bottom-left with shadow)
                let mana_text = spell_def.mana_cost.to_string();
                self.draw_text_sharp(&mana_text, x + 3.0 * scale, y + slot_size - 4.0, 16.0, Color::new(0.0, 0.0, 0.0, 0.8));
                self.draw_text_sharp(&mana_text, x + 2.0 * scale, y + slot_size - 5.0, 16.0, Color::new(0.4, 0.6, 1.0, 1.0));

                // Check cooldown
                let on_cooldown = state.spell_cooldowns.get(spell_def.id).map_or(false, |&t| now < t);
                let insufficient_mana = player_mp < spell_def.mana_cost;

                if on_cooldown {
                    // Dark semi-transparent overlay for cooldown
                    draw_rectangle(x + 2.0, y + 2.0, slot_size - 4.0, slot_size - 4.0, Color::new(0.0, 0.0, 0.0, 0.55));

                    // Show remaining cooldown time
                    let remaining = state.spell_cooldowns.get(spell_def.id).map_or(0.0, |&t| (t - now).max(0.0));
                    let cd_text = format!("{:.1}", remaining);
                    let cd_w = self.measure_text_sharp(&cd_text, 14.0).width;
                    self.draw_text_sharp(
                        &cd_text,
                        x + (slot_size - cd_w) / 2.0,
                        y + slot_size / 2.0 + 4.0,
                        14.0,
                        WHITE,
                    );
                } else if insufficient_mana {
                    // Red-tinted overlay for insufficient mana
                    draw_rectangle(x + 2.0, y + 2.0, slot_size - 4.0, slot_size - 4.0, Color::new(0.6, 0.1, 0.1, 0.45));
                }
            } else {
                // Empty spell slot (no unlocked spell at this index)
                self.draw_inventory_slot(x, y, slot_size, false, slot_state);
            }

            // Slot number badge (top-right)
            let num_text = (i + 1).to_string();
            let text_w = self.measure_text_sharp(&num_text, 16.0).width;
            let badge_w = text_w + 2.0;
            let badge_h = 13.0;
            let num_x = x + slot_size - badge_w - 1.0;
            let num_y = y + 1.0;
            draw_rectangle(num_x, num_y, badge_w, badge_h, Color::new(0.0, 0.0, 0.0, 0.5));
            self.draw_text_sharp(&num_text, num_x + 1.0, num_y + 11.0, 16.0, TEXT_NORMAL);
        }
    }

    /// Render the toggle button between items and spells bar
    fn render_bar_toggle_button(&self, state: &GameState, hovered: &Option<UiElementId>, layout: &mut UiLayout) {
        let scale = state.ui_state.ui_scale;
        let slot_size = (QUICK_SLOT_SIZE * scale).max(MIN_SLOT_SIZE);
        let spacing = QUICK_SLOT_SPACING * scale;
        let total_width = 5.0 * slot_size + 4.0 * spacing;

        let (sw, sh) = virtual_screen_size();
        let start_x = (sw - total_width) / 2.0;
        let start_y = sh - EXP_BAR_GAP * scale - slot_size;

        // Position the toggle button to the left of the quick slots bar
        let btn_w = (40.0 * scale).max(34.0);
        let btn_h = slot_size;
        let btn_x = start_x - btn_w - spacing;
        let btn_y = start_y;

        let bounds = Rect::new(btn_x, btn_y, btn_w, btn_h);
        layout.add(UiElementId::SpellBarToggle, bounds);

        let is_hovered = matches!(hovered, Some(UiElementId::SpellBarToggle));

        // Button background
        let bg_color = if is_hovered {
            SLOT_HOVER_BG
        } else {
            SLOT_BG_FILLED
        };
        let border_color = if is_hovered {
            SLOT_HOVER_BORDER
        } else {
            SLOT_BORDER
        };

        // Draw beveled slot background
        draw_rectangle(btn_x, btn_y, btn_w, btn_h, bg_color);
        draw_rectangle_lines(btn_x, btn_y, btn_w, btn_h, 1.0, border_color);

        // Draw icon/label based on current mode
        let label = if state.ui_state.spell_bar_active {
            "Sp"
        } else {
            "It"
        };
        let label_color = if state.ui_state.spell_bar_active {
            Color::new(0.5, 0.4, 0.9, 1.0) // Purple for spells
        } else {
            Color::new(0.7, 0.6, 0.4, 1.0) // Gold/brown for items
        };

        let font_size = 16.0;
        let text_w = self.measure_text_sharp(label, font_size).width;
        self.draw_text_sharp(
            label,
            btn_x + (btn_w - text_w) / 2.0,
            btn_y + (btn_h + font_size * 0.6) / 2.0,
            font_size,
            label_color,
        );

        // Tooltip on hover
        if is_hovered {
            let tooltip_text = if state.ui_state.spell_bar_active {
                "Spell Bar (click to switch to Items)"
            } else {
                "Item Bar (click to switch to Spells)"
            };
            let padding = 6.0;
            let tip_dims = self.measure_text_sharp(tooltip_text, 16.0);
            let tip_w = (tip_dims.width + padding * 2.0).floor();
            let tip_h = (18.0 + padding * 2.0).floor();
            let tip_x = (btn_x + btn_w / 2.0 - tip_w / 2.0).floor();
            let tip_y = (btn_y - tip_h - 4.0).floor();

            draw_rectangle(tip_x - 1.0, tip_y - 1.0, tip_w + 2.0, tip_h + 2.0, SLOT_BORDER);
            draw_rectangle(tip_x, tip_y, tip_w, tip_h, SLOT_BG_FILLED);
            self.draw_text_sharp(
                tooltip_text,
                (tip_x + padding).floor(),
                (tip_y + padding + 14.0).floor(),
                16.0,
                TEXT_NORMAL,
            );
        }
    }

    pub(crate) fn render_ground_item_overlays(&self, state: &GameState, hovered: &Option<UiElementId>, layout: &mut UiLayout) {
        let zoom = state.camera.zoom;

        for (item_id, item) in &state.ground_items {
            let (screen_x, screen_y) = world_to_screen(item.x, item.y, &state.camera);

            // Clickable area - centered on where items actually render (slightly above tile center)
            let click_width = 44.0 * zoom;
            let click_height = 28.0 * zoom;
            let bounds = Rect::new(
                screen_x - click_width / 2.0,
                screen_y - click_height,
                click_width,
                click_height,
            );
            layout.add(UiElementId::GroundItem(item_id.clone()), bounds);

            // Check if hovered
            let is_hovered = matches!(hovered, Some(UiElementId::GroundItem(id)) if id == item_id);

            if is_hovered {
                // Draw tile hover effect
                self.render_tile_hover(item.x as i32, item.y as i32, &state.camera);

                // Get item definition for display name
                let item_def = state.item_registry.get_or_placeholder(&item.item_id);

                // Build label text
                let label = if item.quantity > 1 {
                    format!("{} (x{})", item_def.display_name, item.quantity)
                } else {
                    item_def.display_name.clone()
                };

                // Draw label just above the clickable area
                let font_size = 16.0 * zoom;
                let label_width = self.measure_text_sharp(&label, font_size).width;
                let label_x = screen_x - label_width / 2.0;
                // Gold piles sit lower, so offset label down by 12px
                let gold_offset = if item.item_id == "gold" { 22.0 * zoom } else { 0.0 };
                let label_y = screen_y - click_height - 16.0 * zoom + gold_offset;

                // Background for readability
                let padding = 4.0 * zoom;
                draw_rectangle(
                    label_x - padding,
                    label_y - 14.0 * zoom,
                    label_width + padding * 2.0,
                    18.0 * zoom,
                    Color::from_rgba(0, 0, 0, 180),
                );

                // Label text
                self.draw_text_sharp(&label, label_x, label_y, font_size, WHITE);
            }
        }
    }
}
