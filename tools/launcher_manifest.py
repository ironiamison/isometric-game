#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
import pathlib


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1024 * 64), b""):
            h.update(chunk)
    return h.hexdigest()


def gather_files(root):
    files = []
    for dirpath, _, filenames in os.walk(root):
        for name in filenames:
            full = pathlib.Path(dirpath) / name
            rel = full.relative_to(root).as_posix()
            size = full.stat().st_size
            digest = sha256_file(full)
            executable = os.access(full, os.X_OK)
            files.append({
                "path": rel,
                "sha256": digest,
                "size": size,
                "executable": executable if executable else None,
            })
    return files


def build_platform_block(root, entrypoint):
    files = gather_files(root)
    for entry in files:
        if entry["path"] == entrypoint:
            entry["executable"] = True
    for entry in files:
        if entry["executable"] is None:
            del entry["executable"]
    return {
        "entrypoint": entrypoint,
        "files": files,
    }


def main():
    parser = argparse.ArgumentParser(description="Build launcher manifest JSON.")
    parser.add_argument("--platform", required=True, help="Platform key, e.g. macos-arm64")
    parser.add_argument("--input", required=True, help="Input directory for that platform")
    parser.add_argument("--entrypoint", required=True, help="Entry point path relative to input")
    parser.add_argument("--version", required=True, help="Release version")
    parser.add_argument("--out", required=True, help="Output manifest path")
    parser.add_argument("--merge", action="store_true", help="Merge into existing manifest")
    args = parser.parse_args()

    input_dir = pathlib.Path(args.input).resolve()
    if not input_dir.exists():
        raise SystemExit(f"Input directory not found: {input_dir}")

    platform_block = build_platform_block(input_dir, args.entrypoint)

    if args.merge and pathlib.Path(args.out).exists():
        with open(args.out, "r", encoding="utf-8") as f:
            manifest = json.load(f)
    else:
        manifest = {"version": args.version, "platforms": {}}

    manifest["version"] = args.version
    manifest.setdefault("platforms", {})
    manifest["platforms"][args.platform] = platform_block

    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2, sort_keys=True)

    print(f"Wrote {args.out}")


if __name__ == "__main__":
    main()
