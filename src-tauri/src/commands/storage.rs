//! Settings → Storage (SPEC §6.9).

use tauri::State;

use crate::error::AppResult;
use crate::models::StorageInfo;
use crate::services::maintenance::MaintenanceService;

#[tauri::command]
#[specta::specta]
pub async fn get_storage_info(service: State<'_, MaintenanceService>) -> AppResult<StorageInfo> {
    service.storage_info().await
}

#[tauri::command]
#[specta::specta]
pub async fn clear_image_cache(service: State<'_, MaintenanceService>) -> AppResult<()> {
    service.clear_image_cache().await
}

/// Deletes expired articles and compacts the database.
#[tauri::command]
#[specta::specta]
pub async fn compact_database(service: State<'_, MaintenanceService>) -> AppResult<()> {
    service.cleanup().await?;
    service.compact().await
}
