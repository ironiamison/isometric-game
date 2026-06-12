use super::*;

#[derive(Serialize)]
struct CharacterListResponse {
    success: bool,
    characters: Option<Vec<CharacterInfo>>,
    error: Option<String>,
}

#[derive(Serialize)]
pub(super) struct CharacterInfo {
    pub(super) id: i64,
    pub(super) name: String,
    pub(super) level: i32,
    pub(super) gender: String,
    pub(super) skin: String,
    #[serde(rename = "hairStyle")]
    pub(super) hair_style: Option<i32>,
    #[serde(rename = "hairColor")]
    pub(super) hair_color: Option<i32>,
    #[serde(rename = "playedTime")]
    pub(super) played_time: i64,
    #[serde(rename = "equippedHead")]
    pub(super) equipped_head: Option<String>,
    #[serde(rename = "equippedBody")]
    pub(super) equipped_body: Option<String>,
    #[serde(rename = "equippedWeapon")]
    pub(super) equipped_weapon: Option<String>,
    #[serde(rename = "equippedBack")]
    pub(super) equipped_back: Option<String>,
    #[serde(rename = "equippedFeet")]
    pub(super) equipped_feet: Option<String>,
    #[serde(rename = "spriteHead")]
    pub(super) sprite_head: Option<String>,
    #[serde(rename = "spriteBody")]
    pub(super) sprite_body: Option<String>,
    #[serde(rename = "spriteWeapon")]
    pub(super) sprite_weapon: Option<String>,
    #[serde(rename = "spriteBack")]
    pub(super) sprite_back: Option<String>,
    #[serde(rename = "spriteFeet")]
    pub(super) sprite_feet: Option<String>,
}

impl CharacterInfo {
    pub(super) fn resolve_sprite(
        item_registry: &crate::data::item_registry::ItemRegistry,
        item_id: &Option<String>,
    ) -> Option<String> {
        item_id
            .as_ref()
            .and_then(|id| item_registry.get(id).map(|def| def.sprite.clone()))
    }
}

#[derive(Deserialize)]
pub(super) struct CreateCharacterRequest {
    name: String,
    gender: String,
    skin: String,
    #[serde(default)]
    hair_style: Option<i32>,
    #[serde(default)]
    hair_color: Option<i32>,
}

#[derive(Serialize)]
struct CreateCharacterResponse {
    success: bool,
    character: Option<CharacterInfo>,
    error: Option<String>,
}

#[derive(Serialize)]
struct DeleteCharacterResponse {
    success: bool,
    error: Option<String>,
}

/// Helper to extract auth token and account info from headers
pub(super) fn extract_auth(
    headers: &axum::http::HeaderMap,
    sessions: &AuthSessions,
) -> Option<(i64, String)> {
    let auth_header = headers.get("Authorization")?;
    let auth_str = auth_header.to_str().ok()?;
    let token = auth_str.strip_prefix("Bearer ")?;
    let session = get_auth_session(sessions, token)?;
    Some((session.account_id, session.username))
}

/// GET /api/characters - List all characters for the authenticated account
pub(super) async fn list_characters(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let (account_id, _username) = match extract_auth(&headers, &state.auth_sessions) {
        Some(auth) => auth,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(CharacterListResponse {
                    success: false,
                    characters: None,
                    error: Some("Not authenticated".to_string()),
                }),
            );
        }
    };

    match state.db.get_characters_for_account(account_id).await {
        Ok(chars) => {
            let char_infos: Vec<CharacterInfo> = chars
                .into_iter()
                .map(|c| {
                    let sprite_head = CharacterInfo::resolve_sprite(
                        &state.content.item_registry,
                        &c.equipped_head,
                    );
                    let sprite_body = CharacterInfo::resolve_sprite(
                        &state.content.item_registry,
                        &c.equipped_body,
                    );
                    let sprite_weapon = CharacterInfo::resolve_sprite(
                        &state.content.item_registry,
                        &c.equipped_weapon,
                    );
                    let sprite_back = CharacterInfo::resolve_sprite(
                        &state.content.item_registry,
                        &c.equipped_back,
                    );
                    let sprite_feet = CharacterInfo::resolve_sprite(
                        &state.content.item_registry,
                        &c.equipped_feet,
                    );
                    CharacterInfo {
                        id: c.id,
                        name: c.name.clone(),
                        level: c.skills.combat_level(),
                        gender: c.gender,
                        skin: c.skin,
                        hair_style: c.hair_style,
                        hair_color: c.hair_color,
                        played_time: c.played_time,
                        equipped_head: c.equipped_head,
                        equipped_body: c.equipped_body,
                        equipped_weapon: c.equipped_weapon,
                        equipped_back: c.equipped_back,
                        equipped_feet: c.equipped_feet,
                        sprite_head,
                        sprite_body,
                        sprite_weapon,
                        sprite_back,
                        sprite_feet,
                    }
                })
                .collect();

            (
                StatusCode::OK,
                Json(CharacterListResponse {
                    success: true,
                    characters: Some(char_infos),
                    error: None,
                }),
            )
        }
        Err(e) => {
            error!(
                "Failed to list characters for account {}: {}",
                account_id, e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CharacterListResponse {
                    success: false,
                    characters: None,
                    error: Some("Failed to list characters".to_string()),
                }),
            )
        }
    }
}

/// POST /api/characters - Create a new character
pub(super) async fn create_character(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateCharacterRequest>,
) -> impl IntoResponse {
    let (account_id, _username) = match extract_auth(&headers, &state.auth_sessions) {
        Some(auth) => auth,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(CreateCharacterResponse {
                    success: false,
                    character: None,
                    error: Some("Not authenticated".to_string()),
                }),
            );
        }
    };

    // Validate character name
    let name = req.name.trim();
    if name.len() < 2 {
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateCharacterResponse {
                success: false,
                character: None,
                error: Some("Character name must be at least 2 characters".to_string()),
            }),
        );
    }
    if name.len() > 16 {
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateCharacterResponse {
                success: false,
                character: None,
                error: Some("Character name must be at most 16 characters".to_string()),
            }),
        );
    }
    // Only allow alphanumeric characters and spaces
    if !name.chars().all(|c| c.is_alphanumeric() || c == ' ') {
        return (
            StatusCode::BAD_REQUEST,
            Json(CreateCharacterResponse {
                success: false,
                character: None,
                error: Some(
                    "Character name can only contain letters, numbers, and spaces".to_string(),
                ),
            }),
        );
    }

    // Check character limit
    match state.db.count_characters_for_account(account_id).await {
        Ok(count) if count >= MAX_CHARACTERS_PER_ACCOUNT => {
            return (
                StatusCode::BAD_REQUEST,
                Json(CreateCharacterResponse {
                    success: false,
                    character: None,
                    error: Some(format!(
                        "Character limit reached (max {})",
                        MAX_CHARACTERS_PER_ACCOUNT
                    )),
                }),
            );
        }
        Err(e) => {
            error!("Failed to count characters: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateCharacterResponse {
                    success: false,
                    character: None,
                    error: Some("Failed to check character count".to_string()),
                }),
            );
        }
        _ => {}
    }

    // Create the character
    match state
        .db
        .create_character(
            account_id,
            name,
            &req.gender,
            &req.skin,
            req.hair_style,
            req.hair_color,
        )
        .await
    {
        Ok(char_data) => {
            info!("Created character '{}' for account {}", name, account_id);
            (
                StatusCode::CREATED,
                Json(CreateCharacterResponse {
                    success: true,
                    character: Some({
                        let sprite_head = CharacterInfo::resolve_sprite(
                            &state.content.item_registry,
                            &char_data.equipped_head,
                        );
                        let sprite_body = CharacterInfo::resolve_sprite(
                            &state.content.item_registry,
                            &char_data.equipped_body,
                        );
                        let sprite_weapon = CharacterInfo::resolve_sprite(
                            &state.content.item_registry,
                            &char_data.equipped_weapon,
                        );
                        let sprite_back = CharacterInfo::resolve_sprite(
                            &state.content.item_registry,
                            &char_data.equipped_back,
                        );
                        let sprite_feet = CharacterInfo::resolve_sprite(
                            &state.content.item_registry,
                            &char_data.equipped_feet,
                        );
                        CharacterInfo {
                            id: char_data.id,
                            name: char_data.name,
                            level: char_data.skills.combat_level(),
                            gender: char_data.gender,
                            skin: char_data.skin,
                            hair_style: char_data.hair_style,
                            hair_color: char_data.hair_color,
                            played_time: char_data.played_time,
                            equipped_head: char_data.equipped_head,
                            equipped_body: char_data.equipped_body,
                            equipped_weapon: char_data.equipped_weapon,
                            equipped_back: char_data.equipped_back,
                            equipped_feet: char_data.equipped_feet,
                            sprite_head,
                            sprite_body,
                            sprite_weapon,
                            sprite_back,
                            sprite_feet,
                        }
                    }),
                    error: None,
                }),
            )
        }
        Err(e) => {
            let status = if e.contains("already exists") {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_REQUEST
            };
            (
                status,
                Json(CreateCharacterResponse {
                    success: false,
                    character: None,
                    error: Some(e),
                }),
            )
        }
    }
}

/// DELETE /api/characters/:id - Delete a character
pub(super) async fn delete_character(
    State(state): State<AppState>,
    Path(character_id): Path<i64>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let (account_id, _username) = match extract_auth(&headers, &state.auth_sessions) {
        Some(auth) => auth,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(DeleteCharacterResponse {
                    success: false,
                    error: Some("Not authenticated".to_string()),
                }),
            );
        }
    };

    if state.online_characters.contains(&character_id) {
        warn!(
            "Delete rejected: Character {} is currently online",
            character_id
        );
        return (
            StatusCode::CONFLICT,
            Json(DeleteCharacterResponse {
                success: false,
                error: Some("Cannot delete a character that is currently logged in".to_string()),
            }),
        );
    }

    match state.db.delete_character(character_id, account_id).await {
        Ok(true) => {
            info!(
                "Deleted character {} for account {}",
                character_id, account_id
            );
            (
                StatusCode::OK,
                Json(DeleteCharacterResponse {
                    success: true,
                    error: None,
                }),
            )
        }
        Ok(false) => {
            // Character doesn't exist or doesn't belong to this account
            (
                StatusCode::NOT_FOUND,
                Json(DeleteCharacterResponse {
                    success: false,
                    error: Some("Character not found".to_string()),
                }),
            )
        }
        Err(e) => {
            error!("Failed to delete character {}: {}", character_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DeleteCharacterResponse {
                    success: false,
                    error: Some("Failed to delete character".to_string()),
                }),
            )
        }
    }
}
