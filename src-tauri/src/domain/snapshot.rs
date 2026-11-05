use serde::Serialize;

use crate::domain::settings::Settings;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub downloads: Vec<DownloadSummary>,
    pub settings: Settings,
    pub integration: IntegrationStatus,
    pub file_icons: Vec<FileIcon>,
}

impl Snapshot {
    pub fn empty(settings: Settings) -> Self {
        Self {
            downloads: Vec::new(),
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
    pub status: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed: u64,
    pub created_at: String,
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
