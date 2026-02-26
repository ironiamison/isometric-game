#!/usr/bin/env python3
"""
Sprite atlas packer for WASM fast loading.

Combines individual sprite PNGs into atlas textures with JSON metadata.
Categories packed: objects, walls, inventory, players, hair, equipment,
weapons, enemies, farming, effects.

Each spritesheet is packed as a single unit to preserve animation logic.

Usage: python3 tools/pack_atlases.py
"""

import json
import os
from pathlib import Path

from PIL import Image

ASSETS_DIR = Path(__file__).parent.parent / "client" / "assets"
SPRITES_DIR = ASSETS_DIR / "sprites"
UI_DIR = ASSETS_DIR / "ui"
MANIFEST_PATH = ASSETS_DIR / "sprite_manifest.json"
ANIMATED_SPRITES_PATH = ASSETS_DIR / "animated_sprites.json"
MAX_ATLAS_SIZE = 4096


def collect_sprites(directory: Path, recursive: bool = False, base_dir: Path = None, key_transform=None):
    """Collect all PNGs in a directory, returning {key: path} sorted by height desc.

    If recursive=True, recursively scan subdirectories and use relative paths as keys
    (e.g., "equipment/back/backpack" for equipment/back/backpack.png).

    key_transform: optional function to transform the key before storing.
    """
    sprites = {}
    if base_dir is None:
        base_dir = directory

    if recursive:
        for png in sorted(directory.rglob("*.png")):
            # Use relative path from base_dir as key (without .png extension)
            rel_path = png.relative_to(base_dir)
            key = str(rel_path.with_suffix(""))
            if key_transform:
                key = key_transform(key)
            sprites[key] = png
    else:
        for png in sorted(directory.glob("*.png")):
            key = png.stem
            if key_transform:
                key = key_transform(key)
            sprites[key] = png
    return sprites


def pack_atlas(sprites: dict[str, Path]):
    """
    Row-pack sprites into atlas(es) up to MAX_ATLAS_SIZE x MAX_ATLAS_SIZE.
    Returns list of (Image, {key: {x, y, w, h}}) tuples.
    """
    # Load all images and sort by height descending for better packing
    images = {}
    for key, path in sprites.items():
        img = Image.open(path).convert("RGBA")
        images[key] = img

    sorted_keys = sorted(images.keys(), key=lambda k: (-images[k].height, -images[k].width))

    atlases = []
    remaining = list(sorted_keys)

    while remaining:
        rects = {}
        cur_x = 0
        cur_y = 0
        row_height = 0
        atlas_width = 0
        atlas_height = 0
        placed = []

        for key in remaining:
            img = images[key]
            w, h = img.size

            if w > MAX_ATLAS_SIZE or h > MAX_ATLAS_SIZE:
                print(f"  WARNING: sprite {key} is {w}x{h}, exceeds max atlas size, skipping")
                placed.append(key)
                continue

            # Try to place in current row
            if cur_x + w <= MAX_ATLAS_SIZE:
                if cur_y + h > MAX_ATLAS_SIZE:
                    # Doesn't fit vertically, skip to next atlas
                    continue
                rects[key] = {"x": cur_x, "y": cur_y, "w": w, "h": h}
                row_height = max(row_height, h)
                cur_x += w
                atlas_width = max(atlas_width, cur_x)
                atlas_height = max(atlas_height, cur_y + row_height)
                placed.append(key)
            else:
                # Start new row
                cur_x = 0
                cur_y += row_height
                row_height = 0

                if cur_y + h > MAX_ATLAS_SIZE:
                    # Atlas full
                    continue

                rects[key] = {"x": cur_x, "y": cur_y, "w": w, "h": h}
                row_height = max(row_height, h)
                cur_x += w
                atlas_width = max(atlas_width, cur_x)
                atlas_height = max(atlas_height, cur_y + row_height)
                placed.append(key)

        if not placed:
            print(f"  ERROR: Could not place any sprites, {len(remaining)} remaining")
            break

        # Create the atlas image
        atlas_img = Image.new("RGBA", (atlas_width, atlas_height), (0, 0, 0, 0))
        for key, rect in rects.items():
            atlas_img.paste(images[key], (rect["x"], rect["y"]))

        atlases.append((atlas_img, rects))
        remaining = [k for k in remaining if k not in placed]

    return atlases


def pack_category(category: str, recursive: bool = False, key_transform=None):
    """Pack a single category and return atlas info for manifest.

    If recursive=True, recursively scan subdirectories and preserve path keys.
    key_transform: optional function to transform sprite keys.
    """
    sprite_dir = SPRITES_DIR / category
    if not sprite_dir.exists():
        print(f"  Skipping {category}: directory not found")
        return None

    sprites = collect_sprites(sprite_dir, recursive=recursive, base_dir=sprite_dir, key_transform=key_transform)
    if not sprites:
        print(f"  Skipping {category}: no sprites found")
        return None

    print(f"  Packing {len(sprites)} {category} sprites...")
    atlases = pack_atlas(sprites)

    if len(atlases) == 1:
        atlas_img, rects = atlases[0]
        atlas_filename = f"{category}_atlas.png"
        atlas_path = SPRITES_DIR / atlas_filename
        atlas_img.save(atlas_path, optimize=True)
        print(f"  -> {atlas_filename}: {atlas_img.width}x{atlas_img.height}, {len(rects)} sprites")
        return {
            "file": f"sprites/{atlas_filename}",
            "sprites": rects,
        }
    else:
        # Multiple atlases needed
        all_sprites = {}
        files = []
        for i, (atlas_img, rects) in enumerate(atlases):
            atlas_filename = f"{category}_atlas_{i}.png"
            atlas_path = SPRITES_DIR / atlas_filename
            atlas_img.save(atlas_path, optimize=True)
            print(f"  -> {atlas_filename}: {atlas_img.width}x{atlas_img.height}, {len(rects)} sprites")
            # Tag each sprite with its atlas index
            for key, rect in rects.items():
                rect["atlas"] = i
                all_sprites[key] = rect
            files.append(f"sprites/{atlas_filename}")
        return {
            "files": files,
            "sprites": all_sprites,
        }


def pack_ui_category(category: str, key_transform=None):
    """Pack a UI category (prayers, spells) and return atlas info for manifest."""
    ui_subdir = UI_DIR / category
    if not ui_subdir.exists():
        print(f"  Skipping ui/{category}: directory not found")
        return None

    sprites = collect_sprites(ui_subdir, recursive=False, key_transform=key_transform)
    if not sprites:
        print(f"  Skipping ui/{category}: no sprites found")
        return None

    print(f"  Packing {len(sprites)} ui/{category} icons...")
    atlases = pack_atlas(sprites)

    if len(atlases) == 1:
        atlas_img, rects = atlases[0]
        atlas_filename = f"{category}_atlas.png"
        atlas_path = UI_DIR / atlas_filename
        atlas_img.save(atlas_path, optimize=True)
        print(f"  -> ui/{atlas_filename}: {atlas_img.width}x{atlas_img.height}, {len(rects)} icons")
        return {
            "file": f"ui/{atlas_filename}",
            "sprites": rects,
        }
    else:
        # Multiple atlases needed (unlikely for small UI icons)
        all_sprites = {}
        files = []
        for i, (atlas_img, rects) in enumerate(atlases):
            atlas_filename = f"{category}_atlas_{i}.png"
            atlas_path = UI_DIR / atlas_filename
            atlas_img.save(atlas_path, optimize=True)
            print(f"  -> ui/{atlas_filename}: {atlas_img.width}x{atlas_img.height}, {len(rects)} icons")
            for key, rect in rects.items():
                rect["atlas"] = i
                all_sprites[key] = rect
            files.append(f"ui/{atlas_filename}")
        return {
            "files": files,
            "sprites": all_sprites,
        }


def pack_ui_misc_icons():
    """Pack miscellaneous UI icons (arrows, small icons) into a single atlas."""
    # List of individual UI icons to combine
    icon_files = [
        "quest_complete.png",
        "gold_nugget.png",
        "circular_stone.png",
        "chat_small.png",
        "fishing_skill.png",
        "coin_small.png",
        "trout.png",
        "attack_button.png",
        "up_arrow.png",
        "down_arrow.png",
        "left_arrow.png",
        "right_arrow.png",
    ]

    sprites = {}
    for filename in icon_files:
        path = UI_DIR / filename
        if path.exists():
            key = path.stem  # filename without extension
            sprites[key] = path

    if not sprites:
        print("  Skipping ui_misc: no icons found")
        return None

    print(f"  Packing {len(sprites)} miscellaneous UI icons...")
    atlases = pack_atlas(sprites)

    if len(atlases) == 1:
        atlas_img, rects = atlases[0]
        atlas_filename = "ui_misc_atlas.png"
        atlas_path = UI_DIR / atlas_filename
        atlas_img.save(atlas_path, optimize=True)
        print(f"  -> ui/{atlas_filename}: {atlas_img.width}x{atlas_img.height}, {len(rects)} icons")
        return {
            "file": f"ui/{atlas_filename}",
            "sprites": rects,
        }
    else:
        # Multiple atlases (unlikely)
        all_sprites = {}
        files = []
        for i, (atlas_img, rects) in enumerate(atlases):
            atlas_filename = f"ui_misc_atlas_{i}.png"
            atlas_path = UI_DIR / atlas_filename
            atlas_img.save(atlas_path, optimize=True)
            print(f"  -> ui/{atlas_filename}: {atlas_img.width}x{atlas_img.height}, {len(rects)} icons")
            for key, rect in rects.items():
                rect["atlas"] = i
                all_sprites[key] = rect
            files.append(f"ui/{atlas_filename}")
        return {
            "files": files,
            "sprites": all_sprites,
        }


def load_animated_sprites():
    """Load animated sprite metadata if available."""
    if ANIMATED_SPRITES_PATH.exists():
        with open(ANIMATED_SPRITES_PATH) as f:
            data = json.load(f)
        total = sum(len(v) for v in data.values())
        print(f"Loaded animated_sprites.json ({total} animated sprites)")
        return data
    print("No animated_sprites.json found, skipping animation metadata")
    return {}


def apply_animation_metadata(atlas_info, category, animated_sprites):
    """Add 'frames' field to animated sprite entries in atlas info."""
    if not atlas_info or category not in animated_sprites:
        return
    anim_data = animated_sprites[category]
    count = 0
    for sprite_id, rect in atlas_info["sprites"].items():
        if sprite_id in anim_data:
            rect["frames"] = anim_data[sprite_id]["frames"]
            count += 1
    if count > 0:
        print(f"  Tagged {count} animated {category} sprites with frame counts")


def main():
    print("Sprite Atlas Packer")
    print("=" * 40)

    # Load animated sprite metadata
    animated_sprites = load_animated_sprites()

    # Load existing manifest
    with open(MANIFEST_PATH) as f:
        manifest = json.load(f)

    # Key transform for player sprites: "player_male_tan" -> "male_tan"
    def player_key_transform(key):
        if key.startswith("player_"):
            return key[7:]  # Remove "player_" prefix
        return key

    # Key transform for hair sprites: "hair_0" -> "male_0", "hair_female_0" -> "female_0"
    def hair_key_transform(key):
        if key.startswith("hair_female_"):
            return "female_" + key[12:]
        elif key.startswith("hair_"):
            return "male_" + key[5:]
        return key

    # Key transform for farming sprites: "farming_potato" -> "potato"
    def farming_key_transform(key):
        if key.startswith("farming_"):
            return key[8:]
        return key

    # Key transform for equipment sprites: "back/quiver" -> "quiver", "body/hero_armor" -> "hero_armor"
    def equipment_key_transform(key):
        # Strip subdirectory prefix, keep just the filename
        if "/" in key:
            return key.split("/")[-1]
        return key

    # Categories to pack: (category_name, recursive, key_transform)
    categories_to_pack = [
        ("objects", False, None),
        ("walls", False, None),
        ("inventory", False, None),
        ("players", False, player_key_transform),
        ("hair", False, hair_key_transform),
        ("equipment", True, equipment_key_transform),  # Equipment has subdirectories (back, body, feet, head)
        ("weapons", False, None),
        ("enemies", False, None),
        ("farming", False, farming_key_transform),
        ("effects", False, None),
    ]

    for category, recursive, key_transform in categories_to_pack:
        atlas_info = pack_category(category, recursive=recursive, key_transform=key_transform)
        if atlas_info:
            apply_animation_metadata(atlas_info, category, animated_sprites)
            manifest[f"{category}_atlas"] = atlas_info

    # Pack UI categories (prayers, spells)
    print()
    print("Packing UI icons...")
    ui_categories = [
        ("prayers", None),
        ("spells", None),
    ]
    for category, key_transform in ui_categories:
        atlas_info = pack_ui_category(category, key_transform=key_transform)
        if atlas_info:
            manifest[f"{category}_atlas"] = atlas_info

    # Pack miscellaneous UI icons
    atlas_info = pack_ui_misc_icons()
    if atlas_info:
        manifest["ui_misc_atlas"] = atlas_info

    # Write updated manifest
    with open(MANIFEST_PATH, "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")

    print()
    print("Done! Updated sprite_manifest.json with atlas info.")


if __name__ == "__main__":
    main()
