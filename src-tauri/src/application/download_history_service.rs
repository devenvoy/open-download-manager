use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};

use crate::{
    application::{
        download_event_publisher::DownloadEventPublisher,
        download_executor::DownloadStopReason,
        download_history_policy::MAX_ACTIVE_DOWNLOADS,
        download_history_repository::DownloadHistoryRepository,
        error::{AppError, AppResult},
    },
    domain::download::{
        sanitize_filename, DownloadRecord, DownloadStatus, ReviewDownloadRequest,
    },
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

    pub fn with_downloads<R>(&self, f: impl FnOnce(&[DownloadRecord]) -> R) -> AppResult<R> {
        let downloads = self
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        Ok(f(&downloads))
    }

    pub(crate) fn next_queued_download_ids(&self, limit: usize) -> AppResult<Vec<String>> {
        let downloads = self
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        let ids = downloads
            .iter()
            .filter(|d| d.status == DownloadStatus::Queued && !d.needs_review)
            .take(limit)
            .map(|d| d.id.clone())
            .collect();
        Ok(ids)
    }

    pub(crate) fn active_download_count(&self) -> AppResult<usize> {
        let downloads = self
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        let count = downloads
            .iter()
            .filter(|d| matches!(d.status, DownloadStatus::Connecting | DownloadStatus::Downloading | DownloadStatus::Assembling))
            .count();
        Ok(count)
    }

    pub fn download(&self, id: &str) -> AppResult<DownloadRecord> {
        self.downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?
            .iter()
            .find(|download| download.id == id)
            .cloned()
            .ok_or_else(|| AppError::download("The download no longer exists."))
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
                        | DownloadStatus::Scheduled
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

    pub(crate) fn begin_download(
        &self,
        id: &str,
        retry: bool,
        last_try: String,
    ) -> AppResult<DownloadRecord> {
        let mut downloads = self
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        let index = downloads
            .iter()
            .position(|download| download.id == id)
            .ok_or_else(|| AppError::download("The download no longer exists."))?;
        if downloads[index].needs_review {
            return Err(AppError::download(
                "Review this browser download before starting it.",
            ));
        }
        let status = &downloads[index].status;
        let can_start = if retry {
            matches!(status, DownloadStatus::Failed | DownloadStatus::Cancelled)
        } else {
            matches!(status, DownloadStatus::Queued | DownloadStatus::Paused)
        };
        if !can_start {
            return Err(AppError::download(
                "This download cannot be started in its current state.",
            ));
        }

        let previous = downloads[index].clone();
        downloads[index].status = DownloadStatus::Connecting;
        downloads[index].last_try = Some(last_try);
        downloads[index].error = None;
        downloads[index].speed = 0;
        if retry {
            downloads[index].downloaded = 0;
            downloads[index].total = None;
        }
        if let Err(error) = self.repository.save(&downloads) {
            downloads[index] = previous;
            return Err(error);
        }
        let started = downloads[index].clone();
        drop(downloads);
        self.publish_change();
        Ok(started)
    }

    pub(crate) fn begin_due_scheduled_downloads(
        &self,
        now: DateTime<Utc>,
        last_try: String,
    ) -> AppResult<Vec<DownloadRecord>> {
        let mut downloads = self
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        let due_indexes = downloads
            .iter()
            .enumerate()
            .filter_map(|(index, download)| {
                if download.status != DownloadStatus::Scheduled {
                    return None;
                }
                let due = download
                    .scheduled_at
                    .as_deref()
                    .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                    .is_some_and(|scheduled| scheduled.with_timezone(&Utc) <= now);
                due.then_some(index)
            })
            .collect::<Vec<_>>();
        if due_indexes.is_empty() {
            return Ok(Vec::new());
        }

        let previous = due_indexes
            .iter()
            .map(|index| downloads[*index].clone())
            .collect::<Vec<_>>();
        for index in &due_indexes {
            downloads[*index].status = DownloadStatus::Connecting;
            downloads[*index].last_try = Some(last_try.clone());
            downloads[*index].error = None;
            downloads[*index].speed = 0;
        }
        if let Err(error) = self.repository.save(&downloads) {
            for (index, previous) in due_indexes.iter().zip(previous) {
                downloads[*index] = previous;
            }
            return Err(error);
        }
        let started = due_indexes
            .iter()
            .map(|index| downloads[*index].clone())
            .collect();
        drop(downloads);
        self.publish_change();
        Ok(started)
    }

    pub(crate) fn review_download(
        &self,
        request: &ReviewDownloadRequest,
    ) -> AppResult<DownloadRecord> {
        let mut downloads = self
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        let index = downloads
            .iter()
            .position(|download| download.id == request.id)
            .ok_or_else(|| AppError::download("The download no longer exists."))?;

        if !downloads[index].needs_review {
            return Err(AppError::download(
                "This download is not waiting for confirmation.",
            ));
        }

        if let Some(dir) = &request.directory {
            let p = std::path::Path::new(dir);
            if !p.is_absolute() {
                return Err(AppError::invalid_download("Choose an absolute save folder."));
            }
        }

        if let Some(desc) = &request.description {
            if desc.chars().count() > 4096 {
                return Err(AppError::invalid_download("Description is too long."));
            }
        }

        let scheduled_time = if let Some(scheduled) = &request.scheduled_at {
            let parsed = DateTime::parse_from_rfc3339(scheduled)
                .map_err(|_| AppError::invalid_download("The scheduled time is invalid."))?;
            if parsed <= Utc::now() {
                return Err(AppError::invalid_download(
                    "The scheduled time must be in the future.",
                ));
            }
            Some(parsed.to_rfc3339())
        } else {
            None
        };

        let previous = downloads[index].clone();

        if let Some(filename) = &request.filename {
            downloads[index].filename = sanitize_filename(filename);
        }
        if let Some(directory) = &request.directory {
            downloads[index].directory = Some(directory.clone());
        }
        if let Some(category) = request.category {
            downloads[index].category = Some(category);
        }
        if let Some(desc) = &request.description {
            downloads[index].description = Some(desc.clone());
        }

        downloads[index].scheduled_at = scheduled_time.clone();
        downloads[index].needs_review = false;

        let queue_only = request.queue_only.unwrap_or(false);
        if queue_only {
            downloads[index].status = DownloadStatus::Paused;
        } else if scheduled_time.is_some() {
            downloads[index].status = DownloadStatus::Scheduled;
        } else {
            downloads[index].status = DownloadStatus::Queued;
        }

        if let Err(error) = self.repository.save(&downloads) {
            downloads[index] = previous;
            return Err(error);
        }

        let updated = downloads[index].clone();
        drop(downloads);
        self.publish_change();
        Ok(updated)
    }

    pub(crate) fn stop_download(
        &self,
        id: &str,
        reason: DownloadStopReason,
    ) -> AppResult<DownloadRecord> {
        let mut downloads = self
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        let index = downloads
            .iter()
            .position(|download| download.id == id)
            .ok_or_else(|| AppError::download("The download no longer exists."))?;
        let status = &downloads[index].status;
        let can_stop = match reason {
            DownloadStopReason::Pause => matches!(
                status,
                DownloadStatus::Queued | DownloadStatus::Connecting | DownloadStatus::Downloading
            ),
            DownloadStopReason::Cancel => !matches!(
                status,
                DownloadStatus::Completed | DownloadStatus::Cancelled
            ),
        };
        if !can_stop {
            return Err(AppError::download(
                "This download cannot be stopped in its current state.",
            ));
        }

        let previous = downloads[index].clone();
        match reason {
            DownloadStopReason::Pause => {
                downloads[index].status = DownloadStatus::Paused;
                downloads[index].speed = 0;
            }
            DownloadStopReason::Cancel => {
                downloads[index].status = DownloadStatus::Cancelled;
                downloads[index].needs_review = false;
                downloads[index].downloaded = 0;
                downloads[index].total = None;
                downloads[index].speed = 0;
                downloads[index].error = None;
            }
        }
        if let Err(error) = self.repository.save(&downloads) {
            downloads[index] = previous;
            return Err(error);
        }
        let stopped = downloads[index].clone();
        drop(downloads);
        self.publish_change();
        Ok(stopped)
    }

    pub fn remove_download(&self, id: &str) -> AppResult<DownloadRecord> {
        let mut downloads = self
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        let index = downloads
            .iter()
            .position(|download| download.id == id)
            .ok_or_else(|| AppError::download("The download no longer exists."))?;
        let removed = downloads.remove(index);
        if let Err(error) = self.repository.save(&downloads) {
            downloads.insert(index, removed);
            return Err(error);
        }
        drop(downloads);
        self.publish_change();
        Ok(removed)
    }

    pub fn remove_completed_downloads(&self) -> AppResult<Vec<String>> {
        let mut downloads = self
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        if !downloads.iter().any(|d| d.status == DownloadStatus::Completed) {
            return Ok(Vec::new());
        }
        let mut completed = Vec::new();
        let mut remaining = Vec::with_capacity(downloads.len());
        for (index, download) in downloads.drain(..).enumerate() {
            if download.status == DownloadStatus::Completed {
                completed.push((index, download));
            } else {
                remaining.push(download);
            }
        }
        if let Err(error) = self.repository.save(&remaining) {
            let mut all = remaining;
            for (index, item) in completed {
                let insert_at = index.min(all.len());
                all.insert(insert_at, item);
            }
            *downloads = all;
            return Err(error);
        }
        *downloads = remaining;
        let removed_ids = completed.into_iter().map(|(_, d)| d.id).collect();
        drop(downloads);
        self.publish_change();
        Ok(removed_ids)
    }

    pub(crate) fn update_active_download(
        &self,
        id: &str,
        update: impl FnOnce(&mut DownloadRecord),
    ) -> AppResult<bool> {
        let mut downloads = self
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        let index = downloads
            .iter()
            .position(|download| download.id == id)
            .ok_or_else(|| AppError::download("The download no longer exists."))?;
        if !matches!(
            downloads[index].status,
            DownloadStatus::Connecting | DownloadStatus::Downloading
        ) {
            return Ok(false);
        }
        let previous = downloads[index].clone();
        update(&mut downloads[index]);
        if let Err(error) = self.repository.save(&downloads) {
            downloads[index] = previous;
            return Err(error);
        }
        drop(downloads);
        self.publish_change();
        Ok(true)
    }

    pub fn notify_complete(&self, id: &str) -> AppResult<()> {
        let downloads = self
            .downloads
            .lock()
            .map_err(|_| AppError::storage("Download history is unavailable."))?;
        if let Some(download) = downloads.iter().find(|d| d.id == id) {
            let record = download.clone();
            drop(downloads);
            if let Some(events) = &self.events {
                if let Err(error) = events.publish_complete(&record) {
                    eprintln!("Could not notify completion: {}", error.message);
                }
            }
        }
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
