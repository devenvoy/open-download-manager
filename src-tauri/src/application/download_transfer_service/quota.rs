// Created: 2026-11-10
// Reason: refactor: modularize monolithic download_transfer_service into focused coordinators
// Description: Rolling window download quota enforcement, pause, resume, and persistence.

use crate::domain::download::DownloadStatus;
use crate::application::download_executor::DownloadStopReason;

use super::DownloadTransferService;

impl DownloadTransferService {
    pub fn pause_active_for_quota(&self, explanation: &str) {
        let active_ids = match self.history.with_downloads(|downloads| {
            downloads
                .iter()
                .filter(|d| {
                    matches!(
                        d.status,
                        DownloadStatus::Downloading
                            | DownloadStatus::Connecting
                            | DownloadStatus::Assembling
                    )
                })
                .map(|d| d.id.clone())
                .collect::<Vec<_>>()
        }) {
            Ok(ids) => ids,
            Err(_) => return,
        };

        for id in active_ids {
            self.quota_service.mark_quota_paused(&id);
            let _ = self.stop_download(&id, DownloadStopReason::Pause);
            let _ = self.history.update_active_download(&id, |d| {
                d.notice = Some(explanation.to_string());
            });
        }
    }

    pub fn resume_quota_paused_downloads(&self) {
        let paused_ids = self.quota_service.drain_quota_paused();
        for id in paused_ids {
            let _ = self.history.update_active_download(&id, |d| {
                d.notice = None;
                d.status = DownloadStatus::Queued;
            });
        }
        self.pump_queue();
    }

    pub fn persist_quota_state(&self) {
        if let (Some(mut settings), Some(repo)) = (
            self.settings.read().ok().and_then(|g| g.clone()),
            self.settings_repository.read().ok().and_then(|g| g.clone()),
        ) {
            settings.window_limit = self.quota_service.current_limit();
            let _ = repo.save(&settings);
        }
    }
}
