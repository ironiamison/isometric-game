# Quest Panel Redesign — OSRS-Style Quest List

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the current "active quests only" quest log with a full OSRS-style quest list panel showing all quests color-coded by status, with a detail view on click.

**Architecture:** New `QuestCatalog` server message sends all quest definitions on login. Client renders a scrollable list of all quests (red/yellow/green) in the same-sized panel as inventory. Clicking a quest swaps the panel content to a detail view with description, requirements, and objectives (if active).

**Tech Stack:** Rust server (Axum/Tokio, MessagePack protocol), Rust client (Macroquad)

---

### Task 1: Server — Add QuestCatalog to ServerMessage enum

**Files:**
- Modify: `rust-server/src/protocol.rs:444-463` (add variant after QuestStateSync)
- Modify: `rust-server/src/protocol.rs:1053-1061` (add QuestCatalogEntryData struct near QuestObjectiveData)
- Modify: `rust-server/src/protocol.rs:1140-1143` (add msg_type match arm)

**Step 1: Add the data struct**

Add after `QuestObjectiveData` struct (line ~1061):

```rust
/// Quest catalog entry for sending all quest info to client
#[derive(Debug, Clone, Serialize)]
pub struct QuestCatalogEntryData {
    pub quest_id: String,
    pub name: String,
    pub description: String,
    pub giver_npc_name: String,
    pub level_required: i32,
    pub required_quest_id: Option<String>,
    pub required_quest_name: Option<String>,
}
```

**Step 2: Add ServerMessage variant**

Add after `QuestStateSync` variant (line ~463):

```rust
QuestCatalog {
    quests: Vec<QuestCatalogEntryData>,
},
```

**Step 3: Add msg_type match arm**

In `msg_type()` function, add after the QuestStateSync arm (line ~1143):

```rust
ServerMessage::QuestCatalog { .. } => "questCatalog",
```

**Step 4: Add MessagePack encoding**

Add encoding after the `QuestStateSync` encoding block (after line ~2435). Follow the same `Value::Map` pattern used by other quest messages:

```rust
ServerMessage::QuestCatalog { quests } => {
    let mut map = Vec::new();
    let quest_values: Vec<Value> = quests
        .iter()
        .map(|q| {
            let mut qmap = Vec::new();
            qmap.push((
                Value::String("quest_id".into()),
                Value::String(q.quest_id.clone().into()),
            ));
            qmap.push((
                Value::String("name".into()),
                Value::String(q.name.clone().into()),
            ));
            qmap.push((
                Value::String("description".into()),
                Value::String(q.description.clone().into()),
            ));
            qmap.push((
                Value::String("giver_npc_name".into()),
                Value::String(q.giver_npc_name.clone().into()),
            ));
            qmap.push((
                Value::String("level_required".into()),
                Value::Integer((q.level_required as i64).into()),
            ));
            if let Some(ref req_id) = q.required_quest_id {
                qmap.push((
                    Value::String("required_quest_id".into()),
                    Value::String(req_id.clone().into()),
                ));
            }
            if let Some(ref req_name) = q.required_quest_name {
                qmap.push((
                    Value::String("required_quest_name".into()),
                    Value::String(req_name.clone().into()),
                ));
            }
            Value::Map(qmap)
        })
        .collect();
    map.push((Value::String("quests".into()), Value::Array(quest_values)));
    Value::Map(map)
}
```

**Step 5: Build and verify**

Run: `cd rust-server && cargo check 2>&1 | head -20`
Expected: No new errors (may have pre-existing warnings)

**Step 6: Commit**

```bash
git add rust-server/src/protocol.rs
git commit -m "feat: add QuestCatalog server message for quest panel redesign"
```

---

### Task 2: Server — Build and send QuestCatalog on login

**Files:**
- Modify: `rust-server/src/game.rs:1724-1733` (add new method after get_completed_quest_sync_message)
- Modify: `rust-server/src/main.rs:1670-1673` (send catalog after QuestStateSync)

**Step 1: Add catalog builder method to GameRoom**

Add after `get_completed_quest_sync_message()` in `game.rs` (after line ~1733):

```rust
/// Build the full quest catalog for sending to client on login
pub async fn build_quest_catalog(&self) -> ServerMessage {
    let all_quests = self.quest_registry.all_quests().await;
    let npcs = self.npcs.read().await;

    // Build a map of prototype_id -> display_name from loaded NPCs
    let npc_names: HashMap<String, String> = npcs
        .values()
        .map(|npc| (npc.prototype_id.clone(), npc.stats.display_name.clone()))
        .collect();

    let mut entries: Vec<QuestCatalogEntryData> = Vec::new();
    for quest in &all_quests {
        let giver_npc_name = npc_names
            .get(&quest.giver_npc)
            .cloned()
            .unwrap_or_else(|| quest.giver_npc.clone());

        // Resolve prerequisite quest name
        let (required_quest_id, required_quest_name) = if let Some(ref prev_id) = quest.chain.previous {
            let prev_name = all_quests
                .iter()
                .find(|q| q.id == *prev_id)
                .map(|q| q.name.clone());
            (Some(prev_id.clone()), prev_name)
        } else {
            (None, None)
        };

        entries.push(QuestCatalogEntryData {
            quest_id: quest.id.clone(),
            name: quest.name.clone(),
            description: quest.description.clone(),
            giver_npc_name,
            level_required: quest.level_required,
            required_quest_id,
            required_quest_name,
        });
    }

    ServerMessage::QuestCatalog { quests: entries }
}
```

Note: You'll need to add `use std::collections::HashMap;` at the top if not already imported, and import `QuestCatalogEntryData` from protocol.

**Step 2: Send catalog on login**

In `main.rs`, add after the QuestStateSync send block (after line ~1673):

```rust
// Send full quest catalog for the quest panel
let quest_catalog = room.build_quest_catalog().await;
if let Ok(bytes) = protocol::encode_server_message(&quest_catalog) {
    let _ = sender.send(Message::Binary(bytes)).await;
}
```

**Step 3: Build and verify**

Run: `cd rust-server && cargo check 2>&1 | head -20`
Expected: No new errors

**Step 4: Commit**

```bash
git add rust-server/src/game.rs rust-server/src/main.rs
git commit -m "feat: build and send QuestCatalog on player login"
```

---

### Task 3: Client — Add QuestCatalog state and message handling

**Files:**
- Modify: `client/src/game/state.rs:1034-1040` (add catalog + selected_quest state)
- Modify: `client/src/game/state.rs:1200-1206` (initialize new fields)
- Modify: `client/src/network/message_handler.rs:1530-1541` (add questCatalog handler after questStateSync)

**Step 1: Add structs and state fields**

In `state.rs`, add a new struct near the existing `ActiveQuest` / `QuestObjective` structs:

```rust
/// A quest from the server catalog (static info for all quests)
pub struct QuestCatalogEntry {
    pub quest_id: String,
    pub name: String,
    pub description: String,
    pub giver_npc_name: String,
    pub level_required: i32,
    pub required_quest_id: Option<String>,
    pub required_quest_name: Option<String>,
}
```

Add to `UiState` fields (after `quest_log_scroll` at line ~1040):

```rust
pub quest_catalog: Vec<QuestCatalogEntry>,
pub selected_quest_id: Option<String>,
```

Initialize in `UiState::default()` / `new()` (after quest_log_scroll init at line ~1206):

```rust
quest_catalog: Vec::new(),
selected_quest_id: None,
```

**Step 2: Handle the message**

In `message_handler.rs`, add a new handler after the `"questStateSync"` block (after line ~1541):

```rust
"questCatalog" => {
    if let Some(value) = data {
        state.ui_state.quest_catalog.clear();
        if let Some(quests) = extract_array(value, "quests") {
            for q in quests {
                let quest_id = extract_string(q, "quest_id").unwrap_or_default();
                let name = extract_string(q, "name").unwrap_or_default();
                let description = extract_string(q, "description").unwrap_or_default();
                let giver_npc_name = extract_string(q, "giver_npc_name").unwrap_or_default();
                let level_required = extract_i32(q, "level_required").unwrap_or(0);
                let required_quest_id = extract_string(q, "required_quest_id");
                let required_quest_name = extract_string(q, "required_quest_name");
                state.ui_state.quest_catalog.push(QuestCatalogEntry {
                    quest_id,
                    name,
                    description,
                    giver_npc_name,
                    level_required,
                    required_quest_id,
                    required_quest_name,
                });
            }
        }
        log::info!("Received quest catalog with {} quests", state.ui_state.quest_catalog.len());
    }
}
```

Make sure to import `QuestCatalogEntry` at the top of the file.

**Step 3: Build and verify**

Run: `cd client && cargo check 2>&1 | head -20`
Expected: No new errors

**Step 4: Commit**

```bash
git add client/src/game/state.rs client/src/network/message_handler.rs
git commit -m "feat: add QuestCatalogEntry state and message handler on client"
```

---

### Task 4: Client — Rewrite quest list view (render_quest_log)

**Files:**
- Modify: `client/src/render/ui/quest.rs:47-428` (rewrite render_quest_log)

This is the big UI rewrite. Replace the current `render_quest_log()` which only shows active quests with a new version that shows ALL quests from the catalog, color-coded.

**Step 1: Rewrite render_quest_log**

Replace the entire `render_quest_log` method. The new version should:

1. **Panel frame:** Same size as inventory (`INV_WIDTH` x `INV_HEIGHT` = 240x322), positioned right side above menu buttons. Use `draw_panel_frame` and `draw_corner_accents`.

2. **Header:** "QUESTS" title with decorative separator dots (same style as current).

3. **If `selected_quest_id` is Some:** Delegate to `render_quest_detail()` (Task 5) and return early.

4. **Content area:** Scrollable list of quest names from `quest_catalog`.

5. **Determine quest status for each catalog entry:**
   - If `completed_quest_ids.contains(&entry.quest_id)` → Green (completed)
   - Else if `active_quests.iter().any(|q| q.id == entry.quest_id)` → Yellow (in progress)
   - Else → Red (not started)

6. **Sort quests:** Group by status (yellow first, red second, green last), alphabetically within each group.

7. **Render each quest name** as a single clickable text line:
   - Color: `Color::new(1.0, 0.843, 0.0, 1.0)` for yellow (in-progress), `Color::new(1.0, 0.267, 0.267, 1.0)` for red (not started), `Color::new(0.0, 0.8, 0.0, 1.0)` for green (completed)
   - Register `UiElementId::QuestLogEntry(idx)` for click/hover detection
   - Hover highlight with `SLOT_HOVER_BG`
   - Word-wrap long names

8. **Scrollbar:** Same scrollbar logic as current implementation (scissor clip + thumb).

9. **Footer:** `"X / Y Complete"` where X = completed count, Y = total catalog count.

Key rendering constants to use:
- `line_height = 17.0 * s`
- Content inset: `8.0 * s` padding inside the panel frame
- Use `self.draw_text_sharp()` for all text
- Use `self.wrap_text()` for word wrapping

**Step 2: Build and verify**

Run: `cd client && cargo check 2>&1 | head -20`
Expected: No new errors

**Step 3: Commit**

```bash
git add client/src/render/ui/quest.rs
git commit -m "feat: rewrite quest list to show all quests with color-coded status"
```

---

### Task 5: Client — Add quest detail view

**Files:**
- Modify: `client/src/render/ui/quest.rs` (add render_quest_detail method)

**Step 1: Add the detail render method**

Add a new method `render_quest_detail` to the `Renderer` impl block. This is called from `render_quest_log` when `selected_quest_id` is `Some`.

Parameters: `&self, state: &GameState, hovered: &Option<UiElementId>, layout: &mut UiLayout, content_x, content_y, content_w, content_h` (the content area rect passed from render_quest_log).

Layout (top to bottom, scrollable):

1. **"< Back" button** — Use a new `UiElementId::QuestDetailBack` (you'll need to add this to the UiElementId enum). Draw in `TEXT_DIM` color, `TEXT_TITLE` on hover.

2. **Quest name** — In status color (red/yellow/green), word-wrapped.

3. **Horizontal separator line** — thin line in `SLOT_BORDER` color.

4. **Description** — Quest description text in `TEXT_DIM`, word-wrapped.

5. **Another separator.**

6. **Requirements section:**
   - `"Start: "` label in `TEXT_DIM`, NPC name in `TEXT_NORMAL`
   - `"Level: X"` in `TEXT_DIM` (only if level_required > 0)
   - `"Requires: [Quest Name]"` — colored green if that quest is in completed_quest_ids, red if not (only if required_quest_id is Some)

7. **Objectives section** (only if quest is active — found in `active_quests`):
   - Another separator
   - `"Objectives:"` label
   - Each objective with `[ ]`/`[+]` checkbox and `(current/target)` count
   - Same styling as current quest log objectives

Use scissor clipping + scrollbar if content overflows, same pattern as list view. Use a separate scroll offset (can reuse `quest_log_scroll` since list and detail don't show simultaneously).

**Step 2: Add UiElementId variants**

In the file that defines `UiElementId` (likely `client/src/ui/mod.rs` or similar), add:

```rust
QuestDetailBack,
```

**Step 3: Build and verify**

Run: `cd client && cargo check 2>&1 | head -20`
Expected: No new errors

**Step 4: Commit**

```bash
git add client/src/render/ui/quest.rs client/src/ui/
git commit -m "feat: add quest detail view with description, requirements, and objectives"
```

---

### Task 6: Client — Input handling for quest list clicks and detail back

**Files:**
- Modify: `client/src/input/handler.rs` (quest log click handling and back button)

**Step 1: Handle quest entry click → open detail view**

Find the section that handles `UiElementId::QuestLogEntry` clicks (if it exists) or add handling in the menu button click section. When a `QuestLogEntry(idx)` is clicked:

```rust
UiElementId::QuestLogEntry(idx) => {
    audio.play_sfx("enter");
    // Map idx back to quest_id using the sorted catalog order
    // Build the same sorted list as render_quest_log uses
    let mut sorted_quests: Vec<&QuestCatalogEntry> = state.ui_state.quest_catalog.iter().collect();
    sorted_quests.sort_by(|a, b| {
        let status_a = quest_status_order(a, &state.ui_state);
        let status_b = quest_status_order(b, &state.ui_state);
        status_a.cmp(&status_b).then(a.name.cmp(&b.name))
    });
    if let Some(entry) = sorted_quests.get(idx) {
        state.ui_state.selected_quest_id = Some(entry.quest_id.clone());
        state.ui_state.quest_log_scroll = 0.0; // reset scroll for detail view
    }
}
```

You'll need a helper function `quest_status_order` that returns 0 for in-progress, 1 for not-started, 2 for completed. This helper should be shared between the renderer and input handler — consider putting it as a method on `UiState` or as a free function in `state.rs`.

**Step 2: Handle back button click**

```rust
UiElementId::QuestDetailBack => {
    audio.play_sfx("enter");
    state.ui_state.selected_quest_id = None;
    state.ui_state.quest_log_scroll = 0.0;
}
```

**Step 3: Reset selected_quest_id when closing panel**

In every place that sets `quest_log_open = false`, also set `selected_quest_id = None`. There are ~15 such places in handler.rs. The simplest approach: add a helper method to UiState:

```rust
pub fn close_quest_log(&mut self) {
    self.quest_log_open = false;
    self.quest_log_scroll = 0.0;
    self.selected_quest_id = None;
}
```

Then replace all `state.ui_state.quest_log_open = false;` with `state.ui_state.close_quest_log();`

**Step 4: Build and verify**

Run: `cd client && cargo check 2>&1 | head -20`
Expected: No new errors

**Step 5: Commit**

```bash
git add client/src/input/handler.rs client/src/game/state.rs
git commit -m "feat: add quest list click-to-detail and back button input handling"
```

---

### Task 7: Integration test — Build both and verify

**Step 1: Build server**

Run: `cd rust-server && cargo build 2>&1 | tail -5`
Expected: Compiles successfully

**Step 2: Build client**

Run: `cd client && cargo build 2>&1 | tail -5`
Expected: Compiles successfully

**Step 3: Commit any fixes**

If any compilation errors were found in steps 1-2, fix them and commit:

```bash
git add -A
git commit -m "fix: resolve compilation issues in quest panel redesign"
```
