use crate::{application::error::AppResult, domain::download::DownloadRecord};

pub trait DownloadHistoryRepository: Send + Sync {
    fn load(&self) -> AppResult<Vec<DownloadRecord>>;
    fn save(&self, downloads: &[DownloadRecord]) -> AppResult<()>;
}
