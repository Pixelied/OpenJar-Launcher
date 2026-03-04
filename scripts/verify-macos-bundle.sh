#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_APP_PATH="${ROOT_DIR}/src-tauri/target/release/bundle/macos/OpenJar Launcher.app"
APP_PATH="${1:-${DEFAULT_APP_PATH}}"
PLIST_PATH="${APP_PATH}/Contents/Info.plist"
EXPECTED_BUNDLE_ID="io.github.pixelied.openjarlauncher"
EXPECTED_NAME="OpenJar Launcher"
EXPECTED_CATEGORY="public.app-category.games"

if [[ ! -d "${APP_PATH}" ]]; then
  echo "ERROR: app bundle not found at '${APP_PATH}'"
  echo "Build first with: npm run tauri:build"
  exit 1
fi

if [[ ! -f "${PLIST_PATH}" ]]; then
  echo "ERROR: Info.plist not found at '${PLIST_PATH}'"
  exit 1
fi

if ! command -v /usr/libexec/PlistBuddy >/dev/null 2>&1; then
  echo "ERROR: /usr/libexec/PlistBuddy is required on macOS."
  exit 1
fi

read_plist_key() {
  local key="$1"
  /usr/libexec/PlistBuddy -c "Print :${key}" "${PLIST_PATH}" 2>/dev/null || true
}

check_key_equals() {
  local key="$1"
  local expected="$2"
  local actual
  actual="$(read_plist_key "${key}")"
  if [[ "${actual}" != "${expected}" ]]; then
    echo "ERROR: ${key} mismatch: expected '${expected}', got '${actual}'"
    return 1
  fi
  echo "OK: ${key}='${actual}'"
  return 0
}

check_key_non_empty() {
  local key="$1"
  local actual
  actual="$(read_plist_key "${key}")"
  if [[ -z "${actual}" ]]; then
    echo "ERROR: ${key} missing/empty"
    return 1
  fi
  echo "OK: ${key}='${actual}'"
  return 0
}

failures=0
warnings=0

check_key_equals "CFBundleIdentifier" "${EXPECTED_BUNDLE_ID}" || failures=$((failures + 1))
check_key_equals "CFBundleName" "${EXPECTED_NAME}" || failures=$((failures + 1))
check_key_equals "CFBundleDisplayName" "${EXPECTED_NAME}" || failures=$((failures + 1))
check_key_equals "LSApplicationCategoryType" "${EXPECTED_CATEGORY}" || failures=$((failures + 1))
check_key_non_empty "CFBundleShortVersionString" || failures=$((failures + 1))
check_key_non_empty "CFBundleVersion" || failures=$((failures + 1))

if command -v mdls >/dev/null 2>&1; then
  mdls_bundle_id="$(mdls -raw -name kMDItemCFBundleIdentifier "${APP_PATH}" 2>/dev/null || true)"
  mdls_display_name="$(mdls -raw -name kMDItemDisplayName "${APP_PATH}" 2>/dev/null || true)"
  if [[ "${mdls_bundle_id}" == "(null)" || -z "${mdls_bundle_id}" ]]; then
    echo "WARN: mdls could not read kMDItemCFBundleIdentifier for bundle."
    warnings=$((warnings + 1))
  else
    echo "OK: mdls bundle id='${mdls_bundle_id}'"
  fi
  if [[ "${mdls_display_name}" == "(null)" || -z "${mdls_display_name}" ]]; then
    echo "WARN: mdls could not read kMDItemDisplayName for bundle."
    warnings=$((warnings + 1))
  else
    echo "OK: mdls display name='${mdls_display_name}'"
  fi
else
  echo "WARN: mdls not available; skipping Spotlight metadata read."
  warnings=$((warnings + 1))
fi

LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ -x "${LSREGISTER}" ]]; then
  if "${LSREGISTER}" -dump 2>/dev/null | grep -Fq "${APP_PATH}"; then
    echo "OK: LaunchServices dump contains bundle path."
  else
    echo "WARN: LaunchServices dump does not contain bundle path yet."
    echo "      Try: ${LSREGISTER} -f \"${APP_PATH}\""
    warnings=$((warnings + 1))
  fi
else
  echo "WARN: lsregister tool unavailable; skipping LaunchServices check."
  warnings=$((warnings + 1))
fi

if command -v mdfind >/dev/null 2>&1; then
  if mdfind "kMDItemCFBundleIdentifier == '${EXPECTED_BUNDLE_ID}'" | grep -Fq "${APP_PATH}"; then
    echo "OK: Spotlight index can find this app by bundle identifier."
  else
    echo "WARN: Spotlight query does not currently return this app path."
    echo "      If this is a fresh local build, try:"
    echo "      mdimport \"${APP_PATH}\""
    echo "      /usr/bin/killall Finder"
    warnings=$((warnings + 1))
  fi
else
  echo "WARN: mdfind not available; skipping Spotlight query check."
  warnings=$((warnings + 1))
fi

if [[ "${failures}" -gt 0 ]]; then
  echo
  echo "FAILED: ${failures} required check(s) failed."
  exit 1
fi

echo
echo "PASS: required bundle metadata checks passed."
if [[ "${warnings}" -gt 0 ]]; then
  echo "PASS WITH WARNINGS: ${warnings} non-blocking discoverability hint(s)."
fi
