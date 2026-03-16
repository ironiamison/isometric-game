use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Animation Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum AnimationType {
    Blob = 0,
    Humanoid = 1,
    Quadruped = 2,
    Flying = 3,
}

impl Default for AnimationType {
    fn default() -> Self {
        AnimationType::Blob
    }
}

impl AnimationType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "blob" => AnimationType::Blob,
            "humanoid" => AnimationType::Humanoid,
            "quadruped" => AnimationType::Quadruped,
            "flying" => AnimationType::Flying,
            _ => AnimationType::Blob,
        }
    }
}

// ============================================================================
// Raw TOML Structures (direct deserialization)
// ============================================================================

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RawEntityStats {
    pub level: Option<i32>,
    pub max_hp: Option<i32>,
    pub damage: Option<i32>,
    pub attack_bonus: Option<i32>,
    pub defence_bonus: Option<i32>,
    pub attack_range: Option<i32>,
    pub aggro_range: Option<i32>,
    pub chase_range: Option<i32>,
    pub move_cooldown_ms: Option<u64>,
    pub attack_cooldown_ms: Option<u64>,
    pub respawn_time_ms: Option<u64>,
    pub hp_regen_percent_per_sec: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RawEntityRewards {
    pub exp_base: Option<i32>,
    pub gold_min: Option<i32>,
    pub gold_max: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LootEntry {
    pub item_id: String,
    pub drop_chance: f32,
    #[serde(default = "default_one")]
    pub quantity_min: i32,
    #[serde(default = "default_one")]
    pub quantity_max: i32,
}

fn default_one() -> i32 {
    1
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RawEntityBehaviors {
    #[serde(default)]
    pub hostile: bool,
    #[serde(default)]
    pub merchant: bool,
    #[serde(default)]
    pub quest_giver: bool,
    #[serde(default)]
    pub banker: bool,
    #[serde(default)]
    pub craftsman: bool,
    #[serde(default)]
    pub teleporter: bool,
    #[serde(default)]
    pub altar: bool,
    #[serde(default)]
    pub plot_seller: bool,
    #[serde(default)]
    pub slayer_master: bool,
    #[serde(default)]
    pub koth_rewards: bool,
    #[serde(default)]
    pub friendly: bool,
    #[serde(default)]
    pub wander_enabled: bool,
    pub wander_radius: Option<i32>,
    pub wander_pause_min_ms: Option<u64>,
    pub wander_pause_max_ms: Option<u64>,
    #[serde(default)]
    pub no_shadow: bool,
    pub render_offset_y: Option<f32>,
    /// Initial facing direction (e.g. "down", "up", "left", "right")
    pub facing: Option<String>,
    #[serde(default)]
    pub station_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawSpeechConfig {
    #[serde(default = "default_speech_radius")]
    pub radius: i32,
    #[serde(default = "default_speech_interval_min")]
    pub interval_min_ms: u64,
    #[serde(default = "default_speech_interval_max")]
    pub interval_max_ms: u64,
    #[serde(default)]
    pub messages: Vec<String>,
}

fn default_speech_radius() -> i32 {
    5
}
fn default_speech_interval_min() -> u64 {
    15000
}
fn default_speech_interval_max() -> u64 {
    45000
}

#[derive(Debug, Clone)]
pub struct SpeechConfig {
    pub radius: i32,
    pub interval_min_ms: u64,
    pub interval_max_ms: u64,
    pub messages: Vec<String>,
}

impl From<&RawSpeechConfig> for SpeechConfig {
    fn from(raw: &RawSpeechConfig) -> Self {
        Self {
            radius: raw.radius,
            interval_min_ms: raw.interval_min_ms,
            interval_max_ms: raw.interval_max_ms,
            messages: raw.messages.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MerchantConfig {
    pub shop_id: String,
    #[serde(default = "default_buy_mult")]
    pub buy_multiplier: f32,
    #[serde(default = "default_sell_mult")]
    pub sell_multiplier: f32,
    pub restock_interval_minutes: Option<u32>,
    /// Which recipe categories this merchant offers (e.g. ["smithing"], ["alchemy"])
    /// Empty = no crafting tab shown
    #[serde(default)]
    pub crafting_categories: Vec<String>,
    /// Which crafting stations this merchant provides access to (e.g. ["workbench", "tanning_rack"])
    /// Empty = no station filter (all stations allowed)
    #[serde(default)]
    pub crafting_stations: Vec<String>,
    /// Quest that must be completed before this shop is accessible
    #[serde(default)]
    pub required_quest: Option<String>,
}

fn default_buy_mult() -> f32 {
    0.5
}
fn default_sell_mult() -> f32 {
    1.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct QuestGiverConfig {
    #[serde(default)]
    pub available_quests: Vec<String>,
    pub requires_reputation: Option<ReputationRequirement>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReputationRequirement {
    pub faction: String,
    pub level: i32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DialogueConfig {
    pub greeting: Option<String>,
    pub shop_open: Option<String>,
    pub quest_available: Option<String>,
    pub quest_complete: Option<String>,
}

/// Raw entity prototype as loaded directly from TOML
#[derive(Debug, Clone, Deserialize)]
pub struct RawEntityPrototype {
    pub extends: Option<String>,

    pub display_name: Option<String>,
    pub sprite: Option<String>,
    pub animation_type: Option<String>,
    pub description: Option<String>,

    #[serde(default)]
    pub stats: RawEntityStats,

    #[serde(default)]
    pub rewards: RawEntityRewards,

    #[serde(default)]
    pub loot: Vec<LootEntry>,

    #[serde(default)]
    pub behaviors: RawEntityBehaviors,

    pub merchant: Option<MerchantConfig>,
    pub quest_giver: Option<QuestGiverConfig>,
    pub dialogue: Option<DialogueConfig>,
    pub speech: Option<RawSpeechConfig>,
}

// ============================================================================
// Resolved Structures (after inheritance)
// ============================================================================

#[derive(Debug, Clone)]
pub struct ResolvedStats {
    pub level: i32,
    pub max_hp: i32,
    pub damage: i32,
    pub attack_bonus: i32,
    pub defence_bonus: i32,
    pub attack_range: i32,
    pub aggro_range: i32,
    pub chase_range: i32,
    pub move_cooldown_ms: u64,
    pub attack_cooldown_ms: u64,
    pub respawn_time_ms: u64,
    pub hp_regen_percent_per_sec: f32,
}

impl Default for ResolvedStats {
    fn default() -> Self {
        Self {
            level: 1,
            max_hp: 100,
            damage: 10,
            attack_bonus: 0,
            defence_bonus: 0,
            attack_range: 1,
            aggro_range: 5,
            chase_range: 8,
            move_cooldown_ms: 500,
            attack_cooldown_ms: 800,
            respawn_time_ms: 10000,
            hp_regen_percent_per_sec: 2.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedRewards {
    pub exp_base: i32,
    pub gold_min: i32,
    pub gold_max: i32,
}

impl Default for ResolvedRewards {
    fn default() -> Self {
        Self {
            exp_base: 10,
            gold_min: 1,
            gold_max: 5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EntityBehaviors {
    pub hostile: bool,
    pub merchant: bool,
    pub quest_giver: bool,
    pub banker: bool,
    pub craftsman: bool,
    pub teleporter: bool,
    pub altar: bool,
    pub plot_seller: bool,
    pub slayer_master: bool,
    pub koth_rewards: bool,
    pub friendly: bool,
    pub wander_enabled: bool,
    pub wander_radius: i32,
    pub wander_pause_min_ms: u64,
    pub wander_pause_max_ms: u64,
    pub no_shadow: bool,
    pub render_offset_y: f32,
    /// Initial facing direction (e.g. "down", "up", "left", "right")
    pub facing: Option<String>,
    pub station_type: Option<String>,
}

impl Default for EntityBehaviors {
    fn default() -> Self {
        Self {
            hostile: false,
            merchant: false,
            quest_giver: false,
            banker: false,
            craftsman: false,
            teleporter: false,
            altar: false,
            plot_seller: false,
            slayer_master: false,
            koth_rewards: false,
            friendly: false,
            wander_enabled: false,
            wander_radius: 3,
            wander_pause_min_ms: 2000,
            wander_pause_max_ms: 5000,
            no_shadow: false,
            render_offset_y: 0.0,
            facing: None,
            station_type: None,
        }
    }
}

impl From<&RawEntityBehaviors> for EntityBehaviors {
    fn from(raw: &RawEntityBehaviors) -> Self {
        Self {
            hostile: raw.hostile,
            merchant: raw.merchant,
            quest_giver: raw.quest_giver,
            banker: raw.banker,
            craftsman: raw.craftsman,
            teleporter: raw.teleporter,
            altar: raw.altar,
            plot_seller: raw.plot_seller,
            slayer_master: raw.slayer_master,
            koth_rewards: raw.koth_rewards,
            friendly: raw.friendly,
            wander_enabled: raw.wander_enabled,
            wander_radius: raw.wander_radius.unwrap_or(3),
            wander_pause_min_ms: raw.wander_pause_min_ms.unwrap_or(2000),
            wander_pause_max_ms: raw.wander_pause_max_ms.unwrap_or(5000),
            no_shadow: raw.no_shadow,
            render_offset_y: raw.render_offset_y.unwrap_or(0.0),
            facing: raw.facing.clone(),
            station_type: raw.station_type.clone(),
        }
    }
}

/// Fully resolved entity prototype (after inheritance resolution)
#[derive(Debug, Clone)]
pub struct EntityPrototype {
    pub id: String,
    pub display_name: String,
    pub sprite: String,
    pub animation_type: AnimationType,
    pub description: String,

    pub stats: ResolvedStats,
    pub rewards: ResolvedRewards,
    pub loot: Vec<LootEntry>,

    pub behaviors: EntityBehaviors,
    pub merchant: Option<MerchantConfig>,
    pub quest_giver: Option<QuestGiverConfig>,
    pub dialogue: DialogueConfig,
    pub speech: Option<SpeechConfig>,
}

impl EntityPrototype {
    /// Check if this entity is a hostile monster
    pub fn is_hostile(&self) -> bool {
        self.behaviors.hostile || self.stats.damage > 0
    }

    /// Check if this entity is an NPC with interactions
    pub fn is_npc(&self) -> bool {
        self.behaviors.merchant
            || self.behaviors.quest_giver
            || self.behaviors.banker
            || self.behaviors.craftsman
            || self.behaviors.teleporter
            || self.behaviors.altar
            || self.behaviors.plot_seller
    }
}
