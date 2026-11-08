// Created: 2026-11-06
// Reason: feat(media): modular production-grade media tools resolver and test suite
// Last updated: 2026-11-07
// Reason: fix(media): handle ffmpeg errors, yt-dlp execution, stream truncation and UI error dialogs
// Description: Domain specifications for external media processing dependencies such as FFmpeg and yt-dlp.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Supported media processing tools
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaToolType {
    Ffmpeg,
    YtDlp,
}

impl MediaToolType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ffmpeg => "ffmpeg",
            Self::YtDlp => "yt-dlp",
        }
    }

    pub fn env_var_override(&self) -> &'static str {
        match self {
            Self::Ffmpeg => "AD_FFMPEG_PATH",
            Self::YtDlp => "AD_YTDLP_PATH",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "ffmpeg" => Some(Self::Ffmpeg),
            "yt-dlp" | "ytdlp" => Some(Self::YtDlp),
            _ => None,
        }
    }
}

/// The origin tier where a media tool was located on the host
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolResolutionSource {
    EnvironmentOverride,
    BundledSidecar,
    ManagedUserData,
    SystemPath,
}

/// Status of a media tool for UI and engine readiness
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaToolStatus {
    pub tool_type: MediaToolType,
    pub is_available: bool,
    pub path: Option<PathBuf>,
    pub source: Option<ToolResolutionSource>,
}

/// Specification for fetching a tool binary
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDownloadSpec {
    pub url: String,
    pub binary_name: String,
}
