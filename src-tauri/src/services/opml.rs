//! OPML import and export (SPEC §6.4). Importing only adds subscriptions, which is quick; the
//! feeds are then fetched by the scheduler like any other due feed, with refresh progress.

use std::collections::HashMap;

use crate::clock;
use crate::error::{AppError, AppResult};
use crate::fetch::normalize_input_url;
use crate::models::ImportSummary;
use crate::opml::{self, ExportFeed, ExportFolder};
use crate::store::feeds::{self, NewFeed};
use crate::store::{folders, Store};

#[derive(Clone)]
pub struct OpmlService {
    store: Store,
}

impl OpmlService {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    /// Subscribes to every new feed in an OPML document, creating its folders. Feeds are due
    /// immediately, so the caller should wake the scheduler.
    pub async fn import(&self, bytes: Vec<u8>) -> AppResult<ImportSummary> {
        let outlines = opml::parse(&bytes).map_err(AppError::invalid_input)?;
        self.store
            .run(move |conn| {
                let tx = conn.transaction()?;
                let now = clock::now_unix();
                let mut summary = ImportSummary::default();
                let mut known = feeds::all_urls(&tx)?;
                let mut folder_ids: HashMap<String, i64> = folders::list(&tx)?
                    .into_iter()
                    .map(|f| (f.name.to_lowercase(), f.id))
                    .collect();
                for outline in outlines {
                    let Ok(url) = normalize_input_url(&outline.url) else {
                        summary.invalid += 1;
                        continue;
                    };
                    let url = url.to_string();
                    if !known.insert(url.clone()) {
                        summary.duplicates += 1;
                        continue;
                    }
                    let folder_id = match outline.folder.as_deref().map(str::trim) {
                        Some(name) if !name.is_empty() => {
                            match folder_ids.get(&name.to_lowercase()) {
                                Some(id) => Some(*id),
                                None => {
                                    let id = folders::create(&tx, name)?;
                                    folder_ids.insert(name.to_lowercase(), id);
                                    summary.folders += 1;
                                    Some(id)
                                }
                            }
                        }
                        _ => None,
                    };
                    // `title` is usually the feed's own title and `text` what the exporting app
                    // showed; a difference is a custom title worth keeping.
                    let title = outline
                        .title
                        .clone()
                        .or_else(|| outline.text.clone())
                        .unwrap_or_else(|| host_of(&url));
                    let custom_title = outline.text.filter(|text| *text != title);
                    let id = feeds::insert(
                        &tx,
                        &NewFeed {
                            url: &url,
                            title: &title,
                            custom_title: custom_title.as_deref(),
                            site_url: outline.site_url.as_deref(),
                            description: None,
                            folder_id,
                            now,
                        },
                    )?;
                    feeds::reschedule(&tx, id, Some(now))?;
                    summary.feeds += 1;
                }
                tx.commit()?;
                Ok(summary)
            })
            .await
    }

    /// An OPML 2.0 document with every feed, its folder and custom title.
    pub async fn export(&self) -> AppResult<String> {
        self.store
            .run(|conn| {
                let rows = feeds::export_rows(conn)?;
                let to_export = |row: &feeds::ExportRow| ExportFeed {
                    text: row
                        .custom_title
                        .clone()
                        .unwrap_or_else(|| row.title.clone()),
                    title: row.title.clone(),
                    url: row.url.clone(),
                    site_url: row.site_url.clone(),
                };
                let folders: Vec<ExportFolder> = folders::list(conn)?
                    .into_iter()
                    .map(|folder| ExportFolder {
                        feeds: rows
                            .iter()
                            .filter(|r| r.folder_id == Some(folder.id))
                            .map(to_export)
                            .collect(),
                        name: folder.name,
                    })
                    .collect();
                let root: Vec<ExportFeed> = rows
                    .iter()
                    .filter(|r| r.folder_id.is_none())
                    .map(to_export)
                    .collect();
                Ok(opml::export(
                    &folders,
                    &root,
                    &chrono::Utc::now().to_rfc2822(),
                ))
            })
            .await
    }
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
