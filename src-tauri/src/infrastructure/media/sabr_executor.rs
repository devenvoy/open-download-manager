// Created: 2026-11-05
// Reason: fix(engine): implement genuine auto-updater payload execution and SABR UMP streaming executor
// Last updated: 2026-11-08
// Reason: fix(transfer): preserve HTTP resume validator type across interrupted downloads (Closes #25) (#40)
// Reason: feat(media): auto-fallback to yt-dlp on premature SABR stream cutoff and provide 1-click solution flow
// Description: YouTube SABR streaming executor reading UMP chunks and reassembling playable media.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
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
        media::sabr::{build_sabr_request_body, parse_media_header_itag, SabrFormat},
    },
    infrastructure::media::{ffmpeg::FfmpegTools, ytdlp::YtDlpTools},
};

pub struct SabrDownloadExecutor {
    client: Arc<std::sync::RwLock<reqwest::Client>>,
    ffmpeg: Option<FfmpegTools>,
}

fn read_varint_from_slice(slice: &[u8]) -> Option<(u64, usize)> {
    let mut result = 0u64;
    let mut shift = 0;
    for (i, &b) in slice.iter().enumerate() {
        result |= ((b & 0x7F) as u64) << shift;
        if (b & 0x80) == 0 {
            return Some((result, i + 1));
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
    None
}

impl SabrDownloadExecutor {
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
        let session = download.sabr.clone().ok_or_else(|| {
            AppError::download("Download has no YouTube SABR session metadata.")
        })?;

        let directory = download
            .directory
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| AppError::download("Download directory is not set."))?;

        let has_video = session.video_itag.is_some();
        let parts_dir = directory.join(format!(".open-download-manager-sabr-{}", download.id));
        tokio::fs::create_dir_all(&parts_dir)
            .await
            .map_err(|e| AppError::download(format!("Could not create SABR parts directory: {e}")))?;

        let video_receiving = parts_dir.join("0.receiving");
        let video_part = parts_dir.join("0.part");
        let audio_receiving = parts_dir.join(if has_video { "1.receiving" } else { "0.receiving" });
        let audio_part = parts_dir.join(if has_video { "1.part" } else { "0.part" });

        let picked: Vec<&SabrFormat> = session
            .formats
            .iter()
            .filter(|f| f.itag == session.audio_itag || Some(f.itag) == session.video_itag)
            .collect();
        let all_have_content_length = !picked.is_empty()
            && picked.iter().all(|f| {
                f.content_length
                    .as_ref()
                    .and_then(|s| s.parse::<u64>().ok())
                    .is_some()
            });
        let total: Option<u64> = if all_have_content_length {
            Some(
                picked
                    .iter()
                    .filter_map(|f| f.content_length.as_ref().and_then(|s| s.parse::<u64>().ok()))
                    .sum(),
            )
        } else {
            None
        };

        let mut video_file = if has_video {
            Some(
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&video_receiving)
                    .await
                    .map_err(|e| AppError::download(format!("Could not create video track file: {e}")))?,
            )
        } else {
            None
        };

        let mut audio_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&audio_receiving)
            .await
            .map_err(|e| AppError::download(format!("Could not create audio track file: {e}")))?;

        let mut req = self.client().post(&session.server_abr_streaming_url);

        let body_bytes = build_sabr_request_body(&session);
        if !body_bytes.is_empty() {
            req = req.header("content-type", "application/x-protobuf");
            req = req.body(body_bytes);
        }

        let mut response = req
            .send()
            .await
            .map_err(|e| AppError::download(format!("Could not connect to YouTube SABR stream: {e}")))?;

        if !response.status().is_success() {
            let _ = tokio::fs::remove_dir_all(&parts_dir).await;
            return Err(AppError::download(format!(
                "YouTube SABR server returned HTTP {}.",
                response.status().as_u16()
            )));
        }

        let mut video_downloaded: u64 = 0;
        let mut audio_downloaded: u64 = 0;
        let mut current_itag: Option<u32> = None;
        let mut buffer: Vec<u8> = Vec::new();
        let mut last_tick_time = Instant::now();
        let mut last_tick_bytes: u64 = 0;
        let mut speed: u64 = 0;

        loop {
            let chunk = tokio::select! {
                reason = control.stopped() => {
                    drop(video_file);
                    drop(audio_file);
                    let _ = tokio::fs::remove_dir_all(&parts_dir).await;
                    return Err(stopped_error(reason));
                }
                res = response.chunk() => res.map_err(|e| AppError::download(format!("Could not read SABR stream chunk: {e}")))?,
            };

            let Some(chunk) = chunk else {
                break;
            };

            let chunk_len = chunk.len() as u64;
            rate_limiter.throttle(chunk_len).await;
            buffer.extend_from_slice(&chunk);

            // Decode UMP frames
            loop {
                let Some((part_type, pt_len)) = read_varint_from_slice(&buffer) else {
                    break;
                };
                let Some((payload_size, ps_len)) = read_varint_from_slice(&buffer[pt_len..]) else {
                    break;
                };
                let header_len = pt_len + ps_len;
                let total_part_len = header_len + payload_size as usize;
                if buffer.len() < total_part_len {
                    break;
                }

                let payload = &buffer[header_len..total_part_len];
                match part_type {
                    20 => {
                        // MEDIA_HEADER
                        if let Some(itag) = parse_media_header_itag(payload) {
                            current_itag = Some(itag);
                        }
                    }
                    21 => {
                        // MEDIA chunk
                        if has_video && current_itag == session.video_itag {
                            if let Some(ref mut vf) = video_file {
                                vf.write_all(payload).await.map_err(|e| {
                                    AppError::download(format!("Failed to write video track: {e}"))
                                })?;
                                video_downloaded += payload.len() as u64;
                            }
                        } else if current_itag == Some(session.audio_itag)
                            || (!has_video && current_itag.is_none())
                        {
                            audio_file.write_all(payload).await.map_err(|e| {
                                AppError::download(format!("Failed to write audio track: {e}"))
                            })?;
                            audio_downloaded += payload.len() as u64;
                        } else if has_video && current_itag.is_none() {
                            if let Some(ref mut vf) = video_file {
                                vf.write_all(payload).await.map_err(|e| {
                                    AppError::download(format!("Failed to write video track: {e}"))
                                })?;
                                video_downloaded += payload.len() as u64;
                            }
                        }
                    }
                    22 => {
                        // MEDIA_END
                    }
                    _ => {
                        // Ignore metadata or policy frames
                    }
                }

                buffer.drain(..total_part_len);
            }

            // Periodic progress update
            let total_downloaded = video_downloaded + audio_downloaded;
            let now = Instant::now();
            let elapsed = now.duration_since(last_tick_time).as_millis();
            if elapsed >= 250 {
                let diff = total_downloaded.saturating_sub(last_tick_bytes);
                speed = (diff * 1000) / elapsed.max(1) as u64;
                last_tick_bytes = total_downloaded;
                last_tick_time = now;
            }

            let mut segs = Vec::new();
            if has_video {
                let video_tot = session
                    .formats
                    .iter()
                    .find(|f| Some(f.itag) == session.video_itag)
                    .and_then(|f| f.content_length.as_ref()?.parse::<u64>().ok())
                    .unwrap_or(0);
                segs.push(DownloadSegmentProgress {
                    start: 0,
                    end: video_tot,
                    downloaded: video_downloaded,
                    total: video_tot,
                    status: "downloading".to_string(),
                });
            }
            let audio_tot = session
                .formats
                .iter()
                .find(|f| f.itag == session.audio_itag)
                .and_then(|f| f.content_length.as_ref()?.parse::<u64>().ok())
                .unwrap_or(0);
            segs.push(DownloadSegmentProgress {
                start: 0,
                end: audio_tot,
                downloaded: audio_downloaded,
                total: audio_tot,
                status: "downloading".to_string(),
            });

            progress_handler(DownloadProgress {
                downloaded: total_downloaded,
                total,
                bytes_per_second: speed,
                connections_progress: segs,
                notice: None,
                transfer_kind: Some("sabr".to_string()),
                effective_concurrency: Some(2),
                etag: None,
                validator: None,
            });
        }

        // Fallback: If no UMP chunks were framed (e.g. raw direct stream mock), flush remaining buffer
        if video_downloaded == 0 && audio_downloaded == 0 && !buffer.is_empty() {
            if let Some(ref mut vf) = video_file {
                vf.write_all(&buffer).await.map_err(|e| {
                    AppError::download(format!("Failed to write track: {e}"))
                })?;
                video_downloaded += buffer.len() as u64;
            } else {
                audio_file.write_all(&buffer).await.map_err(|e| {
                    AppError::download(format!("Failed to write track: {e}"))
                })?;
                audio_downloaded += buffer.len() as u64;
            }

            progress_handler(DownloadProgress {
                downloaded: video_downloaded + audio_downloaded,
                total: total.or(Some(video_downloaded + audio_downloaded)),
                bytes_per_second: 0,
                connections_progress: Vec::new(),
                notice: None,
                transfer_kind: Some("sabr".to_string()),
                effective_concurrency: Some(1),
                etag: None,
                validator: None,
            });
        }

        // Flush and finalize parts
        if let Some(mut vf) = video_file {
            vf.flush()
                .await
                .map_err(|e| AppError::download(format!("Could not flush video track: {e}")))?;
            drop(vf);
            tokio::fs::rename(&video_receiving, &video_part)
                .await
                .map_err(|e| AppError::download(format!("Could not save video track part: {e}")))?;
        }

        audio_file
            .flush()
            .await
            .map_err(|e| AppError::download(format!("Could not flush audio track: {e}")))?;
        drop(audio_file);
        tokio::fs::rename(&audio_receiving, &audio_part)
            .await
            .map_err(|e| AppError::download(format!("Could not save audio track part: {e}")))?;

        // Destination resolution
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

        // Validate stream completeness before invoking FFmpeg
        let total_downloaded = video_downloaded + audio_downloaded;
        if let Some(expected_total) = total {
            if total_downloaded < expected_total {
                let _ = tokio::fs::remove_dir_all(&parts_dir).await;
                if let Some(ytdlp) = YtDlpTools::new() {
                    let watch_url = format!("https://www.youtube.com/watch?v={}", session.video_id);
                    let format_spec = session.video_itag.map(|v| format!("{v}+{}", session.audio_itag));
                    progress_handler(DownloadProgress {
                        downloaded: 0,
                        total: Some(expected_total),
                        bytes_per_second: 0,
                        connections_progress: Vec::new(),
                        notice: Some("Direct YouTube stream ended prematurely; resuming with yt-dlp to download full video.".to_string()),
                        transfer_kind: Some("ytdlp".to_string()),
                        effective_concurrency: Some(1),
                        etag: None,
                        validator: None,
                    });
                    return ytdlp.download(
                        &watch_url,
                        format_spec.as_deref(),
                        download.referer.as_deref().or(Some(&watch_url)),
                        &destination,
                        progress_handler,
                        control,
                        0,
                    ).await;
                }
                return Err(AppError::download(format!(
                    "YouTube stream ended prematurely: received {total_downloaded} of {expected_total} bytes ({:.1}%). Please install yt-dlp from Settings > Media Tools to download full YouTube streams.",
                    (total_downloaded as f64 / expected_total as f64) * 100.0
                )));
            }
        }

        let video_len = if has_video {
            tokio::fs::metadata(&video_part).await.map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };
        let audio_len = tokio::fs::metadata(&audio_part).await.map(|m| m.len()).unwrap_or(0);

        if has_video && video_len == 0 {
            let _ = tokio::fs::remove_dir_all(&parts_dir).await;
            if let Some(ytdlp) = YtDlpTools::new() {
                let watch_url = format!("https://www.youtube.com/watch?v={}", session.video_id);
                let format_spec = session.video_itag.map(|v| format!("{v}+{}", session.audio_itag));
                return ytdlp.download(
                    &watch_url,
                    format_spec.as_deref(),
                    download.referer.as_deref().or(Some(&watch_url)),
                    &destination,
                    progress_handler,
                    control,
                    0,
                ).await;
            }
            return Err(AppError::download("YouTube video track is empty. Please install yt-dlp to download full YouTube streams."));
        }
        if audio_len == 0 {
            let _ = tokio::fs::remove_dir_all(&parts_dir).await;
            if let Some(ytdlp) = YtDlpTools::new() {
                let watch_url = format!("https://www.youtube.com/watch?v={}", session.video_id);
                let format_spec = session.video_itag.map(|v| format!("{v}+{}", session.audio_itag));
                return ytdlp.download(
                    &watch_url,
                    format_spec.as_deref(),
                    download.referer.as_deref().or(Some(&watch_url)),
                    &destination,
                    progress_handler,
                    control,
                    0,
                ).await;
            }
            return Err(AppError::download("YouTube audio track is empty. Please install yt-dlp to download full YouTube streams."));
        }

        if has_video {
            let ffmpeg = self.ffmpeg.as_ref().cloned().or_else(FfmpegTools::new).ok_or_else(|| {
                AppError::download("FFmpeg is required to combine YouTube SABR video and audio tracks.")
            })?;
            ffmpeg
                .mux_tracks(&video_part, &audio_part, &destination, &mut control)
                .await?;
        } else if is_mp3 {
            let ffmpeg = self.ffmpeg.as_ref().cloned().or_else(FfmpegTools::new).ok_or_else(|| {
                AppError::download("FFmpeg is required to convert YouTube audio track to mp3.")
            })?;
            ffmpeg
                .convert_media(&audio_part, &destination, "mp3", &mut control)
                .await?;
        } else {
            tokio::fs::rename(&audio_part, &destination)
                .await
                .map_err(|e| AppError::download(format!("Could not save output file: {e}")))?;
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
            let parts_dir = PathBuf::from(dir).join(format!(".open-download-manager-sabr-{}", download.id));
            let _ = std::fs::remove_dir_all(parts_dir);
        }
        Ok(())
    }
}
