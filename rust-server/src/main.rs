#![forbid(unsafe_code)]

use crate::data::item_def::EquipmentSlot;
use crate::skills::Skills;
use axum::{
    Json, Router,
    extract::{
        ConnectInfo, Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
};
use dashmap::{DashMap, DashSet};
use futures::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::Row;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, OwnedRwLockReadGuard, RwLock, mpsc, watch};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{error, info, warn};
use uuid::Uuid;

mod app_state;
mod arena;
mod boss;
mod characters;
mod chest;
mod chunk;
mod client_messages;
mod collection_log;
mod config;
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
mod instances;
mod interior;
mod interior_registry;
mod item;
mod koth;
mod log_buffer;
mod matchmaking;
mod mining;
mod npc;
mod perf_metrics;
mod pharaoh_boss;
mod prayer;
mod protocol;
mod quest;
mod resource_contracts;
mod scroll_spell;
mod security;
mod server_auth;
mod shop;
mod skills;
mod slayer;
mod spectator;
mod spell;
mod stats_api;
mod tilemap;
mod waystone;
mod websocket;
mod woodcutting;
mod world;

use app_state::*;
use characters::*;
use client_messages::*;
use config::ServerConfig;
use crafting::CraftingRegistry;
use data::ItemRegistry;
use db::Database;
use entity::EntityRegistry;
use game::GameRoom;
use instance::InstanceManager;
use instances::*;
use interior_registry::InteriorRegistry;
use matchmaking::*;
use prayer::PrayerRegistry;
use protocol::{ClientMessage, ServerMessage};
use quest::{ObjectiveType, QuestRegistry};
use security::{RateLimiter, SessionTokenSigner};
use server_auth::*;
use spectator::*;
use stats_api::*;
use websocket::*;

// ============================================================================
// App State
// ============================================================================

/// Game session data for a connected player
#[derive(Clone)]
struct GameSession {
    room_id: String,
    player_id: String,
    character_name: String,          // Character name for display
    character_id: i64,               // Database character ID
    account_id: i64,                 // Database account ID
    auth_token: String,              // Token used for this session (for validation)
    current_map: Option<String>,     // Interior map ID to auto-enter on connect (None = overworld)
    entrance_x: Option<f32>,         // Overworld X where player entered interior
    entrance_y: Option<f32>,         // Overworld Y where player entered interior
    is_new_character: bool,          // True if played_time == 0 (for tutorial)
    command_gate: Arc<RwLock<bool>>, // True while this session may mutate player state
}

#[derive(Clone)]
struct AppState {
    config: Arc<ServerConfig>,
    rooms: Arc<DashMap<String, Arc<GameRoom>>>,
    room_creation_lock: Arc<Mutex<()>>,
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
    collection_log_defs: Arc<collection_log::CollectionLogDefinitions>,
    collection_log_display_names: Arc<Vec<(String, String)>>,
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
    /// Serializes matchmaking and takeover for the same character.
    character_session_locks: Arc<DashMap<i64, Arc<Mutex<()>>>>,
    /// In-memory log buffer for /api/logs endpoint
    log_buffer: log_buffer::LogBuffer,
    /// In-memory rolling performance metrics for /api/perf endpoint
    perf_metrics: perf_metrics::PerfMetrics,
    /// Derived stats rows for public leaderboard/profile endpoints.
    leaderboard_cache: Arc<RwLock<LeaderboardCache>>,
}

const GAME_ROOM_NAME: &str = "game_room";

// ============================================================================
// HTTP Handlers - Authentication
// ============================================================================

/// Auth sessions: token -> (account_id, username)
type AuthSessions = Arc<DashMap<String, AuthSession>>;

#[derive(Clone)]
struct AuthSession {
    account_id: i64,
    username: String,
    expires_at: Instant,
}

impl AuthSession {
    fn new(account_id: i64, username: String, ttl: Duration) -> Self {
        Self {
            account_id,
            username,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_valid(&self) -> bool {
        Instant::now() < self.expires_at
    }
}

fn get_auth_session(sessions: &AuthSessions, token: &str) -> Option<AuthSession> {
    let session = sessions.get(token).map(|entry| entry.clone())?;
    if session.is_valid() {
        Some(session)
    } else {
        sessions.remove(token);
        None
    }
}

fn has_valid_auth_session(sessions: &AuthSessions, token: &str) -> bool {
    get_auth_session(sessions, token).is_some()
}

// ============================================================================
// WebSocket Handler
// ============================================================================

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

    let config = Arc::new(
        ServerConfig::from_env()
            .unwrap_or_else(|error| panic!("Invalid server configuration: {error}")),
    );
    let state = AppState::new(log_buffer, config.clone()).await;
    let room_load_start = std::time::Instant::now();
    state.get_or_create_room(GAME_ROOM_NAME).await;
    info!(
        "Preloaded game room in {}ms",
        room_load_start.elapsed().as_millis()
    );

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
            let mut load = perf_metrics::PerfLoad {
                rooms: room_count,
                ..perf_metrics::PerfLoad::default()
            };
            for room in tick_state.rooms.iter() {
                let room_start = std::time::Instant::now();
                let tick_telemetry = room.tick().await;
                let room_ms = room_start.elapsed().as_secs_f64() * 1000.0;
                load.connected_players += tick_telemetry.active_players;
                load.overworld_players += tick_telemetry.overworld_players;
                load.instance_players += tick_telemetry.instance_players;
                load.spectators += tick_telemetry.spectators;
                let slow_context = (room_ms > 50.0).then(|| {
                    format!(
                        "players={} overworld={} instance_players={} spectators={} phases_ms=pre_npc:{} npc_world:{} sync:{} arena:{} chunk_unload:{} prayer:{} farming:{} restock:{}",
                        tick_telemetry.active_players,
                        tick_telemetry.overworld_players,
                        tick_telemetry.instance_players,
                        tick_telemetry.spectators,
                        tick_telemetry.pre_npc_ms,
                        tick_telemetry.npc_world_ms,
                        tick_telemetry.state_sync_ms,
                        tick_telemetry.arena_ms,
                        tick_telemetry.chunk_unload_ms,
                        tick_telemetry.prayer_drain_ms,
                        tick_telemetry.farming_growth_ms,
                        tick_telemetry.restock_ms,
                    )
                });
                tick_state
                    .perf_metrics
                    .record_room_tick(&room.name, room_ms, slow_context);
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
            tick_state.perf_metrics.record_load(load);
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

    // Spawn auto-save loop (every 30 seconds). SQLite has one writer, so writes
    // are intentionally serialized instead of creating a task stampede.
    let save_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await;
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
                if !has_valid_auth_session(&save_state.auth_sessions, &session_data.auth_token) {
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

            // Phase 2: Write snapshots without holding gameplay locks.
            if !snapshots.is_empty() {
                let write_phase_start = std::time::Instant::now();
                let mut saved_count = 0;

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
                    match save_state
                        .db
                        .save_character(character_id, &save_data, played_time_delta)
                        .await
                    {
                        Ok(()) => saved_count += 1,
                        Err(e) => {
                            warn!("Auto-save failed for character {}: {}", character_name, e)
                        }
                    }

                    if let Some(quest_state) = quest_state
                        && let Err(e) = save_state
                            .db
                            .save_character_quest_state(character_id, &quest_state)
                            .await
                    {
                        warn!("Quest auto-save failed for {}: {}", character_name, e);
                    }

                    let _ = save_state
                        .db
                        .save_discovered_recipes(character_id, &discovered_recipes)
                        .await;

                    if let Some(ref slayer) = slayer_state {
                        let _ = save_state
                            .db
                            .save_character_slayer_state(character_id, slayer)
                            .await;
                    }

                    let _ = save_state
                        .db
                        .save_unlocked_spells(character_id, &unlocked_spells)
                        .await;
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
            perf_state.auth_rate_limiter.prune_expired();
            perf_state.matchmake_rate_limiter.prune_expired();
            perf_state
                .auth_sessions
                .retain(|_, session| session.is_valid());
            let perf = perf_state.perf_metrics.snapshot(3, 0);
            info!(
                "[PERF] uptime={}s load(rooms={} players={} overworld={} instance={} spectators={}) tick_loop(p95={}ms p99={}ms max={}ms) room_tick(p95={}ms p99={}ms max={}ms) autosave_total(p95={}ms max={}ms) handler(p95={}ms max={}ms) ws_send(p95={}ms max={}ms) movement(reject_rate={}%, attempts={}, rejected={}, reasons=tile:{}({}%) player:{}({}%) npc:{}({}%) chair:{}({}%) arena:{}({}%), stale_packets={}({}%) seq_gaps={}({}%) input_gaps={}({}%) stale_intent_clears={}({}%)) state_sync(drop_rate={}%, skip_rate={}%, attempts={}, capacity_skips={}, drops={}, full={}({}%), delta={}({}%), fallback_self={}, raw_bytes={}, wire_bytes={}, wire_vs_raw={}%) counters(overruns={} slow_room_ticks={} slow_autosaves={} slow_handlers={} slow_ws_sends={})",
                perf.uptime_seconds,
                perf.current_load.rooms,
                perf.current_load.connected_players,
                perf.current_load.overworld_players,
                perf.current_load.instance_players,
                perf.current_load.spectators,
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

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(config.allowed_origins.clone()))
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ]);

    // Build router
    let mut app = Router::new()
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
        .route("/api/stats/entities", get(stats_entities));

    if config.admin_api_token.is_some() {
        app = app
            .route("/api/perf", get(api_perf))
            .route("/api/logs", get(api_logs));
        info!("Authenticated operational endpoints enabled");
    } else {
        info!("Operational endpoints disabled; set AEVEN_ADMIN_API_TOKEN to enable them");
    }
    let app = app.layer(cors);

    // Graceful shutdown: save all players on Ctrl+C
    let shutdown_state = state.clone();

    let app = app.with_state(state);

    let addr = config.bind_addr;
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
                        &save_data,
                        0, // played_time_delta - skip for shutdown
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

                let _ = shutdown_state
                    .db
                    .save_discovered_recipes(character_id, &discovered_recipes)
                    .await;

                if let Some(ref slayer) = slayer_state {
                    let _ = shutdown_state
                        .db
                        .save_character_slayer_state(character_id, slayer)
                        .await;
                }

                let _ = shutdown_state
                    .db
                    .save_unlocked_spells(character_id, &unlocked_spells)
                    .await;
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
