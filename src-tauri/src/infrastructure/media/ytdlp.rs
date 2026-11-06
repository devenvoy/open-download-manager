use std::{path::{Path, PathBuf}, process::Stdio, time::Duration};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt}, process::Command};

use crate::{
    application::{
        download_executor::{stopped_error, DownloadControl, DownloadProgressHandler},
        download_progress::DownloadProgress,
        download_result::DownloadResult,
        error::{AppError, AppResult},
    },
    domain::{
        download::DownloadSegmentProgress,
        media::{
            choices_from_info, parse_playlist_json, parse_youtube_playlist_url, youtube_error,
            youtube_url, PlaylistMetadata, YouTubeFormatChoice,
        },
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

        let mut command = Command::new(&self.executable);
        command
            .arg("--skip-download")
            .arg("--dump-single-json")
            .arg("--no-playlist")
            .arg("--no-colors")
            .arg(&canonical_url)
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
                return Err(AppError::download("YouTube request timed out."));
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

        choices_from_info(&json_value, &canonical_url).map_err(AppError::download)
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
            .arg("--progress")
            .arg("--progress-delta")
            .arg("0.5")
            .arg("--progress-template")
            .arg("download:ADPROGRESS:%(progress)j");

        if let Some(ffmpeg_path) = super::tools::find_tool_path("ffmpeg") {
            command.arg("--ffmpeg-location").arg(ffmpeg_path);
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

        let is_mp3 = destination
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".mp3");
        if is_mp3 {
            command.arg("--extract-audio").arg("--audio-format").arg("mp3");
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
                            if let Some(json_str) = l.strip_prefix("ADPROGRESS:") {
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                                    let downloaded = val.get("downloaded_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let total = val.get("total_bytes").and_then(|v| v.as_u64())
                                        .or_else(|| val.get("total_bytes_estimate").and_then(|v| v.as_u64()));
                                    let speed = val.get("speed").and_then(|v| v.as_f64()).map(|s| s as u64).unwrap_or(0);

                                    progress_handler(DownloadProgress {
                                        downloaded,
                                        total,
                                        bytes_per_second: speed,
                                        connections_progress: vec![DownloadSegmentProgress {
                                            start: 0,
                                            end: total.unwrap_or(downloaded),
                                            downloaded,
                                            total: total.unwrap_or(downloaded),
                                            status: "downloading".to_string(),
                                        }],
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
            let (msg, _, _) = youtube_error(&combined_err);
            return Err(AppError::download(msg));
        }

        let total_size = tokio::fs::metadata(destination)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        if total_size == 0 {
            return Err(AppError::download("YouTube download produced an empty file."));
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
