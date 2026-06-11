use super::GameRoom;
use crate::protocol::ServerMessage;
use std::collections::HashSet;

fn gathering_stopped_message(player_id: &str, reason: &str) -> ServerMessage {
    ServerMessage::GatheringStopped {
        player_id: player_id.to_string(),
        reason: reason.to_string(),
    }
}

fn woodcutting_stopped_message(player_id: &str, reason: &str) -> ServerMessage {
    ServerMessage::WoodcuttingStopped {
        player_id: player_id.to_string(),
        reason: reason.to_string(),
    }
}

fn find_crafting_interruptions(
    moved_players: &HashSet<String>,
    active_crafters: &HashSet<String>,
) -> Vec<String> {
    moved_players
        .iter()
        .filter(|id| active_crafters.contains(*id))
        .cloned()
        .collect()
}

impl GameRoom {
    pub(in crate::game) async fn handle_post_movement_effects(
        &self,
        moved_players: &HashSet<String>,
        woodcutting_stopped: Vec<String>,
    ) {
        {
            let mut players = self.players.write().await;
            for player in players.values_mut() {
                player.is_dashing = false;
            }
        }

        {
            let mut gathering = self.gathering.write().await;
            let mut stopped = Vec::new();
            for id in moved_players {
                if gathering.is_gathering(id) {
                    gathering.stop_gathering(id);
                    stopped.push(id.clone());
                }
            }

            drop(gathering);

            for id in stopped {
                self.broadcast_to_zone(&id, gathering_stopped_message(&id, "moved"))
                    .await;
            }
        }

        for id in woodcutting_stopped {
            self.broadcast_to_zone(&id, woodcutting_stopped_message(&id, "moved"))
                .await;
        }

        let crafting_to_cancel: Vec<String> = {
            let players = self.players.read().await;
            let active_crafters: HashSet<String> = players
                .values()
                .filter(|player| player.crafting_state.is_some())
                .map(|player| player.id.clone())
                .collect();
            find_crafting_interruptions(moved_players, &active_crafters)
        };

        for id in crafting_to_cancel {
            self.cancel_crafting(&id, "interrupted").await;
        }

        self.process_timed_crafting_completions().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_message_helpers_preserve_player_and_reason() {
        match gathering_stopped_message("char_1", "moved") {
            ServerMessage::GatheringStopped { player_id, reason } => {
                assert_eq!(player_id, "char_1");
                assert_eq!(reason, "moved");
            }
            other => panic!("expected GatheringStopped, got {:?}", other),
        }

        match woodcutting_stopped_message("char_2", "interrupted") {
            ServerMessage::WoodcuttingStopped { player_id, reason } => {
                assert_eq!(player_id, "char_2");
                assert_eq!(reason, "interrupted");
            }
            other => panic!("expected WoodcuttingStopped, got {:?}", other),
        }
    }

    #[test]
    fn find_crafting_interruptions_only_returns_moved_active_crafters() {
        let active_crafters = HashSet::from([
            "char_2".to_string(),
            "char_4".to_string(),
            "char_3".to_string(),
        ]);

        let moved_players = HashSet::from([
            "char_1".to_string(),
            "char_2".to_string(),
            "char_3".to_string(),
        ]);

        assert_eq!(
            find_crafting_interruptions(&moved_players, &active_crafters)
                .into_iter()
                .collect::<HashSet<_>>(),
            HashSet::from(["char_2".to_string(), "char_3".to_string()])
        );
    }
}
