import { useState } from "react";
import { probeToolVersions, type ToolVersion } from "../lib/ipc";

export default function DownloadsPage() {
  const [versions, setVersions] = useState<ToolVersion[] | null>(null);
  const [loading, setLoading] = useState(false);

  async function check() {
    setLoading(true);
    try {
      setVersions(await probeToolVersions());
    } catch (e) {
      setVersions([{ name: "error", version: String(e) }]);
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="page">
      <header className="page-header">
        <h2>下载</h2>
        <p className="muted">M2 占位：URL 输入与队列将在 M3 / M4 完成。</p>
      </header>
      <div className="placeholder">
        <button onClick={check} disabled={loading}>
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
      </div>
    </div>
  );
}
