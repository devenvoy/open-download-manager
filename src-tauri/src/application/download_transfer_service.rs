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
        download::{filename_from_url, DownloadRecord, DownloadRequest, DownloadStatus},
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
            for id in &ids {
                self.spawn_download(id.clone());
            }
        }
        Ok(ids)
    }

    pub fn set_max_concurrent(&self, value: u8) {
        self.concurrency.set_max_concurrent(value);
    }

    fn spawn_download(&self, id: String) {
        let service = self.clone();
        tokio::spawn(async move {
            if let Err(error) = service.run_download(&id).await {
                eprintln!("Download {id} failed: {}", error.message);
                service.fail_download(&id, error.message);
            }
        });
    }

    async fn run_download(&self, id: &str) -> AppResult<()> {
        let record = self.history.find_download(id)?;
        let _permit = self.concurrency.acquire().await?;
        self.history.update_download(id, |download| {
            download.status = DownloadStatus::Connecting;
            download.last_try = Some(chrono::Utc::now().to_rfc3339());
            download.error = None;
        })?;

        let history = self.history.clone();
        let download_id = id.to_owned();
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
        self.history.update_download(id, |download| {
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
