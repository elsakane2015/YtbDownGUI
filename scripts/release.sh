#!/usr/bin/env bash
#
# One-shot release build with Xcode-style version + build number.
# - Marketing version comes from tauri.conf.json's `version`.
# - Build number lives in `.buildnumber` at the repo root and is
#   auto-incremented every time this script runs (kept 3-digit-zero-padded).
# - Post-build: patches CFBundleVersion in the .app's Info.plist, re-signs,
#   then rebuilds the DMG with `<version>_b<build>` in the filename.
#
# Usage:
#   bash scripts/release.sh
#
# Output:
#   src-tauri/target/universal-apple-darwin/release/bundle/dmg/
#     YtbDownGUI_<version>_b<build>_universal.dmg

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

# --- bump build number ----------------------------------------------------
BUILD_FILE="${REPO_ROOT}/.buildnumber"
PREV=$(cat "${BUILD_FILE}" 2>/dev/null | tr -d '[:space:]' || echo "0")
PREV=${PREV:-0}
NEXT=$((10#${PREV} + 1))
BUILD_STR=$(printf "%03d" "${NEXT}")
echo "${BUILD_STR}" > "${BUILD_FILE}"

# --- read marketing version from tauri.conf.json -------------------------
VERSION=$(node -p "require('./src-tauri/tauri.conf.json').version")
echo "Building YtbDownGUI v${VERSION} (Build ${BUILD_STR})…"

# --- run tauri build ------------------------------------------------------
pnpm tauri build --target universal-apple-darwin

# --- locate output --------------------------------------------------------
BUNDLE_DIR="${REPO_ROOT}/src-tauri/target/universal-apple-darwin/release/bundle"
APP="${BUNDLE_DIR}/macos/YtbDownGUI.app"
if [[ ! -d "${APP}" ]]; then
  echo "ERROR: ${APP} not found"
  exit 1
fi

# --- patch CFBundleVersion ------------------------------------------------
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion ${BUILD_STR}" "${APP}/Contents/Info.plist"
echo "Patched CFBundleVersion = ${BUILD_STR}"

# --- re-sign (Info.plist mutation invalidates the signature) -------------
codesign --force --deep --sign - "${APP}"
echo "Re-signed ad-hoc"

# --- rebuild DMG with build-number-suffixed name --------------------------
DMG_DIR="${BUNDLE_DIR}/dmg"
mkdir -p "${DMG_DIR}"
# remove any stale dmg from this run
rm -f "${DMG_DIR}/YtbDownGUI_${VERSION}_universal.dmg"
DMG_FINAL="${DMG_DIR}/YtbDownGUI_${VERSION}_b${BUILD_STR}_universal.dmg"
rm -f "${DMG_FINAL}"

hdiutil create \
  -volname "YtbDownGUI ${VERSION}" \
  -srcfolder "${APP}" \
  -ov \
  -format UDZO \
  "${DMG_FINAL}" >/dev/null
echo "DMG: ${DMG_FINAL}"

# --- summary --------------------------------------------------------------
echo
echo "==========================================="
echo "  YtbDownGUI v${VERSION} (Build ${BUILD_STR})"
echo "==========================================="
echo "  .app : ${APP}"
echo "  .dmg : ${DMG_FINAL}"
echo "  size : $(du -h "${DMG_FINAL}" | awk '{print $1}')"
echo "  sha  : $(shasum -a 256 "${DMG_FINAL}" | awk '{print $1}')"
echo
echo "Next build: $(printf "%03d" $((NEXT + 1)))"
