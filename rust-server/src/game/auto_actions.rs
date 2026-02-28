use super::{AutoAction, AutoActionTarget, AutoActionType, GameRoom};
use crate::protocol::ServerMessage;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn parse_auto_action_type(action: &str) -> Option<AutoActionType> {
    match action {
        "attack" => Some(AutoActionType::Attack),
        "mine" => Some(AutoActionType::Mine),
        "chop" => Some(AutoActionType::Chop),
        _ => None,
    }
}

fn parse_resource_target(target_id: &str) -> Option<(i32, i32, u32)> {
    let parts: Vec<&str> = target_id.split(',').collect();
    if parts.len() != 3 {
        return None;
    }

    Some((
        parts[0].parse::<i32>().ok()?,
        parts[1].parse::<i32>().ok()?,
        parts[2].parse::<u32>().ok()?,
    ))
}

fn auto_action_stopped_message(reason: &str) -> ServerMessage {
    ServerMessage::AutoActionStopped {
        reason: reason.to_string(),
    }
}

impl GameRoom {
    pub async fn handle_start_auto_action(
        &self,
        player_id: &str,
        target_type: &str,
        target_id: &str,
        action: &str,
    ) {
        let action_type = match parse_auto_action_type(action) {
            Some(action_type) => action_type,
            None => {
                tracing::warn!("Invalid auto-action type: {}", action);
                return;
            }
        };

        let target = match target_type {
            "npc" => {
                let player_instance = self.player_instances.read().await.get(player_id).cloned();
                if let Some(instance_id) = player_instance.as_ref() {
                    if let Some(instance) = self.instance_manager.get_by_instance_id(instance_id) {
                        let npcs = instance.npcs.read().await;
                        if let Some(npc) = npcs.get(target_id) {
                            if !npc.is_alive() {
                                self.send_to_player(
                                    player_id,
                                    auto_action_stopped_message("target_dead"),
                                )
                                .await;
                                return;
                            }
                            if action_type == AutoActionType::Attack && !npc.is_attackable() {
                                return;
                            }
                        } else {
                            return;
                        }
                    } else {
                        return;
                    }
                } else {
                    let npcs = self.npcs.read().await;
                    if let Some(npc) = npcs.get(target_id) {
                        if !npc.is_alive() {
                            self.send_to_player(
                                player_id,
                                auto_action_stopped_message("target_dead"),
                            )
                            .await;
                            return;
                        }
                        if action_type == AutoActionType::Attack && !npc.is_attackable() {
                            return;
                        }
                    } else {
                        return;
                    }
                }

                AutoActionTarget::Npc {
                    npc_id: target_id.to_string(),
                }
            }
            "player" => {
                if target_id == player_id {
                    return;
                }

                let players = self.players.read().await;
                if let Some(target) = players.get(target_id) {
                    if !target.active || target.is_dead {
                        return;
                    }
                } else {
                    return;
                }

                AutoActionTarget::Player {
                    player_id: target_id.to_string(),
                }
            }
            "resource" => {
                if self.player_instances.read().await.contains_key(player_id) {
                    return;
                }

                let (x, y, gid) = match parse_resource_target(target_id) {
                    Some(parsed) => parsed,
                    None => {
                        tracing::warn!("Invalid resource target_id format: {}", target_id);
                        return;
                    }
                };

                AutoActionTarget::Resource { x, y, gid }
            }
            _ => {
                tracing::warn!("Invalid auto-action target_type: {}", target_type);
                return;
            }
        };

        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                if player.is_dead {
                    return;
                }
                player.auto_action = Some(AutoAction {
                    target,
                    action: action_type,
                    started_at: now_ms(),
                });
            }
        }

        self.send_to_player(
            player_id,
            ServerMessage::AutoActionStarted {
                target_type: target_type.to_string(),
                target_id: target_id.to_string(),
                action: action.to_string(),
            },
        )
        .await;

        tracing::info!(
            "Player {} started auto-action: {} on {} {}",
            player_id,
            action,
            target_type,
            target_id
        );
    }

    pub async fn handle_cancel_auto_action(&self, player_id: &str) {
        let had_action = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                let had_action = player.auto_action.is_some();
                player.auto_action = None;
                had_action
            } else {
                false
            }
        };

        if had_action {
            self.send_to_player(player_id, auto_action_stopped_message("cancelled"))
                .await;
        }
    }

    pub(in crate::game) async fn clear_auto_action(&self, player_id: &str, reason: &str) {
        let had_action = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                let had_action = player.auto_action.is_some();
                player.auto_action = None;
                had_action
            } else {
                false
            }
        };

        if had_action {
            self.send_to_player(player_id, auto_action_stopped_message(reason))
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_auto_action_type_accepts_only_supported_values() {
        assert_eq!(
            parse_auto_action_type("attack"),
            Some(AutoActionType::Attack)
        );
        assert_eq!(parse_auto_action_type("mine"), Some(AutoActionType::Mine));
        assert_eq!(parse_auto_action_type("chop"), Some(AutoActionType::Chop));
        assert_eq!(parse_auto_action_type("fish"), None);
    }

    #[test]
    fn parse_resource_target_requires_three_numeric_parts() {
        assert_eq!(parse_resource_target("10,20,30"), Some((10, 20, 30)));
        assert_eq!(parse_resource_target("10,20"), None);
        assert_eq!(parse_resource_target("10,twenty,30"), None);
    }

    #[test]
    fn auto_action_stopped_message_preserves_reason() {
        match auto_action_stopped_message("target_dead") {
            ServerMessage::AutoActionStopped { reason } => assert_eq!(reason, "target_dead"),
            other => panic!("expected AutoActionStopped, got {:?}", other),
        }
    }
}
