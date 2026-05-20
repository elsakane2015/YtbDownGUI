import { useCallback, useEffect, useState } from "react";
import { FolderIcon } from "../components/Icons";
import {
  getSettings,
  openPath,
  updateSettings,
  type Settings,
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
  const [busy, setBusy] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  useEffect(() => {
    getSettings().then(setSettings).catch((e) => setToast(String(e)));
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
          所有改动即时保存到 ~/Library/Application Support/com.elsakane2015.ytbdowngui/settings.json
        </p>
      </header>

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
              className="icon-btn"
              title="在访达中打开"
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
      {toast && (
        <div className="toast" onClick={() => setToast(null)}>
          {toast}
        </div>
      )}
    </div>
  );
}
