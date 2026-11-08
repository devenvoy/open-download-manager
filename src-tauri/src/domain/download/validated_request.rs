// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Description: Domain-validated download request guaranteed to contain canonical paths and sanitized URLs.

use std::path::PathBuf;

use crate::domain::{
    category::DownloadCategory,
    download::DownloadAddress,
    media::sabr::NormalizedSabrSession,
};

#[derive(Clone, Debug)]
pub struct ValidatedDownloadRequest {
    pub urls: Vec<DownloadAddress>,
    pub filename: Option<String>,
    pub directory: PathBuf,
    pub temp_directory: Option<PathBuf>,
    pub category: Option<DownloadCategory>,
    pub description: Option<String>,
    pub queue_only: bool,
    pub review_only: bool,
    pub scheduled_at: Option<String>,
    pub queue_id: Option<String>,
    pub audio_url: Option<String>,
    pub kind: Option<String>,
    pub sabr: Option<NormalizedSabrSession>,
    pub reuse_active: bool,
    pub referer: Option<String>,
    pub youtube_format: Option<String>,
    pub conflict_policy: Option<super::conflict::FileConflictPolicy>,
    pub credentials: Option<super::request::DownloadCredentials>,
    pub completion_actions: Option<Vec<crate::domain::completion::CompletionAction>>,
}
