// Created: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Last updated: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Description: HTTP authorization header construction for Basic and Bearer token requests.

use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT_ENCODING, AUTHORIZATION, LOCATION, RANGE,
};
use url::Url;

use crate::{
    application::{download_executor::DownloadControl, error::{AppError, AppResult}},
    domain::{
        credentials::{format_basic_auth_header, CredentialMatcher},
        download::BrowserRequestHeaders,
    },
    infrastructure::http_download_executor::{
        policy::MAX_REDIRECTS, retry::send_with_retry, validator::SegmentValidator,
    },
};

pub struct ResolvedResponse {
    pub response: reqwest::Response,
    pub final_url: Url,
    pub auth_header: Option<String>,
}

/// Builds the forwarded browser header set for one request. Session headers
/// are only replayed to the original download host so a redirect cannot leak
/// cookies or authorization to a third-party server.
pub(crate) fn forwarded_headers(
    headers: &BrowserRequestHeaders,
    initial_host: &str,
    current_host: &str,
) -> HeaderMap {
    let mut map = HeaderMap::new();
    if headers.is_empty() || !initial_host.eq_ignore_ascii_case(current_host) {
        return map;
    }
    for (name, value) in headers.as_map() {
        // Referer is handled separately because it is validated as a URL.
        if name == "referer" {
            continue;
        }
        let Ok(name) = HeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        let Ok(value) = HeaderValue::from_str(value) else {
            continue;
        };
        map.insert(name, value);
    }
    map
}

fn insert_header(map: &mut HeaderMap, name: reqwest::header::HeaderName, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        map.insert(name, value);
    }
}

/// Dispatches an HTTP GET request with redirect protection and credential matching:
/// 1. Re-evaluates destination URL against `CredentialMatcher` on every 3xx redirect.
/// 2. If destination URL changes host or moves out of the matched path prefix boundary,
///    source credentials are NOT leaked.
/// 3. If destination URL matches a rule for the new destination, attaches credentials for that rule.
/// 4. Respects in-memory overrides only within the original host and path scope.
pub async fn send_authenticated_request_with_redirects(
    client: &reqwest::Client,
    initial_url: &Url,
    control: &mut DownloadControl,
    override_creds: Option<&(String, String)>,
    matcher: Option<&CredentialMatcher>,
    range_offset: Option<u64>,
    range_end: Option<u64>,
    validator: Option<&SegmentValidator>,
    referer: Option<&str>,
    request_headers: &BrowserRequestHeaders,
) -> AppResult<ResolvedResponse> {
    let mut current_url = initial_url.clone();
    let initial_host = initial_url.host_str().unwrap_or("").to_ascii_lowercase();
    let initial_path = initial_url.path().to_string();

    for redirect_count in 0..=MAX_REDIRECTS {
        let current_host = current_url.host_str().unwrap_or("").to_ascii_lowercase();

        // Evaluate credentials for current_url
        let auth_header = if let Some((user, pass)) = override_creds {
            // Override credentials apply if within same host and path prefix
            if current_host == initial_host && current_url.path().starts_with(&initial_path) {
                Some(format_basic_auth_header(user, pass))
            } else if let Some(m) = matcher {
                m.match_url(&current_url)
                    .map(|r| format_basic_auth_header(&r.username, &r.secret))
            } else {
                None
            }
        } else if let Some(m) = matcher {
            m.match_url(&current_url)
                .map(|r| format_basic_auth_header(&r.username, &r.secret))
        } else {
            None
        };

        let req_client = client.clone();
        let req_url = current_url.clone();
        let req_auth = auth_header.clone();
        let val_header = validator.and_then(|v| v.if_range_value().map(|_| v.header()));
        let val_value = validator.and_then(|v| v.if_range_value().map(|s| s.to_owned()));
        let req_referer = referer.map(|r| r.to_string());
        let forwarded = forwarded_headers(request_headers, &initial_host, &current_host);

        let response = send_with_retry(control, move || {
            let mut headers = forwarded.clone();
            if let Some(ref auth) = req_auth {
                insert_header(&mut headers, AUTHORIZATION, auth);
            }
            if let Some(ref r) = req_referer {
                insert_header(&mut headers, reqwest::header::REFERER, r);
            }
            if let Some(start) = range_offset {
                if let Some(end) = range_end {
                    insert_header(&mut headers, RANGE, &format!("bytes={start}-{end}"));
                } else if start > 0 {
                    insert_header(&mut headers, RANGE, &format!("bytes={start}-"));
                }
            }
            if let (Some(ref h), Some(ref v)) = (&val_header, &val_value) {
                insert_header(&mut headers, h.clone(), v);
            }
            headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
            req_client.get(req_url.clone()).headers(headers)
        })
        .await?;

        if response.status().is_redirection() {
            if redirect_count >= MAX_REDIRECTS {
                return Err(AppError::network("Too many HTTP redirects."));
            }
            let loc_header = response.headers().get(LOCATION).ok_or_else(|| {
                AppError::network("HTTP redirection response missing Location header.")
            })?;
            let loc_str = loc_header.to_str().map_err(|_| {
                AppError::network("Invalid non-ASCII Location header in redirect.")
            })?;
            let next_url = current_url.join(loc_str).map_err(|e| {
                AppError::network(format!("Could not resolve redirect location: {e}"))
            })?;
            current_url = next_url;
            continue;
        }

        return Ok(ResolvedResponse {
            response,
            final_url: current_url,
            auth_header,
        });
    }

    Err(AppError::network("Too many HTTP redirects."))
}
