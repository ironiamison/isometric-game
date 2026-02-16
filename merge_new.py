#!/usr/bin/env python3
"""
Merge *_new PNG folders into their target folders with sequential numbering.

Finds the highest number already used in the target folder, then renames and
moves all PNGs from the source (_new) folder using the next sequential numbers.
Handles overlapping names by reassigning every source file to a new number.

Usage:
  python merge_new.py [--dry-run] [--base-dir PATH]

Default base dir: client/assets/sprites
Merge pairs: objects_new -> objects, walls_new -> walls
"""

import argparse
import re
import shutil
from pathlib import Path


def get_max_number(folder: Path) -> int:
    """Return the highest numeric value from PNG filenames (e.g. 1081 from 1081.png)."""
    max_n = 0
    for p in folder.glob("*.png"):
        name = p.stem
        if name.isdigit():
            max_n = max(max_n, int(name))
    return max_n


def get_sorted_png_paths(folder: Path) -> list[Path]:
    """Return paths to all PNGs in folder, sorted by numeric stem (then by name)."""
    paths = list(folder.glob("*.png"))
    def sort_key(p: Path) -> tuple:
        s = p.stem
        return (int(s) if s.isdigit() else -1, p.name)
    paths.sort(key=sort_key)
    return paths


def merge_folder(source: Path, dest: Path, dry_run: bool = False) -> int:
    """
    Move all PNGs from source to dest, renaming to sequential numbers
    starting from (max number in dest) + 1. Returns number of files moved.
    """
    if not source.is_dir():
        print(f"  Skip (source missing): {source}")
        return 0
    if not dest.is_dir():
        print(f"  Skip (dest missing): {dest}")
        return 0

    next_num = get_max_number(dest) + 1
    sources = get_sorted_png_paths(source)
    if not sources:
        print(f"  No PNGs in {source}")
        return 0

    moved = 0
    for p in sources:
        new_name = f"{next_num}.png"
        dest_file = dest / new_name
        if dry_run:
            print(f"  [dry-run] {p.name} -> {dest_file.relative_to(dest.parent)}")
        else:
            shutil.move(str(p), str(dest_file))
        next_num += 1
        moved += 1

    return moved


def main() -> None:
    parser = argparse.ArgumentParser(description="Merge *_new PNG folders into target folders with sequential numbering.")
    parser.add_argument("--dry-run", action="store_true", help="Print what would be done without moving files")
    parser.add_argument("--base-dir", type=Path, default=Path("client/assets/sprites"), help="Base sprites directory")
    args = parser.parse_args()

    base = args.base_dir.resolve()
    if not base.is_dir():
        print(f"Base directory not found: {base}")
        return

    pairs = [
        (base / "objects_new", base / "objects"),
        (base / "walls_new", base / "walls"),
    ]

    for source, dest in pairs:
        label = source.name
        print(f"\n{label} -> {dest.name}")
        n = merge_folder(source, dest, dry_run=args.dry_run)
        print(f"  Moved {n} file(s)")

    print()


if __name__ == "__main__":
    main()
