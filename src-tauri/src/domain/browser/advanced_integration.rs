// Created: 2026-11-08
// Reason: feat(media): add opt-in advanced browser media adapter (Closes #38)
// Description: Domain capability contract, state machine, and privacy guardrails for opt-in advanced browser media integration.

use serde::{Deserialize, Serialize};

/// Operating system and environment availability for advanced integration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AdvancedIntegrationAvailability {
    Supported {
        platform: String,
        details: String,
    },
    Unsupported {
        platform: String,
        reason: String,
    },
}

impl AdvancedIntegrationAvailability {
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Supported { .. })
    }

    pub fn platform_name(&self) -> &str {
        match self {
            Self::Supported { platform, .. } => platform.as_str(),
            Self::Unsupported { platform, .. } => platform.as_str(),
        }
    }
}

/// Operational state of the advanced browser integration mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum AdvancedIntegrationState {
    /// Disabled by default; ordinary browser integration operates normally.
    Disabled,
    /// User requested activation; explicit consent is required.
    AwaitingConsent,
    /// Successfully enabled, verified, and active.
    Active,
    /// Health check or conflict occurred; automatically rolled back to standard integration.
    Degraded {
        reason: String,
        rolled_back: bool,
    },
    /// The current platform or architecture does not support advanced integration.
    Unsupported {
        reason: String,
    },
}

/// Bounded health verification result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResult {
    pub success: bool,
    pub checked_at: u64,
    pub latency_ms: u64,
    pub conflict_detected: bool,
    pub details: String,
}

impl HealthCheckResult {
    pub fn healthy(latency_ms: u64, details: impl Into<String>) -> Self {
        Self {
            success: true,
            checked_at: current_timestamp(),
            latency_ms,
            conflict_detected: false,
            details: details.into(),
        }
    }

    pub fn failure(details: impl Into<String>) -> Self {
        Self {
            success: false,
            checked_at: current_timestamp(),
            latency_ms: 0,
            conflict_detected: false,
            details: details.into(),
        }
    }

    pub fn conflict(details: impl Into<String>) -> Self {
        Self {
            success: false,
            checked_at: current_timestamp(),
            latency_ms: 0,
            conflict_detected: true,
            details: details.into(),
        }
    }
}

/// Record of an executed rollback action restoring standard integration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackAction {
    pub executed: bool,
    pub reason: String,
    pub standard_integration_intact: bool,
    pub uninstalled: bool,
    pub timestamp: u64,
}

impl RollbackAction {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            executed: true,
            reason: reason.into(),
            standard_integration_intact: true,
            uninstalled: true,
            timestamp: current_timestamp(),
        }
    }
}

/// Full capability contract for advanced browser media integration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedIntegrationCapability {
    pub availability: AdvancedIntegrationAvailability,
    pub required_permission: Option<String>,
    pub state: AdvancedIntegrationState,
    pub last_health_check: Option<HealthCheckResult>,
    pub rollback_action: Option<RollbackAction>,
}

impl AdvancedIntegrationCapability {
    pub fn new(availability: AdvancedIntegrationAvailability) -> Self {
        let state = if availability.is_supported() {
            AdvancedIntegrationState::Disabled
        } else {
            let reason = match &availability {
                AdvancedIntegrationAvailability::Unsupported { reason, .. } => reason.clone(),
                _ => "Unsupported platform".to_owned(),
            };
            AdvancedIntegrationState::Unsupported { reason }
        };

        let required_permission = if availability.is_supported() {
            Some("Network stream observer and player integration permission".to_owned())
        } else {
            None
        };

        Self {
            availability,
            required_permission,
            state,
            last_health_check: None,
            rollback_action: None,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, AdvancedIntegrationState::Active)
    }

    pub fn is_supported(&self) -> bool {
        self.availability.is_supported()
    }

    pub fn can_enable(&self) -> bool {
        self.is_supported() && !self.is_active()
    }
}

/// User-approved media request filter ensuring privacy and isolation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedMediaCandidate {
    pub url: String,
    pub media_type: Option<String>,
    pub user_approved: bool,
    pub filename: Option<String>,
    pub referer: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaFilterError {
    ConsentRequired,
    InvalidUrl(String),
    NotMediaTraffic(String),
    CredentialsEmbedded,
}

impl std::fmt::Display for MediaFilterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConsentRequired => write!(f, "Only explicitly approved media requests can be captured."),
            Self::InvalidUrl(msg) => write!(f, "Invalid media URL: {msg}"),
            Self::NotMediaTraffic(msg) => write!(f, "Unrelated web traffic discarded: {msg}"),
            Self::CredentialsEmbedded => write!(f, "URLs with embedded credentials are not allowed."),
        }
    }
}

impl std::error::Error for MediaFilterError {}

/// Privacy guardrail: validates that the candidate is explicitly user-approved and is genuine media.
/// Drops ambient or unrelated HTTP payloads and scrubs sensitive query tokens.
pub fn filter_advanced_media_candidate(
    candidate: &AdvancedMediaCandidate,
) -> Result<AdvancedMediaCandidate, MediaFilterError> {
    if !candidate.user_approved {
        return Err(MediaFilterError::ConsentRequired);
    }

    let parsed = url::Url::parse(&candidate.url)
        .map_err(|e| MediaFilterError::InvalidUrl(e.to_string()))?;

    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(MediaFilterError::InvalidUrl("Scheme must be HTTP or HTTPS".to_owned()));
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(MediaFilterError::CredentialsEmbedded);
    }

    // Inspect media suitability
    let path = parsed.path().to_ascii_lowercase();
    let is_media_extension = [
        ".mp4", ".m4v", ".mkv", ".webm", ".avi", ".mov", ".flv",
        ".mp3", ".m4a", ".aac", ".ogg", ".opus", ".flac", ".wav",
        ".m3u8", ".mpd", ".ts",
    ]
    .iter()
    .any(|ext| path.ends_with(ext));

    let is_media_mime = candidate.media_type.as_deref().map_or(false, |mime| {
        let m = mime.to_ascii_lowercase();
        m.starts_with("video/")
            || m.starts_with("audio/")
            || m.contains("mpegurl")
            || m.contains("dash+xml")
            || m.contains("octet-stream")
    });

    if !is_media_extension && !is_media_mime {
        return Err(MediaFilterError::NotMediaTraffic(
            "Request does not match known media streaming types or file extensions".to_owned(),
        ));
    }

    // Scrub ephemeral query tokens from URL for clean retention
    let mut sanitized_url = parsed.clone();
    let query_pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(k, _)| {
            let key = k.to_ascii_lowercase();
            !key.contains("token")
                && !key.contains("auth")
                && !key.contains("signature")
                && !key.contains("expires")
                && !key.contains("session")
        })
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if query_pairs.is_empty() {
        sanitized_url.set_query(None);
    } else {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in &query_pairs {
            serializer.append_pair(k, v);
        }
        sanitized_url.set_query(Some(&serializer.finish()));
    }

    Ok(AdvancedMediaCandidate {
        url: sanitized_url.to_string(),
        media_type: candidate.media_type.clone(),
        user_approved: true,
        filename: candidate.filename.clone(),
        referer: candidate.referer.clone(),
    })
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
