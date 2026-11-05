use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::application::error::{AppError, AppResult};

pub const NATIVE_HOST_NAME: &str = "com.awesome.downloader";

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
            description: "Awesome Downloader browser bridge".to_owned(),
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
        Ok(NativeHostManifest {
            name: NATIVE_HOST_NAME.to_owned(),
            description: "Awesome Downloader browser bridge".to_owned(),
            path: host_exe_path.to_string_lossy().into_owned(),
            host_type: "stdio".to_owned(),
            allowed_origins: Some(vec![format!("chrome-extension://{extension_id}/")]),
            allowed_extensions: None,
        })
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

    tokio::fs::write(&manifest_path, json_content)
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

    Ok(manifest_path)
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

    Ok(())
}
