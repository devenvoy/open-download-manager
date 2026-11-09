// Created: 2026-11-05
// Reason: feat: add media handling for HLS, FFmpeg, yt-dlp, and stream progress
// Last updated: 2026-11-10
// Reason: refactor: modularize HLS executor into plan, segment, and coordinator modules
// Description: HLS master and media playlist parsing, variant resolution, and stream execution planning.

use std::collections::HashMap;

use crate::{
    application::error::{AppError, AppResult},
    domain::media::{parse_hls, scoped_headers, HlsMediaPlaylist, ParsedHls},
};

use super::HlsDownloadExecutor;

#[derive(Clone)]
pub struct ResolvedHlsPlan {
    pub urls: Vec<String>,
    pub segments: Vec<crate::domain::media::HlsSegment>,
    pub video_count: usize,
    pub has_separate_audio: bool,
    pub duration: f64,
    pub extension: &'static str,
}

impl HlsDownloadExecutor {
    pub async fn resolve_plan(
        &self,
        url: &str,
        audio_url: Option<&str>,
        headers: &HashMap<String, String>,
        depth: usize,
    ) -> AppResult<ResolvedHlsPlan> {
        if depth > 3 {
            return Err(AppError::download("Too many nested HLS playlists."));
        }

        let mut req = self.client().get(url);
        for (k, v) in headers {
            if let Ok(name) = reqwest::header::HeaderName::from_bytes(k.as_bytes()) {
                if let Ok(val) = reqwest::header::HeaderValue::from_str(v) {
                    req = req.header(name, val);
                }
            }
        }
        let resp = req
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

                let target_audio = chosen.audio_url.as_deref().or(audio_url);
                if let Some(target_audio_url) = target_audio {
                    let audio_headers = scoped_headers(headers, url, target_audio_url);
                    let audio = Box::pin(self.resolve_media_playlist(target_audio_url, &audio_headers)).await?;
                    let mut combined_urls = video.urls;
                    combined_urls.extend(audio.urls);
                    let mut combined_segments = video.segments;
                    combined_segments.extend(audio.segments);
                    Ok(ResolvedHlsPlan {
                        urls: combined_urls,
                        segments: combined_segments,
                        video_count,
                        has_separate_audio: true,
                        duration: video.duration,
                        extension: ".mp4",
                    })
                } else {
                    Ok(ResolvedHlsPlan {
                        urls: video.urls,
                        segments: video.segments,
                        video_count,
                        has_separate_audio: false,
                        duration: video.duration,
                        extension: video.extension,
                    })
                }
            }
            ParsedHls::Media(media) => {
                let video_count = media.urls.len();
                if let Some(target_audio_url) = audio_url {
                    let audio_headers = scoped_headers(headers, url, target_audio_url);
                    let audio = Box::pin(self.resolve_media_playlist(target_audio_url, &audio_headers)).await?;
                    let mut combined_urls = media.urls;
                    combined_urls.extend(audio.urls);
                    let mut combined_segments = media.segments;
                    combined_segments.extend(audio.segments);
                    Ok(ResolvedHlsPlan {
                        urls: combined_urls,
                        segments: combined_segments,
                        video_count,
                        has_separate_audio: true,
                        duration: media.duration,
                        extension: ".mp4",
                    })
                } else {
                    Ok(ResolvedHlsPlan {
                        urls: media.urls,
                        segments: media.segments,
                        video_count,
                        has_separate_audio: false,
                        duration: media.duration,
                        extension: media.extension,
                    })
                }
            }
        }
    }

    pub(crate) async fn resolve_media_playlist(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
    ) -> AppResult<HlsMediaPlaylist> {
        let mut req = self.client().get(url);
        for (k, v) in headers {
            if let Ok(name) = reqwest::header::HeaderName::from_bytes(k.as_bytes()) {
                if let Ok(val) = reqwest::header::HeaderValue::from_str(v) {
                    req = req.header(name, val);
                }
            }
        }
        let resp = req
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
            ParsedHls::Master(master) => {
                let chosen = master
                    .variants
                    .first()
                    .ok_or_else(|| AppError::download("No audio variants found in master playlist."))?;
                Box::pin(self.resolve_media_playlist(&chosen.url, headers)).await
            }
        }
    }
}
