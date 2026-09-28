//! Keeps the WebView on the app's own pages: links never open inside the app (SPEC §8.2).

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{AppHandle, Manager, Runtime, Url};
use tauri_plugin_opener::OpenerExt;

use crate::error::{AppError, AppResult};

const EXTERNAL_SCHEMES: [&str; 3] = ["http", "https", "mailto"];

pub fn is_external_link(url: &Url) -> bool {
    EXTERNAL_SCHEMES.contains(&url.scheme())
}

pub fn open_external<R: Runtime>(app: &AppHandle<R>, url: &Url) -> AppResult<()> {
    if !is_external_link(url) {
        return Err(AppError::invalid_input(format!(
            "Refusing to open a {} link",
            url.scheme()
        )));
    }
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|err| AppError::internal(format!("Could not open the link: {err}")))
}

/// Hosts that click-to-load video embeds (SPEC §8.2) navigate their iframes to. On Linux,
/// WebKitGTK reports iframe navigations to the guard as well, so these must load in place
/// rather than open in the browser. The CSP's `frame-src` still limits which embeds the app
/// itself creates.
const EMBED_HOSTS: [&str; 7] = [
    "youtube-nocookie.com",
    "youtube.com",
    "google.com",
    "gstatic.com",
    "vimeo.com",
    "vimeocdn.com",
    "doubleclick.net",
];

fn is_embed_navigation(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host_str().is_some_and(|host| {
            EMBED_HOSTS
                .iter()
                .any(|h| host == *h || host.ends_with(&format!(".{h}")))
        })
}

/// Whether `url` is one of the app's own pages (bundled assets, or the dev server in debug).
fn is_app_url(url: &Url, dev_url: Option<&Url>) -> bool {
    match url.scheme() {
        "tauri" => true,
        "about" => url.as_str() == "about:blank",
        "http" | "https" if url.host_str() == Some("tauri.localhost") => true,
        _ => dev_url.is_some_and(|dev| dev.origin() == url.origin()),
    }
}

/// Plugin that cancels navigation away from the app and hands web links to the OS instead.
pub fn guard<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("navigation-guard")
        .on_navigation(|webview, url| {
            let config = webview.config();
            let dev_url = if cfg!(debug_assertions) {
                config.build.dev_url.as_ref()
            } else {
                None
            };
            if is_app_url(url, dev_url) || is_embed_navigation(url) {
                return true;
            }
            if is_external_link(url) {
                if let Err(err) = open_external(webview.app_handle(), url) {
                    tracing::warn!(%err, "failed to open external link");
                }
            } else {
                tracing::warn!(scheme = url.scheme(), "blocked navigation");
            }
            false
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn app_pages_are_allowed() {
        assert!(is_app_url(&url("tauri://localhost/index.html"), None));
        assert!(is_app_url(&url("http://tauri.localhost/"), None));
        let dev = url("http://localhost:1420");
        assert!(is_app_url(
            &url("http://localhost:1420/src/main.ts"),
            Some(&dev)
        ));
        assert!(!is_app_url(&url("http://localhost:1420/"), None));
    }

    #[test]
    fn web_pages_are_not_app_pages() {
        let dev = url("http://localhost:1420");
        assert!(!is_app_url(&url("https://example.com/"), Some(&dev)));
        assert!(!is_app_url(&url("http://localhost:8080/"), Some(&dev)));
        assert!(!is_app_url(&url("file:///etc/passwd"), None));
    }

    #[test]
    fn video_embeds_navigate_in_place() {
        assert!(is_embed_navigation(&url(
            "https://www.youtube-nocookie.com/embed/abc"
        )));
        assert!(is_embed_navigation(&url(
            "https://player.vimeo.com/video/1"
        )));
        assert!(!is_embed_navigation(&url("http://www.youtube.com/")));
        assert!(!is_embed_navigation(&url("https://notyoutube.com/")));
        assert!(!is_embed_navigation(&url("https://example.com/")));
    }

    #[test]
    fn only_web_and_mail_links_open_externally() {
        assert!(is_external_link(&url("https://example.com/a")));
        assert!(is_external_link(&url("mailto:someone@example.com")));
        assert!(!is_external_link(&url("file:///etc/passwd")));
        assert!(!is_external_link(&url("javascript:alert(1)")));
    }
}
