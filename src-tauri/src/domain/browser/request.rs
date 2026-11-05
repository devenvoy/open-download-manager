use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::domain::download::sanitize_filename;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserIncomingMessage {
    pub action: String,
    pub url: Option<String>,
    pub youtube_format: Option<String>,
    pub start_now: Option<bool>,
    pub audio_url: Option<String>,
    pub output_format: Option<String>,
    pub urls: Option<Vec<String>>,
    pub filename: Option<String>,
    pub referer: Option<String>,
    #[serde(rename = "requestHeaders")]
    pub request_headers: Option<HashMap<String, String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedBrowserRequest {
    pub action: String,
    pub url: String,
    pub youtube_format: Option<String>,
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
    if !matches!(msg.action.as_str(), "download" | "formats") {
        return Err("Unsupported native action.".to_owned());
    }

    let raw_url = msg.url.ok_or_else(|| "URL is required.".to_owned())?;
    let valid_url = validate_url(&raw_url)?;

    if msg.action == "formats" {
        let parsed = Url::parse(&valid_url).map_err(|_| "Invalid URL.".to_owned())?;
        let host = parsed.host_str().unwrap_or("");
        if !matches!(host, "www.youtube.com" | "youtube.com" | "m.youtube.com")
            || parsed.path() != "/watch"
        {
            return Err("A YouTube watch URL is required for formats action.".to_owned());
        }
        let video_id = parsed
            .query_pairs()
            .find(|(k, _)| k == "v")
            .map(|(_, v)| v.to_string())
            .ok_or_else(|| "A YouTube watch URL is required.".to_owned())?;
        if video_id.len() != 11
            || !video_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err("A YouTube watch URL is required.".to_owned());
        }
        return Ok(ValidatedBrowserRequest {
            action: "formats".to_owned(),
            url: format!("https://www.youtube.com/watch?v={video_id}"),
            youtube_format: None,
            start_now: false,
            audio_url: None,
            output_format: None,
            urls: None,
            filename: None,
            referer: None,
            headers: HashMap::new(),
        });
    }

    if let Some(format) = &msg.output_format {
        if !matches!(format.as_str(), "mp4" | "mp3") {
            return Err("Invalid output format.".to_owned());
        }
    }

    let filename = if let Some(raw_name) = msg.filename {
        if raw_name.len() > 255 || raw_name == "." || raw_name == ".." {
            return Err("Invalid filename.".to_owned());
        }
        let sanitized = sanitize_filename(&raw_name);
        if sanitized.is_empty() {
            return Err("Invalid filename.".to_owned());
        }
        Some(sanitized)
    } else {
        None
    };

    let audio_url = if let Some(a_url) = msg.audio_url {
        Some(validate_url(&a_url)?)
    } else {
        None
    };

    let referer = if let Some(ref_url) = msg.referer {
        Some(validate_url(&ref_url)?)
    } else {
        None
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
        action: "download".to_owned(),
        url: valid_url,
        youtube_format: msg.youtube_format,
        start_now: msg.start_now.unwrap_or(false),
        audio_url,
        output_format: msg.output_format,
        urls,
        filename,
        referer,
        headers: filtered_headers,
    })
}
