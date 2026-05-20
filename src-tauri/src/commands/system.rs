//! Native-shell integration: open a path in Finder, reveal a file selected,
//! or open an external URL in the system browser. macOS-only for now.

use crate::error::{AppError, AppResult};
use std::path::Path;
use std::process::Command;

/// Open an external URL in the system default browser.
/// Separate from `open_path` because URLs don't have a filesystem existence
/// check (which would falsely reject every URL we pass in).
#[tauri::command]
pub fn open_url(url: String) -> AppResult<()> {
    // Only allow http(s) URLs to avoid the IPC turning into a generic
    // arbitrary-command launcher.
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(AppError::Other(format!("not an http(s) URL: {url}")));
    }
    Command::new("open")
        .arg(&url)
        .status()
        .map_err(|e| AppError::Other(format!("open url: {e}")))?;
    Ok(())
}

/// Open a file or directory (delegates to macOS `open <path>`).
#[tauri::command]
pub fn open_path(path: String) -> AppResult<()> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(AppError::Other(format!("path not found: {path}")));
    }
    Command::new("open")
        .arg(&path)
        .status()
        .map_err(|e| AppError::Other(format!("open: {e}")))?;
    Ok(())
}

/// Reveal a file in Finder with it selected (`open -R <path>`).
#[tauri::command]
pub fn reveal_in_finder(path: String) -> AppResult<()> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(AppError::Other(format!("path not found: {path}")));
    }
    Command::new("open")
        .arg("-R")
        .arg(&path)
        .status()
        .map_err(|e| AppError::Other(format!("open -R: {e}")))?;
    Ok(())
}
