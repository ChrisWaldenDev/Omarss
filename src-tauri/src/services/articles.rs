//! Reading: sidebar, article lists and the reader (SPEC §6.2).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::clock;
use crate::content::{
    for_display, html_to_text, strip_tracking_params, DisplayOptions, ImageDisplay,
};
use crate::error::{AppError, AppResult};
use crate::models::{
    Article, ArticleCursor, ArticleListItem, ArticlePage, ArticleQuery, FeedNode, FolderNode,
    MarkAllReadResult, OlderThan, Sidebar, View,
};
use crate::services::images::proxy_url;
use crate::services::settings::{LoadImages, SettingsService};
use crate::store::{articles, feeds, folders, Store};

/// Longest excerpt shown in the list (SPEC §6.2: "first ~140 chars of summary").
const EXCERPT_CHARS: usize = 140;
pub const MAX_PAGE_SIZE: u32 = 500;

/// Undo token and the articles a "Mark all as read" changed.
type MarkAllBatch = (u32, Vec<i64>);

#[derive(Clone)]
pub struct ArticleService {
    store: Store,
    settings: SettingsService,
    /// The last "Mark all as read", so it can be undone (SPEC §6.2).
    last_mark_all: Arc<Mutex<Option<MarkAllBatch>>>,
    next_token: Arc<AtomicU32>,
}

impl ArticleService {
    pub fn new(store: Store, settings: SettingsService) -> Self {
        Self {
            store,
            settings,
            last_mark_all: Arc::default(),
            next_token: Arc::new(AtomicU32::new(1)),
        }
    }

    pub async fn sidebar(&self) -> AppResult<Sidebar> {
        self.store
            .run(|conn| {
                let counts = articles::counts(conn, clock::local_day_start())?;
                let deleted_id = feeds::deleted_feeds_id(conn)?;
                let mut all_feeds = feeds::sidebar_rows(conn)?;
                let deleted_feeds = match deleted_id {
                    Some(id) => {
                        let node = all_feeds
                            .iter()
                            .position(|f| f.id == id)
                            .map(|i| all_feeds.remove(i));
                        node.filter(|_| feeds::article_count(conn, id).is_ok_and(|n| n > 0))
                    }
                    None => None,
                };
                let folders = folders::list(conn)?
                    .into_iter()
                    .map(|folder| {
                        let feeds: Vec<FeedNode> = all_feeds
                            .iter()
                            .filter(|f| f.folder_id == Some(folder.id))
                            .cloned()
                            .collect();
                        FolderNode {
                            id: folder.id,
                            name: folder.name,
                            unread_count: feeds.iter().map(|f| f.unread_count).sum(),
                            feeds,
                        }
                    })
                    .collect();
                let root = all_feeds
                    .into_iter()
                    .filter(|f| f.folder_id.is_none())
                    .collect();
                Ok(Sidebar {
                    counts,
                    folders,
                    feeds: root,
                    deleted_feeds,
                })
            })
            .await
    }

    /// One page of articles; pass `next` back as `after` for the following page.
    pub async fn list(
        &self,
        query: ArticleQuery,
        after: Option<ArticleCursor>,
        limit: u32,
    ) -> AppResult<ArticlePage> {
        let limit = limit.clamp(1, MAX_PAGE_SIZE);
        let settings = self.settings.get().await?;
        let thumbnails = settings.list_thumbnails && settings.load_images == LoadImages::Always;
        self.store
            .run(move |conn| {
                let filter = articles::ListFilter {
                    view: &query.view,
                    unread_only: query.unread_only,
                    sort: query.sort,
                    today_start: clock::local_day_start(),
                };
                // Ask for one extra row to learn whether another page exists.
                let mut rows = articles::list(conn, &filter, after, limit + 1)?;
                let more = rows.len() > limit as usize;
                rows.truncate(limit as usize);
                let next = more
                    .then(|| rows.last())
                    .flatten()
                    .map(|row| ArticleCursor {
                        published_at: row.published_at,
                        id: row.id,
                    });
                let items = rows
                    .into_iter()
                    .map(|row| ArticleListItem {
                        id: row.id,
                        feed_id: row.feed_id,
                        feed_title: row.feed_title,
                        title: row.title,
                        summary: html_to_text(&row.summary_html, EXCERPT_CHARS),
                        published_at: Some(row.published_at),
                        is_read: row.is_read,
                        is_starred: row.is_starred,
                        thumbnail: row
                            .thumbnail_url
                            .filter(|_| thumbnails)
                            .map(|url| proxy_url(&url)),
                    })
                    .collect();
                Ok(ArticlePage { items, next })
            })
            .await
    }

    /// An article prepared for the reader: images go through the image proxy (or wait for a
    /// click) and, if enabled, `utm_*` parameters leave its links (SPEC §8.4).
    pub async fn get(&self, id: i64) -> AppResult<Article> {
        let settings = self.settings.get().await?;
        let images = match settings.load_images {
            LoadImages::Never => ImageDisplay::ClickToLoad,
            LoadImages::Always | LoadImages::Opened => ImageDisplay::Load,
        };
        let strip = settings.strip_tracking_params;
        self.store
            .run(move |conn| {
                let row = articles::get(conn, id)?;
                let content_html = for_display(
                    &row.content_html,
                    &DisplayOptions {
                        images,
                        strip_tracking_params: strip,
                        proxy: &proxy_url,
                    },
                );
                Ok(Article {
                    id: row.id,
                    feed_id: row.feed_id,
                    feed_title: row.feed_title,
                    title: row.title,
                    url: row.url.map(|url| {
                        if strip {
                            strip_tracking_params(&url)
                        } else {
                            url
                        }
                    }),
                    author: row.author,
                    images_blocked: content_html.contains("data-omarss-"),
                    content_html,
                    published_at: row.published_at,
                    is_read: row.is_read,
                    is_starred: row.is_starred,
                    enclosures: row.enclosures,
                })
            })
            .await
    }

    pub async fn set_read(&self, ids: Vec<i64>, read: bool) -> AppResult<u32> {
        self.store
            .run(move |conn| {
                let changed = articles::set_read(conn, &ids, read, clock::now_unix())?;
                Ok(u32::try_from(changed).unwrap_or(u32::MAX))
            })
            .await
    }

    pub async fn set_starred(&self, id: i64, starred: bool) -> AppResult<()> {
        self.store
            .run(move |conn| articles::set_starred(conn, id, starred, clock::now_unix()))
            .await
    }

    /// Marks every unread article in `view` read, optionally only those older than a day or a
    /// week (SPEC §6.2). The result's token undoes it until the next "mark all".
    pub async fn mark_all_read(
        &self,
        view: View,
        older_than: Option<OlderThan>,
    ) -> AppResult<MarkAllReadResult> {
        let now = clock::now_unix();
        let before = older_than.map(|o| now - o.seconds());
        let ids = self
            .store
            .run(move |conn| {
                let tx = conn.transaction()?;
                let ids = articles::unread_ids(&tx, &view, clock::local_day_start(), before)?;
                articles::set_read(&tx, &ids, true, now)?;
                tx.commit()?;
                Ok(ids)
            })
            .await?;
        let count = u32::try_from(ids.len()).unwrap_or(u32::MAX);
        if ids.is_empty() {
            return Ok(MarkAllReadResult {
                count,
                undo_token: None,
            });
        }
        let token = self.next_token.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut last) = self.last_mark_all.lock() {
            *last = Some((token, ids));
        }
        Ok(MarkAllReadResult {
            count,
            undo_token: Some(token),
        })
    }

    /// Marks the articles of the "mark all" with `token` unread again; returns how many.
    pub async fn undo_mark_all_read(&self, token: u32) -> AppResult<u32> {
        let ids = match self.last_mark_all.lock() {
            Ok(mut last) if last.as_ref().is_some_and(|(t, _)| *t == token) => {
                last.take().map(|(_, ids)| ids).unwrap_or_default()
            }
            _ => return Err(AppError::not_found("That can no longer be undone")),
        };
        self.store
            .run(move |conn| {
                let tx = conn.transaction()?;
                let changed = articles::set_read(&tx, &ids, false, clock::now_unix())?;
                tx.commit()?;
                Ok(u32::try_from(changed).unwrap_or(u32::MAX))
            })
            .await
    }
}
