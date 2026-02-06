use std::collections::HashMap;
use std::path::Path;
use rand::Rng;
use serde::Deserialize;
use tracing::info;
use crate::chunk::CHUNK_SIZE;

// ---------------------------------------------------------------------------
// TOML deserialization structures
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, Clone)]
pub struct GatheringZoneConfig {
    pub skill: String,
    pub level_required: i32,
    pub loot_table: String,
    pub bonus_spawn_frequency: u64,
    pub base_gather_speed: f32,
    pub base_xp: i64,
}

#[derive(Deserialize, Debug, Clone)]
struct GatheringZonesFile {
    zones: HashMap<String, GatheringZoneConfig>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LootTable {
    pub skill: String,
    pub tiers: HashMap<String, LootTier>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LootTier {
    pub base_weight: f32,
    pub level_scaling: f32,
    pub items: Vec<LootItem>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LootItem {
    pub id: String,
    pub level: i32,
    pub weight: f32,
    pub xp_bonus: i64,
}

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GatheringMarker {
    pub x: i32,
    pub y: i32,
    pub zone_id: String,
}

#[derive(Debug, Clone)]
pub struct PlayerGatheringState {
    pub zone_id: String,
    pub marker_x: i32,
    pub marker_y: i32,
    pub last_gather_tick: u64,
    pub buff_expires_at: u64,
}

#[derive(Debug, Clone)]
pub struct BonusTile {
    pub x: i32,
    pub y: i32,
    pub zone_id: String,
    pub spawn_time: u64,
    pub telegraph_duration: u64,
}

#[derive(Debug, Clone)]
pub struct GatherResult {
    pub item_id: String,
    pub xp_gained: i64,
}

#[derive(Debug, Clone)]
pub enum BonusTileEvent {
    Spawned { x: i32, y: i32, zone_id: String },
    Expired { x: i32, y: i32 },
}

// ---------------------------------------------------------------------------
// GatheringSystem
// ---------------------------------------------------------------------------

pub struct GatheringSystem {
    pub zones: HashMap<String, GatheringZoneConfig>,
    pub loot_tables: HashMap<String, LootTable>,
    pub markers: Vec<GatheringMarker>,
    pub occupied_markers: HashMap<(i32, i32), String>,
    pub player_states: HashMap<String, PlayerGatheringState>,
    pub bonus_tiles: Vec<BonusTile>,
    pub last_bonus_check: HashMap<String, u64>,
}

// ---------------------------------------------------------------------------
// Loading functions
// ---------------------------------------------------------------------------

pub fn load_zones(path: &Path) -> Result<HashMap<String, GatheringZoneConfig>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read zones file {:?}: {}", path, e))?;
    let file: GatheringZonesFile = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse zones TOML: {}", e))?;
    info!("Loaded {} gathering zones", file.zones.len());
    Ok(file.zones)
}

pub fn load_loot_tables(path: &Path) -> Result<HashMap<String, LootTable>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read loot tables file {:?}: {}", path, e))?;
    let tables: HashMap<String, LootTable> = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse loot tables TOML: {}", e))?;
    info!("Loaded {} loot tables", tables.len());
    Ok(tables)
}

#[derive(Deserialize, Debug)]
struct GatheringMarkersFile {
    markers: Vec<GatheringMarkerConfig>,
}

#[derive(Deserialize, Debug)]
struct GatheringMarkerConfig {
    x: i32,
    y: i32,
    zone_id: String,
}

pub fn load_markers(path: &Path) -> Result<Vec<GatheringMarker>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read markers file {:?}: {}", path, e))?;
    let file: GatheringMarkersFile = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse markers TOML: {}", e))?;
    let markers: Vec<GatheringMarker> = file.markers.into_iter().map(|m| GatheringMarker {
        x: m.x,
        y: m.y,
        zone_id: m.zone_id,
    }).collect();
    info!("Loaded {} gathering markers", markers.len());
    Ok(markers)
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl GatheringSystem {
    pub fn new() -> Self {
        Self {
            zones: HashMap::new(),
            loot_tables: HashMap::new(),
            markers: Vec::new(),
            occupied_markers: HashMap::new(),
            player_states: HashMap::new(),
            bonus_tiles: Vec::new(),
            last_bonus_check: HashMap::new(),
        }
    }

    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let zones = load_zones(&data_dir.join("gathering_zones.toml"))?;
        let loot_tables = load_loot_tables(&data_dir.join("loot_tables.toml"))?;
        let markers = match load_markers(&data_dir.join("gathering_markers.toml")) {
            Ok(m) => m,
            Err(e) => {
                info!("No gathering markers loaded: {}", e);
                Vec::new()
            }
        };
        Ok(Self {
            zones,
            loot_tables,
            markers,
            occupied_markers: HashMap::new(),
            player_states: HashMap::new(),
            bonus_tiles: Vec::new(),
            last_bonus_check: HashMap::new(),
        })
    }

    pub fn start_gathering(
        &mut self,
        player_id: &str,
        marker_x: i32,
        marker_y: i32,
        player_level: i32,
        current_time: u64,
    ) -> Result<String, String> {
        // Find the marker at the given position
        let marker = self
            .markers
            .iter()
            .find(|m| m.x == marker_x && m.y == marker_y)
            .ok_or_else(|| format!("No gathering marker at ({}, {})", marker_x, marker_y))?;

        let zone_id = marker.zone_id.clone();

        // Check if marker is occupied
        if self.occupied_markers.contains_key(&(marker_x, marker_y)) {
            return Err("Marker is already occupied".to_string());
        }

        // Check level requirement
        let zone = self
            .zones
            .get(&zone_id)
            .ok_or_else(|| format!("Zone '{}' not found in config", zone_id))?;

        if player_level < zone.level_required {
            return Err(format!(
                "Requires level {} (you are level {})",
                zone.level_required, player_level
            ));
        }

        // Set occupied and player state
        self.occupied_markers
            .insert((marker_x, marker_y), player_id.to_string());

        self.player_states.insert(
            player_id.to_string(),
            PlayerGatheringState {
                zone_id: zone_id.clone(),
                marker_x,
                marker_y,
                last_gather_tick: current_time,
                buff_expires_at: 0,
            },
        );

        info!(
            "Player {} started gathering at ({}, {}) in zone '{}'",
            player_id, marker_x, marker_y, zone_id
        );

        Ok(zone_id)
    }

    pub fn stop_gathering(&mut self, player_id: &str) -> Option<PlayerGatheringState> {
        let state = self.player_states.remove(player_id)?;
        self.occupied_markers
            .remove(&(state.marker_x, state.marker_y));
        info!("Player {} stopped gathering", player_id);
        Some(state)
    }

    pub fn is_gathering(&self, player_id: &str) -> bool {
        self.player_states.contains_key(player_id)
    }

    /// Returns the set of player IDs currently gathering (for state sync)
    pub fn gathering_player_ids(&self) -> std::collections::HashSet<String> {
        self.player_states.keys().cloned().collect()
    }

    pub fn tick_gathering(
        &mut self,
        player_id: &str,
        player_fishing_level: i32,
        current_time: u64,
        prayer_speed_multiplier: f32,
    ) -> Option<GatherResult> {
        let state = self.player_states.get(player_id)?;
        let zone = self.zones.get(&state.zone_id)?;

        // Determine gather speed (2x if buffed, plus prayer bonus)
        let has_buff = state.buff_expires_at > 0 && current_time < state.buff_expires_at;
        let buff_multiplier = if has_buff { 2.0 } else { 1.0 };
        // Combined multiplier: buff and prayer stack multiplicatively
        let total_multiplier = buff_multiplier * prayer_speed_multiplier;
        let gather_speed_ms = (zone.base_gather_speed * 1000.0 / total_multiplier) as u64;

        // Check if enough time has elapsed
        if current_time < state.last_gather_tick + gather_speed_ms {
            return None;
        }

        // Update last tick
        let loot_table_id = zone.loot_table.clone();
        let base_xp = zone.base_xp;

        let state_mut = self.player_states.get_mut(player_id)?;
        state_mut.last_gather_tick = current_time;

        // Roll loot
        if let Some((item_id, xp_bonus)) = self.roll_loot(&loot_table_id, player_fishing_level) {
            let xp_gained = base_xp + xp_bonus;
            info!(
                "Player {} gathered '{}' for {} xp",
                player_id, item_id, xp_gained
            );
            Some(GatherResult { item_id, xp_gained })
        } else {
            // Fallback if loot roll fails - still grant base xp with no item
            None
        }
    }

    pub fn roll_loot(&self, table_id: &str, player_level: i32) -> Option<(String, i64)> {
        let table = self.loot_tables.get(table_id)?;
        let mut rng = rand::thread_rng();

        // Calculate effective tier weights based on player level
        let tier_weights: Vec<(&String, &LootTier, f32)> = table
            .tiers
            .iter()
            .map(|(name, tier)| {
                let effective_weight =
                    (tier.base_weight + tier.level_scaling * player_level as f32).max(0.0);
                (name, tier, effective_weight)
            })
            .collect();

        let total_tier_weight: f32 = tier_weights.iter().map(|(_, _, w)| w).sum();
        if total_tier_weight <= 0.0 {
            return None;
        }

        // Roll for tier
        let mut roll = rng.gen_range(0.0..total_tier_weight);
        let mut selected_tier: Option<&LootTier> = None;
        for (_, tier, weight) in &tier_weights {
            roll -= weight;
            if roll <= 0.0 {
                selected_tier = Some(tier);
                break;
            }
        }
        let tier = selected_tier?;

        // Filter items by level requirement
        let eligible_items: Vec<&LootItem> = tier
            .items
            .iter()
            .filter(|item| player_level >= item.level)
            .collect();

        if eligible_items.is_empty() {
            return None;
        }

        // Roll for item within tier
        let total_item_weight: f32 = eligible_items.iter().map(|i| i.weight).sum();
        if total_item_weight <= 0.0 {
            return None;
        }

        let mut item_roll = rng.gen_range(0.0..total_item_weight);
        for item in &eligible_items {
            item_roll -= item.weight;
            if item_roll <= 0.0 {
                return Some((item.id.clone(), item.xp_bonus));
            }
        }

        // Fallback to last eligible item
        eligible_items
            .last()
            .map(|item| (item.id.clone(), item.xp_bonus))
    }

    pub fn add_marker(&mut self, marker: GatheringMarker) {
        info!(
            "Added gathering marker at ({}, {}) for zone '{}'",
            marker.x, marker.y, marker.zone_id
        );
        self.markers.push(marker);
    }

    pub fn tick_bonus_tiles(&mut self, current_time: u64) -> Vec<BonusTileEvent> {
        let mut events = Vec::new();

        // Expire old bonus tiles (telegraph_duration has elapsed)
        let mut i = 0;
        while i < self.bonus_tiles.len() {
            let tile = &self.bonus_tiles[i];
            if current_time >= tile.spawn_time + tile.telegraph_duration {
                let expired = self.bonus_tiles.remove(i);
                events.push(BonusTileEvent::Expired {
                    x: expired.x,
                    y: expired.y,
                });
            } else {
                i += 1;
            }
        }

        // Spawn new bonus tiles per zone frequency
        let mut rng = rand::thread_rng();
        let zone_ids: Vec<String> = self.zones.keys().cloned().collect();

        for zone_id in &zone_ids {
            let zone = match self.zones.get(zone_id) {
                Some(z) => z,
                None => continue,
            };

            let freq_ms = zone.bonus_spawn_frequency * 1000;
            let last_check = self.last_bonus_check.get(zone_id).copied().unwrap_or(0);

            if current_time < last_check + freq_ms {
                continue;
            }

            self.last_bonus_check
                .insert(zone_id.clone(), current_time);

            // Pick a random marker in this zone to spawn a bonus tile near
            // Only spawn bonus tiles in chunk 0,0 of the overworld
            let zone_markers: Vec<&GatheringMarker> = self
                .markers
                .iter()
                .filter(|m| {
                    m.zone_id == *zone_id
                        && m.x >= 0
                        && m.x < CHUNK_SIZE as i32
                        && m.y >= 0
                        && m.y < CHUNK_SIZE as i32
                })
                .collect();

            if zone_markers.is_empty() {
                continue;
            }

            let marker = zone_markers[rng.gen_range(0..zone_markers.len())];
            // Spawn bonus tile at a small random offset from the marker
            let offset_x = rng.gen_range(-2..=2);
            let offset_y = rng.gen_range(-2..=2);
            let bx = marker.x + offset_x;
            let by = marker.y + offset_y;

            let bonus = BonusTile {
                x: bx,
                y: by,
                zone_id: zone_id.clone(),
                spawn_time: current_time,
                telegraph_duration: 5000,
            };

            info!(
                "Bonus tile spawned at ({}, {}) in zone '{}'",
                bx, by, zone_id
            );

            events.push(BonusTileEvent::Spawned {
                x: bx,
                y: by,
                zone_id: zone_id.clone(),
            });

            self.bonus_tiles.push(bonus);
        }

        events
    }

    pub fn claim_bonus_tile(
        &mut self,
        player_id: &str,
        x: i32,
        y: i32,
        current_time: u64,
    ) -> bool {
        // Find and remove the bonus tile at (x, y)
        let idx = self.bonus_tiles.iter().position(|t| t.x == x && t.y == y);
        let idx = match idx {
            Some(i) => i,
            None => return false,
        };

        self.bonus_tiles.remove(idx);

        // Apply 2x gather speed buff for 30 seconds
        if let Some(state) = self.player_states.get_mut(player_id) {
            state.buff_expires_at = current_time + 30_000;
            info!(
                "Player {} claimed bonus tile at ({}, {}), buff until {}",
                player_id,
                x,
                y,
                state.buff_expires_at
            );
            true
        } else {
            false
        }
    }
}
