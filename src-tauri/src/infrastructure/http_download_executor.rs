use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use reqwest::header::HeaderName;
use tokio::{
    fs::{self, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
};
use url::Url;

use crate::{
    application::{
        download_executor::{
            stopped_error, DownloadControl, DownloadExecution, DownloadExecutor,
            DownloadProgressHandler, DownloadStopReason,
        },
        download_progress::DownloadProgress,
        download_rate_limiter::DownloadRateLimiter,
        download_result::DownloadResult,
        error::{AppError, AppResult},
    },
    domain::download::{sanitize_filename, DownloadRecord, DownloadSegmentProgress},
};

mod policy;

use self::policy::{
    CONNECT_TIMEOUT_SECONDS, INITIAL_RETRY_DELAY_MILLISECONDS, MAX_REDIRECTS,
    MAX_RETRY_DELAY_SECONDS, MAX_TRANSIENT_RETRIES, PROGRESS_EVENT_INTERVAL_MILLISECONDS,
    TEMPORARY_DOWNLOAD_PREFIX,
};

#[derive(Clone)]
pub struct HttpDownloadExecutor {
    client: reqwest::Client,
    segment_validators: Arc<Mutex<HashMap<String, SegmentValidator>>>,
}

#[derive(Clone, PartialEq, Eq)]
struct SegmentValidator {
    header: HeaderName,
    entity_header: HeaderName,
    value: String,
    total: u64,
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
        Ok(Self {
            client,
            segment_validators: Arc::new(Mutex::new(HashMap::new())),
        })
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
        let destination = directory.join(filename);
        fs::create_dir_all(&directory).await.map_err(|error| {
            AppError::download(format!("Could not create the download folder: {error}"))
        })?;
        if fs::try_exists(&destination).await.map_err(|error| {
            AppError::download(format!("Could not check the destination file: {error}"))
        })? {
            return Err(AppError::download("A file with that name already exists."));
        }

        let temporary = directory.join(format!("{TEMPORARY_DOWNLOAD_PREFIX}{}.part", download.id));
        if download.connections > 1
            && !fs::try_exists(&temporary).await.map_err(|error| {
                AppError::download(format!("Could not check the partial download: {error}"))
            })?
        {
            let existing_parts = self
                .has_segment_parts(&directory, &download.id, download.connections)
                .await?;
            let range_info = self.discover_ranges(url.clone(), &mut control).await?;
            if let Some(validator) = range_info {
                let previous = self
                    .segment_validators
                    .lock()
                    .map_err(|_| AppError::storage("Download resume state is unavailable."))?
                    .get(&download.id)
                    .cloned();
                if existing_parts && previous.as_ref() != Some(&validator) {
                    self.remove_segment_parts(&directory, &download.id, download.connections)
                        .await?;
                }
                self.segment_validators
                    .lock()
                    .map_err(|_| AppError::storage("Download resume state is unavailable."))?
                    .insert(download.id.clone(), validator.clone());
                let segments = segment_ranges(
                    validator.total,
                    download.connections,
                    &directory,
                    &download.id,
                )
                .await?;
                let assembly_segments = segments.clone();
                self.download_segments(
                    &download.id,
                    url.clone(),
                    &validator,
                    segments,
                    &directory,
                    &mut control,
                    rate_limiter.clone(),
                    &progress_handler,
                )
                .await?;
                self.concat_segments(
                    &download.id,
                    &assembly_segments,
                    &directory,
                    &temporary,
                    &mut control,
                )
                .await?;
                self.segment_validators
                    .lock()
                    .map_err(|_| AppError::storage("Download resume state is unavailable."))?
                    .remove(&download.id);
                let result = self
                    .finalize(
                        &download.id,
                        temporary,
                        destination,
                        validator.total,
                        Some(validator.total),
                    )
                    .await?;
                if let Err(error) = self
                    .remove_segment_parts(&directory, &download.id, download.connections)
                    .await
                {
                    eprintln!(
                        "Could not remove completed segment files for {}: {}",
                        download.id, error.message
                    );
                }
                return Ok(result);
            }
            if existing_parts {
                self.remove_segment_parts(&directory, &download.id, download.connections)
                    .await?;
            }
        }
        let mut downloaded = match fs::metadata(&temporary).await {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => {
                return Err(AppError::download(format!(
                    "Could not inspect the partial download: {error}"
                )));
            }
        };
        let mut response = self
            .send_request(url.clone(), downloaded, &mut control)
            .await?;
        if downloaded > 0 && response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            fs::remove_file(&temporary).await.map_err(|error| {
                AppError::download(format!("Could not restart the partial download: {error}"))
            })?;
            downloaded = 0;
            response = self.send_request(url, 0, &mut control).await?;
        }
        let mut total = response.content_length();
        if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
            if downloaded == 0 {
                return Err(AppError::download(
                    "The server returned an unexpected partial response.",
                ));
            }
            let (start, end, range_total) = content_range(&response)?;
            if start != downloaded || end < start || range_total.is_some_and(|total| end >= total) {
                return Err(AppError::download(
                    "The server returned an invalid byte range for the partial download.",
                ));
            }
            total = range_total.or_else(|| {
                response
                    .content_length()
                    .and_then(|length| downloaded.checked_add(length))
            });
        } else if response.status().is_success() {
            downloaded = 0;
        } else {
            return Err(AppError::download(format!(
                "The server returned HTTP {}.",
                response.status().as_u16()
            )));
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(downloaded > 0)
            .truncate(downloaded == 0)
            .open(&temporary)
            .await
            .map_err(|error| {
                AppError::download(format!(
                    "Could not create a temporary download file: {error}"
                ))
            })?;

        progress_handler(DownloadProgress {
            downloaded,
            total,
            bytes_per_second: 0,
            connections_progress: Vec::new(),
        });
        let mut bytes_since_tick = 0_u64;
        let mut last_tick = Instant::now();
        loop {
            let chunk = tokio::select! {
                reason = control.stopped() => {
                    file.flush().await.map_err(|error| {
                        AppError::download(format!("Could not save the partial download: {error}"))
                    })?;
                    file.sync_all().await.map_err(|error| {
                        AppError::download(format!("Could not save the partial download: {error}"))
                    })?;
                    return Err(stopped_error(reason));
                }
                result = response.chunk() => result.map_err(|_| {
                    AppError::download("The server response could not be read.")
                })?,
            };
            let Some(chunk) = chunk else {
                break;
            };
            let chunk_size = u64::try_from(chunk.len())
                .map_err(|_| AppError::download("The downloaded chunk size is invalid."))?;
            let throttled = tokio::select! {
                reason = control.stopped() => Err(stopped_error(reason)),
                () = rate_limiter.throttle(chunk_size) => Ok(()),
            };
            if let Err(error) = throttled {
                file.flush().await.map_err(|error| {
                    AppError::download(format!("Could not save the partial download: {error}"))
                })?;
                file.sync_all().await.map_err(|error| {
                    AppError::download(format!("Could not save the partial download: {error}"))
                })?;
                return Err(error);
            }
            file.write_all(&chunk).await.map_err(|error| {
                AppError::download(format!("Could not write the download file: {error}"))
            })?;
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
                    connections_progress: Vec::new(),
                });
                last_tick = Instant::now();
                bytes_since_tick = 0;
            }
        }
        if let Some(reason) = control.current_stop_reason() {
            file.flush().await.map_err(|error| {
                AppError::download(format!("Could not save the partial download: {error}"))
            })?;
            file.sync_all().await.map_err(|error| {
                AppError::download(format!("Could not save the partial download: {error}"))
            })?;
            return Err(stopped_error(reason));
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
        if let Some(reason) = control.current_stop_reason() {
            return Err(stopped_error(reason));
        }
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

    async fn discover_ranges(
        &self,
        url: Url,
        control: &mut DownloadControl,
    ) -> AppResult<Option<SegmentValidator>> {
        let response = send_with_retry(control, || {
            self.client
                .get(url.clone())
                .header(reqwest::header::RANGE, "bytes=0-0")
                .header(reqwest::header::ACCEPT_ENCODING, "identity")
        })
        .await?;
        if response.status() != reqwest::StatusCode::PARTIAL_CONTENT
            || response
                .headers()
                .get(reqwest::header::CONTENT_ENCODING)
                .is_some_and(|encoding| encoding.as_bytes() != b"identity")
        {
            return Ok(None);
        }
        let Ok((0, 0, Some(total))) = content_range(&response) else {
            return Ok(None);
        };
        if total < 2 {
            return Ok(None);
        }
        let validator = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.starts_with("W/"))
            .map(|value| SegmentValidator {
                header: reqwest::header::IF_RANGE,
                entity_header: reqwest::header::ETAG,
                value: value.to_owned(),
                total,
            })
            .or_else(|| {
                response
                    .headers()
                    .get(reqwest::header::LAST_MODIFIED)
                    .and_then(|value| value.to_str().ok())
                    .map(|value| SegmentValidator {
                        header: reqwest::header::IF_RANGE,
                        entity_header: reqwest::header::LAST_MODIFIED,
                        value: value.to_owned(),
                        total,
                    })
            });
        Ok(validator)
    }

    async fn has_segment_parts(
        &self,
        directory: &std::path::Path,
        id: &str,
        count: u8,
    ) -> AppResult<bool> {
        for index in 0..count {
            if fs::try_exists(segment_path(directory, id, index))
                .await
                .map_err(|error| {
                    AppError::download(format!("Could not check a partial segment: {error}"))
                })?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn remove_segment_parts(
        &self,
        directory: &std::path::Path,
        id: &str,
        count: u8,
    ) -> AppResult<()> {
        for index in 0..count {
            match fs::remove_file(segment_path(directory, id, index)).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(AppError::download(format!(
                        "Could not remove a partial segment: {error}"
                    )));
                }
            }
        }
        Ok(())
    }

    async fn download_segments(
        &self,
        id: &str,
        url: Url,
        validator: &SegmentValidator,
        segments: Vec<DownloadSegmentProgress>,
        directory: &std::path::Path,
        control: &mut DownloadControl,
        rate_limiter: Arc<DownloadRateLimiter>,
        progress_handler: &DownloadProgressHandler,
    ) -> AppResult<()> {
        let segments = Arc::new(Mutex::new(segments));
        let mut jobs = tokio::task::JoinSet::new();
        {
            let current = segments
                .lock()
                .map_err(|_| AppError::storage("Download progress is unavailable."))?
                .clone();
            progress_handler(DownloadProgress {
                downloaded: current.iter().map(|segment| segment.downloaded).sum(),
                total: Some(validator.total),
                bytes_per_second: 0,
                connections_progress: current,
            });
        }
        let segment_count = segments
            .lock()
            .map_err(|_| AppError::storage("Download progress is unavailable."))?
            .len();
        for index in 0..segment_count {
            let segment = segments
                .lock()
                .map_err(|_| AppError::storage("Download progress is unavailable."))?[index]
                .clone();
            if segment.downloaded == segment.total {
                continue;
            }
            let client = self.client.clone();
            let segment_index = u8::try_from(index)
                .map_err(|_| AppError::download("The segment index is invalid."))?;
            let path = segment_path(directory, id, segment_index);
            let segment_progress = segments.clone();
            let control = control.clone();
            let validator = validator.clone();
            let rate_limiter = rate_limiter.clone();
            let progress_handler = progress_handler.clone();
            let url = url.clone();
            jobs.spawn(async move {
                download_segment(
                    client,
                    url,
                    path,
                    index,
                    segment,
                    validator,
                    segment_progress,
                    control,
                    rate_limiter,
                    progress_handler,
                )
                .await
            });
        }
        while let Some(result) = jobs.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error))
                    if matches!(error.code, "download_paused" | "download_cancelled") =>
                {
                    while jobs.join_next().await.is_some() {}
                    return Err(error);
                }
                Ok(Err(error)) => {
                    jobs.abort_all();
                    while jobs.join_next().await.is_some() {}
                    return Err(error);
                }
                Err(error) => {
                    jobs.abort_all();
                    while jobs.join_next().await.is_some() {}
                    return Err(AppError::download(format!(
                        "A segmented transfer stopped unexpectedly: {error}"
                    )));
                }
            }
        }
        Ok(())
    }

    async fn concat_segments(
        &self,
        id: &str,
        segments: &[DownloadSegmentProgress],
        directory: &std::path::Path,
        temporary: &std::path::Path,
        control: &mut DownloadControl,
    ) -> AppResult<()> {
        let mut output = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(temporary)
            .await
            .map_err(|error| {
                AppError::download(format!("Could not assemble the download file: {error}"))
            })?;
        let mut buffer = vec![0; 64 * 1024];
        for (index, _) in segments.iter().enumerate() {
            let segment_index = u8::try_from(index)
                .map_err(|_| AppError::download("The segment index is invalid."))?;
            let path = segment_path(directory, id, segment_index);
            let mut input = fs::File::open(path).await.map_err(|error| {
                AppError::download(format!("Could not read a completed segment: {error}"))
            })?;
            loop {
                let count = input.read(&mut buffer).await.map_err(|error| {
                    AppError::download(format!("Could not read a completed segment: {error}"))
                })?;
                if count == 0 {
                    break;
                }
                if let Some(reason) = control.current_stop_reason() {
                    output.sync_all().await.map_err(|error| {
                        AppError::download(format!("Could not save the partial assembly: {error}"))
                    })?;
                    drop(output);
                    fs::remove_file(temporary).await.map_err(|error| {
                        AppError::download(format!(
                            "Could not discard the partial assembly: {error}"
                        ))
                    })?;
                    return Err(stopped_error(reason));
                }
                output.write_all(&buffer[..count]).await.map_err(|error| {
                    AppError::download(format!("Could not assemble the download file: {error}"))
                })?;
            }
        }
        if let Some(reason) = control.current_stop_reason() {
            output.sync_all().await.map_err(|error| {
                AppError::download(format!("Could not save the partial assembly: {error}"))
            })?;
            drop(output);
            fs::remove_file(temporary).await.map_err(|error| {
                AppError::download(format!("Could not discard the partial assembly: {error}"))
            })?;
            return Err(stopped_error(reason));
        }
        output.flush().await.map_err(|error| {
            AppError::download(format!("Could not finalize the assembled file: {error}"))
        })?;
        output.sync_all().await.map_err(|error| {
            AppError::download(format!("Could not finalize the assembled file: {error}"))
        })?;
        Ok(())
    }

    async fn finalize(
        &self,
        id: &str,
        temporary: PathBuf,
        destination: PathBuf,
        downloaded: u64,
        total: Option<u64>,
    ) -> AppResult<DownloadResult> {
        fs::hard_link(&temporary, &destination)
            .await
            .map_err(|error| {
                AppError::download(format!("Could not move the download into place: {error}"))
            })?;
        if let Err(error) = fs::remove_file(&temporary).await {
            eprintln!("Could not remove temporary file for download {id}: {error}");
        }
        Ok(DownloadResult {
            downloaded,
            total: total.or(Some(downloaded)),
            path: destination,
        })
    }

    async fn send_request(
        &self,
        url: Url,
        offset: u64,
        control: &mut DownloadControl,
    ) -> AppResult<reqwest::Response> {
        send_with_retry(control, || {
            let request = self.client.get(url.clone());
            if offset > 0 {
                request.header(reqwest::header::RANGE, format!("bytes={offset}-"))
            } else {
                request
            }
        })
        .await
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
            let cleanup_record = download.clone();
            let result = executor
                .transfer(
                    download,
                    Arc::clone(&progress_handler),
                    control,
                    rate_limiter,
                )
                .await;
            if result
                .as_ref()
                .is_err_and(|error| error.code != "download_paused")
            {
                if let Err(error) = executor.discard_partial(&cleanup_record) {
                    eprintln!("Could not remove a partial download: {}", error.message);
                }
            }
            result
        })
    }

    fn discard_partial(&self, download: &DownloadRecord) -> AppResult<()> {
        let Some(directory) = download.directory.as_deref() else {
            return Ok(());
        };
        let temporary = PathBuf::from(directory)
            .join(format!("{TEMPORARY_DOWNLOAD_PREFIX}{}.part", download.id));
        remove_partial_file(&temporary)?;
        for index in 0..download.connections.max(1) {
            remove_partial_file(&segment_path(
                &PathBuf::from(directory),
                &download.id,
                index,
            ))?;
        }
        self.segment_validators
            .lock()
            .map_err(|_| AppError::storage("Download resume state is unavailable."))?
            .remove(&download.id);
        Ok(())
    }
}

fn remove_partial_file(path: &std::path::Path) -> AppResult<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::download(format!(
            "Could not remove partial file {}: {error}",
            path.display()
        ))),
    }
}

async fn send_with_retry(
    control: &mut DownloadControl,
    mut build_request: impl FnMut() -> reqwest::RequestBuilder,
) -> AppResult<reqwest::Response> {
    for retry in 0..=MAX_TRANSIENT_RETRIES {
        let response = tokio::select! {
            reason = control.stopped() => return Err(stopped_error(reason)),
            response = build_request().send() => response,
        };
        let response = match response {
            Ok(response) => response,
            Err(error)
                if retry < MAX_TRANSIENT_RETRIES && (error.is_connect() || error.is_timeout()) =>
            {
                wait_before_retry(control, retry_delay(retry, None)).await?;
                continue;
            }
            Err(_) => return Err(AppError::download("Could not connect to the server.")),
        };
        if retry < MAX_TRANSIENT_RETRIES && is_retryable_status(response.status()) {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            drop(response);
            wait_before_retry(control, retry_delay(retry, retry_after)).await?;
            continue;
        }
        return Ok(response);
    }
    Err(AppError::download("Could not connect to the server."))
}

fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status.is_server_error()
        || status == reqwest::StatusCode::REQUEST_TIMEOUT
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
}

fn retry_delay(retry: u8, retry_after_seconds: Option<u64>) -> Duration {
    let maximum_delay = Duration::from_secs(MAX_RETRY_DELAY_SECONDS);
    let delay = retry_after_seconds
        .map(Duration::from_secs)
        .unwrap_or_else(|| {
            let multiplier = 1_u64 << u32::from(retry);
            Duration::from_millis(INITIAL_RETRY_DELAY_MILLISECONDS.saturating_mul(multiplier))
        });
    delay.min(maximum_delay)
}

async fn wait_before_retry(control: &mut DownloadControl, delay: Duration) -> AppResult<()> {
    tokio::select! {
        reason = control.stopped() => Err(stopped_error(reason)),
        () = tokio::time::sleep(delay) => Ok(()),
    }
}

fn segment_path(directory: &std::path::Path, id: &str, index: u8) -> PathBuf {
    directory.join(format!(
        "{TEMPORARY_DOWNLOAD_PREFIX}{id}.segment-{index}.part"
    ))
}

async fn segment_ranges(
    total: u64,
    requested_connections: u8,
    directory: &std::path::Path,
    id: &str,
) -> AppResult<Vec<DownloadSegmentProgress>> {
    let count = u64::from(requested_connections.max(1)).min(total);
    let count =
        usize::try_from(count).map_err(|_| AppError::download("The segment count is invalid."))?;
    let mut segments = Vec::with_capacity(count);
    let count_u64 =
        u64::try_from(count).map_err(|_| AppError::download("The segment count is invalid."))?;
    let base_size = total / count_u64;
    let remainder = total % count_u64;
    let mut start = 0_u64;
    for index in 0..count {
        let index = u64::try_from(index)
            .map_err(|_| AppError::download("The segment index is invalid."))?;
        let segment_total = base_size + u64::from(index < remainder);
        let end = start + segment_total - 1;
        let index =
            u8::try_from(index).map_err(|_| AppError::download("The segment index is invalid."))?;
        let path = segment_path(directory, id, index);
        let downloaded = match fs::metadata(path).await {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => {
                return Err(AppError::download(format!(
                    "Could not inspect a partial segment: {error}"
                )));
            }
        };
        if downloaded > segment_total {
            return Err(AppError::download(
                "A partial segment is larger than its assigned byte range.",
            ));
        }
        segments.push(DownloadSegmentProgress {
            start,
            end,
            downloaded,
            total: segment_total,
            status: if downloaded == segment_total {
                "complete"
            } else {
                "queued"
            }
            .to_owned(),
        });
        start = end + 1;
    }
    Ok(segments)
}

async fn download_segment(
    client: reqwest::Client,
    url: Url,
    path: PathBuf,
    index: usize,
    segment: DownloadSegmentProgress,
    validator: SegmentValidator,
    segments: Arc<Mutex<Vec<DownloadSegmentProgress>>>,
    mut control: DownloadControl,
    rate_limiter: Arc<DownloadRateLimiter>,
    progress_handler: DownloadProgressHandler,
) -> AppResult<()> {
    let start = segment
        .start
        .checked_add(segment.downloaded)
        .ok_or_else(|| AppError::download("The segment offset is invalid."))?;
    let mut response = send_with_retry(&mut control, || {
        client
            .get(url.clone())
            .header(
                reqwest::header::RANGE,
                format!("bytes={start}-{}", segment.end),
            )
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .header(validator.header.clone(), &validator.value)
    })
    .await?;
    if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(AppError::download(
            "The server did not honor a segmented byte-range request.",
        ));
    }
    let (actual_start, actual_end, actual_total) = content_range(&response)?;
    if actual_start != start
        || actual_end != segment.end
        || actual_total != Some(validator.total)
        || response
            .headers()
            .get(&validator.entity_header)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value != validator.value)
        || response
            .headers()
            .get(reqwest::header::CONTENT_ENCODING)
            .is_some_and(|value| value.as_bytes() != b"identity")
    {
        return Err(AppError::download(
            "The server returned an inconsistent byte range for a segment.",
        ));
    }
    let expected_remaining = segment.total - segment.downloaded;
    if response
        .content_length()
        .is_some_and(|length| length != expected_remaining)
    {
        return Err(AppError::download(
            "The server returned an unexpected segment size.",
        ));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(segment.downloaded > 0)
        .truncate(segment.downloaded == 0)
        .open(path)
        .await
        .map_err(|error| {
            AppError::download(format!("Could not create a partial segment: {error}"))
        })?;
    update_segment_progress(
        &segments,
        index,
        segment.downloaded,
        "downloading",
        validator.total,
        &progress_handler,
    )?;
    let mut downloaded = segment.downloaded;
    loop {
        let chunk = tokio::select! {
            reason = control.stopped() => {
                file.sync_all().await.map_err(|error| {
                    AppError::download(format!("Could not save a partial segment: {error}"))
                })?;
                update_segment_progress(
                    &segments,
                    index,
                    downloaded,
                    stop_status(reason),
                    validator.total,
                    &progress_handler,
                )?;
                return Err(stopped_error(reason));
            }
            chunk = response.chunk() => chunk.map_err(|_| {
                AppError::download("The server response could not be read.")
            })?,
        };
        let Some(chunk) = chunk else {
            break;
        };
        let chunk_size = u64::try_from(chunk.len())
            .map_err(|_| AppError::download("The downloaded chunk size is invalid."))?;
        if downloaded
            .checked_add(chunk_size)
            .is_none_or(|value| value > segment.total)
        {
            return Err(AppError::download(
                "The server sent more data than the requested byte range.",
            ));
        }
        let throttled = tokio::select! {
            reason = control.stopped() => Err(stopped_error(reason)),
            () = rate_limiter.throttle(chunk_size) => Ok(()),
        };
        if let Err(error) = throttled {
            file.sync_all().await.map_err(|error| {
                AppError::download(format!("Could not save a partial segment: {error}"))
            })?;
            let reason = control
                .current_stop_reason()
                .unwrap_or(DownloadStopReason::Cancel);
            update_segment_progress(
                &segments,
                index,
                downloaded,
                stop_status(reason),
                validator.total,
                &progress_handler,
            )?;
            return Err(error);
        }
        file.write_all(&chunk).await.map_err(|error| {
            AppError::download(format!("Could not write a partial segment: {error}"))
        })?;
        downloaded += chunk_size;
        update_segment_progress(
            &segments,
            index,
            downloaded,
            "downloading",
            validator.total,
            &progress_handler,
        )?;
    }
    file.flush().await.map_err(|error| {
        AppError::download(format!("Could not finalize a partial segment: {error}"))
    })?;
    file.sync_all().await.map_err(|error| {
        AppError::download(format!("Could not finalize a partial segment: {error}"))
    })?;
    if downloaded != segment.total {
        return Err(AppError::download(
            "The server response ended before the requested segment was complete.",
        ));
    }
    update_segment_progress(
        &segments,
        index,
        downloaded,
        "complete",
        validator.total,
        &progress_handler,
    )
}

fn stop_status(reason: DownloadStopReason) -> &'static str {
    match reason {
        DownloadStopReason::Pause => "paused",
        DownloadStopReason::Cancel => "cancelled",
    }
}

fn update_segment_progress(
    segments: &Arc<Mutex<Vec<DownloadSegmentProgress>>>,
    index: usize,
    downloaded: u64,
    status: &str,
    total: u64,
    progress_handler: &DownloadProgressHandler,
) -> AppResult<()> {
    let mut segments = segments
        .lock()
        .map_err(|_| AppError::storage("Download progress is unavailable."))?;
    let segment = segments
        .get_mut(index)
        .ok_or_else(|| AppError::download("The segment no longer exists."))?;
    segment.downloaded = downloaded;
    segment.status = status.to_owned();
    let downloaded = segments
        .iter()
        .map(|segment| segment.downloaded)
        .fold(0_u64, u64::saturating_add);
    let connections_progress = segments.clone();
    drop(segments);
    progress_handler(DownloadProgress {
        downloaded,
        total: Some(total),
        bytes_per_second: 0,
        connections_progress,
    });
    Ok(())
}

fn content_range(response: &reqwest::Response) -> AppResult<(u64, u64, Option<u64>)> {
    let value = response
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("bytes "))
        .ok_or_else(|| AppError::download("The server returned an invalid byte range."))?;
    let (range, total) = value
        .split_once('/')
        .ok_or_else(|| AppError::download("The server returned an invalid byte range."))?;
    let (start, end) = range
        .split_once('-')
        .ok_or_else(|| AppError::download("The server returned an invalid byte range."))?;
    let start = start
        .parse()
        .map_err(|_| AppError::download("The server returned an invalid byte range."))?;
    let end = end
        .parse()
        .map_err(|_| AppError::download("The server returned an invalid byte range."))?;
    let total = if total == "*" {
        None
    } else {
        Some(
            total
                .parse()
                .map_err(|_| AppError::download("The server returned an invalid byte range."))?,
        )
    };
    Ok((start, end, total))
}
