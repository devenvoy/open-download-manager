// Created: 2026-11-08
// Reason: feat(theme): add light, dark, and system theme options to the desktop app (Closes #4)
// Description: Human-scenario integration tests verifying theme settings, persistence, schema migration, and validation.

use std::{fs, path::PathBuf};

use open_download_manager_lib::{
    application::settings_service::SettingsService,
    domain::settings::{AppTheme, SettingsPatch},
    infrastructure::settings_store::SettingsStore,
};

fn temporary_directory() -> PathBuf {
    let dir = tempfile::Builder::new()
        .prefix("odm-theme-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep();
    dir.join("store")
}

#[test]
fn human_scenario_theme_defaults_to_system() {
    let directory = temporary_directory();
    let default_download_dir = std::env::temp_dir().join("downloads");
    let service = SettingsService::new(SettingsStore::new(&directory), default_download_dir)
        .expect("default settings should load");

    assert_eq!(
        service.settings().theme,
        AppTheme::System,
        "Acceptance criteria: System must be the default theme option"
    );
    assert_eq!(service.settings().theme.as_str(), "system");
}

#[test]
fn human_scenario_theme_selection_persists_across_app_restarts() {
    let directory = temporary_directory();
    let default_download_dir = std::env::temp_dir().join("downloads");

    // 1. Initial boot: default is system
    let mut service = SettingsService::new(SettingsStore::new(&directory), default_download_dir.clone())
        .expect("initial settings should load");
    assert_eq!(service.settings().theme, AppTheme::System);

    // 2. User selects Dark mode in Options dialog
    let updated = service
        .update(SettingsPatch {
            theme: Some(AppTheme::Dark),
            ..Default::default()
        })
        .expect("updating theme to dark must succeed");
    assert_eq!(updated.theme, AppTheme::Dark);
    assert_eq!(updated.theme.as_str(), "dark");

    // 3. Simulate app restart: reload from disk
    let reloaded_service = SettingsService::new(SettingsStore::new(&directory), default_download_dir.clone())
        .expect("reloaded settings should load");
    assert_eq!(
        reloaded_service.settings().theme,
        AppTheme::Dark,
        "Dark theme must persist across restarts"
    );

    // 4. User changes theme to Light mode
    let mut service2 = reloaded_service;
    let light_updated = service2
        .update(SettingsPatch {
            theme: Some(AppTheme::Light),
            ..Default::default()
        })
        .expect("updating theme to light must succeed");
    assert_eq!(light_updated.theme, AppTheme::Light);
    assert_eq!(light_updated.theme.as_str(), "light");

    // 5. Restart again: verify Light persists
    let reloaded_service2 = SettingsService::new(SettingsStore::new(&directory), default_download_dir.clone())
        .expect("reloaded settings should load");
    assert_eq!(
        reloaded_service2.settings().theme,
        AppTheme::Light,
        "Light theme must persist across restarts"
    );

    // 6. User switches back to System
    let mut service3 = reloaded_service2;
    let system_updated = service3
        .update(SettingsPatch {
            theme: Some(AppTheme::System),
            ..Default::default()
        })
        .expect("updating theme to system must succeed");
    assert_eq!(system_updated.theme, AppTheme::System);

    let reloaded_service3 = SettingsService::new(SettingsStore::new(&directory), default_download_dir)
        .expect("reloaded settings should load");
    assert_eq!(reloaded_service3.settings().theme, AppTheme::System);
}

#[test]
fn human_scenario_safe_migration_preserves_legacy_settings_without_theme() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("create test store directory");
    let settings_file = directory.join("settings.json");

    // Construct a valid v1 settings JSON file missing the "theme" property
    let legacy_json = serde_json::json!({
        "version": 1,
        "settings": {
            "defaultDirectory": std::env::temp_dir().join("legacy-downloads"),
            "maxConcurrent": 4,
            "connections": 6,
            "speedLimitKbps": 1024,
            "categoryDirectories": {},
            "temporaryDirectory": null,
            "notifyOnComplete": true,
            "revealOnComplete": false,
            "confirmBrowserDownloads": true,
            "startWithWindows": false,
            "hideCompletionDialog": true,
            "proxy": {
                "enabled": false,
                "protocol": "http",
                "host": "",
                "port": 8080,
                "username": null,
                "password": null,
                "resolveDnsRemotely": false
            },
            "capturePolicy": {
                "enabled": true,
                "confirmationMode": "automatic",
                "fileTypes": ["zip", "iso"],
                "excludedHosts": [],
                "excludedAddresses": []
            },
            "completionActions": [],
            "clipboardMonitoring": {
                "enabled": false,
                "autoEnqueue": false
            },
            "connectionOverrides": [],
            "windowLimit": null
        }
    });

    fs::write(&settings_file, serde_json::to_vec_pretty(&legacy_json).unwrap())
        .expect("write legacy settings");

    // Load store: it must load successfully and default theme to System
    let service = SettingsService::new(
        SettingsStore::new(&directory),
        std::env::temp_dir().join("fallback"),
    )
    .expect("legacy settings without theme must deserialize cleanly");

    assert_eq!(
        service.settings().theme,
        AppTheme::System,
        "Missing theme in stored JSON must default to System"
    );
    assert_eq!(service.settings().max_concurrent, 4);
    assert_eq!(service.settings().connections, 6);
}

#[test]
fn human_scenario_invalid_theme_in_patch_is_rejected() {
    let directory = temporary_directory();
    let service = SettingsService::new(
        SettingsStore::new(&directory),
        std::env::temp_dir().join("downloads"),
    )
    .expect("settings load");

    // JSON patch with invalid theme value
    let invalid_json = serde_json::json!({
        "theme": "cyberpunk-neon"
    });

    let patch_result = serde_json::from_value::<SettingsPatch>(invalid_json);
    assert!(
        patch_result.is_err(),
        "Unknown theme value must fail deserialization and be rejected"
    );

    // Existing settings remain untouched
    assert_eq!(service.settings().theme, AppTheme::System);
}
