// Created: 2026-11-06
// Reason: feat(network): implement HTTP and HTTPS proxy support with reachability test (fixes #11)
// Last updated: 2026-11-08
// Reason: refactor(architecture): decouple proxy domain model from reqwest and encapsulate settings update orchestration
// Reason: feat(proxy): add SOCKS5 proxy support and protocol-specific proxy settings (Closes #34)
// Description: Proxy server configuration entity supporting HTTP, HTTPS, and SOCKS5 proxy protocols.

use serde::{Deserialize, Serialize};

use super::validation_error::SettingsValidationError;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProxyProtocol {
    Direct,
    Http,
    Https,
    Socks5,
}

impl ProxyProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Http => "http",
            Self::Https => "https",
            Self::Socks5 => "socks5",
        }
    }
}

impl Default for ProxyProtocol {
    fn default() -> Self {
        Self::Http
    }
}

pub const DEFAULT_PROXY_PORT: u16 = 8080;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProxySettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub protocol: ProxyProtocol,
    #[serde(default)]
    pub host: String,
    #[serde(default = "default_proxy_port")]
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub resolve_dns_remotely: bool,
}

fn default_proxy_port() -> u16 {
    DEFAULT_PROXY_PORT
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            protocol: ProxyProtocol::Http,
            host: String::new(),
            port: DEFAULT_PROXY_PORT,
            username: None,
            password: None,
            resolve_dns_remotely: false,
        }
    }
}

impl ProxySettings {
    pub fn clean_host(&self) -> String {
        let host = self.host.trim();
        let host = host.strip_prefix("http://").unwrap_or(host);
        let host = host.strip_prefix("https://").unwrap_or(host);
        let host = host.strip_prefix("socks5://").unwrap_or(host);
        let host = host.strip_prefix("socks5h://").unwrap_or(host);
        let host = host.trim_end_matches('/');
        host.to_string()
    }

    pub fn validate(&self) -> Result<(), SettingsValidationError> {
        if self.port == 0 {
            return Err(SettingsValidationError::new(
                "Proxy port must be between 1 and 65535.",
            ));
        }

        if self.enabled && self.protocol != ProxyProtocol::Direct {
            let clean = self.clean_host();
            if clean.is_empty() {
                return Err(SettingsValidationError::new("Proxy host cannot be empty."));
            }
            if clean.chars().any(|c| c.is_whitespace() || c == '/' || c == '@') {
                return Err(SettingsValidationError::new(
                    "Proxy host must be a valid hostname or IP address without credentials, slashes, or spaces.",
                ));
            }
        }

        Ok(())
    }

    pub fn sanitize_error(&self, msg: &str) -> String {
        let mut sanitized = msg.to_string();
        if let Some(ref pass) = self.password {
            if !pass.is_empty() {
                sanitized = sanitized.replace(pass, "******");
            }
        }
        if let Some(ref user) = self.username {
            let trimmed = user.trim();
            if !trimmed.is_empty() {
                sanitized = sanitized.replace(trimmed, "******");
            }
        }
        if let Some(at_pos) = sanitized.find('@') {
            if let Some(proto_pos) = sanitized[..at_pos].rfind("://") {
                let cred_start = proto_pos + 3;
                if cred_start < at_pos {
                    sanitized.replace_range(cred_start..at_pos, "******:******");
                }
            }
        }
        sanitized
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestResult {
    pub success: bool,
    pub message: String,
}
