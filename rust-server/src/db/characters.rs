use super::*;

impl Database {
    pub async fn get_characters_for_account(
        &self,
        account_id: i64,
    ) -> Result<Vec<CharacterData>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT id, account_id, name, gender, skin, hair_style, hair_color, x, y, hp, prayer_points, mp, gold,
                equipped_head, equipped_body, equipped_weapon, equipped_back, equipped_feet,
                equipped_ring, equipped_gloves, equipped_necklace, equipped_belt,
                inventory_json, skills_json, played_time, is_admin, created_at, current_map,
                sitting_at_x, sitting_at_y, entrance_x, entrance_y,
                bank_json, bank_gold, bank_max_slots, combat_style_prefs, z
            FROM characters WHERE account_id = ? ORDER BY created_at DESC"#,
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| {
                let skills =
                    Skills::from_json(&r.try_get::<String, _>("skills_json").unwrap_or_default());

                CharacterData {
                    id: r.get("id"),
                    account_id: r.get("account_id"),
                    name: r.get("name"),
                    gender: r.get("gender"),
                    skin: r.get("skin"),
                    hair_style: r.try_get::<Option<i32>, _>("hair_style").unwrap_or(None),
                    hair_color: r.try_get::<Option<i32>, _>("hair_color").unwrap_or(None),
                    x: r.get("x"),
                    y: r.get("y"),
                    z: r.try_get::<i32, _>("z").unwrap_or(0),
                    hp: r.get("hp"),
                    prayer_points: r
                        .try_get::<Option<i32>, _>("prayer_points")
                        .unwrap_or(None)
                        .unwrap_or(10 + skills.prayer.level),
                    mp: r
                        .try_get::<Option<i32>, _>("mp")
                        .unwrap_or(None)
                        .unwrap_or(10 + skills.magic.level * 2),
                    skills,
                    gold: r.get("gold"),
                    equipped_head: r
                        .try_get::<String, _>("equipped_head")
                        .ok()
                        .filter(|s| !s.is_empty()),
                    equipped_body: r
                        .try_get::<String, _>("equipped_body")
                        .ok()
                        .filter(|s| !s.is_empty()),
                    equipped_weapon: r
                        .try_get::<String, _>("equipped_weapon")
                        .ok()
                        .filter(|s| !s.is_empty()),
                    equipped_back: r
                        .try_get::<String, _>("equipped_back")
                        .ok()
                        .filter(|s| !s.is_empty()),
                    equipped_feet: r
                        .try_get::<String, _>("equipped_feet")
                        .ok()
                        .filter(|s| !s.is_empty()),
                    equipped_ring: r
                        .try_get::<String, _>("equipped_ring")
                        .ok()
                        .filter(|s| !s.is_empty()),
                    equipped_gloves: r
                        .try_get::<String, _>("equipped_gloves")
                        .ok()
                        .filter(|s| !s.is_empty()),
                    equipped_necklace: r
                        .try_get::<String, _>("equipped_necklace")
                        .ok()
                        .filter(|s| !s.is_empty()),
                    equipped_belt: r
                        .try_get::<String, _>("equipped_belt")
                        .ok()
                        .filter(|s| !s.is_empty()),
                    inventory_json: r.get("inventory_json"),
                    played_time: r.get("played_time"),
                    created_at: r.get("created_at"),
                    is_admin: r.try_get::<bool, _>("is_admin").unwrap_or(false),
                    current_map: r
                        .try_get::<Option<String>, _>("current_map")
                        .unwrap_or(None),
                    sitting_at_x: r.try_get::<Option<i32>, _>("sitting_at_x").unwrap_or(None),
                    sitting_at_y: r.try_get::<Option<i32>, _>("sitting_at_y").unwrap_or(None),
                    entrance_x: r.try_get::<Option<f32>, _>("entrance_x").unwrap_or(None),
                    entrance_y: r.try_get::<Option<f32>, _>("entrance_y").unwrap_or(None),
                    bank_json: r
                        .try_get::<String, _>("bank_json")
                        .unwrap_or_else(|_| "[]".to_string()),
                    bank_gold: r.try_get::<i32, _>("bank_gold").unwrap_or(0),
                    bank_max_slots: r.try_get::<i32, _>("bank_max_slots").unwrap_or(50) as u32,
                    combat_style_prefs: r
                        .try_get::<String, _>("combat_style_prefs")
                        .unwrap_or_else(|_| "{}".to_string()),
                }
            })
            .collect())
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
        if let Some(style) = hair_style {
            if style < 0 || style > 5 {
                return Err(format!("Invalid hair style: {} (must be 0-5)", style));
            }
        }
        if let Some(color) = hair_color {
            if color < 0 || color > 9 {
                return Err(format!("Invalid hair color: {} (must be 0-9)", color));
            }
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
        let row = sqlx::query(
            r#"SELECT id, account_id, name, gender, skin, hair_style, hair_color, x, y, hp, prayer_points, mp, gold,
                equipped_head, equipped_body, equipped_weapon, equipped_back, equipped_feet,
                equipped_ring, equipped_gloves, equipped_necklace, equipped_belt,
                inventory_json, skills_json, played_time, is_admin, created_at, current_map,
                sitting_at_x, sitting_at_y, entrance_x, entrance_y,
                bank_json, bank_gold, bank_max_slots, combat_style_prefs, z
            FROM characters WHERE id = ?"#,
        )
        .bind(character_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| {
            let skills =
                Skills::from_json(&r.try_get::<String, _>("skills_json").unwrap_or_default());

            CharacterData {
                id: r.get("id"),
                account_id: r.get("account_id"),
                name: r.get("name"),
                gender: r.get("gender"),
                skin: r.get("skin"),
                hair_style: r.try_get::<Option<i32>, _>("hair_style").unwrap_or(None),
                hair_color: r.try_get::<Option<i32>, _>("hair_color").unwrap_or(None),
                x: r.get("x"),
                y: r.get("y"),
                z: r.try_get::<i32, _>("z").unwrap_or(0),
                hp: r.get("hp"),
                prayer_points: r
                    .try_get::<Option<i32>, _>("prayer_points")
                    .unwrap_or(None)
                    .unwrap_or(10 + skills.prayer.level),
                mp: r
                    .try_get::<Option<i32>, _>("mp")
                    .unwrap_or(None)
                    .unwrap_or(10 + skills.magic.level * 2),
                skills,
                gold: r.get("gold"),
                equipped_head: r
                    .try_get::<String, _>("equipped_head")
                    .ok()
                    .filter(|s| !s.is_empty()),
                equipped_body: r
                    .try_get::<String, _>("equipped_body")
                    .ok()
                    .filter(|s| !s.is_empty()),
                equipped_weapon: r
                    .try_get::<String, _>("equipped_weapon")
                    .ok()
                    .filter(|s| !s.is_empty()),
                equipped_back: r
                    .try_get::<String, _>("equipped_back")
                    .ok()
                    .filter(|s| !s.is_empty()),
                equipped_feet: r
                    .try_get::<String, _>("equipped_feet")
                    .ok()
                    .filter(|s| !s.is_empty()),
                equipped_ring: r
                    .try_get::<String, _>("equipped_ring")
                    .ok()
                    .filter(|s| !s.is_empty()),
                equipped_gloves: r
                    .try_get::<String, _>("equipped_gloves")
                    .ok()
                    .filter(|s| !s.is_empty()),
                equipped_necklace: r
                    .try_get::<String, _>("equipped_necklace")
                    .ok()
                    .filter(|s| !s.is_empty()),
                equipped_belt: r
                    .try_get::<String, _>("equipped_belt")
                    .ok()
                    .filter(|s| !s.is_empty()),
                inventory_json: r.get("inventory_json"),
                played_time: r.get("played_time"),
                created_at: r.get("created_at"),
                is_admin: r.try_get::<bool, _>("is_admin").unwrap_or(false),
                current_map: r
                    .try_get::<Option<String>, _>("current_map")
                    .unwrap_or(None),
                sitting_at_x: r.try_get::<Option<i32>, _>("sitting_at_x").unwrap_or(None),
                sitting_at_y: r.try_get::<Option<i32>, _>("sitting_at_y").unwrap_or(None),
                entrance_x: r.try_get::<Option<f32>, _>("entrance_x").unwrap_or(None),
                entrance_y: r.try_get::<Option<f32>, _>("entrance_y").unwrap_or(None),
                bank_json: r
                    .try_get::<String, _>("bank_json")
                    .unwrap_or_else(|_| "[]".to_string()),
                bank_gold: r.try_get::<i32, _>("bank_gold").unwrap_or(0),
                bank_max_slots: r.try_get::<i32, _>("bank_max_slots").unwrap_or(50) as u32,
                combat_style_prefs: r
                    .try_get::<String, _>("combat_style_prefs")
                    .unwrap_or_else(|_| "{}".to_string()),
            }
        }))
    }

    pub async fn delete_character(
        &self,
        character_id: i64,
        account_id: i64,
    ) -> Result<bool, sqlx::Error> {
        // Delete related quest data first
        sqlx::query("DELETE FROM character_quests WHERE character_id = ?")
            .bind(character_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM character_flags WHERE character_id = ?")
            .bind(character_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM character_quest_availability WHERE character_id = ?")
            .bind(character_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM arena_stats WHERE character_id = ?")
            .bind(character_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM friendships WHERE requester_id = ? OR recipient_id = ?")
            .bind(character_id)
            .bind(character_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM discovered_recipes WHERE character_id = ?")
            .bind(character_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM character_unlocked_spells WHERE character_id = ?")
            .bind(character_id)
            .execute(&self.pool)
            .await?;

        // Delete the character (only if owned by this account)
        let result = sqlx::query("DELETE FROM characters WHERE id = ? AND account_id = ?")
            .bind(character_id)
            .bind(account_id)
            .execute(&self.pool)
            .await?;

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
        x: f32,
        y: f32,
        z: i32,
        hp: i32,
        prayer_points: i32,
        mp: i32,
        skills: &Skills,
        gold: i32,
        inventory_json: &str,
        equipped_head: Option<&str>,
        equipped_body: Option<&str>,
        equipped_weapon: Option<&str>,
        equipped_back: Option<&str>,
        equipped_feet: Option<&str>,
        equipped_ring: Option<&str>,
        equipped_gloves: Option<&str>,
        equipped_necklace: Option<&str>,
        equipped_belt: Option<&str>,
        played_time_delta: i64,
        current_map: Option<&str>,
        sitting_at_x: Option<i32>,
        sitting_at_y: Option<i32>,
        entrance_x: Option<f32>,
        entrance_y: Option<f32>,
        bank_json: &str,
        bank_gold: i32,
        bank_max_slots: u32,
        combat_style_prefs: &str,
    ) -> Result<(), sqlx::Error> {
        // Serialize skills to JSON for the skills_json column
        let skills_json = serde_json::to_string(skills).unwrap_or_else(|_| "{}".to_string());

        // For backward compatibility, we also write to legacy columns with derived values
        let max_hp = skills.hitpoints.level;
        let level = skills.combat_level();

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
        .bind(x)
        .bind(y)
        .bind(z)
        .bind(hp)
        .bind(prayer_points)
        .bind(mp)
        .bind(max_hp)
        .bind(level)
        .bind(gold)
        .bind(inventory_json)
        .bind(&skills_json)
        .bind(equipped_head)
        .bind(equipped_body)
        .bind(equipped_weapon)
        .bind(equipped_back)
        .bind(equipped_feet)
        .bind(equipped_ring)
        .bind(equipped_gloves)
        .bind(equipped_necklace)
        .bind(equipped_belt)
        .bind(played_time_delta)
        .bind(current_map)
        .bind(sitting_at_x)
        .bind(sitting_at_y)
        .bind(entrance_x)
        .bind(entrance_y)
        .bind(bank_json)
        .bind(bank_gold)
        .bind(bank_max_slots as i32)
        .bind(combat_style_prefs)
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
