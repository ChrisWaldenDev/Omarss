//! Typed app settings backed by the `settings` table (SPEC §11.2).
//!
//! Each field of [`Settings`] is stored as its own row (`key` = field name, `value` = JSON).
//! Missing, unknown or invalid rows fall back to the field's default, so adding a setting
//! never needs a migration and a bad value never prevents startup.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use specta::Type;

use crate::error::{AppError, AppResult};
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

/// Three panes side by side, or two with the list expanding into the reader (SPEC §6.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Layout {
    #[default]
    ThreePane,
    TwoPane,
}

/// When an article counts as read (SPEC §6.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MarkReadMode {
    #[default]
    OnOpen,
    /// After it has been open for `markReadDelaySeconds`.
    AfterDelay,
    /// When it scrolls past the top of the list (and when opened).
    OnScroll,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ReaderFont {
    #[default]
    System,
    Sans,
    Serif,
}

/// "Load remote images" (SPEC §8.4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum LoadImages {
    /// Everywhere, including list thumbnails.
    #[default]
    Always,
    /// Only in articles you open (no list thumbnails).
    Opened,
    /// Never automatically; the reader offers a button to load them.
    Never,
}

/// Global refresh interval choices in minutes (SPEC §6.5); `0` = manual only.
pub const REFRESH_INTERVALS: [u32; 6] = [15, 30, 60, 120, 360, 0];
/// Retention choices in days (SPEC §6.9); `0` = forever.
pub const RETENTION_DAYS: [u32; 5] = [30, 90, 180, 365, 0];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: Theme,
    /// Global refresh interval in minutes; `0` means manual refresh only (SPEC §6.5).
    pub refresh_interval_minutes: u32,
    pub refresh_on_startup: bool,
    /// Mark an article unread again when its feed updates it (SPEC §7.4).
    pub mark_updated_unread: bool,
    /// Skip automatic refresh on metered connections (SPEC §7.2; Windows only).
    pub pause_on_metered: bool,

    pub layout: Layout,
    pub mark_read_mode: MarkReadMode,
    pub mark_read_delay_seconds: u32,
    pub list_thumbnails: bool,

    pub reader_font: ReaderFont,
    /// Pixels.
    pub reader_font_size: u32,
    /// Maximum line length in characters.
    pub reader_line_width: u32,
    /// Percent of the font size.
    pub reader_line_height: u32,

    /// `#rrggbb`, or `None` for the theme's own accent (SPEC §6.8).
    pub accent_color: Option<String>,
    /// Percent, 80–150 (SPEC §6.8).
    pub ui_scale: u32,
    /// Pane widths in pixels (SPEC §6.8: resizable and persisted).
    pub sidebar_width: u32,
    pub list_width: u32,

    /// Keep articles this many days; `0` keeps them forever (SPEC §6.9).
    pub retention_days: u32,
    pub load_images: LoadImages,
    /// Remove `utm_*` parameters from article links (SPEC §8.4).
    pub strip_tracking_params: bool,
    /// Image cache limit in megabytes (SPEC §8.4).
    pub image_cache_mb: u32,
    /// Manual `http://`, `https://` or `socks5://` proxy; empty uses the system proxy (SPEC §7.1).
    pub proxy_url: String,

    /// Rebound keyboard shortcuts: action id → keys. Actions not listed use their defaults
    /// (SPEC §6.7).
    pub shortcuts: BTreeMap<String, Vec<String>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            refresh_interval_minutes: 30,
            refresh_on_startup: true,
            mark_updated_unread: false,
            pause_on_metered: false,
            layout: Layout::default(),
            mark_read_mode: MarkReadMode::default(),
            mark_read_delay_seconds: 3,
            list_thumbnails: true,
            reader_font: ReaderFont::default(),
            reader_font_size: 17,
            reader_line_width: 70,
            reader_line_height: 165,
            accent_color: None,
            ui_scale: 100,
            sidebar_width: 240,
            list_width: 360,
            retention_days: 90,
            load_images: LoadImages::default(),
            strip_tracking_params: true,
            image_cache_mb: 500,
            proxy_url: String::new(),
            shortcuts: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// Rejects values the settings UI never offers, with a message saying what's wrong.
    pub fn validate(&self) -> AppResult<()> {
        let range = |value: u32, min: u32, max: u32, what: &str| {
            if (min..=max).contains(&value) {
                Ok(())
            } else {
                Err(AppError::invalid_input(format!(
                    "{what} must be between {min} and {max}"
                )))
            }
        };
        if !REFRESH_INTERVALS.contains(&self.refresh_interval_minutes) {
            return Err(AppError::invalid_input("Unsupported refresh interval"));
        }
        if !RETENTION_DAYS.contains(&self.retention_days) {
            return Err(AppError::invalid_input("Unsupported retention period"));
        }
        range(
            self.mark_read_delay_seconds,
            1,
            60,
            "The mark-as-read delay",
        )?;
        range(self.reader_font_size, 12, 32, "The font size")?;
        range(self.reader_line_width, 40, 120, "The line width")?;
        range(self.reader_line_height, 120, 220, "The line height")?;
        range(self.ui_scale, 80, 150, "The interface scale")?;
        range(self.sidebar_width, 160, 600, "The sidebar width")?;
        range(self.list_width, 220, 900, "The list width")?;
        range(self.image_cache_mb, 50, 10_000, "The image cache size")?;
        if let Some(color) = &self.accent_color {
            if !is_hex_color(color) {
                return Err(AppError::invalid_input(
                    "The accent colour must look like #1f5bc4",
                ));
            }
        }
        if !self.proxy_url.trim().is_empty() {
            validate_proxy_url(&self.proxy_url)?;
        }
        for keys in self.shortcuts.values() {
            if keys.len() > 4 || keys.iter().any(|k| k.is_empty() || k.len() > 40) {
                return Err(AppError::invalid_input("Invalid keyboard shortcut"));
            }
        }
        Ok(())
    }

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
        let settings: Settings = serde_json::from_value(Value::Object(merged)).unwrap_or_default();
        // A value that parses but is out of range (e.g. edited by hand) falls back to its
        // default field by field, like an unparsable one.
        settings.repair()
    }

    fn repair(mut self) -> Self {
        if self.validate().is_ok() {
            return self;
        }
        let defaults = Settings::default();
        macro_rules! reset_if_invalid {
            ($($field:ident),*) => {$(
                let candidate = Settings { $field: self.$field.clone(), ..defaults.clone() };
                if candidate.validate().is_err() {
                    tracing::warn!(key = stringify!($field), "ignoring out-of-range setting");
                    self.$field = defaults.$field.clone();
                }
            )*};
        }
        reset_if_invalid!(
            refresh_interval_minutes,
            retention_days,
            mark_read_delay_seconds,
            reader_font_size,
            reader_line_width,
            reader_line_height,
            ui_scale,
            sidebar_width,
            list_width,
            image_cache_mb,
            accent_color,
            proxy_url,
            shortcuts
        );
        self
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

fn is_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn validate_proxy_url(value: &str) -> AppResult<url::Url> {
    let url = url::Url::parse(value.trim())
        .ok()
        .filter(|u| {
            matches!(u.scheme(), "http" | "https" | "socks5" | "socks5h") && u.host_str().is_some()
        })
        .ok_or_else(|| {
            AppError::invalid_input(
                "The proxy must be an address like http://host:8080 or socks5://host:1080",
            )
        })?;
    Ok(url)
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
        settings.validate()?;
        let rows = settings.to_rows();
        self.store
            .run(move |conn| store::settings::set_many(conn, &rows))
            .await?;
        Ok(settings)
    }

    /// Internal app state kept beside the settings (e.g. when maintenance last ran). Keys start
    /// with `state.` so they never collide with a setting.
    pub async fn get_state(&self, key: &'static str) -> AppResult<Option<Value>> {
        self.store
            .run(move |conn| {
                Ok(store::settings::get(conn, &format!("state.{key}"))?
                    .and_then(|raw| serde_json::from_str(&raw).ok()))
            })
            .await
    }

    pub async fn set_state(&self, key: &'static str, value: Value) -> AppResult<()> {
        let rows = vec![(format!("state.{key}"), value.to_string())];
        self.store
            .run(move |conn| store::settings::set_many(conn, &rows))
            .await
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
        let stored = settings.to_rows();
        let json = serde_json::to_value(&settings).unwrap();
        assert_eq!(stored.len(), json.as_object().unwrap().len());
        for expected in rows(&[
            ("markUpdatedUnread", "false"),
            ("refreshIntervalMinutes", "30"),
            ("refreshOnStartup", "true"),
            ("theme", "\"light\""),
            ("layout", "\"threePane\""),
            ("loadImages", "\"always\""),
            ("accentColor", "null"),
            ("shortcuts", "{}"),
        ]) {
            assert!(stored.contains(&expected), "{expected:?} missing");
        }
    }

    #[test]
    fn update_rejects_values_the_ui_never_offers() {
        let (_dir, svc) = service();
        for bad in [
            Settings {
                refresh_interval_minutes: 7,
                ..Settings::default()
            },
            Settings {
                retention_days: 12,
                ..Settings::default()
            },
            Settings {
                ui_scale: 300,
                ..Settings::default()
            },
            Settings {
                accent_color: Some("red".into()),
                ..Settings::default()
            },
            Settings {
                proxy_url: "ftp://proxy".into(),
                ..Settings::default()
            },
        ] {
            let err = block_on(svc.update(bad)).unwrap_err();
            assert_eq!(err.kind, crate::error::ErrorKind::InvalidInput);
        }
        let good = Settings {
            accent_color: Some("#12abEF".into()),
            proxy_url: "socks5://127.0.0.1:1080".into(),
            ..Settings::default()
        };
        assert_eq!(block_on(svc.update(good.clone())).unwrap(), good);
    }

    #[test]
    fn out_of_range_stored_values_fall_back_field_by_field() {
        let parsed = Settings::from_rows(rows(&[
            ("uiScale", "999"),
            ("readerFontSize", "20"),
            ("retentionDays", "13"),
        ]));
        assert_eq!(parsed.ui_scale, 100);
        assert_eq!(parsed.retention_days, 90);
        assert_eq!(parsed.reader_font_size, 20);
    }

    #[test]
    fn state_rows_do_not_leak_into_settings() {
        let (_dir, svc) = service();
        block_on(svc.set_state("lastVacuumAt", serde_json::json!(42))).unwrap();
        assert_eq!(
            block_on(svc.get_state("lastVacuumAt")).unwrap(),
            Some(serde_json::json!(42))
        );
        assert_eq!(block_on(svc.get()).unwrap(), Settings::default());
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
