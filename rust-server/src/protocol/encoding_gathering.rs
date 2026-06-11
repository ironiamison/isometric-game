use super::*;

pub(super) fn encode(msg: &ServerMessage) -> Option<Value> {
    let value = match msg {
        ServerMessage::GatheringStarted {
            player_id,
            marker_x,
            marker_y,
            zone_id,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("marker_x".into()),
                Value::Integer((*marker_x as i64).into()),
            ));
            map.push((
                Value::String("marker_y".into()),
                Value::Integer((*marker_y as i64).into()),
            ));
            map.push((
                Value::String("zone_id".into()),
                Value::String(zone_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::GatheringResult {
            player_id,
            item_id,
            xp_gained,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("xp_gained".into()),
                Value::Integer((*xp_gained).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::GatheringStopped { player_id, reason } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("reason".into()),
                Value::String(reason.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::WoodcuttingStarted {
            player_id,
            tree_x,
            tree_y,
            tree_type,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("tree_x".into()),
                Value::Integer((*tree_x as i64).into()),
            ));
            map.push((
                Value::String("tree_y".into()),
                Value::Integer((*tree_y as i64).into()),
            ));
            map.push((
                Value::String("tree_type".into()),
                Value::String(tree_type.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::WoodcuttingSwing {
            player_id,
            tree_x,
            tree_y,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("tree_x".into()),
                Value::Integer((*tree_x as i64).into()),
            ));
            map.push((
                Value::String("tree_y".into()),
                Value::Integer((*tree_y as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::WoodcuttingResult {
            player_id,
            item_id,
            xp_gained,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("xp_gained".into()),
                Value::Integer((*xp_gained).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::WoodcuttingStopped { player_id, reason } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("reason".into()),
                Value::String(reason.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::TreeDepleted {
            x,
            y,
            gid,
            respawn_delay_ms,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            map.push((
                Value::String("gid".into()),
                Value::Integer((*gid as i64).into()),
            ));
            map.push((
                Value::String("respawn_delay_ms".into()),
                Value::Integer((*respawn_delay_ms as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::TreeRespawned { x, y, gid } => {
            let mut map = Vec::new();
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            map.push((
                Value::String("gid".into()),
                Value::Integer((*gid as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::DepletedTreesSync { trees } => {
            let tree_values: Vec<Value> = trees
                .iter()
                .map(|t| {
                    let mut tree_map = Vec::new();
                    tree_map.push((
                        Value::String("x".into()),
                        Value::Integer((t.x as i64).into()),
                    ));
                    tree_map.push((
                        Value::String("y".into()),
                        Value::Integer((t.y as i64).into()),
                    ));
                    tree_map.push((
                        Value::String("gid".into()),
                        Value::Integer((t.gid as i64).into()),
                    ));
                    Value::Map(tree_map)
                })
                .collect();
            let mut map = Vec::new();
            map.push((Value::String("trees".into()), Value::Array(tree_values)));
            Value::Map(map)
        }
        // Mining system messages
        ServerMessage::MiningStarted {
            player_id,
            rock_x,
            rock_y,
            rock_type,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("rock_x".into()),
                Value::Integer((*rock_x as i64).into()),
            ));
            map.push((
                Value::String("rock_y".into()),
                Value::Integer((*rock_y as i64).into()),
            ));
            map.push((
                Value::String("rock_type".into()),
                Value::String(rock_type.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::MiningSwing {
            player_id,
            rock_x,
            rock_y,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("rock_x".into()),
                Value::Integer((*rock_x as i64).into()),
            ));
            map.push((
                Value::String("rock_y".into()),
                Value::Integer((*rock_y as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::MiningResult {
            player_id,
            item_id,
            xp_gained,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("xp_gained".into()),
                Value::Integer((*xp_gained).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::MiningStopped { player_id, reason } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("reason".into()),
                Value::String(reason.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::RockDepleted {
            x,
            y,
            gid,
            respawn_delay_ms,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            map.push((
                Value::String("gid".into()),
                Value::Integer((*gid as i64).into()),
            ));
            map.push((
                Value::String("respawn_delay_ms".into()),
                Value::Integer((*respawn_delay_ms as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::RockRespawned { x, y, gid } => {
            let mut map = Vec::new();
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            map.push((
                Value::String("gid".into()),
                Value::Integer((*gid as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::DepletedRocksSync { rocks } => {
            let rock_values: Vec<Value> = rocks
                .iter()
                .map(|r| {
                    let mut rock_map = Vec::new();
                    rock_map.push((
                        Value::String("x".into()),
                        Value::Integer((r.x as i64).into()),
                    ));
                    rock_map.push((
                        Value::String("y".into()),
                        Value::Integer((r.y as i64).into()),
                    ));
                    rock_map.push((
                        Value::String("gid".into()),
                        Value::Integer((r.gid as i64).into()),
                    ));
                    Value::Map(rock_map)
                })
                .collect();
            let mut map = Vec::new();
            map.push((Value::String("rocks".into()), Value::Array(rock_values)));
            Value::Map(map)
        }
        _ => return None,
    };
    Some(value)
}
