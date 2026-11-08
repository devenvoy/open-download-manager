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
    pub category: Option<DownloadCategory>,
    pub description: Option<String>,
    pub queue_only: bool,
    pub review_only: bool,
    pub scheduled_at: Option<String>,
    pub audio_url: Option<String>,
    pub kind: Option<String>,
    pub sabr: Option<NormalizedSabrSession>,
    pub reuse_active: bool,
    pub referer: Option<String>,
    pub youtube_format: Option<String>,
    pub conflict_policy: Option<super::conflict::FileConflictPolicy>,
    pub credentials: Option<super::request::DownloadCredentials>,
}
