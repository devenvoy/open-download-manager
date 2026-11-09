// Created: 2026-11-05
// Reason: feat: add media handling for HLS, FFmpeg, yt-dlp, and stream progress
// Last updated: 2026-11-10
// Reason: refactor: modularize HLS executor into plan, segment, and coordinator modules
// Description: HLS stream executor downloading individual media segments and assembling into continuous streams.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{
    application::{
        download_executor::{stopped_error, DownloadControl, DownloadProgressHandler},
        download_rate_limiter::DownloadRateLimiter,
        download_result::DownloadResult,
        error::{AppError, AppResult},
        throughput_meter::ThroughputMeter,
    },
    domain::{
        download::{DownloadRecord, DownloadSegmentProgress},
        media::StreamSegment,
    },
    infrastructure::media::ffmpeg::FfmpegTools,
};

pub mod plan;
pub mod segment;

pub use plan::ResolvedHlsPlan;
use segment::{concat_files, download_hls_segment};

pub(crate) struct WorkerProgressState {
    pub(crate) current_segment: Option<usize>,
    pub(crate) segment_downloaded: u64,
}

pub(crate) struct HlsTransferState {
    pub(crate) segments: Vec<StreamSegment>,
    pub(crate) meter: ThroughputMeter,
    pub(crate) worker_progress: Vec<WorkerProgressState>,
}

impl HlsTransferState {
    pub(crate) fn build_connections_progress(&self, segment_count: usize) -> Vec<DownloadSegmentProgress> {
        self.worker_progress
            .iter()
            .enumerate()
            .map(|(w_idx, w)| {
                let (info, downloaded) = match w.current_segment {
                    Some(seg_idx) => (format!("Segment {}/{}", seg_idx + 1, segment_count), w.segment_downloaded),
                    None => ("Idle".to_string(), 0),
                };
                DownloadSegmentProgress {
                    start: w_idx as u64,
                    end: segment_count as u64,
                    downloaded,
                    total: 0,
                    status: info,
                }
            })
            .collect()
    }
}

pub struct HlsDownloadExecutor {
    client: Arc<std::sync::RwLock<reqwest::Client>>,
    ffmpeg: Option<FfmpegTools>,
}

impl HlsDownloadExecutor {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client: Arc::new(std::sync::RwLock::new(client)),
            ffmpeg: FfmpegTools::new(),
        }
    }

    pub fn new_shared(client: Arc<std::sync::RwLock<reqwest::Client>>) -> Self {
        Self {
            client,
            ffmpeg: FfmpegTools::new(),
        }
    }

    pub fn client(&self) -> reqwest::Client {
        self.client.read().unwrap().clone()
    }

    pub async fn transfer(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
        mut control: DownloadControl,
        rate_limiter: Arc<DownloadRateLimiter>,
    ) -> AppResult<DownloadResult> {
        let directory = PathBuf::from(
            download
                .directory
                .as_deref()
                .ok_or_else(|| AppError::download("No download directory was specified."))?,
        );
        tokio::fs::create_dir_all(&directory)
            .await
            .map_err(|e| AppError::download(format!("Could not create download folder: {e}")))?;

        let mut headers = HashMap::new();
        if let Some(ref referer) = download.referer {
            headers.insert("Referer".to_string(), referer.clone());
        }
        let plan = self
            .resolve_plan(&download.url, download.audio_url.as_deref(), &headers, 0)
            .await?;

        let parts_dir = directory.join(format!(".open-download-manager-hls-{}", download.id));
        tokio::fs::create_dir_all(&parts_dir)
            .await
            .map_err(|e| AppError::download(format!("Could not create parts directory: {e}")))?;

        let segment_count = plan.urls.len();
        let segments_state: Vec<StreamSegment> = {
            let mut state = Vec::with_capacity(segment_count);
            for i in 0..segment_count {
                let part_path = parts_dir.join(format!("{i}.part"));
                let (done, downloaded) = match tokio::fs::metadata(&part_path).await {
                    Ok(m) => (true, m.len()),
                    Err(_) => (false, 0),
                };
                state.push(StreamSegment { done, downloaded });
            }
            state
        };

        let initial_downloaded: u64 = segments_state.iter().map(|s| s.downloaded).sum();
        let worker_count = (download.connections as usize).clamp(1, segment_count);
        let state = Arc::new(Mutex::new(HlsTransferState {
            segments: segments_state,
            meter: ThroughputMeter::with_initial_bytes(initial_downloaded),
            worker_progress: (0..worker_count)
                .map(|_| WorkerProgressState {
                    current_segment: None,
                    segment_downloaded: 0,
                })
                .collect(),
        }));
        let next_index = Arc::new(Mutex::new(0_usize));
        let referer = download.referer.clone();

        let mut workers = tokio::task::JoinSet::new();
        let segments_plan = plan.segments.clone();
        for worker_id in 0..worker_count {
            let client = self.client();
            let segments = segments_plan.clone();
            let state = state.clone();
            let next_index = next_index.clone();
            let parts_dir = parts_dir.clone();
            let mut control = control.clone();
            let rate_limiter = rate_limiter.clone();
            let progress_handler = progress_handler.clone();
            let referer = referer.clone();

            workers.spawn(async move {
                loop {
                    if let Some(reason) = control.current_stop_reason() {
                        return Err(stopped_error(reason));
                    }
                    let index = {
                        let mut idx_lock = next_index.lock().unwrap();
                        let i = *idx_lock;
                        if i >= segments.len() {
                            break;
                        }
                        *idx_lock += 1;
                        i
                    };

                    let is_already_done = {
                        let st = state.lock().unwrap();
                        st.segments[index].done
                    };
                    if is_already_done {
                        continue;
                    }

                    download_hls_segment(
                        &client,
                        &segments[index],
                        index,
                        worker_id,
                        segment_count,
                        &parts_dir,
                        &state,
                        &mut control,
                        rate_limiter.clone(),
                        &progress_handler,
                        referer.as_deref(),
                    )
                    .await?;
                }
                if let Ok(mut st) = state.lock() {
                    st.worker_progress[worker_id].current_segment = None;
                }
                Ok(())
            });
        }

        while let Some(res) = workers.join_next().await {
            match res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    workers.abort_all();
                    while workers.join_next().await.is_some() {}
                    return Err(e);
                }
                Err(e) => {
                    workers.abort_all();
                    while workers.join_next().await.is_some() {}
                    return Err(AppError::download(format!("HLS segment task failed: {e}")));
                }
            }
        }

        // All segments downloaded, now assemble
        let destination_filename = if download.filename.ends_with(plan.extension) {
            download.filename.clone()
        } else {
            let base_name = download
                .filename
                .strip_suffix(".m3u8")
                .or_else(|| {
                    if plan.extension == ".mp4" {
                        download.filename.strip_suffix(".ts")
                    } else {
                        None
                    }
                })
                .unwrap_or(&download.filename);
            format!("{base_name}{}", plan.extension)
        };
        let destination = directory.join(destination_filename);

        if plan.has_separate_audio {
            let ffmpeg = self
                .ffmpeg
                .as_ref()
                .ok_or_else(|| AppError::download("FFmpeg is required to mux audio and video."))?;

            let video_track = parts_dir.join("video.track");
            let audio_track = parts_dir.join("audio.track");

            concat_files(&parts_dir, 0, plan.video_count, &video_track).await?;
            concat_files(&parts_dir, plan.video_count, segment_count, &audio_track).await?;

            ffmpeg
                .mux_tracks(&video_track, &audio_track, &destination, &mut control)
                .await?;
        } else {
            concat_files(&parts_dir, 0, segment_count, &destination).await?;
        }

        let total_size = tokio::fs::metadata(&destination)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        let _ = tokio::fs::remove_dir_all(&parts_dir).await;

        Ok(DownloadResult {
            downloaded: total_size,
            total: Some(total_size),
            path: destination,
        })
    }

    pub fn discard_partial(&self, download: &DownloadRecord) -> AppResult<()> {
        if let Some(dir) = &download.directory {
            let parts_dir = PathBuf::from(dir).join(format!(".open-download-manager-hls-{}", download.id));
            let _ = std::fs::remove_dir_all(parts_dir);
        }
        Ok(())
    }
}
