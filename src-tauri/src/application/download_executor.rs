// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-08
// Reason: feat(network): implement HTTP and HTTPS proxy support with reachability test (fixes #11)
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Description: Abstractions, traits, control signals, and cancellation error types for download executors.

use std::{future::Future, pin::Pin, sync::Arc};

use tokio::sync::watch;

use crate::{
    application::{
        download_progress::DownloadProgress,
        download_rate_limiter::DownloadRateLimiter,
        download_result::DownloadResult,
        error::{AppError, AppResult},
    },
    domain::download::DownloadRecord,
};

pub type DownloadExecution =
    Pin<Box<dyn Future<Output = AppResult<DownloadResult>> + Send + 'static>>;
pub type DownloadProgressHandler = Arc<dyn Fn(DownloadProgress) + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadStopReason {
    Pause,
    Cancel,
}

#[derive(Clone)]
pub struct DownloadControl {
    stop: watch::Receiver<Option<DownloadStopReason>>,
}

impl DownloadControl {
    pub fn new(stop: watch::Receiver<Option<DownloadStopReason>>) -> Self {
        Self { stop }
    }

    pub async fn stopped(&mut self) -> DownloadStopReason {
        loop {
            if let Some(reason) = *self.stop.borrow() {
                return reason;
            }
            if self.stop.changed().await.is_err() {
                return DownloadStopReason::Cancel;
            }
        }
    }

    pub fn current_stop_reason(&self) -> Option<DownloadStopReason> {
        *self.stop.borrow()
    }
}

pub fn stopped_error(reason: DownloadStopReason) -> AppError {
    match reason {
        DownloadStopReason::Pause => AppError {
            code: "download_paused",
            message: "The download was paused.".to_owned(),
        },
        DownloadStopReason::Cancel => AppError {
            code: "download_cancelled",
            message: "The download was cancelled.".to_owned(),
        },
    }
}

pub trait DownloadExecutor: Send + Sync {
    fn execute(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
        control: DownloadControl,
        rate_limiter: Arc<DownloadRateLimiter>,
    ) -> DownloadExecution;

    fn discard_partial(&self, download: &DownloadRecord) -> AppResult<()>;

    fn set_proxy(&self, _proxy: Option<&crate::domain::settings::ProxySettings>) -> AppResult<()> {
        Ok(())
    }

    fn set_connection_overrides(
        &self,
        _overrides: &[crate::domain::settings::ConnectionOverride],
        _default_connections: u8,
    ) {
    }
}
