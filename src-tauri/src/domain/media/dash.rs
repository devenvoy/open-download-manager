// Created: 2026-11-08
// Reason: feat(media): expand browser media panel and adaptive stream support (Closes #37)
// Last updated: 2026-11-08
// Reason: feat(media): expand browser media panel and adaptive stream support (Closes #37)
// Description: MPEG-DASH MPD manifest parser extracting representations, segment templates, and initialization URLs.

use url::Url;

const MAX_DASH_MANIFEST_SIZE: usize = 1024 * 1024; // 1 MiB max
const MAX_DASH_SEGMENTS: usize = 20_000;

#[derive(Clone, Debug, PartialEq)]
pub struct DashTrack {
    pub id: String,
    pub mime_type: String,
    pub codecs: Option<String>,
    pub bandwidth: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub segment_urls: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DashPlan {
    pub video_track: Option<DashTrack>,
    pub audio_track: Option<DashTrack>,
    pub is_dynamic: bool,
    pub has_drm: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DashError {
    OversizedManifest,
    MalformedManifest(String),
    EncryptedDrm(String),
    LiveStream(String),
    UnsupportedTemplate(String),
    NoPlayableTracks(String),
}

impl std::fmt::Display for DashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OversizedManifest => write!(f, "DASH MPD manifest exceeds 1 MiB limit."),
            Self::MalformedManifest(msg) => write!(f, "Invalid DASH MPD manifest: {msg}"),
            Self::EncryptedDrm(msg) => write!(f, "Encrypted DASH stream: {msg}"),
            Self::LiveStream(msg) => write!(f, "Live DASH stream: {msg}"),
            Self::UnsupportedTemplate(msg) => write!(f, "Unsupported DASH segment template: {msg}"),
            Self::NoPlayableTracks(msg) => write!(f, "No playable tracks in DASH manifest: {msg}"),
        }
    }
}

impl std::error::Error for DashError {}

fn extract_attribute<'a>(tag: &'a str, attr_name: &str) -> Option<&'a str> {
    for quote in ['"', '\''] {
        let pattern = format!("{attr_name}={quote}");
        let mut search_from = 0;
        while let Some(rel_idx) = tag[search_from..].find(&pattern) {
            let start = search_from + rel_idx;
            let is_boundary = start == 0
                || tag.as_bytes()[start - 1].is_ascii_whitespace()
                || tag.as_bytes()[start - 1] == b'<'
                || tag.as_bytes()[start - 1] == b'>';
            if is_boundary {
                let val_start = start + pattern.len();
                if let Some(val_len) = tag[val_start..].find(quote) {
                    return Some(&tag[val_start..val_start + val_len]);
                }
            }
            search_from = start + pattern.len();
        }
    }
    None
}

fn resolve_url(relative_or_absolute: &str, base: &str) -> Result<String, DashError> {
    let base_url = Url::parse(base).map_err(|e| DashError::MalformedManifest(e.to_string()))?;
    let resolved = base_url
        .join(relative_or_absolute.trim())
        .map_err(|e| DashError::MalformedManifest(e.to_string()))?;
    Ok(resolved.to_string())
}

/// Parse a DASH MPD manifest with bounds checking and explicit classification of unsupported streams.
pub fn parse_dash(xml: &str, base_url: &str) -> Result<DashPlan, DashError> {
    if xml.len() > MAX_DASH_MANIFEST_SIZE {
        return Err(DashError::OversizedManifest);
    }

    let trimmed = xml.trim_start();
    if !trimmed.contains("<MPD") && !trimmed.contains("<mpd") {
        return Err(DashError::MalformedManifest("Missing <MPD> root element.".to_string()));
    }

    // DRM Check: ContentProtection elements indicate Widevine, PlayReady, ClearKey, FairPlay
    if xml.contains("<ContentProtection") || xml.contains("<contentProtection") || xml.contains("cenc:pssh") {
        return Err(DashError::EncryptedDrm(
            "Encrypted DRM DASH streams are not supported. This stream requires DRM licenses and cannot be downloaded.".to_string()
        ));
    }

    // Dynamic / Live stream check
    if let Some(mpd_idx) = xml.find("<MPD").or_else(|| xml.find("<mpd")) {
        if let Some(mpd_tag_end) = xml[mpd_idx..].find('>') {
            let mpd_header = &xml[mpd_idx..mpd_idx + mpd_tag_end];
            if let Some(mpd_type) = extract_attribute(mpd_header, "type") {
                if mpd_type.eq_ignore_ascii_case("dynamic") {
                    return Err(DashError::LiveStream(
                        "Live DASH streams are not supported yet. Choose an on-demand video.".to_string(),
                    ));
                }
            }
        }
    }

    // Check for complex dynamic SegmentTemplate
    if xml.contains("<SegmentTemplate") && (xml.contains("$Number$") || xml.contains("$Time$")) && !xml.contains("<SegmentTimeline") {
        return Err(DashError::UnsupportedTemplate(
            "Dynamic SegmentTemplate DASH manifests without an explicit timeline are not supported. Choose an MP4 or HLS format if available.".to_string()
        ));
    }

    // Simple bounded parser for static VOD AdaptationSets and Representations
    let mut video_tracks = Vec::new();
    let mut audio_tracks = Vec::new();

    // Iterate through AdaptationSets
    let parts: Vec<&str> = xml.split("<AdaptationSet").collect();
    for adaptation_str in parts.into_iter().skip(1) {
        let end_idx = adaptation_str.find("</AdaptationSet>").unwrap_or(adaptation_str.len());
        let adaptation_body = &adaptation_str[..end_idx];
        let tag_header_end = adaptation_body.find('>').unwrap_or(adaptation_body.len());
        let adapt_header = &adaptation_body[..tag_header_end];

        let content_type = extract_attribute(adapt_header, "contentType")
            .or_else(|| extract_attribute(adapt_header, "mimeType"))
            .unwrap_or("");

        let is_video = content_type.starts_with("video");
        let is_audio = content_type.starts_with("audio");

        // Parse representations in this AdaptationSet
        let rep_parts: Vec<&str> = adaptation_body.split("<Representation").collect();
        for rep_str in rep_parts.into_iter().skip(1) {
            let rep_end = rep_str.find("</Representation>").or_else(|| rep_str.find("/>")).unwrap_or(rep_str.len());
            let rep_body = &rep_str[..rep_end];
            let rep_header_end = rep_body.find('>').unwrap_or(rep_body.len());
            let rep_header = &rep_body[..rep_header_end];

            let id = extract_attribute(rep_header, "id").unwrap_or("default").to_string();
            let bandwidth = extract_attribute(rep_header, "bandwidth")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            let width = extract_attribute(rep_header, "width").and_then(|v| v.parse::<u32>().ok());
            let height = extract_attribute(rep_header, "height").and_then(|v| v.parse::<u32>().ok());
            let codecs = extract_attribute(rep_header, "codecs").map(|s| s.to_string());
            let mime_type = extract_attribute(rep_header, "mimeType")
                .map(|s| s.to_string())
                .unwrap_or_else(|| if is_video { "video/mp4".to_string() } else { "audio/mp4".to_string() });

            // Extract segment URLs (BaseURL or SegmentList)
            let mut segment_urls = Vec::new();
            if let Some(base_url_start) = rep_body.find("<BaseURL>") {
                let after = &rep_body[base_url_start + 9..];
                if let Some(base_url_end) = after.find("</BaseURL>") {
                    let base_val = &after[..base_url_end];
                    if let Ok(resolved) = resolve_url(base_val, base_url) {
                        segment_urls.push(resolved);
                    }
                }
            }

            // SegmentList with SegmentURL elements
            if let Some(seg_list_start) = rep_body.find("<SegmentList>") {
                let after_list = &rep_body[seg_list_start + 13..];
                let list_end = after_list.find("</SegmentList>").unwrap_or(after_list.len());
                let list_body = &after_list[..list_end];

                // Check for Initialization
                if let Some(init_start) = list_body.find("<Initialization") {
                    let init_end = list_body[init_start..].find('>').unwrap_or(0);
                    let init_tag = &list_body[init_start..init_start + init_end];
                    if let Some(src) = extract_attribute(init_tag, "sourceURL") {
                        if let Ok(resolved) = resolve_url(src, base_url) {
                            segment_urls.push(resolved);
                        }
                    }
                }

                for seg_part in list_body.split("<SegmentURL").skip(1) {
                    if segment_urls.len() >= MAX_DASH_SEGMENTS {
                        break;
                    }
                    let seg_end = seg_part.find('>').unwrap_or(seg_part.len());
                    let seg_tag = &seg_part[..seg_end];
                    if let Some(media) = extract_attribute(seg_tag, "media") {
                        if let Ok(resolved) = resolve_url(media, base_url) {
                            segment_urls.push(resolved);
                        }
                    }
                }
            }

            let track = DashTrack {
                id,
                mime_type,
                codecs,
                bandwidth,
                width,
                height,
                segment_urls,
            };

            if is_video || width.is_some() || height.is_some() {
                video_tracks.push(track);
            } else if is_audio {
                audio_tracks.push(track);
            }
        }
    }

    if video_tracks.is_empty() && audio_tracks.is_empty() {
        return Err(DashError::NoPlayableTracks(
            "The DASH manifest did not contain any unencrypted playable video or audio representations.".to_string()
        ));
    }

    // Sort by bandwidth descending
    video_tracks.sort_unstable_by(|a, b| b.bandwidth.cmp(&a.bandwidth));
    audio_tracks.sort_unstable_by(|a, b| b.bandwidth.cmp(&a.bandwidth));

    Ok(DashPlan {
        video_track: video_tracks.into_iter().next(),
        audio_track: audio_tracks.into_iter().next(),
        is_dynamic: false,
        has_drm: false,
    })
}
