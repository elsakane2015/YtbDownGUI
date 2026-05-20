//! Check GitHub for a newer yt-dlp release than the one we have bundled and
//! optionally download it into `$APP_DATA/bin/yt-dlp` on demand.
//!
//! Per the M5 spec: we do NOT auto-update. We just emit a UI event when a
//! newer release exists and let the user click "更新".

use crate::core::settings::SettingsStore;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::CommandEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub release_url: String,
    pub asset_url: String,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

/// Run the version check. Emits `ytdlp-update:available` with `UpdateInfo`
/// when a newer release exists; emits `ytdlp-update:up-to-date` otherwise.
pub async fn check(app: &AppHandle) -> AppResult<Option<UpdateInfo>> {
    let current = match current_version(app).await {
        Ok(v) if !v.is_empty() => v,
        Ok(_) => {
            eprintln!("[ytdlp-update] current version is empty (sidecar emitted no stdout); skipping check");
            return Ok(None);
        }
        Err(e) => {
            eprintln!("[ytdlp-update] failed to read current version ({e}); skipping check");
            return Ok(None);
        }
    };

    let client = build_http_client(app, Duration::from_secs(15))?;

    let resp = client
        .get("https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest")
        .send()
        .await
        .map_err(|e| AppError::Other(format!("github request: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::Other(format!(
            "github status {}",
            resp.status()
        )));
    }
    let release: GhRelease = resp
        .json()
        .await
        .map_err(|e| AppError::Other(format!("github json: {e}")))?;

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == "yt-dlp_macos")
        .ok_or_else(|| AppError::Other("no yt-dlp_macos asset in latest release".into()))?;

    if release.tag_name == current {
        let _ = app.emit("ytdlp-update:up-to-date", &current);
        return Ok(None);
    }

    let info = UpdateInfo {
        current,
        latest: release.tag_name.clone(),
        release_url: release.html_url.clone(),
        asset_url: asset.browser_download_url.clone(),
    };
    let _ = app.emit("ytdlp-update:available", &info);
    Ok(Some(info))
}

/// Download the asset to `$APP_DATA/bin/yt-dlp` and make it executable.
/// On the next run, the binary manager prefers this path over the sidecar.
pub async fn install(app: &AppHandle, info: &UpdateInfo) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Other(format!("app_data_dir: {e}")))?
        .join("bin");
    std::fs::create_dir_all(&dir)?;
    let out_path = dir.join("yt-dlp");

    let client = build_http_client(app, Duration::from_secs(300))?;
    let resp = client
        .get(&info.asset_url)
        .send()
        .await
        .map_err(|e| AppError::Other(format!("download: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::Other(format!(
            "download status {}",
            resp.status()
        )));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::Other(format!("download body: {e}")))?;
    let tmp = out_path.with_extension("tmp");
    std::fs::write(&tmp, &bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = std::fs::metadata(&tmp)?.permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(&tmp, perm)?;
    }
    std::fs::rename(&tmp, &out_path)?;
    let _ = app.emit("ytdlp-update:installed", &info.latest);
    Ok(out_path)
}

/// Build a reqwest client that respects the user's proxy setting when
/// `ytdlp_update_use_proxy` is on. Falls back to direct connection on any
/// proxy-URL parse error so a malformed proxy can't break update checks.
fn build_http_client(
    app: &AppHandle,
    timeout: Duration,
) -> AppResult<reqwest::Client> {
    let settings = app.state::<SettingsStore>().get();
    let mut builder = reqwest::Client::builder()
        .user_agent("YtbDownGUI/0.1 (+https://github.com/elsakane2015/YtbDownGUI)")
        .timeout(timeout);
    if settings.ytdlp_update_use_proxy && !settings.proxy.trim().is_empty() {
        match reqwest::Proxy::all(settings.proxy.trim()) {
            Ok(proxy) => builder = builder.proxy(proxy),
            Err(e) => eprintln!("[ytdlp-update] proxy parse failed: {e}; going direct"),
        }
    }
    builder
        .build()
        .map_err(|e| AppError::Other(format!("reqwest build: {e}")))
}

/// Resolve the current yt-dlp version. Prefers the user-installed (auto-
/// updated) binary at `$APP_DATA/bin/yt-dlp` over the bundled sidecar so
/// the "update available" banner doesn't keep firing forever after the
/// user has already updated. Returns the trimmed first stdout line.
async fn current_version(app: &AppHandle) -> AppResult<String> {
    let cmd = crate::core::download::yt_dlp_command(app)?.args(["--version"]);
    let (mut rx, _child) = cmd
        .spawn()
        .map_err(|e| AppError::Other(format!("spawn: {e}")))?;
    let mut out = String::new();
    let mut err = String::new();
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(b) => out.push_str(&String::from_utf8_lossy(&b)),
            CommandEvent::Stderr(b) => err.push_str(&String::from_utf8_lossy(&b)),
            CommandEvent::Terminated(_) => break,
            _ => {}
        }
    }
    // PyInstaller occasionally pushes its banner to stderr; if stdout was
    // empty (buffer flush quirks) we fall back to scanning stderr for a
    // version-shaped line.
    let line = out
        .lines()
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            err.lines()
                .find(|l| {
                    l.chars().take(8).all(|c| c.is_ascii_digit() || c == '.')
                        && l.contains('.')
                })
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_default();
    Ok(line)
}
