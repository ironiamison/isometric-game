use super::*;

type GridPosition = (i32, i32);
type Path = Vec<GridPosition>;
type SplicePathCandidate = (i32, SpliceCandidate, Path);
type SpliceDestinationCandidate = (i32, SpliceCandidate, GridPosition, Path);

pub(super) fn sync_path_index(path_state: &mut PathState, player_pos: (i32, i32)) {
    while path_state.current_index < path_state.path.len() {
        let (wx, wy) = path_state.path[path_state.current_index];
        if (wx, wy) == player_pos {
            path_state.current_index += 1;
        } else {
            break;
        }
    }

    if path_state.current_index < path_state.path.len() {
        if let Some(found_idx) = path_state
            .path
            .iter()
            .enumerate()
            .skip(path_state.current_index)
            .find_map(|(i, &(wx, wy))| {
                if (wx, wy) == player_pos {
                    Some(i)
                } else {
                    None
                }
            })
        {
            path_state.current_index = found_idx + 1;
        }
    }
}

/// Build set of tiles occupied by entities (other players + NPCs) for pathfinding
pub(super) fn build_occupied_set(
    state: &GameState,
    include_chairs: bool,
    include_players: bool,
) -> HashSet<(i32, i32)> {
    let mut occupied = HashSet::new();

    // When in interior mode, don't count overworld players as obstacles
    // (they shouldn't be in our instance anyway)
    let in_interior = state.current_interior.is_some();

    // Add other players (not local player)
    // Skip if in interior - we'll only see players in our instance from server updates
    if include_players && !in_interior {
        for (id, player) in &state.players {
            if state.local_player_id.as_ref() == Some(id) {
                continue;
            }
            if !player.is_dead {
                // Use server-authoritative coordinates to match server-side collision checks.
                occupied.insert((
                    player.server_x.round() as i32,
                    player.server_y.round() as i32,
                ));
            }
        }
    }

    // Add all alive NPCs
    for npc in state.npcs.values() {
        if npc.is_alive() {
            // Use server-authoritative coordinates to avoid interpolation skew.
            occupied.insert((npc.server_x.round() as i32, npc.server_y.round() as i32));
        }
    }

    if include_chairs {
        for (cx, cy) in &state.chair_positions {
            occupied.insert((*cx, *cy));
        }
    }

    occupied
}

pub(super) fn preferred_adjacent_tile_for_target(
    state: &GameState,
    target: (i32, i32),
) -> Option<(i32, i32)> {
    let player = state.get_local_player()?;
    let dx = player.x - target.0 as f32;
    let dy = player.y - target.1 as f32;

    if dx.abs() < 0.01 && dy.abs() < 0.01 {
        return None;
    }

    if dx.abs() >= dy.abs() {
        Some((target.0 + if dx > 0.0 { 1 } else { -1 }, target.1))
    } else {
        Some((target.0, target.1 + if dy > 0.0 { 1 } else { -1 }))
    }
}

#[derive(Clone)]
struct SpliceCandidate {
    pos: (i32, i32),
    steps_to_pos: i32,
    prefix_range: Option<(usize, usize)>,
}

fn splice_candidates(
    state: &GameState,
    start: (i32, i32),
    max_splice_ahead: usize,
) -> Vec<SpliceCandidate> {
    let mut candidates = Vec::new();
    candidates.push(SpliceCandidate {
        pos: start,
        steps_to_pos: 0,
        prefix_range: None,
    });

    let Some(path_state) = state.auto_path.as_ref() else {
        return candidates;
    };

    if path_state.current_index >= path_state.path.len() {
        return candidates;
    }

    let max_idx = (path_state.current_index + max_splice_ahead).min(path_state.path.len() - 1);
    let start_is_next = path_state.path[path_state.current_index] == start;
    let base_steps: i32 = if start_is_next { 0 } else { 1 };

    for i in path_state.current_index..=max_idx {
        let pos = path_state.path[i];
        if pos == start {
            continue;
        }
        let steps_to_pos = base_steps + (i as i32 - path_state.current_index as i32);
        candidates.push(SpliceCandidate {
            pos,
            steps_to_pos,
            prefix_range: Some((path_state.current_index, i)),
        });
    }

    candidates
}

pub(super) fn find_path_with_optimistic_splice(
    state: &GameState,
    start: (i32, i32),
    goal: (i32, i32),
    occupied: &HashSet<(i32, i32)>,
    max_distance: i32,
) -> Option<Path> {
    find_path_with_limited_splice(state, start, goal, occupied, max_distance, 6)
}

// For plain click-to-move, only preserve the currently committed next tile.
// Splicing too far ahead into an old route can create loops/backtracking when
// the player rapidly retargets destinations.
pub(super) fn find_path_with_committed_step_splice(
    state: &GameState,
    start: (i32, i32),
    goal: (i32, i32),
    occupied: &HashSet<(i32, i32)>,
    max_distance: i32,
) -> Option<Path> {
    find_path_with_limited_splice(state, start, goal, occupied, max_distance, 1)
}

pub(super) fn find_path_with_limited_splice(
    state: &GameState,
    start: (i32, i32),
    goal: (i32, i32),
    occupied: &HashSet<(i32, i32)>,
    max_distance: i32,
    max_splice_ahead: usize,
) -> Option<Path> {
    let candidates = splice_candidates(state, start, max_splice_ahead);
    let path_state = state.auto_path.as_ref();

    let mut best: Option<SplicePathCandidate> = None;

    for cand in candidates {
        if let Some(path) =
            pathfinding::find_path(cand.pos, goal, &state.chunk_manager, occupied, max_distance)
        {
            let steps_from_pos = path.len().saturating_sub(1) as i32;
            let total_steps = cand.steps_to_pos + steps_from_pos;
            let better = match &best {
                None => true,
                Some((best_total, best_cand, _)) => {
                    total_steps < *best_total
                        || (total_steps == *best_total
                            && cand.steps_to_pos > best_cand.steps_to_pos)
                }
            };
            if better {
                best = Some((total_steps, cand.clone(), path));
            }
        }
    }

    let (_, cand, path) = best?;

    if let (Some((start_idx, end_idx)), Some(path_state)) = (cand.prefix_range, path_state) {
        let mut combined = Vec::new();
        combined.extend_from_slice(&path_state.path[start_idx..=end_idx]);
        if path.len() > 1 {
            combined.extend_from_slice(&path[1..]);
        }
        return Some(combined);
    }

    Some(path)
}

/// Get the attack range for the local player's equipped weapon (1 for melee/unarmed, >1 for ranged).
pub(super) fn get_local_weapon_range(state: &GameState) -> i32 {
    let weapon_id = state
        .local_player_id
        .as_ref()
        .and_then(|id| state.players.get(id))
        .and_then(|p| p.equipped_weapon.as_ref());
    if let Some(weapon_id) = weapon_id {
        if let Some(item_def) = state.item_registry.get(weapon_id) {
            return item_def.range.unwrap_or(1);
        }
    }
    1
}

/// Check if a position is within attack range of a target (matches server logic).
/// Uses Manhattan distance (diamond shape) for all ranges.
pub(super) fn in_attack_range(px: i32, py: i32, tx: i32, ty: i32, weapon_range: i32) -> bool {
    let dx = (px - tx).abs();
    let dy = (py - ty).abs();
    if weapon_range == 1 {
        (dx + dy) == 1 // Cardinal adjacency for melee
    } else {
        (dx + dy) <= weapon_range && (dx > 0 || dy > 0) // Manhattan for ranged
    }
}

pub(super) fn find_path_to_attack_with_optimistic_splice(
    state: &GameState,
    start: (i32, i32),
    target: (i32, i32),
    occupied: &HashSet<(i32, i32)>,
    max_distance: i32,
    weapon_range: i32,
) -> Option<pathfinding::DestinationPath> {
    if weapon_range <= 1 {
        let preferred = preferred_adjacent_tile_for_target(state, target);
        return find_path_to_adjacent_with_optimistic_splice(
            state,
            start,
            target,
            occupied,
            max_distance,
            preferred,
        );
    }

    // For ranged: use optimistic splice candidates with range-based pathfinding
    let candidates = splice_candidates(state, start, 6);

    let mut best: Option<SpliceDestinationCandidate> = None;

    for cand in candidates {
        if let Some((dest, path)) = pathfinding::find_path_within_range(
            cand.pos,
            target,
            &state.chunk_manager,
            occupied,
            max_distance,
            weapon_range,
        ) {
            let steps_from_pos = path.len().saturating_sub(1) as i32;
            let total_steps = cand.steps_to_pos + steps_from_pos;
            let better = match &best {
                None => true,
                Some((best_total, best_cand, _, _)) => {
                    total_steps < *best_total
                        || (total_steps == *best_total
                            && cand.steps_to_pos > best_cand.steps_to_pos)
                }
            };
            if better {
                best = Some((total_steps, cand.clone(), dest, path));
            }
        }
    }

    let (_, cand, dest, path) = best?;

    let path_state = state.auto_path.as_ref();
    if let (Some((start_idx, end_idx)), Some(path_state)) = (cand.prefix_range, path_state) {
        let mut combined = Vec::new();
        combined.extend_from_slice(&path_state.path[start_idx..=end_idx]);
        if path.len() > 1 {
            combined.extend_from_slice(&path[1..]);
        }
        return Some((dest, combined));
    }

    Some((dest, path))
}

pub(super) fn find_path_to_adjacent_with_optimistic_splice(
    state: &GameState,
    start: (i32, i32),
    target: (i32, i32),
    occupied: &HashSet<(i32, i32)>,
    max_distance: i32,
    preferred_adjacent: Option<(i32, i32)>,
) -> Option<pathfinding::DestinationPath> {
    let candidates = splice_candidates(state, start, 6);
    let path_state = state.auto_path.as_ref();

    let mut best: Option<SpliceDestinationCandidate> = None;

    for cand in candidates {
        if let Some((dest, path)) = pathfinding::find_path_to_adjacent_prefer(
            cand.pos,
            target,
            &state.chunk_manager,
            occupied,
            max_distance,
            preferred_adjacent,
        ) {
            let steps_from_pos = path.len().saturating_sub(1) as i32;
            let total_steps = cand.steps_to_pos + steps_from_pos;
            let better = match &best {
                None => true,
                Some((best_total, best_cand, _, _)) => {
                    total_steps < *best_total
                        || (total_steps == *best_total
                            && cand.steps_to_pos > best_cand.steps_to_pos)
                }
            };
            if better {
                best = Some((total_steps, cand.clone(), dest, path));
            }
        }
    }

    let (_, cand, dest, path) = best?;

    if let (Some((start_idx, end_idx)), Some(path_state)) = (cand.prefix_range, path_state) {
        let mut combined = Vec::new();
        combined.extend_from_slice(&path_state.path[start_idx..=end_idx]);
        if path.len() > 1 {
            combined.extend_from_slice(&path[1..]);
        }
        return Some((dest, combined));
    }

    Some((dest, path))
}

pub(super) fn face_target_if_needed(
    state: &mut GameState,
    commands: &mut Vec<InputCommand>,
    dx: f32,
    dy: f32,
) {
    let dir = crate::game::Direction::from_velocity(dx, dy);
    if let Some(local_id) = &state.local_player_id {
        if let Some(player) = state.players.get(local_id) {
            if player.direction == dir {
                return;
            }
        }
    }
    queue_face(state, commands, dir as u8);
}

/// Pathfind to within attack range of a player and set up attack, or attack immediately if in range.
pub(super) fn pathfind_and_attack_player(
    state: &mut GameState,
    commands: &mut Vec<InputCommand>,
    target_id: &str,
) {
    if let Some(local_id) = &state.local_player_id.clone() {
        if let Some(local_player) = state.players.get(local_id) {
            if let Some(target) = state.players.get(target_id) {
                let px = local_player.server_x.round() as i32;
                let py = local_player.server_y.round() as i32;
                let tx = target.server_x.round() as i32;
                let ty = target.server_y.round() as i32;
                let weapon_range = get_local_weapon_range(state);
                if !in_attack_range(px, py, tx, ty, weapon_range) {
                    let occupied = build_occupied_set(state, true, true);
                    const MAX_PATH_DISTANCE: i32 = 32;
                    if let Some((dest, path)) = find_path_to_attack_with_optimistic_splice(
                        state,
                        (px, py),
                        (tx, ty),
                        &occupied,
                        MAX_PATH_DISTANCE,
                        weapon_range,
                    ) {
                        state.auto_path = Some(PathState {
                            path,
                            current_index: 0,
                            destination: dest,
                            pickup_target: None,
                            interact_target: None,
                            interact_object_target: None,
                            waystone_target: None,
                            browse_stall_target: None,
                        });
                    }
                } else {
                    let dir = crate::game::Direction::from_velocity(
                        target.server_x - local_player.x,
                        target.server_y - local_player.y,
                    );
                    queue_face(state, commands, dir as u8);
                    commands.push(InputCommand::StartAutoAction {
                        target_type: "player".to_string(),
                        target_id: target_id.to_string(),
                        action: "attack".to_string(),
                    });
                }
            }
        }
    }
}

/// Pathfind to within attack range of an NPC and set up attack, or attack immediately if in range.
pub(super) fn pathfind_and_attack_npc(
    state: &mut GameState,
    commands: &mut Vec<InputCommand>,
    npc_id: &str,
) {
    if let Some(local_id) = &state.local_player_id.clone() {
        if let Some(player) = state.players.get(local_id) {
            if let Some(npc) = state.npcs.get(npc_id) {
                let px = player.server_x.round() as i32;
                let py = player.server_y.round() as i32;
                let nx = npc.server_x.round() as i32;
                let ny = npc.server_y.round() as i32;
                // For multi-tile NPCs, use the nearest footprint tile for range/pathfinding
                let closest_x = px.clamp(nx, nx + npc.size - 1);
                let closest_y = py.clamp(ny, ny + npc.size - 1);
                let weapon_range = get_local_weapon_range(state);
                if !in_attack_range(px, py, closest_x, closest_y, weapon_range) {
                    let mut occupied = build_occupied_set(state, true, true);
                    // Remove NPC footprint tiles so pathfinding can route to them
                    for dy in 0..npc.size {
                        for dx in 0..npc.size {
                            occupied.remove(&(nx + dx, ny + dy));
                        }
                    }
                    const MAX_PATH_DISTANCE: i32 = 32;
                    if let Some((dest, path)) = find_path_to_attack_with_optimistic_splice(
                        state,
                        (px, py),
                        (closest_x, closest_y),
                        &occupied,
                        MAX_PATH_DISTANCE,
                        weapon_range,
                    ) {
                        state.auto_path = Some(PathState {
                            path,
                            current_index: 0,
                            destination: dest,
                            pickup_target: None,
                            interact_target: None,
                            interact_object_target: None,
                            waystone_target: None,
                            browse_stall_target: None,
                        });
                    }
                } else {
                    let dir = crate::game::Direction::from_velocity(
                        closest_x as f32 - player.x,
                        closest_y as f32 - player.y,
                    );
                    queue_face(state, commands, dir as u8);
                    if let Some(aa) = state.auto_action_state.as_ref() {
                        if auto_action_target_settled(aa, state) {
                            commands.push(InputCommand::StartAutoAction {
                                target_type: "npc".to_string(),
                                target_id: npc_id.to_string(),
                                action: "attack".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Pathfind to NPC and execute an action when in range, or do it immediately if close enough.
pub(super) fn pathfind_and_interact_npc(
    state: &mut GameState,
    commands: &mut Vec<InputCommand>,
    npc_id: &str,
    on_interact: impl FnOnce(&mut GameState, &mut Vec<InputCommand>, &str),
) {
    const INTERACT_RANGE: f32 = 2.5;
    let should_interact = if let Some(local_id) = &state.local_player_id.clone() {
        if let Some(player) = state.players.get(local_id) {
            if let Some(npc) = state.npcs.get(npc_id) {
                let dx = npc.server_x - player.server_x;
                let dy = npc.server_y - player.server_y;
                let dist = (dx * dx + dy * dy).sqrt();
                Some(dist < INTERACT_RANGE)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    match should_interact {
        Some(true) => {
            on_interact(state, commands, npc_id);
        }
        Some(false) => {
            if let Some(local_id) = &state.local_player_id.clone() {
                if let Some(player) = state.players.get(local_id) {
                    if let Some(npc) = state.npcs.get(npc_id) {
                        let px = player.server_x.round() as i32;
                        let py = player.server_y.round() as i32;
                        let nx = npc.server_x.round() as i32;
                        let ny = npc.server_y.round() as i32;
                        let occupied = build_occupied_set(state, false, true);
                        const MAX_PATH_DISTANCE: i32 = 32;
                        if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                            (px, py),
                            (nx, ny),
                            &state.chunk_manager,
                            &occupied,
                            MAX_PATH_DISTANCE,
                        ) {
                            let npc_id_owned = npc_id.to_string();
                            state.auto_path = Some(PathState {
                                path,
                                current_index: 0,
                                destination: dest,
                                pickup_target: None,
                                interact_target: Some(npc_id_owned),
                                interact_object_target: None,
                                waystone_target: None,
                                browse_stall_target: None,
                            });
                        }
                    }
                }
            }
        }
        None => {}
    }
}

/// Pathfind to adjacent tile of a resource and start auto-action, or do it immediately if adjacent.
pub(super) fn pathfind_and_resource(
    state: &mut GameState,
    commands: &mut Vec<InputCommand>,
    tile_x: i32,
    tile_y: i32,
    target_id: &str,
    action: &str,
) {
    if let Some(player) = state.get_local_player() {
        let px = player.server_x.round() as i32;
        let py = player.server_y.round() as i32;
        let cdx = (px - tile_x).abs();
        let cdy = (py - tile_y).abs();
        if (cdx + cdy) != 1 {
            let occupied = build_occupied_set(state, true, true);
            const MAX_PATH_DISTANCE: i32 = 32;
            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                (px, py),
                (tile_x, tile_y),
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
            ) {
                state.auto_path = Some(PathState {
                    path,
                    current_index: 0,
                    destination: dest,
                    pickup_target: None,
                    interact_target: None,
                    interact_object_target: None,
                    waystone_target: None,
                    browse_stall_target: None,
                });
            }
        } else {
            let dir = crate::game::Direction::from_velocity(
                tile_x as f32 - px as f32,
                tile_y as f32 - py as f32,
            );
            queue_face(state, commands, dir as u8);
            commands.push(InputCommand::StartAutoAction {
                target_type: "resource".to_string(),
                target_id: target_id.to_string(),
                action: action.to_string(),
            });
        }
    }
}

/// Pathfind to a tile.
pub(super) fn pathfind_to_tile(
    state: &mut GameState,
    commands: &mut Vec<InputCommand>,
    tile_x: i32,
    tile_y: i32,
) {
    // Cancel any existing auto-action or follow
    if state.auto_action_state.is_some() {
        state.auto_action_state = None;
        commands.push(InputCommand::CancelAutoAction);
    }
    state.follow_target = None;
    state.follow_arrived_target_pos = None;
    state.follow_target_move_time = 0.0;

    const MAX_PATH_DISTANCE: i32 = 32;
    if let Some(player) = state.get_local_player() {
        // Start path plans from authoritative server tile to avoid
        // planning from a visual/interpolated future tile.
        let px = player.server_x.round() as i32;
        let py = player.server_y.round() as i32;
        let dist = (tile_x - px).abs().max((tile_y - py).abs());
        if dist <= MAX_PATH_DISTANCE
            && state
                .chunk_manager
                .is_walkable(tile_x as f32, tile_y as f32)
        {
            let occupied = build_occupied_set(state, true, true);
            if let Some(path) = pathfinding::find_path(
                (px, py),
                (tile_x, tile_y),
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
            ) {
                state.auto_path = Some(PathState {
                    path,
                    current_index: 0,
                    destination: (tile_x, tile_y),
                    pickup_target: None,
                    interact_target: None,
                    interact_object_target: None,
                    waystone_target: None,
                    browse_stall_target: None,
                });
            }
        }
    }
}

pub(super) fn rebuild_path_state(
    template: &PathState,
    path: Vec<(i32, i32)>,
    destination: (i32, i32),
) -> PathState {
    let mut next = template.clone();
    next.path = path;
    next.current_index = 0;
    next.destination = destination;
    next
}

pub(super) fn rebuild_current_auto_path(state: &mut GameState) -> bool {
    const MAX_PATH_DISTANCE: i32 = 32;

    let Some(template) = state.auto_path.clone() else {
        return false;
    };
    let Some(player) = state.get_local_player() else {
        return false;
    };
    let start = (
        player.server_x.round() as i32,
        player.server_y.round() as i32,
    );

    if let Some(aa) = state.auto_action_state.clone() {
        if let Some((txf, tyf)) = auto_action_target_pos(&aa, state) {
            let target = (txf.round() as i32, tyf.round() as i32);
            let mut occupied = build_occupied_set(state, true, true);
            match aa.target_type.as_str() {
                "npc" => {
                    // Remove all footprint tiles for multi-tile NPCs
                    if let Some(npc) = state.npcs.get(&aa.target_id) {
                        let nx = npc.server_x.round() as i32;
                        let ny = npc.server_y.round() as i32;
                        for dy in 0..npc.size {
                            for dx in 0..npc.size {
                                occupied.remove(&(nx + dx, ny + dy));
                            }
                        }
                    } else {
                        occupied.remove(&target);
                    }
                }
                "player" => {
                    occupied.remove(&target);
                }
                _ => {}
            }
            let preferred = preferred_adjacent_tile_for_target(state, target);
            if let Some((dest, path)) = pathfinding::find_path_to_adjacent_prefer(
                start,
                target,
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
                preferred,
            ) {
                state.auto_path = Some(rebuild_path_state(&template, path, dest));
                return true;
            }
        }
        return false;
    }

    if let Some(follow_id) = state.follow_target.clone() {
        if let Some(target) = state.players.get(&follow_id) {
            let target_tile = (
                target.server_x.round() as i32,
                target.server_y.round() as i32,
            );
            let mut occupied = build_occupied_set(state, true, true);
            occupied.remove(&target_tile);
            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                start,
                target_tile,
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
            ) {
                state.auto_path = Some(rebuild_path_state(&template, path, dest));
                return true;
            }
        }
        return false;
    }

    if let Some(item_id) = template.pickup_target.clone() {
        if let Some(item) = state.ground_items.get(&item_id) {
            let target = item.tile_coords();
            let occupied = build_occupied_set(state, true, true);
            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                start,
                target,
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
            ) {
                state.auto_path = Some(rebuild_path_state(&template, path, dest));
                return true;
            }
        }
        return false;
    }

    if let Some(npc_id) = template.interact_target.clone() {
        if let Some(npc) = state.npcs.get(&npc_id) {
            let target = (npc.server_x.round() as i32, npc.server_y.round() as i32);
            let occupied = build_occupied_set(state, false, true);
            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                start,
                target,
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
            ) {
                state.auto_path = Some(rebuild_path_state(&template, path, dest));
                return true;
            }
        }
        return false;
    }

    if let Some(target) = template.interact_object_target {
        let occupied = build_occupied_set(state, true, true);
        if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
            start,
            target,
            &state.chunk_manager,
            &occupied,
            MAX_PATH_DISTANCE,
        ) {
            state.auto_path = Some(rebuild_path_state(&template, path, dest));
            return true;
        }
        return false;
    }

    if let Some(target) = template.waystone_target {
        let occupied = build_occupied_set(state, true, true);
        if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
            start,
            target,
            &state.chunk_manager,
            &occupied,
            MAX_PATH_DISTANCE,
        ) {
            state.auto_path = Some(rebuild_path_state(&template, path, dest));
            return true;
        }
        return false;
    }

    if let Some(player_id) = template.browse_stall_target.clone() {
        if let Some(target) = state.players.get(&player_id) {
            let target_tile = (
                target.server_x.round() as i32,
                target.server_y.round() as i32,
            );
            let mut occupied = build_occupied_set(state, true, true);
            occupied.remove(&target_tile);
            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                start,
                target_tile,
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
            ) {
                state.auto_path = Some(rebuild_path_state(&template, path, dest));
                return true;
            }
        }
        return false;
    }

    if let Some((chair_x, chair_y)) = state.pending_chair_sit {
        let occupied = build_occupied_set(state, true, true);
        if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
            start,
            (chair_x, chair_y),
            &state.chunk_manager,
            &occupied,
            MAX_PATH_DISTANCE,
        ) {
            state.auto_path = Some(rebuild_path_state(&template, path, dest));
            return true;
        }
        return false;
    }

    if let Some(patch_id) = state.pending_harvest_patch.clone() {
        if let Some(patch) = state.farming_patches.get(&patch_id) {
            let occupied = build_occupied_set(state, true, true);
            if let Some((dest, path)) = pathfinding::find_path_to_adjacent(
                start,
                (patch.x, patch.y),
                &state.chunk_manager,
                &occupied,
                MAX_PATH_DISTANCE,
            ) {
                state.auto_path = Some(rebuild_path_state(&template, path, dest));
                return true;
            }
        }
        return false;
    }

    let goal = template.destination;
    if !state
        .chunk_manager
        .is_walkable(goal.0 as f32, goal.1 as f32)
    {
        return false;
    }
    let occupied = build_occupied_set(state, true, true);
    if let Some(path) = pathfinding::find_path(
        start,
        goal,
        &state.chunk_manager,
        &occupied,
        MAX_PATH_DISTANCE,
    ) {
        state.auto_path = Some(rebuild_path_state(&template, path, goal));
        return true;
    }

    false
}

/// Returns (combat_level_required, slayer_level_required) for a slayer master entity type.
pub(super) fn slayer_master_requirements(entity_type: &str) -> (i32, i32) {
    match entity_type {
        "slayer_master_turael" => (0, 1),
        "slayer_master_mazchna" => (20, 30),
        "slayer_master_chaeldar" => (40, 60),
        _ => (0, 1), // Unknown master, let server validate
    }
}
