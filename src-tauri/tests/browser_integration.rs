use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
};

use open_download_manager_lib::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        integration_service::IntegrationService,
        shell_service::ShellService,
    },
    domain::{
        browser::{sanitize_browser_request, BrowserIncomingMessage},
        category::DownloadCategory,
        download::{DownloadAction, DownloadRequest, DownloadStatus, ReviewDownloadRequest},
        settings::Settings,
    },
    infrastructure::{
        bridge::LoopbackBridge,
        browser::{
            create_manifest,
            scanner::{inspect_firefox_profile, inspect_profile, EXTENSION_ID, FIREFOX_EXTENSION_ID},
            BrowserType, NATIVE_HOST_NAME,
        },
        download_history_store::DownloadHistoryStore,
        http_download_executor::HttpDownloadExecutor,
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
        url: Some("https://example.com/video.mp4".to_owned()),
        youtube_format: Some("137+140".to_owned()),
        sabr: None,
        start_now: Some(true),
        audio_url: None,
        output_format: Some("mp4".to_owned()),
        urls: None,
        filename: Some("my video.mp4".to_owned()),
        referer: Some("https://example.com/page".to_owned()),
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

    // Formats action requires a YouTube watch URL
    let invalid_formats = BrowserIncomingMessage {
        action: "formats".to_owned(),
        url: Some("https://example.com/not-youtube".to_owned()),
        youtube_format: None,
        sabr: None,
        start_now: None,
        audio_url: None,
        output_format: None,
        urls: None,
        filename: None,
        referer: None,
        request_headers: None,
    };
    assert!(sanitize_browser_request(invalid_formats).is_err());

    let valid_formats = BrowserIncomingMessage {
        action: "formats".to_owned(),
        url: Some("https://www.youtube.com/watch?v=wLdZWyLOJCI".to_owned()),
        youtube_format: None,
        sabr: None,
        start_now: None,
        audio_url: None,
        output_format: None,
        urls: None,
        filename: None,
        referer: None,
        request_headers: None,
    };
    let sanitized_formats = sanitize_browser_request(valid_formats).unwrap();
    assert_eq!(sanitized_formats.action, "formats");
    assert_eq!(sanitized_formats.url, "https://www.youtube.com/watch?v=wLdZWyLOJCI");
}

#[test]
fn native_host_manifest_structures_match_browser_specifications() {
    let dummy_exe = PathBuf::from("/usr/local/bin/open-download-manager-native-host");
    let chrome_manifest = create_manifest(BrowserType::Chrome, "abcdefghijklmnopabcdefghijklmnop", &dummy_exe).unwrap();
    assert_eq!(chrome_manifest.name, NATIVE_HOST_NAME);
    assert_eq!(
        chrome_manifest.allowed_origins.unwrap(),
        vec!["chrome-extension://abcdefghijklmnopabcdefghijklmnop/".to_owned()]
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

#[tokio::test]
async fn loopback_bridge_authenticates_and_handles_browser_downloads() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("create directory");

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service");
    let executor = HttpDownloadExecutor::new().expect("http executor");
    let service = DownloadTransferService::new(history.clone(), executor, 2);
    let settings = Settings::new(directory.clone());

    let bridge = LoopbackBridge::start(directory.clone(), service, settings)
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
        category: None,
        description: Some("From browser".to_owned()),
        queue_only: false,
        review_only: Some(true),
        scheduled_at: None,
        audio_url: None,
        kind: None,
        sabr: None,
        reuse_active: None,
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
        category: None,
        description: None,
        queue_only: None,
        scheduled_at: None,
    };
    let err = service.review_download(relative_rev).unwrap_err();
    assert!(err.message.contains("Choose an absolute save folder."));

    // Past schedule rejected
    let past_rev = ReviewDownloadRequest {
        id: id.clone(),
        filename: None,
        directory: None,
        category: None,
        description: None,
        queue_only: None,
        scheduled_at: Some("2020-01-01T00:00:00Z".to_owned()),
    };
    let err = service.review_download(past_rev).unwrap_err();
    assert!(err.message.contains("The scheduled time must be in the future."));

    // Description too long (> 4096)
    let overlong_rev = ReviewDownloadRequest {
        id: id.clone(),
        filename: None,
        directory: None,
        category: None,
        description: Some("x".repeat(5000)),
        queue_only: None,
        scheduled_at: None,
    };
    let err = service.review_download(overlong_rev).unwrap_err();
    assert!(err.message.contains("Description is too long."));

    // 4. Valid confirmation review
    let valid_rev = ReviewDownloadRequest {
        id: id.clone(),
        filename: Some("confirmed_image.iso".to_owned()),
        directory: Some(directory.to_string_lossy().into_owned()),
        category: Some(DownloadCategory::Archives),
        description: Some("User confirmed".to_owned()),
        queue_only: Some(false),
        scheduled_at: None,
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
        category: None,
        description: None,
        queue_only: None,
        scheduled_at: None,
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
        category: None,
        description: None,
        queue_only: false,
        review_only: Some(true),
        scheduled_at: None,
        audio_url: None,
        kind: None,
        sabr: None,
        reuse_active: None,
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
        category: None,
        description: None,
        queue_only: false,
        review_only: Some(true),
        scheduled_at: None,
        audio_url: None,
        kind: None,
        sabr: None,
        reuse_active: None,
    };
    let ids2 = service.add(req2, &settings).unwrap();
    let id2 = &ids2[0];

    let rev_later = ReviewDownloadRequest {
        id: id2.clone(),
        filename: Some("later_saved.bin".to_owned()),
        directory: None,
        category: None,
        description: None,
        queue_only: Some(true),
        scheduled_at: None,
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

    // 1. Profile with active connected extension
    let connected_profile = temp_root.join("ConnectedProfile");
    fs::create_dir_all(&connected_profile).unwrap();
    let connected_prefs = serde_json::json!({
        "extensions": {
            "settings": {
                EXTENSION_ID: {
                    "state": 1,
                    "manifest": { "version": "1.0.0" }
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
                EXTENSION_ID: {
                    "disable_reasons": [8192],
                    "manifest": { "version": "1.0.0" }
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
                EXTENSION_ID: {
                    "state": 0,
                    "disable_reasons": [1]
                }
            }
        }
    });
    fs::write(disabled_profile.join("Preferences"), disabled_prefs.to_string()).unwrap();
    let (conn, state) = inspect_profile(&disabled_profile);
    assert!(!conn);
    assert_eq!(state, "disabled");

    // 4. Profile with external uninstalls (removed)
    let removed_profile = temp_root.join("RemovedProfile");
    fs::create_dir_all(&removed_profile).unwrap();
    let removed_prefs = serde_json::json!({
        "extensions": {
            "settings": {},
            "external_uninstalls": [EXTENSION_ID]
        }
    });
    fs::write(removed_profile.join("Preferences"), removed_prefs.to_string()).unwrap();
    let (conn, state) = inspect_profile(&removed_profile);
    assert!(!conn);
    assert_eq!(state, "removed");

    // 5. Empty profile (missing)
    let empty_profile = temp_root.join("EmptyProfile");
    fs::create_dir_all(&empty_profile).unwrap();
    let (conn, state) = inspect_profile(&empty_profile);
    assert!(!conn);
    assert_eq!(state, "missing");

    // 6. Firefox profile with extensions.json
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
