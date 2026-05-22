//! Login webview window: opens a dedicated WKWebView pointing at a site's
//! login URL, watches navigation, and exposes cookie extraction so the rest
//! of the app can grab session cookies once the user is signed in.
//!
//! Intentionally keeps zero site-specific branching — every site comes from
//! `core::sites`. The window's title is kept in sync with the loaded URL so
//! the user can always see what domain they are on.
//!
//! Once a window is open, a background poller checks every 2s for the
//! site's marker cookie. As soon as it appears (the user finished signing
//! in), cookies are persisted and the window is closed — so the user does
//! not have to remember to come back and click "Finish".

use crate::core::{
    cookies::{self, StoredCookie},
    sites::{self, Site},
};
use crate::error::{AppError, AppResult};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tauri::{webview::PageLoadEvent, AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const LOGIN_WINDOW_LABEL: &str = "login";

/// Open the login window and start a background watcher that auto-saves
/// cookies once the site's marker cookie appears.
pub fn open(app: &AppHandle, site: &Site) -> AppResult<WebviewWindow> {
    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
        let _ = existing.set_focus();
        return Err(AppError::LoginInProgress(site.id.into()));
    }

    let target_url_str = site.login_url.to_string();
    let display = site.display_name.to_string();
    let initial_title = format!("登录 {} · {}", site.display_name, site.login_url);

    // ─── Windows WebView2 white-screen workaround ─────────────────────────
    // Tauri 2 / WebView2 can white-screen or hang when a webview window is
    // created directly on an external login URL from Windows. Build the
    // window on a bundled local page first, then navigate after the webview
    // exists. This also avoids relying on `data:` URL parsing/rendering in
    // WebView2, which was another source of blank windows in packaged builds.

    #[cfg(target_os = "windows")]
    let user_agent = Some(
        // WebView2's default UA includes "Edg/" which some sites treat
        // as an embedded browser. Plain Chrome UA bypasses that check.
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
         (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36"
            .to_string(),
    );
    #[cfg(not(target_os = "windows"))]
    let user_agent: Option<String> = None;

    // on_page_load fires twice per page (Started + Finished). Use an atomic
    // flag so we only navigate once, and only after the stub has fully loaded.
    let navigated = Arc::new(AtomicBool::new(false));
    let navigated_for_cb = navigated.clone();
    // Capture the URL as a plain String for the JS eval below.
    let target_url_str_for_cb = target_url_str.clone();

    let mut builder = WebviewWindowBuilder::new(
        app,
        LOGIN_WINDOW_LABEL,
        WebviewUrl::App("login-stub.html".into()),
    )
    .title(initial_title)
    .inner_size(1000.0, 720.0)
    .min_inner_size(720.0, 520.0)
    .resizable(true)
    .decorations(true)
    .closable(true)
    .visible(true)
    .focused(true)
    .center()
    .transparent(false)
    .on_page_load(move |win, payload| {
        let url = payload.url().to_string();
        if url.contains("login-stub.html") {
            // Wait for Finished (stub fully rendered) before navigating.
            // Use JS eval rather than win.navigate(): calling WebView2's
            // Navigate() synchronously inside a NavigationCompleted handler
            // triggers a COM re-entrancy guard and the call is silently
            // dropped, leaving the webview stuck on the stub.
            if payload.event() == PageLoadEvent::Finished
                && !navigated_for_cb.swap(true, Ordering::SeqCst)
            {
                let escaped = target_url_str_for_cb
                    .replace('\\', "\\\\")
                    .replace('\'', "\\'");
                let js = format!("window.location.replace('{escaped}');");
                if let Err(e) = win.eval(&js) {
                    crate::core::log::write(format!("[login] eval navigate failed: {e}"));
                }
            }
        } else {
            let _ = win.set_title(&format!("登录 {display} · {url}"));
        }
    });
    if let Some(ua) = user_agent {
        builder = builder.user_agent(&ua);
    }
    let win = builder.build()?;

    crate::core::log::write(format!(
        "[login:{}] window built on local stub, waiting to navigate to {}",
        site.id, target_url_str
    ));

    spawn_watcher(app.clone(), site.id.to_string());
    Ok(win)
}

/// Fetch cookies from the open login window scoped to `site.cookies_for_url`.
pub fn fetch_cookies(window: &WebviewWindow, site: &Site) -> AppResult<Vec<StoredCookie>> {
    let url: tauri::Url = site
        .cookies_for_url
        .parse()
        .map_err(|e| AppError::Other(format!("bad cookies URL: {e}")))?;
    let cookies = window
        .cookies_for_url(url)
        .map_err(|e| AppError::Other(format!("cookies_for_url failed: {e}")))?;
    Ok(cookies.into_iter().map(cookie_to_stored).collect())
}

fn cookie_to_stored(c: cookie::Cookie<'static>) -> StoredCookie {
    let expires = match c.expires() {
        Some(cookie::Expiration::DateTime(dt)) => Some(dt.unix_timestamp()),
        _ => None,
    };
    StoredCookie {
        name: c.name().to_string(),
        value: c.value().to_string(),
        domain: c.domain().unwrap_or("").to_string(),
        path: c.path().unwrap_or("/").to_string(),
        secure: c.secure().unwrap_or(false),
        http_only: c.http_only().unwrap_or(false),
        expires,
    }
}

pub fn close(app: &AppHandle) -> AppResult<()> {
    if let Some(w) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
        w.close()?;
    }
    Ok(())
}

/// Spawn a background task that polls the login window for the site's marker
/// cookie. On detection: save all cookies, close the window, emit
/// `account:updated`. Exits silently if the user closes the window manually.
fn spawn_watcher(app: AppHandle, site_id: String) {
    tauri::async_runtime::spawn(async move {
        let site = match sites::find(&site_id) {
            Some(s) => s,
            None => return,
        };

        eprintln!(
            "[login:{site_id}] watcher started, looking for marker cookie '{}'",
            site.logged_in_marker_cookie
        );

        // Cap at ~20 minutes so a forgotten login window doesn't poll forever.
        let mut tick: u32 = 0;
        for _ in 0..1200 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            tick += 1;

            let win = match app.get_webview_window(LOGIN_WINDOW_LABEL) {
                Some(w) => w,
                None => {
                    eprintln!("[login:{site_id}] window closed by user, watcher exiting");
                    let _ = app.emit("login:cancelled", site.id);
                    return;
                }
            };

            // Fetch ALL cookies in this webview (not filtered by URL). The URL
            // filter can hide cookies on other related domains (e.g. Google's
            // auth cookies live on .google.com but are visible from a YouTube
            // session). Filtering ourselves on the marker name is the most
            // robust way to detect login completion.
            let cookies = match win.cookies() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[login:{site_id}] tick {tick}: cookies() error: {e}");
                    continue;
                }
            };

            if tick <= 3 || tick % 5 == 0 {
                let names: Vec<&str> = cookies.iter().map(|c| c.name()).collect();
                eprintln!(
                    "[login:{site_id}] tick {tick}: {} cookies: {:?}",
                    cookies.len(),
                    names
                );
            }

            let has_marker = cookies
                .iter()
                .any(|c| c.name() == site.logged_in_marker_cookie);
            if !has_marker {
                continue;
            }

            let stored: Vec<StoredCookie> = cookies.into_iter().map(cookie_to_stored).collect();
            let data_dir = match crate::core::paths::data_dir(&app) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("[login:{site_id}] no app_data_dir: {e}");
                    return;
                }
            };

            if let Err(e) = cookies::save(&data_dir, site.id, &stored) {
                eprintln!("[login:{site_id}] save failed: {e}");
                let _ = app.emit("login:failed", format!("save error: {e}"));
                return;
            }

            eprintln!(
                "[login:{site_id}] detected marker {}, saved {} cookies",
                site.logged_in_marker_cookie,
                stored.len()
            );
            let _ = win.close();
            let _ = app.emit("account:updated", site.id);
            let _ = app.emit("login:succeeded", site.id);
            return;
        }

        eprintln!("[login:{site_id}] watcher timed out after 20 minutes");
        let _ = app.emit("login:timeout", site.id);
    });
}
