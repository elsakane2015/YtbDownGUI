import { useEffect, useMemo, useState } from "react";
import { listSupportedSites } from "../lib/ipc";

type Props = {
  active: boolean;
};

export default function SupportedSitesPage({ active }: Props) {
  const [sites, setSites] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!active || sites.length > 0) return;
    let cancelled = false;
    setLoading(true);
    listSupportedSites()
      .then((items) => {
        if (cancelled) return;
        setSites(items);
        setError(null);
      })
      .catch((e) => {
        if (cancelled) return;
        setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [active, sites.length]);

  const sortedSites = useMemo(
    () => [...sites].sort((a, b) => a.localeCompare(b)),
    [sites],
  );

  return (
    <div className="page supported-sites-page">
      <header className="page-header">
        <h2>支持网站</h2>
        <p className="muted">
          当前 yt-dlp 支持的下载站点，共 {sites.length} 个。
        </p>
      </header>

      {loading && <div className="empty-card">正在读取支持网站列表…</div>}
      {error && <div className="empty-card">读取失败：{error}</div>}
      {!loading && !error && (
        <ul className="site-grid">
          {sortedSites.map((site) => (
            <li key={site} className="site-item" title={site}>
              {site}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
