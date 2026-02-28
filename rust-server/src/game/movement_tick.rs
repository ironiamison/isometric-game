use super::{
    Direction, GameRoom, MOVE_COOLDOWN_TICKS, MOVE_INTENT_STALE_TIMEOUT_MS, TickTelemetry,
};
use std::collections::{HashMap, HashSet};

pub(in crate::game) struct MovementTickState {
    pub gathering_player_ids: HashSet<String>,
    pub moved_players: HashSet<String>,
    pub woodcutting_player_ids: HashSet<String>,
    pub woodcutting_stopped: Vec<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum MoveCheck {
    Valid,
    AutoSit,
    BlockedTile,
    BlockedPlayer,
    BlockedNpc,
    BlockedChair,
    BlockedArena,
}

fn movement_intent_is_stale(current_time: u64, last_move_input_ms: u64) -> bool {
    current_time.saturating_sub(last_move_input_ms) > MOVE_INTENT_STALE_TIMEOUT_MS
}

fn is_approaching_chair_from_front(move_dir: Direction, chair_dir: Direction) -> bool {
    match chair_dir {
        Direction::Down => move_dir == Direction::Up,
        Direction::Up => move_dir == Direction::Down,
        Direction::Left => move_dir == Direction::Right,
        Direction::Right => move_dir == Direction::Left,
        _ => false,
    }
}

impl GameRoom {
    pub(in crate::game) async fn process_player_movement_tick(
        &self,
        current_time: u64,
        current_tick: u64,
        tick_telemetry: &mut TickTelemetry,
    ) -> MovementTickState {
        {
            let mut players = self.players.write().await;
            for player in players
                .values_mut()
                .filter(|player| player.active && !player.is_dead)
            {
                if (player.move_dx == 0 && player.move_dy == 0) || player.pending_move_seq.is_none()
                {
                    continue;
                }

                if movement_intent_is_stale(current_time, player.last_move_input_ms) {
                    let stale_ms = current_time.saturating_sub(player.last_move_input_ms);
                    let seq = player.pending_move_seq;
                    tick_telemetry.movement_stale_intent_clears += 1;
                    tracing::warn!(
                        "Clearing stale move intent for {} after {}ms without input (seq={:?} pos=({}, {}) intent=({}, {}))",
                        player.id,
                        stale_ms,
                        seq,
                        player.x,
                        player.y,
                        player.move_dx,
                        player.move_dy
                    );
                    player.reject_pending_move();
                }
            }
        }

        let pending_moves: Vec<(String, i32, i32, i32, i32, u32)> = {
            let players = self.players.read().await;
            players
                .values()
                .filter(|player| player.active && !player.is_dead)
                .filter(|player| player.move_dx != 0 || player.move_dy != 0)
                .filter(|player| current_tick - player.last_move_tick >= MOVE_COOLDOWN_TICKS)
                .filter_map(|player| {
                    player.pending_move_seq.map(|seq| {
                        (
                            player.id.clone(),
                            player.x + player.move_dx,
                            player.y + player.move_dy,
                            player.move_dx,
                            player.move_dy,
                            seq,
                        )
                    })
                })
                .collect()
        };
        tick_telemetry.pending_moves = pending_moves.len();

        let pending_player_ids: Vec<String> = pending_moves
            .iter()
            .map(|(id, _, _, _, _, _)| id.clone())
            .collect();
        let pending_move_sequences: HashMap<String, u32> = pending_moves
            .iter()
            .map(|(id, _, _, _, _, seq)| (id.clone(), *seq))
            .collect();

        let player_instance_map: HashMap<String, String> = {
            let instances = self.player_instances.read().await;
            instances.clone()
        };

        let mut overworld_player_positions: HashSet<(i32, i32)> = HashSet::new();
        let mut instance_player_positions: HashMap<String, HashSet<(i32, i32)>> = HashMap::new();
        {
            let players = self.players.read().await;
            for player in players
                .values()
                .filter(|player| player.active && !player.is_dead)
            {
                if let Some(instance_id) = player_instance_map.get(&player.id) {
                    instance_player_positions
                        .entry(instance_id.clone())
                        .or_default()
                        .insert((player.x, player.y));
                } else {
                    overworld_player_positions.insert((player.x, player.y));
                }
            }
        }

        let npc_positions: HashSet<(i32, i32)> = {
            let npcs = self.npcs.read().await;
            npcs.values()
                .filter(|npc| npc.is_alive())
                .map(|npc| (npc.x, npc.y))
                .collect()
        };

        let chair_snapshot: HashMap<(i32, i32), (Option<String>, Direction)> = {
            let chairs = self.chairs.read().await;
            chairs
                .iter()
                .map(|(position, chair)| (*position, (chair.occupied_by.clone(), chair.direction)))
                .collect()
        };

        let mut instance_collision_snapshots: HashMap<String, (Vec<bool>, u32, u32)> =
            HashMap::new();
        let mut instance_npc_positions: HashMap<String, HashSet<(i32, i32)>> = HashMap::new();
        {
            let needed_instances: HashSet<&String> = pending_moves
                .iter()
                .filter_map(|(id, _, _, _, _, _)| player_instance_map.get(id))
                .collect();

            for instance_id in needed_instances {
                if let Some(instance) = self.instance_manager.get_by_instance_id(instance_id) {
                    let collision = instance.collision.read().await;
                    instance_collision_snapshots.insert(
                        instance_id.clone(),
                        (collision.clone(), instance.map_width, instance.map_height),
                    );

                    let npcs = instance.npcs.read().await;
                    let npc_positions: HashSet<(i32, i32)> = npcs
                        .values()
                        .filter(|npc| npc.is_alive())
                        .map(|npc| (npc.x, npc.y))
                        .collect();
                    instance_npc_positions.insert(instance_id.clone(), npc_positions);
                }
            }
        }

        let (arena_fighting, arena_ring_zone, arena_fighters) = {
            let arena = self.arena_manager.read().await;
            let fighting = arena.is_fighting();
            let ring_zone = arena.active_ring_zone().cloned();
            let fighters = arena
                .active_fighters
                .iter()
                .cloned()
                .collect::<HashSet<_>>();
            (fighting, ring_zone, fighters)
        };

        let chunks_guard = self.world.chunks_read().await;

        let mut check_move = |id: &str,
                              target_x: i32,
                              target_y: i32,
                              move_dir: Direction,
                              record_telemetry: bool|
         -> MoveCheck {
            let player_instance = player_instance_map.get(id);
            let is_overworld = player_instance.is_none();

            if is_overworld {
                let coord = crate::chunk::ChunkCoord::from_world(target_x, target_y);
                let walkable = if let Some(chunk) = chunks_guard.get(&coord) {
                    let (local_x, local_y) = crate::chunk::world_to_local(target_x, target_y);
                    chunk.is_walkable_local(local_x, local_y)
                } else {
                    false
                };
                if !walkable {
                    if record_telemetry {
                        tick_telemetry.rejected_tile_blocked += 1;
                    }
                    return MoveCheck::BlockedTile;
                }
            } else if let Some(instance_id) = player_instance {
                let walkable = if let Some((collision, map_width, map_height)) =
                    instance_collision_snapshots.get(instance_id)
                {
                    if target_x < 0
                        || target_y < 0
                        || target_x >= *map_width as i32
                        || target_y >= *map_height as i32
                    {
                        false
                    } else {
                        let index = (target_y as u32 * map_width + target_x as u32) as usize;
                        !collision.get(index).copied().unwrap_or(true)
                    }
                } else {
                    false
                };
                if !walkable {
                    if record_telemetry {
                        tick_telemetry.rejected_tile_blocked += 1;
                    }
                    return MoveCheck::BlockedTile;
                }
            }

            let player_blocked = if is_overworld {
                overworld_player_positions.contains(&(target_x, target_y))
            } else {
                player_instance
                    .and_then(|instance_id| instance_player_positions.get(instance_id))
                    .is_some_and(|positions| positions.contains(&(target_x, target_y)))
            };
            if player_blocked {
                if record_telemetry {
                    tick_telemetry.rejected_player_blocked += 1;
                }
                return MoveCheck::BlockedPlayer;
            }

            let npc_blocked = if is_overworld {
                npc_positions.contains(&(target_x, target_y))
            } else if let Some(instance_id) = player_instance {
                instance_npc_positions
                    .get(instance_id)
                    .is_some_and(|positions| positions.contains(&(target_x, target_y)))
            } else {
                false
            };
            if npc_blocked {
                if record_telemetry {
                    tick_telemetry.rejected_npc_blocked += 1;
                }
                return MoveCheck::BlockedNpc;
            }

            if is_overworld {
                if let Some((occupied_by, chair_dir)) = chair_snapshot.get(&(target_x, target_y)) {
                    if record_telemetry {
                        tick_telemetry.rejected_chair_blocked += 1;
                    }
                    if occupied_by.is_none()
                        && is_approaching_chair_from_front(move_dir, *chair_dir)
                    {
                        return MoveCheck::AutoSit;
                    }
                    return MoveCheck::BlockedChair;
                }
            }

            if arena_fighting && arena_fighters.contains(id) {
                if let Some(ring_zone) = &arena_ring_zone {
                    if !ring_zone.contains(target_x, target_y) {
                        if record_telemetry {
                            tick_telemetry.rejected_arena_blocked += 1;
                        }
                        return MoveCheck::BlockedArena;
                    }
                }
            }

            MoveCheck::Valid
        };

        let mut valid_moves = Vec::new();
        let mut auto_sit_requests = Vec::new();
        for (id, target_x, target_y, sampled_dx, sampled_dy, sampled_seq) in pending_moves {
            let move_dir = Direction::from_velocity(sampled_dx as f32, sampled_dy as f32);
            match check_move(&id, target_x, target_y, move_dir, true) {
                MoveCheck::Valid => {
                    valid_moves.push((id, target_x, target_y, sampled_dx, sampled_dy, sampled_seq));
                }
                MoveCheck::AutoSit => {
                    auto_sit_requests.push((id, target_x, target_y, sampled_seq));
                }
                _ => {}
            }
        }

        let current_pending_seqs: HashMap<String, u32> = {
            let players = self.players.read().await;
            players
                .values()
                .filter_map(|player| player.pending_move_seq.map(|seq| (player.id.clone(), seq)))
                .collect()
        };
        valid_moves.retain(|(id, _, _, _, _, sampled_seq)| {
            current_pending_seqs.get(id).copied() == Some(*sampled_seq)
        });

        let gathering_player_ids = {
            let gathering = self.gathering.read().await;
            gathering.gathering_player_ids()
        };

        let mut moved_players = HashSet::new();
        let mut moved_positions = Vec::new();

        {
            let mut players = self.players.write().await;

            for (id, target_x, target_y, sampled_dx, sampled_dy, sampled_seq) in valid_moves {
                if let Some(player) = players.get_mut(&id) {
                    if player.pending_move_seq != Some(sampled_seq) {
                        if let Some(new_seq) = player.pending_move_seq {
                            if current_tick - player.last_move_tick >= MOVE_COOLDOWN_TICKS {
                                let new_dx = player.move_dx;
                                let new_dy = player.move_dy;
                                if new_dx != 0 || new_dy != 0 {
                                    let new_target_x = player.x + new_dx;
                                    let new_target_y = player.y + new_dy;
                                    let move_dir =
                                        Direction::from_velocity(new_dx as f32, new_dy as f32);
                                    match check_move(
                                        &id,
                                        new_target_x,
                                        new_target_y,
                                        move_dir,
                                        true,
                                    ) {
                                        MoveCheck::Valid => {
                                            player.direction = move_dir;
                                            player.x = new_target_x;
                                            player.y = new_target_y;
                                            player.last_move_tick = current_tick;
                                            player.mark_move_seq_processed(new_seq);
                                            moved_players.insert(id.clone());
                                            if !self.quest_locations.is_empty() {
                                                moved_positions.push((
                                                    id.clone(),
                                                    new_target_x,
                                                    new_target_y,
                                                ));
                                            }
                                            if player.pending_move_seq == Some(new_seq) {
                                                player.clear_move_intent();
                                            }
                                        }
                                        MoveCheck::AutoSit => {
                                            auto_sit_requests.push((
                                                id.clone(),
                                                new_target_x,
                                                new_target_y,
                                                new_seq,
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    if sampled_dx != 0 || sampled_dy != 0 {
                        player.direction =
                            Direction::from_velocity(sampled_dx as f32, sampled_dy as f32);
                    }
                    player.x = target_x;
                    player.y = target_y;
                    player.last_move_tick = current_tick;
                    player.mark_move_seq_processed(sampled_seq);
                    moved_players.insert(id.clone());
                    if !self.quest_locations.is_empty() {
                        moved_positions.push((id.clone(), target_x, target_y));
                    }

                    if player.pending_move_seq == Some(sampled_seq) {
                        player.clear_move_intent();
                    }
                }
            }

            for player_id in &pending_player_ids {
                if !moved_players.contains(player_id) {
                    if let Some(player) = players.get_mut(player_id) {
                        if let Some(sampled_seq) = pending_move_sequences.get(player_id) {
                            player.mark_move_seq_processed(*sampled_seq);
                            if player.pending_move_seq == Some(*sampled_seq) {
                                player.clear_move_intent();
                            }
                        }
                    }
                }
            }

            for player in players.values_mut() {
                let active_ids: Vec<String> = player.active_prayers.iter().cloned().collect();
                let prayer_effects = self.prayer_registry.calculate_effects(&active_ids);
                player.apply_regen(current_time, prayer_effects.hp_regen_multiplier);
            }

            for player in players.values_mut() {
                if player.active {
                    player.tick_buffs(current_time);
                }
            }
        }

        drop(chunks_guard);

        for (id, tile_x, tile_y, sampled_seq) in auto_sit_requests {
            if current_pending_seqs.get(&id).copied() == Some(sampled_seq) {
                self.handle_sit_chair(&id, tile_x, tile_y).await;
            }
        }

        tick_telemetry.rejected_moves =
            pending_player_ids.len().saturating_sub(moved_players.len());

        let (woodcutting_player_ids, woodcutting_stopped) = {
            let mut woodcutting = self.woodcutting.write().await;
            let mut stopped = Vec::new();
            for id in &moved_players {
                if woodcutting.is_woodcutting(id) {
                    woodcutting.stop_woodcutting(id);
                    stopped.push(id.clone());
                }
            }
            let ids = woodcutting.woodcutting_player_ids();
            (ids, stopped)
        };

        for (player_id, x, y) in &moved_positions {
            for (location_id, location) in &self.quest_locations {
                let dx = (x - location.x).abs();
                let dy = (y - location.y).abs();
                if dx <= location.radius && dy <= location.radius {
                    self.process_quest_location_reached(player_id, location_id, *x, *y)
                        .await;
                }
            }
        }

        MovementTickState {
            gathering_player_ids,
            moved_players,
            woodcutting_player_ids,
            woodcutting_stopped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_intent_is_stale_only_after_timeout() {
        assert!(!movement_intent_is_stale(1_000, 400));
        assert!(!movement_intent_is_stale(1_000, 300));
        assert!(movement_intent_is_stale(1_001, 300));
    }

    #[test]
    fn chair_front_check_only_accepts_opposite_cardinal_direction() {
        assert!(is_approaching_chair_from_front(
            Direction::Up,
            Direction::Down
        ));
        assert!(is_approaching_chair_from_front(
            Direction::Down,
            Direction::Up
        ));
        assert!(is_approaching_chair_from_front(
            Direction::Left,
            Direction::Right
        ));
        assert!(is_approaching_chair_from_front(
            Direction::Right,
            Direction::Left
        ));
        assert!(!is_approaching_chair_from_front(
            Direction::Down,
            Direction::Down
        ));
        assert!(!is_approaching_chair_from_front(
            Direction::UpLeft,
            Direction::Down
        ));
    }
}
