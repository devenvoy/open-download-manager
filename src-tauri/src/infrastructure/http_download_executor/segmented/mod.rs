// Created: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Last updated: 2026-11-10
// Reason: refactor: modularize segmented download engine into discovery, worker, progress submodules
// Description: Segmented HTTP download engine splitting files across concurrent range connections.

use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use url::Url;

use crate::{
    application::{
        download_executor::{DownloadControl, DownloadProgressHandler},
        download_progress::DownloadProgress,
        download_rate_limiter::DownloadRateLimiter,
        error::{AppError, AppResult},
    },
    domain::download::DownloadSegmentProgress,
};

use super::{
    policy::PROGRESS_EVENT_INTERVAL_MILLISECONDS,
    validator::SegmentValidator,
};

pub mod discovery;
pub mod progress;
pub mod worker;

pub(crate) use discovery::{discover_ranges, segment_ranges};
pub(crate) use worker::run_segment_worker;

pub const MIN_CHUNK_REASSIGN_BYTES: u64 = 2 * 1024 * 1024; // 2 MB minimum remaining to split a chunk

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
    auth_header: Option<String>,
    referer: Option<String>,
    request_headers: crate::domain::download::BrowserRequestHeaders,
    initial_host: String,
) -> AppResult<Vec<DownloadSegmentProgress>> {
    let segments_shared = Arc::new(Mutex::new(segments));
    let meter = Arc::new(Mutex::new(crate::application::throughput_meter::ThroughputMeter::new()));
    let auth_shared: Option<Arc<str>> = auth_header.map(Arc::from);
    let referer_shared: Option<Arc<str>> = referer.map(Arc::from);
    let headers_shared = Arc::new(request_headers);
    let initial_host_shared: Arc<str> = Arc::from(initial_host);

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
            etag: validator.value().map(|s| s.to_string()),
            validator: validator.validator.clone(),
            filename: None,
        });
    }

    let worker_count = usize::from(connections.max(1));
    let mut jobs = tokio::task::JoinSet::new();

    let ticker_segments = segments_shared.clone();
    let ticker_meter = meter.clone();
    let ticker_validator = validator.clone();
    let ticker_progress = progress_handler.clone();
    let mut ticker_control = control.clone();

    let ticker_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(PROGRESS_EVENT_INTERVAL_MILLISECONDS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let (total_downloaded, connections_progress) = {
                        let Ok(guard) = ticker_segments.lock() else { break; };
                        let total: u64 = guard.iter().map(|s| s.downloaded).sum();
                        (total, guard.clone())
                    };
                    let speed = ticker_meter
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
                    let count = u8::try_from(connections_progress.len()).unwrap_or(1);
                    ticker_progress(DownloadProgress {
                        downloaded: total_downloaded,
                        total: Some(ticker_validator.total),
                        bytes_per_second: speed,
                        connections_progress,
                        notice: None,
                        transfer_kind: Some("segmented_http".to_string()),
                        effective_concurrency: Some(count),
                        etag: ticker_validator.value().map(|s| s.to_string()),
                        validator: ticker_validator.validator.clone(),
                        filename: None,
                    });
                }
                _ = ticker_control.stopped() => {
                    break;
                }
            }
        }
    });

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
        let worker_auth = auth_shared.clone();
        let worker_referer = referer_shared.clone();
        let worker_headers = headers_shared.clone();
        let worker_initial_host = initial_host_shared.clone();

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
                worker_auth,
                worker_referer,
                worker_headers,
                worker_initial_host,
            )
            .await
        });
    }

    while let Some(result) = jobs.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) if matches!(error.code, "download_paused" | "download_cancelled") => {
                ticker_handle.abort();
                jobs.abort_all();
                while jobs.join_next().await.is_some() {}
                return Err(error);
            }
            Ok(Err(error)) => {
                ticker_handle.abort();
                jobs.abort_all();
                while jobs.join_next().await.is_some() {}
                return Err(error);
            }
            Err(error) => {
                ticker_handle.abort();
                jobs.abort_all();
                while jobs.join_next().await.is_some() {}
                return Err(AppError::download(format!(
                    "A segmented transfer stopped unexpectedly: {error}"
                )));
            }
        }
    }

    ticker_handle.abort();

    let final_segments = segments_shared
        .lock()
        .map_err(|_| AppError::storage("Download progress is unavailable."))?
        .clone();
    let final_downloaded: u64 = final_segments.iter().map(|s| s.downloaded).sum();
    let final_speed = meter.lock().ok().map(|m| m.current_throughput()).unwrap_or(0);
    let final_count = u8::try_from(final_segments.len()).unwrap_or(1);
    progress_handler(DownloadProgress {
        downloaded: final_downloaded,
        total: Some(validator.total),
        bytes_per_second: final_speed,
        connections_progress: final_segments.clone(),
        notice: None,
        transfer_kind: Some("segmented_http".to_string()),
        effective_concurrency: Some(final_count),
        etag: validator.value().map(|s| s.to_string()),
        validator: validator.validator.clone(),
        filename: None,
    });
    Ok(final_segments)
}
