// Created: 2026-11-05
// Reason: feat: implement Phase C browser integration UI, scanner, and review confirmation workflow
// Last updated: 2026-11-10
// Reason: refactor: modularize browser scanner into focused submodules
// Description: Profile inspection logic checking browser preferences and extension installation status.

use std::path::Path;

use super::{EXTENSION_ID, FIREFOX_EXTENSION_ID};

pub fn inspect_profile(profile_dir: &Path) -> (bool, String) {
    let mut settings = serde_json::Map::new();
    let mut removed = Vec::new();

    for file in ["Preferences", "Secure Preferences"] {
        let path = profile_dir.join(file);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(ext) = json.get("extensions") {
                    if let Some(st) = ext.get("settings").and_then(|s| s.as_object()) {
                        for (k, v) in st {
                            settings.insert(k.clone(), v.clone());
                        }
                    }
                    if let Some(uninstalls) = ext.get("external_uninstalls").and_then(|u| u.as_array()) {
                        for item in uninstalls {
                            if let Some(s) = item.as_str() {
                                removed.push(s.to_owned());
                            }
                        }
                    }
                }
            }
        }
    }

    let mut found_entry = EXTENSION_ID.and_then(|id| settings.get(id));
    if found_entry.is_none() {
        for val in settings.values() {
            if let Some(manifest) = val.get("manifest") {
                if manifest.get("name").and_then(|n| n.as_str()) == Some("Open Download Manager Integration") {
                    found_entry = Some(val);
                    break;
                }
            }
        }
    }

    if let Some(entry) = found_entry {
        let reasons = entry.get("disable_reasons");
        let awaiting_approval = match reasons {
            Some(serde_json::Value::Array(arr)) => arr
                .iter()
                .any(|r| r.as_u64().map_or(false, |bits| (bits & 8192) != 0)),
            Some(serde_json::Value::Number(num)) => num.as_u64().map_or(false, |bits| (bits & 8192) != 0),
            _ => false,
        };

        if awaiting_approval {
            return (false, "approval".to_owned());
        }

        let is_disabled = match reasons {
            Some(serde_json::Value::Array(arr)) => !arr.is_empty(),
            Some(serde_json::Value::Number(num)) => num.as_u64().map_or(false, |n| n != 0),
            _ => entry.get("state").and_then(|s| s.as_u64()) == Some(0),
        };

        if is_disabled {
            return (false, "disabled".to_owned());
        }

        let is_connected = entry.get("state").and_then(|s| s.as_u64()) == Some(1)
            || entry.get("manifest").and_then(|m| m.get("version")).is_some();

        if is_connected {
            return (true, "connected".to_owned());
        }
    }

    if let Some(id) = EXTENSION_ID {
        if removed.iter().any(|s| s == id) {
            return (false, "removed".to_owned());
        }
    }

    (false, "missing".to_owned())
}

pub fn inspect_firefox_profile(profile_dir: &Path) -> bool {
    inspect_firefox_profile_status(profile_dir).0
}

pub fn inspect_firefox_profile_status(profile_dir: &Path) -> (bool, String) {
    let extensions_json = profile_dir.join("extensions.json");
    if let Ok(content) = std::fs::read_to_string(&extensions_json) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(addons) = json.get("addons").and_then(|a| a.as_array()) {
                for addon in addons {
                    if addon.get("id").and_then(|s| s.as_str()) == Some(FIREFOX_EXTENSION_ID) {
                        let is_disabled = addon.get("active").and_then(|b| b.as_bool()) == Some(false)
                            || addon.get("appDisabled").and_then(|b| b.as_bool()) == Some(true);
                        if is_disabled {
                            return (false, "disabled".to_owned());
                        }
                        return (true, "connected".to_owned());
                    }
                }
            }
        }
    }
    (false, "missing".to_owned())
}
