use super::*;

const CHARACTER_SELECT: &str = r#"
    SELECT id, account_id, name, gender, skin, hair_style, hair_color, x, y, hp,
           prayer_points, mp, gold, equipped_head, equipped_body, equipped_weapon,
           equipped_back, equipped_feet, equipped_ring, equipped_gloves,
           equipped_necklace, equipped_belt, inventory_json, skills_json, played_time,
           is_admin, created_at, current_map, sitting_at_x, sitting_at_y, entrance_x,
           entrance_y, bank_json, bank_gold, bank_max_slots, combat_style_prefs, z
    FROM characters
"#;

#[derive(sqlx::FromRow)]
struct CharacterRow {
    id: i64,
    account_id: i64,
    name: String,
    gender: String,
    skin: String,
    hair_style: Option<i32>,
    hair_color: Option<i32>,
    x: f32,
    y: f32,
    z: i32,
    hp: i32,
    prayer_points: Option<i32>,
    mp: Option<i32>,
    gold: i32,
    equipped_head: Option<String>,
    equipped_body: Option<String>,
    equipped_weapon: Option<String>,
    equipped_back: Option<String>,
    equipped_feet: Option<String>,
    equipped_ring: Option<String>,
    equipped_gloves: Option<String>,
    equipped_necklace: Option<String>,
    equipped_belt: Option<String>,
    inventory_json: String,
    skills_json: Option<String>,
    played_time: i64,
    is_admin: bool,
    created_at: Option<String>,
    current_map: Option<String>,
    sitting_at_x: Option<i32>,
    sitting_at_y: Option<i32>,
    entrance_x: Option<f32>,
    entrance_y: Option<f32>,
    bank_json: String,
    bank_gold: i32,
    bank_max_slots: i64,
    combat_style_prefs: String,
}

impl TryFrom<CharacterRow> for CharacterData {
    type Error = sqlx::Error;

    fn try_from(row: CharacterRow) -> Result<Self, Self::Error> {
        let skills = Skills::try_from_json(row.skills_json.as_deref().unwrap_or_default())
            .map_err(|error| {
                sqlx::Error::Protocol(format!(
                    "character {} has invalid skills_json: {error}",
                    row.id
                ))
            })?;
        let bank_max_slots = u32::try_from(row.bank_max_slots).map_err(|_| {
            sqlx::Error::Protocol(format!(
                "character {} has invalid bank_max_slots: {}",
                row.id, row.bank_max_slots
            ))
        })?;
        if bank_max_slots == 0 {
            return Err(sqlx::Error::Protocol(format!(
                "character {} has zero bank_max_slots",
                row.id
            )));
        }

        Ok(Self {
            id: row.id,
            account_id: row.account_id,
            name: row.name,
            gender: row.gender,
            skin: row.skin,
            hair_style: row.hair_style,
            hair_color: row.hair_color,
            x: row.x,
            y: row.y,
            z: row.z,
            hp: row.hp,
            prayer_points: row.prayer_points.unwrap_or(10 + skills.prayer.level),
            mp: row.mp.unwrap_or(10 + skills.magic.level * 2),
            skills,
            gold: row.gold,
            equipped_head: non_empty(row.equipped_head),
            equipped_body: non_empty(row.equipped_body),
            equipped_weapon: non_empty(row.equipped_weapon),
            equipped_back: non_empty(row.equipped_back),
            equipped_feet: non_empty(row.equipped_feet),
            equipped_ring: non_empty(row.equipped_ring),
            equipped_gloves: non_empty(row.equipped_gloves),
            equipped_necklace: non_empty(row.equipped_necklace),
            equipped_belt: non_empty(row.equipped_belt),
            inventory_json: row.inventory_json,
            played_time: row.played_time,
            created_at: row.created_at,
            is_admin: row.is_admin,
            current_map: row.current_map,
            sitting_at_x: row.sitting_at_x,
            sitting_at_y: row.sitting_at_y,
            entrance_x: row.entrance_x,
            entrance_y: row.entrance_y,
            bank_json: row.bank_json,
            bank_gold: row.bank_gold,
            bank_max_slots,
            combat_style_prefs: row.combat_style_prefs,
        })
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

impl Database {
    pub async fn get_characters_for_account(
        &self,
        account_id: i64,
    ) -> Result<Vec<CharacterData>, sqlx::Error> {
        let query = format!("{CHARACTER_SELECT} WHERE account_id = ? ORDER BY created_at DESC");
        let rows = sqlx::query_as::<_, CharacterRow>(&query)
            .bind(account_id)
            .fetch_all(&self.pool)
            .await?;

        rows.into_iter().map(CharacterData::try_from).collect()
    }

    pub async fn is_character_name_taken(&self, name: &str) -> Result<bool, sqlx::Error> {
        let row =
            sqlx::query("SELECT COUNT(*) as count FROM characters WHERE LOWER(name) = LOWER(?)")
                .bind(name)
                .fetch_one(&self.pool)
                .await?;
        let count: i64 = row.get("count");
        Ok(count > 0)
    }

    pub async fn count_characters_for_account(&self, account_id: i64) -> Result<i64, sqlx::Error> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM characters WHERE account_id = ?")
            .bind(account_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get("count"))
    }

    pub async fn create_character(
        &self,
        account_id: i64,
        name: &str,
        gender: &str,
        skin: &str,
        hair_style: Option<i32>,
        hair_color: Option<i32>,
    ) -> Result<CharacterData, String> {
        // Validate gender and skin
        if !GENDERS.contains(&gender) {
            return Err(format!("Invalid gender: {}", gender));
        }
        if !SKINS.contains(&skin) {
            return Err(format!("Invalid skin: {}", skin));
        }

        // Validate hair_style (0-5) and hair_color (0-9) if provided
        if let Some(style) = hair_style
            && (!(0..=5).contains(&style))
        {
            return Err(format!("Invalid hair style: {} (must be 0-5)", style));
        }
        if let Some(color) = hair_color
            && (!(0..=9).contains(&color))
        {
            return Err(format!("Invalid hair color: {} (must be 0-9)", color));
        }

        // Check if name is already taken (case-insensitive)
        match self.is_character_name_taken(name).await {
            Ok(true) => return Err("Character name already exists".to_string()),
            Ok(false) => {}
            Err(e) => return Err(format!("Database error checking name: {}", e)),
        }

        // Starting equipment for new characters (Tier 0 Cursed Lands gear)
        let starting_weapon = "chain";
        let starting_body = if gender == "female" {
            "peasant_suit_female"
        } else {
            "torn_clothes"
        };
        let starting_feet = "worn_sandals";
        let starting_gold = 25;

        let result = sqlx::query(
            r#"INSERT INTO characters
               (account_id, name, gender, skin, hair_style, hair_color,
                equipped_weapon, equipped_body, equipped_feet, gold, x, y)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, -30.0, 19.0)"#,
        )
        .bind(account_id)
        .bind(name)
        .bind(gender)
        .bind(skin)
        .bind(hair_style)
        .bind(hair_color)
        .bind(starting_weapon)
        .bind(starting_body)
        .bind(starting_feet)
        .bind(starting_gold)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint failed") {
                "Character name already exists".to_string()
            } else {
                format!("Database error: {}", e)
            }
        })?;

        let character_id = result.last_insert_rowid();
        tracing::info!(
            "Created character: {} (id: {}) for account {} with starting gear (pitchfork, clothes, sandals, {}g)",
            name,
            character_id,
            account_id,
            starting_gold
        );

        // Fetch and return the created character
        self.get_character(character_id)
            .await
            .map_err(|e| format!("Failed to fetch created character: {}", e))?
            .ok_or_else(|| "Failed to find created character".to_string())
    }

    pub async fn get_character(
        &self,
        character_id: i64,
    ) -> Result<Option<CharacterData>, sqlx::Error> {
        let query = format!("{CHARACTER_SELECT} WHERE id = ?");
        let row = sqlx::query_as::<_, CharacterRow>(&query)
            .bind(character_id)
            .fetch_optional(&self.pool)
            .await?;

        row.map(CharacterData::try_from).transpose()
    }

    pub async fn delete_character(
        &self,
        character_id: i64,
        account_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM characters WHERE id = ? AND account_id = ?)",
        )
        .bind(character_id)
        .bind(account_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !owned {
            transaction.rollback().await?;
            return Ok(false);
        }

        sqlx::query("DELETE FROM friendships WHERE requester_id = ? OR recipient_id = ?")
            .bind(character_id)
            .bind(character_id)
            .execute(&mut *transaction)
            .await?;
        for table in [
            "character_quests",
            "character_flags",
            "character_quest_availability",
            "arena_stats",
            "discovered_recipes",
            "character_unlocked_spells",
            "character_slayer",
            "collection_log",
            "player_titles",
            "crafting_orders_available",
            "crafting_orders_generation",
            "crafting_orders_active",
            "crafting_order_stats",
        ] {
            // Table names are fixed identifiers owned by this module.
            sqlx::query(&format!("DELETE FROM {table} WHERE character_id = ?"))
                .bind(character_id)
                .execute(&mut *transaction)
                .await?;
        }

        let result = sqlx::query("DELETE FROM characters WHERE id = ? AND account_id = ?")
            .bind(character_id)
            .bind(account_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            tracing::info!(
                "Deleted character {} for account {}",
                character_id,
                account_id
            );
        }
        Ok(deleted)
    }

    pub async fn save_character(
        &self,
        character_id: i64,
        save: &crate::game::PlayerSaveData,
        played_time_delta: i64,
    ) -> Result<(), sqlx::Error> {
        // Serialize skills to JSON for the skills_json column
        let skills_json = serde_json::to_string(&save.skills).map_err(|error| {
            sqlx::Error::Protocol(format!("failed to serialize skills: {error}"))
        })?;

        // For backward compatibility, we also write to legacy columns with derived values
        let max_hp = save.skills.hitpoints.level;
        let level = save.skills.combat_level();

        sqlx::query(
            r#"UPDATE characters SET
                x = ?, y = ?, z = ?, hp = ?, prayer_points = ?, mp = ?, max_hp = ?, level = ?,
                gold = ?, inventory_json = ?, skills_json = ?,
                equipped_head = ?, equipped_body = ?, equipped_weapon = ?,
                equipped_back = ?, equipped_feet = ?, equipped_ring = ?,
                equipped_gloves = ?, equipped_necklace = ?, equipped_belt = ?,
                played_time = played_time + ?,
                current_map = ?,
                sitting_at_x = ?,
                sitting_at_y = ?,
                entrance_x = ?,
                entrance_y = ?,
                bank_json = ?,
                bank_gold = ?,
                bank_max_slots = ?,
                combat_style_prefs = ?
            WHERE id = ?"#,
        )
        .bind(save.x)
        .bind(save.y)
        .bind(save.z)
        .bind(save.hp)
        .bind(save.prayer_points)
        .bind(save.mp)
        .bind(max_hp)
        .bind(level)
        .bind(save.gold)
        .bind(&save.inventory_json)
        .bind(&skills_json)
        .bind(save.equipped_head.as_deref())
        .bind(save.equipped_body.as_deref())
        .bind(save.equipped_weapon.as_deref())
        .bind(save.equipped_back.as_deref())
        .bind(save.equipped_feet.as_deref())
        .bind(save.equipped_ring.as_deref())
        .bind(save.equipped_gloves.as_deref())
        .bind(save.equipped_necklace.as_deref())
        .bind(save.equipped_belt.as_deref())
        .bind(played_time_delta)
        .bind(save.current_map.as_deref())
        .bind(save.sitting_at_x)
        .bind(save.sitting_at_y)
        .bind(save.entrance_x)
        .bind(save.entrance_y)
        .bind(&save.bank_json)
        .bind(save.bank_gold)
        .bind(save.bank_max_slots as i32)
        .bind(&save.combat_style_prefs)
        .bind(character_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn load_character_quest_state(
        &self,
        character_id: i64,
    ) -> Result<PlayerQuestState, sqlx::Error> {
        let mut state = PlayerQuestState::new();

        // Load quests from character_quests table
        let quest_rows = sqlx::query(
            "SELECT quest_id, state, objectives_json, started_at, completed_at FROM character_quests WHERE character_id = ?"
        )
        .bind(character_id)
        .fetch_all(&self.pool)
        .await?;

        for row in quest_rows {
            let quest_id: String = row.get("quest_id");
            let state_str: String = row.get("state");
            let objectives_json: String = row.get("objectives_json");
            let started_at: Option<String> = row.get("started_at");
            let completed_at: Option<String> = row.get("completed_at");

            let status = QuestStatus::from_str(&state_str).unwrap_or(QuestStatus::Active);

            // Parse timestamps
            let started_at_dt = started_at.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            });
            let completed_at_dt = completed_at.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            });

            // Deserialize objectives
            let objectives = QuestProgress::objectives_from_json(&objectives_json);

            let progress = QuestProgress {
                quest_id: quest_id.clone(),
                status,
                objectives,
                started_at: started_at_dt,
                completed_at: completed_at_dt,
            };

            match status {
                QuestStatus::Completed => {
                    state.completed_quests.push(quest_id);
                }
                QuestStatus::Active | QuestStatus::ReadyToComplete => {
                    state.active_quests.insert(quest_id, progress);
                }
                _ => {
                    // Failed/Abandoned quests are stored but not active
                }
            }
        }

        // Load available quests
        let available_rows =
            sqlx::query("SELECT quest_id FROM character_quest_availability WHERE character_id = ?")
                .bind(character_id)
                .fetch_all(&self.pool)
                .await?;

        for row in available_rows {
            let quest_id: String = row.get("quest_id");
            // Only add if not already active or completed
            if !state.active_quests.contains_key(&quest_id)
                && !state.completed_quests.contains(&quest_id)
            {
                state.available_quests.push(quest_id);
            }
        }

        // Load flags
        let flag_rows =
            sqlx::query("SELECT flag_name, flag_value FROM character_flags WHERE character_id = ?")
                .bind(character_id)
                .fetch_all(&self.pool)
                .await?;

        for row in flag_rows {
            let flag_name: String = row.get("flag_name");
            let flag_value: Option<String> = row.get("flag_value");
            if let Some(value) = flag_value {
                state.flags.insert(flag_name, value);
            }
        }

        tracing::debug!(
            "Loaded quest state for character {}: {} active, {} completed, {} available",
            character_id,
            state.active_quests.len(),
            state.completed_quests.len(),
            state.available_quests.len()
        );

        Ok(state)
    }

    pub async fn save_character_quest_state(
        &self,
        character_id: i64,
        state: &PlayerQuestState,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // Save active quests
        for (quest_id, progress) in &state.active_quests {
            let objectives_json = progress.objectives_to_json();
            let started_at = progress.started_at.map(|dt| dt.to_rfc3339());
            let completed_at = progress.completed_at.map(|dt| dt.to_rfc3339());

            sqlx::query(
                r#"INSERT INTO character_quests (character_id, quest_id, state, objectives_json, started_at, completed_at)
                   VALUES (?, ?, ?, ?, ?, ?)
                   ON CONFLICT(character_id, quest_id) DO UPDATE SET
                       state = excluded.state,
                       objectives_json = excluded.objectives_json,
                       started_at = excluded.started_at,
                       completed_at = excluded.completed_at"#
            )
            .bind(character_id)
            .bind(quest_id)
            .bind(progress.status.as_str())
            .bind(&objectives_json)
            .bind(&started_at)
            .bind(&completed_at)
            .execute(&mut *tx)
            .await?;
        }

        // Save completed quests
        for quest_id in &state.completed_quests {
            sqlx::query(
                r#"INSERT INTO character_quests (character_id, quest_id, state, completed_at)
                   VALUES (?, ?, 'completed', CURRENT_TIMESTAMP)
                   ON CONFLICT(character_id, quest_id) DO UPDATE SET
                       state = 'completed',
                       completed_at = CURRENT_TIMESTAMP"#,
            )
            .bind(character_id)
            .bind(quest_id)
            .execute(&mut *tx)
            .await?;
        }

        // Save available quests
        for quest_id in &state.available_quests {
            sqlx::query(
                r#"INSERT OR IGNORE INTO character_quest_availability (character_id, quest_id)
                   VALUES (?, ?)"#,
            )
            .bind(character_id)
            .bind(quest_id)
            .execute(&mut *tx)
            .await?;
        }

        // Save flags
        for (flag_name, flag_value) in &state.flags {
            sqlx::query(
                r#"INSERT INTO character_flags (character_id, flag_name, flag_value)
                   VALUES (?, ?, ?)
                   ON CONFLICT(character_id, flag_name) DO UPDATE SET
                       flag_value = excluded.flag_value"#,
            )
            .bind(character_id)
            .bind(flag_name)
            .bind(flag_value)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        tracing::debug!(
            "Saved quest state for character {}: {} active, {} completed",
            character_id,
            state.active_quests.len(),
            state.completed_quests.len()
        );

        Ok(())
    }
}
