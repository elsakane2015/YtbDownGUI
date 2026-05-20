import { useCallback, useEffect, useMemo, useState } from "react";
import {
  cancelJob,
  clearFinished,
  defaultDownloadDir,
  enqueueBatch,
  enqueueDownload,
  listJobs,
  onDownloadProgress,
  onDownloadState,
  onProbeStatus,
  openPath,
  probe,
  revealInFinder,
  type DownloadJob,
  type FormatSelection,
  type PlaylistEntry,
  type ProbeResult,
  type Stream,
  type SubMode,
  type SubtitleSelection,
  type VideoInfo,
} from "../lib/ipc";

type PlaylistProbe = {
  title: string;
  collection_kind: "playlist" | "channel";
  total: number;
  entries: PlaylistEntry[];
};

type Mode = "auto" | "combined" | "split" | "audio_only";

const DEFAULT_MANUAL_LANGS = ["zh-CN", "zh-Hans", "en"];

export default function DownloadsPage() {
  const [url, setUrl] = useState("");
  const [probing, setProbing] = useState(false);
  const [probeStatus, setProbeStatus] = useState<string>("");
  const [video, setVideo] = useState<VideoInfo | null>(null);
  const [playlist, setPlaylist] = useState<PlaylistProbe | null>(null);
  const [probeMsg, setProbeMsg] = useState<string | null>(null);
  const [outputDir, setOutputDir] = useState<string>("");

  // format picker state
  const [mode, setMode] = useState<Mode>("auto");
  const [combinedId, setCombinedId] = useState<string>("");
  const [videoId, setVideoId] = useState<string>("");
  const [audioId, setAudioId] = useState<string>("");
  const [splitContainer, setSplitContainer] = useState<"mp4" | "mkv">("mp4");
  const [audioOnlyId, setAudioOnlyId] = useState<string>("");
  const [audioConvert, setAudioConvert] = useState<
    "" | "mp3" | "m4a" | "opus" | "flac"
  >("");

  // subtitle picker state
  const [subLangs, setSubLangs] = useState<Set<string>>(new Set());
  const [autoLangs, setAutoLangs] = useState<Set<string>>(new Set());
  const [subMode, setSubMode] = useState<SubMode>("sidecar");

  // jobs
  const [jobs, setJobs] = useState<DownloadJob[]>([]);

  useEffect(() => {
    defaultDownloadDir().then(setOutputDir).catch(() => {});
    listJobs().then(setJobs).catch(() => {});
    const unState = onDownloadState((job) => {
      setJobs((prev) => {
        const idx = prev.findIndex((j) => j.id === job.id);
        if (idx === -1) return [...prev, job];
        const copy = [...prev];
        copy[idx] = job;
        return copy;
      });
    });
    const unProg = onDownloadProgress(({ id, progress }) => {
      setJobs((prev) =>
        prev.map((j) => (j.id === id ? { ...j, progress } : j)),
      );
    });
    const unProbe = onProbeStatus((msg) => setProbeStatus(msg));
    return () => {
      unState.then((fn) => fn());
      unProg.then((fn) => fn());
      unProbe.then((fn) => fn());
    };
  }, []);

  // After probe, pre-select 1080p H.264 defaults in each mode
  useEffect(() => {
    if (!video) return;
    setMode("auto");
    setCombinedId(
      pickDefault(video.combined_streams, 1080, "avc1")?.format_id ??
        video.combined_streams[0]?.format_id ??
        "",
    );
    setVideoId(
      pickDefault(video.video_streams, 1080, "avc1")?.format_id ??
        video.video_streams[0]?.format_id ??
        "",
    );
    setAudioId(video.audio_streams[0]?.format_id ?? "");
    setAudioOnlyId(video.audio_streams[0]?.format_id ?? "");

    // Subtitles: pre-check zh-CN / zh-Hans / en if available
    const availableManual = new Set(video.subtitles.map((s) => s.lang));
    const preSubs = new Set<string>();
    for (const l of DEFAULT_MANUAL_LANGS) {
      if (availableManual.has(l)) preSubs.add(l);
    }
    setSubLangs(preSubs);
    setAutoLangs(new Set());
    setSubMode("sidecar");
  }, [video]);

  const handleProbe = useCallback(async () => {
    if (!url.trim()) return;
    setProbing(true);
    setProbeMsg(null);
    setProbeStatus("准备探测…");
    setVideo(null);
    setPlaylist(null);
    try {
      const res: ProbeResult = await probe(url.trim());
      if (res.kind === "single_video") {
        setVideo(res);
      } else if (res.kind === "playlist") {
        setPlaylist({
          title: res.title,
          collection_kind: res.collection_kind,
          total: res.total,
          entries: res.entries,
        });
      }
    } catch (e) {
      setProbeMsg(`探测失败：${e}`);
    } finally {
      setProbing(false);
      setProbeStatus("");
    }
  }, [url]);

  const buildSelection = (): FormatSelection => {
    switch (mode) {
      case "combined":
        return { kind: "combined", format_id: combinedId };
      case "split":
        return {
          kind: "split",
          video_id: videoId,
          audio_id: audioId,
          container: splitContainer,
        };
      case "audio_only":
        return {
          kind: "audio_only",
          audio_id: audioOnlyId,
          convert_to: audioConvert === "" ? null : audioConvert,
        };
      case "auto":
      default:
        return { kind: "auto", max_height: 1080, prefer_codec: "avc1" };
    }
  };

  const handleEnqueue = async () => {
    if (!video) return;
    const subs: SubtitleSelection = {
      manual_langs: Array.from(subLangs),
      auto_langs: Array.from(autoLangs),
      mode: subMode,
      convert_to: subLangs.size + autoLangs.size > 0 ? "srt" : null,
    };
    const selection = buildSelection();
    const expectedTotal = estimateSelectionBytes(video, selection, {
      mode,
      combinedId,
      videoId,
      audioId,
      audioOnlyId,
    });
    try {
      await enqueueDownload({
        url: url.trim(),
        title_hint: video.title,
        selection,
        subtitles: subs,
        output_dir: outputDir || null,
        batch_id: null,
        expected_total_bytes: expectedTotal,
        video_id_hint: video.id || null,
      });
      setProbeMsg("已加入下载队列");
    } catch (e) {
      setProbeMsg(`入队失败：${e}`);
    }
  };

  return (
    <div className="page">
      <header className="page-header">
        <h2>下载</h2>
        <p className="muted">
          粘贴 YouTube / Bilibili 视频 URL，分析后选择画质与字幕。
        </p>
      </header>

      <div className="urlbar">
        <input
          className="url-input"
          placeholder="https://www.youtube.com/watch?v=..."
          value={url}
          onChange={(e) => setUrl(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && handleProbe()}
        />
        <button onClick={handleProbe} disabled={probing || !url.trim()}>
          {probing ? "分析中…" : "分析"}
        </button>
      </div>

      {probing && (
        <div className="probe-progress">
          <div className="probe-indicator" />
          <span className="muted small">{probeStatus || "请稍候…"}</span>
        </div>
      )}

      {probeMsg && <div className="hint">{probeMsg}</div>}

      {video && (
        <section className="probe-result">
          <div className="video-card">
            {video.thumbnail && (
              <img className="thumb" src={video.thumbnail} alt="" />
            )}
            <div className="meta">
              <h3>{video.title}</h3>
              <p className="muted">
                {video.uploader ?? "—"}
                {video.duration_s != null
                  ? ` · ${fmtDuration(video.duration_s)}`
                  : ""}
                {video.site_id ? ` · ${video.site_id}` : ""}
              </p>
            </div>
          </div>

          <FormatPicker
            video={video}
            mode={mode}
            setMode={setMode}
            combinedId={combinedId}
            setCombinedId={setCombinedId}
            videoId={videoId}
            setVideoId={setVideoId}
            audioId={audioId}
            setAudioId={setAudioId}
            splitContainer={splitContainer}
            setSplitContainer={setSplitContainer}
            audioOnlyId={audioOnlyId}
            setAudioOnlyId={setAudioOnlyId}
            audioConvert={audioConvert}
            setAudioConvert={setAudioConvert}
          />

          <SubtitlePicker
            video={video}
            subLangs={subLangs}
            setSubLangs={setSubLangs}
            autoLangs={autoLangs}
            setAutoLangs={setAutoLangs}
            subMode={subMode}
            setSubMode={setSubMode}
          />

          <div className="output-dir">
            <label>下载到：</label>
            <input
              value={outputDir}
              onChange={(e) => setOutputDir(e.currentTarget.value)}
              className="dir-input"
            />
            <button
              className="icon-btn"
              title="在访达中打开"
              onClick={() => outputDir && openPath(outputDir)}
            >
              <FolderIcon />
            </button>
          </div>

          <div className="action-row">
            <button className="primary" onClick={handleEnqueue}>
              下载
            </button>
          </div>
        </section>
      )}

      {playlist && (
        <PlaylistPanel
          playlist={playlist}
          outputDir={outputDir}
          setOutputDir={setOutputDir}
          onMessage={setProbeMsg}
        />
      )}

      <JobsList jobs={jobs} />
    </div>
  );
}

// ---- PlaylistPanel ----

function PlaylistPanel({
  playlist,
  outputDir,
  setOutputDir,
  onMessage,
}: {
  playlist: PlaylistProbe;
  outputDir: string;
  setOutputDir: (s: string) => void;
  onMessage: (s: string) => void;
}) {
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [keyword, setKeyword] = useState("");
  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");
  const [maxRows, setMaxRows] = useState(50);

  // Subtitle prefs (batch mode — picked from a fixed list since per-video
  // probe is too slow)
  const [subLangs, setSubLangs] = useState<Set<string>>(
    new Set(["zh-CN", "zh-Hans", "en"]),
  );
  const [autoLangs, setAutoLangs] = useState<Set<string>>(new Set());
  const [subMode, setSubMode] = useState<SubMode>("sidecar");

  // Batch-wide quality settings (applied uniformly to every checked video).
  // Two modes — same structure as single-video picker, but no per-video
  // format_id selectors (would mean nothing across a heterogeneous list).
  type BatchMode = "video_audio" | "audio_only";
  const [batchMode, setBatchMode] = useState<BatchMode>("video_audio");
  const [batchMaxHeight, setBatchMaxHeight] = useState<number | null>(1080);
  const [batchVideoCodec, setBatchVideoCodec] = useState<string>("avc1");
  const [batchAudioCodec, setBatchAudioCodec] = useState<string>("");
  const [batchContainer, setBatchContainer] = useState<"mp4" | "mkv">("mp4");
  const [batchAudioOnlyCodec, setBatchAudioOnlyCodec] = useState<string>("");
  const [batchConvertTo, setBatchConvertTo] = useState<
    "" | "mp3" | "m4a" | "opus" | "flac"
  >("");

  const visible = useMemo(() => {
    const kw = keyword.trim().toLowerCase();
    let rows = playlist.entries.filter((e) => {
      if (kw && !e.title.toLowerCase().includes(kw)) return false;
      if (dateFrom && e.upload_date && e.upload_date < dateFrom) return false;
      if (dateTo && e.upload_date && e.upload_date > dateTo) return false;
      return true;
    });
    if (maxRows > 0 && rows.length > maxRows) rows = rows.slice(0, maxRows);
    return rows;
  }, [playlist.entries, keyword, dateFrom, dateTo, maxRows]);

  const toggle = (id: string) => {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const selectAllVisible = () => {
    setChecked((prev) => {
      const next = new Set(prev);
      for (const e of visible) next.add(e.id);
      return next;
    });
  };
  const clearVisible = () => {
    setChecked((prev) => {
      const next = new Set(prev);
      for (const e of visible) next.delete(e.id);
      return next;
    });
  };
  const invertVisible = () => {
    setChecked((prev) => {
      const next = new Set(prev);
      for (const e of visible) {
        if (next.has(e.id)) next.delete(e.id);
        else next.add(e.id);
      }
      return next;
    });
  };
  const selectFirstN = (n: number) => {
    setChecked((prev) => {
      const next = new Set(prev);
      for (const e of visible.slice(0, n)) next.add(e.id);
      return next;
    });
  };

  const toggleLang = (set: Set<string>, lang: string) => {
    const next = new Set(set);
    if (next.has(lang)) next.delete(lang);
    else next.add(lang);
    return next;
  };

  const handleBatchDownload = async () => {
    if (checked.size === 0) {
      onMessage("请先勾选至少一个视频");
      return;
    }
    const selectionForEstimate: FormatSelection =
      batchMode === "video_audio"
        ? {
            kind: "auto",
            max_height: batchMaxHeight,
            prefer_codec: batchVideoCodec || null,
          }
        : {
            kind: "audio_only",
            audio_id: null,
            convert_to: batchConvertTo === "" ? null : batchConvertTo,
          };

    const bytesPerSec = estimateBytesPerSecond(selectionForEstimate);

    const entries = Array.from(checked)
      .map((id) => playlist.entries.find((e) => e.id === id))
      .filter((e): e is PlaylistEntry => !!e)
      .map((e) => ({
        url: e.url,
        title_hint: e.title,
        expected_total_bytes:
          e.duration_s != null && bytesPerSec > 0
            ? Math.round(bytesPerSec * e.duration_s)
            : null,
        video_id_hint: e.id || null,
      }));

    const subs: SubtitleSelection = {
      manual_langs: Array.from(subLangs),
      auto_langs: Array.from(autoLangs),
      mode: subMode,
      convert_to: subLangs.size + autoLangs.size > 0 ? "srt" : null,
    };

    const selection: FormatSelection =
      batchMode === "video_audio"
        ? {
            kind: "auto",
            max_height: batchMaxHeight,
            prefer_codec: batchVideoCodec || null,
            prefer_audio_codec: batchAudioCodec || null,
            container: batchContainer,
          }
        : {
            kind: "audio_only",
            audio_id: null,
            prefer_codec: batchAudioOnlyCodec || null,
            convert_to: batchConvertTo === "" ? null : batchConvertTo,
          };

    try {
      const res = await enqueueBatch({
        entries,
        selection,
        subtitles: subs,
        output_dir: outputDir || null,
      });
      onMessage(`已加入下载队列：${res.job_ids.length} 个任务 (批次 ${res.batch_id.slice(0, 8)})`);
    } catch (e) {
      onMessage(`批量入队失败：${e}`);
    }
  };

  const collectionLabel =
    playlist.collection_kind === "channel" ? "频道" : "播放列表";

  return (
    <section className="playlist-panel">
      <div className="picker">
        <div className="playlist-head">
          <div>
            <h3>{playlist.title}</h3>
            <p className="muted small">
              {collectionLabel} · 共 {playlist.total} 个视频
            </p>
          </div>
        </div>
      </div>

      <div className="picker">
        <h4>过滤（仅缩小可见范围，不影响勾选）</h4>
        <div className="filter-row">
          <div className="col">
            <label className="muted small">标题包含</label>
            <input
              value={keyword}
              onChange={(e) => setKeyword(e.currentTarget.value)}
              placeholder="关键词"
              className="dir-input"
            />
          </div>
          <div className="col">
            <label className="muted small">起始日期</label>
            <input
              type="date"
              value={dateFrom ? dateFrom : ""}
              onChange={(e) => setDateFrom(e.currentTarget.value)}
              className="dir-input"
            />
          </div>
          <div className="col">
            <label className="muted small">截止日期</label>
            <input
              type="date"
              value={dateTo ? dateTo : ""}
              onChange={(e) => setDateTo(e.currentTarget.value)}
              className="dir-input"
            />
          </div>
          <div className="col">
            <label className="muted small">最多显示</label>
            <input
              type="number"
              min={1}
              max={1000}
              value={maxRows}
              onChange={(e) =>
                setMaxRows(Math.max(1, Number(e.currentTarget.value) || 50))
              }
              className="dir-input"
            />
          </div>
        </div>
      </div>

      <div className="picker">
        <div className="bulk-row">
          <button onClick={selectAllVisible}>全选可见</button>
          <button className="secondary" onClick={clearVisible}>
            取消全选可见
          </button>
          <button className="secondary" onClick={invertVisible}>
            反选可见
          </button>
          <button className="secondary" onClick={() => selectFirstN(10)}>
            仅前 10
          </button>
          <button className="secondary" onClick={() => selectFirstN(20)}>
            仅前 20
          </button>
          <span className="muted small spacer">
            已勾选 {checked.size} / 过滤后 {visible.length} / 总{" "}
            {playlist.entries.length}
          </span>
        </div>

        <ul className="entry-list">
          {visible.map((e) => (
            <li key={e.id} className="entry-row">
              <label className="entry-check">
                <input
                  type="checkbox"
                  checked={checked.has(e.id)}
                  onChange={() => toggle(e.id)}
                />
              </label>
              {e.thumbnail && (
                <img className="entry-thumb" src={e.thumbnail} alt="" />
              )}
              <div className="entry-meta">
                <div className="entry-title">{e.title}</div>
                <div className="muted small">
                  {e.uploader ?? "—"}
                  {e.upload_date ? ` · ${e.upload_date}` : ""}
                  {e.duration_s != null
                    ? ` · ${fmtDuration(e.duration_s)}`
                    : ""}
                </div>
              </div>
            </li>
          ))}
          {visible.length === 0 && (
            <li className="entry-empty muted small">
              无匹配项（调整过滤条件）
            </li>
          )}
        </ul>
      </div>

      <div className="picker">
        <h4>画质 / 编码（整批统一）</h4>
        <div className="seg">
          {(
            [
              ["video_audio", "视频 + 音频"],
              ["audio_only", "仅音频"],
            ] as [BatchMode, string][]
          ).map(([m, label]) => (
            <button
              key={m}
              className={batchMode === m ? "seg-btn active" : "seg-btn"}
              onClick={() => setBatchMode(m)}
            >
              {label}
            </button>
          ))}
        </div>

        {batchMode === "video_audio" && (
          <>
            <div className="split-row">
              <div className="col">
                <label className="muted small">最高分辨率</label>
                <select
                  value={batchMaxHeight ?? "none"}
                  onChange={(e) => {
                    const v = e.currentTarget.value;
                    setBatchMaxHeight(v === "none" ? null : Number(v));
                  }}
                >
                  <option value="480">480p</option>
                  <option value="720">720p</option>
                  <option value="1080">1080p</option>
                  <option value="1440">1440p (2K)</option>
                  <option value="2160">2160p (4K)</option>
                  <option value="none">无上限（取最高）</option>
                </select>
              </div>
              <div className="col">
                <label className="muted small">视频编码</label>
                <select
                  value={batchVideoCodec}
                  onChange={(e) => setBatchVideoCodec(e.currentTarget.value)}
                >
                  <option value="avc1">H.264 (avc1) · 兼容性最好</option>
                  <option value="vp9">VP9 · 体积更小</option>
                  <option value="av01">AV1 · 最新最省</option>
                  <option value="">任意</option>
                </select>
              </div>
              <div className="col">
                <label className="muted small">音频编码</label>
                <select
                  value={batchAudioCodec}
                  onChange={(e) => setBatchAudioCodec(e.currentTarget.value)}
                >
                  <option value="">任意</option>
                  <option value="mp4a">AAC (mp4a)</option>
                  <option value="opus">Opus</option>
                </select>
              </div>
              <div className="col">
                <label className="muted small">容器</label>
                <select
                  value={batchContainer}
                  onChange={(e) =>
                    setBatchContainer(e.currentTarget.value as "mp4" | "mkv")
                  }
                >
                  <option value="mp4">mp4</option>
                  <option value="mkv">mkv</option>
                </select>
              </div>
            </div>
            <p className="muted small" style={{ marginTop: 8 }}>
              当某视频缺少所选编码时自动回落到任意编码，永不放弃下载。
            </p>
          </>
        )}

        {batchMode === "audio_only" && (
          <>
            <div className="split-row">
              <div className="col">
                <label className="muted small">音频编码偏好</label>
                <select
                  value={batchAudioOnlyCodec}
                  onChange={(e) =>
                    setBatchAudioOnlyCodec(e.currentTarget.value)
                  }
                >
                  <option value="">任意（最佳音频）</option>
                  <option value="mp4a">AAC (mp4a)</option>
                  <option value="opus">Opus</option>
                </select>
              </div>
              <div className="col">
                <label className="muted small">转换为</label>
                <select
                  value={batchConvertTo}
                  onChange={(e) =>
                    setBatchConvertTo(
                      e.currentTarget.value as
                        | ""
                        | "mp3"
                        | "m4a"
                        | "opus"
                        | "flac",
                    )
                  }
                >
                  <option value="">原格式（不转换）</option>
                  <option value="mp3">mp3</option>
                  <option value="m4a">m4a</option>
                  <option value="opus">opus</option>
                  <option value="flac">flac</option>
                </select>
              </div>
            </div>
          </>
        )}
      </div>

      <div className="picker">
        <h4>字幕（整批统一）</h4>
        <div className="sub-row">
          <div className="col">
            <label className="muted small">手动字幕语言</label>
            <div className="chip-wrap">
              {["zh-CN", "zh-Hans", "zh-TW", "zh-Hant", "en", "ja", "ko"].map(
                (lang) => (
                  <button
                    key={lang}
                    className={subLangs.has(lang) ? "chip active" : "chip"}
                    onClick={() => setSubLangs(toggleLang(subLangs, lang))}
                  >
                    {lang}
                  </button>
                ),
              )}
            </div>
          </div>
          <div className="col">
            <label className="muted small">自动字幕 (YouTube AI)</label>
            <div className="chip-wrap">
              {["zh-CN", "zh-Hans", "en"].map((lang) => (
                <button
                  key={lang}
                  className={autoLangs.has(lang) ? "chip active" : "chip"}
                  onClick={() => setAutoLangs(toggleLang(autoLangs, lang))}
                >
                  {lang}
                </button>
              ))}
            </div>
          </div>
        </div>
        <div className="seg sub-mode">
          {(
            [
              ["sidecar", "独立 srt 文件"],
              ["embedded", "嵌入到视频"],
              ["both", "两者都要"],
            ] as [SubMode, string][]
          ).map(([m, label]) => (
            <button
              key={m}
              className={subMode === m ? "seg-btn active" : "seg-btn"}
              onClick={() => setSubMode(m)}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      <div className="output-dir">
        <label>下载到：</label>
        <input
          value={outputDir}
          onChange={(e) => setOutputDir(e.currentTarget.value)}
          className="dir-input"
        />
        <button
          className="icon-btn"
          title="在访达中打开"
          onClick={() => outputDir && openPath(outputDir)}
        >
          <FolderIcon />
        </button>
      </div>

      <div className="action-row">
        <button
          className="primary"
          onClick={handleBatchDownload}
          disabled={checked.size === 0}
        >
          下载已勾选 ({checked.size})
        </button>
      </div>
    </section>
  );
}

// ---- FormatPicker ----

function FormatPicker(props: {
  video: VideoInfo;
  mode: Mode;
  setMode: (m: Mode) => void;
  combinedId: string;
  setCombinedId: (s: string) => void;
  videoId: string;
  setVideoId: (s: string) => void;
  audioId: string;
  setAudioId: (s: string) => void;
  splitContainer: "mp4" | "mkv";
  setSplitContainer: (c: "mp4" | "mkv") => void;
  audioOnlyId: string;
  setAudioOnlyId: (s: string) => void;
  audioConvert: "" | "mp3" | "m4a" | "opus" | "flac";
  setAudioConvert: (c: "" | "mp3" | "m4a" | "opus" | "flac") => void;
}) {
  const { video } = props;
  return (
    <section className="picker">
      <h4>画质 / 编码</h4>
      <div className="seg">
        {(
          [
            ["auto", "智能 (1080p H.264)"],
            ["combined", "组合流"],
            ["split", "视频 + 音频分选"],
            ["audio_only", "仅音频"],
          ] as [Mode, string][]
        ).map(([m, label]) => (
          <button
            key={m}
            className={props.mode === m ? "seg-btn active" : "seg-btn"}
            onClick={() => props.setMode(m)}
          >
            {label}
          </button>
        ))}
      </div>

      {props.mode === "auto" && (
        <p className="muted small">
          自动选择 ≤ 1080p 的 H.264 视频流 + 最佳音频，合并为 mp4。
        </p>
      )}

      {props.mode === "combined" && (
        <select
          value={props.combinedId}
          onChange={(e) => props.setCombinedId(e.currentTarget.value)}
        >
          {video.combined_streams.map((s) => (
            <option key={s.format_id} value={s.format_id}>
              {streamLabel(s)}
            </option>
          ))}
          {video.combined_streams.length === 0 && (
            <option disabled>无组合流可用</option>
          )}
        </select>
      )}

      {props.mode === "split" && (
        <div className="split-row">
          <div className="col">
            <label className="muted small">视频流</label>
            <select
              value={props.videoId}
              onChange={(e) => props.setVideoId(e.currentTarget.value)}
            >
              {video.video_streams.map((s) => (
                <option key={s.format_id} value={s.format_id}>
                  {streamLabel(s)}
                </option>
              ))}
            </select>
          </div>
          <div className="col">
            <label className="muted small">音频流</label>
            <select
              value={props.audioId}
              onChange={(e) => props.setAudioId(e.currentTarget.value)}
            >
              {video.audio_streams.map((s) => (
                <option key={s.format_id} value={s.format_id}>
                  {audioLabel(s)}
                </option>
              ))}
            </select>
          </div>
          <div className="col">
            <label className="muted small">容器</label>
            <select
              value={props.splitContainer}
              onChange={(e) =>
                props.setSplitContainer(e.currentTarget.value as "mp4" | "mkv")
              }
            >
              <option value="mp4">mp4</option>
              <option value="mkv">mkv</option>
            </select>
          </div>
        </div>
      )}

      {props.mode === "audio_only" && (
        <div className="split-row">
          <div className="col">
            <label className="muted small">音频流</label>
            <select
              value={props.audioOnlyId}
              onChange={(e) => props.setAudioOnlyId(e.currentTarget.value)}
            >
              {video.audio_streams.map((s) => (
                <option key={s.format_id} value={s.format_id}>
                  {audioLabel(s)}
                </option>
              ))}
            </select>
          </div>
          <div className="col">
            <label className="muted small">转换为</label>
            <select
              value={props.audioConvert}
              onChange={(e) =>
                props.setAudioConvert(
                  e.currentTarget.value as "" | "mp3" | "m4a" | "opus" | "flac",
                )
              }
            >
              <option value="">原格式（不转换）</option>
              <option value="mp3">mp3</option>
              <option value="m4a">m4a</option>
              <option value="opus">opus</option>
              <option value="flac">flac</option>
            </select>
          </div>
        </div>
      )}
    </section>
  );
}

// ---- SubtitlePicker ----

function SubtitlePicker(props: {
  video: VideoInfo;
  subLangs: Set<string>;
  setSubLangs: (s: Set<string>) => void;
  autoLangs: Set<string>;
  setAutoLangs: (s: Set<string>) => void;
  subMode: SubMode;
  setSubMode: (m: SubMode) => void;
}) {
  const toggle = (set: Set<string>, lang: string) => {
    const next = new Set(set);
    if (next.has(lang)) next.delete(lang);
    else next.add(lang);
    return next;
  };

  const manualLangs = useMemo(
    () => props.video.subtitles.map((s) => s.lang),
    [props.video],
  );
  const autoCaps = useMemo(
    () => props.video.auto_subtitles.map((s) => s.lang),
    [props.video],
  );

  // Preferred langs first
  const sortLangs = (langs: string[]) => {
    const pri = ["zh-CN", "zh-Hans", "zh-TW", "zh-Hant", "zh", "en"];
    return [...langs].sort((a, b) => {
      const ai = pri.indexOf(a);
      const bi = pri.indexOf(b);
      if (ai !== -1 && bi !== -1) return ai - bi;
      if (ai !== -1) return -1;
      if (bi !== -1) return 1;
      return a.localeCompare(b);
    });
  };

  return (
    <section className="picker">
      <h4>字幕</h4>
      <div className="sub-row">
        <div className="col">
          <label className="muted small">手动字幕（共 {manualLangs.length}）</label>
          <div className="chip-wrap">
            {sortLangs(manualLangs).map((lang) => (
              <button
                key={lang}
                className={
                  props.subLangs.has(lang) ? "chip active" : "chip"
                }
                onClick={() => props.setSubLangs(toggle(props.subLangs, lang))}
              >
                {lang}
              </button>
            ))}
            {manualLangs.length === 0 && (
              <span className="muted small">无手动字幕</span>
            )}
          </div>
        </div>
        <div className="col">
          <label className="muted small">
            自动字幕（YouTube AI 生成 · 共 {autoCaps.length}）
          </label>
          <div className="chip-wrap">
            {sortLangs(autoCaps)
              .slice(0, 12)
              .map((lang) => (
                <button
                  key={lang}
                  className={
                    props.autoLangs.has(lang) ? "chip active" : "chip"
                  }
                  onClick={() =>
                    props.setAutoLangs(toggle(props.autoLangs, lang))
                  }
                >
                  {lang}
                </button>
              ))}
            {autoCaps.length === 0 && (
              <span className="muted small">无自动字幕</span>
            )}
          </div>
        </div>
      </div>
      <div className="seg sub-mode">
        {(
          [
            ["sidecar", "独立 srt 文件"],
            ["embedded", "嵌入到视频"],
            ["both", "两者都要"],
          ] as [SubMode, string][]
        ).map(([m, label]) => (
          <button
            key={m}
            className={props.subMode === m ? "seg-btn active" : "seg-btn"}
            onClick={() => props.setSubMode(m)}
          >
            {label}
          </button>
        ))}
      </div>
    </section>
  );
}

// ---- JobsList ----

function JobsList({ jobs }: { jobs: DownloadJob[] }) {
  if (jobs.length === 0) {
    return null;
  }
  const finishedExists = jobs.some(
    (j) =>
      j.state === "done" ||
      j.state === "failed" ||
      j.state === "canceled" ||
      j.state === "skipped",
  );

  return (
    <section className="jobs">
      <div className="jobs-head">
        <h4>任务（{jobs.length}）</h4>
        {finishedExists && (
          <button className="secondary small" onClick={() => clearFinished()}>
            清理已完成
          </button>
        )}
      </div>
      <ul className="jobs-list">
        {jobs.map((j) => (
          <JobRow key={j.id} job={j} />
        ))}
      </ul>
    </section>
  );
}

function JobRow({ job: j }: { job: DownloadJob }) {
  const pct = fillPercent(j);
  const status = jobStatusText(j);
  const title = j.title ?? j.url;

  const actions = (
    <div className="job-actions">
      {(j.state === "pending" || j.state === "running") && (
        <button
          className="secondary small"
          onClick={() => cancelJob(j.id)}
        >
          取消
        </button>
      )}
      {j.state === "done" && j.output_path && (
        <button
          className="secondary small"
          onClick={() => revealInFinder(j.output_path!)}
        >
          <FolderIcon /> 在访达中显示
        </button>
      )}
      {(j.state === "skipped" || (j.state === "failed" && j.output_dir)) && (
        <button
          className="secondary small"
          onClick={() =>
            j.output_path
              ? revealInFinder(j.output_path)
              : openPath(j.output_dir)
          }
        >
          <FolderIcon /> 打开文件夹
        </button>
      )}
    </div>
  );

  // Mirror of actions used only for layout spacing in the white-text overlay.
  // visibility:hidden keeps its width so .job-status aligns with the dark layer.
  const actionsPlaceholder = (
    <div className="job-actions" aria-hidden style={{ visibility: "hidden" }}>
      {actions.props.children}
    </div>
  );

  return (
    <li className={`job state-${j.state}`} title={j.error ?? title}>
      <div className="job-fill" style={{ width: `${pct}%` }} />
      <div className="job-content">
        <span className="job-title">{title}</span>
        <span className="job-status">{status}</span>
        {actions}
      </div>
      <div
        className="job-content job-content-white"
        style={{ clipPath: `inset(0 calc(100% - ${pct}%) 0 0)` }}
        aria-hidden
      >
        <span className="job-title">{title}</span>
        <span className="job-status">{status}</span>
        {actionsPlaceholder}
      </div>
    </li>
  );
}

function fillPercent(j: DownloadJob): number {
  switch (j.state) {
    case "pending":
      return 0;
    case "running":
      return clamp(j.progress?.percent ?? 0);
    case "done":
    case "skipped":
      return 100;
    case "failed":
      return Math.max(30, clamp(j.progress?.percent ?? 0));
    case "canceled":
      return clamp(j.progress?.percent ?? 0);
  }
}

function clamp(p: number): number {
  return Math.max(0, Math.min(100, p));
}

function jobStatusText(j: DownloadJob): string {
  switch (j.state) {
    case "pending":
      return "等待中";
    case "running": {
      const p = j.progress;
      const parts: string[] = [`${(p?.percent ?? 0).toFixed(0)}%`];
      if (p?.stage && p.stage !== "downloading") parts.push(p.stage);
      if (p?.speed) parts.push(p.speed);
      if (p?.eta) parts.push(`ETA ${p.eta}`);
      return parts.join(" · ");
    }
    case "done": {
      const dur = j.completed_at_ms
        ? Math.max(0, Math.round((j.completed_at_ms - j.created_at_ms) / 1000))
        : 0;
      return `已完成 · ${fmtElapsed(dur)}`;
    }
    case "failed": {
      const err = (j.error ?? "未知错误").split("\n")[0];
      return `失败 · ${truncate(err, 60)}`;
    }
    case "canceled":
      return "已取消";
    case "skipped":
      return "已存在（跳过）";
  }
}

function fmtElapsed(s: number): string {
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  const sec = s % 60;
  if (m < 60) return `${m}m${sec}s`;
  const h = Math.floor(m / 60);
  return `${h}h${m % 60}m`;
}

function truncate(s: string, n: number): string {
  return s.length <= n ? s : s.slice(0, n - 1) + "…";
}

// ---- helpers ----

/// Rough bytes/sec estimate for a quality choice. Used for batch
/// downloads where per-video stream sizes aren't probed (flat-playlist).
/// Numbers are H.264 baselines with codec efficiency multipliers; close
/// enough to drive a progress bar that's not stuck at 0%.
function estimateBytesPerSecond(selection: FormatSelection): number {
  if (selection.kind === "audio_only") {
    return 16 * 1024; // ≈ 128 kbps AAC
  }
  if (selection.kind === "auto") {
    const h = selection.max_height ?? 1080;
    const codec = selection.prefer_codec ?? "avc1";
    const videoKbps =
      h <= 480
        ? 700
        : h <= 720
          ? 1500
          : h <= 1080
            ? 3000
            : h <= 1440
              ? 6000
              : 12000;
    const efficiency = codec === "vp9" ? 0.7 : codec === "av01" ? 0.5 : 1.0;
    return ((videoKbps * efficiency + 128) * 1000) / 8;
  }
  return 0;
}

/// Best-effort estimate of total bytes for a chosen format selection.
/// First tries the stream's reported `filesize_bytes`; if missing, falls
/// back to `tbr_kbps × duration_s` (yt-dlp doesn't always set filesize on
/// adaptive streams). Returns null only when both are unknown.
function estimateStreamBytes(s: Stream, duration_s: number | null): number | null {
  if (s.filesize_bytes != null && s.filesize_bytes > 0) return s.filesize_bytes;
  if (s.tbr_kbps != null && s.tbr_kbps > 0 && duration_s != null && duration_s > 0) {
    return Math.round((s.tbr_kbps * 1000 * duration_s) / 8);
  }
  return null;
}

function estimateSelectionBytes(
  video: VideoInfo,
  selection: FormatSelection,
  ctx: {
    mode: Mode;
    combinedId: string;
    videoId: string;
    audioId: string;
    audioOnlyId: string;
  },
): number | null {
  const byId = (id: string) =>
    video.combined_streams.find((s) => s.format_id === id) ??
    video.video_streams.find((s) => s.format_id === id) ??
    video.audio_streams.find((s) => s.format_id === id) ??
    null;

  if (selection.kind === "combined") {
    const s = byId(ctx.combinedId);
    return s ? estimateStreamBytes(s, video.duration_s) : null;
  }
  if (selection.kind === "split") {
    const v = byId(ctx.videoId);
    const a = byId(ctx.audioId);
    const vb = v ? estimateStreamBytes(v, video.duration_s) : null;
    const ab = a ? estimateStreamBytes(a, video.duration_s) : null;
    if (vb != null && ab != null) return vb + ab;
    return vb ?? ab; // partial is better than nothing
  }
  if (selection.kind === "audio_only") {
    const s = byId(ctx.audioOnlyId);
    return s ? estimateStreamBytes(s, video.duration_s) : null;
  }
  // Auto: pick the best video+audio stream pair matching the constraints.
  const targetHeight = selection.max_height ?? 1080;
  const preferCodec = selection.prefer_codec ?? "";
  const video_pick = pickDefault(
    video.video_streams,
    targetHeight,
    preferCodec || "avc1",
  );
  const audio_pick = video.audio_streams[0] ?? null;
  const vb = video_pick
    ? estimateStreamBytes(video_pick, video.duration_s)
    : null;
  const ab = audio_pick
    ? estimateStreamBytes(audio_pick, video.duration_s)
    : null;
  if (vb != null && ab != null) return vb + ab;
  // Fallback to combined stream
  const combined_pick = pickDefault(
    video.combined_streams,
    targetHeight,
    preferCodec || "avc1",
  );
  return combined_pick
    ? estimateStreamBytes(combined_pick, video.duration_s)
    : null;
}

function pickDefault(
  streams: Stream[],
  targetHeight: number,
  preferCodec: string,
): Stream | null {
  // Among streams with height <= target, pick highest height, preferring codec.
  const eligible = streams.filter(
    (s) => s.height == null || s.height <= targetHeight,
  );
  const sorted = [...eligible].sort((a, b) => {
    const hb = b.height ?? 0;
    const ha = a.height ?? 0;
    if (hb !== ha) return hb - ha;
    // Codec preference: bonus to streams whose vcodec starts with preferCodec
    const ap = a.vcodec.startsWith(preferCodec) ? 1 : 0;
    const bp = b.vcodec.startsWith(preferCodec) ? 1 : 0;
    return bp - ap;
  });
  return sorted[0] ?? null;
}

function streamLabel(s: Stream): string {
  const parts: string[] = [];
  if (s.height) parts.push(`${s.height}p${s.fps && s.fps > 30 ? Math.round(s.fps) : ""}`);
  parts.push(s.ext);
  parts.push(shortCodec(s.vcodec));
  if (s.acodec && s.acodec !== "none") parts.push("+音频");
  if (s.tbr_kbps) parts.push(`${Math.round(s.tbr_kbps)}k`);
  if (s.filesize_bytes) parts.push(fmtBytes(s.filesize_bytes));
  return `[${s.format_id}] ${parts.join(" · ")}`;
}

function audioLabel(s: Stream): string {
  const parts: string[] = [];
  parts.push(shortCodec(s.acodec));
  if (s.tbr_kbps) parts.push(`${Math.round(s.tbr_kbps)}k`);
  parts.push(s.ext);
  if (s.filesize_bytes) parts.push(fmtBytes(s.filesize_bytes));
  return `[${s.format_id}] ${parts.join(" · ")}`;
}

function shortCodec(c: string): string {
  if (c.startsWith("avc1")) return "H.264";
  if (c.startsWith("vp9")) return "VP9";
  if (c.startsWith("vp09")) return "VP9";
  if (c.startsWith("av01")) return "AV1";
  if (c.startsWith("mp4a")) return "AAC";
  if (c === "opus") return "Opus";
  return c;
}

function fmtBytes(b: number): string {
  if (b < 1024) return `${b}B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(0)}KB`;
  if (b < 1024 * 1024 * 1024) return `${(b / 1024 / 1024).toFixed(1)}MB`;
  return `${(b / 1024 / 1024 / 1024).toFixed(2)}GB`;
}

function fmtDuration(s: number): string {
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
  return `${m}:${String(sec).padStart(2, "0")}`;
}

function FolderIcon() {
  // Minimal SF-Symbols-flavoured folder glyph.
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinejoin="round"
      strokeLinecap="round"
      aria-hidden
    >
      <path d="M2 4.5C2 3.67 2.67 3 3.5 3h2.4l1.2 1.6h5.4c.83 0 1.5.67 1.5 1.5v6.4c0 .83-.67 1.5-1.5 1.5h-9C2.67 14 2 13.33 2 12.5v-8Z" />
    </svg>
  );
}

function stateLabel(s: DownloadJob["state"]): string {
  return {
    pending: "等待中",
    running: "下载中",
    done: "已完成",
    failed: "失败",
    canceled: "已取消",
    skipped: "已存在（跳过）",
  }[s];
}
