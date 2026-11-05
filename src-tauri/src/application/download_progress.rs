#[derive(Clone, Copy, Debug, Default)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub bytes_per_second: u64,
}
