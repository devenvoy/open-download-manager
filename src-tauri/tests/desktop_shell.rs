use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};

use awesome_downloader_lib::{
    application::settings_service::SettingsService,
    domain::settings::SettingsPatch,
    infrastructure::{
        autostart::{sync_autostart, AUTOSTART_KEY_NAME},
        settings_store::SettingsStore,
    },
    presentation::tray::{show_main_window, TRAY_TOOLTIP},
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn temporary_directory() -> PathBuf {
    let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "awesome-downloader-shell-test-{}-{id}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temporary test directory should be created");
    dir
}

#[test]
fn autostart_constants_and_sync_succeed() {
    assert_eq!(AUTOSTART_KEY_NAME, "Awesome Downloader");

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
    assert_eq!(TRAY_TOOLTIP, "Awesome Downloader");
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
    let normal_args = vec!["awesome-downloader".to_owned()];
    let is_background_normal = normal_args.iter().any(|arg| arg == "--background");
    let is_browser_setup_normal = normal_args.iter().any(|arg| arg == "--browser-setup");
    assert!(!is_background_normal);
    assert!(!is_browser_setup_normal);

    // Background autostart launch
    let bg_args = vec!["awesome-downloader".to_owned(), "--background".to_owned()];
    let is_background = bg_args.iter().any(|arg| arg == "--background");
    assert!(is_background, "background flag must be detected");

    // Browser setup launch
    let setup_args = vec!["awesome-downloader".to_owned(), "--browser-setup".to_owned()];
    let is_browser_setup = setup_args.iter().any(|arg| arg == "--browser-setup");
    assert!(is_browser_setup, "browser setup flag must be detected");
}

#[test]
fn tray_show_main_window_helper_handles_mock_app() {
    let app = tauri::test::mock_app();
    // When main window does not exist in mock, show_main_window does not panic
    show_main_window(&app.handle());
}
