// Created: 2026-11-08
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Last updated: 2026-11-08
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Description: Pure domain models, policies, and validation rules for clipboard link observation.

use serde::{Deserialize, Serialize};
use url::Url;

use super::download::filename_from_url;

pub const MAX_CLIPBOARD_TEXT_LENGTH: usize = 4096;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardMonitoringSettings {
    pub enabled: bool,
    #[serde(default)]
    pub auto_enqueue: bool,
}

impl Default for ClipboardMonitoringSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_enqueue: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardCandidate {
    pub url: String,
    pub host: String,
    pub filename: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardRejectionReason {
    Disabled,
    Oversized,
    EmptyOrWhitespace,
    MultilineOrWhitespace,
    NotAUrl,
    UnsupportedScheme,
    ContainsCredentials,
    Duplicate,
}

pub struct ClipboardPolicy;

impl ClipboardPolicy {
    pub fn evaluate(
        text: &str,
        settings: &ClipboardMonitoringSettings,
        last_seen_url: Option<&str>,
    ) -> Result<ClipboardCandidate, ClipboardRejectionReason> {
        if !settings.enabled {
            return Err(ClipboardRejectionReason::Disabled);
        }

        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(ClipboardRejectionReason::EmptyOrWhitespace);
        }

        if text.len() > MAX_CLIPBOARD_TEXT_LENGTH {
            return Err(ClipboardRejectionReason::Oversized);
        }

        if trimmed.lines().count() > 1 || trimmed.chars().any(|c| c.is_whitespace()) {
            return Err(ClipboardRejectionReason::MultilineOrWhitespace);
        }

        let parsed = Url::parse(trimmed).map_err(|_| ClipboardRejectionReason::NotAUrl)?;
        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" {
            return Err(ClipboardRejectionReason::UnsupportedScheme);
        }

        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(ClipboardRejectionReason::ContainsCredentials);
        }

        let host = match parsed.host_str() {
            Some(h) if !h.is_empty() => h.to_ascii_lowercase(),
            _ => return Err(ClipboardRejectionReason::NotAUrl),
        };

        let normalized_url = parsed.to_string();
        if let Some(last) = last_seen_url {
            if last == normalized_url {
                return Err(ClipboardRejectionReason::Duplicate);
            }
        }

        let filename = filename_from_url(&parsed);

        Ok(ClipboardCandidate {
            url: normalized_url,
            host,
            filename,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_by_default() {
        let settings = ClipboardMonitoringSettings::default();
        assert!(!settings.enabled);
        assert!(!settings.auto_enqueue);

        let result = ClipboardPolicy::evaluate("https://example.com/archive.zip", &settings, None);
        assert_eq!(result, Err(ClipboardRejectionReason::Disabled));
    }

    #[test]
    fn test_valid_http_and_https_links() {
        let settings = ClipboardMonitoringSettings {
            enabled: true,
            auto_enqueue: false,
        };

        let result = ClipboardPolicy::evaluate("https://example.com/downloads/setup.exe", &settings, None);
        assert!(result.is_ok());
        let candidate = result.unwrap();
        assert_eq!(candidate.url, "https://example.com/downloads/setup.exe");
        assert_eq!(candidate.host, "example.com");
        assert_eq!(candidate.filename, "setup.exe");

        let http_result = ClipboardPolicy::evaluate("  http://myhost.org/path/file.tar.gz  ", &settings, None);
        assert!(http_result.is_ok());
        let http_cand = http_result.unwrap();
        assert_eq!(http_cand.url, "http://myhost.org/path/file.tar.gz");
        assert_eq!(http_cand.host, "myhost.org");
        assert_eq!(http_cand.filename, "file.tar.gz");
    }

    #[test]
    fn test_rejects_embedded_credentials() {
        let settings = ClipboardMonitoringSettings {
            enabled: true,
            auto_enqueue: false,
        };

        let with_user = ClipboardPolicy::evaluate("https://admin@example.com/file.zip", &settings, None);
        assert_eq!(with_user, Err(ClipboardRejectionReason::ContainsCredentials));

        let with_pass = ClipboardPolicy::evaluate("http://admin:secret123@example.com/file.zip", &settings, None);
        assert_eq!(with_pass, Err(ClipboardRejectionReason::ContainsCredentials));
    }

    #[test]
    fn test_rejects_unsupported_schemes() {
        let settings = ClipboardMonitoringSettings {
            enabled: true,
            auto_enqueue: false,
        };

        assert_eq!(
            ClipboardPolicy::evaluate("ftp://example.com/file.zip", &settings, None),
            Err(ClipboardRejectionReason::UnsupportedScheme)
        );
        assert_eq!(
            ClipboardPolicy::evaluate("javascript:alert(1)", &settings, None),
            Err(ClipboardRejectionReason::UnsupportedScheme)
        );
        assert_eq!(
            ClipboardPolicy::evaluate("file:///etc/passwd", &settings, None),
            Err(ClipboardRejectionReason::UnsupportedScheme)
        );
    }

    #[test]
    fn test_rejects_oversized_and_non_url_text() {
        let settings = ClipboardMonitoringSettings {
            enabled: true,
            auto_enqueue: false,
        };

        let huge = "https://example.com/".to_string() + &"a".repeat(5000);
        assert_eq!(
            ClipboardPolicy::evaluate(&huge, &settings, None),
            Err(ClipboardRejectionReason::Oversized)
        );

        assert_eq!(
            ClipboardPolicy::evaluate("just some ordinary text copied to clipboard", &settings, None),
            Err(ClipboardRejectionReason::MultilineOrWhitespace)
        );

        assert_eq!(
            ClipboardPolicy::evaluate("https://example.com/file1 https://example.com/file2", &settings, None),
            Err(ClipboardRejectionReason::MultilineOrWhitespace)
        );

        assert_eq!(
            ClipboardPolicy::evaluate("line1\nhttps://example.com/file.zip", &settings, None),
            Err(ClipboardRejectionReason::MultilineOrWhitespace)
        );
    }

    #[test]
    fn test_duplicate_suppression() {
        let settings = ClipboardMonitoringSettings {
            enabled: true,
            auto_enqueue: false,
        };

        let url = "https://example.com/file.zip";
        let first = ClipboardPolicy::evaluate(url, &settings, None);
        assert!(first.is_ok());

        let second = ClipboardPolicy::evaluate(url, &settings, Some("https://example.com/file.zip"));
        assert_eq!(second, Err(ClipboardRejectionReason::Duplicate));
    }
}
