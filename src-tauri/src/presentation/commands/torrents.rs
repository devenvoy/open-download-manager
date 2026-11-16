// Created: 2026-11-11
// Reason: feat(torrent): add isolated BitTorrent subsystem
// Description: Tauri IPC commands for the torrent window. These commands only touch
// TorrentService and never the download history, transfer service, or queue model.

use tauri::State;

use crate::{
    application::error::AppResult,
    domain::torrent::{
        TorrentAction, TorrentAddOptions, TorrentAddResult, TorrentEngineStatus, TorrentFileItem,
        TorrentListItem, TorrentPeerItem, TorrentPiecesInfo, TorrentReviewRequest, TorrentSettings,
        TorrentTrackerStatus,
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

#[tauri::command(async)]
pub fn torrent_status(state: State<'_, AppState>) -> TorrentEngineStatus {
    let _timer = crate::infrastructure::perf::track_command("torrent_status");
    state.torrents.status()
}

#[tauri::command(async)]
pub fn torrent_list(state: State<'_, AppState>) -> AppResult<Vec<TorrentListItem>> {
    let _timer = crate::infrastructure::perf::track_command("torrent_list");
    state.torrents.list()
}

#[tauri::command(async)]
pub async fn torrent_add_magnet(
    state: State<'_, AppState>,
    magnet: String,
    options: Option<TorrentAddOptions>,
) -> AppResult<TorrentAddResult> {
    let _timer = crate::infrastructure::perf::track_command("torrent_add_magnet");
    state
        .torrents
        .add_magnet(magnet, options.unwrap_or_default())
        .await
        .inspect_err(|error| {
            report_failure("add magnet", error);
        })
}

#[tauri::command(async)]
pub async fn torrent_add_file(
    state: State<'_, AppState>,
    path: String,
    options: Option<TorrentAddOptions>,
) -> AppResult<TorrentAddResult> {
    let _timer = crate::infrastructure::perf::track_command("torrent_add_file");
    state
        .torrents
        .add_file(path, options.unwrap_or_default())
        .await
        .inspect_err(|error| {
            report_failure("add file", error);
        })
}

#[tauri::command(async)]
pub fn torrent_take_review(state: State<'_, AppState>) -> Option<TorrentReviewRequest> {
    let _timer = crate::infrastructure::perf::track_command("torrent_take_review");
    let review = state.torrents.take_pending_review();
    if review.is_some() {
        eprintln!("[odm] Torrent review handed over to the Torrents window");
    }
    review
}

#[tauri::command(async)]
pub async fn torrent_action(
    state: State<'_, AppState>,
    id: usize,
    action: TorrentAction,
) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("torrent_action");
    state.torrents.action(id, action).await.inspect_err(|error| {
        report_failure("action", error);
    })
}

#[tauri::command(async)]
pub fn torrent_files(state: State<'_, AppState>, id: usize) -> AppResult<Vec<TorrentFileItem>> {
    let _timer = crate::infrastructure::perf::track_command("torrent_files");
    state.torrents.files(id)
}

#[tauri::command(async)]
pub fn torrent_tracker_status(
    state: State<'_, AppState>,
    id: usize,
) -> AppResult<Vec<TorrentTrackerStatus>> {
    let _timer = crate::infrastructure::perf::track_command("torrent_tracker_status");
    state.torrents.tracker_status(id)
}

#[tauri::command(async)]
pub fn torrent_peers(
    state: State<'_, AppState>,
    id: usize,
) -> AppResult<Vec<TorrentPeerItem>> {
    let _timer = crate::infrastructure::perf::track_command("torrent_peers");
    state.torrents.peers(id)
}

#[tauri::command(async)]
pub fn torrent_pieces(
    state: State<'_, AppState>,
    id: usize,
) -> AppResult<TorrentPiecesInfo> {
    let _timer = crate::infrastructure::perf::track_command("torrent_pieces");
    state.torrents.pieces(id)
}

#[tauri::command(async)]
pub async fn torrent_set_files(
    state: State<'_, AppState>,
    id: usize,
    included: Vec<usize>,
) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("torrent_set_files");
    state
        .torrents
        .set_files(id, included)
        .await
        .inspect_err(|error| {
            report_failure("set files", error);
        })
}

#[tauri::command(async)]
pub fn torrent_settings_get(state: State<'_, AppState>) -> TorrentSettings {
    let _timer = crate::infrastructure::perf::track_command("torrent_settings_get");
    state.torrents.settings()
}

#[tauri::command(async)]
pub async fn torrent_settings_set(
    state: State<'_, AppState>,
    settings: TorrentSettings,
) -> AppResult<TorrentSettings> {
    let _timer = crate::infrastructure::perf::track_command("torrent_settings_set");
    state.torrents.update_settings(settings)
}

#[tauri::command(async)]
pub fn torrent_set_watching(state: State<'_, AppState>, watching: bool) {
    let _timer = crate::infrastructure::perf::track_command("torrent_set_watching");
    state.torrents.set_watching(watching);
}

#[tauri::command(async)]
pub async fn torrent_restart_engine(state: State<'_, AppState>) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("torrent_restart_engine");
    state.torrents.restart_engine().await
}
