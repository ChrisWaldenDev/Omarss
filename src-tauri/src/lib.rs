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
mod network;
mod opml;
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
use crate::services::images::ImageCache;
use crate::services::maintenance::MaintenanceService;
use crate::services::opml::OpmlService;
use crate::services::settings::SettingsService;
use crate::store::Store;

fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::get_app_info,
            commands::settings::get_custom_css,
            commands::articles::get_sidebar,
            commands::articles::list_articles,
            commands::articles::get_article,
            commands::articles::set_articles_read,
            commands::articles::set_article_starred,
            commands::articles::mark_all_read,
            commands::articles::undo_mark_all_read,
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
            commands::opml::import_opml,
            commands::opml::export_opml,
            commands::storage::get_storage_info,
            commands::storage::clear_image_cache,
            commands::storage::compact_database,
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
        .plugin(tauri_plugin_dialog::init())
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
            if current.ui_scale != 100 {
                commands::settings::apply_ui_scale(app.handle(), current.ui_scale);
            }

            let version = app.package_info().version.to_string();
            let http = HttpClient::new(&version, Some(&current.proxy_url)).or_else(|err| {
                tracing::warn!(%err, "ignoring the saved proxy");
                HttpClient::new(&version, None)
            })?;
            let feeds = FeedService::new(
                store.clone(),
                http.clone(),
                settings.clone(),
                paths.icons_dir.clone(),
            );
            let articles = ArticleService::new(store.clone(), settings.clone());
            let images = ImageCache::new(
                paths.image_cache_dir.clone(),
                http.clone(),
                current.image_cache_mb,
            );
            let maintenance = MaintenanceService::new(
                store.clone(),
                settings.clone(),
                images.clone(),
                paths.data_dir.clone(),
            );
            let opml = OpmlService::new(store.clone());
            let scheduler = Scheduler::start(
                app.handle().clone(),
                feeds.clone(),
                settings.clone(),
                Some(maintenance.clone()),
            );
            let idle = scheduler.clone();
            maintenance.start(move || !idle.status().running);

            app.manage(store);
            app.manage(settings);
            app.manage(http);
            app.manage(feeds);
            app.manage(articles);
            app.manage(images);
            app.manage(maintenance);
            app.manage(opml);
            app.manage(scheduler);
            app.manage(paths);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Omarss");
}
