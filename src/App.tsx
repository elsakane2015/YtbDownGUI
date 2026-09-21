import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import DownloadsPage from "./pages/DownloadsPage";
import AccountsPage from "./pages/AccountsPage";
import SettingsPage from "./pages/SettingsPage";
import SupportedSitesPage from "./pages/SupportedSitesPage";
import {
  appVersion,
  checkAppUpdate,
  installAppUpdate,
  installYtdlpUpdate,
  onAppUpdateProgress,
  onYtdlpUpdateAvailable,
  onYtdlpUpdateInstalled,
  type AppUpdateInfo,
  type AppUpdateProgress,
  type UpdateSource,
  type YtdlpUpdateInfo,
} from "./lib/ipc";
import "./App.css";

type Tab = "downloads" | "accounts" | "settings" | "sites";

const NAV: { id: Tab; label: string }[] = [
  { id: "downloads", label: "下载" },
  { id: "accounts", label: "账号" },
  { id: "settings", label: "设置" },
  { id: "sites", label: "支持网站" },
];

function App() {
  const [tab, setTab] = useState<Tab>("downloads");
  const [appUpdate, setAppUpdate] = useState<AppUpdateInfo | null>(null);
  const [appUpdateProgress, setAppUpdateProgress] =
    useState<AppUpdateProgress | null>(null);
  const [appUpdateError, setAppUpdateError] = useState("");
  const [updatingApp, setUpdatingApp] = useState(false);
  const [ytdlpUpdate, setYtdlpUpdate] = useState<YtdlpUpdateInfo | null>(null);
  const [updatingYtdlp, setUpdatingYtdlp] = useState(false);
  const [ytdlpUpdateError, setYtdlpUpdateError] = useState("");
  const [platform, setPlatform] = useState<string>("");

  // Tag <body> with the OS so CSS can branch (macOS overlay-title-bar
  // padding, traffic-light spacing, etc.) without sprinkling JS conditions
  // through every rule.
  useEffect(() => {
    appVersion()
      .then((v) => {
        setPlatform(v.platform);
        document.body.dataset.platform = v.platform;
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    let disposed = false;
    const check = async () => {
      try {
        const official = await checkAppUpdate("official");
        if (!disposed && official) setAppUpdate(official);
      } catch {
        try {
          const github = await checkAppUpdate("github");
          if (!disposed && github) setAppUpdate(github);
        } catch {
          // Update checks must never interrupt normal app startup.
        }
      }
    };
    const timer = window.setInterval(check, 24 * 60 * 60 * 1000);
    const unlisten = onAppUpdateProgress((progress) =>
      setAppUpdateProgress(progress),
    );
    void check();
    return () => {
      disposed = true;
      window.clearInterval(timer);
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const unAvail = onYtdlpUpdateAvailable((info) => setYtdlpUpdate(info));
    const unInst = onYtdlpUpdateInstalled(() => setYtdlpUpdate(null));
    return () => {
      unAvail.then((fn) => fn());
      unInst.then((fn) => fn());
    };
  }, []);

  const handleYtdlpUpdate = async (source: UpdateSource) => {
    setYtdlpUpdateError("");
    setUpdatingYtdlp(true);
    try {
      await installYtdlpUpdate(source);
    } catch (e) {
      setYtdlpUpdateError(String(e));
    } finally {
      setUpdatingYtdlp(false);
    }
  };

  const handleAppUpdate = async (source: UpdateSource) => {
    setAppUpdateError("");
    setAppUpdateProgress(null);
    setUpdatingApp(true);
    try {
      await installAppUpdate(source);
    } catch (e) {
      setAppUpdateError(String(e));
      setUpdatingApp(false);
    }
  };

  const appProgressLabel = (() => {
    if (!updatingApp) return "";
    if (appUpdateProgress?.finished) return "正在安装…";
    if (appUpdateProgress?.total) {
      const percent = Math.min(
        100,
        Math.round(
          (appUpdateProgress.downloaded / appUpdateProgress.total) * 100,
        ),
      );
      return `下载中 ${percent}%`;
    }
    return "下载中…";
  })();

  // Explicit drag via Tauri's window.startDragging() — works reliably on
  // macOS overlay title bars even when the window is already focused,
  // unlike `-webkit-app-region: drag` / `data-tauri-drag-region`.
  //
  // NOTE: do NOT await — by the time the awaited promise resolves the
  // mouse event has already finished and the OS won't see the drag.
  // Fire-and-forget keeps the call in the same tick.
  const handleTitlebarMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return; // primary button only
    if ((e.target as HTMLElement).closest("button, input, select, a, textarea"))
      return;
    // Suppress the browser's default text-selection-drag (which shows the
    // I-beam cursor) before handing off to the OS-level window drag.
    e.preventDefault();
    getCurrentWindow()
      .startDragging()
      .catch((err) => console.error("startDragging:", err));
  };

  const handleTitlebarDoubleClick = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button, input, select, a, textarea"))
      return;
    const win = getCurrentWindow();
    win.isMaximized().then((isMax) => {
      if (isMax) win.unmaximize();
      else win.maximize();
    });
  };

  const isMacOS = platform === "macos";

  return (
    <main className="app">
      <div
        className="titlebar"
        onMouseDown={handleTitlebarMouseDown}
        onDoubleClick={handleTitlebarDoubleClick}
      >
        {isMacOS && <div className="titlebar-drag" />}
        <nav className="tabs">
          {NAV.map((n) => (
            <button
              key={n.id}
              className={tab === n.id ? "tab active" : "tab"}
              onClick={() => setTab(n.id)}
            >
              {n.label}
            </button>
          ))}
        </nav>
      </div>
      {appUpdate && (
        <div className="update-banner app-update-banner">
          <span>
            YtbDownGUI 新版可用：<strong>v{appUpdate.latest}</strong>{" "}
            <span className="muted small">（当前 v{appUpdate.current}）</span>
            {appUpdateError && (
              <span className="update-error"> · {appUpdateError}</span>
            )}
          </span>
          <div>
            {updatingApp ? (
              <span className="update-progress">{appProgressLabel}</span>
            ) : (
              <>
                <button
                  className="primary small"
                  onClick={() => handleAppUpdate("official")}
                >
                  YtbDownGUI 官方源（推荐）
                </button>
                <button
                  className="secondary small"
                  onClick={() => handleAppUpdate("github")}
                >
                  GitHub
                </button>
                <button
                  className="secondary small"
                  onClick={() => setAppUpdate(null)}
                >
                  稍后
                </button>
              </>
            )}
          </div>
        </div>
      )}
      {ytdlpUpdate && (
        <div className="update-banner">
          <span>
            yt-dlp 新版可用：<strong>{ytdlpUpdate.latest}</strong>{" "}
            <span className="muted small">（当前 {ytdlpUpdate.current || "未知"}）</span>
            {ytdlpUpdateError && (
              <span className="update-error"> · {ytdlpUpdateError}</span>
            )}
          </span>
          <div>
            <button
              className="primary small"
              onClick={() => handleYtdlpUpdate("official")}
              disabled={updatingYtdlp}
            >
              {updatingYtdlp ? "更新中…" : "YtbDownGUI 官方源（推荐）"}
            </button>
            <button
              className="secondary small"
              onClick={() => handleYtdlpUpdate("github")}
              disabled={updatingYtdlp}
            >
              GitHub
            </button>
            <button
              className="secondary small"
              onClick={() => setYtdlpUpdate(null)}
            >
              忽略
            </button>
          </div>
        </div>
      )}
      <section className="content">
        {/* All pages stay mounted so their local state (probed URL,
            playlist selection, login progress) survives tab switches.
            Only the active page is visible. */}
        <div style={{ display: tab === "downloads" ? "block" : "none" }}>
          <DownloadsPage />
        </div>
        <div style={{ display: tab === "accounts" ? "block" : "none" }}>
          <AccountsPage />
        </div>
        <div style={{ display: tab === "settings" ? "block" : "none" }}>
          <SettingsPage />
        </div>
        <div style={{ display: tab === "sites" ? "block" : "none" }}>
          <SupportedSitesPage active={tab === "sites"} />
        </div>
      </section>
    </main>
  );
}

export default App;
