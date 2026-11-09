// Created: 2026-11-08
// Reason: feat(media): expand browser media panel and adaptive stream support (Closes #37)
// Last updated: 2026-11-08
// Reason: feat(media): expand browser media panel and adaptive stream support (Closes #37)
// Description: Media candidate acceptance policies, URL redacting, and format role classification rules.

use url::Url;

/// The version of the media capture policy specification.
pub const MEDIA_CAPTURE_POLICY_VERSION: u32 = 1;

/// Registered video MIME families recognized by Open Download Manager.
pub const VIDEO_MIME_TYPES: &[&str] = &[
    "video/mp4",
    "video/webm",
    "video/x-msvideo",
    "video/avi",
    "video/msvideo",
    "video/mpeg",
    "video/mpg",
    "video/x-mpeg",
    "video/mp2t",
    "video/quicktime",
    "video/x-quicktime",
    "video/x-ms-wmv",
    "video/x-ms-asf",
    "video/x-ms-wvx",
    "video/asf",
    "video/wmv",
    "video/x-flv",
    "video/flv",
    "video/x-f4v",
    "video/f4v",
    "video/x-m4v",
    "video/m4v",
    "video/divx",
    "video/x-divx",
    "video/3gpp",
    "video/3gpp2",
    "video/ogg",
    "video/x-matroska",
    "video/mkv",
];

/// Registered audio MIME families.
pub const AUDIO_MIME_TYPES: &[&str] = &[
    "audio/mp4",
    "audio/mp3",
    "audio/mpeg",
    "audio/webm",
    "audio/aac",
    "audio/ogg",
    "audio/opus",
    "audio/wav",
    "audio/x-wav",
    "audio/flac",
    "audio/x-flac",
    "audio/x-m4a",
    "audio/m4a",
];

/// Registered adaptive stream MIME types.
pub const HLS_MIME_TYPES: &[&str] = &[
    "application/vnd.apple.mpegurl",
    "application/x-mpegurl",
    "audio/mpegurl",
    "audio/x-mpegurl",
];

pub const DASH_MIME_TYPES: &[&str] = &[
    "application/dash+xml",
];

/// Video file extensions.
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "webm", "avi", "mpg", "mpeg", "mov", "wmv", "asf", "wvx",
    "flv", "f4v", "m4v", "divx", "mkv", "3gp", "3g2", "ogv", "ts",
];

/// Audio file extensions.
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "aac", "ogg", "opus", "wav", "flac", "weba",
];

/// Sensitive query parameter names that must be redacted in UI diagnostics and logs.
const SENSITIVE_QUERY_PARAMS: &[&str] = &[
    "sig",
    "signature",
    "token",
    "access_token",
    "auth_token",
    "auth",
    "key",
    "key_id",
    "key-pair-id",
    "hmac",
    "expire",
    "expires",
    "exp",
    "session",
    "session_id",
    "sessionid",
    "secret",
    "password",
    "pass",
    "sparams",
];

/// Redact signed URL query parameters and user credentials so sensitive tokens
/// never leak to logs, UI diagnostics, or disk records.
pub fn redact_media_url(raw_url: &str) -> String {
    let mut parsed = match Url::parse(raw_url) {
        Ok(u) => u,
        Err(_) => return raw_url.to_string(),
    };

    // Redact password if present in URL authority
    if parsed.password().is_some() {
        let _ = parsed.set_password(Some("******"));
    }

    // Redact sensitive query parameters
    let pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(k, v)| {
            let k_lower = k.to_ascii_lowercase();
            if SENSITIVE_QUERY_PARAMS.iter().any(|&s| s == k_lower) {
                (k.into_owned(), "******".to_string())
            } else {
                (k.into_owned(), v.into_owned())
            }
        })
        .collect();

    if !pairs.is_empty() {
        parsed.query_pairs_mut().clear().extend_pairs(pairs.into_iter());
    }

    parsed.to_string()
}

/// The classified delivery route for a captured media stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaRoute {
    /// Standard direct HTTP/HTTPS media file (MP4, WebM, AVI, etc.)
    DirectMedia {
        mime: Option<String>,
        extension: Option<String>,
    },
    /// YouTube / Google Video delivery network route
    YouTubeDelivery {
        endpoint: String,
        is_sabr: bool,
        video_id: Option<String>,
    },
    /// HTTP Live Streaming playlist (HLS)
    HlsStream {
        is_master: bool,
    },
    /// Dynamic Adaptive Streaming over HTTP (DASH / MPD)
    DashManifest,
    /// Unsupported adaptive format with explanatory reason
    UnsupportedAdaptive {
        reason: String,
    },
    /// Unsupported format with explanatory reason
    UnsupportedFormat {
        mime_or_ext: String,
        reason: String,
    },
    /// Not recognized as media
    NonMedia,
}

/// Versioned media capture policy catalog for MIME and host rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaCapturePolicy {
    pub version: u32,
}

impl Default for MediaCapturePolicy {
    fn default() -> Self {
        Self {
            version: MEDIA_CAPTURE_POLICY_VERSION,
        }
    }
}

impl MediaCapturePolicy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a MIME type matches known video types.
    pub fn is_video_mime(&self, mime: &str) -> bool {
        let base_mime = mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
        VIDEO_MIME_TYPES.iter().any(|&v| v == base_mime)
    }

    /// Check if a MIME type matches known audio types.
    pub fn is_audio_mime(&self, mime: &str) -> bool {
        let base_mime = mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
        AUDIO_MIME_TYPES.iter().any(|&a| a == base_mime)
    }

    /// Check if a MIME type matches HLS.
    pub fn is_hls_mime(&self, mime: &str) -> bool {
        let base_mime = mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
        HLS_MIME_TYPES.iter().any(|&h| h == base_mime)
    }

    /// Check if a MIME type matches DASH.
    pub fn is_dash_mime(&self, mime: &str) -> bool {
        let base_mime = mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
        DASH_MIME_TYPES.iter().any(|&d| d == base_mime)
    }

    /// Check if an extension matches known video formats.
    pub fn is_video_extension(&self, ext: &str) -> bool {
        let clean = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        VIDEO_EXTENSIONS.iter().any(|&v| v == clean)
    }

    /// Check if an extension matches known audio formats.
    pub fn is_audio_extension(&self, ext: &str) -> bool {
        let clean = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        AUDIO_EXTENSIONS.iter().any(|&a| a == clean)
    }

    /// Check if a host is part of YouTube or Google Video delivery infrastructure.
    pub fn is_youtube_or_googlevideo_host(&self, host: &str) -> bool {
        let host_lower = host.to_ascii_lowercase();
        host_lower == "youtube.com"
            || host_lower.ends_with(".youtube.com")
            || host_lower == "googlevideo.com"
            || host_lower.ends_with(".googlevideo.com")
            || host_lower == "youtu.be"
            || host_lower.ends_with(".youtu.be")
    }

    /// Classify a media candidate based on its URL and optional MIME type.
    pub fn classify(&self, raw_url: &str, mime: Option<&str>) -> MediaRoute {
        let parsed = match Url::parse(raw_url) {
            Ok(u) => u,
            Err(_) => return MediaRoute::NonMedia,
        };

        let host = parsed.host_str().unwrap_or("");
        let path = parsed.path();
        let path_lower = path.to_ascii_lowercase();
        let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();

        // 1. YouTube / Google Video dispatch
        if self.is_youtube_or_googlevideo_host(host) {
            let is_sabr = parsed
                .query_pairs()
                .any(|(k, v)| k == "sabr" && v == "1");

            let video_id = parsed
                .query_pairs()
                .find(|(k, _)| k == "v")
                .map(|(_, v)| v.into_owned())
                .or_else(|| {
                    if path_lower.starts_with("/shorts/") {
                        path.strip_prefix("/shorts/").and_then(|s| s.split('/').next()).map(|s| s.to_string())
                    } else if host.ends_with("youtu.be") {
                        path.strip_prefix('/').and_then(|s| s.split('/').next()).map(|s| s.to_string())
                    } else {
                        None
                    }
                });

            let endpoint = if path_lower == "/videoplayback" {
                "videoplayback".to_string()
            } else if path_lower.contains("player") {
                "player".to_string()
            } else if path_lower.contains("get_video_info") {
                "get_video_info".to_string()
            } else {
                path_lower
            };

            return MediaRoute::YouTubeDelivery {
                endpoint,
                is_sabr,
                video_id,
            };
        }

        // 2. Adaptive Streams: HLS (.m3u8)
        if ext == "m3u8" || mime.map_or(false, |m| self.is_hls_mime(m)) {
            let is_master = path_lower.contains("master") || path_lower.contains("playlist");
            return MediaRoute::HlsStream { is_master };
        }

        // 3. Adaptive Streams: DASH (.mpd)
        if ext == "mpd" || mime.map_or(false, |m| self.is_dash_mime(m)) {
            return MediaRoute::DashManifest;
        }

        // 4. Direct video/audio by MIME
        if let Some(m) = mime {
            if self.is_video_mime(m) || self.is_audio_mime(m) {
                return MediaRoute::DirectMedia {
                    mime: Some(m.to_string()),
                    extension: if !ext.is_empty() { Some(ext) } else { None },
                };
            }
        }

        // 5. Direct video/audio by extension
        if self.is_video_extension(&ext) || self.is_audio_extension(&ext) {
            return MediaRoute::DirectMedia {
                mime: mime.map(|m| m.to_string()),
                extension: Some(ext),
            };
        }

        MediaRoute::NonMedia
    }
}
