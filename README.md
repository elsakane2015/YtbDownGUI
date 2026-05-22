# YtbDownGUI

> **中文** · [English](./README.en.md)

基于 [yt-dlp](https://github.com/yt-dlp/yt-dlp) + [ffmpeg](https://ffmpeg.org/) 的跨平台 GUI 视频下载器（macOS + Windows），用 Tauri v2 (Rust + React) 构建。

它跟现有的 yt-dlp 命令行包装最大的不同是 **内嵌 WebView 登录**：在 App 内开一个窗口，像在浏览器里一样登录目标网站，App 自动接管 session cookies 喂给 yt-dlp。不需要再手动导出 `cookies.txt` 了。

> ⚠️ 自用工具。仅下载你有权访问的内容，请尊重网站服务条款和创作者权益。

## 功能特性

- **单视频下载** — 粘贴 URL → 分析 → 选画质（组合流 / 视频+音频分选 / 仅音频）→ 选字幕 → 下载。
- **播放列表 / 频道批量下载** — 粘贴 playlist 或 channel URL → 过滤条目（日期范围 / 关键词 / 条目上限）→ 复选框勾选 → 应用统一画质 → 整批下载。
- **三段式画质选择** — H.264 / VP9 / AV1 编码偏好、480p–4K 分辨率上限、音频编码独立选择、mp4 / mkv 容器。
- **字幕** — 每个视频探测后展示可用语言列表（手动字幕 + YouTube 自动字幕独立勾选），可选独立 `.srt` 文件或嵌入到容器。
- **二进制内置** — 打包 yt-dlp 2026.03.17 + ffmpeg 7.1.1，朋友机器零依赖。macOS 为 universal（ARM + Intel），Windows 为 x64 portable zip。
- **App 内 yt-dlp 更新** — 启动时检查 GitHub 是否有新版，有则弹蓝色横幅"更新"按钮，自动下载并替换本地二进制（macOS 存至 `~/Library/Application Support/com.litotime.ytbdowngui/bin/`，Windows 存至 `%APPDATA%\com.litotime.ytbdowngui\bin\` 或 portable 模式下 `<程序目录>\data\bin\`）。
- **原生风格 UI** — macOS：交通灯标题栏、SF 字体、跟随系统亮暗模式、标题栏可拖动。Windows：标准窗口装饰，跟随系统亮暗模式。
- **状态持久化** — 任务列表跨 App 重启保留；设置以 JSON 文件保存。

### 支持的登录站点

| 站点 | URL 模式 | Marker cookie |
|---|---|---|
| YouTube | `youtube.com`, `youtu.be` | `SAPISID` |
| Bilibili | `bilibili.com`, `b23.tv` | `SESSDATA` |
| X (Twitter) | `x.com`, `twitter.com` | `auth_token` |
| 腾讯视频 | `v.qq.com` | `vqq_vuserid` |
| 抖音 | `douyin.com` | `sessionid_ss` |
| TikTok | `tiktok.com` | `sessionid` |
| Pinterest | `pinterest.com`, `pin.it` | `_pinterest_sess` |

这些站点的公开内容（以及 [yt-dlp 支持的 1000+ 站点](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md)）以游客身份就可以下载。登录只在以下场景需要：YouTube 高清晰度、Bilibili 登录墙内容、推特敏感内容/年龄限制、TikTok 区域锁等。

## 安装（给朋友看）

### macOS

1. 从 [Releases](https://github.com/elsakane2015/YtbDownGUI/releases) 页面下载 `.dmg`。
2. 双击 `.dmg`，把 `YtbDownGUI.app` 拖到 `Applications` 文件夹。
3. 打开 App。macOS Sequoia 15.1+ 可能会提示「已损坏，无法打开」或「无法验证此 App 是否包含恶意软件」。
4. 打开"终端"运行下面这条命令一次：
   ```bash
   xattr -dr com.apple.quarantine /Applications/YtbDownGUI.app
   ```
5. 再次打开应该正常启动。

> App 用的是 ad-hoc 签名（没有 Apple Developer ID 公证）。这条 `xattr` 是 macOS 对 App Store 之外的应用首次打开时的一次性要求。

### Windows

1. 从 [Releases](https://github.com/elsakane2015/YtbDownGUI/releases) 页面下载 `YtbDownGUI_<版本>_windows_x64.zip`。
2. 解压到任意文件夹（如 `C:\Program Files\YtbDownGUI` 或桌面）。
3. 直接双击 `YtbDownGUI.exe` 运行，无需安装。
4. 首次启动 Windows Defender SmartScreen 可能弹出"Windows 已保护你的电脑"提示，点击"更多信息" → "仍要运行"即可。

> 这是 portable 免安装版本，所有数据（设置、cookies、任务历史）默认存在 `%APPDATA%\com.litotime.ytbdowngui\` 下。如需完全便携（U 盘携带），在 `YtbDownGUI.exe` 同目录创建一个空文件 `portable.txt`，数据将改存到 `<程序目录>\data\`。

## 首次使用

1. 打开 **设置** tab。设置下载目录、默认画质、（可选）代理。
2. 打开 **账号** tab。点击你要下载的站点右边的 **登录** 按钮。弹出登录窗口，正常登录站点。检测到 session cookie 后窗口自动关闭，cookies 已保存。
3. 打开 **下载** tab。粘贴 URL → 点 **分析** → 选画质 + 字幕 → 点 **下载**。

文件落在你设置的下载目录里。每个任务右侧有 **在访达中显示**（macOS）/ **在资源管理器中显示**（Windows）按钮直接跳过去。

## 从源码构建

### macOS

```bash
# 依赖
brew install rust node pnpm
rustup target add aarch64-apple-darwin x86_64-apple-darwin

# Clone
git clone https://github.com/elsakane2015/YtbDownGUI.git
cd YtbDownGUI

# 拉取内置 sidecar 二进制（yt-dlp + ffmpeg 各架构 + universal）
bash scripts/fetch-binaries.sh

# 安装 JS 依赖 + 启动 dev
pnpm install
pnpm tauri dev

# 正式 release：自动 .buildnumber +1，构建 universal .app + .dmg，
# 修补 CFBundleVersion，ad-hoc 重签名，commit + push，
# 创建 GitHub Release（含 DMG），触发 Windows GitHub Actions 构建。
# 需要 gh CLI 已登录。
bash scripts/release.sh

# （也可以直接跑 pnpm tauri build；只是不会自增 build 号，DMG 也不会带 build 编号）
pnpm tauri build --target universal-apple-darwin
```

### Windows

Windows 版通过 GitHub Actions 构建（`.github/workflows/release-windows.yml`），推送 `v*-b*` 格式的 tag 时自动触发，产物为 portable zip，附加到对应的 GitHub Release。

本地构建步骤（需要 Windows 10/11 + PowerShell 7+）：

```powershell
# 安装 Rust（winget 或 rustup-init.exe）
winget install Rustlang.Rustup
rustup target add x86_64-pc-windows-msvc

# 安装 Node + pnpm（winget 或 nvm-windows）
winget install OpenJS.NodeJS.LTS
npm install -g pnpm

# Clone
git clone https://github.com/elsakane2015/YtbDownGUI.git
cd YtbDownGUI

# 拉取内置 sidecar 二进制（yt-dlp.exe + ffmpeg.exe）
pwsh scripts/fetch-binaries-windows.ps1

# 安装 JS 依赖 + 启动 dev
pnpm install
pnpm tauri dev

# 构建（不打 MSI/NSIS，直接输出 exe）
pnpm tauri build --target x86_64-pc-windows-msvc --no-bundle
```

### 版本号体系

Xcode 风格的 marketing version + build number。
- **marketing version** 在 `src-tauri/tauri.conf.json` 的 `"version": "0.0.1"` 字段。
- **build number** 在仓库根目录 `.buildnumber` 文件，由 `scripts/release.sh` 自动递增（保持 3 位零填充：`001`、`002`、…）。
- 两者在 **设置** tab 底部以 `v0.0.1 (002)` 形式显示。
- macOS 应用菜单 → 关于 YtbDownGUI 也能看到同样的版本号 + LitoTime / GitHub 超链接。

## 架构（一段话说完）

- **Tauri v2** 处理窗口、IPC、sidecar 二进制管理。
- **React + Vite + TypeScript** 前端，手写 CSS（不上 Material 类组件库）。macOS 模拟原生窗口风格，Windows 使用系统标准装饰。
- **yt-dlp** 作为 sidecar 子进程调用，解析输出获取进度（PyInstaller stdout 缓冲过于顽固，所以用轮询 `.part` 文件大小作为可靠的进度来源）。
- **ffmpeg** 也是 sidecar，通过 `--ffmpeg-location` 告诉 yt-dlp 合并视频音频时去哪儿找它。
- **Cookies** 直接通过 `WebviewWindow::cookies()` API（Tauri 2.3+）从内嵌登录窗口提取，按 [Netscape `cookies.txt` 格式](http://fileformats.archiveteam.org/wiki/Netscape_cookies.txt) 序列化喂给 yt-dlp。
- **设置 + 任务历史** 以 JSON 文件持久化。macOS 存在 `~/Library/Application Support/com.litotime.ytbdowngui/`；Windows 存在 `%APPDATA%\com.litotime.ytbdowngui\`（portable 模式下为 `<程序目录>\data\`）。

## 已知限制

- **不支持 DRM 内容**。任何被 Widevine / FairPlay 加密的内容（腾讯 VIP 影视、Netflix 等）任何 yt-dlp 类工具都下不了。这是底层限制，不是 bug。
- **内嵌 WebView 兼容性**（历史问题，现已基本解决）— App 会在登录窗口注入脚本隐藏自动化特征（`navigator.webdriver`），Windows 端另换用真实 Chrome UA，主流网站的 bot 检测拦截问题已大幅改善。如遇特定站点仍无法在 App 内完成登录，请在 issue 中反馈。
- **进度条数据来自文件轮询而不是 yt-dlp 实时进度**。yt-dlp 是 PyInstaller 打包的，stdout 在非 TTY 下深度块缓冲，`PYTHONUNBUFFERED` / PTY 都救不了。轮询 `.part` 文件大小可以给出准确的百分比和速度，但比 yt-dlp 原生进度晚几百毫秒。
- **macOS Sequoia 15.1+ 强制至少 ad-hoc 签名才能开**（macOS 专属）。Intel Mac 也可以跑 universal 包，但 OS 的拦截更严格一点。Windows 无此限制，但首次运行 SmartScreen 会弹出提示，点"仍要运行"即可。

## 许可证

待定。内置的 `yt-dlp` 是 [Unlicense](https://github.com/yt-dlp/yt-dlp/blob/master/LICENSE)；`ffmpeg` 使用 LGPL 2.1+ 编译版本（[BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds)）。本仓库 App 代码目前未声明许可（all rights reserved）—— 需要补 License 的话开 issue。

## 链接

- 作者：[LitoTime](https://litotime.com)
- 仓库：[github.com/elsakane2015/YtbDownGUI](https://github.com/elsakane2015/YtbDownGUI)
