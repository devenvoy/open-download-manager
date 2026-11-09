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

#[tauri::command]
pub fn list_queues(
    state: State<'_, AppState>,
) -> AppResult<Vec<DownloadQueue>> {
    Ok(state.queue_service()?.list_queues())
}

#[tauri::command]
pub fn create_queue(
    state: State<'_, AppState>,
    name: String,
    concurrency_limit: Option<u8>,
    speed_limit_kbps: Option<u32>,
    schedule: Option<QueueSchedule>,
) -> AppResult<DownloadQueue> {
    state.queue_service()?.create_queue(
        name,
        concurrency_limit.unwrap_or(DEFAULT_QUEUE_CONCURRENCY),
        speed_limit_kbps.unwrap_or(0),
        schedule,
    )
}

#[tauri::command]
pub fn update_queue(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    enabled: Option<bool>,
    concurrency_limit: Option<u8>,
    speed_limit_kbps: Option<u32>,
    schedule: Option<Option<QueueSchedule>>,
) -> AppResult<DownloadQueue> {
    state.queue_service()?.update_queue(
        &id,
        name,
        enabled,
        concurrency_limit,
        speed_limit_kbps,
        schedule,
    )
}

#[tauri::command]
pub fn delete_queue(
    state: State<'_, AppState>,
    id: String,
    disposition: Option<QueueDisposition>,
) -> AppResult<Vec<String>> {
    state.queue_service()?.delete_queue(
        &id,
        disposition.unwrap_or(QueueDisposition::MoveToDefault),
    )
}

#[tauri::command]
pub fn start_queue(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<DownloadQueue> {
    let result = state.queue_service()?.start_queue(&id)?;
    state.download_transfer.pump_queue();
    Ok(result)
}

#[tauri::command]
pub fn pause_queue(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<DownloadQueue> {
    let result = state.queue_service()?.pause_queue(&id)?;
    state.download_transfer.pause_queue_downloads(&id);
    Ok(result)
}

#[tauri::command]
pub fn assign_download_to_queue(
    state: State<'_, AppState>,
    download_id: String,
    queue_id: String,
) -> AppResult<()> {
    state.queue_service()?.assign_download(download_id, &queue_id)?;
    state.download_transfer.pump_queue();
    Ok(())
}

#[tauri::command]
pub fn reorder_queue_items(
    state: State<'_, AppState>,
    queue_id: String,
    item_ids: Vec<String>,
) -> AppResult<()> {
    state.queue_service()?.reorder_items(&queue_id, &item_ids)?;
    state.download_transfer.pump_queue();
    Ok(())
}
