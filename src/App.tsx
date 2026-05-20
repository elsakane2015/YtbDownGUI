import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
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

  return (
    <main className="app">
      <div
        className="titlebar"
        onMouseDown={handleTitlebarMouseDown}
        onDoubleClick={handleTitlebarDoubleClick}
      >
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
