use serde::{Deserialize, Serialize};

use crate::application::error::{AppError, AppResult};
use super::validation_error::SettingsValidationError;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProxyProtocol {
    Http,
    Https,
}

impl ProxyProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
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
        }
    }
}

impl ProxySettings {
    pub fn clean_host(&self) -> String {
        let host = self.host.trim();
        let host = host.strip_prefix("http://").unwrap_or(host);
        let host = host.strip_prefix("https://").unwrap_or(host);
        let host = host.trim_end_matches('/');
        host.to_string()
    }

    pub fn validate(&self) -> Result<(), SettingsValidationError> {
        if self.port == 0 {
            return Err(SettingsValidationError::new(
                "Proxy port must be between 1 and 65535.",
            ));
        }

        if self.enabled {
            let clean = self.clean_host();
            if clean.is_empty() {
                return Err(SettingsValidationError::new("Proxy host cannot be empty."));
            }
            if clean.chars().any(|c| c.is_whitespace() || c == '/') {
                return Err(SettingsValidationError::new(
                    "Proxy host must be a valid hostname or IP address without slashes or spaces.",
                ));
            }
        }

        Ok(())
    }

    pub fn to_reqwest_proxy(&self) -> AppResult<reqwest::Proxy> {
        let clean = self.clean_host();
        if clean.is_empty() {
            return Err(AppError::invalid_settings("Proxy host cannot be empty."));
        }
        if self.port == 0 {
            return Err(AppError::invalid_settings(
                "Proxy port must be between 1 and 65535.",
            ));
        }

        let proxy_url = format!("{}://{}:{}", self.protocol.as_str(), clean, self.port);
        let mut proxy = reqwest::Proxy::all(&proxy_url).map_err(|err| {
            AppError::invalid_settings(format!("Invalid proxy configuration '{proxy_url}': {err}"))
        })?;

        if let Some(username) = &self.username {
            let user = username.trim();
            if !user.is_empty() {
                let password = self.password.as_deref().unwrap_or("");
                proxy = proxy.basic_auth(user, password);
            }
        }

        Ok(proxy)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestResult {
    pub success: bool,
    pub message: String,
}
