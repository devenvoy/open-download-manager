// Created: 2026-11-05
// Reason: feat: implement Phase C browser integration UI, scanner, and review confirmation workflow
// Last updated: 2026-11-10
// Reason: refactor: modularize browser scanner into focused submodules
// Description: Filesystem resolution for browser binaries, integration paths, and user data directories.

use std::path::{Path, PathBuf};
#[cfg(all(not(windows), not(target_os = "macos")))]
use std::process::Command;

use crate::infrastructure::browser::registration::BrowserType;

pub fn resolve_integration_root() -> PathBuf {
    if let Ok(dir) = std::env::current_dir() {
        if dir.join("extension").join("manifest.json").exists() {
            return dir;
        }
        if dir.join("_up_").join("extension").join("manifest.json").exists() {
            return dir.join("_up_");
        }
        if dir.join("resources").join("extension").join("manifest.json").exists() {
            return dir.join("resources");
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            if parent.join("extension").join("manifest.json").exists() {
                return parent.to_path_buf();
            }
            if parent.join("_up_").join("extension").join("manifest.json").exists() {
                return parent.join("_up_");
            }
            if parent.join("resources").join("extension").join("manifest.json").exists() {
                return parent.join("resources");
            }
            if let Some(grandparent) = parent.parent() {
                if grandparent.join("extension").join("manifest.json").exists() {
                    return grandparent.to_path_buf();
                }
                if grandparent.join("_up_").join("extension").join("manifest.json").exists() {
                    return grandparent.join("_up_");
                }
                if grandparent.join("resources").join("extension").join("manifest.json").exists() {
                    return grandparent.join("resources");
                }
            }
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(parent) = manifest_dir.parent() {
        if parent.join("extension").join("manifest.json").exists() {
            return parent.to_path_buf();
        }
        if parent.join("_up_").join("extension").join("manifest.json").exists() {
            return parent.join("_up_");
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
            if let Some(grandparent) = parent.parent() {
                let candidate = grandparent.join(bin_name);
                if candidate.exists() {
                    return candidate;
                }
            }
        }
    }

    let candidate = integration_root.join(bin_name);
    if candidate.exists() {
        return candidate;
    }
    if let Some(parent) = integration_root.parent() {
        let candidate = parent.join(bin_name);
        if candidate.exists() {
            return candidate;
        }
    }

    let root_target_debug = integration_root
        .join("target")
        .join("debug")
        .join(bin_name);
    if root_target_debug.exists() {
        return root_target_debug;
    }

    let root_target_release = integration_root
        .join("target")
        .join("release")
        .join(bin_name);
    if root_target_release.exists() {
        return root_target_release;
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

#[allow(dead_code)]
pub fn user_home_dir() -> Option<PathBuf> {
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
