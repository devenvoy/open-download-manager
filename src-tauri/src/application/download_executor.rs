use std::{future::Future, pin::Pin, sync::Arc};

use crate::{
    application::{
        download_progress::DownloadProgress, download_result::DownloadResult, error::AppResult,
    },
    domain::download::DownloadRecord,
};

pub type DownloadExecution =
    Pin<Box<dyn Future<Output = AppResult<DownloadResult>> + Send + 'static>>;
pub type DownloadProgressHandler = Arc<dyn Fn(DownloadProgress) + Send + Sync>;

pub trait DownloadExecutor: Send + Sync {
    fn execute(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
    ) -> DownloadExecution;
}
