use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use awesome_downloader_lib::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
    },
    domain::{
        browser::{sanitize_browser_request, BrowserIncomingMessage},
        settings::Settings,
    },
    infrastructure::{
        bridge::LoopbackBridge,
        browser::{create_manifest, BrowserType, NATIVE_HOST_NAME},
        download_history_store::DownloadHistoryStore,
        http_download_executor::HttpDownloadExecutor,
    },
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn temporary_directory() -> PathBuf {
    let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "awesome-downloader-browser-test-{}-{id}",
        std::process::id()
    ))
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
    let dummy_exe = PathBuf::from("/usr/local/bin/awesome-downloader-native-host");
    let chrome_manifest = create_manifest(BrowserType::Chrome, "abcdefghijklmnopabcdefghijklmnop", &dummy_exe).unwrap();
    assert_eq!(chrome_manifest.name, NATIVE_HOST_NAME);
    assert_eq!(
        chrome_manifest.allowed_origins.unwrap(),
        vec!["chrome-extension://abcdefghijklmnopabcdefghijklmnop/".to_owned()]
    );
    assert!(chrome_manifest.allowed_extensions.is_none());

    let firefox_manifest = create_manifest(BrowserType::Firefox, "addon@awesome.downloader", &dummy_exe).unwrap();
    assert_eq!(firefox_manifest.name, NATIVE_HOST_NAME);
    assert_eq!(
        firefox_manifest.allowed_extensions.unwrap(),
        vec!["addon@awesome.downloader".to_owned()]
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
