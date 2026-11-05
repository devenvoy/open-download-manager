use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadAction {
    Pause,
    Resume,
    Retry,
    Cancel,
    Remove,
    Open,
    Reveal,
}

impl DownloadAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Retry => "retry",
            Self::Cancel => "cancel",
            Self::Remove => "remove",
            Self::Open => "open",
            Self::Reveal => "reveal",
        }
    }
}
