use crate::core::update_source::UpdateSource;
use crate::core::ytdlp_update::{self, UpdateInfo};
use crate::error::{AppError, AppResult};
use tauri::AppHandle;

#[tauri::command]
pub async fn check_ytdlp_update(
    app: AppHandle,
    source: UpdateSource,
) -> AppResult<Option<UpdateInfo>> {
    ytdlp_update::check(&app, source).await
}

#[tauri::command]
pub async fn install_ytdlp_update(app: AppHandle, source: UpdateSource) -> AppResult<String> {
    let info = ytdlp_update::check(&app, source)
        .await?
        .ok_or_else(|| AppError::Other(format!("{}没有可安装的 yt-dlp 新版", source.label())))?;
    let path = ytdlp_update::install(&app, &info).await?;
    Ok(path.display().to_string())
}
