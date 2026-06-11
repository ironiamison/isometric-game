use super::*;

// ============================================================================
// Encoding/Decoding
// ============================================================================

/// Pre-encode a PlayerUpdate to rmpv::Value for reuse across per-player StateSync messages.
pub fn player_update_to_value(p: &PlayerUpdate) -> rmpv::Value {
    use rmpv::Value;
    let mut pmap = Vec::with_capacity(30);
    pmap.push((
        Value::String("id".into()),
        Value::String(p.id.clone().into()),
    ));
    pmap.push((
        Value::String("name".into()),
        Value::String(p.name.clone().into()),
    ));
    pmap.push((
        Value::String("x".into()),
        Value::Integer((p.x as i64).into()),
    ));
    pmap.push((
        Value::String("y".into()),
        Value::Integer((p.y as i64).into()),
    ));
    pmap.push((
        Value::String("z".into()),
        Value::Integer((p.z as i64).into()),
    ));
    pmap.push((
        Value::String("direction".into()),
        Value::Integer((p.direction as i64).into()),
    ));
    pmap.push((
        Value::String("velX".into()),
        Value::Integer((p.vel_x as i64).into()),
    ));
    pmap.push((
        Value::String("velY".into()),
        Value::Integer((p.vel_y as i64).into()),
    ));
    if let Some(seq) = p.move_ack_seq {
        pmap.push((
            Value::String("moveAckSeq".into()),
            Value::Integer((seq as i64).into()),
        ));
    }
    pmap.push((
        Value::String("hp".into()),
        Value::Integer((p.hp as i64).into()),
    ));
    pmap.push((
        Value::String("maxHp".into()),
        Value::Integer((p.max_hp as i64).into()),
    ));
    pmap.push((
        Value::String("combatLevel".into()),
        Value::Integer((p.combat_level as i64).into()),
    ));
    pmap.push((
        Value::String("hitpointsLevel".into()),
        Value::Integer((p.hitpoints_level as i64).into()),
    ));
    pmap.push((
        Value::String("attackLevel".into()),
        Value::Integer((p.attack_level as i64).into()),
    ));
    pmap.push((
        Value::String("strengthLevel".into()),
        Value::Integer((p.strength_level as i64).into()),
    ));
    pmap.push((
        Value::String("defenceLevel".into()),
        Value::Integer((p.defence_level as i64).into()),
    ));
    pmap.push((
        Value::String("rangedLevel".into()),
        Value::Integer((p.ranged_level as i64).into()),
    ));
    pmap.push((
        Value::String("gold".into()),
        Value::Integer((p.gold as i64).into()),
    ));
    pmap.push((
        Value::String("gender".into()),
        Value::String(p.gender.clone().into()),
    ));
    pmap.push((
        Value::String("skin".into()),
        Value::String(p.skin.clone().into()),
    ));
    pmap.push((
        Value::String("hair_style".into()),
        match p.hair_style {
            Some(v) => Value::Integer((v as i64).into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("hair_color".into()),
        match p.hair_color {
            Some(v) => Value::Integer((v as i64).into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_head".into()),
        match &p.equipped_head {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_body".into()),
        match &p.equipped_body {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_weapon".into()),
        match &p.equipped_weapon {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_back".into()),
        match &p.equipped_back {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_feet".into()),
        match &p.equipped_feet {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_ring".into()),
        match &p.equipped_ring {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_gloves".into()),
        match &p.equipped_gloves {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_necklace".into()),
        match &p.equipped_necklace {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("equipped_belt".into()),
        match &p.equipped_belt {
            Some(v) => Value::String(v.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((Value::String("is_admin".into()), Value::Boolean(p.is_admin)));
    pmap.push((Value::String("sitting".into()), Value::Boolean(p.sitting)));
    pmap.push((
        Value::String("is_gathering".into()),
        Value::Boolean(p.is_gathering),
    ));
    pmap.push((Value::String("dashing".into()), Value::Boolean(p.dashing)));
    pmap.push((
        Value::String("has_stall".into()),
        Value::Boolean(p.has_stall),
    ));
    pmap.push((
        Value::String("stall_name".into()),
        match &p.stall_name {
            Some(name) => Value::String(name.clone().into()),
            None => Value::Nil,
        },
    ));
    pmap.push((
        Value::String("combatStyle".into()),
        Value::String(p.combat_style.clone().into()),
    ));
    pmap.push((
        Value::String("mp".into()),
        Value::Integer((p.mp as i64).into()),
    ));
    pmap.push((
        Value::String("maxMp".into()),
        Value::Integer((p.max_mp as i64).into()),
    ));
    pmap.push((
        Value::String("title".into()),
        match &p.title {
            Some(t) => Value::String(t.clone().into()),
            None => Value::Nil,
        },
    ));
    Value::Map(pmap)
}

/// Pre-encode an NpcUpdate to rmpv::Value for reuse across per-player StateSync messages.
pub fn npc_update_to_value(n: &NpcUpdate) -> rmpv::Value {
    use rmpv::Value;
    let mut nmap = Vec::with_capacity(20);
    nmap.push((
        Value::String("id".into()),
        Value::String(n.id.clone().into()),
    ));
    nmap.push((
        Value::String("entity_type".into()),
        Value::String(n.entity_type.clone().into()),
    ));
    nmap.push((
        Value::String("prototype_id".into()),
        Value::String(n.prototype_id.clone().into()),
    ));
    nmap.push((
        Value::String("display_name".into()),
        Value::String(n.display_name.clone().into()),
    ));
    nmap.push((
        Value::String("x".into()),
        Value::Integer((n.x as i64).into()),
    ));
    nmap.push((
        Value::String("y".into()),
        Value::Integer((n.y as i64).into()),
    ));
    nmap.push((
        Value::String("z".into()),
        Value::Integer((n.z as i64).into()),
    ));
    nmap.push((
        Value::String("direction".into()),
        Value::Integer((n.direction as i64).into()),
    ));
    nmap.push((
        Value::String("hp".into()),
        Value::Integer((n.hp as i64).into()),
    ));
    nmap.push((
        Value::String("max_hp".into()),
        Value::Integer((n.max_hp as i64).into()),
    ));
    nmap.push((
        Value::String("level".into()),
        Value::Integer((n.level as i64).into()),
    ));
    nmap.push((
        Value::String("state".into()),
        Value::Integer((n.state as i64).into()),
    ));
    nmap.push((Value::String("hostile".into()), Value::Boolean(n.hostile)));
    nmap.push((
        Value::String("is_quest_giver".into()),
        Value::Boolean(n.is_quest_giver),
    ));
    nmap.push((
        Value::String("can_turn_in_quest".into()),
        Value::Boolean(n.can_turn_in_quest),
    ));
    nmap.push((
        Value::String("is_merchant".into()),
        Value::Boolean(n.is_merchant),
    ));
    nmap.push((Value::String("is_altar".into()), Value::Boolean(n.is_altar)));
    nmap.push((
        Value::String("is_banker".into()),
        Value::Boolean(n.is_banker),
    ));
    nmap.push((
        Value::String("is_slayer_master".into()),
        Value::Boolean(n.is_slayer_master),
    ));
    nmap.push((
        Value::String("is_friendly".into()),
        Value::Boolean(n.is_friendly),
    ));
    nmap.push((
        Value::String("is_port_master".into()),
        Value::Boolean(n.is_port_master),
    ));
    if let Some(ref st) = n.station_type {
        nmap.push((
            Value::String("station_type".into()),
            Value::String(st.clone().into()),
        ));
    }
    nmap.push((Value::String("move_speed".into()), Value::F32(n.move_speed)));
    nmap.push((
        Value::String("just_attacked".into()),
        Value::Boolean(n.just_attacked),
    ));
    nmap.push((
        Value::String("no_shadow".into()),
        Value::Boolean(n.no_shadow),
    ));
    nmap.push((
        Value::String("render_offset_y".into()),
        Value::F32(n.render_offset_y),
    ));
    if n.size > 1 {
        nmap.push((
            Value::String("size".into()),
            Value::Integer((n.size as i64).into()),
        ));
    }
    Value::Map(nmap)
}

/// Encode a SlayerTaskData to rmpv::Value (or Nil if None).
pub(super) fn slayer_task_to_value(task: &Option<SlayerTaskData>) -> rmpv::Value {
    use rmpv::Value;
    match task {
        Some(t) => {
            let mut map = Vec::new();
            map.push((
                Value::String("monster_id".into()),
                Value::String(t.monster_id.clone().into()),
            ));
            map.push((
                Value::String("display_name".into()),
                Value::String(t.display_name.clone().into()),
            ));
            map.push((
                Value::String("kills_current".into()),
                Value::Integer((t.kills_current as i64).into()),
            ));
            map.push((
                Value::String("kills_required".into()),
                Value::Integer((t.kills_required as i64).into()),
            ));
            map.push((
                Value::String("xp_per_kill".into()),
                Value::Integer(t.xp_per_kill.into()),
            ));
            map.push((
                Value::String("master_id".into()),
                Value::String(t.master_id.clone().into()),
            ));
            map.push((
                Value::String("points_on_complete".into()),
                Value::Integer((t.points_on_complete as i64).into()),
            ));
            Value::Map(map)
        }
        None => Value::Nil,
    }
}

/// Encode a SlayerRewardData to rmpv::Value.
pub(super) fn slayer_reward_to_value(r: &SlayerRewardData) -> rmpv::Value {
    use rmpv::Value;
    let mut map = Vec::new();
    map.push((
        Value::String("id".into()),
        Value::String(r.id.clone().into()),
    ));
    map.push((
        Value::String("display_name".into()),
        Value::String(r.display_name.clone().into()),
    ));
    map.push((
        Value::String("description".into()),
        Value::String(r.description.clone().into()),
    ));
    map.push((
        Value::String("cost".into()),
        Value::Integer((r.cost as i64).into()),
    ));
    map.push((
        Value::String("category".into()),
        Value::String(r.category.clone().into()),
    ));
    map.push((
        Value::String("target_id".into()),
        match &r.target_id {
            Some(id) => Value::String(id.clone().into()),
            None => Value::Nil,
        },
    ));
    map.push((
        Value::String("quantity".into()),
        Value::Integer((r.quantity as i64).into()),
    ));
    Value::Map(map)
}

/// Encode a StateSync message from pre-built rmpv::Values (avoids re-encoding per player).
pub fn encode_state_sync_from_values(
    tick: u64,
    player_values: Vec<rmpv::Value>,
    npc_values: Vec<rmpv::Value>,
    instance_id: &str,
) -> Result<Vec<u8>, String> {
    use rmpv::Value;
    let mut map = Vec::new();
    map.push((Value::String("tick".into()), Value::Integer(tick.into())));
    if !instance_id.is_empty() {
        map.push((
            Value::String("instanceId".into()),
            Value::String(instance_id.into()),
        ));
    }
    map.push((Value::String("players".into()), Value::Array(player_values)));
    map.push((Value::String("npcs".into()), Value::Array(npc_values)));

    let array = Value::Array(vec![
        Value::Integer(13.into()),
        Value::String("stateSync".into()),
        Value::Map(map),
    ]);

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &array)
        .map_err(|e| format!("Failed to encode message: {}", e))?;
    Ok(buf)
}

/// Encode a delta StateSync message with optional removed entity lists.
/// When `full` is true, this is a complete snapshot (same as encode_state_sync_from_values).
/// When `full` is false, only changed entities + removal lists are included.
pub fn encode_delta_state_sync(
    tick: u64,
    player_values: Vec<rmpv::Value>,
    npc_values: Vec<rmpv::Value>,
    instance_id: &str,
    full: bool,
    removed_players: &[String],
    removed_npcs: &[String],
) -> Result<Vec<u8>, String> {
    use rmpv::Value;
    let mut map = Vec::new();
    map.push((Value::String("tick".into()), Value::Integer(tick.into())));
    if !instance_id.is_empty() {
        map.push((
            Value::String("instanceId".into()),
            Value::String(instance_id.into()),
        ));
    }
    map.push((Value::String("players".into()), Value::Array(player_values)));
    map.push((Value::String("npcs".into()), Value::Array(npc_values)));

    if !full {
        map.push((Value::String("full".into()), Value::Boolean(false)));
        if !removed_players.is_empty() {
            map.push((
                Value::String("removedPlayers".into()),
                Value::Array(
                    removed_players
                        .iter()
                        .map(|id| Value::String(id.clone().into()))
                        .collect(),
                ),
            ));
        }
        if !removed_npcs.is_empty() {
            map.push((
                Value::String("removedNpcs".into()),
                Value::Array(
                    removed_npcs
                        .iter()
                        .map(|id| Value::String(id.clone().into()))
                        .collect(),
                ),
            ));
        }
    }

    let array = Value::Array(vec![
        Value::Integer(13.into()),
        Value::String("stateSync".into()),
        Value::Map(map),
    ]);

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &array)
        .map_err(|e| format!("Failed to encode message: {}", e))?;
    Ok(buf)
}
