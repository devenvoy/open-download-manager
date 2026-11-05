use crate::application::error::AppResult;

pub trait DownloadEventPublisher: Send + Sync {
    fn publish_change(&self) -> AppResult<()>;
}
