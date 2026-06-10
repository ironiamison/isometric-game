# Collection Log Popup UI Rework

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the tabbed quest/collection panel with a standalone popup panel featuring item icon grids and a category sidebar, similar to OSRS's collection log.

**Architecture:** Client-only UI rework. Revert quest panel to original, create new standalone popup panel with sidebar + grid layout, add keyboard shortcut and quest panel link.

**Tech Stack:** Rust client (Macroquad), immediate-mode UI rendering

---

### Task 1: Revert quest panel to original

**Files:**
- Modify: `client/src/render/ui/quest.rs`
- Modify: `client/src/ui/layout.rs`
- Modify: `client/src/input/handler.rs`

Remove the tab system from the quest panel:
- Restore the original "Quests" title header (replace the two-tab header)
- Remove the `render_collection_log_content` and `draw_collection_back_button` methods
- Remove `QuestsTab`, `CollectionLogTab`, `CollectionLogCategory`, `CollectionLogSubcategory`, `CollectionLogBack`, `CollectionLogScrollArea`, `CollectionLogScrollbar` from UiElementId
- Remove their click handlers from input/handler.rs
- Keep the `collection_tab_active` field removal for later (Task 2 handles state)

Add a "Collection Log" clickable link in the quest panel footer, next to the completion count. Clicking it opens the collection log popup (sets `collection_log_open = true`) and closes the quest panel.

Add UiElementId variant: `CollectionLogLink`

### Task 2: Update client state fields

**Files:**
- Modify: `client/src/game/state.rs`

Replace the tab-based collection state with popup state:
- Remove: `collection_tab_active`
- Add: `collection_log_open: bool`
- Rename: `collection_category` -> `collection_log_selected_category: Option<String>`
- Rename: `collection_subcategory` -> `collection_log_selected_subcategory: Option<String>`
- Rename: `collection_scroll` -> `collection_log_sidebar_scroll: f32`
- Add: `collection_log_grid_scroll: f32`
- Keep: `collection_log_definitions`, `collection_log`, `collection_scroll_drag`

Update `close_quest_log()` to no longer reset collection state.
Add `close_collection_log()` method that resets all collection state.

### Task 3: Create the popup panel renderer

**Files:**
- Create: `client/src/render/ui/collection_log.rs`
- Modify: `client/src/render/ui/mod.rs` (add module)
- Modify: `client/src/render/renderer.rs` (call render_collection_log)

Create a new file `collection_log.rs` with the popup panel rendering:

**Panel frame:**
- Centered on screen
- Width: `min(480.0 * s, sw - 32.0)`, Height: `min(360.0 * s, sh - 64.0)`
- Use `draw_panel_frame` + `draw_corner_accents` (same as other panels)
- Header with "Collection Log" title and close button (X)
- Footer with "N / M Collected" total

**Left sidebar (35% width):**
- Scrollable category tree
- 4 top-level categories: Monster Drops, Boss Rewards, Skilling, Quest Rewards
- Each category is clickable to expand/collapse, showing subcategories indented
- Each entry shows "name (got/total)" with completion coloring
- Selected subcategory is highlighted
- Scroll support for long lists

**Right grid area (65% width):**
- When a subcategory is selected: show item icon grid
- Grid of square slots (32x32 or similar, scaled)
- Obtained items: render via `self.draw_item_icon(item_id, x, y, slot_w, slot_h, state, false)`
- Unobtained items: draw empty dark slot (SLOT_BG_EMPTY with SLOT_BORDER)
- Hover over obtained item: show item name tooltip
- Scroll support for grids with many items
- When no subcategory selected: show "Select a category" placeholder text

**UiElementId variants needed:**
- `CollectionLogClose`
- `CollectionLogCategoryHeader(usize)` — expand/collapse
- `CollectionLogSubcategoryEntry(usize)` — select subcategory
- `CollectionLogSidebarScrollArea`
- `CollectionLogSidebarScrollbar`
- `CollectionLogGridScrollArea`
- `CollectionLogGridScrollbar`
- `CollectionLogGridItem(usize)` — for hover/tooltip

### Task 4: Input handling + keyboard shortcut

**Files:**
- Modify: `client/src/input/handler.rs`

Add click handlers for all new UiElementId variants:
- `CollectionLogLink` — open popup, close quest panel
- `CollectionLogClose` — close popup
- `CollectionLogCategoryHeader(idx)` — toggle expand/collapse of category
- `CollectionLogSubcategoryEntry(idx)` — select subcategory, reset grid scroll
- Grid scroll area mouse wheel handling
- Sidebar scroll area mouse wheel handling

Add keyboard shortcut:
- `V` key: toggle `collection_log_open`
- `Escape`: close collection log if open
- Opening collection log closes other panels (inventory, quest, etc.)

### Task 5: Mutual exclusivity + integration

**Files:**
- Modify: `client/src/input/handler.rs`

Ensure:
- Opening collection log closes inventory, quest panel, crafting, etc.
- Opening other panels closes collection log
- Escape closes collection log
- Collection log link in quest footer works correctly
