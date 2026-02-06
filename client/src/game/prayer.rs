//! Prayer definitions for the client-side prayer system
//! Contains prayer data used for UI display and validation

/// Prayer category for color-coding and grouping
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PrayerCategory {
    Attack,
    Defence,
    Strength,
    Gathering,
    HpRegen,
    Protection,
}

/// Static prayer definition for UI display
#[derive(Clone, Debug)]
pub struct PrayerDef {
    pub id: &'static str,
    pub name: &'static str,
    pub level_req: i32,
    pub category: PrayerCategory,
    pub description: &'static str,
}

/// All 14 prayers in display order
pub const PRAYERS: [PrayerDef; 14] = [
    PrayerDef { id: "clarity", name: "Clarity", level_req: 1, category: PrayerCategory::Attack, description: "+5% Attack accuracy" },
    PrayerDef { id: "thick_skin", name: "Thick Skin", level_req: 1, category: PrayerCategory::Defence, description: "+5% Defence" },
    PrayerDef { id: "burst_of_strength", name: "Burst of Strength", level_req: 4, category: PrayerCategory::Strength, description: "+5% Strength" },
    PrayerDef { id: "improved_clarity", name: "Improved Clarity", level_req: 10, category: PrayerCategory::Attack, description: "+10% Attack accuracy" },
    PrayerDef { id: "rock_skin", name: "Rock Skin", level_req: 10, category: PrayerCategory::Defence, description: "+10% Defence" },
    PrayerDef { id: "superhuman_strength", name: "Superhuman Strength", level_req: 13, category: PrayerCategory::Strength, description: "+10% Strength" },
    PrayerDef { id: "resourcefulness", name: "Resourcefulness", level_req: 16, category: PrayerCategory::Gathering, description: "+10% Gathering yield" },
    PrayerDef { id: "rapid_heal", name: "Rapid Heal", level_req: 22, category: PrayerCategory::HpRegen, description: "2x HP regeneration" },
    PrayerDef { id: "steel_skin", name: "Steel Skin", level_req: 28, category: PrayerCategory::Defence, description: "+15% Defence" },
    PrayerDef { id: "incredible_clarity", name: "Incredible Clarity", level_req: 31, category: PrayerCategory::Attack, description: "+15% Attack accuracy" },
    PrayerDef { id: "ultimate_strength", name: "Ultimate Strength", level_req: 31, category: PrayerCategory::Strength, description: "+15% Strength" },
    PrayerDef { id: "protection", name: "Protection", level_req: 37, category: PrayerCategory::Protection, description: "Reduce damage by 25%" },
    PrayerDef { id: "greater_resourcefulness", name: "Greater Resourcefulness", level_req: 40, category: PrayerCategory::Gathering, description: "+20% Gathering yield" },
    PrayerDef { id: "greater_protection", name: "Greater Protection", level_req: 55, category: PrayerCategory::Protection, description: "Reduce damage by 40%" },
];
