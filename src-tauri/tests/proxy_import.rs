// Created: 2026-11-08
// Reason: feat(proxy): import system and browser proxy configurations (Closes #39)
// Description: Human-scenario integration tests for discovering and transactionally importing system/browser proxies.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use open_download_manager_lib::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        error::AppResult,
        integration_service::IntegrationService,
        proxy_discovery_service::{MockProxyDiscoveryAdapter, ProxyDiscoveryService},
        settings_service::{SettingsRepository, SettingsService},
        shell_service::ShellService,
    },
    domain::settings::{
        DetectedProxyConfig, DetectedProxyMode, ProtocolRoute, ProxyProtocol, ProxySettings,
        ProxySource, SettingsPatch,
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore, http_download_executor::HttpDownloadExecutor,
        settings_store::SettingsStore,
    },
    presentation::{
        app_state::AppState,
        commands::{apply_detected_proxy, detect_proxy, update_settings},
    },
};
use tauri::Manager;

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("open-download-manager-proxy-import-test-")
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

fn create_test_app(
    data_dir: &Path,
    mock_proxy: Option<Arc<MockProxyDiscoveryAdapter>>,
) -> tauri::App<tauri::test::MockRuntime> {
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
        Some(data_dir.join("Downloads")),
        DummyShell,
    );

    let file_icons = open_download_manager_lib::infrastructure::file_icons::FileIconProvider::new(
        data_dir.join("FileTypeIcons"),
    );

    let update_client = Arc::new(
        open_download_manager_lib::infrastructure::updater::GitHubReleaseClient::default_client(),
    );
    let update_service =
        open_download_manager_lib::application::updater_service::UpdateService::new(
            update_client,
            "0.1.2",
            true,
        );

    let cred_store = std::sync::Arc::new(
        open_download_manager_lib::infrastructure::credentials::CredentialStore::new(data_dir),
    );
    let cred_service = std::sync::Arc::new(
        open_download_manager_lib::application::credential_service::CredentialService::new(
            cred_store,
        ),
    );

    let mut app_state = AppState::new(
        settings,
        history,
        transfer,
        cred_service,
        integration,
        file_icons,
        update_service,
    );

    if let Some(mock) = mock_proxy {
        app_state = app_state.with_proxy_discovery(Arc::new(ProxyDiscoveryService::new(mock)));
    }

    app.manage(app_state);
    app
}

#[tokio::test]
async fn human_scenario_no_proxy_detected_reports_direct_mode() {
    let data_dir = temporary_directory();
    let mock = Arc::new(MockProxyDiscoveryAdapter::new());
    mock.set_result(DetectedProxyConfig::direct(ProxySource::System));

    let app = create_test_app(&data_dir, Some(mock));
    let detected = detect_proxy(app.state::<AppState>()).expect("should detect proxy");

    assert_eq!(detected.mode, DetectedProxyMode::Direct);
    assert!(detected.routes.is_empty());
    assert!(detected.pac_url.is_none());
    assert!(detected.can_import);
    assert!(!detected.credentials_available);
    assert!(detected.unsupported_fields.is_empty());

    // Applying direct sets proxy to direct / disabled
    let updated =
        apply_detected_proxy(app.state::<AppState>(), detected).expect("should apply direct proxy");
    assert_eq!(updated.proxy.protocol, ProxyProtocol::Direct);
    assert!(!updated.proxy.enabled);

    let _ = std::fs::remove_dir_all(&data_dir);
}

#[tokio::test]
async fn human_scenario_manual_http_proxy_discovery_and_transactional_apply() {
    let data_dir = temporary_directory();
    let mock = Arc::new(MockProxyDiscoveryAdapter::new());

    let routes = vec![ProtocolRoute {
        protocol: "http".to_string(),
        host: "10.0.0.1".to_string(),
        port: 8080,
        supported: true,
        reason: None,
    }];
    mock.set_result(DetectedProxyConfig::manual(
        ProxySource::System,
        routes,
        false,
        Vec::new(),
        None,
    ));

    let app = create_test_app(&data_dir, Some(mock));
    let detected = detect_proxy(app.state::<AppState>()).expect("should detect proxy");

    assert_eq!(detected.mode, DetectedProxyMode::Manual);
    assert!(detected.can_import);
    let import_set = detected.import_settings.as_ref().unwrap();
    assert_eq!(import_set.host, "10.0.0.1");
    assert_eq!(import_set.port, 8080);
    assert!(import_set.enabled);

    // Apply detected proxy transactionally
    let updated = apply_detected_proxy(app.state::<AppState>(), detected)
        .expect("should apply detected manual proxy");
    assert!(updated.proxy.enabled);
    assert_eq!(updated.proxy.protocol, ProxyProtocol::Http);
    assert_eq!(updated.proxy.host, "10.0.0.1");
    assert_eq!(updated.proxy.port, 8080);

    // Verify persisted settings on disk match
    let disk_store = SettingsStore::new(data_dir.clone());
    let reloaded = disk_store
        .load(data_dir.clone())
        .expect("should load settings from disk");
    assert_eq!(reloaded.proxy.host, "10.0.0.1");
    assert_eq!(reloaded.proxy.port, 8080);

    let _ = std::fs::remove_dir_all(&data_dir);
}

#[tokio::test]
async fn human_scenario_pac_url_detected_reports_unsupported_pac_without_silent_direct() {
    let data_dir = temporary_directory();
    let mock = Arc::new(MockProxyDiscoveryAdapter::new());

    let pac_url = "http://wpad.corp.internal/wpad.dat".to_string();
    mock.set_result(DetectedProxyConfig::pac(ProxySource::System, pac_url.clone()));

    let app = create_test_app(&data_dir, Some(mock));
    let detected = detect_proxy(app.state::<AppState>()).expect("should detect proxy");

    assert_eq!(detected.mode, DetectedProxyMode::Pac);
    assert_eq!(detected.pac_url, Some(pac_url));
    assert!(!detected.can_import);
    assert!(detected.import_settings.is_none());
    assert!(!detected.unsupported_fields.is_empty());
    assert!(detected.unsupported_fields[0].contains("PAC script auto-routing is unavailable"));

    // Attempting to apply PAC proxy must fail transactionally with clear error
    let apply_res = apply_detected_proxy(app.state::<AppState>(), detected);
    assert!(
        apply_res.is_err(),
        "Applying unsupported PAC config must fail"
    );

    // Verify active proxy is unchanged (not silently fallen back to direct or modified)
    let disk_store = SettingsStore::new(data_dir.clone());
    let reloaded = disk_store
        .load(data_dir.clone())
        .expect("should reload settings");
    assert!(!reloaded.proxy.enabled);

    let _ = std::fs::remove_dir_all(&data_dir);
}

#[tokio::test]
async fn human_scenario_authentication_detected_never_exposes_secret() {
    let data_dir = temporary_directory();
    let mock = Arc::new(MockProxyDiscoveryAdapter::new());

    let routes = vec![ProtocolRoute {
        protocol: "http".to_string(),
        host: "secure-proxy.internal".to_string(),
        port: 3128,
        supported: true,
        reason: None,
    }];
    mock.set_result(DetectedProxyConfig::manual(
        ProxySource::Environment,
        routes,
        true, // credentials present in environment
        Vec::new(),
        Some("service_account".to_string()),
    ));

    let app = create_test_app(&data_dir, Some(mock));
    let detected = detect_proxy(app.state::<AppState>()).expect("should detect proxy");

    assert!(detected.credentials_available);
    let import_set = detected.import_settings.as_ref().unwrap();
    assert_eq!(import_set.username, Some("service_account".to_string()));
    assert!(
        import_set.password.is_none(),
        "Detected proxy config must never expose password"
    );

    // Test JSON serialization: confirm secret is never leaked in JSON sent to frontend
    let json_str = serde_json::to_string(&detected).expect("should serialize to JSON");
    assert!(json_str.contains("\"credentialsAvailable\":true"));
    assert!(!json_str.contains("secret"));

    let _ = std::fs::remove_dir_all(&data_dir);
}

#[tokio::test]
async fn human_scenario_unsupported_protocol_routes_are_reported_individually() {
    let data_dir = temporary_directory();
    let mock = Arc::new(MockProxyDiscoveryAdapter::new());

    let routes = vec![
        ProtocolRoute {
            protocol: "http".to_string(),
            host: "proxy.corp".to_string(),
            port: 8080,
            supported: true,
            reason: None,
        },
        ProtocolRoute {
            protocol: "ftp".to_string(),
            host: "proxy.corp".to_string(),
            port: 2121,
            supported: false,
            reason: Some("FTP proxy routing is not supported for HTTP/HTTPS transfers.".to_string()),
        },
    ];

    mock.set_result(DetectedProxyConfig::manual(
        ProxySource::System,
        routes,
        false,
        Vec::new(),
        None,
    ));

    let app = create_test_app(&data_dir, Some(mock));
    let detected = detect_proxy(app.state::<AppState>()).expect("should detect proxy");

    assert_eq!(detected.routes.len(), 2);
    let ftp_route = detected
        .routes
        .iter()
        .find(|r| r.protocol == "ftp")
        .expect("ftp route exists");
    assert!(!ftp_route.supported);
    assert!(ftp_route.reason.is_some());
    assert!(detected.unsupported_fields.iter().any(|f| f.contains("FTP")));

    let _ = std::fs::remove_dir_all(&data_dir);
}

#[tokio::test]
async fn human_scenario_cancellation_preserves_active_proxy() {
    let data_dir = temporary_directory();
    let mock = Arc::new(MockProxyDiscoveryAdapter::new());

    let initial_proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Http,
        host: "existing-proxy.internal".to_string(),
        port: 8080,
        username: None,
        password: None,
        resolve_dns_remotely: false,
    };

    let app = create_test_app(&data_dir, Some(mock.clone()));
    // Set initial active proxy
    update_settings(
        app.state::<AppState>(),
        SettingsPatch {
            proxy: Some(initial_proxy.clone()),
            ..Default::default()
        },
    )
    .expect("initial proxy should apply");

    // Configure mock with a different discovered proxy
    let routes = vec![ProtocolRoute {
        protocol: "http".to_string(),
        host: "new-discovered-proxy.internal".to_string(),
        port: 3128,
        supported: true,
        reason: None,
    }];
    mock.set_result(DetectedProxyConfig::manual(
        ProxySource::System,
        routes,
        false,
        Vec::new(),
        None,
    ));

    // User triggers detection
    let detected = detect_proxy(app.state::<AppState>()).expect("proxy detected");
    assert_eq!(detected.import_settings.as_ref().unwrap().port, 3128);

    // User cancels dialog -> apply_detected_proxy is NEVER called.
    // Verify initial proxy remains active and untouched in disk storage and state!
    let disk_store = SettingsStore::new(data_dir.clone());
    let current = disk_store
        .load(data_dir.clone())
        .expect("should reload settings");
    assert_eq!(current.proxy.host, "existing-proxy.internal");
    assert_eq!(current.proxy.port, 8080);

    let _ = std::fs::remove_dir_all(&data_dir);
}

#[tokio::test]
async fn human_scenario_reconfiguration_failure_preserves_last_working_proxy() {
    let data_dir = temporary_directory();
    let mock = Arc::new(MockProxyDiscoveryAdapter::new());

    let working_proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Http,
        host: "working-proxy.internal".to_string(),
        port: 8080,
        username: None,
        password: None,
        resolve_dns_remotely: false,
    };

    let app = create_test_app(&data_dir, Some(mock.clone()));
    update_settings(
        app.state::<AppState>(),
        SettingsPatch {
            proxy: Some(working_proxy.clone()),
            ..Default::default()
        },
    )
    .expect("working proxy should apply");

    // Construct an invalid detected config that fails validation (invalid host with spaces)
    let invalid_routes = vec![ProtocolRoute {
        protocol: "http".to_string(),
        host: "invalid host with spaces".to_string(),
        port: 8080,
        supported: true,
        reason: None,
    }];
    let invalid_config = DetectedProxyConfig::manual(
        ProxySource::System,
        invalid_routes,
        false,
        Vec::new(),
        None,
    );

    let result = apply_detected_proxy(app.state::<AppState>(), invalid_config);
    assert!(
        result.is_err(),
        "Applying invalid proxy configuration must fail"
    );

    // Verify last working proxy was preserved intact
    let disk_store = SettingsStore::new(data_dir.clone());
    let current = disk_store
        .load(data_dir.clone())
        .expect("should reload settings");
    assert_eq!(current.proxy.host, "working-proxy.internal");
    assert_eq!(current.proxy.port, 8080);
    assert!(current.proxy.enabled);

    let _ = std::fs::remove_dir_all(&data_dir);
}
