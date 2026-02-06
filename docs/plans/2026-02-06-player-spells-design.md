# Player Spells System Design

## Overview

Add a spell system with damage spells (cast on enemies) and heal spells (self-cast). Spells are learned automatically via Magic skill level-ups, managed in a spell book UI, and cast from quick slots.

## Data & Skills

### New Magic Skill
- Added to the existing Skills system alongside Combat, Hitpoints, Fishing, etc.
- XP gained from casting damage spells (based on damage dealt, similar to Combat XP)
- Healing spells also grant Magic XP (based on amount healed)

### Spell Definitions

New `SpellDef` data structure (similar to existing `ItemDef`/`RecipeDef`):

| Spell      | Type   | Magic Level | Mana Cost | Cooldown | Damage/Heal              | Effect Sprite  |
|------------|--------|-------------|-----------|----------|--------------------------|----------------|
| Dark Hand  | Damage | 1           | 5         | 1.5s     | Low (scales with Magic)  | dark_hand.png  |
| Dark Eater | Damage | 15          | 15        | 3s       | High (scales with Magic) | dark_eater.png |
| Heal       | Heal   | 5           | 10        | 5s       | Scales with Magic level  | self_heal.png  |

### Mana System
- Add `mp` and `max_mp` to server-side Player struct (client already has these fields)
- Max MP scales with Magic level: `10 + magic_level * 2`
- Passive regen: 1 MP every 3 seconds
- Synced to client via existing StateSync

## Protocol & Server Logic

### New Protocol Messages

**ClientMessage:**
- `CastSpell { spell_id: String }` — player requests to cast a spell

**ServerMessage:**
- `SpellEffect { caster_id: String, target_id: Option<String>, spell_id: String, target_x: i32, target_y: i32 }` — tells clients to play the visual effect at a location
- `SpellBookUpdate { spells: Vec<SpellInfo> }` — sent on login and level-up, lists available spells with cooldown/cost info
- Reuse existing `DamageEvent` for spell damage
- Reuse existing `PlayerAttack { attack_type: "spell" }` for casting animation

### Server Casting Flow
1. Player sends `CastSpell { spell_id }`
2. Server validates: known spell? high enough Magic level? enough mana? cooldown ready? valid target (for damage spells)?
3. **Damage spells:** hit/miss calculation using Magic level vs target defense, roll damage, send `DamageEvent` + `SpellEffect`
4. **Heal:** calculate heal amount from Magic level, clamp to max HP, send `SpellEffect` to nearby players
5. Deduct mana, start cooldown timer, award Magic XP

### Cooldown Tracking
- Server-side `HashMap<String, u64>` per Player mapping `spell_id -> last_cast_tick`
- Client also tracks cooldowns locally for UI feedback (grey out slots, show timer)

## Client UI & Visuals

### Spell Book UI
- New panel (toggled by button or key) showing all spells
- Locked spells greyed out with level requirement shown
- Each spell displays: icon, name, level req, mana cost, cooldown, description
- Drag spells from spell book into quick slots to equip them

### Quick Slots Integration
- Extend quick slots to hold either items or spells (slot type enum)
- Pressing 1-5 with a spell slot sends `CastSpell` instead of using an item
- Show mana cost on the slot, grey out when on cooldown or insufficient mana

### Spell Effects
- Load effect sprite sheets, play animated frames at the target tile position
- Damage spells: effect plays on target NPC's tile
- Heal: effect plays on caster's own tile
- Frame count and size parsed from sprite sheet dimensions
- Effects are temporary — play once and despawn

### Mana Bar
- Display MP bar below HP bar on the HUD

### Teleport Effect (bonus)
- Play bubbles_warp.png on the player's tile when admin `/teleport` command is used
