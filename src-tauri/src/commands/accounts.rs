//! IPC commands related to per-site accounts: list, login, logout.

use crate::core::{cookies, login_window, sites};
use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Serialize)]
pub struct AccountStatus {
    pub site_id: String,
    pub display_name: String,
    pub logged_in: bool,
    pub cookie_count: usize,
}

#[tauri::command]
pub fn list_accounts(app: AppHandle) -> AppResult<Vec<AccountStatus>> {
    let data_dir = app_data_dir(&app)?;
    Ok(sites::SITES
        .iter()
        .map(|s| {
            let st = cookies::status(&data_dir, s.id, s.logged_in_marker_cookie);
            AccountStatus {
                site_id: s.id.to_string(),
                display_name: s.display_name.to_string(),
                logged_in: st.has_marker,
                cookie_count: st.cookie_count,
            }
        })
        .collect())
}

#[tauri::command]
pub fn start_login(app: AppHandle, site_id: String) -> AppResult<()> {
    let site = sites::find(&site_id).ok_or_else(|| AppError::UnknownSite(site_id.clone()))?;
    let _win = login_window::open(&app, site)?;
    Ok(())
}

#[tauri::command]
pub fn finish_login(app: AppHandle, site_id: String) -> AppResult<usize> {
    let site = sites::find(&site_id).ok_or_else(|| AppError::UnknownSite(site_id.clone()))?;
    let win = app
        .get_webview_window(login_window::LOGIN_WINDOW_LABEL)
        .ok_or_else(|| AppError::Other("login window not open".into()))?;

    let cookies = login_window::fetch_cookies(&win, site)?;
    let data_dir = app_data_dir(&app)?;
    cookies::save(&data_dir, site.id, &cookies)?;
    let _ = win.close();

    let _ = app.emit("account:updated", &site_id);
    Ok(cookies.len())
}

#[tauri::command]
pub fn cancel_login(app: AppHandle) -> AppResult<()> {
    login_window::close(&app)?;
    Ok(())
}

#[tauri::command]
pub fn logout(app: AppHandle, site_id: String) -> AppResult<()> {
    let site = sites::find(&site_id).ok_or_else(|| AppError::UnknownSite(site_id.clone()))?;
    let data_dir = app_data_dir(&app)?;
    cookies::delete(&data_dir, site.id)?;
    let _ = app.emit("account:updated", &site_id);
    Ok(())
}

/// Export the current cookies for a site as a Netscape cookies.txt in a
/// temp file. Returns the file path. Useful for piping into yt-dlp.
#[tauri::command]
pub fn export_cookies_netscape(app: AppHandle, site_id: String) -> AppResult<String> {
    let site = sites::find(&site_id).ok_or_else(|| AppError::UnknownSite(site_id.clone()))?;
    let data_dir = app_data_dir(&app)?;
    let cookies = cookies::load(&data_dir, site.id)?;
    let temp_dir = data_dir.join("tmp");
    std::fs::create_dir_all(&temp_dir)?;
    let out = temp_dir.join(format!("{}.cookies.txt", site.id));
    cookies::write_netscape(&cookies, &out)?;
    Ok(out.to_string_lossy().into_owned())
}

fn app_data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::Other(format!("app_data_dir: {e}")))
}
