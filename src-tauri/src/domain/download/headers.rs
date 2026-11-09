// Created: 2026-11-10
// Reason: fix(browser): replay captured browser request headers for session-gated downloads
// Last updated: 2026-11-10
// Description: Sanitized, non-persisted browser request headers forwarded to the HTTP executor.

use std::collections::HashMap;

use serde::Deserialize;

pub const MAX_FORWARDED_HEADER_VALUE_BYTES: usize = 16_384;

const FORWARDED_HEADER_NAMES: [&str; 5] = [
    "cookie",
    "authorization",
    "user-agent",
    "origin",
    "referer",
];

/// Filters untrusted browser headers down to the small set of request headers
/// that are required to replay a download request. Invalid or oversized values
/// are dropped instead of failing the whole download.
pub fn sanitize_forward_headers(headers: HashMap<String, String>) -> HashMap<String, String> {
    headers
        .into_iter()
        .filter_map(|(name, value)| {
            let name = name.to_ascii_lowercase();
            if !FORWARDED_HEADER_NAMES.contains(&name.as_str()) {
                return None;
            }
            if value.is_empty()
                || value.len() > MAX_FORWARDED_HEADER_VALUE_BYTES
                || value.contains('\r')
                || value.contains('\n')
            {
                return None;
            }
            Some((name, value))
        })
        .collect()
}

/// Browser request headers that travel with a download record in memory.
///
/// Values are redacted from `Debug` output and the field is skipped during
/// history serialization so session cookies are never written to disk.
#[derive(Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct BrowserRequestHeaders(HashMap<String, String>);

impl BrowserRequestHeaders {
    pub fn sanitize(headers: HashMap<String, String>) -> Self {
        Self(sanitize_forward_headers(headers))
    }

    pub fn as_map(&self) -> &HashMap<String, String> {
        &self.0
    }

    pub fn into_map(self) -> HashMap<String, String> {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for BrowserRequestHeaders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrowserRequestHeaders(<redacted>)")
    }
}
