use std::sync::Mutex;

use tauri::State;

use crate::{
    application::{
        download_history_service::DownloadHistoryService,
        error::{AppError, AppResult},
        settings_service::SettingsService,
    },
    domain::{settings::SettingsPatch, snapshot::Snapshot},
};

pub struct AppState {
    settings: Mutex<SettingsService>,
    download_history: DownloadHistoryService,
}

impl AppState {
    pub fn new(settings: SettingsService, download_history: DownloadHistoryService) -> Self {
        Self {
            settings: Mutex::new(settings),
            download_history,
        }
    }
}

#[tauri::command]
pub fn snapshot(state: State<'_, AppState>) -> AppResult<Snapshot> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::storage("Application settings are unavailable."))?;
    Ok(Snapshot::new(
        settings.settings().clone(),
        state.download_history.downloads().to_vec(),
    ))
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
    settings.update(patch)
}
