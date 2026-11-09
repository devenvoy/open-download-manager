// Created: 2026-11-08
// Reason: feat(proxy): import system and browser proxy configurations (Closes #39)
// Description: Platform adapters detecting system, browser, and environment proxy settings.

use std::{collections::HashMap, path::PathBuf, process::Command};

use url::Url;

use crate::{
    application::{error::AppResult, proxy_discovery_service::ProxyDiscoveryAdapter},
    domain::settings::proxy_discovery::{
        DetectedProxyConfig, ProtocolRoute, ProxySource,
    },
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

/// Parses the output of macOS `scutil --proxy`
pub fn parse_scutil_output(output: &str) -> Option<DetectedProxyConfig> {
    let mut map: HashMap<String, String> = HashMap::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some((k, v)) = trimmed.split_once(':') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }

    // Check PAC proxy
    if map.get("ProxyAutoConfigEnable").map(|v| v.as_str()) == Some("1") {
        if let Some(pac_url) = map.get("ProxyAutoConfigURLString") {
            if !pac_url.is_empty() {
                return Some(DetectedProxyConfig::pac(
                    ProxySource::System,
                    pac_url.clone(),
                ));
            }
        }
    }

    let mut routes = Vec::new();

    // Check HTTP
    if map.get("HTTPEnable").map(|v| v.as_str()) == Some("1") {
        if let Some(host) = map.get("HTTPProxy") {
            let port = map
                .get("HTTPPort")
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(8080);
            routes.push(ProtocolRoute {
                protocol: "http".to_string(),
                host: host.clone(),
                port,
                supported: true,
                reason: None,
            });
        }
    }

    // Check HTTPS
    if map.get("HTTPSEnable").map(|v| v.as_str()) == Some("1") {
        if let Some(host) = map.get("HTTPSProxy") {
            let port = map
                .get("HTTPSPort")
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(8443);
            routes.push(ProtocolRoute {
                protocol: "https".to_string(),
                host: host.clone(),
                port,
                supported: true,
                reason: None,
            });
        }
    }

    // Check SOCKS
    if map.get("SOCKSEnable").map(|v| v.as_str()) == Some("1") {
        if let Some(host) = map.get("SOCKSProxy") {
            let port = map
                .get("SOCKSPort")
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(1080);
            routes.push(ProtocolRoute {
                protocol: "socks5".to_string(),
                host: host.clone(),
                port,
                supported: true,
                reason: None,
            });
        }
    }

    // Check FTP
    if map.get("FTPEnable").map(|v| v.as_str()) == Some("1") {
        if let Some(host) = map.get("FTPProxy") {
            let port = map
                .get("FTPPort")
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(2121);
            routes.push(ProtocolRoute {
                protocol: "ftp".to_string(),
                host: host.clone(),
                port,
                supported: false,
                reason: Some("FTP proxy routing is not supported for HTTP/HTTPS transfers.".to_string()),
            });
        }
    }

    if routes.is_empty() {
        None
    } else {
        Some(DetectedProxyConfig::manual(
            ProxySource::System,
            routes,
            false,
            Vec::new(),
            None,
        ))
    }
}

#[cfg(target_os = "macos")]
fn detect_macos_system_proxy() -> Option<DetectedProxyConfig> {
    let output = Command::new("scutil").arg("--proxy").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_scutil_output(&stdout)
}

/// Parses the output of Windows `reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings"`
pub fn parse_windows_registry_output(output: &str) -> Option<DetectedProxyConfig> {
    let mut proxy_enable = false;
    let mut proxy_server = None;
    let mut auto_config_url = None;

    for line in output.lines() {
        let line = line.trim();
        if line.contains("AutoConfigURL") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                auto_config_url = Some(parts[2..].join(" "));
            }
        } else if line.contains("ProxyEnable") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let val = parts[parts.len() - 1];
                if val == "0x1" || val == "1" {
                    proxy_enable = true;
                }
            }
        } else if line.contains("ProxyServer") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                proxy_server = Some(parts[2..].join(" "));
            }
        }
    }

    if let Some(pac) = auto_config_url {
        let pac = pac.trim().to_string();
        if !pac.is_empty() {
            return Some(DetectedProxyConfig::pac(ProxySource::System, pac));
        }
    }

    if proxy_enable {
        if let Some(server_str) = proxy_server {
            let server_str = server_str.trim();
            if !server_str.is_empty() {
                let mut routes = Vec::new();
                if server_str.contains('=') {
                    // Multi-protocol: http=host:port;https=host:port;socks=host:port;ftp=host:port
                    for entry in server_str.split(';') {
                        let entry = entry.trim();
                        if let Some((proto, endpoint)) = entry.split_once('=') {
                            if let Some((host, port)) = parse_host_port(endpoint) {
                                let (proto_key, supported, reason) = match proto.to_lowercase().as_str() {
                                    "http" => ("http".to_string(), true, None),
                                    "https" => ("https".to_string(), true, None),
                                    "socks" | "socks5" => ("socks5".to_string(), true, None),
                                    "ftp" => ("ftp".to_string(), false, Some("FTP proxy routing is not supported for HTTP/HTTPS transfers.".to_string())),
                                    other => (other.to_string(), false, Some(format!("{other} proxy routing is not supported."))),
                                };
                                routes.push(ProtocolRoute {
                                    protocol: proto_key,
                                    host,
                                    port,
                                    supported,
                                    reason,
                                });
                            }
                        }
                    }
                } else if let Some((host, port)) = parse_host_port(server_str) {
                    routes.push(ProtocolRoute {
                        protocol: "http".to_string(),
                        host: host.clone(),
                        port,
                        supported: true,
                        reason: None,
                    });
                    routes.push(ProtocolRoute {
                        protocol: "https".to_string(),
                        host,
                        port,
                        supported: true,
                        reason: None,
                    });
                }

                if !routes.is_empty() {
                    return Some(DetectedProxyConfig::manual(
                        ProxySource::System,
                        routes,
                        false,
                        Vec::new(),
                        None,
                    ));
                }
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn detect_windows_system_proxy() -> Option<DetectedProxyConfig> {
    let output = Command::new("reg")
        .args([
            "query",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_windows_registry_output(&stdout)
}

/// Parses environment variables (HTTP_PROXY, HTTPS_PROXY, ALL_PROXY, etc.)
pub fn parse_env_vars<F>(get_env: F) -> Option<DetectedProxyConfig>
where
    F: Fn(&str) -> Option<String>,
{
    // Check PAC in env
    if let Some(pac_url) = get_env("AUTO_PROXY").or_else(|| get_env("auto_proxy")) {
        let pac_url = pac_url.trim().to_string();
        if !pac_url.is_empty() {
            return Some(DetectedProxyConfig::pac(
                ProxySource::Environment,
                pac_url,
            ));
        }
    }

    let mut routes = Vec::new();
    let mut credentials_available = false;
    let mut suggested_user = None;

    let probe_env = [
        ("http", &["HTTP_PROXY", "http_proxy"][..]),
        ("https", &["HTTPS_PROXY", "https_proxy"][..]),
        ("socks5", &["ALL_PROXY", "all_proxy"][..]),
        ("ftp", &["FTP_PROXY", "ftp_proxy"][..]),
    ];

    for (proto_type, keys) in probe_env {
        for key in keys {
            if let Some(val) = get_env(key) {
                let trimmed = val.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if let Some((host, port, has_auth, username)) = parse_proxy_url(trimmed) {
                    if has_auth {
                        credentials_available = true;
                    }
                    if username.is_some() && suggested_user.is_none() {
                        suggested_user = username;
                    }

                    let (supported, reason) = if proto_type == "ftp" {
                        (
                            false,
                            Some("FTP proxy routing is not supported for HTTP/HTTPS transfers.".to_string()),
                        )
                    } else {
                        (true, None)
                    };

                    routes.push(ProtocolRoute {
                        protocol: proto_type.to_string(),
                        host,
                        port,
                        supported,
                        reason,
                    });
                    break;
                }
            }
        }
    }

    if routes.is_empty() {
        None
    } else {
        Some(DetectedProxyConfig::manual(
            ProxySource::Environment,
            routes,
            credentials_available,
            Vec::new(),
            suggested_user,
        ))
    }
}

/// Inspects browser configuration files (Chrome, Edge, Brave, etc.) for proxy rules.
pub fn detect_browser_proxy() -> Option<DetectedProxyConfig> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    let home_path = PathBuf::from(home);

    let candidates = [
        (
            "Google Chrome",
            home_path.join(if cfg!(target_os = "macos") {
                "Library/Application Support/Google/Chrome/Default/Preferences"
            } else if cfg!(target_os = "windows") {
                "AppData/Local/Google/Chrome/User Data/Default/Preferences"
            } else {
                ".config/google-chrome/Default/Preferences"
            }),
        ),
        (
            "Brave",
            home_path.join(if cfg!(target_os = "macos") {
                "Library/Application Support/BraveSoftware/Brave-Browser/Default/Preferences"
            } else if cfg!(target_os = "windows") {
                "AppData/Local/BraveSoftware/Brave-Browser/User Data/Default/Preferences"
            } else {
                ".config/BraveSoftware/Brave-Browser/Default/Preferences"
            }),
        ),
        (
            "Microsoft Edge",
            home_path.join(if cfg!(target_os = "macos") {
                "Library/Application Support/Microsoft Edge/Default/Preferences"
            } else if cfg!(target_os = "windows") {
                "AppData/Local/Microsoft/Edge/User Data/Default/Preferences"
            } else {
                ".config/microsoft-edge/Default/Preferences"
            }),
        ),
    ];

    for (browser_name, pref_path) in candidates {
        if pref_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&pref_path) {
                if let Some(config) = parse_browser_preferences(&content, browser_name) {
                    return Some(config);
                }
            }
        }
    }

    None
}

/// Parses browser JSON preferences file looking for proxy settings
pub fn parse_browser_preferences(json_content: &str, browser_name: &str) -> Option<DetectedProxyConfig> {
    let parsed: serde_json::Value = serde_json::from_str(json_content).ok()?;
    let proxy = parsed.get("proxy")?;

    let mode = proxy.get("mode").and_then(|m| m.as_str())?;
    match mode {
        "pac_script" => {
            let pac_url = proxy
                .get("pac_url")
                .and_then(|u| u.as_str())
                .unwrap_or_default()
                .trim()
                .to_string();
            if !pac_url.is_empty() {
                return Some(DetectedProxyConfig::pac(
                    ProxySource::Browser(browser_name.to_string()),
                    pac_url,
                ));
            }
        }
        "fixed_servers" => {
            let server = proxy
                .get("server")
                .and_then(|s| s.as_str())
                .unwrap_or_default()
                .trim();
            if !server.is_empty() {
                if let Some((host, port)) = parse_host_port(server) {
                    let routes = vec![
                        ProtocolRoute {
                            protocol: "http".to_string(),
                            host: host.clone(),
                            port,
                            supported: true,
                            reason: None,
                        },
                        ProtocolRoute {
                            protocol: "https".to_string(),
                            host,
                            port,
                            supported: true,
                            reason: None,
                        },
                    ];
                    return Some(DetectedProxyConfig::manual(
                        ProxySource::Browser(browser_name.to_string()),
                        routes,
                        false,
                        Vec::new(),
                        None,
                    ));
                }
            }
        }
        _ => {}
    }

    None
}

fn parse_host_port(endpoint: &str) -> Option<(String, u16)> {
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
fn parse_proxy_url(raw: &str) -> Option<(String, u16, bool, Option<String>)> {
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

#[cfg(test)]
mod tests {
    use super::*;
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
