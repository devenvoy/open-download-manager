use crate::domain::download::DownloadSegmentProgress;

#[derive(Clone, Debug, Default)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub bytes_per_second: u64,
    pub connections_progress: Vec<DownloadSegmentProgress>,
    pub notice: Option<String>,
}
