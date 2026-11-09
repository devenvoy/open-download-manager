// Created: 2026-11-10
// Reason: refactor: modularize segmented download engine into discovery, worker, progress submodules
// Description: Segment progress tracking and stop reason translation.

use std::sync::{Arc, Mutex};

use crate::{
    application::{
        download_executor::{DownloadProgressHandler, DownloadStopReason},
        download_progress::DownloadProgress,
        error::{AppError, AppResult},
    },
    domain::download::DownloadSegmentProgress,
};

use super::super::validator::SegmentValidator;

pub fn stop_status(reason: DownloadStopReason) -> &'static str {
    match reason {
        DownloadStopReason::Pause => "paused",
        DownloadStopReason::Cancel => "cancelled",
    }
}

pub fn update_segment_progress(
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
        .and_then(|mut m| {
            if let Some(s) = m.update(total_downloaded) {
                Some(s)
            } else {
                Some(m.current_throughput())
            }
        })
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
        etag: validator.value().map(|s| s.to_string()),
        validator: validator.validator.clone(),
    });
    Ok(())
}
