use std::sync::Mutex;

use crate::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        integration_service::IntegrationService, settings_service::SettingsService,
    },
    infrastructure::file_icons::FileIconProvider,
};

pub struct AppState {
    pub(super) settings: Mutex<SettingsService>,
    pub(super) download_history: DownloadHistoryService,
    pub(super) download_transfer: DownloadTransferService,
    pub(super) integration: IntegrationService,
    pub(super) file_icons: FileIconProvider,
}

impl AppState {
    pub fn new(
        settings: SettingsService,
        download_history: DownloadHistoryService,
        download_transfer: DownloadTransferService,
        integration: IntegrationService,
        file_icons: FileIconProvider,
    ) -> Self {
        Self {
            settings: Mutex::new(settings),
            download_history,
            download_transfer,
            integration,
            file_icons,
        }
    }
}
