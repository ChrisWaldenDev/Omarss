//! SQLite storage: connection pool, migrations and repositories.
//!
//! All SQL in the app lives under this module (SPEC §4.1).

pub mod migrations;
pub mod settings;

use std::path::Path;

use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;

use crate::error::{AppError, AppResult};

const POOL_SIZE: u32 = 8;

/// Handle to the app database. Cheap to clone.
#[derive(Clone)]
pub struct Store {
    pool: r2d2::Pool<SqliteConnectionManager>,
}

impl Store {
    /// Opens (creating if needed) the database at `db_path`, migrates it to the latest schema,
    /// and returns a pooled handle.
    pub fn open(db_path: &Path, backup_dir: &Path) -> AppResult<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Migrate on a dedicated connection before any pooled connection exists.
        // Foreign keys stay off here (the SQLite default) so migrations may rebuild tables.
        let mut conn = Connection::open(db_path)?;
        let mode: String =
            conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            tracing::warn!(%mode, "could not enable WAL journal mode");
        }
        let report = migrations::migrate(&mut conn, migrations::MIGRATIONS, backup_dir)?;
        if report.from_version != report.to_version {
            tracing::info!(
                from = report.from_version,
                to = report.to_version,
                "database migrated"
            );
        }
        drop(conn);

        let manager = SqliteConnectionManager::file(db_path).with_init(|conn| {
            conn.execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA busy_timeout = 5000;",
            )
        });
        let pool = r2d2::Pool::builder().max_size(POOL_SIZE).build(manager)?;
        Ok(Self { pool })
    }

    /// Runs `f` with a pooled connection on a blocking thread, so async commands never block
    /// the async runtime on disk I/O.
    pub async fn run<T, F>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&mut Connection) -> AppResult<T> + Send + 'static,
        T: Send + 'static,
    {
        let pool = self.pool.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let mut conn = pool.get()?;
            f(&mut conn)
        })
        .await
        .map_err(|err| AppError::internal(format!("Database task failed: {err}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_a_migrated_wal_database_with_foreign_keys() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("nested").join("omarss.sqlite");
        let store = Store::open(&db, &dir.path().join("backups")).unwrap();
        assert!(db.exists());

        let (mode, fks, version) = tauri::async_runtime::block_on(store.run(|conn| {
            let mode: String = conn.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
            let fks: i64 = conn.pragma_query_value(None, "foreign_keys", |r| r.get(0))?;
            Ok((mode, fks, migrations::schema_version(conn)?))
        }))
        .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
        assert_eq!(fks, 1);
        assert_eq!(version, migrations::MIGRATIONS.len() as u32);
    }

    #[test]
    fn reopening_an_up_to_date_database_takes_no_backup() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("omarss.sqlite");
        let backups = dir.path().join("backups");
        drop(Store::open(&db, &backups).unwrap());
        drop(Store::open(&db, &backups).unwrap());
        assert!(!backups.exists());
    }
}
