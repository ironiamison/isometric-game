use std::collections::HashMap;

use dashmap::{DashMap, mapref::one::RefMut};
use tokio::sync::{RwLock, broadcast, mpsc};

use super::PlayerUpdate;
use crate::npc::NpcUpdate;

pub(super) const FULL_SYNC_INTERVAL: u64 = 20;

type MessageSender = mpsc::Sender<Vec<u8>>;
pub(super) type SenderSnapshot = HashMap<String, MessageSender>;

/// Owns connection-facing state for a room.
///
/// Gameplay code receives cloned channel handles and never retains a transport
/// lock while reading or mutating world state.
pub(super) struct RoomTransport {
    broadcast_tx: broadcast::Sender<Vec<u8>>,
    player_senders: RwLock<SenderSnapshot>,
    spectator_senders: RwLock<SenderSnapshot>,
    sync_states: DashMap<String, PlayerSyncState>,
}

impl RoomTransport {
    pub(super) fn new(broadcast_capacity: usize) -> Self {
        let (broadcast_tx, _) = broadcast::channel(broadcast_capacity);
        Self {
            broadcast_tx,
            player_senders: RwLock::new(HashMap::new()),
            spectator_senders: RwLock::new(HashMap::new()),
            sync_states: DashMap::new(),
        }
    }

    pub(super) fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        self.broadcast_tx.subscribe()
    }

    pub(super) fn broadcast(&self, bytes: Vec<u8>) {
        let _ = self.broadcast_tx.send(bytes);
    }

    pub(super) async fn register_player(&self, player_id: &str, sender: MessageSender) {
        self.player_senders
            .write()
            .await
            .insert(player_id.to_string(), sender);
        self.sync_states
            .insert(player_id.to_string(), PlayerSyncState::new());
    }

    pub(super) async fn unregister_player(&self, player_id: &str) {
        self.player_senders.write().await.remove(player_id);
        self.sync_states.remove(player_id);
    }

    pub(super) async fn player_sender(&self, player_id: &str) -> Option<MessageSender> {
        self.player_senders.read().await.get(player_id).cloned()
    }

    pub(super) async fn player_senders(&self) -> SenderSnapshot {
        self.player_senders.read().await.clone()
    }

    pub(super) async fn add_spectator(&self, spectator_id: &str, sender: MessageSender) {
        self.spectator_senders
            .write()
            .await
            .insert(spectator_id.to_string(), sender);
    }

    pub(super) async fn remove_spectator(&self, spectator_id: &str) {
        self.spectator_senders.write().await.remove(spectator_id);
    }

    pub(super) async fn spectator_count(&self) -> usize {
        self.spectator_senders.read().await.len()
    }

    pub(super) async fn spectator_senders(&self) -> SenderSnapshot {
        self.spectator_senders.read().await.clone()
    }

    pub(super) fn sync_state(
        &self,
        player_id: &str,
    ) -> Option<RefMut<'_, String, PlayerSyncState>> {
        self.sync_states.get_mut(player_id)
    }

    pub(super) fn reset_sync_state(&self, player_id: &str) {
        if let Some(mut state) = self.sync_states.get_mut(player_id) {
            *state = PlayerSyncState::new();
        }
    }
}

pub(super) struct PlayerSyncState {
    pub(super) context_id: String,
    pub(super) last_players: HashMap<String, PlayerUpdate>,
    pub(super) last_npcs: HashMap<String, NpcUpdate>,
    pub(super) last_full_sync_tick: u64,
    pub(super) next_full_sync_tick: u64,
}

impl PlayerSyncState {
    fn new() -> Self {
        Self {
            context_id: String::new(),
            last_players: HashMap::new(),
            last_npcs: HashMap::new(),
            last_full_sync_tick: 0,
            next_full_sync_tick: 0,
        }
    }

    pub(super) fn ensure_context(&mut self, context_id: &str) {
        if self.context_id != context_id {
            *self = Self::new();
            self.context_id = context_id.to_string();
        }
    }
}

pub(super) fn full_sync_offset(player_id: &str) -> u64 {
    player_id.bytes().fold(0u64, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as u64)
    }) % FULL_SYNC_INTERVAL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_change_forces_a_new_full_sync() {
        let mut state = PlayerSyncState::new();
        state.ensure_context("instance-a");
        state.last_full_sync_tick = 10;
        state.next_full_sync_tick = 30;

        state.ensure_context("instance-a");
        assert_eq!(state.last_full_sync_tick, 10);

        state.ensure_context("");
        assert_eq!(state.context_id, "");
        assert_eq!(state.last_full_sync_tick, 0);
        assert_eq!(state.next_full_sync_tick, 0);
        assert!(state.last_players.is_empty());
        assert!(state.last_npcs.is_empty());
    }

    #[tokio::test]
    async fn unregistering_a_player_removes_sender_and_sync_state() {
        let transport = RoomTransport::new(8);
        let (sender, _) = mpsc::channel(1);
        transport.register_player("player-1", sender).await;

        assert!(transport.player_sender("player-1").await.is_some());
        assert!(transport.sync_state("player-1").is_some());

        transport.unregister_player("player-1").await;

        assert!(transport.player_sender("player-1").await.is_none());
        assert!(transport.sync_state("player-1").is_none());
    }
}
