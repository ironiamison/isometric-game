use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use uuid::Uuid;

use crate::chunk::ChunkCoord;
use crate::entity::{EntityPrototype, EntityRegistry};
use crate::data::ItemRegistry;
use crate::prayer::PrayerRegistry;
use crate::data::item_def::WeaponType;
use crate::skills::{Skills, SkillType, calculate_hit, calculate_max_hit, roll_damage};
use crate::item::{self, GroundItem, Inventory, GOLD_ITEM_ID};
use crate::npc::{Npc, NpcUpdate};
use crate::protocol::{ServerMessage, QuestObjectiveData};
use crate::quest::{QuestRegistry, QuestRunner, PlayerQuestState, QuestEvent};
use crate::shop::{ShopRegistry, ShopDefinition, ShopStockItem};
use crate::world::World;

// ============================================================================
// Constants
// ============================================================================

const TICK_RATE: f32 = 20.0;

// Grid-based movement: ticks between tile moves (5 ticks * 50ms = 250ms per tile)
const MOVE_COOLDOWN_TICKS: u64 = 5;

const MAP_WIDTH: u32 = 32;
const MAP_HEIGHT: u32 = 32;
const STARTING_HP: i32 = 100;

// Combat constants
const ATTACK_RANGE: i32 = 1; // Maximum distance to attack (in tiles)
const ATTACK_COOLDOWN_MS: u64 = 700; // Slightly shorter than client (800ms) to account for network latency
const PLAYER_HP_REGEN_PERCENT: f32 = 2.0;
const REGEN_INTERVAL_MS: u64 = 30000;

// Prayer drain interval (60 ticks = 3 seconds at 20 ticks/second)
const PRAYER_DRAIN_INTERVAL_TICKS: u64 = 60;

// View distance for StateSync culling (Chebyshev distance in tiles)
const VIEW_DISTANCE: i32 = 40;

// World spawn point (chunk 0,0) - where players respawn after death
const WORLD_SPAWN_X: i32 = 15;
const WORLD_SPAWN_Y: i32 = 4;

// ============================================================================
// NPC Speech Helper
// ============================================================================

/// Check NPC speech timer and collect recipients in a single pass.
/// Replaces the duplicated O(n²) pattern used for overworld/public/private instances.
fn check_npc_speech(
    npc: &mut Npc,
    nearby_players: &[(&str, i32, i32)],
    current_time: u64,
    speech_events: &mut Vec<(String, String, String)>,
) {
    let messages = match npc.speech_messages {
        Some(ref m) if !m.is_empty() && npc.is_alive() => m,
        _ => return,
    };

    // Single pass: collect players within speech radius
    let radius = npc.speech_radius;
    let npc_x = npc.x;
    let npc_y = npc.y;
    let recipients: Vec<&str> = nearby_players
        .iter()
        .filter(|(_, px, py)| {
            let dx = (npc_x - px).abs();
            let dy = (npc_y - py).abs();
            dx.max(dy) <= radius
        })
        .map(|(pid, _, _)| *pid)
        .collect();

    if recipients.is_empty() {
        // No players nearby — reset timer
        npc.next_speech_at = 0;
        return;
    }

    if npc.next_speech_at == 0 {
        // First time a player is nearby — set initial timer
        let delay = npc.speech_interval_min_ms
            + (rand::random::<u64>() % (npc.speech_interval_max_ms - npc.speech_interval_min_ms + 1));
        npc.next_speech_at = current_time + delay;
    } else if current_time >= npc.next_speech_at {
        // Time to speak!
        let idx = rand::random::<usize>() % messages.len();
        let message = &messages[idx];
        let npc_id = &npc.id;
        for pid in &recipients {
            speech_events.push((pid.to_string(), npc_id.clone(), message.clone()));
        }
        let delay = npc.speech_interval_min_ms
            + (rand::random::<u64>() % (npc.speech_interval_max_ms - npc.speech_interval_min_ms + 1));
        npc.next_speech_at = current_time + delay;
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
}

// ============================================================================
// Player Save Data (for database persistence)
// ============================================================================

#[derive(Debug, Clone)]
pub struct PlayerSaveData {
    pub x: f32,
    pub y: f32,
    pub hp: i32,
    pub prayer_points: i32,
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
    pub fn from_velocity(dx: f32, dy: f32) -> Self {
        if dx == 0.0 && dy == 0.0 {
            return Direction::Down;
        }

        let angle = dy.atan2(dx);
        let octant = ((angle + std::f32::consts::PI) / (std::f32::consts::PI / 4.0)).round() as i32 % 8;

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
// Player
// ============================================================================

#[derive(Debug, Clone)]
pub struct Player {
    pub id: String,
    pub name: String,
    // Grid position (integer tile coordinates)
    pub x: i32,
    pub y: i32,
    pub spawn_x: i32,
    pub spawn_y: i32,
    // Queued movement direction (-1, 0, or 1)
    pub move_dx: i32,
    pub move_dy: i32,
    pub last_move_tick: u64, // Tick-based movement cooldown
    pub direction: Direction,
    pub hp: i32,
    pub skills: Skills, // Combat skills (Hitpoints determines max HP)
    pub active: bool, // Whether WebSocket is connected
    pub target_id: Option<String>, // Currently targeted entity (player or NPC)
    pub last_attack_time: u64, // Timestamp of last attack (ms)
    pub is_dead: bool,
    pub death_time: u64, // When the player died (for respawn timer)
    pub inventory: Inventory,
    // Character appearance
    pub gender: String, // "male" or "female"
    pub skin: String,   // "tan", "pale", "brown", "purple", "orc", "ghost", "skeleton"
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
}

const PLAYER_RESPAWN_TIME_MS: u64 = 5000; // 5 seconds to respawn

impl Player {
    pub fn new(id: &str, name: &str, spawn_x: i32, spawn_y: i32, gender: &str, skin: &str, hair_style: Option<i32>, hair_color: Option<i32>) -> Self {
        let skills = Skills::new(); // HP 10, Attack/Strength/Defence 1
        Self {
            id: id.to_string(),
            name: name.to_string(),
            x: spawn_x,
            y: spawn_y,
            spawn_x,
            spawn_y,
            move_dx: 0,
            move_dy: 0,
            last_move_tick: 0,
            direction: Direction::Down,
            hp: skills.hitpoints.level, // HP = Hitpoints level
            prayer_points: 10 + skills.prayer.level, // Prayer points = 10 + Prayer level
            mp: 10 + skills.magic.level * 2, // Mana = 10 + magic_level * 2
            skills,
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
            crafting_state: None,
            active_prayers: HashSet::new(),
            spell_cooldowns: HashMap::new(),
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
        bonus
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
        bonus
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
        bonus
    }

    /// Award combat XP based on damage dealt.
    /// Combat skill gets 4 XP per damage.
    /// Hitpoints gets 1.33 XP per damage (1/3 of combat rate).
    /// Returns a vector of (SkillType, xp_gained, total_xp, level, leveled_up) for skills that gained XP.
    pub fn award_combat_xp(&mut self, damage: i32) -> Vec<(SkillType, i64, i64, i32, bool)> {
        use crate::skills::{COMBAT_XP_PER_DAMAGE, HITPOINTS_XP_PER_DAMAGE};

        let mut results = Vec::new();

        // Combat XP = 4 per damage (full amount to single Combat skill)
        let combat_xp = (damage as f64 * COMBAT_XP_PER_DAMAGE) as i64;
        // Hitpoints XP = 1.33 per damage
        let hp_xp = (damage as f64 * HITPOINTS_XP_PER_DAMAGE) as i64;

        // Award Combat XP
        let combat_leveled = self.skills.combat.add_xp(combat_xp);
        if combat_leveled {
            tracing::info!("{} leveled up Combat to {}!", self.name, self.skills.combat.level);
        }
        results.push((SkillType::Combat, combat_xp, self.skills.combat.xp, self.skills.combat.level, combat_leveled));

        // Award Hitpoints XP
        let old_hp_level = self.skills.hitpoints.level;
        let hp_leveled = self.skills.hitpoints.add_xp(hp_xp);
        if hp_leveled {
            // Hitpoints level up means max HP increased
            let new_max = self.skills.hitpoints.level;
            tracing::info!("{} leveled up Hitpoints to {}! (Max HP: {})", self.name, new_max, new_max);
            // Heal the difference (new levels worth of HP)
            self.hp += new_max - old_hp_level;
        }
        results.push((SkillType::Hitpoints, hp_xp, self.skills.hitpoints.xp, self.skills.hitpoints.level, hp_leveled));

        results
    }

    pub fn is_alive(&self) -> bool {
        !self.is_dead && self.hp > 0
    }

    pub fn die(&mut self, current_time: u64) {
        self.is_dead = true;
        self.death_time = current_time;
        self.hp = 0;
        self.move_dx = 0;
        self.move_dy = 0;
        self.target_id = None;
        // Deactivate all prayers on death
        self.active_prayers.clear();
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
        self.death_time = 0;
        self.target_id = None;
        self.last_regen_time = 0;
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
                let base_regen = ((max_hp as f32 * PLAYER_HP_REGEN_PERCENT) / 100.0).ceil().max(1.0);
                let regen = (base_regen * hp_regen_multiplier).ceil() as i32;
                self.hp = (self.hp + regen).min(max_hp);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerUpdate {
    pub id: String,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub direction: u8,
    // Velocity for client-side prediction (-1, 0, or 1)
    pub vel_x: i32,
    pub vel_y: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub combat_level: i32,
    // Individual skill levels
    pub hitpoints_level: i32,
    pub combat_skill_level: i32,
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
    pub mp: i32,
    pub max_mp: i32,
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
    /// Tracks which instance each player is currently in (None = overworld)
    player_instances: Arc<RwLock<HashMap<String, String>>>,
    /// Instance manager for looking up instance NPCs
    instance_manager: Arc<crate::instance::InstanceManager>,
    /// Arena duel manager (active when players are in duel_arena instance)
    arena_manager: RwLock<crate::arena::ArenaManager>,
    /// Database reference for arena stats persistence
    db: Option<Arc<crate::db::Database>>,
    /// Gathering system (fishing)
    gathering: RwLock<crate::gathering::GatheringSystem>,
    /// Woodcutting system
    woodcutting: RwLock<crate::woodcutting::WoodcuttingSystem>,
    /// Chair GID -> direction mapping (loaded from config)
    chair_gids: HashMap<u32, Direction>,
    /// Chair positions on the map: (tile_x, tile_y) -> ChairState
    chairs: RwLock<HashMap<(i32, i32), ChairState>>,
    /// Farming system (allotment patches, crop growth)
    farming: RwLock<crate::farming::FarmingSystem>,
    /// Cached portal tile positions (immutable after init, no lock needed)
    portal_tiles: std::collections::HashSet<(i32, i32)>,
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
    ) -> Self {
        let (tx, _) = broadcast::channel(256);
        let world = Arc::new(World::new("maps/world_0"));

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
                    let npc_id = spawn.unique_id.clone()
                        .unwrap_or_else(|| format!("npc_{}", npc_counter));
                    npc_counter += 1;

                    if let Some(prototype) = entity_registry.get(&spawn.entity_id) {
                        // Use spawn's level if specified, otherwise use prototype's level
                        let level = spawn.level.unwrap_or(prototype.stats.level);
                        tracing::info!(
                            "Spawning {} at ({}, {}) level {}",
                            spawn.entity_id, spawn.world_x, spawn.world_y, level
                        );
                        let npc = Npc::from_prototype(
                            &npc_id,
                            &spawn.entity_id,
                            prototype,
                            spawn.world_x,
                            spawn.world_y,
                            level,
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
        let mut gathering = match crate::gathering::GatheringSystem::load(std::path::Path::new("data")) {
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
            tracing::info!("Loaded {} gathering markers from chunk data", chunk_marker_count);
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
            tracing::info!("Cached {} portal tiles for NPC collision", portal_tiles.len());
        }

        // Load woodcutting system
        let woodcutting = match crate::woodcutting::WoodcuttingSystem::load(std::path::Path::new("data")) {
            Ok(w) => {
                tracing::info!("Loaded woodcutting system with {} tree types", w.tree_types.len());
                w
            }
            Err(e) => {
                tracing::warn!("Failed to load woodcutting system: {} (using empty)", e);
                crate::woodcutting::WoodcuttingSystem::new()
            }
        };

        // Load farming system
        let mut farming = match crate::farming::FarmingSystem::load(std::path::Path::new("data")) {
            Ok(f) => {
                tracing::info!("Loaded farming system with {} crops, {} patches", f.crops.len(), f.patches.len());
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

        // Load chair config
        let mut chair_gids: HashMap<u32, Direction> = HashMap::new();
        match std::fs::read_to_string("data/chairs.toml") {
            Ok(content) => {
                match toml::from_str::<ChairsConfig>(&content) {
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
                }
            }
            Err(e) => tracing::warn!("Failed to read chairs.toml: {} (no chairs)", e),
        }

        // Populate chair positions from chunk map objects
        let mut chairs: HashMap<(i32, i32), ChairState> = HashMap::new();
        for coord in &chunk_coords {
            if let Some(chunk) = world.get_or_load_chunk(*coord).await {
                for obj in &chunk.objects {
                    if let Some(&dir) = chair_gids.get(&obj.gid) {
                        chairs.insert((obj.tile_x, obj.tile_y), ChairState {
                            direction: dir,
                            occupied_by: None,
                        });
                    }
                }
            }
        }
        if !chairs.is_empty() {
            tracing::info!("Found {} chairs on the map", chairs.len());
        }

        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            players: RwLock::new(HashMap::new()),
            npcs: RwLock::new(npcs),
            ground_items: RwLock::new(HashMap::new()),
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
            player_instances,
            instance_manager,
            arena_manager: RwLock::new(crate::arena::ArenaManager::new(crate::arena::ArenaConfig::default())),
            db,
            gathering: RwLock::new(gathering),
            woodcutting: RwLock::new(woodcutting),
            chair_gids,
            chairs: RwLock::new(chairs),
            farming: RwLock::new(farming),
            portal_tiles,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        self.broadcast_tx.subscribe()
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
        tracing::debug!("Registered sender for player {}", player_id);
    }

    /// Unregister a player's message sender
    pub async fn unregister_player_sender(&self, player_id: &str) {
        let mut senders = self.player_senders.write().await;
        senders.remove(player_id);
        tracing::debug!("Unregistered sender for player {}", player_id);
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
                p.id, p.x, p.y, world_x, world_y,
                world_x + p.width, world_y + p.height, p.target_map
            );
        }

        chunk.portals.iter().find(|p| {
            let world_x = chunk_base_x + p.x;
            let world_y = chunk_base_y + p.y;
            let in_portal = player.x >= world_x && player.x < world_x + p.width &&
                           player.y >= world_y && player.y < world_y + p.height;
            if in_portal {
                debug!("Player {} is inside portal '{}'", player_id, p.id);
            }
            in_portal
        }).cloned()
    }

    /// Send a message to a specific player (unicast)
    /// SECURITY: Use this for private data like inventory updates
    pub async fn send_to_player(&self, player_id: &str, msg: ServerMessage) {
        use crate::protocol::encode_server_message;

        let senders = self.player_senders.read().await;
        if let Some(sender) = senders.get(player_id) {
            if let Ok(bytes) = encode_server_message(&msg) {
                if let Err(e) = sender.try_send(bytes) {
                    tracing::warn!("Failed to send unicast to {}: {}", player_id, e);
                }
            }
        } else {
            tracing::debug!("No sender registered for player {}", player_id);
        }
    }

    pub async fn reserve_player(&self, player_id: &str, name: &str, gender: &str, skin: &str, hair_style: Option<i32>, hair_color: Option<i32>) {
        let (spawn_x, spawn_y) = self.world.get_spawn_position().await;
        let mut players = self.players.write().await;
        let player = Player::new(player_id, name, spawn_x, spawn_y, gender, skin, hair_style, hair_color);
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
        hp: i32,
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
    ) {
        let mut player = Player::new(player_id, name, x, y, gender, skin, hair_style, hair_color);

        // Restore saved stats
        player.hp = hp.min(skills.hitpoints.level); // Cap HP at max (hitpoints level)
        player.skills = skills;
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

        // Restore inventory from JSON - support both old (u8) and new (String) formats
        // Skip invalid slots (empty item_id or quantity <= 0) to prevent ghost items
        if let Ok(slots) = serde_json::from_str::<Vec<(usize, String, i32)>>(inventory_json) {
            // New format: (slot_idx, item_id, quantity)
            for (slot_idx, item_id, quantity) in slots {
                if slot_idx < player.inventory.slots.len() && !item_id.is_empty() && quantity > 0 {
                    player.inventory.slots[slot_idx] = Some(item::InventorySlot::new(item_id, quantity));
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
                    }.to_string();
                    player.inventory.slots[slot_idx] = Some(item::InventorySlot::new(item_id, quantity));
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
            name, x, y, hp, player.combat_level(), gold, gender, skin
        );

        let mut players = self.players.write().await;
        players.insert(player_id.to_string(), player);
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

                    let placement_data: Vec<crate::protocol::ArenaPlacementData> = placements.iter().map(|p| {
                        crate::protocol::ArenaPlacementData {
                            rank: p.rank,
                            player_id: p.player_id.clone(),
                            player_name: p.player_name.clone(),
                            kills: p.kills,
                            gold_reward: p.gold_reward,
                        }
                    }).collect();

                    self.broadcast_to_arena(ServerMessage::ArenaMatchEnd {
                        placements: placement_data,
                    }).await;

                    // Broadcast elimination for the disconnected player
                    self.broadcast_to_arena(ServerMessage::ArenaPlayerEliminated {
                        player_id: disconnected_id.clone(),
                        player_name: "Disconnected".to_string(),
                        killer_id: "disconnect".to_string(),
                        killer_name: "Disconnect".to_string(),
                        remaining: 0,
                    }).await;
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
        players.remove(player_id);
    }

    /// Get player data for saving to database
    pub async fn get_player_save_data(&self, player_id: &str) -> Option<PlayerSaveData> {
        // Check if player is in an instance and get the map_id
        let current_map = if self.player_instances.read().await.contains_key(player_id) {
            self.instance_manager.find_player_instance(player_id).await
                .map(|inst| inst.map_id.clone())
        } else {
            None
        };

        let players = self.players.read().await;
        players.get(player_id).map(|p| {
            // Serialize inventory to JSON - new format with string item IDs
            // Filter out empty/invalid slots to prevent ghost items
            let inventory_slots: Vec<(usize, String, i32)> = p.inventory.slots
                .iter()
                .enumerate()
                .filter_map(|(idx, slot)| {
                    slot.as_ref()
                        .filter(|s| s.quantity > 0 && !s.item_id.is_empty())
                        .map(|s| (idx, s.item_id.clone(), s.quantity))
                })
                .collect();
            let inventory_json = serde_json::to_string(&inventory_slots).unwrap_or_else(|_| "[]".to_string());

            PlayerSaveData {
                x: p.x as f32,
                y: p.y as f32,
                hp: p.hp,
                prayer_points: p.prayer_points,
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
            }
        })
    }

    /// Batch-snapshot save data for multiple players in a single lock acquisition.
    /// Returns a map of player_id -> (PlayerSaveData, Option<PlayerQuestState>, HashSet<String>)
    pub async fn get_bulk_save_data(&self, player_ids: &[String]) -> HashMap<String, (PlayerSaveData, Option<PlayerQuestState>, HashSet<String>)> {
        let mut result = HashMap::new();

        // Snapshot instance assignments once
        let instance_map: HashMap<String, String> = {
            let instances = self.player_instances.read().await;
            player_ids.iter()
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

        // Single lock on players to snapshot all save data + discovered recipes
        {
            let players = self.players.read().await;
            for pid in player_ids {
                if let Some(p) = players.get(pid) {
                    let inventory_slots: Vec<(usize, String, i32)> = p.inventory.slots
                        .iter()
                        .enumerate()
                        .filter_map(|(idx, slot)| {
                            slot.as_ref()
                                .filter(|s| s.quantity > 0 && !s.item_id.is_empty())
                                .map(|s| (idx, s.item_id.clone(), s.quantity))
                        })
                        .collect();
                    let inventory_json = serde_json::to_string(&inventory_slots).unwrap_or_else(|_| "[]".to_string());

                    let save_data = PlayerSaveData {
                        x: p.x as f32,
                        y: p.y as f32,
                        hp: p.hp,
                        prayer_points: p.prayer_points,
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
                        current_map: map_ids.get(pid).cloned(),
                        sitting_at_x: p.sitting_at.map(|(x, _)| x),
                        sitting_at_y: p.sitting_at.map(|(_, y)| y),
                    };

                    let recipes = p.discovered_recipes.clone();
                    result.insert(pid.clone(), (save_data, None, recipes));
                }
            }
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
        players.get(player_id)
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
                let objectives: Vec<QuestObjectiveData> = quest.objectives
                    .iter()
                    .map(|o| {
                        // Get current progress from saved state
                        let (current, completed) = progress.objectives
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

    pub async fn get_player_appearance(&self, player_id: &str) -> Option<(String, String)> {
        let players = self.players.read().await;
        players.get(player_id).map(|p| (p.gender.clone(), p.skin.clone()))
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
    pub async fn get_ground_items_in_instance(&self, instance_id: Option<&str>) -> Vec<ServerMessage> {
        let items = self.ground_items.read().await;
        items.values()
            .filter(|item| {
                match (&item.instance_id, instance_id) {
                    (None, None) => true,  // Both overworld
                    (Some(a), Some(b)) => a == b,  // Same instance
                    _ => false,  // Different zones
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
        players.get(player_id).map(|p| ServerMessage::InventoryUpdate {
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
            combat_level: p.skills.combat.level,
            combat_xp: p.skills.combat.xp,
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
        })
    }

    pub async fn get_player_prayer_state(&self, player_id: &str) -> Option<ServerMessage> {
        let players = self.players.read().await;
        players.get(player_id).map(|p| ServerMessage::PrayerStateUpdate {
            points: p.prayer_points,
            max_points: p.max_prayer_points(),
            active_prayers: p.active_prayers.iter().cloned().collect(),
        })
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
        );

        let mut npcs = self.npcs.write().await;
        npcs.insert(npc_id.clone(), npc);
        tracing::info!("Admin spawned NPC {} at ({}, {})", prototype_id, x, y);
        Some(npc_id)
    }

    pub async fn handle_move(&self, player_id: &str, dx: f32, dy: f32) {
        let mut chair_to_free: Option<(i32, i32)> = None;
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                // Auto-stand when trying to move while sitting (only in chair facing direction)
                if let Some(pos) = player.sitting_at {
                    // Determine intended movement direction
                    let move_dir = if dx.abs() > dy.abs() {
                        if dx > 0.1 { Some(Direction::Right) } else if dx < -0.1 { Some(Direction::Left) } else { None }
                    } else if dy.abs() > 0.1 {
                        if dy > 0.1 { Some(Direction::Down) } else { Some(Direction::Up) }
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
                    player.move_dx = 0;
                    player.move_dy = 0;
                } else {
                    // Convert to grid movement (-1, 0, or 1)
                    // No diagonal movement in grid-based system
                    let move_dx: i32;
                    let move_dy: i32;

                    if dx.abs() > dy.abs() {
                        // Horizontal priority
                        move_dx = if dx > 0.1 { 1 } else if dx < -0.1 { -1 } else { 0 };
                        move_dy = 0;
                    } else if dy.abs() > 0.1 {
                        // Vertical priority
                        move_dx = 0;
                        move_dy = if dy > 0.1 { 1 } else if dy < -0.1 { -1 } else { 0 };
                    } else {
                        move_dx = 0;
                        move_dy = 0;
                    }

                    player.move_dx = move_dx;
                    player.move_dy = move_dy;

                    if move_dx != 0 || move_dy != 0 {
                        player.direction = Direction::from_velocity(move_dx as f32, move_dy as f32);
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
    }

    /// Handle face command - change direction without moving
    pub async fn handle_face(&self, player_id: &str, direction: u8) {
        tracing::info!("[SERVER] handle_face called: player_id={}, direction={}", player_id, direction);
        let should_stop_gathering = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                // Don't allow direction changes while sitting
                if player.sitting_at.is_some() {
                    return;
                }
                let old_dir = player.direction;
                player.direction = Direction::from_u8(direction);
                tracing::info!("[SERVER] Updated player direction: {:?} -> {:?}", old_dir, player.direction);
                // Ensure player is not moving when just facing
                player.move_dx = 0;
                player.move_dy = 0;

                // Check if player is gathering and no longer facing a marker
                let gathering = self.gathering.read().await;
                if gathering.is_gathering(player_id) {
                    let (fdx, fdy): (i32, i32) = match player.direction {
                        Direction::Down => (0, 1),
                        Direction::Up => (0, -1),
                        Direction::Left => (-1, 0),
                        Direction::Right => (1, 0),
                        Direction::DownLeft => (-1, 1),
                        Direction::DownRight => (1, 1),
                        Direction::UpLeft => (-1, -1),
                        Direction::UpRight => (1, -1),
                    };
                    let face_x = player.x + fdx;
                    let face_y = player.y + fdy;
                    let facing_marker = gathering.markers.iter().any(|m| m.x == face_x && m.y == face_y);
                    !facing_marker
                } else {
                    false
                }
            } else {
                tracing::warn!("[SERVER] handle_face: player not found: {}", player_id);
                return;
            }
        };

        if should_stop_gathering {
            self.handle_stop_gathering(player_id).await;
        }
    }

    pub async fn handle_chat(&self, player_id: &str, text: &str) {
        let sanitized = text.trim().chars().take(200).collect::<String>();
        if sanitized.is_empty() {
            return;
        }

        // Check for commands (messages starting with /)
        if sanitized.starts_with('/') {
            self.handle_chat_command(player_id, &sanitized).await;
            return;
        }

        // Regular chat message
        let players = self.players.read().await;
        if let Some(player) = players.get(player_id) {
            let msg = ServerMessage::ChatMessage {
                sender_id: player_id.to_string(),
                sender_name: player.name.clone(),
                text: sanitized,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            };
            drop(players); // Release lock before broadcast
            self.broadcast(msg).await;
        }
    }

    /// Handle chat commands (messages starting with /)
    async fn handle_chat_command(&self, player_id: &str, text: &str) {
        let parts: Vec<&str> = text.split_whitespace().collect();
        let command = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();

        // Check if player is admin for privileged commands
        let is_admin = {
            let players = self.players.read().await;
            players.get(player_id).map(|p| p.is_admin).unwrap_or(false)
        };

        // Admin-only commands check
        let admin_commands = ["/give", "/setlevel", "/teleport", "/spawn", "/heal", "/kill", "/god", "/announce", "/arena"];
        if admin_commands.contains(&command.as_str()) && !is_admin {
            self.send_system_message(player_id, "This command requires admin privileges.").await;
            return;
        }

        match command.as_str() {
            "/give" => {
                // /give item_id [quantity]
                if parts.len() < 2 {
                    self.send_system_message(player_id, "Usage: /give <item_id> [quantity]").await;
                    return;
                }

                let item_id = parts[1];
                let quantity = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);

                // Validate item exists
                if self.item_registry.get(item_id).is_none() {
                    self.send_system_message(player_id, &format!("Unknown item: {}", item_id)).await;
                    return;
                }

                // Add item to player's inventory
                // add_item returns the quantity that DIDN'T fit (0 = all added successfully)
                let (leftover, inventory_update, gold) = {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        let leftover = player.inventory.add_item(item_id, quantity, &self.item_registry);
                        (leftover, player.inventory.to_update(), player.inventory.gold)
                    } else {
                        (quantity, vec![], 0)
                    }
                };

                let added = quantity - leftover;
                if added > 0 {
                    tracing::info!("Player {} spawned {}x {}", player_id, added, item_id);
                    self.send_system_message(player_id, &format!("Gave {}x {}", added, item_id)).await;

                    // Send inventory update
                    self.send_to_player(player_id, ServerMessage::InventoryUpdate {
                        player_id: player_id.to_string(),
                        slots: inventory_update,
                        gold,
                    }).await;
                } else {
                    self.send_system_message(player_id, "Inventory full").await;
                }
            }
            "/setlevel" => {
                // /setlevel <skill> <level> - Sets a specific skill to the given level
                // /setlevel <level> - Sets all skills to the given level
                use crate::skills::{Skill, SkillType};

                if parts.len() < 2 {
                    self.send_system_message(player_id, "Usage: /setlevel <skill> <level> or /setlevel <level>").await;
                    return;
                }

                let (skill_type, level) = if parts.len() >= 3 {
                    // /setlevel <skill> <level>
                    let skill_type = match SkillType::from_str(parts[1]) {
                        Some(st) => st,
                        None => {
                            let valid: Vec<&str> = SkillType::all().iter().map(|s| s.as_str()).collect();
                            self.send_system_message(player_id, &format!("Unknown skill '{}'. Valid skills: {}", parts[1], valid.join(", "))).await;
                            return;
                        }
                    };
                    let level: i32 = match parts[2].parse() {
                        Ok(l) if l >= 1 && l <= 99 => l,
                        _ => {
                            self.send_system_message(player_id, "Level must be between 1 and 99").await;
                            return;
                        }
                    };
                    (Some(skill_type), level)
                } else {
                    // /setlevel <level>
                    let level: i32 = match parts[1].parse() {
                        Ok(l) if l >= 1 && l <= 99 => l,
                        _ => {
                            // Maybe they typed a skill name without a level
                            if SkillType::from_str(parts[1]).is_some() {
                                self.send_system_message(player_id, "Usage: /setlevel <skill> <level>").await;
                            } else {
                                self.send_system_message(player_id, "Level must be between 1 and 99").await;
                            }
                            return;
                        }
                    };
                    (None, level)
                };

                {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        if let Some(st) = skill_type {
                            *player.skills.get_mut(st) = Skill::new(level);
                            if st == SkillType::Hitpoints {
                                player.hp = player.max_hp();
                            }
                            tracing::info!("Player {} set {} to level {}", player_id, st.as_str(), level);
                        } else {
                            for &st in SkillType::all() {
                                *player.skills.get_mut(st) = Skill::new(level);
                            }
                            player.hp = player.max_hp();
                            tracing::info!("Player {} set all skills to level {}", player_id, level);
                        }
                    } else {
                        return;
                    }
                };

                if let Some(st) = skill_type {
                    self.send_system_message(player_id, &format!("{} set to level {}", st.as_str(), level)).await;
                } else {
                    let combat_level = {
                        let players = self.players.read().await;
                        players.get(player_id).map(|p| p.skills.combat_level()).unwrap_or(0)
                    };
                    self.send_system_message(player_id, &format!("All skills set to level {} (Combat Level: {})", level, combat_level)).await;
                }
            }
            "/help" => {
                if is_admin {
                    self.send_system_message(player_id, "Commands: /give <item> [qty], /setlevel [skill] <lvl>, /teleport <x> <y>, /spawn <npc> [x] [y], /heal [player], /kill <player>, /god, /announce <msg>, /items, /help").await;
                } else {
                    self.send_system_message(player_id, "Commands: /items, /help").await;
                }
            }
            "/items" => {
                // List available items
                let items: Vec<&String> = self.item_registry.ids().collect();
                let list = items.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
                self.send_system_message(player_id, &format!("Items: {}", list)).await;
            }
            "/teleport" => {
                // /teleport <x> <y>
                if parts.len() < 3 {
                    self.send_system_message(player_id, "Usage: /teleport <x> <y>").await;
                    return;
                }
                let x: i32 = match parts[1].parse() {
                    Ok(v) => v,
                    Err(_) => {
                        self.send_system_message(player_id, "Invalid x coordinate").await;
                        return;
                    }
                };
                let y: i32 = match parts[2].parse() {
                    Ok(v) => v,
                    Err(_) => {
                        self.send_system_message(player_id, "Invalid y coordinate").await;
                        return;
                    }
                };
                {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        player.x = x;
                        player.y = y;
                        tracing::info!("Player {} teleported to ({}, {})", player_id, x, y);
                    }
                }
                self.send_system_message(player_id, &format!("Teleported to ({}, {})", x, y)).await;
                // Send teleport warp visual effect at the destination
                self.broadcast_to_zone(player_id, ServerMessage::SpellEffect {
                    caster_id: player_id.to_string(),
                    target_id: None,
                    spell_id: "teleport".to_string(),
                    target_x: x,
                    target_y: y,
                }).await;
            }
            "/spawn" => {
                // /spawn <npc_id> [x] [y]
                if parts.len() < 2 {
                    self.send_system_message(player_id, "Usage: /spawn <npc_id> [x] [y]").await;
                    return;
                }
                let npc_id = parts[1];

                // Get spawn position (player position if not specified)
                let (spawn_x, spawn_y) = {
                    let players = self.players.read().await;
                    if let Some(player) = players.get(player_id) {
                        let x = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(player.x);
                        let y = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(player.y);
                        (x, y)
                    } else {
                        return;
                    }
                };

                // Check if NPC type exists
                if self.entity_registry.get(npc_id).is_none() {
                    let available: Vec<&String> = self.entity_registry.ids().collect();
                    let list = available.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
                    self.send_system_message(player_id, &format!("Unknown NPC: {}. Available: {}", npc_id, list)).await;
                    return;
                }

                // Spawn the NPC
                if let Some(spawned_id) = self.spawn_npc_at(npc_id, spawn_x as f32, spawn_y as f32).await {
                    self.send_system_message(player_id, &format!("Spawned {} at ({}, {}) [id: {}]", npc_id, spawn_x, spawn_y, spawned_id)).await;
                    tracing::info!("Admin {} spawned {} at ({}, {})", player_id, npc_id, spawn_x, spawn_y);
                }
            }
            "/heal" => {
                // /heal [player_name]
                let target_name = parts.get(1).map(|s| *s);

                let healed = {
                    let mut players = self.players.write().await;
                    if let Some(name) = target_name {
                        // Find player by name
                        if let Some(player) = players.values_mut().find(|p| p.name.eq_ignore_ascii_case(name)) {
                            player.hp = player.max_hp();
                            player.is_dead = false;
                            Some(player.name.clone())
                        } else {
                            None
                        }
                    } else {
                        // Heal self
                        if let Some(player) = players.get_mut(player_id) {
                            player.hp = player.max_hp();
                            player.is_dead = false;
                            Some(player.name.clone())
                        } else {
                            None
                        }
                    }
                };

                match healed {
                    Some(name) => self.send_system_message(player_id, &format!("Healed {} to full HP", name)).await,
                    None => self.send_system_message(player_id, "Player not found").await,
                }
            }
            "/kill" => {
                // /kill <player_name>
                if parts.len() < 2 {
                    self.send_system_message(player_id, "Usage: /kill <player_name>").await;
                    return;
                }
                let target_name = parts[1];

                let killed = {
                    let current_time = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    let mut players = self.players.write().await;
                    if let Some(player) = players.values_mut().find(|p| p.name.eq_ignore_ascii_case(target_name)) {
                        let id = player.id.clone();
                        let name = player.name.clone();
                        player.die(current_time);
                        Some((id, name))
                    } else {
                        None
                    }
                };

                match killed {
                    Some((target_id, name)) => {
                        self.send_system_message(player_id, &format!("Killed {}", name)).await;
                        tracing::info!("Admin {} killed player {}", player_id, name);

                        // Handle arena death if applicable
                        let arena_death = {
                            let arena = self.arena_manager.read().await;
                            arena.is_fighting() && arena.is_in_ring(&target_id)
                        };
                        if arena_death {
                            let (eliminated_name, killer_name, remaining) = {
                                let mut arena = self.arena_manager.write().await;
                                arena.on_player_death(&target_id, Some(player_id));
                                let eliminated_name = arena.match_stats.fighter_names.get(&target_id).cloned().unwrap_or_default();
                                let killer_name = arena.match_stats.fighter_names.get(player_id).cloned().unwrap_or_default();
                                let remaining = arena.active_fighters.len() as u32;
                                (eliminated_name, killer_name, remaining)
                            };

                            // Teleport to spectator
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
                            }).await;

                            // Check match end
                            let should_end = {
                                let arena = self.arena_manager.read().await;
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

                                self.broadcast_to_arena(ServerMessage::ArenaMatchEnd {
                                    placements: placements.iter().map(|p| crate::protocol::ArenaPlacementData {
                                        rank: p.rank,
                                        player_id: p.player_id.clone(),
                                        player_name: p.player_name.clone(),
                                        kills: p.kills,
                                        gold_reward: p.gold_reward,
                                    }).collect(),
                                }).await;

                                self.broadcast_to_arena(ServerMessage::ArenaStateUpdate {
                                    state: "results".to_string(),
                                    countdown_remaining: None,
                                    queued_count: 0,
                                    fighter_count: 0,
                                    entry_fee: {
                                        let arena = self.arena_manager.read().await;
                                        arena.config.entry_fee
                                    },
                                }).await;
                            }
                        } else {
                            // Normal death broadcast
                            self.broadcast(ServerMessage::PlayerDied {
                                id: target_id,
                                killer_id: player_id.to_string(),
                            }).await;
                        }
                    }
                    None => self.send_system_message(player_id, "Player not found").await,
                }
            }
            "/god" => {
                // Toggle god mode (invincibility)
                let (enabled, player_name) = {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        player.is_god_mode = !player.is_god_mode;
                        (player.is_god_mode, player.name.clone())
                    } else {
                        return;
                    }
                };
                let status = if enabled { "enabled" } else { "disabled" };
                self.send_system_message(player_id, &format!("God mode {}", status)).await;
                tracing::info!("Admin {} ({}) toggled god mode: {}", player_name, player_id, status);
            }
            "/announce" => {
                // /announce <message>
                if parts.len() < 2 {
                    self.send_system_message(player_id, "Usage: /announce <message>").await;
                    return;
                }
                let message = parts[1..].join(" ");

                // Broadcast announcement to all players
                self.broadcast(ServerMessage::Announcement {
                    text: message.clone(),
                }).await;
                tracing::info!("Admin {} announced: {}", player_id, message);
            }
            "/arena" => {
                // Arena commands (admin only)
                if !is_admin {
                    self.send_system_message(player_id, "Arena commands require admin privileges.").await;
                    return;
                }

                let sub = parts.get(1).map(|s| s.to_lowercase()).unwrap_or_default();
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                match sub.as_str() {
                    "fee" => {
                        if let Some(fee) = parts.get(2).and_then(|s| s.parse::<i32>().ok()) {
                            if fee < 0 {
                                self.send_system_message(player_id, "Fee must be non-negative.").await;
                                return;
                            }
                            let mut arena = self.arena_manager.write().await;
                            arena.set_entry_fee(fee);
                            self.send_system_message(player_id, &format!("Arena entry fee set to {} gold.", fee)).await;
                        } else {
                            self.send_system_message(player_id, "Usage: /arena fee <gold>").await;
                        }
                    }
                    "start" => {
                        let result = {
                            let mut arena = self.arena_manager.write().await;
                            arena.start_countdown(current_time, None)
                        };
                        match result {
                            Ok(charges) => {
                                // Deduct gold from players
                                {
                                    let mut players = self.players.write().await;
                                    for (pid, amount) in &charges {
                                        if let Some(p) = players.get_mut(pid) {
                                            p.inventory.gold -= amount;
                                        }
                                    }
                                }
                                // Send inventory updates and notify
                                for (pid, _) in &charges {
                                    let update = {
                                        let players = self.players.read().await;
                                        players.get(pid).map(|p| (p.inventory.to_update(), p.inventory.gold))
                                    };
                                    if let Some((slots, gold)) = update {
                                        self.send_to_player(pid, ServerMessage::InventoryUpdate {
                                            player_id: pid.clone(),
                                            slots,
                                            gold,
                                        }).await;
                                    }
                                }
                                self.send_system_message(player_id, "Arena countdown started!").await;
                            }
                            Err(e) => {
                                self.send_system_message(player_id, &e).await;
                            }
                        }
                    }
                    "timer" => {
                        if let Some(seconds) = parts.get(2).and_then(|s| s.parse::<u64>().ok()) {
                            let result = {
                                let mut arena = self.arena_manager.write().await;
                                arena.start_countdown(current_time, Some(seconds * 1000))
                            };
                            match result {
                                Ok(charges) => {
                                    {
                                        let mut players = self.players.write().await;
                                        for (pid, amount) in &charges {
                                            if let Some(p) = players.get_mut(pid) {
                                                p.inventory.gold -= amount;
                                            }
                                        }
                                    }
                                    for (pid, _) in &charges {
                                        let update = {
                                            let players = self.players.read().await;
                                            players.get(pid).map(|p| (p.inventory.to_update(), p.inventory.gold))
                                        };
                                        if let Some((slots, gold)) = update {
                                            self.send_to_player(pid, ServerMessage::InventoryUpdate {
                                                player_id: pid.clone(),
                                                slots,
                                                gold,
                                            }).await;
                                        }
                                    }
                                    self.send_system_message(player_id, &format!("Arena countdown started ({}s)!", seconds)).await;
                                }
                                Err(e) => self.send_system_message(player_id, &e).await,
                            }
                        } else {
                            self.send_system_message(player_id, "Usage: /arena timer <seconds>").await;
                        }
                    }
                    "cancel" => {
                        let refunds = {
                            let mut arena = self.arena_manager.write().await;
                            arena.cancel()
                        };
                        // Refund gold
                        {
                            let mut players = self.players.write().await;
                            for (pid, amount) in &refunds {
                                if let Some(p) = players.get_mut(pid) {
                                    p.inventory.gold += amount;
                                }
                            }
                        }
                        for (pid, _) in &refunds {
                            let update = {
                                let players = self.players.read().await;
                                players.get(pid).map(|p| (p.inventory.to_update(), p.inventory.gold))
                            };
                            if let Some((slots, gold)) = update {
                                self.send_to_player(pid, ServerMessage::InventoryUpdate {
                                    player_id: pid.clone(),
                                    slots,
                                    gold,
                                }).await;
                            }
                        }
                        self.send_system_message(player_id, "Arena cancelled. All fees refunded.").await;
                    }
                    "status" => {
                        let status = {
                            let arena = self.arena_manager.read().await;
                            arena.get_status_text()
                        };
                        self.send_system_message(player_id, &status).await;
                    }
                    _ => {
                        self.send_system_message(player_id, "Usage: /arena <start|timer|fee|cancel|status>").await;
                    }
                }
            }
            _ => {
                self.send_system_message(player_id, &format!("Unknown command: {}. Try /help", command)).await;
            }
        }
    }

    /// Send a system message to a specific player
    async fn send_system_message(&self, player_id: &str, text: &str) {
        let msg = ServerMessage::ChatMessage {
            sender_id: "system".to_string(),
            sender_name: "[System]".to_string(),
            text: text.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        self.send_to_player(player_id, msg).await;
    }

    pub async fn handle_attack(&self, player_id: &str) {
        // Determine attacker's instance context (None = overworld)
        let attacker_instance = self.player_instances.read().await.get(player_id).cloned();

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Get attacker info including combat stats
        let (attacker_name, attacker_x, attacker_y, attacker_dir, last_attack,
             combat_level, attack_bonus, strength_bonus) = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
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

            let base_atk_bonus = player.attack_bonus(&self.item_registry);
            let base_str_bonus = player.strength_bonus(&self.item_registry);

            // Apply prayer bonuses to attack and strength
            let active_ids: Vec<String> = player.active_prayers.iter().cloned().collect();
            let prayer_effects = self.prayer_registry.calculate_effects(&active_ids);
            let atk_bonus = prayer_effects.apply_attack_bonus(base_atk_bonus);
            let str_bonus = prayer_effects.apply_strength_bonus(base_str_bonus);

            (
                player.name.clone(),
                player.x, player.y, player.direction,
                player.last_attack_time,
                player.skills.combat.level,
                atk_bonus,
                str_bonus,
            )
        };

        // Check cooldown
        if current_time - last_attack < ATTACK_COOLDOWN_MS {
            tracing::info!("[ATTACK] Cooldown not met: current_time={}, last_attack={}, ATTACK_COOLDOWN_MS={}", current_time, last_attack, ATTACK_COOLDOWN_MS);
            return;
        }

        // Get weapon range and type
        let (weapon_range, weapon_type) = {
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

        // Broadcast attack animation to all clients (plays even if no target hit)
        let attack_type = match weapon_type {
            WeaponType::Ranged => "ranged",
            WeaponType::Melee => "melee",
        };
        self.broadcast(ServerMessage::PlayerAttack {
            player_id: player_id.to_string(),
            attack_type: attack_type.to_string(),
        }).await;

        // Find target based on weapon range
        let mut target_id: Option<String> = None;
        let mut is_npc = false;
        let mut target_tile_x = attacker_x;
        let mut target_tile_y = attacker_y;

        // Direction vectors for 8 directions
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

        // Scan tiles in facing direction up to weapon range
        for dist in 1..=weapon_range {
            let check_x = attacker_x + dir_dx * dist;
            let check_y = attacker_y + dir_dy * dist;

            // For ranged weapons, check line of sight
            if weapon_range > 1 && !self.world.has_line_of_sight(attacker_x, attacker_y, check_x, check_y).await {
                tracing::debug!("{} ranged attack blocked by wall at ({}, {})", attacker_name, check_x, check_y);
                break;
            }

            // Check NPCs at this tile (overworld NPCs only - instance NPCs are separate)
            if attacker_instance.is_none() {
                let npcs = self.npcs.read().await;
                for (npc_id, npc) in npcs.iter() {
                    if npc.is_alive() && npc.is_attackable() && npc.x == check_x && npc.y == check_y {
                        target_id = Some(npc_id.clone());
                        is_npc = true;
                        target_tile_x = check_x;
                        target_tile_y = check_y;
                        tracing::info!("{} found NPC target: {} at ({}, {}) range {}", attacker_name, npc.name(), check_x, check_y, dist);
                        break;
                    }
                }
            }
            if target_id.is_some() { break; }

            // Check players at this tile (must be in same instance context)
            {
                let players = self.players.read().await;
                let instances = self.player_instances.read().await;
                for (pid, player) in players.iter() {
                    if pid != player_id && player.active && player.hp > 0 && player.x == check_x && player.y == check_y {
                        // Only target players in the same context (both overworld, or same instance)
                        let target_instance = instances.get(pid.as_str()).cloned();
                        if target_instance != attacker_instance {
                            continue;
                        }
                        target_id = Some(pid.clone());
                        is_npc = false;
                        target_tile_x = check_x;
                        target_tile_y = check_y;
                        tracing::info!("{} found player target: {} at ({}, {}) range {}", attacker_name, player.name, check_x, check_y, dist);
                        break;
                    }
                }
            }
            if target_id.is_some() { break; }
        }

        // No valid target found
        let target_id = match target_id {
            Some(id) => id,
            None => {
                tracing::debug!("{} attack missed - no target in range {} facing {:?}", attacker_name, weapon_range, attacker_dir);
                return;
            }
        };

        // Update attacker's last attack time and stop movement
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.last_attack_time = current_time;
                // Stop movement when attacking (player must stand still to attack)
                player.move_dx = 0;
                player.move_dy = 0;
            }
        }

        // Apply damage to target using hit/miss mechanics
        // 1. Roll attack vs defence to determine if we hit
        // 2. If hit, calculate max hit from strength and roll damage
        let (target_hp, target_name, target_died, actual_damage) = if is_npc {
            // NPCs use their level as both attack and defence level (no equipment bonuses)
            let mut npcs = self.npcs.write().await;
            if let Some(npc) = npcs.get_mut(&target_id) {
                // NPC's defence = level, no equipment bonus
                let npc_defence_level = npc.level;
                let npc_defence_bonus = npc.stats.defence_bonus;

                // Check if attack hits (combat_level used for both attack and strength)
                if !calculate_hit(combat_level, attack_bonus, npc_defence_level, npc_defence_bonus) {
                    // Miss - deal 0 damage
                    let name = npc.name();
                    tracing::info!(
                        "{} misses {} (atk {} + {} vs def {} + {})",
                        attacker_name, name, combat_level, attack_bonus, npc_defence_level, npc_defence_bonus
                    );
                    (npc.hp, name, false, 0)
                } else {
                    // Hit - calculate and apply damage
                    let max_hit = calculate_max_hit(combat_level, strength_bonus);
                    let damage = roll_damage(max_hit);
                    let died = npc.take_damage(damage, current_time, Some(player_id));
                    let name = npc.name();
                    tracing::info!(
                        "{} hits {} for {} damage (max: {}, HP: {})",
                        attacker_name, name, damage, max_hit, npc.hp
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

                // Get target's defence stats (uses combat level for defence)
                let target_combat_level = target.skills.combat.level;
                let base_defence_bonus = target.defence_bonus(&self.item_registry);

                // Apply prayer bonuses to target's defence
                let target_active_ids: Vec<String> = target.active_prayers.iter().cloned().collect();
                let target_prayer_effects = self.prayer_registry.calculate_effects(&target_active_ids);
                let target_defence_bonus = target_prayer_effects.apply_defence_bonus(base_defence_bonus);

                // Check if attack hits
                if !calculate_hit(combat_level, attack_bonus, target_combat_level, target_defence_bonus) {
                    // Miss - deal 0 damage
                    let name = target.name.clone();
                    tracing::info!(
                        "{} misses {} (cmb {} + {} vs cmb {} + {})",
                        attacker_name, name, combat_level, attack_bonus, target_combat_level, target_defence_bonus
                    );
                    (target.hp, name, false, 0)
                } else {
                    // Hit - calculate and apply damage
                    let max_hit = calculate_max_hit(combat_level, strength_bonus);
                    let raw_damage = roll_damage(max_hit);
                    // Apply prayer damage reduction
                    let damage = target_prayer_effects.apply_damage_reduction(raw_damage);
                    target.hp = (target.hp - damage).max(0);
                    let name = target.name.clone();
                    let died = target.hp <= 0;
                    if died {
                        target.die(current_time);
                    }
                    tracing::info!(
                        "{} hits {} for {} damage (max: {}, raw: {}, HP: {})",
                        attacker_name, name, damage, max_hit, raw_damage, target.hp
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
                    Some(attacker.award_combat_xp(actual_damage))
                } else {
                    None
                }
            };

            if let Some(results) = xp_results {
                for (skill_type, xp_gained, total_xp, level, leveled_up) in results {
                    self.send_to_player(player_id, ServerMessage::SkillXp {
                        player_id: player_id.to_string(),
                        skill: skill_type.as_str().to_string(),
                        xp_gained,
                        total_xp,
                        level,
                    }).await;

                    if leveled_up {
                        tracing::info!("Player {} leveled up {} to {}", player_id, skill_type.as_str(), level);
                        self.broadcast(ServerMessage::SkillLevelUp {
                            player_id: player_id.to_string(),
                            skill: skill_type.as_str().to_string(),
                            new_level: level,
                        }).await;
                    }
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
                let (prototype_id, npc_level) = {
                    let npcs = self.npcs.read().await;
                    npcs.get(&target_id)
                        .map(|n| (n.prototype_id.clone(), n.level))
                        .unwrap_or(("unknown".to_string(), 1))
                };

                // Broadcast NPC death
                let death_msg = ServerMessage::NpcDied {
                    id: target_id.clone(),
                    killer_id: player_id.to_string(),
                };
                self.broadcast(death_msg).await;

                // Process quest kill event
                self.process_quest_kill(player_id, &prototype_id).await;

                // Spawn item drops from prototype loot table
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                // Get killer's current instance for loot zone tracking
                let killer_instance = {
                    let instances = self.player_instances.read().await;
                    instances.get(player_id).cloned()
                };

                let drops = if let Some(prototype) = self.entity_registry.get(&prototype_id) {
                    crate::entity::generate_loot_from_prototype(
                        prototype, target_x, target_y, player_id, current_time, npc_level, killer_instance
                    )
                } else {
                    vec![] // No prototype found, no drops
                };

                for item in drops {
                    let mut items = self.ground_items.write().await;

                    // For gold, try to combine with existing pile at same tile
                    if item.item_id == "gold" {
                        let tile_x = item.x.floor() as i32;
                        let tile_y = item.y.floor() as i32;

                        // Find existing gold at same tile with same owner
                        let existing_gold_id = items.iter()
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
                        let eliminated_name = arena.match_stats.fighter_names.get(&target_id).cloned().unwrap_or_default();
                        let killer_name = arena.match_stats.fighter_names.get(player_id).cloned().unwrap_or_default();
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
                    }).await;

                    // Check if match should end
                    let should_end = {
                        let arena = self.arena_manager.read().await;
                        tracing::info!("[ARENA] After death: active_fighters={:?}, state={:?}, check_match_end={}",
                            arena.active_fighters, arena.state, arena.check_match_end());
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

                        let placement_data: Vec<crate::protocol::ArenaPlacementData> = placements.iter().map(|p| {
                            crate::protocol::ArenaPlacementData {
                                rank: p.rank,
                                player_id: p.player_id.clone(),
                                player_name: p.player_name.clone(),
                                kills: p.kills,
                                gold_reward: p.gold_reward,
                            }
                        }).collect();

                        self.broadcast_to_arena(ServerMessage::ArenaMatchEnd {
                            placements: placement_data,
                        }).await;

                        self.broadcast_to_arena(ServerMessage::ArenaStateUpdate {
                            state: "results".to_string(),
                            countdown_remaining: None,
                            queued_count: 0,
                            fighter_count: 0,
                            entry_fee: {
                                let arena = self.arena_manager.read().await;
                                arena.config.entry_fee
                            },
                        }).await;

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
                                    players.get(&placement.player_id).map(|p| {
                                        (p.inventory.to_update(), p.inventory.gold)
                                    })
                                };
                                if let Some((slots, gold)) = update {
                                    self.send_to_player(&placement.player_id, ServerMessage::InventoryUpdate {
                                        player_id: placement.player_id.clone(),
                                        slots,
                                        gold,
                                    }).await;
                                }
                            }
                        }

                        // Save arena stats to DB
                        if let Some(ref db) = self.db {
                            for placement in &placements {
                                if let Some(char_id) = placement.player_id.strip_prefix("char_").and_then(|s| s.parse::<i64>().ok()) {
                                    let won = placement.rank == 1;
                                    let died = placement.rank > 1;
                                    if let Err(e) = db.update_arena_stats(
                                        char_id,
                                        won,
                                        placement.kills,
                                        died,
                                        placement.gold_reward,
                                    ).await {
                                        tracing::warn!("Failed to save arena stats for {}: {}", placement.player_id, e);
                                    }
                                }
                            }
                        }

                        // Notify winner
                        if let Some(winner) = placements.iter().find(|p| p.rank == 1) {
                            self.send_system_message(&winner.player_id, &format!(
                                "You won the arena match! +{} gold", winner.gold_reward
                            )).await;
                        }
                    }
                } else {
                    // Normal player death
                    let death_msg = ServerMessage::PlayerDied {
                        id: target_id.clone(),
                        killer_id: player_id.to_string(),
                    };
                    self.broadcast(death_msg).await;

                    // Send prayer state update to dying player (prayers cleared on death)
                    let (points, max_points) = {
                        let players = self.players.read().await;
                        if let Some(p) = players.get(&target_id) {
                            (p.prayer_points, p.max_prayer_points())
                        } else {
                            (0, 1)
                        }
                    };
                    self.send_to_player(&target_id, ServerMessage::PrayerStateUpdate {
                        points,
                        max_points,
                        active_prayers: vec![],  // Cleared on death
                    }).await;
                }
            }
        }
    }

    /// Process quest kill event
    async fn process_quest_kill(&self, player_id: &str, entity_type: &str) {
        // Get player's quest state
        let mut quest_states = self.player_quest_states.write().await;
        let quest_state = quest_states.entry(player_id.to_string())
            .or_insert_with(PlayerQuestState::new);

        // Create kill event
        let event = QuestEvent::MonsterKilled {
            player_id: player_id.to_string(),
            entity_type: entity_type.to_string(),
            level: 1, // TODO: Get actual monster level from context
        };

        // Process the event through the registry
        let results = self.quest_registry.process_event(&event, quest_state).await;

        // Handle results - send notifications to player
        for result in results {
            if let (Some(objective_id), Some(current), Some(target)) =
                (&result.objective_id, result.new_progress, result.target)
            {
                tracing::info!(
                    "Player {} progress on objective {} for quest {}: {}/{}",
                    player_id, objective_id, result.quest_id, current, target
                );

                // Send objective progress update to player
                if let Some(sender) = self.player_senders.read().await.get(player_id) {
                    let msg = ServerMessage::QuestObjectiveProgress {
                        quest_id: result.quest_id.clone(),
                        objective_id: objective_id.clone(),
                        current,
                        target,
                    };
                    if let Ok(data) = crate::protocol::encode_server_message(&msg) {
                        let _ = sender.send(data).await;
                    }
                }

                if result.objective_completed {
                    tracing::info!(
                        "Player {} completed objective {} for quest {}",
                        player_id, objective_id, result.quest_id
                    );
                }
            }

            if result.quest_ready {
                tracing::info!(
                    "Player {} quest {} is ready to complete!",
                    player_id, result.quest_id
                );
            }
        }
    }

    /// Process quest item collection event
    async fn process_quest_item_collect(&self, player_id: &str, item_id: &str, count: i32) {
        let mut quest_states = self.player_quest_states.write().await;
        let quest_state = quest_states.entry(player_id.to_string())
            .or_insert_with(PlayerQuestState::new);

        let event = QuestEvent::ItemCollected {
            player_id: player_id.to_string(),
            item_id: item_id.to_string(),
            count,
        };

        let results = self.quest_registry.process_event(&event, quest_state).await;

        for result in results {
            if let (Some(objective_id), Some(current), Some(target)) =
                (&result.objective_id, result.new_progress, result.target)
            {
                tracing::info!(
                    "Player {} collected quest item objective {} for quest {}: {}/{}",
                    player_id, objective_id, result.quest_id, current, target
                );

                if let Some(sender) = self.player_senders.read().await.get(player_id) {
                    let msg = ServerMessage::QuestObjectiveProgress {
                        quest_id: result.quest_id.clone(),
                        objective_id: objective_id.clone(),
                        current,
                        target,
                    };
                    if let Ok(data) = crate::protocol::encode_server_message(&msg) {
                        let _ = sender.send(data).await;
                    }
                }
            }
        }
    }

    /// Process quest tree depletion event
    async fn process_quest_tree_deplete(&self, player_id: &str, tree_type: &str, tree_x: i32, tree_y: i32) {
        tracing::info!(
            "Processing tree depletion quest event: player={}, tree_type={}, pos=({}, {})",
            player_id, tree_type, tree_x, tree_y
        );

        let mut quest_states = self.player_quest_states.write().await;
        let quest_state = quest_states.entry(player_id.to_string())
            .or_insert_with(PlayerQuestState::new);

        let event = QuestEvent::TreeDepleted {
            player_id: player_id.to_string(),
            tree_type: tree_type.to_string(),
            x: tree_x,
            y: tree_y,
        };

        let results = self.quest_registry.process_event(&event, quest_state).await;
        tracing::info!("Tree depletion quest event returned {} results", results.len());

        for result in results {
            if let (Some(objective_id), Some(current), Some(target)) =
                (&result.objective_id, result.new_progress, result.target)
            {
                tracing::info!(
                    "Player {} depleted tree for quest objective {} in quest {}: {}/{}",
                    player_id, objective_id, result.quest_id, current, target
                );

                if let Some(sender) = self.player_senders.read().await.get(player_id) {
                    let msg = ServerMessage::QuestObjectiveProgress {
                        quest_id: result.quest_id.clone(),
                        objective_id: objective_id.clone(),
                        current,
                        target,
                    };
                    if let Ok(data) = crate::protocol::encode_server_message(&msg) {
                        let _ = sender.send(data).await;
                    }
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
        tracing::info!("Target request: player {} -> target '{}'", player_id, target_id);

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
                    // Check if target is an NPC
                    let npcs = self.npcs.read().await;
                    npcs.get(target_id).map(|n| n.is_alive()).unwrap_or(false)
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
            self.send_system_message(player_id, &format!(
                "That item is protected for {} more second{}.",
                secs,
                if secs == 1 { "" } else { "s" }
            )).await;
            return;
        }

        if let Some((picked_item_id, quantity)) = item_info {
            // Check if player has inventory space before removing from ground
            let has_space = {
                let players = self.players.read().await;
                match players.get(player_id) {
                    Some(player) => player.inventory.has_space_for(&picked_item_id, quantity, &self.item_registry),
                    None => return,
                }
            };

            if !has_space {
                self.send_system_message(player_id, "Your inventory is full.").await;
                return;
            }

            // Remove item from ground
            let removed = {
                let mut items = self.ground_items.write().await;
                items.remove(item_id).is_some()
            };

            if removed {
                // Get display name from registry for logging
                let display_name = self.item_registry
                    .get(&picked_item_id)
                    .map(|def| def.display_name.as_str())
                    .unwrap_or(&picked_item_id);
                tracing::debug!("Player {} picked up {} x{}", player_id, display_name, quantity);

                // Add to player's inventory
                let (inventory_update, gold) = {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        player.inventory.add_item(&picked_item_id, quantity, &self.item_registry);
                        (player.inventory.to_update(), player.inventory.gold)
                    } else {
                        return;
                    }
                };

                // Process quest item collection
                self.process_quest_item_collect(player_id, &picked_item_id, quantity).await;

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
                tracing::warn!("Player {} in instance {} but instance not found", player_id, inst_id);
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
                tracing::warn!("Player {} tried to interact with unknown NPC {}", player_id, npc_id);
                return;
            }
        };

        // Must be within interaction range (2 tiles) and NPC must be alive
        if distance > 2.5 || !is_alive {
            tracing::debug!(
                "Player {} can't interact with NPC {} (distance: {}, alive: {})",
                player_id, npc_id, distance, is_alive
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
                                    i + 1, name, wins, kills, gold
                                ));
                            }
                        }
                        self.send_to_player(player_id, ServerMessage::ShowDialogue {
                            quest_id: String::new(),
                            npc_id: npc_id.to_string(),
                            speaker: "Arena Leaderboard".to_string(),
                            text,
                            choices: vec![crate::protocol::DialogueChoice {
                                id: "close".to_string(),
                                text: "Close".to_string(),
                            }],
                        }).await;
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch arena leaderboard: {}", e);
                        self.send_system_message(player_id, "Failed to load leaderboard.").await;
                    }
                }
            }
            return;
        }

        // Altar interaction - show pray option and prayer point restoration
        let is_altar = self.entity_registry.get(&entity_type)
            .map(|p| p.behaviors.altar)
            .unwrap_or(false);

        if is_altar {
            // Get player's current prayer points
            let (current_points, max_points) = {
                let players = self.players.read().await;
                match players.get(player_id) {
                    Some(p) => (p.prayer_points, p.max_prayer_points()),
                    None => return,
                }
            };

            let altar_name = self.entity_registry.get(&entity_type)
                .map(|p| p.display_name.clone())
                .unwrap_or_else(|| "Altar".to_string());

            let text = if current_points < max_points {
                format!(
                    "You stand before the sacred altar.\n\nPrayer Points: {}/{}\n\nWould you like to pray and restore your prayer points?",
                    current_points, max_points
                )
            } else {
                format!(
                    "You stand before the sacred altar.\n\nPrayer Points: {}/{}\n\nYour prayer is already full. You may offer bones here for enhanced experience.",
                    current_points, max_points
                )
            };

            // Use a special quest_id format "altar:{npc_id}" to identify altar dialogues
            self.send_to_player(player_id, ServerMessage::ShowDialogue {
                quest_id: format!("altar:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker: altar_name,
                text,
                choices: vec![
                    crate::protocol::DialogueChoice {
                        id: "pray".to_string(),
                        text: "Pray".to_string(),
                    },
                    crate::protocol::DialogueChoice {
                        id: "close".to_string(),
                        text: "Close".to_string(),
                    },
                ],
            }).await;
            return;
        }

        // Plot seller interaction - show plot purchase dialogue
        let is_plot_seller = self.entity_registry.get(&entity_type)
            .map(|p| p.behaviors.plot_seller)
            .unwrap_or(false);

        if is_plot_seller {
            let npc_name = self.entity_registry.get(&entity_type)
                .map(|p| p.display_name.clone())
                .unwrap_or_else(|| "Master Farmer".to_string());

            self.send_to_player(player_id, ServerMessage::ShowDialogue {
                quest_id: format!("plot_seller:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker: npc_name,
                text: "Ah, welcome! I've been tending these fields for decades. What can I help you with?".to_string(),
                choices: vec![
                    crate::protocol::DialogueChoice {
                        id: "buy_plots".to_string(),
                        text: "Buy allotment plot".to_string(),
                    },
                    crate::protocol::DialogueChoice {
                        id: "nevermind".to_string(),
                        text: "Nevermind".to_string(),
                    },
                ],
            }).await;
            return;
        }

        // Check entity prototype for behaviors
        let prototype = self.entity_registry.get(&entity_type);

        // Check if this NPC is a merchant/craftsman
        let is_merchant = prototype.as_ref()
            .map(|p| p.behaviors.merchant || p.behaviors.craftsman)
            .unwrap_or(false);

        // Check if this NPC has quests associated with it
        let quests = self.quest_registry.get_quests_for_npc(&entity_type).await;

        // If merchant/craftsman and no quests (or quest_giver behavior is not set), open shop
        let is_quest_giver = prototype.as_ref()
            .map(|p| p.behaviors.quest_giver)
            .unwrap_or(false);

        // Check if all quests for this NPC are completed (for merchant fallback)
        let all_quests_completed = if is_merchant && is_quest_giver && !quests.is_empty() {
            let quest_states = self.player_quest_states.read().await;
            if let Some(quest_state) = quest_states.get(player_id) {
                quests.iter().all(|q| quest_state.is_quest_completed(&q.id))
            } else {
                false
            }
        } else {
            false
        };

        if is_merchant && (quests.is_empty() || !is_quest_giver || all_quests_completed) {
            tracing::info!("Player {} opening shop with NPC {} ({})", player_id, npc_id, entity_type);

            // Get merchant config to load shop data
            if let Some(proto) = prototype {
                if let Some(merchant_config) = &proto.merchant {
                    // Get shop definition from registry
                    let shop_registry = self.shop_registry.read().await;
                    if let Some(shop_def) = shop_registry.get(&merchant_config.shop_id) {
                        // Build shop data with current prices
                        let stock = shop_def.stock.iter().map(|item| {
                            let base_price = self.item_registry
                                .get(&item.item_id)
                                .map(|def| def.base_price)
                                .unwrap_or(10);
                            let price = (base_price as f32 * merchant_config.sell_multiplier).max(1.0) as i32;

                            crate::protocol::ShopStockItemData {
                                item_id: item.item_id.clone(),
                                quantity: item.current_quantity,
                                price,
                            }
                        }).collect();

                        let shop_data = crate::protocol::ShopData {
                            shop_id: shop_def.id.clone(),
                            display_name: shop_def.display_name.clone(),
                            buy_multiplier: merchant_config.buy_multiplier,
                            sell_multiplier: merchant_config.sell_multiplier,
                            show_crafting: merchant_config.show_crafting,
                            stock,
                        };

                        drop(shop_registry);

                        let msg = ServerMessage::ShopData {
                            npc_id: npc_id.to_string(),
                            shop: shop_data,
                        };
                        self.send_to_player(player_id, msg).await;
                        return;
                    } else {
                        tracing::warn!("Shop '{}' not found for merchant NPC {}", merchant_config.shop_id, npc_id);
                    }
                }
            }

            // Fallback: send empty ShopOpen if shop data couldn't be loaded
            let msg = ServerMessage::ShopOpen {
                npc_id: npc_id.to_string(),
            };
            self.send_to_player(player_id, msg).await;
            return;
        }

        if quests.is_empty() {
            tracing::debug!("NPC {} ({}) has no quests", npc_id, entity_type);
            // Could show generic dialogue here
            return;
        }

        // Get or create player quest state
        let mut quest_states = self.player_quest_states.write().await;
        let quest_state = quest_states.entry(player_id.to_string())
            .or_insert_with(PlayerQuestState::new);

        // Find the appropriate quest to interact with
        // Priority: 1) Active quest ready to complete, 2) Active quest in progress,
        //           3) Available new quest (or repeatable completed quest), 4) Completed quest (for post-completion dialogue)
        let mut target_quest: Option<(String, &str)> = None;
        let mut completed_quest: Option<(String, &str)> = None;

        for quest in &quests {
            let quest_id = &quest.id;

            if let Some(progress) = quest_state.get_quest(quest_id) {
                // Player has this quest active
                if progress.status == crate::quest::QuestStatus::ReadyToComplete {
                    target_quest = Some((quest_id.clone(), "ready_to_complete"));
                    break;
                } else if progress.status == crate::quest::QuestStatus::Active {
                    target_quest = Some((quest_id.clone(), "in_progress"));
                    // Don't break, keep looking for ready_to_complete
                }
            } else if quest_state.is_quest_completed(quest_id) {
                // Quest is completed
                if quest.repeatable {
                    // Repeatable quest - treat as not started so player can do it again
                    if target_quest.is_none() {
                        // Remove from completed list so it can be restarted
                        quest_state.completed_quests.retain(|id| id != quest_id);
                        target_quest = Some((quest_id.clone(), "not_started"));
                    }
                } else {
                    // Non-repeatable - save as fallback for post-completion dialogue
                    if completed_quest.is_none() {
                        completed_quest = Some((quest_id.clone(), "completed"));
                    }
                }
            } else {
                // Player doesn't have this quest and hasn't completed it
                if target_quest.is_none() {
                    target_quest = Some((quest_id.clone(), "not_started"));
                }
            }
        }

        // Use completed quest as fallback if no other quest is found
        if target_quest.is_none() {
            target_quest = completed_quest;
        }

        if let Some((quest_id, state)) = target_quest {
            tracing::info!(
                "Player {} interacting with quest {} (state: {})",
                player_id, quest_id, state
            );

            // Trigger NpcInteraction event to complete any talk_to objectives
            // This must happen BEFORE running the script so the script sees the updated state
            let event = QuestEvent::NpcInteraction {
                player_id: player_id.to_string(),
                npc_id: entity_type.clone(),
            };
            let talk_results = self.quest_registry.process_event(&event, quest_state).await;
            for result in talk_results {
                if result.quest_ready {
                    tracing::info!(
                        "Player {} quest {} is now ready to complete after talking to NPC",
                        player_id, result.quest_id
                    );
                }
            }

            // Run the quest script interaction
            let result = self.quest_runner.run_on_interact(
                player_id,
                &quest_id,
                quest_state,
                None, // No choice yet
            ).await;

            match result {
                Ok(script_result) => {
                    // Send dialogue to player
                    if let Some(dialogue) = script_result.dialogue {
                        let choices: Vec<crate::protocol::DialogueChoice> = dialogue.choices
                            .into_iter()
                            .map(|c| crate::protocol::DialogueChoice {
                                id: c.id,
                                text: c.text,
                            })
                            .collect();

                        let msg = ServerMessage::ShowDialogue {
                            quest_id: quest_id.clone(),
                            npc_id: npc_id.to_string(),
                            speaker: dialogue.speaker,
                            text: dialogue.text,
                            choices,
                        };
                        self.send_to_player(player_id, msg).await;

                        // Persist dialogue step for proper dialogue flow
                        if let Some(step) = script_result.new_dialogue_step {
                            let step_key = format!("{}_dialogue_step", quest_id);
                            quest_state.set_flag(&step_key, &step.to_string());
                        }
                    }

                    // Handle quest acceptance
                    if script_result.quest_accepted {
                        if let Some(quest) = self.quest_registry.get(&quest_id).await {
                            let objective_targets: Vec<(String, i32)> = quest.objectives
                                .iter()
                                .map(|o| (o.id.clone(), o.count))
                                .collect();
                            quest_state.start_quest(&quest_id, &objective_targets);
                            tracing::info!("Player {} accepted quest {}", player_id, quest_id);

                            // Send QuestAccepted to client
                            let objectives: Vec<QuestObjectiveData> = quest.objectives
                                .iter()
                                .map(|o| QuestObjectiveData {
                                    id: o.id.clone(),
                                    description: o.description.clone(),
                                    current: 0,
                                    target: o.count,
                                    completed: false,
                                })
                                .collect();
                            let msg = ServerMessage::QuestAccepted {
                                quest_id: quest_id.clone(),
                                quest_name: quest.name.clone(),
                                objectives,
                            };
                            self.send_to_player(player_id, msg).await;
                        }
                    }

                    // Handle quest completion
                    if script_result.quest_completed {
                        quest_state.complete_quest(&quest_id);
                        if let Some(quest) = self.quest_registry.get(&quest_id).await {
                            let msg = ServerMessage::QuestCompleted {
                                quest_id: quest_id.clone(),
                                quest_name: quest.name.clone(),
                                rewards_exp: quest.rewards.exp,
                                rewards_gold: quest.rewards.gold,
                            };
                            self.send_to_player(player_id, msg).await;

                            // Grant rewards
                            let mut players = self.players.write().await;
                            if let Some(player) = players.get_mut(player_id) {
                                player.inventory.gold += quest.rewards.gold;
                                for item_reward in &quest.rewards.items {
                                    player.inventory.add_item(&item_reward.item_id, item_reward.count, &self.item_registry);
                                }
                                let slots = player.inventory.to_update();
                                let gold = player.inventory.gold;
                                drop(players);
                                self.send_to_player(player_id, ServerMessage::InventoryUpdate {
                                    player_id: player_id.to_string(),
                                    slots,
                                    gold,
                                }).await;
                            }
                        }
                        tracing::info!("Player {} completed quest {}", player_id, quest_id);
                    }

                    // Handle granted items from give_item()
                    if !script_result.granted_items.is_empty() {
                        let mut players = self.players.write().await;
                        if let Some(player) = players.get_mut(player_id) {
                            for (item_id, count) in &script_result.granted_items {
                                player.inventory.add_item(item_id, *count, &self.item_registry);
                            }
                            let slots = player.inventory.to_update();
                            let gold = player.inventory.gold;
                            drop(players);
                            self.send_to_player(player_id, ServerMessage::InventoryUpdate {
                                player_id: player_id.to_string(),
                                slots,
                                gold,
                            }).await;
                        }
                    }

                    // Send notifications
                    for notification in script_result.notifications {
                        tracing::info!("Quest notification for {}: {}", player_id, notification);
                        // TODO: Send notification message to client
                    }
                }
                Err(e) => {
                    tracing::error!("Quest script error: {}", e);
                }
            }
        }
    }

    /// Handle dialogue choice from player
    pub async fn handle_dialogue_choice(&self, player_id: &str, quest_id: &str, choice_id: &str) {
        // Non-quest dialogues (e.g. leaderboard) just close
        if quest_id.is_empty() {
            self.send_to_player(player_id, ServerMessage::DialogueClosed).await;
            return;
        }

        // Handle altar dialogue choices (format: "altar:{altar_id}")
        if let Some(altar_id) = quest_id.strip_prefix("altar:") {
            self.send_to_player(player_id, ServerMessage::DialogueClosed).await;
            if choice_id == "pray" {
                self.handle_pray_at_altar(player_id, altar_id).await;
            }
            // "close" choice just closes the dialogue (already sent DialogueClosed above)
            return;
        }

        // Handle plot seller dialogue choices (format: "plot_seller:{npc_id}")
        if let Some(npc_id) = quest_id.strip_prefix("plot_seller:") {
            if choice_id == "buy_plots" {
                // Show the plot purchase screen
                self.show_plot_purchase_dialogue(player_id, npc_id).await;
            } else if let Some(plot_str) = choice_id.strip_prefix("unlock_") {
                self.send_to_player(player_id, ServerMessage::DialogueClosed).await;
                if let Ok(plot_id) = plot_str.parse::<u32>() {
                    self.handle_plot_purchase(player_id, plot_id).await;
                }
            } else {
                // "owned_N", "locked_N", "nevermind" all just close
                self.send_to_player(player_id, ServerMessage::DialogueClosed).await;
            }
            return;
        }

        let mut quest_states = self.player_quest_states.write().await;
        let quest_state = quest_states.entry(player_id.to_string())
            .or_insert_with(PlayerQuestState::new);

        // Run the quest script with the player's choice
        let result = self.quest_runner.run_on_interact(
            player_id,
            quest_id,
            quest_state,
            Some(choice_id),
        ).await;

        // Get the NPC id from the quest giver
        let npc_id = if let Some(quest) = self.quest_registry.get(quest_id).await {
            quest.giver_npc.clone()
        } else {
            String::new()
        };

        match result {
            Ok(script_result) => {
                // Send next dialogue if any, otherwise close the dialogue
                if let Some(dialogue) = script_result.dialogue {
                    let choices: Vec<crate::protocol::DialogueChoice> = dialogue.choices
                        .into_iter()
                        .map(|c| crate::protocol::DialogueChoice {
                            id: c.id,
                            text: c.text,
                        })
                        .collect();

                    let msg = ServerMessage::ShowDialogue {
                        quest_id: quest_id.to_string(),
                        npc_id: npc_id.clone(),
                        speaker: dialogue.speaker,
                        text: dialogue.text,
                        choices,
                    };
                    self.send_to_player(player_id, msg).await;
                    // Persist dialogue step for proper dialogue flow
                    if let Some(step) = script_result.new_dialogue_step {
                        let step_key = format!("{}_dialogue_step", quest_id);
                        quest_state.set_flag(&step_key, &step.to_string());
                    }
                } else {
                    // No follow-up dialogue - tell client to close
                    let msg = ServerMessage::DialogueClosed;
                    self.send_to_player(player_id, msg).await;

                    // Reset dialogue step since conversation ended
                    let step_key = format!("{}_dialogue_step", quest_id);
                    quest_state.flags.remove(&step_key);
                }

                // Handle quest acceptance
                if script_result.quest_accepted {
                    if let Some(quest) = self.quest_registry.get(quest_id).await {
                        let objective_targets: Vec<(String, i32)> = quest.objectives
                            .iter()
                            .map(|o| (o.id.clone(), o.count))
                            .collect();
                        quest_state.start_quest(quest_id, &objective_targets);
                        tracing::info!("Player {} accepted quest {}", player_id, quest_id);

                        // Send QuestAccepted to client
                        let objectives: Vec<QuestObjectiveData> = quest.objectives
                            .iter()
                            .map(|o| QuestObjectiveData {
                                id: o.id.clone(),
                                description: o.description.clone(),
                                current: 0,
                                target: o.count,
                                completed: false,
                            })
                            .collect();
                        let msg = ServerMessage::QuestAccepted {
                            quest_id: quest_id.to_string(),
                            quest_name: quest.name.clone(),
                            objectives,
                        };
                        self.send_to_player(player_id, msg).await;
                    }
                }

                // Handle quest completion
                if script_result.quest_completed {
                    quest_state.complete_quest(quest_id);
                    if let Some(quest) = self.quest_registry.get(quest_id).await {
                        let msg = ServerMessage::QuestCompleted {
                            quest_id: quest_id.to_string(),
                            quest_name: quest.name.clone(),
                            rewards_exp: quest.rewards.exp,
                            rewards_gold: quest.rewards.gold,
                        };
                        self.send_to_player(player_id, msg).await;

                        let mut players = self.players.write().await;
                        if let Some(player) = players.get_mut(player_id) {
                            player.inventory.gold += quest.rewards.gold;
                            for item_reward in &quest.rewards.items {
                                player.inventory.add_item(&item_reward.item_id, item_reward.count, &self.item_registry);
                            }
                            let slots = player.inventory.to_update();
                            let gold = player.inventory.gold;
                            drop(players);
                            self.send_to_player(player_id, ServerMessage::InventoryUpdate {
                                player_id: player_id.to_string(),
                                slots,
                                gold,
                            }).await;
                        }
                    }
                    tracing::info!("Player {} completed quest {}", player_id, quest_id);
                }

                // Handle granted items from give_item()
                if !script_result.granted_items.is_empty() {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        for (item_id, count) in &script_result.granted_items {
                            player.inventory.add_item(item_id, *count, &self.item_registry);
                        }
                        let slots = player.inventory.to_update();
                        let gold = player.inventory.gold;
                        drop(players);
                        self.send_to_player(player_id, ServerMessage::InventoryUpdate {
                            player_id: player_id.to_string(),
                            slots,
                            gold,
                        }).await;
                    }
                }
            }
            Err(e) => {
                tracing::error!("Quest script error: {}", e);
            }
        }
    }

    pub async fn handle_use_item(&self, player_id: &str, slot_index: u8) {
        // Block consumables during arena fights
        {
            let arena = self.arena_manager.read().await;
            if arena.is_fighting() && arena.is_in_ring(player_id) {
                self.send_system_message(player_id, "You can't use items during an arena fight!").await;
                return;
            }
        }

        // Check if this is a recipe scroll before the normal use_item path
        {
            let players = self.players.read().await;
            if let Some(player) = players.get(player_id) {
                if player.is_dead {
                    return;
                }
                if let Some(slot) = player.inventory.slots.get(slot_index as usize).and_then(|s| s.as_ref()) {
                    if slot.item_id.starts_with("recipe_") {
                        drop(players);
                        self.handle_use_recipe_scroll(player_id, slot_index).await;
                        return;
                    }
                }
            }
        }

        // Get player and try to use item
        let (used_item_id, effect, inventory_update, gold) = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                // Dead players can't use items
                if player.is_dead {
                    return;
                }

                if let Some(item_id) = player.inventory.use_item(slot_index as usize, &self.item_registry) {
                    // Get effect from registry
                    let effect = if let Some(def) = self.item_registry.get(&item_id) {
                        use crate::data::UseEffect;
                        match &def.use_effect {
                            Some(UseEffect::Heal { amount }) => {
                                player.hp = (player.hp + amount).min(player.max_hp());
                                format!("heal:{}", amount)
                            }
                            Some(UseEffect::RestoreMana { amount }) => {
                                // Mana not implemented yet
                                format!("mana:{}", amount)
                            }
                            Some(UseEffect::RestorePrayer { amount }) => {
                                // Prayer potion formula: amount + floor(prayer_level / 4)
                                let prayer_level = player.skills.prayer.level;
                                let restore_amount = amount + (prayer_level / 4);
                                let old_points = player.prayer_points;
                                player.prayer_points = (player.prayer_points + restore_amount).min(player.max_prayer_points());
                                let actual_restored = player.prayer_points - old_points;
                                format!("prayer:{}", actual_restored)
                            }
                            Some(UseEffect::Buff { stat, amount, duration_ms }) => {
                                // Buffs not implemented yet
                                format!("buff:{}:{}:{}", stat, amount, duration_ms)
                            }
                            Some(UseEffect::Teleport { destination }) => {
                                // Teleport not implemented yet
                                format!("teleport:{}", destination)
                            }
                            None => "none".to_string(),
                        }
                    } else {
                        "none".to_string()
                    };

                    let update = player.inventory.to_update();
                    (Some(item_id), effect, update, player.inventory.gold)
                } else {
                    return;
                }
            } else {
                return;
            }
        };

        if let Some(item_id) = used_item_id {
            let display_name = self.item_registry
                .get(&item_id)
                .map(|def| def.display_name.as_str())
                .unwrap_or(&item_id);
            tracing::debug!("Player {} used {} ({})", player_id, display_name, effect);

            // SECURITY: Unicast item used and inventory update (private to this player)
            let used_msg = ServerMessage::ItemUsed {
                player_id: player_id.to_string(),
                slot: slot_index,
                item_id: item_id.clone(),
                effect,
            };
            self.send_to_player(player_id, used_msg).await;

            let inv_msg = ServerMessage::InventoryUpdate {
                player_id: player_id.to_string(),
                slots: inventory_update,
                gold,
            };
            self.send_to_player(player_id, inv_msg).await;
        }
    }

    /// Handle using a recipe scroll item to discover a new recipe
    async fn handle_use_recipe_scroll(&self, player_id: &str, slot_index: u8) {
        // Extract item_id, recipe_id, and perform checks
        let (item_id, recipe_id, inventory_update, gold) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) if p.active && !p.is_dead => p,
                _ => return,
            };

            // Get item from slot
            let item_id = match player.inventory.slots.get(slot_index as usize).and_then(|s| s.as_ref()) {
                Some(slot) => slot.item_id.clone(),
                None => return,
            };

            // Extract recipe_id by stripping "recipe_" prefix
            let recipe_id = match item_id.strip_prefix("recipe_") {
                Some(id) => id.to_string(),
                None => return,
            };

            // Verify the recipe exists in the crafting registry
            if self.crafting_registry.get(&recipe_id).is_none() {
                drop(players);
                self.send_system_message(player_id, "This recipe scroll is for an unknown recipe.").await;
                return;
            }

            // Check if already discovered
            if player.discovered_recipes.contains(&recipe_id) {
                drop(players);
                self.send_system_message(player_id, "You already know this recipe.").await;
                return;
            }

            // Consume the scroll from inventory
            if let Some(ref mut slot) = player.inventory.slots[slot_index as usize] {
                slot.quantity -= 1;
                if slot.quantity <= 0 {
                    player.inventory.slots[slot_index as usize] = None;
                }
            }

            // Add to discovered recipes
            player.discovered_recipes.insert(recipe_id.clone());

            let update = player.inventory.to_update();
            let gold = player.inventory.gold;
            (item_id, recipe_id, update, gold)
        };

        let display_name = self.item_registry
            .get(&item_id)
            .map(|def| def.display_name.clone())
            .unwrap_or_else(|| item_id.clone());
        tracing::info!("Player {} used recipe scroll {} -> discovered recipe {}", player_id, display_name, recipe_id);

        // Save to database
        if let Some(ref db) = self.db {
            if let Some(character_id) = Self::parse_character_id(player_id) {
                if let Err(e) = db.save_discovered_recipe(character_id, &recipe_id).await {
                    tracing::warn!("Failed to save discovered recipe to DB: {}", e);
                }
            }
        }

        // Send RecipeDiscovered message
        self.send_to_player(
            player_id,
            ServerMessage::RecipeDiscovered {
                recipe_id: recipe_id.clone(),
            },
        ).await;

        // Send inventory update
        self.send_to_player(
            player_id,
            ServerMessage::InventoryUpdate {
                player_id: player_id.to_string(),
                slots: inventory_update,
                gold,
            },
        ).await;
    }

    /// Handle a crafting request from a player
    pub async fn handle_craft(&self, player_id: &str, recipe_id: &str) {
        use crate::protocol::RecipeResult as ProtoRecipeResult;

        // Get recipe definition
        let recipe = match self.crafting_registry.get(recipe_id) {
            Some(r) => r.clone(),
            None => {
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftResult {
                        success: false,
                        recipe_id: recipe_id.to_string(),
                        error: Some("Recipe not found".to_string()),
                        items_gained: vec![],
                    },
                )
                .await;
                return;
            }
        };

        // Get player and perform all checks
        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        // Check level requirement (use combat level for now, will add crafting skill later)
        if player.combat_level() < recipe.level_required {
            drop(players);
            self.send_to_player(
                player_id,
                ServerMessage::CraftResult {
                    success: false,
                    recipe_id: recipe_id.to_string(),
                    error: Some(format!("Requires combat level {}", recipe.level_required)),
                    items_gained: vec![],
                },
            )
            .await;
            return;
        }

        // Check all ingredients (using string IDs now)
        for ingredient in &recipe.ingredients {
            if !player.inventory.has_item(&ingredient.item_id, ingredient.count) {
                drop(players);
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftResult {
                        success: false,
                        recipe_id: recipe_id.to_string(),
                        error: Some("Missing ingredients".to_string()),
                        items_gained: vec![],
                    },
                )
                .await;
                return;
            }
        }

        // Check inventory space for results
        for result in &recipe.results {
            if !player.inventory.has_space_for(&result.item_id, result.count, &self.item_registry) {
                drop(players);
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftResult {
                        success: false,
                        recipe_id: recipe_id.to_string(),
                        error: Some("Inventory full".to_string()),
                        items_gained: vec![],
                    },
                )
                .await;
                return;
            }
        }

        // All checks passed - consume ingredients
        for ingredient in &recipe.ingredients {
            player.inventory.remove_item(&ingredient.item_id, ingredient.count);
        }

        // Add results
        let mut items_gained = Vec::new();
        for result in &recipe.results {
            player.inventory.add_item(&result.item_id, result.count, &self.item_registry);
            let display_name = self.item_registry
                .get(&result.item_id)
                .map(|def| def.display_name.clone())
                .unwrap_or_else(|| result.item_id.clone());
            items_gained.push(ProtoRecipeResult {
                item_id: result.item_id.clone(),
                item_name: display_name,
                count: result.count,
            });
        }

        // Get inventory update
        let inventory_update = player.inventory.to_update();
        let gold = player.inventory.gold;
        drop(players);

        tracing::info!(
            "Player {} crafted {} (gained {:?})",
            player_id,
            recipe_id,
            items_gained
        );

        // Send success result
        self.send_to_player(
            player_id,
            ServerMessage::CraftResult {
                success: true,
                recipe_id: recipe_id.to_string(),
                error: None,
                items_gained,
            },
        )
        .await;

        // Send inventory update
        self.send_to_player(
            player_id,
            ServerMessage::InventoryUpdate {
                player_id: player_id.to_string(),
                slots: inventory_update,
                gold,
            },
        )
        .await;
    }

    /// Handle start craft - supports both instant and timed crafting
    pub async fn handle_start_craft(&self, player_id: &str, recipe_id: &str) {
        use crate::crafting::definition::RecipeCategory;

        // Get recipe definition
        let recipe = match self.crafting_registry.get(recipe_id) {
            Some(r) => r.clone(),
            None => {
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftingCancelled {
                        reason: "Recipe not found".to_string(),
                    },
                )
                .await;
                return;
            }
        };

        // Get player and perform all checks
        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        // Check if already crafting
        if player.crafting_state.is_some() {
            drop(players);
            self.send_to_player(
                player_id,
                ServerMessage::CraftingCancelled {
                    reason: "Already crafting".to_string(),
                },
            )
            .await;
            return;
        }

        // Check level requirement - smithing recipes check smithing level, others use combat level
        let level_check_passed = if recipe.category == RecipeCategory::Smithing {
            player.skills.smithing.level >= recipe.level_required
        } else {
            player.combat_level() >= recipe.level_required
        };

        if !level_check_passed {
            let skill_name = if recipe.category == RecipeCategory::Smithing {
                "smithing"
            } else {
                "combat"
            };
            drop(players);
            self.send_to_player(
                player_id,
                ServerMessage::CraftingCancelled {
                    reason: format!("Requires {} level {}", skill_name, recipe.level_required),
                },
            )
            .await;
            return;
        }

        // Check recipe discovery requirement
        if recipe.requires_discovery && !player.discovered_recipes.contains(recipe_id) {
            drop(players);
            self.send_to_player(
                player_id,
                ServerMessage::CraftingCancelled {
                    reason: "Recipe not yet discovered".to_string(),
                },
            )
            .await;
            return;
        }

        // Check all ingredients
        for ingredient in &recipe.ingredients {
            if !player.inventory.has_item(&ingredient.item_id, ingredient.count) {
                drop(players);
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftingCancelled {
                        reason: "Missing ingredients".to_string(),
                    },
                )
                .await;
                return;
            }
        }

        // Check inventory space for results
        for result in &recipe.results {
            if !player.inventory.has_space_for(&result.item_id, result.count, &self.item_registry) {
                drop(players);
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftingCancelled {
                        reason: "Inventory full".to_string(),
                    },
                )
                .await;
                return;
            }
        }

        // All checks passed - consume ingredients
        let mut consumed_materials = Vec::new();
        for ingredient in &recipe.ingredients {
            player.inventory.remove_item(&ingredient.item_id, ingredient.count);
            consumed_materials.push((ingredient.item_id.clone(), ingredient.count));
        }

        if recipe.craft_time_ms == 0 {
            // Instant craft - add results immediately
            let mut items_gained = Vec::new();
            for result in &recipe.results {
                player.inventory.add_item(&result.item_id, result.count, &self.item_registry);
                items_gained.push((result.item_id.clone(), result.count as u32));
            }

            // Award smithing XP if applicable
            let xp_gained = recipe.xp;
            let mut xp_results = Vec::new();
            if xp_gained > 0 && recipe.category == RecipeCategory::Smithing {
                let leveled = player.skills.smithing.add_xp(xp_gained as i64);
                xp_results.push((
                    SkillType::Smithing,
                    xp_gained as i64,
                    player.skills.smithing.xp,
                    player.skills.smithing.level,
                    leveled,
                ));
            }

            let inventory_update = player.inventory.to_update();
            let gold = player.inventory.gold;
            drop(players);

            tracing::info!(
                "Player {} instant-crafted {} (gained {:?})",
                player_id, recipe_id, items_gained
            );

            // Send crafting completed
            self.send_to_player(
                player_id,
                ServerMessage::CraftingCompleted {
                    recipe_id: recipe_id.to_string(),
                    items_gained,
                    xp_gained,
                },
            )
            .await;

            // Send inventory update
            self.send_to_player(
                player_id,
                ServerMessage::InventoryUpdate {
                    player_id: player_id.to_string(),
                    slots: inventory_update,
                    gold,
                },
            )
            .await;

            // Send XP updates
            for (skill_type, xp_amount, total_xp, level, leveled_up) in xp_results {
                self.send_to_player(player_id, ServerMessage::SkillXp {
                    player_id: player_id.to_string(),
                    skill: skill_type.as_str().to_string(),
                    xp_gained: xp_amount,
                    total_xp,
                    level,
                }).await;

                if leveled_up {
                    tracing::info!("Player {} leveled up {} to {}", player_id, skill_type.as_str(), level);
                    self.broadcast(ServerMessage::SkillLevelUp {
                        player_id: player_id.to_string(),
                        skill: skill_type.as_str().to_string(),
                        new_level: level,
                    }).await;
                }
            }
        } else {
            // Timed craft - set crafting state, materials already consumed
            player.crafting_state = Some(CraftingState {
                recipe_id: recipe_id.to_string(),
                started_at: std::time::Instant::now(),
                duration_ms: recipe.craft_time_ms,
                consumed_materials,
            });

            let inventory_update = player.inventory.to_update();
            let gold = player.inventory.gold;
            drop(players);

            tracing::info!(
                "Player {} started timed craft {} ({}ms)",
                player_id, recipe_id, recipe.craft_time_ms
            );

            // Send inventory update (materials consumed)
            self.send_to_player(
                player_id,
                ServerMessage::InventoryUpdate {
                    player_id: player_id.to_string(),
                    slots: inventory_update,
                    gold,
                },
            )
            .await;

            // Send crafting started
            self.send_to_player(
                player_id,
                ServerMessage::CraftingStarted {
                    recipe_id: recipe_id.to_string(),
                    duration_ms: recipe.craft_time_ms,
                },
            )
            .await;
        }
    }

    /// Cancel an active timed craft, refunding materials
    pub async fn handle_cancel_craft(&self, player_id: &str) {
        self.cancel_crafting(player_id, "cancelled").await;
    }

    /// Internal helper to cancel crafting with a reason, refunding materials
    pub async fn cancel_crafting(&self, player_id: &str, reason: &str) {
        let refund_result = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                if let Some(crafting) = player.crafting_state.take() {
                    // Refund consumed materials
                    for (item_id, count) in &crafting.consumed_materials {
                        player.inventory.add_item(item_id, *count, &self.item_registry);
                    }
                    Some((player.inventory.to_update(), player.inventory.gold))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some((inv_update, gold)) = refund_result {
            self.send_to_player(
                player_id,
                ServerMessage::CraftingCancelled {
                    reason: reason.to_string(),
                },
            )
            .await;

            // Send inventory update (materials refunded)
            self.send_to_player(
                player_id,
                ServerMessage::InventoryUpdate {
                    player_id: player_id.to_string(),
                    slots: inv_update,
                    gold,
                },
            )
            .await;
        }
    }

    /// Handle shop buy transaction
    pub async fn handle_shop_buy(&self, player_id: &str, npc_id: &str, item_id: &str, quantity: i32) {
        // Validate quantity
        if quantity <= 0 {
            self.send_shop_result(player_id, false, "buy", item_id, 0, 0, Some("Invalid quantity")).await;
            return;
        }

        // Get player position and gold
        let (player_x, player_y, player_gold) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => (p.x, p.y, p.inventory.gold),
                _ => return,
            }
        };

        // Check if player is in an instance
        let instance_id = {
            let instances = self.player_instances.read().await;
            instances.get(player_id).cloned()
        };

        // Get NPC position and prototype ID - check instance NPCs first, then overworld
        let npc_info = if let Some(ref inst_id) = instance_id {
            if let Some(instance) = self.instance_manager.find_player_instance(player_id).await {
                let npcs = instance.npcs.read().await;
                npcs.get(npc_id).map(|npc| {
                    let dx = (npc.x - player_x) as f32;
                    let dy = (npc.y - player_y) as f32;
                    let distance = (dx * dx + dy * dy).sqrt();
                    (npc.prototype_id.clone(), distance, npc.is_alive())
                })
            } else {
                tracing::warn!("Player {} in instance {} but instance not found", player_id, inst_id);
                None
            }
        } else {
            let npcs = self.npcs.read().await;
            npcs.get(npc_id).map(|npc| {
                let dx = (npc.x - player_x) as f32;
                let dy = (npc.y - player_y) as f32;
                let distance = (dx * dx + dy * dy).sqrt();
                (npc.prototype_id.clone(), distance, npc.is_alive())
            })
        };

        let (prototype_id, distance, is_alive) = match npc_info {
            Some(info) => info,
            None => {
                self.send_shop_result(player_id, false, "buy", item_id, 0, 0, Some("NPC not found")).await;
                return;
            }
        };

        // Check distance (must be within 10 tiles — generous to allow for NPC wandering)
        if distance > 10.0 || !is_alive {
            self.send_shop_result(player_id, false, "buy", item_id, 0, 0, Some("Too far from merchant")).await;
            return;
        }

        // Get prototype and merchant config
        let merchant_config = match self.entity_registry.get(&prototype_id) {
            Some(proto) => match &proto.merchant {
                Some(config) => config.clone(),
                None => {
                    self.send_shop_result(player_id, false, "buy", item_id, 0, 0, Some("Not a merchant")).await;
                    return;
                }
            },
            None => {
                self.send_shop_result(player_id, false, "buy", item_id, 0, 0, Some("Invalid merchant")).await;
                return;
            }
        };

        // Get shop definition and check stock
        let mut shop_registry = self.shop_registry.write().await;
        let shop = match shop_registry.get_mut(&merchant_config.shop_id) {
            Some(s) => s,
            None => {
                drop(shop_registry);
                self.send_shop_result(player_id, false, "buy", item_id, 0, 0, Some("Shop not found")).await;
                return;
            }
        };

        // Check if item is in stock
        let stock_item = match shop.get_stock_mut(item_id) {
            Some(s) => s,
            None => {
                drop(shop_registry);
                self.send_shop_result(player_id, false, "buy", item_id, 0, 0, Some("Item not sold here")).await;
                return;
            }
        };

        // Check stock quantity
        if stock_item.current_quantity < quantity {
            drop(shop_registry);
            self.send_shop_result(player_id, false, "buy", item_id, 0, 0, Some("Insufficient stock")).await;
            return;
        }

        // Get item definition for base price
        let item_def = match self.item_registry.get(item_id) {
            Some(def) => def.clone(),
            None => {
                drop(shop_registry);
                self.send_shop_result(player_id, false, "buy", item_id, 0, 0, Some("Item not found")).await;
                return;
            }
        };

        // Calculate total cost
        let base_price = item_def.base_price;
        let unit_price = (base_price as f32 * merchant_config.sell_multiplier).max(1.0) as i32;
        let total_cost = unit_price * quantity;

        // Check if player has enough gold
        if player_gold < total_cost {
            drop(shop_registry);
            self.send_shop_result(player_id, false, "buy", item_id, 0, 0, Some("Not enough gold")).await;
            return;
        }

        // Check if player has inventory space
        {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => p,
                _ => {
                    drop(shop_registry);
                    return;
                }
            };

            if !player.inventory.has_space_for(item_id, quantity, &self.item_registry) {
                drop(shop_registry);
                self.send_shop_result(player_id, false, "buy", item_id, 0, 0, Some("Inventory full")).await;
                return;
            }
        }

        // All checks passed - process transaction
        stock_item.current_quantity -= quantity;
        let new_stock = stock_item.current_quantity;
        drop(shop_registry);

        // Update player inventory
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.inventory.gold -= total_cost;
                player.inventory.add_item(item_id, quantity, &self.item_registry);

                let inventory_update = player.inventory.to_update();
                let gold = player.inventory.gold;
                drop(players);

                // Send success result
                self.send_shop_result(player_id, true, "buy", item_id, quantity, -total_cost, None).await;

                // Send inventory update
                self.send_to_player(
                    player_id,
                    ServerMessage::InventoryUpdate {
                        player_id: player_id.to_string(),
                        slots: inventory_update,
                        gold,
                    },
                )
                .await;

                // Broadcast stock update to nearby players
                self.broadcast_shop_stock_update(npc_id, item_id, new_stock).await;

                tracing::info!(
                    "Player {} bought {}x{} from {} for {} gold",
                    player_id, quantity, item_id, npc_id, total_cost
                );
            }
        }
    }

    /// Send shop result message to player
    async fn send_shop_result(
        &self,
        player_id: &str,
        success: bool,
        action: &str,
        item_id: &str,
        quantity: i32,
        gold_change: i32,
        error: Option<&str>,
    ) {
        self.send_to_player(
            player_id,
            ServerMessage::ShopResult {
                success,
                action: action.to_string(),
                item_id: item_id.to_string(),
                quantity,
                gold_change,
                error: error.map(|s| s.to_string()),
            },
        )
        .await;
    }

    /// Broadcast shop stock update to all players
    async fn broadcast_shop_stock_update(&self, npc_id: &str, item_id: &str, new_quantity: i32) {
        self.broadcast(ServerMessage::ShopStockUpdate {
            npc_id: npc_id.to_string(),
            item_id: item_id.to_string(),
            new_quantity,
        })
        .await;
    }

    /// Handle shop sell transaction
    pub async fn handle_shop_sell(&self, player_id: &str, npc_id: &str, item_id: &str, quantity: i32) {
        // Validate quantity
        if quantity <= 0 {
            self.send_shop_result(player_id, false, "sell", item_id, 0, 0, Some("Invalid quantity")).await;
            return;
        }

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

        // Get NPC position and prototype ID - check instance NPCs first, then overworld
        let npc_info = if let Some(ref inst_id) = instance_id {
            if let Some(instance) = self.instance_manager.find_player_instance(player_id).await {
                let npcs = instance.npcs.read().await;
                npcs.get(npc_id).map(|npc| {
                    let dx = (npc.x - player_x) as f32;
                    let dy = (npc.y - player_y) as f32;
                    let distance = (dx * dx + dy * dy).sqrt();
                    (npc.prototype_id.clone(), distance, npc.is_alive())
                })
            } else {
                tracing::warn!("Player {} in instance {} but instance not found", player_id, inst_id);
                None
            }
        } else {
            let npcs = self.npcs.read().await;
            npcs.get(npc_id).map(|npc| {
                let dx = (npc.x - player_x) as f32;
                let dy = (npc.y - player_y) as f32;
                let distance = (dx * dx + dy * dy).sqrt();
                (npc.prototype_id.clone(), distance, npc.is_alive())
            })
        };

        let (prototype_id, distance, is_alive) = match npc_info {
            Some(info) => info,
            None => {
                self.send_shop_result(player_id, false, "sell", item_id, 0, 0, Some("NPC not found")).await;
                return;
            }
        };

        // Check distance (must be within 10 tiles — generous to allow for NPC wandering)
        if distance > 10.0 || !is_alive {
            self.send_shop_result(player_id, false, "sell", item_id, 0, 0, Some("Too far from merchant")).await;
            return;
        }

        // Get prototype and merchant config
        let merchant_config = match self.entity_registry.get(&prototype_id) {
            Some(proto) => match &proto.merchant {
                Some(config) => config.clone(),
                None => {
                    self.send_shop_result(player_id, false, "sell", item_id, 0, 0, Some("Not a merchant")).await;
                    return;
                }
            },
            None => {
                self.send_shop_result(player_id, false, "sell", item_id, 0, 0, Some("Invalid merchant")).await;
                return;
            }
        };

        // Get item definition
        let item_def = match self.item_registry.get(item_id) {
            Some(def) => def.clone(),
            None => {
                self.send_shop_result(player_id, false, "sell", item_id, 0, 0, Some("Item not found")).await;
                return;
            }
        };

        // Check if item is sellable
        if !item_def.sellable {
            self.send_shop_result(player_id, false, "sell", item_id, 0, 0, Some("Item cannot be sold")).await;
            return;
        }

        // Check if player has the item
        let has_item = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => p.inventory.has_item(item_id, quantity),
                _ => false,
            }
        };

        if !has_item {
            self.send_shop_result(player_id, false, "sell", item_id, 0, 0, Some("You don't have enough of that item")).await;
            return;
        }

        // Calculate sell price
        let base_price = item_def.base_price;
        let unit_price = (base_price as f32 * merchant_config.buy_multiplier).max(1.0) as i32;
        let total_value = unit_price * quantity;

        // Process transaction
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                // Remove items from inventory
                player.inventory.remove_item(item_id, quantity);
                // Add gold
                player.inventory.gold += total_value;

                let inventory_update = player.inventory.to_update();
                let gold = player.inventory.gold;
                drop(players);

                // Send success result
                self.send_shop_result(player_id, true, "sell", item_id, quantity, total_value, None).await;

                // Send inventory update
                self.send_to_player(
                    player_id,
                    ServerMessage::InventoryUpdate {
                        player_id: player_id.to_string(),
                        slots: inventory_update,
                        gold,
                    },
                )
                .await;

                tracing::info!(
                    "Player {} sold {}x{} to {} for {} gold",
                    player_id, quantity, item_id, npc_id, total_value
                );
            }
        }
    }

    /// Handle equipping an item from inventory
    pub async fn handle_equip(&self, player_id: &str, slot_index: u8) {
        use crate::data::item_def::EquipmentSlot;

        let slot_idx = slot_index as usize;

        // Get item info from inventory slot
        let item_info = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => p,
                _ => return,
            };

            match player.inventory.slots.get(slot_idx) {
                Some(Some(slot)) => Some((
                    slot.item_id.clone(),
                    player.skills.combat.level,
                    player.equipped_head.clone(),
                    player.equipped_body.clone(),
                    player.equipped_weapon.clone(),
                    player.equipped_back.clone(),
                    player.equipped_feet.clone(),
                    player.equipped_ring.clone(),
                    player.equipped_gloves.clone(),
                    player.equipped_necklace.clone(),
                    player.equipped_belt.clone(),
                )),
                _ => None,
            }
        };

        let (item_id, combat_level, equipped_head, equipped_body, equipped_weapon, equipped_back, equipped_feet, equipped_ring, equipped_gloves, equipped_necklace, equipped_belt) = match item_info {
            Some(info) => info,
            None => {
                self.send_to_player(player_id, ServerMessage::EquipResult {
                    success: false,
                    slot_type: "unknown".to_string(),
                    item_id: None,
                    error: Some("No item in that slot".to_string()),
                }).await;
                return;
            }
        };

        // Get item definition
        let item_def = match self.item_registry.get(&item_id) {
            Some(def) => def,
            None => {
                self.send_to_player(player_id, ServerMessage::EquipResult {
                    success: false,
                    slot_type: "unknown".to_string(),
                    item_id: None,
                    error: Some("Item not found".to_string()),
                }).await;
                return;
            }
        };

        // Check if item is equippable and get its slot type
        let (equip_stats, equip_slot) = match &item_def.equipment {
            Some(stats) if stats.slot_type != EquipmentSlot::None => (stats, stats.slot_type),
            _ => {
                self.send_to_player(player_id, ServerMessage::EquipResult {
                    success: false,
                    slot_type: "unknown".to_string(),
                    item_id: None,
                    error: Some("Item cannot be equipped".to_string()),
                }).await;
                return;
            }
        };

        let slot_type_str = equip_slot.as_str().to_string();

        // Check skill level requirements
        // Both attack and defence requirements are checked against the unified combat level
        let level_required = equip_stats.attack_level_required.max(equip_stats.defence_level_required);
        if level_required > 0 && combat_level < level_required {
            self.send_to_player(player_id, ServerMessage::EquipResult {
                success: false,
                slot_type: slot_type_str,
                item_id: None,
                error: Some(format!("Requires Combat level {}", level_required)),
            }).await;
            return;
        }

        // Get currently equipped item for this slot
        let currently_equipped = match equip_slot {
            EquipmentSlot::Head => equipped_head,
            EquipmentSlot::Body => equipped_body,
            EquipmentSlot::Weapon => equipped_weapon,
            EquipmentSlot::Back => equipped_back,
            EquipmentSlot::Feet => equipped_feet,
            EquipmentSlot::Ring => equipped_ring,
            EquipmentSlot::Gloves => equipped_gloves,
            EquipmentSlot::Necklace => equipped_necklace,
            EquipmentSlot::Belt => equipped_belt,
            EquipmentSlot::None => None,
        };

        // Perform the equip operation
        let (inventory_update, gold, new_equipment) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) => p,
                None => return,
            };

            // If something is equipped in this slot, swap it to the inventory slot
            if let Some(ref old_item_id) = currently_equipped {
                player.inventory.slots[slot_idx] = Some(item::InventorySlot::new(old_item_id.clone(), 1));
            } else {
                player.inventory.slots[slot_idx] = None;
            }

            // Equip the new item to the appropriate slot
            match equip_slot {
                EquipmentSlot::Head => player.equipped_head = Some(item_id.clone()),
                EquipmentSlot::Body => player.equipped_body = Some(item_id.clone()),
                EquipmentSlot::Weapon => player.equipped_weapon = Some(item_id.clone()),
                EquipmentSlot::Back => player.equipped_back = Some(item_id.clone()),
                EquipmentSlot::Feet => player.equipped_feet = Some(item_id.clone()),
                EquipmentSlot::Ring => player.equipped_ring = Some(item_id.clone()),
                EquipmentSlot::Gloves => player.equipped_gloves = Some(item_id.clone()),
                EquipmentSlot::Necklace => player.equipped_necklace = Some(item_id.clone()),
                EquipmentSlot::Belt => player.equipped_belt = Some(item_id.clone()),
                EquipmentSlot::None => {}
            }

            (
                player.inventory.to_update(),
                player.inventory.gold,
                (
                    player.equipped_head.clone(),
                    player.equipped_body.clone(),
                    player.equipped_weapon.clone(),
                    player.equipped_back.clone(),
                    player.equipped_feet.clone(),
                    player.equipped_ring.clone(),
                    player.equipped_gloves.clone(),
                    player.equipped_necklace.clone(),
                    player.equipped_belt.clone(),
                ),
            )
        };

        tracing::info!("Player {} equipped {} to {} slot", player_id, item_id, slot_type_str);

        // If equipping a non-fishing-rod weapon while gathering, stop gathering
        if equip_slot == EquipmentSlot::Weapon && item_id != "fishing_rod" {
            let is_gathering = {
                let gathering = self.gathering.read().await;
                gathering.is_gathering(player_id)
            };
            if is_gathering {
                self.handle_stop_gathering(player_id).await;
            }
        }

        // Send success result
        self.send_to_player(player_id, ServerMessage::EquipResult {
            success: true,
            slot_type: slot_type_str,
            item_id: Some(item_id.clone()),
            error: None,
        }).await;

        // Send inventory update
        self.send_to_player(player_id, ServerMessage::InventoryUpdate {
            player_id: player_id.to_string(),
            slots: inventory_update,
            gold,
        }).await;

        // Broadcast equipment update to all players
        self.broadcast(ServerMessage::EquipmentUpdate {
            player_id: player_id.to_string(),
            equipped_head: new_equipment.0,
            equipped_body: new_equipment.1,
            equipped_weapon: new_equipment.2,
            equipped_back: new_equipment.3,
            equipped_feet: new_equipment.4,
            equipped_ring: new_equipment.5,
            equipped_gloves: new_equipment.6,
            equipped_necklace: new_equipment.7,
            equipped_belt: new_equipment.8,
        }).await;
    }

    /// Handle unequipping an item
    pub async fn handle_unequip(&self, player_id: &str, slot_type: &str) {
        // Validate slot type
        let valid_slots = ["head", "body", "weapon", "back", "feet", "ring", "gloves", "necklace", "belt"];
        if !valid_slots.contains(&slot_type) {
            self.send_to_player(player_id, ServerMessage::EquipResult {
                success: false,
                slot_type: slot_type.to_string(),
                item_id: None,
                error: Some("Unknown equipment slot".to_string()),
            }).await;
            return;
        }

        // Check if something is equipped and if inventory has space
        let equipped_item = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => p,
                _ => return,
            };

            let equipped_ref = match slot_type {
                "head" => &player.equipped_head,
                "body" => &player.equipped_body,
                "weapon" => &player.equipped_weapon,
                "back" => &player.equipped_back,
                "feet" => &player.equipped_feet,
                "ring" => &player.equipped_ring,
                "gloves" => &player.equipped_gloves,
                "necklace" => &player.equipped_necklace,
                "belt" => &player.equipped_belt,
                _ => return,
            };

            match equipped_ref {
                Some(item_id) => {
                    // Check if inventory has space
                    if !player.inventory.has_space_for(item_id, 1, &self.item_registry) {
                        self.send_to_player(player_id, ServerMessage::EquipResult {
                            success: false,
                            slot_type: slot_type.to_string(),
                            item_id: None,
                            error: Some("Inventory full".to_string()),
                        }).await;
                        return;
                    }
                    Some(item_id.clone())
                }
                None => {
                    self.send_to_player(player_id, ServerMessage::EquipResult {
                        success: false,
                        slot_type: slot_type.to_string(),
                        item_id: None,
                        error: Some("Nothing equipped".to_string()),
                    }).await;
                    return;
                }
            }
        };

        let item_id = match equipped_item {
            Some(id) => id,
            None => return,
        };

        // Perform the unequip operation
        let (inventory_update, gold, new_equipment) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) => p,
                None => return,
            };

            // Clear equipped item from appropriate slot
            match slot_type {
                "head" => player.equipped_head = None,
                "body" => player.equipped_body = None,
                "weapon" => player.equipped_weapon = None,
                "back" => player.equipped_back = None,
                "feet" => player.equipped_feet = None,
                "ring" => player.equipped_ring = None,
                "gloves" => player.equipped_gloves = None,
                "necklace" => player.equipped_necklace = None,
                "belt" => player.equipped_belt = None,
                _ => {}
            }

            // Add to inventory
            player.inventory.add_item(&item_id, 1, &self.item_registry);

            (
                player.inventory.to_update(),
                player.inventory.gold,
                (
                    player.equipped_head.clone(),
                    player.equipped_body.clone(),
                    player.equipped_weapon.clone(),
                    player.equipped_back.clone(),
                    player.equipped_feet.clone(),
                    player.equipped_ring.clone(),
                    player.equipped_gloves.clone(),
                    player.equipped_necklace.clone(),
                    player.equipped_belt.clone(),
                ),
            )
        };

        tracing::info!("Player {} unequipped {} from {} slot", player_id, item_id, slot_type);

        // If unequipping weapon while gathering, stop gathering
        if slot_type == "weapon" {
            let is_gathering = {
                let gathering = self.gathering.read().await;
                gathering.is_gathering(player_id)
            };
            if is_gathering {
                self.handle_stop_gathering(player_id).await;
            }
        }

        // Send success result
        self.send_to_player(player_id, ServerMessage::EquipResult {
            success: true,
            slot_type: slot_type.to_string(),
            item_id: Some(item_id),
            error: None,
        }).await;

        // Send inventory update
        self.send_to_player(player_id, ServerMessage::InventoryUpdate {
            player_id: player_id.to_string(),
            slots: inventory_update,
            gold,
        }).await;

        // Broadcast equipment update to all players
        self.broadcast(ServerMessage::EquipmentUpdate {
            player_id: player_id.to_string(),
            equipped_head: new_equipment.0,
            equipped_body: new_equipment.1,
            equipped_weapon: new_equipment.2,
            equipped_back: new_equipment.3,
            equipped_feet: new_equipment.4,
            equipped_ring: new_equipment.5,
            equipped_gloves: new_equipment.6,
            equipped_necklace: new_equipment.7,
            equipped_belt: new_equipment.8,
        }).await;
    }

    /// Handle dropping an item from inventory to the ground
    /// If target_x/target_y provided, drop at that tile (must be adjacent to player)
    pub async fn handle_drop_item(&self, player_id: &str, slot_index: u8, quantity: u32, target_x: Option<i32>, target_y: Option<i32>) {
        let slot_idx = slot_index as usize;

        // Get player position and item info
        let drop_info: Option<(i32, i32, String, i32)> = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => p,
                _ => return,
            };

            // Determine drop location
            let (drop_x, drop_y) = if let (Some(tx), Some(ty)) = (target_x, target_y) {
                // Validate target is adjacent to player (Chebyshev distance <= 1)
                let dx = (tx - player.x).abs();
                let dy = (ty - player.y).abs();
                if dx <= 1 && dy <= 1 {
                    (tx, ty)
                } else {
                    // Target too far, drop at player position
                    (player.x, player.y)
                }
            } else {
                // No target specified, drop at player position
                (player.x, player.y)
            };

            match player.inventory.slots.get(slot_idx) {
                Some(Some(slot)) => {
                    let qty_to_drop = (quantity as i32).min(slot.quantity);
                    if qty_to_drop <= 0 {
                        return;
                    }
                    Some((drop_x, drop_y, slot.item_id.clone(), qty_to_drop))
                }
                _ => None,
            }
        };

        let (drop_x, drop_y, item_id, qty_to_drop) = match drop_info {
            Some(info) => info,
            None => return,
        };

        // Remove item from inventory (manipulate slot directly)
        let (inventory_update, gold) = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                if let Some(slot) = &mut player.inventory.slots[slot_idx] {
                    slot.quantity -= qty_to_drop;
                    if slot.quantity <= 0 {
                        player.inventory.slots[slot_idx] = None;
                    }
                }
                (player.inventory.to_update(), player.inventory.gold)
            } else {
                return;
            }
        };

        // Create ground item with owner protection
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let drop_x_f = drop_x as f32;
        let drop_y_f = drop_y as f32;

        // Get player's current instance
        let instance_id = {
            let instances = self.player_instances.read().await;
            instances.get(player_id).cloned()
        };

        let ground_item = GroundItem::new_in_instance(
            &uuid::Uuid::new_v4().to_string(),
            &item_id,
            drop_x_f,
            drop_y_f,
            qty_to_drop,
            Some(player_id.to_string()),
            current_time,
            instance_id,
        );

        tracing::info!("Player {} dropped {}x {} (protected for 10s)", player_id, qty_to_drop, item_id);

        // Broadcast item drop to players in same zone
        self.broadcast_to_zone(player_id, ServerMessage::ItemDropped {
            id: ground_item.id.clone(),
            item_id: item_id.clone(),
            x: drop_x_f,
            y: drop_y_f,
            quantity: qty_to_drop,
        }).await;

        // Store in ground_items
        {
            let mut items = self.ground_items.write().await;
            items.insert(ground_item.id.clone(), ground_item);
        }

        // Send inventory update to dropping player
        self.send_to_player(player_id, ServerMessage::InventoryUpdate {
            player_id: player_id.to_string(),
            slots: inventory_update,
            gold,
        }).await;
    }

    /// Handle dropping gold to the ground
    pub async fn handle_drop_gold(&self, player_id: &str, amount: i32) {
        // Validate amount
        if amount <= 0 {
            return;
        }

        // Get player position and validate gold amount
        let drop_info: Option<(i32, i32, i32)> = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => p,
                _ => return,
            };

            if amount > player.inventory.gold {
                return;
            }

            Some((player.x, player.y, player.inventory.gold))
        };

        let (player_x, player_y, _current_gold) = match drop_info {
            Some(info) => info,
            None => return,
        };

        // Deduct gold from inventory
        let (inventory_update, new_gold) = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.inventory.gold -= amount;
                (player.inventory.to_update(), player.inventory.gold)
            } else {
                return;
            }
        };

        // Create ground item with owner protection
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let drop_x = player_x as f32;
        let drop_y = player_y as f32;

        // Get player's current instance
        let instance_id = {
            let instances = self.player_instances.read().await;
            instances.get(player_id).cloned()
        };

        let ground_item = GroundItem::new_in_instance(
            &uuid::Uuid::new_v4().to_string(),
            GOLD_ITEM_ID,
            drop_x,
            drop_y,
            amount,
            Some(player_id.to_string()),
            current_time,
            instance_id,
        );

        tracing::info!("Player {} dropped {}g (protected for 10s)", player_id, amount);

        // Broadcast item drop to players in same zone
        self.broadcast_to_zone(player_id, ServerMessage::ItemDropped {
            id: ground_item.id.clone(),
            item_id: GOLD_ITEM_ID.to_string(),
            x: drop_x,
            y: drop_y,
            quantity: amount,
        }).await;

        // Store in ground_items
        {
            let mut items = self.ground_items.write().await;
            items.insert(ground_item.id.clone(), ground_item);
        }

        // Send inventory update to dropping player
        self.send_to_player(player_id, ServerMessage::InventoryUpdate {
            player_id: player_id.to_string(),
            slots: inventory_update,
            gold: new_gold,
        }).await;
    }

    /// Swap two inventory slots
    pub async fn handle_swap_slots(&self, player_id: &str, from_slot: u8, to_slot: u8) {
        let from_idx = from_slot as usize;
        let to_idx = to_slot as usize;

        // Validate slot indices
        if from_idx >= 20 || to_idx >= 20 || from_idx == to_idx {
            return;
        }

        // Perform the swap and get inventory update
        let (inventory_update, gold) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) if p.active && !p.is_dead => p,
                _ => return,
            };

            // Swap the slots
            player.inventory.slots.swap(from_idx, to_idx);

            (player.inventory.to_update(), player.inventory.gold)
        };

        tracing::debug!("Player {} swapped slots {} <-> {}", player_id, from_slot, to_slot);

        // Send inventory update to the player
        self.send_to_player(player_id, ServerMessage::InventoryUpdate {
            player_id: player_id.to_string(),
            slots: inventory_update,
            gold,
        }).await;
    }

    pub async fn get_gathering_markers_message(&self) -> ServerMessage {
        let gathering = self.gathering.read().await;
        let markers = gathering.markers.iter().map(|m| {
            let skill = gathering.zones.get(&m.zone_id)
                .map(|z| z.skill.clone())
                .unwrap_or_else(|| "unknown".to_string());
            crate::protocol::GatheringMarkerData {
                x: m.x,
                y: m.y,
                zone_id: m.zone_id.clone(),
                skill,
            }
        }).collect();
        ServerMessage::GatheringMarkers { markers }
    }

    pub async fn handle_start_gathering(&self, player_id: &str, marker_x: i32, marker_y: i32) {
        // Gathering zones are overworld-only; reject if player is in an instance
        if self.player_instances.read().await.contains_key(player_id) {
            return;
        }

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let (fishing_level, player_x, player_y, player_dir, equipped_weapon) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) => (p.skills.fishing.level, p.x, p.y, p.direction, p.equipped_weapon.clone()),
                None => return,
            }
        };

        // Check player is facing the gathering marker
        let (fdx, fdy): (i32, i32) = match player_dir {
            Direction::Down => (0, 1),
            Direction::Up => (0, -1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
            Direction::DownLeft => (-1, 1),
            Direction::DownRight => (1, 1),
            Direction::UpLeft => (-1, -1),
            Direction::UpRight => (1, -1),
        };
        if player_x + fdx != marker_x || player_y + fdy != marker_y {
            self.send_to_player(player_id, ServerMessage::Error {
                code: 400,
                message: "You must face the gathering spot".to_string(),
            }).await;
            return;
        }

        // Check equipment requirements before starting
        {
            let gathering = self.gathering.read().await;
            if let Some(zone) = gathering.get_zone_for_marker(marker_x, marker_y) {
                let required_weapon = match zone.skill.as_str() {
                    "fishing" => Some("fishing_rod"),
                    _ => None,
                };
                if let Some(required) = required_weapon {
                    if equipped_weapon.as_deref() != Some(required) {
                        drop(gathering);
                        self.send_to_player(player_id, ServerMessage::Error {
                            code: 400,
                            message: format!("You need a {} to do that", required.replace('_', " ")),
                        }).await;
                        return;
                    }
                }
            }
        }

        let mut gathering = self.gathering.write().await;
        match gathering.start_gathering(player_id, marker_x, marker_y, fishing_level, current_time) {
            Ok(zone_id) => {
                self.broadcast(ServerMessage::GatheringStarted {
                    player_id: player_id.to_string(),
                    marker_x,
                    marker_y,
                    zone_id,
                }).await;
            }
            Err(msg) => {
                self.send_to_player(player_id, ServerMessage::Error {
                    code: 400,
                    message: msg,
                }).await;
            }
        }
    }

    pub async fn handle_stop_gathering(&self, player_id: &str) {
        let mut gathering = self.gathering.write().await;
        if gathering.stop_gathering(player_id).is_some() {
            self.broadcast(ServerMessage::GatheringStopped {
                player_id: player_id.to_string(),
                reason: "cancelled".to_string(),
            }).await;
        }
    }

    // ========================================================================
    // Woodcutting System
    // ========================================================================

    /// Handle a single chop attempt on a tree (player presses attack while facing tree with axe)
    pub async fn handle_chop_tree(&self, player_id: &str, tree_x: i32, tree_y: i32, tree_gid: u32) {
        // Woodcutting is overworld-only; reject if player is in an instance
        if self.player_instances.read().await.contains_key(player_id) {
            return;
        }

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let (woodcutting_level, player_x, player_y, player_dir, equipped_weapon) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) => (p.skills.woodcutting.level, p.x, p.y, p.direction, p.equipped_weapon.clone()),
                None => return,
            }
        };

        // Check player is adjacent to the tree (within 1 tile)
        let dx = (player_x - tree_x).abs();
        let dy = (player_y - tree_y).abs();
        if dx > 1 || dy > 1 || (dx == 0 && dy == 0) {
            self.send_to_player(player_id, ServerMessage::Error {
                code: 400,
                message: "You need to be next to the tree".to_string(),
            }).await;
            return;
        }

        // Check if player has an axe equipped with sufficient level
        let has_valid_axe = if let Some(ref weapon_id) = equipped_weapon {
            if let Some(item_def) = self.item_registry.get(weapon_id) {
                if let Some(ref equip) = item_def.equipment {
                    if equip.chop_speed_multiplier > 0.0 {
                        if equip.woodcutting_level_required > woodcutting_level {
                            self.send_to_player(player_id, ServerMessage::Error {
                                code: 400,
                                message: format!(
                                    "You need Woodcutting level {} to use this axe",
                                    equip.woodcutting_level_required
                                ),
                            }).await;
                            return;
                        }
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if !has_valid_axe {
            self.send_to_player(player_id, ServerMessage::Error {
                code: 400,
                message: "You need an axe to chop trees".to_string(),
            }).await;
            return;
        }

        // Perform the chop
        let mut woodcutting = self.woodcutting.write().await;
        let chop_result = woodcutting.chop_once(tree_x, tree_y, tree_gid, woodcutting_level, current_time);
        drop(woodcutting);

        match chop_result {
            Ok(result) => {
                // Broadcast the swing animation to all players
                self.broadcast(ServerMessage::WoodcuttingSwing {
                    player_id: player_id.to_string(),
                    tree_x,
                    tree_y,
                }).await;

                if result.success {
                    // Add log to inventory
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        let leftover = player.inventory.add_item(&result.log_item_id, 1, &self.item_registry);
                        if leftover > 0 {
                            drop(players);
                            self.send_to_player(player_id, ServerMessage::Error {
                                code: 400,
                                message: "Your inventory is full!".to_string(),
                            }).await;
                            return;
                        }

                        // Award XP
                        let leveled_up = player.skills.woodcutting.add_xp(result.xp_gained);
                        let new_xp = player.skills.woodcutting.xp;
                        let new_level = player.skills.woodcutting.level;
                        let inv_update = player.inventory.to_update();
                        let gold = player.inventory.gold;
                        drop(players);

                        // Send inventory update
                        self.send_to_player(player_id, ServerMessage::InventoryUpdate {
                            player_id: player_id.to_string(),
                            slots: inv_update,
                            gold,
                        }).await;

                        // Send XP update
                        self.send_to_player(player_id, ServerMessage::SkillXp {
                            player_id: player_id.to_string(),
                            skill: "woodcutting".to_string(),
                            xp_gained: result.xp_gained,
                            total_xp: new_xp,
                            level: new_level,
                        }).await;

                        // Send chop result
                        self.send_to_player(player_id, ServerMessage::WoodcuttingResult {
                            player_id: player_id.to_string(),
                            item_id: result.log_item_id.clone(),
                            xp_gained: result.xp_gained,
                        }).await;

                        // Handle level up
                        if leveled_up {
                            self.broadcast(ServerMessage::SkillLevelUp {
                                player_id: player_id.to_string(),
                                skill: "woodcutting".to_string(),
                                new_level,
                            }).await;
                        }
                    }
                }

                // Handle tree depletion
                if result.tree_depleted {
                    let respawn_delay = result.respawn_delay_ms.unwrap_or(7500);
                    self.broadcast(ServerMessage::TreeDepleted {
                        x: tree_x,
                        y: tree_y,
                        gid: tree_gid,
                        respawn_delay_ms: respawn_delay,
                    }).await;

                    // Process quest event for tree depletion
                    self.process_quest_tree_deplete(player_id, &result.tree_type_id, tree_x, tree_y).await;
                }
            }
            Err(msg) => {
                self.send_to_player(player_id, ServerMessage::Error {
                    code: 400,
                    message: msg,
                }).await;
            }
        }
    }

    // ========================================================================
    // Chair System
    // ========================================================================

    pub async fn get_chair_positions_message(&self) -> ServerMessage {
        let chairs = self.chairs.read().await;
        let positions: Vec<(i32, i32)> = chairs.keys().cloned().collect();
        ServerMessage::ChairPositions { positions }
    }

    pub async fn handle_sit_chair(&self, player_id: &str, tile_x: i32, tile_y: i32) {
        // Chairs are overworld-only; reject if player is in an instance
        if self.player_instances.read().await.contains_key(player_id) {
            return;
        }

        // Validate chair exists and is unoccupied
        {
            let chairs = self.chairs.read().await;
            let chair = match chairs.get(&(tile_x, tile_y)) {
                Some(c) => c,
                None => {
                    self.send_to_player(player_id, ServerMessage::Error {
                        code: 400,
                        message: "No chair at that position".to_string(),
                    }).await;
                    return;
                }
            };
            if chair.occupied_by.is_some() {
                self.send_to_player(player_id, ServerMessage::Error {
                    code: 400,
                    message: "Chair is occupied".to_string(),
                }).await;
                return;
            }
        }

        // Check player position - must be directly behind the chair (opposite of facing direction)
        let (player_x, player_y, already_sitting) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) => (p.x, p.y, p.sitting_at.is_some()),
                None => return,
            }
        };

        if already_sitting {
            return;
        }

        // Get chair direction to validate approach
        let chair_dir = {
            let chairs = self.chairs.read().await;
            match chairs.get(&(tile_x, tile_y)) {
                Some(c) => c.direction,
                None => return,
            }
        };

        // Player must be at the tile in front of the chair (in the facing direction)
        let (expected_x, expected_y) = match chair_dir {
            Direction::Down => (tile_x, tile_y + 1),
            Direction::Up => (tile_x, tile_y - 1),
            Direction::Left => (tile_x - 1, tile_y),
            Direction::Right => (tile_x + 1, tile_y),
            _ => (tile_x, tile_y + 1), // fallback for diagonal dirs
        };

        if player_x != expected_x || player_y != expected_y {
            self.send_to_player(player_id, ServerMessage::Error {
                code: 400,
                message: "Must approach chair from the front".to_string(),
            }).await;
            return;
        }

        // Sit down
        let direction = {
            let mut chairs = self.chairs.write().await;
            if let Some(chair) = chairs.get_mut(&(tile_x, tile_y)) {
                // Double-check not occupied (race condition guard)
                if chair.occupied_by.is_some() {
                    return;
                }
                chair.occupied_by = Some(player_id.to_string());
                chair.direction
            } else {
                return;
            }
        };

        // Cancel any active gathering (fishing, etc.)
        self.handle_stop_gathering(player_id).await;

        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.sitting_at = Some((tile_x, tile_y));
                player.x = tile_x;
                player.y = tile_y;
                player.direction = direction;
                player.move_dx = 0;
                player.move_dy = 0;
            }
        }

        self.send_to_player(player_id, ServerMessage::SitResult {
            success: true,
            tile_x,
            tile_y,
            direction: direction as u8,
        }).await;
    }

    pub async fn handle_stand_up(&self, player_id: &str) {
        let sitting_at = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) => p.sitting_at,
                None => return,
            }
        };

        if let Some((tx, ty)) = sitting_at {
            // Get the chair's facing direction
            let chair_direction = {
                let chairs = self.chairs.read().await;
                chairs.get(&(tx, ty)).map(|c| c.direction)
            };

            // Free the chair
            {
                let mut chairs = self.chairs.write().await;
                if let Some(chair) = chairs.get_mut(&(tx, ty)) {
                    if chair.occupied_by.as_deref() == Some(player_id) {
                        chair.occupied_by = None;
                    }
                }
            }
            // Clear player sitting state and move to tile in front of chair
            {
                let mut players = self.players.write().await;
                if let Some(player) = players.get_mut(player_id) {
                    player.sitting_at = None;
                    if let Some(dir) = chair_direction {
                        let (dx, dy) = match dir {
                            Direction::Up => (0, -1),
                            Direction::Down => (0, 1),
                            Direction::Left => (-1, 0),
                            Direction::Right => (1, 0),
                            _ => (0, 0),
                        };
                        player.x = tx + dx;
                        player.y = ty + dy;
                    }
                }
            }
        }
    }

    pub async fn handle_plant_seed(&self, player_id: &str, patch_id: &str, item_id: &str) {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Get player farming level and check they have the seed
        let farming_level = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) => p,
                None => return,
            };
            if !player.inventory.has_item(item_id, 1) {
                self.send_to_player(player_id, ServerMessage::Error {
                    code: 400,
                    message: "You don't have that seed".to_string(),
                }).await;
                return;
            }
            player.skills.farming.level
        };

        // Try to plant
        let result = {
            let mut farming = self.farming.write().await;
            farming.plant_seed(patch_id, item_id, player_id, farming_level, current_time)
        };

        match result {
            Ok((crop_id, xp)) => {
                // Consume the seed
                let inv_update = {
                    let mut players = self.players.write().await;
                    let player = players.get_mut(player_id).unwrap();
                    player.inventory.remove_item(item_id, 1);

                    // Grant planting XP
                    let leveled = player.skills.farming.add_xp(xp);
                    let skill = &player.skills.farming;
                    let gold = player.inventory.gold;

                    (player.inventory.to_update(), gold, skill.xp, skill.level, leveled)
                };

                // Send inventory update
                self.send_to_player(player_id, ServerMessage::InventoryUpdate {
                    player_id: player_id.to_string(),
                    slots: inv_update.0,
                    gold: inv_update.1,
                }).await;

                // Send XP gain
                self.send_to_player(player_id, ServerMessage::SkillXp {
                    player_id: player_id.to_string(),
                    skill: "farming".to_string(),
                    xp_gained: xp,
                    total_xp: inv_update.2,
                    level: inv_update.3,
                }).await;

                if inv_update.4 {
                    self.broadcast(ServerMessage::SkillLevelUp {
                        player_id: player_id.to_string(),
                        skill: "farming".to_string(),
                        new_level: inv_update.3,
                    }).await;
                }

                // Persist to database
                if let Some(ref db) = self.db {
                    if let Err(e) = db.save_farming_patch(patch_id, player_id, &crop_id, current_time).await {
                        tracing::warn!("Failed to save farming patch {}: {}", patch_id, e);
                    }
                }

                // Send patch update to this player only (per-player instanced)
                self.send_to_player(player_id, ServerMessage::PatchStateUpdate {
                    patch_id: patch_id.to_string(),
                    state: "growing".to_string(),
                    crop_id,
                    growth_stage: 0,
                    owner_id: player_id.to_string(),
                }).await;

                // Fire quest event for planting
                self.process_quest_item_collect(player_id, &format!("plant_{}", item_id), 1).await;
            }
            Err(e) => {
                self.send_to_player(player_id, ServerMessage::Error {
                    code: 400,
                    message: e,
                }).await;
            }
        }
    }

    pub async fn handle_harvest_crop(&self, player_id: &str, patch_id: &str) {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let result = {
            let mut farming = self.farming.write().await;
            farming.harvest_crop(patch_id, player_id, current_time)
        };

        match result {
            Ok(harvest) => {
                // Add produce + optional seed to inventory, grant XP
                let inv_update = {
                    let mut players = self.players.write().await;
                    let player = players.get_mut(player_id).unwrap();

                    player.inventory.add_item(&harvest.produce_item, harvest.amount, &self.item_registry);
                    if harvest.seed_returned {
                        player.inventory.add_item(&harvest.seed_item, 1, &self.item_registry);
                    }

                    let leveled = player.skills.farming.add_xp(harvest.xp_gained);
                    let skill = &player.skills.farming;
                    let gold = player.inventory.gold;

                    (player.inventory.to_update(), gold, skill.xp, skill.level, leveled)
                };

                // Send inventory update
                self.send_to_player(player_id, ServerMessage::InventoryUpdate {
                    player_id: player_id.to_string(),
                    slots: inv_update.0,
                    gold: inv_update.1,
                }).await;

                // Send XP gain
                self.send_to_player(player_id, ServerMessage::SkillXp {
                    player_id: player_id.to_string(),
                    skill: "farming".to_string(),
                    xp_gained: harvest.xp_gained,
                    total_xp: inv_update.2,
                    level: inv_update.3,
                }).await;

                if inv_update.4 {
                    self.broadcast(ServerMessage::SkillLevelUp {
                        player_id: player_id.to_string(),
                        skill: "farming".to_string(),
                        new_level: inv_update.3,
                    }).await;
                }

                // Send patch reset to this player only (per-player instanced)
                self.send_to_player(player_id, ServerMessage::PatchStateUpdate {
                    patch_id: patch_id.to_string(),
                    state: "empty".to_string(),
                    crop_id: String::new(),
                    growth_stage: 0,
                    owner_id: String::new(),
                }).await;

                // Delete from database
                if let Some(ref db) = self.db {
                    if let Err(e) = db.delete_farming_patch(patch_id, player_id).await {
                        tracing::warn!("Failed to delete farming patch {}: {}", patch_id, e);
                    }
                }

                // Fire quest event for harvesting
                self.process_quest_item_collect(player_id, &format!("harvest_{}", harvest.produce_item), harvest.amount).await;
            }
            Err(e) => {
                self.send_to_player(player_id, ServerMessage::Error {
                    code: 400,
                    message: e,
                }).await;
            }
        }
    }

    /// Show the plot purchase dialogue with available plots
    async fn show_plot_purchase_dialogue(&self, player_id: &str, npc_id: &str) {
        let npc_name = {
            let npcs = self.npcs.read().await;
            npcs.get(npc_id)
                .map(|n| self.entity_registry.get(&n.prototype_id)
                    .map(|p| p.display_name.clone())
                    .unwrap_or_else(|| "Master Farmer".to_string()))
                .unwrap_or_else(|| "Master Farmer".to_string())
        };

        // Get player's farming level and gold
        let (farming_level, gold) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) => (p.skills.farming.level, p.inventory.gold),
                None => return,
            }
        };

        // Get player's unlocked plots
        let farming = self.farming.read().await;
        let unlocked = farming.get_unlocked_plots(player_id);

        // Build dialogue choices
        let mut choices = Vec::new();
        for req in crate::farming::PLOT_REQUIREMENTS {
            let owned = unlocked.contains(&req.plot_id);
            if owned {
                choices.push(crate::protocol::DialogueChoice {
                    id: format!("owned_{}", req.plot_id),
                    text: format!("Plot {} (Owned)", req.plot_id),
                });
            } else if farming_level < req.farming_level {
                choices.push(crate::protocol::DialogueChoice {
                    id: format!("locked_{}", req.plot_id),
                    text: format!("Plot {} - {}gp (Requires Farming {})", req.plot_id, req.gold_cost, req.farming_level),
                });
            } else if gold < req.gold_cost {
                choices.push(crate::protocol::DialogueChoice {
                    id: format!("locked_{}", req.plot_id),
                    text: format!("Plot {} - {}gp (Not enough gold)", req.plot_id, req.gold_cost),
                });
            } else {
                choices.push(crate::protocol::DialogueChoice {
                    id: format!("unlock_{}", req.plot_id),
                    text: format!("Plot {} - {}gp", req.plot_id, req.gold_cost),
                });
            }
        }
        choices.push(crate::protocol::DialogueChoice {
            id: "nevermind".to_string(),
            text: "Go back".to_string(),
        });

        let text = format!(
            "Each allotment plot gives you 16 farming patches to grow crops.\n\nYour gold: {}gp | Farming level: {}",
            gold, farming_level
        );

        self.send_to_player(player_id, ServerMessage::ShowDialogue {
            quest_id: format!("plot_seller:{}", npc_id),
            npc_id: npc_id.to_string(),
            speaker: npc_name,
            text,
            choices,
        }).await;
    }

    /// Handle a player purchasing a farming plot
    async fn handle_plot_purchase(&self, player_id: &str, plot_id: u32) {
        // Find the plot requirement
        let req = match crate::farming::PLOT_REQUIREMENTS.iter().find(|r| r.plot_id == plot_id) {
            Some(r) => r,
            None => {
                self.send_system_message(player_id, "Invalid plot.").await;
                return;
            }
        };

        // Check not already unlocked
        {
            let farming = self.farming.read().await;
            if farming.is_plot_unlocked(player_id, plot_id) {
                self.send_system_message(player_id, "You already own this plot.").await;
                return;
            }
        }

        // Check farming level and gold
        let (farming_level, gold) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) => (p.skills.farming.level, p.inventory.gold),
                None => return,
            }
        };

        if farming_level < req.farming_level {
            self.send_system_message(player_id,
                &format!("You need Farming level {} to unlock this plot.", req.farming_level)
            ).await;
            return;
        }

        if gold < req.gold_cost {
            self.send_system_message(player_id,
                &format!("You need {}gp to unlock this plot. You have {}gp.", req.gold_cost, gold)
            ).await;
            return;
        }

        // Deduct gold
        let inv_update = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) => p,
                None => return,
            };
            player.inventory.gold -= req.gold_cost;
            (player.inventory.to_update(), player.inventory.gold)
        };

        // Send inventory update (gold changed)
        self.send_to_player(player_id, ServerMessage::InventoryUpdate {
            player_id: player_id.to_string(),
            slots: inv_update.0,
            gold: inv_update.1,
        }).await;

        // Unlock the plot
        {
            let mut farming = self.farming.write().await;
            farming.unlock_plot(player_id, plot_id);
        }

        // Persist to database
        if let Some(ref db) = self.db {
            if let Err(e) = db.save_plot_unlock(player_id, plot_id).await {
                tracing::error!("Failed to save plot unlock: {}", e);
            }
        }

        // Send success message
        self.send_system_message(player_id,
            &format!("You've unlocked Plot {}! 16 new allotment patches are now available.", plot_id)
        ).await;

        // Send updated farming patches (now includes newly unlocked plot)
        let patches_msg = self.get_farming_patches_message(player_id).await;
        self.send_to_player(player_id, patches_msg).await;
    }

    /// Get farming patch states message for a specific connecting client
    pub async fn get_farming_patches_message(&self, player_id: &str) -> ServerMessage {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let farming = self.farming.read().await;
        let updates = farming.get_player_patch_states(player_id, current_time);
        let patches = updates.into_iter().map(|u| {
            let patch = farming.patches.get(&u.patch_id).unwrap();
            crate::protocol::FarmingPatchData {
                patch_id: u.patch_id,
                x: patch.x,
                y: patch.y,
                state: u.state,
                crop_id: u.crop_id,
                growth_stage: u.growth_stage,
                owner_id: u.owner_id,
            }
        }).collect();
        let unlocked_plots = farming.get_unlocked_plots(player_id);

        // Build tile overrides for all farming patches (locked=65, unlocked=62)
        let tile_overrides = farming.patches.values().map(|patch| {
            let tile_id = if farming.is_plot_unlocked(player_id, patch.plot) { 62 } else { 65 };
            crate::protocol::TileOverride {
                x: patch.x,
                y: patch.y,
                tile_id,
            }
        }).collect();

        ServerMessage::FarmingPatchStates { patches, unlocked_plots, tile_overrides }
    }

    pub async fn tick(&self) {
        let tick_start = std::time::Instant::now();
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
            let active_coords: Vec<ChunkCoord> = {
                let chunks = self.player_chunks.read().await;
                chunks.values().cloned().collect()
            };
            if !active_coords.is_empty() {
                self.world.unload_distant_chunks(&active_coords, 5).await;
            }
        }

        // Handle player respawns
        let mut respawned_players = Vec::new();
        let mut chairs_to_free = Vec::new();
        {
            let mut players = self.players.write().await;
            for player in players.values_mut() {
                if !player.active {
                    continue;
                }

                if player.ready_to_respawn(current_time) {
                    if let Some(chair_coords) = player.respawn() {
                        chairs_to_free.push((player.id.clone(), chair_coords));
                    }
                    respawned_players.push((
                        player.id.clone(),
                        player.x,
                        player.y,
                        player.hp,
                        player.prayer_points,
                        player.max_prayer_points(),
                    ));
                }
            }
        }

        // Free any chairs that respawning players were sitting in
        if !chairs_to_free.is_empty() {
            let mut chairs = self.chairs.write().await;
            for (player_id, (tx, ty)) in chairs_to_free {
                if let Some(chair) = chairs.get_mut(&(tx, ty)) {
                    if chair.occupied_by.as_deref() == Some(&player_id) {
                        chair.occupied_by = None;
                    }
                }
            }
        }

        // Stop gathering for respawning players
        for (id, _, _, _, _, _) in &respawned_players {
            self.handle_stop_gathering(id).await;
        }

        // Broadcast respawns
        for (id, x, y, hp, prayer_points, max_prayer_points) in respawned_players {
            tracing::info!("Player {} respawned at ({}, {})", id, x, y);
            self.broadcast(ServerMessage::PlayerRespawned { id: id.clone(), x, y, hp }).await;

            // Send prayer state update with restored prayer points
            self.send_to_player(&id, ServerMessage::PrayerStateUpdate {
                points: prayer_points,
                max_points: max_prayer_points,
                active_prayers: vec![],  // No active prayers after respawn
            }).await;
        }

        // Collect pending moves (id, target_x, target_y)
        // Use tick-based cooldown for deterministic timing (5 ticks = 250ms)
        let pending_moves: Vec<(String, i32, i32, Direction)> = {
            let players = self.players.read().await;
            players.values()
                .filter(|p| p.active && !p.is_dead)
                .filter(|p| p.move_dx != 0 || p.move_dy != 0)
                .filter(|p| current_tick - p.last_move_tick >= MOVE_COOLDOWN_TICKS)
                .map(|p| (p.id.clone(), p.x + p.move_dx, p.y + p.move_dy, p.direction))
                .collect()
        };
        // Track all players with pending moves so we can clear rejected ones
        let pending_player_ids: Vec<String> = pending_moves.iter().map(|(id, _, _, _)| id.clone()).collect();

        // Collect current entity positions for collision checking
        let player_positions: std::collections::HashSet<(i32, i32)> = {
            let players = self.players.read().await;
            players.values()
                .filter(|p| p.active && !p.is_dead)
                .map(|p| (p.x, p.y))
                .collect()
        };
        let npc_positions: std::collections::HashSet<(i32, i32)> = {
            let npcs = self.npcs.read().await;
            npcs.values()
                .filter(|n| n.is_alive())
                .map(|n| (n.x, n.y))
                .collect()
        };

        // Get players in instances to skip world collision for them
        let players_in_instances: std::collections::HashSet<String> = {
            let instances = self.player_instances.read().await;
            instances.keys().cloned().collect()
        };

        // Snapshot chair state for collision checking (position -> (occupied_by, direction))
        let chair_snapshot: HashMap<(i32, i32), (Option<String>, Direction)> = {
            let chairs = self.chairs.read().await;
            chairs.iter().map(|(k, v)| (*k, (v.occupied_by.clone(), v.direction))).collect()
        };

        // Check walkability and entity collision for each pending move
        // Grab chunks lock once for all walkability checks (avoids per-move lock acquisition)
        let chunks_guard = self.world.chunks_read().await;
        let mut valid_moves: Vec<(String, i32, i32)> = Vec::new();
        let mut auto_sit_requests: Vec<(String, i32, i32)> = Vec::new();
        for (id, target_x, target_y, move_dir) in pending_moves {
            // Skip world collision check for players in interiors
            // Interior collision is handled client-side for now
            // TODO: Add server-side interior collision checking
            if !players_in_instances.contains(&id) {
                // Check static tile collision (only for overworld players)
                let coord = crate::chunk::ChunkCoord::from_world(target_x, target_y);
                let walkable = if let Some(chunk) = chunks_guard.get(&coord) {
                    let (lx, ly) = crate::chunk::world_to_local(target_x, target_y);
                    chunk.is_walkable_local(lx, ly)
                } else {
                    false
                };
                if !walkable {
                    continue;
                }
            }
            // Check if another player is on the target tile
            if player_positions.contains(&(target_x, target_y)) {
                continue;
            }
            // Check if an NPC is on the target tile
            if npc_positions.contains(&(target_x, target_y)) {
                continue;
            }
            // Check if target tile is a chair (overworld only - instances have their own maps)
            if !players_in_instances.contains(&id) {
                if let Some((occupied_by, chair_dir)) = chair_snapshot.get(&(target_x, target_y)) {
                    // Only allow sitting when approaching from the chair's facing direction
                    // (player walks toward the chair from in front, so move_dir is opposite of chair_dir)
                    let is_approaching_from_front = match chair_dir {
                        Direction::Down => move_dir == Direction::Up,
                        Direction::Up => move_dir == Direction::Down,
                        Direction::Left => move_dir == Direction::Right,
                        Direction::Right => move_dir == Direction::Left,
                        _ => false,
                    };
                    if is_approaching_from_front && occupied_by.is_none() {
                        // Correct direction and unoccupied - auto-sit
                        auto_sit_requests.push((id, target_x, target_y));
                    }
                    // Always block the move (chair is solid from all directions)
                    continue;
                }
            }
            // Block fighters from leaving the ring during arena fights
            {
                let arena = self.arena_manager.read().await;
                if arena.is_fighting() && arena.is_in_ring(&id) {
                    if let Some(ring_zone) = arena.active_ring_zone() {
                        if !ring_zone.contains(target_x, target_y) {
                            continue;
                        }
                    }
                }
            }
            valid_moves.push((id, target_x, target_y));
        }
        drop(chunks_guard); // Release before NPC AI section acquires its own

        // Process auto-sit requests (players who walked into unoccupied chairs)
        for (id, tile_x, tile_y) in auto_sit_requests {
            self.handle_sit_chair(&id, tile_x, tile_y).await;
        }

        // Track which players moved this tick
        let moved_players: std::collections::HashSet<String> = valid_moves.iter().map(|(id, _, _)| id.clone()).collect();

        // Get set of gathering player IDs for state sync
        let gathering_player_ids: std::collections::HashSet<String> = {
            let gathering = self.gathering.read().await;
            gathering.gathering_player_ids()
        };

        // Stop woodcutting for moved players + get remaining IDs for StateSync (single lock)
        let (woodcutting_player_ids, woodcutting_stopped): (std::collections::HashSet<String>, Vec<String>) = {
            let mut woodcutting = self.woodcutting.write().await;
            let mut stopped = Vec::new();
            for id in &moved_players {
                if woodcutting.is_woodcutting(id) {
                    woodcutting.stop_woodcutting(id);
                    stopped.push(id.clone());
                }
            }
            // Get IDs AFTER stopping, so StateSync reflects accurate state
            let ids = woodcutting.woodcutting_player_ids();
            (ids, stopped)
        };

        {
            let mut players = self.players.write().await;

            // Apply valid moves
            for (id, target_x, target_y) in valid_moves {
                if let Some(player) = players.get_mut(&id) {
                    player.x = target_x;
                    player.y = target_y;
                    player.last_move_tick = current_tick;
                }
            }

            // Clear movement intent for players whose moves were rejected
            // This prevents broadcasting stale velocity (which causes client-side
            // prediction into walls) and stops the server from retrying invalid moves
            for player_id in &pending_player_ids {
                if !moved_players.contains(player_id) {
                    if let Some(player) = players.get_mut(player_id) {
                        player.move_dx = 0;
                        player.move_dy = 0;
                    }
                }
            }

            // Apply HP regen to all players with prayer multiplier
            for player in players.values_mut() {
                let active_ids: Vec<String> = player.active_prayers.iter().cloned().collect();
                let prayer_effects = self.prayer_registry.calculate_effects(&active_ids);
                player.apply_regen(current_time, prayer_effects.hp_regen_multiplier);
            }
        }

        // Prayer drain + mana regen (every 60 ticks / 3 seconds)
        // Combined into single players.write() to reduce lock contention
        if current_tick % PRAYER_DRAIN_INTERVAL_TICKS == 0 {
            struct PrayerDrainUpdate {
                player_id: String,
                new_points: i32,
                max_points: i32,
                active_prayers: Vec<String>,
                depleted: bool,
            }

            let updates: Vec<PrayerDrainUpdate> = {
                let mut players = self.players.write().await;
                let mut updates = Vec::new();

                for player in players.values_mut() {
                    if !player.active || player.is_dead {
                        continue;
                    }

                    // Mana regeneration
                    let max_mp = player.max_mp();
                    if player.mp < max_mp {
                        player.mp = (player.mp + 1).min(max_mp);
                    }

                    // Prayer drain
                    if !player.active_prayers.is_empty() {
                        let active_ids: Vec<String> = player.active_prayers.iter().cloned().collect();
                        let effects = self.prayer_registry.calculate_effects(&active_ids);
                        let drain_amount = effects.total_drain_rate.ceil() as i32;

                        if drain_amount > 0 {
                            let old_points = player.prayer_points;
                            player.prayer_points = (player.prayer_points - drain_amount).max(0);

                            let depleted = player.prayer_points == 0 && old_points > 0;

                            if depleted {
                                player.active_prayers.clear();
                                tracing::debug!(
                                    "Player {} ran out of prayer points, all prayers deactivated",
                                    player.id
                                );
                            }

                            updates.push(PrayerDrainUpdate {
                                player_id: player.id.clone(),
                                new_points: player.prayer_points,
                                max_points: player.max_prayer_points(),
                                active_prayers: player.active_prayers.iter().cloned().collect(),
                                depleted,
                            });
                        }
                    }
                }
                updates
            };

            // Send prayer state updates to affected players
            for update in updates {
                self.send_to_player(&update.player_id, ServerMessage::PrayerStateUpdate {
                    points: update.new_points,
                    max_points: update.max_points,
                    active_prayers: update.active_prayers,
                }).await;

                if update.depleted {
                    self.send_system_message(&update.player_id, "You have run out of prayer points").await;
                }
            }
        }

        // Re-acquire players read lock for generating player updates
        let player_updates = {
            let players = self.players.read().await;
            let mut player_updates = Vec::new();

            // Generate player updates
            for player in players.values() {
                if !player.active {
                    continue;
                }

                player_updates.push(PlayerUpdate {
                    id: player.id.clone(),
                    name: player.name.clone(),
                    x: player.x,
                    y: player.y,
                    direction: player.direction as u8,
                    // Send movement intent directly (what player wants to do)
                    vel_x: player.move_dx,
                    vel_y: player.move_dy,
                    hp: player.hp,
                    max_hp: player.max_hp(),
                    combat_level: player.combat_level(),
                    hitpoints_level: player.skills.hitpoints.level,
                    combat_skill_level: player.skills.combat.level,
                    gold: player.inventory.gold,
                    gender: player.gender.clone(),
                    skin: player.skin.clone(),
                    hair_style: player.hair_style,
                    hair_color: player.hair_color,
                    equipped_head: player.equipped_head.clone(),
                    equipped_body: player.equipped_body.clone(),
                    equipped_weapon: player.equipped_weapon.clone(),
                    equipped_back: player.equipped_back.clone(),
                    equipped_feet: player.equipped_feet.clone(),
                    equipped_ring: player.equipped_ring.clone(),
                    equipped_gloves: player.equipped_gloves.clone(),
                    equipped_necklace: player.equipped_necklace.clone(),
                    equipped_belt: player.equipped_belt.clone(),
                    is_admin: player.is_admin,
                    sitting: player.sitting_at.is_some(),
                    is_gathering: gathering_player_ids.contains(&player.id),
                    is_woodcutting: woodcutting_player_ids.contains(&player.id),
                    mp: player.mp,
                    max_mp: player.max_mp(),
                });
            }
            player_updates
        };

        // Stop gathering for any player who moved
        {
            let mut gathering = self.gathering.write().await;
            let mut stopped = Vec::new();
            for id in &moved_players {
                if gathering.is_gathering(id) {
                    gathering.stop_gathering(id);
                    stopped.push(id.clone());
                }
            }
            drop(gathering);
            for id in stopped {
                self.broadcast(ServerMessage::GatheringStopped {
                    player_id: id.clone(),
                    reason: "moved".to_string(),
                }).await;
            }
        }

        // Broadcast woodcutting stop for players who moved (lock already released above)
        for id in woodcutting_stopped {
            self.broadcast(ServerMessage::WoodcuttingStopped {
                player_id: id.clone(),
                reason: "moved".to_string(),
            }).await;
        }

        // Cancel crafting for any player who moved
        {
            let crafting_to_cancel: Vec<String> = {
                let players = self.players.read().await;
                moved_players.iter()
                    .filter(|id| players.get(*id).map_or(false, |p| p.crafting_state.is_some()))
                    .cloned()
                    .collect()
            };
            for id in crafting_to_cancel {
                self.cancel_crafting(&id, "interrupted").await;
            }
        }

        // Check timed crafting completions
        {
            use crate::crafting::definition::RecipeCategory;

            // Phase 1: Collect completed crafts (read lock)
            struct CraftCompletion {
                pid: String,
                recipe_id: String,
                items_gained: Vec<(String, u32)>,
                xp_gained: u32,
                is_smithing: bool,
                inv_update: Vec<crate::item::InventorySlotUpdate>,
                gold: i32,
                smithing_xp: i64,
                smithing_total_xp: i64,
                smithing_level: i32,
                smithing_leveled: bool,
            }

            let completions = {
                let mut players = self.players.write().await;
                let mut completions: Vec<CraftCompletion> = Vec::new();

                let player_ids: Vec<String> = players.keys().cloned().collect();
                for pid in player_ids {
                    let player = players.get_mut(&pid).unwrap();
                    if !player.active || player.is_dead {
                        continue;
                    }

                    let should_complete = player.crafting_state.as_ref().map_or(false, |cs| {
                        cs.started_at.elapsed().as_millis() as u64 >= cs.duration_ms
                    });

                    if !should_complete {
                        continue;
                    }

                    let crafting = player.crafting_state.take().unwrap();
                    let recipe = match self.crafting_registry.get(&crafting.recipe_id) {
                        Some(r) => r.clone(),
                        None => continue,
                    };

                    // Add results to inventory
                    let mut items_gained = Vec::new();
                    for result in &recipe.results {
                        player.inventory.add_item(&result.item_id, result.count, &self.item_registry);
                        items_gained.push((result.item_id.clone(), result.count as u32));
                    }

                    // Award smithing XP if applicable
                    let xp_gained = recipe.xp;
                    let is_smithing = recipe.category == RecipeCategory::Smithing;
                    let (smithing_xp, smithing_total_xp, smithing_level, smithing_leveled) =
                        if xp_gained > 0 && is_smithing {
                            let leveled = player.skills.smithing.add_xp(xp_gained as i64);
                            (xp_gained as i64, player.skills.smithing.xp, player.skills.smithing.level, leveled)
                        } else {
                            (0, 0, 0, false)
                        };

                    completions.push(CraftCompletion {
                        pid: pid.clone(),
                        recipe_id: crafting.recipe_id,
                        items_gained,
                        xp_gained,
                        is_smithing,
                        inv_update: player.inventory.to_update(),
                        gold: player.inventory.gold,
                        smithing_xp,
                        smithing_total_xp,
                        smithing_level,
                        smithing_leveled,
                    });
                }
                completions
            };

            // Phase 2: Send messages (no locks held)
            for comp in completions {
                tracing::info!(
                    "Player {} completed timed craft {} (gained {:?})",
                    comp.pid, comp.recipe_id, comp.items_gained
                );

                self.send_to_player(&comp.pid, ServerMessage::CraftingCompleted {
                    recipe_id: comp.recipe_id,
                    items_gained: comp.items_gained,
                    xp_gained: comp.xp_gained,
                }).await;

                self.send_to_player(&comp.pid, ServerMessage::InventoryUpdate {
                    player_id: comp.pid.clone(),
                    slots: comp.inv_update,
                    gold: comp.gold,
                }).await;

                // Send smithing XP if applicable
                if comp.is_smithing && comp.smithing_xp > 0 {
                    self.send_to_player(&comp.pid, ServerMessage::SkillXp {
                        player_id: comp.pid.clone(),
                        skill: "smithing".to_string(),
                        xp_gained: comp.smithing_xp,
                        total_xp: comp.smithing_total_xp,
                        level: comp.smithing_level,
                    }).await;

                    if comp.smithing_leveled {
                        tracing::info!("Player {} leveled up smithing to {}", comp.pid, comp.smithing_level);
                        self.broadcast(ServerMessage::SkillLevelUp {
                            player_id: comp.pid.clone(),
                            skill: "smithing".to_string(),
                            new_level: comp.smithing_level,
                        }).await;
                    }
                }
            }
        }

        // Get player positions for NPC AI (only alive overworld players, grid positions)
        let player_positions: Vec<(String, i32, i32, i32)> = {
            let players = self.players.read().await;
            let gathering = self.gathering.read().await;
            let instances = self.player_instances.read().await;
            players.values()
                .filter(|p| p.active && p.is_alive())
                // Players in instances are invisible to overworld NPCs
                .filter(|p| !instances.contains_key(&p.id))
                // Players in gathering zones are invisible to NPCs
                .filter(|p| !gathering.is_gathering(&p.id))
                .map(|p| (p.id.clone(), p.x, p.y, p.hp))
                .collect()
        };

        // Build spatial index: chunk coord -> players in that chunk (for NPC speech lookups)
        let mut players_by_chunk: HashMap<(i32, i32), Vec<(&str, i32, i32)>> = HashMap::new();
        {
            use crate::chunk::CHUNK_SIZE;
            for (pid, px, py, _) in &player_positions {
                let cx = px.div_euclid(CHUNK_SIZE as i32);
                let cy = py.div_euclid(CHUNK_SIZE as i32);
                players_by_chunk.entry((cx, cy)).or_default().push((pid.as_str(), *px, *py));
            }
        }

        let mut npc_updates = Vec::new();
        let mut respawned_npcs = Vec::new();
        let mut npc_attacks: Vec<(String, String, i32, i32, i32)> = Vec::new(); // (npc_id, target_id, npc_level, max_hit, attack_bonus)
        let mut npc_speech_events: Vec<(String, String, String)> = Vec::new(); // (player_id, npc_id, message)
        {
            let mut npcs = self.npcs.write().await;

            // Borrow loaded chunks for synchronous walkability checks during NPC updates
            let chunks_guard = self.world.chunks_read().await;
            let walkable_check = |wx: i32, wy: i32| -> bool {
                let coord = crate::chunk::ChunkCoord::from_world(wx, wy);
                if let Some(chunk) = chunks_guard.get(&coord) {
                    let (lx, ly) = crate::chunk::world_to_local(wx, wy);
                    chunk.is_walkable_local(lx, ly)
                } else {
                    false
                }
            };

            // Build shared occupied tiles set once: all alive NPC positions + player positions + portal tiles
            let mut occupied_tiles: std::collections::HashSet<(i32, i32)> = npcs
                .values()
                .filter(|n| n.is_alive())
                .map(|n| (n.x, n.y))
                .collect();
            for (_, px, py, _) in &player_positions {
                occupied_tiles.insert((*px, *py));
            }
            // Add cached portal tiles so NPCs cannot walk onto or path through portals
            occupied_tiles.extend(&self.portal_tiles);

            for npc in npcs.values_mut() {
                // Check for respawn
                if npc.ready_to_respawn(current_time) {
                    npc.respawn();
                    respawned_npcs.push((npc.id.clone(), npc.x, npc.y));
                    occupied_tiles.insert((npc.x, npc.y));
                }

                // Temporarily remove this NPC's position so it doesn't collide with itself
                let old_pos = (npc.x, npc.y);
                occupied_tiles.remove(&old_pos);

                // Run NPC AI update
                if let Some((target_id, max_hit)) = npc.update(delta_time, &player_positions, &occupied_tiles, current_time, &walkable_check) {
                    npc_attacks.push((npc.id.clone(), target_id, npc.level, max_hit, npc.stats.attack_bonus));
                }

                // Verify NPC isn't on a blocked tile (debug check)
                let new_pos = (npc.x, npc.y);
                if old_pos != new_pos && !walkable_check(npc.x, npc.y) {
                    tracing::error!("BUG: NPC {} moved from {:?} to blocked tile {:?}!", npc.id, old_pos, new_pos);
                }

                // Re-insert updated position
                if npc.is_alive() {
                    occupied_tiles.insert((npc.x, npc.y));
                }

                // Apply HP regen
                npc.apply_regen(current_time);

                // Check NPC speech using spatial index
                {
                    use crate::chunk::CHUNK_SIZE;
                    let chunk_radius = (npc.speech_radius as f32 / CHUNK_SIZE as f32).ceil() as i32;
                    let npc_cx = npc.x.div_euclid(CHUNK_SIZE as i32);
                    let npc_cy = npc.y.div_euclid(CHUNK_SIZE as i32);
                    let mut nearby: Vec<(&str, i32, i32)> = Vec::new();
                    for dx in -chunk_radius..=chunk_radius {
                        for dy in -chunk_radius..=chunk_radius {
                            if let Some(players) = players_by_chunk.get(&(npc_cx + dx, npc_cy + dy)) {
                                nearby.extend_from_slice(players);
                            }
                        }
                    }
                    check_npc_speech(npc, &nearby, current_time, &mut npc_speech_events);
                }

                // Add to updates (all NPCs including dead ones for client awareness)
                npc_updates.push(NpcUpdate::from(&*npc));
            }
        }

        // Send NPC speech bubbles to nearby players
        for (player_id, npc_id, message) in npc_speech_events {
            self.send_to_player(&player_id, ServerMessage::NpcSpeech {
                npc_id: npc_id.clone(),
                message: message.clone(),
            }).await;
        }

        // Process NPC speech for instance NPCs (interiors)
        {
            let players = self.players.read().await;
            let mut instance_players: HashMap<String, Vec<(String, i32, i32)>> = HashMap::new();
            let player_inst = self.player_instances.read().await;
            for (pid, inst_id) in player_inst.iter() {
                if let Some(p) = players.get(pid) {
                    if p.active && p.is_alive() {
                        instance_players.entry(inst_id.clone()).or_default().push((pid.clone(), p.x, p.y));
                    }
                }
            }
            drop(players);
            drop(player_inst);

            let mut instance_speech_events: Vec<(String, String, String)> = Vec::new();

            // Check public and private instances using shared helper
            for entry in self.instance_manager.public_instances.iter()
                .map(|e| e.value().clone())
                .chain(self.instance_manager.private_instances.iter().map(|e| e.value().clone()))
            {
                if let Some(inst_players) = instance_players.get(&entry.id) {
                    let as_refs: Vec<(&str, i32, i32)> = inst_players.iter()
                        .map(|(pid, x, y)| (pid.as_str(), *x, *y))
                        .collect();
                    let mut npcs = entry.npcs.write().await;
                    for npc in npcs.values_mut() {
                        check_npc_speech(npc, &as_refs, current_time, &mut instance_speech_events);
                    }
                }
            }

            for (player_id, npc_id, message) in instance_speech_events {
                self.send_to_player(&player_id, ServerMessage::NpcSpeech {
                    npc_id: npc_id.clone(),
                    message: message.clone(),
                }).await;
            }
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

                    // Player uses their combat skill level and equipment bonus
                    let player_defence_level = target.skills.combat.level;
                    let base_defence_bonus = target.defence_bonus(&self.item_registry);

                    // Apply prayer bonuses to player's defence
                    let active_ids: Vec<String> = target.active_prayers.iter().cloned().collect();
                    let prayer_effects = self.prayer_registry.calculate_effects(&active_ids);
                    let player_defence_bonus = prayer_effects.apply_defence_bonus(base_defence_bonus);

                    // Roll hit/miss
                    if !calculate_hit(npc_attack_level, npc_attack_bonus, player_defence_level, player_defence_bonus) {
                        // Miss - deal 0 damage
                        tracing::debug!(
                            "NPC {} misses {} (atk {} vs def {} + {})",
                            npc_id, target_id, npc_attack_level, player_defence_level, player_defence_bonus
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
                            npc_id, target_id, damage, max_hit, raw_damage, target.hp
                        );
                        (target.hp, target.x as f32, target.y as f32, died, damage)
                    }
                } else {
                    continue;
                }
            };

            // Broadcast damage event to players in the same zone
            self.broadcast_to_zone(&target_id, ServerMessage::DamageEvent {
                source_id: npc_id.clone(),
                target_id: target_id.clone(),
                damage,
                target_hp,
                target_x,
                target_y,
                projectile: None,
            }).await;

            // Interrupt crafting if player took damage
            if damage > 0 {
                self.cancel_crafting(&target_id, "interrupted").await;
            }

            // Handle player death
            if died {
                tracing::info!("NPC {} killed player {}", npc_id, target_id);
                self.broadcast(ServerMessage::PlayerDied {
                    id: target_id.clone(),
                    killer_id: npc_id.clone(),
                }).await;

                // Send prayer state update to dying player (prayers cleared on death)
                let (points, max_points) = {
                    let players = self.players.read().await;
                    if let Some(p) = players.get(&target_id) {
                        (p.prayer_points, p.max_prayer_points())
                    } else {
                        (0, 1)
                    }
                };
                self.send_to_player(&target_id, ServerMessage::PrayerStateUpdate {
                    points,
                    max_points,
                    active_prayers: vec![],  // Cleared on death
                }).await;
            }
        }

        // Broadcast respawns
        for (id, x, y) in respawned_npcs {
            self.broadcast(ServerMessage::NpcRespawned { id, x, y }).await;
        }

        // Check for expired items (60 second lifetime)
        let expired_items: Vec<String> = {
            let items = self.ground_items.read().await;
            items.iter()
                .filter(|(_, item)| item.is_expired(current_time))
                .map(|(id, _)| id.clone())
                .collect()
        };

        // Remove and broadcast despawned items
        for item_id in expired_items {
            let mut items = self.ground_items.write().await;
            if items.remove(&item_id).is_some() {
                drop(items);
                self.broadcast(ServerMessage::ItemDespawned { item_id }).await;
            }
        }

        // Gathering tick: process active gatherers and bonus tiles
        // Split into separate lock phases to avoid holding gathering + players simultaneously
        struct GatherTick {
            pid: String,
            item_id: String,
            xp_gained: i64,
            total_xp: i64,
            level: i32,
            leveled: bool,
            inv_update: Vec<crate::item::InventorySlotUpdate>,
            gold: i32,
        }

        // Phase 1a: Read player stats for active gatherers (players.read only)
        let gatherer_stats: HashMap<String, (i32, f32)> = {
            let players = self.players.read().await;
            let gathering = self.gathering.read().await;
            gathering.player_states.keys()
                .filter_map(|pid| {
                    players.get(pid).map(|p| {
                        let active_ids: Vec<String> = p.active_prayers.iter().cloned().collect();
                        let effects = self.prayer_registry.calculate_effects(&active_ids);
                        (pid.clone(), (p.skills.fishing.level, effects.gather_speed_multiplier()))
                    })
                })
                .collect()
        };

        // Phase 1b: Process gathering ticks (gathering.write only)
        struct GatherResult { pid: String, item_id: String, xp_gained: i64 }
        let (gather_results, bonus_events) = {
            let mut gathering = self.gathering.write().await;
            let mut results: Vec<GatherResult> = Vec::new();
            let gatherer_ids: Vec<String> = gathering.player_states.keys().cloned().collect();
            for pid in &gatherer_ids {
                let (fishing_level, prayer_speed) = gatherer_stats.get(pid).copied().unwrap_or((1, 1.0));
                if let Some(result) = gathering.tick_gathering(pid, fishing_level, current_time, prayer_speed) {
                    results.push(GatherResult { pid: pid.clone(), item_id: result.item_id, xp_gained: result.xp_gained });
                }
            }
            let bonus_events = gathering.tick_bonus_tiles(current_time);
            (results, bonus_events)
        };

        // Phase 1c: Apply results to player inventories (players.write only)
        let mut inventory_full_players: Vec<String> = Vec::new();
        let gather_ticks = {
            let mut players = self.players.write().await;
            let mut ticks: Vec<GatherTick> = Vec::new();
            for gr in &gather_results {
                if let Some(player) = players.get_mut(&gr.pid) {
                    let leftover = player.inventory.add_item(&gr.item_id, 1, &self.item_registry);
                    if leftover > 0 {
                        inventory_full_players.push(gr.pid.clone());
                        continue;
                    }
                    let leveled = player.skills.fishing.add_xp(gr.xp_gained);
                    ticks.push(GatherTick {
                        pid: gr.pid.clone(),
                        item_id: gr.item_id.clone(),
                        xp_gained: gr.xp_gained,
                        total_xp: player.skills.fishing.xp,
                        level: player.skills.fishing.level,
                        leveled,
                        inv_update: player.inventory.to_update(),
                        gold: player.inventory.gold,
                    });
                }
            }
            ticks
        };

        // Phase 1d: Stop gathering for inventory-full players (gathering.write only)
        if !inventory_full_players.is_empty() {
            let mut gathering = self.gathering.write().await;
            for pid in &inventory_full_players {
                gathering.stop_gathering(pid);
            }
        }

        // Phase 2: Send messages (no locks held)
        for tick in gather_ticks {
            self.send_to_player(&tick.pid, ServerMessage::GatheringResult {
                player_id: tick.pid.clone(),
                item_id: tick.item_id,
                xp_gained: tick.xp_gained,
            }).await;
            self.send_to_player(&tick.pid, ServerMessage::InventoryUpdate {
                player_id: tick.pid.clone(),
                slots: tick.inv_update,
                gold: tick.gold,
            }).await;
            self.send_to_player(&tick.pid, ServerMessage::SkillXp {
                player_id: tick.pid.clone(),
                skill: "fishing".to_string(),
                xp_gained: tick.xp_gained,
                total_xp: tick.total_xp,
                level: tick.level,
            }).await;
            if tick.leveled {
                self.broadcast(ServerMessage::SkillLevelUp {
                    player_id: tick.pid.clone(),
                    skill: "fishing".to_string(),
                    new_level: tick.level,
                }).await;
            }
        }
        for pid in inventory_full_players {
            self.broadcast(ServerMessage::GatheringStopped {
                player_id: pid.clone(),
                reason: "inventory_full".to_string(),
            }).await;
        }
        // Bonus tiles only appear in chunk 0,0 of the overworld (not in instances)
        for event in bonus_events {
            match event {
                crate::gathering::BonusTileEvent::Spawned { x, y, zone_id } => {
                    self.send_to_overworld_players(ServerMessage::BonusTileSpawned {
                        x, y, zone_id, telegraph_duration: 5000,
                    }, None).await;
                }
                crate::gathering::BonusTileEvent::Expired { x, y } => {
                    self.send_to_overworld_players(ServerMessage::BonusTileExpired { x, y }, None).await;
                }
            }
        }

        // Tree respawn tick: check for trees that should respawn
        let tree_respawn_events = {
            let mut woodcutting = self.woodcutting.write().await;
            woodcutting.tick_respawns(current_time)
        };
        for event in tree_respawn_events {
            self.broadcast(ServerMessage::TreeRespawned {
                x: event.x,
                y: event.y,
                gid: event.gid,
            }).await;
        }

        // Farming tick: check growth stage transitions (every 5 seconds / ~100 ticks)
        if current_tick % 100 == 50 {
            let updates = {
                let mut farming = self.farming.write().await;
                farming.tick_growth(current_time)
            };
            for (target_player_id, update) in updates {
                self.send_to_player(&target_player_id, ServerMessage::PatchStateUpdate {
                    patch_id: update.patch_id,
                    state: update.state,
                    crop_id: update.crop_id,
                    growth_stage: update.growth_stage,
                    owner_id: update.owner_id,
                }).await;
            }
        }

        // Check for shop restocks (every 60 seconds)
        {
            let last_restock = *self.last_shop_restock.read().await;
            if last_restock.elapsed().as_secs() >= 60 {
                self.restock_shops().await;
                let mut last = self.last_shop_restock.write().await;
                *last = std::time::Instant::now();
            }
        }

        // Send state sync to each player, filtering by instance and view distance
        // Snapshot lock data quickly, then release locks before expensive encoding
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

        // Build position lookup for O(1) access during culling
        let player_pos_map: HashMap<&str, (i32, i32)> = player_updates.iter()
            .map(|p| (p.id.as_str(), (p.x, p.y)))
            .collect();

        // Separate overworld vs instance senders
        let mut instance_groups: HashMap<&str, Vec<(&String, &mpsc::Sender<Vec<u8>>)>> = HashMap::new();
        let mut overworld_senders: Vec<(&String, &mpsc::Sender<Vec<u8>>)> = Vec::new();
        for (player_id, sender) in senders_snapshot.iter() {
            match instance_snapshot.get(player_id) {
                Some(inst_id) => instance_groups.entry(inst_id.as_str()).or_default().push((player_id, sender)),
                None => overworld_senders.push((player_id, sender)),
            }
        }

        // Pre-filter player updates by instance
        let mut players_by_instance: HashMap<&str, Vec<&PlayerUpdate>> = HashMap::new();
        let mut overworld_players: Vec<&PlayerUpdate> = Vec::new();
        for p in &player_updates {
            match instance_snapshot.get(&p.id) {
                Some(inst_id) => players_by_instance.entry(inst_id.as_str()).or_default().push(p),
                None => overworld_players.push(p),
            }
        }

        // Instance groups: encode once per instance, send to all players in that instance
        for (inst_id, group_senders) in &instance_groups {
            let player_values: Vec<rmpv::Value> = players_by_instance
                .get(inst_id)
                .map(|ps| ps.iter().map(|p| crate::protocol::player_update_to_value(p)).collect())
                .unwrap_or_default();

            let npc_values: Vec<rmpv::Value> = if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                instance.get_npc_updates().await.iter()
                    .map(|n| crate::protocol::npc_update_to_value(n))
                    .collect()
            } else {
                Vec::new()
            };

            if let Ok(bytes) = crate::protocol::encode_state_sync_from_values(tick, player_values, npc_values, inst_id) {
                for (pid, sender) in group_senders {
                    if let Err(e) = sender.try_send(bytes.clone()) {
                        tracing::debug!("StateSync drop for {}: {}", pid, e);
                    }
                }
            }
        }

        // Overworld: pre-encode all player/NPC Values once, then filter per-player
        let prebuilt_players: Vec<(i32, i32, rmpv::Value)> = overworld_players.iter()
            .map(|p| (p.x, p.y, crate::protocol::player_update_to_value(p)))
            .collect();
        let prebuilt_npcs: Vec<(i32, i32, rmpv::Value)> = npc_updates.iter()
            .map(|n| (n.x, n.y, crate::protocol::npc_update_to_value(n)))
            .collect();

        for (player_id, sender) in &overworld_senders {
            let (px, py) = match player_pos_map.get(player_id.as_str()) {
                Some(pos) => *pos,
                None => continue,
            };

            let nearby_players: Vec<rmpv::Value> = prebuilt_players.iter()
                .filter(|(x, y, _)| (x - px).abs().max((y - py).abs()) <= VIEW_DISTANCE)
                .map(|(_, _, v)| v.clone())
                .collect();

            let nearby_npcs: Vec<rmpv::Value> = prebuilt_npcs.iter()
                .filter(|(x, y, _)| (x - px).abs().max((y - py).abs()) <= VIEW_DISTANCE)
                .map(|(_, _, v)| v.clone())
                .collect();

            if let Ok(bytes) = crate::protocol::encode_state_sync_from_values(tick, nearby_players, nearby_npcs, "") {
                if let Err(e) = sender.try_send(bytes) {
                    tracing::debug!("StateSync drop for {}: {}", player_id, e);
                }
            }
        }

        // Arena tick: zone detection + state machine
        self.arena_tick(current_time).await;

        // Log slow ticks for debugging latency spikes
        let tick_duration = tick_start.elapsed();
        if tick_duration.as_millis() > 50 {
            tracing::warn!("Slow tick {}: {}ms", current_tick, tick_duration.as_millis());
        }
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

                    if in_queue_zone && !is_queued && arena.state == crate::arena::ArenaState::Idle {
                        if !arena.queue_rejected.contains(player_id) {
                            if let Err(e) = arena.queue_player(player_id, &player.name, player.inventory.gold) {
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
                    let fighter_ids: Vec<String> = fighters.iter().map(|(id, _)| id.clone()).collect();

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
                    self.broadcast_to_arena(ServerMessage::ArenaMatchStart {
                        fighter_ids,
                    }).await;
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
                            if let Err(e) = db.update_arena_stats(
                                0, // character_id will be resolved in the save path
                                won,
                                placement.kills,
                                died,
                                placement.gold_reward,
                            ).await {
                                tracing::warn!("Failed to save arena stats for {}: {}", placement.player_id, e);
                            }
                        }
                    }

                    let placement_data: Vec<crate::protocol::ArenaPlacementData> = placements.iter().map(|p| {
                        crate::protocol::ArenaPlacementData {
                            rank: p.rank,
                            player_id: p.player_id.clone(),
                            player_name: p.player_name.clone(),
                            kills: p.kills,
                            gold_reward: p.gold_reward,
                        }
                    }).collect();

                    self.broadcast_to_arena(ServerMessage::ArenaMatchEnd {
                        placements: placement_data,
                    }).await;

                    // Send inventory updates to all fighters who earned gold
                    for placement in &placements {
                        if placement.gold_reward > 0 {
                            let update = {
                                let players = self.players.read().await;
                                players.get(&placement.player_id).map(|p| {
                                    (p.inventory.to_update(), p.inventory.gold)
                                })
                            };
                            if let Some((slots, gold)) = update {
                                self.send_to_player(&placement.player_id, ServerMessage::InventoryUpdate {
                                    player_id: placement.player_id.clone(),
                                    slots,
                                    gold,
                                }).await;
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
                    }).await;
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
                    }).await;
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

    /// Restock all shops that have restock intervals
    async fn restock_shops(&self) {
        let mut shop_registry = self.shop_registry.write().await;
        let npcs = self.npcs.read().await;

        // Collect shops that need restocking by checking NPCs with merchant configs
        let mut restocked_shops = Vec::new();

        for proto in self.entity_registry.all() {
            if let Some(merchant_config) = &proto.merchant {
                // Check if this merchant has a restock interval
                if merchant_config.restock_interval_minutes.is_some() {
                    // Get and restock the shop
                    if let Some(shop) = shop_registry.get_mut(&merchant_config.shop_id) {
                        shop.restock();

                        // Find all NPCs of this type to broadcast stock updates
                        let npc_ids: Vec<String> = npcs
                            .iter()
                            .filter(|(_, npc)| npc.prototype_id == proto.id)
                            .map(|(npc_id, _)| npc_id.clone())
                            .collect();

                        // Collect stock updates for broadcasting
                        for npc_id in npc_ids {
                            for stock_item in &shop.stock {
                                restocked_shops.push((
                                    npc_id.clone(),
                                    stock_item.item_id.clone(),
                                    stock_item.current_quantity,
                                ));
                            }
                        }

                        tracing::info!(
                            "Restocked shop '{}' for entity type '{}'",
                            merchant_config.shop_id,
                            proto.id
                        );
                    }
                }
            }
        }

        drop(shop_registry);
        drop(npcs);

        // Broadcast all stock updates
        for (npc_id, item_id, quantity) in restocked_shops {
            self.broadcast_shop_stock_update(&npc_id, &item_id, quantity).await;
        }
    }

    /// Handle chunk request from client
    pub async fn handle_chunk_request(&self, chunk_x: i32, chunk_y: i32) -> Option<ServerMessage> {
        use crate::protocol::{ChunkLayerData, ChunkObjectData, ChunkWallData, ChunkPortalData};
        use crate::chunk::WallEdge;

        let coord = ChunkCoord::new(chunk_x, chunk_y);
        if let Some(chunk) = self.world.get_chunk_data(coord).await {
            let layers: Vec<ChunkLayerData> = chunk.layers.iter().map(|layer| {
                ChunkLayerData {
                    layer_type: layer.layer_type as u8,
                    tiles: layer.tiles.clone(),
                }
            }).collect();

            let collision = chunk.pack_collision();

            let objects: Vec<ChunkObjectData> = chunk.objects.iter().map(|obj| {
                ChunkObjectData {
                    gid: obj.gid,
                    tile_x: obj.tile_x,
                    tile_y: obj.tile_y,
                    width: obj.width,
                    height: obj.height,
                }
            }).collect();

            let portals: Vec<ChunkPortalData> = chunk.portals.iter().map(|p| ChunkPortalData {
                id: p.id.clone(),
                x: p.x,
                y: p.y,
                width: p.width,
                height: p.height,
                target_map: p.target_map.clone(),
                target_spawn: p.target_spawn.clone(),
            }).collect();

            Some(ServerMessage::ChunkData {
                chunk_x,
                chunk_y,
                layers,
                collision,
                objects,
                walls: chunk.walls.iter().map(|w| ChunkWallData {
                    gid: w.gid,
                    tile_x: w.tile_x,
                    tile_y: w.tile_y,
                    edge: match w.edge {
                        WallEdge::Down => "down".to_string(),
                        WallEdge::Right => "right".to_string(),
                    },
                }).collect(),
                portals,
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

        let entities: Vec<ClientEntityDef> = self.entity_registry
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

    // =========================================================================
    // Friend System Handlers
    // =========================================================================

    /// Extract character_id from player_id (format: "char_{id}")
    fn parse_character_id(player_id: &str) -> Option<i64> {
        player_id.strip_prefix("char_").and_then(|s| s.parse().ok())
    }

    /// Get player_id from character_id
    fn make_player_id(character_id: i64) -> String {
        format!("char_{}", character_id)
    }

    /// Handle sending a friend request
    pub async fn handle_send_friend_request(&self, player_id: &str, target_name: &str) {
        let Some(db) = &self.db else {
            self.send_to_player(player_id, ServerMessage::FriendActionResult {
                action: "send_request".to_string(),
                success: false,
                error: Some("Database not available".to_string()),
            }).await;
            return;
        };

        let Some(requester_id) = Self::parse_character_id(player_id) else {
            return;
        };

        // Look up target character by name
        let target_id = match db.get_character_id_by_name(target_name).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                self.send_to_player(player_id, ServerMessage::FriendActionResult {
                    action: "send_request".to_string(),
                    success: false,
                    error: Some(format!("Player '{}' not found", target_name)),
                }).await;
                return;
            }
            Err(e) => {
                tracing::error!("Failed to look up player: {}", e);
                self.send_to_player(player_id, ServerMessage::FriendActionResult {
                    action: "send_request".to_string(),
                    success: false,
                    error: Some("Failed to look up player".to_string()),
                }).await;
                return;
            }
        };

        // Can't friend yourself
        if requester_id == target_id {
            self.send_to_player(player_id, ServerMessage::FriendActionResult {
                action: "send_request".to_string(),
                success: false,
                error: Some("You can't add yourself as a friend".to_string()),
            }).await;
            return;
        }

        // Create the friend request
        match db.create_friend_request(requester_id, target_id).await {
            Ok(()) => {
                // Get requester's name
                let requester_name = {
                    let players = self.players.read().await;
                    players.get(player_id).map(|p| p.name.clone()).unwrap_or_default()
                };

                self.send_to_player(player_id, ServerMessage::FriendActionResult {
                    action: "send_request".to_string(),
                    success: true,
                    error: None,
                }).await;

                // Notify the target if they're online
                let target_player_id = Self::make_player_id(target_id);
                self.send_to_player(&target_player_id, ServerMessage::FriendRequestReceived {
                    from_id: requester_id,
                    from_name: requester_name.clone(),
                }).await;

                // Also send them a chat notification
                self.send_to_player(&target_player_id, ServerMessage::ChatMessage {
                    sender_id: "system".to_string(),
                    sender_name: "System".to_string(),
                    text: format!("{} sent you a friend request!", requester_name),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                }).await;
            }
            Err(e) => {
                self.send_to_player(player_id, ServerMessage::FriendActionResult {
                    action: "send_request".to_string(),
                    success: false,
                    error: Some(e),
                }).await;
            }
        }
    }

    /// Handle accepting a friend request
    pub async fn handle_accept_friend_request(&self, player_id: &str, requester_id: i64) {
        let Some(db) = &self.db else {
            self.send_to_player(player_id, ServerMessage::FriendActionResult {
                action: "accept_request".to_string(),
                success: false,
                error: Some("Database not available".to_string()),
            }).await;
            return;
        };

        let Some(recipient_id) = Self::parse_character_id(player_id) else {
            return;
        };

        match db.accept_friend_request(requester_id, recipient_id).await {
            Ok(()) => {
                // Get recipient name (the one accepting) - try online players first, then database
                let recipient_name = {
                    let players = self.players.read().await;
                    players.get(player_id).map(|p| p.name.clone())
                };
                let recipient_name = match recipient_name {
                    Some(name) => name,
                    None => db.get_character_name_by_id(recipient_id).await.ok().flatten().unwrap_or_default(),
                };

                // Get requester name (the one who sent the request) - try online players first, then database
                let requester_player_id = Self::make_player_id(requester_id);
                let requester_name = {
                    let players = self.players.read().await;
                    players.get(&requester_player_id).map(|p| p.name.clone())
                };
                let requester_name = match requester_name {
                    Some(name) => name,
                    None => db.get_character_name_by_id(requester_id).await.ok().flatten().unwrap_or_default(),
                };

                // Notify the accepting player
                self.send_to_player(player_id, ServerMessage::FriendActionResult {
                    action: "accept_request".to_string(),
                    success: true,
                    error: None,
                }).await;

                // Also send them the new friend info
                self.send_to_player(player_id, ServerMessage::FriendRequestAccepted {
                    friend_id: requester_id,
                    friend_name: requester_name.clone(),
                }).await;

                // Notify the original requester if online
                self.send_to_player(&requester_player_id, ServerMessage::FriendRequestAccepted {
                    friend_id: recipient_id,
                    friend_name: recipient_name.clone(),
                }).await;

                // Send chat notification to requester
                self.send_to_player(&requester_player_id, ServerMessage::ChatMessage {
                    sender_id: "system".to_string(),
                    sender_name: "System".to_string(),
                    text: format!("{} accepted your friend request!", recipient_name),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                }).await;
            }
            Err(e) => {
                self.send_to_player(player_id, ServerMessage::FriendActionResult {
                    action: "accept_request".to_string(),
                    success: false,
                    error: Some(e),
                }).await;
            }
        }
    }

    /// Handle declining a friend request
    pub async fn handle_decline_friend_request(&self, player_id: &str, requester_id: i64) {
        let Some(db) = &self.db else {
            self.send_to_player(player_id, ServerMessage::FriendActionResult {
                action: "decline_request".to_string(),
                success: false,
                error: Some("Database not available".to_string()),
            }).await;
            return;
        };

        let Some(recipient_id) = Self::parse_character_id(player_id) else {
            return;
        };

        match db.decline_friend_request(requester_id, recipient_id).await {
            Ok(()) => {
                self.send_to_player(player_id, ServerMessage::FriendActionResult {
                    action: "decline_request".to_string(),
                    success: true,
                    error: None,
                }).await;

                // Optionally notify the requester
                let requester_player_id = Self::make_player_id(requester_id);
                self.send_to_player(&requester_player_id, ServerMessage::FriendRequestDeclined {
                    by_id: recipient_id,
                }).await;
            }
            Err(e) => {
                self.send_to_player(player_id, ServerMessage::FriendActionResult {
                    action: "decline_request".to_string(),
                    success: false,
                    error: Some(e),
                }).await;
            }
        }
    }

    /// Handle removing a friend
    pub async fn handle_remove_friend(&self, player_id: &str, friend_id: i64) {
        let Some(db) = &self.db else {
            self.send_to_player(player_id, ServerMessage::FriendActionResult {
                action: "remove_friend".to_string(),
                success: false,
                error: Some("Database not available".to_string()),
            }).await;
            return;
        };

        let Some(character_id) = Self::parse_character_id(player_id) else {
            return;
        };

        match db.remove_friend(character_id, friend_id).await {
            Ok(()) => {
                self.send_to_player(player_id, ServerMessage::FriendActionResult {
                    action: "remove_friend".to_string(),
                    success: true,
                    error: None,
                }).await;

                // Notify both players
                self.send_to_player(player_id, ServerMessage::FriendRemoved {
                    friend_id,
                }).await;

                let friend_player_id = Self::make_player_id(friend_id);
                self.send_to_player(&friend_player_id, ServerMessage::FriendRemoved {
                    friend_id: character_id,
                }).await;
            }
            Err(e) => {
                self.send_to_player(player_id, ServerMessage::FriendActionResult {
                    action: "remove_friend".to_string(),
                    success: false,
                    error: Some(e),
                }).await;
            }
        }
    }

    /// Handle request for online players list
    pub async fn handle_get_online_players(&self, player_id: &str) {
        let Some(db) = &self.db else {
            return;
        };

        let Some(character_id) = Self::parse_character_id(player_id) else {
            return;
        };

        // Get all online players from the room
        let players = self.players.read().await;
        let mut online_players = Vec::new();

        for player in players.values() {
            if !player.active {
                continue;
            }

            if let Some(pid) = Self::parse_character_id(&player.id) {
                // Check if this player is a friend
                let is_friend = db.are_friends(character_id, pid).await.unwrap_or(false);

                online_players.push(crate::protocol::OnlinePlayerInfo {
                    id: pid,
                    name: player.name.clone(),
                    is_friend,
                });
            }
        }

        self.send_to_player(player_id, ServerMessage::OnlinePlayersList {
            players: online_players,
        }).await;
    }

    /// Send friends list and pending requests to a player (called on connect)
    pub async fn send_friends_data(&self, player_id: &str, online_characters: &dashmap::DashSet<i64>) {
        tracing::info!("send_friends_data called for player_id: {}", player_id);

        let Some(db) = &self.db else {
            tracing::warn!("No database connection in send_friends_data");
            return;
        };

        let Some(character_id) = Self::parse_character_id(player_id) else {
            tracing::warn!("Could not parse character_id from player_id: {}", player_id);
            return;
        };

        tracing::info!("Fetching friends data for character_id: {}", character_id);

        // Get friends list
        match db.get_friends_list(character_id).await {
            Ok(friends) => {
                tracing::info!("Found {} friends for character {}", friends.len(), character_id);
                let friend_infos: Vec<crate::protocol::FriendInfo> = friends
                    .into_iter()
                    .map(|(id, name)| {
                        let online = online_characters.contains(&id);
                        crate::protocol::FriendInfo { id, name, online }
                    })
                    .collect();

                self.send_to_player(player_id, ServerMessage::FriendsList {
                    friends: friend_infos,
                }).await;
            }
            Err(e) => {
                tracing::error!("Error fetching friends list: {:?}", e);
            }
        }

        // Get pending friend requests
        match db.get_pending_requests(character_id).await {
            Ok(requests) => {
                tracing::info!("Found {} pending friend requests for character {}", requests.len(), character_id);
                let request_infos: Vec<crate::protocol::PendingRequestInfo> = requests
                    .into_iter()
                    .map(|(from_id, from_name)| {
                        tracing::info!("  - Request from {} (id: {})", from_name, from_id);
                        crate::protocol::PendingRequestInfo { from_id, from_name }
                    })
                    .collect();

                self.send_to_player(player_id, ServerMessage::PendingFriendRequests {
                    requests: request_infos,
                }).await;
            }
            Err(e) => {
                tracing::error!("Error fetching pending requests: {:?}", e);
            }
        }
    }

    /// Notify friends that a player came online or went offline
    pub async fn broadcast_friend_status(&self, player_id: &str, online: bool) {
        let Some(db) = &self.db else {
            return;
        };

        let Some(character_id) = Self::parse_character_id(player_id) else {
            return;
        };

        // Get all friends of this player
        if let Ok(friends) = db.get_friends_list(character_id).await {
            for (friend_id, _) in friends {
                let friend_player_id = Self::make_player_id(friend_id);
                self.send_to_player(&friend_player_id, ServerMessage::FriendStatusChanged {
                    friend_id: character_id,
                    online,
                }).await;
            }
        }
    }

    // =========================================================================
    // Prayer System
    // =========================================================================

    /// Handle toggling a prayer on/off
    pub async fn handle_toggle_prayer(&self, player_id: &str, prayer_id: &str) {
        // Get prayer definition
        let prayer = match self.prayer_registry.get(prayer_id) {
            Some(p) => p.clone(),
            None => {
                tracing::warn!("Player {} tried to toggle unknown prayer: {}", player_id, prayer_id);
                return;
            }
        };

        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) => p,
            None => return,
        };

        // Check if player is dead
        if player.is_dead {
            return;
        }

        // If prayer is already active, deactivate it
        if player.active_prayers.contains(prayer_id) {
            player.active_prayers.remove(prayer_id);
            tracing::debug!("Player {} deactivated prayer: {}", player_id, prayer_id);
        } else {
            // Check prayer level requirement
            if player.skills.prayer.level < prayer.level_req {
                drop(players);
                self.send_system_message(player_id, &format!(
                    "You need Prayer level {} to use {}",
                    prayer.level_req, prayer.name
                )).await;
                return;
            }

            // Check if player has prayer points
            if player.prayer_points <= 0 {
                drop(players);
                self.send_system_message(player_id, "You have no prayer points remaining").await;
                return;
            }

            // Check for category conflicts and deactivate conflicting prayer
            let mut conflicting_prayer: Option<String> = None;
            for active_id in &player.active_prayers {
                if let Some(active_prayer) = self.prayer_registry.get(active_id) {
                    if active_prayer.category == prayer.category {
                        conflicting_prayer = Some(active_id.clone());
                        break;
                    }
                }
            }

            // Deactivate conflicting prayer if any
            if let Some(conflict_id) = conflicting_prayer {
                player.active_prayers.remove(&conflict_id);
                tracing::debug!(
                    "Player {} deactivated conflicting prayer {} to activate {}",
                    player_id, conflict_id, prayer_id
                );
            }

            // Activate the new prayer
            player.active_prayers.insert(prayer_id.to_string());
            tracing::debug!("Player {} activated prayer: {}", player_id, prayer_id);
        }

        // Build and send prayer state update
        let points = player.prayer_points;
        let max_points = player.max_prayer_points();
        let active_prayers: Vec<String> = player.active_prayers.iter().cloned().collect();
        drop(players);

        self.send_to_player(player_id, ServerMessage::PrayerStateUpdate {
            points,
            max_points,
            active_prayers,
        }).await;
    }

    /// Handle burying bones from inventory
    pub async fn handle_bury_bones(&self, player_id: &str, slot: usize) {
        tracing::debug!("Player {} burying bones from slot: {}", player_id, slot);

        // Get item info and validate it's bones
        let (item_id, item_name, prayer_xp) = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) => p,
                None => return,
            };

            // Check if slot is valid and has an item
            let slot_item = match player.inventory.slots.get(slot).and_then(|s| s.as_ref()) {
                Some(item) => item,
                None => {
                    drop(players);
                    self.send_system_message(player_id, "There's nothing in that slot.").await;
                    return;
                }
            };

            let item_id = slot_item.item_id.clone();

            // Get item definition
            let item_def = match self.item_registry.get(&item_id) {
                Some(def) => def,
                None => {
                    drop(players);
                    self.send_system_message(player_id, "Unknown item.").await;
                    return;
                }
            };

            // Check if it's bones (has prayer_xp > 0)
            if !item_def.is_bones() {
                drop(players);
                self.send_system_message(player_id, "You can only bury bones.").await;
                return;
            }

            (item_id, item_def.display_name.clone(), item_def.prayer_xp)
        };

        // Remove bones from inventory and award XP
        let (inv_update, xp_result) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) => p,
                None => return,
            };

            // Remove one bone from the slot
            if let Some(ref mut slot_item) = player.inventory.slots[slot] {
                slot_item.quantity -= 1;
                if slot_item.quantity <= 0 {
                    player.inventory.slots[slot] = None;
                }
            }

            // Award Prayer XP
            let leveled = player.skills.prayer.add_xp(prayer_xp as i64);
            let xp_result = (
                prayer_xp as i64,
                player.skills.prayer.xp,
                player.skills.prayer.level,
                leveled,
            );

            // If leveled up, update max prayer points
            if leveled {
                player.prayer_points = player.max_prayer_points();
            }

            let inv_update = (player.inventory.to_update(), player.inventory.gold);

            (inv_update, xp_result)
        };

        // Send chat message
        self.send_system_message(player_id, &format!("You bury the {}.", item_name.to_lowercase())).await;

        // Send inventory update
        self.send_to_player(player_id, ServerMessage::InventoryUpdate {
            player_id: player_id.to_string(),
            slots: inv_update.0,
            gold: inv_update.1,
        }).await;

        // Send XP update
        self.send_to_player(player_id, ServerMessage::SkillXp {
            player_id: player_id.to_string(),
            skill: "prayer".to_string(),
            xp_gained: xp_result.0,
            total_xp: xp_result.1,
            level: xp_result.2,
        }).await;

        // Broadcast level up if applicable
        if xp_result.3 {
            tracing::info!("Player {} leveled up Prayer to {}", player_id, xp_result.2);
            self.broadcast(ServerMessage::SkillLevelUp {
                player_id: player_id.to_string(),
                skill: "prayer".to_string(),
                new_level: xp_result.2,
            }).await;

            // Send updated prayer state with new max points
            let (points, max_points, active_prayers) = {
                let players = self.players.read().await;
                if let Some(player) = players.get(player_id) {
                    (
                        player.prayer_points,
                        player.max_prayer_points(),
                        player.active_prayers.iter().cloned().collect::<Vec<_>>(),
                    )
                } else {
                    return;
                }
            };

            self.send_to_player(player_id, ServerMessage::PrayerStateUpdate {
                points,
                max_points,
                active_prayers,
            }).await;
        }
    }

    /// Handle praying at an altar to restore prayer points
    pub async fn handle_pray_at_altar(&self, player_id: &str, altar_id: &str) {
        tracing::debug!("Player {} praying at altar: {}", player_id, altar_id);

        // Get player position
        let player_pos = {
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

        // Get altar info and validate it's an altar
        let altar_info = if let Some(ref _inst_id) = instance_id {
            // Check instance NPCs
            if let Some(instance) = self.instance_manager.find_player_instance(player_id).await {
                let npcs = instance.npcs.read().await;
                npcs.get(altar_id).map(|npc| {
                    let dx = (npc.x - player_pos.0) as f32;
                    let dy = (npc.y - player_pos.1) as f32;
                    let distance = (dx * dx + dy * dy).sqrt();
                    (npc.prototype_id.clone(), distance)
                })
            } else {
                None
            }
        } else {
            // Check overworld NPCs
            let npcs = self.npcs.read().await;
            npcs.get(altar_id).map(|npc| {
                let dx = (npc.x - player_pos.0) as f32;
                let dy = (npc.y - player_pos.1) as f32;
                let distance = (dx * dx + dy * dy).sqrt();
                (npc.prototype_id.clone(), distance)
            })
        };

        let (entity_type, distance) = match altar_info {
            Some(info) => info,
            None => {
                self.send_system_message(player_id, "Altar not found.").await;
                return;
            }
        };

        // Check distance
        if distance > 2.5 {
            self.send_system_message(player_id, "You need to be closer to the altar.").await;
            return;
        }

        // Validate it's actually an altar
        let is_altar = self.entity_registry.get(&entity_type)
            .map(|proto| proto.behaviors.altar)
            .unwrap_or(false);

        if !is_altar {
            self.send_system_message(player_id, "That's not an altar.").await;
            return;
        }

        // Restore prayer points
        let (restored, points, max_points, active_prayers) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) => p,
                None => return,
            };

            let max_points = player.max_prayer_points();
            let old_points = player.prayer_points;

            if player.prayer_points >= max_points {
                (0, max_points, max_points, player.active_prayers.iter().cloned().collect::<Vec<_>>())
            } else {
                player.prayer_points = max_points;
                let restored = max_points - old_points;
                (restored, max_points, max_points, player.active_prayers.iter().cloned().collect::<Vec<_>>())
            }
        };

        if restored > 0 {
            self.send_system_message(player_id, &format!(
                "You pray at the altar. Your prayer points have been restored. (+{} points)",
                restored
            )).await;
        } else {
            self.send_system_message(player_id, "You pray at the altar. Your prayer is already full.").await;
        }

        // Send prayer state update
        self.send_to_player(player_id, ServerMessage::PrayerStateUpdate {
            points,
            max_points,
            active_prayers,
        }).await;
    }

    /// Handle casting a spell
    pub async fn handle_cast_spell(&self, player_id: &str, spell_id: &str) {
        // 1. Look up spell definition
        let spell_def = match crate::spell::get_spell(spell_id) {
            Some(s) => s,
            None => {
                self.send_to_player(player_id, ServerMessage::SpellResult {
                    success: false,
                    reason: Some("Unknown spell".to_string()),
                }).await;
                return;
            }
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

            // Check magic level
            if player.skills.magic.level < spell_def.magic_level_req {
                self.send_to_player(player_id, ServerMessage::SpellResult {
                    success: false,
                    reason: Some("Magic level too low".to_string()),
                }).await;
                return;
            }
            // Check mana
            if player.mp < spell_def.mana_cost {
                self.send_to_player(player_id, ServerMessage::SpellResult {
                    success: false,
                    reason: Some("Not enough mana".to_string()),
                }).await;
                return;
            }
            // Check cooldown
            if let Some(&last_cast) = player.spell_cooldowns.get(spell_def.id) {
                if current_time < last_cast + spell_def.cooldown_ms {
                    self.send_to_player(player_id, ServerMessage::SpellResult {
                        success: false,
                        reason: Some("Spell on cooldown".to_string()),
                    }).await;
                    return;
                }
            }
        }

        // 3. Dispatch based on spell type
        match spell_def.spell_type {
            crate::spell::SpellType::Damage => self.cast_damage_spell(player_id, spell_def, current_time).await,
            crate::spell::SpellType::Heal => self.cast_heal_spell(player_id, spell_def, current_time).await,
        }
    }

    /// Cast a damage spell on the player's current target
    async fn cast_damage_spell(&self, player_id: &str, spell_def: &crate::spell::SpellDef, current_time: u64) {
        // 1. Get attacker info and target
        let (caster_name, caster_x, caster_y, target_id_opt, magic_level, combat_level) = {
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
                player.skills.combat.level,
            )
        };

        // Effective attack level for spells: blend of combat and magic so low-magic is still usable
        let effective_level = (combat_level + magic_level) / 2;

        // Must have a target
        let target_id = match target_id_opt {
            Some(id) => id,
            None => {
                self.send_to_player(player_id, ServerMessage::SpellResult {
                    success: false,
                    reason: Some("No target selected".to_string()),
                }).await;
                return;
            }
        };

        // Determine caster's instance context (None = overworld)
        let caster_instance = self.player_instances.read().await.get(player_id).cloned();

        // 2. Resolve target: check NPCs first, then players (same pattern as handle_attack)
        let mut is_npc = false;
        let mut target_x: i32 = 0;
        let mut target_y: i32 = 0;
        let mut target_exists = false;

        // Check NPCs (overworld NPCs only targetable by overworld casters)
        if caster_instance.is_none() {
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
                if target.active && target.hp > 0 && !target.is_dead && target_instance == caster_instance {
                    is_npc = false;
                    target_x = target.x;
                    target_y = target.y;
                    target_exists = true;
                }
            }
        }

        if !target_exists {
            self.send_to_player(player_id, ServerMessage::SpellResult {
                success: false,
                reason: Some("Invalid target".to_string()),
            }).await;
            return;
        }

        // 3. Check range (Chebyshev distance, 5 tiles for spells)
        let dx = (caster_x - target_x).abs();
        let dy = (caster_y - target_y).abs();
        let distance = dx.max(dy);
        if distance > 5 {
            self.send_to_player(player_id, ServerMessage::SpellResult {
                success: false,
                reason: Some("Target out of range".to_string()),
            }).await;
            return;
        }

        // 4. Deduct mana and set cooldown
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.mp -= spell_def.mana_cost;
                player.spell_cooldowns.insert(spell_def.id.to_string(), current_time);
                // Stop movement when casting
                player.move_dx = 0;
                player.move_dy = 0;
            }
        }

        // 5. Broadcast casting animation
        self.broadcast_to_zone(player_id, ServerMessage::PlayerAttack {
            player_id: player_id.to_string(),
            attack_type: "spell".to_string(),
        }).await;

        // 6. Calculate hit/miss using blended combat+magic level
        let attack_bonus = 0; // Spells don't use equipment attack bonus
        let (target_hp, target_name, target_died, actual_damage) = if is_npc {
            let mut npcs = self.npcs.write().await;
            if let Some(npc) = npcs.get_mut(&target_id) {
                let npc_defence_level = npc.level;
                let npc_defence_bonus = npc.stats.defence_bonus;

                if !crate::skills::calculate_hit(effective_level, attack_bonus, npc_defence_level, npc_defence_bonus) {
                    // Miss
                    let name = npc.name();
                    tracing::info!(
                        "{} spell misses {} (eff {} [cmb{}+mag{}] vs def {})",
                        caster_name, name, effective_level, combat_level, magic_level, npc_defence_level
                    );
                    (npc.hp, name, false, 0)
                } else {
                    // Hit
                    let max_hit = crate::spell::calculate_spell_max_hit(magic_level, spell_def.base_power);
                    let damage = crate::spell::roll_spell_damage(max_hit);
                    let died = npc.take_damage(damage, current_time, Some(player_id));
                    let name = npc.name();
                    tracing::info!(
                        "{} spell hits {} for {} damage (max: {}, HP: {})",
                        caster_name, name, damage, max_hit, npc.hp
                    );
                    (npc.hp, name, died, damage)
                }
            } else {
                return;
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

                let target_combat_level = target.skills.combat.level;
                let base_defence_bonus = target.defence_bonus(&self.item_registry);

                // Apply prayer bonuses to target's defence
                let target_active_ids: Vec<String> = target.active_prayers.iter().cloned().collect();
                let target_prayer_effects = self.prayer_registry.calculate_effects(&target_active_ids);
                let target_defence_bonus = target_prayer_effects.apply_defence_bonus(base_defence_bonus);

                if !crate::skills::calculate_hit(effective_level, attack_bonus, target_combat_level, target_defence_bonus) {
                    // Miss
                    let name = target.name.clone();
                    tracing::info!(
                        "{} spell misses {} (eff {} [cmb{}+mag{}] vs cmb {} + {})",
                        caster_name, name, effective_level, combat_level, magic_level, target_combat_level, target_defence_bonus
                    );
                    (target.hp, name, false, 0)
                } else {
                    // Hit
                    let max_hit = crate::spell::calculate_spell_max_hit(magic_level, spell_def.base_power);
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
                        caster_name, name, damage, max_hit, raw_damage, target.hp
                    );
                    (target.hp, name, died, damage)
                }
            } else {
                return;
            }
        };

        // 7. Broadcast SpellEffect to nearby players in the zone
        self.broadcast_to_zone(player_id, ServerMessage::SpellEffect {
            caster_id: player_id.to_string(),
            target_id: Some(target_id.clone()),
            spell_id: spell_def.id.to_string(),
            target_x,
            target_y,
        }).await;

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

        // 9. Award Magic XP and Hitpoints XP
        if actual_damage > 0 {
            let xp_results = {
                let mut players = self.players.write().await;
                if let Some(attacker) = players.get_mut(player_id) {
                    let magic_xp = (actual_damage as f64 * crate::skills::MAGIC_XP_PER_DAMAGE) as i64;
                    let hp_xp = (actual_damage as f64 * crate::skills::HITPOINTS_XP_PER_DAMAGE) as i64;

                    let mut results = Vec::new();

                    // Award Magic XP
                    let magic_leveled = attacker.skills.magic.add_xp(magic_xp);
                    results.push((SkillType::Magic, magic_xp, attacker.skills.magic.xp, attacker.skills.magic.level, magic_leveled));

                    // Award Hitpoints XP
                    let old_hp_level = attacker.skills.hitpoints.level;
                    let hp_leveled = attacker.skills.hitpoints.add_xp(hp_xp);
                    if hp_leveled {
                        let new_max = attacker.skills.hitpoints.level;
                        attacker.hp += new_max - old_hp_level;
                    }
                    results.push((SkillType::Hitpoints, hp_xp, attacker.skills.hitpoints.xp, attacker.skills.hitpoints.level, hp_leveled));

                    Some(results)
                } else {
                    None
                }
            };

            if let Some(results) = xp_results {
                for (skill_type, xp_gained, total_xp, level, leveled_up) in results {
                    self.send_to_player(player_id, ServerMessage::SkillXp {
                        player_id: player_id.to_string(),
                        skill: skill_type.as_str().to_string(),
                        xp_gained,
                        total_xp,
                        level,
                    }).await;

                    if leveled_up {
                        tracing::info!("Player {} leveled up {} to {}", player_id, skill_type.as_str(), level);
                        self.broadcast(ServerMessage::SkillLevelUp {
                            player_id: player_id.to_string(),
                            skill: skill_type.as_str().to_string(),
                            new_level: level,
                        }).await;
                    }
                }
            }
        }

        // 10. Interrupt crafting if target is a player who took damage
        if !is_npc && actual_damage > 0 {
            self.cancel_crafting(&target_id, "interrupted").await;
        }

        // 11. Handle death
        if target_died {
            tracing::info!("{} killed {} with spell {}", caster_name, target_name, spell_def.name);
            if is_npc {
                // Get NPC info for exp and loot
                let (prototype_id, npc_level) = {
                    let npcs = self.npcs.read().await;
                    npcs.get(&target_id)
                        .map(|n| (n.prototype_id.clone(), n.level))
                        .unwrap_or(("unknown".to_string(), 1))
                };

                // Broadcast NPC death
                self.broadcast(ServerMessage::NpcDied {
                    id: target_id.clone(),
                    killer_id: player_id.to_string(),
                }).await;

                // Process quest kill event
                self.process_quest_kill(player_id, &prototype_id).await;

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
                        prototype, target_x as f32, target_y as f32, player_id, drop_time, npc_level, killer_instance
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

                        let existing_gold_id = items.iter()
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
                        let eliminated_name = arena.match_stats.fighter_names.get(&target_id).cloned().unwrap_or_default();
                        let killer_name = arena.match_stats.fighter_names.get(player_id).cloned().unwrap_or_default();
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
                    }).await;

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

                        let placement_data: Vec<crate::protocol::ArenaPlacementData> = placements.iter().map(|p| {
                            crate::protocol::ArenaPlacementData {
                                rank: p.rank,
                                player_id: p.player_id.clone(),
                                player_name: p.player_name.clone(),
                                kills: p.kills,
                                gold_reward: p.gold_reward,
                            }
                        }).collect();

                        self.broadcast_to_arena(ServerMessage::ArenaMatchEnd {
                            placements: placement_data,
                        }).await;

                        self.broadcast_to_arena(ServerMessage::ArenaStateUpdate {
                            state: "results".to_string(),
                            countdown_remaining: None,
                            queued_count: 0,
                            fighter_count: 0,
                            entry_fee: {
                                let arena = self.arena_manager.read().await;
                                arena.config.entry_fee
                            },
                        }).await;

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
                                    players.get(&placement.player_id).map(|p| {
                                        (p.inventory.to_update(), p.inventory.gold)
                                    })
                                };
                                if let Some((slots, gold)) = update {
                                    self.send_to_player(&placement.player_id, ServerMessage::InventoryUpdate {
                                        player_id: placement.player_id.clone(),
                                        slots,
                                        gold,
                                    }).await;
                                }
                            }
                        }

                        // Save arena stats to DB
                        if let Some(ref db) = self.db {
                            for placement in &placements {
                                if let Some(char_id) = placement.player_id.strip_prefix("char_").and_then(|s| s.parse::<i64>().ok()) {
                                    let won = placement.rank == 1;
                                    let died = placement.rank > 1;
                                    if let Err(e) = db.update_arena_stats(
                                        char_id,
                                        won,
                                        placement.kills,
                                        died,
                                        placement.gold_reward,
                                    ).await {
                                        tracing::warn!("Failed to save arena stats for {}: {}", placement.player_id, e);
                                    }
                                }
                            }
                        }

                        if let Some(winner) = placements.iter().find(|p| p.rank == 1) {
                            self.send_system_message(&winner.player_id, &format!(
                                "You won the arena match! +{} gold", winner.gold_reward
                            )).await;
                        }
                    }
                } else {
                    // Normal player death
                    self.broadcast(ServerMessage::PlayerDied {
                        id: target_id.clone(),
                        killer_id: player_id.to_string(),
                    }).await;

                    // Send prayer state update to dying player (prayers cleared on death)
                    let (points, max_points) = {
                        let players = self.players.read().await;
                        if let Some(p) = players.get(&target_id) {
                            (p.prayer_points, p.max_prayer_points())
                        } else {
                            (0, 1)
                        }
                    };
                    self.send_to_player(&target_id, ServerMessage::PrayerStateUpdate {
                        points,
                        max_points,
                        active_prayers: vec![],
                    }).await;
                }
            }
        }
    }

    /// Cast a heal spell on self
    async fn cast_heal_spell(&self, player_id: &str, spell_def: &crate::spell::SpellDef, current_time: u64) {
        // Get caster info
        let (caster_x, caster_y, magic_level, current_hp, max_hp) = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) if !p.is_dead && p.active => p,
                _ => return,
            };
            (player.x, player.y, player.skills.magic.level, player.hp, player.max_hp())
        };

        // 1. Deduct mana and set cooldown
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.mp -= spell_def.mana_cost;
                player.spell_cooldowns.insert(spell_def.id.to_string(), current_time);
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
        self.broadcast_to_zone(player_id, ServerMessage::SpellEffect {
            caster_id: player_id.to_string(),
            target_id: None,
            spell_id: spell_def.id.to_string(),
            target_x: caster_x,
            target_y: caster_y,
        }).await;

        // 5. Broadcast casting animation
        self.broadcast_to_zone(player_id, ServerMessage::PlayerAttack {
            player_id: player_id.to_string(),
            attack_type: "spell".to_string(),
        }).await;

        // 6. Award Magic XP based on amount healed
        if actual_heal > 0 {
            let xp_results = {
                let mut players = self.players.write().await;
                if let Some(caster) = players.get_mut(player_id) {
                    let magic_xp = (actual_heal as f64 * crate::skills::MAGIC_XP_PER_HEAL) as i64;

                    let magic_leveled = caster.skills.magic.add_xp(magic_xp);
                    Some((SkillType::Magic, magic_xp, caster.skills.magic.xp, caster.skills.magic.level, magic_leveled))
                } else {
                    None
                }
            };

            if let Some((skill_type, xp_gained, total_xp, level, leveled_up)) = xp_results {
                self.send_to_player(player_id, ServerMessage::SkillXp {
                    player_id: player_id.to_string(),
                    skill: skill_type.as_str().to_string(),
                    xp_gained,
                    total_xp,
                    level,
                }).await;

                if leveled_up {
                    tracing::info!("Player {} leveled up {} to {}", player_id, skill_type.as_str(), level);
                    self.broadcast(ServerMessage::SkillLevelUp {
                        player_id: player_id.to_string(),
                        skill: skill_type.as_str().to_string(),
                        new_level: level,
                    }).await;
                }
            }
        }

        tracing::info!("Player {} healed for {} HP with spell {}", player_id, actual_heal, spell_def.name);
    }

    /// Handle offering bones at an altar for bonus XP
    pub async fn handle_offer_bones(&self, player_id: &str, slot: usize, altar_id: &str) {
        tracing::debug!("Player {} offering bones at altar {} from slot {}", player_id, altar_id, slot);

        // Get player position
        let player_pos = {
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

        // Get altar info and validate it's an altar
        let altar_info = if let Some(ref _inst_id) = instance_id {
            // Check instance NPCs
            if let Some(instance) = self.instance_manager.find_player_instance(player_id).await {
                let npcs = instance.npcs.read().await;
                npcs.get(altar_id).map(|npc| {
                    let dx = (npc.x - player_pos.0) as f32;
                    let dy = (npc.y - player_pos.1) as f32;
                    let distance = (dx * dx + dy * dy).sqrt();
                    (npc.prototype_id.clone(), distance)
                })
            } else {
                None
            }
        } else {
            // Check overworld NPCs
            let npcs = self.npcs.read().await;
            npcs.get(altar_id).map(|npc| {
                let dx = (npc.x - player_pos.0) as f32;
                let dy = (npc.y - player_pos.1) as f32;
                let distance = (dx * dx + dy * dy).sqrt();
                (npc.prototype_id.clone(), distance)
            })
        };

        let (entity_type, distance) = match altar_info {
            Some(info) => info,
            None => {
                self.send_system_message(player_id, "Altar not found.").await;
                return;
            }
        };

        // Check distance
        if distance > 2.5 {
            self.send_system_message(player_id, "You need to be closer to the altar.").await;
            return;
        }

        // Validate it's actually an altar
        let is_altar = self.entity_registry.get(&entity_type)
            .map(|proto| proto.behaviors.altar)
            .unwrap_or(false);

        if !is_altar {
            self.send_system_message(player_id, "That's not an altar.").await;
            return;
        }

        // Get item info and validate it's bones
        let (item_id, item_name, base_prayer_xp) = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) => p,
                None => return,
            };

            // Check if slot is valid and has an item
            let slot_item = match player.inventory.slots.get(slot).and_then(|s| s.as_ref()) {
                Some(item) => item,
                None => {
                    drop(players);
                    self.send_system_message(player_id, "There's nothing in that slot.").await;
                    return;
                }
            };

            let item_id = slot_item.item_id.clone();

            // Get item definition
            let item_def = match self.item_registry.get(&item_id) {
                Some(def) => def,
                None => {
                    drop(players);
                    self.send_system_message(player_id, "Unknown item.").await;
                    return;
                }
            };

            // Check if it's bones (has prayer_xp > 0)
            if !item_def.is_bones() {
                drop(players);
                self.send_system_message(player_id, "You can only offer bones at the altar.").await;
                return;
            }

            (item_id, item_def.display_name.clone(), item_def.prayer_xp)
        };

        // Calculate altar XP bonus (~2.5x normal bury XP)
        // Regular bones: 5 -> 12, Big bones: 15 -> 37, Dragon bones: 72 -> 180
        let altar_xp = match item_id.as_str() {
            "regular_bones" => 12,
            "big_bones" => 37,
            "dragon_bones" => 180,
            _ => (base_prayer_xp as f32 * 2.5) as i32, // Fallback for other bone types
        };

        // Remove bones from inventory and award XP
        let (inv_update, xp_result) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) => p,
                None => return,
            };

            // Remove one bone from the slot
            if let Some(ref mut slot_item) = player.inventory.slots[slot] {
                slot_item.quantity -= 1;
                if slot_item.quantity <= 0 {
                    player.inventory.slots[slot] = None;
                }
            }

            // Award Prayer XP (altar bonus)
            let leveled = player.skills.prayer.add_xp(altar_xp as i64);
            let xp_result = (
                altar_xp as i64,
                player.skills.prayer.xp,
                player.skills.prayer.level,
                leveled,
            );

            // If leveled up, update max prayer points
            if leveled {
                player.prayer_points = player.max_prayer_points();
            }

            let inv_update = (player.inventory.to_update(), player.inventory.gold);

            (inv_update, xp_result)
        };

        // Send chat message
        self.send_system_message(player_id, &format!(
            "The gods are pleased with your offering. (+{} Prayer XP)",
            altar_xp
        )).await;

        // Send inventory update
        self.send_to_player(player_id, ServerMessage::InventoryUpdate {
            player_id: player_id.to_string(),
            slots: inv_update.0,
            gold: inv_update.1,
        }).await;

        // Send XP update
        self.send_to_player(player_id, ServerMessage::SkillXp {
            player_id: player_id.to_string(),
            skill: "prayer".to_string(),
            xp_gained: xp_result.0,
            total_xp: xp_result.1,
            level: xp_result.2,
        }).await;

        // Broadcast level up if applicable
        if xp_result.3 {
            tracing::info!("Player {} leveled up Prayer to {}", player_id, xp_result.2);
            self.broadcast(ServerMessage::SkillLevelUp {
                player_id: player_id.to_string(),
                skill: "prayer".to_string(),
                new_level: xp_result.2,
            }).await;

            // Send updated prayer state with new max points
            let (points, max_points, active_prayers) = {
                let players = self.players.read().await;
                if let Some(player) = players.get(player_id) {
                    (
                        player.prayer_points,
                        player.max_prayer_points(),
                        player.active_prayers.iter().cloned().collect::<Vec<_>>(),
                    )
                } else {
                    return;
                }
            };

            self.send_to_player(player_id, ServerMessage::PrayerStateUpdate {
                points,
                max_points,
                active_prayers,
            }).await;
        }
    }

    pub async fn handle_offer_all_bones(&self, player_id: &str, item_id: &str, altar_id: &str) {
        tracing::debug!("Player {} offering all {} at altar {}", player_id, item_id, altar_id);

        // Get player position
        let player_pos = {
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

        // Get altar info and validate it's an altar
        let altar_info = if let Some(ref _inst_id) = instance_id {
            if let Some(instance) = self.instance_manager.find_player_instance(player_id).await {
                let npcs = instance.npcs.read().await;
                npcs.get(altar_id).map(|npc| {
                    let dx = (npc.x - player_pos.0) as f32;
                    let dy = (npc.y - player_pos.1) as f32;
                    let distance = (dx * dx + dy * dy).sqrt();
                    (npc.prototype_id.clone(), distance)
                })
            } else {
                None
            }
        } else {
            let npcs = self.npcs.read().await;
            npcs.get(altar_id).map(|npc| {
                let dx = (npc.x - player_pos.0) as f32;
                let dy = (npc.y - player_pos.1) as f32;
                let distance = (dx * dx + dy * dy).sqrt();
                (npc.prototype_id.clone(), distance)
            })
        };

        let (entity_type, distance) = match altar_info {
            Some(info) => info,
            None => {
                self.send_system_message(player_id, "Altar not found.").await;
                return;
            }
        };

        if distance > 2.5 {
            self.send_system_message(player_id, "You need to be closer to the altar.").await;
            return;
        }

        let is_altar = self.entity_registry.get(&entity_type)
            .map(|proto| proto.behaviors.altar)
            .unwrap_or(false);

        if !is_altar {
            self.send_system_message(player_id, "That's not an altar.").await;
            return;
        }

        // Validate item is bones
        let (item_name, base_prayer_xp) = match self.item_registry.get(item_id) {
            Some(def) if def.is_bones() => (def.display_name.clone(), def.prayer_xp),
            _ => {
                self.send_system_message(player_id, "You can only offer bones at the altar.").await;
                return;
            }
        };

        // Calculate altar XP per bone (matches handle_offer_bones logic)
        let altar_xp_per = match item_id {
            "regular_bones" => 12,
            "big_bones" => 37,
            "dragon_bones" => 180,
            _ => (base_prayer_xp as f32 * 2.5) as i32,
        };

        // Count and remove all bones of this type, award XP
        let (total_bones, inv_update, xp_result) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) => p,
                None => return,
            };

            // Count total bones of this type across all slots
            let mut total = 0i32;
            for i in 0..player.inventory.slots.len() {
                if let Some(ref s) = player.inventory.slots[i] {
                    if s.item_id == item_id {
                        total += s.quantity;
                        player.inventory.slots[i] = None;
                    }
                }
            }

            if total == 0 {
                drop(players);
                self.send_system_message(player_id, "You don't have any of those bones.").await;
                return;
            }

            let total_xp = altar_xp_per as i64 * total as i64;
            let leveled = player.skills.prayer.add_xp(total_xp);
            let xp_result = (
                total_xp,
                player.skills.prayer.xp,
                player.skills.prayer.level,
                leveled,
            );

            if leveled {
                player.prayer_points = player.max_prayer_points();
            }

            let inv_update = (player.inventory.to_update(), player.inventory.gold);
            (total, inv_update, xp_result)
        };

        self.send_system_message(player_id, &format!(
            "The gods are pleased with your offering of {} {}. (+{} Prayer XP)",
            total_bones, item_name, xp_result.0
        )).await;

        self.send_to_player(player_id, ServerMessage::InventoryUpdate {
            player_id: player_id.to_string(),
            slots: inv_update.0,
            gold: inv_update.1,
        }).await;

        self.send_to_player(player_id, ServerMessage::SkillXp {
            player_id: player_id.to_string(),
            skill: "prayer".to_string(),
            xp_gained: xp_result.0,
            total_xp: xp_result.1,
            level: xp_result.2,
        }).await;

        if xp_result.3 {
            tracing::info!("Player {} leveled up Prayer to {}", player_id, xp_result.2);
            self.broadcast(ServerMessage::SkillLevelUp {
                player_id: player_id.to_string(),
                skill: "prayer".to_string(),
                new_level: xp_result.2,
            }).await;

            // Send updated prayer state with new max points
            let (points, max_points, active_prayers) = {
                let players = self.players.read().await;
                if let Some(player) = players.get(player_id) {
                    (
                        player.prayer_points,
                        player.max_prayer_points(),
                        player.active_prayers.iter().cloned().collect::<Vec<_>>(),
                    )
                } else {
                    return;
                }
            };

            self.send_to_player(player_id, ServerMessage::PrayerStateUpdate {
                points,
                max_points,
                active_prayers,
            }).await;
        }
    }
}
