use std::sync::Mutex;

use crate::application::{
    download_history_service::DownloadHistoryService,
    download_transfer_service::DownloadTransferService, settings_service::SettingsService,
};

pub struct AppState {
    pub(super) settings: Mutex<SettingsService>,
    pub(super) download_history: DownloadHistoryService,
    pub(super) download_transfer: DownloadTransferService,
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
