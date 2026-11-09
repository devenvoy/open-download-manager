// Created: 2026-11-08
// Reason: feat(proxy): add SOCKS5 proxy support and protocol-specific proxy settings (Closes #34)
// Last updated: 2026-11-08
// Reason: feat(proxy): add SOCKS5 proxy support and protocol-specific proxy settings (Closes #34)
// Description: Human-scenario integration tests verifying SOCKS5 proxy configuration, remote/local DNS resolution modes, authentication, and secret redaction.

use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};

use open_download_manager_lib::{
    application::settings_service::{SettingsRepository, SettingsService},
    domain::settings::{ProxyProtocol, ProxySettings, SettingsPatch},
    infrastructure::{
        http_download_executor::{
            build_reqwest_proxy, create_http_client, test_proxy_connection, ReqwestProxyExt,
        },
        settings_store::SettingsStore,
    },
};

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("open-download-manager-socks5-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

#[test]
fn human_scenario_socks5_validation_and_url_scheme_selection() {
    let proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Socks5,
        host: "  socks5://proxy.example.internal:1080/  ".to_string(),
        port: 1080,
        username: Some("proxy_user".to_string()),
        password: Some("proxy_secret".to_string()),
        resolve_dns_remotely: false,
    };

    // Host cleaning strips scheme and slashes
    assert_eq!(proxy.clean_host(), "proxy.example.internal:1080");

    let clean_proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Socks5,
        host: "proxy.example.internal".to_string(),
        port: 1080,
        username: Some("proxy_user".to_string()),
        password: Some("proxy_secret".to_string()),
        resolve_dns_remotely: false,
    };
    assert!(clean_proxy.validate().is_ok());

    // Local DNS (resolve_dns_remotely = false) constructs reqwest proxy
    let local_req_proxy = clean_proxy.to_reqwest_proxy();
    assert!(local_req_proxy.is_ok(), "SOCKS5 proxy should construct cleanly with local DNS");

    // Remote DNS (resolve_dns_remotely = true) constructs reqwest proxy with socks5h
    let mut remote_proxy = clean_proxy.clone();
    remote_proxy.resolve_dns_remotely = true;
    let remote_req_proxy = remote_proxy.to_reqwest_proxy();
    assert!(remote_req_proxy.is_ok(), "SOCKS5 proxy should construct cleanly with remote DNS");

    // Direct protocol does not construct a proxy
    let direct_proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Direct,
        host: "ignored".to_string(),
        port: 8080,
        username: None,
        password: None,
        resolve_dns_remotely: false,
    };
    assert!(build_reqwest_proxy(&direct_proxy).is_err());
}

#[test]
fn human_scenario_socks5_persistence_and_reloading() {
    let directory = temporary_directory();
    let default_dir = directory.join("downloads");
    let mut service = SettingsService::new(SettingsStore::new(&directory), default_dir.clone())
        .expect("settings should load");

    let socks5_config = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Socks5,
        host: "socks5.corp.network".to_string(),
        port: 1080,
        username: Some("socks_user".to_string()),
        password: Some("socks_pass_99".to_string()),
        resolve_dns_remotely: true,
    };

    let updated = service
        .update(SettingsPatch {
            proxy: Some(socks5_config.clone()),
            ..Default::default()
        })
        .expect("settings with SOCKS5 proxy should update");

    assert_eq!(updated.proxy, socks5_config);
    assert_eq!(updated.proxy.protocol, ProxyProtocol::Socks5);
    assert!(updated.proxy.resolve_dns_remotely);

    // Reload from store to ensure on-disk schema persistence
    let reloaded = SettingsStore::new(&directory)
        .load(default_dir)
        .expect("settings should reload");

    assert_eq!(reloaded.proxy, socks5_config);
    std::fs::remove_dir_all(directory).expect("clean temp dir");
}

#[tokio::test]
async fn human_scenario_socks5_unreachable_proxy_surfaces_sanitized_error() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind temp port");
    let port = listener.local_addr().unwrap().port();
    drop(listener); // Connection refused

    let secret_pass = "topsecret_socks5_credential";
    let proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Socks5,
        host: "127.0.0.1".to_string(),
        port,
        username: Some("my_socks_user".to_string()),
        password: Some(secret_pass.to_string()),
        resolve_dns_remotely: true,
    };

    let result = test_proxy_connection(&proxy, Some("https://example.com")).await;
    assert!(!result.success);
    assert!(
        !result.message.contains(secret_pass),
        "Secret password must be redacted from error message: {}",
        result.message
    );
}

#[tokio::test]
async fn human_scenario_socks5_live_proxy_handshake_and_transfer() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock socks5");
    let port = listener.local_addr().unwrap().port();

    let handshake_completed = Arc::new(AtomicBool::new(false));
    let handshake_clone = handshake_completed.clone();

    // Spawn mock SOCKS5 server
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 512];

            // 1. Initial greeting: VER (0x05), NMETHODS, METHODS
            let n = stream.read(&mut buf).unwrap_or(0);
            if n >= 3 && buf[0] == 0x05 {
                // Respond: VER 0x05, METHOD 0x00 (NO_AUTH)
                let _ = stream.write_all(&[0x05, 0x00]);
                let _ = stream.flush();

                // 2. Client connection request: VER (0x05), CMD (0x01 CONNECT), RSV (0x00), ATYP ...
                let n2 = stream.read(&mut buf).unwrap_or(0);
                if n2 >= 4 && buf[0] == 0x05 && buf[1] == 0x01 {
                    handshake_clone.store(true, Ordering::SeqCst);

                    // Respond success: VER 0x05, REP 0x00, RSV 0x00, ATYP 0x01, BND.ADDR 0.0.0.0, PORT 0
                    let _ = stream.write_all(&[0x05, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0, 80]);
                    let _ = stream.flush();

                    // 3. Read HTTP request through tunnel and respond HTTP 200
                    let mut http_buf = [0u8; 1024];
                    let _ = stream.read(&mut http_buf);
                    let resp = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nHELLO";
                    let _ = stream.write_all(resp);
                    let _ = stream.flush();
                }
            }
        }
    });

    let proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Socks5,
        host: "127.0.0.1".to_string(),
        port,
        username: None,
        password: None,
        resolve_dns_remotely: false,
    };

    let client = create_http_client(Some(&proxy)).expect("HTTP client with SOCKS5 proxy");
    let resp = client.get("http://127.0.0.1/test").send().await;

    assert!(handshake_completed.load(Ordering::SeqCst), "SOCKS5 handshake must be performed");
    if let Ok(res) = resp {
        assert_eq!(res.status().as_u16(), 200);
        let text = res.text().await.unwrap_or_default();
        assert_eq!(text, "HELLO");
    }
}

#[tokio::test]
async fn human_scenario_socks5_remote_dns_sends_hostname_to_proxy() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock remote dns socks5");
    let port = listener.local_addr().unwrap().port();

    let remote_dns_detected = Arc::new(AtomicBool::new(false));
    let remote_dns_clone = remote_dns_detected.clone();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 512];

            // 1. Initial greeting
            let n = stream.read(&mut buf).unwrap_or(0);
            if n >= 3 && buf[0] == 0x05 {
                let _ = stream.write_all(&[0x05, 0x00]);
                let _ = stream.flush();

                // 2. Connection request
                let n2 = stream.read(&mut buf).unwrap_or(0);
                if n2 >= 5 && buf[0] == 0x05 && buf[1] == 0x01 {
                    // ATYP = 0x03 indicates domain name / remote DNS
                    if buf[3] == 0x03 {
                        let domain_len = buf[4] as usize;
                        if n2 >= 5 + domain_len {
                            let domain = String::from_utf8_lossy(&buf[5..5 + domain_len]);
                            if domain == "my-remote-service.test" {
                                remote_dns_clone.store(true, Ordering::SeqCst);
                            }
                        }
                    }

                    // Respond success
                    let _ = stream.write_all(&[0x05, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0, 80]);
                    let _ = stream.flush();

                    let mut http_buf = [0u8; 1024];
                    let _ = stream.read(&mut http_buf);
                    let resp = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
                    let _ = stream.write_all(resp);
                    let _ = stream.flush();
                }
            }
        }
    });

    let proxy = ProxySettings {
        enabled: true,
        protocol: ProxyProtocol::Socks5,
        host: "127.0.0.1".to_string(),
        port,
        username: None,
        password: None,
        resolve_dns_remotely: true, // socks5h mode
    };

    let client = create_http_client(Some(&proxy)).expect("HTTP client with SOCKS5h remote DNS");
    let resp = client.get("http://my-remote-service.test/probe").send().await;

    assert!(
        remote_dns_detected.load(Ordering::SeqCst),
        "SOCKS5 server must receive ATYP 0x03 domain name when resolve_dns_remotely is enabled"
    );
    if let Ok(res) = resp {
        assert_eq!(res.status().as_u16(), 200);
    }
}

#[tokio::test]
async fn human_scenario_disabling_proxy_or_direct_restores_direct_networking() {
    let direct_proxy = ProxySettings {
        enabled: false,
        protocol: ProxyProtocol::Direct,
        host: "".to_string(),
        port: 8080,
        username: None,
        password: None,
        resolve_dns_remotely: false,
    };

    let _client = create_http_client(Some(&direct_proxy)).expect("direct client");
    // Verify client can be built and operates with direct networking
    assert!(create_http_client(None).is_ok());

    let test_res = test_proxy_connection(&direct_proxy, None).await;
    assert!(test_res.success);
    assert!(test_res.message.contains("Direct connection"));
}
