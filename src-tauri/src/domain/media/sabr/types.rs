// Created: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Last updated: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Description: Data models for SABR streaming contexts, track itags, and video identification.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SabrAudioTrack {
    pub id: Option<String>,
    #[serde(default)]
    pub audio_is_default: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SabrColorInfo {
    pub transfer_characteristics: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SabrFormat {
    pub itag: u32,
    pub mime_type: String,
    #[serde(default)]
    pub bitrate: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fps: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_bitrate: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_channels: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_length: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approx_duration_ms: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_duration_sec: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xtags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_quality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_track: Option<SabrAudioTrack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_info: Option<SabrColorInfo>,
    #[serde(default)]
    pub is_drc: bool,
    #[serde(default)]
    pub is_vb: bool,
    #[serde(default)]
    pub has_audio: bool,
    #[serde(default)]
    pub has_video: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_track_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SabrClientInfo {
    #[serde(default = "default_client_name")]
    pub client_name: u32,
    #[serde(default)]
    pub client_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_make: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
}

fn default_client_name() -> u32 {
    1
}

impl Default for SabrClientInfo {
    fn default() -> Self {
        Self {
            client_name: 1,
            client_version: String::new(),
            hl: None,
            device_make: None,
            device_model: None,
            os_name: None,
            os_version: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SabrSession {
    pub video_id: String,
    pub server_abr_streaming_url: String,
    pub video_playback_ustreamer_config: String,
    #[serde(default)]
    pub client_info: Option<SabrClientInfo>,
    pub formats: Vec<SabrFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_itag: Option<u32>,
    pub audio_itag: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub po_token: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedSabrSession {
    pub video_id: String,
    pub server_abr_streaming_url: String,
    pub video_playback_ustreamer_config: String,
    pub client_info: SabrClientInfo,
    pub formats: Vec<SabrFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_itag: Option<u32>,
    pub audio_itag: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub po_token: Option<String>,
}

impl NormalizedSabrSession {
    pub fn to_session(&self) -> SabrSession {
        SabrSession {
            video_id: self.video_id.clone(),
            server_abr_streaming_url: self.server_abr_streaming_url.clone(),
            video_playback_ustreamer_config: self.video_playback_ustreamer_config.clone(),
            client_info: Some(self.client_info.clone()),
            formats: self.formats.clone(),
            video_itag: self.video_itag,
            audio_itag: self.audio_itag,
            request_body: self.request_body.clone(),
            po_token: self.po_token.clone(),
        }
    }
}

#[derive(Default)]
pub struct ParsedSabrSeed {
    pub client_info: Option<SabrClientInfo>,
    pub po_token: Option<Vec<u8>>,
}
