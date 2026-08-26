# YtbDown iOS

这是仓库根目录下独立维护的原生 SwiftUI/Xcode 工程。它不依赖 macOS/Windows
桌面端，也不使用中转服务器：页面解析、媒体下载和音视频合并都在 iPhone 内完成。

## 首次打开与真机调试

当前电脑已经恢复了 Python 运行时，可以直接打开：

```bash
open ios/YtbDownIOS.xcodeproj
```

从新的 Git clone 开始时，先执行：

```bash
./ios/scripts/fetch-python-runtime.sh
cd ios && xcodegen generate
open YtbDownIOS.xcodeproj
```

然后在 Xcode 中：

1. 打开 **Xcode → Settings → Accounts**，登录自己的 Apple 账户。
2. 进入 YtbDownIOS target 的 **Signing & Capabilities**，选择自己的 Team。工程当前的
   Bundle ID 是 `com.litotime.ytbdowngui.ios`；若签名冲突，改成自己的唯一标识。
3. 用数据线或同一网络连接 iPhone，在运行设备列表中选择它。
4. iPhone 首次调试需在 **设置 → 隐私与安全性 → 开发者模式** 中开启开发者模式，
   按提示重启并确认。
5. 点击 Xcode 的运行按钮。之后下载文件位于 **文件 → 我的 iPhone → YtbDown**。

只有 `project.yml` 改动或在新 clone 中恢复工程时才需要重新执行 `xcodegen generate`。

## 当前能力

- App 内嵌 CPython 3.13、yt-dlp 和 JavaScriptCore，不启动外部进程。
- 已实测公开 YouTube 页面：列出清晰度并按用户选择解析真实媒体地址。
- 支持 yt-dlp 能匿名解析、且返回普通 HTTP(S) 媒体流的其他网站；具体兼容性取决于站点。
- 支持 H.264/HEVC 视频与 AAC 音频，分离轨道使用 AVFoundation 在 iPhone 本地合并为 MP4。
- YouTube 使用内嵌 Python 网络栈下载并通过文件大小轮询显示进度；其他网站使用 iOS 后台 URLSession。任务记录会持久保留。
- 可在“设置”中选择把新下载保存到系统“照片”App，或者保存到“文件”App →“我的 iPhone”→“YtbDown”；YtbDown 文件夹中的成品也可从“任务”页分享。
- YouTube 的 GoogleVideo 直链会绑定网络出口 IP；为避免 VPN/代理使内嵌解析器与 iOS 系统下载走不同出口而造成 HTTP 403，YouTube 的解析与传输统一使用内嵌 Python 网络栈。YouTube 下载期间需保持 App 在前台，其他网站仍使用 iOS 后台传输。

## 目前需要注意

- **不是离线下载器**：不经过自建服务器，但 iPhone 仍会直接访问视频来源网站。
- **画质通常最高约 1080p**：很多 2K/4K 只有 VP9、AV1 或 Opus，系统原生合并器不能替代
  ffmpeg 处理这些组合；当前会过滤掉不能可靠合并的格式。
- **暂未做网站登录**：年龄限制、私密内容、登录墙和部分地区限制内容会失败。后续应增加
  内嵌网页登录和 Cookie 导入，不能直接照搬桌面 WebView 登录代码。
- 暂不支持 DRM、播放列表批量、字幕、纯音频选择和需要 HLS/DASH 重封装的特殊来源。
- App 调试包约 90 MB，主要来自 Python 标准库和解析器；Release 包会小一些。
- 非 YouTube 下载可在后台继续；YouTube 下载及超大文件的本地合并阶段应保持 App 在前台。
- 视频网站经常改变接口，内嵌 yt-dlp 与 YouTube JS/PO Token 适配层需要定期更新。

## 不上架 App Store 时的签名

- 本工程已经固定为付费团队 `HCDD4TQBT8`，使用 **Apple Development + Automatic Signing**，
  只用于已登记 iPhone 的本地安装和 Xcode 调试，不包含 App Store 上传或发布步骤。
- 2026-08-26 已完成一次签名真机构建验证；本机生成的 Provisioning Profile 到期时间是
  **2027-03-30**，因此不需要每 7 天重新安装。到期前再用 Xcode 构建安装一次即可续期。
- Provisioning Profile 和证书由 Apple/Xcode 保存在本机与开发者账户中，不应提交到 Git。
- 只有改用免费 Apple 账户的 **Personal Team** 时，才会遇到 7 天过期、10 个 App ID、
  每个平台 3 台设备和每台设备 3 个 App 的限制。
- iOS 16 及以上运行开发签名 App 必须开启“开发者模式”。

Apple 官方说明：
[会员与 Personal Team 限制](https://developer.apple.com/support/compare-memberships/)、
[开启开发者模式](https://developer.apple.com/documentation/xcode/enabling-developer-mode-on-a-device)、
[在真机运行 App](https://developer.apple.com/documentation/xcode/running-your-app-on-simulated-or-physical-devices)。

## 第三方许可

iOS 本地解析桥接层基于 GPLv3 的 Keraunos 方案改造；本项目为自用没有障碍，但如果把 iOS App
发给其他人，即使不经过 App Store，也应同时按 GPLv3 提供对应 iOS 源码与许可说明。详情见
[`THIRD_PARTY_NOTICES.md`](./THIRD_PARTY_NOTICES.md) 和
[`ThirdParty/Keraunos/LICENSE`](./ThirdParty/Keraunos/LICENSE)。这也是 iOS 代码与桌面代码保持
独立目录的重要原因之一。
