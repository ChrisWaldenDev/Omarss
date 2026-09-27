//! Refresh commands. Refreshing runs in the background and reports through events
//! (`refresh:progress`, `refresh:done`), so these return immediately (SPEC §4.1).

use tauri::State;

use crate::error::AppResult;
use crate::models::{RefreshStatus, RefreshTarget};
use crate::scheduler::Scheduler;

#[tauri::command]
#[specta::specta]
pub async fn refresh(scheduler: State<'_, Scheduler>, target: RefreshTarget) -> AppResult<()> {
    scheduler.request(target);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_refresh_status(scheduler: State<'_, Scheduler>) -> AppResult<RefreshStatus> {
    Ok(scheduler.status())
}
