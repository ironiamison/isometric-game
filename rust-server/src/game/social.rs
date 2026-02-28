use super::GameRoom;
use crate::protocol::ServerMessage;

fn friend_action_result(action: &str, success: bool, error: Option<String>) -> ServerMessage {
    ServerMessage::FriendActionResult {
        action: action.to_string(),
        success,
        error,
    }
}

fn system_chat_message(text: String, timestamp: u64) -> ServerMessage {
    ServerMessage::ChatMessage {
        sender_id: "system".to_string(),
        sender_name: "System".to_string(),
        text,
        timestamp,
        channel: "system".to_string(),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

impl GameRoom {
    pub fn parse_character_id(player_id: &str) -> Option<i64> {
        player_id.strip_prefix("char_").and_then(|s| s.parse().ok())
    }

    pub fn make_player_id(character_id: i64) -> String {
        format!("char_{}", character_id)
    }

    pub async fn handle_send_friend_request(&self, player_id: &str, target_name: &str) {
        let Some(db) = &self.db else {
            self.send_to_player(
                player_id,
                friend_action_result(
                    "send_request",
                    false,
                    Some("Database not available".to_string()),
                ),
            )
            .await;
            return;
        };

        let Some(requester_id) = Self::parse_character_id(player_id) else {
            return;
        };

        let target_id = match db.get_character_id_by_name(target_name).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                self.send_to_player(
                    player_id,
                    friend_action_result(
                        "send_request",
                        false,
                        Some(format!("Player '{}' not found", target_name)),
                    ),
                )
                .await;
                return;
            }
            Err(error) => {
                tracing::error!("Failed to look up player: {}", error);
                self.send_to_player(
                    player_id,
                    friend_action_result(
                        "send_request",
                        false,
                        Some("Failed to look up player".to_string()),
                    ),
                )
                .await;
                return;
            }
        };

        if requester_id == target_id {
            self.send_to_player(
                player_id,
                friend_action_result(
                    "send_request",
                    false,
                    Some("You can't add yourself as a friend".to_string()),
                ),
            )
            .await;
            return;
        }

        match db.create_friend_request(requester_id, target_id).await {
            Ok(()) => {
                let requester_name = {
                    let players = self.players.read().await;
                    players
                        .get(player_id)
                        .map(|player| player.name.clone())
                        .unwrap_or_default()
                };

                self.send_to_player(player_id, friend_action_result("send_request", true, None))
                    .await;

                let target_player_id = Self::make_player_id(target_id);
                self.send_to_player(
                    &target_player_id,
                    ServerMessage::FriendRequestReceived {
                        from_id: requester_id,
                        from_name: requester_name.clone(),
                    },
                )
                .await;

                self.send_to_player(
                    &target_player_id,
                    system_chat_message(
                        format!("{} sent you a friend request!", requester_name),
                        now_ms(),
                    ),
                )
                .await;
            }
            Err(error) => {
                self.send_to_player(
                    player_id,
                    friend_action_result("send_request", false, Some(error)),
                )
                .await;
            }
        }
    }

    pub async fn handle_accept_friend_request(&self, player_id: &str, requester_id: i64) {
        let Some(db) = &self.db else {
            self.send_to_player(
                player_id,
                friend_action_result(
                    "accept_request",
                    false,
                    Some("Database not available".to_string()),
                ),
            )
            .await;
            return;
        };

        let Some(recipient_id) = Self::parse_character_id(player_id) else {
            return;
        };

        match db.accept_friend_request(requester_id, recipient_id).await {
            Ok(()) => {
                let recipient_name = {
                    let players = self.players.read().await;
                    players.get(player_id).map(|player| player.name.clone())
                };
                let recipient_name = match recipient_name {
                    Some(name) => name,
                    None => db
                        .get_character_name_by_id(recipient_id)
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                };

                let requester_player_id = Self::make_player_id(requester_id);
                let requester_name = {
                    let players = self.players.read().await;
                    players
                        .get(&requester_player_id)
                        .map(|player| player.name.clone())
                };
                let requester_name = match requester_name {
                    Some(name) => name,
                    None => db
                        .get_character_name_by_id(requester_id)
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                };

                self.send_to_player(
                    player_id,
                    friend_action_result("accept_request", true, None),
                )
                .await;

                self.send_to_player(
                    player_id,
                    ServerMessage::FriendRequestAccepted {
                        friend_id: requester_id,
                        friend_name: requester_name.clone(),
                    },
                )
                .await;

                self.send_to_player(
                    &requester_player_id,
                    ServerMessage::FriendRequestAccepted {
                        friend_id: recipient_id,
                        friend_name: recipient_name.clone(),
                    },
                )
                .await;

                self.send_to_player(
                    &requester_player_id,
                    system_chat_message(
                        format!("{} accepted your friend request!", recipient_name),
                        now_ms(),
                    ),
                )
                .await;
            }
            Err(error) => {
                self.send_to_player(
                    player_id,
                    friend_action_result("accept_request", false, Some(error)),
                )
                .await;
            }
        }
    }

    pub async fn handle_decline_friend_request(&self, player_id: &str, requester_id: i64) {
        let Some(db) = &self.db else {
            self.send_to_player(
                player_id,
                friend_action_result(
                    "decline_request",
                    false,
                    Some("Database not available".to_string()),
                ),
            )
            .await;
            return;
        };

        let Some(recipient_id) = Self::parse_character_id(player_id) else {
            return;
        };

        match db.decline_friend_request(requester_id, recipient_id).await {
            Ok(()) => {
                self.send_to_player(
                    player_id,
                    friend_action_result("decline_request", true, None),
                )
                .await;

                let requester_player_id = Self::make_player_id(requester_id);
                self.send_to_player(
                    &requester_player_id,
                    ServerMessage::FriendRequestDeclined {
                        by_id: recipient_id,
                    },
                )
                .await;
            }
            Err(error) => {
                self.send_to_player(
                    player_id,
                    friend_action_result("decline_request", false, Some(error)),
                )
                .await;
            }
        }
    }

    pub async fn handle_remove_friend(&self, player_id: &str, friend_id: i64) {
        let Some(db) = &self.db else {
            self.send_to_player(
                player_id,
                friend_action_result(
                    "remove_friend",
                    false,
                    Some("Database not available".to_string()),
                ),
            )
            .await;
            return;
        };

        let Some(character_id) = Self::parse_character_id(player_id) else {
            return;
        };

        match db.remove_friend(character_id, friend_id).await {
            Ok(()) => {
                self.send_to_player(player_id, friend_action_result("remove_friend", true, None))
                    .await;

                self.send_to_player(player_id, ServerMessage::FriendRemoved { friend_id })
                    .await;

                let friend_player_id = Self::make_player_id(friend_id);
                self.send_to_player(
                    &friend_player_id,
                    ServerMessage::FriendRemoved {
                        friend_id: character_id,
                    },
                )
                .await;
            }
            Err(error) => {
                self.send_to_player(
                    player_id,
                    friend_action_result("remove_friend", false, Some(error)),
                )
                .await;
            }
        }
    }

    pub async fn handle_get_online_players(&self, player_id: &str) {
        let Some(db) = &self.db else {
            return;
        };

        let Some(character_id) = Self::parse_character_id(player_id) else {
            return;
        };

        let players = self.players.read().await;
        let mut online_players = Vec::new();

        for player in players.values() {
            if !player.active {
                continue;
            }

            if let Some(pid) = Self::parse_character_id(&player.id) {
                let is_friend = db.are_friends(character_id, pid).await.unwrap_or(false);
                online_players.push(crate::protocol::OnlinePlayerInfo {
                    id: pid,
                    name: player.name.clone(),
                    is_friend,
                });
            }
        }

        self.send_to_player(
            player_id,
            ServerMessage::OnlinePlayersList {
                players: online_players,
            },
        )
        .await;
    }

    pub async fn send_friends_data(
        &self,
        player_id: &str,
        online_characters: &dashmap::DashSet<i64>,
    ) {
        tracing::info!("send_friends_data called for player_id: {}", player_id);

        let Some(db) = &self.db else {
            tracing::warn!("No database connection in send_friends_data");
            return;
        };

        let Some(character_id) = Self::parse_character_id(player_id) else {
            tracing::warn!("Could not parse character_id from player_id: {}", player_id);
            return;
        };

        tracing::info!("Fetching friends data for character_id: {}", character_id);

        match db.get_friends_list(character_id).await {
            Ok(friends) => {
                tracing::info!(
                    "Found {} friends for character {}",
                    friends.len(),
                    character_id
                );
                let friend_infos: Vec<crate::protocol::FriendInfo> = friends
                    .into_iter()
                    .map(|(id, name)| crate::protocol::FriendInfo {
                        id,
                        name,
                        online: online_characters.contains(&id),
                    })
                    .collect();

                self.send_to_player(
                    player_id,
                    ServerMessage::FriendsList {
                        friends: friend_infos,
                    },
                )
                .await;
            }
            Err(error) => {
                tracing::error!("Error fetching friends list: {:?}", error);
            }
        }

        match db.get_pending_requests(character_id).await {
            Ok(requests) => {
                tracing::info!(
                    "Found {} pending friend requests for character {}",
                    requests.len(),
                    character_id
                );
                let request_infos: Vec<crate::protocol::PendingRequestInfo> = requests
                    .into_iter()
                    .map(|(from_id, from_name)| {
                        tracing::info!("  - Request from {} (id: {})", from_name, from_id);
                        crate::protocol::PendingRequestInfo { from_id, from_name }
                    })
                    .collect();

                self.send_to_player(
                    player_id,
                    ServerMessage::PendingFriendRequests {
                        requests: request_infos,
                    },
                )
                .await;
            }
            Err(error) => {
                tracing::error!("Error fetching pending requests: {:?}", error);
            }
        }
    }

    pub async fn broadcast_friend_status(&self, player_id: &str, online: bool) {
        let Some(db) = &self.db else {
            return;
        };

        let Some(character_id) = Self::parse_character_id(player_id) else {
            return;
        };

        if let Ok(friends) = db.get_friends_list(character_id).await {
            for (friend_id, _) in friends {
                let friend_player_id = Self::make_player_id(friend_id);
                self.send_to_player(
                    &friend_player_id,
                    ServerMessage::FriendStatusChanged {
                        friend_id: character_id,
                        online,
                    },
                )
                .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_id_round_trip_uses_char_prefix() {
        let player_id = GameRoom::make_player_id(42);
        assert_eq!(player_id, "char_42");
        assert_eq!(GameRoom::parse_character_id(&player_id), Some(42));
    }

    #[test]
    fn parse_character_id_rejects_invalid_formats() {
        assert_eq!(GameRoom::parse_character_id("42"), None);
        assert_eq!(GameRoom::parse_character_id("char_abc"), None);
        assert_eq!(GameRoom::parse_character_id("player_42"), None);
    }

    #[test]
    fn system_chat_message_uses_system_sender_fields() {
        let message = system_chat_message("hello".to_string(), 123);
        match message {
            ServerMessage::ChatMessage {
                sender_id,
                sender_name,
                text,
                timestamp,
                channel,
            } => {
                assert_eq!(sender_id, "system");
                assert_eq!(sender_name, "System");
                assert_eq!(text, "hello");
                assert_eq!(timestamp, 123);
                assert_eq!(channel, "system");
            }
            other => panic!("expected ChatMessage, got {:?}", other),
        }
    }
}
