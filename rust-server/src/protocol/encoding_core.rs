use super::*;

pub(super) fn encode(msg: &ServerMessage) -> Option<Value> {
    let value = match msg {
        ServerMessage::Welcome {
            player_id,
            is_new_character,
            protocol_version,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("is_new_character".into()),
                Value::Boolean(*is_new_character),
            ));
            map.push((
                Value::String("protocol_version".into()),
                Value::Integer(u64::from(*protocol_version).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::PlayerJoined {
            id,
            name,
            x,
            y,
            gender,
            skin,
            hair_style,
            hair_color,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            map.push((
                Value::String("name".into()),
                Value::String(name.clone().into()),
            ));
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            map.push((
                Value::String("gender".into()),
                Value::String(gender.clone().into()),
            ));
            map.push((
                Value::String("skin".into()),
                Value::String(skin.clone().into()),
            ));
            map.push((
                Value::String("hair_style".into()),
                match hair_style {
                    Some(style) => Value::Integer((*style as i64).into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("hair_color".into()),
                match hair_color {
                    Some(color) => Value::Integer((*color as i64).into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::PlayerLeft { id } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            Value::Map(map)
        }
        ServerMessage::StateSync {
            tick,
            players,
            npcs,
            instance_id,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("tick".into()), Value::Integer((*tick).into())));
            if !instance_id.is_empty() {
                map.push((
                    Value::String("instanceId".into()),
                    Value::String(instance_id.clone().into()),
                ));
            }

            let player_values: Vec<Value> = players
                .iter()
                .map(|p| {
                    let mut pmap = Vec::new();
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
                        Value::String("direction".into()),
                        Value::Integer((p.direction as i64).into()),
                    ));
                    // Include velocity for client-side prediction
                    pmap.push((
                        Value::String("velX".into()),
                        Value::Integer((p.vel_x as i64).into()),
                    ));
                    pmap.push((
                        Value::String("velY".into()),
                        Value::Integer((p.vel_y as i64).into()),
                    ));
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
                    // Individual skill levels
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
                            Some(style) => Value::Integer((style as i64).into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("hair_color".into()),
                        match p.hair_color {
                            Some(color) => Value::Integer((color as i64).into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_head".into()),
                        match &p.equipped_head {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_body".into()),
                        match &p.equipped_body {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_weapon".into()),
                        match &p.equipped_weapon {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_back".into()),
                        match &p.equipped_back {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_feet".into()),
                        match &p.equipped_feet {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_ring".into()),
                        match &p.equipped_ring {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_gloves".into()),
                        match &p.equipped_gloves {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_necklace".into()),
                        match &p.equipped_necklace {
                            Some(item_id) => Value::String(item_id.clone().into()),
                            None => Value::Nil,
                        },
                    ));
                    pmap.push((
                        Value::String("equipped_belt".into()),
                        match &p.equipped_belt {
                            Some(item_id) => Value::String(item_id.clone().into()),
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
                    Value::Map(pmap)
                })
                .collect();
            map.push((Value::String("players".into()), Value::Array(player_values)));

            let npc_values: Vec<Value> = npcs
                .iter()
                .map(|n| {
                    let mut nmap = Vec::new();
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
                    Value::Map(nmap)
                })
                .collect();
            map.push((Value::String("npcs".into()), Value::Array(npc_values)));

            Value::Map(map)
        }
        ServerMessage::ChatMessage {
            sender_id,
            sender_name,
            text,
            timestamp,
            channel,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("senderId".into()),
                Value::String(sender_id.clone().into()),
            ));
            map.push((
                Value::String("senderName".into()),
                Value::String(sender_name.clone().into()),
            ));
            map.push((
                Value::String("text".into()),
                Value::String(text.clone().into()),
            ));
            map.push((
                Value::String("timestamp".into()),
                Value::Integer((*timestamp).into()),
            ));
            map.push((
                Value::String("channel".into()),
                Value::String(channel.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::TargetChanged {
            player_id,
            target_id,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("target_id".into()),
                match target_id {
                    Some(id) => Value::String(id.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::Error { code, message } => {
            let mut map = Vec::new();
            map.push((
                Value::String("code".into()),
                Value::Integer((*code as i64).into()),
            ));
            map.push((
                Value::String("message".into()),
                Value::String(message.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::Announcement { text } => {
            let mut map = Vec::new();
            map.push((
                Value::String("text".into()),
                Value::String(text.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::NpcSpeech { npc_id, message } => {
            let mut map = Vec::new();
            map.push((
                Value::String("npcId".into()),
                Value::String(npc_id.clone().into()),
            ));
            map.push((
                Value::String("message".into()),
                Value::String(message.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::Pong { timestamp } => {
            let mut map = Vec::new();
            map.push((Value::String("timestamp".into()), Value::F64(*timestamp)));
            Value::Map(map)
        }
        ServerMessage::TopPlayerChanged {
            player_name,
            second_player_name,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_name".into()),
                match player_name {
                    Some(name) => Value::String(name.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("second_player_name".into()),
                match second_player_name {
                    Some(name) => Value::String(name.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        _ => return None,
    };
    Some(value)
}
