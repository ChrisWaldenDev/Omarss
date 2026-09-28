//! Storage locations (SPEC §11.1).
//!
//! Base directories come from Tauri's path resolver (which honours `XDG_*` on Linux), with an
//! `omarss` sub-directory, e.g. `~/.local/share/omarss` or `%LOCALAPPDATA%\omarss`.

use std::path::PathBuf;

use tauri::{Manager, Runtime};

use crate::error::AppResult;

pub const APP_DIR_NAME: &str = "omarss";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub db_file: PathBuf,
    pub backups_dir: PathBuf,
    pub log_dir: PathBuf,
    /// Cached favicons (SPEC §7.5).
    pub icons_dir: PathBuf,
    /// The image proxy's cache (SPEC §8.4).
    pub image_cache_dir: PathBuf,
    /// User configuration, e.g. `custom.css` (SPEC §6.8).
    pub config_dir: PathBuf,
}

impl AppPaths {
    pub fn resolve<R: Runtime>(app: &impl Manager<R>) -> AppResult<Self> {
        Ok(Self::in_dirs(
            app.path().local_data_dir()?.join(APP_DIR_NAME),
            app.path().config_dir()?.join(APP_DIR_NAME),
        ))
    }

    fn in_dirs(data_dir: PathBuf, config_dir: PathBuf) -> Self {
        Self {
            db_file: data_dir.join("omarss.sqlite"),
            backups_dir: data_dir.join("backups"),
            log_dir: data_dir.join("logs"),
            icons_dir: data_dir.join("icons"),
            image_cache_dir: data_dir.join("images"),
            data_dir,
            config_dir,
        }
    }

    /// The user's theme overrides (SPEC §6.8).
    pub fn custom_css(&self) -> PathBuf {
        self.config_dir.join("custom.css")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_lives_under_the_data_dir() {
        let root = PathBuf::from("data").join(APP_DIR_NAME);
        let config = PathBuf::from("config").join(APP_DIR_NAME);
        let paths = AppPaths::in_dirs(root.clone(), config.clone());
        for p in [
            &paths.db_file,
            &paths.backups_dir,
            &paths.log_dir,
            &paths.icons_dir,
            &paths.image_cache_dir,
        ] {
            assert!(p.starts_with(&root));
        }
        assert!(paths.custom_css().starts_with(&config));
    }
}
