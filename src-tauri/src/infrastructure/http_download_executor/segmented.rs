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

pub const MIN_CHUNK_REASSIGN_BYTES: u64 = 2 * 1024 * 1024; // 2 MB minimum remaining to split a chunk

pub(crate) async fn discover_ranges(
    client: &reqwest::Client,
    url: Url,
    control: &mut DownloadControl,
) -> AppResult<Option<(SegmentValidator, Option<String>)>> {
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
    let cd_filename = response
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .and_then(crate::domain::download::parse_content_disposition_filename);
    Ok(extract_validator(&response, total).map(|v| (v, cd_filename)))
}

pub(crate) async fn segment_ranges(
    total: u64,
    requested_connections: u8,
    directory: &Path,
    id: &str,
    existing: Option<&[DownloadSegmentProgress]>,
) -> AppResult<Vec<DownloadSegmentProgress>> {
    if let Some(existing_segs) = existing {
        if !existing_segs.is_empty() {
            let mut valid = true;
            let mut expected_start = 0_u64;
            for s in existing_segs {
                if s.start != expected_start || s.end < s.start {
                    valid = false;
                    break;
                }
                expected_start = s.end + 1;
            }
            if valid && expected_start == total {
                let mut restored = Vec::with_capacity(existing_segs.len());
                for (idx, seg) in existing_segs.iter().enumerate() {
                    let path = segment_path(directory, id, idx);
                    let downloaded = match fs::metadata(&path).await {
                        Ok(metadata) => metadata.len().min(seg.total),
                        Err(_) => 0,
                    };
                    restored.push(DownloadSegmentProgress {
                        start: seg.start,
                        end: seg.end,
                        downloaded,
                        total: seg.total,
                        status: if downloaded == seg.total {
                            "complete"
                        } else {
                            "queued"
                        }
                        .to_owned(),
                    });
                }
                return Ok(restored);
            }
        }
    }

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
        let index_u64 =
            u64::try_from(index).map_err(|_| AppError::download("The segment index is invalid."))?;
        let segment_total = base_size + u64::from(index_u64 < remainder);
        let end = start + segment_total - 1;
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
    connections: u8,
    directory: &Path,
    control: &mut DownloadControl,
    rate_limiter: Arc<DownloadRateLimiter>,
    progress_handler: &DownloadProgressHandler,
) -> AppResult<Vec<DownloadSegmentProgress>> {
    let segments_shared = Arc::new(Mutex::new(segments));
    let meter = Arc::new(Mutex::new(crate::application::throughput_meter::ThroughputMeter::new()));

    // Initial progress report
    {
        let current = segments_shared
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
            etag: Some(validator.value().to_string()),
            validator: Some(validator.validator.clone()),
        });
    }

    let worker_count = usize::from(connections.max(1));
    let mut jobs = tokio::task::JoinSet::new();

    for worker_id in 0..worker_count {
        let worker_client = client.clone();
        let worker_url = url.clone();
        let worker_dir = directory.to_path_buf();
        let worker_id_str = id.to_owned();
        let worker_segments = segments_shared.clone();
        let worker_control = control.clone();
        let worker_validator = validator.clone();
        let worker_rate_limiter = rate_limiter.clone();
        let worker_progress = progress_handler.clone();
        let worker_meter = meter.clone();

        jobs.spawn(async move {
            run_segment_worker(
                worker_client,
                worker_url,
                worker_dir,
                worker_id_str,
                worker_id,
                worker_segments,
                worker_validator,
                worker_control,
                worker_rate_limiter,
                worker_progress,
                worker_meter,
            )
            .await
        });
    }

    while let Some(result) = jobs.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) if matches!(error.code, "download_paused" | "download_cancelled") => {
                jobs.abort_all();
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

    let final_segments = segments_shared
        .lock()
        .map_err(|_| AppError::storage("Download progress is unavailable."))?
        .clone();
    Ok(final_segments)
}

async fn run_segment_worker(
    client: reqwest::Client,
    url: Url,
    directory: PathBuf,
    id: String,
    worker_id: usize,
    segments: Arc<Mutex<Vec<DownloadSegmentProgress>>>,
    validator: SegmentValidator,
    mut control: DownloadControl,
    rate_limiter: Arc<DownloadRateLimiter>,
    progress_handler: DownloadProgressHandler,
    meter: Arc<Mutex<crate::application::throughput_meter::ThroughputMeter>>,
) -> AppResult<()> {
    loop {
        if let Some(reason) = control.current_stop_reason() {
            return Err(stopped_error(reason));
        }

        let chunk_assignment = {
            let mut guard = segments
                .lock()
                .map_err(|_| AppError::storage("Download progress is unavailable."))?;

            // 1. Look for initial queued chunk
            let queued_pos = guard.iter().position(|s| s.status == "queued");
            if let Some(pos) = queued_pos {
                guard[pos].status = "downloading".to_owned();
                Some(pos)
            } else {
                // 2. Dynamic Chunk Rebalancing / Work Stealing (IDM feature)
                // When an idle worker has no queued chunk, check active connections for largest remaining range >= MIN_CHUNK_REASSIGN_BYTES
                let mut best_candidate: Option<(usize, u64)> = None;
                for (idx, seg) in guard.iter().enumerate() {
                    if seg.status == "downloading" {
                        let remaining = seg.total.saturating_sub(seg.downloaded);
                        if remaining >= MIN_CHUNK_REASSIGN_BYTES {
                            if best_candidate.is_none() || remaining > best_candidate.unwrap().1 {
                                best_candidate = Some((idx, remaining));
                            }
                        }
                    }
                }

                if let Some((target_idx, _)) = best_candidate {
                    let seg = &guard[target_idx];
                    let start = seg.start;
                    let downloaded = seg.downloaded;
                    let end = seg.end;

                    let unread_start = start + downloaded;
                    let unread_len = end.saturating_sub(unread_start) + 1;
                    if unread_len >= MIN_CHUNK_REASSIGN_BYTES {
                        let split_point = unread_start + (unread_len / 2);
                        if split_point > unread_start && split_point < end {
                            // Shorten the target chunk
                            guard[target_idx].end = split_point;
                            guard[target_idx].total = split_point - start + 1;

                            // Create reassigned upper-half chunk
                            let new_chunk_idx = guard.len();
                            let new_chunk = DownloadSegmentProgress {
                                start: split_point + 1,
                                end,
                                downloaded: 0,
                                total: end - (split_point + 1) + 1,
                                status: "downloading".to_owned(),
                            };
                            guard.push(new_chunk);

                            let notice = format!(
                                "The file chunk {} is reassigned to connection {}.",
                                new_chunk_idx + 1,
                                worker_id + 1
                            );

                            let total_downloaded =
                                guard.iter().map(|s| s.downloaded).sum();
                            let connections_progress = guard.clone();
                            drop(guard);

                            let bytes_per_second = meter
                                .lock()
                                .ok()
                                .and_then(|mut m| m.update(total_downloaded))
                                .unwrap_or(0);
                            progress_handler(DownloadProgress {
                                downloaded: total_downloaded,
                                total: Some(validator.total),
                                bytes_per_second,
                                connections_progress,
                                notice: Some(notice),
                                transfer_kind: Some("segmented_http".to_string()),
                                effective_concurrency: Some(worker_id as u8 + 1),
                                etag: Some(validator.value().to_string()),
                                validator: Some(validator.validator.clone()),
                            });

                            Some(new_chunk_idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        };

        match chunk_assignment {
            Some(chunk_idx) => {
                let path = segment_path(&directory, &id, chunk_idx);
                download_chunk(
                    client.clone(),
                    url.clone(),
                    path,
                    chunk_idx,
                    &segments,
                    &validator,
                    &mut control,
                    rate_limiter.clone(),
                    &progress_handler,
                    &meter,
                )
                .await?;
            }
            None => {
                // No more chunks to download or steal; worker exits
                break;
            }
        }
    }
    Ok(())
}

async fn download_chunk(
    client: reqwest::Client,
    url: Url,
    path: PathBuf,
    index: usize,
    segments: &Arc<Mutex<Vec<DownloadSegmentProgress>>>,
    validator: &SegmentValidator,
    control: &mut DownloadControl,
    rate_limiter: Arc<DownloadRateLimiter>,
    progress_handler: &DownloadProgressHandler,
    meter: &Arc<Mutex<crate::application::throughput_meter::ThroughputMeter>>,
) -> AppResult<()> {
    let (start_offset, end_offset, downloaded_init) = {
        let guard = segments
            .lock()
            .map_err(|_| AppError::storage("Download progress is unavailable."))?;
        let seg = guard
            .get(index)
            .ok_or_else(|| AppError::download("The chunk no longer exists."))?;
        (seg.start, seg.end, seg.downloaded)
    };

    if downloaded_init >= (end_offset - start_offset + 1) {
        update_segment_progress(
            segments,
            index,
            downloaded_init,
            "complete",
            &validator,
            progress_handler,
            meter,
            None,
        )?;
        return Ok(());
    }

    let req_start = start_offset + downloaded_init;
    let req_end = end_offset;

    let mut response = send_with_retry(control, || {
        client
            .get(url.clone())
            .header(reqwest::header::RANGE, format!("bytes={req_start}-{req_end}"))
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .header(validator.header(), validator.if_range_value())
    })
    .await?;

    if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(AppError::download(
            "The server did not honor a segmented byte-range request.",
        ));
    }
    let (actual_start, actual_end, actual_total) = content_range(&response)?;
    if actual_start != req_start
        || actual_end != req_end
        || actual_total != Some(validator.total)
        || response
            .headers()
            .get(&validator.entity_header())
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value != validator.value())
        || response
            .headers()
            .get(reqwest::header::CONTENT_ENCODING)
            .is_some_and(|value| value.as_bytes() != b"identity")
    {
        return Err(AppError::download(
            "The server returned an inconsistent byte range for a segment.",
        ));
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(downloaded_init > 0)
        .truncate(downloaded_init == 0)
        .open(&path)
        .await
        .map_err(|error| {
            AppError::download(format!("Could not create a partial segment: {error}"))
        })?;

    update_segment_progress(
        segments,
        index,
        downloaded_init,
        "downloading",
        &validator,
        progress_handler,
        meter,
        None,
    )?;

    let mut downloaded = downloaded_init;
    let mut midstream_retries = 0_u8;

    'chunk_stream: loop {
        let current_target = {
            let guard = segments
                .lock()
                .map_err(|_| AppError::storage("Download progress is unavailable."))?;
            guard[index].total
        };

        if downloaded >= current_target {
            break 'chunk_stream;
        }

        let chunk = tokio::select! {
            reason = control.stopped() => {
                let _ = file.sync_all().await;
                update_segment_progress(
                    segments,
                    index,
                    downloaded,
                    stop_status(reason),
                    &validator,
                    progress_handler,
                    meter,
                    None,
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
                let cur_target = {
                    let guard = segments
                        .lock()
                        .map_err(|_| AppError::storage("Download progress is unavailable."))?;
                    guard[index].total
                };
                if downloaded < cur_target {
                    if midstream_retries < MAX_TRANSIENT_RETRIES
                        && control.current_stop_reason().is_none()
                    {
                        midstream_retries += 1;
                        let _ = file.flush().await;
                        let _ = file.sync_all().await;
                        wait_before_retry(control, retry_delay(midstream_retries, None)).await?;
                        let (cur_start, cur_end) = {
                            let guard = segments
                                .lock()
                                .map_err(|_| AppError::storage("Download progress is unavailable."))?;
                            (guard[index].start, guard[index].end)
                        };
                        let start = cur_start + downloaded;
                        let next_resp = send_with_retry(control, || {
                            client
                                .get(url.clone())
                                .header(
                                    reqwest::header::RANGE,
                                    format!("bytes={start}-{cur_end}"),
                                )
                                .header(reqwest::header::ACCEPT_ENCODING, "identity")
                                .header(validator.header(), validator.if_range_value())
                        })
                        .await;
                        match next_resp {
                            Ok(new_resp) => {
                                if new_resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
                                    if let Ok((act_s, act_e, act_tot)) = content_range(&new_resp) {
                                        let entity_ok = new_resp
                                            .headers()
                                            .get(&validator.entity_header())
                                            .and_then(|v| v.to_str().ok())
                                            .map_or(true, |v| v == validator.value());
                                        let encoding_ok = !new_resp
                                            .headers()
                                            .get(reqwest::header::CONTENT_ENCODING)
                                            .is_some_and(|v| v.as_bytes() != b"identity");
                                        if act_s == start
                                            && act_e == cur_end
                                            && act_tot == Some(validator.total)
                                            && entity_ok
                                            && encoding_ok
                                        {
                                            response = new_resp;
                                            continue 'chunk_stream;
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
                    break 'chunk_stream;
                }
            }
            Err(read_error) => {
                if midstream_retries < MAX_TRANSIENT_RETRIES
                    && control.current_stop_reason().is_none()
                {
                    midstream_retries += 1;
                    let _ = file.flush().await;
                    let _ = file.sync_all().await;
                    wait_before_retry(control, retry_delay(midstream_retries, None)).await?;
                    let (cur_start, cur_end) = {
                        let guard = segments
                            .lock()
                            .map_err(|_| AppError::storage("Download progress is unavailable."))?;
                        (guard[index].start, guard[index].end)
                    };
                    let start = cur_start + downloaded;
                    let next_resp = send_with_retry(control, || {
                        client
                            .get(url.clone())
                            .header(
                                reqwest::header::RANGE,
                                format!("bytes={start}-{cur_end}"),
                            )
                            .header(reqwest::header::ACCEPT_ENCODING, "identity")
                            .header(validator.header(), validator.if_range_value())
                    })
                    .await;
                    match next_resp {
                        Ok(new_resp) => {
                            if new_resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
                                if let Ok((act_s, act_e, act_tot)) = content_range(&new_resp) {
                                    let entity_ok = new_resp
                                        .headers()
                                        .get(&validator.entity_header())
                                        .and_then(|v| v.to_str().ok())
                                        .map_or(true, |v| v == validator.value());
                                    let encoding_ok = !new_resp
                                        .headers()
                                        .get(reqwest::header::CONTENT_ENCODING)
                                        .is_some_and(|v| v.as_bytes() != b"identity");
                                    if act_s == start
                                        && act_e == cur_end
                                        && act_tot == Some(validator.total)
                                        && entity_ok
                                        && encoding_ok
                                    {
                                        response = new_resp;
                                        continue 'chunk_stream;
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

        let current_target = {
            let guard = segments
                .lock()
                .map_err(|_| AppError::storage("Download progress is unavailable."))?;
            guard[index].total
        };
        let remaining_in_chunk = current_target.saturating_sub(downloaded);
        if remaining_in_chunk == 0 {
            break 'chunk_stream;
        }

        let chunk_size = u64::try_from(chunk.len())
            .map_err(|_| AppError::download("The downloaded chunk size is invalid."))?;
        let to_write = chunk_size.min(remaining_in_chunk);

        let throttled = tokio::select! {
            reason = control.stopped() => Err(stopped_error(reason)),
            () = rate_limiter.throttle(to_write) => Ok(()),
        };
        if let Err(error) = throttled {
            let _ = file.sync_all().await;
            let reason = control
                .current_stop_reason()
                .unwrap_or(DownloadStopReason::Cancel);
            update_segment_progress(
                segments,
                index,
                downloaded,
                stop_status(reason),
                &validator,
                progress_handler,
                meter,
                None,
            )?;
            return Err(error);
        }

        file.write_all(&chunk[..to_write as usize])
            .await
            .map_err(|error| {
                AppError::download(format!("Could not write a partial segment: {error}"))
            })?;
        downloaded += to_write;

        update_segment_progress(
            segments,
            index,
            downloaded,
            "downloading",
            &validator,
            progress_handler,
            meter,
            None,
        )?;

        if downloaded >= current_target {
            break 'chunk_stream;
        }
    }

    file.flush().await.map_err(|error| {
        AppError::download(format!("Could not finalize a partial segment: {error}"))
    })?;
    file.sync_all().await.map_err(|error| {
        AppError::download(format!("Could not finalize a partial segment: {error}"))
    })?;

    update_segment_progress(
        segments,
        index,
        downloaded,
        "complete",
        &validator,
        progress_handler,
        meter,
        None,
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
    validator: &SegmentValidator,
    progress_handler: &DownloadProgressHandler,
    meter: &Arc<Mutex<crate::application::throughput_meter::ThroughputMeter>>,
    notice: Option<String>,
) -> AppResult<()> {
    let mut segments_guard = segments
        .lock()
        .map_err(|_| AppError::storage("Download progress is unavailable."))?;
    if let Some(segment) = segments_guard.get_mut(index) {
        segment.downloaded = downloaded;
        segment.status = status.to_owned();
    }
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
        total: Some(validator.total),
        bytes_per_second,
        connections_progress,
        notice,
        transfer_kind: Some("segmented_http".to_string()),
        effective_concurrency: Some(count),
        etag: Some(validator.value().to_string()),
        validator: Some(validator.validator.clone()),
    });
    Ok(())
}
