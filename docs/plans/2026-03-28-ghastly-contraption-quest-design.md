# "A Ghastly Contraption" — Quest Design

**Quest ID:** `ghastly_contraption`
**Location:** Haunted House instance (48x48 map, `maps/interiors/Haunted_house.json`)
**Requirements:** Ranged level 20
**Rewards:** Leather Attractor + 500 XP + 100 Gold
**Type:** Lua-scripted (puzzles, dialogue, combat)

## Overview

The player enters a haunted house and meets Professor Oddwick, an eccentric inventor who bought the place and immediately regretted it. The house is infested with ghosts and he needs help clearing out the source — an angry poltergeist in the basement. Through environmental puzzles, a charming ghost NPC, and a basement boss fight, the player collects haunted components that Oddwick uses to build the Leather Attractor.

## The Cast

### Professor Oddwick (quest giver)
- **Location:** Near the entrance, first room
- **Personality:** Eccentric, enthusiastic, well-meaning but unreliable. Talks fast, gets excited about weird things. Too scared to explore the house himself but won't admit it.
- **Role:** Gives the quest, provides a (partially wrong) hint for the candle puzzle, builds the attractor at the end.

### Barnaby (friendly ghost)
- **Location:** Mid-house area, past the first gate
- **Personality:** Friendly, confused, doesn't know he's dead. Thinks everyone else is the ghost. Lonely but cheerful.
- **Role:** Has the basement key (thinks it's a lucky charm). The player must pass his absurd "prove you're alive" interrogation to get it.

### The Poltergeist (basement boss)
- **Entity ID:** `haunted_poltergeist`
- **Level:** 28
- **HP:** 150
- **Location:** Basement (accessed via stairs behind second locked gate)
- **Behavior:** Hostile, aggressive. The source of the hauntings.
- **Drops:** Haunted Ectoplasm (x1), Spectral Coil (x1) — quest items

## Quest Progression

### Step 1: Meet Oddwick

Player enters the haunted house and finds Oddwick in the first room near the entrance. He's surrounded by half-built gadgets and looks frazzled.

**Dialogue:**
- Oddwick explains he bought the house at auction ("A steal! Literally — the previous owner's ghost stole all the furniture.")
- He's been working on a device to neutralize spectral energy but needs components from the basement
- The basement is locked — there are gates deeper in the house controlled by old mechanisms
- He mentions candles: "The previous owner had some kind of candle system for the gates. I think the order was... tall one first, then the red one, then... hmm, I forget. Maybe the skull candle was second? Or third?"

**Quest accepted.** First objective: find a way to open the first gate.

### Step 2: The Candle Puzzle

**Finding the tinderbox:**
- Player searches a bookshelf/stack of books in the first area
- Finds a **Tinderbox** (quest item, used automatically when interacting with candles)

**The candles:**
- 4 candles are placed around the first room area, each visually distinct (by position/description in dialogue)
- Correct order: Skull candle → Tall candle → Red candle → Small candle
- Oddwick's hint suggests: "Tall one first, then... skull? Or was skull second?" — he gets the first two swapped
- Lighting them in wrong order: candles extinguish with a ghostly laugh, spooky flicker effect, reset
- Lighting them correctly: the first gate opens with a creak

**Puzzle design notes:**
- Oddwick's hint gives you 3 of 4 candles but the order is slightly off
- Player needs 1-3 attempts to figure it out
- Each wrong attempt has a fun flavor message ("A cold wind snuffs the flames... somewhere, a ghost laughs at you.")

### Step 3: Meet Barnaby

Past the first gate, the player finds Barnaby floating in the mid-house area (the room with the piano, bookshelves, etc.).

**Dialogue — "Prove You're Alive" Interrogation:**

Barnaby is suspicious. He's been alone in the house for ages and thinks everyone who enters is a ghost.

> **Barnaby:** "Oh! A visitor! ...Wait. Are you alive? Or are you one of THEM?"

The player must answer 3 questions. Each has a correct answer, a wrong answer, and a funny answer. Wrong answers make Barnaby more suspicious but allow retry.

**Question 1: "Do you breathe?"**
- "Yes, obviously." → Barnaby: "Hmm... that's EXACTLY what a ghost pretending to breathe would say."
- "Watch me." (correct) → Barnaby: "Oh! Your chest moves! ...Unless that's a trick."
- "Do YOU breathe?" → Barnaby: "Of course I do! I'm perfectly alive! ...Aren't I?"

**Question 2: "What's your favorite food?"**
- "I don't eat." → Barnaby: "AHA! Ghost confirmed!"
- "Bread and stew." (correct) → Barnaby: "Ooh, that does sound like a living person thing..."
- "Ectoplasm." → Barnaby: "See, I KNEW— wait, really? That's disgusting even for a ghost."

**Question 3: "Can you walk through walls?"**
- "Yes." → Barnaby: "Ghost! I knew it!"
- "No, I used the door." (correct) → Barnaby: "The DOOR? Nobody uses doors anymore! ...Maybe you ARE alive."
- "Can YOU?" → Barnaby: *floats through a wall and back* "See? Easy! ...Wait, can you not do that?"

After passing all 3:
> **Barnaby:** "Alright, alright, I believe you. You're alive. How exciting! I haven't talked to a living person in... how long have I been here?"

Player asks about the basement key. Barnaby pulls out the key thinking it's his lucky charm.

> **Barnaby:** "This old thing? I found it years ago. It's my lucky charm! ...Fine, take it. It's not like I need luck. I'm already dead. ...Wait, what?"

Player receives **Basement Key**.

### Step 4: The Basement Fight

Player uses the Basement Key on the gate near the stairs (left side of the map). Descends to the basement.

**The Poltergeist:**
- Level 28, 150 HP
- Hostile on sight, melee/ranged hybrid
- Standard combat — no special mechanics, just a solid fight appropriate for Ranged 20+ players
- On death, drops: **Haunted Ectoplasm** (x1) and **Spectral Coil** (x1)

### Step 5: Return to Oddwick — The Build

Player brings the Haunted Ectoplasm and Spectral Coil back to Oddwick.

**Dialogue — First Attempt (failure):**
> **Oddwick:** "You got them! Magnificent! Now hold still while I... calibrate the ectoplasmic resonance matrix..."
> *Sparks fly. A small explosion. Smoke fills the room.*
> **Oddwick:** "...That wasn't supposed to happen."

Barnaby floats in:
> **Barnaby:** "Is it supposed to be on fire?"
> **Oddwick:** "No, Barnaby. Thank you."

**Dialogue — Second Attempt (success):**
> **Oddwick:** "Right. Slight adjustment... reverse the polarity... and..."
> *A hum fills the air. The device glows softly.*
> **Oddwick:** "IT WORKS! The Leather Attractor! It uses the spectral energy to magnetically recall projectiles. Arrows, bolts — they'll come right back to you! Well, 60% of the time."
> **Barnaby:** "Can it recall my memories? I can't remember where I left my body."
> **Oddwick:** "...No, Barnaby."

**Player receives: Leather Attractor**

Quest complete.

## Post-Quest: Improved Attractor Upgrade

After completing the quest, Oddwick offers a new dialogue option:

> **Oddwick:** "I've been tinkering with the design. I think I can enhance the attractor's recovery field... but I'll need rare materials."

**Requirements:** Ranged level 50 + Leather Attractor (consumed) + 6 Ancient Fragments
**Result:** Improved Attractor (72% ammo save, +4 ranged attack/strength)
**Format:** Dialogue interaction with Oddwick, not a separate quest.

## New Entities Required

| Type | ID | Details |
|------|----|---------|
| NPC | `prof_oddwick` | Near entrance, quest giver |
| NPC | `barnaby_ghost` | Mid-house, friendly ghost |
| Monster | `haunted_poltergeist` | Basement, level 28, 150 HP |
| Item | `tinderbox` | Quest item, found on bookshelf |
| Item | `basement_key` | Quest item, from Barnaby |
| Item | `haunted_ectoplasm` | Quest item, poltergeist drop |
| Item | `spectral_coil` | Quest item, poltergeist drop |
| Script | `ghastly_contraption.lua` | All dialogue, puzzles, quest logic |

## Quest Objectives (TOML)

1. `talk_to` — Professor Oddwick (accept quest)
2. `collect_item` — Tinderbox (search bookshelf)
3. `reach_location` — Open the first gate (candle puzzle solved via Lua)
4. `talk_to` — Barnaby (dialogue puzzle, receive basement key)
5. `kill_monster` — Defeat the Poltergeist (x1)
6. `collect_item` — Haunted Ectoplasm (x1, from poltergeist)
7. `collect_item` — Spectral Coil (x1, from poltergeist)
8. `talk_to` — Return to Oddwick (triggers build sequence, rewards)

## Technical Notes

- Candle puzzle is entirely Lua-driven — track candle light state, validate order, trigger gate open
- Barnaby dialogue is Lua-driven — track which questions answered correctly
- Poltergeist is a standard hostile NPC, no special boss state machine needed
- Gate opening could be implemented as collision toggle on specific tiles (or portal activation)
- The tinderbox "search" interaction needs a map object the player can interact with
- Post-quest upgrade is a dialogue branch on Oddwick after quest completion
