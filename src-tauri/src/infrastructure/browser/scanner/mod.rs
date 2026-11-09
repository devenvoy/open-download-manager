// Created: 2026-11-05
// Reason: feat: implement Phase C browser integration UI, scanner, and review confirmation workflow
// Last updated: 2026-11-10
// Reason: refactor: modularize browser scanner into focused submodules
// Description: Filesystem scanner locating browser binary paths and user profile extension directories.

use std::{path::Path, process::Command};

use crate::{
    application::error::{AppError, AppResult},
    domain::snapshot::{BrowserStatus, ConnectedBrowser, IntegrationStatus},
    infrastructure::browser::registration::BrowserType,
};

pub mod packager;
pub mod paths;
pub mod profile;

pub use packager::*;
pub use paths::*;
pub use profile::*;

pub const EXTENSION_ID: Option<&str> = Some(super::registration::DEFAULT_CHROME_EXTENSION_ID);
pub const STORE_URL: Option<&str> = None;
pub const FIREFOX_EXTENSION_ID: &str = "open-download-manager@opendownloadmanager.local";

pub fn is_store_browser(browser: BrowserType) -> bool {
    STORE_URL.is_some()
        && matches!(
            browser,
            BrowserType::Chrome | BrowserType::Edge | BrowserType::Brave
        )
}

pub fn scan_browsers(
    data_directory: &Path,
    integration_root: &Path,
    bridge_port: Option<u16>,
) -> IntegrationStatus {
    let supported = [
        (BrowserType::Chrome, "Chrome", "chrome://extensions/"),
        (BrowserType::Edge, "Edge", "edge://extensions/"),
        (BrowserType::Brave, "Brave", "brave://extensions/"),
        (BrowserType::Opera, "Opera", "opera://extensions/"),
        (
            BrowserType::Firefox,
            "Firefox",
            "about:debugging#/runtime/this-firefox",
        ),
    ];

    let mut browsers = Vec::new();
    let mut connected_browsers = Vec::new();

    for (b_type, name, ext_page) in supported {
        let exe = find_browser_executable(b_type);
        let installed = exe.is_some();
        let mut connected = false;
        let mut profiles_count = 0;
        let mut setup_state = "missing".to_owned();

        if let Some(data_root) = get_user_data_dir(b_type) {
            if b_type.is_firefox() {
                let mut states = Vec::new();
                if let Ok(entries) = std::fs::read_dir(&data_root) {
                    for entry in entries.flatten() {
                        if entry.path().is_dir() {
                            let (conn, state) = inspect_firefox_profile_status(&entry.path());
                            if conn {
                                connected = true;
                                profiles_count += 1;
                            }
                            states.push(state);
                        }
                    }
                }
                if connected {
                    setup_state = "connected".to_owned();
                } else if states.iter().any(|s| s == "disabled") {
                    setup_state = "disabled".to_owned();
                } else {
                    setup_state = "missing".to_owned();
                }
            } else {
                let mut profile_dirs = Vec::new();
                if b_type == BrowserType::Opera {
                    profile_dirs.push(data_root.clone());
                } else if let Ok(entries) = std::fs::read_dir(&data_root) {
                    for entry in entries.flatten() {
                        let file_name = entry.file_name();
                        let name_str = file_name.to_string_lossy();
                        if entry.path().is_dir()
                            && (name_str == "Default" || name_str.starts_with("Profile "))
                        {
                            profile_dirs.push(entry.path());
                        }
                    }
                }

                let mut states = Vec::new();
                for dir in profile_dirs {
                    let (conn, state) = inspect_profile(&dir);
                    if conn {
                        connected = true;
                        profiles_count += 1;
                    }
                    states.push(state);
                }

                if connected {
                    setup_state = "connected".to_owned();
                } else if states.iter().any(|s| s == "approval") {
                    setup_state = "approval".to_owned();
                } else if states.iter().any(|s| s == "disabled") {
                    setup_state = "disabled".to_owned();
                } else if states.iter().any(|s| s == "removed") {
                    setup_state = "removed".to_owned();
                } else {
                    setup_state = "missing".to_owned();
                }
            }
        }

        let store_url = if is_store_browser(b_type) {
            STORE_URL.map(str::to_owned)
        } else {
            None
        };

        let extension_path = if b_type.is_firefox() {
            data_directory
                .join("NativeMessaging")
                .join("firefox-extension")
                .to_string_lossy()
                .into_owned()
        } else {
            let direct = integration_root.join("extension");
            let data_chrome = data_directory.join("NativeMessaging").join("chrome-extension");
            if direct.join("manifest.json").exists() {
                direct.to_string_lossy().into_owned()
            } else if data_chrome.join("manifest.json").exists() {
                data_chrome.to_string_lossy().into_owned()
            } else {
                direct.to_string_lossy().into_owned()
            }
        };

        if connected {
            connected_browsers.push(ConnectedBrowser {
                browser: name.to_owned(),
                name: name.to_owned(),
                connected: true,
                profiles: profiles_count,
            });
        }

        browsers.push(BrowserStatus {
            name: name.to_owned(),
            installed,
            connected,
            setup_state,
            profiles: profiles_count,
            icon: String::new(),
            extension_page: ext_page.to_owned(),
            store_url,
            extension_path,
            support: "supported".to_owned(),
        });
    }

    let default_extension_path = {
        let direct = integration_root.join("extension");
        let data_chrome = data_directory.join("NativeMessaging").join("chrome-extension");
        if direct.join("manifest.json").exists() {
            direct.to_string_lossy().into_owned()
        } else if data_chrome.join("manifest.json").exists() {
            data_chrome.to_string_lossy().into_owned()
        } else {
            direct.to_string_lossy().into_owned()
        }
    };

    IntegrationStatus {
        port: bridge_port,
        connected_browsers,
        browsers,
        connection_error: None,
        extension_path: default_extension_path,
        integration_path: integration_root
            .join("integration")
            .to_string_lossy()
            .into_owned(),
        packaged: !cfg!(debug_assertions),
        executable: std::env::current_exe()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    }
}

pub fn launch_browser(browser: BrowserType, target_url: Option<&str>) -> AppResult<()> {
    let exe = find_browser_executable(browser)
        .ok_or_else(|| AppError::browser(format!("{:?} was not found.", browser)))?;

    let mut cmd = Command::new(&exe);
    if let Some(url) = target_url {
        cmd.arg(url);
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        cmd.creation_flags(DETACHED_PROCESS);
    }

    cmd.spawn()
        .map_err(|e| AppError::browser(format!("Failed to launch browser: {e}")))?;

    Ok(())
}
