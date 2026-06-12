use super::animation::{
    get_back_static_frame, get_back_static_offset, get_body_armor_frame, get_body_armor_offset,
    get_boot_frame, get_boot_offset, get_hair_offset, get_head_frame, get_head_offset,
    get_offhand_frame, get_offhand_offset, get_weapon_frame, get_weapon_offset, Gender,
    NpcAnimation, NpcAnimationLayout, BACK_STATIC_SPRITE_HEIGHT, BACK_STATIC_SPRITE_WIDTH,
    BODY_ARMOR_SPRITE_HEIGHT, BODY_ARMOR_SPRITE_WIDTH, BOOT_SPRITE_HEIGHT, BOOT_SPRITE_WIDTH,
    HAIR_SPRITE_HEIGHT, HAIR_SPRITE_WIDTH, HEAD_SPRITE_HEIGHT, HEAD_SPRITE_WIDTH,
    OFFHAND_SPRITE_HEIGHT, OFFHAND_SPRITE_WIDTH, SPRITE_HEIGHT, SPRITE_WIDTH, WEAPON_SPRITE_HEIGHT,
    WEAPON_SPRITE_WIDTH,
};
use super::font::BitmapFont;
use super::isometric::{
    calculate_depth, calculate_depth_z, screen_to_world, world_to_screen, world_to_screen_exact,
    world_to_screen_z, TILE_HEIGHT, TILE_WIDTH,
};
use super::shaders;
use super::ui::common::{SlotState, CORNER_ACCENT_SIZE, EXP_BAR_GAP};
use crate::game::npc::{Npc, NpcState};
use crate::game::ore_types::get_ore_info;
use crate::game::tilemap::get_tile_color;
use crate::game::tree_types::get_tree_info;
use crate::game::world_map::{
    WorldMapSnapshot, WORLD_MAP_POI_KIND_CHEST, WORLD_MAP_POI_KIND_QUEST,
    WORLD_MAP_POI_KIND_SERVICE, WORLD_MAP_POI_KIND_TELEPORT, WORLD_MAP_POI_KIND_TREE,
};
use crate::game::{
    Camera, ChatChannel, ChunkLayerType, ConnectionStatus, Direction, DragSource, GameState,
    GroundItem, LayerType, MapObject, Player, Wall, WallEdge, CHUNK_SIZE,
};
use crate::ui::{UiElementId, UiLayout};
use crate::util::{asset_path, virtual_screen_size, SpriteAtlasInfo, SpriteManifest};
use macroquad::material::{
    gl_use_default_material, gl_use_material, load_material, Material, MaterialParams,
};
use macroquad::miniquad::ShaderSource;
use macroquad::miniquad::UniformDesc;
use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;
use macroquad::texture::{render_target_ex, RenderTargetParams};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

type TextMeasureCache = HashMap<i32, HashMap<String, TextDimensions>>;
type TextWrapCache = HashMap<(i32, i32), HashMap<String, Vec<String>>>;

mod asset_helpers;
mod asset_loaders;
mod assets;
mod core;
mod effects;
mod entities;
mod frame;
mod map_objects;
mod minimap_geometry;
mod minimap_render;
mod overlays;
mod player;
mod terrain;
mod transitions;
mod ui_frame;
mod ui_primitives;
mod world_effects;

use asset_loaders::*;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ChatLinesCacheKey {
    chat_revision: u64,
    max_chat_width_x100: i32,
    font_size_x100: i32,
    active_tab: u8,
    hide_system_in_public: bool,
}

#[derive(Default)]
struct ChatLinesCache {
    key: Option<ChatLinesCacheKey>,
    lines: Vec<(String, Color)>,
}

const TEXT_MEASURE_CACHE_BUCKET_LIMIT: usize = 2048;
const TEXT_WRAP_CACHE_BUCKET_LIMIT: usize = 512;

/// Tileset configuration
const TILESET_TILE_WIDTH: f32 = 64.0;
const TILESET_TILE_HEIGHT: f32 = 32.0;
const TILESET_COLUMNS: u32 = 32;

/// Available player appearance options
pub const GENDERS: &[&str] = &["male", "female"];
pub const SKINS: &[&str] = &["tan", "pale", "brown", "fish", "orc", "panda", "skeleton"];

/// Render target size for silhouette compositing (pixels at 1x scale).
/// Must be large enough to contain the player sprite plus equipment/weapon overhang.
const SILHOUETTE_RT_SIZE: u32 = 160;
/// Anchor X in the render target where the player sprite's draw_x maps to.
const SILHOUETTE_ANCHOR_X: f32 = 63.0;
/// Anchor Y in the render target where the player sprite's draw_y maps to.
const SILHOUETTE_ANCHOR_Y: f32 = 41.0;

/// Objects tileset firstgid (GID = firstGid + spriteFileId, matching mapper-config.json)
const OBJECTS_FIRSTGID: u32 = 1162;

/// Walls tileset firstgid (GID = firstGid + spriteFileId, matching mapper-config.json)
const WALLS_FIRSTGID: u32 = 1;

// ============================================================================
// Inventory UI Color Palette - Medieval Fantasy Theme
// ============================================================================

// Panel backgrounds (neutral dark gray, darker to lighter for depth)
const PANEL_BG_DARK: Color = Color::new(0.071, 0.071, 0.094, 0.961); // rgba(18, 18, 24, 245)
const PANEL_BG_MID: Color = Color::new(0.110, 0.110, 0.149, 1.0); // rgba(28, 28, 38, 255)
// Over-world HUD containers. The bottom bars use a near-solid dark fill so the gaps
// between buttons read as one cohesive unit; the chat box stays translucent. Bordered
// with a dark muted bronze (see draw_hud_tray).
const HUD_FILL_SOLID: Color = Color::new(0.071, 0.071, 0.094, 0.94);
const HUD_FILL_TRANSLUCENT: Color = Color::new(0.094, 0.094, 0.122, 0.62);
const HUD_BORDER: Color = Color::new(0.36, 0.27, 0.17, 1.0); // dark muted bronze

// Frame/Border colors (bronze/gold medieval theme)
const FRAME_OUTER: Color = Color::new(0.322, 0.243, 0.165, 1.0); // rgba(82, 62, 42, 255)
const FRAME_MID: Color = Color::new(0.557, 0.424, 0.267, 1.0); // rgba(142, 108, 68, 255)
const FRAME_INNER: Color = Color::new(0.729, 0.580, 0.361, 1.0); // rgba(186, 148, 92, 255)
const FRAME_ACCENT: Color = Color::new(0.855, 0.698, 0.424, 1.0); // rgba(218, 178, 108, 255)

// Header strip (matches ui::common) — used by the HUD stat cluster + minimap frame
const HEADER_BG: Color = Color::new(0.141, 0.125, 0.165, 1.0); // rgba(36, 32, 42, 255)
const HEADER_BORDER: Color = Color::new(0.463, 0.384, 0.267, 1.0); // rgba(118, 98, 68, 255)

// HUD stat bar fills - two-tone: the lighter MAIN tone fills most of the bar with a
// thin darker band along the bottom edge (a filled-vessel look).
const STAT_HP_MAIN: Color = Color::new(0.42, 0.66, 0.38, 1.0);
const STAT_HP_DARK: Color = Color::new(0.25, 0.46, 0.26, 1.0);
const STAT_MP_MAIN: Color = Color::new(0.49, 0.40, 0.78, 1.0);
const STAT_MP_DARK: Color = Color::new(0.31, 0.25, 0.56, 1.0);
const STAT_PRAYER_MAIN: Color = Color::new(0.37, 0.62, 0.68, 1.0);
const STAT_PRAYER_DARK: Color = Color::new(0.23, 0.45, 0.52, 1.0);

// Slot colors
const SLOT_BG_EMPTY: Color = Color::new(0.086, 0.086, 0.118, 1.0); // rgba(22, 22, 30, 255)
const SLOT_BG_FILLED: Color = Color::new(0.125, 0.125, 0.173, 1.0); // rgba(32, 32, 44, 255)
const SLOT_INNER_SHADOW: Color = Color::new(0.047, 0.047, 0.063, 1.0); // rgba(12, 12, 16, 255)
const SLOT_HIGHLIGHT: Color = Color::new(0.188, 0.188, 0.251, 1.0); // rgba(48, 48, 64, 255)
const SLOT_BORDER: Color = Color::new(0.227, 0.212, 0.188, 1.0); // rgba(58, 54, 48, 255)

// Hover/Selection states
const SLOT_HOVER_BG: Color = Color::new(0.188, 0.188, 0.282, 1.0); // rgba(48, 48, 72, 255)
const SLOT_HOVER_BORDER: Color = Color::new(0.659, 0.580, 0.424, 1.0); // rgba(168, 148, 108, 255)
const SLOT_SELECTED_BORDER: Color = Color::new(0.855, 0.737, 0.502, 1.0); // rgba(218, 188, 128, 255)
const SLOT_DRAG_SOURCE: Color = Color::new(0.314, 0.392, 0.627, 0.706); // rgba(80, 100, 160, 180)

// Text colors (used by stats panel)
const TEXT_TITLE: Color = Color::new(0.855, 0.737, 0.502, 1.0); // rgba(218, 188, 128, 255)
const TEXT_NORMAL: Color = Color::new(0.824, 0.824, 0.855, 1.0); // rgba(210, 210, 218, 255)
const TEXT_DIM: Color = Color::new(0.502, 0.502, 0.541, 1.0); // rgba(128, 128, 138, 255)

// Layout constant for draw_panel_frame helper
const FRAME_THICKNESS: f32 = 4.0;

// ============================================================================
// Health Bar Colors - Ornate Medieval Style
// ============================================================================

// Health bar frame (bronze-tinted dark metal)
const HEALTHBAR_FRAME_DARK: Color = Color::new(0.18, 0.14, 0.10, 1.0); // Dark bronze outline
const HEALTHBAR_FRAME_MID: Color = Color::new(0.35, 0.27, 0.18, 1.0); // Mid bronze
const HEALTHBAR_FRAME_LIGHT: Color = Color::new(0.55, 0.43, 0.28, 1.0); // Light bronze
const HEALTHBAR_FRAME_ACCENT: Color = Color::new(0.72, 0.58, 0.38, 1.0); // Gold highlight

// Health bar background (recessed dark)
const HEALTHBAR_BG_OUTER: Color = Color::new(0.04, 0.04, 0.05, 1.0); // Outer shadow
const HEALTHBAR_BG_INNER: Color = Color::new(0.08, 0.07, 0.09, 1.0); // Inner dark

// Health colors - rich jewel tones (dark/mid/light for gradient effect)
const HEALTH_GREEN_DARK: Color = Color::new(0.12, 0.45, 0.22, 1.0); // Emerald base
const HEALTH_GREEN_MID: Color = Color::new(0.20, 0.62, 0.32, 1.0); // Emerald bright
const HEALTH_GREEN_LIGHT: Color = Color::new(0.35, 0.78, 0.48, 1.0); // Emerald highlight

const HEALTH_YELLOW_DARK: Color = Color::new(0.65, 0.45, 0.08, 1.0); // Amber base
const HEALTH_YELLOW_MID: Color = Color::new(0.85, 0.62, 0.12, 1.0); // Amber bright
const HEALTH_YELLOW_LIGHT: Color = Color::new(0.95, 0.78, 0.25, 1.0); // Amber highlight

const HEALTH_RED_DARK: Color = Color::new(0.55, 0.12, 0.12, 1.0); // Ruby base
const HEALTH_RED_MID: Color = Color::new(0.75, 0.18, 0.18, 1.0); // Ruby bright
const HEALTH_RED_LIGHT: Color = Color::new(0.90, 0.35, 0.35, 1.0); // Ruby highlight

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
            SpriteStore::Atlas(atlas) => atlas.get(key).map(|(tex, rect)| (tex, Some(rect))),
            SpriteStore::Individual(map) => map.get(key).map(|tex| (tex, None)),
        }
    }

    /// Returns the number of sprites in this store.
    pub fn len(&self) -> usize {
        match self {
            SpriteStore::Atlas(atlas) => atlas.rects.len(),
            SpriteStore::Individual(map) => map.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
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
            SpritesheetStore::Individual(map) => map.get(key).map(|tex| (tex, None)),
        }
    }

    /// Get the spritesheet dimensions for a given key.
    /// For individual textures, returns the texture size.
    /// For atlas, returns the rect dimensions (the spritesheet size within the atlas).
    pub fn get_dimensions(&self, key: &str) -> Option<(f32, f32)> {
        match self {
            SpritesheetStore::Atlas { rects, .. } => rects.get(key).map(|r| (r.w, r.h)),
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

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

const MINIMAP_MARGIN: f32 = 12.0;
const MINIMAP_PREVIEW_Y: f32 = 8.0;
const MINIMAP_PREVIEW_WIDTH: f32 = 188.0;
const MINIMAP_PREVIEW_HEIGHT: f32 = 140.0;
const MINIMAP_WORLD_TEXT_SIZE: f32 = 16.0;
const MINIMAP_VISIBLE_CHUNK_RADIUS: f32 = 0.8;
const MINIMAP_PREVIEW_TILE_BUDGET: usize = 9_000;
const MINIMAP_PANEL_TILE_BUDGET: usize = 16_000;
const MINIMAP_PANEL_MIN_ZOOM: f32 = 1.0;
const MINIMAP_PANEL_MAX_ZOOM: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum MinimapMarkerKind {
    Player,
    Teleport,
    Enemy,
    Tree,
    Quest,
    Service,
    Chest,
}

#[derive(Clone, Debug)]
struct MinimapMarker {
    kind: MinimapMarkerKind,
    x: f32,
    y: f32,
    label: String,
    /// Index into map-icons.png sprite sheet (0-9). Used for icon rendering.
    icon_index: u8,
}

#[derive(Clone, Copy, Debug)]
struct MinimapBounds {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

impl MinimapBounds {
    fn width(&self) -> f32 {
        (self.max_x - self.min_x).max(1.0)
    }

    fn height(&self) -> f32 {
        (self.max_y - self.min_y).max(1.0)
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
    pub(crate) npc_sprites: SpritesheetStore,
    /// NPC sprites too large for the atlas, loaded individually
    npc_overflow_sprites: HashMap<String, Texture2D>,
    /// Set of entity types whose idle frames (1 and 3) are non-transparent
    npc_idle_anim_set: HashSet<String>,
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
    /// HUD stat-bar icons (16x16): health / magic / prayer
    pub(crate) health_stat_icon: Option<Texture2D>,
    pub(crate) magic_stat_icon: Option<Texture2D>,
    pub(crate) prayer_stat_icon: Option<Texture2D>,
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
    /// Arrow projectile spritesheet (arrow_angles.png: 7 types × 4 angles, 31x27 per frame)
    arrow_projectile_texture: Option<Texture2D>,
    /// Auto-retaliate toggle icon
    pub(crate) auto_retaliate_icon: Option<Texture2D>,
    /// Exit portal arrow textures for interior maps
    exit_arrow_up: Option<Texture2D>,
    exit_arrow_down: Option<Texture2D>,
    exit_arrow_left: Option<Texture2D>,
    exit_arrow_right: Option<Texture2D>,
    /// Cached wrapped chat lines to avoid rebuilding/wrapping every frame.
    chat_lines_cache: RefCell<ChatLinesCache>,
    /// CPU copy of the tileset texture for minimap color sampling.
    tileset_image_cache: RefCell<Option<Image>>,
    /// Cached minimap tint color per tile id.
    minimap_tile_color_cache: RefCell<HashMap<u32, Color>>,
    /// Cached text measurements bucketed by font size.
    text_measure_cache: RefCell<TextMeasureCache>,
    /// Cached wrapped lines bucketed by (max width, font size).
    text_wrap_cache: RefCell<TextWrapCache>,
    /// Font scale multiplier for UI text. Set to ui_scale before UI rendering,
    /// reset to 1.0 for world rendering. Snaps to nearest multiple of 8 for
    /// pixel-perfect monogram font rendering.
    pub(crate) font_scale: Cell<f32>,
    /// Deferred XP drop feed position (x, start_y) — set in render_ui, drawn after interactive UI
    xp_drop_pos: Cell<Option<(f32, f32)>>,
    /// Off-screen render target for compositing the player silhouette at full opacity
    silhouette_rt: RefCell<Option<RenderTarget>>,
    /// Animated object sprites: sprite_id -> frame_count
    animated_objects: HashMap<u32, u32>,
    /// Animated wall sprites: sprite_id -> frame_count
    animated_walls: HashMap<u32, u32>,
    /// Destination flag icon for minimap pathfinding destination marker
    destination_flag: Option<Texture2D>,
    /// Click effect spritesheets (4 frames, 16x16 each)
    click_walk_texture: Option<Texture2D>,
    click_attack_texture: Option<Texture2D>,
    click_interact_texture: Option<Texture2D>,
    /// Map icons sprite sheet (16x16 icons: dead_tree, oak, oak2, willow, maple, yew, quest, portal, enemy, station)
    map_icons: Option<Texture2D>,
    /// Pre-computed pixel-perfect outline textures for map icon hover state (18x18 per icon to accommodate 1px border)
    map_icons_outlines: Option<Texture2D>,
    /// Reusable lookup tables for tree/rock effects (cleared + rebuilt each frame to avoid allocations)
    falling_tree_positions: RefCell<HashSet<(i32, i32)>>,
    tree_shake_offsets: RefCell<HashMap<(i32, i32), f32>>,
    crumbling_rock_positions: RefCell<HashSet<(i32, i32)>>,
    rock_shake_offsets: RefCell<HashMap<(i32, i32), f32>>,
}

impl Renderer {}
