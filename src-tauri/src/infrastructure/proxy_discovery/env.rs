// Created: 2026-11-10
// Reason: refactor: modularize proxy discovery into platform and helper modules
// Description: Environment variable proxy detection (HTTP_PROXY, HTTPS_PROXY, ALL_PROXY, etc.).

use crate::domain::settings::proxy_discovery::{DetectedProxyConfig, ProtocolRoute, ProxySource};

use super::helpers::parse_proxy_url;

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
