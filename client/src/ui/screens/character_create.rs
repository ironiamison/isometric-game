use super::*;

use crate::render::ui::common::{
    draw_corner_accents, draw_panel_frame, draw_screen_button_alpha, ButtonVariant, DANGER_TEXT,
    FRAME_ACCENT, FRAME_OUTER, TEXT_DIM, TEXT_NORMAL, TEXT_TITLE,
};

pub struct CharacterCreateScreen {
    session: AuthSession,
    form: CreateForm,
    auth_client: AuthClient,
    font: BitmapFont,
    player_sprites: SpritesheetStore,
    hair_sprites: SpritesheetStore,
    starfield: StarfieldBackground,
    #[cfg(target_arch = "wasm32")]
    loading: bool,
}

const HAIR_STYLES: usize = 6; // 0-5
const HAIR_COLORS: usize = 10; // 0-9 (20 frames / 2 front-back pairs)

const FIELD_BOX_H: f32 = 34.0;
const LABEL_DY: f32 = 14.0; // label baseline above its row's box
const BOX_DY: f32 = 20.0; // box top below the row's label line
const HAIR_MID_GAP: f32 = 10.0; // gap between Style and Color half-boxes
const ACTION_GAP: f32 = 8.0; // gap between Create and Cancel
const ARROW_ZONE_FULL: f32 = 50.0; // click width of < / > on full-width steppers
const ARROW_ZONE_HALF: f32 = 35.0; // click width of < / > on half-width hair steppers

/// Precomputed geometry for the create-character form, centered as a group
/// (mirrors char-select's `CharSelectLayout`) so `update` hit-testing and
/// `render` drawing can never drift apart.
///
/// `compute` centers the form for a standalone screen; `from_panel` derives the
/// same inner geometry from an arbitrary panel rect, which lets the character
/// box morph (resize) the form in place during the select→create transition.
pub(crate) struct CreateLayout {
    pub header_y: f32,    // title baseline, above panel
    pub panel: Rect,      // bronze framed panel
    portrait: Rect,       // recessed portrait inset (inside panel, left)
    form_x: f32,          // form column left
    form_w: f32,          // form column width
    form_top: f32,        // y of first form row
    row_h: f32,           // vertical spacing per field row
    pub action_bar: Rect, // Create/Cancel row below panel
}

impl CreateLayout {
    /// Inner padding (FRAME_THICKNESS + 10), gap below panel, and action-bar
    /// height — kept identical to `CharSelectLayout` so the two layouts line up
    /// when one morphs into the other.
    const PAD: f32 = 14.0;
    const ACTION_GAP_Y: f32 = 12.0;
    const ACTION_H: f32 = 44.0;
    const ROWS: f32 = 4.0; // Name, Gender, Skin, Hair(Style+Color)

    pub(crate) fn compute(sw: f32, sh: f32) -> Self {
        let panel_w = 520.0_f32.min(sw - 24.0);
        let panel_x = (sw - panel_w) / 2.0;
        let header_h = 28.0;
        let hint_h = 28.0;

        let inner_h = Self::ROWS * 58.0; // natural row height 58 -> 232
        let panel_h = inner_h + Self::PAD * 2.0;

        // center the whole group vertically
        let block_h = header_h + panel_h + Self::ACTION_GAP_Y + Self::ACTION_H + hint_h;
        let block_top = ((sh - block_h) / 2.0).max(12.0);
        let panel_top = block_top + header_h;
        let panel = Rect::new(panel_x, panel_top, panel_w, panel_h);
        Self::from_panel(panel)
    }

    /// Derive the inner form geometry from a given panel rect. Header sits 8px
    /// above the panel and the action bar 12px below — matching `compute` and
    /// `CharSelectLayout` so transitions stay aligned.
    pub(crate) fn from_panel(panel: Rect) -> Self {
        let pad = Self::PAD;
        let inner_w = panel.w - pad * 2.0;
        let inner_h = panel.h - pad * 2.0;
        let portrait_w = 150.0;
        let form_gap = 24.0;
        let form_w = inner_w - portrait_w - form_gap;
        let row_h = inner_h / Self::ROWS;

        let header_y = panel.y - 8.0;
        let portrait = Rect::new(panel.x + pad, panel.y + pad, portrait_w, inner_h);
        let form_x = portrait.x + portrait_w + form_gap;
        let form_top = panel.y + pad;
        let action_bar = Rect::new(
            panel.x,
            panel.y + panel.h + Self::ACTION_GAP_Y,
            panel.w,
            Self::ACTION_H,
        );

        Self {
            header_y,
            panel,
            portrait,
            form_x,
            form_w,
            form_top,
            row_h,
            action_bar,
        }
    }

    /// Label baseline for form row `row`.
    fn label_y(&self, row: usize) -> f32 {
        self.form_top + row as f32 * self.row_h + LABEL_DY
    }

    /// Full-width field box for form row `row` (Name/Gender/Skin).
    fn row_box(&self, row: usize) -> Rect {
        Rect::new(
            self.form_x,
            self.form_top + row as f32 * self.row_h + BOX_DY,
            self.form_w,
            FIELD_BOX_H,
        )
    }

    /// Hair row (row 3) half-width boxes: (style_left, color_right).
    fn hair_boxes(&self) -> (Rect, Rect) {
        let y = self.form_top + 3.0 * self.row_h + BOX_DY;
        let half_w = (self.form_w - HAIR_MID_GAP) / 2.0;
        let style = Rect::new(self.form_x, y, half_w, FIELD_BOX_H);
        let color = Rect::new(self.form_x + half_w + HAIR_MID_GAP, y, half_w, FIELD_BOX_H);
        (style, color)
    }

    /// Create / Cancel button rects in the action bar.
    fn button_rects(&self) -> (Rect, Rect) {
        let bw = (self.action_bar.w - ACTION_GAP) / 2.0;
        let create = Rect::new(self.action_bar.x, self.action_bar.y, bw, self.action_bar.h);
        let cancel = Rect::new(
            self.action_bar.x + bw + ACTION_GAP,
            self.action_bar.y,
            bw,
            self.action_bar.h,
        );
        (create, cancel)
    }
}

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum CreateField {
    Name,
    Gender,
    Skin,
    HairStyle,
    HairColor,
}

/// What a `CreateForm` interaction resolved to this frame. The host (standalone
/// screen or the embedded select-mode) turns `Submit` into an auth request and
/// `Cancel` into a return to the roster.
pub(crate) enum CreateAction {
    None,
    Cancel,
    Submit {
        name: String,
        gender: &'static str,
        skin: &'static str,
        hair_style: Option<i32>,
        hair_color: Option<i32>,
    },
}

/// The character-creation form (state + input + drawing), independent of where
/// it is hosted. Owned by `CharacterCreateScreen` (standalone) and embedded in
/// `CharacterSelectScreen` for the in-place create mode.
pub(crate) struct CreateForm {
    pub name: String,
    gender_index: usize,
    skin_index: usize,
    hair_style_index: Option<usize>, // None = bald, Some(0-5) = style
    hair_color_index: usize,         // 0-9
    active_field: CreateField,
    error_message: Option<String>,
    touch_detected: bool,
}

impl CreateForm {
    pub(crate) fn new() -> Self {
        Self {
            name: String::new(),
            gender_index: 0,
            skin_index: 0,
            hair_style_index: None,
            hair_color_index: 0,
            active_field: CreateField::Name,
            error_message: None,
            touch_detected: false,
        }
    }

    /// Reset to a fresh form (called when (re)entering the create flow).
    pub(crate) fn reset(&mut self) {
        let touch = self.touch_detected;
        *self = Self::new();
        self.touch_detected = touch;
    }

    /// Push a server-side error (e.g. name taken) for display.
    pub(crate) fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
    }

    fn handle_name_input(&mut self) {
        while let Some(c) = get_char_pressed() {
            if (c.is_alphanumeric() || c == '_' || c == '-') && self.name.len() < 16 {
                self.name.push(c);
                self.error_message = None;
            }
        }
        if is_key_pressed(KeyCode::Backspace) {
            self.name.pop();
            self.error_message = None;
        }
    }

    /// Validate the current form and build a `Submit` action, or set an error
    /// and return `None`.
    fn try_submit(&mut self) -> Option<CreateAction> {
        let name = self.name.trim();
        if name.len() < 2 {
            self.error_message = Some("Name must be at least 2 characters".to_string());
            return None;
        }
        if name.len() > 16 {
            self.error_message = Some("Name must be at most 16 characters".to_string());
            return None;
        }
        let gender = GENDERS[self.gender_index];
        let skin = SKINS[self.skin_index];
        let hair_style = self.hair_style_index.map(|i| i as i32);
        let hair_color = if self.hair_style_index.is_some() {
            Some(self.hair_color_index as i32)
        } else {
            None
        };
        Some(CreateAction::Submit {
            name: name.to_string(),
            gender,
            skin,
            hair_style,
            hair_color,
        })
    }

    /// Process a frame of input against the given layout. Returns the resolved
    /// action (mostly `None`).
    pub(crate) fn update(&mut self, l: &CreateLayout, audio: &AudioManager) -> CreateAction {
        let (input_pos, clicked, _is_touch) = get_input_state();
        let mx = input_pos.x;
        let my = input_pos.y;

        // Hide keyboard hints once a touch device is detected.
        if !touches().is_empty() {
            self.touch_detected = true;
        }

        // Handle name input when name field is active
        if self.active_field == CreateField::Name {
            self.handle_name_input();
        }

        // Mouse: Click on fields to focus / steppers to cycle
        if clicked {
            // Name field box
            let name_box = l.row_box(0);
            if point_in_rect(mx, my, name_box.x, name_box.y, name_box.w, name_box.h) {
                self.active_field = CreateField::Name;
                show_keyboard(true);
            }

            // Gender field box
            let gb = l.row_box(1);
            if point_in_rect(mx, my, gb.x, gb.y, gb.w, gb.h) {
                self.active_field = CreateField::Gender;
                show_keyboard(false);
                if point_in_rect(mx, my, gb.x, gb.y, ARROW_ZONE_FULL, gb.h) {
                    self.gender_index = if self.gender_index == 0 {
                        GENDERS.len() - 1
                    } else {
                        self.gender_index - 1
                    };
                }
                if point_in_rect(
                    mx,
                    my,
                    gb.x + gb.w - ARROW_ZONE_FULL,
                    gb.y,
                    ARROW_ZONE_FULL,
                    gb.h,
                ) {
                    self.gender_index = (self.gender_index + 1) % GENDERS.len();
                }
            }

            // Skin field box
            let sb = l.row_box(2);
            if point_in_rect(mx, my, sb.x, sb.y, sb.w, sb.h) {
                self.active_field = CreateField::Skin;
                show_keyboard(false);
                if point_in_rect(mx, my, sb.x, sb.y, ARROW_ZONE_FULL, sb.h) {
                    self.skin_index = if self.skin_index == 0 {
                        SKINS.len() - 1
                    } else {
                        self.skin_index - 1
                    };
                }
                if point_in_rect(
                    mx,
                    my,
                    sb.x + sb.w - ARROW_ZONE_FULL,
                    sb.y,
                    ARROW_ZONE_FULL,
                    sb.h,
                ) {
                    self.skin_index = (self.skin_index + 1) % SKINS.len();
                }
            }

            let (style_box, color_box) = l.hair_boxes();

            // Hair style field box (left half of hair row)
            if point_in_rect(mx, my, style_box.x, style_box.y, style_box.w, style_box.h) {
                self.active_field = CreateField::HairStyle;
                show_keyboard(false);
                if point_in_rect(
                    mx,
                    my,
                    style_box.x,
                    style_box.y,
                    ARROW_ZONE_HALF,
                    style_box.h,
                ) {
                    self.hair_style_index = match self.hair_style_index {
                        None => Some(HAIR_STYLES - 1),
                        Some(0) => None,
                        Some(i) => Some(i - 1),
                    };
                }
                if point_in_rect(
                    mx,
                    my,
                    style_box.x + style_box.w - ARROW_ZONE_HALF,
                    style_box.y,
                    ARROW_ZONE_HALF,
                    style_box.h,
                ) {
                    self.hair_style_index = match self.hair_style_index {
                        None => Some(0),
                        Some(i) if i >= HAIR_STYLES - 1 => None,
                        Some(i) => Some(i + 1),
                    };
                }
            }

            // Hair color field box (right half of hair row, only if style selected)
            if self.hair_style_index.is_some()
                && point_in_rect(mx, my, color_box.x, color_box.y, color_box.w, color_box.h)
            {
                self.active_field = CreateField::HairColor;
                show_keyboard(false);
                if point_in_rect(
                    mx,
                    my,
                    color_box.x,
                    color_box.y,
                    ARROW_ZONE_HALF,
                    color_box.h,
                ) {
                    self.hair_color_index = if self.hair_color_index == 0 {
                        HAIR_COLORS - 1
                    } else {
                        self.hair_color_index - 1
                    };
                }
                if point_in_rect(
                    mx,
                    my,
                    color_box.x + color_box.w - ARROW_ZONE_HALF,
                    color_box.y,
                    ARROW_ZONE_HALF,
                    color_box.h,
                ) {
                    self.hair_color_index = (self.hair_color_index + 1) % HAIR_COLORS;
                }
            }

            // Action bar: Create + Cancel
            let (create_rect, cancel_rect) = l.button_rects();
            if point_in_rect(
                mx,
                my,
                create_rect.x,
                create_rect.y,
                create_rect.w,
                create_rect.h,
            ) {
                show_keyboard(false);
                if let Some(action) = self.try_submit() {
                    return action;
                }
                return CreateAction::None;
            }
            if point_in_rect(
                mx,
                my,
                cancel_rect.x,
                cancel_rect.y,
                cancel_rect.w,
                cancel_rect.h,
            ) {
                show_keyboard(false);
                return CreateAction::Cancel;
            }
        }

        // Keyboard: Navigate between fields with Tab or Up/Down
        if is_key_pressed(KeyCode::Tab) || is_key_pressed(KeyCode::Down) {
            audio.play_sfx("enter");
            self.active_field = match self.active_field {
                CreateField::Name => CreateField::Gender,
                CreateField::Gender => CreateField::Skin,
                CreateField::Skin => CreateField::HairStyle,
                CreateField::HairStyle => {
                    if self.hair_style_index.is_some() {
                        CreateField::HairColor
                    } else {
                        CreateField::Name
                    }
                }
                CreateField::HairColor => CreateField::Name,
            };
        }
        if is_key_pressed(KeyCode::Up) {
            audio.play_sfx("enter");
            self.active_field = match self.active_field {
                CreateField::Name => {
                    if self.hair_style_index.is_some() {
                        CreateField::HairColor
                    } else {
                        CreateField::HairStyle
                    }
                }
                CreateField::Gender => CreateField::Name,
                CreateField::Skin => CreateField::Gender,
                CreateField::HairStyle => CreateField::Skin,
                CreateField::HairColor => CreateField::HairStyle,
            };
        }

        // Keyboard: Left/Right or A/D to cycle options
        if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
            match self.active_field {
                CreateField::Gender => {
                    self.gender_index = if self.gender_index == 0 {
                        GENDERS.len() - 1
                    } else {
                        self.gender_index - 1
                    };
                }
                CreateField::Skin => {
                    self.skin_index = if self.skin_index == 0 {
                        SKINS.len() - 1
                    } else {
                        self.skin_index - 1
                    };
                }
                CreateField::HairStyle => {
                    self.hair_style_index = match self.hair_style_index {
                        None => Some(HAIR_STYLES - 1),
                        Some(0) => None,
                        Some(i) => Some(i - 1),
                    };
                }
                CreateField::HairColor => {
                    self.hair_color_index = if self.hair_color_index == 0 {
                        HAIR_COLORS - 1
                    } else {
                        self.hair_color_index - 1
                    };
                }
                _ => {}
            }
        }
        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
            match self.active_field {
                CreateField::Gender => {
                    self.gender_index = (self.gender_index + 1) % GENDERS.len();
                }
                CreateField::Skin => {
                    self.skin_index = (self.skin_index + 1) % SKINS.len();
                }
                CreateField::HairStyle => {
                    self.hair_style_index = match self.hair_style_index {
                        None => Some(0),
                        Some(i) if i >= HAIR_STYLES - 1 => None,
                        Some(i) => Some(i + 1),
                    };
                }
                CreateField::HairColor => {
                    self.hair_color_index = (self.hair_color_index + 1) % HAIR_COLORS;
                }
                _ => {}
            }
        }

        // Keyboard: Cancel
        if is_key_pressed(KeyCode::Escape) {
            show_keyboard(false);
            return CreateAction::Cancel;
        }

        // Keyboard: Create character
        if is_key_pressed(KeyCode::Enter) {
            audio.play_sfx("enter");
            if let Some(action) = self.try_submit() {
                return action;
            }
        }

        CreateAction::None
    }

    /// Recessed gold-on-focus field box (matches login), faded by `alpha`.
    fn draw_field_box(&self, rect: Rect, active: bool, alpha: f32) {
        let fill = if active {
            Color::from_rgba(22, 22, 32, 250)
        } else {
            Color::from_rgba(14, 14, 20, 235)
        };
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, fade(fill, alpha));
        let border = if active { FRAME_ACCENT } else { FRAME_OUTER };
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, fade(border, alpha));
    }

    /// Draw a label above a field. Gold when active, dim otherwise.
    fn draw_field_label(
        &self,
        font: &BitmapFont,
        text: &str,
        x: f32,
        baseline: f32,
        active: bool,
        alpha: f32,
    ) {
        let color = if active { TEXT_TITLE } else { TEXT_DIM };
        font.draw_text(text, x, baseline, 16.0, fade(color, alpha));
    }

    /// Draw a `< value >` stepper inside an already-drawn field box.
    fn draw_stepper(
        &self,
        font: &BitmapFont,
        rect: Rect,
        value: &str,
        active: bool,
        enabled: bool,
        alpha: f32,
    ) {
        self.draw_field_box(rect, active && enabled, alpha);
        let arrow = if active && enabled {
            FRAME_ACCENT
        } else {
            TEXT_DIM
        };
        let val_color = if enabled { TEXT_NORMAL } else { TEXT_DIM };
        let by = rect.y + rect.h / 2.0 + 6.0;
        font.draw_text("<", rect.x + 12.0, by, 16.0, fade(arrow, alpha));
        let vw = font.measure_text(value, 16.0).width;
        font.draw_text(
            value,
            (rect.x + (rect.w - vw) / 2.0).floor(),
            by,
            16.0,
            fade(val_color, alpha),
        );
        font.draw_text(">", rect.x + rect.w - 20.0, by, 16.0, fade(arrow, alpha));
    }

    /// Draw the whole form (portrait + 4 rows + action buttons + error + hint)
    /// inside `l`, faded by `alpha`. The host draws the shared panel frame and
    /// title so they can crossfade independently during the morph.
    pub(crate) fn render(
        &self,
        font: &BitmapFont,
        player_sprites: &SpritesheetStore,
        hair_sprites: &SpritesheetStore,
        l: &CreateLayout,
        alpha: f32,
    ) {
        let (input_pos, _, _) = get_input_state();
        let (mx, my) = (input_pos.x, input_pos.y);

        // Portrait inset (recessed dark square + bronze edge + corner accents)
        let p = l.portrait;
        draw_rectangle(
            p.x,
            p.y,
            p.w,
            p.h,
            fade(Color::from_rgba(12, 12, 18, 255), alpha),
        );
        draw_rectangle_lines(p.x, p.y, p.w, p.h, 1.0, fade(FRAME_OUTER, alpha));
        // Centered sprite
        let sprite_x = (p.x + (p.w - SPRITE_WIDTH) / 2.0).floor();
        let sprite_y = (p.y + (p.h - SPRITE_HEIGHT) / 2.0).floor();
        let empty_equip = SpritesheetStore::Individual(HashMap::new());
        draw_character_preview(
            player_sprites,
            hair_sprites,
            &empty_equip,
            GENDERS[self.gender_index],
            SKINS[self.skin_index],
            self.hair_style_index.map(|i| i as i32),
            self.hair_color_index as i32,
            None,
            None,
            None,
            sprite_x,
            sprite_y,
            fade(WHITE, alpha),
        );

        // --- Row 0: Name ---
        let name_active = self.active_field == CreateField::Name;
        let name_box = l.row_box(0);
        self.draw_field_label(font, "Name", l.form_x, l.label_y(0), name_active, alpha);
        self.draw_field_box(name_box, name_active, alpha);
        let cursor = if name_active && (get_time() * 2.0) as i32 % 2 == 0 {
            "|"
        } else {
            ""
        };
        let name_display = if self.name.is_empty() && !name_active {
            "Enter name...".to_string()
        } else {
            format!("{}{}", self.name, cursor)
        };
        let name_color = if self.name.is_empty() && !name_active {
            TEXT_DIM
        } else {
            TEXT_NORMAL
        };
        font.draw_text(
            &name_display,
            name_box.x + 10.0,
            name_box.y + name_box.h / 2.0 + 6.0,
            16.0,
            fade(name_color, alpha),
        );

        // --- Row 1: Gender ---
        let gender_active = self.active_field == CreateField::Gender;
        self.draw_field_label(font, "Gender", l.form_x, l.label_y(1), gender_active, alpha);
        self.draw_stepper(
            font,
            l.row_box(1),
            GENDERS[self.gender_index],
            gender_active,
            true,
            alpha,
        );

        // --- Row 2: Skin ---
        let skin_active = self.active_field == CreateField::Skin;
        self.draw_field_label(font, "Skin", l.form_x, l.label_y(2), skin_active, alpha);
        self.draw_stepper(
            font,
            l.row_box(2),
            SKINS[self.skin_index],
            skin_active,
            true,
            alpha,
        );

        // --- Row 3: Hair (Style + Color) ---
        let (style_box, color_box) = l.hair_boxes();
        let style_active = self.active_field == CreateField::HairStyle;
        self.draw_field_label(
            font,
            "Style",
            style_box.x,
            l.label_y(3),
            style_active,
            alpha,
        );
        let style_value = match self.hair_style_index {
            None => "Bald".to_string(),
            Some(i) => format!("{}", i + 1),
        };
        self.draw_stepper(font, style_box, &style_value, style_active, true, alpha);

        let color_active = self.active_field == CreateField::HairColor;
        let color_enabled = self.hair_style_index.is_some();
        self.draw_field_label(
            font,
            "Color",
            color_box.x,
            l.label_y(3),
            color_active && color_enabled,
            alpha,
        );
        let color_value = if color_enabled {
            format!("{}", self.hair_color_index + 1)
        } else {
            "-".to_string()
        };
        self.draw_stepper(
            font,
            color_box,
            &color_value,
            color_active,
            color_enabled,
            alpha,
        );

        // --- Action bar (Create + Cancel) ---
        let (create_rect, cancel_rect) = l.button_rects();
        let create_hovered = point_in_rect(
            mx,
            my,
            create_rect.x,
            create_rect.y,
            create_rect.w,
            create_rect.h,
        );
        let cancel_hovered = point_in_rect(
            mx,
            my,
            cancel_rect.x,
            cancel_rect.y,
            cancel_rect.w,
            cancel_rect.h,
        );
        draw_screen_button_alpha(
            font,
            create_rect,
            "Create",
            create_hovered,
            ButtonVariant::Primary,
            alpha,
        );
        draw_screen_button_alpha(
            font,
            cancel_rect,
            "Cancel",
            cancel_hovered,
            ButtonVariant::Neutral,
            alpha,
        );

        // Error message (just above the action bar)
        if let Some(ref error) = self.error_message {
            let ew = font.measure_text(error, 16.0).width;
            font.draw_text(
                error,
                (l.panel.x + (l.panel.w - ew) / 2.0).floor(),
                l.action_bar.y - 8.0,
                16.0,
                fade(DANGER_TEXT, alpha),
            );
        }

        // Hint line below the action bar (hidden on touch devices)
        if !self.touch_detected {
            let hint = "[Tab] Next field \u{00b7} [A/D] Change \u{00b7} [Enter] Create \u{00b7} [Esc] Cancel";
            let hw = font.measure_text(hint, 16.0).width;
            let hint_y = l.action_bar.y + l.action_bar.h + 16.0;
            font.draw_text(
                hint,
                (l.panel.x + (l.panel.w - hw) / 2.0).floor(),
                hint_y,
                16.0,
                fade(TEXT_DIM, alpha),
            );
        }
    }
}

impl CharacterCreateScreen {
    pub fn new(session: AuthSession, server_url: &str) -> Self {
        Self {
            session,
            form: CreateForm::new(),
            auth_client: AuthClient::new(server_url),
            font: BitmapFont::default(),
            player_sprites: SpritesheetStore::Individual(HashMap::new()),
            hair_sprites: SpritesheetStore::Individual(HashMap::new()),
            starfield: StarfieldBackground::new(),
            #[cfg(target_arch = "wasm32")]
            loading: false,
        }
    }

    /// Use pre-loaded assets from the renderer (avoids duplicate loading)
    pub fn use_renderer_assets(
        &mut self,
        font: BitmapFont,
        player: SpritesheetStore,
        hair: SpritesheetStore,
    ) {
        self.font = font;
        self.player_sprites = player;
        self.hair_sprites = hair;
    }

    pub async fn load_font(&mut self) {
        // If assets were pre-loaded via use_renderer_assets(), skip loading
        if !self.player_sprites.is_empty() {
            return;
        }

        self.font =
            BitmapFont::load_or_default("assets/fonts/monogram/ttf/monogram-extended.ttf").await;
        self.player_sprites = load_player_sprites().await;

        let mut hair_map = HashMap::new();
        for style in 0..HAIR_STYLES as i32 {
            // Male hair sprites
            let path = asset_path(&format!("assets/sprites/hair/hair_{}.png", style));
            match load_texture(&path).await {
                Ok(tex) => {
                    tex.set_filter(FilterMode::Nearest);
                    hair_map.insert(format!("male_{}", style), tex);
                }
                Err(e) => {
                    log::warn!("Failed to load hair sprite {}: {}", path, e);
                }
            }
            // Female hair sprites
            let path = asset_path(&format!("assets/sprites/hair/hair_female_{}.png", style));
            match load_texture(&path).await {
                Ok(tex) => {
                    tex.set_filter(FilterMode::Nearest);
                    hair_map.insert(format!("female_{}", style), tex);
                }
                Err(e) => {
                    log::warn!("Failed to load female hair sprite {}: {}", path, e);
                }
            }
        }
        self.hair_sprites = SpritesheetStore::Individual(hair_map);
    }
}

impl Screen for CharacterCreateScreen {
    fn update(&mut self, audio: &AudioManager) -> ScreenState {
        // WASM: poll pending requests
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(result) = self.auth_client.poll() {
                self.loading = false;
                match result {
                    AuthResult::CharacterCreated(Ok(char_info)) => {
                        self.session.characters.push(char_info);
                        return ScreenState::ToCharacterSelect(self.session.clone());
                    }
                    AuthResult::CharacterCreated(Err(e)) => {
                        self.form.set_error(e.to_string());
                    }
                    _ => {}
                }
            }
        }

        let (sw, sh) = virtual_screen_size();
        self.starfield.update(get_frame_time(), sw, sh);

        let l = CreateLayout::compute(sw, sh);
        match self.form.update(&l, audio) {
            CreateAction::Cancel => {
                return ScreenState::ToCharacterSelect(self.session.clone());
            }
            CreateAction::Submit {
                name,
                gender,
                skin,
                hair_style,
                hair_color,
            } => {
                #[cfg(not(target_arch = "wasm32"))]
                match self.auth_client.create_character(
                    &self.session.token,
                    &name,
                    gender,
                    skin,
                    hair_style,
                    hair_color,
                ) {
                    Ok(char_info) => {
                        self.session.characters.push(char_info);
                        return ScreenState::ToCharacterSelect(self.session.clone());
                    }
                    Err(e) => {
                        self.form.set_error(e.to_string());
                    }
                }
                #[cfg(target_arch = "wasm32")]
                if !self.auth_client.is_busy() {
                    self.loading = true;
                    self.auth_client.start_create_character(
                        &self.session.token,
                        &name,
                        gender,
                        skin,
                        hair_style,
                        hair_color,
                    );
                }
            }
            CreateAction::None => {}
        }

        ScreenState::Continue
    }

    fn render(&self) {
        let (sw, sh) = virtual_screen_size();
        let l = CreateLayout::compute(sw, sh);

        // Background
        self.starfield.draw(sw, sh, 1.0);

        // Title (centered, above panel)
        let title = "CREATE CHARACTER";
        let tw = self.font.measure_text(title, 16.0).width;
        self.font.draw_text(
            title,
            ((sw - tw) / 2.0).floor(),
            l.header_y,
            16.0,
            TEXT_TITLE,
        );

        // Bronze-framed panel (shared box)
        draw_panel_frame(l.panel.x, l.panel.y, l.panel.w, l.panel.h);
        draw_corner_accents(l.panel.x, l.panel.y, l.panel.w, l.panel.h);

        // Form contents
        self.form.render(
            &self.font,
            &self.player_sprites,
            &self.hair_sprites,
            &l,
            1.0,
        );
    }
}
