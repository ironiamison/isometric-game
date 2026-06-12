//! Crafting Recipe Definitions
//!
//! Defines the data structures for crafting recipes, including TOML
//! deserialization (Raw*) and resolved versions with defaults applied.

use serde::{Deserialize, Serialize};

/// Recipe categories for UI organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RecipeCategory {
    Consumables,
    #[default]
    Materials,
    Equipment,
    Tools,
    Smithing,
    Alchemy,
    Cooking,
    Fletching,
    Leatherworking,
}

impl RecipeCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecipeCategory::Consumables => "consumables",
            RecipeCategory::Materials => "materials",
            RecipeCategory::Equipment => "equipment",
            RecipeCategory::Tools => "tools",
            RecipeCategory::Smithing => "smithing",
            RecipeCategory::Alchemy => "alchemy",
            RecipeCategory::Cooking => "cooking",
            RecipeCategory::Fletching => "fletching",
            RecipeCategory::Leatherworking => "leatherworking",
        }
    }
}

// ============================================================================
// Raw TOML Structures
// ============================================================================

fn default_count() -> i32 {
    1
}

/// Raw ingredient entry from TOML
#[derive(Debug, Clone, Deserialize)]
pub struct RawIngredient {
    pub item_id: String,
    #[serde(default = "default_count")]
    pub count: i32,
}

/// Raw result entry from TOML
#[derive(Debug, Clone, Deserialize)]
pub struct RawResult {
    pub item_id: String,
    #[serde(default = "default_count")]
    pub count: i32,
}

/// Raw recipe definition from TOML
#[derive(Debug, Clone, Deserialize)]
pub struct RawRecipeDefinition {
    pub display_name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub category: RecipeCategory,
    pub section: Option<String>,
    #[serde(default)]
    pub level_required: i32,
    #[serde(default)]
    pub ingredients: Vec<RawIngredient>,
    #[serde(default)]
    pub results: Vec<RawResult>,
    pub station: Option<String>,
    pub craft_time_ms: Option<u64>,
    pub xp: Option<u32>,
    pub requires_discovery: Option<bool>,
    pub required_tool: Option<String>,
    pub burn_result: Option<String>,
    pub burn_stop_level: Option<i32>,
}

// ============================================================================
// Resolved Structures
// ============================================================================

/// Ingredient in a resolved recipe
#[derive(Debug, Clone, Serialize)]
pub struct Ingredient {
    pub item_id: String,
    pub count: i32,
}

/// Result from crafting
#[derive(Debug, Clone, Serialize)]
pub struct CraftResult {
    pub item_id: String,
    pub count: i32,
}

/// A fully resolved recipe definition
#[derive(Debug, Clone)]
pub struct RecipeDefinition {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub category: RecipeCategory,
    pub section: Option<String>,
    pub level_required: i32,
    pub ingredients: Vec<Ingredient>,
    pub results: Vec<CraftResult>,
    pub station: Option<String>,
    pub craft_time_ms: u64,
    pub xp: u32,
    pub requires_discovery: bool,
    pub required_tool: Option<String>,
    pub burn_result: Option<String>,
    pub burn_stop_level: Option<i32>,
}

impl RecipeDefinition {
    /// Create a resolved RecipeDefinition from raw TOML data
    pub fn from_raw(id: &str, raw: &RawRecipeDefinition) -> Self {
        Self {
            id: id.to_string(),
            display_name: raw
                .display_name
                .clone()
                .unwrap_or_else(|| id.replace('_', " ")),
            description: raw.description.clone().unwrap_or_default(),
            category: raw.category,
            section: raw.section.clone(),
            level_required: raw.level_required,
            ingredients: raw
                .ingredients
                .iter()
                .map(|i| Ingredient {
                    item_id: i.item_id.clone(),
                    count: i.count,
                })
                .collect(),
            results: raw
                .results
                .iter()
                .map(|r| CraftResult {
                    item_id: r.item_id.clone(),
                    count: r.count,
                })
                .collect(),
            station: raw.station.clone(),
            craft_time_ms: raw.craft_time_ms.unwrap_or(0),
            xp: raw.xp.unwrap_or(0),
            requires_discovery: raw.requires_discovery.unwrap_or(false),
            required_tool: raw.required_tool.clone(),
            burn_result: raw.burn_result.clone(),
            burn_stop_level: raw.burn_stop_level,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_recipe() {
        let toml_str = r#"
            [test_recipe]
            display_name = "Test Recipe"
            description = "A test recipe"
            category = "consumables"
            level_required = 5

            [[test_recipe.ingredients]]
            item_id = "slime_core"
            count = 3

            [[test_recipe.results]]
            item_id = "health_potion"
            count = 1
        "#;

        let parsed: std::collections::HashMap<String, RawRecipeDefinition> =
            toml::from_str(toml_str).unwrap();

        assert!(parsed.contains_key("test_recipe"));
        let raw = &parsed["test_recipe"];
        assert_eq!(raw.display_name, Some("Test Recipe".to_string()));
        assert_eq!(raw.category, RecipeCategory::Consumables);
        assert_eq!(raw.level_required, 5);
        assert_eq!(raw.ingredients.len(), 1);
        assert_eq!(raw.results.len(), 1);
    }

    #[test]
    fn test_recipe_defaults() {
        let toml_str = r#"
            [minimal]
            [[minimal.ingredients]]
            item_id = "test"
            [[minimal.results]]
            item_id = "output"
        "#;

        let parsed: std::collections::HashMap<String, RawRecipeDefinition> =
            toml::from_str(toml_str).unwrap();

        let recipe = RecipeDefinition::from_raw("minimal", &parsed["minimal"]);
        assert_eq!(recipe.display_name, "minimal");
        assert_eq!(recipe.category, RecipeCategory::Materials);
        assert_eq!(recipe.level_required, 0);
        assert_eq!(recipe.ingredients[0].count, 1);
    }
}
