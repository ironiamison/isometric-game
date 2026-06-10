# Master Crafting Orders — Design

## Overview

A crafting commission system accessed via a new "Orders" tab on the existing Adventure Board. Players accept orders to craft and deliver items for gold, XP, and a special currency (Commission Marks). Two tiers — regular (single-skill) and masterwork (multi-skill) — serve both casual and dedicated skillers.

This feature also introduces the player title system (previously designed but unimplemented) as the primary prestige reward, generalized to support titles from any source.

## Core Loop

1. Player opens Adventure Board → "Orders" tab
2. Sees 3–5 daily orders filtered by their skill levels
3. Accepts one order at a time
4. Crafts/gathers required items through normal gameplay
5. Returns to any Adventure Board to turn in
6. Receives gold + XP (regular) or gold + XP + Commission Marks (masterwork)
7. Spends marks at a Master Artisan NPC for prestige rewards

## Order Tiers

### Regular Orders

- Single-skill requirement: "Deliver 30 Oak Planks" or "Deliver 10 Strength Potions"
- Skill level gate: orders scale to the player's level in that skill
- Reward: gold + XP in the relevant skill
- Available to all players with any trained crafting/gathering skill

### Masterwork Orders

- Multi-skill chains: "Deliver 5 Mithril Longswords" (mining → smelting → smithing)
- Unlocked at 40+ in the primary skill
- 2-skill chains award 2 Commission Marks
- 3-skill chains award 4 Commission Marks
- Reward: gold + XP across involved skills + Commission Marks

## Order Data Format

Order templates are TOML data files, one per skill family.

```toml
# rust-server/data/orders/smithing.toml
[[orders]]
id = "smith_iron_daggers"
tier = "regular"
skill = "smithing"
min_level = 15
items = [{ id = "iron_dagger", quantity = 20 }]
rewards = { gold = 500, xp = { smithing = 300 } }

[[orders]]
id = "masterwork_mithril_longswords"
tier = "masterwork"
skill = "smithing"
min_level = 40
items = [{ id = "mithril_longsword", quantity = 5 }]
rewards = { gold = 2000, xp = { smithing = 800, mining = 400 }, marks = 3 }
```

Skill families with orders: smithing, alchemy, fletching, leatherworking, cooking, mining, woodcutting, fishing.

## Daily Generation

- On daily reset (or first login after reset), server selects 3–5 orders from the pool filtered by the player's skill levels
- At least 1 regular and 1 masterwork (if eligible) guaranteed
- Orders are per-player (no shared competition for items)
- Unaccepted orders refresh next day
- An accepted order persists until completed or abandoned

## Player State

Stored in SQLite alongside existing contract data:

```sql
CREATE TABLE IF NOT EXISTS crafting_orders_available (
    character_id INTEGER NOT NULL,
    order_id TEXT NOT NULL,
    generated_date TEXT NOT NULL,
    PRIMARY KEY (character_id, order_id),
    FOREIGN KEY(character_id) REFERENCES characters(id)
);

CREATE TABLE IF NOT EXISTS crafting_orders_active (
    character_id INTEGER PRIMARY KEY,
    order_id TEXT NOT NULL,
    accepted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(character_id) REFERENCES characters(id)
);

ALTER TABLE characters ADD COLUMN commission_marks INTEGER DEFAULT 0;
```

No partial progress tracking — turn-in checks inventory for required items, removes them, grants rewards.

## Adventure Board Integration

- New "Orders" tab alongside existing board content
- Shows available orders with: skill icon, item requirements, tier badge (regular/masterwork), rewards
- Accepted order shows at top with a checkmark indicator when items are ready
- Abandon button on the accepted order (no penalty, order returns to available pool)

## Commission Marks & Master Artisan NPC

A "Master Artisan" NPC placed near the village crafting area. Uses existing shop UI priced in Commission Marks.

### Prestige Shop

| Item | Cost | Type |
|------|------|------|
| "Apprentice Artisan" title | 10 marks | Title |
| "Master Smith" title | 30 marks | Skill-specific title |
| "Master Alchemist" title | 30 marks | Skill-specific title |
| "Master Fletcher" title | 30 marks | Skill-specific title |
| "Master Chef" title | 30 marks | Skill-specific title |
| "Grandmaster Artisan" title | 100 marks | All-skills title |
| Gilded tool skins | 20 marks | Cosmetic |
| Artisan's Cape | 50 marks | Cosmetic equipment |
| Bonus order slot (+1 active) | 40 marks | Gameplay unlock |

Marks are scarce: ~2–4 per masterwork order, roughly 1 prestige unlock per week of active play.

## Player Title System

Generalizes the previously designed arena title system to support titles from any source.

### Database

```sql
CREATE TABLE IF NOT EXISTS player_titles (
    character_id INTEGER NOT NULL,
    title_id TEXT NOT NULL,
    unlocked_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (character_id, title_id),
    FOREIGN KEY(character_id) REFERENCES characters(id)
);

ALTER TABLE characters ADD COLUMN active_title TEXT DEFAULT NULL;
```

### Title Display

- Format: `PlayerName (Title)` suffix style
- Admins: `PlayerName (GM)(Title)` if both present
- Rendered overhead in name tags (below name, same font, slightly smaller or dimmed)
- Rendered in chat: `PlayerName (Title): message text`

### Wire Format

- `PlayerUpdate` gains `title: Option<String>` (display text, not ID)
- `Player` struct gains `active_title: Option<String>` loaded at login
- Title is sent each tick as part of normal state sync — no extra messages needed

### Chat Commands

- `/title list` — show all unlocked titles
- `/title set <title_id>` — equip a title
- `/title clear` — remove active title

### Title Definitions

Crafting titles are purchased from the Master Artisan (see shop above). Arena titles unlock automatically from milestones (as in the original design). Both use the same `player_titles` table.

| Source | Title ID | Display Text | Condition |
|--------|----------|-------------|-----------|
| Crafting | `artisan_apprentice` | Apprentice Artisan | Purchase (10 marks) |
| Crafting | `master_smith` | Master Smith | Purchase (30 marks) |
| Crafting | `master_alchemist` | Master Alchemist | Purchase (30 marks) |
| Crafting | `master_fletcher` | Master Fletcher | Purchase (30 marks) |
| Crafting | `master_chef` | Master Chef | Purchase (30 marks) |
| Crafting | `grandmaster_artisan` | Grandmaster Artisan | Purchase (100 marks) |
| Arena | `arena_novice` | Brawler | 1 arena win |
| Arena | `arena_fighter` | Fighter | 10 arena wins |
| Arena | `arena_veteran` | Veteran | 50 arena wins |
| Arena | `arena_champion` | Champion | 100 arena wins |
| Arena | `arena_legend` | Legend | 250 arena wins |

### Overhead Rendering

In `render_name_tags` (`client/src/render/renderer.rs:5419`):
- After drawing player name + level, check for title
- If title present, append ` (Title)` to the name string in a slightly different color (e.g., gold for masterwork titles, white for standard)
- No layout changes needed — title is part of the name string

### Chat Rendering

In chat message construction (`client/src/game/state.rs:306`):
- `sender_name` includes title when present: `"PlayerName (Title)"`
- Server sends the formatted name; client doesn't need title logic for chat

## File Targets

### Server

- `rust-server/data/orders/*.toml` — order templates per skill
- `rust-server/src/game/crafting_orders.rs` — order generation, acceptance, turn-in logic
- `rust-server/src/game/titles.rs` — title DB operations, `/title` command handling
- `rust-server/src/game.rs` — Player struct gains `active_title`, `commission_marks`
- `rust-server/src/db.rs` — new tables, migrations
- `rust-server/src/protocol.rs` — PlayerUpdate gains `title` field

### Client

- `client/src/render/renderer.rs` — title in name tags
- `client/src/game/state.rs` — ChatMessage includes title in sender name
- `client/src/game/entities.rs` — Player struct gains `title` field
- Adventure Board UI — new Orders tab (wherever board UI currently lives)

## Success Criteria

- A player with trained crafting skills can pick up an order from the Adventure Board and turn it in for rewards
- Masterwork orders require outputs from multiple skills and award Commission Marks
- Commission Marks can be spent at the Master Artisan for titles and cosmetics
- Titles display overhead and in chat for all players to see
- `/title` commands let players manage their active title
- The title system is generic enough to support arena titles and future title sources
