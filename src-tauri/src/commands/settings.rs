//! IPC commands for reading and updating user settings.

use crate::core::settings::{Settings, SettingsStore};
use crate::error::AppResult;
use serde::Deserialize;
use tauri::{Emitter, State};

#[tauri::command]
pub fn get_settings(store: State<'_, SettingsStore>) -> Settings {
    store.get()
}

/// Partial update — only fields present in the patch are changed.
#[derive(Debug, Deserialize)]
pub struct SettingsPatch {
    pub download_dir: Option<String>,
    pub max_concurrency: Option<usize>,
    pub default_quality_max_height: Option<Option<u32>>,
    pub default_quality_prefer_codec: Option<String>,
    pub proxy: Option<String>,
    pub auto_check_ytdlp_updates: Option<bool>,
    pub ytdlp_update_use_proxy: Option<bool>,
}

#[tauri::command]
pub fn update_settings(
    app: tauri::AppHandle,
    store: State<'_, SettingsStore>,
    patch: SettingsPatch,
) -> AppResult<Settings> {
    let updated = store.update(|s| {
        if let Some(v) = patch.download_dir {
            s.download_dir = v;
        }
        if let Some(v) = patch.max_concurrency {
            s.max_concurrency = v.clamp(1, 16);
        }
        if let Some(v) = patch.default_quality_max_height {
            s.default_quality.max_height = v;
        }
        if let Some(v) = patch.default_quality_prefer_codec {
            s.default_quality.prefer_codec = v;
        }
        if let Some(v) = patch.proxy {
            s.proxy = v;
        }
        if let Some(v) = patch.auto_check_ytdlp_updates {
            s.auto_check_ytdlp_updates = v;
        }
        if let Some(v) = patch.ytdlp_update_use_proxy {
            s.ytdlp_update_use_proxy = v;
        }
    })?;
    let _ = app.emit("settings:updated", &updated);
    Ok(updated)
}
