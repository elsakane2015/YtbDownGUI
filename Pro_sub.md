# Pro Subscription and License Plan

## 1. Goal

为 YtbDown 增加 Pro 订阅和下载限制：

- 免费版终身只能成功下载 10 个视频。
- Pro 解除下载数量限制。
- 使用 Stripe 收款。
- 支付成功后由自建服务端生成 license key，并通过邮件发送给用户。
- 用户无需注册账号，使用购买邮箱 + license key 激活。
- 默认每个 license 支持 3 台设备。
- 旧设备不在身边时，支持自动迁移，减少人工处理。

核心原则：Stripe 负责支付和订阅状态；我们自己的 License Server 负责序列号、设备激活、离线 token、设备迁移和客户端授权判断。

## 2. Product Rules

### 免费版

- 免费额度：终身 10 个成功下载。
- 单视频成功下载算 1 个。
- 批量下载中每个成功条目算 1 个。
- 下载失败、取消不消耗额度。
- 文件已存在并进入 `Skipped` 状态时，默认不消耗额度。
- 第 11 个成功下载前，后端必须拒绝入队或拒绝开始下载。

### Pro

- Pro 用户不限制下载数量。
- 默认一个 license 可激活 3 台设备。
- 激活后客户端可以离线使用一段时间。
- 订阅过期、退款、争议、后台禁用后，Pro 权限在 token 过期或刷新失败后失效。

### 用户体系

v1 不做传统账号注册和密码登录。

用户身份采用：

- Stripe Checkout 收集的购买邮箱。
- 服务端生成的 license key。
- 必要时通过购买邮箱发送验证码或 magic link 进行设备迁移确认。

这样可以减少注册、忘记密码、账号安全等额外复杂度，同时满足桌面软件授权需求。

## 3. Stripe Payment Flow

Stripe 不会自动生成软件序列号。序列号应由我们的服务端在 Stripe webhook 确认支付成功后生成。

推荐流程：

1. 用户点击 App 或官网里的“购买 Pro”。
2. 打开 Stripe Checkout。
3. 用户完成支付。
4. Stripe 发送 webhook 到 License Server。
5. License Server 验证 webhook 签名。
6. License Server 根据 Stripe customer / subscription / payment 信息创建 license。
7. License Server 生成 license key。
8. License Server 发送邮件给用户。
9. 用户在 App 内输入 license key 激活 Pro。

支付成功页只显示：

> 购买成功。激活码将发送到你的邮箱，请回到 App 中输入激活码完成 Pro 激活。

不要依赖成功页跳转来开通权限。用户关闭浏览器、网络中断或跳转失败时，webhook 仍然必须能完成发码。

## 4. Stripe Events

服务端至少处理这些 Stripe webhook：

- `checkout.session.completed`
  - 一次性购买或订阅首次支付完成。
  - 创建 license。
  - 生成 license key。
  - 发送邮件。
- `invoice.payment_succeeded`
  - 订阅续费成功。
  - 保持 license active。
  - 更新 `current_period_end`。
- `invoice.payment_failed`
  - 续费失败。
  - 可标记为 `past_due`，不要立刻禁用，等待 Stripe 的重试周期。
- `customer.subscription.updated`
  - 同步订阅状态、周期结束时间、取消标记。
- `customer.subscription.deleted`
  - 订阅已取消且到期。
  - 将 license 标记为 expired。
- `charge.refunded`
  - 退款后禁用或撤销 license。
- `charge.dispute.created`
  - 支付争议时可临时禁用 license，避免风险。

Webhook 必须幂等：

- 保存 Stripe event id。
- 同一个 event 重复到达时直接返回成功，不重复生成 license 或重复发邮件。

## 5. License Key and Email

### License Key 格式

建议格式：

```text
YTB-XXXX-XXXX-XXXX-XXXX
```

实现要求：

- 使用安全随机数生成。
- 数据库只保存 key hash，不保存明文。
- 邮件中发送明文 license key。
- App 本地只保存必要信息，例如 key 后四位、license id、签名 token。

### 邮件内容

支付成功后发送邮件：

```text
主题：你的 YtbDown Pro 激活码

感谢购买 YtbDown Pro。

激活码：
YTB-XXXX-XXXX-XXXX-XXXX

使用方式：
1. 打开 YtbDown。
2. 进入 设置 > Pro。
3. 输入激活码并点击激活。

设备限制：
此激活码最多可同时激活 3 台设备。

如果你更换电脑，直接在新设备中激活即可。若设备数量已满，系统会自动迁移最久未使用的设备，或通过购买邮箱验证码确认后完成迁移。
```

邮件发送失败时：

- 不要回滚 license。
- 记录 `email_delivery_failed`。
- 后台提供重发邮件能力。

## 6. License Server

### Recommended Stack

可以使用任一熟悉的服务端栈。推荐简单组合：

- Node.js + Fastify / NestJS
- PostgreSQL
- Redis 可选，用于限流和验证码
- 邮件服务：Resend、Postmark、SendGrid 或 AWS SES
- 签名算法：Ed25519

License Server 是唯一持有 Stripe secret key、webhook secret、邮件 API key、token signing private key 的地方。

客户端不能直接访问 Stripe API，也不能内置任何服务端密钥。

### Core Tables

#### `licenses`

- `id`
- `license_key_hash`
- `purchase_email`
- `stripe_customer_id`
- `stripe_subscription_id`
- `stripe_checkout_session_id`
- `plan`
- `status`: `active` / `past_due` / `expired` / `disabled` / `refunded` / `disputed`
- `activation_limit`: 默认 3
- `current_period_end`
- `created_at`
- `updated_at`

#### `devices`

- `id`
- `license_id`
- `device_id`
- `device_name`
- `platform`
- `app_version`
- `status`: `active` / `revoked`
- `activated_at`
- `last_seen_at`
- `revoked_at`
- `revoke_reason`

#### `license_tokens`

- `id`
- `license_id`
- `device_id`
- `issued_at`
- `expires_at`
- `revoked_at`

#### `stripe_events`

- `id`
- `stripe_event_id`
- `event_type`
- `payload_hash`
- `processed_at`

#### `device_transfer_events`

- `id`
- `license_id`
- `old_device_id`
- `new_device_id`
- `method`: `auto_idle` / `email_code`
- `created_at`

#### `audit_logs`

- `id`
- `license_id`
- `event_type`
- `metadata`
- `created_at`

## 7. Server APIs

### `POST /v1/licenses/activate`

Request:

```json
{
  "license_key": "YTB-XXXX-XXXX-XXXX-XXXX",
  "device_id": "random-device-id",
  "device_name": "Xue's MacBook Pro",
  "platform": "macos",
  "app_version": "0.0.1"
}
```

Behavior:

- 校验 license key。
- 检查 license 状态是否可用。
- 如果当前设备已激活，直接刷新 token。
- 如果设备数未满，激活新设备。
- 如果设备数已满，进入自动迁移策略。
- 返回签名 entitlement token。

Response:

```json
{
  "status": "active",
  "plan": "pro",
  "activation_limit": 3,
  "active_device_count": 2,
  "token": "signed-token",
  "token_expires_at": "2026-06-25T00:00:00Z",
  "license_email": "user@example.com"
}
```

### `POST /v1/licenses/refresh`

Request:

```json
{
  "token": "signed-token",
  "device_id": "random-device-id",
  "app_version": "0.0.1"
}
```

Behavior:

- 验证 token。
- 确认 license 状态仍可用。
- 确认 device 没有被撤销。
- 更新 `last_seen_at`。
- 签发新 token。

### `POST /v1/licenses/deactivate`

Request:

```json
{
  "token": "signed-token",
  "device_id": "random-device-id"
}
```

Behavior:

- 撤销当前设备。
- 释放一个激活名额。
- 客户端本地清除 Pro token。

### `POST /v1/licenses/send-transfer-code`

Request:

```json
{
  "license_key": "YTB-XXXX-XXXX-XXXX-XXXX",
  "device_id": "new-device-id"
}
```

Behavior:

- 仅在设备已满且不能自动迁移时使用。
- 向购买邮箱发送 6 位验证码。
- 验证码有效期 10 分钟。

### `POST /v1/licenses/activate-with-transfer-code`

Request:

```json
{
  "license_key": "YTB-XXXX-XXXX-XXXX-XXXX",
  "device_id": "new-device-id",
  "device_name": "New PC",
  "platform": "windows",
  "app_version": "0.0.1",
  "code": "123456"
}
```

Behavior:

- 校验验证码。
- 自动撤销最久未使用的活跃设备。
- 激活新设备。
- 发送迁移通知邮件。

## 8. Automatic Device Migration

目标：旧设备不在身边时，用户尽量不需要打开网页管理设备，也不需要联系客服。

默认规则：

- 每个 license 最多 3 台活跃设备。
- 新设备激活时，如果未满 3 台，直接激活。
- 如果已满 3 台：
  - 若存在超过 30 天未使用的设备，自动撤销最久未使用设备，并激活新设备。
  - 若所有设备最近 30 天内都活跃，则要求购买邮箱验证码。
  - 验证码通过后，撤销最久未使用设备，并激活新设备。

防滥用规则：

- 每个 license 每 30 天最多自动迁移 3 次。
- 每个 license 每 24 小时最多发送 5 次验证码。
- 每个 license 每 24 小时最多成功激活 5 次。
- 触发频率限制后返回明确错误，提示联系支持。

通知规则：

- 每次自动撤销旧设备后，发送邮件通知购买邮箱。
- 邮件包含旧设备名称、新设备名称、时间、平台。
- 邮件中提供“不是本人操作”的支持入口。

App 内体验：

- 用户输入 license key 激活。
- 如果可以自动迁移，直接成功，不额外打扰用户。
- 如果需要验证码，在 App 内出现验证码输入框。
- 用户无需打开网页设备管理页。

## 9. Entitlement Token

服务端签发短期 token，客户端本地验签。

推荐：

- 算法：Ed25519
- 有效期：30 天
- 服务端保存私钥
- 客户端内置公钥

Token payload:

```json
{
  "iss": "ytbdown-license-server",
  "aud": "ytbdown-app",
  "license_id": "lic_xxx",
  "device_id": "dev_xxx",
  "plan": "pro",
  "activation_limit": 3,
  "iat": 1779667200,
  "exp": 1782259200
}
```

客户端策略：

- 启动时先本地验签 token。
- token 未过期时，允许 Pro 离线使用。
- 后台尝试 refresh。
- refresh 失败但 token 未过期，不立即降级。
- token 过期且 refresh 失败，降级为免费版。

这样可以兼顾用户体验和授权控制。

## 10. Client Integration

当前项目是 Tauri v2 + React。下载入口在 Rust 后端：

- `enqueue_download`
- `enqueue_batch`
- `QueueManager`

下载限制必须放在 Rust 后端，不能只做前端按钮限制。

### Rust Backend

新增模块：

- `src-tauri/src/core/entitlement.rs`
- `src-tauri/src/commands/entitlement.rs`

本地文件：

- `$APP_DATA/entitlement.json`

保存内容：

- `device_id`
- `license_id`
- `license_email`
- `license_key_last4`
- `signed_token`
- `token_expires_at`
- `trial_used_count`
- `trial_reserved_count`

新增 IPC：

- `get_entitlement_status`
- `activate_pro`
- `refresh_pro`
- `deactivate_pro`
- `send_transfer_code`
- `activate_with_transfer_code`

### Download Enforcement

后端规则：

- Pro token 有效：允许入队。
- 免费版：
  - 单视频入队前检查剩余额度。
  - 批量入队前检查选中数量是否超过剩余额度。
  - 超过额度时整个批次拒绝，不做部分入队。
- 任务成功完成时确认消耗额度。
- 任务失败或取消时释放预留额度。

错误信息应可被前端识别，例如：

```json
{
  "code": "quota_exceeded",
  "message": "免费版最多可下载 10 个视频，激活 Pro 后可解除限制。"
}
```

### React UI

下载页：

- 显示免费额度：`已用 X / 10`
- Pro 状态显示：`Pro 已激活`
- 免费额度不足时，下载按钮提示升级 Pro。

批量面板：

- 当选中数量超过剩余额度时提示。
- 后端仍然必须最终拦截。

设置页：

- 新增 Pro 区域：
  - 显示当前状态。
  - 输入 license key。
  - 激活按钮。
  - 退出激活按钮。
  - 需要验证码时显示验证码输入框。
  - 购买 Pro 按钮，打开 Stripe Checkout 或产品页。

## 11. Security Notes

需要接受现实：桌面客户端无法做到绝对防破解。

v1 目标是防止普通用户通过改本地 JSON 解锁，而不是抵御专业逆向。

关键措施：

- Pro 状态以服务端签名 token 为准。
- 客户端只内置公钥，不内置私钥。
- license key 明文只在用户邮件和激活请求中出现。
- 数据库保存 license key hash。
- 所有设备激活和迁移都由服务端决定。
- Stripe webhook 必须验签。
- 设备迁移必须限频。

## 12. Rollout Plan

### Phase 1: Documentation and Product Setup

- 确认 Pro 价格。
- 在 Stripe 创建产品和价格。
- 配置 Checkout。
- 配置 webhook endpoint。
- 配置邮件服务。

### Phase 2: License Server

- 实现数据库 schema。
- 实现 Stripe webhook。
- 实现 license key 生成和邮件发送。
- 实现 activate / refresh / deactivate。
- 实现自动设备迁移和验证码迁移。

### Phase 3: Client Entitlement

- Rust 增加 entitlement store。
- 后端下载入队增加免费额度和 Pro 判断。
- React 增加 Pro 激活 UI。
- React 增加下载额度提示。

### Phase 4: End-to-End Test

- 使用 Stripe test mode 完整跑通：
  - 支付
  - webhook
  - 发码
  - App 激活
  - 下载超过 10 个
  - 订阅取消
  - token 过期降级

### Phase 5: Production

- 切换 Stripe live mode。
- 发布新版本 App。
- 监控 webhook、邮件投递、激活失败率、设备迁移次数。

## 13. Test Scenarios

- 免费用户成功下载 10 个后，第 11 个被后端拒绝。
- 免费用户剩余 2 个额度时，批量选择 3 个，整个批次拒绝。
- 下载失败不消耗免费额度。
- 下载取消不消耗免费额度。
- Stripe 支付完成后生成 license key。
- Stripe webhook 重复发送时不会生成多个 license。
- 邮件发送失败时 license 仍保留，可后台重发。
- Pro license 可激活 3 台设备。
- 第 4 台设备激活时，如果存在 30 天未使用设备，自动替换。
- 第 4 台设备激活时，如果所有设备近期活跃，要求邮箱验证码。
- 验证码正确后，新设备自动激活，旧设备自动撤销。
- token 未过期且离线时，Pro 仍可用。
- token 过期且 refresh 失败时，降级为免费版。
- 订阅取消到期后，license 变为 expired。
- 退款后，license 变为 refunded 或 disabled。

## 14. Recommended Defaults

- 免费额度：10 个成功下载。
- Pro 设备数：3 台。
- token 有效期：30 天。
- 闲置设备自动迁移阈值：30 天未使用。
- 每 30 天最多自动迁移：3 次。
- 验证码有效期：10 分钟。
- 每 24 小时最多发送验证码：5 次。
- 每 24 小时最多成功激活：5 次。

