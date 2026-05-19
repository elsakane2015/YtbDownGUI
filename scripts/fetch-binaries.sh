#!/usr/bin/env bash
#
# Fetch yt-dlp and ffmpeg binaries for macOS (arm64 + x86_64) and place them in
# src-tauri/binaries/ named per Tauri sidecar convention:
#     <name>-<rustc-target-triple>
#
# Run from repo root: bash scripts/fetch-binaries.sh
# Re-runnable — checks existing files and only downloads what's missing.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${ROOT_DIR}/src-tauri/binaries"
mkdir -p "${OUT}"

# Versions — bump these to update bundled binaries.
YTDLP_VERSION="2025.04.30"           # https://github.com/yt-dlp/yt-dlp/releases
FFMPEG_BUILD="7.1"                    # https://www.osxexperts.net/ (static universal builds)

# --- yt-dlp ----------------------------------------------------------------
# yt-dlp publishes universal macOS binaries (`yt-dlp_macos`).
# Same binary works for both arm64 and x86_64; we duplicate it under both
# triple-suffixed names because Tauri's sidecar resolver expects per-triple files.

YTDLP_URL="https://github.com/yt-dlp/yt-dlp/releases/download/${YTDLP_VERSION}/yt-dlp_macos"
YTDLP_TMP="${OUT}/.yt-dlp_macos.tmp"

if [[ ! -f "${OUT}/yt-dlp-aarch64-apple-darwin" || ! -f "${OUT}/yt-dlp-x86_64-apple-darwin" ]]; then
  echo "[1/2] Downloading yt-dlp ${YTDLP_VERSION}…"
  curl -fL --progress-bar -o "${YTDLP_TMP}" "${YTDLP_URL}"
  chmod +x "${YTDLP_TMP}"
  cp "${YTDLP_TMP}" "${OUT}/yt-dlp-aarch64-apple-darwin"
  cp "${YTDLP_TMP}" "${OUT}/yt-dlp-x86_64-apple-darwin"
  rm -f "${YTDLP_TMP}"
  echo "  → ${OUT}/yt-dlp-{aarch64,x86_64}-apple-darwin"
else
  echo "[1/2] yt-dlp already present, skipping."
fi

# --- ffmpeg ----------------------------------------------------------------
# Static universal builds from https://www.osxexperts.net (single binary, ~80MB).
# Fallback: evermeet.cx hosts arm64 + x86_64 builds separately.
#
# We try osxexperts first (universal); on failure use evermeet's per-arch builds.

FFMPEG_AARCH64="${OUT}/ffmpeg-aarch64-apple-darwin"
FFMPEG_X86_64="${OUT}/ffmpeg-x86_64-apple-darwin"

fetch_ffmpeg_evermeet() {
  local arch="$1"           # arm64 or amd64
  local out_path="$2"
  local url="https://evermeet.cx/ffmpeg/getrelease/ffmpeg/zip"
  if [[ "${arch}" == "arm64" ]]; then
    # evermeet hosts arm64 builds at a different path
    url="https://www.osxexperts.net/ffmpeg711arm.zip"
  fi
  local tmp_zip="${OUT}/.ffmpeg-${arch}.zip"
  local tmp_extract="${OUT}/.ffmpeg-${arch}-extract"
  echo "    fetching ${arch} from ${url}"
  curl -fL --progress-bar -o "${tmp_zip}" "${url}"
  mkdir -p "${tmp_extract}"
  unzip -o -q "${tmp_zip}" -d "${tmp_extract}"
  # The zip may contain ffmpeg directly or inside a subdir; find it.
  local found
  found="$(find "${tmp_extract}" -maxdepth 3 -name ffmpeg -type f -perm -u+x | head -n1)"
  if [[ -z "${found}" ]]; then
    echo "    ERROR: ffmpeg not found inside zip" >&2
    return 1
  fi
  mv "${found}" "${out_path}"
  chmod +x "${out_path}"
  rm -rf "${tmp_zip}" "${tmp_extract}"
}

if [[ ! -f "${FFMPEG_AARCH64}" ]]; then
  echo "[2/2] Downloading ffmpeg (arm64)…"
  fetch_ffmpeg_evermeet arm64 "${FFMPEG_AARCH64}"
  echo "  → ${FFMPEG_AARCH64}"
else
  echo "[2/2a] ffmpeg arm64 already present, skipping."
fi

if [[ ! -f "${FFMPEG_X86_64}" ]]; then
  echo "[2/2b] Downloading ffmpeg (x86_64)…"
  fetch_ffmpeg_evermeet amd64 "${FFMPEG_X86_64}"
  echo "  → ${FFMPEG_X86_64}"
else
  echo "[2/2b] ffmpeg x86_64 already present, skipping."
fi

# --- summary ---------------------------------------------------------------
echo
echo "Done. Sidecar binaries in ${OUT}:"
ls -lh "${OUT}" | awk 'NR>1 {printf "  %s  %s\n", $5, $NF}'
echo
echo "Quick smoke test:"
"${OUT}/yt-dlp-$(uname -m | sed 's/x86_64/x86_64/;s/arm64/aarch64/')-apple-darwin" --version
"${OUT}/ffmpeg-$(uname -m | sed 's/x86_64/x86_64/;s/arm64/aarch64/')-apple-darwin" -version | head -1
