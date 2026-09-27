//! Backend → frontend events (SPEC §4.3). Names are namespaced as the spec lists them.

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[tauri_specta(event_name = "refresh:progress")]
#[serde(rename_all = "camelCase")]
pub struct RefreshProgress {
    pub done: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[tauri_specta(event_name = "refresh:done")]
#[serde(rename_all = "camelCase")]
pub struct RefreshDone {
    pub new_count: u32,
    pub errors: u32,
    /// Every feed failed to connect, so automatic refresh waits for the network.
    pub offline: bool,
}

/// Articles were added or changed outside a user action (refresh, subscribe).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[tauri_specta(event_name = "articles:changed")]
#[serde(rename_all = "camelCase")]
pub struct ArticlesChanged {
    pub new_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[tauri_specta(event_name = "feed:error")]
#[serde(rename_all = "camelCase")]
pub struct FeedError {
    pub feed_id: i64,
    pub message: String,
}
