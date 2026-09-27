use std::fmt;

use serde::Serialize;
use specta::Type;

pub type AppResult<T> = Result<T, AppError>;

/// Broad category of an [`AppError`], so the frontend can react without parsing messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Database,
    Io,
    NotFound,
    InvalidInput,
    Internal,
}

/// The error type returned by every public backend function (SPEC §4.1).
///
/// Serialises to the frontend as `{ kind, message }`, where `message` is readable by users.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
}

impl AppError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn database(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Database, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, message)
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidInput, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        Self::database(format!("Database error: {err}"))
    }
}

impl From<r2d2::Error> for AppError {
    fn from(err: r2d2::Error) -> Self {
        Self::database(format!("Could not get a database connection: {err}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::new(ErrorKind::Io, format!("File system error: {err}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::internal(format!("Serialisation error: {err}"))
    }
}

impl From<tauri::Error> for AppError {
    fn from(err: tauri::Error) -> Self {
        Self::internal(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_as_kind_and_message() {
        let err = AppError::not_found("Article 7 not found");
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "kind": "not_found", "message": "Article 7 not found" })
        );
    }

    #[test]
    fn converts_sqlite_errors_to_database_kind() {
        let err: AppError = rusqlite::Error::QueryReturnedNoRows.into();
        assert_eq!(err.kind, ErrorKind::Database);
        assert!(!err.message.is_empty());
    }
}
