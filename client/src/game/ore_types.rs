//! Ore type definitions for mining
//! Maps rock GIDs to their display name and level requirements

/// Information about an ore type
#[derive(Debug, Clone, Copy)]
pub struct OreTypeInfo {
    pub name: &'static str,
    pub level_required: i32,
}

const COPPER: OreTypeInfo = OreTypeInfo {
    name: "Copper Rock",
    level_required: 1,
};
const TIN: OreTypeInfo = OreTypeInfo {
    name: "Tin Rock",
    level_required: 5,
};
const IRON: OreTypeInfo = OreTypeInfo {
    name: "Iron Rock",
    level_required: 15,
};
const STEEL: OreTypeInfo = OreTypeInfo {
    name: "Steel Rock",
    level_required: 30,
};

/// Get ore info for a given GID, if it's a rock
pub fn get_ore_info(gid: u32) -> Option<&'static OreTypeInfo> {
    match gid {
        // Copper rocks: mapper id 454 → GID 1616
        1616 => Some(&COPPER),

        // Tin rocks (reuses old iron rock sprite)
        1581 => Some(&TIN),

        // Iron rocks: mapper id 455 → GID 1617
        1617 => Some(&IRON),

        // Steel rocks: GID 9997
        9997 => Some(&STEEL),

        _ => None,
    }
}
