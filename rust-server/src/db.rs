use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions, SqliteJournalMode};
use sqlx::Row;
use crate::quest::state::{PlayerQuestState, QuestProgress, QuestStatus, ObjectiveProgress};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Account data - separate from character data
#[derive(Debug, Clone)]
pub struct AccountData {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub created_at: Option<String>,
    pub last_login: Option<String>,
}

use crate::skills::{Skills, LegacySkills};

/// Character data - belongs to an account
#[derive(Debug, Clone)]
pub struct CharacterData {
    pub id: i64,
    pub account_id: i64,
    pub name: String,
    pub gender: String,         // "male" or "female"
    pub skin: String,           // "tan", "pale", "brown", "purple", "orc", "ghost", "skeleton"
    pub hair_style: Option<i32>, // 0-5 (or None for bald)
    pub hair_color: Option<i32>, // 0-6 (color variant index)
    pub x: f32,
    pub y: f32,
    pub hp: i32,
    pub prayer_points: i32,
    pub skills: Skills,         // Combat skills (Hitpoints, Attack, Strength, Defence)
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
    pub played_time: i64,       // Seconds played
    pub created_at: Option<String>,
    pub is_admin: bool,         // Game Master privileges
    pub current_map: Option<String>, // Interior map ID if player is in an instance (NULL = overworld)
    pub sitting_at_x: Option<i32>,   // Chair tile X if sitting (NULL = not sitting)
    pub sitting_at_y: Option<i32>,   // Chair tile Y if sitting (NULL = not sitting)
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

impl Database {
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let options: SqliteConnectOptions = database_url.parse::<SqliteConnectOptions>()?
            .journal_mode(SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        // Run migrations
        Self::migrate(&pool).await?;

        Ok(Self { pool })
    }

    async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        // Create accounts table (new)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                last_login TEXT
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create characters table (new)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS characters (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id INTEGER NOT NULL,
                name TEXT UNIQUE NOT NULL,
                gender TEXT NOT NULL DEFAULT 'male',
                skin TEXT NOT NULL DEFAULT 'tan',
                x REAL DEFAULT 15.0,
                y REAL DEFAULT 4.0,
                hp INTEGER DEFAULT 10,
                max_hp INTEGER DEFAULT 10,
                level INTEGER DEFAULT 3,
                gold INTEGER DEFAULT 0,
                equipped_head TEXT,
                equipped_body TEXT,
                equipped_weapon TEXT,
                equipped_back TEXT,
                equipped_feet TEXT,
                equipped_ring TEXT,
                equipped_gloves TEXT,
                equipped_necklace TEXT,
                equipped_belt TEXT,
                inventory_json TEXT DEFAULT '[]',
                skills_json TEXT,
                played_time INTEGER DEFAULT 0,
                is_admin BOOLEAN DEFAULT FALSE,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (account_id) REFERENCES accounts(id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Migration: Add skills_json column if it doesn't exist (for existing databases)
        // SQLite doesn't have IF NOT EXISTS for ALTER TABLE, so we check first
        let column_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'skills_json'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !column_exists {
            sqlx::query("ALTER TABLE characters ADD COLUMN skills_json TEXT")
                .execute(pool)
                .await
                .ok(); // Ignore error if column already exists
        }

        // Migration: Add hair_style column if it doesn't exist
        let hair_style_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'hair_style'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !hair_style_exists {
            sqlx::query("ALTER TABLE characters ADD COLUMN hair_style INTEGER DEFAULT NULL")
                .execute(pool)
                .await
                .ok();
        }

        // Migration: Add hair_color column if it doesn't exist
        let hair_color_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'hair_color'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !hair_color_exists {
            sqlx::query("ALTER TABLE characters ADD COLUMN hair_color INTEGER DEFAULT NULL")
                .execute(pool)
                .await
                .ok();
        }

        // Migration: Add current_map column if it doesn't exist
        let current_map_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'current_map'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !current_map_exists {
            sqlx::query("ALTER TABLE characters ADD COLUMN current_map TEXT DEFAULT NULL")
                .execute(pool)
                .await
                .ok();
        }

        // Migration: Add sitting_at columns if they don't exist
        let sitting_at_x_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'sitting_at_x'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !sitting_at_x_exists {
            sqlx::query("ALTER TABLE characters ADD COLUMN sitting_at_x INTEGER DEFAULT NULL")
                .execute(pool)
                .await
                .ok();
            sqlx::query("ALTER TABLE characters ADD COLUMN sitting_at_y INTEGER DEFAULT NULL")
                .execute(pool)
                .await
                .ok();
        }

        // Migration: Add prayer_points column if it doesn't exist
        let prayer_points_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'prayer_points'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !prayer_points_exists {
            sqlx::query("ALTER TABLE characters ADD COLUMN prayer_points INTEGER DEFAULT NULL")
                .execute(pool)
                .await
                .ok();
        }

        // Character quest tables (renamed from player_*)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS character_quests (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                character_id INTEGER NOT NULL,
                quest_id TEXT NOT NULL,
                state TEXT NOT NULL DEFAULT 'active',
                objectives_json TEXT DEFAULT '{}',
                started_at TEXT DEFAULT CURRENT_TIMESTAMP,
                completed_at TEXT,
                FOREIGN KEY(character_id) REFERENCES characters(id),
                UNIQUE(character_id, quest_id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS character_flags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                character_id INTEGER NOT NULL,
                flag_name TEXT NOT NULL,
                flag_value TEXT,
                FOREIGN KEY(character_id) REFERENCES characters(id),
                UNIQUE(character_id, flag_name)
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS character_quest_availability (
                character_id INTEGER NOT NULL,
                quest_id TEXT NOT NULL,
                unlocked_at TEXT DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY(character_id, quest_id),
                FOREIGN KEY(character_id) REFERENCES characters(id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Arena stats table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS arena_stats (
                character_id INTEGER PRIMARY KEY,
                total_wins INTEGER DEFAULT 0,
                total_matches INTEGER DEFAULT 0,
                total_kills INTEGER DEFAULT 0,
                total_deaths INTEGER DEFAULT 0,
                current_streak INTEGER DEFAULT 0,
                best_streak INTEGER DEFAULT 0,
                total_gold_won INTEGER DEFAULT 0,
                FOREIGN KEY(character_id) REFERENCES characters(id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Farming patches table - per-player instanced patch state
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS farming_patches (
                patch_id TEXT NOT NULL,
                player_id TEXT NOT NULL,
                crop_id TEXT NOT NULL,
                planted_at INTEGER NOT NULL,
                PRIMARY KEY (patch_id, player_id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Farming plot unlocks - tracks which plots each player has purchased
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS farming_plot_unlocks (
                player_id TEXT NOT NULL,
                plot_id INTEGER NOT NULL,
                unlocked_at INTEGER NOT NULL,
                PRIMARY KEY (player_id, plot_id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Farming contracts - one active contract per player
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS farming_contracts (
                player_id TEXT PRIMARY KEY,
                difficulty TEXT NOT NULL,
                crop_id TEXT NOT NULL,
                amount_required INTEGER NOT NULL,
                amount_harvested INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Friendships table - for friend system
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS friendships (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                requester_id INTEGER NOT NULL,
                recipient_id INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (requester_id) REFERENCES characters(id),
                FOREIGN KEY (recipient_id) REFERENCES characters(id),
                UNIQUE(requester_id, recipient_id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create index for faster friend lookups
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_friendships_requester ON friendships(requester_id)"
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_friendships_recipient ON friendships(recipient_id)"
        )
        .execute(pool)
        .await?;

        // Discovered recipes table - tracks which recipes a player has discovered
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS discovered_recipes (
                character_id INTEGER NOT NULL,
                recipe_id TEXT NOT NULL,
                discovered_at TEXT DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (character_id, recipe_id),
                FOREIGN KEY (character_id) REFERENCES characters(id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        tracing::info!("Database migrations complete");
        Ok(())
    }

    // =========================================================================
    // Arena Stats Functions
    // =========================================================================

    /// Get arena stats for a character, creating a default row if none exists
    pub async fn get_arena_stats(&self, character_id: i64) -> Result<ArenaStatsData, sqlx::Error> {
        let row = sqlx::query_as::<_, ArenaStatsData>(
            "SELECT character_id, total_wins, total_matches, total_kills, total_deaths, current_streak, best_streak, total_gold_won FROM arena_stats WHERE character_id = ?"
        )
        .bind(character_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.unwrap_or(ArenaStatsData {
            character_id,
            total_wins: 0,
            total_matches: 0,
            total_kills: 0,
            total_deaths: 0,
            current_streak: 0,
            best_streak: 0,
            total_gold_won: 0,
        }))
    }

    /// Update arena stats after a match
    pub async fn update_arena_stats(
        &self,
        character_id: i64,
        won: bool,
        kills: i32,
        died: bool,
        gold_won: i32,
    ) -> Result<(), sqlx::Error> {
        // Upsert: insert or update
        let win_inc = if won { 1 } else { 0 };
        let death_inc = if died { 1 } else { 0 };

        sqlx::query(
            r#"
            INSERT INTO arena_stats (character_id, total_wins, total_matches, total_kills, total_deaths, current_streak, best_streak, total_gold_won)
            VALUES (?, ?, 1, ?, ?, ?, ?, ?)
            ON CONFLICT(character_id) DO UPDATE SET
                total_wins = total_wins + ?,
                total_matches = total_matches + 1,
                total_kills = total_kills + ?,
                total_deaths = total_deaths + ?,
                current_streak = CASE WHEN ? = 1 THEN current_streak + 1 ELSE 0 END,
                best_streak = MAX(best_streak, CASE WHEN ? = 1 THEN current_streak + 1 ELSE 0 END),
                total_gold_won = total_gold_won + ?
            "#,
        )
        // INSERT values
        .bind(character_id)
        .bind(win_inc)
        .bind(kills)
        .bind(death_inc)
        .bind(if won { 1 } else { 0 }) // current_streak
        .bind(if won { 1 } else { 0 }) // best_streak
        .bind(gold_won)
        // UPDATE values
        .bind(win_inc)
        .bind(kills)
        .bind(death_inc)
        .bind(win_inc) // for CASE in current_streak
        .bind(win_inc) // for CASE in best_streak
        .bind(gold_won)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get top 10 arena players for leaderboard
    pub async fn get_arena_leaderboard(&self) -> Result<Vec<(String, i32, i32, i32)>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (String, i32, i32, i32)>(
            r#"
            SELECT c.name, a.total_kills, a.total_wins, a.total_gold_won
            FROM arena_stats a
            JOIN characters c ON c.id = a.character_id
            ORDER BY a.total_wins DESC, a.total_kills DESC
            LIMIT 10
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    // =========================================================================
    // Account CRUD Functions (new)
    // =========================================================================

    /// Create a new account (no character created)
    pub async fn create_account(
        &self,
        username: &str,
        password: &str,
    ) -> Result<i64, String> {
        // Hash the password with Argon2
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| format!("Failed to hash password: {}", e))?
            .to_string();

        let result = sqlx::query(
            "INSERT INTO accounts (username, password_hash) VALUES (?, ?)",
        )
        .bind(username)
        .bind(&password_hash)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint failed") {
                "Username already exists".to_string()
            } else {
                format!("Database error: {}", e)
            }
        })?;

        let account_id = result.last_insert_rowid();
        tracing::info!("Created account: {} (id: {})", username, account_id);
        Ok(account_id)
    }

    /// Verify account password and return account data if valid
    pub async fn verify_account_password(&self, username: &str, password: &str) -> Option<AccountData> {
        let row = sqlx::query(
            "SELECT id, username, password_hash, created_at, last_login FROM accounts WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .ok()??;

        let account = AccountData {
            id: row.get("id"),
            username: row.get("username"),
            password_hash: row.get("password_hash"),
            created_at: row.get("created_at"),
            last_login: row.get("last_login"),
        };

        // Verify password
        if let Ok(parsed_hash) = PasswordHash::new(&account.password_hash) {
            if Argon2::default()
                .verify_password(password.as_bytes(), &parsed_hash)
                .is_ok()
            {
                // Update last login time
                let _ = sqlx::query("UPDATE accounts SET last_login = CURRENT_TIMESTAMP WHERE id = ?")
                    .bind(account.id)
                    .execute(&self.pool)
                    .await;
                return Some(account);
            }
        }
        None
    }

    // =========================================================================
    // Character CRUD Functions (new)
    // =========================================================================

    /// Get all characters for an account
    pub async fn get_characters_for_account(&self, account_id: i64) -> Result<Vec<CharacterData>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT id, account_id, name, gender, skin, hair_style, hair_color, x, y, hp, prayer_points, gold,
                equipped_head, equipped_body, equipped_weapon, equipped_back, equipped_feet,
                equipped_ring, equipped_gloves, equipped_necklace, equipped_belt,
                inventory_json, skills_json, played_time, is_admin, created_at, current_map,
                sitting_at_x, sitting_at_y
            FROM characters WHERE account_id = ? ORDER BY created_at DESC"#,
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| {
            // Try to load skills from JSON column, with legacy format migration
            let skills = r.try_get::<String, _>("skills_json")
                .ok()
                .and_then(|json| {
                    // First try the new format (hitpoints + combat)
                    if let Ok(skills) = serde_json::from_str::<Skills>(&json) {
                        return Some(skills);
                    }
                    // Fall back to legacy format (hitpoints + attack + strength + defence)
                    // and convert to new format by summing combat XP
                    if let Ok(legacy) = serde_json::from_str::<LegacySkills>(&json) {
                        return Some(legacy.to_skills());
                    }
                    None
                })
                .unwrap_or_else(Skills::new);

            CharacterData {
                id: r.get("id"),
                account_id: r.get("account_id"),
                name: r.get("name"),
                gender: r.get("gender"),
                skin: r.get("skin"),
                hair_style: r.try_get::<Option<i32>, _>("hair_style").unwrap_or(None),
                hair_color: r.try_get::<Option<i32>, _>("hair_color").unwrap_or(None),
                x: r.get("x"),
                y: r.get("y"),
                hp: r.get("hp"),
                prayer_points: r.try_get::<Option<i32>, _>("prayer_points").unwrap_or(None).unwrap_or(10 + skills.prayer.level),
                skills,
                gold: r.get("gold"),
                equipped_head: r.try_get::<String, _>("equipped_head").ok().filter(|s| !s.is_empty()),
                equipped_body: r.try_get::<String, _>("equipped_body").ok().filter(|s| !s.is_empty()),
                equipped_weapon: r.try_get::<String, _>("equipped_weapon").ok().filter(|s| !s.is_empty()),
                equipped_back: r.try_get::<String, _>("equipped_back").ok().filter(|s| !s.is_empty()),
                equipped_feet: r.try_get::<String, _>("equipped_feet").ok().filter(|s| !s.is_empty()),
                equipped_ring: r.try_get::<String, _>("equipped_ring").ok().filter(|s| !s.is_empty()),
                equipped_gloves: r.try_get::<String, _>("equipped_gloves").ok().filter(|s| !s.is_empty()),
                equipped_necklace: r.try_get::<String, _>("equipped_necklace").ok().filter(|s| !s.is_empty()),
                equipped_belt: r.try_get::<String, _>("equipped_belt").ok().filter(|s| !s.is_empty()),
                inventory_json: r.get("inventory_json"),
                played_time: r.get("played_time"),
                created_at: r.get("created_at"),
                is_admin: r.try_get::<bool, _>("is_admin").unwrap_or(false),
                current_map: r.try_get::<Option<String>, _>("current_map").unwrap_or(None),
                sitting_at_x: r.try_get::<Option<i32>, _>("sitting_at_x").unwrap_or(None),
                sitting_at_y: r.try_get::<Option<i32>, _>("sitting_at_y").unwrap_or(None),
            }
        }).collect())
    }

    /// Check if a character name is already taken (case-insensitive, globally unique)
    pub async fn is_character_name_taken(&self, name: &str) -> Result<bool, sqlx::Error> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM characters WHERE LOWER(name) = LOWER(?)")
            .bind(name)
            .fetch_one(&self.pool)
            .await?;
        let count: i64 = row.get("count");
        Ok(count > 0)
    }

    /// Count characters for an account
    pub async fn count_characters_for_account(&self, account_id: i64) -> Result<i64, sqlx::Error> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM characters WHERE account_id = ?")
            .bind(account_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get("count"))
    }

    /// Create a new character for an account
    pub async fn create_character(
        &self,
        account_id: i64,
        name: &str,
        gender: &str,
        skin: &str,
        hair_style: Option<i32>,
        hair_color: Option<i32>,
    ) -> Result<CharacterData, String> {
        // Validate gender and skin
        if !GENDERS.contains(&gender) {
            return Err(format!("Invalid gender: {}", gender));
        }
        if !SKINS.contains(&skin) {
            return Err(format!("Invalid skin: {}", skin));
        }

        // Validate hair_style (0-5) and hair_color (0-9) if provided
        if let Some(style) = hair_style {
            if style < 0 || style > 5 {
                return Err(format!("Invalid hair style: {} (must be 0-5)", style));
            }
        }
        if let Some(color) = hair_color {
            if color < 0 || color > 9 {
                return Err(format!("Invalid hair color: {} (must be 0-9)", color));
            }
        }

        // Check if name is already taken (case-insensitive)
        match self.is_character_name_taken(name).await {
            Ok(true) => return Err("Character name already exists".to_string()),
            Ok(false) => {}
            Err(e) => return Err(format!("Database error checking name: {}", e)),
        }

        // Starting equipment for new characters (Tier 0 Cursed Lands gear)
        let starting_weapon = "chain";
        let starting_body = if gender == "female" { "peasant_suit_female" } else { "torn_clothes" };
        let starting_feet = "worn_sandals";
        let starting_gold = 25;

        let result = sqlx::query(
            r#"INSERT INTO characters
               (account_id, name, gender, skin, hair_style, hair_color,
                equipped_weapon, equipped_body, equipped_feet, gold, x, y)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 15.0, 4.0)"#,
        )
        .bind(account_id)
        .bind(name)
        .bind(gender)
        .bind(skin)
        .bind(hair_style)
        .bind(hair_color)
        .bind(starting_weapon)
        .bind(starting_body)
        .bind(starting_feet)
        .bind(starting_gold)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint failed") {
                "Character name already exists".to_string()
            } else {
                format!("Database error: {}", e)
            }
        })?;

        let character_id = result.last_insert_rowid();
        tracing::info!("Created character: {} (id: {}) for account {} with starting gear (pitchfork, clothes, sandals, {}g)",
            name, character_id, account_id, starting_gold);

        // Fetch and return the created character
        self.get_character(character_id).await
            .map_err(|e| format!("Failed to fetch created character: {}", e))?
            .ok_or_else(|| "Failed to find created character".to_string())
    }

    /// Get a character by ID
    pub async fn get_character(&self, character_id: i64) -> Result<Option<CharacterData>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT id, account_id, name, gender, skin, hair_style, hair_color, x, y, hp, prayer_points, gold,
                equipped_head, equipped_body, equipped_weapon, equipped_back, equipped_feet,
                equipped_ring, equipped_gloves, equipped_necklace, equipped_belt,
                inventory_json, skills_json, played_time, is_admin, created_at, current_map,
                sitting_at_x, sitting_at_y
            FROM characters WHERE id = ?"#,
        )
        .bind(character_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| {
            // Try to load skills from JSON column, with legacy format migration
            let skills = r.try_get::<String, _>("skills_json")
                .ok()
                .and_then(|json| {
                    // First try the new format (hitpoints + combat)
                    if let Ok(skills) = serde_json::from_str::<Skills>(&json) {
                        return Some(skills);
                    }
                    // Fall back to legacy format (hitpoints + attack + strength + defence)
                    // and convert to new format by summing combat XP
                    if let Ok(legacy) = serde_json::from_str::<LegacySkills>(&json) {
                        return Some(legacy.to_skills());
                    }
                    None
                })
                .unwrap_or_else(Skills::new);

            CharacterData {
                id: r.get("id"),
                account_id: r.get("account_id"),
                name: r.get("name"),
                gender: r.get("gender"),
                skin: r.get("skin"),
                hair_style: r.try_get::<Option<i32>, _>("hair_style").unwrap_or(None),
                hair_color: r.try_get::<Option<i32>, _>("hair_color").unwrap_or(None),
                x: r.get("x"),
                y: r.get("y"),
                hp: r.get("hp"),
                prayer_points: r.try_get::<Option<i32>, _>("prayer_points").unwrap_or(None).unwrap_or(10 + skills.prayer.level),
                skills,
                gold: r.get("gold"),
                equipped_head: r.try_get::<String, _>("equipped_head").ok().filter(|s| !s.is_empty()),
                equipped_body: r.try_get::<String, _>("equipped_body").ok().filter(|s| !s.is_empty()),
                equipped_weapon: r.try_get::<String, _>("equipped_weapon").ok().filter(|s| !s.is_empty()),
                equipped_back: r.try_get::<String, _>("equipped_back").ok().filter(|s| !s.is_empty()),
                equipped_feet: r.try_get::<String, _>("equipped_feet").ok().filter(|s| !s.is_empty()),
                equipped_ring: r.try_get::<String, _>("equipped_ring").ok().filter(|s| !s.is_empty()),
                equipped_gloves: r.try_get::<String, _>("equipped_gloves").ok().filter(|s| !s.is_empty()),
                equipped_necklace: r.try_get::<String, _>("equipped_necklace").ok().filter(|s| !s.is_empty()),
                equipped_belt: r.try_get::<String, _>("equipped_belt").ok().filter(|s| !s.is_empty()),
                inventory_json: r.get("inventory_json"),
                played_time: r.get("played_time"),
                created_at: r.get("created_at"),
                is_admin: r.try_get::<bool, _>("is_admin").unwrap_or(false),
                current_map: r.try_get::<Option<String>, _>("current_map").unwrap_or(None),
                sitting_at_x: r.try_get::<Option<i32>, _>("sitting_at_x").unwrap_or(None),
                sitting_at_y: r.try_get::<Option<i32>, _>("sitting_at_y").unwrap_or(None),
            }
        }))
    }

    /// Delete a character (with ownership verification)
    pub async fn delete_character(&self, character_id: i64, account_id: i64) -> Result<bool, sqlx::Error> {
        // Delete related quest data first
        sqlx::query("DELETE FROM character_quests WHERE character_id = ?")
            .bind(character_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM character_flags WHERE character_id = ?")
            .bind(character_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM character_quest_availability WHERE character_id = ?")
            .bind(character_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM arena_stats WHERE character_id = ?")
            .bind(character_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM friendships WHERE requester_id = ? OR recipient_id = ?")
            .bind(character_id)
            .bind(character_id)
            .execute(&self.pool)
            .await?;

        // Delete the character (only if owned by this account)
        let result = sqlx::query("DELETE FROM characters WHERE id = ? AND account_id = ?")
            .bind(character_id)
            .bind(account_id)
            .execute(&self.pool)
            .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            tracing::info!("Deleted character {} for account {}", character_id, account_id);
        }
        Ok(deleted)
    }

    /// Save character data
    pub async fn save_character(
        &self,
        character_id: i64,
        x: f32,
        y: f32,
        hp: i32,
        prayer_points: i32,
        skills: &Skills,
        gold: i32,
        inventory_json: &str,
        equipped_head: Option<&str>,
        equipped_body: Option<&str>,
        equipped_weapon: Option<&str>,
        equipped_back: Option<&str>,
        equipped_feet: Option<&str>,
        equipped_ring: Option<&str>,
        equipped_gloves: Option<&str>,
        equipped_necklace: Option<&str>,
        equipped_belt: Option<&str>,
        played_time_delta: i64,
        current_map: Option<&str>,
        sitting_at_x: Option<i32>,
        sitting_at_y: Option<i32>,
    ) -> Result<(), sqlx::Error> {
        // Serialize skills to JSON for the skills_json column
        let skills_json = serde_json::to_string(skills).unwrap_or_else(|_| "{}".to_string());

        // For backward compatibility, we also write to legacy columns with derived values
        let max_hp = skills.hitpoints.level;
        let level = skills.combat_level();

        sqlx::query(
            r#"UPDATE characters SET
                x = ?, y = ?, hp = ?, prayer_points = ?, max_hp = ?, level = ?,
                gold = ?, inventory_json = ?, skills_json = ?,
                equipped_head = ?, equipped_body = ?, equipped_weapon = ?,
                equipped_back = ?, equipped_feet = ?, equipped_ring = ?,
                equipped_gloves = ?, equipped_necklace = ?, equipped_belt = ?,
                played_time = played_time + ?,
                current_map = ?,
                sitting_at_x = ?,
                sitting_at_y = ?
            WHERE id = ?"#,
        )
        .bind(x)
        .bind(y)
        .bind(hp)
        .bind(prayer_points)
        .bind(max_hp)
        .bind(level)
        .bind(gold)
        .bind(inventory_json)
        .bind(&skills_json)
        .bind(equipped_head)
        .bind(equipped_body)
        .bind(equipped_weapon)
        .bind(equipped_back)
        .bind(equipped_feet)
        .bind(equipped_ring)
        .bind(equipped_gloves)
        .bind(equipped_necklace)
        .bind(equipped_belt)
        .bind(played_time_delta)
        .bind(current_map)
        .bind(sitting_at_x)
        .bind(sitting_at_y)
        .bind(character_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // =========================================================================
    // Character Quest State Functions (new - uses character_id)
    // =========================================================================

    /// Load quest state for a character from database
    pub async fn load_character_quest_state(&self, character_id: i64) -> Result<PlayerQuestState, sqlx::Error> {
        let mut state = PlayerQuestState::new();

        // Load quests from character_quests table
        let quest_rows = sqlx::query(
            "SELECT quest_id, state, objectives_json, started_at, completed_at FROM character_quests WHERE character_id = ?"
        )
        .bind(character_id)
        .fetch_all(&self.pool)
        .await?;

        for row in quest_rows {
            let quest_id: String = row.get("quest_id");
            let state_str: String = row.get("state");
            let objectives_json: String = row.get("objectives_json");
            let started_at: Option<String> = row.get("started_at");
            let completed_at: Option<String> = row.get("completed_at");

            let status = QuestStatus::from_str(&state_str).unwrap_or(QuestStatus::Active);

            // Parse timestamps
            let started_at_dt = started_at.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc)));
            let completed_at_dt = completed_at.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc)));

            // Deserialize objectives
            let objectives = QuestProgress::objectives_from_json(&objectives_json);

            let progress = QuestProgress {
                quest_id: quest_id.clone(),
                status,
                objectives,
                started_at: started_at_dt,
                completed_at: completed_at_dt,
            };

            match status {
                QuestStatus::Completed => {
                    state.completed_quests.push(quest_id);
                }
                QuestStatus::Active | QuestStatus::ReadyToComplete => {
                    state.active_quests.insert(quest_id, progress);
                }
                _ => {
                    // Failed/Abandoned quests are stored but not active
                }
            }
        }

        // Load available quests
        let available_rows = sqlx::query(
            "SELECT quest_id FROM character_quest_availability WHERE character_id = ?"
        )
        .bind(character_id)
        .fetch_all(&self.pool)
        .await?;

        for row in available_rows {
            let quest_id: String = row.get("quest_id");
            // Only add if not already active or completed
            if !state.active_quests.contains_key(&quest_id) && !state.completed_quests.contains(&quest_id) {
                state.available_quests.push(quest_id);
            }
        }

        // Load flags
        let flag_rows = sqlx::query(
            "SELECT flag_name, flag_value FROM character_flags WHERE character_id = ?"
        )
        .bind(character_id)
        .fetch_all(&self.pool)
        .await?;

        for row in flag_rows {
            let flag_name: String = row.get("flag_name");
            let flag_value: Option<String> = row.get("flag_value");
            if let Some(value) = flag_value {
                state.flags.insert(flag_name, value);
            }
        }

        tracing::debug!("Loaded quest state for character {}: {} active, {} completed, {} available",
            character_id,
            state.active_quests.len(),
            state.completed_quests.len(),
            state.available_quests.len()
        );

        Ok(state)
    }

    /// Save quest state for a character to database
    pub async fn save_character_quest_state(&self, character_id: i64, state: &PlayerQuestState) -> Result<(), sqlx::Error> {
        // Save active quests
        for (quest_id, progress) in &state.active_quests {
            let objectives_json = progress.objectives_to_json();
            let started_at = progress.started_at.map(|dt| dt.to_rfc3339());
            let completed_at = progress.completed_at.map(|dt| dt.to_rfc3339());

            sqlx::query(
                r#"INSERT INTO character_quests (character_id, quest_id, state, objectives_json, started_at, completed_at)
                   VALUES (?, ?, ?, ?, ?, ?)
                   ON CONFLICT(character_id, quest_id) DO UPDATE SET
                       state = excluded.state,
                       objectives_json = excluded.objectives_json,
                       started_at = excluded.started_at,
                       completed_at = excluded.completed_at"#
            )
            .bind(character_id)
            .bind(quest_id)
            .bind(progress.status.as_str())
            .bind(&objectives_json)
            .bind(&started_at)
            .bind(&completed_at)
            .execute(&self.pool)
            .await?;
        }

        // Save completed quests
        for quest_id in &state.completed_quests {
            sqlx::query(
                r#"INSERT INTO character_quests (character_id, quest_id, state, completed_at)
                   VALUES (?, ?, 'completed', CURRENT_TIMESTAMP)
                   ON CONFLICT(character_id, quest_id) DO UPDATE SET
                       state = 'completed',
                       completed_at = CURRENT_TIMESTAMP"#
            )
            .bind(character_id)
            .bind(quest_id)
            .execute(&self.pool)
            .await?;
        }

        // Save available quests
        for quest_id in &state.available_quests {
            sqlx::query(
                r#"INSERT OR IGNORE INTO character_quest_availability (character_id, quest_id)
                   VALUES (?, ?)"#
            )
            .bind(character_id)
            .bind(quest_id)
            .execute(&self.pool)
            .await?;
        }

        // Save flags
        for (flag_name, flag_value) in &state.flags {
            sqlx::query(
                r#"INSERT INTO character_flags (character_id, flag_name, flag_value)
                   VALUES (?, ?, ?)
                   ON CONFLICT(character_id, flag_name) DO UPDATE SET
                       flag_value = excluded.flag_value"#
            )
            .bind(character_id)
            .bind(flag_name)
            .bind(flag_value)
            .execute(&self.pool)
            .await?;
        }

        tracing::debug!("Saved quest state for character {}: {} active, {} completed",
            character_id,
            state.active_quests.len(),
            state.completed_quests.len()
        );

        Ok(())
    }

    // =========================================================================
    // Farming Patch Persistence
    // =========================================================================

    /// Save a planted farming patch to the database (per-player instanced)
    pub async fn save_farming_patch(
        &self,
        patch_id: &str,
        player_id: &str,
        crop_id: &str,
        planted_at: u64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO farming_patches (patch_id, player_id, crop_id, planted_at)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(patch_id)
        .bind(player_id)
        .bind(crop_id)
        .bind(planted_at as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete a farming patch for a specific player (after harvest or reset)
    pub async fn delete_farming_patch(&self, patch_id: &str, player_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM farming_patches WHERE patch_id = ? AND player_id = ?")
            .bind(patch_id)
            .bind(player_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Load all planted farming patches from the database
    pub async fn load_farming_patches(&self) -> Result<Vec<(String, String, String, u64)>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT patch_id, player_id, crop_id, planted_at FROM farming_patches"
        )
        .fetch_all(&self.pool)
        .await?;

        let patches = rows.iter().map(|row| {
            let patch_id: String = row.get("patch_id");
            let player_id: String = row.get("player_id");
            let crop_id: String = row.get("crop_id");
            let planted_at: i64 = row.get("planted_at");
            (patch_id, player_id, crop_id, planted_at as u64)
        }).collect();

        Ok(patches)
    }

    // =========================================================================
    // Farming Plot Unlock Persistence
    // =========================================================================

    /// Save a plot unlock for a player
    pub async fn save_plot_unlock(
        &self,
        player_id: &str,
        plot_id: u32,
    ) -> Result<(), sqlx::Error> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO farming_plot_unlocks (player_id, plot_id, unlocked_at)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(player_id)
        .bind(plot_id as i32)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Load all plot unlocks from the database
    pub async fn load_plot_unlocks(&self) -> Result<Vec<(String, u32)>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT player_id, plot_id FROM farming_plot_unlocks"
        )
        .fetch_all(&self.pool)
        .await?;

        let unlocks = rows.iter().map(|row| {
            let player_id: String = row.get("player_id");
            let plot_id: i32 = row.get("plot_id");
            (player_id, plot_id as u32)
        }).collect();

        Ok(unlocks)
    }

    // =========================================================================
    // Farming Contract Persistence
    // =========================================================================

    /// Save a new farming contract
    pub async fn save_farming_contract(
        &self,
        player_id: &str,
        difficulty: &str,
        crop_id: &str,
        amount_required: i32,
        amount_harvested: i32,
        created_at: u64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO farming_contracts (player_id, difficulty, crop_id, amount_required, amount_harvested, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(player_id)
        .bind(difficulty)
        .bind(crop_id)
        .bind(amount_required)
        .bind(amount_harvested)
        .bind(created_at as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update contract progress
    pub async fn update_farming_contract_progress(
        &self,
        player_id: &str,
        amount_harvested: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE farming_contracts SET amount_harvested = ? WHERE player_id = ?"
        )
        .bind(amount_harvested)
        .bind(player_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete a farming contract (on completion or abandonment)
    pub async fn delete_farming_contract(&self, player_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM farming_contracts WHERE player_id = ?")
            .bind(player_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Load all active farming contracts
    pub async fn load_farming_contracts(&self) -> Result<Vec<(String, String, String, i32, i32, u64)>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT player_id, difficulty, crop_id, amount_required, amount_harvested, created_at FROM farming_contracts"
        )
        .fetch_all(&self.pool)
        .await?;

        let contracts = rows.iter().map(|row| {
            let player_id: String = row.get("player_id");
            let difficulty: String = row.get("difficulty");
            let crop_id: String = row.get("crop_id");
            let amount_required: i32 = row.get("amount_required");
            let amount_harvested: i32 = row.get("amount_harvested");
            let created_at: i64 = row.get("created_at");
            (player_id, difficulty, crop_id, amount_required, amount_harvested, created_at as u64)
        }).collect();

        Ok(contracts)
    }

    // =========================================================================
    // Discovered Recipes Functions
    // =========================================================================

    /// Load discovered recipes for a character
    pub async fn load_discovered_recipes(&self, character_id: i64) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT recipe_id FROM discovered_recipes WHERE character_id = ?"
        )
        .bind(character_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|row| row.get("recipe_id")).collect())
    }

    /// Save a newly discovered recipe for a character
    pub async fn save_discovered_recipe(
        &self,
        character_id: i64,
        recipe_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO discovered_recipes (character_id, recipe_id)
            VALUES (?, ?)
            "#,
        )
        .bind(character_id)
        .bind(recipe_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // =========================================================================
    // Friend System Functions
    // =========================================================================

    /// Send a friend request (creates pending friendship)
    pub async fn create_friend_request(
        &self,
        requester_id: i64,
        recipient_id: i64,
    ) -> Result<(), String> {
        // Check if friendship already exists in either direction
        let existing = sqlx::query(
            r#"SELECT id FROM friendships
               WHERE (requester_id = ? AND recipient_id = ?)
                  OR (requester_id = ? AND recipient_id = ?)"#
        )
        .bind(requester_id)
        .bind(recipient_id)
        .bind(recipient_id)
        .bind(requester_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if existing.is_some() {
            return Err("Friend request already exists or you are already friends".to_string());
        }

        sqlx::query(
            "INSERT INTO friendships (requester_id, recipient_id, status) VALUES (?, ?, 'pending')"
        )
        .bind(requester_id)
        .bind(recipient_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        tracing::info!("Friend request created: {} -> {}", requester_id, recipient_id);
        Ok(())
    }

    /// Accept a friend request (changes status to accepted)
    pub async fn accept_friend_request(
        &self,
        requester_id: i64,
        recipient_id: i64,
    ) -> Result<(), String> {
        let result = sqlx::query(
            "UPDATE friendships SET status = 'accepted' WHERE requester_id = ? AND recipient_id = ? AND status = 'pending'"
        )
        .bind(requester_id)
        .bind(recipient_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if result.rows_affected() == 0 {
            return Err("No pending friend request found".to_string());
        }

        tracing::info!("Friend request accepted: {} <- {}", requester_id, recipient_id);
        Ok(())
    }

    /// Decline a friend request (deletes the pending friendship)
    pub async fn decline_friend_request(
        &self,
        requester_id: i64,
        recipient_id: i64,
    ) -> Result<(), String> {
        let result = sqlx::query(
            "DELETE FROM friendships WHERE requester_id = ? AND recipient_id = ? AND status = 'pending'"
        )
        .bind(requester_id)
        .bind(recipient_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if result.rows_affected() == 0 {
            return Err("No pending friend request found".to_string());
        }

        tracing::info!("Friend request declined: {} <- {}", requester_id, recipient_id);
        Ok(())
    }

    /// Remove a friend (deletes the accepted friendship in either direction)
    pub async fn remove_friend(
        &self,
        character_id: i64,
        friend_id: i64,
    ) -> Result<(), String> {
        let result = sqlx::query(
            r#"DELETE FROM friendships
               WHERE status = 'accepted'
                 AND ((requester_id = ? AND recipient_id = ?)
                   OR (requester_id = ? AND recipient_id = ?))"#
        )
        .bind(character_id)
        .bind(friend_id)
        .bind(friend_id)
        .bind(character_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if result.rows_affected() == 0 {
            return Err("Friendship not found".to_string());
        }

        tracing::info!("Friendship removed: {} <-> {}", character_id, friend_id);
        Ok(())
    }

    /// Get all accepted friends for a character (returns character_id and name)
    pub async fn get_friends_list(&self, character_id: i64) -> Result<Vec<(i64, String)>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT c.id, c.name FROM characters c
               INNER JOIN friendships f ON
                   (f.requester_id = ? AND f.recipient_id = c.id AND f.status = 'accepted')
                OR (f.recipient_id = ? AND f.requester_id = c.id AND f.status = 'accepted')"#
        )
        .bind(character_id)
        .bind(character_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| {
            (r.get("id"), r.get("name"))
        }).collect())
    }

    /// Get pending friend requests received by a character (returns requester_id and name)
    pub async fn get_pending_requests(&self, character_id: i64) -> Result<Vec<(i64, String)>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT c.id, c.name FROM characters c
               INNER JOIN friendships f ON f.requester_id = c.id
               WHERE f.recipient_id = ? AND f.status = 'pending'"#
        )
        .bind(character_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| {
            (r.get("id"), r.get("name"))
        }).collect())
    }

    /// Get character ID by name (for sending friend requests by name)
    pub async fn get_character_id_by_name(&self, name: &str) -> Result<Option<i64>, sqlx::Error> {
        let row = sqlx::query("SELECT id FROM characters WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| r.get("id")))
    }

    /// Get character name by ID
    pub async fn get_character_name_by_id(&self, id: i64) -> Result<Option<String>, sqlx::Error> {
        let row = sqlx::query("SELECT name FROM characters WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| r.get("name")))
    }

    /// Check if two characters are friends
    pub async fn are_friends(&self, char1_id: i64, char2_id: i64) -> Result<bool, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT id FROM friendships
               WHERE status = 'accepted'
                 AND ((requester_id = ? AND recipient_id = ?)
                   OR (requester_id = ? AND recipient_id = ?))"#
        )
        .bind(char1_id)
        .bind(char2_id)
        .bind(char2_id)
        .bind(char1_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.is_some())
    }

    /// Check if there's a pending request from requester to recipient
    pub async fn has_pending_request(&self, requester_id: i64, recipient_id: i64) -> Result<bool, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id FROM friendships WHERE requester_id = ? AND recipient_id = ? AND status = 'pending'"
        )
        .bind(requester_id)
        .bind(recipient_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.is_some())
    }
}
