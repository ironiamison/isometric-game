# Bank Reorganization: Drag & Drop + Auto-Sort

## Overview

Add drag-and-drop rearrangement within the bank grid, plus an auto-sort button. All operations are server-authoritative, consistent with the existing bank architecture.

## New Protocol Messages

### Client → Server

- **`BankSwapSlots { slot_a: u32, slot_b: u32 }`** — Swap two bank slots. If both contain the same item type, merge stacks (up to 99). Otherwise, swap positions. Moving to an empty slot is just a swap with `None`.
- **`BankSort`** — Auto-sort the entire bank by category then alphabetically.

### Server → Client

No new server messages needed — both operations respond with the existing `BankUpdate { slots, gold }` message.

## Server Handling

### BankSwapSlots

1. Validate both slot indices are within `0..bank.slots.len()`
2. If both slots contain the same `item_id`, merge: combine quantities up to 99, leave remainder in source slot (or `None` if fully merged)
3. Otherwise, swap the two `Option<InventorySlot>` entries
4. Send `BankUpdate` with new state

### BankSort

1. Collect all `Some` slots into a vec
2. Sort by `(category_priority, display_name)`:
   - Equipment = 0
   - Consumable = 1
   - Material = 2
   - Quest = 3
3. Rebuild slots vec: sorted items packed to front, `None` for remaining empty slots
4. Send `BankUpdate` with new state

Category is derived from the existing `ItemCategory` enum on `ItemDefinition` — no new data fields needed.

**Edge cases:**
- Split stacks of the same item sort adjacent but do NOT auto-merge (predictable behavior)
- Empty bank or single-item bank: no-op, still sends BankUpdate for consistency

## Client-Side Drag & Drop

### New State

```rust
pub struct BankDrag {
    pub from_slot: usize,      // which bank slot we picked up
    pub offset_x: f32,         // mouse offset from item icon top-left
    pub offset_y: f32,         // so the icon doesn't snap to cursor center
}
```

Added to `GameState`:
```rust
pub bank_drag: Option<BankDrag>
```

### Interaction Flow

1. **Mouse down** on a bank slot with an item → record slot index and cursor offset, but don't start drag yet
2. **Mouse move** while held, exceeds 4px dead zone → start drag (set `bank_drag`)
3. **Mouse move** while dragging → render dragged item icon at cursor, highlight hovered target slot
4. **Mouse up** over another bank slot → send `BankSwapSlots { from_slot, to_slot }`, clear drag state
5. **Mouse up** outside bank grid / over nothing → cancel, clear drag state
6. **Right-click or Escape** while dragging → cancel, clear drag state

The 4px dead zone prevents accidental drags when the user just wants to click (deposit/withdraw). Existing click behavior is fully preserved.

### Visual Feedback

- **Source slot**: dimmed/empty appearance while dragging
- **Dragged icon**: rendered on top of everything at ~80% opacity, follows cursor with offset
- **Target slot**: highlight border when hovered
- **Merge hint**: subtle "+" indicator when hovering over a stack of the same item type

## Auto-Sort Button

### UI Placement

Small icon button in the top-right of the bank panel, near the close (X) button. Unobtrusive, doesn't take up much space.

- Tooltip on hover: "Sort bank"
- Sends `BankSort` message on click

## Scope Summary

- 2 new protocol messages (`BankSwapSlots`, `BankSort`)
- 2 new server handlers (swap + sort logic)
- Client drag state tracking + dead zone detection
- Dragged item rendering layer
- 1 new button in bank UI header
- No changes to existing click-based deposit/withdraw behavior
- No new item definition fields needed (uses existing `ItemCategory`)
