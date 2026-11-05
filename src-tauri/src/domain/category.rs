use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum DownloadCategory {
    Video,
    Audio,
    Archives,
    Documents,
    Programs,
    Other,
}

impl DownloadCategory {
    pub const ALL: &[Self] = &[
        Self::Video,
        Self::Audio,
        Self::Archives,
        Self::Documents,
        Self::Programs,
        Self::Other,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Video => "Video",
            Self::Audio => "Audio",
            Self::Archives => "Archives",
            Self::Documents => "Documents",
            Self::Programs => "Programs",
            Self::Other => "Other",
        }
    }
}

impl fmt::Display for DownloadCategory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
