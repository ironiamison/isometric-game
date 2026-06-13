use super::*;

use super::character_create::{CreateAction, CreateForm, CreateLayout};
use crate::render::ui::common::{
    draw_corner_accents, draw_panel_frame, draw_screen_button, draw_screen_button_alpha,
    ButtonVariant, CHIP_TEXT_DARK, DANGER_TEXT, FRAME_ACCENT, FRAME_INNER, FRAME_OUTER,
    FRAME_THICKNESS, PANEL_BG_DARK, TEXT_DIM, TEXT_GOLD, TEXT_NORMAL, TEXT_TITLE,
};

/// Which face of the character box is showing. `Create` overlays the roster
/// with the in-place creation form; `anim_t` drives the crossfade + resize.
#[derive(PartialEq, Clone, Copy)]
enum SelectMode {
    Roster,
    Create,
}

/// Duration (seconds) of the roster⇄create morph.
const MORPH_SECS: f32 = 0.18;

/// Smoothstep ease for the morph progress.
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Linear interpolate two rects.
fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::new(
        a.x + (b.x - a.x) * t,
        a.y + (b.y - a.y) * t,
        a.w + (b.w - a.w) * t,
        a.h + (b.h - a.h) * t,
    )
}

/// Maximum characters per account
const MAX_CHARACTERS: usize = 3;

const CARD_HEIGHT: f32 = 92.0;
const CARD_GAP: f32 = 6.0;
const ACTION_BAR_H: f32 = 44.0;
/// Fixed panel height for the empty-state invitation (no character cards).
const EMPTY_PANEL_H: f32 = 300.0;

/// Rows the roster list shows: character cards plus the create row (when under
/// the cap). Returns 0 for the empty state, which uses a fixed invitation panel.
fn card_row_count(characters_len: usize) -> usize {
    match characters_len {
        0 => 0,
        n if n < MAX_CHARACTERS => n + 1,
        n => n,
    }
}

/// Precomputed geometry for the character-select screen, computed once from the
/// screen size and shared by both `update` (hit-testing) and `render` (drawing)
/// so the two can never drift apart. (Mirrors login's `LoginLayout` pattern.)
struct CharSelectLayout {
    header_y: f32,       // baseline for the title / username row (above the panel)
    panel: Rect,         // bronze-framed roster panel
    list_x: f32,         // inner content x (inside frame padding)
    list_w: f32,         // inner content width
    list_top: f32,       // first card y (inside frame)
    list_visible_h: f32, // clip height for the scrollable list
    action_bar: Rect,    // full action-bar row below the panel
}

impl CharSelectLayout {
    /// Inner padding, gap below panel, and action-bar height — kept identical to
    /// `CreateLayout` so the roster box can morph (resize) into the create box.
    const PAD: f32 = FRAME_THICKNESS + 10.0;
    const ACTION_GAP_Y: f32 = 12.0;

    fn compute(sw: f32, sh: f32, card_rows: usize) -> Self {
        let panel_w = 540.0_f32.min(sw - 24.0);
        let panel_x = (sw - panel_w) / 2.0;
        let action_h = ACTION_BAR_H;
        let header_h = 28.0; // title sits just above the panel
        let hint_h = 28.0; // hint line sits just below the action bar
        let pad = Self::PAD;
        let item_height = CARD_HEIGHT + CARD_GAP;

        // Size the panel to its content (max 3 cards + create row) rather than
        // filling the screen. Empty state uses a fixed invitation height.
        let content_h = if card_rows == 0 {
            EMPTY_PANEL_H
        } else {
            card_rows as f32 * item_height + pad * 2.0
        };
        // Never exceed the available vertical space (forces scrolling on short
        // screens / when content is unexpectedly tall).
        let max_panel_h =
            ((sh - 24.0).max(200.0) - header_h - Self::ACTION_GAP_Y - action_h - hint_h).max(160.0);
        let panel_h = content_h.min(max_panel_h).max(160.0);

        // Center the whole group (header + panel + action bar + hint) vertically.
        let block_h = header_h + panel_h + Self::ACTION_GAP_Y + action_h + hint_h;
        let block_top = ((sh - block_h) / 2.0).max(12.0);
        let panel_top = block_top + header_h;
        let panel = Rect::new(panel_x, panel_top, panel_w, panel_h);

        Self::from_panel(panel)
    }

    /// Derive the inner roster geometry from a given panel rect. Header sits 8px
    /// above the panel and the action bar 12px below — matching `compute` and
    /// `CreateLayout` so transitions stay aligned.
    fn from_panel(panel: Rect) -> Self {
        let pad = Self::PAD;
        Self {
            header_y: panel.y - 8.0,
            list_x: panel.x + pad,
            list_w: panel.w - pad * 2.0,
            list_top: panel.y + pad,
            list_visible_h: panel.h - pad * 2.0,
            action_bar: Rect::new(
                panel.x,
                panel.y + panel.h + Self::ACTION_GAP_Y,
                panel.w,
                ACTION_BAR_H,
            ),
            panel,
        }
    }
}

/// Format played-time seconds as a compact `1m` / `9h 51m` / `149h 44m` string.
fn format_played_time(seconds: i64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

/// Fill color for a level chip: bronze at low levels warming to gold at high
/// levels. Linear ramp from level 1 (FRAME_OUTER) to level 100+ (FRAME_ACCENT).
fn level_chip_color(level: i32) -> Color {
    let t = ((level - 1) as f32 / 99.0).clamp(0.0, 1.0);
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    Color::new(
        lerp(FRAME_OUTER.r, FRAME_ACCENT.r),
        lerp(FRAME_OUTER.g, FRAME_ACCENT.g),
        lerp(FRAME_OUTER.b, FRAME_ACCENT.b),
        1.0,
    )
}

pub struct CharacterSelectScreen {
    session: AuthSession,
    characters: Vec<CharacterInfo>,
    selected_index: usize,
    error_message: Option<String>,
    auth_client: AuthClient,
    font: BitmapFont,
    confirm_delete: bool,
    player_sprites: SpritesheetStore,
    hair_sprites: SpritesheetStore,
    equipment_sprites: SpritesheetStore,
    // Scroll state for character list on small screens
    list_scroll_offset: f32,
    touch_scroll_id: Option<u64>,
    touch_scroll_last_y: f32,
    /// When true, spectator world is rendered behind this screen — use dark overlay instead of solid bg
    pub has_spectator_backdrop: bool,
    starfield: StarfieldBackground,
    /// Roster vs. in-place create form.
    mode: SelectMode,
    /// Morph progress: 0 = full roster, 1 = full create form.
    anim_t: f32,
    /// Embedded creation form (used when `mode == Create`).
    create_form: CreateForm,
    #[cfg(target_arch = "wasm32")]
    loading: bool,
    #[cfg(target_arch = "wasm32")]
    needs_initial_load: bool,
    #[cfg(target_arch = "wasm32")]
    needs_equipment_load: bool,
}

impl CharacterSelectScreen {
    pub fn new(session: AuthSession, server_url: &str) -> Self {
        let auth_client = AuthClient::new(server_url);

        // Use characters from login response (no separate request needed)
        let characters = session.characters.clone();

        Self {
            session,
            characters,
            selected_index: 0,
            error_message: None,
            auth_client,
            font: BitmapFont::default(),
            confirm_delete: false,
            player_sprites: SpritesheetStore::Individual(HashMap::new()),
            hair_sprites: SpritesheetStore::Individual(HashMap::new()),
            equipment_sprites: SpritesheetStore::Individual(HashMap::new()),
            list_scroll_offset: 0.0,
            touch_scroll_id: None,
            touch_scroll_last_y: 0.0,
            has_spectator_backdrop: false,
            starfield: StarfieldBackground::new(),
            mode: SelectMode::Roster,
            anim_t: 0.0,
            create_form: CreateForm::new(),
            #[cfg(target_arch = "wasm32")]
            loading: false,
            #[cfg(target_arch = "wasm32")]
            needs_initial_load: true,
            #[cfg(target_arch = "wasm32")]
            needs_equipment_load: false,
        }
    }

    /// Use pre-loaded assets from the renderer (avoids duplicate loading)
    pub fn use_renderer_assets(
        &mut self,
        font: BitmapFont,
        player: SpritesheetStore,
        hair: SpritesheetStore,
        equipment: SpritesheetStore,
    ) {
        self.font = font;
        self.player_sprites = player;
        self.hair_sprites = hair;
        self.equipment_sprites = equipment;
    }

    /// Load font and sprites asynchronously - call this after creating the screen
    pub async fn load_font(&mut self) {
        // If assets were pre-loaded via use_renderer_assets(), skip loading
        if !self.player_sprites.is_empty() {
            return;
        }

        self.font =
            BitmapFont::load_or_default("assets/fonts/monogram/ttf/monogram-extended.ttf").await;

        // Load sprites from atlas or individually
        let manifest = SpriteManifest::load().await;

        if let Some(ref atlas_info) = manifest.players_atlas {
            if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                self.player_sprites = SpritesheetStore::Atlas {
                    texture: tex,
                    rects,
                };
            }
        }
        if self.player_sprites.is_empty() {
            self.player_sprites = load_player_sprites().await;
        }

        if let Some(ref atlas_info) = manifest.hair_atlas {
            if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                self.hair_sprites = SpritesheetStore::Atlas {
                    texture: tex,
                    rects,
                };
            }
        }
        if self.hair_sprites.is_empty() {
            let mut hair_map = HashMap::new();
            for style in 0..6i32 {
                let path = asset_path(&format!("assets/sprites/hair/hair_{}.png", style));
                if let Ok(tex) = load_texture(&path).await {
                    tex.set_filter(FilterMode::Nearest);
                    hair_map.insert(format!("male_{}", style), tex);
                }
                let path = asset_path(&format!("assets/sprites/hair/hair_female_{}.png", style));
                if let Ok(tex) = load_texture(&path).await {
                    tex.set_filter(FilterMode::Nearest);
                    hair_map.insert(format!("female_{}", style), tex);
                }
            }
            self.hair_sprites = SpritesheetStore::Individual(hair_map);
        }

        self.load_equipment_sprites(&manifest).await;
    }

    async fn load_equipment_sprites(&mut self, manifest: &SpriteManifest) {
        // Try atlas first
        if let Some(ref atlas_info) = manifest.equipment_atlas {
            if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                self.equipment_sprites = SpritesheetStore::Atlas {
                    texture: tex,
                    rects,
                };
                return;
            }
        }

        // Fallback: load individual equipment sprites
        let mut sprite_keys: Vec<String> = Vec::new();
        for c in &self.characters {
            for key in [
                &c.sprite_head,
                &c.sprite_body,
                &c.sprite_weapon,
                &c.sprite_back,
                &c.sprite_feet,
            ]
            .into_iter()
            .flatten()
            {
                if !sprite_keys.contains(key) && !self.equipment_sprites.contains(key) {
                    sprite_keys.push(key.clone());
                }
            }
        }

        if sprite_keys.is_empty() {
            return;
        }

        let mut equip_map = HashMap::new();
        for sprite_key in &sprite_keys {
            for entry in &manifest.equipment {
                let key = entry.rsplit('/').next().unwrap_or(entry);
                if key == sprite_key {
                    let path = asset_path(&format!("assets/sprites/{}.png", entry));
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        equip_map.insert(sprite_key.clone(), tex);
                    }
                    break;
                }
            }
        }
        self.equipment_sprites = SpritesheetStore::Individual(equip_map);
    }

    /// Load equipment sprites if characters have been loaded (WASM only)
    #[cfg(target_arch = "wasm32")]
    pub async fn load_equipment_if_needed(&mut self) {
        // Skip if equipment already loaded from renderer
        if self.equipment_sprites.len() > 0 {
            self.needs_equipment_load = false;
            return;
        }
        if self.needs_equipment_load {
            self.needs_equipment_load = false;
            let manifest = SpriteManifest::load().await;
            self.load_equipment_sprites(&manifest).await;
        }
    }

    /// Set an error message to display
    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
    }

    /// Refresh the character list from the server
    pub fn refresh_characters(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Ok(chars) = self.auth_client.get_characters(&self.session.token) {
                self.characters = chars;
                // Keep the session in sync so navigating away and back (which
                // rebuilds this screen from the session) doesn't resurrect a
                // deleted character.
                self.session.characters = self.characters.clone();
                if self.selected_index >= self.characters.len() && !self.characters.is_empty() {
                    self.selected_index = self.characters.len() - 1;
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        if !self.auth_client.is_busy() {
            self.loading = true;
            self.auth_client.start_get_characters(&self.session.token);
        }
    }

    /// Draw text with pixel font for sharp rendering
    fn draw_text_sharp(&self, text: &str, x: f32, y: f32, font_size: f32, color: Color) {
        self.font.draw_text(text, x, y, font_size, color);
    }

    fn measure_text_sharp(&self, text: &str, font_size: f32) -> TextDimensions {
        self.font.measure_text(text, font_size)
    }

    /// Draw a single character roster card (portrait, name, level chip, meta, played time).
    fn draw_character_card(
        &self,
        rect: Rect,
        character: &CharacterInfo,
        selected: bool,
        hovered: bool,
        alpha: f32,
    ) {
        // Card fill
        let fill = if selected {
            Color::from_rgba(46, 38, 22, 255) // warm gold-tinted recess
        } else if hovered {
            Color::from_rgba(30, 30, 42, 255)
        } else {
            PANEL_BG_DARK
        };
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, fade(fill, alpha));

        if selected {
            draw_rectangle(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                fade(
                    Color {
                        a: 0.06,
                        ..FRAME_ACCENT
                    },
                    alpha,
                ),
            );
            draw_rectangle_lines(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                2.0,
                fade(FRAME_ACCENT, alpha),
            );
        } else if hovered {
            // 2px (not 1px) so all four edges land on whole pixels — a 1px
            // outline drops the top/left edges to sub-pixel rounding, leaving
            // only the bottom/right visible. A faint wash lifts the whole box.
            draw_rectangle(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                fade(
                    Color {
                        a: 0.04,
                        ..FRAME_INNER
                    },
                    alpha,
                ),
            );
            draw_rectangle_lines(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                2.0,
                fade(FRAME_INNER, alpha),
            );
        } else {
            draw_rectangle_lines(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                1.0,
                fade(FRAME_OUTER, alpha),
            );
        }

        // Portrait inset (recessed dark square with bronze edge)
        let inset = rect.h - 12.0;
        let inset_x = (rect.x + 8.0).floor();
        let inset_y = (rect.y + 6.0).floor();
        draw_rectangle(
            inset_x,
            inset_y,
            inset,
            inset,
            fade(Color::from_rgba(12, 12, 18, 255), alpha),
        );
        draw_rectangle_lines(
            inset_x,
            inset_y,
            inset,
            inset,
            1.0,
            fade(FRAME_OUTER, alpha),
        );

        // Composite character sprite, centered in the inset
        let preview_x = (inset_x + (inset - SPRITE_WIDTH) / 2.0).floor();
        let preview_y = (inset_y + (inset - SPRITE_HEIGHT) / 2.0).floor();
        draw_character_preview(
            &self.player_sprites,
            &self.hair_sprites,
            &self.equipment_sprites,
            &character.gender,
            &character.skin,
            character.hair_style,
            character.hair_color.unwrap_or(0),
            character.sprite_body.as_deref(),
            character.sprite_back.as_deref(),
            character.sprite_feet.as_deref(),
            preview_x,
            preview_y,
            fade(WHITE, alpha),
        );

        // Text column (vertically centered on the card's midline)
        let center_y = rect.y + rect.h / 2.0;
        let text_x = inset_x + inset + 12.0;
        let name_color = if selected { TEXT_TITLE } else { TEXT_NORMAL };
        self.draw_text_sharp(
            &character.name,
            text_x,
            center_y - 8.0,
            16.0,
            fade(name_color, alpha),
        );

        // Level chip
        let chip_label = format!("Lv {}", character.level);
        let chip_text_w = self.measure_text_sharp(&chip_label, 16.0).width;
        let chip_w = chip_text_w + 14.0;
        let chip_h = 18.0;
        let chip_x = text_x;
        let chip_y = center_y + 2.0;
        let chip_color = level_chip_color(character.level);
        draw_rectangle(chip_x, chip_y, chip_w, chip_h, fade(chip_color, alpha));
        // Readable label: light text on dark (low-level) chips, dark text on
        // bright gold (high-level) chips.
        let chip_lum = 0.299 * chip_color.r + 0.587 * chip_color.g + 0.114 * chip_color.b;
        let chip_text = if chip_lum < 0.5 {
            TEXT_NORMAL
        } else {
            CHIP_TEXT_DARK
        };
        self.draw_text_sharp(
            &chip_label,
            chip_x + 7.0,
            chip_y + 14.0,
            16.0,
            fade(chip_text, alpha),
        );

        // Played time (right-aligned) + dim "played" label beneath
        let time_str = format_played_time(character.played_time);
        let tw = self.measure_text_sharp(&time_str, 16.0).width;
        let right = rect.x + rect.w - 12.0;
        self.draw_text_sharp(
            &time_str,
            right - tw,
            center_y - 8.0,
            16.0,
            fade(TEXT_NORMAL, alpha),
        );
        let pl = "played";
        let plw = self.measure_text_sharp(pl, 16.0).width;
        self.draw_text_sharp(
            pl,
            right - plw,
            center_y + 12.0,
            16.0,
            fade(TEXT_DIM, alpha),
        );
    }

    /// Action-bar button rects when characters exist: [Play, Delete, Logout].
    fn action_button_rects(l: &CharSelectLayout) -> [Rect; 3] {
        let gap = 8.0;
        let bw = ((l.action_bar.w - gap * 2.0) / 3.0).max(0.0);
        let bh = l.action_bar.h;
        let by = l.action_bar.y;
        [
            Rect::new(l.action_bar.x, by, bw, bh),
            Rect::new(l.action_bar.x + bw + gap, by, bw, bh),
            Rect::new(l.action_bar.x + (bw + gap) * 2.0, by, bw, bh),
        ]
    }

    /// Right-aligned Logout button rect for the empty state.
    fn empty_logout_rect(l: &CharSelectLayout) -> Rect {
        let bw = 110.0_f32;
        Rect::new(
            l.action_bar.x + l.action_bar.w - bw,
            l.action_bar.y,
            bw,
            l.action_bar.h,
        )
    }

    /// Centered "+ Create Character" button rect for the empty state.
    /// MUST match the y-chain render uses (circle_cy + 56 + 28 + 20, then +24).
    fn empty_create_rect(l: &CharSelectLayout) -> Rect {
        let cx = l.panel.x + l.panel.w / 2.0;
        let circle_cy = l.panel.y + l.panel.h * 0.34;
        let ty = circle_cy + 56.0 + 28.0 + 20.0;
        let btn_w = 180.0_f32;
        let btn_h = 34.0_f32;
        Rect::new(
            (cx - btn_w / 2.0).floor(),
            (ty + 24.0).floor(),
            btn_w,
            btn_h,
        )
    }

    /// Draw a dashed rectangle outline (used for the create-new-character row).
    fn draw_dashed_rect(&self, x: f32, y: f32, w: f32, h: f32, color: Color, alpha: f32) {
        let color = fade(color, alpha);
        let dash = 6.0;
        let gap = 4.0;
        let mut dx = x;
        while dx < x + w {
            let e = (dx + dash).min(x + w);
            draw_line(dx, y, e, y, 1.0, color);
            draw_line(dx, y + h, e, y + h, 1.0, color);
            dx += dash + gap;
        }
        let mut dy = y;
        while dy < y + h {
            let e = (dy + dash).min(y + h);
            draw_line(x, dy, x, e, 1.0, color);
            draw_line(x + w, dy, x + w, e, 1.0, color);
            dy += dash + gap;
        }
    }

    /// Begin the in-place create flow: fresh form, animate the box into it.
    fn enter_create(&mut self) {
        self.create_form.reset();
        self.mode = SelectMode::Create;
    }

    /// Leave the create flow, animating the box back to the roster.
    fn exit_create(&mut self) {
        self.mode = SelectMode::Roster;
    }

    /// Append a freshly created character to both the live list and the session
    /// (kept in sync, per the delete-resurrection fix) and select it.
    fn add_character(&mut self, character: CharacterInfo) {
        self.characters.push(character.clone());
        self.session.characters.push(character);
        self.selected_index = self.characters.len().saturating_sub(1);
        self.error_message = None;
    }

    /// Geometry of the dashed "+ Create new character" row. When the list isn't
    /// scrolling (the desktop common case) the row stretches to fill the leftover
    /// panel space so its bottom inset matches the left/right padding — equal
    /// margins on all sides. While scrolling (small mobile screens) it keeps a
    /// single card height so the list stays uniform.
    fn create_row_rect(
        &self,
        l: &CharSelectLayout,
        scroll_offset: f32,
        needs_scroll: bool,
    ) -> Rect {
        let item_height = CARD_HEIGHT + CARD_GAP;
        let top = l.list_top + self.characters.len() as f32 * item_height - scroll_offset;
        let h = if needs_scroll {
            CARD_HEIGHT
        } else {
            (l.list_top + l.list_visible_h - top).max(CARD_HEIGHT)
        };
        Rect::new(l.list_x, top, l.list_w, h)
    }

    /// Draw the roster contents (cards/empty-state, action bar, error, hint)
    /// inside `l`, faded by `alpha`. The caller draws the shared panel frame and
    /// the (crossfaded) title so the box can morph independently of its content.
    fn render_roster(&self, l: &CharSelectLayout, alpha: f32) {
        let (sw, sh) = virtual_screen_size();
        let (input_pos, _, _) = get_input_state();
        let (mx, my) = (input_pos.x, input_pos.y);

        if self.characters.is_empty() {
            // ---- Empty state: centered invitation inside the panel ----
            let cx = l.panel.x + l.panel.w / 2.0;
            let circle_cy = l.panel.y + l.panel.h * 0.34;
            draw_circle_lines(cx, circle_cy, 28.0, 2.0, fade(FRAME_OUTER, alpha));
            // A simple "+" inside the circle
            draw_line(
                cx - 10.0,
                circle_cy,
                cx + 10.0,
                circle_cy,
                2.0,
                fade(FRAME_ACCENT, alpha),
            );
            draw_line(
                cx,
                circle_cy - 10.0,
                cx,
                circle_cy + 10.0,
                2.0,
                fade(FRAME_ACCENT, alpha),
            );

            let headline = "Your story begins here";
            let hw = self.measure_text_sharp(headline, 16.0).width;
            let mut ty = circle_cy + 56.0;
            self.draw_text_sharp(
                headline,
                (cx - hw / 2.0).floor(),
                ty,
                16.0,
                fade(TEXT_TITLE, alpha),
            );

            ty += 28.0;
            let line1 = "No heroes yet. Create your first character";
            let l1w = self.measure_text_sharp(line1, 16.0).width;
            self.draw_text_sharp(
                line1,
                (cx - l1w / 2.0).floor(),
                ty,
                16.0,
                fade(TEXT_DIM, alpha),
            );
            ty += 20.0;
            let line2 = "to set foot in the realm of Aeven.";
            let l2w = self.measure_text_sharp(line2, 16.0).width;
            self.draw_text_sharp(
                line2,
                (cx - l2w / 2.0).floor(),
                ty,
                16.0,
                fade(TEXT_DIM, alpha),
            );

            // Centered Create Character button
            let create_rect = Self::empty_create_rect(l);
            let create_hovered = point_in_rect(
                mx,
                my,
                create_rect.x,
                create_rect.y,
                create_rect.w,
                create_rect.h,
            );
            draw_screen_button_alpha(
                &self.font,
                create_rect,
                "+ Create Character",
                create_hovered,
                ButtonVariant::Primary,
                alpha,
            );
        } else {
            // ---- Cards list with scissor clipping + scrollbar ----
            let item_height = CARD_HEIGHT + CARD_GAP;
            let row_count = self.characters.len()
                + if self.characters.len() < MAX_CHARACTERS {
                    1
                } else {
                    0
                };
            let total_list_height = row_count as f32 * item_height;
            let max_scroll = (total_list_height - l.list_visible_h).max(0.0);
            let scroll_offset = self.list_scroll_offset.clamp(0.0, max_scroll);
            let needs_scroll = max_scroll > 0.0;

            // Set up scissor clipping for the list area
            if needs_scroll {
                let physical_w = screen_width();
                let physical_h = screen_height();
                let scale_x = physical_w / sw;
                let scale_y = physical_h / sh;
                let mut gl = unsafe { macroquad::window::get_internal_gl() };
                gl.flush();
                gl.quad_gl.scissor(Some((
                    (l.list_x * scale_x) as i32,
                    (l.list_top * scale_y) as i32,
                    (l.list_w * scale_x) as i32,
                    (l.list_visible_h * scale_y) as i32,
                )));
            }

            for (i, character) in self.characters.iter().enumerate() {
                let card_y = l.list_top + i as f32 * item_height - scroll_offset;
                // Skip rows fully outside the visible area
                if card_y + CARD_HEIGHT < l.list_top || card_y > l.list_top + l.list_visible_h {
                    continue;
                }
                let card_rect = Rect::new(l.list_x, card_y, l.list_w, CARD_HEIGHT);
                let is_selected = i == self.selected_index;
                let is_hovered = point_in_rect(mx, my, l.list_x, card_y, l.list_w, CARD_HEIGHT)
                    && my >= l.list_top
                    && my <= l.list_top + l.list_visible_h;
                self.draw_character_card(card_rect, character, is_selected, is_hovered, alpha);
            }

            // Create row (only when below the cap), inside the clipped list.
            // The dashed border lights up bright gold on hover (or keyboard
            // selection); otherwise it sits quiet in dim bronze.
            if self.characters.len() < MAX_CHARACTERS {
                let create_idx = self.characters.len();
                let row = self.create_row_rect(l, scroll_offset, needs_scroll);
                if !(row.y + row.h < l.list_top || row.y > l.list_top + l.list_visible_h) {
                    let is_create_selected = self.selected_index == create_idx;
                    let is_create_hovered = point_in_rect(mx, my, row.x, row.y, row.w, row.h)
                        && my >= l.list_top
                        && my <= l.list_top + l.list_visible_h;
                    let outline = if is_create_selected {
                        TEXT_GOLD
                    } else if is_create_hovered {
                        FRAME_ACCENT
                    } else {
                        FRAME_OUTER
                    };
                    self.draw_dashed_rect(row.x, row.y, row.w, row.h, outline, alpha);

                    let label = "+ Create new character";
                    let lw = self.measure_text_sharp(label, 16.0).width;
                    let label_x = (row.x + (row.w - lw) / 2.0).floor();
                    let label_y = row.y + row.h / 2.0 + 6.0;
                    let label_color =
                        Color::new(FRAME_ACCENT.r, FRAME_ACCENT.g, FRAME_ACCENT.b, 0.9);
                    self.draw_text_sharp(label, label_x, label_y, 16.0, fade(label_color, alpha));
                }
            }

            // Disable scissor clipping, then draw the scrollbar — but only when
            // fully settled in the roster (alpha == 1). Mid-morph the panel
            // shrinks below the content and would briefly spawn a scrollbar; on
            // desktop that flicker is pure noise (the bar only matters on small
            // mobile screens), so suppress it during the transition.
            if needs_scroll {
                let mut gl = unsafe { macroquad::window::get_internal_gl() };
                gl.flush();
                gl.quad_gl.scissor(None);

                if alpha >= 0.999 {
                    let scrollbar_w = 4.0;
                    let scrollbar_x = l.list_x + l.list_w - 6.0;
                    let track_h = l.list_visible_h;
                    let thumb_ratio = l.list_visible_h / total_list_height;
                    let thumb_h = (track_h * thumb_ratio).max(20.0);
                    let scroll_ratio = if max_scroll > 0.0 {
                        scroll_offset / max_scroll
                    } else {
                        0.0
                    };
                    let thumb_y = l.list_top + (track_h - thumb_h) * scroll_ratio;

                    // Track
                    draw_rectangle(
                        scrollbar_x,
                        l.list_top,
                        scrollbar_w,
                        track_h,
                        Color::new(0.855, 0.698, 0.424, 0.10),
                    );
                    // Thumb
                    draw_rectangle(
                        scrollbar_x,
                        thumb_y,
                        scrollbar_w,
                        thumb_h,
                        Color::new(1.0, 1.0, 1.0, 0.3),
                    );
                }
            }
        }

        // ---- Action bar (outside the clipped region) ----
        if self.characters.is_empty() {
            // Single right-aligned Logout button
            let logout_rect = Self::empty_logout_rect(l);
            let logout_hovered = point_in_rect(
                mx,
                my,
                logout_rect.x,
                logout_rect.y,
                logout_rect.w,
                logout_rect.h,
            );
            draw_screen_button_alpha(
                &self.font,
                logout_rect,
                "Logout",
                logout_hovered,
                ButtonVariant::Neutral,
                alpha,
            );
        } else {
            let [play_rect, del_rect, logout_rect] = Self::action_button_rects(l);

            let play_hovered =
                point_in_rect(mx, my, play_rect.x, play_rect.y, play_rect.w, play_rect.h);
            draw_screen_button_alpha(
                &self.font,
                play_rect,
                "Play",
                play_hovered,
                ButtonVariant::Primary,
                alpha,
            );

            let del_hovered = point_in_rect(mx, my, del_rect.x, del_rect.y, del_rect.w, del_rect.h);
            draw_screen_button_alpha(
                &self.font,
                del_rect,
                "Delete",
                del_hovered,
                ButtonVariant::Danger,
                alpha,
            );

            let logout_hovered = point_in_rect(
                mx,
                my,
                logout_rect.x,
                logout_rect.y,
                logout_rect.w,
                logout_rect.h,
            );
            draw_screen_button_alpha(
                &self.font,
                logout_rect,
                "Logout",
                logout_hovered,
                ButtonVariant::Neutral,
                alpha,
            );
        }

        // Error message (just above the action bar)
        if let Some(ref error) = self.error_message {
            let ew = self.measure_text_sharp(error, 16.0).width;
            self.draw_text_sharp(
                error,
                ((sw - ew) / 2.0).floor(),
                l.action_bar.y - 8.0,
                16.0,
                fade(DANGER_TEXT, alpha),
            );
        }

        // Hint line below the action bar
        let hint_y = l.action_bar.y + l.action_bar.h + 16.0;
        let hint: &str = if self.characters.is_empty() {
            "[N] create character"
        } else {
            #[cfg(not(target_os = "android"))]
            {
                "[W/S] navigate \u{00b7} [Enter] play \u{00b7} [N] new"
            }
            #[cfg(target_os = "android")]
            {
                "[Enter] play \u{00b7} [N] new"
            }
        };
        let hw = self.measure_text_sharp(hint, 16.0).width;
        self.draw_text_sharp(
            hint,
            ((sw - hw) / 2.0).floor(),
            hint_y,
            16.0,
            fade(TEXT_DIM, alpha),
        );
    }
}

impl Screen for CharacterSelectScreen {
    fn update(&mut self, audio: &AudioManager) -> ScreenState {
        // Advance the roster⇄create morph toward its target.
        let target = if self.mode == SelectMode::Create {
            1.0
        } else {
            0.0
        };
        if self.anim_t != target {
            let step = get_frame_time() / MORPH_SECS;
            self.anim_t = if self.anim_t < target {
                (self.anim_t + step).min(target)
            } else {
                (self.anim_t - step).max(target)
            };
        }

        // WASM: poll pending requests (characters now come with login response)
        #[cfg(target_arch = "wasm32")]
        {
            // Skip initial load - characters are included in login response
            if self.needs_initial_load {
                self.needs_initial_load = false;
            }
            if let Some(result) = self.auth_client.poll() {
                self.loading = false;
                match result {
                    AuthResult::Characters(Ok(chars)) => {
                        self.characters = chars;
                        // Keep the session in sync so navigating away and back
                        // (which rebuilds this screen from the session) doesn't
                        // resurrect a deleted character.
                        self.session.characters = self.characters.clone();
                        if self.selected_index >= self.characters.len()
                            && !self.characters.is_empty()
                        {
                            self.selected_index = self.characters.len() - 1;
                        }
                        self.needs_equipment_load = true;
                    }
                    AuthResult::Characters(Err(e)) => {
                        self.error_message = Some(e.to_string());
                    }
                    AuthResult::CharacterDeleted(Ok(())) => {
                        // Refresh after delete
                        self.refresh_characters();
                    }
                    AuthResult::CharacterDeleted(Err(e)) => {
                        self.error_message = Some(e.to_string());
                    }
                    AuthResult::CharacterCreated(Ok(char_info)) => {
                        self.add_character(char_info);
                        self.needs_equipment_load = true;
                        self.exit_create();
                    }
                    AuthResult::CharacterCreated(Err(e)) => {
                        self.create_form.set_error(e.to_string());
                    }
                    _ => {}
                }
            }
        }

        let (sw, sh) = virtual_screen_size();
        self.starfield.update(get_frame_time(), sw, sh);

        // ---- In-place create mode ----
        if self.mode == SelectMode::Create {
            // Only accept form input once the morph has fully settled, so the
            // click that opened the form (and the moving rects mid-tween) can't
            // be mis-hit.
            if self.anim_t >= 1.0 {
                let lc = CreateLayout::compute(sw, sh);
                match self.create_form.update(&lc, audio) {
                    CreateAction::Cancel => self.exit_create(),
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
                                self.add_character(char_info);
                                self.exit_create();
                            }
                            Err(e) => self.create_form.set_error(e.to_string()),
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
            }
            return ScreenState::Continue;
        }

        // While the box is still animating back to the roster, swallow roster
        // input until it settles.
        if self.anim_t > 0.0 {
            return ScreenState::Continue;
        }

        let (input_pos, clicked, _is_touch) = get_input_state();
        let mx = input_pos.x;
        let my = input_pos.y;

        let l = CharSelectLayout::compute(sw, sh, card_row_count(self.characters.len()));
        let item_height = CARD_HEIGHT + CARD_GAP;
        let create_selectable =
            !self.characters.is_empty() && self.characters.len() < MAX_CHARACTERS;
        // Number of rows in the scrollable list, including the create row when shown.
        let row_count = self.characters.len() + if create_selectable { 1 } else { 0 };
        let total_list_height = row_count as f32 * item_height;
        let max_scroll = (total_list_height - l.list_visible_h).max(0.0);
        self.list_scroll_offset = self.list_scroll_offset.clamp(0.0, max_scroll);

        // Touch drag scrolling (only when there is a list to scroll)
        if !self.characters.is_empty() {
            let all_touches: Vec<Touch> = touches();
            if let Some(tracking_id) = self.touch_scroll_id {
                if let Some(touch) = all_touches.iter().find(|t| t.id == tracking_id) {
                    match touch.phase {
                        TouchPhase::Moved | TouchPhase::Stationary => {
                            let (_, vy) = screen_to_virtual(touch.position.x, touch.position.y);
                            let dy = self.touch_scroll_last_y - vy;
                            self.list_scroll_offset =
                                (self.list_scroll_offset + dy).clamp(0.0, max_scroll);
                            self.touch_scroll_last_y = vy;
                        }
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            self.touch_scroll_id = None;
                        }
                        _ => {}
                    }
                } else {
                    self.touch_scroll_id = None;
                }
            } else if !self.confirm_delete {
                for touch in &all_touches {
                    if touch.phase == TouchPhase::Started {
                        let (vx, vy) = screen_to_virtual(touch.position.x, touch.position.y);
                        // Only start scroll if touch is in the list area
                        if vx >= l.list_x
                            && vx <= l.list_x + l.list_w
                            && vy >= l.list_top
                            && vy <= l.list_top + l.list_visible_h
                        {
                            self.touch_scroll_id = Some(touch.id);
                            self.touch_scroll_last_y = vy;
                            break;
                        }
                    }
                }
            }

            // Mouse wheel scrolling
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y != 0.0 && my >= l.list_top && my <= l.list_top + l.list_visible_h {
                self.list_scroll_offset =
                    (self.list_scroll_offset - wheel_y * 30.0).clamp(0.0, max_scroll);
            }
        }

        // Delete confirmation mode
        if self.confirm_delete {
            // Keyboard shortcuts
            if is_key_pressed(KeyCode::Y) {
                if self.selected_index < self.characters.len() {
                    let char_id = self.characters[self.selected_index].id;
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if self
                            .auth_client
                            .delete_character(&self.session.token, char_id)
                            .is_ok()
                        {
                            self.refresh_characters();
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    if !self.auth_client.is_busy() {
                        self.loading = true;
                        self.auth_client
                            .start_delete_character(&self.session.token, char_id);
                    }
                }
                self.confirm_delete = false;
                return ScreenState::Continue;
            }
            if is_key_pressed(KeyCode::N) || is_key_pressed(KeyCode::Escape) {
                self.confirm_delete = false;
                return ScreenState::Continue;
            }

            // Mouse clicks on Yes/No buttons
            if clicked {
                let box_w = 450.0_f32.min(sw - 20.0);
                let box_h = 150.0;
                let box_x = (sw - box_w) / 2.0;
                let box_y = (sh - box_h) / 2.0;

                // Yes button area (left side of button text)
                let yes_x = box_x + 70.0;
                let yes_y = box_y + 85.0;
                if point_in_rect(mx, my, yes_x, yes_y, 100.0, 30.0) {
                    if self.selected_index < self.characters.len() {
                        let char_id = self.characters[self.selected_index].id;
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            if self
                                .auth_client
                                .delete_character(&self.session.token, char_id)
                                .is_ok()
                            {
                                self.refresh_characters();
                            }
                        }
                        #[cfg(target_arch = "wasm32")]
                        if !self.auth_client.is_busy() {
                            self.loading = true;
                            self.auth_client
                                .start_delete_character(&self.session.token, char_id);
                        }
                    }
                    self.confirm_delete = false;
                    return ScreenState::Continue;
                }

                // No button area (right side of button text)
                let no_x = box_x + 250.0;
                if point_in_rect(mx, my, no_x, yes_y, 100.0, 30.0) {
                    self.confirm_delete = false;
                    return ScreenState::Continue;
                }
            }

            return ScreenState::Continue;
        }

        // Mouse: click on character cards / create row (mirrors render's list geometry)
        if clicked && !self.characters.is_empty() {
            let scroll_offset = self.list_scroll_offset;
            let needs_scroll = max_scroll > 0.0;
            for i in 0..row_count {
                let is_create_row = create_selectable && i == self.characters.len();
                // Create row may be stretched to fill the panel (see
                // create_row_rect); character rows are a fixed card height.
                let rect = if is_create_row {
                    self.create_row_rect(&l, scroll_offset, needs_scroll)
                } else {
                    Rect::new(
                        l.list_x,
                        l.list_top + i as f32 * item_height - scroll_offset,
                        l.list_w,
                        CARD_HEIGHT,
                    )
                };
                // visible-band check identical to render
                if rect.y + rect.h < l.list_top || rect.y > l.list_top + l.list_visible_h {
                    continue;
                }
                if point_in_rect(mx, my, rect.x, rect.y, rect.w, rect.h)
                    && my >= l.list_top
                    && my <= l.list_top + l.list_visible_h
                {
                    if is_create_row {
                        self.enter_create();
                        return ScreenState::Continue;
                    }
                    if i == self.selected_index {
                        let character = &self.characters[i];
                        return ScreenState::StartGame {
                            session: self.session.clone(),
                            character_id: character.id,
                            character_name: character.name.clone(),
                        };
                    } else {
                        self.selected_index = i;
                    }
                    break;
                }
            }
        }

        // Mouse: action-bar / empty-state button click handling
        if clicked {
            if self.characters.is_empty() {
                // Empty state: Create button (in panel) + Logout (action bar)
                let create_rect = Self::empty_create_rect(&l);
                if point_in_rect(
                    mx,
                    my,
                    create_rect.x,
                    create_rect.y,
                    create_rect.w,
                    create_rect.h,
                ) {
                    {
                        self.enter_create();
                        return ScreenState::Continue;
                    }
                }
                let logout_rect = Self::empty_logout_rect(&l);
                if point_in_rect(
                    mx,
                    my,
                    logout_rect.x,
                    logout_rect.y,
                    logout_rect.w,
                    logout_rect.h,
                ) {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let _ = self.auth_client.logout(&self.session.token);
                    }
                    return ScreenState::ToLogin;
                }
            } else {
                let [play_rect, del_rect, logout_rect] = Self::action_button_rects(&l);
                if point_in_rect(mx, my, play_rect.x, play_rect.y, play_rect.w, play_rect.h)
                    && self.selected_index < self.characters.len()
                {
                    let character = &self.characters[self.selected_index];
                    return ScreenState::StartGame {
                        session: self.session.clone(),
                        character_id: character.id,
                        character_name: character.name.clone(),
                    };
                }
                if point_in_rect(mx, my, del_rect.x, del_rect.y, del_rect.w, del_rect.h)
                    && self.selected_index < self.characters.len()
                {
                    self.confirm_delete = true;
                }
                if point_in_rect(
                    mx,
                    my,
                    logout_rect.x,
                    logout_rect.y,
                    logout_rect.w,
                    logout_rect.h,
                ) {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let _ = self.auth_client.logout(&self.session.token);
                    }
                    return ScreenState::ToLogin;
                }
            }
        }

        // Keyboard: navigate (includes the create row when shown)
        let max_index = if self.characters.is_empty() {
            0
        } else {
            self.characters.len() - 1 + if create_selectable { 1 } else { 0 }
        };
        self.selected_index = self.selected_index.min(max_index);
        if (is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W)) && self.selected_index > 0 {
            self.selected_index -= 1;
        }
        if (is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S))
            && self.selected_index < max_index
        {
            self.selected_index += 1;
        }

        // Keyboard: create
        if is_key_pressed(KeyCode::N) && self.characters.len() < MAX_CHARACTERS {
            {
                self.enter_create();
                return ScreenState::Continue;
            }
        }

        // Keyboard: delete (only a real character)
        if (is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::X))
            && self.selected_index < self.characters.len()
        {
            self.confirm_delete = true;
        }

        // Keyboard: logout
        if is_key_pressed(KeyCode::Escape) {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = self.auth_client.logout(&self.session.token);
            }
            return ScreenState::ToLogin;
        }

        // Keyboard: Enter — play selected character, or trigger create if the create row
        // is focused or the roster is empty.
        if is_key_pressed(KeyCode::Enter) {
            if self.characters.is_empty()
                || (create_selectable && self.selected_index == self.characters.len())
            {
                if self.characters.len() < MAX_CHARACTERS {
                    {
                        self.enter_create();
                        return ScreenState::Continue;
                    }
                }
            } else if self.selected_index < self.characters.len() {
                let character = &self.characters[self.selected_index];
                return ScreenState::StartGame {
                    session: self.session.clone(),
                    character_id: character.id,
                    character_name: character.name.clone(),
                };
            }
        }

        ScreenState::Continue
    }

    fn render(&self) {
        let (sw, sh) = virtual_screen_size();
        let (input_pos, _, _) = get_input_state();
        let (mx, my) = (input_pos.x, input_pos.y);

        // Background
        if self.has_spectator_backdrop {
            draw_rectangle(0.0, 0.0, sw, sh, Color::from_rgba(15, 15, 25, 160));
        } else {
            self.starfield.draw(sw, sh, 1.0);
        }

        // Natural layouts for each face, plus the interpolated box that morphs
        // between them. te==0 -> pure roster, te==1 -> pure create form.
        let roster_layout =
            CharSelectLayout::compute(sw, sh, card_row_count(self.characters.len()));
        let create_layout = CreateLayout::compute(sw, sh);
        let te = ease(self.anim_t);
        let roster_alpha = 1.0 - te;
        let create_alpha = te;
        let panel = lerp_rect(roster_layout.panel, create_layout.panel, te);

        // Shared bronze frame (drawn once, fully opaque — it's the box itself).
        draw_panel_frame(panel.x, panel.y, panel.w, panel.h);
        draw_corner_accents(panel.x, panel.y, panel.w, panel.h);

        // Crossfaded titles above the panel.
        let header_y = panel.y - 8.0;
        if roster_alpha > 0.001 {
            let title = "SELECT CHARACTER";
            let tw = self.measure_text_sharp(title, 16.0).width;
            self.draw_text_sharp(
                title,
                ((sw - tw) / 2.0).floor(),
                header_y,
                16.0,
                fade(TEXT_TITLE, roster_alpha),
            );
        }
        if create_alpha > 0.001 {
            let title = "CREATE CHARACTER";
            let tw = self.measure_text_sharp(title, 16.0).width;
            self.draw_text_sharp(
                title,
                ((sw - tw) / 2.0).floor(),
                header_y,
                16.0,
                fade(TEXT_TITLE, create_alpha),
            );
        }

        // Roster + create contents, each glued to the (resizing) shared panel.
        if roster_alpha > 0.001 {
            self.render_roster(&CharSelectLayout::from_panel(panel), roster_alpha);
        }
        if create_alpha > 0.001 {
            self.create_form.render(
                &self.font,
                &self.player_sprites,
                &self.hair_sprites,
                &CreateLayout::from_panel(panel),
                create_alpha,
            );
        }

        // ---- Delete confirmation dialog (bronze reskin; KEEP geometry) ----
        if self.confirm_delete {
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.7));

            let box_w = 450.0_f32.min(sw - 20.0);
            let box_h = 150.0;
            let box_x = (sw - box_w) / 2.0;
            let box_y = (sh - box_h) / 2.0;

            draw_panel_frame(box_x, box_y, box_w, box_h);
            draw_corner_accents(box_x, box_y, box_w, box_h);

            if self.selected_index < self.characters.len() {
                let char_name = &self.characters[self.selected_index].name;
                let delete_text = format!("Delete '{}'?", char_name);
                let delete_width = self.measure_text_sharp(&delete_text, 16.0).width;
                self.draw_text_sharp(
                    &delete_text,
                    (box_x + (box_w - delete_width) / 2.0).floor(),
                    box_y + 50.0,
                    16.0,
                    TEXT_TITLE,
                );
            }

            // Yes/No buttons — geometry must match update's hit-rects exactly.
            let yes_x = box_x + 70.0;
            let yes_y = box_y + 85.0;
            let no_x = box_x + 250.0;
            let yes_hovered = point_in_rect(mx, my, yes_x, yes_y, 100.0, 30.0);
            let no_hovered = point_in_rect(mx, my, no_x, yes_y, 100.0, 30.0);
            draw_screen_button(
                &self.font,
                Rect::new(yes_x, yes_y, 100.0, 30.0),
                "Yes, delete",
                yes_hovered,
                ButtonVariant::Danger,
            );
            draw_screen_button(
                &self.font,
                Rect::new(no_x, yes_y, 100.0, 30.0),
                "No, cancel",
                no_hovered,
                ButtonVariant::Neutral,
            );
            #[allow(clippy::needless_return)]
            // explicit guard: nothing should draw after the dialog
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn played_time_formats() {
        assert_eq!(format_played_time(0), "0m");
        assert_eq!(format_played_time(59), "0m");
        assert_eq!(format_played_time(60), "1m");
        assert_eq!(format_played_time(9 * 3600 + 51 * 60), "9h 51m");
        assert_eq!(format_played_time(149 * 3600 + 44 * 60), "149h 44m");
        assert_eq!(format_played_time(3600), "1h 0m");
    }

    #[test]
    fn level_chip_ramp_endpoints_and_clamp() {
        let low = level_chip_color(1);
        assert!((low.r - 0.322).abs() < 0.01 && (low.g - 0.243).abs() < 0.01);
        let high = level_chip_color(100);
        assert!((high.r - 0.855).abs() < 0.01 && (high.g - 0.698).abs() < 0.01);
        let clamped_low = level_chip_color(0);
        assert!((clamped_low.r - low.r).abs() < 0.001);
        let clamped_high = level_chip_color(126);
        assert!((clamped_high.r - high.r).abs() < 0.001);
        let mid = level_chip_color(50);
        assert!(mid.r > low.r && mid.r < high.r);
    }
}

// ============================================================================
// Character Create Screen
// ============================================================================
