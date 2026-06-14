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

// Head equipment (helmet) sprite dimensions (matching renderer)
const HEAD_SPRITE_WIDTH: f32 = 30.0;
const HEAD_SPRITE_HEIGHT: f32 = 34.0;
// Default weapon frame dimensions (per-weapon sizes come from the manifest)
const WEAPON_SPRITE_WIDTH: f32 = 68.0;
const WEAPON_SPRITE_HEIGHT: f32 = 84.0;

/// Composite a helmet (head equipment) with hair using the in-game head+hair
/// shader, for the static idle/down pose. The shader fills the helmet's (8,0,0)
/// marker pixels with hair and discards everything else that's transparent, so
/// hair only peeks through where the artist intended (no spikes poking out).
///
/// The UV-transform math mirrors `render_player` in `render/renderer/player.rs`
/// for `AnimationState::Idle` facing `Direction::Down` at zoom 1.0.
#[allow(clippy::too_many_arguments)]
fn draw_head_hair_composite(
    material: &Material,
    head_tex: &Texture2D,
    head_offset: Option<(f32, f32)>,
    hair_tex: &Texture2D,
    hair_offset: Option<(f32, f32)>,
    hair_color: i32,
    x: f32,
    y: f32,
    tint: Color,
) {
    let (head_atlas_x, head_atlas_y) = head_offset.unwrap_or((0.0, 0.0));
    let (hair_atlas_x, hair_atlas_y) = hair_offset.unwrap_or((0.0, 0.0));

    // UVs are in full-texture space, so use the whole texture dimensions.
    let head_full_w = head_tex.width();
    let head_full_h = head_tex.height();
    let hair_full_w = hair_tex.width();
    let hair_full_h = hair_tex.height();

    // Idle/down frame indices and offsets (front-facing).
    let hair_src_x = (hair_color * 2) as f32 * HAIR_SPRITE_WIDTH; // front hair frame
    let head_src_x = 0.0; // front head frame
    let (hair_pos_x, hair_pos_y) = (-1.0_f32, -3.0_f32); // get_hair_offset(Idle, Down)
    let (head_pos_x, head_pos_y) = (1.0_f32, -7.0_f32); // get_head_offset(Idle, Down)

    // Pixel delta from the head origin to the (centered) hair origin.
    let hair_base_x = (SPRITE_WIDTH - HAIR_SPRITE_WIDTH) / 2.0 + hair_pos_x;
    let delta_x = hair_base_x - head_pos_x;
    let delta_y = hair_pos_y - head_pos_y;

    let head_uv_x = (head_atlas_x + head_src_x) / head_full_w;
    let head_uv_y = head_atlas_y / head_full_h;
    let hair_uv_x = (hair_atlas_x + hair_src_x) / hair_full_w;
    let hair_uv_y = hair_atlas_y / hair_full_h;
    let hair_uv_w = HAIR_SPRITE_WIDTH / hair_full_w;
    let hair_uv_h = HAIR_SPRITE_HEIGHT / hair_full_h;

    let scale_x = head_full_w * hair_uv_w / HAIR_SPRITE_WIDTH;
    let scale_y = head_full_h * hair_uv_h / HAIR_SPRITE_HEIGHT;
    let off_x = hair_uv_x - head_uv_x * scale_x - delta_x * hair_uv_w / HAIR_SPRITE_WIDTH;
    let off_y = hair_uv_y - head_uv_y * scale_y - delta_y * hair_uv_h / HAIR_SPRITE_HEIGHT;

    material.set_texture("HairTexture", hair_tex.clone());
    material.set_uniform("HairUvTransform", [off_x, off_y, scale_x, scale_y]);
    // The shader tints via this uniform (it ignores vertex color), so route the
    // fade alpha through here.
    material.set_uniform("Tint", [tint.r, tint.g, tint.b, tint.a]);
    gl_use_material(material);

    draw_texture_ex(
        head_tex,
        x + head_pos_x,
        y + head_pos_y,
        WHITE,
        DrawTextureParams {
            source: Some(Rect::new(
                head_atlas_x + head_src_x,
                head_atlas_y,
                HEAD_SPRITE_WIDTH,
                HEAD_SPRITE_HEIGHT,
            )),
            ..Default::default()
        },
    );

    gl_use_default_material();
}

/// Draw a character preview sprite at the given position.
/// Renders at native pixel size (no scaling) for crisp pixel art.
#[allow(clippy::too_many_arguments)]
fn draw_character_preview(
    sprites: &SpritesheetStore,
    hair_sprites: &SpritesheetStore,
    equipment_sprites: &SpritesheetStore,
    weapon_sprites: &SpritesheetStore,
    weapon_frame_sizes: &HashMap<String, (f32, f32)>,
    gender: &str,
    skin: &str,
    hair_style: Option<i32>,
    hair_color: i32,
    sprite_body: Option<&str>,
    sprite_back: Option<&str>,
    sprite_feet: Option<&str>,
    sprite_head: Option<&str>,
    sprite_weapon: Option<&str>,
    // Head+hair composite shader. When `Some` and a helmet is equipped, hair is
    // composited into the helmet's marker pixels (matches the in-game renderer).
    head_hair_material: Option<&Material>,
    x: f32,
    y: f32,
    tint: Color,
) {
    let key = format!("{}_{}", gender, skin);
    if let Some((texture, player_offset)) = sprites.get(&key) {
        let (player_atlas_x, player_atlas_y) = player_offset.unwrap_or((0.0, 0.0));

        // 0. Draw weapon under-layer (before everything, like the in-game renderer).
        // Idle/down pose: weapon frame 0, no flip. Offset mirrors get_weapon_offset()
        // for AnimationState::Idle facing Down: base (-17,-3) + state (-7,-6) = (-24,-9).
        if let Some(weapon_id) = sprite_weapon {
            if let Some((weapon_tex, weapon_offset)) = weapon_sprites.get(weapon_id) {
                let (weapon_atlas_x, weapon_atlas_y) = weapon_offset.unwrap_or((0.0, 0.0));
                let (fw, fh) = weapon_frame_sizes
                    .get(weapon_id)
                    .copied()
                    .unwrap_or((WEAPON_SPRITE_WIDTH, WEAPON_SPRITE_HEIGHT));
                draw_texture_ex(
                    weapon_tex,
                    x - 24.0,
                    y - 9.0,
                    tint,
                    DrawTextureParams {
                        // Frame 0 (standing front) is at the atlas origin.
                        source: Some(Rect::new(weapon_atlas_x, weapon_atlas_y, fw, fh)),
                        ..Default::default()
                    },
                );
            }
        }

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
                        tint,
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
            tint,
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
                        tint,
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
                        tint,
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

        // 4. Hair + head equipment (helmet), composited like the in-game renderer.
        //
        // Helmet sprites paint a special marker color (8,0,0) on pixels where hair
        // should peek through; every other transparent pixel shows nothing. Only
        // the head+hair shader knows this, so when a helmet is equipped and the
        // shader is available we composite head+hair through it (hair/spikes
        // outside the marked area are correctly hidden). Otherwise we fall back to
        // drawing hair, then the helmet on top.
        // Resolve helmet + hair textures up front (both are Copy: references plus
        // a Copy offset tuple), so they can be reused across both branches.
        let hair_key = hair_style.map(|s| format!("{}_{}", gender, s));
        let helmet = sprite_head.and_then(|id| equipment_sprites.get(id));
        let hair = hair_key.as_deref().and_then(|k| hair_sprites.get(k));

        if let (Some((head_tex, head_off)), Some((hair_tex, hair_off)), Some(material)) =
            (helmet, hair, head_hair_material)
        {
            // Shader composite path (replaces both the hair and helmet draws).
            draw_head_hair_composite(
                material, head_tex, head_off, hair_tex, hair_off, hair_color, x, y, tint,
            );
        } else {
            // Fallback: draw hair (after body armor so it appears on top)...
            if let Some((hair_tex, hair_off)) = hair {
                let (hair_atlas_x, hair_atlas_y) = hair_off.unwrap_or((0.0, 0.0));
                let hair_src_x = hair_atlas_x + (hair_color * 2) as f32 * HAIR_SPRITE_WIDTH;
                // Center hair on player: (34 - 28) / 2 = 3, then offset -1
                let hair_x = x + (SPRITE_WIDTH - HAIR_SPRITE_WIDTH) / 2.0 - 1.0;
                let hair_y = y - 3.0;
                draw_texture_ex(
                    hair_tex,
                    hair_x,
                    hair_y,
                    tint,
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

            // ...then the helmet on top (idle/down: frame 0, offset (1,-7)).
            if let Some((head_tex, head_off)) = helmet {
                let (head_atlas_x, head_atlas_y) = head_off.unwrap_or((0.0, 0.0));
                draw_texture_ex(
                    head_tex,
                    x + 1.0,
                    y - 7.0,
                    tint,
                    DrawTextureParams {
                        source: Some(Rect::new(
                            head_atlas_x,
                            head_atlas_y,
                            HEAD_SPRITE_WIDTH,
                            HEAD_SPRITE_HEIGHT,
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
                        tint,
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
                        tint,
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
                        tint,
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
            Color {
                a: tint.a,
                ..Color::from_rgba(100, 100, 100, 255)
            },
        );
    }
}

/// Multiply a color's alpha by `a` — the crossfade primitive shared by the
/// character box's roster and create layers during the morph transition.
fn fade(c: Color, a: f32) -> Color {
    Color { a: c.a * a, ..c }
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
