#!/bin/bash
# Generate sprite manifest for Android builds
# Run this from the client directory whenever sprites are added/removed
#
# Updates only the sprite list arrays (enemies, equipment, weapons, inventory,
# objects, walls) while preserving all other fields (weapon_frame_sizes, atlases, etc.)

set -e

cd "$(dirname "$0")/.."

MANIFEST="assets/sprite_manifest.json"

echo "Generating sprite manifest..."

# Helper: produce a JSON array from .png files in a directory
json_array() {
    local dir="$1"
    local strip_prefix="$2"

    find "$dir" -name "*.png" 2>/dev/null | \
        sed "s|${strip_prefix}/||;s|\.png$||" | \
        sort | \
        jq -R . | jq -s .
}

enemies=$(json_array "assets/sprites/enemies" "assets/sprites/enemies")
equipment=$(json_array "assets/sprites/equipment" "assets/sprites")
weapons=$(json_array "assets/sprites/weapons" "assets/sprites/weapons")
inventory=$(json_array "assets/sprites/inventory" "assets/sprites/inventory")
objects=$(json_array "assets/sprites/objects" "assets/sprites/objects")
walls=$(json_array "assets/sprites/walls" "assets/sprites/walls")

# Merge the new arrays into the existing manifest, preserving all other keys
jq \
    --argjson enemies "$enemies" \
    --argjson equipment "$equipment" \
    --argjson weapons "$weapons" \
    --argjson inventory "$inventory" \
    --argjson objects "$objects" \
    --argjson walls "$walls" \
    '.enemies = $enemies | .equipment = $equipment | .weapons = $weapons | .inventory = $inventory | .objects = $objects | .walls = $walls' \
    "$MANIFEST" > "${MANIFEST}.tmp" && mv "${MANIFEST}.tmp" "$MANIFEST"

echo "Generated $MANIFEST"
echo "  - enemies: $(echo "$enemies" | jq length)"
echo "  - equipment: $(echo "$equipment" | jq length)"
echo "  - weapons: $(echo "$weapons" | jq length)"
echo "  - inventory: $(echo "$inventory" | jq length)"
echo "  - objects: $(echo "$objects" | jq length)"
echo "  - walls: $(echo "$walls" | jq length)"
