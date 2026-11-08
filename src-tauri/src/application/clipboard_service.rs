// Created: 2026-11-08
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Last updated: 2026-11-08
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Description: Application orchestration service managing clipboard polling, policy checks, and event dispatch.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};
use std::time::Duration;

use tokio::sync::watch;

use crate::domain::clipboard::{
    ClipboardCandidate, ClipboardMonitoringSettings, ClipboardPolicy,
};

pub trait ClipboardReader: Send + Sync {
    fn read_text(&self) -> Result<Option<String>, String>;
}

pub trait ClipboardEventPublisher: Send + Sync {
    fn publish_candidate(&self, candidate: &ClipboardCandidate) -> Result<(), String>;
}

pub struct ClipboardMonitoringService {
    reader: Arc<dyn ClipboardReader>,
    event_publisher: Option<Arc<dyn ClipboardEventPublisher>>,
    settings: Arc<RwLock<ClipboardMonitoringSettings>>,
    last_seen_url: Arc<Mutex<Option<String>>>,
    shutdown_tx: Arc<Mutex<Option<watch::Sender<bool>>>>,
    poll_interval: Duration,
    is_running: Arc<AtomicBool>,
}

impl ClipboardMonitoringService {
    pub fn new(
        reader: Arc<dyn ClipboardReader>,
        event_publisher: Option<Arc<dyn ClipboardEventPublisher>>,
    ) -> Self {
        Self::with_interval(
            reader,
            event_publisher,
            Duration::from_millis(1500),
        )
    }

    pub fn with_interval(
        reader: Arc<dyn ClipboardReader>,
        event_publisher: Option<Arc<dyn ClipboardEventPublisher>>,
        interval: Duration,
    ) -> Self {
        Self {
            reader,
            event_publisher,
            settings: Arc::new(RwLock::new(ClipboardMonitoringSettings::default())),
            last_seen_url: Arc::new(Mutex::new(None)),
            shutdown_tx: Arc::new(Mutex::new(None)),
            poll_interval: interval,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn settings(&self) -> ClipboardMonitoringSettings {
        self.settings.read().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn set_settings(&self, new_settings: ClipboardMonitoringSettings) {
        let was_enabled = {
            let mut s = self.settings.write().unwrap_or_else(|p| p.into_inner());
            let prev = s.enabled;
            *s = new_settings.clone();
            prev
        };

        if new_settings.enabled && !was_enabled {
            self.start_loop();
        } else if !new_settings.enabled && was_enabled {
            self.stop_loop();
        }
    }

    pub fn dismiss(&self, url: &str) {
        if let Ok(mut lock) = self.last_seen_url.lock() {
            *lock = Some(url.to_string());
        }
    }

    pub fn poll_once(&self) -> Option<ClipboardCandidate> {
        let current_settings = self.settings();
        if !current_settings.enabled {
            return None;
        }

        let read_result = self.reader.read_text();
        let raw_text = match read_result {
            Ok(Some(text)) => text,
            _ => return None,
        };

        let last_seen = self.last_seen_url.lock().ok().and_then(|l| l.clone());
        let candidate = ClipboardPolicy::evaluate(&raw_text, &current_settings, last_seen.as_deref()).ok()?;

        if let Ok(mut lock) = self.last_seen_url.lock() {
            *lock = Some(candidate.url.clone());
        }

        if let Some(ref publisher) = self.event_publisher {
            let _ = publisher.publish_candidate(&candidate);
        }

        Some(candidate)
    }

    pub fn start_loop(&self) {
        let mut shutdown_guard = self.shutdown_tx.lock().unwrap_or_else(|p| p.into_inner());
        if shutdown_guard.is_some() {
            return;
        }

        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };

        let (tx, mut rx) = watch::channel(false);
        *shutdown_guard = Some(tx);
        self.is_running.store(true, Ordering::SeqCst);

        let reader = self.reader.clone();
        let publisher = self.event_publisher.clone();
        let settings = self.settings.clone();
        let last_seen_url = self.last_seen_url.clone();
        let poll_interval = self.poll_interval;
        let is_running = self.is_running.clone();

        handle.spawn(async move {
            let mut ticker = tokio::time::interval(poll_interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let current_settings = settings.read().map(|s| s.clone()).unwrap_or_default();
                        if !current_settings.enabled {
                            break;
                        }

                        let text_opt = match reader.read_text() {
                            Ok(text) => text,
                            Err(_) => None,
                        };

                        if let Some(raw_text) = text_opt {
                            let last_seen = last_seen_url.lock().ok().and_then(|l| l.clone());
                            if let Ok(candidate) = ClipboardPolicy::evaluate(&raw_text, &current_settings, last_seen.as_deref()) {
                                if let Ok(mut lock) = last_seen_url.lock() {
                                    *lock = Some(candidate.url.clone());
                                }
                                if let Some(ref publ) = publisher {
                                    let _ = publ.publish_candidate(&candidate);
                                }
                            }
                        }
                    }
                    _ = rx.changed() => {
                        if *rx.borrow() {
                            break;
                        }
                    }
                }
            }
            is_running.store(false, Ordering::SeqCst);
        });
    }

    pub fn stop_loop(&self) {
        let mut shutdown_guard = self.shutdown_tx.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(tx) = shutdown_guard.take() {
            let _ = tx.send(true);
        }
        self.is_running.store(false, Ordering::SeqCst);
    }
}

impl Drop for ClipboardMonitoringService {
    fn drop(&mut self) {
        self.stop_loop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestClipboardReader {
        content: Mutex<Option<String>>,
    }

    impl TestClipboardReader {
        fn new(initial: Option<&str>) -> Self {
            Self {
                content: Mutex::new(initial.map(|s| s.to_string())),
            }
        }

        fn set(&self, val: Option<&str>) {
            *self.content.lock().unwrap() = val.map(|s| s.to_string());
        }
    }

    impl ClipboardReader for TestClipboardReader {
        fn read_text(&self) -> Result<Option<String>, String> {
            Ok(self.content.lock().unwrap().clone())
        }
    }

    struct TestPublisher {
        dispatched: Mutex<Vec<ClipboardCandidate>>,
    }

    impl TestPublisher {
        fn new() -> Self {
            Self {
                dispatched: Mutex::new(Vec::new()),
            }
        }
    }

    impl ClipboardEventPublisher for TestPublisher {
        fn publish_candidate(&self, candidate: &ClipboardCandidate) -> Result<(), String> {
            self.dispatched.lock().unwrap().push(candidate.clone());
            Ok(())
        }
    }

    #[test]
    fn test_service_disabled_by_default_polls_nothing() {
        let reader = Arc::new(TestClipboardReader::new(Some("https://example.com/archive.zip")));
        let publisher = Arc::new(TestPublisher::new());
        let service = ClipboardMonitoringService::new(reader, Some(publisher.clone()));

        assert_eq!(service.poll_once(), None);
        assert!(publisher.dispatched.lock().unwrap().is_empty());
    }

    #[test]
    fn test_service_enabled_polls_valid_url() {
        let reader = Arc::new(TestClipboardReader::new(Some("https://example.com/archive.zip")));
        let publisher = Arc::new(TestPublisher::new());
        let service = ClipboardMonitoringService::new(reader.clone(), Some(publisher.clone()));

        service.set_settings(ClipboardMonitoringSettings {
            enabled: true,
            auto_enqueue: false,
        });

        let cand = service.poll_once().expect("candidate expected");
        assert_eq!(cand.url, "https://example.com/archive.zip");
        assert_eq!(cand.host, "example.com");
        assert_eq!(cand.filename, "archive.zip");

        assert_eq!(publisher.dispatched.lock().unwrap().len(), 1);

        // Immediate next poll returns None due to duplicate suppression
        assert_eq!(service.poll_once(), None);
        assert_eq!(publisher.dispatched.lock().unwrap().len(), 1);

        // Reader updates to new URL
        reader.set(Some("https://cdn.example.org/installer.pkg"));
        let cand2 = service.poll_once().expect("second candidate expected");
        assert_eq!(cand2.filename, "installer.pkg");
        assert_eq!(publisher.dispatched.lock().unwrap().len(), 2);
    }

    #[test]
    fn test_service_dismiss_prevents_subsequent_prompt() {
        let reader = Arc::new(TestClipboardReader::new(Some("https://example.com/item.iso")));
        let publisher = Arc::new(TestPublisher::new());
        let service = ClipboardMonitoringService::new(reader, Some(publisher.clone()));

        service.set_settings(ClipboardMonitoringSettings {
            enabled: true,
            auto_enqueue: false,
        });

        service.dismiss("https://example.com/item.iso");
        assert_eq!(service.poll_once(), None);
        assert!(publisher.dispatched.lock().unwrap().is_empty());
    }
}
