# Bank Reorganization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add drag-and-drop slot rearrangement within the bank grid and an auto-sort button, all server-authoritative.

**Architecture:** Two new client→server messages (`BankSwapSlots`, `BankSort`) handled by the server with existing `BankUpdate` responses. Client tracks drag state with a dead zone to distinguish clicks from drags. Sort uses existing `ItemCategory` enum.

**Tech Stack:** Rust server (Axum/Tokio), Rust client (Macroquad), MessagePack protocol

**Design doc:** `docs/plans/2026-02-23-bank-reorganization-design.md`

---

## Task 1: Add `BankSwapSlots` and `BankSort` to Server Protocol

**Files:**
- Modify: `rust-server/src/protocol.rs:213-232` (ClientMessage enum — bank section)
- Modify: `rust-server/src/protocol.rs:5233-5251` (decode_client_message — bank section)

**Step 1: Add enum variants to ClientMessage**

After line 232 (`BankDepositAll,`), add:

```rust
    /// Swap (or merge) two bank slots
    #[serde(rename = "bankSwapSlots")]
    BankSwapSlots { slot_a: u32, slot_b: u32 },

    /// Auto-sort entire bank by category then alphabetically
    #[serde(rename = "bankSort")]
    BankSort,
```

**Step 2: Add decode cases for the new messages**

After line 5251 (`"bankDepositAll" => Ok(ClientMessage::BankDepositAll),`), add:

```rust
        "bankSwapSlots" => {
            let slot_a = extract_u32(msg_data, "slot_a").unwrap_or(0);
            let slot_b = extract_u32(msg_data, "slot_b").unwrap_or(0);
            Ok(ClientMessage::BankSwapSlots { slot_a, slot_b })
        }
        "bankSort" => Ok(ClientMessage::BankSort),
```

**Step 3: Build and verify it compiles**

Run: `cd rust-server && cargo build 2>&1 | head -30`
Expected: Compiles (warnings about unused variants are fine — handlers come next)

**Step 4: Commit**

```bash
git add rust-server/src/protocol.rs
git commit -m "add BankSwapSlots and BankSort protocol messages"
```

---

## Task 2: Add `handle_bank_swap_slots` Server Handler

**Files:**
- Modify: `rust-server/src/game.rs` (after `handle_bank_deposit_all` ~line 7588)
- Modify: `rust-server/src/main.rs:3571-3573` (dispatch section)

**Step 1: Add the swap handler to GameRoom**

Add after the `handle_bank_deposit_all` function (after line ~7593):

```rust
    /// Swap two bank slots. If both contain the same item, merge stacks (up to 99).
    pub async fn handle_bank_swap_slots(&self, player_id: &str, slot_a: u32, slot_b: u32) {
        if slot_a == slot_b {
            return;
        }

        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        let len = player.bank.slots.len();
        let a = slot_a as usize;
        let b = slot_b as usize;
        if a >= len || b >= len {
            return;
        }

        // Check if both slots have the same item (merge case)
        let should_merge = match (&player.bank.slots[a], &player.bank.slots[b]) {
            (Some(sa), Some(sb)) => sa.item_id == sb.item_id,
            _ => false,
        };

        if should_merge {
            // Merge: move quantity from slot_a into slot_b, up to BANK_MAX_STACK
            let src_qty = player.bank.slots[a].as_ref().unwrap().quantity;
            let dst_qty = player.bank.slots[b].as_ref().unwrap().quantity;
            let can_add = item::BANK_MAX_STACK - dst_qty;
            let transfer = src_qty.min(can_add);

            player.bank.slots[b].as_mut().unwrap().quantity += transfer;
            let remaining = src_qty - transfer;
            if remaining <= 0 {
                player.bank.slots[a] = None;
            } else {
                player.bank.slots[a].as_mut().unwrap().quantity = remaining;
            }
        } else {
            // Swap
            player.bank.slots.swap(a, b);
        }

        let bank_msg = ServerMessage::BankUpdate {
            slots: player.bank.to_update(),
            gold: player.bank.gold,
        };
        drop(players);
        self.send_to_player(player_id, bank_msg).await;
    }
```

**Step 2: Add dispatch in main.rs**

After the `BankDepositAll` dispatch block (~line 3573), add:

```rust
        ClientMessage::BankSwapSlots { slot_a, slot_b } => {
            room.handle_bank_swap_slots(player_id, slot_a, slot_b).await;
        }
```

**Step 3: Build and verify**

Run: `cd rust-server && cargo build 2>&1 | head -30`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add rust-server/src/game.rs rust-server/src/main.rs
git commit -m "add handle_bank_swap_slots server handler"
```

---

## Task 3: Add `handle_bank_sort` Server Handler

**Files:**
- Modify: `rust-server/src/game.rs` (after the swap handler from Task 2)
- Modify: `rust-server/src/main.rs` (dispatch section, after swap dispatch)

**Step 1: Add sort_priority method to ItemCategory**

In `rust-server/src/data/item_def.rs`, add after the `Default` impl (~line 20):

```rust
impl ItemCategory {
    /// Priority for bank sorting: lower = sorted first
    pub fn sort_priority(self) -> u8 {
        match self {
            ItemCategory::Equipment => 0,
            ItemCategory::Consumable => 1,
            ItemCategory::Material => 2,
            ItemCategory::Quest => 3,
        }
    }
}
```

**Step 2: Add the sort handler to GameRoom**

Add after `handle_bank_swap_slots`:

```rust
    /// Sort bank by item category then alphabetically by display name.
    pub async fn handle_bank_sort(&self, player_id: &str) {
        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        // Collect non-empty slots
        let mut items: Vec<item::InventorySlot> = player
            .bank
            .slots
            .iter()
            .filter_map(|s| s.clone())
            .collect();

        // Sort by (category_priority, display_name)
        let registry = &self.item_registry;
        items.sort_by(|a, b| {
            let def_a = registry.get(&a.item_id);
            let def_b = registry.get(&b.item_id);
            let cat_a = def_a.map(|d| d.category.sort_priority()).unwrap_or(255);
            let cat_b = def_b.map(|d| d.category.sort_priority()).unwrap_or(255);
            let name_a = def_a.map(|d| d.display_name.as_str()).unwrap_or(&a.item_id);
            let name_b = def_b.map(|d| d.display_name.as_str()).unwrap_or(&b.item_id);
            cat_a.cmp(&cat_b).then_with(|| name_a.cmp(name_b))
        });

        // Rebuild slots: items packed to front, None for the rest
        let total = player.bank.slots.len();
        player.bank.slots = items.into_iter().map(Some).collect();
        player.bank.slots.resize(total, None);

        let bank_msg = ServerMessage::BankUpdate {
            slots: player.bank.to_update(),
            gold: player.bank.gold,
        };
        drop(players);
        self.send_to_player(player_id, bank_msg).await;
    }
```

**Step 3: Add dispatch in main.rs**

After the `BankSwapSlots` dispatch, add:

```rust
        ClientMessage::BankSort => {
            room.handle_bank_sort(player_id).await;
        }
```

**Step 4: Build and verify**

Run: `cd rust-server && cargo build 2>&1 | head -30`
Expected: Compiles successfully

**Step 5: Commit**

```bash
git add rust-server/src/data/item_def.rs rust-server/src/game.rs rust-server/src/main.rs
git commit -m "add handle_bank_sort server handler with category+alpha ordering"
```

---

## Task 4: Add Client Protocol Messages for `BankSwapSlots` and `BankSort`

**Files:**
- Modify: `client/src/network/messages.rs:105-118` (ClientMessage enum — bank section)
- Modify: `client/src/input/handler.rs:518-533` (InputCommand enum — bank section)
- Modify: `client/src/app.rs:457-472` (InputCommand → ClientMessage mapping)

**Step 1: Add ClientMessage variants**

In `client/src/network/messages.rs`, after `BankDepositAll` (~line 118), add:

```rust
    #[serde(rename = "bankSwapSlots")]
    BankSwapSlots { slot_a: u32, slot_b: u32 },

    #[serde(rename = "bankSort")]
    BankSort,
```

**Step 2: Add encoding in the encode match arm**

Find where `BankDepositAll` is encoded (~line 410) and add after it:

```rust
            ClientMessage::BankSwapSlots { slot_a, slot_b } => {
                data.insert("slot_a".into(), Value::Integer((*slot_a as i64).into()));
                data.insert("slot_b".into(), Value::Integer((*slot_b as i64).into()));
                "bankSwapSlots"
            }
            ClientMessage::BankSort => "bankSort",
```

**Step 3: Add InputCommand variants**

In `client/src/input/handler.rs`, after `BankDepositAll,` (~line 533), add:

```rust
    BankSwapSlots {
        slot_a: u32,
        slot_b: u32,
    },
    BankSort,
```

**Step 4: Add InputCommand → ClientMessage mapping**

In `client/src/app.rs`, after the `BankDepositAll` mapping (~line 472), add:

```rust
            InputCommand::BankSwapSlots { slot_a, slot_b } => ClientMessage::BankSwapSlots {
                slot_a: *slot_a,
                slot_b: *slot_b,
            },
            InputCommand::BankSort => ClientMessage::BankSort,
```

**Step 5: Build and verify**

Run: `cd client && cargo build 2>&1 | head -30`
Expected: Compiles (warnings about unused variants are fine)

**Step 6: Commit**

```bash
git add client/src/network/messages.rs client/src/input/handler.rs client/src/app.rs
git commit -m "add BankSwapSlots and BankSort client protocol messages"
```

---

## Task 5: Add Bank Drag State and Sort Button to Client UI State

**Files:**
- Modify: `client/src/game/state.rs:1215-1222` (bank state fields)
- Modify: `client/src/ui/layout.rs:143-154` (UiElementId enum)

**Step 1: Add BankDrag struct and state fields**

In `client/src/game/state.rs`, near the `BankQuantityDialog` struct (~line 1025), add:

```rust
/// Tracks an active drag operation within the bank grid
pub struct BankDrag {
    pub from_slot: usize,
    pub mouse_start_x: f32,
    pub mouse_start_y: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub active: bool,  // false = pending (haven't exceeded dead zone), true = actively dragging
}
```

Add to the bank fields section of `UiState` (after `bank_inv_scroll_drag`):

```rust
    pub bank_drag: Option<BankDrag>,
```

Initialize it in the `UiState` constructor/default (near ~line 1407):

```rust
    bank_drag: None,
```

**Step 2: Add UiElementId variants**

In `client/src/ui/layout.rs`, after `BankDepositAllButton` (~line 154), add:

```rust
    BankSortButton,
```

**Step 3: Build and verify**

Run: `cd client && cargo build 2>&1 | head -30`
Expected: Compiles

**Step 4: Commit**

```bash
git add client/src/game/state.rs client/src/ui/layout.rs
git commit -m "add BankDrag state and BankSortButton UI element"
```

---

## Task 6: Render Sort Button in Bank Header

**Files:**
- Modify: `client/src/render/ui/bank.rs:114-122` (header section, near close button)

**Step 1: Add sort button rendering**

The close button is at the far right of the header. Insert the sort button just to its left. Before the close button code (~line 114), add:

```rust
        // Sort button (between help and close)
        let sort_size = 20.0 * s;
        let sort_x = header_x + header_w - close_size - sort_size - 14.0 * s; // left of close button
        let sort_y = header_y + (header_h - sort_size) / 2.0;
        let sort_rect = Rect::new(sort_x, sort_y, sort_size, sort_size);
        layout.add(UiElementId::BankSortButton, sort_rect);
        let sort_hovered = matches!(hovered, Some(UiElementId::BankSortButton));
        let sort_bg = if sort_hovered {
            Color::new(0.25, 0.22, 0.30, 1.0)
        } else {
            Color::new(0.15, 0.13, 0.18, 1.0)
        };
        let sort_border = if sort_hovered {
            Color::new(0.6, 0.55, 0.7, 1.0)
        } else {
            Color::new(0.35, 0.32, 0.40, 1.0)
        };
        draw_rectangle(sort_x, sort_y, sort_size, sort_size, sort_border);
        draw_rectangle(
            sort_x + 1.0,
            sort_y + 1.0,
            sort_size - 2.0,
            sort_size - 2.0,
            sort_bg,
        );
        // Draw a simple sort icon: downward arrow with lines (AZ↓ style)
        let sort_text_color = if sort_hovered {
            TEXT_GOLD
        } else {
            Color::new(0.7, 0.65, 0.5, 1.0)
        };
        // Use a simple "S" or arrow character for sort
        let s_dims = self.measure_text_sharp("S", 14.0);
        self.draw_text_sharp(
            "S",
            sort_x + (sort_size - s_dims.width) / 2.0,
            sort_y + sort_size * 0.71,
            14.0,
            sort_text_color,
        );
```

Note: The sort button uses "S" as a simple icon. This follows the same visual pattern as the "?" help button and "X" close button.

**Step 2: Add tooltip for sort button**

Check if there's a tooltip system for bank buttons. If hovering displays a tooltip, add "Sort bank" text for `BankSortButton`. If not, skip this — the "S" icon is self-explanatory given the context.

**Step 3: Build and verify**

Run: `cd client && cargo build 2>&1 | head -30`
Expected: Compiles

**Step 4: Commit**

```bash
git add client/src/render/ui/bank.rs
git commit -m "render sort button in bank header"
```

---

## Task 7: Handle Sort Button Click

**Files:**
- Modify: `client/src/input/handler.rs:3527-3670` (bank click handling section)

**Step 1: Add sort button click handler**

In the bank click handling section, near the other button handlers (after `BankCloseButton` ~line 3540), add a case for the sort button:

```rust
                    UiElementId::BankSortButton => {
                        commands.push(InputCommand::BankSort);
                        state.pending_sfx.push("enter".to_string());
                        return commands;
                    }
```

**Step 2: Build and verify**

Run: `cd client && cargo build 2>&1 | head -30`
Expected: Compiles

**Step 3: Commit**

```bash
git add client/src/input/handler.rs
git commit -m "handle sort button click to send BankSort command"
```

---

## Task 8: Implement Bank Drag — Mouse Down (Pending Drag)

**Files:**
- Modify: `client/src/input/handler.rs:3577-3611` (BankSlot click handling)

**Step 1: Capture mouse-down for potential drag**

The current `BankSlot(idx)` handling immediately processes clicks. We need to split this into:
- On mouse **press** on a bank slot with an item: record a pending drag (but don't start dragging yet)
- On mouse **release** without exceeding dead zone: process the original click (withdraw)
- On mouse **move** exceeding dead zone: start active drag

Modify the `BankSlot(idx)` handling. The key insight: we need to detect `is_mouse_button_pressed` (just pressed this frame) vs `is_mouse_button_released` (just released). Currently the code triggers on click (which is typically press or release depending on the framework).

Check how clicks are detected in the bank handler code. The existing pattern uses `is_mouse_button_pressed(MouseButton::Left)` for click detection. We need to:

1. On mouse **down** on a BankSlot: if no drag active, record pending drag state
2. On mouse **up**: if pending drag (never exceeded dead zone), process as normal click
3. Between frames: if pending drag + mouse moved > 4px, promote to active drag

The cleanest approach: intercept at the **start** of bank input handling (before click processing), check for drag state updates every frame.

Add a drag update section at the top of the bank input handler (before click checks, around line ~3443):

```rust
        // ===== Bank Drag State Machine =====
        let (mx, my) = mouse_position();

        // If we have an active drag and mouse is released, complete the drag
        if let Some(ref drag) = state.ui_state.bank_drag {
            if drag.active && is_mouse_button_released(MouseButton::Left) {
                let from = drag.from_slot;
                // Check what slot the mouse is over
                if let Some(UiElementId::BankSlot(to_slot)) = hovered {
                    if from != *to_slot {
                        commands.push(InputCommand::BankSwapSlots {
                            slot_a: from as u32,
                            slot_b: *to_slot as u32,
                        });
                        state.pending_sfx.push("enter".to_string());
                    }
                }
                state.ui_state.bank_drag = None;
                return commands;
            }

            // If right-click or escape during drag, cancel
            if is_mouse_button_pressed(MouseButton::Right) || is_key_pressed(KeyCode::Escape) {
                state.ui_state.bank_drag = None;
                return commands;
            }

            // If pending drag, check dead zone
            if !drag.active {
                let dx = mx - drag.mouse_start_x;
                let dy = my - drag.mouse_start_y;
                if dx * dx + dy * dy > 16.0 {
                    // 4px dead zone (4^2 = 16)
                    // Promote to active drag
                    if let Some(ref mut d) = state.ui_state.bank_drag {
                        d.active = true;
                    }
                    return commands; // consume this frame
                }

                // If mouse released while still pending, treat as normal click
                if is_mouse_button_released(MouseButton::Left) {
                    let from = drag.from_slot;
                    state.ui_state.bank_drag = None;
                    // Fall through to normal click handling below
                    // Process withdraw for slot `from`
                    if let Some(Some((ref item_id, qty))) =
                        state.ui_state.bank_slots.get(from)
                    {
                        let item_id = item_id.clone();
                        let qty = *qty;
                        let shift_held = is_key_down(KeyCode::LeftShift)
                            || is_key_down(KeyCode::RightShift);
                        let ctrl_held = is_key_down(KeyCode::LeftControl)
                            || is_key_down(KeyCode::RightControl);
                        if shift_held {
                            commands.push(InputCommand::BankWithdraw {
                                item_id,
                                quantity: qty,
                            });
                        } else if ctrl_held && qty > 1 {
                            state.ui_state.bank_quantity_dialog =
                                Some(BankQuantityDialog {
                                    input: String::new(),
                                    cursor: 0,
                                    action: BankQuantityAction::WithdrawItem,
                                    item_id: Some(item_id),
                                    max_quantity: qty,
                                });
                        } else {
                            commands.push(InputCommand::BankWithdraw {
                                item_id,
                                quantity: 1,
                            });
                        }
                        state.pending_sfx.push("enter".to_string());
                    }
                    return commands;
                }
            }

            // While actively dragging, consume all input (don't process clicks)
            if drag.active {
                return commands;
            }
        }

        // Initiate pending drag on mouse press over a bank slot with an item
        if state.ui_state.bank_drag.is_none()
            && is_mouse_button_pressed(MouseButton::Left)
        {
            if let Some(UiElementId::BankSlot(idx)) = hovered {
                if let Some(Some(_)) = state.ui_state.bank_slots.get(*idx) {
                    state.ui_state.bank_drag = Some(BankDrag {
                        from_slot: *idx,
                        mouse_start_x: mx,
                        mouse_start_y: my,
                        offset_x: 0.0, // will be refined when rendering
                        offset_y: 0.0,
                        active: false,
                    });
                    // Don't return yet - let the frame continue in case mouse
                    // is released same frame (instant click)
                }
            }
        }
```

Note: The existing `BankSlot(idx)` click handler in the click section below should be removed or guarded so it doesn't double-fire — since pending drag now handles the withdraw logic on mouse-up. The simplest approach: wrap the existing BankSlot click handler with `if state.ui_state.bank_drag.is_none()` so it only runs when no drag is in progress.

**Step 2: Build and verify**

Run: `cd client && cargo build 2>&1 | head -30`
Expected: Compiles

**Step 3: Commit**

```bash
git add client/src/input/handler.rs
git commit -m "implement bank drag state machine with dead zone"
```

---

## Task 9: Render Dragged Item and Visual Feedback

**Files:**
- Modify: `client/src/render/ui/bank.rs` (in `render_bank_grid` and after main bank render)

**Step 1: Dim the source slot during active drag**

In `render_bank_grid()` where individual slots are drawn (~lines 312-338), add a check: if `state.ui_state.bank_drag` is active and the current slot index matches `from_slot`, draw the slot dimmed (reduce alpha or skip drawing the item icon).

Inside the slot rendering loop, when drawing the item icon:

```rust
        let is_drag_source = state
            .ui_state
            .bank_drag
            .as_ref()
            .map(|d| d.active && d.from_slot == i)
            .unwrap_or(false);
```

If `is_drag_source`, skip drawing the item icon in that slot (or draw it at ~30% opacity).

**Step 2: Highlight the hovered target slot**

When actively dragging, if the hovered element is a `BankSlot`, draw a highlight border around it. Add after the slot background drawing:

```rust
        let is_drag_target = state
            .ui_state
            .bank_drag
            .as_ref()
            .map(|d| d.active && matches!(hovered, Some(UiElementId::BankSlot(h)) if *h == i))
            .unwrap_or(false);

        if is_drag_target {
            // Draw highlight border
            draw_rectangle_lines(sx, sy, slot_size, slot_size, 2.0, TEXT_GOLD);
        }
```

If the target slot contains the same item as the source, draw a small "+" to hint at merging:

```rust
        if is_drag_target {
            if let Some(ref drag) = state.ui_state.bank_drag {
                let src_item = state.ui_state.bank_slots.get(drag.from_slot)
                    .and_then(|s| s.as_ref()).map(|(id, _)| id.as_str());
                let dst_item = state.ui_state.bank_slots.get(i)
                    .and_then(|s| s.as_ref()).map(|(id, _)| id.as_str());
                if src_item.is_some() && src_item == dst_item {
                    self.draw_text_sharp("+", sx + 2.0, sy + 12.0, 12.0, TEXT_GOLD);
                }
            }
        }
```

**Step 3: Render the floating dragged item**

After the main bank grid and all UI is rendered (at the end of `render_bank`, so it draws on top of everything), add:

```rust
        // Render dragged item floating at cursor
        if let Some(ref drag) = state.ui_state.bank_drag {
            if drag.active {
                if let Some(Some((ref item_id, qty))) =
                    state.ui_state.bank_slots.get(drag.from_slot)
                {
                    let (mx, my) = mouse_position();
                    // Draw item icon at cursor position with reduced opacity
                    // Use the same slot_size as the grid slots
                    let drag_x = mx - slot_size / 2.0;
                    let drag_y = my - slot_size / 2.0;

                    // Semi-transparent background
                    draw_rectangle(
                        drag_x,
                        drag_y,
                        slot_size,
                        slot_size,
                        Color::new(0.12, 0.10, 0.16, 0.6),
                    );

                    // Draw item icon at 80% opacity
                    // Use the same icon rendering logic as render_bank_grid
                    if let Some(texture) = self.get_item_texture(item_id) {
                        draw_texture_ex(
                            texture,
                            drag_x,
                            drag_y,
                            Color::new(1.0, 1.0, 1.0, 0.8),
                            DrawTextureParams {
                                dest_size: Some(Vec2::new(slot_size, slot_size)),
                                ..Default::default()
                            },
                        );
                    }

                    // Draw quantity
                    if *qty > 1 {
                        let qty_str = format!("{}", qty);
                        self.draw_text_sharp(
                            &qty_str,
                            drag_x + 2.0,
                            drag_y + slot_size - 2.0,
                            10.0,
                            Color::new(1.0, 1.0, 0.6, 0.8),
                        );
                    }
                }
            }
        }
```

Note: Adapt this to match exactly how item icons are rendered in `render_bank_grid`. The texture lookup method name (`get_item_texture`) may differ — check the existing rendering code for the exact function/pattern used to draw item icons in bank slots and replicate it.

**Step 4: Build and verify**

Run: `cd client && cargo build 2>&1 | head -30`
Expected: Compiles

**Step 5: Commit**

```bash
git add client/src/render/ui/bank.rs
git commit -m "render dragged item, dim source slot, highlight drop target"
```

---

## Task 10: Clean Up Drag State on Bank Close

**Files:**
- Modify: `client/src/input/handler.rs` (BankCloseButton handler ~line 3534)
- Modify: `client/src/network/message_handler.rs` (where bank_open is set to false, if applicable)

**Step 1: Clear drag on bank close**

In the `BankCloseButton` handler, add:

```rust
    state.ui_state.bank_drag = None;
```

Also ensure drag is cleared when the bank is closed by server (e.g., walking away from banker). Check the message handler where `bank_open = false` is set and add the same clear there.

**Step 2: Clear drag when bank_open becomes false**

In `client/src/network/message_handler.rs`, if there's a handler that closes the bank, add `state.ui_state.bank_drag = None;` there too.

**Step 3: Build and verify**

Run: `cd client && cargo build 2>&1 | head -30`
Expected: Compiles

**Step 4: Commit**

```bash
git add client/src/input/handler.rs client/src/network/message_handler.rs
git commit -m "clear bank drag state on bank close"
```

---

## Task 11: Integration Test — Full Flow

**Step 1: Build both server and client**

Run: `cd rust-server && cargo build && cd ../client && cargo build`
Expected: Both compile successfully

**Step 2: Manual testing checklist**

Launch the game and test:

1. Open bank at a banker NPC
2. **Sort button visible**: Small "S" button in top-right of header, between help(?) and close(X)
3. **Sort button works**: Click "S" — items reorder by category then alphabetically, empty slots pack to end
4. **Drag to swap**: Click and drag an item to another slot — items swap positions
5. **Drag to merge**: Drag a stack onto the same item type — quantities merge (up to 99)
6. **Dead zone**: Click an item without moving mouse — normal withdraw behavior works as before
7. **Shift+click still works**: Shift+click on bank item withdraws all
8. **Ctrl+click still works**: Ctrl+click opens quantity dialog
9. **Cancel drag**: Right-click or Escape during drag cancels it
10. **Drag to empty**: Drag item to empty slot — item moves there
11. **Visual feedback**: Source slot dims during drag, target highlights, floating icon follows cursor
12. **Deposit all still works**: "Deposit All" button functions normally

**Step 3: Commit any fixes**

If any issues are found during testing, fix and commit.

**Step 4: Final commit**

```bash
git add -A
git commit -m "bank reorganization: drag+drop swap/merge and auto-sort button"
```
