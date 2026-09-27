//! Refresh batches: offline detection and per-host concurrency (SPEC §7.2).

use std::net::TcpListener;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::fixtures::TestApp;
use crate::clock;
use crate::events::{ArticlesChanged, FeedError, RefreshDone, RefreshProgress};
use crate::models::{RefreshStatus, RefreshTarget};
use crate::scheduler::{run_batch, OFFLINE_RETRY};
use crate::services::settings::Settings;
use crate::store::feeds::{self, NewFeed};

fn mock_app() -> tauri::App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    tauri_specta::Builder::<tauri::test::MockRuntime>::new()
        .events(tauri_specta::collect_events![
            RefreshProgress,
            RefreshDone,
            ArticlesChanged,
            FeedError
        ])
        .mount_events(&app);
    app
}

/// A port nothing listens on.
fn closed_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn add_feed(app: &TestApp, url: String) -> i64 {
    app.store
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
        .unwrap()
}

#[tokio::test]
async fn unreachable_everywhere_means_offline_not_broken_feeds() {
    let app = TestApp::new();
    let port = closed_port();
    let a = add_feed(&app, format!("http://127.0.0.1:{port}/a")).await;
    let b = add_feed(&app, format!("http://localhost:{port}/b")).await;
    let tauri_app = mock_app();
    let status = Mutex::new(RefreshStatus::default());
    let before = clock::now_unix();

    let jobs = app.feeds.jobs_for(&[RefreshTarget::All]).await.unwrap();
    run_batch(
        tauri_app.handle(),
        &app.feeds,
        &Settings::default(),
        &status,
        jobs,
    )
    .await;

    assert!(status.lock().unwrap().offline);
    for id in [a, b] {
        let feed = app.feeds.record(id).await.unwrap();
        assert_eq!(feed.error_count, 0, "not counted as a feed error");
        let next = feed.next_fetch_at.unwrap();
        assert!((before + OFFLINE_RETRY..=before + OFFLINE_RETRY + 5).contains(&next));
    }
}

#[tokio::test]
async fn one_unreachable_host_is_a_feed_error() {
    let app = TestApp::new();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            "<rss version=\"2.0\"><channel><title>ok</title><item><title>i</title><guid>g</guid></item></channel></rss>",
            "application/rss+xml",
        ))
        .mount(&server)
        .await;
    let up = add_feed(&app, format!("{}/up", server.uri())).await;
    let down = add_feed(&app, format!("http://localhost:{}/down", closed_port())).await;
    let tauri_app = mock_app();
    let status = Mutex::new(RefreshStatus::default());

    let jobs = app.feeds.jobs_for(&[RefreshTarget::All]).await.unwrap();
    run_batch(
        tauri_app.handle(),
        &app.feeds,
        &Settings::default(),
        &status,
        jobs,
    )
    .await;

    let status = status.lock().unwrap().clone();
    assert!(!status.offline && !status.running);
    assert_eq!((status.done, status.total), (2, 2));
    assert_eq!(app.feeds.record(up).await.unwrap().error_count, 0);
    let broken = app.feeds.record(down).await.unwrap();
    assert_eq!(broken.error_count, 1);
    assert!(broken.last_error.unwrap().contains("Couldn't connect"));
}

#[tokio::test]
async fn at_most_two_requests_per_host_at_once() {
    let app = TestApp::new();
    let server = MockServer::start().await;
    let delay = Duration::from_millis(300);
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(
                    "<rss version=\"2.0\"><channel><title>t</title></channel></rss>",
                    "application/rss+xml",
                )
                .set_delay(delay),
        )
        .mount(&server)
        .await;
    for n in 0..4 {
        add_feed(&app, format!("{}/{n}", server.uri())).await;
    }
    let tauri_app = mock_app();
    let status = Mutex::new(RefreshStatus::default());
    let jobs = app.feeds.jobs_for(&[RefreshTarget::All]).await.unwrap();
    assert_eq!(jobs.len(), 4);

    let started = Instant::now();
    run_batch(
        tauri_app.handle(),
        &app.feeds,
        &Settings::default(),
        &status,
        jobs,
    )
    .await;
    assert!(
        started.elapsed() >= delay * 2,
        "4 requests to one host need ≥ 2 rounds"
    );
}

#[tokio::test]
async fn refresh_requests_cover_each_feed_once() {
    let app = TestApp::new();
    let folder = app.feeds.create_folder("F".into()).await.unwrap();
    let a = add_feed(&app, "https://a.example/feed".into()).await;
    let b = add_feed(&app, "https://b.example/feed".into()).await;
    app.feeds
        .reorder(crate::models::SidebarOrder {
            folders: vec![folder],
            feeds: vec![
                crate::models::FeedPlacement {
                    id: a,
                    folder_id: Some(folder),
                },
                crate::models::FeedPlacement {
                    id: b,
                    folder_id: None,
                },
            ],
        })
        .await
        .unwrap();
    let ids = |jobs: Vec<(i64, String)>| jobs.into_iter().map(|(id, _)| id).collect::<Vec<_>>();
    assert_eq!(
        ids(app
            .feeds
            .jobs_for(&[RefreshTarget::Folder { id: folder }])
            .await
            .unwrap()),
        [a]
    );
    assert_eq!(
        ids(app
            .feeds
            .jobs_for(&[
                RefreshTarget::All,
                RefreshTarget::Feed { id: a },
                RefreshTarget::Folder { id: folder }
            ])
            .await
            .unwrap()),
        [a, b]
    );
}

#[tokio::test]
async fn a_refresh_with_nothing_to_fetch_still_finishes() {
    let app = TestApp::new();
    let tauri_app = mock_app();
    let settings = crate::services::settings::SettingsService::new(app.store.clone());
    let scheduler =
        crate::scheduler::Scheduler::start(tauri_app.handle().clone(), app.feeds.clone(), settings);
    scheduler.request(RefreshTarget::All);
    let mut status = scheduler.status();
    for _ in 0..40 {
        status = scheduler.status();
        if status.last_finished_at.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        status.last_finished_at.is_some(),
        "the UI is told the refresh finished"
    );
    assert!(!status.running);
}
