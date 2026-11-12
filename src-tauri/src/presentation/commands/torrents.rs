// Created: 2026-11-11
// Reason: feat(torrent): add isolated BitTorrent subsystem
// Description: Tauri IPC commands for the torrent window. These commands only touch
// TorrentService and never the download history, transfer service, or queue model.

use tauri::State;

use crate::{
    application::error::AppResult,
    domain::torrent::{
        TorrentAction, TorrentAddOptions, TorrentAddResult, TorrentEngineStatus, TorrentFileItem,
        TorrentListItem, TorrentReviewRequest, TorrentSettings, TorrentTrackerStatus,
    },
    presentation::app_state::AppState,
};

/// Reports a failed torrent operation on the console.
///
/// The window shows the message as a toast, but a toast disappears again and
/// the console keeps the full context of what the engine rejected.
fn report_failure(operation: &str, error: &crate::application::error::AppError) {
    eprintln!("[odm] torrent {operation} failed: {}", error.message);
}

#[tauri::command]
pub fn torrent_status(state: State<'_, AppState>) -> TorrentEngineStatus {
    state.torrents.status()
}

#[tauri::command]
pub fn torrent_list(state: State<'_, AppState>) -> AppResult<Vec<TorrentListItem>> {
    state.torrents.list()
}

#[tauri::command]
pub async fn torrent_add_magnet(
    state: State<'_, AppState>,
    magnet: String,
    options: Option<TorrentAddOptions>,
) -> AppResult<TorrentAddResult> {
    state
        .torrents
        .add_magnet(magnet, options.unwrap_or_default())
        .await
        .map_err(|error| {
            report_failure("add magnet", &error);
            error
        })
}

#[tauri::command]
pub async fn torrent_add_file(
    state: State<'_, AppState>,
    path: String,
    options: Option<TorrentAddOptions>,
) -> AppResult<TorrentAddResult> {
    state
        .torrents
        .add_file(path, options.unwrap_or_default())
        .await
        .map_err(|error| {
            report_failure("add file", &error);
            error
        })
}

#[tauri::command]
pub fn torrent_take_review(state: State<'_, AppState>) -> Option<TorrentReviewRequest> {
    let review = state.torrents.take_pending_review();
    if review.is_some() {
        eprintln!("[odm] Torrent review handed over to the Torrents window");
    }
    review
}

#[tauri::command]
pub async fn torrent_action(
    state: State<'_, AppState>,
    id: usize,
    action: TorrentAction,
) -> AppResult<()> {
    state.torrents.action(id, action).await.map_err(|error| {
        report_failure("action", &error);
        error
    })
}

#[tauri::command]
pub fn torrent_files(state: State<'_, AppState>, id: usize) -> AppResult<Vec<TorrentFileItem>> {
    state.torrents.files(id)
}

#[tauri::command]
pub fn torrent_tracker_status(
    state: State<'_, AppState>,
    id: usize,
) -> AppResult<Vec<TorrentTrackerStatus>> {
    state.torrents.tracker_status(id)
}

#[tauri::command]
pub async fn torrent_set_files(
    state: State<'_, AppState>,
    id: usize,
    included: Vec<usize>,
) -> AppResult<()> {
    state
        .torrents
        .set_files(id, included)
        .await
        .map_err(|error| {
            report_failure("set files", &error);
            error
        })
}

#[tauri::command]
pub fn torrent_settings_get(state: State<'_, AppState>) -> TorrentSettings {
    state.torrents.settings()
}

#[tauri::command]
pub async fn torrent_settings_set(
    state: State<'_, AppState>,
    settings: TorrentSettings,
) -> AppResult<TorrentSettings> {
    state.torrents.update_settings(settings)
}

#[tauri::command]
pub fn torrent_set_watching(state: State<'_, AppState>, watching: bool) {
    state.torrents.set_watching(watching);
}

#[tauri::command]
pub async fn torrent_restart_engine(state: State<'_, AppState>) -> AppResult<()> {
    state.torrents.restart_engine().await
}
