use tauri::State;

use crate::{
    application::{
        error::{AppError, AppResult},
        integration_service::BrowserSetupResult,
    },
    domain::{
        download::{DownloadAction, DownloadRequest, ReviewDownloadRequest},
        settings::SettingsPatch,
        snapshot::{IntegrationStatus, Snapshot},
    },
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
    let downloads = state.download_history.downloads()?;
    state.file_icons.load(&downloads);
    let icons = state.file_icons.snapshot();
    Ok(Snapshot::with_integration(
        settings.settings().clone(),
        downloads,
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
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| AppError::storage("Application settings are unavailable."))?;
    let updated = settings.update(patch)?;
    state
        .download_transfer
        .set_max_concurrent(updated.max_concurrent);
    state
        .download_transfer
        .set_speed_limit(updated.speed_limit_kbps);
    let _ = crate::infrastructure::autostart::sync_autostart(updated.start_with_windows);
    Ok(updated)
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
