// Created: 2026-11-05
// Reason: feat: migrate history and adopt Bun
// Last updated: 2026-11-05
// Reason: rebrand: migrate project to Open Download Manager
// Description: Integration tests verifying download history functionality, edge cases, and error recovery.

use std::fs;

use open_download_manager_lib::{
    application::download_history_service::DownloadHistoryService,
    domain::{download::DownloadStatus, settings::Settings, snapshot::Snapshot},
    infrastructure::download_history_store::DownloadHistoryStore,
};

fn temporary_directory() -> tempfile::TempDir {
    tempfile::tempdir().expect("temporary data directory should be created")
}

#[test]
fn legacy_history_loads_with_runtime_state_reset_and_secrets_excluded() {
    let temp = temporary_directory();
    let directory = temp.path();
    let history_path = directory.join("downloads.json");
    let original_history = br#"{
        "version": 1,
        "settings": {},
        "downloads": [
            {
                "id": "download-1",
                "url": "https://example.test/file?token=private",
                "filename": "file.bin",
                "status": "downloading",
                "downloaded": 24,
                "total": 100,
                "speed": 32,
                "createdAt": "2025-01-02T03:04:05.000Z",
                "headers": {"authorization": "private-header"},
                "validator": "private-validator",
                "sabr": {"cookie": "private-session"},
                "segments": [{"url": "https://example.test/segment?signature=private"}]
            },
            {
                "id": "download-2",
                "url": "https://example.test/complete",
                "filename": "complete.bin",
                "status": "complete",
                "downloaded": 9,
                "total": 9,
                "speed": 0,
                "createdAt": "2025-01-03T03:04:05.000Z"
            }
        ]
    }"#;
    fs::write(&history_path, original_history).expect("legacy history should be written");

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("legacy history should load");
    let downloads = history.downloads().expect("history should remain readable");

    assert_eq!(downloads.len(), 2);
    assert_eq!(
        downloads[0].status,
        open_download_manager_lib::domain::download::DownloadStatus::Paused
    );
    assert_eq!(downloads[0].speed, 0);
    assert_eq!(downloads[0].downloaded, 24);
    assert_eq!(
        downloads[1].status,
        open_download_manager_lib::domain::download::DownloadStatus::Completed
    );
    let snapshot = Snapshot::new(
        Settings::new(directory.join("downloads")),
        downloads.to_vec(),
    );
    let serialized = serde_json::to_string(&snapshot).expect("history snapshot should serialize");
    let serialized_snapshot: serde_json::Value =
        serde_json::from_str(&serialized).expect("snapshot should be valid JSON");
    assert_eq!(serialized_snapshot["downloads"][0]["status"], "paused");
    assert_eq!(
        serialized_snapshot["downloads"][0]["url"],
        "https://example.test/file"
    );
    assert!(!serialized.contains("private-header"));
    assert!(!serialized.contains("private-validator"));
    assert!(!serialized.contains("private-session"));
    assert!(!serialized.contains("signature"));
    assert!(!serialized.contains("token=private"));
    assert_eq!(
        fs::read(&history_path).expect("legacy history should remain readable"),
        original_history
    );
}

#[test]
fn missing_history_is_an_empty_list() {
    let temp = temporary_directory();
    let directory = temp.path();

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(directory))
        .expect("missing history is expected for a fresh installation");

    assert!(history
        .downloads()
        .expect("history should remain readable")
        .is_empty());
}

#[test]
fn malformed_or_unsupported_history_returns_an_error() {
    let temp = temporary_directory();
    let directory = temp.path();
    let path = directory.join("downloads.json");
    fs::write(&path, b"{not-json").expect("malformed history should be written");

    let malformed = match DownloadHistoryService::new(DownloadHistoryStore::new(directory)) {
        Ok(_) => panic!("malformed history should not be hidden"),
        Err(error) => error,
    };
    assert_eq!(malformed.code, "storage_error");

    fs::write(&path, br#"{"version":2,"downloads":[]}"#)
        .expect("unsupported history version should be written");
    let unsupported = match DownloadHistoryService::new(DownloadHistoryStore::new(directory)) {
        Ok(_) => panic!("unsupported history versions should be reported"),
        Err(error) => error,
    };
    assert_eq!(unsupported.code, "storage_error");
}

#[test]
fn legacy_segment_purge_marks_history_records_as_needs_restart() {
    let temp = temporary_directory();
    let directory = temp.path();
    let history_path = directory.join("downloads.json");

    // Write a download record that was paused/downloading in legacy format
    let history_json = serde_json::json!({
        "version": 1,
        "downloads": [{
            "id": "legacy-purge-test",
            "url": "https://example.test/bigfile.bin",
            "filename": "bigfile.bin",
            "status": "downloading",
            "downloaded": 500000,
            "total": 1000000,
            "directory": directory.to_str().unwrap(),
            "createdAt": "2026-11-01T00:00:00Z"
        }]
    });
    fs::write(&history_path, serde_json::to_vec_pretty(&history_json).unwrap()).unwrap();

    // Create stale legacy segment files
    let part_0 = directory.join(".odm.legacy-purge-test.segment-0.part");
    let part_1 = directory.join(".odm.legacy-purge-test.segment-1.part");
    fs::write(&part_0, b"legacy-segment-0-data").unwrap();
    fs::write(&part_1, b"legacy-segment-1-data").unwrap();
    assert!(part_0.exists());
    assert!(part_1.exists());

    // Initialize history store
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(directory)).unwrap();
    let records = history.downloads().unwrap();
    assert_eq!(records.len(), 1);

    let record = &records[0];
    assert_eq!(record.id, "legacy-purge-test");
    // Verify stale segment files were deleted
    assert!(!part_0.exists(), "Legacy segment-0 file must be purged");
    assert!(!part_1.exists(), "Legacy segment-1 file must be purged");

    // Verify record was marked needs restart
    assert_eq!(record.status, DownloadStatus::Failed);
    assert_eq!(record.downloaded, 0, "Downloaded counter must be reset to 0");
    assert!(
        record.error.as_deref().unwrap_or("").contains("needs restart"),
        "Error message must indicate download needs restart"
    );
}
