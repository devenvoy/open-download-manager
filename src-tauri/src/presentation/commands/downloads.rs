// Created: 2026-11-10
// Reason: refactor: modularize monolithic presentation commands into domain submodules
// Description: Tauri IPC command handlers for download creation, actions, reviews, and import/export.

use tauri::State;

use crate::{
    application::error::{AppError, AppResult},
    domain::{
        completion::CompletionAction,
        download::{
            ConflictResolutionOutcome, DownloadAction, DownloadRequest, ReviewDownloadRequest,
        },
        media::AddPlaylistDownloadsRequest,
    },
    presentation::app_state::AppState,
};

#[tauri::command(async)]
pub fn review_download(
    state: State<'_, AppState>,
    request: ReviewDownloadRequest,
) -> AppResult<Vec<String>> {
    let _timer = crate::infrastructure::perf::track_command("review_download");
    state.download_transfer.review_download(request)
}

#[tauri::command(async)]
pub fn check_destination_conflict(
    state: State<'_, AppState>,
    directory: String,
    filename: String,
) -> AppResult<ConflictResolutionOutcome> {
    let _timer = crate::infrastructure::perf::track_command("check_destination_conflict");
    state
        .download_transfer
        .check_destination_conflict(&directory, &filename)
}

#[tauri::command(async)]
pub async fn add_download(
    state: State<'_, AppState>,
    request: DownloadRequest,
) -> AppResult<Vec<String>> {
    let _timer = crate::infrastructure::perf::track_command("add_download");
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::storage("Application settings are unavailable."))?
        .settings()
        .clone();
    state.download_transfer.add(request, &settings)
}

#[tauri::command(async)]
pub fn download_action(
    state: State<'_, AppState>,
    id: String,
    action: DownloadAction,
) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("download_action");
    state.download_transfer.action(&id, action)
}

#[tauri::command(async)]
pub fn remove_completed(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let _timer = crate::infrastructure::perf::track_command("remove_completed");
    state.download_transfer.remove_completed()
}

#[tauri::command(async)]
pub fn add_playlist_downloads(
    state: State<'_, AppState>,
    request: AddPlaylistDownloadsRequest,
) -> AppResult<usize> {
    let _timer = crate::infrastructure::perf::track_command("add_playlist_downloads");
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::storage("Application settings are unavailable."))?
        .settings()
        .clone();
    state
        .download_transfer
        .add_playlist_downloads(request, &settings)
}

#[tauri::command(async)]
pub fn export_downloads_to_file(app: tauri::AppHandle) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("export_downloads_to_file");
    crate::presentation::menu::export_downloads_flow(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn import_downloads_from_file(app: tauri::AppHandle) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("import_downloads_from_file");
    crate::presentation::menu::import_downloads_flow(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn update_download_completion_actions(
    state: State<'_, AppState>,
    id: String,
    actions: Option<Vec<CompletionAction>>,
) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("update_download_completion_actions");
    state
        .download_transfer
        .update_download_completion_actions(&id, actions)
}

#[tauri::command(async)]
pub async fn retry_download_actions(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("retry_download_actions");
    state.download_transfer.retry_completion_actions(&id).await
}
