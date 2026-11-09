// Created: 2026-11-10
// Reason: refactor: modularize proxy discovery into platform and helper modules
// Description: macOS system proxy detection parsing scutil output.

use std::collections::HashMap;
#[cfg(target_os = "macos")]
use std::process::Command;

use crate::domain::settings::proxy_discovery::{DetectedProxyConfig, ProtocolRoute, ProxySource};

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
pub fn detect_macos_system_proxy() -> Option<DetectedProxyConfig> {
    let output = Command::new("scutil").arg("--proxy").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_scutil_output(&stdout)
}
