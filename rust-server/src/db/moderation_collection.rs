use super::*;

impl Database {
    pub async fn check_ban_by_account(&self, account_id: i64) -> Option<(Option<String>, String)> {
        sqlx::query_as::<_, (Option<String>, String)>(
            "SELECT reason, expires_at FROM bans WHERE account_id = ? AND expires_at > datetime('now') ORDER BY expires_at DESC LIMIT 1"
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
    }

    pub async fn check_ban_by_ip(&self, ip: &str) -> Option<(Option<String>, String)> {
        sqlx::query_as::<_, (Option<String>, String)>(
            "SELECT reason, expires_at FROM bans WHERE ip_address = ? AND expires_at > datetime('now') ORDER BY expires_at DESC LIMIT 1"
        )
        .bind(ip)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
    }

    pub async fn insert_ban(
        &self,
        account_id: i64,
        ip_address: Option<&str>,
        banned_by: &str,
        reason: Option<&str>,
        hours: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO bans (account_id, ip_address, banned_by, reason, banned_at, expires_at) VALUES (?, ?, ?, ?, datetime('now'), datetime('now', '+' || ? || ' hours'))"
        )
        .bind(account_id)
        .bind(ip_address)
        .bind(banned_by)
        .bind(reason)
        .bind(hours)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_account_id_by_character_name(&self, name: &str) -> Option<i64> {
        sqlx::query_scalar::<_, i64>(
            "SELECT account_id FROM characters WHERE name = ? COLLATE NOCASE LIMIT 1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
    }

    pub async fn save_collection_entry(
        &self,
        character_id: i64,
        item_id: &str,
        source: &str,
        source_detail: &str,
        obtained_at: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO collection_log (character_id, item_id, source, source_detail, obtained_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(character_id)
        .bind(item_id)
        .bind(source)
        .bind(source_detail)
        .bind(obtained_at)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn load_collection_log(
        &self,
        character_id: i64,
    ) -> Result<Vec<(String, String, String, String)>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT item_id, source, source_detail, obtained_at FROM collection_log WHERE character_id = ?",
        )
        .bind(character_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                (
                    row.get("item_id"),
                    row.get("source"),
                    row.get::<String, _>("source_detail"),
                    row.get("obtained_at"),
                )
            })
            .collect())
    }

    pub async fn get_player_titles(&self, character_id: i64) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT title_id FROM player_titles WHERE character_id = ?")
                .bind(character_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub async fn unlock_title(&self, character_id: i64, title_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR IGNORE INTO player_titles (character_id, title_id) VALUES (?, ?)")
            .bind(character_id)
            .bind(title_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_active_title(&self, character_id: i64) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT active_title FROM characters WHERE id = ?")
                .bind(character_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|(t,)| t))
    }

    pub async fn set_active_title(
        &self,
        character_id: i64,
        title_id: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE characters SET active_title = ? WHERE id = ?")
            .bind(title_id)
            .bind(character_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_commission_marks(&self, character_id: i64) -> Result<i32, sqlx::Error> {
        let row: (i32,) = sqlx::query_as("SELECT commission_marks FROM characters WHERE id = ?")
            .bind(character_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn add_commission_marks(
        &self,
        character_id: i64,
        amount: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE characters SET commission_marks = commission_marks + ? WHERE id = ?")
            .bind(amount)
            .bind(character_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn spend_commission_marks(
        &self,
        character_id: i64,
        amount: i32,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE characters SET commission_marks = commission_marks - ? WHERE id = ? AND commission_marks >= ?",
        )
        .bind(amount)
        .bind(character_id)
        .bind(amount)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
