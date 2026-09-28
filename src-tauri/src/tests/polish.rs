//! M3 features against a real (temporary) database and a mock HTTP server: mark all as read
//! with undo, OPML import/export, display preparation, the image cache and retention.

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::fixtures::TestApp;
use crate::clock;
use crate::error::ErrorKind;
use crate::fetch::HttpClient;
use crate::models::{ArticleQuery, OlderThan, SortOrder, View};
use crate::services::images::{decode_proxy_path, proxy_url, ImageCache};
use crate::services::maintenance::MaintenanceService;
use crate::services::opml::OpmlService;
use crate::services::settings::{LoadImages, Settings};
use crate::store::articles::{self, NewArticle};
use crate::store::feeds::{self, NewFeed};

const HOUR: i64 = 3_600;
const DAY: i64 = 86_400;

fn article(guid: String, published_at: i64, html: &str) -> NewArticle {
    NewArticle {
        url: Some(format!("https://site.example/{guid}?utm_source=rss&id=1")),
        guid,
        title: "Title".into(),
        author: None,
        summary_html: None,
        content_html: Some(html.to_string()),
        published_at,
        updated_at: None,
        content_hash: "h".into(),
        enclosures: Vec::new(),
        thumbnail_url: Some("https://img.example/thumb.jpg".into()),
    }
}

/// A feed with `count` articles `step` seconds apart, newest `newest`.
async fn seed(
    app: &TestApp,
    url: &str,
    folder_id: Option<i64>,
    count: i64,
    step: i64,
    newest: i64,
) -> i64 {
    let url = url.to_string();
    app.store
        .run(move |conn| {
            let tx = conn.transaction()?;
            let id = feeds::insert(
                &tx,
                &NewFeed {
                    url: &url,
                    title: &url,
                    custom_title: None,
                    site_url: None,
                    description: None,
                    folder_id,
                    now: newest,
                },
            )?;
            let items: Vec<NewArticle> = (0..count)
                .map(|i| article(format!("{id}-{i}"), newest - i * step, "<p>x</p>"))
                .collect();
            articles::upsert(&tx, id, &items, newest, false)?;
            tx.commit()?;
            Ok(id)
        })
        .await
        .unwrap()
}

async fn unread(app: &TestApp) -> u32 {
    app.articles.sidebar().await.unwrap().counts.unread
}

fn all(unread_only: bool) -> ArticleQuery {
    ArticleQuery {
        view: View::All,
        unread_only,
        sort: SortOrder::NewestFirst,
    }
}

#[tokio::test]
async fn mark_all_read_follows_the_view_and_age_and_can_be_undone() {
    let app = TestApp::new();
    let now = clock::now_unix();
    let folder = app.feeds.create_folder("Tech".into()).await.unwrap();
    let a = seed(
        &app,
        "https://a.example/feed",
        Some(folder),
        10,
        12 * HOUR,
        now,
    )
    .await;
    seed(&app, "https://b.example/feed", None, 4, HOUR, now).await;
    assert_eq!(unread(&app).await, 14);

    // Older than a day in feed A: articles 3–9 (36 h and older).
    let result = app
        .articles
        .mark_all_read(View::Feed { id: a }, Some(OlderThan::Day))
        .await
        .unwrap();
    assert_eq!(result.count, 7);
    assert_eq!(unread(&app).await, 7);

    let first = result.undo_token.unwrap();
    let folder_result = app
        .articles
        .mark_all_read(View::Folder { id: folder }, None)
        .await
        .unwrap();
    assert_eq!(folder_result.count, 3);
    assert_eq!(unread(&app).await, 4, "feed B is outside the folder");

    // Only the latest "mark all" can be undone.
    let stale = app.articles.undo_mark_all_read(first).await.unwrap_err();
    assert_eq!(stale.kind, ErrorKind::NotFound);
    let restored = app
        .articles
        .undo_mark_all_read(folder_result.undo_token.unwrap())
        .await
        .unwrap();
    assert_eq!(restored, 3);
    assert_eq!(unread(&app).await, 7);
    assert!(app
        .articles
        .undo_mark_all_read(folder_result.undo_token.unwrap())
        .await
        .is_err());

    let everything = app.articles.mark_all_read(View::All, None).await.unwrap();
    assert_eq!(everything.count, 7);
    assert_eq!(unread(&app).await, 0);
    let nothing = app.articles.mark_all_read(View::All, None).await.unwrap();
    assert_eq!((nothing.count, nothing.undo_token), (0, None));
}

#[tokio::test]
async fn opml_import_creates_folders_skips_duplicates_and_round_trips() {
    let app = TestApp::new();
    let now = clock::now_unix();
    seed(&app, "https://existing.example/feed", None, 1, HOUR, now).await;
    let existing_folder = app.feeds.create_folder("News".into()).await.unwrap();
    let opml = OpmlService::new(app.store.clone());

    let file = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0"><head><title>Export</title></head><body>
  <outline text="news">
    <outline type="rss" text="Daily" title="Daily" xmlUrl="https://daily.example/rss"/>
    <outline type="rss" text="Again" xmlUrl="https://existing.example/feed"/>
  </outline>
  <outline text="Tech">
    <outline text="Rust">
      <outline type="rss" text="My Rust" title="This Week in Rust" xmlUrl="feed://rust.example/rss.xml" htmlUrl="https://rust.example/"/>
    </outline>
  </outline>
  <outline type="rss" text="Top" xmlUrl="https://top.example/atom.xml"/>
  <outline type="rss" text="Twice" xmlUrl="https://top.example/atom.xml"/>
  <outline type="rss" text="Bad" xmlUrl="javascript:alert(1)"/>
</body></opml>"#;
    let summary = opml.import(file.as_bytes().to_vec()).await.unwrap();
    assert_eq!(
        (
            summary.feeds,
            summary.folders,
            summary.duplicates,
            summary.invalid
        ),
        (3, 1, 2, 1)
    );

    let sidebar = app.articles.sidebar().await.unwrap();
    let names: Vec<&str> = sidebar.folders.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        ["News", "Tech / Rust"],
        "existing folder matched ignoring case"
    );
    assert_eq!(sidebar.folders[0].id, existing_folder);
    assert_eq!(sidebar.folders[0].feeds[0].title, "Daily");
    let rust = &sidebar.folders[1].feeds[0];
    assert_eq!(rust.title, "My Rust", "custom title kept");
    let record = app.feeds.record(rust.id).await.unwrap();
    assert_eq!(record.url, "https://rust.example/rss.xml");
    assert_eq!(record.title, "This Week in Rust");
    assert!(
        record.next_fetch_at.unwrap() <= clock::now_unix(),
        "imported feeds are fetched straight away"
    );

    let exported = opml.export().await.unwrap();
    let again = TestApp::new();
    let summary = OpmlService::new(again.store.clone())
        .import(exported.into_bytes())
        .await
        .unwrap();
    assert_eq!(
        (summary.feeds, summary.folders, summary.duplicates),
        (4, 2, 0)
    );
    let copy = again.articles.sidebar().await.unwrap();
    assert_eq!(copy.folders[1].name, "Tech / Rust");
    assert_eq!(copy.folders[1].feeds[0].title, "My Rust");
    let copied = again
        .feeds
        .record(copy.folders[1].feeds[0].id)
        .await
        .unwrap();
    assert_eq!(copied.title, "This Week in Rust");
    assert_eq!(copied.site_url.as_deref(), Some("https://rust.example/"));

    let broken = opml
        .import(b"<html>nope</html>".to_vec())
        .await
        .unwrap_err();
    assert_eq!(broken.kind, ErrorKind::InvalidInput);
}

#[tokio::test]
async fn display_follows_image_and_privacy_settings() {
    let app = TestApp::new();
    let now = clock::now_unix();
    let html = r#"<p><a href="https://x.example/?utm_medium=feed&amp;page=2" rel="noopener noreferrer">link</a></p><img src="https://img.example/a.png" alt="A">"#;
    let id = app
        .store
        .run(move |conn| {
            let tx = conn.transaction()?;
            let feed = feeds::insert(
                &tx,
                &NewFeed {
                    url: "https://f.example/rss",
                    title: "F",
                    custom_title: None,
                    site_url: None,
                    description: None,
                    folder_id: None,
                    now,
                },
            )?;
            articles::upsert(&tx, feed, &[article("g".into(), now, html)], now, false)?;
            tx.commit()?;
            Ok(feed)
        })
        .await
        .unwrap();
    let article_id = app.articles.list(all(false), None, 1).await.unwrap().items[0].id;
    let _ = id;

    let shown = app.articles.get(article_id).await.unwrap();
    let proxied = proxy_url("https://img.example/a.png");
    assert!(
        shown.content_html.contains(&format!("src=\"{proxied}\"")),
        "{}",
        shown.content_html
    );
    assert!(shown
        .content_html
        .contains("href=\"https://x.example/?page=2\""));
    assert_eq!(shown.url.as_deref(), Some("https://site.example/g?id=1"));
    assert!(!shown.images_blocked);
    let list = app.articles.list(all(false), None, 1).await.unwrap();
    assert_eq!(
        list.items[0]
            .thumbnail
            .as_deref()
            .and_then(|t| decode_proxy_path(t.rsplit('/').next()?)),
        Some("https://img.example/thumb.jpg".to_string())
    );

    app.settings
        .update(Settings {
            load_images: LoadImages::Never,
            strip_tracking_params: false,
            ..Settings::default()
        })
        .await
        .unwrap();
    let blocked = app.articles.get(article_id).await.unwrap();
    assert!(blocked.images_blocked);
    assert!(blocked
        .content_html
        .contains(&format!("data-omarss-src=\"{proxied}\"")));
    assert!(!blocked.content_html.contains(" src="));
    assert!(blocked.content_html.contains("utm_medium=feed"));
    assert_eq!(
        blocked.url.as_deref(),
        Some("https://site.example/g?utm_source=rss&id=1")
    );
    let list = app.articles.list(all(false), None, 1).await.unwrap();
    assert_eq!(
        list.items[0].thumbnail, None,
        "no thumbnails unless images always load"
    );
}

fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::RgbaImage::new(2, 2)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

#[tokio::test]
async fn image_cache_serves_images_offline_and_rejects_non_images() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/a.png"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(png(), "application/octet-stream"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/page"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("<html></html>", "text/html"))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(
        dir.path().join("images"),
        HttpClient::new("test", None).unwrap(),
        500,
    );
    let url = format!("{}/a.png", server.uri());

    let first = cache.load(&url).await.unwrap();
    assert_eq!(first.content_type, "image/png", "sniffed from the bytes");
    // The cache write happens in the background; wait for it.
    for _ in 0..50 {
        if cache.size_bytes() > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let second = cache.load(&url).await.unwrap();
    assert_eq!(
        second, first,
        "served from disk (the mock allows one request)"
    );

    let page = format!("{}/page", server.uri());
    assert_eq!(cache.load(&page).await, None);
    assert_eq!(
        cache.load(&page).await,
        None,
        "failures aren't retried at once"
    );
    assert_eq!(cache.load("https://pixel.wp.com/g.gif").await, None);
    assert_eq!(cache.load("file:///etc/passwd").await, None);
}

#[tokio::test]
async fn cleanup_follows_the_retention_setting() {
    let app = TestApp::new();
    let now = clock::now_unix();
    let feed = seed(&app, "https://old.example/feed", None, 70, DAY, now).await;
    let images = ImageCache::new(
        app._dir.path().join("images"),
        HttpClient::new("test", None).unwrap(),
        500,
    );
    let maintenance = MaintenanceService::new(
        app.store.clone(),
        app.settings.clone(),
        images,
        app._dir.path().to_path_buf(),
    );
    app.settings
        .update(Settings {
            retention_days: 0,
            ..Settings::default()
        })
        .await
        .unwrap();
    assert_eq!(
        maintenance.cleanup().await.unwrap(),
        0,
        "forever keeps everything"
    );

    app.settings
        .update(Settings {
            retention_days: 30,
            ..Settings::default()
        })
        .await
        .unwrap();
    // 70 articles, one a day: keep the newest 50 regardless of age.
    assert_eq!(maintenance.cleanup().await.unwrap(), 20);
    let info = maintenance.storage_info().await.unwrap();
    assert_eq!(info.article_count, 50);
    assert!(info.database_bytes > 0);
    maintenance.compact().await.unwrap();
    assert_eq!(
        app.feeds.record(feed).await.unwrap().url,
        "https://old.example/feed"
    );
}
