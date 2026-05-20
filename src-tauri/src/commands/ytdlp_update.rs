use crate::core::ytdlp_update::{self, UpdateInfo};
use crate::error::AppResult;
use tauri::AppHandle;

#[tauri::command]
pub async fn check_ytdlp_update(app: AppHandle) -> AppResult<Option<UpdateInfo>> {
    ytdlp_update::check(&app).await
}

#[tauri::command]
pub async fn install_ytdlp_update(
    app: AppHandle,
    info: UpdateInfo,
) -> AppResult<String> {
    let path = ytdlp_update::install(&app, &info).await?;
    Ok(path.display().to_string())
}
