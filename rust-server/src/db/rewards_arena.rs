use super::*;

impl Database {
    pub async fn add_koth_pending_reward(
        &self,
        player_id: &str,
        item_id: &str,
        quantity: u32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO koth_pending_rewards (player_id, item_id, quantity) VALUES (?, ?, ?)",
        )
        .bind(player_id)
        .bind(item_id)
        .bind(quantity as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_koth_pending_rewards(
        &self,
        player_id: &str,
    ) -> Result<Vec<(i64, String, u32)>, sqlx::Error> {
        let rows: Vec<(i64, String, i64)> = sqlx::query_as(
            "SELECT id, item_id, quantity FROM koth_pending_rewards WHERE player_id = ? ORDER BY created_at",
        )
        .bind(player_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, item_id, qty)| (id, item_id, qty as u32))
            .collect())
    }

    pub async fn claim_koth_pending_rewards(
        &self,
        player_id: &str,
    ) -> Result<Vec<(String, u32)>, sqlx::Error> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "DELETE FROM koth_pending_rewards WHERE player_id = ? RETURNING item_id, quantity",
        )
        .bind(player_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(item_id, qty)| u32::try_from(qty).ok().map(|qty| (item_id, qty)))
            .collect())
    }

    pub async fn add_boss_pending_reward(
        &self,
        player_id: &str,
        item_id: &str,
        quantity: u32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO boss_pending_rewards (player_id, item_id, quantity) VALUES (?, ?, ?)",
        )
        .bind(player_id)
        .bind(item_id)
        .bind(quantity as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_boss_pending_rewards(
        &self,
        player_id: &str,
    ) -> Result<Vec<(i64, String, u32)>, sqlx::Error> {
        let rows: Vec<(i64, String, i64)> = sqlx::query_as(
            "SELECT id, item_id, quantity FROM boss_pending_rewards WHERE player_id = ? ORDER BY created_at",
        )
        .bind(player_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, item_id, qty)| (id, item_id, qty as u32))
            .collect())
    }

    pub async fn claim_boss_pending_rewards(
        &self,
        player_id: &str,
    ) -> Result<Vec<(String, u32)>, sqlx::Error> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "DELETE FROM boss_pending_rewards WHERE player_id = ? RETURNING item_id, quantity",
        )
        .bind(player_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(item_id, qty)| u32::try_from(qty).ok().map(|qty| (item_id, qty)))
            .collect())
    }

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

    pub async fn increment_character_monster_kills(
        &self,
        character_id: i64,
        kills: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE characters
            SET monster_kills = COALESCE(monster_kills, 0) + ?
            WHERE id = ?
            "#,
        )
        .bind(kills)
        .bind(character_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
