//! Subscriptions, fetching and ingest (SPEC §6.1, §7).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::clock;
use crate::content::content_hash;
use crate::error::{AppError, AppResult};
use crate::feed::{self, ParsedFeed, ParsedItem};
use crate::fetch::{
    discovery, favicon, normalize_input_url, HttpClient, Request, Response, FEED_ACCEPT,
    FEED_MAX_BYTES,
};
use crate::models::{
    DiscoveredFeed, Enclosure, FeedDetails, FeedPreview, FeedUpdate, PreviewItem, RefreshTarget,
    SidebarOrder, SubscribeRequest,
};
use crate::scheduler::{base_interval, next_after_error, next_after_success, MIN_AUTO_INTERVAL};
use crate::services::settings::{Settings, SettingsService};
use crate::store::articles::{self, NewArticle};
use crate::store::feeds::{self, FetchSuccess, NewFeed, SettingsUpdate};
use crate::store::{folders, Store};

/// Favicons are re-fetched weekly (SPEC §7.5)…
const ICON_MAX_AGE: i64 = 7 * 86_400;
/// …and a failed attempt isn't retried for a day.
const ICON_RETRY_AFTER: i64 = 86_400;
/// Dates further in the future than this are treated as missing (broken feed clocks).
const MAX_FUTURE_SKEW: i64 = 86_400;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RefreshOutcome {
    pub new_articles: usize,
    pub updated_articles: usize,
    pub error: Option<RefreshFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshFailure {
    pub message: String,
    /// Couldn't connect at all. Not recorded here; the scheduler decides whether the feed or
    /// the network is at fault.
    pub connection: bool,
}

impl RefreshOutcome {
    fn failed(message: impl Into<String>, connection: bool) -> Self {
        Self {
            error: Some(RefreshFailure {
                message: message.into(),
                connection,
            }),
            ..Self::default()
        }
    }
}

#[derive(Clone)]
pub struct FeedService {
    store: Store,
    http: HttpClient,
    settings: SettingsService,
    icons_dir: Arc<PathBuf>,
    icon_attempts: Arc<Mutex<HashMap<i64, i64>>>,
}

impl FeedService {
    pub fn new(
        store: Store,
        http: HttpClient,
        settings: SettingsService,
        icons_dir: PathBuf,
    ) -> Self {
        Self {
            store,
            http,
            settings,
            icons_dir: Arc::new(icons_dir),
            icon_attempts: Arc::default(),
        }
    }

    pub fn icons_dir(&self) -> &std::path::Path {
        &self.icons_dir
    }

    pub async fn discover(&self, input: &str) -> AppResult<Vec<DiscoveredFeed>> {
        let url = normalize_input_url(input)?;
        discovery::discover(&self.http, &url).await
    }

    async fn fetch_feed(&self, input: &str) -> AppResult<(Response, ParsedFeed)> {
        let url = normalize_input_url(input)?;
        let response = self
            .http
            .get(Request::new(url.as_str(), FEED_ACCEPT, FEED_MAX_BYTES))
            .await?;
        if !response.is_success() {
            return Err(AppError::network(format!(
                "{} responded with HTTP {}",
                url.host_str().unwrap_or("The server"),
                response.status
            )));
        }
        let parsed = feed::parse(
            &response.body,
            response.content_type.as_deref(),
            &response.url,
        )
        .map_err(AppError::invalid_feed)?;
        Ok((response, parsed))
    }

    /// Fetches a feed without subscribing: its title and latest five items (SPEC §6.1).
    pub async fn preview(&self, url: &str) -> AppResult<FeedPreview> {
        let (response, parsed) = self.fetch_feed(url).await?;
        let mut items = parsed.items;
        items.sort_by_key(|item| std::cmp::Reverse(item.published_at));
        Ok(FeedPreview {
            title: parsed
                .title
                .unwrap_or_else(|| host_of(&response.permanent_url)),
            site_url: parsed.site_url,
            items: items
                .into_iter()
                .take(5)
                .map(|item| PreviewItem {
                    title: item.title,
                    url: item.url,
                    published_at: item.published_at,
                })
                .collect(),
            url: response.permanent_url,
        })
    }

    /// Subscribes and stores the feed's current articles; the favicon follows in the
    /// background. Returns the new feed's id.
    pub async fn subscribe(&self, request: SubscribeRequest) -> AppResult<i64> {
        let (response, parsed) = self.fetch_feed(&request.url).await?;
        let settings = self.settings.get().await?;
        let now = clock::now_unix();
        let next_fetch_at = next_after_success(
            base_interval(None, &settings),
            &[
                parsed.update_hint_secs,
                response.max_age,
                response.retry_after,
            ],
            now,
        );
        let title = parsed
            .title
            .clone()
            .unwrap_or_else(|| host_of(&response.permanent_url));
        let custom_title = request
            .title
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty() && *t != title);
        let items = to_new_articles(&parsed.items, now);
        let feed_url = response.permanent_url.clone();
        let site_url = parsed.site_url.clone();
        let description = feed::short_description(parsed.description.as_deref());
        let folder_id = request.folder_id;

        let id = self
            .store
            .run(move |conn| {
                let tx = conn.transaction()?;
                if let Some(folder) = folder_id {
                    if !folders::exists(&tx, folder)? {
                        return Err(AppError::invalid_input("That folder no longer exists"));
                    }
                }
                let id = feeds::insert(
                    &tx,
                    &NewFeed {
                        url: &response.permanent_url,
                        title: &title,
                        custom_title: custom_title.as_deref(),
                        site_url: site_url.as_deref(),
                        description: description.as_deref(),
                        folder_id,
                        now,
                    },
                )?;
                feeds::record_success(
                    &tx,
                    id,
                    &FetchSuccess {
                        new_url: None,
                        etag: response.etag.as_deref(),
                        last_modified: response.last_modified.as_deref(),
                        title: None,
                        site_url: None,
                        description: None,
                        fetched_at: now,
                        next_fetch_at,
                    },
                )?;
                articles::upsert(&tx, id, &items, now, false)?;
                tx.commit()?;
                Ok(id)
            })
            .await?;

        let this = self.clone();
        let (icon_url, site_url) = (parsed.icon_url, parsed.site_url);
        tauri::async_runtime::spawn(async move {
            this.refresh_icon(id, None, icon_url, site_url, feed_url)
                .await;
        });
        Ok(id)
    }

    pub async fn due(&self, now: i64) -> AppResult<Vec<(i64, String)>> {
        self.store.run(move |conn| feeds::due(conn, now)).await
    }

    pub async fn next_due_at(&self) -> AppResult<Option<i64>> {
        self.store.run(|conn| feeds::next_due_at(conn)).await
    }

    /// The feeds a set of manual refresh requests covers, without duplicates.
    pub async fn jobs_for(&self, targets: &[RefreshTarget]) -> AppResult<Vec<(i64, String)>> {
        let targets = targets.to_vec();
        self.store
            .run(move |conn| {
                let mut jobs: Vec<(i64, String)> = Vec::new();
                for target in targets {
                    let found = match target {
                        RefreshTarget::All => feeds::refreshable(conn, None)?,
                        RefreshTarget::Folder { id } => feeds::refreshable(conn, Some(id))?,
                        RefreshTarget::Feed { id } => match feeds::get(conn, id) {
                            Ok(feed) if !feed.is_deleted_feeds() => vec![(feed.id, feed.url)],
                            _ => Vec::new(),
                        },
                    };
                    for job in found {
                        if !jobs.iter().any(|(id, _)| *id == job.0) {
                            jobs.push(job);
                        }
                    }
                }
                Ok(jobs)
            })
            .await
    }

    /// Fetches one feed and stores what's new (SPEC §7.1–§7.4). Connection failures are
    /// returned rather than recorded, so the scheduler can detect being offline.
    pub async fn refresh_feed(&self, id: i64, settings: &Settings) -> AppResult<RefreshOutcome> {
        let feed = self.store.run(move |conn| feeds::get(conn, id)).await?;
        if feed.is_deleted_feeds() {
            return Ok(RefreshOutcome::default());
        }
        let base = base_interval(feed.fetch_interval, settings);
        let request = Request {
            url: &feed.url,
            accept: FEED_ACCEPT,
            max_bytes: FEED_MAX_BYTES,
            etag: feed.etag.as_deref(),
            last_modified: feed.last_modified.as_deref(),
        };
        let response = match self.http.get(request).await {
            Ok(response) => response,
            Err(err) if err.connection => return Ok(RefreshOutcome::failed(err.message, true)),
            Err(err) => return self.fail(id, err.message, None, false, settings).await,
        };

        match response.status {
            410 => {
                return self
                    .fail(
                        id,
                        "The feed no longer exists (HTTP 410). Refreshing is paused.".into(),
                        None,
                        true,
                        settings,
                    )
                    .await
            }
            429 | 503 => {
                let message = format!("The server asked to slow down (HTTP {})", response.status);
                return self
                    .fail(id, message, response.retry_after, false, settings)
                    .await;
            }
            _ => {}
        }

        let now = clock::now_unix();
        let new_url = (response.permanent_url != feed.url).then(|| response.permanent_url.clone());
        if response.is_not_modified() {
            let next_fetch_at =
                next_after_success(base, &[response.max_age, response.retry_after], now);
            let (etag, last_modified) = (response.etag.clone(), response.last_modified.clone());
            self.store
                .run(move |conn| {
                    feeds::record_success(
                        conn,
                        id,
                        &FetchSuccess {
                            new_url: new_url.as_deref(),
                            etag: etag.as_deref(),
                            last_modified: last_modified.as_deref(),
                            title: None,
                            site_url: None,
                            description: None,
                            fetched_at: now,
                            next_fetch_at,
                        },
                    )
                })
                .await?;
            self.refresh_icon(
                id,
                feed.icon_path.clone(),
                None,
                feed.site_url.clone(),
                feed.url.clone(),
            )
            .await;
            return Ok(RefreshOutcome::default());
        }
        if !response.is_success() {
            let message = format!("The server responded with HTTP {}", response.status);
            return self.fail(id, message, None, false, settings).await;
        }

        let parsed = match feed::parse(
            &response.body,
            response.content_type.as_deref(),
            &response.url,
        ) {
            Ok(parsed) => parsed,
            Err(message) => return self.fail(id, message, None, false, settings).await,
        };
        let next_fetch_at = next_after_success(
            base,
            &[
                parsed.update_hint_secs,
                response.max_age,
                response.retry_after,
            ],
            now,
        );
        let items = to_new_articles(&parsed.items, now);
        let mark_unread = settings.mark_updated_unread;
        let (title, site_url) = (parsed.title.clone(), parsed.site_url.clone());
        let description = feed::short_description(parsed.description.as_deref());
        let (etag, last_modified) = (response.etag.clone(), response.last_modified.clone());
        let stats = self
            .store
            .run(move |conn| {
                let tx = conn.transaction()?;
                feeds::record_success(
                    &tx,
                    id,
                    &FetchSuccess {
                        new_url: new_url.as_deref(),
                        etag: etag.as_deref(),
                        last_modified: last_modified.as_deref(),
                        title: title.as_deref(),
                        site_url: site_url.as_deref(),
                        description: description.as_deref(),
                        fetched_at: now,
                        next_fetch_at,
                    },
                )?;
                let stats = articles::upsert(&tx, id, &items, now, mark_unread)?;
                tx.commit()?;
                Ok(stats)
            })
            .await?;
        self.refresh_icon(
            id,
            feed.icon_path.clone(),
            parsed.icon_url,
            parsed.site_url.or(feed.site_url),
            feed.url.clone(),
        )
        .await;
        Ok(RefreshOutcome {
            new_articles: stats.inserted,
            updated_articles: stats.updated,
            error: None,
        })
    }

    async fn fail(
        &self,
        id: i64,
        message: String,
        retry_after: Option<i64>,
        pause: bool,
        settings: &Settings,
    ) -> AppResult<RefreshOutcome> {
        self.record_failure(id, &message, retry_after, pause, settings)
            .await?;
        Ok(RefreshOutcome::failed(message, false))
    }

    /// Records a failed fetch and schedules the retry with backoff (SPEC §7.2).
    pub async fn record_failure(
        &self,
        id: i64,
        message: &str,
        retry_after: Option<i64>,
        pause: bool,
        settings: &Settings,
    ) -> AppResult<()> {
        let message = message.to_string();
        let settings = settings.clone();
        self.store
            .run(move |conn| {
                let feed = feeds::get(conn, id)?;
                let now = clock::now_unix();
                let next = if pause {
                    None
                } else {
                    next_after_error(
                        base_interval(feed.fetch_interval, &settings),
                        feed.error_count + 1,
                        retry_after,
                        now,
                    )
                };
                feeds::record_error(conn, id, &message, now, next, pause)
            })
            .await
    }

    /// Moves a feed's next fetch without counting an error (used while offline).
    pub async fn defer(&self, id: i64, at: i64) -> AppResult<()> {
        self.store
            .run(move |conn| feeds::reschedule(conn, id, Some(at)))
            .await
    }

    /// Fetches the favicon when there is none or it's a week old, at most once a day.
    async fn refresh_icon(
        &self,
        id: i64,
        current: Option<String>,
        feed_icon: Option<String>,
        site_url: Option<String>,
        feed_url: String,
    ) {
        let now = clock::now_unix();
        let fresh = current
            .as_deref()
            .and_then(icon_timestamp)
            .is_some_and(|at| now - at < ICON_MAX_AGE);
        if fresh {
            return;
        }
        {
            let Ok(mut attempts) = self.icon_attempts.lock() else {
                return;
            };
            if attempts
                .get(&id)
                .is_some_and(|at| now - at < ICON_RETRY_AFTER)
            {
                return;
            }
            attempts.insert(id, now);
        }
        let Some(png) = favicon::fetch_icon(
            &self.http,
            feed_icon.as_deref(),
            site_url.as_deref(),
            &feed_url,
        )
        .await
        else {
            return;
        };
        let name = format!("{id}-{now}.png");
        let dir = self.icons_dir.clone();
        let result = self
            .store
            .run(move |conn| {
                std::fs::create_dir_all(dir.as_ref())?;
                std::fs::write(dir.join(&name), &png)?;
                feeds::set_icon(conn, id, Some(&name))?;
                if let Some(old) = current.filter(|old| *old != name) {
                    let _ = std::fs::remove_file(dir.join(old));
                }
                Ok(())
            })
            .await;
        if let Err(err) = result {
            tracing::warn!(feed = id, %err, "could not save favicon");
        }
    }

    pub async fn details(&self, id: i64) -> AppResult<FeedDetails> {
        self.store
            .run(move |conn| Ok(feeds::get(conn, id)?.details()))
            .await
    }

    /// Edit feed (SPEC §6.1): custom title, folder, refresh interval, pause.
    pub async fn update(&self, id: i64, update: FeedUpdate) -> AppResult<FeedDetails> {
        if let Some(secs) = update.fetch_interval {
            if secs != 0 && secs < MIN_AUTO_INTERVAL {
                return Err(AppError::invalid_input(
                    "Feeds can't refresh more often than every 10 minutes",
                ));
            }
        }
        let settings = self.settings.get().await?;
        self.store
            .run(move |conn| {
                let tx = conn.transaction()?;
                let feed = feeds::get(&tx, id)?;
                if let Some(folder) = update.folder_id {
                    if !folders::exists(&tx, folder)? {
                        return Err(AppError::invalid_input("That folder no longer exists"));
                    }
                }
                let now = clock::now_unix();
                let next_fetch_at =
                    base_interval(update.fetch_interval, &settings).map(|interval| {
                        feed.last_fetched_at
                            .map_or(now, |last| last + interval.max(MIN_AUTO_INTERVAL))
                            .max(now)
                    });
                let custom_title = update
                    .custom_title
                    .as_deref()
                    .map(str::trim)
                    .filter(|t| !t.is_empty() && *t != feed.title);
                feeds::update_settings(
                    &tx,
                    id,
                    &SettingsUpdate {
                        custom_title,
                        folder_id: update.folder_id,
                        fetch_interval: update.fetch_interval,
                        paused: update.paused,
                        next_fetch_at,
                    },
                )?;
                let details = feeds::get(&tx, id)?.details();
                tx.commit()?;
                Ok(details)
            })
            .await
    }

    /// Unsubscribe (SPEC §6.1): starred articles move to "Deleted feeds".
    pub async fn unsubscribe(&self, id: i64) -> AppResult<()> {
        let dir = self.icons_dir.clone();
        self.store
            .run(move |conn| {
                let tx = conn.transaction()?;
                let icon = feeds::delete_keeping_starred(&tx, id, clock::now_unix())?;
                tx.commit()?;
                if let Some(icon) = icon {
                    let _ = std::fs::remove_file(dir.join(icon));
                }
                Ok(())
            })
            .await
    }

    pub async fn create_folder(&self, name: String) -> AppResult<i64> {
        self.store
            .run(move |conn| folders::create(conn, &name))
            .await
    }

    pub async fn rename_folder(&self, id: i64, name: String) -> AppResult<()> {
        self.store
            .run(move |conn| folders::rename(conn, id, &name))
            .await
    }

    /// Deletes a folder; its feeds move to the top level.
    pub async fn delete_folder(&self, id: i64) -> AppResult<()> {
        self.store.run(move |conn| folders::delete(conn, id)).await
    }

    pub async fn reorder(&self, order: SidebarOrder) -> AppResult<()> {
        self.store
            .run(move |conn| {
                let tx = conn.transaction()?;
                folders::reorder(&tx, &order.folders)?;
                feeds::reorder(&tx, &order.feeds)?;
                tx.commit()?;
                Ok(())
            })
            .await
    }

    #[cfg(test)]
    pub async fn record(&self, id: i64) -> AppResult<feeds::FeedRecord> {
        self.store.run(move |conn| feeds::get(conn, id)).await
    }
}

/// Icon files are named `<feed id>-<unix time>.png`; the time says when it was fetched.
fn icon_timestamp(name: &str) -> Option<i64> {
    name.strip_suffix(".png")?.split_once('-')?.1.parse().ok()
}

fn host_of(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.host_str()
                .map(|h| h.trim_start_matches("www.").to_string())
        })
        .unwrap_or_else(|| url.to_string())
}

/// Parsed items → rows. Missing or absurdly future dates become the time first seen.
fn to_new_articles(items: &[ParsedItem], now: i64) -> Vec<NewArticle> {
    items
        .iter()
        .map(|item| NewArticle {
            guid: item.guid.clone(),
            url: item.url.clone(),
            title: item.title.clone(),
            author: item.author.clone(),
            summary_html: item.summary_html.clone(),
            content_html: item.content_html.clone(),
            published_at: item
                .published_at
                .filter(|at| *at <= now + MAX_FUTURE_SKEW)
                .unwrap_or(now),
            updated_at: item.updated_at,
            content_hash: content_hash(
                &item.title,
                item.summary_html.as_deref(),
                item.content_html.as_deref(),
            ),
            enclosures: item
                .enclosures
                .iter()
                .map(|e| Enclosure {
                    url: e.url.clone(),
                    mime_type: e.mime_type.clone(),
                    length: e.length,
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_names_carry_their_fetch_time() {
        assert_eq!(icon_timestamp("12-1790000000.png"), Some(1_790_000_000));
        assert_eq!(icon_timestamp("12.png"), None);
        assert_eq!(icon_timestamp("garbage"), None);
    }

    #[test]
    fn future_and_missing_dates_become_first_seen() {
        let item = |published_at| ParsedItem {
            guid: "g".into(),
            url: None,
            title: "t".into(),
            author: None,
            summary_html: None,
            content_html: None,
            published_at,
            updated_at: None,
            enclosures: Vec::new(),
        };
        let now = 1_000_000;
        let rows = to_new_articles(
            &[item(None), item(Some(now + 10 * 86_400)), item(Some(5))],
            now,
        );
        let dates: Vec<i64> = rows.iter().map(|r| r.published_at).collect();
        assert_eq!(dates, [now, now, 5]);
    }
}
