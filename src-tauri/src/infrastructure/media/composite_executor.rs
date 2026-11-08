// Created: 2026-11-05
// Reason: feat: add media handling for HLS, FFmpeg, yt-dlp, and stream progress
// Last updated: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Reason: feat(mms): add MMS streaming support or explicit unsupported guidance (Closes #35)
// Description: Composite download executor routing transfers to HTTP, FTP, MMS, HLS, or YouTube executors.

use std::sync::{Arc, RwLock};

use crate::{
    application::{
        download_executor::{
            DownloadControl, DownloadExecution, DownloadExecutor, DownloadProgressHandler,
        },
        download_rate_limiter::DownloadRateLimiter,
        error::AppResult,
    },
    domain::{download::DownloadRecord, settings::ProxySettings},
    infrastructure::{
        ftp::FtpDownloadExecutor,
        http_download_executor::{create_http_client, HttpDownloadExecutor},
        media::{
            hls_executor::HlsDownloadExecutor, paired_executor::PairedDownloadExecutor,
            sabr_executor::SabrDownloadExecutor, youtube_executor::YoutubeDownloadExecutor,
        },
        mms::MmsDownloadExecutor,
    },
};

#[derive(Clone)]
pub struct CompositeDownloadExecutor {
    http_executor: HttpDownloadExecutor,
    ftp_executor: Arc<FtpDownloadExecutor>,
    mms_executor: Arc<MmsDownloadExecutor>,
    hls_executor: Arc<HlsDownloadExecutor>,
    paired_executor: Arc<PairedDownloadExecutor>,
    sabr_executor: Arc<SabrDownloadExecutor>,
    youtube_executor: Arc<YoutubeDownloadExecutor>,
    client: Arc<RwLock<reqwest::Client>>,
}

impl CompositeDownloadExecutor {
    pub fn new(http_executor: HttpDownloadExecutor, client: reqwest::Client) -> Self {
        let shared_client = Arc::new(RwLock::new(client));
        let allocator = http_executor.allocator();
        Self {
            http_executor: HttpDownloadExecutor::with_shared_client_and_allocator(
                shared_client.clone(),
                allocator.clone(),
            ),
            ftp_executor: Arc::new(FtpDownloadExecutor::new(allocator.clone())),
            mms_executor: Arc::new(MmsDownloadExecutor::new(allocator)),
            hls_executor: Arc::new(HlsDownloadExecutor::new_shared(shared_client.clone())),
            paired_executor: Arc::new(PairedDownloadExecutor::new_shared(shared_client.clone())),
            sabr_executor: Arc::new(SabrDownloadExecutor::new_shared(shared_client.clone())),
            youtube_executor: Arc::new(YoutubeDownloadExecutor::new()),
            client: shared_client,
        }
    }

    pub fn with_shared(
        http_executor: HttpDownloadExecutor,
        shared_client: Arc<RwLock<reqwest::Client>>,
    ) -> Self {
        let allocator = http_executor.allocator();
        Self {
            http_executor,
            ftp_executor: Arc::new(FtpDownloadExecutor::new(allocator.clone())),
            mms_executor: Arc::new(MmsDownloadExecutor::new(allocator)),
            hls_executor: Arc::new(HlsDownloadExecutor::new_shared(shared_client.clone())),
            paired_executor: Arc::new(PairedDownloadExecutor::new_shared(shared_client.clone())),
            sabr_executor: Arc::new(SabrDownloadExecutor::new_shared(shared_client.clone())),
            youtube_executor: Arc::new(YoutubeDownloadExecutor::new()),
            client: shared_client,
        }
    }

    pub fn with_credentials(
        mut self,
        credential_store: Arc<crate::infrastructure::credentials::CredentialStore>,
    ) -> Self {
        let allocator = self.http_executor.allocator();
        self.http_executor = self.http_executor.with_credentials(credential_store.clone());
        self.ftp_executor = Arc::new(FtpDownloadExecutor::new(allocator).with_credentials(credential_store));
        self
    }
}

impl DownloadExecutor for CompositeDownloadExecutor {
    fn execute(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
        control: DownloadControl,
        rate_limiter: Arc<DownloadRateLimiter>,
    ) -> DownloadExecution {
        let is_sabr = download.kind.as_deref() == Some("sabr") || download.sabr.is_some();
        let is_paired = download.kind.as_deref() == Some("paired")
            || download.audio_url.is_some();
        let is_hls = download.kind.as_deref() == Some("hls")
            || download.url.ends_with(".m3u8")
            || download.url.contains(".m3u8?");
        let is_direct_stream = download.url.contains("googlevideo.com") || download.url.contains("videoplayback");
        let is_youtube = !is_direct_stream
            && (download.kind.as_deref() == Some("youtube")
                || crate::domain::media::youtube_url(&download.url).is_some()
                || download.youtube_format.is_some());

        let is_ftp = download.url.starts_with("ftp://")
            || download.kind.as_deref() == Some("ftp");

        let is_mms = download.url.starts_with("mms://")
            || download.url.starts_with("mmsh://")
            || download.kind.as_deref() == Some("mms");

        if is_ftp {
            self.ftp_executor
                .execute(download, progress_handler, control, rate_limiter)
        } else if is_mms {
            self.mms_executor
                .execute(download, progress_handler, control, rate_limiter)
        } else if is_sabr {
            let sabr = self.sabr_executor.clone();
            Box::pin(async move {
                sabr.transfer(download, progress_handler, control, rate_limiter)
                    .await
            })
        } else if is_paired {
            let paired = self.paired_executor.clone();
            Box::pin(async move {
                paired
                    .transfer(download, progress_handler, control, rate_limiter)
                    .await
            })
        } else if is_hls {
            let hls = self.hls_executor.clone();
            Box::pin(async move {
                hls.transfer(download, progress_handler, control, rate_limiter)
                    .await
            })
        } else if is_youtube {
            let youtube = self.youtube_executor.clone();
            Box::pin(async move {
                youtube
                    .transfer(download, progress_handler, control, rate_limiter)
                    .await
            })
        } else {
            self.http_executor
                .execute(download, progress_handler, control, rate_limiter)
        }
    }

    fn discard_partial(&self, download: &DownloadRecord) -> AppResult<()> {
        let _ = self.ftp_executor.discard_partial(download);
        let _ = self.mms_executor.discard_partial(download);
        let _ = self.sabr_executor.discard_partial(download);
        let _ = self.paired_executor.discard_partial(download);
        let _ = self.hls_executor.discard_partial(download);
        let _ = self.youtube_executor.discard_partial(download);
        self.http_executor.discard_partial(download)
    }

    fn set_proxy(&self, proxy: Option<&ProxySettings>) -> AppResult<()> {
        let new_client = create_http_client(proxy)?;
        self.http_executor.set_proxy(proxy)?;
        let _ = self.mms_executor.set_proxy(proxy);
        *self.client.write().unwrap() = new_client;
        Ok(())
    }

    fn set_connection_overrides(
        &self,
        overrides: &[crate::domain::settings::ConnectionOverride],
        _default_connections: u8,
    ) {
        self.http_executor.set_connection_overrides(overrides);
    }
}
