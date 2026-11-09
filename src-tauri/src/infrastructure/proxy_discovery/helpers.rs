// Created: 2026-11-10
// Reason: refactor: modularize proxy discovery into platform and helper modules
// Description: URL parsing and host/port extraction utilities for proxy endpoints.

use url::Url;

pub fn parse_host_port(endpoint: &str) -> Option<(String, u16)> {
    let trimmed = endpoint.trim().trim_start_matches("http://").trim_start_matches("https://");
    let trimmed = trimmed.trim_end_matches('/');
    if let Some((h, p)) = trimmed.split_once(':') {
        let port = p.parse::<u16>().ok()?;
        Some((h.to_string(), port))
    } else if !trimmed.is_empty() {
        Some((trimmed.to_string(), 8080))
    } else {
        None
    }
}

/// Parses proxy URL, redacting and never returning passwords.
/// Returns (host, port, credentials_available, username)
pub fn parse_proxy_url(raw: &str) -> Option<(String, u16, bool, Option<String>)> {
    let mut raw_with_scheme = raw.to_string();
    if !raw.contains("://") {
        raw_with_scheme = format!("http://{raw}");
    }

    let url = Url::parse(&raw_with_scheme).ok()?;
    let host = url.host_str()?.to_string();
    let port = url.port().unwrap_or_else(|| {
        match url.scheme() {
            "socks5" | "socks5h" => 1080,
            "https" => 8443,
            "ftp" => 2121,
            _ => 8080,
        }
    });

    let has_auth = !url.username().is_empty() || url.password().is_some();
    let username = if !url.username().is_empty() {
        Some(url.username().to_string())
    } else {
        None
    };

    Some((host, port, has_auth, username))
}
