use super::{GameRoom, MANA_REGEN_INTERVAL_TICKS, PRAYER_DRAIN_INTERVAL_TICKS};
use crate::protocol::ServerMessage;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrayerDrainUpdate {
    player_id: String,
    new_points: i32,
    max_points: i32,
    active_prayers: Vec<String>,
    depleted: bool,
}

fn mana_regen_amount(magic_level: i32) -> i32 {
    1 + (magic_level / 30)
}

fn next_mana_points(current_mp: i32, max_mp: i32, magic_level: i32) -> i32 {
    if current_mp >= max_mp {
        current_mp
    } else {
        (current_mp + mana_regen_amount(magic_level)).min(max_mp)
    }
}

fn apply_prayer_drain(prayer_points: i32, drain_amount: i32) -> (i32, bool) {
    if drain_amount <= 0 {
        return (prayer_points, false);
    }

    let new_points = (prayer_points - drain_amount).max(0);
    let depleted = new_points == 0 && prayer_points > 0;
    (new_points, depleted)
}

fn prayer_state_update_message(update: &PrayerDrainUpdate) -> ServerMessage {
    ServerMessage::PrayerStateUpdate {
        points: update.new_points,
        max_points: update.max_points,
        active_prayers: update.active_prayers.clone(),
    }
}

impl GameRoom {
    pub(in crate::game) async fn process_player_resource_ticks(&self, current_tick: u64) -> u128 {
        if current_tick.is_multiple_of(MANA_REGEN_INTERVAL_TICKS) {
            let mut players = self.players.write().await;
            for player in players.values_mut() {
                if !player.active || player.is_dead {
                    continue;
                }

                let max_mp = player.max_mp();
                player.mp = next_mana_points(player.mp, max_mp, player.skills.magic.level);
            }
        }

        if !current_tick.is_multiple_of(PRAYER_DRAIN_INTERVAL_TICKS) {
            return 0;
        }

        let drain_start = std::time::Instant::now();
        let updates: Vec<PrayerDrainUpdate> = {
            let mut players = self.players.write().await;
            let mut updates = Vec::new();

            for player in players.values_mut() {
                if !player.active || player.is_dead || player.active_prayers.is_empty() {
                    continue;
                }

                let active_ids: Vec<String> = player.active_prayers.iter().cloned().collect();
                let effects = self.prayer_registry.calculate_effects(&active_ids);
                let drain_amount = effects.total_drain_rate.ceil() as i32;

                if drain_amount <= 0 {
                    continue;
                }

                let (new_points, depleted) = apply_prayer_drain(player.prayer_points, drain_amount);
                player.prayer_points = new_points;

                if depleted {
                    player.active_prayers.clear();
                    tracing::debug!(
                        "Player {} ran out of prayer points, all prayers deactivated",
                        player.id
                    );
                }

                updates.push(PrayerDrainUpdate {
                    player_id: player.id.clone(),
                    new_points: player.prayer_points,
                    max_points: player.max_prayer_points(),
                    active_prayers: player.active_prayers.iter().cloned().collect(),
                    depleted,
                });
            }

            updates
        };

        for update in updates {
            self.send_to_player(&update.player_id, prayer_state_update_message(&update))
                .await;

            if update.depleted {
                self.send_system_message(&update.player_id, "You have run out of prayer points")
                    .await;
            }
        }

        drain_start.elapsed().as_millis()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_mana_points_scales_with_magic_level_and_caps_at_max() {
        assert_eq!(next_mana_points(4, 20, 1), 5);
        assert_eq!(next_mana_points(4, 20, 30), 6);
        assert_eq!(next_mana_points(19, 20, 90), 20);
        assert_eq!(next_mana_points(20, 20, 90), 20);
    }

    #[test]
    fn apply_prayer_drain_zeroes_points_and_marks_depleted_once() {
        assert_eq!(apply_prayer_drain(5, 2), (3, false));
        assert_eq!(apply_prayer_drain(5, 5), (0, true));
        assert_eq!(apply_prayer_drain(0, 5), (0, false));
        assert_eq!(apply_prayer_drain(5, 0), (5, false));
    }

    #[test]
    fn prayer_state_update_message_preserves_points_and_active_set() {
        let update = PrayerDrainUpdate {
            player_id: "char_7".to_string(),
            new_points: 3,
            max_points: 17,
            active_prayers: vec!["thick_skin".to_string(), "burst_of_strength".to_string()],
            depleted: false,
        };

        match prayer_state_update_message(&update) {
            ServerMessage::PrayerStateUpdate {
                points,
                max_points,
                active_prayers,
            } => {
                assert_eq!(points, 3);
                assert_eq!(max_points, 17);
                assert_eq!(
                    active_prayers,
                    vec!["thick_skin".to_string(), "burst_of_strength".to_string()]
                );
            }
            other => panic!("expected PrayerStateUpdate, got {:?}", other),
        }
    }
}
