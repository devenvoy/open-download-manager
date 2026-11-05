use serde::{Deserialize, Serialize};

use crate::domain::download::DownloadRecord;

#[derive(Deserialize, Serialize)]
pub(super) struct DownloadHistoryDocument {
    pub(super) version: u8,
    #[serde(default)]
    pub(super) downloads: Vec<DownloadRecord>,
}
