// Created: 2026-11-12
// Reason: fix(browser): detect a live extension connection instead of guessing from protected browser profiles
// Last updated: 2026-11-12
// Description: Records which browsers recently talked to the desktop app so integration status reflects reality.

use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::infrastructure::atomic_file;

pub const CONTACTS_FILE_NAME: &str = "browser-state.json";

/// Contacts older than this are dropped from the state file. The connection
/// state itself is sticky: once a browser talked to the app it stays
/// connected, because an uninstalled extension cannot report its removal.
const KEEP_CONTACTS_MS: u64 = 90 * 24 * 60 * 60 * 1000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserContact {
    pub browser: String,
    pub last_seen_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContactStore {
    #[serde(default)]
    contacts: Vec<BrowserContact>,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

/// Normalizes the browser label reported by the extension to the names the
/// scanner uses for Chrome, Edge, Brave, Opera and Firefox.
pub fn normalize_browser_name(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "chrome" | "chromium" | "google chrome" => Some("Chrome"),
        "edge" | "microsoft edge" | "msedge" => Some("Edge"),
        "brave" | "brave browser" => Some("Brave"),
        "opera" | "opera gx" => Some("Opera"),
        "firefox" => Some("Firefox"),
        _ => None,
    }
}

pub fn contacts_path(data_directory: &Path) -> PathBuf {
    data_directory.join(CONTACTS_FILE_NAME)
}

pub fn load_contacts(data_directory: &Path) -> Vec<BrowserContact> {
    let Ok(content) = std::fs::read_to_string(contacts_path(data_directory)) else {
        return Vec::new();
    };
    serde_json::from_str::<ContactStore>(&content)
        .map(|store| store.contacts)
        .unwrap_or_default()
}

fn save_contacts(data_directory: &Path, contacts: &[BrowserContact]) {
    let store = ContactStore {
        contacts: contacts.to_vec(),
    };
    let Ok(json) = serde_json::to_string_pretty(&store) else {
        return;
    };
    let _ = atomic_file::write_replace(&contacts_path(data_directory), json.as_bytes());
}

/// Stores that a browser extension just talked to the desktop app.
pub fn record_contact(data_directory: &Path, browser: &str, version: Option<&str>) {
    let Some(name) = normalize_browser_name(browser) else {
        return;
    };

    let mut contacts = load_contacts(data_directory);
    let stamp = now_ms();
    match contacts.iter_mut().find(|contact| contact.browser == name) {
        Some(existing) => {
            existing.last_seen_ms = stamp;
            if let Some(version) = version {
                existing.version = Some(version.to_owned());
            }
        }
        None => contacts.push(BrowserContact {
            browser: name.to_owned(),
            last_seen_ms: stamp,
            version: version.map(str::to_owned),
        }),
    }

    // Keep the file small: very old contacts are useless.
    contacts.retain(|contact| stamp.saturating_sub(contact.last_seen_ms) <= KEEP_CONTACTS_MS);
    save_contacts(data_directory, &contacts);
}

/// Extracts the sender identity from a native message payload.
pub fn sender_from_payload(payload: &serde_json::Value) -> Option<(String, Option<String>)> {
    let browser = payload.get("browser").and_then(|value| value.as_str())?;
    if normalize_browser_name(browser).is_none() {
        return None;
    }
    let version = payload
        .get("extensionVersion")
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    Some((browser.to_owned(), version))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_directory() -> PathBuf {
        let directory = tempfile::Builder::new()
            .prefix("open-download-manager-contact-test-")
            .tempdir()
            .expect("temporary test directory")
            .keep();
        directory
    }

    #[test]
    fn browser_names_normalize_to_scanner_names() {
        assert_eq!(normalize_browser_name("chrome"), Some("Chrome"));
        assert_eq!(normalize_browser_name("Chrome"), Some("Chrome"));
        assert_eq!(normalize_browser_name("Microsoft Edge"), Some("Edge"));
        assert_eq!(normalize_browser_name("Brave"), Some("Brave"));
        assert_eq!(normalize_browser_name("Opera GX"), Some("Opera"));
        assert_eq!(normalize_browser_name("Firefox"), Some("Firefox"));
        assert_eq!(normalize_browser_name("Safari"), None);
        assert_eq!(normalize_browser_name(""), None);
    }

    #[test]
    fn contacts_are_recorded_and_read_back() {
        let directory = temporary_directory();
        record_contact(&directory, "chrome", Some("0.2.0"));

        let contacts = load_contacts(&directory);
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].browser, "Chrome");
        assert_eq!(contacts[0].version.as_deref(), Some("0.2.0"));
        assert!(contacts.iter().any(|contact| contact.browser == "Chrome"));
        assert!(!contacts.iter().any(|contact| contact.browser == "Firefox"));

        record_contact(&directory, "Chrome", None);
        let contacts = load_contacts(&directory);
        assert_eq!(contacts.len(), 1);

        std::fs::remove_dir_all(directory).ok();
    }

    #[test]
    fn unknown_browsers_are_not_recorded() {
        let directory = temporary_directory();
        record_contact(&directory, "Safari", None);
        assert!(load_contacts(&directory).is_empty());
        std::fs::remove_dir_all(directory).ok();
    }

    #[test]
    fn old_contacts_are_dropped() {
        let directory = temporary_directory();
        let mut store = ContactStore {
            contacts: vec![BrowserContact {
                browser: "Chrome".to_owned(),
                last_seen_ms: now_ms().saturating_sub(KEEP_CONTACTS_MS + 60_000),
                version: None,
            }],
        };
        store.contacts.retain(|contact| {
            now_ms().saturating_sub(contact.last_seen_ms) <= KEEP_CONTACTS_MS
        });
        assert!(store.contacts.is_empty());
        std::fs::remove_dir_all(directory).ok();
    }

    #[test]
    fn payload_sender_is_extracted() {
        let payload = serde_json::json!({
            "action": "heartbeat",
            "browser": "Chrome",
            "extensionVersion": "0.2.0"
        });
        let (browser, version) = sender_from_payload(&payload).expect("sender");
        assert_eq!(browser, "Chrome");
        assert_eq!(version.as_deref(), Some("0.2.0"));

        let payload = serde_json::json!({"action": "heartbeat"});
        assert!(sender_from_payload(&payload).is_none());
    }
}
