# Pro Development Plan

## 1. Scope

本计划基于 `Pro_sub.md`，用于推进 YtbDown Pro 一次性买断授权开发。

涉及两个仓库：

- 客户端公仓：`/Users/xue/Documents/vscode/YtbDown`
  - GitHub：`https://github.com/elsakane2015/YtbDownGUI.git`
  - 分支：`pro-dev`
  - 职责：Tauri / React 客户端、下载额度展示、Pro 激活 UI、本地 token 验签、下载入队限制。
- 服务端私仓：`/Users/xue/Documents/vscode/ytbdown-license-server`
  - GitHub：`https://github.com/elsakane2015/ytbdown-license-server.git`
  - 分支：`main`
  - 职责：Stripe webhook、license key 生成、邮件发送、设备激活、token 签发、免费额度服务端计数。

开发原则：

- 客户端不保存 Stripe secret、邮件 API key、token 签名私钥。
- 服务端是授权事实来源。
- 客户端只能内置 token 验签公钥和 License Server URL。
- 免费额度最终以服务端计数为准，本地只缓存。
- Pro 是一次性买断，license 永久有效；客户端 token 有效期 7 天。
- 免费版和 Pro 使用同一个 App、同一个 Tauri `identifier`、同一个安装包；Pro 只通过 license 解锁，不另做 Pro App。

## 2. Final Product Decisions

- 购买方式：Stripe 一次性付款，不做订阅。
- 用户体系：不做账号密码，使用购买邮箱 + license key。
- License key 格式：`YTB-XXXX-XXXX-XXXX-XXXX`。
- Pro 设备数：默认 3 台活跃设备。
- 自动换机：
  - 存在 30 天未使用设备时，自动踢出最久未使用设备。
  - 3 台设备都近期活跃时，发送购买邮箱验证码。
- 免费额度：终身 10 个成功下载。
- 免费额度防重装重置：服务端按 `installation_id` 计数。
- `device_id` / `installation_id`：系统安全存储为准，`$APP_DATA` 只缓存。
- Pro token：
  - Ed25519 签名。
  - 7 天有效期。
  - 服务端故障时允许一次 24 小时 emergency grace。
- 服务端离线：
  - 已激活且 token 未过期的 Pro 用户继续可用。
  - 新激活必须等服务端恢复。
  - 免费用户只可用本地缓存额度，缓存用完必须联网同步。
  - 首次启动且没有免费额度缓存时，必须联网初始化额度；服务端不可用时不允许免费下载。

## 3. Recommended Technical Stack

服务端私仓建议使用：

- Runtime：Node.js 22 + TypeScript。
- HTTP：Fastify。
- DB：PostgreSQL。
- ORM / migrations：Prisma。
- Email：Resend。
- Stripe：官方 `stripe` Node SDK。
- Stripe API version：在服务端代码中固定，避免默认版本变化影响 webhook payload。
- Token signing：Ed25519，使用 `jose` 或 `@noble/ed25519`。
- Tests：Vitest。
- Local dev：Docker Compose 启动 PostgreSQL。
- Production deploy：优先使用现有 VPS，跑 Node.js 服务 + PostgreSQL + Caddy 反向代理。

部署建议：

- 有 VPS 时，v1 不建议自部署完整 Supabase。Supabase 自部署组件多，维护成本比本项目需要的 license server 高。
- v1 推荐在 VPS 上自部署 PostgreSQL，配合 Prisma migration 和定期备份。
- 如果后续不想维护数据库，再迁到 Supabase Cloud / Neon 更合适。
- VPS 上建议使用 Docker Compose 管理 `api`、`postgres`、`caddy`，并用系统定时任务或对象存储做数据库备份。
- Caddy 负责 HTTPS 自动签发和续期，减少证书维护成本。

客户端继续使用现有技术：

- Tauri v2。
- Rust backend。
- React frontend。
- Rust key storage：`keyring` crate。
- Rust token verification：Ed25519 verifier crate，例如 `ed25519-dalek`。

## 4. Milestone Overview

进度规则：

- `[ ]` 未开始。
- `[~]` 进行中。
- `[x]` 完成。
- 每完成一项开发或验证，就在本文件里勾选对应 TODO。

### Milestone 0: Repo and Contract Setup

目标：先固定联动接口，避免客户端和服务端各写各的。

服务端：

- [x] 初始化 TypeScript / Fastify / Prisma 项目。
- [x] 增加 `.env.example`，不得提交真实密钥。
- [x] 增加 `docs/api.md` 或 OpenAPI spec。
- [x] 增加 health endpoint：`GET /healthz`。
- [x] 在 API contract 中明确所有错误响应格式：`code`、`message`、可选 `details`。
- [x] 增加隐私政策 / 服务条款最小页面或文档，说明邮箱、设备名、IP、安装 ID、激活日志的用途。
- [x] 增加 Caddy 反代真实 IP 处理：传递 `X-Forwarded-For` / `X-Real-IP`，Fastify 只信任可信反代。

客户端：

- [x] 在 `Pro_sub_plan.md` 留存计划。
- [x] 继续在 `pro-dev` 分支开发。
- [ ] 增加后续所需环境变量设计，但不接入真实服务端前不写死生产 URL。
- [x] 确认免费版和 Pro 不拆成两个 App，不修改现有 Tauri `identifier`。

验收：

- [x] 服务端可本地启动。
- [x] `/healthz` 返回 `200`。
- [x] API contract 文件存在。

### Milestone 1: License Server Core

目标：服务端先具备 license、设备、token、邮件的核心能力。

服务端数据表：

- [ ] `licenses`
- [ ] `devices`
- [ ] `license_tokens`
- [ ] `stripe_events`
- [ ] `device_transfer_events`
- [ ] `free_quota_installations`
- [ ] `free_quota_reservations`
- [ ] `email_events`
- [ ] `audit_logs`

服务端核心模块：

- [ ] `config`
- [ ] `db`
- [ ] `crypto`
- [ ] `license-key`
- [ ] `entitlement-token`
- [ ] `email`
- [ ] `rate-limit`
- [ ] `audit-log`
- [ ] `support`

API：

- [ ] `POST /v1/licenses/activate`
- [ ] `POST /v1/licenses/refresh`
- [ ] `POST /v1/licenses/deactivate`
- [ ] `POST /v1/licenses/send-transfer-code`
- [ ] `POST /v1/licenses/activate-with-transfer-code`
- [ ] `POST /v1/licenses/resend`

验收：

- [ ] 可手动 seed 一个 active license。
- [ ] App 或 curl 输入 license key 可激活设备。
- [ ] token payload 包含 `license_id`、`device_id`、`plan`、`iat`、`exp`。
- [ ] 同一设备再次激活只刷新 token，不新增设备。
- [ ] 第 4 台设备触发自动迁移或验证码流程。

### Milestone 2: Stripe Fulfillment

目标：Stripe 一次性付款后自动生成 license 并发邮件。

服务端 API：

- [ ] `POST /v1/billing/create-checkout-session`
- [ ] `GET /v1/billing/checkout-status`
- [ ] `GET /billing/success`
- [ ] `GET /billing/cancel`
- [ ] `POST /v1/webhooks/stripe`

Stripe events：

- [ ] `checkout.session.completed`
- [ ] `checkout.session.async_payment_succeeded`
- [ ] `checkout.session.async_payment_failed`
- [ ] `charge.refunded`
- [ ] `charge.dispute.created`

实现要求：

- [ ] Checkout 使用 Stripe Price `lookup_key=ytbdown_pro_lifetime_current` 查找当前有效价格，不把具体 price id 写死进业务逻辑。
- [ ] Checkout Session 配置 `success_url=/billing/success?session_id={CHECKOUT_SESSION_ID}` 和 `cancel_url=/billing/cancel`。
- [ ] `/billing/success` 只展示“购买成功，激活码将发送到邮箱”，不承担发码逻辑。
- [ ] `/billing/cancel` 展示“购买已取消，可回到 App 重新购买”。
- [ ] Stripe SDK 初始化时固定 API version。
- [ ] webhook 必须验签。
- [ ] `/v1/webhooks/stripe` 必须使用 raw request body + `Stripe-Signature` 验签，不得在验签前 JSON parse。
- [ ] webhook 必须幂等。
- [ ] 只对已支付成功的一次性 Checkout 创建 license。
- [ ] 同一个 `checkout_session_id` 不得生成多个 license。
- [ ] license key 只在邮件和首次履约中明文出现。
- [ ] DB 保存 license key hash。
- [ ] license key 查询 hash 使用 `HMAC-SHA256(license_key, LICENSE_KEY_HASH_SECRET)`，不使用普通裸 hash。
- [ ] 全额退款禁用 license。
- [ ] 部分退款只记录审计日志。

验收：

- [ ] Stripe test mode 完成支付后，服务端创建 license。
- [ ] Stripe 成功页和取消页能正常打开。
- [ ] 成功页即使打开失败，也不影响 webhook 发码。
- [ ] 邮件发送成功并记录 `email_events`。
- [ ] 重放同一个 webhook 不重复创建 license。
- [ ] webhook body 被提前解析时的验签失败场景有测试覆盖。
- [ ] 支付成功但邮件未收到时，可通过 `checkout-status` 或 `licenses/resend` 恢复。
- [ ] 全额退款后 refresh/activate 不再返回 Pro token。

### Milestone 3: Free Quota Service

目标：免费 10 次额度由服务端计数，避免普通重装重置。

服务端 API：

- [ ] `POST /v1/free-quota/status`
- [ ] `POST /v1/free-quota/reserve`
- [ ] `POST /v1/free-quota/confirm`
- [ ] `POST /v1/free-quota/release`

核心规则：

- [ ] `installation_id` 是免费额度主键。
- [ ] 默认 limit = 10。
- [ ] reserve 可一次预留多个下载名额，用于批量下载。
- [ ] confirm 只确认成功下载。
- [ ] release 释放失败或取消下载的预留。
- [ ] reservation 必须有过期时间，避免客户端崩溃后永久占用额度。
- [ ] reserve 返回 `reservation_id`，confirm/release 必须携带 `reservation_id`。
- [ ] reserve/confirm/release 必须幂等，重复请求不能重复扣减或重复释放。
- [ ] 对 IP + `installation_id` 做基础限流，降低脚本刷免费额度接口风险。

验收：

- [ ] 新 `installation_id` 剩余额度为 10。
- [ ] 首次启动且没有本地额度缓存时，必须联网初始化免费额度。
- [ ] 首次启动且服务端不可用时，免费下载被拒绝并显示需要联网同步。
- [ ] 成功 confirm 10 次后，第 11 次 reserve 被拒绝。
- [ ] 失败或取消 release 后额度恢复。
- [ ] 过期 reservation 可被后台任务清理。
- [ ] 重复 confirm 同一个 `reservation_id` 只扣一次。
- [ ] 重复 release 同一个 `reservation_id` 只释放一次。

### Milestone 4: Client Entitlement Foundation

目标：客户端具备 Pro 状态、本地 token 验签、系统安全存储和服务端通信能力。

Rust 新增模块：

- [ ] `src-tauri/src/core/entitlement.rs`
- [ ] `src-tauri/src/commands/entitlement.rs`

Rust 新增依赖：

- [ ] `keyring`
- [ ] `ed25519-dalek` 或同等 Ed25519 verifier
- [ ] 如需 HTTP 调用复用现有 `reqwest`

本地状态：

- 系统安全存储：
  - `device_id`
  - `installation_id`
- `$APP_DATA/entitlement.json`：
  - `license_id`
  - `license_email`
  - `license_key_last4`
  - `signed_token`
  - `token_expires_at`
  - `trial_used_count_cache`
  - `trial_remaining_count_cache`
  - `emergency_grace_used_for_token`

IPC：

- [ ] `get_entitlement_status`
- [ ] `activate_pro`
- [ ] `refresh_pro`
- [ ] `deactivate_pro`
- [ ] `send_transfer_code`
- [ ] `activate_with_transfer_code`

验收：

- [ ] 首次启动生成并持久化 `device_id` / `installation_id`。
- [ ] 删除 `$APP_DATA/entitlement.json` 后，系统安全存储里的 id 仍可恢复。
- [ ] 系统安全存储不可用时 fallback 到 `$APP_DATA`，并记录 `secure_storage_available=false`。
- [ ] 有效 signed token 本地验签通过时显示 Pro。
- [ ] token 未过期但服务端不可用时，Pro 仍可用。
- [ ] token 过期后触发 refresh 或 emergency grace。

### Milestone 5: Client Download Enforcement

目标：下载限制必须由 Rust 后端强制执行，前端只做提示。

修改点：

- [ ] `commands::download::enqueue_download`
- [ ] `commands::download::enqueue_batch`
- [ ] `QueueManager` job terminal state handling

规则：

- [ ] Pro token 有效：允许入队，不消耗免费额度。
- [ ] 免费单视频：入队前 reserve 1 个额度。
- [ ] 免费批量：入队前 reserve `entries.len()` 个额度。
- [ ] 批量超过剩余额度：整个批次拒绝。
- [ ] Job `Done`：confirm 1 个额度。
- [ ] Job `Failed` / `Canceled`：release 1 个额度。
- [ ] Job `Skipped`：默认 release，不消耗额度。
- [ ] App 重启后，恢复 job 历史时释放不再活动的 reservation。

错误结构：

```json
{
  "code": "quota_exceeded",
  "message": "免费版最多可下载 10 个视频，激活 Pro 后可解除限制。"
}
```

验收：

- [ ] 免费剩余 0 时，单视频入队失败。
- [ ] 免费剩余 2 时，批量 3 个入队失败且没有创建任何 job。
- [ ] 下载失败不消耗额度。
- [ ] 下载取消不消耗额度。
- [ ] 文件已存在 skipped 不消耗额度。
- [ ] 直接调用 Tauri invoke 也无法绕过限制。

### Milestone 6: Client UI

目标：用户能购买、激活、查看状态、处理换机验证码。

`src/lib/ipc.ts`：

- [ ] 增加 entitlement 类型。
- [ ] 增加 entitlement invoke wrappers。

`src/pages/SettingsPage.tsx`：

- [ ] 增加 Pro 区域。
- [ ] 显示当前 plan、license email、设备数、token 到期时间。
- [ ] 输入 license key 激活。
- [ ] 退出激活。
- [ ] 激活返回需要验证码时，显示验证码输入框。
- [ ] 购买 Pro 按钮打开 Checkout URL。
- [ ] 找回激活码入口，调用 resend API。

`src/pages/DownloadsPage.tsx`：

- [ ] 显示免费额度：`已用 X / 10` 或 `剩余 X / 10`。
- [ ] Pro 用户显示 `Pro 已激活`。
- [ ] 批量选择超过免费额度时提前提示。
- [ ] 后端错误 `quota_exceeded` 显示明确升级提示。

验收：

- [ ] 免费用户能看到剩余额度。
- [ ] Pro 用户不再显示限制焦虑信息，只显示已激活状态。
- [ ] 激活失败原因能区分：无效 key、设备满且需要验证码、验证码错误、网络失败、license 被禁用。
- [ ] 服务端不可用时 UI 能说明当前离线授权剩余时间。

### Milestone 7: End-to-End Integration

目标：完整支付到激活到下载链路跑通。

联调路径：

- [ ] 启动本地 License Server。
- [ ] 配置 Stripe test mode webhook。
- [ ] 客户端配置 staging License Server URL 和公钥。
- [ ] 在客户端点击购买 Pro。
- [ ] Stripe test card 支付成功。
- [ ] 服务端 webhook 创建 license。
- [ ] 邮件服务 test mode 或真实测试邮箱收到 license key。
- [ ] 客户端输入 license key 激活。
- [ ] 下载超过 10 个视频仍可继续。
- [ ] 模拟全额退款后，token 过期或 refresh 后降级。

验收：

- [ ] 端到端流程无需手工改 DB。
- [ ] webhook 重放安全。
- [ ] 断网时未过期 token 可用。
- [ ] 设备满额自动迁移和验证码迁移均可验证。

### Milestone 8: Release Preparation

目标：准备正式上线。

服务端：

- [ ] 部署 production License Server 到 VPS。
- [ ] 配置 production PostgreSQL。
- [ ] 配置 Caddy 反向代理和 HTTPS 自动续期。
- [ ] 配置 Caddy 真实 IP 转发，并验证应用层限流读取到客户端 IP。
- [ ] 配置 Stripe live webhook。
- [ ] 配置 Resend production sender domain。
- [ ] 配置 `ytbdown@litotime.com` 发信域名验证和收信转发。
- [ ] 配置日志、错误监控和数据库备份。
- [ ] 配置防火墙，仅开放 80、443、SSH。
- [ ] 配置 PostgreSQL 自动备份和一次恢复演练。
- [ ] 配置日志轮转，避免磁盘被日志打满。
- [ ] 配置 `/healthz` 监控。
- [ ] 生成 production Ed25519 key pair。
- [ ] 私钥只进服务端环境变量或 secret manager。

客户端：

- [ ] 切换 production License Server URL。
- [ ] 内置 production 公钥。
- [ ] 保持现有 Tauri `productName` / `identifier`，不另做 Pro 独立安装包。
- [ ] 购买 / 激活 UI 中提供隐私政策和支持邮箱入口。
- [ ] 更新版本号和 release notes。
- [ ] 构建 macOS / Windows 包。

验收：

- [ ] Stripe live mode 小额真实购买测试成功。
- [ ] license 邮件可达。
- [ ] App 激活 production license 成功。
- [ ] 退款禁用路径验证成功。

## 5. API Contract Summary

### Entitlement Status

客户端统一使用以下状态结构：

```json
{
  "plan": "free",
  "license_email": null,
  "license_key_last4": null,
  "token_expires_at": null,
  "offline_grace_expires_at": null,
  "trial_limit": 10,
  "trial_used_count": 0,
  "trial_remaining_count": 10,
  "server_reachable": true,
  "message": null
}
```

Pro 示例：

```json
{
  "plan": "pro",
  "license_email": "user@example.com",
  "license_key_last4": "ABCD",
  "token_expires_at": "2026-06-02T00:00:00Z",
  "offline_grace_expires_at": null,
  "trial_limit": 10,
  "trial_used_count": 10,
  "trial_remaining_count": 0,
  "server_reachable": true,
  "message": null
}
```

### Activation Result

```json
{
  "kind": "activated",
  "status": {
    "plan": "pro"
  }
}
```

需要验证码：

```json
{
  "kind": "transfer_code_required",
  "challenge_id": "trc_xxx",
  "email_hint": "u***@example.com",
  "expires_at": "2026-05-26T12:10:00Z"
}
```

### Error Codes

客户端需要识别这些错误码：

- `quota_exceeded`
- `license_invalid`
- `license_disabled`
- `license_refunded`
- `device_limit_reached`
- `transfer_code_required`
- `transfer_code_invalid`
- `transfer_rate_limited`
- `server_unreachable`
- `token_expired`

## 6. Local Development Workflow

### Client

```bash
cd /Users/xue/Documents/vscode/YtbDown
git checkout pro-dev
pnpm install
pnpm tauri dev
```

### Server

```bash
cd /Users/xue/Documents/vscode/ytbdown-license-server
pnpm install
docker compose up -d
pnpm prisma migrate dev
pnpm dev
```

### Stripe Webhook Local Test

```bash
stripe listen --forward-to localhost:3000/v1/webhooks/stripe
```

`.env.example` must include placeholders only:

```text
DATABASE_URL=
STRIPE_SECRET_KEY=
STRIPE_WEBHOOK_SECRET=
STRIPE_API_VERSION=
STRIPE_PRICE_LOOKUP_KEY_PRO_LIFETIME=ytbdown_pro_lifetime_current
RESEND_API_KEY=
RESEND_FROM_EMAIL=ytbdown@litotime.com
SUPPORT_EMAIL=ytbdown@litotime.com
TOKEN_PRIVATE_KEY=
TOKEN_PUBLIC_KEY=
TOKEN_KEY_ID=
LICENSE_KEY_HASH_SECRET=
APP_PUBLIC_URL=
PRIVACY_URL=
TERMS_URL=
```

## 7. Testing Matrix

### Server Unit Tests

- [ ] license key generation uniqueness and format.
- [ ] license key hash lookup.
- [ ] token signing and verification.
- [ ] activation limit.
- [ ] idle-device auto migration.
- [ ] active-device transfer code flow.
- [ ] webhook idempotency.
- [ ] Stripe raw body signature verification.
- [ ] full refund disables license.
- [ ] partial refund does not disable license.
- [ ] free quota reserve / confirm / release.
- [ ] free quota reservation idempotency.
- [ ] reservation expiration cleanup.
- [ ] license key HMAC lookup.
- [ ] token `kid` key rotation support.
- [ ] Stripe API version is pinned in SDK initialization.
- [ ] Caddy / trusted proxy IP handling can be configured safely.

### Client Unit Tests

- [ ] entitlement status parsing.
- [ ] token expiry handling.
- [ ] emergency grace calculation.
- [ ] quota error mapping.
- [ ] download state to quota confirm/release mapping.

### Integration Tests

- [ ] activate seeded license.
- [ ] refresh valid token.
- [ ] deactivate device.
- [ ] activate 4th device with idle device replacement.
- [ ] activate 4th device with transfer code.
- [ ] free quota 10 successful downloads then reject.
- [ ] server unavailable with unexpired token.
- [ ] server unavailable with expired token and grace used.

### Manual QA

- [ ] First launch creates stable device id.
- [ ] First launch without quota cache and without server rejects free download with a clear message.
- [ ] Reinstall or delete `$APP_DATA` does not change secure-storage id.
- [ ] Keychain/Credential Manager unavailable fallback is visible in logs/status.
- [ ] Purchase button opens Checkout.
- [ ] Stripe success page and cancel page are readable.
- [ ] License email content is readable.
- [ ] Settings Pro panel states are clear.
- [ ] Download page quota display updates after success/cancel/failure.
- [ ] Privacy/support links open correctly.

## 8. Rollout Checklist

- [x] `Pro_sub_plan.md` committed to client `pro-dev`.
- [x] Server repository initialized with TypeScript/Fastify/Prisma.
- [x] API contract committed in both repos or linked from server docs.
- [ ] Stripe test product and price created.
- [ ] Stripe Price lookup key `ytbdown_pro_lifetime_current` configured.
- [ ] Stripe test webhook configured.
- [ ] Email provider test sender verified.
- [ ] `ytbdown@litotime.com` send domain and receive forwarding verified.
- [ ] Minimal privacy/terms pages published or hosted by License Server.
- [ ] Staging License Server deployed.
- [ ] Client staging build points to staging server.
- [ ] End-to-end test purchase succeeds.
- [ ] Refund path tested.
- [ ] Device migration path tested.
- [ ] Free quota path tested.
- [ ] Production keys generated.
- [ ] Production server deployed.
- [ ] Client production public key embedded.
- [ ] Release build tested on macOS and Windows.

## 9. Implementation Order

Recommended order:

1. Service skeleton and DB schema.
2. License key and token signing.
3. Manual license activation without Stripe.
4. Device activation and migration.
5. Free quota APIs.
6. Stripe Checkout and webhook fulfillment.
7. Email sending and resend.
8. Client entitlement store and token verification.
9. Client download enforcement.
10. Client Pro UI.
11. Full E2E integration.
12. Production rollout.

Reasoning:

- 服务端核心先完成，客户端才能对真实 API 联调。
- 先做 manual seeded license，可以不等 Stripe 就验证客户端激活和 token。
- 免费额度和下载限制必须在客户端 Rust 后端接入，最后再做 UI polish。

## 10. Open Items To Decide Before Coding

以下事项已转成默认方案，除非后续明确改动，否则按这些推进：

- [x] Pro 价格不硬编码在客户端或服务端业务逻辑中。
  - 实际价格在 Stripe Dashboard 中配置。
  - 服务端使用 Stripe Price lookup key：`ytbdown_pro_lifetime_current`。
  - 以后调价在 Stripe 控制台创建新 Price，并切换 lookup key 指向当前价格，不需要发新版客户端。
- [x] 邮件服务默认使用 Resend。
- [x] 服务端部署平台默认使用现有 VPS。
  - 推荐 Docker Compose 部署 Node.js API、PostgreSQL、反向代理。
  - 反向代理使用 Caddy，自动 HTTPS。
- [x] 数据库默认使用 VPS 自部署 PostgreSQL。
  - 不建议 v1 自部署完整 Supabase，维护面太大。
  - 如需 Supabase，优先考虑 Supabase Cloud，而不是自部署 Supabase。
  - 当前项目只需要 PostgreSQL，Prisma 已足够覆盖迁移和查询。
- [x] 客户端生产 License Server URL 已确定：`https://license.ytbdown.litotime.com`。
  - 该域名需要指向 VPS 的 License Server。
  - 客户端发布前会内置这个 URL。
- [x] 支持邮箱和异常申诉入口已确定：`ytbdown@litotime.com`。
  - 该邮箱已配置收信转发。
  - 邮件模板中放这个支持邮箱。
  - 退款争议、设备迁移超限、误封、找回激活码失败都引导到该邮箱。
  - v1 不做完整客服后台，先用邮件 + audit log 处理。
- [x] v1 不采硬件指纹。
  - 免费额度依赖系统安全存储 + 服务端计数防普通重装重置。
  - 刻意删除系统凭据可能重置免费额度，暂时接受该取舍。
- [x] 首次免费额度必须联网初始化。
  - 没有本地额度缓存且服务端不可用时，不允许免费下载。
- [x] Stripe 成功 / 取消页面由 License Server 提供。
  - `/billing/success?session_id=...`
  - `/billing/cancel`
  - 发码仍只由 webhook 履约负责。

如果没有额外决定，开发时按本文默认值推进。

## 11. Progress Summary

- [x] 当前免费版已用 tag 保存：`v0.1.0-free`。
- [x] 客户端 Pro 开发分支已创建：`pro-dev`。
- [x] 客户端已记录方案：`Pro_sub.md`。
- [x] 服务端私仓已创建并记录方案：`Pro_sub.md`。
- [x] 客户端开发计划已创建：`Pro_sub_plan.md`。
- [x] 服务端项目骨架已初始化。
- [ ] 客户端授权代码尚未实现。
- [ ] Stripe test mode 尚未接入。
