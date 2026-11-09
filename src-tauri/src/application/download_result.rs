// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-05
// Reason: refactor: modularize native download architecture
// Description: Transfer completion outcome model carrying final file path and total byte count.

use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct DownloadResult {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub path: PathBuf,
}
