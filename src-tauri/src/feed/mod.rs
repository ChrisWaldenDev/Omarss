//! Feed parsing (SPEC §7.3, §7.4). Wraps `feed-rs`; the rest of the app only sees
//! [`ParsedFeed`]. RSS 0.9x/1.0/2.0, Atom and JSON Feed are supported.

mod dates;
mod encoding;

use feed_rs::model::{self, Entry, FeedType, Text};
use url::Url;

use crate::content::{
    collapse_whitespace, decode_entities, html_to_text, sanitize_html, sha256_hex, text_to_html,
    truncate_words,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFeed {
    pub title: Option<String>,
    pub site_url: Option<String>,
    pub description: Option<String>,
    /// Feed-provided image/icon/logo, used for the favicon.
    pub icon_url: Option<String>,
    /// How often the publisher says the feed changes (RSS `ttl`, `sy:updatePeriod`), in seconds.
    pub update_hint_secs: Option<i64>,
    pub items: Vec<ParsedItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedItem {
    /// Dedupe key (SPEC §7.4): the item's GUID/ID, else its link, else a hash.
    pub guid: String,
    pub url: Option<String>,
    pub title: String,
    pub author: Option<String>,
    /// Sanitised HTML.
    pub summary_html: Option<String>,
    /// Sanitised HTML.
    pub content_html: Option<String>,
    pub published_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub enclosures: Vec<ParsedEnclosure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEnclosure {
    pub url: String,
    pub mime_type: Option<String>,
    pub length: Option<i64>,
}

/// Parses a feed document. `feed_url` resolves relative URLs and is part of generated GUIDs.
pub fn parse(
    bytes: &[u8],
    content_type: Option<&str>,
    feed_url: &str,
) -> Result<ParsedFeed, String> {
    let text = encoding::decode(bytes, content_type);
    let feed = match parse_text(&text, feed_url) {
        Ok(feed) => feed,
        Err(first) => {
            let repaired = encoding::repair(&text);
            if repaired == text {
                return Err(first);
            }
            parse_text(&repaired, feed_url).map_err(|_| first)?
        }
    };
    Ok(convert(feed, feed_url, &text))
}

fn parse_text(text: &str, feed_url: &str) -> Result<model::Feed, String> {
    feed_rs::parser::Builder::new()
        .base_uri(Some(feed_url))
        .sanitize_content(false)
        // Leave missing IDs empty so we can apply our own rule (SPEC §7.4).
        .id_generator(|_, _, _| String::new())
        .timestamp_parser(dates::parse_date)
        .build()
        .parse(text.as_bytes())
        .map_err(|err| format!("Not a valid feed ({err})"))
}

fn convert(feed: model::Feed, feed_url: &str, raw: &str) -> ParsedFeed {
    let feed_base = Url::parse(feed_url).ok();
    let site_url = feed
        .links
        .iter()
        .find(|l| matches!(l.rel.as_deref(), None | Some("alternate")))
        .and_then(|l| absolute(&l.href, feed_base.as_ref()));
    let icon_url = feed
        .icon
        .as_ref()
        .or(feed.logo.as_ref())
        .and_then(|image| absolute(&image.uri, feed_base.as_ref()));
    let ttl_hint = feed.ttl.map(|minutes| i64::from(minutes) * 60);
    let update_hint_secs = match (ttl_hint, syndication_hint(raw)) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };
    let is_json = feed.feed_type == FeedType::JSON;

    let items = feed
        .entries
        .into_iter()
        .map(|entry| convert_entry(entry, feed_url, feed_base.as_ref(), is_json))
        .collect();

    ParsedFeed {
        title: feed
            .title
            .as_ref()
            .map(plain_text)
            .filter(|t| !t.is_empty()),
        site_url,
        description: feed
            .description
            .as_ref()
            .map(plain_text)
            .filter(|t| !t.is_empty()),
        icon_url,
        update_hint_secs,
        items,
    }
}

fn convert_entry(
    entry: Entry,
    feed_url: &str,
    feed_base: Option<&Url>,
    is_json: bool,
) -> ParsedItem {
    // JSON Feed attachments arrive as links with a media type; they're never the item link.
    let is_page_link = |l: &&model::Link| !(is_json && l.media_type.is_some());
    let url = entry
        .links
        .iter()
        .filter(is_page_link)
        .find(|l| matches!(l.rel.as_deref(), None | Some("alternate")))
        .or_else(|| {
            entry
                .links
                .iter()
                .filter(is_page_link)
                .find(|l| l.rel.as_deref() != Some("enclosure"))
        })
        .and_then(|l| absolute(&l.href, feed_base));

    // Relative URLs in content resolve against the item link, then xml:base, then the feed URL.
    let content_base = url
        .as_deref()
        .and_then(|u| Url::parse(u).ok())
        .or_else(|| entry.base.as_deref().and_then(|b| Url::parse(b).ok()))
        .or_else(|| feed_base.cloned());

    // Media RSS entries (e.g. video channels) often only describe themselves in media:group.
    let summary_text = entry
        .summary
        .as_ref()
        .or_else(|| entry.media.iter().find_map(|m| m.description.as_ref()));
    let summary_html = summary_text
        .map(|t| text_as_html(&t.content, t.content_type.as_ref()))
        .map(|html| sanitize_html(&html, content_base.as_ref()))
        .filter(|html| !html.trim().is_empty());
    let content_html = entry
        .content
        .as_ref()
        .and_then(|c| {
            let body = c.body.as_deref()?;
            Some(text_as_html(body, c.content_type.as_ref()))
        })
        .map(|html| sanitize_html(&html, content_base.as_ref()))
        .filter(|html| !html.trim().is_empty());

    let published_at = entry.published.or(entry.updated).map(|d| d.timestamp());
    let updated_at = entry.updated.map(|d| d.timestamp());

    let title = entry
        .title
        .as_ref()
        .map(plain_text)
        .filter(|t| !t.is_empty())
        .or_else(|| {
            summary_html
                .as_deref()
                .or(content_html.as_deref())
                .map(|html| html_to_text(html, 80))
                .filter(|t| !t.is_empty())
        })
        .unwrap_or_else(|| "Untitled".to_string());

    let guid = match entry.id.trim() {
        "" => match &url {
            Some(link) => link.clone(),
            None => sha256_hex(&format!(
                "{}{}{}",
                title,
                published_at.map(|p| p.to_string()).unwrap_or_default(),
                feed_url
            )),
        },
        id => id.to_string(),
    };

    let author = entry
        .authors
        .iter()
        .find_map(|p| p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()))
        .map(|name| collapse_whitespace(&decode_entities(name)));

    ParsedItem {
        guid,
        url,
        title,
        author,
        summary_html,
        content_html,
        published_at,
        updated_at,
        enclosures: enclosures(&entry, feed_base, is_json),
    }
}

/// `<enclosure>` and `media:content` (feed-rs puts both in `media`), Atom `rel="enclosure"`
/// links, and JSON Feed attachments (extra links with a media type).
fn enclosures(entry: &Entry, base: Option<&Url>, is_json: bool) -> Vec<ParsedEnclosure> {
    let mut out: Vec<ParsedEnclosure> = Vec::new();
    let mut push = |url: Option<String>, mime_type: Option<String>, length: Option<u64>| {
        if let Some(url) = url {
            if !out.iter().any(|e| e.url == url) {
                out.push(ParsedEnclosure {
                    url,
                    mime_type,
                    length: length
                        .and_then(|l| i64::try_from(l).ok())
                        .filter(|l| *l > 0),
                });
            }
        }
    };
    for media in &entry.media {
        for content in &media.content {
            push(
                content
                    .url
                    .as_ref()
                    .and_then(|u| absolute(u.as_str(), base)),
                content.content_type.as_ref().map(|t| t.to_string()),
                content.size,
            );
        }
    }
    for link in &entry.links {
        let attachment = is_json && link.rel.is_none() && link.media_type.is_some();
        if link.rel.as_deref() == Some("enclosure") || attachment {
            push(
                absolute(&link.href, base),
                link.media_type.clone(),
                link.length,
            );
        }
    }
    out
}

fn absolute(href: &str, base: Option<&Url>) -> Option<String> {
    crate::content::resolve_web_url(href, base)
}

/// Feed text fields are HTML or plain text; both become plain text for titles.
fn plain_text(text: &Text) -> String {
    let content_type = text.content_type.to_string();
    if content_type.contains("html") {
        html_to_text(&text.content, 0)
    } else {
        collapse_whitespace(&decode_entities(&text.content))
    }
}

fn text_as_html(body: &str, content_type: &str) -> String {
    if content_type.contains("html") {
        body.to_string()
    } else {
        text_to_html(body)
    }
}

/// `sy:updatePeriod` / `sy:updateFrequency` (RSS syndication module) as seconds per update.
fn syndication_hint(raw: &str) -> Option<i64> {
    let period = tag_text(raw, "updatePeriod")?;
    let period_secs: i64 = match period.to_ascii_lowercase().as_str() {
        "hourly" => 3_600,
        "daily" => 86_400,
        "weekly" => 604_800,
        "monthly" => 2_592_000,
        "yearly" => 31_536_000,
        _ => return None,
    };
    let frequency = tag_text(raw, "updateFrequency")
        .and_then(|f| f.parse::<i64>().ok())
        .filter(|f| *f > 0)
        .unwrap_or(1);
    Some(period_secs / frequency)
}

/// Text of the first `<prefix:local>` element in `raw` (any namespace prefix).
fn tag_text<'a>(raw: &'a str, local: &str) -> Option<&'a str> {
    let needle = format!(":{local}>");
    let open_end = raw.find(&needle)? + needle.len();
    let rest = &raw[open_end..];
    let close = rest.find("</")?;
    Some(rest[..close].trim())
}

/// Shortens feed descriptions for storage.
pub fn short_description(description: Option<&str>) -> Option<String> {
    description.map(|d| truncate_words(d, 500))
}
