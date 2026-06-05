//! Dynamic account registry and cookie matching.
//!
//! Static site definitions in `core::sites` still provide stronger defaults
//! for known sites, but account records are persisted dynamically so arbitrary
//! yt-dlp-supported websites can be logged in via the embedded WebView.

use crate::core::{cookies, sites};
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

const REGISTRY_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccountState {
    LoggedIn,
    LoggedOut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    pub account_id: String,
    pub display_name: String,
    pub login_url: String,
    pub primary_host: String,
    #[serde(default)]
    pub cookie_domains: Vec<String>,
    pub status: AccountState,
    #[serde(default)]
    pub cookie_count: usize,
    pub updated_at: i64,
    #[serde(default)]
    pub known_site_id: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountRegistry {
    version: u32,
    accounts: Vec<AccountRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginTarget {
    pub account_id: String,
    pub display_name: String,
    pub login_url: String,
    pub primary_host: String,
    pub known_site_id: Option<String>,
    pub marker_cookie: Option<String>,
    pub manual_finish_required: bool,
}

#[derive(Debug, Clone)]
pub struct PreparedCookies {
    pub path: PathBuf,
    pub user_agent: Option<String>,
}

pub fn registry_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("accounts.json")
}

pub fn normalize_login_url(input: &str) -> AppResult<Url> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::Other("请输入登录网址".into()));
    }
    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let url = Url::parse(&candidate).map_err(|e| AppError::Other(format!("登录网址无效: {e}")))?;
    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(AppError::Other(format!("不支持的登录网址协议: {other}")));
        }
    }
    if url.host_str().is_none() {
        return Err(AppError::Other("登录网址缺少域名".into()));
    }
    Ok(url)
}

pub fn list(app_data_dir: &Path) -> AppResult<Vec<AccountRecord>> {
    let registry = load_registry(app_data_dir)?;
    Ok(registry.accounts)
}

pub fn ensure_login_target_for_url(app_data_dir: &Path, input: &str) -> AppResult<LoginTarget> {
    let url = normalize_login_url(input)?;
    if let Some(site) = sites::match_url(url.as_str()) {
        let record = ensure_known_account(app_data_dir, site)?;
        return Ok(login_target_for_record(
            &record,
            Some(site.logged_in_marker_cookie.to_string()),
            false,
        ));
    }

    let primary_host = canonical_host(
        url.host_str()
            .ok_or_else(|| AppError::Other("登录网址缺少域名".into()))?,
    );
    let account_id = account_id_for_host(&primary_host);
    let mut registry = load_registry(app_data_dir)?;
    let now = now_ts();
    let record = match registry
        .accounts
        .iter_mut()
        .find(|a| a.account_id == account_id)
    {
        Some(existing) => {
            existing.login_url = url.as_str().to_string();
            existing.primary_host = primary_host.clone();
            existing.display_name = display_name_for_host(&primary_host);
            existing.updated_at = now;
            existing.clone()
        }
        None => {
            let record = AccountRecord {
                account_id: account_id.clone(),
                display_name: display_name_for_host(&primary_host),
                login_url: url.as_str().to_string(),
                primary_host: primary_host.clone(),
                cookie_domains: vec![primary_host.clone()],
                status: AccountState::LoggedOut,
                cookie_count: 0,
                updated_at: now,
                known_site_id: None,
                user_agent: None,
            };
            registry.accounts.push(record.clone());
            record
        }
    };
    save_registry(app_data_dir, &registry)?;
    Ok(login_target_for_record(&record, None, true))
}

pub fn ensure_login_target_for_account(
    app_data_dir: &Path,
    account_id: &str,
) -> AppResult<LoginTarget> {
    if let Some(site) = sites::find(account_id) {
        let record = ensure_known_account(app_data_dir, site)?;
        return Ok(login_target_for_record(
            &record,
            Some(site.logged_in_marker_cookie.to_string()),
            false,
        ));
    }

    let registry = load_registry(app_data_dir)?;
    let record = registry
        .accounts
        .iter()
        .find(|a| a.account_id == account_id)
        .cloned()
        .ok_or_else(|| AppError::UnknownSite(account_id.into()))?;
    let marker = record
        .known_site_id
        .as_deref()
        .and_then(sites::find)
        .map(|s| s.logged_in_marker_cookie.to_string());
    Ok(login_target_for_record(
        &record,
        marker,
        record.known_site_id.is_none(),
    ))
}

pub fn save_login_cookies(
    app_data_dir: &Path,
    account_id: &str,
    raw_cookies: Vec<cookies::StoredCookie>,
    user_agent: Option<String>,
) -> AppResult<AccountRecord> {
    let mut registry = load_registry(app_data_dir)?;
    let idx = registry
        .accounts
        .iter()
        .position(|a| a.account_id == account_id)
        .or_else(|| sites::find(account_id).map(|_| ensure_known_index(&mut registry, account_id)))
        .ok_or_else(|| AppError::UnknownSite(account_id.into()))?;

    let record_snapshot = registry.accounts[idx].clone();
    let normalized = normalize_and_filter_cookies(&record_snapshot, raw_cookies);
    cookies::save(app_data_dir, account_id, &normalized)?;

    let domains = cookie_domains(&normalized, &record_snapshot);
    let record = &mut registry.accounts[idx];
    record.cookie_domains = domains;
    record.cookie_count = normalized.len();
    if user_agent.is_some() {
        record.user_agent = user_agent;
    }
    record.status = if normalized.is_empty() {
        AccountState::LoggedOut
    } else {
        AccountState::LoggedIn
    };
    record.updated_at = now_ts();
    let out = record.clone();
    save_registry(app_data_dir, &registry)?;
    Ok(out)
}

pub fn logout(app_data_dir: &Path, account_id: &str) -> AppResult<AccountRecord> {
    let mut registry = load_registry(app_data_dir)?;
    let idx = registry
        .accounts
        .iter()
        .position(|a| a.account_id == account_id)
        .or_else(|| sites::find(account_id).map(|_| ensure_known_index(&mut registry, account_id)))
        .ok_or_else(|| AppError::UnknownSite(account_id.into()))?;

    cookies::delete(app_data_dir, account_id)?;
    let record = &mut registry.accounts[idx];
    record.status = AccountState::LoggedOut;
    record.cookie_count = 0;
    record.updated_at = now_ts();
    let out = record.clone();
    save_registry(app_data_dir, &registry)?;
    Ok(out)
}

pub fn export_cookies(app_data_dir: &Path, account_id: &str) -> AppResult<PathBuf> {
    let stored = cookies::load(app_data_dir, account_id)?;
    let temp_dir = app_data_dir.join("tmp");
    std::fs::create_dir_all(&temp_dir)?;
    let out = temp_dir.join(format!("{}.cookies.txt", safe_file_stem(account_id)));
    cookies::write_netscape(&stored, &out)?;
    Ok(out)
}

pub fn prepare_cookies_for_url(
    app_data_dir: &Path,
    url: &str,
) -> AppResult<Option<PreparedCookies>> {
    let parsed = normalize_login_url(url)?;
    let host = canonical_host(
        parsed
            .host_str()
            .ok_or_else(|| AppError::Other("URL 缺少域名".into()))?,
    );
    let registry = load_registry(app_data_dir)?;
    let known_site = sites::match_url(parsed.as_str());

    let mut best: Option<(i32, AccountRecord)> = None;
    for account in registry.accounts {
        if account.status != AccountState::LoggedIn || account.cookie_count == 0 {
            continue;
        }
        let score = match_score(&host, &account, known_site.map(|s| s.id));
        if score <= 0 {
            continue;
        }
        if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
            best = Some((score, account));
        }
    }

    let Some((_, account)) = best else {
        return Ok(None);
    };
    let stored = cookies::load(app_data_dir, &account.account_id)?;
    let temp_dir = app_data_dir.join("tmp");
    std::fs::create_dir_all(&temp_dir)?;
    let out = temp_dir.join(format!(
        "{}.cookies.txt",
        safe_file_stem(&account.account_id)
    ));
    cookies::write_netscape(&stored, &out)?;
    Ok(Some(PreparedCookies {
        path: out,
        user_agent: account.user_agent,
    }))
}

pub fn domain_matches(host: &str, domain: &str) -> bool {
    let host = canonical_host(host);
    let domain = canonical_host(domain.trim_start_matches('.'));
    !host.is_empty()
        && !domain.is_empty()
        && (host == domain || host.ends_with(&format!(".{domain}")))
}

fn load_registry(app_data_dir: &Path) -> AppResult<AccountRegistry> {
    let path = registry_path(app_data_dir);
    let mut registry = if path.exists() {
        let bytes = std::fs::read(&path)?;
        serde_json::from_slice(&bytes)?
    } else {
        AccountRegistry {
            version: REGISTRY_VERSION,
            accounts: Vec::new(),
        }
    };
    registry.version = REGISTRY_VERSION;
    if migrate_known_cookie_files(app_data_dir, &mut registry) {
        save_registry(app_data_dir, &registry)?;
    }
    Ok(registry)
}

fn save_registry(app_data_dir: &Path, registry: &AccountRegistry) -> AppResult<()> {
    let path = registry_path(app_data_dir);
    let json = serde_json::to_vec_pretty(registry)?;
    std::fs::write(&path, json)?;
    set_owner_only(&path)?;
    Ok(())
}

fn ensure_known_account(
    app_data_dir: &Path,
    site: &'static sites::Site,
) -> AppResult<AccountRecord> {
    let mut registry = load_registry(app_data_dir)?;
    let idx = ensure_known_index(&mut registry, site.id);
    let record = registry.accounts[idx].clone();
    save_registry(app_data_dir, &registry)?;
    Ok(record)
}

fn ensure_known_index(registry: &mut AccountRegistry, site_id: &str) -> usize {
    if let Some(idx) = registry
        .accounts
        .iter()
        .position(|a| a.account_id == site_id)
    {
        return idx;
    }
    let site = sites::find(site_id).expect("known site checked before ensure_known_index");
    registry.accounts.push(AccountRecord {
        account_id: site.id.to_string(),
        display_name: site.display_name.to_string(),
        login_url: site.login_url.to_string(),
        primary_host: canonical_host(site.url_hosts[0]),
        cookie_domains: site.url_hosts.iter().map(|h| canonical_host(h)).collect(),
        status: AccountState::LoggedOut,
        cookie_count: 0,
        updated_at: now_ts(),
        known_site_id: Some(site.id.to_string()),
        user_agent: None,
    });
    registry.accounts.len() - 1
}

fn migrate_known_cookie_files(app_data_dir: &Path, registry: &mut AccountRegistry) -> bool {
    let mut changed = false;
    for site in sites::SITES {
        if registry.accounts.iter().any(|a| a.account_id == site.id) {
            continue;
        }
        let Ok(stored) = cookies::load(app_data_dir, site.id) else {
            continue;
        };
        let mut record = AccountRecord {
            account_id: site.id.to_string(),
            display_name: site.display_name.to_string(),
            login_url: site.login_url.to_string(),
            primary_host: canonical_host(site.url_hosts[0]),
            cookie_domains: Vec::new(),
            status: if stored.is_empty() {
                AccountState::LoggedOut
            } else {
                AccountState::LoggedIn
            },
            cookie_count: stored.len(),
            updated_at: now_ts(),
            known_site_id: Some(site.id.to_string()),
            user_agent: None,
        };
        record.cookie_domains = cookie_domains(&stored, &record);
        registry.accounts.push(record);
        changed = true;
    }
    changed
}

fn login_target_for_record(
    record: &AccountRecord,
    marker_cookie: Option<String>,
    manual_finish_required: bool,
) -> LoginTarget {
    LoginTarget {
        account_id: record.account_id.clone(),
        display_name: record.display_name.clone(),
        login_url: record.login_url.clone(),
        primary_host: record.primary_host.clone(),
        known_site_id: record.known_site_id.clone(),
        marker_cookie,
        manual_finish_required,
    }
}

fn normalize_and_filter_cookies(
    record: &AccountRecord,
    raw_cookies: Vec<cookies::StoredCookie>,
) -> Vec<cookies::StoredCookie> {
    let fallback_host = canonical_host(&record.primary_host);
    let mut out = Vec::new();
    for mut c in raw_cookies {
        if c.domain.trim().is_empty() {
            c.domain = fallback_host.clone();
        } else {
            c.domain = normalize_cookie_domain(&c.domain);
        }
        if c.path.trim().is_empty() {
            c.path = "/".into();
        }

        if record.known_site_id.is_some() || cookie_related_to_record(record, &c) {
            out.push(c);
        }
    }
    out
}

fn cookie_related_to_record(record: &AccountRecord, cookie: &cookies::StoredCookie) -> bool {
    let domain = canonical_host(cookie.domain.trim_start_matches('.'));
    if domain.is_empty() {
        return false;
    }
    if domain_matches(&record.primary_host, &domain)
        || domain_matches(&domain, &record.primary_host)
    {
        return true;
    }
    record
        .cookie_domains
        .iter()
        .any(|d| domain_matches(d, &domain) || domain_matches(&domain, d))
}

fn cookie_domains(cookies: &[cookies::StoredCookie], record: &AccountRecord) -> Vec<String> {
    let mut set = BTreeSet::new();
    for c in cookies {
        let domain = canonical_host(c.domain.trim_start_matches('.'));
        if !domain.is_empty() {
            set.insert(domain);
        }
    }
    if set.is_empty() {
        set.insert(canonical_host(&record.primary_host));
    }
    set.into_iter().collect()
}

fn match_score(host: &str, account: &AccountRecord, known_site_id: Option<&str>) -> i32 {
    let mut score = 0;
    if account
        .known_site_id
        .as_deref()
        .is_some_and(|id| Some(id) == known_site_id)
    {
        score = 10_000;
    }
    for domain in account
        .cookie_domains
        .iter()
        .chain(std::iter::once(&account.primary_host))
    {
        if domain_matches(host, domain) {
            score = score.max(canonical_host(domain).len() as i32);
        }
    }
    score
}

fn account_id_for_host(host: &str) -> String {
    format!("web_{}_{}", safe_file_stem(host), short_hash(host))
}

fn display_name_for_host(host: &str) -> String {
    host.trim_start_matches("www.").to_string()
}

fn canonical_host(host: &str) -> String {
    host.trim()
        .trim_end_matches('.')
        .trim_start_matches('.')
        .to_ascii_lowercase()
}

fn safe_file_stem(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "account".into()
    } else {
        trimmed.into()
    }
}

fn normalize_cookie_domain(raw: &str) -> String {
    let trimmed = raw.trim();
    let domain = canonical_host(trimmed);
    if domain.is_empty() {
        return domain;
    }
    if trimmed.starts_with('.') {
        format!(".{domain}")
    } else {
        domain
    }
}

fn short_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:08x}", hasher.finish() as u32)
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(path)?.permissions();
    perm.set_mode(0o600);
    std::fs::set_permissions(path, perm)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> AppResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn normalizes_missing_scheme() {
        let url = normalize_login_url("example.com/login").unwrap();
        assert_eq!(url.as_str(), "https://example.com/login");
    }

    #[test]
    fn rejects_unsupported_scheme() {
        assert!(normalize_login_url("file:///tmp/a").is_err());
    }

    #[test]
    fn domain_matching_is_boundary_aware() {
        assert!(domain_matches("www.example.com", "example.com"));
        assert!(domain_matches("example.com", ".example.com"));
        assert!(!domain_matches("badexample.com", "example.com"));
        assert!(!domain_matches("example.com.evil.test", "example.com"));
    }

    #[test]
    fn unknown_login_filters_unrelated_cookies_and_fills_domain() {
        let dir = tempdir().unwrap();
        let target = ensure_login_target_for_url(dir.path(), "https://www.example.com").unwrap();
        let saved = save_login_cookies(
            dir.path(),
            &target.account_id,
            vec![
                cookies::StoredCookie {
                    name: "session".into(),
                    value: "abc".into(),
                    domain: "".into(),
                    path: "".into(),
                    secure: true,
                    http_only: true,
                    expires: None,
                },
                cookies::StoredCookie {
                    name: "other".into(),
                    value: "nope".into(),
                    domain: "other.test".into(),
                    path: "/".into(),
                    secure: true,
                    http_only: false,
                    expires: None,
                },
            ],
            None,
        )
        .unwrap();
        assert_eq!(saved.cookie_count, 1);
        let stored = cookies::load(dir.path(), &target.account_id).unwrap();
        assert_eq!(stored[0].domain, "www.example.com");
        assert_eq!(stored[0].path, "/");
    }

    #[test]
    fn preserves_domain_cookie_scope() {
        let dir = tempdir().unwrap();
        let target = ensure_login_target_for_url(dir.path(), "https://example.com").unwrap();
        save_login_cookies(
            dir.path(),
            &target.account_id,
            vec![cookies::StoredCookie {
                name: "wide".into(),
                value: "1".into(),
                domain: ".example.com".into(),
                path: "/".into(),
                secure: true,
                http_only: false,
                expires: None,
            }],
            None,
        )
        .unwrap();
        let stored = cookies::load(dir.path(), &target.account_id).unwrap();
        assert_eq!(stored[0].domain, ".example.com");
        let exported =
            std::fs::read_to_string(export_cookies(dir.path(), &target.account_id).unwrap())
                .unwrap();
        assert!(exported.contains(".example.com\tTRUE\t/\tTRUE\t0\twide\t1"));
    }

    #[test]
    fn migrates_existing_known_site_cookie_file() {
        let dir = tempdir().unwrap();
        cookies::save(
            dir.path(),
            "youtube",
            &[cookies::StoredCookie {
                name: "SAPISID".into(),
                value: "abc".into(),
                domain: ".youtube.com".into(),
                path: "/".into(),
                secure: true,
                http_only: true,
                expires: None,
            }],
        )
        .unwrap();
        cookies::save(
            dir.path(),
            "bilibili",
            &[cookies::StoredCookie {
                name: "SESSDATA".into(),
                value: "abc".into(),
                domain: ".bilibili.com".into(),
                path: "/".into(),
                secure: true,
                http_only: true,
                expires: None,
            }],
        )
        .unwrap();

        let accounts = list(dir.path()).unwrap();
        let youtube = accounts
            .iter()
            .find(|a| a.account_id == "youtube")
            .expect("youtube account migrated");
        assert_eq!(youtube.status, AccountState::LoggedIn);
        assert_eq!(youtube.cookie_count, 1);
        assert_eq!(youtube.known_site_id.as_deref(), Some("youtube"));
        let bilibili = accounts
            .iter()
            .find(|a| a.account_id == "bilibili")
            .expect("bilibili account migrated");
        assert_eq!(bilibili.status, AccountState::LoggedIn);
        assert_eq!(bilibili.cookie_count, 1);
        assert_eq!(bilibili.known_site_id.as_deref(), Some("bilibili"));
    }

    #[test]
    fn logout_preserves_account_for_relogin() {
        let dir = tempdir().unwrap();
        let target = ensure_login_target_for_url(dir.path(), "https://example.com/login").unwrap();
        save_login_cookies(
            dir.path(),
            &target.account_id,
            vec![cookies::StoredCookie {
                name: "session".into(),
                value: "abc".into(),
                domain: "example.com".into(),
                path: "/".into(),
                secure: true,
                http_only: true,
                expires: None,
            }],
            None,
        )
        .unwrap();

        let logged_out = logout(dir.path(), &target.account_id).unwrap();
        assert_eq!(logged_out.status, AccountState::LoggedOut);
        assert_eq!(logged_out.cookie_count, 0);
        assert!(cookies::load(dir.path(), &target.account_id).is_err());

        let accounts = list(dir.path()).unwrap();
        let saved = accounts
            .iter()
            .find(|a| a.account_id == target.account_id)
            .expect("logged-out account remains in registry");
        assert_eq!(saved.status, AccountState::LoggedOut);
        assert_eq!(saved.primary_host, "example.com");

        let relogin = ensure_login_target_for_account(dir.path(), &target.account_id).unwrap();
        assert_eq!(relogin.account_id, target.account_id);
        assert_eq!(relogin.login_url, "https://example.com/login");
        assert!(relogin.manual_finish_required);
    }

    #[test]
    fn prepare_uses_most_specific_matching_account() {
        let dir = tempdir().unwrap();
        let root = ensure_login_target_for_url(dir.path(), "https://example.com").unwrap();
        save_login_cookies(
            dir.path(),
            &root.account_id,
            vec![cookies::StoredCookie {
                name: "root".into(),
                value: "1".into(),
                domain: "example.com".into(),
                path: "/".into(),
                secure: true,
                http_only: false,
                expires: None,
            }],
            None,
        )
        .unwrap();
        let sub = ensure_login_target_for_url(dir.path(), "https://video.example.com").unwrap();
        save_login_cookies(
            dir.path(),
            &sub.account_id,
            vec![cookies::StoredCookie {
                name: "sub".into(),
                value: "2".into(),
                domain: "video.example.com".into(),
                path: "/".into(),
                secure: true,
                http_only: false,
                expires: None,
            }],
            None,
        )
        .unwrap();
        let prepared = prepare_cookies_for_url(dir.path(), "https://video.example.com/watch")
            .unwrap()
            .unwrap();
        let exported = std::fs::read_to_string(prepared.path).unwrap();
        assert!(exported.contains("\tsub\t2"));
    }

    #[test]
    fn prepare_returns_saved_user_agent() {
        let dir = tempdir().unwrap();
        let target = ensure_login_target_for_url(dir.path(), "https://example.com").unwrap();
        save_login_cookies(
            dir.path(),
            &target.account_id,
            vec![cookies::StoredCookie {
                name: "session".into(),
                value: "abc".into(),
                domain: "example.com".into(),
                path: "/".into(),
                secure: true,
                http_only: false,
                expires: None,
            }],
            Some("TestBrowser/1.0".into()),
        )
        .unwrap();

        let prepared = prepare_cookies_for_url(dir.path(), "https://www.example.com/video")
            .unwrap()
            .unwrap();
        assert_eq!(prepared.user_agent.as_deref(), Some("TestBrowser/1.0"));
    }
}
