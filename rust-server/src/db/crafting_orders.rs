use super::*;

impl Database {
    pub async fn get_available_orders(
        &self,
        character_id: i64,
        date: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT order_id FROM crafting_orders_available WHERE character_id = ? AND generated_date = ?",
        )
        .bind(character_id)
        .bind(date)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub async fn save_available_orders(
        &self,
        character_id: i64,
        date: &str,
        order_ids: &[String],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM crafting_orders_available WHERE character_id = ?")
            .bind(character_id)
            .execute(&mut *tx)
            .await?;
        for order_id in order_ids {
            sqlx::query(
                "INSERT INTO crafting_orders_available (character_id, order_id, generated_date) VALUES (?, ?, ?)",
            )
            .bind(character_id)
            .bind(order_id)
            .bind(date)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_orders_generated_date(
        &self,
        character_id: i64,
    ) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT generated_date FROM crafting_orders_generation WHERE character_id = ?",
        )
        .bind(character_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(d,)| d))
    }

    pub async fn set_orders_generated_date(
        &self,
        character_id: i64,
        date: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO crafting_orders_generation (character_id, generated_date) VALUES (?, ?)",
        )
        .bind(character_id)
        .bind(date)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_available_order(
        &self,
        character_id: i64,
        order_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM crafting_orders_available WHERE character_id = ? AND order_id = ?",
        )
        .bind(character_id)
        .bind(order_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_active_order(&self, character_id: i64) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT order_id FROM crafting_orders_active WHERE character_id = ?")
                .bind(character_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(id,)| id))
    }

    pub async fn save_active_order(
        &self,
        character_id: i64,
        order_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO crafting_orders_active (character_id, order_id, accepted_at) VALUES (?, ?, unixepoch())",
        )
        .bind(character_id)
        .bind(order_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_active_order(&self, character_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM crafting_orders_active WHERE character_id = ?")
            .bind(character_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn take_active_order(
        &self,
        character_id: i64,
    ) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(String,)> = sqlx::query_as(
            "DELETE FROM crafting_orders_active WHERE character_id = ? RETURNING order_id",
        )
        .bind(character_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(order_id,)| order_id))
    }

    pub async fn get_crafting_order_stats(
        &self,
        character_id: i64,
    ) -> Result<(i32, i32, i32), sqlx::Error> {
        let row: Option<(i32, i32, i32)> = sqlx::query_as(
            "SELECT orders_completed, masterwork_completed, total_marks_earned FROM crafting_order_stats WHERE character_id = ?",
        )
        .bind(character_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.unwrap_or((0, 0, 0)))
    }

    pub async fn increment_crafting_order_stats(
        &self,
        character_id: i64,
        is_masterwork: bool,
        marks: i32,
    ) -> Result<(), sqlx::Error> {
        let masterwork_inc: i32 = if is_masterwork { 1 } else { 0 };
        sqlx::query(
            r#"
            INSERT INTO crafting_order_stats (character_id, orders_completed, masterwork_completed, total_marks_earned)
            VALUES (?, 1, ?, ?)
            ON CONFLICT(character_id) DO UPDATE SET
                orders_completed = orders_completed + 1,
                masterwork_completed = masterwork_completed + ?,
                total_marks_earned = total_marks_earned + ?
            "#,
        )
        .bind(character_id)
        .bind(masterwork_inc)
        .bind(marks)
        .bind(masterwork_inc)
        .bind(marks)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
