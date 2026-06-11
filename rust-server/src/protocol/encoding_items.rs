use super::*;

pub(super) fn encode(msg: &ServerMessage) -> Option<Value> {
    let value = match msg {
        ServerMessage::ItemDropped {
            id,
            item_id,
            x,
            y,
            quantity,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((Value::String("x".into()), Value::F64(*x as f64)));
            map.push((Value::String("y".into()), Value::F64(*y as f64)));
            map.push((
                Value::String("quantity".into()),
                Value::Integer((*quantity as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ItemPickedUp { item_id, player_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ItemDespawned { item_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ItemQuantityUpdated { id, quantity } => {
            let mut map = Vec::new();
            map.push((Value::String("id".into()), Value::String(id.clone().into())));
            map.push((
                Value::String("quantity".into()),
                Value::Integer((*quantity as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::InventoryUpdate {
            player_id,
            slots,
            gold,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));

            let slot_values: Vec<Value> = slots
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((
                        Value::String("slot".into()),
                        Value::Integer((s.slot as i64).into()),
                    ));
                    smap.push((
                        Value::String("item_id".into()),
                        Value::String(s.item_id.clone().into()),
                    ));
                    smap.push((
                        Value::String("quantity".into()),
                        Value::Integer((s.quantity as i64).into()),
                    ));
                    Value::Map(smap)
                })
                .collect();

            map.push((Value::String("slots".into()), Value::Array(slot_values)));
            map.push((
                Value::String("gold".into()),
                Value::Integer((*gold as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ItemUsed {
            player_id,
            slot,
            item_id,
            effect,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("slot".into()),
                Value::Integer((*slot as i64).into()),
            ));
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("effect".into()),
                Value::String(effect.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ItemDefinitions { items } => {
            let mut map = Vec::new();
            let item_values: Vec<Value> = items
                .iter()
                .map(|i| {
                    let mut imap = Vec::new();
                    imap.push((
                        Value::String("id".into()),
                        Value::String(i.id.clone().into()),
                    ));
                    imap.push((
                        Value::String("displayName".into()),
                        Value::String(i.display_name.clone().into()),
                    ));
                    imap.push((
                        Value::String("sprite".into()),
                        Value::String(i.sprite.clone().into()),
                    ));
                    imap.push((
                        Value::String("category".into()),
                        Value::String(i.category.clone().into()),
                    ));
                    imap.push((
                        Value::String("maxStack".into()),
                        Value::Integer((i.max_stack as i64).into()),
                    ));
                    imap.push((
                        Value::String("description".into()),
                        Value::String(i.description.clone().into()),
                    ));
                    imap.push((
                        Value::String("basePrice".into()),
                        Value::Integer((i.base_price as i64).into()),
                    ));
                    imap.push((Value::String("sellable".into()), Value::Boolean(i.sellable)));
                    // Add equipment fields if present
                    if let Some(ref slot) = i.equipment_slot {
                        imap.push((
                            Value::String("equipment_slot".into()),
                            Value::String(slot.clone().into()),
                        ));
                    }
                    if let Some(level) = i.attack_level_required {
                        imap.push((
                            Value::String("attack_level_required".into()),
                            Value::Integer((level as i64).into()),
                        ));
                    }
                    if let Some(level) = i.defence_level_required {
                        imap.push((
                            Value::String("defence_level_required".into()),
                            Value::Integer((level as i64).into()),
                        ));
                    }
                    if let Some(level) = i.ranged_level_required {
                        imap.push((
                            Value::String("ranged_level_required".into()),
                            Value::Integer((level as i64).into()),
                        ));
                    }
                    if let Some(bonus) = i.attack_bonus {
                        imap.push((
                            Value::String("attack_bonus".into()),
                            Value::Integer((bonus as i64).into()),
                        ));
                    }
                    if let Some(bonus) = i.strength_bonus {
                        imap.push((
                            Value::String("strength_bonus".into()),
                            Value::Integer((bonus as i64).into()),
                        ));
                    }
                    if let Some(def) = i.defence_bonus {
                        imap.push((
                            Value::String("defence_bonus".into()),
                            Value::Integer((def as i64).into()),
                        ));
                    }
                    if let Some(bonus) = i.magic_bonus {
                        imap.push((
                            Value::String("magic_bonus".into()),
                            Value::Integer((bonus as i64).into()),
                        ));
                    }
                    if let Some(level) = i.magic_level_required {
                        imap.push((
                            Value::String("magic_level_required".into()),
                            Value::Integer((level as i64).into()),
                        ));
                    }
                    if let Some(ref wtype) = i.weapon_type {
                        imap.push((
                            Value::String("weapon_type".into()),
                            Value::String(wtype.clone().into()),
                        ));
                    }
                    if let Some(r) = i.range {
                        imap.push((
                            Value::String("range".into()),
                            Value::Integer((r as i64).into()),
                        ));
                    }
                    if i.prayer_xp > 0 {
                        imap.push((
                            Value::String("prayer_xp".into()),
                            Value::Integer((i.prayer_xp as i64).into()),
                        ));
                    }
                    if i.ranged_strength > 0 {
                        imap.push((
                            Value::String("ranged_strength".into()),
                            Value::Integer((i.ranged_strength as i64).into()),
                        ));
                    }
                    if let Some(bonus) = i.ranged_strength_bonus {
                        if bonus > 0 {
                            imap.push((
                                Value::String("ranged_strength_bonus".into()),
                                Value::Integer((bonus as i64).into()),
                            ));
                        }
                    }
                    // Woodcutting-specific fields
                    if let Some(level) = i.woodcutting_level_required {
                        imap.push((
                            Value::String("woodcutting_level_required".into()),
                            Value::Integer((level as i64).into()),
                        ));
                    }
                    if let Some(speed) = i.chop_speed_multiplier {
                        imap.push((
                            Value::String("chop_speed_multiplier".into()),
                            Value::F32(speed),
                        ));
                    }
                    // Mining-specific fields
                    if let Some(level) = i.mining_level_required {
                        imap.push((
                            Value::String("mining_level_required".into()),
                            Value::Integer((level as i64).into()),
                        ));
                    }
                    if let Some(speed) = i.mine_speed_multiplier {
                        imap.push((
                            Value::String("mine_speed_multiplier".into()),
                            Value::F32(speed),
                        ));
                    }
                    if let Some(ref effect) = i.use_effect_type {
                        imap.push((
                            Value::String("use_effect_type".into()),
                            Value::String(effect.clone().into()),
                        ));
                    }
                    Value::Map(imap)
                })
                .collect();
            map.push((Value::String("items".into()), Value::Array(item_values)));
            Value::Map(map)
        }
        ServerMessage::BuffApplied {
            player_id,
            buff_type,
            duration,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("buff_type".into()),
                Value::String(buff_type.clone().into()),
            ));
            map.push((
                Value::String("duration".into()),
                Value::Integer((*duration as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::BuffExpired {
            player_id,
            buff_type,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            map.push((
                Value::String("buff_type".into()),
                Value::String(buff_type.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::PotionBuffsSync { player_id, buffs } => {
            let mut map = Vec::new();
            map.push((
                Value::String("player_id".into()),
                Value::String(player_id.clone().into()),
            ));
            let buff_values: Vec<Value> = buffs
                .iter()
                .map(|b| {
                    let mut bmap = Vec::new();
                    bmap.push((
                        Value::String("stat".into()),
                        Value::String(b.stat.clone().into()),
                    ));
                    bmap.push((
                        Value::String("amount".into()),
                        Value::Integer((b.amount as i64).into()),
                    ));
                    bmap.push((
                        Value::String("remaining_ms".into()),
                        Value::Integer((b.remaining_ms as i64).into()),
                    ));
                    bmap.push((
                        Value::String("source_item_id".into()),
                        Value::String(b.source_item_id.clone().into()),
                    ));
                    Value::Map(bmap)
                })
                .collect();
            map.push((Value::String("buffs".into()), Value::Array(buff_values)));
            Value::Map(map)
        }
        ServerMessage::BankOpen {
            slots,
            gold,
            max_slots,
        } => {
            let mut map = Vec::new();

            let slot_values: Vec<Value> = slots
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((
                        Value::String("slot".into()),
                        Value::Integer((s.slot as i64).into()),
                    ));
                    smap.push((
                        Value::String("item_id".into()),
                        Value::String(s.item_id.clone().into()),
                    ));
                    smap.push((
                        Value::String("quantity".into()),
                        Value::Integer((s.quantity as i64).into()),
                    ));
                    Value::Map(smap)
                })
                .collect();

            map.push((Value::String("slots".into()), Value::Array(slot_values)));
            map.push((
                Value::String("gold".into()),
                Value::Integer((*gold as i64).into()),
            ));
            map.push((
                Value::String("max_slots".into()),
                Value::Integer((*max_slots as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::BankUpdate { slots, gold } => {
            let mut map = Vec::new();

            let slot_values: Vec<Value> = slots
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((
                        Value::String("slot".into()),
                        Value::Integer((s.slot as i64).into()),
                    ));
                    smap.push((
                        Value::String("item_id".into()),
                        Value::String(s.item_id.clone().into()),
                    ));
                    smap.push((
                        Value::String("quantity".into()),
                        Value::Integer((s.quantity as i64).into()),
                    ));
                    Value::Map(smap)
                })
                .collect();

            map.push((Value::String("slots".into()), Value::Array(slot_values)));
            map.push((
                Value::String("gold".into()),
                Value::Integer((*gold as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::BankResult {
            success,
            action,
            error,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("action".into()),
                Value::String(action.clone().into()),
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
        ServerMessage::ChestOpen {
            chest_id,
            name,
            slots,
            total_value,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("chest_id".into()),
                Value::String(chest_id.clone().into()),
            ));
            map.push((
                Value::String("name".into()),
                Value::String(name.clone().into()),
            ));

            let slot_values: Vec<Value> = slots
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((
                        Value::String("slot".into()),
                        Value::Integer((s.slot as i64).into()),
                    ));
                    smap.push((
                        Value::String("item_id".into()),
                        Value::String(s.item_id.clone().into()),
                    ));
                    smap.push((
                        Value::String("quantity".into()),
                        Value::Integer((s.quantity as i64).into()),
                    ));
                    smap.push((
                        Value::String("value".into()),
                        Value::Integer((s.value as i64).into()),
                    ));
                    Value::Map(smap)
                })
                .collect();

            map.push((Value::String("slots".into()), Value::Array(slot_values)));
            map.push((
                Value::String("total_value".into()),
                Value::Integer((*total_value as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ChestUpdate {
            chest_id,
            slots,
            total_value,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("chest_id".into()),
                Value::String(chest_id.clone().into()),
            ));

            let slot_values: Vec<Value> = slots
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((
                        Value::String("slot".into()),
                        Value::Integer((s.slot as i64).into()),
                    ));
                    smap.push((
                        Value::String("item_id".into()),
                        Value::String(s.item_id.clone().into()),
                    ));
                    smap.push((
                        Value::String("quantity".into()),
                        Value::Integer((s.quantity as i64).into()),
                    ));
                    smap.push((
                        Value::String("value".into()),
                        Value::Integer((s.value as i64).into()),
                    ));
                    Value::Map(smap)
                })
                .collect();

            map.push((Value::String("slots".into()), Value::Array(slot_values)));
            map.push((
                Value::String("total_value".into()),
                Value::Integer((*total_value as i64).into()),
            ));
            Value::Map(map)
        }

        // ===== Trade System Messages =====
        ServerMessage::CollectionLogDefinitions {
            entries,
            display_names,
        } => {
            let items: Vec<Value> = entries
                .iter()
                .map(|(item_id, source, source_detail)| {
                    Value::Array(vec![
                        Value::String(item_id.clone().into()),
                        Value::String(source.clone().into()),
                        Value::String(source_detail.clone().into()),
                    ])
                })
                .collect();
            let names: Vec<Value> = display_names
                .iter()
                .map(|(id, name)| {
                    Value::Array(vec![
                        Value::String(id.clone().into()),
                        Value::String(name.clone().into()),
                    ])
                })
                .collect();
            let mut map = Vec::new();
            map.push((Value::String("entries".into()), Value::Array(items)));
            map.push((Value::String("display_names".into()), Value::Array(names)));
            Value::Map(map)
        }
        ServerMessage::CollectionLogSync { entries } => {
            let items: Vec<Value> = entries
                .iter()
                .map(|(item_id, source, source_detail, obtained_at)| {
                    Value::Array(vec![
                        Value::String(item_id.clone().into()),
                        Value::String(source.clone().into()),
                        Value::String(source_detail.clone().into()),
                        Value::String(obtained_at.clone().into()),
                    ])
                })
                .collect();
            let mut map = Vec::new();
            map.push((Value::String("entries".into()), Value::Array(items)));
            Value::Map(map)
        }
        ServerMessage::CollectionLogEntry {
            item_id,
            source,
            source_detail,
            obtained_at,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("source".into()),
                Value::String(source.clone().into()),
            ));
            map.push((
                Value::String("source_detail".into()),
                Value::String(source_detail.clone().into()),
            ));
            map.push((
                Value::String("obtained_at".into()),
                Value::String(obtained_at.clone().into()),
            ));
            Value::Map(map)
        }
        _ => return None,
    };
    Some(value)
}
