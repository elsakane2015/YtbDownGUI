//! Fallback login path for Windows where the embedded WebView2 refuses
//! to render external login pages: open the URL in the user's default
//! browser, let them sign in there, then ask yt-dlp's
//! `--cookies-from-browser` to copy the resulting session cookies into
//! our own store.
//!
//! Two-step IPC:
//! 1. `browser_login_start(site_id)` — open the site in the system
//!    browser. Tells the user to sign in there.
//! 2. `browser_login_import(site_id, browser)` — run yt-dlp with
//!    `--cookies-from-browser <browser> --cookies <tmp>`, read the
//!    written cookies.txt, save into our per-site store. Returns
//!    cookie count on success.

use crate::core::{cookies, download::yt_dlp_command, paths, sites};
use crate::error::{AppError, AppResult};
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::process::CommandEvent;

#[tauri::command]
pub fn browser_login_start(site_id: String) -> AppResult<()> {
    let site = sites::find(&site_id).ok_or_else(|| AppError::UnknownSite(site_id.clone()))?;
    // We send the user to `cookies_for_url` (the post-login URL) rather
    // than `login_url` because most sites' modal login pop-ups live on the
    // homepage and the embedded passport page sometimes white-screens
    // even in real browsers when accessed cold.
    crate::commands::system::open_url(site.cookies_for_url.into())
}

#[tauri::command]
pub async fn browser_login_import(
    app: AppHandle,
    site_id: String,
    browser: String,
) -> AppResult<usize> {
    let site = sites::find(&site_id).ok_or_else(|| AppError::UnknownSite(site_id.clone()))?;
    let data_dir = paths::data_dir(&app)?;
    let tmp_dir = data_dir.join("tmp");
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp_cookies = tmp_dir.join(format!("{}_browser_import.txt", site.id));
    // Clean any leftover so we don't accidentally read stale data on
    // partial failure.
    let _ = std::fs::remove_file(&tmp_cookies);

    let args: Vec<String> = vec![
        "--cookies-from-browser".into(),
        browser.clone(),
        "--cookies".into(),
        tmp_cookies.display().to_string(),
        "--simulate".into(),
        "--skip-download".into(),
        "--no-warnings".into(),
        "--no-playlist".into(),
        site.cookies_for_url.into(),
    ];

    crate::core::log::write(format!(
        "[browser-login:{}] running yt-dlp --cookies-from-browser {browser} on {}",
        site.id, site.cookies_for_url
    ));

    let cmd = yt_dlp_command(&app)?.args(args);
    let (mut rx, _child) = cmd
        .spawn()
        .map_err(|e| AppError::Other(format!("spawn yt-dlp: {e}")))?;
    let mut stderr_tail = String::new();
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stderr(b) => stderr_tail.push_str(&String::from_utf8_lossy(&b)),
            CommandEvent::Terminated(_) => break,
            _ => {}
        }
    }

    if !tmp_cookies.exists() {
        return Err(AppError::Other(format!(
            "yt-dlp didn't write {} — is {browser} installed and signed in to {}? \
             stderr tail: {}",
            tmp_cookies.display(),
            site.cookies_for_url,
            stderr_tail.lines().rev().take(3).collect::<Vec<_>>().join(" | ")
        )));
    }

    let content = std::fs::read_to_string(&tmp_cookies)?;
    let mut parsed = cookies::parse_netscape(&content)?;
    // Filter to cookies whose domain plausibly belongs to this site —
    // yt-dlp may pull adjacent first-party cookies too. We keep anything
    // whose domain contains any of the site's known hosts.
    parsed.retain(|c| {
        site.url_hosts
            .iter()
            .any(|h| c.domain.to_lowercase().contains(h))
    });

    if parsed.is_empty() {
        return Err(AppError::Other(format!(
            "no cookies for {} found in {browser}. Did you sign in to {} in {browser} first?",
            site.id, site.cookies_for_url
        )));
    }

    cookies::save(&data_dir, site.id, &parsed)?;
    let _ = std::fs::remove_file(&tmp_cookies);
    let _ = app.emit("account:updated", site.id);
    let _ = app.emit("login:succeeded", site.id);
    crate::core::log::write(format!(
        "[browser-login:{}] imported {} cookies from {browser}",
        site.id,
        parsed.len()
    ));
    Ok(parsed.len())
}
