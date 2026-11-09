// Created: 2026-11-08
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Last updated: 2026-11-08
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Description: Application quota management and rolling download volume enforcement service.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, RwLock},
};

use chrono::{DateTime, Utc};

use crate::domain::settings::{Clock, DownloadWindowLimit, SystemClock};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuotaStatus {
    Allowed,
    Exceeded {
        resume_at: DateTime<Utc>,
        explanation: String,
    },
}

pub type PersistenceHook = Arc<dyn Fn(&DownloadWindowLimit) + Send + Sync>;

#[derive(Clone)]
pub struct QuotaEnforcementService {
    limit: Arc<RwLock<Option<DownloadWindowLimit>>>,
    clock: Arc<dyn Clock>,
    last_reported_bytes: Arc<Mutex<HashMap<String, u64>>>,
    quota_paused_ids: Arc<Mutex<HashSet<String>>>,
    persistence_hook: Option<PersistenceHook>,
}

impl QuotaEnforcementService {
    pub fn new(initial_limit: Option<DownloadWindowLimit>) -> Self {
        Self::with_clock_and_persistence(initial_limit, Arc::new(SystemClock), None)
    }

    pub fn with_clock(
        initial_limit: Option<DownloadWindowLimit>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self::with_clock_and_persistence(initial_limit, clock, None)
    }

    pub fn with_clock_and_persistence(
        initial_limit: Option<DownloadWindowLimit>,
        clock: Arc<dyn Clock>,
        persistence_hook: Option<PersistenceHook>,
    ) -> Self {
        Self {
            limit: Arc::new(RwLock::new(initial_limit)),
            clock,
            last_reported_bytes: Arc::new(Mutex::new(HashMap::new())),
            quota_paused_ids: Arc::new(Mutex::new(HashSet::new())),
            persistence_hook,
        }
    }

    pub fn set_persistence_hook(&mut self, hook: PersistenceHook) {
        self.persistence_hook = Some(hook);
    }

    /// Updates the configured download window limit from new settings.
    pub fn update_limit(&self, new_limit: Option<DownloadWindowLimit>) {
        let mut lock = self.limit.write().unwrap();
        let now = self.clock.now();

        if let Some(mut updated) = new_limit {
            if let Some(existing) = lock.as_ref() {
                // Preserve current rolling counters if same window parameters and still valid
                if existing.window_hours == updated.window_hours && existing.window_anchor == updated.window_anchor {
                    updated.current_bytes = existing.current_bytes;
                    updated.stopped_until = existing.stopped_until;
                }
            }
            updated.roll_if_needed(now);
            *lock = Some(updated);
        } else {
            *lock = None;
        }
    }

    /// Returns a copy of the current window limit.
    pub fn current_limit(&self) -> Option<DownloadWindowLimit> {
        self.limit.read().unwrap().clone()
    }

    /// Records progress for an active download and returns whether quota is exceeded.
    pub fn record_progress(&self, download_id: &str, current_downloaded: u64) -> QuotaStatus {
        let mut lock = self.limit.write().unwrap();
        let limit = match lock.as_mut() {
            Some(l) if l.enabled => l,
            _ => return QuotaStatus::Allowed,
        };

        let now = self.clock.now();
        let rolled = limit.roll_if_needed(now);

        let mut trackers = self.last_reported_bytes.lock().unwrap();
        if rolled {
            trackers.clear();
        }

        let previous = trackers.insert(download_id.to_string(), current_downloaded).unwrap_or(0);
        let delta = current_downloaded.saturating_sub(previous);

        let newly_exceeded = limit.record_bytes(delta, now);
        let is_exceeded = limit.is_exceeded(now);

        if is_exceeded {
            let explanation = limit.explanation(now);
            let resume_at = limit.stopped_until.unwrap_or_else(|| limit.window_end());

            if newly_exceeded {
                if let Some(hook) = &self.persistence_hook {
                    hook(limit);
                }
            }

            QuotaStatus::Exceeded {
                resume_at,
                explanation,
            }
        } else {
            QuotaStatus::Allowed
        }
    }

    /// Cleans up progress tracking state when a download completes, pauses, or stops.
    pub fn remove_download_tracker(&self, download_id: &str) {
        self.last_reported_bytes.lock().unwrap().remove(download_id);
    }

    /// Checks if the quota is currently exceeded.
    pub fn is_quota_exceeded(&self) -> (bool, Option<String>) {
        let mut lock = self.limit.write().unwrap();
        let limit = match lock.as_mut() {
            Some(l) if l.enabled => l,
            _ => return (false, None),
        };

        let now = self.clock.now();
        if limit.is_exceeded(now) {
            (true, Some(limit.explanation(now)))
        } else {
            (false, None)
        }
    }

    /// Checks if the rolling window has elapsed and needs a reset.
    /// If reset occurred, returns true and triggers persistence.
    pub fn check_window_roll(&self) -> bool {
        let mut lock = self.limit.write().unwrap();
        let limit = match lock.as_mut() {
            Some(l) if l.enabled => l,
            _ => return false,
        };

        let now = self.clock.now();
        if limit.roll_if_needed(now) {
            self.last_reported_bytes.lock().unwrap().clear();
            if let Some(hook) = &self.persistence_hook {
                hook(limit);
            }
            true
        } else {
            false
        }
    }

    /// Flushes the current limit state to disk via the persistence hook.
    pub fn flush_persistence(&self) {
        let lock = self.limit.read().unwrap();
        if let Some(limit) = lock.as_ref() {
            if let Some(hook) = &self.persistence_hook {
                hook(limit);
            }
        }
    }

    /// Adds a download ID to the list of downloads paused by quota.
    pub fn mark_quota_paused(&self, download_id: &str) {
        self.quota_paused_ids.lock().unwrap().insert(download_id.to_string());
    }

    /// Returns and clears all download IDs that were paused by quota.
    pub fn drain_quota_paused(&self) -> Vec<String> {
        let mut ids = self.quota_paused_ids.lock().unwrap();
        ids.drain().collect()
    }
}
