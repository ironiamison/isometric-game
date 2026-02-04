# Monster Material Crafting System Design

A multi-step crafting system that gives purpose to monster drops, adds a new Smithing skill, and introduces station-based crafting with an interactive UI.

## Overview

Players collect materials from early-game monsters, craft intermediate components, then combine those into the **Scavenger Set** - early-mid game equipment that fills gaps in the current progression.

### Core Loop

1. Kill monsters → collect drops (slime cores, snail shells, hedgehog spines, etc.)
2. Visit crafting station (anvil or workbench)
3. Craft components from raw drops
4. Combine components into final gear
5. Gain Smithing XP on successful crafts

## The Scavenger Set

### Equipment Stats

| Slot | Item | Sprite | Stats | Smithing Lvl | Defence Lvl |
|------|------|--------|-------|--------------|-------------|
| Body | Scavenger Vest | `scavenger_vest` | def 3 | 1 | 1 |
| Feet | Scavenger Boots | `scavenger_boots` | def 2 | 1 | 1 |
| Ring | Bone Ring | `bone_ring` | atk 1, str 1, def 1 | 5 | 1 |
| Necklace | Shell Pendant | `shell_pendant` | def 3 | 5 | 1 |
| Weapon | Spine Blade | `spine_blade` | atk 4, str 6 | 5 | — |
| Weapon | Scavenger Bow | `scavenger_bow` | atk 8, str 5, range 6 | 5 | — |
| Ammo | Bone Arrow | `bone_arrow` | Basic ranged ammo | — | — |

### Progression Context

These items fill real gaps in early-game progression:

- **Body:** Between Torn Clothes (def 1) and Salvaged Tunic (def 5)
- **Feet:** Between Peasant Boots (def 1) and Rusty Boots (def 3)
- **Necklace:** First early-game necklace (currently none until level 20)
- **Bow:** First early-game bow (Long Bow requires level 20)

## Crafting Recipes

### Components (Intermediate Materials)

| Component | Sprite | Recipe | Smithing Lvl |
|-----------|--------|--------|--------------|
| Scrap Leather | `scrap_leather` | 3x Piglet + 2x Slime Core | 1 |
| Bone Fragment | `bone_fragment` | 4x Snail Shell + 2x Hedgehog Spine | 1 |
| Crude Binding | `crude_binding` | 3x Worm Segment + 2x Spider Silk | 1 |
| Shell Plate | `shell_plate` | 5x Snail Shell | 1 |
| Blade Shard | `blade_shard` | 4x Hedgehog Spine + 1x Spring Coil | 1 |
| Bow Stave | `bow_stave` | 3x Spring Coil + 2x Worm Segment | 1 |

### Final Gear

| Item | Recipe | Smithing Lvl |
|------|--------|--------------|
| Scavenger Vest | 2x Scrap Leather + 1x Shell Plate + 1x Crude Binding | 1 |
| Scavenger Boots | 1x Scrap Leather + 1x Crude Binding | 1 |
| Bone Ring | 1x Bone Fragment | 5 |
| Shell Pendant | 1x Shell Plate + 1x Crude Binding | 5 |
| Spine Blade | 1x Blade Shard + 1x Bone Fragment | 5 |
| Scavenger Bow | 1x Bow Stave + 1x Crude Binding + 3x Crow Feather | 5 |

### Ammunition (No Smithing Requirement)

| Item | Recipe | Output |
|------|--------|--------|
| Bone Arrow | 1x Hedgehog Spine + 3x Crow Feather | 15 arrows |

## Crafting Stations

### Station Types

| Station | Crafts | Location |
|---------|--------|----------|
| Anvil | Weapons, armor, metal components | Blacksmith building |
| Workbench | Arrows, accessories, bindings | Craftsman area |

### Interaction

- Player must be adjacent to station to interact
- Click station or press interact key to open crafting panel
- Each station shows only recipes relevant to that station type

## Recipe Discovery

### From Blacksmith NPC

Taught when player talks to the Blacksmith NPC (requires meeting Smithing level):

**At Smithing Level 1:**
- All 6 component recipes
- Scavenger Vest
- Scavenger Boots
- Bone Arrow

**At Smithing Level 5:**
- Bone Ring
- Shell Pendant

### From Monster Drops (Recipe Scrolls)

| Recipe Scroll | Drops From | Drop Rate |
|---------------|------------|-----------|
| Recipe: Spine Blade | Spider | 5% |
| Recipe: Scavenger Bow | Crow | 3% |
| Recipe: Scavenger Bow | Springy | 3% |

Recipe scrolls are consumed on use and permanently teach the recipe.

## Crafting UI

### Main Panel Layout

```
+-----------------------------------------------------+
|  ANVIL                                          [X] |
+-----------------------------------------------------+
|  +--------+  +--------+  +--------+  +--------+     |
|  | [icon] |  | [icon] |  | [????] |  | [????] |     |
|  | Recipe |  | Recipe |  | Locked |  | Locked |     |
|  +--------+  +--------+  +--------+  +--------+     |
|                                                     |
|  ---------------------------------------------------+
|                                                     |
|  +----------+   SCAVENGER VEST                      |
|  |          |   "Cobbled-together armor from        |
|  |  [LARGE  |    forest creature scraps."           |
|  |  SPRITE] |                                       |
|  |          |   Materials:                          |
|  +----------+   [icon] Scrap Leather .... 2/2  Y    |
|                 [icon] Shell Plate ...... 1/1  Y    |
|                 [icon] Crude Binding .... 0/1  X    |
|                                                     |
|                 Smithing Level: 1  Y                |
|                                                     |
|                 +---------------------+             |
|                 |       CRAFT         |             |
|                 +---------------------+             |
+-----------------------------------------------------+
```

### UI Elements

| Element | Behavior |
|---------|----------|
| Recipe grid | Known recipes show icons; unknown show "????" |
| Selected recipe | Large sprite preview, name, description |
| Materials list | Required items with owned/needed counts, checkmarks |
| Skill requirement | Smithing level needed with checkmark if met |
| Craft button | Grayed out if requirements not met; glows when ready |

### During Crafting

```
+-----------------------------------------------------+
|                                                     |
|                   CRAFTING...                       |
|                                                     |
|                 +----------+                        |
|                 |  [LARGE  |                        |
|                 |  SPRITE] |                        |
|                 +----------+                        |
|                                                     |
|          [========--------]  65%                    |
|                                                     |
|                 +---------------------+             |
|                 |       CANCEL        |             |
|                 +---------------------+             |
+-----------------------------------------------------+
```

### Crafting Behavior

| Aspect | Behavior |
|--------|----------|
| Progress | Bar fills over crafting duration |
| Visual feedback | Sprite pulses or glows subtly |
| Cancel | Click Cancel button to abort (materials refunded) |
| Interrupt | Taking damage cancels craft (materials refunded) |
| Movement | Player cannot move during crafting |

### On Completion

- Item sprite does a "pop" or shine animation
- "Crafted!" message displayed
- Item added to inventory
- UI returns to recipe selection

## Crafting Duration

| Complexity | Duration | Examples |
|------------|----------|----------|
| Simple | 1.5s | Components, Bone Arrow |
| Standard | 3s | Scavenger Boots, Bone Ring, Shell Pendant |
| Complex | 5s | Scavenger Vest, Spine Blade, Scavenger Bow |

## Smithing Skill

### Integration

- New skill added to Skills panel alongside combat and gathering skills
- Uses same RS-style exponential XP curve as other skills
- XP granted only on successful craft completion (not on cancel/interrupt)

### XP Rewards

| Craft | XP |
|-------|-----|
| Components (any) | 10 |
| Bone Arrow (x15) | 5 |
| Scavenger Boots | 20 |
| Bone Ring | 25 |
| Shell Pendant | 25 |
| Scavenger Vest | 35 |
| Spine Blade | 40 |
| Scavenger Bow | 45 |

### Level Requirements

| Smithing Level | Unlocks |
|----------------|---------|
| 1 | All components, Scavenger Vest, Scavenger Boots, Bone Arrow |
| 5 | Bone Ring, Shell Pendant, Spine Blade, Scavenger Bow |

## Monster Drops

### Existing Drops (No Changes)

| Monster | Level | Drop | Drop Rate | Used In |
|---------|-------|------|-----------|---------|
| Slime | 1 | Slime Core | 25% | Scrap Leather |
| Snail | 1 | Snail Shell | 30% | Bone Fragment, Shell Plate |
| Hedgehog | 2 | Hedgehog Spine | 25% | Bone Fragment, Blade Shard, Bone Arrow |
| Worm | 1 | Worm Segment | 20% | Crude Binding, Bow Stave |
| Springy | 2 | Spring Coil | 20% | Blade Shard, Bow Stave |
| Pig | 1 | Piglet | existing | Scrap Leather |
| Spider | 3 | Spider Silk | 35% | Crude Binding |
| Crow | 2 | Crow Feather | 40% | Bone Arrow, Scavenger Bow |

### New Drops: Recipe Scrolls

| Item | Monster | Drop Rate |
|------|---------|-----------|
| Recipe: Spine Blade | Spider | 5% |
| Recipe: Scavenger Bow | Crow | 3% |
| Recipe: Scavenger Bow | Springy | 3% |

## New Items Summary

### Materials (Category: material)

| Item ID | Display Name | Sprite | Description |
|---------|--------------|--------|-------------|
| `scrap_leather` | Scrap Leather | `scrap_leather` | Crudely tanned leather from forest creatures. |
| `bone_fragment` | Bone Fragment | `bone_fragment` | Bits of shell and bone bound together. |
| `crude_binding` | Crude Binding | `crude_binding` | Fibrous material for holding things together. |
| `shell_plate` | Shell Plate | `shell_plate` | Hardened shell pieces, surprisingly sturdy. |
| `blade_shard` | Blade Shard | `blade_shard` | Sharp spines formed into a crude blade edge. |
| `bow_stave` | Bow Stave | `bow_stave` | A flexible frame ready to be strung. |

### Equipment (Category: equipment)

| Item ID | Display Name | Sprite | Slot |
|---------|--------------|--------|------|
| `scavenger_vest` | Scavenger Vest | `scavenger_vest` | body |
| `scavenger_boots` | Scavenger Boots | `scavenger_boots` | feet |
| `bone_ring` | Bone Ring | `bone_ring` | ring |
| `shell_pendant` | Shell Pendant | `shell_pendant` | necklace |
| `spine_blade` | Spine Blade | `spine_blade` | weapon |
| `scavenger_bow` | Scavenger Bow | `scavenger_bow` | weapon (ranged) |

### Ammunition (Category: material)

| Item ID | Display Name | Sprite | Description |
|---------|--------------|--------|-------------|
| `bone_arrow` | Bone Arrow | `bone_arrow` | Arrows crafted from bone and feather. |

### Recipe Scrolls (Category: material)

| Item ID | Display Name | Sprite | Description |
|---------|--------------|--------|-------------|
| `recipe_spine_blade` | Recipe: Spine Blade | `recipe_scroll` | Teaches how to craft a Spine Blade. |
| `recipe_scavenger_bow` | Recipe: Scavenger Bow | `recipe_scroll` | Teaches how to craft a Scavenger Bow. |

## Data Files to Create/Modify

### New Files

- `rust-server/data/recipes/smithing.toml` - All smithing recipes
- `rust-server/data/crafting_stations.toml` - Station definitions

### Files to Modify

- `rust-server/data/items/equipment.toml` - Add Scavenger Set gear
- `rust-server/data/items/materials.toml` - Add components, arrows, recipe scrolls
- `rust-server/data/entities/monsters/*.toml` - Add recipe scroll drops
- `rust-server/src/skills.rs` - Add Smithing skill
- Client UI files - New crafting panel

## Future Expansion

This system is designed to scale:

- **Mid-game tier:** Spider Silk armor, Crow Feather accessories (Smithing 15-20)
- **Late-game tier:** Reaper gear from Dark Essence + Scythe Fragments (Smithing 40+)
- **Additional stations:** Furnace for smelting, Enchanting table for magic gear
- **Station upgrades:** Better stations = faster crafting or bonus outputs
