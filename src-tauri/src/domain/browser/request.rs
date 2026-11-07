use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::domain::{download::sanitize_filename, media::sabr::{sanitize_sabr, SabrSession}};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserIncomingMessage {
    pub action: String,
    pub download_id: Option<String>,
    pub url: Option<String>,
    pub youtube_format: Option<String>,
    pub sabr: Option<serde_json::Value>,
    pub start_now: Option<bool>,
    pub audio_url: Option<String>,
    pub output_format: Option<String>,
    pub urls: Option<Vec<String>>,
    pub filename: Option<String>,
    pub referer: Option<String>,
    pub canonical_url: Option<String>,
    pub video_id: Option<String>,
    #[serde(rename = "requestHeaders")]
    pub request_headers: Option<HashMap<String, String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedBrowserRequest {
    pub action: String,
    pub download_id: Option<String>,
    pub url: String,
    pub canonical_url: Option<String>,
    pub youtube_format: Option<String>,
    pub sabr: Option<SabrSession>,
    pub start_now: bool,
    pub audio_url: Option<String>,
    pub output_format: Option<String>,
    pub urls: Option<Vec<String>>,
    pub filename: Option<String>,
    pub referer: Option<String>,
    pub headers: HashMap<String, String>,
}

fn validate_url(value: &str) -> Result<String, String> {
    if value.len() > 16_384 {
        return Err("Invalid download URL: length exceeds limit.".to_owned());
    }
    let parsed = Url::parse(value).map_err(|_| "Invalid download URL.".to_owned())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Only HTTP(S) URLs are supported.".to_owned());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("URLs with embedded credentials are not supported.".to_owned());
    }
    Ok(parsed.to_string())
}

pub fn sanitize_browser_request(
    msg: BrowserIncomingMessage,
) -> Result<ValidatedBrowserRequest, String> {
    if !matches!(msg.action.as_str(), "download" | "formats" | "refresh") {
        return Err("Unsupported native action.".to_owned());
    }

    let download_id = msg.download_id.filter(|id| {
        !id.is_empty()
            && id.chars().count() <= 64
            && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    });

    let raw_url = msg.url.ok_or_else(|| "URL is required.".to_owned())?;
    let valid_url = validate_url(&raw_url)?;

    if msg.action == "formats" {
        return Ok(ValidatedBrowserRequest {
            action: "formats".to_owned(),
            download_id: None,
            url: valid_url,
            canonical_url: None,
            youtube_format: None,
            sabr: None,
            start_now: false,
            audio_url: None,
            output_format: None,
            urls: None,
            filename: None,
            referer: None,
            headers: HashMap::new(),
        });
    }

    let sabr = if let Some(raw_sabr) = msg.sabr {
        Some(sanitize_sabr(&raw_sabr, &valid_url)?)
    } else {
        None
    };

    if let Some(format) = &msg.output_format {
        if !matches!(format.as_str(), "mp4" | "webm" | "mp3") {
            return Err("Invalid output format.".to_owned());
        }
    }

    let filename = if let Some(raw_name) = msg.filename {
        if raw_name.chars().count() > 255 || raw_name == "." || raw_name == ".." {
            return Err("Invalid filename.".to_owned());
        }
        let sanitized = sanitize_filename(&raw_name);
        if sanitized.is_empty() {
            return Err("Invalid filename.".to_owned());
        }
        let lower = sanitized.to_ascii_lowercase();
        if lower == "watch"
            || lower == "watch.mp4"
            || lower == "watch.webm"
            || lower == "video"
            || lower == "video.mp4"
            || lower == "videoplayback"
        {
            None
        } else {
            Some(sanitized)
        }
    } else {
        None
    };

    let audio_url = if let Some(a_url) = msg.audio_url {
        Some(validate_url(&a_url)?)
    } else {
        None
    };

    let canonical = msg.canonical_url.and_then(|u| {
        let valid = validate_url(&u).ok()?;
        if valid.contains("youtube.com") || valid.contains("youtu.be") {
            crate::domain::media::youtube_url(&valid)
        } else {
            Some(valid)
        }
    });
    let derived_video_url = if let Some(ref vid) = msg.video_id {
        if vid.len() == 11 && vid.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            Some(format!("https://www.youtube.com/watch?v={vid}"))
        } else {
            None
        }
    } else {
        None
    };

    let effective_canonical = canonical.or(derived_video_url);

    let referer = if let Some(ref_url) = msg.referer {
        let validated = validate_url(&ref_url)?;
        if valid_url.contains("googlevideo.com") && !validated.contains("/watch?v=") && !validated.contains("/shorts/") {
            effective_canonical.clone().or(Some(validated))
        } else {
            Some(validated)
        }
    } else {
        effective_canonical.clone()
    };

    let mut filtered_headers = HashMap::new();
    if let Some(headers) = msg.request_headers {
        let allowed = ["cookie", "authorization", "user-agent", "origin", "referer"];
        for (k, v) in headers {
            let lower_key = k.to_ascii_lowercase();
            if allowed.contains(&lower_key.as_str()) {
                filtered_headers.insert(lower_key, v);
            }
        }
    }

    let urls = if let Some(raw_urls) = msg.urls {
        let mut validated = Vec::new();
        for u in raw_urls {
            validated.push(validate_url(&u)?);
        }
        Some(validated)
    } else {
        None
    };

    Ok(ValidatedBrowserRequest {
        action: msg.action,
        download_id,
        url: valid_url,
        canonical_url: effective_canonical,
        youtube_format: msg.youtube_format,
        sabr,
        start_now: msg.start_now.unwrap_or(false),
        audio_url,
        output_format: msg.output_format,
        urls,
        filename,
        referer,
        headers: filtered_headers,
    })
}
