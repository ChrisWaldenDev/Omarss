//! The refresh scheduler (SPEC §6.5, §7.2): one task that fetches due feeds with global and
//! per-host concurrency limits, backs off on errors, and waits while the machine is offline.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Runtime};
use tauri_specta::Event;
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinSet;

use crate::clock;
use crate::events::{ArticlesChanged, FeedError, RefreshDone, RefreshProgress};
use crate::models::{RefreshStatus, RefreshTarget};
use crate::services::feeds::FeedService;
use crate::services::settings::{Settings, SettingsService};

pub const GLOBAL_CONCURRENCY: usize = 8;
pub const PER_HOST_CONCURRENCY: usize = 2;
/// Automatic refresh never runs more often than this (SPEC §6.5).
pub const MIN_AUTO_INTERVAL: i64 = 600;
/// Error backoff never waits longer than this (SPEC §7.2).
pub const MAX_BACKOFF: i64 = 86_400;
/// How often to check whether the network is back while offline (SPEC §7.2).
pub const OFFLINE_RETRY: i64 = 60;
/// Longest the loop sleeps before re-checking for due feeds.
const MAX_IDLE: i64 = 3_600;

/// The refresh interval for a feed: its override, else the global setting. `None` means
/// manual refresh only (override `0`, or the global "manual only" setting).
pub fn base_interval(feed_override: Option<i64>, settings: &Settings) -> Option<i64> {
    match feed_override {
        Some(secs) if secs <= 0 => None,
        Some(secs) => Some(secs),
        None if settings.refresh_interval_minutes == 0 => None,
        None => Some(i64::from(settings.refresh_interval_minutes) * 60),
    }
}

/// When to fetch again after a success. Publisher hints (RSS `ttl`, `sy:update*`,
/// `Cache-Control: max-age`, `Retry-After`) only apply when longer than the interval.
pub fn next_after_success(base: Option<i64>, hints: &[Option<i64>], now: i64) -> Option<i64> {
    let base = base?;
    let hint = hints.iter().flatten().copied().max().unwrap_or(0);
    Some(now + base.max(hint).max(MIN_AUTO_INTERVAL))
}

/// When to retry after the `error_count`-th consecutive error:
/// interval × 2^min(error_count, 5), capped at 24 h, and never before `Retry-After`.
pub fn next_after_error(
    base: Option<i64>,
    error_count: u32,
    retry_after: Option<i64>,
    now: i64,
) -> Option<i64> {
    let base = base?;
    let backoff = base
        .saturating_mul(1_i64 << error_count.min(5))
        .min(MAX_BACKOFF);
    Some(now + backoff.max(retry_after.unwrap_or(0)).max(MIN_AUTO_INTERVAL))
}

/// `None` just wakes the loop to re-check due times (after a subscribe or an edit).
type Message = Option<RefreshTarget>;

#[derive(Clone)]
pub struct Scheduler {
    tx: mpsc::UnboundedSender<Message>,
    status: Arc<Mutex<RefreshStatus>>,
}

impl Scheduler {
    /// Starts the scheduler task. It refreshes everything once at start-up when the
    /// "refresh on startup" setting is on.
    pub fn start<R: Runtime>(
        app: AppHandle<R>,
        feeds: FeedService,
        settings: SettingsService,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let status = Arc::new(Mutex::new(RefreshStatus::default()));
        let scheduler = Self {
            tx,
            status: status.clone(),
        };
        tauri::async_runtime::spawn(run(app, feeds, settings, rx, status));
        scheduler
    }

    pub fn request(&self, target: RefreshTarget) {
        let _ = self.tx.send(Some(target));
    }

    /// Re-checks when the next feed is due, e.g. after subscribing or changing an interval.
    pub fn reschedule(&self) {
        let _ = self.tx.send(None);
    }

    pub fn status(&self) -> RefreshStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

async fn run<R: Runtime>(
    app: AppHandle<R>,
    feeds: FeedService,
    settings: SettingsService,
    mut rx: mpsc::UnboundedReceiver<Message>,
    status: Arc<Mutex<RefreshStatus>>,
) {
    let mut queued: Vec<RefreshTarget> = Vec::new();
    if settings.get().await.map_or(true, |s| s.refresh_on_startup) {
        queued.push(RefreshTarget::All);
    }
    loop {
        while let Ok(message) = rx.try_recv() {
            queued.extend(message);
        }
        let manual = !queued.is_empty();
        let jobs = if manual {
            feeds.jobs_for(&std::mem::take(&mut queued)).await
        } else {
            feeds.due(clock::now_unix()).await
        };
        let jobs = match jobs {
            Ok(jobs) => jobs,
            Err(err) => {
                tracing::error!(%err, "could not load feeds to refresh");
                Vec::new()
            }
        };
        if manual && jobs.is_empty() {
            // Nothing to fetch (no feeds, or all paused): still tell the UI we're done.
            finish(&app, &status, 0, 0, false, false);
        }
        if jobs.is_empty() {
            let now = clock::now_unix();
            let wait = match feeds.next_due_at().await {
                Ok(Some(at)) => (at - now).clamp(1, MAX_IDLE),
                _ => MAX_IDLE,
            };
            tokio::select! {
                message = rx.recv() => match message {
                    Some(message) => queued.extend(message),
                    None => return,
                },
                _ = tokio::time::sleep(Duration::from_secs(wait as u64)) => {}
            }
            continue;
        }
        let snapshot = settings.get().await.unwrap_or_default();
        run_batch(&app, &feeds, &snapshot, &status, jobs).await;
    }
}

fn update_status(status: &Mutex<RefreshStatus>, f: impl FnOnce(&mut RefreshStatus)) {
    if let Ok(mut status) = status.lock() {
        f(&mut status);
    }
}

pub(crate) async fn run_batch<R: Runtime>(
    app: &AppHandle<R>,
    feeds: &FeedService,
    settings: &Settings,
    status: &Mutex<RefreshStatus>,
    jobs: Vec<(i64, String)>,
) {
    let total = u32::try_from(jobs.len()).unwrap_or(u32::MAX);
    update_status(status, |s| {
        s.running = true;
        s.done = 0;
        s.total = total;
    });
    let _ = RefreshProgress { done: 0, total }.emit(app);

    let global = Arc::new(Semaphore::new(GLOBAL_CONCURRENCY));
    let mut hosts: HashMap<String, Arc<Semaphore>> = HashMap::new();
    let mut tasks = JoinSet::new();
    for (id, url) in jobs {
        let host = url::Url::parse(&url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_default();
        let host_slots = hosts
            .entry(host.clone())
            .or_insert_with(|| Arc::new(Semaphore::new(PER_HOST_CONCURRENCY)))
            .clone();
        let global = global.clone();
        let feeds = feeds.clone();
        let settings = settings.clone();
        tasks.spawn(async move {
            // Take the host slot first so a busy host doesn't hold global slots while waiting.
            let _host = host_slots.acquire_owned().await;
            let _slot = global.acquire_owned().await;
            (id, host, feeds.refresh_feed(id, &settings).await)
        });
    }

    let mut done = 0;
    let mut new_count = 0;
    let mut changed = false;
    let mut errors = 0;
    let mut successes = 0;
    let mut unreachable: Vec<(i64, String, String)> = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        done += 1;
        update_status(status, |s| s.done = done);
        let _ = RefreshProgress { done, total }.emit(app);
        let (id, host, result) = match joined {
            Ok(result) => result,
            Err(err) => {
                tracing::error!(%err, "refresh task failed");
                errors += 1;
                continue;
            }
        };
        match result {
            Ok(outcome) => {
                new_count += outcome.new_articles;
                changed |= outcome.new_articles + outcome.updated_articles > 0;
                match outcome.error {
                    None => successes += 1,
                    Some(failure) if failure.connection => {
                        unreachable.push((id, host, failure.message))
                    }
                    Some(failure) => {
                        errors += 1;
                        let _ = FeedError {
                            feed_id: id,
                            message: failure.message,
                        }
                        .emit(app);
                    }
                }
            }
            Err(err) => {
                errors += 1;
                tracing::warn!(feed = id, %err, "refresh failed");
            }
        }
    }

    // Offline: nothing succeeded, and every attempt (across at least two hosts) failed to even
    // connect. Then it's the network, not the feeds: don't count errors, retry soon.
    let failed_hosts: HashSet<&str> = unreachable.iter().map(|(_, h, _)| h.as_str()).collect();
    let offline = successes == 0 && errors == 0 && failed_hosts.len() >= 2;
    let now = clock::now_unix();
    for (id, _, message) in &unreachable {
        let result = if offline {
            feeds.defer(*id, now + OFFLINE_RETRY).await
        } else {
            errors += 1;
            let _ = FeedError {
                feed_id: *id,
                message: message.clone(),
            }
            .emit(app);
            feeds
                .record_failure(*id, message, None, false, settings)
                .await
        };
        if let Err(err) = result {
            tracing::warn!(feed = id, %err, "could not record refresh failure");
        }
    }
    if offline {
        tracing::info!("all feeds unreachable; treating the machine as offline");
    }

    let new_count = u32::try_from(new_count).unwrap_or(u32::MAX);
    finish(app, status, new_count, errors, offline, changed);
}

fn finish<R: Runtime>(
    app: &AppHandle<R>,
    status: &Mutex<RefreshStatus>,
    new_count: u32,
    errors: u32,
    offline: bool,
    changed: bool,
) {
    update_status(status, |s| {
        s.running = false;
        s.offline = offline;
        s.last_finished_at = Some(clock::now_unix());
    });
    let _ = RefreshDone {
        new_count,
        errors,
        offline,
    }
    .emit(app);
    if changed {
        let _ = ArticlesChanged { new_count }.emit(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(minutes: u32) -> Settings {
        Settings {
            refresh_interval_minutes: minutes,
            ..Settings::default()
        }
    }

    #[test]
    fn interval_comes_from_override_then_global() {
        assert_eq!(base_interval(None, &settings(30)), Some(1800));
        assert_eq!(base_interval(Some(900), &settings(30)), Some(900));
        assert_eq!(
            base_interval(Some(0), &settings(30)),
            None,
            "0 = manual only"
        );
        assert_eq!(
            base_interval(None, &settings(0)),
            None,
            "global manual only"
        );
        assert_eq!(base_interval(Some(3600), &settings(0)), Some(3600));
    }

    #[test]
    fn hints_only_lengthen_the_interval() {
        let now = 1_000;
        assert_eq!(next_after_success(Some(1800), &[], now), Some(now + 1800));
        assert_eq!(
            next_after_success(Some(1800), &[Some(600), None], now),
            Some(now + 1800)
        );
        assert_eq!(
            next_after_success(Some(1800), &[Some(7200), Some(3600)], now),
            Some(now + 7200)
        );
        assert_eq!(next_after_success(None, &[Some(7200)], now), None);
    }

    #[test]
    fn never_refreshes_more_often_than_every_ten_minutes() {
        assert_eq!(
            next_after_success(Some(60), &[], 0),
            Some(MIN_AUTO_INTERVAL)
        );
        assert_eq!(
            next_after_error(Some(60), 0, None, 0),
            Some(MIN_AUTO_INTERVAL)
        );
    }

    #[test]
    fn backoff_doubles_up_to_32x_and_caps_at_a_day() {
        let base = Some(1800);
        let delays: Vec<i64> = (1..=7)
            .map(|n| next_after_error(base, n, None, 0).unwrap())
            .collect();
        assert_eq!(delays, [3600, 7200, 14400, 28800, 57600, 57600, 57600]);
        assert_eq!(
            next_after_error(Some(21_600), 5, None, 0),
            Some(MAX_BACKOFF)
        );
        assert_eq!(
            next_after_error(Some(1800), 1, Some(10_000), 0),
            Some(10_000)
        );
        assert_eq!(next_after_error(None, 3, Some(10_000), 0), None);
    }
}
