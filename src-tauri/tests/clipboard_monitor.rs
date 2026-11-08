// Created: 2026-11-08
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Last updated: 2026-11-08
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Reason: feat(theme): add light, dark, and system theme options to the desktop app (Closes #4)
// Description: Human-scenario integration tests for clipboard monitoring, validation, duplicate suppression, and dismissal.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use open_download_manager_lib::{
    application::{
        clipboard_service::{
            ClipboardEventPublisher, ClipboardMonitoringService,
        },
        settings_service::{SettingsRepository, SettingsService},
    },
    domain::{
        clipboard::{
            ClipboardCandidate, ClipboardMonitoringSettings, ClipboardPolicy,
            ClipboardRejectionReason,
        },
        settings::SettingsPatch,
    },
    infrastructure::{
        clipboard::MockClipboardReader,
        settings_store::SettingsStore,
    },
};

struct RecordingPublisher {
    events: Mutex<Vec<ClipboardCandidate>>,
}

impl RecordingPublisher {
    fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    fn published(&self) -> Vec<ClipboardCandidate> {
        self.events.lock().unwrap().clone()
    }
}

impl ClipboardEventPublisher for RecordingPublisher {
    fn publish_candidate(&self, candidate: &ClipboardCandidate) -> Result<(), String> {
        self.events.lock().unwrap().push(candidate.clone());
        Ok(())
    }
}

fn test_store_directory() -> PathBuf {
    let dir = tempfile::Builder::new()
        .prefix("open-download-manager-clipboard-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep();
    dir.join("store")
}

#[test]
fn human_scenario_default_disabled_does_not_observe_or_prompt() {
    let reader = Arc::new(MockClipboardReader::with_content(
        "https://example.com/downloads/setup.dmg",
    ));
    let publisher = Arc::new(RecordingPublisher::new());
    let service = ClipboardMonitoringService::new(reader, Some(publisher.clone()));

    // Verify initial settings state: strictly opt-in, disabled by default
    assert!(!service.settings().enabled);
    assert!(!service.settings().auto_enqueue);

    // Bounded polling returns None and emits no candidate events
    let result = service.poll_once();
    assert_eq!(result, None);
    assert!(publisher.published().is_empty());
}

#[test]
fn human_scenario_opt_in_enables_detection_with_normalized_host_and_filename() {
    let reader = Arc::new(MockClipboardReader::with_content(
        "  https://CDN.Downloads.Example.Org/releases/installer-v2.1.tar.gz?token=abc  ",
    ));
    let publisher = Arc::new(RecordingPublisher::new());
    let service = ClipboardMonitoringService::new(reader.clone(), Some(publisher.clone()));

    // User explicitly opts in to clipboard monitoring
    service.set_settings(ClipboardMonitoringSettings {
        enabled: true,
        auto_enqueue: false,
    });

    let candidate = service
        .poll_once()
        .expect("valid download URL should be detected");

    assert_eq!(
        candidate.url,
        "https://cdn.downloads.example.org/releases/installer-v2.1.tar.gz?token=abc"
    );
    assert_eq!(candidate.host, "cdn.downloads.example.org");
    assert_eq!(candidate.filename, "installer-v2.1.tar.gz");

    let published = publisher.published();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0], candidate);
}

#[test]
fn human_scenario_embedded_credentials_are_strictly_rejected() {
    let reader = Arc::new(MockClipboardReader::with_content(
        "https://admin:supersecret@internal.corp/confidential.zip",
    ));
    let publisher = Arc::new(RecordingPublisher::new());
    let service = ClipboardMonitoringService::new(reader, Some(publisher.clone()));

    service.set_settings(ClipboardMonitoringSettings {
        enabled: true,
        auto_enqueue: false,
    });

    // Verification: basic credentials user:pass@ rejected, no candidate emitted
    let result = service.poll_once();
    assert_eq!(result, None);
    assert!(publisher.published().is_empty());

    // Domain policy level assertion
    let direct_eval = ClipboardPolicy::evaluate(
        "https://admin:supersecret@internal.corp/confidential.zip",
        &service.settings(),
        None,
    );
    assert_eq!(direct_eval, Err(ClipboardRejectionReason::ContainsCredentials));
}

#[test]
fn human_scenario_oversized_payloads_and_non_url_text_ignored() {
    let settings = ClipboardMonitoringSettings {
        enabled: true,
        auto_enqueue: false,
    };

    // 1. Oversized clipboard payload
    let huge_text = "https://example.com/".to_string() + &"x".repeat(5000);
    assert_eq!(
        ClipboardPolicy::evaluate(&huge_text, &settings, None),
        Err(ClipboardRejectionReason::Oversized)
    );

    // 2. Multiline / code block / casual text
    assert_eq!(
        ClipboardPolicy::evaluate("Here is the download link: https://example.com/file.zip", &settings, None),
        Err(ClipboardRejectionReason::MultilineOrWhitespace)
    );

    assert_eq!(
        ClipboardPolicy::evaluate("SELECT * FROM users;\nDROP TABLE logs;", &settings, None),
        Err(ClipboardRejectionReason::MultilineOrWhitespace)
    );

    // 3. Unsupported schemes
    assert_eq!(
        ClipboardPolicy::evaluate("ftp://example.com/file.iso", &settings, None),
        Err(ClipboardRejectionReason::UnsupportedScheme)
    );
    assert_eq!(
        ClipboardPolicy::evaluate("javascript:void(0)", &settings, None),
        Err(ClipboardRejectionReason::UnsupportedScheme)
    );
    assert_eq!(
        ClipboardPolicy::evaluate("data:text/plain;base64,SGVsbG8=", &settings, None),
        Err(ClipboardRejectionReason::UnsupportedScheme)
    );
}

#[test]
fn human_scenario_duplicate_suppression_prevents_reprompting() {
    let reader = Arc::new(MockClipboardReader::with_content(
        "https://example.com/release.iso",
    ));
    let publisher = Arc::new(RecordingPublisher::new());
    let service = ClipboardMonitoringService::new(reader.clone(), Some(publisher.clone()));

    service.set_settings(ClipboardMonitoringSettings {
        enabled: true,
        auto_enqueue: false,
    });

    // First poll detects candidate
    let first = service.poll_once().expect("first poll detects candidate");
    assert_eq!(first.filename, "release.iso");
    assert_eq!(publisher.published().len(), 1);

    // Second poll with unchanged clipboard content is suppressed
    assert_eq!(service.poll_once(), None);
    assert_eq!(publisher.published().len(), 1);

    // Updating clipboard to a new link detects the new candidate
    reader.set_text(Some("https://example.com/update.zip".to_string()));
    let second = service.poll_once().expect("second poll detects new candidate");
    assert_eq!(second.filename, "update.zip");
    assert_eq!(publisher.published().len(), 2);
}

#[test]
fn human_scenario_dismissal_suppresses_prompt_while_url_remains_on_clipboard() {
    let url = "https://example.com/package.tar.zst";
    let reader = Arc::new(MockClipboardReader::with_content(url));
    let publisher = Arc::new(RecordingPublisher::new());
    let service = ClipboardMonitoringService::new(reader, Some(publisher.clone()));

    service.set_settings(ClipboardMonitoringSettings {
        enabled: true,
        auto_enqueue: false,
    });

    // User dismisses this specific URL
    service.dismiss(url);

    // Candidate is suppressed while on clipboard
    assert_eq!(service.poll_once(), None);
    assert!(publisher.published().is_empty());
}

#[tokio::test]
async fn human_scenario_disabling_monitoring_stops_loop_immediately() {
    let reader = Arc::new(MockClipboardReader::with_content("https://example.com/test.zip"));
    let publisher = Arc::new(RecordingPublisher::new());
    let service = ClipboardMonitoringService::with_interval(
        reader,
        Some(publisher),
        Duration::from_millis(50),
    );

    // Enable starts background worker
    service.set_settings(ClipboardMonitoringSettings {
        enabled: true,
        auto_enqueue: false,
    });
    assert!(service.is_running());

    // Disable terminates loop immediately
    service.set_settings(ClipboardMonitoringSettings {
        enabled: false,
        auto_enqueue: false,
    });
    assert!(!service.is_running());

    // Wait slightly to ensure task exits cleanly
    tokio::time::sleep(Duration::from_millis(70)).await;
    assert!(!service.is_running());
}

#[test]
fn human_scenario_settings_persistence_across_reloads() {
    let directory = test_store_directory();
    let default_directory = std::env::temp_dir().join("downloads");
    let mut service = SettingsService::new(SettingsStore::new(&directory), default_directory.clone())
        .expect("default settings should load");

    // Initially disabled
    assert!(!service.settings().clipboard_monitoring.enabled);
    assert!(!service.settings().clipboard_monitoring.auto_enqueue);

    // Update settings to enable clipboard monitoring and auto enqueue
    let updated = service
        .update(SettingsPatch {
            clipboard_monitoring: Some(ClipboardMonitoringSettings {
                enabled: true,
                auto_enqueue: true,
            }),
            ..Default::default()
        })
        .expect("settings with clipboard monitoring should persist");

    assert!(updated.clipboard_monitoring.enabled);
    assert!(updated.clipboard_monitoring.auto_enqueue);

    // Reload from store to ensure persistence
    let reloaded = SettingsStore::new(&directory)
        .load(default_directory)
        .expect("reloaded settings should load");

    assert_eq!(reloaded.clipboard_monitoring, updated.clipboard_monitoring);
    std::fs::remove_dir_all(directory).expect("temporary directory cleanup");
}
