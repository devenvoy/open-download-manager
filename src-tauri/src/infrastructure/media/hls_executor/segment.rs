// Created: 2026-11-05
// Reason: feat: add media handling for HLS, FFmpeg, yt-dlp, and stream progress
// Last updated: 2026-11-10
// Reason: refactor: modularize HLS executor into plan, segment, and coordinator modules
// Description: Individual HLS chunk download, rate limiting, progress tracking, and file concatenation.

use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::{fs::OpenOptions, io::{AsyncReadExt, AsyncWriteExt}};

use crate::{
    application::{
        download_executor::{stopped_error, DownloadControl, DownloadProgressHandler},
        download_progress::DownloadProgress,
        download_rate_limiter::DownloadRateLimiter,
        error::{AppError, AppResult},
    },
    domain::media::calculate_stream_progress,
};

use super::HlsTransferState;

pub(crate) async fn download_hls_segment(
    client: &reqwest::Client,
    segment: &crate::domain::media::HlsSegment,
    index: usize,
    worker_id: usize,
    segment_count: usize,
    parts_dir: &Path,
    state: &Arc<Mutex<HlsTransferState>>,
    control: &mut DownloadControl,
    rate_limiter: Arc<DownloadRateLimiter>,
    progress_handler: &DownloadProgressHandler,
    referer: Option<&str>,
) -> AppResult<()> {
    let temp_part = parts_dir.join(format!("{index}.receiving"));
    let final_part = parts_dir.join(format!("{index}.part"));

    {
        let mut st = state.lock().unwrap();
        st.worker_progress[worker_id].current_segment = Some(index);
        st.worker_progress[worker_id].segment_downloaded = 0;
    }

    let mut req = client.get(&segment.url);
    if let Some(r) = referer {
        req = req.header(reqwest::header::REFERER, r);
    }
    if let Some(range) = segment.byte_range {
        req = req.header(
            reqwest::header::RANGE,
            format!("bytes={}-{}", range.offset, range.offset + range.length - 1),
        );
    }

    let mut response = tokio::select! {
        reason = control.stopped() => return Err(stopped_error(reason)),
        res = req.send() => res.map_err(|e| {
            let safe_url = crate::domain::media::redact_media_url(&segment.url);
            AppError::download(format!("Could not download segment {index} from {safe_url}: {e}"))
        })?,
    };

    if !response.status().is_success() {
        return Err(AppError::download(format!(
            "HLS segment returned HTTP {}.",
            response.status().as_u16()
        )));
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temp_part)
        .await
        .map_err(|e| AppError::download(format!("Could not create segment file: {e}")))?;

    let is_status_200 = response.status() == reqwest::StatusCode::OK;
    let (mut skip_remaining, max_bytes) = if let Some(range) = segment.byte_range {
        if is_status_200 {
            (range.offset, Some(range.length))
        } else {
            (0, Some(range.length))
        }
    } else {
        (0, None)
    };

    let mut segment_downloaded = 0_u64;
    loop {
        let chunk = tokio::select! {
            reason = control.stopped() => {
                let _ = tokio::fs::remove_file(&temp_part).await;
                return Err(stopped_error(reason));
            }
            res = response.chunk() => res.map_err(|e| AppError::download(format!("Could not read segment chunk: {e}")))?,
        };

        let Some(chunk) = chunk else {
            break;
        };

        let chunk_data = if skip_remaining > 0 {
            let skip = (chunk.len() as u64).min(skip_remaining);
            skip_remaining -= skip;
            &chunk[skip as usize..]
        } else {
            &chunk[..]
        };

        if chunk_data.is_empty() {
            continue;
        }

        let to_write = if let Some(limit) = max_bytes {
            let remaining = limit.saturating_sub(segment_downloaded);
            if remaining == 0 {
                break;
            }
            let take = (chunk_data.len() as u64).min(remaining) as usize;
            &chunk_data[..take]
        } else {
            chunk_data
        };

        let chunk_size = to_write.len() as u64;
        let throttled = tokio::select! {
            reason = control.stopped() => {
                let _ = tokio::fs::remove_file(&temp_part).await;
                return Err(stopped_error(reason));
            }
            () = rate_limiter.throttle(chunk_size) => Ok(()),
        };
        if let Err(e) = throttled {
            return Err(e);
        }

        let write_res = tokio::select! {
            reason = control.stopped() => {
                let _ = tokio::fs::remove_file(&temp_part).await;
                return Err(stopped_error(reason));
            }
            res = file.write_all(to_write) => res.map_err(|e| AppError::download(format!("Could not write segment: {e}"))),
        };
        write_res?;

        segment_downloaded += chunk_size;

        {
            let mut st = state.lock().unwrap();
            st.segments[index].downloaded = segment_downloaded;
            st.worker_progress[worker_id].segment_downloaded = segment_downloaded;

            let total_now: u64 = st.segments.iter().map(|s| s.downloaded).sum();
            if let Some(speed) = st.meter.update(total_now) {
                let stream_prog = calculate_stream_progress(&st.segments);
                let conns = st.build_connections_progress(segment_count);
                let eff_conns = st.worker_progress.len() as u8;
                drop(st);

                progress_handler(DownloadProgress {
                    downloaded: total_now,
                    total: stream_prog.estimated_bytes,
                    bytes_per_second: speed,
                    connections_progress: conns,
                    notice: None,
                    transfer_kind: Some("hls".to_string()),
                    effective_concurrency: Some(eff_conns),
                    etag: None,
                    validator: None,
                });
            }
        }

        if let Some(limit) = max_bytes {
            if segment_downloaded >= limit {
                break;
            }
        }
    }

    file.flush()
        .await
        .map_err(|e| AppError::download(format!("Could not flush segment: {e}")))?;
    drop(file);

    tokio::fs::rename(&temp_part, &final_part)
        .await
        .map_err(|e| AppError::download(format!("Could not commit segment: {e}")))?;

    {
        let mut st = state.lock().unwrap();
        st.segments[index].done = true;
        st.segments[index].downloaded = segment_downloaded;
        st.worker_progress[worker_id].segment_downloaded = segment_downloaded;

        let total_downloaded: u64 = st.segments.iter().map(|s| s.downloaded).sum();
        let all_done = st.segments.iter().all(|s| s.done);
        let speed_opt = st.meter.update(total_downloaded);
        if all_done || speed_opt.is_some() {
            let stream_prog = calculate_stream_progress(&st.segments);
            let speed = speed_opt.unwrap_or_else(|| st.meter.current_throughput());
            let conns = st.build_connections_progress(segment_count);
            let eff_conns = st.worker_progress.len() as u8;
            drop(st);

            progress_handler(DownloadProgress {
                downloaded: total_downloaded,
                total: stream_prog.estimated_bytes,
                bytes_per_second: speed,
                connections_progress: conns,
                notice: None,
                transfer_kind: Some("hls".to_string()),
                effective_concurrency: Some(eff_conns),
                etag: None,
                validator: None,
            });
        }
    }

    Ok(())
}

pub(crate) async fn concat_files(
    parts_dir: &Path,
    start: usize,
    end: usize,
    output: &Path,
) -> AppResult<()> {
    let mut out_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(output)
        .await
        .map_err(|e| AppError::download(format!("Could not create combined file: {e}")))?;

    let mut buffer = vec![0u8; 64 * 1024];
    for i in start..end {
        let part_path = parts_dir.join(format!("{i}.part"));
        let mut in_file = tokio::fs::File::open(&part_path)
            .await
            .map_err(|e| AppError::download(format!("Could not open segment part {i}: {e}")))?;

        loop {
            let bytes_read = in_file
                .read(&mut buffer)
                .await
                .map_err(|e| AppError::download(format!("Could not read segment part {i}: {e}")))?;
            if bytes_read == 0 {
                break;
            }
            out_file
                .write_all(&buffer[..bytes_read])
                .await
                .map_err(|e| AppError::download(format!("Could not write to combined output: {e}")))?;
        }
    }

    out_file
        .flush()
        .await
        .map_err(|e| AppError::download(format!("Could not flush combined output: {e}")))?;
    Ok(())
}
