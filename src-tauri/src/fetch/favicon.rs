//! Favicons (SPEC §7.5): the feed's own image, else the site's `<link rel="icon">`, else
//! `/favicon.ico`; stored as a 32×32 PNG.

use std::io::Cursor;

use image::imageops::FilterType;
use image::{ImageFormat, RgbaImage};
use scraper::{Html, Selector};
use url::Url;

use super::{HttpClient, Request, ICON_MAX_BYTES, IMAGE_ACCEPT, PAGE_ACCEPT, PAGE_MAX_BYTES};

pub const ICON_SIZE: u32 = 32;

/// Returns a 32×32 PNG for the feed, trying each source in order, or `None`.
pub async fn fetch_icon(
    http: &HttpClient,
    feed_icon: Option<&str>,
    site_url: Option<&str>,
    feed_url: &str,
) -> Option<Vec<u8>> {
    if let Some(icon) = feed_icon {
        if let Some(png) = fetch_image(http, icon).await {
            return Some(png);
        }
    }
    let site = site_url
        .and_then(|s| Url::parse(s).ok())
        .or_else(|| Url::parse(feed_url).ok())?;
    if let Ok(page) = http
        .get(Request::new(site.as_str(), PAGE_ACCEPT, PAGE_MAX_BYTES))
        .await
    {
        if page.is_success() {
            let page_url = Url::parse(&page.url).unwrap_or_else(|_| site.clone());
            for icon in icon_links(&String::from_utf8_lossy(&page.body), &page_url) {
                if let Some(png) = fetch_image(http, &icon).await {
                    return Some(png);
                }
            }
        }
    }
    let fallback = site.join("/favicon.ico").ok()?;
    fetch_image(http, fallback.as_str()).await
}

async fn fetch_image(http: &HttpClient, url: &str) -> Option<Vec<u8>> {
    let response = http
        .get(Request::new(url, IMAGE_ACCEPT, ICON_MAX_BYTES))
        .await
        .ok()?;
    if !response.is_success() {
        return None;
    }
    to_icon_png(&response.body)
}

/// Decodes any common image format (PNG, ICO, JPEG, GIF, WebP, BMP) and fits it, centred, on
/// a transparent 32×32 canvas.
pub fn to_icon_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let image = image::load_from_memory(bytes).ok()?;
    let fitted = image
        .resize(ICON_SIZE, ICON_SIZE, FilterType::Lanczos3)
        .to_rgba8();
    let mut canvas = RgbaImage::new(ICON_SIZE, ICON_SIZE);
    let x = i64::from((ICON_SIZE - fitted.width()) / 2);
    let y = i64::from((ICON_SIZE - fitted.height()) / 2);
    image::imageops::overlay(&mut canvas, &fitted, x, y);
    let mut png = Vec::new();
    canvas
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .ok()?;
    Some(png)
}

/// Icon links in a page, most specific first (`icon`, then `shortcut icon`, then touch icons).
pub fn icon_links(html: &str, page_url: &Url) -> Vec<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("link[rel][href]").expect("valid selector");
    let mut ranked: Vec<(u8, String)> = document
        .select(&selector)
        .filter_map(|link| {
            let rel = link.value().attr("rel")?.to_ascii_lowercase();
            let rank = match rel.split_whitespace().collect::<Vec<_>>().as_slice() {
                ["icon"] => 0,
                ["shortcut", "icon"] | ["icon", "shortcut"] => 1,
                r if r.contains(&"apple-touch-icon") => 2,
                r if r.contains(&"icon") => 3,
                _ => return None,
            };
            let href = link.value().attr("href")?;
            Some((rank, crate::content::resolve_web_url(href, Some(page_url))?))
        })
        .collect();
    ranked.sort_by_key(|(rank, _)| *rank);
    ranked.into_iter().map(|(_, url)| url).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 30, 255]));
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn resizes_any_image_to_a_32px_square() {
        for (w, h) in [(16, 16), (64, 64), (100, 50)] {
            let out = to_icon_png(&png(w, h)).expect("decodes");
            let decoded = image::load_from_memory(&out).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (32, 32));
        }
        assert!(to_icon_png(b"<svg xmlns='http://www.w3.org/2000/svg'/>").is_none());
    }

    #[test]
    fn ranks_icon_links() {
        let html = r#"<head>
            <link rel="apple-touch-icon" href="/touch.png">
            <link rel="shortcut icon" href="/short.ico">
            <link rel="icon" type="image/png" href="icons/32.png">
            <link rel="stylesheet" href="/s.css"></head>"#;
        let page = Url::parse("https://site.example/blog/").unwrap();
        assert_eq!(
            icon_links(html, &page),
            [
                "https://site.example/blog/icons/32.png",
                "https://site.example/short.ico",
                "https://site.example/touch.png"
            ]
        );
    }
}
