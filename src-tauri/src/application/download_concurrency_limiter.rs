use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use tokio::sync::Notify;

use crate::application::error::{AppError, AppResult};

use super::download_concurrency_permit::DownloadConcurrencyPermit;

pub(super) struct DownloadConcurrencyLimiter {
    pub(super) active: Mutex<usize>,
    max_concurrent: AtomicUsize,
    pub(super) changed: Notify,
}

impl DownloadConcurrencyLimiter {
    pub(super) fn new(max_concurrent: u8) -> Arc<Self> {
        Arc::new(Self {
            active: Mutex::new(0),
            max_concurrent: AtomicUsize::new(usize::from(max_concurrent)),
            changed: Notify::new(),
        })
    }

    pub(super) fn max_concurrent(&self) -> usize {
        self.max_concurrent.load(Ordering::Acquire)
    }

    pub(super) async fn acquire(self: &Arc<Self>) -> AppResult<DownloadConcurrencyPermit> {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();

            let acquired = {
                let mut active = self
                    .active
                    .lock()
                    .map_err(|_| AppError::storage("Download queue is unavailable."))?;
                if *active < self.max_concurrent.load(Ordering::Acquire) {
                    *active += 1;
                    true
                } else {
                    false
                }
            };
            if acquired {
                return Ok(DownloadConcurrencyPermit::new(self.clone()));
            }
            changed.await;
        }
    }

    pub(super) fn set_max_concurrent(&self, value: u8) {
        self.max_concurrent
            .store(usize::from(value), Ordering::Release);
        self.changed.notify_waiters();
    }
}
