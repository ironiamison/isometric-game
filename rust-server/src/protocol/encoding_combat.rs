use super::*;

pub(super) fn encode(msg: &ServerMessage) -> Option<Value> {
    let value = match msg {
        ServerMessage::PlayerAttack {
            player_id,
            attack_type,
            direction,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("attack_type".into()),
                Value::String(attack_type.clone().into()),
            ));
            map.push((
                Value::String("direction".into()),
                Value::Integer((*direction as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::DamageEvent {
            source_id,
            target_id,
            damage,
            target_hp,
            target_x,
            target_y,
            projectile,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("source_id".into()),
                Value::String(source_id.clone().into()),
            ));
            map.push((
                Value::String("target_id".into()),
                Value::String(target_id.clone().into()),
            ));
            map.push((
                Value::String("damage".into()),
                Value::Integer((*damage as i64).into()),
            ));
            map.push((
                Value::String("target_hp".into()),
                Value::Integer((*target_hp as i64).into()),
            ));
            map.push((
                Value::String("target_x".into()),
                Value::F64(*target_x as f64),
            ));
            map.push((
                Value::String("target_y".into()),
                Value::F64(*target_y as f64),
            ));
            map.push((
                Value::String("projectile".into()),
                match projectile {
                    Some(p) => Value::String(p.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::AttackResult { success, reason } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("reason".into()),
                match reason {
                    Some(r) => Value::String(r.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::NpcDied { id, killer_id } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            map.push((
                Value::String("killer_id".into()),
                Value::String(killer_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::NpcRespawned { id, x, y } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::PlayerDied { id, killer_id } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            map.push((
                Value::String("killer_id".into()),
                Value::String(killer_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::PlayerRespawned { id, x, y, hp } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            map.push((
                Value::String("x".into()),
                Value::Integer((*x as i64).into()),
            ));
            map.push((
                Value::String("y".into()),
                Value::Integer((*y as i64).into()),
            ));
            map.push((
                Value::String("hp".into()),
                Value::Integer((*hp as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::SkillXp {
            player_id,
            skill,
            xp_gained,
            total_xp,
            level,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("skill".into()),
                Value::String(skill.clone().into()),
            ));
            map.push((
                Value::String("xp_gained".into()),
                Value::Integer((*xp_gained).into()),
            ));
            map.push((
                Value::String("total_xp".into()),
                Value::Integer((*total_xp).into()),
            ));
            map.push((
                Value::String("level".into()),
                Value::Integer((*level as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::SkillLevelUp {
            player_id,
            skill,
            new_level,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("skill".into()),
                Value::String(skill.clone().into()),
            ));
            map.push((
                Value::String("new_level".into()),
                Value::Integer((*new_level as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::SkillsSync {
            player_id,
            hitpoints_level,
            hitpoints_xp,
            attack_level,
            attack_xp,
            strength_level,
            strength_xp,
            defence_level,
            defence_xp,
            ranged_level,
            ranged_xp,
            fishing_level,
            fishing_xp,
            farming_level,
            farming_xp,
            smithing_level,
            smithing_xp,
            prayer_level,
            prayer_xp,
            magic_level,
            magic_xp,
            woodcutting_level,
            woodcutting_xp,
            alchemy_level,
            alchemy_xp,
            mining_level,
            mining_xp,
            slayer_level,
            slayer_xp,
            survivalist_level,
            survivalist_xp,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("hitpoints_level".into()),
                Value::Integer((*hitpoints_level as i64).into()),
            ));
            map.push((
                Value::String("hitpoints_xp".into()),
                Value::Integer((*hitpoints_xp).into()),
            ));
            map.push((
                Value::String("attack_level".into()),
                Value::Integer((*attack_level as i64).into()),
            ));
            map.push((
                Value::String("attack_xp".into()),
                Value::Integer((*attack_xp).into()),
            ));
            map.push((
                Value::String("strength_level".into()),
                Value::Integer((*strength_level as i64).into()),
            ));
            map.push((
                Value::String("strength_xp".into()),
                Value::Integer((*strength_xp).into()),
            ));
            map.push((
                Value::String("defence_level".into()),
                Value::Integer((*defence_level as i64).into()),
            ));
            map.push((
                Value::String("defence_xp".into()),
                Value::Integer((*defence_xp).into()),
            ));
            map.push((
                Value::String("ranged_level".into()),
                Value::Integer((*ranged_level as i64).into()),
            ));
            map.push((
                Value::String("ranged_xp".into()),
                Value::Integer((*ranged_xp).into()),
            ));
            map.push((
                Value::String("fishing_level".into()),
                Value::Integer((*fishing_level as i64).into()),
            ));
            map.push((
                Value::String("fishing_xp".into()),
                Value::Integer((*fishing_xp).into()),
            ));
            map.push((
                Value::String("farming_level".into()),
                Value::Integer((*farming_level as i64).into()),
            ));
            map.push((
                Value::String("farming_xp".into()),
                Value::Integer((*farming_xp).into()),
            ));
            map.push((
                Value::String("smithing_level".into()),
                Value::Integer((*smithing_level as i64).into()),
            ));
            map.push((
                Value::String("smithing_xp".into()),
                Value::Integer((*smithing_xp).into()),
            ));
            map.push((
                Value::String("prayer_level".into()),
                Value::Integer((*prayer_level as i64).into()),
            ));
            map.push((
                Value::String("prayer_xp".into()),
                Value::Integer((*prayer_xp).into()),
            ));
            map.push((
                Value::String("magic_level".into()),
                Value::Integer((*magic_level as i64).into()),
            ));
            map.push((
                Value::String("magic_xp".into()),
                Value::Integer((*magic_xp).into()),
            ));
            map.push((
                Value::String("woodcutting_level".into()),
                Value::Integer((*woodcutting_level as i64).into()),
            ));
            map.push((
                Value::String("woodcutting_xp".into()),
                Value::Integer((*woodcutting_xp).into()),
            ));
            map.push((
                Value::String("alchemy_level".into()),
                Value::Integer((*alchemy_level as i64).into()),
            ));
            map.push((
                Value::String("alchemy_xp".into()),
                Value::Integer((*alchemy_xp).into()),
            ));
            map.push((
                Value::String("mining_level".into()),
                Value::Integer((*mining_level as i64).into()),
            ));
            map.push((
                Value::String("mining_xp".into()),
                Value::Integer((*mining_xp).into()),
            ));
            map.push((
                Value::String("slayer_level".into()),
                Value::Integer((*slayer_level as i64).into()),
            ));
            map.push((
                Value::String("slayer_xp".into()),
                Value::Integer((*slayer_xp).into()),
            ));
            map.push((
                Value::String("survivalist_level".into()),
                Value::Integer((*survivalist_level as i64).into()),
            ));
            map.push((
                Value::String("survivalist_xp".into()),
                Value::Integer((*survivalist_xp).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::EquipmentUpdate {
            player_id,
            equipped_head,
            equipped_body,
            equipped_weapon,
            equipped_back,
            equipped_feet,
            equipped_ring,
            equipped_gloves,
            equipped_necklace,
            equipped_belt,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("equipped_head".into()),
                match equipped_head {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_body".into()),
                match equipped_body {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_weapon".into()),
                match equipped_weapon {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_back".into()),
                match equipped_back {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_feet".into()),
                match equipped_feet {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_ring".into()),
                match equipped_ring {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_gloves".into()),
                match equipped_gloves {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_necklace".into()),
                match equipped_necklace {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("equipped_belt".into()),
                match equipped_belt {
                    Some(item_id) => Value::String(item_id.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::EquipResult {
            success,
            slot_type,
            item_id,
            error,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("slot_type".into()),
                Value::String(slot_type.clone().into()),
            ));
            map.push((
                Value::String("item_id".into()),
                match item_id {
                    Some(id) => Value::String(id.clone().into()),
                    None => Value::Nil,
                },
            ));
            map.push((
                Value::String("error".into()),
                match error {
                    Some(e) => Value::String(e.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::PrayerStateUpdate {
            points,
            max_points,
            active_prayers,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("points".into()),
                Value::Integer((*points as i64).into()),
            ));
            map.push((
                Value::String("max_points".into()),
                Value::Integer((*max_points as i64).into()),
            ));
            let prayer_values: Vec<Value> = active_prayers
                .iter()
                .map(|p| Value::String(p.clone().into()))
                .collect();
            map.push((
                Value::String("active_prayers".into()),
                Value::Array(prayer_values),
            ));
            Value::Map(map)
        }
        ServerMessage::SpellEffect {
            caster_id,
            target_id,
            spell_id,
            target_x,
            target_y,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("caster_id".into()),
                Value::String(caster_id.clone().into()),
            ));
            match target_id {
                Some(tid) => map.push((
                    Value::String("target_id".into()),
                    Value::String(tid.clone().into()),
                )),
                None => map.push((Value::String("target_id".into()), Value::Nil)),
            }
            map.push((
                Value::String("spell_id".into()),
                Value::String(spell_id.clone().into()),
            ));
            map.push((
                Value::String("target_x".into()),
                Value::Integer((*target_x as i64).into()),
            ));
            map.push((
                Value::String("target_y".into()),
                Value::Integer((*target_y as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::SpellResult {
            success,
            reason,
            spell_id,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            match reason {
                Some(r) => map.push((
                    Value::String("reason".into()),
                    Value::String(r.clone().into()),
                )),
                None => map.push((Value::String("reason".into()), Value::Nil)),
            }
            if let Some(id) = spell_id {
                map.push((
                    Value::String("spell_id".into()),
                    Value::String(id.clone().into()),
                ));
            }
            Value::Map(map)
        }
        // Woodcutting system messages
        ServerMessage::AutoRetaliateChanged { enabled } => {
            let mut map = Vec::new();
            map.push((Value::String("enabled".into()), Value::Boolean(*enabled)));
            Value::Map(map)
        }
        ServerMessage::ScrollSpellDefinitions { spells } => {
            let mut map = Vec::new();
            let spell_values: Vec<Value> = spells
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((
                        Value::String("id".into()),
                        Value::String(s.id.clone().into()),
                    ));
                    smap.push((
                        Value::String("name".into()),
                        Value::String(s.name.clone().into()),
                    ));
                    smap.push((
                        Value::String("spell_type".into()),
                        Value::String(s.spell_type.clone().into()),
                    ));
                    smap.push((
                        Value::String("mana_cost".into()),
                        Value::Integer((s.mana_cost as i64).into()),
                    ));
                    smap.push((
                        Value::String("cooldown_ms".into()),
                        Value::Integer((s.cooldown_ms as i64).into()),
                    ));
                    smap.push((
                        Value::String("base_power".into()),
                        Value::Integer((s.base_power as i64).into()),
                    ));
                    smap.push((
                        Value::String("effect_sprite".into()),
                        Value::String(s.effect_sprite.clone().into()),
                    ));
                    smap.push((
                        Value::String("pushback_distance".into()),
                        Value::Integer((s.pushback_distance as i64).into()),
                    ));
                    smap.push((
                        Value::String("wall_slam_damage_per_tile".into()),
                        Value::Integer((s.wall_slam_damage_per_tile as i64).into()),
                    ));
                    smap.push((
                        Value::String("description".into()),
                        Value::String(s.description.clone().into()),
                    ));
                    Value::Map(smap)
                })
                .collect();
            map.push((Value::String("spells".into()), Value::Array(spell_values)));
            Value::Map(map)
        }
        ServerMessage::UnlockedSpellsSync { spell_ids } => {
            let mut map = Vec::new();
            let ids: Vec<Value> = spell_ids
                .iter()
                .map(|s| Value::String(s.clone().into()))
                .collect();
            map.push((Value::String("spell_ids".into()), Value::Array(ids)));
            Value::Map(map)
        }
        ServerMessage::SpellUnlocked { spell_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("spell_id".into()),
                Value::String(spell_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::Pushback {
            target_id,
            from_x,
            from_y,
            to_x,
            to_y,
            wall_slam,
            bonus_damage,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("target_id".into()),
                Value::String(target_id.clone().into()),
            ));
            map.push((
                Value::String("from_x".into()),
                Value::Integer((*from_x as i64).into()),
            ));
            map.push((
                Value::String("from_y".into()),
                Value::Integer((*from_y as i64).into()),
            ));
            map.push((
                Value::String("to_x".into()),
                Value::Integer((*to_x as i64).into()),
            ));
            map.push((
                Value::String("to_y".into()),
                Value::Integer((*to_y as i64).into()),
            ));
            map.push((
                Value::String("wall_slam".into()),
                Value::Boolean(*wall_slam),
            ));
            map.push((
                Value::String("bonus_damage".into()),
                Value::Integer((*bonus_damage as i64).into()),
            ));
            Value::Map(map)
        }
        _ => return None,
    };
    Some(value)
}
