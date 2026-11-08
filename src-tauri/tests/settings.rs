// Created: 2026-11-05
// Reason: feat: scaffold Tauri settings shell
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Description: Integration tests verifying settings functionality, edge cases, and error recovery.

use std::{
    collections::BTreeMap,
    path::PathBuf,
};

use open_download_manager_lib::{
    application::settings_service::{SettingsRepository, SettingsService},
    domain::{
        category::DownloadCategory,
        settings::{Settings, SettingsPatch},
    },
    infrastructure::settings_store::SettingsStore,
};

fn temporary_directory() -> PathBuf {
    let dir = tempfile::Builder::new()
        .prefix("open-download-manager-settings-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep();
    dir.join("store")
}

#[test]
fn settings_update_is_validated_and_persisted() {
    let directory = temporary_directory();
    let default_directory = std::env::temp_dir().join("downloads");
    let updated_directory = std::env::temp_dir().join("media");
    let video_directory = updated_directory.join("videos");
    let service = SettingsService::new(SettingsStore::new(&directory), default_directory)
        .expect("default settings should load");
    let mut service = service;
    let mut category_directories = BTreeMap::new();
    category_directories.insert(
        DownloadCategory::Video,
        video_directory.to_string_lossy().into_owned(),
    );

    let updated = service
        .update(SettingsPatch {
            default_directory: Some(updated_directory.to_string_lossy().into_owned()),
            max_concurrent: Some(5),
            connections: Some(12),
            speed_limit_kbps: Some(64),
            category_directories: Some(category_directories),
            temporary_directory: None,
            notify_on_complete: Some(false),
            reveal_on_complete: None,
            confirm_browser_downloads: None,
            start_with_windows: Some(false),
            hide_completion_dialog: None,
            proxy: None,
            capture_policy: None,
            completion_actions: None,
        })
        .expect("valid settings should save");

    assert_eq!(updated.default_directory, updated_directory);
    assert_eq!(updated.max_concurrent, 5);
    assert_eq!(updated.speed_limit_kbps, 64);
    assert_eq!(
        updated.category_directories.get(&DownloadCategory::Video),
        Some(&video_directory)
    );
    assert!(!updated.notify_on_complete);
    assert!(!updated.start_with_windows);

    let reloaded = SettingsStore::new(&directory)
        .load(PathBuf::from("/ignored"))
        .expect("persisted settings should load");
    assert_eq!(reloaded, updated);
    std::fs::remove_dir_all(directory).expect("temporary test data should be removed");
}

#[test]
fn invalid_setting_does_not_change_or_persist_current_state() {
    let directory = temporary_directory();
    let default_directory = std::env::temp_dir().join("downloads");
    let mut service =
        SettingsService::new(SettingsStore::new(&directory), default_directory.clone())
            .expect("default settings should load");
    let invalid = SettingsPatch {
        default_directory: Some("relative/path".to_owned()),
        max_concurrent: Some(99),
        connections: None,
        speed_limit_kbps: None,
        category_directories: None,
        temporary_directory: None,
        notify_on_complete: None,
        reveal_on_complete: None,
        confirm_browser_downloads: None,
        start_with_windows: None,
        hide_completion_dialog: None,
        proxy: None,
        capture_policy: None,
        completion_actions: None,
    };

    let error = service
        .update(invalid)
        .expect_err("invalid settings must be rejected");

    assert_eq!(error.code, "invalid_settings");
    assert_eq!(service.settings(), &Settings::new(default_directory));
    assert!(!directory.exists());
}

#[test]
fn relative_category_directory_is_rejected() {
    let directory = temporary_directory();
    let default_directory = std::env::temp_dir().join("downloads");
    let mut service = SettingsService::new(SettingsStore::new(&directory), default_directory)
        .expect("default settings should load");
    let mut category_directories = BTreeMap::new();
    category_directories.insert(DownloadCategory::Video, "relative/path".to_owned());

    let error = service
        .update(SettingsPatch {
            default_directory: None,
            max_concurrent: None,
            connections: None,
            speed_limit_kbps: None,
            category_directories: Some(category_directories),
            temporary_directory: None,
            notify_on_complete: None,
            reveal_on_complete: None,
            confirm_browser_downloads: None,
            start_with_windows: None,
            hide_completion_dialog: None,
            proxy: None,
            capture_policy: None,
            completion_actions: None,
        })
        .expect_err("category directories must be absolute");

    assert_eq!(error.code, "invalid_settings");
    assert!(!directory.exists());
}
