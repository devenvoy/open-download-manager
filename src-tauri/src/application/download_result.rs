use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct DownloadResult {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub path: PathBuf,
}
