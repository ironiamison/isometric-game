use super::*;

impl Renderer {
    pub(super) fn render_interactive_ui(&self, state: &GameState) -> UiLayout {
        let mut layout = UiLayout::new();
        let hovered = &state.ui_state.hovered_element;

        // Ground item clickable areas and hover labels (world-space, registered first)
        self.render_ground_item_overlays(state, hovered, &mut layout);

        // Quest objective tracker / contract tracker below minimap on the right side.
        // Rendered early so interactive panels (inventory, quest log, etc.) draw on top.
        let preview = self.minimap_preview_rect();
        let s = state.ui_state.ui_scale;
        let preview_enabled = self.minimap_preview_enabled(state);
        let header_font_size = 16.0;
        let scaled_header_size = self.scaled_font_size(header_font_size);
        let extra_gap = (scaled_header_size - header_font_size).max(0.0) * 0.5;
        let tracker_right = (preview.x + preview.w).floor();
        let base_gap = if preview_enabled { 16.0 } else { 14.0 };
        let tracker_gap = base_gap * s + extra_gap;
        let tracker_y = if preview_enabled {
            (preview.y + preview.h + tracker_gap).floor()
        } else {
            (preview.y + tracker_gap).floor()
        };
        let tracker_width = (preview.w + 88.0).max(120.0).min(tracker_right - 10.0);
        let tracker_x = (tracker_right - tracker_width).floor();
        state
            .ui_state
            .quest_tracker_rect
            .set(self.render_quest_tracker(state, tracker_x, tracker_y, tracker_width));

        // Inventory UI (when open)
        if state.ui_state.inventory_open {
            self.render_inventory(state, hovered, &mut layout);
        }

        // Quest Log UI (when open)
        if state.ui_state.quest_log_open {
            self.render_quest_log(state, hovered, &mut layout);
        }

        // Crafting UI (when open)
        if state.ui_state.crafting_open {
            self.render_crafting(state, hovered, &mut layout);
        }

        // Furnace UI (when open)
        if state.ui_state.furnace_open {
            self.render_furnace(state, hovered, &mut layout);
        }

        // Anvil UI (when open)
        if state.ui_state.anvil_open {
            self.render_anvil(state, hovered, &mut layout);
        }

        // Alchemy Station UI (when open)
        if state.ui_state.alchemy_station_open {
            self.render_alchemy_station(state, hovered, &mut layout);
        }

        // Workbench UI (when open)
        if state.ui_state.workbench_open {
            self.render_workbench(state, hovered, &mut layout);
        }

        // Fletching panel (when open)
        if state.ui_state.fletching_open {
            self.render_fletching(state, hovered, &mut layout);
        }

        // Bank UI (when open)
        if state.ui_state.bank_open {
            self.render_bank(state, hovered, &mut layout);
            if let Some(ref dialog) = state.ui_state.bank_quantity_dialog {
                self.render_bank_quantity_dialog(dialog, hovered, &mut layout);
            }
            if state.ui_state.bank_help_open {
                self.render_bank_help_overlay(hovered, &mut layout);
            }
        }

        // Skills panel (when open)
        self.render_skills_panel(state, hovered, &mut layout);

        // Prayer book panel (when open)
        self.render_prayer_panel(state, hovered, &mut layout);

        // Gathering buff timer indicator
        self.render_gathering_buff(state);

        // Character panel (when open)
        self.render_character_panel(state, hovered, &mut layout);

        // Social panel (when open)
        self.render_social_panel(state, hovered, &mut layout);

        // Settings panel (when open)
        if state.ui_state.escape_menu_open {
            self.render_escape_menu(state, &mut layout);
        }

        // Chat log area is intentionally NOT registered for hit detection
        // so that clicks/hovers pass through to the game world beneath it.
        // However, the tab bar above the chat log IS registered for click handling.
        if state.ui_state.chat_log_visible {
            let scale = state.ui_state.ui_scale;
            let chat_x = 10.0;
            let (_, chat_sh) = virtual_screen_size();
            let bg_padding = 6.0 * scale;
            let line_height = 18.0 * scale;
            let max_chat_width = if scale >= 2.0 {
                400.0 * scale - 260.0
            } else {
                360.0 * scale
            };
            let max_visible_lines: usize = if scale >= 2.0 { 6 } else { 7 };
            let chat_area_h = max_visible_lines as f32 * line_height;
            let bg_bottom = chat_sh - EXP_BAR_GAP * scale;
            let clip_h = chat_area_h + bg_padding * 2.0;
            let clip_y = bg_bottom - clip_h;

            let tab_h = 18.0 * scale;
            let num_tabs = 3.0f32;
            let tab_w = (max_chat_width / num_tabs).floor();
            let tab_bar_y = clip_y - tab_h;

            let tab_ids = [
                UiElementId::ChatTabLocal,
                UiElementId::ChatTabGlobal,
                UiElementId::ChatTabSystem,
            ];
            for i in 0..3 {
                let tx = chat_x + i as f32 * tab_w;
                layout.add(
                    tab_ids[i].clone(),
                    macroquad::prelude::Rect::new(tx, tab_bar_y, tab_w, tab_h),
                );
            }

            // Register scrollbar for chat log drag interaction
            // Compute max_scroll from cached line count
            let total_lines = self.chat_lines_cache.borrow().lines.len();
            let total_content_height = total_lines as f32 * line_height;
            let max_scroll_px = (total_content_height - chat_area_h).max(0.0);
            if max_scroll_px > 0.0 {
                let scrollbar_w = 6.0 * scale;
                let scrollbar_x = chat_x + max_chat_width + bg_padding * 2.0 - scrollbar_w;
                let track_y = clip_y;
                let track_h = clip_h;
                layout.add_scrollbar(
                    UiElementId::ChatLogScrollbar,
                    macroquad::prelude::Rect::new(scrollbar_x, track_y, scrollbar_w, track_h),
                );
                layout.set_max_scroll(UiElementId::ChatLogScrollbar, max_scroll_px);
            }
        }

        // Hide bottom bar on mobile for full-screen skill/bank panels (not menu button panels)
        let hide_bottom_bar = cfg!(target_os = "android")
            && (state.ui_state.crafting_open
                || state.ui_state.furnace_open
                || state.ui_state.anvil_open
                || state.ui_state.fletching_open
                || state.ui_state.bank_open
                || state.ui_state.chest_open
                || state.ui_state.shop_data.is_some()
                || state.ui_state.active_dialogue.is_some());
        if !hide_bottom_bar {
            // Quick slots (always visible at bottom, above exp bar)
            self.render_quick_slots(state, hovered, &mut layout);

            // Menu buttons (bottom-right, above exp bar)
            self.render_menu_buttons(state, hovered, &mut layout);
        }

        // Chat button — on desktop rendered elsewhere; on Android it's in the collapsible menu bar

        // Resource contract tracker - left side below stat bars
        if state.resource_contract.is_some() {
            let s = self.font_scale.get();
            let bar_width_contract = 120.0f32;
            let (bar_x, stats_y) = self.minimap_stats_stack_position(state, bar_width_contract);
            let slayer_offset = if state.ui_state.slayer_current_task.is_some() {
                46.0 * s
            } else {
                0.0
            };
            let contract_y = stats_y + 3.0 * (18.0 + 4.0) * s + 14.0 * s + slayer_offset;
            self.render_resource_contract_tracker(state, bar_x, contract_y, 240.0);
        }

        // Slayer chip hover tooltip (rendered here so it draws on top of contract tracker)
        if state.ui_state.slayer_current_task.is_some() {
            if let Some(player) = state.get_local_player() {
                let s = self.font_scale.get();
                let font_size = 16.0;
                let name = &player.name;
                let level_text = format!(" Lv.{}", player.skills.total_level());
                let name_w = self.measure_text_sharp(name, font_size).width;
                let level_w = self.measure_text_sharp(&level_text, font_size).width;
                let total_text_w = name_w + level_w;
                let padding = 6.0;
                let bar_width = (total_text_w + padding * 2.0).max(120.0 * s);
                let bar_height = 18.0 * s;
                let (bar_x, stats_y) = self.minimap_stats_stack_position(state, bar_width);
                let mp_bar_y = stats_y + (bar_height + 4.0 * s);
                let prayer_bar_y = mp_bar_y + bar_height + 4.0 * s;
                let current_time = macroquad::time::get_time();
                let is_skilling = state.is_gathering || state.is_woodcutting;
                let has_stall_bar = state.ui_state.stall_active;
                let has_dash_bar = state.dash_cooldown_end > current_time;
                let chip_y = prayer_bar_y
                    + bar_height
                    + 4.0 * s
                    + if is_skilling { 22.0 * s + 4.0 * s } else { 0.0 }
                    + if has_stall_bar {
                        22.0 * s + 4.0 * s
                    } else {
                        0.0
                    }
                    + if has_dash_bar {
                        22.0 * s + 4.0 * s
                    } else {
                        0.0
                    };
                // Slayer chip is now first (leftmost), so tooltip x = bar_x
                self.render_slayer_task_chip_tooltip(state, bar_x, chip_y);

                // Potion buff chip tooltips (positioned after slayer chip, before combat style)
                if !state.active_potion_buffs.is_empty() {
                    let chip_gap = 4.0 * s;
                    let mut tooltip_cursor_x = bar_x;
                    // Skip past slayer chip
                    let (sw, _) = self.render_slayer_task_chip(state, -10000.0, -10000.0);
                    if sw > 0.0 {
                        tooltip_cursor_x += sw + chip_gap;
                    }
                    // Iterate buff chips
                    for buff in &state.active_potion_buffs {
                        let (bw, bh) =
                            self.render_potion_buff_chip(state, buff, -10000.0, -10000.0);
                        if bw > 0.0 {
                            self.render_potion_buff_chip_tooltip(
                                state,
                                buff,
                                tooltip_cursor_x,
                                chip_y,
                                bw,
                                bh,
                            );
                            tooltip_cursor_x += bw + chip_gap;
                        }
                    }
                }
            }
        }

        // Dialogue box (when active)
        if let Some(dialogue) = &state.ui_state.active_dialogue {
            self.render_dialogue(
                state,
                dialogue,
                hovered,
                &mut layout,
                state.ui_state.dialogue_scroll_offset,
                state.ui_state.dialogue_scroll_drag.dragging,
            );
        }

        // Altar offering panel (when active)
        if let Some(ref panel) = state.ui_state.altar_panel {
            self.render_altar_panel(panel, state, hovered, &mut layout);
        }

        // Chest panel (when open)
        if state.ui_state.chest_open {
            self.render_chest_panel(state, hovered, &mut layout);
        }

        // Slayer panel (when open)
        if state.ui_state.slayer_panel_open {
            self.render_slayer_panel(state, hovered, &mut layout);
        }

        // Trade panel and request popup
        self.render_trade_panel(state, &mut layout);
        self.render_trade_request_popup(state, &mut layout);

        // Collection Log popup (renders over everything)
        if state.ui_state.collection_log_open {
            self.render_collection_log(state, hovered, &mut layout);
        }

        // Stall panels
        self.render_stall_setup_panel(state, &mut layout);
        self.render_stall_browse_panel(state, &mut layout);

        // Gold drop dialog (when active) - rendered after trade/stall so it appears on top
        if let Some(ref dialog) = state.ui_state.gold_drop_dialog {
            self.render_gold_drop_dialog(
                dialog,
                state.inventory.gold,
                state.ui_state.trade_open,
                hovered,
                &mut layout,
            );
        }

        // Stall price dialog (when active)
        if let Some(ref dialog) = state.ui_state.stall_price_dialog {
            self.render_stall_price_dialog(dialog, hovered, &mut layout);
        }

        // KOTH HUD (always visible during KOTH), checkpoint dialog, and game over
        self.render_koth_hud(state);
        self.render_koth_checkpoint(state, hovered, &mut layout);
        self.render_koth_game_over(state, hovered, &mut layout);

        // Boss fight HUD
        self.render_boss_hud(state);

        // Quest completion notifications (on top of dialogue/panels)
        self.render_quest_completed(state);

        // Prayer/Spell help overlay (on top of panels)
        self.render_prayer_help_overlay(state, hovered, &mut layout);

        // Minimap interactions and expanded map overlay
        if !cfg!(target_os = "android") {
            self.render_minimap_overlay(state, hovered, &mut layout);
        }

        // Render context menu on top of everything (except modal minimap)
        if state.ui_state.minimap_panel_open {
            // Minimap panel is modal; suppress other hover/context overlays.
        } else if let Some(ref context_menu) = state.ui_state.context_menu {
            self.render_context_menu(context_menu, state, &mut layout);
        } else {
            // Only render tooltips if context menu is not open
            self.render_item_tooltip(state);
            self.render_skill_tooltip(state, hovered);
            self.render_prayer_tooltip(state, hovered);

            // XP globe tooltip (calculate position to match render_ui exactly)
            if state.get_local_player().is_some() {
                let preview = self.minimap_preview_rect();
                let globe_anchor_x = preview.x;
                let globe_stats_y = preview.y + 20.0;
                self.render_xp_globe_tooltip(&state.xp_globes, globe_anchor_x, globe_stats_y);
            }
        }

        // Render dragged item at cursor (on top of everything)
        if let Some(ref drag) = state.ui_state.drag_state {
            self.render_dragged_item(drag, state);
        }

        // Chat panel (fullscreen overlay, on top of everything)
        self.render_chat_panel(state, hovered, &mut layout);

        layout
    }

    // ========================================================================
    // Inventory UI Helper Functions
    // ========================================================================

    /// Draw the multi-layer medieval panel frame
    pub(crate) fn draw_panel_frame(&self, x: f32, y: f32, w: f32, h: f32) {
        // Layer 1: Outer dark shadow (gives depth from background)
        draw_rectangle(x - 2.0, y - 2.0, w + 4.0, h + 4.0, PANEL_BG_DARK);

        // Layer 2: Dark bronze outer frame
        draw_rectangle(x, y, w, h, FRAME_OUTER);

        // Layer 3: Mid bronze frame (inset 2px)
        draw_rectangle(x + 2.0, y + 2.0, w - 4.0, h - 4.0, FRAME_MID);

        // Layer 4: Main panel background (inset 4px)
        draw_rectangle(
            x + FRAME_THICKNESS,
            y + FRAME_THICKNESS,
            w - FRAME_THICKNESS * 2.0,
            h - FRAME_THICKNESS * 2.0,
            PANEL_BG_MID,
        );

        // Layer 5: Inner highlight line (top and left edges - light source simulation)
        draw_line(
            x + FRAME_THICKNESS,
            y + FRAME_THICKNESS,
            x + w - FRAME_THICKNESS,
            y + FRAME_THICKNESS,
            1.0,
            FRAME_INNER,
        );
        draw_line(
            x + FRAME_THICKNESS,
            y + FRAME_THICKNESS,
            x + FRAME_THICKNESS,
            y + h - FRAME_THICKNESS,
            1.0,
            FRAME_INNER,
        );

        // Layer 6: Inner shadow line (bottom and right edges)
        let shadow = Color::new(0.0, 0.0, 0.0, 0.235);
        draw_line(
            x + FRAME_THICKNESS + 1.0,
            y + h - FRAME_THICKNESS - 1.0,
            x + w - FRAME_THICKNESS,
            y + h - FRAME_THICKNESS - 1.0,
            1.0,
            shadow,
        );
        draw_line(
            x + w - FRAME_THICKNESS - 1.0,
            y + FRAME_THICKNESS + 1.0,
            x + w - FRAME_THICKNESS - 1.0,
            y + h - FRAME_THICKNESS,
            1.0,
            shadow,
        );
    }

    /// Draw decorative corner accents (gold L-shapes at corners)
    pub(crate) fn draw_corner_accents(&self, x: f32, y: f32, w: f32, h: f32) {
        let size = CORNER_ACCENT_SIZE;

        // Top-left corner
        draw_rectangle(x, y, size, 2.0, FRAME_ACCENT);
        draw_rectangle(x, y, 2.0, size, FRAME_ACCENT);

        // Top-right corner
        draw_rectangle(x + w - size, y, size, 2.0, FRAME_ACCENT);
        draw_rectangle(x + w - 2.0, y, 2.0, size, FRAME_ACCENT);

        // Bottom-left corner
        draw_rectangle(x, y + h - 2.0, size, 2.0, FRAME_ACCENT);
        draw_rectangle(x, y + h - size, 2.0, size, FRAME_ACCENT);

        // Bottom-right corner
        draw_rectangle(x + w - size, y + h - 2.0, size, 2.0, FRAME_ACCENT);
        draw_rectangle(x + w - 2.0, y + h - size, 2.0, size, FRAME_ACCENT);
    }

    /// Draw a slim medieval-style health bar above entities
    ///
    /// Creates a polished health bar with:
    /// - Thin 1px dark border with rounded corners
    /// - Jewel-toned health fill with gradient effect
    pub(super) fn draw_entity_health_bar(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        hp_ratio: f32,
        _scale: f32,
    ) {
        // Pixel-align coordinates for crisp rendering
        let x = x.floor();
        let y = y.floor();
        let width = width.floor();
        let height = height.floor();

        // Border color - deep purple-gray for contrast
        let border_color = Color::new(0.18, 0.16, 0.22, 1.0); // rgba(46, 41, 56, 255)

        // === 1px BORDER with rounded corners ===
        // Top edge (inset 1px from corners)
        draw_rectangle(x, y - 1.0, width, 1.0, border_color);
        // Bottom edge (inset 1px from corners)
        draw_rectangle(x, y + height, width, 1.0, border_color);
        // Left edge (inset 1px from corners)
        draw_rectangle(x - 1.0, y, 1.0, height, border_color);
        // Right edge (inset 1px from corners)
        draw_rectangle(x + width, y, 1.0, height, border_color);

        // === INNER BACKGROUND (Dark recessed look) ===
        draw_rectangle(x, y, width, height, HEALTHBAR_BG_OUTER);

        // === HEALTH FILL ===
        if hp_ratio > 0.0 {
            // Select colors based on health level (jewel tones)
            let (color_dark, color_mid, color_light) = if hp_ratio > 0.5 {
                (HEALTH_GREEN_DARK, HEALTH_GREEN_MID, HEALTH_GREEN_LIGHT)
            } else if hp_ratio > 0.25 {
                (HEALTH_YELLOW_DARK, HEALTH_YELLOW_MID, HEALTH_YELLOW_LIGHT)
            } else {
                (HEALTH_RED_DARK, HEALTH_RED_MID, HEALTH_RED_LIGHT)
            };

            let fill_width = (width * hp_ratio).max(1.0).floor();

            // Base fill (darker tone)
            draw_rectangle(x, y, fill_width, height, color_dark);

            // Mid gradient (main color)
            if height > 2.0 {
                draw_rectangle(x, y + 1.0, fill_width, height - 2.0, color_mid);
            }

            // Top highlight (bright shine effect)
            if height > 3.0 {
                let highlight_height = (height * 0.35).max(1.0).floor();
                draw_rectangle(x, y + 1.0, fill_width, highlight_height, color_light);
            }

            // Specular shine (small white gleam)
            if fill_width > 4.0 && height > 2.0 {
                let shine_width = (fill_width * 0.3).min(6.0).max(2.0).floor();
                let shine_color = Color::new(1.0, 1.0, 1.0, 0.4);
                draw_rectangle(x + 1.0, y + 1.0, shine_width, 1.0, shine_color);
            }
        }
    }

    /// Draw an inventory slot with bevel effect
    pub(crate) fn draw_inventory_slot(
        &self,
        x: f32,
        y: f32,
        size: f32,
        has_item: bool,
        state: SlotState,
    ) {
        // Outer slot border (bronze)
        draw_rectangle(x, y, size, size, SLOT_BORDER);

        // Inner recessed area (1px inset)
        let inner_x = x + 1.0;
        let inner_y = y + 1.0;
        let inner_size = size - 2.0;

        // Background based on state
        let bg = match state {
            SlotState::Normal => {
                if has_item {
                    SLOT_BG_FILLED
                } else {
                    SLOT_BG_EMPTY
                }
            }
            SlotState::Hovered => SLOT_HOVER_BG,
            SlotState::Dragging => SLOT_DRAG_SOURCE,
        };
        draw_rectangle(inner_x, inner_y, inner_size, inner_size, bg);

        // Inner shadow (top and left - simulates recessed slot)
        draw_line(
            inner_x,
            inner_y,
            inner_x + inner_size,
            inner_y,
            2.0,
            SLOT_INNER_SHADOW,
        );
        draw_line(
            inner_x,
            inner_y,
            inner_x,
            inner_y + inner_size,
            2.0,
            SLOT_INNER_SHADOW,
        );

        // Inner highlight (bottom and right - subtle)
        draw_line(
            inner_x + 1.0,
            inner_y + inner_size - 1.0,
            inner_x + inner_size,
            inner_y + inner_size - 1.0,
            1.0,
            SLOT_HIGHLIGHT,
        );
        draw_line(
            inner_x + inner_size - 1.0,
            inner_y + 1.0,
            inner_x + inner_size - 1.0,
            inner_y + inner_size,
            1.0,
            SLOT_HIGHLIGHT,
        );

        // State-specific border overlay
        match state {
            SlotState::Hovered => {
                draw_rectangle_lines(x, y, size, size, 2.0, SLOT_HOVER_BORDER);
            }
            SlotState::Dragging => {
                draw_rectangle_lines(x, y, size, size, 2.0, SLOT_SELECTED_BORDER);
            }
            _ => {}
        }
    }

    /// Draw an item icon using sprite or fallback color
    /// Uses the full texture, centered in the slot

    /// Render a dragged item following the cursor

    /// Word-wrap text to fit within a given width (approximate, assumes ~8px per char at size 16)
    /// Prefers breaking on word boundaries, but will break long words if necessary
    pub(crate) fn wrap_text(&self, text: &str, max_width: f32, font_size: f32) -> Vec<String> {
        let scaled_size = self.scaled_font_size(font_size);
        let width_key = (max_width * 100.0).round() as i32;
        let font_key = (scaled_size * 100.0).round() as i32;
        let bucket_key = (width_key, font_key);

        if let Some(bucket) = self.text_wrap_cache.borrow().get(&bucket_key) {
            if let Some(cached) = bucket.get(text) {
                return cached.clone();
            }
        }

        let wrapped = Self::wrap_text_uncached(text, max_width, scaled_size);
        let mut cache = self.text_wrap_cache.borrow_mut();
        let bucket = cache.entry(bucket_key).or_default();
        if bucket.len() < TEXT_WRAP_CACHE_BUCKET_LIMIT {
            bucket.insert(text.to_string(), wrapped.clone());
        }
        wrapped
    }

    pub(super) fn wrap_text_uncached(text: &str, max_width: f32, font_size: f32) -> Vec<String> {
        let char_width = font_size * 0.5; // Approximate character width
        let max_chars = (max_width / char_width) as usize;

        if max_chars == 0 {
            return vec![text.to_string()];
        }

        let mut lines = Vec::new();
        let mut current_line = String::new();

        // Helper to break a long word into chunks that fit
        let break_long_word = |word: &str, max_len: usize| -> Vec<String> {
            let chars: Vec<char> = word.chars().collect();
            chars
                .chunks(max_len)
                .map(|chunk| chunk.iter().collect())
                .collect()
        };

        for word in text.split_whitespace() {
            // If word itself is too long, break it up
            if word.chars().count() > max_chars {
                // First, push current line if not empty
                if !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = String::new();
                }
                // Break the long word into chunks
                let chunks = break_long_word(word, max_chars);
                for (i, chunk) in chunks.iter().enumerate() {
                    if i < chunks.len() - 1 {
                        lines.push(chunk.clone());
                    } else {
                        // Last chunk becomes the new current line
                        current_line = chunk.clone();
                    }
                }
            } else if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.chars().count() + 1 + word.chars().count() <= max_chars {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }

        if lines.is_empty() {
            lines.push(String::new());
        }

        lines
    }
}
