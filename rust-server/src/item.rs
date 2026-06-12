use serde::Serialize;

use crate::data::ItemRegistry;

// ============================================================================
// Constants
// ============================================================================

/// Gold item ID - gold is handled specially (stored in inventory.gold field)
pub const GOLD_ITEM_ID: &str = "gold";
pub const MAX_GOLD: i32 = i32::MAX;

pub fn checked_gold_total(unit_price: i32, quantity: i32) -> Option<i32> {
    if unit_price <= 0 || quantity <= 0 {
        return None;
    }
    unit_price.checked_mul(quantity)
}

pub fn checked_gold_credit(balance: i32, amount: i32) -> Option<i32> {
    if balance < 0 || amount < 0 {
        return None;
    }
    balance.checked_add(amount)
}

pub fn checked_gold_debit(balance: i32, amount: i32) -> Option<i32> {
    if balance < 0 || amount < 0 {
        return None;
    }
    balance.checked_sub(amount).filter(|total| *total >= 0)
}

/// Default max stack for unknown items
pub const DEFAULT_MAX_STACK: i32 = 99;

// ============================================================================
// Inventory
// ============================================================================

pub const INVENTORY_SIZE: usize = 20;

#[derive(Debug, Clone, Serialize)]
pub struct InventorySlot {
    pub item_id: String,
    pub quantity: i32,
}

impl InventorySlot {
    pub fn new(item_id: String, quantity: i32) -> Self {
        Self { item_id, quantity }
    }
}

#[derive(Debug, Clone)]
pub struct Inventory {
    pub slots: Vec<Option<InventorySlot>>,
    pub gold: i32, // Gold is stored separately
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            slots: vec![None; INVENTORY_SIZE],
            gold: 0,
        }
    }

    /// Try to add an item to inventory. Returns the quantity that couldn't fit.
    pub fn add_item(&mut self, item_id: &str, mut quantity: i32, registry: &ItemRegistry) -> i32 {
        // Gold goes to separate counter
        if item_id == GOLD_ITEM_ID {
            if let Some(new_gold) = checked_gold_credit(self.gold, quantity) {
                self.gold = new_gold;
                return 0;
            }
            return quantity;
        }

        let max_stack = registry
            .get(item_id)
            .map(|def| def.max_stack)
            .unwrap_or(DEFAULT_MAX_STACK);

        // First, try to stack with existing items
        for slot in &mut self.slots {
            if quantity <= 0 {
                break;
            }
            if let Some(inv_slot) = slot
                && inv_slot.item_id == item_id
            {
                let can_add = max_stack - inv_slot.quantity;
                if can_add > 0 {
                    let add = quantity.min(can_add);
                    inv_slot.quantity += add;
                    quantity -= add;
                }
            }
        }

        // Then, try to find empty slots for remaining quantity
        for slot in &mut self.slots {
            if quantity <= 0 {
                break;
            }
            if slot.is_none() {
                let add = quantity.min(max_stack);
                *slot = Some(InventorySlot::new(item_id.to_string(), add));
                quantity -= add;
            }
        }

        quantity // Return what couldn't fit
    }

    /// Use an item at the given slot. Returns the item_id if successful.
    pub fn use_item(&mut self, slot_index: usize, registry: &ItemRegistry) -> Option<String> {
        if slot_index >= INVENTORY_SIZE {
            return None;
        }

        if let Some(ref mut slot) = self.slots[slot_index] {
            // Check if item is usable via registry
            let is_usable = registry
                .get(&slot.item_id)
                .map(|def| def.is_usable())
                .unwrap_or(false);

            if is_usable {
                let item_id = slot.item_id.clone();
                slot.quantity -= 1;
                if slot.quantity <= 0 {
                    self.slots[slot_index] = None;
                }
                return Some(item_id);
            }
        }
        None
    }

    /// Get inventory as a serializable update
    pub fn to_update(&self) -> Vec<InventorySlotUpdate> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| {
                slot.as_ref()
                    .filter(|s| s.quantity > 0 && !s.item_id.is_empty())
                    .map(|s| InventorySlotUpdate {
                        slot: i as u8,
                        item_id: s.item_id.clone(),
                        quantity: s.quantity,
                    })
            })
            .collect()
    }

    /// Check if inventory has at least `count` of the specified item
    pub fn has_item(&self, item_id: &str, count: i32) -> bool {
        self.count_item(item_id) >= count
    }

    /// Count total quantity of an item across all slots
    pub fn count_item(&self, item_id: &str) -> i32 {
        self.slots
            .iter()
            .filter_map(|slot| slot.as_ref())
            .filter(|slot| slot.item_id == item_id)
            .map(|slot| slot.quantity)
            .sum()
    }

    /// Remove a specific quantity of an item from inventory
    /// Returns true if successful, false if not enough items
    pub fn remove_item(&mut self, item_id: &str, mut count: i32) -> bool {
        // First check if we have enough
        if !self.has_item(item_id, count) {
            return false;
        }

        // Remove from slots (prefer partial stacks first to consolidate)
        for slot in &mut self.slots {
            if count <= 0 {
                break;
            }
            if let Some(inv_slot) = slot
                && inv_slot.item_id == item_id
            {
                let remove = count.min(inv_slot.quantity);
                inv_slot.quantity -= remove;
                count -= remove;

                // Clear slot if empty
                if inv_slot.quantity <= 0 {
                    *slot = None;
                }
            }
        }

        true
    }

    /// Check if inventory has space for additional items
    pub fn has_space_for(&self, item_id: &str, count: i32, registry: &ItemRegistry) -> bool {
        // Gold always has space (stored separately)
        if item_id == GOLD_ITEM_ID {
            return true;
        }

        let max_stack = registry
            .get(item_id)
            .map(|def| def.max_stack)
            .unwrap_or(DEFAULT_MAX_STACK);
        let mut remaining = count;

        // Check existing stacks for available space
        for slot in &self.slots {
            if remaining <= 0 {
                return true;
            }
            if let Some(inv_slot) = slot
                && inv_slot.item_id == item_id
            {
                let can_add = max_stack - inv_slot.quantity;
                remaining -= can_add;
            }
        }

        if remaining <= 0 {
            return true;
        }

        // Check empty slots
        let empty_slots = self.slots.iter().filter(|s| s.is_none()).count();
        let slots_needed = ((remaining + max_stack - 1) / max_stack).max(0) as usize;

        empty_slots >= slots_needed
    }

    /// Calculate how many of an item can fit in inventory
    pub fn available_space_for(&self, item_id: &str, registry: &ItemRegistry) -> i32 {
        if item_id == GOLD_ITEM_ID {
            return i32::MAX;
        }

        let max_stack = registry
            .get(item_id)
            .map(|def| def.max_stack)
            .unwrap_or(DEFAULT_MAX_STACK);
        let mut space = 0;

        for slot in &self.slots {
            match slot {
                Some(inv_slot) if inv_slot.item_id == item_id => {
                    space += max_stack - inv_slot.quantity;
                }
                None => {
                    space += max_stack;
                }
                _ => {}
            }
        }

        space
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct InventorySlotUpdate {
    pub slot: u8,
    pub item_id: String,
    pub quantity: i32,
}

// ============================================================================
// Bank Vault
// ============================================================================

pub const DEFAULT_BANK_SIZE: usize = 50;
/// Bank uses unlimited stacking: one slot per item type, quantity up to i32::MAX
/// Number of slots added per bank upgrade purchase
pub const BANK_UPGRADE_SLOTS: usize = 10;
/// Maximum bank size after all upgrades
pub const BANK_MAX_SIZE: usize = 200;
/// Gold cost per bank upgrade
pub const BANK_UPGRADE_COST: i32 = 5_000;

#[derive(Debug, Clone)]
pub struct Bank {
    pub slots: Vec<Option<InventorySlot>>,
    pub gold: i32,
}

impl Bank {
    pub fn new() -> Self {
        Self {
            slots: vec![None; DEFAULT_BANK_SIZE],
            gold: 0,
        }
    }

    pub fn new_with_size(size: usize) -> Self {
        Self {
            slots: vec![None; size],
            gold: 0,
        }
    }

    /// Expand bank by appending additional empty slots
    pub fn expand(&mut self, additional: usize) {
        self.slots.extend(std::iter::repeat_n(None, additional));
    }

    /// Try to add an item to the bank. Returns the quantity that couldn't fit.
    /// Each item type occupies exactly one slot with unlimited stacking.
    pub fn add_item(&mut self, item_id: &str, quantity: i32, _registry: &ItemRegistry) -> i32 {
        if item_id == GOLD_ITEM_ID {
            if let Some(new_gold) = checked_gold_credit(self.gold, quantity) {
                self.gold = new_gold;
                return 0;
            }
            return quantity;
        }

        // Find existing slot with this item and add to it
        for slot in &mut self.slots {
            if let Some(inv_slot) = slot
                && inv_slot.item_id == item_id
            {
                inv_slot.quantity = inv_slot.quantity.saturating_add(quantity);
                return 0;
            }
        }

        // No existing slot — place in first empty slot
        for slot in &mut self.slots {
            if slot.is_none() {
                *slot = Some(InventorySlot::new(item_id.to_string(), quantity));
                return 0;
            }
        }

        // No space at all
        quantity
    }

    /// Remove a specific quantity of an item from the bank.
    /// Returns true if successful, false if not enough items.
    pub fn remove_item(&mut self, item_id: &str, mut count: i32) -> bool {
        if !self.has_item(item_id, count) {
            return false;
        }

        for slot in &mut self.slots {
            if count <= 0 {
                break;
            }
            if let Some(inv_slot) = slot
                && inv_slot.item_id == item_id
            {
                let remove = count.min(inv_slot.quantity);
                inv_slot.quantity -= remove;
                count -= remove;
                if inv_slot.quantity <= 0 {
                    *slot = None;
                }
            }
        }

        true
    }

    /// Check if bank has at least `count` of the specified item
    pub fn has_item(&self, item_id: &str, count: i32) -> bool {
        self.count_item(item_id) >= count
    }

    /// Count total quantity of an item across all slots
    pub fn count_item(&self, item_id: &str) -> i32 {
        self.slots
            .iter()
            .filter_map(|slot| slot.as_ref())
            .filter(|slot| slot.item_id == item_id)
            .map(|slot| slot.quantity)
            .sum()
    }

    /// Check if bank has space for additional items.
    /// With unlimited stacking, space exists if the item already has a slot or there's an empty slot.
    pub fn has_space_for(&self, item_id: &str, _count: i32, _registry: &ItemRegistry) -> bool {
        if item_id == GOLD_ITEM_ID {
            return true;
        }

        // If this item already exists in the bank, it stacks into that slot
        for slot in &self.slots {
            if let Some(inv_slot) = slot
                && inv_slot.item_id == item_id
            {
                return true;
            }
        }

        // Otherwise, need at least one empty slot
        self.slots.iter().any(|s| s.is_none())
    }

    /// Get bank contents as a serializable update
    pub fn to_update(&self) -> Vec<InventorySlotUpdate> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| {
                slot.as_ref()
                    .filter(|s| s.quantity > 0 && !s.item_id.is_empty())
                    .map(|s| InventorySlotUpdate {
                        slot: i as u8,
                        item_id: s.item_id.clone(),
                        quantity: s.quantity,
                    })
            })
            .collect()
    }
}

// ============================================================================
// Ground Item (dropped in world)
// ============================================================================

#[derive(Debug, Clone)]
pub struct GroundItem {
    pub id: String,
    pub item_id: String,
    pub x: f32,
    pub y: f32,
    pub quantity: i32,
    pub owner_id: Option<String>, // Player who can pick up (None = anyone)
    pub drop_time: u64,           // When the item was dropped
    pub instance_id: Option<String>, // Which instance this item is in (None = overworld)
}

impl GroundItem {
    pub fn new(
        id: &str,
        item_id: &str,
        x: f32,
        y: f32,
        quantity: i32,
        owner_id: Option<String>,
        current_time: u64,
    ) -> Self {
        Self {
            id: id.to_string(),
            item_id: item_id.to_string(),
            x,
            y,
            quantity,
            owner_id,
            drop_time: current_time,
            instance_id: None,
        }
    }

    pub fn new_in_instance(
        id: &str,
        item_id: &str,
        x: f32,
        y: f32,
        quantity: i32,
        owner_id: Option<String>,
        current_time: u64,
        instance_id: Option<String>,
    ) -> Self {
        Self {
            id: id.to_string(),
            item_id: item_id.to_string(),
            x,
            y,
            quantity,
            owner_id,
            drop_time: current_time,
            instance_id,
        }
    }

    /// Check if the item has expired (60 second lifetime)
    pub fn is_expired(&self, current_time: u64) -> bool {
        const ITEM_LIFETIME_MS: u64 = 60000; // 60 seconds
        // Guard against underflow: items created in the same tick (via auto-action)
        // can have drop_time slightly after the tick's current_time
        current_time.saturating_sub(self.drop_time) > ITEM_LIFETIME_MS
    }

    /// Check if a player can pick up this item
    pub fn can_pickup(&self, player_id: &str, current_time: u64) -> bool {
        // Owner-only period: first 10 seconds
        const OWNER_PERIOD_MS: u64 = 10000;

        if current_time - self.drop_time < OWNER_PERIOD_MS {
            // During owner period, only owner can pick up
            match &self.owner_id {
                Some(owner) => owner == player_id,
                None => true, // No owner = anyone can pick up
            }
        } else {
            // After owner period, anyone can pick up
            true
        }
    }
}

// ============================================================================
// Item Update (sent to client)
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct GroundItemUpdate {
    pub id: String,
    pub item_id: String,
    pub x: f32,
    pub y: f32,
    pub quantity: i32,
}

impl From<&GroundItem> for GroundItemUpdate {
    fn from(item: &GroundItem) -> Self {
        Self {
            id: item.id.clone(),
            item_id: item.item_id.clone(),
            x: item.x,
            y: item.y,
            quantity: item.quantity,
        }
    }
}

#[cfg(test)]
mod economy_tests {
    use super::*;

    #[test]
    fn rejects_reported_stall_total_overflow() {
        assert_eq!(checked_gold_total(1_610_612_737, 4), None);
    }

    #[test]
    fn rejects_credit_above_gold_cap() {
        assert_eq!(checked_gold_credit(i32::MAX, 1), None);
        assert_eq!(checked_gold_credit(i32::MAX - 1, 1), Some(i32::MAX));
    }

    #[test]
    fn rejects_negative_or_insufficient_balances() {
        assert_eq!(checked_gold_credit(-1, 1), None);
        assert_eq!(checked_gold_debit(-1, 1), None);
        assert_eq!(checked_gold_debit(3, 4), None);
        assert_eq!(checked_gold_debit(4, 4), Some(0));
    }
}
