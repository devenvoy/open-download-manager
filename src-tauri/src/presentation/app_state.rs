// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Reason: feat(media): add opt-in advanced browser media adapter (Closes #38)
// Reason: feat(proxy): import system and browser proxy configurations (Closes #39)
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Description: Shared application state holding services, transfer handles, and runtime settings.

use std::sync::{Arc, Mutex};

use crate::{
    application::{
        advanced_integration_service::AdvancedIntegrationService,
        clipboard_service::ClipboardMonitoringService,
        credential_service::CredentialService,
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        error::{AppError, AppResult},
        integration_service::IntegrationService,
        proxy_discovery_service::ProxyDiscoveryService,
        settings_service::SettingsService,
        torrent_service::TorrentService,
        updater_service::UpdateService,
    },
    domain::settings::{DetectedProxyConfig, Settings, SettingsPatch},
    infrastructure::{
        browser::PlatformAdvancedAdapter, file_icons::FileIconProvider, media::ytdlp::YtDlpTools,
        proxy_discovery::SystemProxyDetector,
    },
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
    pub(super) clipboard: Option<Arc<ClipboardMonitoringService>>,
    pub(super) advanced_integration: Arc<AdvancedIntegrationService>,
    pub(super) proxy_discovery: Arc<ProxyDiscoveryService>,
    pub(super) queue_service: Option<Arc<crate::application::queue_service::QueueService>>,
    pub(super) torrents: Arc<TorrentService>,
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
        let advanced_adapter = Arc::new(PlatformAdvancedAdapter::new(std::env::temp_dir()));
        let advanced_integration = Arc::new(AdvancedIntegrationService::new(advanced_adapter));
        let proxy_detector = Arc::new(SystemProxyDetector::new());
        let proxy_discovery = Arc::new(ProxyDiscoveryService::new(proxy_detector));
        Self {
            settings: Mutex::new(settings),
            download_history,
            download_transfer,
            credentials,
            integration,
            file_icons,
            update_service,
            ytdlp: YtDlpTools::new().map(Arc::new),
            clipboard: None,
            advanced_integration,
            proxy_discovery,
            queue_service: None,
            torrents: TorrentService::disabled(),
        }
    }

    pub fn with_queue_service(
        mut self,
        queue_service: Arc<crate::application::queue_service::QueueService>,
    ) -> Self {
        self.download_transfer.set_queue_service(queue_service.clone());
        self.queue_service = Some(queue_service);
        self
    }

    pub fn queue_service(
        &self,
    ) -> AppResult<Arc<crate::application::queue_service::QueueService>> {
        self.queue_service
            .clone()
            .ok_or_else(|| AppError::download("Queue service is unavailable."))
    }

    pub fn with_ytdlp(mut self, ytdlp: Option<Arc<YtDlpTools>>) -> Self {
        self.ytdlp = ytdlp;
        self
    }

    pub fn with_clipboard(mut self, clipboard: Arc<ClipboardMonitoringService>) -> Self {
        self.clipboard = Some(clipboard);
        self
    }

    pub fn with_advanced_integration(
        mut self,
        advanced_integration: Arc<AdvancedIntegrationService>,
    ) -> Self {
        self.advanced_integration = advanced_integration;
        self
    }

    pub fn with_proxy_discovery(
        mut self,
        proxy_discovery: Arc<ProxyDiscoveryService>,
    ) -> Self {
        self.proxy_discovery = proxy_discovery;
        self
    }

    pub fn with_torrents(mut self, torrents: Arc<TorrentService>) -> Self {
        self.torrents = torrents;
        self
    }

    pub fn torrents(&self) -> Arc<TorrentService> {
        self.torrents.clone()
    }

    pub fn apply_detected_proxy(&self, detected: DetectedProxyConfig) -> AppResult<Settings> {
        if !detected.can_import {
            return Err(AppError::invalid_settings(
                "Detected proxy cannot be imported automatically.",
            ));
        }
        let proxy_to_import = detected.import_settings.ok_or_else(|| {
            AppError::invalid_settings("No importable proxy settings were found.")
        })?;
        proxy_to_import
            .validate()
            .map_err(|e| AppError::invalid_settings(e.message))?;

        let patch = SettingsPatch {
            proxy: Some(proxy_to_import),
            ..Default::default()
        };
        self.update_settings(patch)
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
        if let Some(ref clipboard) = self.clipboard {
            clipboard.set_settings(candidate.clipboard_monitoring.clone());
        }

        // 4. Only after runtime reconfiguration succeeds, persist settings to disk
        settings_lock.commit_update(candidate)
    }
}
