// Created: 2026-11-10
// Reason: refactor: modularize MMS domain protocol into variant, session, and asx parser submodules
// Description: MMS protocol variants, scheme detection, and migration explanations.

use serde::{Deserialize, Serialize};

/// Variants of the Microsoft Media Server (MMS) protocol family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MmsVariant {
    /// Modern MMS over HTTP framing (supported via dedicated chunk session executor)
    Mmsh,
    /// Generic MMS protocol identifier (negotiated via MMSH session transport)
    Mms,
    /// Obsolete MMS over TCP port 1755 (deprecated by Microsoft, unsupported)
    Mmst,
    /// Obsolete MMS over UDP port 1755 (deprecated by Microsoft, unsupported)
    Mmsu,
}

impl MmsVariant {
    /// Returns the scheme identifier as a static string.
    pub fn scheme_str(&self) -> &'static str {
        match self {
            Self::Mmsh => "mmsh",
            Self::Mms => "mms",
            Self::Mmst => "mmst",
            Self::Mmsu => "mmsu",
        }
    }

    /// Whether this MMS variant is viable and supported via the dedicated MMS session executor.
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Mmsh | Self::Mms)
    }

    /// Actionable migration explanation for unsupported legacy variants.
    pub fn unsupported_guidance(&self) -> &'static str {
        match self {
            Self::Mmst => {
                "MMST (MMS over TCP port 1755) is an obsolete, unmaintained proprietary protocol \
                 deprecated by Microsoft. Direct MMST connections are unsupported due to security \
                 and protocol obsolescence. Please use an HTTP, HTTPS, or RTSP media source, or \
                 extract the stream URL from an ASX/WVX playlist."
            }
            Self::Mmsu => {
                "MMSU (MMS over UDP port 1755) is an obsolete, unmaintained proprietary protocol \
                 deprecated by Microsoft. Direct MMSU connections are unsupported due to security \
                 risks and lack of flow control. Please use an HTTP, HTTPS, or RTSP media source, \
                 or extract the stream URL from an ASX/WVX playlist."
            }
            Self::Mms | Self::Mmsh => "MMS is supported via dedicated MMS streaming session.",
        }
    }
}

/// Protocol boundary detector for identifying MMS schemes and playlist wrappers during URL intake.
pub struct MmsProtocolDetector;

impl MmsProtocolDetector {
    /// Detects whether the scheme corresponds to an MMS protocol variant.
    pub fn detect_scheme(url_str: &str) -> Option<MmsVariant> {
        let trimmed = url_str.trim();
        let colon = trimmed.find(':')?;
        let scheme = &trimmed[..colon].to_ascii_lowercase();
        match scheme.as_str() {
            "mms" => Some(MmsVariant::Mms),
            "mmsh" => Some(MmsVariant::Mmsh),
            "mmst" => Some(MmsVariant::Mmst),
            "mmsu" => Some(MmsVariant::Mmsu),
            _ => None,
        }
    }

    /// Returns true if the URL has an MMS scheme (mms://, mmsh://, mmst://, mmsu://).
    pub fn is_mms(url_str: &str) -> bool {
        Self::detect_scheme(url_str).is_some()
    }

    /// Detects whether the URL points to an ASX or WVX Windows Media playlist wrapper.
    pub fn is_playlist_wrapper(url_str: &str) -> bool {
        let trimmed = url_str.trim();
        let path = trimmed.split('?').next().unwrap_or(trimmed);
        let path_lower = path.to_ascii_lowercase();
        path_lower.ends_with(".asx")
            || path_lower.ends_with(".wvx")
            || path_lower.ends_with(".wmx")
            || path_lower.ends_with(".wax")
    }

    /// Validates an incoming URL for MMS compatibility or yields clear actionable migration guidance.
    pub fn validate_or_explain(url_str: &str) -> Result<MmsVariant, String> {
        if Self::is_playlist_wrapper(url_str) {
            return Err(
                "ASX and WVX playlists are stream redirector wrappers, not direct media files. \
                 Extract the embedded stream URL using the playlist parser or provide the direct media link."
                    .to_string(),
            );
        }

        match Self::detect_scheme(url_str) {
            Some(variant) => {
                if variant.is_supported() {
                    Ok(variant)
                } else {
                    Err(variant.unsupported_guidance().to_string())
                }
            }
            None => Err("The provided address does not use an MMS streaming protocol.".to_string()),
        }
    }
}
