use tauri::State;

use crate::{
    application::error::{AppError, AppResult},
    domain::{download::DownloadRequest, settings::SettingsPatch, snapshot::Snapshot},
};

use super::app_state::AppState;

#[tauri::command]
pub fn snapshot(state: State<'_, AppState>) -> AppResult<Snapshot> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::storage("Application settings are unavailable."))?;
    Ok(Snapshot::new(
        settings.settings().clone(),
        state.download_history.downloads()?,
    ))
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
    Ok(updated)
}
