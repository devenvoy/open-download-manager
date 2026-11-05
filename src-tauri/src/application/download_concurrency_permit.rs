use std::sync::Arc;

use super::download_concurrency_limiter::DownloadConcurrencyLimiter;

pub(super) struct DownloadConcurrencyPermit {
    limiter: Arc<DownloadConcurrencyLimiter>,
}

impl DownloadConcurrencyPermit {
    pub(super) fn new(limiter: Arc<DownloadConcurrencyLimiter>) -> Self {
        Self { limiter }
    }
}

impl Drop for DownloadConcurrencyPermit {
    fn drop(&mut self) {
        match self.limiter.active.lock() {
            Ok(mut active) => *active -= 1,
            Err(error) => {
                eprintln!("Download queue state was poisoned; recovering its permit count.");
                *error.into_inner() -= 1;
            }
        }
        self.limiter.changed.notify_one();
    }
}
