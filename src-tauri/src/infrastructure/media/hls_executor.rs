use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::{fs::OpenOptions, io::{AsyncReadExt, AsyncWriteExt}};

use crate::{
    application::{
        download_executor::{stopped_error, DownloadControl, DownloadProgressHandler},
        download_progress::DownloadProgress,
        download_rate_limiter::DownloadRateLimiter,
        download_result::DownloadResult,
        error::{AppError, AppResult},
        throughput_meter::ThroughputMeter,
    },
    domain::{
        download::{DownloadRecord, DownloadSegmentProgress},
        media::{
            calculate_stream_progress, parse_hls, scoped_headers, HlsMediaPlaylist,
            ParsedHls, StreamSegment,
        },
    },
    infrastructure::media::ffmpeg::FfmpegTools,
};

struct WorkerProgressState {
    current_segment: Option<usize>,
    segment_downloaded: u64,
}

struct HlsTransferState {
    segments: Vec<StreamSegment>,
    meter: ThroughputMeter,
    worker_progress: Vec<WorkerProgressState>,
}

impl HlsTransferState {
    fn build_connections_progress(&self, segment_count: usize) -> Vec<DownloadSegmentProgress> {
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

pub struct ResolvedHlsPlan {
    pub urls: Vec<String>,
    pub video_count: usize,
    pub has_separate_audio: bool,
    pub duration: f64,
    pub extension: &'static str,
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

    pub async fn resolve_plan(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        depth: usize,
    ) -> AppResult<ResolvedHlsPlan> {
        if depth > 3 {
            return Err(AppError::download("Too many nested HLS playlists."));
        }

        let resp = self
            .client()
            .get(url)
            .send()
            .await
            .map_err(|e| AppError::download(format!("Could not fetch HLS playlist: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::download(format!(
                "HLS playlist returned HTTP {}.",
                resp.status().as_u16()
            )));
        }

        let final_url = resp.url().to_string();
        let text = resp
            .text()
            .await
            .map_err(|e| AppError::download(format!("Could not read HLS playlist: {e}")))?;

        match parse_hls(&text, &final_url).map_err(AppError::download)? {
            ParsedHls::Master(master) => {
                let chosen = master
                    .variants
                    .first()
                    .ok_or_else(|| AppError::download("No playable HLS variants found."))?;

                let video = Box::pin(self.resolve_media_playlist(&chosen.url, headers)).await?;
                let video_count = video.urls.len();

                if let Some(audio_url) = &chosen.audio_url {
                    let audio_headers = scoped_headers(headers, url, audio_url);
                    let audio = Box::pin(self.resolve_media_playlist(audio_url, &audio_headers)).await?;
                    let mut combined_urls = video.urls;
                    combined_urls.extend(audio.urls);
                    Ok(ResolvedHlsPlan {
                        urls: combined_urls,
                        video_count,
                        has_separate_audio: true,
                        duration: video.duration,
                        extension: ".mp4",
                    })
                } else {
                    Ok(ResolvedHlsPlan {
                        urls: video.urls,
                        video_count,
                        has_separate_audio: false,
                        duration: video.duration,
                        extension: video.extension,
                    })
                }
            }
            ParsedHls::Media(media) => {
                let count = media.urls.len();
                Ok(ResolvedHlsPlan {
                    urls: media.urls,
                    video_count: count,
                    has_separate_audio: false,
                    duration: media.duration,
                    extension: media.extension,
                })
            }
        }
    }

    async fn resolve_media_playlist(
        &self,
        url: &str,
        _headers: &HashMap<String, String>,
    ) -> AppResult<HlsMediaPlaylist> {
        let resp = self
            .client()
            .get(url)
            .send()
            .await
            .map_err(|e| AppError::download(format!("Could not fetch HLS media playlist: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::download(format!(
                "HLS media playlist returned HTTP {}.",
                resp.status().as_u16()
            )));
        }

        let final_url = resp.url().to_string();
        let text = resp
            .text()
            .await
            .map_err(|e| AppError::download(format!("Could not read HLS media playlist: {e}")))?;

        match parse_hls(&text, &final_url).map_err(AppError::download)? {
            ParsedHls::Media(media) => Ok(media),
            ParsedHls::Master(_) => Err(AppError::download("Nested master playlists are not supported.")),
        }
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
            .resolve_plan(&download.url, &headers, 0)
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
        for worker_id in 0..worker_count {
            let client = self.client();
            let urls = plan.urls.clone();
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
                        if i >= urls.len() {
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
                        &urls[index],
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
                    return Err(e);
                }
                Err(e) => {
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

async fn download_hls_segment(
    client: &reqwest::Client,
    url: &str,
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

    let mut req = client.get(url);
    if let Some(r) = referer {
        req = req.header(reqwest::header::REFERER, r);
    }
    let mut response = req
        .send()
        .await
        .map_err(|e| AppError::download(format!("Could not download segment {index}: {e}")))?;

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

        let chunk_size = chunk.len() as u64;
        rate_limiter.throttle(chunk_size).await;
        file.write_all(&chunk)
            .await
            .map_err(|e| AppError::download(format!("Could not write segment: {e}")))?;

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
                });
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
        let stream_prog = calculate_stream_progress(&st.segments);
        let speed = st.meter.current_throughput();
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
        });
    }

    Ok(())
}

async fn concat_files(
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
