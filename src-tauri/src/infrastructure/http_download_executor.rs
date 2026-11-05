use std::{path::PathBuf, sync::Arc, time::Instant};

use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};
use url::Url;

use crate::{
    application::{
        download_executor::{DownloadExecution, DownloadExecutor, DownloadProgressHandler},
        download_progress::DownloadProgress,
        download_result::DownloadResult,
        error::{AppError, AppResult},
    },
    domain::download::{sanitize_filename, DownloadRecord},
};

mod policy;

use self::policy::{
    CONNECT_TIMEOUT_SECONDS, MAX_REDIRECTS, PROGRESS_EVENT_INTERVAL_MILLISECONDS,
    TEMPORARY_DOWNLOAD_PREFIX,
};

#[derive(Clone)]
pub struct HttpDownloadExecutor {
    client: reqwest::Client,
}

impl HttpDownloadExecutor {
    pub fn new() -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(CONNECT_TIMEOUT_SECONDS))
            .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
            .build()
            .map_err(|error| {
                AppError::download(format!("Could not initialize HTTP client: {error}"))
            })?;
        Ok(Self { client })
    }

    async fn transfer(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
    ) -> AppResult<DownloadResult> {
        let url = Url::parse(&download.url)
            .map_err(|_| AppError::download("The stored download address is invalid."))?;
        let directory = PathBuf::from(
            download
                .directory
                .as_deref()
                .ok_or_else(|| AppError::download("The download folder is missing."))?,
        );
        let filename = sanitize_filename(&download.filename);
        if filename.is_empty() {
            return Err(AppError::download("The download file name is invalid."));
        }
        let destination = directory.join(filename);
        fs::create_dir_all(&directory).await.map_err(|error| {
            AppError::download(format!("Could not create the download folder: {error}"))
        })?;
        if fs::try_exists(&destination).await.map_err(|error| {
            AppError::download(format!("Could not check the destination file: {error}"))
        })? {
            return Err(AppError::download("A file with that name already exists."));
        }

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
        let temporary = directory.join(format!("{TEMPORARY_DOWNLOAD_PREFIX}{}.part", download.id));
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

        progress_handler(DownloadProgress {
            downloaded: 0,
            total,
            bytes_per_second: 0,
        });
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
            let chunk_size = u64::try_from(chunk.len())
                .map_err(|_| AppError::download("The downloaded chunk size is invalid."))?;
            downloaded = downloaded
                .checked_add(chunk_size)
                .ok_or_else(|| AppError::download("The download size exceeded its limit."))?;
            bytes_since_tick = bytes_since_tick
                .checked_add(chunk_size)
                .ok_or_else(|| AppError::download("The download progress exceeded its limit."))?;
            if last_tick.elapsed()
                >= std::time::Duration::from_millis(PROGRESS_EVENT_INTERVAL_MILLISECONDS)
            {
                let elapsed = last_tick.elapsed().as_secs_f64().max(0.001);
                let speed = (bytes_since_tick as f64 / elapsed) as u64;
                progress_handler(DownloadProgress {
                    downloaded,
                    total,
                    bytes_per_second: speed,
                });
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
            eprintln!(
                "Could not remove temporary file for download {}: {error}",
                download.id
            );
        }
        Ok(DownloadResult {
            downloaded,
            total: total.or(Some(downloaded)),
            path: destination,
        })
    }
}

impl DownloadExecutor for HttpDownloadExecutor {
    fn execute(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
    ) -> DownloadExecution {
        let executor = self.clone();
        Box::pin(async move {
            let temporary = download.directory.as_deref().map(|directory| {
                PathBuf::from(directory)
                    .join(format!("{TEMPORARY_DOWNLOAD_PREFIX}{}.part", download.id))
            });
            let result = executor
                .transfer(download, Arc::clone(&progress_handler))
                .await;
            if result.is_err() {
                if let Some(temporary) = temporary {
                    if let Err(error) = fs::remove_file(temporary).await {
                        if error.kind() != std::io::ErrorKind::NotFound {
                            eprintln!("Could not remove a partial download: {error}");
                        }
                    }
                }
            }
            result
        })
    }
}
