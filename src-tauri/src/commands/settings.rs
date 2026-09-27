use tauri::{AppHandle, State};

use crate::error::AppResult;
use crate::services::settings::{Settings, SettingsService};

#[tauri::command]
#[specta::specta]
pub async fn get_settings(service: State<'_, SettingsService>) -> AppResult<Settings> {
    service.get().await
}

/// Saves the full settings object and returns what was stored.
#[tauri::command]
#[specta::specta]
pub async fn update_settings(
    app: AppHandle,
    service: State<'_, SettingsService>,
    settings: Settings,
) -> AppResult<Settings> {
    let saved = service.update(settings).await?;
    app.set_theme(saved.theme.to_window_theme());
    Ok(saved)
}
