use semver::Version;
use serde::{Deserialize, Serialize};

pub const CHECK_INTERVAL_HOURS: u64 = 6;
pub const CHECK_INTERVAL_SECONDS: u64 = CHECK_INTERVAL_HOURS * 3600;
pub const DEFAULT_REPO_OWNER: &str = "devenvoy";
pub const DEFAULT_REPO_NAME: &str = "awesome-downloader";
pub const USER_AGENT: &str = "Awesome-Downloader";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UpdateReleaseInfo {
    pub version: String,
    pub release_notes: Option<String>,
    pub download_url: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateCheckMode {
    Manual,
    Background,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateCheckOutcome {
    UpToDate {
        version: String,
    },
    PromptToDownload(UpdateReleaseInfo),
    NotifyAvailable(UpdateReleaseInfo),
    DevModeUnavailable,
    SilentNoop,
    Error {
        message: String,
        friendly_detail: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallDecision {
    ReadyToInstall,
    WarnActiveDownloads(usize),
}

/// Detects when GitHub reports a missing production release (404/406 or release not found).
pub fn is_no_published_release(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("unable to find latest version on github")
        || (lower.contains("releases/latest")
            && (lower.contains("404") || lower.contains("406") || lower.contains("not found")))
}

/// Classifies errors into user-friendly explanation messages.
pub fn friendly_update_error(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("enotfound")
        || lower.contains("eai_again")
        || lower.contains("econnrefused")
        || lower.contains("etimedout")
        || lower.contains("network")
        || lower.contains("offline")
        || lower.contains("timed out")
        || lower.contains("dns")
        || lower.contains("connection")
        || lower.contains("connect")
    {
        "Could not reach GitHub Releases. Check your internet connection and try again."
    } else {
        "GitHub Releases returned an unexpected response. Please try again later."
    }
}

/// Strips common prefixes like 'v' or 'V' to compare clean semver strings.
pub fn clean_version_tag(tag: &str) -> &str {
    tag.trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
}

/// Returns true if candidate is strictly higher than current.
pub fn is_newer_version(current: &str, candidate: &str) -> bool {
    let current_clean = clean_version_tag(current);
    let candidate_clean = clean_version_tag(candidate);

    match (
        Version::parse(current_clean),
        Version::parse(candidate_clean),
    ) {
        (Ok(curr), Ok(cand)) => cand > curr,
        _ => false,
    }
}
