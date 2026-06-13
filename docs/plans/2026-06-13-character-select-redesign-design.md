# Character Select Screen Redesign — Design

**Date:** 2026-06-13
**Status:** Approved, ready for implementation

## Goal

Redesign the character select (roster) screen to match the recently-redesigned
login screen's **bronze-bevel medieval-fantasy theme** (commit 528dca4c), while
adopting the improved layout/structure from the provided mockups: a card-based
roster, level chips, a persistent "create" row, an attached action bar, and a
welcoming empty state.

## Constraints / Data Availability

Client-side `CharacterInfo` (`client/src/auth/types.rs`) provides per character:
`id, name, level, gender, skin, hair_style, hair_color, played_time`, plus
equipment + sprite keys for the portrait.

**Not available:** last-location / zone / region name. The server stores raw
coords + an optional interior `current_map` id, but there is no
coordinate→region-name system. **Decision: drop the location line for now**
(no fake data); revisit when a zone-name system exists.

## Visual Direction

- Keep the existing **bronze multi-layer frame** + **gold L-bracket corner
  accents** from the login redesign.
- Reuse the existing **monogram BitmapFont** at native pixel size.
- Reuse login's **animated night-sky background** (twinkling + shooting stars)
  for a seamless login → character-select transition.
- Colors from `render/ui/common.rs` constants (FRAME_*, TEXT_*, PANEL_BG_*),
  plus a few new ones (danger red-brown button, level-chip ramp).

## Layout

### Header (above the framed panel)
- Left: small person glyph + username (e.g. `test`).
- Center: gold `SELECT CHARACTER` title (monogram).

### Main framed panel (bronze frame + corner accents)
Wraps a vertical, scrollable roster list. Last item is always a persistent
**"+ Create new character"** row.

### Action bar (attached below the panel)
Conditional on whether characters exist (see States).

### Hint line (very bottom, dim text)
- With characters: `[W/S] navigate · [Enter] play · [N] new`
- Empty: `[N] create character`

## Character Card

- **Portrait** (left): composite character sprite via existing
  `draw_character_preview` / sprite layers, in a recessed dark square with a
  thin bronze edge. Native sprite size, no scaling blur.
- **Name** (gold, monogram), top line.
- **Meta line**: `[Lv N]` chip + `<Gender> <Race/Skin>` (e.g. `Male Tan`).
  Chip is a rounded-rect whose fill tints along a **bronze→gold ramp by level**
  (low = dark bronze → high = bright gold).
- **Played time** (right-aligned): `9h 51m` over a dim `played` label, formatted
  from `played_time` seconds (`1m`, `9h 51m`, `149h 44m`).

### Card states
- **Selected/focused**: gold-lit — bright gold border + faint warm inner glow.
  Serves as both keyboard cursor and click selection. (Not flat blue.)
- **Hover** (mouse): subtle bronze border brighten.
- **Default**: dark recessed fill + faint bronze edge.

### "+ Create new character" row
Same footprint, dashed/faint bronze outline, centered `+ Create new character`
in dim-gold — visually reads as an action, not a hero.

### Scrolling
List scrolls (wheel + touch drag) when cards overflow; preserve existing
scrollbar. Focus auto-scrolls to stay visible; focus wraps.

## Action Bar

When characters exist — three buttons (gold-on-brown button style):
- **▶ Play** — primary, emphasized; default action.
- **🗑 Delete** — restrained **danger** variant: dark red-brown fill, muted red
  border/text (not a loud saturated block). Opens existing confirm dialog,
  re-skinned to match (bronze frame, danger-red confirm).
- **⎋ Logout** — neutral navy/dark fill, bronze border, dim text.

When empty — only **Logout** (creation is handled by the in-panel button).

## Empty State (new account, zero characters)

Framed panel stays; interior becomes a centered invitation:
- Dashed-circle "add user" glyph.
- Gold headline `Your story begins here`.
- Dim subtext: `No heroes yet. Create your first character to set foot in the
  realm of Aeven.`
- One prominent **`+ Create Character`** button.

## Interaction (preserve current behavior)

- **Keyboard**: `W`/`S` or `↑`/`↓` move focus through cards incl. the Create
  row; `Enter` plays selected (or triggers Create if that row focused); `N`
  create; `Delete`/`X` delete confirm; `Esc` logout.
- **Mouse**: click selects; double-click plays; click Create row/buttons act
  directly; hover states; wheel scroll.
- **Touch**: drag-scroll, tap to select/act.

## Implementation Notes

- Work lives in `client/src/ui/screens/character_select.rs` (`render` +
  `update`), reusing existing struct fields.
- **Extract login's frame / corner-accent / button / animated-background
  drawing into shared helpers** (in `render/ui/common.rs` or a shared module) so
  login and character-select share one source of truth and don't drift.
- New local helpers: `format_played_time(secs)`, `level_chip_color(level)`
  (bronze→gold ramp), `draw_character_card(...)`.
- Add new theme constants for the danger red-brown button and the chip ramp.

## Out of Scope (YAGNI)

- Region/zone-name system for the location line.
- Server protocol changes (no new fields needed).
