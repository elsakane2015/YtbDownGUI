//! Download engine: a small FIFO queue with a concurrency limit, each job
//! running yt-dlp as a child process and streaming progress back to the UI
//! via Tauri events.

use crate::core::{cookies, entitlement::EntitlementStore, settings::SettingsStore, sites};
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::{process::CommandEvent, ShellExt};
use tokio::sync::Semaphore;
use uuid::Uuid;

// --- selection types (mirrors of frontend choices) ------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FormatSelection {
    Auto {
        max_height: Option<u32>,
        prefer_codec: Option<String>, // video codec: "avc1" | "vp9" | "av01" | None
        #[serde(default)]
        prefer_audio_codec: Option<String>, // "mp4a" | "opus" | None (any)
        #[serde(default)]
        container: Option<Container>,
    },
    Combined {
        format_id: String,
    },
    Split {
        video_id: String,
        audio_id: String,
        container: Container,
    },
    AudioOnly {
        #[serde(default)]
        audio_id: Option<String>, // None = let yt-dlp pick best audio (batch mode)
        #[serde(default)]
        prefer_codec: Option<String>, // audio codec preference when no id given
        convert_to: Option<AudioCodec>,
    },
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Container {
    Mp4,
    Mkv,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioCodec {
    Mp3,
    M4a,
    Opus,
    Flac,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SubtitleSelection {
    #[serde(default)]
    pub manual_langs: Vec<String>,
    #[serde(default)]
    pub auto_langs: Vec<String>,
    #[serde(default = "default_sub_mode")]
    pub mode: SubMode,
    #[serde(default)]
    pub convert_to: Option<String>, // e.g. "srt"
}

fn default_sub_mode() -> SubMode {
    SubMode::Sidecar
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SubMode {
    #[default]
    Sidecar,
    Embedded,
    Both,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnqueueRequest {
    pub url: String,
    pub title_hint: Option<String>,
    pub selection: FormatSelection,
    #[serde(default)]
    pub subtitles: SubtitleSelection,
    pub output_dir: Option<String>,
    pub batch_id: Option<String>,
    /// Frontend-computed total expected bytes — used by the .part-file
    /// progress poller to derive percent. Optional: when None, the poller
    /// shows downloaded bytes + speed without a percentage.
    #[serde(default)]
    pub expected_total_bytes: Option<u64>,
    /// Frontend-known video ID (e.g. YouTube watch id, Bilibili BV id).
    /// Used to filter `.part` files in the output dir to those owned by
    /// this job. Optional: when None, the poller sums all `.part` files,
    /// which is fine for single-job dirs.
    #[serde(default)]
    pub video_id_hint: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchEntryRequest {
    pub url: String,
    pub title_hint: Option<String>,
    #[serde(default)]
    pub expected_total_bytes: Option<u64>,
    #[serde(default)]
    pub video_id_hint: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnqueueBatchRequest {
    pub entries: Vec<BatchEntryRequest>,
    pub selection: FormatSelection,
    #[serde(default)]
    pub subtitles: SubtitleSelection,
    pub output_dir: Option<String>,
}

// --- job state (server-side authoritative) --------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Pending,
    Running,
    Done,
    Failed,
    Canceled,
    Skipped, // file already exists at destination
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    pub percent: Option<f64>,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub stage: Option<String>, // "downloading" | "merging" | "post-processing"
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchEnqueueResult {
    pub batch_id: String,
    pub job_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadJob {
    pub id: String,
    pub url: String,
    pub title: Option<String>,
    pub state: JobState,
    pub progress: Option<JobProgress>,
    pub error: Option<String>,
    pub output_dir: String,
    pub output_path: Option<String>, // final absolute path after yt-dlp post-processing
    pub batch_id: Option<String>,
    pub created_at_ms: i64,
    pub completed_at_ms: Option<i64>, // set on terminal state (done/failed/canceled/skipped)
    #[serde(default)]
    pub quota_reservation_id: Option<String>,
    #[serde(default)]
    pub quota_reservation_settled: bool,
}

// --- manager (Tauri-managed singleton) ------------------------------------

pub struct QueueManager {
    inner: Arc<Inner>,
}

struct Inner {
    jobs: Mutex<HashMap<String, DownloadJob>>,
    pending: Mutex<VecDeque<String>>,
    semaphore: Arc<Semaphore>,
    abort_handles: Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>,
}

impl QueueManager {
    pub fn new(concurrency: usize) -> Self {
        Self {
            inner: Arc::new(Inner {
                jobs: Mutex::new(HashMap::new()),
                pending: Mutex::new(VecDeque::new()),
                semaphore: Arc::new(Semaphore::new(concurrency)),
                abort_handles: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Load previously-persisted jobs from disk. Any running/pending entries
    /// are marked Canceled on load (we can't resume yt-dlp child processes
    /// across an app restart).
    pub fn restore_from_disk(&self, app_data_dir: &Path) {
        let path = app_data_dir.join("jobs.json");
        if !path.exists() {
            return;
        }
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => return,
        };
        let mut jobs: Vec<DownloadJob> = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => return,
        };
        for j in jobs.iter_mut() {
            if matches!(j.state, JobState::Pending | JobState::Running) {
                j.state = JobState::Canceled;
                if j.completed_at_ms.is_none() {
                    j.completed_at_ms = Some(now_ms());
                }
                if j.error.is_none() {
                    j.error = Some("App 重启时已取消".into());
                }
            }
        }
        let mut store = self.inner.jobs.lock().unwrap();
        for j in jobs {
            store.insert(j.id.clone(), j);
        }
    }

    /// Snapshot the current jobs and persist to `$APP_DATA/jobs.json`.
    pub fn persist_to_disk(&self, app_data_dir: &Path) -> AppResult<()> {
        std::fs::create_dir_all(app_data_dir)?;
        let path = app_data_dir.join("jobs.json");
        let list = self.list();
        let bytes = serde_json::to_vec_pretty(&list)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn enqueue(
        &self,
        app: &AppHandle,
        req: EnqueueRequest,
        quota_reservation_id: Option<String>,
    ) -> AppResult<String> {
        let id = Uuid::new_v4().to_string();
        let settings_default = app.state::<SettingsStore>().get().download_dir;
        let output_dir = req
            .output_dir
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or(settings_default);
        std::fs::create_dir_all(&output_dir)?;

        let job = DownloadJob {
            id: id.clone(),
            url: req.url.clone(),
            title: req.title_hint.clone(),
            state: JobState::Pending,
            progress: None,
            error: None,
            output_dir: output_dir.clone(),
            output_path: None,
            batch_id: req.batch_id.clone(),
            created_at_ms: now_ms(),
            completed_at_ms: None,
            quota_reservation_id,
            quota_reservation_settled: false,
        };
        self.inner
            .jobs
            .lock()
            .unwrap()
            .insert(id.clone(), job.clone());
        self.inner.pending.lock().unwrap().push_back(id.clone());
        let _ = app.emit("download:state", &job);

        // Spawn worker for this job; it will wait on the semaphore.
        let inner = self.inner.clone();
        let app_handle = app.clone();
        let job_id_for_task = id.clone();
        let handle = tauri::async_runtime::spawn(async move {
            run_one_job(inner, app_handle, job_id_for_task, req).await;
        });
        self.inner
            .abort_handles
            .lock()
            .unwrap()
            .insert(id.clone(), handle);
        Ok(id)
    }

    /// Enqueue a batch of URLs sharing a single `batch_id`. Returns the
    /// batch_id and the per-entry job IDs (in input order).
    pub fn enqueue_batch(
        &self,
        app: &AppHandle,
        req: EnqueueBatchRequest,
        quota_reservation_ids: Vec<Option<String>>,
    ) -> AppResult<BatchEnqueueResult> {
        if quota_reservation_ids.len() != req.entries.len() {
            return Err(AppError::Other(
                "quota reservation count does not match batch size".into(),
            ));
        }
        let batch_id = Uuid::new_v4().to_string();
        let mut job_ids = Vec::with_capacity(req.entries.len());
        for (entry, quota_reservation_id) in req.entries.into_iter().zip(quota_reservation_ids) {
            let id = self.enqueue(
                app,
                EnqueueRequest {
                    url: entry.url,
                    title_hint: entry.title_hint,
                    selection: req.selection.clone(),
                    subtitles: req.subtitles.clone(),
                    output_dir: req.output_dir.clone(),
                    batch_id: Some(batch_id.clone()),
                    expected_total_bytes: entry.expected_total_bytes,
                    video_id_hint: entry.video_id_hint,
                },
                quota_reservation_id,
            )?;
            job_ids.push(id);
        }
        Ok(BatchEnqueueResult { batch_id, job_ids })
    }

    pub fn list(&self) -> Vec<DownloadJob> {
        let jobs = self.inner.jobs.lock().unwrap();
        let mut v: Vec<DownloadJob> = jobs.values().cloned().collect();
        v.sort_by_key(|j| j.created_at_ms);
        v
    }

    pub fn cancel(&self, app: &AppHandle, id: &str) -> AppResult<()> {
        if let Some(handle) = self.inner.abort_handles.lock().unwrap().remove(id) {
            handle.abort();
        }
        let should_cancel = self
            .inner
            .jobs
            .lock()
            .unwrap()
            .get(id)
            .map(|job| matches!(job.state, JobState::Pending | JobState::Running))
            .unwrap_or(false);
        if should_cancel {
            set_state(&self.inner, app, id, JobState::Canceled, None, None);
        }
        Ok(())
    }

    /// Cancel every job with the given batch_id.
    pub fn cancel_batch(&self, app: &AppHandle, batch_id: &str) -> AppResult<usize> {
        let ids: Vec<String> = self
            .inner
            .jobs
            .lock()
            .unwrap()
            .values()
            .filter(|j| j.batch_id.as_deref() == Some(batch_id))
            .filter(|j| matches!(j.state, JobState::Pending | JobState::Running))
            .map(|j| j.id.clone())
            .collect();
        let n = ids.len();
        for id in ids {
            let _ = self.cancel(app, &id);
        }
        Ok(n)
    }

    pub fn clear_finished(&self) -> Vec<DownloadJob> {
        {
            let mut jobs = self.inner.jobs.lock().unwrap();
            jobs.retain(|_, j| {
                !matches!(
                    j.state,
                    JobState::Done | JobState::Failed | JobState::Canceled | JobState::Skipped
                )
            });
        }
        self.list()
    }

    pub fn reconcile_quota_reservations(&self, app: &AppHandle) {
        let actions: Vec<(String, String, JobState)> = self
            .inner
            .jobs
            .lock()
            .unwrap()
            .values()
            .filter(|job| !job.quota_reservation_settled)
            .filter(|job| is_terminal_state(&job.state))
            .filter_map(|job| {
                job.quota_reservation_id
                    .clone()
                    .map(|reservation_id| (job.id.clone(), reservation_id, job.state.clone()))
            })
            .collect();

        for (job_id, reservation_id, state) in actions {
            spawn_quota_settlement(
                self.inner.clone(),
                app.clone(),
                job_id,
                reservation_id,
                state,
            );
        }
    }
}

// --- the per-job task -----------------------------------------------------

async fn run_one_job(inner: Arc<Inner>, app: AppHandle, job_id: String, req: EnqueueRequest) {
    let _permit = match inner.semaphore.clone().acquire_owned().await {
        Ok(p) => p,
        Err(_) => return,
    };

    set_state(&inner, &app, &job_id, JobState::Running, None, None);

    let settings = app.state::<SettingsStore>().get();

    let site = sites::match_url(&req.url);
    let cookies_file = match site {
        Some(s) => prepare_cookies(&app, s.id).ok(),
        None => None,
    };

    let output_dir = req
        .output_dir
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| settings.download_dir.clone());

    let mut args: Vec<String> = vec![
        "--no-config".into(),
        "--newline".into(),
        "--no-warnings".into(),
        "--no-overwrites".into(), // skip if file already exists
        "--no-mtime".into(),
        "-o".into(),
        "%(title)s [%(id)s].%(ext)s".into(),
        "--paths".into(),
        output_dir.clone(),
        // Capture the final post-move path so we can reveal it in Finder.
        "--print".into(),
        "after_move:[YTDLP_FINAL]%(filepath)s".into(),
    ];

    // ffmpeg location: bundled sidecar path resolved at runtime
    if let Ok(ff) = bundled_ffmpeg_path(&app) {
        args.push("--ffmpeg-location".into());
        args.push(ff.display().to_string());
    }
    if let Some(c) = &cookies_file {
        args.push("--cookies".into());
        args.push(c.display().to_string());
    }
    // Proxy from user settings (http://… / socks5://…). Empty = no proxy.
    if !settings.proxy.trim().is_empty() {
        args.push("--proxy".into());
        args.push(settings.proxy.trim().into());
    }

    // Apply format selection
    apply_format_args(&req.selection, &mut args);
    apply_subtitle_args(&req.subtitles, &mut args);

    args.push(req.url.clone());

    // Structured-JSON progress lines emitted by `--progress-template`.
    // Format: `[YTPROG]{"downloaded_bytes":..,"total_bytes":..,...}`.
    let re_progress_json = regex::Regex::new(r"^\[YTPROG\](\{.+\})$").unwrap();
    // Text-form fallback (defensive — useful if --progress-template gets
    // disabled or the user pipes our output somewhere). Keep loose.
    let re_progress_pct = regex::Regex::new(r"\[download\]\s+([\d.]+)%").unwrap();
    let re_progress_speed = regex::Regex::new(r"at\s+([\d.]+\w+/s|Unknown\s*B/s)").unwrap();
    let re_progress_eta = regex::Regex::new(r"ETA\s+([\d:-]+|Unknown)").unwrap();
    // "[download] /full/path/file.mp4 has already been downloaded"
    let re_already =
        regex::Regex::new(r"^\[download\]\s+(.+?)\s+has already been downloaded").unwrap();
    let re_merging = regex::Regex::new(r"Merging formats into|\[Merger\]").unwrap();
    let re_final = regex::Regex::new(r"^\[YTDLP_FINAL\](.+)$").unwrap();

    // yt-dlp_macos is a PyInstaller bundle whose stdout is block-buffered
    // when not on a terminal, so we cannot rely on it for live progress.
    // Instead we spawn a separate poller that watches the `.part` file
    // size — see `spawn_progress_poller` below.
    let cmd = match yt_dlp_command(&app) {
        Ok(c) => c.args(args.clone()),
        Err(e) => {
            fail(&inner, &app, &job_id, &format!("yt-dlp lookup: {e}"));
            return;
        }
    };

    let stop_polling = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let _poll_task = spawn_progress_poller(
        inner.clone(),
        app.clone(),
        job_id.clone(),
        output_dir.clone(),
        req.video_id_hint.clone(),
        req.expected_total_bytes,
        stop_polling.clone(),
    );

    let (mut rx, _child) = match cmd.spawn() {
        Ok(s) => s,
        Err(e) => {
            fail(&inner, &app, &job_id, &format!("spawn: {e}"));
            return;
        }
    };

    let mut last_stderr_tail: Vec<String> = Vec::with_capacity(50);
    let mut saw_already_downloaded = false;
    let mut title_capture: Option<String> = None;
    let mut final_path: Option<String> = None;

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(b) => {
                let text = String::from_utf8_lossy(&b);
                for line in text.lines() {
                    let line = line.trim_end();
                    if !line.is_empty() {
                        eprintln!("[ytdlp:{} stdout] {line}", &job_id[..8]);
                    }

                    if let Some(cap) = re_final.captures(line) {
                        final_path = Some(cap[1].trim().to_string());
                    } else if let Some(cap) = re_progress_json.captures(line) {
                        if let Some(progress) = parse_progress_json(&cap[1]) {
                            set_progress(&inner, &app, &job_id, progress);
                        }
                    } else if let Some(cap) = re_progress_pct.captures(line) {
                        // Fallback: text-form parsing for older yt-dlp / edge cases.
                        let percent: f64 = cap[1].parse().unwrap_or(0.0);
                        let speed = re_progress_speed.captures(line).map(|c| c[1].to_string());
                        let eta = re_progress_eta.captures(line).map(|c| c[1].to_string());
                        set_progress(
                            &inner,
                            &app,
                            &job_id,
                            JobProgress {
                                percent: Some(percent),
                                speed,
                                eta,
                                stage: Some("downloading".into()),
                            },
                        );
                    } else if re_merging.is_match(line) {
                        set_progress(
                            &inner,
                            &app,
                            &job_id,
                            JobProgress {
                                percent: Some(99.0),
                                speed: None,
                                eta: None,
                                stage: Some("merging".into()),
                            },
                        );
                    } else if let Some(cap) = re_already.captures(line) {
                        saw_already_downloaded = true;
                        if final_path.is_none() {
                            final_path = Some(cap[1].trim().to_string());
                        }
                    }

                    // Try to scrape the title from yt-dlp's lines like
                    // "[download] Destination: <title>.f137.mp4"
                    if title_capture.is_none() {
                        if let Some(rest) = line.strip_prefix("[download] Destination: ") {
                            let fname = std::path::Path::new(rest)
                                .file_stem()
                                .map(|s| s.to_string_lossy().to_string());
                            title_capture = fname;
                        }
                    }
                }
            }
            CommandEvent::Stderr(b) => {
                let s = String::from_utf8_lossy(&b);
                for line in s.lines() {
                    if !line.is_empty() {
                        eprintln!("[ytdlp:{} stderr] {line}", &job_id[..8]);
                    }
                    if last_stderr_tail.len() >= 50 {
                        last_stderr_tail.remove(0);
                    }
                    last_stderr_tail.push(line.to_string());
                }
            }
            CommandEvent::Terminated(p) => {
                stop_polling.store(true, std::sync::atomic::Ordering::Relaxed);
                let code = p.code;
                {
                    let mut jobs = inner.jobs.lock().unwrap();
                    if let Some(job) = jobs.get_mut(&job_id) {
                        if let Some(t) = title_capture.clone() {
                            job.title = Some(t);
                        }
                        if let Some(p) = final_path.clone() {
                            job.output_path = Some(p);
                        }
                    }
                }
                if saw_already_downloaded {
                    set_state(
                        &inner,
                        &app,
                        &job_id,
                        JobState::Skipped,
                        None,
                        Some("file already exists".into()),
                    );
                } else if code == Some(0) {
                    set_state(
                        &inner,
                        &app,
                        &job_id,
                        JobState::Done,
                        Some(JobProgress {
                            percent: Some(100.0),
                            speed: None,
                            eta: None,
                            stage: Some("done".into()),
                        }),
                        None,
                    );
                } else {
                    let last = last_stderr_tail.last().cloned().unwrap_or_default();
                    fail(&inner, &app, &job_id, &format!("exit {code:?}: {last}"));
                }
                break;
            }
            _ => {}
        }
    }

    // Cleanup
    inner.abort_handles.lock().unwrap().remove(&job_id);
}

// --- yt-dlp argument translation -----------------------------------------

fn apply_format_args(sel: &FormatSelection, args: &mut Vec<String>) {
    match sel {
        FormatSelection::Auto {
            max_height,
            prefer_codec,
            prefer_audio_codec,
            container,
        } => {
            // Build a video selector like `bv*[height<=H][vcodec^=avc1]`
            // and an audio selector like `ba[acodec^=mp4a]`, then combine
            // with fallbacks so a missing codec doesn't sink the download.
            let mut v_constraints = String::new();
            if let Some(h) = max_height {
                v_constraints.push_str(&format!("[height<={h}]"));
            }
            let mut v_constraints_with_codec = v_constraints.clone();
            if let Some(c) = prefer_codec {
                v_constraints_with_codec.push_str(&format!("[vcodec^={c}]"));
            }
            let mut a_constraints = String::new();
            if let Some(c) = prefer_audio_codec {
                a_constraints.push_str(&format!("[acodec^={c}]"));
            }

            // Layered fallback:
            //   1. video matching both height+codec + audio matching codec
            //   2. video matching height+codec + any audio
            //   3. video matching only height + any audio
            //   4. any best format
            let h_clause = max_height
                .map(|h| format!("[height<={h}]"))
                .unwrap_or_default();
            let expr = format!(
                "bv*{vcc}+ba{ac}/bv*{vcc}+ba/bv*{vc}+ba/best{hc}/best",
                vcc = v_constraints_with_codec,
                ac = a_constraints,
                vc = v_constraints,
                hc = h_clause,
            );
            args.push("-f".into());
            args.push(expr);
            args.push("--merge-output-format".into());
            args.push(
                match container.unwrap_or(Container::Mp4) {
                    Container::Mp4 => "mp4",
                    Container::Mkv => "mkv",
                }
                .into(),
            );
        }
        FormatSelection::Combined { format_id } => {
            args.push("-f".into());
            args.push(format_id.clone());
        }
        FormatSelection::Split {
            video_id,
            audio_id,
            container,
        } => {
            args.push("-f".into());
            args.push(format!("{video_id}+{audio_id}"));
            args.push("--merge-output-format".into());
            args.push(match container {
                Container::Mp4 => "mp4".into(),
                Container::Mkv => "mkv".into(),
            });
        }
        FormatSelection::AudioOnly {
            audio_id,
            prefer_codec,
            convert_to,
        } => {
            args.push("-f".into());
            let selector = if let Some(id) = audio_id {
                id.clone()
            } else if let Some(c) = prefer_codec {
                // best audio matching codec, fall back to any best audio
                format!("ba[acodec^={c}]/ba")
            } else {
                "ba".into()
            };
            args.push(selector);
            if let Some(codec) = convert_to {
                args.push("-x".into());
                args.push("--audio-format".into());
                args.push(
                    match codec {
                        AudioCodec::Mp3 => "mp3",
                        AudioCodec::M4a => "m4a",
                        AudioCodec::Opus => "opus",
                        AudioCodec::Flac => "flac",
                    }
                    .into(),
                );
            }
        }
    }
}

fn apply_subtitle_args(sel: &SubtitleSelection, args: &mut Vec<String>) {
    let want_manual = !sel.manual_langs.is_empty();
    let want_auto = !sel.auto_langs.is_empty();
    if !want_manual && !want_auto {
        return;
    }
    args.push("--write-subs".into());
    if want_auto {
        args.push("--write-auto-subs".into());
    }
    let mut langs = sel.manual_langs.clone();
    if want_auto {
        for l in &sel.auto_langs {
            if !langs.contains(l) {
                langs.push(l.clone());
            }
        }
    }
    args.push("--sub-langs".into());
    args.push(langs.join(","));
    if let Some(target) = &sel.convert_to {
        args.push("--convert-subs".into());
        args.push(target.clone());
    }
    match sel.mode {
        SubMode::Embedded => {
            args.push("--embed-subs".into());
        }
        SubMode::Both => {
            args.push("--embed-subs".into());
            // sidecar files are written by default via --write-subs
        }
        SubMode::Sidecar => {}
    }
}

// --- helpers --------------------------------------------------------------

fn set_state(
    inner: &Arc<Inner>,
    app: &AppHandle,
    id: &str,
    state: JobState,
    progress: Option<JobProgress>,
    error: Option<String>,
) {
    let (snapshot, quota_action) = {
        let mut jobs = inner.jobs.lock().unwrap();
        if let Some(job) = jobs.get_mut(id) {
            let was_terminal = is_terminal_state(&job.state);
            let is_terminal = is_terminal_state(&state);
            job.state = state.clone();
            if let Some(p) = progress {
                job.progress = Some(p);
            }
            if let Some(e) = error {
                job.error = Some(e);
            }
            if is_terminal && job.completed_at_ms.is_none() {
                job.completed_at_ms = Some(now_ms());
            }
            let quota_action = if is_terminal && !was_terminal && !job.quota_reservation_settled {
                job.quota_reservation_id
                    .clone()
                    .map(|reservation_id| (job.id.clone(), reservation_id, state.clone()))
            } else {
                None
            };
            (Some(job.clone()), quota_action)
        } else {
            (None, None)
        }
    };
    if let Some(j) = snapshot {
        let _ = app.emit("download:state", &j);
    }
    if let Some((job_id, reservation_id, terminal_state)) = quota_action {
        spawn_quota_settlement(
            inner.clone(),
            app.clone(),
            job_id,
            reservation_id,
            terminal_state,
        );
    }
}

fn is_terminal_state(state: &JobState) -> bool {
    matches!(
        state,
        JobState::Done | JobState::Failed | JobState::Canceled | JobState::Skipped
    )
}

fn spawn_quota_settlement(
    inner: Arc<Inner>,
    app: AppHandle,
    job_id: String,
    reservation_id: String,
    terminal_state: JobState,
) {
    tauri::async_runtime::spawn(async move {
        let entitlement = app.state::<EntitlementStore>();
        let result = match terminal_state {
            JobState::Done => entitlement.confirm_free_quota(reservation_id.clone()).await,
            JobState::Failed | JobState::Canceled | JobState::Skipped => {
                entitlement.release_free_quota(reservation_id.clone()).await
            }
            JobState::Pending | JobState::Running => return,
        };

        match result {
            Ok(_) => {
                let snapshot = {
                    let mut jobs = inner.jobs.lock().unwrap();
                    if let Some(job) = jobs.get_mut(&job_id) {
                        if job.quota_reservation_id.as_deref() == Some(&reservation_id) {
                            job.quota_reservation_settled = true;
                            Some(job.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                if let Some(job) = snapshot {
                    let _ = app.emit("download:state", &job);
                }
            }
            Err(error) => {
                eprintln!(
                    "[download:{job_id}] quota settlement failed for reservation {reservation_id}: {error}"
                );
            }
        }
    });
}

/// Watch `output_dir` for `.part` files belonging to this job and emit
/// progress events based on their size. This is robust against PyInstaller's
/// stdout block-buffering (which prevents yt-dlp's own progress lines from
/// reaching us in real time on macOS).
///
/// - `video_id_hint` (when provided) filters .part files to those whose
///   name contains the ID, so concurrent jobs in the same dir don't mix.
/// - `expected_total` (when provided) lets us compute percent + ETA.
fn spawn_progress_poller(
    inner: Arc<Inner>,
    app: AppHandle,
    job_id: String,
    output_dir: String,
    video_id_hint: Option<String>,
    expected_total: Option<u64>,
    stop: Arc<AtomicBool>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let dir = PathBuf::from(&output_dir);
        let mut last_bytes: u64 = 0;
        let mut last_time = Instant::now();

        // Smooth the speed reading over the last few samples.
        let mut speed_samples: VecDeque<f64> = VecDeque::with_capacity(5);
        // Track whether we have ever seen .part files. Used to detect the
        // moment they disappear (ffmpeg merged & cleaned up) so we can
        // switch the UI from "下载中" to "合并/后处理中" instead of leaving
        // it stuck at 95% for the 5–10s of yt-dlp postprocessing.
        let mut saw_part_files = false;
        let mut emitted_postprocess = false;
        let mut emitted_pre_part = false;
        let mut tick: u32 = 0;

        crate::core::log::write(format!(
            "[poll:{}] start dir={:?} id_hint={:?} expected_total={:?}",
            &job_id[..8],
            dir,
            video_id_hint,
            expected_total
        ));

        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if stop.load(Ordering::Relaxed) {
                crate::core::log::write(format!(
                    "[poll:{}] stop signal received, exiting",
                    &job_id[..8]
                ));
                break;
            }
            tick += 1;

            let total_bytes = sum_part_files(&dir, video_id_hint.as_deref());

            // Diagnostic: first few ticks always, then every 10 (~5s).
            if tick <= 3 || tick % 10 == 0 {
                let listing = list_dir_for_log(&dir, video_id_hint.as_deref());
                crate::core::log::write(format!(
                    "[poll:{}] tick {tick}: part_bytes={total_bytes} listing={}",
                    &job_id[..8],
                    listing
                ));
            }

            if total_bytes == 0 {
                // Before any .part file exists, just wait. But emit one
                // "downloading at 0%" tick so the UI shows the row is
                // alive instead of looking frozen at pending.
                if !saw_part_files {
                    if !emitted_pre_part {
                        set_progress(
                            &inner,
                            &app,
                            &job_id,
                            JobProgress {
                                percent: Some(0.0),
                                speed: None,
                                eta: None,
                                stage: Some("准备下载".into()),
                            },
                        );
                        emitted_pre_part = true;
                    }
                    continue;
                }
                // .part files have disappeared after previously existing —
                // ffmpeg merge / metadata embed / move are running. Emit a
                // single "post-processing" tick so the UI moves off
                // "下载中 95%" and the user knows we're not stuck.
                if !emitted_postprocess {
                    set_progress(
                        &inner,
                        &app,
                        &job_id,
                        JobProgress {
                            percent: Some(99.0),
                            speed: None,
                            eta: None,
                            stage: Some("合并 / 后处理中".into()),
                        },
                    );
                    emitted_postprocess = true;
                }
                continue;
            }
            saw_part_files = true;

            let now = Instant::now();
            let elapsed = now.duration_since(last_time).as_secs_f64();
            let delta = total_bytes.saturating_sub(last_bytes) as f64;
            let instant_speed = if elapsed > 0.0 { delta / elapsed } else { 0.0 };
            last_bytes = total_bytes;
            last_time = now;

            if speed_samples.len() >= 5 {
                speed_samples.pop_front();
            }
            speed_samples.push_back(instant_speed);
            let avg_speed: f64 = speed_samples.iter().sum::<f64>() / speed_samples.len() as f64;

            let (percent, eta_text) = if let Some(t) = expected_total {
                if t > 0 {
                    let p = ((total_bytes as f64 / t as f64) * 100.0).min(99.0);
                    let eta = if avg_speed > 0.0 && total_bytes < t {
                        let remaining = (t - total_bytes) as f64 / avg_speed;
                        Some(format_eta(remaining as i64))
                    } else {
                        None
                    };
                    (Some(p), eta)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            set_progress(
                &inner,
                &app,
                &job_id,
                JobProgress {
                    percent,
                    speed: Some(format_speed(avg_speed)),
                    eta: eta_text,
                    stage: Some("downloading".into()),
                },
            );
        }
    })
}

/// Compact dir listing for the log: filenames matching the id hint plus a
/// total count. Used when diagnosing "why doesn't progress update".
fn list_dir_for_log(dir: &Path, id_hint: Option<&str>) -> String {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => return format!("(read_dir err: {e})"),
    };
    let mut matching: Vec<String> = Vec::new();
    let mut total = 0usize;
    for entry in entries.flatten() {
        total += 1;
        let name = entry.file_name().to_string_lossy().to_string();
        let matches_id = match id_hint {
            Some(id) if !id.is_empty() => name.contains(id),
            _ => true,
        };
        if matches_id {
            let size = live_file_size(&entry.path());
            matching.push(format!("{name} ({size}B)"));
        }
    }
    format!(
        "[{} entries, {} matching: {:?}]",
        total,
        matching.len(),
        matching
    )
}

fn sum_part_files(dir: &Path, id_hint: Option<&str>) -> u64 {
    let mut total: u64 = 0;
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    for entry in entries.flatten() {
        let name_os = entry.file_name();
        let name = name_os.to_string_lossy();
        // Skip resume-metadata files — they're tiny and unrelated to
        // download progress.
        if name.ends_with(".ytdl") || name.ends_with(".ytdl.part") {
            continue;
        }
        // Skip cross-job pollution: if we know the video id, only count
        // files whose name contains it.
        if let Some(id) = id_hint {
            if !id.is_empty() && !name.contains(id) {
                continue;
            }
        }
        // We previously only counted ".part" files, but on multi-stream
        // downloads (video then audio) yt-dlp renames each segment to its
        // final name before the next segment starts, so the bar would
        // drop to 0% the moment the video segment completed. Counting
        // any matching file (.part OR final) keeps the cumulative bytes
        // monotonic across segments.
        total += live_file_size(&entry.path());
    }
    total
}

/// Read the *current* size of a file that may be actively being written.
/// On Windows, `metadata().len()` returns a cached value from the dir
/// entry that doesn't refresh while yt-dlp is mid-download (we saw 0B
/// reported for 35+ seconds while the file actually grew to 32 MB).
/// Opening the file and seeking to End forces the FS to report the real
/// EOF offset; if the open fails we fall back to metadata.
fn live_file_size(path: &Path) -> u64 {
    use std::io::{Seek, SeekFrom};
    if let Ok(mut f) = std::fs::File::open(path) {
        if let Ok(pos) = f.seek(SeekFrom::End(0)) {
            return pos;
        }
    }
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn parse_progress_json(s: &str) -> Option<JobProgress> {
    let v: serde_json::Value = serde_json::from_str(s).ok()?;
    let downloaded = v.get("downloaded_bytes").and_then(|x| x.as_f64());
    let total = v
        .get("total_bytes")
        .and_then(|x| x.as_f64())
        .or_else(|| v.get("total_bytes_estimate").and_then(|x| x.as_f64()));
    let percent = match (downloaded, total) {
        (Some(d), Some(t)) if t > 0.0 => Some((d / t) * 100.0),
        _ => None,
    };
    let speed = v
        .get("speed")
        .and_then(|x| x.as_f64())
        .map(|bps| format_speed(bps));
    let eta = v
        .get("eta")
        .and_then(|x| x.as_f64())
        .map(|s| format_eta(s as i64));
    let status = v.get("status").and_then(|x| x.as_str()).map(String::from);
    Some(JobProgress {
        percent,
        speed,
        eta,
        stage: status.or_else(|| Some("downloading".into())),
    })
}

fn format_speed(bps: f64) -> String {
    if bps >= 1_073_741_824.0 {
        format!("{:.2} GB/s", bps / 1_073_741_824.0)
    } else if bps >= 1_048_576.0 {
        format!("{:.2} MB/s", bps / 1_048_576.0)
    } else if bps >= 1024.0 {
        format!("{:.1} KB/s", bps / 1024.0)
    } else {
        format!("{:.0} B/s", bps)
    }
}

fn format_eta(seconds: i64) -> String {
    if seconds <= 0 {
        return "--:--".into();
    }
    let s = seconds % 60;
    let m = (seconds / 60) % 60;
    let h = seconds / 3600;
    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

fn set_progress(inner: &Arc<Inner>, app: &AppHandle, id: &str, progress: JobProgress) {
    let snapshot = {
        let mut jobs = inner.jobs.lock().unwrap();
        if let Some(job) = jobs.get_mut(id) {
            job.progress = Some(progress.clone());
            Some((job.id.clone(), progress))
        } else {
            None
        }
    };
    if let Some((id, p)) = snapshot {
        #[derive(Serialize, Clone)]
        struct Payload {
            id: String,
            progress: JobProgress,
        }
        let _ = app.emit("download:progress", Payload { id, progress: p });
    }
}

fn fail(inner: &Arc<Inner>, app: &AppHandle, id: &str, msg: &str) {
    eprintln!("[download:{id}] failed: {msg}");
    set_state(inner, app, id, JobState::Failed, None, Some(msg.into()));
}

fn prepare_cookies(app: &AppHandle, site_id: &str) -> AppResult<PathBuf> {
    let data_dir = crate::core::paths::data_dir(app)?;
    let stored = cookies::load(&data_dir, site_id)?;
    let tmp = data_dir.join("tmp");
    std::fs::create_dir_all(&tmp)?;
    let out = tmp.join(format!("{site_id}.cookies.txt"));
    cookies::write_netscape(&stored, &out)?;
    Ok(out)
}

fn bundled_ffmpeg_path(app: &AppHandle) -> AppResult<PathBuf> {
    bundled_sidecar_path(app, "ffmpeg")
}

#[allow(dead_code)] // kept for future PTY/wrapper experiments
fn bundled_ytdlp_path(app: &AppHandle) -> AppResult<PathBuf> {
    bundled_sidecar_path(app, "yt-dlp")
}

/// Build a yt-dlp Command. If the user has installed an updated yt-dlp via
/// the in-app updater (lands in `$APP_DATA/bin/yt-dlp`), prefer that path
/// over the bundled sidecar. Falls back to the sidecar otherwise.
pub fn yt_dlp_command(app: &AppHandle) -> AppResult<tauri_plugin_shell::process::Command> {
    if let Ok(dir) = crate::core::paths::data_dir(app) {
        let bin_name = if cfg!(target_os = "windows") {
            "yt-dlp.exe"
        } else {
            "yt-dlp"
        };
        let user_bin = dir.join("bin").join(bin_name);
        if user_bin.exists() {
            return Ok(app.shell().command(user_bin.to_string_lossy().to_string()));
        }
    }
    app.shell()
        .sidecar("yt-dlp")
        .map_err(|e| AppError::Other(format!("sidecar yt-dlp: {e}")))
}

/// Resolve a sidecar binary's absolute path. We need this when we want to
/// invoke the sidecar through a wrapper (like `/usr/bin/script` for PTY) or
/// pass it as an argument (yt-dlp's --ffmpeg-location).
fn bundled_sidecar_path(app: &AppHandle, name: &str) -> AppResult<PathBuf> {
    let triple_name = crate::core::paths::sidecar_filename(name);
    let bare_name = if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    };

    // Bundled production layout: alongside the main app binary
    // (Contents/MacOS/ on macOS; same dir as the .exe on Windows).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for c in [dir.join(&triple_name), dir.join(&bare_name)] {
                if c.exists() {
                    return Ok(c);
                }
            }
        }
    }
    // Dev fallback: src-tauri/binaries/
    let dev = std::env::current_dir()?.join("binaries").join(&triple_name);
    if dev.exists() {
        return Ok(dev);
    }
    // Last-resort: resource_dir (older layouts).
    if let Ok(resource_dir) = app.path().resource_dir() {
        for c in [
            resource_dir.join(&triple_name),
            resource_dir.join(&bare_name),
        ] {
            if c.exists() {
                return Ok(c);
            }
        }
    }
    Err(AppError::Other(format!("{name} binary not found")))
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
