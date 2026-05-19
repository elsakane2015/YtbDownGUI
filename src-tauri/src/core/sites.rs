//! Static configuration for sites we know how to log in to.
//!
//! Adding a site = adding one entry here. Other modules treat this as a
//! lookup table; nothing else has site-specific branching.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Site {
    pub id: &'static str,
    pub display_name: &'static str,
    /// Page to load in the login webview when the user clicks "登录 X".
    pub login_url: &'static str,
    /// Domain/url used when querying cookies post-login (controls cookie scope).
    pub cookies_for_url: &'static str,
    /// A cookie that, once present, indicates a successful login.
    pub logged_in_marker_cookie: &'static str,
    /// Substring that identifies a download URL as belonging to this site.
    pub url_substring: &'static str,
}

pub const SITES: &[Site] = &[
    Site {
        id: "youtube",
        display_name: "YouTube",
        login_url: "https://accounts.google.com/ServiceLogin?service=youtube&continue=https%3A%2F%2Fwww.youtube.com%2F",
        cookies_for_url: "https://www.youtube.com",
        logged_in_marker_cookie: "SAPISID",
        url_substring: "youtube.com",
    },
    Site {
        id: "bilibili",
        display_name: "Bilibili",
        login_url: "https://passport.bilibili.com/login",
        cookies_for_url: "https://www.bilibili.com",
        logged_in_marker_cookie: "SESSDATA",
        url_substring: "bilibili.com",
    },
];

pub fn find(id: &str) -> Option<&'static Site> {
    SITES.iter().find(|s| s.id == id)
}

pub fn match_url(url: &str) -> Option<&'static Site> {
    SITES.iter().find(|s| url.contains(s.url_substring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_youtube() {
        assert_eq!(find("youtube").unwrap().display_name, "YouTube");
        assert!(find("nope").is_none());
    }

    #[test]
    fn matches_url() {
        assert_eq!(match_url("https://www.youtube.com/watch?v=abc").unwrap().id, "youtube");
        assert_eq!(match_url("https://www.bilibili.com/video/BV1xx").unwrap().id, "bilibili");
        assert!(match_url("https://example.com/x").is_none());
    }
}
