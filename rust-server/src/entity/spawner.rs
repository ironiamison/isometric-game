use rand::Rng;

use super::prototype::EntityPrototype;
use crate::item::{GOLD_ITEM_ID, GroundItem};

/// Generate loot drops from a prototype's loot table
pub fn generate_loot_from_prototype(
    prototype: &EntityPrototype,
    x: f32,
    y: f32,
    killer_id: &str,
    current_time: u64,
    level: i32,
    instance_id: Option<String>,
) -> Vec<GroundItem> {
    let mut drops = Vec::new();
    let mut rng = rand::thread_rng();
    let mut item_counter = 0u32;

    // Gold drop
    if prototype.rewards.gold_max > 0 {
        let gold_amount =
            rng.r#gen_range(prototype.rewards.gold_min..=prototype.rewards.gold_max) * level; // Scale by level

        if gold_amount > 0 {
            let id = format!("item_{}_{}", current_time, item_counter);
            item_counter += 1;
            drops.push(GroundItem::new_in_instance(
                &id,
                GOLD_ITEM_ID,
                x,
                y,
                gold_amount,
                Some(killer_id.to_string()),
                current_time,
                instance_id.clone(),
            ));
        }
    }

    // Flat loot drops - independent rolls
    for entry in &prototype.loot {
        if rng.r#gen::<f32>() < entry.drop_chance {
            let quantity = rng.r#gen_range(entry.quantity_min..=entry.quantity_max);
            if quantity > 0 {
                let id = format!("item_{}_{}", current_time, item_counter);
                item_counter += 1;

                drops.push(GroundItem::new_in_instance(
                    &id,
                    &entry.item_id,
                    x,
                    y,
                    quantity,
                    Some(killer_id.to_string()),
                    current_time,
                    instance_id.clone(),
                ));
            }
        }
    }

    // Roll tables - each table activates by chance, then picks one weighted entry
    for table in &prototype.loot_tables {
        // Check activation chance
        if rng.r#gen::<f32>() >= table.chance {
            continue;
        }

        // Sum weights
        let total_weight: i32 = table.entries.iter().map(|e| e.weight).sum();
        if total_weight <= 0 {
            continue;
        }

        // Pick a weighted entry
        let mut roll = rng.r#gen_range(0..total_weight);
        for entry in &table.entries {
            roll -= entry.weight;
            if roll < 0 {
                // "nothing" is a reserved keyword meaning no drop
                if entry.item_id != "nothing" {
                    let quantity = rng.r#gen_range(entry.quantity_min..=entry.quantity_max);
                    if quantity > 0 {
                        let id = format!("item_{}_{}", current_time, item_counter);
                        item_counter += 1;

                        drops.push(GroundItem::new_in_instance(
                            &id,
                            &entry.item_id,
                            x,
                            y,
                            quantity,
                            Some(killer_id.to_string()),
                            current_time,
                            instance_id.clone(),
                        ));
                    }
                }
                break;
            }
        }
    }

    drops
}

/// Calculate exp reward for killing an entity
pub fn calculate_exp_reward(prototype: &EntityPrototype, level: i32) -> i32 {
    prototype.rewards.exp_base * level
}
