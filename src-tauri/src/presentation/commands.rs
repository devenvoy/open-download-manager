use std::sync::Mutex;

use tauri::{AppHandle, Emitter, State};

use crate::{
    application::{
        download_history_service::{DownloadEventPublisher, DownloadHistoryService},
        download_transfer_service::DownloadTransferService,
        error::{AppError, AppResult},
        settings_service::SettingsService,
    },
    domain::{download::DownloadRequest, settings::SettingsPatch, snapshot::Snapshot},
};

pub struct AppState {
    settings: Mutex<SettingsService>,
    download_history: DownloadHistoryService,
    download_transfer: DownloadTransferService,
}

impl AppState {
    pub fn new(
        settings: SettingsService,
        download_history: DownloadHistoryService,
        download_transfer: DownloadTransferService,
    ) -> Self {
        Self {
            settings: Mutex::new(settings),
            download_history,
            download_transfer,
        }
    }
}

pub struct TauriDownloadEventPublisher {
    app: AppHandle,
}

impl TauriDownloadEventPublisher {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl DownloadEventPublisher for TauriDownloadEventPublisher {
    fn publish_change(&self) -> AppResult<()> {
        self.app
            .emit("downloader:change", ())
            .map_err(|error| AppError::download(format!("Could not notify the renderer: {error}")))
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
        .set_max_concurrent(updated.max_concurrent)?;
    Ok(updated)
}
