// Created: 2026-11-08
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Description: Domain entity models and evaluation policies for named download queues and recurring automation schedules.

use chrono::{DateTime, Datelike, NaiveTime, Utc};
use serde::{Deserialize, Serialize};

pub const DEFAULT_QUEUE_ID: &str = "default";
pub const DEFAULT_QUEUE_NAME: &str = "Default Queue";
pub const DEFAULT_QUEUE_CONCURRENCY: u8 = 2;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MissedRunPolicy {
    RunOnStartup,
    Skip,
}

impl Default for MissedRunPolicy {
    fn default() -> Self {
        Self::RunOnStartup
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum QueueDisposition {
    MoveToDefault,
    RemoveDownloads,
}

impl Default for QueueDisposition {
    fn default() -> Self {
        Self::MoveToDefault
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueueSchedule {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Days of week: 1 = Monday ... 7 = Sunday (ISO 8601)
    #[serde(default = "default_all_days")]
    pub days_of_week: Vec<u8>,
    /// Start time in 24h format "HH:MM"
    pub start_time: String,
    /// Optional stop time in 24h format "HH:MM"
    #[serde(default)]
    pub stop_time: Option<String>,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default)]
    pub missed_run_policy: MissedRunPolicy,
    #[serde(default)]
    pub next_run_at: Option<String>,
    #[serde(default)]
    pub last_run_at: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_all_days() -> Vec<u8> {
    vec![1, 2, 3, 4, 5, 6, 7]
}

fn default_timezone() -> String {
    "Local".to_string()
}

impl QueueSchedule {
    pub fn parse_time(time_str: &str) -> Option<NaiveTime> {
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        let h: u32 = parts[0].trim().parse().ok()?;
        let m: u32 = parts[1].trim().parse().ok()?;
        NaiveTime::from_hms_opt(h, m, 0)
    }

    /// Computes the next scheduled start instant occurring strictly after `reference_time`.
    pub fn compute_next_start(&self, reference_time: DateTime<Utc>) -> Option<DateTime<Utc>> {
        if !self.enabled || self.days_of_week.is_empty() {
            return None;
        }
        let target_time = Self::parse_time(&self.start_time)?;
        let ref_date = reference_time.date_naive();
        let ref_time = reference_time.time();

        // Check up to 14 days ahead to find next matching day of week
        for day_offset in 0..14 {
            let candidate_date = ref_date + chrono::Duration::days(day_offset);
            let weekday = candidate_date.weekday().number_from_monday() as u8;
            if self.days_of_week.contains(&weekday) {
                if day_offset == 0 && target_time <= ref_time {
                    continue;
                }
                let candidate_naive = candidate_date.and_time(target_time);
                return Some(DateTime::from_naive_utc_and_offset(candidate_naive, Utc));
            }
        }
        None
    }

    /// Checks if a scheduled start was missed while the app was closed between `last_checked` and `now`.
    pub fn is_missed_run(&self, last_checked: DateTime<Utc>, now: DateTime<Utc>) -> bool {
        if !self.enabled || self.days_of_week.is_empty() {
            return false;
        }
        if last_checked >= now {
            return false;
        }

        let start_time = match Self::parse_time(&self.start_time) {
            Some(t) => t,
            None => return false,
        };

        let start_date = last_checked.date_naive();
        let end_date = now.date_naive();
        let mut curr_date = start_date;

        while curr_date <= end_date {
            let weekday = curr_date.weekday().number_from_monday() as u8;
            if self.days_of_week.contains(&weekday) {
                let scheduled_naive = curr_date.and_time(start_time);
                let scheduled_dt: DateTime<Utc> =
                    DateTime::from_naive_utc_and_offset(scheduled_naive, Utc);
                if scheduled_dt > last_checked && scheduled_dt <= now {
                    return true;
                }
            }
            curr_date += chrono::Duration::days(1);
        }
        false
    }

    /// Returns true if `now` falls strictly within the active [start_time, stop_time] window for today.
    pub fn is_in_active_window(&self, now: DateTime<Utc>) -> bool {
        if !self.enabled || self.days_of_week.is_empty() {
            return false;
        }
        let weekday = now.weekday().number_from_monday() as u8;
        if !self.days_of_week.contains(&weekday) {
            return false;
        }

        let start = match Self::parse_time(&self.start_time) {
            Some(t) => t,
            None => return false,
        };
        let stop = match self.stop_time.as_deref().and_then(Self::parse_time) {
            Some(t) => t,
            None => return false,
        };

        let current_time = now.time();
        if start < stop {
            current_time >= start && current_time < stop
        } else {
            // Over-midnight schedule (e.g. 22:00 to 06:00)
            current_time >= start || current_time < stop
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadQueue {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub running: bool,
    #[serde(default = "default_queue_concurrency")]
    pub concurrency_limit: u8,
    #[serde(default)]
    pub speed_limit_kbps: u32,
    #[serde(default)]
    pub item_ids: Vec<String>,
    #[serde(default)]
    pub schedule: Option<QueueSchedule>,
}

fn default_queue_concurrency() -> u8 {
    DEFAULT_QUEUE_CONCURRENCY
}

impl DownloadQueue {
    pub fn default_queue() -> Self {
        Self {
            id: DEFAULT_QUEUE_ID.to_string(),
            name: DEFAULT_QUEUE_NAME.to_string(),
            enabled: true,
            running: true, // Default queue is running by default
            concurrency_limit: DEFAULT_QUEUE_CONCURRENCY,
            speed_limit_kbps: 0,
            item_ids: Vec::new(),
            schedule: None,
        }
    }

    pub fn new(id: String, name: String, concurrency_limit: u8, speed_limit_kbps: u32) -> Self {
        Self {
            id,
            name,
            enabled: true,
            running: false,
            concurrency_limit: concurrency_limit.max(1),
            speed_limit_kbps,
            item_ids: Vec::new(),
            schedule: None,
        }
    }

    pub fn is_default(&self) -> bool {
        self.id == DEFAULT_QUEUE_ID
    }

    pub fn add_item(&mut self, item_id: String) {
        if !self.item_ids.contains(&item_id) {
            self.item_ids.push(item_id);
        }
    }

    pub fn remove_item(&mut self, item_id: &str) -> bool {
        if let Some(pos) = self.item_ids.iter().position(|id| id == item_id) {
            self.item_ids.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn reorder_items(&mut self, new_order: &[String]) {
        let mut reordered = Vec::with_capacity(self.item_ids.len());
        for id in new_order {
            if self.item_ids.contains(id) && !reordered.contains(id) {
                reordered.push(id.clone());
            }
        }
        for id in &self.item_ids {
            if !reordered.contains(id) {
                reordered.push(id.clone());
            }
        }
        self.item_ids = reordered;
    }

    pub fn refresh_next_run(&mut self, now: DateTime<Utc>) {
        if let Some(ref mut sched) = self.schedule {
            sched.next_run_at = sched.compute_next_start(now).map(|dt| dt.to_rfc3339());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_queue_properties() {
        let q = DownloadQueue::default_queue();
        assert_eq!(q.id, DEFAULT_QUEUE_ID);
        assert!(q.is_default());
        assert!(q.running);
        assert_eq!(q.concurrency_limit, DEFAULT_QUEUE_CONCURRENCY);
    }

    #[test]
    fn test_queue_item_ordering_and_deduplication() {
        let mut q = DownloadQueue::new("work".to_string(), "Work".to_string(), 3, 1000);
        q.add_item("item-1".to_string());
        q.add_item("item-2".to_string());
        q.add_item("item-3".to_string());
        q.add_item("item-1".to_string()); // duplicate should be ignored
        assert_eq!(q.item_ids, vec!["item-1", "item-2", "item-3"]);

        // Reorder
        q.reorder_items(&["item-3".to_string(), "item-1".to_string()]);
        assert_eq!(q.item_ids, vec!["item-3", "item-1", "item-2"]);

        // Remove
        assert!(q.remove_item("item-1"));
        assert_eq!(q.item_ids, vec!["item-3", "item-2"]);
    }

    #[test]
    fn test_schedule_compute_next_start_daily() {
        let schedule = QueueSchedule {
            enabled: true,
            days_of_week: vec![1, 2, 3, 4, 5], // Mon-Fri
            start_time: "22:00".to_string(),
            stop_time: Some("06:00".to_string()),
            timezone: "Local".to_string(),
            missed_run_policy: MissedRunPolicy::RunOnStartup,
            next_run_at: None,
            last_run_at: None,
        };

        // Reference: Wednesday at 14:00
        let ref_dt = DateTime::parse_from_rfc3339("2026-11-04T14:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let next = schedule.compute_next_start(ref_dt).expect("next start");
        assert_eq!(next.to_rfc3339(), "2026-11-04T22:00:00+00:00");

        // Reference: Wednesday at 23:00 (past 22:00) -> next should be Thursday at 22:00
        let ref_late = DateTime::parse_from_rfc3339("2026-11-04T23:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let next_day = schedule.compute_next_start(ref_late).expect("next day start");
        assert_eq!(next_day.to_rfc3339(), "2026-11-05T22:00:00+00:00");
    }

    #[test]
    fn test_schedule_over_midnight_active_window() {
        let schedule = QueueSchedule {
            enabled: true,
            days_of_week: vec![1, 2, 3, 4, 5, 6, 7],
            start_time: "22:00".to_string(),
            stop_time: Some("06:00".to_string()),
            timezone: "Local".to_string(),
            missed_run_policy: MissedRunPolicy::RunOnStartup,
            next_run_at: None,
            last_run_at: None,
        };

        let night = DateTime::parse_from_rfc3339("2026-11-04T23:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(schedule.is_in_active_window(night));

        let early_morning = DateTime::parse_from_rfc3339("2026-11-05T04:15:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(schedule.is_in_active_window(early_morning));

        let day = DateTime::parse_from_rfc3339("2026-11-05T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(!schedule.is_in_active_window(day));
    }

    #[test]
    fn test_schedule_missed_run_detection() {
        let schedule = QueueSchedule {
            enabled: true,
            days_of_week: vec![1, 2, 3, 4, 5],
            start_time: "20:00".to_string(),
            stop_time: Some("23:00".to_string()),
            timezone: "Local".to_string(),
            missed_run_policy: MissedRunPolicy::RunOnStartup,
            next_run_at: None,
            last_run_at: None,
        };

        // App closed at 19:30 and reopened at 20:30 (start was 20:00)
        let closed_at = DateTime::parse_from_rfc3339("2026-11-04T19:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let opened_at = DateTime::parse_from_rfc3339("2026-11-04T20:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        assert!(schedule.is_missed_run(closed_at, opened_at));

        // If app opened before scheduled start, not missed
        let early_open = DateTime::parse_from_rfc3339("2026-11-04T19:50:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(!schedule.is_missed_run(closed_at, early_open));
    }
}
