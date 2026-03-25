//! Top bar UI components: experience bar and menu buttons

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

/// Menu button icon frame indices in background_icons.png
/// Order: inventory, character, settings, skills, social, prayer, magic
const ICON_INVENTORY: usize = 0;
const ICON_CHARACTER: usize = 1;
const ICON_SETTINGS: usize = 2;
const ICON_SKILLS: usize = 3;
const ICON_SOCIAL: usize = 4;
const ICON_PRAYER: usize = 5;
const ICON_MAGIC: usize = 6;
const ICON_QUEST: usize = 7;

/// Icon dimensions in the sprite sheet (8 icons, 32x32 each)
const ICON_SIZE: f32 = 32.0;

impl Renderer {
    /// Returns the Y position where UI below the top bar can start
    pub fn get_top_bar_bottom(&self) -> f32 {
        0.0
    }

    /// Render the experience bar at the top of the screen
    pub(crate) fn render_exp_bar(&self, state: &GameState) {
        let (screen_w, _) = virtual_screen_size();

        // Bar fills full width, sits at very top
        let bar_height = EXP_BAR_HEIGHT;
        let bar_x = 0.0;
        let bar_y = 0.0;
        let bar_width = screen_w;

        // Background (slightly transparent)
        let bg_color = Color::new(PANEL_BG_DARK.r, PANEL_BG_DARK.g, PANEL_BG_DARK.b, 0.75);
        draw_rectangle(bar_x, bar_y, bar_width, bar_height, bg_color);

        // Get player skill data - show total level and combat level
        let (combat_level, total_level, avg_progress) =
            if let Some(player) = state.get_local_player() {
                // Calculate average progress across combat skills (HP, Atk, Str, Def)
                let hp_prog = player.skills.hitpoints.level_progress();
                let atk_prog = player.skills.attack.level_progress();
                let str_prog = player.skills.strength.level_progress();
                let def_prog = player.skills.defence.level_progress();
                let avg = (hp_prog + atk_prog + str_prog + def_prog) / 4.0;
                (player.combat_level(), player.skills.total_level(), avg)
            } else {
                (3, 23, 0.0) // Default: new character combat level 3, total 23
            };

        // Draw fill bar showing average skill progress (matching HP bar style)
        let fill_width = bar_width * avg_progress;
        if fill_width > 0.0 {
            let fill_height = bar_height - 2.0;
            let exp_green = Color::new(0.2, 0.7, 0.3, 0.65);
            draw_rectangle(bar_x, bar_y + 1.0, fill_width, fill_height, exp_green);
            // Highlight on top half
            draw_rectangle(
                bar_x,
                bar_y + 1.0,
                fill_width,
                fill_height / 2.0,
                Color::new(1.0, 1.0, 1.0, 0.15),
            );
        }

        // Show combat level and total level
        let exp_text = format!("Combat Lv: {}  |  Total Lv: {}", combat_level, total_level);
        let text_dims = self.measure_text_sharp(&exp_text, 16.0);
        let text_x = (bar_width - text_dims.width) / 2.0;
        let text_y = bar_y + (bar_height + 12.0) / 2.0 - 2.0;

        // Text shadow
        self.draw_text_sharp(
            &exp_text,
            text_x + 1.0,
            text_y + 1.0,
            16.0,
            Color::new(0.0, 0.0, 0.0, 0.8),
        );
        // Text
        self.draw_text_sharp(&exp_text, text_x, text_y, 16.0, TEXT_NORMAL);
    }

    /// Render the menu buttons in the bottom-right corner
    pub(crate) fn render_menu_buttons(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (screen_w, screen_h) = virtual_screen_size();
        let scale = state.ui_state.ui_scale;
        let exp_bar_gap = EXP_BAR_GAP * scale;

        // Buttons with their icon frame indices
        let buttons = [
            (
                UiElementId::MenuButtonInventory,
                ICON_INVENTORY,
                state.ui_state.inventory_open,
            ),
            (
                UiElementId::MenuButtonCharacter,
                ICON_CHARACTER,
                state.ui_state.character_panel_open,
            ),
            (
                UiElementId::MenuButtonSkills,
                ICON_SKILLS,
                state.ui_state.skills_open,
            ),
            (
                UiElementId::MenuButtonPrayer,
                if state.ui_state.prayer_spell_tab == 1 {
                    ICON_MAGIC
                } else {
                    ICON_PRAYER
                },
                state.ui_state.prayer_book_open,
            ),
            (
                UiElementId::MenuButtonQuest,
                ICON_QUEST,
                state.ui_state.quest_log_open,
            ),
            (
                UiElementId::MenuButtonSocial,
                ICON_SOCIAL,
                state.ui_state.social_open,
            ),
            (
                UiElementId::MenuButtonSettings,
                ICON_SETTINGS,
                state.ui_state.escape_menu_open,
            ),
        ];

        if cfg!(target_os = "android") {
            // ===== ANDROID: Collapsible menu — toggle button at bottom-right, expands left =====
            let btn_size = 30.0;
            let btn_spacing = 3.0;
            let toggle_x = (screen_w - btn_size - 6.0).floor();
            let toggle_y = (screen_h - exp_bar_gap - btn_size).floor();

            // Draw toggle button (hamburger / X)
            let expanded = state.ui_state.mobile_menu_expanded;
            let toggle_bounds = Rect::new(toggle_x, toggle_y, btn_size, btn_size);
            layout.add(UiElementId::MenuButtonToggle, toggle_bounds);
            let toggle_hovered = matches!(hovered, Some(UiElementId::MenuButtonToggle));

            let (toggle_bg, toggle_border) = if expanded || toggle_hovered {
                (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
            } else {
                (SLOT_BG_EMPTY, SLOT_BORDER)
            };
            draw_rectangle(toggle_x - 1.0, toggle_y - 1.0, btn_size + 2.0, btn_size + 2.0,
                Color::new(toggle_border.r, toggle_border.g, toggle_border.b, 0.9));
            draw_rectangle(toggle_x, toggle_y, btn_size, btn_size,
                Color::new(toggle_bg.r, toggle_bg.g, toggle_bg.b, 0.85));

            // Draw hamburger icon (3 lines) or X
            let line_color = if expanded { TEXT_GOLD } else { TEXT_NORMAL };
            let cx = toggle_x + btn_size / 2.0;
            let cy = toggle_y + btn_size / 2.0;
            if expanded {
                // X icon
                draw_line(cx - 5.0, cy - 5.0, cx + 5.0, cy + 5.0, 2.0, line_color);
                draw_line(cx + 5.0, cy - 5.0, cx - 5.0, cy + 5.0, 2.0, line_color);
            } else {
                // Hamburger icon
                draw_line(cx - 6.0, cy - 4.0, cx + 6.0, cy - 4.0, 2.0, line_color);
                draw_line(cx - 6.0, cy,       cx + 6.0, cy,       2.0, line_color);
                draw_line(cx - 6.0, cy + 4.0, cx + 6.0, cy + 4.0, 2.0, line_color);
            }

            // Show menu buttons only when expanded, expanding left from toggle
            if expanded {
                // Chat button is first (leftmost), then the 7 regular buttons
                let total_count = buttons.len() + 1; // +1 for chat

                // Chat button (leftmost position)
                let chat_x = toggle_x - total_count as f32 * (btn_size + btn_spacing);
                let chat_y = toggle_y;
                let chat_bounds = Rect::new(chat_x, chat_y, btn_size, btn_size);
                layout.add(UiElementId::ChatButton, chat_bounds);
                let chat_hovered = matches!(hovered, Some(UiElementId::ChatButton));
                let chat_active = state.ui_state.chat_panel_open;
                // Draw chat button with the chat icon texture
                let (chat_bg, chat_border) = if chat_active {
                    (SLOT_HOVER_BG, SLOT_SELECTED_BORDER)
                } else if chat_hovered {
                    (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
                } else {
                    (SLOT_BG_EMPTY, SLOT_BORDER)
                };
                draw_rectangle(chat_x - 1.0, chat_y - 1.0, btn_size + 2.0, btn_size + 2.0,
                    Color::new(chat_border.r, chat_border.g, chat_border.b, 0.9));
                draw_rectangle(chat_x, chat_y, btn_size, btn_size,
                    Color::new(chat_bg.r, chat_bg.g, chat_bg.b, 0.85));
                if let Some(ref tex) = self.chat_small_icon {
                    let icon_size = btn_size - 6.0;
                    let ix = (chat_x + (btn_size - icon_size) / 2.0).floor();
                    let iy = (chat_y + (btn_size - icon_size) / 2.0).floor();
                    let tint = if chat_active { TEXT_GOLD } else if chat_hovered { TEXT_TITLE } else { TEXT_NORMAL };
                    draw_texture_ex(tex, ix, iy, tint, DrawTextureParams {
                        dest_size: Some(Vec2::new(icon_size, icon_size)),
                        ..Default::default()
                    });
                }

                // Regular menu buttons
                let num = buttons.len();
                for (i, (element_id, icon_frame, is_active)) in buttons.iter().enumerate() {
                    let x = toggle_x - (num - i) as f32 * (btn_size + btn_spacing);
                    let y = toggle_y;

                    let bounds = Rect::new(x, y, btn_size, btn_size);
                    layout.add(element_id.clone(), bounds);

                    let is_hovered = hovered.as_ref() == Some(element_id);
                    self.draw_menu_button_icon_scaled(x, y, btn_size, *icon_frame, is_hovered, *is_active, scale);

                    if *element_id == UiElementId::MenuButtonSocial {
                        self.render_social_badge(state, x, y, scale);
                    }
                }
            }
        } else {
            // ===== DESKTOP: Single row of 7 buttons, right-aligned =====
            let button_size = MENU_BUTTON_SIZE * scale;
            let button_spacing = MENU_BUTTON_SPACING * scale;
            let button_y = (screen_h - exp_bar_gap - button_size).floor();

            let num_buttons = 7;
            let total_width =
                num_buttons as f32 * button_size + (num_buttons - 1) as f32 * button_spacing;
            let start_x = (screen_w - total_width - 8.0).floor();

            for (i, (element_id, icon_frame, is_active)) in buttons.iter().enumerate() {
                let x = start_x + i as f32 * (button_size + button_spacing);
                let y = button_y;

                let bounds = Rect::new(x, y, button_size, button_size);
                layout.add(element_id.clone(), bounds);

                let is_hovered = hovered.as_ref() == Some(element_id);
                self.draw_menu_button_icon_scaled(x, y, button_size, *icon_frame, is_hovered, *is_active, scale);

                if *element_id == UiElementId::MenuButtonSocial {
                    self.render_social_badge(state, x, y, scale);
                }
            }
        }
    }

    /// Draw a single menu button with an icon from the sprite sheet (scaled)
    fn draw_menu_button_icon_scaled(
        &self,
        x: f32,
        y: f32,
        size: f32,
        icon_frame: usize,
        is_hovered: bool,
        is_active: bool,
        scale: f32,
    ) {
        // Frame colors based on state
        let (bg_color, border_color) = if is_active {
            // Active state - brighter
            (SLOT_HOVER_BG, SLOT_SELECTED_BORDER)
        } else if is_hovered {
            // Hover state
            (SLOT_HOVER_BG, SLOT_HOVER_BORDER)
        } else {
            // Normal state
            (SLOT_BG_EMPTY, SLOT_BORDER)
        };

        // Outer border with slight transparency
        let border_alpha = Color::new(border_color.r, border_color.g, border_color.b, 0.9);
        draw_rectangle(x - 1.0, y - 1.0, size + 2.0, size + 2.0, border_alpha);

        // Background
        let bg_alpha = Color::new(bg_color.r, bg_color.g, bg_color.b, 0.85);
        draw_rectangle(x, y, size, size, bg_alpha);

        // Inner shadow for depth
        draw_rectangle(x, y, size, 2.0, SLOT_INNER_SHADOW);
        draw_rectangle(x, y, 2.0, size, SLOT_INNER_SHADOW);

        // Bottom/right highlight
        if is_active {
            let accent = Color::new(FRAME_ACCENT.r, FRAME_ACCENT.g, FRAME_ACCENT.b, 0.5);
            draw_line(
                x + 2.0,
                y + size - 1.0,
                x + size - 2.0,
                y + size - 1.0,
                1.0,
                accent,
            );
            draw_line(
                x + size - 1.0,
                y + 2.0,
                x + size - 1.0,
                y + size - 2.0,
                1.0,
                accent,
            );
        }

        // Draw icon from sprite sheet if available
        // Keep icon at native 32x32 for crisp pixel art - only scale the container
        let icon_size = ICON_SIZE; // Don't scale pixel art icons
        if let Some(ref texture) = self.menu_button_icons {
            // Calculate source rectangle for this frame
            let src_x = icon_frame as f32 * ICON_SIZE;
            let src_rect = Rect::new(src_x, 0.0, ICON_SIZE, ICON_SIZE);

            // Center icon in button, floor to integer pixels for crisp pixel art
            let icon_x = (x + (size - icon_size) / 2.0).floor();
            let icon_y = (y + (size - icon_size) / 2.0).floor();

            // Tint based on state
            let tint = if is_active {
                TEXT_GOLD
            } else if is_hovered {
                TEXT_TITLE
            } else {
                TEXT_NORMAL
            };

            draw_texture_ex(
                texture,
                icon_x,
                icon_y,
                tint,
                DrawTextureParams {
                    source: Some(src_rect),
                    dest_size: Some(Vec2::new(icon_size, icon_size)),
                    ..Default::default()
                },
            );
        } else {
            // Fallback: draw text label if texture not loaded (native font size)
            let labels = ["I", "C", "K", "P", "S", "O"]; // Inventory, Character, sKills, Prayer, Social, Options
            let label = labels.get(icon_frame).unwrap_or(&"?");

            let text_dims = self.measure_text_sharp(label, 16.0);
            let text_x = x + (size - text_dims.width) / 2.0;
            let text_y = y + (size + 12.0) / 2.0;

            let text_color = if is_active {
                TEXT_GOLD
            } else if is_hovered {
                TEXT_TITLE
            } else {
                TEXT_NORMAL
            };

            self.draw_text_sharp(label, text_x, text_y, 16.0, text_color);
        }
    }
}
