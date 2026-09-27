//! `tracing` setup: a daily-rotated log file (7 kept) plus stderr in debug builds (SPEC §3, §11.4).
//! Set `RUST_LOG` to override the default filter.

use std::path::Path;

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const DEFAULT_FILTER: &str = "warn,omarss_lib=info";

pub fn init(log_dir: &Path) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    // The appender prunes old files on start-up and complains if the directory is missing.
    let _ = std::fs::create_dir_all(log_dir);
    let file = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("omarss")
        .filename_suffix("log")
        .max_log_files(7)
        .build(log_dir);
    let file_error = file.as_ref().err().map(ToString::to_string);
    let file_layer = file
        .ok()
        .map(|appender| fmt::layer().with_ansi(false).with_writer(appender));
    let stderr_layer = cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr));

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .try_init();

    if let Some(err) = file_error {
        tracing::warn!(%err, dir = %log_dir.display(), "file logging disabled");
    }
}
