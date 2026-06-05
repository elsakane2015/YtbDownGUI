//! IPC commands related to dynamic accounts: list, login, logout.

use crate::core::{accounts as account_store, login_window};
use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Serialize)]
pub struct AccountStatus {
    pub account_id: String,
    pub display_name: String,
    pub login_url: String,
    pub primary_host: String,
    pub status: String,
    pub logged_in: bool,
    pub cookie_count: usize,
    pub known_site_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoginStartResult {
    pub account_id: String,
    pub display_name: String,
    pub login_url: String,
    pub manual_finish_required: bool,
}

#[tauri::command]
pub fn list_accounts(app: AppHandle) -> AppResult<Vec<AccountStatus>> {
    let data_dir = app_data_dir(&app)?;
    let accounts = account_store::list(&data_dir)?;
    Ok(accounts.into_iter().map(AccountStatus::from).collect())
}

#[tauri::command]
pub async fn start_login(app: AppHandle, account_id: String) -> AppResult<LoginStartResult> {
    let data_dir = app_data_dir(&app)?;
    let target = account_store::ensure_login_target_for_account(&data_dir, &account_id)?;
    let result = LoginStartResult::from(&target);
    let _win = login_window::open_target(&app, target)?;
    Ok(result)
}

#[tauri::command]
pub async fn start_login_by_url(app: AppHandle, url: String) -> AppResult<LoginStartResult> {
    let data_dir = app_data_dir(&app)?;
    let target = account_store::ensure_login_target_for_url(&data_dir, &url)?;
    let result = LoginStartResult::from(&target);
    let _win = login_window::open_target(&app, target)?;
    Ok(result)
}

#[tauri::command]
pub fn finish_login(app: AppHandle, account_id: String) -> AppResult<usize> {
    let win = app
        .get_webview_window(login_window::LOGIN_WINDOW_LABEL)
        .ok_or_else(|| AppError::Other("login window not open".into()))?;

    let cookies = login_window::fetch_all_cookies(&win)?;
    let data_dir = app_data_dir(&app)?;
    let record = account_store::save_login_cookies(
        &data_dir,
        &account_id,
        cookies,
        login_window::current_login_user_agent(),
    )?;
    login_window::mark_finished();
    let _ = win.close();

    let _ = app.emit("account:updated", &account_id);
    let _ = app.emit(
        "login:succeeded",
        login_window::LoginEventPayload {
            account_id,
            display_name: record.display_name,
            cookie_count: record.cookie_count,
        },
    );
    Ok(record.cookie_count)
}

#[tauri::command]
pub fn cancel_login(app: AppHandle) -> AppResult<()> {
    login_window::close(&app)?;
    Ok(())
}

#[tauri::command]
pub fn logout(app: AppHandle, account_id: String) -> AppResult<()> {
    let data_dir = app_data_dir(&app)?;
    account_store::logout(&data_dir, &account_id)?;
    let _ = app.emit("account:updated", &account_id);
    Ok(())
}

/// Export the current cookies for a site as a Netscape cookies.txt in a
/// temp file. Returns the file path. Useful for piping into yt-dlp.
#[tauri::command]
pub fn export_cookies_netscape(app: AppHandle, account_id: String) -> AppResult<String> {
    let data_dir = app_data_dir(&app)?;
    let out = account_store::export_cookies(&data_dir, &account_id)?;
    Ok(out.to_string_lossy().into_owned())
}

fn app_data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    crate::core::paths::data_dir(app)
}

impl From<account_store::AccountRecord> for AccountStatus {
    fn from(record: account_store::AccountRecord) -> Self {
        let logged_in = record.status == account_store::AccountState::LoggedIn;
        AccountStatus {
            account_id: record.account_id,
            display_name: record.display_name,
            login_url: record.login_url,
            primary_host: record.primary_host,
            status: match record.status {
                account_store::AccountState::LoggedIn => "logged_in".into(),
                account_store::AccountState::LoggedOut => "logged_out".into(),
            },
            logged_in,
            cookie_count: record.cookie_count,
            known_site_id: record.known_site_id,
        }
    }
}

impl From<&account_store::LoginTarget> for LoginStartResult {
    fn from(target: &account_store::LoginTarget) -> Self {
        LoginStartResult {
            account_id: target.account_id.clone(),
            display_name: target.display_name.clone(),
            login_url: target.login_url.clone(),
            manual_finish_required: target.manual_finish_required,
        }
    }
}
