use super::*;

#[derive(Deserialize)]
pub(super) struct RegisterRequest {
    username: String,
    password: String,
}

#[derive(Deserialize)]
pub(super) struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct AuthResponse {
    success: bool,
    token: Option<String>,
    username: Option<String>,
    characters: Option<Vec<CharacterInfo>>,
    error: Option<String>,
}

pub(super) async fn register_account(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    let client_ip = state.config.client_ip(&headers, addr).to_string();

    if !state.auth_rate_limiter.check(&client_ip) {
        warn!("Rate limit exceeded for registration from {}", client_ip);
        return Json(AuthResponse {
            success: false,
            token: None,
            username: None,
            characters: None,
            error: Some("Too many requests. Please try again later.".to_string()),
        });
    }

    // Validate input
    if req.username.len() < 3 {
        return Json(AuthResponse {
            success: false,
            token: None,
            username: None,
            characters: None,
            error: Some("Username must be at least 3 characters".to_string()),
        });
    }
    if req.password.len() < 6 {
        return Json(AuthResponse {
            success: false,
            token: None,
            username: None,
            characters: None,
            error: Some("Password must be at least 6 characters".to_string()),
        });
    }

    match state.db.create_account(&req.username, &req.password).await {
        Ok(account_id) => {
            let token = Uuid::new_v4().to_string();
            state.auth_sessions.insert(
                token.clone(),
                AuthSession::new(
                    account_id,
                    req.username.clone(),
                    state.config.auth_session_ttl,
                ),
            );

            info!(
                "Account registered: {} (id: {}) from {}",
                req.username, account_id, client_ip
            );

            Json(AuthResponse {
                success: true,
                token: Some(token),
                username: Some(req.username),
                characters: Some(vec![]), // New accounts have no characters
                error: None,
            })
        }
        Err(e) => Json(AuthResponse {
            success: false,
            token: None,
            username: None,
            characters: None,
            error: Some(e),
        }),
    }
}

pub(super) async fn login_account(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let client_ip = state.config.client_ip(&headers, addr).to_string();

    if !state.auth_rate_limiter.check(&client_ip) {
        warn!("Rate limit exceeded for login from {}", client_ip);
        return Json(AuthResponse {
            success: false,
            token: None,
            username: None,
            characters: None,
            error: Some("Too many login attempts. Please try again later.".to_string()),
        });
    }

    match state
        .db
        .verify_account_password(&req.username, &req.password)
        .await
    {
        Ok(Some(account)) => {
            // Check for active ban on this account
            if let Some((reason, expires_at)) = state.db.check_ban_by_account(account.id).await {
                let msg = match reason {
                    Some(r) => format!("Account banned until {}. Reason: {}", expires_at, r),
                    None => format!("Account banned until {}.", expires_at),
                };
                return Json(AuthResponse {
                    success: false,
                    token: None,
                    username: None,
                    characters: None,
                    error: Some(msg),
                });
            }
            let token = Uuid::new_v4().to_string();
            state.auth_sessions.insert(
                token.clone(),
                AuthSession::new(
                    account.id,
                    req.username.clone(),
                    state.config.auth_session_ttl,
                ),
            );

            info!(
                "Account logged in: {} (id: {}) from {}",
                req.username, account.id, client_ip
            );

            // Fetch characters for this account to include in response
            let characters = match state.db.get_characters_for_account(account.id).await {
                Ok(chars) => Some(
                    chars
                        .into_iter()
                        .map(|c| {
                            let sprite_head = CharacterInfo::resolve_sprite(
                                &state.item_registry,
                                &c.equipped_head,
                            );
                            let sprite_body = CharacterInfo::resolve_sprite(
                                &state.item_registry,
                                &c.equipped_body,
                            );
                            let sprite_weapon = CharacterInfo::resolve_sprite(
                                &state.item_registry,
                                &c.equipped_weapon,
                            );
                            let sprite_back = CharacterInfo::resolve_sprite(
                                &state.item_registry,
                                &c.equipped_back,
                            );
                            let sprite_feet = CharacterInfo::resolve_sprite(
                                &state.item_registry,
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
                        .collect(),
                ),
                Err(e) => {
                    warn!(
                        "Failed to fetch characters for account {}: {}",
                        account.id, e
                    );
                    None
                }
            };

            Json(AuthResponse {
                success: true,
                token: Some(token),
                username: Some(req.username),
                characters,
                error: None,
            })
        }
        Ok(None) => {
            state.auth_rate_limiter.record_failure(&client_ip);
            warn!(
                "Failed login attempt for '{}' from {}",
                req.username, client_ip
            );

            Json(AuthResponse {
                success: false,
                token: None,
                username: None,
                characters: None,
                error: Some("Invalid username or password".to_string()),
            })
        }
        Err(error) => {
            error!(
                "Database error during login for {}: {}",
                req.username, error
            );
            Json(AuthResponse {
                success: false,
                token: None,
                username: None,
                characters: None,
                error: Some("Login service temporarily unavailable".to_string()),
            })
        }
    }
}

pub(super) async fn logout_account(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    if let Some(auth) = headers.get("Authorization")
        && let Ok(auth_str) = auth.to_str()
        && let Some(token) = auth_str.strip_prefix("Bearer ")
    {
        let command_gates: Vec<Arc<RwLock<bool>>> = state
            .sessions
            .iter()
            .filter(|session| session.auth_token == token)
            .map(|session| session.command_gate.clone())
            .collect();
        for command_gate in command_gates {
            let mut active = command_gate.write().await;
            *active = false;
        }
        state.auth_sessions.remove(token);
    }
    Json(serde_json::json!({ "success": true }))
}

// ============================================================================
// HTTP Handlers - Characters
// ============================================================================

/// Maximum characters per account
pub(super) const MAX_CHARACTERS_PER_ACCOUNT: i64 = 3;
