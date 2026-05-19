import { useCallback, useEffect, useState } from "react";
import {
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

export default function AccountsPage() {
  const [accounts, setAccounts] = useState<AccountStatus[]>([]);
  const [activeLogin, setActiveLogin] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

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

  const handleLogin = async (siteId: string) => {
    setBusy(siteId);
    try {
      await startLogin(siteId);
      setActiveLogin(siteId);
      setToast(`登录窗口已打开 (${siteId})。登录成功后会自动保存 cookies，无需手动操作。`);
    } catch (e) {
      setToast(String(e));
    } finally {
      setBusy(null);
    }
  };

  const handleFinish = async () => {
    if (!activeLogin) return;
    setBusy(activeLogin);
    try {
      const n = await finishLogin(activeLogin);
      setToast(`已保存 ${n} 个 cookies (${activeLogin})`);
      setActiveLogin(null);
    } catch (e) {
      setToast(`完成登录失败: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  const handleCancel = async () => {
    try {
      await cancelLogin();
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
          在内嵌窗口登录目标站点后，App 自动接管 cookies 喂给 yt-dlp。
        </p>
      </header>
      <ul className="cards">
        {accounts.map((a) => (
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
                {activeLogin === a.site_id ? (
                  <>
                    <button onClick={handleFinish} disabled={busy !== null}>
                      完成登录
                    </button>
                    <button onClick={handleCancel} className="secondary">
                      取消
                    </button>
                  </>
                ) : (
                  <>
                    <button
                      onClick={() => handleLogin(a.site_id)}
                      disabled={activeLogin !== null || busy !== null}
                    >
                      {a.logged_in ? "重新登录" : "登录"}
                    </button>
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
        ))}
      </ul>
      {toast && (
        <div className="toast" onClick={() => setToast(null)}>
          {toast}
        </div>
      )}
    </div>
  );
}
