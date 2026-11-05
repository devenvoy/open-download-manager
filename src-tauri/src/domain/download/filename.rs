use url::Url;

use super::download_policy::{
    DEFAULT_DOWNLOAD_FILENAME, INVALID_FILENAME_CHARACTERS, INVALID_URL_DISPLAY_TEXT,
    MAX_DOWNLOAD_FILENAME_CHARACTERS, WINDOWS_RESERVED_BASENAMES,
};

pub fn sanitize_filename(value: &str) -> String {
    let mut filename: String = value
        .chars()
        .map(|character| {
            if character.is_control() || INVALID_FILENAME_CHARACTERS.contains(&character) {
                '_'
            } else {
                character
            }
        })
        .collect();
    while matches!(filename.chars().last(), Some('.' | ' ')) {
        filename.pop();
    }
    truncate_filename(&mut filename);

    let basename = filename
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(['.', ' ']);
    if WINDOWS_RESERVED_BASENAMES.contains(&basename.to_ascii_lowercase().as_str()) {
        filename.insert_str(0, "download_");
        truncate_filename(&mut filename);
    }
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
    url.path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|segment| !segment.is_empty())
        .map(sanitize_filename)
        .filter(|filename| !filename.is_empty())
        .unwrap_or_else(|| DEFAULT_DOWNLOAD_FILENAME.to_owned())
}

fn truncate_filename(filename: &mut String) {
    if filename.chars().count() > MAX_DOWNLOAD_FILENAME_CHARACTERS {
        *filename = filename
            .chars()
            .take(MAX_DOWNLOAD_FILENAME_CHARACTERS)
            .collect();
    }
}
