# "A Ghastly Contraption" — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a haunted house quest where the player solves a candle puzzle, passes a ghost's "prove you're alive" dialogue challenge, fights a poltergeist, and receives the Leather Attractor.

**Architecture:** New quest data (TOML + Lua script) using the existing quest system. Two quest-giver NPCs (Oddwick and Barnaby) both reference the same quest so the Lua script handles both interactions. The script uses objective progress to determine which NPC the player is talking to and branches dialogue accordingly. One hostile monster entity (poltergeist) in the basement. No new Rust code needed — all data-driven.

**Tech Stack:** TOML (quest + entity data), Lua (quest script), JSON (map entity spawns)

**Design doc:** `docs/plans/2026-03-28-ghastly-contraption-quest-design.md`

---

### Task 1: Quest Items

**Files:**
- Modify: `rust-server/data/items/tools.toml`

**Step 1: Add quest item definitions**

Append to the end of `rust-server/data/items/tools.toml`:

```toml
# =============================================================================
# A Ghastly Contraption - Quest Items
# =============================================================================

[tinderbox]
display_name = "Tinderbox"
sprite = "tinderbox"
description = "A small box for striking a flame. Smells faintly of sulphur."
category = "quest"
max_stack = 1
base_price = 0
sellable = false

[basement_key]
display_name = "Basement Key"
sprite = "basement_key"
description = "A tarnished iron key. Barnaby insists it's a lucky charm."
category = "quest"
max_stack = 1
base_price = 0
sellable = false

[haunted_ectoplasm]
display_name = "Haunted Ectoplasm"
sprite = "haunted_ectoplasm"
description = "A glowing, gelatinous substance pulsing with spectral energy."
category = "quest"
max_stack = 1
base_price = 0
sellable = false

[spectral_coil_quest]
display_name = "Spectral Coil"
sprite = "spectral_coil"
description = "A coiled wire that hums with ghostly resonance."
category = "quest"
max_stack = 1
base_price = 0
sellable = false
```

**Step 2: Verify the server loads the items**

Run: `cd rust-server && cargo check 2>&1 | head -5`
Expected: No new errors (existing warnings OK)

**Step 3: Commit**

```bash
git add rust-server/data/items/tools.toml
git commit -m "feat: add quest items for A Ghastly Contraption"
```

---

### Task 2: NPC Entity Definitions

**Files:**
- Create: `rust-server/data/entities/npcs/ghastly_contraption_quest.toml`

**Step 1: Create the 2 quest NPC definitions**

Create `rust-server/data/entities/npcs/ghastly_contraption_quest.toml`:

```toml
# =============================================================================
# A Ghastly Contraption Quest NPCs
# =============================================================================

# Professor Oddwick - eccentric inventor, quest giver
[prof_oddwick]
display_name = "Professor Oddwick"
sprite = "wise_man"
animation_type = "humanoid"
description = "An eccentric inventor who bought a haunted house at auction. He regrets this decision."

[prof_oddwick.stats]
max_hp = 100
damage = 0
attack_range = 0
aggro_range = 0
chase_range = 0
move_cooldown_ms = 600
attack_cooldown_ms = 0
respawn_time_ms = 0

[prof_oddwick.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[prof_oddwick.behaviors]
hostile = false
quest_giver = true
wander_enabled = false

[prof_oddwick.quest_giver]
available_quests = ["ghastly_contraption"]

[prof_oddwick.speech]
radius = 5
interval_min_ms = 25000
interval_max_ms = 45000
messages = [
    "Almost got it... just need to reverse the polarity...",
    "Did you hear that? ...Probably nothing. Definitely nothing.",
    "I really should have read the property listing more carefully.",
]

[prof_oddwick.dialogue]
greeting = "Oh! You're alive! I mean — of course you are. Welcome to my... home. Such as it is."

# --------------------------------------------------------

# Barnaby - friendly ghost, doesn't know he's dead
[barnaby_ghost]
display_name = "Barnaby"
sprite = "ghost"
animation_type = "standard"
description = "A cheerful ghost who doesn't realize he's dead."

[barnaby_ghost.stats]
max_hp = 100
damage = 0
attack_range = 0
aggro_range = 0
chase_range = 0
move_cooldown_ms = 600
attack_cooldown_ms = 0
respawn_time_ms = 0

[barnaby_ghost.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[barnaby_ghost.behaviors]
hostile = false
quest_giver = true
wander_enabled = true
wander_radius = 3
wander_pause_min_ms = 5000
wander_pause_max_ms = 10000

[barnaby_ghost.quest_giver]
available_quests = ["ghastly_contraption"]

[barnaby_ghost.speech]
radius = 5
interval_min_ms = 20000
interval_max_ms = 40000
messages = [
    "I love what they've done with the place. Very... dusty.",
    "Has anyone seen my body? I seem to have misplaced it.",
    "I've been here for years and the landlord STILL hasn't fixed the draft.",
]

[barnaby_ghost.dialogue]
greeting = "Oh! A visitor! How exciting! ...Wait. Are you alive?"
```

**Step 2: Verify the server loads the NPCs**

Run: `cd rust-server && cargo check 2>&1 | head -5`
Expected: No new errors

**Step 3: Commit**

```bash
git add rust-server/data/entities/npcs/ghastly_contraption_quest.toml
git commit -m "feat: add Oddwick and Barnaby NPC definitions"
```

---

### Task 3: Poltergeist Monster Entity

**Files:**
- Create: `rust-server/data/entities/monsters/haunted_poltergeist.toml`

**Step 1: Create the poltergeist monster definition**

Create `rust-server/data/entities/monsters/haunted_poltergeist.toml`:

```toml
# =============================================================================
# A Ghastly Contraption - Basement Boss
# =============================================================================

[haunted_poltergeist]
display_name = "Enraged Poltergeist"
sprite = "ghost"
animation_type = "standard"
description = "The source of the hauntings. Furious and dangerous."

[haunted_poltergeist.stats]
level = 28
max_hp = 150
damage = 8
attack_bonus = 10
defence_bonus = 5
attack_range = 1
aggro_range = 8
chase_range = 10
move_cooldown_ms = 500
attack_cooldown_ms = 2000
respawn_time_ms = 0

[haunted_poltergeist.rewards]
exp_base = 300
gold_min = 50
gold_max = 150

[[haunted_poltergeist.loot]]
item_id = "haunted_ectoplasm"
drop_chance = 1.0
quantity_min = 1
quantity_max = 1

[[haunted_poltergeist.loot]]
item_id = "spectral_coil_quest"
drop_chance = 1.0
quantity_min = 1
quantity_max = 1

[haunted_poltergeist.behaviors]
hostile = true
wander_enabled = true
wander_radius = 3
wander_pause_min_ms = 3000
wander_pause_max_ms = 6000
```

**Step 2: Verify**

Run: `cd rust-server && cargo check 2>&1 | head -5`
Expected: No new errors

**Step 3: Commit**

```bash
git add rust-server/data/entities/monsters/haunted_poltergeist.toml
git commit -m "feat: add Enraged Poltergeist monster for haunted house"
```

---

### Task 4: Quest TOML Definition

**Files:**
- Create: `rust-server/data/quests/ghastly_contraption/ghastly_contraption.toml`

**Step 1: Create the quest directory and definition**

```bash
mkdir -p rust-server/data/quests/ghastly_contraption
```

Create `rust-server/data/quests/ghastly_contraption/ghastly_contraption.toml`:

```toml
[quest]
id = "ghastly_contraption"
name = "A Ghastly Contraption"
description = "Help Professor Oddwick clear the ghosts from his haunted house and build a device that recovers projectiles."
giver_npc = "prof_oddwick"
level_required = 20
repeatable = false
lua_script = "ghastly_contraption/ghastly_contraption.lua"

[quest.chain]
# Standalone quest — no chain

[[quest.objectives]]
id = "talk_oddwick"
type = "talk_to"
target = "prof_oddwick"
description = "Talk to Professor Oddwick"

[[quest.objectives]]
id = "find_tinderbox"
type = "collect_item"
target = "tinderbox"
count = 1
description = "Search the bookshelves for a tinderbox"
sequential = true

[[quest.objectives]]
id = "open_first_gate"
type = "reach_location"
target = "haunted_house_gate1"
count = 1
description = "Solve the candle puzzle to open the gate"
sequential = true

[[quest.objectives]]
id = "talk_barnaby"
type = "talk_to"
target = "barnaby_ghost"
description = "Convince Barnaby you're alive and get the basement key"
sequential = true

[[quest.objectives]]
id = "defeat_poltergeist"
type = "kill_monster"
target = "haunted_poltergeist"
count = 1
description = "Defeat the Enraged Poltergeist in the basement"
sequential = true

[[quest.objectives]]
id = "collect_ectoplasm"
type = "collect_item"
target = "haunted_ectoplasm"
count = 1
description = "Collect the Haunted Ectoplasm"
sequential = true
consume = true

[[quest.objectives]]
id = "collect_coil"
type = "collect_item"
target = "spectral_coil_quest"
count = 1
description = "Collect the Spectral Coil"
sequential = true
consume = true

[[quest.objectives]]
id = "return_to_oddwick"
type = "talk_to"
target = "prof_oddwick"
description = "Bring the components back to Oddwick"
sequential = true

[quest.rewards]
exp = 500
gold = 100
[[quest.rewards.items]]
id = "leather_attractor"
count = 1

[quest.dialogue]
offer = "Oh! You're alive! Thank goodness. I could really use some help around here..."
accept = "Wonderful! First things first — we need to get past these gates. There's a candle mechanism..."
progress = "Still working on it? Don't give up! ...But also don't die. That would be inconvenient."
complete = "IT WORKS! Take it — the Leather Attractor! 60% projectile recovery rate! ...Give or take."
```

**Step 2: Add quest location for the gate puzzle**

Append to `rust-server/data/quest_locations.toml`:

```toml
# A Ghastly Contraption - first gate (candle puzzle completion zone)
[haunted_house_gate1]
x = 24
y = 30
radius = 3
```

Note: The x/y coordinates should be adjusted to match the actual gate position on the haunted house map. The reach_location objective will trigger when the player walks into this area after the candle puzzle is solved — the Lua script handles the actual puzzle logic, this just tracks that they got past it.

**Step 3: Verify**

Run: `cd rust-server && cargo check 2>&1 | head -5`
Expected: No new errors

**Step 4: Commit**

```bash
git add rust-server/data/quests/ghastly_contraption/
git add rust-server/data/quest_locations.toml
git commit -m "feat: add quest TOML definition for A Ghastly Contraption"
```

---

### Task 5: Lua Quest Script

**Files:**
- Create: `rust-server/data/scripts/quests/ghastly_contraption/ghastly_contraption.lua`

This is the main quest logic. The script handles all dialogue branching based on which NPC the player is interacting with (determined by checking objective progress).

**Step 1: Create the script directory**

```bash
mkdir -p rust-server/data/scripts/quests/ghastly_contraption
```

**Step 2: Write the Lua script**

Create `rust-server/data/scripts/quests/ghastly_contraption/ghastly_contraption.lua`:

```lua
-- A Ghastly Contraption
-- Player helps Professor Oddwick clear a haunted house and build the Leather Attractor.
--
-- NPC routing: Both Oddwick and Barnaby list this quest in available_quests.
-- The script checks objective progress to determine which NPC the player is
-- talking to and routes to the appropriate dialogue handler.

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        show_oddwick_offer(ctx)
        return
    end

    if quest_state == "completed" then
        show_post_complete(ctx)
        return
    end

    if quest_state == "ready_to_complete" then
        show_oddwick_build(ctx)
        return
    end

    -- in_progress: route based on objective state
    route_in_progress(ctx)
end

-- ============================================================================
-- NPC Routing (in_progress state)
-- ============================================================================

function route_in_progress(ctx)
    local tinderbox = ctx:get_objective_progress("find_tinderbox")
    local gate = ctx:get_objective_progress("open_first_gate")
    local barnaby = ctx:get_objective_progress("talk_barnaby")

    -- If tinderbox not found yet, player is talking to Oddwick for hints
    if tinderbox.current < tinderbox.target then
        show_oddwick_hint_tinderbox(ctx)
        return
    end

    -- If gate not opened yet, player has tinderbox — show candle puzzle
    if gate.current < gate.target then
        show_candle_puzzle(ctx)
        return
    end

    -- If Barnaby not convinced yet, this is the Barnaby interaction
    if barnaby.current < barnaby.target then
        show_barnaby_interrogation(ctx)
        return
    end

    -- Otherwise player is mid-quest talking to Oddwick
    show_oddwick_waiting(ctx)
end

-- ============================================================================
-- Step 1: Oddwick Offer
-- ============================================================================

function show_oddwick_offer(ctx)
    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Oh! A real person! You have no idea how glad I am to see you."
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "I bought this house at auction. A steal! ...Literally. The previous owner's ghost stole all the furniture."
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "I've been working on a device to neutralize the spectral energy, but I need components from the basement. Problem is, it's locked behind gates — and the basement is... well, haunted."
    })

    local choice = ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "I need someone brave — or foolish — to help me clear this place out. What do you say?",
        choices = {
            { id = "accept", text = "I'll help you out." },
            { id = "decline", text = "This sounds like your problem." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Professor Oddwick",
            text = "Wonderful! First, we need to get through the gate. There's a candle mechanism — the previous owner was eccentric."
        })
        ctx:show_dialogue({
            speaker = "Professor Oddwick",
            text = "I think the order was... skull candle first, then the tall one, then... red? No wait, I think the tall one was first. Or was skull second? Blast, I can't remember exactly."
        })
        ctx:show_dialogue({
            speaker = "Professor Oddwick",
            text = "You'll need a tinderbox to light them. I'm sure there's one around here somewhere — try the bookshelves."
        })
    else
        ctx:show_dialogue({
            speaker = "Professor Oddwick",
            text = "Fair enough. Can't blame you. But if you change your mind, I'll be here. Not like I can leave — the ghost took my carriage wheels too."
        })
    end
end

-- ============================================================================
-- Oddwick Hints (tinderbox not yet found)
-- ============================================================================

function show_oddwick_hint_tinderbox(ctx)
    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Still looking for the tinderbox? Try searching the bookshelves — I'm sure I saw one buried in there somewhere."
    })
end

-- ============================================================================
-- Step 2: Candle Puzzle
-- ============================================================================
-- Correct order: Skull → Tall → Red → Small
-- Oddwick's hint: "tall one first, then skull? Or skull second?"
-- (He swaps the first two — skull is actually first, tall is second)

function show_candle_puzzle(ctx)
    ctx:show_dialogue({
        speaker = "Narrator",
        text = "Four candles stand before the gate. Each is different — a skull-shaped candle, a tall taper, a red candle, and a small stubby one. They must be lit in the correct order."
    })

    -- Question 1: Which candle first?
    local q1 = ctx:show_dialogue({
        speaker = "Narrator",
        text = "Which candle do you light first?",
        choices = {
            { id = "skull", text = "The skull candle" },
            { id = "tall", text = "The tall taper" },
            { id = "red", text = "The red candle" },
            { id = "small", text = "The small stubby candle" }
        }
    })

    if q1 ~= "skull" then
        show_candle_failure(ctx)
        return
    end

    -- Question 2: Which candle second?
    local q2 = ctx:show_dialogue({
        speaker = "Narrator",
        text = "The skull candle flickers to life with an eerie green flame. Which candle next?",
        choices = {
            { id = "tall", text = "The tall taper" },
            { id = "red", text = "The red candle" },
            { id = "small", text = "The small stubby candle" }
        }
    })

    if q2 ~= "tall" then
        show_candle_failure(ctx)
        return
    end

    -- Question 3: Which candle third?
    local q3 = ctx:show_dialogue({
        speaker = "Narrator",
        text = "The tall taper ignites with a pale blue flame. Which candle next?",
        choices = {
            { id = "red", text = "The red candle" },
            { id = "small", text = "The small stubby candle" }
        }
    })

    if q3 ~= "red" then
        show_candle_failure(ctx)
        return
    end

    -- Final candle (auto — only one left)
    ctx:show_dialogue({
        speaker = "Narrator",
        text = "The red candle bursts into a warm orange flame. You light the last candle — the small stubby one sputters to life."
    })

    ctx:show_dialogue({
        speaker = "Narrator",
        text = "All four candles burn in unison. The gate groans... and slowly creaks open."
    })

    ctx:show_notification("The first gate has opened!")
end

function show_candle_failure(ctx)
    ctx:show_dialogue({
        speaker = "Narrator",
        text = "A cold wind howls through the room. All the candles snuff out at once. Somewhere in the house, a ghost laughs."
    })

    ctx:show_dialogue({
        speaker = "Narrator",
        text = "The candles reset. Perhaps a different order..."
    })
end

-- ============================================================================
-- Step 3: Barnaby's "Prove You're Alive" Interrogation
-- ============================================================================

function show_barnaby_interrogation(ctx)
    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Oh! A visitor! How exciting! ...Wait."
    })

    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Are you alive? Or are you one of THEM? I've had ghosts try to trick me before. Well, I think they were ghosts. Hard to tell these days."
    })

    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "I'm going to need you to prove you're alive. I have a very rigorous three-question test. Ready?"
    })

    -- Question 1: Do you breathe?
    local q1 = ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Question one: Do you breathe?",
        choices = {
            { id = "obviously", text = "Yes, obviously." },
            { id = "watch", text = "Watch me." },
            { id = "do_you", text = "Do YOU breathe?" }
        }
    })

    if q1 == "obviously" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "Hmm... that's EXACTLY what a ghost pretending to breathe would say. Suspicious."
        })
    elseif q1 == "watch" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "Oh! Your chest moves! ...Unless that's a trick. But I'll give you the benefit of the doubt."
        })
    elseif q1 == "do_you" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "Of course I do! I'm perfectly alive! ...Aren't I? Anyway, this is about YOU."
        })
    end

    -- Question 2: What's your favorite food?
    local q2 = ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Question two: What's your favorite food?",
        choices = {
            { id = "none", text = "I don't eat." },
            { id = "bread", text = "Bread and stew." },
            { id = "ecto", text = "Ectoplasm." }
        }
    })

    if q2 == "none" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "AHA! Ghost confirmed! ...Wait, you could just be on a diet. Hmm. Proceed."
        })
    elseif q2 == "bread" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "Ooh, bread and stew! That does sound like a living person thing. I miss stew. ...Do I miss stew? I can't remember."
        })
    elseif q2 == "ecto" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "See, I KNEW— wait, really? That's disgusting even for a ghost. ...Are you feeling alright?"
        })
    end

    -- Question 3: Can you walk through walls?
    local q3 = ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Final question: Can you walk through walls?",
        choices = {
            { id = "yes", text = "Yes." },
            { id = "door", text = "No, I used the door." },
            { id = "can_you", text = "Can YOU?" }
        }
    })

    if q3 == "yes" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "Ghost! I knew it! ...Actually, wait, you haven't floated through anything since you got here. I'll let it slide."
        })
    elseif q3 == "door" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "The DOOR? Nobody uses doors anymore! That's so old-fashioned. ...Maybe you ARE alive."
        })
    elseif q3 == "can_you" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "Can I? Watch this!"
        })
        ctx:show_dialogue({
            speaker = "Narrator",
            text = "Barnaby floats through a wall and back, looking very pleased with himself."
        })
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "See? Easy! ...Wait, can you not do that? Oh dear."
        })
    end

    -- All questions done — Barnaby is convinced (regardless of answers)
    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Alright, alright, I believe you. You're alive. How exciting! I haven't talked to a living person in... actually, how long have I been here?"
    })

    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "So what brings you to my humble haunted house? ...Well, I suppose it's technically the professor's now."
    })

    local key_choice = ctx:show_dialogue({
        speaker = "Player",
        text = "I need to get into the basement. Do you know where the key is?",
        choices = {
            { id = "ask", text = "Do you have a key?" }
        }
    })

    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "A key? You mean this old thing?"
    })

    ctx:show_dialogue({
        speaker = "Narrator",
        text = "Barnaby pulls a tarnished iron key from... somewhere. Best not to think about where a ghost keeps things."
    })

    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "I found it years ago. It's my lucky charm! ...Has it been lucky? I can't remember. I can't remember a lot of things, actually."
    })

    local take_choice = ctx:show_dialogue({
        speaker = "Barnaby",
        text = "You want it? I suppose I don't really NEED luck. I'm already dead. ...Wait. What did I just say?",
        choices = {
            { id = "take", text = "Thanks, Barnaby." },
            { id = "gentle", text = "You're a good ghost, Barnaby." }
        }
    })

    if take_choice == "gentle" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "A good... ghost? I'm a ghost? ...Huh. That actually explains a LOT."
        })
    else
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "Don't mention it! And be careful down there. I hear things. Angry things. ...Could also be the plumbing."
        })
    end

    ctx:give_item("basement_key", 1)
    ctx:show_notification("Received: Basement Key")
end

-- ============================================================================
-- Oddwick Waiting (mid-quest, after Barnaby)
-- ============================================================================

function show_oddwick_waiting(ctx)
    local poltergeist = ctx:get_objective_progress("defeat_poltergeist")
    local ectoplasm = ctx:get_objective_progress("collect_ectoplasm")
    local coil = ctx:get_objective_progress("collect_coil")

    if poltergeist.current < poltergeist.target then
        ctx:show_dialogue({
            speaker = "Professor Oddwick",
            text = "You have the basement key? Excellent! Get down there and deal with whatever's causing all this ruckus. I'll be here. Preparing. Definitely not hiding."
        })
    elseif ectoplasm.current < ectoplasm.target or coil.current < coil.target then
        ctx:show_dialogue({
            speaker = "Professor Oddwick",
            text = "You defeated it?! Did it drop anything? I need spectral components — ectoplasm, a coil, anything that glows ominously!"
        })
    else
        ctx:show_dialogue({
            speaker = "Professor Oddwick",
            text = "You have the components? Quick, hand them over! I've been preparing the assembly rig!"
        })
    end
end

-- ============================================================================
-- Step 5: Oddwick Build Sequence (quest completion)
-- ============================================================================

function show_oddwick_build(ctx)
    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "You got them! Haunted ectoplasm AND a spectral coil! Do you know how rare these are?"
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Well, I don't either, but they FEEL rare. Now hold still while I calibrate the ectoplasmic resonance matrix..."
    })

    -- First attempt: failure
    ctx:show_dialogue({
        speaker = "Narrator",
        text = "Oddwick connects the coil to a leather harness, pours the ectoplasm into a glass chamber, and starts cranking a handle. Sparks fly. The device rattles violently."
    })

    ctx:show_dialogue({
        speaker = "Narrator",
        text = "BANG! A small explosion rocks the table. Smoke fills the room."
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "...That wasn't supposed to happen."
    })

    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Is it supposed to be on fire?"
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "No, Barnaby. Thank you for your observation."
    })

    -- Second attempt: success
    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Right. Slight adjustment... reverse the polarity... carry the two..."
    })

    ctx:show_dialogue({
        speaker = "Narrator",
        text = "A soft hum fills the air. The device glows with a gentle, steady light. It's working."
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "IT WORKS! Behold — the Leather Attractor! It uses spectral energy to magnetically recall projectiles. Arrows, bolts — they'll come right back to you!"
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Well, about 60% of the time. The other 40%... we don't talk about the other 40%."
    })

    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Can it recall my memories? I can't remember where I left my body."
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "...No, Barnaby."
    })

    ctx:complete_quest()

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Take it! You've earned it. And thank you — the house already feels less... murdery."
    })
end

-- ============================================================================
-- Post-Quest: Completed dialogue + Upgrade offer
-- ============================================================================

function show_post_complete(ctx)
    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Ah, my favorite ghost-hunter! The house has been much quieter since you dealt with that poltergeist."
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "I've been tinkering with the attractor design. I think I can enhance the recovery field — push it up to 72% — but I'll need rare materials."
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Bring me your Leather Attractor and 6 Ancient Fragments, and I'll build you the Improved Attractor. You'll also need Ranged level 50 to handle the increased spectral feedback."
    })
end

-- ============================================================================
-- Objective Progress Notifications
-- ============================================================================

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "defeat_poltergeist" and new_count == 1 then
        ctx:show_notification("The poltergeist has been vanquished! Collect the remains and return to Oddwick.")
    end
end
```

**Step 3: Verify**

Run: `cd rust-server && cargo check 2>&1 | head -5`
Expected: No new errors (Lua scripts are loaded at runtime, but verify TOML still parses)

**Step 4: Commit**

```bash
git add rust-server/data/scripts/quests/ghastly_contraption/
git commit -m "feat: add Lua quest script for A Ghastly Contraption"
```

---

### Task 6: Update Haunted House Map — Entity Spawns

**Files:**
- Modify: `rust-server/maps/interiors/Haunted_house.json`

**Step 1: Add NPC and monster entity spawns to the map**

The map's `entities` array is currently empty. Add the three entities — Oddwick near the entrance, Barnaby in the mid-house area (past the first gate), and the Poltergeist in a basement-like area (back of the house).

Update the `"entities": []` in `Haunted_house.json` to:

```json
"entities": [
    {
        "entityId": "prof_oddwick",
        "x": 24,
        "y": 44,
        "uniqueId": "oddwick_1",
        "facing": "south",
        "respawn": false
    },
    {
        "entityId": "barnaby_ghost",
        "x": 30,
        "y": 22,
        "uniqueId": "barnaby_1",
        "facing": "south",
        "respawn": false
    },
    {
        "entityId": "haunted_poltergeist",
        "x": 8,
        "y": 10,
        "uniqueId": "poltergeist_1",
        "facing": "south",
        "respawn": false
    }
]
```

Note: These coordinates are estimates based on the map layout:
- Oddwick at (24, 44) — near the entrance at bottom center
- Barnaby at (30, 22) — mid-house area past the first gate (near the piano room)
- Poltergeist at (8, 10) — upper-left area (basement/back of house, accessed via stairs on the left)

Adjust coordinates based on actual walkable tile positions in the map editor.

**Step 2: Commit**

```bash
git add rust-server/maps/interiors/Haunted_house.json
git commit -m "feat: add NPC and monster spawns to haunted house map"
```

---

### Task 7: Integration Test — Full Quest Walkthrough

**Step 1: Manual server test**

Start the server and verify:

```bash
cd rust-server && cargo run 2>&1 | head -20
```

Expected: Server starts without errors. Check logs for:
- Quest `ghastly_contraption` loaded
- NPC prototypes `prof_oddwick`, `barnaby_ghost`, `haunted_poltergeist` loaded
- Items `tinderbox`, `basement_key`, `haunted_ectoplasm`, `spectral_coil_quest` loaded

**Step 2: In-game walkthrough**

Connect to the server and walk through the quest:

1. Enter the Haunted House instance
2. Talk to Oddwick → accept quest
3. Search bookshelves → receive tinderbox (Note: this requires a searchable object — may need map object or NPC proxy)
4. Interact with candle puzzle → solve it (skull → tall → red → small)
5. Walk through opened gate area → reach_location triggers
6. Talk to Barnaby → answer questions → receive basement key
7. Enter basement area → fight poltergeist
8. Loot ectoplasm + coil
9. Return to Oddwick → watch build sequence → receive Leather Attractor

**Step 3: Commit any coordinate/data adjustments**

```bash
git add -A
git commit -m "fix: adjust quest data after integration testing"
```

---

### Task 8: Tinderbox Search Interaction

The `find_tinderbox` objective is `collect_item` type. The player needs a way to "search" the bookshelves and receive the tinderbox. There are several approaches depending on existing mechanics:

**Option A: Bookshelf NPC proxy**
Create a non-visible "searchable" NPC entity (`haunted_bookshelf`) placed at the bookshelf location. When the player interacts with it, the Lua script gives them the tinderbox. This is the simplest approach if the game already supports interacting with map objects as NPCs.

**Option B: The tinderbox is a ground item pickup**
Place the tinderbox as a lootable item on the ground near the bookshelves. The `collect_item` objective tracks when the player picks it up.

**Option C: Oddwick gives it directly**
Simplify the flow — Oddwick hands the player the tinderbox during the offer dialogue. One fewer interaction point but less exploration.

Choose the approach that best fits existing game mechanics. If using Option A, create a simple NPC entity:

```toml
# in ghastly_contraption_quest.toml NPCs file
[haunted_bookshelf]
display_name = "Dusty Bookshelf"
sprite = "bookshelf"
animation_type = "none"
description = "A bookshelf covered in dust and cobwebs."

[haunted_bookshelf.stats]
max_hp = 999
damage = 0
attack_range = 0
aggro_range = 0
chase_range = 0
move_cooldown_ms = 0
attack_cooldown_ms = 0
respawn_time_ms = 0

[haunted_bookshelf.rewards]
exp_base = 0
gold_min = 0
gold_max = 0

[haunted_bookshelf.behaviors]
hostile = false
quest_giver = true
wander_enabled = false

[haunted_bookshelf.quest_giver]
available_quests = ["ghastly_contraption"]

[haunted_bookshelf.dialogue]
greeting = "A dusty bookshelf. Nothing of interest."
```

And add a handler in the Lua script for when the player interacts with the bookshelf (in `route_in_progress`):

```lua
-- In route_in_progress, before the gate check:
if tinderbox.current < tinderbox.target then
    -- Could be talking to Oddwick OR searching bookshelf
    -- Check if this is the bookshelf interaction by seeing if quest is accepted
    -- (bookshelf interaction only makes sense after quest accepted)
    show_bookshelf_search(ctx)
    return
end
```

```lua
function show_bookshelf_search(ctx)
    ctx:show_dialogue({
        speaker = "Narrator",
        text = "You rummage through the dusty bookshelf. Old tomes, cobwebs, a suspicious amount of cat hair..."
    })

    ctx:show_dialogue({
        speaker = "Narrator",
        text = "Your hand closes around a small metal box buried behind a stack of mouldy encyclopedias."
    })

    ctx:give_item("tinderbox", 1)
    ctx:show_notification("Found: Tinderbox")

    ctx:show_dialogue({
        speaker = "Narrator",
        text = "A tinderbox! This should be able to light those candles."
    })
end
```

This task requires discussion with the developer about which approach fits best. Implement accordingly and commit.

---

### Post-Implementation Notes

**Coordinate tuning:** NPC spawn positions and the quest_location radius need to be fine-tuned in-game. The map editor or direct JSON editing is the best way to get these right.

**Sprites:** The NPC sprites (`wise_man`, `ghost`, `bookshelf`) are placeholders. Swap them for custom sprites when available.

**Post-quest upgrade:** The Improved Attractor upgrade dialogue is shown in `show_post_complete()` but the actual exchange mechanic (consume attractor + 6 ancient fragments → give improved attractor) requires checking player inventory in Lua, which may need a new Lua API method (`ctx:has_item(id, count)`). This can be added as a follow-up task.
