use super::*;

impl Database {
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let options: SqliteConnectOptions = database_url
            .parse::<SqliteConnectOptions>()?
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5))
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
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
                x REAL DEFAULT -30.0,
                y REAL DEFAULT 19.0,
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
                monster_kills INTEGER DEFAULT 0,
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
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'skills_json'",
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

        // Migration: Add monster_kills column if it doesn't exist
        let monster_kills_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'monster_kills'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !monster_kills_exists {
            sqlx::query("ALTER TABLE characters ADD COLUMN monster_kills INTEGER DEFAULT 0")
                .execute(pool)
                .await
                .ok();
        }

        // Migration: Add hair_style column if it doesn't exist
        let hair_style_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'hair_style'",
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
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'hair_color'",
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
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'current_map'",
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
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'sitting_at_x'",
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

        // Migration: Add entrance position columns if they don't exist
        let entrance_x_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'entrance_x'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !entrance_x_exists {
            sqlx::query("ALTER TABLE characters ADD COLUMN entrance_x REAL DEFAULT NULL")
                .execute(pool)
                .await
                .ok();
            sqlx::query("ALTER TABLE characters ADD COLUMN entrance_y REAL DEFAULT NULL")
                .execute(pool)
                .await
                .ok();
        }

        // Migration: Add prayer_points column if it doesn't exist
        let prayer_points_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'prayer_points'",
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

        // Migration: Add mp (mana points) column if it doesn't exist
        let mp_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'mp'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !mp_exists {
            sqlx::query("ALTER TABLE characters ADD COLUMN mp INTEGER DEFAULT NULL")
                .execute(pool)
                .await
                .ok();
        }

        // Migration: Add bank_json column if it doesn't exist
        let bank_json_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'bank_json'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !bank_json_exists {
            sqlx::query("ALTER TABLE characters ADD COLUMN bank_json TEXT DEFAULT '[]'")
                .execute(pool)
                .await
                .ok();
            sqlx::query("ALTER TABLE characters ADD COLUMN bank_gold INTEGER DEFAULT 0")
                .execute(pool)
                .await
                .ok();
        }

        // Migration: Add bank_max_slots column if it doesn't exist
        let bank_max_slots_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'bank_max_slots'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !bank_max_slots_exists {
            sqlx::query("ALTER TABLE characters ADD COLUMN bank_max_slots INTEGER DEFAULT 50")
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

        // Resource contracts - one active cross-skill contract per player
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS resource_contracts (
                player_id TEXT PRIMARY KEY,
                contract_kind TEXT NOT NULL,
                difficulty TEXT NOT NULL,
                target_item_id TEXT NOT NULL,
                target_name TEXT NOT NULL,
                amount_required INTEGER NOT NULL,
                amount_completed INTEGER NOT NULL DEFAULT 0,
                giver_npc_id TEXT NOT NULL,
                giver_name TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS resource_contract_stats (
                player_id TEXT PRIMARY KEY,
                contracts_completed INTEGER NOT NULL DEFAULT 0,
                total_gold_earned INTEGER NOT NULL DEFAULT 0,
                total_xp_earned INTEGER NOT NULL DEFAULT 0
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Daily contract limit columns
        sqlx::query("ALTER TABLE resource_contract_stats ADD COLUMN daily_completed INTEGER NOT NULL DEFAULT 0")
            .execute(pool)
            .await
            .ok();
        sqlx::query(
            "ALTER TABLE resource_contract_stats ADD COLUMN daily_date TEXT NOT NULL DEFAULT ''",
        )
        .execute(pool)
        .await
        .ok();

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
            "CREATE INDEX IF NOT EXISTS idx_friendships_requester ON friendships(requester_id)",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_friendships_recipient ON friendships(recipient_id)",
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

        // Unlocked spells table - tracks which scroll spells a player has learned
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS character_unlocked_spells (
                character_id INTEGER NOT NULL,
                spell_id TEXT NOT NULL,
                unlocked_at TEXT DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (character_id, spell_id),
                FOREIGN KEY (character_id) REFERENCES characters(id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Slayer state table - JSON blob per character
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS character_slayer (
                character_id INTEGER PRIMARY KEY,
                slayer_state_json TEXT NOT NULL DEFAULT '{}'
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Chests table - shared world chests with persisted slot contents
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS chests (
                chest_key TEXT PRIMARY KEY,
                slots_json TEXT NOT NULL DEFAULT '[]'
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Migration: Consolidate bank stacks (one slot per item type, unlimited quantity)
        let bank_consolidated_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'bank_stacks_consolidated'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !bank_consolidated_exists {
            sqlx::query(
                "ALTER TABLE characters ADD COLUMN bank_stacks_consolidated INTEGER DEFAULT 0",
            )
            .execute(pool)
            .await
            .ok();

            // Load all characters' bank_json and consolidate duplicate stacks
            let rows: Vec<(i64, String)> = sqlx::query_as(
                "SELECT id, bank_json FROM characters WHERE bank_json IS NOT NULL AND bank_json != '[]'"
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            let mut migrated = 0u32;
            for (char_id, bank_json) in &rows {
                if let Ok(slots) = serde_json::from_str::<Vec<(usize, String, i32)>>(bank_json) {
                    // Group by item_id and sum quantities using i64 to avoid overflow
                    let mut merged: HashMap<String, i64> = HashMap::new();
                    for (_slot_idx, item_id, qty) in &slots {
                        *merged.entry(item_id.clone()).or_insert(0) += *qty as i64;
                    }

                    // Only rewrite if there were duplicates
                    if merged.len() < slots.len() {
                        let consolidated: Vec<(usize, String, i32)> = merged
                            .into_iter()
                            .enumerate()
                            .map(|(idx, (item_id, qty))| {
                                let clamped = qty.min(i32::MAX as i64) as i32;
                                if qty > i32::MAX as i64 {
                                    tracing::warn!(
                                        "Character {} item {} quantity overflow: {} clamped to {}",
                                        char_id,
                                        item_id,
                                        qty,
                                        clamped
                                    );
                                }
                                (idx, item_id, clamped)
                            })
                            .collect();

                        let new_json = serde_json::to_string(&consolidated)
                            .unwrap_or_else(|_| "[]".to_string());
                        tracing::info!(
                            "Bank migration: character {} consolidated {} slots -> {} slots",
                            char_id,
                            slots.len(),
                            consolidated.len()
                        );

                        sqlx::query("UPDATE characters SET bank_json = ? WHERE id = ?")
                            .bind(&new_json)
                            .bind(char_id)
                            .execute(pool)
                            .await
                            .ok();

                        migrated += 1;
                    }
                }
            }

            if migrated > 0 {
                tracing::info!(
                    "Bank stack consolidation migration: updated {} characters",
                    migrated
                );
            } else {
                tracing::info!("Bank stack consolidation migration: no duplicate stacks found");
            }
        }

        // Migration: Add combat_style_prefs column for per-weapon-type style persistence
        let combat_prefs_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'combat_style_prefs'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !combat_prefs_exists {
            sqlx::query("ALTER TABLE characters ADD COLUMN combat_style_prefs TEXT DEFAULT '{}'")
                .execute(pool)
                .await
                .ok();
        }

        // Migration: Add z column for player elevation persistence
        let z_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'z'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !z_exists {
            sqlx::query("ALTER TABLE characters ADD COLUMN z INTEGER DEFAULT 0")
                .execute(pool)
                .await
                .ok();
        }

        // KOTH pending rewards table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS koth_pending_rewards (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                player_id TEXT NOT NULL,
                item_id TEXT NOT NULL,
                quantity INTEGER NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Boss pending rewards table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS boss_pending_rewards (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                player_id TEXT NOT NULL,
                item_id TEXT NOT NULL,
                quantity INTEGER NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS bans (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id INTEGER NOT NULL,
                ip_address TEXT,
                banned_by TEXT NOT NULL,
                reason TEXT,
                banned_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                FOREIGN KEY (account_id) REFERENCES accounts(id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Collection log table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS collection_log (
                character_id INTEGER NOT NULL,
                item_id TEXT NOT NULL,
                source TEXT NOT NULL,
                source_detail TEXT,
                obtained_at TEXT NOT NULL,
                PRIMARY KEY (character_id, item_id, source)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Player titles table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS player_titles (
                character_id INTEGER NOT NULL,
                title_id TEXT NOT NULL,
                unlocked_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (character_id, title_id),
                FOREIGN KEY(character_id) REFERENCES characters(id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Crafting orders - available daily orders per player
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS crafting_orders_available (
                character_id INTEGER NOT NULL,
                order_id TEXT NOT NULL,
                generated_date TEXT NOT NULL,
                PRIMARY KEY (character_id, order_id),
                FOREIGN KEY(character_id) REFERENCES characters(id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Crafting orders - tracks which date orders were last generated per player
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS crafting_orders_generation (
                character_id INTEGER PRIMARY KEY,
                generated_date TEXT NOT NULL,
                FOREIGN KEY(character_id) REFERENCES characters(id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Crafting orders - currently active order per player
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS crafting_orders_active (
                character_id INTEGER PRIMARY KEY,
                order_id TEXT NOT NULL,
                accepted_at INTEGER NOT NULL,
                FOREIGN KEY(character_id) REFERENCES characters(id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Crafting order lifetime stats
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS crafting_order_stats (
                character_id INTEGER PRIMARY KEY,
                orders_completed INTEGER NOT NULL DEFAULT 0,
                masterwork_completed INTEGER NOT NULL DEFAULT 0,
                total_marks_earned INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY(character_id) REFERENCES characters(id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // ALTER TABLE: active_title and commission_marks on characters
        sqlx::query("ALTER TABLE characters ADD COLUMN active_title TEXT DEFAULT NULL")
            .execute(pool)
            .await
            .ok();
        sqlx::query("ALTER TABLE characters ADD COLUMN commission_marks INTEGER DEFAULT 0")
            .execute(pool)
            .await
            .ok();

        tracing::info!("Database migrations complete");
        Ok(())
    }
}
