use super::*;

impl Renderer {
    /// Task 14: Render crafting progress overlay on the detail panel
    pub(super) fn render_crafting_progress(
        &self,
        state: &GameState,
        hovered: &Option<UiElementId>,
        layout: &mut UiLayout,
        detail_x: f32,
        detail_y: f32,
        detail_width: f32,
        detail_height: f32,
    ) {
        let s = self.font_scale.get();
        let progress = state.ui_state.crafting_progress;

        // "CRAFTING..." text centered, with pulsing ellipsis
        let time = get_time() as f32;
        let dots = match ((time * 2.0) as i32) % 4 {
            0 => "CRAFTING",
            1 => "CRAFTING.",
            2 => "CRAFTING..",
            _ => "CRAFTING...",
        };
        let crafting_dims = self.measure_text_sharp(dots, 16.0);
        let text_x = detail_x + (detail_width - crafting_dims.width) / 2.0;
        self.draw_text_sharp(dots, text_x, detail_y + 40.0 * s, 16.0, TEXT_TITLE);

        // Show the result item name and sprite if we can find the recipe
        if let Some(ref recipe_id) = state.ui_state.crafting_recipe_id {
            if let Some(recipe) = state.recipe_definitions.iter().find(|r| &r.id == recipe_id) {
                // Draw a centered result item sprite (48x48) below the CRAFTING text
                let progress_icon_size = 48.0 * s;
                if let Some(result) = recipe.results.first() {
                    let icon_x = detail_x + (detail_width - progress_icon_size) / 2.0;
                    let icon_y = detail_y + 50.0 * s;
                    self.draw_item_icon(
                        &result.item_id,
                        icon_x,
                        icon_y,
                        progress_icon_size,
                        progress_icon_size,
                        state,
                        true,
                    );
                }

                // Recipe name below the sprite
                let name_dims = self.measure_text_sharp(&recipe.display_name, 16.0);
                let name_x = detail_x + (detail_width - name_dims.width) / 2.0;
                // Pulsing effect on the item name
                let pulse = (time * 3.0).sin() * 0.15 + 0.85;
                let pulse_color = Color::new(
                    CATEGORY_EQUIPMENT.r * pulse,
                    CATEGORY_EQUIPMENT.g * pulse,
                    CATEGORY_EQUIPMENT.b * pulse,
                    1.0,
                );
                self.draw_text_sharp(
                    &recipe.display_name,
                    name_x,
                    detail_y + 50.0 * s + progress_icon_size + 16.0 * s,
                    16.0,
                    pulse_color,
                );

                // Show what it creates
                if let Some(result) = recipe.results.first() {
                    let result_name = state.item_registry.get_display_name(&result.item_id);
                    let result_text = format!("Creating: {} x{}", result_name, result.count);
                    let result_dims = self.measure_text_sharp(&result_text, 16.0);
                    let result_x = detail_x + (detail_width - result_dims.width) / 2.0;
                    self.draw_text_sharp(
                        &result_text,
                        result_x,
                        detail_y + 50.0 * s + progress_icon_size + 36.0 * s,
                        16.0,
                        TEXT_NORMAL,
                    );
                }
            }
        }

        // Progress bar
        let bar_width = detail_width - 40.0 * s;
        let bar_height = 20.0 * s;
        let bar_x = detail_x + 20.0 * s;
        let bar_y = detail_y + detail_height / 2.0 - bar_height / 2.0 + 10.0 * s;

        // Bar background
        draw_rectangle(bar_x, bar_y, bar_width, bar_height, SLOT_BORDER);
        draw_rectangle(
            bar_x + 1.0,
            bar_y + 1.0,
            bar_width - 2.0,
            bar_height - 2.0,
            SLOT_BG_EMPTY,
        );
        draw_line(
            bar_x + 2.0,
            bar_y + 2.0,
            bar_x + bar_width - 2.0,
            bar_y + 2.0,
            1.0,
            SLOT_INNER_SHADOW,
        );

        // Bar fill
        let fill_width = (bar_width - 4.0) * progress;
        if fill_width > 0.0 {
            // Gradient-like fill using two rectangles
            let fill_x = bar_x + 2.0;
            let fill_y = bar_y + 2.0;
            let fill_h = bar_height - 4.0;

            draw_rectangle(
                fill_x,
                fill_y,
                fill_width,
                fill_h,
                Color::new(0.15, 0.4, 0.15, 1.0),
            );
            // Brighter top half
            draw_rectangle(
                fill_x,
                fill_y,
                fill_width,
                fill_h / 2.0,
                Color::new(0.2, 0.55, 0.2, 1.0),
            );
            // Highlight line at top
            draw_line(
                fill_x,
                fill_y,
                fill_x + fill_width,
                fill_y,
                1.0,
                Color::new(0.35, 0.75, 0.35, 1.0),
            );
        }

        // Percentage text below bar
        let pct_text = format!("{}%", (progress * 100.0) as i32);
        let pct_dims = self.measure_text_sharp(&pct_text, 16.0);
        let pct_x = detail_x + (detail_width - pct_dims.width) / 2.0;
        self.draw_text_sharp(
            &pct_text,
            pct_x,
            bar_y + bar_height + 20.0 * s,
            16.0,
            TEXT_NORMAL,
        );

        // CANCEL button
        let cancel_btn_width = 120.0 * s;
        let cancel_btn_height = 28.0 * s;
        let cancel_btn_x = detail_x + (detail_width - cancel_btn_width) / 2.0;
        let cancel_btn_y = detail_y + detail_height - 42.0 * s;

        let cancel_bounds = Rect::new(
            cancel_btn_x,
            cancel_btn_y,
            cancel_btn_width,
            cancel_btn_height,
        );
        layout.add(UiElementId::CraftingCancelButton, cancel_bounds);

        let is_cancel_hovered = matches!(hovered, Some(UiElementId::CraftingCancelButton));
        let (cancel_bg, cancel_border) = if is_cancel_hovered {
            (
                Color::new(0.45, 0.15, 0.15, 1.0),
                Color::new(0.6, 0.2, 0.2, 1.0),
            )
        } else {
            (
                Color::new(0.35, 0.12, 0.12, 1.0),
                Color::new(0.5, 0.18, 0.18, 1.0),
            )
        };

        draw_rectangle(
            cancel_btn_x,
            cancel_btn_y,
            cancel_btn_width,
            cancel_btn_height,
            cancel_border,
        );
        draw_rectangle(
            cancel_btn_x + 1.0,
            cancel_btn_y + 1.0,
            cancel_btn_width - 2.0,
            cancel_btn_height - 2.0,
            cancel_bg,
        );

        let cancel_text = "[ CANCEL ]";
        let cancel_text_w = self.measure_text_sharp(cancel_text, 16.0).width;
        let cancel_text_color = if is_cancel_hovered {
            WHITE
        } else {
            Color::new(0.85, 0.6, 0.6, 1.0)
        };
        self.draw_text_sharp(
            cancel_text,
            cancel_btn_x + (cancel_btn_width - cancel_text_w) / 2.0,
            cancel_btn_y + cancel_btn_height * 0.68,
            16.0,
            cancel_text_color,
        );
    }

    /// Task 20: Render crafting completion animation overlay
    pub(super) fn render_crafting_complete(
        &self,
        state: &GameState,
        recipe_id: &str,
        timer: f32,
        detail_x: f32,
        detail_y: f32,
        detail_width: f32,
        detail_height: f32,
    ) {
        let s_ui = self.font_scale.get();
        // timer goes from 0.0 to 1.0 over ~1 second
        let alpha = 1.0 - timer; // fade out

        // "Crafted!" text with scale-up pop effect
        let scale = if timer < 0.2 {
            // Pop in: scale from 0.5 to 1.2
            0.5 + (timer / 0.2) * 0.7
        } else if timer < 0.35 {
            // Settle: scale from 1.2 to 1.0
            1.2 - ((timer - 0.2) / 0.15) * 0.2
        } else {
            1.0
        };

        let crafted_text = "Crafted!";
        let font_size = 16.0 * scale;
        let crafted_dims = self.measure_text_sharp(crafted_text, font_size);
        let crafted_x = detail_x + (detail_width - crafted_dims.width) / 2.0;
        let crafted_y = detail_y + detail_height / 2.0 - 20.0 * s_ui;

        let text_color = Color::new(0.392, 0.784, 0.392, alpha);
        self.draw_text_sharp(crafted_text, crafted_x, crafted_y, font_size, text_color);

        // Show the item name below
        if let Some(recipe) = state.recipe_definitions.iter().find(|r| r.id == recipe_id) {
            let name_dims = self.measure_text_sharp(&recipe.display_name, 16.0);
            let name_x = detail_x + (detail_width - name_dims.width) / 2.0;
            let name_color = Color::new(
                CATEGORY_EQUIPMENT.r,
                CATEGORY_EQUIPMENT.g,
                CATEGORY_EQUIPMENT.b,
                alpha,
            );
            self.draw_text_sharp(
                &recipe.display_name,
                name_x,
                crafted_y + 25.0 * s_ui,
                16.0,
                name_color,
            );
        }
    }
}
