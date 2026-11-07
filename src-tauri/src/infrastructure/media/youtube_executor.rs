use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    application::{
        download_executor::{DownloadControl, DownloadProgressHandler},
        download_rate_limiter::DownloadRateLimiter,
        download_result::DownloadResult,
        error::{AppError, AppResult},
    },
    domain::{
        download::DownloadRecord,
        media::youtube_url,
    },
    infrastructure::media::ytdlp::YtDlpTools,
};

pub struct YoutubeDownloadExecutor {
    ytdlp: Option<YtDlpTools>,
}

impl YoutubeDownloadExecutor {
    pub fn new() -> Self {
        Self {
            ytdlp: YtDlpTools::new(),
        }
    }

    pub fn client(&self) -> Option<&YtDlpTools> {
        self.ytdlp.as_ref()
    }

    pub async fn transfer(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
        control: DownloadControl,
        rate_limiter: Arc<DownloadRateLimiter>,
    ) -> AppResult<DownloadResult> {
        let ytdlp = self.ytdlp.as_ref().cloned().or_else(YtDlpTools::new).ok_or_else(|| {
            AppError::download(
                "yt-dlp is required to download extracted video streams. Please install it from Settings > Tools.",
            )
        })?;

        // Determine canonical target URL
        let canonical_url = youtube_url(&download.url)
            .or_else(|| download.referer.as_deref().and_then(youtube_url))
            .or_else(|| {
                download.sabr.as_ref().and_then(|s| {
                    if !s.video_id.is_empty() {
                        Some(format!("https://www.youtube.com/watch?v={}", s.video_id))
                    } else {
                        None
                    }
                })
            })
            .or_else(|| {
                if let Ok(parsed) = url::Url::parse(&download.url) {
                    if parsed.host_str().map_or(false, |h| h.contains("googlevideo.com")) {
                        if let Some((_, docid)) = parsed.query_pairs().find(|(k, _)| k == "docid" || k == "id") {
                            let docid_str = docid.to_string();
                            if docid_str.len() == 11 {
                                return Some(format!("https://www.youtube.com/watch?v={docid_str}"));
                            }
                        }
                    }
                }
                None
            });

        let target_url = match canonical_url {
            Some(u) => u,
            None => {
                if download.url.contains("googlevideo.com") {
                    return Err(AppError::download(
                        "Could not resolve canonical YouTube watch URL for this media stream. Please open the video page directly to download.",
                    ));
                } else {
                    download.url.clone()
                }
            }
        };

        let directory = download
            .directory
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| AppError::download("Download directory is not set."))?;

        let filename_lower = download.filename.to_ascii_lowercase();
        let destination_filename = if filename_lower.ends_with(".mp3")
            || filename_lower.ends_with(".webm")
            || filename_lower.ends_with(".mp4")
        {
            download.filename.clone()
        } else {
            let base = Path::new(&download.filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&download.filename);
            format!("{base}.mp4")
        };

        let destination = directory.join(destination_filename);
        let speed_limit = rate_limiter.limit_kbps() as u64;

        ytdlp
            .download(
                &target_url,
                download.youtube_format.as_deref(),
                download.referer.as_deref(),
                &destination,
                progress_handler,
                control,
                speed_limit,
            )
            .await
    }

    pub fn discard_partial(&self, download: &DownloadRecord) -> AppResult<()> {
        let directory = match &download.directory {
            Some(d) => PathBuf::from(d),
            None => return Ok(()),
        };
        let dest = directory.join(&download.filename);
        let part1 = directory.join(format!("{}.part", download.filename));
        let part2 = directory.join(format!("{}.ytdl", download.filename));
        let _ = std::fs::remove_file(part1);
        let _ = std::fs::remove_file(part2);
        let _ = std::fs::remove_file(dest);
        Ok(())
    }
}
