//! Native-shell integration: open a path in Finder, reveal a file selected.
//! macOS-only for now.

use crate::error::{AppError, AppResult};
use std::path::Path;
use std::process::Command;

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
