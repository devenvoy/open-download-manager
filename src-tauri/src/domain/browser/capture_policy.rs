use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfirmationMode {
    Automatic,
    AskForUnknown,
    AlwaysAsk,
}

impl Default for ConfirmationMode {
    fn default() -> Self {
        Self::AskForUnknown
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", content = "reason", rename_all = "kebab-case")]
pub enum CaptureDecision {
    Allowed,
    ExcludedHost(String),
    ExcludedAddress(String),
    TypeNotEnabled(String),
    ConfirmationRequired(String),
}

impl CaptureDecision {
    pub fn decision_str(&self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::ExcludedHost(_) => "excluded-host",
            Self::ExcludedAddress(_) => "excluded-address",
            Self::TypeNotEnabled(_) => "type-not-enabled",
            Self::ConfirmationRequired(_) => "confirmation-required",
        }
    }

    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed | Self::ConfirmationRequired(_))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturePolicy {
    #[serde(default = "default_file_types")]
    pub file_types: Vec<String>,
    #[serde(default)]
    pub excluded_hosts: Vec<String>,
    #[serde(default)]
    pub excluded_addresses: Vec<String>,
    #[serde(default)]
    pub confirmation_mode: ConfirmationMode,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

pub fn default_file_types() -> Vec<String> {
    vec![
        "3gp", "7z", "apk", "avi", "bz2", "deb", "dmg", "exe", "flv", "gz", "iso", "mkv",
        "mov", "mp3", "mp4", "mpeg", "mpg", "msi", "pdf", "pkg", "rar", "rpm", "tar", "tgz",
        "wav", "webm", "wma", "wmv", "zip",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

impl Default for CapturePolicy {
    fn default() -> Self {
        Self {
            file_types: default_file_types(),
            excluded_hosts: Vec::new(),
            excluded_addresses: Vec::new(),
            confirmation_mode: ConfirmationMode::default(),
            enabled: true,
        }
    }
}

impl CapturePolicy {
    pub fn evaluate(&self, url_str: &str, filename: Option<&str>) -> CaptureDecision {
        if !self.enabled {
            return CaptureDecision::TypeNotEnabled("Capture rules disabled".to_owned());
        }

        let parsed_url = match Url::parse(url_str) {
            Ok(u) => u,
            Err(_) => {
                return CaptureDecision::TypeNotEnabled("Invalid URL format".to_owned());
            }
        };

        let host = match parsed_url.host_str() {
            Some(h) => h.to_ascii_lowercase(),
            None => {
                return CaptureDecision::TypeNotEnabled("Missing URL host".to_owned());
            }
        };

        // 1. Host exclusion check
        for pattern in &self.excluded_hosts {
            let pat = pattern.trim().to_ascii_lowercase();
            if pat.is_empty() {
                continue;
            }
            if matches_host(&host, &pat) {
                return CaptureDecision::ExcludedHost(format!("Host '{host}' matches exclusion '{pattern}'"));
            }
        }

        // 2. Address / Path exclusion check
        let path = parsed_url.path();
        let full_url_lower = url_str.to_ascii_lowercase();
        let no_scheme = full_url_lower
            .strip_prefix("https://")
            .or_else(|| full_url_lower.strip_prefix("http://"))
            .unwrap_or(&full_url_lower);
        for pattern in &self.excluded_addresses {
            let pat = pattern.trim().to_ascii_lowercase();
            if pat.is_empty() {
                continue;
            }
            if full_url_lower.starts_with(&pat)
                || path.to_ascii_lowercase().starts_with(&pat)
                || host == pat
                || no_scheme.starts_with(&pat)
            {
                return CaptureDecision::ExcludedAddress(format!("Address matches exclusion '{pattern}'"));
            }
        }

        // 3. AlwaysAsk mode check
        if self.confirmation_mode == ConfirmationMode::AlwaysAsk {
            return CaptureDecision::ConfirmationRequired("Confirmation mode is set to AlwaysAsk".to_owned());
        }

        // 4. File extension check
        let ext = extract_extension(filename, path);
        match ext {
            Some(extension) => {
                let norm_ext = extension.trim_start_matches('.').to_ascii_lowercase();
                if self.file_types.iter().any(|t| t.trim_start_matches('.').eq_ignore_ascii_case(&norm_ext)) {
                    CaptureDecision::Allowed
                } else if self.confirmation_mode == ConfirmationMode::AskForUnknown {
                    CaptureDecision::ConfirmationRequired(format!("Extension '.{norm_ext}' requires confirmation"))
                } else {
                    CaptureDecision::TypeNotEnabled(format!("Extension '.{norm_ext}' not in inclusion list"))
                }
            }
            None => {
                if self.confirmation_mode == ConfirmationMode::AskForUnknown {
                    CaptureDecision::ConfirmationRequired("Unknown file type requires confirmation".to_owned())
                } else {
                    CaptureDecision::TypeNotEnabled("No recognized file extension".to_owned())
                }
            }
        }
    }
}

fn matches_host(host: &str, pattern: &str) -> bool {
    let pat = pattern.trim_start_matches("*.");
    if host == pat || pattern == "*" {
        return true;
    }
    if host.ends_with(&format!(".{pat}")) {
        return true;
    }
    false
}

fn extract_extension<'a>(filename: Option<&'a str>, path: &'a str) -> Option<&'a str> {
    if let Some(name) = filename {
        if let Some(idx) = name.rfind('.') {
            let ext = &name[idx + 1..];
            if !ext.is_empty() && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Some(ext);
            }
        }
    }

    let segment = path.rsplit('/').next().unwrap_or("");
    if let Some(idx) = segment.rfind('.') {
        let ext = &segment[idx + 1..];
        if !ext.is_empty() && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Some(ext);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_policy_defaults() {
        let policy = CapturePolicy::default();
        assert!(policy.enabled);
        assert_eq!(policy.confirmation_mode, ConfirmationMode::AskForUnknown);
        assert!(policy.file_types.contains(&"zip".to_string()));
    }

    #[test]
    fn test_host_exclusions() {
        let mut policy = CapturePolicy::default();
        policy.excluded_hosts = vec!["bank.example.test".to_owned(), "*.internal.corp".to_owned()];

        assert_eq!(
            policy.evaluate("https://bank.example.test/statement.pdf", None).decision_str(),
            "excluded-host"
        );
        assert_eq!(
            policy.evaluate("https://portal.internal.corp/app.zip", None).decision_str(),
            "excluded-host"
        );
        assert_eq!(
            policy.evaluate("https://public.example.test/download.zip", None).decision_str(),
            "allowed"
        );
    }

    #[test]
    fn test_address_exclusions() {
        let mut policy = CapturePolicy::default();
        policy.excluded_addresses = vec!["/private/".to_owned(), "https://example.test/secret".to_owned()];

        assert_eq!(
            policy.evaluate("https://example.test/private/doc.pdf", None).decision_str(),
            "excluded-address"
        );
        assert_eq!(
            policy.evaluate("https://example.test/secret_data.zip", None).decision_str(),
            "excluded-address"
        );
        assert_eq!(
            policy.evaluate("https://example.test/public/doc.pdf", None).decision_str(),
            "allowed"
        );
    }

    #[test]
    fn test_confirmation_modes() {
        let mut policy = CapturePolicy::default();
        policy.confirmation_mode = ConfirmationMode::AlwaysAsk;
        assert_eq!(
            policy.evaluate("https://example.test/archive.zip", None).decision_str(),
            "confirmation-required"
        );

        policy.confirmation_mode = ConfirmationMode::AskForUnknown;
        assert_eq!(
            policy.evaluate("https://example.test/unknown_file.xyz", None).decision_str(),
            "confirmation-required"
        );
        assert_eq!(
            policy.evaluate("https://example.test/archive.zip", None).decision_str(),
            "allowed"
        );

        policy.confirmation_mode = ConfirmationMode::Automatic;
        assert_eq!(
            policy.evaluate("https://example.test/unknown_file.xyz", None).decision_str(),
            "type-not-enabled"
        );
        assert_eq!(
            policy.evaluate("https://example.test/archive.zip", None).decision_str(),
            "allowed"
        );
    }
}
