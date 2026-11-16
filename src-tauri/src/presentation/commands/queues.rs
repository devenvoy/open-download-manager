// Created: 2026-11-10
// Reason: refactor: modularize monolithic presentation commands into domain submodules
// Description: Tauri IPC command handlers for queue creation, updating, item assignment, and queue scheduling.

use tauri::State;

use crate::{
    application::error::AppResult,
    domain::queue::{
        DownloadQueue, QueueDisposition, QueueSchedule, DEFAULT_QUEUE_CONCURRENCY,
    },
    presentation::app_state::AppState,
};

#[tauri::command(async)]
pub fn list_queues(
    state: State<'_, AppState>,
) -> AppResult<Vec<DownloadQueue>> {
    let _timer = crate::infrastructure::perf::track_command("list_queues");
    Ok(state.queue_service()?.list_queues())
}

#[tauri::command(async)]
pub fn create_queue(
    state: State<'_, AppState>,
    name: String,
    concurrency_limit: Option<u8>,
    speed_limit_kbps: Option<u32>,
    schedule: Option<QueueSchedule>,
) -> AppResult<DownloadQueue> {
    let _timer = crate::infrastructure::perf::track_command("create_queue");
    state.queue_service()?.create_queue(
        name,
        concurrency_limit.unwrap_or(DEFAULT_QUEUE_CONCURRENCY),
        speed_limit_kbps.unwrap_or(0),
        schedule,
    )
}

#[tauri::command(async)]
pub fn update_queue(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    enabled: Option<bool>,
    concurrency_limit: Option<u8>,
    speed_limit_kbps: Option<u32>,
    schedule: Option<Option<QueueSchedule>>,
) -> AppResult<DownloadQueue> {
    let _timer = crate::infrastructure::perf::track_command("update_queue");
    state.queue_service()?.update_queue(
        &id,
        name,
        enabled,
        concurrency_limit,
        speed_limit_kbps,
        schedule,
    )
}

#[tauri::command(async)]
pub fn delete_queue(
    state: State<'_, AppState>,
    id: String,
    disposition: Option<QueueDisposition>,
) -> AppResult<Vec<String>> {
    let _timer = crate::infrastructure::perf::track_command("delete_queue");
    state.queue_service()?.delete_queue(
        &id,
        disposition.unwrap_or(QueueDisposition::MoveToDefault),
    )
}

#[tauri::command(async)]
pub fn start_queue(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<DownloadQueue> {
    let _timer = crate::infrastructure::perf::track_command("start_queue");
    let result = state.queue_service()?.start_queue(&id)?;
    state.download_transfer.pump_queue();
    Ok(result)
}

#[tauri::command(async)]
pub fn pause_queue(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<DownloadQueue> {
    let _timer = crate::infrastructure::perf::track_command("pause_queue");
    let result = state.queue_service()?.pause_queue(&id)?;
    state.download_transfer.pause_queue_downloads(&id);
    Ok(result)
}

#[tauri::command(async)]
pub fn assign_download_to_queue(
    state: State<'_, AppState>,
    download_id: String,
    queue_id: String,
) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("assign_download_to_queue");
    state.queue_service()?.assign_download(download_id, &queue_id)?;
    state.download_transfer.pump_queue();
    Ok(())
}

#[tauri::command(async)]
pub fn reorder_queue_items(
    state: State<'_, AppState>,
    queue_id: String,
    item_ids: Vec<String>,
) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("reorder_queue_items");
    state.queue_service()?.reorder_items(&queue_id, &item_ids)?;
    state.download_transfer.pump_queue();
    Ok(())
}
