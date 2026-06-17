use super::*;

pub(super) fn encode(msg: &ServerMessage) -> Option<Value> {
    let value = match msg {
        ServerMessage::ArenaStateUpdate {
            state,
            countdown_remaining,
            queued_count,
            fighter_count,
            entry_fee,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("state".into()),
                Value::String(state.clone().into()),
            ));
            map.push((
                Value::String("countdownRemaining".into()),
                match countdown_remaining {
                    Some(r) => Value::Integer((*r as i64).into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("queuedCount".into()),
                Value::Integer((*queued_count as i64).into()),
            ));
            map.push((
                Value::String("fighterCount".into()),
                Value::Integer((*fighter_count as i64).into()),
            ));
            map.push((
                Value::String("entryFee".into()),
                Value::Integer((*entry_fee as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ArenaMatchStart { fighter_ids } => {
            let mut map = Vec::new();
            let ids: Vec<Value> = fighter_ids
                .iter()
                .map(|id| Value::String(id.clone().into()))
                .collect();
            map.push((Value::String("fighterIds".into()), Value::Array(ids)));
            Value::Map(map)
        }
        ServerMessage::ArenaPlayerEliminated {
            player_id,
            player_name,
            killer_id,
            killer_name,
            remaining,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("playerId".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("playerName".into()),
                Value::String(player_name.clone().into()),
            ));
            map.push((
                Value::String("killerId".into()),
                Value::String(killer_id.clone().into()),
            ));
            map.push((
                Value::String("killerName".into()),
                Value::String(killer_name.clone().into()),
            ));
            map.push((
                Value::String("remaining".into()),
                Value::Integer((*remaining as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ArenaMatchEnd { placements } => {
            let mut map = Vec::new();
            let placement_values: Vec<Value> = placements
                .iter()
                .map(|p| {
                    let mut pmap = Vec::new();
                    pmap.push((
                        Value::String("rank".into()),
                        Value::Integer((p.rank as i64).into()),
                    ));
                    pmap.push((
                        Value::String("playerId".into()),
                        Value::String(p.player_id.clone().into()),
                    ));
                    pmap.push((
                        Value::String("playerName".into()),
                        Value::String(p.player_name.clone().into()),
                    ));
                    pmap.push((
                        Value::String("kills".into()),
                        Value::Integer((p.kills as i64).into()),
                    ));
                    pmap.push((
                        Value::String("goldReward".into()),
                        Value::Integer((p.gold_reward as i64).into()),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((
                Value::String("placements".into()),
                Value::Array(placement_values),
            ));
            Value::Map(map)
        }
        ServerMessage::ArenaStatsUpdate {
            wins,
            kills,
            deaths,
            current_streak,
            best_streak,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("wins".into()),
                Value::Integer((*wins as i64).into()),
            ));
            map.push((
                Value::String("kills".into()),
                Value::Integer((*kills as i64).into()),
            ));
            map.push((
                Value::String("deaths".into()),
                Value::Integer((*deaths as i64).into()),
            ));
            map.push((
                Value::String("currentStreak".into()),
                Value::Integer((*current_streak as i64).into()),
            ));
            map.push((
                Value::String("bestStreak".into()),
                Value::Integer((*best_streak as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::KothStateUpdate {
            phase,
            wave,
            points,
            enemies_alive,
            enemies_total,
            countdown_ms,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("phase".into()),
                Value::String(phase.clone().into()),
            ));
            map.push((
                Value::String("wave".into()),
                Value::Integer((*wave as i64).into()),
            ));
            map.push((
                Value::String("points".into()),
                Value::Integer((*points as i64).into()),
            ));
            map.push((
                Value::String("enemiesAlive".into()),
                Value::Integer((*enemies_alive as i64).into()),
            ));
            map.push((
                Value::String("enemiesTotal".into()),
                Value::Integer((*enemies_total as i64).into()),
            ));
            map.push((
                Value::String("countdownMs".into()),
                Value::Integer((*countdown_ms as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::KothCheckpoint {
            wave,
            points,
            rewards,
            next_wave_enemy_count,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("wave".into()),
                Value::Integer((*wave as i64).into()),
            ));
            map.push((
                Value::String("points".into()),
                Value::Integer((*points as i64).into()),
            ));
            let reward_values: Vec<Value> = rewards
                .iter()
                .map(|r| {
                    let mut rmap = Vec::new();
                    rmap.push((
                        Value::String("itemId".into()),
                        Value::String(r.item_id.clone().into()),
                    ));
                    rmap.push((
                        Value::String("quantity".into()),
                        Value::Integer((r.quantity as i64).into()),
                    ));
                    Value::Map(rmap)
                })
                .collect();
            map.push((Value::String("rewards".into()), Value::Array(reward_values)));
            map.push((
                Value::String("nextWaveEnemyCount".into()),
                Value::Integer((*next_wave_enemy_count as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::KothGameOver {
            waves_completed,
            total_points,
            rewards,
            victory,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("wavesCompleted".into()),
                Value::Integer((*waves_completed as i64).into()),
            ));
            map.push((
                Value::String("totalPoints".into()),
                Value::Integer((*total_points as i64).into()),
            ));
            let reward_values: Vec<Value> = rewards
                .iter()
                .map(|r| {
                    let mut rmap = Vec::new();
                    rmap.push((
                        Value::String("itemId".into()),
                        Value::String(r.item_id.clone().into()),
                    ));
                    rmap.push((
                        Value::String("quantity".into()),
                        Value::Integer((r.quantity as i64).into()),
                    ));
                    Value::Map(rmap)
                })
                .collect();
            map.push((Value::String("rewards".into()), Value::Array(reward_values)));
            map.push((Value::String("victory".into()), Value::Boolean(*victory)));
            Value::Map(map)
        }
        ServerMessage::BossStateUpdate {
            boss_id,
            hp,
            max_hp,
            phase,
            wurm_state,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("bossId".into()),
                Value::String(boss_id.clone().into()),
            ));
            map.push((
                Value::String("hp".into()),
                Value::Integer((*hp as i64).into()),
            ));
            map.push((
                Value::String("maxHp".into()),
                Value::Integer((*max_hp as i64).into()),
            ));
            map.push((
                Value::String("phase".into()),
                Value::String(phase.clone().into()),
            ));
            map.push((
                Value::String("wurmState".into()),
                Value::String(wurm_state.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::AoeWarning {
            tiles,
            delay_ms,
            effect,
        } => {
            let mut map = Vec::new();
            let tile_values: Vec<Value> = tiles
                .iter()
                .map(|(x, y)| {
                    Value::Array(vec![
                        Value::Integer((*x as i64).into()),
                        Value::Integer((*y as i64).into()),
                    ])
                })
                .collect();
            map.push((Value::String("tiles".into()), Value::Array(tile_values)));
            map.push((
                Value::String("delayMs".into()),
                Value::Integer((*delay_ms as i64).into()),
            ));
            map.push((
                Value::String("effect".into()),
                Value::String(effect.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::AoeDamage {
            tiles,
            damage,
            effect,
        } => {
            let mut map = Vec::new();
            let tile_values: Vec<Value> = tiles
                .iter()
                .map(|(x, y)| {
                    Value::Array(vec![
                        Value::Integer((*x as i64).into()),
                        Value::Integer((*y as i64).into()),
                    ])
                })
                .collect();
            map.push((Value::String("tiles".into()), Value::Array(tile_values)));
            map.push((
                Value::String("damage".into()),
                Value::Integer((*damage as i64).into()),
            ));
            map.push((
                Value::String("effect".into()),
                Value::String(effect.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::Explosion {
            x,
            y,
            radius,
            damage,
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
                Value::String("radius".into()),
                Value::Integer((*radius as i64).into()),
            ));
            map.push((
                Value::String("damage".into()),
                Value::Integer((*damage as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ReaperMark {
            player_id,
            duration_ms,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("playerId".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("durationMs".into()),
                Value::Integer((*duration_ms as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::FriendRequestReceived { from_id, from_name } => {
            let mut map = Vec::new();
            map.push((
                Value::String("from_id".into()),
                Value::Integer((*from_id).into()),
            ));
            map.push((
                Value::String("from_name".into()),
                Value::String(from_name.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::FriendRequestAccepted {
            friend_id,
            friend_name,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("friend_id".into()),
                Value::Integer((*friend_id).into()),
            ));
            map.push((
                Value::String("friend_name".into()),
                Value::String(friend_name.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::FriendRequestDeclined { by_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("by_id".into()),
                Value::Integer((*by_id).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::FriendRemoved { friend_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("friend_id".into()),
                Value::Integer((*friend_id).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::FriendsList { friends } => {
            let mut map = Vec::new();
            let friend_values: Vec<Value> = friends
                .iter()
                .map(|f| {
                    let mut fmap = Vec::new();
                    fmap.push((Value::String("id".into()), Value::Integer(f.id.into())));
                    fmap.push((
                        Value::String("name".into()),
                        Value::String(f.name.clone().into()),
                    ));
                    fmap.push((Value::String("online".into()), Value::Boolean(f.online)));
                    Value::Map(fmap)
                })
                .collect();
            map.push((Value::String("friends".into()), Value::Array(friend_values)));
            Value::Map(map)
        }
        ServerMessage::PendingFriendRequests { requests } => {
            let mut map = Vec::new();
            let request_values: Vec<Value> = requests
                .iter()
                .map(|r| {
                    let mut rmap = Vec::new();
                    rmap.push((
                        Value::String("from_id".into()),
                        Value::Integer(r.from_id.into()),
                    ));
                    rmap.push((
                        Value::String("from_name".into()),
                        Value::String(r.from_name.clone().into()),
                    ));
                    Value::Map(rmap)
                })
                .collect();
            map.push((
                Value::String("requests".into()),
                Value::Array(request_values),
            ));
            Value::Map(map)
        }
        ServerMessage::OnlinePlayersList { players } => {
            let mut map = Vec::new();
            let player_values: Vec<Value> = players
                .iter()
                .map(|p| {
                    let mut pmap = Vec::new();
                    pmap.push((Value::String("id".into()), Value::Integer(p.id.into())));
                    pmap.push((
                        Value::String("name".into()),
                        Value::String(p.name.clone().into()),
                    ));
                    pmap.push((
                        Value::String("is_friend".into()),
                        Value::Boolean(p.is_friend),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((Value::String("players".into()), Value::Array(player_values)));
            Value::Map(map)
        }
        ServerMessage::FriendStatusChanged { friend_id, online } => {
            let mut map = Vec::new();
            map.push((
                Value::String("friend_id".into()),
                Value::Integer((*friend_id).into()),
            ));
            map.push((Value::String("online".into()), Value::Boolean(*online)));
            Value::Map(map)
        }
        ServerMessage::FriendActionResult {
            action,
            success,
            error,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("action".into()),
                Value::String(action.clone().into()),
            ));
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            if let Some(err) = error {
                map.push((
                    Value::String("error".into()),
                    Value::String(err.clone().into()),
                ));
            }
            Value::Map(map)
        }
        // Crafting system messages
        _ => return None,
    };
    Some(value)
}
