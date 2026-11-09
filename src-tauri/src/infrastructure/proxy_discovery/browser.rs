// Created: 2026-11-10
// Reason: refactor: modularize proxy discovery into platform and helper modules
// Description: Browser preferences proxy detection for Chrome, Brave, and Edge.

use std::path::PathBuf;

use crate::domain::settings::proxy_discovery::{DetectedProxyConfig, ProtocolRoute, ProxySource};

use super::helpers::parse_host_port;

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
