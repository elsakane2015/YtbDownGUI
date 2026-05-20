//! Probe a video URL via `yt-dlp -J --flat-playlist` and shape the result
//! into our own `ProbeResult` enum (SingleVideo / Playlist / Channel).
//!
//! `--flat-playlist` is a no-op for single videos (they still come back with
//! their full formats array) but stops yt-dlp from recursing into each entry
//! of a playlist / channel — so a 1000-video channel returns in a few
//! seconds with metadata only.

use crate::core::{cookies, settings::SettingsStore, sites};
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::CommandEvent;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProbeResult {
    SingleVideo(VideoInfo),
    Playlist {
        title: String,
        collection_kind: CollectionKind,
        total: usize,
        entries: Vec<PlaylistEntry>,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionKind {
    Playlist,
    Channel,
}

#[derive(Debug, Clone, Serialize)]
pub struct VideoInfo {
    pub id: String,
    pub title: String,
    pub uploader: Option<String>,
    pub duration_s: Option<u32>,
    pub thumbnail: Option<String>,
    pub combined_streams: Vec<Stream>,
    pub video_streams: Vec<Stream>,
    pub audio_streams: Vec<Stream>,
    pub subtitles: Vec<Subtitle>,
    pub auto_subtitles: Vec<Subtitle>,
    pub site_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Stream {
    pub format_id: String,
    pub ext: String,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub vcodec: String,
    pub acodec: String,
    pub tbr_kbps: Option<f64>,
    pub filesize_bytes: Option<u64>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Subtitle {
    pub lang: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaylistEntry {
    pub id: String,
    pub url: String,
    pub title: String,
    pub duration_s: Option<u32>,
    pub uploader: Option<String>,
    /// ISO `YYYY-MM-DD` if upstream gives us a date, else None.
    pub upload_date: Option<String>,
    pub view_count: Option<u64>,
    pub thumbnail: Option<String>,
}

// --- yt-dlp JSON shape (only the fields we touch) -------------------------

#[derive(Debug, Deserialize)]
struct YtJson {
    #[serde(default, rename = "_type")]
    kind: Option<String>,
    id: Option<String>,
    title: Option<String>,
    uploader: Option<String>,
    channel: Option<String>,
    duration: Option<f64>,
    thumbnail: Option<String>,
    #[serde(default)]
    formats: Vec<YtFormat>,
    #[serde(default)]
    subtitles: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    automatic_captions: serde_json::Map<String, serde_json::Value>,
    // Playlist fields
    #[serde(default)]
    entries: Vec<YtEntry>,
    playlist_count: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct YtFormat {
    format_id: String,
    #[serde(default)]
    ext: Option<String>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    fps: Option<f64>,
    #[serde(default)]
    vcodec: Option<String>,
    #[serde(default)]
    acodec: Option<String>,
    #[serde(default)]
    tbr: Option<f64>,
    #[serde(default)]
    filesize: Option<u64>,
    #[serde(default)]
    filesize_approx: Option<u64>,
    #[serde(default)]
    format_note: Option<String>,
    #[serde(default)]
    protocol: Option<String>,
}

#[derive(Debug, Deserialize)]
struct YtEntry {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    webpage_url: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    uploader: Option<String>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    upload_date: Option<String>, // "YYYYMMDD"
    #[serde(default)]
    view_count: Option<u64>,
    #[serde(default)]
    thumbnails: Vec<serde_json::Value>,
    #[serde(default)]
    thumbnail: Option<String>,
}

// --- entry point ----------------------------------------------------------

pub async fn probe(app: &AppHandle, url: &str) -> AppResult<ProbeResult> {
    let site = sites::match_url(url);
    let cookies_file = match site {
        Some(s) => prepare_cookies(app, s.id).ok(),
        None => None,
    };
    let use_flat = site.map(|s| s.use_flat_playlist).unwrap_or(true);

    let json = run_yt_dlp_dump_json(app, url, cookies_file.as_deref(), use_flat).await?;
    let parsed: YtJson = serde_json::from_str(&json)
        .map_err(|e| AppError::Other(format!("yt-dlp JSON parse: {e}")))?;

    let is_collection = matches!(
        parsed.kind.as_deref(),
        Some("playlist") | Some("multi_video")
    ) || !parsed.entries.is_empty();

    if is_collection {
        let collection_kind = classify_collection(url);
        let entries: Vec<PlaylistEntry> = parsed.entries.iter().map(shape_entry).collect();
        Ok(ProbeResult::Playlist {
            title: parsed.title.unwrap_or_else(|| "未命名列表".into()),
            collection_kind,
            total: parsed.playlist_count.unwrap_or(entries.len()),
            entries,
        })
    } else {
        Ok(ProbeResult::SingleVideo(shape_video(parsed, site.map(|s| s.id))))
    }
}

fn classify_collection(url: &str) -> CollectionKind {
    if url.contains("/@")
        || url.contains("/channel/")
        || url.contains("/c/")
        || url.contains("/user/")
        || url.contains("space.bilibili.com")
    {
        CollectionKind::Channel
    } else {
        CollectionKind::Playlist
    }
}

fn shape_entry(e: &YtEntry) -> PlaylistEntry {
    let url = e
        .webpage_url
        .clone()
        .or_else(|| e.url.clone())
        .unwrap_or_default();
    let upload_date = e.upload_date.as_deref().and_then(|s| {
        if s.len() == 8 {
            Some(format!("{}-{}-{}", &s[..4], &s[4..6], &s[6..8]))
        } else {
            None
        }
    });
    let thumb = e.thumbnail.clone().or_else(|| {
        // Pick the largest thumbnail (last element in yt-dlp's array convention)
        e.thumbnails
            .last()
            .and_then(|v| v.get("url"))
            .and_then(|s| s.as_str())
            .map(String::from)
    });
    PlaylistEntry {
        id: e.id.clone().unwrap_or_default(),
        url,
        title: e.title.clone().unwrap_or_default(),
        duration_s: e.duration.map(|d| d as u32),
        uploader: e.uploader.clone().or_else(|| e.channel.clone()),
        upload_date,
        view_count: e.view_count,
        thumbnail: thumb,
    }
}

fn shape_video(yt: YtJson, site_id: Option<&str>) -> VideoInfo {
    let (mut combined, mut vonly, mut aonly) = (vec![], vec![], vec![]);
    for f in yt.formats {
        let vcodec = f.vcodec.clone().unwrap_or_else(|| "none".into());
        let acodec = f.acodec.clone().unwrap_or_else(|| "none".into());
        let has_v = vcodec != "none" && !vcodec.is_empty();
        let has_a = acodec != "none" && !acodec.is_empty();

        // Skip storyboards/images
        if let Some(proto) = &f.protocol {
            if proto == "mhtml" {
                continue;
            }
        }

        let stream = Stream {
            format_id: f.format_id,
            ext: f.ext.unwrap_or_default(),
            height: f.height,
            fps: f.fps,
            vcodec,
            acodec,
            tbr_kbps: f.tbr,
            filesize_bytes: f.filesize.or(f.filesize_approx),
            note: f.format_note.unwrap_or_default(),
        };

        match (has_v, has_a) {
            (true, true) => combined.push(stream),
            (true, false) => vonly.push(stream),
            (false, true) => aonly.push(stream),
            _ => {}
        }
    }

    vonly.sort_by(|a, b| b.height.unwrap_or(0).cmp(&a.height.unwrap_or(0)));
    combined.sort_by(|a, b| b.height.unwrap_or(0).cmp(&a.height.unwrap_or(0)));
    aonly.sort_by(|a, b| {
        b.tbr_kbps
            .unwrap_or(0.0)
            .partial_cmp(&a.tbr_kbps.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let subtitles = collect_subs(&yt.subtitles);
    let auto_subtitles = collect_subs(&yt.automatic_captions);

    VideoInfo {
        id: yt.id.unwrap_or_default(),
        title: yt.title.unwrap_or_default(),
        uploader: yt.uploader.or(yt.channel),
        duration_s: yt.duration.map(|d| d as u32),
        thumbnail: yt.thumbnail,
        combined_streams: combined,
        video_streams: vonly,
        audio_streams: aonly,
        subtitles,
        auto_subtitles,
        site_id: site_id.map(String::from),
    }
}

fn collect_subs(map: &serde_json::Map<String, serde_json::Value>) -> Vec<Subtitle> {
    let mut out: Vec<Subtitle> = map
        .iter()
        .map(|(lang, entries)| {
            let name = entries
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|v| v.get("name"))
                .and_then(|n| n.as_str())
                .map(String::from);
            Subtitle {
                lang: lang.clone(),
                name,
            }
        })
        .collect();
    out.sort_by(|a, b| a.lang.cmp(&b.lang));
    out
}

// --- subprocess plumbing --------------------------------------------------

async fn run_yt_dlp_dump_json(
    app: &AppHandle,
    url: &str,
    cookies_file: Option<&std::path::Path>,
    use_flat_playlist: bool,
) -> AppResult<String> {
    let mut args: Vec<String> = vec!["-J".into(), "--no-warnings".into()];
    if use_flat_playlist {
        args.push("--flat-playlist".into());
    }
    // Send extractor status lines to stderr so we can stream them as
    // "probe:status" events to the UI (otherwise long channel probes look
    // frozen for 10-30s).
    args.push("--progress".into());
    if let Some(c) = cookies_file {
        args.push("--cookies".into());
        args.push(c.display().to_string());
    }
    // Apply user-configured proxy if any
    let settings = app.state::<SettingsStore>().get();
    if !settings.proxy.trim().is_empty() {
        args.push("--proxy".into());
        args.push(settings.proxy.trim().into());
    }
    args.push(url.into());

    let _ = app.emit("probe:status", "正在启动 yt-dlp…");

    let cmd = crate::core::download::yt_dlp_command(app)?.args(&args);
    let (mut rx, _child) = cmd
        .spawn()
        .map_err(|e| AppError::Other(format!("spawn yt-dlp: {e}")))?;

    let mut stdout = String::new();
    let mut stderr = String::new();
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(b) => stdout.push_str(&String::from_utf8_lossy(&b)),
            CommandEvent::Stderr(b) => {
                let chunk = String::from_utf8_lossy(&b);
                for line in chunk.lines() {
                    if let Some(msg) = friendly_probe_status(line) {
                        let _ = app.emit("probe:status", msg);
                    }
                }
                stderr.push_str(&chunk);
            }
            CommandEvent::Terminated(p) => {
                if p.code != Some(0) {
                    return Err(AppError::Other(format!(
                        "yt-dlp exit {:?}: {}",
                        p.code,
                        stderr.lines().last().unwrap_or("").trim()
                    )));
                }
                break;
            }
            _ => {}
        }
    }
    let _ = stderr;
    let _ = app.emit::<&str>("probe:status", "");
    Ok(stdout)
}

/// Translate a yt-dlp stderr/status line into a short Chinese phrase we want
/// to show in the UI. Returns None for noise we'd rather not surface.
fn friendly_probe_status(line: &str) -> Option<String> {
    let l = line.trim();
    if l.is_empty() {
        return None;
    }
    // Status lines look like "[youtube:tab] Foo: Downloading webpage"
    // or "[youtube:tab] Playlist Foo: Downloading 100 items".
    if l.starts_with('[') {
        if let Some(rest) = l.splitn(2, ']').nth(1) {
            let rest = rest.trim();
            if rest.contains("Downloading webpage") {
                return Some("正在加载网页…".into());
            }
            if rest.contains("Downloading API JSON") {
                return Some("正在请求 API…".into());
            }
            if let Some(idx) = rest.find("Downloading ") {
                // Show the entire downloading-X-items phrase
                return Some(format!("{}…", &rest[idx..]));
            }
            if rest.contains("Extracting URL") {
                return Some("正在解析 URL…".into());
            }
            return Some(rest.to_string());
        }
    }
    None
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
