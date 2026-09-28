//! Subscription and folder commands (SPEC §6.1).

use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::error::AppResult;
use crate::events::ArticlesChanged;
use crate::models::{
    DiscoveredFeed, FeedDetails, FeedPreview, FeedUpdate, SidebarOrder, SubscribeRequest,
};
use crate::scheduler::Scheduler;
use crate::services::feeds::FeedService;

/// Finds the feeds behind a feed or website address.
#[tauri::command]
#[specta::specta]
pub async fn discover_feeds(
    service: State<'_, FeedService>,
    url: String,
) -> AppResult<Vec<DiscoveredFeed>> {
    service.discover(&url).await
}

#[tauri::command]
#[specta::specta]
pub async fn preview_feed(service: State<'_, FeedService>, url: String) -> AppResult<FeedPreview> {
    service.preview(&url).await
}

/// Subscribes and returns the new feed's id.
#[tauri::command]
#[specta::specta]
pub async fn subscribe_feed(
    app: AppHandle,
    service: State<'_, FeedService>,
    scheduler: State<'_, Scheduler>,
    request: SubscribeRequest,
) -> AppResult<i64> {
    let id = service.subscribe(request).await?;
    scheduler.reschedule();
    let _ = ArticlesChanged { new_count: 0 }.emit(&app);
    Ok(id)
}

#[tauri::command]
#[specta::specta]
pub async fn get_feed(service: State<'_, FeedService>, id: i64) -> AppResult<FeedDetails> {
    service.details(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_feed(
    service: State<'_, FeedService>,
    scheduler: State<'_, Scheduler>,
    id: i64,
    update: FeedUpdate,
) -> AppResult<FeedDetails> {
    let details = service.update(id, update).await?;
    scheduler.reschedule();
    Ok(details)
}

/// Unsubscribes; starred articles are kept under "Deleted feeds".
#[tauri::command]
#[specta::specta]
pub async fn unsubscribe_feed(service: State<'_, FeedService>, id: i64) -> AppResult<()> {
    service.unsubscribe(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_folder(service: State<'_, FeedService>, name: String) -> AppResult<i64> {
    service.create_folder(name).await
}

#[tauri::command]
#[specta::specta]
pub async fn rename_folder(
    service: State<'_, FeedService>,
    id: i64,
    name: String,
) -> AppResult<()> {
    service.rename_folder(id, name).await
}

/// Deletes a folder; its feeds move to the top level.
#[tauri::command]
#[specta::specta]
pub async fn delete_folder(service: State<'_, FeedService>, id: i64) -> AppResult<()> {
    service.delete_folder(id).await
}

/// Saves the sidebar order after a drag and drop.
#[tauri::command]
#[specta::specta]
pub async fn reorder_sidebar(
    service: State<'_, FeedService>,
    order: SidebarOrder,
) -> AppResult<()> {
    service.reorder(order).await
}
