# Isometric Launcher

Pure Rust desktop launcher that updates and launches the native client.

## Local build

```bash
cd launcher
cargo run --release
```

## Config

By default the launcher looks for `launcher-config.toml` next to the executable or in the current working directory. You can also set `LAUNCHER_BASE_URL` to override the base URL.

## Manifest format

```json
{
  "version": "2.17.1",
  "platforms": {
    "macos-arm64": {
      "entrypoint": "isometric-client",
      "files": [
        {
          "path": "isometric-client",
          "sha256": "...",
          "size": 123456,
          "executable": true
        },
        {
          "path": "assets/sprites.png",
          "sha256": "...",
          "size": 98765
        }
      ]
    }
  }
}
```

If a file entry has no `url`, the launcher downloads from:

```
{base_url}/{platform_key}/{path}
```

## Platform keys

- `macos-arm64`
- `macos-x86_64`
- `windows-x86_64`
- `linux-x86_64`

## Logo

Place a PNG at `assets/logo.png` next to the launcher executable (or in the working directory) to render a logo above the progress bar.

## Building a manifest

Use the helper script after preparing a per-platform folder that contains the client binary plus all assets needed at runtime.
You can generate that folder with the packaging helper:

```bash
python3 tools/package_client.py \
  --platform macos-arm64 \
  --client-dir client \
  --out dist/client/macos-arm64 \
  --include-db
```

Then build the manifest:

```bash
python3 tools/launcher_manifest.py \
  --platform macos-arm64 \
  --input /path/to/dist/macos-arm64 \
  --entrypoint isometric-client \
  --version 2.17.1 \
  --out /path/to/dist/manifest.json
```

Run it again with `--merge` for other platforms:

```bash
python3 tools/launcher_manifest.py \
  --platform windows-x86_64 \
  --input /path/to/dist/windows-x86_64 \
  --entrypoint isometric-client.exe \
  --version 2.17.1 \
  --out /path/to/dist/manifest.json \
  --merge
```

## R2 layout (recommended)

Upload your files to R2 in this layout:

```
/isometric/manifest.json
/isometric/macos-arm64/...
/isometric/windows-x86_64/...
/isometric/linux-x86_64/...
```

Then set `base_url` to `https://YOUR_CDN/isometric`.
