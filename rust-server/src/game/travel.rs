use super::GameRoom;
use crate::protocol::{DialogueChoice, ServerMessage};

const NORTH_OBELISK: (i32, i32) = (92, -163);

fn is_near_northern_obelisk(x: i32, y: i32) -> bool {
    (x - NORTH_OBELISK.0).abs() <= 2 && (y - NORTH_OBELISK.1).abs() <= 2
}

fn waystone_prompt_message(
    waystone_id: &str,
    waystone_name: &str,
    destination_name: &str,
) -> ServerMessage {
    ServerMessage::ShowDialogue {
        quest_id: format!("waystone:{}", waystone_id),
        npc_id: String::new(),
        speaker: waystone_name.to_string(),
        text: format!(
            "The waystone hums with energy. Travel to the {}?",
            destination_name
        ),
        choices: vec![
            DialogueChoice {
                id: "teleport".to_string(),
                text: "Yes, teleport me.".to_string(),
            },
            DialogueChoice {
                id: "cancel".to_string(),
                text: "Not now.".to_string(),
            },
        ],
    }
}

impl GameRoom {
    /// Handle player interacting with a world map object (obelisk, etc.)
    pub async fn handle_interact_object(&self, player_id: &str, x: i32, y: i32) {
        tracing::debug!(
            "handle_interact_object: player={} x={} y={}",
            player_id,
            x,
            y
        );

        if self.handle_obelisk_quest_interaction(player_id, x, y).await {
            tracing::debug!("handle_interact_object: handled by quest interaction");
            return;
        }

        if self.try_open_chest(player_id, x, y).await {
            tracing::debug!("handle_interact_object: handled by chest");
            return;
        }

        let waystone = {
            let waystones = self.waystone_manager.read().await;
            waystones.get_at(x, y).cloned()
        };

        if let Some(waystone) = waystone.as_ref() {
            tracing::debug!(
                "handle_interact_object: found waystone '{}' at ({},{})",
                waystone.id,
                waystone.x,
                waystone.y
            );
            self.handle_waystone_interaction(player_id, waystone).await;
            return;
        }

        tracing::debug!(
            "handle_interact_object: no waystone or quest interaction found at ({},{})",
            x,
            y
        );
    }

    async fn handle_waystone_interaction(
        &self,
        player_id: &str,
        waystone: &crate::waystone::WaystoneDef,
    ) {
        let quest_completed = {
            let quest_states = self.player_quest_states.read().await;
            let has_state = quest_states.get(player_id).is_some();
            let completed = quest_states
                .get(player_id)
                .map(|state| state.is_quest_completed(&waystone.quest_required))
                .unwrap_or(false);
            tracing::debug!(
                "handle_waystone_interaction: waystone='{}' quest_required='{}' has_quest_state={} quest_completed={}",
                waystone.id,
                waystone.quest_required,
                has_state,
                completed
            );
            completed
        };

        if quest_completed {
            let destination_name = {
                let waystones = self.waystone_manager.read().await;
                waystones
                    .get_destination(&waystone.id)
                    .map(|destination| destination.name.clone())
                    .unwrap_or_else(|| "unknown".to_string())
            };

            self.send_to_player(
                player_id,
                waystone_prompt_message(&waystone.id, &waystone.name, &destination_name),
            )
            .await;
        } else {
            self.send_system_message(
                player_id,
                "The ancient stone stands silent. Perhaps someone nearby knows more about it.",
            )
            .await;
        }
    }

    /// Handle clicking the northern obelisk during the quest (before waystone is unlocked).
    /// Returns true if an active quest interaction was handled, false to fall through to waystone.
    async fn handle_obelisk_quest_interaction(&self, player_id: &str, x: i32, y: i32) -> bool {
        if !is_near_northern_obelisk(x, y) {
            return false;
        }

        let quest_info = {
            let quest_states = self.player_quest_states.read().await;
            if let Some(state) = quest_states.get(player_id) {
                if let Some(progress) = state.active_quests.get("obelisk_connection") {
                    let reach_done = progress
                        .objectives
                        .get("reach_north_obelisk")
                        .map(|objective| objective.completed)
                        .unwrap_or(false);
                    let kill_done = progress
                        .objectives
                        .get("kill_hedgehog")
                        .map(|objective| objective.completed)
                        .unwrap_or(false);
                    Some((reach_done, kill_done))
                } else {
                    None
                }
            } else {
                None
            }
        };

        match quest_info {
            Some((true, false)) => {
                self.send_to_player(
                    player_id,
                    ServerMessage::ShowDialogue {
                        quest_id: String::new(),
                        npc_id: String::new(),
                        speaker: "Ancient Obelisk".to_string(),
                        text: "The stone hums faintly... something buried beneath is disrupting the flow of energy. You'll need to dig it out. Perhaps there's a tool lying nearby...".to_string(),
                        choices: vec![],
                    },
                )
                .await;
                true
            }
            Some((true, true)) => {
                self.send_to_player(
                    player_id,
                    ServerMessage::ShowDialogue {
                        quest_id: String::new(),
                        npc_id: String::new(),
                        speaker: "Ancient Obelisk".to_string(),
                        text: "The stone pulses with renewed energy. You feel the connection snap into place, reaching far to the south. The waystone is restored! Return to Researcher Orin with the good news.".to_string(),
                        choices: vec![],
                    },
                )
                .await;
                true
            }
            Some((false, _)) => {
                self.send_to_player(
                    player_id,
                    ServerMessage::ShowDialogue {
                        quest_id: String::new(),
                        npc_id: String::new(),
                        speaker: "Ancient Obelisk".to_string(),
                        text: "The ancient stone thrums with dormant power. You sense it's waiting for something...".to_string(),
                        choices: vec![],
                    },
                )
                .await;
                true
            }
            None => false,
        }
    }

    /// Teleport a player to the destination of a waystone
    pub(in crate::game) async fn teleport_to_waystone(&self, player_id: &str, waystone_id: &str) {
        let destination = {
            let waystones = self.waystone_manager.read().await;
            waystones.get_destination(waystone_id).cloned()
        };

        if let Some(destination) = destination {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.x = destination.x;
                player.y = destination.y + 1;
                player.move_dx = 0;
                player.move_dy = 0;
                tracing::info!(
                    "Player {} teleported to waystone {} at ({}, {})",
                    player_id,
                    destination.name,
                    destination.x,
                    destination.y
                );
            }
        }

        // Stop gathering (fishing, etc.) on teleport
        self.handle_stop_gathering(player_id).await;
    }

    /// Handle direct waystone teleport (right-click Teleport, no dialogue)
    pub async fn handle_use_waystone(&self, player_id: &str, x: i32, y: i32) {
        let waystone = {
            let waystones = self.waystone_manager.read().await;
            waystones.get_at(x, y).cloned()
        };

        let Some(waystone) = waystone else {
            return;
        };

        let quest_completed = {
            let quest_states = self.player_quest_states.read().await;
            quest_states
                .get(player_id)
                .map(|state| state.is_quest_completed(&waystone.quest_required))
                .unwrap_or(false)
        };

        if quest_completed {
            self.teleport_to_waystone(player_id, &waystone.id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn northern_obelisk_proximity_uses_two_tile_radius() {
        assert!(is_near_northern_obelisk(92, -163));
        assert!(is_near_northern_obelisk(94, -161));
        assert!(!is_near_northern_obelisk(95, -163));
        assert!(!is_near_northern_obelisk(92, -160));
    }

    #[test]
    fn waystone_prompt_message_uses_dialogue_routing_prefix_and_choices() {
        let message = waystone_prompt_message("north", "North Stone", "South Ruins");

        match message {
            ServerMessage::ShowDialogue {
                quest_id,
                speaker,
                text,
                choices,
                ..
            } => {
                assert_eq!(quest_id, "waystone:north");
                assert_eq!(speaker, "North Stone");
                assert!(text.contains("South Ruins"));
                assert_eq!(choices.len(), 2);
                assert_eq!(choices[0].id, "teleport");
                assert_eq!(choices[1].id, "cancel");
            }
            other => panic!("expected ShowDialogue, got {:?}", other),
        }
    }
}
