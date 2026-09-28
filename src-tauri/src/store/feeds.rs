//! Repository for `feeds` (and the account they belong to).

use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::error::{AppError, AppResult};
use crate::models::{FeedDetails, FeedNode, FeedPlacement};

/// Sentinel URL of the "Deleted feeds" pseudo-feed, which keeps starred articles from
/// unsubscribed feeds (SPEC §6.1). It is paused and never fetched.
pub const DELETED_FEEDS_URL: &str = "omarss:deleted-feeds";
const DELETED_FEEDS_TITLE: &str = "Deleted feeds";

pub fn local_account_id(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT id FROM accounts WHERE kind = 'local' ORDER BY id LIMIT 1",
        [],
        |row| row.get(0),
    )?)
}

pub struct NewFeed<'a> {
    pub url: &'a str,
    pub title: &'a str,
    pub custom_title: Option<&'a str>,
    pub site_url: Option<&'a str>,
    pub description: Option<&'a str>,
    pub folder_id: Option<i64>,
    pub now: i64,
}

pub fn insert(tx: &Transaction, feed: &NewFeed) -> AppResult<i64> {
    let account_id = local_account_id(tx)?;
    let sort_order: i64 = tx.query_row(
        "SELECT coalesce(max(sort_order) + 1, 0) FROM feeds WHERE account_id = ?1",
        [account_id],
        |row| row.get(0),
    )?;
    let result = tx.execute(
        "INSERT INTO feeds (account_id, folder_id, url, site_url, title, custom_title,
                            description, sort_order, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            account_id,
            feed.folder_id,
            feed.url,
            feed.site_url,
            feed.title,
            feed.custom_title,
            feed.description,
            sort_order,
            feed.now
        ],
    );
    match result {
        Ok(_) => Ok(tx.last_insert_rowid()),
        Err(rusqlite::Error::SqliteFailure(e, _))
            if e.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            Err(AppError::conflict("You're already subscribed to this feed"))
        }
        Err(err) => Err(err.into()),
    }
}

/// Everything about a feed that fetching and the settings dialog need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedRecord {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub custom_title: Option<String>,
    pub site_url: Option<String>,
    pub folder_id: Option<i64>,
    pub icon_path: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub last_fetched_at: Option<i64>,
    pub next_fetch_at: Option<i64>,
    pub fetch_interval: Option<i64>,
    pub error_count: u32,
    pub last_error: Option<String>,
    pub paused: bool,
    pub user_agent: Option<String>,
}

impl FeedRecord {
    pub fn is_deleted_feeds(&self) -> bool {
        self.url == DELETED_FEEDS_URL
    }

    pub fn details(&self) -> FeedDetails {
        FeedDetails {
            id: self.id,
            url: self.url.clone(),
            title: self.title.clone(),
            custom_title: self.custom_title.clone(),
            site_url: self.site_url.clone(),
            folder_id: self.folder_id,
            fetch_interval: self.fetch_interval,
            paused: self.paused,
            error_count: self.error_count,
            last_error: self.last_error.clone(),
            last_fetched_at: self.last_fetched_at,
            user_agent: self.user_agent.clone(),
        }
    }
}

const RECORD_COLUMNS: &str = "id, url, title, custom_title, site_url, folder_id, icon_path, etag,
    last_modified, last_fetched_at, next_fetch_at, fetch_interval, error_count, last_error, paused,
    user_agent";

fn record(row: &Row) -> rusqlite::Result<FeedRecord> {
    Ok(FeedRecord {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        custom_title: row.get(3)?,
        site_url: row.get(4)?,
        folder_id: row.get(5)?,
        icon_path: row.get(6)?,
        etag: row.get(7)?,
        last_modified: row.get(8)?,
        last_fetched_at: row.get(9)?,
        next_fetch_at: row.get(10)?,
        fetch_interval: row.get(11)?,
        error_count: row.get(12)?,
        last_error: row.get(13)?,
        paused: row.get(14)?,
        user_agent: row.get(15)?,
    })
}

pub fn get(conn: &Connection, id: i64) -> AppResult<FeedRecord> {
    conn.query_row(
        &format!("SELECT {RECORD_COLUMNS} FROM feeds WHERE id = ?1"),
        [id],
        record,
    )
    .optional()?
    .ok_or_else(|| AppError::not_found(format!("Feed {id} not found")))
}

/// All feeds, for building the sidebar. Unread counts exclude hidden articles.
pub fn sidebar_rows(conn: &Connection) -> AppResult<Vec<FeedNode>> {
    let mut stmt = conn.prepare_cached(
        "SELECT f.id, f.folder_id, coalesce(nullif(f.custom_title, ''), f.title), f.site_url,
                coalesce(u.unread, 0), f.error_count, f.last_error, f.icon_path, f.paused
         FROM feeds f
         LEFT JOIN (SELECT feed_id, count(*) AS unread FROM articles
                    WHERE is_read = 0 AND hidden = 0 GROUP BY feed_id) u ON u.feed_id = f.id
         ORDER BY f.sort_order, f.title COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(FeedNode {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                title: row.get(2)?,
                site_url: row.get(3)?,
                unread_count: row.get(4)?,
                error_count: row.get(5)?,
                last_error: row.get(6)?,
                icon: row.get(7)?,
                paused: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn article_count(conn: &Connection, feed_id: i64) -> AppResult<u32> {
    Ok(conn.query_row(
        "SELECT count(*) FROM articles WHERE feed_id = ?1",
        [feed_id],
        |row| row.get(0),
    )?)
}

pub fn deleted_feeds_id(conn: &Connection) -> AppResult<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT id FROM feeds WHERE url = ?1",
            [DELETED_FEEDS_URL],
            |row| row.get(0),
        )
        .optional()?)
}

/// Feeds whose automatic refresh is due, most overdue first.
pub fn due(conn: &Connection, now: i64) -> AppResult<Vec<(i64, String)>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, url FROM feeds
         WHERE paused = 0 AND next_fetch_at IS NOT NULL AND next_fetch_at <= ?1
         ORDER BY next_fetch_at",
    )?;
    let rows = stmt
        .query_map([now], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn next_due_at(conn: &Connection) -> AppResult<Option<i64>> {
    Ok(conn.query_row(
        "SELECT min(next_fetch_at) FROM feeds WHERE paused = 0",
        [],
        |row| row.get(0),
    )?)
}

/// Feeds a manual refresh should fetch: every unpaused feed, or those in one folder.
pub fn refreshable(conn: &Connection, folder_id: Option<i64>) -> AppResult<Vec<(i64, String)>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, url FROM feeds
         WHERE paused = 0 AND url != ?1 AND (?2 IS NULL OR folder_id = ?2)
         ORDER BY sort_order",
    )?;
    let rows = stmt
        .query_map(params![DELETED_FEEDS_URL, folder_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// What a successful fetch (200 or 304) changes.
pub struct FetchSuccess<'a> {
    /// New URL after a permanent redirect.
    pub new_url: Option<&'a str>,
    pub etag: Option<&'a str>,
    pub last_modified: Option<&'a str>,
    /// Feed metadata; `None` after a 304.
    pub title: Option<&'a str>,
    pub site_url: Option<&'a str>,
    pub description: Option<&'a str>,
    pub fetched_at: i64,
    pub next_fetch_at: Option<i64>,
}

pub fn record_success(conn: &Connection, id: i64, s: &FetchSuccess) -> AppResult<()> {
    conn.execute(
        "UPDATE feeds SET etag = coalesce(?2, etag), last_modified = coalesce(?3, last_modified),
                title = coalesce(?4, title), site_url = coalesce(?5, site_url),
                description = coalesce(?6, description),
                last_fetched_at = ?7, next_fetch_at = ?8, error_count = 0, last_error = NULL
         WHERE id = ?1",
        params![
            id,
            s.etag,
            s.last_modified,
            s.title,
            s.site_url,
            s.description,
            s.fetched_at,
            s.next_fetch_at
        ],
    )?;
    if let Some(url) = s.new_url {
        // Another subscription may already use that URL; then keep the old one.
        let moved = conn.execute(
            "UPDATE OR IGNORE feeds SET url = ?2 WHERE id = ?1",
            params![id, url],
        )?;
        if moved == 0 {
            tracing::warn!(feed = id, "permanent redirect target is already subscribed");
        }
    }
    Ok(())
}

pub fn record_error(
    conn: &Connection,
    id: i64,
    message: &str,
    fetched_at: i64,
    next_fetch_at: Option<i64>,
    pause: bool,
) -> AppResult<()> {
    conn.execute(
        "UPDATE feeds SET error_count = error_count + 1, last_error = ?2, last_fetched_at = ?3,
                next_fetch_at = ?4, paused = paused OR ?5
         WHERE id = ?1",
        params![id, message, fetched_at, next_fetch_at, pause],
    )?;
    Ok(())
}

pub fn reschedule(conn: &Connection, id: i64, next_fetch_at: Option<i64>) -> AppResult<()> {
    conn.execute(
        "UPDATE feeds SET next_fetch_at = ?2 WHERE id = ?1",
        params![id, next_fetch_at],
    )?;
    Ok(())
}

pub fn set_icon(conn: &Connection, id: i64, icon_path: Option<&str>) -> AppResult<()> {
    conn.execute(
        "UPDATE feeds SET icon_path = ?2 WHERE id = ?1",
        params![id, icon_path],
    )?;
    Ok(())
}

pub struct SettingsUpdate<'a> {
    pub custom_title: Option<&'a str>,
    pub folder_id: Option<i64>,
    pub fetch_interval: Option<i64>,
    pub paused: bool,
    pub next_fetch_at: Option<i64>,
    pub user_agent: Option<&'a str>,
}

pub fn update_settings(conn: &Connection, id: i64, u: &SettingsUpdate) -> AppResult<()> {
    let changed = conn.execute(
        "UPDATE feeds SET custom_title = ?2, folder_id = ?3, fetch_interval = ?4, paused = ?5,
                next_fetch_at = ?6, user_agent = ?7
         WHERE id = ?1",
        params![
            id,
            u.custom_title,
            u.folder_id,
            u.fetch_interval,
            u.paused,
            u.next_fetch_at,
            u.user_agent
        ],
    )?;
    if changed == 0 {
        return Err(AppError::not_found(format!("Feed {id} not found")));
    }
    Ok(())
}

/// Points a feed at a new address (SPEC §6.1 "Edit URL"): cached validators and the error
/// state are reset so the next fetch starts clean.
pub fn change_url(conn: &Connection, id: i64, url: &str) -> AppResult<()> {
    let result = conn.execute(
        "UPDATE feeds SET url = ?2, etag = NULL, last_modified = NULL, error_count = 0,
                last_error = NULL
         WHERE id = ?1",
        params![id, url],
    );
    match result {
        Ok(0) => Err(AppError::not_found(format!("Feed {id} not found"))),
        Ok(_) => Ok(()),
        Err(rusqlite::Error::SqliteFailure(e, _))
            if e.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            Err(AppError::conflict(
                "You're already subscribed to that address",
            ))
        }
        Err(err) => Err(err.into()),
    }
}

/// Deletes a feed and its articles, except starred ones, which move to the "Deleted feeds"
/// pseudo-feed (SPEC §6.1). Returns the deleted feed's icon file, if any.
pub fn delete_keeping_starred(tx: &Transaction, id: i64, now: i64) -> AppResult<Option<String>> {
    let feed = get(tx, id)?;
    if feed.is_deleted_feeds() {
        return Err(AppError::invalid_input(
            "\"Deleted feeds\" can't be unsubscribed",
        ));
    }
    let starred: i64 = tx.query_row(
        "SELECT count(*) FROM articles WHERE feed_id = ?1 AND is_starred = 1",
        [id],
        |row| row.get(0),
    )?;
    if starred > 0 {
        let target = match deleted_feeds_id(tx)? {
            Some(target) => target,
            None => insert(
                tx,
                &NewFeed {
                    url: DELETED_FEEDS_URL,
                    title: DELETED_FEEDS_TITLE,
                    custom_title: None,
                    site_url: None,
                    description: None,
                    folder_id: None,
                    now,
                },
            )
            .and_then(|new_id| {
                tx.execute("UPDATE feeds SET paused = 1 WHERE id = ?1", [new_id])?;
                Ok(new_id)
            })?,
        };
        // Prefix GUIDs with the old feed id so they stay unique within the pseudo-feed.
        tx.execute(
            "UPDATE articles SET feed_id = ?2, guid = ?1 || ':' || guid
             WHERE feed_id = ?1 AND is_starred = 1",
            params![id, target],
        )?;
    }
    tx.execute("DELETE FROM feeds WHERE id = ?1", [id])?;
    Ok(feed.icon_path)
}

/// Applies a sidebar drag-and-drop result: order and folder of every listed feed.
pub fn reorder(tx: &Transaction, feeds: &[FeedPlacement]) -> AppResult<()> {
    let mut stmt =
        tx.prepare_cached("UPDATE feeds SET sort_order = ?2, folder_id = ?3 WHERE id = ?1")?;
    for (index, feed) in feeds.iter().enumerate() {
        stmt.execute(params![feed.id, index as i64, feed.folder_id])?;
    }
    Ok(())
}

/// Every subscribed feed address (for skipping duplicates on import).
pub fn all_urls(conn: &Connection) -> AppResult<HashSet<String>> {
    let mut stmt = conn.prepare_cached("SELECT url FROM feeds")?;
    let urls = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<HashSet<String>, _>>()?;
    Ok(urls)
}

/// A feed as OPML export needs it, in sidebar order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportRow {
    pub folder_id: Option<i64>,
    pub title: String,
    pub custom_title: Option<String>,
    pub url: String,
    pub site_url: Option<String>,
}

pub fn export_rows(conn: &Connection) -> AppResult<Vec<ExportRow>> {
    let mut stmt = conn.prepare_cached(
        "SELECT folder_id, title, nullif(custom_title, ''), url, site_url FROM feeds
         WHERE url != ?1 ORDER BY sort_order, title COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map([DELETED_FEEDS_URL], |row| {
            Ok(ExportRow {
                folder_id: row.get(0)?,
                title: row.get(1)?,
                custom_title: row.get(2)?,
                url: row.get(3)?,
                site_url: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
