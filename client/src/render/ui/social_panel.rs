//! Social panel rendering - Friends list, online players, friend requests

use macroquad::prelude::*;
use crate::game::{GameState, SocialTab};
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use super::super::Renderer;
use super::common::*;

/// Social panel dimensions (matches INV_WIDTH for consistency)
const SOCIAL_PANEL_WIDTH: f32 = 240.0;
const SOCIAL_PANEL_HEIGHT: f32 = 320.0;
const SOCIAL_HEADER_HEIGHT: f32 = 28.0;
const SOCIAL_TAB_HEIGHT: f32 = 26.0;
const SOCIAL_ROW_HEIGHT: f32 = 32.0;
const SOCIAL_PADDING: f32 = 8.0;
const SOCIAL_INPUT_HEIGHT: f32 = 28.0;
const SOCIAL_FONT_SIZE: f32 = 16.0;

/// Online status indicator colors
const STATUS_ONLINE: Color = Color::new(0.2, 0.8, 0.3, 1.0);   // Green
const STATUS_OFFLINE: Color = Color::new(0.4, 0.4, 0.45, 1.0); // Grey
const FRIEND_ICON_COLOR: Color = Color::new(0.9, 0.5, 0.5, 1.0); // Pink heart

impl Renderer {
    /// Render the social panel when open
    pub(crate) fn render_social_panel(&self, state: &GameState, hovered: &Option<UiElementId>, layout: &mut UiLayout) {
        if !state.ui_state.social_open {
            return;
        }

        let (screen_w, screen_h) = virtual_screen_size();
        let scale = state.ui_state.ui_scale;

        // Scaled dimensions
        let panel_width = SOCIAL_PANEL_WIDTH * scale;
        let panel_height_full = SOCIAL_PANEL_HEIGHT * scale;
        let frame_thickness = FRAME_THICKNESS * scale;
        let header_height = SOCIAL_HEADER_HEIGHT * scale;
        let tab_height = SOCIAL_TAB_HEIGHT * scale;
        let row_height = SOCIAL_ROW_HEIGHT * scale;
        let padding = SOCIAL_PADDING * scale;
        let input_height = SOCIAL_INPUT_HEIGHT * scale;
        let button_size = MENU_BUTTON_SIZE * scale;
        let exp_bar_gap = EXP_BAR_GAP * scale;

        // Position panel on right side, above the menu buttons
        let panel_x = screen_w - panel_width - 8.0;
        let button_area_height = button_size + exp_bar_gap;

        // Calculate the minimum Y the panel can reach
        let min_panel_y = 4.0;
        let max_available_height = screen_h - button_area_height - 8.0 - min_panel_y;

        // Clamp panel height if it would exceed available space
        let panel_height = panel_height_full.min(max_available_height);
        let panel_y = screen_h - button_area_height - panel_height - 8.0;

        // Draw panel frame
        self.draw_panel_frame(panel_x, panel_y, panel_width, panel_height);
        self.draw_corner_accents(panel_x, panel_y, panel_width, panel_height);

        // Header
        let header_x = panel_x + frame_thickness;
        let header_y = panel_y + frame_thickness;
        let header_w = panel_width - frame_thickness * 2.0;

        draw_rectangle(header_x, header_y, header_w, header_height, HEADER_BG);
        draw_line(
            header_x + 6.0 * scale,
            header_y + header_height,
            header_x + header_w - 6.0 * scale,
            header_y + header_height,
            1.0,
            HEADER_BORDER,
        );

        // Header text with pending request count badge
        let pending_count = state.social_state.pending_request_count;
        let header_text = if pending_count > 0 {
            format!("Social ({})", pending_count)
        } else {
            "Social".to_string()
        };
        let text_dims = self.measure_text_sharp(&header_text, 16.0);
        let text_x = header_x + (header_w - text_dims.width) / 2.0;
        self.draw_text_sharp(&header_text, text_x, header_y + (header_height + 12.0) / 2.0, 16.0, TEXT_TITLE);

        // Tabs
        let tab_y = header_y + header_height + 2.0;
        let tab_width = (header_w - 4.0) / 3.0;

        let tabs = [
            (SocialTab::Nearby, "Nearby", UiElementId::SocialTabNearby),
            (SocialTab::Online, "Online", UiElementId::SocialTabOnline),
            (SocialTab::Friends, "Friends", UiElementId::SocialTabFriends),
        ];

        for (i, (tab, label, element_id)) in tabs.iter().enumerate() {
            let tab_x = header_x + 2.0 + i as f32 * tab_width;
            let bounds = Rect::new(tab_x, tab_y, tab_width - 2.0, tab_height);
            layout.add(element_id.clone(), bounds);

            let is_active = state.social_state.active_tab == *tab;
            let is_hovered = matches!(hovered, Some(id) if id == element_id);

            let bg_color = if is_active {
                SLOT_HOVER_BG
            } else if is_hovered {
                SLOT_BG_FILLED
            } else {
                SLOT_BG_EMPTY
            };
            let border_color = if is_active { SLOT_HOVER_BORDER } else { SLOT_BORDER };

            draw_rectangle(tab_x, tab_y, tab_width - 2.0, tab_height, border_color);
            draw_rectangle(tab_x + 1.0, tab_y + 1.0, tab_width - 4.0, tab_height - 2.0, bg_color);

            let label_dims = self.measure_text_sharp(label, SOCIAL_FONT_SIZE);
            let label_x = tab_x + (tab_width - 2.0 - label_dims.width) / 2.0;
            let text_color = if is_active { TEXT_TITLE } else { TEXT_NORMAL };
            self.draw_text_sharp(label, label_x, tab_y + (tab_height + 10.0) / 2.0, SOCIAL_FONT_SIZE, text_color);
        }

        // Content area
        let content_y = tab_y + tab_height + padding;
        let content_h = panel_height - frame_thickness * 2.0 - header_height - tab_height - padding * 2.0;

        match state.social_state.active_tab {
            SocialTab::Nearby => {
                self.render_nearby_players(state, hovered, layout, header_x, content_y, header_w, content_h, row_height, padding, scale);
            }
            SocialTab::Online => {
                self.render_online_players(state, hovered, layout, header_x, content_y, header_w, content_h, row_height, padding, scale);
            }
            SocialTab::Friends => {
                self.render_friends_tab(state, hovered, layout, header_x, content_y, header_w, content_h, row_height, padding, input_height, scale);
            }
        }
    }

    /// Render nearby players list
    fn render_nearby_players(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        row_height: f32,
        padding: f32,
        scale: f32,
    ) {
        // Build nearby players list from current room players
        let local_id = state.local_player_id.as_ref();
        let nearby: Vec<_> = state.players.values()
            .filter(|p| Some(&p.id) != local_id)
            .collect();

        if nearby.is_empty() {
            self.draw_text_sharp("No players nearby", x + padding, y + 20.0, SOCIAL_FONT_SIZE, TEXT_DIM);
            return;
        }

        // Add scroll area for touch detection
        let scroll_area = Rect::new(x, y, width, height);
        layout.add(UiElementId::SocialScrollArea, scroll_area);

        // Calculate scroll bounds
        let total_content_height = nearby.len() as f32 * row_height;
        let max_scroll = (total_content_height - height).max(0.0);
        let scroll_offset = state.social_state.list_scroll_offset.clamp(0.0, max_scroll);

        // Calculate visible range
        let first_visible = (scroll_offset / row_height).floor() as usize;
        let visible_count = (height / row_height).ceil() as usize + 1;
        let last_visible = (first_visible + visible_count).min(nearby.len());

        let mut row_y = y - (scroll_offset % row_height);
        for i in first_visible..last_visible {
            if row_y + row_height < y {
                row_y += row_height;
                continue;
            }
            if row_y > y + height {
                break;
            }

            let player = &nearby[i];

            // Only add clickable bounds if row is fully visible
            if row_y >= y && row_y + row_height <= y + height {
                let bounds = Rect::new(x + padding, row_y, width - padding * 2.0, row_height);
                layout.add(UiElementId::SocialPlayerRow(i), bounds);
            }

            let is_hovered = matches!(hovered, Some(UiElementId::SocialPlayerRow(idx)) if *idx == i);
            let bg_color = if is_hovered { SLOT_HOVER_BG } else { SLOT_BG_EMPTY };

            draw_rectangle(x + padding, row_y, width - padding * 2.0, row_height - 2.0, bg_color);

            // Check if this player is a friend
            let player_char_id = player.id.strip_prefix("char_")
                .and_then(|s| s.parse::<i64>().ok());
            let is_friend = player_char_id
                .map(|id| state.social_state.friends.iter().any(|f| f.id == id))
                .unwrap_or(false);

            // Player name (no icon for nearby - they're all here)
            let name_x = x + padding + 8.0;
            self.draw_text_sharp(&player.name, name_x, row_y + (row_height + 10.0) / 2.0, SOCIAL_FONT_SIZE, TEXT_NORMAL);

            // Friend indicator (heart on the right)
            if is_friend {
                let heart_x = x + width - padding - 16.0;
                self.draw_text_sharp("♥", heart_x, row_y + (row_height + 10.0) / 2.0, SOCIAL_FONT_SIZE, FRIEND_ICON_COLOR);
            }

            row_y += row_height;
        }

        // Draw scrollbar if needed
        if max_scroll > 0.0 {
            let scrollbar_width = 4.0 * scale;
            let scrollbar_x = x + width - scrollbar_width - 2.0;
            let scrollbar_height = height;
            let thumb_size = (height / total_content_height).min(1.0);
            let thumb_height = scrollbar_height * thumb_size;
            let thumb_y = y + (scrollbar_height - thumb_height) * (scroll_offset / max_scroll);

            // Track
            draw_rectangle(scrollbar_x, y, scrollbar_width, scrollbar_height, Color::new(0.1, 0.1, 0.12, 0.5));
            // Thumb
            draw_rectangle(scrollbar_x, thumb_y, scrollbar_width, thumb_height, Color::new(0.4, 0.4, 0.45, 0.8));
        }
    }

    /// Render online players list
    fn render_online_players(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        row_height: f32,
        padding: f32,
        scale: f32,
    ) {
        if state.social_state.online_players.is_empty() {
            self.draw_text_sharp("Loading...", x + padding, y + 20.0, SOCIAL_FONT_SIZE, TEXT_DIM);
            return;
        }

        // Add scroll area for touch detection
        let scroll_area = Rect::new(x, y, width, height);
        layout.add(UiElementId::SocialScrollArea, scroll_area);

        let total_items = state.social_state.online_players.len();

        // Calculate scroll bounds
        let total_content_height = total_items as f32 * row_height;
        let max_scroll = (total_content_height - height).max(0.0);
        let scroll_offset = state.social_state.list_scroll_offset.clamp(0.0, max_scroll);

        // Calculate visible range
        let first_visible = (scroll_offset / row_height).floor() as usize;
        let visible_count = (height / row_height).ceil() as usize + 1;
        let last_visible = (first_visible + visible_count).min(total_items);

        let mut row_y = y - (scroll_offset % row_height);
        for i in first_visible..last_visible {
            if row_y + row_height < y {
                row_y += row_height;
                continue;
            }
            if row_y > y + height {
                break;
            }

            let player = &state.social_state.online_players[i];

            // Only add clickable bounds if row is fully visible
            if row_y >= y && row_y + row_height <= y + height {
                let bounds = Rect::new(x + padding, row_y, width - padding * 2.0, row_height);
                layout.add(UiElementId::SocialPlayerRow(i), bounds);
            }

            let is_hovered = matches!(hovered, Some(UiElementId::SocialPlayerRow(idx)) if *idx == i);
            let bg_color = if is_hovered { SLOT_HOVER_BG } else { SLOT_BG_EMPTY };

            draw_rectangle(x + padding, row_y, width - padding * 2.0, row_height - 2.0, bg_color);

            // Online status dot
            let dot_x = x + padding + 8.0;
            let dot_y = row_y + row_height / 2.0;
            draw_circle(dot_x, dot_y, 4.0 * scale, STATUS_ONLINE);

            // Player name
            let name_x = dot_x + 12.0 * scale;
            self.draw_text_sharp(&player.name, name_x, row_y + (row_height + 10.0) / 2.0, SOCIAL_FONT_SIZE, TEXT_NORMAL);

            // Friend indicator or Add button
            if player.is_friend {
                let heart_x = x + width - padding - 16.0;
                self.draw_text_sharp("♥", heart_x, row_y + (row_height + 10.0) / 2.0, SOCIAL_FONT_SIZE, FRIEND_ICON_COLOR);
            }

            row_y += row_height;
        }

        // Draw scrollbar if needed
        if max_scroll > 0.0 {
            let scrollbar_width = 4.0 * scale;
            let scrollbar_x = x + width - scrollbar_width - 2.0;
            let scrollbar_height = height;
            let thumb_size = (height / total_content_height).min(1.0);
            let thumb_height = scrollbar_height * thumb_size;
            let thumb_y = y + (scrollbar_height - thumb_height) * (scroll_offset / max_scroll);

            // Track
            draw_rectangle(scrollbar_x, y, scrollbar_width, scrollbar_height, Color::new(0.1, 0.1, 0.12, 0.5));
            // Thumb
            draw_rectangle(scrollbar_x, thumb_y, scrollbar_width, thumb_height, Color::new(0.4, 0.4, 0.45, 0.8));
        }
    }

    /// Render friends tab with pending requests, friends list, and add input
    fn render_friends_tab(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        row_height: f32,
        padding: f32,
        input_height: f32,
        scale: f32,
    ) {
        let mut current_y = y;

        // Pending friend requests section (if any)
        if !state.social_state.pending_requests.is_empty() {
            self.draw_text_sharp("Friend Requests", x + padding, current_y + 14.0, SOCIAL_FONT_SIZE, TEXT_TITLE);
            current_y += 20.0;

            for (i, request) in state.social_state.pending_requests.iter().enumerate() {
                if current_y + row_height > y + height - input_height - padding {
                    break;
                }

                let row_bg = Rect::new(x + padding, current_y, width - padding * 2.0, row_height);
                draw_rectangle(row_bg.x, row_bg.y, row_bg.w, row_bg.h - 2.0, SLOT_BG_EMPTY);

                // Requester name
                self.draw_text_sharp(&request.from_name, x + padding + 8.0, current_y + (row_height + 10.0) / 2.0, SOCIAL_FONT_SIZE, TEXT_NORMAL);

                // Button dimensions
                let btn_size = 24.0 * scale;
                let btn_y = current_y + (row_height - btn_size) / 2.0;

                // Accept button (green with checkmark)
                let accept_x = x + width - padding - btn_size * 2.0 - 4.0 * scale;
                let accept_bounds = Rect::new(accept_x, btn_y, btn_size, btn_size);
                layout.add(UiElementId::SocialRequestAccept(i), accept_bounds);
                let accept_hovered = matches!(hovered, Some(UiElementId::SocialRequestAccept(idx)) if *idx == i);
                let accept_bg = if accept_hovered { STATUS_ONLINE } else { Color::new(0.15, 0.4, 0.2, 1.0) };
                draw_rectangle(accept_bounds.x, accept_bounds.y, accept_bounds.w, accept_bounds.h, accept_bg);
                // Center the checkmark
                let check_dims = self.measure_text_sharp("✓", SOCIAL_FONT_SIZE);
                let check_x = accept_x + (btn_size - check_dims.width) / 2.0;
                let check_y = btn_y + (btn_size + check_dims.height) / 2.0;
                self.draw_text_sharp("✓", check_x, check_y, SOCIAL_FONT_SIZE, WHITE);

                // Decline button (red with X)
                let decline_x = accept_x + btn_size + 4.0 * scale;
                let decline_bounds = Rect::new(decline_x, btn_y, btn_size, btn_size);
                layout.add(UiElementId::SocialRequestDecline(i), decline_bounds);
                let decline_hovered = matches!(hovered, Some(UiElementId::SocialRequestDecline(idx)) if *idx == i);
                let decline_bg = if decline_hovered { Color::new(0.9, 0.25, 0.25, 1.0) } else { Color::new(0.5, 0.18, 0.18, 1.0) };
                draw_rectangle(decline_bounds.x, decline_bounds.y, decline_bounds.w, decline_bounds.h, decline_bg);
                // Center the X
                let x_dims = self.measure_text_sharp("X", SOCIAL_FONT_SIZE);
                let x_icon_x = decline_x + (btn_size - x_dims.width) / 2.0;
                let x_icon_y = btn_y + (btn_size + x_dims.height) / 2.0;
                self.draw_text_sharp("X", x_icon_x, x_icon_y, SOCIAL_FONT_SIZE, WHITE);

                current_y += row_height;
            }

            // Divider
            current_y += 4.0;
            draw_line(x + padding, current_y, x + width - padding, current_y, 1.0, HEADER_BORDER);
            current_y += 8.0;
        }

        // Friends list header
        let friends_label = format!("Friends ({})", state.social_state.friends.len());
        self.draw_text_sharp(&friends_label, x + padding, current_y + 14.0, SOCIAL_FONT_SIZE, TEXT_TITLE);
        current_y += 20.0;

        // Friends list (scrollable)
        if state.social_state.friends.is_empty() {
            self.draw_text_sharp("No friends yet", x + padding, current_y + 14.0, SOCIAL_FONT_SIZE, TEXT_DIM);
        } else {
            // Available height for friends list
            let friends_area_height = y + height - input_height - padding * 2.0 - current_y;

            // Add scroll area for touch detection
            let scroll_area = Rect::new(x, current_y, width, friends_area_height);
            layout.add(UiElementId::SocialScrollArea, scroll_area);

            let total_items = state.social_state.friends.len();
            let total_content_height = total_items as f32 * row_height;
            let max_scroll = (total_content_height - friends_area_height).max(0.0);
            let scroll_offset = state.social_state.friends_scroll_offset.clamp(0.0, max_scroll);

            // Calculate visible range
            let first_visible = (scroll_offset / row_height).floor() as usize;
            let visible_count = (friends_area_height / row_height).ceil() as usize + 1;
            let last_visible = (first_visible + visible_count).min(total_items);

            let list_start_y = current_y;
            let mut row_y = current_y - (scroll_offset % row_height);

            for i in first_visible..last_visible {
                if row_y + row_height < list_start_y {
                    row_y += row_height;
                    continue;
                }
                if row_y > list_start_y + friends_area_height {
                    break;
                }

                let friend = &state.social_state.friends[i];

                // Only add clickable bounds if row is fully visible
                if row_y >= list_start_y && row_y + row_height <= list_start_y + friends_area_height {
                    let bounds = Rect::new(x + padding, row_y, width - padding * 2.0, row_height);
                    layout.add(UiElementId::SocialFriendRow(i), bounds);
                }

                let is_hovered = matches!(hovered, Some(UiElementId::SocialFriendRow(idx)) if *idx == i);
                let bg_color = if is_hovered { SLOT_HOVER_BG } else { SLOT_BG_EMPTY };

                draw_rectangle(x + padding, row_y, width - padding * 2.0, row_height - 2.0, bg_color);

                // Online status dot
                let dot_x = x + padding + 8.0;
                let dot_y = row_y + row_height / 2.0;
                let dot_color = if friend.online { STATUS_ONLINE } else { STATUS_OFFLINE };
                draw_circle(dot_x, dot_y, 4.0 * scale, dot_color);

                // Friend name
                let name_x = dot_x + 12.0 * scale;
                let name_color = if friend.online { TEXT_NORMAL } else { TEXT_DIM };
                self.draw_text_sharp(&friend.name, name_x, row_y + (row_height + 10.0) / 2.0, SOCIAL_FONT_SIZE, name_color);

                row_y += row_height;
            }

            // Draw scrollbar if needed
            if max_scroll > 0.0 {
                let scrollbar_width = 4.0 * scale;
                let scrollbar_x = x + width - scrollbar_width - 2.0;
                let scrollbar_height = friends_area_height;
                let thumb_size = (friends_area_height / total_content_height).min(1.0);
                let thumb_height = scrollbar_height * thumb_size;
                let thumb_y = list_start_y + (scrollbar_height - thumb_height) * (scroll_offset / max_scroll);

                // Track
                draw_rectangle(scrollbar_x, list_start_y, scrollbar_width, scrollbar_height, Color::new(0.1, 0.1, 0.12, 0.5));
                // Thumb
                draw_rectangle(scrollbar_x, thumb_y, scrollbar_width, thumb_height, Color::new(0.4, 0.4, 0.45, 0.8));
            }
        }

        // Add friend input at bottom
        let input_y = y + height - input_height - padding;
        draw_rectangle(x + padding, input_y, width - padding * 2.0 - 50.0, input_height, SLOT_BG_EMPTY);
        draw_rectangle(x + padding + 1.0, input_y + 1.0, width - padding * 2.0 - 52.0, input_height - 2.0, PANEL_BG_DARK);

        // Input text or placeholder
        let input_text = if state.social_state.add_friend_input.is_empty() && !state.social_state.add_friend_focused {
            "Add by name..."
        } else {
            &state.social_state.add_friend_input
        };
        let text_color = if state.social_state.add_friend_input.is_empty() && !state.social_state.add_friend_focused { TEXT_DIM } else { TEXT_NORMAL };

        // Draw input text with cursor if focused
        let text_x = x + padding + 6.0;
        let text_y = input_y + (input_height + 10.0) / 2.0;
        self.draw_text_sharp(input_text, text_x, text_y, SOCIAL_FONT_SIZE, text_color);

        // Draw cursor if focused
        if state.social_state.add_friend_focused {
            let cursor_x = text_x + self.measure_text_sharp(&state.social_state.add_friend_input, SOCIAL_FONT_SIZE).width;
            let cursor_y = input_y + 4.0;
            let cursor_height = input_height - 8.0;
            // Blinking cursor (blink every 0.5s)
            let time = macroquad::time::get_time();
            if (time * 2.0) as i32 % 2 == 0 {
                draw_rectangle(cursor_x, cursor_y, 2.0, cursor_height, TEXT_NORMAL);
            }
        }

        // Draw focused border
        if state.social_state.add_friend_focused {
            draw_rectangle_lines(x + padding, input_y, width - padding * 2.0 - 50.0, input_height, 2.0, SLOT_HOVER_BORDER);
        }

        // Input bounds
        let input_bounds = Rect::new(x + padding, input_y, width - padding * 2.0 - 50.0, input_height);
        layout.add(UiElementId::SocialAddFriendInput, input_bounds);

        // Send button
        let send_x = x + width - padding - 45.0;
        let send_bounds = Rect::new(send_x, input_y, 40.0, input_height);
        layout.add(UiElementId::SocialAddFriendButton, send_bounds);

        let send_hovered = matches!(hovered, Some(UiElementId::SocialAddFriendButton));
        let send_bg = if send_hovered { SLOT_HOVER_BG } else { SLOT_BG_FILLED };
        draw_rectangle(send_x, input_y, 40.0, input_height, SLOT_BORDER);
        draw_rectangle(send_x + 1.0, input_y + 1.0, 38.0, input_height - 2.0, send_bg);
        self.draw_text_sharp("Add", send_x + 8.0, input_y + (input_height + 10.0) / 2.0, SOCIAL_FONT_SIZE, TEXT_NORMAL);
    }

    /// Render notification badge on social button when there are pending requests
    pub(crate) fn render_social_badge(&self, state: &GameState, button_x: f32, button_y: f32, scale: f32) {
        let pending_count = state.social_state.pending_request_count;
        if pending_count == 0 {
            return;
        }

        // Draw red notification dot in top-right corner of button
        let badge_radius = 10.0 * scale;
        let badge_x = button_x + MENU_BUTTON_SIZE * scale - badge_radius + 2.0;
        let badge_y = button_y + badge_radius - 2.0;

        // Badge background (darker outer, brighter inner)
        draw_circle(badge_x, badge_y, badge_radius, Color::new(0.6, 0.1, 0.1, 1.0));
        draw_circle(badge_x, badge_y, badge_radius - 1.5, Color::new(0.9, 0.15, 0.15, 1.0));

        // Badge number with 16pt font, centered
        if pending_count <= 9 {
            let count_text = pending_count.to_string();
            let text_dims = self.measure_text_sharp(&count_text, 16.0);
            // Center text both horizontally and vertically in the badge
            let text_x = badge_x - text_dims.width / 2.0;
            let text_y = badge_y + 6.0; // Adjust for baseline
            self.draw_text_sharp(&count_text, text_x, text_y, 16.0, WHITE);
        }
    }
}
