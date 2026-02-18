//! Tree type definitions for woodcutting
//! Maps tree GIDs to their display name and level requirements

/// Information about a tree type
#[derive(Debug, Clone, Copy)]
pub struct TreeTypeInfo {
    pub name: &'static str,
    pub level_required: i32,
}

/// Oak tree info
const OAK: TreeTypeInfo = TreeTypeInfo {
    name: "Oak Tree",
    level_required: 1,
};
/// Willow tree info
const WILLOW: TreeTypeInfo = TreeTypeInfo {
    name: "Willow Tree",
    level_required: 15,
};
/// Maple tree info
const MAPLE: TreeTypeInfo = TreeTypeInfo {
    name: "Maple Tree",
    level_required: 45,
};
/// Yew tree info
const YEW: TreeTypeInfo = TreeTypeInfo {
    name: "Yew Tree",
    level_required: 60,
};

/// Get tree info for a given GID, if it's a tree
pub fn get_tree_info(gid: u32) -> Option<&'static TreeTypeInfo> {
    match gid {
        // Oak trees: sprites 101, 102, 103, 286-290, 647, 648, 831
        // GIDs = 1162 + sprite_number
        1263 | 1264 | 1265 | 1448 | 1449 | 1450 | 1451 | 1452 | 1809 | 1810 | 1993 => Some(&OAK),

        // Willow trees: sprites 528, 529, 530
        1690 | 1691 | 1692 => Some(&WILLOW),

        // Maple trees: sprites 985-992
        2147 | 2148 | 2149 | 2150 | 2151 | 2152 | 2153 | 2154 => Some(&MAPLE),

        // Yew trees: sprites 993-1000
        2155 | 2156 | 2157 | 2158 | 2159 | 2160 | 2161 | 2162 => Some(&YEW),

        _ => None,
    }
}

/// Check if a GID represents a tree
pub fn is_tree_gid(gid: u32) -> bool {
    get_tree_info(gid).is_some()
}
