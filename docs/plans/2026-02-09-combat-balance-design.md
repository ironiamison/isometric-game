# Progressive Combat Balance Design

## Overview

Rebalance combat to create satisfying progression where:
- Endgame players dominate low-level content (99% hit, 1-2 shot kills)
- Fresh players can fight above their level with risk (need food, possible but dangerous)
- Gear provides meaningful upgrades without making players invincible

## Combat Formula

### Hit Calculation (RS-style with tuned constants)

```rust
attack_roll = random(0, attacker_level × (attack_bonus + 20))
defence_roll = random(0, defender_level × (defence_bonus + 20))
hit = attack_roll > defence_roll
```

**Changes from current:**
- Base constant: 64 → 20 (gear matters more relative to base)
- Max gear bonus: ~100 → ~30 (less extreme scaling)

### Damage Calculation

```rust
max_hit = 1 + (level / 8) + (strength_bonus / 4)
damage = random(1, max_hit)  // minimum 1 damage on hit
```

**Changes from current:**
- Floor of 1 damage (no more 0-damage hits)
- Strength bonus impact: /640 → /4
- Level contribution: /10 → /8

## Gear Tiers

### 4 Tiers with Level Requirements

| Tier | Name | Level Req | Attack | Strength | Defence |
|------|------|-----------|--------|----------|---------|
| 1 | Starter | 1 | +1 to +4 | +1 to +3 | +1 to +5 |
| 2 | Mid | 20 | +6 to +12 | +4 to +8 | +6 to +12 |
| 3 | Late | 40 | +14 to +20 | +10 to +15 | +14 to +22 |
| 4 | Endgame | 60 | +22 to +30 | +16 to +22 | +24 to +35 |

### Stat Distribution by Slot

| Slot | Primary Stats | Notes |
|------|---------------|-------|
| Weapon | Attack, Strength | Main damage source |
| Body | Defence, Strength | Bulk of defence |
| Head | Defence | Moderate defence |
| Legs | Defence | (future slot) |
| Gloves | Attack, Strength | Small offensive boost |
| Boots | Defence, Strength | Small mixed |
| Ring | Any | Specialty/set bonuses |
| Necklace | Any | Specialty/set bonuses |
| Back | Defence | Capes, cloaks |

### Full Set Totals

| Tier | Total Attack | Total Strength | Total Defence |
|------|--------------|----------------|---------------|
| 1 (full starter) | +5 | +4 | +8 |
| 2 (full mid) | +15 | +10 | +18 |
| 3 (full late) | +25 | +18 | +30 |
| 4 (full endgame) | +35 | +25 | +45 |

### Future Expansion
- More tiers for endgame content
- Separate sets for melee, ranged, magic
- Set bonuses for wearing matching pieces

## Mob Stats

### New Prototype Fields

```toml
[slime.stats]
level = 1
max_hp = 12
damage = 2
attack_bonus = 0    # NEW - affects hit chance
defence_bonus = 0   # NEW - affects evasion
```

### Mob Archetypes

| Archetype | Attack Bonus | Defence Bonus | HP | Damage | Example |
|-----------|--------------|---------------|-----|--------|---------|
| Balanced | +0 to +5 | +0 to +5 | Medium | Medium | Wolf, Goblin |
| Glass Cannon | +8 to +15 | -5 to +0 | Low | High | Spider, Assassin |
| Tank | -5 to +0 | +10 to +20 | High | Low | Crab, Golem, Turtle |
| Swarm | -5 to +0 | -5 to +0 | Very Low | Low | Rat, Bat, Insect |
| Boss | +10 to +20 | +10 to +25 | Very High | High | Pig King, Dungeon Boss |

### Zone Stat Ranges

| Zone | Levels | Base HP | Base Damage | Total Bonus Range |
|------|--------|---------|-------------|-------------------|
| Starter | 1-5 | 8-20 | 1-3 | +0 to +5 |
| Forest | 6-15 | 20-45 | 3-6 | +5 to +15 |
| Dangerous | 16-30 | 45-80 | 6-12 | +10 to +25 |
| Late | 31-50 | 80-150 | 12-20 | +15 to +35 |
| Endgame | 51+ | 150-500+ | 20-35 | +25 to +50 |

### Level Scaling

Applied to base stats from prototype:
- HP: +10% per level above 1
- Damage: +15% per level above 1

## XP System

### XP Rates (unchanged)
- Combat: 4 XP per damage dealt
- Hitpoints: 1.33 XP per damage dealt
- Mob kill bonus: `exp_base × mob_level`

### Scaling
- Flat (no level-based multipliers)
- Same XP whether fighting above or below your level

## Combat Scenarios

### Fresh player (Combat 3, no gear) vs Slime (Level 1, +0/+0)
- Player hit chance: ~75%
- Player max hit: 1
- Slime hit chance: ~25%
- Hits to kill: ~12

*Feel: Slow but safe. Player learning combat.*

### Fresh player vs Wolf (Level 10, +5/+5)
- Player hit chance: ~19%
- Player max hit: 1
- Wolf hit chance: ~81%
- Hits to kill: ~35

*Feel: Dangerous! Need food and luck. But possible.*

### Mid player (Combat 30, +15/+10/+18) vs Ogre (Level 25, +10/+10)
- Player hit chance: ~78%
- Player max hit: 6
- Ogre hit chance: ~40%
- Hits to kill: ~17

*Feel: Fair fight. Take some damage, use food occasionally.*

### Endgame player (Combat 70, +35/+25/+45) vs Slime
- Player hit chance: ~99%
- Player max hit: 15
- Slime hit chance: <1%
- Hits to kill: 1-2

*Feel: Total domination. Farming mode.*

## Implementation Checklist

### Code Changes
- [ ] Update hit formula in `skills.rs` (base 64 → 20)
- [ ] Update max_hit formula in `skills.rs`
- [ ] Add `attack_bonus` and `defence_bonus` to mob prototype structs
- [ ] Update NPC combat to use new bonus fields
- [ ] Ensure minimum 1 damage on successful hit

### Data Changes
- [ ] Rebalance all equipment in `equipment.toml`
- [ ] Add attack_bonus/defence_bonus to all mob TOML files
- [ ] Review and adjust mob base HP/damage values
- [ ] Set appropriate levels for each zone's mobs

### Testing
- [ ] Test fresh player vs starter mobs
- [ ] Test fresh player vs dangerous mobs (verify "risky but possible")
- [ ] Test mid-level player vs appropriate mobs
- [ ] Test endgame player vs low-level mobs (verify domination)
- [ ] Verify XP rates feel appropriate
