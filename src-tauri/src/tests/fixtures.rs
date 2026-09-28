//! Helpers shared by the tests: fixture files, a temporary app store and services.

use std::path::PathBuf;

use crate::fetch::HttpClient;
use crate::services::articles::ArticleService;
use crate::services::feeds::FeedService;
use crate::services::settings::SettingsService;
use crate::store::Store;

pub fn feed_fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/tests/fixtures/feeds")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

pub fn feed_fixture_names() -> Vec<String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/tests/fixtures/feeds");
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// A fresh database and the services the app builds on it.
pub struct TestApp {
    pub _dir: tempfile::TempDir,
    pub store: Store,
    pub feeds: FeedService,
    pub articles: ArticleService,
}

impl TestApp {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(
            &dir.path().join("omarss.sqlite"),
            &dir.path().join("backups"),
        )
        .unwrap();
        let settings = SettingsService::new(store.clone());
        let http = HttpClient::new("test").unwrap();
        let feeds = FeedService::new(store.clone(), http, settings, dir.path().join("icons"));
        let articles = ArticleService::new(store.clone());
        Self {
            _dir: dir,
            store,
            feeds,
            articles,
        }
    }

    pub fn icons_dir(&self) -> PathBuf {
        self._dir.path().join("icons")
    }
}
