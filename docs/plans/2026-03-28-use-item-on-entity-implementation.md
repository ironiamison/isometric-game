# Use Item on Entity — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add "use item on entity" mechanic — select an inventory item, click a world entity to use it on. Supports Lua quest callbacks and TOML-defined interactions.

**Architecture:** Client adds selected-item state to UiState, sends new `UseItemOn` message. Server resolves item + NPC, checks range, routes to quest Lua `on_use_item` callback first, then TOML fallback. Quest flags exposed to Lua for persistent state (candle puzzle tracking).

**Tech Stack:** Rust (client + server), Lua (quest scripts), TOML (item interactions), MessagePack (protocol)

**Design doc:** `docs/plans/2026-03-28-use-item-on-entity-design.md`

---

### Task 1: Protocol — Add UseItemOn Message

**Files:**
- Modify: `rust-server/src/protocol.rs`
- Modify: `client/src/network/messages.rs`

**Step 1: Add UseItemOn to server ClientMessage enum**

In `rust-server/src/protocol.rs`, find the `ClientMessage` enum (around line 40). Add a new variant after `UseItem`:

```rust
    UseItemOn { slot_index: u8, target_npc_id: String },
```

Also add the debug name in the `message_type_name` function (around line 419):

```rust
            ClientMessage::UseItemOn { .. } => "UseItemOn",
```

**Step 2: Add deserialization for UseItemOn**

In `rust-server/src/protocol.rs`, find the `decode_client_message` function's match on message type string (around line 6569). Add a new arm after the `"useItem"` arm:

```rust
            "useItemOn" => {
                let slot_index = msg_data
                    .as_map()
                    .and_then(|map| {
                        map.iter()
                            .find(|(k, _)| k.as_str() == Some("slot_index"))
                    })
                    .and_then(|(_, v)| v.as_u64().map(|u| u as u8))
                    .unwrap_or(0);
                let target_npc_id = extract_string(msg_data, "target_npc_id").unwrap_or_default();
                Ok(ClientMessage::UseItemOn {
                    slot_index,
                    target_npc_id,
                })
            }
```

**Step 3: Add UseItemOn to client ClientMessage enum**

In `client/src/network/messages.rs`, find the `ClientMessage` enum. Add:

```rust
    #[serde(rename = "useItemOn")]
    UseItemOn { slot_index: u32, target_npc_id: String },
```

**Step 4: Verify both compile**

Run: `cd rust-server && cargo check 2>&1 | tail -3`
Run: `cd client && cargo check 2>&1 | tail -3`

**Step 5: Commit**

```bash
git add rust-server/src/protocol.rs client/src/network/messages.rs
git commit -m "feat: add UseItemOn protocol message type"
```

---

### Task 2: Client — Item Selection State & Rendering

**Files:**
- Modify: `client/src/game/state.rs` (UiState struct)
- Modify: `client/src/input/handler.rs` (click handling)

**Step 1: Add selected_inventory_slot to UiState**

In `client/src/game/state.rs`, find the `UiState` struct (around line 1397). Add the field near the inventory-related fields:

```rust
    /// Which inventory slot is selected for "use on entity" targeting (None = no selection)
    pub selected_inventory_slot: Option<usize>,
```

In the `Default` impl (around line 1689), add:

```rust
            selected_inventory_slot: None,
```

**Step 2: Modify inventory left-click to toggle selection**

In `client/src/input/handler.rs`, find the `UiElementId::InventorySlot(idx)` left-click handler (around line 3240). The current behavior is: double-click equips/uses, first click starts drag.

Modify the **first click** (non-double-click) case. Instead of immediately starting a drag, toggle item selection:

Find the section where a single click on an inventory slot starts drag state. Before that drag logic, add:

```rust
// Single click: toggle item selection for "use on entity"
if state.ui_state.selected_inventory_slot == Some(*idx) {
    // Clicking selected slot again deselects
    state.ui_state.selected_inventory_slot = None;
    return commands;
} else if slot.item_id != "" {
    // Select this slot
    state.ui_state.selected_inventory_slot = Some(*idx);
    return commands;
}
```

Note: This replaces the drag-start on first click. Drag should still work on mouse-down-and-move (which is a separate code path). Read the existing code carefully — the drag state is likely set in the mouse-down handler, while the click is in the mouse-up handler. Make sure selection only happens on mouse-up (click), not mouse-down.

**Step 3: Clear selection on ESC and other deselect triggers**

Find where ESC is handled for the inventory (the general input section). Add:

```rust
// ESC clears item selection
if is_key_pressed(KeyCode::Escape) && state.ui_state.selected_inventory_slot.is_some() {
    state.ui_state.selected_inventory_slot = None;
    return commands;
}
```

Also clear selection when inventory closes, dialogue opens, or any major UI state change.

**Step 4: Visual feedback — highlight selected slot**

Find where inventory slots are rendered (likely in `client/src/render/ui/inventory.rs` or similar). Add a highlight/glow effect when `state.ui_state.selected_inventory_slot == Some(slot_index)`. This could be a colored border or overlay.

Search for where inventory slot backgrounds are drawn and add a conditional highlight:

```rust
if state.ui_state.selected_inventory_slot == Some(slot_index) {
    // Draw a highlight border or tinted overlay
    draw_rectangle(x, y, width, height, Color::new(1.0, 1.0, 0.0, 0.3)); // yellow tint
}
```

**Step 5: Verify client compiles**

Run: `cd client && cargo check 2>&1 | tail -3`

**Step 6: Commit**

```bash
git add client/src/game/state.rs client/src/input/handler.rs client/src/render/ui/inventory.rs
git commit -m "feat: add inventory item selection state and visual highlight"
```

---

### Task 3: Client — Use Item on Entity Click

**Files:**
- Modify: `client/src/input/handler.rs` (NPC click handling)
- Modify: `client/src/app.rs` (InputCommand to ClientMessage mapping)

**Step 1: Add UseItemOnEntity InputCommand**

In `client/src/input/handler.rs`, find the `InputCommand` enum (around line 1488). Add:

```rust
    UseItemOnEntity { slot_index: u8, npc_id: String },
```

**Step 2: Modify NPC click to check for selected item**

In `client/src/input/handler.rs`, find the friendly NPC interaction handler (around line 10420-10524). Before the existing `InputCommand::Interact` logic, check if an item is selected:

```rust
// If an inventory item is selected, use it on this NPC instead of interacting
if let Some(selected_slot) = state.ui_state.selected_inventory_slot {
    if dist_to_player < INTERACT_RANGE {
        commands.push(InputCommand::UseItemOnEntity {
            slot_index: selected_slot as u8,
            npc_id: npc_id.clone(),
        });
        state.ui_state.selected_inventory_slot = None; // Clear selection after use
    } else {
        // Walk to NPC first, then use item
        // Set auto_path with a new target type for "use item on arrival"
        // For now, require the player to be in range
        state.ui_state.selected_inventory_slot = None;
    }
    return commands;
}
```

Note: Full pathfind-then-use-item support can be added later. For now, require the player to be in range.

**Step 3: Clear selection when clicking empty ground**

Find where clicking on empty ground (no NPC, no object) is handled. Add:

```rust
if state.ui_state.selected_inventory_slot.is_some() {
    state.ui_state.selected_inventory_slot = None;
    return commands; // Don't move, just deselect
}
```

**Step 4: Map InputCommand to ClientMessage**

In `client/src/app.rs`, find where `InputCommand` variants are mapped to `ClientMessage` (around line 415-431). Add:

```rust
InputCommand::UseItemOnEntity { slot_index, npc_id } => {
    ClientMessage::UseItemOn {
        slot_index: *slot_index as u32,
        target_npc_id: npc_id.clone(),
    }
}
```

**Step 5: Verify client compiles**

Run: `cd client && cargo check 2>&1 | tail -3`

**Step 6: Commit**

```bash
git add client/src/input/handler.rs client/src/app.rs
git commit -m "feat: send UseItemOn message when clicking entity with selected item"
```

---

### Task 4: Server — UseItemOn Handler & Range Check

**Files:**
- Modify: `rust-server/src/main.rs` (message dispatch)
- Modify: `rust-server/src/game.rs` (handler implementation)

**Step 1: Add message dispatch**

In `rust-server/src/main.rs`, find the `ClientMessage` match (around line 4790). Add a new arm:

```rust
        ClientMessage::UseItemOn {
            slot_index,
            target_npc_id,
        } => {
            room.handle_use_item_on(player_id, slot_index, &target_npc_id)
                .await;
        }
```

**Step 2: Implement handler in game.rs**

In `rust-server/src/game.rs`, add the handler method. Place it near `handle_npc_interact` (around line 5008):

```rust
    pub async fn handle_use_item_on(
        &self,
        player_id: &str,
        slot_index: u8,
        target_npc_id: &str,
    ) {
        // 1. Get player position and inventory item
        let (player_x, player_y, item_id) = {
            let players = self.players.read().await;
            let Some(player) = players.get(player_id) else {
                return;
            };
            let item_id = player
                .inventory
                .slots
                .get(slot_index as usize)
                .and_then(|s| s.as_ref())
                .map(|s| s.item_id.clone());
            (player.x, player.y, item_id)
        };

        let Some(item_id) = item_id else {
            return; // Empty slot
        };

        // 2. Get NPC info (check instance first, then overworld)
        let instance_id = self
            .player_instances
            .read()
            .await
            .get(player_id)
            .cloned()
            .flatten();

        let npc_info = if let Some(ref _inst_id) = instance_id {
            if let Some(instance) = self.instance_manager.find_player_instance(player_id).await {
                let npcs = instance.npcs.read().await;
                npcs.get(target_npc_id).map(|npc| {
                    let dx = (npc.x - player_x) as f32;
                    let dy = (npc.y - player_y) as f32;
                    let distance = (dx * dx + dy * dy).sqrt();
                    (npc.prototype_id.clone(), npc.id.clone(), distance)
                })
            } else {
                None
            }
        } else {
            let npcs = self.npcs.read().await;
            npcs.get(target_npc_id).map(|npc| {
                let dx = (npc.x - player_x) as f32;
                let dy = (npc.y - player_y) as f32;
                let distance = (dx * dx + dy * dy).sqrt();
                (npc.prototype_id.clone(), npc.id.clone(), distance)
            })
        };

        let Some((entity_type, npc_runtime_id, distance)) = npc_info else {
            return; // NPC not found
        };

        // 3. Range check
        if distance > 2.5 {
            return;
        }

        // 4. Try quest Lua handlers first
        let handled = self
            .handle_use_item_on_quest(player_id, &item_id, &entity_type, &npc_runtime_id)
            .await;

        if handled {
            return;
        }

        // 5. TODO: TOML item_interactions fallback (future task)

        // 6. Nothing matched
        self.send_to_player(
            player_id,
            ServerMessage::ShowNotification {
                text: "Nothing interesting happens.".to_string(),
            },
        )
        .await;
    }
```

**Step 3: Verify server compiles**

The `handle_use_item_on_quest` method doesn't exist yet — stub it out as a method that returns `false` so the server compiles:

```rust
    async fn handle_use_item_on_quest(
        &self,
        _player_id: &str,
        _item_id: &str,
        _entity_type: &str,
        _npc_id: &str,
    ) -> bool {
        false // TODO: implement in Task 5
    }
```

Run: `cd rust-server && cargo check 2>&1 | tail -3`

**Step 4: Commit**

```bash
git add rust-server/src/main.rs rust-server/src/game.rs
git commit -m "feat: add server handler for UseItemOn with range check"
```

---

### Task 5: Server — Quest Lua on_use_item Callback

**Files:**
- Modify: `rust-server/src/quest/runner.rs` (new Lua callback runner + flag methods)
- Modify: `rust-server/src/game.rs` or `rust-server/src/game/quests.rs` (quest routing)

**Step 1: Add get_flag and set_flag to Lua context**

In `rust-server/src/quest/runner.rs`, in the `run_interact_sync` method (where `ctx_table` is built), add `get_flag` and `set_flag` methods after the existing methods. These read/write from the quest state flags:

```rust
        // Add get_flag method — read persistent quest flag
        let flags_clone = ctx.quest_state.flags.clone();
        let get_flag = lua.create_function(move |lua, (_this, flag_name): (Table, String)| {
            match flags_clone.get(&flag_name) {
                Some(value) => Ok(Value::String(lua.create_string(value)?)),
                None => Ok(Value::Nil),
            }
        })?;
        ctx_table.set("get_flag", get_flag)?;

        // Add set_flag method — write persistent quest flag
        let set_flag = lua.create_function(|_lua, (this, flag_name, flag_value): (Table, String, String)| {
            let result: Table = this.get("_result")?;
            let flags: Table = result
                .get::<Table>("_flags")
                .unwrap_or_else(|_| _lua.create_table().unwrap());
            flags.set(flag_name, flag_value)?;
            result.set("_flags", flags)?;
            Ok(())
        })?;
        ctx_table.set("set_flag", set_flag)?;
```

In the result extraction section (after extracting granted_items), add flag extraction:

```rust
        // Extract flags to persist
        if let Ok(flags) = result_table.get::<Table>("_flags") {
            for pair in flags.pairs::<String, String>() {
                if let Ok((key, value)) = pair {
                    result.flags_to_set.push((key, value));
                }
            }
        }
```

Add `flags_to_set: Vec<(String, String)>` to the `ScriptResult` struct and its `Default` impl.

**Step 2: Add run_on_use_item to QuestRunner**

In `rust-server/src/quest/runner.rs`, add a new method similar to `run_on_objective_progress`:

```rust
    /// Run the on_use_item handler for a quest
    /// Returns (handled: bool, notifications: Vec<String>, flags: Vec<(String, String)>, granted_items: Vec<(String, i32)>)
    pub async fn run_on_use_item(
        &self,
        player_id: &str,
        quest_id: &str,
        quest_state: &PlayerQuestState,
        item_id: &str,
        entity_type: &str,
        npc_id: &str,
    ) -> Result<Option<ScriptResult>, String> {
        self.load_quest_script(player_id, quest_id).await?;

        let states = self.player_states.read().await;
        let state = states
            .get(player_id)
            .ok_or_else(|| format!("No Lua state for player {}", player_id))?;

        if !state.has_function("on_use_item") {
            return Ok(None); // No handler = not handled
        }

        let ctx = QuestContext::new(
            player_id.to_string(),
            quest_id.to_string(),
            quest_state.clone(),
        );

        // Build context with full API (same as run_interact_sync but without dialogue step tracking)
        // For on_use_item, we use a simpler context since it's not a dialogue flow
        let lua = &state.lua;
        let ctx_table = lua.create_table().map_err(|e| format!("Lua error: {}", e))?;

        // Add quest state
        let quest_state_str = ctx.get_quest_state_string();
        ctx_table.set("_quest_state", quest_state_str).map_err(|e| format!("Lua error: {}", e))?;
        ctx_table.set("_quest_id", ctx.quest_id.clone()).map_err(|e| format!("Lua error: {}", e))?;

        // Add result accumulator
        let result_table = lua.create_table().map_err(|e| format!("Lua error: {}", e))?;
        result_table.set("notifications", lua.create_table().map_err(|e| format!("Lua error: {}", e))?).map_err(|e| format!("Lua error: {}", e))?;
        ctx_table.set("_result", result_table.clone()).map_err(|e| format!("Lua error: {}", e))?;

        // Add show_notification
        let show_notification = lua.create_function(|lua, (this, text): (Table, String)| {
            let result: Table = this.get("_result")?;
            let notifs: Table = result.get("notifications")?;
            let len = notifs.len()? + 1;
            notifs.set(len, text)?;
            Ok(())
        }).map_err(|e| format!("Lua error: {}", e))?;
        ctx_table.set("show_notification", show_notification).map_err(|e| format!("Lua error: {}", e))?;

        // Add get_objective_progress
        let objectives = ctx.quest_state.clone();
        let qid = ctx.quest_id.clone();
        let get_objective_progress = lua.create_function(move |lua, (_this, obj_id): (Table, String)| {
            let progress_table = lua.create_table()?;
            if let Some(quest_progress) = objectives.get_quest(&qid) {
                if let Some(obj) = quest_progress.objectives.get(&obj_id) {
                    progress_table.set("current", obj.current)?;
                    progress_table.set("target", obj.target)?;
                } else {
                    progress_table.set("current", 0)?;
                    progress_table.set("target", 0)?;
                }
            } else {
                progress_table.set("current", 0)?;
                progress_table.set("target", 0)?;
            }
            Ok(progress_table)
        }).map_err(|e| format!("Lua error: {}", e))?;
        ctx_table.set("get_objective_progress", get_objective_progress).map_err(|e| format!("Lua error: {}", e))?;

        // Add get_flag
        let flags_clone = ctx.quest_state.flags.clone();
        let get_flag = lua.create_function(move |lua, (_this, flag_name): (Table, String)| {
            match flags_clone.get(&flag_name) {
                Some(value) => Ok(Value::String(lua.create_string(value)?)),
                None => Ok(Value::Nil),
            }
        }).map_err(|e| format!("Lua error: {}", e))?;
        ctx_table.set("get_flag", get_flag).map_err(|e| format!("Lua error: {}", e))?;

        // Add set_flag
        let set_flag = lua.create_function(|_lua, (this, flag_name, flag_value): (Table, String, String)| {
            let result: Table = this.get("_result")?;
            let flags: Table = result
                .get::<Table>("_flags")
                .unwrap_or_else(|_| _lua.create_table().unwrap());
            flags.set(flag_name, flag_value)?;
            result.set("_flags", flags)?;
            Ok(())
        }).map_err(|e| format!("Lua error: {}", e))?;
        ctx_table.set("set_flag", set_flag).map_err(|e| format!("Lua error: {}", e))?;

        // Add give_item
        let give_item = lua.create_function(|lua, (this, item_id, count): (Table, String, Option<i32>)| {
            let result: Table = this.get("_result")?;
            let items: Table = result.get::<Table>("_granted_items").unwrap_or_else(|_| lua.create_table().unwrap());
            let len = items.len().unwrap_or(0);
            let entry = lua.create_table()?;
            entry.set("item_id", item_id)?;
            entry.set("count", count.unwrap_or(1))?;
            items.set(len + 1, entry)?;
            result.set("_granted_items", items)?;
            Ok(())
        }).map_err(|e| format!("Lua error: {}", e))?;
        ctx_table.set("give_item", give_item).map_err(|e| format!("Lua error: {}", e))?;

        // Call on_use_item(ctx, item_id, entity_type, npc_id)
        let on_use_item: Function = lua.globals().get("on_use_item").map_err(|e| format!("Lua error: {}", e))?;
        let lua_result = on_use_item.call::<Value>((ctx_table.clone(), item_id, entity_type, npc_id));

        match lua_result {
            Ok(value) => {
                // Check if handler returned true (handled)
                let handled = match value {
                    Value::Boolean(b) => b,
                    _ => true, // Default to handled if function exists and didn't error
                };

                if !handled {
                    return Ok(None);
                }

                // Extract results
                let mut script_result = ScriptResult::default();

                // Extract notifications
                if let Ok(notifs) = result_table.get::<Table>("notifications") {
                    for pair in notifs.pairs::<i32, String>() {
                        if let Ok((_, text)) = pair {
                            script_result.notifications.push(text);
                        }
                    }
                }

                // Extract flags
                if let Ok(flags) = result_table.get::<Table>("_flags") {
                    for pair in flags.pairs::<String, String>() {
                        if let Ok((key, value)) = pair {
                            script_result.flags_to_set.push((key, value));
                        }
                    }
                }

                // Extract granted items
                if let Ok(items) = result_table.get::<Table>("_granted_items") {
                    for pair in items.pairs::<i64, Table>() {
                        if let Ok((_, entry)) = pair {
                            if let (Ok(item_id), Ok(count)) = (entry.get::<String>("item_id"), entry.get::<i32>("count")) {
                                script_result.granted_items.push((item_id, count));
                            }
                        }
                    }
                }

                Ok(Some(script_result))
            }
            Err(e) => Err(format!("Script error in on_use_item: {}", e)),
        }
    }
```

**Step 3: Add flags_to_set to ScriptResult**

In `rust-server/src/quest/runner.rs`, find the `ScriptResult` struct (around line 72) and add:

```rust
    /// Flags to persist in quest state
    pub flags_to_set: Vec<(String, String)>,
```

And in the `Default` impl:

```rust
            flags_to_set: Vec::new(),
```

**Step 4: Implement handle_use_item_on_quest**

In `rust-server/src/game.rs` (or `rust-server/src/game/quests.rs`), replace the stub with the real implementation:

```rust
    async fn handle_use_item_on_quest(
        &self,
        player_id: &str,
        item_id: &str,
        entity_type: &str,
        npc_id: &str,
    ) -> bool {
        let mut quest_states = self.player_quest_states.write().await;
        let quest_state = quest_states
            .entry(player_id.to_string())
            .or_insert_with(PlayerQuestState::new);

        // Try each active quest's on_use_item handler
        let active_quest_ids: Vec<String> = quest_state
            .active_quests
            .keys()
            .cloned()
            .collect();

        for quest_id in active_quest_ids {
            match self
                .quest_runner
                .run_on_use_item(player_id, &quest_id, quest_state, item_id, entity_type, npc_id)
                .await
            {
                Ok(Some(script_result)) => {
                    // Persist flags
                    for (key, value) in &script_result.flags_to_set {
                        quest_state.set_flag(key, value);
                    }

                    // Send notifications
                    for notification in &script_result.notifications {
                        self.send_to_player(
                            player_id,
                            ServerMessage::ShowNotification {
                                text: notification.clone(),
                            },
                        )
                        .await;
                    }

                    // Grant items
                    self.grant_script_items(player_id, &script_result.granted_items, quest_state)
                        .await;

                    return true; // Handled
                }
                Ok(None) => continue, // Not handled by this quest
                Err(e) => {
                    tracing::error!("on_use_item error for quest {}: {}", quest_id, e);
                    continue;
                }
            }
        }

        false
    }
```

**Step 5: Verify server compiles**

Run: `cd rust-server && cargo check 2>&1 | tail -3`

**Step 6: Commit**

```bash
git add rust-server/src/quest/runner.rs rust-server/src/game.rs rust-server/src/game/quests.rs
git commit -m "feat: add on_use_item Lua callback with flag read/write support"
```

---

### Task 6: Candle Puzzle — Lua Implementation

**Files:**
- Modify: `rust-server/data/scripts/quests/ghastly_contraption/ghastly_contraption.lua`
- Modify: `rust-server/data/scripts/quests/ghastly_contraption/test_ghastly_contraption.lua`

**Step 1: Add on_use_item callback to the quest script**

Append to the end of `ghastly_contraption.lua` (before `on_objective_progress`):

```lua
-- ============================================================================
-- Use Item on Entity — Candle Puzzle
-- ============================================================================

-- Candle mapping: unique_id -> candle name and flame color
local CANDLE_INFO = {
    candle_1 = { name = "skull candle", flame = "an eerie green" },
    candle_2 = { name = "tall taper", flame = "a pale blue" },
    candle_3 = { name = "red candle", flame = "a warm orange" },
    candle_4 = { name = "small stubby candle", flame = "a sputtering yellow" },
}

-- Correct order
local CANDLE_ORDER = { "candle_1", "candle_2", "candle_3", "candle_4" }

function on_use_item(ctx, item_id, entity_type, npc_id)
    -- Only handle tinderbox on candles
    if item_id ~= "tinderbox" or entity_type ~= "haunted_candles" then
        return false
    end

    -- Check quest is active and we're at the candle puzzle step
    local gate = ctx:get_objective_progress("open_first_gate")
    if gate.current >= gate.target then
        ctx:show_notification("The candles are already lit. The gate is open.")
        return true
    end

    local tinderbox = ctx:get_objective_progress("find_tinderbox")
    if tinderbox.current < tinderbox.target then
        ctx:show_notification("You don't have a tinderbox.")
        return true
    end

    -- Get current lit candles from flag
    local lit_str = ctx:get_flag("candles_lit") or ""
    local lit = {}
    if lit_str ~= "" then
        for id in string.gmatch(lit_str, "([^,]+)") do
            table.insert(lit, id)
        end
    end

    -- Check if this candle is already lit
    for _, id in ipairs(lit) do
        if id == npc_id then
            local info = CANDLE_INFO[npc_id]
            if info then
                ctx:show_notification("The " .. info.name .. " is already lit.")
            end
            return true
        end
    end

    -- Check if this is the correct next candle
    local next_index = #lit + 1
    local expected = CANDLE_ORDER[next_index]

    if npc_id ~= expected then
        -- Wrong candle! Reset all
        ctx:set_flag("candles_lit", "")
        ctx:show_notification("A cold wind howls through the room. All the candles snuff out at once. Somewhere, a ghost laughs.")
        return true
    end

    -- Correct candle — light it
    table.insert(lit, npc_id)
    local new_lit_str = table.concat(lit, ",")
    ctx:set_flag("candles_lit", new_lit_str)

    local info = CANDLE_INFO[npc_id]
    if info then
        ctx:show_notification("The " .. info.name .. " flickers to life with " .. info.flame .. " flame.")
    end

    -- Check if all candles are lit
    if #lit == #CANDLE_ORDER then
        ctx:show_notification("All four candles burn in unison. The gate groans... and slowly creaks open!")
        ctx:set_flag("candles_lit", "") -- Clean up
    end

    return true
end
```

**Step 2: Remove the old dialogue-based candle puzzle**

The `show_candle_puzzle` and `show_candle_failure` functions are no longer needed. Remove them or comment them out. Update `route_in_progress` to remove the candle puzzle routing via dialogue — the puzzle now happens through `on_use_item` only.

Update the routing in `route_in_progress`:

```lua
    -- If gate not opened yet, player has tinderbox — hint about candles
    if gate.current < gate.target then
        if npc == "haunted_candles" then
            ctx:show_notification("Use the tinderbox on each candle to light them.")
        elseif npc == "prof_oddwick" then
            show_oddwick_hint_candles(ctx)
        end
        return
    end
```

**Step 3: Handle open_first_gate objective completion**

The `on_use_item` callback sets flags but doesn't complete the `open_first_gate` objective. The server needs to fire a `LocationReached` event or directly update the objective after the Lua callback returns.

Option: In the `handle_use_item_on_quest` method (server side), after the Lua handler returns, check if the `candles_lit` flag has all 4 candles and manually complete the `open_first_gate` objective.

Simpler option: Add objective completion to the Lua callback. We need a new Lua method `ctx:complete_objective(objective_id)`. Add this to the on_use_item context setup:

```rust
        // Add complete_objective method
        let complete_objective = lua.create_function(|_lua, (this, objective_id): (Table, String)| {
            let result: Table = this.get("_result")?;
            let completions: Table = result
                .get::<Table>("_completed_objectives")
                .unwrap_or_else(|_| _lua.create_table().unwrap());
            let len = completions.len().unwrap_or(0);
            completions.set(len + 1, objective_id)?;
            result.set("_completed_objectives", completions)?;
            Ok(())
        })?;
        ctx_table.set("complete_objective", complete_objective)?;
```

Then in the Lua script, when all 4 candles are lit:

```lua
    if #lit == #CANDLE_ORDER then
        ctx:show_notification("All four candles burn in unison. The gate groans... and slowly creaks open!")
        ctx:complete_objective("open_first_gate")
        ctx:set_flag("candles_lit", "")
    end
```

And in the server's `handle_use_item_on_quest`, extract and process completed objectives:

```rust
                    // Complete objectives
                    if let Some(completed_objectives) = &script_result.completed_objectives {
                        for objective_id in completed_objectives {
                            if let Some(progress) = quest_state.get_quest_mut(&quest_id) {
                                if progress.update_objective(objective_id, 1) {
                                    self.send_quest_progress_update(
                                        player_id, &quest_id, objective_id, 1, 1,
                                    ).await;
                                }
                            }
                        }
                    }
```

Add `completed_objectives: Option<Vec<String>>` to `ScriptResult`.

**Step 4: Update Lua tests**

Add new tests to `test_ghastly_contraption.lua` for the `on_use_item` callback:

```lua
-- Add ctx:set_flag and ctx:get_flag to mock
-- Add ctx:complete_objective to mock
-- Test: correct candle order lights all 4
-- Test: wrong candle resets
-- Test: already lit candle gives message
-- Test: non-tinderbox item returns false
-- Test: non-candle entity returns false
```

**Step 5: Verify and commit**

Run: `cd rust-server && cargo check 2>&1 | tail -3`
Run: `cd rust-server/data/scripts/quests/ghastly_contraption && lua test_ghastly_contraption.lua`

```bash
git add rust-server/src/quest/runner.rs rust-server/src/game.rs rust-server/src/game/quests.rs
git add rust-server/data/scripts/quests/ghastly_contraption/
git commit -m "feat: implement candle puzzle via use-item-on-entity"
```

---

### Task 7: TOML Item Interactions (Future Framework)

**Files:**
- Create: `rust-server/data/item_interactions.toml`
- Modify: `rust-server/src/game.rs` (fallback handler)

**Step 1: Create the TOML file with a placeholder**

Create `rust-server/data/item_interactions.toml`:

```toml
# Item-on-Entity Interactions
# These are checked when no quest Lua script handles the use-item action.
#
# Fields:
#   item = item ID from inventory
#   target_entity = entity prototype ID
#   message = text shown to player
#   consume_item = whether to remove the item (default: false)
#   result_item = item to give instead (optional)

# Example (uncomment when needed):
# [[interactions]]
# item = "bone"
# target_entity = "altar"
# message = "You bury the bones at the altar."
# consume_item = true
```

**Step 2: Commit**

```bash
git add rust-server/data/item_interactions.toml
git commit -m "feat: add item_interactions.toml framework for generic use-item actions"
```

Note: The actual TOML loading and matching can be implemented when the first non-quest interaction is needed. The framework is in place.

---

### Post-Implementation Notes

**Testing:** After all tasks, do a full in-game walkthrough:
1. Accept quest from Oddwick
2. Search bookshelf → get tinderbox
3. Select tinderbox in inventory (click to highlight)
4. Click candle_1 (skull) → lights green
5. Click candle_2 (tall) → lights blue
6. Click candle_3 (red) → lights orange
7. Click candle_4 (small) → lights yellow, gate opens
8. Try wrong order → all reset with ghost laugh
9. Continue quest (Barnaby, poltergeist, etc.)

**NPC sprite:** The `haunted_candles` entity uses sprite `candle` — make sure this sprite exists or swap to an available one.

**Walk-to-then-use:** Task 3 notes that pathfinding to entity before using item is not implemented. For now, player must be within 2.5 tiles. Add pathfind support later if needed.
