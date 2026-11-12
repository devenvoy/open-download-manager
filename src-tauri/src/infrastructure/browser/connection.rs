// Created: 2026-11-12
// Reason: fix(browser): report an explicit connection state instead of guessing from protected browser profiles
// Last updated: 2026-11-12
// Description: Connection state machine for the browser extension, driven by real native messaging traffic.

use std::time::{SystemTime, UNIX_EPOCH};

/// The extension sends a heartbeat while the desktop app runs. When the
/// browser keeps running but stops answering, the extension was removed or
/// disabled.
pub const FRESH_CONTACT_MS: u64 = 5 * 60 * 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserConnectionState {
    /// The browser is not installed on this computer.
    NotInstalled,
    /// The browser is installed but the extension never talked to the app.
    SetupNeeded,
    /// The extension talked to the app recently.
    Connected,
    /// The extension talked before, but the browser is running and stays
    /// silent: the extension was removed or disabled.
    Disconnected,
}

impl BrowserConnectionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotInstalled => "not-installed",
            Self::SetupNeeded => "setup-needed",
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
        }
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

/// Decides the connection state of a browser.
///
/// `heard_at` is the moment the extension last talked to the desktop app.
/// `browser_running` is probed lazily: it is only needed when the last contact
/// is stale.
pub fn connection_state<F>(
    installed: bool,
    heard_at: Option<u64>,
    now_ms: u64,
    browser_running: F,
) -> BrowserConnectionState
where
    F: FnOnce() -> bool,
{
    if !installed {
        return BrowserConnectionState::NotInstalled;
    }

    match heard_at {
        None => BrowserConnectionState::SetupNeeded,
        Some(last_seen) => {
            if now_ms.saturating_sub(last_seen) <= FRESH_CONTACT_MS {
                BrowserConnectionState::Connected
            } else if browser_running() {
                BrowserConnectionState::Disconnected
            } else {
                // The browser is closed, so it cannot report anything. Keep the
                // last known state instead of nagging the user.
                BrowserConnectionState::Connected
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_000_000_000;

    #[test]
    fn browser_that_is_not_installed_is_never_connected() {
        assert_eq!(
            connection_state(false, None, NOW, || true),
            BrowserConnectionState::NotInstalled
        );
        assert_eq!(
            connection_state(false, Some(NOW), NOW, || false),
            BrowserConnectionState::NotInstalled
        );
    }

    #[test]
    fn installed_browser_that_never_talked_needs_setup() {
        assert_eq!(
            connection_state(true, None, NOW, || true),
            BrowserConnectionState::SetupNeeded
        );
    }

    #[test]
    fn fresh_contact_means_connected() {
        assert_eq!(
            connection_state(true, Some(NOW - 1_000), NOW, || true),
            BrowserConnectionState::Connected
        );
        assert_eq!(
            connection_state(true, Some(NOW - FRESH_CONTACT_MS), NOW, || true),
            BrowserConnectionState::Connected
        );
    }

    #[test]
    fn stale_contact_with_running_browser_means_removed_extension() {
        assert_eq!(
            connection_state(true, Some(NOW - FRESH_CONTACT_MS - 1), NOW, || true),
            BrowserConnectionState::Disconnected
        );
    }

    #[test]
    fn stale_contact_with_closed_browser_keeps_the_last_state() {
        assert_eq!(
            connection_state(true, Some(NOW - FRESH_CONTACT_MS - 1), NOW, || false),
            BrowserConnectionState::Connected
        );
    }

    #[test]
    fn the_browser_probe_is_only_used_for_stale_contacts() {
        let mut probed = false;
        let state = connection_state(true, Some(NOW - 1_000), NOW, || {
            probed = true;
            true
        });
        assert_eq!(state, BrowserConnectionState::Connected);
        assert!(!probed, "a fresh contact must not probe the browser process");
    }
}
