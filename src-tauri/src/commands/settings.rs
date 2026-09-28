use tauri::{AppHandle, Manager, Runtime, State};

use crate::error::AppResult;
use crate::fetch::{HttpClient, PROJECT_URL};
use crate::models::AppInfo;
use crate::network;
use crate::paths::AppPaths;
use crate::scheduler::Scheduler;
use crate::services::images::ImageCache;
use crate::services::settings::{Settings, SettingsService};

/// The biggest custom stylesheet loaded (SPEC §6.8).
const MAX_CUSTOM_CSS_BYTES: u64 = 512 * 1024;

#[tauri::command]
#[specta::specta]
pub async fn get_settings(service: State<'_, SettingsService>) -> AppResult<Settings> {
    service.get().await
}

/// Saves the full settings object and returns what was stored. Changes apply straight away
/// (SPEC §11.2).
#[tauri::command]
#[specta::specta]
pub async fn update_settings(
    app: AppHandle,
    service: State<'_, SettingsService>,
    http: State<'_, HttpClient>,
    images: State<'_, ImageCache>,
    scheduler: State<'_, Scheduler>,
    settings: Settings,
) -> AppResult<Settings> {
    let before = service.get().await?;
    // Check the proxy before saving, so a bad one is never stored.
    if settings.proxy_url != before.proxy_url {
        http.set_proxy(Some(&settings.proxy_url))?;
    }
    let saved = match service.update(settings).await {
        Ok(saved) => saved,
        Err(err) => {
            let _ = http.set_proxy(Some(&before.proxy_url));
            return Err(err);
        }
    };
    app.set_theme(saved.theme.to_window_theme());
    if saved.ui_scale != before.ui_scale {
        apply_ui_scale(&app, saved.ui_scale);
    }
    if saved.image_cache_mb != before.image_cache_mb {
        images.set_max_mb(saved.image_cache_mb);
    }
    if saved.refresh_interval_minutes != before.refresh_interval_minutes
        || saved.pause_on_metered != before.pause_on_metered
    {
        scheduler.reschedule();
    }
    Ok(saved)
}

/// Interface scale (SPEC §6.8) as native WebView zoom, so layout reflows like browser zoom.
pub fn apply_ui_scale<R: Runtime>(app: &AppHandle<R>, percent: u32) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(err) = window.set_zoom(f64::from(percent) / 100.0) {
            tracing::warn!(%err, "could not apply the interface scale");
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_app_info(app: AppHandle, paths: State<'_, AppPaths>) -> AppResult<AppInfo> {
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        platform: std::env::consts::OS.to_string(),
        homepage: PROJECT_URL.to_string(),
        data_dir: paths.data_dir.display().to_string(),
        config_dir: paths.config_dir.display().to_string(),
        metered_supported: network::METERED_SUPPORTED,
    })
}

/// The user's `custom.css` from the config folder, if there is one (SPEC §6.8).
#[tauri::command]
#[specta::specta]
pub async fn get_custom_css(paths: State<'_, AppPaths>) -> AppResult<Option<String>> {
    let path = paths.custom_css();
    tauri::async_runtime::spawn_blocking(move || {
        let Ok(meta) = std::fs::metadata(&path) else {
            return Ok(None);
        };
        if meta.len() > MAX_CUSTOM_CSS_BYTES {
            tracing::warn!("custom.css is too big; ignoring it");
            return Ok(None);
        }
        Ok(Some(std::fs::read_to_string(&path)?))
    })
    .await
    .map_err(|err| crate::error::AppError::internal(err.to_string()))?
}
