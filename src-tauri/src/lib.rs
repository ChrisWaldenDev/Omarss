#[cfg(any(debug_assertions, test))]
mod bindings;
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

use tauri::Manager;

use crate::paths::AppPaths;
use crate::services::settings::SettingsService;
use crate::store::Store;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    // Keep the frontend's bindings current while developing. A debug binary run outside the
    // source tree just skips this.
    #[cfg(debug_assertions)]
    if let Err(err) = bindings::export(&builder, std::path::Path::new(bindings::BINDINGS_PATH)) {
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
                version = %app.package_info().version,
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
