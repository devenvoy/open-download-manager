use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
    sync::Notify,
};
use url::Url;
use uuid::Uuid;

use crate::{
    application::{
        download_history_service::DownloadHistoryService,
        error::{AppError, AppResult},
    },
    domain::{
        download::{sanitize_filename, DownloadRecord, DownloadRequest, DownloadStatus},
        settings::Settings,
    },
};

#[derive(Clone)]
pub struct DownloadTransferService {
    history: DownloadHistoryService,
    client: reqwest::Client,
    concurrency: DownloadConcurrencyLimiter,
}

#[derive(Clone)]
struct DownloadConcurrencyLimiter {
    state: Arc<DownloadConcurrencyState>,
}

struct DownloadConcurrencyState {
    active: Mutex<usize>,
    max_concurrent: AtomicUsize,
    changed: Notify,
}

struct DownloadConcurrencyPermit {
    state: Arc<DownloadConcurrencyState>,
}

impl DownloadConcurrencyLimiter {
    fn new(max_concurrent: u8) -> Self {
        Self {
            state: Arc::new(DownloadConcurrencyState {
                active: Mutex::new(0),
                max_concurrent: AtomicUsize::new(usize::from(max_concurrent)),
                changed: Notify::new(),
            }),
        }
    }

    async fn acquire(&self) -> AppResult<DownloadConcurrencyPermit> {
        loop {
            let changed = self.state.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();

            let acquired = {
                let mut active = self
                    .state
                    .active
                    .lock()
                    .map_err(|_| AppError::storage("Download queue is unavailable."))?;
                if *active < self.state.max_concurrent.load(Ordering::Acquire) {
                    *active += 1;
                    true
                } else {
                    false
                }
            };
            if acquired {
                return Ok(DownloadConcurrencyPermit {
                    state: self.state.clone(),
                });
            }
            changed.await;
        }
    }

    fn set_max_concurrent(&self, value: u8) {
        self.state
            .max_concurrent
            .store(usize::from(value), Ordering::Release);
        self.state.changed.notify_waiters();
    }
}

impl Drop for DownloadConcurrencyPermit {
    fn drop(&mut self) {
        match self.state.active.lock() {
            Ok(mut active) => *active -= 1,
            Err(error) => {
                eprintln!("Download queue state was poisoned; recovering its permit count.");
                *error.into_inner() -= 1;
            }
        }
        self.state.changed.notify_one();
    }
}

impl DownloadTransferService {
    pub fn new(history: DownloadHistoryService, max_concurrent: u8) -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|error| {
                AppError::download(format!("Could not initialize HTTP client: {error}"))
            })?;
        Ok(Self {
            history,
            client,
            concurrency: DownloadConcurrencyLimiter::new(max_concurrent),
        })
    }

    pub fn add(&self, request: DownloadRequest, settings: &Settings) -> AppResult<Vec<String>> {
        let default_directory = request
            .category
            .as_ref()
            .and_then(|category| settings.category_directories.get(category))
            .cloned()
            .unwrap_or_else(|| settings.default_directory.clone());
        let request = request
            .validate(default_directory)
            .map_err(AppError::invalid_download)?;
        let directory = request
            .directory
            .to_str()
            .ok_or_else(|| {
                AppError::invalid_download(
                    "The download folder cannot be represented as a supported path.",
                )
            })?
            .to_owned();
        let mut records = Vec::with_capacity(request.urls.len());
        for (index, url) in request.urls.iter().enumerate() {
            let filename = request
                .filename
                .clone()
                .unwrap_or_else(|| filename_from_url(url));
            records.push(DownloadRecord {
                id: Uuid::new_v4().to_string(),
                url: url.as_str().to_owned(),
                filename,
                status: DownloadStatus::Queued,
                downloaded: 0,
                total: None,
                speed: 0,
                created_at: chrono::Utc::now().to_rfc3339(),
                needs_review: false,
                scheduled_at: None,
                description: request.description.clone(),
                category: request.category.clone(),
                directory: Some(directory.clone()),
                path: None,
                last_try: None,
                error: None,
                notice: (request.urls.len() > 1)
                    .then(|| format!("Address {} of {}.", index + 1, request.urls.len())),
                estimated: false,
                kind: Some("http".to_owned()),
                resumable: Some(false),
                duration: None,
            });
        }

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

    pub fn set_max_concurrent(&self, value: u8) -> AppResult<()> {
        self.concurrency.set_max_concurrent(value);
        Ok(())
    }

    fn spawn_download(&self, id: String) {
        let service = self.clone();
        tokio::spawn(async move {
            if let Err(error) = service.run_download(&id).await {
                service.cleanup_partial(&id).await;
                eprintln!("Download {id} failed: {}", error.message);
                service.fail_download(&id, error.message);
            }
        });
    }

    async fn run_download(&self, id: &str) -> AppResult<()> {
        let record = self.history.find_download(id)?;
        let url = Url::parse(&record.url)
            .map_err(|_| AppError::download("The stored download address is invalid."))?;
        let directory = PathBuf::from(
            record
                .directory
                .as_deref()
                .ok_or_else(|| AppError::download("The download folder is missing."))?,
        );
        let destination = directory.join(sanitize_filename(&record.filename));
        fs::create_dir_all(&directory).await.map_err(|error| {
            AppError::download(format!("Could not create the download folder: {error}"))
        })?;
        if fs::try_exists(&destination).await.map_err(|error| {
            AppError::download(format!("Could not check the destination file: {error}"))
        })? {
            return Err(AppError::download("A file with that name already exists."));
        }

        let _permit = self.concurrency.acquire().await?;
        self.history.update_download(id, |download| {
            download.status = DownloadStatus::Connecting;
            download.last_try = Some(chrono::Utc::now().to_rfc3339());
            download.error = None;
        })?;

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| AppError::download("Could not connect to the server."))?;
        if !response.status().is_success() {
            return Err(AppError::download(format!(
                "The server returned HTTP {}.",
                response.status().as_u16()
            )));
        }
        let total = response.content_length();
        let temporary = directory.join(format!(".awesome-downloader-{id}.part"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await
            .map_err(|error| {
                AppError::download(format!(
                    "Could not create a temporary download file: {error}"
                ))
            })?;
        self.history.update_download(id, |download| {
            download.status = DownloadStatus::Downloading;
            download.total = total;
            download.path = Some(destination.to_string_lossy().into_owned());
        })?;

        let mut response = response;
        let mut downloaded = 0_u64;
        let mut bytes_since_tick = 0_u64;
        let mut last_tick = Instant::now();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| AppError::download("The server response could not be read."))?
        {
            file.write_all(&chunk).await.map_err(|error| {
                AppError::download(format!("Could not write the download file: {error}"))
            })?;
            downloaded = downloaded.saturating_add(chunk.len() as u64);
            bytes_since_tick = bytes_since_tick.saturating_add(chunk.len() as u64);
            if last_tick.elapsed() >= std::time::Duration::from_millis(250) {
                let elapsed = last_tick.elapsed().as_secs_f64().max(0.001);
                let speed = (bytes_since_tick as f64 / elapsed) as u64;
                self.history.update_progress(id, downloaded, speed)?;
                last_tick = Instant::now();
                bytes_since_tick = 0;
            }
        }
        if total.is_some_and(|expected| expected != downloaded) {
            return Err(AppError::download(
                "The downloaded byte count did not match the server's declared size.",
            ));
        }
        file.flush().await.map_err(|error| {
            AppError::download(format!("Could not finalize the download file: {error}"))
        })?;
        file.sync_all().await.map_err(|error| {
            AppError::download(format!("Could not finalize the download file: {error}"))
        })?;
        drop(file);
        fs::hard_link(&temporary, &destination)
            .await
            .map_err(|error| {
                AppError::download(format!("Could not move the download into place: {error}"))
            })?;
        if let Err(error) = fs::remove_file(&temporary).await {
            eprintln!("Could not remove temporary file for download {id}: {error}");
        }
        self.history.update_download(id, |download| {
            download.status = DownloadStatus::Completed;
            download.downloaded = downloaded;
            if download.total.is_none() {
                download.total = Some(downloaded);
            }
            download.speed = 0;
            download.path = Some(destination.to_string_lossy().into_owned());
        })?;
        Ok(())
    }

    async fn cleanup_partial(&self, id: &str) {
        let directory = match self.history.find_download(id) {
            Ok(download) => download.directory.map(PathBuf::from),
            Err(error) => {
                eprintln!("Could not locate partial download {id}: {}", error.message);
                return;
            }
        };
        let Some(directory) = directory else {
            return;
        };
        let path = directory.join(format!(".awesome-downloader-{id}.part"));
        if let Err(error) = fs::remove_file(path).await {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!("Could not remove partial download {id}: {error}");
            }
        }
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

fn filename_from_url(url: &Url) -> String {
    url.path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|segment| !segment.is_empty())
        .map(sanitize_filename)
        .filter(|filename| !filename.is_empty())
        .unwrap_or_else(|| "download".to_owned())
}
