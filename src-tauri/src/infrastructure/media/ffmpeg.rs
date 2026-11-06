use std::{path::{Path, PathBuf}, process::Stdio};
use tokio::{io::AsyncReadExt, process::Command};

use crate::application::{
    download_executor::{stopped_error, DownloadControl},
    error::{AppError, AppResult},
};

#[derive(Clone)]
pub struct FfmpegTools {
    executable: PathBuf,
}

impl FfmpegTools {
    pub fn new() -> Option<Self> {
        let path = find_ffmpeg_path()?;
        Some(Self { executable: path })
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub async fn mux_tracks(
        &self,
        video_track: &Path,
        audio_track: &Path,
        output_file: &Path,
        control: &mut DownloadControl,
    ) -> AppResult<()> {
        let mut command = Command::new(&self.executable);
        command
            .arg("-nostdin")
            .arg("-v")
            .arg("error")
            .arg("-y")
            .arg("-protocol_whitelist")
            .arg("file,pipe")
            .arg("-i")
            .arg(video_track)
            .arg("-protocol_whitelist")
            .arg("file,pipe")
            .arg("-i")
            .arg(audio_track)
            .arg("-map")
            .arg("0:v:0")
            .arg("-map")
            .arg("1:a:0")
            .arg("-c")
            .arg("copy")
            .arg("-movflags")
            .arg("+faststart")
            .arg(output_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW

        run_command_with_control(command, control, output_file).await
    }

    pub async fn convert_media(
        &self,
        source_track: &Path,
        output_file: &Path,
        format: &str,
        control: &mut DownloadControl,
    ) -> AppResult<()> {
        if !matches!(format, "mp3" | "mp4") {
            return Err(AppError::download("Unsupported output format."));
        }

        let mut command = Command::new(&self.executable);
        command
            .arg("-nostdin")
            .arg("-v")
            .arg("error")
            .arg("-y")
            .arg("-protocol_whitelist")
            .arg("file,pipe")
            .arg("-i")
            .arg(source_track);

        if format == "mp3" {
            command
                .arg("-map")
                .arg("0:a:0")
                .arg("-vn")
                .arg("-c:a")
                .arg("libmp3lame")
                .arg("-q:a")
                .arg("2");
        } else {
            command
                .arg("-map")
                .arg("0:v:0")
                .arg("-map")
                .arg("0:a:0?")
                .arg("-c")
                .arg("copy")
                .arg("-movflags")
                .arg("+faststart");
        }
        command
            .arg(output_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW

        run_command_with_control(command, control, output_file).await
    }
}

async fn run_command_with_control(
    mut command: Command,
    control: &mut DownloadControl,
    output_file: &Path,
) -> AppResult<()> {
    let mut child = command
        .spawn()
        .map_err(|e| AppError::download(format!("Could not spawn ffmpeg: {e}")))?;

    let mut stderr_handle = child.stderr.take();
    let mut stderr_bytes = Vec::new();

    tokio::select! {
        reason = control.stopped() => {
            let _ = child.kill().await;
            let _ = tokio::fs::remove_file(output_file).await;
            Err(stopped_error(reason))
        }
        res = async {
            let stderr_fut = async {
                if let Some(ref mut handle) = stderr_handle {
                    let _ = handle.read_to_end(&mut stderr_bytes).await;
                }
            };
            let (status, _) = tokio::join!(child.wait(), stderr_fut);
            status
        } => {
            let status = res.map_err(|e| AppError::download(format!("ffmpeg process error: {e}")))?;
            if !status.success() {
                let _ = tokio::fs::remove_file(output_file).await;
                let stderr_str = String::from_utf8_lossy(&stderr_bytes);
                return Err(AppError::download(format_ffmpeg_error(status.code(), &stderr_str)));
            }
            Ok(())
        }
    }
}

fn format_ffmpeg_error(code: Option<i32>, stderr: &str) -> String {
    let clean = stderr.trim();
    if clean.is_empty() {
        return match code {
            Some(c) => format!("Media assembly failed (FFmpeg process exited with code {c})."),
            None => "Media assembly process terminated unexpectedly.".to_string(),
        };
    }

    let error_line = clean
        .lines()
        .rev()
        .find(|l| {
            let lower = l.to_ascii_lowercase();
            lower.contains("error") || lower.contains("invalid") || lower.contains("failed") || lower.contains("cannot")
        })
        .or_else(|| clean.lines().last())
        .unwrap_or(clean);

    let short_error = error_line.trim().chars().take(300).collect::<String>();
    format!("Media assembly failed: {short_error}")
}

fn find_ffmpeg_path() -> Option<PathBuf> {
    super::tools::find_tool_path("ffmpeg")
}
