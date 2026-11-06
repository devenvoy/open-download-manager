use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    application::error::{AppError, AppResult},
    domain::snapshot::{BrowserStatus, ConnectedBrowser, IntegrationStatus},
    infrastructure::browser::registration::BrowserType,
};

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

pub fn resolve_integration_root() -> PathBuf {
    if let Ok(dir) = std::env::current_dir() {
        if dir.join("extension").join("manifest.json").exists() {
            return dir;
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            if parent.join("extension").join("manifest.json").exists() {
                return parent.to_path_buf();
            }
            if let Some(grandparent) = parent.parent() {
                if grandparent.join("extension").join("manifest.json").exists() {
                    return grandparent.to_path_buf();
                }
            }
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(parent) = manifest_dir.parent() {
        if parent.join("extension").join("manifest.json").exists() {
            return parent.to_path_buf();
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn resolve_host_exe(integration_root: &Path) -> PathBuf {
    #[cfg(windows)]
    let bin_name = "open-download-manager-native-host.exe";
    #[cfg(not(windows))]
    let bin_name = "open-download-manager-native-host";

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join(bin_name);
            if candidate.exists() {
                return candidate;
            }
        }
    }

    let target_debug = integration_root
        .join("src-tauri")
        .join("target")
        .join("debug")
        .join(bin_name);
    if target_debug.exists() {
        return target_debug;
    }

    let target_release = integration_root
        .join("src-tauri")
        .join("target")
        .join("release")
        .join(bin_name);
    if target_release.exists() {
        return target_release;
    }

    integration_root
        .join("integration")
        .join(if cfg!(windows) {
            "native-host.exe"
        } else {
            "native-host"
        })
}

pub fn find_browser_executable(browser: BrowserType) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let local = std::env::var("LOCALAPPDATA").ok().map(PathBuf::from);
        let prog = std::env::var("ProgramFiles").ok().map(PathBuf::from);
        let prog_x86 = std::env::var("ProgramFiles(x86)").ok().map(PathBuf::from);

        let candidates: Vec<Option<PathBuf>> = match browser {
            BrowserType::Chrome => vec![
                local.as_ref().map(|p| p.join("Google/Chrome/Application/chrome.exe")),
                prog.as_ref().map(|p| p.join("Google/Chrome/Application/chrome.exe")),
                prog_x86.as_ref().map(|p| p.join("Google/Chrome/Application/chrome.exe")),
            ],
            BrowserType::Edge => vec![
                prog_x86.as_ref().map(|p| p.join("Microsoft/Edge/Application/msedge.exe")),
                prog.as_ref().map(|p| p.join("Microsoft/Edge/Application/msedge.exe")),
                local.as_ref().map(|p| p.join("Microsoft/Edge/Application/msedge.exe")),
            ],
            BrowserType::Brave => vec![
                local.as_ref().map(|p| p.join("BraveSoftware/Brave-Browser/Application/brave.exe")),
                prog.as_ref().map(|p| p.join("BraveSoftware/Brave-Browser/Application/brave.exe")),
                prog_x86.as_ref().map(|p| p.join("BraveSoftware/Brave-Browser/Application/brave.exe")),
            ],
            BrowserType::Opera => vec![
                local.as_ref().map(|p| p.join("Programs/Opera/launcher.exe")),
                local.as_ref().map(|p| p.join("Programs/Opera GX/launcher.exe")),
                prog.as_ref().map(|p| p.join("Opera/launcher.exe")),
                prog_x86.as_ref().map(|p| p.join("Opera/launcher.exe")),
            ],
            BrowserType::Firefox => vec![
                prog.as_ref().map(|p| p.join("Mozilla Firefox/firefox.exe")),
                prog_x86.as_ref().map(|p| p.join("Mozilla Firefox/firefox.exe")),
                local.as_ref().map(|p| p.join("Mozilla Firefox/firefox.exe")),
            ],
        };

        for path in candidates.into_iter().flatten() {
            if path.exists() {
                return Some(path);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let candidates: &[&str] = match browser {
            BrowserType::Chrome => &[
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "~/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            ],
            BrowserType::Edge => &[
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
                "~/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            ],
            BrowserType::Brave => &[
                "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
                "~/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
            ],
            BrowserType::Opera => &[
                "/Applications/Opera.app/Contents/MacOS/Opera",
                "~/Applications/Opera.app/Contents/MacOS/Opera",
            ],
            BrowserType::Firefox => &[
                "/Applications/Firefox.app/Contents/MacOS/firefox",
                "~/Applications/Firefox.app/Contents/MacOS/firefox",
                "/Applications/Firefox Developer Edition.app/Contents/MacOS/firefox",
                "~/Applications/Firefox Developer Edition.app/Contents/MacOS/firefox",
                "/Applications/Firefox Nightly.app/Contents/MacOS/firefox",
                "~/Applications/Firefox Nightly.app/Contents/MacOS/firefox",
            ],
        };

        for &raw in candidates {
            let path = if let Some(stripped) = raw.strip_prefix("~/") {
                user_home_dir().map(|h| h.join(stripped))
            } else {
                Some(PathBuf::from(raw))
            };
            if let Some(p) = path {
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let bins: &[&str] = match browser {
            BrowserType::Chrome => &[
                "google-chrome",
                "google-chrome-stable",
                "chromium",
                "chromium-browser",
            ],
            BrowserType::Edge => &[
                "microsoft-edge",
                "microsoft-edge-stable",
                "microsoft-edge-dev",
            ],
            BrowserType::Brave => &["brave-browser", "brave"],
            BrowserType::Opera => &["opera", "opera-beta"],
            BrowserType::Firefox => &["firefox", "firefox-esr", "firefox-bin"],
        };
        for &bin in bins {
            if let Ok(output) = Command::new("which").arg(bin).output() {
                if output.status.success() {
                    let s = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                    if !s.is_empty() {
                        return Some(PathBuf::from(s));
                    }
                }
            }
        }
    }

    None
}

fn user_home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
        .map(PathBuf::from)
}

pub fn get_user_data_dir(browser: BrowserType) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let local = std::env::var("LOCALAPPDATA").ok().map(PathBuf::from);
        let appdata = std::env::var("APPDATA").ok().map(PathBuf::from);
        match browser {
            BrowserType::Chrome => local.map(|p| p.join("Google/Chrome/User Data")),
            BrowserType::Edge => local.map(|p| p.join("Microsoft/Edge/User Data")),
            BrowserType::Brave => local.map(|p| p.join("BraveSoftware/Brave-Browser/User Data")),
            BrowserType::Opera => appdata.map(|p| p.join("Opera Software/Opera Stable")),
            BrowserType::Firefox => appdata.map(|p| p.join("Mozilla/Firefox/Profiles")),
        }
    }

    #[cfg(target_os = "macos")]
    {
        let home = user_home_dir()?;
        let app_support = home.join("Library/Application Support");
        match browser {
            BrowserType::Chrome => Some(app_support.join("Google/Chrome")),
            BrowserType::Edge => Some(app_support.join("Microsoft Edge")),
            BrowserType::Brave => Some(app_support.join("BraveSoftware/Brave-Browser")),
            BrowserType::Opera => Some(app_support.join("com.operasoftware.Opera")),
            BrowserType::Firefox => Some(app_support.join("Firefox/Profiles")),
        }
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let home = user_home_dir()?;
        let config = home.join(".config");
        match browser {
            BrowserType::Chrome => {
                let google_chrome = config.join("google-chrome");
                if google_chrome.exists() {
                    Some(google_chrome)
                } else {
                    let chromium = config.join("chromium");
                    if chromium.exists() {
                        Some(chromium)
                    } else {
                        Some(google_chrome)
                    }
                }
            }
            BrowserType::Edge => Some(config.join("microsoft-edge")),
            BrowserType::Brave => Some(config.join("BraveSoftware/Brave-Browser")),
            BrowserType::Opera => Some(config.join("opera")),
            BrowserType::Firefox => Some(home.join(".mozilla/firefox")),
        }
    }
}

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
            integration_root
                .join("extension")
                .to_string_lossy()
                .into_owned()
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

    IntegrationStatus {
        port: bridge_port,
        connected_browsers,
        browsers,
        connection_error: None,
        extension_path: integration_root
            .join("extension")
            .to_string_lossy()
            .into_owned(),
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

pub fn prepare_firefox_extension(
    data_directory: &Path,
    integration_root: &Path,
) -> std::io::Result<PathBuf> {
    let folder = data_directory.join("NativeMessaging").join("firefox-extension");
    std::fs::create_dir_all(&folder)?;

    let source = integration_root.join("extension");
    if source.exists() {
        copy_dir_recursive(&source, &folder)?;
        let firefox_manifest = folder.join("manifest.firefox.json");
        if firefox_manifest.exists() {
            let target_manifest = folder.join("manifest.json");
            std::fs::copy(&firefox_manifest, &target_manifest)?;
        }
    }

    Ok(folder)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
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
