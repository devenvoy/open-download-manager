use std::sync::Mutex;

use crate::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        integration_service::IntegrationService, settings_service::SettingsService,
        updater_service::UpdateService,
    },
    infrastructure::file_icons::FileIconProvider,
};

pub struct AppState {
    pub(super) settings: Mutex<SettingsService>,
    pub(super) download_history: DownloadHistoryService,
    pub(super) download_transfer: DownloadTransferService,
    pub(super) integration: IntegrationService,
    pub(super) file_icons: FileIconProvider,
    pub(super) update_service: UpdateService,
}

impl AppState {
    pub fn new(
        settings: SettingsService,
        download_history: DownloadHistoryService,
        download_transfer: DownloadTransferService,
        integration: IntegrationService,
        file_icons: FileIconProvider,
        update_service: UpdateService,
    ) -> Self {
        Self {
            settings: Mutex::new(settings),
            download_history,
            download_transfer,
            integration,
            file_icons,
            update_service,
        }
    }
}
