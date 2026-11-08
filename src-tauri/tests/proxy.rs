// Created: 2026-11-06
// Reason: feat(network): implement HTTP and HTTPS proxy support with reachability test (fixes #11)
// Last updated: 2026-11-08
// Reason: refactor(architecture): decouple proxy domain model from reqwest and encapsulate settings update orchestration
// Reason: feat(proxy): add SOCKS5 proxy support and protocol-specific proxy settings (Closes #34)
// Description: Integration tests verifying proxy functionality, edge cases, and error recovery.

use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use base64::Engine;
use open_download_manager_lib::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        settings_service::{SettingsRepository, SettingsService},
    },
    domain::{
        download::{DownloadRequest, DownloadStatus},
        settings::{ProxyProtocol, ProxySettings, Settings, SettingsPatch},
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        http_download_executor::{create_http_client, HttpDownloadExecutor, ReqwestProxyExt},
        settings_store::SettingsStore,
    },
    presentation::commands::test_proxy_connection,
};

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("open-download-manager-proxy-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

#[test]
fn proxy_settings_validation_and_host_cleaning() {
    let mut proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Http,
        host: "  http://proxy.internal.corp/  ".to_string(),
        port: 8080,
        username: Some(" user ".to_string()),
        password: Some("secret".to_string()),
        resolve_dns_remotely: false,
    };

    assert_eq!(proxy.clean_host(), "proxy.internal.corp");
    assert!(proxy.validate().is_ok());

    let req_proxy = proxy.to_reqwest_proxy();
    assert!(req_proxy.is_ok(), "Reqwest proxy should construct cleanly");

    // Port 0 is rejected
    proxy.port = 0;
    assert!(proxy.validate().is_err());

    // Empty host when enabled is rejected
    proxy.port = 8080;
    proxy.host = "   ".to_string();
    assert!(proxy.validate().is_err());

    // Host with spaces inside is rejected
    proxy.host = "bad host name".to_string();
    assert!(proxy.validate().is_err());

    // When disabled, empty host is allowed
    proxy.enabled = false;
    proxy.host = "".to_string();
    assert!(proxy.validate().is_ok());
}

#[test]
fn proxy_settings_persistence_and_update() {
    let directory = temporary_directory();
    let default_dir = directory.join("downloads");
    let mut service = SettingsService::new(SettingsStore::new(&directory), default_dir.clone())
        .expect("settings should load");

    // Initially disabled
    assert!(!service.settings().proxy.enabled);
    assert_eq!(service.settings().proxy.port, 8080);

    // Update with proxy configuration
    let proxy_config = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Https,
        host: "10.0.0.1".to_string(),
        port: 3128,
        username: Some("admin".to_string()),
        password: Some("pass123".to_string()),
        resolve_dns_remotely: false,
    };

    let updated = service
        .update(SettingsPatch {
            proxy: Some(proxy_config.clone()),
            ..Default::default()
        })
        .expect("settings with proxy should update");

    assert_eq!(updated.proxy, proxy_config);

    // Reload from store to verify atomic persistence
    let reloaded = SettingsStore::new(&directory)
        .load(default_dir)
        .expect("persisted settings should reload");
    assert_eq!(reloaded.proxy, proxy_config);

    fs::remove_dir_all(directory).expect("clean temp dir");
}

#[tokio::test]
async fn test_proxy_connection_success_with_loopback_server() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let port = addr.port();

    let hit = Arc::new(AtomicBool::new(false));
    let hit_clone = hit.clone();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            hit_clone.store(true, Ordering::SeqCst);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        }
    });

    let proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Http,
        host: "127.0.0.1".to_string(),
        port,
        username: None,
        password: None,
        resolve_dns_remotely: false,
    };

    let test_url = Some(format!("http://127.0.0.1:{port}/probe"));
    let result = test_proxy_connection(proxy, test_url)
        .await
        .expect("command should succeed");

    assert!(result.success, "Result should be successful: {:?}", result);
    assert!(
        result.message.contains("200"),
        "Message should report HTTP 200: {}",
        result.message
    );
    assert!(hit.load(Ordering::SeqCst), "Proxy was contacted");
}

#[tokio::test]
async fn test_proxy_connection_with_authentication_check() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let port = addr.port();

    let expected_auth = base64::engine::general_purpose::STANDARD.encode("validuser:validpass");
    let expected_header = format!("proxy-authorization: Basic {expected_auth}");

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let read_bytes = stream.read(&mut buf).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..read_bytes]).to_lowercase();

            if req_str.contains(&expected_header.to_lowercase()) {
                let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
            } else {
                let _ = stream.write_all(
                    b"HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=\"test\"\r\nContent-Length: 0\r\n\r\n",
                );
            }
        }
    });

    // 1. Test with invalid credentials
    let bad_proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Http,
        host: "127.0.0.1".to_string(),
        port,
        username: Some("wronguser".to_string()),
        password: Some("wrongpass".to_string()),
        resolve_dns_remotely: false,
    };

    let test_url = Some(format!("http://127.0.0.1:{port}/probe"));
    let bad_result = test_proxy_connection(bad_proxy, test_url.clone())
        .await
        .expect("command should return result");

    assert!(!bad_result.success);
    assert!(
        bad_result.message.contains("407") || bad_result.message.contains("authentication"),
        "Should mention 407 authentication required: {}",
        bad_result.message
    );

    // 2. Test with valid credentials
    let good_proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Http,
        host: "127.0.0.1".to_string(),
        port,
        username: Some("validuser".to_string()),
        password: Some("validpass".to_string()),
        resolve_dns_remotely: false,
    };

    let good_result = test_proxy_connection(good_proxy, test_url)
        .await
        .expect("command should return result");

    assert!(good_result.success, "Good credentials must succeed: {:?}", good_result);
}

#[tokio::test]
async fn test_proxy_connection_unreachable_fails_gracefully() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind temp port");
    let port = listener.local_addr().unwrap().port();
    drop(listener); // Close immediately so connection is refused

    let proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Http,
        host: "127.0.0.1".to_string(),
        port,
        username: None,
        password: None,
        resolve_dns_remotely: false,
    };

    let test_url = Some(format!("http://127.0.0.1:{port}/probe"));
    let result = test_proxy_connection(proxy, test_url)
        .await
        .expect("command should not crash");

    assert!(!result.success);
    assert!(
        result.message.to_lowercase().contains("connect")
            || result.message.to_lowercase().contains("refused")
            || result.message.to_lowercase().contains("failed"),
        "Message should report connection error: {}",
        result.message
    );
}

#[tokio::test]
async fn download_transfer_routes_through_configured_proxy() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind proxy");
    let proxy_addr = listener.local_addr().expect("proxy addr");
    let proxy_port = proxy_addr.port();

    let proxy_received_request = Arc::new(AtomicBool::new(false));
    let proxy_received_clone = proxy_received_request.clone();

    let payload = b"Hello from proxy transferred download!";

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let read_bytes = stream.read(&mut buf).unwrap_or(0);
            if read_bytes > 0 {
                proxy_received_clone.store(true, Ordering::SeqCst);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(payload);
                let _ = stream.flush();
            }
        }
    });

    let proxy_settings = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Http,
        host: "127.0.0.1".to_string(),
        port: proxy_port,
        username: None,
        password: None,
        resolve_dns_remotely: false,
    };

    let client = create_http_client(Some(&proxy_settings)).expect("client with proxy");
    let executor = HttpDownloadExecutor::with_client(client);

    let temp = temporary_directory();
    let history_store = DownloadHistoryStore::new(temp.clone());
    let history = DownloadHistoryService::new(history_store).expect("history service");
    let transfer = DownloadTransferService::new(history.clone(), executor, 2);

    let mut app_settings = Settings::new(temp.clone());
    app_settings.connections = 1;
    app_settings.proxy = proxy_settings;

    let target_url = format!("http://example.internal/file.bin");
    let ids = transfer
        .add(
            DownloadRequest {
                urls: vec![target_url],
                filename: Some("proxy_file.bin".to_string()),
                directory: Some(temp.to_string_lossy().to_string()),
                queue_only: false,
                review_only: None,
                ..Default::default()
            },
            &app_settings,
        )
        .expect("add download");

    let download_id = &ids[0];

    // Wait for download completion
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Ok(downloads) = history.downloads() {
            if let Some(rec) = downloads.into_iter().find(|d| &d.id == download_id) {
                if rec.status == DownloadStatus::Completed {
                    break;
                }
            }
        }
    }

    let downloads = history.downloads().expect("history exists");
    let record = downloads
        .into_iter()
        .find(|d| &d.id == download_id)
        .expect("record exists");
    assert_eq!(record.status, DownloadStatus::Completed);
    assert_eq!(record.downloaded, payload.len() as u64);

    let file_path = temp.join("proxy_file.bin");
    assert!(file_path.exists(), "Downloaded file must exist on disk");
    let content = fs::read(&file_path).expect("read downloaded file");
    assert_eq!(content, payload);

    assert!(
        proxy_received_request.load(Ordering::SeqCst),
        "Proxy server must have received the HTTP transfer connection"
    );

    let _ = fs::remove_dir_all(temp);
}
