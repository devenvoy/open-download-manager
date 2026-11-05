use std::{path::{Path, PathBuf}, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};

use crate::{
    application::error::{AppError, AppResult},
    domain::media::{choices_from_info, youtube_error, youtube_url, YouTubeFormatChoice},
};

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
}

fn find_ytdlp_path() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("AD_YTDLP_PATH") {
        let path = PathBuf::from(env_path);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let candidate = parent.join(if cfg!(windows) { "tools/yt-dlp.exe" } else { "tools/yt-dlp" });
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let default_binary = if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" };
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let full_path = dir.join(default_binary);
            if full_path.is_file() {
                return Some(full_path);
            }
        }
    }

    None
}
