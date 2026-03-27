use dashmap::DashMap;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

use crate::interior::InstanceType;
use crate::npc::Npc;

/// Tracks an active instance
pub struct Instance {
    pub id: String,
    pub map_id: String,
    pub instance_type: InstanceType,
    pub owner_id: Option<String>, // For private instances
    pub players: RwLock<HashSet<String>>,
    /// NPCs spawned in this instance
    pub npcs: RwLock<HashMap<String, Npc>>,
    /// Whether NPCs have been spawned for this instance
    pub npcs_spawned: RwLock<bool>,
    /// Collision grid for interior walkability (width * height bools, true = blocked)
    pub collision: RwLock<Vec<bool>>,
    /// Interior map width in tiles
    pub map_width: u32,
    /// Interior map height in tiles
    pub map_height: u32,
    /// Optional heightmap for elevation lookups
    pub heightmap: RwLock<Option<Vec<u8>>>,
    /// Whether PVP is enabled in this instance
    pub pvp_enabled: bool,
}

impl Instance {
    pub async fn player_count(&self) -> usize {
        self.players.read().await.len()
    }

    pub async fn add_player(&self, player_id: &str) {
        self.players.write().await.insert(player_id.to_string());
    }

    pub async fn remove_player(&self, player_id: &str) -> usize {
        let mut players = self.players.write().await;
        players.remove(player_id);
        players.len()
    }

    pub async fn has_player(&self, player_id: &str) -> bool {
        self.players.read().await.contains(player_id)
    }

    /// Get all player IDs in this instance
    pub async fn get_player_ids(&self) -> Vec<String> {
        self.players.read().await.iter().cloned().collect()
    }

    /// Spawn NPCs from entity definitions (call once when instance is created)
    pub async fn spawn_npcs(
        &self,
        entities: &[crate::interior::InteriorEntitySpawn],
        entity_registry: &crate::entity::registry::EntityRegistry,
    ) {
        info!(
            "spawn_npcs called for instance {} with {} entity definitions",
            self.id,
            entities.len()
        );

        let mut spawned = self.npcs_spawned.write().await;
        if *spawned {
            info!("NPCs already spawned for instance {}, skipping", self.id);
            return; // Already spawned
        }

        let mut npcs = self.npcs.write().await;
        for (i, spawn) in entities.iter().enumerate() {
            let npc_id = spawn
                .unique_id
                .clone()
                .unwrap_or_else(|| format!("{}_{}", self.id, i));

            if let Some(prototype) = entity_registry.get(&spawn.entity_id) {
                // Use spawn's level if specified, otherwise use prototype's level
                let level = spawn.level.unwrap_or(prototype.stats.level);
                info!(
                    "Spawning {} at ({}, {}) level {} in instance {}",
                    spawn.entity_id, spawn.x, spawn.y, level, self.id
                );
                let npc = Npc::from_prototype(
                    &npc_id,
                    &spawn.entity_id,
                    prototype,
                    spawn.x,
                    spawn.y,
                    level,
                    spawn.facing.as_deref(),
                );
                npcs.insert(npc_id, npc);
            } else {
                tracing::warn!(
                    "Prototype '{}' not found for instance {}",
                    spawn.entity_id,
                    self.id
                );
            }
        }

        *spawned = true;
        info!("Spawned {} NPCs in instance {}", npcs.len(), self.id);
    }

    /// Set collision data for this instance (decoded from base64)
    pub async fn set_collision(&self, collision_bytes: &[u8]) {
        let total = (self.map_width * self.map_height) as usize;
        let mut collision = vec![false; total];
        for (i, blocked) in collision.iter_mut().enumerate() {
            if i / 8 < collision_bytes.len() {
                *blocked = (collision_bytes[i / 8] >> (i % 8)) & 1 == 1;
            }
        }
        *self.collision.write().await = collision;
    }

    /// Check if a tile is walkable in this instance
    pub fn is_walkable_sync(&self, collision: &[bool], x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.map_width as i32 || y >= self.map_height as i32 {
            return false;
        }
        let idx = (y as u32 * self.map_width + x as u32) as usize;
        !collision.get(idx).copied().unwrap_or(true)
    }

    /// Get all NPCs in this instance
    pub async fn get_npcs(&self) -> HashMap<String, Npc> {
        self.npcs.read().await.clone()
    }

    /// Set heightmap data for this instance
    pub async fn set_heightmap(&self, data: Vec<u8>) {
        *self.heightmap.write().await = Some(data);
    }

    /// Get height at a tile position (0 if no heightmap)
    pub fn get_height_at_sync(&self, heightmap: &Option<Vec<u8>>, x: i32, y: i32) -> i32 {
        if x < 0 || y < 0 || x >= self.map_width as i32 || y >= self.map_height as i32 {
            return 0;
        }
        if let Some(hm) = heightmap {
            let idx = (y as u32 * self.map_width + x as u32) as usize;
            hm.get(idx).copied().unwrap_or(0) as i32
        } else {
            0
        }
    }

    /// Get NPC updates for sending to clients
    pub async fn get_npc_updates(&self) -> Vec<crate::npc::NpcUpdate> {
        let npcs = self.npcs.read().await;
        npcs.values()
            .filter(|npc| !npc.hidden)
            .map(|npc| crate::npc::NpcUpdate::from(npc))
            .collect()
    }
}

/// Manages all active instances
pub struct InstanceManager {
    /// Public instances: one per map_id
    pub public_instances: DashMap<String, Arc<Instance>>,
    /// Private instances: keyed by (owner_player_id, map_id)
    pub private_instances: DashMap<(String, String), Arc<Instance>>,
}

impl InstanceManager {
    pub fn new() -> Self {
        Self {
            public_instances: DashMap::new(),
            private_instances: DashMap::new(),
        }
    }

    /// Get or create a public instance for a map
    pub fn get_or_create_public(
        &self,
        map_id: &str,
        width: u32,
        height: u32,
        pvp_enabled: bool,
    ) -> (Arc<Instance>, bool) {
        if let Some(instance) = self.public_instances.get(map_id) {
            return (instance.clone(), false);
        }

        let instance_id = format!("pub_{}", map_id);
        let instance = Arc::new(Instance {
            id: instance_id.clone(),
            map_id: map_id.to_string(),
            instance_type: InstanceType::Public,
            owner_id: None,
            players: RwLock::new(HashSet::new()),
            npcs: RwLock::new(HashMap::new()),
            npcs_spawned: RwLock::new(false),
            collision: RwLock::new(Vec::new()),
            map_width: width,
            map_height: height,
            heightmap: RwLock::new(None),
            pvp_enabled,
        });

        self.public_instances
            .insert(map_id.to_string(), instance.clone());
        info!("Created public instance: {} (pvp={})", instance_id, pvp_enabled);
        (instance, true)
    }

    /// Get or create a private instance for a player
    pub fn get_or_create_private(
        &self,
        map_id: &str,
        owner_id: &str,
        width: u32,
        height: u32,
        pvp_enabled: bool,
    ) -> (Arc<Instance>, bool) {
        let key = (owner_id.to_string(), map_id.to_string());

        if let Some(instance) = self.private_instances.get(&key) {
            return (instance.clone(), false);
        }

        let instance_id = format!("priv_{}_{}", map_id, Uuid::new_v4());
        let instance = Arc::new(Instance {
            id: instance_id.clone(),
            map_id: map_id.to_string(),
            instance_type: InstanceType::Private,
            owner_id: Some(owner_id.to_string()),
            players: RwLock::new(HashSet::new()),
            npcs: RwLock::new(HashMap::new()),
            npcs_spawned: RwLock::new(false),
            collision: RwLock::new(Vec::new()),
            map_width: width,
            map_height: height,
            heightmap: RwLock::new(None),
            pvp_enabled,
        });

        self.private_instances.insert(key, instance.clone());
        info!(
            "Created private instance: {} for owner {} (pvp={})",
            instance_id, owner_id, pvp_enabled
        );
        (instance, true)
    }

    /// Remove a private instance (called when empty)
    pub fn remove_private(&self, owner_id: &str, map_id: &str) {
        let key = (owner_id.to_string(), map_id.to_string());
        if self.private_instances.remove(&key).is_some() {
            info!("Removed private instance for {} / {}", owner_id, map_id);
        }
    }

    /// Find which instance a player is in by checking all instances
    pub async fn find_player_instance(&self, player_id: &str) -> Option<Arc<Instance>> {
        for entry in self.public_instances.iter() {
            if entry.value().has_player(player_id).await {
                return Some(entry.value().clone());
            }
        }
        for entry in self.private_instances.iter() {
            if entry.value().has_player(player_id).await {
                return Some(entry.value().clone());
            }
        }
        None
    }

    /// Get an instance by its instance ID (not map_id)
    pub fn get_by_instance_id(&self, instance_id: &str) -> Option<Arc<Instance>> {
        // Check public instances
        for entry in self.public_instances.iter() {
            if entry.value().id == instance_id {
                return Some(entry.value().clone());
            }
        }
        // Check private instances
        for entry in self.private_instances.iter() {
            if entry.value().id == instance_id {
                return Some(entry.value().clone());
            }
        }
        None
    }
}

impl Default for InstanceManager {
    fn default() -> Self {
        Self::new()
    }
}
