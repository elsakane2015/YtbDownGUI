import { useCallback, useEffect, useState } from "react";
import { FolderIcon } from "../components/Icons";
import {
  activatePro,
  activateWithTransferCode,
  appVersion,
  createCheckoutSession,
  deactivatePro,
  getSettings,
  getEntitlementStatus,
  getSupportContact,
  openPath,
  openUrl,
  pickFolder,
  resendLicense,
  sendTransferCode,
  updateSettings,
  type ActivateProResult,
  type AppVersion,
  type EntitlementStatus,
  type Settings,
  type SupportContact,
} from "../lib/ipc";

const CODEC_OPTIONS: { value: string; label: string }[] = [
  { value: "avc1", label: "H.264 (avc1) · 兼容性最好" },
  { value: "vp9", label: "VP9 · 体积更小" },
  { value: "av01", label: "AV1 · 最新最省" },
  { value: "", label: "任意" },
];

const HEIGHT_OPTIONS: { value: string; label: string }[] = [
  { value: "480", label: "480p" },
  { value: "720", label: "720p" },
  { value: "1080", label: "1080p" },
  { value: "1440", label: "1440p (2K)" },
  { value: "2160", label: "2160p (4K)" },
  { value: "none", label: "无上限（取最高）" },
];

export default function SettingsPage() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [version, setVersion] = useState<AppVersion | null>(null);
  const [entitlement, setEntitlement] = useState<EntitlementStatus | null>(null);
  const [support, setSupport] = useState<SupportContact | null>(null);
  const [licenseKey, setLicenseKey] = useState("");
  const [transferCode, setTransferCode] = useState("");
  const [transferRequired, setTransferRequired] = useState<{
    email_hint: string;
    active_device_count: number;
  } | null>(null);
  const [purchaseEmail, setPurchaseEmail] = useState("");
  const [busy, setBusy] = useState(false);
  const [proBusy, setProBusy] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  useEffect(() => {
    getSettings().then(setSettings).catch((e) => setToast(String(e)));
    appVersion().then(setVersion).catch(() => {});
    getEntitlementStatus().then(setEntitlement).catch((e) => setToast(String(e)));
    getSupportContact().then(setSupport).catch(() => {});
  }, []);

  const patch = useCallback(
    async (p: Partial<Record<keyof Settings, unknown>>) => {
      setBusy(true);
      try {
        // The Rust patch struct has flatter shape; map default_quality.*
        // explicitly here so the API stays simple from JS.
        const apiPatch: Record<string, unknown> = {};
        if (p.download_dir !== undefined) apiPatch.download_dir = p.download_dir;
        if (p.max_concurrency !== undefined)
          apiPatch.max_concurrency = p.max_concurrency;
        if (p.proxy !== undefined) apiPatch.proxy = p.proxy;
        if (p.auto_check_ytdlp_updates !== undefined)
          apiPatch.auto_check_ytdlp_updates = p.auto_check_ytdlp_updates;
        if (p.ytdlp_update_use_proxy !== undefined)
          apiPatch.ytdlp_update_use_proxy = p.ytdlp_update_use_proxy;
        if (p.default_quality !== undefined) {
          const dq = p.default_quality as Settings["default_quality"];
          apiPatch.default_quality_max_height = dq.max_height;
          apiPatch.default_quality_prefer_codec = dq.prefer_codec;
        }
        const next = await updateSettings(apiPatch);
        setSettings(next);
      } catch (e) {
        setToast(`保存失败：${e}`);
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const reloadEntitlement = useCallback(async () => {
    const next = await getEntitlementStatus();
    setEntitlement(next);
    return next;
  }, []);

  const handleActivate = useCallback(async () => {
    if (!licenseKey.trim()) {
      setToast("请输入激活码");
      return;
    }
    setProBusy(true);
    try {
      const result: ActivateProResult = await activatePro(licenseKey);
      if (result.kind === "activated") {
        setEntitlement(result.status);
        setTransferRequired(null);
        setTransferCode("");
        setToast("Pro 已激活");
      } else {
        setTransferRequired({
          email_hint: result.email_hint,
          active_device_count: result.active_device_count,
        });
        setToast("设备名额已满，需要邮箱验证码完成换机");
      }
    } catch (e) {
      setToast(formatProError(e));
    } finally {
      setProBusy(false);
    }
  }, [licenseKey]);

  const handleSendTransferCode = useCallback(async () => {
    if (!licenseKey.trim()) {
      setToast("请输入激活码");
      return;
    }
    setProBusy(true);
    try {
      const result = await sendTransferCode(licenseKey);
      setTransferRequired({
        email_hint: result.email_hint,
        active_device_count: transferRequired?.active_device_count ?? 0,
      });
      setToast(`验证码已发送到 ${result.email_hint}`);
    } catch (e) {
      setToast(formatProError(e));
    } finally {
      setProBusy(false);
    }
  }, [licenseKey, transferRequired?.active_device_count]);

  const handleActivateWithCode = useCallback(async () => {
    if (!licenseKey.trim() || !transferCode.trim()) {
      setToast("请输入激活码和 6 位验证码");
      return;
    }
    setProBusy(true);
    try {
      const status = await activateWithTransferCode(licenseKey, transferCode);
      setEntitlement(status);
      setTransferRequired(null);
      setTransferCode("");
      setToast("Pro 已激活");
    } catch (e) {
      setToast(formatProError(e));
    } finally {
      setProBusy(false);
    }
  }, [licenseKey, transferCode]);

  const handleDeactivate = useCallback(async () => {
    setProBusy(true);
    try {
      const status = await deactivatePro();
      setEntitlement(status);
      setToast("已退出当前设备激活");
    } catch (e) {
      setToast(formatProError(e));
    } finally {
      setProBusy(false);
    }
  }, []);

  const handlePurchase = useCallback(async () => {
    if (!purchaseEmail.trim()) {
      setToast("请输入接收激活码的邮箱");
      return;
    }
    setProBusy(true);
    try {
      const session = await createCheckoutSession(purchaseEmail);
      await openUrl(session.checkout_url);
      setToast("已打开付款页面，激活码会发送到购买邮箱");
    } catch (e) {
      setToast(formatProError(e));
    } finally {
      setProBusy(false);
    }
  }, [purchaseEmail]);

  const handleResend = useCallback(async () => {
    if (!purchaseEmail.trim()) {
      setToast("请输入购买邮箱");
      return;
    }
    setProBusy(true);
    try {
      const result = await resendLicense(purchaseEmail);
      setToast(`如果邮箱存在购买记录，激活码会发送到 ${result.email_hint}`);
    } catch (e) {
      setToast(formatProError(e));
    } finally {
      setProBusy(false);
    }
  }, [purchaseEmail]);

  if (!settings) {
    return (
      <div className="page">
        <header className="page-header">
          <h2>设置</h2>
        </header>
        <p className="muted">加载中…</p>
      </div>
    );
  }

  return (
    <div className="page">
      <header className="page-header">
        <h2>设置</h2>
        <p className="muted">
          所有改动即时保存到 ~/Library/Application Support/com.litotime.ytbdowngui/settings.json
        </p>
      </header>

      <section className="settings-card">
        <div className="settings-section-head">
          <h3>Pro 授权</h3>
          <button
            className="secondary small"
            onClick={reloadEntitlement}
            disabled={proBusy}
          >
            刷新状态
          </button>
        </div>
        <div className="pro-status-grid">
          <StatusItem
            label="当前版本"
            value={entitlement?.pro_active ? "Pro 已激活" : "免费版"}
          />
          <StatusItem
            label="购买邮箱"
            value={entitlement?.license_email ?? "未绑定"}
          />
          <StatusItem
            label="Token 到期"
            value={formatDateTime(entitlement?.token_expires_at)}
          />
          <StatusItem
            label="设备状态"
            value={
              entitlement?.pro_active
                ? `当前设备已激活 · ${shortId(entitlement.device_id)}`
                : `未激活 · ${shortId(entitlement?.device_id)}`
            }
          />
        </div>
        {entitlement?.token_validation_error &&
          entitlement.token_validation_error !== "token_missing" && (
            <div className="hint">
              {entitlement.token_validation_error === "emergency_grace_active"
                ? "授权服务暂时不可用，当前设备正在使用离线宽限。"
                : `本地授权不可用：${entitlement.token_validation_error}`}
            </div>
          )}
        <div className="settings-row">
          <label>激活码</label>
          <div className="settings-control">
            <input
              className="dir-input mono-input"
              value={licenseKey}
              onChange={(e) => setLicenseKey(e.currentTarget.value)}
              placeholder="YTB-XXXX-XXXX-XXXX-XXXX"
            />
            <button
              className="primary"
              onClick={handleActivate}
              disabled={proBusy}
            >
              激活
            </button>
            <button
              className="secondary"
              onClick={handleDeactivate}
              disabled={proBusy || !entitlement?.pro_active}
            >
              退出激活
            </button>
          </div>
        </div>
        {transferRequired && (
          <div className="settings-row">
            <label>换机验证码</label>
            <div className="settings-control">
              <input
                className="num-input mono-input"
                value={transferCode}
                maxLength={6}
                onChange={(e) =>
                  setTransferCode(e.currentTarget.value.replace(/\D/g, ""))
                }
                placeholder="6 位"
              />
              <button
                className="secondary"
                onClick={handleSendTransferCode}
                disabled={proBusy}
              >
                发送验证码
              </button>
              <button
                className="primary"
                onClick={handleActivateWithCode}
                disabled={proBusy || transferCode.length !== 6}
              >
                完成换机
              </button>
              <span className="muted small">
                已有 {transferRequired.active_device_count} 台设备，验证码发送到{" "}
                {transferRequired.email_hint}
              </span>
            </div>
          </div>
        )}
        <div className="settings-row">
          <label>购买邮箱</label>
          <div className="settings-control">
            <input
              className="dir-input"
              value={purchaseEmail}
              onChange={(e) => setPurchaseEmail(e.currentTarget.value)}
              placeholder="user@example.com"
            />
            <button className="primary" onClick={handlePurchase} disabled={proBusy}>
              购买 Pro
            </button>
            <button className="secondary" onClick={handleResend} disabled={proBusy}>
              找回激活码
            </button>
          </div>
        </div>
        {support && (
          <div className="pro-links">
            <button
              className="link-button"
              onClick={() => openUrl(support.support_url).catch(() => {})}
            >
              支持
            </button>
            <button
              className="link-button"
              onClick={() => openUrl(support.privacy_url).catch(() => {})}
            >
              隐私政策
            </button>
            <button
              className="link-button"
              onClick={() => openUrl(support.terms_url).catch(() => {})}
            >
              授权条款
            </button>
            <span className="muted small">{support.support_email}</span>
          </div>
        )}
      </section>

      <section className="settings-card">
        <h3>下载</h3>
        <div className="settings-row">
          <label>下载目录</label>
          <div className="settings-control">
            <input
              className="dir-input"
              value={settings.download_dir}
              onChange={(e) =>
                setSettings({ ...settings, download_dir: e.currentTarget.value })
              }
              onBlur={(e) => patch({ download_dir: e.currentTarget.value })}
            />
            <button
              className="secondary small"
              onClick={async () => {
                const picked = await pickFolder(settings.download_dir);
                if (picked) patch({ download_dir: picked });
              }}
            >
              浏览…
            </button>
            <button
              className="icon-btn"
              title="打开此目录"
              onClick={() => openPath(settings.download_dir).catch(() => {})}
            >
              <FolderIcon />
            </button>
          </div>
        </div>
        <div className="settings-row">
          <label>最大并发数</label>
          <div className="settings-control">
            <input
              type="number"
              min={1}
              max={16}
              value={settings.max_concurrency}
              onChange={(e) =>
                setSettings({
                  ...settings,
                  max_concurrency: Math.max(
                    1,
                    Math.min(16, Number(e.currentTarget.value) || 1),
                  ),
                })
              }
              onBlur={() => patch({ max_concurrency: settings.max_concurrency })}
              className="num-input"
            />
            <span className="muted small">
              并发数变更对**新加入**的任务生效，已运行任务不受影响
            </span>
          </div>
        </div>
      </section>

      <section className="settings-card">
        <h3>默认画质（单视频下载预选）</h3>
        <div className="settings-row">
          <label>最高分辨率</label>
          <div className="settings-control">
            <select
              value={settings.default_quality.max_height ?? "none"}
              onChange={(e) => {
                const v = e.currentTarget.value;
                const next = {
                  ...settings.default_quality,
                  max_height: v === "none" ? null : Number(v),
                };
                setSettings({ ...settings, default_quality: next });
                patch({ default_quality: next });
              }}
            >
              {HEIGHT_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </div>
        </div>
        <div className="settings-row">
          <label>视频编码</label>
          <div className="settings-control">
            <select
              value={settings.default_quality.prefer_codec}
              onChange={(e) => {
                const next = {
                  ...settings.default_quality,
                  prefer_codec: e.currentTarget.value,
                };
                setSettings({ ...settings, default_quality: next });
                patch({ default_quality: next });
              }}
            >
              {CODEC_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </div>
        </div>
      </section>

      <section className="settings-card">
        <h3>网络</h3>
        <div className="settings-row">
          <label>代理</label>
          <div className="settings-control">
            <input
              className="dir-input"
              placeholder="http://127.0.0.1:7890 / socks5://127.0.0.1:1080（留空 = 不走代理）"
              value={settings.proxy}
              onChange={(e) =>
                setSettings({ ...settings, proxy: e.currentTarget.value })
              }
              onBlur={(e) => patch({ proxy: e.currentTarget.value })}
            />
          </div>
        </div>
        <div className="settings-row">
          <label>yt-dlp 更新</label>
          <div className="settings-control settings-control-stack">
            <label className="checkbox-row">
              <input
                type="checkbox"
                checked={settings.auto_check_ytdlp_updates}
                onChange={(e) =>
                  patch({ auto_check_ytdlp_updates: e.currentTarget.checked })
                }
              />
              <span>启动时后台检查 yt-dlp 是否有新版（不自动下载，仅 UI 提示）</span>
            </label>
            <label className="checkbox-row">
              <input
                type="checkbox"
                checked={settings.ytdlp_update_use_proxy}
                disabled={!settings.proxy.trim()}
                onChange={(e) =>
                  patch({ ytdlp_update_use_proxy: e.currentTarget.checked })
                }
              />
              <span>
                更新检查 / 下载也走上面配置的代理
                {!settings.proxy.trim() && (
                  <span className="muted small">（未配置代理时此项无效）</span>
                )}
              </span>
            </label>
          </div>
        </div>
      </section>

      {busy && <div className="hint">保存中…</div>}

      <footer className="app-version-footer">
        <div>
          YtbDownGUI
          {version ? ` · v${version.version} (${version.build})` : ""}
        </div>
        <div className="footer-links">
          <a
            href="https://litotime.com"
            onClick={(e) => {
              e.preventDefault();
              openUrl("https://litotime.com").catch(() => {});
            }}
          >
            LitoTime
          </a>
          <span className="dot">·</span>
          <a
            href="https://github.com/elsakane2015/YtbDownGUI"
            onClick={(e) => {
              e.preventDefault();
              openUrl("https://github.com/elsakane2015/YtbDownGUI").catch(
                () => {},
              );
            }}
          >
            GitHub
          </a>
        </div>
      </footer>

      {toast && (
        <div className="toast" onClick={() => setToast(null)}>
          {toast}
        </div>
      )}
    </div>
  );
}

function StatusItem({ label, value }: { label: string; value: string }) {
  return (
    <div className="pro-status-item">
      <span className="muted small">{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function formatDateTime(value: string | null | undefined) {
  if (!value) return "未生成";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

function shortId(value: string | null | undefined) {
  if (!value) return "未知设备";
  return value.length > 12 ? `${value.slice(0, 8)}…${value.slice(-4)}` : value;
}

function formatProError(error: unknown) {
  const raw = String(error);
  const jsonStart = raw.indexOf("{");
  if (jsonStart >= 0) {
    try {
      const parsed = JSON.parse(raw.slice(jsonStart)) as {
        code?: string;
        message?: string;
      };
      if (parsed.code === "license_invalid") return "激活码无效或不存在";
      if (parsed.code === "license_disabled") return "该授权已被禁用";
      if (parsed.code === "transfer_code_invalid") return "验证码错误或已过期";
      if (parsed.code === "license_resend_rate_limited") {
        return "找回邮件请求过于频繁，请稍后再试";
      }
      if (parsed.message) return parsed.message;
    } catch {
      // Fall through to plain text.
    }
  }
  if (raw.includes("transfer")) return "设备名额已满，需要邮箱验证码完成换机";
  if (raw.includes("network") || raw.includes("request failed")) {
    return "无法连接授权服务，请检查网络后重试";
  }
  return raw;
}
