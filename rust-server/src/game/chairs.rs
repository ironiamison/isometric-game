use super::{Direction, GameRoom};
use crate::protocol::ServerMessage;

fn expected_chair_approach(tile_x: i32, tile_y: i32, direction: Direction) -> (i32, i32) {
    match direction {
        Direction::Down => (tile_x, tile_y + 1),
        Direction::Up => (tile_x, tile_y - 1),
        Direction::Left => (tile_x - 1, tile_y),
        Direction::Right => (tile_x + 1, tile_y),
        _ => (tile_x, tile_y + 1),
    }
}

fn stand_up_position(tile_x: i32, tile_y: i32, direction: Direction) -> (i32, i32) {
    match direction {
        Direction::Up => (tile_x, tile_y - 1),
        Direction::Down => (tile_x, tile_y + 1),
        Direction::Left => (tile_x - 1, tile_y),
        Direction::Right => (tile_x + 1, tile_y),
        _ => (tile_x, tile_y),
    }
}

impl GameRoom {
    pub async fn get_chair_positions_message(&self) -> ServerMessage {
        let chairs = self.chairs.read().await;
        let positions: Vec<(i32, i32)> = chairs.keys().cloned().collect();
        ServerMessage::ChairPositions { positions }
    }

    pub async fn handle_sit_chair(&self, player_id: &str, tile_x: i32, tile_y: i32) {
        if self.player_instances.read().await.contains_key(player_id) {
            return;
        }

        {
            let chairs = self.chairs.read().await;
            let chair = match chairs.get(&(tile_x, tile_y)) {
                Some(chair) => chair,
                None => {
                    self.send_to_player(
                        player_id,
                        ServerMessage::Error {
                            code: 400,
                            message: "No chair at that position".to_string(),
                        },
                    )
                    .await;
                    return;
                }
            };
            if chair.occupied_by.is_some() {
                self.send_to_player(
                    player_id,
                    ServerMessage::Error {
                        code: 400,
                        message: "Chair is occupied".to_string(),
                    },
                )
                .await;
                return;
            }
        }

        let (player_x, player_y, already_sitting) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) => (player.x, player.y, player.sitting_at.is_some()),
                None => return,
            }
        };

        if already_sitting {
            return;
        }

        let chair_direction = {
            let chairs = self.chairs.read().await;
            match chairs.get(&(tile_x, tile_y)) {
                Some(chair) => chair.direction,
                None => return,
            }
        };

        let (expected_x, expected_y) = expected_chair_approach(tile_x, tile_y, chair_direction);
        if player_x != expected_x || player_y != expected_y {
            self.send_to_player(
                player_id,
                ServerMessage::Error {
                    code: 400,
                    message: "Must approach chair from the front".to_string(),
                },
            )
            .await;
            return;
        }

        let direction = {
            let mut chairs = self.chairs.write().await;
            if let Some(chair) = chairs.get_mut(&(tile_x, tile_y)) {
                if chair.occupied_by.is_some() {
                    return;
                }
                chair.occupied_by = Some(player_id.to_string());
                chair.direction
            } else {
                return;
            }
        };

        self.handle_stop_gathering(player_id).await;

        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.sitting_at = Some((tile_x, tile_y));
                player.x = tile_x;
                player.y = tile_y;
                player.direction = direction;
                player.reject_pending_move();
            }
        }

        self.send_to_player(
            player_id,
            ServerMessage::SitResult {
                success: true,
                tile_x,
                tile_y,
                direction: direction as u8,
            },
        )
        .await;
    }

    pub async fn handle_stand_up(&self, player_id: &str) {
        let sitting_at = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) => player.sitting_at,
                None => return,
            }
        };

        if let Some((tile_x, tile_y)) = sitting_at {
            let chair_direction = {
                let chairs = self.chairs.read().await;
                chairs.get(&(tile_x, tile_y)).map(|chair| chair.direction)
            };

            {
                let mut chairs = self.chairs.write().await;
                if let Some(chair) = chairs.get_mut(&(tile_x, tile_y))
                    && chair.occupied_by.as_deref() == Some(player_id)
                {
                    chair.occupied_by = None;
                }
            }

            {
                let mut players = self.players.write().await;
                if let Some(player) = players.get_mut(player_id) {
                    player.sitting_at = None;
                    if let Some(direction) = chair_direction {
                        let (new_x, new_y) = stand_up_position(tile_x, tile_y, direction);
                        player.x = new_x;
                        player.y = new_y;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_chair_approach_matches_front_tile_by_direction() {
        assert_eq!(expected_chair_approach(10, 20, Direction::Down), (10, 21));
        assert_eq!(expected_chair_approach(10, 20, Direction::Up), (10, 19));
        assert_eq!(expected_chair_approach(10, 20, Direction::Left), (9, 20));
        assert_eq!(expected_chair_approach(10, 20, Direction::Right), (11, 20));
    }

    #[test]
    fn stand_up_position_uses_tile_in_front_of_chair() {
        assert_eq!(stand_up_position(10, 20, Direction::Down), (10, 21));
        assert_eq!(stand_up_position(10, 20, Direction::Up), (10, 19));
        assert_eq!(stand_up_position(10, 20, Direction::Left), (9, 20));
        assert_eq!(stand_up_position(10, 20, Direction::Right), (11, 20));
        assert_eq!(stand_up_position(10, 20, Direction::DownLeft), (10, 20));
    }
}
