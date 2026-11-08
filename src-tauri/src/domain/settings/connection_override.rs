// Created: 2026-11-08
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Last updated: 2026-11-08
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Description: Per-domain connection count exception rule and host pattern matching evaluator.

use serde::{Deserialize, Serialize};

use super::{policy::MAX_CONNECTIONS_PER_DOWNLOAD, validation_error::SettingsValidationError};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionOverride {
    pub host_pattern: String,
    pub connections: u8,
}

impl ConnectionOverride {
    pub fn new(host_pattern: impl Into<String>, connections: u8) -> Self {
        Self {
            host_pattern: host_pattern.into(),
            connections,
        }
    }

    /// Normalizes the host pattern: trims whitespace, strips schemes and ports, and lowers case.
    pub fn clean_pattern(&self) -> String {
        clean_host_pattern(&self.host_pattern)
    }

    /// Validates the override rule.
    pub fn validate(&self) -> Result<(), SettingsValidationError> {
        let raw = self.host_pattern.trim();
        if raw.is_empty() {
            return Err(SettingsValidationError::new(
                "connectionOverride hostPattern cannot be empty.",
            ));
        }

        if raw.contains('/') || raw.contains('@') {
            return Err(SettingsValidationError::new(format!(
                "connectionOverride hostPattern '{raw}' must not contain paths or credentials."
            )));
        }

        let pattern = self.clean_pattern();
        if pattern.is_empty() {
            return Err(SettingsValidationError::new(
                "connectionOverride hostPattern cannot be empty.",
            ));
        }

        if pattern.contains(':') {
            return Err(SettingsValidationError::new(format!(
                "connectionOverride hostPattern '{pattern}' must not contain ports."
            )));
        }

        if !(1..=MAX_CONNECTIONS_PER_DOWNLOAD).contains(&self.connections) {
            return Err(SettingsValidationError::new(format!(
                "connectionOverride connections must be between 1 and {MAX_CONNECTIONS_PER_DOWNLOAD}."
            )));
        }

        // Validate wildcard rules
        if let Some(rest) = pattern.strip_prefix("*.") {
            if rest.is_empty() || rest.contains('*') {
                return Err(SettingsValidationError::new(format!(
                    "Invalid wildcard pattern '{pattern}'. Wildcards must be in the form '*.domain.tld'."
                )));
            }
        } else if pattern.contains('*') {
            return Err(SettingsValidationError::new(format!(
                "Invalid pattern '{pattern}'. Wildcards are only supported as a leading '*.' prefix."
            )));
        }

        Ok(())
    }

    /// Matches this override pattern against a target host.
    /// Returns a match score: None if not matching, Some(score) if matching (higher score = more specific).
    pub fn match_score(&self, target_host: &str) -> Option<usize> {
        let target = clean_host(target_host);
        let pattern = self.clean_pattern();

        if target.is_empty() || pattern.is_empty() {
            return None;
        }

        // 1. Exact match (highest precedence)
        if target == pattern {
            return Some(10_000 + pattern.len());
        }

        // 2. Wildcard prefix matching: "*.example.com"
        if let Some(domain_suffix) = pattern.strip_prefix("*.") {
            // Match subdomains (e.g., "cdn.example.com" ends with ".example.com")
            if target.ends_with(domain_suffix) {
                let prefix_len = target.len() - domain_suffix.len();
                if prefix_len > 0 && target.as_bytes()[prefix_len - 1] == b'.' {
                    // Subdomain match: score proportional to pattern length
                    return Some(1_000 + pattern.len());
                }
            }
            // Also match the apex domain itself if pattern was *.example.com
            if target == domain_suffix {
                return Some(500 + pattern.len());
            }
        }

        None
    }
}

/// Cleans and normalizes a host pattern.
pub fn clean_host_pattern(raw: &str) -> String {
    let mut pattern = raw.trim().to_lowercase();
    if let Some(stripped) = pattern.strip_prefix("http://") {
        pattern = stripped.to_string();
    } else if let Some(stripped) = pattern.strip_prefix("https://") {
        pattern = stripped.to_string();
    }

    // Strip trailing slash or path if any
    if let Some(idx) = pattern.find('/') {
        pattern.truncate(idx);
    }

    // Strip port if any (unless it's an IPv6 bracket)
    if !pattern.starts_with('[') {
        if let Some(idx) = pattern.find(':') {
            pattern.truncate(idx);
        }
    }

    pattern
}

/// Cleans and normalizes a target hostname for matching.
pub fn clean_host(raw: &str) -> String {
    clean_host_pattern(raw)
}

/// Resolves the effective connection count for a given host among a set of overrides.
/// Follows safe precedence: exact host > most specific wildcard > global default.
pub fn resolve_connections_for_host(
    overrides: &[ConnectionOverride],
    host: &str,
    default_connections: u8,
) -> u8 {
    let mut best_match: Option<(usize, u8)> = None;

    for rule in overrides {
        if let Some(score) = rule.match_score(host) {
            match best_match {
                None => best_match = Some((score, rule.connections)),
                Some((best_score, _)) if score > best_score => {
                    best_match = Some((score, rule.connections));
                }
                _ => {}
            }
        }
    }

    best_match.map(|(_, conns)| conns).unwrap_or(default_connections)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_host_matching() {
        let rule = ConnectionOverride::new("legacy.example.test", 1);
        assert!(rule.match_score("legacy.example.test").is_some());
        assert!(rule.match_score("LEGACY.EXAMPLE.TEST").is_some());
        assert!(rule.match_score("other.example.test").is_none());
    }

    #[test]
    fn test_wildcard_matching() {
        let rule = ConnectionOverride::new("*.example.test", 2);
        assert!(rule.match_score("cdn.example.test").is_some());
        assert!(rule.match_score("sub.cdn.example.test").is_some());
        assert!(rule.match_score("example.test").is_some());
        assert!(rule.match_score("notexample.test").is_none());
        assert!(rule.match_score("example.org").is_none());
    }

    #[test]
    fn test_precedence_exact_beats_wildcard() {
        let overrides = vec![
            ConnectionOverride::new("*.example.test", 2),
            ConnectionOverride::new("special.example.test", 1),
        ];

        assert_eq!(
            resolve_connections_for_host(&overrides, "special.example.test", 8),
            1
        );
        assert_eq!(
            resolve_connections_for_host(&overrides, "cdn.example.test", 8),
            2
        );
        assert_eq!(
            resolve_connections_for_host(&overrides, "other.com", 8),
            8
        );
    }

    #[test]
    fn test_precedence_more_specific_wildcard_wins() {
        let overrides = vec![
            ConnectionOverride::new("*.example.test", 4),
            ConnectionOverride::new("*.cdn.example.test", 1),
        ];

        assert_eq!(
            resolve_connections_for_host(&overrides, "node1.cdn.example.test", 8),
            1
        );
        assert_eq!(
            resolve_connections_for_host(&overrides, "api.example.test", 8),
            4
        );
    }

    #[test]
    fn test_validation() {
        assert!(ConnectionOverride::new("valid.com", 1).validate().is_ok());
        assert!(ConnectionOverride::new("*.valid.com", 32).validate().is_ok());
        assert!(ConnectionOverride::new("", 1).validate().is_err());
        assert!(ConnectionOverride::new("valid.com", 0).validate().is_err());
        assert!(ConnectionOverride::new("valid.com", 33).validate().is_err());
        assert!(ConnectionOverride::new("valid.com/path", 1).validate().is_err());
        assert!(ConnectionOverride::new("user:pass@valid.com", 1).validate().is_err());
        assert!(ConnectionOverride::new("foo.*.bar.com", 1).validate().is_err());
    }
}
