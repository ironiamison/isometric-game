# The Pharaoh's Curse — Quest Design

## Overview

**Quest ID:** `pharaohs_curse`
**Chain ID:** `desert_pharaoh` (quest 1 of future chain)
**Requirements:** Combat level 50+
**Type:** Lua-scripted (complex dialogue, puzzle, boss)

A mystery quest where the player investigates rumors of an ancient cursed pharaoh buried beneath the desert pyramid. Through conversations with 4 NPCs, the player pieces together the story, solves a riddle to obtain a key, and descends into the pyramid tomb to confront the awakened pharaoh in a boss fight.

## Quest Flow

1. Player hears rumors from desert NPCs (4 conversations across the map)
2. Player finds a hidden house in the desert containing an ancient book
3. Reading the book presents a riddle — the answer comes from clues in the earlier NPC conversations
4. Solving the riddle grants the Pharaoh's Key
5. Player uses the key on a locked door inside the pyramid
6. Beyond the door: a tomb chamber where interacting with a sarcophagus awakens the Cursed Pharaoh
7. Boss fight: stationary caster, ranged projectile attacks, summons minion waves
8. Defeat the pharaoh to complete the quest

## NPC Dialogue & Clues

### 1. Desert Merchant (desert town/market area)
- Casual conversation — business is slow because travelers avoid the pyramid
- Mentions hearing "chanting from beneath the sand" at night
- **Clue:** The pharaoh's name was **"Kha'reth"**

### 2. Nomad Elder (desert camp/oasis)
- Tells the full legend — Kha'reth was a pharaoh who enslaved his people to build a tomb that would grant him immortality
- His priests sealed him inside when they realized he'd been corrupted by dark magic
- **Clue:** The seal was bound by **"three stars of the southern sky"**

### 3. Tomb Researcher (near pyramid entrance)
- Has been translating inscriptions on the pyramid walls
- Shows the player a partial translation referencing a ritual
- Warns the player not to go inside — others have tried and never returned
- **Clue:** The ritual involved **"blood of the scorpion"**

### 4. Hermit (hidden house in the desert)
- Old scholar who has spent decades studying Kha'reth
- Has the ancient book but won't let just anyone read it
- Requires the player to prove they know the story (dialogue check: have you talked to the others?)
- Opens the book — presents the riddle

## The Riddle

> "Speak the name of the cursed one, the binding above, and the price below."

Player must choose the correct 3 dialogue options in sequence:
1. **Kha'reth** (from the merchant)
2. **Three stars** (from the elder)
3. **Blood of the scorpion** (from the researcher)

Wrong answers allow retry — the hermit says "That's not right... perhaps you should speak to more people."

Solving the riddle grants the **Pharaoh's Key**.

## Boss Fight — Kha'reth, the Cursed Pharaoh

### Arena
- Private instance (solo only)
- Interior map: `pyramid_tomb`
- Player uses Pharaoh's Key on locked door inside the pyramid
- Dark chamber with a glowing sarcophagus in the center
- Interacting with sarcophagus triggers: "The sarcophagus cracks open... dark energy floods the chamber"

### Boss Stats
- **Name:** Kha'reth, the Cursed Pharaoh
- **Entity ID:** `khareth_pharaoh`
- **Level:** 55
- **HP:** 300
- **Position:** Stationary, center of arena
- **Ranged attack:** Shadow projectiles

### Phase 1 — Awakening (100%–66% HP)
- Shadow projectiles at player (damage ~8, every 2s)
- Summons 2 mummy minions every 15 seconds
- Minions: `pharaoh_mummy`, melee, level 30, 25 HP

### Phase 2 — Wrath (66%–33% HP)
- Projectile rate increases (every 1.5s)
- Summons 3 minions every 12 seconds
- Minions upgrade: `pharaoh_skeleton`, melee, level 35, 35 HP

### Phase 3 — Desperation (33%–0% HP)
- Projectile rate every 1s, damage increases to ~12
- Summons 4 minions every 10 seconds (mix of mummies and skeletons)
- Arena shrinks — AoE damage on outer tiles ("cursed sand creeps inward")

### Death
- Pharaoh crumbles, dark energy dissipates
- Reward chest spawns
- Quest completes

## Quest Objectives (Sequential)

1. `talk_to` — Desert Merchant
2. `talk_to` — Nomad Elder
3. `talk_to` — Tomb Researcher
4. `talk_to` — Hermit (triggers book + riddle via Lua)
5. `reach_location` — Enter the pyramid tomb chamber
6. `kill_monster` — Defeat Kha'reth (count: 1)

## Rewards
- Combat XP (scaled for level 50+)
- Gold reward
- Loot from boss chest (TBD)
- Unlocks next quest in the `desert_pharaoh` chain

## New Entities Required

| Type | ID | Details |
|------|----|---------|
| NPC | `desert_merchant_quest` | Desert town/market |
| NPC | `nomad_elder` | Desert camp/oasis |
| NPC | `tomb_researcher` | Near pyramid entrance |
| NPC | `desert_hermit` | Hidden house in desert |
| Boss | `khareth_pharaoh` | Level 55, 300 HP, stationary caster |
| Minion | `pharaoh_mummy` | Level 30, 25 HP, melee |
| Minion | `pharaoh_skeleton` | Level 35, 35 HP, melee |
| Item | `pharaohs_key` | Quest item |
| Map | `pyramid_tomb` | Boss arena interior |
| Script | `pharaohs_curse.lua` | Dialogue, riddle, boss trigger |

## Boss Implementation Notes

This is a new boss archetype — **stationary caster** — distinct from the Desert Wurm's dig/emerge/AoE pattern:
- Boss does not move
- Primary threat is ranged projectile spam + minion pressure
- Phase transitions increase projectile rate, minion count, and introduce arena shrink
- Needs new boss state machine (separate from `WurmState`)
- Projectile attack reuses existing spell/projectile system
- Minion spawning similar to Wurm's `SpawnMinion` event but spawns combat NPCs instead of exploding rocks
