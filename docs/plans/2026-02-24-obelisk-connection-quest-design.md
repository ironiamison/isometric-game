# Obelisk Connection Quest - Design Document

## Overview

A new quest where the player discovers a magical obelisk south-east of the starting town, meets a scholar studying it, and is sent on a journey to restore the broken connection to a second obelisk far to the north. The reward is unlocking fast-travel between the two obelisks.

## Quest Flow

### Phase 1 - The Scholar (First Obelisk: 88, 34)
1. Player finds **Researcher Orin** (new NPC at 89, 34) standing next to the southern obelisk
2. Talk to Orin → He explains the obelisk is a magical waystone linked to another far to the north, but the connection is broken
3. He can't leave his post, so he asks the player to travel north and try to restore it
4. **Objective: Reach the second obelisk at (92, -163)**

### Phase 2 - The Blocked Obelisk (Second Obelisk: 92, -163)
5. Player arrives and interacts with the obelisk → dialogue: *"The stone hums faintly... something buried beneath is disrupting the flow of energy. You'll need to dig it out."* It also hints at a shovel lying nearby.
6. A **persistent ground item spawn** places a shovel a few tiles from the obelisk
7. Player picks up the shovel, uses it (double-click from inventory) while standing on a **dig site trigger tile** ~4-5 tiles from the obelisk
8. A **hedgehog** bursts out of the ground (quest-spawned, level 6)
9. **Objective: Kill the hedgehog**

### Phase 3 - Restoration
10. After the hedgehog dies, player interacts with the obelisk again → *"The stone pulses with renewed energy. The connection is restored!"*
11. **Objective: Return to Researcher Orin**

### Phase 4 - Reward
12. Talk to Orin → he's delighted, both obelisks are unlocked as **waystone fast-travel points**

---

## New Data Files

### 1. Quest Definition: `rust-server/data/quests/exploration/obelisk_connection.toml`

```toml
[quest]
id = "obelisk_connection"
name = "The Obelisk Connection"
description = "Researcher Orin has discovered that two ancient obelisks share a magical link, but the connection is broken. Travel north to find the second obelisk and restore it."
giver_npc = "researcher_orin"
level_required = 1
repeatable = false
lua_script = "exploration/obelisk_connection.lua"

[[quest.objectives]]
id = "reach_north_obelisk"
type = "reach_location"
target = "north_obelisk"
count = 1
description = "Find the northern obelisk"

[[quest.objectives]]
id = "kill_hedgehog"
type = "kill_monster"
target = "hedgehog"
count = 1
description = "Defeat the creature blocking the connection"
sequential = true

[[quest.objectives]]
id = "return_to_orin"
type = "talk_to"
target = "researcher_orin"
description = "Return to Researcher Orin"
sequential = true

[quest.rewards]
exp = 500
gold = 75

[quest.dialogue]
offer = "Ah, a traveler! You see this obelisk? It's no ordinary stone. I've been studying it for weeks. It resonates with magical energy - and I believe there's another one just like it, far to the north. They were once connected, but something has severed the link. I can't leave my research here, but perhaps you could find the other obelisk and try to restore the connection?"
accept = "Wonderful! Head north - far north. The second obelisk should be out in the wilderness. When you find it, try interacting with it. The stones have a will of their own. Good luck, traveler!"
progress = "Still searching for the northern obelisk? It's a long journey, but I believe in you. Head north and keep your eyes open."
complete = "You did it! I can feel the resonance already - the connection between the stones is alive again! As a reward, both obelisks should now respond to your touch. You can use them to travel between the two locations instantly. Remarkable work, truly remarkable!"
```

### 2. Quest Lua Script: `rust-server/data/scripts/quests/exploration/obelisk_connection.lua`

```lua
-- Obelisk Connection Quest Script
-- Given by Researcher Orin near the southern obelisk

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        return show_completed_dialogue(ctx)
    end
end

function show_offer_dialogue(ctx)
    local choice = ctx:show_dialogue({
        speaker = "Researcher Orin",
        text = "Ah, a traveler! You see this obelisk? It's no ordinary stone. I've spent weeks studying its resonance - it pulses with ancient magic. I believe there's another one far to the north, and they were once connected. Something has severed the link.\n\nI can't leave my research here, but... would you be willing to find the other obelisk and try to restore the connection?",
        choices = {
            { id = "accept", text = "I'll find it." },
            { id = "decline", text = "Not right now." },
            { id = "ask_more", text = "What kind of magic?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Researcher Orin",
            text = "Wonderful! Head north - far north. The second obelisk should be deep in the wilderness. When you find it, try touching it. The stones seem to respond to people. Good luck, traveler!"
        })
    elseif choice == "ask_more" then
        ctx:show_dialogue({
            speaker = "Researcher Orin",
            text = "Teleportation magic, I believe. In the old texts, they called them waystones - paired monuments that could transport someone from one to the other in an instant. Imagine the possibilities!"
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Researcher Orin",
            text = "No rush, friend. The stones have been here for centuries. They'll wait a little longer."
        })
    end
end

function show_progress_dialogue(ctx)
    local objectives = ctx:get_objectives()

    if not objectives.reach_north_obelisk.completed then
        ctx:show_dialogue({
            speaker = "Researcher Orin",
            text = "Still here? The northern obelisk is a long journey from here. Head north and keep your eyes open - you'll know it when you see it."
        })
    elseif not objectives.kill_hedgehog.completed then
        ctx:show_dialogue({
            speaker = "Researcher Orin",
            text = "You found it? But there's something blocking the connection? Hmm... the old texts mention that sometimes creatures nest near sources of magical energy. You may need to clear whatever is disrupting it."
        })
    end
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Researcher Orin",
        text = "You did it! I can feel the resonance humming between the stones - the connection is alive again! Both obelisks should now respond to your touch. Step up to either one and you can travel to the other in an instant. Remarkable work, truly remarkable!"
    })
    ctx:complete_quest()
    ctx:unlock_waystone("south_obelisk")
    ctx:unlock_waystone("north_obelisk")
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Researcher Orin",
        text = "The resonance is strong today. The waystones are working beautifully. Thank you again, traveler."
    })
end
```

### 3. Quest Location: add to `rust-server/data/quest_locations.toml`

```toml
[north_obelisk]
x = 92
y = -163
radius = 3
```

### 4. NPC Definition: add to `rust-server/data/entities/npcs/villagers.toml`

```toml
# ============================================================================
# Researcher Orin - Obelisk Quest Giver (Near Southern Obelisk)
# ============================================================================
[researcher_orin]
display_name = "Researcher Orin"
sprite = "jackson"
animation_type = "humanoid"
description = "A scholarly researcher fascinated by the ancient obelisks and their magical properties."

[researcher_orin.stats]
max_hp = 200
damage = 0
attack_range = 0
aggro_range = 0
chase_range = 0
move_cooldown_ms = 0
attack_cooldown_ms = 0
respawn_time_ms = 0

[researcher_orin.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[researcher_orin.behaviors]
hostile = false
quest_giver = true
wander_enabled = false

[researcher_orin.quest_giver]
available_quests = ["obelisk_connection"]

[researcher_orin.speech]
radius = 5
interval_min_ms = 20000
interval_max_ms = 40000
messages = [
    "Fascinating... the resonance is undeniable...",
    "These stones hold secrets older than any kingdom.",
    "I've read about paired waystones in the old texts...",
    "If only I could find the other one...",
    "The magic here is ancient, but still alive.",
]

[researcher_orin.dialogue]
greeting = "Ah, hello! Are you interested in ancient magic? This obelisk is quite remarkable."
quest_available = "I could use someone brave enough to journey north for me."
quest_complete = "The waystones are connected once more, thanks to you!"
```

### 5. NPC Spawn

Add Researcher Orin to the entity spawns for the chunk containing (89, 34). The spawn entry:

```rust
EntitySpawn {
    entity_id: "researcher_orin",
    world_x: 89,
    world_y: 34,
    level: None,
    respawn: true,
    respawn_time_override: None,
    facing: Some("left"),  // facing the obelisk
    unique_id: Some("researcher_orin"),
}
```

*(Exact mechanism depends on how overworld chunk spawns are configured - may be in chunk JSON data or code.)*

---

## New Systems

### 1. Persistent Ground Item Spawns

**New file: `rust-server/data/ground_spawns.toml`**

```toml
# Persistent ground item spawns
# Items that always exist at a location and respawn after being picked up

[[spawns]]
id = "obelisk_shovel"
item_id = "shovel"
x = 93.0
y = -162.0
quantity = 1
respawn_seconds = 30
```

**Server-side implementation:**
- New module or extension to `rust-server/src/item.rs`
- Load spawns from TOML on startup
- Track state per spawn: `Available` or `PickedUp { at: Instant }`
- On game tick, check picked-up spawns against respawn timer → re-create `GroundItem`
- Persistent spawns skip the normal 60-second despawn, have no owner
- When a persistent ground item is picked up, mark the spawn as `PickedUp` instead of just removing

### 2. Shovel Item & Dig UseEffect

**New item in `rust-server/data/items/tools.toml`** (new file):

```toml
[shovel]
display_name = "Old Shovel"
sprite = "shovel"
description = "A worn but sturdy shovel. Useful for digging into soft ground."
category = "quest"
max_stack = 1
base_price = 5
sellable = false
```

Item use_effect in the TOML (exact format may need to match current parsing):
```toml
[shovel.use_effect]
type = "dig"
```

**New UseEffect variant:**

```rust
// Add to UseEffect enum in item_def.rs
Dig,  // no fields - behavior is location-based
```

**Dig UseEffect behavior (server-side):**
1. Player uses item with `Dig` effect
2. Server looks up dig sites from config
3. Finds any dig site within `radius` of player's current position
4. Checks quest requirements (player on correct quest + objective stage)
5. If match: spawn entity at dig site coords, send message/animation
6. If no match: send chat message *"There's nothing to dig here."*
7. **Shovel is NOT consumed** - skip quantity decrement for `Dig` effect

### 3. Dig Sites

**New file: `rust-server/data/dig_sites.toml`**

```toml
# Dig sites - locations where using a shovel triggers quest events

[[sites]]
id = "obelisk_blockage"
x = 94
y = -160
radius = 1
quest_id = "obelisk_connection"
quest_objective_index = 1  # kill_hedgehog objective (0-indexed)
spawn_entity = "hedgehog"
spawn_level = 6
one_time = true  # only triggers once per player
```

**Server-side implementation:**
- Load dig sites from TOML on startup
- When `Dig` use effect is triggered, iterate sites and check:
  - Player distance <= radius
  - Player has quest active at the required objective
  - Site hasn't already been triggered for this player
- On trigger: spawn NPC at (x, y), mark site as triggered for player
- Spawned NPC should be tagged so its death advances the quest objective

### 4. Obelisk Interaction (Second Obelisk)

The second obelisk needs to be interactable as a world object. Two interaction points during the quest:

**First interaction (before digging):**
- Player clicks the obelisk at (92, -163)
- Server checks quest state → player has reached the location but hasn't killed the hedgehog
- Show dialogue: *"The stone hums faintly... something buried beneath is disrupting the flow of energy. You'll need to dig it out. Perhaps there's a tool nearby..."*

**Second interaction (after killing hedgehog):**
- Player clicks the obelisk again
- Server checks quest state → hedgehog killed
- Show dialogue: *"The stone pulses with renewed energy. You feel the connection snap into place, reaching far to the south. The waystone is restored!"*
- Quest advances to "return to Orin" objective

**Implementation options:**
- Add `ClientMessage::InteractObject { object_id }` or reuse existing interaction with object coordinates
- Server maps obelisk coordinates to interaction handlers
- Could use Lua scripting for the obelisk dialogue too

### 5. Waystone Fast-Travel System

**New file: `rust-server/data/waystones.toml`**

```toml
# Waystone teleport network
# Pairs of linked locations unlocked by quest completion

[[waystones]]
id = "south_obelisk"
name = "Southern Obelisk"
x = 88
y = 34
linked_to = "north_obelisk"
quest_required = "obelisk_connection"

[[waystones]]
id = "north_obelisk"
name = "Northern Obelisk"
x = 92
y = -163
linked_to = "south_obelisk"
quest_required = "obelisk_connection"
```

**Interaction (after quest completion):**
- **Click obelisk** → Dialogue: *"The waystone hums with energy. Travel to the [Northern/Southern] Obelisk?"* with Yes/No choices
- **Right-click obelisk** → Context menu: "Teleport" → instant travel, no dialogue
- Both require `obelisk_connection` quest completed

**Server-side:**
- Load waystones on startup as `HashMap<String, Waystone>`
- On obelisk interaction, check if player has completed `quest_required`
- If unlocked: teleport player to linked waystone coordinates (reuse existing teleport logic)
- If locked: show lore text or quest-appropriate dialogue

---

## Modified Existing Code

| File | Change |
|------|--------|
| `rust-server/src/data/item_def.rs` | Add `Dig` variant to `UseEffect` enum |
| `rust-server/src/item.rs` | Handle `Dig` effect in use_item (no consume, check dig sites) |
| `rust-server/src/item.rs` | Persistent ground spawn loading + respawn tick logic |
| `rust-server/src/game.rs` | Tick integration for persistent spawns, dig site mob spawning |
| `rust-server/src/protocol.rs` | Add `InteractObject` client message (if needed for obelisk clicks) |
| `client/src/render/ui/context_menu.rs` | Add "Teleport" option for waystone objects |
| `client/src/input/handler.rs` | Handle obelisk click → send interact message |
| `rust-server/data/quest_locations.toml` | Add `north_obelisk` location |
| `rust-server/data/entities/npcs/villagers.toml` | Add Researcher Orin NPC definition |

---

## New Assets Needed

| Asset | Status |
|-------|--------|
| `jackson.png` (Researcher Orin sprite) | Already exists at `client/assets/sprites/enemies/jackson.png` |
| `shovel.png` (inventory icon) | **Needed** - item sprite for inventory display |
| Obelisk map object (GID 798+x) | Already placed on map at both locations |
