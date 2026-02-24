#!/usr/bin/env bash
set -euo pipefail

# Tauri's generated bundle_dmg.sh can fail if a stale volume is already mounted.
# Detach the exact mountpoint first so DMG bundling starts from a clean state.
if [[ "$(uname -s)" != "Darwin" ]]; then
  exit 0
fi

VOLUME_PATH="/Volumes/OpenJar Launcher"
if [[ ! -d "$VOLUME_PATH" ]]; then
  exit 0
fi

echo "[prebuild-dmg-cleanup] Detaching stale DMG mount at: $VOLUME_PATH"
hdiutil detach "$VOLUME_PATH" >/dev/null 2>&1 || hdiutil detach -force "$VOLUME_PATH" >/dev/null 2>&1 || true
