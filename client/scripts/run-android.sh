#!/usr/bin/env bash
set -euo pipefail

# Run the game on an Android emulator
# Usage: ./scripts/run-android.sh [--no-build]

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CLIENT_DIR="$(dirname "$SCRIPT_DIR")"
ANDROID_DIR="$CLIENT_DIR/android"
APK="$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk"
PACKAGE="com.newaeven.game"
ACTIVITY="com.newaeven.game.MainActivity"
AVD="Pixel_7"

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
EMU="$ANDROID_HOME/emulator/emulator"
ADB="$ANDROID_HOME/platform-tools/adb"

# --- Build unless --no-build ---
if [[ "${1:-}" != "--no-build" ]]; then
    echo "==> Generating sprite manifest..."
    "$SCRIPT_DIR/generate_sprite_manifest.sh"

    echo "==> Building Android APK..."
    cd "$ANDROID_DIR" && ./gradlew assembleDebug
    cd "$CLIENT_DIR"
fi

# --- Start emulator if not already running ---
if ! "$ADB" devices 2>/dev/null | grep -q "emulator.*device$"; then
    echo "==> Starting emulator ($AVD)..."
    "$EMU" -avd "$AVD" -no-snapshot-load &
    EMU_PID=$!

    echo "==> Waiting for device to boot..."
    "$ADB" wait-for-device
    # Wait for boot animation to finish
    while [[ "$("$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" != "1" ]]; do
        sleep 1
    done
    echo "==> Emulator booted."
else
    echo "==> Emulator already running."
fi

# --- Install and launch ---
echo "==> Installing APK..."
"$ADB" install -r "$APK"

echo "==> Launching app..."
"$ADB" shell am start -n "$PACKAGE/$ACTIVITY"

echo "==> Done! Tailing logcat (Ctrl+C to stop)..."
"$ADB" logcat -s "RustStdoutStderr:V" "newaeven:V" "macroquad:V"
