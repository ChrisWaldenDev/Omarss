//! Feed discovery (SPEC §6.1): the address may be a feed, or a web page that links to feeds.

use futures::future::join_all;
use scraper::{Html, Selector};
use url::Url;

use super::{HttpClient, Request, FEED_ACCEPT, FEED_MAX_BYTES, PAGE_ACCEPT, PAGE_MAX_BYTES};
use crate::error::{AppError, AppResult};
use crate::feed;
use crate::models::DiscoveredFeed;

const FEED_LINK_TYPES: [&str; 3] = [
    "application/rss+xml",
    "application/atom+xml",
    "application/feed+json",
];
/// JSON Feed 1.0 advertised itself as plain `application/json`, but so does every
/// WordPress page's REST API link (`/wp-json/...`). Links of this type are only kept if
/// they turn out to be a feed.
const UNVERIFIED_LINK_TYPE: &str = "application/json";
const COMMON_PATHS: [&str; 6] = [
    "/feed",
    "/rss",
    "/atom.xml",
    "/feed.xml",
    "/index.xml",
    "/rss.xml",
];

/// Finds the feeds for `input`: the address itself if it is a feed; otherwise the page's
/// `<link rel="alternate">` feeds; otherwise any of the common feed paths that exist.
pub async fn discover(http: &HttpClient, input: &Url) -> AppResult<Vec<DiscoveredFeed>> {
    let response = http
        .get(Request::new(input.as_str(), PAGE_ACCEPT, PAGE_MAX_BYTES))
        .await?;
    if !response.is_success() {
        return Err(AppError::network(format!(
            "{} responded with HTTP {}",
            host(input),
            response.status
        )));
    }
    if let Ok(parsed) = feed::parse(
        &response.body,
        response.content_type.as_deref(),
        &response.url,
    ) {
        return Ok(vec![DiscoveredFeed {
            url: response.permanent_url,
            title: parsed.title,
        }]);
    }

    let page_url = Url::parse(&response.url).unwrap_or_else(|_| input.clone());
    let links = feed_links(&String::from_utf8_lossy(&response.body), &page_url);
    let checked = links.into_iter().map(|link| async move {
        if !link.unverified {
            return Some(link.feed);
        }
        let probed = probe(http, &link.feed.url).await?;
        Some(DiscoveredFeed {
            url: probed.url,
            title: link.feed.title.or(probed.title),
        })
    });
    let linked = dedupe(join_all(checked).await.into_iter().flatten());
    if !linked.is_empty() {
        return Ok(linked);
    }

    let probes = COMMON_PATHS
        .iter()
        .filter_map(|path| page_url.join(path).ok())
        .map(|url| async move { probe(http, url.as_str()).await });
    let found = dedupe(join_all(probes).await.into_iter().flatten());
    if found.is_empty() {
        return Err(AppError::invalid_feed(format!("No feed found at {input}")));
    }
    Ok(found)
}

/// Fetches `url` and returns it if it parses as a feed.
async fn probe(http: &HttpClient, url: &str) -> Option<DiscoveredFeed> {
    let response = http
        .get(Request::new(url, FEED_ACCEPT, FEED_MAX_BYTES))
        .await
        .ok()?;
    if !response.is_success() {
        return None;
    }
    let parsed = feed::parse(
        &response.body,
        response.content_type.as_deref(),
        &response.url,
    )
    .ok()?;
    Some(DiscoveredFeed {
        url: response.permanent_url,
        title: parsed.title,
    })
}

/// Keeps the first feed for each URL, in order.
fn dedupe(candidates: impl IntoIterator<Item = DiscoveredFeed>) -> Vec<DiscoveredFeed> {
    let mut found: Vec<DiscoveredFeed> = Vec::new();
    for candidate in candidates {
        if !found.iter().any(|f| f.url == candidate.url) {
            found.push(candidate);
        }
    }
    found
}

/// A feed advertised by a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedLink {
    pub feed: DiscoveredFeed,
    /// Advertised as plain `application/json`, which may not be a feed at all: it must be
    /// fetched and parsed before it is offered.
    pub unverified: bool,
}

/// `<link rel="alternate" type="…feed type…" href="…">` elements, resolved against the page
/// (honouring `<base href>`).
pub fn feed_links(html: &str, page_url: &Url) -> Vec<FeedLink> {
    let document = Html::parse_document(html);
    let base_selector = Selector::parse("base[href]").expect("valid selector");
    let base = document
        .select(&base_selector)
        .next()
        .and_then(|b| b.value().attr("href"))
        .and_then(|href| page_url.join(href).ok())
        .unwrap_or_else(|| page_url.clone());

    let link_selector = Selector::parse("link[rel][href][type]").expect("valid selector");
    let mut feeds: Vec<FeedLink> = Vec::new();
    for link in document.select(&link_selector) {
        let el = link.value();
        let is_alternate = el.attr("rel").is_some_and(|rel| {
            rel.split_whitespace()
                .any(|r| r.eq_ignore_ascii_case("alternate"))
        });
        let media_type = el.attr("type").unwrap_or("").trim().to_ascii_lowercase();
        let media_type = media_type.split(';').next().unwrap_or("").trim();
        let unverified = media_type == UNVERIFIED_LINK_TYPE;
        if !is_alternate || !(unverified || FEED_LINK_TYPES.contains(&media_type)) {
            continue;
        }
        let Some(url) = el
            .attr("href")
            .and_then(|href| crate::content::resolve_web_url(href, Some(&base)))
        else {
            continue;
        };
        if !feeds.iter().any(|f| f.feed.url == url) {
            feeds.push(FeedLink {
                feed: DiscoveredFeed {
                    url,
                    title: el
                        .attr("title")
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty()),
                },
                unverified,
            });
        }
    }
    feeds
}

fn host(url: &Url) -> &str {
    url.host_str().unwrap_or("The server")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_alternate_feed_links() {
        let html = r#"<html><head>
            <link rel="stylesheet" type="text/css" href="/style.css">
            <link rel="alternate" type="application/rss+xml" title="Posts" href="/feed.xml">
            <link rel="Alternate" type="application/atom+xml; charset=utf-8" href="comments/atom">
            <link rel="alternate" type="application/feed+json" href="https://other.example/feed.json">
            <link rel="alternate" type="text/html" hreflang="fr" href="/fr/">
            <link rel="alternate" type="application/rss+xml" href="/feed.xml">
            </head><body></body></html>"#;
        let page = Url::parse("https://blog.example/posts/").unwrap();
        let feeds = feed_links(html, &page);
        let urls: Vec<&str> = feeds.iter().map(|f| f.feed.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://blog.example/feed.xml",
                "https://blog.example/posts/comments/atom",
                "https://other.example/feed.json"
            ]
        );
        assert_eq!(feeds[0].feed.title.as_deref(), Some("Posts"));
        assert!(feeds.iter().all(|f| !f.unverified));
    }

    #[test]
    fn honours_base_href() {
        let html = r#"<head><base href="https://cdn.example/site/"><link rel="alternate" type="application/rss+xml" href="rss"></head>"#;
        let feeds = feed_links(html, &Url::parse("https://blog.example/").unwrap());
        assert_eq!(feeds[0].feed.url, "https://cdn.example/site/rss");
    }

    #[test]
    fn plain_json_links_are_marked_unverified() {
        let html = r#"<head>
            <link rel="alternate" type="application/rss+xml" href="/feed/">
            <link rel="alternate" type="application/json" href="/wp-json/wp/v2/pages/42">
            </head>"#;
        let feeds = feed_links(html, &Url::parse("https://wp.example/about/").unwrap());
        let found: Vec<(&str, bool)> = feeds
            .iter()
            .map(|f| (f.feed.url.as_str(), f.unverified))
            .collect();
        assert_eq!(
            found,
            [
                ("https://wp.example/feed/", false),
                ("https://wp.example/wp-json/wp/v2/pages/42", true)
            ]
        );
    }
}
