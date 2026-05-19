import { useState } from "react";
import DownloadsPage from "./pages/DownloadsPage";
import AccountsPage from "./pages/AccountsPage";
import SettingsPage from "./pages/SettingsPage";
import "./App.css";

type Tab = "downloads" | "accounts" | "settings";

const NAV: { id: Tab; label: string }[] = [
  { id: "downloads", label: "下载" },
  { id: "accounts", label: "账号" },
  { id: "settings", label: "设置" },
];

function App() {
  const [tab, setTab] = useState<Tab>("downloads");

  return (
    <main className="app">
      <div className="titlebar">
        <div className="titlebar-drag" />
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
      <section className="content">
        {tab === "downloads" && <DownloadsPage />}
        {tab === "accounts" && <AccountsPage />}
        {tab === "settings" && <SettingsPage />}
      </section>
    </main>
  );
}

export default App;
