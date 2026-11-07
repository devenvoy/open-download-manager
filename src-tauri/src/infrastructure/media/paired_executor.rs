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
    domain::{
        download::{DownloadRecord, DownloadSegmentProgress},
        media::youtube_url,
    },
    infrastructure::media::{ffmpeg::FfmpegTools, ytdlp::YtDlpTools},
};

pub struct PairedDownloadExecutor {
    client: Arc<std::sync::RwLock<reqwest::Client>>,
    ffmpeg: Option<FfmpegTools>,
    ytdlp: Option<YtDlpTools>,
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
            client: Arc::new(std::sync::RwLock::new(client)),
            ffmpeg: FfmpegTools::new(),
            ytdlp: YtDlpTools::new(),
        }
    }

    pub fn new_shared(client: Arc<std::sync::RwLock<reqwest::Client>>) -> Self {
        Self {
            client,
            ffmpeg: FfmpegTools::new(),
            ytdlp: YtDlpTools::new(),
        }
    }

    pub fn client(&self) -> reqwest::Client {
        self.client.read().unwrap().clone()
    }

    pub async fn transfer(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
        control: DownloadControl,
        rate_limiter: Arc<DownloadRateLimiter>,
    ) -> AppResult<DownloadResult> {
        let audio_url = download.audio_url.clone().ok_or_else(|| {
            AppError::download("Paired download requires both video and audio URLs.")
        })?;
        let video_url = download.url.clone();

        match self
            .native_paired(
                &download,
                &video_url,
                &audio_url,
                progress_handler.clone(),
                control.clone(),
                rate_limiter.clone(),
            )
            .await
        {
            Ok(res) => Ok(res),
            Err(err) => {
                let canonical_url = download
                    .referer
                    .as_deref()
                    .and_then(youtube_url)
                    .or_else(|| youtube_url(&video_url))
                    .or_else(|| {
                        if let Ok(parsed) = url::Url::parse(&video_url) {
                            if let Some((_, docid)) = parsed.query_pairs().find(|(k, _)| k == "docid" || k == "id") {
                                let s = docid.to_string();
                                if s.len() == 11 {
                                    return Some(format!("https://www.youtube.com/watch?v={s}"));
                                }
                            }
                        }
                        None
                    });

                if let (Some(ytdlp), Some(target_url)) = (
                    self.ytdlp.as_ref().cloned().or_else(YtDlpTools::new),
                    canonical_url,
                ) {
                    let directory = download
                        .directory
                        .as_ref()
                        .map(PathBuf::from)
                        .ok_or_else(|| AppError::download("Download directory is not set."))?;

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
                    let speed_limit = rate_limiter.limit_kbps() as u64;

                    let format_spec = download.youtube_format.clone().or_else(|| {
                        let v_itag = url::Url::parse(&video_url).ok().and_then(|u| {
                            u.query_pairs().find(|(k, _)| k == "itag").map(|(_, v)| v.to_string())
                        });
                        let a_itag = url::Url::parse(&audio_url).ok().and_then(|u| {
                            u.query_pairs().find(|(k, _)| k == "itag").map(|(_, v)| v.to_string())
                        });
                        match (v_itag, a_itag) {
                            (Some(v), Some(a)) => Some(format!("{v}+{a}")),
                            (Some(v), None) => Some(v),
                            _ => None,
                        }
                    });

                    return ytdlp
                        .download(
                            &target_url,
                            format_spec.as_deref(),
                            download.referer.as_deref(),
                            &destination,
                            progress_handler,
                            control,
                            speed_limit,
                        )
                        .await;
                }
                Err(err)
            }
        }
    }

    async fn native_paired(
        &self,
        download: &DownloadRecord,
        video_url: &str,
        audio_url: &str,
        progress_handler: DownloadProgressHandler,
        mut control: DownloadControl,
        rate_limiter: Arc<DownloadRateLimiter>,
    ) -> AppResult<DownloadResult> {

        let directory = download
            .directory
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| AppError::download("Download directory is not set."))?;

        let parts_dir = directory.join(format!(".open-download-manager-paired-{}", download.id));
        tokio::fs::create_dir_all(&parts_dir)
            .await
            .map_err(|e| AppError::download(format!("Could not create parts directory: {e}")))?;

        let urls = [video_url.to_string(), audio_url.to_string()];

        // Probe both URLs to discover Content-Length if available
        let mut totals: [Option<u64>; 2] = [None, None];
        for (i, url) in urls.iter().enumerate() {
            if let Ok(parsed) = url::Url::parse(url) {
                if let Some((_, clen)) = parsed.query_pairs().find(|(k, _)| k == "clen") {
                    if let Ok(len) = clen.parse::<u64>() {
                        totals[i] = Some(len);
                    }
                }
            }
            if totals[i].is_none() {
                if let Ok(resp) = self.client().head(url.clone()).send().await {
                    if resp.status().is_success() {
                        totals[i] = resp.content_length();
                    }
                }
            }
        }

        let initial_total = match (totals[0], totals[1]) {
            (Some(v), Some(a)) => Some(v + a),
            (Some(v), None) => Some(v),
            (None, Some(a)) => Some(a),
            _ => None,
        };
        progress_handler(DownloadProgress {
            downloaded: 0,
            total: initial_total,
            bytes_per_second: 0,
            connections_progress: vec![
                DownloadSegmentProgress {
                    start: 0,
                    end: totals[0].unwrap_or(0),
                    downloaded: 0,
                    total: totals[0].unwrap_or(0),
                    status: "connecting".to_string(),
                },
                DownloadSegmentProgress {
                    start: 0,
                    end: totals[1].unwrap_or(0),
                    downloaded: 0,
                    total: totals[1].unwrap_or(0),
                    status: "connecting".to_string(),
                },
            ],
            notice: None,
        });

        let state = Arc::new(Mutex::new(PairedState {
            downloaded: [0, 0],
            totals,
            last_tick_bytes: 0,
            last_tick_time: Instant::now(),
            speed: 0,
        }));

        let mut workers = tokio::task::JoinSet::new();

        for (index, url) in urls.into_iter().enumerate() {
            let client = self.client();
            let parts_dir = parts_dir.clone();
            let mut control = control.clone();
            let rate_limiter = rate_limiter.clone();
            let progress_handler = progress_handler.clone();
            let state = state.clone();

            workers.spawn(async move {
                let temp_part = parts_dir.join(format!("{index}.receiving"));
                let final_part = parts_dir.join(format!("{index}.part"));

                let mut response = client
                    .get(url)
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
                            notice: None,
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

        // Validate stream completeness before invoking FFmpeg
        let truncation_error = {
            let st = state.lock().unwrap();
            let track_names = ["video", "audio"];
            let mut err = None;
            for (i, name) in track_names.iter().enumerate() {
                if let Some(expected) = st.totals[i] {
                    let actual = st.downloaded[i];
                    if actual < expected {
                        err = Some(format!(
                            "The {name} stream was interrupted prematurely (received {actual} of {expected} bytes). The download cannot be assembled."
                        ));
                        break;
                    }
                }
            }
            err
        };

        if let Some(msg) = truncation_error {
            let _ = tokio::fs::remove_dir_all(&parts_dir).await;
            return Err(AppError::download(msg));
        }

        let video_part = parts_dir.join("0.part");
        let audio_part = parts_dir.join("1.part");

        let is_mp3 = download.filename.to_ascii_lowercase().ends_with(".mp3");
        let video_len = tokio::fs::metadata(&video_part).await.map(|m| m.len()).unwrap_or(0);
        let audio_len = tokio::fs::metadata(&audio_part).await.map(|m| m.len()).unwrap_or(0);

        if !is_mp3 && video_len == 0 {
            let _ = tokio::fs::remove_dir_all(&parts_dir).await;
            return Err(AppError::download("Downloaded video track is empty. The stream could not be assembled."));
        }
        if audio_len == 0 {
            let _ = tokio::fs::remove_dir_all(&parts_dir).await;
            return Err(AppError::download("Downloaded audio track is empty. The stream could not be assembled."));
        }

        // Both video and audio parts downloaded successfully, now mux or convert
        let ffmpeg = self.ffmpeg.as_ref().cloned().or_else(FfmpegTools::new).ok_or_else(|| {
            AppError::download("FFmpeg is required to combine adaptive video and audio tracks.")
        })?;
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
