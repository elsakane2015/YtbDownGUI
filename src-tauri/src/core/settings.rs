//! User-configurable settings persisted as JSON in `$APP_DATA/settings.json`.
//!
//! Plain `serde_json` rather than a plugin — settings is one small file we
//! load on startup and rewrite atomically when changed.

use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub download_dir: String,
    pub max_concurrency: usize,
    pub default_quality: DefaultQuality,
    /// `http://host:port` / `https://host:port` / `socks5://host:port` etc.
    /// Empty string means no proxy.
    #[serde(default)]
    pub proxy: String,
    /// If true, check GitHub for newer yt-dlp on startup.
    #[serde(default = "default_true")]
    pub auto_check_ytdlp_updates: bool,
    /// If true, the yt-dlp update check and download go through the same
    /// `proxy` setting (when one is configured). On networks where GitHub
    /// is reachable directly but YouTube is not, set this to false.
    #[serde(default = "default_true")]
    pub ytdlp_update_use_proxy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultQuality {
    /// None = no cap (download the highest available).
    pub max_height: Option<u32>,
    /// Empty string = any codec.
    pub prefer_codec: String,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        let download_dir = if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home)
                .join("Downloads")
                .join("YtbDownGUI")
                .to_string_lossy()
                .into_owned()
        } else {
            "/tmp/YtbDownGUI".to_string()
        };
        Self {
            download_dir,
            max_concurrency: 2,
            default_quality: DefaultQuality {
                max_height: Some(1080),
                prefer_codec: "avc1".into(),
            },
            proxy: String::new(),
            auto_check_ytdlp_updates: true,
            ytdlp_update_use_proxy: true,
        }
    }
}

/// Thread-safe in-memory cache of settings, backed by disk.
pub struct SettingsStore {
    path: PathBuf,
    state: Mutex<Settings>,
}

impl SettingsStore {
    pub fn load(app_data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(app_data_dir)?;
        let path = app_data_dir.join("settings.json");
        let settings = if path.exists() {
            let bytes = std::fs::read(&path)?;
            // Best-effort parse: fall back to defaults on schema mismatch so
            // an old version's settings.json never blocks app start.
            serde_json::from_slice::<Settings>(&bytes).unwrap_or_else(|e| {
                eprintln!("[settings] parse error: {e}, using defaults");
                Settings::default()
            })
        } else {
            Settings::default()
        };
        Ok(Self {
            path,
            state: Mutex::new(settings),
        })
    }

    pub fn get(&self) -> Settings {
        self.state.lock().unwrap().clone()
    }

    pub fn update<F>(&self, mutator: F) -> AppResult<Settings>
    where
        F: FnOnce(&mut Settings),
    {
        let snapshot = {
            let mut s = self.state.lock().unwrap();
            mutator(&mut *s);
            s.clone()
        };
        self.persist(&snapshot)?;
        Ok(snapshot)
    }

    fn persist(&self, s: &Settings) -> AppResult<()> {
        let bytes = serde_json::to_vec_pretty(s)?;
        // Atomic-ish write: tmp then rename.
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}
