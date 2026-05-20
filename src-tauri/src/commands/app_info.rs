//! Version + build number for the About display.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AppVersion {
    pub version: String,
    pub build: String,
}

#[tauri::command]
pub fn app_version() -> AppVersion {
    AppVersion {
        version: env!("CARGO_PKG_VERSION").to_string(),
        build: env!("APP_BUILD_NUMBER").to_string(),
    }
}
