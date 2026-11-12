// Created: 2026-11-12
// Reason: fix(browser): detect a removed extension instead of keeping a stale connected state
// Last updated: 2026-11-12
// Description: Reports whether a browser process is running, used to tell "browser closed" apart from "extension removed".

use std::{
    collections::HashMap,
    process::Command,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use crate::infrastructure::browser::{registration::BrowserType, scanner::find_browser_executable};

/// The process probe is cached briefly so a burst of snapshots does not spawn
/// a process for every browser.
const CACHE_TTL: Duration = Duration::from_secs(20);

fn cache() -> &'static Mutex<HashMap<&'static str, (Instant, bool)>> {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, (Instant, bool)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Reports whether the browser is currently running.
pub fn is_browser_running(browser: BrowserType) -> bool {
    let key = browser.manifest_name();
    let mut cache = cache().lock().unwrap_or_else(|error| error.into_inner());
    if let Some((checked_at, running)) = cache.get(key) {
        if checked_at.elapsed() < CACHE_TTL {
            return *running;
        }
    }
    let running = probe(browser);
    cache.insert(key, (Instant::now(), running));
    running
}

fn probe(browser: BrowserType) -> bool {
    let Some(executable) = find_browser_executable(browser) else {
        return false;
    };

    #[cfg(windows)]
    {
        let image = executable
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        Command::new("tasklist")
            .args(["/FI", &format!("IMAGENAME eq {image}"), "/NH"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).contains(&image))
            .unwrap_or(false)
    }

    #[cfg(not(windows))]
    {
        Command::new("pgrep")
            .arg("-f")
            .arg(&executable)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}
