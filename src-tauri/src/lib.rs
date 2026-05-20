mod commands;
mod core;
mod error;

use crate::core::download::QueueManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(QueueManager::new(2))
        .invoke_handler(tauri::generate_handler![
            commands::probe::probe_tool_versions,
            commands::probe::probe,
            commands::accounts::list_accounts,
            commands::accounts::start_login,
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
            commands::download::default_download_dir,
            commands::system::open_path,
            commands::system::reveal_in_finder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
