//! Read-side commands for the sidebar, article list and reader.

use tauri::State;

use crate::error::AppResult;
use crate::models::{Article, ArticleCursor, ArticlePage, ArticleQuery, Sidebar};
use crate::services::articles::ArticleService;

#[tauri::command]
#[specta::specta]
pub async fn get_sidebar(service: State<'_, ArticleService>) -> AppResult<Sidebar> {
    service.sidebar().await
}

/// One page of the list. Pass the previous page's `next` as `after` to continue.
#[tauri::command]
#[specta::specta]
pub async fn list_articles(
    service: State<'_, ArticleService>,
    query: ArticleQuery,
    after: Option<ArticleCursor>,
    limit: u32,
) -> AppResult<ArticlePage> {
    service.list(query, after, limit).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_article(service: State<'_, ArticleService>, id: i64) -> AppResult<Article> {
    service.get(id).await
}

/// Marks articles read or unread; returns how many changed.
#[tauri::command]
#[specta::specta]
pub async fn set_articles_read(
    service: State<'_, ArticleService>,
    ids: Vec<i64>,
    read: bool,
) -> AppResult<u32> {
    service.set_read(ids, read).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_article_starred(
    service: State<'_, ArticleService>,
    id: i64,
    starred: bool,
) -> AppResult<()> {
    service.set_starred(id, starred).await
}
