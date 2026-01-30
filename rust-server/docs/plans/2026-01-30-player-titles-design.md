# Player Titles & Arena Retention - Design

## Overview

A player title system tied to arena achievements. Titles display as a suffix on player names (e.g. `Nyx (Champion)`). Players unlock titles by hitting milestones and choose which to display.

## Title Display

- Format: `PlayerName (Title)` — suffix style
- Admins show `(GM)` as before; if admin + title, show both: `Nyx (GM)(Champion)`
- Title is sent as `Option<String>` on `PlayerUpdate` each tick
- Client appends it to the nameplate if present

## Data Model

### New table: `player_titles`

```sql
CREATE TABLE IF NOT EXISTS player_titles (
    character_id INTEGER NOT NULL,
    title_id TEXT NOT NULL,
    unlocked_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (character_id, title_id),
    FOREIGN KEY(character_id) REFERENCES characters(id)
);
```

### Characters table addition

```sql
ALTER TABLE characters ADD COLUMN active_title TEXT DEFAULT NULL;
```

## Title Definitions

| Title ID | Display Text | Condition |
|----------|-------------|-----------|
| `arena_novice` | Brawler | 1 arena win |
| `arena_fighter` | Fighter | 10 arena wins |
| `arena_veteran` | Veteran | 50 arena wins |
| `arena_champion` | Champion | 100 arena wins |
| `arena_legend` | Legend | 250 arena wins |
| `arena_slayer` | Slayer | 100 arena kills |
| `arena_executioner` | Executioner | 500 arena kills |
| `arena_streak_3` | Hot Streak | 3 win streak (best_streak) |
| `arena_streak_10` | Unstoppable | 10 win streak (best_streak) |
| `arena_rich` | High Roller | 10,000 total gold won |

## Unlock Flow

1. Match ends, `update_arena_stats` saves to DB
2. Read back updated stats for each participant
3. Check each title threshold against stats
4. Insert newly unlocked titles into `player_titles`
5. Send system message: `"Title unlocked: Champion! Use /title set arena_champion to equip it."`

## Chat Commands

- `/title list` — Show all unlocked titles
- `/title set <title_id>` — Equip a title
- `/title clear` — Remove active title
- `/title` — Show current title and usage help

## Wire Format

`PlayerUpdate` gains: `title: Option<String>` (display text, not ID).

`Player` struct gains: `active_title: Option<String>` (display text, loaded at login).

## Future Retention Ideas

- **Daily First Win Bonus** — 2x gold on first arena win each day
- **Kill Streak Announcements** — "Nyx is on a RAMPAGE!" at 3+ kills in FFA
- **Arena Season Titles** — Monthly "January Champion" for top winner, permanent title
- **Escalating Entry Fee Matches** — Admin sets high-stakes matches
- **Win Streak Gold Multiplier** — Consecutive wins: 1x, 1.5x, 2x, 2.5x (capped), resets on loss
