// Copyright (c) 2026 Open Download Manager contributors
// SPDX-License-Identifier: MIT
// Reason: feat(engine): adaptive learned-cap host limiter with Retry-After and gradual ramp-up (W3/W8)

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const DEFAULT_BACKOFF: Duration = Duration::from_secs(2);
const RAMP_UP_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
struct HostLimitEntry {
    /// Learned maximum concurrency that the server allows without 429
    learned_cap: u8,
    /// When any active backoff expires
    backoff_until: Instant,
    /// Last refusal event timestamp
    last_refusal: Instant,
    /// Consecutive successful transfers
    success_count: u32,
}

#[derive(Debug, Clone, Default)]
pub struct HostLimiter {
    hosts: Arc<Mutex<HashMap<String, HostLimitEntry>>>,
}

impl HostLimiter {
    pub fn new() -> Self {
        Self {
            hosts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Records an HTTP 429 / 503 refusal.
    /// Sets learned cap to the number of connections that were succeeding (`active_concurrency - 1`).
    pub fn record_refusal(
        &self,
        host: &str,
        active_concurrency: u8,
        retry_after: Option<Duration>,
    ) {
        let mut guard = match self.hosts.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let now = Instant::now();
        let backoff = retry_after.unwrap_or(DEFAULT_BACKOFF);
        // If 429 occurred at active_concurrency, the server allows at most active_concurrency - 1
        let safe_cap = active_concurrency.saturating_sub(1).max(1);

        let key = host.to_ascii_lowercase();
        let entry = guard.entry(key).or_insert(HostLimitEntry {
            learned_cap: safe_cap,
            backoff_until: now + backoff,
            last_refusal: now,
            success_count: 0,
        });

        entry.learned_cap = entry.learned_cap.min(safe_cap);
        entry.backoff_until = now + backoff;
        entry.last_refusal = now;
        entry.success_count = 0;
    }

    /// Backwards compatible alias for record_refusal with default backoff.
    pub fn record_penalty(&self, host: &str, current_concurrency: u8) {
        self.record_refusal(host, current_concurrency, None);
    }

    /// Records a successful connection or segment completion for gradual ramp-up.
    pub fn record_success(&self, host: &str) {
        let mut guard = match self.hosts.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let key = host.to_ascii_lowercase();
        if let Some(entry) = guard.get_mut(&key) {
            entry.success_count = entry.success_count.saturating_add(1);
            // If successfully transferring past ramp up interval, gradually probe +1 connection
            if entry.last_refusal.elapsed() >= RAMP_UP_INTERVAL && entry.success_count >= 10 {
                entry.learned_cap = entry.learned_cap.saturating_add(1);
                entry.last_refusal = Instant::now();
                entry.success_count = 0;
            }
        }
    }

    /// Resolves the effective concurrency limit for a host, factoring in learned cap and backoff.
    pub fn resolve_concurrency(&self, host: &str, desired: u8) -> u8 {
        let mut guard = match self.hosts.lock() {
            Ok(g) => g,
            Err(_) => return desired,
        };

        let key = host.to_ascii_lowercase();
        if let Some(entry) = guard.get_mut(&key) {
            let now = Instant::now();
            if now < entry.backoff_until {
                // Under active Retry-After backoff, throttle to single connection
                return 1;
            }
            // After cooldown, allow learned cap
            return desired.min(entry.learned_cap);
        }

        desired
    }

    /// Checks if a host is currently under an active Retry-After backoff.
    pub fn is_backing_off(&self, host: &str) -> Option<Duration> {
        let guard = self.hosts.lock().ok()?;
        let entry = guard.get(&host.to_ascii_lowercase())?;
        let now = Instant::now();
        if now < entry.backoff_until {
            Some(entry.backoff_until - now)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_limiter_learns_cap_and_settles() {
        let limiter = HostLimiter::new();
        let host = "cdn.example.com";

        // Desired 8 connections before any refusal
        assert_eq!(limiter.resolve_concurrency(host, 8), 8);

        // Server refuses when 4 connections are active -> learned cap becomes 3
        limiter.record_refusal(host, 4, Some(Duration::from_millis(50)));
        assert_eq!(limiter.resolve_concurrency(host, 8), 1); // in backoff

        std::thread::sleep(Duration::from_millis(60));
        // Backoff expired: settles at learned cap = 3
        assert_eq!(limiter.resolve_concurrency(host, 8), 3);
        assert_eq!(limiter.resolve_concurrency(host, 2), 2);
    }

    #[test]
    fn test_host_limiter_gradual_ramp_up() {
        let limiter = HostLimiter::new();
        let host = "cdn.example.com";

        limiter.record_refusal(host, 3, Some(Duration::from_millis(1)));
        std::thread::sleep(Duration::from_millis(5));

        // Learned cap is 2
        assert_eq!(limiter.resolve_concurrency(host, 8), 2);

        // Simulate successful chunks
        for _ in 0..15 {
            limiter.record_success(host);
        }

        // Wait past ramp interval or trigger ramp
        if let Ok(mut g) = limiter.hosts.lock() {
            if let Some(e) = g.get_mut(host) {
                e.last_refusal -= Duration::from_secs(20);
            }
        }
        limiter.record_success(host);
        // Probed to 3
        assert_eq!(limiter.resolve_concurrency(host, 8), 3);
    }
}
