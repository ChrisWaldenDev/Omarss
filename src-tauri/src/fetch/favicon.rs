//! Favicons (SPEC §7.5): the feed's own image, else the site's `<link rel="icon">`, else
//! `/favicon.ico`; stored as a 32×32 PNG.

use std::io::Cursor;

use image::imageops::FilterType;
use image::{ImageFormat, ImageReader, Limits, RgbaImage};
use scraper::{Html, Selector};
use url::Url;

use super::{HttpClient, Request, ICON_MAX_BYTES, IMAGE_ACCEPT, PAGE_ACCEPT, PAGE_MAX_BYTES};

pub const ICON_SIZE: u32 = 32;
/// The largest source image decoded, per side (touch icons and logos are rarely over 512).
const MAX_ICON_DIMENSION: u32 = 1024;
/// The most memory one decode may allocate.
const MAX_ICON_ALLOC: u64 = 32 * 1024 * 1024;

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
/// a transparent 32×32 canvas. The bytes come from arbitrary servers, so decoding is capped
/// in size and memory: an icon never needs more than `MAX_ICON_DIMENSION` pixels a side.
pub fn to_icon_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_ICON_DIMENSION);
    limits.max_image_height = Some(MAX_ICON_DIMENSION);
    limits.max_alloc = Some(MAX_ICON_ALLOC);
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    reader.limits(limits);
    let image = reader.decode().ok()?;
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

    /// A PNG whose header declares `width`×`height` but holds no pixel data.
    fn png_header(width: u32, height: u32) -> Vec<u8> {
        fn crc32(bytes: &[u8]) -> u32 {
            let mut crc = !0u32;
            for &b in bytes {
                crc ^= u32::from(b);
                for _ in 0..8 {
                    crc = if crc & 1 == 1 {
                        (crc >> 1) ^ 0xEDB8_8320
                    } else {
                        crc >> 1
                    };
                }
            }
            !crc
        }
        fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
            let body = [kind.as_slice(), data].concat();
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(&body);
            out.extend_from_slice(&crc32(&body).to_be_bytes());
        }
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        // 8-bit RGBA, default compression/filter, no interlace.
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &[]);
        chunk(&mut out, b"IEND", &[]);
        out
    }

    #[test]
    fn rejects_images_declaring_huge_dimensions() {
        let huge = png_header(60_000, 60_000);
        let reader = ImageReader::new(Cursor::new(&huge))
            .with_guessed_format()
            .unwrap();
        assert_eq!(
            reader.into_dimensions().unwrap(),
            (60_000, 60_000),
            "the crafted header is valid"
        );
        assert!(to_icon_png(&huge).is_none());
        // Under the library's default 512 MiB cap, but still far too big for an icon: it's
        // refused before any pixel buffer is allocated.
        assert!(to_icon_png(&png_header(8_000, 8_000)).is_none());
        assert!(to_icon_png(&png(MAX_ICON_DIMENSION + 1, 1)).is_none());
        assert!(to_icon_png(&png(MAX_ICON_DIMENSION, 1)).is_some());
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
