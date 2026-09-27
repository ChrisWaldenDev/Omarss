use tauri::AppHandle;

use crate::error::{AppError, AppResult};
use crate::navigation;

/// Opens a link in the OS default browser or mail client (SPEC §6.2, §8.2).
#[tauri::command]
#[specta::specta]
pub async fn open_external(app: AppHandle, url: String) -> AppResult<()> {
    let url = tauri::Url::parse(&url)
        .map_err(|_| AppError::invalid_input(format!("Not a valid link: {url}")))?;
    navigation::open_external(&app, &url)
}
