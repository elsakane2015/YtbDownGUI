# 动态账号登录开发 TODO

更新时间：2026-06-05

## 目标

把账号页从固定站点列表改成动态账号管理：

- 一个登录入口：点击后输入网址并打开内置 WebView 登录。
- 一个已登录卡片：列出已保存 cookies 的网站。
- 一个已登出卡片：列出移除 cookies 但保留记录的网站，可重新登录。
- 下载和分析视频时，自动为匹配的网站注入对应 cookies 给 yt-dlp。

原则上，只要 yt-dlp 支持该网站，并且登录态可以通过普通 HTTP cookies 表达，本软件就可以用这条链路支持登录下载。

## 重要边界

- yt-dlp 支持下载不代表一定能通过 WebView 登录成功；网站可能依赖验证码、设备验证、SSO、localStorage、IndexedDB、设备指纹或特殊请求头。
- 未知网站没有统一的 marker cookie，不能可靠自动判断登录完成；未知站点应由用户手动点击“完成登录”保存 cookies。
- “登出”先定义为“本应用移除该网站 cookies”，不承诺远端账号真正退出登录。
- DRM 内容仍然不支持，cookies 不能绕过 Widevine / FairPlay。
- cookies 是账号凭证，动态登录任意网站后需要避免保存或导出无关站点 cookies。

## P0 设计确认

- [x] 定义“已登录”的语义：未知网站默认表示“已保存 cookies”，不保证远端账号验证成功。
- [x] 决定未知站点登录完成方式：用户手动点“完成登录”；已知站点继续支持 marker cookie 自动完成。
- [x] 定义登出语义：先做“本应用移除 cookies”，不承诺远端网站真正退出登录。
- [x] 决定 UI 是否提示用户不要登录网银、邮箱等高度敏感网站：不在主流程增加阻断提示，先保留在文档边界说明里。

## P1 后端数据模型

- [x] 新增动态账号 registry，例如 `$APP_DATA/accounts.json`。
- [x] 账号字段包含：`account_id`、`display_name`、`login_url`、`primary_host`、`cookie_domains`、`status`、`cookie_count`、`updated_at`。
- [x] 保留现有固定站点配置，用作已知站点增强信息：登录 URL、marker cookie、关联域名、playlist 探测策略。
- [x] 兼容迁移现有 `cookies/<site_id>.json`，避免升级后老用户账号消失。
- [x] 明确 cookie 文件命名规则，避免动态 host 中的特殊字符影响路径安全。

## P1 URL 与域名匹配

- [x] 新增 URL 规范化：无协议时补 `https://`，只允许 `http` / `https`。
- [x] 实现 host/domain 匹配函数，避免 `badexample.com` 误匹配 `example.com`。
- [x] 支持从下载 URL 找到最合适账号：优先已知站点，其次动态账号，多个匹配用最长 domain 优先。
- [x] 支持一个账号包含多个 cookie domain。
- [x] 处理 `www.`、移动站子域名、短链域名和登录域名不同的情况。

## P1 Cookie 保存与导出

- [x] 改造 cookie 存储，不再只按固定 `site_id` 保存。
- [x] 保存 WebView cookies 时过滤到目标网站相关域，避免把无关站点 cookies 混进账号。
- [x] 处理 host-only cookie：如果 cookie domain 为空，保存时补当前 host。
- [x] 导出 `cookies.txt` 时只导出当前账号相关 cookies。
- [x] 继续设置文件权限；Windows 后续考虑 Credential Manager 或加密存储。
- [x] 避免把第三方 SSO 的通用身份站 cookies 无脑存进目标网站账号。

## P1 登录窗口

- [x] 支持 `start_login_by_url(url)`。
- [x] 已知站点：使用站点配置的登录 URL 和 marker cookie。
- [x] 未知站点：直接打开用户输入 URL，不启用 marker 自动完成。
- [x] 未知站点登录窗口需要“完成登录”和“取消”。
- [x] 登录成功后创建或更新 registry 账号，并保存 cookies。
- [x] 记录登录时 user agent，便于后续 yt-dlp 下载时复用。

## P1 下载 / 分析接入

- [x] 修改 `src-tauri/src/core/probe.rs`，从动态账号匹配 cookies。
- [x] 修改 `src-tauri/src/core/download.rs`，下载时自动注入匹配的 `cookies.txt`。
- [x] 已知站点仍保留 `use_flat_playlist` 策略。
- [x] 动态账号可记录登录 UA，后续下载时给 yt-dlp 加 `--user-agent`。
- [x] 代理设置需要和登录态风险一起考虑：登录 WebView 与 yt-dlp 下载 IP 不一致时，部分网站可能触发风控。

## P1 IPC

- [x] `list_accounts` 返回动态账号列表，并区分 `logged_in` / `logged_out`。
- [x] 新增 `start_login_by_url`。
- [x] 修改 `finish_login` 支持动态账号。
- [x] 修改 `logout`：删除 cookies，但保留 registry 记录，移动到已登出。
- [x] 修改 `export_cookies_netscape` 支持动态账号 ID。
- [x] 登录事件 payload 从单纯 `site_id` 升级为账号信息，方便前端刷新和展示。

## P1 前端账号页

- [x] 顶部改成一个“登录”按钮。
- [x] 点击后弹出 URL 输入框或内联输入区。
- [x] 登录中显示当前 URL、完成登录、取消。
- [x] “已登录卡片”列出所有有 cookies 的账号。
- [x] “已登出卡片”列出移除 cookies 但保留记录的网站。
- [x] 已登出卡片提供“重新登录”。
- [x] 已登录卡片保留“重新登录 / 导出 cookies.txt / 登出”。
- [x] 未知网站的状态文案用“已保存 cookies”，避免误导为远端账号已验证。

## P2 质量与测试

- [x] Rust 单测：URL 规范化、domain 匹配、账号选择优先级。
- [x] Rust 单测：host-only cookie 补 domain、Netscape 导出。
- [x] 迁移测试：旧 `youtube.json` / `bilibili.json` 能出现在新账号列表。
- [x] 前端构建测试：`pnpm build`。
- [ ] 手测：YouTube、Bilibili、一个未知 yt-dlp 支持网站。
- [ ] 手测：登出后进入已登出卡片，重新登录后回到已登录卡片。
- [ ] 手测：下载 URL 能自动使用对应动态 cookies。

## 实测记录

- [x] 2026-06-05：未知网站 `mooc1.chaoxing.com` 可通过账号页登录并保存 cookies，账号管理链路有效。
- [ ] 2026-06-05：`mooc1.chaoxing.com` 探测失败，yt-dlp 返回 `Unsupported URL`；该结果不能作为“未知 yt-dlp 支持网站”下载链路通过证据。

## 建议实施顺序

1. 先做账号 registry、URL/domain 匹配、cookie 保存和导出。
2. 再把 `probe.rs` 和 `download.rs` 接到动态账号匹配逻辑。
3. 然后改 IPC 和账号页 UI。
4. 最后做迁移、测试和安全细节。

不要先只改 UI。否则界面可能显示“已登录”，但下载和分析流程仍不会自动使用动态 cookies。
