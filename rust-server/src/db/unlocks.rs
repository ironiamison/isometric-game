use super::*;

impl Database {
    pub async fn load_discovered_recipes(
        &self,
        character_id: i64,
    ) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query("SELECT recipe_id FROM discovered_recipes WHERE character_id = ?")
            .bind(character_id)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows.iter().map(|row| row.get("recipe_id")).collect())
    }

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

    pub async fn save_discovered_recipes(
        &self,
        character_id: i64,
        recipe_ids: &HashSet<String>,
    ) -> Result<(), sqlx::Error> {
        if recipe_ids.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;
        for recipe_id in recipe_ids {
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO discovered_recipes (character_id, recipe_id)
                VALUES (?, ?)
                "#,
            )
            .bind(character_id)
            .bind(recipe_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn load_unlocked_spells(
        &self,
        character_id: i64,
    ) -> Result<Vec<String>, sqlx::Error> {
        let rows =
            sqlx::query("SELECT spell_id FROM character_unlocked_spells WHERE character_id = ?")
                .bind(character_id)
                .fetch_all(&self.pool)
                .await?;

        Ok(rows.iter().map(|row| row.get("spell_id")).collect())
    }

    pub async fn save_unlocked_spell(
        &self,
        character_id: i64,
        spell_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO character_unlocked_spells (character_id, spell_id)
            VALUES (?, ?)
            "#,
        )
        .bind(character_id)
        .bind(spell_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn save_unlocked_spells(
        &self,
        character_id: i64,
        spell_ids: &HashSet<String>,
    ) -> Result<(), sqlx::Error> {
        if spell_ids.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;
        for spell_id in spell_ids {
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO character_unlocked_spells (character_id, spell_id)
                VALUES (?, ?)
                "#,
            )
            .bind(character_id)
            .bind(spell_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
