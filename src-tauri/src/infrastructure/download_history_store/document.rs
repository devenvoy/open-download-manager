// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-06
// Reason: perf: eliminate eager cloning across history queries and apply zero-copy serialization
// Description: JSON schema document serialization format for persistent download history.

use serde::{Deserialize, Serialize};

use crate::domain::download::{DownloadRecord, SerializableDownloadRecordRef};

#[derive(Deserialize, Serialize)]
pub(super) struct DownloadHistoryDocument {
    pub(super) version: u8,
    #[serde(default)]
    pub(super) downloads: Vec<DownloadRecord>,
}

#[derive(Serialize)]
pub(super) struct BorrowedDownloadHistoryDocument<'a> {
    pub(super) version: u8,
    pub(super) downloads: Vec<SerializableDownloadRecordRef<'a>>,
}
