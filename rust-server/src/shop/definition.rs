//! Shop Definition Structures
//!
//! Defines data structures for shop definitions and stock management.

use serde::{Deserialize, Serialize};

/// A shop definition with stock management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopDefinition {
    pub id: String,
    pub display_name: String,
    pub stock: Vec<ShopStockItem>,
}

/// An item stocked in a shop with quantity and restock configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopStockItem {
    pub item_id: String,
    #[serde(default)]
    pub max_quantity: i32,
    #[serde(default)]
    pub restock_rate: i32,
    /// When true, the item is in infinite supply: it never depletes on purchase
    /// and is skipped during restock. `max_quantity`/`restock_rate` are ignored.
    #[serde(default)]
    pub unlimited: bool,
    #[serde(skip)]
    pub current_quantity: i32,
}

impl ShopDefinition {
    /// Initialize stock quantities to their maximum values
    pub fn initialize_stock(&mut self) {
        for item in &mut self.stock {
            item.current_quantity = item.max_quantity;
        }
    }

    /// Get immutable reference to stock item by item_id
    pub fn get_stock(&self, item_id: &str) -> Option<&ShopStockItem> {
        self.stock.iter().find(|s| s.item_id == item_id)
    }

    /// Get mutable reference to stock item by item_id
    pub fn get_stock_mut(&mut self, item_id: &str) -> Option<&mut ShopStockItem> {
        self.stock.iter_mut().find(|s| s.item_id == item_id)
    }

    /// Restock all items by their restock_rate, capped at max_quantity.
    /// Returns only the stock entries that actually changed.
    pub fn restock(&mut self) -> Vec<(String, i32)> {
        let mut changed = Vec::new();
        for item in &mut self.stock {
            if item.unlimited {
                continue;
            }
            let old_quantity = item.current_quantity;
            let new_quantity = (item.current_quantity + item.restock_rate).min(item.max_quantity);
            if new_quantity != old_quantity {
                item.current_quantity = new_quantity;
                changed.push((item.item_id.clone(), new_quantity));
            }
        }
        changed
    }
}
