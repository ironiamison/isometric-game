# Pharaoh's Curse — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a desert quest where the player investigates an ancient cursed pharaoh through NPC conversations, solves a dialogue riddle, and fights a stationary caster boss beneath the pyramid.

**Architecture:** New quest data (TOML + Lua script) using the existing quest system. New boss type (`PharaohState`) in a separate module alongside the existing `BossState` (Desert Wurm), reusing `BossEvent` and the boss tick pipeline. The pharaoh boss is a stationary caster that fires ranged projectiles and spawns melee minions in phases.

**Tech Stack:** Rust (server), TOML (data), Lua (quest script), JSON (interior map)

**Design doc:** `docs/plans/2026-03-23-pharaohs-curse-quest-design.md`

---

### Task 1: Quest Item — Pharaoh's Key

**Files:**
- Modify: `rust-server/data/items/tools.toml`

**Step 1: Add the quest item definition**

Append to the end of `rust-server/data/items/tools.toml`:

```toml
# =============================================================================
# Pharaoh's Curse - Quest Items
# =============================================================================

[pharaohs_key]
display_name = "Pharaoh's Key"
sprite = "pharaohs_key"
description = "An ancient key inscribed with hieroglyphs. It hums with dark energy."
category = "quest"
max_stack = 1
base_price = 0
sellable = false
```

**Step 2: Verify the server loads the item**

Run: `cd rust-server && cargo check 2>&1 | head -5`
Expected: No new errors (existing warnings OK)

**Step 3: Commit**

```bash
git add rust-server/data/items/tools.toml
git commit -m "feat: add Pharaoh's Key quest item"
```

---

### Task 2: NPC Entity Definitions

**Files:**
- Create: `rust-server/data/entities/npcs/desert_pharaoh_quest.toml`

**Step 1: Create the 4 quest NPC definitions**

Create `rust-server/data/entities/npcs/desert_pharaoh_quest.toml`:

```toml
# =============================================================================
# Pharaoh's Curse Quest NPCs
# =============================================================================

# Desert Merchant - first hint about the pyramid
[desert_merchant_quest]
display_name = "Desert Merchant"
sprite = "desert_merchant"
animation_type = "humanoid"
description = "A trader who has heard strange sounds from the pyramid."

[desert_merchant_quest.stats]
max_hp = 100
damage = 0
attack_range = 0
aggro_range = 0
chase_range = 0
move_cooldown_ms = 600
attack_cooldown_ms = 0
respawn_time_ms = 0

[desert_merchant_quest.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[desert_merchant_quest.behaviors]
hostile = false
wander_enabled = true
wander_radius = 2
wander_pause_min_ms = 6000
wander_pause_max_ms = 12000

[desert_merchant_quest.speech]
radius = 5
interval_min_ms = 30000
interval_max_ms = 50000
messages = [
    "Business has been slow... nobody wants to come near the pyramid anymore.",
    "I hear chanting beneath the sand at night. Gives me the shivers.",
]

[desert_merchant_quest.dialogue]
greeting = "Welcome, traveler. Looking to trade? ...Or are you here about the pyramid?"

# --------------------------------------------------------

# Nomad Elder - tells the legend of Kha'reth
[nomad_elder]
display_name = "Nomad Elder"
sprite = "nomad_elder"
animation_type = "humanoid"
description = "A wise elder who knows the desert's ancient history."

[nomad_elder.stats]
max_hp = 100
damage = 0
attack_range = 0
aggro_range = 0
chase_range = 0
move_cooldown_ms = 600
attack_cooldown_ms = 0
respawn_time_ms = 0

[nomad_elder.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[nomad_elder.behaviors]
hostile = false
wander_enabled = false

[nomad_elder.speech]
radius = 5
interval_min_ms = 35000
interval_max_ms = 55000
messages = [
    "The desert remembers what men forget.",
    "The stars above guide us... and bind things below.",
]

[nomad_elder.dialogue]
greeting = "Sit, young one. The desert has stories to tell."

# --------------------------------------------------------

# Tomb Researcher - studying pyramid inscriptions
[tomb_researcher]
display_name = "Tomb Researcher"
sprite = "tomb_researcher"
animation_type = "humanoid"
description = "A scholar camped near the pyramid, translating ancient inscriptions."

[tomb_researcher.stats]
max_hp = 100
damage = 0
attack_range = 0
aggro_range = 0
chase_range = 0
move_cooldown_ms = 600
attack_cooldown_ms = 0
respawn_time_ms = 0

[tomb_researcher.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[tomb_researcher.behaviors]
hostile = false
wander_enabled = true
wander_radius = 3
wander_pause_min_ms = 8000
wander_pause_max_ms = 15000

[tomb_researcher.speech]
radius = 5
interval_min_ms = 25000
interval_max_ms = 45000
messages = [
    "These hieroglyphs mention a ritual... blood of the scorpion...",
    "Others have gone inside the pyramid. None came back.",
]

[tomb_researcher.dialogue]
greeting = "Careful around here. I've been studying these inscriptions for months."

# --------------------------------------------------------

# Desert Hermit - guards the ancient book
[desert_hermit]
display_name = "Desert Hermit"
sprite = "desert_hermit"
animation_type = "humanoid"
description = "A reclusive scholar who has spent decades studying the cursed pharaoh."

[desert_hermit.stats]
max_hp = 100
damage = 0
attack_range = 0
aggro_range = 0
chase_range = 0
move_cooldown_ms = 600
attack_cooldown_ms = 0
respawn_time_ms = 0

[desert_hermit.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[desert_hermit.behaviors]
hostile = false
wander_enabled = false

[desert_hermit.speech]
radius = 5
interval_min_ms = 40000
interval_max_ms = 60000
messages = [
    "Kha'reth... even speaking the name feels dangerous.",
    "The book holds the answers, but only for those who already know the questions.",
]

[desert_hermit.dialogue]
greeting = "You've come a long way to find me. What do you seek?"
```

**Step 2: Verify**

Run: `cd rust-server && cargo check 2>&1 | head -5`
Expected: No new errors

**Step 3: Commit**

```bash
git add rust-server/data/entities/npcs/desert_pharaoh_quest.toml
git commit -m "feat: add 4 quest NPCs for Pharaoh's Curse quest"
```

---

### Task 3: Boss & Minion Entity Definitions

**Files:**
- Create: `rust-server/data/entities/monsters/pharaoh_boss.toml`

**Step 1: Create boss and minion entity definitions**

Create `rust-server/data/entities/monsters/pharaoh_boss.toml`:

```toml
# =============================================================================
# Kha'reth, the Cursed Pharaoh - Stationary Caster Boss
# =============================================================================

[khareth_pharaoh]
display_name = "Kha'reth, the Cursed Pharaoh"
sprite = "khareth_pharaoh"
animation_type = "humanoid"
size = 2
description = "An ancient pharaoh corrupted by dark magic, sealed beneath the pyramid for millennia."

[khareth_pharaoh.stats]
level = 55
max_hp = 300
damage = 8
attack_range = 10
aggro_range = 999
chase_range = 0
move_cooldown_ms = 0
attack_cooldown_ms = 2000
respawn_time_ms = 0

[khareth_pharaoh.rewards]
exp_base = 800
gold_min = 5000
gold_max = 20000

[khareth_pharaoh.behaviors]
hostile = true
no_shadow = true

# Loot tables TBD — placeholder for now
[[khareth_pharaoh.loot]]
item_id = "pharaoh_bones"
drop_chance = 1.0
quantity_min = 1
quantity_max = 3

# =============================================================================
# Pharaoh Mummy - Phase 1 Minion
# =============================================================================

[pharaoh_mummy]
display_name = "Risen Mummy"
sprite = "pharaoh_mummy"
animation_type = "humanoid"
description = "A mummified servant of Kha'reth, risen to defend its master."

[pharaoh_mummy.stats]
level = 30
max_hp = 25
damage = 4
attack_range = 1
aggro_range = 8
chase_range = 12
move_cooldown_ms = 400
attack_cooldown_ms = 1500
respawn_time_ms = 0

[pharaoh_mummy.rewards]
exp_base = 30
gold_min = 0
gold_max = 0

[pharaoh_mummy.behaviors]
hostile = true

# =============================================================================
# Pharaoh Skeleton - Phase 2 Minion (stronger)
# =============================================================================

[pharaoh_skeleton]
display_name = "Cursed Skeleton"
sprite = "pharaoh_skeleton"
animation_type = "humanoid"
description = "A skeletal warrior bound by Kha'reth's dark magic."

[pharaoh_skeleton.stats]
level = 35
max_hp = 35
damage = 5
attack_range = 1
aggro_range = 8
chase_range = 12
move_cooldown_ms = 350
attack_cooldown_ms = 1400
respawn_time_ms = 0

[pharaoh_skeleton.rewards]
exp_base = 40
gold_min = 0
gold_max = 0

[pharaoh_skeleton.behaviors]
hostile = true
```

**Step 2: Verify**

Run: `cd rust-server && cargo check 2>&1 | head -5`

**Step 3: Commit**

```bash
git add rust-server/data/entities/monsters/pharaoh_boss.toml
git commit -m "feat: add Kha'reth boss and minion entity definitions"
```

---

### Task 4: Quest TOML Definition

**Files:**
- Create: `rust-server/data/quests/desert_pharaoh/pharaohs_curse.toml`

**Step 1: Create the quest directory and definition**

```bash
mkdir -p rust-server/data/quests/desert_pharaoh
```

Create `rust-server/data/quests/desert_pharaoh/pharaohs_curse.toml`:

```toml
[quest]
id = "pharaohs_curse"
name = "The Pharaoh's Curse"
description = "Investigate rumors of an ancient entity sealed beneath the desert pyramid. Speak to the locals, uncover the truth, and confront what lies below."
giver_npc = "desert_merchant_quest"
level_required = 50
repeatable = false
lua_script = "desert_pharaoh/pharaohs_curse.lua"

[quest.chain]
# First quest in the desert_pharaoh chain — no previous

[[quest.objectives]]
id = "talk_merchant"
type = "talk_to"
target = "desert_merchant_quest"
description = "Speak to the Desert Merchant about the pyramid"

[[quest.objectives]]
id = "talk_elder"
type = "talk_to"
target = "nomad_elder"
description = "Seek the Nomad Elder's knowledge of the legend"
sequential = true

[[quest.objectives]]
id = "talk_researcher"
type = "talk_to"
target = "tomb_researcher"
description = "Consult the Tomb Researcher near the pyramid"
sequential = true

[[quest.objectives]]
id = "talk_hermit"
type = "talk_to"
target = "desert_hermit"
description = "Find the Desert Hermit and solve the riddle"
sequential = true

[[quest.objectives]]
id = "enter_tomb"
type = "reach_location"
target = "pharaoh_tomb_entrance"
count = 1
description = "Enter the pharaoh's tomb beneath the pyramid"
sequential = true

[[quest.objectives]]
id = "defeat_khareth"
type = "kill_monster"
target = "khareth_pharaoh"
count = 1
description = "Defeat Kha'reth, the Cursed Pharaoh"
sequential = true

[quest.rewards]
exp = 1500
gold = 2000

[quest.dialogue]
offer = "You're brave to come out here. I've been hearing strange chanting from the pyramid at night... something is stirring beneath the sand."
accept = "If you're serious about investigating, talk to the Nomad Elder at the oasis. He knows the old stories."
progress = "Have you spoken to the others? The pyramid holds dark secrets..."
complete = "You... defeated whatever was down there? The chanting has stopped. The desert feels lighter somehow. Thank you, adventurer."
```

**Step 2: Verify**

Run: `cd rust-server && cargo check 2>&1 | head -5`

**Step 3: Commit**

```bash
git add rust-server/data/quests/desert_pharaoh/pharaohs_curse.toml
git commit -m "feat: add Pharaoh's Curse quest definition"
```

---

### Task 5: Quest Location

**Files:**
- Modify: `rust-server/data/quest_locations.toml`

**Step 1: Add the tomb entrance location**

Append to `rust-server/data/quest_locations.toml`:

```toml
# Pharaoh's Curse - tomb entrance inside pyramid
[pharaoh_tomb_entrance]
x = 0
y = 0
radius = 2
```

> **Note:** The x/y coordinates are placeholders — update once the map placement is decided.

**Step 2: Commit**

```bash
git add rust-server/data/quest_locations.toml
git commit -m "feat: add pharaoh tomb entrance quest location"
```

---

### Task 6: Lua Quest Script

**Files:**
- Create: `rust-server/data/scripts/quests/desert_pharaoh/pharaohs_curse.lua`

**Step 1: Create the script directory**

```bash
mkdir -p rust-server/data/scripts/quests/desert_pharaoh
```

**Step 2: Write the Lua quest script**

Create `rust-server/data/scripts/quests/desert_pharaoh/pharaohs_curse.lua`:

```lua
-- The Pharaoh's Curse - Quest 1 of the Desert Pharaoh chain
-- Player investigates an ancient cursed pharaoh through NPC conversations,
-- solves a riddle, and fights a boss beneath the pyramid.

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        show_completed_dialogue(ctx)
    end
end

-- ============================================================================
-- Quest Offer (Desert Merchant)
-- ============================================================================

function show_offer_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "Ah, a traveler! Business has been terrible lately. Nobody wants to come near the pyramid anymore."
    })

    ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "I hear chanting from beneath the sand at night. The locals whisper about an ancient pharaoh named Kha'reth who was sealed away long ago."
    })

    local choice = ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "If you're brave enough to investigate, you should speak to the Nomad Elder at the oasis. He knows the old stories better than anyone.",
        choices = {
            { id = "accept", text = "I'll look into it." },
            { id = "decline", text = "That sounds too dangerous." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Desert Merchant",
            text = "Good luck out there. The elder's camp is at the oasis. And remember the name - Kha'reth. You'll need it."
        })
    else
        ctx:show_dialogue({
            speaker = "Desert Merchant",
            text = "Can't blame you. But if you change your mind, I'll be here."
        })
    end
end

-- ============================================================================
-- Progress Dialogue (dispatches based on current objective)
-- ============================================================================

function show_progress_dialogue(ctx)
    local merchant = ctx:get_objective_progress("talk_merchant")
    local elder = ctx:get_objective_progress("talk_elder")
    local researcher = ctx:get_objective_progress("talk_researcher")
    local hermit = ctx:get_objective_progress("talk_hermit")

    -- Nomad Elder dialogue
    if merchant.current >= merchant.target and elder.current < elder.target then
        show_elder_dialogue(ctx)
        return
    end

    -- Tomb Researcher dialogue
    if elder.current >= elder.target and researcher.current < researcher.target then
        show_researcher_dialogue(ctx)
        return
    end

    -- Desert Hermit dialogue (riddle)
    if researcher.current >= researcher.target and hermit.current < hermit.target then
        show_hermit_dialogue(ctx)
        return
    end

    -- Default progress reminder
    ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "Still investigating? Keep talking to the people who know this land."
    })
end

-- ============================================================================
-- Nomad Elder — tells the legend, clue: "three stars of the southern sky"
-- ============================================================================

function show_elder_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Nomad Elder",
        text = "Sit, young one. You ask about the pyramid? Then I will tell you of Kha'reth."
    })

    ctx:show_dialogue({
        speaker = "Nomad Elder",
        text = "He was a pharaoh who enslaved his people to build a tomb that would grant him eternal life. But the magic he sought was dark and corrupting."
    })

    ctx:show_dialogue({
        speaker = "Nomad Elder",
        text = "When his priests saw what he had become, they sealed him inside. The binding was powerful - tied to the three stars of the southern sky."
    })

    ctx:show_dialogue({
        speaker = "Nomad Elder",
        text = "But seals weaken with time. If the chanting has returned... the binding may be failing. Seek the researcher near the pyramid - she has been studying the inscriptions."
    })
end

-- ============================================================================
-- Tomb Researcher — clue: "blood of the scorpion"
-- ============================================================================

function show_researcher_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Tomb Researcher",
        text = "You've spoken to the elder? Good. Then you know what we're dealing with."
    })

    ctx:show_dialogue({
        speaker = "Tomb Researcher",
        text = "I've been translating the inscriptions on these walls for months. They describe a ritual that Kha'reth performed - one that required the blood of the scorpion."
    })

    ctx:show_dialogue({
        speaker = "Tomb Researcher",
        text = "Others have gone inside to investigate. None of them came back. There's a locked door deep within the pyramid that no one has been able to open."
    })

    ctx:show_dialogue({
        speaker = "Tomb Researcher",
        text = "If you're truly going after this... there's a hermit who lives in a hidden house out in the desert. He's spent decades studying Kha'reth. He may know how to get past that door."
    })
end

-- ============================================================================
-- Desert Hermit — the riddle puzzle
-- ============================================================================

function show_hermit_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Desert Hermit",
        text = "So... someone finally comes seeking the truth about Kha'reth. I've waited a long time for this."
    })

    ctx:show_dialogue({
        speaker = "Desert Hermit",
        text = "I have the book that holds the key - literally. But I cannot give it to just anyone. You must prove you understand the story."
    })

    ctx:show_dialogue({
        speaker = "Desert Hermit",
        text = "The book asks three questions. Answer them from what you've learned, and the key is yours."
    })

    -- Riddle Question 1: Name of the cursed one
    local q1 = ctx:show_dialogue({
        speaker = "Ancient Book",
        text = "Speak the name of the cursed one.",
        choices = {
            { id = "khareth", text = "Kha'reth" },
            { id = "wrong1a", text = "Osirath" },
            { id = "wrong1b", text = "Amenhotep" }
        }
    })

    if q1 ~= "khareth" then
        show_riddle_failure(ctx)
        return
    end

    -- Riddle Question 2: The binding above
    local q2 = ctx:show_dialogue({
        speaker = "Ancient Book",
        text = "Name the binding above.",
        choices = {
            { id = "wrong2a", text = "The light of the sun" },
            { id = "three_stars", text = "Three stars of the southern sky" },
            { id = "wrong2b", text = "The desert winds" }
        }
    })

    if q2 ~= "three_stars" then
        show_riddle_failure(ctx)
        return
    end

    -- Riddle Question 3: The price below
    local q3 = ctx:show_dialogue({
        speaker = "Ancient Book",
        text = "Name the price below.",
        choices = {
            { id = "wrong3a", text = "Tears of the fallen" },
            { id = "wrong3b", text = "Gold of the kingdom" },
            { id = "scorpion_blood", text = "Blood of the scorpion" }
        }
    })

    if q3 ~= "scorpion_blood" then
        show_riddle_failure(ctx)
        return
    end

    -- All three correct!
    ctx:show_dialogue({
        speaker = "Desert Hermit",
        text = "The book trembles... the pages glow with ancient light..."
    })

    ctx:give_item("pharaohs_key", 1)

    ctx:show_dialogue({
        speaker = "Desert Hermit",
        text = "The Pharaoh's Key. It will open the sealed door within the pyramid. But be warned - what lies beyond has had millennia to grow in power."
    })

    ctx:show_dialogue({
        speaker = "Desert Hermit",
        text = "Go to the pyramid. Find the locked door deep inside. And may the stars protect you from what sleeps below."
    })
end

function show_riddle_failure(ctx)
    ctx:show_dialogue({
        speaker = "Desert Hermit",
        text = "That's not right. The book snaps shut. Perhaps you should speak to more people and learn the full story before trying again."
    })
end

-- ============================================================================
-- Quest Complete
-- ============================================================================

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "You... you went down there? And survived?!"
    })

    ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "The chanting has stopped. The desert feels lighter somehow. Whatever you did down there... thank you."
    })

    ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "But I wonder... was Kha'reth the only thing sealed beneath these sands? The elder spoke of others..."
    })

    ctx:complete_quest()
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "The pyramid is quiet now, thanks to you. But sometimes I still feel something watching from beneath the sand..."
    })
end

-- ============================================================================
-- Objective Progress Notifications
-- ============================================================================

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "defeat_khareth" and new_count == 1 then
        ctx:show_notification("Kha'reth has been defeated! Return to the Desert Merchant.")
    end
end
```

**Step 3: Commit**

```bash
git add rust-server/data/scripts/quests/desert_pharaoh/pharaohs_curse.lua
git commit -m "feat: add Pharaoh's Curse Lua quest script with riddle puzzle"
```

---

### Task 7: Pharaoh Boss State Machine

This is the core new Rust code. Create a new boss module for the stationary caster pattern, separate from the Desert Wurm's `BossState`.

**Files:**
- Create: `rust-server/src/pharaoh_boss.rs`
- Modify: `rust-server/src/main.rs` (add `mod pharaoh_boss;`)

**Step 1: Create the pharaoh boss state machine**

Create `rust-server/src/pharaoh_boss.rs`. This module follows the same pattern as `rust-server/src/boss.rs` but implements a stationary caster instead of a digging wurm.

Key differences from `BossState`:
- No movement states (Surface/Submerging/Digging/Emerging) — pharaoh is always stationary
- `PharaohState` enum: `Active` or `Dead`
- Fires ranged projectiles at closest player on a timer
- Spawns melee minions at arena edges on a timer
- Phase 3 adds arena shrink (AoE damage on outer tiles)

```rust
use crate::boss::{BossEvent, BossPhase};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PHASE1_PROJECTILE_INTERVAL: u64 = 2000;
const PHASE2_PROJECTILE_INTERVAL: u64 = 1500;
const PHASE3_PROJECTILE_INTERVAL: u64 = 1000;

const PHASE1_MINION_INTERVAL: u64 = 15000;
const PHASE2_MINION_INTERVAL: u64 = 12000;
const PHASE3_MINION_INTERVAL: u64 = 10000;

const PHASE1_MINION_COUNT: u32 = 2;
const PHASE2_MINION_COUNT: u32 = 3;
const PHASE3_MINION_COUNT: u32 = 4;

const PHASE1_PROJECTILE_DAMAGE: i32 = 8;
const PHASE3_PROJECTILE_DAMAGE: i32 = 12;

const ARENA_SHRINK_DAMAGE: i32 = 5;
const ARENA_SHRINK_WARNING_MS: u64 = 2000;
const ARENA_SHRINK_INTERVAL: u64 = 20000;

const MAX_MINIONS: u32 = 8;

// ---------------------------------------------------------------------------
// Pharaoh state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum PharaohState {
    Active,
    Dead,
}

// ---------------------------------------------------------------------------
// Phase config
// ---------------------------------------------------------------------------

struct PhaseConfig {
    projectile_interval: u64,
    projectile_damage: i32,
    minion_interval: u64,
    minion_count: u32,
    minion_prototype: &'static str,
    arena_shrink: bool,
}

fn phase_config(phase: &BossPhase) -> PhaseConfig {
    match phase {
        BossPhase::Hunt => PhaseConfig {
            projectile_interval: PHASE1_PROJECTILE_INTERVAL,
            projectile_damage: PHASE1_PROJECTILE_DAMAGE,
            minion_interval: PHASE1_MINION_INTERVAL,
            minion_count: PHASE1_MINION_COUNT,
            minion_prototype: "pharaoh_mummy",
            arena_shrink: false,
        },
        BossPhase::Storm => PhaseConfig {
            projectile_interval: PHASE2_PROJECTILE_INTERVAL,
            projectile_damage: PHASE1_PROJECTILE_DAMAGE,
            minion_interval: PHASE2_MINION_INTERVAL,
            minion_count: PHASE2_MINION_COUNT,
            minion_prototype: "pharaoh_skeleton",
            arena_shrink: false,
        },
        BossPhase::Frenzy => PhaseConfig {
            projectile_interval: PHASE3_PROJECTILE_INTERVAL,
            projectile_damage: PHASE3_PROJECTILE_DAMAGE,
            minion_interval: PHASE3_MINION_INTERVAL,
            minion_count: PHASE3_MINION_COUNT,
            // Mix: alternates between mummy and skeleton based on counter
            minion_prototype: "pharaoh_mummy",
            arena_shrink: true,
        },
    }
}

// ---------------------------------------------------------------------------
// Pharaoh boss state per-instance
// ---------------------------------------------------------------------------

pub struct PharaohBossState {
    pub instance_id: String,
    pub boss_npc_id: String,
    pub phase: BossPhase,
    pub state: PharaohState,
    pub boss_hp: i32,
    pub boss_max_hp: i32,
    pub boss_x: i32,
    pub boss_y: i32,
    pub map_width: i32,
    pub map_height: i32,
    pub last_projectile_time: u64,
    pub last_minion_spawn_time: u64,
    pub last_arena_shrink_time: u64,
    pub minion_counter: u32,
    pub live_minion_count: u32,
    pub player_ids: Vec<String>,
    pub death_time: u64,
    pub countdown_sent: u8,
    pub damage_dealers: std::collections::HashSet<String>,
    pub arena_shrink_layer: i32, // how many tiles inward the damage zone extends
}

impl PharaohBossState {
    pub fn new(
        instance_id: String,
        boss_npc_id: String,
        boss_hp: i32,
        boss_max_hp: i32,
        boss_x: i32,
        boss_y: i32,
        map_width: i32,
        map_height: i32,
        current_time: u64,
    ) -> Self {
        Self {
            instance_id,
            boss_npc_id,
            phase: BossPhase::Hunt,
            state: PharaohState::Active,
            boss_hp,
            boss_max_hp,
            boss_x,
            boss_y,
            map_width,
            map_height,
            last_projectile_time: current_time,
            last_minion_spawn_time: current_time,
            last_arena_shrink_time: current_time,
            minion_counter: 0,
            live_minion_count: 0,
            player_ids: Vec::new(),
            death_time: 0,
            countdown_sent: 0,
            damage_dealers: std::collections::HashSet::new(),
            arena_shrink_layer: 0,
        }
    }

    pub fn is_dead(&self) -> bool {
        self.state == PharaohState::Dead
    }

    pub fn add_player(&mut self, player_id: String) {
        if !self.player_ids.contains(&player_id) {
            self.player_ids.push(player_id);
        }
    }

    /// Main tick — returns events for GameRoom to process.
    /// Called every server tick (50ms).
    pub fn tick(&mut self, current_time: u64) -> Vec<BossEvent> {
        if self.state == PharaohState::Dead {
            return vec![];
        }

        let mut events = Vec::new();

        // Update phase based on HP
        let old_phase = self.phase.clone();
        self.phase = if self.boss_hp > (self.boss_max_hp * 2 / 3) {
            BossPhase::Hunt
        } else if self.boss_hp > (self.boss_max_hp / 3) {
            BossPhase::Storm
        } else {
            BossPhase::Frenzy
        };

        if self.phase != old_phase {
            let phase_name = match &self.phase {
                BossPhase::Hunt => "Awakening",
                BossPhase::Storm => "Wrath",
                BossPhase::Frenzy => "Desperation",
            };
            events.push(BossEvent::Announcement {
                instance_id: self.instance_id.clone(),
                message: format!("Kha'reth enters phase: {}!", phase_name),
            });
        }

        let config = phase_config(&self.phase);

        // --- Ranged projectile attack ---
        if current_time - self.last_projectile_time >= config.projectile_interval {
            self.last_projectile_time = current_time;

            // Fire projectile at boss position (handled by boss_tick to target closest player)
            events.push(BossEvent::AoeWarning {
                instance_id: self.instance_id.clone(),
                tiles: vec![], // placeholder — boss_tick resolves actual target
                delay_ms: 0,
                effect: format!("pharaoh_projectile:{}", config.projectile_damage),
            });
        }

        // --- Minion spawning ---
        if current_time - self.last_minion_spawn_time >= config.minion_interval
            && self.live_minion_count < MAX_MINIONS
        {
            self.last_minion_spawn_time = current_time;

            for i in 0..config.minion_count {
                if self.live_minion_count >= MAX_MINIONS {
                    break;
                }
                self.minion_counter += 1;
                let npc_id = format!("pharaoh_minion_{}", self.minion_counter);

                // Spawn at arena edges — distribute around perimeter
                let (spawn_x, spawn_y) = self.get_edge_spawn(i);

                let prototype = if self.phase == BossPhase::Frenzy && self.minion_counter % 2 == 0 {
                    "pharaoh_skeleton"
                } else {
                    config.minion_prototype
                };

                events.push(BossEvent::SpawnMinion {
                    instance_id: self.instance_id.clone(),
                    npc_id: npc_id.clone(),
                    x: spawn_x,
                    y: spawn_y,
                });

                self.live_minion_count += 1;
            }
        }

        // --- Arena shrink (Phase 3 only) ---
        if config.arena_shrink
            && current_time - self.last_arena_shrink_time >= ARENA_SHRINK_INTERVAL
        {
            self.last_arena_shrink_time = current_time;
            self.arena_shrink_layer += 1;

            let damage_tiles = self.get_shrink_tiles();

            if !damage_tiles.is_empty() {
                events.push(BossEvent::AoeWarning {
                    instance_id: self.instance_id.clone(),
                    tiles: damage_tiles.clone(),
                    delay_ms: ARENA_SHRINK_WARNING_MS,
                    effect: "cursed_sand".to_string(),
                });
                events.push(BossEvent::AoeDamage {
                    instance_id: self.instance_id.clone(),
                    tiles: damage_tiles,
                    damage: ARENA_SHRINK_DAMAGE,
                    effect: "cursed_sand".to_string(),
                });
            }
        }

        // State update
        events.push(BossEvent::StateUpdate {
            instance_id: self.instance_id.clone(),
            boss_hp: self.boss_hp,
            boss_max_hp: self.boss_max_hp,
            phase: match &self.phase {
                BossPhase::Hunt => "awakening".to_string(),
                BossPhase::Storm => "wrath".to_string(),
                BossPhase::Frenzy => "desperation".to_string(),
            },
            wurm_state: "stationary".to_string(),
        });

        events
    }

    /// Get spawn position at arena edge for minion index
    fn get_edge_spawn(&self, index: u32) -> (i32, i32) {
        let margin = 1;
        let perimeter_positions = vec![
            (margin, margin),                                    // top-left
            (self.map_width - margin - 1, margin),               // top-right
            (margin, self.map_height - margin - 1),              // bottom-left
            (self.map_width - margin - 1, self.map_height - margin - 1), // bottom-right
            (self.map_width / 2, margin),                        // top-center
            (margin, self.map_height / 2),                       // left-center
            (self.map_width - margin - 1, self.map_height / 2),  // right-center
            (self.map_width / 2, self.map_height - margin - 1),  // bottom-center
        ];
        let idx = (self.minion_counter as usize + index as usize) % perimeter_positions.len();
        perimeter_positions[idx]
    }

    /// Get tiles affected by arena shrink
    fn get_shrink_tiles(&self) -> Vec<(i32, i32)> {
        let mut tiles = Vec::new();
        let layer = self.arena_shrink_layer;

        for x in 0..self.map_width {
            for y in 0..self.map_height {
                if x < layer || x >= self.map_width - layer
                    || y < layer || y >= self.map_height - layer
                {
                    tiles.push((x, y));
                }
            }
        }
        tiles
    }

    pub fn on_minion_died(&mut self) {
        self.live_minion_count = self.live_minion_count.saturating_sub(1);
    }
}
```

**Step 2: Register the module**

Add `pub mod pharaoh_boss;` to `rust-server/src/main.rs` near the existing `pub mod boss;` declaration.

**Step 3: Verify**

Run: `cd rust-server && cargo check 2>&1 | head -20`
Expected: Compiles (existing warnings OK, no new errors)

**Step 4: Commit**

```bash
git add rust-server/src/pharaoh_boss.rs rust-server/src/main.rs
git commit -m "feat: add PharaohBossState - stationary caster boss state machine"
```

---

### Task 8: Integrate Pharaoh Boss into Boss Tick Pipeline

**Files:**
- Modify: `rust-server/src/game/boss_tick.rs`
- Modify: `rust-server/src/game.rs` (add `pharaoh_boss_states` field to `GameRoom`)

**Step 1: Add `pharaoh_boss_states` to GameRoom**

In `rust-server/src/game.rs`, find the `boss_states` field in the `GameRoom` struct and add a parallel field:

```rust
pub pharaoh_boss_states: RwLock<HashMap<String, crate::pharaoh_boss::PharaohBossState>>,
```

Initialize it in `GameRoom::new()` alongside `boss_states`:

```rust
pharaoh_boss_states: RwLock::new(HashMap::new()),
```

**Step 2: Add pharaoh boss tick processing**

In `rust-server/src/game/boss_tick.rs`, add a new method `process_pharaoh_boss_tick` that follows the same pattern as `process_boss_tick` (lines 12-87) but iterates `pharaoh_boss_states` instead.

Key differences in event handling:
- `SpawnMinion`: use the prototype from the event's `npc_id` prefix to determine `pharaoh_mummy` vs `pharaoh_skeleton` (or pass prototype through a new mechanism)
- `AoeWarning` with `effect` starting with `"pharaoh_projectile:"`: resolve the closest player to the boss, fire a projectile spell at them using the existing spell/projectile system
- Minion death tracking: call `pharaoh_boss.on_minion_died()` when a `pharaoh_minion_*` NPC dies

**Step 3: Add `start_pharaoh_boss_session` method**

Follow the pattern of `start_boss_session` (lines 833-863):

```rust
pub async fn start_pharaoh_boss_session(
    &self,
    instance_id: &str,
    boss_npc_id: &str,
    boss_hp: i32,
    boss_max_hp: i32,
    boss_x: i32,
    boss_y: i32,
    map_width: i32,
    map_height: i32,
    current_time: u64,
) {
    let boss = crate::pharaoh_boss::PharaohBossState::new(
        instance_id.to_string(),
        boss_npc_id.to_string(),
        boss_hp,
        boss_max_hp,
        boss_x,
        boss_y,
        map_width,
        map_height,
        current_time,
    );
    let mut states = self.pharaoh_boss_states.write().await;
    states.insert(instance_id.to_string(), boss);
    tracing::info!(
        "Pharaoh boss session started in instance {} (npc: {})",
        instance_id,
        boss_npc_id
    );
}
```

**Step 4: Call `process_pharaoh_boss_tick` from the main tick loop**

Find where `process_boss_tick` is called in the main game tick loop (in `rust-server/src/game.rs`) and add `self.process_pharaoh_boss_tick(current_time).await;` right after it.

**Step 5: Verify**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 6: Commit**

```bash
git add rust-server/src/game.rs rust-server/src/game/boss_tick.rs
git commit -m "feat: integrate pharaoh boss into game tick pipeline"
```

---

### Task 9: Pharaoh Boss Instance Entry Point

**Files:**
- Modify: `rust-server/src/main.rs`

**Step 1: Add the pharaoh boss map constant**

In `rust-server/src/game/boss_tick.rs`, add:

```rust
pub const PHARAOH_BOSS_MAP_ID: &str = "pyramid_tomb";
```

**Step 2: Add instance entry hook**

In `rust-server/src/main.rs`, find the Desert Wurm boss instance check (around line 4509):

```rust
if interior.id == crate::game::boss_tick::BOSS_MAP_ID {
```

Add a similar block after it for the pharaoh boss:

```rust
// Start or join pharaoh boss session if entering pyramid tomb
if interior.id == crate::game::boss_tick::PHARAOH_BOSS_MAP_ID {
    let ct = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    if room.has_pharaoh_boss_session(&instance.id).await {
        room.add_pharaoh_boss_player(&instance.id, player_id).await;
    } else {
        let npcs = instance.npcs.read().await;
        if let Some(boss_npc) = npcs.values().find(|n| n.prototype_id == "khareth_pharaoh") {
            room.start_pharaoh_boss_session(
                &instance.id,
                &boss_npc.id,
                boss_npc.hp,
                boss_npc.max_hp,
                boss_npc.x,
                boss_npc.y,
                instance.map_width as i32,
                instance.map_height as i32,
                ct,
            )
            .await;
        }
    }
}
```

**Step 3: Add `has_pharaoh_boss_session` and `add_pharaoh_boss_player` helpers**

In `rust-server/src/game/boss_tick.rs`, add these following the pattern of `has_boss_session` (line 814) and `add_boss_player` (line 820):

```rust
pub async fn has_pharaoh_boss_session(&self, instance_id: &str) -> bool {
    let states = self.pharaoh_boss_states.read().await;
    states.contains_key(instance_id)
}

pub async fn add_pharaoh_boss_player(&self, instance_id: &str, player_id: &str) {
    let mut states = self.pharaoh_boss_states.write().await;
    if let Some(boss) = states.get_mut(instance_id) {
        boss.add_player(player_id.to_string());
    }
}
```

**Step 4: Verify**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 5: Commit**

```bash
git add rust-server/src/main.rs rust-server/src/game/boss_tick.rs
git commit -m "feat: add pyramid tomb instance entry point for pharaoh boss"
```

---

### Task 10: Pharaoh Projectile Attack Implementation

The pharaoh's ranged attack needs to fire a projectile spell at the closest player. This task implements the projectile resolution in the boss tick handler.

**Files:**
- Modify: `rust-server/src/game/boss_tick.rs`

**Step 1: Implement projectile targeting in `handle_pharaoh_boss_event`**

When processing the `AoeWarning` event with `effect` starting with `"pharaoh_projectile:"`:

1. Get all player positions in the instance
2. Find the closest player to the boss
3. Fire a projectile from boss position to that player's position
4. Apply damage on hit

This should reuse the existing spell/projectile system. Search for how spells are fired at NPCs/players to find the exact mechanism.

> **Note to implementer:** Search for `ProjectileSpell` or `fire_projectile` or `SpellCast` in the codebase to find the existing projectile system. The pharaoh should fire using that same system, dealing the damage specified in the effect string.

**Step 2: Verify**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 3: Commit**

```bash
git add rust-server/src/game/boss_tick.rs
git commit -m "feat: implement pharaoh ranged projectile attack"
```

---

### Task 11: Interior Map — Pyramid Tomb

**Files:**
- Create: `rust-server/maps/interiors/pyramid_tomb.json`

**Step 1: Create the boss arena map**

The map needs to follow the same JSON format as existing interior maps (e.g., `rust-server/maps/interiors/desert_boss_cave.json`). Read that file to understand the exact format.

Create `rust-server/maps/interiors/pyramid_tomb.json` with:
- A ~20x20 tile tomb chamber
- Instance type: `private` (solo boss fight)
- One spawn point (entrance)
- One portal (exit back to pyramid interior)
- One NPC spawn: `khareth_pharaoh` at the center of the room
- Collision walls around the perimeter

> **Note to implementer:** Copy the structure from `desert_boss_cave.json` and adapt the dimensions, NPC spawns, and portals. The user will refine the map layout in the map editor later.

**Step 2: Commit**

```bash
git add rust-server/maps/interiors/pyramid_tomb.json
git commit -m "feat: add pyramid tomb interior map for pharaoh boss"
```

---

### Task 12: Pharaoh Minion Death Handling

**Files:**
- Modify: `rust-server/src/game/boss_tick.rs`
- Modify: `rust-server/src/game.rs` (where NPC death is processed)

**Step 1: Track pharaoh minion deaths**

Find where `check_boss_minion_death` is called (when an NPC dies in an instance). Add a parallel check for pharaoh minions:

```rust
// In the NPC death handler, after check_boss_minion_death:
if npc_id.starts_with("pharaoh_minion_") {
    self.check_pharaoh_minion_death(npc_id, instance_id, current_time).await;
}
```

**Step 2: Implement `check_pharaoh_minion_death`**

```rust
pub(in crate::game) async fn check_pharaoh_minion_death(
    &self,
    npc_id: &str,
    instance_id: &str,
    _current_time: u64,
) {
    if !npc_id.starts_with("pharaoh_minion_") {
        return;
    }
    let mut states = self.pharaoh_boss_states.write().await;
    if let Some(boss) = states.get_mut(instance_id) {
        boss.on_minion_died();
    }
}
```

**Step 3: Verify**

Run: `cd rust-server && cargo check 2>&1 | head -20`

**Step 4: Commit**

```bash
git add rust-server/src/game.rs rust-server/src/game/boss_tick.rs
git commit -m "feat: handle pharaoh minion death tracking"
```

---

### Task 13: End-to-End Verification

**Step 1: Full compilation check**

Run: `cd rust-server && cargo build 2>&1 | tail -20`
Expected: Compiles successfully

**Step 2: Verify all data files load**

Run the server briefly and check logs for any loading errors related to the new quest, NPCs, or entities:

```bash
cd rust-server && cargo run 2>&1 | head -50
```

Look for:
- Quest `pharaohs_curse` loaded
- NPC entities `desert_merchant_quest`, `nomad_elder`, `tomb_researcher`, `desert_hermit` loaded
- Monster entities `khareth_pharaoh`, `pharaoh_mummy`, `pharaoh_skeleton` loaded
- Item `pharaohs_key` loaded
- Interior `pyramid_tomb` loaded

**Step 3: Commit any fixes**

---

## Summary

| Task | Description | Type |
|------|-------------|------|
| 1 | Pharaoh's Key quest item | Data |
| 2 | 4 quest NPC entity definitions | Data |
| 3 | Boss + minion entity definitions | Data |
| 4 | Quest TOML definition | Data |
| 5 | Quest location entry | Data |
| 6 | Lua quest script (dialogue + riddle) | Script |
| 7 | PharaohBossState state machine | Rust |
| 8 | Integrate into boss tick pipeline | Rust |
| 9 | Instance entry point | Rust |
| 10 | Projectile attack implementation | Rust |
| 11 | Interior map (pyramid tomb) | Map |
| 12 | Minion death handling | Rust |
| 13 | End-to-end verification | Testing |

Tasks 1-6 are data/script only (no Rust changes). Tasks 7-12 are the Rust implementation. Task 13 is verification.
