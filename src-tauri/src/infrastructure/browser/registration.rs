// Created: 2026-11-05
// Reason: feat: add native messaging host, loopback bridge, and browser registration
// Last updated: 2026-11-08
// Reason: refactor(core): elevate transfer architecture with ThroughputMeter, serialized queue pump mutex, centralized extension config, and connection progress metrics
// Reason: fix(browser): sync and repair native messaging host manifests with active executable path
// Description: Manifest installation logic writing browser-specific native messaging JSON files.

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::application::error::{AppError, AppResult};

pub const NATIVE_HOST_NAME: &str = "com.open.downloadmanager";
pub const DEFAULT_CHROME_EXTENSION_ID: &str = "hlomaolpnkkbikjacplamegklgegpjbj";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserType {
    Chrome,
    Edge,
    Brave,
    Opera,
    Firefox,
}

impl BrowserType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "chrome" => Some(Self::Chrome),
            "edge" => Some(Self::Edge),
            "brave" => Some(Self::Brave),
            "opera" => Some(Self::Opera),
            "firefox" => Some(Self::Firefox),
            _ => None,
        }
    }

    pub fn registry_key(&self) -> &'static str {
        match self {
            Self::Chrome | Self::Opera => r"Google\Chrome",
            Self::Edge => r"Microsoft\Edge",
            Self::Brave => r"BraveSoftware\Brave-Browser",
            Self::Firefox => r"Mozilla",
        }
    }

    pub fn is_firefox(&self) -> bool {
        matches!(self, Self::Firefox)
    }

    pub fn manifest_name(&self) -> &'static str {
        match self {
            Self::Chrome | Self::Opera => "Chrome",
            Self::Edge => "Edge",
            Self::Brave => "Brave",
            Self::Firefox => "Firefox",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NativeHostManifest {
    pub name: String,
    pub description: String,
    pub path: String,
    #[serde(rename = "type")]
    pub host_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_origins: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_extensions: Option<Vec<String>>,
}

pub fn create_manifest(
    browser: BrowserType,
    extension_id: &str,
    host_exe_path: &Path,
) -> AppResult<NativeHostManifest> {
    if browser.is_firefox() {
        let is_valid = !extension_id.is_empty()
            && extension_id.len() <= 128
            && extension_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | '{' | '}' | '-'));
        if !is_valid {
            return Err(AppError::browser("Copy the extension ID from your browser’s extensions page."));
        }
        Ok(NativeHostManifest {
            name: NATIVE_HOST_NAME.to_owned(),
            description: "Open Download Manager browser bridge".to_owned(),
            path: host_exe_path.to_string_lossy().into_owned(),
            host_type: "stdio".to_owned(),
            allowed_origins: None,
            allowed_extensions: Some(vec![extension_id.to_owned()]),
        })
    } else {
        let is_valid = extension_id.len() == 32
            && extension_id.chars().all(|c| ('a'..='p').contains(&c));
        if !is_valid {
            return Err(AppError::browser("Copy the extension ID from your browser’s extensions page."));
        }
        let mut origins = vec![
            format!("chrome-extension://{DEFAULT_CHROME_EXTENSION_ID}/"),
            format!("chrome-extension://{extension_id}/"),
        ];
        origins.sort();
        origins.dedup();
        Ok(NativeHostManifest {
            name: NATIVE_HOST_NAME.to_owned(),
            description: "Open Download Manager browser bridge".to_owned(),
            path: host_exe_path.to_string_lossy().into_owned(),
            host_type: "stdio".to_owned(),
            allowed_origins: Some(origins),
            allowed_extensions: None,
        })
    }
}

/// Resolves the OS-specific native messaging hosts directory for a given browser
pub fn browser_native_messaging_dir(browser: BrowserType, home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let app_support = home.join("Library/Application Support");
        match browser {
            BrowserType::Chrome => app_support.join("Google/Chrome/NativeMessagingHosts"),
            BrowserType::Edge => app_support.join("Microsoft Edge/NativeMessagingHosts"),
            BrowserType::Brave => app_support.join("BraveSoftware/Brave-Browser/NativeMessagingHosts"),
            BrowserType::Opera => app_support.join("com.operasoftware.Opera/NativeMessagingHosts"),
            BrowserType::Firefox => app_support.join("Mozilla/NativeMessagingHosts"),
        }
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        match browser {
            BrowserType::Firefox => home.join(".mozilla/native-messaging-hosts"),
            BrowserType::Chrome => home.join(".config/google-chrome/NativeMessagingHosts"),
            BrowserType::Edge => home.join(".config/microsoft-edge/NativeMessagingHosts"),
            BrowserType::Brave => home.join(".config/BraveSoftware/Brave-Browser/NativeMessagingHosts"),
            BrowserType::Opera => home.join(".config/opera/NativeMessagingHosts"),
        }
    }
    #[cfg(windows)]
    {
        let _ = browser;
        home.to_path_buf()
    }
}

pub async fn register_browser_manifest(
    browser: BrowserType,
    extension_id: &str,
    data_directory: &Path,
    host_exe_path: &Path,
) -> AppResult<PathBuf> {
    let manifest = create_manifest(browser, extension_id, host_exe_path)?;
    let folder = data_directory.join("NativeMessaging");
    tokio::fs::create_dir_all(&folder)
        .await
        .map_err(|e| AppError::browser(format!("Could not create NativeMessaging folder: {e}")))?;

    let manifest_path = folder.join(format!("{}.json", browser.manifest_name()));
    let json_content = serde_json::to_string_pretty(&manifest)
        .map_err(|e| AppError::browser(format!("Could not serialize manifest: {e}")))?;

    tokio::fs::write(&manifest_path, &json_content)
        .await
        .map_err(|e| AppError::browser(format!("Could not write manifest: {e}")))?;

    #[cfg(windows)]
    {
        let key = format!(
            r"HKCU\Software\{}\NativeMessagingHosts\{}",
            browser.registry_key(),
            NATIVE_HOST_NAME
        );
        let status = tokio::process::Command::new("reg.exe")
            .arg("ADD")
            .arg(&key)
            .arg("/ve")
            .arg("/t")
            .arg("REG_SZ")
            .arg("/d")
            .arg(&manifest_path)
            .arg("/f")
            .status()
            .await
            .map_err(|e| AppError::browser(format!("Could not run reg.exe: {e}")))?;

        if !status.success() {
            return Err(AppError::browser("Could not register native messaging host in Windows registry."));
        }
    }

    #[cfg(not(windows))]
    {
        let is_test = data_directory.to_string_lossy().contains("open-download-manager-")
            || data_directory.to_string_lossy().contains("test");
        if !is_test {
            if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
                let nm_dir = browser_native_messaging_dir(browser, &home);
                if let Ok(_) = tokio::fs::create_dir_all(&nm_dir).await {
                    let target_file = nm_dir.join(format!("{NATIVE_HOST_NAME}.json"));
                    let mut final_manifest = manifest.clone();
                    if let Ok(existing_bytes) = tokio::fs::read(&target_file).await {
                        if let Ok(existing_manifest) = serde_json::from_slice::<NativeHostManifest>(&existing_bytes) {
                            if let Some(existing_origins) = existing_manifest.allowed_origins {
                                if let Some(origins) = &mut final_manifest.allowed_origins {
                                    origins.extend(existing_origins);
                                    origins.sort();
                                    origins.dedup();
                                }
                            }
                            if let Some(existing_exts) = existing_manifest.allowed_extensions {
                                if let Some(exts) = &mut final_manifest.allowed_extensions {
                                    exts.extend(existing_exts);
                                    exts.sort();
                                    exts.dedup();
                                }
                            }
                        }
                    }
                    if let Ok(final_json) = serde_json::to_string_pretty(&final_manifest) {
                        let _ = tokio::fs::write(&target_file, &final_json).await;
                    }
                }
            }
        }
    }

    Ok(manifest_path)
}

/// Synchronizes and repairs native messaging host manifests across installed browsers.
/// Ensures the executable path points to the active `host_exe_path` and preserves existing origins.
pub fn sync_browser_manifests(
    data_directory: &Path,
    host_exe_path: &Path,
) -> Vec<BrowserType> {
    let mut updated = Vec::new();
    let browsers = [
        BrowserType::Chrome,
        BrowserType::Edge,
        BrowserType::Brave,
        BrowserType::Opera,
        BrowserType::Firefox,
    ];

    let host_str = host_exe_path.to_string_lossy().into_owned();

    // 1. Sync internal data directory manifests (used directly on Windows registry)
    let folder = data_directory.join("NativeMessaging");
    if folder.exists() {
        for &browser in &browsers {
            let manifest_path = folder.join(format!("{}.json", browser.manifest_name()));
            if manifest_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                    if let Ok(mut manifest) = serde_json::from_str::<NativeHostManifest>(&content) {
                        let current = PathBuf::from(&manifest.path);
                        if !current.exists() || current != host_exe_path {
                            manifest.path = host_str.clone();
                            if let Ok(json) = serde_json::to_string_pretty(&manifest) {
                                let _ = std::fs::write(&manifest_path, json);
                                updated.push(browser);
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Sync OS browser native messaging directories (macOS & Linux)
    #[cfg(not(windows))]
    {
        let is_test = data_directory.to_string_lossy().contains("open-download-manager-")
            || data_directory.to_string_lossy().contains("test");
        if !is_test {
            if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
                for &browser in &browsers {
                    let nm_dir = browser_native_messaging_dir(browser, &home);
                    let target_file = nm_dir.join(format!("{NATIVE_HOST_NAME}.json"));

                    if target_file.exists() {
                        if let Ok(content) = std::fs::read_to_string(&target_file) {
                            if let Ok(mut manifest) = serde_json::from_str::<NativeHostManifest>(&content) {
                                let current = PathBuf::from(&manifest.path);
                                if !current.exists() || current != host_exe_path {
                                    manifest.path = host_str.clone();
                                    if let Ok(json) = serde_json::to_string_pretty(&manifest) {
                                        let _ = std::fs::write(&target_file, json);
                                        if !updated.contains(&browser) {
                                            updated.push(browser);
                                        }
                                    }
                                }
                            }
                        }
                    } else if nm_dir.exists() {
                        let default_ext_id = if browser.is_firefox() {
                            crate::infrastructure::browser::FIREFOX_EXTENSION_ID
                        } else {
                            DEFAULT_CHROME_EXTENSION_ID
                        };
                        if let Ok(manifest) = create_manifest(browser, default_ext_id, host_exe_path) {
                            if let Ok(json) = serde_json::to_string_pretty(&manifest) {
                                let _ = std::fs::write(&target_file, json);
                                if !updated.contains(&browser) {
                                    updated.push(browser);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    updated
}

pub async fn unregister_browser_manifest(
    browser: BrowserType,
    data_directory: &Path,
) -> AppResult<()> {
    let manifest_path = data_directory
        .join("NativeMessaging")
        .join(format!("{}.json", browser.manifest_name()));
    let _ = tokio::fs::remove_file(manifest_path).await;

    #[cfg(windows)]
    {
        let key = format!(
            r"HKCU\Software\{}\NativeMessagingHosts\{}",
            browser.registry_key(),
            NATIVE_HOST_NAME
        );
        let _ = tokio::process::Command::new("reg.exe")
            .arg("DELETE")
            .arg(&key)
            .arg("/f")
            .status()
            .await;
    }

    #[cfg(not(windows))]
    {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            let nm_dir = browser_native_messaging_dir(browser, &home);
            let _ = tokio::fs::remove_file(nm_dir.join(format!("{NATIVE_HOST_NAME}.json"))).await;
        }
    }

    Ok(())
}
