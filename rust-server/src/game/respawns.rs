use super::{GameRoom, WORLD_SPAWN_X, WORLD_SPAWN_Y};
use crate::protocol::ServerMessage;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RespawnedPlayer {
    id: String,
    x: i32,
    y: i32,
    hp: i32,
    prayer_points: i32,
    max_prayer_points: i32,
}

fn overworld_transition_message() -> ServerMessage {
    ServerMessage::MapTransition {
        map_type: "overworld".to_string(),
        map_id: "world_0".to_string(),
        spawn_x: WORLD_SPAWN_X as f32,
        spawn_y: WORLD_SPAWN_Y as f32,
        instance_id: String::new(),
    }
}

fn respawn_broadcast_message(player: &RespawnedPlayer) -> ServerMessage {
    ServerMessage::PlayerRespawned {
        id: player.id.clone(),
        x: player.x,
        y: player.y,
        hp: player.hp,
    }
}

fn respawn_prayer_state_message(player: &RespawnedPlayer) -> ServerMessage {
    ServerMessage::PrayerStateUpdate {
        points: player.prayer_points,
        max_points: player.max_prayer_points,
        active_prayers: vec![],
    }
}

impl GameRoom {
    pub(in crate::game) async fn handle_player_respawns(&self, current_time: u64) {
        let mut respawned_players = Vec::new();
        let mut chairs_to_free = Vec::new();
        {
            let mut players = self.players.write().await;
            for player in players.values_mut() {
                if !player.active {
                    continue;
                }

                if player.ready_to_respawn(current_time) {
                    if let Some(chair_coords) = player.respawn() {
                        chairs_to_free.push((player.id.clone(), chair_coords));
                    }
                    respawned_players.push(RespawnedPlayer {
                        id: player.id.clone(),
                        x: player.x,
                        y: player.y,
                        hp: player.hp,
                        prayer_points: player.prayer_points,
                        max_prayer_points: player.max_prayer_points(),
                    });
                }
            }
        }

        if !chairs_to_free.is_empty() {
            let mut chairs = self.chairs.write().await;
            for (player_id, (tile_x, tile_y)) in chairs_to_free {
                if let Some(chair) = chairs.get_mut(&(tile_x, tile_y)) {
                    if chair.occupied_by.as_deref() == Some(&player_id) {
                        chair.occupied_by = None;
                    }
                }
            }
        }

        for player in &respawned_players {
            self.handle_stop_gathering(&player.id).await;
        }

        for player in &respawned_players {
            let instance_id = self.player_instances.write().await.remove(&player.id);
            if let Some(instance_id) = instance_id {
                // If respawning from the boss cave, use the cave exit coordinates
                let is_boss_instance = instance_id.contains(crate::game::boss_tick::BOSS_MAP_ID);

                self.reset_sync_state(&player.id).await;
                if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                    let other_players: Vec<String> = instance
                        .get_player_ids()
                        .await
                        .into_iter()
                        .filter(|id| id != &player.id)
                        .collect();

                    let remaining = instance.remove_player(&player.id).await;
                    if remaining == 0
                        && instance.instance_type == crate::interior::InstanceType::Private
                    {
                        if let Some(owner_id) = &instance.owner_id {
                            self.instance_manager
                                .remove_private(owner_id, &instance.map_id);
                        }
                    }

                    for other_id in &other_players {
                        self.send_to_player(
                            other_id,
                            ServerMessage::PlayerLeft {
                                id: player.id.clone(),
                            },
                        )
                        .await;
                        self.send_to_player(
                            &player.id,
                            ServerMessage::PlayerLeft {
                                id: other_id.clone(),
                            },
                        )
                        .await;
                    }
                }

                if is_boss_instance {
                    // Death in boss cave = respawn at world spawn (starting village),
                    // not at the cave entrance
                    {
                        let mut players = self.players.write().await;
                        if let Some(p) = players.get_mut(&player.id) {
                            p.x = WORLD_SPAWN_X;
                            p.y = WORLD_SPAWN_Y;
                        }
                    }
                    self.send_to_player(&player.id, overworld_transition_message())
                        .await;
                } else {
                    self.send_to_player(&player.id, overworld_transition_message())
                        .await;
                }
            }
        }

        for mut player in respawned_players {
            // Update coordinates if they were overridden by boss exit
            {
                let players = self.players.read().await;
                if let Some(p) = players.get(&player.id) {
                    player.x = p.x;
                    player.y = p.y;
                }
            }
            tracing::info!(
                "Player {} respawned at ({}, {})",
                player.id,
                player.x,
                player.y
            );
            self.broadcast_to_zone(&player.id, respawn_broadcast_message(&player))
                .await;
            self.send_to_player(&player.id, respawn_prayer_state_message(&player))
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overworld_transition_message_targets_world_spawn() {
        match overworld_transition_message() {
            ServerMessage::MapTransition {
                map_type,
                map_id,
                spawn_x,
                spawn_y,
                instance_id,
            } => {
                assert_eq!(map_type, "overworld");
                assert_eq!(map_id, "world_0");
                assert_eq!(spawn_x, WORLD_SPAWN_X as f32);
                assert_eq!(spawn_y, WORLD_SPAWN_Y as f32);
                assert!(instance_id.is_empty());
            }
            other => panic!("expected MapTransition, got {:?}", other),
        }
    }

    #[test]
    fn respawn_messages_preserve_player_state_and_clear_prayers() {
        let player = RespawnedPlayer {
            id: "char_1".to_string(),
            x: 15,
            y: 4,
            hp: 100,
            prayer_points: 12,
            max_prayer_points: 20,
        };

        match respawn_broadcast_message(&player) {
            ServerMessage::PlayerRespawned { id, x, y, hp } => {
                assert_eq!(id, "char_1");
                assert_eq!(x, 15);
                assert_eq!(y, 4);
                assert_eq!(hp, 100);
            }
            other => panic!("expected PlayerRespawned, got {:?}", other),
        }

        match respawn_prayer_state_message(&player) {
            ServerMessage::PrayerStateUpdate {
                points,
                max_points,
                active_prayers,
            } => {
                assert_eq!(points, 12);
                assert_eq!(max_points, 20);
                assert!(active_prayers.is_empty());
            }
            other => panic!("expected PrayerStateUpdate, got {:?}", other),
        }
    }
}
