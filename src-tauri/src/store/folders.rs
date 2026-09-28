//! Repository for `folders`. Deleting a folder moves its feeds to the top level
//! (`feeds.folder_id` is `ON DELETE SET NULL`).

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::error::{AppError, AppResult};
use crate::store::feeds::local_account_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderRow {
    pub id: i64,
    pub name: String,
}

pub fn list(conn: &Connection) -> AppResult<Vec<FolderRow>> {
    let mut stmt = conn
        .prepare_cached("SELECT id, name FROM folders ORDER BY sort_order, name COLLATE NOCASE")?;
    let rows = stmt
        .query_map([], |row| {
            Ok(FolderRow {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn clean_name(name: &str) -> AppResult<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::invalid_input("Folder names can't be empty"));
    }
    Ok(name)
}

fn map_duplicate(err: rusqlite::Error, name: &str) -> AppError {
    match err {
        rusqlite::Error::SqliteFailure(e, _)
            if e.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            AppError::conflict(format!("There's already a folder called \"{name}\""))
        }
        other => other.into(),
    }
}

/// Folder names are unique per account, ignoring case ("News" and "news" would be confusing).
fn ensure_unique(
    conn: &Connection,
    account_id: i64,
    name: &str,
    except: Option<i64>,
) -> AppResult<()> {
    let taken = conn
        .query_row(
            "SELECT 1 FROM folders WHERE account_id = ?1 AND name = ?2 COLLATE NOCASE
             AND id IS NOT ?3",
            params![account_id, name, except],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if taken {
        return Err(AppError::conflict(format!(
            "There's already a folder called \"{name}\""
        )));
    }
    Ok(())
}

pub fn create(conn: &Connection, name: &str) -> AppResult<i64> {
    let name = clean_name(name)?;
    let account_id = local_account_id(conn)?;
    ensure_unique(conn, account_id, name, None)?;
    conn.execute(
        "INSERT INTO folders (account_id, name, sort_order)
         VALUES (?1, ?2, (SELECT coalesce(max(sort_order) + 1, 0) FROM folders WHERE account_id = ?1))",
        params![account_id, name],
    )
    .map_err(|err| map_duplicate(err, name))?;
    Ok(conn.last_insert_rowid())
}

pub fn rename(conn: &Connection, id: i64, name: &str) -> AppResult<()> {
    let name = clean_name(name)?;
    ensure_unique(conn, local_account_id(conn)?, name, Some(id))?;
    let changed = conn
        .execute(
            "UPDATE folders SET name = ?2 WHERE id = ?1",
            params![id, name],
        )
        .map_err(|err| map_duplicate(err, name))?;
    if changed == 0 {
        return Err(AppError::not_found(format!("Folder {id} not found")));
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    let changed = conn.execute("DELETE FROM folders WHERE id = ?1", [id])?;
    if changed == 0 {
        return Err(AppError::not_found(format!("Folder {id} not found")));
    }
    Ok(())
}

pub fn exists(conn: &Connection, id: i64) -> AppResult<bool> {
    Ok(conn
        .query_row("SELECT 1 FROM folders WHERE id = ?1", [id], |_| Ok(()))
        .optional()?
        .is_some())
}

pub fn reorder(tx: &Transaction, folder_ids: &[i64]) -> AppResult<()> {
    let mut stmt = tx.prepare_cached("UPDATE folders SET sort_order = ?2 WHERE id = ?1")?;
    for (index, id) in folder_ids.iter().enumerate() {
        stmt.execute(params![id, index as i64])?;
    }
    Ok(())
}
