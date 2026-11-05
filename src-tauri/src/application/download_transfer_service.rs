use std::sync::Arc;

use crate::{
    application::{
        download_concurrency_limiter::DownloadConcurrencyLimiter,
        download_executor::{DownloadExecutor, DownloadProgressHandler},
        download_history_service::DownloadHistoryService,
        download_transfer_policy::{
            HTTP_DOWNLOAD_KIND, NEW_DOWNLOAD_IS_ESTIMATED, NEW_DOWNLOAD_IS_RESUMABLE,
            NEW_DOWNLOAD_NEEDS_REVIEW,
        },
        error::{AppError, AppResult},
    },
    domain::{
        download::{
            filename_from_url, DownloadAction, DownloadRecord, DownloadRequest, DownloadStatus,
        },
        settings::Settings,
    },
};

#[derive(Clone)]
pub struct DownloadTransferService {
    history: DownloadHistoryService,
    executor: Arc<dyn DownloadExecutor>,
    concurrency: Arc<DownloadConcurrencyLimiter>,
}

impl DownloadTransferService {
    pub fn new(
        history: DownloadHistoryService,
        executor: impl DownloadExecutor + 'static,
        max_concurrent: u8,
    ) -> Self {
        Self {
            history,
            executor: Arc::new(executor),
            concurrency: DownloadConcurrencyLimiter::new(max_concurrent),
        }
    }

    pub fn add(&self, request: DownloadRequest, settings: &Settings) -> AppResult<Vec<String>> {
        let default_directory = request
            .category
            .and_then(|category| settings.category_directories.get(&category))
            .cloned()
            .unwrap_or_else(|| settings.default_directory.clone());
        let request = request
            .validate(default_directory)
            .map_err(AppError::invalid_download)?;
        let directory = request.directory.to_str().ok_or_else(|| {
            AppError::invalid_download(
                "The download folder cannot be represented as a supported path.",
            )
        })?;

        let records = request
            .urls
            .iter()
            .enumerate()
            .map(|(index, url)| {
                let filename = request
                    .filename
                    .clone()
                    .unwrap_or_else(|| filename_from_url(url.url()));
                DownloadRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    url: url.url().as_str().to_owned(),
                    filename,
                    status: DownloadStatus::Queued,
                    downloaded: 0,
                    total: None,
                    speed: 0,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    needs_review: NEW_DOWNLOAD_NEEDS_REVIEW,
                    scheduled_at: None,
                    description: request.description.clone(),
                    category: request.category,
                    directory: Some(directory.to_owned()),
                    path: None,
                    last_try: None,
                    error: None,
                    notice: (request.urls.len() > 1)
                        .then(|| format!("Address {} of {}.", index + 1, request.urls.len())),
                    estimated: NEW_DOWNLOAD_IS_ESTIMATED,
                    kind: Some(HTTP_DOWNLOAD_KIND.to_owned()),
                    resumable: Some(NEW_DOWNLOAD_IS_RESUMABLE),
                    duration: None,
                }
            })
            .collect::<Vec<_>>();
        let ids = records
            .iter()
            .map(|record| record.id.clone())
            .collect::<Vec<_>>();
        self.history.add_records(records)?;

        if !request.queue_only {
            self.start_queued_downloads(&ids)?;
        }
        Ok(ids)
    }

    pub fn action(&self, id: &str, action: DownloadAction) -> AppResult<()> {
        match action {
            DownloadAction::Resume => self.start_queued_downloads(&[id.to_owned()]),
            DownloadAction::Pause
            | DownloadAction::Retry
            | DownloadAction::Cancel
            | DownloadAction::Remove
            | DownloadAction::Open
            | DownloadAction::Reveal => Err(AppError::not_migrated(format!(
                "The {} action is not available in the Tauri migration yet.",
                action.as_str()
            ))),
        }
    }

    pub fn set_max_concurrent(&self, value: u8) {
        self.concurrency.set_max_concurrent(value);
    }

    fn start_queued_downloads(&self, ids: &[String]) -> AppResult<()> {
        let records = self
            .history
            .begin_queued_downloads(ids, chrono::Utc::now().to_rfc3339())?;
        for record in records {
            self.spawn_download(record);
        }
        Ok(())
    }

    fn spawn_download(&self, record: DownloadRecord) {
        let id = record.id.clone();
        let service = self.clone();
        tokio::spawn(async move {
            if let Err(error) = service.run_download(record).await {
                eprintln!("Download {id} failed: {}", error.message);
                service.fail_download(&id, error.message);
            }
        });
    }

    async fn run_download(&self, record: DownloadRecord) -> AppResult<()> {
        let id = record.id.clone();
        let _permit = self.concurrency.acquire().await?;

        let history = self.history.clone();
        let download_id = id.clone();
        let progress_handler: DownloadProgressHandler = Arc::new(move |progress| {
            if let Err(error) = history.update_download(&download_id, |download| {
                download.status = DownloadStatus::Downloading;
                download.downloaded = progress.downloaded;
                download.total = progress.total;
                download.speed = progress.bytes_per_second;
            }) {
                eprintln!(
                    "Could not update download progress for {download_id}: {}",
                    error.message
                );
            }
        });
        let result = self.executor.execute(record, progress_handler).await?;
        self.history.update_download(&id, |download| {
            download.status = DownloadStatus::Completed;
            download.downloaded = result.downloaded;
            download.total = result.total;
            download.speed = 0;
            download.path = Some(result.path.to_string_lossy().into_owned());
        })
    }

    fn fail_download(&self, id: &str, message: String) {
        if let Err(error) = self.history.update_download(id, |download| {
            download.status = DownloadStatus::Failed;
            download.speed = 0;
            download.error = Some(message);
        }) {
            eprintln!("Could not save failed download {id}: {}", error.message);
        }
    }
}
