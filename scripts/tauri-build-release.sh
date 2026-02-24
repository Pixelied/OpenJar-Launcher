#!/usr/bin/env bash
set -euo pipefail

extract_target_arg() {
  local prev=""
  for arg in "$@"; do
    if [[ "$prev" == "--target" ]]; then
      echo "$arg"
      return 0
    fi
    if [[ "$arg" == --target=* ]]; then
      echo "${arg#--target=}"
      return 0
    fi
    prev="$arg"
  done
  return 1
}

pick_bundle_root() {
  local target_arg="${1:-}"
  if [[ -n "$target_arg" && -d "src-tauri/target/${target_arg}/release/bundle" ]]; then
    echo "src-tauri/target/${target_arg}/release/bundle"
    return 0
  fi
  if [[ -d "src-tauri/target/release/bundle" ]]; then
    echo "src-tauri/target/release/bundle"
    return 0
  fi
  local found
  found="$(find src-tauri/target -type d -path '*/release/bundle' 2>/dev/null | head -n 1)"
  if [[ -n "$found" ]]; then
    echo "$found"
    return 0
  fi
  return 1
}

target_arg="$(extract_target_arg "$@" || true)"

if [[ "$(uname -s)" != "Darwin" ]]; then
  case "$(uname -s)" in
    Linux)
      tauri build --bundles appimage,updater "$@"
      ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
      tauri build --bundles nsis,msi,updater "$@"
      ;;
    *)
      tauri build --bundles updater "$@"
      ;;
  esac
  exit 0
fi

bash scripts/prebuild-dmg-cleanup.sh
tauri build --bundles app,updater "$@"

BUNDLE_ROOT="$(pick_bundle_root "$target_arg" || true)"
if [[ -z "${BUNDLE_ROOT:-}" ]]; then
  echo "[tauri-build-release] Could not locate Tauri bundle directory under src-tauri/target." >&2
  exit 1
fi

APP_PATH="$(find "$BUNDLE_ROOT/macos" -maxdepth 1 -type d -name '*.app' 2>/dev/null | head -n 1)"
if [[ -z "${APP_PATH:-}" || ! -d "$APP_PATH" ]]; then
  echo "[tauri-build-release] Expected app bundle missing under: $BUNDLE_ROOT/macos" >&2
  find "$BUNDLE_ROOT" -maxdepth 4 -type d -name "*.app" -print || true
  exit 1
fi

VERSION="$(node -p "require('./src-tauri/tauri.conf.json').package.version")"
ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
  arm64|aarch64) ARCH_TAG="aarch64" ;;
  x86_64|amd64) ARCH_TAG="x86_64" ;;
  *) ARCH_TAG="$ARCH_RAW" ;;
esac

OUT_DIR="$BUNDLE_ROOT/dmg"
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
