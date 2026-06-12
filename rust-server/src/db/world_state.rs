use super::*;

impl Database {
    pub async fn load_character_slayer_state(
        &self,
        character_id: i64,
    ) -> Result<crate::slayer::PlayerSlayerState, String> {
        let row =
            sqlx::query("SELECT slayer_state_json FROM character_slayer WHERE character_id = ?")
                .bind(character_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| format!("Failed to load slayer state: {}", e))?;

        match row {
            Some(row) => {
                let json: String = sqlx::Row::get(&row, "slayer_state_json");
                serde_json::from_str(&json)
                    .map_err(|e| format!("Failed to parse slayer state: {}", e))
            }
            None => Ok(crate::slayer::PlayerSlayerState::default()),
        }
    }

    pub async fn save_character_slayer_state(
        &self,
        character_id: i64,
        state: &crate::slayer::PlayerSlayerState,
    ) -> Result<(), String> {
        let json = serde_json::to_string(state)
            .map_err(|e| format!("Failed to serialize slayer state: {}", e))?;

        sqlx::query("INSERT OR REPLACE INTO character_slayer (character_id, slayer_state_json) VALUES (?, ?)")
            .bind(character_id)
            .bind(&json)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to save slayer state: {}", e))?;

        Ok(())
    }

    pub async fn save_chest(&self, chest_key: &str, slots_json: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO chests (chest_key, slots_json) VALUES (?, ?)
               ON CONFLICT(chest_key) DO UPDATE SET slots_json = excluded.slots_json"#,
        )
        .bind(chest_key)
        .bind(slots_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_all_chests(&self) -> Result<HashMap<String, String>, sqlx::Error> {
        let rows = sqlx::query("SELECT chest_key, slots_json FROM chests")
            .fetch_all(&self.pool)
            .await?;

        let mut result = HashMap::new();
        for row in rows {
            let key: String = row.get("chest_key");
            let json: String = row.get("slots_json");
            result.insert(key, json);
        }
        Ok(result)
    }

    pub async fn save_all_chests(
        &self,
        chests: &HashMap<String, String>,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        for (key, json) in chests {
            sqlx::query(
                r#"INSERT INTO chests (chest_key, slots_json) VALUES (?, ?)
                   ON CONFLICT(chest_key) DO UPDATE SET slots_json = excluded.slots_json"#,
            )
            .bind(key)
            .bind(json)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_top_total_level_players(
        &self,
    ) -> (Option<(String, i32)>, Option<(String, i32)>) {
        let rows = sqlx::query("SELECT name, skills_json FROM characters WHERE is_admin = FALSE")
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();

        let mut first: Option<(String, i32)> = None;
        let mut second: Option<(String, i32)> = None;
        for row in rows {
            let name: String = match row.try_get("name") {
                Ok(n) => n,
                Err(_) => continue,
            };
            let skills_json: String = row.try_get("skills_json").unwrap_or_default();
            let skills = Skills::from_json(&skills_json);
            let total = skills.total_level();
            if first.as_ref().is_none_or(|(_, b)| total > *b) {
                second = first.take();
                first = Some((name, total));
            } else if second.as_ref().is_none_or(|(_, b)| total > *b) {
                second = Some((name, total));
            }
        }
        (first, second)
    }
}
