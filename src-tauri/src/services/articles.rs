//! Reading: sidebar, article lists and the reader (SPEC §6.2).

use crate::clock;
use crate::content::html_to_text;
use crate::error::AppResult;
use crate::models::{
    Article, ArticleCursor, ArticleListItem, ArticlePage, ArticleQuery, FeedNode, FolderNode,
    Sidebar,
};
use crate::store::{articles, feeds, folders, Store};

/// Longest excerpt shown in the list (SPEC §6.2: "first ~140 chars of summary").
const EXCERPT_CHARS: usize = 140;
pub const MAX_PAGE_SIZE: u32 = 500;

#[derive(Clone)]
pub struct ArticleService {
    store: Store,
}

impl ArticleService {
    pub fn new(store: Store) -> Self {
        Self { store }
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
                    })
                    .collect();
                Ok(ArticlePage { items, next })
            })
            .await
    }

    pub async fn get(&self, id: i64) -> AppResult<Article> {
        self.store
            .run(move |conn| {
                let row = articles::get(conn, id)?;
                Ok(Article {
                    id: row.id,
                    feed_id: row.feed_id,
                    feed_title: row.feed_title,
                    title: row.title,
                    url: row.url,
                    author: row.author,
                    content_html: row.content_html,
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
}
