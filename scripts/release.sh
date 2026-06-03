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
# - Automatically commits .buildnumber, pushes a release tag at the current
#   main commit to trigger Windows GitHub Actions, then creates a GitHub
#   Release page and uploads the macOS DMG.
#
# Requirements:
#   gh (GitHub CLI) must be installed and authenticated.
#
# Usage:
#   bash scripts/release.sh
#   bash scripts/release.sh -free
#   bash scripts/release.sh --free
#
# Output:
#   releases/v<version>-b<build>/YtbDownGUI_<version>_b<build>_universal.dmg

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

usage() {
  cat <<'EOF'
Usage:
  bash scripts/release.sh         Build main/free release
  bash scripts/release.sh -free   Build main/free release; requires main branch
  bash scripts/release.sh --free  Same as -free

Pro releases must be built from the pro-dev branch:
  git checkout pro-dev
  bash scripts/release.sh -pro
EOF
}

REQUESTED_CHANNEL="${RELEASE_CHANNEL:-free}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    -free|--free)
      REQUESTED_CHANNEL="free"
      ;;
    -auto|--auto)
      REQUESTED_CHANNEL="free"
      ;;
    -pro|--pro)
      echo "ERROR: -pro must be run on the pro-dev branch, not main."
      echo "Run: git checkout pro-dev && bash scripts/release.sh -pro"
      exit 1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "ERROR: unknown argument: $1"
      usage
      exit 1
      ;;
  esac
  shift
done

if [[ "${REQUESTED_CHANNEL}" != "free" ]]; then
  echo "ERROR: main branch release script only supports free releases."
  echo "Run Pro releases from pro-dev with: bash scripts/release.sh -pro"
  exit 1
fi

CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [[ "${CURRENT_BRANCH}" != "main" ]]; then
  echo "ERROR: -free must be run on main. Current branch: ${CURRENT_BRANCH}"
  echo "Run: git checkout main && git pull"
  exit 1
fi

# --- bump build number ----------------------------------------------------
BUILD_FILE="${REPO_ROOT}/.buildnumber"
PREV=$(cat "${BUILD_FILE}" 2>/dev/null | tr -d '[:space:]' || echo "0")
PREV=${PREV:-0}
NEXT=$((10#${PREV} + 1))
BUILD_STR=$(printf "%03d" "${NEXT}")

# --- read marketing version from tauri.conf.json -------------------------
VERSION=$(node -p "require('./src-tauri/tauri.conf.json').version")
TAG="v${VERSION}-b${BUILD_STR}"

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  echo "ERROR: local tag already exists: ${TAG}"
  echo "Delete or choose a new build number before releasing."
  exit 1
fi
REMOTE_TAG_STATUS=0
git ls-remote --exit-code --tags origin "refs/tags/${TAG}" >/dev/null 2>&1 || REMOTE_TAG_STATUS=$?
if [[ "${REMOTE_TAG_STATUS}" -eq 0 ]]; then
  echo "ERROR: remote tag already exists: ${TAG}"
  echo "Do not reuse release tags; increment .buildnumber or delete the stale tag intentionally."
  exit 1
elif [[ "${REMOTE_TAG_STATUS}" -ne 2 ]]; then
  echo "ERROR: could not verify whether remote tag exists: ${TAG}"
  echo "Check network/GitHub access, then retry."
  exit 1
fi

echo "${BUILD_STR}" > "${BUILD_FILE}"
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
RELEASE_DIR="${REPO_ROOT}/releases/${TAG}"
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

# --- commit .buildnumber + push branch/tag --------------------------------
git add "${BUILD_FILE}"
git commit -m "chore: bump build number to ${BUILD_STR}"
git push
echo "Pushed commit"

# Create and push the tag ourselves instead of letting `gh release create`
# create it through the Release API. A normal `git push` tag event is what
# reliably triggers `.github/workflows/release-windows.yml`.
git tag "${TAG}"
git push origin "refs/tags/${TAG}"
echo "Pushed tag: ${TAG}"

# --- create GitHub Release page + macOS DMG -------------------------------
# The Windows GitHub Actions workflow was triggered by the pushed tag. It will
# attach its zip to this release once the Windows build finishes.
RELEASE_NOTES="## macOS
下载 \`.dmg\`，拖入 Applications，首次打开运行：
\`\`\`bash
xattr -dr com.apple.quarantine /Applications/YtbDownGUI.app
\`\`\`

## Windows
Windows 版正在构建中，稍后自动附到此 Release。
下载 \`YtbDownGUI-*-windows-x64.zip\`，解压后直接双击 \`YtbDownGUI.exe\`。
首次启动 SmartScreen 弹窗点「更多信息」→「仍要运行」。"

gh release create "${TAG}" \
  "${DMG_FINAL}" \
  --verify-tag \
  --title "v${VERSION} (Build ${BUILD_STR})" \
  --notes "${RELEASE_NOTES}"
echo "GitHub Release created: ${TAG}"

# Sync the tag that gh just created on the remote back to local
git fetch --tags --force
echo "Local tags synced"

# --- summary --------------------------------------------------------------
echo
echo "==========================================="
echo "  YtbDownGUI v${VERSION} (Build ${BUILD_STR})"
echo "==========================================="
echo "  .app : ${RELEASE_DIR}/YtbDownGUI.app"
echo "  .dmg : ${DMG_FINAL}"
echo "  size : $(du -h "${DMG_FINAL}" | awk '{print $1}')"
echo "  sha  : $(shasum -a 256 "${DMG_FINAL}" | awk '{print $1}')"
echo "  tag  : ${TAG} (git-pushed to GitHub; Windows build triggered)"
echo
echo "Next build: $(printf "%03d" $((NEXT + 1)))"
