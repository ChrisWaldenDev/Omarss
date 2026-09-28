//! Retention and storage upkeep (SPEC §6.9): old articles are deleted daily and after each
//! refresh that brought new ones, the query planner is kept tuned, and the database is
//! compacted weekly when the app is idle.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::clock;
use crate::error::AppResult;
use crate::models::StorageInfo;
use crate::services::images::ImageCache;
use crate::services::settings::SettingsService;
use crate::store::{articles, maintenance, Store};

/// Every feed keeps at least this many of its newest articles, whatever their age.
pub const KEEP_PER_FEED: u32 = 50;
/// First cleanup after start-up, so it doesn't compete with the initial refresh.
const FIRST_RUN_DELAY: Duration = Duration::from_secs(120);
const DAY: i64 = 86_400;
const VACUUM_EVERY: i64 = 7 * DAY;
/// While refreshing, a due vacuum is retried after this long.
const BUSY_RETRY: Duration = Duration::from_secs(600);
const LAST_VACUUM_KEY: &str = "lastVacuumAt";

#[derive(Clone)]
pub struct MaintenanceService {
    store: Store,
    settings: SettingsService,
    images: ImageCache,
    data_dir: Arc<PathBuf>,
    /// One cleanup or vacuum at a time.
    busy: Arc<tokio::sync::Mutex<()>>,
}

impl MaintenanceService {
    pub fn new(
        store: Store,
        settings: SettingsService,
        images: ImageCache,
        data_dir: PathBuf,
    ) -> Self {
        Self {
            store,
            settings,
            images,
            data_dir: Arc::new(data_dir),
            busy: Arc::default(),
        }
    }

    /// Deletes expired articles (keeping starred, tagged and each feed's newest 50), then runs
    /// `PRAGMA optimize`. Returns how many articles were deleted.
    pub async fn cleanup(&self) -> AppResult<usize> {
        let settings = self.settings.get().await?;
        let _busy = self.busy.lock().await;
        let retention_days = settings.retention_days;
        let deleted = self
            .store
            .run(move |conn| {
                let deleted = if retention_days == 0 {
                    0
                } else {
                    let cutoff = clock::now_unix() - i64::from(retention_days) * DAY;
                    articles::delete_expired(conn, cutoff, KEEP_PER_FEED)?
                };
                maintenance::optimize(conn)?;
                Ok(deleted)
            })
            .await?;
        if deleted > 0 {
            tracing::info!(deleted, "retention cleanup");
        }
        Ok(deleted)
    }

    /// "Compact database": `VACUUM` now.
    pub async fn compact(&self) -> AppResult<()> {
        let _busy = self.busy.lock().await;
        self.store
            .run(|conn| {
                maintenance::vacuum(conn)?;
                maintenance::optimize(conn)
            })
            .await?;
        self.settings
            .set_state(LAST_VACUUM_KEY, clock::now_unix().into())
            .await
    }

    pub async fn storage_info(&self) -> AppResult<StorageInfo> {
        let (database_bytes, article_count) = self
            .store
            .run(|conn| {
                Ok((
                    maintenance::database_bytes(conn)?,
                    maintenance::article_count(conn)?,
                ))
            })
            .await?;
        let images = self.images.clone();
        let image_cache_bytes = tauri::async_runtime::spawn_blocking(move || images.size_bytes())
            .await
            .unwrap_or(0);
        Ok(StorageInfo {
            database_bytes,
            image_cache_bytes,
            article_count,
            data_dir: self.data_dir.display().to_string(),
        })
    }

    pub async fn clear_image_cache(&self) -> AppResult<()> {
        let images = self.images.clone();
        tauri::async_runtime::spawn_blocking(move || images.clear())
            .await
            .map_err(|err| crate::error::AppError::internal(err.to_string()))??;
        Ok(())
    }

    /// Whether the weekly vacuum is due.
    async fn vacuum_due(&self) -> bool {
        let last = self
            .settings
            .get_state(LAST_VACUUM_KEY)
            .await
            .ok()
            .flatten()
            .and_then(|v| v.as_i64());
        match last {
            Some(at) => clock::now_unix() - at >= VACUUM_EVERY,
            None => {
                // First run: start the weekly clock rather than vacuuming a fresh database.
                let _ = self
                    .settings
                    .set_state(LAST_VACUUM_KEY, clock::now_unix().into())
                    .await;
                false
            }
        }
    }

    /// Runs cleanup shortly after start-up and then daily; vacuums weekly when `is_idle`.
    pub fn start(&self, is_idle: impl Fn() -> bool + Send + Sync + 'static) {
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(FIRST_RUN_DELAY).await;
            loop {
                if let Err(err) = this.cleanup().await {
                    tracing::warn!(%err, "retention cleanup failed");
                }
                let mut next_cleanup = Duration::from_secs(DAY as u64);
                while this.vacuum_due().await {
                    if is_idle() {
                        if let Err(err) = this.compact().await {
                            tracing::warn!(%err, "vacuum failed");
                        }
                        break;
                    }
                    tokio::time::sleep(BUSY_RETRY).await;
                    next_cleanup = next_cleanup.saturating_sub(BUSY_RETRY);
                }
                tokio::time::sleep(next_cleanup).await;
            }
        });
    }
}
