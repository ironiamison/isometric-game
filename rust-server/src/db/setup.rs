use super::*;

const LEGACY_CHARACTER_COLUMNS: &[(&str, &str)] = &[
    ("skills_json", "TEXT"),
    ("monster_kills", "INTEGER NOT NULL DEFAULT 0"),
    ("hair_style", "INTEGER DEFAULT NULL"),
    ("hair_color", "INTEGER DEFAULT NULL"),
    ("current_map", "TEXT DEFAULT NULL"),
    ("sitting_at_x", "INTEGER DEFAULT NULL"),
    ("sitting_at_y", "INTEGER DEFAULT NULL"),
    ("entrance_x", "REAL DEFAULT NULL"),
    ("entrance_y", "REAL DEFAULT NULL"),
    ("prayer_points", "INTEGER DEFAULT NULL"),
    ("mp", "INTEGER DEFAULT NULL"),
    ("bank_json", "TEXT NOT NULL DEFAULT '[]'"),
    ("bank_gold", "INTEGER NOT NULL DEFAULT 0"),
    ("bank_max_slots", "INTEGER NOT NULL DEFAULT 50"),
    ("bank_stacks_consolidated", "INTEGER NOT NULL DEFAULT 0"),
    ("combat_style_prefs", "TEXT NOT NULL DEFAULT '{}'"),
    ("z", "INTEGER NOT NULL DEFAULT 0"),
    ("active_title", "TEXT DEFAULT NULL"),
    ("commission_marks", "INTEGER NOT NULL DEFAULT 0"),
];

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

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|error| sqlx::Error::Migrate(Box::new(error)))?;
        Self::upgrade_legacy_schema(&pool).await?;
        Self::consolidate_legacy_banks(&pool).await?;

        tracing::info!("Database migrations complete");
        Ok(Self { pool })
    }

    async fn upgrade_legacy_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        for &(column, definition) in LEGACY_CHARACTER_COLUMNS {
            add_column_if_missing(pool, "characters", column, definition).await?;
        }
        add_column_if_missing(
            pool,
            "resource_contract_stats",
            "daily_completed",
            "INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
        add_column_if_missing(
            pool,
            "resource_contract_stats",
            "daily_date",
            "TEXT NOT NULL DEFAULT ''",
        )
        .await?;
        Ok(())
    }

    async fn consolidate_legacy_banks(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, bank_json FROM characters WHERE bank_stacks_consolidated = 0",
        )
        .fetch_all(pool)
        .await?;
        if rows.is_empty() {
            return Ok(());
        }

        let mut transaction = pool.begin().await?;
        for (character_id, bank_json) in rows {
            let slots: Vec<(usize, String, i32)> =
                serde_json::from_str(&bank_json).map_err(|error| {
                    sqlx::Error::Protocol(format!(
                        "character {character_id} has invalid bank_json: {error}"
                    ))
                })?;
            let mut merged = HashMap::<String, i64>::new();
            for (_, item_id, quantity) in slots {
                *merged.entry(item_id).or_default() += i64::from(quantity);
            }
            let consolidated = merged
                .into_iter()
                .enumerate()
                .map(|(index, (item_id, quantity))| {
                    let quantity = i32::try_from(quantity).map_err(|_| {
                        sqlx::Error::Protocol(format!(
                            "character {character_id} bank quantity exceeds i32 for {item_id}"
                        ))
                    })?;
                    Ok((index, item_id, quantity))
                })
                .collect::<Result<Vec<_>, sqlx::Error>>()?;
            let consolidated_json = serde_json::to_string(&consolidated).map_err(|error| {
                sqlx::Error::Protocol(format!(
                    "failed to serialize bank for character {character_id}: {error}"
                ))
            })?;

            sqlx::query(
                "UPDATE characters SET bank_json = ?, bank_stacks_consolidated = 1 WHERE id = ?",
            )
            .bind(consolidated_json)
            .bind(character_id)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }
}

async fn add_column_if_missing(
    pool: &SqlitePool,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), sqlx::Error> {
    let exists: bool = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('{table}') WHERE name = ?"
    ))
    .bind(column)
    .fetch_one(pool)
    .await?;
    if exists {
        return Ok(());
    }

    // Table and column names are constants owned by this module, never user input.
    let statement = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
    sqlx::query(&statement).execute(pool).await?;
    tracing::info!("Added legacy database column {table}.{column}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database_url(path: &std::path::Path) -> String {
        format!("sqlite://{}?mode=rwc", path.display())
    }

    #[tokio::test]
    async fn fresh_database_records_migrations_and_current_schema() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fresh.db");
        let database = Database::new(&database_url(&path)).await.unwrap();

        let migration_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(database.pool())
            .await
            .unwrap();
        let protocol_column: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('characters') WHERE name = 'combat_style_prefs'",
        )
        .fetch_one(database.pool())
        .await
        .unwrap();

        assert_eq!(migration_count, 1);
        assert!(protocol_column);
    }

    #[tokio::test]
    async fn legacy_database_is_upgraded_and_bank_data_is_consolidated() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy.db");
        let url = database_url(&path);
        let pool = SqlitePoolOptions::new().connect(&url).await.unwrap();
        sqlx::query(
            "CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                last_login TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE characters (
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
                bank_json TEXT DEFAULT '[]',
                bank_gold INTEGER DEFAULT 0,
                bank_max_slots INTEGER DEFAULT 50,
                played_time INTEGER DEFAULT 0,
                is_admin BOOLEAN DEFAULT FALSE,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO accounts (username, password_hash) VALUES ('legacy', 'hash')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO characters (account_id, name, bank_json)
             VALUES (1, 'Legacy Hero', '[[0,\"oak_log\",2],[1,\"oak_log\",3]]')",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let database = Database::new(&url).await.unwrap();
        let (bank_json, consolidated): (String, i64) = sqlx::query_as(
            "SELECT bank_json, bank_stacks_consolidated FROM characters WHERE id = 1",
        )
        .fetch_one(database.pool())
        .await
        .unwrap();
        let slots: Vec<(usize, String, i32)> = serde_json::from_str(&bank_json).unwrap();

        assert_eq!(slots, vec![(0, "oak_log".to_string(), 5)]);
        assert_eq!(consolidated, 1);
    }
}
