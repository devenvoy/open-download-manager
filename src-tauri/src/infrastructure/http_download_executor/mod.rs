// Created: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Reason: feat(transfer): recover interrupted transfers and refresh expired links (Closes #3)
// Description: Core HTTP download executor supporting single-stream and multi-segmented transfers.

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
        download::{
            sanitize_filename, DestinationAllocator, DownloadRecord, FileConflictPolicy,
        },
        settings::{ConnectionOverride, ProxySettings},
    },
};

pub(crate) mod assembler;
pub(crate) mod auth;
pub(crate) mod client;
pub(crate) mod policy;
pub mod retry;
pub(crate) mod segmented;
pub(crate) mod single_stream;
pub(crate) mod validator;

pub use client::{
    build_reqwest_proxy, create_http_client, sanitize_proxy_error, test_proxy_connection,
    ReqwestProxyExt,
};
use policy::TEMPORARY_DOWNLOAD_PREFIX;
use validator::SegmentValidator;
use crate::infrastructure::credentials::CredentialStore;

#[derive(Clone)]
pub struct HttpDownloadExecutor {
    client: Arc<RwLock<reqwest::Client>>,
    segment_validators: Arc<Mutex<HashMap<String, SegmentValidator>>>,
    allocator: DestinationAllocator,
    credential_store: Option<Arc<CredentialStore>>,
    connection_overrides: Arc<RwLock<Vec<ConnectionOverride>>>,
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
            allocator: DestinationAllocator::new(),
            credential_store: None,
            connection_overrides: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_shared_client(client: Arc<RwLock<reqwest::Client>>) -> Self {
        Self {
            client,
            segment_validators: Arc::new(Mutex::new(HashMap::new())),
            allocator: DestinationAllocator::new(),
            credential_store: None,
            connection_overrides: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_shared_client_and_allocator(
        client: Arc<RwLock<reqwest::Client>>,
        allocator: DestinationAllocator,
    ) -> Self {
        Self {
            client,
            segment_validators: Arc::new(Mutex::new(HashMap::new())),
            allocator,
            credential_store: None,
            connection_overrides: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_credentials(mut self, credential_store: Arc<CredentialStore>) -> Self {
        self.credential_store = Some(credential_store);
        self
    }

    pub fn allocator(&self) -> DestinationAllocator {
        self.allocator.clone()
    }

    pub fn client(&self) -> reqwest::Client {
        self.client.read().unwrap().clone()
    }

    pub fn set_proxy(&self, proxy: Option<&ProxySettings>) -> AppResult<()> {
        let new_client = create_http_client(proxy)?;
        *self.client.write().unwrap() = new_client;
        Ok(())
    }

    pub fn set_connection_overrides(&self, overrides: &[ConnectionOverride]) {
        *self.connection_overrides.write().unwrap() = overrides.to_vec();
    }

    pub fn resolve_connections(&self, host: &str, fallback: u8) -> u8 {
        let guard = self.connection_overrides.read().unwrap();
        crate::domain::settings::resolve_connections_for_host(&guard, host, fallback)
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
        let mut destination = directory.join(&filename);
        fs::create_dir_all(&directory).await.map_err(|error| {
            AppError::download(format!("Could not create the download folder: {error}"))
        })?;

        if destination.is_dir() {
            return Err(AppError::download(
                "The destination path is an existing directory.",
            ));
        }

        let destination_exists = fs::try_exists(&destination).await.map_err(|error| {
            AppError::download(format!("Could not check the destination file: {error}"))
        })?;
        let is_claimed = self.allocator.is_claimed(&destination);

        if destination_exists {
            if let Ok(meta) = fs::metadata(&destination).await {
                let dest_len = meta.len();
                let is_already_done = match download.total {
                    Some(tot) if tot > 0 => dest_len == tot,
                    _ => download.downloaded > 0 && dest_len == download.downloaded,
                };
                if is_already_done {
                    return Ok(DownloadResult {
                        downloaded: dest_len,
                        total: Some(dest_len),
                        path: destination,
                    });
                }
            }
        }

        if destination_exists || is_claimed {
            match download.conflict_policy {
                Some(FileConflictPolicy::Overwrite) => {
                    // Safe overwrite: Destination remains untouched on disk while downloading.
                    // Atomic replacement happens only in assembler::finalize after full validation.
                }
                Some(FileConflictPolicy::Rename) => {
                    destination = self
                        .allocator
                        .allocate_conflict_free_destination(&directory, &filename);
                }
                Some(FileConflictPolicy::Skip) => {
                    return Err(AppError {
                        code: "download_skipped",
                        message: "Download skipped because destination file already exists."
                            .to_owned(),
                    });
                }
                Some(FileConflictPolicy::Cancel) => {
                    return Err(AppError {
                        code: "download_cancelled",
                        message: "Download cancelled due to destination conflict.".to_owned(),
                    });
                }
                None | Some(FileConflictPolicy::Ask) => {
                    return Err(AppError::conflict("A file with that name already exists."));
                }
            }
        }

        let _claim_guard = self.allocator.claim_guard(destination.clone());

        let matcher = self.credential_store.as_ref().and_then(|s| s.matcher().ok());
        let override_creds = download.credentials_override.as_ref();
        let ref_str = download.referer.as_deref();

        let mut metrics = crate::infrastructure::metrics::TransferMetrics::new(
            &download.id,
            if download.connections > 1 { "segmented_http" } else { "single_http" },
            download.connections,
        );
        metrics.record_first_request();

        let temp_dir = download
            .temp_directory
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| directory.clone());

        fs::create_dir_all(&temp_dir)
            .await
            .map_err(|error| {
                AppError::download(format!("Could not create temporary directory: {error}"))
            })?;

        let initial_host = url.host_str().unwrap_or("");
        let mut effective_connections = self.resolve_connections(initial_host, download.connections);

        let temporary = temp_dir.join(format!("{TEMPORARY_DOWNLOAD_PREFIX}{}.part", download.id));
        if effective_connections > 1
            && !fs::try_exists(&temporary).await.map_err(|error| {
                AppError::download(format!("Could not check the partial download: {error}"))
            })?
        {
            let existing_parts = assembler::has_segment_parts(
                &temp_dir,
                &download.id,
            )
            .await?;
            let range_info =
                segmented::discover_ranges(&self.client(), url.clone(), &mut control, override_creds, matcher.as_ref(), ref_str).await?;
            if let Some(discovery) = range_info {
                let final_host = discovery.final_url.host_str().unwrap_or("");
                effective_connections = self.resolve_connections(final_host, effective_connections);
                if effective_connections > 1 {
                    let validator = discovery.validator;
                    if let Some(better_name) = discovery.cd_filename {
                        let current_stem = destination
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("");
                        let has_ext = destination.extension().is_some();
                        let sanitized_better = sanitize_filename(&better_name);
                        if !sanitized_better.is_empty() && (!has_ext || crate::domain::download::is_generic_filename(current_stem) || sanitized_better != current_stem) {
                            if download.conflict_policy == Some(FileConflictPolicy::Rename) {
                                destination = self.allocator.allocate_conflict_free_destination(&directory, &sanitized_better);
                            } else {
                                destination = directory.join(sanitized_better);
                            }
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
                            download
                                .resume_validator()
                                .map(|v| SegmentValidator::new(v, validator.total))
                                .or_else(|| {
                                    if download.total == Some(validator.total) && validator.total > 0 {
                                        Some(SegmentValidator::without_validator(validator.total))
                                    } else {
                                        None
                                    }
                                })
                        });
                    let mut restart_notice = None;
                    if existing_parts {
                        if previous.is_none() {
                            assembler::remove_segment_parts(&temp_dir, &download.id).await?;
                            restart_notice = Some(
                                "The download could not be verified with a saved validator; restarted download from the beginning."
                                    .to_owned(),
                            );
                        } else if previous.as_ref() != Some(&validator) {
                            assembler::remove_segment_parts(&temp_dir, &download.id).await?;
                            restart_notice = Some(
                                "The remote file changed on the server; restarted download from the beginning."
                                    .to_owned(),
                            );
                        }
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
                            effective_concurrency: Some(effective_connections),
                            etag: validator.value().map(|s| s.to_string()),
                            validator: validator.validator.clone(),
                        });
                    }
                    let segments = segmented::segment_ranges(
                        validator.total,
                        effective_connections,
                        &temp_dir,
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
                        discovery.final_url,
                        &validator,
                        segments,
                        effective_connections,
                        &temp_dir,
                        &mut control,
                        rate_limiter.clone(),
                        &progress_handler,
                        discovery.auth_header,
                        download.referer.clone(),
                    )
                    .await;

                    match download_res {
                        Ok(completed_segments) => {
                            assembler::concat_segments(
                                &download.id,
                                &completed_segments,
                                &temp_dir,
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
                                &temp_dir,
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
                                &temp_dir,
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
                                validator: None,
                            });
                        }
                    }
                } else if existing_parts {
                    assembler::remove_segment_parts(
                        &temp_dir,
                        &download.id,
                    )
                    .await?;
                }
            } else if existing_parts {
                assembler::remove_segment_parts(
                    &temp_dir,
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
                download
                    .resume_validator()
                    .map(|v| SegmentValidator::new(v, download.total.unwrap_or(0)))
                    .or_else(|| {
                        download.total.map(|tot| SegmentValidator::without_validator(tot))
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
            override_creds,
            matcher.as_ref(),
            ref_str,
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
        let temp_dir = download
            .temp_directory
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(directory));
        let temporary = temp_dir
            .join(format!("{TEMPORARY_DOWNLOAD_PREFIX}{}.part", download.id));
        assembler::remove_partial_file(&temporary)?;
        assembler::remove_all_segment_parts_sync(&temp_dir, &download.id)?;
        self.segment_validators
            .lock()
            .map_err(|_| AppError::storage("Download resume state is unavailable."))?
            .remove(&download.id);
        Ok(())
    }

    fn set_proxy(&self, proxy: Option<&ProxySettings>) -> AppResult<()> {
        self.set_proxy(proxy)
    }

    fn set_connection_overrides(
        &self,
        overrides: &[crate::domain::settings::ConnectionOverride],
        _default_connections: u8,
    ) {
        self.set_connection_overrides(overrides);
    }
}
