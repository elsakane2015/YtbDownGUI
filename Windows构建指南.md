# Windows 构建指南

你不需要 Windows 机器、不需要装 Visual Studio、什么都不用装 —— **GitHub Actions** 替你跑构建。

> 本文档假设：你已经把代码推到了 [github.com/elsakane2015/YtbDownGUI](https://github.com/elsakane2015/YtbDownGUI)，并且本地装了 [GitHub CLI (`gh`)](https://cli.github.com)（macOS：`brew install gh`）。

## 它怎么工作的

仓库里有这两个文件，是 Windows 构建链路的全部：

- `.github/workflows/release-windows.yml` — GitHub Actions 配置，描述了在云端 Windows 机器上怎么打包
- `scripts/fetch-binaries-windows.ps1` — 拉取 yt-dlp.exe 和 ffmpeg.exe 的 PowerShell 脚本

触发后会自动：

1. 借一台 GitHub 提供的 Windows 服务器（免费）
2. 安装 Rust + Node + pnpm
3. 拉 yt-dlp.exe + ffmpeg.exe
4. 跑 `pnpm tauri build` 出 `YtbDownGUI.exe`
5. 把 .exe + sidecar + WebView2Loader.dll + portable.txt + README 打包成 zip
6. 上传 zip 到 GitHub（artifact 或 Release 附件）

**一次构建 = 10–15 分钟**，免费。

---

## 三种触发方式

### 方式 A：手动触发（最常用，调试时用这个）

**用网页：**
1. 打开 https://github.com/elsakane2015/YtbDownGUI/actions
2. 左侧栏点 **Release · Windows**
3. 右上角点 **Run workflow ▼**
4. **Branch**: 选 `main`（默认）
5. **Tag** 输入框：留空（不会上传到任何 Release，只生成 artifact 给你下载）
6. 点绿色的 **Run workflow** 按钮

**用命令行：**
```bash
# 在仓库目录下
gh workflow run release-windows.yml

# 看跑得怎么样
gh run list --workflow=release-windows.yml --limit 3
gh run view              # 看最新一次的进度
gh run watch             # 实时跟踪日志
```

10-15 分钟后构建完成。

**拿构建产物：**
```bash
# 列出最新一次跑的 artifacts
gh run download

# 或精确指定（artifact 名字会显示在 Actions 网页上）
gh run download <run-id> -n "YtbDownGUI-0.0.1-b005-windows-x64.zip"
```

下载下来就是个 zip，里面包含 `YtbDownGUI/` 文件夹（.exe + sidecar + portable.txt），传给 Windows 朋友解压双击即可。

---

### 方式 B：发布版本（推 tag 自动跑）

跟 macOS 的发布流程一致 —— 推一个形如 `v*-b*` 的 tag，workflow 自动构建并 **直接挂到对应的 GitHub Release**：

```bash
# 1. 本地先用 release.sh 跑 macOS 打包（这步会自动 .buildnumber +1）
bash scripts/release.sh
# 假设这次出了 b005

# 2. 推 tag
git tag v0.0.1-b005
git push origin v0.0.1-b005

# 3. 先创建 GitHub Release 把 macOS dmg 挂上去
gh release create v0.0.1-b005 \
  releases/v0.0.1-b005/YtbDownGUI_0.0.1_b005_universal.dmg \
  --title "YtbDownGUI v0.0.1 (Build 005)" \
  --notes "本次更新…"

# 4. 此时 tag push 已经触发了 Windows workflow，它会自动构建并 attach zip
#    到刚刚创建的 Release，无需你手动上传
gh run watch    # 等 ~10 分钟
```

完成后 GitHub Release 页面会同时有 `.dmg`（macOS）和 `.zip`（Windows）两个附件。

> 注意：第 4 步 workflow 只会"上传到对应 tag 的 Release"，所以**必须先 create Release 再等 workflow 跑完**。或者反过来：先 push tag、等 workflow 跑、然后再 create Release（不过 workflow 会因为没找到 release 跳过 upload，需要手动 `gh release upload v0.0.1-b005 <zip>` 补一次）。

---

### 方式 C：把 Windows .zip 上传到已存在的 Release

如果 Release 已经在，只想补一个 Windows .zip：

**网页**：方式 A 的步骤 5，把 **Tag** 输入框填上目标 tag（如 `v0.0.1-b005`），workflow 跑完会直接 attach。

**命令行**：
```bash
gh workflow run release-windows.yml -f tag=v0.0.1-b005
```

---

## 看构建跑得怎么样

### 网页
https://github.com/elsakane2015/YtbDownGUI/actions

- 黄色圆点：正在跑
- 绿色对勾：成功
- 红色叉：失败 → 点进去看哪一步红了

### 命令行
```bash
# 列出最近的运行
gh run list --workflow=release-windows.yml

# 实时跟踪当前正在跑的
gh run watch

# 看失败的某次具体日志
gh run view <run-id> --log-failed
```

---

## 常见问题

### 1. workflow 显示需要权限？
默认仓库 Actions 应该是开启的。如果手动触发显示权限不足：

仓库 → **Settings** → **Actions** → **General** → "Allow all actions and reusable workflows"。

### 2. 第一次跑 cargo build 超慢
首次会下载 + 编译所有 Rust 依赖（数百个 crate），可能 15+ 分钟。**第二次起 < 5 分钟**（依赖被 GitHub Actions 缓存命中，`Swatinem/rust-cache@v2` 已经配置好了）。

### 3. yt-dlp / ffmpeg 下载失败
通常是 GitHub Release 偶发抽风。重跑 workflow 一次就好：
```bash
gh run rerun <run-id>
```

### 4. 构建成功但 attach 到 Release 失败
原因：方式 B 第 4 步先后顺序问题，Release 还不存在。手动补：
```bash
# 1. 下载 workflow artifact
gh run download <run-id>
# 2. 上传到目标 release
gh release upload v0.0.1-b005 YtbDownGUI-0.0.1-b005-windows-x64.zip
```

### 5. Windows 朋友打开 zip 后双击 .exe，提示 "SmartScreen 已阻止"
**这是正常的**，因为我们是 ad-hoc / 未签名应用。朋友需要：

1. 点提示框里的 **更多信息**
2. 出现 **仍要运行** 按钮，点它

之后这台机器上就不会再问了。

要彻底没这个提示需要花 ~200 美元/年买 Windows 代码签名证书 + EV 证书（升级到不再被 SmartScreen 拦截还更贵），对自用工具来说不值得。

### 6. 朋友机器太老（Win10 早期版本 / Win7/Win8）打不开
我们的最低支持是 **Windows 10 1809+**（2018 年 10 月版本之后）。原因是 Tauri 用的 WebView2 运行时需要这个。再老的 Windows 需要朋友自己装 [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) 才能跑。

---

## 文件清单

零依赖打包的 zip 解开后长这样：

```
YtbDownGUI/
├── YtbDownGUI.exe              主程序，universal x86_64 二进制
├── yt-dlp.exe                  捆绑的下载引擎
├── ffmpeg.exe                  捆绑的转码工具
├── WebView2Loader.dll          Tauri 运行时
├── resources/
│   └── Credits.rtf             About 面板用，Windows 上无效果但留着
├── portable.txt                便携模式开关 —— 删掉则数据存系统 %APPDATA%
└── README.txt                  双语简短说明
```

`portable.txt` 存在时（默认）：用户数据写在 `<zip解压目录>/YtbDownGUI/data/`，**整个文件夹拷到 U 盘里数据也跟着走**。

`portable.txt` 删掉后：用户数据写在 `C:\Users\<名字>\AppData\Roaming\com.litotime.ytbdowngui\`，跟常规 Windows 应用一致。

---

## 我（开发者）应该做的事

```
日常 ─────────────────────────────────────────────
        改代码 → 本地 macOS 测试 → commit & push
        到 main 分支

发布 ─────────────────────────────────────────────
   1.   本地：bash scripts/release.sh
        （自动 .buildnumber +1，出 macOS dmg）

   2.   git tag v0.0.1-bXXX && git push --tags

   3.   gh release create v0.0.1-bXXX \
            releases/v0.0.1-bXXX/*.dmg \
            --title "..." --notes "..."

   4.   等 ~10 分钟，回 GitHub Release 页面，
        Windows zip 应该自动 attach 上去了
```

完事。两个平台一次搞定。

---

## 进一步阅读

- GitHub Actions 基础：https://docs.github.com/zh/actions
- `gh` CLI 文档：https://cli.github.com/manual/
- Tauri 跨平台构建：https://v2.tauri.app/distribute/
