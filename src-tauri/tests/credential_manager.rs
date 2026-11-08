// Created: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Description: Integration tests verifying credential manager functionality, edge cases, and error recovery.

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
        credential_service::CredentialService, download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
    },
    domain::{
        credentials::CredentialRule,
        download::{DownloadCredentials, DownloadRequest, DownloadStatus},
        settings::Settings,
    },
    infrastructure::{
        credentials::CredentialStore,
        download_history_store::DownloadHistoryStore,
        ftp::FtpDownloadExecutor,
        http_download_executor::HttpDownloadExecutor,
        media::CompositeDownloadExecutor,
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    net::TcpListener as TokioTcpListener,
};

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("open-download-manager-cred-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

async fn wait_for_terminal_status(
    history: &DownloadHistoryService,
    id: &str,
) -> DownloadStatus {
    for _ in 0..100 {
        if let Ok(record) = history.download(id) {
            if matches!(
                record.status,
                DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
            ) {
                return record.status;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    history.download(id).map(|r| r.status).unwrap_or(DownloadStatus::Failed)
}

// -------------------------------------------------------------------------
// Scenario 1: User downloads protected file with matching stored credentials
// -------------------------------------------------------------------------
#[tokio::test]
async fn human_scenario_authenticated_http_download_with_stored_rule() {
    let test_payload = b"RESTRICTED_PAYLOAD_CONFIDENTIAL_12345";
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let port = listener.local_addr().expect("port").port();

    let auth_received = Arc::new(AtomicBool::new(false));
    let auth_received_clone = auth_received.clone();

    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.expect("incoming stream");
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            // Expected Basic Auth: alice:secret123 -> YWxpY2U6c2VjcmV0MTIz
            let expected_auth = base64::engine::general_purpose::STANDARD.encode("alice:secret123");
            let expected_header = format!("Basic {expected_auth}");

            if req_str.contains(&format!("authorization: {expected_header}"))
                || req_str.contains(&format!("Authorization: {expected_header}"))
            {
                auth_received_clone.store(true, Ordering::SeqCst);
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/pdf\r\nConnection: close\r\n\r\n",
                    test_payload.len()
                );
                let _ = stream.write_all(test_payload);
            } else {
                let _ = write!(
                    stream,
                    "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"Internal\"\r\nContent-Length: 12\r\nConnection: close\r\n\r\nUnauthorized"
                );
            }
            let _ = stream.flush();
        }
    });

    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let cred_store = Arc::new(CredentialStore::new(&dir));
    let cred_service = CredentialService::new(cred_store.clone());

    // User stores credential rule for this host and path prefix
    let rule = CredentialRule::new(
        "rule-1",
        "http",
        "127.0.0.1",
        Some(port),
        "/internal/",
        "alice",
        "secret123",
    )
    .unwrap();
    cred_service.save(rule).unwrap();

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let http_executor = HttpDownloadExecutor::new().unwrap().with_credentials(cred_store.clone());
    let composite = CompositeDownloadExecutor::with_shared(
        http_executor,
        Arc::new(std::sync::RwLock::new(reqwest::Client::new())),
    )
    .with_credentials(cred_store);

    let service = DownloadTransferService::new(history.clone(), composite, 2);
    let settings = Settings::new(dir.clone());

    // Add download with clean URL without credentials
    let url = format!("http://127.0.0.1:{port}/internal/annual-report.pdf");
    let req = DownloadRequest {
        urls: vec![url],
        filename: Some("annual-report.pdf".to_owned()),
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

    let ids = service.add(req, &settings).unwrap();
    let status = wait_for_terminal_status(&history, &ids[0]).await;

    assert_eq!(status, DownloadStatus::Completed, "Download should complete with valid credentials");
    assert!(auth_received.load(Ordering::SeqCst), "Server should have received basic auth header");

    let downloaded_file = dir.join("annual-report.pdf");
    assert_eq!(fs::read(downloaded_file).unwrap(), test_payload);

    // Verify secret is NEVER persisted to downloads-tauri.json
    let history_file = dir.join("downloads-tauri.json");
    if history_file.exists() {
        let history_json = fs::read_to_string(history_file).unwrap();
        assert!(!history_json.contains("secret123"), "Secret must never be written to history file");
    }

    let _ = fs::remove_dir_all(dir);
}

// -------------------------------------------------------------------------
// Scenario 2: Strict path boundary enforcement prevents credential leakage
// -------------------------------------------------------------------------
#[tokio::test]
async fn human_scenario_path_boundary_enforcement_prevents_credential_leak() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let port = listener.local_addr().expect("port").port();

    let leaked_auth = Arc::new(AtomicBool::new(false));
    let leaked_auth_clone = leaked_auth.clone();

    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.expect("incoming stream");
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            // If request is for /team-private/ and has Authorization, record leakage
            if req_str.contains("/team-private/") && req_str.to_lowercase().contains("authorization: basic") {
                leaked_auth_clone.store(true, Ordering::SeqCst);
            }

            // Always reject unauthenticated requests
            let _ = write!(
                stream,
                "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"Private\"\r\nContent-Length: 12\r\nConnection: close\r\n\r\nUnauthorized"
            );
            let _ = stream.flush();
        }
    });

    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let cred_store = Arc::new(CredentialStore::new(&dir));
    let cred_service = CredentialService::new(cred_store.clone());

    // Rule is strictly for `/team/`
    let rule = CredentialRule::new(
        "rule-team",
        "http",
        "127.0.0.1",
        Some(port),
        "/team/",
        "team_lead",
        "super_secret_team_pass",
    )
    .unwrap();
    cred_service.save(rule).unwrap();

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let http_executor = HttpDownloadExecutor::new().unwrap().with_credentials(cred_store.clone());
    let service = DownloadTransferService::new(history.clone(), http_executor, 2);
    let settings = Settings::new(dir.clone());

    // Candidate URL is `/team-private/doc.pdf`. It starts with "/team" as string prefix,
    // but NOT as segment boundary "/team/".
    let url = format!("http://127.0.0.1:{port}/team-private/doc.pdf");
    let req = DownloadRequest {
        urls: vec![url],
        filename: Some("doc.pdf".to_owned()),
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

    let ids = service.add(req, &settings).unwrap();
    let status = wait_for_terminal_status(&history, &ids[0]).await;

    assert_eq!(status, DownloadStatus::Failed, "Download must fail with 401");
    assert!(!leaked_auth.load(Ordering::SeqCst), "Credentials must NOT leak to /team-private/");

    let record = history.download(&ids[0]).unwrap();
    assert!(
        record.error.as_deref().unwrap_or("").contains("Authentication required")
            || record.error.as_deref().unwrap_or("").contains("401"),
        "Error message should mention authentication requirement"
    );

    let _ = fs::remove_dir_all(dir);
}

// -------------------------------------------------------------------------
// Scenario 3: Cross-domain HTTP 302 redirect strips Authorization header
// -------------------------------------------------------------------------
#[tokio::test]
async fn human_scenario_redirect_cross_domain_strips_authorization_header() {
    let test_data = b"UNPROTECTED_REDIRECTED_PAYLOAD_SUCCESS";

    // Server B (Destination server - different port)
    let listener_b = TcpListener::bind("127.0.0.1:0").expect("bind listener B");
    let port_b = listener_b.local_addr().expect("port B").port();
    let leaked_auth_to_b = Arc::new(AtomicBool::new(false));
    let leaked_auth_to_b_clone = leaked_auth_to_b.clone();

    thread::spawn(move || {
        for stream in listener_b.incoming() {
            let mut stream = stream.expect("incoming B");
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            if req_str.to_lowercase().contains("authorization:") {
                leaked_auth_to_b_clone.store(true, Ordering::SeqCst);
            }

            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                test_data.len()
            );
            let _ = stream.write_all(test_data);
            let _ = stream.flush();
        }
    });

    // Server A (Origin server - requires auth, then redirects to Server B)
    let listener_a = TcpListener::bind("127.0.0.1:0").expect("bind listener A");
    let port_a = listener_a.local_addr().expect("port A").port();
    let auth_received_on_a = Arc::new(AtomicBool::new(false));
    let auth_received_on_a_clone = auth_received_on_a.clone();

    thread::spawn(move || {
        for stream in listener_a.incoming() {
            let mut stream = stream.expect("incoming A");
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            let expected_auth = base64::engine::general_purpose::STANDARD.encode("serverA_user:serverA_secret");
            if req_str.contains(&format!("Basic {expected_auth}")) {
                auth_received_on_a_clone.store(true, Ordering::SeqCst);
                // Return 302 Found pointing to Server B
                let _ = write!(
                    stream,
                    "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port_b}/public/file.bin\r\nConnection: close\r\n\r\n"
                );
            } else {
                let _ = write!(
                    stream,
                    "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic\r\nConnection: close\r\n\r\n"
                );
            }
            let _ = stream.flush();
        }
    });

    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let cred_store = Arc::new(CredentialStore::new(&dir));

    // Stored rule strictly for Server A
    let rule_a = CredentialRule::new(
        "rule-a",
        "http",
        "127.0.0.1",
        Some(port_a),
        "/protected/",
        "serverA_user",
        "serverA_secret",
    )
    .unwrap();
    cred_store.add_or_update(rule_a).unwrap();

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let http_executor = HttpDownloadExecutor::new().unwrap().with_credentials(cred_store);
    let service = DownloadTransferService::new(history.clone(), http_executor, 2);
    let settings = Settings::new(dir.clone());

    let url = format!("http://127.0.0.1:{port_a}/protected/start.bin");
    let req = DownloadRequest {
        urls: vec![url],
        filename: Some("redirect_result.bin".to_owned()),
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

    let ids = service.add(req, &settings).unwrap();
    let status = wait_for_terminal_status(&history, &ids[0]).await;

    assert_eq!(status, DownloadStatus::Completed);
    assert!(auth_received_on_a.load(Ordering::SeqCst), "Server A must receive credentials");
    assert!(!leaked_auth_to_b.load(Ordering::SeqCst), "Server B must NOT receive Server A's credentials");

    let downloaded = fs::read(dir.join("redirect_result.bin")).unwrap();
    assert_eq!(downloaded, test_data);

    let _ = fs::remove_dir_all(dir);
}

// -------------------------------------------------------------------------
// Scenario 4: Cross-domain redirect to a destination with its OWN stored rule
// -------------------------------------------------------------------------
#[tokio::test]
async fn human_scenario_redirect_to_destination_with_its_own_rule_attaches_new_credentials() {
    let final_data = b"DESTINATION_AUTHENTICATED_FILE_CONTENT";

    // Server B (Destination server - requires userB:secretB)
    let listener_b = TcpListener::bind("127.0.0.1:0").expect("bind listener B");
    let port_b = listener_b.local_addr().expect("port B").port();
    let auth_b_correct = Arc::new(AtomicBool::new(false));
    let auth_b_clone = auth_b_correct.clone();

    thread::spawn(move || {
        for stream in listener_b.incoming() {
            let mut stream = stream.expect("incoming B");
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            let expected_auth_b = base64::engine::general_purpose::STANDARD.encode("userB:secretB");
            if req_str.contains(&format!("Basic {expected_auth_b}")) {
                auth_b_clone.store(true, Ordering::SeqCst);
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    final_data.len()
                );
                let _ = stream.write_all(final_data);
            } else {
                let _ = write!(
                    stream,
                    "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic\r\nConnection: close\r\n\r\n"
                );
            }
            let _ = stream.flush();
        }
    });

    // Server A (Origin server - requires userA:secretA, redirects to Server B)
    let listener_a = TcpListener::bind("127.0.0.1:0").expect("bind listener A");
    let port_a = listener_a.local_addr().expect("port A").port();
    let auth_a_correct = Arc::new(AtomicBool::new(false));
    let auth_a_clone = auth_a_correct.clone();

    thread::spawn(move || {
        for stream in listener_a.incoming() {
            let mut stream = stream.expect("incoming A");
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            let expected_auth_a = base64::engine::general_purpose::STANDARD.encode("userA:secretA");
            if req_str.contains(&format!("Basic {expected_auth_a}")) {
                auth_a_clone.store(true, Ordering::SeqCst);
                let _ = write!(
                    stream,
                    "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port_b}/secure/archive.tar\r\nConnection: close\r\n\r\n"
                );
            } else {
                let _ = write!(
                    stream,
                    "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic\r\nConnection: close\r\n\r\n"
                );
            }
            let _ = stream.flush();
        }
    });

    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let cred_store = Arc::new(CredentialStore::new(&dir));

    // Stored rules for BOTH Server A and Server B
    let rule_a = CredentialRule::new("rule-a", "http", "127.0.0.1", Some(port_a), "/jump/", "userA", "secretA").unwrap();
    let rule_b = CredentialRule::new("rule-b", "http", "127.0.0.1", Some(port_b), "/secure/", "userB", "secretB").unwrap();
    cred_store.add_or_update(rule_a).unwrap();
    cred_store.add_or_update(rule_b).unwrap();

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let http_executor = HttpDownloadExecutor::new().unwrap().with_credentials(cred_store);
    let service = DownloadTransferService::new(history.clone(), http_executor, 2);
    let settings = Settings::new(dir.clone());

    let url = format!("http://127.0.0.1:{port_a}/jump/file.bin");
    let req = DownloadRequest {
        urls: vec![url],
        filename: Some("archive.tar".to_owned()),
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

    let ids = service.add(req, &settings).unwrap();
    let status = wait_for_terminal_status(&history, &ids[0]).await;

    assert_eq!(status, DownloadStatus::Completed);
    assert!(auth_a_correct.load(Ordering::SeqCst), "Server A must be authenticated with userA:secretA");
    assert!(auth_b_correct.load(Ordering::SeqCst), "Server B must be authenticated with userB:secretB");

    let downloaded = fs::read(dir.join("archive.tar")).unwrap();
    assert_eq!(downloaded, final_data);

    let _ = fs::remove_dir_all(dir);
}

// -------------------------------------------------------------------------
// Scenario 5: In-memory credential override takes precedence and is not persisted
// -------------------------------------------------------------------------
#[tokio::test]
async fn human_scenario_in_memory_override_takes_precedence_and_is_not_persisted() {
    let test_data = b"OVERRIDE_AUTH_SUCCESS_DATA";
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let port = listener.local_addr().expect("port").port();

    let override_matched = Arc::new(AtomicBool::new(false));
    let override_matched_clone = override_matched.clone();

    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.expect("incoming stream");
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            let expected_auth = base64::engine::general_purpose::STANDARD.encode("session_user:one_time_token");
            if req_str.contains(&format!("Basic {expected_auth}")) {
                override_matched_clone.store(true, Ordering::SeqCst);
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    test_data.len()
                );
                let _ = stream.write_all(test_data);
            } else {
                let _ = write!(
                    stream,
                    "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic\r\nConnection: close\r\n\r\n"
                );
            }
            let _ = stream.flush();
        }
    });

    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let cred_store = Arc::new(CredentialStore::new(&dir));

    // Stored persistent rule has DIFFERENT (e.g. outdated) credentials
    let old_rule = CredentialRule::new("rule-old", "http", "127.0.0.1", Some(port), "/", "old_user", "old_password").unwrap();
    cred_store.add_or_update(old_rule).unwrap();

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let http_executor = HttpDownloadExecutor::new().unwrap().with_credentials(cred_store);
    let service = DownloadTransferService::new(history.clone(), http_executor, 2);
    let settings = Settings::new(dir.clone());

    // User provides per-download in-memory credentials override
    let url = format!("http://127.0.0.1:{port}/file.bin");
    let req = DownloadRequest {
        urls: vec![url],
        filename: Some("override_test.bin".to_owned()),
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
        credentials: Some(DownloadCredentials {
            username: "session_user".to_owned(),
            password: Some("one_time_token".to_owned()),
        }),
        completion_actions: None,
    };

    let ids = service.add(req, &settings).unwrap();
    let status = wait_for_terminal_status(&history, &ids[0]).await;

    assert_eq!(status, DownloadStatus::Completed);
    assert!(override_matched.load(Ordering::SeqCst), "Override credentials must be used instead of persistent rule");

    // Verify secret is NOT persisted to downloads-tauri.json
    let history_file = dir.join("downloads-tauri.json");
    if history_file.exists() {
        let history_json = fs::read_to_string(history_file).unwrap();
        assert!(!history_json.contains("one_time_token"), "Override secret must never be written to history file");
    }

    let _ = fs::remove_dir_all(dir);
}

// -------------------------------------------------------------------------
// Scenario 6: Real FTP download using site credentials stored without credentials in URL
// -------------------------------------------------------------------------
#[tokio::test]
async fn human_scenario_ftp_download_with_stored_rule_authenticates_clean_url() {
    let ftp_payload = b"FTP_AUTHENTICATED_CORPORATE_DOCUMENT_CONTENT";
    let listener = TokioTcpListener::bind("127.0.0.1:0").await.expect("bind ftp listener");
    let ftp_port = listener.local_addr().expect("ftp port").port();

    let user_authenticated = Arc::new(AtomicBool::new(false));
    let pass_authenticated = Arc::new(AtomicBool::new(false));
    let user_flag = user_authenticated.clone();
    let pass_flag = pass_authenticated.clone();

    // Spawn real mock FTP server
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let u_flag = user_flag.clone();
            let p_flag = pass_flag.clone();
            tokio::spawn(async move {
                let _ = stream.write_all(b"220 OpenDownloadManager Test FTP Ready\r\n").await;
                let (reader, mut write_half) = stream.into_split();
                let mut buf_reader = tokio::io::BufReader::new(reader);
                let mut line = String::new();
                let mut pasv_listener: Option<TokioTcpListener> = None;

                while let Ok(n) = buf_reader.read_line(&mut line).await {
                    if n == 0 { break; }
                    let trimmed = line.trim();
                    let (cmd, arg) = match trimmed.split_once(' ') {
                        Some((c, a)) => (c.to_ascii_uppercase(), a.trim()),
                        None => (trimmed.to_ascii_uppercase(), ""),
                    };

                    match cmd.as_str() {
                        "USER" => {
                            if arg == "ftp_corp_user" {
                                u_flag.store(true, Ordering::SeqCst);
                                let _ = write_half.write_all(b"331 User name okay, need password\r\n").await;
                            } else {
                                let _ = write_half.write_all(b"530 Not logged in\r\n").await;
                            }
                        }
                        "PASS" => {
                            if arg == "ftp_corp_pass987" {
                                p_flag.store(true, Ordering::SeqCst);
                                let _ = write_half.write_all(b"230 User logged in, proceed\r\n").await;
                            } else {
                                let _ = write_half.write_all(b"530 Login incorrect\r\n").await;
                            }
                        }
                        "FEAT" => {
                            let _ = write_half.write_all(b"211-Features:\r\n SIZE\r\n REST STREAM\r\n211 End\r\n").await;
                        }
                        "TYPE" => {
                            let _ = write_half.write_all(b"200 Type set to I\r\n").await;
                        }
                        "SIZE" => {
                            let resp = format!("213 {}\r\n", ftp_payload.len());
                            let _ = write_half.write_all(resp.as_bytes()).await;
                        }
                        "MDTM" => {
                            let _ = write_half.write_all(b"213 20261108120000\r\n").await;
                        }
                        "PASV" => {
                            if let Ok(pasv) = TokioTcpListener::bind("127.0.0.1:0").await {
                                let p = pasv.local_addr().unwrap().port();
                                let resp = format!("227 Entering Passive Mode (127,0,0,1,{},{})\r\n", p >> 8, p & 0xFF);
                                pasv_listener = Some(pasv);
                                let _ = write_half.write_all(resp.as_bytes()).await;
                            } else {
                                let _ = write_half.write_all(b"425 Cannot open passive connection\r\n").await;
                            }
                        }
                        "RETR" => {
                            let _ = write_half.write_all(b"150 Opening BINARY mode data connection\r\n").await;
                            let _ = write_half.flush().await;

                            if let Some(listener) = pasv_listener.take() {
                                if let Ok((mut data_conn, _)) = listener.accept().await {
                                    let _ = data_conn.write_all(ftp_payload).await;
                                    let _ = data_conn.flush().await;
                                    drop(data_conn);
                                }
                            }
                            let _ = write_half.write_all(b"226 Transfer complete\r\n").await;
                        }
                        "QUIT" => {
                            let _ = write_half.write_all(b"221 Goodbye\r\n").await;
                            break;
                        }
                        _ => {
                            let _ = write_half.write_all(b"502 Command not implemented\r\n").await;
                        }
                    }
                    let _ = write_half.flush().await;
                    line.clear();
                }
            });
        }
    });

    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let cred_store = Arc::new(CredentialStore::new(&dir));

    // Stored rule for FTP server
    let rule = CredentialRule::new(
        "rule-ftp",
        "ftp",
        "127.0.0.1",
        Some(ftp_port),
        "/",
        "ftp_corp_user",
        "ftp_corp_pass987",
    )
    .unwrap();
    cred_store.add_or_update(rule).unwrap();

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let allocator = open_download_manager_lib::domain::download::DestinationAllocator::new();
    let ftp_executor = FtpDownloadExecutor::new(allocator).with_credentials(cred_store);
    let service = DownloadTransferService::new(history.clone(), ftp_executor, 1);
    let settings = Settings::new(dir.clone());

    // Clean FTP URL without inline username/password: ftp://127.0.0.1:port/corp/specs.pdf
    let url = format!("ftp://127.0.0.1:{ftp_port}/corp/specs.pdf");
    let req = DownloadRequest {
        urls: vec![url],
        filename: Some("specs.pdf".to_owned()),
        directory: Some(dir.to_string_lossy().into_owned()),
        temp_directory: None,
        category: None,
        description: None,
        queue_only: false,
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: Some("ftp".to_owned()),
        sabr: None,
        reuse_active: None,
        referer: None,
        youtube_format: None,
        conflict_policy: None,
        credentials: None,
        completion_actions: None,
    };

    let ids = service.add(req, &settings).unwrap();
    let status = wait_for_terminal_status(&history, &ids[0]).await;

    assert_eq!(status, DownloadStatus::Completed, "FTP transfer should complete with stored credentials");
    assert!(user_authenticated.load(Ordering::SeqCst), "FTP server must have received USER command with stored username");
    assert!(pass_authenticated.load(Ordering::SeqCst), "FTP server must have received PASS command with stored password");

    let downloaded = fs::read(dir.join("specs.pdf")).unwrap();
    assert_eq!(downloaded, ftp_payload);

    let _ = fs::remove_dir_all(dir);
}
