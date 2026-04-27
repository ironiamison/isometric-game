use super::chunk::ChunkManager;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// State for tracking automated pathfinding movement
#[derive(Debug, Clone)]
pub struct PathState {
    pub path: Vec<(i32, i32)>,           // Waypoints (grid coordinates)
    pub current_index: usize,            // Current waypoint being targeted
    pub destination: (i32, i32),         // Final target
    pub pickup_target: Option<String>,   // Item ID to pick up when path completes
    pub interact_target: Option<String>, // NPC ID to interact with on path completion
    pub interact_object_target: Option<(i32, i32)>, // Map object (x,y) to interact with on path completion
    pub waystone_target: Option<(i32, i32)>, // Waystone (x,y) to teleport directly on path completion
    pub browse_stall_target: Option<String>, // Player ID to browse stall on path completion
}

/// Node for A* priority queue
#[derive(Clone, Eq, PartialEq)]
struct Node {
    pos: (i32, i32),
    g_cost: i32, // Cost from start
    f_cost: i32, // g + heuristic
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap: lower f_cost = higher priority
        other
            .f_cost
            .cmp(&self.f_cost)
            .then_with(|| other.g_cost.cmp(&self.g_cost))
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Manhattan distance heuristic for grid movement
fn heuristic(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

/// Check if movement between two tiles is allowed by height rules.
/// Matches server logic: can step up at most 1 block, can step down any amount.
fn can_traverse_height(chunk_manager: &ChunkManager, from: (i32, i32), to: (i32, i32)) -> bool {
    let from_h = chunk_manager.get_height(from.0, from.1) as i32;
    let to_h = chunk_manager.get_height(to.0, to.1) as i32;
    let height_diff = to_h - from_h;
    // Can step up at most 1 block; can step down any amount
    height_diff <= 1
}

/// Find a path from start to goal using A* algorithm.
/// Returns None if no path exists or goal is too far.
///
/// # Arguments
/// * `start` - Starting grid position
/// * `goal` - Target grid position
/// * `chunk_manager` - For walkability and height checks
/// * `occupied` - Set of tiles occupied by entities (players/NPCs)
/// * `max_distance` - Maximum allowed distance (Chebyshev)
pub fn find_path(
    start: (i32, i32),
    goal: (i32, i32),
    chunk_manager: &ChunkManager,
    occupied: &HashSet<(i32, i32)>,
    max_distance: i32,
) -> Option<Vec<(i32, i32)>> {
    // Early exit: already at goal
    if start == goal {
        return Some(vec![goal]);
    }

    // Early exit: goal too far (Chebyshev distance)
    let chebyshev_dist = (goal.0 - start.0).abs().max((goal.1 - start.1).abs());
    if chebyshev_dist > max_distance {
        return None;
    }

    // Early exit: goal not walkable
    if !chunk_manager.is_walkable(goal.0 as f32, goal.1 as f32) {
        return None;
    }

    // A* algorithm
    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();

    g_score.insert(start, 0);
    open_set.push(Node {
        pos: start,
        g_cost: 0,
        f_cost: heuristic(start, goal),
    });

    // 4-directional movement (matching server grid movement)
    let neighbors = [(0, -1), (0, 1), (-1, 0), (1, 0)];

    // Limit iterations to prevent lag
    const MAX_ITERATIONS: i32 = 1000;
    let mut iterations = 0;

    while let Some(current) = open_set.pop() {
        iterations += 1;
        if iterations > MAX_ITERATIONS {
            return None; // Too complex, give up
        }

        // Found the goal
        if current.pos == goal {
            // Reconstruct path
            let mut path = vec![goal];
            let mut current_pos = goal;
            while let Some(&prev) = came_from.get(&current_pos) {
                path.push(prev);
                current_pos = prev;
            }
            path.reverse();
            return Some(path);
        }

        let current_g = *g_score.get(&current.pos).unwrap_or(&i32::MAX);

        // Skip if we've found a better path to this node
        if current.g_cost > current_g {
            continue;
        }

        // Explore neighbors
        for (dx, dy) in neighbors {
            let neighbor = (current.pos.0 + dx, current.pos.1 + dy);

            // Check walkability
            if !chunk_manager.is_walkable(neighbor.0 as f32, neighbor.1 as f32) {
                continue;
            }

            // Check height traversability (can't climb more than 1 block)
            if !can_traverse_height(chunk_manager, current.pos, neighbor) {
                continue;
            }

            // Check if tile is occupied by an entity (allow goal tile for path completion)
            if neighbor != goal && occupied.contains(&neighbor) {
                continue;
            }

            let tentative_g = current_g + 1;

            if tentative_g < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                // This path is better
                came_from.insert(neighbor, current.pos);
                g_score.insert(neighbor, tentative_g);

                open_set.push(Node {
                    pos: neighbor,
                    g_cost: tentative_g,
                    f_cost: tentative_g + heuristic(neighbor, goal),
                });
            }
        }
    }

    // No path found
    None
}

/// Find the best adjacent tile (N/S/E/W only) to a target position.
/// Returns the adjacent tile and the path to it, or None if no path exists.
pub fn find_path_to_adjacent(
    start: (i32, i32),
    target: (i32, i32),
    chunk_manager: &ChunkManager,
    occupied: &HashSet<(i32, i32)>,
    max_distance: i32,
) -> Option<((i32, i32), Vec<(i32, i32)>)> {
    // Check all 4 cardinal directions
    let adjacent_tiles = [
        (target.0, target.1 - 1), // North
        (target.0, target.1 + 1), // South
        (target.0 - 1, target.1), // West
        (target.0 + 1, target.1), // East
    ];

    let mut best_path: Option<((i32, i32), Vec<(i32, i32)>)> = None;
    let mut best_length = i32::MAX;

    for adj in adjacent_tiles {
        // Skip if not walkable
        if !chunk_manager.is_walkable(adj.0 as f32, adj.1 as f32) {
            continue;
        }

        // Skip if occupied by another entity
        if occupied.contains(&adj) {
            continue;
        }

        // Try to find a path to this adjacent tile
        if let Some(path) = find_path(start, adj, chunk_manager, occupied, max_distance) {
            let path_len = path.len() as i32;
            if path_len < best_length {
                best_length = path_len;
                best_path = Some((adj, path));
            }
        }
    }

    best_path
}

/// Find the shortest path to any walkable tile within Chebyshev distance `range` of the target.
/// Used for ranged weapon attacks where the player doesn't need to be adjacent.
/// Returns the destination tile and the path to it, or None if no path exists.
pub fn find_path_within_range(
    start: (i32, i32),
    target: (i32, i32),
    chunk_manager: &ChunkManager,
    occupied: &HashSet<(i32, i32)>,
    max_distance: i32,
    range: i32,
) -> Option<((i32, i32), Vec<(i32, i32)>)> {
    // Already in range?
    let dx = (start.0 - target.0).abs();
    let dy = (start.1 - target.1).abs();
    if (dx + dy) <= range && (dx > 0 || dy > 0) {
        return Some((start, vec![start]));
    }

    // A* with modified goal: any tile within Manhattan distance `range` of target
    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();

    // Heuristic: distance to edge of the "in range" zone (Manhattan)
    let range_heuristic = |pos: (i32, i32)| -> i32 {
        let cdx = (pos.0 - target.0).abs();
        let cdy = (pos.1 - target.1).abs();
        // How many steps to get within Manhattan range
        ((cdx + cdy) - range).max(0)
    };

    g_score.insert(start, 0);
    open_set.push(Node {
        pos: start,
        g_cost: 0,
        f_cost: range_heuristic(start),
    });

    let neighbors = [(0, -1), (0, 1), (-1, 0), (1, 0)];
    const MAX_ITERATIONS: i32 = 1000;
    let mut iterations = 0;

    while let Some(current) = open_set.pop() {
        iterations += 1;
        if iterations > MAX_ITERATIONS {
            return None;
        }

        // Check if we've reached a tile within Manhattan range
        let cdx = (current.pos.0 - target.0).abs();
        let cdy = (current.pos.1 - target.1).abs();
        if (cdx + cdy) <= range && (cdx > 0 || cdy > 0) {
            // Reconstruct path
            let mut path = vec![current.pos];
            let mut pos = current.pos;
            while let Some(&prev) = came_from.get(&pos) {
                path.push(prev);
                pos = prev;
            }
            path.reverse();
            return Some((current.pos, path));
        }

        let current_g = *g_score.get(&current.pos).unwrap_or(&i32::MAX);
        if current.g_cost > current_g {
            continue;
        }

        for (ndx, ndy) in neighbors {
            let neighbor = (current.pos.0 + ndx, current.pos.1 + ndy);

            if !chunk_manager.is_walkable(neighbor.0 as f32, neighbor.1 as f32) {
                continue;
            }

            // Check height traversability (can't climb more than 1 block)
            if !can_traverse_height(chunk_manager, current.pos, neighbor) {
                continue;
            }

            // Don't walk onto the target tile itself
            if neighbor == target {
                continue;
            }

            if occupied.contains(&neighbor) {
                continue;
            }

            // Check max distance from start
            let dist_from_start = (neighbor.0 - start.0)
                .abs()
                .max((neighbor.1 - start.1).abs());
            if dist_from_start > max_distance {
                continue;
            }

            let tentative_g = current_g + 1;
            if tentative_g < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                came_from.insert(neighbor, current.pos);
                g_score.insert(neighbor, tentative_g);
                open_set.push(Node {
                    pos: neighbor,
                    g_cost: tentative_g,
                    f_cost: tentative_g + range_heuristic(neighbor),
                });
            }
        }
    }

    None
}

/// Find the best adjacent tile with a preferred approach side.
/// Preference is used only as a tie-breaker among equally short paths.
pub fn find_path_to_adjacent_prefer(
    start: (i32, i32),
    target: (i32, i32),
    chunk_manager: &ChunkManager,
    occupied: &HashSet<(i32, i32)>,
    max_distance: i32,
    preferred_adjacent: Option<(i32, i32)>,
) -> Option<((i32, i32), Vec<(i32, i32)>)> {
    // Check all 4 cardinal directions
    let adjacent_tiles = [
        (target.0, target.1 - 1), // North
        (target.0, target.1 + 1), // South
        (target.0 - 1, target.1), // West
        (target.0 + 1, target.1), // East
    ];

    let mut best_path: Option<((i32, i32), Vec<(i32, i32)>)> = None;
    let mut best_length = i32::MAX;
    let mut best_preferred = false;
    let mut best_secondary = i32::MAX;

    for adj in adjacent_tiles {
        // Skip if not walkable
        if !chunk_manager.is_walkable(adj.0 as f32, adj.1 as f32) {
            continue;
        }

        // Skip if occupied by another entity
        if occupied.contains(&adj) {
            continue;
        }

        if let Some(path) = find_path(start, adj, chunk_manager, occupied, max_distance) {
            let path_len = path.len() as i32;
            let is_preferred = preferred_adjacent.map_or(false, |p| p == adj);
            let secondary = heuristic(start, adj);

            let better = path_len < best_length
                || (path_len == best_length && is_preferred && !best_preferred)
                || (path_len == best_length
                    && is_preferred == best_preferred
                    && secondary < best_secondary);

            if better {
                best_length = path_len;
                best_preferred = is_preferred;
                best_secondary = secondary;
                best_path = Some((adj, path));
            }
        }
    }

    best_path
}
