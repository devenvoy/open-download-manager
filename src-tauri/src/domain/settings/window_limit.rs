// Created: 2026-11-08
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Last updated: 2026-11-08
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Description: Rolling download volume limit over time window with UTC clock abstraction and quota state.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::validation_error::SettingsValidationError;

/// Injectable UTC clock abstraction for testable time-based rolling limits.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// Standard system wall-clock using `chrono::Utc::now()`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Controllable mock clock for integration testing of quota window transitions and clock shifts.
#[derive(Clone, Debug)]
pub struct MockClock {
    now: Arc<std::sync::RwLock<DateTime<Utc>>>,
}

impl MockClock {
    pub fn new(initial: DateTime<Utc>) -> Self {
        Self {
            now: Arc::new(std::sync::RwLock::new(initial)),
        }
    }

    pub fn set(&self, time: DateTime<Utc>) {
        *self.now.write().unwrap() = time;
    }

    pub fn advance(&self, duration: Duration) {
        let mut lock = self.now.write().unwrap();
        *lock = *lock + duration;
    }
}

impl Clock for MockClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.read().unwrap()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadWindowLimit {
    #[serde(default)]
    pub enabled: bool,
    /// Duration of the rolling window in hours (e.g. 12).
    pub window_hours: u32,
    /// Maximum allowed bytes within the window.
    pub max_bytes: u64,
    /// Anchor timestamp marking the start of the current window in UTC.
    pub window_anchor: DateTime<Utc>,
    /// Accumulated bytes downloaded in the current rolling window.
    #[serde(default)]
    pub current_bytes: u64,
    /// When quota is exceeded, the timestamp until which downloads must be paused in UTC.
    #[serde(default)]
    pub stopped_until: Option<DateTime<Utc>>,
}

impl DownloadWindowLimit {
    pub fn new(window_hours: u32, max_bytes: u64, anchor: DateTime<Utc>) -> Self {
        Self {
            enabled: true,
            window_hours,
            max_bytes,
            window_anchor: anchor,
            current_bytes: 0,
            stopped_until: None,
        }
    }

    pub fn validate(&self) -> Result<(), SettingsValidationError> {
        if self.enabled {
            if self.window_hours == 0 || self.window_hours > 8760 {
                return Err(SettingsValidationError::new(
                    "windowLimit windowHours must be between 1 and 8760 (1 year).",
                ));
            }
            if self.max_bytes == 0 {
                return Err(SettingsValidationError::new(
                    "windowLimit maxBytes must be greater than 0.",
                ));
            }
        }
        Ok(())
    }

    /// Calculates the expiration timestamp of the current window.
    pub fn window_end(&self) -> DateTime<Utc> {
        self.window_anchor + Duration::hours(self.window_hours as i64)
    }

    /// Checks if the window has rolled past `now` and advances it if necessary.
    /// Returns `true` if the window rolled over and usage was reset.
    pub fn roll_if_needed(&mut self, now: DateTime<Utc>) -> bool {
        let window_duration = Duration::hours(self.window_hours.max(1) as i64);
        let end = self.window_anchor + window_duration;

        // If clock jumped backwards before anchor, reset anchor safely
        if now < self.window_anchor {
            self.window_anchor = now;
            self.current_bytes = 0;
            self.stopped_until = None;
            return true;
        }

        if now >= end {
            // How many full window periods elapsed?
            let elapsed_ms = (now - self.window_anchor).num_milliseconds();
            let window_ms = window_duration.num_milliseconds();
            if window_ms > 0 {
                let periods = elapsed_ms / window_ms;
                self.window_anchor = self.window_anchor + Duration::milliseconds(periods * window_ms);
            } else {
                self.window_anchor = now;
            }
            self.current_bytes = 0;
            self.stopped_until = None;
            return true;
        }

        false
    }

    /// Records transferred bytes and checks if quota has been exceeded.
    /// Returns `true` if quota became newly exceeded.
    pub fn record_bytes(&mut self, bytes: u64, now: DateTime<Utc>) -> bool {
        if !self.enabled {
            return false;
        }

        self.roll_if_needed(now);
        self.current_bytes = self.current_bytes.saturating_add(bytes);

        if self.current_bytes >= self.max_bytes {
            if self.stopped_until.is_none() {
                self.stopped_until = Some(self.window_end());
                return true;
            }
        }

        false
    }

    /// Returns `true` if quota is currently exceeded at `now`.
    pub fn is_exceeded(&mut self, now: DateTime<Utc>) -> bool {
        if !self.enabled {
            return false;
        }

        self.roll_if_needed(now);
        self.current_bytes >= self.max_bytes
    }

    /// Formats the resume explanation string for notifications and download notices.
    pub fn explanation(&self, now: DateTime<Utc>) -> String {
        let resume_at = self.stopped_until.unwrap_or_else(|| self.window_end());
        // If resume is within 24 hours of now, format HH:MM
        if (resume_at - now).num_hours() < 24 {
            format!("Quota reached; resumes at {}", resume_at.format("%H:%M"))
        } else {
            format!("Quota reached; resumes at {}", resume_at.format("%Y-%m-%d %H:%M"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_limit_roll_and_quota() {
        let anchor = Utc::now();
        let mut limit = DownloadWindowLimit::new(12, 1000, anchor);

        assert!(!limit.is_exceeded(anchor));

        // Transfer 900 bytes: not exceeded
        assert!(!limit.record_bytes(900, anchor));
        assert!(!limit.is_exceeded(anchor));
        assert!(limit.stopped_until.is_none());

        // Transfer 150 bytes: reaches 1050 >= 1000 -> exceeded!
        assert!(limit.record_bytes(150, anchor));
        assert!(limit.is_exceeded(anchor));
        assert!(limit.stopped_until.is_some());

        let explanation = limit.explanation(anchor);
        assert!(explanation.starts_with("Quota reached; resumes at "));

        // 6 hours later: still in window, still exceeded
        let mid = anchor + Duration::hours(6);
        assert!(!limit.roll_if_needed(mid));
        assert!(limit.is_exceeded(mid));

        // 12 hours later: window expires, rolls, quota resets!
        let next_window = anchor + Duration::hours(12);
        assert!(limit.roll_if_needed(next_window));
        assert!(!limit.is_exceeded(next_window));
        assert_eq!(limit.current_bytes, 0);
        assert!(limit.stopped_until.is_none());
    }

    #[test]
    fn test_clock_jump_backwards() {
        let anchor = Utc::now();
        let mut limit = DownloadWindowLimit::new(12, 1000, anchor);
        limit.current_bytes = 500;

        let past = anchor - Duration::hours(3);
        assert!(limit.roll_if_needed(past));
        assert_eq!(limit.window_anchor, past);
        assert_eq!(limit.current_bytes, 0);
    }

    #[test]
    fn test_validation() {
        let anchor = Utc::now();
        assert!(DownloadWindowLimit::new(12, 1000, anchor).validate().is_ok());
        assert!(DownloadWindowLimit::new(0, 1000, anchor).validate().is_err());
        assert!(DownloadWindowLimit::new(12, 0, anchor).validate().is_err());
    }
}
