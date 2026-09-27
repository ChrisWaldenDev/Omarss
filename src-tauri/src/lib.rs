mod clock;
mod commands;
mod demo;
mod error;
mod logging;
mod models;
mod navigation;
mod paths;
mod services;
mod store;

use std::path::Path;

use specta_typescript::Typescript;
use tauri::Manager;

use crate::paths::AppPaths;
use crate::services::settings::SettingsService;
use crate::store::Store;

/// Generated TypeScript bindings for every command and IPC type (SPEC §4.3).
const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/types.ts");

fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::articles::get_sidebar,
            commands::articles::list_articles,
            commands::articles::get_article,
            commands::system::open_external,
        ])
        // IDs are SQLite rowids and timestamps are Unix seconds: both fit in a JS number.
        .dangerously_cast_bigints_to_number()
}

/// Writes the TypeScript bindings to `path`, touching the file only if its content changed
/// (so the Vite dev server doesn't reload for nothing).
fn export_bindings(builder: &tauri_specta::Builder<tauri::Wry>, path: &Path) -> Result<(), String> {
    let tmp = std::env::temp_dir().join(format!("omarss-bindings-{}.ts", std::process::id()));
    builder
        .export(Typescript::default(), &tmp)
        .map_err(|err| err.to_string())?;
    let fresh = std::fs::read_to_string(&tmp).map_err(|err| err.to_string());
    let _ = std::fs::remove_file(&tmp);
    let fresh = fresh?;
    if std::fs::read_to_string(path).ok().as_deref() != Some(fresh.as_str()) {
        std::fs::write(path, fresh).map_err(|err| format!("{}: {err}", path.display()))?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    // Keep the frontend's bindings current while developing. A debug binary run outside the
    // source tree just skips this.
    #[cfg(debug_assertions)]
    if let Err(err) = export_bindings(&builder, Path::new(BINDINGS_PATH)) {
        eprintln!("warning: could not export TypeScript bindings: {err}");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(navigation::guard())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let paths = AppPaths::resolve(app.handle())?;
            logging::init(&paths.log_dir);
            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                data_dir = %paths.data_dir.display(),
                "starting omarss"
            );

            let store = Store::open(&paths.db_file, &paths.backups_dir).inspect_err(|err| {
                tracing::error!(%err, "failed to open the database");
            })?;
            let settings = SettingsService::new(store.clone());
            let current = tauri::async_runtime::block_on(settings.get())?;
            app.set_theme(current.theme.to_window_theme());

            app.manage(store);
            app.manage(settings);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running omarss");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regenerates `src/lib/types.ts`. CI fails if this leaves the file changed, i.e. if the
    /// committed bindings are stale.
    #[test]
    fn export_typescript_bindings() {
        export_bindings(&specta_builder(), Path::new(BINDINGS_PATH)).unwrap();
        let ts = std::fs::read_to_string(BINDINGS_PATH).unwrap();
        for name in [
            "getSettings",
            "updateSettings",
            "listArticles",
            "openExternal",
        ] {
            assert!(ts.contains(name), "bindings are missing {name}");
        }
    }
}
