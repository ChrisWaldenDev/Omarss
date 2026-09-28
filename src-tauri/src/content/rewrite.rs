//! Rewrites of already-sanitised HTML (SPEC §8.2–§8.4): video embeds become click-to-load
//! placeholders, tracking pixels are dropped, and at display time images go through the image
//! proxy and `utm_*` parameters leave links.
//!
//! Input is always ammonia's output, which html5ever serialises predictably: lowercase names,
//! double-quoted attribute values with `&`, `"` and no-break spaces escaped, and every `<` in
//! text escaped. A small start-tag scanner is therefore enough; this never sees raw feed HTML.

use url::Url;

use super::decode_entities;

/// A start tag found by [`scan`].
struct StartTag<'a> {
    name: &'a str,
    attrs: Vec<(String, String)>,
}

impl StartTag<'_> {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn set(&mut self, name: &str, value: String) {
        match self.attrs.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => slot.1 = value,
            None => self.attrs.push((name.to_string(), value)),
        }
    }

    fn remove(&mut self, name: &str) -> Option<String> {
        let index = self.attrs.iter().position(|(n, _)| n == name)?;
        Some(self.attrs.remove(index).1)
    }

    fn to_html(&self) -> String {
        let mut out = format!("<{}", self.name);
        for (name, value) in &self.attrs {
            out.push_str(&format!(" {name}=\"{}\"", escape_attr(value)));
        }
        out.push('>');
        out
    }
}

enum Action {
    Keep,
    /// Replace just the start tag (for void elements like `img`, the whole element).
    Replace(String),
    /// Replace everything up to and including the matching end tag.
    ReplaceElement(String),
}

/// Calls `on_tag` for every start tag and applies what it returns.
fn scan(html: &str, mut on_tag: impl FnMut(&mut StartTag) -> Action) -> String {
    let mut out = String::with_capacity(html.len());
    let mut copied = 0;
    let mut pos = 0;
    while let Some(offset) = html[pos..].find('<') {
        let lt = pos + offset;
        let Some((mut tag, end)) = parse_start_tag(html, lt) else {
            pos = lt + 1;
            continue;
        };
        match on_tag(&mut tag) {
            Action::Keep => pos = end,
            Action::Replace(replacement) => {
                out.push_str(&html[copied..lt]);
                out.push_str(&replacement);
                copied = end;
                pos = end;
            }
            Action::ReplaceElement(replacement) => {
                let close = format!("</{}>", tag.name);
                let element_end = html[end..]
                    .find(&close)
                    .map_or(end, |p| end + p + close.len());
                out.push_str(&html[copied..lt]);
                out.push_str(&replacement);
                copied = element_end;
                pos = element_end;
            }
        }
    }
    out.push_str(&html[copied..]);
    out
}

/// Parses the start tag at `lt` (which points at `<`); returns it and the index after `>`.
fn parse_start_tag(html: &str, lt: usize) -> Option<(StartTag<'_>, usize)> {
    let bytes = html.as_bytes();
    let mut i = lt + 1;
    if !bytes.get(i)?.is_ascii_alphabetic() {
        return None;
    }
    let name_start = i;
    while bytes
        .get(i)
        .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'-')
    {
        i += 1;
    }
    let name = &html[name_start..i];
    let mut attrs = Vec::new();
    loop {
        while bytes.get(i)?.is_ascii_whitespace() {
            i += 1;
        }
        match bytes.get(i)? {
            b'>' => return Some((StartTag { name, attrs }, i + 1)),
            b'/' => {
                i += 1;
                continue;
            }
            _ => {}
        }
        let attr_start = i;
        while bytes
            .get(i)
            .is_some_and(|b| !b.is_ascii_whitespace() && !matches!(b, b'=' | b'>' | b'/'))
        {
            i += 1;
        }
        let attr_name = html[attr_start..i].to_ascii_lowercase();
        while bytes.get(i)?.is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if bytes.get(i) == Some(&b'=') {
            i += 1;
            while bytes.get(i)?.is_ascii_whitespace() {
                i += 1;
            }
            let quote = *bytes.get(i)?;
            if quote == b'"' || quote == b'\'' {
                let end = i + 1 + html[i + 1..].find(quote as char)?;
                value = decode_entities(&html[i + 1..end]);
                i = end + 1;
            } else {
                let start = i;
                while bytes
                    .get(i)
                    .is_some_and(|b| !b.is_ascii_whitespace() && *b != b'>')
                {
                    i += 1;
                }
                value = decode_entities(&html[start..i]);
            }
        }
        if !attr_name.is_empty() {
            attrs.push((attr_name, value));
        }
    }
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('\u{a0}', "&nbsp;")
}

// ---- Ingest time ---------------------------------------------------------------------------

/// The canonical, privacy-friendly embed URL for a YouTube or Vimeo player (SPEC §8.2), or
/// `None` for anything else.
pub fn canonical_embed(src: &str) -> Option<String> {
    let src = src.trim();
    let absolute = if src.starts_with("//") {
        format!("https:{src}")
    } else {
        src.to_string()
    };
    let url = Url::parse(&absolute).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?;
    match host.strip_prefix("www.").unwrap_or(host) {
        "youtube.com" | "m.youtube.com" | "youtube-nocookie.com" => {
            let id = url.path().strip_prefix("/embed/")?;
            let valid = (1..=64).contains(&id.len())
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'));
            valid.then(|| format!("{YOUTUBE_EMBED}{id}"))
        }
        "player.vimeo.com" => {
            let id = url.path().strip_prefix("/video/")?;
            let valid = (1..=20).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit());
            valid.then(|| format!("{VIMEO_EMBED}{id}"))
        }
        _ => None,
    }
}

const YOUTUBE_EMBED: &str = "https://www.youtube-nocookie.com/embed/";
const VIMEO_EMBED: &str = "https://player.vimeo.com/video/";

/// A click-to-load placeholder for a canonical embed URL. The reader turns it into an iframe
/// only when clicked; until then nothing is loaded from the video site.
fn embed_placeholder(src: &str) -> String {
    let (label, watch) = if let Some(id) = src.strip_prefix(YOUTUBE_EMBED) {
        (
            "YouTube video",
            format!("https://www.youtube.com/watch?v={id}"),
        )
    } else if let Some(id) = src.strip_prefix(VIMEO_EMBED) {
        ("Vimeo video", format!("https://vimeo.com/{id}"))
    } else {
        return String::new();
    };
    format!(
        "<figure class=\"omarss-embed\" data-embed=\"{}\"><a href=\"{}\" rel=\"noopener noreferrer\">{label}</a></figure>",
        escape_attr(src),
        escape_attr(&watch)
    )
}

/// Final pass after ammonia at ingest: allowed iframes become embed placeholders (others are
/// removed with their content), and tracking pixels and source-less images are dropped.
pub fn finish_sanitized(html: &str) -> String {
    if !html.contains("<iframe") && !html.contains("<img") {
        return html.to_string();
    }
    scan(html, |tag| match tag.name {
        "iframe" => Action::ReplaceElement(
            tag.attr("src")
                .and_then(canonical_embed)
                .map(|src| embed_placeholder(&src))
                .unwrap_or_default(),
        ),
        "img" if is_unwanted_image(tag) => Action::Replace(String::new()),
        _ => Action::Keep,
    })
}

fn is_unwanted_image(tag: &StartTag) -> bool {
    let Some(src) = tag.attr("src").filter(|s| !s.trim().is_empty()) else {
        return true;
    };
    let dimension = |name| tag.attr(name).and_then(|v| v.trim().parse::<u32>().ok());
    let pixel =
        matches!((dimension("width"), dimension("height")), (Some(w), Some(h)) if w <= 1 && h <= 1);
    pixel || is_tracker(src)
}

/// Hosts (and host + path prefixes) that only serve tracking pixels and analytics (SPEC §8.4).
const TRACKERS: &[&str] = &[
    "feeds.feedburner.com/~r/",
    "feeds.feedburner.com/~ff/",
    "feedproxy.google.com/~r/",
    "pixel.wp.com",
    "stats.wordpress.com",
    "pixel.quantserve.com",
    "pi.feedsportal.com",
    "www.google-analytics.com",
    "google-analytics.com",
    "ssl.google-analytics.com",
    "stats.g.doubleclick.net",
    "ad.doubleclick.net",
    "pubads.g.doubleclick.net",
    "www.facebook.com/tr",
    "analytics.twitter.com",
    "t.co/i/adsct",
    "sb.scorecardresearch.com",
    "b.scorecardresearch.com",
    "counter.theconversation.com",
    "piwik.",
    "matomo.",
    "api.segment.io",
    "ct.pinterest.com",
    "px.ads.linkedin.com",
    "bat.bing.com",
    "mailchi.mp/track",
    "list-manage.com/track",
    "open.substack.com/track",
    "www.assoc-amazon.com",
    "rss.buysellads.com",
    "feeds.wordpress.com/1.0/comments/",
];

/// Whether `url` points at a known tracker (SPEC §8.4's small built-in blocklist).
pub fn is_tracker(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url.trim()) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let location = format!("{host}{}", parsed.path());
    TRACKERS.iter().any(|entry| {
        if entry.ends_with('.') {
            host.starts_with(entry)
        } else if entry.contains('/') {
            location.starts_with(entry)
        } else {
            host == *entry || host.ends_with(&format!(".{entry}"))
        }
    })
}

/// A list thumbnail for an article: its first reasonably sized http(s) image.
pub fn first_image(html: &str) -> Option<String> {
    let mut found = None;
    scan(html, |tag| {
        if found.is_none() && tag.name == "img" {
            let small = ["width", "height"].iter().any(|name| {
                tag.attr(name)
                    .and_then(|v| v.trim().parse::<u32>().ok())
                    .is_some_and(|v| v < 48)
            });
            let src = tag.attr("src").unwrap_or_default();
            if !small && (src.starts_with("https://") || src.starts_with("http://")) {
                found = Some(src.to_string());
            }
        }
        Action::Keep
    });
    found
}

// ---- Display time --------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageDisplay {
    /// Images load through the proxy.
    Load,
    /// Image addresses move to `data-omarss-*` attributes; the reader swaps them back when the
    /// user asks to load images.
    ClickToLoad,
}

pub struct DisplayOptions<'a> {
    pub images: ImageDisplay,
    pub strip_tracking_params: bool,
    /// Turns a remote image URL into its image-proxy URL.
    pub proxy: &'a dyn Fn(&str) -> String,
}

/// Prepares stored article HTML for the reader: images go through the image proxy (or wait for
/// a click), `utm_*` parameters leave links, and known trackers are removed (SPEC §8.4).
pub fn for_display(html: &str, options: &DisplayOptions) -> String {
    scan(html, |tag| {
        let before = tag.to_html();
        match tag.name {
            "img" => {
                if tag.attr("src").is_some_and(is_tracker) {
                    return Action::Replace(String::new());
                }
                rewrite_image_attr(tag, "src", options);
                rewrite_image_attr(tag, "srcset", options);
            }
            "source" => rewrite_image_attr(tag, "srcset", options),
            "video" => rewrite_image_attr(tag, "poster", options),
            "a" if options.strip_tracking_params => {
                if let Some(href) = tag.attr("href").map(strip_tracking_params) {
                    tag.set("href", href);
                }
            }
            _ => return Action::Keep,
        }
        let after = tag.to_html();
        if after == before {
            Action::Keep
        } else {
            Action::Replace(after)
        }
    })
}

fn rewrite_image_attr(tag: &mut StartTag, name: &str, options: &DisplayOptions) {
    let Some(value) = tag.remove(name) else {
        return;
    };
    let proxied = if name == "srcset" {
        value
            .split(',')
            .map(|candidate| {
                let candidate = candidate.trim();
                match candidate.split_once(char::is_whitespace) {
                    Some((url, descriptor)) => {
                        format!("{} {}", proxy_one(url, options), descriptor.trim())
                    }
                    None => proxy_one(candidate, options),
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        proxy_one(&value, options)
    };
    let target = match options.images {
        ImageDisplay::Load => name.to_string(),
        ImageDisplay::ClickToLoad if value.starts_with("data:") => name.to_string(),
        ImageDisplay::ClickToLoad => format!("data-omarss-{name}"),
    };
    tag.set(&target, proxied);
}

fn proxy_one(url: &str, options: &DisplayOptions) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        (options.proxy)(url)
    } else {
        url.to_string()
    }
}

/// Removes `utm_*` query parameters (SPEC §8.4). Anything that isn't a URL is returned as is.
pub fn strip_tracking_params(url: &str) -> String {
    let Ok(mut parsed) = Url::parse(url) else {
        return url.to_string();
    };
    if parsed.query().is_none() {
        return url.to_string();
    }
    let kept: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(key, _)| !key.to_ascii_lowercase().starts_with("utm_"))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let removed = parsed.query_pairs().count() != kept.len();
    if !removed {
        return url.to_string();
    }
    if kept.is_empty() {
        parsed.set_query(None);
    } else {
        parsed.query_pairs_mut().clear().extend_pairs(kept);
    }
    parsed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxy(url: &str) -> String {
        format!("omarss-img://localhost/img/{}", url.len())
    }

    fn display(html: &str, images: ImageDisplay, strip: bool) -> String {
        for_display(
            html,
            &DisplayOptions {
                images,
                strip_tracking_params: strip,
                proxy: &proxy,
            },
        )
    }

    #[test]
    fn scanner_handles_quotes_and_entities() {
        let html = r#"<p title="a>b">x</p><img alt="Tom &amp; &quot;Jerry&quot;" src="https://a.example/x.png?a=1&amp;b=2">"#;
        let mut seen = Vec::new();
        let out = scan(html, |tag| {
            seen.push((tag.name.to_string(), tag.attrs.clone()));
            Action::Keep
        });
        assert_eq!(out, html);
        assert_eq!(seen[0].1, vec![("title".into(), "a>b".into())]);
        assert_eq!(
            seen[1].1,
            vec![
                ("alt".into(), "Tom & \"Jerry\"".into()),
                ("src".into(), "https://a.example/x.png?a=1&b=2".into())
            ]
        );
    }

    #[test]
    fn embeds_are_canonicalised() {
        for (src, expected) in [
            (
                "https://www.youtube.com/embed/dQw4w9WgXcQ?rel=0",
                Some("https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ"),
            ),
            (
                "//www.youtube-nocookie.com/embed/abc_-1",
                Some("https://www.youtube-nocookie.com/embed/abc_-1"),
            ),
            (
                "https://player.vimeo.com/video/76979871?h=1",
                Some("https://player.vimeo.com/video/76979871"),
            ),
            ("https://evil.example/embed/x", None),
            ("https://www.youtube.com/watch?v=x", None),
            ("https://www.youtube.com/embed/<script>", None),
            ("https://player.vimeo.com/video/12ab", None),
            ("javascript:alert(1)", None),
        ] {
            assert_eq!(canonical_embed(src).as_deref(), expected, "{src}");
        }
    }

    #[test]
    fn iframes_become_placeholders_or_disappear() {
        let out = finish_sanitized(
            r#"<p>a</p><iframe src="https://www.youtube-nocookie.com/embed/abc">fallback <b>x</b></iframe><iframe>gone</iframe><p>b</p>"#,
        );
        assert_eq!(
            out,
            "<p>a</p><figure class=\"omarss-embed\" data-embed=\"https://www.youtube-nocookie.com/embed/abc\"><a href=\"https://www.youtube.com/watch?v=abc\" rel=\"noopener noreferrer\">YouTube video</a></figure><p>b</p>"
        );
    }

    #[test]
    fn tracking_pixels_are_removed() {
        let out = finish_sanitized(
            r#"<p>x</p><img src="https://a.example/p.gif" width="1" height="1"><img src="https://pixel.wp.com/g.gif?x=1"><img src="https://feeds.feedburner.com/~r/Blog/~4/abc"><img alt="none"><img src="https://a.example/photo.jpg" width="1" height="400">"#,
        );
        assert_eq!(
            out,
            r#"<p>x</p><img src="https://a.example/photo.jpg" width="1" height="400">"#
        );
    }

    #[test]
    fn tracker_matching_is_by_host_or_prefix() {
        assert!(is_tracker("https://stats.wordpress.com/b.gif"));
        assert!(is_tracker("https://x.google-analytics.com/collect"));
        assert!(is_tracker("https://piwik.example.org/piwik.php"));
        assert!(!is_tracker("https://notgoogle-analytics.com/a.png"));
        assert!(!is_tracker("https://feeds.feedburner.com/Blog"));
        assert!(!is_tracker("not a url"));
    }

    #[test]
    fn thumbnails_skip_tiny_and_non_web_images() {
        assert_eq!(
            first_image(
                r#"<img src="data:image/png;base64,AA"><img src="https://s.w.org/emoji.png" width="16" height="16"><img src="https://a.example/big.jpg"><img src="https://a.example/second.jpg">"#
            )
            .as_deref(),
            Some("https://a.example/big.jpg")
        );
        assert_eq!(first_image("<p>no images</p>"), None);
    }

    #[test]
    fn display_proxies_images() {
        let out = display(
            r#"<img src="https://a.example/x.png" srcset="https://a.example/x.png 1x, https://a.example/x2.png 2x" alt="A"><img src="data:image/gif;base64,AA"><video poster="https://a.example/p.jpg" src="https://a.example/v.mp4"></video>"#,
            ImageDisplay::Load,
            false,
        );
        assert_eq!(
            out,
            r#"<img alt="A" src="omarss-img://localhost/img/23" srcset="omarss-img://localhost/img/23 1x, omarss-img://localhost/img/24 2x"><img src="data:image/gif;base64,AA"><video src="https://a.example/v.mp4" poster="omarss-img://localhost/img/23"></video>"#
        );
    }

    #[test]
    fn click_to_load_moves_addresses_aside() {
        let out = display(
            r#"<img src="https://a.example/x.png" alt="A">"#,
            ImageDisplay::ClickToLoad,
            false,
        );
        assert_eq!(
            out,
            r#"<img alt="A" data-omarss-src="omarss-img://localhost/img/23">"#
        );
    }

    #[test]
    fn display_strips_utm_params_and_trackers() {
        let out = display(
            r#"<a href="https://a.example/p?utm_source=rss&amp;id=3&amp;UTM_Medium=x" rel="noopener noreferrer">l</a><img src="https://pixel.wp.com/g.gif">"#,
            ImageDisplay::Load,
            true,
        );
        assert_eq!(
            out,
            r#"<a href="https://a.example/p?id=3" rel="noopener noreferrer">l</a>"#
        );
        let kept = display(
            r#"<a href="https://a.example/p?utm_source=rss">l</a>"#,
            ImageDisplay::Load,
            false,
        );
        assert_eq!(
            kept,
            r#"<a href="https://a.example/p?utm_source=rss">l</a>"#
        );
    }

    #[test]
    fn strips_only_utm_params() {
        assert_eq!(
            strip_tracking_params("https://a.example/?utm_source=x&utm_campaign=y"),
            "https://a.example/"
        );
        assert_eq!(
            strip_tracking_params("https://a.example/?q=rust&utm_source=x#frag"),
            "https://a.example/?q=rust#frag"
        );
        assert_eq!(
            strip_tracking_params("https://a.example/?q=a%20b"),
            "https://a.example/?q=a%20b"
        );
        assert_eq!(strip_tracking_params("mailto:x@y"), "mailto:x@y");
    }
}
