#!/usr/bin/env python3
"""
Detect animated sprites by analyzing object/wall PNGs for multi-frame layouts.

Animated sprites contain 2-4 identical-silhouette frames laid out horizontally.
This script detects them by:
1. Checking if width is evenly divisible by candidate frame counts (4, 3, 2)
2. Comparing alpha-channel similarity between frames (>0.85 = same silhouette)
3. Verifying frames aren't identical (pixel difference > threshold = actual animation)

Output: client/assets/animated_sprites.json

Usage: python3 tools/detect_animated_sprites.py
"""

import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

ASSETS_DIR = Path(__file__).parent.parent / "client" / "assets"
SPRITES_DIR = ASSETS_DIR / "sprites"
OUTPUT_PATH = ASSETS_DIR / "animated_sprites.json"

# Alpha similarity threshold: frames must share >85% of their silhouette
ALPHA_SIMILARITY_THRESHOLD = 0.85
# Minimum pixel difference between frames to confirm actual animation (not just duplicated)
MIN_CONTENT_DIFFERENCE = 0.005
# Only check these candidate frame counts (prefer higher counts first)
CANDIDATE_FRAME_COUNTS = [4, 3, 2]


def compute_alpha_similarity(frame_a: np.ndarray, frame_b: np.ndarray) -> float:
    """Compare alpha channels of two frames. Returns ratio of matching pixels."""
    alpha_a = (frame_a[:, :, 3] > 0).astype(np.uint8)
    alpha_b = (frame_b[:, :, 3] > 0).astype(np.uint8)
    total = alpha_a.size
    if total == 0:
        return 0.0
    matching = np.sum(alpha_a == alpha_b)
    return matching / total


def compute_content_difference(frame_a: np.ndarray, frame_b: np.ndarray) -> float:
    """Compute normalized pixel difference between two RGBA frames."""
    diff = np.abs(frame_a.astype(np.int16) - frame_b.astype(np.int16))
    # Only count differences where at least one pixel is non-transparent
    mask = (frame_a[:, :, 3] > 0) | (frame_b[:, :, 3] > 0)
    if not np.any(mask):
        return 0.0
    # Mean absolute difference across RGB channels in visible pixels
    rgb_diff = diff[:, :, :3]
    visible_diff = rgb_diff[mask]
    return np.mean(visible_diff) / 255.0


def detect_animation(img_path: Path) -> tuple[int, float] | None:
    """
    Analyze a sprite PNG for multi-frame animation.
    Returns (frame_count, fps) if animated, None otherwise.
    """
    img = Image.open(img_path).convert("RGBA")
    w, h = img.size

    # Skip tiny sprites (unlikely to be multi-frame)
    if w < 32 or h < 16:
        return None

    pixels = np.array(img)

    for frame_count in CANDIDATE_FRAME_COUNTS:
        if w % frame_count != 0:
            continue

        frame_w = w // frame_count
        # Skip if individual frames would be too narrow - animated frames
        # should be at least 32px wide to avoid false positives on small tiles
        if frame_w < 32:
            continue

        # Split into frames
        frames = [pixels[:, i * frame_w : (i + 1) * frame_w] for i in range(frame_count)]

        # Check alpha similarity between all adjacent pairs
        all_similar = True
        for i in range(frame_count - 1):
            sim = compute_alpha_similarity(frames[i], frames[i + 1])
            if sim < ALPHA_SIMILARITY_THRESHOLD:
                all_similar = False
                break

        if not all_similar:
            continue

        # Check that frames aren't all identical (i.e., there's actual animation)
        has_difference = False
        for i in range(frame_count - 1):
            diff = compute_content_difference(frames[i], frames[i + 1])
            if diff > MIN_CONTENT_DIFFERENCE:
                has_difference = True
                break

        if has_difference:
            return (frame_count, 4.0)  # Default 4 FPS for ambient animations

    return None


def scan_directory(sprite_dir: Path) -> dict:
    """Scan a directory of numbered PNGs for animated sprites."""
    results = {}
    png_files = sorted(sprite_dir.glob("*.png"))
    total = len(png_files)

    for i, png_path in enumerate(png_files):
        sprite_id = png_path.stem
        if (i + 1) % 200 == 0 or i == total - 1:
            print(f"  Scanned {i + 1}/{total}...")

        result = detect_animation(png_path)
        if result is not None:
            frame_count, fps = result
            results[sprite_id] = {"frames": frame_count, "fps": fps}

    return results


def main():
    print("Animated Sprite Detector")
    print("=" * 40)

    output = {"objects": {}, "walls": {}}

    objects_dir = SPRITES_DIR / "objects"
    walls_dir = SPRITES_DIR / "walls"

    if objects_dir.exists():
        print(f"\nScanning objects ({len(list(objects_dir.glob('*.png')))} sprites)...")
        output["objects"] = scan_directory(objects_dir)
        print(f"  Found {len(output['objects'])} animated object sprites")
    else:
        print(f"  Objects directory not found: {objects_dir}")

    if walls_dir.exists():
        print(f"\nScanning walls ({len(list(walls_dir.glob('*.png')))} sprites)...")
        output["walls"] = scan_directory(walls_dir)
        print(f"  Found {len(output['walls'])} animated wall sprites")
    else:
        print(f"  Walls directory not found: {walls_dir}")

    # Write output
    with open(OUTPUT_PATH, "w") as f:
        json.dump(output, f, indent=2, sort_keys=True)
        f.write("\n")

    total = len(output["objects"]) + len(output["walls"])
    print(f"\nDone! Found {total} animated sprites total.")
    print(f"Output written to: {OUTPUT_PATH}")
    print("\nReview the output and adjust fps values or remove false positives as needed.")


if __name__ == "__main__":
    main()
