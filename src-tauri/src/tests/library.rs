//! The reading side and subscription management, against a real (temporary) database.

use std::collections::HashSet;

use super::fixtures::{feed_fixture, TestApp};
use crate::clock;
use crate::error::ErrorKind;
use crate::feed;
use crate::models::{
    ArticleCursor, ArticleQuery, Enclosure, FeedPlacement, FeedUpdate, SidebarOrder, SortOrder,
    View,
};
use crate::store::articles::{self, NewArticle};
use crate::store::feeds::{self, NewFeed};

fn article(guid: &str, published_at: i64, body: &str) -> NewArticle {
    NewArticle {
        guid: guid.to_string(),
        url: Some(format!("https://site.example/{guid}")),
        title: format!("Article {guid}"),
        author: None,
        summary_html: Some(format!("<p>{body}</p>")),
        content_html: None,
        published_at,
        updated_at: None,
        content_hash: crate::content::content_hash(guid, Some(body), None),
        enclosures: Vec::new(),
    }
}

/// Inserts a feed with `count` articles, one hour apart, newest at `newest`.
async fn seed(app: &TestApp, url: &str, folder_id: Option<i64>, count: usize, newest: i64) -> i64 {
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
                .map(|i| article(&format!("{id}-{i}"), newest - i as i64 * 3600, "body"))
                .collect();
            articles::upsert(&tx, id, &items, newest, false)?;
            tx.commit()?;
            Ok(id)
        })
        .await
        .unwrap()
}

fn query(view: View, unread_only: bool) -> ArticleQuery {
    ArticleQuery {
        view,
        unread_only,
        sort: SortOrder::NewestFirst,
    }
}

async fn all_pages(app: &TestApp, q: ArticleQuery, page_size: u32) -> Vec<(i64, i64)> {
    let mut seen = Vec::new();
    let mut after: Option<ArticleCursor> = None;
    loop {
        let page = app
            .articles
            .list(q.clone(), after, page_size)
            .await
            .unwrap();
        assert!(page.items.len() <= page_size as usize);
        seen.extend(page.items.iter().map(|i| (i.published_at.unwrap(), i.id)));
        match page.next {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    seen
}

#[tokio::test]
async fn keyset_pagination_covers_every_article_once_in_order() {
    let app = TestApp::new();
    let now = clock::now_unix();
    seed(&app, "https://a.example/feed", None, 130, now).await;
    seed(&app, "https://b.example/feed", None, 120, now - 1800).await;

    let newest = all_pages(&app, query(View::All, false), 100).await;
    assert_eq!(newest.len(), 250);
    assert_eq!(
        newest
            .iter()
            .map(|(_, id)| id)
            .collect::<HashSet<_>>()
            .len(),
        250
    );
    assert!(
        newest.windows(2).all(|w| w[0] > w[1]),
        "strictly newest first"
    );

    let mut q = query(View::All, false);
    q.sort = SortOrder::OldestFirst;
    let oldest = all_pages(&app, q, 64).await;
    assert!(oldest.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(oldest.len(), 250);
}

#[tokio::test]
async fn views_filters_and_counts() {
    let app = TestApp::new();
    let now = clock::now_unix();
    let folder = app.feeds.create_folder("Tech".into()).await.unwrap();
    let a = seed(&app, "https://a.example/feed", Some(folder), 5, now).await;
    let b = seed(&app, "https://b.example/feed", None, 3, now - 3 * 86_400).await;

    let a_items = app
        .articles
        .list(query(View::Feed { id: a }, false), None, 50)
        .await
        .unwrap()
        .items;
    app.articles
        .set_read(vec![a_items[0].id, a_items[1].id], true)
        .await
        .unwrap();
    app.articles.set_starred(a_items[0].id, true).await.unwrap();

    let len = |v: View, unread: bool| {
        let app = &app;
        async move {
            app.articles
                .list(query(v, unread), None, 100)
                .await
                .unwrap()
                .items
                .len()
        }
    };
    assert_eq!(len(View::All, false).await, 8);
    assert_eq!(len(View::All, true).await, 6);
    assert_eq!(len(View::Unread, false).await, 6);
    assert_eq!(
        len(View::Starred, true).await,
        1,
        "starred ignores the unread filter"
    );
    assert_eq!(len(View::Folder { id: folder }, true).await, 3);
    assert_eq!(len(View::Feed { id: b }, false).await, 3);
    assert!(
        len(View::Today, false).await <= 5,
        "b's articles are days old"
    );

    let sidebar = app.articles.sidebar().await.unwrap();
    assert_eq!(sidebar.counts.unread, 6);
    assert_eq!(sidebar.counts.starred, 1);
    assert_eq!(sidebar.folders.len(), 1);
    assert_eq!(sidebar.folders[0].unread_count, 3);
    assert_eq!(sidebar.folders[0].feeds[0].unread_count, 3);
    assert_eq!(sidebar.feeds.len(), 1, "b is at the top level");
    assert_eq!(sidebar.feeds[0].unread_count, 3);

    assert_eq!(
        app.articles
            .set_read(vec![a_items[0].id], true)
            .await
            .unwrap(),
        0,
        "already read"
    );
    app.articles
        .set_read(vec![a_items[0].id], false)
        .await
        .unwrap();
    assert!(!app.articles.get(a_items[0].id).await.unwrap().is_read);
    assert_eq!(
        app.articles.get(-1).await.unwrap_err().kind,
        ErrorKind::NotFound
    );
}

#[tokio::test]
async fn upsert_deduplicates_on_guid_and_only_rewrites_changed_articles() {
    let app = TestApp::new();
    let now = clock::now_unix();
    let id = seed(&app, "https://a.example/feed", None, 0, now).await;
    let run = |items: Vec<NewArticle>, mark_unread: bool| {
        let store = app.store.clone();
        async move {
            store
                .run(move |conn| {
                    let tx = conn.transaction()?;
                    let stats = articles::upsert(&tx, id, &items, now, mark_unread)?;
                    tx.commit()?;
                    Ok(stats)
                })
                .await
                .unwrap()
        }
    };
    let stats = run(
        vec![article("x", now, "one"), article("y", now, "one")],
        false,
    )
    .await;
    assert_eq!((stats.inserted, stats.updated), (2, 0));
    let stats = run(
        vec![article("x", now, "one"), article("y", now, "one")],
        false,
    )
    .await;
    assert_eq!((stats.inserted, stats.updated), (0, 0), "unchanged");
    let mut changed = article("x", now, "two");
    changed.enclosures = vec![Enclosure {
        url: "https://site.example/x.mp3".into(),
        mime_type: Some("audio/mpeg".into()),
        length: Some(10),
    }];
    let stats = run(vec![changed], false).await;
    assert_eq!((stats.inserted, stats.updated), (0, 1));

    let x = app
        .articles
        .list(query(View::Feed { id }, false), None, 10)
        .await
        .unwrap()
        .items;
    let x = x.iter().find(|i| i.title == "Article x").unwrap();
    let full = app.articles.get(x.id).await.unwrap();
    assert!(
        full.content_html.contains("two"),
        "falls back to the summary: {}",
        full.content_html
    );
    assert_eq!(full.enclosures.len(), 1);

    // A feed repeating a GUID within one document yields one article.
    let parsed = feed::parse(
        &feed_fixture("duplicate_guids.xml"),
        None,
        "https://dupes.example/feed",
    )
    .unwrap();
    assert_eq!(parsed.items.len(), 3);
    let dupes = seed(&app, "https://dupes.example/feed", None, 0, now).await;
    let items: Vec<NewArticle> = parsed
        .items
        .iter()
        .map(|i| {
            let mut a = article(&i.guid, now, &i.title);
            a.title = i.title.clone();
            a
        })
        .collect();
    let store = app.store.clone();
    store
        .run(move |conn| {
            let tx = conn.transaction()?;
            articles::upsert(&tx, dupes, &items, now, false)?;
            tx.commit()?;
            Ok(())
        })
        .await
        .unwrap();
    let stored = app
        .articles
        .list(query(View::Feed { id: dupes }, false), None, 10)
        .await
        .unwrap();
    assert_eq!(stored.items.len(), 2);
}

#[tokio::test]
async fn unsubscribing_keeps_starred_articles_under_deleted_feeds() {
    let app = TestApp::new();
    let now = clock::now_unix();
    let a = seed(&app, "https://a.example/feed", None, 3, now).await;
    let b = seed(&app, "https://b.example/feed", None, 2, now).await;
    for feed in [a, b] {
        let items = app
            .articles
            .list(query(View::Feed { id: feed }, false), None, 10)
            .await
            .unwrap()
            .items;
        app.articles.set_starred(items[0].id, true).await.unwrap();
    }

    app.feeds.unsubscribe(a).await.unwrap();
    let sidebar = app.articles.sidebar().await.unwrap();
    assert_eq!(sidebar.feeds.len(), 1, "a is gone, b remains");
    let deleted = sidebar.deleted_feeds.expect("pseudo-feed appears");
    assert_eq!(deleted.title, "Deleted feeds");
    let kept = app
        .articles
        .list(query(View::Feed { id: deleted.id }, false), None, 10)
        .await
        .unwrap();
    assert_eq!(kept.items.len(), 1);
    assert!(kept.items[0].is_starred);
    assert_eq!(app.articles.sidebar().await.unwrap().counts.starred, 2);

    app.feeds.unsubscribe(b).await.unwrap();
    let kept = app
        .articles
        .list(query(View::Feed { id: deleted.id }, false), None, 10)
        .await
        .unwrap();
    assert_eq!(kept.items.len(), 2);
    let err = app.feeds.unsubscribe(deleted.id).await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidInput);
    assert!(app
        .feeds
        .jobs_for(&[crate::models::RefreshTarget::All])
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn folders_rename_delete_and_reorder() {
    let app = TestApp::new();
    let now = clock::now_unix();
    let news = app.feeds.create_folder("News".into()).await.unwrap();
    let tech = app.feeds.create_folder("Tech".into()).await.unwrap();
    assert_eq!(
        app.feeds
            .create_folder(" news ".into())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    assert_eq!(
        app.feeds.create_folder("  ".into()).await.unwrap_err().kind,
        ErrorKind::InvalidInput
    );
    app.feeds.rename_folder(news, "World".into()).await.unwrap();

    let a = seed(&app, "https://a.example/feed", Some(news), 1, now).await;
    let b = seed(&app, "https://b.example/feed", Some(news), 1, now).await;
    let c = seed(&app, "https://c.example/feed", None, 1, now).await;

    app.feeds
        .reorder(SidebarOrder {
            folders: vec![tech, news],
            feeds: vec![
                FeedPlacement {
                    id: c,
                    folder_id: Some(tech),
                },
                FeedPlacement {
                    id: b,
                    folder_id: Some(news),
                },
                FeedPlacement {
                    id: a,
                    folder_id: None,
                },
            ],
        })
        .await
        .unwrap();
    let sidebar = app.articles.sidebar().await.unwrap();
    let names: Vec<&str> = sidebar.folders.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Tech", "World"]);
    assert_eq!(sidebar.folders[0].feeds[0].id, c);
    assert_eq!(sidebar.folders[1].feeds[0].id, b);
    assert_eq!(sidebar.feeds[0].id, a);

    app.feeds.delete_folder(news).await.unwrap();
    let sidebar = app.articles.sidebar().await.unwrap();
    assert_eq!(sidebar.folders.len(), 1);
    let root: HashSet<i64> = sidebar.feeds.iter().map(|f| f.id).collect();
    assert_eq!(
        root,
        HashSet::from([a, b]),
        "the folder's feeds moved to the top level"
    );
}

#[tokio::test]
async fn editing_a_feed() {
    let app = TestApp::new();
    let now = clock::now_unix();
    let folder = app.feeds.create_folder("Blogs".into()).await.unwrap();
    let id = seed(&app, "https://a.example/feed", None, 1, now).await;

    let details = app
        .feeds
        .update(
            id,
            FeedUpdate {
                custom_title: Some("  My name  ".into()),
                folder_id: Some(folder),
                fetch_interval: Some(3600),
                paused: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(details.custom_title.as_deref(), Some("My name"));
    assert_eq!(details.folder_id, Some(folder));
    let sidebar = app.articles.sidebar().await.unwrap();
    assert_eq!(sidebar.folders[0].feeds[0].title, "My name");
    let list = app
        .articles
        .list(query(View::All, false), None, 5)
        .await
        .unwrap();
    assert_eq!(list.items[0].feed_title, "My name");

    let manual = app
        .feeds
        .update(
            id,
            FeedUpdate {
                custom_title: None,
                folder_id: None,
                fetch_interval: Some(0),
                paused: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        (manual.custom_title, manual.folder_id, manual.paused),
        (None, None, true)
    );
    assert_eq!(
        app.feeds.record(id).await.unwrap().next_fetch_at,
        None,
        "manual only"
    );

    let too_fast = FeedUpdate {
        custom_title: None,
        folder_id: None,
        fetch_interval: Some(60),
        paused: false,
    };
    assert_eq!(
        app.feeds.update(id, too_fast).await.unwrap_err().kind,
        ErrorKind::InvalidInput
    );
    let bad_folder = FeedUpdate {
        custom_title: None,
        folder_id: Some(999),
        fetch_interval: None,
        paused: false,
    };
    assert_eq!(
        app.feeds.update(id, bad_folder).await.unwrap_err().kind,
        ErrorKind::InvalidInput
    );
}
