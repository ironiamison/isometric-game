# Use Item on Entity — Design

## Overview

Add an "use item on entity" mechanic where the player selects an inventory item, then clicks a world entity to use it on. Like OSRS: select tinderbox, click candle, it lights. Supports both Lua quest callbacks (complex logic) and TOML-defined interactions (generic actions).

## Client — Item Selection

Clicking an inventory slot selects the item — highlighted with a glow/border. Cursor may show the item sprite or a use-mode indicator.

- Click same slot again or press ESC → deselect
- Click different inventory slot → switch selection
- Click empty ground → deselect (no movement)
- Click world entity within interaction range → send `UseItemOnEntity` message
- Click world entity out of range → walk to entity, fire action on arrival

**New client state:**
- `ui_state.selected_inventory_slot: Option<usize>`

**New input command:**
- `InputCommand::UseItemOnEntity { slot_index: u8, npc_id: String }`

Works identically on desktop (click) and mobile (tap).

## Protocol

**New client message:**
```
ClientMessage::UseItemOnEntity { slot_index: u8, npc_id: String }
```

## Server Routing

Handler flow:
1. Resolve `slot_index` → `item_id` from player inventory
2. Resolve `npc_id` → `entity_type` and `unique_id` from the NPC/entity
3. Range check (same interaction range as NPC talking)
4. Check active quest Lua scripts for `on_use_item` handler → call with `(ctx, item_id, entity_type, unique_id)`
5. If Lua returns `true` → handled, done
6. If no Lua handler or returns `false` → check TOML item interactions
7. If TOML match → execute action (message, consume item, etc.)
8. If nothing matches → "Nothing interesting happens."

## Lua Callback

```lua
function on_use_item(ctx, item_id, entity_type, unique_id)
    if item_id == "tinderbox" and entity_type == "haunted_candles" then
        handle_candle_light(ctx, unique_id)
        return true
    end
    return false
end
```

Receives the full quest context (same as `on_interact`) — can read/write flags, show dialogue, give items. Returns `true` if handled, `false` to fall through to TOML.

Called for ALL active quests that have an `on_use_item` function. First quest that returns `true` wins.

## TOML Item Interactions

For generic non-quest actions. Defined in `rust-server/data/item_interactions.toml`:

```toml
[[interactions]]
item = "bone"
target_entity = "altar"
action = "bury"
message = "You bury the bones at the altar."
consume_item = true

[[interactions]]
item = "bucket"
target_entity = "well"
action = "fill"
message = "You fill the bucket with water."
consume_item = false
result_item = "bucket_water"
```

## Candle Puzzle (Ghastly Contraption)

Four `haunted_candles` entities with unique IDs:
- `candle_1` (32,45) → Skull candle
- `candle_2` (32,36) → Tall taper
- `candle_3` (45,45) → Red candle
- `candle_4` (45,36) → Small stubby candle

**Correct order:** candle_1 → candle_2 → candle_3 → candle_4

**Quest flag:** `candles_lit` — comma-separated list (e.g. `"candle_1,candle_2"`)

**Flow:**
1. Player uses tinderbox on candle → `on_use_item` fires
2. Read `candles_lit` flag, parse list
3. Check if this candle is correct next in sequence
4. Correct → add to list, save flag, show flame message
5. All 4 lit → gate opens, `open_first_gate` objective completes, notification
6. Wrong → reset flag, show failure ("cold wind snuffs the flames")

Each candle has a distinct flame color on success:
- Skull: eerie green
- Tall: pale blue
- Red: warm orange
- Small: sputtering yellow
