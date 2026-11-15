// Created: 2026-11-07
// Reason: feat(media): complete YouTube media pipeline, desktop quality selector, resilient download resumption, and native messaging registration
// Last updated: 2026-11-08
// Reason: feat(transfer): implement IDM-identical chunk rebalancing, recovery, and eager probing
// Description: YouTube format executor acquiring streaming URLs and delegating to adaptive downloaders.

use std::{
    path::PathBuf,
    sync::Arc,
};

use super::ytdlp::YtDlpTools;
use crate::{
    application::{
        download_executor::{DownloadControl, DownloadProgressHandler},
        download_rate_limiter::DownloadRateLimiter,
        download_result::DownloadResult,
        error::{AppError, AppResult},
    },
    domain::download::DownloadRecord,
};

pub struct YoutubeDownloadExecutor;

impl YoutubeDownloadExecutor {
    pub fn new() -> Self {
        Self
    }

    pub async fn transfer(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
        control: DownloadControl,
        _rate_limiter: Arc<DownloadRateLimiter>,
    ) -> AppResult<DownloadResult> {
        if let Some(ytdlp) = YtDlpTools::new() {
            let directory = download
                .directory
                .as_ref()
                .map(PathBuf::from)
                .ok_or_else(|| AppError::download("Download directory is not set."))?;
            let destination = directory.join(&download.filename);
            return ytdlp
                .download(
                    &download.url,
                    download.youtube_format.as_deref(),
                    download.referer.as_deref(),
                    &destination,
                    progress_handler,
                    control,
                    0,
                )
                .await;
        }
        Err(AppError::download(format!(
            "yt-dlp is required to download media from '{}'. Please install yt-dlp in Settings > Media Tools.",
            download.url
        )))
    }

    pub fn discard_partial(&self, download: &DownloadRecord) -> AppResult<()> {
        let directory = match &download.directory {
            Some(d) => PathBuf::from(d),
            None => return Ok(()),
        };
        let dest = directory.join(&download.filename);
        let part1 = directory.join(format!("{}.part", download.filename));
        let _ = std::fs::remove_file(part1);
        let _ = std::fs::remove_file(dest);
        Ok(())
    }
}
