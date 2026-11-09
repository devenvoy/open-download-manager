// Created: 2026-11-05
// Reason: feat: add media handling for HLS, FFmpeg, yt-dlp, and stream progress
// Last updated: 2026-11-10
// Reason: refactor: modularize youtube domain module into focused submodules
// Description: Data models representing YouTube and generic media format choices.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YouTubeFormatChoice {
    pub url: String,
    pub provider: String,
    pub youtube_format: String,
    pub format: String,
    pub quality: String,
    pub fps: u32,
    pub audio_state: String,
    pub title: String,
    pub output_format: String,
    pub estimated_size: u64,
    pub duration: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_url: Option<String>,
}
