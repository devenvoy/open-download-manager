// Created: 2026-11-08
// Reason: feat(proxy): import system and browser proxy configurations (Closes #39)
// Last updated: 2026-11-10
// Reason: refactor: modularize proxy discovery into platform and helper modules
// Description: Platform adapters detecting system, browser, and environment proxy settings.

pub mod browser;
pub mod env;
pub mod helpers;
pub mod macos;
pub mod windows;

pub use browser::{detect_browser_proxy, parse_browser_preferences};
pub use env::parse_env_vars;
pub use macos::parse_scutil_output;
#[cfg(target_os = "macos")]
pub use macos::detect_macos_system_proxy;
pub use windows::parse_windows_registry_output;
#[cfg(target_os = "windows")]
pub use windows::detect_windows_system_proxy;

use crate::{
    application::{error::AppResult, proxy_discovery_service::ProxyDiscoveryAdapter},
    domain::settings::proxy_discovery::{DetectedProxyConfig, ProxySource},
};

pub struct SystemProxyDetector;

impl SystemProxyDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemProxyDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ProxyDiscoveryAdapter for SystemProxyDetector {
    fn detect(&self) -> AppResult<DetectedProxyConfig> {
        // 1. Try OS system proxy detection first
        #[cfg(target_os = "macos")]
        if let Some(config) = detect_macos_system_proxy() {
            return Ok(config);
        }

        #[cfg(target_os = "windows")]
        if let Some(config) = detect_windows_system_proxy() {
            return Ok(config);
        }

        // 2. Try browser proxy detection (e.g. Chrome/Edge/Brave preferences)
        if let Some(config) = detect_browser_proxy() {
            return Ok(config);
        }

        // 3. Try standard environment variables (HTTP_PROXY, HTTPS_PROXY, ALL_PROXY, etc.)
        if let Some(config) = parse_env_vars(|k| std::env::var(k).ok()) {
            return Ok(config);
        }

        // 4. Fallback to direct (no proxy)
        Ok(DetectedProxyConfig::direct(ProxySource::System))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::domain::settings::DetectedProxyMode;

    #[test]
    fn test_parse_scutil_output_pac() {
        let output = r#"
<dictionary> {
  ExceptionsList : <array> {
    0 : *.local
    1 : 169.254/16
  }
  ProxyAutoConfigEnable : 1
  ProxyAutoConfigURLString : http://corp.internal/proxy.pac
}
"#;
        let config = parse_scutil_output(output).expect("should parse PAC from scutil");
        assert_eq!(config.mode, DetectedProxyMode::Pac);
        assert_eq!(
            config.pac_url,
            Some("http://corp.internal/proxy.pac".to_string())
        );
        assert!(!config.can_import);
    }

    #[test]
    fn test_parse_scutil_output_manual_http_and_ftp() {
        let output = r#"
<dictionary> {
  HTTPEnable : 1
  HTTPPort : 8888
  HTTPProxy : 10.0.0.1
  FTPEnable : 1
  FTPPort : 21
  FTPProxy : 10.0.0.1
}
"#;
        let config = parse_scutil_output(output).expect("should parse manual proxy");
        assert_eq!(config.mode, DetectedProxyMode::Manual);
        assert!(config.can_import);
        assert_eq!(config.routes.len(), 2);
        assert_eq!(config.routes[0].protocol, "http");
        assert!(config.routes[0].supported);
        assert_eq!(config.routes[1].protocol, "ftp");
        assert!(!config.routes[1].supported);
        assert!(config.unsupported_fields.iter().any(|f| f.contains("FTP")));
    }

    #[test]
    fn test_parse_windows_registry_pac() {
        let output = r#"
HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Internet Settings
    AutoConfigURL    REG_SZ    http://autoproxy.company.com/wpad.dat
    ProxyEnable    REG_DWORD    0x1
"#;
        let config = parse_windows_registry_output(output).expect("should parse pac");
        assert_eq!(config.mode, DetectedProxyMode::Pac);
        assert_eq!(
            config.pac_url,
            Some("http://autoproxy.company.com/wpad.dat".to_string())
        );
    }

    #[test]
    fn test_parse_windows_registry_manual() {
        let output = r#"
HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Internet Settings
    ProxyEnable    REG_DWORD    0x1
    ProxyServer    REG_SZ    http=192.168.1.100:8080;https=192.168.1.100:8443
"#;
        let config = parse_windows_registry_output(output).expect("should parse manual");
        assert_eq!(config.mode, DetectedProxyMode::Manual);
        assert!(config.can_import);
        assert_eq!(config.routes.len(), 2);
    }

    #[test]
    fn test_parse_env_vars_with_credentials_withholds_password() {
        let env_map: HashMap<&str, &str> = [
            ("http_proxy", "http://agent_user:s3cr3t_pass@127.0.0.1:3128"),
            ("ftp_proxy", "ftp://ftp.example.com:2121"),
        ]
        .into_iter()
        .collect();

        let config = parse_env_vars(|k| env_map.get(k).map(|s| s.to_string()))
            .expect("should parse env proxy");

        assert_eq!(config.mode, DetectedProxyMode::Manual);
        assert!(config.can_import);
        assert!(config.credentials_available);
        let settings = config.import_settings.expect("settings");
        assert_eq!(settings.host, "127.0.0.1");
        assert_eq!(settings.port, 3128);
        assert_eq!(settings.username, Some("agent_user".to_string()));
        assert!(settings.password.is_none(), "Password MUST be withheld");
    }

    #[test]
    fn test_parse_browser_preferences_fixed_servers() {
        let json = r#"{
            "proxy": {
                "mode": "fixed_servers",
                "server": "127.0.0.1:9090"
            }
        }"#;
        let config = parse_browser_preferences(json, "Google Chrome").expect("should parse browser proxy");
        assert_eq!(config.source, ProxySource::Browser("Google Chrome".to_string()));
        assert_eq!(config.mode, DetectedProxyMode::Manual);
        assert!(config.can_import);
        assert_eq!(config.import_settings.unwrap().port, 9090);
    }
}
