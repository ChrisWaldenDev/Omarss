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

/// Position in an article list; pass back to get the next page (keyset pagination).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArticleCursor {
    pub published_at: i64,
    pub id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArticlePage {
    pub items: Vec<ArticleListItem>,
    /// `None` when this is the last page.
    pub next: Option<ArticleCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Sidebar {
    pub counts: ViewCounts,
    pub folders: Vec<FolderNode>,
    /// Feeds that are not in any folder.
    pub feeds: Vec<FeedNode>,
    /// Starred articles kept from unsubscribed feeds, if there are any.
    pub deleted_feeds: Option<FeedNode>,
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
    /// Custom title if set, else the feed's own title.
    pub title: String,
    pub site_url: Option<String>,
    pub unread_count: u32,
    pub error_count: u32,
    pub last_error: Option<String>,
    /// File name of the cached favicon (served as `omarss-img://…/icon/<name>`).
    pub icon: Option<String>,
    pub paused: bool,
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
    /// Image-proxy URL of the list thumbnail, when thumbnails are on (SPEC §6.2).
    pub thumbnail: Option<String>,
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
    /// Sanitised HTML, prepared for display: images point at the image proxy (SPEC §8.4).
    pub content_html: String,
    /// Images wait for a click ("Load remote images: Never"); their addresses are in
    /// `data-omarss-src`/`data-omarss-srcset`/`data-omarss-poster`.
    pub images_blocked: bool,
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

/// A feed found by discovery (SPEC §6.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredFeed {
    pub url: String,
    pub title: Option<String>,
}

/// What a feed looks like before subscribing (SPEC §6.1: title and the last 5 items).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FeedPreview {
    pub url: String,
    pub title: String,
    pub site_url: Option<String>,
    pub items: Vec<PreviewItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewItem {
    pub title: String,
    pub url: Option<String>,
    pub published_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeRequest {
    pub url: String,
    pub folder_id: Option<i64>,
    /// Optional custom title.
    pub title: Option<String>,
}

/// Everything the "Edit feed" dialog shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FeedDetails {
    pub id: i64,
    pub url: String,
    /// The title the feed itself provides.
    pub title: String,
    pub custom_title: Option<String>,
    pub site_url: Option<String>,
    pub folder_id: Option<i64>,
    /// Refresh interval override in seconds; `None` uses the global setting, `0` means
    /// manual refresh only.
    pub fetch_interval: Option<i64>,
    pub paused: bool,
    pub error_count: u32,
    pub last_error: Option<String>,
    pub last_fetched_at: Option<i64>,
    /// User-Agent override for sites that block unknown agents (SPEC §7.1).
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FeedUpdate {
    /// The feed address; changing it resets the feed's error state and fetches it again.
    pub url: String,
    /// Empty or `None` clears the custom title.
    pub custom_title: Option<String>,
    pub folder_id: Option<i64>,
    /// See [`FeedDetails::fetch_interval`].
    pub fetch_interval: Option<i64>,
    pub paused: bool,
    /// Empty or `None` uses the default User-Agent.
    pub user_agent: Option<String>,
}

/// The full sidebar order after a drag and drop: folders top to bottom, then every feed top
/// to bottom with the folder it now belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SidebarOrder {
    pub folders: Vec<i64>,
    pub feeds: Vec<FeedPlacement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FeedPlacement {
    pub id: i64,
    pub folder_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RefreshTarget {
    All,
    Feed { id: i64 },
    Folder { id: i64 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RefreshStatus {
    pub running: bool,
    pub done: u32,
    pub total: u32,
    /// Every feed failed to connect in the last run, so automatic refresh is waiting for the
    /// network to come back (SPEC §7.2).
    pub offline: bool,
    pub last_finished_at: Option<i64>,
}

/// "Mark all as read" can be limited to older articles (SPEC §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum OlderThan {
    Day,
    Week,
}

impl OlderThan {
    pub fn seconds(self) -> i64 {
        match self {
            OlderThan::Day => 86_400,
            OlderThan::Week => 7 * 86_400,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MarkAllReadResult {
    pub count: u32,
    /// Pass to `undo_mark_all_read`; `None` when nothing changed.
    pub undo_token: Option<u32>,
}

/// What an OPML import did (SPEC §6.4).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub feeds: u32,
    pub folders: u32,
    /// Already subscribed, or listed twice in the file.
    pub duplicates: u32,
    /// Outlines whose address isn't a web address.
    pub invalid: u32,
}

/// Sizes shown in Settings → Storage (SPEC §6.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    pub database_bytes: u64,
    pub image_cache_bytes: u64,
    pub article_count: u32,
    pub data_dir: String,
}

/// Facts about this installation for Settings → About and platform-specific settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub platform: String,
    pub homepage: String,
    pub data_dir: String,
    pub config_dir: String,
    /// Metered-connection detection is available (Windows).
    pub metered_supported: bool,
}
