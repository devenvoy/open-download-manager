use serde::Serialize;

use crate::domain::{
    download::{redact_download_url, DownloadRecord, DownloadStatus},
    settings::Settings,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub downloads: Vec<DownloadSummary>,
    pub settings: Settings,
    pub integration: IntegrationStatus,
    pub file_icons: Vec<FileIcon>,
}

impl Snapshot {
    pub fn new(settings: Settings, downloads: Vec<DownloadRecord>) -> Self {
        Self {
            downloads: downloads.into_iter().map(DownloadSummary::from).collect(),
            settings,
            integration: IntegrationStatus {
                browsers: Vec::new(),
                connection_error: Some(
                    "Browser handoff has not been migrated to Tauri yet.".to_owned(),
                ),
            },
            file_icons: Vec::new(),
        }
    }
}

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
    pub category: Option<String>,
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationStatus {
    pub browsers: Vec<BrowserStatus>,
    pub connection_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserStatus {
    pub name: String,
    pub installed: bool,
    pub connected: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIcon {
    pub extension: String,
    pub data_url: String,
}
