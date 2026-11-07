use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
};

use tokio::fs;
use url::Url;

use crate::{
    application::{
        download_executor::{
            DownloadControl, DownloadExecution, DownloadExecutor, DownloadProgressHandler,
        },
        download_progress::DownloadProgress,
        download_rate_limiter::DownloadRateLimiter,
        download_result::DownloadResult,
        error::{AppError, AppResult},
    },
    domain::{
        download::{sanitize_filename, DownloadRecord},
        settings::ProxySettings,
    },
};

pub(crate) mod assembler;
pub(crate) mod client;
pub(crate) mod policy;
pub(crate) mod retry;
pub(crate) mod segmented;
pub(crate) mod single_stream;
pub(crate) mod validator;

pub use client::create_http_client;
use policy::TEMPORARY_DOWNLOAD_PREFIX;
use validator::SegmentValidator;

#[derive(Clone)]
pub struct HttpDownloadExecutor {
    client: Arc<RwLock<reqwest::Client>>,
    segment_validators: Arc<Mutex<HashMap<String, SegmentValidator>>>,
}

impl HttpDownloadExecutor {
    pub fn new() -> AppResult<Self> {
        let client = create_http_client(None)?;
        Ok(Self::with_client(client))
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            client: Arc::new(RwLock::new(client)),
            segment_validators: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_shared_client(client: Arc<RwLock<reqwest::Client>>) -> Self {
        Self {
            client,
            segment_validators: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn client(&self) -> reqwest::Client {
        self.client.read().unwrap().clone()
    }

    pub fn set_proxy(&self, proxy: Option<&ProxySettings>) -> AppResult<()> {
        let new_client = create_http_client(proxy)?;
        *self.client.write().unwrap() = new_client;
        Ok(())
    }

    async fn transfer(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
        mut control: DownloadControl,
        rate_limiter: Arc<DownloadRateLimiter>,
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
        let mut destination = directory.join(filename);
        fs::create_dir_all(&directory).await.map_err(|error| {
            AppError::download(format!("Could not create the download folder: {error}"))
        })?;
        if fs::try_exists(&destination).await.map_err(|error| {
            AppError::download(format!("Could not check the destination file: {error}"))
        })? {
            return Err(AppError::download("A file with that name already exists."));
        }

        let mut metrics = crate::infrastructure::metrics::TransferMetrics::new(
            &download.id,
            if download.connections > 1 { "segmented_http" } else { "single_http" },
            download.connections,
        );
        metrics.record_first_request();

        let temporary = directory.join(format!("{TEMPORARY_DOWNLOAD_PREFIX}{}.part", download.id));
        if download.connections > 1
            && !fs::try_exists(&temporary).await.map_err(|error| {
                AppError::download(format!("Could not check the partial download: {error}"))
            })?
        {
            let existing_parts = assembler::has_segment_parts(
                &directory,
                &download.id,
            )
            .await?;
            let range_info =
                segmented::discover_ranges(&self.client(), url.clone(), &mut control).await?;
            if let Some((validator, suggested_filename)) = range_info {
                if let Some(better_name) = suggested_filename {
                    let current_stem = destination
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    if crate::domain::download::is_generic_filename(current_stem) {
                        destination = destination.with_file_name(better_name);
                    }
                }
                metrics.range_supported = true;
                metrics.known_total = Some(validator.total);
                let previous = self
                    .segment_validators
                    .lock()
                    .map_err(|_| AppError::storage("Download resume state is unavailable."))?
                    .get(&download.id)
                    .cloned()
                    .or_else(|| {
                        download.etag.as_ref().map(|etag| SegmentValidator {
                            header: reqwest::header::IF_RANGE,
                            entity_header: reqwest::header::ETAG,
                            value: etag.clone(),
                            total: validator.total,
                        })
                    });
                let mut restart_notice = None;
                if existing_parts && previous.as_ref() != Some(&validator) {
                    assembler::remove_segment_parts(
                        &directory,
                        &download.id,
                    )
                    .await?;
                    restart_notice = Some(
                        "The remote file changed on the server; restarted download from the beginning."
                            .to_owned(),
                    );
                }
                self.segment_validators
                    .lock()
                    .map_err(|_| AppError::storage("Download resume state is unavailable."))?
                    .insert(download.id.clone(), validator.clone());
                if let Some(notice) = restart_notice {
                    progress_handler(DownloadProgress {
                        downloaded: 0,
                        total: Some(validator.total),
                        bytes_per_second: 0,
                        connections_progress: Vec::new(),
                        notice: Some(notice),
                        transfer_kind: Some("segmented_http".to_string()),
                        effective_concurrency: Some(download.connections),
                        etag: Some(validator.value.clone()),
                    });
                }
                let segments = segmented::segment_ranges(
                    validator.total,
                    download.connections,
                    &directory,
                    &download.id,
                    if download.connections_progress.is_empty() {
                        None
                    } else {
                        Some(&download.connections_progress)
                    },
                )
                .await?;
                let download_res = segmented::download_segments(
                    self.client(),
                    &download.id,
                    url.clone(),
                    &validator,
                    segments,
                    download.connections,
                    &directory,
                    &mut control,
                    rate_limiter.clone(),
                    &progress_handler,
                )
                .await;

                match download_res {
                    Ok(completed_segments) => {
                        assembler::concat_segments(
                            &download.id,
                            &completed_segments,
                            &directory,
                            &temporary,
                            &mut control,
                        )
                        .await?;
                        self.segment_validators
                            .lock()
                            .map_err(|_| AppError::storage("Download resume state is unavailable."))?
                            .remove(&download.id);
                        let result = assembler::finalize(
                            &download.id,
                            temporary,
                            destination,
                            validator.total,
                            Some(validator.total),
                        )
                        .await?;
                        if let Err(error) = assembler::remove_segment_parts(
                            &directory,
                            &download.id,
                        )
                        .await
                        {
                            eprintln!(
                                "Could not remove completed segment files for {}: {}",
                                download.id, error.message
                            );
                        }
                        metrics.record_completion(result.downloaded);
                        return Ok(result);
                    }
                    Err(error) if matches!(error.code, "download_paused" | "download_cancelled") => {
                        return Err(error);
                    }
                    Err(error) => {
                        // Safe Segmented Fallback (IDM-style):
                        // On range inconsistency or mid-stream segment failure, clean up
                        // incomplete segment parts and seamlessly fall back to single stream from byte 0.
                        metrics.set_fallback(format!("segmented_range_failed: {}", error.message));
                        let _ = assembler::remove_segment_parts(
                            &directory,
                            &download.id,
                        )
                        .await;
                        progress_handler(DownloadProgress {
                            downloaded: 0,
                            total: Some(validator.total),
                            bytes_per_second: 0,
                            connections_progress: Vec::new(),
                            notice: Some(
                                "Segmented range requests were not honored by the server; restarted as single stream."
                                    .to_owned(),
                            ),
                            transfer_kind: Some("single_http".to_string()),
                            effective_concurrency: Some(1),
                            etag: None,
                        });
                    }
                }
            } else if existing_parts {
                assembler::remove_segment_parts(
                    &directory,
                    &download.id,
                )
                .await?;
            }
        }

        let previous_validator = self
            .segment_validators
            .lock()
            .map_err(|_| AppError::storage("Download resume state is unavailable."))?
            .get(&download.id)
            .cloned()
            .or_else(|| {
                download.etag.as_ref().map(|etag| SegmentValidator {
                    header: reqwest::header::IF_RANGE,
                    entity_header: reqwest::header::ETAG,
                    value: etag.clone(),
                    total: download.total.unwrap_or(0),
                })
            });

        let (result, new_val) = single_stream::download_single_stream(
            self.client(),
            &download.id,
            url,
            temporary,
            destination,
            previous_validator,
            &progress_handler,
            &mut control,
            rate_limiter,
        )
        .await?;

        if let Some(validator) = new_val {
            if let Ok(mut map) = self.segment_validators.lock() {
                map.insert(download.id.clone(), validator);
            }
        }

        metrics.record_completion(result.downloaded);

        Ok(result)
    }
}

impl DownloadExecutor for HttpDownloadExecutor {
    fn execute(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
        control: DownloadControl,
        rate_limiter: Arc<DownloadRateLimiter>,
    ) -> DownloadExecution {
        let executor = self.clone();
        Box::pin(async move {
            executor
                .transfer(
                    download,
                    Arc::clone(&progress_handler),
                    control,
                    rate_limiter,
                )
                .await
        })
    }

    fn discard_partial(&self, download: &DownloadRecord) -> AppResult<()> {
        let Some(directory) = download.directory.as_deref() else {
            return Ok(());
        };
        let temporary = PathBuf::from(directory)
            .join(format!("{TEMPORARY_DOWNLOAD_PREFIX}{}.part", download.id));
        assembler::remove_partial_file(&temporary)?;
        assembler::remove_all_segment_parts_sync(std::path::Path::new(directory), &download.id)?;
        self.segment_validators
            .lock()
            .map_err(|_| AppError::storage("Download resume state is unavailable."))?
            .remove(&download.id);
        Ok(())
    }

    fn set_proxy(&self, proxy: Option<&ProxySettings>) -> AppResult<()> {
        self.set_proxy(proxy)
    }
}
