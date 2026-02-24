#!/usr/bin/env bash
set -euo pipefail

# Tauri's generated bundle_dmg.sh can fail if a stale volume is already mounted.
# Detach the exact mountpoint first so DMG bundling starts from a clean state.
if [[ "$(uname -s)" != "Darwin" ]]; then
  exit 0
fi

VOLUME_PATH="/Volumes/OpenJar Launcher"
if [[ ! -d "$VOLUME_PATH" ]]; then
  :
fi

if [[ -d "$VOLUME_PATH" ]]; then
  echo "[prebuild-dmg-cleanup] Detaching stale DMG mount at: $VOLUME_PATH"
  hdiutil detach "$VOLUME_PATH" >/dev/null 2>&1 || hdiutil detach -force "$VOLUME_PATH" >/dev/null 2>&1 || true
fi

# Remove stale DMG artifacts left by interrupted bundling.
# If an old rw.*.dmg sits under bundle/macos, create-dmg may recursively pack it,
# causing runaway image size growth and "No space left on device".
for bundle_root in \
  "src-tauri/target/release/bundle" \
  "src-tauri/target/aarch64-apple-darwin/release/bundle" \
  "src-tauri/target/x86_64-apple-darwin/release/bundle"
do
  [[ -d "$bundle_root" ]] || continue
  for dir in "$bundle_root/macos" "$bundle_root/dmg"; do
    [[ -d "$dir" ]] || continue
    while IFS= read -r stale; do
      [[ -n "$stale" ]] || continue
      echo "[prebuild-dmg-cleanup] Removing stale DMG artifact: $stale"
      rm -f "$stale" || true
    done < <(find "$dir" -maxdepth 1 -type f \( -name "*.dmg" -o -name "rw.*.dmg" \) 2>/dev/null)
  done
done
