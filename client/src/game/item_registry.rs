use macroquad::prelude::*;
use std::collections::HashMap;

/// Equipment stats for equippable items
#[derive(Debug, Clone)]
pub struct EquipmentStats {
    pub slot_type: String,
    pub attack_level_required: i32,      // For weapons
    pub defence_level_required: i32,     // For armor
    pub ranged_level_required: i32,      // For bows
    pub attack_bonus: i32,               // Accuracy
    pub strength_bonus: i32,             // Max hit
    pub defence_bonus: i32,              // Avoid hits
    pub magic_bonus: i32,                // Spell accuracy
    pub magic_level_required: i32,       // For staves/magic items
    pub woodcutting_level_required: i32, // For axes
    pub chop_speed_multiplier: f32,      // Woodcutting speed (0.0 if not an axe)
    pub mining_level_required: i32,      // For pickaxes
    pub mine_speed_multiplier: f32,      // Mining speed (0.0 if not a pickaxe)
    pub ranged_strength_bonus: i32,      // Ranged max hit bonus
}

/// Item definition received from server
#[derive(Debug, Clone)]
pub struct ItemDefinition {
    pub id: String,
    pub display_name: String,
    pub sprite: String,
    pub category: String,
    pub max_stack: i32,
    pub description: String,
    pub base_price: i32,
    pub sellable: bool,
    /// Equipment stats (only for equippable items)
    pub equipment: Option<EquipmentStats>,
    /// Weapon type (e.g., "melee", "bow", "staff")
    pub weapon_type: Option<String>,
    /// Attack range in tiles (1 for melee, >1 for ranged)
    pub range: Option<i32>,
    /// Prayer XP granted when offered at an altar
    pub prayer_xp: i32,
    /// Ranged strength bonus for ammunition (arrows)
    pub ranged_strength: i32,
    /// Use effect type (e.g. "dig", "heal") - determines context menu actions
    pub use_effect: Option<String>,
}

impl ItemDefinition {
    /// Get fallback color based on category (used when sprite not available)
    pub fn category_color(&self) -> Color {
        match self.category.as_str() {
            "consumable" => RED,
            "material" => GRAY,
            "equipment" => BLUE,
            "quest" => YELLOW,
            _ => WHITE,
        }
    }
}

/// Client-side item registry populated from server
pub struct ItemRegistry {
    items: HashMap<String, ItemDefinition>,
}

impl ItemRegistry {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    /// Load item definitions received from server
    pub fn load_from_server(&mut self, items: Vec<ItemDefinition>) {
        self.items.clear();
        for item in items {
            self.items.insert(item.id.clone(), item);
        }
        log::info!("Loaded {} item definitions from server", self.items.len());
    }

    /// Get item definition by ID
    pub fn get(&self, id: &str) -> Option<&ItemDefinition> {
        self.items.get(id)
    }

    /// Get item definition or a placeholder for unknown items
    pub fn get_or_placeholder(&self, id: &str) -> ItemDefinition {
        self.items
            .get(id)
            .cloned()
            .unwrap_or_else(|| ItemDefinition {
                id: id.to_string(),
                display_name: format!("Unknown ({})", id),
                sprite: "item_unknown".to_string(),
                category: "material".to_string(),
                max_stack: 99,
                description: "Unknown item".to_string(),
                base_price: 0,
                sellable: false,
                equipment: None,
                weapon_type: None,
                range: None,
                prayer_xp: 0,
                ranged_strength: 0,
                use_effect: None,
            })
    }

    /// Get the sprite key for an item ID (resolves item_id → sprite field)
    pub fn get_sprite_key<'a>(&'a self, id: &'a str) -> &'a str {
        self.items
            .get(id)
            .map(|def| def.sprite.as_str())
            .unwrap_or(id)
    }

    /// Get display name for an item ID
    pub fn get_display_name<'a>(&'a self, id: &'a str) -> &'a str {
        self.items
            .get(id)
            .map(|def| def.display_name.as_str())
            .unwrap_or(id)
    }

    /// Check if registry has any items loaded
    pub fn is_loaded(&self) -> bool {
        !self.items.is_empty()
    }

    /// Get the number of items in the registry
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Default for ItemRegistry {
    fn default() -> Self {
        Self::new()
    }
}
