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

pub(super) async fn guest_login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let client_ip = state.config.client_ip(&headers, addr).to_string();

    if !state.auth_rate_limiter.check(&client_ip) {
        warn!("Rate limit exceeded for guest login from {}", client_ip);
        return Json(AuthResponse {
            success: false,
            token: None,
            username: None,
            characters: None,
            error: Some("Too many requests. Please try again later.".to_string()),
        });
    }

    match state.db.create_guest_account().await {
        Ok((account_id, username)) => {
            let token = Uuid::new_v4().to_string();
            state.auth_sessions.insert(
                token.clone(),
                AuthSession::new(account_id, username.clone(), state.config.auth_session_ttl),
            );

            info!(
                "Guest session created: {} (id: {}) from {}",
                username, account_id, client_ip
            );

            Json(AuthResponse {
                success: true,
                token: Some(token),
                username: Some(username),
                characters: Some(vec![]),
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

fn verify_solana_wallet_signature(
    pubkey_base58: &str,
    message: &[u8],
    signature_base64: &str,
) -> Result<(), String> {
    use base64::Engine;
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    let pubkey_bytes = bs58::decode(pubkey_base58)
        .into_vec()
        .map_err(|e| format!("Invalid wallet pubkey: {e}"))?;
    if pubkey_bytes.len() != 32 {
        return Err("Invalid wallet pubkey length".to_string());
    }

    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_base64)
        .map_err(|e| format!("Invalid signature encoding: {e}"))?;
    if signature_bytes.len() != 64 {
        return Err("Invalid signature length".to_string());
    }

    let verifying_key = VerifyingKey::from_bytes(
        &pubkey_bytes
            .try_into()
            .map_err(|_| "Invalid wallet pubkey bytes".to_string())?,
    )
    .map_err(|e| format!("Invalid wallet pubkey: {e}"))?;
    let signature = Signature::from_bytes(
        &signature_bytes
            .try_into()
            .map_err(|_| "Invalid signature bytes".to_string())?,
    );

    verifying_key
        .verify(message, &signature)
        .map_err(|_| "Signature verification failed".to_string())
}

fn wallet_display_username(pubkey: &str) -> String {
    if pubkey.len() >= 8 {
        format!("Wallet_{}…{}", &pubkey[..4], &pubkey[pubkey.len() - 4..])
    } else {
        format!("Wallet_{pubkey}")
    }
}

async fn auth_response_for_account(
    state: &AppState,
    account_id: i64,
    username: String,
) -> AuthResponse {
    let token = Uuid::new_v4().to_string();
    state.auth_sessions.insert(
        token.clone(),
        AuthSession::new(account_id, username.clone(), state.config.auth_session_ttl),
    );

    let characters = match state.db.get_characters_for_account(account_id).await {
        Ok(chars) => Some(
            chars
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
                .collect(),
        ),
        Err(e) => {
            warn!(
                "Failed to fetch characters for account {}: {}",
                account_id, e
            );
            None
        }
    };

    AuthResponse {
        success: true,
        token: Some(token),
        username: Some(username),
        characters,
        error: None,
    }
}

#[derive(Serialize)]
struct WalletChallengeResponse {
    nonce: String,
    message: String,
}

#[derive(Deserialize)]
pub(super) struct WalletLoginRequest {
    pubkey: String,
    signature: String,
    nonce: String,
}

pub(super) async fn wallet_challenge(State(state): State<AppState>) -> impl IntoResponse {
    state
        .wallet_challenges
        .retain(|_, challenge| challenge.expires_at > Instant::now());

    let nonce = Uuid::new_v4().to_string();
    let message = format!(
        "Sign in to Solstead\nNonce: {nonce}\nIssued: {}",
        chrono::Utc::now().to_rfc3339()
    );
    state.wallet_challenges.insert(
        nonce.clone(),
        WalletChallenge {
            message: message.clone(),
            expires_at: Instant::now() + Duration::from_secs(300),
        },
    );

    Json(WalletChallengeResponse { nonce, message })
}

pub(super) async fn wallet_login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(req): Json<WalletLoginRequest>,
) -> impl IntoResponse {
    let client_ip = state.config.client_ip(&headers, addr).to_string();

    if !state.auth_rate_limiter.check(&client_ip) {
        return Json(AuthResponse {
            success: false,
            token: None,
            username: None,
            characters: None,
            error: Some("Too many requests. Please try again later.".to_string()),
        });
    }

    let Some(challenge) = state.wallet_challenges.get(&req.nonce) else {
        return Json(AuthResponse {
            success: false,
            token: None,
            username: None,
            characters: None,
            error: Some("Invalid or expired sign-in challenge".to_string()),
        });
    };

    if challenge.expires_at <= Instant::now() {
        drop(challenge);
        state.wallet_challenges.remove(&req.nonce);
        return Json(AuthResponse {
            success: false,
            token: None,
            username: None,
            characters: None,
            error: Some("Sign-in challenge expired".to_string()),
        });
    }

    let message = challenge.message.clone();
    drop(challenge);

    if let Err(error) = verify_solana_wallet_signature(&req.pubkey, message.as_bytes(), &req.signature)
    {
        state.auth_rate_limiter.record_failure(&client_ip);
        warn!("Wallet login failed verification from {}: {}", client_ip, error);
        return Json(AuthResponse {
            success: false,
            token: None,
            username: None,
            characters: None,
            error: Some("Wallet signature verification failed".to_string()),
        });
    }

    state.wallet_challenges.remove(&req.nonce);

    let account = match state.db.get_account_by_wallet(&req.pubkey).await {
        Ok(existing) => existing,
        Err(error) => {
            error!("Database error during wallet login: {}", error);
            return Json(AuthResponse {
                success: false,
                token: None,
                username: None,
                characters: None,
                error: Some("Login service temporarily unavailable".to_string()),
            });
        }
    };

    let (account_id, username) = if let Some(account) = account {
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
        (account.id, account.username)
    } else {
        let username = wallet_display_username(&req.pubkey);
        match state.db.create_wallet_account(&req.pubkey, &username).await {
            Ok(account_id) => (account_id, username),
            Err(error) => {
                return Json(AuthResponse {
                    success: false,
                    token: None,
                    username: None,
                    characters: None,
                    error: Some(error),
                });
            }
        }
    };

    info!(
        "Wallet login: {} (id: {}) from {}",
        username, account_id, client_ip
    );

    Json(auth_response_for_account(&state, account_id, username).await)
}
