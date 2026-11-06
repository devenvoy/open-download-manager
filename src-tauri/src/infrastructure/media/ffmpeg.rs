use std::{path::{Path, PathBuf}, process::Stdio};
use tokio::process::Command;

use crate::application::{
    download_executor::{stopped_error, DownloadControl},
    error::{AppError, AppResult},
};

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

    tokio::select! {
        reason = control.stopped() => {
            let _ = child.kill().await;
            let _ = tokio::fs::remove_file(output_file).await;
            Err(stopped_error(reason))
        }
        status = child.wait() => {
            let status = status.map_err(|e| AppError::download(format!("ffmpeg process error: {e}")))?;
            if !status.success() {
                let _ = tokio::fs::remove_file(output_file).await;
                return Err(AppError::download(format!("Media assembly failed with exit code: {:?}", status.code())));
            }
            Ok(())
        }
    }
}

fn find_ffmpeg_path() -> Option<PathBuf> {
    super::tools_manager::find_tool_path("ffmpeg")
}
