// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-08
// Reason: feat(transfer): add FTP transfers with passive mode, authentication, and resume (Closes #28)
// Reason: feat(transfer): recover interrupted transfers and refresh expired links (Closes #3)
// Reason: feat(mms): add MMS streaming support or explicit unsupported guidance (Closes #35)
// Description: Filename sanitization, reserved Windows name guards, and generic name detection helpers.

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
    if url.scheme() == "ftp" && (!url.username().is_empty() || url.password().is_some()) {
        let host = url.host_str().unwrap_or_default();
        let port_part = match url.port() {
            Some(port) if port != 21 => format!(":{port}"),
            _ => String::new(),
        };
        let path = url.path();
        return format!("ftp://******:******@{host}{port_part}{path}");
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}

pub fn sanitize_diagnostic_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let words = text.split_whitespace();
    let mut first = true;
    for word in words {
        if !first {
            result.push(' ');
        }
        first = false;
        if word.starts_with("http://")
            || word.starts_with("https://")
            || word.starts_with("ftp://")
            || word.starts_with("mms://")
            || word.starts_with("mmsh://")
            || word.starts_with("mmst://")
            || word.starts_with("mmsu://")
        {
            let trailing_punct_len = word
                .chars()
                .rev()
                .take_while(|c| matches!(c, ')' | ']' | '}' | ',' | ';' | ':' | '.' | '"' | '\''))
                .count();
            let (url_part, punct) = word.split_at(word.len() - trailing_punct_len);
            result.push_str(&redact_download_url(url_part));
            result.push_str(punct);
        } else {
            result.push_str(word);
        }
    }
    result
}

pub fn is_generic_filename(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    let stem = std::path::Path::new(&lower)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&lower);
    matches!(
        stem,
        "" | "watch" | "video" | "videoplayback" | "media" | "stream" | "download" | "file" | "index"
    )
}

fn is_valid_disposition_filename(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    let stem = std::path::Path::new(&lower)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&lower);
    !matches!(
        stem,
        "" | "watch" | "video" | "videoplayback" | "media" | "stream" | "download" | "file" | "index"
    )
}

pub fn percent_decode(input: &str) -> String {
    percent_encoding_decode(input).unwrap_or_else(|_| input.to_string())
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
                        if is_valid_disposition_filename(&sanitized) {
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
            if is_valid_disposition_filename(&sanitized) {
                return Some(sanitized);
            }
        }
    }
    None
}

pub fn extension_for_mime_type(mime: &str) -> Option<&'static str> {
    let clean_mime = mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match clean_mime.as_str() {
        // Video formats
        "video/x-matroska" | "video/mkv" => Some("mkv"),
        "video/mp4" => Some("mp4"),
        "video/webm" => Some("webm"),
        "video/quicktime" => Some("mov"),
        "video/x-msvideo" | "video/avi" => Some("avi"),
        "video/x-flv" => Some("flv"),
        "video/mpeg" => Some("mpeg"),
        "video/3gpp" => Some("3gp"),
        "video/mp2t" => Some("ts"),
        // Audio formats
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/aac" => Some("aac"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "audio/ogg" | "audio/vorbis" => Some("ogg"),
        "audio/wav" | "audio/x-wav" => Some("wav"),
        "audio/x-m4a" | "audio/m4a" => Some("m4a"),
        "audio/opus" => Some("opus"),
        // Documents and archives
        "application/pdf" => Some("pdf"),
        "application/zip" => Some("zip"),
        "application/x-rar-compressed" | "application/vnd.rar" | "application/x-rar" => Some("rar"),
        "application/x-7z-compressed" => Some("7z"),
        "application/x-tar" => Some("tar"),
        "application/gzip" | "application/x-gzip" => Some("gz"),
        "application/json" => Some("json"),
        "application/xml" | "text/xml" => Some("xml"),
        "text/html" => Some("html"),
        "text/plain" => Some("txt"),
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        // Generic binary streams should NOT be assigned an extension
        "application/octet-stream" | "" => None,
        // Fallback to mime_guess crate
        other => mime_guess::get_mime_extensions_str(other).and_then(|exts| exts.first().copied()),
    }
}

pub fn resolve_response_filename(
    current_name: &str,
    content_disposition: Option<&str>,
    content_type: Option<&str>,
) -> Option<String> {
    let cd_filename = content_disposition.and_then(parse_content_disposition_filename);
    let mime_ext = content_type.and_then(extension_for_mime_type);

    if let Some(cd_name) = cd_filename {
        let has_ext = std::path::Path::new(&cd_name)
            .extension()
            .is_some_and(|e| !e.is_empty());
        if has_ext {
            return Some(cd_name);
        } else if let Some(ext) = mime_ext {
            return Some(format!("{cd_name}.{ext}"));
        } else {
            return Some(cd_name);
        }
    }

    // No Content-Disposition filename found.
    let current_has_ext = std::path::Path::new(current_name)
        .extension()
        .is_some_and(|e| !e.is_empty());

    if !current_has_ext {
        if let Some(ext) = mime_ext {
            return Some(format!("{current_name}.{ext}"));
        }
    } else if is_generic_filename(current_name) || current_name.starts_with("download_") {
        if let Some(ext) = mime_ext {
            let stem = std::path::Path::new(current_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(current_name);
            return Some(format!("{stem}.{ext}"));
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
