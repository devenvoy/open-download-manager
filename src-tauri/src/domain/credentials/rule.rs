use serde::{Deserialize, Serialize};

pub const DEFAULT_HTTP_PORT: u16 = 80;
pub const DEFAULT_HTTPS_PORT: u16 = 443;
pub const DEFAULT_FTP_PORT: u16 = 21;

pub fn default_port_for_scheme(scheme: &str) -> Option<u16> {
    match scheme {
        "http" => Some(DEFAULT_HTTP_PORT),
        "https" => Some(DEFAULT_HTTPS_PORT),
        "ftp" => Some(DEFAULT_FTP_PORT),
        _ => None,
    }
}

/// A stored site/path authentication rule for automated basic authentication.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRule {
    pub id: String,
    pub scheme: String,
    pub host: String,
    pub port: Option<u16>,
    pub path_prefix: String,
    pub username: String,
    pub secret: String,
}

impl CredentialRule {
    pub fn new(
        id: impl Into<String>,
        scheme: impl AsRef<str>,
        host: impl AsRef<str>,
        port: Option<u16>,
        path_prefix: impl AsRef<str>,
        username: impl AsRef<str>,
        secret: impl Into<String>,
    ) -> Result<Self, String> {
        let id_str = id.into().trim().to_string();
        if id_str.is_empty() {
            return Err("Rule ID cannot be empty.".to_string());
        }

        let scheme_norm = scheme.as_ref().trim().to_ascii_lowercase();
        if !matches!(scheme_norm.as_str(), "http" | "https" | "ftp") {
            return Err(format!(
                "Unsupported scheme '{scheme_norm}'. Only http, https, and ftp are allowed."
            ));
        }

        let host_norm = host.as_ref().trim().to_ascii_lowercase();
        if host_norm.is_empty() {
            return Err("Host cannot be empty.".to_string());
        }
        if host_norm.contains('/') || host_norm.contains(':') || host_norm.contains('@') {
            return Err("Host must not contain slashes, colons, or credentials.".to_string());
        }

        let normalized_port = match port {
            Some(p) => {
                if p == 0 {
                    return Err("Port must be between 1 and 65535.".to_string());
                }
                if default_port_for_scheme(&scheme_norm) == Some(p) {
                    None
                } else {
                    Some(p)
                }
            }
            None => None,
        };

        let raw_path = path_prefix.as_ref().trim();
        let path_norm = normalize_path_prefix(raw_path);

        let user_norm = username.as_ref().trim().to_string();
        if user_norm.is_empty() {
            return Err("Username cannot be empty.".to_string());
        }

        let secret_str = secret.into();
        if secret_str.is_empty() {
            return Err("Password/secret cannot be empty.".to_string());
        }

        Ok(Self {
            id: id_str,
            scheme: scheme_norm,
            host: host_norm,
            port: normalized_port,
            path_prefix: path_norm,
            username: user_norm,
            secret: secret_str,
        })
    }

    /// Redacts secret for UI display or diagnostics.
    pub fn redacted(&self) -> Self {
        Self {
            id: self.id.clone(),
            scheme: self.scheme.clone(),
            host: self.host.clone(),
            port: self.port,
            path_prefix: self.path_prefix.clone(),
            username: self.username.clone(),
            secret: "******".to_string(),
        }
    }

    /// Computes deterministic matching signature (scheme, host, port, path_prefix).
    pub fn signature(&self) -> (String, String, Option<u16>, String) {
        (
            self.scheme.clone(),
            self.host.clone(),
            self.port,
            self.path_prefix.clone(),
        )
    }
}

/// Normalizes path prefix with strict segment boundaries:
/// - Leading slash is guaranteed.
/// - Duplicate consecutive slashes are consolidated.
/// - Trailing slash is ensured for non-root paths so segment boundary matching is strict.
pub fn normalize_path_prefix(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }

    let mut result = String::with_capacity(trimmed.len() + 2);
    if !trimmed.starts_with('/') {
        result.push('/');
    }

    let mut last_was_slash = false;
    for ch in trimmed.chars() {
        if ch == '/' {
            if !last_was_slash {
                result.push('/');
                last_was_slash = true;
            }
        } else {
            result.push(ch);
            last_was_slash = false;
        }
    }

    if !result.ends_with('/') {
        result.push('/');
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_prefix() {
        assert_eq!(normalize_path_prefix(""), "/");
        assert_eq!(normalize_path_prefix("/"), "/");
        assert_eq!(normalize_path_prefix("team"), "/team/");
        assert_eq!(normalize_path_prefix("/team"), "/team/");
        assert_eq!(normalize_path_prefix("/team/"), "/team/");
        assert_eq!(normalize_path_prefix("///team///project///"), "/team/project/");
    }

    #[test]
    fn test_credential_rule_normalization() {
        let rule = CredentialRule::new(
            "rule-1",
            "HTTPS",
            "Example.COM",
            Some(443), // Default https port -> normalizes to None
            "/team/docs",
            "alice ",
            "secret123",
        )
        .unwrap();

        assert_eq!(rule.scheme, "https");
        assert_eq!(rule.host, "example.com");
        assert_eq!(rule.port, None);
        assert_eq!(rule.path_prefix, "/team/docs/");
        assert_eq!(rule.username, "alice");
        assert_eq!(rule.secret, "secret123");
    }

    #[test]
    fn test_invalid_rule_rejected() {
        assert!(CredentialRule::new("r", "ssh", "example.com", None, "/", "u", "p").is_err());
        assert!(CredentialRule::new("r", "http", "bad:host", None, "/", "u", "p").is_err());
        assert!(CredentialRule::new("r", "http", "example.com", Some(0), "/", "u", "p").is_err());
        assert!(CredentialRule::new("r", "http", "example.com", None, "/", "", "p").is_err());
        assert!(CredentialRule::new("r", "http", "example.com", None, "/", "u", "").is_err());
    }
}
