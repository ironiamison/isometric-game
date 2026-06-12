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
            let chat_input_open = state.ui_state.chat_open
                && !matches!(state.ui_state.chat_active_tab, ChatChannel::System);
            let bg_bottom = chat_sh - EXP_BAR_GAP * scale;
            // When input bar is open, shrink visible area so messages don't render behind it
            let effective_bottom = if chat_input_open {
                bg_bottom - 28.0 * scale
            } else {
                bg_bottom
            };
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
            let num_tabs = 3.0f32;
            let tab_w = (max_chat_width / num_tabs).floor();
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

            for i in 0..3 {
                let tx = chat_x + i as f32 * tab_w;
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

                let bg = if is_active {
                    Color::new(0.15, 0.15, 0.2, 0.85)
                } else if is_hovered {
                    Color::new(0.1, 0.1, 0.15, 0.7)
                } else {
                    Color::new(0.05, 0.05, 0.08, 0.65)
                };

                draw_rectangle(tx, tab_bar_y, tab_w, tab_h, bg);

                if is_active {
                    // Gold underline for active tab
                    draw_rectangle(
                        tx + 2.0,
                        tab_bar_y + tab_h - 2.0,
                        tab_w - 4.0,
                        2.0,
                        Color::new(0.76, 0.60, 0.23, 1.0),
                    );
                }

                let label_size: f32 = 16.0;
                let tw = self.measure_text_sharp(tab_names[i], label_size).width;
                self.draw_text_sharp(
                    tab_names[i],
                    (tx + (tab_w - tw) / 2.0).floor(),
                    (tab_bar_y + tab_h / 2.0 + label_size * 0.35).floor(),
                    label_size,
                    if is_active {
                        WHITE
                    } else if has_unread {
                        Color::new(0.92, 0.92, 0.92, 1.0)
                    } else {
                        Color::new(0.6, 0.6, 0.6, 1.0)
                    },
                );
            }

            if state.ui_state.chat_log_background {
                draw_rectangle(
                    clip_x,
                    clip_y,
                    clip_w,
                    clip_h,
                    Color::new(0.0, 0.0, 0.0, 0.45),
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
            let mut bar_width = (total_text_w + padding * 2.0).max(120.0 * s);
            let tag_height = 22.0 * s;
            let mut bar_height = 18.0 * s;

            if !cfg!(target_os = "android") && self.minimap_preview_enabled(state) {
                self.render_minimap_preview(state);
            }

            // Downstream anchors for indicators/chips
            let bar_x: f32;
            let prayer_bar_y: f32;

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
            } else {
                // ===== DESKTOP: Vertical stacked bars (top-left) =====
                let (name_tag_x, name_tag_y) = self.local_name_tag_position(state);
                draw_rectangle(
                    name_tag_x,
                    name_tag_y,
                    bar_width,
                    tag_height,
                    Color::new(0.0, 0.0, 0.0, 0.45),
                );

                let text_x = name_tag_x + (bar_width - total_text_w) / 2.0;
                let text_y = (name_tag_y + tag_height * 0.73).floor();
                self.draw_text_sharp(name, text_x, text_y, font_size, TEXT_TITLE);
                self.draw_text_sharp(&level_text, text_x + name_w, text_y, font_size, TEXT_DIM);

                let (bx, stats_y) = self.minimap_stats_stack_position(state, bar_width);
                bar_x = bx;

                // ===== HP BAR =====
                let hp_bar_x = bar_x;
                let hp_bar_y = stats_y;
                let hp_ratio = player.hp as f32 / player.max_hp.max(1) as f32;

                draw_rectangle(hp_bar_x, hp_bar_y, bar_width, bar_height, SLOT_INNER_SHADOW);
                draw_rectangle(
                    hp_bar_x + 1.0,
                    hp_bar_y + 1.0,
                    bar_width - 2.0,
                    bar_height - 2.0,
                    Color::new(0.08, 0.08, 0.10, 1.0),
                );

                let hp_fill_w = (bar_width - 4.0) * hp_ratio;
                if hp_fill_w > 0.0 {
                    let hp_color = if hp_ratio > 0.5 {
                        Color::new(0.2, 0.7, 0.3, 1.0)
                    } else if hp_ratio > 0.25 {
                        Color::new(0.8, 0.6, 0.1, 1.0)
                    } else {
                        Color::new(0.8, 0.2, 0.2, 1.0)
                    };
                    draw_rectangle(
                        hp_bar_x + 2.0,
                        hp_bar_y + 2.0,
                        hp_fill_w,
                        bar_height - 4.0,
                        hp_color,
                    );
                    draw_rectangle(
                        hp_bar_x + 2.0,
                        hp_bar_y + 2.0,
                        hp_fill_w,
                        (bar_height - 4.0) / 2.0,
                        Color::new(1.0, 1.0, 1.0, 0.25),
                    );
                }

                let hp_text = format!("{}/{}", player.hp, player.max_hp);
                let hp_text_w = self.measure_text_sharp(&hp_text, font_size).width;
                self.draw_text_sharp(
                    &hp_text,
                    (hp_bar_x + (bar_width - hp_text_w) / 2.0).floor(),
                    (hp_bar_y + bar_height * 0.78).floor(),
                    font_size,
                    TEXT_NORMAL,
                );

                // ===== MP BAR =====
                let mp_bar_x = bar_x;
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

                draw_rectangle(mp_bar_x, mp_bar_y, bar_width, bar_height, SLOT_INNER_SHADOW);
                draw_rectangle(
                    mp_bar_x + 1.0,
                    mp_bar_y + 1.0,
                    bar_width - 2.0,
                    bar_height - 2.0,
                    Color::new(0.08, 0.08, 0.10, 1.0),
                );

                let mp_fill_w = (bar_width - 4.0) * mp_ratio;
                if mp_fill_w > 0.0 {
                    let mp_color = Color::new(0.3, 0.2, 0.8, 1.0);
                    draw_rectangle(
                        mp_bar_x + 2.0,
                        mp_bar_y + 2.0,
                        mp_fill_w,
                        bar_height - 4.0,
                        mp_color,
                    );
                    draw_rectangle(
                        mp_bar_x + 2.0,
                        mp_bar_y + 2.0,
                        mp_fill_w,
                        (bar_height - 4.0) / 2.0,
                        Color::new(0.5, 0.4, 0.95, 1.0),
                    );
                }

                let mp_text = format!("{}/{}", mp, max_mp);
                let mp_text_w = self.measure_text_sharp(&mp_text, font_size).width;
                self.draw_text_sharp(
                    &mp_text,
                    (mp_bar_x + (bar_width - mp_text_w) / 2.0).floor(),
                    (mp_bar_y + bar_height * 0.78).floor(),
                    font_size,
                    TEXT_NORMAL,
                );

                // ===== PRAYER BAR =====
                let prayer_bar_x = bar_x;
                prayer_bar_y = mp_bar_y + bar_height + 4.0 * s;
                let prayer_ratio = if state.max_prayer_points > 0 {
                    state.prayer_points as f32 / state.max_prayer_points as f32
                } else {
                    0.0
                };
                let prayer_low = prayer_ratio < 0.2 && state.max_prayer_points > 0;

                let border_color = if prayer_low {
                    let flash = ((macroquad::time::get_time() * 2.0).sin() * 0.3 + 0.7) as f32;
                    Color::new(0.4 * flash + 0.2, 0.15, 0.15, 1.0)
                } else {
                    SLOT_INNER_SHADOW
                };
                draw_rectangle(
                    prayer_bar_x,
                    prayer_bar_y,
                    bar_width,
                    bar_height,
                    border_color,
                );
                draw_rectangle(
                    prayer_bar_x + 1.0,
                    prayer_bar_y + 1.0,
                    bar_width - 2.0,
                    bar_height - 2.0,
                    Color::new(0.08, 0.08, 0.10, 1.0),
                );

                let prayer_fill_w = (bar_width - 4.0) * prayer_ratio;
                if prayer_fill_w > 0.0 {
                    let prayer_color = Color::new(0.2, 0.7, 0.85, 1.0);
                    draw_rectangle(
                        prayer_bar_x + 2.0,
                        prayer_bar_y + 2.0,
                        prayer_fill_w,
                        bar_height - 4.0,
                        prayer_color,
                    );
                    draw_rectangle(
                        prayer_bar_x + 2.0,
                        prayer_bar_y + 2.0,
                        prayer_fill_w,
                        (bar_height - 4.0) / 2.0,
                        Color::new(1.0, 1.0, 1.0, 0.25),
                    );
                }

                let prayer_text = format!("{}/{}", state.prayer_points, state.max_prayer_points);
                let prayer_text_w = self.measure_text_sharp(&prayer_text, font_size).width;
                let prayer_text_color = if prayer_low {
                    let flash = ((macroquad::time::get_time() * 2.0).sin() * 0.15 + 0.85) as f32;
                    Color::new(1.0, 0.7 + 0.3 * flash, 0.7 + 0.3 * flash, 1.0)
                } else {
                    TEXT_NORMAL
                };
                self.draw_text_sharp(
                    &prayer_text,
                    (prayer_bar_x + (bar_width - prayer_text_w) / 2.0).floor(),
                    (prayer_bar_y + bar_height * 0.78).floor(),
                    font_size,
                    prayer_text_color,
                );
            }

            // ===== Gathering/Woodcutting status indicator (below prayer bar) =====
            let is_skilling = state.is_gathering || state.is_woodcutting;
            if is_skilling {
                let gather_y = prayer_bar_y + bar_height + 4.0 * s;
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
                draw_rectangle(bar_x, gather_y, bar_width, gather_h, bg_color);
                draw_rectangle_lines(bar_x, gather_y, bar_width, gather_h, 1.0, border_color);
                // Animated dots
                let dot_count = ((macroquad::time::get_time() * 2.0) as usize % 4) as usize;
                let dots = ".".repeat(dot_count);
                let label = format!("{}{}", action_name, dots);
                let label_w = self.measure_text_sharp(&label, 16.0).width;
                self.draw_text_sharp(
                    &label,
                    (bar_x + (bar_width - label_w) / 2.0).floor(),
                    (gather_y + gather_h * 0.68).floor(),
                    16.0,
                    text_color,
                );
            }

            // ===== Store Open status indicator (below gathering status or prayer bar) =====
            let has_stall_bar = state.ui_state.stall_active;
            if has_stall_bar {
                let stall_bar_y = prayer_bar_y
                    + bar_height
                    + 4.0 * s
                    + if is_skilling { 22.0 * s + 4.0 * s } else { 0.0 };
                let stall_h = 22.0 * s;
                let bg_color = Color::new(0.05, 0.18, 0.08, 0.7);
                let border_color = Color::new(0.2, 0.55, 0.25, 0.5);
                let text_color = Color::new(0.5, 0.9, 0.55, 0.9);
                draw_rectangle(bar_x, stall_bar_y, bar_width, stall_h, bg_color);
                draw_rectangle_lines(bar_x, stall_bar_y, bar_width, stall_h, 1.0, border_color);
                let label = "Store Open";
                let label_w = self.measure_text_sharp(label, 16.0).width;
                self.draw_text_sharp(
                    label,
                    (bar_x + (bar_width - label_w) / 2.0).floor(),
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
            let dash_bar_y = prayer_bar_y + bar_height + 4.0 * s + status_bars_offset;
            let current_time = macroquad::time::get_time();
            if state.dash_cooldown_end > current_time {
                let remaining = (state.dash_cooldown_end - current_time) as f32;
                let total_cooldown = 3.0f32;
                let progress = 1.0 - (remaining / total_cooldown).clamp(0.0, 1.0);
                let dash_h = 22.0 * s;

                // Background
                let bg_color = Color::new(0.15, 0.08, 0.15, 0.7);
                let border_color = Color::new(0.5, 0.25, 0.5, 0.5);
                draw_rectangle(bar_x, dash_bar_y, bar_width, dash_h, bg_color);
                draw_rectangle_lines(bar_x, dash_bar_y, bar_width, dash_h, 1.0, border_color);

                // Fill bar
                let fill_w = (bar_width - 4.0) * progress;
                if fill_w > 0.0 {
                    let fill_color = Color::new(0.6, 0.3, 0.8, 0.8);
                    draw_rectangle(
                        bar_x + 2.0,
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
                    (bar_x + (bar_width - text_w) / 2.0).floor(),
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
                let chip_row_y = prayer_bar_y
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
                let chip_gap = 4.0 * s;
                let mut chip_cursor_x = bar_x;
                let mut crh: f32 = 0.0;

                // Slayer task chip (first, if active)
                let has_slayer_chip = state.ui_state.slayer_current_task.is_some();
                let slayer_chip_x = chip_cursor_x;
                let (slayer_w, slayer_h) =
                    self.render_slayer_task_chip(state, slayer_chip_x, chip_row_y);
                if slayer_w > 0.0 {
                    chip_cursor_x += slayer_w + chip_gap;
                    crh = crh.max(slayer_h);
                }

                // Potion buff chips (after slayer task)
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

                // Combat style chip (after potion buffs)
                let (combat_w, combat_h) =
                    self.render_combat_style_chip(state, chip_cursor_x, chip_row_y);
                if combat_w > 0.0 {
                    crh = crh.max(combat_h);
                }

                has_any_chip = combat_w > 0.0 || has_slayer_chip || has_buff_chip;
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
            let drop_start_y = prayer_bar_y + bar_height + extra_offset + 145.0;
            self.xp_drop_pos.set(Some((10.0, drop_start_y)));
        }

        // Note: Interactive UI (inventory, crafting, dialogue, quick slots) is rendered
        // by render_interactive_ui() which is called by the main render loop.
        // XP drops are rendered after interactive UI via render_deferred_xp_drops().

        // Area banner (location name during transitions)
        if state.area_banner.is_visible() {
            self.render_area_banner(&state.area_banner.text, state.area_banner.opacity());
        }

        // Chat input box (when open) - scale with UI scale
        // Hidden on System tab (read-only channel)
        if state.ui_state.chat_open
            && !matches!(state.ui_state.chat_active_tab, ChatChannel::System)
        {
            let (_, input_sh) = virtual_screen_size();
            let input_x = 10.0;
            let scale = state.ui_state.ui_scale;
            let input_y = input_sh - EXP_BAR_GAP * scale - 24.0 * scale - 4.0 * scale;
            let input_width = 360.0 * scale;
            let input_height = 24.0 * scale;
            let text_padding = 5.0 * scale;
            let font_size: f32 = 16.0;

            // Channel indicator
            let (indicator, indicator_color) = match state.ui_state.chat_active_tab {
                ChatChannel::Local => ("[Public] ", WHITE),
                ChatChannel::Global => ("[Global] ", SKYBLUE),
                ChatChannel::System => ("[System] ", YELLOW),
            };
            let indicator_w = self.measure_text_sharp(indicator, font_size).width;
            let text_area_width = input_width - text_padding * 2.0 - 12.0 * scale - indicator_w; // Extra margin for scroll indicators + indicator

            // Background
            draw_rectangle(
                input_x,
                input_y,
                input_width,
                input_height,
                Color::from_rgba(0, 0, 0, 180),
            );

            let input_text = &state.ui_state.chat_input;
            let cursor_pos = state.ui_state.chat_cursor;
            // Draw channel indicator and visible text
            let text_y_offset = 17.0 * scale;
            let text_start_x = input_x + text_padding + indicator_w;
            self.draw_text_sharp(
                indicator,
                input_x + text_padding,
                input_y + text_y_offset,
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

                self.draw_text_sharp(
                    &visible_text,
                    text_start_x,
                    input_y + text_y_offset,
                    font_size,
                    WHITE,
                );

                // Draw scroll indicators if text is clipped
                if scroll_offset > 0 {
                    self.draw_text_sharp(
                        "<",
                        text_start_x - 8.0 * scale,
                        input_y + text_y_offset,
                        font_size,
                        GRAY,
                    );
                }
                if visible_end < char_count {
                    self.draw_text_sharp(
                        ">",
                        input_x + input_width - 10.0 * scale,
                        input_y + text_y_offset,
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
