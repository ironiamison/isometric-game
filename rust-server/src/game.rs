use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

use crate::chunk::{CHUNK_SIZE, ChunkCoord};
use crate::data::ItemRegistry;
use crate::data::item_def::WeaponType;
use crate::entity::EntityRegistry;
use crate::item::{self, GroundItem, Inventory};
use crate::npc::{Npc, NpcUpdate};
use crate::prayer::PrayerRegistry;
use crate::protocol::{QuestCatalogEntryData, QuestObjectiveData, ServerMessage};
use crate::quest::{ObjectiveType, PlayerQuestState, QuestRegistry, QuestRunner};
use crate::shop::ShopRegistry;
use crate::skills::{SkillType, Skills, calculate_hit, calculate_max_hit, roll_damage};
use crate::world::World;
mod arena_tick;
mod auto_actions;
mod bank;
mod bootstrap;
mod boss_events;
pub(crate) mod boss_tick;
mod chairs;
mod chat;
mod chests;
mod combat;
mod crafting;
pub(crate) mod crafting_orders;
pub(crate) mod crate_loot;
mod farming;
mod grand_exchange;
mod instance_npc_tick;
mod interactions;
mod inventory;
pub(crate) mod koth_tick;
#[cfg(test)]
mod load_test;
mod movement;
mod movement_tick;
mod npc_speech;
mod npc_tick;
mod player_state;
mod post_movement;
mod prayer;
mod prestige_shop;
mod quests;
mod resource_contracts;
mod resources;
mod respawns;
mod room_io;
mod shop;
mod slayer;
mod social;
mod spells;
mod stall;
mod tick;
mod tick_auto_actions;
mod tick_resources;
mod tick_snapshots;
pub(crate) mod titles;
mod trade;
mod transport;
mod travel;
mod world_map;

use transport::{FULL_SYNC_INTERVAL, RoomTransport, full_sync_offset};

// ============================================================================
// Constants
// ============================================================================

const TICK_RATE: f32 = 20.0;

// Grid-based movement: ticks between tile moves (5 ticks * 50ms = 250ms per tile)
const MOVE_COOLDOWN_TICKS: u64 = 5;
// If a non-zero move intent is not refreshed within this window, clear it.
// Prevents "keeps moving after key-up" when stop/update packets are delayed.
const MOVE_INTENT_STALE_TIMEOUT_MS: u64 = 700;
// Warn when move input cadence is irregular while moving (ingress jitter signal).
const MOVE_INPUT_GAP_WARN_MS: u64 = 250;
const MOVE_INPUT_WARN_THROTTLE_MS: u64 = 2_000;

// Dash: 1 tile forward, 4 second cooldown (80 ticks at 20Hz)
const DASH_COOLDOWN_TICKS: u64 = 80;
const DASH_DISTANCE: i32 = 1;

const MAP_WIDTH: u32 = 32;
const MAP_HEIGHT: u32 = 32;
const STARTING_HP: i32 = 100;

// Combat constants
const ATTACK_RANGE: i32 = 1; // Maximum distance to attack (in tiles)
const ATTACK_COOLDOWN_MS: u64 = 700;
const RANGED_ATTACK_COOLDOWN_MS: u64 = 700;
const PLAYER_HP_REGEN_PERCENT: f32 = 2.0;
const REGEN_INTERVAL_MS: u64 = 15000;

// Prayer drain interval (60 ticks = 3 seconds at 20 ticks/second)
const PRAYER_DRAIN_INTERVAL_TICKS: u64 = 60;

// Mana regen interval (60 ticks = 3 seconds at 20 ticks/second)
const MANA_REGEN_INTERVAL_TICKS: u64 = 60;

// View distance for StateSync culling (Chebyshev distance in tiles)
const VIEW_DISTANCE: i32 = 40;
// Keep some sender queue headroom so StateSync doesn't starve lower-frequency critical updates.
const STATE_SYNC_MIN_QUEUE_CAPACITY: usize = 8;

fn is_within_view(source_x: i32, source_y: i32, target_x: i32, target_y: i32) -> bool {
    (source_x - target_x).abs().max((source_y - target_y).abs()) <= VIEW_DISTANCE
}

fn is_visible_event_recipient(
    source_instance: Option<&str>,
    source_x: i32,
    source_y: i32,
    recipient_instance: Option<&str>,
    recipient_x: i32,
    recipient_y: i32,
) -> bool {
    source_instance == recipient_instance
        && is_within_view(source_x, source_y, recipient_x, recipient_y)
}

// World spawn point (chunk 0,0) - where players respawn after death
pub const WORLD_SPAWN_X: i32 = 15;
pub const WORLD_SPAWN_Y: i32 = 4;
// Preload a small ring of overworld chunks near spawn at startup and on transitions
pub const SPAWN_PRELOAD_RADIUS: i32 = 3;

/// Compute a facing Direction from a delta (dx, dy) vector.
fn direction_from_delta(dx: i32, dy: i32) -> Direction {
    if dx == 0 && dy == 0 {
        return Direction::Down;
    }
    // Cardinal only — pick the dominant axis, break ties with vertical
    if dx.abs() > dy.abs() {
        if dx > 0 {
            Direction::Right
        } else {
            Direction::Left
        }
    } else if dy > 0 {
        Direction::Down
    } else {
        Direction::Up
    }
}

// ============================================================================
// Cooking Burn Helper
// ============================================================================

/// Check if a cooking recipe burns based on player level.
/// Returns true if the food should burn.
/// Burn chance: 50% at recipe's level_required, linearly decreasing to 0% at burn_stop_level.
fn check_burn(recipe: &crate::crafting::definition::RecipeDefinition, player_level: i32) -> bool {
    if let (Some(_burn_result), Some(burn_stop)) = (&recipe.burn_result, recipe.burn_stop_level) {
        if player_level >= burn_stop {
            return false;
        }
        let level_range = (burn_stop - recipe.level_required) as f64;
        if level_range <= 0.0 {
            return false;
        }
        let burn_chance = ((burn_stop - player_level) as f64 / level_range) * 0.5;
        rand::random::<f64>() < burn_chance
    } else {
        false
    }
}

// ============================================================================
// Crafting State (for timed crafting)
// ============================================================================

/// Tracks an active timed crafting operation
#[derive(Debug, Clone)]
pub struct CraftingState {
    pub recipe_id: String,
    pub started_at: std::time::Instant,
    pub duration_ms: u64,
    /// Materials consumed at start (for refund on cancel/interrupt)
    pub consumed_materials: Vec<(String, i32)>,
    /// Remaining crafts after this one completes (0 = single craft)
    pub batch_remaining: u32,
    /// Original total for batch progress display (1 = single craft)
    pub batch_total: u32,
}

// ============================================================================
// Player Save Data (for database persistence)
// ============================================================================

#[derive(Debug, Clone)]
pub struct PlayerSaveData {
    pub x: f32,
    pub y: f32,
    pub z: i32,
    pub hp: i32,
    pub prayer_points: i32,
    pub mp: i32,
    pub skills: Skills,
    pub gold: i32,
    pub inventory_json: String,
    pub gender: String,
    pub skin: String,
    pub equipped_head: Option<String>,
    pub equipped_body: Option<String>,
    pub equipped_weapon: Option<String>,
    pub equipped_back: Option<String>,
    pub equipped_feet: Option<String>,
    pub equipped_ring: Option<String>,
    pub equipped_gloves: Option<String>,
    pub equipped_necklace: Option<String>,
    pub equipped_belt: Option<String>,
    pub current_map: Option<String>,
    pub sitting_at_x: Option<i32>,
    pub sitting_at_y: Option<i32>,
    pub entrance_x: Option<f32>,
    pub entrance_y: Option<f32>,
    pub bank_json: String,
    pub bank_gold: i32,
    pub bank_max_slots: u32,
    pub combat_style_prefs: String, // JSON: {"melee":"aggressive","ranged":"rapid"}
}

/// Complete domain input for restoring a persisted character into a room.
///
/// Keeping this as a named payload prevents persistence column order from
/// leaking into gameplay APIs.
#[derive(Debug, Clone)]
pub struct PlayerRestoreData {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub hp: i32,
    pub prayer_points: i32,
    pub mp: i32,
    pub skills: Skills,
    pub gold: i32,
    pub inventory_json: String,
    pub gender: String,
    pub skin: String,
    pub hair_style: Option<i32>,
    pub hair_color: Option<i32>,
    pub equipped_head: Option<String>,
    pub equipped_body: Option<String>,
    pub equipped_weapon: Option<String>,
    pub equipped_back: Option<String>,
    pub equipped_feet: Option<String>,
    pub equipped_ring: Option<String>,
    pub equipped_gloves: Option<String>,
    pub equipped_necklace: Option<String>,
    pub equipped_belt: Option<String>,
    pub is_admin: bool,
    pub account_id: i64,
    pub ip_address: Option<String>,
    pub sitting_at_x: Option<i32>,
    pub sitting_at_y: Option<i32>,
    pub bank_json: String,
    pub bank_gold: i32,
    pub bank_max_slots: u32,
    pub combat_style_prefs_json: String,
}

// ============================================================================
// Direction
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Direction {
    Down = 0,
    Left = 1,
    Up = 2,
    Right = 3,
    DownLeft = 4,
    DownRight = 5,
    UpLeft = 6,
    UpRight = 7,
}

impl Direction {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "down" => Direction::Down,
            "up" => Direction::Up,
            "left" => Direction::Left,
            "right" => Direction::Right,
            "down_left" | "downleft" => Direction::DownLeft,
            "down_right" | "downright" => Direction::DownRight,
            "up_left" | "upleft" => Direction::UpLeft,
            "up_right" | "upright" => Direction::UpRight,
            _ => Direction::Down,
        }
    }

    pub fn from_velocity(dx: f32, dy: f32) -> Self {
        if dx == 0.0 && dy == 0.0 {
            return Direction::Down;
        }
        // Cardinal only — pick the dominant axis, break ties with vertical
        if dx.abs() > dy.abs() {
            if dx > 0.0 {
                Direction::Right
            } else {
                Direction::Left
            }
        } else if dy > 0.0 {
            Direction::Down
        } else {
            Direction::Up
        }
    }

    /// Snap any diagonal to its nearest cardinal direction.
    pub fn to_cardinal(self) -> Self {
        match self {
            Direction::UpLeft | Direction::Up => Direction::Up,
            Direction::UpRight | Direction::Right => Direction::Right,
            Direction::DownRight | Direction::Down => Direction::Down,
            Direction::DownLeft | Direction::Left => Direction::Left,
        }
    }

    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Direction::Down,
            1 => Direction::Left,
            2 => Direction::Up,
            3 => Direction::Right,
            4 => Direction::DownLeft,
            5 => Direction::DownRight,
            6 => Direction::UpLeft,
            7 => Direction::UpRight,
            _ => Direction::Down,
        }
    }
}

// ============================================================================
// Auto-Action System (OSRS-style click-to-act)
// ============================================================================

/// What the player is auto-acting on
#[derive(Debug, Clone)]
pub enum AutoActionTarget {
    Npc { npc_id: String },
    Player { player_id: String },
    Resource { x: i32, y: i32, gid: u32 },
}

/// What type of action to repeat
#[derive(Debug, Clone, PartialEq)]
pub enum AutoActionType {
    Attack,
    Mine,
    Chop,
}

/// Server-authoritative auto-action state. When set, the tick loop
/// automatically repeats the action whenever cooldown is ready and
/// the player is in range of the target.
#[derive(Debug, Clone)]
pub struct AutoAction {
    pub target: AutoActionTarget,
    pub action: AutoActionType,
    pub started_at: u64,
}

// ============================================================================
// Combat Styles
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum CombatStyle {
    #[default]
    Accurate,
    Aggressive,
    Defensive,
    Controlled,
    Rapid,
    Longrange,
}

impl CombatStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            CombatStyle::Accurate => "accurate",
            CombatStyle::Aggressive => "aggressive",
            CombatStyle::Defensive => "defensive",
            CombatStyle::Controlled => "controlled",
            CombatStyle::Rapid => "rapid",
            CombatStyle::Longrange => "longrange",
        }
    }

    pub fn from_str(s: &str) -> Option<CombatStyle> {
        match s.to_lowercase().as_str() {
            "accurate" => Some(CombatStyle::Accurate),
            "aggressive" => Some(CombatStyle::Aggressive),
            "defensive" => Some(CombatStyle::Defensive),
            "controlled" => Some(CombatStyle::Controlled),
            "rapid" => Some(CombatStyle::Rapid),
            "longrange" => Some(CombatStyle::Longrange),
            _ => None,
        }
    }

    /// Get available combat styles for a weapon type
    pub fn available_styles(weapon_type: WeaponType) -> &'static [CombatStyle] {
        match weapon_type {
            WeaponType::Melee => &[
                CombatStyle::Accurate,
                CombatStyle::Aggressive,
                CombatStyle::Defensive,
                CombatStyle::Controlled,
            ],
            WeaponType::Ranged => &[
                CombatStyle::Accurate,
                CombatStyle::Rapid,
                CombatStyle::Longrange,
            ],
        }
    }

    /// Check if this style is valid for the given weapon type
    pub fn is_valid_for(&self, weapon_type: WeaponType) -> bool {
        CombatStyle::available_styles(weapon_type).contains(self)
    }
}

// ============================================================================
// Player
// ============================================================================

#[derive(Debug, Clone)]
pub struct Player {
    pub id: String,
    pub name: String,
    // Grid position (integer tile coordinates)
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Whether the player is on the ground (can jump)
    pub grounded: bool,
    /// Remaining jump ticks (counts down from 6; rise for first 3, fall after)
    pub jump_ticks: u32,
    /// How many ticks the player has been falling (for accelerating gravity)
    pub fall_ticks: u32,
    pub spawn_x: i32,
    pub spawn_y: i32,
    // Queued movement direction (-1, 0, or 1)
    pub move_dx: i32,
    pub move_dy: i32,
    // Direction of last successful move (for stable StateSync vel)
    pub last_move_vel_x: i32,
    pub last_move_vel_y: i32,
    pub pending_move_seq: Option<u32>,
    pub last_received_move_seq: u32,
    pub last_processed_move_seq: u32,
    pub last_move_tick: u64, // Tick-based movement cooldown
    pub cast_stall_ticks: u64,
    pub last_move_input_ms: u64,
    pub last_move_input_warn_ms: u64,
    pub direction: Direction,
    pub hp: i32,
    pub skills: Skills,            // Combat skills (Hitpoints determines max HP)
    pub combat_style: CombatStyle, // Active combat style for XP distribution
    pub combat_style_prefs: HashMap<String, CombatStyle>, // Per-weapon-type preferences (e.g. "melee" -> Aggressive)
    pub active: bool,                                     // Whether WebSocket is connected
    pub target_id: Option<String>, // Currently targeted entity (player or NPC)
    pub last_attack_time: u64,     // Timestamp of last attack (ms)
    pub is_dead: bool,
    pub death_time: u64, // When the player died (for respawn timer)
    pub inventory: Inventory,
    // Character appearance
    pub gender: String,          // "male" or "female"
    pub skin: String,            // "tan", "pale", "brown", "purple", "orc", "ghost", "skeleton"
    pub hair_style: Option<i32>, // 0-5 (or None for bald)
    pub hair_color: Option<i32>, // 0-6 (color variant index)
    // Equipment
    pub equipped_head: Option<String>,
    pub equipped_body: Option<String>,
    pub equipped_weapon: Option<String>,
    pub equipped_back: Option<String>,
    pub equipped_feet: Option<String>,
    pub equipped_ring: Option<String>,
    pub equipped_gloves: Option<String>,
    pub equipped_necklace: Option<String>,
    pub equipped_belt: Option<String>,
    // Admin privileges
    pub is_admin: bool,
    pub is_god_mode: bool, // Invincibility for admins
    pub account_id: i64,
    pub ip_address: Option<String>,
    // HP regeneration tracking
    pub last_regen_time: u64,
    /// Tile coordinates of chair player is sitting on (None if not sitting)
    pub sitting_at: Option<(i32, i32)>,
    /// Recipes this player has discovered (for requires_discovery recipes)
    pub discovered_recipes: HashSet<String>,
    /// Spells this player has unlocked via scroll items
    pub unlocked_spells: HashSet<String>,
    /// Active timed crafting operation (None if not crafting)
    pub crafting_state: Option<CraftingState>,
    /// Current prayer points (drained by active prayers)
    pub prayer_points: i32,
    /// Set of currently active prayer IDs
    pub active_prayers: HashSet<String>,
    /// Current mana points
    pub mp: i32,
    /// Consecutive ticks this player has been blocked by another player (for ghosting through)
    pub ghost_player_ticks: u32,
    /// Per-spell cooldown tracking: spell_id -> last_cast_time_ms
    pub spell_cooldowns: HashMap<String, u64>,
    /// Active temporary buffs (from potions etc.), not persisted to DB
    pub active_buffs: Vec<ActiveBuff>,
    /// Bank vault for storing items safely
    pub bank: item::Bank,
    /// Current maximum bank slots (upgradeable from 50 to 100)
    pub bank_max_slots: u32,
    /// Tick when player last dashed (for cooldown)
    pub last_dash_tick: u64,
    /// True during the tick the player dashed (for StateSync broadcast)
    pub is_dashing: bool,
    /// Active auto-action (OSRS-style click-to-act). Processed each server tick.
    pub auto_action: Option<AutoAction>,
    /// Whether the player will auto-retaliate when attacked (OSRS-style)
    pub auto_retaliate: bool,
    /// Last time the player performed a manual action (ms). Used for 5-minute
    /// auto-retaliate idle timeout — after 5 min of only retaliating, stop.
    pub last_activity_time: u64,
    /// Active player stall (None if no stall open)
    pub stall: Option<PlayerStall>,
    /// Collection log: set of (item_id, source) pairs this player has obtained
    pub collection_log: HashSet<(String, String)>,
    /// Display text of equipped title (e.g. "Master Smith")
    pub active_title: Option<String>,
}

// ============================================================================
// Trade System
// ============================================================================

pub const TRADE_MAX_DISTANCE: i32 = 3;

#[derive(Debug, Clone)]
pub struct TradeOfferEntry {
    pub inv_slot: u8,
    pub item_id: String,
    pub quantity: i32,
}

#[derive(Debug, Clone)]
pub struct TradeOffer {
    pub items: Vec<TradeOfferEntry>,
    pub gold: i32,
    pub accepted: bool,
}

impl TradeOffer {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            gold: 0,
            accepted: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TradeSession {
    pub player_a: String,
    pub player_b: String,
    pub offer_a: TradeOffer,
    pub offer_b: TradeOffer,
}

// ============================================================================
// Player Stall System
// ============================================================================

pub const STALL_MAX_SLOTS: usize = 10;

#[derive(Debug, Clone)]
pub struct StallSlot {
    pub item_id: String,
    pub quantity: i32,
    pub price: i32,
}

#[derive(Debug, Clone)]
pub struct PlayerStall {
    pub name: String,
    pub slots: Vec<Option<StallSlot>>,
    pub active: bool,
}

impl PlayerStall {
    pub fn new(name: String) -> Self {
        Self {
            name,
            slots: (0..STALL_MAX_SLOTS).map(|_| None).collect(),
            active: false,
        }
    }
}

/// Unified spell representation for both static (const) and scroll-based spells.
/// Used at cast time to avoid duplicating casting logic.
struct ResolvedSpell {
    id: String,
    spell_type: crate::spell::SpellType,
    magic_level_req: Option<i32>, // None for scroll spells (gated by unlocked_spells)
    mana_cost: i32,
    cooldown_ms: u64,
    base_power: i32,
    effect_sprite: String,
    pushback_distance: i32,
    wall_slam_damage_per_tile: i32,
    is_scroll_spell: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActiveBuff {
    pub stat: String, // "attack", "strength", "defence"
    pub amount: i32,
    pub expires_at: u64, // millisecond timestamp when buff expires
    pub source_item_id: String,
}

const PLAYER_RESPAWN_TIME_MS: u64 = 5000; // 5 seconds to respawn
const AUTO_RETALIATE_IDLE_TIMEOUT_MS: u64 = 5 * 60 * 1000; // 5 minutes

impl Player {
    pub fn new(
        id: &str,
        name: &str,
        spawn_x: i32,
        spawn_y: i32,
        gender: &str,
        skin: &str,
        hair_style: Option<i32>,
        hair_color: Option<i32>,
    ) -> Self {
        let skills = Skills::new(); // HP 10, Attack/Strength/Defence 1
        Self {
            id: id.to_string(),
            name: name.to_string(),
            x: spawn_x,
            y: spawn_y,
            z: 0,
            grounded: true,
            jump_ticks: 0,
            fall_ticks: 0,
            spawn_x,
            spawn_y,
            move_dx: 0,
            move_dy: 0,
            last_move_vel_x: 0,
            last_move_vel_y: 0,
            pending_move_seq: None,
            last_received_move_seq: 0,
            last_processed_move_seq: 0,
            last_move_tick: 0,
            cast_stall_ticks: 0,
            last_move_input_ms: 0,
            last_move_input_warn_ms: 0,
            direction: Direction::Down,
            hp: skills.hitpoints.level, // HP = Hitpoints level
            prayer_points: 10 + skills.prayer.level, // Prayer points = 10 + Prayer level
            mp: 10 + skills.magic.level * 2, // Mana = 10 + magic_level * 2
            skills,
            combat_style: CombatStyle::default(),
            combat_style_prefs: HashMap::new(),
            active: false,
            target_id: None,
            last_attack_time: 0,
            is_dead: false,
            death_time: 0,
            inventory: Inventory::new(),
            gender: gender.to_string(),
            skin: skin.to_string(),
            hair_style,
            hair_color,
            equipped_head: None,
            equipped_body: None,
            equipped_weapon: None,
            equipped_back: None,
            equipped_feet: None,
            equipped_ring: None,
            equipped_gloves: None,
            equipped_necklace: None,
            equipped_belt: None,
            is_admin: false,
            is_god_mode: false,
            account_id: 0,
            ip_address: None,
            last_regen_time: 0,
            sitting_at: None,
            discovered_recipes: HashSet::new(),
            unlocked_spells: HashSet::new(),
            crafting_state: None,
            active_prayers: HashSet::new(),
            ghost_player_ticks: 0,
            spell_cooldowns: HashMap::new(),
            active_buffs: Vec::new(),
            bank: item::Bank::new(),
            bank_max_slots: item::DEFAULT_BANK_SIZE as u32,
            last_dash_tick: 0,
            is_dashing: false,
            auto_action: None,
            auto_retaliate: true,
            last_activity_time: 0,
            stall: None,
            collection_log: HashSet::new(),
            active_title: None,
        }
    }

    /// Max HP is determined by Hitpoints skill level
    pub fn max_hp(&self) -> i32 {
        self.skills.hitpoints.level
    }

    /// Max prayer points is 10 + Prayer skill level
    pub fn max_prayer_points(&self) -> i32 {
        10 + self.skills.prayer.level
    }

    /// Max mana points is determined by Magic skill level: 10 + level * 2
    pub fn max_mp(&self) -> i32 {
        10 + self.skills.magic.level * 2
    }

    /// Combat level calculated from all combat skills
    pub fn combat_level(&self) -> i32 {
        self.skills.combat_level()
    }

    /// Get all equipped item IDs for stat calculation
    fn all_equipped(&self) -> [&Option<String>; 9] {
        [
            &self.equipped_head,
            &self.equipped_body,
            &self.equipped_weapon,
            &self.equipped_back,
            &self.equipped_feet,
            &self.equipped_ring,
            &self.equipped_gloves,
            &self.equipped_necklace,
            &self.equipped_belt,
        ]
    }

    /// Calculate total attack bonus (accuracy) from equipped items
    pub fn attack_bonus(&self, item_registry: &ItemRegistry) -> i32 {
        let mut bonus = 0;
        for equipped in self.all_equipped() {
            if let Some(item_id) = equipped
                && let Some(def) = item_registry.get(item_id)
                && let Some(equip) = &def.equipment
            {
                bonus += equip.attack_bonus;
            }
        }
        bonus + self.buff_bonus("attack")
    }

    /// Calculate total strength bonus (max hit) from equipped items
    pub fn strength_bonus(&self, item_registry: &ItemRegistry) -> i32 {
        let mut bonus = 0;
        for equipped in self.all_equipped() {
            if let Some(item_id) = equipped
                && let Some(def) = item_registry.get(item_id)
                && let Some(equip) = &def.equipment
            {
                bonus += equip.strength_bonus;
            }
        }
        bonus + self.buff_bonus("strength")
    }

    /// Calculate total ranged strength bonus from equipped items
    pub fn ranged_strength_bonus(&self, item_registry: &ItemRegistry) -> i32 {
        let mut bonus = 0;
        for equipped in self.all_equipped() {
            if let Some(item_id) = equipped
                && let Some(def) = item_registry.get(item_id)
                && let Some(equip) = &def.equipment
            {
                bonus += equip.ranged_strength_bonus;
            }
        }
        bonus
    }

    /// Calculate total defence bonus from equipped items
    pub fn defence_bonus(&self, item_registry: &ItemRegistry) -> i32 {
        let mut bonus = 0;
        for equipped in self.all_equipped() {
            if let Some(item_id) = equipped
                && let Some(def) = item_registry.get(item_id)
                && let Some(equip) = &def.equipment
            {
                bonus += equip.defence_bonus;
            }
        }
        bonus + self.buff_bonus("defence")
    }

    /// Calculate total magic bonus from equipped items
    pub fn magic_bonus(&self, item_registry: &ItemRegistry) -> i32 {
        let mut bonus = 0;
        for equipped in self.all_equipped() {
            if let Some(item_id) = equipped
                && let Some(def) = item_registry.get(item_id)
                && let Some(equip) = &def.equipment
            {
                bonus += equip.magic_bonus;
            }
        }
        bonus + self.buff_bonus("magic")
    }

    /// Award combat XP based on damage dealt and active combat style.
    /// Focused styles: 4 XP/dmg to one skill. Controlled: 1.33 XP/dmg to each of atk/str/def.
    /// Rapid: 4 XP/dmg to Ranged. Longrange: 2 XP/dmg to Ranged + 2 XP/dmg to Defence.
    /// Accurate with ranged weapon: 4 XP/dmg to Ranged (not Attack).
    /// Hitpoints always gets 1.33 XP per damage.
    /// Returns a vector of (SkillType, xp_gained, total_xp, level, leveled_up) for skills that gained XP.
    pub fn award_combat_xp(
        &mut self,
        damage: i32,
        style: CombatStyle,
        weapon_type: WeaponType,
    ) -> Vec<(SkillType, i64, i64, i32, bool)> {
        use crate::skills::{
            ATTACK_XP_PER_DAMAGE, CONTROLLED_XP_PER_DAMAGE, DEFENCE_XP_PER_DAMAGE,
            HITPOINTS_XP_PER_DAMAGE, LONGRANGE_DEFENCE_XP_PER_DAMAGE,
            LONGRANGE_RANGED_XP_PER_DAMAGE, RANGED_XP_PER_DAMAGE, STRENGTH_XP_PER_DAMAGE,
        };

        let mut results = Vec::new();

        // Hitpoints XP = 1.33 per damage
        let hp_xp = (damage as f64 * HITPOINTS_XP_PER_DAMAGE) as i64;

        // Award combat XP based on style
        match style {
            CombatStyle::Accurate => {
                if weapon_type == WeaponType::Ranged {
                    // Accurate with bow gives Ranged XP
                    let xp = (damage as f64 * RANGED_XP_PER_DAMAGE) as i64;
                    let leveled = self.skills.ranged.add_xp(xp);
                    if leveled {
                        tracing::info!(
                            "{} leveled up Ranged to {}!",
                            self.name,
                            self.skills.ranged.level
                        );
                    }
                    results.push((
                        SkillType::Ranged,
                        xp,
                        self.skills.ranged.xp,
                        self.skills.ranged.level,
                        leveled,
                    ));
                } else {
                    let xp = (damage as f64 * ATTACK_XP_PER_DAMAGE) as i64;
                    let leveled = self.skills.attack.add_xp(xp);
                    if leveled {
                        tracing::info!(
                            "{} leveled up Attack to {}!",
                            self.name,
                            self.skills.attack.level
                        );
                    }
                    results.push((
                        SkillType::Attack,
                        xp,
                        self.skills.attack.xp,
                        self.skills.attack.level,
                        leveled,
                    ));
                }
            }
            CombatStyle::Aggressive => {
                let xp = (damage as f64 * STRENGTH_XP_PER_DAMAGE) as i64;
                let leveled = self.skills.strength.add_xp(xp);
                if leveled {
                    tracing::info!(
                        "{} leveled up Strength to {}!",
                        self.name,
                        self.skills.strength.level
                    );
                }
                results.push((
                    SkillType::Strength,
                    xp,
                    self.skills.strength.xp,
                    self.skills.strength.level,
                    leveled,
                ));
            }
            CombatStyle::Defensive => {
                let xp = (damage as f64 * DEFENCE_XP_PER_DAMAGE) as i64;
                let leveled = self.skills.defence.add_xp(xp);
                if leveled {
                    tracing::info!(
                        "{} leveled up Defence to {}!",
                        self.name,
                        self.skills.defence.level
                    );
                }
                results.push((
                    SkillType::Defence,
                    xp,
                    self.skills.defence.xp,
                    self.skills.defence.level,
                    leveled,
                ));
            }
            CombatStyle::Controlled => {
                let xp = (damage as f64 * CONTROLLED_XP_PER_DAMAGE) as i64;
                for (skill_type, skill) in [
                    (SkillType::Attack, &mut self.skills.attack),
                    (SkillType::Strength, &mut self.skills.strength),
                    (SkillType::Defence, &mut self.skills.defence),
                ] {
                    let leveled = skill.add_xp(xp);
                    if leveled {
                        tracing::info!(
                            "{} leveled up {} to {}!",
                            self.name,
                            skill_type.as_str(),
                            skill.level
                        );
                    }
                    results.push((skill_type, xp, skill.xp, skill.level, leveled));
                }
            }
            CombatStyle::Rapid => {
                let xp = (damage as f64 * RANGED_XP_PER_DAMAGE) as i64;
                let leveled = self.skills.ranged.add_xp(xp);
                if leveled {
                    tracing::info!(
                        "{} leveled up Ranged to {}!",
                        self.name,
                        self.skills.ranged.level
                    );
                }
                results.push((
                    SkillType::Ranged,
                    xp,
                    self.skills.ranged.xp,
                    self.skills.ranged.level,
                    leveled,
                ));
            }
            CombatStyle::Longrange => {
                let ranged_xp = (damage as f64 * LONGRANGE_RANGED_XP_PER_DAMAGE) as i64;
                let ranged_leveled = self.skills.ranged.add_xp(ranged_xp);
                if ranged_leveled {
                    tracing::info!(
                        "{} leveled up Ranged to {}!",
                        self.name,
                        self.skills.ranged.level
                    );
                }
                results.push((
                    SkillType::Ranged,
                    ranged_xp,
                    self.skills.ranged.xp,
                    self.skills.ranged.level,
                    ranged_leveled,
                ));

                let def_xp = (damage as f64 * LONGRANGE_DEFENCE_XP_PER_DAMAGE) as i64;
                let def_leveled = self.skills.defence.add_xp(def_xp);
                if def_leveled {
                    tracing::info!(
                        "{} leveled up Defence to {}!",
                        self.name,
                        self.skills.defence.level
                    );
                }
                results.push((
                    SkillType::Defence,
                    def_xp,
                    self.skills.defence.xp,
                    self.skills.defence.level,
                    def_leveled,
                ));
            }
        }

        // Award Hitpoints XP
        let old_hp_level = self.skills.hitpoints.level;
        let hp_leveled = self.skills.hitpoints.add_xp(hp_xp);
        if hp_leveled {
            // Hitpoints level up means max HP increased
            let new_max = self.skills.hitpoints.level;
            tracing::info!(
                "{} leveled up Hitpoints to {}! (Max HP: {})",
                self.name,
                new_max,
                new_max
            );
            // Heal the difference (new levels worth of HP)
            self.hp += new_max - old_hp_level;
        }
        results.push((
            SkillType::Hitpoints,
            hp_xp,
            self.skills.hitpoints.xp,
            self.skills.hitpoints.level,
            hp_leveled,
        ));

        results
    }

    pub fn is_alive(&self) -> bool {
        !self.is_dead && self.hp > 0
    }

    pub fn die(&mut self, current_time: u64) {
        self.is_dead = true;
        self.death_time = current_time;
        self.hp = 0;
        self.reject_pending_move();
        self.target_id = None;
        // Deactivate all prayers on death
        self.active_prayers.clear();
    }

    fn mark_move_seq_processed(&mut self, seq: u32) {
        if seq > self.last_processed_move_seq {
            self.last_processed_move_seq = seq;
        }
    }

    fn clear_move_intent(&mut self) {
        self.move_dx = 0;
        self.move_dy = 0;
        self.pending_move_seq = None;
    }

    /// Clear intent and also reset the last-move vel (explicit stop).
    fn stop_moving(&mut self) {
        self.clear_move_intent();
        self.last_move_vel_x = 0;
        self.last_move_vel_y = 0;
    }

    fn reject_pending_move(&mut self) {
        if let Some(seq) = self.pending_move_seq {
            self.mark_move_seq_processed(seq);
        }
        self.stop_moving();
    }

    pub fn ready_to_respawn(&self, current_time: u64) -> bool {
        self.is_dead && (current_time - self.death_time >= PLAYER_RESPAWN_TIME_MS)
    }

    /// Respawns the player, returning the chair coordinates they were sitting at (if any)
    /// so the caller can free the chair
    pub fn respawn(&mut self) -> Option<(i32, i32)> {
        let chair_to_free = self.sitting_at.take();
        // Always respawn at the world spawn point (chunk 0,0), not where they logged in
        self.x = WORLD_SPAWN_X;
        self.y = WORLD_SPAWN_Y;
        self.hp = self.max_hp(); // Use method since max_hp is now derived from skills
        self.prayer_points = self.max_prayer_points(); // Restore prayer points on respawn
        self.mp = self.max_mp(); // Restore mana on respawn
        self.is_dead = false;
        self.auto_action = None;
        self.death_time = 0;
        self.target_id = None;
        self.last_regen_time = 0;
        // Reset movement cooldown so the player can move immediately after respawning
        self.last_move_tick = 0;
        self.fall_ticks = 0;
        self.stop_moving();
        chair_to_free
    }

    /// Apply passive HP regeneration with optional prayer multiplier
    pub fn apply_regen(&mut self, current_time: u64, hp_regen_multiplier: f32) {
        if self.is_dead {
            return;
        }
        // First tick after spawn/respawn - just initialize timer, don't regen yet
        if self.last_regen_time == 0 {
            self.last_regen_time = current_time;
            return;
        }
        if current_time - self.last_regen_time >= REGEN_INTERVAL_MS {
            self.last_regen_time = current_time;
            let max_hp = self.max_hp();
            if self.hp < max_hp && self.hp > 0 {
                // Base regen calculation with prayer multiplier
                let base_regen = ((max_hp as f32 * PLAYER_HP_REGEN_PERCENT) / 100.0)
                    .ceil()
                    .max(1.0);
                let regen = (base_regen * hp_regen_multiplier).ceil() as i32;
                self.hp = (self.hp + regen).min(max_hp);
            }
        }
    }

    /// Get total buff bonus for a stat
    pub fn buff_bonus(&self, stat: &str) -> i32 {
        self.active_buffs
            .iter()
            .filter(|b| b.stat == stat)
            .map(|b| b.amount)
            .sum()
    }

    /// Apply a buff, replacing any existing buff for the same stat (no stacking)
    pub fn apply_buff(
        &mut self,
        stat: String,
        amount: i32,
        duration_ms: u64,
        current_time_ms: u64,
        source_item_id: String,
    ) {
        self.active_buffs.retain(|b| b.stat != stat);
        self.active_buffs.push(ActiveBuff {
            stat,
            amount,
            expires_at: current_time_ms + duration_ms,
            source_item_id,
        });
    }

    /// Remove expired buffs, returns true if any were removed
    pub fn tick_buffs(&mut self, current_time_ms: u64) -> bool {
        let before = self.active_buffs.len();
        self.active_buffs.retain(|b| b.expires_at > current_time_ms);
        self.active_buffs.len() != before
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlayerUpdate {
    pub id: String,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub direction: u8,
    // Velocity for client-side prediction (-1, 0, or 1)
    pub vel_x: i32,
    pub vel_y: i32,
    pub move_ack_seq: Option<u32>,
    pub hp: i32,
    pub max_hp: i32,
    pub combat_level: i32,
    // Individual skill levels
    pub hitpoints_level: i32,
    pub attack_level: i32,
    pub strength_level: i32,
    pub defence_level: i32,
    pub ranged_level: i32,
    pub gold: i32,
    // Character appearance
    pub gender: String,
    pub skin: String,
    pub hair_style: Option<i32>,
    pub hair_color: Option<i32>,
    // Equipment
    pub equipped_head: Option<String>,
    pub equipped_body: Option<String>,
    pub equipped_weapon: Option<String>,
    pub equipped_back: Option<String>,
    pub equipped_feet: Option<String>,
    pub equipped_ring: Option<String>,
    pub equipped_gloves: Option<String>,
    pub equipped_necklace: Option<String>,
    pub equipped_belt: Option<String>,
    // Admin status
    pub is_admin: bool,
    pub sitting: bool,
    pub is_gathering: bool,
    pub is_woodcutting: bool,
    pub dashing: bool,
    pub mp: i32,
    pub max_mp: i32,
    pub has_stall: bool,
    pub stall_name: Option<String>,
    pub combat_style: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TickTelemetry {
    pub active_players: usize,
    pub overworld_players: usize,
    pub instance_players: usize,
    pub spectators: usize,
    pub pending_moves: usize,
    pub rejected_moves: usize,
    pub rejected_tile_blocked: usize,
    pub rejected_player_blocked: usize,
    pub rejected_npc_blocked: usize,
    pub rejected_chair_blocked: usize,
    pub rejected_arena_blocked: usize,
    pub state_sync_send_attempts: usize,
    pub state_sync_capacity_skips: usize,
    pub state_sync_try_send_drops: usize,
    pub state_sync_full_sends: usize,
    pub state_sync_delta_sends: usize,
    pub state_sync_fallback_self_only_sends: usize,
    pub state_sync_raw_bytes: usize,
    pub state_sync_bytes_sent: usize,
    pub movement_stale_packets_ignored: usize,
    pub movement_seq_gap_events: usize,
    pub movement_input_gap_events: usize,
    pub movement_stale_intent_clears: usize,
    pub pre_npc_ms: u64,
    pub npc_world_ms: u64,
    pub state_sync_ms: u64,
    pub arena_ms: u64,
    pub chunk_unload_ms: u64,
    pub prayer_drain_ms: u64,
    pub farming_growth_ms: u64,
    pub restock_ms: u64,
}

#[derive(Default)]
struct MovementAnomalyCounters {
    stale_packets_ignored: AtomicU64,
    seq_gap_events: AtomicU64,
    input_gap_events: AtomicU64,
}

// ============================================================================
// Chair System
// ============================================================================

/// A chair's state on the map
#[derive(Debug, Clone)]
pub struct ChairState {
    pub direction: Direction,
    pub occupied_by: Option<String>,
}

/// Config for loading chairs from TOML
#[derive(Debug, Deserialize)]
struct ChairsConfig {
    chairs: Vec<ChairConfigEntry>,
}

#[derive(Debug, Deserialize)]
struct ChairConfigEntry {
    gid: u32,
    direction: String,
}

#[cfg(test)]
mod interaction_security_tests {
    use super::{
        DialogueGrant, VIEW_DISTANCE, consume_dialogue_choice, is_visible_event_recipient,
        is_within_view, same_interaction_context,
    };
    use std::collections::HashSet;

    #[test]
    fn dialogue_choices_require_the_exact_server_grant_and_are_single_use() {
        let mut grant = DialogueGrant {
            quest_id: "banker:npc_1".to_string(),
            npc_interaction: None,
            choices: HashSet::from(["open_bank".to_string()]),
        };

        assert!(!consume_dialogue_choice(
            &mut grant,
            "resource_contract_master:npc_370:lumberjack_pete",
            "contracts"
        ));
        assert!(!consume_dialogue_choice(
            &mut grant,
            "banker:npc_1",
            "upgrade"
        ));
        assert!(consume_dialogue_choice(
            &mut grant,
            "banker:npc_1",
            "open_bank"
        ));
        assert!(!consume_dialogue_choice(
            &mut grant,
            "banker:npc_1",
            "open_bank"
        ));
    }

    #[test]
    fn interaction_context_rejects_different_instances() {
        let instance_a = "instance-a".to_string();
        let instance_b = "instance-b".to_string();

        assert!(same_interaction_context(None, None));
        assert!(same_interaction_context(
            Some(&instance_a),
            Some(&instance_a)
        ));
        assert!(!same_interaction_context(None, Some(&instance_a)));
        assert!(!same_interaction_context(
            Some(&instance_a),
            Some(&instance_b)
        ));
    }

    #[test]
    fn positional_visibility_uses_the_state_sync_view_boundary() {
        assert!(is_within_view(10, 10, 10 + VIEW_DISTANCE, 10));
        assert!(is_within_view(
            10,
            10,
            10 + VIEW_DISTANCE,
            10 - VIEW_DISTANCE
        ));
        assert!(!is_within_view(10, 10, 11 + VIEW_DISTANCE, 10));
    }

    #[test]
    fn positional_visibility_rejects_other_instances_and_distant_players() {
        assert!(is_visible_event_recipient(None, 10, 10, None, 10, 10));
        assert!(!is_visible_event_recipient(
            None,
            10,
            10,
            None,
            11 + VIEW_DISTANCE,
            10
        ));
        assert!(!is_visible_event_recipient(
            Some("instance-a"),
            10,
            10,
            Some("instance-b"),
            10,
            10
        ));
    }
}

// ============================================================================
// Quest Locations (reach_location objectives)
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
struct QuestLocation {
    x: i32,
    y: i32,
    radius: i32,
}

#[derive(Debug, Clone)]
struct NpcInteractionGrant {
    npc_id: String,
    instance_id: Option<String>,
}

#[derive(Debug, Clone)]
struct DialogueGrant {
    quest_id: String,
    npc_interaction: Option<NpcInteractionGrant>,
    choices: HashSet<String>,
}

fn consume_dialogue_choice(grant: &mut DialogueGrant, quest_id: &str, choice_id: &str) -> bool {
    if grant.quest_id != quest_id || !grant.choices.contains(choice_id) {
        return false;
    }
    grant.choices.clear();
    true
}

fn same_interaction_context(first: Option<&String>, second: Option<&String>) -> bool {
    first == second
}

// ============================================================================
// Game Room
// ============================================================================

pub struct GameRoom {
    pub id: String,
    pub name: String,
    players: RwLock<HashMap<String, Player>>,
    npcs: RwLock<HashMap<String, Npc>>,
    ground_items: RwLock<HashMap<String, GroundItem>>,
    /// Ground item ids currently known by each client for visibility reconciliation.
    visible_ground_items: RwLock<HashMap<String, HashSet<String>>>,
    world: Arc<World>,
    /// Entity prototype registry for spawning and loot
    entity_registry: Arc<EntityRegistry>,
    /// Quest registry for quest definitions
    quest_registry: Arc<QuestRegistry>,
    /// Quest script runner for Lua execution
    quest_runner: Arc<QuestRunner>,
    /// Per-player quest state
    player_quest_states: RwLock<HashMap<String, PlayerQuestState>>,
    /// Crafting recipe registry
    crafting_registry: Arc<crate::crafting::CraftingRegistry>,
    /// Item definition registry
    item_registry: Arc<ItemRegistry>,
    /// Prayer definition registry
    prayer_registry: Arc<PrayerRegistry>,
    /// Shop registry for merchant NPCs
    shop_registry: RwLock<ShopRegistry>,
    /// Last time shops were restocked
    last_shop_restock: RwLock<std::time::Instant>,
    /// Track which chunk each player is in for streaming updates
    player_chunks: RwLock<HashMap<String, ChunkCoord>>,
    tick: RwLock<u64>,
    /// Connection channels and per-client synchronization baselines.
    transport: RoomTransport,
    /// Tracks which instance each player is currently in (None = overworld)
    player_instances: Arc<RwLock<HashMap<String, String>>>,
    /// Last server-validated NPC interaction for each player.
    npc_interaction_grants: RwLock<HashMap<String, NpcInteractionGrant>>,
    /// Dialogues and choices actually displayed by the server.
    dialogue_grants: RwLock<HashMap<String, DialogueGrant>>,
    /// Instance manager for looking up instance NPCs
    instance_manager: Arc<crate::instance::InstanceManager>,
    /// Arena duel manager (active when players are in duel_arena instance)
    arena_manager: RwLock<crate::arena::ArenaManager>,
    /// Active KOTH (King of the Hill) sessions: instance_id -> KothState
    koth_states: RwLock<crate::koth::KothStates>,
    /// Active boss fight sessions: instance_id -> BossState
    boss_states: RwLock<crate::boss::BossStates>,
    /// Active pharaoh boss fight sessions: instance_id -> PharaohBossState
    pharaoh_boss_states: RwLock<HashMap<String, crate::pharaoh_boss::PharaohBossState>>,
    /// Database reference for arena stats persistence
    db: Option<Arc<crate::db::Database>>,
    /// Gathering system (fishing)
    gathering: RwLock<crate::gathering::GatheringSystem>,
    /// Woodcutting system
    woodcutting: RwLock<crate::woodcutting::WoodcuttingSystem>,
    /// Mining system
    mining: RwLock<crate::mining::MiningSystem>,
    /// Chair GID -> direction mapping (loaded from config)
    chair_gids: HashMap<u32, Direction>,
    /// Chair positions on the map: (tile_x, tile_y) -> ChairState
    chairs: RwLock<HashMap<(i32, i32), ChairState>>,
    /// Farming system (allotment patches, crop growth)
    farming: RwLock<crate::farming::FarmingSystem>,
    /// Shared cross-skill resource contracts (farming, mining, woodcutting)
    resource_contracts: RwLock<crate::resource_contracts::ResourceContractManager>,
    /// Cached portal tile positions (immutable after init, no lock needed)
    portal_tiles: std::collections::HashSet<(i32, i32)>,
    /// Quest locations for reach_location objectives (location_id -> QuestLocation)
    quest_locations: HashMap<String, QuestLocation>,
    /// Slayer task/reward registry (loaded from data/slayer/)
    slayer_registry: Arc<crate::slayer::SlayerRegistry>,
    /// Per-player slayer state (current task, points, blocked/unlocked)
    player_slayer_states: RwLock<HashMap<String, crate::slayer::PlayerSlayerState>>,
    /// Interior registry for looking up map flags (e.g. requires_slayer_task)
    interior_registry: Arc<crate::interior::InteriorRegistry>,
    /// Scroll-exclusive spell definitions (loaded from TOML)
    scroll_spell_registry: Arc<crate::scroll_spell::ScrollSpellRegistry>,
    /// Persistent ground item spawn manager (respawning world items)
    ground_spawn_manager: RwLock<crate::ground_spawn::GroundSpawnManager>,
    /// Dig site manager for shovel-triggered quest events
    dig_site_manager: RwLock<crate::dig_site::DigSiteManager>,
    /// Waystone fast-travel manager (loaded from TOML)
    waystone_manager: RwLock<crate::waystone::WaystoneManager>,
    /// Chest definition registry (loaded from TOML)
    chest_registry: Arc<crate::chest::ChestRegistry>,
    /// Chest runtime state manager
    chest_manager: RwLock<crate::chest::ChestManager>,
    /// Tracks which chest each player has open (player_id -> chest_key)
    player_open_chests: RwLock<HashMap<String, String>>,
    /// Active trade sessions: trade_id -> TradeSession
    trades: RwLock<HashMap<String, TradeSession>>,
    /// Player -> trade_id mapping for quick lookup
    player_trades: RwLock<HashMap<String, String>>,
    /// Pending trade requests: target_id -> (requester_id, tick_when_sent)
    trade_requests: RwLock<HashMap<String, (String, u64)>>,
    /// Static overworld atlas + POIs for the expanded client world map.
    overworld_world_map: ServerMessage,
    /// Chunk coordinates where PVP is allowed in the overworld (allowlist)
    pvp_zones: HashSet<(i32, i32)>,
    /// Movement anomaly counters exported through /api/perf.
    movement_anomalies: MovementAnomalyCounters,
    /// Crafting order template registry (loaded from data/orders/)
    crafting_order_registry: crafting_orders::CraftingOrderRegistry,
    /// Crate loot table registry (loaded from data/crate_loot/)
    pub crate_loot_registry: crate_loot::CrateLootRegistry,
    /// Cached name of the all-time highest total level player (gold trophy)
    top_level_player_name: RwLock<Option<String>>,
    /// Cached total level value of the #1 player
    top_level_value: RwLock<i32>,
    /// Cached name of the 2nd highest total level player (silver trophy)
    second_level_player_name: RwLock<Option<String>>,
    /// Cached total level value of the #2 player
    second_level_value: RwLock<i32>,
}
