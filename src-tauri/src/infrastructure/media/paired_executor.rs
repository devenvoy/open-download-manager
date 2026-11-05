use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

use crate::{
    application::{
        download_executor::{stopped_error, DownloadControl, DownloadProgressHandler},
        download_progress::DownloadProgress,
        download_rate_limiter::DownloadRateLimiter,
        download_result::DownloadResult,
        error::{AppError, AppResult},
    },
    domain::download::{DownloadRecord, DownloadSegmentProgress},
    infrastructure::media::ffmpeg::FfmpegTools,
};

pub struct PairedDownloadExecutor {
    client: reqwest::Client,
    ffmpeg: Option<FfmpegTools>,
}

struct PairedState {
    downloaded: [u64; 2],
    totals: [Option<u64>; 2],
    last_tick_bytes: u64,
    last_tick_time: Instant,
    speed: u64,
}

impl PairedDownloadExecutor {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            ffmpeg: FfmpegTools::new(),
        }
    }

    pub async fn transfer(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
        mut control: DownloadControl,
        rate_limiter: Arc<DownloadRateLimiter>,
    ) -> AppResult<DownloadResult> {
        let audio_url = download.audio_url.clone().ok_or_else(|| {
            AppError::download("Paired download requires both video and audio URLs.")
        })?;
        let video_url = download.url.clone();

        let directory = download
            .directory
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| AppError::download("Download directory is not set."))?;

        let parts_dir = directory.join(format!(".open-download-manager-paired-{}", download.id));
        tokio::fs::create_dir_all(&parts_dir)
            .await
            .map_err(|e| AppError::download(format!("Could not create parts directory: {e}")))?;

        let urls = [video_url, audio_url];

        // Probe both URLs to discover Content-Length if available
        let mut totals: [Option<u64>; 2] = [None, None];
        for (i, url) in urls.iter().enumerate() {
            if let Ok(resp) = self.client.head(url).send().await {
                if resp.status().is_success() {
                    totals[i] = resp.content_length();
                }
            }
        }

        let state = Arc::new(Mutex::new(PairedState {
            downloaded: [0, 0],
            totals,
            last_tick_bytes: 0,
            last_tick_time: Instant::now(),
            speed: 0,
        }));

        let mut workers = tokio::task::JoinSet::new();

        for (index, url) in urls.into_iter().enumerate() {
            let client = self.client.clone();
            let parts_dir = parts_dir.clone();
            let mut control = control.clone();
            let rate_limiter = rate_limiter.clone();
            let progress_handler = progress_handler.clone();
            let state = state.clone();

            workers.spawn(async move {
                let temp_part = parts_dir.join(format!("{index}.receiving"));
                let final_part = parts_dir.join(format!("{index}.part"));

                let mut response = client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| AppError::download(format!("Could not connect to track {index}: {e}")))?;

                if !response.status().is_success() {
                    return Err(AppError::download(format!(
                        "Track {index} returned HTTP {}.",
                        response.status().as_u16()
                    )));
                }

                if let Some(cl) = response.content_length() {
                    let mut st = state.lock().unwrap();
                    st.totals[index] = Some(cl);
                }

                let mut file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&temp_part)
                    .await
                    .map_err(|e| AppError::download(format!("Could not create track file: {e}")))?;

                loop {
                    let chunk = tokio::select! {
                        reason = control.stopped() => {
                            let _ = tokio::fs::remove_file(&temp_part).await;
                            return Err(stopped_error(reason));
                        }
                        res = response.chunk() => res.map_err(|e| AppError::download(format!("Could not read track chunk: {e}")))?,
                    };

                    let Some(chunk) = chunk else {
                        break;
                    };

                    let chunk_size = chunk.len() as u64;
                    rate_limiter.throttle(chunk_size).await;
                    file.write_all(&chunk)
                        .await
                        .map_err(|e| AppError::download(format!("Could not write track: {e}")))?;

                    {
                        let mut st = state.lock().unwrap();
                        st.downloaded[index] += chunk_size;
                        let now = Instant::now();
                        let elapsed = now.duration_since(st.last_tick_time).as_millis();
                        if elapsed >= 250 {
                            let total_now = st.downloaded[0] + st.downloaded[1];
                            let diff = total_now.saturating_sub(st.last_tick_bytes);
                            st.speed = (diff * 1000) / elapsed.max(1) as u64;
                            st.last_tick_bytes = total_now;
                            st.last_tick_time = now;
                        }

                        let total_downloaded = st.downloaded[0] + st.downloaded[1];
                        let total = match (st.totals[0], st.totals[1]) {
                            (Some(v), Some(a)) => Some(v + a),
                            _ => None,
                        };
                        let speed = st.speed;
                        let segs = vec![
                            DownloadSegmentProgress {
                                start: 0,
                                end: st.totals[0].unwrap_or(0),
                                downloaded: st.downloaded[0],
                                total: st.totals[0].unwrap_or(0),
                                status: "downloading".to_string(),
                            },
                            DownloadSegmentProgress {
                                start: 0,
                                end: st.totals[1].unwrap_or(0),
                                downloaded: st.downloaded[1],
                                total: st.totals[1].unwrap_or(0),
                                status: "downloading".to_string(),
                            },
                        ];

                        progress_handler(DownloadProgress {
                            downloaded: total_downloaded,
                            total,
                            bytes_per_second: speed,
                            connections_progress: segs,
                        });
                    }
                }

                file.flush()
                    .await
                    .map_err(|e| AppError::download(format!("Could not flush track: {e}")))?;
                drop(file);

                tokio::fs::rename(&temp_part, &final_part)
                    .await
                    .map_err(|e| AppError::download(format!("Could not save track part: {e}")))?;

                Ok(())
            });
        }

        while let Some(res) = workers.join_next().await {
            match res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    let _ = tokio::fs::remove_dir_all(&parts_dir).await;
                    return Err(e);
                }
                Err(e) => {
                    let _ = tokio::fs::remove_dir_all(&parts_dir).await;
                    return Err(AppError::download(format!("Track task failed: {e}")));
                }
            }
        }

        // Both video and audio parts downloaded successfully, now mux or convert
        let ffmpeg = self.ffmpeg.as_ref().ok_or_else(|| {
            AppError::download("FFmpeg is required to combine adaptive video and audio tracks.")
        })?;

        let video_part = parts_dir.join("0.part");
        let audio_part = parts_dir.join("1.part");

        let is_mp3 = download.filename.to_ascii_lowercase().ends_with(".mp3");
        let destination_filename = if is_mp3 {
            download.filename.clone()
        } else if download.filename.to_ascii_lowercase().ends_with(".mp4") {
            download.filename.clone()
        } else {
            let base = Path::new(&download.filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&download.filename);
            format!("{base}.mp4")
        };

        let destination = directory.join(destination_filename);

        if is_mp3 {
            ffmpeg
                .convert_media(&audio_part, &destination, "mp3", &mut control)
                .await?;
        } else {
            ffmpeg
                .mux_tracks(&video_part, &audio_part, &destination, &mut control)
                .await?;
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
            let parts_dir = PathBuf::from(dir).join(format!(".open-download-manager-paired-{}", download.id));
            let _ = std::fs::remove_dir_all(parts_dir);
        }
        Ok(())
    }
}
