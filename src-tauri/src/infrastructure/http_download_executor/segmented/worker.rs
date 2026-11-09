// Created: 2026-11-10
// Reason: refactor: modularize segmented download engine into discovery, worker, progress submodules
// Description: Segment worker task and chunk streaming engine with dynamic range rebalancing.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{
    fs::OpenOptions,
    io::AsyncWriteExt,
};
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
    super::{
        assembler::segment_path,
        policy::{MAX_TRANSIENT_RETRIES, READ_TIMEOUT_SECONDS},
        retry::{retry_delay, send_with_retry, wait_before_retry},
        validator::{content_range, SegmentValidator},
    },
    progress::{stop_status, update_segment_progress},
    MIN_CHUNK_REASSIGN_BYTES,
};

pub(crate) async fn run_segment_worker(
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
    auth_header: Option<Arc<str>>,
    referer: Option<Arc<str>>,
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
                                .and_then(|mut m| {
                                    if let Some(s) = m.update(total_downloaded) {
                                        Some(s)
                                    } else {
                                        Some(m.current_throughput())
                                    }
                                })
                                .unwrap_or(0);
                            progress_handler(DownloadProgress {
                                downloaded: total_downloaded,
                                total: Some(validator.total),
                                bytes_per_second,
                                connections_progress,
                                notice: Some(notice),
                                transfer_kind: Some("segmented_http".to_string()),
                                effective_concurrency: Some(worker_id as u8 + 1),
                                etag: validator.value().map(|s| s.to_string()),
                                validator: validator.validator.clone(),
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
                    auth_header.clone(),
                    referer.clone(),
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
    auth_header: Option<Arc<str>>,
    referer: Option<Arc<str>>,
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
            validator,
            progress_handler,
            meter,
            None,
        )?;
        return Ok(());
    }

    let req_start = start_offset + downloaded_init;
    let req_end = end_offset;

    let auth_for_init = auth_header.clone();
    let ref_for_init = referer.clone();
    let mut response = send_with_retry(control, || {
        let mut req = client
            .get(url.clone())
            .header(reqwest::header::RANGE, format!("bytes={req_start}-{req_end}"))
            .header(reqwest::header::ACCEPT_ENCODING, "identity");
        if let Some(if_range) = validator.if_range_value() {
            req = req.header(validator.header(), if_range);
        }
        if let Some(ref auth) = auth_for_init {
            req = req.header(reqwest::header::AUTHORIZATION, auth.as_ref());
        }
        if let Some(ref r) = ref_for_init {
            req = req.header(reqwest::header::REFERER, r.as_ref());
        }
        req
    })
    .await?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AppError::link_expired(
            "Authentication required or session expired (HTTP 401). Refresh the link to resume.",
        ));
    }
    if response.status() == reqwest::StatusCode::FORBIDDEN
        || response.status() == reqwest::StatusCode::GONE
    {
        return Err(AppError::link_expired(format!(
            "The download link has expired (HTTP {}). Refresh the link to resume.",
            response.status().as_u16()
        )));
    }

    if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(AppError::download(
            "The server did not honor a segmented byte-range request.",
        ));
    }
    let (actual_start, actual_end, actual_total) = content_range(&response)?;
    let entity_ok = match (validator.entity_header(), validator.value()) {
        (Some(h), Some(v)) => response
            .headers()
            .get(&h)
            .and_then(|value| value.to_str().ok())
            .map_or(true, |value| value == v),
        _ => true,
    };
    if actual_start != req_start
        || actual_end != req_end
        || actual_total != Some(validator.total)
        || !entity_ok
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
        validator,
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
                    validator,
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
                        let auth_retry1 = auth_header.clone();
                        let ref_retry1 = referer.clone();
                        let next_resp = send_with_retry(control, || {
                            let mut req = client
                                .get(url.clone())
                                .header(
                                    reqwest::header::RANGE,
                                    format!("bytes={start}-{cur_end}"),
                                )
                                .header(reqwest::header::ACCEPT_ENCODING, "identity");
                            if let Some(if_range) = validator.if_range_value() {
                                req = req.header(validator.header(), if_range);
                            }
                            if let Some(ref auth) = auth_retry1 {
                                req = req.header(reqwest::header::AUTHORIZATION, auth.as_ref());
                            }
                            if let Some(ref r) = ref_retry1 {
                                req = req.header(reqwest::header::REFERER, r.as_ref());
                            }
                            req
                        })
                        .await;
                        match next_resp {
                            Ok(new_resp) => {
                                if new_resp.status() == reqwest::StatusCode::UNAUTHORIZED {
                                    return Err(AppError::link_expired(
                                        "Authentication required or session expired (HTTP 401). Refresh the link to resume.",
                                    ));
                                }
                                if new_resp.status() == reqwest::StatusCode::FORBIDDEN
                                    || new_resp.status() == reqwest::StatusCode::GONE
                                {
                                    return Err(AppError::link_expired(format!(
                                        "The download link has expired (HTTP {}). Refresh the link to resume.",
                                        new_resp.status().as_u16()
                                    )));
                                }
                                if new_resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
                                    if let Ok((act_s, act_e, act_tot)) = content_range(&new_resp) {
                                        let entity_ok = match (validator.entity_header(), validator.value()) {
                                            (Some(h), Some(expected_val)) => new_resp
                                                .headers()
                                                .get(&h)
                                                .and_then(|hdr| hdr.to_str().ok())
                                                .map_or(true, |actual| actual == expected_val),
                                            _ => true,
                                        };
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
                    let auth_retry2 = auth_header.clone();
                    let ref_retry2 = referer.clone();
                    let next_resp = send_with_retry(control, || {
                        let mut req = client
                            .get(url.clone())
                            .header(
                                reqwest::header::RANGE,
                                format!("bytes={start}-{cur_end}"),
                            )
                            .header(reqwest::header::ACCEPT_ENCODING, "identity");
                        if let Some(if_range) = validator.if_range_value() {
                            req = req.header(validator.header(), if_range);
                        }
                        if let Some(ref auth) = auth_retry2 {
                            req = req.header(reqwest::header::AUTHORIZATION, auth.as_ref());
                        }
                        if let Some(ref r) = ref_retry2 {
                            req = req.header(reqwest::header::REFERER, r.as_ref());
                        }
                        req
                    })
                    .await;
                    match next_resp {
                        Ok(new_resp) => {
                            if new_resp.status() == reqwest::StatusCode::UNAUTHORIZED {
                                return Err(AppError::link_expired(
                                    "Authentication required or session expired (HTTP 401). Refresh the link to resume.",
                                ));
                            }
                            if new_resp.status() == reqwest::StatusCode::FORBIDDEN
                                || new_resp.status() == reqwest::StatusCode::GONE
                            {
                                return Err(AppError::link_expired(format!(
                                    "The download link has expired (HTTP {}). Refresh the link to resume.",
                                    new_resp.status().as_u16()
                                )));
                            }
                            if new_resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
                                if let Ok((act_s, act_e, act_tot)) = content_range(&new_resp) {
                                    let entity_ok = match (validator.entity_header(), validator.value()) {
                                        (Some(h), Some(expected_val)) => new_resp
                                            .headers()
                                            .get(&h)
                                            .and_then(|hdr| hdr.to_str().ok())
                                            .map_or(true, |actual| actual == expected_val),
                                        _ => true,
                                    };
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
                validator,
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

        if let Ok(mut guard) = segments.lock() {
            if let Some(segment) = guard.get_mut(index) {
                segment.downloaded = downloaded;
            }
        }

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
        validator,
        progress_handler,
        meter,
        None,
    )
}
