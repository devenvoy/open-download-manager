use crate::{
    application::error::AppResult,
    domain::download::DownloadRecord,
};

pub trait DownloadEventPublisher: Send + Sync {
    fn publish_change(&self) -> AppResult<()>;
    fn publish_complete(&self, _download: &DownloadRecord) -> AppResult<()> {
        Ok(())
    }
}
