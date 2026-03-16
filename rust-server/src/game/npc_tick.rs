use super::GameRoom;
use super::npc_speech::{
    ChunkPlayerEntry, NpcSpeechEvent, check_npc_speech, nearby_players_for_speech,
};
use crate::chunk::{ChunkCoord, world_to_local};
use crate::npc::NpcUpdate;
use std::collections::{HashMap, HashSet};

type PlayerPosition = (String, i32, i32, i32);

pub(in crate::game) type NpcAttack = (String, String, i32, i32, i32);
pub(in crate::game) type NpcRespawn = (String, i32, i32);

pub(in crate::game) struct OverworldNpcTickResult {
    pub npc_updates: Vec<NpcUpdate>,
    pub respawned_npcs: Vec<NpcRespawn>,
    pub npc_attacks: Vec<NpcAttack>,
    pub npc_speech_events: Vec<NpcSpeechEvent>,
}

fn build_overworld_occupied_tiles(
    npc_positions: impl Iterator<Item = (i32, i32)>,
    player_positions: &[(String, i32, i32, i32)],
    portal_tiles: &HashSet<(i32, i32)>,
) -> HashSet<(i32, i32)> {
    let mut occupied_tiles: HashSet<(i32, i32)> = npc_positions.collect();

    for (_, px, py, _) in player_positions {
        occupied_tiles.insert((*px, *py));
    }

    occupied_tiles.extend(portal_tiles.iter().copied());
    occupied_tiles
}

fn blocked_move_log(old_pos: (i32, i32), new_pos: (i32, i32), npc_id: &str) -> Option<String> {
    if old_pos != new_pos {
        Some(format!(
            "BUG: NPC {} moved from {:?} to blocked tile {:?}!",
            npc_id, old_pos, new_pos
        ))
    } else {
        None
    }
}

impl GameRoom {
    pub(in crate::game) async fn process_overworld_npc_tick(
        &self,
        current_time: u64,
        delta_time: f32,
        player_positions: &[PlayerPosition],
        players_by_chunk: &HashMap<(i32, i32), Vec<ChunkPlayerEntry>>,
    ) -> OverworldNpcTickResult {
        let mut npc_updates = Vec::new();
        let mut respawned_npcs = Vec::new();
        let mut npc_attacks = Vec::new();
        let mut npc_speech_events = Vec::new();

        {
            let mut npcs = self.npcs.write().await;
            let chunks_guard = self.world.chunks_read().await;
            let walkable_check = |wx: i32, wy: i32| -> bool {
                let coord = ChunkCoord::from_world(wx, wy);
                if let Some(chunk) = chunks_guard.get(&coord) {
                    let (lx, ly) = world_to_local(wx, wy);
                    chunk.is_walkable_local(lx, ly)
                } else {
                    false
                }
            };
            let height_check = |wx: i32, wy: i32| -> i32 {
                let coord = ChunkCoord::from_world(wx, wy);
                if let Some(chunk) = chunks_guard.get(&coord) {
                    let (lx, ly) = world_to_local(wx, wy);
                    chunk.get_height(lx as u32, ly as u32) as i32
                } else {
                    0
                }
            };

            let mut occupied_tiles = build_overworld_occupied_tiles(
                npcs.values()
                    .filter(|npc| npc.is_alive())
                    .flat_map(|npc| npc.occupied_tiles()),
                player_positions,
                &self.portal_tiles,
            );

            for npc in npcs.values_mut() {
                if npc.ready_to_respawn(current_time) {
                    npc.respawn();
                    respawned_npcs.push((npc.id.clone(), npc.x, npc.y));
                    for tile in npc.occupied_tiles() {
                        occupied_tiles.insert(tile);
                    }
                }

                let old_pos = (npc.x, npc.y);
                for tile in npc.occupied_tiles() {
                    occupied_tiles.remove(&tile);
                }

                if let Some((target_id, max_hit)) = npc.update(
                    delta_time,
                    player_positions,
                    &occupied_tiles,
                    current_time,
                    &walkable_check,
                    &height_check,
                ) {
                    npc_attacks.push((
                        npc.id.clone(),
                        target_id,
                        npc.level,
                        max_hit,
                        npc.stats.attack_bonus,
                    ));
                }

                let new_pos = (npc.x, npc.y);
                if !walkable_check(npc.x, npc.y) {
                    if let Some(message) = blocked_move_log(old_pos, new_pos, &npc.id) {
                        tracing::error!("{}", message);
                    }
                }

                if npc.is_alive() {
                    for tile in npc.occupied_tiles() {
                        occupied_tiles.insert(tile);
                    }
                }

                npc.apply_regen(current_time);

                let nearby =
                    nearby_players_for_speech(players_by_chunk, npc.x, npc.y, npc.speech_radius);
                check_npc_speech(npc, &nearby, current_time, &mut npc_speech_events);

                npc_updates.push(NpcUpdate::from(&*npc));
            }
        }

        OverworldNpcTickResult {
            npc_updates,
            respawned_npcs,
            npc_attacks,
            npc_speech_events,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_overworld_occupied_tiles_combines_npcs_players_and_portals() {
        let player_positions = vec![
            ("char_1".to_string(), 10, 11, 20),
            ("char_2".to_string(), 15, 16, 20),
        ];
        let portal_tiles = HashSet::from([(1, 1), (2, 2)]);

        let occupied = build_overworld_occupied_tiles(
            vec![(3, 3), (4, 4)].into_iter(),
            &player_positions,
            &portal_tiles,
        );

        assert!(occupied.contains(&(3, 3)));
        assert!(occupied.contains(&(4, 4)));
        assert!(occupied.contains(&(10, 11)));
        assert!(occupied.contains(&(15, 16)));
        assert!(occupied.contains(&(1, 1)));
        assert!(occupied.contains(&(2, 2)));
    }

    #[test]
    fn blocked_move_log_only_reports_actual_move() {
        assert!(blocked_move_log((5, 5), (5, 5), "npc_1").is_none());
        assert_eq!(
            blocked_move_log((5, 5), (6, 5), "npc_1"),
            Some("BUG: NPC npc_1 moved from (5, 5) to blocked tile (6, 5)!".to_string())
        );
    }
}
