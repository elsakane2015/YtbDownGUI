//! Check for a newer yt-dlp and install it into `$APP_DATA/bin`.
//! Both GitHub and the YtbDownGUI official mirror are supported. Every
//! downloaded binary is SHA-256 verified and executed before it replaces the
//! last known-good copy.

use crate::core::settings::SettingsStore;
use crate::core::update_source::UpdateSource;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::CommandEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub release_url: String,
    pub asset_url: String,
    pub sha256: String,
    pub source: UpdateSource,
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

#[derive(Debug, Deserialize)]
struct OfficialUpdate {
    current: String,
    latest: String,
    release_url: String,
    asset_url: String,
    sha256: String,
}

pub async fn check(app: &AppHandle, source: UpdateSource) -> AppResult<Option<UpdateInfo>> {
    let current = match current_version(app).await {
        Ok(version) if !version.is_empty() => version,
        Ok(_) => return Ok(None),
        Err(error) => {
            eprintln!("[ytdlp-update] failed to read current version ({error}); skipping check");
            return Ok(None);
        }
    };

    let info = match source {
        UpdateSource::Official => check_official(app, &current).await?,
        UpdateSource::Github => check_github(app, &current).await?,
    };
    if let Some(info) = &info {
        let _ = app.emit("ytdlp-update:available", info);
    } else {
        let _ = app.emit("ytdlp-update:up-to-date", &current);
    }
    Ok(info)
}

async fn check_official(app: &AppHandle, current: &str) -> AppResult<Option<UpdateInfo>> {
    let target = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else {
        std::env::consts::ARCH
    };
    let url = format!("https://upgrade.ytbdown.litotime.com/v1/ytdlp/{target}/{arch}/{current}");
    let response = build_http_client(app, Duration::from_secs(20))?
        .get(url)
        .send()
        .await
        .map_err(|error| AppError::Other(format!("官方源请求失败：{error}")))?;
    if response.status() == reqwest::StatusCode::NO_CONTENT {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(AppError::Other(format!(
            "官方源返回 HTTP {}",
            response.status()
        )));
    }
    let update: OfficialUpdate = response
        .json()
        .await
        .map_err(|error| AppError::Other(format!("官方源数据无效：{error}")))?;
    Ok(Some(UpdateInfo {
        current: update.current,
        latest: update.latest,
        release_url: update.release_url,
        asset_url: update.asset_url,
        sha256: update.sha256,
        source: UpdateSource::Official,
    }))
}

async fn check_github(app: &AppHandle, current: &str) -> AppResult<Option<UpdateInfo>> {
    let client = build_http_client(app, Duration::from_secs(20))?;
    let response = client
        .get("https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest")
        .send()
        .await
        .map_err(|error| AppError::Other(format!("GitHub 请求失败：{error}")))?;
    if !response.status().is_success() {
        return Err(AppError::Other(format!(
            "GitHub 返回 HTTP {}",
            response.status()
        )));
    }
    let release: GhRelease = response
        .json()
        .await
        .map_err(|error| AppError::Other(format!("GitHub 数据无效：{error}")))?;
    if release.tag_name.trim_start_matches('v') == current.trim_start_matches('v') {
        return Ok(None);
    }

    let asset_name = platform_asset_name();
    let asset = release
        .assets
        .iter()
        .find(|candidate| candidate.name == asset_name)
        .ok_or_else(|| AppError::Other(format!("GitHub 发布缺少 {asset_name}")))?;
    let checksum_asset = release
        .assets
        .iter()
        .find(|candidate| candidate.name == "SHA2-256SUMS")
        .ok_or_else(|| AppError::Other("GitHub 发布缺少 SHA2-256SUMS".into()))?;
    let checksums = client
        .get(&checksum_asset.browser_download_url)
        .send()
        .await
        .map_err(|error| AppError::Other(format!("下载校验文件失败：{error}")))?
        .error_for_status()
        .map_err(|error| AppError::Other(format!("下载校验文件失败：{error}")))?
        .text()
        .await
        .map_err(|error| AppError::Other(format!("读取校验文件失败：{error}")))?;
    let sha256 = checksum_for(&checksums, asset_name)
        .ok_or_else(|| AppError::Other(format!("校验文件中没有 {asset_name}")))?;

    Ok(Some(UpdateInfo {
        current: current.to_string(),
        latest: release.tag_name.trim_start_matches('v').to_string(),
        release_url: release.html_url,
        asset_url: asset.browser_download_url.clone(),
        sha256,
        source: UpdateSource::Github,
    }))
}

fn checksum_for(checksums: &str, asset_name: &str) -> Option<String> {
    checksums.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let checksum = parts.next()?;
        let filename = parts.next()?.trim_start_matches('*');
        (filename == asset_name && checksum.len() == 64).then(|| checksum.to_ascii_lowercase())
    })
}

fn platform_asset_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "yt-dlp_macos"
    } else {
        "yt-dlp"
    }
}

pub async fn install(app: &AppHandle, info: &UpdateInfo) -> AppResult<PathBuf> {
    let dir = crate::core::paths::data_dir(app)?.join("bin");
    std::fs::create_dir_all(&dir)?;
    let out_path = dir.join(if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    });

    let response = build_http_client(app, Duration::from_secs(300))?
        .get(&info.asset_url)
        .send()
        .await
        .map_err(|error| AppError::Other(format!("下载失败：{error}")))?;
    if !response.status().is_success() {
        return Err(AppError::Other(format!(
            "下载返回 HTTP {}",
            response.status()
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| AppError::Other(format!("读取下载内容失败：{error}")))?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual != info.sha256.to_ascii_lowercase() {
        return Err(AppError::Other(format!(
            "yt-dlp 校验失败：期望 {}，实际 {actual}",
            info.sha256
        )));
    }

    let temporary = out_path.with_extension("download");
    let backup = out_path.with_extension("backup");
    std::fs::write(&temporary, &bytes)?;
    make_executable(&temporary)?;
    validate_binary(&temporary, &info.latest)?;

    let _ = std::fs::remove_file(&backup);
    if out_path.exists() {
        std::fs::rename(&out_path, &backup)?;
    }
    if let Err(error) = std::fs::rename(&temporary, &out_path) {
        if backup.exists() {
            let _ = std::fs::rename(&backup, &out_path);
        }
        return Err(error.into());
    }
    let _ = std::fs::remove_file(&backup);
    let _ = app.emit("ytdlp-update:installed", &info.latest);
    Ok(out_path)
}

fn make_executable(path: &Path) -> AppResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn validate_binary(path: &Path, expected_version: &str) -> AppResult<()> {
    let output = std::process::Command::new(path)
        .arg("--version")
        .output()
        .map_err(|error| AppError::Other(format!("无法运行新版 yt-dlp：{error}")))?;
    let actual = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success()
        || actual.trim_start_matches('v') != expected_version.trim_start_matches('v')
    {
        return Err(AppError::Other(format!(
            "新版 yt-dlp 自检失败：期望 {expected_version}，实际 {}",
            if actual.is_empty() { "未知" } else { &actual }
        )));
    }
    Ok(())
}

fn build_http_client(app: &AppHandle, timeout: Duration) -> AppResult<reqwest::Client> {
    let settings = app.state::<SettingsStore>().get();
    let mut builder = reqwest::Client::builder()
        .user_agent("YtbDownGUI/1 (+https://github.com/elsakane2015/YtbDownGUI)")
        .timeout(timeout);
    if settings.ytdlp_update_use_proxy && !settings.proxy.trim().is_empty() {
        match reqwest::Proxy::all(settings.proxy.trim()) {
            Ok(proxy) => builder = builder.proxy(proxy),
            Err(error) => eprintln!("[ytdlp-update] proxy parse failed: {error}; going direct"),
        }
    }
    builder
        .build()
        .map_err(|error| AppError::Other(format!("网络初始化失败：{error}")))
}

async fn current_version(app: &AppHandle) -> AppResult<String> {
    let command = crate::core::download::yt_dlp_command(app)?.args(["--version"]);
    let (mut receiver, _child) = command
        .spawn()
        .map_err(|error| AppError::Other(format!("启动 yt-dlp 失败：{error}")))?;
    let mut stdout = String::new();
    let mut stderr = String::new();
    while let Some(event) = receiver.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => stdout.push_str(&String::from_utf8_lossy(&bytes)),
            CommandEvent::Stderr(bytes) => stderr.push_str(&String::from_utf8_lossy(&bytes)),
            CommandEvent::Terminated(_) => break,
            _ => {}
        }
    }
    Ok(stdout
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .or_else(|| {
            stderr
                .lines()
                .find(|line| line.contains('.'))
                .map(str::trim)
        })
        .unwrap_or_default()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::checksum_for;

    #[test]
    fn extracts_checksum_for_requested_asset() {
        let checksums = "aaaa yt-dlp\n0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef *yt-dlp.exe\n";
        assert_eq!(
            checksum_for(checksums, "yt-dlp.exe").as_deref(),
            Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        );
    }
}
