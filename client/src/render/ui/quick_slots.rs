//! Unified hotkey bar rendering — items + spells mixed in 5 slots with presets

use super::super::isometric::world_to_screen_z;
use super::super::Renderer;
use super::common::*;
use crate::game::hotkey::HotkeySlotBinding;
use crate::game::spell::SPELLS;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

impl Renderer {
    pub(crate) fn render_quick_slots(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        // On Android, render 3 circular slots above the attack/use buttons instead
        if cfg!(target_os = "android") {
            self.render_mobile_quick_slots(state, hovered, layout);
            return;
        }

        let scale = state.ui_state.ui_scale;
        let slot_size = (QUICK_SLOT_SIZE * scale).max(MIN_SLOT_SIZE);
        let spacing = QUICK_SLOT_SPACING * scale;
        let total_slots_width = 5.0 * slot_size + 4.0 * spacing;

        let (sw, sh) = virtual_screen_size();

        // --- Compute positions for the left control column ---
        // A single vertical column: increment-page (▲) / editor (✱), stacked.
        // The column is bottom-aligned with the slot row so each control is
        // comfortably sized and readable.
        let col_w = (28.0 * scale).max(26.0);
        let box_gap = (2.0 * scale).max(2.0);
        let caret_h = (18.0 * scale).max(18.0); // top increment button
        let asterisk_h = (20.0 * scale).max(20.0); // middle editor button
        let col_h = caret_h + asterisk_h + box_gap;
        let left_controls_w = col_w + spacing;

        // Center the whole assembly (left column + slots)
        let total_w = left_controls_w + total_slots_width;
        let base_x = (sw - total_w) / 2.0;
        let slots_start_x = base_x + left_controls_w;
        let slots_start_y = sh - EXP_BAR_GAP * scale - slot_size;

        // No tray behind the hotbar — elements float, each slot carries its own
        // menu-button-style box (matching the action-bar buttons).

        // --- Left control column (▲ increment / ✱ editor) ---
        // Both buttons use a filled box that lightens on hover.
        let col_x = base_x;
        // Bottom edge aligns with the slot row; the column extends upward.
        let col_top = slots_start_y + slot_size - col_h;

        // Same box style as the hotkey slots: 1px 0.9-alpha outer border rect +
        // 0.85-alpha fill + top/left inner shadow, with identical hover colors.
        let draw_btn_box = |x: f32, y: f32, w: f32, h: f32, active: bool| {
            let (bg_color, border_color) = if active {
                (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
            } else {
                (SLOT_BG_EMPTY, SLOT_BORDER)
            };
            draw_rectangle(
                x - 1.0,
                y - 1.0,
                w + 2.0,
                h + 2.0,
                Color::new(border_color.r, border_color.g, border_color.b, 0.9),
            );
            draw_rectangle(
                x,
                y,
                w,
                h,
                Color::new(bg_color.r, bg_color.g, bg_color.b, 0.85),
            );
            draw_rectangle(x, y, w, 2.0, SLOT_INNER_SHADOW);
            draw_rectangle(x, y, 2.0, h, SLOT_INNER_SHADOW);
        };

        // Top box: increment hotkey page (^ button — cycles and wraps).
        let inc_y = col_top;
        layout.add(
            UiElementId::HotkeyPresetUp,
            Rect::new(col_x, inc_y, col_w, caret_h),
        );
        let inc_hovered = matches!(hovered, Some(UiElementId::HotkeyPresetUp));
        draw_btn_box(col_x, inc_y, col_w, caret_h, inc_hovered);
        if let Some(arrow) = self.arrow_raise_icon.as_ref() {
            // 16x16 raise-arrow texture, centered in the box (pixel-snapped, nearest-filtered).
            let a = (16.0 * scale).floor().min(caret_h).min(col_w);
            let ax = (col_x + (col_w - a) / 2.0).floor();
            let ay = (inc_y + (caret_h - a) / 2.0).floor();
            let tint = if inc_hovered { WHITE } else { TEXT_NORMAL };
            draw_texture_ex(
                arrow,
                ax,
                ay,
                tint,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(a, a)),
                    ..Default::default()
                },
            );
        }

        // Middle box: editor / settings (gear button).
        let ed_y = inc_y + caret_h + box_gap;
        layout.add(
            UiElementId::HotkeySettingsCog,
            Rect::new(col_x, ed_y, col_w, asterisk_h),
        );
        let ed_active = matches!(hovered, Some(UiElementId::HotkeySettingsCog))
            || state.ui_state.hotkey_settings_open;
        draw_btn_box(col_x, ed_y, col_w, asterisk_h, ed_active);
        if let Some(gear) = self.gear_icon.as_ref() {
            // 16x16 gear texture, centered in the box (pixel-snapped, nearest-filtered).
            let g = (16.0 * scale).floor().min(asterisk_h).min(col_w);
            let gx = (col_x + (col_w - g) / 2.0).floor();
            let gy = (ed_y + (asterisk_h - g) / 2.0).floor();
            let tint = if ed_active { WHITE } else { TEXT_NORMAL };
            draw_texture_ex(
                gear,
                gx,
                gy,
                tint,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(g, g)),
                    ..Default::default()
                },
            );
        }

        // --- 5 Unified Slots ---
        let active_preset = state.ui_state.hotkey_bar.active();
        let now = macroquad::time::get_time();
        let player_mp = state.get_local_player().map(|p| p.mp).unwrap_or(0);

        for i in 0..5 {
            let x = slots_start_x + i as f32 * (slot_size + spacing);
            let y = slots_start_y;

            let bounds = Rect::new(x, y, slot_size, slot_size);
            layout.add(UiElementId::QuickSlot(i), bounds);

            let is_hovered = matches!(hovered, Some(UiElementId::QuickSlot(idx)) if *idx == i);

            // Slot background: identical to the menu buttons (1px outer border rect +
            // 0.85-alpha fill + inner top/left shadow). Same color states as the buttons.
            let (bg_color, border_color) = if is_hovered {
                (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
            } else {
                (SLOT_BG_EMPTY, SLOT_BORDER)
            };
            draw_rectangle(
                x - 1.0,
                y - 1.0,
                slot_size + 2.0,
                slot_size + 2.0,
                Color::new(border_color.r, border_color.g, border_color.b, 0.9),
            );
            draw_rectangle(
                x,
                y,
                slot_size,
                slot_size,
                Color::new(bg_color.r, bg_color.g, bg_color.b, 0.85),
            );
            draw_rectangle(x, y, slot_size, 2.0, SLOT_INNER_SHADOW);
            draw_rectangle(x, y, 2.0, slot_size, SLOT_INNER_SHADOW);

            match &active_preset.slots[i] {
                HotkeySlotBinding::Empty => {}
                HotkeySlotBinding::Item { item_id } => {
                    // Look up item in inventory
                    let inv_slot = state.inventory.find_slot_by_item_id(item_id);
                    let quantity = inv_slot.and_then(|idx| {
                        state
                            .inventory
                            .slots
                            .get(idx)
                            .and_then(|s| s.as_ref())
                            .map(|s| s.quantity)
                    });
                    let has_item = quantity.is_some();
                    let is_ghost = !has_item; // Depleted — show at 30% opacity

                    // Draw item icon (ghost = 30% opacity via tint)
                    if is_ghost {
                        // Ghost: draw with reduced alpha
                        let tint = Color::new(1.0, 1.0, 1.0, 0.3);
                        self.draw_item_icon_tinted(
                            item_id, x, y, slot_size, slot_size, state, tint,
                        );
                    } else {
                        self.draw_item_icon(item_id, x, y, slot_size, slot_size, state, false);
                    }

                    // Quantity badge
                    if let Some(qty) = quantity {
                        if qty > 1 {
                            let qty_text = qty.to_string();
                            self.draw_text_sharp(
                                &qty_text,
                                x + 3.0 * scale,
                                y + slot_size - 4.0,
                                16.0,
                                Color::new(0.0, 0.0, 0.0, 0.8),
                            );
                            self.draw_text_sharp(
                                &qty_text,
                                x + 2.0 * scale,
                                y + slot_size - 5.0,
                                16.0,
                                TEXT_NORMAL,
                            );
                        }
                    }
                }
                HotkeySlotBinding::Spell { spell_id } => {
                    // Look up spell from static spells or scroll spell definitions
                    let spell_info: Option<(&str, &str, crate::game::spell::SpellType, i32)> =
                        SPELLS
                            .iter()
                            .find(|s| s.id == spell_id)
                            .map(|s| (s.id, s.name, s.spell_type, s.mana_cost))
                            .or_else(|| {
                                state
                                    .scroll_spell_definitions
                                    .iter()
                                    .find(|s| s.id == *spell_id)
                                    .map(|s| {
                                        (s.id.as_str(), s.name.as_str(), s.spell_type, s.mana_cost)
                                    })
                            });

                    if let Some((id, name, spell_type, mana_cost)) = spell_info {
                        // Spell icon
                        if let Some((texture, source_rect)) = self.spell_icons.get(id) {
                            let icon_size = slot_size - 8.0;
                            let icon_x = (x + (slot_size - icon_size) / 2.0).floor();
                            let icon_y = (y + (slot_size - icon_size) / 2.0).floor();
                            draw_texture_ex(
                                texture,
                                icon_x,
                                icon_y,
                                WHITE,
                                DrawTextureParams {
                                    source: source_rect,
                                    dest_size: Some(Vec2::new(icon_size, icon_size)),
                                    ..Default::default()
                                },
                            );
                        } else {
                            // Fallback: colored rectangle with spell's first letter
                            let color = match spell_type {
                                crate::game::spell::SpellType::Damage => {
                                    Color::new(0.6, 0.15, 0.15, 0.9)
                                }
                                crate::game::spell::SpellType::Heal => {
                                    Color::new(0.15, 0.5, 0.15, 0.9)
                                }
                                crate::game::spell::SpellType::Teleport => {
                                    Color::new(0.2, 0.3, 0.6, 0.9)
                                }
                            };
                            let pad = 4.0;
                            draw_rectangle(
                                x + pad,
                                y + pad,
                                slot_size - pad * 2.0,
                                slot_size - pad * 2.0,
                                color,
                            );
                            let letter = &name[..1];
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

                        // Mana cost badge (bottom-left)
                        let mana_text = mana_cost.to_string();
                        self.draw_text_sharp(
                            &mana_text,
                            x + 3.0 * scale,
                            y + slot_size - 4.0,
                            16.0,
                            Color::new(0.0, 0.0, 0.0, 0.8),
                        );
                        self.draw_text_sharp(
                            &mana_text,
                            x + 2.0 * scale,
                            y + slot_size - 5.0,
                            16.0,
                            Color::new(0.4, 0.6, 1.0, 1.0),
                        );

                        // Cooldown overlay
                        let on_cooldown = state.spell_cooldowns.get(id).is_some_and(|&t| now < t);
                        let insufficient_mana = player_mp < mana_cost;

                        if on_cooldown {
                            draw_rectangle(
                                x + 2.0,
                                y + 2.0,
                                slot_size - 4.0,
                                slot_size - 4.0,
                                Color::new(0.0, 0.0, 0.0, 0.55),
                            );
                            let remaining = state
                                .spell_cooldowns
                                .get(id)
                                .map_or(0.0, |&t| (t - now).max(0.0));
                            let cd_text = if remaining >= 60.0 {
                                let mins = (remaining / 60.0).floor() as u32;
                                let secs = (remaining % 60.0).floor() as u32;
                                format!("{}:{:02}", mins, secs)
                            } else {
                                format!("{:.1}", remaining)
                            };
                            let cd_w = self.measure_text_sharp(&cd_text, 16.0).width;
                            self.draw_text_sharp(
                                &cd_text,
                                x + (slot_size - cd_w) / 2.0,
                                y + slot_size / 2.0 + 4.0,
                                16.0,
                                WHITE,
                            );
                        } else if insufficient_mana {
                            draw_rectangle(
                                x + 2.0,
                                y + 2.0,
                                slot_size - 4.0,
                                slot_size - 4.0,
                                Color::new(0.6, 0.1, 0.1, 0.45),
                            );
                        }
                    } else {
                        // Unknown spell — slot background already drawn above
                    }
                }
            }

            // Slot number badge (top-right) — always drawn
            let num_text = (i + 1).to_string();
            let text_w = self.measure_text_sharp(&num_text, 16.0).width;
            let badge_w = text_w + 2.0;
            let badge_h = 13.0;
            let num_x = x + slot_size - badge_w - 1.0;
            let num_y = y + 1.0;
            draw_rectangle(
                num_x,
                num_y,
                badge_w,
                badge_h,
                Color::new(0.0, 0.0, 0.0, 0.5),
            );
            self.draw_text_sharp(&num_text, num_x + 1.0, num_y + 11.0, 16.0, TEXT_NORMAL);
        }

        // --- Settings Popup ---
        if state.ui_state.hotkey_settings_open {
            self.render_hotkey_settings_popup(
                state,
                hovered,
                layout,
                slots_start_x,
                slots_start_y,
                slot_size,
                spacing,
            );
        }
    }

    /// Render 3 circular quick slots above the attack/use buttons (mobile only)
    fn render_mobile_quick_slots(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        // Hide when any panel is open
        let ui = &state.ui_state;
        if ui.inventory_open
            || ui.character_panel_open
            || ui.skills_open
            || ui.prayer_book_open
            || ui.escape_menu_open
            || ui.quest_log_open
            || ui.social_open
            || ui.chat_panel_open
            || ui.crafting_open
            || ui.furnace_open
            || ui.anvil_open
            || ui.fletching_open
            || ui.bank_open
            || ui.chest_open
            || ui.shop_data.is_some()
            || state.ui_state.active_dialogue.is_some()
        {
            return;
        }

        let (sw, sh) = virtual_screen_size();
        let _scale = state.ui_state.ui_scale;
        let active_preset = state.ui_state.hotkey_bar.active();
        let now = macroquad::time::get_time();
        let player_mp = state.get_local_player().map(|p| p.mp).unwrap_or(0);

        let radius = 20.0;
        // Arc around the attack button center (sw-42, sh-120)
        let attack_cx = sw - 42.0;
        let attack_cy = sh - 120.0;
        let arc_dist = 65.0; // distance from attack center to slot center
                             // 3 slots in an arc: left (170°), upper-left (130°), top (90°)
        let angles: [f32; 3] = [170.0_f32, 130.0, 90.0];

        for (i, angle) in angles.iter().enumerate() {
            let angle_rad = angle.to_radians();
            let cx = attack_cx + arc_dist * angle_rad.cos();
            let cy = attack_cy - arc_dist * angle_rad.sin();
            let slot_rect = Rect::new(cx - radius, cy - radius, radius * 2.0, radius * 2.0);
            layout.add(UiElementId::QuickSlot(i), slot_rect);

            let is_hovered = matches!(hovered, Some(UiElementId::QuickSlot(idx)) if *idx == i);

            // Circle background
            let bg_color = if is_hovered {
                Color::new(0.25, 0.22, 0.18, 0.85)
            } else {
                Color::new(0.1, 0.1, 0.13, 0.75)
            };
            draw_circle(cx, cy, radius, bg_color);
            let border_color = if is_hovered {
                Color::new(0.557, 0.424, 0.267, 1.0)
            } else {
                Color::new(0.35, 0.3, 0.25, 0.8)
            };
            draw_circle_lines(cx, cy, radius, 1.5, border_color);

            // Icon area (square inscribed in circle)
            let icon_size = radius * 1.3;
            let icon_x = cx - icon_size / 2.0;
            let icon_y = cy - icon_size / 2.0;

            match &active_preset.slots[i] {
                HotkeySlotBinding::Empty => {
                    // Empty slot — just the circle
                }
                HotkeySlotBinding::Item { item_id } => {
                    let inv_slot = state.inventory.find_slot_by_item_id(item_id);
                    let quantity = inv_slot.and_then(|idx| {
                        state
                            .inventory
                            .slots
                            .get(idx)
                            .and_then(|s| s.as_ref())
                            .map(|s| s.quantity)
                    });
                    let has_item = quantity.is_some();

                    if has_item {
                        self.draw_item_icon(
                            item_id, icon_x, icon_y, icon_size, icon_size, state, false,
                        );
                    } else {
                        let tint = Color::new(1.0, 1.0, 1.0, 0.3);
                        self.draw_item_icon_tinted(
                            item_id, icon_x, icon_y, icon_size, icon_size, state, tint,
                        );
                    }

                    // Quantity badge
                    if let Some(qty) = quantity {
                        if qty > 1 {
                            let qty_text = qty.to_string();
                            self.draw_text_sharp(
                                &qty_text,
                                cx - radius + 4.0,
                                cy + radius - 4.0,
                                16.0,
                                Color::new(0.0, 0.0, 0.0, 0.8),
                            );
                            self.draw_text_sharp(
                                &qty_text,
                                cx - radius + 3.0,
                                cy + radius - 5.0,
                                16.0,
                                TEXT_NORMAL,
                            );
                        }
                    }
                }
                HotkeySlotBinding::Spell { spell_id } => {
                    let spell_info: Option<(&str, &str, crate::game::spell::SpellType, i32)> =
                        SPELLS
                            .iter()
                            .find(|s| s.id == spell_id)
                            .map(|s| (s.id, s.name, s.spell_type, s.mana_cost))
                            .or_else(|| {
                                state
                                    .scroll_spell_definitions
                                    .iter()
                                    .find(|s| s.id == *spell_id)
                                    .map(|s| {
                                        (s.id.as_str(), s.name.as_str(), s.spell_type, s.mana_cost)
                                    })
                            });

                    if let Some((id, name, spell_type, mana_cost)) = spell_info {
                        // Spell icon
                        if let Some((texture, source_rect)) = self.spell_icons.get(id) {
                            draw_texture_ex(
                                texture,
                                icon_x,
                                icon_y,
                                WHITE,
                                DrawTextureParams {
                                    source: source_rect,
                                    dest_size: Some(Vec2::new(icon_size, icon_size)),
                                    ..Default::default()
                                },
                            );
                        } else {
                            let color = match spell_type {
                                crate::game::spell::SpellType::Damage => {
                                    Color::new(0.6, 0.15, 0.15, 0.9)
                                }
                                crate::game::spell::SpellType::Heal => {
                                    Color::new(0.15, 0.5, 0.15, 0.9)
                                }
                                crate::game::spell::SpellType::Teleport => {
                                    Color::new(0.2, 0.3, 0.6, 0.9)
                                }
                            };
                            draw_circle(cx, cy, radius - 4.0, color);
                            let letter = &name[..1];
                            let lw = self.measure_text_sharp(letter, 18.0).width;
                            self.draw_text_sharp(letter, cx - lw / 2.0, cy + 5.0, 18.0, WHITE);
                        }

                        // Mana cost badge
                        let mana_text = mana_cost.to_string();
                        self.draw_text_sharp(
                            &mana_text,
                            cx - radius + 4.0,
                            cy + radius - 4.0,
                            16.0,
                            Color::new(0.0, 0.0, 0.0, 0.8),
                        );
                        self.draw_text_sharp(
                            &mana_text,
                            cx - radius + 3.0,
                            cy + radius - 5.0,
                            16.0,
                            Color::new(0.4, 0.6, 1.0, 1.0),
                        );

                        // Cooldown overlay
                        let on_cooldown = state.spell_cooldowns.get(id).is_some_and(|&t| now < t);
                        let insufficient_mana = player_mp < mana_cost;

                        if on_cooldown {
                            draw_circle(cx, cy, radius - 1.0, Color::new(0.0, 0.0, 0.0, 0.55));
                            let remaining = state
                                .spell_cooldowns
                                .get(id)
                                .map_or(0.0, |&t| (t - now).max(0.0));
                            let cd_text = format!("{:.1}", remaining);
                            let cd_w = self.measure_text_sharp(&cd_text, 16.0).width;
                            self.draw_text_sharp(&cd_text, cx - cd_w / 2.0, cy + 5.0, 16.0, WHITE);
                        } else if insufficient_mana {
                            draw_circle(cx, cy, radius - 1.0, Color::new(0.6, 0.1, 0.1, 0.45));
                        }
                    }
                }
            }

            // Slot number badge (small, at top)
            let num_text = (i + 1).to_string();
            let tw = self.measure_text_sharp(&num_text, 12.0).width;
            self.draw_text_sharp(
                &num_text,
                cx - tw / 2.0,
                cy - radius + 10.0,
                12.0,
                Color::new(1.0, 1.0, 1.0, 0.5),
            );
        }
    }

    /// Render the hotkey settings popup above the bar
    fn render_hotkey_settings_popup(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        slots_x: f32,
        slots_y: f32,
        slot_size: f32,
        spacing: f32,
    ) {
        let scale = state.ui_state.ui_scale;
        let total_slots_w = 5.0 * slot_size + 4.0 * spacing;

        // Floating bronze window above the hotkey bar, matching the game's panels.
        let pad = 8.0 * scale;
        let top_pad = 16.0 * scale; // space beneath the floating title tab
        let tab_h = (22.0 * scale).max(20.0);
        let row_gap = 8.0 * scale;
        let preview_slot_size = (36.0 * scale).max(32.0);

        let popup_w = total_slots_w + 2.0 * pad;
        let popup_h = top_pad + tab_h + row_gap + preview_slot_size + pad;
        let popup_x = slots_x - pad;
        let popup_y = slots_y - popup_h - 6.0;
        let inner_x = popup_x + pad;
        let inner_w = popup_w - 2.0 * pad;

        // ===== Bronze panel frame + gold corner accents (the game's window style) =====
        self.draw_panel_frame(popup_x, popup_y, popup_w, popup_h);
        self.draw_corner_accents(popup_x, popup_y, popup_w, popup_h);

        // ===== Floating title tab =====
        {
            let title_text = "HOTKEY PRESETS";
            let title_width = self.measure_text_sharp(title_text, 16.0).width + 28.0;
            let title_x = popup_x + (popup_w - title_width) / 2.0;
            let title_y = popup_y - 8.0;
            let title_h = 26.0;
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
            draw_line(
                title_x + 2.0,
                title_y + 2.0,
                title_x + title_width - 2.0,
                title_y + 2.0,
                1.0,
                FRAME_INNER,
            );
            self.draw_text_sharp(title_text, title_x + 14.0, title_y + 18.0, 16.0, TEXT_TITLE);
            draw_rectangle(title_x, title_y, 3.0, 1.0, FRAME_ACCENT);
            draw_rectangle(title_x + title_width - 3.0, title_y, 3.0, 1.0, FRAME_ACCENT);
        }

        // ===== Preset tabs row =====
        let tab_w = inner_w / 5.0;
        let tab_y = (popup_y + top_pad).floor();
        for i in 0..5 {
            let tx = (inner_x + i as f32 * tab_w).floor();
            let bw = (tab_w - 2.0).floor();
            let tab_bounds = Rect::new(tx, tab_y, bw, tab_h);
            layout.add(UiElementId::HotkeySettingsPresetTab(i), tab_bounds);

            let is_active = state.ui_state.hotkey_bar.active_preset == i;
            let is_tab_hovered =
                matches!(hovered, Some(UiElementId::HotkeySettingsPresetTab(idx)) if *idx == i);

            let bg = if is_active {
                SLOT_HOVER_BG
            } else if is_tab_hovered {
                SLOT_BG_FILLED
            } else {
                PANEL_BG_MID
            };
            // Active preset gets the bright gold selected-border so it clearly pops.
            let border = if is_active {
                SLOT_SELECTED_BORDER
            } else if is_tab_hovered {
                SLOT_HOVER_BORDER
            } else {
                SLOT_BORDER
            };
            // Full crisp border on all four sides (outer border rect + inset fill).
            draw_rectangle(tx, tab_y, bw, tab_h, border);
            draw_rectangle(tx + 1.0, tab_y + 1.0, bw - 2.0, tab_h - 2.0, bg);

            let label = (i + 1).to_string();
            let lw = self.measure_text_sharp(&label, 16.0).width;
            let text_color = if is_active { TEXT_TITLE } else { TEXT_DIM };
            self.draw_text_sharp(
                &label,
                (tx + (bw - lw) / 2.0).floor(),
                (tab_y + tab_h / 2.0 + 5.0).floor(),
                16.0,
                text_color,
            );
        }

        // ===== Slot preview row =====
        let preview_spacing = (inner_w - 5.0 * preview_slot_size) / 4.0;
        let preview_y = tab_y + tab_h + row_gap;
        let active_preset = state.ui_state.hotkey_bar.active();

        for i in 0..5 {
            let px = inner_x + i as f32 * (preview_slot_size + preview_spacing);
            let py = preview_y;

            let slot_bounds = Rect::new(px, py, preview_slot_size, preview_slot_size);
            layout.add(UiElementId::HotkeySettingsSlot(i), slot_bounds);

            let is_slot_hovered =
                matches!(hovered, Some(UiElementId::HotkeySettingsSlot(idx)) if *idx == i);
            let s_state = if is_slot_hovered {
                SlotState::Hovered
            } else {
                SlotState::Normal
            };

            match &active_preset.slots[i] {
                HotkeySlotBinding::Empty => {
                    self.draw_inventory_slot(px, py, preview_slot_size, false, s_state);
                }
                HotkeySlotBinding::Item { item_id } => {
                    self.draw_inventory_slot(px, py, preview_slot_size, true, s_state);
                    self.draw_item_icon(
                        item_id,
                        px,
                        py,
                        preview_slot_size,
                        preview_slot_size,
                        state,
                        false,
                    );
                }
                HotkeySlotBinding::Spell { spell_id } => {
                    self.draw_inventory_slot(px, py, preview_slot_size, true, s_state);
                    if let Some((texture, source_rect)) = self.spell_icons.get(spell_id.as_str()) {
                        let icon_size = preview_slot_size - 6.0;
                        let icon_x = (px + (preview_slot_size - icon_size) / 2.0).floor();
                        let icon_y = (py + (preview_slot_size - icon_size) / 2.0).floor();
                        draw_texture_ex(
                            texture,
                            icon_x,
                            icon_y,
                            WHITE,
                            DrawTextureParams {
                                source: source_rect,
                                dest_size: Some(Vec2::new(icon_size, icon_size)),
                                ..Default::default()
                            },
                        );
                    } else {
                        let name = SPELLS
                            .iter()
                            .find(|s| s.id == spell_id)
                            .map(|s| s.name.to_string())
                            .or_else(|| {
                                state
                                    .scroll_spell_definitions
                                    .iter()
                                    .find(|s| s.id == *spell_id)
                                    .map(|s| s.name.clone())
                            });
                        if let Some(name) = name {
                            let letter = &name[..1];
                            let lw = self.measure_text_sharp(letter, 16.0).width;
                            self.draw_text_sharp(
                                letter,
                                px + (preview_slot_size - lw) / 2.0,
                                py + preview_slot_size * 0.65,
                                16.0,
                                Color::new(0.5, 0.4, 0.9, 1.0),
                            );
                        }
                    }
                }
            }

            // X (clear) button — small button at top-right of each preview slot
            if !matches!(&active_preset.slots[i], HotkeySlotBinding::Empty) {
                let clear_size = 16.0;
                let cx = (px + preview_slot_size - clear_size - 1.0).floor();
                let cy = (py + 1.0).floor();
                let clear_bounds = Rect::new(cx, cy, clear_size, clear_size);
                layout.add(UiElementId::HotkeySettingsSlotClear(i), clear_bounds);

                let clear_hovered = matches!(
                    hovered,
                    Some(UiElementId::HotkeySettingsSlotClear(idx)) if *idx == i
                );
                let (clear_bg, clear_border) = if clear_hovered {
                    (
                        Color::new(0.62, 0.18, 0.18, 0.95),
                        Color::new(0.85, 0.45, 0.40, 1.0),
                    )
                } else {
                    (Color::new(0.28, 0.10, 0.10, 0.8), SLOT_BORDER)
                };
                draw_rectangle(cx, cy, clear_size, clear_size, clear_bg);
                draw_rectangle_lines(cx, cy, clear_size, clear_size, 1.0, clear_border);
                // Native 16px glyph (multiple-of-8 bitmap size) so it stays crisp.
                let xd = self.measure_text_sharp("X", 16.0);
                self.draw_text_sharp(
                    "X",
                    (cx + (clear_size - xd.width) / 2.0).floor(),
                    (cy + (clear_size + xd.height) / 2.0 - 1.0).floor(),
                    16.0,
                    if clear_hovered { WHITE } else { TEXT_NORMAL },
                );
            }
        }
    }

    /// Draw item icon with a custom tint (for ghost/depleted items)
    pub(crate) fn draw_item_icon_tinted(
        &self,
        item_id: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        state: &GameState,
        tint: Color,
    ) {
        let sprite_key = state.item_registry.get_sprite_key(item_id);
        if let Some((texture, source_rect)) = self.item_sprites.get(sprite_key) {
            let (icon_width, icon_height) = if let Some(r) = source_rect {
                (r.w, r.h)
            } else {
                (texture.width(), texture.height())
            };
            let offset_x = (w - icon_width) / 2.0;
            let offset_y = (h - icon_height) / 2.0;
            draw_texture_ex(
                texture,
                x + offset_x,
                y + offset_y,
                tint,
                DrawTextureParams {
                    source: source_rect,
                    ..Default::default()
                },
            );
        } else {
            // Fallback: draw the item id text dimly
            let item_def = state.item_registry.get_or_placeholder(item_id);
            let letter = if item_def.display_name.is_empty() {
                "?"
            } else {
                &item_def.display_name[..1]
            };
            let lw = self.measure_text_sharp(letter, 16.0).width;
            self.draw_text_sharp(letter, x + (w - lw) / 2.0, y + h * 0.65, 16.0, tint);
        }
    }

    pub(crate) fn render_ground_item_overlays(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        // Ground item labels are world-space (already scaled by zoom), not UI,
        // so reset font_scale to avoid double-scaling with ui_scale.
        let prev_font_scale = self.font_scale.get();
        self.font_scale.set(1.0);

        let zoom = state.camera.zoom;

        for (item_id, item) in &state.ground_items {
            let item_z = state.chunk_manager.get_height(item.x as i32, item.y as i32) as f32;
            let (screen_x, screen_y) = world_to_screen_z(item.x, item.y, item_z, &state.camera);

            // Clickable area - cover the full isometric tile so hovering/clicking
            // anywhere on the tile triggers the ground item interaction
            let click_width = 64.0 * zoom;
            let click_height = 32.0 * zoom;
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
                let item_z = state.chunk_manager.get_height(item.x as i32, item.y as i32) as i32;
                self.render_tile_hover(item.x as i32, item.y as i32, item_z, &state.camera);

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
                let gold_offset = if item.item_id == "gold" {
                    22.0 * zoom
                } else {
                    0.0
                };
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

        self.font_scale.set(prev_font_scale);
    }
}
