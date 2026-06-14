use super::*;

impl Renderer {
    pub(super) fn render_ui(&self, state: &GameState) {
        // Server announcements (top of screen)
        let current_time = macroquad::time::get_time();
        for (i, announcement) in state.ui_state.announcements.iter().enumerate() {
            let age = current_time - announcement.time;
            // Fade out after 6 seconds (announcements last 8 seconds total)
            let alpha = if age > 6.0 {
                ((8.0 - age) / 2.0 * 255.0) as u8
            } else {
                255
            };

            let font_size = 32.0;
            let text = format!("[ANNOUNCEMENT] {}", announcement.text);
            let text_dims = self.measure_text_sharp(&text, font_size);
            let (sw, _) = virtual_screen_size();
            let text_x = (sw - text_dims.width) / 2.0;
            let text_y = 50.0 + (i as f32 * 35.0);

            // Dark background for visibility
            let padding = 10.0;
            let rect_h = text_dims.height + padding;
            let rect_y = text_y - text_dims.offset_y - padding / 2.0;
            draw_rectangle(
                text_x - padding,
                rect_y,
                text_dims.width + padding * 2.0,
                rect_h,
                Color::from_rgba(0, 0, 0, (180.0 * alpha as f32 / 255.0) as u8),
            );

            // Gold text with black outline
            let gold_color = Color::from_rgba(255, 215, 0, alpha);
            for ox in [-1.0, 1.0] {
                for oy in [-1.0, 1.0] {
                    self.draw_text_sharp(
                        &text,
                        text_x + ox,
                        text_y + oy,
                        font_size,
                        Color::from_rgba(0, 0, 0, alpha),
                    );
                }
            }
            self.draw_text_sharp(&text, text_x, text_y, font_size, gold_color);
        }

        // "You Died" overlay for local player
        if let Some(player) = state.get_local_player() {
            if player.is_dead {
                let (sw, sh) = virtual_screen_size();
                // Dark overlay
                draw_rectangle(0.0, 0.0, sw, sh, Color::from_rgba(0, 0, 0, 150));

                // "YOU DIED" text
                let text = "YOU DIED";
                let font_size = 64.0;
                let text_dims = self.measure_text_sharp(text, font_size);
                let text_x = (sw - text_dims.width) / 2.0;
                let text_y = sh / 2.0 - 20.0;

                // Red text with outline
                for ox in [-2.0, 2.0] {
                    for oy in [-2.0, 2.0] {
                        self.draw_text_sharp(text, text_x + ox, text_y + oy, font_size, BLACK);
                    }
                }
                self.draw_text_sharp(text, text_x, text_y, font_size, RED);

                // Respawn countdown (5 seconds)
                let time_since_death = macroquad::time::get_time() - player.death_time;
                let respawn_time = 5.0 - time_since_death;
                if respawn_time > 0.0 {
                    let countdown_text = format!("Respawning in {:.1}s", respawn_time);
                    let countdown_dims = self.measure_text_sharp(&countdown_text, 16.0);
                    self.draw_text_sharp(
                        &countdown_text,
                        (sw - countdown_dims.width) / 2.0,
                        text_y + 50.0,
                        24.0,
                        WHITE,
                    );
                }
            }
        }

        // Chat messages (bottom-left) with text wrapping - only if visible
        // Scale with UI scale for readability
        if state.ui_state.chat_log_visible {
            let scale = state.ui_state.ui_scale;
            let chat_x = 10.0;
            let (_, chat_sh) = virtual_screen_size();
            // Layout: BG bottom aligned with hotkey bar bottom, text inside with padding
            let bg_padding = 6.0 * scale;
            let line_height = 18.0 * scale;
            let max_chat_width = if scale >= 2.0 {
                400.0 * scale - 260.0
            } else {
                360.0 * scale
            };
            let font_size: f32 = 16.0;
            let max_visible_lines: usize = if scale >= 2.0 { 6 } else { 7 };
            let chat_area_h = max_visible_lines as f32 * line_height;

            // BG rectangle positioned from the hotkey bar bottom edge
            let bg_bottom = chat_sh - EXP_BAR_GAP * scale;
            // Always reserve a row at the bottom for the input hint ("Press Enter to chat…")
            // so the chat reads as a complete framed element.
            let input_row_h = 24.0 * scale;
            let effective_bottom = bg_bottom - input_row_h;
            let clip_h = chat_area_h + bg_padding * 2.0;
            let clip_x = chat_x - bg_padding;
            let clip_y = bg_bottom - clip_h;
            let clip_w = max_chat_width + bg_padding * 2.0;

            // Text baselines inside the BG, bg_padding from edges
            let chat_bottom_y = effective_bottom - bg_padding;
            let chat_top_y = clip_y + bg_padding;

            // Tab bar above chat log
            let tab_h = 18.0 * scale;
            let tab_names = ["Public", "Global", "System"];
            let tab_channels = [ChatChannel::Local, ChatChannel::Global, ChatChannel::System];
            let tab_bar_y = clip_y - tab_h;
            let latest_local_ts = state
                .ui_state
                .chat_messages
                .latest_timestamp(&ChatChannel::Local);
            let latest_global_ts = state
                .ui_state
                .chat_messages
                .latest_timestamp(&ChatChannel::Global);
            let latest_system_ts = state
                .ui_state
                .chat_messages
                .latest_timestamp(&ChatChannel::System);

            // ===== Framed container: semi-opaque backing =====
            // No top inset — the active tab box must align exactly with the hover/hit
            // boundary (which starts at tab_bar_y), so the gold tab isn't taller than its
            // clickable area.
            let top_inset = 0.0;
            let cont_x = clip_x;
            let cont_y = tab_bar_y - top_inset;
            let cont_w = clip_w;
            let _cont_h = (clip_y + clip_h) - cont_y;

            // Compute tab rects first so we can size the tab-strip fill to match.
            let tab_rects = self.chat_tab_rects(clip_x + 2.0, tab_bar_y, tab_h);
            let tabs_right = tab_rects[2].x + tab_rects[2].w;

            if state.ui_state.chat_log_background {
                // Tab-strip fill: only as wide as the three tabs (so empty space above the
                // content area doesn't show a floating background).
                let tab_strip_h = clip_y - cont_y; // top_inset + tab_h
                draw_rectangle(
                    cont_x,
                    cont_y,
                    tabs_right - cont_x,
                    tab_strip_h,
                    HUD_FILL_TRANSLUCENT,
                );
                // Content body fill: full width from the divider down.
                draw_rectangle(cont_x, clip_y, cont_w, clip_h, HUD_FILL_TRANSLUCENT);
                // Divider below the tab row (full width).
                draw_line(
                    cont_x,
                    tab_bar_y + tab_h,
                    cont_x + cont_w,
                    tab_bar_y + tab_h,
                    1.0,
                    HEADER_BORDER,
                );
            }
            for i in 0..3 {
                let r = tab_rects[i];
                let is_active = std::mem::discriminant(&state.ui_state.chat_active_tab)
                    == std::mem::discriminant(&tab_channels[i]);
                let is_hovered = state.ui_state.hovered_element.as_ref()
                    == Some(
                        &[
                            UiElementId::ChatTabLocal,
                            UiElementId::ChatTabGlobal,
                            UiElementId::ChatTabSystem,
                        ][i],
                    );
                let has_unread = match tab_channels[i] {
                    ChatChannel::Local => latest_local_ts > state.ui_state.chat_last_seen_local,
                    ChatChannel::Global => latest_global_ts > state.ui_state.chat_last_seen_global,
                    ChatChannel::System => latest_system_ts > state.ui_state.chat_last_seen_system,
                };

                if is_active {
                    // Gold-lit active tab — extends from the container top down to the divider.
                    let a_top = cont_y;
                    let a_h = (tab_bar_y + tab_h) - a_top;
                    // First tab: pull the left edge flush to the container edge (no clearance).
                    let a_left = if i == 0 { cont_x } else { r.x };
                    let a_w = (r.x + r.w) - a_left;
                    draw_rectangle(
                        a_left,
                        a_top,
                        a_w,
                        a_h,
                        Color::new(0.180, 0.165, 0.110, 0.92),
                    );
                    draw_rectangle(a_left, a_top, a_w, 1.0, FRAME_ACCENT);
                    draw_rectangle(a_left, a_top, 1.0, a_h, FRAME_ACCENT);
                    draw_rectangle(a_left + a_w - 1.0, a_top, 1.0, a_h, FRAME_ACCENT);
                } else if is_hovered {
                    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.22));
                }

                let label_size: f32 = 16.0;
                let tw = self.measure_text_sharp(tab_names[i], label_size).width;
                self.draw_text_sharp(
                    tab_names[i],
                    (r.x + (r.w - tw) / 2.0).floor(),
                    (r.y + r.h / 2.0 + label_size * 0.28).floor(),
                    label_size,
                    if is_active {
                        Color::new(1.0, 0.86, 0.45, 1.0) // gold-lit
                    } else if has_unread {
                        Color::new(0.92, 0.92, 0.92, 1.0)
                    } else {
                        Color::new(0.6, 0.6, 0.6, 1.0)
                    },
                );
            }

            // Build wrapped chat lines only when chat content or layout changes.
            let cache_key = ChatLinesCacheKey {
                chat_revision: state.ui_state.chat_revision,
                max_chat_width_x100: (max_chat_width * 100.0).round() as i32,
                font_size_x100: (font_size * 100.0).round() as i32,
                active_tab: match state.ui_state.chat_active_tab {
                    ChatChannel::Local => 0,
                    ChatChannel::Global => 1,
                    ChatChannel::System => 2,
                },
                hide_system_in_public: state.ui_state.hide_system_in_public,
            };

            let rebuild_chat_cache = {
                let cache = self.chat_lines_cache.borrow();
                cache.key != Some(cache_key)
            };

            if rebuild_chat_cache {
                let active_msgs = state
                    .ui_state
                    .chat_messages
                    .channel(&state.ui_state.chat_active_tab);
                let hide_system = state.ui_state.hide_system_in_public
                    && matches!(state.ui_state.chat_active_tab, ChatChannel::Local);
                let mut rebuilt_lines: Vec<(String, Color)> =
                    Vec::with_capacity(active_msgs.len() * 2);

                for msg in active_msgs
                    .iter()
                    .filter(|m| !hide_system || !matches!(m.channel, ChatChannel::System))
                {
                    let (color, text) = match msg.channel {
                        ChatChannel::Local => (WHITE, format!("{}: {}", msg.sender_name, msg.text)),
                        ChatChannel::Global => {
                            (SKYBLUE, format!("[G] {}: {}", msg.sender_name, msg.text))
                        }
                        ChatChannel::System => {
                            (YELLOW, format!("{} {}", msg.sender_name, msg.text))
                        }
                    };
                    let wrapped_lines = self.wrap_text(&text, max_chat_width, font_size);
                    for line in wrapped_lines {
                        rebuilt_lines.push((line, color));
                    }
                }

                let mut cache = self.chat_lines_cache.borrow_mut();
                cache.key = Some(cache_key);
                cache.lines = rebuilt_lines;
            }

            let cache = self.chat_lines_cache.borrow();
            let all_lines = &cache.lines;

            // Apply smooth pixel-based scroll offset
            let total_lines = all_lines.len();
            let total_content_height = total_lines as f32 * line_height;
            let max_scroll_px = (total_content_height - chat_area_h).max(0.0);
            let scroll_px = state.ui_state.chat_message_scroll.min(max_scroll_px);

            // Calculate which lines are visible and the sub-pixel offset
            let scroll_lines = scroll_px / line_height;
            let fractional_offset = (scroll_lines.fract()) * line_height;
            let scroll_lines_int = scroll_lines.floor() as usize;

            // We need one extra line for smooth scrolling (partially visible at top/bottom)
            let visible_lines = max_visible_lines + 1;
            let end = total_lines.saturating_sub(scroll_lines_int);
            let start = end.saturating_sub(visible_lines);

            // Scissor clip text to the background box bounds
            let physical_w = screen_width();
            let physical_h = screen_height();
            let (vw, vh) = virtual_screen_size();
            let sx = physical_w / vw;
            let sy = physical_h / vh;
            {
                let mut gl = unsafe { get_internal_gl() };
                gl.flush();
                gl.quad_gl.scissor(Some((
                    (clip_x * sx) as i32,
                    (clip_y * sy) as i32,
                    (clip_w * sx) as i32,
                    (clip_h * sy) as i32,
                )));
            }

            let mut current_y = chat_bottom_y + fractional_offset;
            for i in (start..end).rev() {
                if current_y >= chat_top_y - line_height && current_y <= chat_bottom_y + line_height
                {
                    let (ref line, color) = all_lines[i];
                    self.draw_text_sharp(line, chat_x, current_y, font_size, color);
                }
                current_y -= line_height;
            }

            // Disable scissor clipping
            {
                let mut gl = unsafe { get_internal_gl() };
                gl.flush();
                gl.quad_gl.scissor(None);
            }

            // Draw scrollbar on the right edge
            if max_scroll_px > 0.0 {
                let scrollbar_w = 6.0 * scale;
                let scrollbar_x = clip_x + clip_w - scrollbar_w;
                let track_y = clip_y;
                let track_h = clip_h;

                // Track
                draw_rectangle(
                    scrollbar_x,
                    track_y,
                    scrollbar_w,
                    track_h,
                    Color::new(0.1, 0.09, 0.12, 0.6),
                );

                // Thumb - size proportional to visible area, position based on scroll
                let visible_ratio = (chat_area_h / total_content_height).min(1.0);
                let thumb_h = (track_h * visible_ratio).max(12.0 * scale);
                // scroll_px=0 means at bottom (most recent), max_scroll_px means scrolled to top
                let scroll_ratio = scroll_px / max_scroll_px;
                let thumb_y = track_y + (track_h - thumb_h) * (1.0 - scroll_ratio);

                let is_dragging = state.ui_state.chat_scroll_drag.dragging;
                let thumb_color = if is_dragging {
                    Color::new(1.0, 1.0, 1.0, 0.6)
                } else {
                    Color::new(1.0, 1.0, 1.0, 0.35)
                };
                draw_rectangle(
                    scrollbar_x + 1.0,
                    thumb_y,
                    scrollbar_w - 2.0,
                    thumb_h,
                    thumb_color,
                );
            }

            // ===== Input hint row (bottom of the framed container) =====
            if state.ui_state.chat_log_background && !state.ui_state.chat_open {
                let row_y = bg_bottom - input_row_h;
                draw_line(cont_x, row_y, cont_x + cont_w, row_y, 1.0, HEADER_BORDER);
                let icon_size = 14.0 * scale;
                let icon_x = cont_x + 8.0 * scale;
                let icon_y = row_y + (input_row_h - icon_size) / 2.0;
                if let Some(ref tex) = self.chat_small_icon {
                    draw_texture_ex(
                        tex,
                        icon_x.floor(),
                        icon_y.floor(),
                        TEXT_DIM,
                        DrawTextureParams {
                            dest_size: Some(Vec2::new(icon_size, icon_size)),
                            ..Default::default()
                        },
                    );
                }
                self.draw_text_sharp(
                    "Press Enter to chat...",
                    (icon_x + icon_size + 6.0 * scale).floor(),
                    (row_y + input_row_h * 0.66).floor(),
                    font_size,
                    TEXT_DIM,
                );
            }
        }

        // Top HUD: minimap on right, local name/stats on left.
        if let Some(player) = state.get_local_player() {
            let padding = 6.0;
            let font_size = 16.0;
            let s = self.font_scale.get();

            // Measure text first to calculate widths
            let name = &player.name;
            let level_text = format!(" Lv.{}", player.skills.total_level());
            let name_w = self.measure_text_sharp(name, font_size).width;
            let level_w = self.measure_text_sharp(&level_text, font_size).width;
            let total_text_w = name_w + level_w;

            // Both bars use same width (at least 120, or text width + padding)
            let mut bar_width = self.hud_bar_width(state);
            let mut bar_height = 18.0 * s;
            // Extra space the desktop cluster reserves below the bars (style row + frame).
            let hud_below = self.hud_below_bars_offset();

            if !cfg!(target_os = "android") && self.minimap_preview_enabled(state) {
                let t = macroquad::time::get_time();
                self.render_minimap_preview(state);
                self.dbg_ui_minimap_ms
                    .set((macroquad::time::get_time() - t) * 1000.0);
            } else {
                self.dbg_ui_minimap_ms.set(0.0);
            }

            // Downstream anchors for indicators/chips
            let bar_x: f32;
            let prayer_bar_y: f32;
            // Geometry the transient indicators (fishing/stall/dash) and chip row use so
            // they line up flush with the stat bars: the bars are trimmed in from the
            // right (bar_rw), so the indicators must match that exact width rather than
            // running the full bar_width and overhanging.
            let indicator_x: f32;
            let indicator_w: f32;

            if cfg!(target_os = "android") {
                // ===== ANDROID: Compact horizontal bars at top-left =====
                let hud_x = 10.0;
                let hud_y = 10.0;
                let compact_h = 18.0;
                let compact_bar_w = 75.0;
                let gap = 3.0;

                // Name tag (compact)
                let name_tag_w = total_text_w + padding * 2.0;
                draw_rectangle(
                    hud_x,
                    hud_y,
                    name_tag_w,
                    compact_h,
                    Color::new(0.0, 0.0, 0.0, 0.55),
                );
                let ntx = hud_x + padding;
                let nty = (hud_y + compact_h * 0.73).floor();
                self.draw_text_sharp(name, ntx, nty, font_size, TEXT_TITLE);
                self.draw_text_sharp(&level_text, ntx + name_w, nty, font_size, TEXT_DIM);

                let mut cx = hud_x + name_tag_w + gap;

                // HP bar
                let hp_ratio = player.hp as f32 / player.max_hp.max(1) as f32;
                let hp_color = if hp_ratio > 0.5 {
                    Color::new(0.2, 0.7, 0.3, 1.0)
                } else if hp_ratio > 0.25 {
                    Color::new(0.8, 0.6, 0.1, 1.0)
                } else {
                    Color::new(0.8, 0.2, 0.2, 1.0)
                };
                draw_rectangle(
                    cx,
                    hud_y,
                    compact_bar_w,
                    compact_h,
                    Color::new(0.08, 0.08, 0.10, 0.85),
                );
                let hp_fill = (compact_bar_w - 2.0) * hp_ratio;
                if hp_fill > 0.0 {
                    draw_rectangle(cx + 1.0, hud_y + 1.0, hp_fill, compact_h - 2.0, hp_color);
                    draw_rectangle(
                        cx + 1.0,
                        hud_y + 1.0,
                        hp_fill,
                        (compact_h - 2.0) / 2.0,
                        Color::new(1.0, 1.0, 1.0, 0.2),
                    );
                }
                let hp_text = format!("{}/{}", player.hp, player.max_hp);
                let hp_tw = self.measure_text_sharp(&hp_text, font_size).width;
                self.draw_text_sharp(
                    &hp_text,
                    (cx + (compact_bar_w - hp_tw) / 2.0).floor(),
                    (hud_y + compact_h * 0.78).floor(),
                    font_size,
                    TEXT_NORMAL,
                );
                cx += compact_bar_w + gap;

                // MP bar
                let (mp, max_mp) = (player.mp, player.max_mp);
                let mp_ratio = if max_mp > 0 {
                    mp as f32 / max_mp as f32
                } else {
                    0.0
                };
                draw_rectangle(
                    cx,
                    hud_y,
                    compact_bar_w,
                    compact_h,
                    Color::new(0.08, 0.08, 0.10, 0.85),
                );
                let mp_fill = (compact_bar_w - 2.0) * mp_ratio;
                if mp_fill > 0.0 {
                    draw_rectangle(
                        cx + 1.0,
                        hud_y + 1.0,
                        mp_fill,
                        compact_h - 2.0,
                        Color::new(0.3, 0.2, 0.8, 1.0),
                    );
                    draw_rectangle(
                        cx + 1.0,
                        hud_y + 1.0,
                        mp_fill,
                        (compact_h - 2.0) / 2.0,
                        Color::new(0.5, 0.4, 0.95, 1.0),
                    );
                }
                let mp_text = format!("{}/{}", mp, max_mp);
                let mp_tw = self.measure_text_sharp(&mp_text, font_size).width;
                self.draw_text_sharp(
                    &mp_text,
                    (cx + (compact_bar_w - mp_tw) / 2.0).floor(),
                    (hud_y + compact_h * 0.78).floor(),
                    font_size,
                    TEXT_NORMAL,
                );
                cx += compact_bar_w + gap;

                // Prayer bar
                let prayer_ratio = if state.max_prayer_points > 0 {
                    state.prayer_points as f32 / state.max_prayer_points as f32
                } else {
                    0.0
                };
                draw_rectangle(
                    cx,
                    hud_y,
                    compact_bar_w,
                    compact_h,
                    Color::new(0.08, 0.08, 0.10, 0.85),
                );
                let pr_fill = (compact_bar_w - 2.0) * prayer_ratio;
                if pr_fill > 0.0 {
                    draw_rectangle(
                        cx + 1.0,
                        hud_y + 1.0,
                        pr_fill,
                        compact_h - 2.0,
                        Color::new(0.2, 0.7, 0.85, 1.0),
                    );
                    draw_rectangle(
                        cx + 1.0,
                        hud_y + 1.0,
                        pr_fill,
                        (compact_h - 2.0) / 2.0,
                        Color::new(1.0, 1.0, 1.0, 0.2),
                    );
                }
                let pr_text = format!("{}/{}", state.prayer_points, state.max_prayer_points);
                let pr_tw = self.measure_text_sharp(&pr_text, font_size).width;
                self.draw_text_sharp(
                    &pr_text,
                    (cx + (compact_bar_w - pr_tw) / 2.0).floor(),
                    (hud_y + compact_h * 0.78).floor(),
                    font_size,
                    TEXT_NORMAL,
                );

                // Set anchors for downstream indicators/chips
                bar_x = 10.0;
                bar_width = 100.0;
                bar_height = compact_h;
                prayer_bar_y = hud_y;
                // Android bars run the full width, so indicators match it directly.
                indicator_x = bar_x;
                indicator_w = bar_width;
            } else {
                // ===== DESKTOP: portrait + name header, icon-prefixed bars, style row =====
                let (name_tag_x, name_tag_y) = self.local_name_tag_position(state);

                // Frameless HUD cluster: the portrait, name/level and stat bars sit directly
                // on the world. The bars keep their own recessed backgrounds; only the
                // identity block (portrait + name + level) gets a small translucent backing.

                // Portrait box: clips the head, which is drawn at a fixed integer scale, so a
                // tighter box just trims the empty margin/hair around it (head stays the same
                // size). Right edge lands ~at the icon-column edge so the text clears the hair.
                let portrait_size = 22.0 * s;
                let portrait_x = name_tag_x - 4.0 * s;

                // Iconless stat bars: the bars (and the downstream HUD indicators that anchor
                // off bar_x) start at the nameplate box's left edge and run full width — no
                // icon gutter — so the whole cluster reads flush-left under the box.
                let (_bx, stats_y) = self.minimap_stats_stack_position(state, bar_width);
                let hpad = 4.0 * s;
                bar_x = portrait_x - hpad; // box + bar left edge
                let bar_rx = bar_x;
                let bar_rw = bar_width - 20.0 * s; // trimmed in from the right
                                                   // Transient indicators + chip row share the stat bars' exact left edge
                                                   // and trimmed width so the whole cluster reads as one aligned column.
                indicator_x = bar_rx;
                indicator_w = bar_rw;

                // Name + level sit to the right of the portrait inside the nameplate, with a
                // bit of breathing room so they don't touch the head.
                let txt_x = portrait_x + portrait_size + 8.0 * s;
                let level_label = format!("Level {}", player.skills.total_level());

                // Translucent nameplate backing (chat-box fill) wrapping the portrait + name +
                // level. Its left edge doubles as the bars' left edge; expanded a little on the
                // right + bottom so the text always fits with padding.
                let name_w = self.measure_text_sharp(name, 16.0).width;
                let level_w = self.measure_text_sharp(&level_label, 16.0).width;
                let hb_x = bar_x;
                let hb_y = name_tag_y - hpad;
                let hb_right = txt_x + name_w.max(level_w) + hpad + 2.0 * s;
                let hb_bottom = name_tag_y + 24.0 * s;
                draw_rectangle(
                    hb_x,
                    hb_y,
                    hb_right - hb_x,
                    hb_bottom - hb_y,
                    HUD_FILL_TRANSLUCENT,
                );

                let t_portrait = macroquad::time::get_time();
                self.draw_player_head_portrait(player, portrait_x, name_tag_y, portrait_size);
                self.dbg_ui_portrait_ms
                    .set((macroquad::time::get_time() - t_portrait) * 1000.0);
                self.draw_text_sharp(
                    name,
                    txt_x,
                    (name_tag_y + 11.0 * s).floor(),
                    16.0,
                    TEXT_NORMAL,
                );
                self.draw_text_sharp(
                    &level_label,
                    txt_x,
                    (name_tag_y + 21.0 * s).floor(),
                    16.0,
                    TEXT_DIM,
                );

                // ===== HP BAR =====
                let hp_bar_y = stats_y;
                let hp_ratio = player.hp as f32 / player.max_hp.max(1) as f32;
                let (hp_main, hp_dark) = if hp_ratio > 0.5 {
                    (STAT_HP_MAIN, STAT_HP_DARK)
                } else if hp_ratio > 0.25 {
                    (
                        Color::new(0.85, 0.65, 0.20, 1.0),
                        Color::new(0.58, 0.42, 0.10, 1.0),
                    )
                } else {
                    (
                        Color::new(0.80, 0.32, 0.32, 1.0),
                        Color::new(0.54, 0.17, 0.17, 1.0),
                    )
                };
                self.draw_hud_stat_bar_fill(
                    bar_rx,
                    hp_bar_y,
                    bar_rw,
                    bar_height,
                    hp_ratio,
                    SLOT_INNER_SHADOW,
                    hp_main,
                    hp_dark,
                );
                let hp_text = format!("{} / {}", player.hp, player.max_hp);
                let hp_text_w = self.measure_text_sharp(&hp_text, font_size).width;
                self.draw_text_outlined(
                    &hp_text,
                    (bar_rx + (bar_rw - hp_text_w) / 2.0).floor(),
                    (hp_bar_y + bar_height * 0.78).floor(),
                    font_size,
                    WHITE,
                );

                // ===== MP BAR =====
                let mp_bar_y = hp_bar_y + bar_height + 4.0 * s;
                let (mp, max_mp) = state
                    .get_local_player()
                    .map(|p| (p.mp, p.max_mp))
                    .unwrap_or((0, 12));
                let mp_ratio = if max_mp > 0 {
                    mp as f32 / max_mp as f32
                } else {
                    0.0
                };
                self.draw_hud_stat_bar_fill(
                    bar_rx,
                    mp_bar_y,
                    bar_rw,
                    bar_height,
                    mp_ratio,
                    SLOT_INNER_SHADOW,
                    STAT_MP_MAIN,
                    STAT_MP_DARK,
                );
                let mp_text = format!("{} / {}", mp, max_mp);
                let mp_text_w = self.measure_text_sharp(&mp_text, font_size).width;
                self.draw_text_outlined(
                    &mp_text,
                    (bar_rx + (bar_rw - mp_text_w) / 2.0).floor(),
                    (mp_bar_y + bar_height * 0.78).floor(),
                    font_size,
                    WHITE,
                );

                // ===== PRAYER BAR =====
                prayer_bar_y = mp_bar_y + bar_height + 4.0 * s;
                let prayer_ratio = if state.max_prayer_points > 0 {
                    state.prayer_points as f32 / state.max_prayer_points as f32
                } else {
                    0.0
                };
                let prayer_low = prayer_ratio < 0.2 && state.max_prayer_points > 0;
                let prayer_border = if prayer_low {
                    let flash = ((macroquad::time::get_time() * 2.0).sin() * 0.3 + 0.7) as f32;
                    Color::new(0.4 * flash + 0.2, 0.15, 0.15, 1.0)
                } else {
                    SLOT_INNER_SHADOW
                };
                self.draw_hud_stat_bar_fill(
                    bar_rx,
                    prayer_bar_y,
                    bar_rw,
                    bar_height,
                    prayer_ratio,
                    prayer_border,
                    STAT_PRAYER_MAIN,
                    STAT_PRAYER_DARK,
                );
                let prayer_text = format!("{} / {}", state.prayer_points, state.max_prayer_points);
                let prayer_text_w = self.measure_text_sharp(&prayer_text, font_size).width;
                self.draw_text_outlined(
                    &prayer_text,
                    (bar_rx + (bar_rw - prayer_text_w) / 2.0).floor(),
                    (prayer_bar_y + bar_height * 0.78).floor(),
                    font_size,
                    WHITE,
                );

                // Hover tooltip naming each bar (Health / Mana / Prayer). Detect the hover
                // directly off the mouse position (converted to virtual coords). Drawn here in
                // the non-interactive pass, so any panel drawn later cleanly occludes it.
                let (vsw, vsh) = virtual_screen_size();
                let (raw_mx, raw_my) = macroquad::input::mouse_position();
                let mx = raw_mx * vsw / macroquad::window::screen_width();
                let my = raw_my * vsh / macroquad::window::screen_height();
                let over = |by: f32| {
                    mx >= bar_rx && mx <= bar_rx + bar_rw && my >= by && my <= by + bar_height
                };
                let tip = if over(hp_bar_y) {
                    Some(format!("Health: {} / {}", player.hp, player.max_hp))
                } else if over(mp_bar_y) {
                    Some(format!("Mana: {} / {}", mp, max_mp))
                } else if over(prayer_bar_y) {
                    Some(format!(
                        "Prayer: {} / {}",
                        state.prayer_points, state.max_prayer_points
                    ))
                } else {
                    None
                };
                if let Some(label) = tip {
                    let pad = 5.0 * s;
                    let tw = self.measure_text_sharp(&label, font_size).width;
                    let box_w = tw + pad * 2.0;
                    let box_h = 22.0 * s;
                    // Below-right of the cursor, clamped to stay on screen.
                    let mut bx = mx + 14.0 * s;
                    let mut by = my + 14.0 * s;
                    if bx + box_w > vsw {
                        bx = vsw - box_w;
                    }
                    if by + box_h > vsh {
                        by = vsh - box_h;
                    }
                    draw_rectangle(
                        bx + 2.0,
                        by + 2.0,
                        box_w,
                        box_h,
                        Color::new(0.0, 0.0, 0.0, 0.4),
                    );
                    draw_rectangle(
                        bx - 1.0,
                        by - 1.0,
                        box_w + 2.0,
                        box_h + 2.0,
                        crate::render::ui::common::TOOLTIP_FRAME,
                    );
                    draw_rectangle(bx, by, box_w, box_h, crate::render::ui::common::TOOLTIP_BG);
                    self.draw_text_sharp(
                        &label,
                        (bx + pad).floor(),
                        (by + 15.0 * s).floor(),
                        font_size,
                        TEXT_NORMAL,
                    );
                }
            }

            // Bottom of the stat cluster: where transient HUD indicators begin. On
            // desktop this clears the embedded style row + cluster frame; on android
            // hud_below is 0 so this matches the old prayer-bar-bottom anchor.
            let stack_bottom = prayer_bar_y + bar_height + hud_below;

            // ===== Gathering/Woodcutting status indicator (below the cluster) =====
            let is_skilling = state.is_gathering || state.is_woodcutting;
            if is_skilling {
                let gather_y = stack_bottom + 4.0 * s;
                let gather_h = 22.0 * s;
                // Semi-transparent background (blue for fishing, brown for woodcutting)
                let (bg_color, border_color, text_color, action_name) = if state.is_woodcutting {
                    (
                        Color::new(0.15, 0.10, 0.05, 0.7),
                        Color::new(0.5, 0.35, 0.2, 0.5),
                        Color::new(0.9, 0.7, 0.4, 0.9),
                        "Chopping",
                    )
                } else {
                    (
                        Color::new(0.05, 0.15, 0.25, 0.7),
                        Color::new(0.2, 0.5, 0.7, 0.5),
                        Color::new(0.4, 0.8, 0.95, 0.9),
                        "Fishing",
                    )
                };
                draw_rectangle(indicator_x, gather_y, indicator_w, gather_h, bg_color);
                draw_rectangle_lines(
                    indicator_x,
                    gather_y,
                    indicator_w,
                    gather_h,
                    1.0,
                    border_color,
                );
                // Animated dots
                let dot_count = ((macroquad::time::get_time() * 2.0) as usize % 4) as usize;
                let dots = ".".repeat(dot_count);
                let label = format!("{}{}", action_name, dots);
                let label_w = self.measure_text_sharp(&label, 16.0).width;
                self.draw_text_sharp(
                    &label,
                    (indicator_x + (indicator_w - label_w) / 2.0).floor(),
                    (gather_y + gather_h * 0.68).floor(),
                    16.0,
                    text_color,
                );
            }

            // ===== Store Open status indicator (below gathering status or prayer bar) =====
            let has_stall_bar = state.ui_state.stall_active;
            if has_stall_bar {
                let stall_bar_y =
                    stack_bottom + 4.0 * s + if is_skilling { 22.0 * s + 4.0 * s } else { 0.0 };
                let stall_h = 22.0 * s;
                let bg_color = Color::new(0.05, 0.18, 0.08, 0.7);
                let border_color = Color::new(0.2, 0.55, 0.25, 0.5);
                let text_color = Color::new(0.5, 0.9, 0.55, 0.9);
                draw_rectangle(indicator_x, stall_bar_y, indicator_w, stall_h, bg_color);
                draw_rectangle_lines(
                    indicator_x,
                    stall_bar_y,
                    indicator_w,
                    stall_h,
                    1.0,
                    border_color,
                );
                let label = "Store Open";
                let label_w = self.measure_text_sharp(label, 16.0).width;
                self.draw_text_sharp(
                    label,
                    (indicator_x + (indicator_w - label_w) / 2.0).floor(),
                    (stall_bar_y + stall_h * 0.68).floor(),
                    16.0,
                    text_color,
                );
            }

            // ===== Dash cooldown indicator (below stall/gathering status or prayer bar) =====
            let status_bars_offset = if is_skilling { 22.0 * s + 4.0 * s } else { 0.0 }
                + if has_stall_bar {
                    22.0 * s + 4.0 * s
                } else {
                    0.0
                };
            let dash_bar_y = stack_bottom + 4.0 * s + status_bars_offset;
            let current_time = macroquad::time::get_time();
            if state.dash_cooldown_end > current_time {
                let remaining = (state.dash_cooldown_end - current_time) as f32;
                let total_cooldown = 4.0f32;
                let progress = 1.0 - (remaining / total_cooldown).clamp(0.0, 1.0);
                let dash_h = 22.0 * s;

                // Background
                let bg_color = Color::new(0.15, 0.08, 0.15, 0.7);
                let border_color = Color::new(0.5, 0.25, 0.5, 0.5);
                draw_rectangle(indicator_x, dash_bar_y, indicator_w, dash_h, bg_color);
                draw_rectangle_lines(
                    indicator_x,
                    dash_bar_y,
                    indicator_w,
                    dash_h,
                    1.0,
                    border_color,
                );

                // Fill bar
                let fill_w = (indicator_w - 4.0) * progress;
                if fill_w > 0.0 {
                    let fill_color = Color::new(0.6, 0.3, 0.8, 0.8);
                    draw_rectangle(
                        indicator_x + 2.0,
                        dash_bar_y + 2.0,
                        fill_w,
                        dash_h - 4.0,
                        fill_color,
                    );
                }

                // Text
                let remaining_text = format!("Dash {:.1}s", remaining);
                let text_w = self.measure_text_sharp(&remaining_text, 16.0).width;
                let text_color = Color::new(0.8, 0.6, 0.95, 0.9);
                self.draw_text_sharp(
                    &remaining_text,
                    (indicator_x + (indicator_w - text_w) / 2.0).floor(),
                    (dash_bar_y + dash_h * 0.68).floor(),
                    16.0,
                    text_color,
                );
            }

            // XP Globes (to the left of minimap)
            let preview = self.minimap_preview_rect();
            let globe_anchor_x = preview.x;
            // Align globe top edge with minimap top edge.
            let globe_stats_y = preview.y + 20.0;
            self.render_xp_globes(&state.xp_globes, globe_anchor_x, globe_stats_y);

            // HUD chips (below gathering/stall/dash indicators): combat style + slayer task side-by-side
            // Skip on Android to keep mobile HUD clean
            let has_dash_bar = state.dash_cooldown_end > current_time;
            let has_any_chip;
            let chip_row_h: f32;

            if !cfg!(target_os = "android") {
                let chip_row_y = stack_bottom
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
                let chip_gap = 4.0 * s;
                let mut chip_cursor_x = indicator_x;
                let mut crh: f32 = 0.0;

                // Slayer task chip (first, if active)
                let has_slayer_chip = state.ui_state.slayer_current_task.is_some();
                let (slayer_w, slayer_h) =
                    self.render_slayer_task_chip(state, chip_cursor_x, chip_row_y);
                if slayer_w > 0.0 {
                    chip_cursor_x += slayer_w + chip_gap;
                    crh = crh.max(slayer_h);
                }

                // Resource contract chip (after slayer task, if active)
                let has_contract_chip = state.resource_contract.is_some();
                let (contract_w, contract_h) =
                    self.render_resource_contract_chip(state, chip_cursor_x, chip_row_y);
                if contract_w > 0.0 {
                    chip_cursor_x += contract_w + chip_gap;
                    crh = crh.max(contract_h);
                }

                // Potion buff chips (after the contract)
                let mut has_buff_chip = false;
                for buff in &state.active_potion_buffs {
                    let (buff_w, buff_h) =
                        self.render_potion_buff_chip(state, buff, chip_cursor_x, chip_row_y);
                    if buff_w > 0.0 {
                        has_buff_chip = true;
                        chip_cursor_x += buff_w + chip_gap;
                        crh = crh.max(buff_h);
                    }
                }

                // Combat style now lives in the stat cluster's embedded style row,
                // so it's no longer drawn as a floating chip here.
                let _ = chip_cursor_x;

                has_any_chip = has_slayer_chip || has_contract_chip || has_buff_chip;
                chip_row_h = crh;
            } else {
                has_any_chip = false;
                chip_row_h = 0.0;
            }

            // XP Drop Feed position (below gathering/stall status or MP bar)
            // Actual rendering is deferred to after interactive UI so drops appear above panel overlays
            let extra_offset = if is_skilling { 22.0 + 4.0 } else { 0.0 }
                + if has_stall_bar { 22.0 + 4.0 } else { 0.0 }
                + if has_dash_bar { 22.0 + 4.0 } else { 0.0 }
                + if has_any_chip {
                    chip_row_h / s + 4.0
                } else {
                    0.0
                };
            let drop_start_y = stack_bottom + extra_offset + 145.0;
            self.xp_drop_pos.set(Some((bar_x, drop_start_y)));
        }
        self.dbg_ui_hud_ms
            .set((macroquad::time::get_time() - dbg_hud_t0) * 1000.0);

        // Note: Interactive UI (inventory, crafting, dialogue, quick slots) is rendered
        // by render_interactive_ui() which is called by the main render loop.
        // XP drops are rendered after interactive UI via render_deferred_xp_drops().

        // Area banner (location name during transitions)
        if state.area_banner.is_visible() {
            self.render_area_banner(
                &state.area_banner.text,
                state.area_banner.opacity(),
                state.area_banner.slide_offset(),
            );
        }

        // Chat input box (when open) - scale with UI scale
        // Hidden on System tab (read-only channel)
        if state.ui_state.chat_open
            && !matches!(state.ui_state.chat_active_tab, ChatChannel::System)
        {
            let (_, input_sh) = virtual_screen_size();
            let scale = state.ui_state.ui_scale;
            // Sit in the same spot as the "Press Enter to chat…" hint row, inside the chat box.
            let chat_x = 10.0;
            let bg_padding = 6.0 * scale;
            let max_chat_width = if scale >= 2.0 {
                400.0 * scale - 260.0
            } else {
                360.0 * scale
            };
            let clip_x = chat_x - bg_padding;
            let clip_w = max_chat_width + bg_padding * 2.0;
            let bg_bottom = input_sh - EXP_BAR_GAP * scale;
            let input_row_h = 24.0 * scale;
            // Shared row anchor — identical to the hint row so border/icon/text align
            // pixel-for-pixel whether the input is open or hidden.
            let row_y = bg_bottom - input_row_h;
            // Align the input box flush to the container edges (below the divider, down to
            // the container bottom).
            let input_x = clip_x;
            let input_y = row_y + 1.0;
            let input_width = clip_w;
            let input_height = input_row_h - 1.0;
            let font_size: f32 = 16.0;

            // Channel indicator
            let (indicator, indicator_color) = match state.ui_state.chat_active_tab {
                ChatChannel::Local => ("[P] ", WHITE),
                ChatChannel::Global => ("[G] ", SKYBLUE),
                ChatChannel::System => ("[S] ", YELLOW),
            };
            let indicator_w = self.measure_text_sharp(indicator, font_size).width;

            // Icon + text positions anchored to row_y, matching the hint row exactly.
            let icon_size = 14.0 * scale;
            let icon_x = (clip_x + 8.0 * scale).floor();
            let icon_y = (row_y + (input_row_h - icon_size) / 2.0).floor();
            let text_y = (row_y + input_row_h * 0.66).floor();
            let indicator_start_x = (icon_x + icon_size + 6.0 * scale).floor();
            let text_start_x = indicator_start_x + indicator_w;
            let text_area_width = (input_x + input_width) - text_start_x - 12.0 * scale;

            // Divider — identical draw_line call as the hint row so they render the same pixel.
            draw_line(clip_x, row_y, clip_x + clip_w, row_y, 1.0, HEADER_BORDER);
            draw_rectangle(
                input_x,
                input_y,
                input_width,
                input_height,
                Color::new(0.055, 0.055, 0.075, 0.82),
            );

            // Chat bubble icon — same position as the hint row.
            if let Some(ref tex) = self.chat_small_icon {
                draw_texture_ex(
                    tex,
                    icon_x,
                    icon_y,
                    TEXT_NORMAL,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(icon_size, icon_size)),
                        ..Default::default()
                    },
                );
            }

            let input_text = &state.ui_state.chat_input;
            let cursor_pos = state.ui_state.chat_cursor;
            // Indicator text at same baseline as hint row text.
            self.draw_text_sharp(
                indicator,
                indicator_start_x,
                text_y,
                font_size,
                indicator_color,
            );

            if input_text.is_empty() {
                // Fast path for idle chat input (common case in classic mode).
                let cursor_blink = (macroquad::time::get_time() * 2.0) as i32 % 2 == 0;
                if cursor_blink {
                    draw_line(
                        text_start_x + 1.0,
                        input_y + 4.0 * scale,
                        text_start_x + 1.0,
                        input_y + input_height - 4.0 * scale,
                        1.0,
                        WHITE,
                    );
                }
            } else {
                let char_count = input_text.chars().count();

                // Calculate how many chars fit by measuring actual text width
                let measure_chars_that_fit = |text: &str, max_width: f32| -> usize {
                    let chars: Vec<char> = text.chars().collect();
                    for i in (1..=chars.len()).rev() {
                        let substr: String = chars[..i].iter().collect();
                        if self.measure_text_sharp(&substr, font_size).width <= max_width {
                            return i;
                        }
                    }
                    0
                };

                // Determine scroll offset to keep cursor visible
                let scroll_offset = if self.measure_text_sharp(input_text, font_size).width
                    <= text_area_width
                {
                    // Text fits entirely, no scroll needed
                    0
                } else {
                    // Find offset that keeps cursor visible
                    // Start by trying to show text ending at cursor
                    let text_to_cursor: String = input_text.chars().take(cursor_pos).collect();
                    let cursor_text_width =
                        self.measure_text_sharp(&text_to_cursor, font_size).width;

                    if cursor_text_width <= text_area_width {
                        // Cursor is visible from start
                        0
                    } else {
                        // Need to scroll - find how many chars to skip to show cursor
                        let chars: Vec<char> = input_text.chars().collect();
                        let mut offset = 0;
                        for i in 0..cursor_pos {
                            let visible: String = chars[i..cursor_pos].iter().collect();
                            if self.measure_text_sharp(&visible, font_size).width <= text_area_width
                            {
                                offset = i;
                                break;
                            }
                        }
                        offset
                    }
                };

                // Get visible portion of text that fits
                let chars_from_offset: String = input_text.chars().skip(scroll_offset).collect();
                let visible_char_count =
                    measure_chars_that_fit(&chars_from_offset, text_area_width);
                let visible_text: String = input_text
                    .chars()
                    .skip(scroll_offset)
                    .take(visible_char_count)
                    .collect();
                let visible_end = scroll_offset + visible_char_count;

                self.draw_text_sharp(&visible_text, text_start_x, text_y, font_size, WHITE);

                // Draw scroll indicators if text is clipped
                if scroll_offset > 0 {
                    self.draw_text_sharp("<", text_start_x - 8.0 * scale, text_y, font_size, GRAY);
                }
                if visible_end < char_count {
                    self.draw_text_sharp(
                        ">",
                        input_x + input_width - 10.0 * scale,
                        text_y,
                        font_size,
                        GRAY,
                    );
                }

                // Blinking cursor at correct position within visible text
                let cursor_blink = (macroquad::time::get_time() * 2.0) as i32 % 2 == 0;
                if cursor_blink {
                    let cursor_visible_pos = cursor_pos.saturating_sub(scroll_offset);
                    let text_before_cursor: String =
                        visible_text.chars().take(cursor_visible_pos).collect();
                    let cursor_x = self
                        .measure_text_sharp(&text_before_cursor, font_size)
                        .width;
                    draw_line(
                        text_start_x + cursor_x + 1.0,
                        input_y + 4.0 * scale,
                        text_start_x + cursor_x + 1.0,
                        input_y + input_height - 4.0 * scale,
                        1.0,
                        WHITE,
                    );
                }
            }

            // Re-draw the scrollbar on top of the input box so it stays visible.
            // The chat_log_visible block draws it earlier; this reinstates it after the input.
            let total_lines_sc = self.chat_lines_cache.borrow().lines.len();
            let line_height_sc = 18.0 * scale;
            let max_vis_sc: usize = if scale >= 2.0 { 6 } else { 7 };
            let chat_area_h_sc = max_vis_sc as f32 * line_height_sc;
            let bg_pad_sc = 6.0 * scale;
            let clip_h_sc = chat_area_h_sc + bg_pad_sc * 2.0;
            let clip_y_sc = bg_bottom - clip_h_sc;
            let total_content_sc = total_lines_sc as f32 * line_height_sc;
            let max_scroll_sc = (total_content_sc - chat_area_h_sc).max(0.0);
            if max_scroll_sc > 0.0 {
                let scroll_sc = state.ui_state.chat_message_scroll.min(max_scroll_sc);
                let sb_w = 6.0 * scale;
                let sb_x = clip_x + clip_w - sb_w;
                // Redraw track over the input row area only (row_y..bg_bottom).
                draw_rectangle(
                    sb_x,
                    row_y,
                    sb_w,
                    bg_bottom - row_y,
                    Color::new(0.1, 0.09, 0.12, 0.6),
                );
                let visible_ratio_sc = (chat_area_h_sc / total_content_sc).min(1.0);
                let thumb_h_sc = (clip_h_sc * visible_ratio_sc).max(12.0 * scale);
                let thumb_y_sc =
                    clip_y_sc + (clip_h_sc - thumb_h_sc) * (1.0 - scroll_sc / max_scroll_sc);
                let is_drag = state.ui_state.chat_scroll_drag.dragging;
                let thumb_c = if is_drag {
                    Color::new(1.0, 1.0, 1.0, 0.6)
                } else {
                    Color::new(1.0, 1.0, 1.0, 0.35)
                };
                if thumb_y_sc + thumb_h_sc > row_y {
                    let vt = thumb_y_sc.max(row_y);
                    let vh = (thumb_y_sc + thumb_h_sc - vt).max(0.0);
                    draw_rectangle(sb_x + 1.0, vt, sb_w - 2.0, vh, thumb_c);
                }
            }
        }
    }

    /// Render XP drop feed above interactive UI overlays (called after render_interactive_ui)
    pub(super) fn render_deferred_xp_drops(&self, state: &GameState) {
        if let Some((x, start_y)) = self.xp_drop_pos.get() {
            self.render_xp_drop_feed(&state.xp_drop_feed, x, start_y);
            self.xp_drop_pos.set(None);
        }
    }
}
