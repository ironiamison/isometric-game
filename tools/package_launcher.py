#!/usr/bin/env python3
import argparse
import os
import shutil
import tarfile
import tempfile
import zipfile
from pathlib import Path


def copy_file(src: Path, dst: Path) -> None:
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dst)


def add_dir_to_zip(zipf: zipfile.ZipFile, base_dir: Path) -> None:
    for root, dirs, files in os.walk(base_dir):
        root_path = Path(root)
        rel_root = root_path.relative_to(base_dir.parent)
        if not files and not dirs:
            info = zipfile.ZipInfo(str(rel_root) + "/")
            info.external_attr = (0o755 & 0xFFFF) << 16
            zipf.writestr(info, "")
        for name in files:
            file_path = root_path / name
            rel_path = file_path.relative_to(base_dir.parent)
            info = zipfile.ZipInfo(str(rel_path))
            mode = file_path.stat().st_mode
            info.external_attr = (mode & 0xFFFF) << 16
            with open(file_path, "rb") as f:
                zipf.writestr(info, f.read(), compress_type=zipfile.ZIP_DEFLATED)


def build_zip(src_dir: Path, out_path: Path) -> None:
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(out_path, "w") as zipf:
        add_dir_to_zip(zipf, src_dir)


def build_tar_gz(src_dir: Path, out_path: Path) -> None:
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(out_path, "w:gz") as tar:
        tar.add(src_dir, arcname=src_dir.name)


def main() -> None:
    parser = argparse.ArgumentParser(description="Package the launcher for distribution.")
    parser.add_argument("--platform", required=True, help="Platform label, e.g. win64")
    parser.add_argument("--bin", required=True, help="Path to launcher binary")
    parser.add_argument("--config", required=True, help="Path to launcher-config.toml")
    parser.add_argument("--logo", required=True, help="Path to logo.png")
    parser.add_argument("--format", choices=["zip", "tar.gz"], default="zip")
    parser.add_argument("--out", required=True, help="Output archive path")
    args = parser.parse_args()

    bin_path = Path(args.bin).resolve()
    config_path = Path(args.config).resolve()
    logo_path = Path(args.logo).resolve()
    app_icon_path = logo_path.parent / "app-icon.png"
    out_path = Path(args.out).resolve()

    if not bin_path.exists():
        raise SystemExit(f"Binary not found: {bin_path}")
    if not config_path.exists():
        raise SystemExit(f"Config not found: {config_path}")
    if not logo_path.exists():
        raise SystemExit(f"Logo not found: {logo_path}")

    with tempfile.TemporaryDirectory() as tmp:
        staging_root = Path(tmp) / "new-aeven-launcher"
        copy_file(bin_path, staging_root / bin_path.name)
        copy_file(config_path, staging_root / "launcher-config.toml")
        copy_file(logo_path, staging_root / "assets" / "logo.png")
        if app_icon_path.exists():
            copy_file(app_icon_path, staging_root / "assets" / "app-icon.png")

        if args.format == "zip":
            build_zip(staging_root, out_path)
        else:
            build_tar_gz(staging_root, out_path)

    print(f"Packaged launcher: {out_path}")


if __name__ == "__main__":
    main()
