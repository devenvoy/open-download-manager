// Created: 2026-11-08
// Reason: feat(media): expand browser media panel and adaptive stream support (Closes #37)
// Last updated: 2026-11-08
// Reason: feat(media): expand browser media panel and adaptive stream support (Closes #37)
// Description: Media panel candidate format representation containing URLs, bitrates, resolutions, and roles.

use serde::{Deserialize, Serialize};

use super::policy::redact_media_url;

/// The role/content composition of a media candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaRole {
    /// Combined multiplexed video and audio track
    VideoWithAudio,
    /// Video-only stream (requires separate audio or silent)
    VideoOnly,
    /// Audio-only track
    AudioOnly,
    /// Adaptive master manifest representing multiple selectable variants
    AdaptiveMaster,
    /// Unclassified media role
    Unknown,
}

impl MediaRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VideoWithAudio => "Video + audio",
            Self::VideoOnly => "Video only",
            Self::AudioOnly => "Audio only",
            Self::AdaptiveMaster => "Adaptive stream",
            Self::Unknown => "Unknown",
        }
    }
}

/// A media candidate discovered by the browser extension or desktop resolver.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaCandidate {
    /// The page URL where the media was embedded, sanitized.
    #[serde(default)]
    pub page_url: Option<String>,
    /// The actual media URL (may contain signed tokens).
    pub media_url: String,
    /// The MIME type of the candidate if known.
    #[serde(default)]
    pub mime_type: Option<String>,
    /// Codecs specification (e.g. "avc1.64001f, mp4a.40.2").
    #[serde(default)]
    pub codecs: Option<String>,
    /// Video width in pixels.
    #[serde(default)]
    pub width: Option<u32>,
    /// Video height in pixels.
    #[serde(default)]
    pub height: Option<u32>,
    /// Bitrate in bits per second.
    #[serde(default)]
    pub bitrate: Option<u64>,
    /// Role of this media candidate.
    #[serde(default = "default_role")]
    pub role: MediaRole,
    /// Whether this stream is currently unsupported (e.g. DRM or unsupported live stream).
    #[serde(default)]
    pub unsupported: bool,
    /// Explanatory reason if unsupported.
    #[serde(default)]
    pub unsupported_reason: Option<String>,
}

fn default_role() -> MediaRole {
    MediaRole::Unknown
}

impl MediaCandidate {
    pub fn new(media_url: impl Into<String>) -> Self {
        Self {
            page_url: None,
            media_url: media_url.into(),
            mime_type: None,
            codecs: None,
            width: None,
            height: None,
            bitrate: None,
            role: MediaRole::Unknown,
            unsupported: false,
            unsupported_reason: None,
        }
    }

    /// Return a safe version of the media URL with sensitive query strings and passwords redacted.
    pub fn sanitized_display_url(&self) -> String {
        redact_media_url(&self.media_url)
    }

    /// Whether this candidate is valid for queueing.
    pub fn is_selectable(&self) -> bool {
        !self.unsupported
    }

    /// Format label for user presentation (e.g. "1080p (Video + audio)").
    pub fn format_label(&self) -> String {
        let quality = if let Some(h) = self.height {
            format!("{h}p")
        } else {
            "Audio".to_string()
        };
        format!("{} ({})", quality, self.role.as_str())
    }
}
