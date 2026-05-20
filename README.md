# YtbDownGUI

> **中文** · [English](./README.en.md)

基于 [yt-dlp](https://github.com/yt-dlp/yt-dlp) + [ffmpeg](https://ffmpeg.org/) 的 macOS GUI 视频下载器，用 Tauri v2 (Rust + React) 构建。

它跟现有的 yt-dlp 命令行包装最大的不同是 **内嵌 WebView 登录**：在 App 内开一个窗口，像在浏览器里一样登录目标网站，App 自动接管 session cookies 喂给 yt-dlp。不需要再手动导出 `cookies.txt` 了。

> ⚠️ 自用工具。仅下载你有权访问的内容，请尊重网站服务条款和创作者权益。

## 功能特性

- **单视频下载** — 粘贴 URL → 分析 → 选画质（组合流 / 视频+音频分选 / 仅音频）→ 选字幕 → 下载。
- **播放列表 / 频道批量下载** — 粘贴 playlist 或 channel URL → 过滤条目（日期范围 / 关键词 / 条目上限）→ 复选框勾选 → 应用统一画质 → 整批下载。
- **三段式画质选择** — H.264 / VP9 / AV1 编码偏好、480p–4K 分辨率上限、音频编码独立选择、mp4 / mkv 容器。
- **字幕** — 每个视频探测后展示可用语言列表（手动字幕 + YouTube 自动字幕独立勾选），可选独立 `.srt` 文件或嵌入到容器。
- **二进制内置** — 包内打包 yt-dlp 2026.03.17（universal）+ ffmpeg 7.1.1，朋友机器零依赖。
- **App 内 yt-dlp 更新** — 启动时检查 GitHub 是否有新版，有则弹蓝色横幅"更新"按钮，下载到 `~/Library/Application Support/com.litotime.ytbdowngui/bin/`。
- **macOS 原生风格** — 交通灯标题栏、SF 字体、跟随系统亮暗模式、标题栏可拖动。
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

1. 从 [Releases](https://github.com/elsakane2015/YtbDownGUI/releases) 页面下载 `.dmg`。
2. 双击 `.dmg`，把 `YtbDownGUI.app` 拖到 `Applications` 文件夹。
3. 打开 App。macOS Sequoia 15.1+ 可能会提示「已损坏，无法打开」或「无法验证此 App 是否包含恶意软件」。
4. 打开"终端"运行下面这条命令一次：
   ```bash
   xattr -dr com.apple.quarantine /Applications/YtbDownGUI.app
   ```
5. 再次打开应该正常启动。

> App 用的是 ad-hoc 签名（没有 Apple Developer ID 公证）。这条 `xattr` 是 macOS 对 App Store 之外的应用首次打开时的一次性要求。

## 首次使用

1. 打开 **设置** tab。设置下载目录、默认画质、（可选）代理。
2. 打开 **账号** tab。点击你要下载的站点右边的 **登录** 按钮。弹出登录窗口，正常登录站点。检测到 session cookie 后窗口自动关闭，cookies 已保存。
3. 打开 **下载** tab。粘贴 URL → 点 **分析** → 选画质 + 字幕 → 点 **下载**。

文件落在你设置的下载目录里。每个任务右侧有 **在访达中显示** 按钮直接跳过去。

## 从源码构建

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
# 修补 CFBundleVersion，ad-hoc 重签名，输出到
# releases/v<version>-b<build>/ 文件夹（旧版本保留不覆盖）
bash scripts/release.sh

# （也可以直接跑 pnpm tauri build；只是不会自增 build 号，DMG 也不会带 build 编号）
pnpm tauri build --target universal-apple-darwin
```

### 版本号体系

Xcode 风格的 marketing version + build number。
- **marketing version** 在 `src-tauri/tauri.conf.json` 的 `"version": "0.0.1"` 字段。
- **build number** 在仓库根目录 `.buildnumber` 文件，由 `scripts/release.sh` 自动递增（保持 3 位零填充：`001`、`002`、…）。
- 两者在 **设置** tab 底部以 `v0.0.1 (002)` 形式显示。
- macOS 应用菜单 → 关于 YtbDownGUI 也能看到同样的版本号 + LitoTime / GitHub 超链接。

## 架构（一段话说完）

- **Tauri v2** 处理窗口、IPC、sidecar 二进制管理。
- **React + Vite + TypeScript** 前端，手写 CSS 模拟 macOS 原生风格（不上 Material 类组件库）。
- **yt-dlp** 作为 sidecar 子进程调用，解析输出获取进度（PyInstaller stdout 缓冲过于顽固，所以用轮询 `.part` 文件大小作为可靠的进度来源）。
- **ffmpeg** 也是 sidecar，通过 `--ffmpeg-location` 告诉 yt-dlp 合并视频音频时去哪儿找它。
- **Cookies** 直接通过 `WebviewWindow::cookies()` API（Tauri 2.3+）从内嵌登录窗口提取，按 [Netscape `cookies.txt` 格式](http://fileformats.archiveteam.org/wiki/Netscape_cookies.txt) 序列化喂给 yt-dlp。
- **设置 + 任务历史** 以 JSON 文件持久化在 `~/Library/Application Support/com.litotime.ytbdowngui/`。

## 已知限制

- **不支持 DRM 内容**。任何被 Widevine / FairPlay 加密的内容（腾讯 VIP 影视、Netflix 等）任何 yt-dlp 类工具都下不了。这是底层限制，不是 bug。
- **WKWebView 偶尔被 Google 拦"不安全浏览器"** — 万一在 App 内登录不了 YouTube，可以先在 Safari 里登录，等后续版本支持 `--cookies-from-browser safari` 兜底（暂时还没接入 UI）。
- **进度条数据来自文件轮询而不是 yt-dlp 实时进度**。yt-dlp_macos 是 PyInstaller 打包的，stdout 在非 TTY 下深度块缓冲，`PYTHONUNBUFFERED` / `script` PTY 都救不了。轮询 `.part` 文件大小可以给出准确的百分比和速度，但比 yt-dlp 原生进度晚几百毫秒。
- **macOS Sequoia 15.1+ 强制至少 ad-hoc 签名才能开**。Intel Mac 也可以跑 universal 包，但 OS 的拦截更严格一点。

## 许可证

待定。内置的 `yt-dlp` 是 [Unlicense](https://github.com/yt-dlp/yt-dlp/blob/master/LICENSE)；`ffmpeg` 按编译版本不同是 LGPL/GPL。本仓库 App 代码目前未声明许可（all rights reserved）—— 需要补 License 的话开 issue。

## 链接

- 作者：[LitoTime](https://litotime.com)
- 仓库：[github.com/elsakane2015/YtbDownGUI](https://github.com/elsakane2015/YtbDownGUI)
