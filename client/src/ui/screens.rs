use macroquad::miniquad::window::show_keyboard;
use macroquad::prelude::*;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;

use crate::audio::AudioManager;
#[cfg(target_arch = "wasm32")]
use crate::auth::AuthResult;
use crate::auth::{AuthClient, AuthSession, CharacterInfo};
use crate::render::{BitmapFont, SpritesheetStore};
use crate::util::{asset_path, virtual_screen_size, SpriteManifest};

// Sprite sheet constants for character preview
const SPRITE_WIDTH: f32 = 34.0;
const SPRITE_HEIGHT: f32 = 78.0;

// Equipment sprite constants (matching renderer)
const BODY_ARMOR_SPRITE_WIDTH: f32 = 34.0;
const BODY_ARMOR_SPRITE_HEIGHT: f32 = 77.0;
const BOOT_SPRITE_WIDTH: f32 = 34.0;
const BOOT_SPRITE_HEIGHT: f32 = 27.0;
const BACK_STATIC_SPRITE_WIDTH: f32 = 50.0;
const BACK_STATIC_SPRITE_HEIGHT: f32 = 63.0;
const OFFHAND_SPRITE_WIDTH: f32 = 38.5;
const OFFHAND_SPRITE_HEIGHT: f32 = 38.0;
const GENDERS: [&str; 2] = ["male", "female"];
const SKINS: [&str; 7] = ["tan", "pale", "brown", "fish", "orc", "panda", "skeleton"];

/// Convert screen coordinates to virtual coordinates (for Android scaling)
fn screen_to_virtual(x: f32, y: f32) -> (f32, f32) {
    let (vw, vh) = virtual_screen_size();
    let screen_w = screen_width();
    let screen_h = screen_height();

    // On desktop, virtual == screen, so this is a no-op
    let vx = x * vw / screen_w;
    let vy = y * vh / screen_h;
    (vx, vy)
}

/// Get input position and click state from either mouse or touch
/// Returns (position, just_clicked, is_touching)
fn get_input_state() -> (Vec2, bool, bool) {
    let touches: Vec<Touch> = touches();

    // Check for touch input first (mobile)
    for touch in &touches {
        if touch.phase == TouchPhase::Started {
            let (vx, vy) = screen_to_virtual(touch.position.x, touch.position.y);
            return (vec2(vx, vy), true, true);
        }
    }

    // Check for any active touch (for position tracking)
    if let Some(touch) = touches.first() {
        let (vx, vy) = screen_to_virtual(touch.position.x, touch.position.y);
        return (vec2(vx, vy), false, true);
    }

    // Fall back to mouse input (desktop)
    let (mx, my) = mouse_position();
    let (vx, vy) = screen_to_virtual(mx, my);
    let clicked = is_mouse_button_pressed(MouseButton::Left);
    (vec2(vx, vy), clicked, false)
}

/// Load all player sprite textures (gender x skin combinations) as individual files
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
async fn load_player_sprites() -> SpritesheetStore {
    let mut sprites = HashMap::new();
    let genders = ["male", "female"];

    for gender in &genders {
        for skin in &SKINS {
            let path = asset_path(&format!(
                "assets/sprites/players/player_{}_{}.png",
                gender, skin
            ));
            if let Ok(texture) = load_texture(&path).await {
                texture.set_filter(FilterMode::Nearest);
                let key = format!("{}_{}", gender, skin);
                sprites.insert(key, texture);
            }
        }
    }

    SpritesheetStore::Individual(sprites)
}

/// Fallback for platforms without the `image` crate (WASM, Android)
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
async fn load_player_sprites() -> SpritesheetStore {
    SpritesheetStore::Individual(HashMap::new())
}

/// Load a spritesheet atlas texture and return the texture + rect mappings
async fn load_spritesheet_atlas(
    atlas_info: &crate::util::SpriteAtlasInfo,
) -> Option<(Texture2D, HashMap<String, Rect>)> {
    let path = asset_path(&format!("assets/{}", atlas_info.file));
    match load_texture(&path).await {
        Ok(tex) => {
            tex.set_filter(FilterMode::Nearest);
            let rects = atlas_info
                .sprites
                .iter()
                .map(|(key, sr)| {
                    (
                        key.clone(),
                        Rect::new(sr.x as f32, sr.y as f32, sr.w as f32, sr.h as f32),
                    )
                })
                .collect();
            log::info!(
                "Loaded spritesheet atlas {} ({}x{}, {} sprites)",
                atlas_info.file,
                tex.width(),
                tex.height(),
                atlas_info.sprites.len()
            );
            Some((tex, rects))
        }
        Err(e) => {
            log::warn!("Failed to load spritesheet atlas {}: {}", path, e);
            None
        }
    }
}

/// Check if a point is inside a rectangle
fn point_in_rect(px: f32, py: f32, rx: f32, ry: f32, rw: f32, rh: f32) -> bool {
    px >= rx && px < rx + rw && py >= ry && py < ry + rh
}

// Hair sprite dimensions (different from player sprites)
const HAIR_SPRITE_WIDTH: f32 = 28.0;
const HAIR_SPRITE_HEIGHT: f32 = 54.0;

/// Draw a character preview sprite at the given position.
/// Renders at native pixel size (no scaling) for crisp pixel art.
fn draw_character_preview(
    sprites: &SpritesheetStore,
    hair_sprites: &SpritesheetStore,
    equipment_sprites: &SpritesheetStore,
    gender: &str,
    skin: &str,
    hair_style: Option<i32>,
    hair_color: i32,
    sprite_body: Option<&str>,
    sprite_back: Option<&str>,
    sprite_feet: Option<&str>,
    x: f32,
    y: f32,
) {
    let key = format!("{}_{}", gender, skin);
    if let Some((texture, player_offset)) = sprites.get(&key) {
        let (player_atlas_x, player_atlas_y) = player_offset.unwrap_or((0.0, 0.0));

        // 1. Draw back items behind player (quiver/cape - frame 1 for "down" direction)
        if let Some(back_id) = sprite_back {
            if let Some((equip_sprite, equip_offset)) = equipment_sprites.get(back_id) {
                let (equip_atlas_x, equip_atlas_y) = equip_offset.unwrap_or((0.0, 0.0));
                let (equip_w, _equip_h) = equipment_sprites
                    .get_dimensions(back_id)
                    .unwrap_or((equip_sprite.width(), equip_sprite.height()));
                let is_offhand = equip_w > 8.0 * BACK_STATIC_SPRITE_WIDTH;
                if !is_offhand {
                    let back_src_x = equip_atlas_x + 1.0 * BACK_STATIC_SPRITE_WIDTH;
                    draw_texture_ex(
                        equip_sprite,
                        x,
                        y - 15.0,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                back_src_x,
                                equip_atlas_y,
                                BACK_STATIC_SPRITE_WIDTH,
                                BACK_STATIC_SPRITE_HEIGHT,
                            )),
                            flip_x: true,
                            ..Default::default()
                        },
                    );
                }
            }
        }

        // 2. Draw base character sprite (idle frame 0,0)
        draw_texture_ex(
            texture,
            x,
            y,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(
                    player_atlas_x,
                    player_atlas_y,
                    SPRITE_WIDTH,
                    SPRITE_HEIGHT,
                )),
                ..Default::default()
            },
        );

        // 3. Draw body armor (frame 0 for idle/down, offset y=-3)
        if let Some(body_id) = sprite_body {
            if let Some((equip_sprite, equip_offset)) = equipment_sprites.get(body_id) {
                let (equip_atlas_x, equip_atlas_y) = equip_offset.unwrap_or((0.0, 0.0));
                let (equip_w, equip_h) = equipment_sprites
                    .get_dimensions(body_id)
                    .unwrap_or((equip_sprite.width(), equip_sprite.height()));
                let is_single_row = equip_w > equip_h * 2.0;
                if is_single_row {
                    draw_texture_ex(
                        equip_sprite,
                        x,
                        y - 3.0,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                equip_atlas_x,
                                equip_atlas_y,
                                BODY_ARMOR_SPRITE_WIDTH,
                                BODY_ARMOR_SPRITE_HEIGHT,
                            )),
                            ..Default::default()
                        },
                    );
                } else {
                    // Old grid-style format - same layout as player sprite
                    draw_texture_ex(
                        equip_sprite,
                        x,
                        y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                equip_atlas_x,
                                equip_atlas_y,
                                SPRITE_WIDTH,
                                SPRITE_HEIGHT,
                            )),
                            ..Default::default()
                        },
                    );
                }
            }
        }

        // 4. Draw hair (after body armor so it appears on top)
        if let Some(style) = hair_style {
            let hair_key = format!("{}_{}", gender, style);
            if let Some((hair_tex, hair_offset)) = hair_sprites.get(&hair_key) {
                let (hair_atlas_x, hair_atlas_y) = hair_offset.unwrap_or((0.0, 0.0));
                let hair_frame_index = hair_color * 2; // front frame
                let hair_src_x = hair_atlas_x + hair_frame_index as f32 * HAIR_SPRITE_WIDTH;
                // Center hair on player: (34 - 28) / 2 = 3, then offset -1
                let hair_x = x + (SPRITE_WIDTH - HAIR_SPRITE_WIDTH) / 2.0 - 1.0;
                let hair_y = y - 3.0;

                draw_texture_ex(
                    hair_tex,
                    hair_x,
                    hair_y,
                    WHITE,
                    DrawTextureParams {
                        source: Some(Rect::new(
                            hair_src_x,
                            hair_atlas_y,
                            HAIR_SPRITE_WIDTH,
                            HAIR_SPRITE_HEIGHT,
                        )),
                        ..Default::default()
                    },
                );
            }
        }

        // 5. Draw boots (frame 0 for idle/down)
        if let Some(feet_id) = sprite_feet {
            if let Some((equip_sprite, equip_offset)) = equipment_sprites.get(feet_id) {
                let (equip_atlas_x, equip_atlas_y) = equip_offset.unwrap_or((0.0, 0.0));
                let (equip_w, equip_h) = equipment_sprites
                    .get_dimensions(feet_id)
                    .unwrap_or((equip_sprite.width(), equip_sprite.height()));
                let is_single_row = equip_w > equip_h;
                if is_single_row {
                    draw_texture_ex(
                        equip_sprite,
                        x,
                        y + 46.0,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                equip_atlas_x,
                                equip_atlas_y,
                                BOOT_SPRITE_WIDTH,
                                BOOT_SPRITE_HEIGHT,
                            )),
                            ..Default::default()
                        },
                    );
                } else {
                    draw_texture_ex(
                        equip_sprite,
                        x,
                        y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                equip_atlas_x,
                                equip_atlas_y,
                                SPRITE_WIDTH,
                                SPRITE_HEIGHT,
                            )),
                            ..Default::default()
                        },
                    );
                }
            }
        }

        // 6. Draw back items in front of player (offhand/shield - frame 0 for idle/down)
        if let Some(back_id) = sprite_back {
            if let Some((equip_sprite, equip_offset)) = equipment_sprites.get(back_id) {
                let (equip_atlas_x, equip_atlas_y) = equip_offset.unwrap_or((0.0, 0.0));
                let (equip_w, _equip_h) = equipment_sprites
                    .get_dimensions(back_id)
                    .unwrap_or((equip_sprite.width(), equip_sprite.height()));
                let is_offhand = equip_w > 8.0 * BACK_STATIC_SPRITE_WIDTH;
                if is_offhand {
                    draw_texture_ex(
                        equip_sprite,
                        x - 2.0,
                        y + 20.0,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                equip_atlas_x,
                                equip_atlas_y,
                                OFFHAND_SPRITE_WIDTH,
                                OFFHAND_SPRITE_HEIGHT,
                            )),
                            ..Default::default()
                        },
                    );
                }
            }
        }
    } else {
        draw_rectangle(
            x,
            y,
            SPRITE_WIDTH,
            SPRITE_HEIGHT,
            Color::from_rgba(100, 100, 100, 255),
        );
    }
}

/// Result of screen update - tells main loop what to do next
pub enum ScreenState {
    /// Stay on current screen
    Continue,
    /// Move to character select with auth session
    ToCharacterSelect(AuthSession),
    /// Move to character creation screen
    ToCharacterCreate(AuthSession),
    /// Start the game with the selected character
    StartGame {
        session: AuthSession,
        character_id: i64,
        character_name: String,
    },
    /// Guest mode (dev only)
    StartGuestMode,
    /// Go back to login
    ToLogin,
}

pub trait Screen {
    fn update(&mut self, audio: &AudioManager) -> ScreenState;
    fn render(&self);
}

// ============================================================================
// Login Screen
// ============================================================================

mod character_create;
mod character_select;
mod login;
mod starfield;
pub use starfield::StarfieldBackground;

pub use character_create::CharacterCreateScreen;
pub use character_select::CharacterSelectScreen;
pub use login::LoginScreen;
