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
    /// Bare hostnames that identify download URLs as belonging to this site.
    /// We match by exact host or `*.host` (subdomains), so `www.youtube.com`
    /// and `m.youtube.com` both match the `youtube.com` entry, but `nox.com`
    /// does NOT match `x.com`.
    pub url_hosts: &'static [&'static str],
    /// Whether `--flat-playlist` returns useful metadata for playlist /
    /// channel entries on this site. True for YouTube (fast probe with full
    /// titles), false for Bilibili (multi-part `?p=N` series — flat mode
    /// returns only entry URLs, so we must do a full probe).
    pub use_flat_playlist: bool,
}

pub const SITES: &[Site] = &[
    Site {
        id: "youtube",
        display_name: "YouTube",
        login_url: "https://accounts.google.com/ServiceLogin?service=youtube&continue=https%3A%2F%2Fwww.youtube.com%2F",
        cookies_for_url: "https://www.youtube.com",
        logged_in_marker_cookie: "SAPISID",
        url_hosts: &["youtube.com", "youtu.be"],
        use_flat_playlist: true,
    },
    Site {
        id: "bilibili",
        display_name: "Bilibili",
        // The standalone passport.bilibili.com/login page hangs WebView2
        // on Windows (white screen, no event response). The homepage's
        // modal login flow is more compatible — same SESSDATA cookie
        // ends up on .bilibili.com when the user signs in there.
        login_url: "https://www.bilibili.com/",
        cookies_for_url: "https://www.bilibili.com",
        logged_in_marker_cookie: "SESSDATA",
        url_hosts: &["bilibili.com", "b23.tv"],
        // Bilibili's --flat-playlist returns only URLs for multi-part videos
        // (no title/duration/thumbnail). We trade probe speed for usable data.
        use_flat_playlist: false,
    },
    Site {
        id: "twitter",
        display_name: "X (Twitter)",
        login_url: "https://x.com/i/flow/login",
        cookies_for_url: "https://x.com",
        logged_in_marker_cookie: "auth_token",
        url_hosts: &["x.com", "twitter.com"],
        use_flat_playlist: true,
    },
    Site {
        id: "tencent_video",
        display_name: "腾讯视频",
        login_url: "https://v.qq.com/",
        cookies_for_url: "https://v.qq.com",
        logged_in_marker_cookie: "vqq_vuserid",
        url_hosts: &["v.qq.com"],
        use_flat_playlist: true,
    },
    Site {
        id: "douyin",
        display_name: "抖音",
        login_url: "https://www.douyin.com/",
        cookies_for_url: "https://www.douyin.com",
        logged_in_marker_cookie: "sessionid_ss",
        url_hosts: &["douyin.com"],
        use_flat_playlist: true,
    },
    Site {
        id: "tiktok",
        display_name: "TikTok",
        login_url: "https://www.tiktok.com/login",
        cookies_for_url: "https://www.tiktok.com",
        logged_in_marker_cookie: "sessionid",
        url_hosts: &["tiktok.com"],
        use_flat_playlist: true,
    },
    Site {
        id: "pinterest",
        display_name: "Pinterest",
        login_url: "https://www.pinterest.com/login/",
        cookies_for_url: "https://www.pinterest.com",
        logged_in_marker_cookie: "_pinterest_sess",
        url_hosts: &["pinterest.com", "pin.it"],
        use_flat_playlist: true,
    },
];

pub fn find(id: &str) -> Option<&'static Site> {
    SITES.iter().find(|s| s.id == id)
}

/// Resolve a download URL to its owning site by parsing the URL's host and
/// matching exact-or-subdomain against each site's `url_hosts`.
pub fn match_url(url_str: &str) -> Option<&'static Site> {
    let parsed = url::Url::parse(url_str).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    SITES.iter().find(|s| {
        s.url_hosts
            .iter()
            .any(|h| host == *h || host.ends_with(&format!(".{h}")))
    })
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
    fn matches_youtube_variants() {
        assert_eq!(
            match_url("https://www.youtube.com/watch?v=abc").unwrap().id,
            "youtube"
        );
        assert_eq!(
            match_url("https://m.youtube.com/watch?v=abc").unwrap().id,
            "youtube"
        );
        assert_eq!(match_url("https://youtu.be/abc").unwrap().id, "youtube");
    }

    #[test]
    fn matches_x_and_twitter() {
        assert_eq!(
            match_url("https://x.com/user/status/12345").unwrap().id,
            "twitter"
        );
        assert_eq!(
            match_url("https://twitter.com/user/status/12345").unwrap().id,
            "twitter"
        );
        assert_eq!(
            match_url("https://mobile.twitter.com/foo").unwrap().id,
            "twitter"
        );
    }

    #[test]
    fn no_false_positives() {
        // x.com is a substring of nox.com but should NOT match Twitter.
        assert!(match_url("https://nox.com/x").is_none());
        // Random URL: no match.
        assert!(match_url("https://example.com/x").is_none());
    }

    #[test]
    fn matches_bilibili() {
        assert_eq!(
            match_url("https://www.bilibili.com/video/BV1xx").unwrap().id,
            "bilibili"
        );
        assert_eq!(match_url("https://b23.tv/abc").unwrap().id, "bilibili");
    }

    #[test]
    fn matches_new_sites() {
        assert_eq!(
            match_url("https://v.qq.com/x/cover/abc/xyz.html").unwrap().id,
            "tencent_video"
        );
        assert_eq!(
            match_url("https://www.douyin.com/video/12345").unwrap().id,
            "douyin"
        );
        assert_eq!(
            match_url("https://www.tiktok.com/@user/video/123").unwrap().id,
            "tiktok"
        );
        assert_eq!(
            match_url("https://www.pinterest.com/pin/12345/").unwrap().id,
            "pinterest"
        );
        assert_eq!(match_url("https://pin.it/abc").unwrap().id, "pinterest");
        // v.qq.com should NOT capture other qq.com services
        assert!(match_url("https://mail.qq.com/").is_none());
    }
}
