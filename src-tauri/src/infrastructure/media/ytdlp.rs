// Created: 2026-11-05
// Reason: feat: add media handling for HLS, FFmpeg, yt-dlp, and stream progress
// Last updated: 2026-11-08
// Reason: fix(transfer): preserve HTTP resume validator type across interrupted downloads (Closes #25) (#40)
// Description: yt-dlp command-line wrapper extracting stream formats, playlist items, and video metadata.

use std::{path::{Path, PathBuf}, process::Stdio, time::Duration};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt}, process::Command};
use url::Url;

use crate::{
    application::{
        download_executor::{stopped_error, DownloadControl, DownloadProgressHandler},
        download_progress::DownloadProgress,
        download_result::DownloadResult,
        error::{AppError, AppResult},
    },
    domain::media::{
        choices_from_info, parse_playlist_json, parse_youtube_playlist_url,
        site_choices_from_info, youtube_error, youtube_url, PlaylistMetadata,
        YouTubeFormatChoice,
    },
};

#[derive(Clone)]
pub struct YtDlpTools {
    executable: PathBuf,
}

impl YtDlpTools {
    pub fn new() -> Option<Self> {
        let path = find_ytdlp_path()?;
        Some(Self { executable: path })
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub async fn fetch_formats(&self, input_url: &str) -> AppResult<Vec<YouTubeFormatChoice>> {
        let canonical_url = youtube_url(input_url)
            .ok_or_else(|| AppError::download("A valid YouTube video URL is required."))?;
        self.fetch_site_formats_from_url(&canonical_url, true).await
    }

    pub async fn fetch_site_formats(&self, input_url: &str) -> AppResult<Vec<YouTubeFormatChoice>> {
        let parsed = Url::parse(input_url)
            .map_err(|_| AppError::download("A valid HTTP(S) video page is required."))?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(AppError::download("A valid HTTP(S) video page is required."));
        }

        // Fast-path native HLS format sniffer (< 10 ms)
        if input_url.contains(".m3u8") {
            if let Ok(choices) = Self::sniff_hls_formats(input_url).await {
                if !choices.is_empty() {
                    return Ok(choices);
                }
            }
        }

        let is_yt = youtube_url(input_url).is_some();
        self.fetch_site_formats_from_url(parsed.as_str(), is_yt).await
    }

    pub fn format_choices_from_parsed_hls(
        parsed: crate::domain::media::ParsedHls,
        original_url: &str,
    ) -> Vec<YouTubeFormatChoice> {
        match parsed {
            crate::domain::media::ParsedHls::Master(master) => {
                let mut choices = Vec::new();
                for (i, variant) in master.variants.into_iter().enumerate() {
                    let quality = variant.resolution.clone().unwrap_or_else(|| {
                        if variant.bandwidth > 0 {
                            format!("{} kbps", variant.bandwidth / 1000)
                        } else {
                            "Auto".to_string()
                        }
                    });
                    let audio_state = if variant.audio_url.is_some() {
                        "Separate audio track".to_string()
                    } else {
                        "Combined audio/video".to_string()
                    };
                    choices.push(YouTubeFormatChoice {
                        url: variant.url,
                        provider: "hls".to_string(),
                        youtube_format: format!("hls-{}", variant.bandwidth),
                        format: "mp4".to_string(),
                        quality,
                        fps: 30,
                        audio_state,
                        title: format!("HLS Stream (Stream #{})", i + 1),
                        output_format: "mp4".to_string(),
                        estimated_size: 0,
                        duration: 0.0,
                        audio_url: variant.audio_url,
                    });
                }
                choices
            }
            crate::domain::media::ParsedHls::Media(media) => {
                vec![YouTubeFormatChoice {
                    url: original_url.to_string(),
                    provider: "hls".to_string(),
                    youtube_format: "hls-direct".to_string(),
                    format: media.extension.trim_start_matches('.').to_string(),
                    quality: "Original".to_string(),
                    fps: 30,
                    audio_state: "Combined audio/video".to_string(),
                    title: "HLS Media Stream".to_string(),
                    output_format: "mp4".to_string(),
                    estimated_size: 0,
                    duration: media.duration,
                    audio_url: None,
                }]
            }
        }
    }

    pub async fn sniff_hls_formats(url: &str) -> AppResult<Vec<YouTubeFormatChoice>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| AppError::download(format!("Could not create HTTP client for HLS sniff: {e}")))?;

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| AppError::download(format!("Could not fetch HLS playlist: {e}")))?;

        if !response.status().is_success() {
            return Err(AppError::download(format!(
                "HLS playlist fetch returned status {}",
                response.status()
            )));
        }

        let text = response
            .text()
            .await
            .map_err(|e| AppError::download(format!("Could not read HLS playlist: {e}")))?;

        let parsed = crate::domain::media::parse_hls(&text, url)
            .map_err(AppError::download)?;

        Ok(Self::format_choices_from_parsed_hls(parsed, url))
    }

    async fn fetch_site_formats_from_url(
        &self,
        input_url: &str,
        is_youtube: bool,
    ) -> AppResult<Vec<YouTubeFormatChoice>> {
        let mut command = Command::new(&self.executable);
        command
            .arg("--skip-download")
            .arg("--dump-single-json")
            .arg("--no-playlist")
            .arg("--no-colors")
            .arg(input_url)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let mut child = command
            .spawn()
            .map_err(|e| AppError::download(format!("Could not spawn yt-dlp: {e}")))?;

        let mut stdout_handle = child
            .stdout
            .take()
            .ok_or_else(|| AppError::download("yt-dlp stdout unavailable"))?;
        let mut stderr_handle = child
            .stderr
            .take()
            .ok_or_else(|| AppError::download("yt-dlp stderr unavailable"))?;
        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();

        let timeout = Duration::from_secs(45);
        let status = tokio::select! {
            _ = tokio::time::sleep(timeout) => {
                let _ = child.kill().await;
                return Err(AppError::download("Video format discovery timed out."));
            }
            res = async {
                let (status, _, _) = tokio::try_join!(
                    child.wait(),
                    stdout_handle.read_to_end(&mut stdout_bytes),
                    stderr_handle.read_to_end(&mut stderr_bytes),
                )?;
                Ok::<_, std::io::Error>(status)
            } => res.map_err(|e| AppError::download(format!("yt-dlp error: {e}")))?,
        };

        if !status.success() {
            let stderr = String::from_utf8_lossy(&stderr_bytes);
            if is_youtube {
                let (message, _, _) = youtube_error(&stderr);
                return Err(AppError::download(message));
            }
            return Err(AppError::download(
                "Could not discover video formats on this page. It may require sign-in, use DRM, or expose an unsupported player.",
            ));
        }

        let json_value: serde_json::Value = serde_json::from_slice(&stdout_bytes)
            .map_err(|e| AppError::download(format!("Invalid video extractor response: {e}")))?;

        if is_youtube {
            choices_from_info(&json_value, input_url).map_err(AppError::download)
        } else {
            site_choices_from_info(&json_value, input_url).map_err(AppError::download)
        }
    }

    pub async fn fetch_playlist_info(&self, input_url: &str) -> AppResult<PlaylistMetadata> {
        let target = parse_youtube_playlist_url(input_url)
            .ok_or_else(|| AppError::download("A valid YouTube playlist URL is required."))?;

        let mut command = Command::new(&self.executable);
        command
            .arg("--skip-download")
            .arg("--flat-playlist")
            .arg("--dump-single-json")
            .arg("--no-colors")
            .arg("--playlist-end")
            .arg("500")
            .arg(&target.canonical_playlist_url)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let mut child = command
            .spawn()
            .map_err(|e| AppError::download(format!("Could not spawn yt-dlp: {e}")))?;

        let mut stdout_handle = child
            .stdout
            .take()
            .ok_or_else(|| AppError::download("yt-dlp stdout unavailable"))?;
        let mut stderr_handle = child
            .stderr
            .take()
            .ok_or_else(|| AppError::download("yt-dlp stderr unavailable"))?;
        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();

        let timeout = Duration::from_secs(60);
        let status = tokio::select! {
            _ = tokio::time::sleep(timeout) => {
                let _ = child.kill().await;
                return Err(AppError::download("YouTube playlist request timed out."));
            }
            res = async {
                let (status, _, _) = tokio::try_join!(
                    child.wait(),
                    stdout_handle.read_to_end(&mut stdout_bytes),
                    stderr_handle.read_to_end(&mut stderr_bytes),
                )?;
                Ok::<_, std::io::Error>(status)
            } => res.map_err(|e| AppError::download(format!("yt-dlp error: {e}")))?,
        };

        if !status.success() {
            let stderr = String::from_utf8_lossy(&stderr_bytes);
            let (msg, _, _) = youtube_error(&stderr);
            return Err(AppError::download(msg));
        }

        let json_value: serde_json::Value = serde_json::from_slice(&stdout_bytes)
            .map_err(|e| AppError::download(format!("Invalid YouTube JSON: {e}")))?;

        parse_playlist_json(&json_value, &target.canonical_playlist_url).map_err(AppError::download)
    }

    pub async fn download(
        &self,
        video_url: &str,
        format_spec: Option<&str>,
        referer: Option<&str>,
        destination: &Path,
        progress_handler: DownloadProgressHandler,
        mut control: DownloadControl,
        speed_limit_kbps: u64,
    ) -> AppResult<DownloadResult> {
        let mut command = Command::new(&self.executable);
        command
            .arg("--newline")
            .arg("--no-colors")
            .arg("--no-playlist")
            .arg("--concurrent-fragments")
            .arg("8")
            .arg("--progress")
            .arg("--progress-delta")
            .arg("0.5")
            .arg("--progress-template")
            .arg("download:ADPROGRESS:%(progress)j")
            .arg("--embed-metadata")
            .arg("--embed-chapters")
            .arg("--embed-subs")
            .arg("--write-subs")
            .arg("--sub-langs")
            .arg("all")
            .arg("--no-abort-on-error");

        if let Some(ffmpeg_path) = super::tools::find_tool_path("ffmpeg") {
            command.arg("--ffmpeg-location").arg(ffmpeg_path);
        }
        if let Some(referer) = referer {
            command.arg("--referer").arg(referer);
        }

        let output_format = destination
            .to_string_lossy()
            .to_ascii_lowercase()
            .rsplit_once('.')
            .map(|(_, extension)| extension.to_owned())
            .unwrap_or_default();

        let is_mp4 = output_format == "mp4" || output_format.is_empty();

        if let Some(spec) = format_spec {
            let clean = spec.trim().trim_start_matches("audio:");
            let resilient_spec = if clean.contains('+') {
                let parts: Vec<&str> = clean.splitn(2, '+').collect();
                let v = parts[0].trim();
                let a = parts[1].trim();
                if is_mp4 {
                    format!("{v}+{a}/{v}+ba[acodec^=mp4a]/{v}+ba/bv*[vcodec^=avc]+ba[acodec^=mp4a]/bv*+ba/best")
                } else {
                    format!("{v}+{a}/{v}+ba/{v}+bestaudio/bv*+ba/best")
                }
            } else if !clean.is_empty() {
                if is_mp4 {
                    format!("{clean}/ba[acodec^=mp4a]/ba/bestaudio/best")
                } else {
                    format!("{clean}/ba/bestaudio/best")
                }
            } else if is_mp4 {
                "bv*[vcodec^=avc]+ba[acodec^=mp4a]/bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/bv*+ba/best".to_string()
            } else {
                "bv*+ba/best".to_string()
            };
            command.arg("-f").arg(resilient_spec);
        } else if is_mp4 {
            command.arg("-f").arg("bv*[vcodec^=avc]+ba[acodec^=mp4a]/bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/bv*+ba/best");
        } else {
            command.arg("-f").arg("bv*+ba/best");
        }

        if is_mp4 {
            // Prioritize standard H.264/AAC for seamless playback on native media players (QuickTime / Windows Media)
            command.arg("--format-sort").arg("vcodec:h264,acodec:aac");
        }

        if output_format == "mp3" {
            command.arg("--extract-audio").arg("--audio-format").arg("mp3");
        } else if matches!(output_format.as_str(), "mp4" | "webm") {
            command.arg("--merge-output-format").arg(&output_format);
        } else {
            command.arg("--merge-output-format").arg("mp4");
        }

        if speed_limit_kbps > 0 {
            command.arg("--limit-rate").arg(format!("{speed_limit_kbps}K"));
        }

        command.arg("-o").arg(destination);
        command.arg(video_url);

        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let mut child = command
            .spawn()
            .map_err(|e| AppError::download(format!("Could not spawn yt-dlp: {e}")))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::download("yt-dlp stdout unavailable"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AppError::download("yt-dlp stderr unavailable"))?;

        let mut reader = tokio::io::BufReader::new(stdout).lines();
        let mut stderr_reader = tokio::io::BufReader::new(stderr).lines();
        let mut stderr_lines: Vec<String> = Vec::new();

        loop {
            tokio::select! {
                reason = control.stopped() => {
                    let _ = child.kill().await;
                    let _ = tokio::fs::remove_file(destination).await;
                    return Err(stopped_error(reason));
                }
                line = reader.next_line() => {
                    match line {
                        Ok(Some(l)) => {
                            let trimmed = l.trim();
                            if let Some(pos) = trimmed.find("ADPROGRESS:") {
                                let json_str = &trimmed[pos + "ADPROGRESS:".len()..];
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                                    let downloaded = val.get("downloaded_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let total = val.get("total_bytes").and_then(|v| v.as_u64())
                                        .or_else(|| val.get("total_bytes_estimate").and_then(|v| v.as_u64()));
                                    let speed = val.get("speed").and_then(|v| v.as_f64()).map(|s| s as u64).unwrap_or(0);

                                    progress_handler(DownloadProgress {
                                        downloaded,
                                        total,
                                        bytes_per_second: speed,
                                        connections_progress: Vec::new(),
                                        notice: None,
                                        transfer_kind: Some("ytdlp".to_string()),
                                        effective_concurrency: Some(8),
                                        etag: None,
                                        validator: None,
                                        filename: None,
                                        resumable: Some(true),
                                    });
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(_) => break,
                    }
                }
                err_line = stderr_reader.next_line() => {
                    if let Ok(Some(el)) = err_line {
                        if stderr_lines.len() < 50 {
                            stderr_lines.push(el);
                        }
                    }
                }
            }
        }

        let status = child
            .wait()
            .await
            .map_err(|e| AppError::download(format!("yt-dlp process error: {e}")))?;

        if !status.success() {
            let _ = tokio::fs::remove_file(destination).await;
            let combined_err = stderr_lines.join("\n");
            let msg = if youtube_url(video_url).is_some() {
                youtube_error(&combined_err).0
            } else {
                extractor_error(&combined_err)
            };
            return Err(AppError::download(msg));
        }

        let total_size = tokio::fs::metadata(destination)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        if total_size == 0 {
            return Err(AppError::download("Video download produced an empty file."));
        }

        Ok(DownloadResult {
            downloaded: total_size,
            total: Some(total_size),
            path: destination.to_path_buf(),
        })
    }
}

fn find_ytdlp_path() -> Option<PathBuf> {
    super::tools::find_tool_path("yt-dlp")
}

fn extractor_error(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("http error 429") || lower.contains("too many requests") {
        return "The video site is temporarily refusing requests. Try again later.".to_owned();
    }
    if lower.contains("sign in")
        || lower.contains("login required")
        || lower.contains("authentication required")
    {
        return "This video page requires sign-in or browser verification, which is not supported."
            .to_owned();
    }

    let redacted_lines: Vec<String> = raw
        .lines()
        .map(|line| {
            line.split_whitespace()
                .map(|token| {
                    if token.starts_with("http://") || token.starts_with("https://") {
                        "[URL]"
                    } else {
                        token
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    redacted_lines
        .iter()
        .rev()
        .find(|line| line.starts_with("ERROR:"))
        .or_else(|| redacted_lines.last())
        .map(|line| line.trim().chars().take(700).collect())
        .filter(|message: &String| !message.is_empty())
        .unwrap_or_else(|| "Video download failed.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::media::{HlsMasterPlaylist, HlsMediaPlaylist, HlsVariant, ParsedHls};

    #[test]
    fn test_format_choices_from_hls_master_variants() {
        let master = HlsMasterPlaylist {
            variants: vec![
                HlsVariant {
                    url: "https://example.com/1080p.m3u8".to_string(),
                    bandwidth: 5_000_000,
                    resolution: Some("1920x1080".to_string()),
                    audio_group: Some("audio".to_string()),
                    audio_url: Some("https://example.com/audio.m3u8".to_string()),
                },
                HlsVariant {
                    url: "https://example.com/720p.m3u8".to_string(),
                    bandwidth: 2_500_000,
                    resolution: Some("1280x720".to_string()),
                    audio_group: None,
                    audio_url: None,
                },
            ],
        };

        let choices = YtDlpTools::format_choices_from_parsed_hls(
            ParsedHls::Master(master),
            "https://example.com/master.m3u8",
        );

        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].quality, "1920x1080");
        assert_eq!(choices[0].youtube_format, "hls-5000000");
        assert_eq!(choices[0].audio_state, "Separate audio track");
        assert_eq!(choices[0].audio_url.as_deref(), Some("https://example.com/audio.m3u8"));

        assert_eq!(choices[1].quality, "1280x720");
        assert_eq!(choices[1].youtube_format, "hls-2500000");
        assert_eq!(choices[1].audio_state, "Combined audio/video");
        assert_eq!(choices[1].audio_url, None);
    }

    #[test]
    fn test_format_choices_from_hls_media_playlist() {
        let media = HlsMediaPlaylist {
            urls: vec!["https://example.com/seg1.ts".to_string()],
            segments: vec![],
            duration: 120.5,
            extension: ".ts",
            init_map: None,
            init_byte_range: None,
        };

        let choices = YtDlpTools::format_choices_from_parsed_hls(
            ParsedHls::Media(media),
            "https://example.com/media.m3u8",
        );

        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].quality, "Original");
        assert_eq!(choices[0].youtube_format, "hls-direct");
        assert_eq!(choices[0].duration, 120.5);
    }
}
