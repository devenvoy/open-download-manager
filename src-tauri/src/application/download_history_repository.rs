// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-05
// Reason: refactor: modularize native download architecture
// Description: Abstract repository trait defining persistence operations for download records.

use crate::{application::error::AppResult, domain::download::DownloadRecord};

pub trait DownloadHistoryRepository: Send + Sync {
    fn load(&self) -> AppResult<Vec<DownloadRecord>>;
    fn save(&self, downloads: &[DownloadRecord]) -> AppResult<()>;
}
