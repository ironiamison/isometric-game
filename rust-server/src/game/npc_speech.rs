use super::GameRoom;
use crate::npc::Npc;
use crate::protocol::ServerMessage;
use std::collections::HashMap;

pub(in crate::game) type NpcSpeechEvent = (String, String, String);
pub(in crate::game) type ChunkPlayerEntry = (String, i32, i32);

fn npc_speech_message(npc_id: &str, message: &str) -> ServerMessage {
    ServerMessage::NpcSpeech {
        npc_id: npc_id.to_string(),
        message: message.to_string(),
    }
}

pub(in crate::game) fn nearby_players_for_speech<'a>(
    players_by_chunk: &'a HashMap<(i32, i32), Vec<ChunkPlayerEntry>>,
    npc_x: i32,
    npc_y: i32,
    speech_radius: i32,
) -> Vec<(&'a str, i32, i32)> {
    use crate::chunk::CHUNK_SIZE;

    let chunk_radius = (speech_radius as f32 / CHUNK_SIZE as f32).ceil() as i32;
    let npc_cx = npc_x.div_euclid(CHUNK_SIZE as i32);
    let npc_cy = npc_y.div_euclid(CHUNK_SIZE as i32);
    let mut nearby = Vec::new();

    for dx in -chunk_radius..=chunk_radius {
        for dy in -chunk_radius..=chunk_radius {
            if let Some(players) = players_by_chunk.get(&(npc_cx + dx, npc_cy + dy)) {
                nearby.extend(players.iter().map(|(pid, px, py)| (pid.as_str(), *px, *py)));
            }
        }
    }

    nearby
}

pub(in crate::game) fn check_npc_speech(
    npc: &mut Npc,
    nearby_players: &[(&str, i32, i32)],
    current_time: u64,
    speech_events: &mut Vec<NpcSpeechEvent>,
) {
    let messages = match npc.speech_messages {
        Some(ref m) if !m.is_empty() && npc.is_alive() => m,
        _ => return,
    };

    let radius = npc.speech_radius;
    let npc_x = npc.x;
    let npc_y = npc.y;
    let recipients: Vec<&str> = nearby_players
        .iter()
        .filter(|(_, px, py)| {
            let dx = (npc_x - px).abs();
            let dy = (npc_y - py).abs();
            dx.max(dy) <= radius
        })
        .map(|(pid, _, _)| *pid)
        .collect();

    if recipients.is_empty() {
        npc.next_speech_at = 0;
        return;
    }

    if npc.next_speech_at == 0 {
        let delay = npc.speech_interval_min_ms
            + (rand::random::<u64>()
                % (npc.speech_interval_max_ms - npc.speech_interval_min_ms + 1));
        npc.next_speech_at = current_time + delay;
    } else if current_time >= npc.next_speech_at {
        let idx = rand::random::<usize>() % messages.len();
        let message = &messages[idx];
        let npc_id = &npc.id;
        for pid in &recipients {
            speech_events.push((pid.to_string(), npc_id.clone(), message.clone()));
        }
        let delay = npc.speech_interval_min_ms
            + (rand::random::<u64>()
                % (npc.speech_interval_max_ms - npc.speech_interval_min_ms + 1));
        npc.next_speech_at = current_time + delay;
    }
}

impl GameRoom {
    pub(in crate::game) async fn send_npc_speech_events(&self, events: Vec<NpcSpeechEvent>) {
        for (player_id, npc_id, message) in events {
            self.send_to_player(&player_id, npc_speech_message(&npc_id, &message))
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::AnimationType;
    use crate::entity::prototype::{
        DialogueConfig, EntityBehaviors, EntityPrototype, ResolvedRewards, ResolvedStats,
        SpeechConfig,
    };

    fn speaking_test_npc() -> Npc {
        let prototype = EntityPrototype {
            id: "test_npc".to_string(),
            display_name: "Test NPC".to_string(),
            sprite: "test".to_string(),
            animation_type: AnimationType::Blob,
            description: "Test".to_string(),
            stats: ResolvedStats {
                level: 1,
                max_hp: 10,
                damage: 1,
                attack_bonus: 0,
                defence_bonus: 0,
                attack_range: 1,
                aggro_range: 0,
                chase_range: 0,
                move_cooldown_ms: 500,
                attack_cooldown_ms: 500,
                respawn_time_ms: 1000,
                hp_regen_percent_per_sec: 0.0,
            },
            rewards: ResolvedRewards::default(),
            loot: Vec::new(),
            behaviors: EntityBehaviors {
                friendly: true,
                ..EntityBehaviors::default()
            },
            merchant: None,
            quest_giver: None,
            dialogue: DialogueConfig::default(),
            speech: Some(SpeechConfig {
                radius: 2,
                interval_min_ms: 10,
                interval_max_ms: 10,
                messages: vec!["Hello".to_string()],
            }),
        };

        Npc::from_prototype("npc_1", "test_npc", &prototype, 10, 10, 1, None)
    }

    #[test]
    fn nearby_players_for_speech_collects_from_neighboring_chunks() {
        let players_by_chunk = HashMap::from([
            ((0, 0), vec![("char_1".to_string(), 15, 4)]),
            ((1, 0), vec![("char_2".to_string(), 32, 5)]),
            ((2, 0), vec![("char_3".to_string(), 64, 5)]),
        ]);

        let nearby = nearby_players_for_speech(&players_by_chunk, 31, 4, 64);
        let ids: Vec<&str> = nearby.into_iter().map(|(id, _, _)| id).collect();

        assert!(ids.contains(&"char_1"));
        assert!(ids.contains(&"char_2"));
        assert!(ids.contains(&"char_3"));
    }

    #[test]
    fn check_npc_speech_sets_timer_then_emits_when_due() {
        let mut npc = speaking_test_npc();
        let nearby = vec![("char_1", 10, 11)];
        let mut events = Vec::new();

        check_npc_speech(&mut npc, &nearby, 100, &mut events);
        assert!(events.is_empty());
        assert_eq!(npc.next_speech_at, 110);

        check_npc_speech(&mut npc, &nearby, 110, &mut events);
        assert_eq!(
            events,
            vec![(
                "char_1".to_string(),
                "npc_1".to_string(),
                "Hello".to_string()
            )]
        );
        assert_eq!(npc.next_speech_at, 120);
    }
}
