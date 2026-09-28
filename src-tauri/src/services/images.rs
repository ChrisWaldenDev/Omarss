//! The image proxy's on-disk cache (SPEC §8.4). Article images load through
//! `omarss-img://…/img/<base64url(url)>`; the backend fetches them without cookies or a
//! `Referer`, keeps them for offline reading, and evicts the least recently used files once
//! the cache is over its size limit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use tokio::sync::Semaphore;

use crate::clock;
use crate::content::{is_tracker, sha256_hex};
use crate::fetch::{HttpClient, Request, IMAGE_ACCEPT, IMAGE_MAX_BYTES};

/// Concurrent image downloads.
const FETCH_CONCURRENCY: usize = 6;
/// A failed image isn't retried for this long (seconds).
const FAILURE_TTL: i64 = 600;
/// Eviction frees space down to this share of the limit, so it doesn't run on every write.
const EVICT_TO_PERCENT: u64 = 90;

/// URL through which the WebView loads a remote image. Custom protocols are
/// `http://<scheme>.localhost/` on Windows and `<scheme>://localhost/` elsewhere (the same
/// rule as `convertFileSrc` in the frontend).
pub fn proxy_url(url: &str) -> String {
    let encoded = URL_SAFE_NO_PAD.encode(url.as_bytes());
    if cfg!(windows) {
        format!("http://{}.localhost/img/{encoded}", crate::protocol::SCHEME)
    } else {
        format!("{}://localhost/img/{encoded}", crate::protocol::SCHEME)
    }
}

/// The image URL inside a proxy path (`img/<base64url>`), if it is a web address.
pub fn decode_proxy_path(encoded: &str) -> Option<String> {
    let bytes = URL_SAFE_NO_PAD.decode(encoded.trim_end_matches('=')).ok()?;
    let url = String::from_utf8(bytes).ok()?;
    (url.starts_with("http://") || url.starts_with("https://")).then_some(url)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedImage {
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone)]
pub struct ImageCache {
    inner: Arc<Inner>,
}

struct Inner {
    dir: PathBuf,
    http: HttpClient,
    max_bytes: AtomicU64,
    /// Bytes on disk; `None` until first measured.
    size: Mutex<Option<u64>>,
    fetches: Semaphore,
    failures: Mutex<HashMap<String, i64>>,
}

impl ImageCache {
    pub fn new(dir: PathBuf, http: HttpClient, max_mb: u32) -> Self {
        Self {
            inner: Arc::new(Inner {
                dir,
                http,
                max_bytes: AtomicU64::new(mb(max_mb)),
                size: Mutex::new(None),
                fetches: Semaphore::new(FETCH_CONCURRENCY),
                failures: Mutex::default(),
            }),
        }
    }

    /// Applies a new size limit, evicting straight away if the cache is now too big.
    pub fn set_max_mb(&self, max_mb: u32) {
        self.inner.max_bytes.store(mb(max_mb), Ordering::Relaxed);
        let this = self.clone();
        tauri::async_runtime::spawn_blocking(move || this.evict_if_needed());
    }

    /// Returns the image at `url`, from the cache or freshly downloaded. `None` if it can't be
    /// loaded, isn't an image, or is a known tracker.
    pub async fn load(&self, url: &str) -> Option<CachedImage> {
        if !(url.starts_with("http://") || url.starts_with("https://")) || is_tracker(url) {
            return None;
        }
        let path = self.path_for(url);
        let cached = {
            let path = path.clone();
            tauri::async_runtime::spawn_blocking(move || read_entry(&path))
                .await
                .ok()
                .flatten()
        };
        if cached.is_some() {
            return cached;
        }
        if self.recently_failed(url) {
            return None;
        }
        let _permit = self.inner.fetches.acquire().await.ok()?;
        let image = match self.download(url).await {
            Some(image) => image,
            None => {
                if let Ok(mut failures) = self.inner.failures.lock() {
                    failures.insert(url.to_string(), clock::now_unix());
                }
                return None;
            }
        };
        let this = self.clone();
        let stored = image.clone();
        tauri::async_runtime::spawn_blocking(move || match write_entry(&path, &stored) {
            Ok(written) => {
                this.add_size(written);
                this.evict_if_needed();
            }
            Err(err) => tracing::warn!(%err, "could not cache an image"),
        });
        Some(image)
    }

    async fn download(&self, url: &str) -> Option<CachedImage> {
        let response = self
            .inner
            .http
            .get(Request::new(url, IMAGE_ACCEPT, IMAGE_MAX_BYTES))
            .await
            .ok()?;
        if !response.is_success() || response.body.is_empty() {
            return None;
        }
        let content_type = image_type(response.content_type.as_deref(), &response.body)?;
        Some(CachedImage {
            content_type,
            bytes: response.body,
        })
    }

    fn recently_failed(&self, url: &str) -> bool {
        let Ok(mut failures) = self.inner.failures.lock() else {
            return false;
        };
        let now = clock::now_unix();
        failures.retain(|_, at| now - *at < FAILURE_TTL);
        failures.contains_key(url)
    }

    fn path_for(&self, url: &str) -> PathBuf {
        self.inner.dir.join(sha256_hex(url))
    }

    /// Bytes used by cached images (SPEC §6.9's storage view).
    pub fn size_bytes(&self) -> u64 {
        let size = scan(&self.inner.dir).iter().map(|e| e.len).sum();
        if let Ok(mut cached) = self.inner.size.lock() {
            *cached = Some(size);
        }
        size
    }

    /// Deletes every cached image ("Clear image cache", SPEC §6.9).
    pub fn clear(&self) -> std::io::Result<()> {
        for entry in scan(&self.inner.dir) {
            std::fs::remove_file(entry.path)?;
        }
        if let Ok(mut size) = self.inner.size.lock() {
            *size = Some(0);
        }
        Ok(())
    }

    fn add_size(&self, bytes: u64) {
        if let Ok(mut size) = self.inner.size.lock() {
            if let Some(size) = size.as_mut() {
                *size += bytes;
            }
        }
    }

    /// Deletes the least recently used files until the cache is under its limit.
    fn evict_if_needed(&self) {
        let max = self.inner.max_bytes.load(Ordering::Relaxed);
        let known = self.inner.size.lock().ok().and_then(|s| *s);
        if known.is_some_and(|size| size <= max) {
            return;
        }
        let mut entries = scan(&self.inner.dir);
        let mut total: u64 = entries.iter().map(|e| e.len).sum();
        if total > max {
            let target = max / 100 * EVICT_TO_PERCENT;
            entries.sort_by_key(|e| e.used);
            for entry in entries {
                if total <= target {
                    break;
                }
                if std::fs::remove_file(&entry.path).is_ok() {
                    total -= entry.len;
                }
            }
        }
        if let Ok(mut size) = self.inner.size.lock() {
            *size = Some(total);
        }
    }
}

/// Megabytes as the settings UI shows them (decimal, like file sizes elsewhere in the app).
fn mb(value: u32) -> u64 {
    u64::from(value) * 1_000_000
}

struct Entry {
    path: PathBuf,
    len: u64,
    used: SystemTime,
}

fn scan(dir: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.flatten()
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            meta.is_file().then(|| Entry {
                path: entry.path(),
                len: meta.len(),
                used: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            })
        })
        .collect()
}

/// Cache files hold the content type, a newline, then the image bytes.
fn read_entry(path: &Path) -> Option<CachedImage> {
    let data = std::fs::read(path).ok()?;
    let newline = data.iter().position(|b| *b == b'\n')?;
    let content_type = std::str::from_utf8(&data[..newline]).ok()?.to_string();
    // Reading counts as use: bump the modification time the LRU eviction sorts by.
    if let Ok(file) = std::fs::File::options().append(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
    Some(CachedImage {
        content_type,
        bytes: data[newline + 1..].to_vec(),
    })
}

fn write_entry(path: &Path, image: &CachedImage) -> std::io::Result<u64> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut data = Vec::with_capacity(image.content_type.len() + 1 + image.bytes.len());
    data.extend_from_slice(image.content_type.as_bytes());
    data.push(b'\n');
    data.extend_from_slice(&image.bytes);
    // Write then rename, so a reader never sees half a file.
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, &data)?;
    std::fs::rename(&tmp, path)?;
    Ok(data.len() as u64)
}

/// The image's media type, from the header when it names an image, else sniffed from the
/// bytes. `None` for anything that isn't an image (e.g. an HTML error page).
fn image_type(header: Option<&str>, body: &[u8]) -> Option<String> {
    let declared = header
        .and_then(|h| h.split(';').next())
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| h.starts_with("image/") && h.len() < 64);
    if let Some(declared) = declared {
        return Some(declared);
    }
    let sniffed = image::guess_format(body).ok()?;
    Some(sniffed.to_mime_type().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_urls_round_trip() {
        let url = "https://img.example/a b/ü.png?x=1&y=2";
        let proxied = proxy_url(url);
        let encoded = proxied.rsplit('/').next().unwrap();
        assert!(encoded
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
        assert_eq!(decode_proxy_path(encoded).as_deref(), Some(url));
        assert_eq!(
            decode_proxy_path(&URL_SAFE_NO_PAD.encode("file:///etc/passwd")),
            None
        );
        assert_eq!(decode_proxy_path("!!!"), None);
    }

    #[test]
    fn only_images_are_accepted() {
        assert_eq!(
            image_type(Some("image/jpeg; charset=binary"), b"").as_deref(),
            Some("image/jpeg")
        );
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
        assert_eq!(
            image_type(Some("application/octet-stream"), &png).as_deref(),
            Some("image/png")
        );
        assert_eq!(image_type(Some("text/html"), b"<html>"), None);
        assert_eq!(
            image_type(None, b"GIF89a....").as_deref(),
            Some("image/gif")
        );
    }

    #[test]
    fn entries_round_trip_and_eviction_removes_least_recently_used() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(
            dir.path().to_path_buf(),
            HttpClient::new("test", None).unwrap(),
            1,
        );
        let image = |n: usize| CachedImage {
            content_type: "image/png".into(),
            bytes: vec![7; n],
        };
        let old = cache.path_for("https://a.example/old.png");
        let new = cache.path_for("https://a.example/new.png");
        write_entry(&old, &image(600 * 1024)).unwrap();
        let past = SystemTime::now() - std::time::Duration::from_secs(3600);
        std::fs::File::options()
            .append(true)
            .open(&old)
            .unwrap()
            .set_modified(past)
            .unwrap();
        write_entry(&new, &image(600 * 1024)).unwrap();
        assert_eq!(read_entry(&new), Some(image(600 * 1024)));

        cache.evict_if_needed();
        assert!(!old.exists(), "least recently used goes first");
        assert!(new.exists());
        assert!(cache.size_bytes() <= 1_000_000);

        cache.clear().unwrap();
        assert_eq!(cache.size_bytes(), 0);
    }
}
