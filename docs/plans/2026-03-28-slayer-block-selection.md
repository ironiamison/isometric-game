# Slayer Block Monster Selection UI

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow players to select which monster to block when purchasing the "Block Monster" slayer reward.

**Architecture:** The server sends a list of all blockable monsters (deduplicated across all masters) with the panel open message. The client renders these as a selectable list in the Blocks tab. When the player selects a monster and clicks Buy on the block reward, the selected monster ID is sent as `target_monster_id`.

**Tech Stack:** Rust server (Axum), Rust client (Macroquad), MessagePack protocol

---

### Task 1: Server - Add `get_all_blockable_monsters()` to SlayerRegistry

**Files:**
- Modify: `rust-server/src/slayer/registry.rs:15-82`

**Step 1: Add the method**

Add after `get_slayer_requirement()` (line 85), before `assign_task()` (line 87):

```rust
/// Returns deduplicated (monster_id, display_name) pairs across all masters.
pub fn get_all_blockable_monsters(&self) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for master in self.masters.values() {
        for task in &master.tasks {
            if seen.insert(task.monster_id.clone()) {
                result.push((task.monster_id.clone(), task.display_name.clone()));
            }
        }
    }
    result.sort_by(|a, b| a.1.cmp(&b.1));
    result
}
```

**Step 2: Verify server compiles**

Run: `cd rust-server && cargo check 2>&1 | tail -5`
Expected: no new errors

**Step 3: Commit**

```
feat(slayer): add get_all_blockable_monsters to SlayerRegistry
```

---

### Task 2: Server - Add `blockable_monsters` to SlayerPanelOpen

**Files:**
- Modify: `rust-server/src/protocol.rs:1233-1242` (ServerMessage enum variant)
- Modify: `rust-server/src/protocol.rs:5714-5765` (MessagePack serialization)
- Modify: `rust-server/src/game/slayer.rs:136-157` (panel open handler)

**Step 1: Add field to ServerMessage::SlayerPanelOpen**

In `rust-server/src/protocol.rs`, add `blockable_monsters: Vec<(String, String)>` to the `SlayerPanelOpen` variant, after `unlocked_monsters`:

```rust
SlayerPanelOpen {
    master_id: String,
    master_name: String,
    current_task: Option<SlayerTaskData>,
    points: i32,
    tasks_completed: i32,
    rewards: Vec<SlayerRewardData>,
    blocked_monsters: Vec<String>,
    unlocked_monsters: Vec<String>,
    blockable_monsters: Vec<(String, String)>,
},
```

**Step 2: Serialize the new field**

In the MessagePack serialization block (`protocol.rs:5714-5765`), add the `blockable_monsters` destructure and serialization. Update the destructure at line 5714 to include `blockable_monsters`, then add before the final `Value::Map(map)`:

```rust
let blockable: Vec<Value> = blockable_monsters
    .iter()
    .map(|(id, name)| {
        Value::Map(vec![
            (Value::String("id".into()), Value::String(id.clone().into())),
            (Value::String("name".into()), Value::String(name.clone().into())),
        ])
    })
    .collect();
map.push((
    Value::String("blockable_monsters".into()),
    Value::Array(blockable),
));
```

**Step 3: Populate in panel open handler**

In `rust-server/src/game/slayer.rs`, in the `handle_slayer_open_panel` method (~line 144-157), add `blockable_monsters` to the `SlayerPanelOpen` construction:

```rust
ServerMessage::SlayerPanelOpen {
    master_id: master.id.clone(),
    master_name: master.display_name.clone(),
    current_task: slayer_task_data_from_state(&state),
    points: state.points,
    tasks_completed: state.tasks_completed,
    rewards,
    blocked_monsters: state.blocked_monsters.clone(),
    unlocked_monsters: state.unlocked_monsters.clone(),
    blockable_monsters: self.slayer_registry.get_all_blockable_monsters(),
},
```

**Step 4: Verify server compiles**

Run: `cd rust-server && cargo check 2>&1 | tail -5`
Expected: no new errors

**Step 5: Commit**

```
feat(slayer): send blockable_monsters list in SlayerPanelOpen
```

---

### Task 3: Client - Add state fields and parse blockable_monsters

**Files:**
- Modify: `client/src/game/state.rs:1620-1621` (add fields)
- Modify: `client/src/game/state.rs:1858-1861` (add defaults)
- Modify: `client/src/network/message_handler.rs:4070-4087` (parse new field)

**Step 1: Add state fields**

In `client/src/game/state.rs`, after `slayer_unlocked_monsters` (line 1621), add:

```rust
pub slayer_blockable_monsters: Vec<(String, String)>,
pub slayer_selected_block_monster: Option<usize>,
```

**Step 2: Add defaults**

In the `Default`-like init block (~line 1858-1861), after `slayer_unlocked_monsters: Vec::new(),` add:

```rust
slayer_blockable_monsters: Vec::new(),
slayer_selected_block_monster: None,
```

**Step 3: Parse blockable_monsters in message handler**

In `client/src/network/message_handler.rs`, in the `"slayerPanelOpen"` handler (~line 4083), after the `slayer_unlocked_monsters` line and before `slayer_panel_open = true`, add:

```rust
state.ui_state.slayer_blockable_monsters = extract_blockable_monsters(value, "blockable_monsters");
state.ui_state.slayer_selected_block_monster = None;
```

Add the extraction function near the other slayer extraction functions (~line 114):

```rust
fn extract_blockable_monsters(
    value: &rmpv::Value,
    key: &str,
) -> Vec<(String, String)> {
    let mut result = Vec::new();
    if let Some(arr) = extract_map_field(value, key) {
        if let rmpv::Value::Array(ref items) = *arr {
            for item in items {
                let id = extract_string(item, "id").unwrap_or_default();
                let name = extract_string(item, "name").unwrap_or_default();
                if !id.is_empty() {
                    result.push((id, name));
                }
            }
        }
    }
    result
}
```

**Step 4: Verify client compiles**

Run: `cd client && cargo check 2>&1 | tail -5`
Expected: no new errors

**Step 5: Commit**

```
feat(slayer): parse blockable_monsters on client
```

---

### Task 4: Client - Add UI element and render monster selection in Blocks tab

**Files:**
- Modify: `client/src/ui/layout.rs:256` (add new UiElementId variant)
- Modify: `client/src/render/ui/slayer_panel.rs:872-996` (render monster selection list)
- Modify: `client/src/input/handler.rs:2810-2818` (wire selected monster into buy command)
- Modify: `client/src/input/handler.rs:2819-2828` (add handler for monster selection click)

**Step 1: Add UiElementId variant**

In `client/src/ui/layout.rs`, after `SlayerRemoveBlock(usize)` (line 256), add:

```rust
SlayerBlockMonsterSelect(usize),
```

**Step 2: Render selectable monster list in Blocks tab**

In `client/src/render/ui/slayer_panel.rs`, replace the entire block at lines 872-996 (the `if active_tab == 3 {` section) with the new implementation. The new version shows:

1. A "Select monster to block:" header
2. A list of blockable monsters (excluding already-blocked ones) as selectable rows — clicking one highlights it
3. The existing "Currently Blocked:" section with Remove buttons

Replace lines 872-996:

```rust
            // For Blocks tab, show selectable monster list + currently blocked monsters
            if active_tab == 3 {
                // Separator if there are rewards above
                if !filtered_rewards.is_empty() {
                    let sep_y =
                        content_y + 4.0 * s + row_idx as f32 * (row_h + row_sp) - scroll_offset;
                    if sep_y >= content_y && sep_y <= content_y + content_h {
                        draw_line(
                            content_x + 10.0 * s,
                            sep_y + 4.0 * s,
                            content_x + content_w - 10.0 * s,
                            sep_y + 4.0 * s,
                            1.0,
                            HEADER_BORDER,
                        );
                    }
                    row_idx += 1;
                }

                // Filter out already-blocked monsters from the selectable list
                let available: Vec<(usize, &(String, String))> = state
                    .ui_state
                    .slayer_blockable_monsters
                    .iter()
                    .enumerate()
                    .filter(|(_, (id, _))| !state.ui_state.slayer_blocked_monsters.contains(id))
                    .collect();

                if !available.is_empty() {
                    // "Select monster to block:" header
                    let select_header_y =
                        content_y + 4.0 * s + row_idx as f32 * (row_h + row_sp) - scroll_offset;
                    if select_header_y >= content_y && select_header_y <= content_y + content_h {
                        self.draw_text_sharp(
                            "Select monster to block:",
                            content_x + 10.0 * s,
                            select_header_y + 16.0 * s,
                            16.0,
                            TEXT_TITLE,
                        );
                    }
                    row_idx += 1;

                    // Render selectable monster rows
                    for (orig_idx, (_monster_id, monster_name)) in &available {
                        let item_y = content_y + 4.0 * s
                            + row_idx as f32 * (row_h + row_sp)
                            - scroll_offset;

                        if item_y + row_h >= content_y && item_y <= content_y + content_h {
                            let is_selected =
                                state.ui_state.slayer_selected_block_monster == Some(*orig_idx);

                            // Row background - highlight if selected
                            let row_bg = if is_selected {
                                Color::new(0.15, 0.18, 0.12, 0.9)
                            } else if row_idx % 2 == 0 {
                                Color::new(0.08, 0.08, 0.10, 0.6)
                            } else {
                                Color::new(0.06, 0.06, 0.08, 0.6)
                            };
                            draw_rectangle(
                                content_x + 2.0,
                                item_y,
                                content_w - 4.0,
                                row_h,
                                row_bg,
                            );

                            // Selection border if selected
                            if is_selected {
                                draw_rectangle_lines(
                                    content_x + 2.0,
                                    item_y,
                                    content_w - 4.0,
                                    row_h,
                                    1.0,
                                    FRAME_ACCENT,
                                );
                            }

                            // Monster name
                            let name_color = if is_selected { TEXT_GOLD } else { TEXT_NORMAL };
                            self.draw_text_sharp(
                                monster_name,
                                content_x + 10.0 * s,
                                item_y + row_h * 0.55,
                                16.0,
                                name_color,
                            );

                            // Make the whole row clickable
                            let row_bounds = Rect::new(
                                content_x + 2.0,
                                item_y,
                                content_w - 4.0,
                                row_h,
                            );
                            layout.add(
                                UiElementId::SlayerBlockMonsterSelect(*orig_idx),
                                row_bounds,
                            );
                        }

                        row_idx += 1;
                    }

                    // Add a gap before blocked section
                    row_idx += 1;
                }

                // Blocked monsters header
                let blocked_header_y =
                    content_y + 4.0 * s + row_idx as f32 * (row_h + row_sp) - scroll_offset;
                if blocked_header_y >= content_y && blocked_header_y <= content_y + content_h {
                    let blocked_label = "Currently Blocked:";
                    self.draw_text_sharp(
                        blocked_label,
                        content_x + 10.0 * s,
                        blocked_header_y + 16.0 * s,
                        16.0,
                        TEXT_TITLE,
                    );
                }
                row_idx += 1;

                if state.ui_state.slayer_blocked_monsters.is_empty() {
                    let empty_y =
                        content_y + 4.0 * s + row_idx as f32 * (row_h + row_sp) - scroll_offset;
                    if empty_y >= content_y && empty_y <= content_y + content_h {
                        self.draw_text_sharp(
                            "No blocked monsters",
                            content_x + 10.0 * s,
                            empty_y + 16.0 * s,
                            16.0,
                            TEXT_DIM,
                        );
                    }
                } else {
                    for (i, monster_name) in
                        state.ui_state.slayer_blocked_monsters.iter().enumerate()
                    {
                        let item_y =
                            content_y + 4.0 * s + row_idx as f32 * (row_h + row_sp) - scroll_offset;

                        if item_y + row_h >= content_y && item_y <= content_y + content_h {
                            // Row background
                            let row_bg = if row_idx % 2 == 0 {
                                Color::new(0.08, 0.08, 0.10, 0.6)
                            } else {
                                Color::new(0.06, 0.06, 0.08, 0.6)
                            };
                            draw_rectangle(content_x + 2.0, item_y, content_w - 4.0, row_h, row_bg);

                            // Monster name
                            self.draw_text_sharp(
                                monster_name,
                                content_x + 10.0 * s,
                                item_y + row_h * 0.55,
                                16.0,
                                TEXT_NORMAL,
                            );

                            // Remove button (reddish)
                            let remove_w = 70.0 * s;
                            let remove_h = 24.0 * s;
                            let remove_x = content_x + content_w - remove_w - 8.0 * s;
                            let remove_y = item_y + (row_h - remove_h) / 2.0;
                            let remove_bounds = Rect::new(remove_x, remove_y, remove_w, remove_h);
                            layout.add(UiElementId::SlayerRemoveBlock(i), remove_bounds);

                            let is_remove_hovered = matches!(
                                hovered,
                                Some(UiElementId::SlayerRemoveBlock(idx)) if *idx == i
                            );

                            let (remove_bg, remove_border) = if is_remove_hovered {
                                (
                                    Color::new(0.5, 0.15, 0.15, 1.0),
                                    Color::new(0.7, 0.25, 0.25, 1.0),
                                )
                            } else {
                                (
                                    Color::new(0.35, 0.1, 0.1, 1.0),
                                    Color::new(0.5, 0.18, 0.18, 1.0),
                                )
                            };

                            draw_rectangle(remove_x, remove_y, remove_w, remove_h, remove_border);
                            draw_rectangle(
                                remove_x + 1.0,
                                remove_y + 1.0,
                                remove_w - 2.0,
                                remove_h - 2.0,
                                remove_bg,
                            );

                            let remove_text = "Remove";
                            let remove_text_color = if is_remove_hovered {
                                WHITE
                            } else {
                                TEXT_NORMAL
                            };
                            let remove_dims = self.measure_text_sharp(remove_text, 16.0);
                            self.draw_text_sharp(
                                remove_text,
                                remove_x + (remove_w - remove_dims.width) / 2.0,
                                remove_y + remove_h * 0.71,
                                16.0,
                                remove_text_color,
                            );
                        }

                        row_idx += 1;
                    }
                }
            }
```

**Step 3: Handle monster selection click and wire into buy command**

In `client/src/input/handler.rs`, add a handler for `SlayerBlockMonsterSelect` clicks. Add after the `SlayerRemoveBlock` handler (~line 2828):

```rust
UiElementId::SlayerBlockMonsterSelect(idx) => {
    state.ui_state.slayer_selected_block_monster = Some(*idx);
}
```

Then modify the `SlayerBuyReward` handler (lines 2810-2818) to use the selected monster for block purchases:

```rust
UiElementId::SlayerBuyReward(idx) => {
    if let Some(reward) = state.ui_state.slayer_rewards.get(*idx) {
        if state.ui_state.slayer_points >= reward.cost {
            let target = if reward.category == "block" {
                state
                    .ui_state
                    .slayer_selected_block_monster
                    .and_then(|i| {
                        state
                            .ui_state
                            .slayer_blockable_monsters
                            .get(i)
                            .map(|(id, _)| id.clone())
                    })
            } else {
                reward.target_id.clone()
            };
            commands.push(InputCommand::SlayerBuyReward {
                reward_id: reward.id.clone(),
                target_monster_id: target,
            });
        }
    }
}
```

**Step 4: Clear selection after successful block purchase**

In `client/src/network/message_handler.rs`, in the `"slayerResult"` handler (~line 4118-4133), after updating points, add logic to refresh the blocked list on successful block:

```rust
let success = extract_bool(value, "success").unwrap_or(false);
let action = extract_string(value, "action");
// ... existing code ...
// Clear block selection on successful block purchase
if success && action.as_deref() == Some("buy_reward") {
    state.ui_state.slayer_selected_block_monster = None;
}
// Update blocked list from slayerResult if present
if let Some(blocked) = extract_map_field(value, "blocked_monsters") {
    if let rmpv::Value::Array(_) = *blocked {
        state.ui_state.slayer_blocked_monsters =
            extract_string_array(value, "blocked_monsters");
    }
}
```

**Step 5: Server - include updated blocked_monsters in block purchase result**

In `rust-server/src/game/slayer.rs`, update the success response in `handle_slayer_buy_reward` (~line 499-509) to include the updated blocked list. We need to add `blocked_monsters` to the `SlayerResult` message.

Actually, the simpler approach: after a successful block purchase, the server should send a `SlayerStateSync` which already includes `blocked_monsters`. Check if this already happens — if not, add it.

Looking at the code, `handle_slayer_buy_reward` only sends a `SlayerResult`. The client needs the updated `blocked_monsters` list. The cleanest fix: send a `SlayerStateSync` after successful block/unblock purchases.

In `rust-server/src/game/slayer.rs`, after the success `SlayerResult` send in `handle_slayer_buy_reward` (~line 509), if the plan was `BlockMonster`, also send a state sync:

```rust
// After the slayer_result send, check if we need to sync block list
if matches!(plan, RewardPurchasePlan::BlockMonster(_)) {
    self.send_to_player(
        player_id,
        ServerMessage::SlayerStateSync {
            current_task: slayer_task_data_from_state(&state),
            points: state.points,
            tasks_completed: state.tasks_completed,
            blocked_monsters: state.blocked_monsters.clone(),
            unlocked_monsters: state.unlocked_monsters.clone(),
        },
    )
    .await;
}
```

Check `SlayerStateSync` variant exists and has these fields — we already saw it parsed on the client at line 4135.

Similarly, in `handle_slayer_remove_block` (~line 523-534), after the success result, add a state sync:

```rust
self.send_to_player(
    player_id,
    ServerMessage::SlayerStateSync {
        current_task: slayer_task_data_from_state(&state),
        points: state.points,
        tasks_completed: state.tasks_completed,
        blocked_monsters: state.blocked_monsters.clone(),
        unlocked_monsters: state.unlocked_monsters.clone(),
    },
)
.await;
```

**Step 6: Verify both compile**

Run: `cd rust-server && cargo check 2>&1 | tail -5`
Run: `cd client && cargo check 2>&1 | tail -5`
Expected: no new errors

**Step 7: Commit**

```
feat(slayer): add monster selection UI for block purchases
```

---

### Task 5: Update empty state for Blocks tab

**Files:**
- Modify: `client/src/render/ui/slayer_panel.rs:767`

**Step 1: Fix the empty-state guard**

The current guard `active_tab != 2` means the "No rewards available" text is suppressed for Equipment tab. It should also be suppressed for the Blocks tab (index 3) since Blocks tab has its own content. Change line 767:

```rust
if filtered_rewards.is_empty() && active_tab != 2 && active_tab != 3 {
```

**Step 2: Verify client compiles**

Run: `cd client && cargo check 2>&1 | tail -5`
Expected: no new errors

**Step 3: Commit**

```
fix(slayer): suppress empty-state text on Blocks tab
```
