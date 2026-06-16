use super::*;

impl Database {
    #[allow(clippy::too_many_arguments)]
    pub async fn save_farming_patch(
        &self,
        patch_id: &str,
        player_id: &str,
        crop_id: &str,
        planted_at: u64,
        lives_remaining: u32,
        health: &str,
        composted: bool,
        disease_cycle_marker: u32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO farming_patches
                (patch_id, player_id, crop_id, planted_at, lives_remaining, health, composted, disease_cycle_marker)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(patch_id)
        .bind(player_id)
        .bind(crop_id)
        .bind(planted_at as i64)
        .bind(lives_remaining as i64)
        .bind(health)
        .bind(composted as i64)
        .bind(disease_cycle_marker as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_farming_patch_lives(
        &self,
        patch_id: &str,
        player_id: &str,
        lives_remaining: u32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE farming_patches SET lives_remaining = ? WHERE patch_id = ? AND player_id = ?",
        )
        .bind(lives_remaining as i64)
        .bind(patch_id)
        .bind(player_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_farming_patch_health(
        &self,
        patch_id: &str,
        player_id: &str,
        health: &str,
        disease_cycle_marker: u32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE farming_patches SET health = ?, disease_cycle_marker = ? WHERE patch_id = ? AND player_id = ?",
        )
        .bind(health)
        .bind(disease_cycle_marker as i64)
        .bind(patch_id)
        .bind(player_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_farming_patch_composted(
        &self,
        patch_id: &str,
        player_id: &str,
        composted: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE farming_patches SET composted = ? WHERE patch_id = ? AND player_id = ?",
        )
        .bind(composted as i64)
        .bind(patch_id)
        .bind(player_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_farming_patch(
        &self,
        patch_id: &str,
        player_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM farming_patches WHERE patch_id = ? AND player_id = ?")
            .bind(patch_id)
            .bind(player_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// One-time retirement of legacy hardcoded farming patches: credit each
    /// character's bank with the refunded seeds and delete the dead patch rows.
    /// Everything runs in a single transaction so a mid-way crash can't refund a
    /// player twice — either the seeds land and the rows vanish, or neither does.
    /// Idempotent across restarts because the rows are gone after a success.
    pub async fn retire_legacy_farming_patches(
        &self,
        refunds: &[LegacyFarmingRefund],
    ) -> Result<(), sqlx::Error> {
        if refunds.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for refund in refunds {
            // Merge the refunded seeds into the character's bank (one stack per
            // item, matching the consolidated bank_json layout).
            if !refund.seeds.is_empty() {
                let bank_json: Option<String> =
                    sqlx::query_scalar("SELECT bank_json FROM characters WHERE id = ?")
                        .bind(refund.character_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                if let Some(bank_json) = bank_json {
                    let mut slots: Vec<(usize, String, i32)> = serde_json::from_str(&bank_json)
                        .map_err(|error| {
                            sqlx::Error::Protocol(format!(
                                "character {} has invalid bank_json: {error}",
                                refund.character_id
                            ))
                        })?;
                    for (seed_id, quantity) in &refund.seeds {
                        if let Some(slot) = slots.iter_mut().find(|(_, item, _)| item == seed_id) {
                            slot.2 = slot.2.saturating_add(*quantity);
                        } else {
                            let next_slot =
                                slots.iter().map(|(index, ..)| index + 1).max().unwrap_or(0);
                            slots.push((next_slot, seed_id.clone(), *quantity));
                        }
                    }
                    let updated = serde_json::to_string(&slots).map_err(|error| {
                        sqlx::Error::Protocol(format!(
                            "failed to serialize bank for character {}: {error}",
                            refund.character_id
                        ))
                    })?;
                    sqlx::query("UPDATE characters SET bank_json = ? WHERE id = ?")
                        .bind(updated)
                        .bind(refund.character_id)
                        .execute(&mut *tx)
                        .await?;
                } else {
                    tracing::warn!(
                        "Legacy farming refund: character {} not found, skipping seed credit",
                        refund.character_id
                    );
                }
            }
            // Drop the dead rows now that their seeds are accounted for.
            for patch_id in &refund.patch_ids {
                sqlx::query("DELETE FROM farming_patches WHERE patch_id = ? AND player_id = ?")
                    .bind(patch_id)
                    .bind(&refund.player_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn load_farming_patches(&self) -> Result<Vec<FarmingPatchRow>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT patch_id, player_id, crop_id, planted_at, lives_remaining, health, composted, disease_cycle_marker FROM farming_patches",
        )
        .fetch_all(&self.pool)
        .await?;

        let patches = rows
            .iter()
            .map(|row| {
                let planted_at: i64 = row.get("planted_at");
                let lives_remaining: i64 = row.get("lives_remaining");
                let composted: i64 = row.get("composted");
                let disease_cycle_marker: i64 = row.get("disease_cycle_marker");
                FarmingPatchRow {
                    patch_id: row.get("patch_id"),
                    player_id: row.get("player_id"),
                    crop_id: row.get("crop_id"),
                    planted_at: planted_at as u64,
                    lives_remaining: lives_remaining as u32,
                    health: row.get("health"),
                    composted: composted != 0,
                    disease_cycle_marker: disease_cycle_marker as u32,
                }
            })
            .collect();

        Ok(patches)
    }

    pub async fn save_plot_unlock(&self, player_id: &str, plot_id: u32) -> Result<(), sqlx::Error> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO farming_plot_unlocks (player_id, plot_id, unlocked_at)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(player_id)
        .bind(plot_id as i32)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_plot_unlocks(&self) -> Result<Vec<(String, u32)>, sqlx::Error> {
        let rows = sqlx::query("SELECT player_id, plot_id FROM farming_plot_unlocks")
            .fetch_all(&self.pool)
            .await?;

        let unlocks = rows
            .iter()
            .map(|row| {
                let player_id: String = row.get("player_id");
                let plot_id: i32 = row.get("plot_id");
                (player_id, plot_id as u32)
            })
            .collect();

        Ok(unlocks)
    }

    pub async fn save_farming_contract(
        &self,
        player_id: &str,
        difficulty: &str,
        crop_id: &str,
        amount_required: i32,
        amount_harvested: i32,
        created_at: u64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO farming_contracts (player_id, difficulty, crop_id, amount_required, amount_harvested, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(player_id)
        .bind(difficulty)
        .bind(crop_id)
        .bind(amount_required)
        .bind(amount_harvested)
        .bind(created_at as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_farming_contract_progress(
        &self,
        player_id: &str,
        amount_harvested: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE farming_contracts SET amount_harvested = ? WHERE player_id = ?")
            .bind(amount_harvested)
            .bind(player_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_farming_contract(&self, player_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM farming_contracts WHERE player_id = ?")
            .bind(player_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn load_farming_contracts(
        &self,
    ) -> Result<Vec<(String, String, String, i32, i32, u64)>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT player_id, difficulty, crop_id, amount_required, amount_harvested, created_at FROM farming_contracts"
        )
        .fetch_all(&self.pool)
        .await?;

        let contracts = rows
            .iter()
            .map(|row| {
                let player_id: String = row.get("player_id");
                let difficulty: String = row.get("difficulty");
                let crop_id: String = row.get("crop_id");
                let amount_required: i32 = row.get("amount_required");
                let amount_harvested: i32 = row.get("amount_harvested");
                let created_at: i64 = row.get("created_at");
                (
                    player_id,
                    difficulty,
                    crop_id,
                    amount_required,
                    amount_harvested,
                    created_at as u64,
                )
            })
            .collect();

        Ok(contracts)
    }

    pub async fn save_resource_contract(
        &self,
        player_id: &str,
        contract_kind: &str,
        difficulty: &str,
        target_item_id: &str,
        target_name: &str,
        amount_required: i32,
        amount_completed: i32,
        giver_npc_id: &str,
        giver_name: &str,
        created_at: u64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO resource_contracts (
                player_id,
                contract_kind,
                difficulty,
                target_item_id,
                target_name,
                amount_required,
                amount_completed,
                giver_npc_id,
                giver_name,
                created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(player_id)
        .bind(contract_kind)
        .bind(difficulty)
        .bind(target_item_id)
        .bind(target_name)
        .bind(amount_required)
        .bind(amount_completed)
        .bind(giver_npc_id)
        .bind(giver_name)
        .bind(created_at as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_resource_contract_progress(
        &self,
        player_id: &str,
        amount_completed: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE resource_contracts SET amount_completed = ? WHERE player_id = ?")
            .bind(amount_completed)
            .bind(player_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_resource_contract(&self, player_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM resource_contracts WHERE player_id = ?")
            .bind(player_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn load_resource_contracts(
        &self,
    ) -> Result<
        Vec<(
            String,
            String,
            String,
            String,
            String,
            i32,
            i32,
            String,
            String,
            u64,
        )>,
        sqlx::Error,
    > {
        let rows = sqlx::query(
            r#"
            SELECT
                player_id,
                contract_kind,
                difficulty,
                target_item_id,
                target_name,
                amount_required,
                amount_completed,
                giver_npc_id,
                giver_name,
                created_at
            FROM resource_contracts
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                (
                    row.get("player_id"),
                    row.get("contract_kind"),
                    row.get("difficulty"),
                    row.get("target_item_id"),
                    row.get("target_name"),
                    row.get("amount_required"),
                    row.get("amount_completed"),
                    row.get("giver_npc_id"),
                    row.get("giver_name"),
                    row.get::<i64, _>("created_at") as u64,
                )
            })
            .collect())
    }

    pub async fn get_resource_contract_stats(
        &self,
        player_id: &str,
    ) -> Result<(i32, i32, i64), sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT contracts_completed, total_gold_earned, total_xp_earned
            FROM resource_contract_stats
            WHERE player_id = ?
            "#,
        )
        .bind(player_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(match row {
            Some(row) => (
                row.get("contracts_completed"),
                row.get("total_gold_earned"),
                row.get("total_xp_earned"),
            ),
            None => (0, 0, 0),
        })
    }

    pub async fn add_resource_contract_completion(
        &self,
        player_id: &str,
        gold_earned: i32,
        xp_earned: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO resource_contract_stats (
                player_id,
                contracts_completed,
                total_gold_earned,
                total_xp_earned
            )
            VALUES (?, 1, ?, ?)
            ON CONFLICT(player_id) DO UPDATE SET
                contracts_completed = contracts_completed + 1,
                total_gold_earned = total_gold_earned + excluded.total_gold_earned,
                total_xp_earned = total_xp_earned + excluded.total_xp_earned
            "#,
        )
        .bind(player_id)
        .bind(gold_earned)
        .bind(xp_earned)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_daily_contracts_completed(
        &self,
        player_id: &str,
        today: &str,
    ) -> Result<i32, sqlx::Error> {
        let row: Option<(i32, String)> = sqlx::query_as(
            "SELECT daily_completed, daily_date FROM resource_contract_stats WHERE player_id = ?",
        )
        .bind(player_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(match row {
            Some((count, date)) if date == today => count,
            _ => 0,
        })
    }

    pub async fn increment_daily_contracts(
        &self,
        player_id: &str,
        today: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE resource_contract_stats
            SET daily_completed = CASE WHEN daily_date = ? THEN daily_completed + 1 ELSE 1 END,
                daily_date = ?
            WHERE player_id = ?
            "#,
        )
        .bind(today)
        .bind(today)
        .bind(player_id)
        .execute(&self.pool)
        .await?;

        // If no row existed, the UPDATE affected 0 rows — insert a new one
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO resource_contract_stats (player_id, daily_completed, daily_date)
            VALUES (?, 1, ?)
            "#,
        )
        .bind(player_id)
        .bind(today)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::LegacyFarmingRefund;

    async fn temp_db() -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let db = Database::new(&url).await.unwrap();
        (dir, db)
    }

    async fn bank_of(db: &Database, character_id: i64) -> Vec<(usize, String, i32)> {
        let json: String = sqlx::query_scalar("SELECT bank_json FROM characters WHERE id = ?")
            .bind(character_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[tokio::test]
    async fn retire_legacy_patches_refunds_seeds_and_deletes_rows() {
        let (_dir, db) = temp_db().await;
        let account = db.create_account("farmer", "password123").await.unwrap();
        let character = db
            .create_character(account, "Nyx", "female", "tan", None, None)
            .await
            .unwrap();
        let player_id = format!("char_{}", character.id);

        // Seed an existing bank stack so we can prove merge-vs-append behaviour.
        sqlx::query("UPDATE characters SET bank_json = ? WHERE id = ?")
            .bind(r#"[[0,"oak_seed",2],[1,"rune_axe",1]]"#)
            .bind(character.id)
            .execute(&db.pool)
            .await
            .unwrap();

        // A planted legacy row that will be retired, plus a live "fp_" row that must survive.
        db.save_farming_patch("p1_allotment_2", &player_id, "potato", 0, 1, "healthy", false, 0)
            .await
            .unwrap();
        db.save_farming_patch("fp_live", &player_id, "potato", 0, 1, "healthy", false, 0)
            .await
            .unwrap();

        let refund = LegacyFarmingRefund {
            character_id: character.id,
            player_id: player_id.clone(),
            // Refund 4 potato (new stack) + 1 oak (merges into existing stack of 2).
            seeds: vec![("potato_seed".to_string(), 4), ("oak_seed".to_string(), 1)],
            patch_ids: vec!["p1_allotment_2".to_string()],
        };
        db.retire_legacy_farming_patches(&[refund.clone()]).await.unwrap();

        let bank = bank_of(&db, character.id).await;
        let qty = |item: &str| bank.iter().find(|(_, id, _)| id == item).map(|(_, _, q)| *q);
        assert_eq!(qty("oak_seed"), Some(3), "oak_seed merges into the existing stack");
        assert_eq!(qty("potato_seed"), Some(4), "potato_seed is appended as a new stack");
        assert_eq!(qty("rune_axe"), Some(1), "unrelated items are untouched");

        // The legacy row is gone; the live fp_ row remains. Bootstrap derives its
        // refund plan only from rows still present, so on the next boot there is
        // nothing left to retire — that's what makes the migration idempotent.
        let remaining = db.load_farming_patches().await.unwrap();
        let ids: Vec<_> = remaining.iter().map(|r| r.patch_id.as_str()).collect();
        assert_eq!(ids, vec!["fp_live"]);

        // An empty plan (what a re-boot now produces) is a no-op.
        let before = bank_of(&db, character.id).await;
        db.retire_legacy_farming_patches(&[]).await.unwrap();
        assert_eq!(bank_of(&db, character.id).await, before, "empty plan leaves the bank unchanged");
    }
}
