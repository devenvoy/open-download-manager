use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};
use url::Url;

use crate::{
    application::{
        download_executor::{
            stopped_error, DownloadControl, DownloadProgressHandler,
        },
        download_progress::DownloadProgress,
        download_rate_limiter::DownloadRateLimiter,
        download_result::DownloadResult,
        error::{AppError, AppResult},
    },
};

use super::{
    assembler::finalize,
    policy::{
        MAX_TRANSIENT_RETRIES, PROGRESS_EVENT_INTERVAL_MILLISECONDS, READ_TIMEOUT_SECONDS,
    },
    retry::{retry_delay, send_with_retry, wait_before_retry},
    validator::{content_range, extract_validator, SegmentValidator},
};

pub(crate) async fn download_single_stream(
    client: reqwest::Client,
    download_id: &str,
    url: Url,
    temporary: PathBuf,
    destination: PathBuf,
    previous_validator: Option<SegmentValidator>,
    progress_handler: &DownloadProgressHandler,
    control: &mut DownloadControl,
    rate_limiter: Arc<DownloadRateLimiter>,
) -> AppResult<(DownloadResult, Option<SegmentValidator>)> {
    let mut downloaded = match fs::metadata(&temporary).await {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => {
            return Err(AppError::download(format!(
                "Could not inspect the partial download: {error}"
            )));
        }
    };
    let initial_downloaded = downloaded;

    let mut restart_notice = None;
    let mut response = send_stream_request(
        &client,
        url.clone(),
        downloaded,
        previous_validator.as_ref(),
        control,
    )
    .await?;

    if downloaded > 0 && response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        fs::remove_file(&temporary).await.map_err(|error| {
            AppError::download(format!("Could not restart the partial download: {error}"))
        })?;
        downloaded = 0;
        restart_notice = Some(
            "The server could not satisfy the byte range; restarted download from the beginning."
                .to_owned(),
        );
        response = send_stream_request(&client, url.clone(), 0, None, control).await?;
    }

    let mut total = response.content_length();
    let mut supports_range = response.status() == reqwest::StatusCode::PARTIAL_CONTENT
        || response
            .headers()
            .get(reqwest::header::ACCEPT_RANGES)
            .is_some_and(|value| value.as_bytes() == b"bytes");

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
        supports_range = true;
    } else if response.status().is_success() {
        if initial_downloaded > 0 {
            downloaded = 0;
            restart_notice = Some(
                "The remote file changed or server does not support resuming; restarted download from the beginning."
                    .to_owned(),
            );
        }
    } else if response.status() == reqwest::StatusCode::FORBIDDEN
        || response.status() == reqwest::StatusCode::GONE
    {
        return Err(AppError::download(format!(
            "The download link has expired (HTTP {}). Refresh the link to resume.",
            response.status().as_u16()
        )));
    } else {
        return Err(AppError::download(format!(
            "The server returned HTTP {}.",
            response.status().as_u16()
        )));
    }

    let new_validator = extract_validator(&response, total.unwrap_or(downloaded));

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
        notice: restart_notice,
    });

    let mut bytes_since_tick = 0_u64;
    let mut last_tick = Instant::now();
    let mut midstream_retries = 0_u8;

    'stream: loop {
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
            result = tokio::time::timeout(
                Duration::from_secs(READ_TIMEOUT_SECONDS),
                response.chunk(),
            ) => match result {
                Ok(chunk_res) => chunk_res.map_err(|e| e.to_string()),
                Err(_) => Err("Connection timed out while waiting for data from server.".to_owned()),
            },
        };

        let chunk = match chunk {
            Ok(Some(chunk)) => chunk,
            Ok(None) => {
                if total.is_some_and(|expected| expected != downloaded) {
                    if midstream_retries < MAX_TRANSIENT_RETRIES
                        && control.current_stop_reason().is_none()
                    {
                        midstream_retries += 1;
                        file.flush().await.map_err(|error| {
                            AppError::download(format!("Could not save the partial download: {error}"))
                        })?;
                        file.sync_all().await.map_err(|error| {
                            AppError::download(format!("Could not save the partial download: {error}"))
                        })?;
                        wait_before_retry(control, retry_delay(midstream_retries, None)).await?;

                        let next_request = if supports_range && downloaded > 0 {
                            send_stream_request(&client, url.clone(), downloaded, new_validator.as_ref(), control).await
                        } else {
                            send_stream_request(&client, url.clone(), 0, None, control).await
                        };

                        match next_request {
                            Ok(new_response) => {
                                if new_response.status() == reqwest::StatusCode::FORBIDDEN
                                    || new_response.status() == reqwest::StatusCode::GONE
                                {
                                    return Err(AppError::download(format!(
                                        "The download link has expired (HTTP {}). Refresh the link to resume.",
                                        new_response.status().as_u16()
                                    )));
                                }
                                if supports_range
                                    && downloaded > 0
                                    && new_response.status() == reqwest::StatusCode::PARTIAL_CONTENT
                                {
                                    if let Ok((start, _, _)) = content_range(&new_response) {
                                        if start == downloaded {
                                            response = new_response;
                                            continue 'stream;
                                        }
                                    }
                                }
                                drop(file);
                                let new_file = OpenOptions::new()
                                    .write(true)
                                    .create(true)
                                    .truncate(true)
                                    .open(&temporary)
                                    .await
                                    .map_err(|error| {
                                        AppError::download(format!(
                                            "Could not create a temporary download file: {error}"
                                        ))
                                    })?;
                                file = new_file;
                                downloaded = 0;
                                total = new_response.content_length();
                                supports_range = new_response
                                    .headers()
                                    .get(reqwest::header::ACCEPT_RANGES)
                                    .is_some_and(|value| value.as_bytes() == b"bytes");
                                progress_handler(DownloadProgress {
                                    downloaded: 0,
                                    total,
                                    bytes_per_second: 0,
                                    connections_progress: Vec::new(),
                                    notice: Some(
                                        "The remote file changed during transfer; restarted download from the beginning."
                                            .to_owned(),
                                    ),
                                });
                                response = new_response;
                                continue 'stream;
                            }
                            Err(error) => return Err(error),
                        }
                    } else {
                        return Err(AppError::download(
                            "The downloaded byte count did not match the server's declared size.",
                        ));
                    }
                } else {
                    break 'stream;
                }
            }
            Err(read_error) => {
                if midstream_retries < MAX_TRANSIENT_RETRIES
                    && control.current_stop_reason().is_none()
                {
                    midstream_retries += 1;
                    file.flush().await.map_err(|error| {
                        AppError::download(format!("Could not save the partial download: {error}"))
                    })?;
                    file.sync_all().await.map_err(|error| {
                        AppError::download(format!("Could not save the partial download: {error}"))
                    })?;
                    wait_before_retry(control, retry_delay(midstream_retries, None)).await?;

                    let next_request = if supports_range && downloaded > 0 {
                        send_stream_request(&client, url.clone(), downloaded, new_validator.as_ref(), control).await
                    } else {
                        send_stream_request(&client, url.clone(), 0, None, control).await
                    };

                    match next_request {
                        Ok(new_response) => {
                            if new_response.status() == reqwest::StatusCode::FORBIDDEN
                                || new_response.status() == reqwest::StatusCode::GONE
                            {
                                return Err(AppError::download(format!(
                                    "The download link has expired (HTTP {}). Refresh the link to resume.",
                                    new_response.status().as_u16()
                                )));
                            }
                            if supports_range
                                && downloaded > 0
                                && new_response.status() == reqwest::StatusCode::PARTIAL_CONTENT
                            {
                                if let Ok((start, _, _)) = content_range(&new_response) {
                                    if start == downloaded {
                                        response = new_response;
                                        continue 'stream;
                                    }
                                }
                            }
                            drop(file);
                            let new_file = OpenOptions::new()
                                .write(true)
                                .create(true)
                                .truncate(true)
                                .open(&temporary)
                                .await
                                .map_err(|error| {
                                    AppError::download(format!(
                                        "Could not create a temporary download file: {error}"
                                    ))
                                })?;
                            file = new_file;
                            downloaded = 0;
                            total = new_response.content_length();
                            supports_range = new_response
                                .headers()
                                .get(reqwest::header::ACCEPT_RANGES)
                                .is_some_and(|value| value.as_bytes() == b"bytes");
                            progress_handler(DownloadProgress {
                                downloaded: 0,
                                total,
                                bytes_per_second: 0,
                                connections_progress: Vec::new(),
                                notice: Some(
                                    "The remote file changed during transfer; restarted download from the beginning."
                                        .to_owned(),
                                ),
                            });
                            response = new_response;
                            continue 'stream;
                        }
                        Err(error) => return Err(error),
                    }
                } else {
                    return Err(AppError::download(format!(
                        "The server response could not be read: {read_error}"
                    )));
                }
            }
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

        if last_tick.elapsed() >= Duration::from_millis(PROGRESS_EVENT_INTERVAL_MILLISECONDS) {
            let elapsed = last_tick.elapsed().as_secs_f64().max(0.001);
            let speed = (bytes_since_tick as f64 / elapsed) as u64;
            progress_handler(DownloadProgress {
                downloaded,
                total,
                bytes_per_second: speed,
                connections_progress: Vec::new(),
                notice: None,
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

    let result = finalize(
        download_id,
        temporary,
        destination,
        downloaded,
        total,
    )
    .await?;

    Ok((result, new_validator))
}

pub(crate) async fn send_stream_request(
    client: &reqwest::Client,
    url: Url,
    offset: u64,
    validator: Option<&SegmentValidator>,
    control: &mut DownloadControl,
) -> AppResult<reqwest::Response> {
    let client = client.clone();
    let val_header = validator.map(|v| v.header.clone());
    let val_value = validator.map(|v| v.value.clone());
    send_with_retry(control, move || {
        let mut request = client.get(url.clone());
        if offset > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
            if let (Some(ref h), Some(ref v)) = (&val_header, &val_value) {
                request = request.header(h.clone(), v.clone());
            }
        }
        request
    })
    .await
}
