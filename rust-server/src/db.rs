use crate::quest::state::{PlayerQuestState, QuestProgress, QuestStatus};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use chrono::{DateTime, Utc};
use sqlx::Row;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

mod accounts;
mod chain;
mod characters;
mod crafting_orders;
mod farming_contracts;
mod moderation_collection;
mod rewards_arena;
mod setup;
mod social;
mod unlocks;
mod world_state;

/// Account data - separate from character data
#[derive(Debug, Clone)]
pub struct AccountData {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub created_at: Option<String>,
    pub last_login: Option<String>,
}

use crate::skills::Skills;

/// Character data - belongs to an account
#[derive(Debug, Clone)]
pub struct CharacterData {
    pub id: i64,
    pub account_id: i64,
    pub name: String,
    pub gender: String,          // "male" or "female"
    pub skin: String,            // "tan", "pale", "brown", "purple", "orc", "ghost", "skeleton"
    pub hair_style: Option<i32>, // 0-5 (or None for bald)
    pub hair_color: Option<i32>, // 0-6 (color variant index)
    pub x: f32,
    pub y: f32,
    pub z: i32,
    pub hp: i32,
    pub prayer_points: i32,
    pub mp: i32,
    pub skills: Skills, // Combat skills (Hitpoints, Attack, Strength, Defence)
    pub gold: i32,
    pub inventory_json: String, // JSON serialized inventory
    // Equipment slots
    pub equipped_head: Option<String>,
    pub equipped_body: Option<String>,
    pub equipped_weapon: Option<String>,
    pub equipped_back: Option<String>,
    pub equipped_feet: Option<String>,
    pub equipped_ring: Option<String>,
    pub equipped_gloves: Option<String>,
    pub equipped_necklace: Option<String>,
    pub equipped_belt: Option<String>,
    pub played_time: i64, // Seconds played
    pub created_at: Option<String>,
    pub is_admin: bool,              // Game Master privileges
    pub current_map: Option<String>, // Interior map ID if player is in an instance (NULL = overworld)
    pub sitting_at_x: Option<i32>,   // Chair tile X if sitting (NULL = not sitting)
    pub sitting_at_y: Option<i32>,   // Chair tile Y if sitting (NULL = not sitting)
    pub entrance_x: Option<f32>,     // Overworld X where player entered interior (for exit)
    pub entrance_y: Option<f32>,     // Overworld Y where player entered interior (for exit)
    pub bank_json: String,           // JSON serialized bank vault contents
    pub bank_gold: i32,              // Gold stored in bank
    pub bank_max_slots: u32,         // Current max bank slots (upgradeable)
    pub combat_style_prefs: String,  // JSON: per-weapon-type style preferences
}

// Available appearance options
pub const GENDERS: &[&str] = &["male", "female"];
pub const SKINS: &[&str] = &["tan", "pale", "brown", "fish", "orc", "panda", "skeleton"];

/// Arena stats data from the database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ArenaStatsData {
    pub character_id: i64,
    pub total_wins: i32,
    pub total_matches: i32,
    pub total_kills: i32,
    pub total_deaths: i32,
    pub current_streak: i32,
    pub best_streak: i32,
    pub total_gold_won: i32,
}

pub struct Database {
    pool: SqlitePool,
}
