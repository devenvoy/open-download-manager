// Created: 2026-11-05
// Reason: feat: start queued downloads via Tauri actions
// Last updated: 2026-11-05
// Reason: feat: implement Phase A download context actions and filesystem parity
// Description: Enumeration of user and system download control actions (pause, resume, cancel, delete).

use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadAction {
    Pause,
    Resume,
    Retry,
    Cancel,
    Remove,
    Open,
    Reveal,
    OpenWith,
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
            Self::OpenWith => "openWith",
        }
    }
}
