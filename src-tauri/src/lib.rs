mod commands;
mod core;
mod error;

use crate::core::download::QueueManager;
use crate::core::entitlement::EntitlementStore;
use crate::core::paths;
use crate::core::settings::SettingsStore;
use std::time::Duration;
use tauri::{Emitter, Manager};

pub fn verify_embedded_license_config() -> Result<(), String> {
    core::entitlement::validate_embedded_public_key()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data_dir = paths::data_dir(&app.handle()).map_err(|e| e.to_string())?;
            crate::core::log::init(&data_dir);
            let settings = SettingsStore::load(&data_dir).map_err(|e| e.to_string())?;
            let concurrency = settings.get().max_concurrency;
            app.manage(settings);
            let entitlement = EntitlementStore::load(&data_dir).map_err(|e| e.to_string())?;
            app.manage(entitlement);

            let queue = QueueManager::new(concurrency);
            // Restore the previous run's job history so the user doesn't lose
            // their list on app restart.
            queue.restore_from_disk(&data_dir);
            app.manage(queue);
            app.handle()
                .state::<QueueManager>()
                .reconcile_quota_reservations(&app.handle());

            // Renew Pro before the seven-day token lifetime runs out. Check
            // shortly after startup and retry every six hours while the token
            // is inside its refresh window. A failed refresh never clears a
            // still-valid local token.
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    loop {
                        let refresh_due = app_handle
                            .state::<EntitlementStore>()
                            .pro_refresh_due()
                            .unwrap_or(false);
                        if refresh_due {
                            match app_handle.state::<EntitlementStore>().refresh_pro().await {
                                Ok(status) => {
                                    let _ = app_handle.emit("entitlement:updated", status);
                                }
                                Err(error) => {
                                    eprintln!("[entitlement] background refresh failed: {error}");
                                }
                            }
                        }
                        tokio::time::sleep(Duration::from_secs(6 * 60 * 60)).await;
                    }
                });
            }

            // Background task: persist job history every 5s so a hard crash
            // loses at most a few seconds of state. The explicit on-exit
            // hook below handles graceful quit.
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        if let Ok(dir) = paths::data_dir(&app_handle) {
                            let queue = app_handle.state::<QueueManager>();
                            let _ = queue.persist_to_disk(&dir);
                        }
                    }
                });
            }

            // Background task: check yt-dlp on startup and every 24 hours.
            // Prefer the official YtbDownGUI mirror and fall back to GitHub
            // when the mirror is temporarily unavailable.
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    loop {
                        if app_handle
                            .state::<SettingsStore>()
                            .get()
                            .auto_check_ytdlp_updates
                        {
                            let official = crate::core::ytdlp_update::check(
                                &app_handle,
                                crate::core::update_source::UpdateSource::Official,
                            )
                            .await;
                            if official.is_err() {
                                let _ = crate::core::ytdlp_update::check(
                                    &app_handle,
                                    crate::core::update_source::UpdateSource::Github,
                                )
                                .await;
                            }
                        }
                        tokio::time::sleep(Duration::from_secs(24 * 60 * 60)).await;
                    }
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Persist jobs synchronously when the user closes the last window
            // so nothing is lost between the 5-second autosave ticks.
            if let tauri::WindowEvent::Destroyed = event {
                let app_handle = window.app_handle().clone();
                if let Ok(dir) = paths::data_dir(&app_handle) {
                    let queue = app_handle.state::<QueueManager>();
                    let _ = queue.persist_to_disk(&dir);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::probe::probe_tool_versions,
            commands::probe::probe,
            commands::probe::list_supported_sites,
            commands::accounts::list_accounts,
            commands::accounts::start_login,
            commands::accounts::start_login_by_url,
            commands::accounts::finish_login,
            commands::accounts::cancel_login,
            commands::accounts::logout,
            commands::accounts::export_cookies_netscape,
            commands::download::enqueue_download,
            commands::download::enqueue_batch,
            commands::download::list_jobs,
            commands::download::cancel_job,
            commands::download::cancel_batch,
            commands::download::clear_finished,
            commands::download::remove_job,
            commands::download::default_download_dir,
            commands::entitlement::get_entitlement_status,
            commands::entitlement::activate_pro,
            commands::entitlement::refresh_pro,
            commands::entitlement::deactivate_pro,
            commands::entitlement::send_transfer_code,
            commands::entitlement::activate_with_transfer_code,
            commands::entitlement::create_checkout_session,
            commands::entitlement::resend_license,
            commands::entitlement::get_support_contact,
            commands::entitlement::sync_free_quota_status,
            commands::entitlement::reserve_free_quota,
            commands::entitlement::confirm_free_quota,
            commands::entitlement::release_free_quota,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::system::open_path,
            commands::system::open_url,
            commands::system::reveal_in_finder,
            commands::ytdlp_update::check_ytdlp_update,
            commands::ytdlp_update::install_ytdlp_update,
            commands::app_update::check_app_update,
            commands::app_update::install_app_update,
            commands::app_info::app_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
