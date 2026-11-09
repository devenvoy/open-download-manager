// Created: 2026-11-10
// Reason: refactor: modularize monolithic presentation commands into domain submodules
// Description: Tauri IPC command handlers for application settings, snapshot queries, directory validation, and updater checks.

use tauri::State;

use crate::{
    application::error::{AppError, AppResult},
    domain::{
        settings::{Settings, SettingsPatch},
        snapshot::{DownloadSummary, IntegrationStatus, Snapshot},
        updater::UpdateCheckMode,
    },
    presentation::app_state::AppState,
};

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
pub fn update_settings(
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> AppResult<Settings> {
    state.update_settings(patch)
}

#[tauri::command]
pub fn validate_directory(path: String) -> AppResult<()> {
    crate::infrastructure::fs_utils::validate_directory_writable(std::path::Path::new(&path))
        .map_err(AppError::invalid_settings)
}

#[tauri::command]
pub fn show_about_dialog(app: tauri::AppHandle) -> AppResult<()> {
    crate::presentation::menu::show_about_dialog(&app);
    Ok(())
}

#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let service = state.update_service.clone();
    crate::presentation::updater_controller::run_update_check(
        &app,
        &service,
        UpdateCheckMode::Manual,
    )
    .await;
    Ok(())
}
