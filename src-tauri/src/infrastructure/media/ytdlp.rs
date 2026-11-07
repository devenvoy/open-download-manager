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
        let is_yt = youtube_url(input_url).is_some();
        self.fetch_site_formats_from_url(parsed.as_str(), is_yt).await
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
            .arg("download:ADPROGRESS:%(progress)j");

        if let Some(ffmpeg_path) = super::tools::find_tool_path("ffmpeg") {
            command.arg("--ffmpeg-location").arg(ffmpeg_path);
        }
        if let Some(referer) = referer {
            command.arg("--referer").arg(referer);
        }

        if let Some(spec) = format_spec {
            let clean = spec.trim().trim_start_matches("audio:");
            let resilient_spec = if clean.contains('+') {
                let parts: Vec<&str> = clean.splitn(2, '+').collect();
                let v = parts[0].trim();
                let a = parts[1].trim();
                format!("{v}+{a}/{v}+ba/{v}+bestaudio/bv*+ba/best")
            } else if !clean.is_empty() {
                format!("{clean}/ba/bestaudio/best")
            } else {
                "bv*+ba/best".to_string()
            };
            command.arg("-f").arg(resilient_spec);
        } else {
            command.arg("-f").arg("bv*+ba/best");
        }

        let output_format = destination
            .to_string_lossy()
            .to_ascii_lowercase()
            .rsplit_once('.')
            .map(|(_, extension)| extension.to_owned())
            .unwrap_or_default();
        if output_format == "mp3" {
            command.arg("--extract-audio").arg("--audio-format").arg("mp3");
        } else if matches!(output_format.as_str(), "mp4" | "webm") {
            command.arg("--merge-output-format").arg(output_format);
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
