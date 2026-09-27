//! Types shared with the frontend over IPC. TypeScript definitions are generated from these
//! (SPEC §4.3); never hand-write them on the frontend.
//!
//! IDs and timestamps are `i64` (SQLite rowids and UTC Unix seconds). They are exported to
//! TypeScript as `number`, which is exact for every value these can take.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Which set of articles the list shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum View {
    All,
    Unread,
    Starred,
    Today,
    Feed { id: i64 },
    Folder { id: i64 },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SortOrder {
    #[default]
    NewestFirst,
    OldestFirst,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArticleQuery {
    pub view: View,
    pub unread_only: bool,
    pub sort: SortOrder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Sidebar {
    pub counts: ViewCounts,
    pub folders: Vec<FolderNode>,
    /// Feeds that are not in any folder.
    pub feeds: Vec<FeedNode>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ViewCounts {
    pub unread: u32,
    pub starred: u32,
    /// Unread articles published since local midnight.
    pub today: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderNode {
    pub id: i64,
    pub name: String,
    pub unread_count: u32,
    pub feeds: Vec<FeedNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FeedNode {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub title: String,
    pub site_url: Option<String>,
    pub unread_count: u32,
    pub error_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArticleListItem {
    pub id: i64,
    pub feed_id: i64,
    pub feed_title: String,
    pub title: String,
    /// Plain-text excerpt, at most ~140 characters.
    pub summary: String,
    pub published_at: Option<i64>,
    pub is_read: bool,
    pub is_starred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Article {
    pub id: i64,
    pub feed_id: i64,
    pub feed_title: String,
    pub title: String,
    pub url: Option<String>,
    pub author: Option<String>,
    /// Sanitised HTML.
    pub content_html: String,
    pub published_at: Option<i64>,
    pub is_read: bool,
    pub is_starred: bool,
    pub enclosures: Vec<Enclosure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Enclosure {
    pub url: String,
    pub mime_type: Option<String>,
    pub length: Option<i64>,
}
