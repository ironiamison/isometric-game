#!/usr/bin/env python3
import argparse
import os
import shutil
from pathlib import Path


def copy_tree(src: Path, dst: Path) -> None:
    if dst.exists():
        shutil.rmtree(dst)
    shutil.copytree(src, dst)


def copy_file(src: Path, dst: Path) -> None:
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dst)


def default_binary_name() -> str:
    return "new-aeven.exe" if os.name == "nt" else "new-aeven"


def main() -> None:
    parser = argparse.ArgumentParser(description="Package the native client for the launcher.")
    parser.add_argument("--platform", required=True, help="Platform key, e.g. macos-arm64")
    parser.add_argument("--client-dir", default="client", help="Client project directory")
    parser.add_argument("--out", default=None, help="Output directory (default: dist/client/<platform>)")
    parser.add_argument("--bin", default=None, help="Path to client binary")
    parser.add_argument("--assets", default="assets", help="Assets directory inside client dir")
    parser.add_argument("--include-db", action="store_true", help="Include game.db if present")
    parser.add_argument(
        "--atlas-only",
        action="store_true",
        help="Remove per-sprite directories for atlas-backed categories (smaller downloads).",
    )
    args = parser.parse_args()

    client_dir = Path(args.client_dir).resolve()
    if not client_dir.exists():
        raise SystemExit(f"Client dir not found: {client_dir}")

    out_dir = Path(args.out) if args.out else Path("dist") / "client" / args.platform
    out_dir = out_dir.resolve()

    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    binary_path = Path(args.bin) if args.bin else client_dir / "target" / "release" / default_binary_name()
    binary_path = binary_path.resolve()
    if not binary_path.exists():
        raise SystemExit(f"Binary not found: {binary_path}")

    copy_file(binary_path, out_dir / binary_path.name)

    assets_dir = client_dir / args.assets
    if assets_dir.exists():
        copy_tree(assets_dir, out_dir / "assets")
    else:
        print(f"Warning: assets directory not found: {assets_dir}")

    if args.atlas_only:
        sprites_root = out_dir / "assets" / "sprites"
        # Remove tiles_extracted — client only uses the compiled tiles.png
        tiles_extracted = sprites_root / "tiles_extracted"
        if tiles_extracted.exists():
            shutil.rmtree(tiles_extracted)
        atlas_dirs = [
            "players",
            "hair",
            "equipment",
            "weapons",
            "inventory",
            "objects",
            "walls",
            "enemies",
            "farming",
            "effects",
        ]
        missing_atlases = []
        for name in atlas_dirs:
            atlas_png = sprites_root / f"{name}_atlas.png"
            if not atlas_png.exists():
                missing_atlases.append(atlas_png)
        if missing_atlases:
            raise SystemExit(
                "Atlas-only packaging requested, but missing atlases:\n"
                + "\n".join(str(p) for p in missing_atlases)
            )
        # Sprites too large for the atlas that must survive atlas-only packaging
        overflow_sprites = {
            "enemies": ["big_wurm.png"],
        }
        for name in atlas_dirs:
            dir_path = sprites_root / name
            if not dir_path.exists():
                continue
            keep = overflow_sprites.get(name, [])
            if keep:
                # Preserve overflow sprites, delete everything else
                kept = {}
                for fname in keep:
                    src = dir_path / fname
                    if src.exists():
                        kept[fname] = src.read_bytes()
                shutil.rmtree(dir_path)
                if kept:
                    dir_path.mkdir(parents=True)
                    for fname, data in kept.items():
                        (dir_path / fname).write_bytes(data)
            else:
                shutil.rmtree(dir_path)

    if args.include_db:
        db_path = client_dir / "game.db"
        if db_path.exists():
            copy_file(db_path, out_dir / db_path.name)
        else:
            print(f"Note: game.db not found at {db_path}, skipping.")

    print(f"Packaged client for {args.platform} in {out_dir}")


if __name__ == "__main__":
    main()
