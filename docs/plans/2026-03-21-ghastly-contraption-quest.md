# "A Ghastly Contraption" — Quest Design

**Goal:** A medium-length, RS-inspired quest in a new haunted house instance that rewards the Leather Attractor. Features quirky NPC dialogue, combat, light puzzle/exploration, and a humorous ghost interaction.

## Overview

- **Quest Name:** A Ghastly Contraption
- **Quest NPC:** Professor Oddwick — eccentric inventor living in an abandoned haunted house
- **Location:** Haunted House instance (new map, entered via door/portal in overworld)
- **Requirements:** Ranged level 20
- **Rewards:** Leather Attractor + 500 XP + 100 Gold
- **Tone:** Quirky, humorous, RS-style. Oddwick talks too fast, gets excited about weird things, and treats dangerous undead like minor inconveniences. The player is an unwilling lab assistant.

## Quest Hook

A notice board or NPC in town mentions a "strange inventor seeking an assistant" at the old haunted house. When the player arrives, Oddwick cheerfully explains he's *this close* to perfecting a device that can magnetically recover projectiles — he just needs a few things from around the house. Unfortunately, the house is haunted. He doesn't seem concerned.

## Quest Steps

### Step 1: Talk to Professor Oddwick
The player enters the haunted house and finds Oddwick in the foyer, surrounded by bizarre equipment. He explains he's building a device that can "magnetically recall projectiles" but needs three components from around the house. He's too busy (scared) to get them himself.

### Step 2: Retrieve the Spectral Coil (combat)
In the basement, the player finds a strange glowing coil on a table — but picking it up awakens 3 restless spirits that must be defeated. Not a hard fight, more atmospheric.

### Step 3: Retrieve the Lodestone Fragment (puzzle/exploration)
In the upstairs study, bookshelves block access to the lodestone. The player needs to find a rusty key hidden in a nearby room (search a dresser/container) to unlock a display case. When they grab it, the room goes dark momentarily and a ghoul appears — defeat it.

### Step 4: Bring a "Volunteer" (humorous NPC interaction)
Oddwick also needs an undead specimen for "calibration purposes." He asks you to lure a specific ghost — Barnaby, a friendly but confused ghost in the attic — downstairs. You talk to Barnaby and convince him to follow you back to Oddwick. Dialogue is funny — Barnaby doesn't realize he's dead.

### Step 5: Return to Oddwick (completion)
Give him the coil and lodestone. He uses Barnaby's "spectral energy" (Barnaby is mildly annoyed) to assemble the attractor. He hands you the Leather Attractor.

## Improved Attractor Upgrade

After completing the quest, Oddwick offers a new dialogue option: "I've been tinkering... I think I can enhance your device."

- **Requirements:** Ranged level 50 + Leather Attractor (consumed) + 6 Ancient Fragments
- **Result:** Improved Attractor (72% ammo save)
- **Format:** Dialogue interaction, not a separate quest

## Technical Notes

- Haunted house is a new instance map
- Quest uses Lua script for dialogue (Oddwick + Barnaby personality)
- Objective types needed: talk_to, collect_item (spectral coil, lodestone fragment), kill_monster (spirits, ghoul), talk_to (Barnaby)
- Upgrade interaction is a post-quest dialogue branch on Oddwick
- Quest items (spectral coil, lodestone fragment) are quest-category items
