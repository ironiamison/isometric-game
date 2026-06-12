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
            let tab_bar_y = clip_y - tab_h;
            let clip_x = chat_x - bg_padding;
            let tab_rects = self.chat_tab_rects(clip_x + 2.0, tab_bar_y, tab_h);

            let tab_ids = [
                UiElementId::ChatTabLocal,
                UiElementId::ChatTabGlobal,
                UiElementId::ChatTabSystem,
            ];
            for (i, tab_id) in tab_ids.iter().enumerate() {
                layout.add(tab_id.clone(), tab_rects[i]);
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
            let contract_y = stats_y
                + 3.0 * (18.0 + 4.0) * s
                + 14.0 * s
                + slayer_offset
                + self.hud_below_bars_offset();
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
                    + self.hud_below_bars_offset()
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

    /// Simplified cluster frame for the HUD portrait/stats panel. Mirrors the menu
    /// button treatment (1px outer border + inner shadow) over a chat-box style
    /// translucent fill, rather than the heavier multi-layer bronze `draw_panel_frame`.
    pub(crate) fn draw_hud_cluster_frame(&self, x: f32, y: f32, w: f32, h: f32) {
        // Outer border with slight transparency (matches menu buttons).
        let border = Color::new(SLOT_BORDER.r, SLOT_BORDER.g, SLOT_BORDER.b, 0.9);
        draw_rectangle(x - 1.0, y - 1.0, w + 2.0, h + 2.0, border);

        // Semi-transparent background (matches the chat box fill).
        draw_rectangle(x, y, w, h, HUD_FILL_TRANSLUCENT);

        // Inner shadow on top/left edges for a touch of depth (matches menu buttons).
        draw_rectangle(x, y, w, 2.0, SLOT_INNER_SHADOW);
        draw_rectangle(x, y, 2.0, h, SLOT_INNER_SHADOW);
    }

    /// Draw text with a 1px black stroke for legibility over colored bars.
    pub(crate) fn draw_text_outlined(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        let outline = Color::new(0.0, 0.0, 0.0, 0.9);
        for ox in [-1.0, 1.0] {
            for oy in [-1.0, 1.0] {
                self.draw_text_sharp(text, x + ox, y + oy, size, outline);
            }
        }
        self.draw_text_sharp(text, x, y, size, color);
    }

    /// Draw a HUD stat bar: recessed background + two-tone fill (lighter MAIN tone on
    /// top, a thin darker band along the bottom). `border` lets the prayer bar pulse.
    pub(crate) fn draw_hud_stat_bar_fill(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        ratio: f32,
        border: Color,
        main: Color,
        dark: Color,
    ) {
        draw_rectangle(x, y, w, h, border);
        draw_rectangle(x + 1.0, y + 1.0, w - 2.0, h - 2.0, Color::new(0.08, 0.08, 0.10, 1.0));
        let fill_w = (w - 4.0) * ratio;
        if fill_w > 0.0 {
            // Lighter main tone fills the whole inner height...
            draw_rectangle(x + 2.0, y + 2.0, fill_w, h - 4.0, main);
            // ...with a thin darker band along the bottom edge.
            let dark_h = ((h - 4.0) * 0.34).max(2.0).floor();
            draw_rectangle(x + 2.0, y + h - 2.0 - dark_h, fill_w, dark_h, dark);
        }
    }

    /// Draw a live head portrait of the player (base head + hair), cropped to the head
    /// and rendered at the UI scale (pixel-perfect) inside the gold portrait box. Falls
    /// back to the generic silhouette if the player's base sprite isn't loaded.
    pub(crate) fn draw_player_head_portrait(&self, player: &Player, x: f32, y: f32, size: f32) {
        // No frame — the head sits directly on the cluster's panel fill (keeps the
        // cluster compact). Base player sprite (front idle); bail to silhouette if missing.
        let key = format!("{}_{}", player.gender, player.skin);
        let Some((base_tex, base_off)) = self.player_sprites.get(&key) else {
            self.draw_hud_portrait(x, y, size);
            return;
        };
        let (patlas_x, patlas_y) = base_off.unwrap_or((0.0, 0.0));

        // Front-facing sprite-sheet dimensions.
        const SW: f32 = 34.0;
        const SH: f32 = 78.0;
        const HAIRW: f32 = 28.0;
        const HAIRH: f32 = 54.0;

        let inner = size;
        let ix = x;
        let iy = y;

        // Draw pixel art at an INTEGER magnification (per the codebase convention for
        // pixel-art icons) so texels stay uniform — fractional UI scales otherwise make
        // the sprite look slightly stretched. Center horizontally on the sprite and crop
        // lower (sprite y ~13) so the head fills the square.
        let scale = self.font_scale.get().round().max(1.0);
        // Hair sprites sit slightly left of center on the head; when the player has hair
        // (i.e. is not bald), nudge the whole portrait (base + hair) right by 2px so the
        // styled head reads centered in the box. A bald head has no hair occupying the top
        // rows, so raise it a few px to better fill the box.
        let hair_nudge = if player.hair_style.is_some() { 2.0 * scale } else { 0.0 };
        let bald_rise = if player.hair_style.is_none() { 3.0 * scale } else { 0.0 };
        // Female sprites carry the head a touch lower than males, so raise them in the box
        // (equivalently, crop a few rows lower) to show more of the face.
        let female_rise = if player.gender == "female" { 3.0 * scale } else { 0.0 };
        let ox = (ix + inner / 2.0 - 17.0 * scale + hair_nudge).floor();
        let oy = (iy + inner / 2.0 - 13.0 * scale - bald_rise - female_rise).floor();

        // Clip to the recessed interior so only the head shows.
        let (vw, vh) = virtual_screen_size();
        let sx = macroquad::window::screen_width() / vw;
        let sy = macroquad::window::screen_height() / vh;
        {
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            gl.quad_gl.scissor(Some((
                (ix * sx) as i32,
                (iy * sy) as i32,
                (inner * sx) as i32,
                (inner * sy) as i32,
            )));
        }

        // 1. Base body (head shows at the top).
        draw_texture_ex(
            base_tex,
            ox,
            oy,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(patlas_x, patlas_y, SW, SH)),
                dest_size: Some(Vec2::new(SW * scale, SH * scale)),
                ..Default::default()
            },
        );

        // 2. Hair (front frame = color * 2).
        if let Some(style) = player.hair_style {
            let hair_key = format!("{}_{}", player.gender, style);
            if let Some((hair_tex, hair_off)) = self.hair_sprites.get(&hair_key) {
                let (hatlas_x, hatlas_y) = hair_off.unwrap_or((0.0, 0.0));
                let color = player.hair_color.unwrap_or(0).max(0);
                let hair_src_x = hatlas_x + (color * 2) as f32 * HAIRW;
                draw_texture_ex(
                    hair_tex,
                    ox + 2.0 * scale,
                    oy - 3.0 * scale,
                    WHITE,
                    DrawTextureParams {
                        source: Some(Rect::new(hair_src_x, hatlas_y, HAIRW, HAIRH)),
                        dest_size: Some(Vec2::new(HAIRW * scale, HAIRH * scale)),
                        ..Default::default()
                    },
                );
            }
        }

        // Clear the clip.
        {
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            gl.quad_gl.scissor(None);
        }
    }

    /// Draw the fallback HUD portrait: a simple person silhouette (no frame), used when
    /// the player's base sprite isn't loaded. Matches the borderless real-head portrait.
    pub(crate) fn draw_hud_portrait(&self, x: f32, y: f32, size: f32) {
        // Person silhouette (head + shoulders) in a soft steel-blue.
        let c = Color::new(0.52, 0.60, 0.72, 1.0);
        let cx = x + size / 2.0;
        let head_r = size * 0.15;
        draw_circle(cx, y + size * 0.37, head_r, c);
        let by = y + size * 0.52;
        let bottom = y + size * 0.80;
        let top_half = size * 0.16;
        let bot_half = size * 0.30;
        draw_triangle(
            Vec2::new(cx - bot_half, bottom),
            Vec2::new(cx + bot_half, bottom),
            Vec2::new(cx - top_half, by),
            c,
        );
        draw_triangle(
            Vec2::new(cx + bot_half, bottom),
            Vec2::new(cx - top_half, by),
            Vec2::new(cx + top_half, by),
            c,
        );
    }

    /// Heart glyph (HP icon) drawn at center (cx, cy).
    pub(crate) fn draw_hud_heart(&self, cx: f32, cy: f32, r: f32, color: Color) {
        draw_circle(cx - r * 0.45, cy - r * 0.25, r * 0.55, color);
        draw_circle(cx + r * 0.45, cy - r * 0.25, r * 0.55, color);
        draw_triangle(
            Vec2::new(cx - r, cy + r * 0.02),
            Vec2::new(cx + r, cy + r * 0.02),
            Vec2::new(cx, cy + r * 1.05),
            color,
        );
    }

    /// Flask/potion glyph (MP icon) drawn at center (cx, cy).
    pub(crate) fn draw_hud_flask(&self, cx: f32, cy: f32, r: f32, color: Color) {
        // Neck.
        draw_rectangle(cx - r * 0.28, cy - r * 0.95, r * 0.56, r * 0.7, color);
        // Body.
        draw_circle(cx, cy + r * 0.3, r * 0.75, color);
        // Cork.
        draw_rectangle(
            cx - r * 0.34,
            cy - r * 1.15,
            r * 0.68,
            r * 0.28,
            Color::new(color.r * 0.7, color.g * 0.7, color.b * 0.7, color.a),
        );
    }

    /// Draw a 16x16 HUD stat icon centered at (cx, cy), at integer UI scale (crisp).
    pub(crate) fn draw_hud_stat_icon(&self, tex: &Texture2D, cx: f32, cy: f32) {
        let scale = self.font_scale.get().round().max(1.0);
        let size = 16.0 * scale;
        draw_texture_ex(
            tex,
            (cx - size / 2.0).floor(),
            (cy - size / 2.0).floor(),
            WHITE,
            DrawTextureParams {
                dest_size: Some(Vec2::new(size, size)),
                ..Default::default()
            },
        );
    }

    /// Lightning-bolt glyph (Prayer icon) drawn at center (cx, cy).
    pub(crate) fn draw_hud_bolt(&self, cx: f32, cy: f32, r: f32, color: Color) {
        draw_triangle(
            Vec2::new(cx + r * 0.25, cy - r),
            Vec2::new(cx - r * 0.55, cy + r * 0.15),
            Vec2::new(cx + r * 0.05, cy + r * 0.1),
            color,
        );
        draw_triangle(
            Vec2::new(cx - r * 0.05, cy - r * 0.1),
            Vec2::new(cx + r * 0.55, cy - r * 0.15),
            Vec2::new(cx - r * 0.25, cy + r),
            color,
        );
    }

    /// Draw a shared over-world HUD container: a clean 2px bronze bevel + the shared
    /// translucent navy fill (no gold corner brackets — distinguishable but not flashy).
    /// Used by the chat backing and both bottom bars so they read as one family with the
    /// framed panels while staying lighter over the world.
    /// `solid`: bottom bars pass `true` (near-opaque, cohesive); the chat box passes
    /// `false` (translucent, world shows through).
    pub(crate) fn draw_hud_tray(&self, x: f32, y: f32, w: f32, h: f32, solid: bool) {
        let fill = if solid {
            HUD_FILL_SOLID
        } else {
            HUD_FILL_TRANSLUCENT
        };
        draw_rectangle(x, y, w, h, fill);
    }

    /// Compact, left-packed chat tab rects (Public / Global / System), each sized to its
    /// own label so they fill only the top-left instead of stretching. Shared by the
    /// renderer (draw) and the input layout (hit-test) so they stay in sync.
    pub(crate) fn chat_tab_rects(&self, chat_x: f32, tab_bar_y: f32, tab_h: f32) -> [Rect; 3] {
        let s = self.font_scale.get();
        let names = ["Public", "Global", "System"];
        let pad = 12.0 * s;
        let gap = 4.0 * s;
        let mut x = chat_x;
        let mut out = [Rect::new(0.0, 0.0, 0.0, 0.0); 3];
        for (i, name) in names.iter().enumerate() {
            let tw = self.measure_text_sharp(name, 16.0).width;
            let w = (tw + pad * 2.0).floor();
            out[i] = Rect::new(x.floor(), tab_bar_y, w, tab_h);
            x += w + gap;
        }
        out
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
                let shine_width = (fill_width * 0.3).clamp(2.0, 6.0).floor();
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
