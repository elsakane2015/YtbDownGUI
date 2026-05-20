//! Version + build number + platform identification for the UI.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AppVersion {
    pub version: String,
    pub build: String,
    /// "macos" / "windows" / "linux". Front-end uses this to gate
    /// platform-specific UI bits (traffic-light spacing, native vs
    /// custom title bar, etc.).
    pub platform: String,
}

#[tauri::command]
pub fn app_version() -> AppVersion {
    AppVersion {
        version: env!("CARGO_PKG_VERSION").to_string(),
        build: env!("APP_BUILD_NUMBER").to_string(),
        platform: std::env::consts::OS.to_string(),
    }
}
