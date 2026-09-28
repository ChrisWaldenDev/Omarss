//! Repository for the `settings` table: raw key → JSON-text rows.

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppResult;

pub fn get_all(conn: &Connection) -> AppResult<Vec<(String, String)>> {
    let mut stmt = conn.prepare_cached("SELECT key, value FROM settings")?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()?)
}

/// Upserts every `(key, json_value)` pair in a single transaction.
pub fn set_many(conn: &mut Connection, entries: &[(String, String)]) -> AppResult<()> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        )?;
        for (key, value) in entries {
            stmt.execute(params![key, value])?;
        }
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::migrations::{migrate, MIGRATIONS};

    #[test]
    fn set_many_inserts_then_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = Connection::open(dir.path().join("t.sqlite")).unwrap();
        migrate(&mut conn, MIGRATIONS, dir.path()).unwrap();

        assert!(get_all(&conn).unwrap().is_empty());
        set_many(&mut conn, &[("theme".into(), "\"dark\"".into())]).unwrap();
        set_many(&mut conn, &[("theme".into(), "\"light\"".into())]).unwrap();
        assert_eq!(
            get_all(&conn).unwrap(),
            vec![("theme".to_string(), "\"light\"".to_string())]
        );
    }
}
