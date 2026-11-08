// Created: 2026-11-05
// Reason: feat: add native messaging host, loopback bridge, and browser registration
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Description: Integration tests verifying browser integration functionality, edge cases, and error recovery.

use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::PathBuf,
};

use open_download_manager_lib::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        integration_service::IntegrationService,
        settings_service::SettingsRepository,
        shell_service::ShellService,
    },
    domain::{
        browser::{sanitize_browser_request, BrowserIncomingMessage, CapturePolicy, ConfirmationMode},
        category::DownloadCategory,
        download::{DownloadAction, DownloadRequest, DownloadStatus, ReviewDownloadRequest},
        settings::Settings,
    },
    infrastructure::{
        bridge::LoopbackBridge,
        browser::{
            browser_native_messaging_dir, create_manifest,
            scanner::{
                inspect_firefox_profile, inspect_firefox_profile_status, inspect_profile,
                FIREFOX_EXTENSION_ID,
            },
            BrowserType, NATIVE_HOST_NAME,
        },
        download_history_store::DownloadHistoryStore,
        http_download_executor::HttpDownloadExecutor,
        settings_store::SettingsStore,
    },
};

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("open-download-manager-browser-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

#[test]
fn browser_request_sanitizer_enforces_rules_and_preserves_whitelisted_headers() {
    let mut headers = HashMap::new();
    headers.insert("cookie".to_owned(), "auth=secret".to_owned());
    headers.insert("authorization".to_owned(), "Bearer token".to_owned());
    headers.insert("x-custom-tracking".to_owned(), "bad".to_owned());

    let msg = BrowserIncomingMessage {
        action: "download".to_owned(),
        download_id: None,
        url: Some("https://example.com/video.mp4".to_owned()),
        youtube_format: Some("137+140".to_owned()),
        sabr: None,
        start_now: Some(true),
        audio_url: None,
        output_format: Some("mp4".to_owned()),
        urls: None,
        filename: Some("my video.mp4".to_owned()),
        referer: Some("https://example.com/page".to_owned()),
        canonical_url: None,
        video_id: None,
        request_headers: Some(headers),
    };

    let sanitized = sanitize_browser_request(msg).unwrap();
    assert_eq!(sanitized.action, "download");
    assert_eq!(sanitized.url, "https://example.com/video.mp4");
    assert!(sanitized.start_now);
    assert_eq!(sanitized.output_format.as_deref(), Some("mp4"));
    assert_eq!(sanitized.filename.as_deref(), Some("my video.mp4"));
    assert_eq!(sanitized.referer.as_deref(), Some("https://example.com/page"));
    assert_eq!(sanitized.headers.get("cookie").map(String::as_str), Some("auth=secret"));
    assert_eq!(sanitized.headers.get("authorization").map(String::as_str), Some("Bearer token"));
    assert!(!sanitized.headers.contains_key("x-custom-tracking"));

    let webm_request = BrowserIncomingMessage {
        action: "download".to_owned(),
        url: Some("https://example.com/watch/1".to_owned()),
        output_format: Some("webm".to_owned()),
        ..BrowserIncomingMessage::default()
    };
    assert_eq!(
        sanitize_browser_request(webm_request)
            .unwrap()
            .output_format
            .as_deref(),
        Some("webm")
    );

    let generic_formats = BrowserIncomingMessage {
        action: "formats".to_owned(),
        download_id: None,
        url: Some("https://example.com/not-youtube".to_owned()),
        youtube_format: None,
        sabr: None,
        start_now: None,
        audio_url: None,
        output_format: None,
        urls: None,
        filename: None,
        referer: None,
        canonical_url: None,
        video_id: None,
        request_headers: None,
    };
    let sanitized_generic_formats = sanitize_browser_request(generic_formats).unwrap();
    assert_eq!(sanitized_generic_formats.url, "https://example.com/not-youtube");
    assert!(sanitized_generic_formats.canonical_url.is_none());

    let youtube_formats = BrowserIncomingMessage {
        action: "formats".to_owned(),
        download_id: None,
        url: Some("https://www.youtube.com/watch?v=wLdZWyLOJCI".to_owned()),
        youtube_format: None,
        sabr: None,
        start_now: None,
        audio_url: None,
        output_format: None,
        urls: None,
        filename: None,
        referer: None,
        canonical_url: None,
        video_id: None,
        request_headers: None,
    };
    let sanitized_youtube_formats = sanitize_browser_request(youtube_formats).unwrap();
    assert_eq!(sanitized_youtube_formats.action, "formats");
    assert_eq!(
        sanitized_youtube_formats.url,
        "https://www.youtube.com/watch?v=wLdZWyLOJCI"
    );

    let invalid_formats = BrowserIncomingMessage {
        action: "formats".to_owned(),
        url: Some("file:///video".to_owned()),
        ..BrowserIncomingMessage::default()
    };
    assert!(sanitize_browser_request(invalid_formats).is_err());
}

#[test]
fn native_host_manifest_structures_match_browser_specifications() {
    let dummy_exe = PathBuf::from("/usr/local/bin/open-download-manager-native-host");
    let chrome_manifest = create_manifest(BrowserType::Chrome, "abcdefghijklmnopabcdefghijklmnop", &dummy_exe).unwrap();
    assert_eq!(chrome_manifest.name, NATIVE_HOST_NAME);
    assert!(
        chrome_manifest
            .allowed_origins
            .unwrap()
            .contains(&"chrome-extension://abcdefghijklmnopabcdefghijklmnop/".to_owned())
    );
    assert!(chrome_manifest.allowed_extensions.is_none());

    let firefox_manifest = create_manifest(BrowserType::Firefox, "addon@open.downloadmanager", &dummy_exe).unwrap();
    assert_eq!(firefox_manifest.name, NATIVE_HOST_NAME);
    assert_eq!(
        firefox_manifest.allowed_extensions.unwrap(),
        vec!["addon@open.downloadmanager".to_owned()]
    );
    assert!(firefox_manifest.allowed_origins.is_none());

    // Invalid extension ID checks
    assert!(create_manifest(BrowserType::Chrome, "invalid-short-id", &dummy_exe).is_err());
    assert!(create_manifest(BrowserType::Firefox, "", &dummy_exe).is_err());
}

#[test]
fn test_browser_native_messaging_directories_resolve() {
    let home = PathBuf::from("/mock/home/user");
    let chrome_dir = browser_native_messaging_dir(BrowserType::Chrome, &home);
    let edge_dir = browser_native_messaging_dir(BrowserType::Edge, &home);
    let brave_dir = browser_native_messaging_dir(BrowserType::Brave, &home);
    let opera_dir = browser_native_messaging_dir(BrowserType::Opera, &home);
    let firefox_dir = browser_native_messaging_dir(BrowserType::Firefox, &home);

    #[cfg(target_os = "macos")]
    {
        assert_eq!(chrome_dir, home.join("Library/Application Support/Google/Chrome/NativeMessagingHosts"));
        assert_eq!(edge_dir, home.join("Library/Application Support/Microsoft Edge/NativeMessagingHosts"));
        assert_eq!(brave_dir, home.join("Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts"));
        assert_eq!(opera_dir, home.join("Library/Application Support/com.operasoftware.Opera/NativeMessagingHosts"));
        assert_eq!(firefox_dir, home.join("Library/Application Support/Mozilla/NativeMessagingHosts"));
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        assert_eq!(chrome_dir, home.join(".config/google-chrome/NativeMessagingHosts"));
        assert_eq!(edge_dir, home.join(".config/microsoft-edge/NativeMessagingHosts"));
        assert_eq!(brave_dir, home.join(".config/BraveSoftware/Brave-Browser/NativeMessagingHosts"));
        assert_eq!(opera_dir, home.join(".config/opera/NativeMessagingHosts"));
        assert_eq!(firefox_dir, home.join(".mozilla/native-messaging-hosts"));
    }
}

#[tokio::test]
async fn loopback_bridge_authenticates_and_handles_browser_downloads() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("create directory");

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service");
    let executor = HttpDownloadExecutor::new().expect("http executor");
    let service = DownloadTransferService::new(history.clone(), executor, 2);
    let settings = Settings::new(directory.clone());

    let captured_id = std::sync::Arc::new(std::sync::Mutex::new(None));
    let captured_id_clone = captured_id.clone();
    let on_download = std::sync::Arc::new(move |id: String| {
        *captured_id_clone.lock().unwrap() = Some(id);
    });

    let bridge = LoopbackBridge::start(directory.clone(), service, settings, Some(on_download))
        .await
        .expect("bridge start");

    let port = bridge.config.port;
    let token = bridge.config.token.clone();

    let client = reqwest::Client::new();
    let base_url = format!("http://127.0.0.1:{port}");

    // 1. Origin header rejected with 403
    let forbidden_resp = client
        .get(format!("{base_url}/health"))
        .header("Origin", "https://evil.test")
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden_resp.status().as_u16(), 403);

    // 2. Missing or bad token rejected with 401
    let unauth_resp = client
        .get(format!("{base_url}/health"))
        .header("Authorization", "Bearer invalid-token")
        .send()
        .await
        .unwrap();
    assert_eq!(unauth_resp.status().as_u16(), 401);

    // 3. Valid /health check
    let health_resp = client
        .get(format!("{base_url}/health"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(health_resp.status().as_u16(), 200);
    let health_bytes = health_resp.bytes().await.unwrap();
    let health_json: serde_json::Value = serde_json::from_slice(&health_bytes).unwrap();
    assert_eq!(health_json["ok"], true);

    // 4. POST /downloads accepts valid payload
    let download_payload = serde_json::json!({
        "action": "download",
        "url": "https://example.com/test.bin",
        "startNow": false
    });
    let download_resp = client
        .post(format!("{base_url}/downloads"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(serde_json::to_vec(&download_payload).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(download_resp.status().as_u16(), 201);
    let download_bytes = download_resp.bytes().await.unwrap();
    let download_json: serde_json::Value = serde_json::from_slice(&download_bytes).unwrap();
    assert_eq!(download_json["ok"], true);
    let returned_id = download_json["ids"][0].as_str().unwrap().to_string();
    assert_eq!(*captured_id.lock().unwrap(), Some(returned_id));
    assert!(!download_json["ids"].as_array().unwrap().is_empty());

    // Verify record in history
    let downloads = history.downloads().unwrap();
    assert_eq!(downloads.len(), 1);
    assert_eq!(downloads[0].url, "https://example.com/test.bin");

    // Stop bridge and check bridge.json cleanup
    bridge.stop().await;
    assert!(!directory.join("bridge.json").exists());

    fs::remove_dir_all(directory).expect("cleanup directory");
}

struct DummyShellService;
impl ShellService for DummyShellService {
    fn open_path(&self, _path: &std::path::Path) -> open_download_manager_lib::application::error::AppResult<()> {
        Ok(())
    }
    fn reveal_path(&self, _path: &std::path::Path) -> open_download_manager_lib::application::error::AppResult<()> {
        Ok(())
    }
    fn open_with(&self, _path: &std::path::Path) -> open_download_manager_lib::application::error::AppResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn browser_download_review_lifecycle_and_validation() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("create directory");

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service");
    let executor = HttpDownloadExecutor::new().expect("http executor");
    let service = DownloadTransferService::new(history.clone(), executor, 2);
    let mut settings = Settings::new(directory.clone());
    settings.confirm_browser_downloads = true;

    // 1. Add download with review_only = true (simulate browser interception)
    let request = DownloadRequest {
        urls: vec!["https://example.com/unconfirmed.iso".to_owned()],
        filename: None,
        directory: None,
        temp_directory: None,
        category: None,
        description: Some("From browser".to_owned()),
        queue_only: false,
        review_only: Some(true),
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

    let ids = service.add(request, &settings).unwrap();
    assert_eq!(ids.len(), 1);
    let id = ids[0].clone();

    // Verify initial state is Paused and needs_review is true
    let d = history.downloads().unwrap().into_iter().find(|item| item.id == id).unwrap();
    assert!(d.needs_review, "Download must have needs_review flag set");
    assert_eq!(d.status, DownloadStatus::Paused);

    // 2. Attempt to resume directly without reviewing must fail
    let resume_err = service.action(&id, DownloadAction::Resume).unwrap_err();
    assert!(
        resume_err.message.contains("Review this browser download before starting it."),
        "Unreviewed download cannot be resumed"
    );

    // 3. Validation errors during review:
    // Relative directory rejected
    let relative_rev = ReviewDownloadRequest {
        id: id.clone(),
        filename: None,
        directory: Some("relative/path".to_owned()),
        temp_directory: None,
        category: None,
        description: None,
        queue_only: None,
        scheduled_at: None,
        conflict_policy: None,
        credentials: None,
    };
    let err = service.review_download(relative_rev).unwrap_err();
    assert!(err.message.contains("Choose an absolute save folder."));

    // Past schedule rejected
    let past_rev = ReviewDownloadRequest {
        id: id.clone(),
        filename: None,
        directory: None,
        temp_directory: None,
        category: None,
        description: None,
        queue_only: None,
        scheduled_at: Some("2020-01-01T00:00:00Z".to_owned()),
        conflict_policy: None,
        credentials: None,
    };
    let err = service.review_download(past_rev).unwrap_err();
    assert!(err.message.contains("The scheduled time must be in the future."));

    // Description too long (> 4096)
    let overlong_rev = ReviewDownloadRequest {
        id: id.clone(),
        filename: None,
        directory: None,
        temp_directory: None,
        category: None,
        description: Some("x".repeat(5000)),
        queue_only: None,
        scheduled_at: None,
        conflict_policy: None,
        credentials: None,
    };
    let err = service.review_download(overlong_rev).unwrap_err();
    assert!(err.message.contains("Description is too long."));

    // 4. Valid confirmation review
    let valid_rev = ReviewDownloadRequest {
        id: id.clone(),
        filename: Some("confirmed_image.iso".to_owned()),
        directory: Some(directory.to_string_lossy().into_owned()),
        temp_directory: None,
        category: Some(DownloadCategory::Archives),
        description: Some("User confirmed".to_owned()),
        queue_only: Some(false),
        scheduled_at: None,
        conflict_policy: None,
        credentials: None,
    };

    let confirmed_ids = service.review_download(valid_rev).unwrap();
    assert_eq!(confirmed_ids, vec![id.clone()]);

    // Verify record state updated
    let updated = history.downloads().unwrap().into_iter().find(|item| item.id == id).unwrap();
    assert!(!updated.needs_review, "needs_review must be cleared after review");
    assert_eq!(updated.filename, "confirmed_image.iso");
    assert_eq!(updated.category, Some(DownloadCategory::Archives));
    assert_eq!(updated.description.as_deref(), Some("User confirmed"));

    // 5. Subsequent review attempt must fail because it's no longer waiting for confirmation
    let duplicate_rev = ReviewDownloadRequest {
        id: id.clone(),
        filename: None,
        directory: None,
        temp_directory: None,
        category: None,
        description: None,
        queue_only: None,
        scheduled_at: None,
        conflict_policy: None,
        credentials: None,
    };
    let dup_err = service.review_download(duplicate_rev).unwrap_err();
    assert!(dup_err.message.contains("This download is not waiting for confirmation."));

    fs::remove_dir_all(directory).expect("cleanup directory");
}

#[tokio::test]
async fn browser_review_cancellation_and_download_later() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("create directory");

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service");
    let executor = HttpDownloadExecutor::new().expect("http executor");
    let service = DownloadTransferService::new(history.clone(), executor, 2);
    let settings = Settings::new(directory.clone());

    // 1. Cancel unreviewed download
    let req1 = DownloadRequest {
        urls: vec!["https://example.com/cancel-me.bin".to_owned()],
        filename: None,
        directory: None,
        temp_directory: None,
        category: None,
        description: None,
        queue_only: false,
        review_only: Some(true),
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
    let ids1 = service.add(req1, &settings).unwrap();
    let id1 = &ids1[0];

    service.action(id1, DownloadAction::Cancel).unwrap();
    let record1 = history.downloads().unwrap().into_iter().find(|d| &d.id == id1).unwrap();
    assert_eq!(record1.status, DownloadStatus::Cancelled);
    assert!(!record1.needs_review, "needs_review must be reset on cancel");

    // 2. Review with Download Later (queue_only = true)
    let req2 = DownloadRequest {
        urls: vec!["https://example.com/later.bin".to_owned()],
        filename: None,
        directory: None,
        temp_directory: None,
        category: None,
        description: None,
        queue_only: false,
        review_only: Some(true),
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
    let ids2 = service.add(req2, &settings).unwrap();
    let id2 = &ids2[0];

    let rev_later = ReviewDownloadRequest {
        id: id2.clone(),
        filename: Some("later_saved.bin".to_owned()),
        directory: None,
        temp_directory: None,
        category: None,
        description: None,
        queue_only: Some(true),
        scheduled_at: None,
        conflict_policy: None,
        credentials: None,
    };
    service.review_download(rev_later).unwrap();
    let record2 = history.downloads().unwrap().into_iter().find(|d| &d.id == id2).unwrap();
    assert_eq!(record2.status, DownloadStatus::Paused);
    assert!(!record2.needs_review);

    fs::remove_dir_all(directory).expect("cleanup directory");
}

#[test]
fn profile_scanner_accurately_classifies_extension_states() {
    let temp_root = temporary_directory();
    fs::create_dir_all(&temp_root).expect("create directory");
    let test_id = "abcdefghijklmnopabcdefghijklmnop";

    // 1. Profile with active connected extension
    let connected_profile = temp_root.join("ConnectedProfile");
    fs::create_dir_all(&connected_profile).unwrap();
    let connected_prefs = serde_json::json!({
        "extensions": {
            "settings": {
                test_id: {
                    "state": 1,
                    "manifest": {
                        "name": "Open Download Manager Integration",
                        "version": "1.0.0"
                    }
                }
            }
        }
    });
    fs::write(connected_profile.join("Preferences"), connected_prefs.to_string()).unwrap();
    let (conn, state) = inspect_profile(&connected_profile);
    assert!(conn);
    assert_eq!(state, "connected");

    // 2. Profile with extension awaiting approval (reason 8192)
    let approval_profile = temp_root.join("ApprovalProfile");
    fs::create_dir_all(&approval_profile).unwrap();
    let approval_prefs = serde_json::json!({
        "extensions": {
            "settings": {
                test_id: {
                    "disable_reasons": [8192],
                    "manifest": {
                        "name": "Open Download Manager Integration",
                        "version": "1.0.0"
                    }
                }
            }
        }
    });
    fs::write(approval_profile.join("Preferences"), approval_prefs.to_string()).unwrap();
    let (conn, state) = inspect_profile(&approval_profile);
    assert!(!conn);
    assert_eq!(state, "approval");

    // 3. Profile with disabled extension
    let disabled_profile = temp_root.join("DisabledProfile");
    fs::create_dir_all(&disabled_profile).unwrap();
    let disabled_prefs = serde_json::json!({
        "extensions": {
            "settings": {
                test_id: {
                    "state": 0,
                    "disable_reasons": [1],
                    "manifest": {
                        "name": "Open Download Manager Integration",
                        "version": "1.0.0"
                    }
                }
            }
        }
    });
    fs::write(disabled_profile.join("Preferences"), disabled_prefs.to_string()).unwrap();
    let (conn, state) = inspect_profile(&disabled_profile);
    assert!(!conn);
    assert_eq!(state, "disabled");

    // 4. Empty profile (missing)
    let empty_profile = temp_root.join("EmptyProfile");
    fs::create_dir_all(&empty_profile).unwrap();
    let (conn, state) = inspect_profile(&empty_profile);
    assert!(!conn);
    assert_eq!(state, "missing");

    // 5. Firefox profile with extensions.json (connected)
    let firefox_profile = temp_root.join("FirefoxProfile");
    fs::create_dir_all(&firefox_profile).unwrap();
    let firefox_addons = serde_json::json!({
        "addons": [
            {
                "id": FIREFOX_EXTENSION_ID,
                "active": true,
                "appDisabled": false
            }
        ]
    });
    fs::write(firefox_profile.join("extensions.json"), firefox_addons.to_string()).unwrap();
    assert!(inspect_firefox_profile(&firefox_profile));
    let (conn, state) = inspect_firefox_profile_status(&firefox_profile);
    assert!(conn);
    assert_eq!(state, "connected");

    // 6. Firefox profile with disabled extension
    let firefox_disabled = temp_root.join("FirefoxDisabledProfile");
    fs::create_dir_all(&firefox_disabled).unwrap();
    let firefox_disabled_addons = serde_json::json!({
        "addons": [
            {
                "id": FIREFOX_EXTENSION_ID,
                "active": false,
                "appDisabled": false
            }
        ]
    });
    fs::write(firefox_disabled.join("extensions.json"), firefox_disabled_addons.to_string()).unwrap();
    let (conn_dis, state_dis) = inspect_firefox_profile_status(&firefox_disabled);
    assert!(!conn_dis);
    assert_eq!(state_dis, "disabled");

    fs::remove_dir_all(temp_root).expect("cleanup directory");
}

#[tokio::test]
async fn browser_setup_actions_and_firefox_manifest_preparation() {
    let temp_root = temporary_directory();
    let data_dir = temp_root.join("UserData");
    let integration_root = temp_root.join("Integration");
    let ext_dir = integration_root.join("extension");
    fs::create_dir_all(&ext_dir).unwrap();
    fs::write(ext_dir.join("manifest.json"), r#"{"name":"test"}"#).unwrap();
    fs::write(ext_dir.join("manifest.firefox.json"), r#"{"name":"test-firefox"}"#).unwrap();

    let shell = DummyShellService;
    let service = IntegrationService::with_shell(data_dir.clone(), integration_root.clone(), shell);

    // 1. Prepare Firefox extension
    let prep_res = service.browser_setup("prepare", "Firefox").await.unwrap();
    assert_eq!(prep_res.browser.as_deref(), Some("Firefox"));
    let prepared_path = PathBuf::from(prep_res.extension_path.unwrap());
    assert!(prepared_path.exists());
    let prepared_manifest = fs::read_to_string(prepared_path.join("manifest.json")).unwrap();
    assert!(prepared_manifest.contains("test-firefox"));

    // 2. Copy action returns path
    let copy_res = service.browser_setup("copy", "Chrome").await.unwrap();
    assert_eq!(copy_res.browser.as_deref(), Some("Chrome"));
    assert!(copy_res.extension_path.unwrap().contains("extension"));

    // 3. Store action rejected on Firefox
    let store_err = service.browser_setup("store", "Firefox").await.unwrap_err();
    assert!(store_err.message.contains("Store setup is unavailable for this browser."));

    // 4. Invalid browser rejected
    let bad_browser = service.browser_setup("copy", "Safari").await.unwrap_err();
    assert!(bad_browser.message.contains("Choose Chrome, Edge, Brave, Opera, or Firefox."));

    // 5. Invalid action rejected
    let bad_action = service.browser_setup("invalidAction", "Chrome").await.unwrap_err();
    assert!(bad_action.message.contains("Unknown browser setup action."));

    fs::remove_dir_all(temp_root).expect("cleanup directory");
}

fn serve_real_world_expiring_media_stream(full_file: Vec<u8>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("fixture listener bind");
    let address = listener.local_addr().expect("fixture address");
    std::thread::spawn(move || {
        // Request 1: Initial request with initial token -> sends first 64KB, then drops connection
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"real-video-v1\"\r\nConnection: close\r\n\r\n",
                full_file.len()
            );
            let _ = stream.write_all(&full_file[..64 * 1024]);
        }

        // Request 2: Retry with the OLD expired token -> returns 403 Forbidden
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let req_str = String::from_utf8_lossy(&buf);
            if req_str.contains("expired_token") {
                let _ = write!(
                    stream,
                    "HTTP/1.1 403 Forbidden\r\nContent-Length: 15\r\nConnection: close\r\n\r\nAccess Expired!"
                );
            }
        }

        // Request 3: Fresh request with fresh_token from browser refresh -> Range request accepted!
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);
            let offset = req_str
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("range")
                        .then(|| {
                            value
                                .trim()
                                .strip_prefix("bytes=")?
                                .trim_end_matches('-')
                                .parse::<usize>()
                                .ok()
                        })
                        .flatten()
                })
                .unwrap_or(0);

            if offset > 0 {
                let _ = write!(
                    stream,
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"real-video-v1\"\r\nConnection: close\r\n\r\n",
                    offset,
                    full_file.len() - 1,
                    full_file.len(),
                    full_file.len() - offset
                );
                let _ = stream.write_all(&full_file[offset..]);
            } else {
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"real-video-v1\"\r\nConnection: close\r\n\r\n",
                    full_file.len()
                );
                let _ = stream.write_all(&full_file);
            }
        }
    });
    format!("http://{address}")
}

#[tokio::test]
async fn real_user_scenario_browser_link_expired_and_refreshed_through_bridge() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("create directory");

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service");
    let executor = HttpDownloadExecutor::new().expect("http executor");
    let service = DownloadTransferService::new(history.clone(), executor, 2);
    let mut settings = Settings::new(directory.clone());
    settings.connections = 1;

    let bridge = LoopbackBridge::start(directory.clone(), service, settings, None)
        .await
        .expect("bridge start");

    let port = bridge.config.port;
    let token = bridge.config.token.clone();
    let client = reqwest::Client::new();
    let bridge_url = format!("http://127.0.0.1:{port}");

    // Mock media stream: 256 KB file
    let full_file = (0..256 * 1024)
        .map(|i| (i % 251) as u8)
        .collect::<Vec<_>>();
    let stream_server = serve_real_world_expiring_media_stream(full_file.clone());

    // 1. Real browser user initiates download of a video stream with a temporary token
    let initial_payload = serde_json::json!({
        "action": "download",
        "url": format!("{stream_server}/video.mp4?token=expired_token"),
        "filename": "vacation_video.mp4",
        "referer": "https://video.example.com/watch/101",
        "startNow": true
    });

    let resp = client
        .post(format!("{bridge_url}/downloads"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(serde_json::to_vec(&initial_payload).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
    let resp_json: serde_json::Value = resp.json().await.unwrap();
    let download_id = resp_json["ids"][0].as_str().unwrap().to_string();

    // 2. The transfer starts, downloads 64KB, then disconnects. Reconnect with expired token fails with 403.
    // The download status transitions to Failed with expired error message.
    let mut final_status = DownloadStatus::Connecting;
    for _ in 0..100 {
        if let Ok(d) = history.download(&download_id) {
            if d.status == DownloadStatus::Failed {
                final_status = d.status;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(final_status, DownloadStatus::Failed);

    let failed_rec = history.download(&download_id).unwrap();
    assert!(failed_rec.error.as_deref().unwrap_or("").contains("expired"));
    let download_dir = PathBuf::from(failed_rec.directory.as_ref().unwrap());

    // Verify verified partial bytes (64KB) are preserved on disk
    let partial_file = download_dir.join(format!(".open-download-manager-{download_id}.part"));
    assert!(partial_file.exists());
    assert_eq!(fs::metadata(&partial_file).unwrap().len(), 64 * 1024);

    // 3. User returns to the video page in browser and clicks Download (extension sends fresh URL with new token)
    let refresh_payload = serde_json::json!({
        "action": "download",
        "url": format!("{stream_server}/video.mp4?token=fresh_token"),
        "filename": "vacation_video.mp4",
        "referer": "https://video.example.com/watch/101",
        "startNow": true
    });

    let refresh_resp = client
        .post(format!("{bridge_url}/downloads"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(serde_json::to_vec(&refresh_payload).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(refresh_resp.status().as_u16(), 201);
    let refresh_json: serde_json::Value = refresh_resp.json().await.unwrap();
    // Must reuse the exact same download record
    assert_eq!(refresh_json["ids"][0].as_str().unwrap(), download_id);

    // 4. Download resumes from 64KB using Range request, preserves bytes, and finishes!
    let mut completed = false;
    for _ in 0..100 {
        if let Ok(d) = history.download(&download_id) {
            if d.status == DownloadStatus::Completed {
                completed = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(completed, "Download should complete successfully after refresh");

    // 5. Verify full file integrity on disk (every single byte matches!)
    let dest_file = download_dir.join("vacation_video.mp4");
    assert!(dest_file.exists());
    let saved_bytes = fs::read(&dest_file).unwrap();
    assert_eq!(saved_bytes.len(), full_file.len());
    assert_eq!(saved_bytes, full_file);

    // 6. Verify explanation notice was attached
    let record = history.download(&download_id).unwrap();
    assert!(record.notice.as_deref().unwrap_or("").contains("refreshed"));

    bridge.stop().await;
    fs::remove_dir_all(directory).expect("cleanup directory");
}

#[tokio::test]
async fn loopback_bridge_capture_policy_and_exclusions() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("create directory");

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service");
    let executor = HttpDownloadExecutor::new().expect("http executor");
    let service = DownloadTransferService::new(history.clone(), executor, 2);
    let mut settings = Settings::new(directory.clone());
    settings.confirm_browser_downloads = false;
    settings.capture_policy = CapturePolicy {
        file_types: vec!["zip".to_owned(), "tar.gz".to_owned()],
        excluded_hosts: vec!["blocked.com".to_owned(), "*.internal.corp".to_owned()],
        excluded_addresses: vec!["192.168.1.100".to_owned(), "10.0.0.0/8".to_owned()],
        confirmation_mode: ConfirmationMode::AskForUnknown,
        enabled: true,
    };

    let bridge = LoopbackBridge::start(directory.clone(), service, settings, None)
        .await
        .expect("bridge start");

    let port = bridge.config.port;
    let token = bridge.config.token.clone();
    let client = reqwest::Client::new();
    let bridge_url = format!("http://127.0.0.1:{port}");

    // 1. GET /policy returns the active policy
    let policy_resp = client
        .get(format!("{bridge_url}/policy"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(policy_resp.status().as_u16(), 200);
    let policy_json: serde_json::Value = policy_resp.json().await.unwrap();
    assert_eq!(policy_json["ok"], true);
    assert_eq!(
        policy_json["policy"]["excludedHosts"][0].as_str().unwrap(),
        "blocked.com"
    );
    assert_eq!(
        policy_json["policy"]["confirmationMode"].as_str().unwrap(),
        "askForUnknown"
    );

    // 2. Excluded host returns 403 Forbidden with decision excluded-host
    let blocked_host_resp = client
        .post(format!("{bridge_url}/downloads"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(serde_json::to_vec(&serde_json::json!({
            "action": "download",
            "url": "https://blocked.com/somefile.zip",
            "startNow": true
        })).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(blocked_host_resp.status().as_u16(), 403);
    let blocked_host_json: serde_json::Value = blocked_host_resp.json().await.unwrap();
    assert_eq!(blocked_host_json["decision"], "excluded-host");
    assert!(blocked_host_json["error"].as_str().unwrap().contains("blocked.com"));

    // 3. Excluded address (CIDR / IP) returns 403 Forbidden with decision excluded-address
    let blocked_addr_resp = client
        .post(format!("{bridge_url}/downloads"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(serde_json::to_vec(&serde_json::json!({
            "action": "download",
            "url": "https://192.168.1.100/package.zip",
            "startNow": true
        })).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(blocked_addr_resp.status().as_u16(), 403);
    let blocked_addr_json: serde_json::Value = blocked_addr_resp.json().await.unwrap();
    assert_eq!(blocked_addr_json["decision"], "excluded-address");

    // 4. Unknown extension in AskForUnknown mode requires confirmation (needs_review = true)
    let unknown_ext_resp = client
        .post(format!("{bridge_url}/downloads"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(serde_json::to_vec(&serde_json::json!({
            "action": "download",
            "url": "https://allowed.example.com/installer.dmg",
            "startNow": true
        })).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(unknown_ext_resp.status().as_u16(), 201);
    let unknown_json: serde_json::Value = unknown_ext_resp.json().await.unwrap();
    let unk_id = unknown_json["ids"][0].as_str().unwrap();
    let unk_rec = history.download(unk_id).unwrap();
    assert!(unk_rec.needs_review, "Unknown extension in AskForUnknown mode must require review");

    // 5. Allowed extension in AskForUnknown mode does NOT require review when confirm_browser_downloads is false
    let allowed_ext_resp = client
        .post(format!("{bridge_url}/downloads"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(serde_json::to_vec(&serde_json::json!({
            "action": "download",
            "url": "https://allowed.example.com/archive.zip",
            "startNow": true
        })).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(allowed_ext_resp.status().as_u16(), 201);
    let allowed_json: serde_json::Value = allowed_ext_resp.json().await.unwrap();
    let allowed_id = allowed_json["ids"][0].as_str().unwrap();
    let allowed_rec = history.download(allowed_id).unwrap();
    assert!(!allowed_rec.needs_review, "Included extension should not force review");

    bridge.stop().await;
    fs::remove_dir_all(directory).expect("cleanup directory");
}

fn serve_test_static_file(body: Vec<u8>) -> (String, std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("fixture listener bind");
    let address = listener.local_addr().expect("fixture address");
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let running_clone = running.clone();
    std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        while running_clone.load(std::sync::atomic::Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(&body);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    (format!("http://{address}"), running)
}

#[tokio::test]
async fn real_user_scenario_browser_capture_rules_workflow_with_intranet_and_review() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("create directory");

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service");
    let executor = HttpDownloadExecutor::new().expect("http executor");
    let service = DownloadTransferService::new(history.clone(), executor, 2);
    let mut settings = Settings::new(directory.clone());
    settings.confirm_browser_downloads = false;
    settings.capture_policy = CapturePolicy {
        file_types: vec!["zip".to_owned(), "iso".to_owned()],
        excluded_hosts: vec!["*.internal.corp".to_owned(), "secret.bank.com".to_owned()],
        excluded_addresses: vec!["10.0.0.0/8".to_owned(), "/restricted/".to_owned()],
        confirmation_mode: ConfirmationMode::AskForUnknown,
        enabled: true,
    };

    // Save initial settings to store
    let settings_store = SettingsStore::new(&directory);
    settings_store.save(&settings).expect("save initial settings");

    let bridge = LoopbackBridge::start(directory.clone(), service.clone(), settings.clone(), None)
        .await
        .expect("bridge start");

    let port = bridge.config.port;
    let token = bridge.config.token.clone();
    let client = reqwest::Client::new();
    let bridge_url = format!("http://127.0.0.1:{port}");

    // Mock web server serving downloadable software files
    let sample_payload = b"GENUINE_USER_DOWNLOAD_PAYLOAD".to_vec();
    let (server_base, server_running) = serve_test_static_file(sample_payload.clone());

    // 1. Real human user visits a site and triggers an allowed file (e.g. tool.zip)
    let allowed_url = format!("{server_base}/tool.zip");
    let resp1 = client
        .post(format!("{bridge_url}/downloads"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(serde_json::to_vec(&serde_json::json!({
            "action": "download",
            "url": allowed_url,
            "filename": "tool.zip",
            "startNow": true
        })).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status().as_u16(), 201);
    let json1: serde_json::Value = resp1.json().await.unwrap();
    let id1 = json1["ids"][0].as_str().unwrap().to_string();

    // Verify it downloads and completes automatically without needing manual confirmation
    let mut completed1 = false;
    for _ in 0..100 {
        if let Ok(rec) = history.download(&id1) {
            if rec.status == DownloadStatus::Completed {
                completed1 = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(completed1, "Allowed standard file must complete automatically");
    let rec1 = history.download(&id1).unwrap();
    assert!(!rec1.needs_review);

    // 2. Real user encounters a company intranet link matching excluded_hosts
    let intranet_resp = client
        .post(format!("{bridge_url}/downloads"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(serde_json::to_vec(&serde_json::json!({
            "action": "download",
            "url": "https://dev.internal.corp/firmware.zip",
            "startNow": true
        })).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(intranet_resp.status().as_u16(), 403);
    let intranet_json: serde_json::Value = intranet_resp.json().await.unwrap();
    assert_eq!(intranet_json["decision"], "excluded-host");
    assert!(intranet_json["error"].as_str().unwrap().contains("dev.internal.corp"));

    // 3. Real user encounters a restricted path on an address exclusion list
    let restricted_resp = client
        .post(format!("{bridge_url}/downloads"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(serde_json::to_vec(&serde_json::json!({
            "action": "download",
            "url": format!("{server_base}/restricted/export.zip"),
            "startNow": true
        })).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(restricted_resp.status().as_u16(), 403);
    let restricted_json: serde_json::Value = restricted_resp.json().await.unwrap();
    assert_eq!(restricted_json["decision"], "excluded-address");

    // 4. Real user encounters an unrecognized file format (.xyz) in AskForUnknown mode
    let unknown_url = format!("{server_base}/installer.xyz");
    let resp_unk = client
        .post(format!("{bridge_url}/downloads"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(serde_json::to_vec(&serde_json::json!({
            "action": "download",
            "url": unknown_url,
            "filename": "installer.xyz",
            "startNow": true
        })).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(resp_unk.status().as_u16(), 201);
    let json_unk: serde_json::Value = resp_unk.json().await.unwrap();
    let id_unk = json_unk["ids"][0].as_str().unwrap().to_string();

    // Verify it is paused in needs_review state
    let rec_unk = history.download(&id_unk).unwrap();
    assert!(rec_unk.needs_review, "Unknown extension must trigger review dialog");
    assert_eq!(rec_unk.status, DownloadStatus::Paused);

    // User reviews the unknown file and explicitly clicks "Start Download"
    service
        .review_download(ReviewDownloadRequest {
            id: id_unk.clone(),
            filename: Some("installer.xyz".to_owned()),
            directory: Some(directory.to_string_lossy().into_owned()),
            temp_directory: None,
            category: None,
            description: Some("User confirmed unknown file".to_owned()),
            queue_only: Some(false),
            scheduled_at: None,
            conflict_policy: None,
            credentials: None,
        })
        .expect("user review approval");

    // The download proceeds and finishes
    let mut completed_unk = false;
    for _ in 0..100 {
        if let Ok(rec) = history.download(&id_unk) {
            if rec.status == DownloadStatus::Completed {
                completed_unk = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(completed_unk, "User-approved unknown download must complete successfully");

    // 5. User reconfigures settings dynamically: adds "xyz" to allowed file_types
    let mut updated_settings = settings.clone();
    updated_settings.capture_policy.file_types.push("xyz".to_owned());
    settings_store.save(&updated_settings).expect("save updated settings");

    // Extension refreshes policy from /policy endpoint
    let policy_poll = client
        .get(format!("{bridge_url}/policy"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let pol_json: serde_json::Value = policy_poll.json().await.unwrap();
    let types = pol_json["policy"]["fileTypes"].as_array().unwrap();
    assert!(types.iter().any(|t| t.as_str() == Some("xyz")), "Bridge must serve updated policy dynamically");

    // User now downloads another .xyz file -> now accepted directly without manual review
    let resp_xyz2 = client
        .post(format!("{bridge_url}/downloads"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(serde_json::to_vec(&serde_json::json!({
            "action": "download",
            "url": format!("{server_base}/package2.xyz"),
            "filename": "package2.xyz",
            "startNow": true
        })).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(resp_xyz2.status().as_u16(), 201);
    let json_xyz2: serde_json::Value = resp_xyz2.json().await.unwrap();
    let id_xyz2 = json_xyz2["ids"][0].as_str().unwrap().to_string();

    let rec_xyz2 = history.download(&id_xyz2).unwrap();
    assert!(!rec_xyz2.needs_review, "Newly whitelisted extension must not require review");

    server_running.store(false, std::sync::atomic::Ordering::Relaxed);
    bridge.stop().await;
    fs::remove_dir_all(directory).expect("cleanup directory");
}
