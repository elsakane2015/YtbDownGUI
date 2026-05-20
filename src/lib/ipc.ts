// Typed wrappers around Tauri's invoke + event listen.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type ToolVersion = { name: string; version: string };

export type AccountStatus = {
  site_id: string;
  display_name: string;
  logged_in: boolean;
  cookie_count: number;
};

// ---- probe ----

export type Stream = {
  format_id: string;
  ext: string;
  height: number | null;
  fps: number | null;
  vcodec: string;
  acodec: string;
  tbr_kbps: number | null;
  filesize_bytes: number | null;
  note: string;
};

export type Subtitle = { lang: string; name: string | null };

export type VideoInfo = {
  id: string;
  title: string;
  uploader: string | null;
  duration_s: number | null;
  thumbnail: string | null;
  combined_streams: Stream[];
  video_streams: Stream[];
  audio_streams: Stream[];
  subtitles: Subtitle[];
  auto_subtitles: Subtitle[];
  site_id: string | null;
};

export type PlaylistEntry = {
  id: string;
  url: string;
  title: string;
  duration_s: number | null;
  uploader: string | null;
  upload_date: string | null; // ISO YYYY-MM-DD
  view_count: number | null;
  thumbnail: string | null;
};

export type ProbeResult =
  | ({ kind: "single_video" } & VideoInfo)
  | {
      kind: "playlist";
      title: string;
      collection_kind: "playlist" | "channel";
      total: number;
      entries: PlaylistEntry[];
    };

export type SingleVideo = VideoInfo & { kind: "single_video" };

// ---- download ----

export type FormatSelection =
  | {
      kind: "auto";
      max_height: number | null;
      prefer_codec: string | null; // video codec
      prefer_audio_codec?: string | null;
      container?: "mp4" | "mkv" | null;
    }
  | { kind: "combined"; format_id: string }
  | {
      kind: "split";
      video_id: string;
      audio_id: string;
      container: "mp4" | "mkv";
    }
  | {
      kind: "audio_only";
      audio_id: string | null; // null = let yt-dlp pick best audio
      prefer_codec?: string | null;
      convert_to: "mp3" | "m4a" | "opus" | "flac" | null;
    };

export type SubMode = "sidecar" | "embedded" | "both";

export type SubtitleSelection = {
  manual_langs: string[];
  auto_langs: string[];
  mode: SubMode;
  convert_to: string | null;
};

export type EnqueueRequest = {
  url: string;
  title_hint: string | null;
  selection: FormatSelection;
  subtitles: SubtitleSelection;
  output_dir: string | null;
  batch_id: string | null;
  expected_total_bytes?: number | null;
  video_id_hint?: string | null;
};

export type BatchEntryRequest = {
  url: string;
  title_hint: string | null;
  expected_total_bytes?: number | null;
  video_id_hint?: string | null;
};

export type EnqueueBatchRequest = {
  entries: BatchEntryRequest[];
  selection: FormatSelection;
  subtitles: SubtitleSelection;
  output_dir: string | null;
};

export type BatchEnqueueResult = {
  batch_id: string;
  job_ids: string[];
};

export type JobState =
  | "pending"
  | "running"
  | "done"
  | "failed"
  | "canceled"
  | "skipped";

export type JobProgress = {
  percent: number | null;
  speed: string | null;
  eta: string | null;
  stage: string | null;
};

export type DownloadJob = {
  id: string;
  url: string;
  title: string | null;
  state: JobState;
  progress: JobProgress | null;
  error: string | null;
  output_dir: string;
  output_path: string | null;
  batch_id: string | null;
  created_at_ms: number;
  completed_at_ms: number | null;
};

// ---- IPC wrappers ----

export const probeToolVersions = () =>
  invoke<ToolVersion[]>("probe_tool_versions");

export const probe = (url: string) => invoke<ProbeResult>("probe", { url });

export const listAccounts = () => invoke<AccountStatus[]>("list_accounts");

export const startLogin = (siteId: string) =>
  invoke<void>("start_login", { siteId });

export const finishLogin = (siteId: string) =>
  invoke<number>("finish_login", { siteId });

export const cancelLogin = () => invoke<void>("cancel_login");

export const logout = (siteId: string) => invoke<void>("logout", { siteId });

export const exportCookiesNetscape = (siteId: string) =>
  invoke<string>("export_cookies_netscape", { siteId });

export const enqueueDownload = (req: EnqueueRequest) =>
  invoke<string>("enqueue_download", { req });

export const enqueueBatch = (req: EnqueueBatchRequest) =>
  invoke<BatchEnqueueResult>("enqueue_batch", { req });

export const listJobs = () => invoke<DownloadJob[]>("list_jobs");

export const cancelJob = (id: string) => invoke<void>("cancel_job", { id });

export const cancelBatch = (batchId: string) =>
  invoke<number>("cancel_batch", { batchId });

export const clearFinished = () =>
  invoke<DownloadJob[]>("clear_finished");

export const defaultDownloadDir = () => invoke<string>("default_download_dir");

// ---- settings ----

export type DefaultQuality = {
  max_height: number | null;
  prefer_codec: string;
};

export type Settings = {
  download_dir: string;
  max_concurrency: number;
  default_quality: DefaultQuality;
  proxy: string;
  auto_check_ytdlp_updates: boolean;
  ytdlp_update_use_proxy: boolean;
};

export type SettingsPatch = {
  download_dir?: string;
  max_concurrency?: number;
  default_quality_max_height?: number | null;
  default_quality_prefer_codec?: string;
  proxy?: string;
  auto_check_ytdlp_updates?: boolean;
  ytdlp_update_use_proxy?: boolean;
};

export const getSettings = () => invoke<Settings>("get_settings");

export const updateSettings = (patch: SettingsPatch) =>
  invoke<Settings>("update_settings", { patch });

export const onSettingsUpdated = (
  cb: (settings: Settings) => void,
): Promise<UnlistenFn> =>
  listen<Settings>("settings:updated", (e) => cb(e.payload));

// ---- yt-dlp update ----

export type YtdlpUpdateInfo = {
  current: string;
  latest: string;
  release_url: string;
  asset_url: string;
};

export const checkYtdlpUpdate = () =>
  invoke<YtdlpUpdateInfo | null>("check_ytdlp_update");

export const installYtdlpUpdate = (info: YtdlpUpdateInfo) =>
  invoke<string>("install_ytdlp_update", { info });

export const onYtdlpUpdateAvailable = (
  cb: (info: YtdlpUpdateInfo) => void,
): Promise<UnlistenFn> =>
  listen<YtdlpUpdateInfo>("ytdlp-update:available", (e) => cb(e.payload));

export const onYtdlpUpdateInstalled = (
  cb: (version: string) => void,
): Promise<UnlistenFn> =>
  listen<string>("ytdlp-update:installed", (e) => cb(e.payload));

export const openPath = (path: string) => invoke<void>("open_path", { path });

export const revealInFinder = (path: string) =>
  invoke<void>("reveal_in_finder", { path });

// ---- event listeners ----

export const onAccountUpdated = (
  cb: (siteId: string) => void,
): Promise<UnlistenFn> => listen<string>("account:updated", (e) => cb(e.payload));

export const onLoginEvent = (
  kind: "succeeded" | "cancelled" | "timeout" | "failed",
  cb: (payload: string) => void,
): Promise<UnlistenFn> =>
  listen<string>(`login:${kind}`, (e) => cb(e.payload));

export const onDownloadState = (
  cb: (job: DownloadJob) => void,
): Promise<UnlistenFn> =>
  listen<DownloadJob>("download:state", (e) => cb(e.payload));

export const onDownloadProgress = (
  cb: (p: { id: string; progress: JobProgress }) => void,
): Promise<UnlistenFn> =>
  listen<{ id: string; progress: JobProgress }>("download:progress", (e) =>
    cb(e.payload),
  );

export const onProbeStatus = (
  cb: (msg: string) => void,
): Promise<UnlistenFn> =>
  listen<string>("probe:status", (e) => cb(e.payload));
