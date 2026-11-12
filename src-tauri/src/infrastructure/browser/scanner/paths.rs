// Created: 2026-11-05
// Reason: feat: implement Phase C browser integration UI, scanner, and review confirmation workflow
// Last updated: 2026-11-12
// Reason: fix(browser): resolve the bundled extension and native host from one authoritative location
// Description: Filesystem resolution for browser binaries, the bundled extension, and the native messaging host.

use std::path::{Path, PathBuf};
#[cfg(all(not(windows), not(target_os = "macos")))]
use std::process::Command;

use crate::infrastructure::browser::registration::BrowserType;

/// The native messaging host is bundled next to the desktop executable
/// (the Tauri CLI copies every binary in `src-tauri/src/bin` there).
pub fn resolve_host_exe() -> PathBuf {
    #[cfg(windows)]
    let bin_name = "open-download-manager-native-host.exe";
    #[cfg(not(windows))]
    let bin_name = "open-download-manager-native-host";

    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(bin_name)))
        .unwrap_or_else(|| PathBuf::from(bin_name))
}

/// The bundled browser extension lives inside the app resource directory.
///
/// `bundle.resources` ships two folders next to the app:
///   - `extension`         for Chromium browsers
///   - `extension-firefox` for Firefox (it carries the Firefox manifest)
///
/// The app copies one of them into the user's Downloads folder when the user
/// asks for it, because a browser cannot load an extension from inside an
/// application bundle.
pub fn extension_dir_for(resource_dir: &Path, browser: BrowserType) -> PathBuf {
    let name = if browser.is_firefox() {
        "extension-firefox"
    } else {
        "extension"
    };
    resource_dir.join(name)
}

/// The visible folder the user loads the extension from.
pub fn user_extension_dir(downloads_dir: &Path, browser: BrowserType) -> PathBuf {
    let name = if browser.is_firefox() {
        "extension-firefox"
    } else {
        "extension"
    };
    downloads_dir.join("Open Download Manager Extension").join(name)
}

/// The user's Downloads folder. The extension is delivered there so it can be
/// picked in the browser without hunting through hidden folders.
pub fn user_downloads_dir() -> Option<PathBuf> {
    user_home_dir().map(|home| home.join("Downloads"))
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
                local.as_ref().map(|p| p.join("Mozilla/Firefox/firefox.exe")),
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

#[allow(dead_code)]
pub fn user_home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
        .map(PathBuf::from)
}
