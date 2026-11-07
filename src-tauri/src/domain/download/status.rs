use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Scheduled,
    Paused,
    Cancelled,
    Downloading,
    #[serde(rename = "probing", alias = "connecting")]
    Connecting,
    Assembling,
    #[serde(rename = "complete", alias = "completed", alias = "done")]
    Completed,
    #[serde(rename = "error", alias = "failed")]
    Failed,
    #[serde(rename = "skipped")]
    Skipped,
}

impl DownloadStatus {
    pub fn after_restart(self) -> Self {
        match self {
            Self::Downloading | Self::Connecting | Self::Assembling => Self::Paused,
            status => status,
        }
    }
}
