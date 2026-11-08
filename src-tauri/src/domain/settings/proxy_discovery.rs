// Created: 2026-11-08
// Reason: feat(proxy): import system and browser proxy configurations (Closes #39)
// Description: Domain entities and evaluation rules for discovered system and browser proxy configurations.

use serde::{Deserialize, Serialize};

use super::proxy::{ProxyProtocol, ProxySettings};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProxySource {
    System,
    Browser(String),
    Environment,
    Manual,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DetectedProxyMode {
    Direct,
    Manual,
    Pac,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolRoute {
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub supported: bool,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DetectedProxyConfig {
    pub source: ProxySource,
    pub mode: DetectedProxyMode,
    pub pac_url: Option<String>,
    pub routes: Vec<ProtocolRoute>,
    pub credentials_available: bool,
    pub unsupported_fields: Vec<String>,
    pub can_import: bool,
    pub import_settings: Option<ProxySettings>,
}

impl DetectedProxyConfig {
    pub fn direct(source: ProxySource) -> Self {
        Self {
            source,
            mode: DetectedProxyMode::Direct,
            pac_url: None,
            routes: Vec::new(),
            credentials_available: false,
            unsupported_fields: Vec::new(),
            can_import: true,
            import_settings: Some(ProxySettings {
                enabled: false,
                protocol: ProxyProtocol::Direct,
                host: String::new(),
                port: 8080,
                username: None,
                password: None,
                resolve_dns_remotely: false,
            }),
        }
    }

    pub fn pac(source: ProxySource, pac_url: String) -> Self {
        Self {
            source,
            mode: DetectedProxyMode::Pac,
            pac_url: Some(pac_url),
            routes: Vec::new(),
            credentials_available: false,
            unsupported_fields: vec![
                "PAC script auto-routing is unavailable; automatic script evaluation is not supported."
                    .to_string(),
            ],
            can_import: false,
            import_settings: None,
        }
    }

    pub fn manual(
        source: ProxySource,
        routes: Vec<ProtocolRoute>,
        credentials_available: bool,
        mut unsupported_fields: Vec<String>,
        suggested_username: Option<String>,
    ) -> Self {
        // Find first supported route (HTTP, HTTPS, or SOCKS5)
        let supported_route = routes.iter().find(|r| r.supported);

        // Record any unsupported routes in unsupported_fields if not already present
        for route in &routes {
            if !route.supported {
                let note = format!(
                    "{} proxy ({}:{}) is not supported for transfer routing.",
                    route.protocol.to_uppercase(),
                    route.host,
                    route.port
                );
                if !unsupported_fields.contains(&note) {
                    unsupported_fields.push(note);
                }
            }
        }

        if let Some(route) = supported_route {
            let protocol = match route.protocol.to_lowercase().as_str() {
                "http" => ProxyProtocol::Http,
                "https" => ProxyProtocol::Https,
                "socks5" | "socks" => ProxyProtocol::Socks5,
                _ => ProxyProtocol::Http,
            };

            let import_settings = ProxySettings {
                enabled: true,
                protocol,
                host: route.host.clone(),
                port: route.port,
                username: suggested_username,
                password: None, // Passwords must never be exposed or pre-filled in preview
                resolve_dns_remotely: protocol == ProxyProtocol::Socks5,
            };

            Self {
                source,
                mode: DetectedProxyMode::Manual,
                pac_url: None,
                routes,
                credentials_available,
                unsupported_fields,
                can_import: true,
                import_settings: Some(import_settings),
            }
        } else {
            Self {
                source,
                mode: DetectedProxyMode::Manual,
                pac_url: None,
                routes,
                credentials_available,
                unsupported_fields,
                can_import: false,
                import_settings: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_detected_config() {
        let config = DetectedProxyConfig::direct(ProxySource::System);
        assert_eq!(config.mode, DetectedProxyMode::Direct);
        assert!(config.can_import);
        assert_eq!(
            config.import_settings.unwrap().protocol,
            ProxyProtocol::Direct
        );
    }

    #[test]
    fn test_pac_detected_config_rejects_silent_direct() {
        let config = DetectedProxyConfig::pac(
            ProxySource::System,
            "http://internal.corp/wpad.dat".to_string(),
        );
        assert_eq!(config.mode, DetectedProxyMode::Pac);
        assert!(!config.can_import);
        assert!(config.import_settings.is_none());
        assert!(!config.unsupported_fields.is_empty());
        assert!(config.unsupported_fields[0].contains("PAC script auto-routing is unavailable"));
    }

    #[test]
    fn test_manual_config_strips_password_and_identifies_unsupported_routes() {
        let routes = vec![
            ProtocolRoute {
                protocol: "http".to_string(),
                host: "proxy.corp.internal".to_string(),
                port: 8080,
                supported: true,
                reason: None,
            },
            ProtocolRoute {
                protocol: "ftp".to_string(),
                host: "proxy.corp.internal".to_string(),
                port: 2121,
                supported: false,
                reason: Some("FTP proxy routing is not supported".to_string()),
            },
        ];

        let config = DetectedProxyConfig::manual(
            ProxySource::Environment,
            routes,
            true, // credentials detected
            Vec::new(),
            Some("corp_user".to_string()),
        );

        assert!(config.can_import);
        assert!(config.credentials_available);
        let settings = config.import_settings.expect("should have import settings");
        assert_eq!(settings.host, "proxy.corp.internal");
        assert_eq!(settings.port, 8080);
        assert_eq!(settings.username, Some("corp_user".to_string()));
        assert!(
            settings.password.is_none(),
            "password must NEVER be populated in preview"
        );
        assert!(config.unsupported_fields.iter().any(|f| f.contains("FTP")));
    }
}
