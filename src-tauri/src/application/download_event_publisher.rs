// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-05
// Reason: feat(media): implement notifications, file icons, and adaptive dual-stream muxing (Phase E)
// Description: Event publisher broadcasting download lifecycle, progress, and status events across the app.

use crate::{
    application::error::AppResult,
    domain::download::DownloadRecord,
};

pub trait DownloadEventPublisher: Send + Sync {
    fn publish_change(&self) -> AppResult<()>;
    fn publish_complete(&self, _download: &DownloadRecord) -> AppResult<()> {
        Ok(())
    }
}
