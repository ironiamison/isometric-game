use super::*;

#[derive(sqlx::FromRow)]
struct AccountRow {
    id: i64,
    username: String,
    password_hash: String,
    created_at: Option<String>,
    last_login: Option<String>,
}

impl From<AccountRow> for AccountData {
    fn from(row: AccountRow) -> Self {
        Self {
            id: row.id,
            username: row.username,
            password_hash: row.password_hash,
            created_at: row.created_at,
            last_login: row.last_login,
        }
    }
}

impl Database {
    pub async fn create_account(&self, username: &str, password: &str) -> Result<i64, String> {
        let password = password.to_owned();
        let password_hash = tokio::task::spawn_blocking(move || {
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(password.as_bytes(), &salt)
                .map(|hash| hash.to_string())
                .map_err(|e| format!("Failed to hash password: {}", e))
        })
        .await
        .map_err(|e| format!("Password hashing task failed: {}", e))??;

        let result = sqlx::query("INSERT INTO accounts (username, password_hash) VALUES (?, ?)")
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

    pub async fn verify_account_password(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<AccountData>, sqlx::Error> {
        let row = sqlx::query_as::<_, AccountRow>(
            "SELECT id, username, password_hash, created_at, last_login FROM accounts WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;
        let Some(account) = row.map(AccountData::from) else {
            return Ok(None);
        };

        let password = password.to_owned();
        let password_hash = account.password_hash.clone();
        let verified = tokio::task::spawn_blocking(move || {
            PasswordHash::new(&password_hash)
                .ok()
                .is_some_and(|parsed_hash| {
                    Argon2::default()
                        .verify_password(password.as_bytes(), &parsed_hash)
                        .is_ok()
                })
        })
        .await
        .unwrap_or(false);

        if verified {
            sqlx::query("UPDATE accounts SET last_login = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(account.id)
                .execute(&self.pool)
                .await?;
            return Ok(Some(account));
        }
        Ok(None)
    }
}
