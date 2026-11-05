use tauri::{AppHandle, Emitter};

use crate::application::{
    download_event_publisher::DownloadEventPublisher,
    error::{AppError, AppResult},
};

use super::event_policy::DOWNLOAD_CHANGE_EVENT;

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
            .emit(DOWNLOAD_CHANGE_EVENT, ())
            .map_err(|error| AppError::download(format!("Could not notify the renderer: {error}")))
    }
}
