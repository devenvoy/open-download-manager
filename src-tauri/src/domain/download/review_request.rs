// Created: 2026-11-05
// Reason: feat: implement Phase C browser integration UI, scanner, and review confirmation workflow
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Description: Download confirmation review payload used by frontend confirmation dialogs.

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
