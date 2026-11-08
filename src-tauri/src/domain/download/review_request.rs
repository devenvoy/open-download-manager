use serde::Deserialize;

use crate::domain::category::DownloadCategory;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDownloadRequest {
    pub id: String,
    pub filename: Option<String>,
    pub directory: Option<String>,
    #[serde(default)]
    pub temp_directory: Option<String>,
    pub category: Option<DownloadCategory>,
    pub description: Option<String>,
    #[serde(default)]
    pub queue_only: Option<bool>,
    pub scheduled_at: Option<String>,
    #[serde(default)]
    pub conflict_policy: Option<super::conflict::FileConflictPolicy>,
    #[serde(default)]
    pub credentials: Option<super::request::DownloadCredentials>,
}
