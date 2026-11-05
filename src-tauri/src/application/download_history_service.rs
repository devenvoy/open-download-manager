use crate::{application::error::AppResult, domain::download::DownloadRecord};

pub trait DownloadHistoryRepository: Send + Sync {
    fn load(&self) -> AppResult<Vec<DownloadRecord>>;
}

#[derive(Debug)]
pub struct DownloadHistoryService {
    downloads: Vec<DownloadRecord>,
}

impl DownloadHistoryService {
    pub fn new(repository: impl DownloadHistoryRepository) -> AppResult<Self> {
        Ok(Self {
            downloads: repository.load()?,
        })
    }

    pub fn downloads(&self) -> &[DownloadRecord] {
        &self.downloads
    }
}
