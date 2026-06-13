use super::*;
use tokio::sync::broadcast;

impl GameRoom {
    fn is_pvp_zone(&self, world_x: i32, world_y: i32) -> bool {
        let chunk = ChunkCoord::from_world(world_x, world_y);
        self.pvp_zones.contains(&(chunk.x, chunk.y))
    }

    pub(super) async fn is_pvp_allowed(&self, player_id: &str, world_x: i32, world_y: i32) -> bool {
        let instances = self.player_instances.read().await;
        let instance_id = instances.get(player_id).cloned();
        drop(instances);
        match instance_id {
            Some(inst_id) => {
                // In an instance — check instance pvp flag
                if let Some(inst) = self.instance_manager.get_by_instance_id(&inst_id) {
                    inst.pvp_enabled
                } else {
                    false
                }
            }
            None => {
                // Overworld — check pvp zone allowlist
                self.is_pvp_zone(world_x, world_y)
            }
        }
    }

    pub async fn init_top_level_player(&self) {
        if let Some(ref db) = self.db {
            let (first, second) = db.get_top_total_level_players().await;
            if let Some((name, total)) = first {
                tracing::info!("Top total level player: {} (level {})", name, total);
                *self.top_level_player_name.write().await = Some(name);
                *self.top_level_value.write().await = total;
            } else {
                tracing::info!("No characters found for top level player");
            }
            if let Some((name, total)) = second {
                tracing::info!("2nd total level player: {} (level {})", name, total);
                *self.second_level_player_name.write().await = Some(name);
                *self.second_level_value.write().await = total;
            }
        }
    }

    pub async fn check_top_player_after_level_up(&self, player_name: &str, new_total_level: i32) {
        let current_top = *self.top_level_value.read().await;
        let current_second = *self.second_level_value.read().await;
        let is_current_top =
            self.top_level_player_name.read().await.as_deref() == Some(player_name);
        let is_current_second =
            self.second_level_player_name.read().await.as_deref() == Some(player_name);

        let mut changed = false;

        if new_total_level > current_top {
            // New #1 — old #1 becomes #2 (unless it's the same player updating their own score)
            if !is_current_top {
                let old_first_name = self.top_level_player_name.read().await.clone();
                let old_first_val = current_top;
                *self.second_level_player_name.write().await = old_first_name;
                *self.second_level_value.write().await = old_first_val;
            }
            *self.top_level_player_name.write().await = Some(player_name.to_string());
            *self.top_level_value.write().await = new_total_level;
            changed = true;
        } else if is_current_top {
            // Current #1 leveled up but still #1 — just update value
            *self.top_level_value.write().await = new_total_level;
        } else if new_total_level > current_second {
            // New #2
            *self.second_level_player_name.write().await = Some(player_name.to_string());
            *self.second_level_value.write().await = new_total_level;
            changed = true;
        } else if is_current_second {
            // Current #2 leveled up but still #2 — just update value
            *self.second_level_value.write().await = new_total_level;
        }

        if changed {
            let first = self.top_level_player_name.read().await.clone();
            let second = self.second_level_player_name.read().await.clone();
            self.broadcast(ServerMessage::TopPlayerChanged {
                player_name: first,
                second_player_name: second,
            })
            .await;
        }
    }

    pub async fn broadcast_skill_level_up(&self, player_id: &str, skill: &str, new_level: i32) {
        self.broadcast(ServerMessage::SkillLevelUp {
            player_id: player_id.to_string(),
            skill: skill.to_string(),
            new_level,
        })
        .await;

        // Check if this level-up changes the rankings (skip admins — they're excluded from rankings)
        let players = self.players.read().await;
        if let Some(player) = players.get(player_id)
            && !player.is_admin
        {
            let name = player.name.clone();
            let total = player.skills.total_level();
            drop(players);
            self.check_top_player_after_level_up(&name, total).await;
        }
    }

    pub async fn get_top_player_message(&self) -> ServerMessage {
        let first = self.top_level_player_name.read().await.clone();
        let second = self.second_level_player_name.read().await.clone();
        ServerMessage::TopPlayerChanged {
            player_name: first,
            second_player_name: second,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        self.transport.subscribe()
    }

    pub async fn add_spectator(&self, spectator_id: &str, sender: mpsc::Sender<Vec<u8>>) {
        self.transport.add_spectator(spectator_id, sender).await;
    }

    pub async fn remove_spectator(&self, spectator_id: &str) {
        self.transport.remove_spectator(spectator_id).await;
    }

    pub async fn spectator_count(&self) -> usize {
        self.transport.spectator_count().await
    }

    pub async fn broadcast(&self, msg: ServerMessage) {
        if let Ok(bytes) = crate::protocol::encode_server_message(&msg) {
            self.transport.broadcast(bytes);
        }
    }

    /// Broadcast a positional event to players who can currently see the source player.
    pub async fn broadcast_to_zone(&self, source_player_id: &str, msg: ServerMessage) {
        self.broadcast_to_zone_except(source_player_id, msg, None)
            .await;
    }

    pub async fn broadcast_to_zone_except(
        &self,
        source_player_id: &str,
        msg: ServerMessage,
        exclude: Option<&str>,
    ) {
        let (source_x, source_y) = {
            let players = self.players.read().await;
            let Some(source) = players.get(source_player_id) else {
                return;
            };
            (source.x, source.y)
        };
        let source_instance = self
            .player_instances
            .read()
            .await
            .get(source_player_id)
            .cloned();
        let senders = self.transport.player_senders().await;
        let recipients: Vec<(String, mpsc::Sender<Vec<u8>>)> = {
            let player_instances = self.player_instances.read().await;
            let players = self.players.read().await;
            senders
                .iter()
                .filter(|(player_id, _)| {
                    exclude != Some(player_id.as_str())
                        && players.get(*player_id).is_some_and(|player| {
                            player.active
                                && is_visible_event_recipient(
                                    source_instance.as_deref(),
                                    source_x,
                                    source_y,
                                    player_instances.get(*player_id).map(String::as_str),
                                    player.x,
                                    player.y,
                                )
                        })
                })
                .map(|(player_id, sender)| (player_id.clone(), sender.clone()))
                .collect()
        };

        if let Ok(bytes) = crate::protocol::encode_server_message(&msg) {
            for (_, sender) in &recipients {
                let _ = sender.try_send(bytes.clone());
            }
        }
        self.record_ground_item_visibility(
            &msg,
            recipients.iter().map(|(player_id, _)| player_id.as_str()),
        )
        .await;
    }

    /// Broadcast a positional event around coordinates in an instance or the overworld.
    pub async fn broadcast_to_area(
        &self,
        instance_id: Option<&str>,
        source_x: i32,
        source_y: i32,
        msg: ServerMessage,
    ) {
        let senders = self.transport.player_senders().await;
        let recipients: Vec<(String, mpsc::Sender<Vec<u8>>)> = {
            let player_instances = self.player_instances.read().await;
            let players = self.players.read().await;
            senders
                .iter()
                .filter(|(player_id, _)| {
                    players.get(*player_id).is_some_and(|player| {
                        player.active
                            && is_visible_event_recipient(
                                instance_id,
                                source_x,
                                source_y,
                                player_instances.get(*player_id).map(String::as_str),
                                player.x,
                                player.y,
                            )
                    })
                })
                .map(|(player_id, sender)| (player_id.clone(), sender.clone()))
                .collect()
        };

        if let Ok(bytes) = crate::protocol::encode_server_message(&msg) {
            for (_, sender) in &recipients {
                let _ = sender.try_send(bytes.clone());
            }
        }
        self.record_ground_item_visibility(
            &msg,
            recipients.iter().map(|(player_id, _)| player_id.as_str()),
        )
        .await;
    }

    async fn record_ground_item_visibility<'a>(
        &self,
        msg: &ServerMessage,
        recipients: impl Iterator<Item = &'a str>,
    ) {
        let (item_id, visible) = match msg {
            ServerMessage::ItemDropped { id, .. } => (id, true),
            ServerMessage::ItemPickedUp { item_id, .. }
            | ServerMessage::ItemDespawned { item_id } => (item_id, false),
            _ => return,
        };
        let mut visibility = self.visible_ground_items.write().await;
        for player_id in recipients {
            let known = visibility.entry(player_id.to_string()).or_default();
            if visible {
                known.insert(item_id.clone());
            } else {
                known.remove(item_id);
            }
        }
    }

    pub async fn broadcast_to_zone_by_instance(
        &self,
        instance_id: Option<&str>,
        msg: ServerMessage,
    ) {
        let senders = self.transport.player_senders().await;
        let recipients: Vec<mpsc::Sender<Vec<u8>>> = {
            let player_instances = self.player_instances.read().await;
            senders
                .iter()
                .filter(|(player_id, _)| {
                    player_instances.get(*player_id).map(String::as_str) == instance_id
                })
                .map(|(_, sender)| sender.clone())
                .collect()
        };

        if let Ok(bytes) = crate::protocol::encode_server_message(&msg) {
            for sender in recipients {
                let _ = sender.try_send(bytes.clone());
            }
        }
    }

    pub async fn send_to_overworld_players(&self, msg: ServerMessage, exclude: Option<&str>) {
        let positional_source = match &msg {
            ServerMessage::PlayerJoined { id, .. } | ServerMessage::PlayerLeft { id } => {
                Some(id.clone())
            }
            _ => None,
        };
        if let Some(source_player_id) = positional_source
            && self.players.read().await.contains_key(&source_player_id)
        {
            self.broadcast_to_zone_except(&source_player_id, msg, exclude)
                .await;
            return;
        }

        let senders = self.transport.player_senders().await;
        let recipients: Vec<mpsc::Sender<Vec<u8>>> = {
            let player_instances = self.player_instances.read().await;
            senders
                .iter()
                .filter(|(player_id, _)| {
                    exclude != Some(player_id.as_str())
                        && !player_instances.contains_key(*player_id)
                })
                .map(|(_, sender)| sender.clone())
                .collect()
        };

        if let Ok(bytes) = crate::protocol::encode_server_message(&msg) {
            for sender in recipients {
                let _ = sender.try_send(bytes.clone());
            }
        }
    }

    pub async fn register_player_sender(&self, player_id: &str, sender: mpsc::Sender<Vec<u8>>) {
        self.transport.register_player(player_id, sender).await;
        tracing::debug!("Registered sender for player {}", player_id);
    }

    pub async fn unregister_player_sender(&self, player_id: &str) {
        self.transport.unregister_player(player_id).await;
        tracing::debug!("Unregistered sender for player {}", player_id);
    }

    pub async fn reset_sync_state(&self, player_id: &str) {
        self.transport.reset_sync_state(player_id);
    }

    pub async fn find_portal_at_player(&self, player_id: &str) -> Option<crate::chunk::Portal> {
        use crate::chunk::CHUNK_SIZE;
        use tracing::{debug, trace};

        let (player_x, player_y) = {
            let players = self.players.read().await;
            let player = players.get(player_id)?;
            (player.x, player.y)
        };
        let coord = ChunkCoord::from_world(player_x, player_y);

        debug!(
            "Looking for portal at player {} position ({}, {}), chunk ({}, {})",
            player_id, player_x, player_y, coord.x, coord.y
        );

        let chunk = self.world.get_or_load_chunk(coord).await?;

        debug!("Chunk has {} portals", chunk.portals.len());

        // Portal coordinates in chunk JSON are LOCAL (0-31), need to convert to WORLD coords
        let chunk_base_x = coord.x * CHUNK_SIZE as i32;
        let chunk_base_y = coord.y * CHUNK_SIZE as i32;

        for p in &chunk.portals {
            let world_x = chunk_base_x + p.x;
            let world_y = chunk_base_y + p.y;
            trace!(
                "Portal '{}' at local ({}, {}) -> world ({}, {}) to ({}, {}), target: {}",
                p.id,
                p.x,
                p.y,
                world_x,
                world_y,
                world_x + p.width,
                world_y + p.height,
                p.target_map
            );
        }

        chunk
            .portals
            .iter()
            .find(|p| {
                let world_x = chunk_base_x + p.x;
                let world_y = chunk_base_y + p.y;
                let in_portal = player_x >= world_x
                    && player_x < world_x + p.width
                    && player_y >= world_y
                    && player_y < world_y + p.height;
                if in_portal {
                    debug!("Player {} is inside portal '{}'", player_id, p.id);
                }
                in_portal
            })
            .cloned()
    }

    pub async fn send_to_player(&self, player_id: &str, msg: ServerMessage) {
        use crate::protocol::encode_server_message;

        match &msg {
            ServerMessage::ShowDialogue {
                quest_id,
                npc_id,
                choices,
                ..
            } => {
                // A no-choice dialogue's only valid client action is "Continue", which the
                // client sends as the synthetic "__continue__" choice. Grant it explicitly,
                // otherwise continue-style dialogues (e.g. quest completion) fail authorization.
                let allowed = if choices.is_empty() {
                    std::iter::once("__continue__".to_string()).collect()
                } else {
                    choices.iter().map(|choice| choice.id.clone()).collect()
                };
                self.register_dialogue_grant(player_id, quest_id, npc_id, allowed)
                    .await;
            }
            ServerMessage::AdventureBoardState {
                npc_id,
                offers,
                active_contract,
                crafting_orders,
                crafting_order_active,
                ..
            } => {
                // The board is a bespoke panel rather than a ShowDialogue, so derive its valid
                // choice set explicitly — otherwise every board action fails authorization.
                let allowed = super::resource_contracts::adventure_board_choice_ids(
                    offers,
                    active_contract,
                    crafting_orders,
                    crafting_order_active,
                );
                let quest_id = format!("adventure_board:{}", npc_id);
                self.register_dialogue_grant(player_id, &quest_id, npc_id, allowed)
                    .await;
            }
            ServerMessage::DialogueClosed => {
                self.dialogue_grants.write().await.remove(player_id);
            }
            _ => {}
        }

        let sender = self.transport.player_sender(player_id).await;
        if let Some(sender) = sender {
            if let Ok(bytes) = encode_server_message(&msg)
                && let Err(e) = sender.try_send(bytes)
            {
                match e {
                    tokio::sync::mpsc::error::TrySendError::Full(_) => {
                        tracing::debug!("Unicast queue full for {}; dropping message", player_id);
                    }
                    tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                        tracing::warn!("Failed to send unicast to {}: channel closed", player_id);
                    }
                }
            }
        } else {
            tracing::debug!("No sender registered for player {}", player_id);
        }
    }

    async fn npc_context_for_player(
        &self,
        player_id: &str,
        npc_id: &str,
    ) -> Option<(String, f32, bool, Option<String>)> {
        let (player_x, player_y) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) if player.active && !player.is_dead => (player.x, player.y),
                _ => return None,
            }
        };
        let instance_id = self.player_instances.read().await.get(player_id).cloned();

        let npc_data = if let Some(instance_id) = instance_id.as_ref() {
            let instance = self.instance_manager.get_by_instance_id(instance_id)?;
            let npcs = instance.npcs.read().await;
            npcs.get(npc_id)
                .map(|npc| (npc.prototype_id.clone(), npc.x, npc.y, npc.is_alive()))
        } else {
            let npcs = self.npcs.read().await;
            npcs.get(npc_id)
                .map(|npc| (npc.prototype_id.clone(), npc.x, npc.y, npc.is_alive()))
        }?;

        let dx = (npc_data.1 - player_x) as f32;
        let dy = (npc_data.2 - player_y) as f32;
        Some((
            npc_data.0,
            (dx * dx + dy * dy).sqrt(),
            npc_data.3,
            instance_id,
        ))
    }

    async fn validate_npc_grant(
        &self,
        player_id: &str,
        grant: &NpcInteractionGrant,
        max_distance: f32,
    ) -> Option<String> {
        let (prototype_id, distance, alive, instance_id) = self
            .npc_context_for_player(player_id, &grant.npc_id)
            .await?;
        if instance_id != grant.instance_id || distance > max_distance || !alive {
            return None;
        }
        Some(prototype_id)
    }

    pub(in crate::game) async fn validate_active_npc_interaction(
        &self,
        player_id: &str,
        max_distance: f32,
    ) -> Option<(String, String)> {
        let grant = self
            .npc_interaction_grants
            .read()
            .await
            .get(player_id)
            .cloned()?;
        let prototype_id = self
            .validate_npc_grant(player_id, &grant, max_distance)
            .await?;
        Some((grant.npc_id, prototype_id))
    }

    /// Record the set of dialogue choices a player is allowed to send back for the panel we just
    /// sent them, carrying over the originating NPC interaction so proximity can be re-validated
    /// when the choice arrives. Shared by `ShowDialogue` and the bespoke `AdventureBoardState`.
    async fn register_dialogue_grant(
        &self,
        player_id: &str,
        quest_id: &str,
        npc_id: &str,
        choices: std::collections::HashSet<String>,
    ) {
        let inherited_interaction = {
            let grants = self.dialogue_grants.read().await;
            grants
                .get(player_id)
                .filter(|grant| grant.choices.is_empty())
                .and_then(|grant| grant.npc_interaction.clone())
        };
        let current_interaction = {
            let grants = self.npc_interaction_grants.read().await;
            grants
                .get(player_id)
                .filter(|grant| grant.npc_id == *npc_id)
                .cloned()
        };
        let npc_interaction = if npc_id.is_empty() {
            None
        } else {
            current_interaction.or(inherited_interaction)
        };
        self.dialogue_grants.write().await.insert(
            player_id.to_string(),
            DialogueGrant {
                quest_id: quest_id.to_string(),
                npc_interaction,
                choices,
            },
        );
    }

    pub(super) async fn authorize_dialogue_choice(
        &self,
        player_id: &str,
        quest_id: &str,
        choice_id: &str,
    ) -> bool {
        let grant = {
            let mut grants = self.dialogue_grants.write().await;
            let Some(grant) = grants.get_mut(player_id) else {
                return false;
            };
            if !consume_dialogue_choice(grant, quest_id, choice_id) {
                return false;
            }
            grant.clone()
        };

        if let Some(npc_grant) = grant.npc_interaction.as_ref()
            && self
                .validate_npc_grant(player_id, npc_grant, 2.5)
                .await
                .is_none()
        {
            self.dialogue_grants.write().await.remove(player_id);
            return false;
        }
        true
    }

    pub(in crate::game) async fn players_share_interaction_context(
        &self,
        first_id: &str,
        second_id: &str,
        max_distance: i32,
    ) -> bool {
        let instances = self.player_instances.read().await;
        if !same_interaction_context(instances.get(first_id), instances.get(second_id)) {
            return false;
        }
        drop(instances);

        let players = self.players.read().await;
        let (Some(first), Some(second)) = (players.get(first_id), players.get(second_id)) else {
            return false;
        };
        first.active
            && second.active
            && !first.is_dead
            && !second.is_dead
            && (first.x - second.x).abs() <= max_distance
            && (first.y - second.y).abs() <= max_distance
    }
}
