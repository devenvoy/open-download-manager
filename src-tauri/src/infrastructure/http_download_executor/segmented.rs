use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use url::Url;

use crate::{
    application::{
        download_executor::{
            stopped_error, DownloadControl, DownloadProgressHandler, DownloadStopReason,
        },
        download_progress::DownloadProgress,
        download_rate_limiter::DownloadRateLimiter,
        error::{AppError, AppResult},
    },
    domain::download::DownloadSegmentProgress,
};

use super::{
    assembler::segment_path,
    policy::{MAX_TRANSIENT_RETRIES, READ_TIMEOUT_SECONDS},
    retry::{retry_delay, send_with_retry, wait_before_retry},
    validator::{content_range, extract_validator, SegmentValidator},
};

pub(crate) async fn discover_ranges(
    client: &reqwest::Client,
    url: Url,
    control: &mut DownloadControl,
) -> AppResult<Option<SegmentValidator>> {
    let response = send_with_retry(control, || {
        client
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
    Ok(extract_validator(&response, total))
}

pub(crate) async fn segment_ranges(
    total: u64,
    requested_connections: u8,
    directory: &Path,
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
        let index =
            u64::try_from(index).map_err(|_| AppError::download("The segment index is invalid."))?;
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

pub(crate) async fn download_segments(
    client: reqwest::Client,
    id: &str,
    url: Url,
    validator: &SegmentValidator,
    segments: Vec<DownloadSegmentProgress>,
    directory: &Path,
    control: &mut DownloadControl,
    rate_limiter: Arc<DownloadRateLimiter>,
    progress_handler: &DownloadProgressHandler,
) -> AppResult<()> {
    let segments = Arc::new(Mutex::new(segments));
    let meter = Arc::new(Mutex::new(crate::application::throughput_meter::ThroughputMeter::new()));
    let mut jobs = tokio::task::JoinSet::new();
    {
        let current = segments
            .lock()
            .map_err(|_| AppError::storage("Download progress is unavailable."))?
            .clone();
        let count = current.len() as u8;
        progress_handler(DownloadProgress {
            downloaded: current.iter().map(|segment| segment.downloaded).sum(),
            total: Some(validator.total),
            bytes_per_second: 0,
            connections_progress: current,
            notice: None,
            transfer_kind: Some("segmented_http".to_string()),
            effective_concurrency: Some(count),
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
        let segment_client = client.clone();
        let segment_index =
            u8::try_from(index).map_err(|_| AppError::download("The segment index is invalid."))?;
        let path = segment_path(directory, id, segment_index);
        let segment_progress = segments.clone();
        let control = control.clone();
        let validator = validator.clone();
        let rate_limiter = rate_limiter.clone();
        let progress_handler = progress_handler.clone();
        let meter = meter.clone();
        let url = url.clone();
        jobs.spawn(async move {
            download_segment(
                segment_client,
                url,
                path,
                index,
                segment,
                validator,
                segment_progress,
                control,
                rate_limiter,
                progress_handler,
                meter,
            )
            .await
        });
    }
    while let Some(result) = jobs.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) if matches!(error.code, "download_paused" | "download_cancelled") => {
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
    meter: Arc<Mutex<crate::application::throughput_meter::ThroughputMeter>>,
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
        &meter,
    )?;
    let mut downloaded = segment.downloaded;
    let mut midstream_retries = 0_u8;
    'segment_stream: loop {
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
                    &meter,
                )?;
                return Err(stopped_error(reason));
            }
            result = tokio::time::timeout(
                Duration::from_secs(READ_TIMEOUT_SECONDS),
                response.chunk(),
            ) => match result {
                Ok(chunk_res) => chunk_res.map_err(|e| e.to_string()),
                Err(_) => Err("Connection timed out while waiting for segment data from server.".to_owned()),
            },
        };
        let chunk = match chunk {
            Ok(Some(chunk)) => chunk,
            Ok(None) => {
                if downloaded < segment.total {
                    if midstream_retries < MAX_TRANSIENT_RETRIES
                        && control.current_stop_reason().is_none()
                    {
                        midstream_retries += 1;
                        file.flush().await.map_err(|error| {
                            AppError::download(format!("Could not save a partial segment: {error}"))
                        })?;
                        file.sync_all().await.map_err(|error| {
                            AppError::download(format!("Could not save a partial segment: {error}"))
                        })?;
                        wait_before_retry(&mut control, retry_delay(midstream_retries, None)).await?;
                        let start = segment.start + downloaded;
                        let next_resp = send_with_retry(&mut control, || {
                            client
                                .get(url.clone())
                                .header(
                                    reqwest::header::RANGE,
                                    format!("bytes={start}-{}", segment.end),
                                )
                                .header(reqwest::header::ACCEPT_ENCODING, "identity")
                                .header(validator.header.clone(), &validator.value)
                        })
                        .await;
                        match next_resp {
                            Ok(new_resp) => {
                                if new_resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
                                    if let Ok((actual_start, actual_end, _)) = content_range(&new_resp) {
                                        if actual_start == start && actual_end == segment.end {
                                            response = new_resp;
                                            continue 'segment_stream;
                                        }
                                    }
                                }
                                return Err(AppError::download(
                                    "The server returned an inconsistent byte range for a segment.",
                                ));
                            }
                            Err(error) => return Err(error),
                        }
                    } else {
                        return Err(AppError::download(
                            "The server response ended before the requested segment was complete.",
                        ));
                    }
                } else {
                    break 'segment_stream;
                }
            }
            Err(read_error) => {
                if midstream_retries < MAX_TRANSIENT_RETRIES
                    && control.current_stop_reason().is_none()
                {
                    midstream_retries += 1;
                    file.flush().await.map_err(|error| {
                        AppError::download(format!("Could not save a partial segment: {error}"))
                    })?;
                    file.sync_all().await.map_err(|error| {
                        AppError::download(format!("Could not save a partial segment: {error}"))
                    })?;
                    wait_before_retry(&mut control, retry_delay(midstream_retries, None)).await?;
                    let start = segment.start + downloaded;
                    let next_resp = send_with_retry(&mut control, || {
                        client
                            .get(url.clone())
                            .header(
                                reqwest::header::RANGE,
                                format!("bytes={start}-{}", segment.end),
                            )
                            .header(reqwest::header::ACCEPT_ENCODING, "identity")
                            .header(validator.header.clone(), &validator.value)
                    })
                    .await;
                    match next_resp {
                        Ok(new_resp) => {
                            if new_resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
                                if let Ok((actual_start, actual_end, _)) = content_range(&new_resp) {
                                    if actual_start == start && actual_end == segment.end {
                                        response = new_resp;
                                        continue 'segment_stream;
                                    }
                                }
                            }
                            return Err(AppError::download(
                                "The server returned an inconsistent byte range for a segment.",
                            ));
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
                &meter,
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
            &meter,
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
        &meter,
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
    meter: &Arc<Mutex<crate::application::throughput_meter::ThroughputMeter>>,
) -> AppResult<()> {
    let mut segments_guard = segments
        .lock()
        .map_err(|_| AppError::storage("Download progress is unavailable."))?;
    let segment = segments_guard
        .get_mut(index)
        .ok_or_else(|| AppError::download("The segment no longer exists."))?;
    segment.downloaded = downloaded;
    segment.status = status.to_owned();
    let total_downloaded = segments_guard
        .iter()
        .map(|segment| segment.downloaded)
        .fold(0_u64, u64::saturating_add);
    let connections_progress = segments_guard.clone();
    drop(segments_guard);

    let bytes_per_second = meter
        .lock()
        .ok()
        .and_then(|mut m| m.update(total_downloaded))
        .unwrap_or(0);
    let count = u8::try_from(connections_progress.len()).unwrap_or(1);

    progress_handler(DownloadProgress {
        downloaded: total_downloaded,
        total: Some(total),
        bytes_per_second,
        connections_progress,
        notice: None,
        transfer_kind: Some("segmented_http".to_string()),
        effective_concurrency: Some(count),
    });
    Ok(())
}
