//! Native-shell integration: open a path in Finder/Explorer, reveal a file
//! selected, or open an external URL in the system browser. Implementations
//! are split per-OS via `cfg`.

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
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&url)
            .status()
            .map_err(|e| AppError::Other(format!("open url: {e}")))?;
    }
    #[cfg(target_os = "windows")]
    {
        // `cmd /c start "" <url>` — the empty pair of quotes is the
        // window-title argument that `start` insists on when the URL
        // looks like a quoted string itself.
        Command::new("cmd")
            .args(["/c", "start", "", &url])
            .status()
            .map_err(|e| AppError::Other(format!("start url: {e}")))?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&url)
            .status()
            .map_err(|e| AppError::Other(format!("xdg-open url: {e}")))?;
    }
    Ok(())
}

/// Open a file or directory in the system file browser
/// (Finder on macOS, Explorer on Windows, xdg-open on Linux).
#[tauri::command]
pub fn open_path(path: String) -> AppResult<()> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(AppError::Other(format!("path not found: {path}")));
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .status()
            .map_err(|e| AppError::Other(format!("open: {e}")))?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer.exe")
            .arg(&path)
            .status()
            .map_err(|e| AppError::Other(format!("explorer: {e}")))?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&path)
            .status()
            .map_err(|e| AppError::Other(format!("xdg-open: {e}")))?;
    }
    Ok(())
}

/// Reveal a file in the system file browser with it selected.
/// macOS: `open -R <path>`. Windows: `explorer /select,<path>`.
/// Linux falls back to opening the parent directory.
#[tauri::command]
pub fn reveal_in_finder(path: String) -> AppResult<()> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(AppError::Other(format!("path not found: {path}")));
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(&path)
            .status()
            .map_err(|e| AppError::Other(format!("open -R: {e}")))?;
    }
    #[cfg(target_os = "windows")]
    {
        // `/select,<path>` (no space after the comma) tells Explorer to
        // open the parent dir and highlight the file.
        Command::new("explorer.exe")
            .arg(format!("/select,{}", path))
            .status()
            .map_err(|e| AppError::Other(format!("explorer /select: {e}")))?;
    }
    #[cfg(target_os = "linux")]
    {
        // No universal "reveal" verb on Linux; open the parent folder.
        let parent = p.parent().unwrap_or(p);
        Command::new("xdg-open")
            .arg(parent)
            .status()
            .map_err(|e| AppError::Other(format!("xdg-open parent: {e}")))?;
    }
    Ok(())
}
