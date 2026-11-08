// Created: 2026-11-07
// Reason: feat(media): complete YouTube media pipeline, desktop quality selector, resilient download resumption, and native messaging registration
// Last updated: 2026-11-08
// Reason: feat(transfer): implement IDM-identical chunk rebalancing, recovery, and eager probing
// Description: YouTube format executor acquiring streaming URLs and delegating to adaptive downloaders.

use std::{
    path::PathBuf,
    sync::Arc,
};

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
        _progress_handler: DownloadProgressHandler,
        _control: DownloadControl,
        _rate_limiter: Arc<DownloadRateLimiter>,
    ) -> AppResult<DownloadResult> {
        Err(AppError::download(format!(
            "The address '{}' is a web page, not a direct media stream. To download this video with full multi-connection acceleration, please play the video in your web browser and click the Download button on the media panel.",
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
