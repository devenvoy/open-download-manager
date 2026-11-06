use std::path::{Path, PathBuf};

use crate::{
    application::error::{AppError, AppResult},
    domain::media_tools::{MediaToolType, ToolDownloadSpec},
};

use super::super::adapter::MediaToolAdapter;

/// Fallback adapter for hosts without an automated media tool source.
#[derive(Clone, Copy, Debug)]
pub struct UnsupportedAdapter;

impl MediaToolAdapter for UnsupportedAdapter {
    fn id(&self) -> &'static str {
        "unsupported"
    }

    fn executable_name(&self, tool: MediaToolType) -> &'static str {
        match (tool, cfg!(windows)) {
            (MediaToolType::Ffmpeg, true) => "ffmpeg.exe",
            (MediaToolType::YtDlp, true) => "yt-dlp.exe",
            (MediaToolType::Ffmpeg, false) => "ffmpeg",
            (MediaToolType::YtDlp, false) => "yt-dlp",
        }
    }

    fn download_spec(&self, _tool: MediaToolType) -> AppResult<ToolDownloadSpec> {
        Err(AppError::download(
            "Automated media tool download is not supported on this platform. Install the tool manually or set AD_FFMPEG_PATH / AD_YTDLP_PATH.",
        ))
    }

    fn bundled_tool_candidates(&self, exe_name: &str, app_dir: &Path) -> Vec<PathBuf> {
        vec![app_dir.join("tools").join(exe_name), app_dir.join(exe_name)]
    }
}
