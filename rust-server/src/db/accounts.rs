use super::*;
use uuid::Uuid;

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

    /// Create a ephemeral guest account with a random username and password.
    pub async fn create_guest_account(&self) -> Result<(i64, String), String> {
        let username = format!("Guest_{}", &Uuid::new_v4().simple().to_string()[..8]);
        let password = Uuid::new_v4().to_string();
        let account_id = self.create_account(&username, &password).await?;
        Ok((account_id, username))
    }

    pub async fn get_account_by_wallet(
        &self,
        wallet_pubkey: &str,
    ) -> Result<Option<AccountData>, sqlx::Error> {
        let row = sqlx::query_as::<_, AccountRow>(
            "SELECT id, username, password_hash, created_at, last_login FROM accounts WHERE wallet_pubkey = ?",
        )
        .bind(wallet_pubkey)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(AccountData::from))
    }

    pub async fn create_wallet_account(
        &self,
        wallet_pubkey: &str,
        username: &str,
    ) -> Result<i64, String> {
        let password = Uuid::new_v4().to_string();
        let password_hash = tokio::task::spawn_blocking(move || {
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(password.as_bytes(), &salt)
                .map(|hash| hash.to_string())
                .map_err(|e| format!("Failed to hash password: {}", e))
        })
        .await
        .map_err(|e| format!("Password hashing task failed: {}", e))??;

        let result = sqlx::query(
            "INSERT INTO accounts (username, password_hash, wallet_pubkey) VALUES (?, ?, ?)",
        )
        .bind(username)
        .bind(&password_hash)
        .bind(wallet_pubkey)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint failed") {
                "Wallet already linked to another account".to_string()
            } else {
                format!("Database error: {}", e)
            }
        })?;

        let account_id = result.last_insert_rowid();
        tracing::info!("Created wallet account: {} (id: {})", username, account_id);
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
