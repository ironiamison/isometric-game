use macroquad::prelude::*;
use macroquad::models::{Mesh, Vertex, draw_mesh};
use macroquad::material::{load_material, gl_use_material, gl_use_default_material, Material, MaterialParams};
use macroquad::miniquad::UniformDesc;
use macroquad::miniquad::ShaderSource;
use std::collections::HashMap;
use crate::util::{asset_path, SpriteManifest, SpriteAtlasInfo, virtual_screen_size};
use crate::game::{GameState, Player, Camera, ConnectionStatus, LayerType, GroundItem, ChunkLayerType, CHUNK_SIZE, MapObject, ChatChannel, Direction, DragSource, Wall, WallEdge};
use crate::game::tree_types::get_tree_info;
use crate::game::npc::{Npc, NpcState};
use crate::game::tilemap::get_tile_color;
use crate::ui::{UiElementId, UiLayout};
use super::ui::common::{SlotState, CORNER_ACCENT_SIZE, EXP_BAR_GAP};
use super::isometric::{world_to_screen, world_to_screen_exact, screen_to_world, TILE_WIDTH, TILE_HEIGHT, calculate_depth};
use super::animation::{SPRITE_WIDTH, SPRITE_HEIGHT, WEAPON_SPRITE_WIDTH, WEAPON_SPRITE_HEIGHT, BOOT_SPRITE_WIDTH, BOOT_SPRITE_HEIGHT, BODY_ARMOR_SPRITE_WIDTH, BODY_ARMOR_SPRITE_HEIGHT, HEAD_SPRITE_WIDTH, HEAD_SPRITE_HEIGHT, BACK_STATIC_SPRITE_WIDTH, BACK_STATIC_SPRITE_HEIGHT, OFFHAND_SPRITE_WIDTH, OFFHAND_SPRITE_HEIGHT, HAIR_SPRITE_WIDTH, HAIR_SPRITE_HEIGHT, NpcAnimation, get_weapon_frame, get_weapon_offset, get_boot_frame, get_boot_offset, get_body_armor_frame, get_body_armor_offset, get_head_frame, get_head_offset, get_back_static_frame, get_back_static_offset, get_offhand_frame, get_offhand_offset, get_hair_offset, AnimationState, Gender};
use super::font::BitmapFont;
use super::shaders;

/// Format a u32 into a stack-allocated buffer, returning a &str.
/// Avoids heap allocation from .to_string() in hot paths.
fn u32_to_str(n: u32, buf: &mut [u8; 12]) -> &str {
    use std::io::Write;
    let mut cursor = std::io::Cursor::new(&mut buf[..]);
    write!(cursor, "{}", n).unwrap();
    let len = cursor.position() as usize;
    std::str::from_utf8(&buf[..len]).unwrap()
}

/// Timing data from a render pass
#[derive(Default, Clone)]
pub struct RenderTimings {
    pub ground_ms: f64,
    pub entities_ms: f64,
    pub overhead_ms: f64,
    pub effects_ms: f64,
    pub ui_ms: f64,
    pub total_ms: f64,
}

/// Tileset configuration
const TILESET_TILE_WIDTH: f32 = 64.0;
const TILESET_TILE_HEIGHT: f32 = 32.0;
const TILESET_COLUMNS: u32 = 32;

/// Available player appearance options
pub const GENDERS: &[&str] = &["male", "female"];
pub const SKINS: &[&str] = &["tan", "pale", "brown", "fish", "orc", "panda", "skeleton"];

/// Objects tileset firstgid from objects.tsx (used to map gids to sprite filenames)
const OBJECTS_FIRSTGID: u32 = 1249;
/// Offset to convert local tile id to sprite filename number
const OBJECTS_ID_OFFSET: u32 = 87;

/// Walls tileset firstgid (walls use gid = firstGid + fileId, where firstGid=1)
const WALLS_FIRSTGID: u32 = 1;
/// Offset to convert wall gid to sprite filename (file IDs start at 101)
const WALLS_ID_OFFSET: u32 = 101;

// ============================================================================
// Inventory UI Color Palette - Medieval Fantasy Theme
// ============================================================================

// Panel backgrounds (darker to lighter for depth)
const PANEL_BG_DARK: Color = Color::new(0.071, 0.071, 0.094, 0.961);    // rgba(18, 18, 24, 245)
const PANEL_BG_MID: Color = Color::new(0.110, 0.110, 0.149, 1.0);       // rgba(28, 28, 38, 255)

// Frame/Border colors (bronze/gold medieval theme)
const FRAME_OUTER: Color = Color::new(0.322, 0.243, 0.165, 1.0);        // rgba(82, 62, 42, 255)
const FRAME_MID: Color = Color::new(0.557, 0.424, 0.267, 1.0);          // rgba(142, 108, 68, 255)
const FRAME_INNER: Color = Color::new(0.729, 0.580, 0.361, 1.0);        // rgba(186, 148, 92, 255)
const FRAME_ACCENT: Color = Color::new(0.855, 0.698, 0.424, 1.0);       // rgba(218, 178, 108, 255)

// Slot colors
const SLOT_BG_EMPTY: Color = Color::new(0.086, 0.086, 0.118, 1.0);      // rgba(22, 22, 30, 255)
const SLOT_BG_FILLED: Color = Color::new(0.125, 0.125, 0.173, 1.0);     // rgba(32, 32, 44, 255)
const SLOT_INNER_SHADOW: Color = Color::new(0.047, 0.047, 0.063, 1.0);  // rgba(12, 12, 16, 255)
const SLOT_HIGHLIGHT: Color = Color::new(0.188, 0.188, 0.251, 1.0);     // rgba(48, 48, 64, 255)
const SLOT_BORDER: Color = Color::new(0.227, 0.212, 0.188, 1.0);        // rgba(58, 54, 48, 255)

// Hover/Selection states
const SLOT_HOVER_BG: Color = Color::new(0.188, 0.188, 0.282, 1.0);      // rgba(48, 48, 72, 255)
const SLOT_HOVER_BORDER: Color = Color::new(0.659, 0.580, 0.424, 1.0);  // rgba(168, 148, 108, 255)
const SLOT_SELECTED_BORDER: Color = Color::new(0.855, 0.737, 0.502, 1.0); // rgba(218, 188, 128, 255)
const SLOT_DRAG_SOURCE: Color = Color::new(0.314, 0.392, 0.627, 0.706); // rgba(80, 100, 160, 180)

// Text colors (used by stats panel)
const TEXT_TITLE: Color = Color::new(0.855, 0.737, 0.502, 1.0);         // rgba(218, 188, 128, 255)
const TEXT_NORMAL: Color = Color::new(0.824, 0.824, 0.855, 1.0);        // rgba(210, 210, 218, 255)
const TEXT_DIM: Color = Color::new(0.502, 0.502, 0.541, 1.0);           // rgba(128, 128, 138, 255)

// Layout constant for draw_panel_frame helper
const FRAME_THICKNESS: f32 = 4.0;

// ============================================================================
// Health Bar Colors - Ornate Medieval Style
// ============================================================================

// Health bar frame (bronze-tinted dark metal)
const HEALTHBAR_FRAME_DARK: Color = Color::new(0.18, 0.14, 0.10, 1.0);   // Dark bronze outline
const HEALTHBAR_FRAME_MID: Color = Color::new(0.35, 0.27, 0.18, 1.0);    // Mid bronze
const HEALTHBAR_FRAME_LIGHT: Color = Color::new(0.55, 0.43, 0.28, 1.0);  // Light bronze
const HEALTHBAR_FRAME_ACCENT: Color = Color::new(0.72, 0.58, 0.38, 1.0); // Gold highlight

// Health bar background (recessed dark)
const HEALTHBAR_BG_OUTER: Color = Color::new(0.04, 0.04, 0.05, 1.0);     // Outer shadow
const HEALTHBAR_BG_INNER: Color = Color::new(0.08, 0.07, 0.09, 1.0);     // Inner dark

// Health colors - rich jewel tones (dark/mid/light for gradient effect)
const HEALTH_GREEN_DARK: Color = Color::new(0.12, 0.45, 0.22, 1.0);      // Emerald base
const HEALTH_GREEN_MID: Color = Color::new(0.20, 0.62, 0.32, 1.0);       // Emerald bright
const HEALTH_GREEN_LIGHT: Color = Color::new(0.35, 0.78, 0.48, 1.0);     // Emerald highlight

const HEALTH_YELLOW_DARK: Color = Color::new(0.65, 0.45, 0.08, 1.0);     // Amber base
const HEALTH_YELLOW_MID: Color = Color::new(0.85, 0.62, 0.12, 1.0);      // Amber bright
const HEALTH_YELLOW_LIGHT: Color = Color::new(0.95, 0.78, 0.25, 1.0);    // Amber highlight

const HEALTH_RED_DARK: Color = Color::new(0.55, 0.12, 0.12, 1.0);        // Ruby base
const HEALTH_RED_MID: Color = Color::new(0.75, 0.18, 0.18, 1.0);         // Ruby bright
const HEALTH_RED_LIGHT: Color = Color::new(0.90, 0.35, 0.35, 1.0);       // Ruby highlight

/// A sprite atlas: one texture containing many sprites, with rect lookups
pub struct SpriteAtlas {
    pub texture: Texture2D,
    pub rects: HashMap<String, Rect>,
}

impl SpriteAtlas {
    /// Look up a sprite by key, returning the atlas texture and source rect
    pub fn get(&self, key: &str) -> Option<(&Texture2D, Rect)> {
        self.rects.get(key).map(|r| (&self.texture, *r))
    }
}

/// Sprite storage: either an atlas (WASM) or individual textures (desktop)
pub enum SpriteStore {
    Atlas(SpriteAtlas),
    Individual(HashMap<String, Texture2D>),
}

impl SpriteStore {
    /// Look up a sprite, returning (texture, source_rect).
    /// For individual textures, source_rect covers the whole texture.
    pub fn get(&self, key: &str) -> Option<(&Texture2D, Option<Rect>)> {
        match self {
            SpriteStore::Atlas(atlas) => {
                atlas.get(key).map(|(tex, rect)| (tex, Some(rect)))
            }
            SpriteStore::Individual(map) => {
                map.get(key).map(|tex| (tex, None))
            }
        }
    }

    /// Returns the number of sprites in this store.
    pub fn len(&self) -> usize {
        match self {
            SpriteStore::Atlas(atlas) => atlas.rects.len(),
            SpriteStore::Individual(map) => map.len(),
        }
    }
}

pub struct WeaponSprite {
    pub texture: Texture2D,
    pub frame_width: f32,
    pub frame_height: f32,
}

/// Storage for spritesheet atlases: each "sprite" is itself a spritesheet (animation strip).
/// When retrieving a sprite, we get the texture and the position within the atlas where the
/// full spritesheet is located. Animation frame calculation happens on top of this offset.
#[derive(Clone)]
pub enum SpritesheetStore {
    /// Atlas mode: one texture contains multiple spritesheets, each identified by key
    Atlas {
        texture: Texture2D,
        /// Maps sprite key -> rect in atlas where the full spritesheet is located
        rects: HashMap<String, Rect>,
    },
    /// Individual mode: each spritesheet is a separate texture
    Individual(HashMap<String, Texture2D>),
}

impl SpritesheetStore {
    /// Look up a spritesheet by key.
    /// Returns (texture, atlas_offset) where:
    /// - texture: the texture to draw from
    /// - atlas_offset: if Some((x, y)), these values should be added to all source rect coordinates.
    pub fn get(&self, key: &str) -> Option<(&Texture2D, Option<(f32, f32)>)> {
        match self {
            SpritesheetStore::Atlas { texture, rects } => {
                rects.get(key).map(|r| (texture, Some((r.x, r.y))))
            }
            SpritesheetStore::Individual(map) => {
                map.get(key).map(|tex| (tex, None))
            }
        }
    }

    /// Get the spritesheet dimensions for a given key.
    /// For individual textures, returns the texture size.
    /// For atlas, returns the rect dimensions (the spritesheet size within the atlas).
    pub fn get_dimensions(&self, key: &str) -> Option<(f32, f32)> {
        match self {
            SpritesheetStore::Atlas { rects, .. } => {
                rects.get(key).map(|r| (r.w, r.h))
            }
            SpritesheetStore::Individual(map) => {
                map.get(key).map(|tex| (tex.width(), tex.height()))
            }
        }
    }

    /// Check if a key exists in this store.
    pub fn contains(&self, key: &str) -> bool {
        match self {
            SpritesheetStore::Atlas { rects, .. } => rects.contains_key(key),
            SpritesheetStore::Individual(map) => map.contains_key(key),
        }
    }

    /// Get count of sprites in this store.
    pub fn len(&self) -> usize {
        match self {
            SpritesheetStore::Atlas { rects, .. } => rects.len(),
            SpritesheetStore::Individual(map) => map.len(),
        }
    }
}

pub struct Renderer {
    player_color: Color,
    local_player_color: Color,
    /// Loaded tileset texture
    tileset: Option<Texture2D>,
    /// Player sprite sheets by appearance key (e.g., "male_tan")
    player_sprites: SpritesheetStore,
    /// Hair sprite sheets by gender and style (e.g., "male_0", "female_0")
    hair_sprites: SpritesheetStore,
    /// Equipment sprite sheets by item ID (e.g., "peasant_suit")
    equipment_sprites: SpritesheetStore,
    /// Weapon sprite sheets by item ID (e.g., "goblin_axe")
    weapon_sprites: SpritesheetStore,
    /// Per-weapon frame size overrides: { "weapon_id": (width, height) }
    weapon_frame_sizes: HashMap<String, (f32, f32)>,
    /// Item inventory sprites by item ID
    pub(crate) item_sprites: SpriteStore,
    /// Map object sprites by filename number (e.g., "101")
    object_sprites: SpriteStore,
    /// Wall sprites by filename number (e.g., "101")
    wall_sprites: SpriteStore,
    /// NPC sprites by entity type (e.g., "pig" -> Texture2D)
    npc_sprites: SpritesheetStore,
    /// Multi-size pixel font for sharp text rendering at various sizes
    font: BitmapFont,
    /// Quest complete banner texture
    pub(crate) quest_complete_texture: Option<Texture2D>,
    /// Gold nugget icon for inventory
    pub(crate) gold_nugget_texture: Option<Texture2D>,
    /// Circular stone backdrop for shop item icons
    pub(crate) circular_stone_texture: Option<Texture2D>,
    /// Menu button icons (inventory, character, settings, skills, social)
    pub(crate) menu_button_icons: Option<Texture2D>,
    /// UI icons sprite sheet (24x24 icons in 10x10 grid)
    pub(crate) ui_icons: Option<Texture2D>,
    /// Small chat icon for quest giver name tags
    pub(crate) chat_small_icon: Option<Texture2D>,
    /// Fishing skill icon (dedicated 32x32 texture)
    pub(crate) fishing_skill_icon: Option<Texture2D>,
    /// Small coin icon for merchant name tags
    pub(crate) coin_small_icon: Option<Texture2D>,
    /// Farming crop sprite sheets by crop name (e.g., "potato" -> Texture2D)
    farming_sprites: SpritesheetStore,
    /// Prayer icons by prayer id (e.g., "clarity" -> Texture2D or atlas rect)
    pub(crate) prayer_icons: SpriteStore,
    /// Spell icons by spell id (e.g., "dark_hand" -> Texture2D or atlas rect)
    pub(crate) spell_icons: SpriteStore,
    /// Miscellaneous UI icons atlas (quest_complete, arrows, etc.)
    pub(crate) ui_misc_atlas: Option<SpriteAtlas>,
    /// Spell effect sprite sheets by effect name (e.g., "dark_hand" -> Texture2D)
    spell_effect_textures: SpritesheetStore,
    /// Material for head+hair composite rendering (shader-based)
    head_hair_material: Option<Material>,
    /// Material for animated water tile rendering (shader-based)
    water_material: Option<Material>,
    /// Material for wave overlay drawn on top of water tiles
    water_overlay_material: Option<Material>,
    /// Exit portal arrow textures for interior maps
    exit_arrow_up: Option<Texture2D>,
    exit_arrow_down: Option<Texture2D>,
    exit_arrow_left: Option<Texture2D>,
    exit_arrow_right: Option<Texture2D>,
}

impl Renderer {
    /// Update the HTML loading overlay progress (WASM only, no-op on other platforms).
    #[cfg(target_arch = "wasm32")]
    fn update_loading(loaded: usize, total: usize, label: &str) {
        use sapp_jsutils::JsObject;
        extern "C" {
            fn loading_set_progress(pct_times_100: i32);
            fn loading_set_label(label: JsObject);
            fn loading_hide();
        }
        let pct = if total > 0 { (loaded as f64 / total as f64 * 10000.0) as i32 } else { 0 };
        unsafe {
            loading_set_progress(pct);
            loading_set_label(JsObject::string(label));
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn hide_loading() {
        extern "C" {
            fn loading_hide();
        }
        unsafe { loading_hide(); }
    }

    pub async fn new(audio: &mut crate::audio::AudioManager) -> Self {
        // Load manifest first to compute total sprite count
        let manifest = SpriteManifest::load().await;

        // Fixed assets: 1 tileset + 14 players + 3 hair + 1 font + 8 UI textures + 1 shader + 4 arrows + 2 music = 34
        const FIXED_ASSETS: usize = 34;
        let manifest_total = manifest.equipment.len()
            + manifest.weapons.len()
            + manifest.inventory.len()
            + manifest.objects.len()
            + manifest.walls.len()
            + manifest.enemies.len();
        let total = FIXED_ASSETS + manifest_total;
        let mut loaded: usize = 0;

        // On WASM, update the HTML overlay. On other platforms, no-op.
        macro_rules! set_loading {
            ($label:expr) => {
                #[cfg(target_arch = "wasm32")]
                Self::update_loading(loaded, total, $label);
            };
        }

        // Preload audio first (music + SFX)
        set_loading!("Loading audio...");
        audio.preload_all().await;
        loaded += 2; // menu.ogg + start.ogg

        set_loading!("Loading tileset...");

        let tileset = match load_texture(&asset_path("assets/sprites/tiles.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load tileset: {}", e);
                None
            }
        };
        loaded += 1;

        // Load player sprites - atlas on WASM/Android, individual on desktop
        set_loading!("Loading player sprites...");
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let player_sprites: SpritesheetStore = if let Some(ref atlas_info) = manifest.players_atlas {
            if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                SpritesheetStore::Atlas { texture: tex, rects }
            } else {
                let mut sprites = HashMap::new();
                for gender in GENDERS {
                    for skin in SKINS {
                        let key = format!("{}_{}", gender, skin);
                        let path = asset_path(&format!("assets/sprites/players/player_{}_{}.png", gender, skin));
                        if let Ok(tex) = load_texture(&path).await {
                            tex.set_filter(FilterMode::Nearest);
                            sprites.insert(key, tex);
                        }
                    }
                }
                SpritesheetStore::Individual(sprites)
            }
        } else {
            let mut sprites = HashMap::new();
            for gender in GENDERS {
                for skin in SKINS {
                    let key = format!("{}_{}", gender, skin);
                    let path = asset_path(&format!("assets/sprites/players/player_{}_{}.png", gender, skin));
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(key, tex);
                    }
                }
            }
            SpritesheetStore::Individual(sprites)
        };

        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        let player_sprites: SpritesheetStore = {
            let mut sprites = HashMap::new();
            for gender in GENDERS {
                for skin in SKINS {
                    let key = format!("{}_{}", gender, skin);
                    let path = format!("assets/sprites/players/player_{}_{}.png", gender, skin);
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(key, tex);
                    }
                }
            }
            SpritesheetStore::Individual(sprites)
        };
        log::info!("Loaded {} player sprite variants", player_sprites.len());

        // Load hair sprites - atlas on WASM/Android, individual on desktop
        set_loading!("Loading hair sprites...");
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let hair_sprites: SpritesheetStore = if let Some(ref atlas_info) = manifest.hair_atlas {
            if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                SpritesheetStore::Atlas { texture: tex, rects }
            } else {
                let mut sprites = HashMap::new();
                for style in 0..6 {
                    let path = asset_path(&format!("assets/sprites/hair/hair_{}.png", style));
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(format!("male_{}", style), tex);
                    }
                    let path = asset_path(&format!("assets/sprites/hair/hair_female_{}.png", style));
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(format!("female_{}", style), tex);
                    }
                }
                SpritesheetStore::Individual(sprites)
            }
        } else {
            let mut sprites = HashMap::new();
            for style in 0..6 {
                let path = asset_path(&format!("assets/sprites/hair/hair_{}.png", style));
                if let Ok(tex) = load_texture(&path).await {
                    tex.set_filter(FilterMode::Nearest);
                    sprites.insert(format!("male_{}", style), tex);
                }
                let path = asset_path(&format!("assets/sprites/hair/hair_female_{}.png", style));
                if let Ok(tex) = load_texture(&path).await {
                    tex.set_filter(FilterMode::Nearest);
                    sprites.insert(format!("female_{}", style), tex);
                }
            }
            SpritesheetStore::Individual(sprites)
        };

        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        let hair_sprites: SpritesheetStore = {
            let mut sprites = HashMap::new();
            for style in 0..6 {
                let path = format!("assets/sprites/hair/hair_{}.png", style);
                if let Ok(tex) = load_texture(&path).await {
                    tex.set_filter(FilterMode::Nearest);
                    sprites.insert(format!("male_{}", style), tex);
                }
                let path = format!("assets/sprites/hair/hair_female_{}.png", style);
                if let Ok(tex) = load_texture(&path).await {
                    tex.set_filter(FilterMode::Nearest);
                    sprites.insert(format!("female_{}", style), tex);
                }
            }
            SpritesheetStore::Individual(sprites)
        };
        log::info!("Loaded {} hair sprite variants", hair_sprites.len());

        // Helper to load an atlas texture and build a SpriteStore
        async fn load_atlas(atlas_info: &SpriteAtlasInfo) -> Option<SpriteAtlas> {
            let path = asset_path(&format!("assets/{}", atlas_info.file));
            match load_texture(&path).await {
                Ok(tex) => {
                    tex.set_filter(FilterMode::Nearest);
                    let rects = atlas_info.sprites.iter().map(|(key, sr)| {
                        (key.clone(), Rect::new(sr.x as f32, sr.y as f32, sr.w as f32, sr.h as f32))
                    }).collect();
                    log::info!("Loaded atlas {} ({}x{}, {} sprites)", atlas_info.file, tex.width(), tex.height(), atlas_info.sprites.len());
                    Some(SpriteAtlas { texture: tex, rects })
                }
                Err(e) => {
                    log::warn!("Failed to load atlas {}: {}", path, e);
                    None
                }
            }
        }

        // Helper to load a spritesheet atlas (for animation spritesheets)
        async fn load_spritesheet_atlas(atlas_info: &SpriteAtlasInfo) -> Option<(Texture2D, HashMap<String, Rect>)> {
            let path = asset_path(&format!("assets/{}", atlas_info.file));
            match load_texture(&path).await {
                Ok(tex) => {
                    tex.set_filter(FilterMode::Nearest);
                    let rects = atlas_info.sprites.iter().map(|(key, sr)| {
                        (key.clone(), Rect::new(sr.x as f32, sr.y as f32, sr.w as f32, sr.h as f32))
                    }).collect();
                    log::info!("Loaded spritesheet atlas {} ({}x{}, {} sprites)", atlas_info.file, tex.width(), tex.height(), atlas_info.sprites.len());
                    Some((tex, rects))
                }
                Err(e) => {
                    log::warn!("Failed to load spritesheet atlas {}: {}", path, e);
                    None
                }
            }
        }

        // Load individual sprites into a HashMap (for non-atlas categories)
        async fn load_individual_sprites(
            items: &[String],
            base: &str,
            loaded: &mut usize,
            total: usize,
            label: &str,
        ) -> HashMap<String, Texture2D> {
            let mut sprites = HashMap::new();
            for item in items {
                let key = item.rsplit('/').next().unwrap_or(item).to_string();
                let path = asset_path(&format!("{}/{}.png", base, item));
                match load_texture(&path).await {
                    Ok(tex) => {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(key, tex);
                    }
                    Err(e) => {
                        log::warn!("Failed to load sprite {}: {}", path, e);
                    }
                }
                *loaded += 1;
                #[cfg(target_arch = "wasm32")]
                Renderer::update_loading(*loaded, total, label);
            }
            log::info!("Loaded {} sprites for {}", sprites.len(), label);
            sprites
        }

        // On WASM/Android, load sprite categories - use atlases when available.
        // On desktop, use the fast directory-scanning loader.
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let (equipment_sprites, weapon_sprites, weapon_frame_sizes, item_sprites, object_sprites, wall_sprites, npc_sprites, farming_sprites, spell_effect_textures) = {
            // Load equipment - atlas if available
            set_loading!("Loading equipment...");
            let equipment: SpritesheetStore = if let Some(ref atlas_info) = manifest.equipment_atlas {
                if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                    loaded += manifest.equipment.len();
                    #[cfg(target_arch = "wasm32")]
                    Self::update_loading(loaded, total, "Loading equipment...");
                    SpritesheetStore::Atlas { texture: tex, rects }
                } else {
                    SpritesheetStore::Individual(load_individual_sprites(&manifest.equipment, "assets/sprites", &mut loaded, total, "Loading equipment...").await)
                }
            } else {
                SpritesheetStore::Individual(load_individual_sprites(&manifest.equipment, "assets/sprites", &mut loaded, total, "Loading equipment...").await)
            };

            // Load weapons - atlas if available
            set_loading!("Loading weapons...");
            let weapons: SpritesheetStore = if let Some(ref atlas_info) = manifest.weapons_atlas {
                if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                    loaded += manifest.weapons.len();
                    #[cfg(target_arch = "wasm32")]
                    Self::update_loading(loaded, total, "Loading weapons...");
                    SpritesheetStore::Atlas { texture: tex, rects }
                } else {
                    SpritesheetStore::Individual(load_individual_sprites(&manifest.weapons, "assets/sprites/weapons", &mut loaded, total, "Loading weapons...").await)
                }
            } else {
                SpritesheetStore::Individual(load_individual_sprites(&manifest.weapons, "assets/sprites/weapons", &mut loaded, total, "Loading weapons...").await)
            };
            // Build weapon frame sizes map
            let wf_sizes: HashMap<String, (f32, f32)> = manifest.weapon_frame_sizes.iter()
                .map(|(k, v)| (k.clone(), (v[0], v[1])))
                .collect();

            // Load items - atlas if available
            set_loading!("Loading items...");
            let items: SpriteStore = if let Some(ref atlas_info) = manifest.inventory_atlas {
                if let Some(atlas) = load_atlas(atlas_info).await {
                    loaded += manifest.inventory.len();
                    #[cfg(target_arch = "wasm32")]
                    Self::update_loading(loaded, total, "Loading items...");
                    SpriteStore::Atlas(atlas)
                } else {
                    SpriteStore::Individual(load_individual_sprites(&manifest.inventory, "assets/sprites/inventory", &mut loaded, total, "Loading items...").await)
                }
            } else {
                SpriteStore::Individual(load_individual_sprites(&manifest.inventory, "assets/sprites/inventory", &mut loaded, total, "Loading items...").await)
            };

            // Load objects - atlas if available
            set_loading!("Loading objects...");
            let objects: SpriteStore = if let Some(ref atlas_info) = manifest.objects_atlas {
                if let Some(atlas) = load_atlas(atlas_info).await {
                    loaded += manifest.objects.len();
                    #[cfg(target_arch = "wasm32")]
                    Self::update_loading(loaded, total, "Loading objects...");
                    SpriteStore::Atlas(atlas)
                } else {
                    SpriteStore::Individual(load_individual_sprites(&manifest.objects, "assets/sprites/objects", &mut loaded, total, "Loading objects...").await)
                }
            } else {
                SpriteStore::Individual(load_individual_sprites(&manifest.objects, "assets/sprites/objects", &mut loaded, total, "Loading objects...").await)
            };

            // Load walls - atlas if available
            set_loading!("Loading walls...");
            let walls: SpriteStore = if let Some(ref atlas_info) = manifest.walls_atlas {
                if let Some(atlas) = load_atlas(atlas_info).await {
                    loaded += manifest.walls.len();
                    #[cfg(target_arch = "wasm32")]
                    Self::update_loading(loaded, total, "Loading walls...");
                    SpriteStore::Atlas(atlas)
                } else {
                    SpriteStore::Individual(load_individual_sprites(&manifest.walls, "assets/sprites/walls", &mut loaded, total, "Loading walls...").await)
                }
            } else {
                SpriteStore::Individual(load_individual_sprites(&manifest.walls, "assets/sprites/walls", &mut loaded, total, "Loading walls...").await)
            };

            // Load NPCs/enemies - atlas if available
            set_loading!("Loading NPCs...");
            let npcs: SpritesheetStore = if let Some(ref atlas_info) = manifest.enemies_atlas {
                if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                    loaded += manifest.enemies.len();
                    #[cfg(target_arch = "wasm32")]
                    Self::update_loading(loaded, total, "Loading NPCs...");
                    SpritesheetStore::Atlas { texture: tex, rects }
                } else {
                    SpritesheetStore::Individual(load_individual_sprites(&manifest.enemies, "assets/sprites/enemies", &mut loaded, total, "Loading NPCs...").await)
                }
            } else {
                SpritesheetStore::Individual(load_individual_sprites(&manifest.enemies, "assets/sprites/enemies", &mut loaded, total, "Loading NPCs...").await)
            };

            // Load farming sprites - atlas if available
            set_loading!("Loading farming...");
            let farming: SpritesheetStore = if let Some(ref atlas_info) = manifest.farming_atlas {
                if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                    SpritesheetStore::Atlas { texture: tex, rects }
                } else {
                    let crop_names = ["potato", "onion", "tomato", "cabbage", "strawberry", "sweetcorn", "wheat", "carrot", "spinach"];
                    let mut sprites = HashMap::new();
                    for crop in &crop_names {
                        let path = asset_path(&format!("assets/sprites/farming/farming_{}.png", crop));
                        if let Ok(tex) = load_texture(&path).await {
                            tex.set_filter(FilterMode::Nearest);
                            sprites.insert(crop.to_string(), tex);
                        }
                    }
                    SpritesheetStore::Individual(sprites)
                }
            } else {
                let crop_names = ["potato", "onion", "tomato", "cabbage", "strawberry", "sweetcorn", "wheat", "carrot", "spinach"];
                let mut sprites = HashMap::new();
                for crop in &crop_names {
                    let path = asset_path(&format!("assets/sprites/farming/farming_{}.png", crop));
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(crop.to_string(), tex);
                    }
                }
                SpritesheetStore::Individual(sprites)
            };

            // Load spell effects - atlas if available
            set_loading!("Loading effects...");
            let effects: SpritesheetStore = if let Some(ref atlas_info) = manifest.effects_atlas {
                if let Some((tex, rects)) = load_spritesheet_atlas(atlas_info).await {
                    SpritesheetStore::Atlas { texture: tex, rects }
                } else {
                    let mut sprites = HashMap::new();
                    for name in &["dark_hand", "dark_eater", "self_heal", "bubbles_warp"] {
                        let path = asset_path(&format!("assets/sprites/effects/{}.png", name));
                        if let Ok(tex) = load_texture(&path).await {
                            tex.set_filter(FilterMode::Nearest);
                            sprites.insert(name.to_string(), tex);
                        }
                    }
                    SpritesheetStore::Individual(sprites)
                }
            } else {
                let mut sprites = HashMap::new();
                for name in &["dark_hand", "dark_eater", "self_heal", "bubbles_warp"] {
                    let path = asset_path(&format!("assets/sprites/effects/{}.png", name));
                    if let Ok(tex) = load_texture(&path).await {
                        tex.set_filter(FilterMode::Nearest);
                        sprites.insert(name.to_string(), tex);
                    }
                }
                SpritesheetStore::Individual(sprites)
            };

            (equipment, weapons, wf_sizes, items, objects, walls, npcs, farming, effects)
        };

        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        let (equipment_sprites, weapon_sprites, weapon_frame_sizes, item_sprites, object_sprites, wall_sprites, npc_sprites, farming_sprites, spell_effect_textures) = {
            use crate::util::load_sprites_from_dir_or_manifest;

            let equipment = SpritesheetStore::Individual(load_sprites_from_dir_or_manifest("assets/sprites/equipment", &manifest.equipment, "assets/sprites").await);
            let weapons = SpritesheetStore::Individual(load_sprites_from_dir_or_manifest("assets/sprites/weapons", &manifest.weapons, "assets/sprites/weapons").await);
            // Build weapon frame sizes map
            let wf_sizes: HashMap<String, (f32, f32)> = manifest.weapon_frame_sizes.iter()
                .map(|(k, v)| (k.clone(), (v[0], v[1])))
                .collect();
            let items = SpriteStore::Individual(load_sprites_from_dir_or_manifest("assets/sprites/inventory", &manifest.inventory, "assets/sprites/inventory").await);
            let objects = SpriteStore::Individual(load_sprites_from_dir_or_manifest("assets/sprites/objects", &manifest.objects, "assets/sprites/objects").await);
            let walls = SpriteStore::Individual(load_sprites_from_dir_or_manifest("assets/sprites/walls", &manifest.walls, "assets/sprites/walls").await);
            let npcs = SpritesheetStore::Individual(load_sprites_from_dir_or_manifest("assets/sprites/enemies", &manifest.enemies, "assets/sprites/enemies").await);
            let farming = SpritesheetStore::Individual(load_sprites_from_dir_or_manifest("assets/sprites/farming", &[], "assets/sprites/farming").await);
            let effects = SpritesheetStore::Individual(load_sprites_from_dir_or_manifest("assets/sprites/effects", &[], "assets/sprites/effects").await);
            (equipment, weapons, wf_sizes, items, objects, walls, npcs, farming, effects)
        };

        set_loading!("Loading fonts...");

        let font = BitmapFont::load_or_default("assets/fonts/monogram/ttf/monogram-extended.ttf").await;
        loaded += 1;

        set_loading!("Loading UI...");

        // Load quest complete banner texture
        let quest_complete_texture = match load_texture(&asset_path("assets/ui/quest_complete.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                log::info!("Loaded quest complete texture: {}x{}", tex.width(), tex.height());
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load quest complete texture: {}", e);
                None
            }
        };

        // Load gold nugget icon for inventory
        let gold_nugget_texture = match load_texture(&asset_path("assets/ui/gold_nugget.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                log::info!("Loaded gold nugget texture: {}x{}", tex.width(), tex.height());
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load gold nugget texture: {}", e);
                None
            }
        };

        // Load circular stone backdrop for shop item icons
        let circular_stone_texture = match load_texture(&asset_path("assets/ui/circular_stone.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                log::info!("Loaded circular stone texture: {}x{}", tex.width(), tex.height());
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load circular stone texture: {}", e);
                None
            }
        };

        // Load menu button icons sprite sheet
        let menu_button_icons = match load_texture(&asset_path("assets/ui/background_icons.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                log::info!("Loaded menu button icons: {}x{}", tex.width(), tex.height());
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load menu button icons: {}", e);
                None
            }
        };

        // Load UI icons sprite sheet (24x24 icons in 10x10 grid)
        let ui_icons = match load_texture(&asset_path("assets/ui/ui_icons.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                log::info!("Loaded UI icons: {}x{}", tex.width(), tex.height());
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load UI icons: {}", e);
                None
            }
        };

        // Load small icons for NPC name tags
        let chat_small_icon = match load_texture(&asset_path("assets/ui/chat_small.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load chat_small icon: {}", e);
                None
            }
        };

        let fishing_skill_icon = match load_texture(&asset_path("assets/ui/fishing_skill.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load fishing_skill icon: {}", e);
                None
            }
        };

        let coin_small_icon = match load_texture(&asset_path("assets/ui/coin_small.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load coin_small icon: {}", e);
                None
            }
        };

        // Load exit portal arrow textures
        let exit_arrow_up = match load_texture(&asset_path("assets/ui/up_arrow.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load up_arrow icon: {}", e);
                None
            }
        };
        let exit_arrow_down = match load_texture(&asset_path("assets/ui/down_arrow.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load down_arrow icon: {}", e);
                None
            }
        };
        let exit_arrow_left = match load_texture(&asset_path("assets/ui/left_arrow.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load left_arrow icon: {}", e);
                None
            }
        };
        let exit_arrow_right = match load_texture(&asset_path("assets/ui/right_arrow.png")).await {
            Ok(tex) => {
                tex.set_filter(FilterMode::Nearest);
                Some(tex)
            }
            Err(e) => {
                log::warn!("Failed to load right_arrow icon: {}", e);
                None
            }
        };

        // farming_sprites loaded via atlas/manifest in earlier block
        log::info!("Farming sprites loaded: {}", farming_sprites.len());

        // Load prayer icons - atlas on WASM/Android, individual files on desktop
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let prayer_icons: SpriteStore = if let Some(ref atlas_info) = manifest.prayers_atlas {
            if let Some(atlas) = load_atlas(atlas_info).await {
                SpriteStore::Atlas(atlas)
            } else {
                SpriteStore::Individual(HashMap::new())
            }
        } else {
            SpriteStore::Individual(HashMap::new())
        };
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        let prayer_icons: SpriteStore = {
            let prayer_names = [
                "clarity", "thick_skin", "burst_of_strength", "improved_clarity",
                "rock_skin", "superhuman_strength", "resourcefulness", "rapid_heal",
                "steel_skin", "incredible_clarity", "ultimate_strength", "protection",
                "greater_resourcefulness", "greater_protection",
            ];
            let mut icons = HashMap::new();
            for prayer in &prayer_names {
                let path = asset_path(&format!("assets/ui/prayers/{}.png", prayer));
                match load_texture(&path).await {
                    Ok(tex) => {
                        tex.set_filter(FilterMode::Nearest);
                        icons.insert(prayer.to_string(), tex);
                    }
                    Err(_) => {}
                }
            }
            SpriteStore::Individual(icons)
        };
        log::info!("Loaded {} prayer icons", prayer_icons.len());

        // Load spell icons - atlas on WASM/Android, individual files on desktop
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let spell_icons: SpriteStore = if let Some(ref atlas_info) = manifest.spells_atlas {
            if let Some(atlas) = load_atlas(atlas_info).await {
                SpriteStore::Atlas(atlas)
            } else {
                SpriteStore::Individual(HashMap::new())
            }
        } else {
            SpriteStore::Individual(HashMap::new())
        };
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        let spell_icons: SpriteStore = {
            // spell_id -> icon_filename mapping (spell ids don't always match filenames)
            let spell_icon_mappings = [
                ("dark_hand", "dark_hand"),
                ("dark_eater", "dark_eater"),
                ("heal", "self_heal"),
            ];
            let mut icons = HashMap::new();
            for (spell_id, icon_name) in &spell_icon_mappings {
                let path = asset_path(&format!("assets/ui/spells/{}.png", icon_name));
                match load_texture(&path).await {
                    Ok(tex) => {
                        tex.set_filter(FilterMode::Nearest);
                        icons.insert(spell_id.to_string(), tex);
                    }
                    Err(_) => {}
                }
            }
            SpriteStore::Individual(icons)
        };
        log::info!("Loaded {} spell icons", spell_icons.len());

        // Load miscellaneous UI icons atlas (WASM/Android only)
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let ui_misc_atlas: Option<SpriteAtlas> = if let Some(ref atlas_info) = manifest.ui_misc_atlas {
            load_atlas(atlas_info).await
        } else {
            None
        };
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        let ui_misc_atlas: Option<SpriteAtlas> = None;

        // spell_effect_textures loaded via atlas/manifest in earlier block
        log::info!("Spell effect textures loaded: {}", spell_effect_textures.len());

        set_loading!("Loading shaders...");

        // Load head+hair composite shader material
        let head_hair_material = match load_material(
            ShaderSource::Glsl {
                vertex: shaders::HEAD_HAIR_VERTEX,
                fragment: shaders::HEAD_HAIR_FRAGMENT,
            },
            MaterialParams {
                textures: vec!["HairTexture".to_string()],
                uniforms: vec![
                    UniformDesc::new("HairUvTransform", UniformType::Float4),
                    UniformDesc::new("Tint", UniformType::Float4),
                ],
                ..Default::default()
            },
        ) {
            Ok(mat) => {
                log::info!("Loaded head+hair composite shader");
                Some(mat)
            }
            Err(e) => {
                log::warn!("Failed to load head+hair shader: {}. Head equipment will render without hair masking.", e);
                None
            }
        };

        // Load water animation shader material
        let water_material = match load_material(
            ShaderSource::Glsl {
                vertex: shaders::WATER_VERTEX,
                fragment: shaders::WATER_FRAGMENT,
            },
            MaterialParams {
                uniforms: vec![
                    UniformDesc::new("Time", UniformType::Float1),
                ],
                ..Default::default()
            },
        ) {
            Ok(mat) => {
                log::info!("Loaded water animation shader");
                Some(mat)
            }
            Err(e) => {
                log::warn!("Failed to load water shader: {}. Water tiles will render without animation.", e);
                None
            }
        };

        // Load water overlay shader material
        let water_overlay_material = match load_material(
            ShaderSource::Glsl {
                vertex: shaders::WATER_VERTEX,
                fragment: shaders::WATER_OVERLAY_FRAGMENT,
            },
            MaterialParams {
                uniforms: vec![
                    UniformDesc::new("Time", UniformType::Float1),
                    UniformDesc::new("WorldPos", UniformType::Float2),
                ],
                pipeline_params: macroquad::miniquad::PipelineParams {
                    color_blend: Some(macroquad::miniquad::BlendState::new(
                        macroquad::miniquad::Equation::Add,
                        macroquad::miniquad::BlendFactor::Value(macroquad::miniquad::BlendValue::SourceAlpha),
                        macroquad::miniquad::BlendFactor::OneMinusValue(macroquad::miniquad::BlendValue::SourceAlpha),
                    )),
                    ..Default::default()
                },
                ..Default::default()
            },
        ) {
            Ok(mat) => {
                log::info!("Loaded water overlay shader");
                Some(mat)
            }
            Err(e) => {
                log::warn!("Failed to load water overlay shader: {}", e);
                None
            }
        };

        #[cfg(target_arch = "wasm32")]
        Self::hide_loading();

        Self {
            player_color: Color::from_rgba(100, 150, 255, 255),
            local_player_color: Color::from_rgba(100, 255, 150, 255),
            tileset,
            player_sprites,
            hair_sprites,
            equipment_sprites,
            weapon_sprites,
            weapon_frame_sizes,
            item_sprites,
            object_sprites,
            wall_sprites,
            npc_sprites,
            font,
            quest_complete_texture,
            gold_nugget_texture,
            circular_stone_texture,
            menu_button_icons,
            ui_icons,
            fishing_skill_icon,
            chat_small_icon,
            coin_small_icon,
            farming_sprites,
            prayer_icons,
            spell_icons,
            ui_misc_atlas,
            spell_effect_textures,
            head_hair_material,
            water_material,
            water_overlay_material,
            exit_arrow_up,
            exit_arrow_down,
            exit_arrow_left,
            exit_arrow_right,
        }
    }

    /// Get the sprite texture for a given player appearance
    /// Returns (texture, atlas_offset) where atlas_offset is Some((x, y)) if from atlas
    fn get_player_sprite(&self, gender: &str, skin: &str) -> Option<(&Texture2D, Option<(f32, f32)>)> {
        let key = format!("{}_{}", gender, skin);
        self.player_sprites.get(&key)
            // Fallback to male_tan if sprite not found
            .or_else(|| self.player_sprites.get("male_tan"))
    }

    /// Get the sprite texture for a map object by its gid
    fn get_object_sprite(&self, gid: u32) -> Option<(&Texture2D, Option<Rect>)> {
        if gid < OBJECTS_FIRSTGID {
            return None;
        }
        let local_id = gid - OBJECTS_FIRSTGID;
        let sprite_number = local_id + OBJECTS_ID_OFFSET;
        let mut buf = [0u8; 12];
        let key = u32_to_str(sprite_number, &mut buf);
        self.object_sprites.get(key)
    }

    /// Get the sprite texture for a wall by its gid
    /// Wall gid = WALLS_FIRSTGID + file_id, where file_id starts at 101
    fn get_wall_sprite(&self, gid: u32) -> Option<(&Texture2D, Option<Rect>)> {
        if gid < WALLS_FIRSTGID {
            return None;
        }
        let file_id = gid - WALLS_FIRSTGID;
        let mut buf = [0u8; 12];
        let key = u32_to_str(file_id, &mut buf);
        self.wall_sprites.get(key)
    }

    /// Draw text with pixel font for sharp rendering
    /// Uses multi-size bitmap font for crisp text at any size
    pub fn draw_text_sharp(&self, text: &str, x: f32, y: f32, font_size: f32, color: Color) {
        // Round to integer pixels for crisp rendering
        self.font.draw_text(text, x.floor(), y.floor(), font_size, color);
    }

    /// Get reference to player sprites for sharing with UI screens
    pub fn player_sprites(&self) -> &SpritesheetStore {
        &self.player_sprites
    }

    /// Get reference to hair sprites for sharing with UI screens
    pub fn hair_sprites(&self) -> &SpritesheetStore {
        &self.hair_sprites
    }

    /// Get reference to equipment sprites for sharing with UI screens
    pub fn equipment_sprites(&self) -> &SpritesheetStore {
        &self.equipment_sprites
    }

    /// Get reference to font for sharing with UI screens
    pub fn font(&self) -> &BitmapFont {
        &self.font
    }

    /// Measure text with pixel font
    pub(crate) fn measure_text_sharp(&self, text: &str, font_size: f32) -> TextDimensions {
        self.font.measure_text(text, font_size)
    }

    /// Draw text with word wrapping to fit within max_width
    /// Returns the total height used
    pub(crate) fn draw_text_wrapped(&self, text: &str, x: f32, y: f32, font_size: f32, color: Color, max_width: f32, line_height: f32) -> f32 {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut current_line = String::new();
        let mut current_y = y;
        let space_width = self.measure_text_sharp(" ", font_size).width;

        for word in words {
            let word_width = self.measure_text_sharp(word, font_size).width;
            let line_width = if current_line.is_empty() {
                word_width
            } else {
                self.measure_text_sharp(&current_line, font_size).width + space_width + word_width
            };

            if line_width > max_width && !current_line.is_empty() {
                // Draw current line and start new one
                self.draw_text_sharp(&current_line, x, current_y, font_size, color);
                current_y += line_height;
                current_line = word.to_string();
            } else {
                // Add word to current line
                if current_line.is_empty() {
                    current_line = word.to_string();
                } else {
                    current_line.push(' ');
                    current_line.push_str(word);
                }
            }
        }

        // Draw remaining line
        if !current_line.is_empty() {
            self.draw_text_sharp(&current_line, x, current_y, font_size, color);
            current_y += line_height;
        }

        current_y - y
    }

    /// Get the UV rect for a tile ID in the tileset
    /// Tiled uses 1-indexed tile IDs (0 = empty)
    fn get_tile_uv(&self, tile_id: u32) -> Option<Rect> {
        if tile_id == 0 {
            return None;
        }

        let tileset = self.tileset.as_ref()?;
        let id = tile_id - 1; // Convert to 0-indexed

        let col = id % TILESET_COLUMNS;
        let row = id / TILESET_COLUMNS;

        let x = col as f32 * TILESET_TILE_WIDTH;
        let y = row as f32 * TILESET_TILE_HEIGHT;

        Some(Rect::new(
            x / tileset.width(),
            y / tileset.height(),
            TILESET_TILE_WIDTH / tileset.width(),
            TILESET_TILE_HEIGHT / tileset.height(),
        ))
    }

    /// Draw a tile sprite from the tileset
    fn draw_tile_sprite(&self, screen_x: f32, screen_y: f32, tile_id: u32, zoom: f32, world_pos: Option<(f32, f32)>, water_effects: bool) {
        let scaled_width = TILE_WIDTH * zoom;
        let scaled_height = TILE_HEIGHT * zoom;

        // Apply water shader for water tiles (tile ID 418)
        let is_water = tile_id == 418 && water_effects;
        if is_water {
            if let Some(ref mat) = self.water_material {
                mat.set_uniform("Time", get_time() as f32);
                gl_use_material(mat);
            }
        }

        if let (Some(tileset), Some(uv)) = (&self.tileset, self.get_tile_uv(tile_id)) {
            let draw_x = screen_x - scaled_width / 2.0;
            let draw_y = screen_y - scaled_height / 2.0;
            let source = Rect::new(
                uv.x * tileset.width(),
                uv.y * tileset.height(),
                TILESET_TILE_WIDTH,
                TILESET_TILE_HEIGHT,
            );

            draw_texture_ex(
                tileset,
                draw_x,
                draw_y,
                WHITE,
                DrawTextureParams {
                    source: Some(source),
                    dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                    ..Default::default()
                },
            );

            if is_water && self.water_material.is_some() {
                gl_use_default_material();
            }

            // Draw wave overlay on top of water tiles
            if is_water {
                if let (Some(ref mat), Some((wx, wy))) = (&self.water_overlay_material, world_pos) {
                    mat.set_uniform("Time", get_time() as f32);
                    mat.set_uniform("WorldPos", (wx, wy));
                    gl_use_material(mat);

                    draw_texture_ex(
                        tileset,
                        draw_x,
                        draw_y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(source),
                            dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                            ..Default::default()
                        },
                    );

                    gl_use_default_material();
                }
            }
        } else {
            let color = get_tile_color(tile_id);
            self.draw_isometric_tile(screen_x, screen_y, color, zoom);

            if is_water && self.water_material.is_some() {
                gl_use_default_material();
            }
        }
    }

    /// Render a loading screen while the world isn't ready yet
    fn render_loading_screen(&self, state: &GameState) {
        let sw = screen_width();
        let sh = screen_height();

        // Determine status message based on connection state
        let status = if state.connection_status == ConnectionStatus::Disconnected {
            "Connecting"
        } else if state.local_player_id.is_none() {
            "Logging in"
        } else if state.get_local_player().is_none() {
            "Loading character"
        } else {
            "Loading world"
        };

        // Animated dots (cycles every 1s)
        let dot_count = ((get_time() * 3.0) as usize % 4) as usize;
        let dots = &"..."[..dot_count];
        let text = format!("{}{}", status, dots);

        let font_size = 32.0;
        let dims = self.measure_text_sharp(&text, font_size);
        let x = ((sw - dims.width) / 2.0).floor();
        let y = ((sh) / 2.0).floor();

        self.draw_text_sharp(&text, x, y, font_size, Color::from_rgba(200, 200, 200, 255));
    }

    pub fn render(&self, state: &GameState) -> (UiLayout, RenderTimings) {
        let render_start = get_time();
        let mut timings = RenderTimings::default();

        // 1. Render ground layer tiles
        let t0 = get_time();
        self.render_tilemap_layer(state, LayerType::Ground);

        // 1.5. Render hovered tile border if hovering over a tile
        if let Some((tile_x, tile_y)) = state.hovered_tile {
            self.render_tile_hover(tile_x, tile_y, &state.camera);
        }

        // 1.6. Render drop zone highlights when dragging from inventory
        if let Some(ref drag) = state.ui_state.drag_state {
            if matches!(drag.source, DragSource::Inventory(_)) {
                if let Some(player) = state.get_local_player() {
                    let player_x = player.x.round() as i32;
                    let player_y = player.y.round() as i32;

                    // Render the 8 adjacent tiles as drop zones
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            if dx == 0 && dy == 0 {
                                continue; // Skip player's tile
                            }
                            let tile_x = player_x + dx;
                            let tile_y = player_y + dy;

                            // Check if this tile is currently hovered
                            let is_hovered = state.hovered_tile == Some((tile_x, tile_y));
                            self.render_drop_zone(tile_x, tile_y, &state.camera, is_hovered);
                        }
                    }
                }
            }
        }
        // 1.7. Render farming patches
        self.render_farming_patches(state);

        // 1.8. Render gathering marker overlays (fishing spots, etc.)
        self.render_gathering_markers(state);

        // 1.8. Render bonus tile indicators (pulsing glow)
        self.render_bonus_tiles(state);

        timings.ground_ms = (get_time() - t0) * 1000.0;

        // Skip entity/world rendering until world is ready
        let world_ready = state.is_world_ready();

        // 2. Collect renderable items (players + NPCs + items + object tiles + map objects) for depth sorting
        let t1 = get_time();
        #[derive(Clone)]
        enum Renderable<'a> {
            Player(&'a Player, bool),
            Npc(&'a Npc),
            Item(&'a GroundItem),
            Tile { x: u32, y: u32, tile_id: u32 },
            ChunkObject(&'a MapObject),
            ChunkObjectShaking(&'a MapObject, f32), // Object with shake offset
            ChunkWall(&'a Wall),
            TreeTimer { tile_x: i32, tile_y: i32, progress: f32 },
            FallingTree { gid: u32, tile_x: i32, tile_y: i32, angle: f32, alpha: f32, y_offset: f32 },
        }

        // Pre-allocate with estimated capacity to reduce allocations
        let chunk_object_estimate: usize = state.chunk_manager.chunks().values()
            .map(|c| c.objects.len() + c.walls.len())
            .sum();
        let estimated_capacity = state.players.len() + state.npcs.len() + state.ground_items.len() + chunk_object_estimate + 100;
        let mut renderables: Vec<(f32, Renderable)> = Vec::with_capacity(estimated_capacity);

        // Only collect world entities when world is ready
        if !world_ready {
            // Show loading screen instead of empty world
            self.render_loading_screen(state);

            timings.entities_ms = (get_time() - t1) * 1000.0;

            // 8. Render UI (non-interactive elements)
            let t4 = get_time();
            self.render_ui(state);

            // 9. Render interactive UI elements and return layout for hit detection
            let layout = self.render_interactive_ui(state);
            timings.ui_ms = (get_time() - t4) * 1000.0;

            timings.total_ms = (get_time() - render_start) * 1000.0;
            return (layout, timings);
        }

        // Add ground items (render below entities)
        for item in state.ground_items.values() {
            let depth = calculate_depth(item.x, item.y, 0); // Lower layer than entities
            renderables.push((depth, Renderable::Item(item)));
        }

        // Add players
        for player in state.players.values() {
            let is_local = state.local_player_id.as_ref() == Some(&player.id);
            let mut depth = calculate_depth(player.x, player.y, 1);
            // Sitting players render on top of the chair object at the same tile
            if player.animation.state == crate::render::animation::AnimationState::SittingChair {
                depth += 0.5;
            }
            renderables.push((depth, Renderable::Player(player, is_local)));
        }

        // Add NPCs
        for npc in state.npcs.values() {
            let depth = calculate_depth(npc.x, npc.y, 1);
            renderables.push((depth, Renderable::Npc(npc)));
        }

        // Compute visible world-space AABB from screen corners (avoids per-object world_to_screen)
        let (cull_screen_w, cull_screen_h) = virtual_screen_size();
        let corners_world = [
            screen_to_world(0.0, 0.0, &state.camera),
            screen_to_world(cull_screen_w, 0.0, &state.camera),
            screen_to_world(0.0, cull_screen_h, &state.camera),
            screen_to_world(cull_screen_w, cull_screen_h, &state.camera),
        ];
        // Margin in world tiles for tall objects that extend above their tile
        let world_cull_margin = 8.0;
        let vis_min_x = corners_world.iter().map(|c| c.0).fold(f32::MAX, f32::min) - world_cull_margin;
        let vis_max_x = corners_world.iter().map(|c| c.0).fold(f32::MIN, f32::max) + world_cull_margin;
        let vis_min_y = corners_world.iter().map(|c| c.1).fold(f32::MAX, f32::min) - world_cull_margin;
        let vis_max_y = corners_world.iter().map(|c| c.1).fold(f32::MIN, f32::max) + world_cull_margin;

        // Add object layer tiles (trees, rocks, buildings) from static tilemap
        for layer in &state.tilemap.layers {
            if layer.layer_type == LayerType::Objects {
                for y in 0..state.tilemap.height {
                    for x in 0..state.tilemap.width {
                        let wx = x as f32;
                        let wy = y as f32;
                        if wx < vis_min_x || wx > vis_max_x || wy < vis_min_y || wy > vis_max_y {
                            continue;
                        }
                        let idx = (y * state.tilemap.width + x) as usize;
                        let tile_id = layer.tiles.get(idx).copied().unwrap_or(0);
                        if tile_id > 0 {
                            let depth = calculate_depth(wx, wy, 1);
                            renderables.push((depth, Renderable::Tile { x, y, tile_id }));
                        }
                    }
                }
            }
        }

        // Add map objects and walls from loaded chunks with chunk-level pre-culling
        let chunk_size = CHUNK_SIZE as f32;
        for (coord, chunk) in state.chunk_manager.chunks().iter() {
            // Chunk-level AABB check: skip entire chunk if outside visible area
            let chunk_min_x = (coord.x * CHUNK_SIZE as i32) as f32;
            let chunk_min_y = (coord.y * CHUNK_SIZE as i32) as f32;
            let chunk_max_x = chunk_min_x + chunk_size;
            let chunk_max_y = chunk_min_y + chunk_size;
            if chunk_max_x < vis_min_x || chunk_min_x > vis_max_x
                || chunk_max_y < vis_min_y || chunk_min_y > vis_max_y {
                continue;
            }

            for obj in &chunk.objects {
                let wx = obj.tile_x as f32;
                let wy = obj.tile_y as f32;
                if wx < vis_min_x || wx > vis_max_x || wy < vis_min_y || wy > vis_max_y {
                    continue;
                }
                // Skip depleted trees (they're hidden until respawn)
                if state.depleted_trees.contains_key(&(obj.tile_x, obj.tile_y)) {
                    continue;
                }
                // Skip trees that are currently falling (we render them with the fall animation)
                let is_falling = state.falling_trees.iter().any(|ft| ft.x == obj.tile_x && ft.y == obj.tile_y);
                if is_falling {
                    continue;
                }
                let depth = calculate_depth(wx, wy, 1);
                // Check if tree is shaking and apply offset
                if let Some(shake) = state.tree_shake_effects.iter().find(|s| s.x == obj.tile_x && s.y == obj.tile_y) {
                    let offset = shake.get_offset();
                    renderables.push((depth, Renderable::ChunkObjectShaking(obj, offset)));
                } else {
                    renderables.push((depth, Renderable::ChunkObject(obj)));
                }
            }
            for wall in &chunk.walls {
                let wx = wall.tile_x as f32;
                let wy = wall.tile_y as f32;
                if wx < vis_min_x || wx > vis_max_x || wy < vis_min_y || wy > vis_max_y {
                    continue;
                }
                let depth = calculate_depth(wx, wy, 1);
                renderables.push((depth, Renderable::ChunkWall(wall)));
            }
        }

        // Add depleted tree respawn timers (depth-sorted with other objects)
        let current_time = macroquad::time::get_time();
        for ((tile_x, tile_y), info) in &state.depleted_trees {
            let wx = *tile_x as f32;
            let wy = *tile_y as f32;
            if wx < vis_min_x || wx > vis_max_x || wy < vis_min_y || wy > vis_max_y {
                continue;
            }
            let total_duration = info.respawn_at - info.depleted_at;
            if total_duration <= 0.0 {
                continue;
            }
            let elapsed = current_time - info.depleted_at;
            let progress = (elapsed / total_duration).clamp(0.0, 1.0) as f32;
            let depth = calculate_depth(wx, wy, 1);
            renderables.push((depth, Renderable::TreeTimer { tile_x: *tile_x, tile_y: *tile_y, progress }));
        }

        // Add falling trees (trees that were just chopped down)
        for ft in &state.falling_trees {
            let wx = ft.x as f32;
            let wy = ft.y as f32;
            if wx < vis_min_x || wx > vis_max_x || wy < vis_min_y || wy > vis_max_y {
                continue;
            }
            let (angle, alpha, y_offset) = ft.get_transform();
            let depth = calculate_depth(wx, wy, 1);
            renderables.push((depth, Renderable::FallingTree {
                gid: ft.gid,
                tile_x: ft.x,
                tile_y: ft.y,
                angle,
                alpha,
                y_offset,
            }));
        }

        // Sort by depth (painter's algorithm)
        // Must use stable sort: items at the same depth (e.g. walls on tiles
        // with equal x+y) must keep a consistent order to avoid flickering.
        renderables.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // 3. Render sorted entities
        for (_, renderable) in renderables {
            match renderable {
                Renderable::Item(item) => {
                    self.render_ground_item(item, &state.camera, state);
                }
                Renderable::Player(player, is_local) => {
                    let is_selected = state.selected_entity_id.as_ref() == Some(&player.id);
                    let is_hovered = state.hovered_entity_id.as_ref() == Some(&player.id);
                    self.render_player(player, is_local, is_selected, is_hovered, &state.camera);
                    if player.is_gathering {
                        // Delay the line until the cast animation finishes
                        let elapsed = macroquad::time::get_time() - player.gathering_started_at;
                        if elapsed > 0.2 {
                            self.render_fishing_line(player, &state.camera);
                        }
                    }
                }
                Renderable::Npc(npc) => {
                    let is_selected = state.selected_entity_id.as_ref() == Some(&npc.id);
                    let is_hovered = state.hovered_entity_id.as_ref() == Some(&npc.id);
                    self.render_npc(npc, is_selected, is_hovered, &state.camera);
                }
                Renderable::Tile { x, y, tile_id } => {
                    let (screen_x, screen_y) = world_to_screen(x as f32, y as f32, &state.camera);
                    self.draw_isometric_object(screen_x, screen_y, tile_id, state.camera.zoom);
                }
                Renderable::ChunkObject(obj) => {
                    self.render_map_object(obj, &state.camera);
                }
                Renderable::ChunkObjectShaking(obj, offset) => {
                    self.render_map_object_shaking(obj, offset, &state.camera);
                }
                Renderable::ChunkWall(wall) => {
                    self.render_wall(wall, &state.camera);
                }
                Renderable::TreeTimer { tile_x, tile_y, progress } => {
                    self.render_tree_timer(tile_x, tile_y, progress, &state.camera);
                }
                Renderable::FallingTree { gid, tile_x, tile_y, angle, alpha, y_offset } => {
                    self.render_falling_tree(gid, tile_x, tile_y, angle, alpha, y_offset, &state.camera);
                }
            }
        }

        // Render leaf particles (world-space, after depth-sorted objects)
        for leaf in &state.leaf_particles {
            // Convert tile coords to screen coords
            let (screen_x, base_screen_y) = world_to_screen(leaf.tile_x, leaf.tile_y, &state.camera);

            // Offset upward by height (height is in unscaled pixels, apply zoom)
            let screen_y = base_screen_y - leaf.height * state.camera.zoom;

            let alpha = leaf.get_alpha();
            let color = Color::new(leaf.color.r, leaf.color.g, leaf.color.b, leaf.color.a * alpha);
            let size = leaf.size * state.camera.zoom;

            // Draw a simple leaf shape (small rotated diamond)
            let cos_r = leaf.rotation.cos();
            let sin_r = leaf.rotation.sin();

            // Draw as a small diamond/leaf shape
            let hw = size * 0.5;
            let hh = size * 0.8;

            let points = [
                (screen_x + cos_r * 0.0 - sin_r * (-hh), screen_y + sin_r * 0.0 + cos_r * (-hh)), // top
                (screen_x + cos_r * hw - sin_r * 0.0, screen_y + sin_r * hw + cos_r * 0.0),       // right
                (screen_x + cos_r * 0.0 - sin_r * hh, screen_y + sin_r * 0.0 + cos_r * hh),       // bottom
                (screen_x + cos_r * (-hw) - sin_r * 0.0, screen_y + sin_r * (-hw) + cos_r * 0.0), // left
            ];

            // Draw as two triangles
            draw_triangle(
                Vec2::new(points[0].0, points[0].1),
                Vec2::new(points[1].0, points[1].1),
                Vec2::new(points[2].0, points[2].1),
                color,
            );
            draw_triangle(
                Vec2::new(points[0].0, points[0].1),
                Vec2::new(points[2].0, points[2].1),
                Vec2::new(points[3].0, points[3].1),
                color,
            );
        }

        // Render subtle local player silhouette (high z-index, visible through trees)
        if let Some(ref local_id) = state.local_player_id {
            if let Some(local_player) = state.players.get(local_id) {
                self.render_player_silhouette(local_player, &state.camera);
            }
        }

        timings.entities_ms = (get_time() - t1) * 1000.0;

        // 4. Render overhead layer (always on top)
        let t2 = get_time();
        self.render_tilemap_layer(state, LayerType::Overhead);

        // 4.1. Render exit portal arrows on interior maps
        self.render_exit_portal_arrows(state);

        timings.overhead_ms = (get_time() - t2) * 1000.0;

        // 4.5. Render name tags above all map elements (overhead, walls, objects, etc.)
        self.render_name_tags(state);
        self.render_tree_name_tag(state);
        self.render_farming_patch_labels(state);

        // 5. Render floating damage numbers
        let t3 = get_time();
        self.render_damage_numbers(state);

        // 6. Render floating level up text
        self.render_level_up_events(state);
        self.render_firework_particles(state);

        // 7. Render chat bubbles above players
        self.render_chat_bubbles(state);

        // 7.5. Render projectiles
        self.render_projectiles(state);

        // 7.6. Render spell effects (animated sprite sheets)
        self.render_spell_effects(state);
        timings.effects_ms = (get_time() - t3) * 1000.0;

        // 8. Render UI (non-interactive elements)
        let t4 = get_time();
        self.render_ui(state);

        // 9. Render interactive UI elements and return layout for hit detection
        let layout = self.render_interactive_ui(state);
        timings.ui_ms = (get_time() - t4) * 1000.0;

        timings.total_ms = (get_time() - render_start) * 1000.0;
        (layout, timings)
    }

    /// Render a single pie chart timer for a depleted tree (called during depth-sorted rendering)
    fn render_tree_timer(&self, tile_x: i32, tile_y: i32, progress: f32, camera: &Camera) {
        let zoom = camera.zoom;

        // Convert tile position to screen position (center of tile)
        let (screen_x, mut screen_y) = world_to_screen(
            tile_x as f32 + 0.5,
            tile_y as f32 + 0.5,
            camera,
        );
        // Adjust Y to center on tile (world_to_screen gives bottom of tile)
        screen_y -= 16.0 * zoom;

        // Draw pie chart timer (15% more opaque for visibility)
        let radius = 12.0 * zoom;
        let bg_color = Color::new(0.0, 0.0, 0.0, 0.50);
        let fill_color = Color::new(0.2, 0.8, 0.2, 0.60);
        let border_color = Color::new(0.1, 0.4, 0.1, 0.75);

        // Draw background circle
        draw_circle(screen_x, screen_y, radius, bg_color);

        // Draw filled pie slice showing progress
        if progress > 0.0 {
            let segments = 32;
            let start_angle = -std::f32::consts::FRAC_PI_2; // Start from top

            // Draw pie as triangle fan
            for i in 0..segments {
                let t1 = i as f32 / segments as f32;
                let t2 = (i + 1) as f32 / segments as f32;
                let angle1 = start_angle + t1 * progress * std::f32::consts::TAU;
                let angle2 = start_angle + t2 * progress * std::f32::consts::TAU;

                let x1 = screen_x + angle1.cos() * radius;
                let y1 = screen_y + angle1.sin() * radius;
                let x2 = screen_x + angle2.cos() * radius;
                let y2 = screen_y + angle2.sin() * radius;

                draw_triangle(
                    Vec2::new(screen_x, screen_y),
                    Vec2::new(x1, y1),
                    Vec2::new(x2, y2),
                    fill_color,
                );
            }
        }

        // Draw border circle
        draw_circle_lines(screen_x, screen_y, radius, 2.0, border_color);
    }

    fn render_level_up_events(&self, state: &GameState) {
        let current_time = macroquad::time::get_time();
        const DURATION: f32 = 1.2;
        const FONT_SIZE: f32 = 16.0;

        for event in &state.level_up_events {
            let age = (current_time - event.time) as f32;
            if age > DURATION {
                continue;
            }

            let t = age / DURATION;
            let float_offset = (age * 40.0).round();

            let (screen_x, screen_y) = world_to_screen(event.x, event.y, &state.camera);
            let height_offset = (SPRITE_HEIGHT - 8.0) / 2.0;
            let final_y = (screen_y - height_offset - float_offset).round();

            let alpha = if t < 0.5 { 1.0 } else { 1.0 - (t - 0.5) * 2.0 };

            let text = format!("LEVEL UP! ({})", event.new_level);
            let text_dims = self.measure_text_sharp(&text, FONT_SIZE);
            let draw_x = (screen_x - text_dims.width / 2.0).round();

            let outline_color = Color::new(0.0, 0.0, 0.0, alpha * 0.9);
            for &(ox, oy) in &[(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                self.draw_text_sharp(&text, draw_x + ox, final_y + oy, FONT_SIZE, outline_color);
            }

            let base_color = Color::new(1.0, 1.0, 0.0, alpha);
            self.draw_text_sharp(&text, draw_x, final_y, FONT_SIZE, base_color);
        }
    }

    fn render_firework_particles(&self, state: &GameState) {
        let current_time = macroquad::time::get_time();
        let zoom = state.camera.zoom;

        for p in &state.firework_particles {
            let age = (current_time - p.time) as f32;
            let max_age = if p.is_spark { 0.5 } else { 1.0 };
            if age > max_age {
                continue;
            }

            // Use the particle's stored origin (position of the player who leveled up)
            let (origin_x, origin_y) = world_to_screen(p.origin_x, p.origin_y, &state.camera);
            let base_y = origin_y - 10.0;

            // Draw trail
            let trail_len = p.trail.len();
            for (i, &(tx, ty)) in p.trail.iter().enumerate() {
                let t = (i + 1) as f32 / (trail_len + 1) as f32;
                let trail_alpha = (t * 150.0 * (1.0 - age / max_age)) as u8;
                let trail_size = p.size * t * 0.6 * zoom;
                let c = Color::from_rgba(p.color.0, p.color.1, p.color.2, trail_alpha);
                draw_circle(origin_x + tx, base_y + ty, trail_size, c);
            }

            // Draw head
            let alpha = ((1.0 - age / max_age) * 255.0) as u8;
            let color = Color::from_rgba(p.color.0, p.color.1, p.color.2, alpha);
            draw_circle(origin_x + p.ox, base_y + p.oy, p.size * zoom, color);
        }
    }

    /// Create a mesh for a rounded rectangle with optional tail (no overlapping geometry)
    fn create_rounded_rect_mesh(x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) -> Mesh {
        Self::create_bubble_mesh(x, y, w, h, r, color, None)
    }

    /// Create a mesh for a chat bubble with tail (no overlapping geometry)
    fn create_bubble_mesh(x: f32, y: f32, w: f32, h: f32, r: f32, color: Color, tail: Option<(f32, f32, f32)>) -> Mesh {
        let color_arr = [
            (color.r * 255.0) as u8,
            (color.g * 255.0) as u8,
            (color.b * 255.0) as u8,
            (color.a * 255.0) as u8,
        ];

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Helper to add a vertex
        let mut add_vertex = |px: f32, py: f32| -> u16 {
            let idx = vertices.len() as u16;
            vertices.push(Vertex {
                position: Vec3::new(px, py, 0.0),
                uv: Vec2::ZERO,
                color: color_arr,
                normal: Vec4::ZERO,
            });
            idx
        };

        // Corner circle segment count
        let segments = 8;

        // Center rectangle vertices (4 corners where the rounded corners meet)
        let c_tl = add_vertex(x + r, y + r);     // top-left inner corner
        let c_tr = add_vertex(x + w - r, y + r); // top-right inner corner
        let c_bl = add_vertex(x + r, y + h - r); // bottom-left inner corner
        let c_br = add_vertex(x + w - r, y + h - r); // bottom-right inner corner

        // Center rectangle (2 triangles)
        indices.extend_from_slice(&[c_tl, c_tr, c_br, c_tl, c_br, c_bl]);

        // Top edge strip
        let t_tl = add_vertex(x + r, y);
        let t_tr = add_vertex(x + w - r, y);
        indices.extend_from_slice(&[t_tl, t_tr, c_tr, t_tl, c_tr, c_tl]);

        // Bottom edge strip
        let b_bl = add_vertex(x + r, y + h);
        let b_br = add_vertex(x + w - r, y + h);
        indices.extend_from_slice(&[c_bl, c_br, b_br, c_bl, b_br, b_bl]);

        // Left edge strip
        let l_tl = add_vertex(x, y + r);
        let l_bl = add_vertex(x, y + h - r);
        indices.extend_from_slice(&[l_tl, c_tl, c_bl, l_tl, c_bl, l_bl]);

        // Right edge strip
        let r_tr = add_vertex(x + w, y + r);
        let r_br = add_vertex(x + w, y + h - r);
        indices.extend_from_slice(&[c_tr, r_tr, r_br, c_tr, r_br, c_br]);

        // Corner fans (quarter circles)
        use std::f32::consts::PI;

        // Top-left corner (180° to 270°)
        let mut prev = l_tl; // starts at left edge
        for i in 1..=segments {
            let angle = PI + (PI / 2.0) * (i as f32 / segments as f32);
            let px = x + r + r * angle.cos();
            let py = y + r + r * angle.sin();
            let curr = add_vertex(px, py);
            indices.extend_from_slice(&[c_tl, prev, curr]);
            prev = curr;
        }

        // Top-right corner (270° to 360°)
        prev = t_tr; // starts at top edge
        for i in 1..=segments {
            let angle = PI * 1.5 + (PI / 2.0) * (i as f32 / segments as f32);
            let px = x + w - r + r * angle.cos();
            let py = y + r + r * angle.sin();
            let curr = add_vertex(px, py);
            indices.extend_from_slice(&[c_tr, prev, curr]);
            prev = curr;
        }

        // Bottom-right corner (0° to 90°)
        prev = r_br; // starts at right edge
        for i in 1..=segments {
            let angle = (PI / 2.0) * (i as f32 / segments as f32);
            let px = x + w - r + r * angle.cos();
            let py = y + h - r + r * angle.sin();
            let curr = add_vertex(px, py);
            indices.extend_from_slice(&[c_br, prev, curr]);
            prev = curr;
        }

        // Bottom-left corner (90° to 180°)
        prev = b_bl; // starts at bottom edge
        for i in 1..=segments {
            let angle = PI / 2.0 + (PI / 2.0) * (i as f32 / segments as f32);
            let px = x + r + r * angle.cos();
            let py = y + h - r + r * angle.sin();
            let curr = add_vertex(px, py);
            indices.extend_from_slice(&[c_bl, prev, curr]);
            prev = curr;
        }

        Mesh {
            vertices,
            indices,
            texture: None,
        }
    }

    /// Render name tags for all hovered/selected players and NPCs.
    /// Called after overhead tiles so names always appear above all map elements.
    fn render_name_tags(&self, state: &GameState) {
        // Player name tags
        for player in state.players.values() {
            let is_selected = state.selected_entity_id.as_ref() == Some(&player.id);
            let is_hovered = state.hovered_entity_id.as_ref() == Some(&player.id);
            if !is_selected && !is_hovered {
                continue;
            }

            let (screen_x, screen_y) = world_to_screen(player.x, player.y, &state.camera);
            let zoom = state.camera.zoom;
            let font_size = 16.0 * zoom;
            let scaled_sprite_height = SPRITE_HEIGHT * zoom;
            let has_sprite = self.get_player_sprite(&player.gender, &player.skin).is_some();
            let name_y_offset = if has_sprite { scaled_sprite_height - 8.0 * zoom } else { 24.0 * zoom };

            let name_width = self.measure_text_sharp(&player.name, font_size).width;
            let gm_width = if player.is_admin { self.measure_text_sharp(" (GM)", font_size).width - 2.0 * zoom } else { 0.0 };
            let total_width = name_width + gm_width;
            let name_x = screen_x - total_width / 2.0;
            let name_y = screen_y - name_y_offset + 2.0 * zoom;

            let padding = 4.0 * zoom;
            let bar_height = 18.0 * zoom;
            draw_rectangle(
                name_x - padding,
                name_y - 14.0 * zoom,
                total_width + padding * 2.0,
                bar_height,
                Color::from_rgba(0, 0, 0, 180),
            );

            self.draw_text_sharp(&player.name, name_x, name_y, font_size, WHITE);

            if player.is_admin {
                let gold_color = Color::from_rgba(255, 215, 0, 255);
                self.draw_text_sharp(" (GM)", name_x + name_width, name_y, font_size, gold_color);
            }
        }

        // NPC name tags
        for npc in state.npcs.values() {
            if npc.death_timer.is_some() || npc.is_death_animation_complete() {
                continue;
            }

            let is_selected = state.selected_entity_id.as_ref() == Some(&npc.id);
            let is_hovered = state.hovered_entity_id.as_ref() == Some(&npc.id);
            if !is_selected && !is_hovered {
                continue;
            }

            let (screen_x, screen_y) = world_to_screen(npc.x, npc.y, &state.camera);
            let zoom = state.camera.zoom;

            // Compute sprite height to find top_y
            let sprite_height = if let Some((_, h)) = self.npc_sprites.get_dimensions(&npc.entity_type) {
                (h * zoom).round()
            } else {
                // Fallback ellipse sizing
                let time = macroquad::time::get_time() as f32;
                let wobble = (time * 2.0 + npc.x + npc.y).sin() * 0.5 + 0.5;
                let radius = (10.0 + wobble * 1.5) * zoom;
                let height_offset = (8.0 + wobble * 2.0) * zoom;
                (height_offset + radius) * 2.0
            };
            let top_y = screen_y - sprite_height + 4.0 * zoom;

            let name_color = if npc.is_hostile() {
                Color::from_rgba(255, 150, 150, 255)
            } else if npc.is_quest_giver {
                Color::from_rgba(150, 220, 255, 255)
            } else if npc.is_merchant {
                Color::from_rgba(150, 255, 150, 255)
            } else {
                Color::from_rgba(255, 255, 255, 255)
            };

            let font_size = 16.0 * zoom;
            let name = npc.name();
            let name_width = self.measure_text_sharp(&name, font_size).width;
            let name_y = top_y - 5.0 * zoom;
            let padding = 4.0 * zoom;

            let small_icon: Option<&Texture2D> = if npc.is_quest_giver {
                self.chat_small_icon.as_ref()
            } else {
                None
            };

            let icon_gap = 4.0 * zoom;
            let (total_width, icon_width) = if let Some(tex) = small_icon {
                let w = tex.width() * zoom;
                (w + icon_gap + name_width, w)
            } else {
                (name_width, 0.0)
            };
            let content_x = screen_x - total_width / 2.0;

            let bar_height = 18.0 * zoom;
            draw_rectangle(
                content_x - padding,
                name_y - 14.0 * zoom,
                total_width + padding * 2.0,
                bar_height,
                Color::from_rgba(0, 0, 0, 180),
            );

            if let Some(tex) = small_icon {
                let icon_h = tex.height() * zoom;
                let bar_top = name_y - 14.0 * zoom;
                let icon_y = bar_top + (bar_height - icon_h) / 2.0;
                draw_texture_ex(tex, content_x, icon_y, WHITE, DrawTextureParams {
                    dest_size: Some(Vec2::new(tex.width() * zoom, icon_h)),
                    ..Default::default()
                });
            }

            let text_x = if small_icon.is_some() {
                content_x + icon_width + icon_gap
            } else {
                content_x
            };

            self.draw_text_sharp(&name, text_x, name_y, font_size, name_color);
        }
    }

    /// Render name tag for hovered tree showing name and level requirement
    fn render_tree_name_tag(&self, state: &GameState) {
        // Only show if we're hovering over a tile
        let Some((tile_x, tile_y)) = state.hovered_tile else {
            return;
        };

        // Check if this tile is depleted (don't show for stumps)
        if state.depleted_trees.contains_key(&(tile_x, tile_y)) {
            return;
        }

        // Check if there's an object at this exact tile (no tall-object extension)
        let Some(obj) = state.chunk_manager.get_object_at_exact(tile_x, tile_y) else {
            return;
        };

        // Check if this object is a tree (by GID)
        let Some(tree_info) = get_tree_info(obj.gid) else {
            return;
        };

        // Get player's woodcutting level
        let player_wc_level = state.get_local_player()
            .map(|p| p.skills.get(crate::game::SkillType::Woodcutting).level)
            .unwrap_or(1);

        let can_chop = player_wc_level >= tree_info.level_required;

        // Get screen position (center of tile, raised up)
        let (screen_x, screen_y) = world_to_screen(tile_x as f32 + 0.5, tile_y as f32 + 0.5, &state.camera);
        let zoom = state.camera.zoom;

        // Get actual sprite height for this tree
        let sprite_height = if let Some((texture, source_rect)) = self.get_object_sprite(obj.gid) {
            let tex_height = if let Some(r) = source_rect { r.h } else { texture.height() };
            tex_height * zoom
        } else {
            80.0 * zoom // Fallback if sprite not found
        };

        // Position the tag above the tree sprite
        let tag_y = screen_y - sprite_height - 5.0 * zoom;

        // Format text: "Oak Tree (Lvl 1)"
        let text = format!("{} (Lvl {})", tree_info.name, tree_info.level_required);
        let font_size = 16.0 * zoom;
        let text_dims = self.measure_text_sharp(&text, font_size);

        // Choose color based on whether player can chop
        let level_color = if can_chop {
            Color::from_rgba(100, 255, 100, 255) // Green
        } else {
            Color::from_rgba(255, 100, 100, 255) // Red
        };

        // Draw background
        let padding = 4.0 * zoom;
        let bar_height = 18.0 * zoom;
        let bar_x = screen_x - text_dims.width / 2.0 - padding;
        let bar_y = tag_y - 14.0 * zoom;

        draw_rectangle(
            bar_x,
            bar_y,
            text_dims.width + padding * 2.0,
            bar_height,
            Color::from_rgba(0, 0, 0, 180),
        );

        // Draw text
        let text_x = screen_x - text_dims.width / 2.0;
        self.draw_text_sharp(&text, text_x, tag_y, font_size, level_color);
    }

    /// Render chat bubbles above players' heads
    fn render_chat_bubbles(&self, state: &GameState) {
        let current_time = macroquad::time::get_time();

        for bubble in &state.chat_bubbles {
            let age = (current_time - bubble.time) as f32;
            if age > 5.0 {
                continue;
            }

            // Find the player this bubble belongs to
            let Some(player) = state.players.get(&bubble.player_id) else {
                continue;
            };

            // Get player screen position
            let (screen_x, screen_y) = world_to_screen(player.x, player.y, &state.camera);

            // Fade out in the last 1 second (age 4-5)
            let alpha = if age > 4.0 {
                ((5.0 - age) * 255.0) as u8
            } else {
                255
            };

            // Word wrap the text - scale with zoom for readability
            let zoom = state.camera.zoom;
            let font_size = 16.0 * zoom;
            let line_height = 18.0 * zoom;
            let max_bubble_width = 220.0 * zoom;
            let padding_h = 4.0 * zoom;
            let padding_v = 1.0 * zoom;
            let tail_height = 6.0 * zoom;
            let corner_radius = 5.0 * zoom;

            let lines = self.wrap_text(&bubble.text, max_bubble_width - padding_h * 2.0, font_size);
            let num_lines = lines.len().max(1);

            // Calculate bubble dimensions
            let mut max_line_width = 0.0f32;
            for line in &lines {
                let width = self.measure_text_sharp(line, font_size).width;
                max_line_width = max_line_width.max(width);
            }

            let bubble_width = (max_line_width + padding_h * 2.0).max(18.0 * zoom);
            let bubble_height = num_lines as f32 * line_height + padding_v * 2.0;

            // Position bubble above player's head
            // Base offset: sprite height (78) minus feet offset (8) = 70, scaled by zoom
            let base_offset = (SPRITE_HEIGHT - 8.0) * zoom;

            // Check if name tag is showing (hovered or selected) - need extra space
            let is_hovered = state.hovered_entity_id.as_ref() == Some(&bubble.player_id);
            let is_selected = state.selected_entity_id.as_ref() == Some(&bubble.player_id);
            let name_offset = if is_hovered || is_selected { 16.0 * zoom } else { 0.0 };

            let bubble_x = screen_x - bubble_width / 2.0;
            let bubble_y = screen_y - base_offset - name_offset - bubble_height - tail_height;

            // Colors with alpha - off-white paper/comic book style
            let bg_alpha = (alpha as f32 * 0.8) as u8; // 80% opacity for background
            let bg_color = Color::from_rgba(255, 250, 240, bg_alpha); // Warm off-white/cream
            let border_color = Color::from_rgba(60, 50, 40, alpha); // Dark brown border
            let text_color = Color::from_rgba(30, 25, 20, alpha); // Dark brown text

            // Draw rounded rectangle bubble body using mesh (no overlapping geometry)
            let r = corner_radius;
            let bx = bubble_x.floor();
            let by = bubble_y.floor();
            let bw = bubble_width.floor();
            let bh = bubble_height.floor();

            // Draw border first (slightly larger rounded rect)
            let border_mesh = Self::create_rounded_rect_mesh(bx - 1.0, by - 1.0, bw + 2.0, bh + 2.0, r + 1.0, border_color);
            draw_mesh(&border_mesh);

            // Draw fill on top using mesh (no overlapping = no alpha stacking)
            let fill_mesh = Self::create_rounded_rect_mesh(bx, by, bw, bh, r, bg_color);
            draw_mesh(&fill_mesh);

            // Draw tail
            let tail_x = screen_x.floor();
            let tail_top_y = by + bh;
            let tail_bottom_y = tail_top_y + tail_height;
            let tail_half_width = 4.0 * zoom;

            // Tail border
            draw_triangle(
                Vec2::new(tail_x - tail_half_width - 1.0, tail_top_y),
                Vec2::new(tail_x + tail_half_width + 1.0, tail_top_y),
                Vec2::new(tail_x, tail_bottom_y + 1.0),
                border_color,
            );
            // Tail fill - use a mesh vertex approach to match the bubble's alpha exactly
            // Create a small mesh for just the tail triangle
            let tail_color_arr = [
                (bg_color.r * 255.0) as u8,
                (bg_color.g * 255.0) as u8,
                (bg_color.b * 255.0) as u8,
                (bg_color.a * 255.0) as u8,
            ];
            let tail_mesh = Mesh {
                vertices: vec![
                    Vertex { position: Vec3::new(tail_x - tail_half_width, tail_top_y, 0.0), uv: Vec2::ZERO, color: tail_color_arr, normal: Vec4::ZERO },
                    Vertex { position: Vec3::new(tail_x + tail_half_width, tail_top_y, 0.0), uv: Vec2::ZERO, color: tail_color_arr, normal: Vec4::ZERO },
                    Vertex { position: Vec3::new(tail_x, tail_bottom_y, 0.0), uv: Vec2::ZERO, color: tail_color_arr, normal: Vec4::ZERO },
                ],
                indices: vec![0, 1, 2],
                texture: None,
            };
            draw_mesh(&tail_mesh);

            // Tail border lines
            draw_line(tail_x - tail_half_width, tail_top_y, tail_x, tail_bottom_y, 1.0, border_color);
            draw_line(tail_x + tail_half_width, tail_top_y, tail_x, tail_bottom_y, 1.0, border_color);

            // Draw text lines (centered)
            let bubble_center_x = bx + bw / 2.0;
            let mut text_y = by + padding_v + font_size * 0.85;

            for line in &lines {
                let line_width = self.measure_text_sharp(line, font_size).width;
                let text_x = bubble_center_x - line_width / 2.0;
                self.draw_text_sharp(line, text_x, text_y, font_size, text_color);
                text_y += line_height;
            }
        }

        // Render NPC speech bubbles
        for npc in state.npcs.values() {
            if npc.state == NpcState::Dead {
                continue;
            }

            let Some((ref text, time)) = npc.speech_bubble else {
                continue;
            };

            let age = (current_time - time) as f32;
            if age > 5.0 {
                continue;
            }

            // Get NPC screen position
            let (screen_x, screen_y) = world_to_screen(npc.x, npc.y, &state.camera);

            // Fade out in the last 1 second (age 4-5)
            let alpha = if age > 4.0 {
                ((5.0 - age) * 255.0) as u8
            } else {
                255
            };

            // Word wrap the text (same params as player bubbles) - scale with zoom
            let zoom = state.camera.zoom;
            let font_size = 16.0 * zoom;
            let line_height = 18.0 * zoom;
            let max_bubble_width = 220.0 * zoom;
            let padding_h = 4.0 * zoom;
            let padding_v = 1.0 * zoom;
            let tail_height = 6.0 * zoom;
            let corner_radius = 5.0 * zoom;

            let lines = self.wrap_text(text, max_bubble_width - padding_h * 2.0, font_size);
            let num_lines = lines.len().max(1);

            let mut max_line_width = 0.0f32;
            for line in &lines {
                let width = self.measure_text_sharp(line, font_size).width;
                max_line_width = max_line_width.max(width);
            }

            let bubble_width = (max_line_width + padding_h * 2.0).max(18.0 * zoom);
            let bubble_height = num_lines as f32 * line_height + padding_v * 2.0;

            // Position bubble above NPC's head
            let base_offset = (SPRITE_HEIGHT - 8.0) * zoom;

            let is_hovered = state.hovered_entity_id.as_ref() == Some(&npc.id);
            let is_selected = state.selected_entity_id.as_ref() == Some(&npc.id);
            let name_offset = if is_hovered || is_selected { 16.0 * zoom } else { 0.0 };

            let bubble_x = screen_x - bubble_width / 2.0;
            let bubble_y = screen_y - base_offset - name_offset - bubble_height - tail_height;

            // Colors with alpha - off-white paper/comic book style
            let bg_alpha = (alpha as f32 * 0.8) as u8;
            let bg_color = Color::from_rgba(255, 250, 240, bg_alpha);
            let border_color = Color::from_rgba(60, 50, 40, alpha);
            let text_color = Color::from_rgba(30, 25, 20, alpha);

            let r = corner_radius;
            let bx = bubble_x.floor();
            let by = bubble_y.floor();
            let bw = bubble_width.floor();
            let bh = bubble_height.floor();

            let border_mesh = Self::create_rounded_rect_mesh(bx - 1.0, by - 1.0, bw + 2.0, bh + 2.0, r + 1.0, border_color);
            draw_mesh(&border_mesh);

            let fill_mesh = Self::create_rounded_rect_mesh(bx, by, bw, bh, r, bg_color);
            draw_mesh(&fill_mesh);

            // Draw tail
            let tail_x = screen_x.floor();
            let tail_top_y = by + bh;
            let tail_bottom_y = tail_top_y + tail_height;
            let tail_half_width = 4.0 * zoom;

            draw_triangle(
                Vec2::new(tail_x - tail_half_width - 1.0, tail_top_y),
                Vec2::new(tail_x + tail_half_width + 1.0, tail_top_y),
                Vec2::new(tail_x, tail_bottom_y + 1.0),
                border_color,
            );

            let tail_color_arr = [
                (bg_color.r * 255.0) as u8,
                (bg_color.g * 255.0) as u8,
                (bg_color.b * 255.0) as u8,
                (bg_color.a * 255.0) as u8,
            ];
            let tail_mesh = Mesh {
                vertices: vec![
                    Vertex { position: Vec3::new(tail_x - tail_half_width, tail_top_y, 0.0), uv: Vec2::ZERO, color: tail_color_arr, normal: Vec4::ZERO },
                    Vertex { position: Vec3::new(tail_x + tail_half_width, tail_top_y, 0.0), uv: Vec2::ZERO, color: tail_color_arr, normal: Vec4::ZERO },
                    Vertex { position: Vec3::new(tail_x, tail_bottom_y, 0.0), uv: Vec2::ZERO, color: tail_color_arr, normal: Vec4::ZERO },
                ],
                indices: vec![0, 1, 2],
                texture: None,
            };
            draw_mesh(&tail_mesh);

            draw_line(tail_x - tail_half_width, tail_top_y, tail_x, tail_bottom_y, 1.0, border_color);
            draw_line(tail_x + tail_half_width, tail_top_y, tail_x, tail_bottom_y, 1.0, border_color);

            // Draw text lines (centered)
            let bubble_center_x = bx + bw / 2.0;
            let mut text_y = by + padding_v + font_size * 0.85;

            for line in &lines {
                let line_width = self.measure_text_sharp(line, font_size).width;
                let text_x = bubble_center_x - line_width / 2.0;
                self.draw_text_sharp(line, text_x, text_y, font_size, text_color);
                text_y += line_height;
            }
        }
    }

    fn render_projectiles(&self, state: &GameState) {
        let current_time = macroquad::time::get_time();

        for projectile in &state.projectiles {
            let (world_x, world_y) = projectile.current_pos(current_time);
            let (screen_x, screen_y_raw) = world_to_screen(world_x, world_y, &state.camera);

            // Offset arrow vertically to match player center (not feet)
            let arrow_y_offset = -40.0 * state.camera.zoom;
            let screen_y = screen_y_raw + arrow_y_offset;

            // Calculate direction in SCREEN space (accounts for isometric transform)
            let (start_screen_x, start_screen_y) = world_to_screen(projectile.start_x, projectile.start_y, &state.camera);
            let (end_screen_x, end_screen_y) = world_to_screen(projectile.end_x, projectile.end_y, &state.camera);
            let dx = end_screen_x - start_screen_x;
            let dy = end_screen_y - start_screen_y;
            let angle = dy.atan2(dx);

            // Snap to isometric angles (2:1 ratio = atan2(1,2) ≈ 26.57°)
            // 8 isometric directions: 0°, 26.57°, 90°, 153.43°, 180°, -153.43°, -90°, -26.57°
            let iso_angle = (0.5_f32).atan(); // atan(1/2) ≈ 26.57° ≈ 0.4636 rad
            let iso_angles: [f32; 8] = [
                0.0,                                    // UpRight (east)
                iso_angle,                              // Right (26.57°)
                std::f32::consts::FRAC_PI_2,           // DownRight (90°)
                std::f32::consts::PI - iso_angle,      // Down (153.43°)
                std::f32::consts::PI,                  // DownLeft (180°)
                -std::f32::consts::PI + iso_angle,     // Left (-153.43°)
                -std::f32::consts::FRAC_PI_2,          // UpLeft (-90°)
                -iso_angle,                            // Up (-26.57°)
            ];

            // Find nearest isometric angle
            let mut snapped_angle = iso_angles[0];
            let mut min_diff = f32::MAX;
            for &iso_ang in &iso_angles {
                let mut diff = (angle - iso_ang).abs();
                // Handle wrap-around at ±180°
                if diff > std::f32::consts::PI {
                    diff = 2.0 * std::f32::consts::PI - diff;
                }
                if diff < min_diff {
                    min_diff = diff;
                    snapped_angle = iso_ang;
                }
            }

            // Direction vector from snapped angle
            let dir_x = snapped_angle.cos();
            let dir_y = snapped_angle.sin();

            // Perpendicular vector for arrow width
            let perp_x = -dir_y;
            let perp_y = dir_x;

            // Arrow dimensions
            let shaft_length = 18.0;
            let shaft_width = 2.0;
            let head_length = 6.0;
            let head_width = 5.0;
            let fletch_length = 4.0;
            let fletch_width = 3.0;

            // Colors
            let shaft_color = Color::new(0.55, 0.35, 0.15, 1.0); // Wood brown
            let head_color = Color::new(0.45, 0.45, 0.5, 1.0);   // Metal gray
            let fletch_color = Color::new(0.85, 0.85, 0.8, 1.0); // Light feathers

            // Arrow positions
            let tip_x = screen_x + dir_x * (shaft_length / 2.0 + head_length);
            let tip_y = screen_y + dir_y * (shaft_length / 2.0 + head_length);
            let back_x = screen_x - dir_x * shaft_length / 2.0;
            let back_y = screen_y - dir_y * shaft_length / 2.0;

            // Draw shaft (thick line)
            draw_line(
                back_x,
                back_y,
                screen_x + dir_x * shaft_length / 2.0,
                screen_y + dir_y * shaft_length / 2.0,
                shaft_width,
                shaft_color,
            );

            // Draw arrowhead (triangle pointing forward)
            let head_base_x = screen_x + dir_x * shaft_length / 2.0;
            let head_base_y = screen_y + dir_y * shaft_length / 2.0;
            draw_triangle(
                Vec2::new(tip_x, tip_y),
                Vec2::new(head_base_x + perp_x * head_width / 2.0, head_base_y + perp_y * head_width / 2.0),
                Vec2::new(head_base_x - perp_x * head_width / 2.0, head_base_y - perp_y * head_width / 2.0),
                head_color,
            );

            // Draw fletching (two small triangles at the back)
            let fletch_base_x = back_x + dir_x * fletch_length;
            let fletch_base_y = back_y + dir_y * fletch_length;

            // Left fletch
            draw_triangle(
                Vec2::new(back_x + perp_x * shaft_width / 2.0, back_y + perp_y * shaft_width / 2.0),
                Vec2::new(fletch_base_x + perp_x * shaft_width / 2.0, fletch_base_y + perp_y * shaft_width / 2.0),
                Vec2::new(back_x + perp_x * fletch_width, back_y + perp_y * fletch_width),
                fletch_color,
            );

            // Right fletch
            draw_triangle(
                Vec2::new(back_x - perp_x * shaft_width / 2.0, back_y - perp_y * shaft_width / 2.0),
                Vec2::new(fletch_base_x - perp_x * shaft_width / 2.0, fletch_base_y - perp_y * shaft_width / 2.0),
                Vec2::new(back_x - perp_x * fletch_width, back_y - perp_y * fletch_width),
                fletch_color,
            );
        }
    }

    fn render_spell_effects(&self, state: &GameState) {
        let current_time = macroquad::time::get_time();

        for effect in &state.spell_effects {
            let elapsed = current_time - effect.time;

            // Look up the effect sprite based on spell_id
            let sprite_name = match effect.spell_id.as_str() {
                "dark_hand" => "dark_hand",
                "dark_eater" => "dark_eater",
                "heal" => "self_heal",
                "teleport" => "bubbles_warp",
                _ => continue,
            };

            let (texture, atlas_offset) = match self.spell_effect_textures.get(sprite_name) {
                Some(t) => t,
                None => continue,
            };

            // Get sprite dimensions (from atlas rect or texture size)
            let (tex_w, tex_h) = self.spell_effect_textures.get_dimensions(sprite_name)
                .unwrap_or((texture.width(), texture.height()));
            let frame_count = 5usize;
            let frame_w = tex_w / frame_count as f32;
            let frame_h = tex_h;
            let fps = 10.0_f64;
            let total_duration = frame_count as f64 / fps;

            if elapsed > total_duration {
                continue; // Animation finished
            }

            let frame_idx = ((elapsed * fps) as usize).min(frame_count - 1);

            // Calculate screen position from world coordinates
            let (screen_x, screen_y) = world_to_screen(
                effect.target_x as f32,
                effect.target_y as f32,
                &state.camera,
            );

            // Draw the current frame, centered on the tile
            let zoom = state.camera.zoom;
            let draw_w = frame_w * zoom;
            let draw_h = frame_h * zoom;
            // Apply atlas offset if present
            let (offset_x, offset_y) = atlas_offset.unwrap_or((0.0, 0.0));
            let source_rect = Rect::new(
                offset_x + frame_idx as f32 * frame_w,
                offset_y,
                frame_w,
                frame_h,
            );

            // Align like isometric objects: center on slightly elevated tile position
            let elevated_y = screen_y - TILE_HEIGHT * zoom * 0.25 - 22.0 * zoom;

            draw_texture_ex(
                texture,
                screen_x - draw_w / 2.0,
                elevated_y - draw_h / 2.0,
                WHITE,
                DrawTextureParams {
                    source: Some(source_rect),
                    dest_size: Some(Vec2::new(draw_w, draw_h)),
                    ..Default::default()
                },
            );
        }
    }

    fn render_damage_numbers(&self, state: &GameState) {
        let current_time = macroquad::time::get_time();
        let zoom = state.camera.zoom;
        const DURATION: f32 = 1.2;
        let font_size = 16.0 * zoom;

        for event in &state.damage_events {
            let age = (current_time - event.time) as f32;
            if age > DURATION {
                continue;
            }

            let t = age / DURATION;

            // Steady float upward - round to whole pixels for crisp movement, scale with zoom
            let float_offset = (age * 40.0 * zoom).round();

            // Compute height offset based on entity type and actual sprite size
            let height_offset = if state.players.contains_key(&event.target_id) {
                (SPRITE_HEIGHT - 8.0) * zoom / 2.0 // Center of player sprite
            } else if let Some(npc) = state.npcs.get(&event.target_id) {
                // Use actual sprite height if available, otherwise fallback to ellipse size
                if let Some((_, h)) = self.npc_sprites.get_dimensions(&npc.entity_type) {
                    h * zoom / 2.0 // Center of NPC sprite
                } else {
                    12.0 * zoom // Center of fallback ellipse
                }
            } else {
                25.0 * zoom // Fallback for unknown entities
            };

            let (screen_x, screen_y) = world_to_screen(event.x, event.y, &state.camera);
            // Round all positions to whole pixels
            let final_y = (screen_y - height_offset - float_offset).round();

            // Fade: visible for first half, then fade out
            let alpha = if t < 0.5 {
                1.0
            } else {
                1.0 - (t - 0.5) * 2.0
            };

            // Text and color
            let (text, base_color) = if event.damage > 0 {
                (format!("-{}", event.damage), Color::new(1.0, 0.3, 0.2, alpha))
            } else if event.damage < 0 {
                (format!("+{}", -event.damage), Color::new(0.3, 1.0, 0.4, alpha))
            } else {
                ("MISS".to_string(), Color::new(0.6, 0.6, 0.6, alpha))
            };

            let text_dims = self.measure_text_sharp(&text, font_size);
            // Round center position to whole pixels
            let draw_x = (screen_x - text_dims.width / 2.0).round();

            // Simple outline - scale offset with zoom
            let outline_offset = 1.0 * zoom;
            let outline_color = Color::new(0.0, 0.0, 0.0, alpha * 0.9);
            for &(ox, oy) in &[(-outline_offset, -outline_offset), (outline_offset, -outline_offset), (-outline_offset, outline_offset), (outline_offset, outline_offset)] {
                self.draw_text_sharp(&text, draw_x + ox, final_y + oy, font_size, outline_color);
            }

            self.draw_text_sharp(&text, draw_x, final_y, font_size, base_color);
        }
    }

    fn render_tilemap_layer(&self, state: &GameState, layer_type: LayerType) {
        // Don't render anything until world is ready (player exists and their chunk is loaded)
        // This prevents showing the fallback test tilemap or empty world during login
        if !state.is_world_ready() {
            return;
        }

        // Convert LayerType to ChunkLayerType for chunk rendering
        let chunk_layer_type = match layer_type {
            LayerType::Ground => ChunkLayerType::Ground,
            LayerType::Objects => ChunkLayerType::Objects,
            LayerType::Overhead => ChunkLayerType::Overhead,
        };

        // Try to render from chunks if any are loaded
        let chunks = state.chunk_manager.chunks();
        if !chunks.is_empty() {
            // Screen bounds for culling (use virtual size for mobile scaling)
            let (screen_w, screen_h) = virtual_screen_size();
            let margin = TILE_WIDTH * 4.0; // Extra margin for chunk edges

            // Check if we're in interior mode
            let interior_size = state.chunk_manager.get_interior_size();

            // Render from chunk manager
            for (coord, chunk) in chunks.iter() {
                // For interiors, the chunk is at (0,0) and uses interior dimensions
                // For overworld, use standard CHUNK_SIZE
                let (tile_width, tile_height) = interior_size.unwrap_or((CHUNK_SIZE, CHUNK_SIZE));

                let chunk_offset_x = coord.x * CHUNK_SIZE as i32;
                let chunk_offset_y = coord.y * CHUNK_SIZE as i32;

                // CHUNK-LEVEL CULLING: Check if chunk is visible before iterating tiles
                // In isometric projection, a chunk forms a diamond. Check all 4 corners.
                let corners = [
                    (chunk_offset_x as f32, chunk_offset_y as f32),                           // top
                    (chunk_offset_x as f32 + tile_width as f32, chunk_offset_y as f32),       // right
                    (chunk_offset_x as f32, chunk_offset_y as f32 + tile_height as f32),       // left
                    (chunk_offset_x as f32 + tile_width as f32, chunk_offset_y as f32 + tile_height as f32), // bottom
                ];

                // Get screen bounds of the chunk
                let mut min_sx = f32::MAX;
                let mut max_sx = f32::MIN;
                let mut min_sy = f32::MAX;
                let mut max_sy = f32::MIN;

                for (wx, wy) in corners {
                    let (sx, sy) = world_to_screen(wx, wy, &state.camera);
                    min_sx = min_sx.min(sx);
                    max_sx = max_sx.max(sx);
                    min_sy = min_sy.min(sy);
                    max_sy = max_sy.max(sy);
                }

                // Skip entire chunk if completely off-screen
                if max_sx < -margin || min_sx > screen_w + margin ||
                   max_sy < -margin || min_sy > screen_h + margin {
                    continue;
                }

                // Find the layer
                for layer in &chunk.layers {
                    if layer.layer_type != chunk_layer_type {
                        continue;
                    }

                    // Compute base screen position for chunk origin, then step incrementally
                    // In isometric: +1 world_x = (+dx, +dy), +1 world_y = (-dx, +dy)
                    // Use _exact (no rounding) for the base to avoid double-rounding jitter
                    let zoom = state.camera.zoom;
                    let dx = (TILE_WIDTH / 2.0) * zoom;
                    let dy = (TILE_HEIGHT / 2.0) * zoom;
                    let (base_sx, base_sy) = world_to_screen_exact(chunk_offset_x as f32, chunk_offset_y as f32, &state.camera);
                    let water_effects = !state.ui_state.graphics_low;

                    // Tile-level culling bounds
                    let tile_margin = TILE_WIDTH * 2.0;
                    let cull_left = -tile_margin;
                    let cull_right = screen_w + tile_margin;
                    let cull_top = -tile_margin;
                    let cull_bottom = screen_h + tile_margin;

                    // Render tiles in isometric order
                    for local_y in 0..tile_height {
                        let row_sx = base_sx - local_y as f32 * dx;
                        let row_sy = base_sy + local_y as f32 * dy;

                        for local_x in 0..tile_width {
                            let idx = (local_y * tile_width + local_x) as usize;
                            let tile_id = layer.tiles.get(idx).copied().unwrap_or(0);

                            if tile_id == 0 {
                                continue;
                            }

                            let screen_x = (row_sx + local_x as f32 * dx).round();
                            let screen_y = (row_sy + local_x as f32 * dy).round();

                            // Tile-level culling (still needed for partially visible chunks)
                            if screen_x < cull_left || screen_x > cull_right
                                || screen_y < cull_top || screen_y > cull_bottom {
                                continue;
                            }

                            let world_x = chunk_offset_x + local_x as i32;
                            let world_y = chunk_offset_y + local_y as i32;

                            // Draw tile sprite (or fallback to colored tile)
                            self.draw_tile_sprite(screen_x, screen_y, tile_id, zoom, Some((world_x as f32, world_y as f32)), water_effects);

                            // Draw collision indicator in debug mode
                            if state.debug_mode && chunk.collision.get(idx).copied().unwrap_or(false) {
                                self.draw_collision_indicator(screen_x, screen_y, zoom);
                            }
                        }
                    }
                }
            }
            return;
        }

        // Fallback: render from old tilemap if no chunks loaded
        let tilemap = &state.tilemap;

        for layer in &tilemap.layers {
            if layer.layer_type != layer_type {
                continue;
            }

            // Render in isometric order (back to front)
            let (vw, vh) = virtual_screen_size();
            let zoom = state.camera.zoom;
            let dx = (TILE_WIDTH / 2.0) * zoom;
            let dy = (TILE_HEIGHT / 2.0) * zoom;
            let (base_sx, base_sy) = world_to_screen_exact(0.0, 0.0, &state.camera);
            let water_effects = !state.ui_state.graphics_low;
            let margin = TILE_WIDTH * 2.0;

            for y in 0..tilemap.height {
                let row_sx = base_sx - y as f32 * dx;
                let row_sy = base_sy + y as f32 * dy;

                for x in 0..tilemap.width {
                    let idx = (y * tilemap.width + x) as usize;
                    let tile_id = layer.tiles.get(idx).copied().unwrap_or(0);

                    if tile_id == 0 {
                        continue; // Skip empty tiles
                    }

                    let screen_x = (row_sx + x as f32 * dx).round();
                    let screen_y = (row_sy + x as f32 * dy).round();

                    // Culling: skip tiles outside viewport
                    if screen_x < -margin || screen_x > vw + margin
                        || screen_y < -margin || screen_y > vh + margin {
                        continue;
                    }

                    // Draw tile sprite (or fallback to colored tile)
                    self.draw_tile_sprite(screen_x, screen_y, tile_id, zoom, Some((x as f32, y as f32)), water_effects);

                    // Draw collision indicator in debug mode
                    if state.debug_mode && tilemap.collision.get(idx).copied().unwrap_or(false) {
                        self.draw_collision_indicator(screen_x, screen_y, zoom);
                    }
                }
            }
        }
    }

    fn draw_collision_indicator(&self, screen_x: f32, screen_y: f32, zoom: f32) {
        let half_w = TILE_WIDTH * zoom / 4.0;
        let half_h = TILE_HEIGHT * zoom / 4.0;
        draw_rectangle_lines(
            screen_x - half_w,
            screen_y - half_h,
            half_w * 2.0,
            half_h * 2.0,
            2.0 * zoom,
            Color::from_rgba(255, 0, 0, 150),
        );
    }

    fn draw_isometric_object(&self, screen_x: f32, screen_y: f32, tile_id: u32, zoom: f32) {
        // Draw shadow ellipse for objects
        draw_ellipse(screen_x, screen_y + 4.0 * zoom, 24.0 * zoom, 16.0 * zoom, 0.0, Color::from_rgba(0, 0, 0, 50));

        // Draw object tile sprite (slightly elevated)
        let elevated_y = screen_y - TILE_HEIGHT * zoom * 0.25;
        self.draw_tile_sprite(screen_x, elevated_y, tile_id, zoom, None, false);
    }

    fn draw_isometric_tile(&self, screen_x: f32, screen_y: f32, color: Color, zoom: f32) {
        // Draw a diamond-shaped tile
        let half_w = TILE_WIDTH * zoom / 2.0;
        let half_h = TILE_HEIGHT * zoom / 2.0;

        // Diamond vertices (clockwise from top)
        let top = (screen_x, screen_y - half_h);
        let right = (screen_x + half_w, screen_y);
        let bottom = (screen_x, screen_y + half_h);
        let left = (screen_x - half_w, screen_y);

        // Draw as two triangles
        draw_triangle(
            Vec2::new(top.0, top.1),
            Vec2::new(right.0, right.1),
            Vec2::new(bottom.0, bottom.1),
            color,
        );
        draw_triangle(
            Vec2::new(top.0, top.1),
            Vec2::new(bottom.0, bottom.1),
            Vec2::new(left.0, left.1),
            color,
        );

        // Draw outline
        let outline_color = Color::from_rgba(80, 80, 90, 255);
        draw_line(top.0, top.1, right.0, right.1, 1.0, outline_color);
        draw_line(right.0, right.1, bottom.0, bottom.1, 1.0, outline_color);
        draw_line(bottom.0, bottom.1, left.0, left.1, 1.0, outline_color);
        draw_line(left.0, left.1, top.0, top.1, 1.0, outline_color);
    }

    /// Draw a selection highlight around the tile at the given world position
    fn render_tile_selection(&self, world_x: f32, world_y: f32, camera: &Camera) {
        // Get the tile the entity is standing on (floor to get tile coords)
        let tile_x = world_x.floor();
        let tile_y = world_y.floor();

        // Get the center of that tile in screen space
        // Offset by half_h to align with where entities visually stand on the tile
        let (center_x, center_y) = world_to_screen(tile_x + 0.5, tile_y + 0.5, camera);
        let center_y = center_y - TILE_HEIGHT * camera.zoom / 2.0;

        // Tile dimensions (half-sizes for diamond corners), scaled by zoom
        let half_w = TILE_WIDTH * camera.zoom / 2.0;
        let half_h = TILE_HEIGHT * camera.zoom / 2.0;

        // Diamond corners (isometric tile shape)
        let top = (center_x, center_y - half_h);
        let right = (center_x + half_w, center_y);
        let bottom = (center_x, center_y + half_h);
        let left = (center_x - half_w, center_y);

        // Pulsing effect
        let pulse = (macroquad::time::get_time() * 3.0).sin() as f32 * 0.3 + 0.7;
        let alpha = (pulse * 255.0) as u8;
        let color = Color::from_rgba(255, 255, 0, alpha);

        // Draw yellow diamond outline
        let line_width = 2.0 * camera.zoom;
        draw_line(top.0, top.1, right.0, right.1, line_width, color);
        draw_line(right.0, right.1, bottom.0, bottom.1, line_width, color);
        draw_line(bottom.0, bottom.1, left.0, left.1, line_width, color);
        draw_line(left.0, left.1, top.0, top.1, line_width, color);
    }

    /// Draw corner indicators for the hovered tile
    pub(crate) fn render_tile_hover(&self, tile_x: i32, tile_y: i32, camera: &Camera) {
        // Get the center of the tile in screen space
        let (center_x, center_y) = world_to_screen(tile_x as f32 + 0.5, tile_y as f32 + 0.5, camera);
        let center_y = center_y - TILE_HEIGHT * camera.zoom / 2.0;

        // Tile dimensions (half-sizes for diamond corners), scaled by zoom
        let half_w = TILE_WIDTH * camera.zoom / 2.0;
        let half_h = TILE_HEIGHT * camera.zoom / 2.0;

        // Diamond corners (isometric tile shape)
        let top = (center_x, center_y - half_h);
        let right = (center_x + half_w, center_y);
        let bottom = (center_x, center_y + half_h);
        let left = (center_x - half_w, center_y);

        // Corner size as fraction of edge length
        let t = 0.2;

        // Thin white lines with some transparency
        let color = Color::from_rgba(255, 255, 255, 180);
        let line_width = 1.0 * camera.zoom;

        // Top corner
        draw_line(top.0, top.1, top.0 + (left.0 - top.0) * t, top.1 + (left.1 - top.1) * t, line_width, color);
        draw_line(top.0, top.1, top.0 + (right.0 - top.0) * t, top.1 + (right.1 - top.1) * t, line_width, color);

        // Right corner
        draw_line(right.0, right.1, right.0 + (top.0 - right.0) * t, right.1 + (top.1 - right.1) * t, line_width, color);
        draw_line(right.0, right.1, right.0 + (bottom.0 - right.0) * t, right.1 + (bottom.1 - right.1) * t, line_width, color);

        // Bottom corner
        draw_line(bottom.0, bottom.1, bottom.0 + (right.0 - bottom.0) * t, bottom.1 + (right.1 - bottom.1) * t, line_width, color);
        draw_line(bottom.0, bottom.1, bottom.0 + (left.0 - bottom.0) * t, bottom.1 + (left.1 - bottom.1) * t, line_width, color);

        // Left corner
        draw_line(left.0, left.1, left.0 + (bottom.0 - left.0) * t, left.1 + (bottom.1 - left.1) * t, line_width, color);
        draw_line(left.0, left.1, left.0 + (top.0 - left.0) * t, left.1 + (top.1 - left.1) * t, line_width, color);
    }

    /// Draw a green drop zone indicator for a tile (when dragging items)
    pub(crate) fn render_drop_zone(&self, tile_x: i32, tile_y: i32, camera: &Camera, is_hovered: bool) {
        // Get the center of the tile in screen space
        let (center_x, center_y) = world_to_screen(tile_x as f32 + 0.5, tile_y as f32 + 0.5, camera);
        let center_y = center_y - TILE_HEIGHT * camera.zoom / 2.0;

        // Tile dimensions (half-sizes for diamond corners), scaled by zoom
        let half_w = TILE_WIDTH * camera.zoom / 2.0;
        let half_h = TILE_HEIGHT * camera.zoom / 2.0;

        // Diamond corners (isometric tile shape)
        let top = (center_x, center_y - half_h);
        let right = (center_x + half_w, center_y);
        let bottom = (center_x, center_y + half_h);
        let left = (center_x - half_w, center_y);

        // Green color - brighter when hovered
        let color = if is_hovered {
            Color::from_rgba(100, 255, 100, 120)
        } else {
            Color::from_rgba(100, 200, 100, 60)
        };

        // Draw filled diamond shape
        draw_triangle(
            Vec2::new(top.0, top.1),
            Vec2::new(right.0, right.1),
            Vec2::new(bottom.0, bottom.1),
            color,
        );
        draw_triangle(
            Vec2::new(top.0, top.1),
            Vec2::new(bottom.0, bottom.1),
            Vec2::new(left.0, left.1),
            color,
        );

        // Draw border if hovered
        if is_hovered {
            let border_color = Color::from_rgba(150, 255, 150, 200);
            let line_width = 1.0 * camera.zoom;
            draw_line(top.0, top.1, right.0, right.1, line_width, border_color);
            draw_line(right.0, right.1, bottom.0, bottom.1, line_width, border_color);
            draw_line(bottom.0, bottom.1, left.0, left.1, line_width, border_color);
            draw_line(left.0, left.1, top.0, top.1, line_width, border_color);
        }
    }

    fn render_player(&self, player: &Player, is_local: bool, is_selected: bool, is_hovered: bool, camera: &Camera) {
        let (screen_x, screen_y) = world_to_screen(player.x, player.y, camera);
        let zoom = camera.zoom;

        // Scaled sprite dimensions
        let scaled_sprite_width = SPRITE_WIDTH * zoom;
        let scaled_sprite_height = SPRITE_HEIGHT * zoom;

        // Dead players are faded
        let alpha = if player.is_dead { 100 } else { 255 };

        // Selection highlight (draw first, behind player)
        if is_selected && !player.is_dead {
            self.render_tile_selection(player.x, player.y, camera);
        }

        // Vertical offset for sitting on chair (shift up to center on tile)
        let sit_offset_y = if player.animation.state == crate::render::animation::AnimationState::SittingChair {
            10.0 * zoom
        } else {
            0.0
        };

        // Draw shadow under player
        draw_ellipse(screen_x, screen_y, 16.0 * zoom, 7.0 * zoom, 0.0, Color::from_rgba(0, 0, 0, 60));

        // Try to render sprite based on player's appearance, fall back to colored circle
        if let Some((player_texture, player_offset)) = self.get_player_sprite(&player.gender, &player.skin) {
            let coords = player.animation.get_sprite_coords();
            let (player_atlas_x, player_atlas_y) = player_offset.unwrap_or((0.0, 0.0));
            let (src_x, src_y, src_w, src_h) = coords.to_source_rect();

            // Tint for local player distinction (slight green tint)
            let tint = if is_local {
                Color::from_rgba(220, 255, 220, alpha)
            } else {
                Color::from_rgba(255, 255, 255, alpha)
            };

            // Position sprite so feet are at screen_y
            let draw_x = screen_x - scaled_sprite_width / 2.0;
            let draw_y = screen_y - scaled_sprite_height + 8.0 * zoom + sit_offset_y; // Offset to align feet with tile

            // Get player gender for gender-specific offsets
            let player_gender = Gender::from_str(&player.gender);

            // Calculate weapon frame info if weapon is equipped (hidden when sitting)
            let is_sitting = matches!(player.animation.state,
                crate::render::animation::AnimationState::SittingChair | crate::render::animation::AnimationState::SittingGround);
            let weapon_info = player.equipped_weapon.as_ref().filter(|_| !is_sitting).and_then(|weapon_id| {
                self.weapon_sprites.get(weapon_id).map(|(tex, atlas_offset)| {
                    let anim_frame = player.animation.frame as u32;
                    let weapon_frame = get_weapon_frame(player.animation.state, player.animation.direction, anim_frame);
                    let (offset_x, offset_y) = get_weapon_offset(player.animation.state, player.animation.direction, anim_frame, player_gender);
                    // Get weapon frame size from manifest, fallback to default
                    let (fw, fh) = self.weapon_frame_sizes.get(weapon_id)
                        .copied()
                        .unwrap_or((WEAPON_SPRITE_WIDTH, WEAPON_SPRITE_HEIGHT));
                    (tex, atlas_offset, weapon_frame, offset_x, offset_y, fw, fh)
                })
            });

            // Scaled weapon dimensions (per-weapon)
            let (scaled_weapon_width, scaled_weapon_height, wf_width, wf_height) = weapon_info.as_ref()
                .map(|(_, _, _, _, _, fw, fh)| (*fw * zoom, *fh * zoom, *fw, *fh))
                .unwrap_or((WEAPON_SPRITE_WIDTH * zoom, WEAPON_SPRITE_HEIGHT * zoom, WEAPON_SPRITE_WIDTH, WEAPON_SPRITE_HEIGHT));

            // Draw weapon under-layer (before player sprite)
            if let Some((weapon_sprite, atlas_offset, ref weapon_frame, offset_x, offset_y, _, _)) = weapon_info {
                let (atlas_x, atlas_y) = atlas_offset.unwrap_or((0.0, 0.0));
                let weapon_src_x = atlas_x + weapon_frame.frame_under as f32 * wf_width;
                let weapon_draw_x = draw_x + offset_x * zoom;
                let weapon_draw_y = draw_y + offset_y * zoom;

                draw_texture_ex(
                    weapon_sprite,
                    weapon_draw_x,
                    weapon_draw_y,
                    tint,
                    DrawTextureParams {
                        source: Some(Rect::new(weapon_src_x, atlas_y, wf_width, wf_height)),
                        dest_size: Some(Vec2::new(scaled_weapon_width, scaled_weapon_height)),
                        flip_x: weapon_frame.flip_h,
                        ..Default::default()
                    },
                );
            }

            // Draw back static items BEHIND player (for down/right directions - tip peeks out)
            if let Some(ref back_item_id) = player.equipped_back {
                if let Some((equip_texture, equip_offset)) = self.equipment_sprites.get(back_item_id) {
                    // Check if this is an offhand item based on dimensions
                    let (equip_w, equip_h) = self.equipment_sprites.get_dimensions(back_item_id)
                        .unwrap_or((equip_texture.width(), equip_texture.height()));
                    let is_offhand = equip_w > equip_h * 8.0;
                    if !is_offhand {
                        let anim_frame = player.animation.frame as u32;
                        let back_frame = get_back_static_frame(player.animation.direction);
                        if back_frame.render_behind {
                            let (back_offset_x, back_offset_y) = get_back_static_offset(player.animation.state, player.animation.direction, anim_frame, player_gender);
                            let (atlas_x, atlas_y) = equip_offset.unwrap_or((0.0, 0.0));
                            let back_src_x = atlas_x + back_frame.frame as f32 * BACK_STATIC_SPRITE_WIDTH;
                            let scaled_back_width = BACK_STATIC_SPRITE_WIDTH * zoom;
                            let scaled_back_height = BACK_STATIC_SPRITE_HEIGHT * zoom;
                            let back_draw_x = draw_x + back_offset_x * zoom;
                            let back_draw_y = draw_y + back_offset_y * zoom;

                            draw_texture_ex(
                                equip_texture,
                                back_draw_x,
                                back_draw_y,
                                tint,
                                DrawTextureParams {
                                    source: Some(Rect::new(back_src_x, atlas_y, BACK_STATIC_SPRITE_WIDTH, BACK_STATIC_SPRITE_HEIGHT)),
                                    dest_size: Some(Vec2::new(scaled_back_width, scaled_back_height)),
                                    flip_x: back_frame.flip_h,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }
            }

            // Draw player sprite
            draw_texture_ex(
                player_texture,
                draw_x,
                draw_y,
                tint,
                DrawTextureParams {
                    source: Some(Rect::new(player_atlas_x + src_x, player_atlas_y + src_y, src_w, src_h)),
                    dest_size: Some(Vec2::new(scaled_sprite_width, scaled_sprite_height)),
                    flip_x: coords.flip_h,
                    ..Default::default()
                },
            );

            // Draw hair and head equipment (after base sprite, before body armor)
            // Check if player has head equipment that we can render with shader
            let head_item_id_ref = player.equipped_head.as_ref();
            let head_sprite_data = head_item_id_ref
                .and_then(|head_item_id| {
                    let (tex, offset) = self.equipment_sprites.get(head_item_id)?;
                    let (w, h) = self.equipment_sprites.get_dimensions(head_item_id)?;
                    Some((tex, offset, w, h))
                });

            let has_shader = self.head_hair_material.is_some();

            if let Some((head_texture, head_offset, _head_rect_w, _head_rect_h)) = head_sprite_data {
                // Player has head equipment - use shader compositing if available
                if has_shader {
                    if let (Some(style), Some(color)) = (player.hair_style, player.hair_color) {
                        let hair_key = format!("{}_{}", player.gender, style);
                        if let Some((hair_texture, hair_atlas_offset)) = self.hair_sprites.get(&hair_key) {
                            // For UV calculations, we need the FULL texture dimensions, not sprite rect dimensions
                            // get_dimensions() returns sprite rect size in atlas mode, but UVs need full texture size
                            let hair_full_tex_w = hair_texture.width();
                            let hair_full_tex_h = hair_texture.height();
                            let head_full_tex_w = head_texture.width();
                            let head_full_tex_h = head_texture.height();

                            // Get atlas offsets (0,0 if not using atlas)
                            let (hair_atlas_x, hair_atlas_y) = hair_atlas_offset.unwrap_or((0.0, 0.0));
                            let (head_atlas_x, head_atlas_y) = head_offset.unwrap_or((0.0, 0.0));

                            // Calculate hair frame info
                            let is_back = matches!(player.animation.direction, Direction::Up | Direction::Left);
                            let frame_index = color * 2 + if is_back { 1 } else { 0 };
                            let hair_src_x = frame_index as f32 * HAIR_SPRITE_WIDTH;

                            // Calculate hair offsets using gender-aware function
                            let anim_frame = player.animation.frame as u32;
                            let (hair_pos_x, hair_pos_y) = get_hair_offset(
                                player.animation.state,
                                player.animation.direction,
                                anim_frame,
                                player_gender,
                                coords.flip_h,
                            );

                            // Calculate head frame info
                            let head_frame = get_head_frame(player.animation.direction);
                            let (head_pos_x, head_pos_y) = get_head_offset(player.animation.state, player.animation.direction, anim_frame, player_gender);
                            let head_src_x = head_frame.frame as f32 * HEAD_SPRITE_WIDTH;

                            // Calculate pixel offset from head origin to hair origin (in unscaled pixels)
                            // Hair is centered: hair_x = (SPRITE_WIDTH - HAIR_SPRITE_WIDTH) / 2 + hair_pos_x = 3 + hair_pos_x
                            // Head uses head_pos_x directly
                            let hair_base_x = (SPRITE_WIDTH - HAIR_SPRITE_WIDTH) / 2.0 + hair_pos_x;
                            let hair_base_y = hair_pos_y; // sit offset already included in get_hair_offset
                            let delta_x = hair_base_x - head_pos_x;
                            let delta_y = hair_base_y - head_pos_y;

                            // Compute UV transform for shader
                            // The shader needs to transform head UV to hair UV
                            // UVs are in full-texture coords [0,1], so we use full texture dimensions

                            // Head source rect in normalized UV (including atlas offset)
                            let head_uv_x = (head_atlas_x + head_src_x) / head_full_tex_w;
                            let head_uv_y = head_atlas_y / head_full_tex_h;
                            let head_uv_w = HEAD_SPRITE_WIDTH / head_full_tex_w;
                            let head_uv_h = HEAD_SPRITE_HEIGHT / head_full_tex_h;

                            // Hair source rect in normalized UV (including atlas offset)
                            let hair_uv_x = (hair_atlas_x + hair_src_x) / hair_full_tex_w;
                            let hair_uv_y = hair_atlas_y / hair_full_tex_h;
                            let hair_uv_w = HAIR_SPRITE_WIDTH / hair_full_tex_w;
                            let hair_uv_h = HAIR_SPRITE_HEIGHT / hair_full_tex_h;

                            // The transform: given head UV (u, v) in full texture coords
                            // 1. Normalize to head frame: local = (u - head_uv_x) / head_uv_w, (v - head_uv_y) / head_uv_h
                            // 2. To pixels: pixel = local * (HEAD_SPRITE_WIDTH, HEAD_SPRITE_HEIGHT)
                            // 3. Offset: hair_pixel = pixel - (delta_x, delta_y)
                            // 4. To hair local: hair_local = hair_pixel / (HAIR_SPRITE_WIDTH, HAIR_SPRITE_HEIGHT)
                            // 5. To hair UV: hair_uv = hair_uv_x + hair_local.x * hair_uv_w, hair_uv_y + hair_local.y * hair_uv_h

                            // Combining and simplifying (see derivation in comments above):
                            // hair_uv.x = offset_x + u * scale_x
                            // hair_uv.y = offset_y + v * scale_y

                            let scale_x = head_full_tex_w * hair_uv_w / HAIR_SPRITE_WIDTH;
                            let scale_y = head_full_tex_h * hair_uv_h / HAIR_SPRITE_HEIGHT;
                            let offset_x = hair_uv_x - head_uv_x * scale_x - delta_x * hair_uv_w / HAIR_SPRITE_WIDTH;
                            let offset_y = hair_uv_y - head_uv_y * scale_y - delta_y * hair_uv_h / HAIR_SPRITE_HEIGHT;

                            // Set up shader
                            let material = self.head_hair_material.as_ref().unwrap();
                            material.set_texture("HairTexture", hair_texture.clone());
                            material.set_uniform("HairUvTransform", [offset_x, offset_y, scale_x, scale_y]);
                            material.set_uniform("Tint", [1.0f32, 1.0f32, 1.0f32, 1.0f32]);
                            gl_use_material(material);

                            // Draw head with shader active
                            let scaled_head_width = HEAD_SPRITE_WIDTH * zoom;
                            let scaled_head_height = HEAD_SPRITE_HEIGHT * zoom;
                            let head_draw_x = draw_x + head_pos_x * zoom;
                            let head_draw_y = draw_y + head_pos_y * zoom;

                            draw_texture_ex(
                                head_texture,
                                head_draw_x,
                                head_draw_y,
                                tint,
                                DrawTextureParams {
                                    source: Some(Rect::new(head_atlas_x + head_src_x, head_atlas_y, HEAD_SPRITE_WIDTH, HEAD_SPRITE_HEIGHT)),
                                    dest_size: Some(Vec2::new(scaled_head_width, scaled_head_height)),
                                    flip_x: head_frame.flip_h,
                                    ..Default::default()
                                },
                            );

                            gl_use_default_material();
                        }
                    } else {
                        // No hair, just draw head normally
                        let anim_frame = player.animation.frame as u32;
                        let head_frame = get_head_frame(player.animation.direction);
                        let (head_pos_offset_x, head_pos_offset_y) = get_head_offset(player.animation.state, player.animation.direction, anim_frame, player_gender);
                        let (head_atlas_x, head_atlas_y) = head_offset.unwrap_or((0.0, 0.0));
                        let head_src_x = head_frame.frame as f32 * HEAD_SPRITE_WIDTH;
                        let scaled_head_width = HEAD_SPRITE_WIDTH * zoom;
                        let scaled_head_height = HEAD_SPRITE_HEIGHT * zoom;
                        let head_draw_x = draw_x + head_pos_offset_x * zoom;
                        let head_draw_y = draw_y + head_pos_offset_y * zoom;

                        draw_texture_ex(
                            head_texture,
                            head_draw_x,
                            head_draw_y,
                            tint,
                            DrawTextureParams {
                                source: Some(Rect::new(head_atlas_x + head_src_x, head_atlas_y, HEAD_SPRITE_WIDTH, HEAD_SPRITE_HEIGHT)),
                                dest_size: Some(Vec2::new(scaled_head_width, scaled_head_height)),
                                flip_x: head_frame.flip_h,
                                ..Default::default()
                            },
                        );
                    }
                } else {
                    // No shader available, draw hair then head (hair will show through transparent areas)
                    // Draw hair first
                    if let (Some(style), Some(color)) = (player.hair_style, player.hair_color) {
                        let hair_key = format!("{}_{}", player.gender, style);
                        if let Some((hair_tex, hair_atlas_offset)) = self.hair_sprites.get(&hair_key) {
                            let is_back = matches!(player.animation.direction, Direction::Up | Direction::Left);
                            let frame_index = color * 2 + if is_back { 1 } else { 0 };
                            let (hair_atlas_x, hair_atlas_y) = hair_atlas_offset.unwrap_or((0.0, 0.0));
                            let hair_src_x = hair_atlas_x + frame_index as f32 * HAIR_SPRITE_WIDTH;
                            let scaled_hair_width = HAIR_SPRITE_WIDTH * zoom;
                            let scaled_hair_height = HAIR_SPRITE_HEIGHT * zoom;

                            // Calculate hair offsets using gender-aware function
                            let anim_frame = player.animation.frame as u32;
                            let (hair_pos_offset_x, hair_pos_offset_y) = get_hair_offset(
                                player.animation.state,
                                player.animation.direction,
                                anim_frame,
                                player_gender,
                                coords.flip_h,
                            );

                            let hair_draw_x = draw_x + (scaled_sprite_width - scaled_hair_width) / 2.0 + hair_pos_offset_x * zoom;
                            let hair_draw_y = draw_y + hair_pos_offset_y * zoom;

                            draw_texture_ex(
                                hair_tex,
                                hair_draw_x,
                                hair_draw_y,
                                tint,
                                DrawTextureParams {
                                    source: Some(Rect::new(hair_src_x, hair_atlas_y, HAIR_SPRITE_WIDTH, HAIR_SPRITE_HEIGHT)),
                                    dest_size: Some(Vec2::new(scaled_hair_width, scaled_hair_height)),
                                    flip_x: coords.flip_h,
                                    ..Default::default()
                                },
                            );
                        }
                    }

                    // Then draw head on top
                    let anim_frame = player.animation.frame as u32;
                    let head_frame = get_head_frame(player.animation.direction);
                    let (head_pos_offset_x, head_pos_offset_y) = get_head_offset(player.animation.state, player.animation.direction, anim_frame, player_gender);
                    let (head_atlas_x, head_atlas_y) = head_offset.unwrap_or((0.0, 0.0));
                    let head_src_x = head_atlas_x + head_frame.frame as f32 * HEAD_SPRITE_WIDTH;
                    let scaled_head_width = HEAD_SPRITE_WIDTH * zoom;
                    let scaled_head_height = HEAD_SPRITE_HEIGHT * zoom;
                    let head_draw_x = draw_x + head_pos_offset_x * zoom;
                    let head_draw_y = draw_y + head_pos_offset_y * zoom;

                    draw_texture_ex(
                        head_texture,
                        head_draw_x,
                        head_draw_y,
                        tint,
                        DrawTextureParams {
                            source: Some(Rect::new(head_src_x, head_atlas_y, HEAD_SPRITE_WIDTH, HEAD_SPRITE_HEIGHT)),
                            dest_size: Some(Vec2::new(scaled_head_width, scaled_head_height)),
                            flip_x: head_frame.flip_h,
                            ..Default::default()
                        },
                    );
                }
            }
            // Hair without head equipment is drawn after body armor (see below)

            // Draw equipment overlay (body armor)
            if let Some(ref body_item_id) = player.equipped_body {
                if let Some((body_texture, body_atlas_offset)) = self.equipment_sprites.get(body_item_id) {
                    // Check if this is a new-style single-row body armor sprite (width > height * 2)
                    // Body armor sprites are wider (16 frames) so use a more aggressive ratio check
                    let (body_w, body_h) = self.equipment_sprites.get_dimensions(body_item_id)
                        .unwrap_or((body_texture.width(), body_texture.height()));
                    let is_single_row = body_w > body_h * 2.0;
                    let (body_atlas_x, body_atlas_y) = body_atlas_offset.unwrap_or((0.0, 0.0));

                    if is_single_row {
                        // New single-row body armor format
                        let anim_frame = player.animation.frame as u32;
                        let armor_frame = get_body_armor_frame(player.animation.state, player.animation.direction, anim_frame);
                        let (armor_offset_x, armor_offset_y) = get_body_armor_offset(player.animation.state, player.animation.direction, anim_frame, player_gender);

                        let armor_src_x = body_atlas_x + armor_frame.frame as f32 * BODY_ARMOR_SPRITE_WIDTH;
                        let scaled_armor_width = BODY_ARMOR_SPRITE_WIDTH * zoom;
                        let scaled_armor_height = BODY_ARMOR_SPRITE_HEIGHT * zoom;

                        let armor_draw_x = draw_x + armor_offset_x * zoom;
                        let armor_draw_y = draw_y + armor_offset_y * zoom;

                        draw_texture_ex(
                            body_texture,
                            armor_draw_x,
                            armor_draw_y,
                            tint,
                            DrawTextureParams {
                                source: Some(Rect::new(armor_src_x, body_atlas_y, BODY_ARMOR_SPRITE_WIDTH, BODY_ARMOR_SPRITE_HEIGHT)),
                                dest_size: Some(Vec2::new(scaled_armor_width, scaled_armor_height)),
                                flip_x: armor_frame.flip_h,
                                ..Default::default()
                            },
                        );
                    } else {
                        // Old grid-style body armor format (matches player sprite sheet layout)
                        draw_texture_ex(
                            body_texture,
                            draw_x,
                            draw_y,
                            tint, // Same tint as player
                            DrawTextureParams {
                                source: Some(Rect::new(body_atlas_x + src_x, body_atlas_y + src_y, src_w, src_h)),
                                dest_size: Some(Vec2::new(scaled_sprite_width, scaled_sprite_height)),
                                flip_x: coords.flip_h,
                                ..Default::default()
                            },
                        );
                    }
                }
            }

            // Draw hair on top of body armor (when no head equipment)
            if player.equipped_head.is_none() {
                if let (Some(style), Some(color)) = (player.hair_style, player.hair_color) {
                    let hair_key = format!("{}_{}", player.gender, style);
                    if let Some((hair_tex, hair_atlas_offset)) = self.hair_sprites.get(&hair_key) {
                        let is_back = matches!(player.animation.direction, Direction::Up | Direction::Left);
                        let frame_index = color * 2 + if is_back { 1 } else { 0 };
                        let (hair_atlas_x, hair_atlas_y) = hair_atlas_offset.unwrap_or((0.0, 0.0));
                        let hair_src_x = hair_atlas_x + frame_index as f32 * HAIR_SPRITE_WIDTH;
                        let scaled_hair_width = HAIR_SPRITE_WIDTH * zoom;
                        let scaled_hair_height = HAIR_SPRITE_HEIGHT * zoom;

                        // Calculate hair offsets using gender-aware function
                        let anim_frame = player.animation.frame as u32;
                        let (hair_pos_offset_x, hair_pos_offset_y) = get_hair_offset(
                            player.animation.state,
                            player.animation.direction,
                            anim_frame,
                            player_gender,
                            coords.flip_h,
                        );

                        let hair_draw_x = draw_x + (scaled_sprite_width - scaled_hair_width) / 2.0 + hair_pos_offset_x * zoom;
                        let hair_draw_y = draw_y + hair_pos_offset_y * zoom;

                        draw_texture_ex(
                            hair_tex,
                            hair_draw_x,
                            hair_draw_y,
                            tint,
                            DrawTextureParams {
                                source: Some(Rect::new(hair_src_x, hair_atlas_y, HAIR_SPRITE_WIDTH, HAIR_SPRITE_HEIGHT)),
                                dest_size: Some(Vec2::new(scaled_hair_width, scaled_hair_height)),
                                flip_x: coords.flip_h,
                                ..Default::default()
                            },
                        );
                    }
                }
            }

            // Draw equipment overlay (boots)
            if let Some(ref feet_item_id) = player.equipped_feet {
                if let Some((feet_texture, feet_atlas_offset)) = self.equipment_sprites.get(feet_item_id) {
                    // Check if this is a new-style single-row boot sprite (width > height)
                    let (feet_w, feet_h) = self.equipment_sprites.get_dimensions(feet_item_id)
                        .unwrap_or((feet_texture.width(), feet_texture.height()));
                    let is_single_row = feet_w > feet_h;
                    let (feet_atlas_x, feet_atlas_y) = feet_atlas_offset.unwrap_or((0.0, 0.0));

                    if is_single_row {
                        // New single-row boot format
                        let anim_frame = player.animation.frame as u32;
                        let boot_frame = get_boot_frame(player.animation.state, player.animation.direction, anim_frame);
                        let (boot_offset_x, boot_offset_y) = get_boot_offset(player.animation.state, player.animation.direction, anim_frame, player_gender);

                        let boot_src_x = feet_atlas_x + boot_frame.frame as f32 * BOOT_SPRITE_WIDTH;
                        let scaled_boot_width = BOOT_SPRITE_WIDTH * zoom;
                        let scaled_boot_height = BOOT_SPRITE_HEIGHT * zoom;

                        let boot_draw_x = draw_x + boot_offset_x * zoom;
                        let boot_draw_y = draw_y + boot_offset_y * zoom;

                        draw_texture_ex(
                            feet_texture,
                            boot_draw_x,
                            boot_draw_y,
                            tint,
                            DrawTextureParams {
                                source: Some(Rect::new(boot_src_x, feet_atlas_y, BOOT_SPRITE_WIDTH, BOOT_SPRITE_HEIGHT)),
                                dest_size: Some(Vec2::new(scaled_boot_width, scaled_boot_height)),
                                flip_x: boot_frame.flip_h,
                                ..Default::default()
                            },
                        );
                    } else {
                        // Old grid-style boot format (matches player sprite sheet layout)
                        draw_texture_ex(
                            feet_texture,
                            draw_x,
                            draw_y,
                            tint,
                            DrawTextureParams {
                                source: Some(Rect::new(feet_atlas_x + src_x, feet_atlas_y + src_y, src_w, src_h)),
                                dest_size: Some(Vec2::new(scaled_sprite_width, scaled_sprite_height)),
                                flip_x: coords.flip_h,
                                ..Default::default()
                            },
                        );
                    }
                }
            }

            // Draw back slot equipment (quiver, shield, etc.)
            if let Some(ref back_item_id) = player.equipped_back {
                if let Some((back_texture, back_atlas_offset)) = self.equipment_sprites.get(back_item_id) {
                    // Detect sprite type by dimensions:
                    // - 16-frame offhand (shield): width > height * 8 (very wide strip)
                    // - 2-frame static back (quiver): width < height * 4 (narrow strip)
                    let (back_w, back_h) = self.equipment_sprites.get_dimensions(back_item_id)
                        .unwrap_or((back_texture.width(), back_texture.height()));
                    let is_offhand = back_w > back_h * 8.0;
                    let (back_atlas_x, back_atlas_y) = back_atlas_offset.unwrap_or((0.0, 0.0));

                    if is_offhand {
                        // 16-frame offhand item (shield)
                        let anim_frame = player.animation.frame as u32;
                        let offhand_frame = get_offhand_frame(player.animation.state, player.animation.direction, anim_frame);
                        let (offhand_offset_x, offhand_offset_y) = get_offhand_offset(player.animation.state, player.animation.direction, anim_frame, player_gender);

                        let offhand_src_x = back_atlas_x + offhand_frame.frame as f32 * OFFHAND_SPRITE_WIDTH;
                        let scaled_offhand_width = OFFHAND_SPRITE_WIDTH * zoom;
                        let scaled_offhand_height = OFFHAND_SPRITE_HEIGHT * zoom;

                        let offhand_draw_x = draw_x + offhand_offset_x * zoom;
                        let offhand_draw_y = draw_y + offhand_offset_y * zoom;

                        draw_texture_ex(
                            back_texture,
                            offhand_draw_x,
                            offhand_draw_y,
                            tint,
                            DrawTextureParams {
                                source: Some(Rect::new(offhand_src_x, back_atlas_y, OFFHAND_SPRITE_WIDTH, OFFHAND_SPRITE_HEIGHT)),
                                dest_size: Some(Vec2::new(scaled_offhand_width, scaled_offhand_height)),
                                flip_x: offhand_frame.flip_h,
                                ..Default::default()
                            },
                        );
                    } else {
                        // 2-frame static back item (quiver, cape)
                        let anim_frame = player.animation.frame as u32;
                        let back_frame = get_back_static_frame(player.animation.direction);

                        // Only render here if visible and NOT rendering behind player
                        // (behind rendering happens before player sprite)
                        if back_frame.visible && !back_frame.render_behind {
                            let (back_pos_offset_x, back_pos_offset_y) = get_back_static_offset(player.animation.state, player.animation.direction, anim_frame, player_gender);

                            let back_src_x = back_atlas_x + back_frame.frame as f32 * BACK_STATIC_SPRITE_WIDTH;
                            let scaled_back_width = BACK_STATIC_SPRITE_WIDTH * zoom;
                            let scaled_back_height = BACK_STATIC_SPRITE_HEIGHT * zoom;

                            let back_draw_x = draw_x + back_pos_offset_x * zoom;
                            let back_draw_y = draw_y + back_pos_offset_y * zoom;

                            draw_texture_ex(
                                back_texture,
                                back_draw_x,
                                back_draw_y,
                                tint,
                                DrawTextureParams {
                                    source: Some(Rect::new(back_src_x, back_atlas_y, BACK_STATIC_SPRITE_WIDTH, BACK_STATIC_SPRITE_HEIGHT)),
                                    dest_size: Some(Vec2::new(scaled_back_width, scaled_back_height)),
                                    flip_x: back_frame.flip_h,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }
            }

            // Draw weapon over-layer (after equipment, for attack frame 2 front overlay)
            if let Some((weapon_sprite, atlas_offset, ref weapon_frame, offset_x, offset_y, _, _)) = weapon_info {
                if let Some(frame_over) = weapon_frame.frame_over {
                    let (atlas_x, atlas_y) = atlas_offset.unwrap_or((0.0, 0.0));
                    let weapon_src_x = atlas_x + frame_over as f32 * wf_width;
                    let weapon_draw_x = draw_x + offset_x * zoom;
                    let weapon_draw_y = draw_y + offset_y * zoom;

                    draw_texture_ex(
                        weapon_sprite,
                        weapon_draw_x,
                        weapon_draw_y,
                        tint,
                        DrawTextureParams {
                            source: Some(Rect::new(weapon_src_x, atlas_y, wf_width, wf_height)),
                            dest_size: Some(Vec2::new(scaled_weapon_width, scaled_weapon_height)),
                            flip_x: weapon_frame.flip_h,
                            ..Default::default()
                        },
                    );
                }
            }
        } else {
            // Fallback: colored circle
            let base_color = if is_local {
                self.local_player_color
            } else {
                self.player_color
            };
            let color = Color::from_rgba(
                (base_color.r * 255.0) as u8,
                (base_color.g * 255.0) as u8,
                (base_color.b * 255.0) as u8,
                alpha,
            );

            let radius = 12.0 * zoom;
            draw_circle(screen_x, screen_y - radius, radius, color);

            // Direction indicator
            let (dx, dy) = player.direction.to_unit_vector();
            let indicator_len = 15.0 * zoom;
            draw_line(
                screen_x,
                screen_y - radius,
                screen_x + dx * indicator_len,
                screen_y - radius + dy * indicator_len * 0.5, // Flatten for isometric
                2.0 * zoom,
                WHITE,
            );
        }

        // Player name (positioned just above head) - only show when hovered or selected
        let has_sprite = self.get_player_sprite(&player.gender, &player.skin).is_some();
        let name_y_offset = if has_sprite { scaled_sprite_height - 8.0 * zoom } else { 24.0 * zoom };

        let show_name = is_selected || is_hovered;
        // Name tag drawing is deferred to render_name_tags() so it appears above all map elements

        // Health bar - only show within 3 seconds of taking damage (and when not at full HP)
        let current_time = macroquad::time::get_time();
        let time_since_damage = current_time - player.last_damage_time;
        let show_health_bar = player.hp < player.max_hp && time_since_damage < 3.0;

        if show_health_bar {
            let bar_width = 32.0 * zoom;
            let bar_height = 6.0 * zoom;
            let bar_x = screen_x - bar_width / 2.0;
            // Position health bar where name would be if name isn't showing, otherwise above the name
            let bar_y = if show_name {
                screen_y - name_y_offset - 16.0 * zoom
            } else {
                screen_y - name_y_offset
            };
            let hp_ratio = player.hp as f32 / player.max_hp.max(1) as f32;

            self.draw_entity_health_bar(bar_x, bar_y, bar_width, bar_height, hp_ratio, zoom);
        }
    }

    /// Renders a semi-transparent silhouette of the player that's always visible
    /// This provides visual feedback when the player is behind tall objects like trees
    fn render_player_silhouette(&self, player: &Player, camera: &Camera) {
        // Don't show silhouette for dead players
        if player.is_dead {
            return;
        }

        let (screen_x, screen_y) = world_to_screen(player.x, player.y, camera);
        let zoom = camera.zoom;

        let scaled_sprite_width = SPRITE_WIDTH * zoom;
        let scaled_sprite_height = SPRITE_HEIGHT * zoom;

        // Subtle semi-transparent tint (~20% opacity)
        let silhouette_tint = Color::from_rgba(255, 255, 255, 50);

        if let Some((player_texture, player_offset)) = self.get_player_sprite(&player.gender, &player.skin) {
            let coords = player.animation.get_sprite_coords();
            let (src_x, src_y, src_w, src_h) = coords.to_source_rect();
            let (player_atlas_x, player_atlas_y) = player_offset.unwrap_or((0.0, 0.0));

            let draw_x = screen_x - scaled_sprite_width / 2.0;
            let draw_y = screen_y - scaled_sprite_height + 8.0 * zoom;

            // Get player gender for gender-specific offsets
            let player_gender = Gender::from_str(&player.gender);

            // Calculate weapon frame info if weapon is equipped (hidden when sitting)
            let is_sitting_sil = matches!(player.animation.state,
                crate::render::animation::AnimationState::SittingChair | crate::render::animation::AnimationState::SittingGround);
            let weapon_info = player.equipped_weapon.as_ref().filter(|_| !is_sitting_sil).and_then(|weapon_id| {
                self.weapon_sprites.get(weapon_id).map(|(tex, atlas_offset)| {
                    let anim_frame = player.animation.frame as u32;
                    let weapon_frame = get_weapon_frame(player.animation.state, player.animation.direction, anim_frame);
                    let (offset_x, offset_y) = get_weapon_offset(player.animation.state, player.animation.direction, anim_frame, player_gender);
                    let (fw, fh) = self.weapon_frame_sizes.get(weapon_id)
                        .copied()
                        .unwrap_or((WEAPON_SPRITE_WIDTH, WEAPON_SPRITE_HEIGHT));
                    (tex, atlas_offset, weapon_frame, offset_x, offset_y, fw, fh)
                })
            });

            let (scaled_weapon_width, scaled_weapon_height, wf_width, wf_height) = weapon_info.as_ref()
                .map(|(_, _, _, _, _, fw, fh)| (*fw * zoom, *fh * zoom, *fw, *fh))
                .unwrap_or((WEAPON_SPRITE_WIDTH * zoom, WEAPON_SPRITE_HEIGHT * zoom, WEAPON_SPRITE_WIDTH, WEAPON_SPRITE_HEIGHT));

            // Draw weapon under-layer (before player sprite)
            if let Some((weapon_sprite, atlas_offset, ref weapon_frame, offset_x, offset_y, _, _)) = weapon_info {
                let (atlas_x, atlas_y) = atlas_offset.unwrap_or((0.0, 0.0));
                let weapon_src_x = atlas_x + weapon_frame.frame_under as f32 * wf_width;
                let weapon_draw_x = draw_x + offset_x * zoom;
                let weapon_draw_y = draw_y + offset_y * zoom;

                draw_texture_ex(
                    weapon_sprite,
                    weapon_draw_x,
                    weapon_draw_y,
                    silhouette_tint,
                    DrawTextureParams {
                        source: Some(Rect::new(weapon_src_x, atlas_y, wf_width, wf_height)),
                        dest_size: Some(Vec2::new(scaled_weapon_width, scaled_weapon_height)),
                        flip_x: weapon_frame.flip_h,
                        ..Default::default()
                    },
                );
            }

            // Draw player base sprite (skip if body armor or head slot equipped to avoid transparency stacking)
            if player.equipped_body.is_none() && player.equipped_head.is_none() {
                draw_texture_ex(
                    player_texture,
                    draw_x,
                    draw_y,
                    silhouette_tint,
                    DrawTextureParams {
                        source: Some(Rect::new(player_atlas_x + src_x, player_atlas_y + src_y, src_w, src_h)),
                        dest_size: Some(Vec2::new(scaled_sprite_width, scaled_sprite_height)),
                        flip_x: coords.flip_h,
                        ..Default::default()
                    },
                );
            }

            // Hair silhouette is drawn after body armor silhouette (see below)

            // Draw equipment silhouette (body armor)
            if let Some(ref body_item_id) = player.equipped_body {
                if let Some((body_texture, body_atlas_offset)) = self.equipment_sprites.get(body_item_id) {
                    let (body_w, body_h) = self.equipment_sprites.get_dimensions(body_item_id)
                        .unwrap_or((body_texture.width(), body_texture.height()));
                    let is_single_row = body_w > body_h * 2.0;
                    let (body_atlas_x, body_atlas_y) = body_atlas_offset.unwrap_or((0.0, 0.0));

                    if is_single_row {
                        // New single-row body armor format
                        let anim_frame = player.animation.frame as u32;
                        let armor_frame = get_body_armor_frame(player.animation.state, player.animation.direction, anim_frame);
                        let (armor_offset_x, armor_offset_y) = get_body_armor_offset(player.animation.state, player.animation.direction, anim_frame, player_gender);

                        let armor_src_x = body_atlas_x + armor_frame.frame as f32 * BODY_ARMOR_SPRITE_WIDTH;
                        let scaled_armor_width = BODY_ARMOR_SPRITE_WIDTH * zoom;
                        let scaled_armor_height = BODY_ARMOR_SPRITE_HEIGHT * zoom;

                        let armor_draw_x = draw_x + armor_offset_x * zoom;
                        let armor_draw_y = draw_y + armor_offset_y * zoom;

                        draw_texture_ex(
                            body_texture,
                            armor_draw_x,
                            armor_draw_y,
                            silhouette_tint,
                            DrawTextureParams {
                                source: Some(Rect::new(armor_src_x, body_atlas_y, BODY_ARMOR_SPRITE_WIDTH, BODY_ARMOR_SPRITE_HEIGHT)),
                                dest_size: Some(Vec2::new(scaled_armor_width, scaled_armor_height)),
                                flip_x: armor_frame.flip_h,
                                ..Default::default()
                            },
                        );
                    } else {
                        // Old grid-style body armor format
                        draw_texture_ex(
                            body_texture,
                            draw_x,
                            draw_y,
                            silhouette_tint,
                            DrawTextureParams {
                                source: Some(Rect::new(body_atlas_x + src_x, body_atlas_y + src_y, src_w, src_h)),
                                dest_size: Some(Vec2::new(scaled_sprite_width, scaled_sprite_height)),
                                flip_x: coords.flip_h,
                                ..Default::default()
                            },
                        );
                    }
                }
            }

            // Draw hair silhouette on top of body armor (when no head equipment)
            if player.equipped_head.is_none() {
                if let (Some(style), Some(color)) = (player.hair_style, player.hair_color) {
                    let hair_key = format!("{}_{}", player.gender, style);
                    if let Some((hair_tex, hair_atlas_offset)) = self.hair_sprites.get(&hair_key) {
                        let is_back = matches!(player.animation.direction, Direction::Up | Direction::Left);
                        let frame_index = color * 2 + if is_back { 1 } else { 0 };
                        let (hair_atlas_x, hair_atlas_y) = hair_atlas_offset.unwrap_or((0.0, 0.0));
                        let hair_src_x = hair_atlas_x + frame_index as f32 * HAIR_SPRITE_WIDTH;

                        let scaled_hair_width = HAIR_SPRITE_WIDTH * zoom;
                        let scaled_hair_height = HAIR_SPRITE_HEIGHT * zoom;

                        // Calculate hair offsets using gender-aware function
                        let anim_frame = player.animation.frame as u32;
                        let (hair_pos_offset_x, hair_pos_offset_y) = get_hair_offset(
                            player.animation.state,
                            player.animation.direction,
                            anim_frame,
                            player_gender,
                            coords.flip_h,
                        );
                        let hair_draw_x = draw_x + (scaled_sprite_width - scaled_hair_width) / 2.0 + hair_pos_offset_x * zoom;
                        let hair_draw_y = draw_y + hair_pos_offset_y * zoom;

                        draw_texture_ex(
                            hair_tex,
                            hair_draw_x,
                            hair_draw_y,
                            silhouette_tint,
                            DrawTextureParams {
                                source: Some(Rect::new(hair_src_x, hair_atlas_y, HAIR_SPRITE_WIDTH, HAIR_SPRITE_HEIGHT)),
                                dest_size: Some(Vec2::new(scaled_hair_width, scaled_hair_height)),
                                flip_x: coords.flip_h,
                                ..Default::default()
                            },
                        );
                    }
                }
            }

            // Draw equipment silhouette (boots)
            if let Some(ref feet_item_id) = player.equipped_feet {
                if let Some((feet_texture, feet_atlas_offset)) = self.equipment_sprites.get(feet_item_id) {
                    let (feet_w, feet_h) = self.equipment_sprites.get_dimensions(feet_item_id)
                        .unwrap_or((feet_texture.width(), feet_texture.height()));
                    let is_single_row = feet_w > feet_h;
                    let (feet_atlas_x, feet_atlas_y) = feet_atlas_offset.unwrap_or((0.0, 0.0));

                    if is_single_row {
                        // New single-row boot format
                        let anim_frame = player.animation.frame as u32;
                        let boot_frame = get_boot_frame(player.animation.state, player.animation.direction, anim_frame);
                        let (boot_offset_x, boot_offset_y) = get_boot_offset(player.animation.state, player.animation.direction, anim_frame, player_gender);

                        let boot_src_x = feet_atlas_x + boot_frame.frame as f32 * BOOT_SPRITE_WIDTH;
                        let scaled_boot_width = BOOT_SPRITE_WIDTH * zoom;
                        let scaled_boot_height = BOOT_SPRITE_HEIGHT * zoom;

                        let boot_draw_x = draw_x + boot_offset_x * zoom;
                        let boot_draw_y = draw_y + boot_offset_y * zoom;

                        draw_texture_ex(
                            feet_texture,
                            boot_draw_x,
                            boot_draw_y,
                            silhouette_tint,
                            DrawTextureParams {
                                source: Some(Rect::new(boot_src_x, feet_atlas_y, BOOT_SPRITE_WIDTH, BOOT_SPRITE_HEIGHT)),
                                dest_size: Some(Vec2::new(scaled_boot_width, scaled_boot_height)),
                                flip_x: boot_frame.flip_h,
                                ..Default::default()
                            },
                        );
                    } else {
                        // Old grid-style boot format
                        draw_texture_ex(
                            feet_texture,
                            draw_x,
                            draw_y,
                            silhouette_tint,
                            DrawTextureParams {
                                source: Some(Rect::new(feet_atlas_x + src_x, feet_atlas_y + src_y, src_w, src_h)),
                                dest_size: Some(Vec2::new(scaled_sprite_width, scaled_sprite_height)),
                                flip_x: coords.flip_h,
                                ..Default::default()
                            },
                        );
                    }
                }
            }


            // Draw weapon over-layer (after equipment)
            if let Some((weapon_sprite, atlas_offset, ref weapon_frame, offset_x, offset_y, _, _)) = weapon_info {
                if let Some(frame_over) = weapon_frame.frame_over {
                    let (atlas_x, atlas_y) = atlas_offset.unwrap_or((0.0, 0.0));
                    let weapon_src_x = atlas_x + frame_over as f32 * wf_width;
                    let weapon_draw_x = draw_x + offset_x * zoom;
                    let weapon_draw_y = draw_y + offset_y * zoom;

                    draw_texture_ex(
                        weapon_sprite,
                        weapon_draw_x,
                        weapon_draw_y,
                        silhouette_tint,
                        DrawTextureParams {
                            source: Some(Rect::new(weapon_src_x, atlas_y, wf_width, wf_height)),
                            dest_size: Some(Vec2::new(scaled_weapon_width, scaled_weapon_height)),
                            flip_x: weapon_frame.flip_h,
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }

    fn render_npc(&self, npc: &Npc, is_selected: bool, is_hovered: bool, camera: &Camera) {
        let (screen_x, screen_y) = world_to_screen(npc.x, npc.y, camera);
        let zoom = camera.zoom;

        // Don't render if death animation is complete
        if npc.is_death_animation_complete() {
            return;
        }

        // Get death tint color if dying, otherwise white
        let tint_color = npc.get_death_color().unwrap_or(WHITE);

        // Selection highlight (draw first, behind NPC) - skip while dying
        if is_selected && npc.death_timer.is_none() {
            self.render_tile_selection(npc.x, npc.y, camera);
        }

        // Name color based on NPC type
        let name_color = if npc.is_hostile() {
            Color::from_rgba(255, 150, 150, 255) // Red for hostile
        } else if npc.is_quest_giver {
            Color::from_rgba(150, 220, 255, 255) // Cyan for quest givers
        } else if npc.is_merchant {
            Color::from_rgba(150, 255, 150, 255) // Light green for merchants
        } else {
            Color::from_rgba(255, 255, 255, 255) // White for other friendly NPCs
        };

        // Try to render with sprite, fall back to ellipse
        let sprite_height = if let Some((npc_texture, npc_atlas_offset)) = self.npc_sprites.get(&npc.entity_type) {
            // Auto-detect frame size from texture (16 frames per sheet)
            let (tex_w, tex_h) = self.npc_sprites.get_dimensions(&npc.entity_type)
                .unwrap_or((npc_texture.width(), npc_texture.height()));
            let frame_width = tex_w / 16.0;
            let frame_height = tex_h;
            let (npc_atlas_x, npc_atlas_y) = npc_atlas_offset.unwrap_or((0.0, 0.0));

            // Get current frame based on animation state and direction
            let frame_index = npc.animation.get_frame_index(npc.direction);
            let src_x = npc_atlas_x + frame_index as f32 * frame_width;

            // Flip horizontally for Right/Left directions
            let flip_x = NpcAnimation::should_flip(npc.direction);

            // Position sprite centered horizontally, feet at world position
            // Round to whole pixels to avoid blurry rendering from subpixel positioning
            let scaled_width = (frame_width * zoom).round();
            let scaled_height = (frame_height * zoom).round();
            let draw_x = (screen_x - scaled_width / 2.0).round();
            let draw_y = (screen_y - scaled_height + 4.0 * zoom).round();

            // Draw shadow
            let shadow_scale = (frame_width / 50.0).clamp(0.5, 2.0);
            draw_ellipse(
                screen_x,
                screen_y,
                16.0 * shadow_scale * zoom,
                6.0 * shadow_scale * zoom,
                0.0,
                Color::from_rgba(0, 0, 0, 60),
            );

            draw_texture_ex(
                npc_texture,
                draw_x,
                draw_y,
                tint_color,
                DrawTextureParams {
                    source: Some(Rect::new(src_x, npc_atlas_y, frame_width, frame_height)),
                    dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                    flip_x,
                    ..Default::default()
                },
            );

            scaled_height
        } else {
            // Fallback: colored ellipse rendering
            let (mut base_color, mut highlight_color) = if npc.is_hostile() {
                (
                    Color::from_rgba(80, 180, 80, 255),
                    Color::from_rgba(120, 220, 120, 255),
                )
            } else {
                (
                    Color::from_rgba(100, 120, 200, 255),
                    Color::from_rgba(140, 160, 240, 255),
                )
            };

            // Apply death tint to ellipse colors
            base_color.r *= tint_color.r;
            base_color.g *= tint_color.g;
            base_color.b *= tint_color.b;
            base_color.a *= tint_color.a;
            highlight_color.r *= tint_color.r;
            highlight_color.g *= tint_color.g;
            highlight_color.b *= tint_color.b;
            highlight_color.a *= tint_color.a;

            // Only animate wobble for hostile/moving NPCs; static NPCs (altars etc.) stay still
            let wobble = if npc.is_hostile() {
                (macroquad::time::get_time() * 4.0 + npc.animation.frame as f64).sin() as f32
            } else {
                0.0
            };
            let radius = (10.0 + wobble * 1.5) * zoom;
            let height_offset = (8.0 + wobble * 2.0) * zoom;

            // Draw shadow
            draw_ellipse(screen_x, screen_y, 16.0 * zoom, 6.0 * zoom, 0.0, Color::from_rgba(0, 0, 0, 60));

            // Draw NPC body (oval blob)
            draw_ellipse(screen_x, screen_y - height_offset, radius, radius * 0.7, 0.0, base_color);

            // Highlight
            draw_ellipse(screen_x - 3.0 * zoom, screen_y - height_offset - 2.0 * zoom, radius * 0.3, radius * 0.2, 0.0, highlight_color);

            (height_offset + radius) * 2.0
        };

        // Skip UI elements (name, health bar, icons) while dying
        if npc.death_timer.is_some() {
            return;
        }

        // Top of NPC for UI elements
        let top_y = screen_y - sprite_height + 4.0 * zoom;

        // Determine icon coords for friendly NPCs (quest givers only)
        let icon_coords: Option<(u32, u32)> = if !npc.is_hostile() && npc.is_quest_giver {
            Some((8, 3))  // Quest giver icon
        } else {
            None
        };

        // Floating icon indicator - only when NOT hovered (when hovered, icon is in name bar)
        if !is_hovered && !is_selected {
            if let (Some((icon_col, icon_row)), Some(ref texture)) = (icon_coords, &self.ui_icons) {
                let icon_size = 24.0;
                let time = macroquad::time::get_time();

                // Use NPC position as offset so icons don't animate in sync
                let phase_offset = (npc.x + npc.y * 1.7) as f64;

                // Pulsing transparency (2 second cycle, 80-100% opacity)
                let alpha_pulse = ((time * 3.14 + phase_offset).sin() * 0.5 + 0.5) as f32;
                let mut alpha = (204.0 + alpha_pulse * 51.0) as u8; // 204-255 (80-100%)

                // Fade icon out when speech bubble appears, fade back in when it disappears
                if let Some((_, bubble_time)) = &npc.speech_bubble {
                    let age = (time - bubble_time) as f32;
                    let icon_alpha = if age < 0.5 {
                        // Fade out over first 0.5s as bubble appears
                        ((1.0 - age / 0.5) * 255.0) as u8
                    } else if age > 4.0 && age <= 5.0 {
                        // Fade back in during last second as bubble fades out
                        (((age - 4.0)) * 255.0) as u8
                    } else if age > 5.0 {
                        255 // Fully visible after bubble is gone
                    } else {
                        0 // Hidden while bubble is showing
                    };
                    alpha = alpha.min(icon_alpha);
                }

                let icon_x = screen_x - (icon_size * zoom) / 2.0;
                let icon_y = top_y - 20.0 * zoom;

                let src_rect = Rect::new(
                    icon_col as f32 * icon_size,
                    icon_row as f32 * icon_size,
                    icon_size,
                    icon_size,
                );

                draw_texture_ex(
                    texture,
                    icon_x,
                    icon_y,
                    Color::from_rgba(255, 255, 255, alpha),
                    DrawTextureParams {
                        source: Some(src_rect),
                        dest_size: Some(Vec2::new(icon_size * zoom, icon_size * zoom)),
                        ..Default::default()
                    },
                );
            }
        }

        // NPC name with level - only show when hovered or selected
        let show_name = is_selected || is_hovered;
        // Name tag drawing is deferred to render_name_tags() so it appears above all map elements

        // Health bar - only show within 3 seconds of taking damage (and when not at full HP)
        let current_time = macroquad::time::get_time();
        let time_since_damage = current_time - npc.last_damage_time;
        let show_health_bar = npc.hp < npc.max_hp && time_since_damage < 3.0;

        if show_health_bar {
            let bar_width = 30.0 * zoom;
            let bar_height = 5.0 * zoom;
            let bar_x = screen_x - bar_width / 2.0;
            // Position health bar where name would be if name isn't showing, otherwise above the name
            let bar_y = if show_name {
                top_y - 20.0 * zoom
            } else {
                top_y - 5.0 * zoom
            };
            let hp_ratio = npc.hp as f32 / npc.max_hp.max(1) as f32;

            self.draw_entity_health_bar(bar_x, bar_y, bar_width, bar_height, hp_ratio, zoom);
        }
    }

    fn render_ground_item(&self, item: &GroundItem, camera: &Camera, state: &GameState) {
        // Special rendering for gold piles
        if item.item_id == "gold" && item.gold_pile.is_some() {
            self.render_gold_pile(item, camera);
            return;
        }

        let (screen_x, screen_y) = world_to_screen(item.x, item.y, camera);
        let zoom = camera.zoom;
        let time = macroquad::time::get_time();
        let elapsed = time - item.animation_time;

        // Animation phase durations (same as gold)
        const ARC_DURATION: f64 = 0.3;
        const BOUNCE_DURATION: f64 = 0.2;
        const SETTLE_DURATION: f64 = 0.1;
        const TOTAL_DURATION: f64 = ARC_DURATION + BOUNCE_DURATION + SETTLE_DURATION;

        // Animation heights
        const ARC_HEIGHT: f32 = 10.0;
        const BOUNCE_HEIGHT: f32 = 4.0;

        // Bob animation (post-settle)
        const BOB_SPEED: f64 = 3.0;
        const BOB_AMPLITUDE: f32 = 2.0;

        // Calculate height offset based on animation phase
        let (height_offset, spawn_progress) = if elapsed < ARC_DURATION {
            // Phase 1: Arc up and down
            let t = (elapsed / ARC_DURATION) as f32;
            let arc = 4.0 * ARC_HEIGHT * t * (1.0 - t);
            (arc, t)
        } else if elapsed < ARC_DURATION + BOUNCE_DURATION {
            // Phase 2: Bounce up
            let t = ((elapsed - ARC_DURATION) / BOUNCE_DURATION) as f32;
            let bounce = 4.0 * BOUNCE_HEIGHT * t * (1.0 - t);
            (bounce, 1.0)
        } else if elapsed < TOTAL_DURATION {
            // Phase 3: Settle
            let t = ((elapsed - ARC_DURATION - BOUNCE_DURATION) / SETTLE_DURATION) as f32;
            let settle = 4.0 * (BOUNCE_HEIGHT * 0.25) * t * (1.0 - t);
            (settle, 1.0)
        } else {
            // Animation complete - gentle bob
            let bob = ((elapsed * BOB_SPEED).sin() as f32) * BOB_AMPLITUDE;
            (bob, 1.0)
        };

        // Shadow rendering - size and alpha respond to height
        const SHADOW_WIDTH: f32 = 14.0;
        const SHADOW_HEIGHT: f32 = 6.0;
        const SHADOW_BASE_ALPHA: f32 = 50.0;

        let height_normalized = height_offset / ARC_HEIGHT; // Normalize to arc height
        let shadow_scale = 1.0 - height_normalized * 0.2;
        let shadow_alpha = ((SHADOW_BASE_ALPHA - height_normalized * 15.0) * spawn_progress).clamp(0.0, 255.0) as u8;

        draw_ellipse(
            screen_x,
            screen_y,
            SHADOW_WIDTH * zoom * shadow_scale,
            SHADOW_HEIGHT * zoom * shadow_scale,
            0.0,
            Color::from_rgba(0, 0, 0, shadow_alpha),
        );

        let item_def = state.item_registry.get_or_placeholder(&item.item_id);
        let item_y = screen_y - 8.0 * zoom - height_offset * zoom;

        // Try to use item sprite, fall back to colored rectangle
        if let Some((texture, source_rect)) = self.item_sprites.get(&item.item_id) {
            // Use texture (or atlas region), centered on the ground position
            let (sprite_w, sprite_h) = if let Some(r) = source_rect {
                (r.w, r.h)
            } else {
                (texture.width(), texture.height())
            };
            let icon_width = sprite_w * zoom;
            let icon_height = sprite_h * zoom;

            draw_texture_ex(
                texture,
                screen_x - icon_width / 2.0,
                item_y - icon_height / 2.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(icon_width, icon_height)),
                    source: source_rect,
                    ..Default::default()
                },
            );
        } else {
            // Fallback to colored rectangle
            let color = item_def.category_color();
            draw_rectangle(screen_x - 6.0 * zoom, item_y - 6.0 * zoom, 16.0 * zoom, 12.0 * zoom, color);
            draw_rectangle_lines(screen_x - 6.0 * zoom, item_y - 6.0 * zoom, 16.0 * zoom, 12.0 * zoom, 1.0, WHITE);
        }
    }

    /// Render a fishing line from the player's rod tip to a landing point in the water
    fn render_fishing_line(&self, player: &Player, camera: &Camera) {
        use crate::game::Direction;
        use super::animation::{get_weapon_offset, get_weapon_frame, should_flip_horizontal, Gender};

        let (screen_x, screen_y) = world_to_screen(player.x, player.y, camera);
        let zoom = camera.zoom;
        let time = macroquad::time::get_time();

        // Compute weapon draw position (same as render_player)
        let draw_x = screen_x - SPRITE_WIDTH * zoom / 2.0;
        let draw_y = screen_y - SPRITE_HEIGHT * zoom + 8.0 * zoom;

        // Get player gender for gender-specific offsets
        let player_gender = Gender::from_str(&player.gender);

        let anim_frame = player.animation.frame as u32;
        let (offset_x, offset_y) = get_weapon_offset(player.animation.state, player.animation.direction, anim_frame, player_gender);
        let weapon_frame = get_weapon_frame(player.animation.state, player.animation.direction, anim_frame);
        let flip = weapon_frame.flip_h;

        // Fishing rod frame size (from manifest: 70x86)
        let fw: f32 = 70.0;
        let fh: f32 = 86.0;

        let weapon_draw_x = draw_x + offset_x * zoom;
        let weapon_draw_y = draw_y + offset_y * zoom;

        // Rod tip position within the weapon frame (in unscaled pixels)
        // These are the approximate pixel positions of the rod tip in each frame
        let (tip_px, tip_py) = match player.animation.direction {
            Direction::Down  => (14.0, 61.0),  // frame 0: rod points down, tip is lower (+2x, -2y adjust)
            Direction::Left  => (16.0, 38.0),  // frame 1: mirrored adjustment (+4x, +8y)
            Direction::Up    => (16.0, 38.0),  // frame 1 flipped: (-4 screen-left, +8y down)
            Direction::Right => (10.0, 61.0),  // frame 0 flipped: mirrored down adjust (-2x, -2y)
        };

        // Account for horizontal flip
        let tip_in_frame_x = if flip { fw - tip_px } else { tip_px };

        let rod_x = weapon_draw_x + tip_in_frame_x * zoom;
        let rod_y = weapon_draw_y + tip_py * zoom;

        // Landing point: center of a tile 2-3 tiles ahead in the facing direction
        // Use player position as seed for stable per-session random distance
        let seed = (player.x * 73.137 + player.y * 37.891) as f32;
        let cast_dist = 2.0 + (seed.sin() * 0.5 + 0.5); // range [2.0, 3.0]
        let (tile_dx, tile_dy): (f32, f32) = match player.animation.direction {
            Direction::Down  => ( 0.0,  cast_dist),
            Direction::Up    => ( 0.0, -cast_dist),
            Direction::Left  => (-cast_dist,  0.0),
            Direction::Right => ( cast_dist,  0.0),
        };

        let (land_base_x, land_base_y) = world_to_screen(
            player.x + tile_dx,
            player.y + tile_dy,
            camera,
        );

        // Gentle sway at the landing point
        let sway_x = (time * 0.8).sin() as f32 * 2.0 * zoom;
        let sway_y = (time * 0.6).cos() as f32 * 1.0 * zoom;
        let land_x = land_base_x + sway_x;
        let land_y = land_base_y + sway_y;

        // Draw line as a catenary curve using segments
        let segments = 8;
        let line_color = Color::new(1.0, 1.0, 1.0, 0.85);
        let line_thickness = (1.0 * zoom).max(0.5);

        for i in 0..segments {
            let t0 = i as f32 / segments as f32;
            let t1 = (i + 1) as f32 / segments as f32;

            let x0 = rod_x + (land_x - rod_x) * t0;
            let x1 = rod_x + (land_x - rod_x) * t1;
            let y0_base = rod_y + (land_y - rod_y) * t0;
            let y1_base = rod_y + (land_y - rod_y) * t1;

            // Parabolic droop, max at midpoint
            let droop_amount = 10.0 * zoom;
            let sag0 = droop_amount * 4.0 * t0 * (1.0 - t0);
            let sag1 = droop_amount * 4.0 * t1 * (1.0 - t1);

            // Slight wind ripple increasing toward the end
            let wind = (time * 2.5 + t0 as f64 * 3.0).sin() as f32 * 1.5 * zoom * t0;

            draw_line(x0 + wind * 0.5, y0_base + sag0, x1 + wind * 0.5, y1_base + sag1, line_thickness, line_color);
        }

        // Small bobber at the landing point
        let bobber_bob = (time * 1.5).sin() as f32 * 1.5 * zoom;
        draw_circle(land_x, land_y + bobber_bob, 2.0 * zoom, Color::new(0.8, 0.2, 0.1, 0.8));
        draw_circle(land_x, land_y + bobber_bob, 1.2 * zoom, Color::new(1.0, 0.4, 0.2, 0.9));
    }

    /// Render farming patches in two passes: signs first (behind), then crops on top
    fn render_farming_patches(&self, state: &GameState) {
        if state.current_interior.is_some() {
            return;
        }
        let zoom = state.camera.zoom;
        let time = macroquad::time::get_time();
        let frame_w = 16.0;
        let frame_h = 32.0;

        // Pass 1: Draw signs behind crops (at the top/back of the tile)
        for patch in state.farming_patches.values() {
            if patch.state != "growing" && patch.state != "harvestable" {
                continue;
            }
            let sign_name = Self::crop_to_sprite_name(&patch.crop_id);
            if let Some((farm_texture, farm_atlas_offset)) = self.farming_sprites.get(sign_name.as_str()) {
                let (screen_x, screen_y) = world_to_screen(patch.x as f32, patch.y as f32, &state.camera);
                let sign_frame = 5u32;
                let (farm_atlas_x, farm_atlas_y) = farm_atlas_offset.unwrap_or((0.0, 0.0));
                let src_x = farm_atlas_x + sign_frame as f32 * frame_w;
                let sign_w = frame_w * zoom;
                let sign_h = frame_h * zoom;
                // Position at the top (back) of the isometric tile
                let sign_x = screen_x - sign_w / 2.0;
                let sign_y = screen_y - sign_h - 4.0 * zoom;
                draw_texture_ex(
                    farm_texture,
                    sign_x,
                    sign_y,
                    WHITE,
                    DrawTextureParams {
                        source: Some(Rect::new(src_x, farm_atlas_y, frame_w, frame_h)),
                        dest_size: Some(Vec2::new(sign_w, sign_h)),
                        ..Default::default()
                    },
                );
            }
        }

        // Pass 2: Draw crops and empty patch fallbacks on top
        for patch in state.farming_patches.values() {
            let (screen_x, screen_y) = world_to_screen(patch.x as f32, patch.y as f32, &state.camera);

            // Determine which sprite frame to show
            let (sprite_name, frame_index) = match patch.state.as_str() {
                "empty" => (None, 0u32),
                "growing" => {
                    let name = Self::crop_to_sprite_name(&patch.crop_id);
                    let frame = match patch.growth_stage {
                        0 => 0,
                        1 => 2,
                        2 => 3,
                        3 => 4,
                        _ => 4,
                    };
                    (Some(name), frame)
                }
                "harvestable" => {
                    let name = Self::crop_to_sprite_name(&patch.crop_id);
                    (Some(name), 4)
                }
                _ => (None, 0),
            };

            // Try to draw sprite
            let drew_sprite = if let Some(ref name) = sprite_name {
                if let Some((crop_texture, crop_atlas_offset)) = self.farming_sprites.get(name.as_str()) {
                    let (crop_atlas_x, crop_atlas_y) = crop_atlas_offset.unwrap_or((0.0, 0.0));
                    let src_x = crop_atlas_x + frame_index as f32 * frame_w;
                    let draw_w = frame_w * zoom;
                    let draw_h = frame_h * zoom;

                    let tint = WHITE;

                    draw_texture_ex(
                        crop_texture,
                        screen_x - draw_w / 2.0,
                        screen_y - draw_h + draw_h * 0.25,
                        tint,
                        DrawTextureParams {
                            source: Some(Rect::new(src_x, crop_atlas_y, frame_w, frame_h)),
                            dest_size: Some(Vec2::new(draw_w, draw_h)),
                            ..Default::default()
                        },
                    );
                    true
                } else {
                    false
                }
            } else {
                false
            };

            // Fallback: draw colored diamond for empty patches or missing sprites
            if !drew_sprite {
                let half_w = 16.0 * zoom;
                let half_h = 8.0 * zoom;
                let base_color = Color::new(0.35, 0.25, 0.15, 0.5);
                let border_color = Color::new(0.45, 0.35, 0.2, 0.6);

                draw_triangle(
                    Vec2::new(screen_x, screen_y - half_h),
                    Vec2::new(screen_x - half_w, screen_y),
                    Vec2::new(screen_x, screen_y + half_h),
                    base_color,
                );
                draw_triangle(
                    Vec2::new(screen_x, screen_y - half_h),
                    Vec2::new(screen_x + half_w, screen_y),
                    Vec2::new(screen_x, screen_y + half_h),
                    base_color,
                );
                draw_line(screen_x, screen_y - half_h, screen_x - half_w, screen_y, 1.0, border_color);
                draw_line(screen_x - half_w, screen_y, screen_x, screen_y + half_h, 1.0, border_color);
                draw_line(screen_x, screen_y + half_h, screen_x + half_w, screen_y, 1.0, border_color);
                draw_line(screen_x + half_w, screen_y, screen_x, screen_y - half_h, 1.0, border_color);
            }

            // Draw soft pulsing green overlay on tile for harvestable crops
            if patch.state == "harvestable" {
                let half_w = 16.0 * zoom;
                let half_h = 8.0 * zoom;
                // Slow, gentle pulse between 0.08 and 0.18 alpha
                let pulse_alpha = ((time * 1.2).sin() as f32 * 0.05 + 0.13).clamp(0.08, 0.18);
                let glow = Color::new(0.2, 0.7, 0.3, pulse_alpha);
                draw_triangle(
                    Vec2::new(screen_x, screen_y - half_h),
                    Vec2::new(screen_x - half_w, screen_y),
                    Vec2::new(screen_x, screen_y + half_h),
                    glow,
                );
                draw_triangle(
                    Vec2::new(screen_x, screen_y - half_h),
                    Vec2::new(screen_x + half_w, screen_y),
                    Vec2::new(screen_x, screen_y + half_h),
                    glow,
                );
            }
        }
    }

    /// Map crop_id from farming config to sprite sheet name
    fn crop_to_sprite_name(crop_id: &str) -> String {
        crop_id.to_string()
    }

    fn render_farming_patch_labels(&self, state: &GameState) {
        if state.current_interior.is_some() {
            return;
        }
        let hovered_tile = match state.hovered_tile {
            Some(t) => t,
            None => return,
        };

        // Check if hovered tile is a farming patch
        let patch_id = match state.farming_patch_positions.get(&hovered_tile) {
            Some(id) => id,
            None => return,
        };
        let patch = match state.farming_patches.get(patch_id) {
            Some(p) => p,
            None => return,
        };

        let (screen_x, screen_y) = world_to_screen(patch.x as f32, patch.y as f32, &state.camera);
        let zoom = state.camera.zoom;

        // Build label text
        let (label, color) = match patch.state.as_str() {
            "empty" => (
                "Allotment (Empty)".to_string(),
                Color::new(0.7, 0.6, 0.4, 1.0),
            ),
            "growing" => {
                let crop_name = patch.crop_id.replace('_', " ");
                let crop_name = crop_name.split_whitespace()
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            Some(f) => f.to_uppercase().to_string() + c.as_str(),
                            None => String::new(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                (
                    format!("{} (Stage {}/4)", crop_name, patch.growth_stage),
                    Color::new(0.4, 0.8, 0.3, 1.0),
                )
            }
            "harvestable" => {
                let crop_name = patch.crop_id.replace('_', " ");
                let crop_name = crop_name.split_whitespace()
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            Some(f) => f.to_uppercase().to_string() + c.as_str(),
                            None => String::new(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                (
                    format!("{} (Ready!)", crop_name),
                    Color::new(1.0, 0.9, 0.3, 1.0),
                )
            }
            _ => (
                "Allotment".to_string(),
                Color::new(0.7, 0.7, 0.7, 1.0),
            ),
        };

        // Scale text with zoom for readability
        let font_size = 16.0 * zoom;
        let label_width = self.measure_text_sharp(&label, font_size).width;
        let label_x = screen_x - label_width / 2.0;
        let label_y = screen_y - 16.0 * zoom;

        // Background
        let padding = 4.0 * zoom;
        let bar_height = 18.0 * zoom;
        draw_rectangle(
            label_x - padding,
            label_y - 14.0 * zoom,
            label_width + padding * 2.0,
            bar_height,
            Color::from_rgba(0, 0, 0, 180),
        );

        // Text
        self.draw_text_sharp(&label, label_x, label_y, font_size, color);
    }

    fn render_gathering_markers(&self, state: &GameState) {
        if !state.debug_mode { return; }
        let zoom = state.camera.zoom;
        for marker in &state.gathering_markers {
            // Map skill type to sprite name
            let sprite_id = match marker.skill.as_str() {
                "fishing" => "trout",
                _ => continue,
            };

            let (screen_x, screen_y) = world_to_screen(marker.x as f32, marker.y as f32, &state.camera);

            if let Some((texture, source_rect)) = self.item_sprites.get(sprite_id) {
                let (sprite_w, sprite_h) = if let Some(r) = source_rect {
                    (r.w, r.h)
                } else {
                    (texture.width(), texture.height())
                };
                let icon_width = sprite_w * zoom;
                let icon_height = sprite_h * zoom;

                let alpha = Color::new(1.0, 1.0, 1.0, 0.7);
                draw_texture_ex(
                    texture,
                    screen_x - icon_width / 2.0,
                    screen_y - icon_height / 2.0,
                    alpha,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(icon_width, icon_height)),
                        source: source_rect,
                        ..Default::default()
                    },
                );
            }
        }
    }

    /// Render bonus tile indicators (pulsing golden glow on the ground)
    fn render_bonus_tiles(&self, state: &GameState) {
        let zoom = state.camera.zoom;
        let time = macroquad::time::get_time();

        for tile in &state.bonus_tiles {
            let elapsed = time - tile.spawn_time;
            let progress = (elapsed / tile.telegraph_duration).min(1.0) as f32;

            // Pulsing alpha: oscillates faster as it approaches expiry
            let pulse_speed = 3.0 + progress as f64 * 8.0;
            let pulse = ((time * pulse_speed).sin() as f32 * 0.5 + 0.5) * 0.4 + 0.2;

            let (screen_x, screen_y) = world_to_screen(tile.x as f32, tile.y as f32, &state.camera);

            // Draw a golden diamond shape (isometric tile highlight)
            let half_w = 16.0 * zoom;
            let half_h = 8.0 * zoom;
            let color = Color::new(1.0, 0.85, 0.2, pulse);

            // Draw as a filled isometric diamond
            draw_triangle(
                Vec2::new(screen_x, screen_y - half_h),
                Vec2::new(screen_x + half_w, screen_y),
                Vec2::new(screen_x, screen_y + half_h),
                color,
            );
            draw_triangle(
                Vec2::new(screen_x, screen_y - half_h),
                Vec2::new(screen_x - half_w, screen_y),
                Vec2::new(screen_x, screen_y + half_h),
                color,
            );

            // Draw a star/sparkle icon in the center
            let star_color = Color::new(1.0, 1.0, 0.6, pulse + 0.3);
            draw_circle(screen_x, screen_y, 3.0 * zoom, star_color);
        }
    }

    /// Render exit portal arrows on interior map edges
    fn render_exit_portal_arrows(&self, state: &GameState) {
        // Only render in interior mode
        let (width, height) = match state.chunk_manager.get_interior_size() {
            Some(size) => size,
            None => return,
        };

        // Get interior chunk (always at 0,0)
        let coord = crate::game::ChunkCoord::new(0, 0);
        let chunk = match state.chunk_manager.chunks().get(&coord) {
            Some(c) => c,
            None => return,
        };

        // Pulsing opacity (70-100%, 2-second cycle)
        let time = macroquad::time::get_time();
        let alpha = (0.7 + 0.3 * (time * 3.14).sin() as f32).clamp(0.0, 1.0);
        let color = Color::new(1.0, 1.0, 1.0, alpha);

        let zoom = state.camera.zoom;
        let arrow_w = 64.0 * zoom;
        let arrow_h = 32.0 * zoom;

        // Track min/max positions for portals on each edge
        // (min_pos, max_pos) where pos is Y for left/right edges, X for top/bottom
        let mut left_span: Option<(i32, i32)> = None;
        let mut right_span: Option<(i32, i32)> = None;
        let mut top_span: Option<(i32, i32)> = None;
        let mut bottom_span: Option<(i32, i32)> = None;

        // Group portals by edge and find spans
        // Use else-if to ensure each portal only counts for ONE edge (priority: bottom > top > right > left)
        for portal in &chunk.portals {
            if portal.y + portal.height >= height as i32 {
                // Bottom edge
                let min_x = portal.x;
                let max_x = portal.x + portal.width;
                bottom_span = Some(match bottom_span {
                    Some((cur_min, cur_max)) => (cur_min.min(min_x), cur_max.max(max_x)),
                    None => (min_x, max_x),
                });
            } else if portal.y == 0 {
                // Top edge
                let min_x = portal.x;
                let max_x = portal.x + portal.width;
                top_span = Some(match top_span {
                    Some((cur_min, cur_max)) => (cur_min.min(min_x), cur_max.max(max_x)),
                    None => (min_x, max_x),
                });
            } else if portal.x + portal.width >= width as i32 {
                // Right edge
                let min_y = portal.y;
                let max_y = portal.y + portal.height;
                right_span = Some(match right_span {
                    Some((cur_min, cur_max)) => (cur_min.min(min_y), cur_max.max(max_y)),
                    None => (min_y, max_y),
                });
            } else if portal.x == 0 {
                // Left edge
                let min_y = portal.y;
                let max_y = portal.y + portal.height;
                left_span = Some(match left_span {
                    Some((cur_min, cur_max)) => (cur_min.min(min_y), cur_max.max(max_y)),
                    None => (min_y, max_y),
                });
            }
        }

        // Draw arrow for each edge that has portals, centered on the span
        if let Some((min_y, max_y)) = left_span {
            if let Some(ref tex) = self.exit_arrow_left {
                let center_y = (min_y + max_y) as f32 / 2.0;
                let (sx, sy) = world_to_screen(-0.5, center_y, &state.camera);
                draw_texture_ex(
                    tex,
                    sx - arrow_w / 2.0,
                    sy - arrow_h / 2.0,
                    color,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(arrow_w, arrow_h)),
                        ..Default::default()
                    },
                );
            }
        }
        if let Some((min_y, max_y)) = right_span {
            if let Some(ref tex) = self.exit_arrow_right {
                let center_y = (min_y + max_y) as f32 / 2.0;
                let (sx, sy) = world_to_screen(width as f32 + 0.5, center_y, &state.camera);
                draw_texture_ex(
                    tex,
                    sx - arrow_w / 2.0,
                    sy - arrow_h / 2.0,
                    color,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(arrow_w, arrow_h)),
                        ..Default::default()
                    },
                );
            }
        }
        if let Some((min_x, max_x)) = top_span {
            if let Some(ref tex) = self.exit_arrow_up {
                let center_x = (min_x + max_x) as f32 / 2.0;
                let (sx, sy) = world_to_screen(center_x, -0.5, &state.camera);
                draw_texture_ex(
                    tex,
                    sx - arrow_w / 2.0,
                    sy - arrow_h / 2.0,
                    color,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(arrow_w, arrow_h)),
                        ..Default::default()
                    },
                );
            }
        }
        if let Some((min_x, max_x)) = bottom_span {
            if let Some(ref tex) = self.exit_arrow_down {
                let center_x = (min_x + max_x) as f32 / 2.0;
                let (sx, sy) = world_to_screen(center_x, height as f32 + 0.5, &state.camera);
                draw_texture_ex(
                    tex,
                    sx - arrow_w / 2.0,
                    sy - arrow_h / 2.0,
                    color,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(arrow_w, arrow_h)),
                        ..Default::default()
                    },
                );
            }
        }
    }

    /// Render gathering buff timer indicator (top-center HUD)
    fn render_gathering_buff(&self, state: &GameState) {
        if !state.is_gathering { return; }
        if let Some(ref buff) = state.gathering_buff {
            let time = macroquad::time::get_time();
            let elapsed = time - buff.start_time;
            let remaining = (buff.duration - elapsed).max(0.0);
            if remaining <= 0.0 { return; }
            let progress = (remaining / buff.duration) as f32;

            let sw = screen_width();
            let bar_w = 120.0;
            let bar_h = 14.0;
            let x = (sw - bar_w) / 2.0;
            let y = 40.0;

            // Background
            draw_rectangle(x - 1.0, y - 1.0, bar_w + 2.0, bar_h + 2.0, Color::new(0.0, 0.0, 0.0, 0.6));
            // Fill
            let fill_color = Color::new(1.0, 0.85, 0.2, 0.8);
            draw_rectangle(x, y, bar_w * progress, bar_h, fill_color);
            // Text
            let label = format!("2x Gather {:.0}s", remaining);
            let font_size = 10.0;
            let text_w = self.font.measure_text(&label, font_size).width;
            self.draw_text_sharp(&label, x + (bar_w - text_w) / 2.0, y + 11.0, font_size, WHITE);
        }
    }

    /// Render a gold pile with multiple animated nuggets
    fn render_gold_pile(&self, item: &GroundItem, camera: &Camera) {
        let (screen_x, screen_y) = world_to_screen(item.x, item.y, camera);
        let zoom = camera.zoom;
        let time = macroquad::time::get_time();

        let pile = match &item.gold_pile {
            Some(p) => p,
            None => return,
        };

        let texture = match &self.gold_nugget_texture {
            Some(t) => t,
            None => return,
        };

        let elapsed = time - pile.spawn_time;

        // Animation phase durations
        const ARC_DURATION: f64 = 0.3;      // Phase 1: arc outward
        const BOUNCE_DURATION: f64 = 0.2;   // Phase 2: bounce up
        const SETTLE_DURATION: f64 = 0.1;   // Phase 3: settle down
        const TOTAL_DURATION: f64 = ARC_DURATION + BOUNCE_DURATION + SETTLE_DURATION;
        const STAGGER_DELAY: f64 = 0.03;

        // Animation heights
        const ARC_HEIGHT: f32 = 10.0;       // Peak height during arc
        const BOUNCE_HEIGHT: f32 = 4.0;     // Peak height during bounce

        // Bob animation (post-settle)
        const BOB_SPEED: f64 = 2.5;
        const BOB_AMPLITUDE: f32 = 1.5;

        // Shadow constants
        const SHADOW_WIDTH: f32 = 18.0;
        const SHADOW_HEIGHT: f32 = 8.0;
        const SHADOW_BASE_ALPHA: f32 = 50.0;

        // Calculate overall spawn progress for shadow fade-in
        let overall_spawn_t = (elapsed / TOTAL_DURATION).clamp(0.0, 1.0) as f32;

        // Calculate average bob for shadow pulse (only after nuggets mostly settled)
        let avg_bob = if overall_spawn_t > 0.7 {
            let bob_strength = ((overall_spawn_t - 0.7) / 0.3).min(1.0);
            let sum: f32 = pile.nuggets.iter().map(|n| {
                ((time * BOB_SPEED + n.phase_offset).sin() as f32) * BOB_AMPLITUDE * zoom
            }).sum();
            (sum / pile.nuggets.len() as f32) * bob_strength
        } else {
            0.0
        };

        // Shadow size and alpha respond to average bob
        let bob_normalized = avg_bob / (BOB_AMPLITUDE * zoom);
        let shadow_scale = 1.0 - bob_normalized * 0.15;
        let shadow_alpha = ((SHADOW_BASE_ALPHA - bob_normalized * 10.0) * overall_spawn_t).clamp(0.0, 255.0) as u8;

        draw_ellipse(
            screen_x,
            screen_y,
            SHADOW_WIDTH * zoom * shadow_scale,
            SHADOW_HEIGHT * zoom * shadow_scale,
            0.0,
            Color::from_rgba(0, 0, 0, shadow_alpha),
        );

        // Sort nuggets by Y offset for proper depth (back to front)
        let mut sorted_indices: Vec<usize> = (0..pile.nuggets.len()).collect();
        sorted_indices.sort_by(|&a, &b| {
            pile.nuggets[a]
                .target_y
                .partial_cmp(&pile.nuggets[b].target_y)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Render each nugget
        for (render_idx, &nugget_idx) in sorted_indices.iter().enumerate() {
            let nugget = &pile.nuggets[nugget_idx];

            // Calculate elapsed time for this nugget (with stagger)
            let nugget_elapsed = elapsed - (render_idx as f64 * STAGGER_DELAY);
            if nugget_elapsed < 0.0 {
                continue; // Nugget hasn't spawned yet
            }

            // Calculate position and height based on animation phase
            let (current_x, current_y, height_offset) = if nugget_elapsed < ARC_DURATION {
                // Phase 1: Arc outward from center to target
                let t = (nugget_elapsed / ARC_DURATION) as f32;
                let ease_t = 1.0 - (1.0 - t).powi(2); // Ease-out quadratic for position

                let x = nugget.target_x * ease_t;
                let y = nugget.target_y * ease_t;
                // Parabolic arc: height = 4 * peak * t * (1 - t)
                let arc = 4.0 * ARC_HEIGHT * t * (1.0 - t);

                (x, y, arc)
            } else if nugget_elapsed < ARC_DURATION + BOUNCE_DURATION {
                // Phase 2: Bounce up from target position
                let t = ((nugget_elapsed - ARC_DURATION) / BOUNCE_DURATION) as f32;
                // Parabolic bounce
                let bounce = 4.0 * BOUNCE_HEIGHT * t * (1.0 - t);

                (nugget.target_x, nugget.target_y, bounce)
            } else if nugget_elapsed < TOTAL_DURATION {
                // Phase 3: Settle down
                let t = ((nugget_elapsed - ARC_DURATION - BOUNCE_DURATION) / SETTLE_DURATION) as f32;
                // Small settling bounce (quarter height of main bounce)
                let settle = 4.0 * (BOUNCE_HEIGHT * 0.25) * t * (1.0 - t);

                (nugget.target_x, nugget.target_y, settle)
            } else {
                // Animation complete - apply bob
                let bob = ((time * BOB_SPEED + nugget.phase_offset).sin() as f32) * BOB_AMPLITUDE;
                (nugget.target_x, nugget.target_y, bob)
            };

            // Calculate final screen position
            let nugget_x = screen_x + current_x * zoom;
            let nugget_y = screen_y + current_y * zoom - height_offset * zoom - 4.0 * zoom;

            // Draw nugget sprite
            let width = texture.width() * zoom;
            let height = texture.height() * zoom;

            draw_texture_ex(
                texture,
                nugget_x - width / 2.0,
                nugget_y - height / 2.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(width, height)),
                    ..Default::default()
                },
            );
        }
    }

    /// Render a map object (tree, rock, decoration) from chunk data
    fn render_map_object(&self, obj: &MapObject, camera: &Camera) {
        // Get screen position for the tile CENTER (add 0.5 to tile coords)
        let (screen_x, screen_y) = world_to_screen(obj.tile_x as f32 + 0.5, obj.tile_y as f32 + 0.5, camera);
        let zoom = camera.zoom;

        // Try to get the sprite for this gid
        if let Some((texture, source_rect)) = self.get_object_sprite(obj.gid) {
            let (tex_width, tex_height) = if let Some(r) = source_rect {
                (r.w, r.h)
            } else {
                (texture.width(), texture.height())
            };

            // Scale the sprite (round to avoid fractional scaling artifacts)
            let scaled_width = (tex_width * zoom).round();
            let scaled_height = (tex_height * zoom).round();

            // Position sprite so its bottom-center aligns with the tile center
            // Round to pixel grid to avoid subpixel rendering artifacts
            let draw_x = (screen_x - scaled_width / 2.0).round();
            let draw_y = (screen_y - scaled_height).round();

            draw_texture_ex(
                texture,
                draw_x,
                draw_y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                    source: source_rect,
                    ..Default::default()
                },
            );
        } else {
            // Fallback: draw a colored placeholder rectangle
            let placeholder_width = (32.0 * zoom).round();
            let placeholder_height = (64.0 * zoom).round();
            let draw_x = (screen_x - placeholder_width / 2.0).round();
            let draw_y = (screen_y - placeholder_height).round();

            draw_rectangle(
                draw_x,
                draw_y,
                placeholder_width,
                placeholder_height,
                Color::from_rgba(100, 150, 100, 200),
            );
            draw_rectangle_lines(
                draw_x,
                draw_y,
                placeholder_width,
                placeholder_height,
                2.0,
                Color::from_rgba(50, 100, 50, 255),
            );
        }
    }

    /// Render a map object with a horizontal shake offset (for trees being chopped)
    fn render_map_object_shaking(&self, obj: &MapObject, shake_offset: f32, camera: &Camera) {
        let (screen_x, screen_y) = world_to_screen(obj.tile_x as f32 + 0.5, obj.tile_y as f32 + 0.5, camera);
        let zoom = camera.zoom;

        if let Some((texture, source_rect)) = self.get_object_sprite(obj.gid) {
            let (tex_width, tex_height) = if let Some(r) = source_rect {
                (r.w, r.h)
            } else {
                (texture.width(), texture.height())
            };

            let scaled_width = (tex_width * zoom).round();
            let scaled_height = (tex_height * zoom).round();

            // Apply shake offset to x position
            let draw_x = (screen_x - scaled_width / 2.0 + shake_offset * zoom).round();
            let draw_y = (screen_y - scaled_height).round();

            draw_texture_ex(
                texture,
                draw_x,
                draw_y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                    source: source_rect,
                    ..Default::default()
                },
            );
        }
    }

    /// Render a falling tree (tree that was just chopped down)
    fn render_falling_tree(&self, gid: u32, tile_x: i32, tile_y: i32, angle: f32, alpha: f32, _y_offset: f32, camera: &Camera) {
        // The pivot point (tree base) should stay fixed at pivot_x, pivot_y
        let (pivot_x, pivot_y) = world_to_screen(tile_x as f32 + 0.5, tile_y as f32 + 0.5, camera);
        let zoom = camera.zoom;

        if let Some((texture, source_rect)) = self.get_object_sprite(gid) {
            let (tex_width, tex_height) = if let Some(r) = source_rect {
                (r.w, r.h)
            } else {
                (texture.width(), texture.height())
            };

            let w = tex_width * zoom;
            let h = tex_height * zoom;

            // Rotate each corner around the pivot (bottom-center of tree)
            // Corners relative to pivot: TL, TR, BR, BL
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let rotate = |rx: f32, ry: f32| -> Vec3 {
                Vec3::new(
                    pivot_x + rx * cos_a - ry * sin_a,
                    pivot_y + rx * sin_a + ry * cos_a,
                    0.0
                )
            };

            let tl = rotate(-w / 2.0, -h);
            let tr = rotate(w / 2.0, -h);
            let br = rotate(w / 2.0, 0.0);
            let bl = rotate(-w / 2.0, 0.0);

            // UV coordinates
            let (u0, v0, u1, v1) = if let Some(r) = source_rect {
                (
                    r.x / texture.width(),
                    r.y / texture.height(),
                    (r.x + r.w) / texture.width(),
                    (r.y + r.h) / texture.height(),
                )
            } else {
                (0.0, 0.0, 1.0, 1.0)
            };

            let color_arr: [u8; 4] = [255, 255, 255, (alpha * 255.0) as u8];

            // Build mesh with 4 vertices and 2 triangles
            let mesh = Mesh {
                vertices: vec![
                    Vertex { position: tl, uv: Vec2::new(u0, v0), color: color_arr, normal: Vec4::ZERO },
                    Vertex { position: tr, uv: Vec2::new(u1, v0), color: color_arr, normal: Vec4::ZERO },
                    Vertex { position: br, uv: Vec2::new(u1, v1), color: color_arr, normal: Vec4::ZERO },
                    Vertex { position: bl, uv: Vec2::new(u0, v1), color: color_arr, normal: Vec4::ZERO },
                ],
                indices: vec![0, 1, 2, 0, 2, 3],
                texture: Some(texture.clone()),
            };

            draw_mesh(&mesh);
        }
    }

    /// Render a wall on a tile edge
    fn render_wall(&self, wall: &Wall, camera: &Camera) {
        let zoom = camera.zoom;

        // Get the tile's top vertex screen position (same as mapper)
        // Use exact coordinates to avoid rounding errors
        let (screen_x, screen_y) = world_to_screen_exact(
            wall.tile_x as f32,
            wall.tile_y as f32,
            camera
        );

        // Tiles are centered on their world_to_screen position, so
        // bottom vertex is at center + half tile height (not full height)
        // Round to pixel grid to avoid subpixel jitter
        let bottom_vertex_x = screen_x.round();
        let bottom_vertex_y = (screen_y + (TILE_HEIGHT / 2.0) * zoom).round();

        // Try to get the wall sprite for this gid
        if let Some((texture, source_rect)) = self.get_wall_sprite(wall.gid) {
            let (tex_width, tex_height) = if let Some(r) = source_rect {
                (r.w, r.h)
            } else {
                (texture.width(), texture.height())
            };

            let scaled_width = (tex_width * zoom).round();
            let scaled_height = (tex_height * zoom).round();

            let (draw_x, draw_y) = match wall.edge {
                WallEdge::Down => {
                    // Bottom-right corner of sprite at bottom vertex
                    (bottom_vertex_x - scaled_width, bottom_vertex_y - scaled_height)
                }
                WallEdge::Right => {
                    // Bottom-left corner of sprite at bottom vertex
                    (bottom_vertex_x, bottom_vertex_y - scaled_height)
                }
            };

            draw_texture_ex(
                texture,
                draw_x.round(),
                draw_y.round(),
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(scaled_width, scaled_height)),
                    source: source_rect,
                    ..Default::default()
                },
            );
        }
    }

    fn render_ui(&self, state: &GameState) {
        // Server announcements (top of screen)
        let current_time = macroquad::time::get_time();
        for (i, announcement) in state.ui_state.announcements.iter().enumerate() {
            let age = current_time - announcement.time;
            // Fade out after 6 seconds (announcements last 8 seconds total)
            let alpha = if age > 6.0 { ((8.0 - age) / 2.0 * 255.0) as u8 } else { 255 };

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
                    self.draw_text_sharp(&text, text_x + ox, text_y + oy, font_size, Color::from_rgba(0, 0, 0, alpha));
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
        // Scale with zoom for readability
        if state.ui_state.chat_log_visible {
            let zoom = state.camera.zoom;
            let scale = state.ui_state.ui_scale;
            let chat_x = 10.0;
            let (_, chat_sh) = virtual_screen_size();
            // Align chat box bottom with the bottom of the quick slot bar
            let bg_padding = 6.0 * zoom;
            let bar_bottom_offset = EXP_BAR_GAP * scale + 6.0; // distance from screen bottom to hotkey bar bottom + extra margin
            let box_bottom = chat_sh - bar_bottom_offset; // box bottom edge aligns with hotkey bar bottom
            let line_height = 18.0 * zoom;
            let max_chat_width = 400.0 * zoom;
            let font_size = 16.0 * zoom;
            let max_visible_lines: usize = 9;
            let chat_area_h = max_visible_lines as f32 * line_height;
            // chat_bottom_y = Y of the last text line baseline, inside the padding
            let chat_bottom_y = box_bottom - bg_padding;
            let chat_top_y = chat_bottom_y - chat_area_h + line_height;

            // Semi-transparent background behind chat log
            let bg_padding = 6.0 * zoom;
            let clip_x = chat_x - bg_padding;
            let clip_y = chat_top_y - bg_padding;
            let clip_w = max_chat_width + bg_padding * 2.0;
            let clip_h = chat_area_h + bg_padding * 2.0;

            if state.ui_state.chat_log_background {
                draw_rectangle(clip_x, clip_y, clip_w, clip_h, Color::new(0.0, 0.0, 0.0, 0.45));
            }

            // Build all wrapped lines
            let mut all_lines: Vec<(String, Color)> = Vec::new();
            for msg in state.ui_state.chat_messages.iter() {
                let (color, text) = match msg.channel {
                    ChatChannel::Local => (WHITE, format!("{}: {}", msg.sender_name, msg.text)),
                    ChatChannel::Global => (SKYBLUE, format!("[G] {}: {}", msg.sender_name, msg.text)),
                    ChatChannel::System => (YELLOW, format!("{} {}", msg.sender_name, msg.text)),
                };
                let wrapped_lines = self.wrap_text(&text, max_chat_width, font_size);
                for line in wrapped_lines {
                    all_lines.push((line, color));
                }
            }

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
                if current_y >= chat_top_y - line_height && current_y <= chat_bottom_y + line_height {
                    let (ref line, color) = all_lines[i];
                    self.draw_text_sharp(line, chat_x, current_y, font_size, color);
                }
                current_y -= line_height;
            }

            // Show scroll indicator if there are older messages
            if start > 0 {
                let indicator = format!("▲ {} more", start);
                let indicator_size = 13.0 * zoom;
                self.draw_text_sharp(&indicator, chat_x, chat_top_y - 2.0 * zoom, indicator_size, Color::from_rgba(180, 180, 180, 160));
            }

            // Disable scissor clipping
            {
                let mut gl = unsafe { get_internal_gl() };
                gl.flush();
                gl.quad_gl.scissor(None);
            }
        }

        // Local player stats panel (top-right corner) - Name tag + HP bar
        if let Some(player) = state.get_local_player() {
            let margin = 12.0;
            let base_y = 8.0;
            let padding = 6.0;
            let font_size = 16.0;

            // Measure text first to calculate widths
            let name = &player.name;
            let level_text = format!(" Lv.{}", player.skills.total_level());
            let name_w = self.measure_text_sharp(name, font_size).width;
            let level_w = self.measure_text_sharp(&level_text, font_size).width;
            let total_text_w = name_w + level_w;

            // Both bars use same width (at least 120, or text width + padding)
            let bar_width = (total_text_w + padding * 2.0).max(120.0);
            let tag_height = 22.0;
            let bar_height = 18.0;

            // Right-align in corner
            let (ui_sw, _) = virtual_screen_size();
            let bar_x = (ui_sw - bar_width - margin).floor();
            let tag_y = base_y;

            // ===== NAME TAG =====
            draw_rectangle(
                bar_x,
                tag_y,
                bar_width,
                tag_height,
                Color::from_rgba(0, 0, 0, 180),
            );

            // Center text in the bar
            let text_x = bar_x + (bar_width - total_text_w) / 2.0;
            let text_y = (tag_y + 16.0).floor();
            self.draw_text_sharp(name, text_x, text_y, font_size, TEXT_TITLE);
            self.draw_text_sharp(&level_text, text_x + name_w, text_y, font_size, TEXT_DIM);

            // ===== HP BAR (below name tag) =====
            let hp_bar_x = bar_x;
            let hp_bar_y = tag_y + tag_height + 4.0;
            let hp_ratio = player.hp as f32 / player.max_hp.max(1) as f32;

            draw_rectangle(hp_bar_x, hp_bar_y, bar_width, bar_height, SLOT_INNER_SHADOW);
            draw_rectangle(hp_bar_x + 1.0, hp_bar_y + 1.0, bar_width - 2.0, bar_height - 2.0, Color::new(0.08, 0.08, 0.10, 1.0));

            let hp_fill_w = (bar_width - 4.0) * hp_ratio;
            if hp_fill_w > 0.0 {
                let hp_color = if hp_ratio > 0.5 {
                    Color::new(0.2, 0.7, 0.3, 1.0)
                } else if hp_ratio > 0.25 {
                    Color::new(0.8, 0.6, 0.1, 1.0)
                } else {
                    Color::new(0.8, 0.2, 0.2, 1.0)
                };
                draw_rectangle(hp_bar_x + 2.0, hp_bar_y + 2.0, hp_fill_w, bar_height - 4.0, hp_color);
                draw_rectangle(hp_bar_x + 2.0, hp_bar_y + 2.0, hp_fill_w, (bar_height - 4.0) / 2.0, Color::new(1.0, 1.0, 1.0, 0.25));
            }

            let hp_text = format!("{}/{}", player.hp, player.max_hp);
            let hp_text_w = self.measure_text_sharp(&hp_text, font_size).width;
            self.draw_text_sharp(&hp_text, (hp_bar_x + (bar_width - hp_text_w) / 2.0).floor(), (hp_bar_y + 14.0).floor(), font_size, TEXT_NORMAL);

            // ===== MP BAR (below HP bar) =====
            let mp_bar_x = bar_x;
            let mp_bar_y = hp_bar_y + bar_height + 4.0;
            let (mp, max_mp) = state.get_local_player()
                .map(|p| (p.mp, p.max_mp))
                .unwrap_or((0, 12));
            let mp_ratio = if max_mp > 0 {
                mp as f32 / max_mp as f32
            } else {
                0.0
            };

            // Background
            draw_rectangle(mp_bar_x, mp_bar_y, bar_width, bar_height, SLOT_INNER_SHADOW);
            draw_rectangle(mp_bar_x + 1.0, mp_bar_y + 1.0, bar_width - 2.0, bar_height - 2.0, Color::new(0.08, 0.08, 0.10, 1.0));

            // MP fill (blue/purple color)
            let mp_fill_w = (bar_width - 4.0) * mp_ratio;
            if mp_fill_w > 0.0 {
                let mp_color = Color::new(0.3, 0.2, 0.8, 1.0);
                draw_rectangle(mp_bar_x + 2.0, mp_bar_y + 2.0, mp_fill_w, bar_height - 4.0, mp_color);
                draw_rectangle(mp_bar_x + 2.0, mp_bar_y + 2.0, mp_fill_w, (bar_height - 4.0) / 2.0, Color::new(0.5, 0.4, 0.95, 1.0));
            }

            // MP text
            let mp_text = format!("{}/{}", mp, max_mp);
            let mp_text_w = self.measure_text_sharp(&mp_text, font_size).width;
            self.draw_text_sharp(&mp_text, (mp_bar_x + (bar_width - mp_text_w) / 2.0).floor(), (mp_bar_y + 14.0).floor(), font_size, TEXT_NORMAL);

            // ===== PRAYER POINTS BAR (below MP bar) =====
            let prayer_bar_x = bar_x;
            let prayer_bar_y = mp_bar_y + bar_height + 4.0;
            let prayer_ratio = if state.max_prayer_points > 0 {
                state.prayer_points as f32 / state.max_prayer_points as f32
            } else {
                0.0
            };
            let prayer_low = prayer_ratio < 0.2 && state.max_prayer_points > 0;

            // Background with subtle flashing border when low
            let border_color = if prayer_low {
                // Subtle flash between normal and slightly red when prayer is low
                let flash = ((macroquad::time::get_time() * 2.0).sin() * 0.3 + 0.7) as f32;
                Color::new(0.4 * flash + 0.2, 0.15, 0.15, 1.0)
            } else {
                SLOT_INNER_SHADOW
            };
            draw_rectangle(prayer_bar_x, prayer_bar_y, bar_width, bar_height, border_color);
            draw_rectangle(prayer_bar_x + 1.0, prayer_bar_y + 1.0, bar_width - 2.0, bar_height - 2.0, Color::new(0.08, 0.08, 0.10, 1.0));

            // Prayer fill (cyan/turquoise color)
            let prayer_fill_w = (bar_width - 4.0) * prayer_ratio;
            if prayer_fill_w > 0.0 {
                let prayer_color = Color::new(0.2, 0.7, 0.85, 1.0);
                draw_rectangle(prayer_bar_x + 2.0, prayer_bar_y + 2.0, prayer_fill_w, bar_height - 4.0, prayer_color);
                draw_rectangle(prayer_bar_x + 2.0, prayer_bar_y + 2.0, prayer_fill_w, (bar_height - 4.0) / 2.0, Color::new(1.0, 1.0, 1.0, 0.25));
            }

            // Prayer text
            let prayer_text = format!("{}/{}", state.prayer_points, state.max_prayer_points);
            let prayer_text_w = self.measure_text_sharp(&prayer_text, font_size).width;
            let prayer_text_color = if prayer_low {
                // Subtle flash on text when low
                let flash = ((macroquad::time::get_time() * 2.0).sin() * 0.15 + 0.85) as f32;
                Color::new(1.0, 0.7 + 0.3 * flash, 0.7 + 0.3 * flash, 1.0)
            } else {
                TEXT_NORMAL
            };
            self.draw_text_sharp(&prayer_text, (prayer_bar_x + (bar_width - prayer_text_w) / 2.0).floor(), (prayer_bar_y + 14.0).floor(), font_size, prayer_text_color);

            // ===== Gathering/Woodcutting status indicator (below prayer bar) =====
            let is_skilling = state.is_gathering || state.is_woodcutting;
            if is_skilling {
                let gather_y = prayer_bar_y + bar_height + 4.0;
                let gather_h = 22.0;
                // Semi-transparent background (blue for fishing, brown for woodcutting)
                let (bg_color, border_color, text_color, action_name) = if state.is_woodcutting {
                    (Color::new(0.15, 0.10, 0.05, 0.7), Color::new(0.5, 0.35, 0.2, 0.5), Color::new(0.9, 0.7, 0.4, 0.9), "Chopping")
                } else {
                    (Color::new(0.05, 0.15, 0.25, 0.7), Color::new(0.2, 0.5, 0.7, 0.5), Color::new(0.4, 0.8, 0.95, 0.9), "Fishing")
                };
                draw_rectangle(bar_x, gather_y, bar_width, gather_h, bg_color);
                draw_rectangle_lines(bar_x, gather_y, bar_width, gather_h, 1.0, border_color);
                // Animated dots
                let dot_count = ((macroquad::time::get_time() * 2.0) as usize % 4) as usize;
                let dots = ".".repeat(dot_count);
                let label = format!("{}{}", action_name, dots);
                let label_w = self.measure_text_sharp(&label, 16.0).width;
                self.draw_text_sharp(&label, (bar_x + (bar_width - label_w) / 2.0).floor(), (gather_y + 15.0).floor(), 16.0, text_color);
            }

            // XP Globes (to the left of player stats)
            let globe_stats_y = tag_y + tag_height / 2.0 + 8.0; // Slightly below name tag center
            self.render_xp_globes(&state.xp_globes, bar_x, globe_stats_y);

            // XP Drop Feed (below gathering status or MP bar)
            let drop_start_y = if is_skilling {
                mp_bar_y + bar_height + 4.0 + 22.0 + 110.0 // ~100px below gathering box
            } else {
                mp_bar_y + bar_height + 110.0
            };
            self.render_xp_drop_feed(&state.xp_drop_feed, bar_x, bar_width, drop_start_y);
        }

        // Note: Interactive UI (inventory, crafting, dialogue, quick slots) is rendered
        // by render_interactive_ui() which is called by the main render loop

        // Area banner (location name during transitions)
        if state.area_banner.is_visible() {
            self.render_area_banner(&state.area_banner.text, state.area_banner.opacity());
        }

        // Chat input box (when open) - scale with zoom for readability
        if state.ui_state.chat_open {
            let zoom = state.camera.zoom;
            let (_, input_sh) = virtual_screen_size();
            let input_x = 10.0;
            let input_y = input_sh - 50.0 * zoom;
            let input_width = 400.0 * zoom;
            let input_height = 24.0 * zoom;
            let text_padding = 5.0 * zoom;
            let text_area_width = input_width - text_padding * 2.0 - 12.0 * zoom; // Extra margin for scroll indicators
            let font_size = 16.0 * zoom;

            // Background
            draw_rectangle(input_x, input_y, input_width, input_height, Color::from_rgba(0, 0, 0, 180));
            draw_rectangle_lines(input_x, input_y, input_width, input_height, 1.0, WHITE);

            let input_text = &state.ui_state.chat_input;
            let cursor_pos = state.ui_state.chat_cursor;
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
            let scroll_offset = if self.measure_text_sharp(input_text, font_size).width <= text_area_width {
                // Text fits entirely, no scroll needed
                0
            } else {
                // Find offset that keeps cursor visible
                // Start by trying to show text ending at cursor
                let text_to_cursor: String = input_text.chars().take(cursor_pos).collect();
                let cursor_text_width = self.measure_text_sharp(&text_to_cursor, font_size).width;

                if cursor_text_width <= text_area_width {
                    // Cursor is visible from start
                    0
                } else {
                    // Need to scroll - find how many chars to skip to show cursor
                    let chars: Vec<char> = input_text.chars().collect();
                    let mut offset = 0;
                    for i in 0..cursor_pos {
                        let visible: String = chars[i..cursor_pos].iter().collect();
                        if self.measure_text_sharp(&visible, font_size).width <= text_area_width {
                            offset = i;
                            break;
                        }
                    }
                    offset
                }
            };

            // Get visible portion of text that fits
            let chars_from_offset: String = input_text.chars().skip(scroll_offset).collect();
            let visible_char_count = measure_chars_that_fit(&chars_from_offset, text_area_width);
            let visible_text: String = input_text.chars().skip(scroll_offset).take(visible_char_count).collect();
            let visible_end = scroll_offset + visible_char_count;

            // Draw visible text
            let text_y_offset = 17.0 * zoom;
            self.draw_text_sharp(&visible_text, input_x + text_padding, input_y + text_y_offset, font_size, WHITE);

            // Draw scroll indicators if text is clipped
            if scroll_offset > 0 {
                self.draw_text_sharp("<", input_x + 1.0, input_y + text_y_offset, font_size, GRAY);
            }
            if visible_end < char_count {
                self.draw_text_sharp(">", input_x + input_width - 10.0 * zoom, input_y + text_y_offset, font_size, GRAY);
            }

            // Blinking cursor at correct position within visible text
            let cursor_blink = (macroquad::time::get_time() * 2.0) as i32 % 2 == 0;
            if cursor_blink {
                let cursor_visible_pos = cursor_pos.saturating_sub(scroll_offset);
                let text_before_cursor: String = visible_text.chars().take(cursor_visible_pos).collect();
                let cursor_x = self.measure_text_sharp(&text_before_cursor, font_size).width;
                draw_line(
                    input_x + text_padding + cursor_x + 1.0,
                    input_y + 4.0 * zoom,
                    input_x + text_padding + cursor_x + 1.0,
                    input_y + input_height - 4.0 * zoom,
                    1.0,
                    WHITE,
                );
            }

            // Hint
            let hint = if state.ui_state.classic_controls {
                "Press Enter to send"
            } else {
                "Press Enter to send, Escape to cancel"
            };
            self.draw_text_sharp(hint, input_x, input_y + input_height + 12.0 * zoom, font_size, LIGHTGRAY);
        }
    }


    /// Render all interactive UI elements and return the layout for hit detection
    fn render_interactive_ui(&self, state: &GameState) -> UiLayout {
        let mut layout = UiLayout::new();
        let hovered = &state.ui_state.hovered_element;

        // Ground item clickable areas and hover labels (world-space, registered first)
        self.render_ground_item_overlays(state, hovered, &mut layout);

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

        // Chat log area is intentionally NOT registered for hit detection
        // so that clicks/hovers pass through to the game world beneath it

        // Quick slots and menu buttons - hide on mobile when crafting/shop panel is open
        let hide_bottom_bar = cfg!(target_os = "android") && state.ui_state.crafting_open;
        if !hide_bottom_bar {
            // Quick slots (always visible at bottom, above exp bar)
            self.render_quick_slots(state, hovered, &mut layout);

            // Menu buttons (bottom-right, above exp bar)
            self.render_menu_buttons(state, hovered, &mut layout);
        }

        // Chat button (top-left, above quest tracker) - mobile only
        #[cfg(target_os = "android")]
        {
            if let Some(tex) = &self.chat_small_icon {
                let icon_w = tex.width();
                let icon_h = tex.height();
                let padding = 6.0;
                let btn_size = icon_w.max(icon_h) + padding * 2.0;
                let btn_x = 10.0;
                let btn_y = 10.0;

                // Circular background
                let center_x = btn_x + btn_size / 2.0;
                let center_y = btn_y + btn_size / 2.0;
                let radius = btn_size / 2.0;
                draw_circle(center_x, center_y, radius, Color::new(0.1, 0.1, 0.13, 0.85));
                draw_circle_lines(center_x, center_y, radius, 1.0, Color::new(0.557, 0.424, 0.267, 1.0));

                // Icon at original size (no scaling)
                draw_texture(tex, btn_x + (btn_size - icon_w) / 2.0, btn_y + (btn_size - icon_h) / 2.0, WHITE);

                layout.add(UiElementId::ChatButton, macroquad::prelude::Rect::new(btn_x, btn_y, btn_size, btn_size));
            }
        }

        // Quest objective tracker (top-left)
        self.render_quest_tracker(state);

        // Quest completion notifications
        self.render_quest_completed(state);

        // Dialogue box (when active)
        if let Some(dialogue) = &state.ui_state.active_dialogue {
            self.render_dialogue(dialogue, hovered, &mut layout, state.ui_state.dialogue_scroll_offset, state.ui_state.dialogue_scrollbar_dragging);
        }

        // Gold drop dialog (when active)
        if let Some(ref dialog) = state.ui_state.gold_drop_dialog {
            self.render_gold_drop_dialog(dialog, state.inventory.gold, hovered, &mut layout);
        }

        // Altar offering panel (when active)
        if let Some(ref panel) = state.ui_state.altar_panel {
            self.render_altar_panel(panel, state, hovered, &mut layout);
        }

        // Prayer/Spell help overlay (on top of panels)
        self.render_prayer_help_overlay(state, hovered, &mut layout);

        // Render context menu on top of everything
        if let Some(ref context_menu) = state.ui_state.context_menu {
            self.render_context_menu(context_menu, state, &mut layout);
        } else {
            // Only render tooltips if context menu is not open
            self.render_item_tooltip(state);
            self.render_skill_tooltip(state, hovered);
            self.render_prayer_tooltip(state, hovered);

            // XP globe tooltip (calculate position to match render_ui exactly)
            if let Some(player) = state.get_local_player() {
                let margin = 12.0;
                let base_y = 25.0;
                let padding = 6.0;
                let font_size = 16.0;
                let tag_height = 22.0;

                // Calculate bar_width based on player name (same as render_ui)
                let name = &player.name;
                let level_text = format!(" Lv.{}", player.skills.total_level());
                let name_w = self.measure_text_sharp(name, font_size).width;
                let level_w = self.measure_text_sharp(&level_text, font_size).width;
                let total_text_w = name_w + level_w;
                let bar_width = (total_text_w + padding * 2.0).max(120.0);

                let (tooltip_sw, _) = virtual_screen_size();
                let bar_x = (tooltip_sw - bar_width - margin).floor();
                let globe_stats_y = base_y + tag_height / 2.0 + 8.0;
                self.render_xp_globe_tooltip(&state.xp_globes, bar_x, globe_stats_y);
            }
        }

        // Render dragged item at cursor (on top of everything)
        if let Some(ref drag) = state.ui_state.drag_state {
            self.render_dragged_item(drag, state);
        }

        // Render escape menu on top of everything
        if state.ui_state.escape_menu_open {
            self.render_escape_menu(state, &mut layout);
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
        draw_rectangle(x + FRAME_THICKNESS, y + FRAME_THICKNESS, w - FRAME_THICKNESS * 2.0, h - FRAME_THICKNESS * 2.0, PANEL_BG_MID);

        // Layer 5: Inner highlight line (top and left edges - light source simulation)
        draw_line(x + FRAME_THICKNESS, y + FRAME_THICKNESS, x + w - FRAME_THICKNESS, y + FRAME_THICKNESS, 1.0, FRAME_INNER);
        draw_line(x + FRAME_THICKNESS, y + FRAME_THICKNESS, x + FRAME_THICKNESS, y + h - FRAME_THICKNESS, 1.0, FRAME_INNER);

        // Layer 6: Inner shadow line (bottom and right edges)
        let shadow = Color::new(0.0, 0.0, 0.0, 0.235);
        draw_line(x + FRAME_THICKNESS + 1.0, y + h - FRAME_THICKNESS - 1.0, x + w - FRAME_THICKNESS, y + h - FRAME_THICKNESS - 1.0, 1.0, shadow);
        draw_line(x + w - FRAME_THICKNESS - 1.0, y + FRAME_THICKNESS + 1.0, x + w - FRAME_THICKNESS - 1.0, y + h - FRAME_THICKNESS, 1.0, shadow);
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
    fn draw_entity_health_bar(&self, x: f32, y: f32, width: f32, height: f32, hp_ratio: f32, _scale: f32) {
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
                let shine_width = (fill_width * 0.3).min(6.0).max(2.0).floor();
                let shine_color = Color::new(1.0, 1.0, 1.0, 0.4);
                draw_rectangle(x + 1.0, y + 1.0, shine_width, 1.0, shine_color);
            }
        }
    }

    /// Draw an inventory slot with bevel effect
    pub(crate) fn draw_inventory_slot(&self, x: f32, y: f32, size: f32, has_item: bool, state: SlotState) {
        // Outer slot border (bronze)
        draw_rectangle(x, y, size, size, SLOT_BORDER);

        // Inner recessed area (1px inset)
        let inner_x = x + 1.0;
        let inner_y = y + 1.0;
        let inner_size = size - 2.0;

        // Background based on state
        let bg = match state {
            SlotState::Normal => if has_item { SLOT_BG_FILLED } else { SLOT_BG_EMPTY },
            SlotState::Hovered => SLOT_HOVER_BG,
            SlotState::Dragging => SLOT_DRAG_SOURCE,
        };
        draw_rectangle(inner_x, inner_y, inner_size, inner_size, bg);

        // Inner shadow (top and left - simulates recessed slot)
        draw_line(inner_x, inner_y, inner_x + inner_size, inner_y, 2.0, SLOT_INNER_SHADOW);
        draw_line(inner_x, inner_y, inner_x, inner_y + inner_size, 2.0, SLOT_INNER_SHADOW);

        // Inner highlight (bottom and right - subtle)
        draw_line(inner_x + 1.0, inner_y + inner_size - 1.0, inner_x + inner_size, inner_y + inner_size - 1.0, 1.0, SLOT_HIGHLIGHT);
        draw_line(inner_x + inner_size - 1.0, inner_y + 1.0, inner_x + inner_size - 1.0, inner_y + inner_size, 1.0, SLOT_HIGHLIGHT);

        // State-specific border overlay
        match state {
            SlotState::Hovered => {
                draw_rectangle_lines(x, y, size, size, 2.0, SLOT_HOVER_BORDER);
            },
            SlotState::Dragging => {
                draw_rectangle_lines(x, y, size, size, 2.0, SLOT_SELECTED_BORDER);
            },
            _ => {}
        }
    }



    /// Draw an item icon using sprite or fallback color
    /// Uses the full texture, centered in the slot

    /// Render a dragged item following the cursor

    /// Word-wrap text to fit within a given width (approximate, assumes ~8px per char at size 16)
    /// Prefers breaking on word boundaries, but will break long words if necessary
    pub(crate) fn wrap_text(&self, text: &str, max_width: f32, font_size: f32) -> Vec<String> {
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
            chars.chunks(max_len)
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

    /// Render fade-in overlay when world first becomes ready
    pub fn render_world_fade_in(&self, state: &GameState) {
        if state.world_fade_in <= 0.0 {
            return;
        }
        let (sw, sh) = virtual_screen_size();
        let bg = Color::from_rgba(30, 30, 40, 255);
        draw_rectangle(
            0.0,
            0.0,
            sw,
            sh,
            Color::new(bg.r, bg.g, bg.b, state.world_fade_in),
        );
    }

    /// Render transition fade overlay
    pub fn render_transition_overlay(&self, state: &GameState) {
        use crate::game::state::TransitionState;

        if state.map_transition.state == TransitionState::None {
            return;
        }

        let alpha = state.map_transition.progress;
        let (trans_sw, trans_sh) = virtual_screen_size();
        draw_rectangle(
            0.0,
            0.0,
            trans_sw,
            trans_sh,
            Color::new(0.0, 0.0, 0.0, alpha),
        );
    }
}
