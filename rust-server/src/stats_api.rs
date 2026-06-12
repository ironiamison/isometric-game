use super::*;

// ============================================================================
// Stats API Handlers (public, read-only)
// ============================================================================

#[derive(Serialize)]
struct StatsOverview {
    online_players: usize,
    total_characters: i64,
    total_accounts: i64,
}

pub(super) async fn stats_overview(State(state): State<AppState>) -> impl IntoResponse {
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

pub(super) async fn stats_online(State(state): State<AppState>) -> impl IntoResponse {
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
pub(super) struct LeaderboardQuery {
    #[serde(default = "default_leaderboard_sort")]
    sort: String,
    #[serde(default = "default_leaderboard_limit")]
    limit: usize,
}

pub(super) fn default_leaderboard_sort() -> String {
    "total_level".to_string()
}
pub(super) fn default_leaderboard_limit() -> usize {
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
    survivalist_level: i32,
    total_level: i32,
    played_time: i64,
    monster_kills: i32,
}

#[derive(Default)]
pub(super) struct LeaderboardCache {
    entries: Vec<LeaderboardEntry>,
    refreshed_at: Option<Instant>,
}

const LEADERBOARD_CACHE_TTL: Duration = Duration::from_secs(10);

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
    survivalist_level: usize,
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
    {
        let cache = state.leaderboard_cache.read().await;
        if cache
            .refreshed_at
            .is_some_and(|refreshed_at| refreshed_at.elapsed() < LEADERBOARD_CACHE_TTL)
        {
            return cache.entries.clone();
        }
    }

    let rows = sqlx::query("SELECT name, skills_json, played_time, monster_kills FROM characters WHERE is_admin = FALSE")
        .fetch_all(state.db.pool())
        .await
        .unwrap_or_default();

    let entries: Vec<LeaderboardEntry> = rows
        .into_iter()
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
                survivalist_level: skills.survivalist.level,
                total_level: skills.total_level(),
                played_time,
                monster_kills,
            })
        })
        .collect();

    let mut cache = state.leaderboard_cache.write().await;
    cache.entries = entries.clone();
    cache.refreshed_at = Some(Instant::now());
    entries
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
        "survivalist_level" => {
            entries.sort_by(|a, b| b.survivalist_level.cmp(&a.survivalist_level))
        }
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

pub(super) async fn stats_leaderboard(
    State(state): State<AppState>,
    Query(query): Query<LeaderboardQuery>,
) -> impl IntoResponse {
    let mut entries = load_leaderboard_entries(&state).await;
    sort_leaderboard_entries(&mut entries, &query.sort);

    entries.truncate(query.limit.min(100));
    Json(entries)
}

pub(super) async fn stats_player_profile(
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
        survivalist_level: stat_rank(&entries, &player, |entry| entry.survivalist_level as i64),
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

pub(super) async fn stats_items(State(state): State<AppState>) -> impl IntoResponse {
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

pub(super) async fn stats_entities(State(state): State<AppState>) -> impl IntoResponse {
    // Collect quest kill-objective targets
    let all_quests = state.quest_registry.all_quests().await;
    let mut quest_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for quest in &all_quests {
        for obj in &quest.objectives {
            if obj.objective_type == ObjectiveType::KillMonster {
                quest_map
                    .entry(obj.target.clone())
                    .or_default()
                    .push(quest.id.clone());
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
            loot: e
                .loot
                .iter()
                .map(|l| StatsEntityLoot {
                    item_id: l.item_id.clone(),
                    drop_chance: l.drop_chance,
                    quantity_min: l.quantity_min,
                    quantity_max: l.quantity_max,
                })
                .collect(),
            loot_tables: e
                .loot_tables
                .iter()
                .map(|t| StatsLootTable {
                    name: t.name.clone(),
                    chance: t.chance,
                    entries: t
                        .entries
                        .iter()
                        .map(|e| StatsLootTableEntry {
                            item_id: e.item_id.clone(),
                            weight: e.weight,
                            quantity_min: e.quantity_min,
                            quantity_max: e.quantity_max,
                        })
                        .collect(),
                })
                .collect(),
            quest_ids: quest_map.get(&e.id).cloned().unwrap_or_default(),
        })
        .collect();
    Json(entities)
}

pub(super) async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().timestamp_millis()
    }))
}

// ============================================================================
// Server Logs
// ============================================================================

#[derive(Deserialize)]
pub(super) struct LogsQuery {
    count: Option<usize>,
    level: Option<String>,
    important: Option<bool>,
}

#[derive(Deserialize)]
pub(super) struct PerfQuery {
    rooms: Option<usize>,
    spikes: Option<usize>,
}

pub(super) async fn api_logs(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(params): Query<LogsQuery>,
) -> axum::response::Response {
    if !is_admin_request(&state, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
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

    Json(entries).into_response()
}

pub(super) async fn api_perf(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(params): Query<PerfQuery>,
) -> axum::response::Response {
    if !is_admin_request(&state, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let top_rooms = params.rooms.unwrap_or(10).clamp(1, 50);
    let recent_spikes = params.spikes.unwrap_or(50).min(200);
    Json(state.perf_metrics.snapshot(top_rooms, recent_spikes)).into_response()
}

fn is_admin_request(state: &AppState, headers: &axum::http::HeaderMap) -> bool {
    let Some(expected) = state.config.admin_api_token.as_deref() else {
        return false;
    };
    let Some(provided) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
    else {
        return false;
    };
    constant_time_eq(expected.as_bytes(), provided.as_bytes())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}
