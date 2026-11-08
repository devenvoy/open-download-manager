use serde::Serialize;

use crate::domain::{
    category::DownloadCategory,
    download::{redact_download_url, DownloadRecord, DownloadSegmentProgress, DownloadStatus},
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
    #[serde(default)]
    pub temp_directory: Option<String>,
    pub last_try: Option<String>,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub estimated: bool,
    pub kind: Option<String>,
    pub resumable: Option<bool>,
    pub duration: Option<f64>,
    pub connections: u8,
    pub connections_progress: Vec<DownloadSegmentProgress>,
    pub transfer_kind: Option<String>,
    pub referer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_actions: Option<Vec<crate::domain::completion::CompletionAction>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_ledger: Vec<crate::domain::completion::ActionLedgerEntry>,
}

impl From<&DownloadRecord> for DownloadSummary {
    fn from(download: &DownloadRecord) -> Self {
        Self {
            id: download.id.clone(),
            url: redact_download_url(&download.url),
            filename: download.filename.clone(),
            status: download.status.clone(),
            downloaded: download.downloaded,
            total: download.total,
            speed: download.speed,
            created_at: download.created_at.clone(),
            needs_review: download.needs_review,
            scheduled_at: download.scheduled_at.clone(),
            description: download.description.clone(),
            category: download.category.clone(),
            directory: download.directory.clone(),
            path: download.path.clone(),
            temp_directory: download.temp_directory.clone(),
            last_try: download.last_try.clone(),
            error: download.error.clone(),
            notice: download.notice.clone(),
            estimated: download.estimated,
            kind: download.kind.clone(),
            resumable: download.resumable,
            duration: download.duration,
            connections: download.connections,
            connections_progress: download.connections_progress.clone(),
            transfer_kind: download.transfer_kind.clone(),
            referer: download.referer.clone(),
            completion_actions: download.completion_actions.clone(),
            action_ledger: download.action_ledger.clone(),
        }
    }
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
            temp_directory: download.temp_directory,
            last_try: download.last_try,
            error: download.error,
            notice: download.notice,
            estimated: download.estimated,
            kind: download.kind,
            resumable: download.resumable,
            duration: download.duration,
            connections: download.connections,
            connections_progress: download.connections_progress,
            transfer_kind: download.transfer_kind,
            referer: download.referer,
            completion_actions: download.completion_actions,
            action_ledger: download.action_ledger,
        }
    }
}
