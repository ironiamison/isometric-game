use crate::data::item_def::EquipmentSlot;
use crate::skills::Skills;
use axum::{
    Json, Router,
    extract::{
        ConnectInfo, Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use dashmap::{DashMap, DashSet};
use futures::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::Row;
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::{RwLock, broadcast, mpsc, watch};
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};
use uuid::Uuid;

mod arena;
mod chest;
mod chunk;
mod crafting;
mod data;
mod db;
mod dig_site;
mod entity;
mod farming;
mod game;
mod gathering;
mod ground_spawn;
mod instance;
mod interior;
mod koth;
mod boss;
mod pharaoh_boss;
mod interior_registry;
mod item;
mod log_buffer;
mod mining;
mod npc;
mod perf_metrics;
mod prayer;
mod protocol;
mod quest;
mod scroll_spell;
mod shop;
mod skills;
mod slayer;
mod spell;
mod tilemap;
mod waystone;
mod woodcutting;
mod world;

use crafting::CraftingRegistry;
use data::ItemRegistry;
use db::Database;
use entity::EntityRegistry;
use game::{GameRoom, Player, PlayerUpdate};
use instance::InstanceManager;
use interior_registry::InteriorRegistry;
use prayer::PrayerRegistry;
use protocol::{ClientMessage, ServerMessage};
use quest::{ObjectiveType, QuestRegistry};

// ============================================================================
// App State
// ============================================================================

/// Game session data for a connected player
#[derive(Clone)]
struct GameSession {
    room_id: String,
    player_id: String,
    character_name: String,      // Character name for display
    character_id: i64,           // Database character ID
    account_id: i64,             // Database account ID
    auth_token: String,          // Token used for this session (for validation)
    current_map: Option<String>, // Interior map ID to auto-enter on connect (None = overworld)
    entrance_x: Option<f32>,     // Overworld X where player entered interior
    entrance_y: Option<f32>,     // Overworld Y where player entered interior
    is_new_character: bool,      // True if played_time == 0 (for tutorial)
}

#[derive(Clone)]
struct AppState {
    rooms: Arc<DashMap<String, Arc<GameRoom>>>,
    // Session ID -> GameSession
    sessions: Arc<DashMap<String, GameSession>>,
    // Auth token -> (Username, Player DB ID)
    auth_sessions: AuthSessions,
    db: Arc<Database>,
    auth_rate_limiter: RateLimiter,
    matchmake_rate_limiter: RateLimiter,
    // SECURITY: Signed session token generator
    token_signer: SessionTokenSigner,
    // Entity and item registries (loaded from TOML at startup)
    entity_registry: Arc<EntityRegistry>,
    item_registry: Arc<ItemRegistry>,
    prayer_registry: Arc<PrayerRegistry>,
    quest_registry: Arc<QuestRegistry>,
    crafting_registry: Arc<CraftingRegistry>,
    chest_registry: Arc<crate::chest::ChestRegistry>,
    interior_registry: Arc<InteriorRegistry>,
    instance_manager: Arc<InstanceManager>,
    /// Tracks which instance each player is currently in (None = overworld)
    player_instances: Arc<RwLock<HashMap<String, String>>>,
    /// Tracks where each player entered their current interior from (for return teleport)
    player_entrance_positions: Arc<RwLock<HashMap<String, (i32, i32)>>>,
    /// Character ID -> last time played_time was flushed to DB (for incremental play time tracking)
    play_time_anchors: Arc<DashMap<i64, std::time::Instant>>,
    /// Character IDs currently online (prevents duplicate sessions)
    online_characters: Arc<DashSet<i64>>,
    /// In-memory log buffer for /api/logs endpoint
    log_buffer: log_buffer::LogBuffer,
    /// In-memory rolling performance metrics for /api/perf endpoint
    perf_metrics: perf_metrics::PerfMetrics,
}

impl AppState {
    async fn new(log_buffer: log_buffer::LogBuffer) -> Self {
        // Initialize database
        let db = Database::new("sqlite:game.db?mode=rwc")
            .await
            .expect("Failed to initialize database");

        // Load entity registry from TOML files
        let mut entity_registry = EntityRegistry::new();
        let data_dir = std::path::Path::new("data");
        if let Err(e) = entity_registry.load_from_directory(data_dir) {
            error!("Failed to load entity registry: {}", e);
        }

        // Load item registry from TOML files
        let mut item_registry = ItemRegistry::new();
        if let Err(e) = item_registry.load_from_directory(data_dir) {
            error!("Failed to load item registry: {}", e);
        }

        // Load prayer registry from TOML file
        let mut prayer_registry = PrayerRegistry::new();
        if let Err(e) = prayer_registry.load_from_directory(data_dir) {
            error!("Failed to load prayer registry: {}", e);
        }

        // Load quest registry from TOML files
        let quest_registry = Arc::new(QuestRegistry::new(data_dir));
        if let Err(e) = quest_registry.load_all().await {
            error!("Failed to load quest registry: {}", e);
        }

        // Load crafting registry from TOML files
        let mut crafting_registry = CraftingRegistry::new();
        if let Err(e) = crafting_registry.load_from_directory(data_dir) {
            error!("Failed to load crafting registry: {}", e);
        }

        // Load chest registry from TOML file
        let mut chest_registry = crate::chest::ChestRegistry::new();
        chest_registry.load_from_file(&data_dir.join("chests.toml"));

        // Load interior registry from JSON files
        let interior_registry = Arc::new(
            InteriorRegistry::load_from_directory("maps/interiors")
                .expect("Failed to load interior registry"),
        );

        // Initialize instance manager
        let instance_manager = Arc::new(InstanceManager::new());

        // Start hot-reload watcher for quest files (dev mode)
        #[cfg(debug_assertions)]
        {
            match quest_registry.start_file_watcher() {
                Ok(mut rx) => {
                    // Spawn task to log reload events
                    tokio::spawn(async move {
                        while let Some(event) = rx.recv().await {
                            match event {
                                quest::HotReloadEvent::Reloaded(path) => {
                                    info!("Quest hot-reload: {}", path);
                                }
                                quest::HotReloadEvent::Error(e) => {
                                    error!("Quest hot-reload error: {}", e);
                                }
                            }
                        }
                    });
                    info!("Quest hot-reload enabled");
                }
                Err(e) => {
                    warn!("Failed to start quest hot-reload: {}", e);
                }
            }
        }

        Self {
            rooms: Arc::new(DashMap::new()),
            sessions: Arc::new(DashMap::new()),
            auth_sessions: Arc::new(DashMap::new()),
            db: Arc::new(db),
            // Auth: 10 attempts per 60 seconds per IP
            auth_rate_limiter: RateLimiter::new(10, 60),
            // Matchmaking: 20 attempts per 60 seconds per IP
            matchmake_rate_limiter: RateLimiter::new(20, 60),
            // SECURITY: Token signer for session tokens
            token_signer: SessionTokenSigner::new(),
            entity_registry: Arc::new(entity_registry),
            item_registry: Arc::new(item_registry),
            prayer_registry: Arc::new(prayer_registry),
            quest_registry,
            crafting_registry: Arc::new(crafting_registry),
            chest_registry: Arc::new(chest_registry),
            interior_registry,
            instance_manager,
            player_instances: Arc::new(RwLock::new(HashMap::new())),
            player_entrance_positions: Arc::new(RwLock::new(HashMap::new())),
            play_time_anchors: Arc::new(DashMap::new()),
            online_characters: Arc::new(DashSet::new()),
            log_buffer,
            perf_metrics: perf_metrics::PerfMetrics::new(),
        }
    }

    async fn get_or_create_room(&self, room_name: &str) -> Arc<GameRoom> {
        // Check if a room with this name already exists
        for room in self.rooms.iter() {
            if room.name == room_name {
                return room.clone();
            }
        }

        // Create new room and store by its UUID
        let room = Arc::new(
            GameRoom::new(
                room_name,
                self.entity_registry.clone(),
                self.quest_registry.clone(),
                self.crafting_registry.clone(),
                self.item_registry.clone(),
                self.prayer_registry.clone(),
                self.player_instances.clone(),
                self.instance_manager.clone(),
                Some(self.db.clone()),
                self.interior_registry.clone(),
                self.chest_registry.clone(),
            )
            .await,
        );
        room.init_top_level_player().await;
        self.rooms.insert(room.id.clone(), room.clone());
        room
    }
}

// ============================================================================
// HTTP Handlers - Authentication
// ============================================================================

/// Auth sessions: token -> (account_id, username)
type AuthSessions = Arc<DashMap<String, (i64, String)>>;

/// Rate limiter entry: (request_count, window_start_time)
type RateLimitEntry = (u32, std::time::Instant);

// ============================================================================
// Signed Session Tokens (Security Hardening)
// ============================================================================

type HmacSha256 = Hmac<Sha256>;

/// Session token validity duration
const SESSION_TOKEN_EXPIRY_SECS: u64 = 300; // 5 minutes

/// Signed session token generator/validator
#[derive(Clone)]
struct SessionTokenSigner {
    /// Secret key for HMAC signing (generated at startup)
    secret: Vec<u8>,
}

impl SessionTokenSigner {
    fn new() -> Self {
        // Generate a random 32-byte secret at startup
        use rand::RngCore;
        let mut secret = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut secret);
        Self { secret }
    }

    /// Create a signed session token
    /// Format: base64(session_id:room_id:expiry_ts:signature)
    fn create_token(&self, session_id: &str, room_id: &str) -> String {
        use base64::Engine;

        let expiry = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + SESSION_TOKEN_EXPIRY_SECS;

        let payload = format!("{}:{}:{}", session_id, room_id, expiry);

        let mut mac =
            HmacSha256::new_from_slice(&self.secret).expect("HMAC can take key of any size");
        mac.update(payload.as_bytes());
        let signature = mac.finalize().into_bytes();

        let token_data = format!(
            "{}:{}",
            payload,
            base64::engine::general_purpose::STANDARD.encode(signature)
        );
        base64::engine::general_purpose::URL_SAFE.encode(token_data)
    }

    /// Validate a signed session token
    /// Returns Some((session_id, room_id)) if valid, None if invalid/expired
    fn validate_token(&self, token: &str) -> Option<(String, String)> {
        use base64::Engine;

        // Decode base64
        let token_data = base64::engine::general_purpose::URL_SAFE
            .decode(token)
            .ok()?;
        let token_str = String::from_utf8(token_data).ok()?;

        // Parse: session_id:room_id:expiry:signature
        let parts: Vec<&str> = token_str.splitn(4, ':').collect();
        if parts.len() != 4 {
            return None;
        }

        let session_id = parts[0];
        let room_id = parts[1];
        let expiry: u64 = parts[2].parse().ok()?;
        let signature_b64 = parts[3];

        // Check expiry
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if now > expiry {
            warn!("Session token expired: {} > {}", now, expiry);
            return None;
        }

        // Verify signature
        let payload = format!("{}:{}:{}", session_id, room_id, expiry);
        let expected_sig = base64::engine::general_purpose::STANDARD
            .decode(signature_b64)
            .ok()?;

        let mut mac =
            HmacSha256::new_from_slice(&self.secret).expect("HMAC can take key of any size");
        mac.update(payload.as_bytes());

        if mac.verify_slice(&expected_sig).is_err() {
            warn!("Session token signature invalid");
            return None;
        }

        Some((session_id.to_string(), room_id.to_string()))
    }
}

/// Simple IP-based rate limiter
#[derive(Clone)]
struct RateLimiter {
    /// IP -> (request_count, window_start)
    entries: Arc<DashMap<String, RateLimitEntry>>,
    /// Max requests per window
    max_requests: u32,
    /// Window duration
    window_duration: Duration,
}

impl RateLimiter {
    fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            max_requests,
            window_duration: Duration::from_secs(window_secs),
        }
    }

    /// Check if request is allowed. Returns true if allowed, false if rate limited.
    fn check(&self, ip: &str) -> bool {
        let now = std::time::Instant::now();

        let mut entry = self.entries.entry(ip.to_string()).or_insert((0, now));
        let (count, window_start) = entry.value_mut();

        // Reset window if expired
        if now.duration_since(*window_start) > self.window_duration {
            *count = 0;
            *window_start = now;
        }

        // Check limit
        if *count >= self.max_requests {
            return false;
        }

        *count += 1;
        true
    }

    /// Record a failed login attempt (for stricter limiting on failures)
    fn record_failure(&self, ip: &str) {
        let now = std::time::Instant::now();
        let mut entry = self.entries.entry(ip.to_string()).or_insert((0, now));
        let (count, _) = entry.value_mut();
        // Add extra penalty for failures
        *count = (*count).saturating_add(2);
    }
}

#[derive(Deserialize)]
struct RegisterRequest {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct LoginRequest {
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

async fn register_account(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    let client_ip = addr.ip().to_string();

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
            // Create auth token - note: (account_id, username) order now
            let token = Uuid::new_v4().to_string();
            state
                .auth_sessions
                .insert(token.clone(), (account_id, req.username.clone()));

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

async fn login_account(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let client_ip = addr.ip().to_string();

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
        Some(account) => {
            // Create auth token - note: (account_id, username) order now
            let token = Uuid::new_v4().to_string();
            state
                .auth_sessions
                .insert(token.clone(), (account.id, req.username.clone()));

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
        None => {
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
    }
}

async fn logout_account(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    if let Some(auth) = headers.get("Authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                state.auth_sessions.remove(token);
            }
        }
    }
    Json(serde_json::json!({ "success": true }))
}

// ============================================================================
// HTTP Handlers - Characters
// ============================================================================

/// Maximum characters per account
const MAX_CHARACTERS_PER_ACCOUNT: i64 = 3;

#[derive(Serialize)]
struct CharacterListResponse {
    success: bool,
    characters: Option<Vec<CharacterInfo>>,
    error: Option<String>,
}

#[derive(Serialize)]
struct CharacterInfo {
    id: i64,
    name: String,
    level: i32,
    gender: String,
    skin: String,
    #[serde(rename = "hairStyle")]
    hair_style: Option<i32>,
    #[serde(rename = "hairColor")]
    hair_color: Option<i32>,
    #[serde(rename = "playedTime")]
    played_time: i64,
    #[serde(rename = "equippedHead")]
    equipped_head: Option<String>,
    #[serde(rename = "equippedBody")]
    equipped_body: Option<String>,
    #[serde(rename = "equippedWeapon")]
    equipped_weapon: Option<String>,
    #[serde(rename = "equippedBack")]
    equipped_back: Option<String>,
    #[serde(rename = "equippedFeet")]
    equipped_feet: Option<String>,
    #[serde(rename = "spriteHead")]
    sprite_head: Option<String>,
    #[serde(rename = "spriteBody")]
    sprite_body: Option<String>,
    #[serde(rename = "spriteWeapon")]
    sprite_weapon: Option<String>,
    #[serde(rename = "spriteBack")]
    sprite_back: Option<String>,
    #[serde(rename = "spriteFeet")]
    sprite_feet: Option<String>,
}

impl CharacterInfo {
    fn resolve_sprite(
        item_registry: &crate::data::item_registry::ItemRegistry,
        item_id: &Option<String>,
    ) -> Option<String> {
        item_id
            .as_ref()
            .and_then(|id| item_registry.get(id).map(|def| def.sprite.clone()))
    }
}

#[derive(Deserialize)]
struct CreateCharacterRequest {
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
fn extract_auth(headers: &axum::http::HeaderMap, sessions: &AuthSessions) -> Option<(i64, String)> {
    let auth_header = headers.get("Authorization")?;
    let auth_str = auth_header.to_str().ok()?;
    let token = auth_str.strip_prefix("Bearer ")?;
    sessions.get(token).map(|r| r.value().clone())
}

/// GET /api/characters - List all characters for the authenticated account
async fn list_characters(
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
                    let sprite_head =
                        CharacterInfo::resolve_sprite(&state.item_registry, &c.equipped_head);
                    let sprite_body =
                        CharacterInfo::resolve_sprite(&state.item_registry, &c.equipped_body);
                    let sprite_weapon =
                        CharacterInfo::resolve_sprite(&state.item_registry, &c.equipped_weapon);
                    let sprite_back =
                        CharacterInfo::resolve_sprite(&state.item_registry, &c.equipped_back);
                    let sprite_feet =
                        CharacterInfo::resolve_sprite(&state.item_registry, &c.equipped_feet);
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
async fn create_character(
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
                            &state.item_registry,
                            &char_data.equipped_head,
                        );
                        let sprite_body = CharacterInfo::resolve_sprite(
                            &state.item_registry,
                            &char_data.equipped_body,
                        );
                        let sprite_weapon = CharacterInfo::resolve_sprite(
                            &state.item_registry,
                            &char_data.equipped_weapon,
                        );
                        let sprite_back = CharacterInfo::resolve_sprite(
                            &state.item_registry,
                            &char_data.equipped_back,
                        );
                        let sprite_feet = CharacterInfo::resolve_sprite(
                            &state.item_registry,
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
async fn delete_character(
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

// ============================================================================
// HTTP Handlers - Matchmaking
// ============================================================================

#[derive(Deserialize)]
struct JoinOptions {
    #[serde(rename = "characterId")]
    character_id: i64,
}

#[derive(Serialize)]
struct MatchmakeResponse {
    room: RoomInfo,
    /// Signed session token for WebSocket upgrade (expires in 5 minutes)
    #[serde(rename = "sessionToken")]
    session_token: String,
}

#[derive(Serialize)]
struct RoomInfo {
    #[serde(rename = "roomId")]
    room_id: String,
    name: String,
    clients: usize,
}

async fn matchmake_join_or_create(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(room_name): Path<String>,
    headers: axum::http::HeaderMap,
    Json(options): Json<JoinOptions>,
) -> impl IntoResponse {
    let client_ip = addr.ip().to_string();

    if !state.matchmake_rate_limiter.check(&client_ip) {
        warn!("Rate limit exceeded for matchmaking from {}", client_ip);
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({ "error": "Too many requests. Please try again later." })),
        )
            .into_response();
    }

    let auth_token = match headers.get("Authorization") {
        Some(auth_header) => match auth_header.to_str() {
            Ok(auth_str) => match auth_str.strip_prefix("Bearer ") {
                Some(token) => token.to_string(),
                None => {
                    warn!("Matchmaking rejected: Invalid Authorization format");
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({
                            "error": "Invalid authorization format. Use 'Bearer <token>'"
                        })),
                    )
                        .into_response();
                }
            },
            Err(_) => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": "Invalid authorization header" })),
                )
                    .into_response();
            }
        },
        None => {
            warn!("Matchmaking rejected: No Authorization header");
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Authorization required. Please login first." })),
            )
                .into_response();
        }
    };

    // Validate token and get authenticated account info
    let (account_id, _username) = match state.auth_sessions.get(&auth_token) {
        Some(auth_data) => auth_data.clone(),
        None => {
            warn!("Matchmaking rejected: Invalid or expired token");
            return (
                StatusCode::UNAUTHORIZED,
                Json(
                    serde_json::json!({ "error": "Invalid or expired token. Please login again." }),
                ),
            )
                .into_response();
        }
    };

    // Load the specified character and verify ownership
    let character_id = options.character_id;
    let character_data = match state.db.get_character(character_id).await {
        Ok(Some(char)) => {
            if char.account_id != account_id {
                warn!(
                    "Matchmaking rejected: Character {} does not belong to account {}",
                    character_id, account_id
                );
                return (
                    StatusCode::FORBIDDEN,
                    Json(
                        serde_json::json!({ "error": "Character does not belong to this account" }),
                    ),
                )
                    .into_response();
            }
            char
        }
        Ok(None) => {
            warn!("Matchmaking rejected: Character {} not found", character_id);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Character not found" })),
            )
                .into_response();
        }
        Err(e) => {
            error!("Failed to load character {}: {}", character_id, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to load character" })),
            )
                .into_response();
        }
    };

    // Check if character is already online — attempt session takeover
    if state.online_characters.contains(&character_id) {
        // Find the old session for this character
        let old_session_id = state
            .sessions
            .iter()
            .find(|entry| entry.value().character_id == character_id)
            .map(|entry| entry.key().clone());

        if let Some(old_sid) = old_session_id {
            // Atomically remove old session — whoever removes it owns cleanup
            if let Some((_, old_sess)) = state.sessions.remove(&old_sid) {
                warn!(
                    "Session takeover: evicting old session {} for character {} ({})",
                    old_sid, old_sess.character_name, character_id
                );

                let old_player_id = old_sess.player_id.clone();

                // Clean up old session state (skip DB save — auto-save covers it)
                if let Some(old_room) = state.rooms.get(&old_sess.room_id) {
                    let old_room = old_room.clone();

                    // Clean up instance tracking
                    {
                        use crate::interior::InstanceType;
                        let removed_instance_id =
                            state.player_instances.write().await.remove(&old_player_id);
                        old_room.reset_sync_state(&old_player_id).await;
                        if let Some(instance_id) = removed_instance_id {
                            if let Some(instance) =
                                state.instance_manager.get_by_instance_id(&instance_id)
                            {
                                let other_players: Vec<String> = instance
                                    .get_player_ids()
                                    .await
                                    .into_iter()
                                    .filter(|id| id != &old_player_id)
                                    .collect();
                                let remaining = instance.remove_player(&old_player_id).await;
                                for other_id in &other_players {
                                    old_room
                                        .send_to_player(
                                            other_id,
                                            ServerMessage::PlayerLeft {
                                                id: old_player_id.clone(),
                                            },
                                        )
                                        .await;
                                }
                                if remaining == 0
                                    && instance.instance_type == InstanceType::Private
                                {
                                    if let Some(owner_id) = &instance.owner_id {
                                        state
                                            .instance_manager
                                            .remove_private(owner_id, &instance.map_id);
                                    }
                                }
                            }
                        }
                    }

                    // Clean up entrance positions
                    state
                        .player_entrance_positions
                        .write()
                        .await
                        .remove(&old_player_id);

                    // Unregister player sender (closes old WebSocket send task)
                    old_room.unregister_player_sender(&old_player_id).await;

                    // Notify friends offline
                    old_room
                        .broadcast_friend_status(&old_player_id, false)
                        .await;

                    // Remove from room and notify overworld
                    old_room.remove_player(&old_player_id).await;
                    old_room
                        .send_to_overworld_players(
                            ServerMessage::PlayerLeft {
                                id: old_player_id.clone(),
                            },
                            None,
                        )
                        .await;
                }

                // Clean up play time anchor (skip saving — auto-save covers it)
                state.play_time_anchors.remove(&character_id);

                // Mark offline (will be re-marked online when new socket connects)
                state.online_characters.remove(&character_id);
            }
        } else {
            // Character marked online but no session found — clean up stale state
            warn!(
                "Session takeover: character {} marked online but no session found, cleaning up",
                character_id
            );
            state.online_characters.remove(&character_id);
        }
    }

    let room = state.get_or_create_room(&room_name).await;
    let room_id = room.id.clone();

    // Create session for this character
    let session_id = Uuid::new_v4().to_string();
    let player_id = format!("char_{}", character_id);

    // NOTE: We do NOT mark the character as online here. That happens when the
    // WebSocket actually connects (in handle_socket or spectator upgrade).
    // This prevents orphaned online_characters entries if the client never connects.

    // Reserve the session with character info
    state.sessions.insert(
        session_id.clone(),
        GameSession {
            room_id: room_id.clone(),
            player_id: player_id.clone(),
            character_name: character_data.name.clone(),
            character_id,
            account_id,
            auth_token: auth_token.clone(),
            current_map: character_data.current_map.clone(),
            entrance_x: character_data.entrance_x,
            entrance_y: character_data.entrance_y,
            is_new_character: character_data.played_time == 0,
        },
    );

    info!(
        "Tutorial: character '{}' played_time={}, is_new_character={}",
        character_data.name,
        character_data.played_time,
        character_data.played_time == 0
    );

    // Start tracking play time for this character
    state
        .play_time_anchors
        .insert(character_id, std::time::Instant::now());

    // Load saved character into the game room
    info!(
        "Loading character: {} (id: {}) at ({}, {}) as {} {}",
        character_data.name,
        character_id,
        character_data.x,
        character_data.y,
        character_data.gender,
        character_data.skin
    );

    room.reserve_player_with_data(
        &player_id,
        &character_data.name,
        character_data.x as i32,
        character_data.y as i32,
        character_data.z,
        character_data.hp,
        character_data.prayer_points,
        character_data.mp,
        character_data.skills.clone(),
        character_data.gold,
        &character_data.inventory_json,
        &character_data.gender,
        &character_data.skin,
        character_data.hair_style,
        character_data.hair_color,
        character_data.equipped_head.clone(),
        character_data.equipped_body.clone(),
        character_data.equipped_weapon.clone(),
        character_data.equipped_back.clone(),
        character_data.equipped_feet.clone(),
        character_data.equipped_ring.clone(),
        character_data.equipped_gloves.clone(),
        character_data.equipped_necklace.clone(),
        character_data.equipped_belt.clone(),
        character_data.is_admin,
        character_data.sitting_at_x,
        character_data.sitting_at_y,
        &character_data.bank_json,
        character_data.bank_gold,
        character_data.bank_max_slots,
        &character_data.combat_style_prefs,
    )
    .await;

    // Load quest state from database
    match state.db.load_character_quest_state(character_id).await {
        Ok(quest_state) => {
            let active_count = quest_state.active_quests.len();
            let completed_count = quest_state.completed_quests.len();
            room.set_player_quest_state(&player_id, quest_state).await;
            if active_count > 0 || completed_count > 0 {
                info!(
                    "Loaded quest state for {}: {} active, {} completed",
                    character_data.name, active_count, completed_count
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load quest state for character {}: {}",
                character_id,
                e
            );
            // Continue with empty quest state (default)
        }
    }

    // Load discovered recipes from database
    match state.db.load_discovered_recipes(character_id).await {
        Ok(recipes) => {
            let count = recipes.len();
            let recipe_set: std::collections::HashSet<String> = recipes.into_iter().collect();
            room.set_player_discovered_recipes(&player_id, recipe_set)
                .await;
            if count > 0 {
                info!(
                    "Loaded {} discovered recipes for {}",
                    count, character_data.name
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load discovered recipes for character {}: {}",
                character_id,
                e
            );
        }
    }

    // Load unlocked spells from database
    match state.db.load_unlocked_spells(character_id).await {
        Ok(spells) => {
            let count = spells.len();
            let spell_set: std::collections::HashSet<String> = spells.into_iter().collect();
            room.set_player_unlocked_spells(&player_id, spell_set).await;
            if count > 0 {
                info!(
                    "Loaded {} unlocked spells for {}",
                    count, character_data.name
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load unlocked spells for character {}: {}",
                character_id,
                e
            );
        }
    }

    // Load slayer state from database
    let slayer_state = state
        .db
        .load_character_slayer_state(character_id)
        .await
        .unwrap_or_default();
    room.set_player_slayer_state(&player_id, slayer_state.clone())
        .await;
    if slayer_state.current_task.is_some() || slayer_state.tasks_completed > 0 {
        info!(
            "Loaded slayer state for {}: {} tasks completed, {} points",
            character_data.name, slayer_state.tasks_completed, slayer_state.points
        );
    }

    let client_count = room.player_count().await;

    // Generate signed session token for WebSocket upgrade
    let session_token = state.token_signer.create_token(&session_id, &room_id);

    info!(
        "Matchmaking: room={}, character={} (id: {})",
        room_id, character_data.name, character_id
    );

    Json(MatchmakeResponse {
        room: RoomInfo {
            room_id,
            name: room_name,
            clients: client_count,
        },
        session_token,
    })
    .into_response()
}

// ============================================================================
// Stats API Handlers (public, read-only)
// ============================================================================

#[derive(Serialize)]
struct StatsOverview {
    online_players: usize,
    total_characters: i64,
    total_accounts: i64,
}

async fn stats_overview(State(state): State<AppState>) -> impl IntoResponse {
    // Count online players from rooms
    let mut online = 0usize;
    for entry in state.rooms.iter() {
        online += entry.value().player_count().await;
    }

    // Count total characters and accounts from DB
    let pool = state.db.pool();
    let total_characters: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM characters WHERE is_admin = FALSE")
            .fetch_one(pool)
            .await
            .unwrap_or(0);
    let total_accounts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    Json(StatsOverview {
        online_players: online,
        total_characters,
        total_accounts,
    })
}

#[derive(Serialize)]
struct OnlinePlayer {
    name: String,
    combat_level: i32,
    hitpoints_level: i32,
    attack_level: i32,
    strength_level: i32,
    defence_level: i32,
    ranged_level: i32,
    total_level: i32,
}

async fn stats_online(State(state): State<AppState>) -> impl IntoResponse {
    let mut players = Vec::new();
    for entry in state.rooms.iter() {
        for p in entry.value().get_all_players().await {
            if p.is_admin {
                continue;
            }
            players.push(OnlinePlayer {
                name: p.name.clone(),
                combat_level: p.skills.combat_level(),
                hitpoints_level: p.skills.hitpoints.level,
                attack_level: p.skills.attack.level,
                strength_level: p.skills.strength.level,
                defence_level: p.skills.defence.level,
                ranged_level: p.skills.ranged.level,
                total_level: p.skills.total_level(),
            });
        }
    }
    Json(players)
}

#[derive(Deserialize)]
struct LeaderboardQuery {
    #[serde(default = "default_leaderboard_sort")]
    sort: String,
    #[serde(default = "default_leaderboard_limit")]
    limit: usize,
}

fn default_leaderboard_sort() -> String {
    "total_level".to_string()
}
fn default_leaderboard_limit() -> usize {
    50
}

#[derive(Serialize, Clone)]
struct LeaderboardEntry {
    name: String,
    combat_level: i32,
    hitpoints_level: i32,
    attack_level: i32,
    strength_level: i32,
    defence_level: i32,
    ranged_level: i32,
    fishing_level: i32,
    farming_level: i32,
    smithing_level: i32,
    prayer_level: i32,
    magic_level: i32,
    woodcutting_level: i32,
    mining_level: i32,
    alchemy_level: i32,
    slayer_level: i32,
    total_level: i32,
    played_time: i64,
    monster_kills: i32,
}

#[derive(Serialize)]
struct PlayerProfileRanks {
    total_level: usize,
    combat_level: usize,
    hitpoints_level: usize,
    attack_level: usize,
    strength_level: usize,
    defence_level: usize,
    ranged_level: usize,
    fishing_level: usize,
    farming_level: usize,
    smithing_level: usize,
    prayer_level: usize,
    magic_level: usize,
    woodcutting_level: usize,
    mining_level: usize,
    alchemy_level: usize,
    slayer_level: usize,
    monster_kills: usize,
    played_time: usize,
}

#[derive(Serialize)]
struct PlayerProfileResponse {
    player: LeaderboardEntry,
    ranks: PlayerProfileRanks,
    total_characters: usize,
}

async fn load_leaderboard_entries(state: &AppState) -> Vec<LeaderboardEntry> {
    let rows = sqlx::query("SELECT name, skills_json, played_time, monster_kills FROM characters WHERE is_admin = FALSE")
        .fetch_all(state.db.pool())
        .await
        .unwrap_or_default();

    rows.into_iter()
        .filter_map(|row| {
            let name: String = row.try_get("name").ok()?;
            let skills_json: String = row.try_get("skills_json").unwrap_or_default();
            let played_time: i64 = row.try_get("played_time").unwrap_or(0);
            let monster_kills: i32 = row.try_get("monster_kills").unwrap_or(0);
            let skills = Skills::from_json(&skills_json);
            Some(LeaderboardEntry {
                name,
                combat_level: skills.combat_level(),
                hitpoints_level: skills.hitpoints.level,
                attack_level: skills.attack.level,
                strength_level: skills.strength.level,
                defence_level: skills.defence.level,
                ranged_level: skills.ranged.level,
                fishing_level: skills.fishing.level,
                farming_level: skills.farming.level,
                smithing_level: skills.smithing.level,
                prayer_level: skills.prayer.level,
                magic_level: skills.magic.level,
                woodcutting_level: skills.woodcutting.level,
                mining_level: skills.mining.level,
                alchemy_level: skills.alchemy.level,
                slayer_level: skills.slayer.level,
                total_level: skills.total_level(),
                played_time,
                monster_kills,
            })
        })
        .collect()
}

fn sort_leaderboard_entries(entries: &mut [LeaderboardEntry], sort: &str) {
    match sort {
        "combat_level" => entries.sort_by(|a, b| b.combat_level.cmp(&a.combat_level)),
        "hitpoints_level" => entries.sort_by(|a, b| b.hitpoints_level.cmp(&a.hitpoints_level)),
        "attack_level" => entries.sort_by(|a, b| b.attack_level.cmp(&a.attack_level)),
        "strength_level" => entries.sort_by(|a, b| b.strength_level.cmp(&a.strength_level)),
        "defence_level" => entries.sort_by(|a, b| b.defence_level.cmp(&a.defence_level)),
        "ranged_level" => entries.sort_by(|a, b| b.ranged_level.cmp(&a.ranged_level)),
        "fishing_level" => entries.sort_by(|a, b| b.fishing_level.cmp(&a.fishing_level)),
        "farming_level" => entries.sort_by(|a, b| b.farming_level.cmp(&a.farming_level)),
        "smithing_level" => entries.sort_by(|a, b| b.smithing_level.cmp(&a.smithing_level)),
        "prayer_level" => entries.sort_by(|a, b| b.prayer_level.cmp(&a.prayer_level)),
        "magic_level" => entries.sort_by(|a, b| b.magic_level.cmp(&a.magic_level)),
        "woodcutting_level" => {
            entries.sort_by(|a, b| b.woodcutting_level.cmp(&a.woodcutting_level))
        }
        "mining_level" => entries.sort_by(|a, b| b.mining_level.cmp(&a.mining_level)),
        "alchemy_level" => entries.sort_by(|a, b| b.alchemy_level.cmp(&a.alchemy_level)),
        "slayer_level" => entries.sort_by(|a, b| b.slayer_level.cmp(&a.slayer_level)),
        "monster_kills" => entries.sort_by(|a, b| b.monster_kills.cmp(&a.monster_kills)),
        "played_time" => entries.sort_by(|a, b| b.played_time.cmp(&a.played_time)),
        _ => entries.sort_by(|a, b| b.total_level.cmp(&a.total_level)),
    }
}

fn stat_rank<F>(entries: &[LeaderboardEntry], player: &LeaderboardEntry, value: F) -> usize
where
    F: Fn(&LeaderboardEntry) -> i64,
{
    let player_value = value(player);
    1 + entries
        .iter()
        .filter(|entry| value(entry) > player_value)
        .count()
}

async fn stats_leaderboard(
    State(state): State<AppState>,
    Query(query): Query<LeaderboardQuery>,
) -> impl IntoResponse {
    let mut entries = load_leaderboard_entries(&state).await;
    sort_leaderboard_entries(&mut entries, &query.sort);

    entries.truncate(query.limit.min(100));
    Json(entries)
}

async fn stats_player_profile(
    State(state): State<AppState>,
    Path(player_name): Path<String>,
) -> impl IntoResponse {
    let entries = load_leaderboard_entries(&state).await;
    let Some(player) = entries
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(&player_name))
        .cloned()
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Player not found"
            })),
        )
            .into_response();
    };

    let ranks = PlayerProfileRanks {
        total_level: stat_rank(&entries, &player, |entry| entry.total_level as i64),
        combat_level: stat_rank(&entries, &player, |entry| entry.combat_level as i64),
        hitpoints_level: stat_rank(&entries, &player, |entry| entry.hitpoints_level as i64),
        attack_level: stat_rank(&entries, &player, |entry| entry.attack_level as i64),
        strength_level: stat_rank(&entries, &player, |entry| entry.strength_level as i64),
        defence_level: stat_rank(&entries, &player, |entry| entry.defence_level as i64),
        ranged_level: stat_rank(&entries, &player, |entry| entry.ranged_level as i64),
        fishing_level: stat_rank(&entries, &player, |entry| entry.fishing_level as i64),
        farming_level: stat_rank(&entries, &player, |entry| entry.farming_level as i64),
        smithing_level: stat_rank(&entries, &player, |entry| entry.smithing_level as i64),
        prayer_level: stat_rank(&entries, &player, |entry| entry.prayer_level as i64),
        magic_level: stat_rank(&entries, &player, |entry| entry.magic_level as i64),
        woodcutting_level: stat_rank(&entries, &player, |entry| entry.woodcutting_level as i64),
        mining_level: stat_rank(&entries, &player, |entry| entry.mining_level as i64),
        alchemy_level: stat_rank(&entries, &player, |entry| entry.alchemy_level as i64),
        slayer_level: stat_rank(&entries, &player, |entry| entry.slayer_level as i64),
        monster_kills: stat_rank(&entries, &player, |entry| entry.monster_kills as i64),
        played_time: stat_rank(&entries, &player, |entry| entry.played_time),
    };

    Json(PlayerProfileResponse {
        player,
        ranks,
        total_characters: entries.len(),
    })
    .into_response()
}

#[derive(Serialize)]
struct StatsEquipment {
    slot_type: String,
    attack_level_required: i32,
    defence_level_required: i32,
    ranged_level_required: i32,
    attack_bonus: i32,
    strength_bonus: i32,
    defence_bonus: i32,
    ranged_strength_bonus: i32,
    weapon_type: String,
    range: i32,
}

#[derive(Serialize)]
struct StatsItem {
    id: String,
    display_name: String,
    sprite: String,
    description: String,
    category: String,
    max_stack: i32,
    base_price: i32,
    sellable: bool,
    equipment: Option<StatsEquipment>,
}

#[derive(Serialize)]
struct StatsEntityLoot {
    item_id: String,
    drop_chance: f32,
    quantity_min: i32,
    quantity_max: i32,
}

#[derive(Serialize)]
struct StatsLootTableEntry {
    item_id: String,
    weight: i32,
    quantity_min: i32,
    quantity_max: i32,
}

#[derive(Serialize)]
struct StatsLootTable {
    name: String,
    chance: f32,
    entries: Vec<StatsLootTableEntry>,
}

#[derive(Serialize)]
struct StatsEntity {
    id: String,
    display_name: String,
    sprite: String,
    description: String,
    level: i32,
    max_hp: i32,
    damage: i32,
    attack_bonus: i32,
    defence_bonus: i32,
    attack_range: i32,
    aggro_range: i32,
    respawn_time_ms: u64,
    hostile: bool,
    exp_base: i32,
    gold_min: i32,
    gold_max: i32,
    loot: Vec<StatsEntityLoot>,
    loot_tables: Vec<StatsLootTable>,
    quest_ids: Vec<String>,
}

async fn stats_items(State(state): State<AppState>) -> impl IntoResponse {
    let items: Vec<StatsItem> = state
        .item_registry
        .all()
        .map(|item| StatsItem {
            id: item.id.clone(),
            display_name: item.display_name.clone(),
            sprite: item.sprite.clone(),
            description: item.description.clone(),
            category: format!("{:?}", item.category).to_lowercase(),
            max_stack: item.max_stack,
            base_price: item.base_price,
            sellable: item.sellable,
            equipment: item.equipment.as_ref().and_then(|eq| {
                if eq.slot_type == EquipmentSlot::None {
                    return None;
                }
                Some(StatsEquipment {
                    slot_type: eq.slot_type.as_str().to_string(),
                    attack_level_required: eq.attack_level_required,
                    defence_level_required: eq.defence_level_required,
                    ranged_level_required: eq.ranged_level_required,
                    attack_bonus: eq.attack_bonus,
                    strength_bonus: eq.strength_bonus,
                    defence_bonus: eq.defence_bonus,
                    ranged_strength_bonus: eq.ranged_strength_bonus,
                    weapon_type: format!("{:?}", eq.weapon_type),
                    range: eq.range,
                })
            }),
        })
        .collect();
    Json(items)
}

async fn stats_entities(State(state): State<AppState>) -> impl IntoResponse {
    // Collect quest kill-objective targets
    let all_quests = state.quest_registry.all_quests().await;
    let mut quest_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for quest in &all_quests {
        for obj in &quest.objectives {
            if obj.objective_type == ObjectiveType::KillMonster {
                quest_map.entry(obj.target.clone()).or_default().push(quest.id.clone());
            }
        }
    }

    let entities: Vec<StatsEntity> = state
        .entity_registry
        .all()
        .filter(|e| e.is_hostile())
        .map(|e| StatsEntity {
            id: e.id.clone(),
            display_name: e.display_name.clone(),
            sprite: e.sprite.clone(),
            description: e.description.clone(),
            level: e.stats.level,
            max_hp: e.stats.max_hp,
            damage: e.stats.damage,
            attack_bonus: e.stats.attack_bonus,
            defence_bonus: e.stats.defence_bonus,
            attack_range: e.stats.attack_range,
            aggro_range: e.stats.aggro_range,
            respawn_time_ms: e.stats.respawn_time_ms,
            hostile: e.behaviors.hostile,
            exp_base: e.rewards.exp_base,
            gold_min: e.rewards.gold_min,
            gold_max: e.rewards.gold_max,
            loot: e.loot.iter().map(|l| StatsEntityLoot {
                item_id: l.item_id.clone(),
                drop_chance: l.drop_chance,
                quantity_min: l.quantity_min,
                quantity_max: l.quantity_max,
            }).collect(),
            loot_tables: e.loot_tables.iter().map(|t| StatsLootTable {
                name: t.name.clone(),
                chance: t.chance,
                entries: t.entries.iter().map(|e| StatsLootTableEntry {
                    item_id: e.item_id.clone(),
                    weight: e.weight,
                    quantity_min: e.quantity_min,
                    quantity_max: e.quantity_max,
                }).collect(),
            }).collect(),
            quest_ids: quest_map.get(&e.id).cloned().unwrap_or_default(),
        })
        .collect();
    Json(entities)
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().timestamp_millis()
    }))
}

// ============================================================================
// Server Logs
// ============================================================================

#[derive(Deserialize)]
struct LogsQuery {
    count: Option<usize>,
    level: Option<String>,
    important: Option<bool>,
}

#[derive(Deserialize)]
struct PerfQuery {
    rooms: Option<usize>,
    spikes: Option<usize>,
}

async fn api_logs(
    State(state): State<AppState>,
    Query(params): Query<LogsQuery>,
) -> impl IntoResponse {
    let important_only = params.important.unwrap_or(false);
    let max_count = if important_only { 5000 } else { 1000 };
    let default_count = if important_only { 500 } else { 200 };
    let count = params.count.unwrap_or(default_count).min(max_count);
    let entries = if important_only {
        state.log_buffer.recent_important(count)
    } else {
        state.log_buffer.recent(count)
    };

    let entries: Vec<_> = if let Some(level_filter) = &params.level {
        let level_upper = level_filter.to_uppercase();
        entries
            .into_iter()
            .filter(|e| e.level == level_upper)
            .collect()
    } else {
        entries
    };

    Json(entries)
}

async fn api_perf(
    State(state): State<AppState>,
    Query(params): Query<PerfQuery>,
) -> impl IntoResponse {
    let top_rooms = params.rooms.unwrap_or(10).clamp(1, 50);
    let recent_spikes = params.spikes.unwrap_or(50).min(200);
    Json(state.perf_metrics.snapshot(top_rooms, recent_spikes))
}

async fn logs_page() -> impl IntoResponse {
    axum::response::Html(include_str!("logs.html"))
}

// ============================================================================
// WebSocket Handler
// ============================================================================

#[derive(Deserialize)]
struct WsQuery {
    /// Signed session token
    #[serde(rename = "sessionToken")]
    session_token: String,
}

// ============================================================================
// Spectator WebSocket Handler
// ============================================================================

const MAX_SPECTATORS: usize = 50;

async fn spectate_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Get or create the main game room
    let room = state.get_or_create_room("game_room").await;

    // Rate limit spectators
    if room.spectator_count().await >= MAX_SPECTATORS {
        return (StatusCode::SERVICE_UNAVAILABLE, "Too many spectators").into_response();
    }

    ws.on_upgrade(move |socket| handle_spectator(socket, state, room))
        .into_response()
}

async fn handle_spectator(socket: WebSocket, state: AppState, room: Arc<GameRoom>) {
    let (mut sender, mut receiver) = socket.split();

    let spectator_id = Uuid::new_v4().to_string();
    info!("Spectator {} connected", spectator_id);

    // Send initial chunks around spawn (5x5 area)
    let spawn_chunk =
        chunk::ChunkCoord::from_world(crate::game::WORLD_SPAWN_X, crate::game::WORLD_SPAWN_Y);
    for dy in -2..=2 {
        for dx in -2..=2 {
            let coord = chunk::ChunkCoord::new(spawn_chunk.x + dx, spawn_chunk.y + dy);
            if let Some(chunk_msg) = room.handle_chunk_request(coord.x, coord.y).await {
                if let Ok(bytes) = protocol::encode_server_message(&chunk_msg) {
                    let _ = sender.send(Message::Binary(bytes)).await;
                }
            }
        }
    }

    // Create channel for sending messages to this spectator
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(256);

    // Register spectator sender so tick loop can send StateSync
    room.add_spectator(&spectator_id, tx.clone()).await;

    // Subscribe to room broadcasts
    let mut broadcast_rx = room.subscribe();

    // Spawn send loop task (forward mpsc + broadcast to WebSocket)
    let send_spectator_id = spectator_id.clone();
    let mut send_task = tokio::spawn(async move {
        let mut ping_interval = tokio::time::interval(Duration::from_secs(15));
        ping_interval.tick().await; // consume immediate first tick
        loop {
            tokio::select! {
                biased;

                // Handle direct messages (spectator StateSync)
                Some(msg) = rx.recv() => {
                    if sender.send(Message::Binary(msg)).await.is_err() {
                        break;
                    }
                }
                // Handle broadcast messages
                result = broadcast_rx.recv() => {
                    match result {
                        Ok(bytes) => {
                            if sender.send(Message::Binary(bytes)).await.is_err() {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("Broadcast lagged for spectator {}: skipped {} messages", send_spectator_id, n);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                // Server-side WebSocket ping to keep connection alive (browsers auto-pong)
                _ = ping_interval.tick() => {
                    if sender.send(Message::Ping(vec![])).await.is_err() {
                        break;
                    }
                }
                else => break,
            }
        }
    });

    // Spawn recv loop task — ignore all messages except SpectatorUpgrade
    // Returns Some((session_id, player_id, character_name, character_id)) if upgraded
    // Shutdown signal: send_task can notify recv_task to stop so it can run cleanup
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    let recv_room = room.clone();
    let recv_spectator_id = spectator_id.clone();
    let recv_state = state.clone();
    let recv_tx = tx.clone();
    let mut recv_task = tokio::spawn(async move {
        // Phase 1: Spectator mode — wait for upgrade request
        loop {
            let msg = match tokio::time::timeout(Duration::from_secs(30), receiver.next()).await {
                Ok(Some(Ok(msg))) => msg,
                Ok(Some(Err(_))) | Ok(None) => break,
                Err(_) => {
                    warn!("Spectator {} connection timed out (no data for 30s)", recv_spectator_id);
                    break;
                }
            };
            match msg {
                Message::Binary(data) => {
                    match protocol::decode_client_message(&data) {
                        Ok(ClientMessage::SpectatorUpgrade { session_token }) => {
                            info!(
                                "Spectator {} upgrade requested, token: {}...",
                                recv_spectator_id,
                                &session_token[..session_token.len().min(8)]
                            );

                            // --- Step 1: Validate session token ---
                            let (session_id, room_id) = match recv_state
                                .token_signer
                                .validate_token(&session_token)
                            {
                                Some((sid, rid)) => (sid, rid),
                                None => {
                                    warn!(
                                        "Spectator {} upgrade rejected: invalid or expired session token",
                                        recv_spectator_id
                                    );
                                    let err_msg = ServerMessage::Error {
                                        code: 401,
                                        message: "Invalid or expired session token".to_string(),
                                    };
                                    if let Ok(bytes) = protocol::encode_server_message(&err_msg) {
                                        let _ = recv_tx.send(bytes).await;
                                    }
                                    continue;
                                }
                            };

                            // --- Step 2: Look up session in state.sessions ---
                            let session =
                                match recv_state.sessions.get(&session_id).map(|s| s.clone()) {
                                    Some(s) if s.room_id == room_id => s,
                                    _ => {
                                        warn!(
                                            "Spectator {} upgrade rejected: invalid session {}",
                                            recv_spectator_id, session_id
                                        );
                                        let err_msg = ServerMessage::Error {
                                            code: 403,
                                            message: "Invalid session".to_string(),
                                        };
                                        if let Ok(bytes) = protocol::encode_server_message(&err_msg)
                                        {
                                            let _ = recv_tx.send(bytes).await;
                                        }
                                        continue;
                                    }
                                };

                            // --- Step 3: Verify auth token is still valid ---
                            if !recv_state.auth_sessions.contains_key(&session.auth_token) {
                                warn!(
                                    "Spectator {} upgrade rejected: auth token expired for session {}",
                                    recv_spectator_id, session_id
                                );
                                let err_msg = ServerMessage::Error {
                                    code: 401,
                                    message: "Auth token expired. Please login again.".to_string(),
                                };
                                if let Ok(bytes) = protocol::encode_server_message(&err_msg) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                                continue;
                            }

                            let player_id = session.player_id.clone();
                            let character_name = session.character_name.clone();
                            let character_id = session.character_id;
                            let current_map = session.current_map.clone();
                            let entrance_x = session.entrance_x;
                            let entrance_y = session.entrance_y;
                            let is_new_character = session.is_new_character;

                            // --- Step 4: Remove spectator registration ---
                            recv_room.remove_spectator(&recv_spectator_id).await;
                            info!(
                                "Spectator {} upgrading to player {} ({})",
                                recv_spectator_id, character_name, player_id
                            );

                            // --- Step 5: Register the existing mpsc sender as the player's sender ---
                            recv_room
                                .register_player_sender(&player_id, recv_tx.clone())
                                .await;

                            // --- Step 6: Mark online and activate the player entity ---
                            recv_state.online_characters.insert(character_id);
                            let player_name = recv_room.activate_player(&player_id).await;

                            // --- Step 7: Send all initial data via the mpsc channel ---

                            // Welcome message
                            let welcome = ServerMessage::Welcome {
                                player_id: player_id.clone(),
                                is_new_character,
                            };
                            if let Ok(bytes) = protocol::encode_server_message(&welcome) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Entity definitions
                            let entity_defs = recv_room.get_entity_definitions();
                            if let Ok(bytes) = protocol::encode_server_message(&entity_defs) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Item definitions
                            let item_defs = recv_state.item_registry.to_client_definitions();
                            if let Ok(bytes) = protocol::encode_server_message(&item_defs) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Recipe definitions
                            let recipe_defs = recv_state.crafting_registry.to_client_definitions();
                            if let Ok(bytes) = protocol::encode_server_message(&recipe_defs) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Discovered recipes
                            let discovered =
                                recv_room.get_player_discovered_recipes(&player_id).await;
                            let discovered_msg = ServerMessage::DiscoveredRecipes {
                                recipes: discovered.into_iter().collect(),
                            };
                            if let Ok(bytes) = protocol::encode_server_message(&discovered_msg) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Scroll spell definitions
                            let scroll_spell_defs_msg =
                                recv_room.get_scroll_spell_definitions_message();
                            if let Ok(bytes) =
                                protocol::encode_server_message(&scroll_spell_defs_msg)
                            {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Unlocked spells
                            let unlocked = recv_room.get_player_unlocked_spells(&player_id).await;
                            let unlocked_msg = ServerMessage::UnlockedSpellsSync {
                                spell_ids: unlocked.into_iter().collect(),
                            };
                            if let Ok(bytes) = protocol::encode_server_message(&unlocked_msg) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Gathering markers
                            let gathering_markers = recv_room.get_gathering_markers_message(None).await;
                            if let Ok(bytes) = protocol::encode_server_message(&gathering_markers) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Farming patches (per-player)
                            let farming_patches =
                                recv_room.get_farming_patches_message(&player_id).await;
                            if let Ok(bytes) = protocol::encode_server_message(&farming_patches) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Farming contract
                            let contract_msg =
                                recv_room.get_farming_contract_message(&player_id).await;
                            if let Ok(bytes) = protocol::encode_server_message(&contract_msg) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Chair positions
                            let chair_positions = recv_room.get_chair_positions_message().await;
                            if let Ok(bytes) = protocol::encode_server_message(&chair_positions) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Overworld chest positions
                            let chest_positions = recv_room.get_chest_positions_message(None).await;
                            if let Ok(bytes) = protocol::encode_server_message(&chest_positions) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Prayer state
                            if let Some(prayer_state) =
                                recv_room.get_player_prayer_state(&player_id).await
                            {
                                if let Ok(bytes) = protocol::encode_server_message(&prayer_state) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Only send overworld data if not reconnecting into an instance
                            let reconnecting_to_instance = current_map.is_some();

                            if !reconnecting_to_instance {
                                // Send nearby chunks
                                if let Some((px, py)) =
                                    recv_room.get_player_position(&player_id).await
                                {
                                    let player_chunk = chunk::ChunkCoord::from_world(px, py);
                                    for dy in -1..=1 {
                                        for dx in -1..=1 {
                                            let coord = chunk::ChunkCoord::new(
                                                player_chunk.x + dx,
                                                player_chunk.y + dy,
                                            );
                                            if let Some(chunk_msg) = recv_room
                                                .handle_chunk_request(coord.x, coord.y)
                                                .await
                                            {
                                                if let Ok(bytes) =
                                                    protocol::encode_server_message(&chunk_msg)
                                                {
                                                    let _ = recv_tx.send(bytes).await;
                                                }
                                            }
                                        }
                                    }
                                }

                                // Send existing players (only overworld, not instanced)
                                {
                                    let instanced_players =
                                        recv_state.player_instances.read().await;
                                    for existing_player in recv_room.get_all_players().await {
                                        if existing_player.id != player_id
                                            && !instanced_players.contains_key(&existing_player.id)
                                        {
                                            let msg = ServerMessage::PlayerJoined {
                                                id: existing_player.id.clone(),
                                                name: existing_player.name.clone(),
                                                x: existing_player.x,
                                                y: existing_player.y,
                                                gender: existing_player.gender.clone(),
                                                skin: existing_player.skin.clone(),
                                                hair_style: existing_player.hair_style,
                                                hair_color: existing_player.hair_color,
                                            };
                                            if let Ok(bytes) = protocol::encode_server_message(&msg)
                                            {
                                                let _ = recv_tx.send(bytes).await;
                                            }
                                        }
                                    }
                                }

                                // Send existing overworld ground items
                                let ground_items =
                                    recv_room.get_ground_items_in_instance(None).await;
                                for item_msg in ground_items {
                                    if let Ok(bytes) = protocol::encode_server_message(&item_msg) {
                                        let _ = recv_tx.send(bytes).await;
                                    }
                                }
                            }

                            // Broadcast PlayerJoined to others
                            let (x, y) = recv_room
                                .get_player_position(&player_id)
                                .await
                                .unwrap_or((0, 0));
                            let (gender, skin) = recv_room
                                .get_player_appearance(&player_id)
                                .await
                                .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
                            let (hair_style, hair_color) = recv_room
                                .get_player_hair(&player_id)
                                .await
                                .unwrap_or((None, None));
                            let player_joined_msg = ServerMessage::PlayerJoined {
                                id: player_id.clone(),
                                name: player_name.clone(),
                                x,
                                y,
                                gender: gender.clone(),
                                skin: skin.clone(),
                                hair_style,
                                hair_color,
                            };

                            // Send PlayerJoined to self first
                            if let Ok(bytes) = protocol::encode_server_message(&player_joined_msg) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Notify other overworld players
                            recv_room
                                .send_to_overworld_players(player_joined_msg, Some(&player_id))
                                .await;

                            // If player was sitting, send SitResult
                            if let Some((sx, sy, direction)) =
                                recv_room.get_player_sitting_info(&player_id).await
                            {
                                let sit_msg = ServerMessage::SitResult {
                                    success: true,
                                    tile_x: sx,
                                    tile_y: sy,
                                    direction,
                                };
                                if let Ok(bytes) = protocol::encode_server_message(&sit_msg) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Quest progression snapshot
                            recv_room
                                .process_quest_progression_snapshot(&player_id)
                                .await;

                            // Active quests
                            for quest_msg in recv_room.get_active_quest_messages(&player_id).await {
                                if let Ok(bytes) = protocol::encode_server_message(&quest_msg) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Completed quest sync
                            let quest_state_sync =
                                recv_room.get_completed_quest_sync_message(&player_id).await;
                            if let Ok(bytes) = protocol::encode_server_message(&quest_state_sync) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Quest catalog
                            let quest_catalog = recv_room.build_quest_catalog().await;
                            if let Ok(bytes) = protocol::encode_server_message(&quest_catalog) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Inventory
                            if let Some(inv_msg) =
                                recv_room.get_player_inventory_update(&player_id).await
                            {
                                if let Ok(bytes) = protocol::encode_server_message(&inv_msg) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Skills
                            if let Some(skills_msg) =
                                recv_room.get_player_skills_sync(&player_id).await
                            {
                                if let Ok(bytes) = protocol::encode_server_message(&skills_msg) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Potion buffs
                            if let Some(buffs_msg) =
                                recv_room.get_player_potion_buffs_sync(&player_id).await
                            {
                                if let Ok(bytes) = protocol::encode_server_message(&buffs_msg) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Top total level player (trophy icon) — refresh from DB and broadcast to all
                            recv_room.init_top_level_player().await;
                            {
                                let top_msg = recv_room.get_top_player_message().await;
                                recv_room.broadcast(top_msg.clone()).await;
                                if let Ok(bytes) = protocol::encode_server_message(&top_msg) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Slayer state
                            {
                                let slayer_state =
                                    recv_room.get_player_slayer_state(&player_id).await;
                                let slayer_task_data =
                                    slayer_state.current_task.as_ref().map(|t| {
                                        crate::protocol::SlayerTaskData {
                                            monster_id: t.monster_id.clone(),
                                            display_name: t.display_name.clone(),
                                            kills_current: t.kills_current,
                                            kills_required: t.kills_required,
                                            xp_per_kill: t.xp_per_kill,
                                            master_id: t.master_id.clone(),
                                            points_on_complete: t.points_on_complete,
                                        }
                                    });
                                let slayer_sync = ServerMessage::SlayerStateSync {
                                    current_task: slayer_task_data,
                                    points: slayer_state.points,
                                    tasks_completed: slayer_state.tasks_completed,
                                    blocked_monsters: slayer_state.blocked_monsters.clone(),
                                    unlocked_monsters: slayer_state.unlocked_monsters.clone(),
                                };
                                if let Ok(bytes) = protocol::encode_server_message(&slayer_sync) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Friends data (must be after sender is registered)
                            recv_room
                                .send_friends_data(&player_id, &recv_state.online_characters)
                                .await;

                            // Notify friends that this player came online
                            recv_room.broadcast_friend_status(&player_id, true).await;

                            // Auto-re-enter instance if applicable
                            if let Some(ref map_id) = current_map {
                                info!(
                                    "Auto-re-entering instance '{}' for reconnecting player {}",
                                    map_id, player_id
                                );
                                auto_enter_instance(
                                    &recv_state,
                                    &recv_room,
                                    &player_id,
                                    map_id,
                                    entrance_x,
                                    entrance_y,
                                )
                                .await;
                            }

                            info!(
                                "Spectator {} fully upgraded to player {} ({})",
                                recv_spectator_id, character_name, player_id
                            );

                            // --- Phase 2: Normal player message handling loop ---
                            let mut last_app_msg = std::time::Instant::now();
                            loop {
                                tokio::select! {
                                    biased;
                                    // Check if send_task died (connection broken)
                                    _ = shutdown_rx.changed() => {
                                        warn!("Send task died for upgraded player {}, proceeding to cleanup", player_id);
                                        break;
                                    }
                                    result = tokio::time::timeout(Duration::from_secs(15), receiver.next()) => {
                                        match result {
                                            Ok(Some(Ok(msg))) => match msg {
                                                Message::Binary(data) => {
                                                    last_app_msg = std::time::Instant::now();
                                                    if let Err(e) = handle_client_message(
                                                        &recv_state,
                                                        &recv_room,
                                                        &player_id,
                                                        &data,
                                                    )
                                                    .await
                                                    {
                                                        warn!(
                                                            "Error handling message from upgraded player {}: {}",
                                                            player_id, e
                                                        );
                                                    }
                                                }
                                                Message::Close(_) => break,
                                                _ => {
                                                    // Pong or other control frame — don't reset app timer
                                                    if last_app_msg.elapsed() > Duration::from_secs(45) {
                                                        warn!("Upgraded player {} timed out (no app messages for 45s)", player_id);
                                                        break;
                                                    }
                                                }
                                            },
                                            Ok(Some(Err(_))) | Ok(None) => break,
                                            Err(_) => {
                                                // Short timeout expired, check app-level activity
                                                if last_app_msg.elapsed() > Duration::from_secs(45) {
                                                    warn!("Upgraded player {} connection timed out (no data for 45s)", player_id);
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // --- Phase 3: Cleanup (player disconnected) ---
                            // Atomically claim session ownership first.
                            // If another handler already took over, skip cleanup.
                            let removed_session = recv_state.sessions.remove(&session_id);
                            if removed_session.is_none() {
                                info!(
                                    "Session {} for {} was superseded by takeover, skipping cleanup",
                                    session_id, character_name
                                );
                                return true;
                            }
                            let (_, removed_sess) = removed_session.unwrap();
                            let character_id = removed_sess.character_id;
                            let should_save = recv_state.auth_sessions.contains_key(&removed_sess.auth_token);

                            info!(
                                "Upgraded player {} ({}) disconnected",
                                character_name, player_id
                            );

                            if should_save {
                                // If player is in a KOTH instance, move them back to
                                // overworld before saving so they don't respawn inside it
                                {
                                    let instance_id = recv_state
                                        .player_instances
                                        .read()
                                        .await
                                        .get(&player_id)
                                        .cloned();
                                    if let Some(ref inst_id) = instance_id {
                                        if let Some((ex, ey)) =
                                            recv_room.get_koth_entrance(inst_id).await
                                        {
                                            // Reset player to overworld position before save
                                            recv_room
                                                .set_player_position_and_z(&player_id, ex, ey, 0)
                                                .await;
                                            // Remove from instance tracking so save doesn't
                                            // record the KOTH map as current_map
                                            recv_state
                                                .player_instances
                                                .write()
                                                .await
                                                .remove(&player_id);
                                        }
                                    }
                                }

                                // Compute played time delta
                                let played_time_delta = recv_state
                                    .play_time_anchors
                                    .remove(&character_id)
                                    .map(|(_, anchor)| anchor.elapsed().as_secs() as i64)
                                    .unwrap_or(0);

                                // Save character state
                                if let Some(mut save_data) =
                                    recv_room.get_player_save_data(&player_id).await
                                {
                                    if save_data.current_map.is_some() {
                                        let entrance_positions =
                                            recv_state.player_entrance_positions.read().await;
                                        if let Some(&(ex, ey)) = entrance_positions.get(&player_id)
                                        {
                                            save_data.entrance_x = Some(ex as f32);
                                            save_data.entrance_y = Some(ey as f32);
                                        }
                                    }
                                    if let Err(e) = recv_state
                                        .db
                                        .save_character(
                                            character_id,
                                            save_data.x,
                                            save_data.y,
                                            save_data.z,
                                            save_data.hp,
                                            save_data.prayer_points,
                                            save_data.mp,
                                            &save_data.skills,
                                            save_data.gold,
                                            &save_data.inventory_json,
                                            save_data.equipped_head.as_deref(),
                                            save_data.equipped_body.as_deref(),
                                            save_data.equipped_weapon.as_deref(),
                                            save_data.equipped_back.as_deref(),
                                            save_data.equipped_feet.as_deref(),
                                            save_data.equipped_ring.as_deref(),
                                            save_data.equipped_gloves.as_deref(),
                                            save_data.equipped_necklace.as_deref(),
                                            save_data.equipped_belt.as_deref(),
                                            played_time_delta,
                                            save_data.current_map.as_deref(),
                                            save_data.sitting_at_x,
                                            save_data.sitting_at_y,
                                            save_data.entrance_x,
                                            save_data.entrance_y,
                                            &save_data.bank_json,
                                            save_data.bank_gold,
                                            save_data.bank_max_slots,
                                            &save_data.combat_style_prefs,
                                        )
                                        .await
                                    {
                                        error!(
                                            "Failed to save character {} on disconnect: {}",
                                            character_name, e
                                        );
                                    } else {
                                        info!(
                                            "Saved character {} to database on disconnect (played_time +{}s)",
                                            character_name, played_time_delta
                                        );
                                    }
                                }

                                // Save quest state
                                if character_id > 0 {
                                    if let Some(quest_state) =
                                        recv_room.get_player_quest_state(&player_id).await
                                    {
                                        if let Err(e) = recv_state
                                            .db
                                            .save_character_quest_state(character_id, &quest_state)
                                            .await
                                        {
                                            error!(
                                                "Failed to save quest state for {} on disconnect: {}",
                                                character_name, e
                                            );
                                        } else if !quest_state.active_quests.is_empty()
                                            || !quest_state.completed_quests.is_empty()
                                        {
                                            info!(
                                                "Saved quest state for {}: {} active, {} completed",
                                                character_name,
                                                quest_state.active_quests.len(),
                                                quest_state.completed_quests.len()
                                            );
                                        }
                                    }
                                }

                                // Save discovered recipes
                                if character_id > 0 {
                                    let discovered =
                                        recv_room.get_player_discovered_recipes(&player_id).await;
                                    for recipe_id in &discovered {
                                        if let Err(e) = recv_state
                                            .db
                                            .save_discovered_recipe(character_id, recipe_id)
                                            .await
                                        {
                                            error!(
                                                "Failed to save discovered recipe {} for {}: {}",
                                                recipe_id, character_name, e
                                            );
                                        }
                                    }
                                    if !discovered.is_empty() {
                                        info!(
                                            "Saved {} discovered recipes for {}",
                                            discovered.len(),
                                            character_name
                                        );
                                    }
                                }

                                // Save unlocked spells
                                if character_id > 0 {
                                    let unlocked =
                                        recv_room.get_player_unlocked_spells(&player_id).await;
                                    for spell_id in &unlocked {
                                        if let Err(e) = recv_state
                                            .db
                                            .save_unlocked_spell(character_id, spell_id)
                                            .await
                                        {
                                            error!(
                                                "Failed to save unlocked spell {} for {}: {}",
                                                spell_id, character_name, e
                                            );
                                        }
                                    }
                                }

                                // Save slayer state
                                if character_id > 0 {
                                    let slayer_state =
                                        recv_room.get_player_slayer_state(&player_id).await;
                                    if slayer_state.current_task.is_some()
                                        || slayer_state.tasks_completed > 0
                                        || slayer_state.points > 0
                                    {
                                        if let Err(e) = recv_state
                                            .db
                                            .save_character_slayer_state(
                                                character_id,
                                                &slayer_state,
                                            )
                                            .await
                                        {
                                            error!(
                                                "Failed to save slayer state for {} on disconnect: {}",
                                                character_name, e
                                            );
                                        } else {
                                            info!(
                                                "Saved slayer state for {}: {} tasks completed, {} points",
                                                character_name,
                                                slayer_state.tasks_completed,
                                                slayer_state.points
                                            );
                                        }
                                    }
                                }
                            } else {
                                warn!(
                                    "Skipping save for {} on disconnect: invalid auth",
                                    character_name
                                );
                            }

                            // Clean up instance tracking
                            {
                                use crate::interior::InstanceType;

                                let removed_instance_id =
                                    recv_state.player_instances.write().await.remove(&player_id);
                                recv_room.reset_sync_state(&player_id).await;
                                if let Some(instance_id) = removed_instance_id {
                                    // Clean up KOTH session if player was in one
                                    recv_room.cleanup_koth_session(&instance_id).await;
                                    if let Some(instance) =
                                        recv_state.instance_manager.get_by_instance_id(&instance_id)
                                    {
                                        let other_players: Vec<String> = instance
                                            .get_player_ids()
                                            .await
                                            .into_iter()
                                            .filter(|id| id != &player_id)
                                            .collect();

                                        let remaining = instance.remove_player(&player_id).await;

                                        for other_id in &other_players {
                                            recv_room
                                                .send_to_player(
                                                    other_id,
                                                    ServerMessage::PlayerLeft {
                                                        id: player_id.to_string(),
                                                    },
                                                )
                                                .await;
                                        }

                                        if remaining == 0
                                            && instance.instance_type == InstanceType::Private
                                        {
                                            if let Some(owner_id) = &instance.owner_id {
                                                recv_state
                                                    .instance_manager
                                                    .remove_private(owner_id, &instance.map_id);
                                            }
                                        }
                                    }
                                }
                            }

                            // Clean up entrance position tracking
                            recv_state
                                .player_entrance_positions
                                .write()
                                .await
                                .remove(&player_id);

                            // Unregister player sender
                            recv_room.unregister_player_sender(&player_id).await;

                            // Notify friends that this player went offline
                            recv_room.broadcast_friend_status(&player_id, false).await;

                            // Mark character as offline
                            recv_state.online_characters.remove(&character_id);

                            recv_room.remove_player(&player_id).await;

                            // Notify overworld players that this player left
                            recv_room
                                .send_to_overworld_players(
                                    ServerMessage::PlayerLeft {
                                        id: player_id.clone(),
                                    },
                                    None,
                                )
                                .await;

                            // Return true to indicate upgrade happened (spectator already removed)
                            return true;
                        }
                        Ok(_) => {
                            // Ignore all other messages from spectators
                        }
                        Err(_) => {
                            // Ignore decode errors
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        // Spectator disconnected without upgrading
        false
    });

    // Wait for either task to finish
    tokio::select! {
        _ = &mut send_task => {
            // Send task died (connection broken) — signal recv_task to stop
            // so it can run Phase 3 cleanup instead of being aborted
            let _ = shutdown_tx.send(true);
            // Wait for recv_task to finish cleanup (with safety timeout)
            match tokio::time::timeout(Duration::from_secs(10), recv_task).await {
                Ok(Ok(true)) => {
                    // recv_task completed and handled player cleanup
                    return;
                }
                _ => {
                    // recv_task didn't complete or wasn't upgraded — clean up spectator
                    room.remove_spectator(&spectator_id).await;
                    info!("Spectator {} disconnected (send task ended)", spectator_id);
                }
            }
        }
        result = &mut recv_task => {
            send_task.abort();
            // If recv_task completed with Ok(true), the player cleanup was already handled
            // inside the task. Only need to remove spectator if NOT upgraded.
            let upgraded = result.unwrap_or(false);
            if !upgraded {
                room.remove_spectator(&spectator_id).await;
                info!("Spectator {} disconnected", spectator_id);
            }
        }
    }
}

// ============================================================================
// Authenticated WebSocket Handler
// ============================================================================

async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(room_id): Path<String>,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Validate signed session token
    let session_id = match state.token_signer.validate_token(&query.session_token) {
        Some((sid, rid)) => {
            if rid != room_id {
                warn!(
                    "WebSocket rejected: Token room_id mismatch ({} != {})",
                    rid, room_id
                );
                return (
                    StatusCode::FORBIDDEN,
                    "Invalid session token: room mismatch",
                )
                    .into_response();
            }
            sid
        }
        None => {
            warn!("WebSocket rejected: Invalid or expired session token");
            return (StatusCode::UNAUTHORIZED, "Invalid or expired session token").into_response();
        }
    };

    // Validate session exists in our store
    let session_data = state.sessions.get(&session_id).map(|s| s.clone());

    match session_data {
        Some(session) if session.room_id == room_id => {
            // Verify the auth token is still valid
            if !state.auth_sessions.contains_key(&session.auth_token) {
                warn!(
                    "WebSocket rejected: Auth token expired for session {}",
                    session_id
                );
                return (
                    StatusCode::UNAUTHORIZED,
                    "Auth token expired. Please login again.",
                )
                    .into_response();
            }

            // Valid session, upgrade to WebSocket
            let player_id = session.player_id.clone();
            let character_name = session.character_name.clone();
            let character_id = session.character_id;
            let current_map = session.current_map.clone();
            let entrance_x = session.entrance_x;
            let entrance_y = session.entrance_y;
            let is_new_character = session.is_new_character;
            ws.on_upgrade(move |socket| {
                handle_socket(
                    socket,
                    state,
                    room_id,
                    player_id,
                    session_id,
                    character_name,
                    character_id,
                    current_map,
                    entrance_x,
                    entrance_y,
                    is_new_character,
                )
            })
        }
        _ => {
            warn!("Invalid session: {} for room {}", session_id, room_id);
            (StatusCode::FORBIDDEN, "Invalid session").into_response()
        }
    }
}

async fn handle_socket(
    socket: WebSocket,
    state: AppState,
    room_id: String,
    player_id: String,
    session_id: String,
    character_name: String,
    character_id: i64,
    current_map: Option<String>, // Interior map to auto-enter on reconnect
    entrance_x: Option<f32>,     // Overworld entrance X (for interior exit)
    entrance_y: Option<f32>,     // Overworld entrance Y (for interior exit)
    is_new_character: bool,      // True if played_time == 0 (for tutorial)
) {
    let (mut sender, mut receiver) = socket.split();

    // Get the room
    let room = match state.rooms.get(&room_id) {
        Some(r) => r.clone(),
        None => {
            error!("Room not found: {}", room_id);
            return;
        }
    };

    // Mark character as online now that WebSocket is actually connected
    state.online_characters.insert(character_id);

    // Activate the player
    let player_name = room.activate_player(&player_id).await;
    info!(
        "Player {} ({}) connected to room {}",
        player_name, player_id, room_id
    );

    // Subscribe to room broadcasts
    let mut broadcast_rx = room.subscribe();

    // Send welcome message
    let welcome = ServerMessage::Welcome {
        player_id: player_id.clone(),
        is_new_character,
    };
    if let Ok(bytes) = protocol::encode_server_message(&welcome) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send entity definitions
    let entity_defs = room.get_entity_definitions();
    if let Ok(bytes) = protocol::encode_server_message(&entity_defs) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send item definitions
    let item_defs = state.item_registry.to_client_definitions();
    if let Ok(bytes) = protocol::encode_server_message(&item_defs) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send recipe definitions
    let recipe_defs = state.crafting_registry.to_client_definitions();
    if let Ok(bytes) = protocol::encode_server_message(&recipe_defs) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send discovered recipes
    let discovered = room.get_player_discovered_recipes(&player_id).await;
    let discovered_msg = ServerMessage::DiscoveredRecipes {
        recipes: discovered.into_iter().collect(),
    };
    if let Ok(bytes) = protocol::encode_server_message(&discovered_msg) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send scroll spell definitions
    let scroll_spell_defs_msg = room.get_scroll_spell_definitions_message();
    if let Ok(bytes) = protocol::encode_server_message(&scroll_spell_defs_msg) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send unlocked spells
    let unlocked = room.get_player_unlocked_spells(&player_id).await;
    let unlocked_msg = ServerMessage::UnlockedSpellsSync {
        spell_ids: unlocked.into_iter().collect(),
    };
    if let Ok(bytes) = protocol::encode_server_message(&unlocked_msg) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send gathering marker positions
    let gathering_markers = room.get_gathering_markers_message(None).await;
    if let Ok(bytes) = protocol::encode_server_message(&gathering_markers) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send farming patch states (per-player instanced)
    let farming_patches = room.get_farming_patches_message(&player_id).await;
    if let Ok(bytes) = protocol::encode_server_message(&farming_patches) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send farming contract state
    let contract_msg = room.get_farming_contract_message(&player_id).await;
    if let Ok(bytes) = protocol::encode_server_message(&contract_msg) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send chair positions
    let chair_positions = room.get_chair_positions_message().await;
    if let Ok(bytes) = protocol::encode_server_message(&chair_positions) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send overworld chest positions
    let chest_positions = room.get_chest_positions_message(None).await;
    if let Ok(bytes) = protocol::encode_server_message(&chest_positions) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send prayer state
    if let Some(prayer_state) = room.get_player_prayer_state(&player_id).await {
        if let Ok(bytes) = protocol::encode_server_message(&prayer_state) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Only send overworld data if the player is NOT reconnecting into an instance
    let reconnecting_to_instance = current_map.is_some();

    if !reconnecting_to_instance {
        // Get player's position and send nearby chunks
        if let Some((px, py)) = room.get_player_position(&player_id).await {
            let player_chunk = chunk::ChunkCoord::from_world(px, py);

            // Preload and send chunks in a 3x3 area around the player
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let coord = chunk::ChunkCoord::new(player_chunk.x + dx, player_chunk.y + dy);
                    if let Some(chunk_msg) = room.handle_chunk_request(coord.x, coord.y).await {
                        if let Ok(bytes) = protocol::encode_server_message(&chunk_msg) {
                            let _ = sender.send(Message::Binary(bytes)).await;
                        }
                    }
                }
            }
        }

        // Send existing players to this client (only overworld players, not those in instances)
        {
            let instanced_players = state.player_instances.read().await;
            for existing_player in room.get_all_players().await {
                if existing_player.id != player_id
                    && !instanced_players.contains_key(&existing_player.id)
                {
                    let msg = ServerMessage::PlayerJoined {
                        id: existing_player.id.clone(),
                        name: existing_player.name.clone(),
                        x: existing_player.x,
                        y: existing_player.y,
                        gender: existing_player.gender.clone(),
                        skin: existing_player.skin.clone(),
                        hair_style: existing_player.hair_style,
                        hair_color: existing_player.hair_color,
                    };
                    if let Ok(bytes) = protocol::encode_server_message(&msg) {
                        let _ = sender.send(Message::Binary(bytes)).await;
                    }
                }
            }
        }

        // Send existing overworld ground items to this client
        let ground_items = room.get_ground_items_in_instance(None).await;
        for item_msg in ground_items {
            if let Ok(bytes) = protocol::encode_server_message(&item_msg) {
                let _ = sender.send(Message::Binary(bytes)).await;
            }
        }
    }

    // Notify others about this player joining
    // Instance players will ignore this via state sync filtering
    let (x, y) = room.get_player_position(&player_id).await.unwrap_or((0, 0));
    let (gender, skin) = room
        .get_player_appearance(&player_id)
        .await
        .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
    let (hair_style, hair_color) = room
        .get_player_hair(&player_id)
        .await
        .unwrap_or((None, None));
    let player_joined_msg = ServerMessage::PlayerJoined {
        id: player_id.clone(),
        name: player_name.clone(),
        x,
        y,
        gender: gender.clone(),
        skin: skin.clone(),
        hair_style,
        hair_color,
    };

    // Send PlayerJoined directly to this client first (so player exists before skills sync)
    if let Ok(bytes) = protocol::encode_server_message(&player_joined_msg) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Notify other overworld players (exclude self to avoid double-receive which overwrites skills)
    room.send_to_overworld_players(player_joined_msg, Some(&player_id))
        .await;

    // If player was sitting on a chair, send SitResult so client shows sitting animation
    if let Some((sx, sy, direction)) = room.get_player_sitting_info(&player_id).await {
        let sit_msg = ServerMessage::SitResult {
            success: true,
            tile_x: sx,
            tile_y: sy,
            direction,
        };
        if let Ok(bytes) = protocol::encode_server_message(&sit_msg) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Bring skill/gold milestone quest objectives up to date before sending quest state
    room.process_quest_progression_snapshot(&player_id).await;

    // Send active quests to this client (from saved state)
    for quest_msg in room.get_active_quest_messages(&player_id).await {
        if let Ok(bytes) = protocol::encode_server_message(&quest_msg) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Send completed quest ids so client can show correct tier lock/completion states after relog
    let quest_state_sync = room.get_completed_quest_sync_message(&player_id).await;
    if let Ok(bytes) = protocol::encode_server_message(&quest_state_sync) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send full quest catalog for the quest panel
    let quest_catalog = room.build_quest_catalog().await;
    if let Ok(bytes) = protocol::encode_server_message(&quest_catalog) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send initial inventory to this client
    if let Some(inv_msg) = room.get_player_inventory_update(&player_id).await {
        if let Ok(bytes) = protocol::encode_server_message(&inv_msg) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Send initial skills to this client
    if let Some(skills_msg) = room.get_player_skills_sync(&player_id).await {
        if let Ok(bytes) = protocol::encode_server_message(&skills_msg) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Send active potion buffs
    if let Some(buffs_msg) = room.get_player_potion_buffs_sync(&player_id).await {
        if let Ok(bytes) = protocol::encode_server_message(&buffs_msg) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Send current top total level player (for trophy icon) — refresh from DB and broadcast to all
    room.init_top_level_player().await;
    {
        let top_msg = room.get_top_player_message().await;
        room.broadcast(top_msg.clone()).await;
        if let Ok(bytes) = protocol::encode_server_message(&top_msg) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Send slayer state sync to this client
    {
        let slayer_state = room.get_player_slayer_state(&player_id).await;
        let slayer_task_data =
            slayer_state
                .current_task
                .as_ref()
                .map(|t| crate::protocol::SlayerTaskData {
                    monster_id: t.monster_id.clone(),
                    display_name: t.display_name.clone(),
                    kills_current: t.kills_current,
                    kills_required: t.kills_required,
                    xp_per_kill: t.xp_per_kill,
                    master_id: t.master_id.clone(),
                    points_on_complete: t.points_on_complete,
                });
        let slayer_sync = ServerMessage::SlayerStateSync {
            current_task: slayer_task_data,
            points: slayer_state.points,
            tasks_completed: slayer_state.tasks_completed,
            blocked_monsters: slayer_state.blocked_monsters.clone(),
            unlocked_monsters: slayer_state.unlocked_monsters.clone(),
        };
        if let Ok(bytes) = protocol::encode_server_message(&slayer_sync) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Create channel for sending messages to this client
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(512);

    // SECURITY: Register this player's sender for unicast messages
    room.register_player_sender(&player_id, tx).await;

    // Send friends list and pending requests (must be after sender is registered)
    room.send_friends_data(&player_id, &state.online_characters)
        .await;

    // Notify friends that this player came online
    room.broadcast_friend_status(&player_id, true).await;

    // If player was in an instance when they disconnected, auto-re-enter it
    if let Some(ref map_id) = current_map {
        info!(
            "Auto-re-entering instance '{}' for reconnecting player {}",
            map_id, player_id
        );
        auto_enter_instance(&state, &room, &player_id, map_id, entrance_x, entrance_y).await;
    }

    // Spawn task to forward messages to WebSocket
    let send_player_id = player_id.clone();
    let send_perf = state.perf_metrics.clone();
    let mut send_task = tokio::spawn(async move {
        let mut ping_interval = tokio::time::interval(Duration::from_secs(15));
        ping_interval.tick().await; // consume immediate first tick
        loop {
            tokio::select! {
                // Bias toward unicast (StateSync) over broadcasts to prevent
                // broadcast floods from starving position updates
                biased;

                // Handle direct messages to this client (StateSync, etc.)
                Some(msg) = rx.recv() => {
                    let send_start = std::time::Instant::now();
                    let msg_len = msg.len();
                    if sender.send(Message::Binary(msg)).await.is_err() {
                        break;
                    }
                    let send_ms = send_start.elapsed().as_secs_f64() * 1000.0;
                    send_perf.record_ws_send("unicast", send_ms, msg_len);
                    if send_ms > 50.0 {
                        tracing::warn!("Slow WS send (unicast): {:.2}ms, {}B for {}", send_ms, msg_len, send_player_id);
                    }
                }
                // Handle broadcast messages (pre-encoded bytes)
                result = broadcast_rx.recv() => {
                    match result {
                        Ok(bytes) => {
                            let send_start = std::time::Instant::now();
                            let msg_len = bytes.len();
                            if sender.send(Message::Binary(bytes)).await.is_err() {
                                break;
                            }
                            let send_ms = send_start.elapsed().as_secs_f64() * 1000.0;
                            send_perf.record_ws_send("broadcast", send_ms, msg_len);
                            if send_ms > 50.0 {
                                tracing::warn!("Slow WS send (broadcast): {:.2}ms, {}B for {}", send_ms, msg_len, send_player_id);
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("Broadcast lagged for {}: skipped {} messages", send_player_id, n);
                            // Continue - receiver position was auto-advanced
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                // Server-side WebSocket ping to keep connection alive (browsers auto-pong)
                _ = ping_interval.tick() => {
                    if sender.send(Message::Ping(vec![])).await.is_err() {
                        break;
                    }
                }
                else => break,
            }
        }
    });

    // Handle incoming messages
    let room_clone = room.clone();
    let player_id_clone = player_id.clone();
    let state_clone = state.clone();
    let mut recv_task = tokio::spawn(async move {
        let mut last_app_msg = std::time::Instant::now();
        loop {
            match tokio::time::timeout(Duration::from_secs(15), receiver.next()).await {
                Ok(Some(Ok(msg))) => match msg {
                    Message::Binary(data) => {
                        last_app_msg = std::time::Instant::now();
                        if let Err(e) =
                            handle_client_message(&state_clone, &room_clone, &player_id_clone, &data)
                                .await
                        {
                            warn!("Error handling message: {}", e);
                        }
                    }
                    Message::Close(_) => break,
                    _ => {
                        // Pong or other control frame — don't reset app timer
                        if last_app_msg.elapsed() > Duration::from_secs(45) {
                            warn!("Player {} timed out (no app messages for 45s)", player_id_clone);
                            break;
                        }
                    }
                },
                Ok(Some(Err(_))) | Ok(None) => break,
                Err(_) => {
                    // Short timeout expired, check app-level activity
                    if last_app_msg.elapsed() > Duration::from_secs(45) {
                        warn!("Player {} connection timed out (no data for 45s)", player_id_clone);
                        break;
                    }
                }
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    // Cleanup — atomically claim session ownership first.
    // If another handler already took over this session, skip all cleanup.
    let removed_session = state.sessions.remove(&session_id);
    if removed_session.is_none() {
        info!(
            "Session {} for {} was superseded by takeover, skipping cleanup",
            session_id, character_name
        );
        return;
    }
    let (_, removed_sess) = removed_session.unwrap();
    let character_id = removed_sess.character_id;
    let should_save = state.auth_sessions.contains_key(&removed_sess.auth_token);

    info!(
        "Character {} disconnected from room {}",
        character_name, room_id
    );

    if should_save {
        // Compute played time delta from anchor
        let played_time_delta = state
            .play_time_anchors
            .remove(&character_id)
            .map(|(_, anchor)| anchor.elapsed().as_secs() as i64)
            .unwrap_or(0);

        // Save character state to database
        if let Some(mut save_data) = room.get_player_save_data(&player_id).await {
            // Populate entrance position from runtime HashMap
            if save_data.current_map.is_some() {
                let entrance_positions = state.player_entrance_positions.read().await;
                if let Some(&(ex, ey)) = entrance_positions.get(&player_id) {
                    save_data.entrance_x = Some(ex as f32);
                    save_data.entrance_y = Some(ey as f32);
                }
            }
            if let Err(e) = state
                .db
                .save_character(
                    character_id,
                    save_data.x,
                    save_data.y,
                    save_data.z,
                    save_data.hp,
                    save_data.prayer_points,
                    save_data.mp,
                    &save_data.skills,
                    save_data.gold,
                    &save_data.inventory_json,
                    save_data.equipped_head.as_deref(),
                    save_data.equipped_body.as_deref(),
                    save_data.equipped_weapon.as_deref(),
                    save_data.equipped_back.as_deref(),
                    save_data.equipped_feet.as_deref(),
                    save_data.equipped_ring.as_deref(),
                    save_data.equipped_gloves.as_deref(),
                    save_data.equipped_necklace.as_deref(),
                    save_data.equipped_belt.as_deref(),
                    played_time_delta,
                    save_data.current_map.as_deref(),
                    save_data.sitting_at_x,
                    save_data.sitting_at_y,
                    save_data.entrance_x,
                    save_data.entrance_y,
                    &save_data.bank_json,
                    save_data.bank_gold,
                    save_data.bank_max_slots,
                    &save_data.combat_style_prefs,
                )
                .await
            {
                error!(
                    "Failed to save character {} on disconnect: {}",
                    character_name, e
                );
            } else {
                info!(
                    "Saved character {} to database on disconnect (played_time +{}s)",
                    character_name, played_time_delta
                );
            }
        }

        // Save quest state to database
        if character_id > 0 {
            if let Some(quest_state) = room.get_player_quest_state(&player_id).await {
                if let Err(e) = state
                    .db
                    .save_character_quest_state(character_id, &quest_state)
                    .await
                {
                    error!(
                        "Failed to save quest state for {} on disconnect: {}",
                        character_name, e
                    );
                } else if !quest_state.active_quests.is_empty()
                    || !quest_state.completed_quests.is_empty()
                {
                    info!(
                        "Saved quest state for {}: {} active, {} completed",
                        character_name,
                        quest_state.active_quests.len(),
                        quest_state.completed_quests.len()
                    );
                }
            }
        }

        // Save discovered recipes to database
        if character_id > 0 {
            let discovered = room.get_player_discovered_recipes(&player_id).await;
            for recipe_id in &discovered {
                if let Err(e) = state
                    .db
                    .save_discovered_recipe(character_id, recipe_id)
                    .await
                {
                    error!(
                        "Failed to save discovered recipe {} for {}: {}",
                        recipe_id, character_name, e
                    );
                }
            }
            if !discovered.is_empty() {
                info!(
                    "Saved {} discovered recipes for {}",
                    discovered.len(),
                    character_name
                );
            }
        }

        // Save unlocked spells to database
        if character_id > 0 {
            let unlocked = room.get_player_unlocked_spells(&player_id).await;
            for spell_id in &unlocked {
                if let Err(e) = state.db.save_unlocked_spell(character_id, spell_id).await {
                    error!(
                        "Failed to save unlocked spell {} for {}: {}",
                        spell_id, character_name, e
                    );
                }
            }
        }

        // Save slayer state to database
        if character_id > 0 {
            let slayer_state = room.get_player_slayer_state(&player_id).await;
            if slayer_state.current_task.is_some()
                || slayer_state.tasks_completed > 0
                || slayer_state.points > 0
            {
                if let Err(e) = state
                    .db
                    .save_character_slayer_state(character_id, &slayer_state)
                    .await
                {
                    error!(
                        "Failed to save slayer state for {} on disconnect: {}",
                        character_name, e
                    );
                } else {
                    info!(
                        "Saved slayer state for {}: {} tasks completed, {} points",
                        character_name, slayer_state.tasks_completed, slayer_state.points
                    );
                }
            }
        }
    } else {
        warn!(
            "Skipping save for {} on disconnect: invalid auth",
            character_name
        );
    }

    // Clean up instance tracking when player disconnects
    // IMPORTANT: We must notify instance peers BEFORE unregistering the sender,
    // and use the instance_id directly (not find_player_instance which scans Instance.players
    // and could race with other operations).
    {
        use crate::interior::InstanceType;

        let removed_instance_id = state.player_instances.write().await.remove(&player_id);
        room.reset_sync_state(&player_id).await;
        if let Some(instance_id) = removed_instance_id {
            // Use get_by_instance_id (direct lookup) instead of find_player_instance (scan)
            if let Some(instance) = state.instance_manager.get_by_instance_id(&instance_id) {
                // Get other players BEFORE removing, so we can notify them
                let other_players: Vec<String> = instance
                    .get_player_ids()
                    .await
                    .into_iter()
                    .filter(|id| id != &player_id)
                    .collect();

                let remaining = instance.remove_player(&player_id).await;

                // Notify instance peers that this player left
                for other_id in &other_players {
                    room.send_to_player(
                        other_id,
                        ServerMessage::PlayerLeft {
                            id: player_id.to_string(),
                        },
                    )
                    .await;
                }

                if remaining == 0 && instance.instance_type == InstanceType::Private {
                    if let Some(owner_id) = &instance.owner_id {
                        state
                            .instance_manager
                            .remove_private(owner_id, &instance.map_id);
                    }
                }
            }
        }
    }

    // Clean up entrance position tracking
    state
        .player_entrance_positions
        .write()
        .await
        .remove(&player_id);

    // SECURITY: Unregister player sender before cleanup
    room.unregister_player_sender(&player_id).await;

    // Notify friends that this player went offline
    room.broadcast_friend_status(&player_id, false).await;

    // Mark character as offline
    state.online_characters.remove(&character_id);

    room.remove_player(&player_id).await;

    // Notify overworld players that this player left.
    // If they were in an instance, instance peers were already notified above.
    // Overworld players still need this in case a stale sprite lingers from
    // a missed enter-instance PlayerLeft.
    room.send_to_overworld_players(
        ServerMessage::PlayerLeft {
            id: player_id.clone(),
        },
        None,
    )
    .await;
}

/// Auto-enter an instance on reconnect (when current_map was saved in DB)
async fn auto_enter_instance(
    state: &AppState,
    room: &GameRoom,
    player_id: &str,
    map_id: &str,
    entrance_x: Option<f32>,
    entrance_y: Option<f32>,
) {
    use crate::interior::InstanceType;
    use crate::protocol::{ChunkLayerData, ChunkPortalData};
    use base64::Engine;

    let interior = match state.interior_registry.get(map_id) {
        Some(i) => i,
        None => {
            warn!(
                "Auto-enter: unknown interior '{}' for player {}, staying in overworld",
                map_id, player_id
            );
            return;
        }
    };

    // Use the default spawn point
    let spawn = match interior.spawn_points.values().next() {
        Some(s) => s.clone(),
        None => {
            warn!("Auto-enter: interior '{}' has no spawn points", map_id);
            return;
        }
    };

    // Get or create instance
    let (instance, is_new) = match interior.instance_type {
        InstanceType::Public => state.instance_manager.get_or_create_public(
            &interior.id,
            interior.size.width,
            interior.size.height,
        ),
        InstanceType::Private => state.instance_manager.get_or_create_private(
            &interior.id,
            player_id,
            interior.size.width,
            interior.size.height,
        ),
    };

    if is_new || !*instance.npcs_spawned.read().await {
        // Load collision data for NPC walkability
        if !interior.collision.is_empty() {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&interior.collision)
            {
                instance.set_collision(&bytes).await;
            }
        }
        instance
            .spawn_npcs(&interior.entities, &state.entity_registry)
            .await;

        // Register gathering markers for this instance
        if !interior.gathering_zones.is_empty() {
            let markers: Vec<crate::gathering::GatheringMarker> = interior
                .gathering_zones
                .iter()
                .map(|gz| crate::gathering::GatheringMarker {
                    x: gz.x,
                    y: gz.y,
                    zone_id: gz.zone_id.clone(),
                })
                .collect();
            room.register_instance_gathering_markers(&instance.id, markers).await;
        }
    }

    // Restore entrance position from DB (for use when exiting the interior)
    if let (Some(ex), Some(ey)) = (entrance_x, entrance_y) {
        let mut entrance_positions = state.player_entrance_positions.write().await;
        entrance_positions.insert(player_id.to_string(), (ex as i32, ey as i32));
    }

    // Track player's instance
    {
        let mut player_instances = state.player_instances.write().await;
        player_instances.insert(player_id.to_string(), instance.id.clone());
    }
    room.reset_sync_state(player_id).await;

    // Notify overworld players that this player has "left"
    room.send_to_overworld_players(
        ServerMessage::PlayerLeft {
            id: player_id.to_string(),
        },
        Some(player_id),
    )
    .await;

    // Get other players already in the instance BEFORE adding
    let other_players_in_instance: Vec<String> = instance.get_player_ids().await;

    instance.add_player(player_id).await;
    // Player position is already correct from DB — don't override with spawn point
    let (player_x, player_y) = room
        .get_player_position(player_id)
        .await
        .unwrap_or((spawn.x as i32, spawn.y as i32));

    // Notify instance players
    if !other_players_in_instance.is_empty() {
        let player_name = room.get_player_name(player_id).await.unwrap_or_default();
        let (gender, skin) = room
            .get_player_appearance(player_id)
            .await
            .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
        let (hair_style, hair_color) = room
            .get_player_hair(player_id)
            .await
            .unwrap_or((None, None));

        for other_id in &other_players_in_instance {
            room.send_to_player(
                other_id,
                ServerMessage::PlayerJoined {
                    id: player_id.to_string(),
                    name: player_name.clone(),
                    x: player_x,
                    y: player_y,
                    gender: gender.clone(),
                    skin: skin.clone(),
                    hair_style,
                    hair_color,
                },
            )
            .await;
        }

        for other_id in &other_players_in_instance {
            if let Some(other_name) = room.get_player_name(other_id).await {
                let (other_x, other_y) = room
                    .get_player_position(other_id)
                    .await
                    .unwrap_or((spawn.x as i32, spawn.y as i32));
                let (other_gender, other_skin) = room
                    .get_player_appearance(other_id)
                    .await
                    .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
                let (other_hair_style, other_hair_color) =
                    room.get_player_hair(other_id).await.unwrap_or((None, None));

                room.send_to_player(
                    player_id,
                    ServerMessage::PlayerJoined {
                        id: other_id.clone(),
                        name: other_name,
                        x: other_x,
                        y: other_y,
                        gender: other_gender,
                        skin: other_skin,
                        hair_style: other_hair_style,
                        hair_color: other_hair_color,
                    },
                )
                .await;
            }
        }
    }

    // Send transition message
    room.send_to_player(
        player_id,
        ServerMessage::MapTransition {
            map_type: "interior".to_string(),
            map_id: interior.id.clone(),
            spawn_x: player_x as f32,
            spawn_y: player_y as f32,
            instance_id: instance.id.clone(),
        },
    )
    .await;

    // Send interior map data
    let layers = vec![
        ChunkLayerData {
            layer_type: 0,
            tiles: interior.layers.ground.clone(),
        },
        ChunkLayerData {
            layer_type: 1,
            tiles: interior.layers.objects.clone(),
        },
        ChunkLayerData {
            layer_type: 2,
            tiles: interior.layers.overhead.clone(),
        },
    ];

    let collision = if interior.collision.is_empty() {
        vec![]
    } else {
        base64::engine::general_purpose::STANDARD
            .decode(&interior.collision)
            .unwrap_or_default()
    };

    let portals: Vec<ChunkPortalData> = interior
        .portals
        .iter()
        .map(|p| ChunkPortalData {
            id: p.id.clone(),
            x: p.x,
            y: p.y,
            width: p.width,
            height: p.height,
            target_map: p.target_map.clone(),
            target_spawn: p.target_spawn.clone().unwrap_or_default(),
        })
        .collect();

    let objects: Vec<protocol::ChunkObjectData> = interior
        .map_objects
        .iter()
        .map(|o| protocol::ChunkObjectData {
            gid: o.gid,
            tile_x: o.x,
            tile_y: o.y,
            width: o.width,
            height: o.height,
        })
        .collect();

    let walls: Vec<protocol::ChunkWallData> = interior
        .walls
        .iter()
        .map(|w| protocol::ChunkWallData {
            gid: w.gid,
            tile_x: w.x,
            tile_y: w.y,
            edge: w.edge.clone(),
        })
        .collect();

    room.send_to_player(
        player_id,
        ServerMessage::InteriorData {
            map_id: interior.id.clone(),
            name: interior.name.clone(),
            instance_id: instance.id.clone(),
            width: interior.size.width,
            height: interior.size.height,
            spawn_x: spawn.x,
            spawn_y: spawn.y,
            layers,
            collision,
            portals,
            objects,
            walls,
            heightmap: interior.heightmap.clone(),
            block_types_down: interior.block_types_down.clone(),
            block_types_right: interior.block_types_right.clone(),
        },
    )
    .await;

    // Send NPC updates
    let npc_updates = instance.get_npc_updates().await;
    if !npc_updates.is_empty() {
        room.send_to_player(
            player_id,
            ServerMessage::StateSync {
                tick: 0,
                players: vec![],
                npcs: npc_updates,
                instance_id: instance.id.clone(),
            },
        )
        .await;
    }

    // Send gathering markers for this instance
    room.send_to_player(
        player_id,
        room.get_gathering_markers_message(Some(&instance.id)).await,
    )
    .await;

    // Send ground items
    let ground_items = room.get_ground_items_in_instance(Some(&instance.id)).await;
    for item_msg in ground_items {
        room.send_to_player(player_id, item_msg).await;
    }

    info!(
        "Auto-entered player {} into instance {} (map: {})",
        player_id, instance.id, map_id
    );
}

async fn handle_enter_portal(state: &AppState, room: &GameRoom, player_id: &str, portal_id: &str) {
    use crate::interior::InstanceType;
    use crate::protocol::{ChunkLayerData, ChunkPortalData};
    use base64::Engine;

    info!(
        "Player {} attempting to enter portal '{}'",
        player_id, portal_id
    );

    // Check if player is currently in an interior
    let current_instance_id = {
        let instances = state.player_instances.read().await;
        instances.get(player_id).cloned()
    };

    // If player is in an interior, handle exit portal
    if let Some(instance_id) = current_instance_id {
        info!(
            "Player {} is in instance '{}', checking for interior exit portal",
            player_id, instance_id
        );

        // Find the interior this instance belongs to
        let interior_id = instance_id
            .strip_prefix("pub_")
            .or_else(|| instance_id.split('_').nth(1))
            .unwrap_or(&instance_id);

        let interior = match state.interior_registry.get(interior_id) {
            Some(i) => i,
            None => {
                error!(
                    "Could not find interior definition for instance '{}'",
                    instance_id
                );
                return;
            }
        };

        // Get player position
        let (player_x, player_y) = match room.get_player_position(player_id).await {
            Some(pos) => pos,
            None => {
                warn!("Player {} not found in room", player_id);
                return;
            }
        };

        // Find the portal in the interior's portal list
        let exit_portal = interior.portals.iter().find(|p| {
            p.id == portal_id
                && player_x >= p.x
                && player_x < p.x + p.width
                && player_y >= p.y
                && player_y < p.y + p.height
        });

        match exit_portal {
            Some(portal) => {
                info!(
                    "Found exit portal '{}' targeting '{}' at ({}, {})",
                    portal.id, portal.target_map, portal.target_x, portal.target_y
                );

                // Compute overworld spawn and update position BEFORE removing
                // from instance tracking, so if the tick loop sees the player as
                // overworld, they're already at the correct spawn position
                // (prevents ghost on portal tile).
                let (spawn_x, spawn_y) = if portal.target_map == "overworld" {
                    let coords = if portal.target_x != 0.0 || portal.target_y != 0.0 {
                        // Portal has explicit exit coordinates - use them
                        info!(
                            "Using portal exit coordinates ({}, {}) for player {}",
                            portal.target_x, portal.target_y, player_id
                        );
                        // Clean up stored entrance since we're not using it
                        let mut entrance_positions = state.player_entrance_positions.write().await;
                        entrance_positions.remove(player_id);
                        (portal.target_x, portal.target_y)
                    } else {
                        // Fall back to stored entrance position
                        let mut entrance_positions = state.player_entrance_positions.write().await;
                        if let Some((x, y)) = entrance_positions.remove(player_id) {
                            info!(
                                "Using stored entrance position ({}, {}) for player {}",
                                x, y, player_id
                            );
                            (x as f32, y as f32)
                        } else {
                            // Default spawn if nothing specified
                            (0.0, 0.0)
                        }
                    };

                    info!(
                        "Player {} exiting to overworld at ({}, {})",
                        player_id, coords.0, coords.1
                    );

                    room.set_player_position(player_id, coords.0 as i32, coords.1 as i32)
                        .await;
                    coords
                } else {
                    (0.0, 0.0)
                };

                // Remove player from both tracking systems and notify others.
                // Use get_by_instance_id (direct lookup by known ID) instead of
                // find_player_instance (scan that races with concurrent removals).
                // Position is already updated (for overworld exits) so the tick loop
                // won't see the player at the old portal position.
                {
                    let mut instances = state.player_instances.write().await;
                    instances.remove(player_id);
                }
                // Clean up KOTH state if exiting a KOTH instance
                room.cleanup_koth_session(&instance_id).await;
                room.reset_sync_state(player_id).await;

                if let Some(instance) = state.instance_manager.get_by_instance_id(&instance_id) {
                    // Get other players in the instance BEFORE removing this player
                    let other_players: Vec<String> = instance
                        .get_player_ids()
                        .await
                        .into_iter()
                        .filter(|id| id != player_id)
                        .collect();

                    let remaining = instance.remove_player(player_id).await;
                    if remaining == 0 && instance.instance_type == InstanceType::Private {
                        if let Some(owner_id) = &instance.owner_id {
                            state
                                .instance_manager
                                .remove_private(owner_id, &instance.map_id);
                        }
                    }

                    // Notify other players in the instance that this player left
                    // AND notify the exiting player that those players "left" their view
                    for other_id in &other_players {
                        // Tell players still in instance that this player left
                        room.send_to_player(
                            other_id,
                            ServerMessage::PlayerLeft {
                                id: player_id.to_string(),
                            },
                        )
                        .await;

                        // Tell the exiting player that the instance players are gone from their view
                        room.send_to_player(
                            player_id,
                            ServerMessage::PlayerLeft {
                                id: other_id.clone(),
                            },
                        )
                        .await;
                    }
                }

                if portal.target_map == "overworld" {
                    // Preload chunks around the overworld spawn before transitioning
                    let spawn_chunk = chunk::ChunkCoord::from_world(
                        spawn_x.floor() as i32,
                        spawn_y.floor() as i32,
                    );
                    room.world()
                        .preload_chunks(spawn_chunk, game::SPAWN_PRELOAD_RADIUS)
                        .await;

                    // Send transition back to overworld
                    room.send_to_player(
                        player_id,
                        ServerMessage::MapTransition {
                            map_type: "overworld".to_string(),
                            map_id: "world_0".to_string(),
                            spawn_x,
                            spawn_y,
                            instance_id: String::new(),
                        },
                    )
                    .await;

                    // Re-send overworld data that was cleared on instance entry
                    room.send_to_player(player_id, room.get_chair_positions_message().await)
                        .await;
                    room.send_to_player(player_id, room.get_gathering_markers_message(None).await)
                        .await;
                    room.send_to_player(player_id, room.get_chest_positions_message(None).await)
                        .await;

                    // Send overworld ground items
                    for item_msg in room.get_ground_items_in_instance(None).await {
                        room.send_to_player(player_id, item_msg).await;
                    }

                    // Notify overworld players that this player has returned
                    {
                        let player_name = room.get_player_name(player_id).await.unwrap_or_default();
                        let (gender, skin) = room
                            .get_player_appearance(player_id)
                            .await
                            .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
                        let (hair_style, hair_color) = room
                            .get_player_hair(player_id)
                            .await
                            .unwrap_or((None, None));
                        room.send_to_overworld_players(
                            ServerMessage::PlayerJoined {
                                id: player_id.to_string(),
                                name: player_name,
                                x: spawn_x as i32,
                                y: spawn_y as i32,
                                gender,
                                skin,
                                hair_style,
                                hair_color,
                            },
                            Some(player_id),
                        )
                        .await;
                    }

                    return;
                } else {
                    // Portal leads to another interior - fall through to normal handling
                    // (would need to update portal struct to work with interior->interior)
                    info!(
                        "Portal leads to another interior '{}' - not yet supported",
                        portal.target_map
                    );
                    return;
                }
            }
            None => {
                warn!(
                    "Player {} tried to use portal '{}' but no matching exit portal found at ({}, {})",
                    player_id, portal_id, player_x, player_y
                );
                return;
            }
        }
    }

    // Player is in overworld - find portal in world chunks
    let portal = match room.find_portal_at_player(player_id).await {
        Some(p) => {
            info!(
                "Found portal at player position: id='{}', target_map='{}', target_spawn='{}'",
                p.id, p.target_map, p.target_spawn
            );
            if p.id == portal_id {
                p
            } else {
                warn!(
                    "Player {} tried to use portal '{}' but is standing on portal '{}'",
                    player_id, portal_id, p.id
                );
                return;
            }
        }
        None => {
            warn!(
                "Player {} tried to use portal '{}' but no portal found at position",
                player_id, portal_id
            );
            return;
        }
    };

    // Get interior definition
    info!("Looking up interior map '{}'", portal.target_map);
    let interior = match state.interior_registry.get(&portal.target_map) {
        Some(i) => {
            info!(
                "Found interior '{}' with {} spawn points",
                i.id,
                i.spawn_points.len()
            );
            i
        }
        None => {
            error!(
                "Portal '{}' references unknown interior '{}'. Available interiors: {:?}",
                portal_id,
                portal.target_map,
                state.interior_registry.list_ids()
            );
            return;
        }
    };

    // Check if this interior requires an active slayer task
    if interior.requires_slayer_task {
        let slayer_state = room.get_player_slayer_state(player_id).await;
        if slayer_state.current_task.is_none() {
            room.send_to_player(
                player_id,
                ServerMessage::ChatMessage {
                    sender_id: "system".to_string(),
                    sender_name: "[System]".to_string(),
                    text: "You need an active slayer task to enter this cave.".to_string(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    channel: "system".to_string(),
                },
            )
            .await;
            return;
        }
    }

    // Get spawn point - try exact name, then "entrance", then first available
    info!(
        "Looking up spawn point '{}' in interior '{}'",
        portal.target_spawn, interior.id
    );
    let spawn = if !portal.target_spawn.is_empty() {
        interior.get_spawn_point(&portal.target_spawn)
    } else {
        None
    };
    let spawn = match spawn
        .or_else(|| interior.get_spawn_point("entrance"))
        .or_else(|| interior.spawn_points.values().next())
    {
        Some(s) => {
            info!(
                "Using spawn point at ({}, {}) in interior '{}'",
                s.x, s.y, interior.id
            );
            s
        }
        None => {
            error!("Interior '{}' has no spawn points at all!", interior.id);
            return;
        }
    };

    // Get or create instance based on type
    let (instance, is_new) = match interior.instance_type {
        InstanceType::Public => state.instance_manager.get_or_create_public(
            &interior.id,
            interior.size.width,
            interior.size.height,
        ),
        InstanceType::Private => state.instance_manager.get_or_create_private(
            &interior.id,
            player_id,
            interior.size.width,
            interior.size.height,
        ),
    };

    // Spawn NPCs if this is a new instance
    if is_new || !*instance.npcs_spawned.read().await {
        // Load collision data for NPC walkability
        if !interior.collision.is_empty() {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&interior.collision)
            {
                instance.set_collision(&bytes).await;
            }
        }
        // Load heightmap if present
        if let Some(ref hm) = interior.heightmap {
            instance.set_heightmap(hm.clone()).await;
        }
        instance
            .spawn_npcs(&interior.entities, &state.entity_registry)
            .await;

        // Register gathering markers for this instance
        if !interior.gathering_zones.is_empty() {
            let markers: Vec<crate::gathering::GatheringMarker> = interior
                .gathering_zones
                .iter()
                .map(|gz| crate::gathering::GatheringMarker {
                    x: gz.x,
                    y: gz.y,
                    zone_id: gz.zone_id.clone(),
                })
                .collect();
            room.register_instance_gathering_markers(&instance.id, markers).await;
        }
    }

    // Store player's entrance position (where they came from) for return teleport
    if let Some((entrance_x, entrance_y)) = room.get_player_position(player_id).await {
        let mut entrance_positions = state.player_entrance_positions.write().await;
        entrance_positions.insert(player_id.to_string(), (entrance_x, entrance_y));
        info!(
            "Stored entrance position ({}, {}) for player {}",
            entrance_x, entrance_y, player_id
        );
    }

    // Track player's instance
    {
        let mut player_instances = state.player_instances.write().await;
        player_instances.insert(player_id.to_string(), instance.id.clone());
    }
    room.reset_sync_state(player_id).await;

    // Notify overworld players that this player has "left" (so they don't see a frozen sprite)
    room.send_to_overworld_players(
        ServerMessage::PlayerLeft {
            id: player_id.to_string(),
        },
        Some(player_id),
    )
    .await;

    // Get other players already in the instance BEFORE adding this player
    let other_players_in_instance: Vec<String> = instance.get_player_ids().await;

    // Add player to instance
    instance.add_player(player_id).await;

    // Update player position to spawn point, including Z from heightmap
    let spawn_z = {
        let hm = instance.heightmap.read().await;
        instance.get_height_at_sync(&hm, spawn.x as i32, spawn.y as i32)
    };
    room.set_player_position_and_z(player_id, spawn.x as i32, spawn.y as i32, spawn_z)
        .await;

    // Notify other players in the instance that this player joined
    if !other_players_in_instance.is_empty() {
        let player_name = room.get_player_name(player_id).await.unwrap_or_default();
        let (gender, skin) = room
            .get_player_appearance(player_id)
            .await
            .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
        let (hair_style, hair_color) = room
            .get_player_hair(player_id)
            .await
            .unwrap_or((None, None));

        for other_id in &other_players_in_instance {
            room.send_to_player(
                other_id,
                ServerMessage::PlayerJoined {
                    id: player_id.to_string(),
                    name: player_name.clone(),
                    x: spawn.x as i32,
                    y: spawn.y as i32,
                    gender: gender.clone(),
                    skin: skin.clone(),
                    hair_style,
                    hair_color,
                },
            )
            .await;
        }

        // Also send existing instance players to the joining player
        for other_id in &other_players_in_instance {
            if let Some(other_name) = room.get_player_name(other_id).await {
                let (other_x, other_y) = room
                    .get_player_position(other_id)
                    .await
                    .unwrap_or((spawn.x as i32, spawn.y as i32));
                let (other_gender, other_skin) = room
                    .get_player_appearance(other_id)
                    .await
                    .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
                let (other_hair_style, other_hair_color) =
                    room.get_player_hair(other_id).await.unwrap_or((None, None));

                room.send_to_player(
                    player_id,
                    ServerMessage::PlayerJoined {
                        id: other_id.clone(),
                        name: other_name,
                        x: other_x,
                        y: other_y,
                        gender: other_gender,
                        skin: other_skin,
                        hair_style: other_hair_style,
                        hair_color: other_hair_color,
                    },
                )
                .await;
            }
        }
    }

    info!(
        "Player {} entered instance {} (map: {}) at ({}, {})",
        player_id, instance.id, interior.id, spawn.x, spawn.y
    );

    // Start KOTH session if entering KOTH arena
    if interior.id == crate::game::koth_tick::KOTH_MAP_ID {
        let ct = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        // Save the player's overworld position so we can teleport them back
        let (entrance_x, entrance_y) = room
            .get_player_position(player_id)
            .await
            .unwrap_or((0, 0));
        room.start_koth_session(
            &instance.id,
            player_id,
            interior.size.width,
            interior.size.height,
            ct,
            entrance_x,
            entrance_y,
        )
        .await;
    }

    // Start or join boss session if entering desert wurm arena
    if interior.id == crate::game::boss_tick::BOSS_MAP_ID {
        let ct = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        if room.has_boss_session(&instance.id).await {
            // Add player to existing boss fight
            room.add_boss_player(&instance.id, player_id).await;
        } else {
            // Find the desert_wurm NPC in the instance and start a new boss session
            let npcs = instance.npcs.read().await;
            if let Some(boss_npc) = npcs.values().find(|n| n.prototype_id == "desert_wurm") {
                room.start_boss_session(
                    &instance.id,
                    &boss_npc.id,
                    boss_npc.hp,
                    boss_npc.max_hp,
                    boss_npc.x,
                    boss_npc.y,
                    instance.map_width as i32,
                    instance.map_height as i32,
                    ct,
                )
                .await;
            }
        }
    }

    // Start or join pharaoh boss session if entering pyramid tomb
    if interior.id == crate::game::boss_tick::PHARAOH_BOSS_MAP_ID {
        let ct = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        if room.has_pharaoh_boss_session(&instance.id).await {
            room.add_pharaoh_boss_player(&instance.id, player_id).await;
        } else {
            let npcs = instance.npcs.read().await;
            if let Some(boss_npc) = npcs.values().find(|n| n.prototype_id == "khareth_pharaoh") {
                room.start_pharaoh_boss_session(
                    &instance.id,
                    &boss_npc.id,
                    boss_npc.hp,
                    boss_npc.max_hp,
                    boss_npc.x,
                    boss_npc.y,
                    instance.map_width as i32,
                    instance.map_height as i32,
                    ct,
                )
                .await;
            }
        }
    }

    // Send transition message to client
    room.send_to_player(
        player_id,
        ServerMessage::MapTransition {
            map_type: "interior".to_string(),
            map_id: interior.id.clone(),
            spawn_x: spawn.x,
            spawn_y: spawn.y,
            instance_id: instance.id.clone(),
        },
    )
    .await;

    // Send interior map data
    let layers = vec![
        ChunkLayerData {
            layer_type: 0,
            tiles: interior.layers.ground.clone(),
        },
        ChunkLayerData {
            layer_type: 1,
            tiles: interior.layers.objects.clone(),
        },
        ChunkLayerData {
            layer_type: 2,
            tiles: interior.layers.overhead.clone(),
        },
    ];

    let collision = if interior.collision.is_empty() {
        vec![]
    } else {
        base64::engine::general_purpose::STANDARD
            .decode(&interior.collision)
            .unwrap_or_default()
    };

    let portals: Vec<ChunkPortalData> = interior
        .portals
        .iter()
        .map(|p| ChunkPortalData {
            id: p.id.clone(),
            x: p.x,
            y: p.y,
            width: p.width,
            height: p.height,
            target_map: p.target_map.clone(),
            target_spawn: p.target_spawn.clone().unwrap_or_default(),
        })
        .collect();

    let objects: Vec<protocol::ChunkObjectData> = interior
        .map_objects
        .iter()
        .map(|o| protocol::ChunkObjectData {
            gid: o.gid,
            tile_x: o.x,
            tile_y: o.y,
            width: o.width,
            height: o.height,
        })
        .collect();

    let walls: Vec<protocol::ChunkWallData> = interior
        .walls
        .iter()
        .map(|w| protocol::ChunkWallData {
            gid: w.gid,
            tile_x: w.x,
            tile_y: w.y,
            edge: w.edge.clone(),
        })
        .collect();

    room.send_to_player(
        player_id,
        ServerMessage::InteriorData {
            map_id: interior.id.clone(),
            name: interior.name.clone(),
            instance_id: instance.id.clone(),
            width: interior.size.width,
            height: interior.size.height,
            spawn_x: spawn.x,
            spawn_y: spawn.y,
            layers,
            collision,
            portals,
            objects,
            walls,
            heightmap: interior.heightmap.clone(),
            block_types_down: interior.block_types_down.clone(),
            block_types_right: interior.block_types_right.clone(),
        },
    )
    .await;

    // Send NPC updates for this instance
    let npc_updates = instance.get_npc_updates().await;
    if !npc_updates.is_empty() {
        info!(
            "Sending {} instance NPCs to player {}",
            npc_updates.len(),
            player_id
        );
        room.send_to_player(
            player_id,
            ServerMessage::StateSync {
                tick: 0,
                players: vec![],
                npcs: npc_updates,
                instance_id: instance.id.clone(),
            },
        )
        .await;
    }

    // Send gathering markers for this instance
    room.send_to_player(
        player_id,
        room.get_gathering_markers_message(Some(&instance.id)).await,
    )
    .await;

    // Send existing ground items in this instance
    let ground_items = room.get_ground_items_in_instance(Some(&instance.id)).await;
    for item_msg in ground_items {
        room.send_to_player(player_id, item_msg).await;
    }

    // Send chest positions for this interior
    let chest_msg = room.get_chest_positions_message(Some(&interior.id)).await;
    if let protocol::ServerMessage::ChestPositions { ref positions } = chest_msg {
        info!(
            "Sending {} chest positions for interior '{}' to player {}",
            positions.len(),
            interior.id,
            player_id
        );
    }
    room.send_to_player(player_id, chest_msg).await;
}

async fn handle_client_message(
    state: &AppState,
    room: &GameRoom,
    player_id: &str,
    data: &[u8],
) -> Result<(), String> {
    let msg = protocol::decode_client_message(data)?;
    let handler_start = std::time::Instant::now();
    let msg_name = msg.name();

    match msg {
        ClientMessage::Move { dx, dy, seq } => {
            room.handle_move(player_id, dx, dy, seq).await;
        }
        ClientMessage::Dash => {
            room.handle_dash(player_id).await;
        }
        ClientMessage::Jump => {
            room.handle_jump(player_id).await;
        }
        ClientMessage::Face { direction } => {
            room.handle_face(player_id, direction).await;
        }
        ClientMessage::Chat { text, channel } => {
            room.handle_chat(player_id, &text, &channel).await;
        }
        ClientMessage::Attack => {
            room.handle_attack(player_id, None, None).await;
        }
        ClientMessage::Target { entity_id } => {
            room.handle_target(player_id, &entity_id).await;
        }
        ClientMessage::Pickup { item_id } => {
            room.handle_pickup(player_id, &item_id).await;
        }
        ClientMessage::UseItem { slot_index } => {
            room.handle_use_item(player_id, slot_index).await;
        }
        ClientMessage::RequestChunk { chunk_x, chunk_y } => {
            if let Some(chunk_msg) = room.handle_chunk_request(chunk_x, chunk_y).await {
                room.send_to_player(player_id, chunk_msg).await;
            }
        }
        ClientMessage::Interact { npc_id } => {
            room.handle_npc_interact(player_id, &npc_id).await;
        }
        ClientMessage::InteractObject { x, y } => {
            room.handle_interact_object(player_id, x, y).await;
        }
        ClientMessage::UseWaystone { x, y } => {
            room.handle_use_waystone(player_id, x, y).await;
        }
        ClientMessage::DialogueChoiceMsg {
            quest_id,
            choice_id,
        } => {
            room.handle_dialogue_choice(player_id, &quest_id, &choice_id)
                .await;
        }
        ClientMessage::AcceptQuest { quest_id: _ } => {
            // Quest acceptance is handled through dialogue choices
            // This is a fallback if client sends direct accept
        }
        ClientMessage::AbandonQuest { quest_id: _ } => {
            // TODO: Implement quest abandonment
        }
        ClientMessage::Craft { recipe_id } => {
            room.handle_craft(player_id, &recipe_id).await;
        }
        ClientMessage::StartCraft { recipe_id } => {
            room.handle_start_craft(player_id, &recipe_id).await;
        }
        ClientMessage::CancelCraft => {
            room.handle_cancel_craft(player_id).await;
        }
        ClientMessage::Equip { slot_index } => {
            room.handle_equip(player_id, slot_index).await;
        }
        ClientMessage::Unequip { slot_type } => {
            room.handle_unequip(player_id, &slot_type).await;
        }
        ClientMessage::DropItem {
            slot_index,
            quantity,
            target_x,
            target_y,
        } => {
            room.handle_drop_item(player_id, slot_index, quantity, target_x, target_y)
                .await;
        }
        ClientMessage::DropGold { amount } => {
            room.handle_drop_gold(player_id, amount).await;
        }
        ClientMessage::SwapSlots { from_slot, to_slot } => {
            room.handle_swap_slots(player_id, from_slot, to_slot).await;
        }
        ClientMessage::ShopBuy {
            npc_id,
            item_id,
            quantity,
        } => {
            room.handle_shop_buy(player_id, &npc_id, &item_id, quantity)
                .await;
        }
        ClientMessage::ShopSell {
            npc_id,
            item_id,
            quantity,
        } => {
            room.handle_shop_sell(player_id, &npc_id, &item_id, quantity)
                .await;
        }
        ClientMessage::EnterPortal { portal_id } => {
            handle_enter_portal(state, room, player_id, &portal_id).await;
        }
        ClientMessage::StartGathering { marker_x, marker_y } => {
            room.handle_start_gathering(player_id, marker_x, marker_y)
                .await;
        }
        ClientMessage::StopGathering => {
            room.handle_stop_gathering(player_id).await;
        }
        ClientMessage::ChopTree {
            tree_x,
            tree_y,
            tree_gid,
        } => {
            room.handle_chop_tree(player_id, tree_x, tree_y, tree_gid)
                .await;
        }
        ClientMessage::MineRock {
            rock_x,
            rock_y,
            rock_gid,
        } => {
            room.handle_mine_rock(player_id, rock_x, rock_y, rock_gid)
                .await;
        }
        ClientMessage::SitChair { tile_x, tile_y } => {
            room.handle_sit_chair(player_id, tile_x, tile_y).await;
        }
        ClientMessage::StandUp => {
            room.handle_stand_up(player_id).await;
        }
        ClientMessage::PlantSeed { patch_id, item_id } => {
            room.handle_plant_seed(player_id, &patch_id, &item_id).await;
        }
        ClientMessage::HarvestCrop { patch_id } => {
            room.handle_harvest_crop(player_id, &patch_id).await;
        }
        // Friend system messages
        ClientMessage::SendFriendRequest { target_name } => {
            room.handle_send_friend_request(player_id, &target_name)
                .await;
        }
        ClientMessage::AcceptFriendRequest { requester_id } => {
            room.handle_accept_friend_request(player_id, requester_id)
                .await;
        }
        ClientMessage::DeclineFriendRequest { requester_id } => {
            room.handle_decline_friend_request(player_id, requester_id)
                .await;
        }
        ClientMessage::RemoveFriend { friend_id } => {
            room.handle_remove_friend(player_id, friend_id).await;
        }
        ClientMessage::GetOnlinePlayers => {
            room.handle_get_online_players(player_id).await;
        }
        // Prayer system messages
        ClientMessage::TogglePrayer { prayer_id } => {
            room.handle_toggle_prayer(player_id, &prayer_id).await;
        }
        ClientMessage::BuryBones { slot } => {
            room.handle_bury_bones(player_id, slot).await;
        }
        ClientMessage::OfferBones { slot, altar_id } => {
            room.handle_offer_bones(player_id, slot, &altar_id).await;
        }
        ClientMessage::OfferAllBones { item_id, altar_id } => {
            room.handle_offer_all_bones(player_id, &item_id, &altar_id)
                .await;
        }
        ClientMessage::PrayAtAltar { altar_id } => {
            room.handle_pray_at_altar(player_id, &altar_id).await;
        }
        // Spell system messages
        ClientMessage::CastSpell { spell_id } => {
            if spell_id == "return_home" {
                // Return Home needs special instance cleanup handling
                use crate::interior::InstanceType;

                let spell_def = match crate::spell::get_spell(&spell_id) {
                    Some(s) => s,
                    None => {
                        room.handle_cast_spell(player_id, &spell_id).await;
                        return Ok(());
                    }
                };
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                let instance_id: Option<String> = {
                    let instances = state.player_instances.read().await;
                    instances.get(player_id).cloned()
                };

                let success = room
                    .cast_return_home_spell(player_id, spell_def, current_time)
                    .await;

                if success {
                    if let Some(instance_id) = instance_id {
                        // Player was in an instance - do instance cleanup
                        {
                            let mut instances = state.player_instances.write().await;
                            instances.remove(player_id);
                        }

                        if let Some(instance) =
                            state.instance_manager.get_by_instance_id(&instance_id)
                        {
                            let other_players: Vec<String> = instance
                                .get_player_ids()
                                .await
                                .into_iter()
                                .filter(|id| id != player_id)
                                .collect();

                            let remaining = instance.remove_player(player_id).await;
                            if remaining == 0 && instance.instance_type == InstanceType::Private {
                                if let Some(owner_id) = &instance.owner_id {
                                    state
                                        .instance_manager
                                        .remove_private(owner_id, &instance.map_id);
                                }
                            }

                            for other_id in &other_players {
                                room.send_to_player(
                                    other_id,
                                    ServerMessage::PlayerLeft {
                                        id: player_id.to_string(),
                                    },
                                )
                                .await;
                                room.send_to_player(
                                    player_id,
                                    ServerMessage::PlayerLeft {
                                        id: other_id.clone(),
                                    },
                                )
                                .await;
                            }
                        }

                        // Clean up entrance position
                        {
                            let mut entrance_positions =
                                state.player_entrance_positions.write().await;
                            entrance_positions.remove(player_id);
                        }

                        // Send map transition to overworld
                        let spawn_x = -30.0_f32;
                        let spawn_y = 19.0_f32;
                        let spawn_chunk = chunk::ChunkCoord::from_world(
                            spawn_x.floor() as i32,
                            spawn_y.floor() as i32,
                        );
                        room.world()
                            .preload_chunks(spawn_chunk, game::SPAWN_PRELOAD_RADIUS)
                            .await;
                        room.send_to_player(
                            player_id,
                            ServerMessage::MapTransition {
                                map_type: "overworld".to_string(),
                                map_id: "world_0".to_string(),
                                spawn_x,
                                spawn_y,
                                instance_id: String::new(),
                            },
                        )
                        .await;

                        // Re-send overworld data
                        room.send_to_player(player_id, room.get_chair_positions_message().await)
                            .await;
                        room.send_to_player(player_id, room.get_gathering_markers_message(None).await)
                            .await;
                        room.send_to_player(
                            player_id,
                            room.get_chest_positions_message(None).await,
                        )
                        .await;

                        // Notify overworld players
                        {
                            let player_name =
                                room.get_player_name(player_id).await.unwrap_or_default();
                            let (gender, skin) = room
                                .get_player_appearance(player_id)
                                .await
                                .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
                            let (hair_style, hair_color) = room
                                .get_player_hair(player_id)
                                .await
                                .unwrap_or((None, None));
                            room.send_to_overworld_players(
                                ServerMessage::PlayerJoined {
                                    id: player_id.to_string(),
                                    name: player_name,
                                    x: spawn_x as i32,
                                    y: spawn_y as i32,
                                    gender,
                                    skin,
                                    hair_style,
                                    hair_color,
                                },
                                Some(player_id),
                            )
                            .await;
                        }
                    }
                }
            } else {
                room.handle_cast_spell(player_id, &spell_id).await;
            }
        }
        // Auth and Register are handled via HTTP endpoints, not WebSocket
        // ===== Bank Messages =====
        ClientMessage::BankDeposit { item_id, quantity } => {
            room.handle_bank_deposit(player_id, &item_id, quantity)
                .await;
        }
        ClientMessage::BankWithdraw { item_id, quantity } => {
            room.handle_bank_withdraw(player_id, &item_id, quantity)
                .await;
        }
        ClientMessage::BankDepositGold { amount } => {
            room.handle_bank_deposit_gold(player_id, amount).await;
        }
        ClientMessage::BankWithdrawGold { amount } => {
            room.handle_bank_withdraw_gold(player_id, amount).await;
        }
        ClientMessage::BankDepositAll => {
            room.handle_bank_deposit_all(player_id).await;
        }
        ClientMessage::BankSwapSlots { slot_a, slot_b } => {
            room.handle_bank_swap_slots(player_id, slot_a, slot_b).await;
        }
        ClientMessage::BankSort => {
            room.handle_bank_sort(player_id).await;
        }

        ClientMessage::Auth { .. } | ClientMessage::Register { .. } => {}
        ClientMessage::StartCraftBatch {
            recipe_id,
            quantity,
        } => {
            room.handle_start_craft_batch(player_id, &recipe_id, quantity)
                .await;
        }
        // Ping/Pong for latency measurement
        ClientMessage::Ping { timestamp } => {
            room.send_to_player(player_id, ServerMessage::Pong { timestamp })
                .await;
        }
        ClientMessage::SlayerGetTask { master_id } => {
            room.handle_slayer_get_task(player_id, &master_id).await;
        }
        ClientMessage::SlayerCancelTask => {
            room.handle_slayer_cancel_task(player_id).await;
        }
        ClientMessage::SlayerBuyReward {
            reward_id,
            target_monster_id,
        } => {
            room.handle_slayer_buy_reward(player_id, &reward_id, target_monster_id)
                .await;
        }
        ClientMessage::SlayerRemoveBlock { monster_id } => {
            room.handle_slayer_remove_block(player_id, &monster_id)
                .await;
        }
        ClientMessage::StartAutoAction {
            target_type,
            target_id,
            action,
        } => {
            room.handle_start_auto_action(player_id, &target_type, &target_id, &action)
                .await;
        }
        ClientMessage::CancelAutoAction => {
            room.handle_cancel_auto_action(player_id).await;
        }
        // ===== Chest System Messages =====
        ClientMessage::OpenChest { x, y } => {
            room.handle_open_chest(player_id, x, y).await;
        }
        ClientMessage::ChestTake { chest_id, slot } => {
            room.handle_chest_take(player_id, &chest_id, slot).await;
        }
        ClientMessage::ChestDeposit {
            chest_id,
            inventory_slot,
        } => {
            room.handle_chest_deposit(player_id, &chest_id, inventory_slot)
                .await;
        }
        ClientMessage::SpectatorUpgrade { .. } => {
            // Handled by spectator WebSocket handler, not the normal game message dispatch
            tracing::warn!(
                "SpectatorUpgrade received in normal message handler for player {}",
                player_id
            );
        }
        // Trade system messages
        ClientMessage::TradeRequest { target_id } => {
            room.handle_trade_request(player_id, &target_id).await;
        }
        ClientMessage::TradeAcceptRequest { requester_id } => {
            room.handle_trade_accept_request(player_id, &requester_id)
                .await;
        }
        ClientMessage::TradeDeclineRequest { requester_id } => {
            room.handle_trade_decline_request(player_id, &requester_id)
                .await;
        }
        ClientMessage::TradeOfferItem {
            slot_index,
            quantity,
        } => {
            room.handle_trade_offer_item(player_id, slot_index, quantity)
                .await;
        }
        ClientMessage::TradeRemoveItem { offer_index } => {
            room.handle_trade_remove_item(player_id, offer_index).await;
        }
        ClientMessage::TradeOfferGold { amount } => {
            room.handle_trade_offer_gold(player_id, amount).await;
        }
        ClientMessage::TradeAccept => {
            room.handle_trade_accept(player_id).await;
        }
        ClientMessage::TradeCancel => {
            room.handle_trade_cancel(player_id).await;
        }
        // Stall system messages
        ClientMessage::StallOpen { name } => {
            room.handle_stall_open(player_id, &name).await;
        }
        ClientMessage::StallClose => {
            room.handle_stall_close(player_id).await;
        }
        ClientMessage::StallSetItem {
            inventory_slot,
            quantity,
            price,
        } => {
            room.handle_stall_set_item(player_id, inventory_slot, quantity, price)
                .await;
        }
        ClientMessage::StallRemoveItem { stall_slot } => {
            room.handle_stall_remove_item(player_id, stall_slot).await;
        }
        ClientMessage::StallUpdatePrice { stall_slot, price } => {
            room.handle_stall_update_price(player_id, stall_slot, price)
                .await;
        }
        ClientMessage::StallBrowse {
            player_id: target_id,
        } => {
            room.handle_stall_browse(player_id, &target_id).await;
        }
        ClientMessage::StallBuy {
            seller_id,
            stall_slot,
            quantity,
        } => {
            room.handle_stall_buy(player_id, &seller_id, stall_slot, quantity)
                .await;
        }
        ClientMessage::SetCombatStyle { style } => {
            if let Some(combat_style) = crate::game::CombatStyle::from_str(&style) {
                room.set_combat_style(player_id, combat_style).await;
            } else {
                tracing::warn!("Player {} sent invalid combat style: {}", player_id, style);
            }
        }
        ClientMessage::KothContinue => {
            let ct = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            room.handle_koth_continue(player_id, ct).await;
        }
        ClientMessage::KothLeave => {
            let ct = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            room.handle_koth_leave(player_id, ct).await;
        }
    }

    let handler_duration = handler_start.elapsed();
    let handler_ms = handler_duration.as_secs_f64() * 1000.0;
    state.perf_metrics.record_handler(msg_name, handler_ms);
    if handler_duration.as_millis() > 20 {
        tracing::warn!(
            "Slow handler: {} took {:.2}ms for player {}",
            msg_name,
            handler_ms,
            player_id
        );
    }

    Ok(())
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    // Initialize logging with in-memory buffer for /api/logs endpoint
    let log_buffer = log_buffer::LogBuffer::new();
    {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive("isometric_server=info".parse().unwrap()),
            )
            .with(tracing_subscriber::fmt::layer())
            .with(log_buffer::LogBufferLayer::new(log_buffer.clone()))
            .init();
    }

    let state = AppState::new(log_buffer).await;

    // Spawn game tick loop
    let tick_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(50)); // 20 Hz
        // Delay (not Burst) prevents cascading catch-up ticks when a tick runs slow.
        // Burst is the default and causes rapid-fire ticks after lag, making it worse.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            let loop_start = std::time::Instant::now();
            let room_count = tick_state.rooms.len();
            for room in tick_state.rooms.iter() {
                let room_start = std::time::Instant::now();
                let tick_telemetry = room.tick().await;
                let room_ms = room_start.elapsed().as_secs_f64() * 1000.0;
                tick_state
                    .perf_metrics
                    .record_room_tick(&room.name, room_ms);
                tick_state.perf_metrics.record_movement(
                    tick_telemetry.pending_moves,
                    tick_telemetry.rejected_moves,
                    tick_telemetry.rejected_tile_blocked,
                    tick_telemetry.rejected_player_blocked,
                    tick_telemetry.rejected_npc_blocked,
                    tick_telemetry.rejected_chair_blocked,
                    tick_telemetry.rejected_arena_blocked,
                );
                tick_state.perf_metrics.record_movement_anomalies(
                    tick_telemetry.movement_stale_packets_ignored,
                    tick_telemetry.movement_seq_gap_events,
                    tick_telemetry.movement_input_gap_events,
                    tick_telemetry.movement_stale_intent_clears,
                );
                tick_state.perf_metrics.record_state_sync(
                    tick_telemetry.state_sync_send_attempts,
                    tick_telemetry.state_sync_capacity_skips,
                    tick_telemetry.state_sync_try_send_drops,
                    tick_telemetry.state_sync_full_sends,
                    tick_telemetry.state_sync_delta_sends,
                    tick_telemetry.state_sync_fallback_self_only_sends,
                    tick_telemetry.state_sync_raw_bytes,
                    tick_telemetry.state_sync_bytes_sent,
                );
            }
            let loop_ms = loop_start.elapsed().as_secs_f64() * 1000.0;
            tick_state
                .perf_metrics
                .record_tick_loop(loop_ms, room_count);
            if loop_ms > 50.0 {
                warn!(
                    "Tick loop overrun: {:.2}ms across {} room(s)",
                    loop_ms, room_count
                );
            }
        }
    });

    // Spawn auto-save loop (every 30 seconds)
    // Snapshots all player data quickly under locks, then spawns DB writes
    // concurrently so they don't block the game tick loop.
    let save_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let cycle_start = std::time::Instant::now();
            let snapshot_phase_start = std::time::Instant::now();

            // Phase 1: Collect valid sessions and batch-snapshot data per room
            let mut snapshots = Vec::new();

            // Group sessions by room for batch snapshotting
            let mut room_players: std::collections::HashMap<String, Vec<(i64, String, String)>> =
                std::collections::HashMap::new();
            for session in save_state.sessions.iter() {
                let session_data = session.value().clone();
                if !save_state
                    .auth_sessions
                    .contains_key(&session_data.auth_token)
                {
                    warn!(
                        "Auto-save skipped for {}: auth token no longer valid",
                        session_data.character_name
                    );
                    continue;
                }
                room_players
                    .entry(session_data.room_id.clone())
                    .or_default()
                    .push((
                        session_data.character_id,
                        session_data.character_name.clone(),
                        session_data.player_id.clone(),
                    ));
            }

            // Batch-snapshot all players per room (single lock acquisition per room)
            for (room_id, players) in &room_players {
                if let Some(room) = save_state.rooms.get(room_id) {
                    let player_ids: Vec<String> =
                        players.iter().map(|(_, _, pid)| pid.clone()).collect();
                    let bulk_data = room.get_bulk_save_data(&player_ids).await;

                    for (character_id, character_name, player_id) in players {
                        if let Some((
                            mut save_data,
                            quest_state,
                            discovered_recipes,
                            slayer_state,
                            unlocked_spells,
                        )) = bulk_data.get(player_id).cloned()
                        {
                            let played_time_delta = save_state
                                .play_time_anchors
                                .get(character_id)
                                .map(|anchor| anchor.elapsed().as_secs() as i64)
                                .unwrap_or(0);
                            save_state
                                .play_time_anchors
                                .insert(*character_id, std::time::Instant::now());

                            // Populate entrance position from runtime HashMap
                            if save_data.current_map.is_some() {
                                let entrance_positions =
                                    save_state.player_entrance_positions.read().await;
                                if let Some(&(ex, ey)) = entrance_positions.get(player_id) {
                                    save_data.entrance_x = Some(ex as f32);
                                    save_data.entrance_y = Some(ey as f32);
                                }
                            }

                            snapshots.push((
                                *character_id,
                                character_name.clone(),
                                save_data,
                                quest_state,
                                played_time_delta,
                                discovered_recipes,
                                slayer_state,
                                unlocked_spells,
                            ));
                        }
                    }
                }
            }
            let snapshot_phase_ms = snapshot_phase_start.elapsed().as_millis();
            let snapshot_count = snapshots.len();
            let mut write_phase_ms = 0u128;

            // Phase 2: Write all snapshots to DB concurrently (no locks held)
            if !snapshots.is_empty() {
                let write_phase_start = std::time::Instant::now();
                let save_count = snapshots.len();
                let mut save_tasks = Vec::with_capacity(save_count);

                for (
                    character_id,
                    character_name,
                    save_data,
                    quest_state,
                    played_time_delta,
                    discovered_recipes,
                    slayer_state,
                    unlocked_spells,
                ) in snapshots
                {
                    let db = save_state.db.clone();
                    save_tasks.push(tokio::spawn(async move {
                        if let Err(e) = db
                            .save_character(
                                character_id,
                                save_data.x,
                                save_data.y,
                                save_data.z,
                                save_data.hp,
                                save_data.prayer_points,
                                save_data.mp,
                                &save_data.skills,
                                save_data.gold,
                                &save_data.inventory_json,
                                save_data.equipped_head.as_deref(),
                                save_data.equipped_body.as_deref(),
                                save_data.equipped_weapon.as_deref(),
                                save_data.equipped_back.as_deref(),
                                save_data.equipped_feet.as_deref(),
                                save_data.equipped_ring.as_deref(),
                                save_data.equipped_gloves.as_deref(),
                                save_data.equipped_necklace.as_deref(),
                                save_data.equipped_belt.as_deref(),
                                played_time_delta,
                                save_data.current_map.as_deref(),
                                save_data.sitting_at_x,
                                save_data.sitting_at_y,
                                save_data.entrance_x,
                                save_data.entrance_y,
                                &save_data.bank_json,
                                save_data.bank_gold,
                                save_data.bank_max_slots,
                                &save_data.combat_style_prefs,
                            )
                            .await
                        {
                            warn!("Auto-save failed for character {}: {}", character_name, e);
                        }

                        if let Some(quest_state) = quest_state {
                            let _ = db
                                .save_character_quest_state(character_id, &quest_state)
                                .await;
                        }

                        // Save discovered recipes
                        for recipe_id in &discovered_recipes {
                            let _ = db.save_discovered_recipe(character_id, recipe_id).await;
                        }

                        // Save slayer state
                        if let Some(ref slayer) = slayer_state {
                            let _ = db.save_character_slayer_state(character_id, slayer).await;
                        }

                        // Save unlocked spells
                        for spell_id in &unlocked_spells {
                            let _ = db.save_unlocked_spell(character_id, spell_id).await;
                        }
                    }));
                }

                // Wait for all saves to finish (they run concurrently)
                let mut saved_count = 0;
                for task in save_tasks {
                    if task.await.is_ok() {
                        saved_count += 1;
                    }
                }

                write_phase_ms = write_phase_start.elapsed().as_millis();
                info!(
                    "Auto-saved {} character(s) to database (snapshot={}ms, write={}ms, total={}ms)",
                    saved_count,
                    snapshot_phase_ms,
                    write_phase_ms,
                    cycle_start.elapsed().as_millis()
                );
            }

            // Save chest data for all rooms
            for room_entry in save_state.rooms.iter() {
                let room = room_entry.value().clone();
                let save_data = room.get_chest_save_data().await;
                if let Err(e) = save_state.db.save_all_chests(&save_data).await {
                    tracing::warn!("Failed to save chest data: {}", e);
                }
            }

            let cycle_ms = cycle_start.elapsed().as_millis();
            save_state.perf_metrics.record_autosave(
                snapshot_phase_ms,
                write_phase_ms,
                cycle_ms,
                room_players.len(),
                snapshot_count,
            );
            if cycle_ms > 250 {
                warn!(
                    "Slow auto-save cycle: {}ms (rooms={}, snapshots={}, snapshot_phase={}ms, write_phase={}ms)",
                    cycle_ms,
                    room_players.len(),
                    snapshot_count,
                    snapshot_phase_ms,
                    write_phase_ms,
                );
            }
        }
    });

    // Spawn periodic perf summary logs
    let perf_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let perf = perf_state.perf_metrics.snapshot(3, 0);
            info!(
                "[PERF] uptime={}s tick_loop(p95={}ms p99={}ms max={}ms) room_tick(p95={}ms p99={}ms max={}ms) autosave_total(p95={}ms max={}ms) handler(p95={}ms max={}ms) ws_send(p95={}ms max={}ms) movement(reject_rate={}%, attempts={}, rejected={}, reasons=tile:{}({}%) player:{}({}%) npc:{}({}%) chair:{}({}%) arena:{}({}%), stale_packets={}({}%) seq_gaps={}({}%) input_gaps={}({}%) stale_intent_clears={}({}%)) state_sync(drop_rate={}%, skip_rate={}%, attempts={}, capacity_skips={}, drops={}, full={}({}%), delta={}({}%), fallback_self={}, raw_bytes={}, wire_bytes={}, wire_vs_raw={}%) counters(overruns={} slow_room_ticks={} slow_autosaves={} slow_handlers={} slow_ws_sends={})",
                perf.uptime_seconds,
                perf.tick_loop_ms.p95_ms,
                perf.tick_loop_ms.p99_ms,
                perf.tick_loop_ms.max_ms,
                perf.room_tick_ms.p95_ms,
                perf.room_tick_ms.p99_ms,
                perf.room_tick_ms.max_ms,
                perf.autosave_total_ms.p95_ms,
                perf.autosave_total_ms.max_ms,
                perf.handler_ms.p95_ms,
                perf.handler_ms.max_ms,
                perf.ws_send_ms.p95_ms,
                perf.ws_send_ms.max_ms,
                perf.derived_rates.movement_reject_rate_pct,
                perf.counters.movement_attempts,
                perf.counters.movement_rejections,
                perf.counters.movement_rejections_tile_blocked,
                perf.derived_rates.movement_reject_tile_share_pct,
                perf.counters.movement_rejections_player_blocked,
                perf.derived_rates.movement_reject_player_share_pct,
                perf.counters.movement_rejections_npc_blocked,
                perf.derived_rates.movement_reject_npc_share_pct,
                perf.counters.movement_rejections_chair_blocked,
                perf.derived_rates.movement_reject_chair_share_pct,
                perf.counters.movement_rejections_arena_blocked,
                perf.derived_rates.movement_reject_arena_share_pct,
                perf.counters.movement_stale_packets_ignored,
                perf.derived_rates.movement_stale_packet_rate_pct,
                perf.counters.movement_seq_gap_events,
                perf.derived_rates.movement_seq_gap_rate_pct,
                perf.counters.movement_input_gap_events,
                perf.derived_rates.movement_input_gap_rate_pct,
                perf.counters.movement_stale_intent_clears,
                perf.derived_rates.movement_stale_intent_clear_rate_pct,
                perf.derived_rates.state_sync_drop_rate_pct,
                perf.derived_rates.state_sync_capacity_skip_rate_pct,
                perf.counters.state_sync_send_attempts,
                perf.counters.state_sync_capacity_skips,
                perf.counters.state_sync_try_send_drops,
                perf.counters.state_sync_full_sends,
                perf.derived_rates.state_sync_full_share_pct,
                perf.counters.state_sync_delta_sends,
                perf.derived_rates.state_sync_delta_share_pct,
                perf.counters.state_sync_fallback_self_only_sends,
                perf.counters.state_sync_raw_bytes,
                perf.counters.state_sync_wire_bytes,
                perf.derived_rates.state_sync_wire_vs_raw_pct,
                perf.counters.tick_loop_overruns,
                perf.counters.slow_room_ticks,
                perf.counters.slow_autosaves,
                perf.counters.slow_handlers,
                perf.counters.slow_ws_sends,
            );
        }
    });

    // Build router
    let app = Router::new()
        // Health check
        .route("/health", get(health_check))
        // Authentication
        .route("/api/register", post(register_account))
        .route("/api/login", post(login_account))
        .route("/api/logout", post(logout_account))
        // Characters
        .route(
            "/api/characters",
            get(list_characters).post(create_character),
        )
        .route("/api/characters/:id", delete(delete_character))
        // Matchmaking
        .route(
            "/matchmake/joinOrCreate/:room",
            post(matchmake_join_or_create),
        )
        // WebSocket
        .route("/spectate", get(spectate_handler))
        .route("/:room_id", get(ws_handler))
        // Stats API (public, read-only)
        .route("/api/stats/overview", get(stats_overview))
        .route("/api/stats/online", get(stats_online))
        .route("/api/stats/leaderboard", get(stats_leaderboard))
        .route("/api/stats/player/:name", get(stats_player_profile))
        .route("/api/stats/items", get(stats_items))
        .route("/api/stats/entities", get(stats_entities))
        .route("/api/perf", get(api_perf))
        // Server logs (admin)
        .route("/api/logs", get(api_logs))
        .route("/logs", get(logs_page))
        // In development, you may want CorsLayer::permissive()
        // For production, specify allowed origins explicitly
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                ]),
        );

    // Graceful shutdown: save all players on Ctrl+C
    let shutdown_state = state.clone();

    let app = app.with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 2567));
    info!("Game server listening on http://{}", addr);
    let shutdown_signal = async move {
        tokio::signal::ctrl_c().await.ok();
        info!("Shutdown signal received, saving all players...");

        // Group sessions by room_id for bulk saving
        let mut room_players: HashMap<String, Vec<(String, i64)>> = HashMap::new();
        for entry in shutdown_state.sessions.iter() {
            room_players
                .entry(entry.room_id.clone())
                .or_default()
                .push((entry.player_id.clone(), entry.character_id));
        }

        // Save all players in each room
        let mut saved_count = 0u32;
        for (room_id, players) in &room_players {
            let room = match shutdown_state.rooms.get(room_id) {
                Some(r) => r.value().clone(),
                None => continue,
            };
            let player_ids: Vec<String> = players.iter().map(|(pid, _)| pid.clone()).collect();
            let char_id_map: HashMap<&str, i64> = players
                .iter()
                .map(|(pid, cid)| (pid.as_str(), *cid))
                .collect();

            let bulk_data = room.get_bulk_save_data(&player_ids).await;
            for (
                player_id,
                (mut save_data, quest_state, discovered_recipes, slayer_state, unlocked_spells),
            ) in bulk_data
            {
                let character_id = match char_id_map.get(player_id.as_str()) {
                    Some(id) => *id,
                    None => continue,
                };

                // Populate entrance position from runtime HashMap
                if save_data.current_map.is_some() {
                    let entrance_positions = shutdown_state.player_entrance_positions.read().await;
                    if let Some(&(ex, ey)) = entrance_positions.get(&player_id) {
                        save_data.entrance_x = Some(ex as f32);
                        save_data.entrance_y = Some(ey as f32);
                    }
                }

                if let Err(e) = shutdown_state
                    .db
                    .save_character(
                        character_id,
                        save_data.x,
                        save_data.y,
                        save_data.z,
                        save_data.hp,
                        save_data.prayer_points,
                        save_data.mp,
                        &save_data.skills,
                        save_data.gold,
                        &save_data.inventory_json,
                        save_data.equipped_head.as_deref(),
                        save_data.equipped_body.as_deref(),
                        save_data.equipped_weapon.as_deref(),
                        save_data.equipped_back.as_deref(),
                        save_data.equipped_feet.as_deref(),
                        save_data.equipped_ring.as_deref(),
                        save_data.equipped_gloves.as_deref(),
                        save_data.equipped_necklace.as_deref(),
                        save_data.equipped_belt.as_deref(),
                        0, // played_time_delta - skip for shutdown
                        save_data.current_map.as_deref(),
                        save_data.sitting_at_x,
                        save_data.sitting_at_y,
                        save_data.entrance_x,
                        save_data.entrance_y,
                        &save_data.bank_json,
                        save_data.bank_gold,
                        save_data.bank_max_slots,
                        &save_data.combat_style_prefs,
                    )
                    .await
                {
                    error!("Shutdown save failed for player {}: {}", player_id, e);
                } else {
                    saved_count += 1;
                }

                if let Some(quest_state) = quest_state {
                    let _ = shutdown_state
                        .db
                        .save_character_quest_state(character_id, &quest_state)
                        .await;
                }

                for recipe_id in &discovered_recipes {
                    let _ = shutdown_state
                        .db
                        .save_discovered_recipe(character_id, recipe_id)
                        .await;
                }

                if let Some(ref slayer) = slayer_state {
                    let _ = shutdown_state
                        .db
                        .save_character_slayer_state(character_id, slayer)
                        .await;
                }

                for spell_id in &unlocked_spells {
                    let _ = shutdown_state
                        .db
                        .save_unlocked_spell(character_id, spell_id)
                        .await;
                }
            }
        }

        info!("Saved {} player(s). Shutting down.", saved_count);
    };

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .tcp_nodelay(true)
    .with_graceful_shutdown(shutdown_signal)
    .await
    .unwrap();
}
