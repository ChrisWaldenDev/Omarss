//! HTTP fetching (SPEC §7.1): conditional GET, size limits, and redirects handled by hand so
//! permanent redirects can update a feed's stored URL.

pub mod discovery;
pub mod favicon;

use std::time::{Duration, SystemTime};

use reqwest::header::{self, HeaderMap};
use reqwest::StatusCode;
use url::Url;

use crate::error::{AppError, AppResult};

pub const FEED_MAX_BYTES: usize = 10 * 1024 * 1024;
pub const PAGE_MAX_BYTES: usize = 5 * 1024 * 1024;
pub const ICON_MAX_BYTES: usize = 1024 * 1024;
const MAX_REDIRECTS: usize = 5;

pub const FEED_ACCEPT: &str = "application/rss+xml, application/atom+xml, application/feed+json, \
    application/json;q=0.9, application/xml;q=0.9, text/xml;q=0.9, */*;q=0.8";
pub const PAGE_ACCEPT: &str = "text/html, application/xhtml+xml, application/rss+xml, \
    application/atom+xml, application/xml;q=0.9, */*;q=0.8";
pub const IMAGE_ACCEPT: &str = "image/png, image/x-icon, image/*;q=0.9, */*;q=0.5";

pub const PROJECT_URL: &str = "https://github.com/ChrisWaldenDev/omarss";

#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
}

pub struct Request<'a> {
    pub url: &'a str,
    pub accept: &'a str,
    pub max_bytes: usize,
    pub etag: Option<&'a str>,
    pub last_modified: Option<&'a str>,
}

impl<'a> Request<'a> {
    pub fn new(url: &'a str, accept: &'a str, max_bytes: usize) -> Self {
        Self {
            url,
            accept,
            max_bytes,
            etag: None,
            last_modified: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    /// The URL that produced this response (after all redirects).
    pub url: String,
    /// The requested URL, updated only through permanent (301/308) redirects; stops changing
    /// at the first temporary one.
    pub permanent_url: String,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    /// `Cache-Control: max-age`, in seconds.
    pub max_age: Option<i64>,
    /// `Retry-After`, in seconds from now.
    pub retry_after: Option<i64>,
}

impl Response {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn is_not_modified(&self) -> bool {
        self.status == 304
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchError {
    pub message: String,
    /// The server couldn't be reached at all (DNS, refused, unreachable). Used to tell
    /// "this machine is offline" apart from "this feed is broken".
    pub connection: bool,
}

impl FetchError {
    fn other(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            connection: false,
        }
    }
}

impl From<FetchError> for AppError {
    fn from(err: FetchError) -> Self {
        AppError::network(err.message)
    }
}

impl HttpClient {
    /// `version` goes into the User-Agent: `Omarss/<version> (+<project url>)`.
    pub fn new(version: &str) -> AppResult<Self> {
        // reqwest is built without a bundled crypto provider; use ring (no C toolchain quirks).
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::builder()
            .user_agent(format!("Omarss/{version} (+{PROJECT_URL})"))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|err| AppError::internal(format!("Could not set up HTTP: {err}")))?;
        Ok(Self { client })
    }

    pub async fn get(&self, request: Request<'_>) -> Result<Response, FetchError> {
        let start = Url::parse(request.url)
            .map_err(|_| FetchError::other(format!("Not a valid address: {}", request.url)))?;
        if !matches!(start.scheme(), "http" | "https") {
            return Err(FetchError::other(
                "Only http and https addresses are supported",
            ));
        }

        let mut current = start.clone();
        let mut permanent = start;
        let mut still_permanent = true;
        for _ in 0..=MAX_REDIRECTS {
            let mut builder = self
                .client
                .get(current.clone())
                .header(header::ACCEPT, request.accept);
            if let Some(etag) = request.etag {
                builder = builder.header(header::IF_NONE_MATCH, etag);
            }
            if let Some(last_modified) = request.last_modified {
                builder = builder.header(header::IF_MODIFIED_SINCE, last_modified);
            }
            let mut response = builder
                .send()
                .await
                .map_err(|err| describe(&err, &current))?;
            let status = response.status();

            if status.is_redirection() && status != StatusCode::NOT_MODIFIED {
                let location = response
                    .headers()
                    .get(header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|loc| current.join(loc).ok())
                    .ok_or_else(|| {
                        FetchError::other(format!("{current} redirected without a destination"))
                    })?;
                let is_permanent = matches!(
                    status,
                    StatusCode::MOVED_PERMANENTLY | StatusCode::PERMANENT_REDIRECT
                );
                still_permanent &= is_permanent;
                if still_permanent {
                    permanent = location.clone();
                }
                current = location;
                continue;
            }

            let headers = response.headers().clone();
            let mut body = Vec::new();
            if !status.is_redirection() {
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|err| describe(&err, &current))?
                {
                    if body.len() + chunk.len() > request.max_bytes {
                        return Err(FetchError::other(format!(
                            "{current} is larger than {} MB",
                            request.max_bytes / (1024 * 1024)
                        )));
                    }
                    body.extend_from_slice(&chunk);
                }
            }
            return Ok(Response {
                status: status.as_u16(),
                url: current.to_string(),
                permanent_url: permanent.to_string(),
                body,
                content_type: header_str(&headers, header::CONTENT_TYPE),
                etag: header_str(&headers, header::ETAG),
                last_modified: header_str(&headers, header::LAST_MODIFIED),
                max_age: header_str(&headers, header::CACHE_CONTROL)
                    .as_deref()
                    .and_then(parse_max_age),
                retry_after: header_str(&headers, header::RETRY_AFTER)
                    .as_deref()
                    .and_then(|v| parse_retry_after(v, SystemTime::now())),
            });
        }
        Err(FetchError::other(format!(
            "Too many redirects (more than {MAX_REDIRECTS})"
        )))
    }
}

fn header_str(headers: &HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn describe(err: &reqwest::Error, url: &Url) -> FetchError {
    let host = url.host_str().unwrap_or("the server");
    // The innermost cause is the readable part ("dns error", "connection refused", …).
    let mut cause: &dyn std::error::Error = err;
    while let Some(source) = cause.source() {
        cause = source;
    }
    if err.is_timeout() {
        FetchError::other(format!("{host} took too long to respond"))
    } else if is_certificate_error(err) {
        // A bad certificate is the site's problem, not a sign that we're offline.
        FetchError::other(format!(
            "{host}'s security certificate isn't valid ({cause})"
        ))
    } else if err.is_connect() {
        FetchError {
            message: format!("Couldn't connect to {host} ({cause})"),
            connection: true,
        }
    } else {
        FetchError::other(format!("Couldn't load {url} ({cause})"))
    }
}

fn is_certificate_error(err: &reqwest::Error) -> bool {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(e) = current {
        let tls = e.downcast_ref::<rustls::Error>().is_some()
            || e.downcast_ref::<std::io::Error>()
                .and_then(|io| io.get_ref())
                .is_some_and(|inner| inner.downcast_ref::<rustls::Error>().is_some());
        if tls || e.to_string().contains("certificate") {
            return true;
        }
        current = e.source();
    }
    false
}

pub fn parse_max_age(cache_control: &str) -> Option<i64> {
    cache_control.split(',').find_map(|directive| {
        let (key, value) = directive.trim().split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("max-age")
            .then(|| value.trim().trim_matches('"').parse().ok())
            .flatten()
    })
}

/// `Retry-After` is either seconds or an HTTP date.
pub fn parse_retry_after(value: &str, now: SystemTime) -> Option<i64> {
    if let Ok(seconds) = value.trim().parse::<i64>() {
        return Some(seconds.max(0));
    }
    let at = httpdate::parse_http_date(value.trim()).ok()?;
    Some(at.duration_since(now).map_or(0, |d| d.as_secs() as i64))
}

/// Accepts what people paste into "Add feed": bare domains, `feed:` URLs, surrounding spaces.
pub fn normalize_input_url(input: &str) -> AppResult<Url> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input("Enter a feed or website address"));
    }
    let without_feed_scheme = trimmed
        .strip_prefix("feed:")
        .or_else(|| trimmed.strip_prefix("rss:"))
        .map(|rest| rest.trim_start_matches("//"))
        .map(|rest| {
            if rest.starts_with("http://") || rest.starts_with("https://") {
                rest.to_string()
            } else {
                format!("https://{rest}")
            }
        })
        .unwrap_or_else(|| trimmed.to_string());
    let candidate = if without_feed_scheme.contains("://") {
        without_feed_scheme
    } else {
        format!("https://{without_feed_scheme}")
    };
    let url = Url::parse(&candidate)
        .map_err(|_| AppError::invalid_input(format!("\"{trimmed}\" isn't a web address")))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(AppError::invalid_input(format!(
            "\"{trimmed}\" isn't a web address"
        )));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cache_hints() {
        assert_eq!(parse_max_age("public, max-age=3600"), Some(3600));
        assert_eq!(parse_max_age("no-cache"), None);
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
        assert_eq!(parse_retry_after("120", now), Some(120));
        assert_eq!(
            parse_retry_after(&httpdate::fmt_http_date(now + Duration::from_secs(90)), now),
            Some(90)
        );
        assert_eq!(parse_retry_after("soon", now), None);
    }

    #[test]
    fn normalises_what_people_paste() {
        let n = |s: &str| normalize_input_url(s).map(|u| u.to_string());
        assert_eq!(n("example-blog.com").unwrap(), "https://example-blog.com/");
        assert_eq!(
            n("  http://x.example/feed ").unwrap(),
            "http://x.example/feed"
        );
        assert_eq!(n("feed://x.example/rss").unwrap(), "https://x.example/rss");
        assert_eq!(
            n("feed:https://x.example/rss").unwrap(),
            "https://x.example/rss"
        );
        assert!(n("").is_err());
        assert!(n("ftp://x.example").is_err());
        assert!(n("not a url at all").is_err());
    }
}
