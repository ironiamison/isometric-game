use super::*;

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
    ) -> Option<AccountData> {
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
            let _ = sqlx::query("UPDATE accounts SET last_login = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(account.id)
                .execute(&self.pool)
                .await;
            return Some(account);
        }
        None
    }
}
