// Created: 2026-11-08
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Description: Application service managing persistent named download queues, order, and recurring schedules.

use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    application::error::{AppError, AppResult},
    domain::{
        queue::{
            DownloadQueue, MissedRunPolicy, QueueDisposition, QueueSchedule,
            DEFAULT_QUEUE_ID,
        },
        settings::window_limit::Clock,
    },
    infrastructure::queue_store::QueueRepository,
};

pub struct QueueService {
    store: Box<dyn QueueRepository>,
    queues: RwLock<Vec<DownloadQueue>>,
    clock: Arc<dyn Clock>,
    last_schedule_check: RwLock<DateTime<Utc>>,
}

impl QueueService {
    pub fn new(store: impl QueueRepository + 'static, clock: Arc<dyn Clock>) -> AppResult<Self> {
        let initial_queues = store.load()?;
        let now = clock.now();
        let mut queues = initial_queues;
        for q in &mut queues {
            q.refresh_next_run(now);
        }
        let _ = store.save(&queues);

        Ok(Self {
            store: Box::new(store),
            queues: RwLock::new(queues),
            clock,
            last_schedule_check: RwLock::new(now),
        })
    }

    pub fn list_queues(&self) -> Vec<DownloadQueue> {
        self.queues.read().unwrap().clone()
    }

    pub fn get_queue(&self, id: &str) -> Option<DownloadQueue> {
        self.queues
            .read()
            .unwrap()
            .iter()
            .find(|q| q.id == id)
            .cloned()
    }

    pub fn running_queues(&self) -> Vec<DownloadQueue> {
        self.queues
            .read()
            .unwrap()
            .iter()
            .filter(|q| q.enabled && q.running)
            .cloned()
            .collect()
    }

    pub fn create_queue(
        &self,
        name: String,
        concurrency_limit: u8,
        speed_limit_kbps: u32,
        schedule: Option<QueueSchedule>,
    ) -> AppResult<DownloadQueue> {
        let trimmed_name = name.trim().to_string();
        if trimmed_name.is_empty() {
            return Err(AppError::invalid_settings("Queue name cannot be empty."));
        }

        let mut guard = self.queues.write().unwrap();
        if guard.iter().any(|q| q.name.eq_ignore_ascii_case(&trimmed_name)) {
            return Err(AppError::invalid_settings(
                "A queue with this name already exists.",
            ));
        }

        let id = format!("queue-{}", Uuid::new_v4().simple());
        let mut queue = DownloadQueue {
            id,
            name: trimmed_name,
            enabled: true,
            running: false,
            concurrency_limit: concurrency_limit.max(1),
            speed_limit_kbps,
            item_ids: Vec::new(),
            schedule,
        };
        queue.refresh_next_run(self.clock.now());

        guard.push(queue.clone());
        self.store.save(&guard)?;
        Ok(queue)
    }

    pub fn update_queue(
        &self,
        id: &str,
        name: Option<String>,
        enabled: Option<bool>,
        concurrency_limit: Option<u8>,
        speed_limit_kbps: Option<u32>,
        schedule: Option<Option<QueueSchedule>>,
    ) -> AppResult<DownloadQueue> {
        let mut guard = self.queues.write().unwrap();
        let queue_idx = guard
            .iter()
            .position(|q| q.id == id)
            .ok_or_else(|| AppError::download("Queue not found."))?;

        if let Some(new_name) = name {
            let trimmed = new_name.trim().to_string();
            if trimmed.is_empty() {
                return Err(AppError::invalid_settings("Queue name cannot be empty."));
            }
            if guard
                .iter()
                .enumerate()
                .any(|(i, q)| i != queue_idx && q.name.eq_ignore_ascii_case(&trimmed))
            {
                return Err(AppError::invalid_settings(
                    "A queue with this name already exists.",
                ));
            }
            guard[queue_idx].name = trimmed;
        }

        if let Some(en) = enabled {
            guard[queue_idx].enabled = en;
        }

        if let Some(conc) = concurrency_limit {
            guard[queue_idx].concurrency_limit = conc.max(1);
        }

        if let Some(spd) = speed_limit_kbps {
            guard[queue_idx].speed_limit_kbps = spd;
        }

        if let Some(sched_opt) = schedule {
            guard[queue_idx].schedule = sched_opt;
        }

        guard[queue_idx].refresh_next_run(self.clock.now());
        let result = guard[queue_idx].clone();
        self.store.save(&guard)?;
        Ok(result)
    }

    pub fn delete_queue(&self, id: &str, disposition: QueueDisposition) -> AppResult<Vec<String>> {
        if id == DEFAULT_QUEUE_ID {
            return Err(AppError::invalid_settings(
                "The default queue cannot be deleted.",
            ));
        }

        let mut guard = self.queues.write().unwrap();
        let queue_idx = guard
            .iter()
            .position(|q| q.id == id)
            .ok_or_else(|| AppError::download("Queue not found."))?;

        let removed_queue = guard.remove(queue_idx);
        let items = removed_queue.item_ids;

        if disposition == QueueDisposition::MoveToDefault {
            if let Some(default_q) = guard.iter_mut().find(|q| q.id == DEFAULT_QUEUE_ID) {
                for item_id in &items {
                    default_q.add_item(item_id.clone());
                }
            }
        }

        self.store.save(&guard)?;
        Ok(items)
    }

    pub fn start_queue(&self, id: &str) -> AppResult<DownloadQueue> {
        let mut guard = self.queues.write().unwrap();
        let queue = guard
            .iter_mut()
            .find(|q| q.id == id)
            .ok_or_else(|| AppError::download("Queue not found."))?;

        queue.running = true;
        let result = queue.clone();
        self.store.save(&guard)?;
        Ok(result)
    }

    pub fn pause_queue(&self, id: &str) -> AppResult<DownloadQueue> {
        let mut guard = self.queues.write().unwrap();
        let queue = guard
            .iter_mut()
            .find(|q| q.id == id)
            .ok_or_else(|| AppError::download("Queue not found."))?;

        queue.running = false;
        let result = queue.clone();
        self.store.save(&guard)?;
        Ok(result)
    }

    pub fn assign_download(&self, download_id: String, target_queue_id: &str) -> AppResult<()> {
        let mut guard = self.queues.write().unwrap();
        // Remove from all queues first
        for q in guard.iter_mut() {
            q.remove_item(&download_id);
        }

        // Add to target queue
        let target = guard
            .iter_mut()
            .find(|q| q.id == target_queue_id)
            .ok_or_else(|| AppError::download("Target queue not found."))?;
        target.add_item(download_id);

        self.store.save(&guard)?;
        Ok(())
    }

    pub fn remove_download(&self, download_id: &str) -> AppResult<()> {
        let mut guard = self.queues.write().unwrap();
        let mut changed = false;
        for q in guard.iter_mut() {
            if q.remove_item(download_id) {
                changed = true;
            }
        }
        if changed {
            self.store.save(&guard)?;
        }
        Ok(())
    }

    pub fn reorder_items(&self, queue_id: &str, new_order: &[String]) -> AppResult<()> {
        let mut guard = self.queues.write().unwrap();
        let queue = guard
            .iter_mut()
            .find(|q| q.id == queue_id)
            .ok_or_else(|| AppError::download("Queue not found."))?;

        queue.reorder_items(new_order);
        self.store.save(&guard)?;
        Ok(())
    }

    /// Evaluates all recurring schedules against the current clock.
    /// Returns `(queues_to_start, queues_to_stop)` and updates next/last run times.
    pub fn check_schedules(&self) -> (Vec<String>, Vec<String>) {
        let now = self.clock.now();
        let last_check = {
            let mut l_guard = self.last_schedule_check.write().unwrap();
            let prev = *l_guard;
            *l_guard = now;
            prev
        };

        let mut to_start = Vec::new();
        let mut to_stop = Vec::new();
        let mut guard = self.queues.write().unwrap();
        let mut changed = false;

        for queue in guard.iter_mut() {
            if !queue.enabled {
                continue;
            }
            if let Some(ref mut sched) = queue.schedule {
                if !sched.enabled {
                    continue;
                }

                let in_window = sched.is_in_active_window(now);
                let missed = sched.is_missed_run(last_check, now);

                if !queue.running {
                    let should_start = if in_window {
                        true
                    } else if missed && sched.missed_run_policy == MissedRunPolicy::RunOnStartup {
                        true
                    } else {
                        false
                    };

                    if should_start {
                        queue.running = true;
                        sched.last_run_at = Some(now.to_rfc3339());
                        sched.next_run_at = sched.compute_next_start(now).map(|d| d.to_rfc3339());
                        to_start.push(queue.id.clone());
                        changed = true;
                    }
                } else if queue.running {
                    // If queue is running, check if stop_time was configured and window ended
                    if sched.stop_time.is_some() && !in_window {
                        queue.running = false;
                        sched.next_run_at = sched.compute_next_start(now).map(|d| d.to_rfc3339());
                        to_stop.push(queue.id.clone());
                        changed = true;
                    }
                }
            }
        }

        if changed {
            let _ = self.store.save(&guard);
        }

        (to_start, to_stop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::settings::window_limit::MockClock;

    struct InMemoryQueueStore {
        queues: std::sync::Mutex<Vec<DownloadQueue>>,
    }

    impl InMemoryQueueStore {
        fn new(queues: Vec<DownloadQueue>) -> Self {
            Self {
                queues: std::sync::Mutex::new(queues),
            }
        }
    }

    impl QueueRepository for InMemoryQueueStore {
        fn load(&self) -> AppResult<Vec<DownloadQueue>> {
            Ok(self.queues.lock().unwrap().clone())
        }
        fn save(&self, queues: &[DownloadQueue]) -> AppResult<()> {
            *self.queues.lock().unwrap() = queues.to_vec();
            Ok(())
        }
    }

    #[test]
    fn test_queue_service_crud_and_safety() {
        let base_time = DateTime::parse_from_rfc3339("2026-11-04T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let clock = Arc::new(MockClock::new(base_time));
        let store = InMemoryQueueStore::new(vec![DownloadQueue::default_queue()]);
        let service = QueueService::new(store, clock).unwrap();

        // Default queue cannot be deleted
        assert!(service
            .delete_queue(DEFAULT_QUEUE_ID, QueueDisposition::MoveToDefault)
            .is_err());

        // Create new queue
        let nightly = service
            .create_queue("Nightly".into(), 2, 500, None)
            .expect("create nightly");
        assert_eq!(nightly.name, "Nightly");
        assert!(!nightly.running);

        // Cannot create duplicate name
        assert!(service
            .create_queue("nightly".into(), 2, 500, None)
            .is_err());

        // Assign download to nightly
        service
            .assign_download("dl-1".into(), &nightly.id)
            .expect("assign download");
        let q = service.get_queue(&nightly.id).unwrap();
        assert_eq!(q.item_ids, vec!["dl-1"]);

        // Delete nightly with MoveToDefault
        let moved_items = service
            .delete_queue(&nightly.id, QueueDisposition::MoveToDefault)
            .expect("delete nightly");
        assert_eq!(moved_items, vec!["dl-1"]);
        let def = service.get_queue(DEFAULT_QUEUE_ID).unwrap();
        assert!(def.item_ids.contains(&"dl-1".to_string()));
    }

    #[test]
    fn test_schedule_trigger_and_stop_with_mock_clock() {
        let base_time = DateTime::parse_from_rfc3339("2026-11-04T21:55:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let clock = Arc::new(MockClock::new(base_time));
        let sched = QueueSchedule {
            enabled: true,
            days_of_week: vec![1, 2, 3, 4, 5],
            start_time: "22:00".into(),
            stop_time: Some("23:00".into()),
            timezone: "Local".into(),
            missed_run_policy: MissedRunPolicy::RunOnStartup,
            next_run_at: None,
            last_run_at: None,
        };

        let store = InMemoryQueueStore::new(vec![DownloadQueue::default_queue()]);
        let service = QueueService::new(store, clock.clone()).unwrap();
        let q = service
            .create_queue("Scheduled".into(), 1, 0, Some(sched))
            .unwrap();

        // At 21:55: not yet start time
        let (started, stopped) = service.check_schedules();
        assert!(started.is_empty());
        assert!(stopped.is_empty());

        // Advance clock past 22:00 to 22:05
        let start_time = DateTime::parse_from_rfc3339("2026-11-04T22:05:00Z")
            .unwrap()
            .with_timezone(&Utc);
        clock.set(start_time);

        let (started, _) = service.check_schedules();
        assert_eq!(started, vec![q.id.clone()]);
        let active_q = service.get_queue(&q.id).unwrap();
        assert!(active_q.running);

        // Advance clock past 23:00 stop time
        let stop_time = DateTime::parse_from_rfc3339("2026-11-04T23:05:00Z")
            .unwrap()
            .with_timezone(&Utc);
        clock.set(stop_time);

        let (_, stopped) = service.check_schedules();
        assert_eq!(stopped, vec![q.id.clone()]);
        let stopped_q = service.get_queue(&q.id).unwrap();
        assert!(!stopped_q.running);
    }
}
