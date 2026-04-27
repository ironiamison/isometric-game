use super::GameRoom;
use super::npc_speech::{NpcSpeechEvent, check_npc_speech};
use super::npc_tick::NpcAttack;
use std::collections::{HashMap, HashSet};

type InstancePlayerPosition = (String, i32, i32);
type InstancePlayerTickPosition = (String, i32, i32, i32);

/// Minion explosion contact: (npc_id, instance_id, npc_x, npc_y)
pub(in crate::game) type MinionContactExplosion = (String, String, i32, i32);

pub(in crate::game) struct InstanceNpcTickResult {
    pub npc_attacks: Vec<NpcAttack>,
    pub speech_events: Vec<NpcSpeechEvent>,
    pub minion_explosions: Vec<MinionContactExplosion>,
}

fn build_instance_occupied_tiles(
    npc_positions: impl Iterator<Item = (i32, i32)>,
    player_positions: &[(String, i32, i32, i32)],
) -> HashSet<(i32, i32)> {
    let mut occupied_tiles: HashSet<(i32, i32)> = npc_positions.collect();

    for (_, px, py, _) in player_positions {
        occupied_tiles.insert((*px, *py));
    }

    occupied_tiles
}

fn speech_refs_for_instance_players(
    player_positions: &[(String, i32, i32, i32)],
) -> Vec<(&str, i32, i32)> {
    player_positions
        .iter()
        .map(|(player_id, x, y, _)| (player_id.as_str(), *x, *y))
        .collect()
}

impl GameRoom {
    async fn collect_instance_players(&self) -> HashMap<String, Vec<InstancePlayerPosition>> {
        let players = self.players.read().await;
        let player_inst = self.player_instances.read().await;
        let mut instance_players: HashMap<String, Vec<InstancePlayerPosition>> = HashMap::new();

        for (player_id, instance_id) in player_inst.iter() {
            if let Some(player) = players.get(player_id) {
                if player.active && player.is_alive() {
                    instance_players
                        .entry(instance_id.clone())
                        .or_default()
                        .push((player_id.clone(), player.x, player.y));
                }
            }
        }

        instance_players
    }

    pub(in crate::game) async fn process_instance_npc_tick(
        &self,
        current_time: u64,
        delta_time: f32,
    ) -> InstanceNpcTickResult {
        let instance_players = self.collect_instance_players().await;
        let mut npc_attacks = Vec::new();
        let mut speech_events = Vec::new();
        let mut minion_explosions: Vec<MinionContactExplosion> = Vec::new();

        for entry in self
            .instance_manager
            .public_instances
            .iter()
            .map(|entry| entry.value().clone())
            .chain(
                self.instance_manager
                    .private_instances
                    .iter()
                    .map(|entry| entry.value().clone()),
            )
        {
            let inst_player_list: Vec<InstancePlayerTickPosition> =
                if let Some(inst_players) = instance_players.get(&entry.id) {
                    let players_guard = self.players.read().await;
                    inst_players
                        .iter()
                        .filter_map(|(player_id, x, y)| {
                            players_guard
                                .get(player_id)
                                .map(|player| (player_id.clone(), *x, *y, player.hp))
                        })
                        .collect()
                } else {
                    continue;
                };

            let collision = entry.collision.read().await;
            let heightmap = entry.heightmap.read().await;
            let walkable_check = |world_x: i32, world_y: i32| -> bool {
                entry.is_walkable_sync(&collision, world_x, world_y)
            };

            let mut npcs = entry.npcs.write().await;
            let mut occupied_tiles = build_instance_occupied_tiles(
                npcs.values()
                    .filter(|npc| npc.is_alive())
                    .flat_map(|npc| npc.occupied_tiles()),
                &inst_player_list,
            );

            for npc in npcs.values_mut() {
                if npc.ready_to_respawn(current_time) {
                    npc.respawn();
                    for tile in npc.occupied_tiles() {
                        occupied_tiles.insert(tile);
                    }
                }

                // Skip AI for hidden NPCs (e.g. boss underground)
                if npc.hidden {
                    continue;
                }

                for tile in npc.occupied_tiles() {
                    occupied_tiles.remove(&tile);
                }

                let height_check =
                    |wx: i32, wy: i32| -> i32 { entry.get_height_at_sync(&heightmap, wx, wy) };
                if let Some((target_id, max_hit)) = npc.update(
                    delta_time,
                    &inst_player_list,
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

                // Check explosive minion proximity with players (explode within attack range)
                if npc.is_alive() && npc.prototype_id == "wurm_minion" {
                    let attack_range = npc.stats.attack_range;
                    for (_player_id, px, py, _php) in &inst_player_list {
                        let dist = npc
                            .occupied_tiles()
                            .map(|(tx, ty)| (tx - *px).abs().max((ty - *py).abs()))
                            .min()
                            .unwrap_or(i32::MAX);
                        if dist <= attack_range {
                            // In range! Kill the minion to trigger explosion
                            npc.hp = 0;
                            npc.state = crate::npc::NpcState::Dead;
                            npc.death_time = current_time;
                            minion_explosions.push((
                                npc.id.clone(),
                                entry.id.clone(),
                                npc.x,
                                npc.y,
                            ));
                            break;
                        }
                    }
                }

                if npc.is_alive() {
                    for tile in npc.occupied_tiles() {
                        occupied_tiles.insert(tile);
                    }
                }

                npc.apply_regen(current_time);

                let player_refs = speech_refs_for_instance_players(&inst_player_list);
                check_npc_speech(npc, &player_refs, current_time, &mut speech_events);
            }

            // Remove dead NPCs that won't respawn after 2 seconds (allows death animation)
            // Skip boss NPCs — they are reset by the boss state machine's TeleportOut handler
            const DEAD_CLEANUP_MS: u64 = 2000;
            npcs.retain(|_, npc| {
                if npc.state == crate::npc::NpcState::Dead
                    && npc.get_respawn_time_ms() == 0
                    && current_time.saturating_sub(npc.death_time) >= DEAD_CLEANUP_MS
                    && npc.prototype_id != "desert_wurm"
                {
                    return false;
                }
                true
            });
        }

        InstanceNpcTickResult {
            npc_attacks,
            speech_events,
            minion_explosions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_instance_occupied_tiles_includes_npcs_and_players() {
        let player_positions = vec![
            ("char_1".to_string(), 3, 4, 10),
            ("char_2".to_string(), 8, 9, 12),
        ];

        let occupied =
            build_instance_occupied_tiles(vec![(1, 1), (2, 2)].into_iter(), &player_positions);

        assert!(occupied.contains(&(1, 1)));
        assert!(occupied.contains(&(2, 2)));
        assert!(occupied.contains(&(3, 4)));
        assert!(occupied.contains(&(8, 9)));
    }

    #[test]
    fn speech_refs_for_instance_players_preserves_ids_and_positions() {
        let player_positions = vec![
            ("char_1".to_string(), 3, 4, 10),
            ("char_2".to_string(), 8, 9, 12),
        ];

        let refs = speech_refs_for_instance_players(&player_positions);

        assert_eq!(refs, vec![("char_1", 3, 4), ("char_2", 8, 9)]);
    }
}
