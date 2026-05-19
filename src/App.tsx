import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type ToolVersion = { name: string; version: string };

function App() {
  const [versions, setVersions] = useState<ToolVersion[] | null>(null);
  const [loading, setLoading] = useState(false);

  async function checkVersions() {
    setLoading(true);
    try {
      const result = await invoke<ToolVersion[]>("probe_tool_versions");
      setVersions(result);
    } catch (e) {
      setVersions([{ name: "error", version: String(e) }]);
    } finally {
      setLoading(false);
    }
  }

  return (
    <main className="app">
      <div className="titlebar-drag" />
      <section className="content">
        <h1>YtbDownGUI</h1>
        <p className="subtitle">M1 smoke test: 验证 sidecar 二进制可执行</p>
        <button onClick={checkVersions} disabled={loading}>
          {loading ? "检查中…" : "检查 yt-dlp 与 ffmpeg 版本"}
        </button>
        {versions && (
          <ul className="versions">
            {versions.map((v) => (
              <li key={v.name}>
                <span className="tool">{v.name}</span>
                <span className="ver">{v.version}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </main>
  );
}

export default App;
