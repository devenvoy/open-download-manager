use std::path::PathBuf;

use awesome_downloader_lib::domain::download::{
    redact_download_url, sanitize_filename, DownloadProtocol, DownloadRequest,
};

fn request(url: &str) -> DownloadRequest {
    DownloadRequest {
        urls: vec![url.to_owned()],
        filename: None,
        directory: None,
        category: None,
        description: None,
        queue_only: false,
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        sabr: None,
        reuse_active: None,
    }
}

#[test]
fn accepts_http_and_https_addresses_and_uses_default_directory() {
    for (url, protocol) in [
        ("http://example.test/file.bin", DownloadProtocol::Http),
        ("https://example.test/file.bin", DownloadProtocol::Https),
    ] {
        let valid = request(url)
            .validate(PathBuf::from("/tmp/downloads"))
            .expect("HTTP and HTTPS should be supported");
        assert_eq!(valid.directory, PathBuf::from("/tmp/downloads"));
        assert_eq!(valid.urls.len(), 1);
        assert_eq!(valid.urls[0].protocol(), protocol);
    }
}

#[test]
fn rejects_unsupported_schemes_embedded_credentials_and_media_streams() {
    for url in [
        "file:///etc/passwd",
        "https://user:password@example.test/file.bin",
        "https://example.test/video.mpd",
        "https://youtube.com/watch?v=private",
    ] {
        assert!(
            request(url)
                .validate(PathBuf::from("/tmp/downloads"))
                .is_err(),
            "{url} should be rejected"
        );
    }
}

#[test]
fn accepts_hls_stream_addresses() {
    let valid = request("https://example.test/video.m3u8")
        .validate(PathBuf::from("/tmp/downloads"))
        .expect("HLS address should be accepted");
    assert_eq!(valid.urls[0].url().as_str(), "https://example.test/video.m3u8");
}

#[test]
fn rejects_relative_download_directories_and_invalid_or_past_schedules() {
    let mut invalid_directory = request("https://example.test/file.bin");
    invalid_directory.directory = Some("downloads".to_owned());
    assert!(invalid_directory
        .validate(PathBuf::from("/tmp/downloads"))
        .is_err());

    for scheduled_at in [
        "not-a-date".to_owned(),
        (chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339(),
    ] {
        let mut scheduled = request("https://example.test/file.bin");
        scheduled.scheduled_at = Some(scheduled_at);
        assert!(scheduled.validate(PathBuf::from("/tmp/downloads")).is_err());
    }
}

#[test]
fn accepts_a_future_rfc3339_schedule() {
    let scheduled_at = (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();
    let mut scheduled = request("https://example.test/file.bin");
    scheduled.scheduled_at = Some(scheduled_at.clone());
    let validated = scheduled
        .validate(PathBuf::from("/tmp/downloads"))
        .expect("future RFC3339 schedule should be accepted");
    assert_eq!(validated.scheduled_at, Some(scheduled_at));
}

#[test]
fn sanitizes_windows_and_path_separators_in_custom_names() {
    assert_eq!(sanitize_filename("../CON.txt"), ".._CON.txt");
    assert_eq!(
        sanitize_filename("folder\\report?.pdf"),
        "folder_report_.pdf"
    );
    assert_eq!(sanitize_filename("..."), "");
}

#[test]
fn redacts_credentials_queries_and_fragments_from_urls() {
    assert_eq!(
        redact_download_url("https://user:password@example.test/file?token=private#section"),
        "https://example.test/file"
    );
}

#[test]
fn malformed_url_is_not_exposed_in_a_snapshot() {
    assert_eq!(
        redact_download_url("https://user:password@bad host/file?token=private"),
        "Invalid URL"
    );
}
