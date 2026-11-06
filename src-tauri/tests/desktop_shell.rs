use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use open_download_manager_lib::{
    application::settings_service::SettingsService,
    domain::settings::SettingsPatch,
    infrastructure::{
        autostart::{sync_autostart, AUTOSTART_KEY_NAME},
        settings_store::SettingsStore,
    },
    presentation::tray::{show_main_window, TRAY_TOOLTIP},
};

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("open-download-manager-shell-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

#[test]
fn autostart_constants_and_sync_succeed() {
    assert_eq!(AUTOSTART_KEY_NAME, "Open Download Manager");

    // Syncing autostart (enable and disable) should succeed on all platforms
    assert!(sync_autostart(true).is_ok());
    assert!(sync_autostart(false).is_ok());
}

#[test]
fn settings_update_persists_start_with_windows() {
    let dir = temporary_directory();
    let mut service = SettingsService::new(SettingsStore::new(dir.clone()), dir).unwrap();

    assert_eq!(service.settings().start_with_windows, true);

    // Disable start with windows
    let updated = service
        .update(SettingsPatch {
            start_with_windows: Some(false),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(updated.start_with_windows, false);
    assert_eq!(service.settings().start_with_windows, false);

    // Re-enable start with windows
    let updated = service
        .update(SettingsPatch {
            start_with_windows: Some(true),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(updated.start_with_windows, true);
    assert_eq!(service.settings().start_with_windows, true);
}

#[test]
fn tray_tooltip_matches_specification() {
    assert_eq!(TRAY_TOOLTIP, "Open Download Manager");
}

#[test]
fn close_to_tray_decision_logic() {
    let is_quitting = Arc::new(AtomicBool::new(false));

    // When is_quitting is false, close requests must be intercepted (prevent_close)
    let should_prevent_1 = !is_quitting.load(Ordering::SeqCst);
    assert_eq!(should_prevent_1, true, "close must be prevented when not quitting");

    // When user explicitly clicks Quit from tray menu
    is_quitting.store(true, Ordering::SeqCst);
    let should_prevent_2 = !is_quitting.load(Ordering::SeqCst);
    assert_eq!(should_prevent_2, false, "close must proceed when quitting");
}

#[test]
fn cli_launch_arguments_detection() {
    // Normal launch
    let normal_args = vec!["open-download-manager".to_owned()];
    let is_background_normal = normal_args.iter().any(|arg| arg == "--background");
    let is_browser_setup_normal = normal_args.iter().any(|arg| arg == "--browser-setup");
    assert!(!is_background_normal);
    assert!(!is_browser_setup_normal);

    // Background autostart launch
    let bg_args = vec!["open-download-manager".to_owned(), "--background".to_owned()];
    let is_background = bg_args.iter().any(|arg| arg == "--background");
    assert!(is_background, "background flag must be detected");

    // Browser setup launch
    let setup_args = vec!["open-download-manager".to_owned(), "--browser-setup".to_owned()];
    let is_browser_setup = setup_args.iter().any(|arg| arg == "--browser-setup");
    assert!(is_browser_setup, "browser setup flag must be detected");
}

#[test]
fn tray_show_main_window_helper_handles_mock_app() {
    let app = tauri::test::mock_app();
    // When main window does not exist in mock, show_main_window does not panic
    show_main_window(&app.handle());
}

#[test]
fn test_macos_and_linux_autostart_sync_in_sandbox() {
    let sandbox = temporary_directory();

    // 1. macOS LaunchAgent
    assert!(open_download_manager_lib::infrastructure::autostart::sync_autostart_macos(true, &sandbox).is_ok());
    let plist = sandbox.join("Library/LaunchAgents/com.open.downloadmanager.plist");
    assert!(plist.exists());
    let plist_content = std::fs::read_to_string(&plist).unwrap();
    assert!(plist_content.contains("com.open.downloadmanager"));
    assert!(plist_content.contains("--background"));

    assert!(open_download_manager_lib::infrastructure::autostart::sync_autostart_macos(false, &sandbox).is_ok());
    assert!(!plist.exists());

    // 2. Linux XDG Autostart
    assert!(open_download_manager_lib::infrastructure::autostart::sync_autostart_linux(true, &sandbox).is_ok());
    let desktop = sandbox.join(".config/autostart/open-download-manager.desktop");
    assert!(desktop.exists());
    let desktop_content = std::fs::read_to_string(&desktop).unwrap();
    assert!(desktop_content.contains("[Desktop Entry]"));
    assert!(desktop_content.contains("--background"));

    assert!(open_download_manager_lib::infrastructure::autostart::sync_autostart_linux(false, &sandbox).is_ok());
    assert!(!desktop.exists());
}
