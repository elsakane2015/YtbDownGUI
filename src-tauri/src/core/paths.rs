//! Cross-platform path helpers.
//!
//! - **macOS**: data lives in the standard `~/Library/Application Support/<id>/`.
//! - **Windows**: portable-mode aware — if a file named `portable.txt` sits
//!   alongside the running .exe, data lives in `<exe-dir>/data/`. Without
//!   that flag we fall back to the platform's `%APPDATA%\<id>\`. This lets
//!   the same zip work as a true "drop and run" portable bundle.
//! - **Linux**: standard XDG data dir.

use crate::error::{AppError, AppResult};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Where settings.json, jobs.json, cookies/, bin/, tmp/ live.
pub fn data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Some(portable) = portable_data_dir() {
            std::fs::create_dir_all(&portable).ok();
            return Ok(portable);
        }
    }
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Other(format!("app_data_dir: {e}")))?;
    std::fs::create_dir_all(&dir).ok();
    Ok(dir)
}

/// Returns Some(<exe-dir>/data) iff `<exe-dir>/portable.txt` exists.
/// Always None on non-Windows builds.
#[cfg(target_os = "windows")]
fn portable_data_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?.to_path_buf();
    if exe_dir.join("portable.txt").exists() {
        Some(exe_dir.join("data"))
    } else {
        None
    }
}

/// Sidecar binary suffix per host triple, including .exe on Windows.
pub fn sidecar_filename(name: &str) -> String {
    let triple = std::env::consts::ARCH;
    let suffix = match (triple, std::env::consts::OS) {
        ("aarch64", "macos") => "aarch64-apple-darwin",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("x86_64", "windows") => "x86_64-pc-windows-msvc",
        ("aarch64", "windows") => "aarch64-pc-windows-msvc",
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu",
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu",
        _ => "aarch64-apple-darwin",
    };
    let ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    format!("{name}-{suffix}{ext}")
}
