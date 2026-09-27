//! Embedded, numbered SQL migrations (SPEC §5).
//!
//! Each migration runs in its own transaction together with the `PRAGMA user_version` bump,
//! so a crash mid-migration leaves the database at the previous version.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// All migrations, in order. Versions must be 1, 2, 3, … with no gaps.
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "0001_init",
    sql: include_str!("../../migrations/0001_init.sql"),
}];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MigrationReport {
    pub from_version: u32,
    pub to_version: u32,
    /// Copy of the database taken before migrating, if the database already had a schema.
    pub backup: Option<PathBuf>,
}

pub fn schema_version(conn: &Connection) -> AppResult<u32> {
    Ok(conn.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

/// Brings the database up to the latest version in `migrations`.
///
/// When the database already has a schema and there is something to apply, a backup is written
/// to `<backup_dir>/pre-migration-<next version>.sqlite` first.
pub fn migrate(
    conn: &mut Connection,
    migrations: &[Migration],
    backup_dir: &Path,
) -> AppResult<MigrationReport> {
    let current = schema_version(conn)?;
    let latest = migrations.last().map_or(0, |m| m.version);
    let mut report = MigrationReport {
        from_version: current,
        to_version: current,
        backup: None,
    };

    if current > latest {
        return Err(AppError::database(format!(
            "This database was created by a newer version of Omarss (schema v{current}; \
             this version supports up to v{latest}). Please update Omarss."
        )));
    }
    if current == latest {
        return Ok(report);
    }

    if current > 0 {
        report.backup = Some(backup(conn, backup_dir, current + 1)?);
    }

    for migration in migrations.iter().filter(|m| m.version > current) {
        tracing::info!(name = migration.name, "applying database migration");
        let tx = conn.transaction()?;
        tx.execute_batch(migration.sql).map_err(|err| {
            AppError::database(format!("Migration {} failed: {err}", migration.name))
        })?;
        tx.pragma_update(None, "user_version", migration.version)?;
        tx.commit()?;
        report.to_version = migration.version;
    }

    let violations: i64 =
        conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    if violations > 0 {
        return Err(AppError::database(format!(
            "Database migration left {violations} foreign key violation(s)"
        )));
    }

    Ok(report)
}

fn backup(conn: &Connection, backup_dir: &Path, next_version: u32) -> AppResult<PathBuf> {
    std::fs::create_dir_all(backup_dir)?;
    let path = backup_dir.join(format!("pre-migration-{next_version}.sqlite"));
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    let target = path
        .to_str()
        .ok_or_else(|| AppError::internal("Backup path is not valid UTF-8"))?;
    // VACUUM INTO writes a consistent copy even while the database is in WAL mode.
    conn.execute("VACUUM INTO ?1", [target])?;
    tracing::info!(path = %path.display(), "backed up database before migration");
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(dir: &Path) -> Connection {
        Connection::open(dir.join("test.sqlite")).unwrap()
    }

    fn table_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    const EXTRA: &[Migration] = &[
        Migration {
            version: 1,
            name: "0001_one",
            sql: "CREATE TABLE one (id INTEGER PRIMARY KEY);",
        },
        Migration {
            version: 2,
            name: "0002_two",
            sql: "CREATE TABLE two (id INTEGER PRIMARY KEY);",
        },
    ];

    #[test]
    fn versions_are_sequential_from_one() {
        for (i, m) in MIGRATIONS.iter().enumerate() {
            assert_eq!(
                m.version as usize,
                i + 1,
                "migration {} out of order",
                m.name
            );
            assert!(m.name.starts_with(&format!("{:04}_", m.version)));
        }
    }

    #[test]
    fn fresh_database_gets_full_schema_without_backup() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = open(dir.path());
        let report = migrate(&mut conn, MIGRATIONS, &dir.path().join("backups")).unwrap();

        assert_eq!(report.from_version, 0);
        assert_eq!(report.to_version, MIGRATIONS.len() as u32);
        assert_eq!(report.backup, None);
        assert!(!dir.path().join("backups").exists());

        let tables = table_names(&conn);
        for expected in [
            "accounts",
            "folders",
            "feeds",
            "articles",
            "enclosures",
            "tags",
            "article_tags",
            "rules",
            "pending_actions",
            "settings",
            "articles_fts",
        ] {
            assert!(tables.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn creates_exactly_one_local_account() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = open(dir.path());
        migrate(&mut conn, MIGRATIONS, dir.path()).unwrap();
        let kinds: Vec<String> = conn
            .prepare("SELECT kind FROM accounts")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(kinds, vec!["local".to_string()]);
    }

    #[test]
    fn fts5_table_is_usable() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = open(dir.path());
        migrate(&mut conn, MIGRATIONS, dir.path()).unwrap();
        conn.execute(
            "INSERT INTO articles_fts (rowid, title, author, body) VALUES (1, 'Café async', 'A', 'b')",
            [],
        )
        .unwrap();
        // remove_diacritics: "cafe" matches "Café".
        let hit: i64 = conn
            .query_row(
                "SELECT rowid FROM articles_fts WHERE articles_fts MATCH 'cafe'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hit, 1);
    }

    #[test]
    fn rerunning_is_a_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = open(dir.path());
        migrate(&mut conn, MIGRATIONS, dir.path()).unwrap();
        let report = migrate(&mut conn, MIGRATIONS, dir.path()).unwrap();
        assert_eq!(report.from_version, report.to_version);
        assert_eq!(report.backup, None);
    }

    #[test]
    fn upgrading_an_existing_database_backs_it_up_first() {
        let dir = tempfile::tempdir().unwrap();
        let backups = dir.path().join("backups");
        let mut conn = open(dir.path());
        migrate(&mut conn, &EXTRA[..1], &backups).unwrap();
        conn.execute("INSERT INTO one (id) VALUES (42)", [])
            .unwrap();

        let report = migrate(&mut conn, EXTRA, &backups).unwrap();
        assert_eq!((report.from_version, report.to_version), (1, 2));
        let backup_path = report.backup.expect("backup should be taken");
        assert_eq!(backup_path, backups.join("pre-migration-2.sqlite"));

        let backup = Connection::open(&backup_path).unwrap();
        assert_eq!(schema_version(&backup).unwrap(), 1);
        let id: i64 = backup
            .query_row("SELECT id FROM one", [], |row| row.get(0))
            .unwrap();
        assert_eq!(id, 42);
        assert!(!table_names(&backup).contains(&"two".to_string()));
    }

    #[test]
    fn failed_migration_rolls_back_and_keeps_version() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = open(dir.path());
        migrate(&mut conn, &EXTRA[..1], dir.path()).unwrap();

        let broken = [
            EXTRA[0],
            Migration {
                version: 2,
                name: "0002_broken",
                sql: "CREATE TABLE half (id INTEGER); THIS IS NOT SQL;",
            },
        ];
        let err = migrate(&mut conn, &broken, dir.path()).unwrap_err();
        assert!(err.message.contains("0002_broken"));
        assert_eq!(schema_version(&conn).unwrap(), 1);
        assert!(!table_names(&conn).contains(&"half".to_string()));
    }

    #[test]
    fn refuses_a_database_from_a_newer_version() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = open(dir.path());
        conn.pragma_update(None, "user_version", 99).unwrap();
        let err = migrate(&mut conn, MIGRATIONS, dir.path()).unwrap_err();
        assert!(err.message.contains("newer version"));
        assert_eq!(schema_version(&conn).unwrap(), 99);
    }
}
