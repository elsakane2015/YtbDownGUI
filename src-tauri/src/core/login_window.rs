//! Login webview window: opens a dedicated WKWebView pointing at a site's
//! login URL, watches navigation, and exposes cookie extraction so the rest
//! of the app can grab session cookies once the user is signed in.
//!
//! Intentionally keeps zero site-specific branching — known-site details and
//! dynamic URLs are normalized into `LoginTarget` before this module sees
//! them. The window's title is kept in sync with the loaded URL so the user
//! can always see what domain they are on.
//!
//! Once a window is open, a background poller checks every 2s for the
//! site's marker cookie. As soon as it appears (the user finished signing
//! in), cookies are persisted and the window is closed — so the user does
//! not have to remember to come back and click "Finish".

use crate::core::{
    accounts::{self, LoginTarget},
    cookies::StoredCookie,
};
use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tauri::{
    webview::PageLoadEvent, AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub const LOGIN_WINDOW_LABEL: &str = "login";
static LOGIN_FINISHED: AtomicBool = AtomicBool::new(false);
static LOGIN_USER_AGENT: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize)]
pub struct LoginEventPayload {
    pub account_id: String,
    pub display_name: String,
    pub cookie_count: usize,
}

pub fn open_target(app: &AppHandle, target: LoginTarget) -> AppResult<WebviewWindow> {
    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
        let _ = existing.set_focus();
        return Err(AppError::LoginInProgress(target.account_id));
    }
    LOGIN_FINISHED.store(false, Ordering::SeqCst);

    let target_url_str = target.login_url.clone();
    let display = target.display_name.clone();
    let initial_title = format!("登录 {} · {}", target.display_name, target.login_url);

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
    #[cfg(target_os = "macos")]
    let user_agent = Some(
        // WKWebView can expose an app-flavoured default UA. A normal desktop
        // Safari UA is closer to the browser environment that strict login
        // pages expect on macOS.
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
         (KHTML, like Gecko) Version/17.4 Safari/605.1.15"
            .to_string(),
    );
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let user_agent = Some(
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
         (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36"
            .to_string(),
    );
    set_current_login_user_agent(user_agent.clone());

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
    // Hide WebView2's automation markers so sites like Bilibili don't
    // return 412 / bot-detection blocks. navigator.webdriver is true by
    // default in WebView2; removing it makes the UA indistinguishable
    // from a real Chrome session.
    .initialization_script(
        "Object.defineProperty(navigator,'webdriver',{get:()=>undefined,configurable:true});\
         if(!window.chrome){window.chrome={runtime:{}};}",
    )
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
        target.account_id, target_url_str
    ));

    spawn_watcher(app.clone(), target);
    Ok(win)
}

pub fn fetch_all_cookies(window: &WebviewWindow) -> AppResult<Vec<StoredCookie>> {
    let cookies = window
        .cookies()
        .map_err(|e| AppError::Other(format!("cookies failed: {e}")))?;
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

pub fn mark_finished() {
    LOGIN_FINISHED.store(true, Ordering::SeqCst);
}

pub fn current_login_user_agent() -> Option<String> {
    LOGIN_USER_AGENT.lock().ok().and_then(|ua| ua.clone())
}

/// Spawn a background task that polls the login window for the target marker
/// cookie. On detection: save all cookies, close the window, emit events.
fn spawn_watcher(app: AppHandle, target: LoginTarget) {
    tauri::async_runtime::spawn(async move {
        let account_id = target.account_id.clone();
        let marker_cookie = target.marker_cookie.clone();

        if let Some(marker) = &marker_cookie {
            eprintln!(
                "[login:{account_id}] watcher started, looking for marker cookie '{}'",
                marker
            );
        } else {
            eprintln!("[login:{account_id}] watcher started, waiting for manual finish");
        }

        // Cap at ~20 minutes so a forgotten login window doesn't poll forever.
        let mut tick: u32 = 0;
        for _ in 0..1200 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            tick += 1;

            let win = match app.get_webview_window(LOGIN_WINDOW_LABEL) {
                Some(w) => w,
                None => {
                    if LOGIN_FINISHED.load(Ordering::SeqCst) {
                        return;
                    }
                    eprintln!("[login:{account_id}] window closed by user, watcher exiting");
                    let _ = app.emit("login:cancelled", account_id);
                    return;
                }
            };

            let Some(marker_cookie) = marker_cookie.as_deref() else {
                continue;
            };

            // Fetch ALL cookies in this webview (not filtered by URL). The URL
            // filter can hide cookies on other related domains (e.g. Google's
            // auth cookies live on .google.com but are visible from a YouTube
            // session). Filtering ourselves on the marker name is the most
            // robust way to detect login completion.
            let cookies = match win.cookies() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[login:{account_id}] tick {tick}: cookies() error: {e}");
                    continue;
                }
            };

            if tick <= 3 || tick % 5 == 0 {
                let names: Vec<&str> = cookies.iter().map(|c| c.name()).collect();
                eprintln!(
                    "[login:{account_id}] tick {tick}: {} cookies: {:?}",
                    cookies.len(),
                    names
                );
            }

            let has_marker = cookies.iter().any(|c| c.name() == marker_cookie);
            if !has_marker {
                continue;
            }

            let stored: Vec<StoredCookie> = cookies.into_iter().map(cookie_to_stored).collect();
            let data_dir = match crate::core::paths::data_dir(&app) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("[login:{account_id}] no app_data_dir: {e}");
                    return;
                }
            };

            let record = match accounts::save_login_cookies(
                &data_dir,
                &account_id,
                stored,
                current_login_user_agent(),
            ) {
                Ok(record) => record,
                Err(e) => {
                    eprintln!("[login:{account_id}] save failed: {e}");
                    let _ = app.emit("login:failed", format!("save error: {e}"));
                    return;
                }
            };

            eprintln!(
                "[login:{account_id}] detected marker {}, saved {} cookies",
                marker_cookie, record.cookie_count
            );
            mark_finished();
            let _ = win.close();
            let _ = app.emit("account:updated", &account_id);
            let _ = app.emit(
                "login:succeeded",
                LoginEventPayload {
                    account_id,
                    display_name: record.display_name,
                    cookie_count: record.cookie_count,
                },
            );
            return;
        }

        eprintln!("[login:{account_id}] watcher timed out after 20 minutes");
        let _ = app.emit("login:timeout", account_id);
    });
}

fn set_current_login_user_agent(user_agent: Option<String>) {
    if let Ok(mut current) = LOGIN_USER_AGENT.lock() {
        *current = user_agent;
    }
}
