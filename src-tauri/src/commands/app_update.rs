use crate::core::paths;
use crate::core::settings::SettingsStore;
use crate::core::update_source::UpdateSource;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use url::Url;

const OFFICIAL_ENDPOINT: &str =
    "https://upgrade.ytbdown.litotime.com/v1/app/pro/{{target}}/{{arch}}/{{current_version}}";
const GITHUB_RELEASES: &str =
    "https://api.github.com/repos/elsakane2015/YtbDownGUI/releases?per_page=30";

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppUpdateInfo {
    pub current: String,
    pub latest: String,
    pub notes: String,
    pub published_at: Option<String>,
    pub source: UpdateSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppUpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub finished: bool,
}

async fn endpoint(app: &AppHandle, source: UpdateSource) -> AppResult<Url> {
    let url = match source {
        UpdateSource::Official => OFFICIAL_ENDPOINT.to_string(),
        UpdateSource::Github => {
            let settings = app.state::<SettingsStore>().get();
            let mut client = reqwest::Client::builder()
                .user_agent("YtbDownGUI updater")
                .timeout(Duration::from_secs(20));
            if settings.ytdlp_update_use_proxy && !settings.proxy.trim().is_empty() {
                let proxy = reqwest::Proxy::all(settings.proxy.trim())
                    .map_err(|error| AppError::Other(format!("代理设置无效：{error}")))?;
                client = client.proxy(proxy);
            }
            let releases: Vec<GithubRelease> = client
                .build()
                .map_err(|error| AppError::Other(format!("网络初始化失败：{error}")))?
                .get(GITHUB_RELEASES)
                .send()
                .await
                .map_err(|error| AppError::Other(format!("读取 GitHub 发布失败：{error}")))?
                .error_for_status()
                .map_err(|error| AppError::Other(format!("读取 GitHub 发布失败：{error}")))?
                .json()
                .await
                .map_err(|error| AppError::Other(format!("GitHub 发布数据无效：{error}")))?;
            releases
                .into_iter()
                .filter(|release| {
                    release.tag_name.starts_with("pro-v") && !release.draft && !release.prerelease
                })
                .find_map(|release| {
                    release
                        .assets
                        .into_iter()
                        .find(|asset| asset.name == "latest.json")
                        .map(|asset| asset.browser_download_url)
                })
                .ok_or_else(|| AppError::Other("GitHub 尚未发布支持自动更新的 Pro 版本".into()))?
        }
    };
    Url::parse(&url).map_err(|error| AppError::Other(format!("更新地址无效：{error}")))
}

async fn available_update(app: &AppHandle, source: UpdateSource) -> AppResult<Option<Update>> {
    // Free and Pro releases share this repository but must never cross-update.
    if env!("APP_BUILD_CHANNEL") != "Pro" {
        return Ok(None);
    }
    let mut builder = app
        .updater_builder()
        .endpoints(vec![endpoint(app, source).await?])
        .map_err(|error| AppError::Other(format!("配置更新源失败：{error}")))?
        .timeout(Duration::from_secs(30));

    let settings = app.state::<SettingsStore>().get();
    if settings.ytdlp_update_use_proxy && !settings.proxy.trim().is_empty() {
        if let Ok(proxy) = Url::parse(settings.proxy.trim()) {
            builder = builder.proxy(proxy);
        }
    }

    builder
        .build()
        .map_err(|error| AppError::Other(format!("初始化更新器失败：{error}")))?
        .check()
        .await
        .map_err(|error| AppError::Other(format!("检查{}失败：{error}", source.label())))
}

#[tauri::command]
pub async fn check_app_update(
    app: AppHandle,
    source: UpdateSource,
) -> AppResult<Option<AppUpdateInfo>> {
    let Some(update) = available_update(&app, source).await? else {
        return Ok(None);
    };
    Ok(Some(AppUpdateInfo {
        current: update.current_version.clone(),
        latest: update.version.clone(),
        notes: update.body.clone().unwrap_or_default(),
        published_at: update.date.map(|date| date.to_string()),
        source,
    }))
}

#[tauri::command]
pub async fn install_app_update(app: AppHandle, source: UpdateSource) -> AppResult<()> {
    #[cfg(target_os = "windows")]
    if std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.join("portable.txt")))
        .is_some_and(|path| path.exists())
    {
        return Err(AppError::Other(
            "便携版无法由安装包原地更新；请下载新版便携 ZIP，替换程序文件并保留 data 文件夹。"
                .into(),
        ));
    }
    let Some(update) = available_update(&app, source).await? else {
        return Err(AppError::Other(format!(
            "{}没有可安装的新版本",
            source.label()
        )));
    };

    if let Ok(data_dir) = paths::data_dir(&app) {
        let queue = app.state::<crate::core::download::QueueManager>();
        let _ = queue.persist_to_disk(&data_dir);
    }

    let progress_app = app.clone();
    let finished_app = app.clone();
    let mut downloaded = 0_u64;
    update
        .download_and_install(
            move |chunk, total| {
                downloaded = downloaded.saturating_add(chunk as u64);
                let _ = progress_app.emit(
                    "app-update:progress",
                    AppUpdateProgress {
                        downloaded,
                        total,
                        finished: false,
                    },
                );
            },
            move || {
                let _ = finished_app.emit(
                    "app-update:progress",
                    AppUpdateProgress {
                        downloaded: 0,
                        total: None,
                        finished: true,
                    },
                );
            },
        )
        .await
        .map_err(|error| AppError::Other(format!("下载或安装更新失败：{error}")))?;

    #[cfg(not(target_os = "windows"))]
    app.restart();

    #[allow(unreachable_code)]
    Ok(())
}
