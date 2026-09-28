//! Fetching, discovery and refresh against a local mock server (SPEC §13: conditional GET,
//! redirects, 410/429, discovery).

use std::io::Cursor;
use std::time::{Duration, Instant};

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

use super::fixtures::TestApp;
use crate::clock;
use crate::error::ErrorKind;
use crate::models::{ArticleQuery, SortOrder, SubscribeRequest, View};
use crate::services::settings::Settings;
use crate::store::feeds::{self, NewFeed};

fn rss(title: &str, items: &[(&str, &str)]) -> String {
    let items: String = items
        .iter()
        .map(|(guid, body)| {
            format!(
                "<item><title>Item {guid}</title><guid>{guid}</guid><link>https://site.example/{guid}</link>\
                 <description>{body}</description><pubDate>Mon, 02 Sep 2024 10:00:00 GMT</pubDate></item>"
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><title>{title}</title>\
         <link>https://site.example/</link><description>d</description>{items}</channel></rss>"
    )
}

fn feed_response(body: String) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body, "application/rss+xml")
}

async fn mount(server: &MockServer, at: &str, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path(at))
        .respond_with(response)
        .mount(server)
        .await;
}

async fn subscribe(app: &TestApp, url: String) -> i64 {
    app.feeds
        .subscribe(SubscribeRequest {
            url,
            folder_id: None,
            title: None,
        })
        .await
        .expect("subscribe")
}

fn view(id: i64) -> ArticleQuery {
    ArticleQuery {
        view: View::Feed { id },
        unread_only: false,
        sort: SortOrder::NewestFirst,
    }
}

async fn titles(app: &TestApp, id: i64) -> Vec<String> {
    let page = app.articles.list(view(id), None, 100).await.unwrap();
    page.items.into_iter().map(|i| i.title).collect()
}

#[tokio::test]
async fn subscribing_stores_the_feed_and_its_articles() {
    let server = MockServer::start().await;
    mount(
        &server,
        "/feed.xml",
        feed_response(rss("Mock Feed", &[("a", "first"), ("b", "second")]))
            .insert_header("ETag", "\"v1\"")
            .insert_header("Last-Modified", "Mon, 02 Sep 2024 10:00:00 GMT"),
    )
    .await;
    let app = TestApp::new();
    let before = clock::now_unix();
    let id = subscribe(&app, format!("{}/feed.xml", server.uri())).await;

    let feed = app.feeds.record(id).await.unwrap();
    assert_eq!(feed.title, "Mock Feed");
    assert_eq!(feed.etag.as_deref(), Some("\"v1\""));
    assert_eq!(feed.site_url.as_deref(), Some("https://site.example/"));
    assert!(
        feed.next_fetch_at.unwrap() >= before + 1800,
        "default 30-minute interval"
    );
    assert_eq!(titles(&app, id).await.len(), 2);

    let again = app
        .feeds
        .subscribe(SubscribeRequest {
            url: format!("{}/feed.xml", server.uri()),
            folder_id: None,
            title: None,
        })
        .await
        .unwrap_err();
    assert_eq!(again.kind, ErrorKind::Conflict);
}

#[tokio::test]
async fn conditional_get_skips_unchanged_feeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/feed.xml"))
        .and(header("if-none-match", "\"v1\""))
        .respond_with(ResponseTemplate::new(304))
        .with_priority(1)
        .mount(&server)
        .await;
    mount(
        &server,
        "/feed.xml",
        feed_response(rss("F", &[("a", "x")])).insert_header("ETag", "\"v1\""),
    )
    .await;
    let app = TestApp::new();
    let id = subscribe(&app, format!("{}/feed.xml", server.uri())).await;

    let outcome = app
        .feeds
        .refresh_feed(id, &Settings::default())
        .await
        .unwrap();
    assert_eq!(outcome.new_articles, 0);
    assert_eq!(outcome.error, None);
    let requests = server.received_requests().await.unwrap();
    let conditional: Vec<&Request> = requests
        .iter()
        .filter(|r| r.url.path() == "/feed.xml" && r.headers.get("if-none-match").is_some())
        .collect();
    assert_eq!(conditional.len(), 1, "the refresh sent If-None-Match");
    assert_eq!(app.feeds.record(id).await.unwrap().error_count, 0);
}

#[tokio::test]
async fn refresh_adds_new_items_and_updates_changed_ones_keeping_state() {
    let server = MockServer::start().await;
    mount(
        &server,
        "/feed.xml",
        feed_response(rss("F", &[("a", "v1"), ("b", "v1")])),
    )
    .await;
    let app = TestApp::new();
    let id = subscribe(&app, format!("{}/feed.xml", server.uri())).await;
    let page = app.articles.list(view(id), None, 10).await.unwrap();
    let a = page.items.iter().find(|i| i.title == "Item a").unwrap().id;
    app.articles.set_read(vec![a], true).await.unwrap();
    app.articles.set_starred(a, true).await.unwrap();

    server.reset().await;
    mount(
        &server,
        "/feed.xml",
        feed_response(rss("F", &[("a", "v2 changed"), ("b", "v1"), ("c", "new")])),
    )
    .await;
    let outcome = app
        .feeds
        .refresh_feed(id, &Settings::default())
        .await
        .unwrap();
    assert_eq!((outcome.new_articles, outcome.updated_articles), (1, 1));
    let article = app.articles.get(a).await.unwrap();
    assert!(article.content_html.contains("v2 changed"));
    assert!(
        article.is_read && article.is_starred,
        "read/star state kept"
    );

    // With "mark updated articles unread" on, a change makes it unread again.
    server.reset().await;
    mount(
        &server,
        "/feed.xml",
        feed_response(rss("F", &[("a", "v3")])),
    )
    .await;
    let settings = Settings {
        mark_updated_unread: true,
        ..Settings::default()
    };
    app.feeds.refresh_feed(id, &settings).await.unwrap();
    assert!(!app.articles.get(a).await.unwrap().is_read);
}

#[tokio::test]
async fn permanent_redirects_update_the_url_and_temporary_ones_do_not() {
    let server = MockServer::start().await;
    mount(&server, "/a", feed_response(rss("F", &[("a", "x")]))).await;
    let app = TestApp::new();
    let id = subscribe(&app, format!("{}/a", server.uri())).await;

    server.reset().await;
    mount(
        &server,
        "/a",
        ResponseTemplate::new(302).insert_header("Location", "/b"),
    )
    .await;
    mount(&server, "/b", feed_response(rss("F", &[("a", "x")]))).await;
    let outcome = app
        .feeds
        .refresh_feed(id, &Settings::default())
        .await
        .unwrap();
    assert_eq!(outcome.error, None);
    assert!(
        app.feeds.record(id).await.unwrap().url.ends_with("/a"),
        "302 keeps the URL"
    );

    server.reset().await;
    mount(
        &server,
        "/a",
        ResponseTemplate::new(301).insert_header("Location", "/c"),
    )
    .await;
    mount(
        &server,
        "/c",
        ResponseTemplate::new(308).insert_header("Location", "/d"),
    )
    .await;
    mount(&server, "/d", feed_response(rss("F", &[("a", "x")]))).await;
    app.feeds
        .refresh_feed(id, &Settings::default())
        .await
        .unwrap();
    assert!(
        app.feeds.record(id).await.unwrap().url.ends_with("/d"),
        "301/308 update it"
    );

    server.reset().await;
    mount(
        &server,
        "/d",
        ResponseTemplate::new(301).insert_header("Location", "/e"),
    )
    .await;
    mount(
        &server,
        "/e",
        ResponseTemplate::new(307).insert_header("Location", "/f"),
    )
    .await;
    mount(&server, "/f", feed_response(rss("F", &[("a", "x")]))).await;
    app.feeds
        .refresh_feed(id, &Settings::default())
        .await
        .unwrap();
    assert!(
        app.feeds.record(id).await.unwrap().url.ends_with("/e"),
        "permanent hops count until the first temporary one"
    );
}

#[tokio::test]
async fn gone_feeds_are_paused() {
    let server = MockServer::start().await;
    mount(&server, "/feed.xml", feed_response(rss("F", &[("a", "x")]))).await;
    let app = TestApp::new();
    let id = subscribe(&app, format!("{}/feed.xml", server.uri())).await;

    server.reset().await;
    mount(&server, "/feed.xml", ResponseTemplate::new(410)).await;
    let outcome = app
        .feeds
        .refresh_feed(id, &Settings::default())
        .await
        .unwrap();
    assert!(outcome.error.unwrap().message.contains("410"));
    let feed = app.feeds.record(id).await.unwrap();
    assert!(feed.paused);
    assert_eq!(feed.error_count, 1);
    assert_eq!(feed.next_fetch_at, None);
}

#[tokio::test]
async fn retry_after_and_backoff_delay_the_next_fetch() {
    let server = MockServer::start().await;
    mount(&server, "/feed.xml", feed_response(rss("F", &[("a", "x")]))).await;
    let app = TestApp::new();
    let id = subscribe(&app, format!("{}/feed.xml", server.uri())).await;

    server.reset().await;
    mount(
        &server,
        "/feed.xml",
        ResponseTemplate::new(429).insert_header("Retry-After", "7200"),
    )
    .await;
    let now = clock::now_unix();
    app.feeds
        .refresh_feed(id, &Settings::default())
        .await
        .unwrap();
    let feed = app.feeds.record(id).await.unwrap();
    assert_eq!(feed.error_count, 1);
    assert!(!feed.paused);
    assert!(
        feed.next_fetch_at.unwrap() >= now + 7200,
        "honours Retry-After"
    );

    server.reset().await;
    mount(&server, "/feed.xml", ResponseTemplate::new(500)).await;
    let now = clock::now_unix();
    app.feeds
        .refresh_feed(id, &Settings::default())
        .await
        .unwrap();
    let feed = app.feeds.record(id).await.unwrap();
    assert_eq!(feed.error_count, 2);
    assert!(feed.last_error.unwrap().contains("500"));
    let delay = feed.next_fetch_at.unwrap() - now;
    assert!(
        (7200..=7210).contains(&delay),
        "30 min × 2² = 2 h, got {delay}"
    );

    // Success resets the error count.
    server.reset().await;
    mount(&server, "/feed.xml", feed_response(rss("F", &[("a", "x")]))).await;
    app.feeds
        .refresh_feed(id, &Settings::default())
        .await
        .unwrap();
    let feed = app.feeds.record(id).await.unwrap();
    assert_eq!((feed.error_count, feed.last_error), (0, None));
}

#[tokio::test]
async fn documents_that_are_not_feeds_are_errors() {
    let server = MockServer::start().await;
    mount(&server, "/feed.xml", feed_response(rss("F", &[("a", "x")]))).await;
    let app = TestApp::new();
    let id = subscribe(&app, format!("{}/feed.xml", server.uri())).await;
    server.reset().await;
    mount(
        &server,
        "/feed.xml",
        ResponseTemplate::new(200)
            .set_body_raw("<html><body>maintenance</body></html>", "text/html"),
    )
    .await;
    let outcome = app
        .feeds
        .refresh_feed(id, &Settings::default())
        .await
        .unwrap();
    assert!(outcome.error.unwrap().message.contains("Not a valid feed"));

    server.reset().await;
    mount(
        &server,
        "/feed.xml",
        ResponseTemplate::new(302).insert_header("Location", "/feed.xml"),
    )
    .await;
    let outcome = app
        .feeds
        .refresh_feed(id, &Settings::default())
        .await
        .unwrap();
    assert!(outcome
        .error
        .unwrap()
        .message
        .contains("Too many redirects"));
}

#[tokio::test]
async fn discovery_finds_linked_feeds_then_common_paths() {
    let server = MockServer::start().await;
    let page = |links: &str| {
        ResponseTemplate::new(200).set_body_raw(
            format!(
                "<!doctype html><html><head><title>Site</title>{links}</head><body></body></html>"
            ),
            "text/html; charset=utf-8",
        )
    };
    mount(
        &server,
        "/linked/",
        page(r#"<link rel="alternate" type="application/rss+xml" title="Posts" href="/feed.xml"><link rel="alternate" type="application/atom+xml" href="comments.atom">"#),
    )
    .await;
    mount(&server, "/plain/", page("")).await;
    mount(
        &server,
        "/rss.xml",
        feed_response(rss("Found by path", &[("a", "x")])),
    )
    .await;
    mount(
        &server,
        "/feed.xml",
        feed_response(rss("Direct", &[("a", "x")])),
    )
    .await;
    let app = TestApp::new();

    let linked = app
        .feeds
        .discover(&format!("{}/linked/", server.uri()))
        .await
        .unwrap();
    let urls: Vec<&str> = linked.iter().map(|f| f.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            format!("{}/feed.xml", server.uri()),
            format!("{}/linked/comments.atom", server.uri())
        ]
    );
    assert_eq!(linked[0].title.as_deref(), Some("Posts"));

    let by_path = app
        .feeds
        .discover(&format!("{}/plain/", server.uri()))
        .await
        .unwrap();
    assert_eq!(by_path.len(), 2, "/feed and /rss.xml exist: {by_path:?}");
    assert!(by_path
        .iter()
        .any(|f| f.title.as_deref() == Some("Found by path")));

    let direct = app
        .feeds
        .discover(&format!("{}/feed.xml", server.uri()))
        .await
        .unwrap();
    assert_eq!(direct.len(), 1);
    assert_eq!(direct[0].title.as_deref(), Some("Direct"));

    server.reset().await;
    mount(&server, "/plain/", page("")).await;
    let err = app
        .feeds
        .discover(&format!("{}/plain/", server.uri()))
        .await
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidFeed);
}

#[tokio::test]
async fn discovery_ignores_wordpress_rest_links_but_keeps_json_feeds() {
    let server = MockServer::start().await;
    let page = |links: &str| {
        ResponseTemplate::new(200).set_body_raw(
            format!("<!doctype html><html><head>{links}</head><body></body></html>"),
            "text/html; charset=utf-8",
        )
    };
    // A WordPress page: its RSS feed plus the REST API link every WP page carries.
    mount(
        &server,
        "/about/",
        page(
            r#"<link rel="alternate" type="application/rss+xml" title="Blog" href="/feed/">
               <link rel="alternate" type="application/json" href="/wp-json/wp/v2/pages/42">
               <link rel="https://api.w.org/" href="/wp-json/">"#,
        ),
    )
    .await;
    mount(
        &server,
        "/wp-json/wp/v2/pages/42",
        ResponseTemplate::new(200).set_body_raw(
            r#"{"id":42,"slug":"about","title":{"rendered":"About"}}"#,
            "application/json",
        ),
    )
    .await;
    // A JSON Feed 1.0 advertised as plain application/json is still found.
    mount(
        &server,
        "/json-site/",
        page(r#"<link rel="alternate" type="application/json" href="/feed.json">"#),
    )
    .await;
    mount(
        &server,
        "/feed.json",
        ResponseTemplate::new(200).set_body_raw(
            r#"{"version":"https://jsonfeed.org/version/1","title":"JSON Blog",
                "items":[{"id":"1","content_text":"hi"}]}"#,
            "application/json",
        ),
    )
    .await;
    let app = TestApp::new();

    let found = app
        .feeds
        .discover(&format!("{}/about/", server.uri()))
        .await
        .unwrap();
    let urls: Vec<&str> = found.iter().map(|f| f.url.as_str()).collect();
    assert_eq!(urls, [format!("{}/feed/", server.uri())]);
    assert_eq!(found[0].title.as_deref(), Some("Blog"));

    let json = app
        .feeds
        .discover(&format!("{}/json-site/", server.uri()))
        .await
        .unwrap();
    assert_eq!(json.len(), 1);
    assert_eq!(json[0].url, format!("{}/feed.json", server.uri()));
    assert_eq!(json[0].title.as_deref(), Some("JSON Blog"));
}

#[tokio::test]
async fn preview_shows_the_five_newest_items() {
    let server = MockServer::start().await;
    let items: String = (1..=8)
        .map(|n| {
            format!(
                "<item><title>Post {n}</title><guid>{n}</guid><pubDate>0{n} Sep 2024 10:00:00 GMT</pubDate></item>"
            )
        })
        .collect();
    let body = format!(
        "<rss version=\"2.0\"><channel><title>Eight</title><link>https://site.example/</link>{items}</channel></rss>"
    );
    mount(&server, "/feed.xml", feed_response(body)).await;
    let app = TestApp::new();
    let preview = app
        .feeds
        .preview(&format!("{}/feed.xml", server.uri()))
        .await
        .unwrap();
    assert_eq!(preview.title, "Eight");
    let titles: Vec<&str> = preview.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(titles, ["Post 8", "Post 7", "Post 6", "Post 5", "Post 4"]);
}

#[tokio::test]
async fn favicons_are_fetched_resized_and_stored() {
    let server = MockServer::start().await;
    let body = format!(
        "<rss version=\"2.0\"><channel><title>Iconic</title><link>{0}/</link>\
         <image><url>{0}/logo.png</url></image><item><title>t</title><guid>g</guid></item></channel></rss>",
        server.uri()
    );
    mount(&server, "/feed.xml", feed_response(body)).await;
    let logo = image::RgbaImage::from_pixel(64, 64, image::Rgba([10, 120, 200, 255]));
    let mut png = Vec::new();
    logo.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    mount(
        &server,
        "/logo.png",
        ResponseTemplate::new(200).set_body_raw(png, "image/png"),
    )
    .await;

    let app = TestApp::new();
    let id = subscribe(&app, format!("{}/feed.xml", server.uri())).await;
    let mut icon = None;
    for _ in 0..50 {
        icon = app.feeds.record(id).await.unwrap().icon_path;
        if icon.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let icon = icon.expect("icon stored in the background");
    assert!(
        icon.starts_with(&format!("{id}-")) && icon.ends_with(".png"),
        "{icon}"
    );
    let stored = image::open(app.icons_dir().join(&icon)).unwrap();
    assert_eq!((stored.width(), stored.height()), (32, 32));
}

#[tokio::test]
async fn refreshing_does_not_wait_for_the_favicon() {
    let server = MockServer::start().await;
    let body = format!(
        "<rss version=\"2.0\"><channel><title>Slow icon</title><link>{0}/</link>\
         <image><url>{0}/logo.png</url></image><item><title>t</title><guid>g</guid></item></channel></rss>",
        server.uri()
    );
    mount(&server, "/feed.xml", feed_response(body)).await;
    let logo = image::RgbaImage::from_pixel(16, 16, image::Rgba([10, 120, 200, 255]));
    let mut png = Vec::new();
    logo.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let icon_delay = Duration::from_millis(1500);
    mount(
        &server,
        "/logo.png",
        ResponseTemplate::new(200)
            .set_body_raw(png, "image/png")
            .set_delay(icon_delay),
    )
    .await;

    // Added straight to the database, so no icon fetch is already under way.
    let app = TestApp::new();
    let url = format!("{}/feed.xml", server.uri());
    let id = app
        .store
        .run(move |conn| {
            let tx = conn.transaction()?;
            let id = feeds::insert(
                &tx,
                &NewFeed {
                    url: &url,
                    title: "t",
                    custom_title: None,
                    site_url: None,
                    description: None,
                    folder_id: None,
                    now: 0,
                },
            )?;
            tx.commit()?;
            Ok(id)
        })
        .await
        .unwrap();

    let started = Instant::now();
    let outcome = app
        .feeds
        .refresh_feed(id, &Settings::default())
        .await
        .unwrap();
    assert!(outcome.error.is_none());
    assert_eq!(outcome.new_articles, 1);
    assert!(
        started.elapsed() < icon_delay,
        "the refresh returned without waiting for the icon ({:?})",
        started.elapsed()
    );
    assert_eq!(app.feeds.record(id).await.unwrap().icon_path, None);

    let mut icon = None;
    for _ in 0..100 {
        icon = app.feeds.record(id).await.unwrap().icon_path;
        if icon.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(icon.is_some(), "the icon is still stored in the background");
}
