//! Database upkeep (SPEC §6.9): statistics, `PRAGMA optimize` and `VACUUM`.

use rusqlite::Connection;

use crate::error::AppResult;

/// Size of the database in bytes (pages in use and free).
pub fn database_bytes(conn: &Connection) -> AppResult<u64> {
    let pages: i64 = conn.pragma_query_value(None, "page_count", |row| row.get(0))?;
    let page_size: i64 = conn.pragma_query_value(None, "page_size", |row| row.get(0))?;
    Ok(u64::try_from(pages * page_size).unwrap_or(0))
}

pub fn article_count(conn: &Connection) -> AppResult<u32> {
    Ok(conn.query_row("SELECT count(*) FROM articles", [], |row| row.get(0))?)
}

pub fn optimize(conn: &Connection) -> AppResult<()> {
    conn.execute_batch("PRAGMA optimize;")?;
    Ok(())
}

/// Rebuilds the database file to reclaim free pages, then truncates the WAL.
pub fn vacuum(conn: &Connection) -> AppResult<()> {
    conn.execute_batch("VACUUM; PRAGMA wal_checkpoint(TRUNCATE);")?;
    Ok(())
}
