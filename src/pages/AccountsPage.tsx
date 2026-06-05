import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import {
  cancelLogin,
  exportCookiesNetscape,
  finishLogin,
  listAccounts,
  logout,
  onAccountUpdated,
  onLoginEvent,
  startLogin,
  startLoginByUrl,
  type AccountStatus,
  type LoginEventPayload,
  type LoginStartResult,
} from "../lib/ipc";

type ActiveLogin = LoginStartResult | null;

function payloadLabel(payload: LoginEventPayload | string) {
  return typeof payload === "string" ? payload : payload.display_name;
}

export default function AccountsPage() {
  const [accounts, setAccounts] = useState<AccountStatus[]>([]);
  const [activeLogin, setActiveLogin] = useState<ActiveLogin>(null);
  const [showLoginForm, setShowLoginForm] = useState(false);
  const [loginUrl, setLoginUrl] = useState("");
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
    const unSucc = onLoginEvent("succeeded", (payload) => {
      setActiveLogin(null);
      setShowLoginForm(false);
      setLoginUrl("");
      setToast(`登录成功 (${payloadLabel(payload)})，cookies 已保存`);
      refresh();
    });
    const unCancel = onLoginEvent("cancelled", () => {
      setActiveLogin(null);
      setToast("登录已取消");
    });
    const unTimeout = onLoginEvent("timeout", () => {
      setActiveLogin(null);
      setToast("登录超时，请重试");
    });
    const unFailed = onLoginEvent("failed", (payload) => {
      setActiveLogin(null);
      setToast(`登录失败：${payloadLabel(payload)}`);
    });
    return () => {
      unAccount.then((fn) => fn());
      unSucc.then((fn) => fn());
      unCancel.then((fn) => fn());
      unTimeout.then((fn) => fn());
      unFailed.then((fn) => fn());
    };
  }, [refresh]);

  const loggedInAccounts = useMemo(
    () => accounts.filter((a) => a.status === "logged_in"),
    [accounts],
  );
  const loggedOutAccounts = useMemo(
    () => accounts.filter((a) => a.status === "logged_out"),
    [accounts],
  );

  const beginLogin = async (result: Promise<LoginStartResult>) => {
    const started = await result;
    setActiveLogin(started);
    setToast(
      started.manual_finish_required
        ? `登录窗口已打开 (${started.display_name})。登录完成后点击"完成登录"保存 cookies。`
        : `登录窗口已打开 (${started.display_name})。检测到登录 cookies 后会自动保存。`,
    );
  };

  const handleLoginByUrl = async (e: FormEvent) => {
    e.preventDefault();
    const url = loginUrl.trim();
    if (!url) {
      setToast("请输入登录网址");
      return;
    }
    setBusy("new-login");
    try {
      await beginLogin(startLoginByUrl(url));
    } catch (err) {
      setToast(String(err));
    } finally {
      setBusy(null);
    }
  };

  const handleRelogin = async (account: AccountStatus) => {
    setBusy(account.account_id);
    try {
      await beginLogin(startLogin(account.account_id));
    } catch (e) {
      setToast(String(e));
    } finally {
      setBusy(null);
    }
  };

  const handleFinish = async () => {
    if (!activeLogin) return;
    setBusy(activeLogin.account_id);
    try {
      const n = await finishLogin(activeLogin.account_id);
      setToast(`已保存 ${n} 个 cookies (${activeLogin.display_name})`);
      setActiveLogin(null);
      setShowLoginForm(false);
      setLoginUrl("");
      refresh();
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

  const handleLogout = async (account: AccountStatus) => {
    setBusy(account.account_id);
    try {
      await logout(account.account_id);
      setToast(`已从本应用移除 ${account.display_name} cookies`);
      refresh();
    } catch (e) {
      setToast(String(e));
    } finally {
      setBusy(null);
    }
  };

  const handleExport = async (account: AccountStatus) => {
    try {
      const path = await exportCookiesNetscape(account.account_id);
      setToast(`cookies.txt: ${path}`);
    } catch (e) {
      setToast(String(e));
    }
  };

  const renderAccountCard = (account: AccountStatus) => {
    const isActive = activeLogin?.account_id === account.account_id;
    const isKnown = account.known_site_id !== null;
    return (
      <li key={account.account_id} className="card">
        <div className="card-head">
          <div>
            <h3>{account.display_name}</h3>
            <p className="muted">
              {account.logged_in
                ? `${isKnown ? "已登录" : "已保存 cookies"} · ${account.cookie_count} cookies`
                : `已登出 · ${account.primary_host}`}
            </p>
          </div>
          <div className="card-actions">
            {isActive && (
              <>
                {activeLogin.manual_finish_required && (
                  <button onClick={handleFinish} disabled={busy !== null}>
                    完成登录
                  </button>
                )}
                <button onClick={handleCancel} className="secondary">
                  取消
                </button>
              </>
            )}
            {!isActive && (
              <>
                <button
                  onClick={() => handleRelogin(account)}
                  disabled={activeLogin !== null || busy !== null}
                >
                  {account.logged_in ? "重新登录" : "登录"}
                </button>
                {account.logged_in && (
                  <button
                    onClick={() => handleExport(account)}
                    className="secondary"
                  >
                    导出 cookies.txt
                  </button>
                )}
                {account.logged_in && (
                  <button
                    onClick={() => handleLogout(account)}
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
  };

  return (
    <div className="page">
      <header className="page-header">
        <h2>账号管理</h2>
        <p className="muted">输入网址登录，App 保存 cookies 后自动喂给 yt-dlp。</p>
      </header>

      <section className="login-entry card">
        <div>
          <h3>网页登录</h3>
          <p className="muted">支持 yt-dlp 可使用 cookies 下载的网站。</p>
        </div>
        <button
          onClick={() => setShowLoginForm((v) => !v)}
          disabled={activeLogin !== null || busy !== null}
        >
          登录
        </button>
      </section>

      {showLoginForm && (
        <form className="login-form card" onSubmit={handleLoginByUrl}>
          <input
            value={loginUrl}
            onChange={(e) => setLoginUrl(e.target.value)}
            placeholder="https://www.youtube.com/"
            disabled={activeLogin !== null || busy !== null}
          />
          <button disabled={activeLogin !== null || busy !== null}>
            打开登录窗口
          </button>
        </form>
      )}

      {activeLogin && (
        <section className="active-login card">
          <div>
            <h3>{activeLogin.display_name}</h3>
            <p className="muted">{activeLogin.login_url}</p>
          </div>
          <div className="card-actions">
            {activeLogin.manual_finish_required && (
              <button onClick={handleFinish} disabled={busy !== null}>
                完成登录
              </button>
            )}
            <button onClick={handleCancel} className="secondary">
              取消
            </button>
          </div>
        </section>
      )}

      <section className="account-section">
        <div className="section-title">
          <h3>已登录</h3>
          <span className="muted">{loggedInAccounts.length}</span>
        </div>
        {loggedInAccounts.length > 0 ? (
          <ul className="cards">{loggedInAccounts.map(renderAccountCard)}</ul>
        ) : (
          <div className="empty-card">暂无已保存 cookies 的网站</div>
        )}
      </section>

      <section className="account-section">
        <div className="section-title">
          <h3>已登出</h3>
          <span className="muted">{loggedOutAccounts.length}</span>
        </div>
        {loggedOutAccounts.length > 0 ? (
          <ul className="cards">{loggedOutAccounts.map(renderAccountCard)}</ul>
        ) : (
          <div className="empty-card">登出后的网站会显示在这里</div>
        )}
      </section>

      {toast && (
        <div className="toast" onClick={() => setToast(null)}>
          {toast}
        </div>
      )}
    </div>
  );
}
