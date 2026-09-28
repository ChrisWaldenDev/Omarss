#[cfg(any(debug_assertions, test))]
mod bindings;
mod clock;
mod commands;
mod content;
mod error;
mod events;
mod feed;
mod fetch;
mod logging;
mod models;
mod navigation;
mod paths;
mod protocol;
mod scheduler;
mod services;
mod store;
#[cfg(test)]
mod tests;

use tauri::Manager;

use crate::fetch::HttpClient;
use crate::paths::AppPaths;
use crate::scheduler::Scheduler;
use crate::services::articles::ArticleService;
use crate::services::feeds::FeedService;
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
            commands::articles::set_articles_read,
            commands::articles::set_article_starred,
            commands::feeds::discover_feeds,
            commands::feeds::preview_feed,
            commands::feeds::subscribe_feed,
            commands::feeds::get_feed,
            commands::feeds::update_feed,
            commands::feeds::unsubscribe_feed,
            commands::feeds::create_folder,
            commands::feeds::rename_folder,
            commands::feeds::delete_folder,
            commands::feeds::reorder_sidebar,
            commands::refresh::refresh,
            commands::refresh::get_refresh_status,
            commands::system::open_external,
        ])
        .events(tauri_specta::collect_events![
            events::RefreshProgress,
            events::RefreshDone,
            events::ArticlesChanged,
            events::FeedError,
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
        .register_asynchronous_uri_scheme_protocol(protocol::SCHEME, protocol::handle)
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let paths = AppPaths::resolve(app.handle())?;
            logging::init(&paths.log_dir);
            tracing::info!(
                version = %app.package_info().version,
                data_dir = %paths.data_dir.display(),
                "starting Omarss"
            );

            let store = Store::open(&paths.db_file, &paths.backups_dir).inspect_err(|err| {
                tracing::error!(%err, "failed to open the database");
            })?;
            let settings = SettingsService::new(store.clone());
            let current = tauri::async_runtime::block_on(settings.get())?;
            app.set_theme(current.theme.to_window_theme());

            let http = HttpClient::new(&app.package_info().version.to_string())?;
            let feeds = FeedService::new(store.clone(), http, settings.clone(), paths.icons_dir);
            let articles = ArticleService::new(store.clone());
            let scheduler = Scheduler::start(app.handle().clone(), feeds.clone(), settings.clone());

            app.manage(store);
            app.manage(settings);
            app.manage(feeds);
            app.manage(articles);
            app.manage(scheduler);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Omarss");
}
