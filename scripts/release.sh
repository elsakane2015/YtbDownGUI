#!/usr/bin/env bash
#
# One-shot release build with Xcode-style version + build number.
# - Marketing version comes from tauri.conf.json's `version`.
# - Build number lives in `.buildnumber` at the repo root and is
#   auto-incremented every time this script runs (kept 3-digit-zero-padded).
# - Post-build: patches CFBundleVersion in the .app's Info.plist, re-signs,
#   then rebuilds the DMG (with an Applications shortcut for drag-install).
# - Each run lands in its own folder under `releases/v<ver>-b<build>/` so
#   older builds aren't overwritten.
#
# Usage:
#   bash scripts/release.sh
#
# Output:
#   releases/v<version>-b<build>/YtbDownGUI_<version>_b<build>_universal.dmg

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

# --- archive folder for this release --------------------------------------
RELEASE_DIR="${REPO_ROOT}/releases/v${VERSION}-b${BUILD_STR}"
mkdir -p "${RELEASE_DIR}"
DMG_FINAL="${RELEASE_DIR}/YtbDownGUI_${VERSION}_b${BUILD_STR}_universal.dmg"
rm -f "${DMG_FINAL}"

# --- stage the DMG contents with /Applications symlink so the drag-install
# UX works (when the user opens the DMG they see both YtbDownGUI.app and
# a shortcut to /Applications, and drag the icon between them).
STAGE=$(mktemp -d "${TMPDIR:-/tmp}/ytbdowngui-dmg.XXXXXX")
trap 'rm -rf "${STAGE}"' EXIT
ditto "${APP}" "${STAGE}/YtbDownGUI.app"
ln -s /Applications "${STAGE}/Applications"

hdiutil create \
  -volname "YtbDownGUI ${VERSION}" \
  -srcfolder "${STAGE}" \
  -ov \
  -format UDZO \
  "${DMG_FINAL}" >/dev/null
echo "DMG: ${DMG_FINAL}"

# Also drop the unsigned .app folder next to it for reference (handy when
# debugging or re-signing without rebuilding).
ditto "${APP}" "${RELEASE_DIR}/YtbDownGUI.app" 2>/dev/null || true

# Tauri's own bundle/dmg output (without the Applications shortcut) is left
# in place — it's the throwaway version the bundler always produces. Our
# canonical artifact is the one under releases/.

# --- summary --------------------------------------------------------------
echo
echo "==========================================="
echo "  YtbDownGUI v${VERSION} (Build ${BUILD_STR})"
echo "==========================================="
echo "  .app : ${RELEASE_DIR}/YtbDownGUI.app"
echo "  .dmg : ${DMG_FINAL}"
echo "  size : $(du -h "${DMG_FINAL}" | awk '{print $1}')"
echo "  sha  : $(shasum -a 256 "${DMG_FINAL}" | awk '{print $1}')"
echo
echo "Next build: $(printf "%03d" $((NEXT + 1)))"
