//! Slayer panel rendering - task display and reward shop

use super::super::Renderer;
use super::common::*;
use crate::game::GameState;
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

/// Reward item row height
const REWARD_ROW_HEIGHT: f32 = 44.0;
const REWARD_ROW_SPACING: f32 = 4.0;
/// Tab names for reward shop categories
const TAB_NAMES: [&str; 4] = ["Potions", "Unlocks", "Equipment", "Blocks"];
/// Tab category keys matching server data
const TAB_CATEGORIES: [&str; 4] = ["potion", "unlock", "equipment", "block"];

impl Renderer {
    /// Render a small HUD chip below the stat bars showing current slayer task
    pub(crate) fn render_slayer_task_chip(&self, state: &GameState, x: f32, y: f32) -> (f32, f32) {
        let task = match &state.ui_state.slayer_current_task {
            Some(t) => t,
            None => return (0.0, 0.0),
        };

        let s = state.ui_state.ui_scale;
        let sprite_area = 24.0 * s;
        let font_sz = 16.0;
        let padding = 3.0 * s;
        let count_text = format!("{}/{}", task.kills_current, task.kills_required);
        let count_dims = self.measure_text_sharp(&count_text, font_sz);
        let chip_w = (sprite_area + padding * 2.0).max(count_dims.width + padding * 2.0);
        let chip_h = padding + sprite_area + 2.0 * s + count_dims.height + padding;

        // Semi-transparent dark background (matches the name/portrait box fill)
        draw_rectangle(x, y, chip_w, chip_h, Color::new(0.094, 0.094, 0.122, 0.62));
        draw_rectangle_lines(x, y, chip_w, chip_h, 1.0, Color::from_rgba(80, 70, 55, 180));

        // Draw NPC sprite (idle frame 0, down-facing)
        if let Some((npc_texture, npc_atlas_offset)) = self.npc_sprites.get(&task.monster_id) {
            let (tex_w, tex_h): (f32, f32) = self
                .npc_sprites
                .get_dimensions(&task.monster_id)
                .unwrap_or((npc_texture.width(), npc_texture.height()));
            let frame_width = tex_w / 16.0;
            let frame_height = tex_h;
            let (atlas_x, atlas_y): (f32, f32) = npc_atlas_offset.unwrap_or((0.0, 0.0));

            let source = Rect::new(atlas_x, atlas_y, frame_width, frame_height);
            let scale = (sprite_area / frame_width).min(sprite_area / frame_height);
            let draw_w = frame_width * scale;
            let draw_h = frame_height * scale;
            let draw_x = x + (chip_w - draw_w) / 2.0;
            let draw_y = y + padding + (sprite_area - draw_h) / 2.0;

            draw_texture_ex(
                npc_texture,
                draw_x,
                draw_y,
                WHITE,
                DrawTextureParams {
                    source: Some(source),
                    dest_size: Some(Vec2::new(draw_w, draw_h)),
                    ..Default::default()
                },
            );
        }

        // Kill count text centered below sprite
        self.draw_text_sharp(
            &count_text,
            (x + (chip_w - count_dims.width) / 2.0).floor(),
            (y + padding + sprite_area + 2.0 * s + count_dims.height * 0.9).floor(),
            font_sz,
            TEXT_NORMAL,
        );

        (chip_w, chip_h)
    }

    /// Render a small HUD chip showing the player's active combat style.
    /// Returns `(chip_width, chip_height)` so the caller can position adjacent chips.
    /// Legacy floating combat-style HUD chip. The desktop HUD now embeds an
    /// interactive style selector in the stat cluster, so this is retained only for
    /// potential reuse (e.g. a future compact mode).
    #[allow(dead_code)]
    pub(crate) fn render_combat_style_chip(&self, state: &GameState, x: f32, y: f32) -> (f32, f32) {
        let style_raw = match state.get_local_player() {
            Some(p) => p.combat_style.clone(),
            None => return (0.0, 0.0),
        };

        let (abbrev, color) = match style_raw.as_str() {
            "accurate" => ("Acc", Color::from_rgba(100, 200, 100, 255)),
            "aggressive" => ("Agg", Color::from_rgba(220, 80, 80, 255)),
            "defensive" => ("Def", Color::from_rgba(80, 140, 220, 255)),
            "controlled" => ("Ctrl", Color::from_rgba(220, 180, 60, 255)),
            _ => ("Acc", Color::from_rgba(100, 200, 100, 255)),
        };

        let s = state.ui_state.ui_scale;
        let font_sz = 16.0;
        let padding = 3.0 * s;

        let label = "Style";
        let label_dims = self.measure_text_sharp(label, font_sz);
        let abbrev_dims = self.measure_text_sharp(abbrev, font_sz);
        let text_w = label_dims.width.max(abbrev_dims.width);
        let chip_w = text_w + padding * 2.0;
        let line_h = label_dims.height + 2.0 * s;
        let chip_h = padding + line_h + abbrev_dims.height + padding;

        // Background + border (matches slayer chip style)
        draw_rectangle(x, y, chip_w, chip_h, Color::from_rgba(0, 0, 0, 180));
        draw_rectangle_lines(x, y, chip_w, chip_h, 1.0, Color::from_rgba(80, 70, 55, 180));

        // "Style" label centered
        self.draw_text_sharp(
            label,
            (x + (chip_w - label_dims.width) / 2.0).floor(),
            (y + padding + label_dims.height * 0.9).floor(),
            font_sz,
            TEXT_DIM,
        );

        // Abbreviation centered, color-coded
        self.draw_text_sharp(
            abbrev,
            (x + (chip_w - abbrev_dims.width) / 2.0).floor(),
            (y + padding + line_h + abbrev_dims.height * 0.9).floor(),
            font_sz,
            color,
        );

        (chip_w, chip_h)
    }

    /// Render a single potion buff HUD chip with sprite + countdown timer.
    /// Returns `(chip_width, chip_height)`.
    pub(crate) fn render_potion_buff_chip(
        &self,
        state: &GameState,
        buff: &crate::game::ActivePotionBuff,
        x: f32,
        y: f32,
    ) -> (f32, f32) {
        let s = state.ui_state.ui_scale;
        let font_sz = 16.0;
        let padding = 3.0 * s;
        let sprite_area = 24.0 * s;

        // Calculate remaining time
        let now = macroquad::time::get_time();
        let remaining = (buff.expires_at - now).max(0.0);
        if remaining <= 0.0 {
            return (0.0, 0.0);
        }

        let remaining_secs = remaining as u64;
        let timer_text = if remaining_secs >= 60 {
            format!("{}:{:02}", remaining_secs / 60, remaining_secs % 60)
        } else {
            format!("{}s", remaining_secs)
        };

        let timer_dims = self.measure_text_sharp(&timer_text, font_sz);
        let chip_w = (sprite_area + padding * 2.0).max(timer_dims.width + padding * 2.0);
        let chip_h = padding + sprite_area + 2.0 * s + timer_dims.height + padding;

        // Border color by stat
        let border_color = match buff.stat.as_str() {
            "attack" => Color::from_rgba(100, 200, 100, 180),
            "strength" => Color::from_rgba(220, 80, 80, 180),
            "defence" => Color::from_rgba(80, 140, 220, 180),
            _ => Color::from_rgba(80, 70, 55, 180),
        };

        // Background + border
        draw_rectangle(x, y, chip_w, chip_h, Color::from_rgba(0, 0, 0, 180));
        draw_rectangle_lines(x, y, chip_w, chip_h, 1.0, border_color);

        // Draw potion sprite using item_sprites
        let sprite_key = state.item_registry.get_sprite_key(&buff.source_item_id);
        if let Some((texture, source_rect)) = self.item_sprites.get(sprite_key) {
            let (icon_w, icon_h) = if let Some(r) = source_rect {
                (r.w, r.h)
            } else {
                (texture.width(), texture.height())
            };
            let scale = (sprite_area / icon_w).min(sprite_area / icon_h);
            let draw_w = icon_w * scale;
            let draw_h = icon_h * scale;
            let draw_x = x + (chip_w - draw_w) / 2.0;
            let draw_y = y + padding + (sprite_area - draw_h) / 2.0;

            draw_texture_ex(
                texture,
                draw_x,
                draw_y,
                WHITE,
                DrawTextureParams {
                    source: source_rect,
                    dest_size: Some(Vec2::new(draw_w, draw_h)),
                    ..Default::default()
                },
            );
        }

        // Timer text centered below sprite
        let timer_color = if remaining_secs <= 10 {
            Color::from_rgba(255, 100, 100, 255)
        } else {
            TEXT_NORMAL
        };
        self.draw_text_sharp(
            &timer_text,
            (x + (chip_w - timer_dims.width) / 2.0).floor(),
            (y + padding + sprite_area + 2.0 * s + timer_dims.height * 0.9).floor(),
            font_sz,
            timer_color,
        );

        (chip_w, chip_h)
    }

    /// Render hover tooltip for a potion buff chip
    pub(crate) fn render_potion_buff_chip_tooltip(
        &self,
        state: &GameState,
        buff: &crate::game::ActivePotionBuff,
        chip_x: f32,
        chip_y: f32,
        chip_w: f32,
        chip_h: f32,
    ) {
        let (raw_mx, raw_my) = mouse_position();
        let (vw, vh) = virtual_screen_size();
        let mx = raw_mx * vw / screen_width();
        let my = raw_my * vh / screen_height();

        if mx < chip_x || mx > chip_x + chip_w || my < chip_y || my > chip_y + chip_h {
            return;
        }

        let s = state.ui_state.ui_scale;
        let tip_font = 16.0;
        let line_h = 18.0 * s;
        let tip_pad = 6.0 * s;

        let now = macroquad::time::get_time();
        let remaining = (buff.expires_at - now).max(0.0);
        let remaining_secs = remaining as u64;
        let time_str = if remaining_secs >= 60 {
            format!(
                "{}m {}s remaining",
                remaining_secs / 60,
                remaining_secs % 60
            )
        } else {
            format!("{}s remaining", remaining_secs)
        };

        // Potion display name from item registry
        let potion_name = state
            .item_registry
            .get_or_placeholder(&buff.source_item_id)
            .display_name
            .clone();

        let stat_label = match buff.stat.as_str() {
            "attack" => "Attack",
            "strength" => "Strength",
            "defence" => "Defence",
            "magic" => "Magic",
            _ => &buff.stat,
        };

        let lines = [
            (potion_name, TEXT_TITLE),
            (format!("+{} {}", buff.amount, stat_label), TEXT_NORMAL),
            (time_str, TEXT_DIM),
        ];

        let tip_w = lines
            .iter()
            .map(|(text, _)| self.measure_text_sharp(text, tip_font).width)
            .fold(0.0f32, f32::max)
            + tip_pad * 2.0;
        let tip_h = tip_pad + lines.len() as f32 * line_h + tip_pad;

        let tip_x = chip_x + chip_w + 4.0 * s;
        let tip_y = chip_y;

        draw_rectangle(
            tip_x,
            tip_y,
            tip_w,
            tip_h,
            Color::from_rgba(12, 12, 18, 240),
        );
        draw_rectangle_lines(
            tip_x,
            tip_y,
            tip_w,
            tip_h,
            1.0,
            Color::from_rgba(80, 70, 55, 200),
        );

        for (i, (text, color)) in lines.iter().enumerate() {
            self.draw_text_sharp(
                text,
                tip_x + tip_pad,
                (tip_y + tip_pad + (i as f32 + 0.8) * line_h).floor(),
                tip_font,
                *color,
            );
        }
    }

    /// Render hover tooltip for slayer task chip (called after other overlapping UI)
    pub(crate) fn render_slayer_task_chip_tooltip(&self, state: &GameState, x: f32, y: f32) {
        let task = match &state.ui_state.slayer_current_task {
            Some(t) => t,
            None => return,
        };

        let s = state.ui_state.ui_scale;
        let sprite_area = 24.0 * s;
        let font_sz = 16.0;
        let padding = 3.0 * s;
        let count_text = format!("{}/{}", task.kills_current, task.kills_required);
        let count_dims = self.measure_text_sharp(&count_text, font_sz);
        let chip_w = (sprite_area + padding * 2.0).max(count_dims.width + padding * 2.0);
        let chip_h = padding + sprite_area + 2.0 * s + count_dims.height + padding;

        let (raw_mx, raw_my) = mouse_position();
        let (vw, vh) = virtual_screen_size();
        let mx = raw_mx * vw / screen_width();
        let my = raw_my * vh / screen_height();

        if mx >= x && mx <= x + chip_w && my >= y && my <= y + chip_h {
            let tip_x = x + chip_w + 4.0 * s;
            let tip_y = y;
            let tip_font = 16.0;
            let line_h = 18.0 * s;
            let tip_pad = 6.0 * s;

            let lines = [
                (format!("Task: {}", task.display_name), TEXT_TITLE),
                (
                    format!("Progress: {}/{}", task.kills_current, task.kills_required),
                    TEXT_NORMAL,
                ),
                (format!("XP/kill: {}", task.xp_per_kill), TEXT_NORMAL),
                (
                    format!("Points on completion: {}", task.points_on_complete),
                    TEXT_NORMAL,
                ),
                (
                    format!("Slayer Points: {}", state.ui_state.slayer_points),
                    TEXT_GOLD,
                ),
                (
                    format!("Tasks completed: {}", state.ui_state.slayer_tasks_completed),
                    TEXT_DIM,
                ),
            ];

            let tip_w = lines
                .iter()
                .map(|(text, _)| self.measure_text_sharp(text, tip_font).width)
                .fold(0.0f32, f32::max)
                + tip_pad * 2.0;
            let tip_h = tip_pad + lines.len() as f32 * line_h + tip_pad;

            draw_rectangle(
                tip_x,
                tip_y,
                tip_w,
                tip_h,
                Color::from_rgba(12, 12, 18, 240),
            );
            draw_rectangle_lines(
                tip_x,
                tip_y,
                tip_w,
                tip_h,
                1.0,
                Color::from_rgba(80, 70, 55, 200),
            );

            for (i, (text, color)) in lines.iter().enumerate() {
                self.draw_text_sharp(
                    text,
                    tip_x + tip_pad,
                    (tip_y + tip_pad + (i as f32 + 0.8) * line_h).floor(),
                    tip_font,
                    *color,
                );
            }
        }
    }

    /// Render a small HUD chip showing the active resource contract: the target item's
    /// icon with the collection progress beneath it. Full detail lives in the hover tooltip.
    pub(crate) fn render_resource_contract_chip(
        &self,
        state: &GameState,
        x: f32,
        y: f32,
    ) -> (f32, f32) {
        let contract = match &state.resource_contract {
            Some(c) => c,
            None => return (0.0, 0.0),
        };

        let s = state.ui_state.ui_scale;
        let sprite_area = 24.0 * s;
        let font_sz = 16.0;
        let padding = 3.0 * s;
        let count_text = format!("{}/{}", contract.amount_completed, contract.amount_required);
        let count_dims = self.measure_text_sharp(&count_text, font_sz);
        let chip_w = (sprite_area + padding * 2.0).max(count_dims.width + padding * 2.0);
        let chip_h = padding + sprite_area + 2.0 * s + count_dims.height + padding;

        // Background (matches the name/portrait box fill) + a green-tinted border to distinguish from the slayer chip's brown.
        draw_rectangle(x, y, chip_w, chip_h, Color::new(0.094, 0.094, 0.122, 0.62));
        draw_rectangle_lines(
            x,
            y,
            chip_w,
            chip_h,
            1.0,
            Color::from_rgba(90, 120, 65, 180),
        );

        // Target item icon.
        if !contract.target_item_id.is_empty() {
            let sprite_key = state.item_registry.get_sprite_key(&contract.target_item_id);
            if let Some((texture, source_rect)) = self.item_sprites.get(sprite_key) {
                let (icon_w, icon_h) = if let Some(r) = source_rect {
                    (r.w, r.h)
                } else {
                    (texture.width(), texture.height())
                };
                let scale = (sprite_area / icon_w).min(sprite_area / icon_h);
                let draw_w = icon_w * scale;
                let draw_h = icon_h * scale;
                let draw_x = x + (chip_w - draw_w) / 2.0;
                let draw_y = y + padding + (sprite_area - draw_h) / 2.0;
                draw_texture_ex(
                    texture,
                    draw_x,
                    draw_y,
                    WHITE,
                    DrawTextureParams {
                        source: source_rect,
                        dest_size: Some(Vec2::new(draw_w, draw_h)),
                        ..Default::default()
                    },
                );
            }
        }

        // Progress count below the icon.
        let complete = contract.amount_completed >= contract.amount_required;
        let count_color = if complete {
            Color::from_rgba(120, 230, 120, 255)
        } else {
            TEXT_NORMAL
        };
        self.draw_text_sharp(
            &count_text,
            (x + (chip_w - count_dims.width) / 2.0).floor(),
            (y + padding + sprite_area + 2.0 * s + count_dims.height * 0.9).floor(),
            font_sz,
            count_color,
        );

        (chip_w, chip_h)
    }

    /// Hover tooltip for the resource contract chip — full contract detail.
    pub(crate) fn render_resource_contract_chip_tooltip(&self, state: &GameState, x: f32, y: f32) {
        let contract = match &state.resource_contract {
            Some(c) => c,
            None => return,
        };

        let s = state.ui_state.ui_scale;
        let sprite_area = 24.0 * s;
        let font_sz = 16.0;
        let padding = 3.0 * s;
        let count_text = format!("{}/{}", contract.amount_completed, contract.amount_required);
        let count_dims = self.measure_text_sharp(&count_text, font_sz);
        let chip_w = (sprite_area + padding * 2.0).max(count_dims.width + padding * 2.0);
        let chip_h = padding + sprite_area + 2.0 * s + count_dims.height + padding;

        let (raw_mx, raw_my) = mouse_position();
        let (vw, vh) = virtual_screen_size();
        let mx = raw_mx * vw / screen_width();
        let my = raw_my * vh / screen_height();
        if mx < x || mx > x + chip_w || my < y || my > y + chip_h {
            return;
        }

        let tip_font = 16.0;
        let line_h = 18.0 * s;
        let tip_pad = 6.0 * s;

        let complete = contract.amount_completed >= contract.amount_required;
        let header = if contract.contract_kind.is_empty() {
            "Contract".to_string()
        } else {
            format!("Contract: {}", contract.contract_kind)
        };
        let mut lines: Vec<(String, Color)> = vec![
            (header, TEXT_TITLE),
            (
                format!("{} ({})", contract.task_text, contract.difficulty),
                TEXT_NORMAL,
            ),
            (
                format!(
                    "{}/{} {}",
                    contract.amount_completed, contract.amount_required, contract.progress_label
                ),
                if complete { TEXT_GOLD } else { TEXT_NORMAL },
            ),
        ];
        if complete {
            lines.push((format!("Return to {}", contract.giver_name), TEXT_DIM));
        }

        let tip_w = lines
            .iter()
            .map(|(text, _)| self.measure_text_sharp(text, tip_font).width)
            .fold(0.0f32, f32::max)
            + tip_pad * 2.0;
        let tip_h = tip_pad + lines.len() as f32 * line_h + tip_pad;
        let tip_x = x + chip_w + 4.0 * s;
        let tip_y = y;

        draw_rectangle(
            tip_x,
            tip_y,
            tip_w,
            tip_h,
            Color::from_rgba(12, 12, 18, 240),
        );
        draw_rectangle_lines(
            tip_x,
            tip_y,
            tip_w,
            tip_h,
            1.0,
            Color::from_rgba(80, 70, 55, 200),
        );

        for (i, (text, color)) in lines.iter().enumerate() {
            self.draw_text_sharp(
                text,
                tip_x + tip_pad,
                (tip_y + tip_pad + (i as f32 + 0.8) * line_h).floor(),
                tip_font,
                *color,
            );
        }
    }

    pub(crate) fn render_slayer_panel(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;

        let panel_width = (500.0 * s).min(sw - 16.0);
        let panel_height = (600.0 * s).min(sh - 16.0);
        let panel_x = (sw - panel_width) / 2.0;
        let panel_y = (sh - panel_height) / 2.0;

        // Semi-transparent overlay
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.85));

        // Panel frame
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        let inner_x = panel_x + FRAME_THICKNESS;
        let inner_w = panel_width - FRAME_THICKNESS * 2.0;
        let padding = 10.0 * s;

        // ===== HEADER SECTION =====
        let header_h = HEADER_HEIGHT * s;
        let header_y = panel_y + FRAME_THICKNESS;

        draw_rectangle(inner_x, header_y, inner_w, header_h, HEADER_BG);
        draw_line(
            inner_x + 10.0 * s,
            header_y + header_h,
            inner_x + inner_w - 10.0 * s,
            header_y + header_h,
            2.0,
            HEADER_BORDER,
        );

        // Title
        let master_name = state
            .ui_state
            .slayer_master_name
            .as_deref()
            .unwrap_or("Slayer Master");
        let title = format!("Slayer Master: {}", master_name);
        let title_dims = self.measure_text_sharp(&title, 16.0);
        self.draw_text_sharp(
            &title,
            inner_x + (inner_w - title_dims.width) / 2.0,
            header_y + header_h * 0.71,
            16.0,
            TEXT_TITLE,
        );

        // Close button (X)
        let is_mobile = cfg!(target_os = "android");
        let close_btn_size = if is_mobile { 32.0 * s } else { 28.0 * s };
        let close_btn_x = inner_x + inner_w - close_btn_size - 6.0 * s;
        let close_btn_y = header_y + (header_h - close_btn_size) / 2.0;
        let close_bounds = Rect::new(close_btn_x, close_btn_y, close_btn_size, close_btn_size);
        layout.add(UiElementId::SlayerCloseButton, close_bounds);

        let is_close_hovered = matches!(hovered, Some(UiElementId::SlayerCloseButton));
        let (close_bg, close_border) = if is_close_hovered {
            (
                Color::new(0.4, 0.15, 0.15, 1.0),
                Color::new(0.6, 0.2, 0.2, 1.0),
            )
        } else {
            (Color::new(0.2, 0.1, 0.1, 1.0), FRAME_MID)
        };
        draw_rectangle(
            close_btn_x,
            close_btn_y,
            close_btn_size,
            close_btn_size,
            close_border,
        );
        draw_rectangle(
            close_btn_x + 1.0,
            close_btn_y + 1.0,
            close_btn_size - 2.0,
            close_btn_size - 2.0,
            close_bg,
        );

        let cx = close_btn_x + close_btn_size / 2.0;
        let cy = close_btn_y + close_btn_size / 2.0;
        let cross = close_btn_size * 0.25;
        let cross_color = if is_close_hovered {
            TEXT_TITLE
        } else {
            TEXT_DIM
        };
        draw_line(
            cx - cross,
            cy - cross,
            cx + cross,
            cy + cross,
            2.0,
            cross_color,
        );
        draw_line(
            cx + cross,
            cy - cross,
            cx - cross,
            cy + cross,
            2.0,
            cross_color,
        );

        // ===== SLAYER POINTS DISPLAY =====
        let points_y = header_y + header_h + padding;
        let points_text = format!("{} Slayer Points", state.ui_state.slayer_points);
        let points_dims = self.measure_text_sharp(&points_text, 16.0);
        self.draw_text_sharp(
            &points_text,
            inner_x + (inner_w - points_dims.width) / 2.0,
            points_y + 14.0 * s,
            16.0,
            TEXT_GOLD,
        );

        // ===== CURRENT TASK SECTION =====
        let task_section_y = points_y + 24.0 * s;
        let task_section_h = 90.0 * s;

        // Section background
        draw_rectangle(
            inner_x + padding,
            task_section_y,
            inner_w - padding * 2.0,
            task_section_h,
            Color::new(0.08, 0.08, 0.10, 1.0),
        );
        draw_rectangle_lines(
            inner_x + padding,
            task_section_y,
            inner_w - padding * 2.0,
            task_section_h,
            1.0,
            SLOT_BORDER,
        );

        let task_content_x = inner_x + padding + 10.0 * s;
        let task_content_w = inner_w - padding * 2.0 - 20.0 * s;

        if let Some(ref task) = state.ui_state.slayer_current_task {
            // Task progress
            let progress_text = format!(
                "{}: {}/{} kills",
                task.display_name, task.kills_current, task.kills_required
            );
            self.draw_text_sharp(
                &progress_text,
                task_content_x,
                task_section_y + 20.0 * s,
                16.0,
                TEXT_NORMAL,
            );

            // Task details
            let details_text = format!(
                "{}xp per kill  |  {} pts on completion",
                task.xp_per_kill, task.points_on_complete
            );
            self.draw_text_sharp(
                &details_text,
                task_content_x,
                task_section_y + 40.0 * s,
                16.0,
                TEXT_DIM,
            );

            // Cancel task button (reddish, costs 30 pts)
            let cancel_w = 140.0 * s;
            let cancel_h = 28.0 * s;
            let cancel_x = task_content_x + task_content_w - cancel_w;
            let cancel_y = task_section_y + task_section_h - cancel_h - 8.0 * s;
            let cancel_bounds = Rect::new(cancel_x, cancel_y, cancel_w, cancel_h);
            layout.add(UiElementId::SlayerCancelTaskButton, cancel_bounds);

            let can_cancel = state.ui_state.slayer_points >= 30;
            let is_cancel_hovered = matches!(hovered, Some(UiElementId::SlayerCancelTaskButton));
            let (cancel_bg, cancel_border) = if !can_cancel {
                (
                    Color::new(0.1, 0.1, 0.1, 1.0),
                    Color::new(0.3, 0.3, 0.3, 1.0),
                )
            } else if is_cancel_hovered {
                (
                    Color::new(0.5, 0.15, 0.15, 1.0),
                    Color::new(0.7, 0.25, 0.25, 1.0),
                )
            } else {
                (
                    Color::new(0.35, 0.1, 0.1, 1.0),
                    Color::new(0.5, 0.18, 0.18, 1.0),
                )
            };

            draw_rectangle(cancel_x, cancel_y, cancel_w, cancel_h, cancel_border);
            draw_rectangle(
                cancel_x + 1.0,
                cancel_y + 1.0,
                cancel_w - 2.0,
                cancel_h - 2.0,
                cancel_bg,
            );

            let cancel_text = "Cancel (-30 pts)";
            let cancel_text_color = if can_cancel { WHITE } else { TEXT_DIM };
            let cancel_text_dims = self.measure_text_sharp(cancel_text, 16.0);
            self.draw_text_sharp(
                cancel_text,
                cancel_x + (cancel_w - cancel_text_dims.width) / 2.0,
                cancel_y + cancel_h * 0.71,
                16.0,
                cancel_text_color,
            );
        } else {
            // No active task
            self.draw_text_sharp(
                "No active task",
                task_content_x,
                task_section_y + 24.0 * s,
                16.0,
                TEXT_DIM,
            );

            // Get new task button (gold theme)
            let get_w = 140.0 * s;
            let get_h = 28.0 * s;
            let get_x = task_content_x + task_content_w - get_w;
            let get_y = task_section_y + task_section_h - get_h - 8.0 * s;
            let get_bounds = Rect::new(get_x, get_y, get_w, get_h);
            layout.add(UiElementId::SlayerGetTaskButton, get_bounds);

            let is_get_hovered = matches!(hovered, Some(UiElementId::SlayerGetTaskButton));
            let (get_bg, get_border) = if is_get_hovered {
                (Color::new(0.235, 0.204, 0.141, 1.0), FRAME_ACCENT)
            } else {
                (Color::new(0.157, 0.141, 0.110, 1.0), FRAME_MID)
            };

            draw_rectangle(get_x, get_y, get_w, get_h, get_border);
            draw_rectangle(get_x + 1.0, get_y + 1.0, get_w - 2.0, get_h - 2.0, get_bg);

            let get_text = "Get New Task";
            let get_text_color = if is_get_hovered {
                TEXT_TITLE
            } else {
                TEXT_NORMAL
            };
            let get_text_dims = self.measure_text_sharp(get_text, 16.0);
            self.draw_text_sharp(
                get_text,
                get_x + (get_w - get_text_dims.width) / 2.0,
                get_y + get_h * 0.71,
                16.0,
                get_text_color,
            );
        }

        // ===== REWARD SHOP SECTION =====
        let shop_y = task_section_y + task_section_h + padding;
        let shop_h = panel_y + panel_height - FRAME_THICKNESS - shop_y - padding;

        // Section label
        let shop_label = "Reward Shop";
        let shop_label_dims = self.measure_text_sharp(shop_label, 16.0);
        self.draw_text_sharp(
            shop_label,
            inner_x + (inner_w - shop_label_dims.width) / 2.0,
            shop_y + 14.0 * s,
            16.0,
            TEXT_TITLE,
        );

        // Tabs
        let tab_y = shop_y + 22.0 * s;
        let tab_h = TAB_HEIGHT * s;
        let tab_area_w = inner_w - padding * 2.0;
        let tab_w = tab_area_w / TAB_NAMES.len() as f32;
        let active_tab = state.ui_state.slayer_reward_tab;

        for (i, tab_name) in TAB_NAMES.iter().enumerate() {
            let tx = inner_x + padding + i as f32 * tab_w;
            let tab_bounds = Rect::new(tx, tab_y, tab_w, tab_h);
            layout.add(UiElementId::SlayerRewardTab(i), tab_bounds);

            let is_selected = i == active_tab;
            let is_tab_hovered =
                matches!(hovered, Some(UiElementId::SlayerRewardTab(idx)) if *idx == i);

            let (tab_bg, tab_border_color) = if is_selected {
                (SLOT_HOVER_BG, SLOT_SELECTED_BORDER)
            } else if is_tab_hovered {
                (Color::new(0.141, 0.141, 0.188, 1.0), SLOT_HOVER_BORDER)
            } else {
                (SLOT_BG_EMPTY, SLOT_BORDER)
            };

            draw_rectangle(tx, tab_y, tab_w, tab_h, tab_border_color);
            draw_rectangle(tx + 1.0, tab_y + 1.0, tab_w - 2.0, tab_h - 2.0, tab_bg);

            // Active tab indicator (gold bottom line)
            if is_selected {
                draw_rectangle(
                    tx + 2.0,
                    tab_y + tab_h - 3.0,
                    tab_w - 4.0,
                    3.0,
                    FRAME_ACCENT,
                );
            }

            let tab_text_color = if is_selected { TEXT_TITLE } else { TEXT_NORMAL };
            let tab_text_dims = self.measure_text_sharp(tab_name, TAB_FONT_SIZE);
            self.draw_text_sharp(
                tab_name,
                tx + (tab_w - tab_text_dims.width) / 2.0,
                tab_y + tab_h * 0.68,
                TAB_FONT_SIZE,
                tab_text_color,
            );
        }

        // Content area below tabs
        let content_y = tab_y + tab_h + 6.0 * s;
        let content_h = shop_y + shop_h - content_y;
        let content_x = inner_x + padding;
        let content_w = inner_w - padding * 2.0;

        // Register scroll area
        let scroll_rect = Rect::new(content_x, content_y, content_w, content_h);
        layout.add(UiElementId::SlayerScrollArea, scroll_rect);

        // Content background
        draw_rectangle(
            content_x,
            content_y,
            content_w,
            content_h,
            Color::new(0.06, 0.06, 0.08, 1.0),
        );
        draw_rectangle_lines(content_x, content_y, content_w, content_h, 1.0, SLOT_BORDER);

        // Filter rewards by active tab category
        let active_category = TAB_CATEGORIES[active_tab];
        let filtered_rewards: Vec<(usize, &crate::game::slayer::SlayerRewardClientData)> = state
            .ui_state
            .slayer_rewards
            .iter()
            .enumerate()
            .filter(|(_, r)| r.category == active_category)
            .collect();

        // Render reward rows with scroll offset
        let row_h = REWARD_ROW_HEIGHT * s;
        let row_sp = REWARD_ROW_SPACING * s;
        let scroll_offset = state.ui_state.slayer_reward_scroll;

        // Scissor clip for scroll area
        let (real_sw, real_sh) = (screen_width(), screen_height());
        let scale_x = real_sw / sw;
        let scale_y = real_sh / sh;
        let clip_x = (content_x * scale_x) as i32;
        let clip_y = (content_y * scale_y) as i32;
        let clip_w = (content_w * scale_x) as i32;
        let clip_h = (content_h * scale_y) as i32;

        unsafe {
            miniquad::gl::glEnable(miniquad::gl::GL_SCISSOR_TEST);
            miniquad::gl::glScissor(clip_x, real_sh as i32 - clip_y - clip_h, clip_w, clip_h);
        }

        if filtered_rewards.is_empty() && active_tab != 2 && active_tab != 3 {
            self.draw_text_sharp(
                "No rewards available",
                content_x + 10.0 * s,
                content_y + 24.0 * s,
                16.0,
                TEXT_DIM,
            );
        } else {
            let mut row_idx = 0;

            // Render reward items
            for (global_idx, reward) in &filtered_rewards {
                let item_y =
                    content_y + 4.0 * s + row_idx as f32 * (row_h + row_sp) - scroll_offset;

                // Skip items outside visible area (but still count them for layout)
                if item_y + row_h >= content_y && item_y <= content_y + content_h {
                    let can_afford = state.ui_state.slayer_points >= reward.cost;

                    // Row background
                    let row_bg = if row_idx % 2 == 0 {
                        Color::new(0.08, 0.08, 0.10, 0.6)
                    } else {
                        Color::new(0.06, 0.06, 0.08, 0.6)
                    };
                    draw_rectangle(content_x + 2.0, item_y, content_w - 4.0, row_h, row_bg);

                    // Reward name
                    let name_color = if can_afford { TEXT_NORMAL } else { TEXT_DIM };
                    self.draw_text_sharp(
                        &reward.display_name,
                        content_x + 10.0 * s,
                        item_y + 16.0 * s,
                        16.0,
                        name_color,
                    );

                    // Description
                    let desc_color = if can_afford {
                        Color::new(0.6, 0.6, 0.65, 1.0)
                    } else {
                        Color::new(0.35, 0.35, 0.38, 1.0)
                    };
                    self.draw_text_sharp(
                        &reward.description,
                        content_x + 10.0 * s,
                        item_y + 34.0 * s,
                        16.0,
                        desc_color,
                    );

                    // Cost (right-aligned)
                    let cost_text = format!("{} pts", reward.cost);
                    let cost_color = if can_afford { TEXT_GOLD } else { TEXT_DIM };
                    let cost_dims = self.measure_text_sharp(&cost_text, 16.0);

                    // Buy button
                    let btn_w = 50.0 * s;
                    let btn_h = 24.0 * s;
                    let btn_x = content_x + content_w - btn_w - 8.0 * s;
                    let btn_y = item_y + (row_h - btn_h) / 2.0;
                    let btn_bounds = Rect::new(btn_x, btn_y, btn_w, btn_h);
                    layout.add(UiElementId::SlayerBuyReward(*global_idx), btn_bounds);

                    let is_buy_hovered = matches!(hovered, Some(UiElementId::SlayerBuyReward(idx)) if *idx == *global_idx);

                    let (btn_bg, btn_border_color) = if !can_afford {
                        (
                            Color::new(0.1, 0.1, 0.1, 1.0),
                            Color::new(0.3, 0.3, 0.3, 1.0),
                        )
                    } else if is_buy_hovered {
                        (Color::new(0.235, 0.204, 0.141, 1.0), FRAME_ACCENT)
                    } else {
                        (Color::new(0.157, 0.141, 0.110, 1.0), FRAME_MID)
                    };

                    draw_rectangle(btn_x, btn_y, btn_w, btn_h, btn_border_color);
                    draw_rectangle(btn_x + 1.0, btn_y + 1.0, btn_w - 2.0, btn_h - 2.0, btn_bg);

                    let btn_text_color = if can_afford { TEXT_NORMAL } else { TEXT_DIM };
                    let buy_text = "Buy";
                    let buy_dims = self.measure_text_sharp(buy_text, 16.0);
                    self.draw_text_sharp(
                        buy_text,
                        btn_x + (btn_w - buy_dims.width) / 2.0,
                        btn_y + btn_h * 0.71,
                        16.0,
                        btn_text_color,
                    );

                    // Cost text to the left of the buy button
                    self.draw_text_sharp(
                        &cost_text,
                        btn_x - cost_dims.width - 8.0 * s,
                        item_y + row_h * 0.55,
                        16.0,
                        cost_color,
                    );
                }

                row_idx += 1;
            }

            // For Blocks tab, show selectable monster list + currently blocked monsters
            // The list gets its own scroll area below the reward row
            if active_tab == 3 {
                let compact_h = 24.0 * s;
                let compact_sp = 2.0 * s;
                let scrollbar_w = 4.0 * s;

                // List area starts after the reward rows (separator + padding)
                let list_top = content_y + 4.0 * s + row_idx as f32 * (row_h + row_sp) + 4.0 * s;
                let list_h = (content_y + content_h - list_top).max(0.0);
                let list_w = content_w;

                // Filter out already-blocked monsters
                let available: Vec<(usize, &(String, String))> = state
                    .ui_state
                    .slayer_blockable_monsters
                    .iter()
                    .enumerate()
                    .filter(|(_, (id, _))| !state.ui_state.slayer_blocked_monsters.contains(id))
                    .collect();

                // Calculate total content height for scroll
                let mut total_h = 0.0_f32;
                if !available.is_empty() {
                    total_h += compact_h + compact_sp; // "Select monster to block:" header
                    total_h += available.len() as f32 * (compact_h + compact_sp);
                    total_h += 4.0 * s; // gap
                }
                total_h += compact_h + compact_sp; // "Currently Blocked:" header
                if state.ui_state.slayer_blocked_monsters.is_empty() {
                    total_h += compact_h + compact_sp; // "No blocked monsters"
                } else {
                    total_h += state.ui_state.slayer_blocked_monsters.len() as f32
                        * (compact_h + compact_sp);
                }

                let max_scroll = (total_h - list_h).max(0.0);
                let block_scroll = state
                    .ui_state
                    .slayer_block_scroll_offset
                    .clamp(0.0, max_scroll);
                let needs_scrollbar = max_scroll > 0.0;
                let draw_w = if needs_scrollbar {
                    list_w - scrollbar_w - 4.0 * s
                } else {
                    list_w
                };

                // Register scroll area for mouse wheel
                layout.add(
                    UiElementId::SlayerBlockScrollArea,
                    Rect::new(content_x, list_top, list_w, list_h),
                );

                // Separator line
                if !filtered_rewards.is_empty() {
                    draw_line(
                        content_x + 10.0 * s,
                        list_top - 2.0 * s,
                        content_x + draw_w - 10.0 * s,
                        list_top - 2.0 * s,
                        1.0,
                        HEADER_BORDER,
                    );
                }

                // Flush macroquad's draw batch before switching scissor
                {
                    let mut gl = unsafe { get_internal_gl() };
                    gl.flush();
                }

                // Set up scissor clip for the block list area
                let list_clip_x = (content_x * scale_x) as i32;
                let list_clip_y = (list_top * scale_y) as i32;
                let list_clip_w = (list_w * scale_x) as i32;
                let list_clip_h = (list_h * scale_y) as i32;
                unsafe {
                    miniquad::gl::glScissor(
                        list_clip_x,
                        real_sh as i32 - list_clip_y - list_clip_h,
                        list_clip_w,
                        list_clip_h,
                    );
                }

                let mut cur_y = list_top - block_scroll;

                if !available.is_empty() {
                    // "Select monster to block:" header
                    if cur_y + compact_h >= list_top && cur_y <= list_top + list_h {
                        self.draw_text_sharp(
                            "Select monster to block:",
                            content_x + 10.0 * s,
                            cur_y + compact_h * 0.7,
                            16.0,
                            TEXT_TITLE,
                        );
                    }
                    cur_y += compact_h + compact_sp;

                    let mut alt = false;
                    for (orig_idx, (_monster_id, monster_name)) in &available {
                        if cur_y + compact_h >= list_top && cur_y <= list_top + list_h {
                            let is_selected =
                                state.ui_state.slayer_selected_block_monster == Some(*orig_idx);

                            let row_bg = if is_selected {
                                Color::new(0.15, 0.18, 0.12, 0.9)
                            } else if alt {
                                Color::new(0.06, 0.06, 0.08, 0.6)
                            } else {
                                Color::new(0.08, 0.08, 0.10, 0.6)
                            };
                            draw_rectangle(content_x + 2.0, cur_y, draw_w - 4.0, compact_h, row_bg);

                            if is_selected {
                                draw_rectangle_lines(
                                    content_x + 2.0,
                                    cur_y,
                                    draw_w - 4.0,
                                    compact_h,
                                    1.0,
                                    FRAME_ACCENT,
                                );
                            }

                            let name_color = if is_selected { TEXT_GOLD } else { TEXT_NORMAL };
                            self.draw_text_sharp(
                                monster_name,
                                content_x + 10.0 * s,
                                cur_y + compact_h * 0.7,
                                16.0,
                                name_color,
                            );

                            layout.add(
                                UiElementId::SlayerBlockMonsterSelect(*orig_idx),
                                Rect::new(content_x + 2.0, cur_y, draw_w - 4.0, compact_h),
                            );
                        }

                        alt = !alt;
                        cur_y += compact_h + compact_sp;
                    }

                    cur_y += 4.0 * s;
                }

                // "Currently Blocked:" header
                if cur_y + compact_h >= list_top && cur_y <= list_top + list_h {
                    self.draw_text_sharp(
                        "Currently Blocked:",
                        content_x + 10.0 * s,
                        cur_y + compact_h * 0.7,
                        16.0,
                        TEXT_TITLE,
                    );
                }
                cur_y += compact_h + compact_sp;

                if state.ui_state.slayer_blocked_monsters.is_empty() {
                    if cur_y + compact_h >= list_top && cur_y <= list_top + list_h {
                        self.draw_text_sharp(
                            "No blocked monsters",
                            content_x + 10.0 * s,
                            cur_y + compact_h * 0.7,
                            16.0,
                            TEXT_DIM,
                        );
                    }
                } else {
                    let mut alt = false;
                    for (i, monster_name) in
                        state.ui_state.slayer_blocked_monsters.iter().enumerate()
                    {
                        if cur_y + compact_h >= list_top && cur_y <= list_top + list_h {
                            let row_bg = if alt {
                                Color::new(0.06, 0.06, 0.08, 0.6)
                            } else {
                                Color::new(0.08, 0.08, 0.10, 0.6)
                            };
                            draw_rectangle(content_x + 2.0, cur_y, draw_w - 4.0, compact_h, row_bg);

                            self.draw_text_sharp(
                                monster_name,
                                content_x + 10.0 * s,
                                cur_y + compact_h * 0.7,
                                16.0,
                                TEXT_NORMAL,
                            );

                            // Compact remove button
                            let remove_w = 56.0 * s;
                            let remove_h = 18.0 * s;
                            let remove_x = content_x + draw_w - remove_w - 6.0 * s;
                            let remove_y = cur_y + (compact_h - remove_h) / 2.0;
                            layout.add(
                                UiElementId::SlayerRemoveBlock(i),
                                Rect::new(remove_x, remove_y, remove_w, remove_h),
                            );

                            let is_remove_hovered = matches!(
                                hovered,
                                Some(UiElementId::SlayerRemoveBlock(idx)) if *idx == i
                            );

                            let (remove_bg, remove_border) = if is_remove_hovered {
                                (
                                    Color::new(0.5, 0.15, 0.15, 1.0),
                                    Color::new(0.7, 0.25, 0.25, 1.0),
                                )
                            } else {
                                (
                                    Color::new(0.35, 0.1, 0.1, 1.0),
                                    Color::new(0.5, 0.18, 0.18, 1.0),
                                )
                            };

                            draw_rectangle(remove_x, remove_y, remove_w, remove_h, remove_border);
                            draw_rectangle(
                                remove_x + 1.0,
                                remove_y + 1.0,
                                remove_w - 2.0,
                                remove_h - 2.0,
                                remove_bg,
                            );

                            let remove_text = "Remove";
                            let remove_text_color = if is_remove_hovered {
                                WHITE
                            } else {
                                TEXT_NORMAL
                            };
                            let remove_dims = self.measure_text_sharp(remove_text, 14.0);
                            self.draw_text_sharp(
                                remove_text,
                                remove_x + (remove_w - remove_dims.width) / 2.0,
                                remove_y + remove_h * 0.72,
                                14.0,
                                remove_text_color,
                            );
                        }

                        alt = !alt;
                        cur_y += compact_h + compact_sp;
                    }
                }

                // Flush list draws so they respect the inner scissor,
                // then restore outer scissor for the scrollbar
                {
                    let mut gl = unsafe { get_internal_gl() };
                    gl.flush();
                }
                unsafe {
                    miniquad::gl::glScissor(
                        clip_x,
                        real_sh as i32 - clip_y - clip_h,
                        clip_w,
                        clip_h,
                    );
                }

                // Draw scrollbar if content overflows
                if needs_scrollbar {
                    let track_h = list_h - 4.0 * s;
                    let track_x = content_x + list_w - scrollbar_w - 2.0 * s;
                    let track_y = list_top + 2.0 * s;

                    // Track background
                    draw_rectangle(
                        track_x,
                        track_y,
                        scrollbar_w,
                        track_h,
                        Color::new(0.1, 0.1, 0.13, 1.0),
                    );

                    // Thumb
                    let visible_ratio = (list_h / total_h).min(1.0);
                    let thumb_h = (track_h * visible_ratio).max(16.0 * s);
                    let scroll_ratio = if max_scroll > 0.0 {
                        block_scroll / max_scroll
                    } else {
                        0.0
                    };
                    let thumb_y = track_y + scroll_ratio * (track_h - thumb_h);

                    let is_dragging = state.ui_state.slayer_block_scroll_drag.dragging;
                    let is_sb_hovered = matches!(hovered, Some(UiElementId::SlayerBlockScrollbar));
                    let thumb_color = if is_dragging || is_sb_hovered {
                        FRAME_ACCENT
                    } else {
                        FRAME_MID
                    };
                    draw_rectangle(track_x, thumb_y, scrollbar_w, thumb_h, thumb_color);

                    layout.add_scrollbar(
                        UiElementId::SlayerBlockScrollbar,
                        Rect::new(track_x, track_y, scrollbar_w, track_h),
                    );
                }
            }
        }

        // Disable scissor test
        unsafe {
            miniquad::gl::glDisable(miniquad::gl::GL_SCISSOR_TEST);
        }
    }
}
