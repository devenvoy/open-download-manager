// Created: 2026-11-05
// Reason: feat: add media handling for HLS, FFmpeg, yt-dlp, and stream progress
// Last updated: 2026-11-10
// Reason: refactor: modularize youtube domain module into focused submodules
// Description: YouTube and yt-dlp error classifier mapping raw process errors to user-friendly messages.

pub fn youtube_error(raw: &str) -> (String, u16, &'static str) {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("http error 429") || lower.contains("too many requests") {
        return (
            "YouTube is temporarily refusing requests (too many requests). Wait before trying again. Repeated clicks will not help; this app does not currently support YouTube sign-in verification."
                .to_owned(),
            429,
            "YOUTUBE_RATE_LIMIT",
        );
    }
    if lower.contains("confirm you are not a bot")
        || lower.contains("sign in")
        || lower.contains("login required")
        || lower.contains("cookies-from-browser")
    {
        return (
            "YouTube requires sign-in or browser verification for this request. The downloader cannot currently complete that verification."
                .to_owned(),
            422,
            "YOUTUBE_VERIFICATION",
        );
    }

    let redacted_lines: Vec<String> = raw
        .lines()
        .map(|line| {
            line.split_whitespace()
                .map(|token| {
                    if token.starts_with("http://") || token.starts_with("https://") {
                        "[URL]"
                    } else {
                        token
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    let error_line = redacted_lines
        .iter()
        .rev()
        .find(|line| line.starts_with("ERROR:"))
        .map(|s| s.as_str())
        .unwrap_or(raw);

    let message: String = error_line.trim().chars().take(700).collect();
    let final_message = if message.is_empty() {
        "Video download failed.".to_owned()
    } else {
        message
    };
    (final_message, 400, "YOUTUBE_ERROR")
}
