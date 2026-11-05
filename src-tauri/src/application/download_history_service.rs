use std::sync::{Arc, Mutex};

use crate::{
    application::{
        download_event_publisher::DownloadEventPublisher,
        download_history_policy::MAX_ACTIVE_DOWNLOADS,
        download_history_repository::DownloadHistoryRepository,
        error::{AppError, AppResult},
    },
    domain::download::{DownloadRecord, DownloadStatus},
};

pub struct DownloadHistoryService {
    downloads: Arc<Mutex<Vec<DownloadRecord>>>,
    repository: Arc<dyn DownloadHistoryRepository>,
    events: Option<Arc<dyn DownloadEventPublisher>>,
}

impl Clone for DownloadHistoryService {
    fn clone(&self) -> Self {
        Self {
            downloads: self.downloads.clone(),
            repository: self.repository.clone(),
            events: self.events.clone(),
        }
    }
}

impl DownloadHistoryService {
    pub fn new(repository: impl DownloadHistoryRepository + 'static) -> AppResult<Self> {
        Self::with_optional_events(repository, None)
    }

    pub fn with_events(
        repository: impl DownloadHistoryRepository + 'static,
        events: Arc<dyn DownloadEventPublisher>,
    ) -> AppResult<Self> {
        Self::with_optional_events(repository, Some(events))
    }

    fn with_optional_events(
        repository: impl DownloadHistoryRepository + 'static,
        events: Option<Arc<dyn DownloadEventPublisher>>,
    ) -> AppResult<Self> {
        let initial_downloads = repository.load()?;
        Ok(Self {
            downloads: Arc::new(Mutex::new(initial_downloads)),
            repository: Arc::new(repository),
            events,
        })
    }

    pub fn downloads(&self) -> AppResult<Vec<DownloadRecord>> {
        self.downloads
            .lock()
            .map(|downloads| downloads.clone())
            .map_err(|_| AppError::storage("Download history is unavailable."))
    }

    pub(crate) fn add_records(&self, records: Vec<DownloadRecord>) -> AppResult<()> {
        let mut downloads = self
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
        if let Err(error) = self.repository.save(&downloads) {
            downloads.truncate(old_len);
            return Err(error);
        }
        drop(downloads);
        self.publish_change();
        Ok(())
    }

    pub(crate) fn find_download(&self, id: &str) -> AppResult<DownloadRecord> {
        let downloads = self
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
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        let index = downloads
            .iter()
            .position(|download| download.id == id)
            .ok_or_else(|| AppError::download("The download no longer exists."))?;
        let previous = downloads[index].clone();
        update(&mut downloads[index]);
        if let Err(error) = self.repository.save(&downloads) {
            downloads[index] = previous;
            return Err(error);
        }
        drop(downloads);
        self.publish_change();
        Ok(())
    }

    fn publish_change(&self) {
        if let Some(events) = &self.events {
            if let Err(error) = events.publish_change() {
                eprintln!(
                    "Could not notify the renderer about download changes: {}",
                    error.message
                );
            }
        }
    }
}
