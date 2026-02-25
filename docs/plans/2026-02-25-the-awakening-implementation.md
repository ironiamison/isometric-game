# The Awakening - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement the 13-quest "Awakening" storyline spanning New Aeven (city) and the Desert (ancient ruins), including all monsters, NPCs, items, quest data, Lua scripts, quest locations, dig sites, and instance stubs.

**Architecture:** This is primarily a data-authoring task. The game's existing systems (quest TOML+Lua, entity prototypes, items, dig sites, instances, quest locations) support everything needed. No Rust code changes required for the core implementation. Special combat mechanics (mana drain, magic-healed enemies, boss phases) are noted as future stretch goals.

**Tech Stack:** TOML (entities, items, quests, locations, dig sites), Lua (quest scripts), JSON (instance maps)

**Reference:** See `docs/plans/2026-02-24-the-awakening-questline-design.md` for the full narrative design.

---

## Task 1: New Item Definitions

Items must be created first because monster loot tables reference item IDs.

**Files:**
- Modify: `rust-server/data/items/materials.toml`
- Modify: `rust-server/data/items/tools.toml`
- Modify: `rust-server/data/items/equipment.toml`

**Step 1: Add quest materials to `materials.toml`**

Append to end of `rust-server/data/items/materials.toml`:

```toml
# === The Awakening quest materials ===

[dampening_crystal]
display_name = "Dampening Crystal"
sprite = "dampening_crystal"
description = "A crystal that absorbs chaotic magical energy. Used to stabilize areas of magical disturbance."
category = "material"
max_stack = 99
base_price = 15

[refined_quartz]
display_name = "Refined Quartz"
sprite = "refined_quartz"
description = "Purified quartz with unusual refractive properties. A key component in magical optics."
category = "material"
max_stack = 99
base_price = 25

[construct_core]
display_name = "Construct Core"
sprite = "construct_core"
description = "The magical core that once animated a construct. Still hums with residual energy."
category = "material"
max_stack = 99
base_price = 40

[aetheri_runestone]
display_name = "Aetheri Runestone"
sprite = "aetheri_runestone"
description = "An ancient stone inscribed with Aetheri sealing magic. Radiates a faint warmth."
category = "quest"
max_stack = 3
base_price = 0
sellable = false

[hollow_kings_fragment]
display_name = "Hollow King's Fragment"
sprite = "hollow_kings_fragment"
description = "A shard of crystallized shadow from the Hollow King. Pulses with dark energy."
category = "material"
max_stack = 1
base_price = 500
```

**Step 2: Add quest items to `tools.toml`**

Append to end of `rust-server/data/items/tools.toml`:

```toml
# === The Awakening quest items ===

[resonance_lens]
display_name = "Resonance Lens"
sprite = "resonance_lens"
description = "A magical lens crafted from refined quartz and construct cores. Reveals hidden Aetheri inscriptions."
category = "quest"
max_stack = 1
base_price = 0
sellable = false

[aetheri_key]
display_name = "Aetheri Key"
sprite = "aetheri_key"
description = "An ornate key of Aetheri design. The metal is warm to the touch and covered in tiny glowing runes."
category = "quest"
max_stack = 1
base_price = 0
sellable = false
```

**Step 3: Add Aetheri equipment to `equipment.toml`**

Append to end of `rust-server/data/items/equipment.toml`:

```toml
# === The Awakening rewards ===

[aetheri_ward]
display_name = "Aetheri Ward"
sprite = "aetheri_ward"
description = "A shield forged from Aetheri metal. It hums softly, dampening nearby magical energy."
category = "equipment"
max_stack = 1
base_price = 800

[aetheri_ward.equipment]
slot_type = "body"
defence_level_required = 30
defence_bonus = 18
attack_bonus = -2

[aetheri_blade]
display_name = "Aetheri Blade"
sprite = "aetheri_blade"
description = "A sword of Aetheri make, its edge shimmers with anti-magical energy. Devastating against magical beings."
category = "equipment"
max_stack = 1
base_price = 1200

[aetheri_blade.equipment]
slot_type = "weapon"
attack_level_required = 30
attack_bonus = 16
strength_bonus = 14
```

**Step 4: Verify server loads items**

Run: `cd rust-server && cargo check 2>&1 | head -5`
Expected: Compiles without item-loading errors (items are loaded dynamically from TOML, no code changes needed)

**Step 5: Commit**

```bash
git add rust-server/data/items/materials.toml rust-server/data/items/tools.toml rust-server/data/items/equipment.toml
git commit -m "feat: add Awakening questline item definitions"
```

---

## Task 2: New Monster Entity Definitions

Monsters must exist before quests reference them. All go in a new file for organizational clarity.

**Files:**
- Create: `rust-server/data/entities/monsters/awakening.toml`

**Step 1: Create the monster definitions file**

Write `rust-server/data/entities/monsters/awakening.toml`:

```toml
# ============================================================
# The Awakening Questline - Monster Definitions
# Level range: 22-40, spanning New Aeven and Desert zones
# ============================================================

# --- New Aeven monsters ---

[animated_construct]
display_name = "Animated Construct"
sprite = "animated_construct"
animation_type = "standard"
description = "Enchanted armor brought to life by chaotic magic. Clanks menacingly as it lurches forward."

[animated_construct.stats]
level = 23
max_hp = 45
damage = 6
attack_bonus = 5
defence_bonus = 10
aggro_range = 4
chase_range = 6
move_cooldown_ms = 700
attack_cooldown_ms = 1800
respawn_time_ms = 15000

[animated_construct.rewards]
exp_base = 30
gold_min = 8
gold_max = 20

[[animated_construct.loot]]
item_id = "dampening_crystal"
drop_chance = 0.65
quantity_min = 1
quantity_max = 1

[[animated_construct.loot]]
item_id = "construct_core"
drop_chance = 0.12
quantity_min = 1
quantity_max = 1

[animated_construct.behaviors]
hostile = true
wander_enabled = true
wander_radius = 3

# ---

[seal_wraith]
display_name = "Seal Wraith"
sprite = "seal_wraith"
animation_type = "standard"
description = "A spectral entity drawn to the broken magic seeping from the ancient seal."

[seal_wraith.stats]
level = 25
max_hp = 40
damage = 7
attack_bonus = 12
defence_bonus = 3
aggro_range = 5
chase_range = 7
move_cooldown_ms = 500
attack_cooldown_ms = 1600
respawn_time_ms = 20000

[seal_wraith.rewards]
exp_base = 35
gold_min = 10
gold_max = 22

[[seal_wraith.loot]]
item_id = "regular_bones"
drop_chance = 1.0

[seal_wraith.behaviors]
hostile = true
wander_enabled = true
wander_radius = 4

# ---

[sand_wraith]
display_name = "Sand Wraith"
sprite = "sand_wraith"
animation_type = "standard"
description = "A creature of swirling sand and dark energy. Should not exist this far from the desert."

[sand_wraith.stats]
level = 27
max_hp = 55
damage = 8
attack_bonus = 10
defence_bonus = 6
aggro_range = 5
chase_range = 8
move_cooldown_ms = 500
attack_cooldown_ms = 1500
respawn_time_ms = 20000

[sand_wraith.rewards]
exp_base = 40
gold_min = 12
gold_max = 28

[[sand_wraith.loot]]
item_id = "regular_bones"
drop_chance = 1.0

[sand_wraith.behaviors]
hostile = true
wander_enabled = true
wander_radius = 5

# --- Desert monsters ---

[desert_scorpion]
display_name = "Desert Scorpion"
sprite = "desert_scorpion"
animation_type = "standard"
description = "A venomous scorpion agitated by the tremors beneath the sand."

[desert_scorpion.stats]
level = 27
max_hp = 50
damage = 8
attack_bonus = 8
defence_bonus = 12
aggro_range = 4
chase_range = 6
move_cooldown_ms = 600
attack_cooldown_ms = 1600
respawn_time_ms = 12000

[desert_scorpion.rewards]
exp_base = 38
gold_min = 10
gold_max = 25

[[desert_scorpion.loot]]
item_id = "regular_bones"
drop_chance = 1.0

[desert_scorpion.behaviors]
hostile = true
wander_enabled = true
wander_radius = 4

# ---

[sand_viper]
display_name = "Sand Viper"
sprite = "sand_viper"
animation_type = "standard"
description = "A fast, venomous snake that burrows in sand near ancient ruins."

[sand_viper.stats]
level = 29
max_hp = 38
damage = 10
attack_bonus = 14
defence_bonus = 4
aggro_range = 3
chase_range = 5
move_cooldown_ms = 400
attack_cooldown_ms = 1200
respawn_time_ms = 12000

[sand_viper.rewards]
exp_base = 42
gold_min = 12
gold_max = 28

[[sand_viper.loot]]
item_id = "regular_bones"
drop_chance = 1.0

[sand_viper.behaviors]
hostile = true
wander_enabled = true
wander_radius = 3

# ---

[sand_golem]
display_name = "Sand Golem"
sprite = "sand_golem"
animation_type = "standard"
description = "An ancient Aetheri guardian made of enchanted sandstone. Slow but devastatingly strong."

[sand_golem.stats]
level = 31
max_hp = 90
damage = 12
attack_bonus = 6
defence_bonus = 22
aggro_range = 4
chase_range = 6
move_cooldown_ms = 800
attack_cooldown_ms = 2200
respawn_time_ms = 25000

[sand_golem.rewards]
exp_base = 55
gold_min = 18
gold_max = 40

[[sand_golem.loot]]
item_id = "regular_bones"
drop_chance = 1.0

[sand_golem.behaviors]
hostile = true
wander_enabled = true
wander_radius = 3

# ---

[hollow_sentinel]
display_name = "Hollow Sentinel"
sprite = "hollow_sentinel"
animation_type = "standard"
description = "An echo of the Hollow King's power. Dark energy seeps from its form, draining the magic of those nearby."

[hollow_sentinel.stats]
level = 34
max_hp = 100
damage = 14
attack_bonus = 15
defence_bonus = 12
aggro_range = 5
chase_range = 7
move_cooldown_ms = 550
attack_cooldown_ms = 1800
respawn_time_ms = 0

[hollow_sentinel.rewards]
exp_base = 65
gold_min = 20
gold_max = 45

[[hollow_sentinel.loot]]
item_id = "aetheri_runestone"
drop_chance = 1.0
quantity_min = 1
quantity_max = 1

[hollow_sentinel.behaviors]
hostile = true

# ---

[hollow_shade]
display_name = "Hollow Shade"
sprite = "hollow_shade"
animation_type = "standard"
description = "A shadowy humanoid that feeds on magical energy. Spells seem to strengthen it."

[hollow_shade.stats]
level = 35
max_hp = 70
damage = 13
attack_bonus = 16
defence_bonus = 10
aggro_range = 5
chase_range = 8
move_cooldown_ms = 500
attack_cooldown_ms = 1500
respawn_time_ms = 25000

[hollow_shade.rewards]
exp_base = 60
gold_min = 18
gold_max = 42

[[hollow_shade.loot]]
item_id = "regular_bones"
drop_chance = 1.0

[hollow_shade.behaviors]
hostile = true
wander_enabled = true
wander_radius = 4

# ---

[hollow_devourer]
display_name = "Hollow Devourer"
sprite = "hollow_devourer"
animation_type = "standard"
description = "A large, twisted creature born from the Hollow King's spreading influence. Ravenously consumes all magic in its path."

[hollow_devourer.stats]
level = 37
max_hp = 95
damage = 15
attack_bonus = 14
defence_bonus = 14
aggro_range = 6
chase_range = 8
move_cooldown_ms = 550
attack_cooldown_ms = 1600
respawn_time_ms = 20000

[hollow_devourer.rewards]
exp_base = 72
gold_min = 22
gold_max = 50

[[hollow_devourer.loot]]
item_id = "regular_bones"
drop_chance = 1.0

[hollow_devourer.behaviors]
hostile = true
wander_enabled = true
wander_radius = 5

# ---

[hollow_fragment]
display_name = "Hollow Fragment"
sprite = "hollow_fragment"
animation_type = "standard"
description = "A small shard of the Hollow King's essence. It seeks to rejoin its master."

[hollow_fragment.stats]
level = 35
max_hp = 25
damage = 8
attack_bonus = 10
defence_bonus = 2
aggro_range = 8
chase_range = 10
move_cooldown_ms = 400
attack_cooldown_ms = 1200
respawn_time_ms = 0

[hollow_fragment.rewards]
exp_base = 20
gold_min = 0
gold_max = 0

[hollow_fragment.behaviors]
hostile = true

# --- Boss ---

[hollow_king]
display_name = "The Hollow King"
sprite = "hollow_king"
animation_type = "standard"
description = "An ancient entity that consumes magic itself. Sealed away by the Aetheri centuries ago, now partially freed."

[hollow_king.stats]
level = 40
max_hp = 500
damage = 20
attack_bonus = 22
defence_bonus = 20
aggro_range = 8
chase_range = 12
move_cooldown_ms = 600
attack_cooldown_ms = 2000
respawn_time_ms = 0

[hollow_king.rewards]
exp_base = 500
gold_min = 200
gold_max = 500

[[hollow_king.loot]]
item_id = "hollow_kings_fragment"
drop_chance = 1.0
quantity_min = 1
quantity_max = 1

[[hollow_king.loot]]
item_id = "aetheri_blade"
drop_chance = 0.5
quantity_min = 1
quantity_max = 1

[[hollow_king.loot]]
item_id = "aetheri_ward"
drop_chance = 0.5
quantity_min = 1
quantity_max = 1

[hollow_king.behaviors]
hostile = true
```

**Step 2: Verify server loads entities**

Run: `cd rust-server && cargo check 2>&1 | head -5`
Expected: No errors

**Step 3: Commit**

```bash
git add rust-server/data/entities/monsters/awakening.toml
git commit -m "feat: add Awakening questline monster definitions (10 new entities)"
```

---

## Task 3: New NPC Entity Definitions

Quest-giver and story NPCs.

**Files:**
- Create: `rust-server/data/entities/npcs/awakening.toml`

**Step 1: Create the NPC definitions file**

Write `rust-server/data/entities/npcs/awakening.toml`:

```toml
# ============================================================
# The Awakening Questline - NPC Definitions
# ============================================================

# --- New Aeven NPCs ---

[guard_captain]
display_name = "Guard Captain Aldric"
sprite = "guard_captain"
animation_type = "humanoid"
description = "The captain of New Aeven's city guard. A practical man overwhelmed by the magical chaos gripping his city."

[guard_captain.stats]
level = 1
max_hp = 100
damage = 0
attack_bonus = 0
defence_bonus = 0
attack_range = 0
aggro_range = 0
chase_range = 0
respawn_time_ms = 0

[guard_captain.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[guard_captain.behaviors]
quest_giver = true
friendly = true
facing = "down"

[guard_captain.quest_giver]
available_quests = ["city_of_sparks"]

[guard_captain.dialogue]
greeting = "Another explosion in the market district... I don't have enough guards for this."

[guard_captain.speech]
radius = 5
interval_min_ms = 30000
interval_max_ms = 60000
messages = [
    "Stay alert. The enchantments are unstable today.",
    "If you see any animated objects, keep your distance.",
    "We've had three incidents this morning alone...",
]

# ---

[archmage_yenara]
display_name = "Archmage Yenara"
sprite = "archmage_yenara"
animation_type = "humanoid"
description = "Head of New Aeven's mage college. Scholarly and determined, she's racing to understand the magical disturbances plaguing the city."

[archmage_yenara.stats]
level = 1
max_hp = 100
damage = 0
attack_bonus = 0
defence_bonus = 0
attack_range = 0
aggro_range = 0
chase_range = 0
respawn_time_ms = 0

[archmage_yenara.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[archmage_yenara.behaviors]
quest_giver = true
friendly = true
facing = "down"

[archmage_yenara.quest_giver]
available_quests = ["containment_protocol", "the_old_foundation", "words_of_the_sealed", "tremors", "the_desert_beckons"]

[archmage_yenara.dialogue]
greeting = "The magical resonance is getting worse. I fear this is only the beginning."

[archmage_yenara.speech]
radius = 5
interval_min_ms = 30000
interval_max_ms = 60000
messages = [
    "These readings are unlike anything in our archives...",
    "The ley lines beneath the city are overloaded.",
    "If we don't find the source soon, the entire ward system could collapse.",
]

# --- Desert NPCs ---

[kael]
display_name = "Kael"
sprite = "kael"
animation_type = "humanoid"
description = "An Aetheri descendant living as a nomadic desert guide. He carries a staff covered in old symbols and the burden of his ancestors' sacrifice."

[kael.stats]
level = 1
max_hp = 100
damage = 0
attack_bonus = 0
defence_bonus = 0
attack_range = 0
aggro_range = 0
chase_range = 0
respawn_time_ms = 0

[kael.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[kael.behaviors]
quest_giver = true
merchant = true
friendly = true
facing = "down"

[kael.quest_giver]
available_quests = ["shifting_sands", "echoes_of_aether", "restoration", "the_second_seal", "the_last_seal", "the_hollow_king"]

[kael.merchant]
shop_id = "kael_desert_supplies"
buy_multiplier = 0.5
sell_multiplier = 1.0
required_quest = "the_hollow_king"

[kael.dialogue]
greeting = "The sands remember what the world has forgotten."
shop_open = "Take what you need. The desert shows no mercy to the unprepared."

[kael.speech]
radius = 5
interval_min_ms = 30000
interval_max_ms = 60000
messages = [
    "My ancestors gave everything to seal what lies below.",
    "The tremors grow stronger. We have little time.",
    "I am the last of the Aetheri. This burden is mine to bear.",
]

# ---

[serah]
display_name = "Serah"
sprite = "serah"
animation_type = "humanoid"
description = "A treasure hunter who came to the desert looking for Aetheri riches. What she found was far more dangerous."

[serah.stats]
level = 1
max_hp = 100
damage = 0
attack_bonus = 0
defence_bonus = 0
attack_range = 0
aggro_range = 0
chase_range = 0
respawn_time_ms = 0

[serah.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[serah.behaviors]
quest_giver = true
friendly = true
facing = "left"

[serah.quest_giver]
available_quests = ["the_tomb_raider"]

[serah.dialogue]
greeting = "You look like someone who can handle themselves. Good - I could use the help."

[serah.speech]
radius = 5
interval_min_ms = 25000
interval_max_ms = 50000
messages = [
    "There's got to be treasure in these ruins somewhere...",
    "I've mapped three dig sites so far. All picked clean.",
    "The deeper ruins give me the creeps, if I'm honest.",
]
```

**Step 2: Verify**

Run: `cd rust-server && cargo check 2>&1 | head -5`

**Step 3: Commit**

```bash
git add rust-server/data/entities/npcs/awakening.toml
git commit -m "feat: add Awakening questline NPC definitions (4 new NPCs)"
```

---

## Task 4: Quest Location Triggers

Define the coordinate-based trigger zones for ReachLocation objectives. Coordinates are placeholders (marked with `# PLACEHOLDER`) that should be updated when zones are built in the mapper.

**Files:**
- Modify: `rust-server/data/quest_locations.toml`

**Step 1: Add all Awakening quest locations**

Append to end of `rust-server/data/quest_locations.toml`:

```toml
# === The Awakening - New Aeven locations ===

[na_market_disturbance]
x = 200
y = 50
radius = 3

[na_college_disturbance]
x = 210
y = 40
radius = 3

[na_gate_disturbance]
x = 190
y = 60
radius = 3

[na_cistern_seal]
x = 8
y = 12
radius = 2

[na_wall_breach]
x = 195
y = 65
radius = 3

# === The Awakening - Desert locations ===

[desert_stone_pillars]
x = 250
y = 100
radius = 3

[desert_buried_archway]
x = 270
y = 110
radius = 3

[desert_sunken_colossus]
x = 260
y = 125
radius = 3

[desert_aetheri_obelisk]
x = 280
y = 105
radius = 2

[desert_inner_chamber]
x = 8
y = 5
radius = 2

[desert_eastern_dig]
x = 290
y = 95
radius = 2

[desert_western_dig]
x = 240
y = 115
radius = 2

[desert_southern_dig]
x = 265
y = 140
radius = 2

[desert_temple_entrance]
x = 8
y = 14
radius = 2

[desert_seal_chamber]
x = 8
y = 3
radius = 2

[desert_sanctum_entrance]
x = 300
y = 130
radius = 2

[sanctum_obelisk_1]
x = 5
y = 8
radius = 1

[sanctum_obelisk_2]
x = 10
y = 4
radius = 1

[sanctum_obelisk_3]
x = 15
y = 8
radius = 1
```

**Step 2: Commit**

```bash
git add rust-server/data/quest_locations.toml
git commit -m "feat: add Awakening questline location triggers"
```

---

## Task 5: Dig Site Definitions

Quest 10 ("Restoration") uses three dig sites that each spawn a Hollow Sentinel. Coordinates are placeholders matching the quest location triggers.

**Files:**
- Modify: `rust-server/data/dig_sites.toml`

**Step 1: Add three dig sites**

Append to end of `rust-server/data/dig_sites.toml`:

```toml
# === The Awakening - Restoration quest dig sites ===

[[sites]]
id = "aetheri_eastern_dig"
x = 290
y = 95
radius = 2
quest_id = "restoration"
quest_objective_id = "dig_eastern"
spawn_entity = "hollow_sentinel"
spawn_level = 34

[[sites]]
id = "aetheri_western_dig"
x = 240
y = 115
radius = 2
quest_id = "restoration"
quest_objective_id = "dig_western"
spawn_entity = "hollow_sentinel"
spawn_level = 34

[[sites]]
id = "aetheri_southern_dig"
x = 265
y = 140
radius = 2
quest_id = "restoration"
quest_objective_id = "dig_southern"
spawn_entity = "hollow_sentinel"
spawn_level = 35
```

**Step 2: Commit**

```bash
git add rust-server/data/dig_sites.toml
git commit -m "feat: add Awakening dig sites for Restoration quest"
```

---

## Task 6: Quest TOML Files - New Aeven Chain (Quests 1-6)

**Files:**
- Create: `rust-server/data/quests/awakening/city_of_sparks.toml`
- Create: `rust-server/data/quests/awakening/containment_protocol.toml`
- Create: `rust-server/data/quests/awakening/the_old_foundation.toml`
- Create: `rust-server/data/quests/awakening/words_of_the_sealed.toml`
- Create: `rust-server/data/quests/awakening/tremors.toml`
- Create: `rust-server/data/quests/awakening/the_desert_beckons.toml`

**Step 1: Create quest directory**

```bash
mkdir -p rust-server/data/quests/awakening
```

**Step 2: Write Quest 1 - City of Sparks**

Write `rust-server/data/quests/awakening/city_of_sparks.toml`:

```toml
[quest]
id = "city_of_sparks"
name = "City of Sparks"
description = "Investigate three sites of magical disturbance throughout New Aeven and report your findings to the Archmage."
giver_npc = "guard_captain"
level_required = 20
repeatable = false
lua_script = "awakening/city_of_sparks.lua"

[quest.chain]
next = "containment_protocol"

[[quest.objectives]]
id = "investigate_market"
type = "reach_location"
target = "na_market_disturbance"
count = 1
description = "Investigate the market district disturbance"
dialogue = "The cobblestones hum beneath your feet. You can feel a deep resonance pulsing from somewhere underground."

[[quest.objectives]]
id = "investigate_college"
type = "reach_location"
target = "na_college_disturbance"
count = 1
description = "Investigate the mage college courtyard disturbance"
dialogue = "The air crackles with stray sparks of magical energy. The resonance is even stronger here."

[[quest.objectives]]
id = "investigate_gate"
type = "reach_location"
target = "na_gate_disturbance"
count = 1
description = "Investigate the city gate disturbance"
dialogue = "A low rumble echoes from deep below. Whatever is causing this, it's beneath the entire city."

[[quest.objectives]]
id = "report_to_yenara"
type = "talk_to"
target = "archmage_yenara"
description = "Report your findings to Archmage Yenara"
sequential = true

[quest.rewards]
exp = 200
gold = 150

[quest.dialogue]
offer = "Adventurer! We've got a crisis on our hands. Magic is going haywire all over the city - enchanted lanterns exploding, objects coming to life. I need someone to investigate three disturbance sites and figure out what's causing this."
accept = "Check the market district, the mage college courtyard, and the city gate. Report anything unusual to Archmage Yenara at the college. She'll know what to make of it."
progress = "Have you investigated all three sites? Archmage Yenara is waiting for your report."
complete = "Good work. Whatever you found, Archmage Yenara will want to hear about it immediately."
```

**Step 3: Write Quest 2 - Containment Protocol**

Write `rust-server/data/quests/awakening/containment_protocol.toml`:

```toml
[quest]
id = "containment_protocol"
name = "Containment Protocol"
description = "Destroy the animated constructs terrorizing the market district and collect their dampening crystals for Archmage Yenara."
giver_npc = "archmage_yenara"
level_required = 22
repeatable = false
lua_script = "awakening/containment_protocol.lua"

[quest.chain]
previous = "city_of_sparks"
next = "the_old_foundation"

[[quest.objectives]]
id = "kill_constructs"
type = "kill_monster"
target = "animated_construct"
count = 12
description = "Destroy 12 Animated Constructs"

[[quest.objectives]]
id = "collect_crystals"
type = "collect_item"
target = "dampening_crystal"
count = 8
description = "Collect 8 Dampening Crystals"

[quest.rewards]
exp = 350
gold = 250

[quest.dialogue]
offer = "The magical disturbances have animated objects throughout the market - suits of armor, crates, even brooms are attacking people. I need you to put them down and bring me the dampening crystals they contain. I can use them to stabilize the area."
accept = "Destroy as many animated constructs as you can and collect their dampening crystals. Be careful - they're stronger than they look."
progress = "How goes the containment? I need those dampening crystals to stabilize the ward matrix."
complete = "Excellent work. These crystals will help, but they're only a stopgap. The source of these disturbances is something far deeper."
```

**Step 4: Write Quest 3 - The Old Foundation**

Write `rust-server/data/quests/awakening/the_old_foundation.toml`:

```toml
[quest]
id = "the_old_foundation"
name = "The Old Foundation"
description = "Explore the ancient cisterns beneath New Aeven, fight through the Seal Wraiths, and find the source of the disturbances."
giver_npc = "archmage_yenara"
level_required = 24
repeatable = false
lua_script = "awakening/the_old_foundation.lua"

[quest.chain]
previous = "containment_protocol"
next = "words_of_the_sealed"

[[quest.objectives]]
id = "kill_seal_wraiths"
type = "kill_monster"
target = "seal_wraith"
count = 8
description = "Destroy 8 Seal Wraiths"

[[quest.objectives]]
id = "find_seal"
type = "reach_location"
target = "na_cistern_seal"
count = 1
description = "Find the source of the disturbance"
sequential = true
dialogue = "Before you stands a massive stone seal, cracked and pulsing with energy. Ancient symbols cover its surface - a language you've never seen. This is the source."

[[quest.objectives]]
id = "report_to_yenara_cisterns"
type = "talk_to"
target = "archmage_yenara"
description = "Report the discovery to Archmage Yenara"
sequential = true

[quest.rewards]
exp = 500
gold = 350

[quest.dialogue]
offer = "My research suggests New Aeven was built on top of something ancient. Beneath the city, past the water cisterns, there should be older stonework. I need you to go down there and find what's causing these disturbances."
accept = "Enter the cisterns beneath the city. Fight through whatever's lurking down there and find the source. Be careful - the magical energy is concentrated underground."
progress = "What did you find in the cisterns? Was there something down there?"
complete = "A sealed wall with unknown symbols? And wraiths drawn to it? This is far more serious than I feared. Those symbols... I need to study them."
```

**Step 5: Write Quest 4 - Words of the Sealed**

Write `rust-server/data/quests/awakening/words_of_the_sealed.toml`:

```toml
[quest]
id = "words_of_the_sealed"
name = "Words of the Sealed"
description = "Gather materials for a Resonance Lens so Archmage Yenara can decipher the ancient inscriptions."
giver_npc = "archmage_yenara"
level_required = 25
repeatable = false
lua_script = "awakening/words_of_the_sealed.lua"

[quest.chain]
previous = "the_old_foundation"
next = "tremors"

[[quest.objectives]]
id = "collect_quartz"
type = "collect_item"
target = "refined_quartz"
count = 5
description = "Collect 5 Refined Quartz"

[[quest.objectives]]
id = "collect_cores"
type = "collect_item"
target = "construct_core"
count = 3
description = "Collect 3 Construct Cores"

[[quest.objectives]]
id = "return_to_yenara"
type = "talk_to"
target = "archmage_yenara"
description = "Bring the materials to Archmage Yenara"
sequential = true

[quest.rewards]
exp = 400
gold = 300
items = [
    { id = "resonance_lens", count = 1 },
]

[quest.dialogue]
offer = "I've partially translated the cistern symbols. They're warnings: 'What sleeps beneath the sand must never wake. The seals hold. The seals must hold.' The writing belongs to a lost civilization called the Aetheri. I need a Resonance Lens to read the rest - but it requires special materials."
accept = "I need 5 Refined Quartz - you can find it while mining - and 3 Construct Cores from those animated constructs. Bring them to me and I'll craft the lens."
progress = "Have you gathered the materials? Every moment we delay, the inscriptions fade further."
complete = "Perfect. Let me assemble the lens... there. The Resonance Lens is complete. Now we can read what the Aetheri were trying to tell us."
```

**Step 6: Write Quest 5 - Tremors**

Write `rust-server/data/quests/awakening/tremors.toml`:

```toml
[quest]
id = "tremors"
name = "Tremors"
description = "Defend New Aeven from Sand Wraiths pouring through a breach in the city wall, then investigate the aftermath."
giver_npc = "archmage_yenara"
level_required = 27
repeatable = false
lua_script = "awakening/tremors.lua"

[quest.chain]
previous = "words_of_the_sealed"
next = "the_desert_beckons"

[[quest.objectives]]
id = "kill_sand_wraiths"
type = "kill_monster"
target = "sand_wraith"
count = 15
description = "Destroy 15 Sand Wraiths at the breach"

[[quest.objectives]]
id = "inspect_breach"
type = "reach_location"
target = "na_wall_breach"
count = 1
description = "Inspect the breach after the battle"
sequential = true
dialogue = "The wraiths came from underground, drawn along a ley line from the desert. The seal in the cistern has cracked further. Time is running out."

[[quest.objectives]]
id = "report_breach"
type = "talk_to"
target = "archmage_yenara"
description = "Report to Archmage Yenara"
sequential = true

[quest.rewards]
exp = 600
gold = 400

[quest.dialogue]
offer = "An earthquake just hit the city! The eastern wall has collapsed and creatures made of sand and shadow are pouring through the breach. We need you on the front line NOW!"
accept = "Get to the eastern wall breach and destroy those Sand Wraiths! Then inspect the damage - I need to know how they got here."
progress = "Is the breach secure? We can't afford to let more of those things into the city."
complete = "A ley line connecting the desert to the seal beneath us... that explains the disturbances. Whatever is sealed down there, its influence stretches far further than I imagined."
```

**Step 7: Write Quest 6 - The Desert Beckons**

Write `rust-server/data/quests/awakening/the_desert_beckons.toml`:

```toml
[quest]
id = "the_desert_beckons"
name = "The Desert Beckons"
description = "Speak with Archmage Yenara about the Resonance Lens findings, then travel to the desert to meet her contact, Kael."
giver_npc = "archmage_yenara"
level_required = 28
repeatable = false
lua_script = "awakening/the_desert_beckons.lua"

[quest.chain]
previous = "tremors"
next = "shifting_sands"

[[quest.objectives]]
id = "meet_kael"
type = "talk_to"
target = "kael"
description = "Meet Kael at the desert's edge"

[quest.rewards]
exp = 300
gold = 200

[quest.dialogue]
offer = "I used the Resonance Lens in the cisterns. The sealed wall contains a map - it shows the location of a primary seal deep in the desert. The Aetheri locked away something called 'The Hollow King.' These disturbances are just aftershocks of its prison weakening. I have a contact in the desert - an Aetheri descendant named Kael. Find him."
accept = "Travel to the desert and find Kael. He lives as a nomadic guide near the desert's edge. Show him my letter and he'll help you. Hurry - the seal won't hold much longer."
progress = "Have you found Kael yet? Time is of the essence."
complete = "You've met Kael? Good. He knows the desert and the Aetheri history better than anyone alive. Trust him. And be careful out there."
```

**Step 8: Commit**

```bash
git add rust-server/data/quests/awakening/
git commit -m "feat: add New Aeven quest chain TOML files (quests 1-6)"
```

---

## Task 7: Quest TOML Files - Desert Chain (Quests 7-13)

**Files:**
- Create: `rust-server/data/quests/awakening/shifting_sands.toml`
- Create: `rust-server/data/quests/awakening/echoes_of_aether.toml`
- Create: `rust-server/data/quests/awakening/the_tomb_raider.toml`
- Create: `rust-server/data/quests/awakening/restoration.toml`
- Create: `rust-server/data/quests/awakening/the_second_seal.toml`
- Create: `rust-server/data/quests/awakening/the_last_seal.toml`
- Create: `rust-server/data/quests/awakening/the_hollow_king.toml`

**Step 1: Write Quest 7 - Shifting Sands**

Write `rust-server/data/quests/awakening/shifting_sands.toml`:

```toml
[quest]
id = "shifting_sands"
name = "Shifting Sands"
description = "Prove you can survive the desert by navigating to three landmarks and dealing with the aggressive wildlife."
giver_npc = "kael"
level_required = 28
repeatable = false
lua_script = "awakening/shifting_sands.lua"

[quest.chain]
previous = "the_desert_beckons"
next = "echoes_of_aether"

[[quest.objectives]]
id = "reach_stone_pillars"
type = "reach_location"
target = "desert_stone_pillars"
count = 1
description = "Navigate to the Stone Pillars"
dialogue = "Massive stone columns rise from the sand, carved with faded Aetheri symbols. The ruins are everywhere."

[[quest.objectives]]
id = "reach_buried_archway"
type = "reach_location"
target = "desert_buried_archway"
count = 1
description = "Navigate to the Buried Archway"
dialogue = "A grand archway, half-swallowed by sand. You can only imagine the building it once belonged to."

[[quest.objectives]]
id = "reach_sunken_colossus"
type = "reach_location"
target = "desert_sunken_colossus"
count = 1
description = "Navigate to the Sunken Colossus"
dialogue = "A colossal stone head protrudes from the dunes, its expression serene despite the centuries of burial. An Aetheri king, perhaps."

[[quest.objectives]]
id = "kill_scorpions"
type = "kill_monster"
target = "desert_scorpion"
count = 10
description = "Kill 10 Desert Scorpions"

[[quest.objectives]]
id = "kill_vipers"
type = "kill_monster"
target = "sand_viper"
count = 5
description = "Kill 5 Sand Vipers"

[quest.rewards]
exp = 500
gold = 350

[quest.dialogue]
offer = "So Yenara sent you. The tremors are real, then. Before I take you deeper, I need to know you can handle the desert. Navigate to three landmarks and deal with whatever attacks you along the way. The creatures are... agitated."
accept = "Find the Stone Pillars, the Buried Archway, and the Sunken Colossus. They mark the boundaries of the Aetheri homeland. And watch for scorpions and vipers - they've been unusually aggressive."
progress = "The desert tests everyone. Have you reached all three landmarks?"
complete = "You survived. Good. The creatures can feel what's happening underground - that's why they're so hostile. Now let me show you something that will explain everything."
```

**Step 2: Write Quest 8 - Echoes of Aether**

Write `rust-server/data/quests/awakening/echoes_of_aether.toml`:

```toml
[quest]
id = "echoes_of_aether"
name = "Echoes of Aether"
description = "Travel to a functioning Aetheri Obelisk and use the Resonance Lens to uncover the truth about the Hollow King."
giver_npc = "kael"
level_required = 30
repeatable = false
lua_script = "awakening/echoes_of_aether.lua"

[quest.chain]
previous = "shifting_sands"
next = "the_tomb_raider"

[[quest.objectives]]
id = "reach_obelisk"
type = "reach_location"
target = "desert_aetheri_obelisk"
count = 1
description = "Travel to the Aetheri Obelisk"
dialogue = "The obelisk stands tall, untouched by the sand. Its surface glows faintly with Aetheri script."

[[quest.objectives]]
id = "talk_kael_vision"
type = "talk_to"
target = "kael"
description = "Discuss the vision with Kael"
sequential = true

[quest.rewards]
exp = 450
gold = 300

[quest.dialogue]
offer = "There's an Aetheri Obelisk nearby that still functions. If you have a Resonance Lens - or even without one, I can help - we can activate it. It will show you what my ancestors saw. What they fought. What they sealed away."
accept = "Follow the path to the obelisk. When you arrive, I'll meet you there. Be ready for what you see - the truth about the Hollow King is not pleasant."
progress = "Have you reached the obelisk? We must see what it shows us."
complete = "Now you understand. The Hollow King consumed every spell thrown at it. My ancestors gave everything - their magic, their civilization, their future - to seal it away. And those seals are failing."
```

**Step 3: Write Quest 9 - The Tomb Raider**

Write `rust-server/data/quests/awakening/the_tomb_raider.toml`:

```toml
[quest]
id = "the_tomb_raider"
name = "The Tomb Raider"
description = "Help the treasure hunter Serah clear Sand Golems from an Aetheri ruin, only to discover something far more important than treasure inside."
giver_npc = "serah"
level_required = 31
repeatable = false
lua_script = "awakening/the_tomb_raider.lua"

[quest.chain]
previous = "echoes_of_aether"
next = "restoration"

[[quest.objectives]]
id = "kill_sand_golems"
type = "kill_monster"
target = "sand_golem"
count = 10
description = "Destroy 10 Sand Golems"

[[quest.objectives]]
id = "reach_inner_chamber"
type = "reach_location"
target = "desert_inner_chamber"
count = 1
description = "Reach the inner chamber"
sequential = true
dialogue = "No gold. No gems. Just another cracked seal, pulsing with dark energy. Serah looks genuinely frightened for the first time."

[[quest.objectives]]
id = "talk_kael_seal"
type = "talk_to"
target = "kael"
description = "Examine the seal with Kael"
sequential = true

[quest.rewards]
exp = 600
gold = 400

[quest.dialogue]
offer = "Hey! You look like you can swing a sword. I found an entrance to an Aetheri chamber down that slope, but it's crawling with these stone... things. Help me clear them out and we split whatever treasure's inside. Deal?"
accept = "Great! The golems are tough - slow, but they hit like a rockslide. Take them down and meet me at the inner chamber. This is going to be a big score, I can feel it!"
progress = "Still fighting those golems? Come on, treasure waits for no one!"
complete = "That's... that's one of the Binding Seals, isn't it? I heard the stories but I never... it's cracking. I can hear it cracking. This isn't treasure hunting anymore."
```

**Step 4: Write Quest 10 - Restoration**

Write `rust-server/data/quests/awakening/restoration.toml`:

```toml
[quest]
id = "restoration"
name = "Restoration"
description = "Gather three Aetheri Runestones from desert dig sites to fuel Kael's sealing ritual. Beware the Hollow Sentinels that guard them."
giver_npc = "kael"
level_required = 33
repeatable = false
lua_script = "awakening/restoration.lua"

[quest.chain]
previous = "the_tomb_raider"
next = "the_second_seal"

[[quest.objectives]]
id = "dig_eastern"
type = "reach_location"
target = "aetheri_eastern_dig_dig"
count = 1
description = "Dig at the Eastern Dig Site"

[[quest.objectives]]
id = "dig_western"
type = "reach_location"
target = "aetheri_western_dig_dig"
count = 1
description = "Dig at the Western Dig Site"

[[quest.objectives]]
id = "dig_southern"
type = "reach_location"
target = "aetheri_southern_dig_dig"
count = 1
description = "Dig at the Southern Dig Site"

[[quest.objectives]]
id = "kill_sentinels"
type = "kill_monster"
target = "hollow_sentinel"
count = 3
description = "Defeat 3 Hollow Sentinels"

[[quest.objectives]]
id = "collect_runestones"
type = "collect_item"
target = "aetheri_runestone"
count = 3
description = "Collect 3 Aetheri Runestones"

[quest.rewards]
exp = 700
gold = 500

[quest.dialogue]
offer = "I know a ritual to reinforce the seals, but it requires Aetheri Runestones - powerful artifacts my ancestors created. Three are buried at dig sites across the desert. But be warned: the Hollow King's power seeps through the weakened seals. Guardians will appear when you dig."
accept = "Take a shovel and dig at the three sites I've marked. When the Hollow Sentinels appear, destroy them and claim the runestones. These creatures drain magical energy on contact - fight them with steel, not spells."
progress = "Have you gathered all three runestones? The ritual requires all of them."
complete = "Three runestones, recovered after centuries beneath the sand. My ancestors would be proud. Now we must reach the second seal and perform the ritual before it's too late."
```

**Step 5: Write Quest 11 - The Second Seal**

Write `rust-server/data/quests/awakening/the_second_seal.toml`:

```toml
[quest]
id = "the_second_seal"
name = "The Second Seal"
description = "Enter the Desert Temple, fight through Hollow Shades, and assist Kael with the sealing ritual at the second Binding Seal."
giver_npc = "kael"
level_required = 35
repeatable = false
lua_script = "awakening/the_second_seal.lua"

[quest.chain]
previous = "restoration"
next = "the_last_seal"

[[quest.objectives]]
id = "kill_hollow_shades"
type = "kill_monster"
target = "hollow_shade"
count = 12
description = "Destroy 12 Hollow Shades"

[[quest.objectives]]
id = "reach_seal_chamber"
type = "reach_location"
target = "desert_seal_chamber"
count = 1
description = "Reach the seal chamber"
sequential = true
dialogue = "The second Binding Seal looms before you, dark energy coursing through its cracks. Kael begins the ritual."

[[quest.objectives]]
id = "assist_kael_ritual"
type = "talk_to"
target = "kael"
description = "Assist Kael with the sealing ritual"
sequential = true

[quest.rewards]
exp = 800
gold = 550

[quest.dialogue]
offer = "The second Binding Seal is inside a temple deeper in the desert. The corruption there is worse - creatures called Hollow Shades patrol its halls. They feed on magic, so spells may not work well against them. Fight with steel."
accept = "Enter the Desert Temple and cut through the Hollow Shades. Reach the seal chamber and I'll perform the ritual with the runestones. Together, we can reinforce the seal."
progress = "Have you cleared the temple? I need a path to the seal chamber."
complete = "The ritual... failed. The seal shattered. And that voice... 'Three seals. Two remain. One is already mine.' The first seal beneath New Aeven has already broken. We were too late."
```

**Step 6: Write Quest 12 - The Last Seal**

Write `rust-server/data/quests/awakening/the_last_seal.toml`:

```toml
[quest]
id = "the_last_seal"
name = "The Last Seal"
description = "Fight through the Hollow Devourers now roaming the desert surface, reach the Aetheri Sanctum, and use Serah's key to enter."
giver_npc = "kael"
level_required = 37
repeatable = false
lua_script = "awakening/the_last_seal.lua"

[quest.chain]
previous = "the_second_seal"
next = "the_hollow_king"

[[quest.objectives]]
id = "kill_devourers"
type = "kill_monster"
target = "hollow_devourer"
count = 10
description = "Destroy 10 Hollow Devourers"

[[quest.objectives]]
id = "reach_sanctum"
type = "reach_location"
target = "desert_sanctum_entrance"
count = 1
description = "Reach the Aetheri Sanctum entrance"
sequential = true
dialogue = "A massive stone door blocks the entrance, covered in Aetheri seals. It requires a key."

[[quest.objectives]]
id = "use_key"
type = "talk_to"
target = "serah"
description = "Use Serah's Aetheri Key to open the Sanctum"
sequential = true

[quest.rewards]
exp = 750
gold = 500

[quest.dialogue]
offer = "One seal remains, deep in the Aetheri Sanctum - the heart of my ancestors' civilization. But the Hollow King's influence is spreading. New creatures roam the surface. The desert itself is changing."
accept = "Fight through the Hollow Devourers and reach the Sanctum entrance. Serah said she would meet us there - she has something that might help us get inside."
progress = "Have you reached the Sanctum? Serah should be waiting at the entrance."
complete = "The Aetheri Key... of course. Serah's 'worthless trinket' was the key to the Sanctum all along. The door is open. What lies within will determine the fate of everything."
```

**Step 7: Write Quest 13 - The Hollow King**

Write `rust-server/data/quests/awakening/the_hollow_king.toml`:

```toml
[quest]
id = "the_hollow_king"
name = "The Hollow King"
description = "Enter the Aetheri Sanctum, activate the three obelisks, and face the Hollow King before the last seal breaks."
giver_npc = "kael"
level_required = 38
repeatable = false
lua_script = "awakening/the_hollow_king.lua"

[quest.chain]
previous = "the_last_seal"

[[quest.objectives]]
id = "kill_hollow_enemies"
type = "kill_monster"
target = "hollow_shade"
count = 10
description = "Fight through the Hollow King's forces"

[[quest.objectives]]
id = "kill_hollow_sentinels_sanctum"
type = "kill_monster"
target = "hollow_sentinel"
count = 5
description = "Destroy the Sanctum's Hollow Sentinels"

[[quest.objectives]]
id = "defeat_hollow_king"
type = "kill_monster"
target = "hollow_king"
count = 1
description = "Defeat the Hollow King"
sequential = true

[[quest.objectives]]
id = "talk_kael_aftermath"
type = "talk_to"
target = "kael"
description = "Speak with Kael"
sequential = true

[quest.rewards]
exp = 1500
gold = 1000
items = [
    { id = "aetheri_ward", count = 1 },
]

[quest.dialogue]
offer = "This is it. The Aetheri Sanctum - the heart of everything my ancestors built. The Hollow King is inside, straining against the last seal. We end this now."
accept = "Fight through whatever guards the Sanctum. The Hollow King feeds on magic - use steel and arrows, not spells. I'll be with you. When the time comes, I'll channel the runestones to weaken it."
progress = "Keep fighting. We must reach the Hollow King before the last seal breaks."
complete = "It's done. The Hollow King is sealed once more. My ancestors' sacrifice was not in vain. I'll stay here and watch over the seal - it's what I was born to do. Thank you, friend. The world owes you a debt it can never repay."
```

**Step 8: Commit**

```bash
git add rust-server/data/quests/awakening/
git commit -m "feat: add Desert quest chain TOML files (quests 7-13)"
```

---

## Task 8: Lua Quest Scripts - New Aeven Chain (Quests 1-6)

**Files:**
- Create: `rust-server/data/scripts/quests/awakening/city_of_sparks.lua`
- Create: `rust-server/data/scripts/quests/awakening/containment_protocol.lua`
- Create: `rust-server/data/scripts/quests/awakening/the_old_foundation.lua`
- Create: `rust-server/data/scripts/quests/awakening/words_of_the_sealed.lua`
- Create: `rust-server/data/scripts/quests/awakening/tremors.lua`
- Create: `rust-server/data/scripts/quests/awakening/the_desert_beckons.lua`

**Step 1: Create scripts directory**

```bash
mkdir -p rust-server/data/scripts/quests/awakening
```

**Step 2: Write Quest 1 script - City of Sparks**

Write `rust-server/data/scripts/quests/awakening/city_of_sparks.lua`:

```lua
-- City of Sparks - Quest 1 of The Awakening
-- Guard Captain Aldric sends the player to investigate disturbance sites

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        ctx:show_dialogue({
            speaker = "Guard Captain Aldric",
            text = "Archmage Yenara speaks highly of you. If you're heading out there again, be careful."
        })
    end
end

function show_offer_dialogue(ctx)
    local choice = ctx:show_dialogue({
        speaker = "Guard Captain Aldric",
        text = "Adventurer! We've got a crisis. Magic is going haywire all over New Aeven - enchanted lanterns exploding, objects coming to life. I need someone to investigate three disturbance sites.",
        choices = {
            { id = "accept", text = "I'll investigate." },
            { id = "ask", text = "What's causing this?" },
            { id = "decline", text = "Not right now." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Guard Captain Aldric",
            text = "Check the market district, the mage college courtyard, and the city gate. Report anything you find to Archmage Yenara at the college."
        })
    elseif choice == "ask" then
        ctx:show_dialogue({
            speaker = "Guard Captain Aldric",
            text = "If I knew that, I wouldn't need help. The Archmage thinks it's something underground. All I know is my guards are getting attacked by enchanted brooms."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Guard Captain Aldric",
            text = "I understand, but people are getting hurt. Come back if you change your mind."
        })
    end
end

function show_progress_dialogue(ctx)
    local market = ctx:get_objective_progress("investigate_market")
    local college = ctx:get_objective_progress("investigate_college")
    local gate = ctx:get_objective_progress("investigate_gate")

    local done = 0
    if market.current >= market.target then done = done + 1 end
    if college.current >= college.target then done = done + 1 end
    if gate.current >= gate.target then done = done + 1 end

    ctx:show_dialogue({
        speaker = "Guard Captain Aldric",
        text = string.format("You've investigated %d of 3 sites. Archmage Yenara is waiting for your full report.", done)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Guard Captain Aldric",
        text = "You've checked all three? Good. Get to Archmage Yenara at the college - she'll want to hear everything."
    })
    ctx:complete_quest()
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "report_to_yenara" then
        ctx:show_notification("Report delivered to Archmage Yenara.")
    end
end
```

**Step 3: Write Quest 2 script - Containment Protocol**

Write `rust-server/data/scripts/quests/awakening/containment_protocol.lua`:

```lua
-- Containment Protocol - Quest 2 of The Awakening

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "The dampening crystals are helping, but the source remains. We must go deeper."
        })
    end
end

function show_offer_dialogue(ctx)
    local choice = ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "The disturbances have animated objects throughout the market - armor, crates, even furniture. They're attacking people. I need you to destroy them and bring me the dampening crystals inside them.",
        choices = {
            { id = "accept", text = "I'll handle it." },
            { id = "ask", text = "What are dampening crystals?" },
            { id = "decline", text = "Not now." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "Destroy the animated constructs and collect their crystals. I can use them to stabilize the ward matrix. Be careful - they're stronger than they look."
        })
    elseif choice == "ask" then
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "When wild magic animates an object, it crystallizes around a core. Those crystals absorb magical energy - exactly what I need to calm these disturbances."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "People are in danger. Please reconsider."
        })
    end
end

function show_progress_dialogue(ctx)
    local kills = ctx:get_objective_progress("kill_constructs")
    local crystals = ctx:get_objective_progress("collect_crystals")

    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = string.format("Progress: %d/%d constructs destroyed, %d/%d crystals collected.", kills.current, kills.target, crystals.current, crystals.target)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "Excellent work. These crystals will help stabilize the area."
    })
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "But this is only a stopgap. The source of these disturbances is something far deeper and far older. I need you to go beneath the city."
    })
    ctx:complete_quest()
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "kill_constructs" and new_count == 12 then
        ctx:show_notification("All constructs destroyed! Collect any remaining crystals.")
    end
end
```

**Step 4: Write Quest 3 script - The Old Foundation**

Write `rust-server/data/scripts/quests/awakening/the_old_foundation.lua`:

```lua
-- The Old Foundation - Quest 3 of The Awakening

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "That seal beneath the city haunts me. Whatever the Aetheri locked away, it's trying to break free."
        })
    end
end

function show_offer_dialogue(ctx)
    local choice = ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "My research confirms it - New Aeven was built on top of something ancient. The cisterns beneath the city lead to older stonework. I need you to go down there and find the source of these disturbances.",
        choices = {
            { id = "accept", text = "I'll explore the cisterns." },
            { id = "decline", text = "Sounds dangerous. Maybe later." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "Be careful down there. The magical energy is concentrated underground - there will be creatures drawn to it. Find the source and report back."
        })
    else
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "It IS dangerous. But so is letting this continue unchecked."
        })
    end
end

function show_progress_dialogue(ctx)
    local wraiths = ctx:get_objective_progress("kill_seal_wraiths")

    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = string.format("How goes the exploration? Wraiths defeated: %d/%d.", wraiths.current, wraiths.target)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "A sealed wall with unknown symbols? And wraiths drawn to it?"
    })
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "This is far more serious than I feared. Those symbols look ancient - older than anything in our archives. I must study them. Thank you for this discovery."
    })
    ctx:complete_quest()
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "find_seal" then
        ctx:show_notification("You've found the ancient seal! Report to Archmage Yenara.")
    end
end
```

**Step 5: Write Quest 4 script - Words of the Sealed**

Write `rust-server/data/scripts/quests/awakening/words_of_the_sealed.lua`:

```lua
-- Words of the Sealed - Quest 4 of The Awakening

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "The Resonance Lens has revealed much. The Aetheri's warnings are clear - and terrifying."
        })
    end
end

function show_offer_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "I've partially translated the cistern symbols. They read: 'What sleeps beneath the sand must never wake. The seals hold. The seals must hold.'"
    })

    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "The writing belongs to a lost civilization called the Aetheri. I need a Resonance Lens to read the rest, but it requires special materials."
    })

    local choice = ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "Can you gather 5 Refined Quartz from mining and 3 Construct Cores from the animated constructs?",
        choices = {
            { id = "accept", text = "I'll gather what you need." },
            { id = "decline", text = "Not right now." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "Refined Quartz can be found while mining. Construct Cores are rarer - you'll need to destroy more animated constructs. Bring everything to me."
        })
    else
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "Every moment we delay, the inscriptions fade further. Please hurry."
        })
    end
end

function show_progress_dialogue(ctx)
    local quartz = ctx:get_objective_progress("collect_quartz")
    local cores = ctx:get_objective_progress("collect_cores")

    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = string.format("Materials: Refined Quartz %d/%d, Construct Cores %d/%d.", quartz.current, quartz.target, cores.current, cores.target)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "You have everything. Let me assemble the lens..."
    })
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "There. The Resonance Lens is complete. With this, we can read what the Aetheri were desperately trying to tell us."
    })
    ctx:complete_quest()
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "collect_quartz" and new_count == 5 then
        ctx:show_notification("All Refined Quartz collected!")
    elseif objective_id == "collect_cores" and new_count == 3 then
        ctx:show_notification("All Construct Cores collected!")
    end
end
```

**Step 6: Write Quest 5 script - Tremors**

Write `rust-server/data/scripts/quests/awakening/tremors.lua`:

```lua
-- Tremors - Quest 5 of The Awakening

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "The wall has been repaired, but the ley line beneath us is still active. The desert holds the answers."
        })
    end
end

function show_offer_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "An earthquake just struck the city! The eastern wall has collapsed and creatures made of sand and shadow are pouring through!"
    })

    local choice = ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "We need you at the breach NOW. Destroy the Sand Wraiths before they overrun the district!",
        choices = {
            { id = "accept", text = "I'm on my way!" },
            { id = "decline", text = "I can't right now." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "Get to the eastern wall and destroy those Sand Wraiths! Then inspect the damage - I need to understand how they got here."
        })
    else
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "People will die if we don't act! Please reconsider!"
        })
    end
end

function show_progress_dialogue(ctx)
    local wraiths = ctx:get_objective_progress("kill_sand_wraiths")

    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = string.format("Sand Wraiths destroyed: %d/%d. Clear the breach!", wraiths.current, wraiths.target)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "A ley line connecting the desert directly to the seal beneath us... that explains everything."
    })
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "Whatever the Aetheri sealed away, its influence stretches far further than I imagined. We need to go to the source - the desert itself."
    })
    ctx:complete_quest()
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "kill_sand_wraiths" and new_count == 15 then
        ctx:show_notification("All Sand Wraiths destroyed! Inspect the breach.")
    end
end
```

**Step 7: Write Quest 6 script - The Desert Beckons**

Write `rust-server/data/scripts/quests/awakening/the_desert_beckons.lua`:

```lua
-- The Desert Beckons - Quest 6 of The Awakening

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "Have you found anything in the desert? Kael should be able to guide you to the primary seal."
        })
    end
end

function show_offer_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "I used the Resonance Lens in the cisterns. The sealed wall contains a map showing the location of a primary seal deep in the desert."
    })

    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "The Aetheri locked away something they called 'The Hollow King' - a being that consumed magic itself. These disturbances are aftershocks of its prison weakening."
    })

    local choice = ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "I have a contact in the desert - an Aetheri descendant named Kael. Will you find him?",
        choices = {
            { id = "accept", text = "I'll find Kael." },
            { id = "ask", text = "The Hollow King?" },
            { id = "decline", text = "I need to prepare first." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "Travel to the desert and find Kael near its edge. Show him my letter. He'll know what to do. Hurry - the seal won't hold much longer."
        })
    elseif choice == "ask" then
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "The inscriptions describe a being that feeds on magical energy. Every spell cast against it made it stronger. The Aetheri sacrificed everything to seal it away. And now those seals are crumbling."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "Prepare quickly. Every hour matters now."
        })
    end
end

function show_progress_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "Have you found Kael yet? He lives as a nomadic guide near the desert's edge. Time is of the essence."
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "You've met Kael? Good. He knows the desert and the Aetheri history better than anyone alive. Trust him, and be careful out there."
    })
    ctx:complete_quest()
end
```

**Step 8: Commit**

```bash
git add rust-server/data/scripts/quests/awakening/
git commit -m "feat: add New Aeven Lua quest scripts (quests 1-6)"
```

---

## Task 9: Lua Quest Scripts - Desert Chain (Quests 7-13)

**Files:**
- Create: `rust-server/data/scripts/quests/awakening/shifting_sands.lua`
- Create: `rust-server/data/scripts/quests/awakening/echoes_of_aether.lua`
- Create: `rust-server/data/scripts/quests/awakening/the_tomb_raider.lua`
- Create: `rust-server/data/scripts/quests/awakening/restoration.lua`
- Create: `rust-server/data/scripts/quests/awakening/the_second_seal.lua`
- Create: `rust-server/data/scripts/quests/awakening/the_last_seal.lua`
- Create: `rust-server/data/scripts/quests/awakening/the_hollow_king.lua`

**Step 1: Write Quest 7 script - Shifting Sands**

Write `rust-server/data/scripts/quests/awakening/shifting_sands.lua`:

```lua
-- Shifting Sands - Quest 7 of The Awakening

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        ctx:show_dialogue({
            speaker = "Kael",
            text = "You've proven yourself in the sands. Now the real work begins."
        })
    end
end

function show_offer_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Kael",
        text = "So Yenara sent you. I felt the tremors too - the seals are weakening. Before I take you deeper into the desert, I need to know you can survive out here."
    })

    local choice = ctx:show_dialogue({
        speaker = "Kael",
        text = "Navigate to three landmarks that mark the old Aetheri borders. And be ready to fight - the creatures here are restless.",
        choices = {
            { id = "accept", text = "I'm ready." },
            { id = "ask", text = "Tell me about the Aetheri." },
            { id = "decline", text = "I need more time." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Kael",
            text = "Find the Stone Pillars, the Buried Archway, and the Sunken Colossus. Watch for scorpions and vipers - they can feel the tremors and it makes them aggressive."
        })
    elseif choice == "ask" then
        ctx:show_dialogue({
            speaker = "Kael",
            text = "My ancestors. They built a civilization of magic here, before the sands took everything. I'm the last of their line. I'll tell you more once I know you can survive the journey."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Kael",
            text = "The desert doesn't wait. But I understand. Come back when you're ready."
        })
    end
end

function show_progress_dialogue(ctx)
    local pillars = ctx:get_objective_progress("reach_stone_pillars")
    local archway = ctx:get_objective_progress("reach_buried_archway")
    local colossus = ctx:get_objective_progress("reach_sunken_colossus")
    local scorpions = ctx:get_objective_progress("kill_scorpions")
    local vipers = ctx:get_objective_progress("kill_vipers")

    local landmarks = 0
    if pillars.current >= pillars.target then landmarks = landmarks + 1 end
    if archway.current >= archway.target then landmarks = landmarks + 1 end
    if colossus.current >= colossus.target then landmarks = landmarks + 1 end

    ctx:show_dialogue({
        speaker = "Kael",
        text = string.format("Landmarks found: %d/3. Scorpions: %d/%d. Vipers: %d/%d.", landmarks, scorpions.current, scorpions.target, vipers.current, vipers.target)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Kael",
        text = "You survived. Good. The creatures can feel what's happening underground - that's why they're hostile."
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "Now let me show you something that will explain everything. There's an Aetheri Obelisk nearby that still functions."
    })
    ctx:complete_quest()
end
```

**Step 2: Write Quest 8 script - Echoes of Aether**

Write `rust-server/data/scripts/quests/awakening/echoes_of_aether.lua`:

```lua
-- Echoes of Aether - Quest 8 of The Awakening
-- The major lore reveal - the Aetheri vision

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        ctx:show_dialogue({
            speaker = "Kael",
            text = "The obelisk is silent now. But its message lives on in what we must do next."
        })
    end
end

function show_offer_dialogue(ctx)
    local choice = ctx:show_dialogue({
        speaker = "Kael",
        text = "There's an Aetheri Obelisk nearby that still holds power. If we activate it, you'll see what my ancestors saw. What they fought. What they sealed. Are you ready?",
        choices = {
            { id = "accept", text = "Show me." },
            { id = "decline", text = "Not yet." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Kael",
            text = "Follow the path to the obelisk. I'll meet you there. Be ready for what you see - the truth is not pleasant."
        })
    else
        ctx:show_dialogue({
            speaker = "Kael",
            text = "Take your time. But know that the seals weaken with every passing hour."
        })
    end
end

function show_progress_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Kael",
        text = "Have you reached the obelisk? I'll meet you there."
    })
end

function complete_quest(ctx)
    -- The vision sequence
    ctx:show_dialogue({
        speaker = "The Obelisk",
        text = "The lens glows as you touch the obelisk. Images flood your mind..."
    })
    ctx:show_dialogue({
        speaker = "The Obelisk",
        text = "A great civilization of towers and magic. The Aetheri, at the height of their power. Then... darkness."
    })
    ctx:show_dialogue({
        speaker = "The Obelisk",
        text = "A shadow rises from the depths. Every spell thrown at it is consumed. It grows larger, stronger, feeding on the magic meant to destroy it."
    })
    ctx:show_dialogue({
        speaker = "The Obelisk",
        text = "The Aetheri elders gather. They pour their magic - their life force - into three great seals. Light fades from their eyes as their power is spent."
    })
    ctx:show_dialogue({
        speaker = "The Obelisk",
        text = "The shadow is pulled into the earth. Sealed. Bound. The Aetheri crumble to dust alongside their creation. The obelisk cracks and goes dark."
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "Now you understand. The Hollow King consumed every spell thrown at it. My ancestors gave everything to seal it away."
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "Their magic, their civilization, their future - all sacrificed. And those seals are failing. We must act before the Hollow King breaks free."
    })
    ctx:complete_quest()
end
```

**Step 3: Write Quest 9 script - The Tomb Raider**

Write `rust-server/data/scripts/quests/awakening/the_tomb_raider.lua`:

```lua
-- The Tomb Raider - Quest 9 of The Awakening

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        ctx:show_dialogue({
            speaker = "Serah",
            text = "Treasure hunting was simpler when the biggest threat was a rusty trap door. Now I'm fighting shadow monsters. Great career choice, Serah."
        })
    end
end

function show_offer_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Serah",
        text = "Hey! You look like someone who can handle themselves. I found an entrance to an Aetheri chamber down that slope, but it's crawling with stone guardians."
    })

    local choice = ctx:show_dialogue({
        speaker = "Serah",
        text = "Help me clear them out and we split whatever treasure's inside. Deal?",
        choices = {
            { id = "accept", text = "Deal. Let's clear them out." },
            { id = "ask", text = "Who are you?" },
            { id = "decline", text = "No thanks." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Serah",
            text = "Great! The golems are tough - slow, but they hit like a rockslide. Take them down and meet me at the inner chamber. This is going to be a big score!"
        })
    elseif choice == "ask" then
        ctx:show_dialogue({
            speaker = "Serah",
            text = "Name's Serah. Professional treasure hunter, amateur archaeologist, and currently in way over my head. Now, about those golems..."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Serah",
            text = "Your loss! More treasure for me. ...If I can get past the golems. Which I definitely can. Probably."
        })
    end
end

function show_progress_dialogue(ctx)
    local golems = ctx:get_objective_progress("kill_sand_golems")

    ctx:show_dialogue({
        speaker = "Serah",
        text = string.format("Golems down: %d/%d. Keep at it! The treasure's waiting!", golems.current, golems.target)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Serah",
        text = "No gold. No gems. Just this... cracked wall, pulsing with dark energy. What IS that?"
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "That is one of three Binding Seals. It's what keeps the Hollow King imprisoned beneath the desert. And it's failing."
    })
    ctx:show_dialogue({
        speaker = "Serah",
        text = "The Hollow... you mean the stories are true? The ancient evil sealed underground? I thought that was just something locals said to keep tourists away."
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "It is very, very real. And we need to reinforce these seals before it breaks free entirely."
    })
    ctx:complete_quest()
end
```

**Step 4: Write Quest 10 script - Restoration**

Write `rust-server/data/scripts/quests/awakening/restoration.lua`:

```lua
-- Restoration - Quest 10 of The Awakening

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        ctx:show_dialogue({
            speaker = "Kael",
            text = "The runestones are ready. Now we must reach the second seal."
        })
    end
end

function show_offer_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Kael",
        text = "I know a ritual to reinforce the seals - one passed down through generations of my family. But it requires Aetheri Runestones, powerful artifacts buried at sacred sites across the desert."
    })

    local choice = ctx:show_dialogue({
        speaker = "Kael",
        text = "I've marked three dig sites. But be warned - the Hollow King's power seeps through the weakened seals. Guardians will appear when you dig. Fight them with steel, not spells.",
        choices = {
            { id = "accept", text = "I'll recover the runestones." },
            { id = "ask", text = "Why steel and not spells?" },
            { id = "decline", text = "I need to prepare." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Kael",
            text = "Take a shovel and dig at the three sites. When the Hollow Sentinels appear, destroy them and claim the runestones. Be ready - they're tough."
        })
    elseif choice == "ask" then
        ctx:show_dialogue({
            speaker = "Kael",
            text = "The Hollow King's creatures feed on magical energy. Your spells will be less effective - or worse, might strengthen them. Cold steel is the answer."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Kael",
            text = "Prepare well. The Sentinels are not to be taken lightly."
        })
    end
end

function show_progress_dialogue(ctx)
    local sentinels = ctx:get_objective_progress("kill_sentinels")
    local runestones = ctx:get_objective_progress("collect_runestones")

    ctx:show_dialogue({
        speaker = "Kael",
        text = string.format("Sentinels defeated: %d/%d. Runestones recovered: %d/%d.", sentinels.current, sentinels.target, runestones.current, runestones.target)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Kael",
        text = "Three runestones, recovered after centuries beneath the sand. I can feel their power. My ancestors would be proud."
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "Now we must reach the second Binding Seal and perform the ritual before it's too late."
    })
    ctx:complete_quest()
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "collect_runestones" and new_count == 3 then
        ctx:show_notification("All three Aetheri Runestones recovered!")
    end
end
```

**Step 5: Write Quest 11 script - The Second Seal**

Write `rust-server/data/scripts/quests/awakening/the_second_seal.lua`:

```lua
-- The Second Seal - Quest 11 of The Awakening
-- The twist: the ritual fails, the seal shatters

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        ctx:show_dialogue({
            speaker = "Kael",
            text = "Two seals broken. One remains. We cannot fail again."
        })
    end
end

function show_offer_dialogue(ctx)
    local choice = ctx:show_dialogue({
        speaker = "Kael",
        text = "The second Binding Seal is inside a temple deeper in the desert. The corruption there is worse - Hollow Shades patrol its halls. They feed on magic, so spells won't work well. Ready?",
        choices = {
            { id = "accept", text = "Let's reinforce the seal." },
            { id = "decline", text = "I need more preparation." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Kael",
            text = "Enter the Desert Temple and clear the Hollow Shades. Reach the seal chamber and I'll perform the ritual. Together, we can save the seal."
        })
    else
        ctx:show_dialogue({
            speaker = "Kael",
            text = "Every moment we wait, the seal weakens further. But go prepare if you must."
        })
    end
end

function show_progress_dialogue(ctx)
    local shades = ctx:get_objective_progress("kill_hollow_shades")

    ctx:show_dialogue({
        speaker = "Kael",
        text = string.format("Hollow Shades destroyed: %d/%d. Clear a path to the seal chamber.", shades.current, shades.target)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Kael",
        text = "The runestones are placed. Now I channel the ritual... focus... HOLD..."
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "No... NO! The corruption is too deep! The seal is-"
    })
    ctx:show_dialogue({
        speaker = "The Hollow King",
        text = "Three seals. Two remain. One is already mine."
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "The seal shattered. And that voice... the first seal beneath New Aeven has already broken completely. We were too late to save it."
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "One seal remains. In the Aetheri Sanctum - the heart of my ancestors' civilization. If that falls, nothing can contain the Hollow King."
    })
    ctx:complete_quest()
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "kill_hollow_shades" and new_count == 12 then
        ctx:show_notification("Path cleared! Reach the seal chamber.")
    end
end
```

**Step 6: Write Quest 12 script - The Last Seal**

Write `rust-server/data/scripts/quests/awakening/the_last_seal.lua`:

```lua
-- The Last Seal - Quest 12 of The Awakening

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        ctx:show_dialogue({
            speaker = "Kael",
            text = "The Sanctum is open. What awaits inside will determine the fate of everything."
        })
    end
end

function show_offer_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Kael",
        text = "One seal remains in the Aetheri Sanctum. But the desert is changing - the Hollow King's influence spreads. New creatures roam the surface. We must hurry."
    })

    local choice = ctx:show_dialogue({
        speaker = "Kael",
        text = "Fight through the Hollow Devourers and reach the Sanctum entrance. Serah said she would meet us there.",
        choices = {
            { id = "accept", text = "Let's end this." },
            { id = "decline", text = "I need to prepare." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Kael",
            text = "The Devourers are the worst yet. Stay sharp and fight your way to the Sanctum. Serah has something that might help us get inside."
        })
    else
        ctx:show_dialogue({
            speaker = "Kael",
            text = "Prepare quickly. The last seal won't hold forever."
        })
    end
end

function show_progress_dialogue(ctx)
    local devourers = ctx:get_objective_progress("kill_devourers")

    ctx:show_dialogue({
        speaker = "Kael",
        text = string.format("Hollow Devourers destroyed: %d/%d. Push through to the Sanctum!", devourers.current, devourers.target)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Serah",
        text = "I came here for treasure. Now I'd settle for the world not ending."
    })
    ctx:show_dialogue({
        speaker = "Serah",
        text = "Here - this key I found in the first ruin. I thought it was worthless, but it fits the Sanctum door perfectly."
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "The Aetheri Key... of course. Only the worthy could enter the Sanctum. The door is open."
    })
    ctx:give_item("aetheri_key", 1)
    ctx:complete_quest()
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "kill_devourers" and new_count == 10 then
        ctx:show_notification("Path cleared! Reach the Aetheri Sanctum entrance.")
    end
end
```

**Step 7: Write Quest 13 script - The Hollow King**

Write `rust-server/data/scripts/quests/awakening/the_hollow_king.lua`:

```lua
-- The Hollow King - Quest 13 of The Awakening
-- The final boss encounter

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        ctx:show_dialogue({
            speaker = "Kael",
            text = "The seal holds. I will watch over it as my ancestors intended. Thank you, my friend."
        })
    end
end

function show_offer_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Kael",
        text = "This is it. The Aetheri Sanctum - the heart of everything my ancestors built. The Hollow King is inside, straining against the last seal."
    })

    local choice = ctx:show_dialogue({
        speaker = "Kael",
        text = "The Hollow King feeds on magic. Use steel and arrows, not spells. When the time comes, I'll channel the remaining runestone energy to weaken it. Are you ready?",
        choices = {
            { id = "accept", text = "Let's finish this." },
            { id = "ask", text = "Any advice?" },
            { id = "decline", text = "I need more time." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Kael",
            text = "Fight through the Sanctum's defenders. When you face the Hollow King, stay close and use melee. I'll be with you every step."
        })
    elseif choice == "ask" then
        ctx:show_dialogue({
            speaker = "Kael",
            text = "It will try to drain your magic. It will spawn fragments of itself. Kill them quickly or they'll heal it. And above all - do not rely on spells until I weaken its resistance."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Kael",
            text = "I understand. But the last seal is cracking as we speak."
        })
    end
end

function show_progress_dialogue(ctx)
    local shades = ctx:get_objective_progress("kill_hollow_enemies")
    local sentinels = ctx:get_objective_progress("kill_hollow_sentinels_sanctum")

    ctx:show_dialogue({
        speaker = "Kael",
        text = string.format("Shades: %d/%d. Sentinels: %d/%d. Keep pushing forward.", shades.current, shades.target, sentinels.current, sentinels.target)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Kael",
        text = "It's weakening! Now - help me channel the runestone energy!"
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "The seal... is reforming. I can feel it binding the Hollow King once more."
    })
    ctx:show_dialogue({
        speaker = "The Hollow King",
        text = "This... is not... over..."
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "The Sanctum is collapsing! We need to get out - NOW!"
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "We made it. The tremors have stopped. The desert air feels... lighter. It's over. For now."
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "I'll stay here and watch over the seal. It's my birthright - what I was born to do."
    })
    ctx:show_dialogue({
        speaker = "Serah",
        text = "Well, I'm heading to New Aeven. Got some Aetheri artifacts that need... studying. At a very expensive price. Take care of yourself out here, friend."
    })
    ctx:show_dialogue({
        speaker = "Kael",
        text = "Thank you. The world owes you a debt it can never repay. Take this - forged from the seal's energy. May it protect you on your journeys."
    })
    ctx:complete_quest()
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "defeat_hollow_king" and new_count == 1 then
        ctx:show_notification("The Hollow King falls! Speak with Kael.")
    end
end
```

**Step 8: Commit**

```bash
git add rust-server/data/scripts/quests/awakening/
git commit -m "feat: add Desert Lua quest scripts (quests 7-13)"
```

---

## Task 10: Instance Stub JSON Files

The three instances need map data (tile layers, collision) which requires the mapper tool to build visually. This task creates valid stub files with minimal placeholder maps that the server can load. The stubs should be replaced with proper maps later using the mapper.

**Files:**
- Create: `rust-server/maps/interiors/na_cisterns.json`
- Create: `rust-server/maps/interiors/desert_temple.json`
- Create: `rust-server/maps/interiors/aetheri_sanctum.json`

**Step 1: Create New Aeven Cisterns instance stub**

Write `rust-server/maps/interiors/na_cisterns.json`:

```json
{
  "id": "na_cisterns",
  "name": "New Aeven Cisterns",
  "instance_type": "private",
  "size": {
    "width": 16,
    "height": 16
  },
  "spawn_points": {
    "entrance": { "x": 8, "y": 15 }
  },
  "layers": {
    "ground": [],
    "objects": [],
    "overhead": []
  },
  "collision": "",
  "entities": [
    { "entityId": "seal_wraith", "x": 4, "y": 8 },
    { "entityId": "seal_wraith", "x": 12, "y": 8 },
    { "entityId": "seal_wraith", "x": 6, "y": 5 },
    { "entityId": "seal_wraith", "x": 10, "y": 5 },
    { "entityId": "seal_wraith", "x": 3, "y": 3 },
    { "entityId": "seal_wraith", "x": 13, "y": 3 },
    { "entityId": "seal_wraith", "x": 7, "y": 2 },
    { "entityId": "seal_wraith", "x": 9, "y": 2 }
  ],
  "mapObjects": [],
  "walls": [],
  "portals": [
    {
      "id": "na_cisterns_exit",
      "x": 8,
      "y": 15,
      "width": 1,
      "height": 1,
      "target_map": "overworld",
      "target_x": 0.0,
      "target_y": 0.0,
      "target_spawn": null
    }
  ],
  "chests": []
}
```

**Step 2: Create Desert Temple instance stub**

Write `rust-server/maps/interiors/desert_temple.json`:

```json
{
  "id": "desert_temple",
  "name": "Desert Temple",
  "instance_type": "private",
  "size": {
    "width": 20,
    "height": 20
  },
  "spawn_points": {
    "entrance": { "x": 10, "y": 19 }
  },
  "layers": {
    "ground": [],
    "objects": [],
    "overhead": []
  },
  "collision": "",
  "entities": [
    { "entityId": "hollow_shade", "x": 5, "y": 15 },
    { "entityId": "hollow_shade", "x": 15, "y": 15 },
    { "entityId": "hollow_shade", "x": 3, "y": 12 },
    { "entityId": "hollow_shade", "x": 17, "y": 12 },
    { "entityId": "hollow_shade", "x": 6, "y": 9 },
    { "entityId": "hollow_shade", "x": 14, "y": 9 },
    { "entityId": "hollow_shade", "x": 4, "y": 6 },
    { "entityId": "hollow_shade", "x": 16, "y": 6 },
    { "entityId": "hollow_shade", "x": 8, "y": 4 },
    { "entityId": "hollow_shade", "x": 12, "y": 4 },
    { "entityId": "hollow_shade", "x": 7, "y": 2 },
    { "entityId": "hollow_shade", "x": 13, "y": 2 }
  ],
  "mapObjects": [],
  "walls": [],
  "portals": [
    {
      "id": "desert_temple_exit",
      "x": 10,
      "y": 19,
      "width": 1,
      "height": 1,
      "target_map": "overworld",
      "target_x": 0.0,
      "target_y": 0.0,
      "target_spawn": null
    }
  ],
  "chests": []
}
```

**Step 3: Create Aetheri Sanctum instance stub**

Write `rust-server/maps/interiors/aetheri_sanctum.json`:

```json
{
  "id": "aetheri_sanctum",
  "name": "Aetheri Sanctum",
  "instance_type": "private",
  "size": {
    "width": 24,
    "height": 24
  },
  "spawn_points": {
    "entrance": { "x": 12, "y": 23 }
  },
  "layers": {
    "ground": [],
    "objects": [],
    "overhead": []
  },
  "collision": "",
  "entities": [
    { "entityId": "hollow_shade", "x": 5, "y": 18 },
    { "entityId": "hollow_shade", "x": 19, "y": 18 },
    { "entityId": "hollow_shade", "x": 8, "y": 15 },
    { "entityId": "hollow_shade", "x": 16, "y": 15 },
    { "entityId": "hollow_shade", "x": 4, "y": 12 },
    { "entityId": "hollow_shade", "x": 20, "y": 12 },
    { "entityId": "hollow_shade", "x": 10, "y": 10 },
    { "entityId": "hollow_shade", "x": 14, "y": 10 },
    { "entityId": "hollow_shade", "x": 6, "y": 8 },
    { "entityId": "hollow_shade", "x": 18, "y": 8 },
    { "entityId": "hollow_sentinel", "x": 3, "y": 14 },
    { "entityId": "hollow_sentinel", "x": 21, "y": 14 },
    { "entityId": "hollow_sentinel", "x": 12, "y": 6 },
    { "entityId": "hollow_sentinel", "x": 7, "y": 4 },
    { "entityId": "hollow_sentinel", "x": 17, "y": 4 },
    { "entityId": "hollow_king", "x": 12, "y": 2, "uniqueId": "the_hollow_king" }
  ],
  "mapObjects": [],
  "walls": [],
  "portals": [
    {
      "id": "aetheri_sanctum_exit",
      "x": 12,
      "y": 23,
      "width": 1,
      "height": 1,
      "target_map": "overworld",
      "target_x": 0.0,
      "target_y": 0.0,
      "target_spawn": null
    }
  ],
  "chests": []
}
```

**Step 4: Commit**

```bash
git add rust-server/maps/interiors/na_cisterns.json rust-server/maps/interiors/desert_temple.json rust-server/maps/interiors/aetheri_sanctum.json
git commit -m "feat: add instance stubs for Cisterns, Desert Temple, and Aetheri Sanctum"
```

---

## Task 11: Verify Full Server Compilation

**Step 1: Run cargo check**

Run: `cd rust-server && cargo check 2>&1 | tail -20`
Expected: `Finished` with no errors. Warnings are acceptable (113 pre-existing).

**Step 2: Run cargo test (if tests exist)**

Run: `cd rust-server && cargo test 2>&1 | tail -20`
Expected: All existing tests pass.

**Step 3: Commit any fixes if needed**

---

## Outstanding Work (Not in This Plan)

These items require visual tools or Rust code changes and should be planned separately:

### Map/Art Work (requires mapper tool)
- [ ] Build New Aeven city zone in the mapper (place NPCs, set coordinates)
- [ ] Build Desert zone in the mapper (place NPCs, monsters, landmarks)
- [ ] Design instance tile maps for Cisterns, Desert Temple, Aetheri Sanctum (replace stubs)
- [ ] Create/source sprite art for all new monsters, NPCs, and items
- [ ] Update quest_locations.toml coordinates once zones are built
- [ ] Update dig_sites.toml coordinates once zones are built
- [ ] Add overworld portals to instance entrances in chunk JSONs

### Stretch Goals (require Rust code changes)
- [ ] Hollow Shade magic-healing mechanic (magic attacks heal instead of damage)
- [ ] Hollow Sentinel mana-drain-on-hit mechanic
- [ ] Hollow King boss phases (magic resistance toggle, add spawning, berserk mode)
- [ ] Magic Drain AoE ability for Hollow King
- [ ] Refined Quartz as a mining drop (add to mining loot tables)
- [ ] Kael's desert supplies shop definition (`data/shops/kael_desert_supplies.toml`)
- [ ] Serah post-quest relocation to New Aeven (conditional NPC placement)
