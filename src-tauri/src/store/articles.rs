//! Repository for `articles` and `enclosures`.

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};

use crate::error::{AppError, AppResult};
use crate::models::{ArticleCursor, Enclosure, SortOrder, View, ViewCounts};

#[derive(Debug, Clone, PartialEq)]
pub struct NewArticle {
    pub guid: String,
    pub url: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub summary_html: Option<String>,
    pub content_html: Option<String>,
    pub published_at: i64,
    pub updated_at: Option<i64>,
    pub content_hash: String,
    pub enclosures: Vec<Enclosure>,
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UpsertStats {
    pub inserted: usize,
    /// Articles whose content changed (the content hash differs).
    pub updated: usize,
    /// Articles with the same content but a changed link, author, update time or
    /// enclosures. These are rewritten silently: they never count as updated, so they
    /// don't mark anything unread or announce new articles.
    pub metadata_updated: usize,
}

/// Inserts new articles and updates changed ones (SPEC §7.4). Articles are matched on
/// `(feed_id, guid)`; an existing article is rewritten when its content hash changed, and
/// keeps its read/star state unless `mark_updated_unread` is set. When only the metadata
/// (url, author, updated_at, enclosures) changed, just that is written and read state is
/// untouched. `published_at` is never rewritten, so list order stays stable.
pub fn upsert(
    tx: &Transaction,
    feed_id: i64,
    items: &[NewArticle],
    now: i64,
    mark_updated_unread: bool,
) -> AppResult<UpsertStats> {
    let mut stats = UpsertStats::default();
    let mut find = tx.prepare_cached(
        "SELECT id, content_hash, url, author, updated_at FROM articles
         WHERE feed_id = ?1 AND guid = ?2",
    )?;
    let mut insert = tx.prepare_cached(
        "INSERT INTO articles (feed_id, guid, url, title, author, summary_html, content_html,
                               published_at, updated_at, fetched_at, content_hash, thumbnail_url)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;
    let mut update = tx.prepare_cached(
        "UPDATE articles SET url = ?2, title = ?3, author = ?4, summary_html = ?5,
                content_html = ?6, updated_at = ?7, content_hash = ?8,
                is_read = CASE WHEN ?9 THEN 0 ELSE is_read END,
                read_at = CASE WHEN ?9 THEN NULL ELSE read_at END,
                thumbnail_url = ?10
         WHERE id = ?1",
    )?;
    let mut update_metadata = tx.prepare_cached(
        "UPDATE articles SET url = ?2, author = ?3, updated_at = ?4 WHERE id = ?1",
    )?;
    for item in items {
        let existing: Option<StoredArticle> = find
            .query_row(params![feed_id, item.guid], |row| {
                Ok(StoredArticle {
                    id: row.get(0)?,
                    content_hash: row.get(1)?,
                    url: row.get(2)?,
                    author: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .optional()?;
        let article_id = match existing {
            None => {
                insert.execute(params![
                    feed_id,
                    item.guid,
                    item.url,
                    item.title,
                    item.author,
                    item.summary_html,
                    item.content_html,
                    item.published_at,
                    item.updated_at,
                    now,
                    item.content_hash,
                    item.thumbnail_url
                ])?;
                stats.inserted += 1;
                tx.last_insert_rowid()
            }
            Some(stored) if stored.content_hash.as_deref() == Some(item.content_hash.as_str()) => {
                let fields_changed = stored.url != item.url
                    || stored.author != item.author
                    || stored.updated_at != item.updated_at;
                let enclosures_changed = enclosures(tx, stored.id)? != item.enclosures;
                if fields_changed {
                    update_metadata.execute(params![
                        stored.id,
                        item.url,
                        item.author,
                        item.updated_at
                    ])?;
                }
                if enclosures_changed {
                    replace_enclosures(tx, stored.id, &item.enclosures)?;
                }
                if fields_changed || enclosures_changed {
                    stats.metadata_updated += 1;
                }
                continue;
            }
            Some(StoredArticle { id, .. }) => {
                update.execute(params![
                    id,
                    item.url,
                    item.title,
                    item.author,
                    item.summary_html,
                    item.content_html,
                    item.updated_at,
                    item.content_hash,
                    mark_updated_unread,
                    item.thumbnail_url
                ])?;
                stats.updated += 1;
                id
            }
        };
        replace_enclosures(tx, article_id, &item.enclosures)?;
    }
    Ok(stats)
}

/// The columns `upsert` compares against an incoming item.
struct StoredArticle {
    id: i64,
    content_hash: Option<String>,
    url: Option<String>,
    author: Option<String>,
    updated_at: Option<i64>,
}

fn enclosures(conn: &Connection, article_id: i64) -> AppResult<Vec<Enclosure>> {
    let mut stmt = conn.prepare_cached(
        "SELECT url, mime_type, length FROM enclosures WHERE article_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([article_id], |row| {
            Ok(Enclosure {
                url: row.get(0)?,
                mime_type: row.get(1)?,
                length: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn replace_enclosures(
    tx: &Transaction,
    article_id: i64,
    enclosures: &[Enclosure],
) -> AppResult<()> {
    tx.execute("DELETE FROM enclosures WHERE article_id = ?1", [article_id])?;
    let mut stmt = tx.prepare_cached(
        "INSERT INTO enclosures (article_id, url, mime_type, length) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for e in enclosures {
        stmt.execute(params![article_id, e.url, e.mime_type, e.length])?;
    }
    Ok(())
}

pub struct ListFilter<'a> {
    pub view: &'a View,
    pub unread_only: bool,
    pub sort: SortOrder,
    /// Start of "today" for the Today view.
    pub today_start: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRow {
    pub id: i64,
    pub feed_id: i64,
    pub feed_title: String,
    pub title: String,
    /// The start of the summary (or content) HTML, for building an excerpt.
    pub summary_html: String,
    pub published_at: i64,
    pub is_read: bool,
    pub is_starred: bool,
    pub thumbnail_url: Option<String>,
}

/// One page of a view, ordered by `(published_at, id)` with keyset pagination.
pub fn list(
    conn: &Connection,
    filter: &ListFilter,
    after: Option<ArticleCursor>,
    limit: u32,
) -> AppResult<Vec<ListRow>> {
    let mut sql = String::from(
        "SELECT a.id, a.feed_id, coalesce(nullif(f.custom_title, ''), f.title), a.title,
                substr(coalesce(nullif(a.summary_html, ''), a.content_html, ''), 1, 3000),
                coalesce(a.published_at, a.fetched_at), a.is_read, a.is_starred, a.thumbnail_url
         FROM articles a JOIN feeds f ON f.id = a.feed_id
         WHERE a.hidden = 0",
    );
    let mut args: Vec<Value> = Vec::new();
    push_view_condition(filter.view, filter.today_start, &mut sql, &mut args);
    // The Starred view lists every starred article; the Unread view is unread by definition.
    if filter.unread_only && !matches!(filter.view, View::Starred | View::Unread) {
        sql.push_str(" AND a.is_read = 0");
    }
    let (cmp, dir) = match filter.sort {
        SortOrder::NewestFirst => ("<", "DESC"),
        SortOrder::OldestFirst => (">", "ASC"),
    };
    if let Some(cursor) = after {
        sql.push_str(&format!(" AND (a.published_at, a.id) {cmp} (?, ?)"));
        args.push(cursor.published_at.into());
        args.push(cursor.id.into());
    }
    sql.push_str(&format!(
        " ORDER BY a.published_at {dir}, a.id {dir} LIMIT ?"
    ));
    args.push(i64::from(limit).into());

    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt
        .query_map(params_from_iter(args), |row| {
            Ok(ListRow {
                id: row.get(0)?,
                feed_id: row.get(1)?,
                feed_title: row.get(2)?,
                title: row.get(3)?,
                summary_html: row.get(4)?,
                published_at: row.get(5)?,
                is_read: row.get(6)?,
                is_starred: row.get(7)?,
                thumbnail_url: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Narrows a query on `articles a` to one view.
fn push_view_condition(view: &View, today_start: i64, sql: &mut String, args: &mut Vec<Value>) {
    match view {
        View::All => {}
        View::Unread => sql.push_str(" AND a.is_read = 0"),
        View::Starred => sql.push_str(" AND a.is_starred = 1"),
        View::Today => {
            sql.push_str(" AND a.published_at >= ?");
            args.push(today_start.into());
        }
        View::Feed { id } => {
            sql.push_str(" AND a.feed_id = ?");
            args.push((*id).into());
        }
        View::Folder { id } => {
            sql.push_str(" AND a.feed_id IN (SELECT id FROM feeds WHERE folder_id = ?)");
            args.push((*id).into());
        }
    }
}

/// Unread articles in a view, optionally only those published before `before` (for "Mark
/// all as read", SPEC §6.2).
pub fn unread_ids(
    conn: &Connection,
    view: &View,
    today_start: i64,
    before: Option<i64>,
) -> AppResult<Vec<i64>> {
    let mut sql = String::from("SELECT a.id FROM articles a WHERE a.hidden = 0 AND a.is_read = 0");
    let mut args: Vec<Value> = Vec::new();
    push_view_condition(view, today_start, &mut sql, &mut args);
    if let Some(before) = before {
        sql.push_str(" AND a.published_at < ?");
        args.push(before.into());
    }
    let mut stmt = conn.prepare(&sql)?;
    let ids = stmt
        .query_map(params_from_iter(args), |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

/// Retention cleanup (SPEC §6.9): deletes articles published before `cutoff`, except starred
/// or tagged ones and the newest `keep_per_feed` of every feed. Returns how many were deleted.
pub fn delete_expired(conn: &Connection, cutoff: i64, keep_per_feed: u32) -> AppResult<usize> {
    Ok(conn.execute(
        "DELETE FROM articles WHERE id IN (
             SELECT r.id FROM (
                 SELECT id, is_starred, coalesce(published_at, fetched_at) AS at,
                        row_number() OVER (PARTITION BY feed_id
                                           ORDER BY published_at DESC, id DESC) AS position
                 FROM articles
             ) r
             WHERE r.position > ?2 AND r.is_starred = 0 AND r.at < ?1
               AND NOT EXISTS (SELECT 1 FROM article_tags t WHERE t.article_id = r.id)
         )",
        params![cutoff, keep_per_feed],
    )?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArticleRow {
    pub id: i64,
    pub feed_id: i64,
    pub feed_title: String,
    pub title: String,
    pub url: Option<String>,
    pub author: Option<String>,
    pub content_html: String,
    pub published_at: Option<i64>,
    pub is_read: bool,
    pub is_starred: bool,
    pub enclosures: Vec<Enclosure>,
}

pub fn get(conn: &Connection, id: i64) -> AppResult<ArticleRow> {
    let mut article = conn
        .query_row(
            "SELECT a.id, a.feed_id, coalesce(nullif(f.custom_title, ''), f.title), a.title, a.url,
                    a.author, coalesce(nullif(a.content_html, ''), a.summary_html, ''),
                    a.published_at, a.is_read, a.is_starred
             FROM articles a JOIN feeds f ON f.id = a.feed_id WHERE a.id = ?1",
            [id],
            |row| {
                Ok(ArticleRow {
                    id: row.get(0)?,
                    feed_id: row.get(1)?,
                    feed_title: row.get(2)?,
                    title: row.get(3)?,
                    url: row.get(4)?,
                    author: row.get(5)?,
                    content_html: row.get(6)?,
                    published_at: row.get(7)?,
                    is_read: row.get(8)?,
                    is_starred: row.get(9)?,
                    enclosures: Vec::new(),
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found(format!("Article {id} not found")))?;
    article.enclosures = enclosures(conn, id)?;
    Ok(article)
}

/// Most ids bound in one statement (SQLite's limit on parameters is 32766).
const IDS_PER_STATEMENT: usize = 500;

/// Marks articles read or unread; returns how many changed. Callers wanting all-or-nothing for
/// long lists pass a transaction.
pub fn set_read(conn: &Connection, ids: &[i64], read: bool, now: i64) -> AppResult<usize> {
    let mut changed = 0;
    for chunk in ids.chunks(IDS_PER_STATEMENT) {
        let placeholders = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "UPDATE articles SET is_read = ?1, read_at = CASE WHEN ?1 THEN ?2 ELSE NULL END
             WHERE is_read != ?1 AND id IN ({placeholders})"
        );
        let mut args: Vec<Value> = vec![Value::from(read), Value::from(now)];
        args.extend(chunk.iter().map(|id| Value::from(*id)));
        changed += conn.execute(&sql, params_from_iter(args))?;
    }
    Ok(changed)
}

pub fn set_starred(conn: &Connection, id: i64, starred: bool, now: i64) -> AppResult<()> {
    let changed = conn.execute(
        "UPDATE articles SET is_starred = ?2, starred_at = CASE WHEN ?2 THEN ?3 ELSE NULL END
         WHERE id = ?1",
        params![id, starred, now],
    )?;
    if changed == 0 {
        return Err(AppError::not_found(format!("Article {id} not found")));
    }
    Ok(())
}

pub fn counts(conn: &Connection, today_start: i64) -> AppResult<ViewCounts> {
    Ok(conn.query_row(
        "SELECT (SELECT count(*) FROM articles WHERE is_read = 0 AND hidden = 0),
                (SELECT count(*) FROM articles WHERE is_starred = 1 AND hidden = 0),
                (SELECT count(*) FROM articles
                 WHERE is_read = 0 AND hidden = 0 AND published_at >= ?1)",
        [today_start],
        |row| {
            Ok(ViewCounts {
                unread: row.get(0)?,
                starred: row.get(1)?,
                today: row.get(2)?,
            })
        },
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::feeds::{self, NewFeed};
    use crate::store::migrations::{migrate, MIGRATIONS};

    const DAY: i64 = 86_400;

    fn item(guid: String, published_at: i64) -> NewArticle {
        NewArticle {
            guid,
            url: None,
            title: "t".into(),
            author: None,
            summary_html: None,
            content_html: None,
            published_at,
            updated_at: None,
            content_hash: "h".into(),
            enclosures: Vec::new(),
            thumbnail_url: None,
        }
    }

    /// A feed with `count` articles, one a day, the newest `now`. Returns (feed, ids newest first).
    fn seed(conn: &mut Connection, url: &str, count: i64, newest: i64) -> (i64, Vec<i64>) {
        let tx = conn.transaction().unwrap();
        let feed = feeds::insert(
            &tx,
            &NewFeed {
                url,
                title: url,
                custom_title: None,
                site_url: None,
                description: None,
                folder_id: None,
                now: newest,
            },
        )
        .unwrap();
        let items: Vec<NewArticle> = (0..count)
            .map(|i| item(format!("{url}-{i}"), newest - i * DAY))
            .collect();
        upsert(&tx, feed, &items, newest, false).unwrap();
        tx.commit().unwrap();
        let mut stmt = conn
            .prepare("SELECT id FROM articles WHERE feed_id = ?1 ORDER BY published_at DESC")
            .unwrap();
        let ids = stmt
            .query_map([feed], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<i64>, _>>()
            .unwrap();
        (feed, ids)
    }

    fn remaining(conn: &Connection, feed: i64) -> Vec<i64> {
        let mut stmt = conn
            .prepare("SELECT id FROM articles WHERE feed_id = ?1 ORDER BY published_at DESC")
            .unwrap();
        stmt.query_map([feed], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<i64>, _>>()
            .unwrap()
    }

    #[test]
    fn retention_keeps_starred_tagged_and_the_newest_fifty() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = Connection::open(dir.path().join("t.sqlite")).unwrap();
        migrate(&mut conn, MIGRATIONS, dir.path()).unwrap();
        let now = 100 * 365 * DAY;
        let (busy, ids) = seed(&mut conn, "https://busy.example/feed", 60, now);
        let (quiet, quiet_ids) = seed(&mut conn, "https://quiet.example/feed", 5, now - 200 * DAY);
        set_starred(&conn, ids[55], true, now).unwrap();
        conn.execute("INSERT INTO tags (id, name) VALUES (1, 'keep')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO article_tags (article_id, tag_id) VALUES (?1, 1)",
            [ids[56]],
        )
        .unwrap();

        // 30 days: articles 31–59 are expired, but 0–49 are the newest fifty.
        let deleted = delete_expired(&conn, now - 30 * DAY, 50).unwrap();
        assert_eq!(deleted, 8, "50–59 minus one starred and one tagged");
        let mut expected: Vec<i64> = ids[..50].to_vec();
        expected.extend([ids[55], ids[56]]);
        assert_eq!(remaining(&conn, busy), expected);
        assert_eq!(remaining(&conn, quiet), quiet_ids, "under fifty: all kept");

        assert_eq!(delete_expired(&conn, now - 30 * DAY, 50).unwrap(), 0);
    }

    #[test]
    fn set_read_handles_more_ids_than_sqlite_allows_in_one_statement() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = Connection::open(dir.path().join("t.sqlite")).unwrap();
        migrate(&mut conn, MIGRATIONS, dir.path()).unwrap();
        let (_, ids) = seed(&mut conn, "https://big.example/feed", 1_200, 1_000 * DAY);
        let mut many = ids.clone();
        many.extend(1_000_000..1_040_000);
        assert_eq!(set_read(&conn, &many, true, 5).unwrap(), 1_200);
        assert_eq!(set_read(&conn, &many, true, 5).unwrap(), 0);
    }
}
