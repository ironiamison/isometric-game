#!/usr/bin/env python3
"""
Extract and reconstruct the tiles.png spritesheet (64×32 cells).

Extract: Split tiles.png into individual tile images (tile_0000.png, tile_0001.png, ...)
Reconstruct: Rebuild the spritesheet from a folder of tiles, including any additional
frames placed in the folder beyond the original count.

Usage:
  # Extract tiles.png to client/assets/sprites/tiles_extracted/
  python tools/tiles_sheet.py extract [--input PATH] [--output DIR]

  # Reconstruct spritesheet from folder (default: tiles_extracted -> tiles.png)
  python tools/tiles_sheet.py reconstruct [--input DIR] [--output PATH]

  # Rename {id}.png files to tile_{id}.png, with IDs starting after existing tile_*.png
  python tools/tiles_sheet.py rename [--input DIR] [--dry-run]
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path

from PIL import Image

TILE_WIDTH = 64
TILE_HEIGHT = 32
TILE_PATTERN = re.compile(r"^tile_(\d+)\.png$", re.IGNORECASE)
NUMERIC_ONLY_PATTERN = re.compile(r"^(\d+)\.png$", re.IGNORECASE)


def extract_tiles(input_path: Path, output_dir: Path) -> int:
    """Extract spritesheet into individual tile PNGs. Returns number of tiles extracted."""
    img = Image.open(input_path).convert("RGBA")
    w, h = img.size
    cols = w // TILE_WIDTH
    rows = h // TILE_HEIGHT

    output_dir.mkdir(parents=True, exist_ok=True)
    count = 0

    for row in range(rows):
        for col in range(cols):
            left = col * TILE_WIDTH
            top = row * TILE_HEIGHT
            box = (left, top, left + TILE_WIDTH, top + TILE_HEIGHT)
            tile = img.crop(box)
            idx = row * cols + col
            out_path = output_dir / f"tile_{idx:04d}.png"
            tile.save(out_path, format="PNG")
            count += 1

    print(f"Extracted {count} tiles ({cols}×{rows}) to {output_dir}")
    return count


def collect_tiles(folder: Path) -> list[tuple[int, Path]]:
    """Collect all tile_*.png files, return [(index, path), ...] sorted by index."""
    tiles: list[tuple[int, int, Path]] = []
    for p in folder.glob("tile_*.png"):
        m = TILE_PATTERN.match(p.name)
        if m:
            idx = int(m.group(1))
            tiles.append((idx, idx, p))
    tiles.sort(key=lambda x: x[0])
    return [(idx, p) for idx, _, p in tiles]


def reconstruct_sheet(input_dir: Path, output_path: Path, cols: int | None = None) -> None:
    """Reconstruct spritesheet from folder of tile PNGs. Uses original column count if known."""
    tiles = collect_tiles(input_dir)
    if not tiles:
        raise SystemExit(f"No tile_*.png files found in {input_dir}")

    # Infer columns from first tile dimensions (all must be 64×32)
    with Image.open(tiles[0][1]) as sample:
        tw, th = sample.size
        if tw != TILE_WIDTH or th != TILE_HEIGHT:
            raise SystemExit(
                f"Tile size mismatch: expected {TILE_WIDTH}×{TILE_HEIGHT}, got {tw}×{th}"
            )

    num_tiles = len(tiles)
    if cols is None:
        cols = min(32, num_tiles)  # default 32 (mapper tileset columns)
    rows = (num_tiles + cols - 1) // cols

    sheet_w = cols * TILE_WIDTH
    sheet_h = rows * TILE_HEIGHT
    sheet = Image.new("RGBA", (sheet_w, sheet_h), (0, 0, 0, 0))

    for idx, (_, path) in enumerate(tiles):
        row = idx // cols
        col = idx % cols
        x = col * TILE_WIDTH
        y = row * TILE_HEIGHT
        with Image.open(path) as tile:
            tile = tile.convert("RGBA")
            sheet.paste(tile, (x, y))

    output_path.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(output_path)
    print(f"Reconstructed {output_path} with {num_tiles} tiles ({cols}×{rows})")


def rename_numeric_tiles(folder: Path, dry_run: bool = False) -> int:
    """
    Rename {id}.png files to tile_{next}.png where next starts after the
    highest existing tile_*.png index. Sorts by current numeric id.
    Returns number of files renamed.
    """
    # Find max index among tile_*.png
    max_idx = -1
    for p in folder.glob("tile_*.png"):
        m = TILE_PATTERN.match(p.name)
        if m:
            max_idx = max(max_idx, int(m.group(1)))

    next_idx = max_idx + 1

    # Collect numeric-only PNGs (e.g. 1.png, 42.png), sorted by current id
    to_rename: list[tuple[int, Path]] = []
    for p in folder.glob("*.png"):
        m = NUMERIC_ONLY_PATTERN.match(p.name)
        if m:
            to_rename.append((int(m.group(1)), p))
    to_rename.sort(key=lambda x: x[0])

    if not to_rename:
        print(f"No {{id}}.png files to rename in {folder}")
        return 0

    last_idx = next_idx + len(to_rename) - 1
    print(f"tile_*.png max index: {max_idx}, renaming {len(to_rename)} files -> tile_{next_idx:04d}.png .. tile_{last_idx:04d}.png")
    for current_id, path in to_rename:
        new_name = f"tile_{next_idx:04d}.png"
        new_path = path.parent / new_name
        if dry_run:
            print(f"  [dry-run] {path.name} -> {new_name}")
        else:
            path.rename(new_path)
            print(f"  {path.name} -> {new_name}")
        next_idx += 1

    return len(to_rename)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Extract or reconstruct the tiles.png spritesheet (64×32 cells)",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    # Extract
    ext = sub.add_parser("extract", help="Extract tiles.png into individual tile images")
    ext.add_argument(
        "--input",
        "-i",
        type=Path,
        default=Path("client/assets/sprites/tiles.png"),
        help="Input spritesheet path",
    )
    ext.add_argument(
        "--output",
        "-o",
        type=Path,
        default=Path("client/assets/sprites/tiles_extracted"),
        help="Output directory for extracted tiles",
    )

    # Reconstruct
    rec = sub.add_parser("reconstruct", help="Reconstruct spritesheet from folder of tiles")
    rec.add_argument(
        "--input",
        "-i",
        type=Path,
        default=Path("client/assets/sprites/tiles_extracted"),
        help="Input directory with tile_*.png files",
    )
    rec.add_argument(
        "--output",
        "-o",
        type=Path,
        default=Path("client/assets/sprites/tiles.png"),
        help="Output spritesheet path",
    )
    rec.add_argument(
        "--columns",
        "-c",
        type=int,
        default=32,
        help="Number of columns (default: 32, matches mapper tileset)",
    )

    # Rename
    rn = sub.add_parser(
        "rename",
        help="Rename {id}.png files to tile_{id}.png, IDs start after existing tile_*.png",
    )
    rn.add_argument(
        "--input",
        "-i",
        type=Path,
        default=Path("client/assets/sprites/tiles_extracted"),
        help="Directory containing tiles",
    )
    rn.add_argument("--dry-run", action="store_true", help="Print renames without doing them")

    args = parser.parse_args()

    if args.command == "extract":
        if not args.input.exists():
            raise SystemExit(f"Input not found: {args.input}")
        extract_tiles(args.input.resolve(), args.output.resolve())

    elif args.command == "reconstruct":
        if not args.input.is_dir():
            raise SystemExit(f"Input directory not found: {args.input}")
        reconstruct_sheet(args.input.resolve(), args.output.resolve(), cols=args.columns)

    elif args.command == "rename":
        if not args.input.is_dir():
            raise SystemExit(f"Input directory not found: {args.input}")
        rename_numeric_tiles(args.input.resolve(), dry_run=args.dry_run)


if __name__ == "__main__":
    main()
