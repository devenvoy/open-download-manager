use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use awesome_downloader_lib::{
    application::download_history_service::DownloadHistoryService,
    domain::{settings::Settings, snapshot::Snapshot},
    infrastructure::download_history_store::DownloadHistoryStore,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn temporary_directory() -> PathBuf {
    let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "awesome-downloader-history-test-{}-{id}",
        std::process::id()
    ))
}

#[test]
fn legacy_history_loads_with_runtime_state_reset_and_secrets_excluded() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary data directory should be created");
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
    let downloads = history.downloads();

    assert_eq!(downloads.len(), 2);
    assert_eq!(
        downloads[0].status,
        awesome_downloader_lib::domain::download::DownloadStatus::Paused
    );
    assert_eq!(downloads[0].speed, 0);
    assert_eq!(downloads[0].downloaded, 24);
    assert_eq!(
        downloads[1].status,
        awesome_downloader_lib::domain::download::DownloadStatus::Completed
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

    fs::remove_dir_all(directory).expect("temporary data directory should be removed");
}

#[test]
fn missing_history_is_an_empty_list() {
    let directory = temporary_directory();

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("missing history is expected for a fresh installation");

    assert!(history.downloads().is_empty());
}

#[test]
fn malformed_or_unsupported_history_returns_an_error() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary data directory should be created");
    let path = directory.join("downloads.json");
    fs::write(&path, b"{not-json").expect("malformed history should be written");

    let malformed = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect_err("malformed history should not be hidden");
    assert_eq!(malformed.code, "storage_error");

    fs::write(&path, br#"{"version":2,"downloads":[]}"#)
        .expect("unsupported history version should be written");
    let unsupported = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect_err("unsupported history versions should be reported");
    assert_eq!(unsupported.code, "storage_error");

    fs::remove_dir_all(directory).expect("temporary data directory should be removed");
}
