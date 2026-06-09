#!/usr/bin/env bash
#
# One-shot release build with Xcode-style version + build number.
# - Marketing version comes from tauri.conf.json's `version`.
# - Build number lives in `.buildnumber` at the repo root and is
#   auto-incremented every time this script runs (kept 3-digit-zero-padded).
# - `main`/free releases keep the historical `v<ver>-b<build>` tag.
# - `pro-dev` releases use `pro-v<ver>-b<build>` and show "Pro" before
#   the version in release artifacts and the app footer.
# - macOS bundles are signed with the configured Developer ID Application
#   identity. The final DMG is rebuilt with an Applications shortcut, signed,
#   notarized through Apple, and stapled.
# - Each run lands in its own folder under `releases/<tag>/` so
#   older builds aren't overwritten.
# - Automatically commits .buildnumber, pushes a release tag at the current
#   commit to trigger Windows GitHub Actions, then creates a GitHub Release
#   page and uploads the macOS DMG.
#
# Requirements:
#   gh (GitHub CLI) must be installed and authenticated.
#   The configured Developer ID Application certificate must be installed in
#   the login keychain.
#   A valid notarytool profile must exist in the configured file-based
#   Keychain (defaults to AC_PASSWORD in the login keychain).
#
# Usage:
#   bash scripts/release.sh        # auto: pro-dev -> Pro, otherwise free
#   bash scripts/release.sh -pro   # require pro-dev and build Pro
#   bash scripts/release.sh -free  # require main and build free
#
# Output:
#   Free: releases/v<version>-b<build>/YtbDownGUI_<version>_b<build>_universal.dmg
#   Pro : releases/pro-v<version>-b<build>/YtbDownGUI_Pro_<version>_b<build>_universal.dmg

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

usage() {
  cat <<'EOF'
Usage:
  bash scripts/release.sh          Auto channel from current branch
  bash scripts/release.sh -pro     Build Pro release; requires pro-dev branch
  bash scripts/release.sh --pro    Same as -pro
  bash scripts/release.sh -free    Build main/free release; requires main branch
  bash scripts/release.sh --free   Same as -free

Environment fallback:
  RELEASE_CHANNEL=pro|free|auto bash scripts/release.sh
  NOTARY_PROFILE=AC_PASSWORD bash scripts/release.sh
  NOTARY_KEYCHAIN=/path/to/login.keychain-db bash scripts/release.sh
EOF
}

REQUESTED_CHANNEL="${RELEASE_CHANNEL:-auto}"
STRICT_BRANCH_CHECK=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    -pro|--pro)
      REQUESTED_CHANNEL="pro"
      STRICT_BRANCH_CHECK=true
      ;;
    -free|--free)
      REQUESTED_CHANNEL="free"
      STRICT_BRANCH_CHECK=true
      ;;
    -auto|--auto)
      REQUESTED_CHANNEL="auto"
      STRICT_BRANCH_CHECK=false
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

CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
RELEASE_CHANNEL="${REQUESTED_CHANNEL}"
if [[ "${RELEASE_CHANNEL}" == "auto" ]]; then
  if [[ "${CURRENT_BRANCH}" == "pro-dev" ]]; then
    RELEASE_CHANNEL="pro"
  else
    RELEASE_CHANNEL="free"
  fi
fi
if [[ "${RELEASE_CHANNEL}" != "free" && "${RELEASE_CHANNEL}" != "pro" ]]; then
  echo "ERROR: RELEASE_CHANNEL must be free, pro, or auto"
  exit 1
fi

if [[ "${STRICT_BRANCH_CHECK}" == true ]]; then
  if [[ "${RELEASE_CHANNEL}" == "pro" && "${CURRENT_BRANCH}" != "pro-dev" ]]; then
    echo "ERROR: -pro must be run on pro-dev. Current branch: ${CURRENT_BRANCH}"
    echo "Run: git checkout pro-dev && git pull"
    exit 1
  fi
  if [[ "${RELEASE_CHANNEL}" == "free" && "${CURRENT_BRANCH}" != "main" ]]; then
    echo "ERROR: -free must be run on main. Current branch: ${CURRENT_BRANCH}"
    echo "Run: git checkout main && git pull"
    exit 1
  fi
fi

if [[ "${RELEASE_CHANNEL}" == "pro" ]]; then
  CHANNEL_LABEL="Pro"
  TAG_PREFIX="pro-v"
  ARTIFACT_PREFIX="YtbDownGUI_Pro"
else
  CHANNEL_LABEL=""
  TAG_PREFIX="v"
  ARTIFACT_PREFIX="YtbDownGUI"
fi
export YTBDOWN_RELEASE_CHANNEL="${RELEASE_CHANNEL}"
export YTBDOWN_BUILD_CHANNEL_LABEL="${CHANNEL_LABEL}"

# Require a real Developer ID identity before the build number is mutated.
# APPLE_SIGNING_IDENTITY can override the repository default when rotating
# certificates or building on another authorized Mac.
CONFIGURED_SIGNING_IDENTITY=$(node -p "require('./src-tauri/tauri.conf.json').bundle.macOS.signingIdentity || ''")
MACOS_SIGNING_IDENTITY="${APPLE_SIGNING_IDENTITY:-${CONFIGURED_SIGNING_IDENTITY}}"
if [[ -z "${MACOS_SIGNING_IDENTITY}" || "${MACOS_SIGNING_IDENTITY}" == "-" ]]; then
  echo "ERROR: macOS release builds require a Developer ID Application signing identity."
  exit 1
fi
if [[ "${MACOS_SIGNING_IDENTITY}" != "Developer ID Application:"* ]]; then
  echo "ERROR: macOS release builds cannot use an Apple Development identity:"
  echo "  ${MACOS_SIGNING_IDENTITY}"
  exit 1
fi
if [[ "${MACOS_SIGNING_IDENTITY}" =~ \(([A-Z0-9]+)\)$ ]]; then
  MACOS_TEAM_ID="${BASH_REMATCH[1]}"
else
  echo "ERROR: could not read Team ID from signing identity:"
  echo "  ${MACOS_SIGNING_IDENTITY}"
  exit 1
fi
if ! security find-identity -v -p codesigning | grep -F "\"${MACOS_SIGNING_IDENTITY}\"" >/dev/null; then
  echo "ERROR: signing identity is not available in the keychain:"
  echo "  ${MACOS_SIGNING_IDENTITY}"
  exit 1
fi
export APPLE_SIGNING_IDENTITY="${MACOS_SIGNING_IDENTITY}"
echo "Signing identity: ${MACOS_SIGNING_IDENTITY}"

# Reuse the Apple ID + app-specific password credentials stored by notarytool.
# The profile belongs to the Developer Team and can notarize multiple apps.
NOTARY_PROFILE="${NOTARY_PROFILE:-AC_PASSWORD}"
NOTARY_KEYCHAIN="${NOTARY_KEYCHAIN:-${HOME}/Library/Keychains/login.keychain-db}"
if [[ ! -f "${NOTARY_KEYCHAIN}" ]]; then
  echo "ERROR: notarytool Keychain file does not exist:"
  echo "  ${NOTARY_KEYCHAIN}"
  exit 1
fi
NOTARY_PROFILE_READY=false
for attempt in 1 2 3; do
  if xcrun notarytool history \
    --keychain-profile "${NOTARY_PROFILE}" \
    --keychain "${NOTARY_KEYCHAIN}" \
    --output-format json >/dev/null; then
    NOTARY_PROFILE_READY=true
    break
  fi
  if [[ "${attempt}" -lt 3 ]]; then
    echo "Notary profile validation failed (attempt ${attempt}/3); retrying..."
    sleep 2
  fi
done
if [[ "${NOTARY_PROFILE_READY}" != true ]]; then
  echo "ERROR: notarytool Keychain profile is unavailable or invalid:"
  echo "  ${NOTARY_PROFILE}"
  echo "Keychain: ${NOTARY_KEYCHAIN}"
  echo "Store or refresh it with:"
  echo "  xcrun notarytool store-credentials ${NOTARY_PROFILE} --team-id ${MACOS_TEAM_ID} --keychain ${NOTARY_KEYCHAIN}"
  exit 1
fi
echo "Notary profile: ${NOTARY_PROFILE} (${NOTARY_KEYCHAIN})"

# Validate release-only requirements before mutating .buildnumber.
pnpm preflight:release

# --- bump build number ----------------------------------------------------
BUILD_FILE="${REPO_ROOT}/.buildnumber"
PREV=$(cat "${BUILD_FILE}" 2>/dev/null | tr -d '[:space:]' || echo "0")
PREV=${PREV:-0}
NEXT=$((10#${PREV} + 1))
BUILD_STR=$(printf "%03d" "${NEXT}")

# --- read marketing version from tauri.conf.json -------------------------
VERSION=$(node -p "require('./src-tauri/tauri.conf.json').version")
VERSION_LABEL="${CHANNEL_LABEL:+${CHANNEL_LABEL} }v${VERSION}"
TAG="${TAG_PREFIX}${VERSION}-b${BUILD_STR}"

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  echo "ERROR: local tag already exists: ${TAG}"
  echo "Delete or choose a new build number before releasing."
  exit 1
fi

REMOTE_TAG_OUTPUT=""
REMOTE_TAG_CHECKED=false
for attempt in 1 2 3; do
  if REMOTE_TAG_OUTPUT=$(git ls-remote --tags origin "refs/tags/${TAG}" 2>&1); then
    REMOTE_TAG_CHECKED=true
    break
  fi
  if [[ "${attempt}" -lt 3 ]]; then
    echo "Remote tag check failed (attempt ${attempt}/3); retrying..."
    sleep 2
  fi
done
if [[ "${REMOTE_TAG_CHECKED}" != true ]]; then
  echo "ERROR: could not verify whether remote tag exists: ${TAG}"
  echo "${REMOTE_TAG_OUTPUT}"
  echo "Check network/GitHub access, then retry."
  exit 1
fi
if [[ -n "${REMOTE_TAG_OUTPUT}" ]]; then
  echo "ERROR: remote tag already exists: ${TAG}"
  echo "Do not reuse release tags; increment .buildnumber or delete the stale tag intentionally."
  exit 1
fi

echo "${BUILD_STR}" > "${BUILD_FILE}"
echo "Building YtbDownGUI ${VERSION_LABEL} (Build ${BUILD_STR})…"

# --- run tauri build ------------------------------------------------------
MACOS_BUILD_CONFIG="{\"bundle\":{\"macOS\":{\"bundleVersion\":\"${BUILD_STR}\"}}}"
pnpm tauri build --target universal-apple-darwin --config "${MACOS_BUILD_CONFIG}"

# --- locate output --------------------------------------------------------
BUNDLE_DIR="${REPO_ROOT}/src-tauri/target/universal-apple-darwin/release/bundle"
APP="${BUNDLE_DIR}/macos/YtbDownGUI.app"
if [[ ! -d "${APP}" ]]; then
  echo "ERROR: ${APP} not found"
  exit 1
fi

# Tauri receives CFBundleVersion before bundling so no post-signature
# Info.plist mutation is needed.
ACTUAL_BUILD_STR=$(/usr/libexec/PlistBuddy -c "Print :CFBundleVersion" "${APP}/Contents/Info.plist")
if [[ "${ACTUAL_BUILD_STR}" != "${BUILD_STR}" ]]; then
  echo "ERROR: expected CFBundleVersion ${BUILD_STR}, got ${ACTUAL_BUILD_STR}"
  exit 1
fi
codesign --verify --deep --strict --verbose=2 "${APP}"
if ! codesign -dvv "${APP}" 2>&1 | grep -F "Authority=${MACOS_SIGNING_IDENTITY}" >/dev/null; then
  echo "ERROR: app was not signed with the expected Developer ID identity."
  exit 1
fi
echo "Verified signed app (CFBundleVersion ${ACTUAL_BUILD_STR})"

# --- archive folder for this release --------------------------------------
RELEASE_DIR="${REPO_ROOT}/releases/${TAG}"
mkdir -p "${RELEASE_DIR}"
DMG_FINAL="${RELEASE_DIR}/${ARTIFACT_PREFIX}_${VERSION}_b${BUILD_STR}_universal.dmg"
rm -f "${DMG_FINAL}"

# --- stage the DMG contents with /Applications symlink so the drag-install
# UX works (when the user opens the DMG they see both YtbDownGUI.app and
# a shortcut to /Applications, and drag the icon between them).
STAGE=$(mktemp -d "${TMPDIR:-/tmp}/ytbdowngui-dmg.XXXXXX")
trap 'rm -rf "${STAGE}"' EXIT
ditto "${APP}" "${STAGE}/YtbDownGUI.app"
ln -s /Applications "${STAGE}/Applications"

hdiutil create \
  -volname "YtbDownGUI ${VERSION_LABEL}" \
  -srcfolder "${STAGE}" \
  -ov \
  -format UDZO \
  "${DMG_FINAL}" >/dev/null
codesign --force --timestamp --sign "${MACOS_SIGNING_IDENTITY}" "${DMG_FINAL}"
codesign --verify --strict --verbose=2 "${DMG_FINAL}"
if ! codesign -dvv "${DMG_FINAL}" 2>&1 | grep -F "Authority=${MACOS_SIGNING_IDENTITY}" >/dev/null; then
  echo "ERROR: DMG was not signed with the expected Developer ID identity."
  exit 1
fi
echo "Signed DMG: ${DMG_FINAL}"

# --- notarize, staple, and verify the final distribution artifact ----------
NOTARY_JSON="${RELEASE_DIR}/notarytool.json"
NOTARY_ERR="${RELEASE_DIR}/notarytool.err"
rm -f "${NOTARY_JSON}" "${NOTARY_ERR}"
echo "Submitting DMG to Apple notarization..."
set +e
xcrun notarytool submit "${DMG_FINAL}" \
  --keychain-profile "${NOTARY_PROFILE}" \
  --keychain "${NOTARY_KEYCHAIN}" \
  --wait \
  --timeout 30m \
  --output-format json >"${NOTARY_JSON}" 2>"${NOTARY_ERR}"
NOTARY_RC=$?
set -e
cat "${NOTARY_ERR}" >&2
cat "${NOTARY_JSON}"

NOTARY_STATUS=$(node -e '
  try {
    const result = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
    process.stdout.write(result.status || "");
  } catch {}
' "${NOTARY_JSON}")
NOTARY_ID=$(node -e '
  try {
    const result = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
    process.stdout.write(result.id || "");
  } catch {}
' "${NOTARY_JSON}")
if [[ "${NOTARY_RC}" -ne 0 || "${NOTARY_STATUS}" != "Accepted" ]]; then
  echo "ERROR: Apple notarization failed with status: ${NOTARY_STATUS:-unknown}"
  if [[ -n "${NOTARY_ID}" ]]; then
    echo "Inspect the log with:"
    echo "  xcrun notarytool log ${NOTARY_ID} --keychain-profile ${NOTARY_PROFILE} --keychain ${NOTARY_KEYCHAIN}"
  fi
  exit 1
fi

xcrun stapler staple "${DMG_FINAL}"
xcrun stapler validate "${DMG_FINAL}"
spctl --assess \
  --type open \
  --context context:primary-signature \
  --verbose=4 \
  "${DMG_FINAL}"
spctl --assess --type execute --verbose=4 "${APP}"
echo "Notarized and stapled DMG: ${DMG_FINAL}"

# Also drop the signed .app folder next to it for reference.
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
# reliably triggers `.github/workflows/release-windows.yml`, and it keeps Pro
# tags attached to the current pro-dev HEAD instead of GitHub's default branch.
git tag "${TAG}"
git push origin "refs/tags/${TAG}"
echo "Pushed tag: ${TAG}"

# --- create GitHub Release page + macOS DMG -------------------------------
# The Windows GitHub Actions workflow was triggered by the pushed tag. It will
# attach its zip to this release once the Windows build finishes.
RELEASE_NOTES="## macOS
下载 \`.dmg\` 并拖入 Applications。如果首次打开仍被系统拦截，可运行：
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
  --title "${VERSION_LABEL} (Build ${BUILD_STR})" \
  --notes "${RELEASE_NOTES}"
echo "GitHub Release created: ${TAG}"

# Sync the tag that gh just created on the remote back to local
git fetch --tags --force
echo "Local tags synced"

# --- summary --------------------------------------------------------------
echo
echo "==========================================="
echo "  YtbDownGUI ${VERSION_LABEL} (Build ${BUILD_STR})"
echo "==========================================="
echo "  .app : ${RELEASE_DIR}/YtbDownGUI.app"
echo "  .dmg : ${DMG_FINAL}"
echo "  size : $(du -h "${DMG_FINAL}" | awk '{print $1}')"
echo "  sha  : $(shasum -a 256 "${DMG_FINAL}" | awk '{print $1}')"
echo "  tag  : ${TAG} (git-pushed to GitHub; Windows build triggered)"
echo
echo "Next build: $(printf "%03d" $((NEXT + 1)))"
