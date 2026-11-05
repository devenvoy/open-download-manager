use serde::Serialize;

use crate::domain::{
    category::DownloadCategory,
    download::{redact_download_url, DownloadRecord, DownloadStatus},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSummary {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub status: DownloadStatus,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed: u64,
    pub created_at: String,
    pub needs_review: bool,
    pub scheduled_at: Option<String>,
    pub description: Option<String>,
    pub category: Option<DownloadCategory>,
    pub directory: Option<String>,
    pub path: Option<String>,
    pub last_try: Option<String>,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub estimated: bool,
    pub kind: Option<String>,
    pub resumable: Option<bool>,
    pub duration: Option<f64>,
}

impl From<DownloadRecord> for DownloadSummary {
    fn from(download: DownloadRecord) -> Self {
        Self {
            id: download.id,
            url: redact_download_url(&download.url),
            filename: download.filename,
            status: download.status,
            downloaded: download.downloaded,
            total: download.total,
            speed: download.speed,
            created_at: download.created_at,
            needs_review: download.needs_review,
            scheduled_at: download.scheduled_at,
            description: download.description,
            category: download.category,
            directory: download.directory,
            path: download.path,
            last_try: download.last_try,
            error: download.error,
            notice: download.notice,
            estimated: download.estimated,
            kind: download.kind,
            resumable: download.resumable,
            duration: download.duration,
        }
    }
}
