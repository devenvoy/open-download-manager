// Created: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Last updated: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Description: Sanitizer for SABR media session contexts stripping expired parameters and credentials.

use base64::prelude::*;
use url::Url;

use super::{
    request::parse_sabr_protobuf_request,
    types::{
        NormalizedSabrSession, ParsedSabrSeed, SabrAudioTrack, SabrClientInfo, SabrColorInfo,
        SabrFormat, SabrSession,
    },
};

pub fn google_video_sabr_url(value: &str) -> Result<String, String> {
    let url = Url::parse(value).map_err(|_| "Invalid YouTube SABR URL.".to_owned())?;
    if url.scheme() != "https" {
        return Err("Invalid YouTube SABR URL.".to_owned());
    }
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    if host != "googlevideo.com" && !host.ends_with(".googlevideo.com") {
        return Err("Invalid YouTube SABR URL.".to_owned());
    }
    if url.path() != "/videoplayback" {
        return Err("Invalid YouTube SABR URL.".to_owned());
    }
    if url.query_pairs().find(|(k, v)| k == "sabr" && v == "1").is_none() {
        return Err("Invalid YouTube SABR URL.".to_owned());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Invalid YouTube SABR URL.".to_owned());
    }
    Ok(url.to_string())
}

pub fn normalize_sabr_session(value: &SabrSession) -> Result<NormalizedSabrSession, String> {
    if value.video_id.len() != 11
        || !value
            .video_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Invalid YouTube SABR session.".to_owned());
    }

    let server_abr_streaming_url = google_video_sabr_url(&value.server_abr_streaming_url)?;

    if value.video_playback_ustreamer_config.is_empty()
        || value.video_playback_ustreamer_config.len() > 131_072
    {
        return Err("Missing YouTube streaming configuration.".to_owned());
    }

    if value.formats.is_empty() || value.formats.len() > 150 {
        return Err("Missing YouTube SABR formats.".to_owned());
    }

    let audio_itag = value.audio_itag;
    let video_itag = value.video_itag;

    let audio_candidates: Vec<&SabrFormat> = value
        .formats
        .iter()
        .filter(|f| {
            f.itag == audio_itag
                && f.mime_type.to_ascii_lowercase().starts_with("audio/mp4")
                && !f.is_drc
                && !f.is_vb
        })
        .collect();

    let audio_raw = audio_candidates
        .iter()
        .find(|f| f.audio_track.as_ref().is_some_and(|t| t.audio_is_default))
        .or_else(|| audio_candidates.iter().find(|f| f.audio_track.is_none()))
        .or_else(|| audio_candidates.first())
        .copied();

    let video_raw = video_itag.and_then(|v_itag| {
        value
            .formats
            .iter()
            .find(|f| f.itag == v_itag && f.mime_type.to_ascii_lowercase().starts_with("video/mp4"))
    });

    let mut chosen_formats = Vec::new();
    if let Some(v) = video_raw {
        let mut fmt = v.clone();
        fmt.has_audio = fmt.mime_type.to_ascii_lowercase().starts_with("audio/");
        fmt.has_video = true;
        chosen_formats.push(fmt);
    }
    if let Some(a) = audio_raw {
        let mut fmt = a.clone();
        fmt.has_audio = true;
        fmt.has_video = fmt.mime_type.to_ascii_lowercase().starts_with("video/");
        fmt.audio_track_id = fmt.audio_track.as_ref().and_then(|t| t.id.clone());
        chosen_formats.push(fmt);
    }

    if !chosen_formats
        .iter()
        .any(|f| f.itag == audio_itag && f.has_audio)
    {
        return Err("The selected YouTube audio track is unavailable.".to_owned());
    }

    if let Some(v_itag) = video_itag {
        if !chosen_formats
            .iter()
            .any(|f| f.itag == v_itag && f.has_video)
        {
            return Err("The selected YouTube video track is unavailable.".to_owned());
        }
    }

    let mut seed: Option<ParsedSabrSeed> = None;
    if let Some(ref rb) = value.request_body {
        if rb.is_empty() || rb.len() > 350_000 || !is_valid_base64_str(rb) {
            return Err("Invalid YouTube SABR request context.".to_owned());
        }
        let decoded = BASE64_STANDARD
            .decode(rb.as_bytes())
            .map_err(|_| "Invalid YouTube SABR request context.".to_owned())?;
        if decoded.is_empty() || decoded.len() > 262_144 {
            return Err("Invalid YouTube SABR request context.".to_owned());
        }
        let parsed = parse_sabr_protobuf_request(&decoded)
            .map_err(|_| "The captured YouTube SABR request context is unreadable.".to_owned())?;
        seed = Some(parsed);
    }

    let captured_client = seed.as_ref().and_then(|s| s.client_info.as_ref());
    let incoming_client = value.client_info.as_ref();

    let client_name = captured_client
        .map(|c| c.client_name)
        .or_else(|| incoming_client.map(|c| c.client_name))
        .unwrap_or(1);

    let client_version = captured_client
        .filter(|c| !c.client_version.is_empty())
        .map(|c| c.client_version.clone())
        .or_else(|| incoming_client.map(|c| c.client_version.clone()))
        .unwrap_or_default();

    let hl = captured_client
        .and_then(|c| c.hl.clone())
        .or_else(|| incoming_client.and_then(|c| c.hl.clone()));
    let device_make = captured_client
        .and_then(|c| c.device_make.clone())
        .or_else(|| incoming_client.and_then(|c| c.device_make.clone()));
    let device_model = captured_client
        .and_then(|c| c.device_model.clone())
        .or_else(|| incoming_client.and_then(|c| c.device_model.clone()));
    let os_name = captured_client
        .and_then(|c| c.os_name.clone())
        .or_else(|| incoming_client.and_then(|c| c.os_name.clone()));
    let os_version = captured_client
        .and_then(|c| c.os_version.clone())
        .or_else(|| incoming_client.and_then(|c| c.os_version.clone()));

    let client_info = SabrClientInfo {
        client_name,
        client_version,
        hl,
        device_make,
        device_model,
        os_name,
        os_version,
    };

    let po_token = seed
        .as_ref()
        .and_then(|s| s.po_token.as_ref())
        .map(|bytes| BASE64_STANDARD.encode(bytes))
        .or_else(|| value.po_token.clone());

    Ok(NormalizedSabrSession {
        video_id: value.video_id.clone(),
        server_abr_streaming_url,
        video_playback_ustreamer_config: value.video_playback_ustreamer_config.clone(),
        client_info,
        formats: chosen_formats,
        video_itag,
        audio_itag,
        request_body: value.request_body.clone(),
        po_token,
    })
}

fn is_valid_base64_str(value: &str) -> bool {
    let mut padding = 0;
    for c in value.chars() {
        if padding > 0 {
            if c != '=' || padding > 2 {
                return false;
            }
            padding += 1;
        } else if c == '=' {
            padding = 1;
        } else if !c.is_ascii_alphanumeric() && c != '+' && c != '/' {
            return false;
        }
    }
    true
}

fn bounded_integer(val: Option<&serde_json::Value>, max: u64) -> Result<u64, String> {
    let num = val
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "Invalid SABR number.".to_owned())?;
    if num > max {
        return Err("Invalid SABR number.".to_owned());
    }
    Ok(num)
}

fn bounded_text(value: &str, max: usize, pattern: fn(char) -> bool) -> Result<String, String> {
    if value.len() > max || !value.chars().all(pattern) {
        return Err("Invalid SABR text.".to_owned());
    }
    Ok(value.to_owned())
}

pub fn sanitize_sabr(
    value: &serde_json::Value,
    download_url: &str,
) -> Result<SabrSession, String> {
    if !value.is_object() {
        return Err("Invalid SABR session.".to_owned());
    }

    let raw_streaming_url = value
        .get("serverAbrStreamingUrl")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid SABR session.".to_owned())?;

    let stream = Url::parse(raw_streaming_url).map_err(|_| "Invalid SABR stream URL.".to_owned())?;
    if stream.as_str() != download_url {
        return Err("Invalid SABR stream URL.".to_owned());
    }

    let host = stream.host_str().unwrap_or("").to_ascii_lowercase();
    if host != "googlevideo.com" && !host.ends_with(".googlevideo.com") {
        return Err("Invalid SABR stream URL.".to_owned());
    }
    if stream.path() != "/videoplayback" {
        return Err("Invalid SABR stream URL.".to_owned());
    }
    if stream.query_pairs().find(|(k, v)| k == "sabr" && v == "1").is_none() {
        return Err("Invalid SABR stream URL.".to_owned());
    }

    let raw_video_id = value
        .get("videoId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid SABR text.".to_owned())?;
    if raw_video_id.len() != 11
        || !raw_video_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Invalid SABR text.".to_owned());
    }

    let raw_ustreamer = value
        .get("videoPlaybackUstreamerConfig")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid SABR text.".to_owned())?;
    let ustreamer_config = bounded_text(raw_ustreamer, 131_072, |c| {
        c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '/' | '=' | '.' | '-' | ':' | ';' | ',' | '~')
    })?;

    let raw_formats = value
        .get("formats")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Invalid SABR formats.".to_owned())?;
    if raw_formats.is_empty() || raw_formats.len() > 150 {
        return Err("Invalid SABR formats.".to_owned());
    }

    let mut formats = Vec::with_capacity(raw_formats.len());
    for raw in raw_formats {
        if !raw.is_object() {
            return Err("Invalid SABR format.".to_owned());
        }

        let itag = bounded_integer(raw.get("itag"), 99_999)? as u32;

        let mime = raw
            .get("mimeType")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Invalid SABR text.".to_owned())?;
        if mime.len() > 200 {
            return Err("Invalid SABR text.".to_owned());
        }
        let lower_mime = mime.to_ascii_lowercase();
        let valid_mime_prefix = lower_mime.starts_with("audio/mp4")
            || lower_mime.starts_with("video/mp4")
            || lower_mime.starts_with("audio/webm")
            || lower_mime.starts_with("video/webm");
        if !valid_mime_prefix || mime.contains('\r') || mime.contains('\n') || mime.contains('\0') {
            return Err("Invalid SABR text.".to_owned());
        }

        let bitrate = raw
            .get("bitrate")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let width = if let Some(v) = raw.get("width") {
            Some(bounded_integer(Some(v), 1_000_000_000)?)
        } else {
            None
        };
        let height = if let Some(v) = raw.get("height") {
            Some(bounded_integer(Some(v), 1_000_000_000)?)
        } else {
            None
        };
        let fps = if let Some(v) = raw.get("fps") {
            Some(bounded_integer(Some(v), 1_000_000_000)?)
        } else {
            None
        };
        let average_bitrate = if let Some(v) = raw.get("averageBitrate") {
            Some(bounded_integer(Some(v), 1_000_000_000)?)
        } else {
            None
        };
        let audio_channels = if let Some(v) = raw.get("audioChannels") {
            Some(bounded_integer(Some(v), 1_000_000_000)?)
        } else {
            None
        };

        let last_modified = if let Some(v) = raw.get("lastModified") {
            let s = v.as_str().or_else(|| v.as_u64().map(|_| "")).unwrap_or("");
            let val_str = if s.is_empty() && v.is_number() {
                v.to_string()
            } else {
                s.to_string()
            };
            Some(bounded_text(&val_str, 32, |c| c.is_ascii_digit())?)
        } else {
            None
        };

        let content_length = if let Some(v) = raw.get("contentLength") {
            let s = v.as_str().or_else(|| v.as_u64().map(|_| "")).unwrap_or("");
            let val_str = if s.is_empty() && v.is_number() {
                v.to_string()
            } else {
                s.to_string()
            };
            Some(bounded_text(&val_str, 32, |c| c.is_ascii_digit())?)
        } else {
            None
        };

        let approx_duration_ms = if let Some(v) = raw.get("approxDurationMs") {
            let s = v.as_str().or_else(|| v.as_u64().map(|_| "")).unwrap_or("");
            let val_str = if s.is_empty() && v.is_number() {
                v.to_string()
            } else {
                s.to_string()
            };
            Some(bounded_text(&val_str, 32, |c| c.is_ascii_digit())?)
        } else {
            None
        };

        let target_duration_sec = if let Some(v) = raw.get("targetDurationSec") {
            let n = v.as_f64().ok_or_else(|| "Invalid SABR duration.".to_owned())?;
            if !n.is_finite() || n < 0.0 || n > 86_400.0 {
                return Err("Invalid SABR duration.".to_owned());
            }
            Some(n)
        } else {
            None
        };

        let xtags = if let Some(v) = raw.get("xtags").and_then(|v| v.as_str()) {
            Some(bounded_text(v, 2048, |c| c != '\r' && c != '\n' && c != '\0')?)
        } else {
            None
        };

        let audio_quality = if let Some(v) = raw.get("audioQuality").and_then(|v| v.as_str()) {
            Some(bounded_text(v, 80, |c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')?)
        } else {
            None
        };

        let quality = if let Some(v) = raw.get("quality").and_then(|v| v.as_str()) {
            Some(bounded_text(v, 80, |c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ' | '(' | ')' | '/' | '+' | ','))?)
        } else {
            None
        };

        let quality_label = if let Some(v) = raw.get("qualityLabel").and_then(|v| v.as_str()) {
            Some(bounded_text(v, 80, |c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ' | '(' | ')' | '/' | '+' | ','))?)
        } else {
            None
        };

        let audio_track = if let Some(at) = raw.get("audioTrack").and_then(|v| v.as_object()) {
            let id = if let Some(id_val) = at.get("id").and_then(|v| v.as_str()) {
                Some(bounded_text(id_val, 100, |c| c != '\r' && c != '\n' && c != '\0')?)
            } else {
                None
            };
            let audio_is_default = at.get("audioIsDefault").and_then(|v| v.as_bool()).unwrap_or(false);
            Some(SabrAudioTrack {
                id,
                audio_is_default,
            })
        } else {
            None
        };

        let color_info = if let Some(ci) = raw.get("colorInfo").and_then(|v| v.as_object()) {
            let tc = if let Some(s) = ci.get("transferCharacteristics").and_then(|v| v.as_str()) {
                Some(bounded_text(s, 80, |c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' '))?)
            } else {
                None
            };
            Some(SabrColorInfo {
                transfer_characteristics: tc,
            })
        } else {
            None
        };

        let is_drc = raw.get("isDrc").and_then(|v| v.as_bool()).unwrap_or(false);
        let is_vb = raw.get("isVb").and_then(|v| v.as_bool()).unwrap_or(false);

        formats.push(SabrFormat {
            itag,
            mime_type: mime.to_owned(),
            bitrate,
            width,
            height,
            fps,
            average_bitrate,
            audio_channels,
            last_modified,
            content_length,
            approx_duration_ms,
            target_duration_sec,
            xtags,
            audio_quality,
            quality,
            quality_label,
            audio_track,
            color_info,
            is_drc,
            is_vb,
            has_audio: false,
            has_video: false,
            audio_track_id: None,
        });
    }

    let audio_itag = bounded_integer(value.get("audioItag"), 99_999)? as u32;
    let video_itag = if let Some(v) = value.get("videoItag") {
        Some(bounded_integer(Some(v), 99_999)? as u32)
    } else {
        None
    };

    let has_audio_format = formats
        .iter()
        .any(|f| f.itag == audio_itag && f.mime_type.to_ascii_lowercase().starts_with("audio/mp4"));
    if !has_audio_format {
        return Err("Selected SABR format is unavailable.".to_owned());
    }

    if let Some(v_itag) = video_itag {
        let has_video_format = formats
            .iter()
            .any(|f| f.itag == v_itag && f.mime_type.to_ascii_lowercase().starts_with("video/mp4"));
        if !has_video_format {
            return Err("Selected SABR format is unavailable.".to_owned());
        }
    }

    let client = value.get("clientInfo").and_then(|v| v.as_object());
    let client_name = if let Some(c) = client {
        bounded_integer(c.get("clientName"), 10_000).unwrap_or(1) as u32
    } else {
        1
    };
    let client_version = if let Some(c) = client {
        if let Some(v) = c.get("clientVersion").and_then(|v| v.as_str()) {
            bounded_text(v, 80, |c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')?
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let client_info = SabrClientInfo {
        client_name,
        client_version,
        hl: None,
        device_make: None,
        device_model: None,
        os_name: None,
        os_version: None,
    };

    let request_body = if let Some(v) = value.get("requestBody").and_then(|v| v.as_str()) {
        let text = bounded_text(v, 350_000, |c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')?;
        let decoded = BASE64_STANDARD
            .decode(text.as_bytes())
            .map_err(|_| "Invalid SABR request body.".to_owned())?;
        if decoded.is_empty() || decoded.len() > 262_144 {
            return Err("Invalid SABR request body.".to_owned());
        }
        let re_encoded = BASE64_STANDARD.encode(&decoded);
        if re_encoded.trim_end_matches('=') != text.trim_end_matches('=') {
            return Err("Invalid SABR request body.".to_owned());
        }
        Some(text)
    } else {
        None
    };

    let session = SabrSession {
        video_id: raw_video_id.to_owned(),
        server_abr_streaming_url: raw_streaming_url.to_owned(),
        video_playback_ustreamer_config: ustreamer_config,
        client_info: Some(client_info),
        formats,
        video_itag,
        audio_itag,
        request_body,
        po_token: None,
    };

    Ok(session)
}
