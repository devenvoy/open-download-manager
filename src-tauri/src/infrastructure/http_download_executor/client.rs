// Created: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Last updated: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Description: Configurable Reqwest HTTP client builder with proxy and TLS configuration.

use std::time::Duration;

use crate::{
    application::error::{AppError, AppResult},
    domain::settings::{ProxySettings, ProxyTestResult},
};

use super::policy::CONNECT_TIMEOUT_SECONDS;

/// Extension trait providing reqwest proxy conversion for `ProxySettings`
/// without polluting the domain layer with infrastructure types.
pub trait ReqwestProxyExt {
    fn to_reqwest_proxy(&self) -> AppResult<reqwest::Proxy>;
}

impl ReqwestProxyExt for ProxySettings {
    fn to_reqwest_proxy(&self) -> AppResult<reqwest::Proxy> {
        build_reqwest_proxy(self)
    }
}

/// Sanitizes credentials and sensitive details from network/client error messages.
pub fn sanitize_proxy_error(msg: &str, proxy: Option<&ProxySettings>) -> String {
    if let Some(p) = proxy {
        p.sanitize_error(msg)
    } else {
        let mut sanitized = msg.to_string();
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

/// Converts domain ProxySettings into an infrastructure `reqwest::Proxy`.
pub fn build_reqwest_proxy(proxy: &ProxySettings) -> AppResult<reqwest::Proxy> {
    let clean = proxy.clean_host();
    if clean.is_empty() {
        return Err(AppError::invalid_settings("Proxy host cannot be empty."));
    }
    if proxy.port == 0 {
        return Err(AppError::invalid_settings(
            "Proxy port must be between 1 and 65535.",
        ));
    }

    let proxy_url = format!("{}://{}:{}", proxy.protocol.as_str(), clean, proxy.port);
    let mut req_proxy = reqwest::Proxy::all(&proxy_url).map_err(|err| {
        let msg = format!("Invalid proxy configuration for {clean}:{}: {err}", proxy.port);
        AppError::invalid_settings(sanitize_proxy_error(&msg, Some(proxy)))
    })?;

    if let Some(username) = &proxy.username {
        let user = username.trim();
        if !user.is_empty() {
            let password = proxy.password.as_deref().unwrap_or("");
            req_proxy = req_proxy.basic_auth(user, password);
        }
    }

    Ok(req_proxy)
}

/// Creates a reqwest client configured with optional proxy settings.
pub fn create_http_client(proxy: Option<&ProxySettings>) -> AppResult<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECONDS))
        .redirect(reqwest::redirect::Policy::none());

    if let Some(proxy_config) = proxy {
        if proxy_config.enabled {
            let req_proxy = build_reqwest_proxy(proxy_config)?;
            builder = builder.proxy(req_proxy);
        } else {
            builder = builder.no_proxy();
        }
    }

    builder.build().map_err(|error| {
        let msg = format!("Could not initialize HTTP client: {error}");
        let sanitized = sanitize_proxy_error(&msg, proxy);
        AppError::network(sanitized)
    })
}

/// Tests proxy connectivity by sending a lightweight probe to a test target.
pub async fn test_proxy_connection(
    proxy: &ProxySettings,
    test_url: Option<&str>,
) -> ProxyTestResult {
    if let Err(e) = proxy.validate() {
        return ProxyTestResult {
            success: false,
            message: sanitize_proxy_error(&e.message, Some(proxy)),
        };
    }

    let reqwest_proxy = match build_reqwest_proxy(proxy) {
        Ok(p) => p,
        Err(e) => {
            return ProxyTestResult {
                success: false,
                message: sanitize_proxy_error(
                    &format!("Invalid proxy configuration: {}", e.message),
                    Some(proxy),
                ),
            }
        }
    };

    let client = match reqwest::Client::builder()
        .proxy(reqwest_proxy)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ProxyTestResult {
                success: false,
                message: sanitize_proxy_error(
                    &format!("Could not initialize test client: {e}"),
                    Some(proxy),
                ),
            }
        }
    };

    let target = test_url.unwrap_or("https://www.google.com/generate_204");

    match client.get(target).send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() || status.as_u16() == 204 {
                ProxyTestResult {
                    success: true,
                    message: format!("Proxy connection successful (HTTP {})", status.as_u16()),
                }
            } else if status.as_u16() == 407 {
                ProxyTestResult {
                    success: false,
                    message: "Proxy authentication required (HTTP 407). Check your username and password.".to_string(),
                }
            } else {
                ProxyTestResult {
                    success: true,
                    message: format!("Proxy reachable (HTTP {})", status.as_u16()),
                }
            }
        }
        Err(e) => {
            let message = if e.is_connect() {
                format!("Could not connect to proxy: {e}")
            } else if e.is_timeout() {
                "Connection to proxy timed out.".to_string()
            } else {
                format!("Proxy test failed: {e}")
            };
            ProxyTestResult {
                success: false,
                message: sanitize_proxy_error(&message, Some(proxy)),
            }
        }
    }
}
