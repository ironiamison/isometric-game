# Crafting Crates — Design

## Overview

Crafting orders reward lootbox items ("crates") that players can open from their inventory for a single random loot roll. Two tiers: Artisan's Crate (regular orders) and Master's Crate (masterwork orders). Loot is level-scaled based on the order's min_level. Crates are bankable and tradeable, enabling stockpiling for opening parties.

## Crate Items

Two new items:

- **Artisan's Crate** (`artisans_crate`) — Rewarded for completing regular crafting orders. "A sturdy crate packed by the Artisan's Guild. Who knows what's inside?"
- **Master's Crate** (`masters_crate`) — Rewarded for completing masterwork crafting orders. "A gilded crate reserved for master craftsmen. Contains something exceptional."

Both are stackable, bankable, tradeable, and opened via right-click "Use" in inventory.

## Loot Tables

Each crate does a single roll. The roll picks a rarity tier first, then picks an item from that tier's pool based on the order's min_level bracket.

### Level Brackets

| Bracket | Order min_level | Label |
|---------|----------------|-------|
| Low | 1–19 | Beginner materials |
| Mid | 20–39 | Intermediate materials |
| High | 40+ | Advanced materials |

### Artisan's Crate Loot

**Rarity weights:** Common 60%, Uncommon 30%, Rare 10%

**Common (resource bundles):**
- Low: 15-25 copper ore, 10-20 oak logs, 15-25 raw shrimp
- Mid: 10-20 iron ore, 10-15 willow logs, 10-20 raw salmon, 5-10 iron bars
- High: 8-15 mithril ore, 8-12 yew logs, 8-15 raw swordfish, 5-8 mithril bars

**Uncommon (useful consumables/materials):**
- Low: 3-5 basic potions, 2-4 bronze bars, 5-10 feathers
- Mid: 2-4 mid-tier potions, 3-5 steel bars, 2-3 rare seeds
- High: 2-3 super potions, 3-5 adamant bars, 1-2 rare seeds

**Rare (notable drops):**
- Low: 1 random bronze/iron equipment piece, 3 commission marks
- Mid: 1 random steel equipment piece, 5 commission marks
- High: 1 random mithril equipment piece, 8 commission marks

### Master's Crate Loot

**Rarity weights:** Common 40%, Uncommon 40%, Rare 15%, Epic 5%

**Common (premium resource bundles):**
- Mid: 15-25 iron ore, 10-15 steel bars, 10-20 raw lobster
- High: 10-20 mithril ore, 8-12 mithril bars, 10-15 raw swordfish

**Uncommon (high-value consumables):**
- Mid: 3-5 super potions, 5-8 steel bars, 3-5 rare seeds
- High: 3-5 super potions, 3-5 adamant bars, 2-3 rare seeds

**Rare (equipment/currency):**
- Mid: 1 random steel/mithril equipment, 10 commission marks
- High: 1 random mithril/adamant equipment, 15 commission marks

**Epic (exclusive):**
- All brackets: 25 commission marks, or 1 exclusive cosmetic item (gilded tool skin, artisan's hat)

## Opening Flow

1. Player right-clicks crate in inventory → "Open" option
2. Server receives UseItem message for the crate
3. Server determines level bracket from a stored metadata field on the crate item, or defaults to the mid bracket
4. Server rolls rarity, then rolls item from the appropriate pool
5. Server removes 1 crate from inventory, adds rolled loot
6. Server sends system message: "You open an Artisan's Crate and find: 15x Iron Ore!"
7. Server sends InventoryUpdate

## Level Bracket Storage

The crate needs to know what level bracket to use when opened. Two approaches:

**Option A — Item variants:** Define separate item IDs per bracket: `artisans_crate_low`, `artisans_crate_mid`, `artisans_crate_high`. Simple, no metadata system needed. Downside: 6 items instead of 2, and they don't stack across brackets.

**Option B — Single item, bracket stored in a metadata/tag field:** One item ID, bracket stored as item metadata. Requires item metadata support.

Recommend **Option A** for simplicity. Players will see "Artisan's Crate (Beginner)" / "(Intermediate)" / "(Advanced)" which also sets expectations about loot quality.

## Item Definitions

6 crate items total:

| Item ID | Display Name | Bracket |
|---------|-------------|---------|
| `artisans_crate_low` | Artisan's Crate (Beginner) | Low |
| `artisans_crate_mid` | Artisan's Crate (Intermediate) | Mid |
| `artisans_crate_high` | Artisan's Crate (Advanced) | High |
| `masters_crate_low` | Master's Crate (Beginner) | Low |
| `masters_crate_mid` | Master's Crate (Intermediate) | Mid |
| `masters_crate_high` | Master's Crate (Advanced) | High |

All: stackable, bankable, not sellable to shops. Category: "misc" or "crate".

## Reward Integration

In `handle_claim_crafting_order`:
- Determine bracket from `template.min_level` (1-19 = low, 20-39 = mid, 40+ = high)
- Add 1x appropriate crate to player inventory alongside existing gold/XP/marks rewards
- Regular orders → artisans_crate_{bracket}
- Masterwork orders → masters_crate_{bracket}

## Loot Table Data Format

```toml
# rust-server/data/crate_loot/artisans_crate.toml

[rarity_weights]
common = 60
uncommon = 30
rare = 10

[[common.low]]
item_id = "copper_ore"
quantity_min = 15
quantity_max = 25

[[common.low]]
item_id = "oak_log"
quantity_min = 10
quantity_max = 20

# ... etc
```

## File Targets

- `rust-server/data/items/crates.toml` — 6 crate item definitions
- `rust-server/data/crate_loot/artisans_crate.toml` — Artisan's Crate loot tables
- `rust-server/data/crate_loot/masters_crate.toml` — Master's Crate loot tables
- `rust-server/src/game/crate_loot.rs` — Loot table loading, roll logic, crate opening handler
- `rust-server/src/game/crafting_orders.rs` — Add crate to claim rewards
- Wire crate opening into the UseItem handler in game.rs
