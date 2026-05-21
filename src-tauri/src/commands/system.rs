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
            .spawn()
            .map_err(|e| AppError::Other(format!("open url: {e}")))?;
    }
    #[cfg(target_os = "windows")]
    {
        // `cmd /c start "" <url>` — the empty pair of quotes is the
        // window-title argument that `start` insists on when the URL
        // looks like a quoted string itself.
        Command::new("cmd")
            .args(["/c", "start", "", &url])
            .spawn()
            .map_err(|e| AppError::Other(format!("start url: {e}")))?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| AppError::Other(format!("xdg-open url: {e}")))?;
    }
    Ok(())
}

/// Open a file or directory in the system file browser
/// (Finder on macOS, Explorer on Windows, xdg-open on Linux).
///
/// All branches use `spawn()` rather than `status()` so we don't block the
/// IPC thread waiting for the GUI shell to exit — Explorer in particular
/// can return with exit code 1 long after a successful open, or stay
/// resident, which made the button feel broken.
#[tauri::command]
pub fn open_path(path: String) -> AppResult<()> {
    let path = normalize_native_path(&path);
    if !Path::new(&path).exists() {
        return Err(AppError::Other(format!("path not found: {path}")));
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| AppError::Other(format!("open: {e}")))?;
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // `raw_arg` bypasses Rust's automatic quoting which mangles paths
        // with spaces when fed to Explorer. We explicitly quote ourselves.
        Command::new("explorer.exe")
            .raw_arg(format!("\"{}\"", path))
            .spawn()
            .map_err(|e| AppError::Other(format!("explorer: {e}")))?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| AppError::Other(format!("xdg-open: {e}")))?;
    }
    Ok(())
}

/// Reveal a file in the system file browser with it selected.
/// macOS: `open -R <path>`. Windows: `explorer /select,<path>`.
/// Linux falls back to opening the parent directory.
#[tauri::command]
pub fn reveal_in_finder(path: String) -> AppResult<()> {
    let path = normalize_native_path(&path);
    let p = Path::new(&path);
    if !p.exists() {
        return Err(AppError::Other(format!("path not found: {path}")));
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| AppError::Other(format!("open -R: {e}")))?;
    }
    #[cfg(target_os = "windows")]
    {
        // We tried `explorer.exe /select,"<path>"` with both auto-quoted
        // arg() and manual raw_arg() formatting — neither reliably
        // highlights the file (Explorer often opens an empty new window
        // or no-ops silently depending on path / username / Windows
        // build). The pragmatic fix: just open the parent directory,
        // same as the "failed-job → 打开文件夹" path which the user
        // confirmed works. We lose the file-highlight nicety but the
        // user can locate the file by name in a Folder window they can
        // actually see.
        use std::os::windows::process::CommandExt;
        let parent = p.parent().unwrap_or(p);
        let parent_str = parent.to_string_lossy();
        Command::new("explorer.exe")
            .raw_arg(format!("\"{}\"", parent_str))
            .spawn()
            .map_err(|e| AppError::Other(format!("explorer parent: {e}")))?;
    }
    #[cfg(target_os = "linux")]
    {
        // No universal "reveal" verb on Linux; open the parent folder.
        let parent = p.parent().unwrap_or(p);
        Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| AppError::Other(format!("xdg-open parent: {e}")))?;
    }
    Ok(())
}

/// Normalise a path string to the host platform's preferred separator.
/// yt-dlp on Windows occasionally emits paths with forward slashes (Python
/// std-lib uses `/` internally on every OS), which Explorer dislikes when
/// passed to `/select,…`. macOS doesn't care either way; this is a no-op
/// there. We do this in user-input parsing too so that pasting an
/// already-tidy Windows path doesn't get double-mangled.
fn normalize_native_path(path: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        path.replace('/', "\\")
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_string()
    }
}
