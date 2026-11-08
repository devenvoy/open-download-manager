// Created: 2026-11-08
// Reason: fix(network): report proxy reconfiguration failures instead of silently saving (Closes #27) (#42)
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Description: Integration tests verifying proxy reconfiguration functionality, edge cases, and error recovery.

use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::Duration,
};

use open_download_manager_lib::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        error::AppResult,
        integration_service::IntegrationService,
        settings_service::{SettingsRepository, SettingsService},
        shell_service::ShellService,
    },
    domain::{
        download::{DownloadRequest, DownloadStatus},
        settings::{ProxyProtocol, ProxySettings, SettingsPatch},
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        http_download_executor::HttpDownloadExecutor,
        settings_store::SettingsStore,
    },
    presentation::{
        app_state::AppState,
        commands::{add_download, snapshot, test_proxy_connection, update_settings},
    },
};
use tauri::Manager;

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("open-download-manager-proxy-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

#[derive(Default, Clone)]
struct DummyShell;

impl ShellService for DummyShell {
    fn open_path(&self, _path: &std::path::Path) -> AppResult<()> {
        Ok(())
    }
    fn reveal_path(&self, _path: &std::path::Path) -> AppResult<()> {
        Ok(())
    }
    fn open_with(&self, _path: &std::path::Path) -> AppResult<()> {
        Ok(())
    }
}

fn create_test_app(data_dir: &Path) -> tauri::App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let settings = SettingsService::new(
        SettingsStore::new(data_dir.to_path_buf()),
        data_dir.to_path_buf(),
    )
    .expect("settings service should initialize");

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(data_dir))
        .expect("history service should initialize");

    let transfer = DownloadTransferService::with_shell(
        history.clone(),
        HttpDownloadExecutor::new().expect("http executor should initialize"),
        2,
        DummyShell,
    );

    let integration = IntegrationService::with_shell(
        data_dir.to_path_buf(),
        data_dir.to_path_buf(),
        DummyShell,
    );

    let file_icons = open_download_manager_lib::infrastructure::file_icons::FileIconProvider::new(
        data_dir.join("FileTypeIcons"),
    );

    let update_client = Arc::new(
        open_download_manager_lib::infrastructure::updater::GitHubReleaseClient::default_client(),
    );
    let update_service = open_download_manager_lib::application::updater_service::UpdateService::new(
        update_client,
        "0.1.2",
        true,
    );

    let cred_store = std::sync::Arc::new(
        open_download_manager_lib::infrastructure::credentials::CredentialStore::new(data_dir),
    );
    let cred_service = std::sync::Arc::new(
        open_download_manager_lib::application::credential_service::CredentialService::new(cred_store),
    );

    let app_state = AppState::new(
        settings,
        history,
        transfer,
        cred_service,
        integration,
        file_icons,
        update_service,
    );
    app.manage(app_state);
    app
}

fn serve_simple_file(bytes: Vec<u8>) -> (String, std::sync::mpsc::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("local address");
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();

    thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .expect("set non-blocking");
        while shutdown_rx.try_recv().is_err() {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0_u8; 1024];
                    let _ = stream.read(&mut buffer);
                    let body = &bytes;
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.write_all(body);
                    let _ = stream.flush();
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    (format!("http://{address}/proxy_target.bin"), shutdown_tx)
}

#[tokio::test]
async fn invalid_proxy_host_or_port_fails_validation_and_leaves_settings_untouched() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let app = create_test_app(&dir);
    let state = app.state::<AppState>();

    let initial_settings = snapshot(state.clone()).unwrap().settings;
    assert!(!initial_settings.proxy.enabled);

    // 1. Attempt to enable proxy with an empty host
    let invalid_patch = SettingsPatch {
        default_directory: None,
        max_concurrent: None,
        connections: None,
        speed_limit_kbps: None,
        category_directories: None,
        temporary_directory: None,
        notify_on_complete: None,
        reveal_on_complete: None,
        confirm_browser_downloads: None,
        start_with_windows: None,
        hide_completion_dialog: None,
        proxy: Some(ProxySettings {
            enabled: true,
            protocol: ProxyProtocol::Http,
            host: "   ".to_string(),
            port: 8080,
            username: None,
            password: None,
        }),
        capture_policy: None,
        completion_actions: None,
    };

    let err = update_settings(state.clone(), invalid_patch)
        .expect_err("update with empty proxy host must fail");
    assert_eq!(err.code, "invalid_settings");
    assert!(err.message.contains("Proxy host cannot be empty"));

    // Verify settings in memory and in store were not modified
    let current = snapshot(state.clone()).unwrap().settings;
    assert_eq!(current, initial_settings);

    // 2. Attempt to configure proxy with port 0
    let invalid_port_patch = SettingsPatch {
        default_directory: None,
        max_concurrent: None,
        connections: None,
        speed_limit_kbps: None,
        category_directories: None,
        temporary_directory: None,
        notify_on_complete: None,
        reveal_on_complete: None,
        confirm_browser_downloads: None,
        start_with_windows: None,
        hide_completion_dialog: None,
        proxy: Some(ProxySettings {
            enabled: true,
            protocol: ProxyProtocol::Http,
            host: "127.0.0.1".to_string(),
            port: 0,
            username: None,
            password: None,
        }),
        capture_policy: None,
        completion_actions: None,
    };

    let err = update_settings(state.clone(), invalid_port_patch)
        .expect_err("update with port 0 must fail");
    assert!(err.message.contains("Proxy port must be between 1 and 65535"));

    // Recheck settings are untouched
    let reloaded = snapshot(state.clone()).unwrap().settings;
    assert_eq!(reloaded, initial_settings);

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn proxy_reconfiguration_failure_surfaces_error_and_preserves_old_client_and_settings() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let app = create_test_app(&dir);
    let state = app.state::<AppState>();

    let (url, _shutdown) = serve_simple_file(b"INITIAL_TRANSFER".to_vec());

    // 1. Perform an initial download to verify the initial client works
    let req = DownloadRequest {
        urls: vec![url.clone()],
        filename: Some("first.bin".to_string()),
        directory: Some(dir.to_string_lossy().into_owned()),
        temp_directory: None,
        category: None,
        description: None,
        queue_only: false,
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        sabr: None,
        reuse_active: None,
        referer: None,
        youtube_format: None,
        conflict_policy: None,
        credentials: None,
        completion_actions: None,
    };

    let ids = add_download(state.clone(), req).await.unwrap();
    for _ in 0..100 {
        let snap = snapshot(state.clone()).unwrap();
        if let Some(record) = snap.downloads.iter().find(|d| d.id == ids[0]) {
            if record.status == DownloadStatus::Completed {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let snap = snapshot(state.clone()).unwrap();
    let record = snap.downloads.iter().find(|d| d.id == ids[0]).unwrap();
    assert_eq!(record.status, DownloadStatus::Completed);

    let initial_settings = snap.settings.clone();

    // 2. Attempt to update proxy with an invalid hostname containing characters that fail URL/proxy creation
    let bad_proxy_patch = SettingsPatch {
        default_directory: None,
        max_concurrent: None,
        connections: None,
        speed_limit_kbps: None,
        category_directories: None,
        temporary_directory: None,
        notify_on_complete: None,
        reveal_on_complete: None,
        confirm_browser_downloads: None,
        start_with_windows: None,
        hide_completion_dialog: None,
        proxy: Some(ProxySettings {
            enabled: true,
            protocol: ProxyProtocol::Http,
            host: "bad host name with spaces".to_string(),
            port: 8080,
            username: None,
            password: None,
        }),
        capture_policy: None,
        completion_actions: None,
    };

    let err = update_settings(state.clone(), bad_proxy_patch)
        .expect_err("update with invalid proxy host must fail");
    assert!(
        err.message.contains("Proxy host must be a valid hostname")
            || err.message.contains("Invalid proxy configuration")
    );

    // 3. Verify settings were NOT updated
    let current_settings = snapshot(state.clone()).unwrap().settings;
    assert_eq!(
        current_settings, initial_settings,
        "Settings must not be updated when proxy reconfiguration fails"
    );

    // 4. Verify existing working client is STILL active and can download
    let req2 = DownloadRequest {
        urls: vec![url],
        filename: Some("second.bin".to_string()),
        directory: Some(dir.to_string_lossy().into_owned()),
        temp_directory: None,
        category: None,
        description: None,
        queue_only: false,
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        sabr: None,
        reuse_active: None,
        referer: None,
        youtube_format: None,
        conflict_policy: None,
        credentials: None,
        completion_actions: None,
    };

    let ids2 = add_download(state.clone(), req2).await.unwrap();
    for _ in 0..100 {
        let snap2 = snapshot(state.clone()).unwrap();
        if let Some(record) = snap2.downloads.iter().find(|d| d.id == ids2[0]) {
            if record.status == DownloadStatus::Completed {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let snap2 = snapshot(state.clone()).unwrap();
    let record2 = snap2.downloads.iter().find(|d| d.id == ids2[0]).unwrap();
    assert_eq!(
        record2.status,
        DownloadStatus::Completed,
        "Subsequent downloads must succeed through the preserved active client"
    );

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn successful_proxy_update_persists_settings_and_updates_runtime() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let app = create_test_app(&dir);
    let state = app.state::<AppState>();

    let valid_proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Http,
        host: "127.0.0.1".to_string(),
        port: 9090,
        username: Some("proxy_user".to_string()),
        password: Some("proxy_pass".to_string()),
    };

    let patch = SettingsPatch {
        default_directory: None,
        max_concurrent: Some(8),
        connections: None,
        speed_limit_kbps: Some(512),
        category_directories: None,
        temporary_directory: None,
        notify_on_complete: None,
        reveal_on_complete: None,
        confirm_browser_downloads: None,
        start_with_windows: None,
        hide_completion_dialog: None,
        proxy: Some(valid_proxy.clone()),
        capture_policy: None,
        completion_actions: None,
    };

    let result = update_settings(state.clone(), patch).expect("valid proxy configuration should succeed");
    assert_eq!(result.proxy, valid_proxy);
    assert_eq!(result.max_concurrent, 8);
    assert_eq!(result.speed_limit_kbps, 512);

    // Verify persisted in memory
    let in_memory = snapshot(state.clone()).unwrap().settings;
    assert_eq!(in_memory.proxy, valid_proxy);
    assert_eq!(in_memory.max_concurrent, 8);

    // Verify persisted on disk in store
    let on_disk = SettingsStore::new(&dir)
        .load(dir.clone())
        .expect("should load from disk");
    assert_eq!(on_disk.proxy, valid_proxy);
    assert_eq!(on_disk.max_concurrent, 8);

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn credentials_never_appear_in_error_messages_or_diagnostics() {
    let secret_password = "very_confidential_password_xyz987!";
    let secret_user = "classified_user_admin";

    let proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Http,
        host: "invalid-nonexistent-domain-that-fails.invalid".to_string(),
        port: 8080,
        username: Some(secret_user.to_string()),
        password: Some(secret_password.to_string()),
    };

    // 1. Test test_proxy_connection error message
    let test_res = test_proxy_connection(proxy.clone(), None).await.unwrap();
    assert!(!test_res.success);
    assert!(
        !test_res.message.contains(secret_password),
        "Secret password leaked in test_proxy_connection result: {}",
        test_res.message
    );
    assert!(
        !test_res.message.contains(secret_user),
        "Secret username leaked in test_proxy_connection result: {}",
        test_res.message
    );

    // 2. Test sanitize_error explicitly
    let raw_err = format!("Connection to http://{secret_user}:{secret_password}@proxy.local:8080 failed");
    let sanitized = proxy.sanitize_error(&raw_err);
    assert!(!sanitized.contains(secret_password));
    assert!(!sanitized.contains(secret_user));

    // 3. Test invalid host with credentials embedded
    let embedded_cred_proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Http,
        host: format!("{secret_user}:{secret_password}@127.0.0.1"),
        port: 8080,
        username: Some(secret_user.to_string()),
        password: Some(secret_password.to_string()),
    };
    let validation_err = embedded_cred_proxy.validate().expect_err("embedded credentials in host must fail");
    assert!(
        !validation_err.message.contains(secret_password),
        "Password leaked in validation error: {}",
        validation_err.message
    );
    assert!(
        !validation_err.message.contains(secret_user),
        "Username leaked in validation error: {}",
        validation_err.message
    );
}
