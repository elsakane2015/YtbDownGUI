import { useCallback, useEffect, useState } from "react";
import {
  browserLoginImport,
  browserLoginStart,
  cancelLogin,
  exportCookiesNetscape,
  finishLogin,
  listAccounts,
  logout,
  onAccountUpdated,
  onLoginEvent,
  startLogin,
  type AccountStatus,
} from "../lib/ipc";

type ActiveLogin =
  | null
  | { kind: "embedded"; siteId: string }
  | { kind: "browser"; siteId: string; browser: string };

export default function AccountsPage() {
  const [accounts, setAccounts] = useState<AccountStatus[]>([]);
  const [activeLogin, setActiveLogin] = useState<ActiveLogin>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  const platform =
    typeof document !== "undefined"
      ? document.body.dataset.platform ?? ""
      : "";
  const defaultBrowser = platform === "windows" ? "edge" : "safari";

  const refresh = useCallback(async () => {
    try {
      setAccounts(await listAccounts());
    } catch (e) {
      setToast(`load accounts: ${e}`);
    }
  }, []);

  useEffect(() => {
    refresh();
    const unAccount = onAccountUpdated(() => refresh());
    const unSucc = onLoginEvent("succeeded", (siteId) => {
      setActiveLogin(null);
      setToast(`登录成功 (${siteId})，cookies 已保存`);
    });
    const unCancel = onLoginEvent("cancelled", () => {
      setActiveLogin(null);
      setToast("登录已取消");
    });
    const unTimeout = onLoginEvent("timeout", () => {
      setActiveLogin(null);
      setToast("登录超时，请重试");
    });
    const unFailed = onLoginEvent("failed", (msg) => {
      setActiveLogin(null);
      setToast(`登录失败：${msg}`);
    });
    return () => {
      unAccount.then((fn) => fn());
      unSucc.then((fn) => fn());
      unCancel.then((fn) => fn());
      unTimeout.then((fn) => fn());
      unFailed.then((fn) => fn());
    };
  }, [refresh]);

  // --- embedded WebView login ---
  const handleLogin = async (siteId: string) => {
    setBusy(siteId);
    try {
      await startLogin(siteId);
      setActiveLogin({ kind: "embedded", siteId });
      setToast(
        `登录窗口已打开 (${siteId})。登录成功后会自动保存 cookies。` +
          `如果窗口卡住，点这里下面的"取消"按钮强制关闭。`,
      );
    } catch (e) {
      setToast(String(e));
    } finally {
      setBusy(null);
    }
  };

  const handleFinish = async () => {
    if (!activeLogin || activeLogin.kind !== "embedded") return;
    const siteId = activeLogin.siteId;
    setBusy(siteId);
    try {
      const n = await finishLogin(siteId);
      setToast(`已保存 ${n} 个 cookies (${siteId})`);
      setActiveLogin(null);
    } catch (e) {
      setToast(`完成登录失败: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  // --- system-browser fallback login (Windows-friendly) ---
  const handleBrowserLoginStart = async (siteId: string) => {
    setBusy(siteId);
    try {
      await browserLoginStart(siteId);
      setActiveLogin({ kind: "browser", siteId, browser: defaultBrowser });
      setToast(
        `已在系统浏览器中打开 ${siteId}。登录完成后，**完全关闭 ${defaultBrowser}（含后台进程）**，再回来点 "从 ${defaultBrowser} 导入 cookies"。浏览器未关闭时 cookie 数据库被锁住，导入会失败。`,
      );
    } catch (e) {
      setToast(String(e));
    } finally {
      setBusy(null);
    }
  };

  const handleBrowserImport = async () => {
    if (!activeLogin || activeLogin.kind !== "browser") return;
    const { siteId, browser } = activeLogin;
    setBusy(siteId);
    try {
      const n = await browserLoginImport(siteId, browser);
      setToast(`从 ${browser} 导入了 ${n} 个 cookies (${siteId})`);
      setActiveLogin(null);
    } catch (e) {
      setToast(`导入失败：${e}`);
    } finally {
      setBusy(null);
    }
  };

  const handleCancel = async () => {
    try {
      if (activeLogin?.kind === "embedded") {
        await cancelLogin();
      }
    } finally {
      setActiveLogin(null);
    }
  };

  const handleLogout = async (siteId: string) => {
    setBusy(siteId);
    try {
      await logout(siteId);
      setToast(`已登出 ${siteId}`);
    } catch (e) {
      setToast(String(e));
    } finally {
      setBusy(null);
    }
  };

  const handleExport = async (siteId: string) => {
    try {
      const path = await exportCookiesNetscape(siteId);
      setToast(`cookies.txt: ${path}`);
    } catch (e) {
      setToast(String(e));
    }
  };

  return (
    <div className="page">
      <header className="page-header">
        <h2>账号管理</h2>
        <p className="muted">
          App 内置登录窗口接管 cookies 喂给 yt-dlp。
          {platform === "windows" && (
            <>
              {" "}
              Windows 上如果内嵌窗口白屏，可用"用浏览器登录"备选路径（系统 Edge
              登录 → 自动导入 cookies）。
            </>
          )}
        </p>
      </header>
      <ul className="cards">
        {accounts.map((a) => {
          const isActive = activeLogin?.siteId === a.site_id;
          return (
            <li key={a.site_id} className="card">
              <div className="card-head">
                <div>
                  <h3>{a.display_name}</h3>
                  <p className="muted">
                    {a.logged_in
                      ? `已登录 · ${a.cookie_count} cookies`
                      : "未登录"}
                  </p>
                </div>
                <div className="card-actions">
                  {isActive && activeLogin?.kind === "embedded" && (
                    <>
                      <button onClick={handleFinish} disabled={busy !== null}>
                        完成登录
                      </button>
                      <button onClick={handleCancel} className="secondary">
                        取消
                      </button>
                    </>
                  )}
                  {isActive && activeLogin?.kind === "browser" && (
                    <>
                      <button
                        onClick={handleBrowserImport}
                        disabled={busy !== null}
                      >
                        从 {activeLogin.browser} 导入 cookies
                      </button>
                      <button onClick={handleCancel} className="secondary">
                        取消
                      </button>
                    </>
                  )}
                  {!isActive && (
                    <>
                      <button
                        onClick={() => handleLogin(a.site_id)}
                        disabled={activeLogin !== null || busy !== null}
                      >
                        {a.logged_in ? "重新登录" : "登录"}
                      </button>
                      {platform === "windows" && (
                        <button
                          className="secondary"
                          onClick={() => handleBrowserLoginStart(a.site_id)}
                          disabled={activeLogin !== null || busy !== null}
                          title="在系统默认浏览器中登录，登录完成后从浏览器读取 cookies"
                        >
                          用浏览器登录
                        </button>
                      )}
                      {a.logged_in && (
                        <button
                          onClick={() => handleExport(a.site_id)}
                          className="secondary"
                        >
                          导出 cookies.txt
                        </button>
                      )}
                      {a.logged_in && (
                        <button
                          onClick={() => handleLogout(a.site_id)}
                          className="secondary danger"
                          disabled={busy !== null}
                        >
                          登出
                        </button>
                      )}
                    </>
                  )}
                </div>
              </div>
            </li>
          );
        })}
      </ul>
      {toast && (
        <div className="toast" onClick={() => setToast(null)}>
          {toast}
        </div>
      )}
    </div>
  );
}
