//! Item tooltip rendering

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::ui::UiElementId;
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

impl Renderer {
    /// Render tooltip for hovered inventory/quick slot items
    pub(crate) fn render_item_tooltip(&self, state: &GameState) {
        // Check if we're hovering over an inventory, quick slot, or equipment slot
        let (item_id, quantity) = match &state.ui_state.hovered_element {
            Some(UiElementId::InventorySlot(idx)) if state.ui_state.inventory_open => {
                if let Some(slot) = state.inventory.slots.get(*idx).and_then(|s| s.as_ref()) {
                    (slot.item_id.clone(), slot.quantity)
                } else {
                    return;
                }
            }
            Some(UiElementId::QuickSlot(idx)) => {
                if state.ui_state.spell_bar_active {
                    // Show spell tooltip
                    let magic_level = state
                        .get_local_player()
                        .map(|p| p.skills.magic.level)
                        .unwrap_or(1);
                    let unlocked: Vec<_> = crate::game::spell::SPELLS
                        .iter()
                        .filter(|s| magic_level >= s.magic_level_req)
                        .collect();
                    if let Some(spell_def) = unlocked.get(*idx) {
                        self.render_quick_slot_spell_tooltip(spell_def, state);
                    }
                    return;
                }
                // Item mode: tooltip from inventory slot
                if let Some(slot) = state.inventory.slots.get(*idx).and_then(|s| s.as_ref()) {
                    (slot.item_id.clone(), slot.quantity)
                } else {
                    return;
                }
            }
            Some(UiElementId::EquipmentSlot(slot_type)) if state.ui_state.inventory_open => {
                let equipped_item =
                    state
                        .get_local_player()
                        .and_then(|p| match slot_type.as_str() {
                            "head" => p.equipped_head.clone(),
                            "body" => p.equipped_body.clone(),
                            "weapon" => p.equipped_weapon.clone(),
                            "back" => p.equipped_back.clone(),
                            "feet" => p.equipped_feet.clone(),
                            "ring" => p.equipped_ring.clone(),
                            "gloves" => p.equipped_gloves.clone(),
                            "necklace" => p.equipped_necklace.clone(),
                            "belt" => p.equipped_belt.clone(),
                            _ => None,
                        });
                if let Some(id) = equipped_item {
                    (id, 1) // Equipment always has quantity 1
                } else {
                    return;
                }
            }
            Some(UiElementId::FurnaceRecipeItem(idx)) if state.ui_state.furnace_open => {
                // Only show tooltip when hovering the left side (icon + recipe info),
                // not the right side where quantity buttons and smelt button live
                let (sw, _) = virtual_screen_size();
                let panel_width = (500.0_f32).min(sw - 16.0);
                let panel_x = (sw - panel_width) / 2.0;
                let right_controls_x = panel_x + panel_width - FRAME_THICKNESS - 8.0 - 170.0;
                let (mouse_x, _) = mouse_position();
                let scale_x = sw / screen_width();
                let virtual_mouse_x = mouse_x * scale_x;
                if virtual_mouse_x >= right_controls_x {
                    return;
                }

                let section_filter = if state.ui_state.furnace_tab == 0 { "materials" } else { "jewelry" };
                let mut furnace_recipes: Vec<_> = state
                    .recipe_definitions
                    .iter()
                    .filter(|r| r.station.as_deref() == Some("furnace"))
                    .filter(|r| r.section.as_deref() == Some(section_filter))
                    .filter(|r| !r.requires_discovery || state.discovered_recipes.contains(&r.id))
                    .collect();
                furnace_recipes.sort_by_key(|r| r.level_required);
                if let Some(recipe) = furnace_recipes.get(*idx) {
                    if let Some(result) = recipe.results.first() {
                        (result.item_id.clone(), result.count)
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            }
            Some(UiElementId::AnvilRecipeCell(idx)) if state.ui_state.anvil_open => {
                let section_filter = if state.ui_state.anvil_tab == 0 { "materials" } else { "equipment" };
                let mut anvil_recipes: Vec<_> = state
                    .recipe_definitions
                    .iter()
                    .filter(|r| r.station.as_deref() == Some("anvil"))
                    .filter(|r| r.section.as_deref() == Some(section_filter))
                    .filter(|r| !r.requires_discovery || state.discovered_recipes.contains(&r.id))
                    .collect();
                anvil_recipes.sort_by_key(|r| r.level_required);
                if let Some(recipe) = anvil_recipes.get(*idx) {
                    if let Some(result) = recipe.results.first() {
                        (result.item_id.clone(), result.count)
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            }
            _ => return,
        };

        // Get item definition from registry
        let item_def = state.item_registry.get_or_placeholder(&item_id);

        // Get player skill levels for requirement checking
        let player_combat_level = state
            .get_local_player()
            .map(|p| p.skills.combat.level)
            .unwrap_or(1);
        let player_woodcutting_level = state
            .get_local_player()
            .map(|p| p.skills.woodcutting.level)
            .unwrap_or(1);
        let player_mining_level = state
            .get_local_player()
            .map(|p| p.skills.mining.level)
            .unwrap_or(1);

        // Get mouse position for tooltip placement
        let (mouse_x, mouse_y) = mouse_position();

        // Enhanced tooltip styling - use 16pt throughout for crisp rendering
        let padding = 10.0;
        let line_height = 20.0;
        let font_size = 16.0;
        let small_font_size = 16.0; // Use 16pt for all text (native size)
        let max_tooltip_width = 200.0;
        let text_width_limit = max_tooltip_width - padding * 2.0;

        // Prepare text strings for measurement
        let name_text = if quantity > 1 {
            format!("{} x{}", item_def.display_name, quantity)
        } else {
            item_def.display_name.clone()
        };

        // Word-wrap the description
        let desc_lines = if !item_def.description.is_empty() {
            self.wrap_text(&item_def.description, text_width_limit, small_font_size)
        } else {
            vec![]
        };

        // Calculate tooltip width based on longest line
        let mut max_w = self.measure_text_sharp(&name_text, font_size).width;
        for line in &desc_lines {
            max_w = max_w.max(self.measure_text_sharp(line, small_font_size).width);
        }

        if let Some(ref equip) = item_def.equipment {
            if equip.attack_bonus != 0 {
                let attack_text = if equip.attack_bonus > 0 {
                    format!("+{} Attack", equip.attack_bonus)
                } else {
                    format!("{} Attack", equip.attack_bonus)
                };
                max_w = max_w.max(self.measure_text_sharp(&attack_text, small_font_size).width);
            }
            if equip.strength_bonus != 0 {
                let strength_text = if equip.strength_bonus > 0 {
                    format!("+{} Strength", equip.strength_bonus)
                } else {
                    format!("{} Strength", equip.strength_bonus)
                };
                max_w = max_w.max(
                    self.measure_text_sharp(&strength_text, small_font_size)
                        .width,
                );
            }
            if equip.defence_bonus != 0 {
                let defence_text = if equip.defence_bonus > 0 {
                    format!("+{} Defence", equip.defence_bonus)
                } else {
                    format!("{} Defence", equip.defence_bonus)
                };
                max_w = max_w.max(
                    self.measure_text_sharp(&defence_text, small_font_size)
                        .width,
                );
            }
            // Measure requirement text
            let is_weapon = equip.slot_type == "weapon";
            let req_text = if is_weapon && equip.attack_level_required > 1 {
                format!("Requires {} Attack", equip.attack_level_required)
            } else if !is_weapon && equip.defence_level_required > 1 {
                format!("Requires {} Defence", equip.defence_level_required)
            } else {
                String::new()
            };
            if !req_text.is_empty() {
                max_w = max_w.max(self.measure_text_sharp(&req_text, small_font_size).width);
            }
            if equip.woodcutting_level_required > 1 {
                let wc_req_text = format!("Requires {} Woodcutting", equip.woodcutting_level_required);
                max_w = max_w.max(self.measure_text_sharp(&wc_req_text, small_font_size).width);
            }
            if equip.mining_level_required > 1 {
                let mining_req_text = format!("Requires {} Mining", equip.mining_level_required);
                max_w = max_w.max(self.measure_text_sharp(&mining_req_text, small_font_size).width);
            }
        }

        let tooltip_width = (max_w + padding * 2.0).ceil().min(max_tooltip_width);

        // Calculate tooltip height based on actual lines drawn
        let mut total_h = padding * 2.0;
        total_h += line_height; // Name
        total_h += line_height; // Category badge

        let has_description = !desc_lines.is_empty();
        let has_equipment = item_def.equipment.is_some();

        if has_description {
            total_h += 2.0; // Small gap
            total_h += desc_lines.len() as f32 * line_height;
        }
        if has_equipment {
            total_h += 2.0; // Small gap
            if let Some(ref equip) = item_def.equipment {
                if equip.attack_bonus != 0 {
                    total_h += line_height;
                }
                if equip.strength_bonus != 0 {
                    total_h += line_height;
                }
                if equip.defence_bonus != 0 {
                    total_h += line_height;
                }
                // Level requirement lines (only if > 1)
                let is_weapon = equip.slot_type == "weapon";
                if (is_weapon && equip.attack_level_required > 1)
                    || (!is_weapon && equip.defence_level_required > 1)
                {
                    total_h += line_height;
                }
                if equip.woodcutting_level_required > 1 {
                    total_h += line_height;
                }
                if equip.mining_level_required > 1 {
                    total_h += line_height;
                }
            }
        }

        let tooltip_height = total_h.ceil();

        // Position tooltip near cursor, but keep on screen
        let (sw, sh) = virtual_screen_size();
        let mut tooltip_x = (mouse_x + 16.0).floor();
        let mut tooltip_y = (mouse_y + 16.0).floor();

        // Clamp to screen bounds
        if tooltip_x + tooltip_width > sw {
            tooltip_x = (mouse_x - tooltip_width - 8.0).floor();
        }
        if tooltip_y + tooltip_height > sh {
            tooltip_y = (mouse_y - tooltip_height - 8.0).floor();
        }

        // Draw tooltip frame (3-layer)
        // Shadow
        draw_rectangle(
            tooltip_x + 2.0,
            tooltip_y + 2.0,
            tooltip_width,
            tooltip_height,
            Color::new(0.0, 0.0, 0.0, 0.4),
        );
        // Frame
        draw_rectangle(
            tooltip_x - 1.0,
            tooltip_y - 1.0,
            tooltip_width + 2.0,
            tooltip_height + 2.0,
            TOOLTIP_FRAME,
        );
        // Background
        draw_rectangle(
            tooltip_x,
            tooltip_y,
            tooltip_width,
            tooltip_height,
            TOOLTIP_BG,
        );

        // Inner highlight (top edge)
        draw_line(
            tooltip_x + 1.0,
            tooltip_y + 1.0,
            tooltip_x + tooltip_width - 1.0,
            tooltip_y + 1.0,
            1.0,
            Color::new(0.227, 0.227, 0.267, 1.0),
        );

        let mut y = tooltip_y + padding + 12.0;

        // Item name (white, bold-ish)
        self.draw_text_sharp(&name_text, tooltip_x + padding, y, font_size, TEXT_NORMAL);
        y += line_height;

        // Category badge
        let category_color = self.get_category_color(&item_def.category);
        let category_text = item_def.category.to_uppercase();
        let badge_w = self.measure_text_sharp(&category_text, 16.0).width + 10.0;
        let badge_h = 20.0;
        let badge_x = tooltip_x + padding;
        let badge_y = y - 14.0;

        // Badge background (tinted)
        let badge_bg = Color::new(category_color.r, category_color.g, category_color.b, 0.2);
        draw_rectangle(badge_x, badge_y, badge_w, badge_h, badge_bg);
        draw_rectangle_lines(badge_x, badge_y, badge_w, badge_h, 1.0, category_color);
        self.draw_text_sharp(&category_text, badge_x + 5.0, y, 16.0, category_color);
        y += line_height;

        // Description section (if any)
        if has_description {
            y += 2.0;
            // Description text
            for line in &desc_lines {
                self.draw_text_sharp(line, tooltip_x + padding, y, small_font_size, TEXT_DIM);
                y += line_height;
            }
        }

        // Equipment stats section
        if let Some(ref equip) = item_def.equipment {
            y += 2.0;
            // Stat colors
            let stat_green = Color::new(0.392, 0.784, 0.392, 1.0); // rgba(100, 200, 100)
            let stat_red = Color::new(1.0, 0.392, 0.392, 1.0); // rgba(255, 100, 100)

            // Attack bonus
            if equip.attack_bonus != 0 {
                let attack_text = if equip.attack_bonus > 0 {
                    format!("+{} Attack", equip.attack_bonus)
                } else {
                    format!("{} Attack", equip.attack_bonus)
                };
                let attack_color = if equip.attack_bonus > 0 {
                    stat_green
                } else {
                    stat_red
                };
                self.draw_text_sharp(
                    &attack_text,
                    tooltip_x + padding,
                    y,
                    small_font_size,
                    attack_color,
                );
                y += line_height;
            }

            // Strength bonus
            if equip.strength_bonus != 0 {
                let strength_text = if equip.strength_bonus > 0 {
                    format!("+{} Strength", equip.strength_bonus)
                } else {
                    format!("{} Strength", equip.strength_bonus)
                };
                let strength_color = if equip.strength_bonus > 0 {
                    stat_green
                } else {
                    stat_red
                };
                self.draw_text_sharp(
                    &strength_text,
                    tooltip_x + padding,
                    y,
                    small_font_size,
                    strength_color,
                );
                y += line_height;
            }

            // Defence bonus
            if equip.defence_bonus != 0 {
                let defence_text = if equip.defence_bonus > 0 {
                    format!("+{} Defence", equip.defence_bonus)
                } else {
                    format!("{} Defence", equip.defence_bonus)
                };
                let defence_color = if equip.defence_bonus > 0 {
                    stat_green
                } else {
                    stat_red
                };
                self.draw_text_sharp(
                    &defence_text,
                    tooltip_x + padding,
                    y,
                    small_font_size,
                    defence_color,
                );
                y += line_height;
            }

            // Level requirements - check highest requirement against combat level
            let level_required = equip
                .attack_level_required
                .max(equip.defence_level_required);
            if level_required > 1 {
                let meets_req = player_combat_level >= level_required;
                let req_color = if meets_req { stat_green } else { stat_red };
                let req_text = format!("Requires {} Combat", level_required);
                self.draw_text_sharp(
                    &req_text,
                    tooltip_x + padding,
                    y,
                    small_font_size,
                    req_color,
                );
                y += line_height;
            }

            // Woodcutting level requirement
            if equip.woodcutting_level_required > 1 {
                let meets_req = player_woodcutting_level >= equip.woodcutting_level_required;
                let req_color = if meets_req { stat_green } else { stat_red };
                let req_text = format!("Requires {} Woodcutting", equip.woodcutting_level_required);
                self.draw_text_sharp(
                    &req_text,
                    tooltip_x + padding,
                    y,
                    small_font_size,
                    req_color,
                );
                y += line_height;
            }

            // Mining level requirement
            if equip.mining_level_required > 1 {
                let meets_req = player_mining_level >= equip.mining_level_required;
                let req_color = if meets_req { stat_green } else { stat_red };
                let req_text = format!("Requires {} Mining", equip.mining_level_required);
                self.draw_text_sharp(
                    &req_text,
                    tooltip_x + padding,
                    y,
                    small_font_size,
                    req_color,
                );
                y += line_height;
            }
        }
    }

    /// Render a spell tooltip near the mouse cursor (for quick slot spell bar)
    fn render_quick_slot_spell_tooltip(
        &self,
        spell_def: &crate::game::spell::SpellDef,
        state: &GameState,
    ) {
        let (mouse_x, mouse_y) = mouse_position();
        let padding = 10.0;
        let line_height = 20.0;
        let font_size = 16.0;
        let max_tooltip_width = 220.0;
        let text_width_limit = max_tooltip_width - padding * 2.0;

        // Prepare text lines
        let name_text = spell_def.name.to_string();
        let mana_text = format!("Mana: {}", spell_def.mana_cost);
        let cooldown_text = format!("Cooldown: {:.1}s", spell_def.cooldown_ms as f64 / 1000.0);
        let req_text = format!("Requires {} Magic", spell_def.magic_level_req);
        let desc_lines = if !spell_def.description.is_empty() {
            self.wrap_text(spell_def.description, text_width_limit, font_size)
        } else {
            vec![]
        };

        // Calculate width
        let mut max_w = self.measure_text_sharp(&name_text, font_size).width;
        max_w = max_w.max(self.measure_text_sharp(&mana_text, font_size).width);
        max_w = max_w.max(self.measure_text_sharp(&cooldown_text, font_size).width);
        max_w = max_w.max(self.measure_text_sharp(&req_text, font_size).width);
        for line in &desc_lines {
            max_w = max_w.max(self.measure_text_sharp(line, font_size).width);
        }
        let tooltip_width = (max_w + padding * 2.0).ceil().min(max_tooltip_width);

        // Calculate height: name + type badge + mana + cooldown + req + description
        let mut total_h = padding * 2.0;
        total_h += line_height; // Name
        total_h += line_height; // Type badge
        total_h += line_height; // Mana cost
        total_h += line_height; // Cooldown
        total_h += line_height; // Requirement
        if !desc_lines.is_empty() {
            total_h += 2.0;
            total_h += desc_lines.len() as f32 * line_height;
        }
        let tooltip_height = total_h.ceil();

        // Position tooltip near cursor, keep on screen
        let (sw, sh) = virtual_screen_size();
        let mut tooltip_x = (mouse_x + 16.0).floor();
        let mut tooltip_y = (mouse_y + 16.0).floor();
        if tooltip_x + tooltip_width > sw {
            tooltip_x = (mouse_x - tooltip_width - 8.0).floor();
        }
        if tooltip_y + tooltip_height > sh {
            tooltip_y = (mouse_y - tooltip_height - 8.0).floor();
        }

        // Draw tooltip frame
        draw_rectangle(
            tooltip_x + 2.0,
            tooltip_y + 2.0,
            tooltip_width,
            tooltip_height,
            Color::new(0.0, 0.0, 0.0, 0.4),
        );
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
        draw_line(
            tooltip_x + 1.0,
            tooltip_y + 1.0,
            tooltip_x + tooltip_width - 1.0,
            tooltip_y + 1.0,
            1.0,
            Color::new(0.227, 0.227, 0.267, 1.0),
        );

        let mut y = tooltip_y + padding + 12.0;

        // Spell name
        self.draw_text_sharp(&name_text, tooltip_x + padding, y, font_size, TEXT_NORMAL);
        y += line_height;

        // Type badge
        let type_text = match spell_def.spell_type {
            crate::game::spell::SpellType::Damage => "DAMAGE",
            crate::game::spell::SpellType::Heal => "HEAL",
            crate::game::spell::SpellType::Teleport => "TELEPORT",
        };
        let type_color = match spell_def.spell_type {
            crate::game::spell::SpellType::Damage => Color::new(0.824, 0.345, 0.345, 1.0),
            crate::game::spell::SpellType::Heal => Color::new(0.392, 0.784, 0.392, 1.0),
            crate::game::spell::SpellType::Teleport => Color::new(0.4, 0.55, 0.9, 1.0),
        };
        let badge_w = self.measure_text_sharp(type_text, 16.0).width + 10.0;
        let badge_h = 20.0;
        let badge_x = tooltip_x + padding;
        let badge_y = y - 14.0;
        let badge_bg = Color::new(type_color.r, type_color.g, type_color.b, 0.2);
        draw_rectangle(badge_x, badge_y, badge_w, badge_h, badge_bg);
        draw_rectangle_lines(badge_x, badge_y, badge_w, badge_h, 1.0, type_color);
        self.draw_text_sharp(type_text, badge_x + 5.0, y, 16.0, type_color);
        y += line_height;

        // Mana cost
        let mana_color = Color::new(0.4, 0.6, 1.0, 1.0);
        self.draw_text_sharp(&mana_text, tooltip_x + padding, y, font_size, mana_color);
        y += line_height;

        // Cooldown
        self.draw_text_sharp(&cooldown_text, tooltip_x + padding, y, font_size, TEXT_DIM);
        y += line_height;

        // Requirement
        let magic_level = state
            .get_local_player()
            .map(|p| p.skills.magic.level)
            .unwrap_or(1);
        let meets_req = magic_level >= spell_def.magic_level_req;
        let req_color = if meets_req {
            Color::new(0.392, 0.784, 0.392, 1.0)
        } else {
            Color::new(1.0, 0.392, 0.392, 1.0)
        };
        self.draw_text_sharp(&req_text, tooltip_x + padding, y, font_size, req_color);
        y += line_height;

        // Description
        if !desc_lines.is_empty() {
            y += 2.0;
            for line in &desc_lines {
                self.draw_text_sharp(line, tooltip_x + padding, y, font_size, TEXT_DIM);
                y += line_height;
            }
        }
    }

    /// Get enhanced category color for tooltips
    pub(crate) fn get_category_color(&self, category: &str) -> Color {
        match category.to_lowercase().as_str() {
            "equipment" => CATEGORY_EQUIPMENT,
            "consumable" => CATEGORY_CONSUMABLE,
            "material" => CATEGORY_MATERIAL,
            "quest" => CATEGORY_QUEST,
            _ => TEXT_NORMAL,
        }
    }
}
