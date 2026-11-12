// Created: 2026-11-05
// Reason: feat: implement Phase C browser integration UI, scanner, and review confirmation workflow
// Last updated: 2026-11-12
// Reason: fix(browser): report an explicit connection state driven by real extension traffic
// Description: Filesystem scanner locating browser binaries and reporting extension connection states.

use std::{path::Path, process::Command};

use crate::{
    application::error::{AppError, AppResult},
    domain::snapshot::{BrowserStatus, ConnectedBrowser, IntegrationStatus},
    infrastructure::browser::{
        connection::{connection_state, BrowserConnectionState},
        contact,
        registration::BrowserType,
    },
};

pub mod paths;
pub mod process;

pub use paths::*;
pub use process::*;

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

/// Reports the installed browsers and the connection state of their
/// extension.
///
/// The state comes from real native messaging traffic. Reading the browser
/// profile folder is not used: macOS blocks other applications from reading
/// the Chrome and Edge profile folders, so a profile check reports "missing"
/// even for a fully working extension.
pub fn scan_browsers(
    data_directory: &Path,
    resource_dir: &Path,
    downloads_dir: Option<&Path>,
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

    let contacts = contact::load_contacts(data_directory);

    let mut browsers = Vec::new();
    let mut connected_browsers = Vec::new();

    for (b_type, name, ext_page) in supported {
        let installed = find_browser_executable(b_type).is_some();
        let contact = contacts.iter().find(|contact| contact.browser == name);
        let last_seen_ms = contact.map(|contact| contact.last_seen_ms);

        let state = connection_state(installed, last_seen_ms, contact::now_ms(), || {
            is_browser_running(b_type)
        });
        let connected = state == BrowserConnectionState::Connected;

        let store_url = if is_store_browser(b_type) {
            STORE_URL.map(str::to_owned)
        } else {
            None
        };

        // The folder the user loads the extension from: a visible folder in
        // the Downloads directory.
        let extension_dir = downloads_dir
            .map(|downloads| user_extension_dir(downloads, b_type))
            .unwrap_or_else(|| extension_dir_for(resource_dir, b_type));
        let extension_available = extension_dir.join("manifest.json").exists();

        if connected {
            connected_browsers.push(ConnectedBrowser {
                browser: name.to_owned(),
                name: name.to_owned(),
                connected: true,
            });
        }

        browsers.push(BrowserStatus {
            name: name.to_owned(),
            installed,
            connected,
            setup_state: state.as_str().to_owned(),
            icon: String::new(),
            extension_page: ext_page.to_owned(),
            store_url,
            extension_path: extension_dir.to_string_lossy().into_owned(),
            extension_available,
            last_seen_ms,
            support: "supported".to_owned(),
        });
    }

    let default_extension_dir = downloads_dir
        .map(|downloads| user_extension_dir(downloads, BrowserType::Chrome))
        .unwrap_or_else(|| extension_dir_for(resource_dir, BrowserType::Chrome));
    let extension_available = default_extension_dir.join("manifest.json").exists();

    IntegrationStatus {
        port: bridge_port,
        connected_browsers,
        browsers,
        connection_error: None,
        extension_path: default_extension_dir.to_string_lossy().into_owned(),
        extension_available,
        integration_path: resource_dir.to_string_lossy().into_owned(),
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
