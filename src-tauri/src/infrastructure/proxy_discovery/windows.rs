// Created: 2026-11-10
// Reason: refactor: modularize proxy discovery into platform and helper modules
// Description: Windows system proxy detection parsing registry settings.

#[cfg(target_os = "windows")]
use std::process::Command;

use crate::domain::settings::proxy_discovery::{DetectedProxyConfig, ProtocolRoute, ProxySource};

use super::helpers::parse_host_port;

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
pub fn detect_windows_system_proxy() -> Option<DetectedProxyConfig> {
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
