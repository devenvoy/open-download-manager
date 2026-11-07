use std::sync::Arc;
use tauri::State;

use crate::{
    application::{
        error::{AppError, AppResult},
        integration_service::BrowserSetupResult,
    },
    domain::{
        download::{
            ConflictResolutionOutcome, DownloadAction, DownloadRequest, ReviewDownloadRequest,
        },
        media::{AddPlaylistDownloadsRequest, PlaylistMetadata},
        settings::{ProxySettings, ProxyTestResult, SettingsPatch},
        snapshot::{DownloadSummary, IntegrationStatus, Snapshot},
    },
    infrastructure::media::ytdlp::YtDlpTools,
};

use super::app_state::AppState;

#[tauri::command]
pub fn snapshot(state: State<'_, AppState>) -> AppResult<Snapshot> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::storage("Application settings are unavailable."))?;
    let integration = state.integration.get_integration_info().unwrap_or_else(|e| {
        IntegrationStatus::unavailable(e.message)
    });
    let (summaries, icons) = state.download_history.with_downloads(|records| {
        state.file_icons.load(records);
        let icons = state.file_icons.snapshot();
        let summaries: Vec<DownloadSummary> = records.iter().map(DownloadSummary::from).collect();
        (summaries, icons)
    })?;
    Ok(Snapshot::with_summaries(
        settings.settings().clone(),
        summaries,
        integration,
    ).with_file_icons(icons))
}

#[tauri::command]
pub fn get_integration_info(state: State<'_, AppState>) -> AppResult<IntegrationStatus> {
    state.integration.get_integration_info()
}

#[tauri::command]
pub async fn register_browser(
    state: State<'_, AppState>,
    browser: String,
    extension_id: String,
) -> AppResult<()> {
    state
        .integration
        .register_browser(&browser, &extension_id)
        .await
}

#[tauri::command]
pub async fn browser_setup_action(
    state: State<'_, AppState>,
    action: String,
    browser: String,
) -> AppResult<BrowserSetupResult> {
    state.integration.browser_setup(&action, &browser).await
}

#[tauri::command]
pub fn review_download(
    state: State<'_, AppState>,
    request: ReviewDownloadRequest,
) -> AppResult<Vec<String>> {
    state.download_transfer.review_download(request)
}

#[tauri::command]
pub fn check_destination_conflict(
    state: State<'_, AppState>,
    directory: String,
    filename: String,
) -> AppResult<ConflictResolutionOutcome> {
    state
        .download_transfer
        .check_destination_conflict(&directory, &filename)
}

#[tauri::command]
pub async fn add_download(
    state: State<'_, AppState>,
    request: DownloadRequest,
) -> AppResult<Vec<String>> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::storage("Application settings are unavailable."))?
        .settings()
        .clone();
    state.download_transfer.add(request, &settings)
}

#[tauri::command]
pub fn download_action(
    state: State<'_, AppState>,
    id: String,
    action: DownloadAction,
) -> AppResult<()> {
    state.download_transfer.action(&id, action)
}

#[tauri::command]
pub fn remove_completed(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    state.download_transfer.remove_completed()
}

#[tauri::command]
pub fn update_settings(
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> AppResult<crate::domain::settings::Settings> {
    state.update_settings(patch)
}

#[tauri::command]
pub async fn test_proxy_connection(
    proxy: ProxySettings,
    test_url: Option<String>,
) -> AppResult<ProxyTestResult> {
    Ok(crate::infrastructure::http_download_executor::test_proxy_connection(
        &proxy,
        test_url.as_deref(),
    )
    .await)
}

#[tauri::command]
pub fn export_downloads_to_file(app: tauri::AppHandle) -> AppResult<()> {
    super::menu::export_downloads_flow(&app);
    Ok(())
}

#[tauri::command]
pub fn import_downloads_from_file(app: tauri::AppHandle) -> AppResult<()> {
    super::menu::import_downloads_flow(&app);
    Ok(())
}

#[tauri::command]
pub fn show_about_dialog(app: tauri::AppHandle) -> AppResult<()> {
    super::menu::show_about_dialog(&app);
    Ok(())
}

#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let service = state.update_service.clone();
    super::updater_controller::run_update_check(
        &app,
        &service,
        crate::domain::updater::UpdateCheckMode::Manual,
    )
    .await;
    Ok(())
}

#[tauri::command]
pub async fn get_playlist_info(
    state: State<'_, AppState>,
    url: String,
) -> AppResult<PlaylistMetadata> {
    let ytdlp = state
        .ytdlp
        .clone()
        .or_else(|| YtDlpTools::new().map(Arc::new))
        .ok_or_else(|| {
            AppError::download(
                "yt-dlp is not available. Ensure yt-dlp is installed and available in PATH or the app tools directory.",
            )
        })?;
    ytdlp.fetch_playlist_info(&url).await
}

#[tauri::command]
pub async fn get_video_formats(
    state: State<'_, AppState>,
    url: String,
) -> AppResult<Vec<crate::domain::media::YouTubeFormatChoice>> {
    let ytdlp = state
        .ytdlp
        .clone()
        .or_else(|| YtDlpTools::new().map(Arc::new))
        .ok_or_else(|| {
            AppError::download(
                "yt-dlp is not available. Ensure yt-dlp is installed and available in PATH or the app tools directory.",
            )
        })?;
    ytdlp.fetch_formats(&url).await
}

#[tauri::command]
pub fn add_playlist_downloads(
    state: State<'_, AppState>,
    request: AddPlaylistDownloadsRequest,
) -> AppResult<usize> {
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

#[tauri::command]
pub fn get_media_tools_status() -> AppResult<Vec<crate::domain::media_tools::MediaToolStatus>> {
    let manager = crate::infrastructure::media::MediaToolsManager::default_service();
    Ok(manager.status_all())
}

#[tauri::command]
pub async fn install_media_tool(
    tool: crate::domain::media_tools::MediaToolType,
) -> AppResult<crate::domain::media_tools::MediaToolStatus> {
    let manager = crate::infrastructure::media::MediaToolsManager::default_service();
    manager.download_tool(tool).await
}
