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
