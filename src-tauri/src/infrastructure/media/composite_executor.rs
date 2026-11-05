use std::sync::Arc;

use crate::{
    application::{
        download_executor::{
            DownloadControl, DownloadExecution, DownloadExecutor, DownloadProgressHandler,
        },
        download_rate_limiter::DownloadRateLimiter,
        error::AppResult,
    },
    domain::download::DownloadRecord,
    infrastructure::{
        http_download_executor::HttpDownloadExecutor,
        media::hls_executor::HlsDownloadExecutor,
    },
};

#[derive(Clone)]
pub struct CompositeDownloadExecutor {
    http_executor: HttpDownloadExecutor,
    hls_executor: Arc<HlsDownloadExecutor>,
}

impl CompositeDownloadExecutor {
    pub fn new(http_executor: HttpDownloadExecutor, client: reqwest::Client) -> Self {
        Self {
            http_executor,
            hls_executor: Arc::new(HlsDownloadExecutor::new(client)),
        }
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
        let is_hls = download.kind.as_deref() == Some("hls")
            || download.url.ends_with(".m3u8")
            || download.url.contains(".m3u8?");

        if is_hls {
            let hls = self.hls_executor.clone();
            Box::pin(async move {
                hls.transfer(download, progress_handler, control, rate_limiter)
                    .await
            })
        } else {
            self.http_executor
                .execute(download, progress_handler, control, rate_limiter)
        }
    }

    fn discard_partial(&self, download: &DownloadRecord) -> AppResult<()> {
        let _ = self.hls_executor.discard_partial(download);
        self.http_executor.discard_partial(download)
    }
}
