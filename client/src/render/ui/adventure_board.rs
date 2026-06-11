use super::super::Renderer;
use super::common::*;
use crate::game::{ActiveDialogue, GameState};
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use macroquad::prelude::*;

fn kind_accent(kind_id: &str) -> Color {
    match kind_id {
        "farming" => Color::new(0.40, 0.80, 0.40, 1.0),
        "mining" => Color::new(0.55, 0.60, 0.75, 1.0),
        "woodcutting" => Color::new(0.72, 0.58, 0.38, 1.0),
        "fishing" => Color::new(0.40, 0.65, 0.85, 1.0),
        "smithing" => Color::new(0.85, 0.45, 0.35, 1.0),
        "cooking" => Color::new(0.90, 0.55, 0.30, 1.0),
        "fletching" => Color::new(0.60, 0.75, 0.45, 1.0),
        "leatherworking" => Color::new(0.70, 0.50, 0.35, 1.0),
        _ => FRAME_ACCENT,
    }
}

impl Renderer {
    pub(crate) fn render_adventure_board_dialogue(
        &self,
        state: &GameState,
        _dialogue: &ActiveDialogue,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
    ) {
        let Some(board) = state.ui_state.adventure_board.as_ref() else {
            return;
        };

        let (sw, sh) = virtual_screen_size();
        let s = state.ui_state.ui_scale;
        let compact = sw < 980.0 * s;

        // Dark overlay matching other panels
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));

        let panel_w = if compact {
            (sw - 20.0).max(320.0)
        } else {
            sw.min(780.0 * s)
        };
        let panel_h = if compact {
            sh.min(540.0 * s)
        } else {
            sh.min(470.0 * s)
        };
        let panel_x = ((sw - panel_w) * 0.5).floor();
        let panel_y = ((sh - panel_h) * 0.5).floor();

        // Use shared panel frame + corner accents
        self.draw_panel_frame(panel_x, panel_y, panel_w, panel_h);
        self.draw_corner_accents(panel_x, panel_y, panel_w, panel_h);

        // ===== HEADER ROW =====
        let header_h = 36.0 * s;
        let header_y = panel_y + 6.0 * s;
        let header_center_y = header_y + header_h * 0.5;

        // Title (left)
        self.draw_text_sharp(
            "ADVENTURE BOARD",
            panel_x + 18.0 * s,
            header_center_y + 5.0 * s,
            16.0,
            TEXT_TITLE,
        );

        // Close button (right, same style as shop/crafting)
        let close_btn_size = 22.0 * s;
        let close_btn_x = panel_x + panel_w - close_btn_size - 10.0 * s;
        let close_btn_y = header_center_y - close_btn_size * 0.5;
        let close_bounds = Rect::new(close_btn_x, close_btn_y, close_btn_size, close_btn_size);
        layout.add(UiElementId::DialogueClose, close_bounds);
        let close_hovered = matches!(hovered, Some(UiElementId::DialogueClose));
        let (close_bg, close_border) = if close_hovered {
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
        let cross_color = if close_hovered { TEXT_TITLE } else { TEXT_DIM };
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

        // Daily info (right, before close button, vertically centered)
        let now_secs = miniquad::date::now() as u64;
        let secs_into_day = now_secs % 86400;
        let remaining = 86400 - secs_into_day;
        let hours = remaining / 3600;
        let minutes = (remaining % 3600) / 60;
        let reset_text = format!(
            "Contracts: {}/{}  |  Resets in {:02}:{:02}",
            board.daily_contracts_completed, board.daily_contract_limit, hours, minutes
        );
        let reset_w = self.measure_text_sharp(&reset_text, 16.0).width;
        self.draw_text_sharp(
            &reset_text,
            close_btn_x - reset_w - 10.0 * s,
            header_center_y + 5.0 * s,
            16.0,
            TEXT_DIM,
        );

        // ===== TAB BAR =====
        let tab_y = panel_y + 42.0 * s;
        let tab_h = 24.0 * s;
        let tab_w = 100.0 * s;
        let tab_gap = 4.0 * s;
        let tab_x_start = panel_x + 12.0 * s;
        let current_tab = state.ui_state.adventure_board_tab;

        let tab_labels = [
            ("CONTRACTS", UiElementId::AdventureBoardTabContracts),
            ("ORDERS", UiElementId::AdventureBoardTabOrders),
        ];
        for (i, (label, element_id)) in tab_labels.iter().enumerate() {
            let tx = tab_x_start + i as f32 * (tab_w + tab_gap);
            let tab_rect = Rect::new(tx, tab_y, tab_w, tab_h);
            layout.add(element_id.clone(), tab_rect);
            let is_active = i as u8 == current_tab;
            let tab_hovered = matches!(hovered, Some(ref h) if *h == *element_id);

            let bg = if is_active {
                SLOT_HOVER_BG
            } else if tab_hovered {
                Color::new(0.14, 0.14, 0.20, 1.0)
            } else {
                SLOT_BG_EMPTY
            };
            let border = if is_active {
                SLOT_SELECTED_BORDER
            } else {
                SLOT_BORDER
            };
            draw_rectangle(tx, tab_y, tab_w, tab_h, border);
            draw_rectangle(tx + 1.0, tab_y + 1.0, tab_w - 2.0, tab_h - 2.0, bg);
            if is_active {
                draw_rectangle(
                    tx + 2.0,
                    tab_y + tab_h - 3.0,
                    tab_w - 4.0,
                    2.0,
                    FRAME_ACCENT,
                );
            }
            let label_w = self.measure_text_sharp(label, 16.0).width;
            self.draw_text_sharp(
                label,
                tx + (tab_w - label_w) * 0.5,
                tab_y + tab_h * 0.70,
                16.0,
                if is_active { TEXT_TITLE } else { TEXT_DIM },
            );
        }

        if current_tab == 0 {
            self.render_contracts_tab(
                state, board, hovered, layout, panel_x, panel_y, panel_w, panel_h, s, compact,
            );
        } else {
            self.render_orders_tab(
                state, board, hovered, layout, panel_x, panel_y, panel_w, panel_h, s, compact,
            );
        }
    }

    fn render_contracts_tab(
        &self,
        state: &GameState,
        board: &crate::game::AdventureBoardPanelState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        panel_x: f32,
        panel_y: f32,
        panel_w: f32,
        panel_h: f32,
        s: f32,
        compact: bool,
    ) {
        // ===== STAT CARDS =====
        let stats_y = panel_y + 72.0 * s;
        let stats_x = panel_x + 12.0 * s;
        let stats_gap = 8.0 * s;
        let stats_w = if compact {
            (panel_w - 24.0 * s - stats_gap * 2.0) / 3.0
        } else {
            120.0 * s
        };
        let stat_cards = [
            ("Completed", board.stats.contracts_completed.to_string()),
            ("Total XP", board.stats.total_xp_earned.to_string()),
            ("Total Gold", board.stats.total_gold_earned.to_string()),
        ];
        for (idx, (label, value)) in stat_cards.iter().enumerate() {
            let card_x = stats_x + idx as f32 * (stats_w + stats_gap);
            draw_rectangle(card_x, stats_y, stats_w, 42.0 * s, SLOT_BORDER);
            draw_rectangle(
                card_x + 1.0,
                stats_y + 1.0,
                stats_w - 2.0,
                40.0 * s,
                SLOT_BG_EMPTY,
            );
            self.draw_text_sharp(label, card_x + 8.0 * s, stats_y + 15.0 * s, 16.0, TEXT_DIM);
            self.draw_text_sharp(
                value,
                card_x + 8.0 * s,
                stats_y + 33.0 * s,
                16.0,
                TEXT_TITLE,
            );
        }

        // ===== LAYOUT =====
        let content_y = stats_y + 52.0 * s;
        let content_h = panel_y + panel_h - content_y - 12.0 * s;

        let left_w = if compact {
            panel_w - 24.0 * s
        } else {
            220.0 * s
        };
        let left_x = panel_x + 12.0 * s;
        let left_y = content_y;
        let left_h = if compact {
            let lane_count = board.offers.len().max(1) as f32;
            let row_h = 52.0 * s;
            let gap = 8.0 * s;
            (lane_count * (row_h + gap) + 16.0 * s).min(content_h * 0.45)
        } else {
            content_h
        };

        // Left panel: offer list
        draw_rectangle(left_x, left_y, left_w, left_h, SLOT_BORDER);
        draw_rectangle(
            left_x + 1.0,
            left_y + 1.0,
            left_w - 2.0,
            left_h - 2.0,
            SLOT_BG_EMPTY,
        );

        let selected_idx = state
            .ui_state
            .adventure_board_selected_offer
            .min(board.offers.len().saturating_sub(1));
        let selected_offer = board.offers.get(selected_idx);

        let row_h = 52.0 * s;
        let row_gap = 8.0 * s;
        let mut row_y = left_y + 8.0 * s;
        for (idx, offer) in board.offers.iter().enumerate() {
            let is_selected = idx == selected_idx;
            let hovered_card =
                matches!(hovered, Some(UiElementId::AdventureBoardOffer(i)) if *i == idx);
            let accent = kind_accent(&offer.kind_id);

            let row_bounds = Rect::new(left_x + 6.0 * s, row_y, left_w - 12.0 * s, row_h);
            layout.add(UiElementId::AdventureBoardOffer(idx), row_bounds);

            let row_bg = if is_selected {
                SLOT_HOVER_BG
            } else if hovered_card {
                Color::new(0.14, 0.14, 0.20, 1.0)
            } else {
                Color::new(0.10, 0.10, 0.14, 1.0)
            };
            let row_border = if is_selected {
                SLOT_SELECTED_BORDER
            } else {
                SLOT_BORDER
            };
            draw_rectangle(
                row_bounds.x,
                row_bounds.y,
                row_bounds.w,
                row_bounds.h,
                row_border,
            );
            draw_rectangle(
                row_bounds.x + 1.0,
                row_bounds.y + 1.0,
                row_bounds.w - 2.0,
                row_bounds.h - 2.0,
                row_bg,
            );

            // Accent bar on the left
            draw_rectangle(
                row_bounds.x + 3.0,
                row_bounds.y + 3.0,
                3.0 * s,
                row_bounds.h - 6.0,
                accent,
            );

            let text_x = row_bounds.x + 10.0 * s;
            let skill_text = format!("Skill {}", offer.skill_level);
            let skill_w = self.measure_text_sharp(&skill_text, 16.0).width;
            let title_max_w = (row_bounds.w - 18.0 * s - skill_w - 8.0 * s).max(40.0 * s);
            let title_text =
                self.truncate_text_to_width(&offer.kind_name.to_uppercase(), title_max_w, 16.0);

            self.draw_text_sharp(
                &title_text,
                text_x,
                row_bounds.y + row_h * 0.36,
                16.0,
                if is_selected { TEXT_TITLE } else { TEXT_NORMAL },
            );
            self.draw_text_sharp(
                &skill_text,
                row_bounds.x + row_bounds.w - skill_w - 8.0 * s,
                row_bounds.y + row_h * 0.36,
                16.0,
                TEXT_DIM,
            );

            let desc_max_w = (row_bounds.w - 18.0 * s).max(40.0 * s);
            let desc_text = self.truncate_text_to_width(&offer.description, desc_max_w, 16.0);
            self.draw_text_sharp(
                &desc_text,
                text_x,
                row_bounds.y + row_h * 0.70,
                16.0,
                TEXT_DIM,
            );

            row_y += row_h + row_gap;
        }

        // ===== RIGHT PANELS =====
        let right_x = if compact {
            panel_x + 12.0 * s
        } else {
            left_x + left_w + 12.0 * s
        };
        let right_y = if compact {
            left_y + left_h + 8.0 * s
        } else {
            content_y
        };
        let right_w = if compact {
            panel_w - 24.0 * s
        } else {
            panel_w - (right_x - panel_x) - 12.0 * s
        };

        // Split right side into detail + active panels
        let active_split = if compact {
            0.0
        } else {
            (right_w * 0.38).floor()
        };
        let detail_w = if compact {
            right_w
        } else {
            right_w - active_split - 10.0 * s
        };
        let active_x = if compact {
            right_x
        } else {
            right_x + detail_w + 10.0 * s
        };
        let active_y = if compact {
            right_y + 200.0 * s
        } else {
            right_y
        };
        let active_w = if compact { right_w } else { active_split };
        let detail_h = if compact { 190.0 * s } else { content_h };
        let active_h = if compact {
            content_h - (active_y - content_y)
        } else {
            content_h
        };

        // ===== DETAIL PANEL (selected offer's difficulties) =====
        draw_rectangle(right_x, right_y, detail_w, detail_h, SLOT_BORDER);
        draw_rectangle(
            right_x + 1.0,
            right_y + 1.0,
            detail_w - 2.0,
            detail_h - 2.0,
            Color::new(0.09, 0.09, 0.13, 1.0),
        );

        if let Some(offer) = selected_offer {
            let accent = kind_accent(&offer.kind_id);

            let header_text = format!("{} CONTRACTS", offer.kind_name.to_uppercase());
            let header_max_w = (detail_w - 24.0 * s).max(40.0 * s);
            let header_display = self.truncate_text_to_width(&header_text, header_max_w, 16.0);
            self.draw_text_sharp(
                &header_display,
                right_x + 12.0 * s,
                right_y + 24.0 * s,
                16.0,
                TEXT_TITLE,
            );

            let desc_max_w = (detail_w - 24.0 * s).max(40.0 * s);
            let desc_display = self.truncate_text_to_width(&offer.description, desc_max_w, 16.0);
            self.draw_text_sharp(
                &desc_display,
                right_x + 12.0 * s,
                right_y + 42.0 * s,
                16.0,
                TEXT_DIM,
            );

            draw_line(
                right_x + 12.0 * s,
                right_y + 52.0 * s,
                right_x + detail_w - 12.0 * s,
                right_y + 52.0 * s,
                1.0,
                HEADER_BORDER,
            );

            let row_y_start = right_y + 60.0 * s;
            let diff_row_h = 46.0 * s;
            let diff_row_gap = 8.0 * s;
            let has_active = board.active_contract.is_some();
            for (idx, difficulty) in offer.difficulties.iter().enumerate() {
                let dy = row_y_start + idx as f32 * (diff_row_h + diff_row_gap);
                let button_id = UiElementId::AdventureBoardDifficulty(idx);
                let row_rect = Rect::new(right_x + 10.0 * s, dy, detail_w - 20.0 * s, diff_row_h);
                layout.add(button_id.clone(), row_rect);
                let row_hovered =
                    matches!(hovered, Some(UiElementId::AdventureBoardDifficulty(i)) if *i == idx);
                let available = difficulty.unlocked && !has_active;

                let row_bg = if row_hovered && available {
                    SLOT_HOVER_BG
                } else if available {
                    Color::new(0.10, 0.10, 0.14, 1.0)
                } else {
                    Color::new(0.08, 0.08, 0.10, 1.0)
                };
                let row_border = if row_hovered && available {
                    SLOT_SELECTED_BORDER
                } else {
                    SLOT_BORDER
                };
                draw_rectangle(row_rect.x, row_rect.y, row_rect.w, row_rect.h, row_border);
                draw_rectangle(
                    row_rect.x + 1.0,
                    row_rect.y + 1.0,
                    row_rect.w - 2.0,
                    row_rect.h - 2.0,
                    row_bg,
                );

                // Accent bar
                if available {
                    draw_rectangle(
                        row_rect.x + 3.0,
                        row_rect.y + 3.0,
                        3.0 * s,
                        row_rect.h - 6.0,
                        accent,
                    );
                }

                // Top row: difficulty name + rewards
                let rewards_text =
                    format!("{} XP  {} gp", difficulty.reward_xp, difficulty.reward_gold);
                let rewards_w = self.measure_text_sharp(&rewards_text, 16.0).width;
                let name_max_w = (row_rect.w - 18.0 * s - rewards_w - 8.0 * s).max(40.0 * s);
                let name_display =
                    self.truncate_text_to_width(&difficulty.difficulty_name, name_max_w, 16.0);
                self.draw_text_sharp(
                    &name_display,
                    row_rect.x + 12.0 * s,
                    row_rect.y + 18.0 * s,
                    16.0,
                    if available { TEXT_NORMAL } else { TEXT_DIM },
                );
                self.draw_text_sharp(
                    &rewards_text,
                    row_rect.x + row_rect.w - rewards_w - 8.0 * s,
                    row_rect.y + 18.0 * s,
                    16.0,
                    TEXT_GOLD,
                );

                // Bottom row: status
                let req_text;
                let (final_status, final_color) = if has_active {
                    ("Active job in progress", TEXT_DIM)
                } else if difficulty.unlocked {
                    ("Take contract", Color::new(0.40, 0.80, 0.40, 1.0))
                } else {
                    req_text = format!("Requires {}", difficulty.level_required);
                    (req_text.as_str(), Color::new(0.85, 0.35, 0.35, 1.0))
                };
                let status_max_w = (row_rect.w - 18.0 * s).max(40.0 * s);
                let status_display = self.truncate_text_to_width(final_status, status_max_w, 16.0);
                self.draw_text_sharp(
                    &status_display,
                    row_rect.x + 12.0 * s,
                    row_rect.y + 36.0 * s,
                    16.0,
                    final_color,
                );
            }

            // Footer hint
            let hint = "Only one resource contract can be active at a time.";
            let hint_max_w = (detail_w - 24.0 * s).max(40.0 * s);
            let hint_display = self.truncate_text_to_width(hint, hint_max_w, 16.0);
            self.draw_text_sharp(
                &hint_display,
                right_x + 12.0 * s,
                right_y + detail_h - 14.0 * s,
                16.0,
                TEXT_DIM,
            );
        }

        // ===== ACTIVE CONTRACT PANEL =====
        draw_rectangle(active_x, active_y, active_w, active_h, SLOT_BORDER);
        draw_rectangle(
            active_x + 1.0,
            active_y + 1.0,
            active_w - 2.0,
            active_h - 2.0,
            Color::new(0.09, 0.09, 0.13, 1.0),
        );
        self.draw_text_sharp(
            "ACTIVE WORK",
            active_x + 12.0 * s,
            active_y + 24.0 * s,
            16.0,
            TEXT_TITLE,
        );

        let text_area_w = (active_w - 24.0 * s).max(40.0 * s);

        if let Some(contract) = board.active_contract.as_ref() {
            let accent = kind_accent(&contract.kind_id);

            // Accent bar under header
            draw_rectangle(
                active_x + 12.0 * s,
                active_y + 32.0 * s,
                active_w - 24.0 * s,
                3.0,
                accent,
            );

            let title_text = format!("{} {}", contract.difficulty_name, contract.kind_name);
            let title_display = self.truncate_text_to_width(&title_text, text_area_w, 16.0);
            self.draw_text_sharp(
                &title_display,
                active_x + 12.0 * s,
                active_y + 54.0 * s,
                16.0,
                TEXT_NORMAL,
            );

            for (i, line) in self
                .wrap_text(&contract.task_text, text_area_w, 16.0)
                .iter()
                .take(2)
                .enumerate()
            {
                self.draw_text_sharp(
                    line,
                    active_x + 12.0 * s,
                    active_y + 74.0 * s + i as f32 * 18.0 * s,
                    16.0,
                    TEXT_DIM,
                );
            }

            let turn_in = format!("Turn in: {}", contract.giver_name);
            let turn_in_display = self.truncate_text_to_width(&turn_in, text_area_w, 16.0);
            self.draw_text_sharp(
                &turn_in_display,
                active_x + 12.0 * s,
                active_y + 110.0 * s,
                16.0,
                TEXT_DIM,
            );

            // Progress bar
            let progress = if contract.amount_required > 0 {
                contract.amount_completed as f32 / contract.amount_required as f32
            } else {
                0.0
            }
            .clamp(0.0, 1.0);
            let bar_x = active_x + 12.0 * s;
            let bar_y = active_y + 124.0 * s;
            let bar_w = active_w - 24.0 * s;
            let bar_h = 16.0 * s;
            draw_rectangle(bar_x, bar_y, bar_w, bar_h, SLOT_BORDER);
            draw_rectangle(
                bar_x + 1.0,
                bar_y + 1.0,
                bar_w - 2.0,
                bar_h - 2.0,
                SLOT_BG_EMPTY,
            );
            draw_rectangle(
                bar_x + 2.0,
                bar_y + 2.0,
                (bar_w - 4.0) * progress,
                bar_h - 4.0,
                accent,
            );
            let progress_text = format!(
                "{}/{} {}",
                contract.amount_completed, contract.amount_required, contract.progress_label
            );
            let progress_display =
                self.truncate_text_to_width(&progress_text, bar_w - 8.0 * s, 16.0);
            self.draw_text_sharp(
                &progress_display,
                bar_x + 6.0 * s,
                bar_y + 12.0 * s,
                16.0,
                TEXT_NORMAL,
            );

            // Rewards
            self.draw_text_sharp(
                &format!("Reward: {} XP", contract.reward_xp),
                active_x + 12.0 * s,
                active_y + 158.0 * s,
                16.0,
                TEXT_GOLD,
            );
            self.draw_text_sharp(
                &format!("Reward: {} gp", contract.reward_gold),
                active_x + 12.0 * s,
                active_y + 176.0 * s,
                16.0,
                TEXT_GOLD,
            );
            if !contract.bonus_item_text.is_empty() {
                let bonus = format!("Bonus: {}", contract.bonus_item_text);
                let bonus_display = self.truncate_text_to_width(&bonus, text_area_w, 16.0);
                self.draw_text_sharp(
                    &bonus_display,
                    active_x + 12.0 * s,
                    active_y + 194.0 * s,
                    16.0,
                    Color::new(0.40, 0.80, 0.40, 1.0),
                );
            }

            // Action buttons
            let btn_y = active_y + active_h - 44.0 * s;
            let btn_h = 30.0 * s;
            let btn_w = ((active_w - 36.0 * s) / 2.0).max(60.0 * s);
            let claim_bounds = Rect::new(active_x + 12.0 * s, btn_y, btn_w, btn_h);
            let abandon_bounds = Rect::new(active_x + 20.0 * s + btn_w, btn_y, btn_w, btn_h);
            layout.add(UiElementId::AdventureBoardClaim, claim_bounds);
            layout.add(UiElementId::AdventureBoardAbandon, abandon_bounds);

            let claim_hovered = matches!(hovered, Some(UiElementId::AdventureBoardClaim));
            let abandon_hovered = matches!(hovered, Some(UiElementId::AdventureBoardAbandon));

            // Claim button
            let claim_border = if contract.can_claim {
                if claim_hovered {
                    SLOT_SELECTED_BORDER
                } else {
                    FRAME_MID
                }
            } else {
                SLOT_BORDER
            };
            let claim_bg = if contract.can_claim {
                if claim_hovered {
                    SLOT_HOVER_BG
                } else {
                    SLOT_BG_EMPTY
                }
            } else {
                Color::new(0.08, 0.08, 0.10, 1.0)
            };
            draw_rectangle(
                claim_bounds.x,
                claim_bounds.y,
                claim_bounds.w,
                claim_bounds.h,
                claim_border,
            );
            draw_rectangle(
                claim_bounds.x + 1.0,
                claim_bounds.y + 1.0,
                claim_bounds.w - 2.0,
                claim_bounds.h - 2.0,
                claim_bg,
            );
            let claim_label = if contract.can_claim {
                "Claim"
            } else {
                "Complete"
            };
            let claim_label_w = self.measure_text_sharp(claim_label, 16.0).width;
            self.draw_text_sharp(
                claim_label,
                claim_bounds.x + (claim_bounds.w - claim_label_w) * 0.5,
                claim_bounds.y + btn_h * 0.67,
                16.0,
                if contract.can_claim {
                    if claim_hovered {
                        TEXT_TITLE
                    } else {
                        TEXT_NORMAL
                    }
                } else {
                    TEXT_DIM
                },
            );

            // Abandon button
            draw_rectangle(
                abandon_bounds.x,
                abandon_bounds.y,
                abandon_bounds.w,
                abandon_bounds.h,
                if abandon_hovered {
                    SLOT_SELECTED_BORDER
                } else {
                    FRAME_MID
                },
            );
            draw_rectangle(
                abandon_bounds.x + 1.0,
                abandon_bounds.y + 1.0,
                abandon_bounds.w - 2.0,
                abandon_bounds.h - 2.0,
                if abandon_hovered {
                    SLOT_HOVER_BG
                } else {
                    SLOT_BG_EMPTY
                },
            );
            let abandon_label = "Abandon";
            let abandon_label_w = self.measure_text_sharp(abandon_label, 16.0).width;
            self.draw_text_sharp(
                abandon_label,
                abandon_bounds.x + (abandon_bounds.w - abandon_label_w) * 0.5,
                abandon_bounds.y + btn_h * 0.67,
                16.0,
                if abandon_hovered {
                    TEXT_TITLE
                } else {
                    TEXT_NORMAL
                },
            );
        } else {
            self.draw_text_sharp(
                "No active contract.",
                active_x + 12.0 * s,
                active_y + 56.0 * s,
                16.0,
                TEXT_NORMAL,
            );

            for (i, line) in self
                .wrap_text(
                    "Choose a lane on the left, then pick a job.",
                    text_area_w,
                    16.0,
                )
                .iter()
                .enumerate()
            {
                self.draw_text_sharp(
                    line,
                    active_x + 12.0 * s,
                    active_y + 78.0 * s + i as f32 * 18.0 * s,
                    16.0,
                    TEXT_DIM,
                );
            }
        }
    }

    fn render_orders_tab(
        &self,
        state: &GameState,
        board: &crate::game::AdventureBoardPanelState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        panel_x: f32,
        panel_y: f32,
        panel_w: f32,
        panel_h: f32,
        s: f32,
        compact: bool,
    ) {
        let order_stats = &board.crafting_order_stats;

        // Gold/amber accent for masterwork
        let masterwork_color = Color::new(0.95, 0.78, 0.25, 1.0);

        // ===== STAT CARDS =====
        let stats_y = panel_y + 72.0 * s;
        let stats_x = panel_x + 12.0 * s;
        let stats_gap = 8.0 * s;
        let stats_w = if compact {
            (panel_w - 24.0 * s - stats_gap * 2.0) / 3.0
        } else {
            120.0 * s
        };
        let stat_cards: [(&str, String); 3] = [
            ("Orders Done", order_stats.orders_completed.to_string()),
            ("Masterwork", order_stats.masterwork_completed.to_string()),
            ("Marks", order_stats.commission_marks.to_string()),
        ];
        for (idx, (label, value)) in stat_cards.iter().enumerate() {
            let card_x = stats_x + idx as f32 * (stats_w + stats_gap);
            draw_rectangle(card_x, stats_y, stats_w, 42.0 * s, SLOT_BORDER);
            draw_rectangle(
                card_x + 1.0,
                stats_y + 1.0,
                stats_w - 2.0,
                40.0 * s,
                SLOT_BG_EMPTY,
            );
            self.draw_text_sharp(label, card_x + 8.0 * s, stats_y + 15.0 * s, 16.0, TEXT_DIM);
            let value_color = if idx == 1 && order_stats.masterwork_completed > 0 {
                masterwork_color
            } else {
                TEXT_TITLE
            };
            self.draw_text_sharp(
                value,
                card_x + 8.0 * s,
                stats_y + 33.0 * s,
                16.0,
                value_color,
            );
        }

        // ===== LAYOUT =====
        let content_y = stats_y + 52.0 * s;
        let content_h = panel_y + panel_h - content_y - 12.0 * s;

        let left_w = if compact {
            panel_w - 24.0 * s
        } else {
            220.0 * s
        };
        let left_x = panel_x + 12.0 * s;
        let left_y = content_y;
        let left_h = if compact {
            let lane_count = board.crafting_orders.len().max(1) as f32;
            let row_h = 52.0 * s;
            let gap = 8.0 * s;
            (lane_count * (row_h + gap) + 16.0 * s).min(content_h * 0.45)
        } else {
            content_h
        };

        // Left panel: order list
        draw_rectangle(left_x, left_y, left_w, left_h, SLOT_BORDER);
        draw_rectangle(
            left_x + 1.0,
            left_y + 1.0,
            left_w - 2.0,
            left_h - 2.0,
            SLOT_BG_EMPTY,
        );

        let selected_idx = state
            .ui_state
            .adventure_board_selected_order
            .min(board.crafting_orders.len().saturating_sub(1));
        let selected_order = board.crafting_orders.get(selected_idx);

        let row_h = 52.0 * s;
        let row_gap = 8.0 * s;
        let mut row_y = left_y + 8.0 * s;
        for (idx, order) in board.crafting_orders.iter().enumerate() {
            let is_selected = idx == selected_idx;
            let hovered_card = matches!(hovered, Some(UiElementId::CraftingOrder(i)) if *i == idx);
            let accent = kind_accent(&order.skill);
            let is_masterwork = order.tier == "masterwork";

            let row_bounds = Rect::new(left_x + 6.0 * s, row_y, left_w - 12.0 * s, row_h);
            layout.add(UiElementId::CraftingOrder(idx), row_bounds);

            let row_bg = if is_selected {
                SLOT_HOVER_BG
            } else if hovered_card {
                Color::new(0.14, 0.14, 0.20, 1.0)
            } else {
                Color::new(0.10, 0.10, 0.14, 1.0)
            };
            let row_border = if is_selected {
                SLOT_SELECTED_BORDER
            } else {
                SLOT_BORDER
            };
            draw_rectangle(
                row_bounds.x,
                row_bounds.y,
                row_bounds.w,
                row_bounds.h,
                row_border,
            );
            draw_rectangle(
                row_bounds.x + 1.0,
                row_bounds.y + 1.0,
                row_bounds.w - 2.0,
                row_bounds.h - 2.0,
                row_bg,
            );

            // Accent bar on the left
            let bar_color = if is_masterwork {
                masterwork_color
            } else {
                accent
            };
            draw_rectangle(
                row_bounds.x + 3.0,
                row_bounds.y + 3.0,
                3.0 * s,
                row_bounds.h - 6.0,
                bar_color,
            );

            let text_x = row_bounds.x + 10.0 * s;

            // Tier badge
            let tier_text = order.tier.to_uppercase();
            let tier_w = self.measure_text_sharp(&tier_text, 16.0).width;
            let title_max_w = (row_bounds.w - 18.0 * s - tier_w - 8.0 * s).max(40.0 * s);

            // Title: skill name
            let title = order.skill.to_uppercase();
            let title_text = self.truncate_text_to_width(&title, title_max_w, 16.0);
            self.draw_text_sharp(
                &title_text,
                text_x,
                row_bounds.y + row_h * 0.36,
                16.0,
                if is_selected { TEXT_TITLE } else { TEXT_NORMAL },
            );
            // Tier on right
            let tier_color = if is_masterwork {
                masterwork_color
            } else {
                TEXT_DIM
            };
            self.draw_text_sharp(
                &tier_text,
                row_bounds.x + row_bounds.w - tier_w - 8.0 * s,
                row_bounds.y + row_h * 0.36,
                16.0,
                tier_color,
            );

            // Items summary
            let items_summary: Vec<String> = order
                .items
                .iter()
                .map(|item| format!("{}x {}", item.quantity, item.item_name))
                .collect();
            let summary = items_summary.join(", ");
            let desc_max_w = (row_bounds.w - 18.0 * s).max(40.0 * s);
            let desc_text = self.truncate_text_to_width(&summary, desc_max_w, 16.0);
            self.draw_text_sharp(
                &desc_text,
                text_x,
                row_bounds.y + row_h * 0.70,
                16.0,
                TEXT_DIM,
            );

            row_y += row_h + row_gap;
        }

        if board.crafting_orders.is_empty() {
            self.draw_text_sharp(
                "No orders left for today.",
                left_x + 12.0 * s,
                left_y + 30.0 * s,
                16.0,
                TEXT_DIM,
            );
            self.draw_text_sharp(
                "Check back tomorrow!",
                left_x + 12.0 * s,
                left_y + 50.0 * s,
                16.0,
                TEXT_DIM,
            );
        }

        // ===== RIGHT PANEL =====
        let right_x = if compact {
            panel_x + 12.0 * s
        } else {
            left_x + left_w + 12.0 * s
        };
        let right_y = if compact {
            left_y + left_h + 8.0 * s
        } else {
            content_y
        };
        let right_w = if compact {
            panel_w - 24.0 * s
        } else {
            panel_w - (right_x - panel_x) - 12.0 * s
        };
        let right_h = if compact {
            content_h - (right_y - content_y)
        } else {
            content_h
        };

        draw_rectangle(right_x, right_y, right_w, right_h, SLOT_BORDER);
        draw_rectangle(
            right_x + 1.0,
            right_y + 1.0,
            right_w - 2.0,
            right_h - 2.0,
            Color::new(0.09, 0.09, 0.13, 1.0),
        );

        let text_area_w = (right_w - 24.0 * s).max(40.0 * s);

        if let Some(active) = board.crafting_order_active.as_ref() {
            // ===== ACTIVE ORDER =====
            let is_masterwork = active.tier == "masterwork";
            let accent = if is_masterwork {
                masterwork_color
            } else {
                kind_accent(&active.skill)
            };

            self.draw_text_sharp(
                "ACTIVE ORDER",
                right_x + 12.0 * s,
                right_y + 24.0 * s,
                16.0,
                TEXT_TITLE,
            );

            // Accent bar under header
            draw_rectangle(
                right_x + 12.0 * s,
                right_y + 32.0 * s,
                right_w - 24.0 * s,
                3.0,
                accent,
            );

            let title_text = format!(
                "{} {} Order",
                active.tier.to_uppercase(),
                active.skill.to_uppercase()
            );
            let title_display = self.truncate_text_to_width(&title_text, text_area_w, 16.0);
            self.draw_text_sharp(
                &title_display,
                right_x + 12.0 * s,
                right_y + 54.0 * s,
                16.0,
                TEXT_NORMAL,
            );

            // Items list
            let mut item_y = right_y + 74.0 * s;
            for item in &active.items {
                let item_text = format!("{}x {}", item.quantity, item.item_name);
                let item_display = self.truncate_text_to_width(&item_text, text_area_w, 16.0);
                self.draw_text_sharp(&item_display, right_x + 20.0 * s, item_y, 16.0, TEXT_NORMAL);
                item_y += 18.0 * s;
            }

            // Rewards
            item_y += 6.0 * s;
            if active.reward_gold > 0 {
                self.draw_text_sharp(
                    &format!("Reward: {} gp", active.reward_gold),
                    right_x + 12.0 * s,
                    item_y,
                    16.0,
                    TEXT_GOLD,
                );
                item_y += 18.0 * s;
            }
            if active.reward_marks > 0 {
                self.draw_text_sharp(
                    &format!("Reward: {} marks", active.reward_marks),
                    right_x + 12.0 * s,
                    item_y,
                    16.0,
                    TEXT_GOLD,
                );
            }

            // Action buttons
            let btn_y = right_y + right_h - 44.0 * s;
            let btn_h = 30.0 * s;
            let btn_w = ((right_w - 36.0 * s) / 2.0).max(60.0 * s);
            let claim_bounds = Rect::new(right_x + 12.0 * s, btn_y, btn_w, btn_h);
            let abandon_bounds = Rect::new(right_x + 20.0 * s + btn_w, btn_y, btn_w, btn_h);
            layout.add(UiElementId::CraftingOrderClaim, claim_bounds);
            layout.add(UiElementId::CraftingOrderAbandon, abandon_bounds);

            let claim_hovered = matches!(hovered, Some(UiElementId::CraftingOrderClaim));
            let abandon_hovered = matches!(hovered, Some(UiElementId::CraftingOrderAbandon));

            // Claim button
            let can_claim = active.can_claim;
            let claim_border = if can_claim {
                if claim_hovered {
                    SLOT_SELECTED_BORDER
                } else {
                    FRAME_MID
                }
            } else {
                SLOT_BORDER
            };
            let claim_bg = if can_claim {
                if claim_hovered {
                    SLOT_HOVER_BG
                } else {
                    SLOT_BG_EMPTY
                }
            } else {
                Color::new(0.08, 0.08, 0.10, 1.0)
            };
            draw_rectangle(
                claim_bounds.x,
                claim_bounds.y,
                claim_bounds.w,
                claim_bounds.h,
                claim_border,
            );
            draw_rectangle(
                claim_bounds.x + 1.0,
                claim_bounds.y + 1.0,
                claim_bounds.w - 2.0,
                claim_bounds.h - 2.0,
                claim_bg,
            );
            let claim_label = if can_claim { "Claim" } else { "Gather Items" };
            let claim_label_w = self.measure_text_sharp(claim_label, 16.0).width;
            self.draw_text_sharp(
                claim_label,
                claim_bounds.x + (claim_bounds.w - claim_label_w) * 0.5,
                claim_bounds.y + btn_h * 0.67,
                16.0,
                if can_claim {
                    if claim_hovered {
                        TEXT_TITLE
                    } else {
                        TEXT_NORMAL
                    }
                } else {
                    TEXT_DIM
                },
            );

            // Abandon button
            draw_rectangle(
                abandon_bounds.x,
                abandon_bounds.y,
                abandon_bounds.w,
                abandon_bounds.h,
                if abandon_hovered {
                    SLOT_SELECTED_BORDER
                } else {
                    FRAME_MID
                },
            );
            draw_rectangle(
                abandon_bounds.x + 1.0,
                abandon_bounds.y + 1.0,
                abandon_bounds.w - 2.0,
                abandon_bounds.h - 2.0,
                if abandon_hovered {
                    SLOT_HOVER_BG
                } else {
                    SLOT_BG_EMPTY
                },
            );
            let abandon_label = "Abandon";
            let abandon_label_w = self.measure_text_sharp(abandon_label, 16.0).width;
            self.draw_text_sharp(
                abandon_label,
                abandon_bounds.x + (abandon_bounds.w - abandon_label_w) * 0.5,
                abandon_bounds.y + btn_h * 0.67,
                16.0,
                if abandon_hovered {
                    TEXT_TITLE
                } else {
                    TEXT_NORMAL
                },
            );
        } else if let Some(order) = selected_order {
            // ===== SELECTED ORDER DETAIL =====
            let is_masterwork = order.tier == "masterwork";
            let accent = if is_masterwork {
                masterwork_color
            } else {
                kind_accent(&order.skill)
            };

            let header_text = format!(
                "{} {} ORDER",
                order.tier.to_uppercase(),
                order.skill.to_uppercase()
            );
            let header_max_w = (right_w - 24.0 * s).max(40.0 * s);
            let header_display = self.truncate_text_to_width(&header_text, header_max_w, 16.0);
            self.draw_text_sharp(
                &header_display,
                right_x + 12.0 * s,
                right_y + 24.0 * s,
                16.0,
                TEXT_TITLE,
            );

            // Accent bar under header
            draw_rectangle(
                right_x + 12.0 * s,
                right_y + 32.0 * s,
                right_w - 24.0 * s,
                3.0,
                accent,
            );

            if order.min_level > 0 {
                self.draw_text_sharp(
                    &format!("Requires level {}", order.min_level),
                    right_x + 12.0 * s,
                    right_y + 50.0 * s,
                    16.0,
                    TEXT_DIM,
                );
            }

            // Items list
            self.draw_text_sharp(
                "Required Items:",
                right_x + 12.0 * s,
                right_y + 70.0 * s,
                16.0,
                TEXT_NORMAL,
            );
            let mut item_y = right_y + 88.0 * s;
            for item in &order.items {
                let item_text = format!("{}x {}", item.quantity, item.item_name);
                let item_display = self.truncate_text_to_width(&item_text, text_area_w, 16.0);
                self.draw_text_sharp(&item_display, right_x + 20.0 * s, item_y, 16.0, TEXT_NORMAL);
                item_y += 18.0 * s;
            }

            // Rewards section
            item_y += 8.0 * s;
            draw_line(
                right_x + 12.0 * s,
                item_y - 4.0 * s,
                right_x + right_w - 12.0 * s,
                item_y - 4.0 * s,
                1.0,
                HEADER_BORDER,
            );
            item_y += 8.0 * s;
            self.draw_text_sharp(
                "Rewards:",
                right_x + 12.0 * s,
                item_y + 4.0 * s,
                16.0,
                TEXT_NORMAL,
            );
            item_y += 22.0 * s;

            if order.reward_gold > 0 {
                self.draw_text_sharp(
                    &format!("{} gp", order.reward_gold),
                    right_x + 20.0 * s,
                    item_y,
                    16.0,
                    TEXT_GOLD,
                );
                item_y += 18.0 * s;
            }
            for (skill, amount) in &order.reward_xp {
                self.draw_text_sharp(
                    &format!("{} {} XP", amount, skill),
                    right_x + 20.0 * s,
                    item_y,
                    16.0,
                    TEXT_GOLD,
                );
                item_y += 18.0 * s;
            }
            if order.reward_marks > 0 {
                self.draw_text_sharp(
                    &format!("{} commission marks", order.reward_marks),
                    right_x + 20.0 * s,
                    item_y,
                    16.0,
                    TEXT_GOLD,
                );
            }

            // Accept button
            let btn_y = right_y + right_h - 44.0 * s;
            let btn_h = 30.0 * s;
            let btn_w = (right_w - 24.0 * s).max(60.0 * s);
            let accept_bounds = Rect::new(right_x + 12.0 * s, btn_y, btn_w, btn_h);
            layout.add(UiElementId::CraftingOrderAccept, accept_bounds);

            let accept_hovered = matches!(hovered, Some(UiElementId::CraftingOrderAccept));
            let has_active = board.crafting_order_active.is_some();
            let available = !has_active;

            let accept_border = if available {
                if accept_hovered {
                    SLOT_SELECTED_BORDER
                } else {
                    FRAME_MID
                }
            } else {
                SLOT_BORDER
            };
            let accept_bg = if available {
                if accept_hovered {
                    SLOT_HOVER_BG
                } else {
                    SLOT_BG_EMPTY
                }
            } else {
                Color::new(0.08, 0.08, 0.10, 1.0)
            };
            draw_rectangle(
                accept_bounds.x,
                accept_bounds.y,
                accept_bounds.w,
                accept_bounds.h,
                accept_border,
            );
            draw_rectangle(
                accept_bounds.x + 1.0,
                accept_bounds.y + 1.0,
                accept_bounds.w - 2.0,
                accept_bounds.h - 2.0,
                accept_bg,
            );
            let accept_label = if has_active {
                "Active order in progress"
            } else {
                "Accept Order"
            };
            let accept_label_w = self.measure_text_sharp(accept_label, 16.0).width;
            self.draw_text_sharp(
                accept_label,
                accept_bounds.x + (accept_bounds.w - accept_label_w) * 0.5,
                accept_bounds.y + btn_h * 0.67,
                16.0,
                if available {
                    if accept_hovered {
                        TEXT_TITLE
                    } else {
                        Color::new(0.40, 0.80, 0.40, 1.0)
                    }
                } else {
                    TEXT_DIM
                },
            );
        } else {
            self.draw_text_sharp(
                "No orders available.",
                right_x + 12.0 * s,
                right_y + 24.0 * s,
                16.0,
                TEXT_NORMAL,
            );
            for (i, line) in self
                .wrap_text(
                    "Crafting orders let you fill commissions for gold and marks.",
                    text_area_w,
                    16.0,
                )
                .iter()
                .enumerate()
            {
                self.draw_text_sharp(
                    line,
                    right_x + 12.0 * s,
                    right_y + 46.0 * s + i as f32 * 18.0 * s,
                    16.0,
                    TEXT_DIM,
                );
            }
        }
    }
}
