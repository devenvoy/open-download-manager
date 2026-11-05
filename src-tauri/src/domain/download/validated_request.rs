use std::path::PathBuf;

use crate::domain::{category::DownloadCategory, download::DownloadAddress};

#[derive(Clone, Debug)]
pub struct ValidatedDownloadRequest {
    pub urls: Vec<DownloadAddress>,
    pub filename: Option<String>,
    pub directory: PathBuf,
    pub category: Option<DownloadCategory>,
    pub description: Option<String>,
    pub queue_only: bool,
    pub scheduled_at: Option<String>,
}
