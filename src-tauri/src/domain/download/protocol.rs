// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-08
// Reason: feat(transfer): add FTP transfers with passive mode, authentication, and resume (Closes #28)
// Description: Supported transfer protocols enumeration (HTTP, HTTPS, FTP) and scheme validation.

use serde::{Deserialize, Serialize};

use super::download_policy::{FTP_PROTOCOL, HTTPS_PROTOCOL, HTTP_PROTOCOL};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadProtocol {
    Http,
    Https,
    Ftp,
}

impl DownloadProtocol {
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            HTTP_PROTOCOL => Some(Self::Http),
            HTTPS_PROTOCOL => Some(Self::Https),
            FTP_PROTOCOL => Some(Self::Ftp),
            _ => None,
        }
    }
}
