//! Typed app settings backed by the `settings` table (SPEC §11.2).
//!
//! Each field of [`Settings`] is stored as its own row (`key` = field name, `value` = JSON).
//! Missing, unknown or invalid rows fall back to the field's default, so adding a setting
//! never needs a migration and a bad value never prevents startup.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use specta::Type;

use crate::error::AppResult;
use crate::store::{self, Store};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

impl Theme {
    /// The native window theme; `None` follows the OS.
    pub fn to_window_theme(self) -> Option<tauri::Theme> {
        match self {
            Theme::Light => Some(tauri::Theme::Light),
            Theme::Dark => Some(tauri::Theme::Dark),
            Theme::System => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: Theme,
    /// Global refresh interval in minutes; `0` means manual refresh only (SPEC §6.5).
    pub refresh_interval_minutes: u32,
    pub refresh_on_startup: bool,
    /// Mark an article unread again when its feed updates it (SPEC §7.4).
    pub mark_updated_unread: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            refresh_interval_minutes: 30,
            refresh_on_startup: true,
            mark_updated_unread: false,
        }
    }
}

impl Settings {
    fn from_rows(rows: Vec<(String, String)>) -> Self {
        let defaults = to_map(&Settings::default());
        let mut merged = defaults.clone();
        for (key, raw) in rows {
            if !defaults.contains_key(&key) {
                continue; // Unknown key, e.g. written by a newer version.
            }
            let Ok(value) = serde_json::from_str::<Value>(&raw) else {
                tracing::warn!(%key, "ignoring unparsable setting");
                continue;
            };
            let mut candidate = merged.clone();
            candidate.insert(key.clone(), value);
            if serde_json::from_value::<Settings>(Value::Object(candidate.clone())).is_ok() {
                merged = candidate;
            } else {
                tracing::warn!(%key, "ignoring invalid setting value");
            }
        }
        serde_json::from_value(Value::Object(merged)).unwrap_or_default()
    }

    fn to_rows(&self) -> Vec<(String, String)> {
        to_map(self)
            .into_iter()
            .map(|(key, value)| (key, value.to_string()))
            .collect()
    }
}

fn to_map(settings: &Settings) -> Map<String, Value> {
    match serde_json::to_value(settings) {
        Ok(Value::Object(map)) => map,
        _ => unreachable!("Settings always serialises to a JSON object"),
    }
}

#[derive(Clone)]
pub struct SettingsService {
    store: Store,
}

impl SettingsService {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    pub async fn get(&self) -> AppResult<Settings> {
        self.store
            .run(|conn| Ok(Settings::from_rows(store::settings::get_all(conn)?)))
            .await
    }

    /// Persists all settings and returns what was stored.
    pub async fn update(&self, settings: Settings) -> AppResult<Settings> {
        let rows = settings.to_rows();
        self.store
            .run(move |conn| store::settings::set_many(conn, &rows))
            .await?;
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::async_runtime::block_on;

    fn service() -> (tempfile::TempDir, SettingsService) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("t.sqlite"), dir.path()).unwrap();
        (dir, SettingsService::new(store))
    }

    fn rows(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn empty_table_yields_defaults() {
        let (_dir, svc) = service();
        assert_eq!(block_on(svc.get()).unwrap(), Settings::default());
        assert_eq!(Settings::default().theme, Theme::System);
    }

    #[test]
    fn update_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("t.sqlite");
        let svc = SettingsService::new(Store::open(&db, dir.path()).unwrap());
        let wanted = Settings {
            theme: Theme::Dark,
            refresh_interval_minutes: 60,
            ..Settings::default()
        };
        assert_eq!(block_on(svc.update(wanted.clone())).unwrap(), wanted);
        drop(svc);

        let reopened = SettingsService::new(Store::open(&db, dir.path()).unwrap());
        assert_eq!(block_on(reopened.get()).unwrap(), wanted);
    }

    #[test]
    fn stores_one_json_row_per_field() {
        let settings = Settings {
            theme: Theme::Light,
            ..Settings::default()
        };
        let mut stored = settings.to_rows();
        stored.sort();
        assert_eq!(
            stored,
            rows(&[
                ("markUpdatedUnread", "false"),
                ("refreshIntervalMinutes", "30"),
                ("refreshOnStartup", "true"),
                ("theme", "\"light\""),
            ])
        );
    }

    #[test]
    fn invalid_or_unknown_rows_fall_back_to_defaults() {
        let parsed =
            Settings::from_rows(rows(&[("theme", "\"purple\""), ("fromTheFuture", "true")]));
        assert_eq!(parsed, Settings::default());

        let parsed = Settings::from_rows(rows(&[("theme", "not json")]));
        assert_eq!(parsed, Settings::default());

        let parsed = Settings::from_rows(rows(&[("theme", "\"dark\"")]));
        assert_eq!(parsed.theme, Theme::Dark);
    }
}
