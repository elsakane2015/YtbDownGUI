# YtbDownGUI

macOS GUI 视频下载器，基于 [yt-dlp](https://github.com/yt-dlp/yt-dlp) + [ffmpeg](https://ffmpeg.org/)，用 Tauri v2 (Rust + React) 构建。

## 核心特性

- 内嵌 WebView 登录目标站点，自动接管 cookies → 下载到高清版本
- 支持单视频 / 播放列表 / 频道批量下载
- 精细化流选择：组合流 / 视频+音频分选 / 仅音频（可转 mp3）
- 字幕：多语言独立 srt 或嵌入 mp4，可抓 YouTube 自动字幕
- 优先支持 YouTube、Bilibili，结构上可扩展到其他 yt-dlp 支持站点

## 状态

> 项目初始化中。详见 [实施计划](https://github.com/elsakane2015/YtbDownGUI/tree/main/docs)（待补）。

## 开发

要求：Node 20+、Rust 1.85+、pnpm。

```bash
pnpm install
pnpm tauri dev
```

## 许可

待定。
