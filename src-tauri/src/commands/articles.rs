//! Read-side commands for the sidebar, article list and reader.
//!
//! M1 serves built-in demo data; M2 replaces `demo` with the feed/article services.

use crate::demo;
use crate::error::{AppError, AppResult};
use crate::models::{Article, ArticleListItem, ArticleQuery, Sidebar};

#[tauri::command]
#[specta::specta]
pub async fn get_sidebar() -> AppResult<Sidebar> {
    Ok(demo::sidebar())
}

#[tauri::command]
#[specta::specta]
pub async fn list_articles(query: ArticleQuery) -> AppResult<Vec<ArticleListItem>> {
    Ok(demo::list_articles(&query))
}

#[tauri::command]
#[specta::specta]
pub async fn get_article(id: i64) -> AppResult<Article> {
    demo::get_article(id).ok_or_else(|| AppError::not_found(format!("Article {id} not found")))
}
