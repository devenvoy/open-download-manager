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

    let name = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|segment| !segment.is_empty())
        .map(sanitize_filename)
        .filter(|filename| !filename.is_empty())
        .unwrap_or_else(|| DEFAULT_DOWNLOAD_FILENAME.to_owned());

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
