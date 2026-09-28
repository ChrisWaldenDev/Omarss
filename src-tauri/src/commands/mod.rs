//! Thin Tauri command handlers. They validate input and delegate to services (SPEC §4).

pub mod articles;
pub mod feeds;
pub mod opml;
pub mod refresh;
pub mod settings;
pub mod storage;
pub mod system;
