// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Description: Shared application state holding services, transfer handles, and runtime settings.

use std::sync::{Arc, Mutex};

use crate::{
    application::{
        credential_service::CredentialService,
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        error::{AppError, AppResult},
        integration_service::IntegrationService,
        settings_service::SettingsService,
        updater_service::UpdateService,
    },
    domain::settings::{Settings, SettingsPatch},
    infrastructure::{file_icons::FileIconProvider, media::ytdlp::YtDlpTools},
};

pub struct AppState {
    pub(super) settings: Mutex<SettingsService>,
    pub(super) download_history: DownloadHistoryService,
    pub(super) download_transfer: DownloadTransferService,
    pub(super) credentials: Arc<CredentialService>,
    pub(super) integration: IntegrationService,
    pub(super) file_icons: FileIconProvider,
    pub(super) update_service: UpdateService,
    pub(super) ytdlp: Option<Arc<YtDlpTools>>,
}

impl AppState {
    pub fn new(
        settings: SettingsService,
        download_history: DownloadHistoryService,
        download_transfer: DownloadTransferService,
        credentials: Arc<CredentialService>,
        integration: IntegrationService,
        file_icons: FileIconProvider,
        update_service: UpdateService,
    ) -> Self {
        Self {
            settings: Mutex::new(settings),
            download_history,
            download_transfer,
            credentials,
            integration,
            file_icons,
            update_service,
            ytdlp: YtDlpTools::new().map(Arc::new),
        }
    }

    pub fn with_ytdlp(mut self, ytdlp: Option<Arc<YtDlpTools>>) -> Self {
        self.ytdlp = ytdlp;
        self
    }

    /// Atomically updates settings: validates candidate in memory, applies runtime
    /// reconfiguration to the download executor first (preserving existing clients upon error),
    /// and persists to disk only when runtime reconfiguration succeeds.
    pub fn update_settings(&self, patch: SettingsPatch) -> AppResult<Settings> {
        let mut settings_lock = self
            .settings
            .lock()
            .map_err(|_| AppError::storage("Application settings are unavailable."))?;

        // 1. Prepare and validate candidate settings in memory without committing to disk
        let candidate = settings_lock.prepare_update(patch)?;

        // Validate that default directory, category directories, and temporary directory are writable
        crate::infrastructure::fs_utils::validate_directory_writable(&candidate.default_directory)
            .map_err(AppError::invalid_settings)?;

        for (category, dir) in &candidate.category_directories {
            crate::infrastructure::fs_utils::validate_directory_writable(dir)
                .map_err(|err| AppError::invalid_settings(format!("Invalid folder for category {}: {err}", category.as_str())))?;
        }

        if let Some(ref temp_dir) = candidate.temporary_directory {
            crate::infrastructure::fs_utils::validate_directory_writable(temp_dir)
                .map_err(|err| AppError::invalid_settings(format!("Invalid temporary parts folder: {err}")))?;
        }

        // 2. If proxy configuration changed, reconfigure runtime transfer executor FIRST
        if candidate.proxy != settings_lock.settings().proxy {
            self.download_transfer.set_proxy(Some(&candidate.proxy))?;
        }

        // 3. Apply other runtime controls
        self.download_transfer
            .set_max_concurrent(candidate.max_concurrent);
        self.download_transfer
            .set_speed_limit(candidate.speed_limit_kbps);
        self.download_transfer.set_settings(candidate.clone());
        let _ = crate::infrastructure::autostart::sync_autostart(candidate.start_with_windows);

        // 4. Only after runtime reconfiguration succeeds, persist settings to disk
        settings_lock.commit_update(candidate)
    }
}
