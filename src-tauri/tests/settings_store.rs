use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use awesome_downloader_lib::{
    application::settings_service::{SettingsRepository, SettingsService},
    domain::settings::SettingsPatch,
    infrastructure::settings_store::SettingsStore,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn temporary_directory() -> PathBuf {
    let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "awesome-downloader-store-test-{}-{id}",
        std::process::id()
    ))
}

#[test]
fn settings_updates_leave_legacy_download_history_untouched() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary data directory should be created");
    let legacy_history = directory.join("downloads.json");
    let original_history = br#"{"version":1,"settings":{"private":"legacy"},"downloads":[]}"#;
    fs::write(&legacy_history, original_history).expect("legacy history should be written");

    let mut service = SettingsService::new(
        SettingsStore::new(&directory),
        std::env::temp_dir().join("downloads"),
    )
    .expect("settings should load without changing legacy history");
    service
        .update(SettingsPatch {
            start_with_windows: Some(false),
            ..empty_patch()
        })
        .expect("settings should save separately from legacy history");
    service
        .update(SettingsPatch {
            notify_on_complete: Some(false),
            ..empty_patch()
        })
        .expect("subsequent updates should replace the settings document");

    assert_eq!(
        fs::read(&legacy_history).expect("legacy history should remain readable"),
        original_history
    );
    let settings = SettingsStore::new(&directory)
        .load(std::env::temp_dir().join("downloads"))
        .expect("latest settings should remain readable");
    assert!(!settings.start_with_windows);
    assert!(!settings.notify_on_complete);
    fs::remove_dir_all(directory).expect("temporary test data should be removed");
}

#[test]
fn malformed_settings_return_an_error_instead_of_resetting_to_defaults() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary data directory should be created");
    fs::write(directory.join("settings.json"), b"{not-json")
        .expect("malformed settings fixture should be written");

    let error = SettingsStore::new(&directory)
        .load(std::env::temp_dir().join("downloads"))
        .expect_err("malformed persisted settings must be reported");

    assert_eq!(error.code, "storage_error");
    fs::remove_dir_all(directory).expect("temporary test data should be removed");
}

fn empty_patch() -> SettingsPatch {
    SettingsPatch {
        default_directory: None,
        max_concurrent: None,
        connections: None,
        speed_limit_kbps: None,
        category_directories: None,
        notify_on_complete: None,
        reveal_on_complete: None,
        confirm_browser_downloads: None,
        start_with_windows: None,
        hide_completion_dialog: None,
    }
}
