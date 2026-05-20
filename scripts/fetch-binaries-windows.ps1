# Fetch yt-dlp.exe and ffmpeg.exe for Windows x64 and place them in
# src-tauri/binaries/ with the Tauri sidecar naming convention.
#
# Re-runnable — skips files that already exist.
# Equivalent to scripts/fetch-binaries.sh but for Windows.
#
# Usage (from repo root):
#   pwsh scripts/fetch-binaries-windows.ps1
#   # or on Windows native:
#   powershell -ExecutionPolicy Bypass -File scripts\fetch-binaries-windows.ps1

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Out = Join-Path $RepoRoot "src-tauri\binaries"
New-Item -ItemType Directory -Force -Path $Out | Out-Null

$YtdlpVersion = "2026.03.17"
$FfmpegVersion = "7.1.1"

# --- yt-dlp ---------------------------------------------------------------
# Official Windows build. Single file, no DLL deps.
$YtdlpUrl = "https://github.com/yt-dlp/yt-dlp/releases/download/$YtdlpVersion/yt-dlp.exe"
$YtdlpDst = Join-Path $Out "yt-dlp-x86_64-pc-windows-msvc.exe"

if (-not (Test-Path $YtdlpDst)) {
    Write-Host "[1/2] Downloading yt-dlp $YtdlpVersion (Windows x64)…"
    Invoke-WebRequest -Uri $YtdlpUrl -OutFile $YtdlpDst
    Write-Host "  -> $YtdlpDst"
} else {
    Write-Host "[1/2] yt-dlp already present, skipping."
}

# --- ffmpeg ---------------------------------------------------------------
# BtbN/FFmpeg-Builds publishes static Windows binaries on GitHub Releases.
# We pull the latest release essentials build (no extra codecs we don't need)
# and extract only ffmpeg.exe.
$FfmpegDst = Join-Path $Out "ffmpeg-x86_64-pc-windows-msvc.exe"

if (-not (Test-Path $FfmpegDst)) {
    Write-Host "[2/2] Downloading ffmpeg $FfmpegVersion (Windows x64)…"
    # Latest essentials build from BtbN/FFmpeg-Builds, the standard source
    # for portable Windows ffmpeg static binaries.
    $FfmpegZipUrl = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
    $TmpZip = Join-Path $Out ".ffmpeg-win64.zip"
    $TmpDir = Join-Path $Out ".ffmpeg-win64-extract"
    if (Test-Path $TmpDir) { Remove-Item -Recurse -Force $TmpDir }

    Invoke-WebRequest -Uri $FfmpegZipUrl -OutFile $TmpZip
    Expand-Archive -Path $TmpZip -DestinationPath $TmpDir -Force

    $FfmpegExe = Get-ChildItem -Path $TmpDir -Filter "ffmpeg.exe" -Recurse |
                 Select-Object -First 1
    if ($null -eq $FfmpegExe) {
        throw "ffmpeg.exe not found inside $TmpZip"
    }
    Copy-Item -Path $FfmpegExe.FullName -Destination $FfmpegDst -Force
    Remove-Item -Force $TmpZip
    Remove-Item -Recurse -Force $TmpDir
    Write-Host "  -> $FfmpegDst"
} else {
    Write-Host "[2/2] ffmpeg already present, skipping."
}

# --- summary --------------------------------------------------------------
Write-Host ""
Write-Host "Done. Windows sidecar binaries in $Out :"
Get-ChildItem -Path $Out -Filter "*-pc-windows-msvc.exe" |
    ForEach-Object { Write-Host ("  {0,-12} {1}" -f ([math]::Round($_.Length / 1MB, 1).ToString() + "MB"), $_.Name) }
