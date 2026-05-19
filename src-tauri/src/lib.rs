mod commands;
mod core;
mod error;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::probe::probe_tool_versions,
            commands::accounts::list_accounts,
            commands::accounts::start_login,
            commands::accounts::finish_login,
            commands::accounts::cancel_login,
            commands::accounts::logout,
            commands::accounts::export_cookies_netscape,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
