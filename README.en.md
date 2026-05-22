# YtbDownGUI

> [中文](./README.md) · **English**

Cross-platform GUI for [yt-dlp](https://github.com/yt-dlp/yt-dlp) + [ffmpeg](https://ffmpeg.org/) (macOS + Windows), built with Tauri v2 (Rust + React).

The differentiator from existing yt-dlp wrappers is an **embedded login WebView**: open a window inside the app, log into the target site as you normally would in a browser, and the app picks up the session cookies and feeds them to yt-dlp. No more manually exporting `cookies.txt`.

> ⚠️ Personal-use tool for downloading content you have the right to access. Respect the site's terms of service and the creators' rights.

## Features

- **Single video** — paste URL, probe, pick quality (combined / video+audio split / audio-only), pick subtitle languages, download.
- **Playlist / channel batch** — paste a playlist or channel URL, filter entries (date range / keyword / max rows), tick the ones you want, apply a unified quality preset, download all at once.
- **Three-section format picker** — H.264 / VP9 / AV1 codec preference, 480p–4K resolution cap, separate audio codec choice, mp4 / mkv container.
- **Subtitles** — pick from each video's available languages (manual + YouTube auto-captions independently), choose between sidecar `.srt` files or embedded into the container.
- **Bundled binaries** — yt-dlp 2026.03.17 and ffmpeg 7.1.1 ship with the app. Zero dependencies on your friend's machine. macOS ships as a universal binary (ARM + Intel); Windows ships as an x64 portable zip.
- **In-app yt-dlp updates** — on startup, checks GitHub for newer yt-dlp; if available a blue banner lets you update in one click. New binary is saved to `~/Library/Application Support/com.litotime.ytbdowngui/bin/` on macOS, or `%APPDATA%\com.litotime.ytbdowngui\bin\` on Windows (portable mode: `<exe-dir>\data\bin\`).
- **Native-feeling UI** — macOS: traffic-light title bar, system font, dark mode follows the OS, draggable toolbar. Windows: standard window chrome, dark mode follows the OS.
- **Persistent state** — job history survives app restarts; settings stored as plain JSON.

### Supported sites (login)

| Site | URL pattern | Marker cookie |
|---|---|---|
| YouTube | `youtube.com`, `youtu.be` | `SAPISID` |
| Bilibili | `bilibili.com`, `b23.tv` | `SESSDATA` |
| X (Twitter) | `x.com`, `twitter.com` | `auth_token` |
| 腾讯视频 | `v.qq.com` | `vqq_vuserid` |
| 抖音 | `douyin.com` | `sessionid_ss` |
| TikTok | `tiktok.com` | `sessionid` |
| Pinterest | `pinterest.com`, `pin.it` | `_pinterest_sess` |

Public content on these sites (and any of the [1000+ sites yt-dlp supports](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md)) works as a guest without login. Login is only needed for: high-resolution YouTube, login-walled Bilibili content, age/sensitive-restricted tweets, region-locked TikTok, etc.

## Install (for friends)

### macOS

1. Download the `.dmg` from the [Releases](https://github.com/elsakane2015/YtbDownGUI/releases) page.
2. Open the `.dmg`, drag `YtbDownGUI.app` to `Applications`.
3. Open the app. On macOS Sequoia 15.1+ you may see "damaged and can't be opened" or "cannot verify it is free of malware".
4. Open Terminal and run:
   ```bash
   xattr -dr com.apple.quarantine /Applications/YtbDownGUI.app
   ```
5. Open the app again — it should launch normally.

> The app is ad-hoc signed (no Apple Developer ID). The quarantine removal is a one-time step macOS requires for any app from outside the App Store.

### Windows

1. Download `YtbDownGUI_<version>_windows_x64.zip` from the [Releases](https://github.com/elsakane2015/YtbDownGUI/releases) page.
2. Extract the zip to any folder (e.g. `C:\Program Files\YtbDownGUI` or your Desktop).
3. Double-click `YtbDownGUI.exe` — no installer needed.
4. On first launch Windows Defender SmartScreen may show "Windows protected your PC". Click **More info → Run anyway**.

> This is a portable build. All data (settings, cookies, job history) is stored in `%APPDATA%\com.litotime.ytbdowngui\` by default. For a fully portable setup (e.g. on a USB drive), create an empty `portable.txt` file next to `YtbDownGUI.exe` and data will be stored in `<exe-dir>\data\` instead.

## First-run setup

1. Open the **设置** tab. Pick a download folder, default quality, optional proxy.
2. Open the **账号** tab. Click **登录** next to the site you want to download from. A login window opens; sign in normally. Once the site's session cookie is detected the window auto-closes and your cookies are saved.
3. Open the **下载** tab. Paste a URL. Click **分析**. Pick quality + subtitles. Click **下载**.

The `.mp4` (or whatever container you picked) lands in your download folder. Each task row has a **Show in Finder** (macOS) / **Show in Explorer** (Windows) button to jump straight to it.

## Build from source

### macOS

```bash
# Prereqs
brew install rust node pnpm
rustup target add aarch64-apple-darwin x86_64-apple-darwin

# Clone
git clone https://github.com/elsakane2015/YtbDownGUI.git
cd YtbDownGUI

# Fetch the bundled sidecar binaries (yt-dlp + ffmpeg per arch)
bash scripts/fetch-binaries.sh

# Install JS deps + run dev
pnpm install
pnpm tauri dev

# One-shot release: bumps .buildnumber, builds universal .app + .dmg,
# patches CFBundleVersion in Info.plist, re-signs ad-hoc, and writes
# YtbDownGUI_<version>_b<build>_universal.dmg
bash scripts/release.sh

# (Plain `pnpm tauri build` also works; it just doesn't bump the build
# number or rename the DMG.)
pnpm tauri build --target universal-apple-darwin
```

### Windows

The Windows release is built via GitHub Actions (`.github/workflows/release-windows.yml`) and triggered automatically when a `v*-b*` tag is pushed. The artifact is a portable zip attached to the GitHub Release.

To build locally (requires Windows 10/11 + PowerShell 7+):

```powershell
# Install Rust (winget or rustup-init.exe)
winget install Rustlang.Rustup
rustup target add x86_64-pc-windows-msvc

# Install Node + pnpm
winget install OpenJS.NodeJS.LTS
npm install -g pnpm

# Clone
git clone https://github.com/elsakane2015/YtbDownGUI.git
cd YtbDownGUI

# Fetch bundled sidecar binaries (yt-dlp.exe + ffmpeg.exe)
pwsh scripts/fetch-binaries-windows.ps1

# Install JS deps + run dev
pnpm install
pnpm tauri dev

# Build (no MSI/NSIS installer — outputs a portable exe)
pnpm tauri build --target x86_64-pc-windows-msvc --no-bundle
```

### Versioning

Xcode-style. The marketing version lives in `src-tauri/tauri.conf.json`
(`"version": "0.0.1"`); the build number lives in `.buildnumber` at the
repo root and is auto-incremented by `scripts/release.sh`. Both surface
in the **设置** tab footer as `v0.0.1 (002)`.

## Architecture (one paragraph)

- **Tauri v2** for window + IPC + sidecar binary plumbing.
- **React + Vite + TypeScript** frontend, hand-rolled CSS (no Material-y component library). macOS mimics native window chrome; Windows uses standard system decoration.
- **yt-dlp** invoked as a sidecar subprocess; output parsed for progress (with `.part`-file polling as a backup because PyInstaller's stdout buffering hides the live progress).
- **ffmpeg** also a sidecar, located via `--ffmpeg-location` so yt-dlp finds it for merging.
- **Cookies** captured directly from the embedded login WebView via `WebviewWindow::cookies()` (Tauri 2.3+) and serialized to the [Netscape `cookies.txt` format](http://fileformats.archiveteam.org/wiki/Netscape_cookies.txt) that yt-dlp expects.
- **Settings + jobs** persisted as plain JSON. macOS: `~/Library/Application Support/com.litotime.ytbdowngui/`; Windows: `%APPDATA%\com.litotime.ytbdowngui\` (or `<exe-dir>\data\` in portable mode).

## Known limitations

- **No DRM**. Anything wrapped in Widevine / FairPlay (Tencent Video VIP movies, Netflix, etc.) can't be downloaded by any yt-dlp-based tool. This isn't a fixable bug.
- **Embedded WebView compatibility** (largely resolved) — The login window injects a script to hide automation markers (`navigator.webdriver`); on Windows it also substitutes a real Chrome user-agent string. Bot-detection blocks from major sites are now rare. If a specific site still refuses to let you sign in inside the app, please open an issue.
- **Live progress is via file polling**, not yt-dlp's stdout. yt-dlp is a PyInstaller bundle whose stdout is block-buffered on non-TTYs, and neither `PYTHONUNBUFFERED` nor PTY wrapping fixes it. File polling sees the `.part` file grow and gives a faithful percent + speed.
- **macOS Sequoia 15.1+ requires at least ad-hoc signing** (macOS only). Intel Macs still work but the OS gate is stricter. On Windows, SmartScreen may warn on first launch — click **More info → Run anyway**.

## License

TBD. The bundled `yt-dlp` is [Unlicense](https://github.com/yt-dlp/yt-dlp/blob/master/LICENSE); `ffmpeg` is LGPL 2.1+ ([BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds), LGPL build). The app code itself is currently unlicensed (all rights reserved) — open an issue if you'd like a license.
