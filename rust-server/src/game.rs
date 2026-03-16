use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::{RwLock, broadcast, mpsc};
use uuid::Uuid;

use crate::chunk::ChunkCoord;
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
mod auto_actions;
mod bank;
mod chairs;
mod chat;
mod chests;
mod crafting;
mod farming;
mod instance_npc_tick;
mod inventory;
pub(crate) mod koth_tick;
pub(crate) mod boss_tick;
mod movement_tick;
mod npc_speech;
mod npc_tick;
mod post_movement;
mod prayer;
mod quests;
mod resources;
mod respawns;
mod shop;
mod slayer;
mod social;
mod stall;
mod tick_resources;
mod tick_snapshots;
mod trade;
mod travel;

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

// Dash: 1 tile forward, 5 second cooldown (100 ticks at 20Hz)
const DASH_COOLDOWN_TICKS: u64 = 100;
const DASH_DISTANCE: i32 = 1;

const MAP_WIDTH: u32 = 32;
const MAP_HEIGHT: u32 = 32;
const STARTING_HP: i32 = 100;

// Combat constants
const ATTACK_RANGE: i32 = 1; // Maximum distance to attack (in tiles)
const ATTACK_COOLDOWN_MS: u64 = 700; // Melee: slightly shorter than client's 800ms to absorb jitter
const RANGED_ATTACK_COOLDOWN_MS: u64 = 1000; // Ranged: slower to balance range advantage
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

// World spawn point (chunk 0,0) - where players respawn after death
pub const WORLD_SPAWN_X: i32 = 15;
pub const WORLD_SPAWN_Y: i32 = 4;
// Preload a small ring of overworld chunks near spawn at startup and on transitions
pub const SPAWN_PRELOAD_RADIUS: i32 = 3;

/// Compute a facing Direction from a delta (dx, dy) vector.
fn direction_from_delta(dx: i32, dy: i32) -> Direction {
    match (dx.signum(), dy.signum()) {
        (0, -1) => Direction::Up,
        (0, 1) => Direction::Down,
        (-1, 0) => Direction::Left,
        (1, 0) => Direction::Right,
        (-1, -1) => Direction::UpLeft,
        (1, -1) => Direction::UpRight,
        (-1, 1) => Direction::DownLeft,
        (1, 1) => Direction::DownRight,
        _ => Direction::Down, // fallback
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

        let angle = dy.atan2(dx);
        let octant =
            ((angle + std::f32::consts::PI) / (std::f32::consts::PI / 4.0)).round() as i32 % 8;

        match octant {
            0 => Direction::Left,
            1 => Direction::UpLeft,
            2 => Direction::Up,
            3 => Direction::UpRight,
            4 => Direction::Right,
            5 => Direction::DownRight,
            6 => Direction::Down,
            7 => Direction::DownLeft,
            _ => Direction::Down,
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
pub enum CombatStyle {
    Accurate,
    Aggressive,
    Defensive,
    Controlled,
    Rapid,
    Longrange,
}

impl Default for CombatStyle {
    fn default() -> Self {
        CombatStyle::Accurate
    }
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
            WeaponType::Melee => &[CombatStyle::Accurate, CombatStyle::Aggressive, CombatStyle::Defensive, CombatStyle::Controlled],
            WeaponType::Ranged => &[CombatStyle::Accurate, CombatStyle::Rapid, CombatStyle::Longrange],
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
    pub last_move_input_ms: u64,
    pub last_move_input_warn_ms: u64,
    pub direction: Direction,
    pub hp: i32,
    pub skills: Skills,            // Combat skills (Hitpoints determines max HP)
    pub combat_style: CombatStyle, // Active combat style for XP distribution
    pub combat_style_prefs: HashMap<String, CombatStyle>, // Per-weapon-type preferences (e.g. "melee" -> Aggressive)
    pub active: bool,              // Whether WebSocket is connected
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
    /// Active player stall (None if no stall open)
    pub stall: Option<PlayerStall>,
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
            last_regen_time: 0,
            sitting_at: None,
            discovered_recipes: HashSet::new(),
            unlocked_spells: HashSet::new(),
            crafting_state: None,
            active_prayers: HashSet::new(),
            spell_cooldowns: HashMap::new(),
            active_buffs: Vec::new(),
            bank: item::Bank::new(),
            bank_max_slots: item::DEFAULT_BANK_SIZE as u32,
            last_dash_tick: 0,
            is_dashing: false,
            auto_action: None,
            stall: None,
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
            if let Some(item_id) = equipped {
                if let Some(def) = item_registry.get(item_id) {
                    if let Some(equip) = &def.equipment {
                        bonus += equip.attack_bonus;
                    }
                }
            }
        }
        bonus + self.buff_bonus("attack")
    }

    /// Calculate total strength bonus (max hit) from equipped items
    pub fn strength_bonus(&self, item_registry: &ItemRegistry) -> i32 {
        let mut bonus = 0;
        for equipped in self.all_equipped() {
            if let Some(item_id) = equipped {
                if let Some(def) = item_registry.get(item_id) {
                    if let Some(equip) = &def.equipment {
                        bonus += equip.strength_bonus;
                    }
                }
            }
        }
        bonus + self.buff_bonus("strength")
    }

    /// Calculate total defence bonus from equipped items
    pub fn defence_bonus(&self, item_registry: &ItemRegistry) -> i32 {
        let mut bonus = 0;
        for equipped in self.all_equipped() {
            if let Some(item_id) = equipped {
                if let Some(def) = item_registry.get(item_id) {
                    if let Some(equip) = &def.equipment {
                        bonus += equip.defence_bonus;
                    }
                }
            }
        }
        bonus + self.buff_bonus("defence")
    }

    /// Calculate total magic bonus from equipped items
    pub fn magic_bonus(&self, item_registry: &ItemRegistry) -> i32 {
        let mut bonus = 0;
        for equipped in self.all_equipped() {
            if let Some(item_id) = equipped {
                if let Some(def) = item_registry.get(item_id) {
                    if let Some(equip) = &def.equipment {
                        bonus += equip.magic_bonus;
                    }
                }
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
    pub fn award_combat_xp(&mut self, damage: i32, style: CombatStyle, weapon_type: WeaponType) -> Vec<(SkillType, i64, i64, i32, bool)> {
        use crate::skills::{
            ATTACK_XP_PER_DAMAGE, STRENGTH_XP_PER_DAMAGE, DEFENCE_XP_PER_DAMAGE,
            CONTROLLED_XP_PER_DAMAGE, HITPOINTS_XP_PER_DAMAGE,
            RANGED_XP_PER_DAMAGE, LONGRANGE_RANGED_XP_PER_DAMAGE, LONGRANGE_DEFENCE_XP_PER_DAMAGE,
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
                        tracing::info!("{} leveled up Ranged to {}!", self.name, self.skills.ranged.level);
                    }
                    results.push((SkillType::Ranged, xp, self.skills.ranged.xp, self.skills.ranged.level, leveled));
                } else {
                    let xp = (damage as f64 * ATTACK_XP_PER_DAMAGE) as i64;
                    let leveled = self.skills.attack.add_xp(xp);
                    if leveled {
                        tracing::info!("{} leveled up Attack to {}!", self.name, self.skills.attack.level);
                    }
                    results.push((SkillType::Attack, xp, self.skills.attack.xp, self.skills.attack.level, leveled));
                }
            }
            CombatStyle::Aggressive => {
                let xp = (damage as f64 * STRENGTH_XP_PER_DAMAGE) as i64;
                let leveled = self.skills.strength.add_xp(xp);
                if leveled {
                    tracing::info!("{} leveled up Strength to {}!", self.name, self.skills.strength.level);
                }
                results.push((SkillType::Strength, xp, self.skills.strength.xp, self.skills.strength.level, leveled));
            }
            CombatStyle::Defensive => {
                let xp = (damage as f64 * DEFENCE_XP_PER_DAMAGE) as i64;
                let leveled = self.skills.defence.add_xp(xp);
                if leveled {
                    tracing::info!("{} leveled up Defence to {}!", self.name, self.skills.defence.level);
                }
                results.push((SkillType::Defence, xp, self.skills.defence.xp, self.skills.defence.level, leveled));
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
                        tracing::info!("{} leveled up {} to {}!", self.name, skill_type.as_str(), skill.level);
                    }
                    results.push((skill_type, xp, skill.xp, skill.level, leveled));
                }
            }
            CombatStyle::Rapid => {
                let xp = (damage as f64 * RANGED_XP_PER_DAMAGE) as i64;
                let leveled = self.skills.ranged.add_xp(xp);
                if leveled {
                    tracing::info!("{} leveled up Ranged to {}!", self.name, self.skills.ranged.level);
                }
                results.push((SkillType::Ranged, xp, self.skills.ranged.xp, self.skills.ranged.level, leveled));
            }
            CombatStyle::Longrange => {
                let ranged_xp = (damage as f64 * LONGRANGE_RANGED_XP_PER_DAMAGE) as i64;
                let ranged_leveled = self.skills.ranged.add_xp(ranged_xp);
                if ranged_leveled {
                    tracing::info!("{} leveled up Ranged to {}!", self.name, self.skills.ranged.level);
                }
                results.push((SkillType::Ranged, ranged_xp, self.skills.ranged.xp, self.skills.ranged.level, ranged_leveled));

                let def_xp = (damage as f64 * LONGRANGE_DEFENCE_XP_PER_DAMAGE) as i64;
                let def_leveled = self.skills.defence.add_xp(def_xp);
                if def_leveled {
                    tracing::info!("{} leveled up Defence to {}!", self.name, self.skills.defence.level);
                }
                results.push((SkillType::Defence, def_xp, self.skills.defence.xp, self.skills.defence.level, def_leveled));
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
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TickTelemetry {
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

// ============================================================================
// Delta Sync State (per-player tracking for bandwidth optimization)
// ============================================================================

const FULL_SYNC_INTERVAL: u64 = 20; // Force full sync every 20 ticks (1 second at 20Hz)

struct PlayerSyncState {
    last_players: HashMap<String, PlayerUpdate>,
    last_npcs: HashMap<String, NpcUpdate>,
    last_full_sync_tick: u64,
}

impl PlayerSyncState {
    fn new() -> Self {
        Self {
            last_players: HashMap::new(),
            last_npcs: HashMap::new(),
            last_full_sync_tick: 0,
        }
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

// ============================================================================
// Game Room
// ============================================================================

pub struct GameRoom {
    pub id: String,
    pub name: String,
    players: RwLock<HashMap<String, Player>>,
    npcs: RwLock<HashMap<String, Npc>>,
    ground_items: RwLock<HashMap<String, GroundItem>>,
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
    broadcast_tx: broadcast::Sender<Vec<u8>>,
    /// Per-player message senders for unicast (SECURITY: private inventory updates)
    player_senders: RwLock<HashMap<String, mpsc::Sender<Vec<u8>>>>,
    /// Per-player delta sync state for bandwidth optimization
    sync_states: RwLock<HashMap<String, PlayerSyncState>>,
    /// Tracks which instance each player is currently in (None = overworld)
    player_instances: Arc<RwLock<HashMap<String, String>>>,
    /// Instance manager for looking up instance NPCs
    instance_manager: Arc<crate::instance::InstanceManager>,
    /// Arena duel manager (active when players are in duel_arena instance)
    arena_manager: RwLock<crate::arena::ArenaManager>,
    /// Active KOTH (King of the Hill) sessions: instance_id -> KothState
    koth_states: RwLock<crate::koth::KothStates>,
    /// Active boss fight sessions: instance_id -> BossState
    boss_states: RwLock<crate::boss::BossStates>,
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
    /// Per-spectator message senders (read-only viewers on login screen)
    spectator_senders: RwLock<HashMap<String, mpsc::Sender<Vec<u8>>>>,
    /// Active trade sessions: trade_id -> TradeSession
    trades: RwLock<HashMap<String, TradeSession>>,
    /// Player -> trade_id mapping for quick lookup
    player_trades: RwLock<HashMap<String, String>>,
    /// Pending trade requests: target_id -> (requester_id, tick_when_sent)
    trade_requests: RwLock<HashMap<String, (String, u64)>>,
    /// Movement anomaly counters exported through /api/perf.
    movement_anomalies: MovementAnomalyCounters,
    /// Cached name of the all-time highest total level player (gold trophy)
    top_level_player_name: RwLock<Option<String>>,
    /// Cached total level value of the #1 player
    top_level_value: RwLock<i32>,
    /// Cached name of the 2nd highest total level player (silver trophy)
    second_level_player_name: RwLock<Option<String>>,
    /// Cached total level value of the #2 player
    second_level_value: RwLock<i32>,
}

impl GameRoom {
    pub async fn new(
        name: &str,
        entity_registry: Arc<EntityRegistry>,
        quest_registry: Arc<QuestRegistry>,
        crafting_registry: Arc<crate::crafting::CraftingRegistry>,
        item_registry: Arc<ItemRegistry>,
        prayer_registry: Arc<PrayerRegistry>,
        player_instances: Arc<RwLock<HashMap<String, String>>>,
        instance_manager: Arc<crate::instance::InstanceManager>,
        db: Option<Arc<crate::db::Database>>,
        interior_registry: Arc<crate::interior::InteriorRegistry>,
        chest_registry: Arc<crate::chest::ChestRegistry>,
    ) -> Self {
        let (tx, _) = broadcast::channel(256);
        let world = Arc::new(World::new("maps/world_0"));
        let (spawn_x, spawn_y) = world.get_spawn_position().await;
        let spawn_chunk = ChunkCoord::from_world(spawn_x, spawn_y);
        tracing::info!(
            "Preloading overworld chunks around spawn ({}, {}) at chunk ({}, {}) radius {}",
            spawn_x,
            spawn_y,
            spawn_chunk.x,
            spawn_chunk.y,
            SPAWN_PRELOAD_RADIUS
        );
        world
            .preload_chunks(spawn_chunk, SPAWN_PRELOAD_RADIUS)
            .await;

        // Create quest runner with the registry
        let quest_runner = Arc::new(QuestRunner::new(quest_registry.clone()));

        // Load all chunks and spawn NPCs from entity_spawns
        let mut npcs = HashMap::new();
        let mut npc_counter = 0u32;

        // Discover all chunk files and load entities from each
        let chunk_coords = world.discover_chunk_coords();
        tracing::info!("Discovered {} chunk files", chunk_coords.len());

        for coord in &chunk_coords {
            if let Some(chunk) = world.get_or_load_chunk(*coord).await {
                for spawn in &chunk.entity_spawns {
                    let npc_id = spawn
                        .unique_id
                        .clone()
                        .unwrap_or_else(|| format!("npc_{}", npc_counter));
                    npc_counter += 1;

                    if let Some(prototype) = entity_registry.get(&spawn.entity_id) {
                        // Use spawn's level if specified, otherwise use prototype's level
                        let level = spawn.level.unwrap_or(prototype.stats.level);
                        tracing::info!(
                            "Spawning {} at ({}, {}) level {}",
                            spawn.entity_id,
                            spawn.world_x,
                            spawn.world_y,
                            level
                        );
                        let npc = Npc::from_prototype(
                            &npc_id,
                            &spawn.entity_id,
                            prototype,
                            spawn.world_x,
                            spawn.world_y,
                            level,
                            spawn.facing.as_deref(),
                        );
                        npcs.insert(npc_id, npc);
                    } else {
                        tracing::warn!("Prototype '{}' not found, skipping spawn", spawn.entity_id);
                    }
                }
            }
        }

        tracing::info!("Spawned {} NPCs from chunk entity_spawns", npcs.len());

        // Load shop registry
        let mut shop_registry = ShopRegistry::new();
        if let Err(e) = shop_registry.load_from_directory(std::path::Path::new("data/shops")) {
            tracing::error!("Failed to load shop registry: {}", e);
        }
        tracing::info!("Loaded {} shop definitions", shop_registry.len());

        // Load gathering system
        let mut gathering =
            match crate::gathering::GatheringSystem::load(std::path::Path::new("data")) {
                Ok(g) => {
                    tracing::info!("Loaded gathering system with {} zones", g.zones.len());
                    g
                }
                Err(e) => {
                    tracing::warn!("Failed to load gathering system: {} (using empty)", e);
                    crate::gathering::GatheringSystem::new()
                }
            };

        // Load gathering markers from chunk data
        let mut chunk_marker_count = 0;
        for coord in &chunk_coords {
            if let Some(chunk) = world.get_or_load_chunk(*coord).await {
                for gz in &chunk.gathering_zones {
                    gathering.add_marker(crate::gathering::GatheringMarker {
                        x: gz.world_x,
                        y: gz.world_y,
                        zone_id: gz.zone_id.clone(),
                    });
                    chunk_marker_count += 1;
                }
            }
        }
        if chunk_marker_count > 0 {
            tracing::info!(
                "Loaded {} gathering markers from chunk data",
                chunk_marker_count
            );
        }

        // Cache portal tile positions (immutable, computed once at startup)
        let mut portal_tiles = std::collections::HashSet::new();
        for coord in &chunk_coords {
            if let Some(chunk) = world.get_or_load_chunk(*coord).await {
                let base_x = coord.x * crate::chunk::CHUNK_SIZE as i32;
                let base_y = coord.y * crate::chunk::CHUNK_SIZE as i32;
                for portal in &chunk.portals {
                    for dx in 0..portal.width {
                        for dy in 0..portal.height {
                            portal_tiles.insert((base_x + portal.x + dx, base_y + portal.y + dy));
                        }
                    }
                }
            }
        }
        if !portal_tiles.is_empty() {
            tracing::info!(
                "Cached {} portal tiles for NPC collision",
                portal_tiles.len()
            );
        }

        // Load quest locations for reach_location objectives
        let quest_locations: HashMap<String, QuestLocation> =
            match std::fs::read_to_string("data/quest_locations.toml") {
                Ok(content) => match toml::from_str::<HashMap<String, QuestLocation>>(&content) {
                    Ok(locs) => {
                        tracing::info!("Loaded {} quest locations", locs.len());
                        locs
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse quest_locations.toml: {}", e);
                        HashMap::new()
                    }
                },
                Err(_) => {
                    tracing::info!("No quest_locations.toml found, skipping");
                    HashMap::new()
                }
            };

        // Load woodcutting system
        let woodcutting =
            match crate::woodcutting::WoodcuttingSystem::load(std::path::Path::new("data")) {
                Ok(w) => {
                    tracing::info!(
                        "Loaded woodcutting system with {} tree types",
                        w.tree_types.len()
                    );
                    w
                }
                Err(e) => {
                    tracing::warn!("Failed to load woodcutting system: {} (using empty)", e);
                    crate::woodcutting::WoodcuttingSystem::new()
                }
            };

        // Load mining system
        let mining = match crate::mining::MiningSystem::load(std::path::Path::new("data")) {
            Ok(m) => {
                tracing::info!("Loaded mining system with {} ore types", m.ore_types.len());
                m
            }
            Err(e) => {
                tracing::warn!("Failed to load mining system: {} (using empty)", e);
                crate::mining::MiningSystem::new()
            }
        };

        // Load farming system
        let mut farming = match crate::farming::FarmingSystem::load(std::path::Path::new("data")) {
            Ok(f) => {
                tracing::info!(
                    "Loaded farming system with {} crops, {} patches",
                    f.crops.len(),
                    f.patches.len()
                );
                f
            }
            Err(e) => {
                tracing::warn!("Failed to load farming system: {} (using empty)", e);
                crate::farming::FarmingSystem::new()
            }
        };

        // Restore planted patches from database
        if let Some(ref db) = db {
            match db.load_farming_patches().await {
                Ok(saved_patches) => {
                    let count = saved_patches.len();
                    for (patch_id, player_id, crop_id, planted_at) in saved_patches {
                        farming.restore_patch(&patch_id, &player_id, &crop_id, planted_at);
                    }
                    if count > 0 {
                        tracing::info!("Restored {} planted farming patches from database", count);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to load farming patches from database: {}", e);
                }
            }
        }

        // Load plot unlocks from database
        if let Some(ref db) = db {
            match db.load_plot_unlocks().await {
                Ok(unlocks) => {
                    let count = unlocks.len();
                    for (player_id, plot_id) in &unlocks {
                        farming.restore_plot_unlock(player_id, *plot_id);
                    }
                    if count > 0 {
                        tracing::info!("Restored {} farming plot unlocks from database", count);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to load farming plot unlocks from database: {}", e);
                }
            }
        }

        // Load farming contracts from database
        if let Some(ref db) = db {
            match db.load_farming_contracts().await {
                Ok(contracts) => {
                    let count = contracts.len();
                    for (
                        player_id,
                        difficulty,
                        crop_id,
                        amount_required,
                        amount_harvested,
                        created_at,
                    ) in &contracts
                    {
                        farming.restore_contract(
                            player_id,
                            difficulty,
                            crop_id,
                            *amount_required,
                            *amount_harvested,
                            *created_at,
                        );
                    }
                    if count > 0 {
                        tracing::info!("Restored {} farming contracts from database", count);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to load farming contracts from database: {}", e);
                }
            }
        }

        // Load slayer registry
        let slayer_registry =
            match crate::slayer::SlayerRegistry::load(std::path::Path::new("data")) {
                Ok(r) => Arc::new(r),
                Err(e) => {
                    tracing::warn!(
                        "Failed to load slayer registry: {}, using empty registry",
                        e
                    );
                    Arc::new(crate::slayer::SlayerRegistry::empty())
                }
            };

        // Load chair config
        let mut chair_gids: HashMap<u32, Direction> = HashMap::new();
        match std::fs::read_to_string("data/chairs.toml") {
            Ok(content) => match toml::from_str::<ChairsConfig>(&content) {
                Ok(config) => {
                    for entry in config.chairs {
                        let dir = match entry.direction.as_str() {
                            "down" => Direction::Down,
                            "left" => Direction::Left,
                            "up" => Direction::Up,
                            "right" => Direction::Right,
                            _ => Direction::Down,
                        };
                        chair_gids.insert(entry.gid, dir);
                    }
                    tracing::info!("Loaded {} chair GID definitions", chair_gids.len());
                }
                Err(e) => tracing::warn!("Failed to parse chairs.toml: {}", e),
            },
            Err(e) => tracing::warn!("Failed to read chairs.toml: {} (no chairs)", e),
        }

        // Populate chair positions from chunk map objects
        let mut chairs: HashMap<(i32, i32), ChairState> = HashMap::new();
        for coord in &chunk_coords {
            if let Some(chunk) = world.get_or_load_chunk(*coord).await {
                for obj in &chunk.objects {
                    if let Some(&dir) = chair_gids.get(&obj.gid) {
                        chairs.insert(
                            (obj.tile_x, obj.tile_y),
                            ChairState {
                                direction: dir,
                                occupied_by: None,
                            },
                        );
                    }
                }
            }
        }
        if !chairs.is_empty() {
            tracing::info!("Found {} chairs on the map", chairs.len());
        }

        // Load scroll spell registry
        let mut scroll_spell_registry = crate::scroll_spell::ScrollSpellRegistry::new();
        let scroll_spells_path = std::path::Path::new("data/spells/scroll_spells.toml");
        if scroll_spells_path.exists() {
            if let Err(e) = scroll_spell_registry.load_from_file(scroll_spells_path) {
                tracing::error!("Failed to load scroll spell registry: {}", e);
            }
        }

        // Load persistent ground spawn definitions and create initial ground items
        let mut ground_spawn_manager =
            crate::ground_spawn::GroundSpawnManager::load(std::path::Path::new("data"));
        let mut ground_items = HashMap::new();
        {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            let initial_spawns = ground_spawn_manager.get_initial_spawns();
            for (spawn_id, item_id, x, y, quantity, instance_id) in initial_spawns {
                let ground_item_id = format!("persistent_{}", spawn_id);
                let ground_item = crate::item::GroundItem::new_in_instance(
                    &ground_item_id,
                    &item_id,
                    x,
                    y,
                    quantity,
                    None,
                    current_time,
                    instance_id,
                );
                ground_spawn_manager.set_active_ground_item(&spawn_id, ground_item_id.clone());
                ground_items.insert(ground_item_id, ground_item);
            }
            if !ground_items.is_empty() {
                tracing::info!(
                    "Created {} persistent ground items from spawns",
                    ground_items.len()
                );
            }
        }

        // Load overworld chest spawns from TOML
        let overworld_chest_spawns = {
            let spawns_path = std::path::Path::new("data/chest_spawns.toml");
            if spawns_path.exists() {
                let content = std::fs::read_to_string(spawns_path).unwrap_or_default();
                let file: crate::chest::ChestSpawnsFile =
                    toml::from_str(&content).unwrap_or_else(|e| {
                        tracing::warn!("Failed to parse chest_spawns.toml: {}", e);
                        crate::chest::ChestSpawnsFile { chests: Vec::new() }
                    });
                file.chests
            } else {
                Vec::new()
            }
        };

        // Collect interior chest placements from interior_registry
        let mut interior_chests = Vec::new();
        for id in interior_registry.list_ids() {
            if let Some(interior) = interior_registry.get(id) {
                tracing::debug!(
                    "Interior '{}' has {} chests defined",
                    id,
                    interior.chests.len()
                );
                for chest_spawn in &interior.chests {
                    tracing::info!(
                        "Interior '{}' chest: {} at ({}, {})",
                        id,
                        chest_spawn.chest_id,
                        chest_spawn.x,
                        chest_spawn.y
                    );
                    interior_chests.push((
                        id.clone(),
                        chest_spawn.chest_id.clone(),
                        chest_spawn.x,
                        chest_spawn.y,
                    ));
                }
            }
        }
        tracing::info!(
            "Collected {} interior chest placements",
            interior_chests.len()
        );

        // Create ChestManager and load saved data
        let mut chest_manager = crate::chest::ChestManager::new();
        chest_manager.init_from_registry(
            &chest_registry,
            &overworld_chest_spawns,
            &interior_chests,
        );
        if let Some(ref db) = db {
            match db.load_all_chests().await {
                Ok(saved) => {
                    chest_manager.load_saved_data(&saved);
                    tracing::info!("Loaded {} saved chest states", saved.len());
                }
                Err(e) => tracing::warn!("Failed to load chest data: {}", e),
            }
        }

        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            players: RwLock::new(HashMap::new()),
            npcs: RwLock::new(npcs),
            ground_items: RwLock::new(ground_items),
            world,
            entity_registry,
            quest_registry,
            quest_runner,
            player_quest_states: RwLock::new(HashMap::new()),
            crafting_registry,
            item_registry,
            prayer_registry,
            shop_registry: RwLock::new(shop_registry),
            last_shop_restock: RwLock::new(std::time::Instant::now()),
            player_chunks: RwLock::new(HashMap::new()),
            tick: RwLock::new(0),
            broadcast_tx: tx,
            player_senders: RwLock::new(HashMap::new()),
            sync_states: RwLock::new(HashMap::new()),
            player_instances,
            instance_manager,
            arena_manager: RwLock::new(crate::arena::ArenaManager::new(
                crate::arena::ArenaConfig::default(),
            )),
            koth_states: RwLock::new(std::collections::HashMap::new()),
            boss_states: RwLock::new(std::collections::HashMap::new()),
            db,
            gathering: RwLock::new(gathering),
            woodcutting: RwLock::new(woodcutting),
            mining: RwLock::new(mining),
            chair_gids,
            chairs: RwLock::new(chairs),
            farming: RwLock::new(farming),
            portal_tiles,
            quest_locations,
            slayer_registry,
            player_slayer_states: RwLock::new(HashMap::new()),
            interior_registry,
            scroll_spell_registry: Arc::new(scroll_spell_registry),
            ground_spawn_manager: RwLock::new(ground_spawn_manager),
            dig_site_manager: RwLock::new(crate::dig_site::DigSiteManager::load(
                std::path::Path::new("data"),
            )),
            waystone_manager: RwLock::new(crate::waystone::WaystoneManager::load(
                std::path::Path::new("data"),
            )),
            chest_registry,
            chest_manager: RwLock::new(chest_manager),
            player_open_chests: RwLock::new(HashMap::new()),
            spectator_senders: RwLock::new(HashMap::new()),
            trades: RwLock::new(HashMap::new()),
            player_trades: RwLock::new(HashMap::new()),
            trade_requests: RwLock::new(HashMap::new()),
            movement_anomalies: MovementAnomalyCounters::default(),
            top_level_player_name: RwLock::new(None),
            top_level_value: RwLock::new(0),
            second_level_player_name: RwLock::new(None),
            second_level_value: RwLock::new(0),
        }
    }

    /// Load the top two total level players from the database at startup.
    pub async fn init_top_level_player(&self) {
        if let Some(ref db) = self.db {
            let (first, second) = db.get_top_total_level_players().await;
            if let Some((name, total)) = first {
                tracing::info!("Top total level player: {} (level {})", name, total);
                *self.top_level_player_name.write().await = Some(name);
                *self.top_level_value.write().await = total;
            } else {
                tracing::info!("No characters found for top level player");
            }
            if let Some((name, total)) = second {
                tracing::info!("2nd total level player: {} (level {})", name, total);
                *self.second_level_player_name.write().await = Some(name);
                *self.second_level_value.write().await = total;
            }
        }
    }

    /// Check if a player's new total level changes the #1 or #2 rankings and broadcast if so.
    pub async fn check_top_player_after_level_up(&self, player_name: &str, new_total_level: i32) {
        let current_top = *self.top_level_value.read().await;
        let current_second = *self.second_level_value.read().await;
        let is_current_top = self.top_level_player_name.read().await.as_deref() == Some(player_name);
        let is_current_second = self.second_level_player_name.read().await.as_deref() == Some(player_name);

        let mut changed = false;

        if new_total_level > current_top {
            // New #1 — old #1 becomes #2 (unless it's the same player updating their own score)
            if !is_current_top {
                let old_first_name = self.top_level_player_name.read().await.clone();
                let old_first_val = current_top;
                *self.second_level_player_name.write().await = old_first_name;
                *self.second_level_value.write().await = old_first_val;
            }
            *self.top_level_player_name.write().await = Some(player_name.to_string());
            *self.top_level_value.write().await = new_total_level;
            changed = true;
        } else if is_current_top {
            // Current #1 leveled up but still #1 — just update value
            *self.top_level_value.write().await = new_total_level;
        } else if new_total_level > current_second {
            // New #2
            *self.second_level_player_name.write().await = Some(player_name.to_string());
            *self.second_level_value.write().await = new_total_level;
            changed = true;
        } else if is_current_second {
            // Current #2 leveled up but still #2 — just update value
            *self.second_level_value.write().await = new_total_level;
        }

        if changed {
            let first = self.top_level_player_name.read().await.clone();
            let second = self.second_level_player_name.read().await.clone();
            self.broadcast(ServerMessage::TopPlayerChanged {
                player_name: first,
                second_player_name: second,
            })
            .await;
        }
    }

    /// Broadcast a SkillLevelUp and check if it changes the top total level players.
    pub async fn broadcast_skill_level_up(&self, player_id: &str, skill: &str, new_level: i32) {
        self.broadcast(ServerMessage::SkillLevelUp {
            player_id: player_id.to_string(),
            skill: skill.to_string(),
            new_level,
        })
        .await;

        // Check if this level-up changes the rankings (skip admins — they're excluded from rankings)
        let players = self.players.read().await;
        if let Some(player) = players.get(player_id) {
            if !player.is_admin {
                let name = player.name.clone();
                let total = player.skills.total_level();
                drop(players);
                self.check_top_player_after_level_up(&name, total).await;
            }
        }
    }

    /// Get the current top players message for sending to newly connecting players.
    pub async fn get_top_player_message(&self) -> ServerMessage {
        let first = self.top_level_player_name.read().await.clone();
        let second = self.second_level_player_name.read().await.clone();
        ServerMessage::TopPlayerChanged {
            player_name: first,
            second_player_name: second,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        self.broadcast_tx.subscribe()
    }

    /// Register a spectator's message channel.
    pub async fn add_spectator(&self, spectator_id: &str, sender: mpsc::Sender<Vec<u8>>) {
        self.spectator_senders
            .write()
            .await
            .insert(spectator_id.to_string(), sender);
    }

    /// Remove a spectator's message channel.
    pub async fn remove_spectator(&self, spectator_id: &str) {
        self.spectator_senders.write().await.remove(spectator_id);
    }

    /// Get current spectator count.
    pub async fn spectator_count(&self) -> usize {
        self.spectator_senders.read().await.len()
    }

    pub async fn broadcast(&self, msg: ServerMessage) {
        if let Ok(bytes) = crate::protocol::encode_server_message(&msg) {
            let _ = self.broadcast_tx.send(bytes);
        }
    }

    /// Broadcast a message only to players in the same zone as the given player
    /// (same instance, or all overworld players if player is in overworld)
    pub async fn broadcast_to_zone(&self, source_player_id: &str, msg: ServerMessage) {
        let player_instances = self.player_instances.read().await;
        let source_instance = player_instances.get(source_player_id).cloned();
        let senders = self.player_senders.read().await;

        if let Ok(bytes) = crate::protocol::encode_server_message(&msg) {
            for (player_id, sender) in senders.iter() {
                let target_instance = player_instances.get(player_id).cloned();
                // Send if both in same instance or both in overworld (None)
                if source_instance == target_instance {
                    let _ = sender.try_send(bytes.clone());
                }
            }
        }
    }

    /// Send a message to all overworld players (those not in any instance), optionally excluding one player
    pub async fn send_to_overworld_players(&self, msg: ServerMessage, exclude: Option<&str>) {
        let player_instances = self.player_instances.read().await;
        let senders = self.player_senders.read().await;

        if let Ok(bytes) = crate::protocol::encode_server_message(&msg) {
            for (player_id, sender) in senders.iter() {
                if let Some(excluded) = exclude {
                    if player_id == excluded {
                        continue;
                    }
                }
                // Only send to players NOT in any instance
                if !player_instances.contains_key(player_id) {
                    let _ = sender.try_send(bytes.clone());
                }
            }
        }
    }

    /// Register a player's message sender for unicast
    pub async fn register_player_sender(&self, player_id: &str, sender: mpsc::Sender<Vec<u8>>) {
        let mut senders = self.player_senders.write().await;
        senders.insert(player_id.to_string(), sender);
        self.sync_states
            .write()
            .await
            .insert(player_id.to_string(), PlayerSyncState::new());
        tracing::debug!("Registered sender for player {}", player_id);
    }

    /// Unregister a player's message sender
    pub async fn unregister_player_sender(&self, player_id: &str) {
        let mut senders = self.player_senders.write().await;
        senders.remove(player_id);
        self.sync_states.write().await.remove(player_id);
        tracing::debug!("Unregistered sender for player {}", player_id);
    }

    /// Reset a player's delta sync state (forces full sync on next tick).
    /// Call on instance transitions so stale entity state doesn't carry over.
    pub async fn reset_sync_state(&self, player_id: &str) {
        if let Some(state) = self.sync_states.write().await.get_mut(player_id) {
            *state = PlayerSyncState::new();
        }
    }

    /// Find a portal at the player's current position
    pub async fn find_portal_at_player(&self, player_id: &str) -> Option<crate::chunk::Portal> {
        use crate::chunk::CHUNK_SIZE;
        use tracing::{debug, trace};

        let players = self.players.read().await;
        let player = players.get(player_id)?;
        let coord = ChunkCoord::from_world(player.x, player.y);

        debug!(
            "Looking for portal at player {} position ({}, {}), chunk ({}, {})",
            player_id, player.x, player.y, coord.x, coord.y
        );

        let chunk = self.world.get_or_load_chunk(coord).await?;

        debug!("Chunk has {} portals", chunk.portals.len());

        // Portal coordinates in chunk JSON are LOCAL (0-31), need to convert to WORLD coords
        let chunk_base_x = coord.x * CHUNK_SIZE as i32;
        let chunk_base_y = coord.y * CHUNK_SIZE as i32;

        for p in &chunk.portals {
            let world_x = chunk_base_x + p.x;
            let world_y = chunk_base_y + p.y;
            trace!(
                "Portal '{}' at local ({}, {}) -> world ({}, {}) to ({}, {}), target: {}",
                p.id,
                p.x,
                p.y,
                world_x,
                world_y,
                world_x + p.width,
                world_y + p.height,
                p.target_map
            );
        }

        chunk
            .portals
            .iter()
            .find(|p| {
                let world_x = chunk_base_x + p.x;
                let world_y = chunk_base_y + p.y;
                let in_portal = player.x >= world_x
                    && player.x < world_x + p.width
                    && player.y >= world_y
                    && player.y < world_y + p.height;
                if in_portal {
                    debug!("Player {} is inside portal '{}'", player_id, p.id);
                }
                in_portal
            })
            .cloned()
    }

    /// Send a message to a specific player (unicast)
    /// SECURITY: Use this for private data like inventory updates
    pub async fn send_to_player(&self, player_id: &str, msg: ServerMessage) {
        use crate::protocol::encode_server_message;

        let senders = self.player_senders.read().await;
        if let Some(sender) = senders.get(player_id) {
            if let Ok(bytes) = encode_server_message(&msg) {
                if let Err(e) = sender.try_send(bytes) {
                    match e {
                        tokio::sync::mpsc::error::TrySendError::Full(_) => {
                            tracing::debug!(
                                "Unicast queue full for {}; dropping message",
                                player_id
                            );
                        }
                        tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                            tracing::warn!(
                                "Failed to send unicast to {}: channel closed",
                                player_id
                            );
                        }
                    }
                }
            }
        } else {
            tracing::debug!("No sender registered for player {}", player_id);
        }
    }

    pub async fn reserve_player(
        &self,
        player_id: &str,
        name: &str,
        gender: &str,
        skin: &str,
        hair_style: Option<i32>,
        hair_color: Option<i32>,
    ) {
        let (spawn_x, spawn_y) = self.world.get_spawn_position().await;
        let mut players = self.players.write().await;
        let player = Player::new(
            player_id, name, spawn_x, spawn_y, gender, skin, hair_style, hair_color,
        );
        players.insert(player_id.to_string(), player);

        // Track player's starting chunk
        let chunk = ChunkCoord::from_world(spawn_x, spawn_y);
        let mut chunks = self.player_chunks.write().await;
        chunks.insert(player_id.to_string(), chunk);
    }

    /// Reserve player with saved data from database
    pub async fn reserve_player_with_data(
        &self,
        player_id: &str,
        name: &str,
        x: i32,
        y: i32,
        z: i32,
        hp: i32,
        prayer_points: i32,
        mp: i32,
        skills: Skills,
        gold: i32,
        inventory_json: &str,
        gender: &str,
        skin: &str,
        hair_style: Option<i32>,
        hair_color: Option<i32>,
        equipped_head: Option<String>,
        equipped_body: Option<String>,
        equipped_weapon: Option<String>,
        equipped_back: Option<String>,
        equipped_feet: Option<String>,
        equipped_ring: Option<String>,
        equipped_gloves: Option<String>,
        equipped_necklace: Option<String>,
        equipped_belt: Option<String>,
        is_admin: bool,
        sitting_at_x: Option<i32>,
        sitting_at_y: Option<i32>,
        bank_json: &str,
        bank_gold: i32,
        bank_max_slots: u32,
        combat_style_prefs_json: &str,
    ) {
        // Validate saved position — if the chunk doesn't exist on disk, reset to spawn
        let (safe_x, safe_y, safe_z) = {
            let coord = ChunkCoord::from_world(x, y);
            if self.world.chunk_file_exists(coord) {
                (x, y, z)
            } else {
                tracing::warn!(
                    "Player {} has invalid position ({}, {}) — chunk {:?} missing on disk, resetting to spawn",
                    player_id, x, y, coord
                );
                (WORLD_SPAWN_X, WORLD_SPAWN_Y, 0)
            }
        };

        let mut player = Player::new(player_id, name, safe_x, safe_y, gender, skin, hair_style, hair_color);
        player.z = safe_z;
        player.bank_max_slots = bank_max_slots;
        player.bank = item::Bank::new_with_size(bank_max_slots as usize);

        // Restore saved stats
        player.skills = skills;
        player.hp = hp.min(player.max_hp()); // Cap HP at max (hitpoints level)
        player.prayer_points = prayer_points.min(player.max_prayer_points());
        player.mp = mp.min(player.max_mp());
        // If player disconnected while dead (hp=0), respawn them
        if player.hp <= 0 {
            player.hp = player.max_hp();
            player.x = WORLD_SPAWN_X;
            player.y = WORLD_SPAWN_Y;
            player.z = 0;
        }
        player.inventory.gold = gold;
        player.equipped_head = equipped_head;
        player.equipped_body = equipped_body;
        player.equipped_weapon = equipped_weapon;
        player.equipped_back = equipped_back;
        player.equipped_feet = equipped_feet;
        player.equipped_ring = equipped_ring;
        player.equipped_gloves = equipped_gloves;
        player.equipped_necklace = equipped_necklace;
        player.equipped_belt = equipped_belt;
        player.is_admin = is_admin;

        // Restore combat style preferences and set active style based on equipped weapon
        if let Ok(prefs) = serde_json::from_str::<HashMap<String, String>>(combat_style_prefs_json) {
            for (weapon_key, style_str) in &prefs {
                if let Some(style) = CombatStyle::from_str(style_str) {
                    player.combat_style_prefs.insert(weapon_key.clone(), style);
                }
            }
        }
        // Determine weapon type from equipped weapon and restore preferred style
        let weapon_type = player.equipped_weapon.as_ref()
            .and_then(|wid| self.item_registry.get(wid))
            .and_then(|def| def.equipment.as_ref())
            .map(|eq| eq.weapon_type)
            .unwrap_or(WeaponType::Melee);
        let weapon_key = match weapon_type {
            WeaponType::Melee => "melee",
            WeaponType::Ranged => "ranged",
        };
        if let Some(&pref_style) = player.combat_style_prefs.get(weapon_key) {
            if pref_style.is_valid_for(weapon_type) {
                player.combat_style = pref_style;
            }
        }

        // Restore inventory from JSON - support both old (u8) and new (String) formats
        // Skip invalid slots (empty item_id or quantity <= 0) to prevent ghost items
        if let Ok(slots) = serde_json::from_str::<Vec<(usize, String, i32)>>(inventory_json) {
            // New format: (slot_idx, item_id, quantity)
            for (slot_idx, item_id, quantity) in slots {
                if slot_idx < player.inventory.slots.len() && !item_id.is_empty() && quantity > 0 {
                    player.inventory.slots[slot_idx] =
                        Some(item::InventorySlot::new(item_id, quantity));
                }
            }
        } else if let Ok(slots) = serde_json::from_str::<Vec<(usize, u8, i32)>>(inventory_json) {
            // Legacy format: (slot_idx, item_type_u8, quantity) - migrate to string IDs
            for (slot_idx, item_type_u8, quantity) in slots {
                if slot_idx < player.inventory.slots.len() && quantity > 0 {
                    let item_id = match item_type_u8 {
                        0 => "health_potion",
                        1 => "mana_potion",
                        3 => "slime_core",
                        4 => "iron_ore",
                        5 => "goblin_ear",
                        _ => continue, // Skip unknown items (2 was gold, handled separately)
                    }
                    .to_string();
                    player.inventory.slots[slot_idx] =
                        Some(item::InventorySlot::new(item_id, quantity));
                }
            }
        }

        // Restore bank from JSON
        player.bank.gold = bank_gold;
        if let Ok(slots) = serde_json::from_str::<Vec<(usize, String, i32)>>(bank_json) {
            for (slot_idx, item_id, quantity) in slots {
                if slot_idx < player.bank.slots.len() && !item_id.is_empty() && quantity > 0 {
                    player.bank.slots[slot_idx] = Some(item::InventorySlot::new(item_id, quantity));
                }
            }
        }

        // Restore sitting state and set direction from chair before inserting into players map
        if let (Some(sx), Some(sy)) = (sitting_at_x, sitting_at_y) {
            let mut chairs = self.chairs.write().await;
            if let Some(chair) = chairs.get_mut(&(sx, sy)) {
                player.sitting_at = Some((sx, sy));
                player.direction = chair.direction;
                chair.occupied_by = Some(player_id.to_string());
            }
            // If chair no longer exists, don't restore sitting state
        }

        tracing::info!(
            "Restored player {} at ({}, {}) with {} HP, combat level {}, {} gold, appearance: {} {}",
            name,
            x,
            y,
            hp,
            player.combat_level(),
            gold,
            gender,
            skin
        );

        let mut players = self.players.write().await;
        players.insert(player_id.to_string(), player);
        drop(players);

        // Track player's starting chunk for systems that reference chunk residency.
        let chunk = ChunkCoord::from_world(x, y);
        let mut chunks = self.player_chunks.write().await;
        chunks.insert(player_id.to_string(), chunk);
    }

    pub async fn activate_player(&self, player_id: &str) -> String {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.active = true;
            return player.name.clone();
        }
        "Unknown".to_string()
    }

    pub async fn remove_player(&self, player_id: &str) {
        // Handle arena disconnect
        {
            let mut arena = self.arena_manager.write().await;
            if let Some((disconnected_id, _killer_id)) = arena.on_player_disconnect(player_id) {
                // If was fighting and match should end, handle it
                if arena.is_fighting() && arena.check_match_end() {
                    let current_time = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    let placements = arena.end_match(current_time);
                    drop(arena);

                    // Distribute rewards
                    {
                        let mut players = self.players.write().await;
                        for placement in &placements {
                            if placement.gold_reward > 0 {
                                if let Some(p) = players.get_mut(&placement.player_id) {
                                    p.inventory.gold += placement.gold_reward;
                                }
                            }
                        }
                    }

                    let placement_data: Vec<crate::protocol::ArenaPlacementData> = placements
                        .iter()
                        .map(|p| crate::protocol::ArenaPlacementData {
                            rank: p.rank,
                            player_id: p.player_id.clone(),
                            player_name: p.player_name.clone(),
                            kills: p.kills,
                            gold_reward: p.gold_reward,
                        })
                        .collect();

                    self.broadcast_to_arena(ServerMessage::ArenaMatchEnd {
                        placements: placement_data,
                    })
                    .await;

                    // Broadcast elimination for the disconnected player
                    self.broadcast_to_arena(ServerMessage::ArenaPlayerEliminated {
                        player_id: disconnected_id.clone(),
                        player_name: "Disconnected".to_string(),
                        killer_id: "disconnect".to_string(),
                        killer_name: "Disconnect".to_string(),
                        remaining: 0,
                    })
                    .await;
                } else {
                    // Refund if was queued (escrow removed in on_player_disconnect)
                    let _ = &disconnected_id; // already handled
                }
            }
        }

        // Free any chair the player was sitting on
        // Extract sitting position first, then release players lock before acquiring chairs lock
        let sitting_pos = {
            let players = self.players.read().await;
            players.get(player_id).and_then(|p| p.sitting_at)
        };
        if let Some((tx, ty)) = sitting_pos {
            let mut chairs = self.chairs.write().await;
            if let Some(chair) = chairs.get_mut(&(tx, ty)) {
                if chair.occupied_by.as_deref() == Some(player_id) {
                    chair.occupied_by = None;
                }
            }
        }

        // Close any open chest
        self.close_player_chest(player_id).await;

        // Cancel any active trade on disconnect
        self.cancel_trade_for_player(player_id, "Partner disconnected")
            .await;

        // Close stall on disconnect, return items to inventory/bank
        self.force_close_stall(player_id).await;

        // Stop any active gathering/woodcutting
        {
            let mut gathering = self.gathering.write().await;
            gathering.stop_gathering(player_id);
        }
        {
            let mut woodcutting = self.woodcutting.write().await;
            woodcutting.stop_woodcutting(player_id);
        }

        // Clean up player chunk tracking
        {
            let mut chunks = self.player_chunks.write().await;
            chunks.remove(player_id);
        }

        // Clean up player quest states
        {
            let mut quest_states = self.player_quest_states.write().await;
            quest_states.remove(player_id);
        }

        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.auto_action = None;
        }
        players.remove(player_id);
    }

    /// Get player data for saving to database
    pub async fn get_player_save_data(&self, player_id: &str) -> Option<PlayerSaveData> {
        // Check if player is in an instance and get the map_id
        let current_map = if self.player_instances.read().await.contains_key(player_id) {
            self.instance_manager
                .find_player_instance(player_id)
                .await
                .map(|inst| inst.map_id.clone())
        } else {
            None
        };

        let players = self.players.read().await;
        players.get(player_id).map(|p| {
            // Serialize inventory to JSON - new format with string item IDs
            // Filter out empty/invalid slots to prevent ghost items
            let inventory_slots: Vec<(usize, String, i32)> = p
                .inventory
                .slots
                .iter()
                .enumerate()
                .filter_map(|(idx, slot)| {
                    slot.as_ref()
                        .filter(|s| s.quantity > 0 && !s.item_id.is_empty())
                        .map(|s| (idx, s.item_id.clone(), s.quantity))
                })
                .collect();
            let inventory_json =
                serde_json::to_string(&inventory_slots).unwrap_or_else(|_| "[]".to_string());

            // Serialize bank to JSON
            let bank_slots: Vec<(usize, String, i32)> = p
                .bank
                .slots
                .iter()
                .enumerate()
                .filter_map(|(idx, slot)| {
                    slot.as_ref()
                        .filter(|s| s.quantity > 0 && !s.item_id.is_empty())
                        .map(|s| (idx, s.item_id.clone(), s.quantity))
                })
                .collect();
            let bank_json = serde_json::to_string(&bank_slots).unwrap_or_else(|_| "[]".to_string());

            PlayerSaveData {
                x: p.x as f32,
                y: p.y as f32,
                z: p.z,
                hp: p.hp,
                prayer_points: p.prayer_points,
                mp: p.mp,
                skills: p.skills.clone(),
                gold: p.inventory.gold,
                inventory_json,
                gender: p.gender.clone(),
                skin: p.skin.clone(),
                equipped_head: p.equipped_head.clone(),
                equipped_body: p.equipped_body.clone(),
                equipped_weapon: p.equipped_weapon.clone(),
                equipped_back: p.equipped_back.clone(),
                equipped_feet: p.equipped_feet.clone(),
                equipped_ring: p.equipped_ring.clone(),
                equipped_gloves: p.equipped_gloves.clone(),
                equipped_necklace: p.equipped_necklace.clone(),
                equipped_belt: p.equipped_belt.clone(),
                current_map: current_map.clone(),
                sitting_at_x: p.sitting_at.map(|(x, _)| x),
                sitting_at_y: p.sitting_at.map(|(_, y)| y),
                entrance_x: None, // Filled in by caller from player_entrance_positions
                entrance_y: None,
                bank_json,
                bank_gold: p.bank.gold,
                bank_max_slots: p.bank_max_slots,
                combat_style_prefs: {
                    let prefs_map: HashMap<&str, &str> = p.combat_style_prefs.iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();
                    serde_json::to_string(&prefs_map).unwrap_or_else(|_| "{}".to_string())
                },
            }
        })
    }

    /// Batch-snapshot save data for multiple players in a single lock acquisition.
    /// Returns a map of player_id -> (PlayerSaveData, Option<PlayerQuestState>, HashSet<String>)
    pub async fn get_bulk_save_data(
        &self,
        player_ids: &[String],
    ) -> HashMap<
        String,
        (
            PlayerSaveData,
            Option<PlayerQuestState>,
            HashSet<String>,
            Option<crate::slayer::PlayerSlayerState>,
            HashSet<String>,
        ),
    > {
        struct RawPlayerSnapshot {
            x: i32,
            y: i32,
            z: i32,
            hp: i32,
            prayer_points: i32,
            mp: i32,
            skills: Skills,
            gold: i32,
            inventory_slots: Vec<(usize, String, i32)>,
            gender: String,
            skin: String,
            equipped_head: Option<String>,
            equipped_body: Option<String>,
            equipped_weapon: Option<String>,
            equipped_back: Option<String>,
            equipped_feet: Option<String>,
            equipped_ring: Option<String>,
            equipped_gloves: Option<String>,
            equipped_necklace: Option<String>,
            equipped_belt: Option<String>,
            sitting_at_x: Option<i32>,
            sitting_at_y: Option<i32>,
            recipes: HashSet<String>,
            unlocked_spells: HashSet<String>,
            bank_slots: Vec<(usize, String, i32)>,
            bank_gold: i32,
            bank_max_slots: u32,
            combat_style_prefs: HashMap<String, CombatStyle>,
        }

        let mut result = HashMap::new();

        // Snapshot instance assignments once
        let instance_map: HashMap<String, String> = {
            let instances = self.player_instances.read().await;
            player_ids
                .iter()
                .filter_map(|pid| instances.get(pid).map(|inst| (pid.clone(), inst.clone())))
                .collect()
        };

        // Resolve map_ids for players in instances (batch)
        let mut map_ids: HashMap<String, String> = HashMap::new();
        for (pid, _inst_id) in &instance_map {
            if let Some(inst) = self.instance_manager.find_player_instance(pid).await {
                map_ids.insert(pid.clone(), inst.map_id.clone());
            }
        }

        // Single lock on players to snapshot all mutable gameplay state.
        // Keep this lock scope minimal; expensive JSON serialization happens after unlock.
        let raw_snapshots: HashMap<String, RawPlayerSnapshot> = {
            let players = self.players.read().await;
            let mut snapshots = HashMap::new();
            for pid in player_ids {
                if let Some(p) = players.get(pid) {
                    let inventory_slots: Vec<(usize, String, i32)> = p
                        .inventory
                        .slots
                        .iter()
                        .enumerate()
                        .filter_map(|(idx, slot)| {
                            slot.as_ref()
                                .filter(|s| s.quantity > 0 && !s.item_id.is_empty())
                                .map(|s| (idx, s.item_id.clone(), s.quantity))
                        })
                        .collect();

                    snapshots.insert(
                        pid.clone(),
                        RawPlayerSnapshot {
                            x: p.x,
                            y: p.y,
                            z: p.z,
                            hp: p.hp,
                            prayer_points: p.prayer_points,
                            mp: p.mp,
                            skills: p.skills.clone(),
                            gold: p.inventory.gold,
                            inventory_slots,
                            gender: p.gender.clone(),
                            skin: p.skin.clone(),
                            equipped_head: p.equipped_head.clone(),
                            equipped_body: p.equipped_body.clone(),
                            equipped_weapon: p.equipped_weapon.clone(),
                            equipped_back: p.equipped_back.clone(),
                            equipped_feet: p.equipped_feet.clone(),
                            equipped_ring: p.equipped_ring.clone(),
                            equipped_gloves: p.equipped_gloves.clone(),
                            equipped_necklace: p.equipped_necklace.clone(),
                            equipped_belt: p.equipped_belt.clone(),
                            sitting_at_x: p.sitting_at.map(|(x, _)| x),
                            sitting_at_y: p.sitting_at.map(|(_, y)| y),
                            recipes: p.discovered_recipes.clone(),
                            unlocked_spells: p.unlocked_spells.clone(),
                            bank_slots: p
                                .bank
                                .slots
                                .iter()
                                .enumerate()
                                .filter_map(|(idx, slot)| {
                                    slot.as_ref()
                                        .filter(|s| s.quantity > 0 && !s.item_id.is_empty())
                                        .map(|s| (idx, s.item_id.clone(), s.quantity))
                                })
                                .collect(),
                            bank_gold: p.bank.gold,
                            bank_max_slots: p.bank_max_slots,
                            combat_style_prefs: p.combat_style_prefs.clone(),
                        },
                    );
                }
            }
            snapshots
        };

        // Build save payloads outside the players lock.
        for (pid, raw) in raw_snapshots {
            let inventory_json =
                serde_json::to_string(&raw.inventory_slots).unwrap_or_else(|_| "[]".to_string());
            let save_data = PlayerSaveData {
                x: raw.x as f32,
                y: raw.y as f32,
                z: raw.z,
                hp: raw.hp,
                prayer_points: raw.prayer_points,
                mp: raw.mp,
                skills: raw.skills,
                gold: raw.gold,
                inventory_json,
                gender: raw.gender,
                skin: raw.skin,
                equipped_head: raw.equipped_head,
                equipped_body: raw.equipped_body,
                equipped_weapon: raw.equipped_weapon,
                equipped_back: raw.equipped_back,
                equipped_feet: raw.equipped_feet,
                equipped_ring: raw.equipped_ring,
                equipped_gloves: raw.equipped_gloves,
                equipped_necklace: raw.equipped_necklace,
                equipped_belt: raw.equipped_belt,
                current_map: map_ids.get(&pid).cloned(),
                sitting_at_x: raw.sitting_at_x,
                sitting_at_y: raw.sitting_at_y,
                entrance_x: None, // Filled in by caller from player_entrance_positions
                entrance_y: None,
                bank_json: serde_json::to_string(&raw.bank_slots)
                    .unwrap_or_else(|_| "[]".to_string()),
                bank_gold: raw.bank_gold,
                bank_max_slots: raw.bank_max_slots,
                combat_style_prefs: {
                    let prefs_map: HashMap<&str, &str> = raw.combat_style_prefs.iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();
                    serde_json::to_string(&prefs_map).unwrap_or_else(|_| "{}".to_string())
                },
            };
            result.insert(
                pid,
                (save_data, None, raw.recipes, None, raw.unlocked_spells),
            );
        }

        // Single lock on quest states
        {
            let quest_states = self.player_quest_states.read().await;
            for pid in player_ids {
                if let Some(entry) = result.get_mut(pid) {
                    entry.1 = quest_states.get(pid).cloned();
                }
            }
        }

        // Single lock on slayer states
        {
            let slayer_states = self.player_slayer_states.read().await;
            for pid in player_ids {
                if let Some(entry) = result.get_mut(pid) {
                    entry.3 = slayer_states.get(pid).cloned();
                }
            }
        }

        result
    }

    /// Initialize quest state for a player (called on join)
    pub async fn set_player_quest_state(&self, player_id: &str, state: PlayerQuestState) {
        let mut quest_states = self.player_quest_states.write().await;
        quest_states.insert(player_id.to_string(), state);
    }

    /// Get quest state for saving (called on disconnect/auto-save)
    pub async fn get_player_quest_state(&self, player_id: &str) -> Option<PlayerQuestState> {
        let quest_states = self.player_quest_states.read().await;
        quest_states.get(player_id).cloned()
    }

    /// Set slayer state for a player (called on join / after changes)
    pub async fn set_player_slayer_state(
        &self,
        player_id: &str,
        state: crate::slayer::PlayerSlayerState,
    ) {
        self.player_slayer_states
            .write()
            .await
            .insert(player_id.to_string(), state);
    }

    /// Get slayer state for a player (returns default if none stored)
    pub async fn get_player_slayer_state(
        &self,
        player_id: &str,
    ) -> crate::slayer::PlayerSlayerState {
        let mut state = self
            .player_slayer_states
            .read()
            .await
            .get(player_id)
            .cloned()
            .unwrap_or_default();
        // Migration: fix old "living_rock" task IDs -> "rock"
        if let Some(ref mut task) = state.current_task {
            if task.monster_id == "living_rock" {
                task.monster_id = "rock".to_string();
                // Persist the fix
                self.player_slayer_states
                    .write()
                    .await
                    .insert(player_id.to_string(), state.clone());
            }
        }
        state
    }

    /// Set discovered recipes for a player (called on connect after loading from DB)
    pub async fn set_player_discovered_recipes(&self, player_id: &str, recipes: HashSet<String>) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.discovered_recipes = recipes;
        }
    }

    /// Get discovered recipes for a player (for saving to DB)
    pub async fn get_player_discovered_recipes(&self, player_id: &str) -> HashSet<String> {
        let players = self.players.read().await;
        players
            .get(player_id)
            .map(|p| p.discovered_recipes.clone())
            .unwrap_or_default()
    }

    /// Discover a new recipe for a player, returns true if newly discovered
    pub async fn discover_recipe(&self, player_id: &str, recipe_id: &str) -> bool {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.discovered_recipes.insert(recipe_id.to_string())
        } else {
            false
        }
    }

    /// Set unlocked spells for a player (called on connect after loading from DB)
    pub async fn set_player_unlocked_spells(&self, player_id: &str, spells: HashSet<String>) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.unlocked_spells = spells;
        }
    }

    /// Get unlocked spells for a player (for saving to DB)
    pub async fn get_player_unlocked_spells(&self, player_id: &str) -> HashSet<String> {
        let players = self.players.read().await;
        players
            .get(player_id)
            .map(|p| p.unlocked_spells.clone())
            .unwrap_or_default()
    }

    /// Build ScrollSpellDefinitions message for sending to clients on connect
    pub fn get_scroll_spell_definitions_message(&self) -> ServerMessage {
        let spells: Vec<crate::protocol::ScrollSpellDefData> = self
            .scroll_spell_registry
            .all()
            .iter()
            .map(|(id, def)| crate::protocol::ScrollSpellDefData {
                id: id.clone(),
                name: def.name.clone(),
                spell_type: match def.spell_type {
                    crate::spell::SpellType::Damage => "damage".to_string(),
                    crate::spell::SpellType::Heal => "heal".to_string(),
                    crate::spell::SpellType::Teleport => "teleport".to_string(),
                },
                mana_cost: def.mana_cost,
                cooldown_ms: def.cooldown_ms,
                base_power: def.base_power,
                effect_sprite: def.effect_sprite.clone(),
                pushback_distance: def.pushback_distance,
                wall_slam_damage_per_tile: def.wall_slam_damage_per_tile,
                description: def.description.clone(),
            })
            .collect();
        ServerMessage::ScrollSpellDefinitions { spells }
    }

    /// Get QuestAccepted messages for all active quests (for syncing on login)
    pub async fn get_active_quest_messages(&self, player_id: &str) -> Vec<ServerMessage> {
        let quest_states = self.player_quest_states.read().await;
        let quest_state = match quest_states.get(player_id) {
            Some(state) => state,
            None => return Vec::new(),
        };

        let mut messages = Vec::new();
        for (quest_id, progress) in &quest_state.active_quests {
            if let Some(quest) = self.quest_registry.get(quest_id).await {
                let objectives: Vec<QuestObjectiveData> = quest
                    .objectives
                    .iter()
                    .map(|o| {
                        // Get current progress from saved state
                        let (current, completed) = progress
                            .objectives
                            .get(&o.id)
                            .map(|p| (p.current, p.completed))
                            .unwrap_or((0, false));
                        QuestObjectiveData {
                            id: o.id.clone(),
                            description: o.description.clone(),
                            current,
                            target: o.count,
                            completed,
                        }
                    })
                    .collect();
                messages.push(ServerMessage::QuestAccepted {
                    quest_id: quest_id.clone(),
                    quest_name: quest.name.clone(),
                    objectives,
                });
            }
        }
        messages
    }

    /// Get a sync message containing completed quest ids for this player (for login UI state)
    pub async fn get_completed_quest_sync_message(&self, player_id: &str) -> ServerMessage {
        let quest_states = self.player_quest_states.read().await;
        let completed_quest_ids = quest_states
            .get(player_id)
            .map(|state| state.completed_quests.clone())
            .unwrap_or_default();

        ServerMessage::QuestStateSync {
            completed_quest_ids,
        }
    }

    /// Build the full quest catalog for sending to client on login
    pub async fn build_quest_catalog(&self) -> ServerMessage {
        let all_quests = self.quest_registry.all_quests().await;
        let npcs = self.npcs.read().await;

        // Build a map of prototype_id -> display_name from loaded NPCs
        let npc_names: std::collections::HashMap<String, String> = npcs
            .values()
            .map(|npc| (npc.prototype_id.clone(), npc.stats.display_name.clone()))
            .collect();

        let mut entries: Vec<QuestCatalogEntryData> = Vec::new();
        for quest in &all_quests {
            let giver_npc_name = npc_names
                .get(&quest.giver_npc)
                .cloned()
                .unwrap_or_else(|| quest.giver_npc.clone());

            // Resolve prerequisite quest name
            let (required_quest_id, required_quest_name) =
                if let Some(ref prev_id) = quest.chain.previous {
                    let prev_name = all_quests
                        .iter()
                        .find(|q| q.id == *prev_id)
                        .map(|q| q.name.clone());
                    (Some(prev_id.clone()), prev_name)
                } else {
                    (None, None)
                };

            let objectives = quest
                .objectives
                .iter()
                .map(|o| QuestObjectiveData {
                    id: o.id.clone(),
                    description: o.description.clone(),
                    current: 0,
                    target: o.count,
                    completed: false,
                })
                .collect();
            entries.push(QuestCatalogEntryData {
                quest_id: quest.id.clone(),
                name: quest.name.clone(),
                description: quest.description.clone(),
                giver_npc_name,
                level_required: quest.level_required,
                required_quest_id,
                required_quest_name,
                objectives,
            });
        }

        ServerMessage::QuestCatalog { quests: entries }
    }

    pub async fn player_count(&self) -> usize {
        let players = self.players.read().await;
        players.values().filter(|p| p.active).count()
    }

    pub async fn get_all_players(&self) -> Vec<Player> {
        let players = self.players.read().await;
        players.values().filter(|p| p.active).cloned().collect()
    }

    /// Get sitting info for a player: returns Some((tile_x, tile_y, direction)) if sitting
    pub async fn get_player_sitting_info(&self, player_id: &str) -> Option<(i32, i32, u8)> {
        let sitting_at = {
            let players = self.players.read().await;
            players.get(player_id)?.sitting_at?
        };
        let (sx, sy) = sitting_at;
        let chairs = self.chairs.read().await;
        let chair = chairs.get(&(sx, sy))?;
        Some((sx, sy, chair.direction as u8))
    }

    pub async fn get_player_position(&self, player_id: &str) -> Option<(i32, i32)> {
        let players = self.players.read().await;
        players.get(player_id).map(|p| (p.x, p.y))
    }

    pub async fn set_player_position(&self, player_id: &str, x: i32, y: i32) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.x = x;
            player.y = y;
        }
    }

    pub async fn set_player_position_and_z(&self, player_id: &str, x: i32, y: i32, z: i32) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.x = x;
            player.y = y;
            player.z = z;
            player.grounded = true;
        }
    }

    pub async fn set_combat_style(&self, player_id: &str, style: CombatStyle) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            // Get current weapon type to validate style
            let weapon_type = player.equipped_weapon.as_ref()
                .and_then(|wid| self.item_registry.get(wid))
                .and_then(|def| def.equipment.as_ref())
                .map(|eq| eq.weapon_type)
                .unwrap_or(WeaponType::Melee);

            // Only set if style is valid for current weapon type
            if style.is_valid_for(weapon_type) {
                player.combat_style = style;
                // Save preference for this weapon type
                let weapon_key = match weapon_type {
                    WeaponType::Melee => "melee",
                    WeaponType::Ranged => "ranged",
                };
                player.combat_style_prefs.insert(weapon_key.to_string(), style);
            }
        }
    }

    pub async fn get_player_appearance(&self, player_id: &str) -> Option<(String, String)> {
        let players = self.players.read().await;
        players
            .get(player_id)
            .map(|p| (p.gender.clone(), p.skin.clone()))
    }

    pub async fn get_player_hair(&self, player_id: &str) -> Option<(Option<i32>, Option<i32>)> {
        let players = self.players.read().await;
        players.get(player_id).map(|p| (p.hair_style, p.hair_color))
    }

    pub async fn get_player_name(&self, player_id: &str) -> Option<String> {
        let players = self.players.read().await;
        players.get(player_id).map(|p| p.name.clone())
    }

    /// Get all ground items in a specific instance (or overworld if None)
    pub async fn get_ground_items_in_instance(
        &self,
        instance_id: Option<&str>,
    ) -> Vec<ServerMessage> {
        let items = self.ground_items.read().await;
        items
            .values()
            .filter(|item| {
                match (&item.instance_id, instance_id) {
                    (None, None) => true,         // Both overworld
                    (Some(a), Some(b)) => a == b, // Same instance
                    _ => false,                   // Different zones
                }
            })
            .map(|item| ServerMessage::ItemDropped {
                id: item.id.clone(),
                item_id: item.item_id.clone(),
                x: item.x,
                y: item.y,
                quantity: item.quantity,
            })
            .collect()
    }

    /// Get the initial inventory update message for a player (used on connection)
    pub async fn get_player_inventory_update(&self, player_id: &str) -> Option<ServerMessage> {
        let players = self.players.read().await;
        players
            .get(player_id)
            .map(|p| ServerMessage::InventoryUpdate {
                player_id: player_id.to_string(),
                slots: p.inventory.to_update(),
                gold: p.inventory.gold,
            })
    }

    /// Get the initial skills sync message for a player (used on connection)
    pub async fn get_player_skills_sync(&self, player_id: &str) -> Option<ServerMessage> {
        let players = self.players.read().await;
        players.get(player_id).map(|p| ServerMessage::SkillsSync {
            player_id: player_id.to_string(),
            hitpoints_level: p.skills.hitpoints.level,
            hitpoints_xp: p.skills.hitpoints.xp,
            attack_level: p.skills.attack.level,
            attack_xp: p.skills.attack.xp,
            strength_level: p.skills.strength.level,
            strength_xp: p.skills.strength.xp,
            defence_level: p.skills.defence.level,
            defence_xp: p.skills.defence.xp,
            ranged_level: p.skills.ranged.level,
            ranged_xp: p.skills.ranged.xp,
            fishing_level: p.skills.fishing.level,
            fishing_xp: p.skills.fishing.xp,
            farming_level: p.skills.farming.level,
            farming_xp: p.skills.farming.xp,
            smithing_level: p.skills.smithing.level,
            smithing_xp: p.skills.smithing.xp,
            prayer_level: p.skills.prayer.level,
            prayer_xp: p.skills.prayer.xp,
            magic_level: p.skills.magic.level,
            magic_xp: p.skills.magic.xp,
            woodcutting_level: p.skills.woodcutting.level,
            woodcutting_xp: p.skills.woodcutting.xp,
            alchemy_level: p.skills.alchemy.level,
            alchemy_xp: p.skills.alchemy.xp,
            mining_level: p.skills.mining.level,
            mining_xp: p.skills.mining.xp,
            slayer_level: p.skills.slayer.level,
            slayer_xp: p.skills.slayer.xp,
            survivalist_level: p.skills.survivalist.level,
            survivalist_xp: p.skills.survivalist.xp,
        })
    }

    /// Build a PotionBuffsSync message for a player's current active buffs
    fn build_potion_buffs_sync(player_id: &str, player: &Player) -> ServerMessage {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        ServerMessage::PotionBuffsSync {
            player_id: player_id.to_string(),
            buffs: player
                .active_buffs
                .iter()
                .map(|b| crate::protocol::PotionBuffEntry {
                    stat: b.stat.clone(),
                    amount: b.amount,
                    remaining_ms: b.expires_at.saturating_sub(now),
                    source_item_id: b.source_item_id.clone(),
                })
                .collect(),
        }
    }

    /// Get potion buffs sync for initial login
    pub async fn get_player_potion_buffs_sync(&self, player_id: &str) -> Option<ServerMessage> {
        let players = self.players.read().await;
        players
            .get(player_id)
            .filter(|p| !p.active_buffs.is_empty())
            .map(|p| Self::build_potion_buffs_sync(player_id, p))
    }

    pub async fn get_all_npcs(&self) -> Vec<Npc> {
        let npcs = self.npcs.read().await;
        npcs.values().cloned().collect()
    }

    /// Spawn an NPC at a specific location (admin command)
    pub async fn spawn_npc_at(&self, prototype_id: &str, x: f32, y: f32) -> Option<String> {
        let Some(prototype) = self.entity_registry.get(prototype_id) else {
            tracing::warn!("Cannot spawn NPC: prototype '{}' not found", prototype_id);
            return None;
        };

        let npc_id = format!("admin_npc_{}", Uuid::new_v4());
        let npc = Npc::from_prototype(
            &npc_id,
            prototype_id,
            prototype,
            x as i32,
            y as i32,
            1, // Default level
            None,
        );

        let mut npcs = self.npcs.write().await;
        npcs.insert(npc_id.clone(), npc);
        tracing::info!("Admin spawned NPC {} at ({}, {})", prototype_id, x, y);
        Some(npc_id)
    }

    pub async fn handle_move(&self, player_id: &str, dx: f32, dy: f32, seq: Option<u32>) {
        // NOTE: Movement does NOT cancel auto-action. The client sends an explicit
        // CancelAutoAction message when the player manually moves (keyboard/dpad),
        // manually attacks, or clicks empty ground. Chase-follow movements must NOT
        // interrupt auto-action, otherwise the player can never catch a moving target.

        let mut chair_to_free: Option<(i32, i32)> = None;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                let move_seq =
                    seq.unwrap_or_else(|| player.last_received_move_seq.saturating_add(1));
                let prev_seq = player.last_received_move_seq;
                if move_seq <= player.last_received_move_seq {
                    self.movement_anomalies
                        .stale_packets_ignored
                        .fetch_add(1, Ordering::Relaxed);
                    // Out-of-order/duplicate move packet. This is especially useful when
                    // diagnosing "continued movement after key-up" reports.
                    if now_ms.saturating_sub(player.last_move_input_warn_ms)
                        >= MOVE_INPUT_WARN_THROTTLE_MS
                    {
                        let is_stop = dx.abs() <= 0.1 && dy.abs() <= 0.1;
                        tracing::warn!(
                            "Ignoring stale move packet for {} (seq={} <= last={}, stop={}, intent=({}, {}) pos=({}, {}))",
                            player_id,
                            move_seq,
                            player.last_received_move_seq,
                            is_stop,
                            player.move_dx,
                            player.move_dy,
                            player.x,
                            player.y
                        );
                        player.last_move_input_warn_ms = now_ms;
                    }
                    return;
                }
                player.last_received_move_seq = move_seq;

                // Detect missing move packets from client/network path.
                let seq_gap = move_seq.saturating_sub(prev_seq);
                if seq_gap > 4 {
                    self.movement_anomalies
                        .seq_gap_events
                        .fetch_add(1, Ordering::Relaxed);
                    if now_ms.saturating_sub(player.last_move_input_warn_ms)
                        >= MOVE_INPUT_WARN_THROTTLE_MS
                    {
                        tracing::warn!(
                            "Move seq gap {} for {} (prev={} recv={} pos=({}, {}) intent=({}, {}))",
                            seq_gap,
                            player_id,
                            prev_seq,
                            move_seq,
                            player.x,
                            player.y,
                            player.move_dx,
                            player.move_dy
                        );
                        player.last_move_input_warn_ms = now_ms;
                    }
                }

                // Track movement-input cadence for diagnostics.
                if player.last_move_input_ms > 0 && (player.move_dx != 0 || player.move_dy != 0) {
                    let gap_ms = now_ms.saturating_sub(player.last_move_input_ms);
                    if gap_ms > MOVE_INPUT_GAP_WARN_MS {
                        self.movement_anomalies
                            .input_gap_events
                            .fetch_add(1, Ordering::Relaxed);
                        if now_ms.saturating_sub(player.last_move_input_warn_ms)
                            >= MOVE_INPUT_WARN_THROTTLE_MS
                        {
                            tracing::warn!(
                                "Move input gap {}ms for {} (seq={} pos=({}, {}) intent=({}, {}))",
                                gap_ms,
                                player_id,
                                move_seq,
                                player.x,
                                player.y,
                                player.move_dx,
                                player.move_dy
                            );
                            player.last_move_input_warn_ms = now_ms;
                        }
                    }
                }
                player.last_move_input_ms = now_ms;

                // Block movement while stall is active
                if player.stall.as_ref().map_or(false, |s| s.active) {
                    return;
                }

                // Auto-stand when trying to move while sitting (only in chair facing direction)
                if let Some(pos) = player.sitting_at {
                    // Determine intended movement direction
                    let move_dir = if dx.abs() > dy.abs() {
                        if dx > 0.1 {
                            Some(Direction::Right)
                        } else if dx < -0.1 {
                            Some(Direction::Left)
                        } else {
                            None
                        }
                    } else if dy.abs() > 0.1 {
                        if dy > 0.1 {
                            Some(Direction::Down)
                        } else {
                            Some(Direction::Up)
                        }
                    } else {
                        None
                    };

                    // Only allow standing up when moving in the chair's facing direction
                    if move_dir == Some(player.direction) {
                        let (fdx, fdy) = match player.direction {
                            Direction::Up => (0, -1),
                            Direction::Down => (0, 1),
                            Direction::Left => (-1, 0),
                            Direction::Right => (1, 0),
                            _ => (0, 0),
                        };
                        player.x = pos.0 + fdx;
                        player.y = pos.1 + fdy;
                        player.sitting_at = None;
                        chair_to_free = Some(pos);
                    }
                    // Either way, don't process actual movement while sitting
                    player.mark_move_seq_processed(move_seq);
                    player.clear_move_intent();
                } else {
                    // Convert to grid movement (-1, 0, or 1)
                    // Supports diagonal movement (both axes non-zero)
                    let move_dx = if dx > 0.1 {
                        1
                    } else if dx < -0.1 {
                        -1
                    } else {
                        0
                    };
                    let move_dy = if dy > 0.1 {
                        1
                    } else if dy < -0.1 {
                        -1
                    } else {
                        0
                    };

                    // Queue movement intent only. Facing updates when a move is
                    // actually applied in the tick loop.
                    if move_dx == 0 && move_dy == 0 {
                        // Stop intent: clear everything including last-move vel.
                        player.mark_move_seq_processed(move_seq);
                        player.stop_moving();
                    } else {
                        player.move_dx = move_dx;
                        player.move_dy = move_dy;
                        player.pending_move_seq = Some(move_seq);
                    }
                }
            }
        }
        // Free chair outside of players lock
        if let Some((tx, ty)) = chair_to_free {
            let mut chairs = self.chairs.write().await;
            if let Some(chair) = chairs.get_mut(&(tx, ty)) {
                if chair.occupied_by.as_deref() == Some(player_id) {
                    chair.occupied_by = None;
                }
            }
        }

        // Close chest if player moved
        self.close_player_chest(player_id).await;
    }

    /// Handle dash - slide up to DASH_DISTANCE tiles in current facing direction
    pub async fn handle_dash(&self, player_id: &str) {
        let current_tick = *self.tick.read().await;

        // Get player state and validate
        let (px, py, direction, last_dash_tick, is_sitting, is_dead, active) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) => (
                    p.x,
                    p.y,
                    p.direction,
                    p.last_dash_tick,
                    p.sitting_at.is_some(),
                    p.is_dead,
                    p.active,
                ),
                None => return,
            }
        };

        if !active || is_dead || is_sitting {
            return;
        }

        // Check cooldown
        if current_tick.saturating_sub(last_dash_tick) < DASH_COOLDOWN_TICKS {
            return;
        }

        // Get direction vector
        let (dx, dy) = match direction {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
            _ => return,
        };

        // Snapshot collision data
        let player_inst = self.player_instances.read().await.get(player_id).cloned();
        let is_overworld = player_inst.is_none();

        let (overworld_player_pos, npc_positions, chair_positions) = {
            let players = self.players.read().await;
            let instances = self.player_instances.read().await;
            let overworld: std::collections::HashSet<(i32, i32)> = players
                .values()
                .filter(|p| {
                    p.active && !p.is_dead && p.id != player_id && !instances.contains_key(&p.id)
                })
                .map(|p| (p.x, p.y))
                .collect();
            drop(instances);
            drop(players);

            let npcs = self.npcs.read().await;
            let npc_pos: std::collections::HashSet<(i32, i32)> = npcs
                .values()
                .filter(|n| n.is_alive())
                .map(|n| (n.x, n.y))
                .collect();
            drop(npcs);

            let chairs = self.chairs.read().await;
            let chair_pos: std::collections::HashSet<(i32, i32)> = chairs.keys().cloned().collect();
            drop(chairs);

            (overworld, npc_pos, chair_pos)
        };

        // Snapshot instance collision data if player is in an instance
        let (inst_collision, inst_width, inst_height, inst_npc_pos, inst_player_pos) =
            if let Some(ref inst_id) = player_inst {
                if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                    let collision = instance.collision.read().await.clone();
                    let width = instance.map_width;
                    let height = instance.map_height;

                    let npcs = instance.npcs.read().await;
                    let npc_pos: std::collections::HashSet<(i32, i32)> = npcs
                        .values()
                        .filter(|n| n.is_alive())
                        .map(|n| (n.x, n.y))
                        .collect();

                    let players = self.players.read().await;
                    let instances = self.player_instances.read().await;
                    let player_pos: std::collections::HashSet<(i32, i32)> = players
                        .values()
                        .filter(|p| {
                            p.active
                                && !p.is_dead
                                && p.id != player_id
                                && instances.get(&p.id).map(|i| i.as_str())
                                    == Some(inst_id.as_str())
                        })
                        .map(|p| (p.x, p.y))
                        .collect();

                    (
                        Some(collision),
                        width,
                        height,
                        Some(npc_pos),
                        Some(player_pos),
                    )
                } else {
                    (None, 0, 0, None, None)
                }
            } else {
                (None, 0, 0, None, None)
            };

        // Walk up to DASH_DISTANCE tiles, stopping at first collision
        let chunks_guard = self.world.chunks_read().await;
        let mut final_x = px;
        let mut final_y = py;

        for step in 1..=DASH_DISTANCE {
            let check_x = px + dx * step;
            let check_y = py + dy * step;

            if is_overworld {
                // Check tile walkability
                let coord = crate::chunk::ChunkCoord::from_world(check_x, check_y);
                let walkable = if let Some(chunk) = chunks_guard.get(&coord) {
                    let (lx, ly) = crate::chunk::world_to_local(check_x, check_y);
                    chunk.is_walkable_local(lx, ly)
                } else {
                    false
                };
                if !walkable {
                    break;
                }

                if overworld_player_pos.contains(&(check_x, check_y)) {
                    break;
                }
                if npc_positions.contains(&(check_x, check_y)) {
                    break;
                }
                if chair_positions.contains(&(check_x, check_y)) {
                    break;
                }
            } else {
                // Instance collision checks
                if let Some(ref collision) = inst_collision {
                    if check_x < 0
                        || check_y < 0
                        || check_x >= inst_width as i32
                        || check_y >= inst_height as i32
                    {
                        break;
                    }
                    let idx = (check_y as u32 * inst_width + check_x as u32) as usize;
                    if collision.get(idx).copied().unwrap_or(true) {
                        break;
                    }
                } else {
                    break; // Instance not found - stop dash for safety
                }
                if let Some(ref player_pos) = inst_player_pos {
                    if player_pos.contains(&(check_x, check_y)) {
                        break;
                    }
                }
                if let Some(ref npc_pos) = inst_npc_pos {
                    if npc_pos.contains(&(check_x, check_y)) {
                        break;
                    }
                }
            }

            final_x = check_x;
            final_y = check_y;
        }
        drop(chunks_guard);

        // Only dash if we can move at least 1 tile
        if final_x == px && final_y == py {
            return;
        }

        // Apply the dash
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.x = final_x;
                player.y = final_y;
                player.last_dash_tick = current_tick;
                player.last_move_tick = current_tick;
                player.is_dashing = true;
                player.reject_pending_move();
            }
        }
    }

    /// Handle jump command - initiate a jump if the player is grounded
    pub async fn handle_jump(&self, player_id: &str) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            if player.grounded && !player.is_dead && player.active {
                player.grounded = false;
                player.jump_ticks = 6; // 6 ticks = 300ms airtime at 20Hz
            }
        }
    }

    /// Handle face command - change direction without moving
    pub async fn handle_face(&self, player_id: &str, direction: u8) {
        let (player_x, player_y, face_dir) = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                // Don't allow direction changes while sitting
                if player.sitting_at.is_some() {
                    return;
                }
                let face_dir = Direction::from_u8(direction);
                player.direction = face_dir;
                // Ensure player is not moving when just facing
                player.reject_pending_move();
                (player.x, player.y, face_dir)
            } else {
                tracing::warn!("handle_face: player not found: {}", player_id);
                return;
            }
        };

        // Determine gathering interruption outside players lock.
        let should_stop_gathering = {
            let gathering = self.gathering.read().await;
            if !gathering.is_gathering(player_id) {
                false
            } else {
                let (fdx, fdy): (i32, i32) = match face_dir {
                    Direction::Down => (0, 1),
                    Direction::Up => (0, -1),
                    Direction::Left => (-1, 0),
                    Direction::Right => (1, 0),
                    Direction::DownLeft => (-1, 1),
                    Direction::DownRight => (1, 1),
                    Direction::UpLeft => (-1, -1),
                    Direction::UpRight => (1, -1),
                };
                let face_x = player_x + fdx;
                let face_y = player_y + fdy;
                let facing_marker = gathering
                    .markers
                    .iter()
                    .any(|m| m.x == face_x && m.y == face_y);
                !facing_marker
            }
        };

        if should_stop_gathering {
            self.handle_stop_gathering(player_id).await;
        }
    }

    pub async fn handle_attack(
        &self,
        player_id: &str,
        direction_override: Option<Direction>,
        forced_target_id: Option<&str>,
    ) {
        // Determine attacker's instance context (None = overworld)
        let attacker_instance = self.player_instances.read().await.get(player_id).cloned();

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Get attacker info including combat stats
        // When direction_override is provided (auto-action), atomically set and read the
        // direction in the same lock to prevent race conditions with client Face commands.
        let (
            attacker_name,
            attacker_x,
            attacker_y,
            attacker_dir,
            last_attack,
            attack_level,
            strength_level,
            attack_bonus,
            strength_bonus,
            equipped_head,
            combat_style,
        ) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) => p,
                None => {
                    tracing::warn!("Attack failed: player {} not found", player_id);
                    return;
                }
            };

            // Dead players can't attack
            if player.is_dead {
                return;
            }

            // Apply direction override atomically before reading direction
            if let Some(dir) = direction_override {
                player.direction = dir;
            }

            let base_atk_bonus = player.attack_bonus(&self.item_registry);
            let base_str_bonus = player.strength_bonus(&self.item_registry);

            // Apply prayer bonuses to attack and strength
            let active_ids: Vec<String> = player.active_prayers.iter().cloned().collect();
            let prayer_effects = self.prayer_registry.calculate_effects(&active_ids);
            let atk_bonus = prayer_effects.apply_attack_bonus(base_atk_bonus);
            let str_bonus = prayer_effects.apply_strength_bonus(base_str_bonus);

            (
                player.name.clone(),
                player.x,
                player.y,
                player.direction,
                player.last_attack_time,
                player.skills.attack.level,
                player.skills.strength.level,
                atk_bonus,
                str_bonus,
                player.equipped_head.clone(),
                player.combat_style,
            )
        };

        // Get weapon range and type (needed before cooldown check)
        let (mut weapon_range, weapon_type) = {
            let players = self.players.read().await;
            if let Some(player) = players.get(player_id) {
                if let Some(ref weapon_id) = player.equipped_weapon {
                    if let Some(item_def) = self.item_registry.get(weapon_id) {
                        if let Some(ref equip) = item_def.equipment {
                            (equip.range, equip.weapon_type)
                        } else {
                            (1, WeaponType::Melee)
                        }
                    } else {
                        (1, WeaponType::Melee)
                    }
                } else {
                    (1, WeaponType::Melee) // Unarmed = melee range 1
                }
            } else {
                return;
            }
        };

        // Check cooldown (ranged has a longer cooldown to balance range advantage)
        let cooldown = if weapon_type == WeaponType::Ranged {
            RANGED_ATTACK_COOLDOWN_MS
        } else {
            ATTACK_COOLDOWN_MS
        };
        if current_time - last_attack < cooldown {
            return;
        }

        // For ranged weapons, override attack/strength with ranged level and apply style bonuses
        let (attack_level, strength_level) = if weapon_type == WeaponType::Ranged {
            let ranged_level = {
                let players = self.players.read().await;
                players.get(player_id).map(|p| p.skills.ranged.level).unwrap_or(1)
            };
            // Accurate style: +3 to effective ranged level for accuracy
            let effective_ranged = if combat_style == CombatStyle::Accurate {
                ranged_level + 3
            } else {
                ranged_level
            };
            // Longrange style: +2 to weapon range
            if combat_style == CombatStyle::Longrange {
                weapon_range += 2;
            }
            // Ranged uses ranged_level for both accuracy and max hit
            (effective_ranged, ranged_level)
        } else {
            (attack_level, strength_level)
        };

        // For ranged weapons, check the player has arrows (but don't consume yet —
        // arrows are only consumed when an actual target is found)
        if weapon_type == WeaponType::Ranged {
            let players = self.players.read().await;
            if let Some(player) = players.get(player_id) {
                let has_arrows = player.inventory.slots.iter().any(|slot| {
                    slot.as_ref()
                        .map_or(false, |s| s.item_id.ends_with("_arrow"))
                });
                if !has_arrows {
                    drop(players);
                    self.send_to_player(
                        player_id,
                        ServerMessage::AttackResult {
                            success: false,
                            reason: Some("no_arrows".to_string()),
                        },
                    )
                    .await;
                    return;
                }
            } else {
                return;
            }
        }

        // Broadcast attack animation to all clients (plays even if no target hit)
        let attack_type = match weapon_type {
            WeaponType::Ranged => "ranged",
            WeaponType::Melee => "melee",
        };
        self.broadcast(ServerMessage::PlayerAttack {
            player_id: player_id.to_string(),
            attack_type: attack_type.to_string(),
            direction: attacker_dir as u8,
        })
        .await;

        // Update attacker's last attack time and stop movement BEFORE target scan.
        // This prevents rapid-fire when attacks miss (no target found in scan direction).
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.last_attack_time = current_time;
                // Stop movement when attacking (player must stand still to attack)
                player.reject_pending_move();
            }
        }

        // Find target based on weapon range
        let mut target_id: Option<String> = None;
        let mut is_npc = false;
        let mut is_instance_npc = false;
        let mut target_tile_x = attacker_x;
        let mut target_tile_y = attacker_y;

        if let Some(forced_id) = forced_target_id {
            // Auto-action: directly target the known entity (bypasses directional scan
            // which can miss targets not on a cardinal/diagonal line)
            if attacker_instance.is_none() {
                let npcs = self.npcs.read().await;
                if let Some(npc) = npcs.get(forced_id) {
                    if npc.is_alive() && npc.is_attackable() {
                        target_id = Some(forced_id.to_string());
                        is_npc = true;
                        target_tile_x = npc.x;
                        target_tile_y = npc.y;
                    }
                }
            } else if let Some(ref inst_id) = attacker_instance {
                if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                    let npcs = instance.npcs.read().await;
                    if let Some(npc) = npcs.get(forced_id) {
                        if npc.is_alive() && npc.is_attackable() {
                            target_id = Some(forced_id.to_string());
                            is_npc = true;
                            is_instance_npc = true;
                            target_tile_x = npc.x;
                            target_tile_y = npc.y;
                        }
                    }
                }
            }
            // Also check players as forced target
            if target_id.is_none() {
                let players = self.players.read().await;
                if let Some(player) = players.get(forced_id) {
                    if player.active && player.hp > 0 {
                        let instances = self.player_instances.read().await;
                        let target_instance = instances.get(forced_id).cloned();
                        if target_instance == attacker_instance {
                            target_id = Some(forced_id.to_string());
                            is_npc = false;
                            target_tile_x = player.x;
                            target_tile_y = player.y;
                        }
                    }
                }
            }
        } else {
            // Manual attack: scan tiles in facing direction up to weapon range
            let (dir_dx, dir_dy): (i32, i32) = match attacker_dir {
                Direction::Up => (0, -1),
                Direction::Down => (0, 1),
                Direction::Left => (-1, 0),
                Direction::Right => (1, 0),
                Direction::UpLeft => (-1, -1),
                Direction::UpRight => (1, -1),
                Direction::DownLeft => (-1, 1),
                Direction::DownRight => (1, 1),
            };

            for dist in 1..=weapon_range {
                let check_x = attacker_x + dir_dx * dist;
                let check_y = attacker_y + dir_dy * dist;

                // For ranged weapons, check line of sight
                if weapon_range > 1
                    && !self
                        .world
                        .has_line_of_sight(attacker_x, attacker_y, check_x, check_y)
                        .await
                {
                    tracing::debug!(
                        "{} ranged attack blocked by wall at ({}, {})",
                        attacker_name,
                        check_x,
                        check_y
                    );
                    break;
                }

                // Check NPCs at this tile
                if attacker_instance.is_none() {
                    // Overworld NPCs
                    let npcs = self.npcs.read().await;
                    for (npc_id, npc) in npcs.iter() {
                        if npc.is_alive()
                            && npc.is_attackable()
                            && npc.x == check_x
                            && npc.y == check_y
                        {
                            target_id = Some(npc_id.clone());
                            is_npc = true;
                            target_tile_x = check_x;
                            target_tile_y = check_y;
                            tracing::info!(
                                "{} found NPC target: {} at ({}, {}) range {}",
                                attacker_name,
                                npc.name(),
                                check_x,
                                check_y,
                                dist
                            );
                            break;
                        }
                    }
                } else if let Some(ref inst_id) = attacker_instance {
                    // Instance NPCs
                    if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                        let npcs = instance.npcs.read().await;
                        for (npc_id, npc) in npcs.iter() {
                            if npc.is_alive()
                                && npc.is_attackable()
                                && npc.x == check_x
                                && npc.y == check_y
                            {
                                target_id = Some(npc_id.clone());
                                is_npc = true;
                                is_instance_npc = true;
                                target_tile_x = check_x;
                                target_tile_y = check_y;
                                tracing::info!(
                                    "{} found instance NPC target: {} at ({}, {}) range {}",
                                    attacker_name,
                                    npc.name(),
                                    check_x,
                                    check_y,
                                    dist
                                );
                                break;
                            }
                        }
                    }
                }
                if target_id.is_some() {
                    break;
                }

                // Check players at this tile (must be in same instance context)
                {
                    let players = self.players.read().await;
                    let instances = self.player_instances.read().await;
                    for (pid, player) in players.iter() {
                        if pid != player_id
                            && player.active
                            && player.hp > 0
                            && player.x == check_x
                            && player.y == check_y
                        {
                            // Only target players in the same context (both overworld, or same instance)
                            let target_instance = instances.get(pid.as_str()).cloned();
                            if target_instance != attacker_instance {
                                continue;
                            }
                            target_id = Some(pid.clone());
                            is_npc = false;
                            target_tile_x = check_x;
                            target_tile_y = check_y;
                            tracing::info!(
                                "{} found player target: {} at ({}, {}) range {}",
                                attacker_name,
                                player.name,
                                check_x,
                                check_y,
                                dist
                            );
                            break;
                        }
                    }
                }
                if target_id.is_some() {
                    break;
                }
            }
        }

        // No valid target found
        let target_id = match target_id {
            Some(id) => id,
            None => {
                tracing::debug!(
                    "{} attack missed - no target in range {} facing {:?}",
                    attacker_name,
                    weapon_range,
                    attacker_dir
                );
                return;
            }
        };

        // In slayer-only areas, players can only attack NPCs matching their active slayer task
        if is_npc {
            if let Some(ref inst_id) = attacker_instance {
                if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                    if let Some(interior) = self.interior_registry.get(&instance.map_id) {
                        if interior.requires_slayer_task {
                            let slayer_state = self.get_player_slayer_state(player_id).await;
                            let npc_prototype = if is_instance_npc {
                                let npcs = instance.npcs.read().await;
                                npcs.get(&target_id).map(|n| n.prototype_id.clone())
                            } else {
                                None
                            };
                            if let Some(proto_id) = npc_prototype {
                                let allowed = match &slayer_state.current_task {
                                    Some(task) => {
                                        proto_id == task.monster_id
                                            || proto_id
                                                .starts_with(&format!("{}_", task.monster_id))
                                    }
                                    None => false,
                                };
                                if !allowed {
                                    self.send_system_message(player_id, "You can only attack your slayer task monster in this area.").await;
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Now that we have a confirmed target, consume 1 arrow for ranged weapons
        // and add the arrow's ranged_strength bonus to damage
        let arrow_strength_bonus = if weapon_type == WeaponType::Ranged {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                let arrow_id = player.inventory.slots.iter().find_map(|slot| {
                    slot.as_ref()
                        .filter(|s| s.item_id.ends_with("_arrow"))
                        .map(|s| s.item_id.clone())
                });
                if let Some(arrow_id) = arrow_id {
                    player.inventory.remove_item(&arrow_id, 1);
                    let inv_update = player.inventory.to_update();
                    let gold = player.inventory.gold;
                    drop(players);
                    self.send_to_player(
                        player_id,
                        ServerMessage::InventoryUpdate {
                            player_id: player_id.to_string(),
                            slots: inv_update,
                            gold,
                        },
                    )
                    .await;
                    // Look up arrow's ranged_strength bonus
                    self.item_registry.get(&arrow_id).map(|def| def.ranged_strength).unwrap_or(0)
                } else {
                    // Arrows ran out between check and consumption (unlikely but safe)
                    return;
                }
            } else {
                return;
            }
        } else {
            0
        };
        let strength_bonus = strength_bonus + arrow_strength_bonus;

        // Fetch slayer state for helmet damage boost check (only if wearing slayer helmet)
        let slayer_task_monster = if equipped_head.as_deref() == Some("slayer_helmet") {
            let slayer_state = self.get_player_slayer_state(player_id).await;
            slayer_state.current_task.map(|t| t.monster_id)
        } else {
            None
        };

        // Apply damage to target using hit/miss mechanics
        // 1. Roll attack vs defence to determine if we hit
        // 2. If hit, calculate max hit from strength and roll damage
        let (target_hp, target_name, target_died, actual_damage) = if is_npc && is_instance_npc {
            // Instance NPC combat
            let instance = self
                .instance_manager
                .get_by_instance_id(attacker_instance.as_ref().unwrap());
            if let Some(inst) = instance {
                let mut npcs = inst.npcs.write().await;
                if let Some(npc) = npcs.get_mut(&target_id) {
                    // Invulnerable NPCs (e.g. boss underground) cannot be hit
                    if npc.invulnerable {
                        let name = npc.name();
                        (npc.hp, name, false, 0)
                    } else {
                    let npc_defence_level = npc.level;
                    let npc_defence_bonus = npc.stats.defence_bonus;

                    if !calculate_hit(
                        attack_level,
                        attack_bonus,
                        npc_defence_level,
                        npc_defence_bonus,
                    ) {
                        npc.take_damage(0, current_time, Some(player_id));
                        let name = npc.name();
                        tracing::info!(
                            "{} misses instance NPC {} (atk {} + {} vs def {} + {})",
                            attacker_name,
                            name,
                            attack_level,
                            attack_bonus,
                            npc_defence_level,
                            npc_defence_bonus
                        );
                        (npc.hp, name, false, 0)
                    } else {
                        let mut max_hit = calculate_max_hit(strength_level, strength_bonus);
                        // Slayer helmet: 15% damage boost against current slayer task
                        if let Some(ref task_monster) = slayer_task_monster {
                            let proto = &npc.prototype_id;
                            if proto == task_monster
                                || proto.starts_with(&format!("{}_", task_monster))
                            {
                                max_hit = ((max_hit as f32) * 1.15).floor() as i32;
                            }
                        }
                        let damage = roll_damage(max_hit).min(npc.hp);
                        let died = npc.take_damage(damage, current_time, Some(player_id));
                        let name = npc.name();
                        tracing::info!(
                            "{} hits instance NPC {} for {} damage (max: {}, HP: {})",
                            attacker_name,
                            name,
                            damage,
                            max_hit,
                            npc.hp
                        );
                        (npc.hp, name, died, damage)
                    }
                    } // end invulnerable else
                } else {
                    return;
                }
            } else {
                return;
            }
        } else if is_npc {
            // Overworld NPC combat
            let mut npcs = self.npcs.write().await;
            if let Some(npc) = npcs.get_mut(&target_id) {
                // NPC's defence = level, no equipment bonus
                let npc_defence_level = npc.level;
                let npc_defence_bonus = npc.stats.defence_bonus;

                // Check if attack hits (attack_level for accuracy)
                if !calculate_hit(
                    attack_level,
                    attack_bonus,
                    npc_defence_level,
                    npc_defence_bonus,
                ) {
                    // Miss - deal 0 damage
                    // Still register aggro so attack attempts interrupt wandering/pathing.
                    npc.take_damage(0, current_time, Some(player_id));
                    let name = npc.name();
                    tracing::info!(
                        "{} misses {} (atk {} + {} vs def {} + {})",
                        attacker_name,
                        name,
                        attack_level,
                        attack_bonus,
                        npc_defence_level,
                        npc_defence_bonus
                    );
                    (npc.hp, name, false, 0)
                } else {
                    // Hit - calculate and apply damage
                    let mut max_hit = calculate_max_hit(strength_level, strength_bonus);
                    // Slayer helmet: 15% damage boost against current slayer task
                    if let Some(ref task_monster) = slayer_task_monster {
                        let proto = &npc.prototype_id;
                        if proto == task_monster || proto.starts_with(&format!("{}_", task_monster))
                        {
                            max_hit = ((max_hit as f32) * 1.15).floor() as i32;
                        }
                    }
                    let damage = roll_damage(max_hit).min(npc.hp);
                    let died = npc.take_damage(damage, current_time, Some(player_id));
                    let name = npc.name();
                    tracing::info!(
                        "{} hits {} for {} damage (max: {}, HP: {})",
                        attacker_name,
                        name,
                        damage,
                        max_hit,
                        npc.hp
                    );
                    (npc.hp, name, died, damage)
                }
            } else {
                return;
            }
        } else {
            // Players have defence from skills and equipment
            let mut players = self.players.write().await;
            if let Some(target) = players.get_mut(&target_id) {
                if target.is_dead {
                    return; // Already dead
                }
                // God mode prevents all damage
                if target.is_god_mode {
                    return;
                }

                // Get target's defence stats
                let target_defence_level = target.skills.defence.level;
                let base_defence_bonus = target.defence_bonus(&self.item_registry);

                // Apply prayer bonuses to target's defence
                let target_active_ids: Vec<String> =
                    target.active_prayers.iter().cloned().collect();
                let target_prayer_effects =
                    self.prayer_registry.calculate_effects(&target_active_ids);
                let target_defence_bonus =
                    target_prayer_effects.apply_defence_bonus(base_defence_bonus);

                // Check if attack hits
                if !calculate_hit(
                    attack_level,
                    attack_bonus,
                    target_defence_level,
                    target_defence_bonus,
                ) {
                    // Miss - deal 0 damage
                    let name = target.name.clone();
                    tracing::info!(
                        "{} misses {} (atk {} + {} vs def {} + {})",
                        attacker_name,
                        name,
                        attack_level,
                        attack_bonus,
                        target_defence_level,
                        target_defence_bonus
                    );
                    (target.hp, name, false, 0)
                } else {
                    // Hit - calculate and apply damage
                    let max_hit = calculate_max_hit(strength_level, strength_bonus);
                    let raw_damage = roll_damage(max_hit);
                    // Apply prayer damage reduction, then clamp to remaining HP
                    let damage = target_prayer_effects.apply_damage_reduction(raw_damage).min(target.hp);
                    target.hp -= damage;
                    let name = target.name.clone();
                    let died = target.hp <= 0;
                    if died {
                        target.die(current_time);
                    }
                    tracing::info!(
                        "{} hits {} for {} damage (max: {}, raw: {}, HP: {})",
                        attacker_name,
                        name,
                        damage,
                        max_hit,
                        raw_damage,
                        target.hp
                    );
                    (target.hp, name, died, damage)
                }
            } else {
                return;
            }
        };

        // Use actual target position for damage event (important for ranged projectiles)
        let target_x = target_tile_x as f32;
        let target_y = target_tile_y as f32;

        // Determine projectile type for ranged attacks
        let projectile = if weapon_type == WeaponType::Ranged {
            Some("arrow".to_string())
        } else {
            None
        };

        // Broadcast damage event to players in the same zone (instance or overworld)
        let damage_msg = ServerMessage::DamageEvent {
            source_id: player_id.to_string(),
            target_id: target_id.clone(),
            damage: actual_damage,
            target_hp,
            target_x,
            target_y,
            projectile,
        };
        self.broadcast_to_zone(player_id, damage_msg).await;

        // Send success result to attacker
        let result_msg = ServerMessage::AttackResult {
            success: true,
            reason: None,
        };
        self.broadcast(result_msg).await;

        // Award combat XP on every successful hit (OSRS style: XP per damage dealt)
        if actual_damage > 0 {
            let xp_results = {
                let mut players = self.players.write().await;
                if let Some(attacker) = players.get_mut(player_id) {
                    let style = attacker.combat_style;
                    Some(attacker.award_combat_xp(actual_damage, style, weapon_type))
                } else {
                    None
                }
            };

            if let Some(results) = xp_results {
                let mut progression_needs_sync = false;
                for (skill_type, xp_gained, total_xp, level, leveled_up) in results {
                    self.send_to_player(
                        player_id,
                        ServerMessage::SkillXp {
                            player_id: player_id.to_string(),
                            skill: skill_type.as_str().to_string(),
                            xp_gained,
                            total_xp,
                            level,
                        },
                    )
                    .await;

                    if leveled_up {
                        tracing::info!(
                            "Player {} leveled up {} to {}",
                            player_id,
                            skill_type.as_str(),
                            level
                        );
                        self.broadcast_skill_level_up(player_id, skill_type.as_str(), level).await;
                        progression_needs_sync = true;
                    }
                }

                if progression_needs_sync {
                    self.process_quest_progression_snapshot(player_id).await;
                }
            }
        }

        // Interrupt crafting if target is a player who took damage
        if !is_npc && actual_damage > 0 {
            self.cancel_crafting(&target_id, "interrupted").await;
        }

        // Handle death
        if target_died {
            tracing::info!("{} killed {}", attacker_name, target_name);
            if is_npc {
                // Get NPC info for exp and loot
                let (prototype_id, npc_level) = if is_instance_npc {
                    let inst = self
                        .instance_manager
                        .get_by_instance_id(attacker_instance.as_ref().unwrap());
                    if let Some(inst) = inst {
                        let npcs = inst.npcs.read().await;
                        npcs.get(&target_id)
                            .map(|n| (n.prototype_id.clone(), n.level))
                            .unwrap_or(("unknown".to_string(), 1))
                    } else {
                        ("unknown".to_string(), 1)
                    }
                } else {
                    let npcs = self.npcs.read().await;
                    npcs.get(&target_id)
                        .map(|n| (n.prototype_id.clone(), n.level))
                        .unwrap_or(("unknown".to_string(), 1))
                };

                // Broadcast NPC death (scoped to zone)
                let death_msg = ServerMessage::NpcDied {
                    id: target_id.clone(),
                    killer_id: player_id.to_string(),
                };
                self.broadcast_to_zone(player_id, death_msg).await;

                // Clear attacker's auto-action since target is dead
                self.clear_auto_action(player_id, "target_dead").await;

                // Persist monster kill count for stats leaderboards.
                self.record_monster_kill(player_id).await;

                // Process quest kill event
                self.process_quest_kill(player_id, &prototype_id).await;

                // Process slayer kill event
                self.process_slayer_kill(player_id, &prototype_id).await;

                // Check KOTH NPC death
                if let Some(ref inst_id) = attacker_instance {
                    let ct = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    self.check_koth_npc_death(&target_id, inst_id, ct).await;

                    // Check boss minion death (player killed a minion via combat)
                    self.check_boss_minion_death(
                        &target_id,
                        inst_id,
                        target_x as i32,
                        target_y as i32,
                        ct,
                    )
                    .await;

                    // Check boss NPC death (player killed the boss)
                    self.check_boss_npc_death(
                        &target_id,
                        inst_id,
                        Some(player_id),
                        ct,
                    )
                    .await;
                }

                // Skip loot drops in boss arena (rewards come from battle master)
                let in_boss_arena = attacker_instance.as_ref()
                    .map(|id| id.contains(crate::game::boss_tick::BOSS_MAP_ID))
                    .unwrap_or(false);

                // Spawn item drops from prototype loot table
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                let drops = if in_boss_arena {
                    vec![]
                } else {
                    // Get killer's current instance for loot zone tracking
                    let killer_instance = {
                        let instances = self.player_instances.read().await;
                        instances.get(player_id).cloned()
                    };

                    if let Some(prototype) = self.entity_registry.get(&prototype_id) {
                        crate::entity::generate_loot_from_prototype(
                            prototype,
                            target_x,
                            target_y,
                            player_id,
                            current_time,
                            npc_level,
                            killer_instance,
                        )
                    } else {
                        vec![]
                    }
                };

                for item in drops {
                    let mut items = self.ground_items.write().await;

                    // For gold, try to combine with existing pile at same tile
                    if item.item_id == "gold" {
                        let tile_x = item.x.floor() as i32;
                        let tile_y = item.y.floor() as i32;

                        // Find existing gold at same tile with same owner
                        let existing_gold_id = items
                            .iter()
                            .find(|(_, existing)| {
                                existing.item_id == "gold"
                                    && existing.x.floor() as i32 == tile_x
                                    && existing.y.floor() as i32 == tile_y
                                    && existing.owner_id == item.owner_id
                            })
                            .map(|(id, _)| id.clone());

                        if let Some(existing_id) = existing_gold_id {
                            // Combine with existing pile
                            if let Some(existing) = items.get_mut(&existing_id) {
                                existing.quantity += item.quantity;
                                let update_msg = ServerMessage::ItemQuantityUpdated {
                                    id: existing_id.clone(),
                                    quantity: existing.quantity,
                                };
                                drop(items); // Release lock before broadcast
                                self.broadcast_to_zone(player_id, update_msg).await;
                            }
                            continue;
                        }
                    }

                    // No existing pile to combine with - create new item
                    let drop_msg = ServerMessage::ItemDropped {
                        id: item.id.clone(),
                        item_id: item.item_id.clone(),
                        x: item.x,
                        y: item.y,
                        quantity: item.quantity,
                    };
                    items.insert(item.id.clone(), item);
                    drop(items); // Release lock before broadcast
                    self.broadcast_to_zone(player_id, drop_msg).await;
                }
            } else {
                // Check if this is an arena fight death
                let arena_death = {
                    let arena = self.arena_manager.read().await;
                    arena.is_fighting() && arena.is_in_ring(&target_id)
                };

                if arena_death {
                    // Arena death: notify arena, teleport to spectator zone
                    let (eliminated_name, killer_name, remaining) = {
                        let mut arena = self.arena_manager.write().await;
                        arena.on_player_death(&target_id, Some(player_id));
                        let eliminated_name = arena
                            .match_stats
                            .fighter_names
                            .get(&target_id)
                            .cloned()
                            .unwrap_or_default();
                        let killer_name = arena
                            .match_stats
                            .fighter_names
                            .get(player_id)
                            .cloned()
                            .unwrap_or_default();
                        let remaining = arena.active_fighters.len() as u32;
                        (eliminated_name, killer_name, remaining)
                    };

                    // Teleport dead player to spectator spawn instead of normal death
                    {
                        let spectator_spawn = {
                            let arena = self.arena_manager.read().await;
                            arena.active_spectator_spawn()
                        };
                        let mut players = self.players.write().await;
                        if let Some(p) = players.get_mut(&target_id) {
                            p.hp = p.skills.hitpoints.level; // Revive
                            p.is_dead = false;
                            p.x = spectator_spawn.0;
                            p.y = spectator_spawn.1;
                        }
                    }

                    // Broadcast elimination
                    self.broadcast_to_arena(ServerMessage::ArenaPlayerEliminated {
                        player_id: target_id.clone(),
                        player_name: eliminated_name,
                        killer_id: player_id.to_string(),
                        killer_name,
                        remaining,
                    })
                    .await;

                    // Check if match should end
                    let should_end = {
                        let arena = self.arena_manager.read().await;
                        tracing::info!(
                            "[ARENA] After death: active_fighters={:?}, state={:?}, check_match_end={}",
                            arena.active_fighters,
                            arena.state,
                            arena.check_match_end()
                        );
                        arena.check_match_end()
                    };
                    if should_end {
                        let current_time = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64;
                        let placements = {
                            let mut arena = self.arena_manager.write().await;
                            arena.end_match(current_time)
                        };
                        tracing::info!("[ARENA] Match ended! {} placements", placements.len());

                        // Distribute rewards
                        {
                            let mut players = self.players.write().await;
                            for placement in &placements {
                                if placement.gold_reward > 0 {
                                    if let Some(p) = players.get_mut(&placement.player_id) {
                                        p.inventory.gold += placement.gold_reward;
                                    }
                                }
                            }
                        }

                        let placement_data: Vec<crate::protocol::ArenaPlacementData> = placements
                            .iter()
                            .map(|p| crate::protocol::ArenaPlacementData {
                                rank: p.rank,
                                player_id: p.player_id.clone(),
                                player_name: p.player_name.clone(),
                                kills: p.kills,
                                gold_reward: p.gold_reward,
                            })
                            .collect();

                        self.broadcast_to_arena(ServerMessage::ArenaMatchEnd {
                            placements: placement_data,
                        })
                        .await;

                        self.broadcast_to_arena(ServerMessage::ArenaStateUpdate {
                            state: "results".to_string(),
                            countdown_remaining: None,
                            queued_count: 0,
                            fighter_count: 0,
                            entry_fee: {
                                let arena = self.arena_manager.read().await;
                                arena.config.entry_fee
                            },
                        })
                        .await;

                        // Teleport all fighters (including winner) to spectator spawn
                        {
                            let spectator_spawn = {
                                let arena = self.arena_manager.read().await;
                                arena.active_spectator_spawn()
                            };
                            let mut players = self.players.write().await;
                            for placement in &placements {
                                if let Some(p) = players.get_mut(&placement.player_id) {
                                    p.x = spectator_spawn.0;
                                    p.y = spectator_spawn.1;
                                    if p.is_dead {
                                        p.hp = p.skills.hitpoints.level;
                                        p.is_dead = false;
                                    }
                                }
                            }
                        }

                        // Send inventory updates for gold rewards
                        for placement in &placements {
                            if placement.gold_reward > 0 {
                                let update = {
                                    let players = self.players.read().await;
                                    players
                                        .get(&placement.player_id)
                                        .map(|p| (p.inventory.to_update(), p.inventory.gold))
                                };
                                if let Some((slots, gold)) = update {
                                    self.send_to_player(
                                        &placement.player_id,
                                        ServerMessage::InventoryUpdate {
                                            player_id: placement.player_id.clone(),
                                            slots,
                                            gold,
                                        },
                                    )
                                    .await;
                                }
                            }
                        }

                        // Save arena stats to DB
                        if let Some(ref db) = self.db {
                            for placement in &placements {
                                if let Some(char_id) = placement
                                    .player_id
                                    .strip_prefix("char_")
                                    .and_then(|s| s.parse::<i64>().ok())
                                {
                                    let won = placement.rank == 1;
                                    let died = placement.rank > 1;
                                    if let Err(e) = db
                                        .update_arena_stats(
                                            char_id,
                                            won,
                                            placement.kills,
                                            died,
                                            placement.gold_reward,
                                        )
                                        .await
                                    {
                                        tracing::warn!(
                                            "Failed to save arena stats for {}: {}",
                                            placement.player_id,
                                            e
                                        );
                                    }
                                }
                            }
                        }

                        // Notify winner
                        if let Some(winner) = placements.iter().find(|p| p.rank == 1) {
                            self.send_system_message(
                                &winner.player_id,
                                &format!("You won the arena match! +{} gold", winner.gold_reward),
                            )
                            .await;
                        }
                    }
                } else {
                    // Normal player death
                    let death_msg = ServerMessage::PlayerDied {
                        id: target_id.clone(),
                        killer_id: player_id.to_string(),
                    };
                    self.broadcast(death_msg).await;

                    self.clear_auto_action(&target_id, "player_died").await;

                    // Send prayer state update to dying player (prayers cleared on death)
                    let (points, max_points) = {
                        let players = self.players.read().await;
                        if let Some(p) = players.get(&target_id) {
                            (p.prayer_points, p.max_prayer_points())
                        } else {
                            (0, 1)
                        }
                    };
                    self.send_to_player(
                        &target_id,
                        ServerMessage::PrayerStateUpdate {
                            points,
                            max_points,
                            active_prayers: vec![], // Cleared on death
                        },
                    )
                    .await;
                }
            }
        }
    }

    /// Check if two directions are close enough (within 45 degrees)
    fn directions_match(dir1: Direction, dir2: Direction) -> bool {
        // Convert to numeric for comparison
        let d1 = dir1 as i32;
        let d2 = dir2 as i32;
        let diff = (d1 - d2).abs();
        // Directions match if they're the same or adjacent (with wraparound)
        diff <= 1 || diff == 7
    }

    pub async fn handle_target(&self, player_id: &str, target_id: &str) {
        tracing::info!(
            "Target request: player {} -> target '{}'",
            player_id,
            target_id
        );

        // Validate target exists (can be player or NPC)
        let valid_target = {
            if target_id.is_empty() {
                true // Clear target
            } else if target_id == player_id {
                false // Can't target self
            } else {
                // Check if target is a player
                let players = self.players.read().await;
                let is_player = players.get(target_id).map(|p| p.active).unwrap_or(false);
                drop(players);

                if is_player {
                    true
                } else {
                    // Check if target is an NPC (overworld first, then instance)
                    let npcs = self.npcs.read().await;
                    let is_overworld_npc =
                        npcs.get(target_id).map(|n| n.is_alive()).unwrap_or(false);
                    drop(npcs);

                    if is_overworld_npc {
                        true
                    } else {
                        // Check instance NPCs
                        let player_inst =
                            self.player_instances.read().await.get(player_id).cloned();
                        if let Some(inst_id) = player_inst {
                            if let Some(instance) =
                                self.instance_manager.get_by_instance_id(&inst_id)
                            {
                                let inst_npcs = instance.npcs.read().await;
                                inst_npcs
                                    .get(target_id)
                                    .map(|n| n.is_alive())
                                    .unwrap_or(false)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                }
            }
        };

        if valid_target {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                let new_target = if target_id.is_empty() {
                    None
                } else {
                    Some(target_id.to_string())
                };
                player.target_id = new_target.clone();
                tracing::info!("{} now targeting {:?}", player.name, new_target);

                // Broadcast target change to all clients
                let msg = ServerMessage::TargetChanged {
                    player_id: player_id.to_string(),
                    target_id: new_target,
                };
                drop(players); // Release lock before broadcast
                self.broadcast(msg).await;
            }
        }
    }

    pub async fn handle_pickup(&self, player_id: &str, item_id: &str) {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Get player position
        let (player_x, player_y) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => (p.x as f32, p.y as f32),
                _ => return, // Player not found, inactive, or dead
            }
        };

        // Check if item exists and can be picked up
        let (item_info, protection_remaining) = {
            let items = self.ground_items.read().await;
            match items.get(item_id) {
                Some(item) => {
                    let dx = item.x - player_x;
                    let dy = item.y - player_y;
                    let distance = (dx * dx + dy * dy).sqrt();

                    if distance > 2.0 {
                        (None, None)
                    } else if !item.can_pickup(player_id, current_time) {
                        let elapsed = current_time.saturating_sub(item.drop_time);
                        let remaining_ms = 10000u64.saturating_sub(elapsed);
                        let remaining_secs = (remaining_ms + 999) / 1000;
                        (None, Some(remaining_secs))
                    } else {
                        (Some((item.item_id.clone(), item.quantity)), None)
                    }
                }
                None => (None, None),
            }
        };

        if let Some(secs) = protection_remaining {
            self.send_system_message(
                player_id,
                &format!(
                    "That item is protected for {} more second{}.",
                    secs,
                    if secs == 1 { "" } else { "s" }
                ),
            )
            .await;
            return;
        }

        if let Some((picked_item_id, quantity)) = item_info {
            // Check if player has inventory space before removing from ground
            let has_space = {
                let players = self.players.read().await;
                match players.get(player_id) {
                    Some(player) => player.inventory.has_space_for(
                        &picked_item_id,
                        quantity,
                        &self.item_registry,
                    ),
                    None => return,
                }
            };

            if !has_space {
                self.send_system_message(player_id, "Your inventory is full.")
                    .await;
                return;
            }

            // Remove item from ground
            let removed = {
                let mut items = self.ground_items.write().await;
                items.remove(item_id).is_some()
            };

            if removed {
                // Check if this was a persistent ground spawn
                {
                    let mut gsm = self.ground_spawn_manager.write().await;
                    gsm.mark_picked_up(item_id);
                }

                // Get display name from registry for logging
                let display_name = self
                    .item_registry
                    .get(&picked_item_id)
                    .map(|def| def.display_name.as_str())
                    .unwrap_or(&picked_item_id);
                tracing::debug!(
                    "Player {} picked up {} x{}",
                    player_id,
                    display_name,
                    quantity
                );

                // Add to player's inventory
                let (inventory_update, gold) = {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        player
                            .inventory
                            .add_item(&picked_item_id, quantity, &self.item_registry);
                        (player.inventory.to_update(), player.inventory.gold)
                    } else {
                        return;
                    }
                };

                // Process quest item collection
                self.process_quest_item_collect(player_id, &picked_item_id, quantity)
                    .await;

                // Broadcast pickup to players in same zone
                let pickup_msg = ServerMessage::ItemPickedUp {
                    item_id: item_id.to_string(),
                    player_id: player_id.to_string(),
                };
                self.broadcast_to_zone(player_id, pickup_msg).await;

                // SECURITY: Unicast inventory update (private - only this player receives)
                let inv_msg = ServerMessage::InventoryUpdate {
                    player_id: player_id.to_string(),
                    slots: inventory_update,
                    gold,
                };
                self.send_to_player(player_id, inv_msg).await;
            }
        }
    }

    /// Handle NPC interaction (quest givers, merchants, etc.)
    pub async fn handle_npc_interact(&self, player_id: &str, npc_id: &str) {
        // Get player position
        let (player_x, player_y) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => (p.x, p.y),
                _ => return,
            }
        };

        // Check if player is in an instance
        let instance_id = {
            let instances = self.player_instances.read().await;
            instances.get(player_id).cloned()
        };

        // Get NPC info - check instance NPCs first, then overworld NPCs
        let npc_info = if let Some(ref inst_id) = instance_id {
            // Player is in an instance - look up instance NPCs
            if let Some(instance) = self.instance_manager.find_player_instance(player_id).await {
                let npcs = instance.npcs.read().await;
                npcs.get(npc_id).map(|npc| {
                    let dx = (npc.x - player_x) as f32;
                    let dy = (npc.y - player_y) as f32;
                    let distance = (dx * dx + dy * dy).sqrt();
                    let entity_type = npc.prototype_id.clone();
                    (entity_type, distance, npc.is_alive())
                })
            } else {
                tracing::warn!(
                    "Player {} in instance {} but instance not found",
                    player_id,
                    inst_id
                );
                None
            }
        } else {
            // Player is in overworld - check room NPCs
            let npcs = self.npcs.read().await;
            npcs.get(npc_id).map(|npc| {
                let dx = (npc.x - player_x) as f32;
                let dy = (npc.y - player_y) as f32;
                let distance = (dx * dx + dy * dy).sqrt();
                let entity_type = npc.prototype_id.clone();
                (entity_type, distance, npc.is_alive())
            })
        };

        let (entity_type, distance, is_alive): (String, f32, bool) = match npc_info {
            Some(info) => info,
            None => {
                tracing::warn!(
                    "Player {} tried to interact with unknown NPC {}",
                    player_id,
                    npc_id
                );
                return;
            }
        };

        // Must be within interaction range (2 tiles) and NPC must be alive
        if distance > 2.5 || !is_alive {
            tracing::debug!(
                "Player {} can't interact with NPC {} (distance: {}, alive: {})",
                player_id,
                npc_id,
                distance,
                is_alive
            );
            return;
        }

        // Arena leaderboard interaction
        if entity_type == "arena_board" {
            if let Some(ref db) = self.db {
                match db.get_arena_leaderboard().await {
                    Ok(entries) => {
                        let mut text = String::from("=== Arena Leaderboard ===\n\n");
                        if entries.is_empty() {
                            text.push_str("No arena matches recorded yet.");
                        } else {
                            text.push_str("Rank | Name | Wins | Kills | Gold Won\n");
                            for (i, (name, kills, wins, gold)) in entries.iter().enumerate() {
                                text.push_str(&format!(
                                    "#{} | {} | {} | {} | {}\n",
                                    i + 1,
                                    name,
                                    wins,
                                    kills,
                                    gold
                                ));
                            }
                        }
                        self.send_to_player(
                            player_id,
                            ServerMessage::ShowDialogue {
                                quest_id: String::new(),
                                npc_id: npc_id.to_string(),
                                speaker: "Arena Leaderboard".to_string(),
                                text,
                                choices: vec![crate::protocol::DialogueChoice {
                                    id: "close".to_string(),
                                    text: "Close".to_string(),
                                }],
                            },
                        )
                        .await;
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch arena leaderboard: {}", e);
                        self.send_system_message(player_id, "Failed to load leaderboard.")
                            .await;
                    }
                }
            }
            return;
        }

        let is_altar = self
            .entity_registry
            .get(&entity_type)
            .map(|p| p.behaviors.altar)
            .unwrap_or(false);

        if is_altar {
            self.show_altar_dialogue(player_id, &npc_id, &entity_type)
                .await;
            return;
        }

        // Plot seller interaction - show plot purchase dialogue
        let is_plot_seller = self
            .entity_registry
            .get(&entity_type)
            .map(|p| p.behaviors.plot_seller)
            .unwrap_or(false);

        if is_plot_seller {
            self.show_master_farmer_dialogue(player_id, &npc_id).await;
            return;
        }

        // Banker interaction - open bank vault
        let is_banker = self
            .entity_registry
            .get(&entity_type)
            .map(|p| p.behaviors.banker)
            .unwrap_or(false);

        if is_banker {
            // Skip dialogue and open bank directly if fully upgraded
            let fully_upgraded = {
                let players = self.players.read().await;
                players
                    .get(player_id)
                    .map(|p| p.bank_max_slots >= item::BANK_MAX_SIZE as u32)
                    .unwrap_or(false)
            };
            if fully_upgraded {
                self.handle_bank_open(player_id).await;
            } else {
                self.show_banker_dialogue(player_id, &npc_id).await;
            }
            return;
        }

        // Slayer master interaction - open slayer panel
        let is_slayer_master = self
            .entity_registry
            .get(&entity_type)
            .map(|p| p.behaviors.slayer_master)
            .unwrap_or(false);

        if is_slayer_master {
            self.handle_slayer_master_interact(player_id, &entity_type)
                .await;
            return;
        }

        // KOTH rewards NPC - show pending rewards
        let is_koth_rewards = self
            .entity_registry
            .get(&entity_type)
            .map(|p| p.behaviors.koth_rewards)
            .unwrap_or(false);

        if is_koth_rewards {
            self.show_koth_rewards_dialogue(player_id, &npc_id).await;
            return;
        }

        // Port master interaction - show travel destinations
        let is_port_master = self
            .entity_registry
            .get(&entity_type)
            .map(|p| p.behaviors.port_master)
            .unwrap_or(false);

        if is_port_master {
            self.show_port_master_dialogue(player_id, &npc_id, &entity_type)
                .await;
            return;
        }

        if self
            .try_open_merchant_shop(player_id, &npc_id, &entity_type)
            .await
        {
            return;
        }

        self.handle_npc_quest_interaction(player_id, npc_id, &entity_type)
            .await;
    }

    /// Handle dialogue choice from player
    pub async fn handle_dialogue_choice(&self, player_id: &str, quest_id: &str, choice_id: &str) {
        // Non-quest dialogues (e.g. leaderboard) just close
        if quest_id.is_empty() {
            self.send_to_player(player_id, ServerMessage::DialogueClosed)
                .await;
            return;
        }

        if let Some(altar_id) = quest_id.strip_prefix("altar:") {
            self.handle_altar_dialogue_choice(player_id, altar_id, choice_id)
                .await;
            return;
        }

        // Handle plot seller dialogue choices (format: "plot_seller:{npc_id}")
        if let Some(npc_id) = quest_id.strip_prefix("plot_seller:") {
            if choice_id == "buy_plots" {
                // Show the plot purchase screen
                self.show_plot_purchase_dialogue(player_id, npc_id).await;
            } else if choice_id == "contracts" {
                // Show the farming contracts screen
                self.show_contract_dialogue(player_id, npc_id).await;
            } else if let Some(diff_str) = choice_id.strip_prefix("accept_") {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
                self.handle_accept_contract(player_id, diff_str).await;
            } else if choice_id == "claim_contract" {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
                self.handle_claim_contract(player_id).await;
            } else if choice_id == "abandon_contract" {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
                self.handle_abandon_contract(player_id).await;
            } else if let Some(plot_str) = choice_id.strip_prefix("unlock_") {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
                if let Ok(plot_id) = plot_str.parse::<u32>() {
                    self.handle_plot_purchase(player_id, plot_id).await;
                }
            } else if choice_id == "nevermind" {
                // Go back to main master farmer dialogue
                self.show_master_farmer_dialogue(player_id, npc_id).await;
            } else {
                // "close", "owned_N", "locked_N" just close
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
            }
            return;
        }

        // Handle banker dialogue choices (format: "banker:{npc_id}")
        if let Some(npc_id) = quest_id.strip_prefix("banker:") {
            if choice_id == "open_bank" {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
                self.handle_bank_open(player_id).await;
            } else if choice_id == "upgrade" {
                self.handle_bank_upgrade(player_id, npc_id).await;
            } else {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
            }
            return;
        }

        // Handle KOTH rewards dialogue choices (format: "koth_rewards:{npc_id}")
        if quest_id.starts_with("koth_rewards:") {
            if choice_id == "claim" {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
                self.claim_koth_rewards(player_id).await;
            } else {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
            }
            return;
        }

        // Handle waystone dialogue choices (format: "waystone:{waystone_id}")
        if let Some(waystone_id) = quest_id.strip_prefix("waystone:") {
            self.send_to_player(player_id, ServerMessage::DialogueClosed)
                .await;
            if choice_id == "teleport" {
                self.teleport_to_waystone(player_id, waystone_id).await;
            }
            return;
        }

        // Handle port master travel choices (format: "port:{npc_id}")
        if let Some(npc_id) = quest_id.strip_prefix("port:") {
            if let Some(dest_str) = choice_id.strip_prefix("port_dest_") {
                if let Ok(dest_index) = dest_str.parse::<usize>() {
                    self.handle_port_travel(player_id, npc_id, dest_index).await;
                }
            } else {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
            }
            return;
        }

        self.handle_quest_dialogue_choice(player_id, quest_id, choice_id)
            .await;
    }

    // ========================================================================
    // Port Master System
    // ========================================================================

    async fn show_port_master_dialogue(&self, player_id: &str, npc_id: &str, entity_type: &str) {
        let prototype = match self.entity_registry.get(entity_type) {
            Some(p) => p,
            None => return,
        };

        let port_config = match &prototype.port {
            Some(c) => c,
            None => return,
        };

        let player_gold = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) => p.inventory.gold,
                None => return,
            }
        };

        let speaker = prototype.display_name.clone();
        let greeting = prototype
            .dialogue
            .greeting
            .clone()
            .unwrap_or_else(|| "Where would you like to travel?".to_string());

        let mut choices: Vec<crate::protocol::DialogueChoice> = port_config
            .destinations
            .iter()
            .enumerate()
            .map(|(i, dest)| {
                let affordable = player_gold >= dest.cost;
                let label = if affordable {
                    format!("{} - {}g", dest.name, dest.cost)
                } else {
                    format!("{} - {}g (not enough gold)", dest.name, dest.cost)
                };
                crate::protocol::DialogueChoice {
                    id: format!("port_dest_{}", i),
                    text: label,
                }
            })
            .collect();

        choices.push(crate::protocol::DialogueChoice {
            id: "close".to_string(),
            text: "Nevermind".to_string(),
        });

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("port:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker,
                text: greeting,
                choices,
            },
        )
        .await;
    }

    async fn handle_port_travel(&self, player_id: &str, npc_id: &str, dest_index: usize) {
        // Close dialogue first
        self.send_to_player(player_id, ServerMessage::DialogueClosed)
            .await;

        // Look up the NPC's prototype to get the port config
        let (entity_type, npc_x, npc_y) = {
            let npcs = self.npcs.read().await;
            match npcs.get(npc_id) {
                Some(npc) => (npc.prototype_id.clone(), npc.x, npc.y),
                None => return,
            }
        };

        let prototype = match self.entity_registry.get(&entity_type) {
            Some(p) => p,
            None => return,
        };

        let port_config = match &prototype.port {
            Some(c) => c,
            None => return,
        };

        let destination = match port_config.destinations.get(dest_index) {
            Some(d) => d,
            None => return,
        };

        // Verify player is still near the NPC
        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        let dx = (player.x - npc_x) as f32;
        let dy = (player.y - npc_y) as f32;
        if (dx * dx + dy * dy).sqrt() > 5.0 {
            return;
        }

        // Check gold
        if player.inventory.gold < destination.cost {
            drop(players);
            self.send_system_message(player_id, "You don't have enough gold for that trip.")
                .await;
            return;
        }

        // Deduct gold and teleport
        player.inventory.gold -= destination.cost;
        player.x = destination.x;
        player.y = destination.y;
        // Reset movement state
        player.last_move_vel_x = 0;
        player.last_move_vel_y = 0;
        player.move_dx = 0;
        player.move_dy = 0;

        let new_gold = player.inventory.gold;
        let slots = player.inventory.to_update();
        drop(players);

        // Send inventory update with new gold
        self.send_to_player(
            player_id,
            ServerMessage::InventoryUpdate {
                player_id: player_id.to_string(),
                slots,
                gold: new_gold,
            },
        )
        .await;

        // Send system message
        self.send_system_message(
            player_id,
            &format!("You travel to {}. (-{}g)", destination.name, destination.cost),
        )
        .await;
    }

    // ========================================================================
    // Chair System
    // ========================================================================

    // ========================================================================
    // Auto-Action System
    // ========================================================================

    pub async fn tick(&self) -> TickTelemetry {
        let mut tick_telemetry = TickTelemetry {
            movement_stale_packets_ignored: self
                .movement_anomalies
                .stale_packets_ignored
                .swap(0, Ordering::Relaxed) as usize,
            movement_seq_gap_events: self
                .movement_anomalies
                .seq_gap_events
                .swap(0, Ordering::Relaxed) as usize,
            movement_input_gap_events: self
                .movement_anomalies
                .input_gap_events
                .swap(0, Ordering::Relaxed) as usize,
            ..TickTelemetry::default()
        };
        let tick_start = std::time::Instant::now();
        let mut chunk_unload_ms = 0u128;
        let mut restock_ms = 0u128;
        let delta_time = 1.0 / TICK_RATE;
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Update tick counter and get current tick for movement timing
        let current_tick = {
            let mut tick = self.tick.write().await;
            *tick += 1;
            *tick
        };

        // Periodically unload distant chunks to prevent unbounded memory/CPU growth
        if current_tick % 100 == 0 {
            let unload_start = std::time::Instant::now();

            // Use live player positions as the source of truth so stale chunk tracking
            // cannot unload chunks around actively moving players.
            let instanced_players: HashSet<String> = {
                let instances = self.player_instances.read().await;
                instances.keys().cloned().collect()
            };

            let active_coords: Vec<ChunkCoord> = {
                let players = self.players.read().await;
                players
                    .values()
                    .filter(|p| p.active && p.is_alive() && !instanced_players.contains(&p.id))
                    .map(|p| ChunkCoord::from_world(p.x, p.y))
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect()
            };

            if !active_coords.is_empty() {
                self.world.unload_distant_chunks(&active_coords, 5).await;
            }
            chunk_unload_ms = unload_start.elapsed().as_millis();
        }

        let pre_npc_start = std::time::Instant::now();

        self.handle_player_respawns(current_time).await;

        let movement_state = self
            .process_player_movement_tick(current_time, current_tick, &mut tick_telemetry)
            .await;
        let gathering_player_ids = movement_state.gathering_player_ids;
        let moved_players = movement_state.moved_players;
        let woodcutting_player_ids = movement_state.woodcutting_player_ids;
        let woodcutting_stopped = movement_state.woodcutting_stopped;

        let prayer_drain_ms = self.process_player_resource_ticks(current_tick).await;

        let player_updates = self
            .collect_player_updates(&gathering_player_ids, &woodcutting_player_ids)
            .await;

        self.handle_post_movement_effects(&moved_players, woodcutting_stopped)
            .await;

        let pre_npc_ms = pre_npc_start.elapsed().as_millis();
        let npc_world_start = std::time::Instant::now();

        let overworld_visibility = self.collect_overworld_visibility_snapshot().await;
        let player_positions = overworld_visibility.player_positions;
        let players_by_chunk = overworld_visibility.players_by_chunk;

        let overworld_npc_tick = self
            .process_overworld_npc_tick(
                current_time,
                delta_time,
                &player_positions,
                &players_by_chunk,
            )
            .await;
        let npc_updates = overworld_npc_tick.npc_updates;
        let respawned_npcs = overworld_npc_tick.respawned_npcs;
        let mut npc_attacks = overworld_npc_tick.npc_attacks;
        self.send_npc_speech_events(overworld_npc_tick.npc_speech_events)
            .await;

        let instance_npc_tick = self
            .process_instance_npc_tick(current_time, delta_time)
            .await;
        npc_attacks.extend(instance_npc_tick.npc_attacks);
        self.send_npc_speech_events(instance_npc_tick.speech_events)
            .await;

        // Process explosive minion contact explosions
        for (npc_id, instance_id, npc_x, npc_y) in instance_npc_tick.minion_explosions {
            self.check_boss_minion_death(&npc_id, &instance_id, npc_x, npc_y, current_time)
                .await;
        }

        // Process NPC attacks on players using hit/miss mechanics
        for (npc_id, target_id, npc_level, max_hit, npc_attack_bonus) in npc_attacks {
            // Players in gathering zones are immune to NPC damage
            {
                let gathering = self.gathering.read().await;
                if gathering.is_gathering(&target_id) {
                    continue;
                }
            }
            let (target_hp, target_x, target_y, died, damage): (i32, f32, f32, bool, i32) = {
                let mut players = self.players.write().await;
                if let Some(target) = players.get_mut(&target_id) {
                    if target.is_dead {
                        continue; // Already dead
                    }
                    // God mode prevents all damage
                    if target.is_god_mode {
                        continue;
                    }

                    // NPC uses its level as attack level
                    let npc_attack_level = npc_level;

                    // Player uses their defence skill level and equipment bonus
                    let player_defence_level = target.skills.defence.level;
                    let base_defence_bonus = target.defence_bonus(&self.item_registry);

                    // Apply prayer bonuses to player's defence
                    let active_ids: Vec<String> = target.active_prayers.iter().cloned().collect();
                    let prayer_effects = self.prayer_registry.calculate_effects(&active_ids);
                    let player_defence_bonus =
                        prayer_effects.apply_defence_bonus(base_defence_bonus);

                    // Roll hit/miss
                    if !calculate_hit(
                        npc_attack_level,
                        npc_attack_bonus,
                        player_defence_level,
                        player_defence_bonus,
                    ) {
                        // Miss - deal 0 damage
                        tracing::debug!(
                            "NPC {} misses {} (atk {} vs def {} + {})",
                            npc_id,
                            target_id,
                            npc_attack_level,
                            player_defence_level,
                            player_defence_bonus
                        );
                        (target.hp, target.x as f32, target.y as f32, false, 0)
                    } else {
                        // Hit - roll damage and apply with prayer damage reduction
                        let raw_damage = roll_damage(max_hit);
                        let damage = prayer_effects.apply_damage_reduction(raw_damage);
                        target.hp = (target.hp - damage).max(0);
                        let died = target.hp <= 0;
                        if died {
                            target.die(current_time);
                        }
                        tracing::debug!(
                            "NPC {} hits {} for {} damage (max: {}, raw: {}, HP: {})",
                            npc_id,
                            target_id,
                            damage,
                            max_hit,
                            raw_damage,
                            target.hp
                        );
                        (target.hp, target.x as f32, target.y as f32, died, damage)
                    }
                } else {
                    continue;
                }
            };

            // Broadcast damage event to players in the same zone
            self.broadcast_to_zone(
                &target_id,
                ServerMessage::DamageEvent {
                    source_id: npc_id.clone(),
                    target_id: target_id.clone(),
                    damage,
                    target_hp,
                    target_x,
                    target_y,
                    projectile: None,
                },
            )
            .await;

            // Interrupt crafting if player took damage
            if damage > 0 {
                self.cancel_crafting(&target_id, "interrupted").await;
            }

            // Note: We intentionally do NOT interrupt auto-action when hit by
            // a different NPC. The player chose their target and should stay
            // locked on it (matching OSRS behavior). Auto-action is only
            // cancelled by: player death, target death, explicit cancel, or movement.

            // Handle player death
            if died {
                tracing::info!("NPC {} killed player {}", npc_id, target_id);
                self.broadcast(ServerMessage::PlayerDied {
                    id: target_id.clone(),
                    killer_id: npc_id.clone(),
                })
                .await;

                self.clear_auto_action(&target_id, "player_died").await;

                // Check KOTH player death
                self.check_koth_player_death(&target_id, current_time).await;

                // Send prayer state update to dying player (prayers cleared on death)
                let (points, max_points) = {
                    let players = self.players.read().await;
                    if let Some(p) = players.get(&target_id) {
                        (p.prayer_points, p.max_prayer_points())
                    } else {
                        (0, 1)
                    }
                };
                self.send_to_player(
                    &target_id,
                    ServerMessage::PrayerStateUpdate {
                        points,
                        max_points,
                        active_prayers: vec![], // Cleared on death
                    },
                )
                .await;
            }
        }

        // Broadcast respawns
        for (id, x, y) in respawned_npcs {
            self.broadcast(ServerMessage::NpcRespawned { id, x, y })
                .await;
        }

        // ====================================================================
        // AUTO-ACTION PROCESSING
        // ====================================================================
        // Process auto-actions for all players (OSRS-style click-to-act).
        // This runs after NPC combat so interruption-by-damage has already been applied.
        {
            // Collect players with active auto-actions
            let auto_action_players: Vec<(String, AutoAction)> = {
                let players = self.players.read().await;
                players
                    .iter()
                    .filter_map(|(id, p)| {
                        if p.active && !p.is_dead {
                            p.auto_action.as_ref().map(|a| (id.clone(), a.clone()))
                        } else {
                            None
                        }
                    })
                    .collect()
            };

            for (pid, auto_action) in auto_action_players {
                match (&auto_action.target, &auto_action.action) {
                    (AutoActionTarget::Npc { npc_id }, AutoActionType::Attack) => {
                        let player_inst = self.player_instances.read().await.get(&pid).cloned();

                        // Validate NPC target is still alive (check instance or overworld)
                        let (npc_alive, npc_pos) = if let Some(ref inst_id) = player_inst {
                            if let Some(instance) =
                                self.instance_manager.get_by_instance_id(inst_id)
                            {
                                let npcs = instance.npcs.read().await;
                                npcs.get(npc_id)
                                    .map_or((false, None), |n| (n.is_alive(), Some((n.x, n.y, n.stats.size))))
                            } else {
                                (false, None)
                            }
                        } else {
                            let npcs = self.npcs.read().await;
                            npcs.get(npc_id)
                                .map_or((false, None), |n| (n.is_alive(), Some((n.x, n.y, n.stats.size))))
                        };

                        if !npc_alive {
                            self.clear_auto_action(&pid, "target_dead").await;
                            continue;
                        }

                        // Check if in range and cooldown ready
                        let (in_range, cooldown_ready) = if let Some((npc_x, npc_y, npc_size)) = npc_pos {
                            let players = self.players.read().await;
                            if let Some(player) = players.get(&pid) {
                                let closest_x = player.x.clamp(npc_x, npc_x + npc_size - 1);
                                let closest_y = player.y.clamp(npc_y, npc_y + npc_size - 1);
                                let dx = (player.x - closest_x).abs();
                                let dy = (player.y - closest_y).abs();
                                let (weapon_range, weapon_is_ranged) =
                                    if let Some(ref weapon_id) = player.equipped_weapon {
                                        if let Some(item_def) = self.item_registry.get(weapon_id) {
                                            item_def.equipment.as_ref().map_or((1, false), |e| {
                                                (e.range, e.weapon_type == WeaponType::Ranged)
                                            })
                                        } else {
                                            (1, false)
                                        }
                                    } else {
                                        (1, false)
                                    };
                                let in_range = if weapon_range == 1 {
                                    (dx + dy) == 1
                                } else {
                                    (dx + dy) <= weapon_range && (dx > 0 || dy > 0)
                                };
                                let cd = if weapon_is_ranged { RANGED_ATTACK_COOLDOWN_MS } else { ATTACK_COOLDOWN_MS };
                                let cooldown_ready =
                                    current_time - player.last_attack_time >= cd;
                                (in_range, cooldown_ready)
                            } else {
                                (false, false)
                            }
                        } else {
                            (false, false)
                        };

                        if in_range && cooldown_ready {
                            // Compute facing direction toward NPC target
                            let face_dir = if let Some((npc_x, npc_y, npc_size)) = npc_pos {
                                let players = self.players.read().await;
                                if let Some(player) = players.get(&pid) {
                                    let closest_x = player.x.clamp(npc_x, npc_x + npc_size - 1);
                                    let closest_y = player.y.clamp(npc_y, npc_y + npc_size - 1);
                                    let dx = closest_x - player.x;
                                    let dy = closest_y - player.y;
                                    Some(direction_from_delta(dx, dy))
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                            self.handle_attack(&pid, face_dir, Some(npc_id)).await;
                        }
                    }

                    (
                        AutoActionTarget::Player {
                            player_id: target_pid,
                        },
                        AutoActionType::Attack,
                    ) => {
                        // Validate player target is still alive and connected
                        let target_valid = {
                            let players = self.players.read().await;
                            players
                                .get(target_pid.as_str())
                                .map_or(false, |p| p.active && !p.is_dead)
                        };
                        if !target_valid {
                            self.clear_auto_action(&pid, "target_dead").await;
                            continue;
                        }

                        // Check same instance context
                        let same_context = {
                            let instances = self.player_instances.read().await;
                            instances.get(pid.as_str()) == instances.get(target_pid.as_str())
                        };
                        if !same_context {
                            self.clear_auto_action(&pid, "interrupted").await;
                            continue;
                        }

                        // Check range and cooldown
                        let (in_range, cooldown_ready) = {
                            let players = self.players.read().await;
                            if let (Some(attacker), Some(target)) =
                                (players.get(&pid), players.get(target_pid.as_str()))
                            {
                                let dx = (attacker.x - target.x).abs();
                                let dy = (attacker.y - target.y).abs();
                                let (weapon_range, weapon_is_ranged) =
                                    if let Some(ref weapon_id) = attacker.equipped_weapon {
                                        if let Some(item_def) = self.item_registry.get(weapon_id) {
                                            item_def.equipment.as_ref().map_or((1, false), |e| {
                                                (e.range, e.weapon_type == WeaponType::Ranged)
                                            })
                                        } else {
                                            (1, false)
                                        }
                                    } else {
                                        (1, false)
                                    };
                                // Manhattan distance for all ranges (diamond shape)
                                let in_range = if weapon_range == 1 {
                                    (dx + dy) == 1
                                } else {
                                    (dx + dy) <= weapon_range && (dx > 0 || dy > 0)
                                };
                                let cd = if weapon_is_ranged { RANGED_ATTACK_COOLDOWN_MS } else { ATTACK_COOLDOWN_MS };
                                let cooldown_ready =
                                    current_time - attacker.last_attack_time >= cd;
                                (in_range, cooldown_ready)
                            } else {
                                (false, false)
                            }
                        };

                        if in_range && cooldown_ready {
                            // Compute facing direction toward player target
                            let face_dir = {
                                let players = self.players.read().await;
                                match (players.get(&pid), players.get(target_pid.as_str())) {
                                    (Some(attacker), Some(target)) => {
                                        let dx = target.x - attacker.x;
                                        let dy = target.y - attacker.y;
                                        Some(direction_from_delta(dx, dy))
                                    }
                                    _ => None,
                                }
                            };
                            // Direction override is applied atomically inside handle_attack
                            self.handle_attack(&pid, face_dir, Some(target_pid)).await;
                        }
                    }

                    (AutoActionTarget::Resource { x, y, gid }, AutoActionType::Mine) => {
                        // Check if rock is depleted
                        let is_depleted = {
                            let mining = self.mining.read().await;
                            mining.is_rock_depleted(*x, *y)
                        };
                        if is_depleted {
                            self.clear_auto_action(&pid, "target_depleted").await;
                            continue;
                        }

                        // Check cardinal adjacency, cooldown, and inventory space
                        let (adjacent, cooldown_ready, inventory_full) = {
                            let mining = self.mining.read().await;
                            let ore_item_id =
                                mining.get_ore_type(*gid).map(|c| c.ore_item_id.clone());
                            let players = self.players.read().await;
                            if let Some(player) = players.get(&pid) {
                                let dx = (player.x - x).abs();
                                let dy = (player.y - y).abs();
                                let adjacent = (dx + dy) == 1;
                                let cooldown_ready =
                                    current_time - player.last_attack_time >= ATTACK_COOLDOWN_MS;
                                let inventory_full = if let Some(ref item_id) = ore_item_id {
                                    !player
                                        .inventory
                                        .has_space_for(item_id, 1, &self.item_registry)
                                } else {
                                    false
                                };
                                (adjacent, cooldown_ready, inventory_full)
                            } else {
                                (false, false, false)
                            }
                        };

                        if inventory_full {
                            self.clear_auto_action(&pid, "inventory_full").await;
                            continue;
                        }

                        if adjacent && cooldown_ready {
                            // Auto-face toward resource
                            {
                                let mut players = self.players.write().await;
                                if let Some(player) = players.get_mut(&pid) {
                                    let ddx = x - player.x;
                                    let ddy = y - player.y;
                                    player.direction = direction_from_delta(ddx, ddy);
                                }
                            }
                            self.handle_mine_rock(&pid, *x, *y, *gid).await;
                        }
                    }

                    (AutoActionTarget::Resource { x, y, gid }, AutoActionType::Chop) => {
                        // Check if tree is depleted
                        let is_depleted = {
                            let woodcutting = self.woodcutting.read().await;
                            woodcutting.is_tree_depleted(*x, *y)
                        };
                        if is_depleted {
                            self.clear_auto_action(&pid, "target_depleted").await;
                            continue;
                        }

                        // Check cardinal adjacency, cooldown, and inventory space
                        let (adjacent, cooldown_ready, inventory_full) = {
                            let woodcutting = self.woodcutting.read().await;
                            let log_item_id = woodcutting
                                .get_tree_type(*gid)
                                .map(|c| c.log_item_id.clone());
                            let players = self.players.read().await;
                            if let Some(player) = players.get(&pid) {
                                let dx = (player.x - x).abs();
                                let dy = (player.y - y).abs();
                                let adjacent = (dx + dy) == 1;
                                let cooldown_ready =
                                    current_time - player.last_attack_time >= ATTACK_COOLDOWN_MS;
                                let inventory_full = if let Some(ref item_id) = log_item_id {
                                    !player
                                        .inventory
                                        .has_space_for(item_id, 1, &self.item_registry)
                                } else {
                                    false
                                };
                                (adjacent, cooldown_ready, inventory_full)
                            } else {
                                (false, false, false)
                            }
                        };

                        if inventory_full {
                            self.clear_auto_action(&pid, "inventory_full").await;
                            continue;
                        }

                        if adjacent && cooldown_ready {
                            // Auto-face toward resource
                            {
                                let mut players = self.players.write().await;
                                if let Some(player) = players.get_mut(&pid) {
                                    let ddx = x - player.x;
                                    let ddy = y - player.y;
                                    player.direction = direction_from_delta(ddx, ddy);
                                }
                            }
                            self.handle_chop_tree(&pid, *x, *y, *gid).await;
                        }
                    }

                    // Invalid combinations (e.g. Attack on Resource) — just clear
                    _ => {
                        self.clear_auto_action(&pid, "interrupted").await;
                    }
                }
            }
        }

        // Check for expired items (60 second lifetime), skip persistent spawns
        let expired_items: Vec<String> = {
            let items = self.ground_items.read().await;
            items
                .iter()
                .filter(|(id, item)| {
                    !id.starts_with("persistent_") && item.is_expired(current_time)
                })
                .map(|(id, _)| id.clone())
                .collect()
        };

        // Remove and broadcast despawned items
        for item_id in expired_items {
            let mut items = self.ground_items.write().await;
            if items.remove(&item_id).is_some() {
                drop(items);
                self.broadcast(ServerMessage::ItemDespawned { item_id })
                    .await;
            }
        }

        // Respawn persistent ground items whose timers have elapsed
        {
            let respawns = {
                let mut gsm = self.ground_spawn_manager.write().await;
                gsm.check_respawns()
            };

            if !respawns.is_empty() {
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                for (spawn_id, item_id, x, y, quantity, instance_id) in respawns {
                    let ground_item_id = format!("persistent_{}", spawn_id);
                    let ground_item = crate::item::GroundItem::new_in_instance(
                        &ground_item_id,
                        &item_id,
                        x,
                        y,
                        quantity,
                        None,
                        current_time,
                        instance_id,
                    );
                    {
                        let mut items = self.ground_items.write().await;
                        items.insert(ground_item_id.clone(), ground_item);
                    }
                    {
                        let mut gsm = self.ground_spawn_manager.write().await;
                        gsm.set_active_ground_item(&spawn_id, ground_item_id);
                    }
                    tracing::debug!("Respawned persistent ground item: {}", spawn_id);
                }
            }
        }

        self.process_resource_ticks(current_time).await;

        let farming_growth_ms = self
            .process_farming_growth_updates(current_tick, current_time)
            .await;

        // Check for shop restocks (every 60 seconds)
        {
            let last_restock = *self.last_shop_restock.read().await;
            if last_restock.elapsed().as_secs() >= 60 {
                let restock_start = std::time::Instant::now();
                self.restock_shops().await;
                let mut last = self.last_shop_restock.write().await;
                *last = std::time::Instant::now();
                restock_ms = restock_start.elapsed().as_millis();
            }
        }

        // Tick chest spawn timers (every tick is fine — the check is cheap)
        self.process_chest_respawns().await;

        let npc_world_ms = npc_world_start.elapsed().as_millis();

        // Send state sync to each player, filtering by instance and view distance
        // Snapshot lock data quickly, then release locks before expensive encoding
        let state_sync_start = std::time::Instant::now();
        let tick = *self.tick.read().await;

        let (instance_snapshot, senders_snapshot) = {
            let player_instances = self.player_instances.read().await;
            let senders = self.player_senders.read().await;

            // Snapshot instance assignments: player_id -> instance_id
            let inst: HashMap<String, String> = player_instances.clone();
            // Snapshot senders (mpsc::Sender is cheap to clone)
            let send: HashMap<String, mpsc::Sender<Vec<u8>>> = senders.clone();

            (inst, send)
            // Both read locks released here
        };

        // Build quest lookup snapshots once per tick so StateSync avoids repeated async
        // registry lookups while evaluating per-player quest marker visibility.
        let all_quests = self.quest_registry.all_quests().await;
        let mut quest_by_id = HashMap::new();
        let mut npc_quest_ids: HashMap<String, Vec<String>> = HashMap::new();
        for quest in all_quests {
            if !quest.giver_npc.is_empty() {
                npc_quest_ids
                    .entry(quest.giver_npc.clone())
                    .or_default()
                    .push(quest.id.clone());
            }
            quest_by_id.insert(quest.id.clone(), quest);
        }

        // Per-player lookup: quest giver prototype IDs that should show turn-in check icons.
        // Includes:
        // 1) Quests already in ReadyToComplete
        // 2) Active quests where all non-giver objectives are complete and the only
        //    remaining step is "talk_to" the giver (return-to-giver prompt)
        let ready_turnin_npc_types_by_player: HashMap<String, HashSet<String>> = {
            let mut out: HashMap<String, HashSet<String>> = HashMap::new();

            let quest_states = self.player_quest_states.read().await;
            for (player_id, state) in quest_states.iter() {
                let mut givers: HashSet<String> = HashSet::new();

                for (quest_id, progress) in state.active_quests.iter() {
                    let Some(quest_def) = quest_by_id.get(quest_id) else {
                        continue;
                    };

                    if quest_def.giver_npc.is_empty() {
                        continue;
                    }

                    // Fast path: quest already ready to complete
                    if progress.status == crate::quest::QuestStatus::ReadyToComplete {
                        givers.insert(quest_def.giver_npc.clone());
                        continue;
                    }

                    if progress.status != crate::quest::QuestStatus::Active {
                        continue;
                    }

                    let giver_npc = quest_def.giver_npc.as_str();
                    let mut has_incomplete_return_to_giver = false;
                    let mut all_other_objectives_complete = true;

                    for objective in &quest_def.objectives {
                        let completed = progress
                            .objectives
                            .get(&objective.id)
                            .map(|o| o.completed)
                            .unwrap_or(false);

                        let is_return_to_giver = objective.objective_type == ObjectiveType::TalkTo
                            && objective.target == giver_npc;

                        if is_return_to_giver {
                            if !completed {
                                has_incomplete_return_to_giver = true;
                            }
                        } else if !completed {
                            all_other_objectives_complete = false;
                            break;
                        }
                    }

                    if has_incomplete_return_to_giver && all_other_objectives_complete {
                        givers.insert(quest_def.giver_npc.clone());
                    }
                }

                if !givers.is_empty() {
                    out.insert(player_id.clone(), givers);
                }
            }

            out
        };

        // For merchant+quest_giver NPCs, hide is_quest_giver when all quests are done.
        // Build NPC -> quest IDs, then check per-player completion.
        let all_npc_quests_done_by_player: HashMap<String, HashSet<String>> = {
            let mut out: HashMap<String, HashSet<String>> = HashMap::new();
            let quest_states = self.player_quest_states.read().await;
            for (player_id, state) in quest_states.iter() {
                let mut done_npcs: HashSet<String> = HashSet::new();
                for (npc_id, quest_ids) in &npc_quest_ids {
                    if !quest_ids.is_empty()
                        && quest_ids.iter().all(|qid| state.is_quest_completed(qid))
                    {
                        done_npcs.insert(npc_id.clone());
                    }
                }
                if !done_npcs.is_empty() {
                    out.insert(player_id.clone(), done_npcs);
                }
            }
            out
        };

        // Build position lookup for O(1) access during culling
        let player_pos_map: HashMap<&str, (i32, i32)> = player_updates
            .iter()
            .map(|p| (p.id.as_str(), (p.x, p.y)))
            .collect();

        // Separate overworld vs instance senders
        let mut instance_groups: HashMap<&str, Vec<(&String, &mpsc::Sender<Vec<u8>>)>> =
            HashMap::new();
        let mut overworld_senders: Vec<(&String, &mpsc::Sender<Vec<u8>>)> = Vec::new();
        for (player_id, sender) in senders_snapshot.iter() {
            match instance_snapshot.get(player_id) {
                Some(inst_id) => instance_groups
                    .entry(inst_id.as_str())
                    .or_default()
                    .push((player_id, sender)),
                None => overworld_senders.push((player_id, sender)),
            }
        }

        // Pre-filter player updates by instance
        let mut players_by_instance: HashMap<&str, Vec<&PlayerUpdate>> = HashMap::new();
        let mut overworld_players: Vec<&PlayerUpdate> = Vec::new();
        for p in &player_updates {
            match instance_snapshot.get(&p.id) {
                Some(inst_id) => players_by_instance
                    .entry(inst_id.as_str())
                    .or_default()
                    .push(p),
                None => overworld_players.push(p),
            }
        }

        // Instance groups: per-player encode because quest turn-in indicators are player-specific.
        for (inst_id, group_senders) in &instance_groups {
            let mut active_receivers: Vec<(&String, &mpsc::Sender<Vec<u8>>)> = Vec::new();
            let mut low_capacity_receivers: Vec<(&String, &mpsc::Sender<Vec<u8>>)> = Vec::new();
            for (pid, sender) in group_senders.iter().copied() {
                if sender.capacity() >= STATE_SYNC_MIN_QUEUE_CAPACITY {
                    active_receivers.push((pid, sender));
                } else {
                    low_capacity_receivers.push((pid, sender));
                }
            }
            tick_telemetry.state_sync_capacity_skips += low_capacity_receivers.len();

            let players_in_instance: Vec<&PlayerUpdate> = players_by_instance
                .get(inst_id)
                .cloned()
                .unwrap_or_default();
            let player_values: Vec<rmpv::Value> = players_in_instance
                .iter()
                .map(|p| crate::protocol::player_update_to_value(p))
                .collect();
            let player_map_in_instance: HashMap<&str, &PlayerUpdate> = players_in_instance
                .iter()
                .map(|p| (p.id.as_str(), *p))
                .collect();

            let instance_npcs: Vec<NpcUpdate> =
                if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                    instance.get_npc_updates().await
                } else {
                    Vec::new()
                };

            for (pid, sender) in active_receivers {
                let ready_turnin_npcs = ready_turnin_npc_types_by_player.get(pid.as_str());
                let done_npcs = all_npc_quests_done_by_player.get(pid.as_str());
                let npc_values: Vec<rmpv::Value> = instance_npcs
                    .iter()
                    .map(|n| {
                        let mut n_for_player = n.clone();
                        // Hide quest giver icon for merchant NPCs whose quests are all done
                        if n_for_player.is_quest_giver && n_for_player.is_merchant {
                            if done_npcs
                                .map(|set| set.contains(n_for_player.prototype_id.as_str()))
                                .unwrap_or(false)
                            {
                                n_for_player.is_quest_giver = false;
                            }
                        }
                        n_for_player.can_turn_in_quest = n_for_player.is_quest_giver
                            && ready_turnin_npcs
                                .map(|set| set.contains(n_for_player.prototype_id.as_str()))
                                .unwrap_or(false);
                        crate::protocol::npc_update_to_value(&n_for_player)
                    })
                    .collect();

                if let Ok(raw) = crate::protocol::encode_state_sync_from_values(
                    tick,
                    player_values.clone(),
                    npc_values,
                    inst_id,
                ) {
                    let raw_len = raw.len();
                    let bytes = crate::protocol::maybe_compress(raw);
                    let bytes_len = bytes.len();
                    tick_telemetry.state_sync_send_attempts += 1;
                    tick_telemetry.state_sync_full_sends += 1;
                    tick_telemetry.state_sync_raw_bytes += raw_len;
                    tick_telemetry.state_sync_bytes_sent += bytes_len;
                    if let Err(e) = sender.try_send(bytes) {
                        tick_telemetry.state_sync_try_send_drops += 1;
                        tracing::debug!("StateSync drop for {}: {}", pid, e);
                    }
                }
            }

            // If a client is under queue pressure, still try to send a tiny self-only delta
            // so local correction/facing remains responsive during transient congestion.
            for (pid, sender) in low_capacity_receivers {
                if sender.capacity() == 0 {
                    continue;
                }
                let Some(self_update) = player_map_in_instance.get(pid.as_str()) else {
                    continue;
                };
                let self_values = vec![crate::protocol::player_update_to_value(self_update)];
                if let Ok(raw) = crate::protocol::encode_delta_state_sync(
                    tick,
                    self_values,
                    Vec::new(),
                    inst_id,
                    false,
                    &[],
                    &[],
                ) {
                    let raw_len = raw.len();
                    let bytes = crate::protocol::maybe_compress(raw);
                    let bytes_len = bytes.len();
                    tick_telemetry.state_sync_send_attempts += 1;
                    tick_telemetry.state_sync_delta_sends += 1;
                    tick_telemetry.state_sync_fallback_self_only_sends += 1;
                    tick_telemetry.state_sync_raw_bytes += raw_len;
                    tick_telemetry.state_sync_bytes_sent += bytes_len;
                    if let Err(e) = sender.try_send(bytes) {
                        tick_telemetry.state_sync_try_send_drops += 1;
                        tracing::debug!("StateSync fallback drop for {}: {}", pid, e);
                    }
                }
            }
        }

        // Overworld: delta-compressed per-player StateSync
        // Build lookup maps for nearby entity filtering
        let overworld_player_map: HashMap<&str, &PlayerUpdate> = overworld_players
            .iter()
            .map(|p| (p.id.as_str(), *p))
            .collect();
        let npc_map: HashMap<&str, &NpcUpdate> =
            npc_updates.iter().map(|n| (n.id.as_str(), n)).collect();

        let current_tick = tick;
        let mut sync_states = self.sync_states.write().await;

        for (player_id, sender) in &overworld_senders {
            if sender.capacity() < STATE_SYNC_MIN_QUEUE_CAPACITY {
                tick_telemetry.state_sync_capacity_skips += 1;
                if sender.capacity() > 0 {
                    if let Some(self_update) = overworld_player_map.get(player_id.as_str()) {
                        let self_values =
                            vec![crate::protocol::player_update_to_value(self_update)];
                        if let Ok(raw) = crate::protocol::encode_delta_state_sync(
                            tick,
                            self_values,
                            Vec::new(),
                            "",
                            false,
                            &[],
                            &[],
                        ) {
                            let raw_len = raw.len();
                            let bytes = crate::protocol::maybe_compress(raw);
                            let bytes_len = bytes.len();
                            tick_telemetry.state_sync_send_attempts += 1;
                            tick_telemetry.state_sync_delta_sends += 1;
                            tick_telemetry.state_sync_fallback_self_only_sends += 1;
                            tick_telemetry.state_sync_raw_bytes += raw_len;
                            tick_telemetry.state_sync_bytes_sent += bytes_len;
                            if let Err(e) = sender.try_send(bytes) {
                                tick_telemetry.state_sync_try_send_drops += 1;
                                tracing::debug!("StateSync fallback drop for {}: {}", player_id, e);
                            }
                        }
                    }
                }
                continue;
            }

            let (px, py) = match player_pos_map.get(player_id.as_str()) {
                Some(pos) => *pos,
                None => continue,
            };

            // Filter nearby entities by view distance
            let nearby_players: HashMap<String, &PlayerUpdate> = overworld_player_map
                .iter()
                .filter(|(_, p)| (p.x - px).abs().max((p.y - py).abs()) <= VIEW_DISTANCE)
                .map(|(_, p)| (p.id.clone(), *p))
                .collect();
            let ready_turnin_npcs = ready_turnin_npc_types_by_player.get(player_id.as_str());
            let done_npcs = all_npc_quests_done_by_player.get(player_id.as_str());
            let nearby_npcs: HashMap<String, NpcUpdate> = npc_map
                .iter()
                .filter(|(_, n)| (n.x - px).abs().max((n.y - py).abs()) <= VIEW_DISTANCE)
                .map(|(_, n)| {
                    let mut n_for_player = (*n).clone();
                    // Hide quest giver icon for merchant NPCs whose quests are all done
                    if n_for_player.is_quest_giver && n_for_player.is_merchant {
                        if done_npcs
                            .map(|set| set.contains(n_for_player.prototype_id.as_str()))
                            .unwrap_or(false)
                        {
                            n_for_player.is_quest_giver = false;
                        }
                    }
                    n_for_player.can_turn_in_quest = n_for_player.is_quest_giver
                        && ready_turnin_npcs
                            .map(|set| set.contains(n_for_player.prototype_id.as_str()))
                            .unwrap_or(false);
                    (n_for_player.id.clone(), n_for_player)
                })
                .collect();

            let sync_state = sync_states
                .entry(player_id.to_string())
                .or_insert_with(PlayerSyncState::new);
            let needs_full = sync_state.last_full_sync_tick == 0
                || (current_tick - sync_state.last_full_sync_tick) >= FULL_SYNC_INTERVAL;

            if needs_full {
                // Full sync: encode all nearby entities
                let player_values: Vec<rmpv::Value> = nearby_players
                    .values()
                    .map(|p| crate::protocol::player_update_to_value(p))
                    .collect();
                let npc_values: Vec<rmpv::Value> = nearby_npcs
                    .values()
                    .map(|n| crate::protocol::npc_update_to_value(n))
                    .collect();

                if let Ok(raw) = crate::protocol::encode_state_sync_from_values(
                    tick,
                    player_values,
                    npc_values,
                    "",
                ) {
                    let raw_len = raw.len();
                    let bytes = crate::protocol::maybe_compress(raw);
                    let bytes_len = bytes.len();
                    tick_telemetry.state_sync_send_attempts += 1;
                    tick_telemetry.state_sync_full_sends += 1;
                    tick_telemetry.state_sync_raw_bytes += raw_len;
                    tick_telemetry.state_sync_bytes_sent += bytes_len;
                    if let Err(e) = sender.try_send(bytes) {
                        tick_telemetry.state_sync_try_send_drops += 1;
                        tracing::debug!("StateSync drop for {}: {}", player_id, e);
                    } else {
                        // Only update sync state if send succeeded
                        sync_state.last_full_sync_tick = current_tick;
                        sync_state.last_players = nearby_players
                            .into_iter()
                            .map(|(id, p)| (id, p.clone()))
                            .collect();
                        sync_state.last_npcs = nearby_npcs;
                    }
                }
            } else {
                // Delta sync: only encode changed/new entities + removal lists
                let mut changed_players: Vec<rmpv::Value> = Vec::new();
                for (id, update) in &nearby_players {
                    // Always include the receiving player's own update — the client
                    // needs continuous position confirmation to correct mispredictions
                    // (e.g. rejected moves hitting walls/NPCs).
                    if id == player_id.as_str() {
                        changed_players.push(crate::protocol::player_update_to_value(update));
                        continue;
                    }
                    match sync_state.last_players.get(id) {
                        Some(last) if last == *update => {} // unchanged, skip
                        _ => changed_players.push(crate::protocol::player_update_to_value(update)),
                    }
                }

                let mut changed_npcs: Vec<rmpv::Value> = Vec::new();
                for (id, update) in &nearby_npcs {
                    match sync_state.last_npcs.get(id) {
                        Some(last) if last == update => {} // unchanged, skip
                        _ => changed_npcs.push(crate::protocol::npc_update_to_value(update)),
                    }
                }

                // Find removed entities (were in last sync but not nearby now)
                let removed_players: Vec<String> = sync_state
                    .last_players
                    .keys()
                    .filter(|id| !nearby_players.contains_key(id.as_str()))
                    .cloned()
                    .collect();
                let removed_npcs: Vec<String> = sync_state
                    .last_npcs
                    .keys()
                    .filter(|id| !nearby_npcs.contains_key(id.as_str()))
                    .cloned()
                    .collect();

                if let Ok(raw) = crate::protocol::encode_delta_state_sync(
                    tick,
                    changed_players,
                    changed_npcs,
                    "",
                    false,
                    &removed_players,
                    &removed_npcs,
                ) {
                    let raw_len = raw.len();
                    let bytes = crate::protocol::maybe_compress(raw);
                    let bytes_len = bytes.len();
                    tick_telemetry.state_sync_send_attempts += 1;
                    tick_telemetry.state_sync_delta_sends += 1;
                    tick_telemetry.state_sync_raw_bytes += raw_len;
                    tick_telemetry.state_sync_bytes_sent += bytes_len;
                    if let Err(e) = sender.try_send(bytes) {
                        tick_telemetry.state_sync_try_send_drops += 1;
                        tracing::debug!("StateSync drop for {}: {}", player_id, e);
                    } else {
                        // Keep a rolling baseline on successful deltas to avoid resending
                        // the same changed entities on every tick.
                        sync_state.last_players = nearby_players
                            .iter()
                            .map(|(id, p)| (id.clone(), (*p).clone()))
                            .collect();
                        sync_state.last_npcs = nearby_npcs.clone();
                    }
                }
            }
        }
        drop(sync_states);

        // === Spectator StateSync ===
        // Generate a single StateSync for all spectators, centered on world spawn.
        // Spectators are read-only observers so we skip quest turn-in icons and delta
        // compression — one full encode shared across every spectator connection.
        let spectator_senders = self.spectator_senders.read().await;
        if !spectator_senders.is_empty() {
            // Gather players near spawn with VIEW_DISTANCE culling
            let mut spectator_player_values: Vec<rmpv::Value> = Vec::new();
            for p in &overworld_players {
                let dx = (p.x - WORLD_SPAWN_X).abs();
                let dy = (p.y - WORLD_SPAWN_Y).abs();
                if dx <= VIEW_DISTANCE && dy <= VIEW_DISTANCE {
                    spectator_player_values.push(crate::protocol::player_update_to_value(p));
                }
            }

            // Gather NPCs near spawn
            let mut spectator_npc_values: Vec<rmpv::Value> = Vec::new();
            for n in &npc_updates {
                let dx = (n.x - WORLD_SPAWN_X).abs();
                let dy = (n.y - WORLD_SPAWN_Y).abs();
                if dx <= VIEW_DISTANCE && dy <= VIEW_DISTANCE {
                    spectator_npc_values.push(crate::protocol::npc_update_to_value(n));
                }
            }

            // Encode once for all spectators (always full sync, no delta tracking)
            if let Ok(raw) = crate::protocol::encode_state_sync_from_values(
                current_tick,
                spectator_player_values,
                spectator_npc_values,
                "",
            ) {
                let bytes = crate::protocol::maybe_compress(raw);
                for sender in spectator_senders.values() {
                    let _ = sender.try_send(bytes.clone());
                }
            }
        }
        drop(spectator_senders);

        let state_sync_ms = state_sync_start.elapsed().as_millis();

        // Cancel trades if players moved too far apart
        {
            let trade_ids: Vec<String> = {
                let trades = self.trades.read().await;
                trades.keys().cloned().collect()
            };
            for trade_id in trade_ids {
                let should_cancel = {
                    let trades = self.trades.read().await;
                    if let Some(session) = trades.get(&trade_id) {
                        let players = self.players.read().await;
                        match (
                            players.get(&session.player_a),
                            players.get(&session.player_b),
                        ) {
                            (Some(a), Some(b)) => {
                                let dx = (a.x - b.x).abs();
                                let dy = (a.y - b.y).abs();
                                dx > TRADE_MAX_DISTANCE
                                    || dy > TRADE_MAX_DISTANCE
                                    || a.is_dead
                                    || b.is_dead
                                    || !a.active
                                    || !b.active
                            }
                            _ => true,
                        }
                    } else {
                        false
                    }
                };
                if should_cancel {
                    let session = {
                        let mut trades = self.trades.write().await;
                        trades.remove(&trade_id)
                    };
                    if let Some(session) = session {
                        {
                            let mut pt = self.player_trades.write().await;
                            pt.remove(&session.player_a);
                            pt.remove(&session.player_b);
                        }
                        let msg = ServerMessage::TradeCancelled {
                            reason: "Too far apart.".to_string(),
                        };
                        self.send_to_player(&session.player_a, msg.clone()).await;
                        self.send_to_player(&session.player_b, msg).await;
                    }
                }
            }
        }

        // Expire old trade requests (20 second timeout)
        {
            let mut requests = self.trade_requests.write().await;
            requests.retain(|_, (_, tick)| current_tick - *tick < 400);
        }

        // Close stall if player died
        {
            let stall_owners: Vec<String> = {
                let players = self.players.read().await;
                players
                    .values()
                    .filter(|p| p.stall.as_ref().map_or(false, |s| s.active) && p.is_dead)
                    .map(|p| p.id.clone())
                    .collect()
            };
            for pid in stall_owners {
                self.force_close_stall(&pid).await;
                self.send_to_player(
                    &pid,
                    ServerMessage::StallClosed {
                        reason: "Shop closed (you died).".to_string(),
                    },
                )
                .await;
            }
        }

        // KOTH tick: wave spawning + phase transitions
        self.process_koth_tick(current_time).await;

        // Boss fight tick
        self.process_boss_tick(current_time).await;

        // Arena tick: zone detection + state machine
        let arena_start = std::time::Instant::now();
        self.arena_tick(current_time).await;
        let arena_ms = arena_start.elapsed().as_millis();

        // Log slow ticks for debugging latency spikes
        let tick_duration = tick_start.elapsed();
        if tick_duration.as_millis() > 50 {
            tracing::warn!(
                "Slow tick {}: {}ms (pre_npc={}ms npc_world={}ms sync={}ms arena={}ms players={} npcs={} overworld_senders={} instance_groups={} chunk_unload={}ms prayer_drain={}ms farming_growth={}ms restock={}ms moves={}/{} reject_reasons(tile={} player={} npc={} chair={} arena={}) sync_attempts={} sync_capacity_skips={} sync_drops={} sync_full={} sync_delta={} sync_fallback={} sync_raw_bytes={} sync_wire_bytes={})",
                current_tick,
                tick_duration.as_millis(),
                pre_npc_ms,
                npc_world_ms,
                state_sync_ms,
                arena_ms,
                player_updates.len(),
                npc_updates.len(),
                overworld_senders.len(),
                instance_groups.len(),
                chunk_unload_ms,
                prayer_drain_ms,
                farming_growth_ms,
                restock_ms,
                tick_telemetry
                    .pending_moves
                    .saturating_sub(tick_telemetry.rejected_moves),
                tick_telemetry.pending_moves,
                tick_telemetry.rejected_tile_blocked,
                tick_telemetry.rejected_player_blocked,
                tick_telemetry.rejected_npc_blocked,
                tick_telemetry.rejected_chair_blocked,
                tick_telemetry.rejected_arena_blocked,
                tick_telemetry.state_sync_send_attempts,
                tick_telemetry.state_sync_capacity_skips,
                tick_telemetry.state_sync_try_send_drops,
                tick_telemetry.state_sync_full_sends,
                tick_telemetry.state_sync_delta_sends,
                tick_telemetry.state_sync_fallback_self_only_sends,
                tick_telemetry.state_sync_raw_bytes,
                tick_telemetry.state_sync_bytes_sent,
            );
        }

        tick_telemetry
    }

    /// Process arena zone detection and state machine events
    async fn arena_tick(&self, current_time: u64) {
        // Auto-queue/dequeue players based on position in queue zone
        let mut queue_errors: Vec<(String, String)> = Vec::new();
        {
            let players = self.players.read().await;
            let player_instances = self.player_instances.read().await;
            let mut arena = self.arena_manager.write().await;

            for (player_id, instance_id) in player_instances.iter() {
                if !instance_id.starts_with("pub_duel_arena") {
                    continue;
                }
                if let Some(player) = players.get(player_id) {
                    if player.is_dead || !player.active {
                        continue;
                    }

                    let in_queue_zone = arena.is_in_queue_zone(player.x, player.y);
                    let is_queued = arena.queued_players.contains(&player_id.to_string());

                    if in_queue_zone && !is_queued && arena.state == crate::arena::ArenaState::Idle
                    {
                        if !arena.queue_rejected.contains(player_id) {
                            if let Err(e) =
                                arena.queue_player(player_id, &player.name, player.inventory.gold)
                            {
                                arena.queue_rejected.insert(player_id.clone());
                                queue_errors.push((player_id.clone(), e));
                            }
                        }
                    } else if !in_queue_zone {
                        if is_queued && arena.state == crate::arena::ArenaState::Idle {
                            arena.dequeue_player(player_id);
                        }
                        arena.queue_rejected.remove(player_id);
                    }
                }
            }
        }
        for (pid, err) in queue_errors {
            self.send_system_message(&pid, &err).await;
        }

        // Process arena state machine
        let events = {
            let mut arena = self.arena_manager.write().await;
            arena.tick(current_time)
        };

        for event in events {
            match event {
                crate::arena::ArenaEvent::FightStarted { fighters } => {
                    let fighter_ids: Vec<String> =
                        fighters.iter().map(|(id, _)| id.clone()).collect();

                    // Teleport fighters to spawn points
                    {
                        let mut players = self.players.write().await;
                        for (player_id, (spawn_x, spawn_y)) in &fighters {
                            if let Some(player) = players.get_mut(player_id) {
                                player.x = *spawn_x;
                                player.y = *spawn_y;
                            }
                        }
                    }

                    // Broadcast match start to all arena players
                    self.broadcast_to_arena(ServerMessage::ArenaMatchStart { fighter_ids })
                        .await;
                }
                crate::arena::ArenaEvent::MatchEnded { placements } => {
                    // Distribute gold rewards
                    {
                        let mut players = self.players.write().await;
                        for placement in &placements {
                            if placement.gold_reward > 0 {
                                if let Some(player) = players.get_mut(&placement.player_id) {
                                    player.inventory.gold += placement.gold_reward;
                                }
                            }
                        }
                    }

                    // Save arena stats to DB
                    if let Some(ref db) = self.db {
                        // We need character IDs - for now we look them up via player name
                        // This is a best-effort save; failures are logged but don't block gameplay
                        for placement in &placements {
                            let won = placement.rank == 1;
                            let died = placement.rank > 1;
                            if let Err(e) = db
                                .update_arena_stats(
                                    0, // character_id will be resolved in the save path
                                    won,
                                    placement.kills,
                                    died,
                                    placement.gold_reward,
                                )
                                .await
                            {
                                tracing::warn!(
                                    "Failed to save arena stats for {}: {}",
                                    placement.player_id,
                                    e
                                );
                            }
                        }
                    }

                    let placement_data: Vec<crate::protocol::ArenaPlacementData> = placements
                        .iter()
                        .map(|p| crate::protocol::ArenaPlacementData {
                            rank: p.rank,
                            player_id: p.player_id.clone(),
                            player_name: p.player_name.clone(),
                            kills: p.kills,
                            gold_reward: p.gold_reward,
                        })
                        .collect();

                    self.broadcast_to_arena(ServerMessage::ArenaMatchEnd {
                        placements: placement_data,
                    })
                    .await;

                    // Send inventory updates to all fighters who earned gold
                    for placement in &placements {
                        if placement.gold_reward > 0 {
                            let update = {
                                let players = self.players.read().await;
                                players
                                    .get(&placement.player_id)
                                    .map(|p| (p.inventory.to_update(), p.inventory.gold))
                            };
                            if let Some((slots, gold)) = update {
                                self.send_to_player(
                                    &placement.player_id,
                                    ServerMessage::InventoryUpdate {
                                        player_id: placement.player_id.clone(),
                                        slots,
                                        gold,
                                    },
                                )
                                .await;
                            }
                        }
                    }
                }
                crate::arena::ArenaEvent::StateChanged { state } => {
                    let (queued_count, fighter_count, entry_fee, countdown_remaining) = {
                        let arena = self.arena_manager.read().await;
                        let remaining = match &arena.state {
                            crate::arena::ArenaState::Countdown { ends_at } => {
                                Some(ends_at.saturating_sub(current_time) as u32)
                            }
                            _ => None,
                        };
                        (
                            arena.queued_players.len() as u32,
                            arena.active_fighters.len() as u32,
                            arena.config.entry_fee,
                            remaining,
                        )
                    };

                    self.broadcast_to_arena(ServerMessage::ArenaStateUpdate {
                        state,
                        countdown_remaining,
                        queued_count,
                        fighter_count,
                        entry_fee,
                    })
                    .await;
                }
                crate::arena::ArenaEvent::ResultsExpired => {
                    // Reset broadcast
                    self.broadcast_to_arena(ServerMessage::ArenaStateUpdate {
                        state: "idle".to_string(),
                        countdown_remaining: None,
                        queued_count: 0,
                        fighter_count: 0,
                        entry_fee: {
                            let arena = self.arena_manager.read().await;
                            arena.config.entry_fee
                        },
                    })
                    .await;
                }
                _ => {}
            }
        }
    }

    /// Broadcast a message to all players in the fight_pit arena instance
    async fn broadcast_to_arena(&self, msg: ServerMessage) {
        let player_instances = self.player_instances.read().await;
        let senders = self.player_senders.read().await;

        if let Ok(bytes) = crate::protocol::encode_server_message(&msg) {
            for (player_id, instance_id) in player_instances.iter() {
                if instance_id.starts_with("pub_duel_arena") {
                    if let Some(sender) = senders.get(player_id) {
                        let _ = sender.try_send(bytes.clone());
                    }
                }
            }
        }
    }

    /// Handle chunk request from client
    pub async fn handle_chunk_request(&self, chunk_x: i32, chunk_y: i32) -> Option<ServerMessage> {
        use crate::chunk::WallEdge;
        use crate::protocol::{ChunkLayerData, ChunkObjectData, ChunkPortalData, ChunkWallData};

        let coord = ChunkCoord::new(chunk_x, chunk_y);
        if let Some(chunk) = self.world.get_chunk_data(coord).await {
            let layers: Vec<ChunkLayerData> = chunk
                .layers
                .iter()
                .map(|layer| ChunkLayerData {
                    layer_type: layer.layer_type as u8,
                    tiles: layer.tiles.clone(),
                })
                .collect();

            let collision = chunk.pack_collision();

            let objects: Vec<ChunkObjectData> = chunk
                .objects
                .iter()
                .map(|obj| ChunkObjectData {
                    gid: obj.gid,
                    tile_x: obj.tile_x,
                    tile_y: obj.tile_y,
                    width: obj.width,
                    height: obj.height,
                })
                .collect();

            let portals: Vec<ChunkPortalData> = chunk
                .portals
                .iter()
                .map(|p| ChunkPortalData {
                    id: p.id.clone(),
                    x: p.x,
                    y: p.y,
                    width: p.width,
                    height: p.height,
                    target_map: p.target_map.clone(),
                    target_spawn: p.target_spawn.clone(),
                })
                .collect();

            Some(ServerMessage::ChunkData {
                chunk_x,
                chunk_y,
                layers,
                collision,
                objects,
                walls: chunk
                    .walls
                    .iter()
                    .map(|w| ChunkWallData {
                        gid: w.gid,
                        tile_x: w.tile_x,
                        tile_y: w.tile_y,
                        edge: match w.edge {
                            WallEdge::Down => "down".to_string(),
                            WallEdge::Right => "right".to_string(),
                        },
                    })
                    .collect(),
                portals,
                heightmap: chunk.height_data.as_ref().map(|h| h.heights.clone()),
                block_types_down: chunk.height_data.as_ref().map(|h| h.block_types_down.clone()),
                block_types_right: chunk.height_data.as_ref().map(|h| h.block_types_right.clone()),
            })
        } else {
            Some(ServerMessage::ChunkNotFound { chunk_x, chunk_y })
        }
    }

    /// Get the World reference for chunk operations
    pub fn world(&self) -> &Arc<World> {
        &self.world
    }

    /// Update player's current chunk and return true if changed
    pub async fn update_player_chunk(&self, player_id: &str, new_chunk: ChunkCoord) -> bool {
        let mut chunks = self.player_chunks.write().await;
        let old_chunk = chunks.get(player_id).copied();
        if old_chunk != Some(new_chunk) {
            chunks.insert(player_id.to_string(), new_chunk);
            return true;
        }
        false
    }

    /// Generate entity definitions message for client sync
    pub fn get_entity_definitions(&self) -> ServerMessage {
        use crate::protocol::ClientEntityDef;

        let entities: Vec<ClientEntityDef> = self
            .entity_registry
            .all()
            .map(|proto| ClientEntityDef {
                id: proto.id.clone(),
                display_name: proto.display_name.clone(),
                sprite: proto.sprite.clone(),
                animation_type: format!("{:?}", proto.animation_type).to_lowercase(),
                max_hp: proto.stats.max_hp,
            })
            .collect();

        ServerMessage::EntityDefinitions { entities }
    }

    /// Handle casting a spell
    pub async fn handle_cast_spell(&self, player_id: &str, spell_id: &str) {
        // 1. Resolve spell: check static spells first, then scroll spell registry
        let resolved = if let Some(s) = crate::spell::get_spell(spell_id) {
            ResolvedSpell {
                id: s.id.to_string(),
                spell_type: s.spell_type,
                magic_level_req: Some(s.magic_level_req),
                mana_cost: s.mana_cost,
                cooldown_ms: s.cooldown_ms,
                base_power: s.base_power,
                effect_sprite: s.effect_sprite.to_string(),
                pushback_distance: 0,
                wall_slam_damage_per_tile: 0,
                is_scroll_spell: false,
            }
        } else if let Some(s) = self.scroll_spell_registry.get(spell_id) {
            ResolvedSpell {
                id: s.id.clone(),
                spell_type: s.spell_type,
                magic_level_req: None, // Scroll spells skip magic level checks
                mana_cost: s.mana_cost,
                cooldown_ms: s.cooldown_ms,
                base_power: s.base_power,
                effect_sprite: s.effect_sprite.clone(),
                pushback_distance: s.pushback_distance,
                wall_slam_damage_per_tile: s.wall_slam_damage_per_tile,
                is_scroll_spell: true,
            }
        } else {
            self.send_to_player(
                player_id,
                ServerMessage::SpellResult {
                    success: false,
                    reason: Some("Unknown spell".to_string()),
                },
            )
            .await;
            return;
        };

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // 2. Validate under read lock first
        {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) if !p.is_dead && p.active => p,
                _ => return,
            };

            // Scroll spells require unlock instead of magic level
            if resolved.is_scroll_spell {
                if !player.unlocked_spells.contains(&resolved.id) {
                    self.send_to_player(
                        player_id,
                        ServerMessage::SpellResult {
                            success: false,
                            reason: Some("You haven't learned this spell".to_string()),
                        },
                    )
                    .await;
                    return;
                }
            } else if let Some(req) = resolved.magic_level_req {
                // Check magic level for static spells
                if player.skills.magic.level < req {
                    self.send_to_player(
                        player_id,
                        ServerMessage::SpellResult {
                            success: false,
                            reason: Some("Magic level too low".to_string()),
                        },
                    )
                    .await;
                    return;
                }
            }
            // Check mana
            if player.mp < resolved.mana_cost {
                self.send_to_player(
                    player_id,
                    ServerMessage::SpellResult {
                        success: false,
                        reason: Some("Not enough mana".to_string()),
                    },
                )
                .await;
                return;
            }
            // Check cooldown
            if let Some(&last_cast) = player.spell_cooldowns.get(&resolved.id) {
                if current_time < last_cast + resolved.cooldown_ms {
                    self.send_to_player(
                        player_id,
                        ServerMessage::SpellResult {
                            success: false,
                            reason: Some("Spell on cooldown".to_string()),
                        },
                    )
                    .await;
                    return;
                }
            }
        }

        // 3. Dispatch based on spell type
        match resolved.spell_type {
            crate::spell::SpellType::Damage => {
                self.cast_damage_spell_resolved(player_id, &resolved, current_time)
                    .await
            }
            crate::spell::SpellType::Heal => {
                self.cast_heal_spell_resolved(player_id, &resolved, current_time)
                    .await
            }
            crate::spell::SpellType::Teleport => {
                // For static teleport spells (Return Home), delegate to existing handler
                if let Some(spell_def) = crate::spell::get_spell(spell_id) {
                    self.cast_return_home_spell(player_id, spell_def, current_time)
                        .await;
                }
            }
        }
    }

    /// Cast a damage spell using a ResolvedSpell (supports both static and scroll spells)
    async fn cast_damage_spell_resolved(
        &self,
        player_id: &str,
        spell_def: &ResolvedSpell,
        current_time: u64,
    ) {
        // 1. Get attacker info and target
        let (
            caster_name,
            caster_x,
            caster_y,
            target_id_opt,
            magic_level,
            attack_level,
            magic_bonus,
        ) = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) if !p.is_dead && p.active => p,
                _ => return,
            };
            (
                player.name.clone(),
                player.x,
                player.y,
                player.target_id.clone(),
                player.skills.magic.level,
                player.skills.attack.level,
                player.magic_bonus(&self.item_registry),
            )
        };

        // Effective attack level for spells: blend of attack and magic
        let effective_level = (attack_level + magic_level) / 2;

        // Must have a target
        let target_id = match target_id_opt {
            Some(id) => id,
            None => {
                self.send_to_player(
                    player_id,
                    ServerMessage::SpellResult {
                        success: false,
                        reason: Some("No target selected".to_string()),
                    },
                )
                .await;
                return;
            }
        };

        // Determine caster's instance context (None = overworld)
        let caster_instance = self.player_instances.read().await.get(player_id).cloned();

        // 2. Resolve target: check NPCs first, then players (same pattern as handle_attack)
        let mut is_npc = false;
        let mut is_instance_npc = false;
        let mut target_x: i32 = 0;
        let mut target_y: i32 = 0;
        let mut target_exists = false;

        // Check NPCs - instance NPCs if in instance, overworld NPCs if in overworld
        if let Some(ref inst_id) = caster_instance {
            if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                let npcs = instance.npcs.read().await;
                if let Some(npc) = npcs.get(&target_id) {
                    if npc.is_alive() && npc.is_attackable() {
                        is_npc = true;
                        is_instance_npc = true;
                        target_x = npc.x;
                        target_y = npc.y;
                        target_exists = true;
                    }
                }
            }
        } else {
            let npcs = self.npcs.read().await;
            if let Some(npc) = npcs.get(&target_id) {
                if npc.is_alive() && npc.is_attackable() {
                    is_npc = true;
                    target_x = npc.x;
                    target_y = npc.y;
                    target_exists = true;
                }
            }
        }

        // Check players if not an NPC (must be in same instance context)
        if !target_exists {
            let players = self.players.read().await;
            let instances = self.player_instances.read().await;
            let target_instance = instances.get(target_id.as_str()).cloned();
            if let Some(target) = players.get(&target_id) {
                if target.active
                    && target.hp > 0
                    && !target.is_dead
                    && target_instance == caster_instance
                {
                    is_npc = false;
                    target_x = target.x;
                    target_y = target.y;
                    target_exists = true;
                }
            }
        }

        if !target_exists {
            self.send_to_player(
                player_id,
                ServerMessage::SpellResult {
                    success: false,
                    reason: Some("Invalid target".to_string()),
                },
            )
            .await;
            return;
        }

        // 3. Check range (Chebyshev distance, 5 tiles for spells)
        let dx = (caster_x - target_x).abs();
        let dy = (caster_y - target_y).abs();
        let distance = dx.max(dy);
        if distance > 5 {
            self.send_to_player(
                player_id,
                ServerMessage::SpellResult {
                    success: false,
                    reason: Some("Target out of range".to_string()),
                },
            )
            .await;
            return;
        }

        // 4. Deduct mana and set cooldown
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.mp -= spell_def.mana_cost;
                player
                    .spell_cooldowns
                    .insert(spell_def.id.to_string(), current_time);
                // Stop movement when casting
                player.reject_pending_move();
            }
        }

        // 5. Broadcast casting animation
        let face_dir = direction_from_delta(target_x - caster_x, target_y - caster_y);
        self.broadcast_to_zone(
            player_id,
            ServerMessage::PlayerAttack {
                player_id: player_id.to_string(),
                attack_type: "spell".to_string(),
                direction: face_dir as u8,
            },
        )
        .await;

        // 6. Calculate hit/miss using blended combat+magic level
        let attack_bonus = magic_bonus; // Spells use magic bonus for accuracy

        // Helper closure-like macro for NPC spell damage (used for both overworld and instance NPCs)
        macro_rules! apply_spell_to_npc {
            ($npc:expr) => {{
                let npc_defence_level = $npc.level;
                let npc_defence_bonus = $npc.stats.defence_bonus;

                if !crate::skills::calculate_hit(
                    effective_level,
                    attack_bonus,
                    npc_defence_level,
                    npc_defence_bonus,
                ) {
                    // Miss
                    $npc.take_damage(0, current_time, Some(player_id));
                    let name = $npc.name();
                    tracing::info!(
                        "{} spell misses {} (eff {} [atk{}+mag{}] vs def {})",
                        caster_name,
                        name,
                        effective_level,
                        attack_level,
                        magic_level,
                        npc_defence_level
                    );
                    ($npc.hp, name, false, 0)
                } else {
                    // Hit
                    let max_hit =
                        crate::spell::calculate_spell_max_hit(magic_level, spell_def.base_power);
                    let damage = crate::spell::roll_spell_damage(max_hit);
                    let died = $npc.take_damage(damage, current_time, Some(player_id));
                    let name = $npc.name();
                    tracing::info!(
                        "{} spell hits {} for {} damage (max: {}, HP: {})",
                        caster_name,
                        name,
                        damage,
                        max_hit,
                        $npc.hp
                    );
                    ($npc.hp, name, died, damage)
                }
            }};
        }

        let (target_hp, target_name, target_died, actual_damage) = if is_npc {
            if is_instance_npc {
                let inst_id = caster_instance.as_ref().unwrap();
                if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                    let mut npcs = instance.npcs.write().await;
                    if let Some(npc) = npcs.get_mut(&target_id) {
                        apply_spell_to_npc!(npc)
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            } else {
                let mut npcs = self.npcs.write().await;
                if let Some(npc) = npcs.get_mut(&target_id) {
                    apply_spell_to_npc!(npc)
                } else {
                    return;
                }
            }
        } else {
            let mut players = self.players.write().await;
            if let Some(target) = players.get_mut(&target_id) {
                if target.is_dead {
                    return;
                }
                if target.is_god_mode {
                    return;
                }

                let target_defence_level = target.skills.defence.level;
                let base_defence_bonus = target.defence_bonus(&self.item_registry);

                // Apply prayer bonuses to target's defence
                let target_active_ids: Vec<String> =
                    target.active_prayers.iter().cloned().collect();
                let target_prayer_effects =
                    self.prayer_registry.calculate_effects(&target_active_ids);
                let target_defence_bonus =
                    target_prayer_effects.apply_defence_bonus(base_defence_bonus);

                if !crate::skills::calculate_hit(
                    effective_level,
                    attack_bonus,
                    target_defence_level,
                    target_defence_bonus,
                ) {
                    // Miss
                    let name = target.name.clone();
                    tracing::info!(
                        "{} spell misses {} (eff {} [atk{}+mag{}] vs def {} + {})",
                        caster_name,
                        name,
                        effective_level,
                        attack_level,
                        magic_level,
                        target_defence_level,
                        target_defence_bonus
                    );
                    (target.hp, name, false, 0)
                } else {
                    // Hit
                    let max_hit =
                        crate::spell::calculate_spell_max_hit(magic_level, spell_def.base_power);
                    let raw_damage = crate::spell::roll_spell_damage(max_hit);
                    let damage = target_prayer_effects.apply_damage_reduction(raw_damage);
                    target.hp = (target.hp - damage).max(0);
                    let name = target.name.clone();
                    let died = target.hp <= 0;
                    if died {
                        target.die(current_time);
                    }
                    tracing::info!(
                        "{} spell hits {} for {} damage (max: {}, raw: {}, HP: {})",
                        caster_name,
                        name,
                        damage,
                        max_hit,
                        raw_damage,
                        target.hp
                    );
                    (target.hp, name, died, damage)
                }
            } else {
                return;
            }
        };

        // 7. Broadcast SpellEffect to nearby players in the zone
        self.broadcast_to_zone(
            player_id,
            ServerMessage::SpellEffect {
                caster_id: player_id.to_string(),
                target_id: Some(target_id.clone()),
                spell_id: spell_def.id.to_string(),
                target_x,
                target_y,
            },
        )
        .await;

        // 8. Broadcast DamageEvent
        let damage_msg = ServerMessage::DamageEvent {
            source_id: player_id.to_string(),
            target_id: target_id.clone(),
            damage: actual_damage,
            target_hp,
            target_x: target_x as f32,
            target_y: target_y as f32,
            projectile: None,
        };
        self.broadcast_to_zone(player_id, damage_msg).await;

        // 8b. Apply pushback if the spell has it and the target was hit
        if spell_def.pushback_distance > 0 && actual_damage > 0 && !target_died {
            let dx = target_x - caster_x;
            let dy = target_y - caster_y;
            // Normalize direction (sign only)
            let dir_x = if dx != 0 { dx.signum() } else { 0 };
            let dir_y = if dy != 0 { dy.signum() } else { 0 };
            // If caster and target are on the same tile, push down as fallback
            let (dir_x, dir_y) = if dir_x == 0 && dir_y == 0 {
                (0, 1)
            } else {
                (dir_x, dir_y)
            };

            let mut final_x = target_x;
            let mut final_y = target_y;
            let mut blocked_tiles = 0;
            let mut wall_slam = false;

            for i in 1..=spell_def.pushback_distance {
                let next_x = target_x + dir_x * i;
                let next_y = target_y + dir_y * i;

                if !self.world.is_tile_walkable(next_x, next_y).await {
                    // Hit a wall - wall slam!
                    wall_slam = true;
                    blocked_tiles = spell_def.pushback_distance - (i - 1);
                    break;
                }
                final_x = next_x;
                final_y = next_y;
            }

            // Apply wall slam bonus damage
            let wall_slam_bonus = if wall_slam {
                blocked_tiles * spell_def.wall_slam_damage_per_tile
            } else {
                0
            };

            // Move the target to the final position
            if is_npc {
                if is_instance_npc {
                    let inst_id = caster_instance.as_ref().unwrap();
                    if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                        let mut npcs = instance.npcs.write().await;
                        if let Some(npc) = npcs.get_mut(&target_id) {
                            npc.x = final_x;
                            npc.y = final_y;
                            if wall_slam_bonus > 0 {
                                npc.take_damage(wall_slam_bonus, current_time, Some(player_id));
                            }
                        }
                    }
                } else {
                    let mut npcs = self.npcs.write().await;
                    if let Some(npc) = npcs.get_mut(&target_id) {
                        npc.x = final_x;
                        npc.y = final_y;
                        if wall_slam_bonus > 0 {
                            npc.take_damage(wall_slam_bonus, current_time, Some(player_id));
                        }
                    }
                }
            } else {
                let mut players = self.players.write().await;
                if let Some(target) = players.get_mut(&target_id) {
                    target.x = final_x;
                    target.y = final_y;
                    target.move_dx = 0;
                    target.move_dy = 0;
                    if wall_slam_bonus > 0 {
                        target.hp = (target.hp - wall_slam_bonus).max(0);
                    }
                }
            }

            // Send Pushback message
            self.broadcast_to_zone(
                player_id,
                ServerMessage::Pushback {
                    target_id: target_id.clone(),
                    from_x: target_x,
                    from_y: target_y,
                    to_x: final_x,
                    to_y: final_y,
                    wall_slam,
                    bonus_damage: wall_slam_bonus,
                },
            )
            .await;

            // Send DamageEvent for wall slam bonus
            if wall_slam_bonus > 0 {
                let slam_hp = if is_npc {
                    if is_instance_npc {
                        let inst_id = caster_instance.as_ref().unwrap();
                        if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                            let npcs = instance.npcs.read().await;
                            npcs.get(&target_id).map(|n| n.hp).unwrap_or(0)
                        } else {
                            0
                        }
                    } else {
                        let npcs = self.npcs.read().await;
                        npcs.get(&target_id).map(|n| n.hp).unwrap_or(0)
                    }
                } else {
                    let players = self.players.read().await;
                    players.get(&target_id).map(|p| p.hp).unwrap_or(0)
                };
                self.broadcast_to_zone(
                    player_id,
                    ServerMessage::DamageEvent {
                        source_id: player_id.to_string(),
                        target_id: target_id.clone(),
                        damage: wall_slam_bonus,
                        target_hp: slam_hp,
                        target_x: final_x as f32,
                        target_y: final_y as f32,
                        projectile: None,
                    },
                )
                .await;
            }
        }

        // 9. Award Magic XP and Hitpoints XP
        if actual_damage > 0 {
            let xp_results = {
                let mut players = self.players.write().await;
                if let Some(attacker) = players.get_mut(player_id) {
                    let magic_xp =
                        (actual_damage as f64 * crate::skills::MAGIC_XP_PER_DAMAGE) as i64;
                    let hp_xp =
                        (actual_damage as f64 * crate::skills::HITPOINTS_XP_PER_DAMAGE) as i64;

                    let mut results = Vec::new();

                    // Award Magic XP
                    let magic_leveled = attacker.skills.magic.add_xp(magic_xp);
                    results.push((
                        SkillType::Magic,
                        magic_xp,
                        attacker.skills.magic.xp,
                        attacker.skills.magic.level,
                        magic_leveled,
                    ));

                    // Award Hitpoints XP
                    let old_hp_level = attacker.skills.hitpoints.level;
                    let hp_leveled = attacker.skills.hitpoints.add_xp(hp_xp);
                    if hp_leveled {
                        let new_max = attacker.skills.hitpoints.level;
                        attacker.hp += new_max - old_hp_level;
                    }
                    results.push((
                        SkillType::Hitpoints,
                        hp_xp,
                        attacker.skills.hitpoints.xp,
                        attacker.skills.hitpoints.level,
                        hp_leveled,
                    ));

                    Some(results)
                } else {
                    None
                }
            };

            if let Some(results) = xp_results {
                let mut progression_needs_sync = false;
                for (skill_type, xp_gained, total_xp, level, leveled_up) in results {
                    self.send_to_player(
                        player_id,
                        ServerMessage::SkillXp {
                            player_id: player_id.to_string(),
                            skill: skill_type.as_str().to_string(),
                            xp_gained,
                            total_xp,
                            level,
                        },
                    )
                    .await;

                    if leveled_up {
                        tracing::info!(
                            "Player {} leveled up {} to {}",
                            player_id,
                            skill_type.as_str(),
                            level
                        );
                        self.broadcast_skill_level_up(player_id, skill_type.as_str(), level).await;
                        progression_needs_sync = true;
                    }
                }

                if progression_needs_sync {
                    self.process_quest_progression_snapshot(player_id).await;
                }
            }
        }

        // 10. Interrupt crafting if target is a player who took damage
        if !is_npc && actual_damage > 0 {
            self.cancel_crafting(&target_id, "interrupted").await;
        }

        // 11. Handle death
        if target_died {
            tracing::info!(
                "{} killed {} with spell {}",
                caster_name,
                target_name,
                spell_def.id
            );
            if is_npc {
                // Get NPC info for exp and loot
                let (prototype_id, npc_level) = if is_instance_npc {
                    let inst_id = caster_instance.as_ref().unwrap();
                    if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                        let npcs = instance.npcs.read().await;
                        npcs.get(&target_id)
                            .map(|n| (n.prototype_id.clone(), n.level))
                            .unwrap_or(("unknown".to_string(), 1))
                    } else {
                        ("unknown".to_string(), 1)
                    }
                } else {
                    let npcs = self.npcs.read().await;
                    npcs.get(&target_id)
                        .map(|n| (n.prototype_id.clone(), n.level))
                        .unwrap_or(("unknown".to_string(), 1))
                };

                // Broadcast NPC death
                self.broadcast(ServerMessage::NpcDied {
                    id: target_id.clone(),
                    killer_id: player_id.to_string(),
                })
                .await;

                // Persist monster kill count for stats leaderboards.
                self.record_monster_kill(player_id).await;

                // Process quest kill event
                self.process_quest_kill(player_id, &prototype_id).await;

                // Process slayer kill event
                self.process_slayer_kill(player_id, &prototype_id).await;

                // Spawn item drops from prototype loot table
                let drop_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                let killer_instance = {
                    let instances = self.player_instances.read().await;
                    instances.get(player_id).cloned()
                };

                let drops = if let Some(prototype) = self.entity_registry.get(&prototype_id) {
                    crate::entity::generate_loot_from_prototype(
                        prototype,
                        target_x as f32,
                        target_y as f32,
                        player_id,
                        drop_time,
                        npc_level,
                        killer_instance,
                    )
                } else {
                    vec![]
                };

                for item in drops {
                    let mut items = self.ground_items.write().await;

                    // For gold, try to combine with existing pile at same tile
                    if item.item_id == "gold" {
                        let tile_x = item.x.floor() as i32;
                        let tile_y = item.y.floor() as i32;

                        let existing_gold_id = items
                            .iter()
                            .find(|(_, existing)| {
                                existing.item_id == "gold"
                                    && existing.x.floor() as i32 == tile_x
                                    && existing.y.floor() as i32 == tile_y
                                    && existing.owner_id == item.owner_id
                            })
                            .map(|(id, _)| id.clone());

                        if let Some(existing_id) = existing_gold_id {
                            if let Some(existing) = items.get_mut(&existing_id) {
                                existing.quantity += item.quantity;
                                let update_msg = ServerMessage::ItemQuantityUpdated {
                                    id: existing_id.clone(),
                                    quantity: existing.quantity,
                                };
                                drop(items);
                                self.broadcast_to_zone(player_id, update_msg).await;
                            }
                            continue;
                        }
                    }

                    let drop_msg = ServerMessage::ItemDropped {
                        id: item.id.clone(),
                        item_id: item.item_id.clone(),
                        x: item.x,
                        y: item.y,
                        quantity: item.quantity,
                    };
                    items.insert(item.id.clone(), item);
                    drop(items);
                    self.broadcast_to_zone(player_id, drop_msg).await;
                }
            } else {
                // Player death from spell
                let arena_death = {
                    let arena = self.arena_manager.read().await;
                    arena.is_fighting() && arena.is_in_ring(&target_id)
                };

                if arena_death {
                    // Arena death handling
                    let (eliminated_name, killer_name, remaining) = {
                        let mut arena = self.arena_manager.write().await;
                        arena.on_player_death(&target_id, Some(player_id));
                        let eliminated_name = arena
                            .match_stats
                            .fighter_names
                            .get(&target_id)
                            .cloned()
                            .unwrap_or_default();
                        let killer_name = arena
                            .match_stats
                            .fighter_names
                            .get(player_id)
                            .cloned()
                            .unwrap_or_default();
                        let remaining = arena.active_fighters.len() as u32;
                        (eliminated_name, killer_name, remaining)
                    };

                    {
                        let spectator_spawn = {
                            let arena = self.arena_manager.read().await;
                            arena.active_spectator_spawn()
                        };
                        let mut players = self.players.write().await;
                        if let Some(p) = players.get_mut(&target_id) {
                            p.hp = p.skills.hitpoints.level;
                            p.is_dead = false;
                            p.x = spectator_spawn.0;
                            p.y = spectator_spawn.1;
                        }
                    }

                    self.broadcast_to_arena(ServerMessage::ArenaPlayerEliminated {
                        player_id: target_id.clone(),
                        player_name: eliminated_name,
                        killer_id: player_id.to_string(),
                        killer_name,
                        remaining,
                    })
                    .await;

                    let should_end = {
                        let arena = self.arena_manager.read().await;
                        arena.check_match_end()
                    };
                    if should_end {
                        let end_time = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64;
                        let placements = {
                            let mut arena = self.arena_manager.write().await;
                            arena.end_match(end_time)
                        };

                        {
                            let mut players = self.players.write().await;
                            for placement in &placements {
                                if placement.gold_reward > 0 {
                                    if let Some(p) = players.get_mut(&placement.player_id) {
                                        p.inventory.gold += placement.gold_reward;
                                    }
                                }
                            }
                        }

                        let placement_data: Vec<crate::protocol::ArenaPlacementData> = placements
                            .iter()
                            .map(|p| crate::protocol::ArenaPlacementData {
                                rank: p.rank,
                                player_id: p.player_id.clone(),
                                player_name: p.player_name.clone(),
                                kills: p.kills,
                                gold_reward: p.gold_reward,
                            })
                            .collect();

                        self.broadcast_to_arena(ServerMessage::ArenaMatchEnd {
                            placements: placement_data,
                        })
                        .await;

                        self.broadcast_to_arena(ServerMessage::ArenaStateUpdate {
                            state: "results".to_string(),
                            countdown_remaining: None,
                            queued_count: 0,
                            fighter_count: 0,
                            entry_fee: {
                                let arena = self.arena_manager.read().await;
                                arena.config.entry_fee
                            },
                        })
                        .await;

                        // Teleport all fighters to spectator spawn
                        {
                            let spectator_spawn = {
                                let arena = self.arena_manager.read().await;
                                arena.active_spectator_spawn()
                            };
                            let mut players = self.players.write().await;
                            for placement in &placements {
                                if let Some(p) = players.get_mut(&placement.player_id) {
                                    p.x = spectator_spawn.0;
                                    p.y = spectator_spawn.1;
                                    if p.is_dead {
                                        p.hp = p.skills.hitpoints.level;
                                        p.is_dead = false;
                                    }
                                }
                            }
                        }

                        // Send inventory updates for gold rewards
                        for placement in &placements {
                            if placement.gold_reward > 0 {
                                let update = {
                                    let players = self.players.read().await;
                                    players
                                        .get(&placement.player_id)
                                        .map(|p| (p.inventory.to_update(), p.inventory.gold))
                                };
                                if let Some((slots, gold)) = update {
                                    self.send_to_player(
                                        &placement.player_id,
                                        ServerMessage::InventoryUpdate {
                                            player_id: placement.player_id.clone(),
                                            slots,
                                            gold,
                                        },
                                    )
                                    .await;
                                }
                            }
                        }

                        // Save arena stats to DB
                        if let Some(ref db) = self.db {
                            for placement in &placements {
                                if let Some(char_id) = placement
                                    .player_id
                                    .strip_prefix("char_")
                                    .and_then(|s| s.parse::<i64>().ok())
                                {
                                    let won = placement.rank == 1;
                                    let died = placement.rank > 1;
                                    if let Err(e) = db
                                        .update_arena_stats(
                                            char_id,
                                            won,
                                            placement.kills,
                                            died,
                                            placement.gold_reward,
                                        )
                                        .await
                                    {
                                        tracing::warn!(
                                            "Failed to save arena stats for {}: {}",
                                            placement.player_id,
                                            e
                                        );
                                    }
                                }
                            }
                        }

                        if let Some(winner) = placements.iter().find(|p| p.rank == 1) {
                            self.send_system_message(
                                &winner.player_id,
                                &format!("You won the arena match! +{} gold", winner.gold_reward),
                            )
                            .await;
                        }
                    }
                } else {
                    // Normal player death
                    self.broadcast(ServerMessage::PlayerDied {
                        id: target_id.clone(),
                        killer_id: player_id.to_string(),
                    })
                    .await;

                    // Send prayer state update to dying player (prayers cleared on death)
                    let (points, max_points) = {
                        let players = self.players.read().await;
                        if let Some(p) = players.get(&target_id) {
                            (p.prayer_points, p.max_prayer_points())
                        } else {
                            (0, 1)
                        }
                    };
                    self.send_to_player(
                        &target_id,
                        ServerMessage::PrayerStateUpdate {
                            points,
                            max_points,
                            active_prayers: vec![],
                        },
                    )
                    .await;
                }
            }
        }
    }

    /// Cast a heal spell on self
    /// Cast a heal spell using a ResolvedSpell (supports both static and scroll spells)
    async fn cast_heal_spell_resolved(
        &self,
        player_id: &str,
        spell_def: &ResolvedSpell,
        current_time: u64,
    ) {
        // Get caster info
        let (caster_x, caster_y, caster_direction, magic_level, current_hp, max_hp) = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) if !p.is_dead && p.active => p,
                _ => return,
            };
            (
                player.x,
                player.y,
                player.direction,
                player.skills.magic.level,
                player.hp,
                player.max_hp(),
            )
        };

        // 1. Deduct mana and set cooldown
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.mp -= spell_def.mana_cost;
                player
                    .spell_cooldowns
                    .insert(spell_def.id.to_string(), current_time);
            }
        }

        // 2. Calculate heal amount
        let heal_amount = crate::spell::calculate_heal_amount(magic_level, spell_def.base_power);
        let actual_heal = heal_amount.min(max_hp - current_hp); // Clamp to not exceed max HP

        // 3. Apply heal
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.hp = (player.hp + heal_amount).min(player.max_hp());
            }
        }

        // 4. Broadcast SpellEffect (target_id = None, target position = caster position)
        self.broadcast_to_zone(
            player_id,
            ServerMessage::SpellEffect {
                caster_id: player_id.to_string(),
                target_id: None,
                spell_id: spell_def.id.to_string(),
                target_x: caster_x,
                target_y: caster_y,
            },
        )
        .await;

        // 5. Broadcast casting animation
        self.broadcast_to_zone(
            player_id,
            ServerMessage::PlayerAttack {
                player_id: player_id.to_string(),
                attack_type: "spell".to_string(),
                direction: caster_direction as u8,
            },
        )
        .await;

        // 6. Award Magic XP based on amount healed
        if actual_heal > 0 {
            let xp_results = {
                let mut players = self.players.write().await;
                if let Some(caster) = players.get_mut(player_id) {
                    let magic_xp = (actual_heal as f64 * crate::skills::MAGIC_XP_PER_HEAL) as i64;

                    let magic_leveled = caster.skills.magic.add_xp(magic_xp);
                    Some((
                        SkillType::Magic,
                        magic_xp,
                        caster.skills.magic.xp,
                        caster.skills.magic.level,
                        magic_leveled,
                    ))
                } else {
                    None
                }
            };

            if let Some((skill_type, xp_gained, total_xp, level, leveled_up)) = xp_results {
                self.send_to_player(
                    player_id,
                    ServerMessage::SkillXp {
                        player_id: player_id.to_string(),
                        skill: skill_type.as_str().to_string(),
                        xp_gained,
                        total_xp,
                        level,
                    },
                )
                .await;

                if leveled_up {
                    tracing::info!(
                        "Player {} leveled up {} to {}",
                        player_id,
                        skill_type.as_str(),
                        level
                    );
                    self.broadcast_skill_level_up(player_id, skill_type.as_str(), level).await;
                    self.process_quest_progression_snapshot(player_id).await;
                }
            }
        }

        tracing::info!(
            "Player {} healed for {} HP with spell {}",
            player_id,
            actual_heal,
            spell_def.id
        );
    }

    /// Cast the Return Home teleport spell. Returns true if the spell was successfully cast.
    pub async fn cast_return_home_spell(
        &self,
        player_id: &str,
        spell_def: &crate::spell::SpellDef,
        current_time: u64,
    ) -> bool {
        // Validate and set cooldown under write lock
        {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) if !p.is_dead && p.active => p,
                _ => return false,
            };

            // Check cooldown
            if let Some(&last_cast) = player.spell_cooldowns.get(spell_def.id) {
                if current_time < last_cast + spell_def.cooldown_ms {
                    drop(players);
                    self.send_to_player(
                        player_id,
                        ServerMessage::SpellResult {
                            success: false,
                            reason: Some("Spell on cooldown".to_string()),
                        },
                    )
                    .await;
                    return false;
                }
            }

            // Set cooldown and move player to spawn
            player
                .spell_cooldowns
                .insert(spell_def.id.to_string(), current_time);
            player.x = WORLD_SPAWN_X;
            player.y = WORLD_SPAWN_Y;
        }

        // Send success result
        self.send_to_player(
            player_id,
            ServerMessage::SpellResult {
                success: true,
                reason: None,
            },
        )
        .await;

        // Send spell effect to the player
        self.send_to_player(
            player_id,
            ServerMessage::SpellEffect {
                caster_id: player_id.to_string(),
                target_id: None,
                spell_id: spell_def.id.to_string(),
                target_x: WORLD_SPAWN_X,
                target_y: WORLD_SPAWN_Y,
            },
        )
        .await;

        tracing::info!(
            "Player {} cast Return Home, teleporting to ({}, {})",
            player_id,
            WORLD_SPAWN_X,
            WORLD_SPAWN_Y
        );
        true
    }
}
