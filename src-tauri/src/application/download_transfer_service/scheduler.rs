// Created: 2026-11-10
// Reason: refactor: modularize monolithic download_transfer_service into focused coordinators
// Description: Download queue pumping, scheduler timers, due downloads initiation, and rate/concurrency settings.

use tokio::sync::watch;

use crate::{
    application::{
        download_executor::DownloadControl,
        download_transfer_policy::SCHEDULER_POLL_INTERVAL_MILLISECONDS,
        error::{AppError, AppResult},
    },
    domain::{
        download::{DownloadAction, DownloadStatus},
        settings::ProxySettings,
    },
};

use super::DownloadTransferService;

impl DownloadTransferService {
    pub fn pump_queue(&self) {
        if self.quota_service.is_quota_exceeded().0 {
            return;
        }
        let _guard = match self.queue_pump_lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let max = self.concurrency.max_concurrent();
        let active = match self.history.active_download_count() {
            Ok(count) => count,
            Err(_) => return,
        };
        if active >= max {
            return;
        }
        let mut available = max - active;

        if let Some(qs) = self.queue_service() {
            let running = qs.running_queues();
            for queue in running {
                if available == 0 {
                    break;
                }
                let mut queue_active = 0;
                if let Ok(controls) = self.controls.lock() {
                    for item_id in &queue.item_ids {
                        if controls.contains_key(item_id) {
                            queue_active += 1;
                        }
                    }
                }

                let queue_limit = queue.concurrency_limit as usize;
                if queue_active < queue_limit {
                    let queue_available = (queue_limit - queue_active).min(available);
                    let mut started_for_queue = 0;
                    for item_id in &queue.item_ids {
                        if started_for_queue >= queue_available {
                            break;
                        }
                        if let Ok(record) = self.history.download(item_id) {
                            if record.status == DownloadStatus::Queued {
                                if let Err(error) = self.start_download(item_id, false) {
                                    eprintln!(
                                        "Failed to auto-start queued download {item_id}: {}",
                                        error.message
                                    );
                                } else {
                                    started_for_queue += 1;
                                    available = available.saturating_sub(1);
                                }
                            }
                        }
                    }
                }
            }
        } else if let Ok(next_ids) = self.history.next_queued_download_ids(available) {
            for id in next_ids {
                if let Err(error) = self.start_download(&id, false) {
                    eprintln!("Failed to auto-start queued download {id}: {}", error.message);
                }
            }
        }
    }

    pub fn pause_queue_downloads(&self, queue_id: &str) {
        if let Some(qs) = self.queue_service() {
            if let Some(q) = qs.get_queue(queue_id) {
                for item_id in q.item_ids {
                    let _ = self.action(&item_id, DownloadAction::Pause);
                }
            }
        }
    }

    pub fn set_max_concurrent(&self, value: u8) {
        self.concurrency.set_max_concurrent(value);
        self.pump_queue();
    }

    pub fn set_speed_limit(&self, value_kbps: u32) {
        self.rate_limiter.set_limit_kbps(value_kbps);
    }

    pub fn set_proxy(&self, proxy: Option<&ProxySettings>) -> AppResult<()> {
        self.executor.set_proxy(proxy)
    }

    pub fn start_scheduler(&self) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(
                SCHEDULER_POLL_INTERVAL_MILLISECONDS,
            ));
            loop {
                interval.tick().await;
                if service.quota_service.check_window_roll() {
                    service.resume_quota_paused_downloads();
                    service.persist_quota_state();
                }
                if let Err(error) = service.start_due_downloads() {
                    eprintln!("Could not start scheduled downloads: {}", error.message);
                }
                if let Some(qs) = service.queue_service() {
                    let (_started, stopped) = qs.check_schedules();
                    for stop_id in stopped {
                        service.pause_queue_downloads(&stop_id);
                    }
                }
                service.pump_queue();
            }
        });
    }

    pub(crate) fn start_due_downloads(&self) -> AppResult<()> {
        let mut controls = self
            .controls
            .lock()
            .map_err(|_| AppError::storage("Download controls are unavailable."))?;
        let records = self
            .history
            .begin_due_scheduled_downloads(chrono::Utc::now(), chrono::Utc::now().to_rfc3339())?;
        let started = records
            .into_iter()
            .map(|record| {
                let (sender, receiver) = watch::channel(None);
                controls.insert(record.id.clone(), sender);
                (record, DownloadControl::new(receiver))
            })
            .collect::<Vec<_>>();
        drop(controls);
        for (record, control) in started {
            self.spawn_download(record, control);
        }
        Ok(())
    }
}
