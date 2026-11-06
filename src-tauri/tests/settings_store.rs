use std::fs;

use open_download_manager_lib::{
    application::settings_service::{SettingsRepository, SettingsService},
    domain::category::DownloadCategory,
    domain::settings::SettingsPatch,
    infrastructure::settings_store::SettingsStore,
};

fn temporary_directory() -> tempfile::TempDir {
    tempfile::tempdir().expect("temporary data directory should be created")
}

#[test]
fn settings_updates_leave_legacy_download_history_untouched() {
    let temp = temporary_directory();
    let directory = temp.path();
    let legacy_history = directory.join("downloads.json");
    let original_history = br#"{"version":1,"settings":{"private":"legacy"},"downloads":[]}"#;
    fs::write(&legacy_history, original_history).expect("legacy history should be written");

    let mut service = SettingsService::new(
        SettingsStore::new(directory),
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
    let settings = SettingsStore::new(directory)
        .load(std::env::temp_dir().join("downloads"))
        .expect("latest settings should remain readable");
    assert!(!settings.start_with_windows);
    assert!(!settings.notify_on_complete);
}

#[test]
fn malformed_settings_return_an_error_instead_of_resetting_to_defaults() {
    let temp = temporary_directory();
    let directory = temp.path();
    fs::write(directory.join("settings.json"), b"{not-json")
        .expect("malformed settings fixture should be written");

    let error = SettingsStore::new(directory)
        .load(std::env::temp_dir().join("downloads"))
        .expect_err("malformed persisted settings must be reported");

    assert_eq!(error.code, "storage_error");
}

#[test]
fn legacy_settings_are_migrated_without_modifying_download_history() {
    let temp = temporary_directory();
    let directory = temp.path();
    let history_path = directory.join("downloads.json");
    let original_history = format!(
        r#"{{
            "version": 1,
            "settings": {{
                "defaultDirectory": {},
                "maxConcurrent": 5,
                "connections": 12,
                "speedLimitKbps": 256,
                "categoryDirectories": {{"Video": {}}},
                "notifyOnComplete": false,
                "revealOnComplete": true,
                "confirmBrowserDownloads": false,
                "startWithWindows": false,
                "hideCompletionDialog": true
            }},
            "downloads": []
        }}"#,
        serde_json::to_string(&directory.to_string_lossy()).unwrap(),
        serde_json::to_string(&directory.to_string_lossy()).unwrap()
    );
    fs::write(&history_path, &original_history).expect("legacy history should be written");

    let loaded = SettingsStore::new(directory)
        .load(std::env::temp_dir().join("downloads"))
        .expect("legacy settings should migrate");

    assert_eq!(loaded.default_directory, directory);
    assert_eq!(loaded.max_concurrent, 5);
    assert_eq!(loaded.connections, 12);
    assert_eq!(loaded.speed_limit_kbps, 256);
    assert_eq!(
        loaded.category_directories.get(&DownloadCategory::Video),
        Some(&directory.to_path_buf())
    );
    assert!(!loaded.notify_on_complete);
    assert!(loaded.reveal_on_complete);
    assert!(!loaded.confirm_browser_downloads);
    assert!(!loaded.start_with_windows);
    assert!(loaded.hide_completion_dialog);
    assert_eq!(
        fs::read_to_string(&history_path).expect("legacy history should remain readable"),
        original_history
    );

    let persisted =
        fs::read(directory.join("settings.json")).expect("versioned settings should be written");
    let persisted: serde_json::Value =
        serde_json::from_slice(&persisted).expect("migrated settings should be valid JSON");
    assert_eq!(persisted["version"], 1);
    assert_eq!(persisted["settings"]["maxConcurrent"], 5);
}

#[test]
fn invalid_legacy_settings_fail_without_replacing_the_history_or_creating_settings() {
    let temp = temporary_directory();
    let directory = temp.path();
    let history_path = directory.join("downloads.json");
    let original_history = br#"{"version":1,"settings":{"maxConcurrent":99},"downloads":[]}"#;
    fs::write(&history_path, original_history).expect("legacy history should be written");

    let error = SettingsStore::new(directory)
        .load(std::env::temp_dir().join("downloads"))
        .expect_err("invalid legacy settings must be reported");

    assert_eq!(error.code, "storage_error");
    assert_eq!(
        fs::read(&history_path).expect("legacy history should remain available"),
        original_history
    );
    assert!(
        !directory.join("settings.json").exists(),
        "invalid settings must not create a migrated settings file"
    );
}

fn empty_patch() -> SettingsPatch {
    SettingsPatch::default()
}
