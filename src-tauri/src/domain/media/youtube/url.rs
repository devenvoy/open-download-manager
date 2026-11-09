// Created: 2026-11-05
// Reason: feat: add media handling for HLS, FFmpeg, yt-dlp, and stream progress
// Last updated: 2026-11-10
// Reason: refactor: modularize youtube domain module into focused submodules
// Description: YouTube video URL parser, ID extractor, and format identifier validator.

use url::Url;

pub fn youtube_url(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    let video_id = if host == "youtu.be" {
        url.path().strip_prefix('/')?.to_owned()
    } else if matches!(
        host.as_str(),
        "youtube.com" | "www.youtube.com" | "m.youtube.com"
    ) {
        if url.path() == "/watch" {
            url.query_pairs().find(|(k, _)| k == "v")?.1.to_string()
        } else if let Some(stripped) = url
            .path()
            .strip_prefix("/shorts/")
            .or_else(|| url.path().strip_prefix("/embed/"))
        {
            stripped.split('/').next()?.to_string()
        } else {
            return None;
        }
    } else {
        return None;
    };

    let is_valid_id = video_id.len() == 11
        && video_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if is_valid_id {
        Some(format!("https://www.youtube.com/watch?v={video_id}"))
    } else {
        None
    }
}

pub(crate) fn valid_format_id(format_id: &str) -> bool {
    !format_id.is_empty()
        && format_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
}
