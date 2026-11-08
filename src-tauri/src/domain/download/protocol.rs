// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-08
// Reason: feat(transfer): add FTP transfers with passive mode, authentication, and resume (Closes #28)
// Reason: feat(mms): add MMS streaming support or explicit unsupported guidance (Closes #35)
// Description: Supported transfer protocols enumeration (HTTP, HTTPS, FTP, MMS) and scheme validation.

use serde::{Deserialize, Serialize};

use super::download_policy::{
    FTP_PROTOCOL, HTTPS_PROTOCOL, HTTP_PROTOCOL, MMSH_PROTOCOL, MMS_PROTOCOL,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadProtocol {
    Http,
    Https,
    Ftp,
    Mms,
}

impl DownloadProtocol {
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            HTTP_PROTOCOL => Some(Self::Http),
            HTTPS_PROTOCOL => Some(Self::Https),
            FTP_PROTOCOL => Some(Self::Ftp),
            MMS_PROTOCOL | MMSH_PROTOCOL => Some(Self::Mms),
            _ => None,
        }
    }
}
