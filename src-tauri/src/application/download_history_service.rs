use std::sync::{Arc, Mutex};

use crate::{
    application::error::{AppError, AppResult},
    domain::download::{DownloadRecord, DownloadStatus},
};

const MAX_ACTIVE_DOWNLOADS: usize = 500;

pub trait DownloadHistoryRepository: Send + Sync {
    fn load(&self) -> AppResult<Vec<DownloadRecord>>;
    fn save(&self, downloads: &[DownloadRecord]) -> AppResult<()>;
}

pub trait DownloadEventPublisher: Send + Sync {
    fn publish_change(&self) -> AppResult<()>;
}

struct NoopEventPublisher;

impl DownloadEventPublisher for NoopEventPublisher {
    fn publish_change(&self) -> AppResult<()> {
        Ok(())
    }
}

struct DownloadHistoryState {
    downloads: Mutex<Vec<DownloadRecord>>,
    repository: Box<dyn DownloadHistoryRepository>,
    events: Arc<dyn DownloadEventPublisher>,
}

#[derive(Clone)]
pub struct DownloadHistoryService {
    state: Arc<DownloadHistoryState>,
}

impl DownloadHistoryService {
    pub fn new(repository: impl DownloadHistoryRepository + 'static) -> AppResult<Self> {
        Self::with_events(repository, Arc::new(NoopEventPublisher))
    }

    pub fn with_events(
        repository: impl DownloadHistoryRepository + 'static,
        events: Arc<dyn DownloadEventPublisher>,
    ) -> AppResult<Self> {
        let downloads = repository.load()?;
        Ok(Self {
            state: Arc::new(DownloadHistoryState {
                downloads: Mutex::new(downloads),
                repository: Box::new(repository),
                events,
            }),
        })
    }

    pub fn downloads(&self) -> AppResult<Vec<DownloadRecord>> {
        self.state
            .downloads
            .lock()
            .map(|downloads| downloads.clone())
            .map_err(|_| AppError::storage("Download history is unavailable."))
    }

    pub(crate) fn add_records(&self, records: Vec<DownloadRecord>) -> AppResult<()> {
        let mut downloads = self
            .state
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        let active_count = downloads
            .iter()
            .filter(|download| {
                matches!(
                    download.status,
                    DownloadStatus::Queued
                        | DownloadStatus::Connecting
                        | DownloadStatus::Downloading
                        | DownloadStatus::Assembling
                )
            })
            .count();
        if active_count.saturating_add(records.len()) > MAX_ACTIVE_DOWNLOADS {
            return Err(AppError::invalid_download(format!(
                "The active download queue is limited to {MAX_ACTIVE_DOWNLOADS} items."
            )));
        }
        let old_len = downloads.len();
        downloads.extend(records);
        if let Err(error) = self.persist(&downloads) {
            downloads.truncate(old_len);
            return Err(error);
        }
        drop(downloads);
        self.publish_change();
        Ok(())
    }

    pub(crate) fn find_download(&self, id: &str) -> AppResult<DownloadRecord> {
        let downloads = self
            .state
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        downloads
            .iter()
            .find(|download| download.id == id)
            .cloned()
            .ok_or_else(|| AppError::download("The download no longer exists."))
    }

    pub(crate) fn update_download(
        &self,
        id: &str,
        update: impl FnOnce(&mut DownloadRecord),
    ) -> AppResult<()> {
        let mut downloads = self
            .state
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        let index = downloads
            .iter()
            .position(|download| download.id == id)
            .ok_or_else(|| AppError::download("The download no longer exists."))?;
        let previous = downloads[index].clone();
        update(&mut downloads[index]);
        if let Err(error) = self.persist(&downloads) {
            downloads[index] = previous;
            return Err(error);
        }
        drop(downloads);
        self.publish_change();
        Ok(())
    }

    pub(crate) fn update_progress(&self, id: &str, downloaded: u64, speed: u64) -> AppResult<()> {
        let mut downloads = self
            .state
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        let download = downloads
            .iter_mut()
            .find(|download| download.id == id)
            .ok_or_else(|| AppError::download("The download no longer exists."))?;
        download.downloaded = downloaded;
        download.speed = speed;
        drop(downloads);
        self.publish_change();
        Ok(())
    }

    pub(crate) fn publish_change(&self) {
        if let Err(error) = self.state.events.publish_change() {
            eprintln!(
                "Could not notify the renderer about download changes: {}",
                error.message
            );
        }
    }

    fn persist(&self, downloads: &[DownloadRecord]) -> AppResult<()> {
        self.state.repository.save(downloads)
    }
}
