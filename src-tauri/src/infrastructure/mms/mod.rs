// Created: 2026-11-08
// Reason: feat(mms): add MMS streaming support or explicit unsupported guidance (Closes #35)
// Description: MMS protocol download executor with explicit session handshake, bounded chunk framing, and cancellation.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};
use url::Url;

use crate::{
    application::{
        download_executor::{
            stopped_error, DownloadControl, DownloadExecution, DownloadExecutor,
            DownloadProgressHandler,
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
        protocol::mms::{
            MmsProtocolDetector, MmsSessionStateMachine,
        },
        settings::ProxySettings,
    },
    infrastructure::http_download_executor::create_http_client,
};

pub const TEMPORARY_DOWNLOAD_PREFIX: &str = ".open-download-manager-";
const MAX_MMS_RETRIES: usize = 3;
const INITIAL_BACKOFF_MS: u64 = 500;

#[derive(Clone)]
pub struct MmsDownloadExecutor {
    allocator: DestinationAllocator,
    client: Arc<RwLock<reqwest::Client>>,
}

impl MmsDownloadExecutor {
    pub fn new(allocator: DestinationAllocator) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            allocator,
            client: Arc::new(RwLock::new(client)),
        }
    }

    pub fn with_client(allocator: DestinationAllocator, client: reqwest::Client) -> Self {
        Self {
            allocator,
            client: Arc::new(RwLock::new(client)),
        }
    }

    pub fn allocator(&self) -> DestinationAllocator {
        self.allocator.clone()
    }

    /// Converts an MMS URL (mms:// or mmsh://) to an HTTP streaming URL for the MMSH transport session.
    pub fn to_http_streaming_url(raw_url: &str) -> AppResult<Url> {
        let trimmed = raw_url.trim();
        let variant = MmsProtocolDetector::detect_scheme(trimmed).ok_or_else(|| {
            AppError::download("The download address does not use an MMS streaming protocol.")
        })?;

        if !variant.is_supported() {
            return Err(AppError::download(variant.unsupported_guidance()));
        }

        let colon = trimmed.find(':').ok_or_else(|| {
            AppError::download("Invalid MMS address: missing scheme delimiter.")
        })?;
        let after_scheme = &trimmed[colon + 1..];
        let http_candidate = format!("http:{after_scheme}");
        let mut parsed = Url::parse(&http_candidate).map_err(|e| {
            AppError::download(format!("Invalid MMS address: {e}"))
        })?;

        if parsed.port() == Some(443) {
            let https_candidate = format!("https:{after_scheme}");
            if let Ok(tls_url) = Url::parse(&https_candidate) {
                parsed = tls_url;
            }
        }

        Ok(parsed)
    }

    /// Internal asynchronous download transfer loop for MMS streaming.
    async fn transfer(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
        control: DownloadControl,
        rate_limiter: Arc<DownloadRateLimiter>,
    ) -> AppResult<DownloadResult> {
        let streaming_url = Self::to_http_streaming_url(&download.url)?;

        let directory = download
            .directory
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| AppError::download("Download destination directory is missing."))?;

        fs::create_dir_all(&directory)
            .await
            .map_err(|e| AppError::storage(format!("Could not create download folder: {e}")))?;

        let filename = sanitize_filename(&download.filename);
        let filename = if filename.is_empty() {
            "mms_media_stream.asf".to_string()
        } else {
            filename
        };

        let mut destination = directory.join(&filename);

        let destination_exists = fs::try_exists(&destination)
            .await
            .map_err(|e| AppError::download(format!("Could not inspect destination file: {e}")))?;
        let is_claimed = self.allocator.is_claimed(&destination);

        if destination_exists || is_claimed {
            match download.conflict_policy {
                Some(FileConflictPolicy::Overwrite) => {}
                Some(FileConflictPolicy::Rename) => {
                    destination = self
                        .allocator
                        .allocate_conflict_free_destination(&directory, &filename);
                }
                Some(FileConflictPolicy::Skip) => {
                    return Err(AppError {
                        code: "download_skipped",
                        message: "The download was skipped because the file already exists.".to_owned(),
                    });
                }
                Some(FileConflictPolicy::Cancel) => {
                    return Err(AppError {
                        code: "download_cancelled",
                        message: "The download was cancelled due to a filename collision.".to_owned(),
                    });
                }
                Some(FileConflictPolicy::Ask) | None => {
                    return Err(AppError::conflict("A file with that name already exists."));
                }
            }
        }

        let _claim_guard = self.allocator.claim_guard(destination.clone());
        let temporary = directory.join(format!("{TEMPORARY_DOWNLOAD_PREFIX}{}.part", download.id));

        let existing_len = match fs::metadata(&temporary).await {
            Ok(metadata) => metadata.len(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => {
                return Err(AppError::storage(format!(
                    "Could not inspect the temporary MMS download file: {error}"
                )));
            }
        };

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&temporary)
            .await
            .map_err(|error| {
                AppError::storage(format!(
                    "Could not open temporary MMS file {}: {error}",
                    temporary.display()
                ))
            })?;

        let mut state_machine = MmsSessionStateMachine::new();
        let mut downloaded_bytes = existing_len;
        let mut attempt = 0;
        let mut total_size;

        let client = self.client.read().unwrap().clone();

        loop {
            if let Some(reason) = control.current_stop_reason() {
                state_machine.cancel();
                return Err(stopped_error(reason));
            }

            state_machine.start_connection(streaming_url.as_str());

            let mut req = client
                .get(streaming_url.clone())
                .header(
                    "Pragma",
                    "no-cache,rate=1.000000,stream-time=0,stream-offset=0:0,request-context=1,max-duration=0",
                )
                .header("Pragma", "xClientGUID={c77e7400-738a-11d2-9add-0020af0a3278}")
                .header("Accept", "*/*")
                .header("User-Agent", "NSPlayer/12.00.10011.16384")
                .header("Connection", "Keep-Alive");

            if downloaded_bytes > 0 {
                state_machine.send_connection_info(downloaded_bytes);
                req = req.header("Range", format!("bytes={downloaded_bytes}-"));
            } else {
                state_machine.send_connection_info(0);
            }

            let mut resp = match req.send().await {
                Ok(r) => r,
                Err(err) => {
                    attempt += 1;
                    if attempt > MAX_MMS_RETRIES {
                        state_machine.fail(&err.to_string());
                        return Err(AppError::network(format!("MMS transport connection failed: {err}")));
                    }
                    let backoff = Duration::from_millis(INITIAL_BACKOFF_MS * (1 << (attempt - 1)));
                    tokio::time::sleep(backoff).await;
                    continue;
                }
            };

            let status = resp.status();
            if !status.is_success() && status.as_u16() != 206 {
                state_machine.fail(&format!("HTTP {status}"));
                return Err(AppError::network(format!(
                    "MMS streaming session was rejected by server with HTTP {status}"
                )));
            }

            // Server returned 200 OK when range was requested -> remote stream cannot resume, restart
            if status.as_u16() == 200 && downloaded_bytes > 0 {
                let _ = file.set_len(0).await;
                downloaded_bytes = 0;
            }

            total_size = resp.content_length().map(|len| len + downloaded_bytes);
            state_machine.begin_streaming();

            let mut stream_completed_cleanly = false;

            loop {
                if let Some(reason) = control.current_stop_reason() {
                    let _ = file.flush().await;
                    state_machine.cancel();
                    return Err(stopped_error(reason));
                }

                let chunk_opt = match resp.chunk().await {
                    Ok(opt) => opt,
                    Err(err) => {
                        state_machine.disconnect(&err.to_string());
                        break;
                    }
                };

                let chunk = match chunk_opt {
                    Some(c) => c,
                    None => {
                        stream_completed_cleanly = true;
                        break;
                    }
                };

                // Unpack MMS / ASF chunk framing if present (handles multiple consecutive chunks in buffer)
                let payload = Self::unpack_mms_chunks(&chunk, &mut stream_completed_cleanly);
                if payload.is_empty() {
                    if stream_completed_cleanly {
                        break;
                    }
                    continue;
                }

                file.write_all(&payload).await.map_err(|error| {
                    AppError::storage(format!("Could not write MMS data to disk: {error}"))
                })?;

                let payload_len = payload.len();
                downloaded_bytes += payload_len as u64;
                state_machine.record_chunk(payload_len);

                progress_handler(DownloadProgress {
                    downloaded: downloaded_bytes,
                    total: total_size,
                    bytes_per_second: 0,
                    connections_progress: Vec::new(),
                    notice: Some("Streaming from MMS media server...".to_string()),
                    transfer_kind: Some("mms".to_string()),
                    effective_concurrency: Some(1),
                    etag: None,
                    validator: None,
                });

                rate_limiter.throttle(payload_len as u64).await;

                if stream_completed_cleanly {
                    break;
                }
            }

            let _ = file.flush().await;

            if stream_completed_cleanly || total_size.is_some_and(|t| downloaded_bytes >= t) {
                state_machine.complete(downloaded_bytes);
                break;
            }

            // If we reached here without clean completion, evaluate retry
            attempt += 1;
            if attempt > MAX_MMS_RETRIES {
                state_machine.complete(downloaded_bytes);
                break;
            }
            let backoff = Duration::from_millis(INITIAL_BACKOFF_MS * (1 << (attempt - 1)));
            tokio::time::sleep(backoff).await;
        }

        drop(file);

        fs::rename(&temporary, &destination)
            .await
            .map_err(|error| {
                AppError::storage(format!(
                    "Could not rename temporary file to destination: {error}"
                ))
            })?;

        Ok(DownloadResult {
            downloaded: downloaded_bytes,
            total: total_size,
            path: destination,
        })
    }

    /// Unpacks media payloads from sequential MMS / ASF chunk framing ($H = header, $D = data, $E = end).
    /// If chunk is unframed (standard progressive stream), returns the entire chunk.
    pub fn unpack_mms_chunks(chunk: &[u8], end_of_stream: &mut bool) -> Vec<u8> {
        let mut cursor = 0;
        let mut result = Vec::new();

        while cursor < chunk.len() {
            if cursor + 4 <= chunk.len() && chunk[cursor] == b'$' {
                let chunk_type = chunk[cursor + 1];
                let chunk_len = u16::from_le_bytes([chunk[cursor + 2], chunk[cursor + 3]]) as usize;
                let payload_start = cursor + 4;
                let payload_end = (payload_start + chunk_len).min(chunk.len());

                match chunk_type {
                    b'H' | b'D' => {
                        result.extend_from_slice(&chunk[payload_start..payload_end]);
                    }
                    b'E' => {
                        *end_of_stream = true;
                        break;
                    }
                    _ => {
                        result.extend_from_slice(&chunk[payload_start..payload_end]);
                    }
                }
                cursor = payload_end;
                if payload_end == chunk.len() && payload_start + chunk_len > chunk.len() {
                    break;
                }
            } else {
                result.extend_from_slice(&chunk[cursor..]);
                break;
            }
        }

        result
    }
}

impl DownloadExecutor for MmsDownloadExecutor {
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
                .transfer(download, progress_handler, control, rate_limiter)
                .await
        })
    }

    fn discard_partial(&self, download: &DownloadRecord) -> AppResult<()> {
        let directory = match download.directory.as_deref() {
            Some(d) => Path::new(d),
            None => return Ok(()),
        };
        let temporary = directory.join(format!("{TEMPORARY_DOWNLOAD_PREFIX}{}.part", download.id));
        if temporary.exists() {
            let _ = std::fs::remove_file(temporary);
        }
        Ok(())
    }

    fn set_proxy(&self, proxy: Option<&ProxySettings>) -> AppResult<()> {
        let new_client = create_http_client(proxy)?;
        *self.client.write().unwrap() = new_client;
        Ok(())
    }
}
