// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-08
// Reason: fix(transfer): preserve HTTP resume validator type across interrupted downloads (Closes #25) (#40)
// Description: Transfer progress metrics model encapsulating bytes downloaded, speeds, and segment statuses.

use crate::domain::download::{DownloadSegmentProgress, TransferValidator};

#[derive(Clone, Debug, Default)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub bytes_per_second: u64,
    pub connections_progress: Vec<DownloadSegmentProgress>,
    pub notice: Option<String>,
    pub transfer_kind: Option<String>,
    pub effective_concurrency: Option<u8>,
    pub etag: Option<String>,
    pub validator: Option<TransferValidator>,
    pub filename: Option<String>,
}
