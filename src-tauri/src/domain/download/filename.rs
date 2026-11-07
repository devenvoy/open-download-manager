use url::Url;

use super::download_policy::{
    DEFAULT_DOWNLOAD_FILENAME, INVALID_URL_DISPLAY_TEXT, MAX_DOWNLOAD_FILENAME_CHARACTERS,
    WINDOWS_RESERVED_BASENAMES,
};

pub fn sanitize_filename(value: &str) -> String {
    if value.trim_matches(['.', ' ']).is_empty() {
        return String::new();
    }

    let basename = value
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(['.', ' ']);
    let safe_input = if WINDOWS_RESERVED_BASENAMES.contains(&basename.to_ascii_lowercase().as_str()) {
        format!("download_{value}")
    } else {
        value.to_string()
    };

    let options = sanitize_filename::Options {
        windows: true,
        truncate: true,
        replacement: "_",
    };
    let mut filename = sanitize_filename::sanitize_with_options(&safe_input, options);
    while matches!(filename.chars().last(), Some('.' | ' ')) {
        filename.pop();
    }
    truncate_filename(&mut filename);
    filename
}

pub fn redact_download_url(value: &str) -> String {
    let Ok(mut url) = Url::parse(value) else {
        return INVALID_URL_DISPLAY_TEXT.to_owned();
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}

pub fn is_generic_filename(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    // Strip extension to normalize: "video.mp4" -> "video"
    let stem = std::path::Path::new(&lower)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&lower);
    matches!(
        stem,
        "" | "watch" | "video" | "videoplayback" | "media" | "stream" | "download" | "file" | "index"
    )
}

fn percent_encoding_decode(input: &str) -> Result<String, ()> {
    let mut bytes = Vec::new();
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next().ok_or(())?;
            let h2 = chars.next().ok_or(())?;
            let hex_str = [h1, h2];
            let byte = u8::from_str_radix(std::str::from_utf8(&hex_str).map_err(|_| ())?, 16).map_err(|_| ())?;
            bytes.push(byte);
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).map_err(|_| ())
}

pub fn parse_content_disposition_filename(header_value: &str) -> Option<String> {
    // RFC 6266 / RFC 5987 parser
    // First look for filename*=utf-8''...
    for part in header_value.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename*=") {
            let rest = rest.trim().trim_matches('"');
            if let Some(idx) = rest.find("''") {
                let encoding = &rest[..idx];
                let encoded_filename = &rest[idx + 2..];
                if encoding.eq_ignore_ascii_case("utf-8") {
                    if let Ok(decoded) = percent_encoding_decode(encoded_filename) {
                        let sanitized = sanitize_filename(&decoded);
                        if !sanitized.is_empty() && !is_generic_filename(&sanitized) {
                            return Some(sanitized);
                        }
                    }
                }
            }
        }
    }
    // Then look for filename="value" or filename=value
    for part in header_value.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename=") {
            let raw = rest.trim().trim_matches('"');
            let sanitized = sanitize_filename(raw);
            if !sanitized.is_empty() && !is_generic_filename(&sanitized) {
                return Some(sanitized);
            }
        }
    }
    None
}

pub fn filename_from_url(url: &Url) -> String {
    let host = url.host_str().unwrap_or("");
    let is_youtube = host.contains("youtube.com") || host.contains("youtu.be") || host.contains("googlevideo.com");
    if is_youtube {
        if let Some((_, v)) = url.query_pairs().find(|(k, _)| k == "v" || k == "docid" || k == "id") {
            let v_str = v.trim();
            if v_str.len() == 11 && v_str.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return format!("YouTube Video {v_str}.mp4");
            }
        }
        return "YouTube Video.mp4".to_string();
    }

    let raw_name = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|segment| !segment.is_empty())
        .map(sanitize_filename);

    let name = match raw_name {
        Some(name) if !name.is_empty() && !is_generic_filename(&name) => name,
        _ => {
            // Check query params (e.g. ?filename= or ?title= or ?file= or ?name=)
            let query_name = url.query_pairs().find_map(|(k, v)| {
                let lower_k = k.to_ascii_lowercase();
                if lower_k == "filename" || lower_k == "title" || lower_k == "file" || lower_k == "name" {
                    let sanitized = sanitize_filename(&v);
                    if !sanitized.is_empty() && !is_generic_filename(&sanitized) {
                        return Some(sanitized);
                    }
                }
                None
            });
            query_name.unwrap_or_else(|| {
                if !host.is_empty() {
                    let clean_host = host.trim_start_matches("www.").replace('.', "_");
                    format!("download_{clean_host}")
                } else {
                    DEFAULT_DOWNLOAD_FILENAME.to_owned()
                }
            })
        }
    };

    if name.to_ascii_lowercase().ends_with(".m3u8") {
        let base = &name[..name.len() - 5];
        format!("{base}.ts")
    } else {
        name
    }
}

fn truncate_filename(filename: &mut String) {
    if filename.chars().count() > MAX_DOWNLOAD_FILENAME_CHARACTERS {
        *filename = filename
            .chars()
            .take(MAX_DOWNLOAD_FILENAME_CHARACTERS)
            .collect();
    }
}
