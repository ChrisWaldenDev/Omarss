//! The `omarss-img://` protocol (SPEC §8.4). For now it serves cached favicons
//! (`icon/<file>`); proxied article images come later.

use std::path::Path;

use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::services::feeds::FeedService;

pub const SCHEME: &str = "omarss-img";

pub fn handle<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let icons_dir = ctx
        .app_handle()
        .try_state::<FeedService>()
        .map(|feeds| feeds.icons_dir().to_path_buf());
    let path = percent_decode(request.uri().path());
    tauri::async_runtime::spawn_blocking(move || {
        let response = match icons_dir {
            Some(dir) => serve(&dir, &path),
            None => not_found(),
        };
        responder.respond(response);
    });
}

fn serve(icons_dir: &Path, path: &str) -> Response<Vec<u8>> {
    let Some(name) = path.trim_start_matches('/').strip_prefix("icon/") else {
        return not_found();
    };
    if !is_icon_file_name(name) {
        return not_found();
    }
    match std::fs::read(icons_dir.join(name)) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            // Icon names change whenever the icon does, so they can be cached forever.
            .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
            .body(bytes)
            .unwrap_or_else(|_| not_found()),
        Err(_) => not_found(),
    }
}

/// `<digits>-<digits>.png` only, so requests can't reach outside the icons folder.
fn is_icon_file_name(name: &str) -> bool {
    name.strip_suffix(".png")
        .and_then(|stem| stem.split_once('-'))
        .is_some_and(|(a, b)| {
            !a.is_empty()
                && !b.is_empty()
                && a.bytes().all(|c| c.is_ascii_digit())
                && b.bytes().all(|c| c.is_ascii_digit())
        })
}

fn not_found() -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Vec::new())
        .expect("static response")
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let hex = |b: u8| (b as char).to_digit(16);
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((high * 16 + low) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_serves_icon_files() {
        assert!(is_icon_file_name("12-1790000000.png"));
        for bad in [
            "../12-1.png",
            "12-1.png/..",
            "a-1.png",
            "12-1.svg",
            "12.png",
            "-1.png",
            "12-.png",
        ] {
            assert!(!is_icon_file_name(bad), "{bad}");
        }
    }

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(percent_decode("/icon%2F12-1.png"), "/icon/12-1.png");
        assert_eq!(percent_decode("/a%zzb%2"), "/a%zzb%2");
    }

    #[test]
    fn serves_existing_icons_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("3-100.png"), b"png").unwrap();
        assert_eq!(serve(dir.path(), "/icon/3-100.png").body(), b"png");
        assert_eq!(
            serve(dir.path(), "/icon/4-100.png").status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            serve(dir.path(), "/other/3-100.png").status(),
            StatusCode::NOT_FOUND
        );
    }
}
