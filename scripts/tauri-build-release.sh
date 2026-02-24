#!/usr/bin/env bash
set -euo pipefail

bash scripts/prebuild-dmg-cleanup.sh
tauri build --bundles app "$@"

if [[ "$(uname -s)" != "Darwin" ]]; then
  exit 0
fi

APP_PATH="src-tauri/target/release/bundle/macos/OpenJar Launcher.app"
if [[ ! -d "$APP_PATH" ]]; then
  echo "[tauri-build-release] Expected app bundle missing: $APP_PATH" >&2
  exit 1
fi

VERSION="$(node -p "require('./src-tauri/tauri.conf.json').package.version")"
ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
  arm64|aarch64) ARCH_TAG="aarch64" ;;
  x86_64|amd64) ARCH_TAG="x86_64" ;;
  *) ARCH_TAG="$ARCH_RAW" ;;
esac

OUT_DIR="src-tauri/target/release/bundle/dmg"
mkdir -p "$OUT_DIR"
OUT_DMG="$OUT_DIR/OpenJar Launcher_${VERSION}_${ARCH_TAG}.dmg"
TMP_DMG="$OUT_DIR/rw.OpenJar Launcher_${VERSION}_${ARCH_TAG}.dmg"
rm -f "$OUT_DMG" "$TMP_DMG"

echo "[tauri-build-release] Creating DMG: $OUT_DMG"
hdiutil create \
  -volname "OpenJar Launcher" \
  -srcfolder "$APP_PATH" \
  -ov \
  -format UDZO \
  "$OUT_DMG"
echo "[tauri-build-release] DMG ready: $OUT_DMG"
