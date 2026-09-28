//! OPML import/export (SPEC §6.4). The file pickers are opened from Rust, so the WebView gets
//! no file-system or dialog permissions (SPEC §11.4).

use std::path::PathBuf;

use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tokio::sync::oneshot;

use crate::error::{AppError, AppResult};
use crate::models::ImportSummary;
use crate::scheduler::Scheduler;
use crate::services::opml::OpmlService;

/// OPML files bigger than this aren't subscription lists.
const MAX_OPML_BYTES: u64 = 20 * 1024 * 1024;
const OPML_EXTENSIONS: [&str; 2] = ["opml", "xml"];

fn to_path(file: FilePath) -> AppResult<PathBuf> {
    file.into_path()
        .map_err(|err| AppError::invalid_input(format!("Can't use that file: {err}")))
}

/// A file dialog attached to the main window, so it opens over the app and is modal to it.
fn file_dialog<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
) -> tauri_plugin_dialog::FileDialogBuilder<R> {
    let dialog = app.dialog().file().set_title(title);
    match app.get_webview_window("main") {
        Some(window) => dialog.set_parent(&window),
        None => dialog,
    }
}

async fn pick_file<R: Runtime>(app: &AppHandle<R>, title: &str) -> AppResult<Option<PathBuf>> {
    let (tx, rx) = oneshot::channel();
    file_dialog(app, title)
        .add_filter("OPML", &OPML_EXTENSIONS)
        .pick_file(move |file| {
            let _ = tx.send(file);
        });
    rx.await.ok().flatten().map(to_path).transpose()
}

async fn pick_save_path<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
    file_name: &str,
) -> AppResult<Option<PathBuf>> {
    let (tx, rx) = oneshot::channel();
    file_dialog(app, title)
        .set_file_name(file_name)
        .add_filter("OPML", &OPML_EXTENSIONS)
        .save_file(move |file| {
            let _ = tx.send(file);
        });
    rx.await.ok().flatten().map(to_path).transpose()
}

/// Asks for an OPML file and subscribes to its feeds. `None` if the user cancelled. The new
/// feeds are fetched in the background by the scheduler (with `refresh:progress`).
#[tauri::command]
#[specta::specta]
pub async fn import_opml(
    app: AppHandle,
    service: State<'_, OpmlService>,
    scheduler: State<'_, Scheduler>,
) -> AppResult<Option<ImportSummary>> {
    let Some(path) = pick_file(&app, "Import subscriptions").await? else {
        return Ok(None);
    };
    let bytes = tauri::async_runtime::spawn_blocking(move || -> AppResult<Vec<u8>> {
        if std::fs::metadata(&path)?.len() > MAX_OPML_BYTES {
            return Err(AppError::invalid_input(
                "That file is too big to be an OPML file",
            ));
        }
        Ok(std::fs::read(&path)?)
    })
    .await
    .map_err(|err| AppError::internal(err.to_string()))??;
    let summary = service.import(bytes).await?;
    if summary.feeds > 0 {
        scheduler.reschedule();
    }
    Ok(Some(summary))
}

/// Asks where to save and writes an OPML 2.0 file of every feed. Returns the file's path, or
/// `None` if the user cancelled.
#[tauri::command]
#[specta::specta]
pub async fn export_opml(
    app: AppHandle,
    service: State<'_, OpmlService>,
) -> AppResult<Option<String>> {
    let Some(path) =
        pick_save_path(&app, "Export subscriptions", "omarss-subscriptions.opml").await?
    else {
        return Ok(None);
    };
    let xml = service.export().await?;
    let shown = path.display().to_string();
    tauri::async_runtime::spawn_blocking(move || std::fs::write(&path, xml))
        .await
        .map_err(|err| AppError::internal(err.to_string()))??;
    Ok(Some(shown))
}
