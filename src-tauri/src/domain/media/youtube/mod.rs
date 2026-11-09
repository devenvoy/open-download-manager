// Created: 2026-11-05
// Reason: feat: add media handling for HLS, FFmpeg, yt-dlp, and stream progress
// Last updated: 2026-11-10
// Reason: refactor: modularize youtube domain module into focused submodules
// Description: YouTube-specific URL detection, format matching, and adaptive stream handoff policy.

pub mod choices;
pub mod error;
pub mod site;
pub mod types;
pub mod url;

pub use choices::choices_from_info;
pub use error::youtube_error;
pub use site::site_choices_from_info;
pub use types::YouTubeFormatChoice;
pub use url::youtube_url;
